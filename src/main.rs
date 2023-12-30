extern crate futures;
extern crate log;

mod mp4;
mod send_video_command;
mod server;

use crate::server::start_telegram_server;

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
