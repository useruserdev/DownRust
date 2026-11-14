// SPDX-License-Identifier: GPL-3.0-or-later
//! Automatic SHA-256 check from a published file next to a download.
mod common;

use std::time::Duration;

use common::{request, wait_for_status, Harness};
use downrust_core::model::{ChecksumState, DownloadStatus};
use downrust_testserver::named_file_sha256;

const DONE: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sidecar_hash_is_found_and_checked() {
    let h = Harness::start().await;
    let url = h.server.url("/files/side-1.bin");
    let rec = h
        .manager
        .add(request(&url, "side-1.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.checksum, Some(ChecksumState::Ok));
    assert_eq!(done.expected_sha256, Some(named_file_sha256()));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hash_from_sums_file_is_checked() {
    let h = Harness::start().await;
    let url = h.server.url("/files/sums-known.bin");
    let rec = h
        .manager
        .add(request(&url, "sums-known.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.checksum, Some(ChecksumState::Ok));
    assert_eq!(done.expected_sha256, Some(named_file_sha256()));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn published_hash_that_differs_fails_the_download() {
    let h = Harness::start().await;
    let url = h.server.url("/files/bad-1.bin");
    let rec = h
        .manager
        .add(request(&url, "bad-1.bin", 2))
        .await
        .expect("added");
    let failed = wait_for_status(&h.manager, rec.id, DownloadStatus::Error, DONE).await;
    assert_eq!(failed.checksum, Some(ChecksumState::Mismatch));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_published_leaves_the_download_alone() {
    let h = Harness::start().await;
    let url = h.server.url("/files/none-1.bin");
    let rec = h
        .manager
        .add(request(&url, "none-1.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.checksum, None);
    assert_eq!(done.expected_sha256, None);
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn setting_off_skips_the_published_check() {
    let h = Harness::start_with(|settings| settings.auto_checksum = false).await;
    let url = h.server.url("/files/side-2.bin");
    let rec = h
        .manager
        .add(request(&url, "side-2.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.checksum, None);
    assert_eq!(done.expected_sha256, None);
    h.manager.shutdown().await.expect("shutdown");
}
