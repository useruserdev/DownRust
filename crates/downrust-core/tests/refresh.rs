// SPDX-License-Identifier: GPL-3.0-or-later
//! Address refresh: a paused download continues from its saved progress through a new address.
mod common;

use std::time::Duration;

use common::{expected_sha, request, sha256_file, wait_downloaded, wait_for_status, Harness, MIB};
use downrust_core::error::Error;
use downrust_core::model::DownloadStatus;
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

fn slow_opts() -> FileOpts {
    FileOpts {
        rate: MIB,
        ..FileOpts::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn refreshed_address_resumes_with_saved_progress() {
    let h = Harness::start().await;
    let size = 8 * MIB;
    let url = h.server.url(&file_url(size, &slow_opts()));
    let rec = h
        .manager
        .add(request(&url, "moved.bin", 4))
        .await
        .expect("added");
    wait_downloaded(&h.manager, rec.id, 2 * MIB, DONE).await;
    h.manager.pause(rec.id).await.expect("pause");
    wait_for_status(&h.manager, rec.id, DownloadStatus::Paused, DONE).await;

    let mut redirect = url::Url::parse(&h.server.url("/redirect")).expect("redirect url");
    redirect
        .query_pairs_mut()
        .append_pair("to", &file_url(size, &slow_opts()));
    let refreshed = h
        .manager
        .refresh_address(rec.id, redirect.to_string())
        .await
        .expect("refreshed");
    assert_eq!(refreshed.url, redirect.to_string());
    assert!(refreshed.downloaded >= 2 * MIB);

    let before = h.server.state.stats().bytes_sent;
    h.manager.resume(rec.id).await.expect("resume");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    let stats = h.server.state.stats();
    assert!(
        stats.bytes_sent - before < size,
        "resume re-sent the whole file"
    );
    assert!(
        stats.bytes_sent < size * 16 / 10,
        "total sent {} exceeds 1.6 x size",
        stats.bytes_sent
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn address_of_a_different_file_is_refused() {
    let h = Harness::start().await;
    let size = 8 * MIB;
    let url = h.server.url(&file_url(size, &slow_opts()));
    let rec = h
        .manager
        .add(request(&url, "same.bin", 4))
        .await
        .expect("added");
    wait_downloaded(&h.manager, rec.id, MIB, DONE).await;
    h.manager.pause(rec.id).await.expect("pause");
    wait_for_status(&h.manager, rec.id, DownloadStatus::Paused, DONE).await;

    let other = h.server.url(&file_url(size / 2, &FileOpts::default()));
    let err = h
        .manager
        .refresh_address(rec.id, other)
        .await
        .expect_err("refused");
    match err {
        Error::InvalidInput(message) => {
            assert_eq!(message, "The new address points to a different file.")
        }
        other => panic!("unexpected error: {other}"),
    }
    let kept = h.manager.get(rec.id).await.expect("exists");
    assert_eq!(kept.url, url);
    assert_eq!(kept.status, DownloadStatus::Paused);
    h.manager.shutdown().await.expect("shutdown");
}
