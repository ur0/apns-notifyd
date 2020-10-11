FROM rust:1.47.0 as builder
LABEL maintainer="u@umangis.me"
WORKDIR /build

COPY Cargo.toml /build
COPY Cargo.lock /build
RUN mkdir /build/src && touch /build/src/lib.rs
RUN cargo build --release && rm /build/src/lib.rs

COPY . /build
RUN cargo build --release

FROM debian:buster-slim
COPY --from=builder /build/target/release/apns_notifyd /
