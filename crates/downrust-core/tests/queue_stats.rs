// SPDX-License-Identifier: GPL-3.0-or-later
//! Queue-empty event and the statistics snapshot after completions.
mod common;

use std::time::{Duration, Instant};

use common::{request, wait_for_status, Harness, MIB};
use downrust_core::model::{Category, DownloadStatus, EngineEvent, StatsTotals};
use downrust_core::util::time::local_today;
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queue_idle_follows_a_completion() {
    let mut h = Harness::start().await;
    let size = MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "idle.bin", 2))
        .await
        .expect("added");
    wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;

    let deadline = Instant::now() + DONE;
    let mut seen = false;
    while !seen && Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), h.events.recv()).await {
            Ok(Some(EngineEvent::QueueIdle)) => seen = true,
            Ok(Some(_)) | Err(_) => {}
            Ok(None) => break,
        }
    }
    assert!(seen, "QueueIdle was not sent after the completion");
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queue_idle_is_not_sent_without_a_completion() {
    let mut h = Harness::start().await;
    let url = h.server.url(&file_url(MIB, &FileOpts::default()));
    let mut paused = request(&url, "paused.bin", 1);
    paused.start = false;
    h.manager.add(paused).await.expect("added");
    let quiet = tokio::time::timeout(Duration::from_millis(500), async {
        loop {
            match h.events.recv().await {
                Some(EngineEvent::QueueIdle) => return true,
                Some(_) => {}
                None => return false,
            }
        }
    })
    .await;
    assert!(quiet.is_err(), "QueueIdle was sent without a completion");
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn completion_counts_on_today_and_all_time() {
    let h = Harness::start().await;
    let size = 2 * MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "counted.bin", 2))
        .await
        .expect("added");
    wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;

    let snapshot = h.manager.stats().expect("stats");
    assert_eq!(snapshot.days.len(), 30);
    let today = snapshot.days.last().expect("today is the last day");
    assert_eq!(today.day, local_today());
    assert_eq!((today.bytes, today.files), (size, 1));
    assert_eq!(
        snapshot.all_time,
        StatsTotals {
            bytes: size,
            files: 1
        }
    );
    let other = snapshot
        .by_category
        .iter()
        .find(|entry| entry.category == Category::Other)
        .expect("other category is listed");
    assert_eq!((other.bytes, other.files), (size, 1));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn empty_history_is_zero_filled() {
    let h = Harness::start().await;
    let snapshot = h.manager.stats().expect("stats");
    assert_eq!(snapshot.days.len(), 30);
    assert!(snapshot
        .days
        .iter()
        .all(|day| day.bytes == 0 && day.files == 0));
    assert!(snapshot
        .days
        .windows(2)
        .all(|pair| pair[0].day < pair[1].day));
    assert_eq!(snapshot.by_category.len(), Category::ALL.len());
    assert_eq!(snapshot.all_time, StatsTotals::default());
    h.manager.shutdown().await.expect("shutdown");
}
