// SPDX-License-Identifier: GPL-3.0-or-later
//! Crash recovery: the process loses its runtime mid-download, and a new manager on the same
//! database and folder resumes the transfer.
mod common;

use std::time::Duration;

use common::{
    expected_sha, request, sha256_file, test_settings, wait_downloaded, wait_for_status, MIB,
};
use downrust_core::manager::DownloadManager;
use downrust_core::model::{DownloadId, DownloadStatus, EngineEvent};
use downrust_core::store::Store;
use downrust_testserver::{file_url, FileOpts, TestServer};
use tokio::sync::mpsc::unbounded_channel;

const DONE: Duration = Duration::from_secs(60);

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("runtime builds")
}

#[test]
fn interrupted_download_resumes_after_restart() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("downrust.sqlite3");
    let server = TestServer::start().expect("test server starts");
    let size = 8 * MIB;
    let opts = FileOpts {
        rate: MIB,
        ..FileOpts::default()
    };
    let url = server.url(&file_url(size, &opts));
    let settings = test_settings(dir.path());

    // First run: start the download, wait until it is well under way, then drop the runtime
    // without shutting the manager down.
    let first = runtime();
    let id: DownloadId = first.block_on(async {
        let store = Store::open(&db_path).expect("store opens");
        let (tx, _events) = unbounded_channel::<EngineEvent>();
        let manager = DownloadManager::start(store, settings.clone(), tx)
            .await
            .expect("manager starts");
        let rec = manager
            .add(request(&url, "crash.bin", 4))
            .await
            .expect("added");
        wait_downloaded(&manager, rec.id, size * 2 / 5, DONE).await;
        rec.id
    });
    first.shutdown_background();

    // Second run: a new manager on the same database sees the download as interrupted.
    let second = runtime();
    second.block_on(async {
        let store = Store::open(&db_path).expect("store reopens");
        let (tx, _events) = unbounded_channel::<EngineEvent>();
        let manager = DownloadManager::start(store, settings.clone(), tx)
            .await
            .expect("manager starts");
        let interrupted = manager.get(id).await.expect("download survives restart");
        assert_eq!(interrupted.status, DownloadStatus::Paused);

        manager.resume(id).await.expect("resume accepted");
        let done = wait_for_status(&manager, id, DownloadStatus::Completed, DONE).await;
        assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
        let sent = server.state.stats().bytes_sent;
        assert!(sent < size * 8 / 5, "total sent {sent} exceeds 1.6 x size");
        manager.shutdown().await.expect("shutdown");
    });
    drop(second);
}
