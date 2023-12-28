FROM rust:buster as builder
WORKDIR /usr/src/rodosol-telegram-bot
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update 
RUN apt-get -y install openssl ca-certificates
RUN rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/rodosol-telegram-bot /usr/local/bin/rodosol-telegram-bot
USER nobody 
ENTRYPOINT ["rodosol-telegram-bot"]
