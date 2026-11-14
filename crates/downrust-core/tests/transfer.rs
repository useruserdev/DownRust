// SPDX-License-Identifier: GPL-3.0-or-later
//! Segmented transfers: completion and hash, dynamic splitting, empty and unknown-length files,
//! category folders and the per-download speed limit.
mod common;

use std::time::{Duration, Instant};

use common::{drain_events, expected_sha, request, sha256_file, wait_for_status, Harness, MIB};
use downrust_core::model::{Category, DownloadStatus, EngineEvent};
use downrust_testserver::{file_url, FileOpts};

const DONE: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn segmented_download_completes_with_matching_hash() {
    let h = Harness::start().await;
    let size = 8 * MIB;
    let opts = FileOpts {
        rate: 4 * MIB,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "segmented.bin", 8))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    let stats = h.server.state.stats();
    assert!(stats.peak_active >= 2, "peak {}", stats.peak_active);
    assert!(
        stats.range_requests >= 8,
        "range requests {}",
        stats.range_requests
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn finished_segment_splits_a_running_one() {
    let mut h = Harness::start().await;
    let size = 16 * MIB;
    let opts = FileOpts {
        rate: 2 * MIB,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "split.bin", 3))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    let most_segments = drain_events(&mut h.events)
        .iter()
        .filter_map(|event| match event {
            EngineEvent::Progress(batch) => Some(batch),
            _ => None,
        })
        .flat_map(|batch| batch.items.iter())
        .filter(|item| item.id == rec.id)
        .map(|item| item.segments.len())
        .max()
        .unwrap_or(0);
    assert!(
        most_segments > 3,
        "expected more than 3 segments after splitting, saw at most {most_segments}"
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_length_single_stream_completes() {
    let h = Harness::start().await;
    let size = 3 * MIB + 17;
    let opts = FileOpts {
        ranges: false,
        length: false,
        ..FileOpts::default()
    };
    let url = h.server.url(&file_url(size, &opts));
    let rec = h
        .manager
        .add(request(&url, "stream.bin", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert!(!done.resumable);
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn zero_byte_file_completes_empty() {
    let h = Harness::start().await;
    let url = h.server.url(&file_url(0, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "empty.bin", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    let meta = std::fs::metadata(done.final_path()).expect("file exists");
    assert_eq!(meta.len(), 0);
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn video_lands_in_its_category_folder() {
    let h = Harness::start_with(|settings| {
        let video = std::path::PathBuf::from(&settings.default_dir).join("Video");
        settings.auto_sort_by_category = true;
        settings
            .category_dirs
            .insert(Category::Video, video.display().to_string());
    })
    .await;
    let size = 2 * MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let rec = h
        .manager
        .add(request(&url, "clip.mp4", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    let expected_path = h.dir.path().join("Video").join("clip.mp4");
    assert_eq!(done.final_path(), expected_path);
    assert_eq!(sha256_file(&expected_path), expected_sha(1, size));
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn per_download_speed_limit_is_kept() {
    let h = Harness::start().await;
    let size = 2 * MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let mut req = request(&url, "limited.bin", 4);
    req.speed_limit = Some(512 * 1024);
    let started = Instant::now();
    let rec = h.manager.add(req).await.expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    let elapsed = started.elapsed();
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    assert!(
        elapsed >= Duration::from_secs(3) && elapsed <= Duration::from_secs(8),
        "2 MiB at 512 KiB/s took {elapsed:?}"
    );
    h.manager.shutdown().await.expect("shutdown");
}
