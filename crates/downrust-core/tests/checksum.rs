// SPDX-License-Identifier: GPL-3.0-or-later
//! SHA-256 checks: expected values given at add time, and a manual check of a finished download.
mod common;

use std::time::Duration;

use common::{expected_sha, request, wait_for_status, Harness, MIB};
use downrust_core::error::Error;
use downrust_core::model::{AddRequest, ChecksumState, DownloadStatus};
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

fn with_checksum(url: &str, name: &str, expected: &str) -> AddRequest {
    AddRequest {
        expected_sha256: Some(expected.to_owned()),
        ..request(url, name, 2)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn matching_checksum_completes_the_download() {
    let h = Harness::start().await;
    let size = MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let expected = expected_sha(1, size).to_ascii_uppercase();
    let rec = h
        .manager
        .add(with_checksum(&url, "good.bin", &expected))
        .await
        .expect("added");
    assert_eq!(rec.expected_sha256, Some(expected.to_ascii_lowercase()));
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.checksum, Some(ChecksumState::Ok));
    assert_eq!(done.error, None);
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_checksum_fails_the_download() {
    let h = Harness::start().await;
    let size = MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let wrong = "0".repeat(64);
    let rec = h
        .manager
        .add(with_checksum(&url, "bad.bin", &wrong))
        .await
        .expect("added");
    let failed = wait_for_status(&h.manager, rec.id, DownloadStatus::Error, DONE).await;
    assert_eq!(failed.checksum, Some(ChecksumState::Mismatch));
    let message = failed.error.expect("error text");
    assert!(
        message.starts_with("Checksum mismatch: expected 000000"),
        "unexpected message: {message}"
    );
    assert!(message.contains(&expected_sha(1, size)));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn malformed_checksum_is_rejected_at_add() {
    let h = Harness::start().await;
    let url = h.server.url(&file_url(MIB, &FileOpts::default()));
    let err = h
        .manager
        .add(with_checksum(&url, "short.bin", "abc123"))
        .await
        .expect_err("rejected");
    match err {
        Error::InvalidInput(message) => {
            assert_eq!(message, "The SHA-256 must be 64 hexadecimal characters.")
        }
        other => panic!("unexpected error: {other}"),
    }
    assert!(h.manager.list().await.expect("list").is_empty());
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn verify_checksum_on_a_completed_download() {
    let h = Harness::start().await;
    let size = MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "manual.bin", 2))
        .await
        .expect("added");
    wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;

    let report = h
        .manager
        .verify_checksum(rec.id, expected_sha(1, size))
        .await
        .expect("verified");
    assert!(report.matches);
    assert_eq!(report.actual, expected_sha(1, size));
    let stored = h.manager.get(rec.id).await.expect("exists");
    assert_eq!(stored.checksum, Some(ChecksumState::Ok));
    assert_eq!(stored.expected_sha256, Some(expected_sha(1, size)));

    let report = h
        .manager
        .verify_checksum(rec.id, "f".repeat(64))
        .await
        .expect("verified");
    assert!(!report.matches);
    let stored = h.manager.get(rec.id).await.expect("exists");
    assert_eq!(stored.checksum, Some(ChecksumState::Mismatch));
    assert_eq!(stored.status, DownloadStatus::Completed);
    h.manager.shutdown().await.expect("shutdown");
}
