# xyncam-bot

#### [Add this bot to your contacts list](https://t.me/xyncam_bot)

This bot gets a picture from Xyn through an RTSP protocol and sends it to the desired telegram channel.

## Usage

Add the @xyncam_bot to your chat and use the following commands:

- `/xyn_now`: sends pictures from Xyn to the channel 

You may also send these commands directly to the bot instead of adding it to a chat.

## Running it locally

Add a `.env` file with the `TELEGRAM_BOT_TOKEN` defined and run `cargo run`.

Or use `docker-compose up` or `docker run` manually with the `Dockerfile` provided.

You should be all set.
