use chrono::Local;
use retina::client::Transport;
use tokio_compat_02::FutureExt;

use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use url::Url;

use telegram_bot::{prelude::*, InputFileUpload};
use telegram_bot::{Api, Message};

use crate::mp4::{self, Opts, Source};

fn get_source() -> Result<Source, Box<dyn std::error::Error>> {
    let url = env::var("CAMERA_URL").expect("CAMERA_URL not set");
    let username = env::var("CAMERA_USERNAME").expect("CAMERA_USERNAME not set");
    let password = env::var("CAMERA_PASSWORD").expect("CAMERA_PASSWORD not set");

    Ok(Source {
        url: Url::parse(&url)?,
        username: Some(username),
        password: Some(password),
    })
}

fn get_output() -> PathBuf {
    let filename = format!("recording_{}.mp4", Local::now());
    let output = PathBuf::from(Path::new(&filename));
    output
}

fn get_recording_options(src: Source, out: PathBuf) -> Result<Opts, Box<dyn std::error::Error>> {
    Ok(Opts {
        src,
        out: out.clone(),
        no_video: false,
        no_audio: true,
        duration: Some(5),
        initial_timestamp: retina::client::InitialTimestampPolicy::Default,
        /*
         * During development and tracing some "Invalid RTSP message" errors,
         * I've found the following log warn message from the retina package:
         *
         * 2023-12-30_09:29:40.74555 [2023-12-30T09:29:40Z WARN  retina::client]
         * Connecting via TCP to known-broken RTSP server "TP-LINK Streaming Media v2015.05.12".
         * See <https://github.com/scottlamb/retina/issues/17>.
         * Consider using UDP instead!
         *
         * This is why all the settings below are being set as such.
         */
        transport: Transport::from_str("udp")?,
        teardown: retina::client::TeardownPolicy::Always,
        allow_loss: true,
    })
}

pub async fn send_video_command(
    api: Api,
    command_msg: Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let feedback_message = api
        .send(command_msg.text_reply("Recording 5 sec video.."))
        .await?;

    let source = get_source()?;
    let output = get_output();
    let options = get_recording_options(source, output.clone())?;
    let mp4_recorder_result = mp4::run(options).compat().await;

    if let Err(recorder_error) = mp4_recorder_result {
        log::error!("Recording has failed. Reason in the next message.");
        log::error!("{:?}", recorder_error);

        let set_error_feedback_message =
            feedback_message.edit_text("Recording has failed. Please try again later.");

        api.send(set_error_feedback_message).await?;
    } else {
        let set_success_feedback_msg = api
            .send(feedback_message.edit_text("Recording done. Uploading."))
            .await?;

        let mp4_recording_input_file =
            InputFileUpload::with_path(output.clone().into_os_string().into_string().unwrap());

        let _video_reply = api
            .send(command_msg.video_reply(mp4_recording_input_file))
            .await?;

        let delete_feedback_msg = api.send(set_success_feedback_msg.delete()).await;

        if let Err(delete_feedback_msg_error) = delete_feedback_msg {
            log::error!(
                "Failed to delete success feedback message '{}' from chat '{}'",
                set_success_feedback_msg.id,
                set_success_feedback_msg.chat.id()
            );
            log::error!("{:?}", delete_feedback_msg_error);
        }

        let file_exists = tokio::fs::try_exists(output.clone()).await?;

        if file_exists {
            let remove_file_result = tokio::fs::remove_file(output.clone()).await;

            if let Err(remove_file_error) = remove_file_result {
                log::error!("Failed to delete file at '{}'", output.display());
                log::error!("{:?}", remove_file_error);
            }
        }
    }

    Ok(())
}
