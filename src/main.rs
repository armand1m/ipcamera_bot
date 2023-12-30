extern crate futures;
extern crate log;

mod mp4;

use chrono::{DateTime, Local};
use retina::client::Transport;

use futures::StreamExt;
use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use url::Url;

use telegram_bot::{prelude::*, InputFileUpload};
use telegram_bot::{Api, Message, MessageKind, UpdateKind};
use tokio_compat_02::FutureExt;

use crate::mp4::{Opts, Source};

#[derive(Debug)]
enum Command {
    XynNow,
}

async fn send_video(api: Api, message: Message) -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("CAMERA_URL").expect("CAMERA_URL not set");
    let username = env::var("CAMERA_USERNAME").expect("CAMERA_USERNAME not set");
    let password = env::var("CAMERA_PASSWORD").expect("CAMERA_PASSWORD not set");

    let src = Source {
        url: Url::try_from(url.as_str())?,
        username: Some(username),
        password: Some(password),
    };

    let now: DateTime<Local> = Local::now();
    let filename = format!("recording_{}.mp4", now);
    let out = PathBuf::from(Path::new(&filename));

    let feedback_message = api
        .send(message.text_reply("Recording 5 sec video.."))
        .await?;

    let result = mp4::run(Opts {
        src,
        out: out.clone(),
        initial_timestamp: retina::client::InitialTimestampPolicy::Default,
        no_video: false,
        no_audio: true,
        allow_loss: true,
        teardown: retina::client::TeardownPolicy::Always,
        duration: Some(5),
        transport: Transport::from_str("udp")?,
    })
    .compat()
    .await;

    match result {
        Ok(()) => {
            let edited_message = api.send(feedback_message.edit_text("Recording done. Uploading.")).await?;

            let reply = InputFileUpload::with_path(out.as_os_str().to_str().unwrap());

            let _ = api.send(message.video_reply(reply)).compat().await;

            let file_exists = tokio::fs::try_exists(out.clone()).compat().await?;

            if file_exists {
                let _ = tokio::fs::remove_file(out).compat().await;
            }

            api.send(edited_message.delete()).await?;
        }

        Err(err) => {
            log::error!("Recording has failed. Reason in the next message.");
            log::error!("{:?}", err);

            api.send(feedback_message.edit_text("Recording has failed. Please try again later.")).await?;
        }
    }

    Ok(())
}

fn get_command(message: &str, bot_name: &str) -> Option<Command> {
    if !message.starts_with('/') {
        return None;
    }

    // splits the bot name from the command, in case it is there
    let mut cmd = message;
    if cmd.ends_with(bot_name) {
        cmd = cmd.rsplit_once('@').unwrap().0;
    }

    match cmd {
        "/xyn_now" => Some(Command::XynNow),
        _ => None,
    }
}

async fn start_telegram_server() -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Setting up telegram server..");

    let bot_name = env::var("TELEGRAM_BOT_NAME").expect("TELEGRAM_BOT_NAME not set");
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");

    let api = Api::new(token);
    let mut stream = api.stream();

    // .compat() is needed here
    // because reqwest uses tokio 0.2
    // while telegram-bot uses tokio 1.x
    //
    // compat() is a trait implemented by the
    // tokio-compat-02 package to allow different libraries using
    // different tokio runtimes to work in the same process
    while let Some(update) = stream.next().compat().await {
        if let Err(err) = update {
            log::error!("Intercepting error from stream");
            log::error!("{:?}", err);

            return Err(Box::new(err));
        };

        if let UpdateKind::Message(message) = update?.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                let command = get_command(data.as_str(), bot_name.as_str());
                let api = api.clone();

                if let Some(Command::XynNow) = command {
                    log::debug!("Triggering {:?} command", Command::XynNow);
                    send_video(api, message).compat().await?
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    log::info!("Initializing process..");

    tokio::select! {
        _ = start_telegram_server() => {},
    };

    tokio::signal::ctrl_c().await.unwrap();

    log::info!("Received Ctrl-C, shutting down.");
}
