FROM rust:buster as builder
WORKDIR /usr/src/xyncam-bot
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update 
RUN apt-get -y install openssl ca-certificates
RUN rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/xyncam-bot /usr/local/bin/xyncam-bot
USER nobody 
ENTRYPOINT ["xyncam-bot"]
