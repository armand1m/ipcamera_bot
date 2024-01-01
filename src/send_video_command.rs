use chrono::Local;
use retina::client::{InitialTimestampPolicy, TeardownPolicy, Transport};
use tokio_compat_02::FutureExt;

use std::env;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use url::Url;

use telegram_bot::{prelude::*, InputFileUpload};
use telegram_bot::{Api, Message};

use crate::mp4::{self, Mp4RecorderOptions, Source};

fn get_source() -> Result<Source, Box<dyn std::error::Error>> {
    let unsafe_url = env::var("CAMERA_URL").expect("CAMERA_URL not set");
    let username = env::var("CAMERA_USERNAME").expect("CAMERA_USERNAME not set");
    let password = env::var("CAMERA_PASSWORD").expect("CAMERA_PASSWORD not set");
    let url = Url::parse(&unsafe_url)?;

    Ok(Source {
        url,
        username,
        password,
    })
}

fn get_output() -> PathBuf {
    let filename = format!("recording_{}.mp4", Local::now());
    let output = PathBuf::from(Path::new(&filename));
    output
}

fn get_recording_options(
    src: Source,
    out: PathBuf,
) -> Result<Mp4RecorderOptions, Box<dyn std::error::Error>> {
    let no_audio_str = env::var("RECORD_NO_AUDIO").unwrap_or("true".to_string());
    let no_video_str = env::var("RECORD_NO_VIDEO").unwrap_or("false".to_string());
    let duration_str = env::var("RECORD_DURATION_SECOND").unwrap_or("5".to_string());

    let no_audio = no_audio_str.parse::<bool>().unwrap();
    let no_video = no_video_str.parse::<bool>().unwrap();
    let duration = Some(duration_str.parse::<u64>().unwrap());

    Ok(Mp4RecorderOptions {
        src,
        out,
        no_video,
        no_audio,
        duration,
        initial_timestamp: InitialTimestampPolicy::Default,
        /*
         * During development and tracing I've got some "Invalid RTSP message" errors
         * and found the following log warn message from the retina package:
         *
         * 2023-12-30_09:29:40.74555 [2023-12-30T09:29:40Z WARN  retina::client]
         * Connecting via TCP to known-broken RTSP server "TP-LINK Streaming Media v2015.05.12".
         * See <https://github.com/scottlamb/retina/issues/17>.
         * Consider using UDP instead!
         *
         * This is why all the settings below are being set as such.
         *
         * If you have a different camera, please take that into consideration.
         *
         * These properties may be loaded from env vars in the future to allow
         * for more flexibility.
         */
        transport: Transport::from_str("udp")?,
        teardown: TeardownPolicy::Always,
        allow_loss: true,
    })
}

pub async fn send_video_command(
    api: Api,
    command_msg: Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let feedback_msg = api
        .send(command_msg.text_reply("Recording 5 sec video.."))
        .await?;

    let source = get_source()?;
    let output = get_output();
    let options = get_recording_options(source, output.clone())?;
    let recording_result = mp4::start_recording(options).compat().await;

    if let Err(recorder_error) = recording_result {
        log::error!("Recording has failed. Reason in the next message.");
        log::error!("{:?}", recorder_error);

        let set_error_feedback_msg =
            feedback_msg.edit_text("Recording has failed. Please try again later.");

        api.send(set_error_feedback_msg).await?;
        return Ok(());
    }

    let set_success_feedback_msg = api
        .send(feedback_msg.edit_text("Recording done. Uploading."))
        .await?;

    let recording_input_file =
        InputFileUpload::with_path(output.clone().into_os_string().into_string().unwrap());

    let _video_reply = api
        .send(command_msg.video_reply(recording_input_file))
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

    Ok(())
}
