// SPDX-License-Identifier: GPL-3.0-or-later
//! Queue behaviour: concurrency limit, pause and resume of all, removal, and the global speed limit.
mod common;

use std::time::{Duration, Instant};

use common::{
    drain_events, expected_sha, request, sha256_file, wait_downloaded, wait_for_status, Harness,
    MIB,
};
use downrust_core::model::{DownloadId, DownloadStatus, EngineEvent};
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

fn slow_url(h: &Harness, size: u64) -> String {
    let opts = FileOpts {
        rate: MIB,
        ..FileOpts::default()
    };
    h.server.url(&file_url(size, &opts))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn second_download_waits_for_the_first() {
    let h = Harness::start_with(|s| s.max_concurrent_downloads = 1).await;
    let size = 2 * MIB;
    let first = h
        .manager
        .add(request(&slow_url(&h, size), "first.bin", 1))
        .await
        .expect("added");
    wait_for_status(&h.manager, first.id, DownloadStatus::Downloading, DONE).await;
    let second = h
        .manager
        .add(request(&slow_url(&h, size), "second.bin", 1))
        .await
        .expect("added");
    let waiting = h.manager.get(second.id).await.expect("exists");
    assert_eq!(waiting.status, DownloadStatus::Queued);

    wait_for_status(&h.manager, first.id, DownloadStatus::Completed, DONE).await;
    let done = wait_for_status(&h.manager, second.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn never_more_than_two_run_at_once() {
    let h = Harness::start_with(|s| s.max_concurrent_downloads = 2).await;
    let size = MIB;
    let mut ids: Vec<DownloadId> = Vec::new();
    for n in 0..4 {
        let rec = h
            .manager
            .add(request(&slow_url(&h, size), &format!("queued-{n}.bin"), 1))
            .await
            .expect("added");
        ids.push(rec.id);
    }
    let deadline = Instant::now() + DONE;
    let mut most_running = 0;
    loop {
        let mut running = 0;
        let mut finished = 0;
        for id in &ids {
            match h.manager.get(*id).await.expect("exists").status {
                DownloadStatus::Downloading => running += 1,
                DownloadStatus::Completed => finished += 1,
                _ => {}
            }
        }
        most_running = most_running.max(running);
        assert!(running <= 2, "{running} downloads running at once");
        if finished == ids.len() {
            break;
        }
        assert!(Instant::now() < deadline, "queue did not finish in time");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(most_running, 2, "two downloads should have run together");
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_all_then_resume_all_completes_every_download() {
    let h = Harness::start().await;
    let size = 4 * MIB;
    let mut ids: Vec<DownloadId> = Vec::new();
    for n in 0..3 {
        let rec = h
            .manager
            .add(request(&slow_url(&h, size), &format!("batch-{n}.bin"), 2))
            .await
            .expect("added");
        ids.push(rec.id);
    }
    for id in &ids {
        wait_downloaded(&h.manager, *id, MIB / 4, DONE).await;
    }
    h.manager.pause_all().await.expect("pause all");
    for id in &ids {
        wait_for_status(&h.manager, *id, DownloadStatus::Paused, DONE).await;
    }
    h.manager.resume_all().await.expect("resume all");
    for id in &ids {
        let done = wait_for_status(&h.manager, *id, DownloadStatus::Completed, DONE).await;
        assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    }
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remove_with_delete_removes_record_and_file() {
    let mut h = Harness::start().await;
    let size = MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "gone.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    let path = done.final_path();
    assert!(path.exists());
    h.manager.remove(rec.id, true).await.expect("removed");
    assert!(!path.exists(), "file should be deleted");
    assert!(
        h.manager.get(rec.id).await.is_err(),
        "record should be gone"
    );
    let removed = drain_events(&mut h.events)
        .iter()
        .any(|event| matches!(event, EngineEvent::Removed(id) if *id == rec.id));
    assert!(removed, "a Removed event should be sent");
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remove_without_delete_keeps_the_file() {
    let h = Harness::start().await;
    let size = MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "kept.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    let path = done.final_path();
    h.manager.remove(rec.id, false).await.expect("removed");
    assert!(path.exists(), "file should stay on disk");
    assert_eq!(sha256_file(&path), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn global_speed_limit_holds_the_speed() {
    let h = Harness::start_with(|s| s.global_speed_limit = Some(MIB)).await;
    let size = 4 * MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let started = Instant::now();
    let rec = h
        .manager
        .add(request(&url, "capped.bin", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    let elapsed = started.elapsed().as_secs_f64();
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    // 4 MiB at 1 MiB/s takes about 4 s. Accept 3.4 s to 6 s.
    assert!(
        (3.4..=6.0).contains(&elapsed),
        "4 MiB at 1 MiB/s took {elapsed:.2} s"
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn moved_download_starts_next_when_the_running_one_completes() {
    use downrust_core::model::QueueMove;

    let h = Harness::start_with(|s| s.max_concurrent_downloads = 1).await;
    let running = h
        .manager
        .add(request(&slow_url(&h, 4 * MIB), "running.bin", 1))
        .await
        .expect("added");
    wait_for_status(&h.manager, running.id, DownloadStatus::Downloading, DONE).await;
    let a = h
        .manager
        .add(request(&slow_url(&h, 2 * MIB), "a.bin", 1))
        .await
        .expect("added");
    let b = h
        .manager
        .add(request(&slow_url(&h, 2 * MIB), "b.bin", 1))
        .await
        .expect("added");
    let c = h
        .manager
        .add(request(&slow_url(&h, 2 * MIB), "c.bin", 1))
        .await
        .expect("added");

    assert!(
        h.manager
            .move_in_queue(running.id, QueueMove::Top)
            .await
            .is_err(),
        "a running download cannot be moved"
    );
    h.manager
        .move_in_queue(c.id, QueueMove::Top)
        .await
        .expect("moved to top");

    wait_for_status(&h.manager, running.id, DownloadStatus::Completed, DONE).await;
    wait_for_status(&h.manager, c.id, DownloadStatus::Downloading, DONE).await;
    assert_eq!(
        h.manager.get(a.id).await.expect("exists").status,
        DownloadStatus::Queued
    );
    assert_eq!(
        h.manager.get(b.id).await.expect("exists").status,
        DownloadStatus::Queued
    );
    h.manager.shutdown().await.expect("shutdown");
}
