use futures::StreamExt;
use std::env;
use tokio_compat_02::FutureExt;

use telegram_bot::{Api, MessageKind, UpdateKind};

use crate::send_video_command::send_video_command;

#[derive(Debug)]
enum Command {
    GetRecordNow,
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

    let get_record_now_command =
        env::var("GET_RECORD_COMMAND").unwrap_or("/camera_now".to_string());

    if cmd == get_record_now_command {
        return Some(Command::GetRecordNow);
    }

    None
}

pub async fn start_telegram_server() -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Starting telegram server..");

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
            log::error!("Intercepting error from stream. Panicking the process. Reason:");
            log::error!("{:?}", err);

            panic!("{:?}", err);
        };

        if let UpdateKind::Message(message) = update?.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                let command = get_command(data.as_str(), bot_name.as_str());
                let api = api.clone();

                if let Some(Command::GetRecordNow) = command {
                    log::debug!("Triggering {:?} command", Command::GetRecordNow);
                    let result = send_video_command(api, message).compat().await;

                    if let Err(err) = result {
                        log::error!("{:?}", err);
                        panic!("Failed to reply send video command. Panicking server so that a reboot happens.");
                    }
                }
            }
        }
    }

    Ok(())
}
