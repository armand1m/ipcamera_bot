use chrono::Local;
use retina::client::{InitialTimestampPolicy, TeardownPolicy, Transport};
use tokio_compat_02::FutureExt;

use futures::future;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{env, fs};
use url::Url;

use telegram_bot::{prelude::*, InputFileUpload};
use telegram_bot::{Api, Message};

use crate::mp4::{self, Mp4RecorderOptions, Source};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraConfig {
    pub cameras: Vec<Camera>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: String,
    pub no_audio: bool,
    pub no_video: bool,
    pub duration: u64,
    pub transport: String,
}

impl From<Camera> for Mp4RecorderOptions {
    fn from(camera: Camera) -> Self {
        let filename = format!("recording_{}.mp4", Local::now());
        let output = PathBuf::from(Path::new(&filename));
        let url = Url::parse(&camera.url).unwrap();
        let transport = Transport::from_str(camera.transport.as_str()).unwrap();
        let udp_transport = Transport::from_str("udp").unwrap();
        let is_udp = transport.to_string() == udp_transport.to_string();

        Mp4RecorderOptions {
            source: Source {
                url,
                username: camera.username,
                password: camera.password,
            },
            output,
            no_video: camera.no_video,
            no_audio: camera.no_audio,
            duration: camera.duration,
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
            transport,
            teardown: TeardownPolicy::Always,
            allow_loss: is_udp,
        }
    }
}

fn get_camera_configs() -> Result<CameraConfig, anyhow::Error> {
    let config_json_path: PathBuf = env::var("CAMERA_CONFIG_PATH")
        .expect("CAMERA_CONFIG not set")
        .into();
    let config_json = fs::read_to_string(config_json_path)?;
    let config: CameraConfig = serde_json::from_str(config_json.as_str())
        .expect("JSON in CAMERA_CONFIG env var is malformed.");

    Ok(config)
}

pub async fn send_video_for_camera(
    camera: Camera,
    api: Api,
    command_msg: Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let options: Mp4RecorderOptions = camera.clone().into();

    let feedback_msg = api
        .send(command_msg.text_reply(format!(
            "Recording {} sec video for camera {}..",
            options.duration, camera.name
        )))
        .await?;

    let recording_result = mp4::start_recording(options.clone()).compat().await;

    if let Err(recorder_error) = recording_result {
        log::error!(
            "Recording for camera {} has failed. Reason in the next message.",
            camera.name
        );
        log::error!("{:?}", recorder_error);

        let set_error_feedback_msg =
            feedback_msg.edit_text("Recording has failed. Please try again later.");

        api.send(set_error_feedback_msg).await?;
        return Ok(());
    }

    let set_success_feedback_msg = api
        .send(feedback_msg.edit_text(format!(
            "Recording for camera {} done. Uploading.",
            camera.name
        )))
        .await?;

    let recording_input_file = InputFileUpload::with_path(
        options
            .output
            .clone()
            .into_os_string()
            .into_string()
            .unwrap(),
    );

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

    let file_exists = tokio::fs::try_exists(options.output.clone()).await?;

    if file_exists {
        let remove_file_result = tokio::fs::remove_file(options.output.clone()).await;

        if let Err(remove_file_error) = remove_file_result {
            log::error!("Failed to delete file at '{}'", options.output.display());
            log::error!("{:?}", remove_file_error);
        }
    }

    Ok(())
}

pub async fn send_video_command(
    api: Api,
    command_msg: Message,
) -> Result<(), Box<dyn std::error::Error>> {
    let camera_config = get_camera_configs()?;

    let _ = future::try_join_all(
        camera_config
            .cameras
            .into_iter()
            .map(|camera| send_video_for_camera(camera, api.clone(), command_msg.clone())),
    )
    .compat()
    .await
    .unwrap();

    Ok(())
}
