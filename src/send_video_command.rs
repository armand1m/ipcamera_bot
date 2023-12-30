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

pub async fn send_video_command(
    api: Api,
    command_msg: Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let feedback_message = api
        .send(command_msg.text_reply("Recording 5 sec video.."))
        .await?;

    let url = env::var("CAMERA_URL").expect("CAMERA_URL not set");
    let username = env::var("CAMERA_USERNAME").expect("CAMERA_USERNAME not set");
    let password = env::var("CAMERA_PASSWORD").expect("CAMERA_PASSWORD not set");

    let filename = format!("recording_{}.mp4", Local::now());
    let out = PathBuf::from(Path::new(&filename));

    let src = Source {
        url: Url::parse(&url)?,
        username: Some(username),
        password: Some(password),
    };

    let recorder_options = Opts {
        src,
        out: out.clone(),
        initial_timestamp: retina::client::InitialTimestampPolicy::Default,
        no_video: false,
        no_audio: true,
        allow_loss: true,
        teardown: retina::client::TeardownPolicy::Always,
        duration: Some(5),
        transport: Transport::from_str("udp")?,
    };

    let mp4_recorder_result = mp4::run(recorder_options).compat().await;

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
            InputFileUpload::with_path(out.clone().into_os_string().into_string().unwrap());

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

        let file_exists = tokio::fs::try_exists(out.clone()).await?;

        if file_exists {
            let remove_file_result = tokio::fs::remove_file(out.clone()).await;

            if let Err(remove_file_error) = remove_file_result {
                log::error!("Failed to delete file at '{}'", out.display());
                log::error!("{:?}", remove_file_error);
            }
        }
    }

    Ok(())
}
