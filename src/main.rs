//! apns_notifyd, external notifier to Apple Push Notification Service
//! for cyrus-imapd.

// Copyright (C) 2020 Umang Raghuvanshi

//  This program is free software: you can redistribute it and/or modify
//  it under the terms of the GNU Affero General Public License as published
//  by the Free Software Foundation, either version 3 of the License, or
//  (at your option) any later version.

//  This program is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//  GNU Affero General Public License for more details.

//  You should have received a copy of the GNU Affero General Public License
//  along with this program.  If not, see <https://www.gnu.org/licenses/>.

#[macro_use]
extern crate log;

use anyhow::{anyhow, Context, Result};
use serde_json::{Map, Value};
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<()> {
    syslog::init(
        syslog::Facility::LOG_USER,
        log::LevelFilter::Info,
        Some("apns_notifyd"),
    )
    .map_err(|err| anyhow!("Failed to init logging: {}", err))?;

    let db_path = std::env::var("APNS_NOTIFYD_DB_PATH")
        .context("Could not get database path from APNS_NOTIFYD_DB_PATH")?;
    let db = sled::open(db_path)?;

    info!("apns_notifyd started");

    let mut stdin = tokio::io::stdin();
    let mut payload = Vec::<u8>::new();
    let mut buf = [0u8; 4096];
    let mut num_read = stdin.read(&mut buf).await?;

    while num_read != 0 {
        payload.extend_from_slice(&buf[0..num_read]);
        num_read = stdin.read(&mut buf).await?;
    }

    let payload_json =
        serde_json::from_slice(payload.as_slice()).context("Failed to parse input JSON")?;

    if let Value::Object(message) = payload_json {
        match message.get("event") {
            Some(Value::String(event)) => match event.as_str() {
                "ApplePushService" => {
                    register(message, &db).context("Device registration failed")?
                }
                "MessageNew" => handle_message(message, &db)
                    .await
                    .context("Failed to push notification")?,
                _ => return Err(anyhow!("Unsupported push event {} ({:?})", event, message)),
            },
            _ => return Err(anyhow!("Invalid input")),
        }
    } else {
        error!("Invalid input");
        std::process::exit(-1);
    }

    Ok(())
}

fn register(registration_info: Map<String, Value>, db: &sled::Db) -> Result<()> {
    let err = RegistrationError();

    let account_id = registration_info
        .get("apsAccountId")
        .ok_or(err)?
        .as_str()
        .ok_or(err)?;
    let device_token = registration_info
        .get("apsDeviceToken")
        .ok_or(err)?
        .as_str()
        .ok_or(err)?;
    let user = registration_info
        .get("user")
        .ok_or(err)?
        .as_str()
        .ok_or(err)?;

    db.insert("device_".to_owned() + device_token, account_id)?;

    db.fetch_and_update(user, |current_devices| {
        let current_devices = match current_devices {
            Some(bytes) => std::str::from_utf8(bytes).expect("Database corruption"),
            None => "",
        };

        Some(format!("{},{}", device_token, current_devices).into_bytes())
    })?;

    info!(
        "Registered device with token {} for push notifications",
        device_token
    );

    Ok(())
}

async fn handle_message(message: Map<String, Value>, db: &sled::Db) -> Result<()> {
    let identity_path = std::env::var("APNS_NOTIFYD_IDENT_PATH")
        .context("Could not get APNS identity keypair from APNS_NOTIFYD_IDENT_PATH")?;
    let identity_bytes = tokio::fs::read_to_string(identity_path).await?;
    let identity = reqwest::Identity::from_pem(identity_bytes.as_bytes())?;
    let topic =
        std::env::var("APNS_NOTIFYD_TOPIC").context("Could not get APNS notification topic")?;
    let err = PushError();
    let expiry = std::time::SystemTime::now()
        .checked_add(std::time::Duration::new(24 * 60 * 60, 0))
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let user = message.get("user").ok_or(err)?.as_str().ok_or(err)?;
    let devices = match db.get(user) {
        Ok(ivec) => match ivec {
            Some(device_bytes) => String::from_utf8(device_bytes.to_vec()).unwrap(),
            _ => return Ok(()),
        },
        Err(e) => return Err(e.into()),
    };

    let client = reqwest::ClientBuilder::new()
        .identity(identity)
        .http2_prior_knowledge()
        .build()?;

    for device in devices.split(',').into_iter().filter(|d| d.len() > 0) {
        let payload = serde_json::json!({
            "aps": {
                "account-id": std::str::from_utf8(&db.get("device_".to_owned() + device)
                .expect("Inconsistent DB state - missing account ID").unwrap())
                .unwrap()
            }
        });

        let url = format!("https://api.push.apple.com/3/device/{}", device);

        let repsonse = client
            .post(&url)
            .header("apns-push-type", "alert")
            .header("apns-expiration", &expiry)
            .header("apns-priority", "10")
            .header("apns-topic", &topic)
            .body(serde_json::to_string(&payload).unwrap())
            .send()
            .await?;

        let status = repsonse.status();
        debug!("APNS response: code {}(device: {})", status, device);

        if status != 200 {
            error!("APNS error: HTTP status {} for device {}", status, device);
        }
    }

    info!(
        "Pushed message {} to APNS",
        message.get("uri").unwrap().as_str().unwrap()
    );

    Ok(())
}

#[derive(Copy, Clone, Debug)]
struct PushError();
impl std::fmt::Display for PushError {
    fn fmt(&self, w: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(w, "Malformed push request")
    }
}
impl std::error::Error for PushError {}

#[derive(Copy, Clone, Debug)]
struct RegistrationError();
impl std::fmt::Display for RegistrationError {
    fn fmt(&self, w: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(w, "Malformed registration request")
    }
}
impl std::error::Error for RegistrationError {}
