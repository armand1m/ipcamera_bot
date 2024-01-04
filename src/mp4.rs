use anyhow::{anyhow, bail, Context, Error};
use futures::{
    future::{pending, Either},
    StreamExt,
};
use log::{debug, error, info, warn};
use retina::{
    client::{
        Credentials, Demuxed, Described, InitialTimestampPolicy, PlayOptions, Session,
        SessionGroup, SessionOptions, SetupOptions, TeardownPolicy, Transport,
    },
    codec::{AudioParameters, CodecItem, ParametersRef},
    rtcp::PacketRef,
};

use std::path::PathBuf;
use std::sync::Arc;
use std::{num::NonZeroU32, time::Duration};
use tokio::{fs::File, time::sleep};

use crate::mp4_writer::Mp4Writer;

#[derive(Debug)]
pub struct Source {
    /// `rtsp://` URL to connect to.
    pub(crate) url: url::Url,

    /// Username to send if the server requires authentication.
    pub(crate) username: String,

    /// Password; requires username.
    pub(crate) password: String,
}

pub struct Mp4RecorderOptions {
    pub(crate) source: Source,

    /// Policy for handling the `rtptime` parameter normally seem in the `RTP-Info` header.
    /// One of `default`, `require`, `ignore`, `permissive`.
    pub(crate) initial_timestamp: InitialTimestampPolicy,

    /// Don't attempt to include video streams.
    pub(crate) no_video: bool,

    /// Don't attempt to include audio streams.
    pub(crate) no_audio: bool,

    /// Allow lost packets mid-stream without aborting.
    pub(crate) allow_loss: bool,

    /// When to issue a `TEARDOWN` request: `auto`, `always`, or `never`.
    pub(crate) teardown: TeardownPolicy,

    /// Duration after which to exit automatically, in seconds.
    pub(crate) duration: Option<u64>,

    /// The transport to use: `tcp` or `udp` (experimental).
    ///
    /// Note: `allow_loss` is strongly recommended with `udp`.
    pub(crate) transport: Transport,

    /// Path to `.mp4` file to write.
    pub(crate) output: PathBuf,
}

/// Copies packets from `session` to `mp4` without handling any cleanup on error.
async fn copy<'a>(
    options: &'a Mp4RecorderOptions,
    session: &'a mut Demuxed,
    mp4_writer: &'a mut Mp4Writer<File>,
) -> Result<(), Error> {
    let sleep = match options.duration {
        Some(secs) => Either::Left(sleep(Duration::from_secs(secs))),
        None => Either::Right(pending()),
    };

    tokio::pin!(sleep);

    loop {
        tokio::select! {
            packet = session.next() => {
                match packet.ok_or_else(|| anyhow!("EOF"))?? {
                    CodecItem::VideoFrame(frame) => {
                        let stream = &session.streams()[frame.stream_id()];
                        let start_ctx = *frame.start_ctx();
                        mp4_writer.video(stream, frame).await.with_context(
                            || format!("Error processing video frame starting with {start_ctx}"))?;
                    },
                    CodecItem::AudioFrame(frame) => {
                        let ctx = *frame.ctx();
                        mp4_writer.audio(frame).await.with_context(
                            || format!("Error processing audio frame, {ctx}"))?;
                    },
                    CodecItem::Rtcp(rtcp) => {
                        if let (Some(timestamp), Some(Ok(Some(sender_report)))) = (rtcp.rtp_timestamp(), rtcp.pkts().next().map(PacketRef::as_sender_report)) {
                            debug!("RTP timestamp={}: Sender Report timestamp={}", timestamp, sender_report.ntp_timestamp());
                        }
                    },
                    CodecItem::MessageFrame(msg) => {
                        debug!("Received Message Frame: {:?}", msg);
                        continue;
                    },
                    codec_item => {
                        debug!("Received Unhandled CodecItem: {:?}", codec_item);
                        continue;
                    }
                };
            },
            _ = &mut sleep => {
                info!("Stopping after {} seconds", options.duration.unwrap());
                break;
            },
        }
    }
    Ok(())
}

