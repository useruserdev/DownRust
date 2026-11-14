// SPDX-License-Identifier: GPL-3.0-or-later
//! Time window: downloads wait while the window is closed and return to the queue when it closes.
mod common;

use std::time::Duration;

use chrono::Timelike;
use common::{expected_sha, request, sha256_file, wait_downloaded, wait_for_status, Harness, MIB};
use downrust_core::model::DownloadStatus;
use downrust_core::settings::Schedule;
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

/// An enabled window that starts one hour after the current local time and lasts one hour.
/// It never includes the present moment.
fn closed_window() -> Schedule {
    let now = chrono::Local::now();
    let minute = u16::try_from(now.hour() * 60 + now.minute()).expect("minute of day");
    Schedule {
        enabled: true,
        start_minute: (minute + 60) % 1440,
        stop_minute: (minute + 120) % 1440,
        ..Schedule::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn download_waits_while_the_window_is_closed() {
    let h = Harness::start_with(|s| s.schedule = closed_window()).await;
    let size = MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "waiting.bin", 2))
        .await
        .expect("added");

    tokio::time::sleep(Duration::from_millis(800)).await;
    let waiting = h.manager.get(rec.id).await.expect("exists");
    assert_eq!(waiting.status, DownloadStatus::Queued);
    assert_eq!(waiting.downloaded, 0);

    let mut settings = h.manager.settings();
    settings.schedule.enabled = false;
    h.manager.update_settings(settings).await.expect("update");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn running_download_returns_to_queue_when_the_window_closes() {
    let h = Harness::start().await;
    let size = 4 * MIB;
    let opts = FileOpts {
        rate: MIB,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "interrupted.bin", 1))
        .await
        .expect("added");
    wait_downloaded(&h.manager, rec.id, MIB / 2, DONE).await;

    let mut settings = h.manager.settings();
    settings.schedule = closed_window();
    h.manager.update_settings(settings).await.expect("update");
    let queued = wait_for_status(&h.manager, rec.id, DownloadStatus::Queued, DONE).await;
    assert!(queued.downloaded > 0, "progress should be kept");

    let mut settings = h.manager.settings();
    settings.schedule.enabled = false;
    h.manager.update_settings(settings).await.expect("update");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}
