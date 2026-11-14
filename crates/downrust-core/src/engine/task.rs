// SPDX-License-Identifier: GPL-3.0-or-later
//! Download task: runs attempts, restarts them when the file on the server changed, and reports
//! the final outcome.
use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::model::{DownloadId, EngineEvent};
use crate::settings::EngineSettings;
use crate::store::Store;

use super::attempt::{run_attempt, AttemptEnd};
use super::checksum::verify_completed;
use super::file::remove_if_exists;
use super::limiter::RateLimiter;
use super::live::LiveStats;

const KEEPS_CHANGING: &str = "the file on the server keeps changing";

/// Everything one download task needs. The manager builds it.
pub(crate) struct TaskContext {
    pub id: DownloadId,
    pub store: Store,
    pub client: reqwest::Client,
    pub settings: EngineSettings,
    pub global_limiter: Arc<RateLimiter>,
    pub limiter: Arc<RateLimiter>,
    pub live: Arc<LiveStats>,
    pub cancel: CancellationToken,
    pub events: UnboundedSender<EngineEvent>,
}

/// How a task ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskOutcome {
    Completed,
    Cancelled,
    Failed(String),
}

/// Runs one download to its end. Failed attempts are reported as `Failed`; a changed file starts
/// the transfer again from zero, and a second restart switches to a single stream.
pub(crate) async fn run_task(ctx: TaskContext) -> TaskOutcome {
    let mut restarts: u32 = 0;
    let mut force_single = false;
    loop {
        match run_attempt(&ctx, force_single).await {
            Ok(AttemptEnd::Completed) => return verify_completed(&ctx).await,
            Ok(AttemptEnd::Cancelled) => return TaskOutcome::Cancelled,
            Ok(AttemptEnd::Restart(reason)) => {
                restarts += 1;
                let Some(next_single) = next_after_restart(restarts) else {
                    return TaskOutcome::Failed(KEEPS_CHANGING.to_owned());
                };
                tracing::warn!(id = ctx.id, %reason, "restarting download from zero");
                if let Err(err) = reset_progress(&ctx) {
                    return TaskOutcome::Failed(err.to_string());
                }
                force_single = next_single;
            }
            Err(err) => return TaskOutcome::Failed(err.to_string()),
        }
    }
}

/// After the n-th restart (1-based): `Some(force_single)` for the next attempt, or `None` when
/// the transfer gives up.
pub(crate) fn next_after_restart(restarts: u32) -> Option<bool> {
    match restarts {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

/// Clears saved progress and deletes the part file.
fn reset_progress(ctx: &TaskContext) -> crate::Result<()> {
    let record = ctx.store.get_download(ctx.id)?;
    ctx.store.clear_progress(ctx.id)?;
    if let Some(record) = record {
        remove_if_exists(&record.part_path())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::client::build_client;
    use crate::model::{Category, DownloadStatus, NewDownload};
    use downrust_testserver::{file_url, full_content, FileOpts, TestServer};

    #[test]
    fn restarts_allow_two_tries_then_give_up() {
        assert_eq!(next_after_restart(1), Some(false));
        assert_eq!(next_after_restart(2), Some(true));
        assert_eq!(next_after_restart(3), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn segmented_download_completes_with_exact_bytes() {
        let server = TestServer::start().expect("test server starts");
        let size: u64 = 1024 * 1024;
        let dir = tempfile::tempdir().expect("temp dir");
        let settings = EngineSettings {
            min_segment_size: 64 * 1024,
            checkpoint_interval_ms: 200,
            retry_base_delay_ms: 50,
            retry_max_delay_ms: 400,
            use_system_proxy: false,
            default_dir: dir.path().display().to_string(),
            ..EngineSettings::default()
        };
        let store = Store::open_in_memory().expect("store opens");
        let record = store
            .insert_download(&NewDownload {
                expected_sha256: None,
                kind: crate::model::DownloadKind::File,
                mirrors: Vec::new(),
                options: None,
                url: server.url(&file_url(size, &FileOpts::default())),
                final_url: None,
                referrer: None,
                file_name: "smoke.bin".to_owned(),
                save_dir: dir.path().display().to_string(),
                category: Category::Other,
                status: DownloadStatus::Queued,
                total_size: None,
                resumable: false,
                connections: 4,
                speed_limit: None,
                etag: None,
                last_modified: None,
                content_type: None,
                headers: Vec::new(),
                queue_position: 1,
                created_at: 0,
            })
            .expect("row inserted");
        let (events, _rx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = TaskContext {
            id: record.id,
            store: store.clone(),
            client: build_client(&settings).expect("client builds"),
            settings,
            global_limiter: Arc::new(RateLimiter::new(0)),
            limiter: Arc::new(RateLimiter::new(0)),
            live: Arc::new(LiveStats::new(4)),
            cancel: CancellationToken::new(),
            events,
        };
        let outcome = run_task(ctx).await;
        assert_eq!(outcome, TaskOutcome::Completed);
        let done = store
            .get_download(record.id)
            .expect("query works")
            .expect("row exists");
        assert_eq!(done.downloaded, size);
        let bytes = std::fs::read(dir.path().join("smoke.bin")).expect("file exists");
        assert_eq!(bytes, full_content(1, size));
        assert!(server.state.stats().range_requests >= 2);
    }
}