/// Writes the `.mp4`, including trying to finish or clean up the file.
async fn write_mp4(
    options: &Mp4RecorderOptions,
    session: Session<Described>,
    audio_params: Option<Box<AudioParameters>>,
) -> Result<(), Error> {
    let mut session = session
        .play(
            PlayOptions::default()
                .initial_timestamp(options.initial_timestamp)
                .enforce_timestamps_with_max_jump_secs(NonZeroU32::new(10).unwrap()),
        )
        .await?
        .demuxed()?;

    // Append into a filename suffixed with ".partial",
    // then try to either rename it into place if
    // it's complete or delete it otherwise.
    const PARTIAL_SUFFIX: &str = ".partial";

    // OsString::push doesn't put in a '/', unlike PathBuf
    let mut tmp_filename = options.output.as_os_str().to_owned();
    tmp_filename.push(PARTIAL_SUFFIX);
    let tmp_filename: PathBuf = tmp_filename.into();

    let output = File::create(&tmp_filename).await?;

    let mut mp4 = Mp4Writer::new(audio_params, options.allow_loss, output).await?;
    let result = copy(options, &mut session, &mut mp4).await;

    if let Err(mp4_error) = mp4.finish().await {
        error!(".mp4 finish failed: {}", mp4_error);

        if let Err(rm_file_error) = tokio::fs::remove_file(&tmp_filename).await {
            error!("and removing .mp4 failed too: {}", rm_file_error);
        }
    } else if let Err(mv_file_error) = tokio::fs::rename(&tmp_filename, &options.output).await {
        error!("unable to completed .mp4 into place: {}", mv_file_error);
    }

    result?;

    Ok(())
}

async fn setup_video_stream(
    session: &mut Session<Described>,
    options: &Mp4RecorderOptions,
) -> Result<Option<usize>, Error> {
    let video_stream_index = if !options.no_video {
        let stream_index = session.streams().iter().position(|stream| {
            if stream.media() != "video" {
                return false;
            }

            if stream.encoding_name() == "h264" {
                info!("Using h264 video stream");
                return true;
            }

            info!(
                "Ignoring {} video stream because it's unsupported",
                stream.encoding_name(),
            );

            false
        });

        if stream_index.is_none() {
            info!("No suitable video stream found");
        }

        stream_index
    } else {
        info!("Ignoring video streams (if any) because of RECORD_NO_VIDEO");
        None
    };

    if let Some(stream_index) = video_stream_index {
        session
            .setup(
                stream_index,
                SetupOptions::default().transport(options.transport.clone()),
            )
            .await?;
    }

    Ok(video_stream_index)
}

async fn setup_audio_stream(
    session: &mut Session<Described>,
    options: &Mp4RecorderOptions,
) -> Result<Option<(usize, Box<AudioParameters>)>, Error> {
    let audio_stream_tuple = if !options.no_audio {
        let audio_params = session
            .streams()
            .iter()
            .enumerate()
            .find_map(|(index, stream)| match stream.parameters() {
                // Only consider audio streams that can produce a .mp4 sample entry.
                Some(ParametersRef::Audio(audio_params)) if audio_params.sample_entry().is_some() => {
                    info!("Using {} audio stream (rfc 6381 codec {})", stream.encoding_name(), audio_params.rfc6381_codec().unwrap());
                    Some((index, Box::new(audio_params.clone())))
                }

                _ if stream.media() == "audio" => {
                    info!("Ignoring {} audio stream because it can't be placed into a .mp4 file without transcoding", stream.encoding_name());
                    None
                }

                _ => None,
            });

        if audio_params.is_none() {
            info!("No suitable audio stream found");
        }

        audio_params
    } else {
        info!("Ignoring audio streams (if any) because of RECORD_NO_AUDIO");
        None
    };

    if let Some((index, _)) = audio_stream_tuple {
        session
            .setup(
                index,
                SetupOptions::default().transport(options.transport.clone()),
            )
            .await?;
    }

    Ok(audio_stream_tuple)
}

pub async fn start_recording(options: Mp4RecorderOptions) -> Result<(), Error> {
    if matches!(options.transport, Transport::Udp(_)) && !options.allow_loss {
        warn!("Using UDP without strongly recommended `allow_loss`!");
    }

    let credentials = Some(Credentials {
        username: options.source.username.clone(),
        password: options.source.password.clone(),
    });

    let session_group = Arc::new(SessionGroup::default());
    let mut session = Session::describe(
        options.source.url.clone(),
        SessionOptions::default()
            .creds(credentials)
            .session_group(session_group.clone())
            .user_agent("IPCameraBot_RustImpl".to_owned())
            .teardown(options.teardown),
    )
    .await?;

    let video_stream_index = setup_video_stream(&mut session, &options).await?;
    let audio_stream_index = setup_audio_stream(&mut session, &options).await?;

    if video_stream_index.is_none() && audio_stream_index.is_none() {
        bail!("Exiting because no video or audio stream was selected; see info log messages above");
    }

    let write_result = write_mp4(
        &options,
        session,
        audio_stream_index.map(|(_index, audio_param)| audio_param),
    )
    .await;

    // Session has now been dropped, on success or failure. A TEARDOWN should
    // be pending if necessary. session_group.await_teardown() will wait for it.
    if let Err(teardown_error) = session_group.await_teardown().await {
        error!("TEARDOWN failed: {}", teardown_error);
    }

    write_result
}
