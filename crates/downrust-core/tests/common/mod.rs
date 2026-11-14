// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers for the integration tests: test settings, a harness with a local server and
//! a manager, and polling, event and hashing helpers.
#![allow(dead_code)]

use std::path::Path;
use std::time::Duration;

use downrust_core::manager::DownloadManager;
use downrust_core::model::{AddRequest, DownloadId, DownloadRecord, DownloadStatus, EngineEvent};
use downrust_core::settings::EngineSettings;
use downrust_core::store::Store;
use downrust_testserver::{full_content, TestServer};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

/// One mebibyte in bytes.
pub const MIB: u64 = 1024 * 1024;

/// Settings that keep the tests fast: small segments, short checkpoints and short retry delays.
pub fn test_settings(dir: &Path) -> EngineSettings {
    EngineSettings {
        min_segment_size: 64 * 1024,
        checkpoint_interval_ms: 200,
        progress_interval_ms: 100,
        retry_base_delay_ms: 50,
        retry_max_delay_ms: 400,
        max_retries: 5,
        use_system_proxy: false,
        auto_sort_by_category: false,
        default_dir: dir.display().to_string(),
        connect_timeout_secs: 5,
        read_timeout_secs: 5,
        ..EngineSettings::default()
    }
}

/// A local test server, a temporary download folder and a running manager.
pub struct Harness {
    pub server: TestServer,
    pub dir: TempDir,
    pub manager: DownloadManager,
    pub events: UnboundedReceiver<EngineEvent>,
    pub store: Store,
}

impl Harness {
    pub async fn start() -> Self {
        Self::start_with(|_| {}).await
    }

    /// Starts like [`Harness::start`], then lets the test change the settings.
    pub async fn start_with(configure: impl FnOnce(&mut EngineSettings)) -> Self {
        let server = TestServer::start().expect("test server starts");
        let dir = tempfile::tempdir().expect("temp dir");
        let mut settings = test_settings(dir.path());
        configure(&mut settings);
        let store = Store::open_in_memory().expect("store opens");
        let (tx, events) = unbounded_channel();
        let manager = DownloadManager::start(store.clone(), settings, tx)
            .await
            .expect("manager starts");
        Self {
            server,
            dir,
            manager,
            events,
            store,
        }
    }
}

/// An add request that starts at once with the given name and connection count.
pub fn request(url: &str, file_name: &str, connections: u8) -> AddRequest {
    AddRequest {
        url: url.to_owned(),
        file_name: Some(file_name.to_owned()),
        connections: Some(connections),
        start: true,
        ..AddRequest::default()
    }
}

/// Polls the download until it reaches `status`. Panics after `limit`.
pub async fn wait_for_status(
    manager: &DownloadManager,
    id: DownloadId,
    status: DownloadStatus,
    limit: Duration,
) -> DownloadRecord {
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        let record = manager.get(id).await.expect("download exists");
        if record.status == status {
            return record;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "download {id} did not reach {status:?}; last status {:?}, error {:?}",
            record.status,
            record.error
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Polls the download until at least `bytes` are downloaded. Panics after `limit`.
pub async fn wait_downloaded(
    manager: &DownloadManager,
    id: DownloadId,
    bytes: u64,
    limit: Duration,
) {
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        let record = manager.get(id).await.expect("download exists");
        if record.downloaded >= bytes {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "download {id} reached only {} of {bytes} bytes",
            record.downloaded
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Takes every event that is waiting in the channel.
pub fn drain_events(events: &mut UnboundedReceiver<EngineEvent>) -> Vec<EngineEvent> {
    let mut out = Vec::new();
    while let Ok(event) = events.try_recv() {
        out.push(event);
    }
    out
}

/// Hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("file readable");
    hex::encode(Sha256::digest(&bytes))
}

/// Hex SHA-256 of the expected content of `/file/{size}` at `version`.
pub fn expected_sha(version: u64, size: u64) -> String {
    hex::encode(Sha256::digest(full_content(version, size)))
}
