FROM rust:buster as builder
WORKDIR /usr/src/ipcamera_bot
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update 
RUN apt-get -y install openssl ca-certificates
RUN rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/ipcamera_bot /usr/local/bin/ipcamera_bot
ENTRYPOINT ["ipcamera_bot"]
