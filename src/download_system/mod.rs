use crate::AppData;
use actix_web::web::Data;
use anyhow::Result;
use tokio;

pub mod download;
pub mod orchestration;
pub mod transfer;

pub async fn start(app_data: Data<AppData>) -> Result<()> {
    let (sender, receiver) = async_channel::unbounded();
    let (download_sender, download_receiver) = async_channel::unbounded();
    let data = app_data.clone();
    let tx = sender.clone();
    download::recover_stuck_downloads(&app_data, &download_sender).await?;
    // Periodic stuck file recovery
    let app_data_clone = app_data.clone();
    let download_sender_clone = download_sender.clone();
    tokio::spawn(async move {
        let interval = app_data_clone.config.stuck_recovery_interval_secs;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
            if let Err(e) = download::recover_stuck_downloads(&app_data_clone, &download_sender_clone).await {
                log::warn!("Periodic stuck file recovery failed: {}", e);
            }
        }
    });
    actix_rt::spawn(async { transfer::produce_transfers(data, tx).await });

    for id in 0..app_data.config.orchestration_workers {
        let data = app_data.clone();
        let tx = sender.clone();
        let rx = receiver.clone();
        let dtx = download_sender.clone();
        orchestration::Worker::start(id, data, tx, rx, dtx);
    }

    for id in 0..app_data.config.download_workers {
        let drx = download_receiver.clone();
        let data = app_data.clone();
        download::Worker::start(id, data, drx)
    }

    Ok(())
}
