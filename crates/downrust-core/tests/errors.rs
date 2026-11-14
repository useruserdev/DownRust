// SPDX-License-Identifier: GPL-3.0-or-later
//! Failures on the server side: a missing file, cut transfers and a busy server.
mod common;

use std::time::Duration;

use common::{expected_sha, request, sha256_file, wait_for_status, Harness, MIB};
use downrust_core::model::DownloadStatus;
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn missing_file_ends_in_error_with_status_code() {
    let h = Harness::start().await;
    let url = h.server.url("/no-such-file");
    let rec = h
        .manager
        .add(request(&url, "missing.bin", 4))
        .await
        .expect("added");
    let failed = wait_for_status(&h.manager, rec.id, DownloadStatus::Error, DONE).await;
    let message = failed.error.unwrap_or_default();
    assert!(message.contains("404"), "error message was {message:?}");
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cut_transfers_are_retried_and_complete() {
    let h = Harness::start().await;
    let size = 8 * MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    h.server.state.fail_next(3, 256 * 1024);
    let rec = h
        .manager
        .add(request(&url, "cut.bin", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn busy_server_reduces_connections_and_completes() {
    let h = Harness::start().await;
    let size = 8 * MIB;
    let opts = FileOpts {
        rate: MIB,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    h.server.state.set_max_concurrent(2);
    let rec = h
        .manager
        .add(request(&url, "busy.bin", 8))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    let stats = h.server.state.stats();
    assert!(stats.peak_active <= 2, "peak {}", stats.peak_active);
    h.manager.shutdown().await.expect("shutdown");
}
