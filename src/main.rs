extern crate futures;
extern crate log;

mod mp4;

use chrono::{DateTime, Local};
use retina::client::Transport;

use url::Url;
use futures::StreamExt;
use std::env;
use std::path::{PathBuf, Path};
use std::str::FromStr;

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

    log::error!("{:?}", src);

    let now: DateTime<Local> = Local::now();
    let filename = format!("recording_{}.mp4", now);
    let out = PathBuf::from(Path::new(&filename));
    api.send(message.text_reply("Recording 5 sec video..")).await?;

    let result = mp4::run(Opts {
        src,
        initial_timestamp: retina::client::InitialTimestampPolicy::Default,
        no_video: false,
        no_audio: true,
        allow_loss: false,
        teardown: retina::client::TeardownPolicy::Auto,
        duration: Some(5),
        transport: Transport::from_str("tcp")?,
        out: out.clone(),
    }).compat().await;
    
    match result {
        Ok(()) => { 
            let reply = InputFileUpload::with_path(out.as_os_str().to_str().unwrap());

            let _ = api.send(message.video_reply(reply)).compat().await;

            let _ = tokio::fs::remove_file(out).compat().await;
        }
        Err(err) => {
            log::error!("{:?}", err);
        }
    }

    Ok(())
}

fn get_command(message: &str, bot_name: &str) -> Option<Command> {
    if !message.starts_with("/") {
        return None;
    }

    // splits the bot name from the command, in case it is there
    let mut cmd = message;
    if cmd.ends_with(bot_name) {
        cmd = cmd.rsplitn(2, '@').skip(1).next().unwrap();
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
        if let UpdateKind::Message(message) = update?.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                let command = get_command(data.as_str(), bot_name.as_str());
                let api = api.clone();

                match command {
                    Some(Command::XynNow) => {
                        log::debug!("Triggering {:?} command", Command::XynNow);
                        send_video(api, message).compat().await?
                    }
                    _ => (),
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
