use super::transfer::{DownloadTarget, TargetType};
use crate::AppData;
use actix_web::web::Data;
use anyhow::{bail, Context, Result};
use async_channel::{Receiver, Sender};
use colored::*;
use file_owner::PathExt;
use futures::StreamExt;
use log::{error, info, warn};
use nix::unistd::Uid;
use std::{fs, path::Path};
use tokio::time::{sleep, timeout};
use std::time::Duration;

#[derive(Clone)]
pub struct Worker {
    _id: usize,
    app_data: Data<AppData>,
    drx: Receiver<DownloadTargetMessage>,
}

impl Worker {
    pub fn start(id: usize, app_data: Data<AppData>, drx: Receiver<DownloadTargetMessage>) {
        let s = Self {
            _id: id,
            app_data,
            drx,
        };

        let _join_handle = actix_rt::spawn(async move { s.work().await });
    }
    async fn work(&self) -> Result<()> {
        loop {
            // Wait for a DownloadTarget
            let dtm = self.drx.recv().await?;

            // Download the target
            let done_status = match download_target(&self.app_data, &dtm.download_target).await {
                Ok(_) => DownloadDoneStatus::Success,
                Err(_) => DownloadDoneStatus::Failed,
            };
            dtm.tx.send(done_status).await?;
        }
    }
}

async fn download_target(app_data: &Data<AppData>, target: &DownloadTarget) -> Result<()> {
    match target.target_type {
        TargetType::Directory => {
            if !Path::new(&target.to).exists() {
                fs::create_dir(&target.to)?;
                if Uid::effective().is_root() {
                    target.to.clone().set_owner(app_data.config.uid)?;
                }
                info!("{}: directory created", &target);
            }
        }
        TargetType::File => {
            // Delete file if already exists
            if !Path::new(&target.to).exists() {
                info!("{}: download {}", &target, "started".yellow());
                match fetch(target, app_data.config.uid, app_data).await {
                    Ok(_) => info!("{}: download {}", &target, "succeeded".green()),
                    Err(e) => {
                        error!("{}: download {}: {}", &target, "failed".red(), e);
                        bail!(e)
                    }
                };
            } else {
                info!("{}: already exists", &target);
            }
        }
    }
    Ok(())
}

// Add a function to scan for stuck .downloading files and requeue them
pub async fn recover_stuck_downloads(app_data: &Data<AppData>, dtx: &Sender<DownloadTargetMessage>) -> Result<()> {
    use walkdir::WalkDir;
    use super::transfer::{DownloadTarget, TargetType};
    use async_channel;
    use log::warn;

    let download_dir = &app_data.config.download_directory;
    for entry in WalkDir::new(download_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if let Some(path_str) = path.to_str() {
            if path_str.ends_with(".downloading") {
                let final_path = path_str.trim_end_matches(".downloading");
                if !Path::new(final_path).exists() {
                    warn!("Found stuck .downloading file: {}. Requeuing for download.", path_str);
                    // Requeue as a DownloadTarget
                    let target = DownloadTarget {
                        from: None, // We don't have the URL, so this will need to be handled by orchestration/transfer logic
                        to: final_path.to_string(),
                        target_type: TargetType::File,
                        top_level: false,
                        transfer_hash: "recovery".to_string(),
                    };
                    let (done_tx, _done_rx) = async_channel::unbounded();
                    dtx.send(DownloadTargetMessage { download_target: target, tx: done_tx }).await?;
                }
            }
        }
    }
    Ok(())
}

// Add retry logic to fetch
async fn fetch(target: &DownloadTarget, uid: u32, app_data: &Data<AppData>) -> Result<()> {
    let tmp_path = format!("{}.downloading", &target.to);
    let url = target.from.clone().context("No URL found")?;
    let max_retries = app_data.config.max_download_retries;
    let backoff_ms = app_data.config.retry_backoff_ms;
    let timeout_secs = app_data.config.download_timeout_secs;
    let mut attempt = 0;
    let mut last_err = None;
    while attempt < max_retries {
        attempt += 1;
        let mut tmp_file = match tokio::fs::File::create(&tmp_path).await {
            Ok(f) => f,
            Err(e) => {
                last_err = Some(anyhow::anyhow!(e));
                break;
            }
        };
        let resp = reqwest::get(url.clone()).await;
        match resp {
            Ok(response) => {
                let content_length = response.content_length();
                let mut byte_stream = response.bytes_stream();
                let mut success = true;
                let mut total_bytes: u64 = 0;
                let download_result = timeout(Duration::from_secs(timeout_secs), async {
                    while let Some(item) = byte_stream.next().await {
                        match item {
                            Ok(bytes) => {
                                total_bytes += bytes.len() as u64;
                                if let Err(e) = tokio::io::copy(&mut bytes.as_ref(), &mut tmp_file).await {
                                    last_err = Some(anyhow::anyhow!(e));
                                    success = false;
                                    break;
                                }
                            }
                            Err(e) => {
                                last_err = Some(anyhow::anyhow!(e));
                                success = false;
                                break;
                            }
                        }
                    }
                }).await;
                if download_result.is_err() {
                    last_err = Some(anyhow::anyhow!("Download timed out"));
                    success = false;
                }
                // File integrity check: compare to Content-Length if available
                if success {
                    if let Some(expected) = content_length {
                        if total_bytes != expected {
                            last_err = Some(anyhow::anyhow!(format!("File size mismatch: expected {} bytes, got {} bytes", expected, total_bytes)));
                            success = false;
                        }
                    }
                }
                if success {
                    if Uid::effective().is_root() {
                        tmp_path.clone().set_owner(uid)?;
                    }
                    if let Err(e) = fs::rename(&tmp_path, &target.to) {
                        last_err = Some(anyhow::anyhow!(e));
                        // Clean up temp file
                        let _ = fs::remove_file(&tmp_path);
                        return Err(anyhow::anyhow!(e));
                    }
                    return Ok(());
                }
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!(e));
            }
        }
        // Exponential backoff
        let backoff = backoff_ms * 2u64.pow(attempt - 1);
        warn!("Download failed for {} (attempt {}/{}). Retrying in {}ms...", &target.to, attempt, max_retries, backoff);
        sleep(Duration::from_millis(backoff)).await;
    }
    // Clean up temp file if all retries failed
    let _ = fs::remove_file(&tmp_path);
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Unknown download error")))
}

#[derive(Debug, Clone)]
pub struct DownloadTargetMessage {
    pub download_target: DownloadTarget,
    pub tx: Sender<DownloadDoneStatus>,
}

#[derive(Debug, Clone)]
pub enum DownloadDoneStatus {
    Success,
    Failed,
}
