// SPDX-License-Identifier: GPL-3.0-or-later
//! Pause and resume of segmented downloads: saved progress is kept, and a changed file restarts.
mod common;

use std::time::Duration;

use common::{expected_sha, request, sha256_file, wait_downloaded, wait_for_status, Harness, MIB};
use downrust_core::model::DownloadStatus;
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

/// Starts an 8 MiB download at 1 MiB/s per connection, pauses it after 2 MiB and returns its id.
async fn start_and_pause(h: &Harness, size: u64) -> i64 {
    let opts = FileOpts {
        rate: MIB,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "resume.bin", 4))
        .await
        .expect("added");
    wait_downloaded(&h.manager, rec.id, 2 * MIB, DONE).await;
    h.manager.pause(rec.id).await.expect("pause accepted");
    let paused = wait_for_status(&h.manager, rec.id, DownloadStatus::Paused, DONE).await;
    assert!(paused.downloaded >= 2 * MIB, "kept {}", paused.downloaded);
    rec.id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resume_continues_from_saved_progress() {
    let h = Harness::start().await;
    let size = 8 * MIB;
    let id = start_and_pause(&h, size).await;

    let paused = h.manager.get(id).await.expect("download exists");
    let saved = h.store.load_segments(id).expect("segments load");
    assert!(!saved.is_empty(), "segment rows must be saved on pause");
    let written: u64 = saved.iter().map(|s| s.written).sum();
    assert!(written <= paused.downloaded);

    let before = h.server.state.stats().bytes_sent;
    h.manager.resume(id).await.expect("resume accepted");
    let done = wait_for_status(&h.manager, id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));

    let stats = h.server.state.stats();
    let after_resume = stats.bytes_sent - before;
    assert!(
        after_resume < size,
        "resume re-sent {after_resume} bytes of {size}"
    );
    assert!(
        stats.bytes_sent < size * 3 / 2,
        "total sent {} exceeds 1.5 x size",
        stats.bytes_sent
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changed_file_restarts_from_zero() {
    let h = Harness::start().await;
    let size = 8 * MIB;
    let id = start_and_pause(&h, size).await;

    h.server.state.set_version(2);
    let before = h.server.state.stats().bytes_sent;
    h.manager.resume(id).await.expect("resume accepted");
    let done = wait_for_status(&h.manager, id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(2, size));

    let after_resume = h.server.state.stats().bytes_sent - before;
    assert!(
        after_resume >= size,
        "restart sent only {after_resume} bytes of {size}"
    );
    h.manager.shutdown().await.expect("shutdown");
}
