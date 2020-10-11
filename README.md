# apns-notifyd

This is an external notifier for Cyrus to push notifications to Apple Push Notification services.

## Usage

The following environment vairables are required:

- `APNS_NOTIFYD_IDENT_PATH` - Path to PEM encoded certificate and key for APNS authentication.

- `APNS_NOTIFYD_DB_PATH` - Path to a directory where `apns-notifyd` will store its internal database.

- `APNS_NOTIFYD_TOPIC` - APNS notification topic, can be obtained from the X.509 subject of the push notification certificate.

## License

Copyright (c) 2020 Umang Raghuvanshi.

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published
by the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU Affero General Public License for more details.

A copy of the license is available in `LICENSE` for your reference.

