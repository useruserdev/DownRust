// SPDX-License-Identifier: GPL-3.0-or-later
//! Servers without range support: one connection, no resume, and unknown length.
mod common;

use std::time::Duration;

use common::{expected_sha, request, sha256_file, wait_downloaded, wait_for_status, Harness, MIB};
use downrust_core::model::DownloadStatus;
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ranges_disabled_uses_one_connection() {
    let h = Harness::start().await;
    let size = 4 * MIB;
    let opts = FileOpts {
        ranges: false,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "plain.bin", 8))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert!(!done.resumable);
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    assert_eq!(h.server.state.stats().peak_active, 1);
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_length_gets_its_size_at_the_end() {
    let h = Harness::start().await;
    let size = 4 * MIB;
    let opts = FileOpts {
        ranges: false,
        length: false,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "unsized.bin", 1))
        .await
        .expect("added");
    assert_eq!(rec.total_size, None);
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.total_size, Some(size));
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_without_ranges_restarts_from_zero() {
    let h = Harness::start().await;
    let size = 4 * MIB;
    let opts = FileOpts {
        ranges: false,
        rate: MIB,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "restart.bin", 4))
        .await
        .expect("added");
    wait_downloaded(&h.manager, rec.id, MIB, DONE).await;
    h.manager.pause(rec.id).await.expect("pause accepted");
    wait_for_status(&h.manager, rec.id, DownloadStatus::Paused, DONE).await;

    let before = h.server.state.stats().bytes_sent;
    h.manager.resume(rec.id).await.expect("resume accepted");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    let after_resume = h.server.state.stats().bytes_sent - before;
    assert!(
        after_resume >= size,
        "non-resumable download sent only {after_resume} bytes after resume"
    );
    h.manager.shutdown().await.expect("shutdown");
}
