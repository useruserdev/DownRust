// SPDX-License-Identifier: GPL-3.0-or-later
//! HLS downloads against the local fixtures: variant choice, AES-128, byte ranges, live refusal,
//! kind detection and resume after a pause. Conversion is off, so ffmpeg is not needed.
mod common;

use std::time::Duration;

use common::{wait_for_status, Harness};
use downrust_core::model::{AddRequest, DownloadKind, DownloadStatus};
use downrust_core::settings::MediaQuality;
use downrust_testserver::{hls_variant_bytes, HLS_SEGMENTS};
use sha2::{Digest, Sha256};

const DONE: Duration = Duration::from_secs(60);

async fn harness(quality: MediaQuality) -> Harness {
    Harness::start_with(move |settings| {
        settings.media.convert_to_mp4 = false;
        settings.media.quality = quality;
    })
    .await
}

fn hls_request(url: &str, name: &str, connections: u8) -> AddRequest {
    AddRequest {
        url: url.to_owned(),
        file_name: Some(name.to_owned()),
        connections: Some(connections),
        kind: Some(DownloadKind::Hls),
        start: true,
        ..AddRequest::default()
    }
}

fn hex_sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn file_sha(path: &std::path::Path) -> String {
    hex_sha(&std::fs::read(path).expect("output exists"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn best_variant_is_joined_in_order() {
    let h = harness(MediaQuality::Best).await;
    let url = h.server.url("/hls/master.m3u8");
    let rec = h
        .manager
        .add(hls_request(&url, "best.ts", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.kind, DownloadKind::Hls);
    assert_eq!(
        file_sha(&done.final_path()),
        hex_sha(&hls_variant_bytes("high"))
    );
    assert_eq!(done.downloaded, hls_variant_bytes("high").len() as u64);
    assert!(!h.dir.path().join("best.ts.downrust-media").exists());
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn worst_quality_picks_the_low_variant() {
    let h = harness(MediaQuality::Worst).await;
    let url = h.server.url("/hls/master.m3u8");
    let rec = h
        .manager
        .add(hls_request(&url, "worst.ts", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(
        file_sha(&done.final_path()),
        hex_sha(&hls_variant_bytes("low"))
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aes_128_segments_are_decrypted() {
    let h = harness(MediaQuality::Best).await;
    let url = h.server.url("/hls/enc/index.m3u8");
    let rec = h
        .manager
        .add(hls_request(&url, "enc.ts", 4))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(
        file_sha(&done.final_path()),
        hex_sha(&hls_variant_bytes("enc"))
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn byte_ranges_of_one_file_are_joined() {
    let h = harness(MediaQuality::Best).await;
    let url = h.server.url("/hls/br/index.m3u8");
    let rec = h
        .manager
        .add(hls_request(&url, "range.ts", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(
        file_sha(&done.final_path()),
        hex_sha(&hls_variant_bytes("br"))
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn live_playlist_is_refused_with_a_clear_message() {
    let h = harness(MediaQuality::Best).await;
    let url = h.server.url("/hls/live/index.m3u8");
    let rec = h
        .manager
        .add(hls_request(&url, "live.ts", 2))
        .await
        .expect("added");
    let failed = wait_for_status(&h.manager, rec.id, DownloadStatus::Error, DONE).await;
    assert_eq!(
        failed.error.as_deref(),
        Some("Live streams are not supported yet.")
    );
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn playlist_address_sets_the_kind_when_none_is_given() {
    let h = harness(MediaQuality::Best).await;
    let url = h.server.url("/hls/master.m3u8");
    let rec = h
        .manager
        .add(AddRequest {
            url,
            start: false,
            ..AddRequest::default()
        })
        .await
        .expect("added");
    assert_eq!(rec.kind, DownloadKind::Hls);
    assert_eq!(rec.file_name, "master.ts");
    h.manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_and_resume_do_not_fetch_finished_parts_again() {
    let h = harness(MediaQuality::Worst).await;
    h.server.state.set_hls_delay_ms(150);
    let url = h.server.url("/hls/low/index.m3u8");
    let rec = h
        .manager
        .add(hls_request(&url, "pause.ts", 1))
        .await
        .expect("added");
    wait_for_requests(&h, 3).await;
    h.manager.pause(rec.id).await.expect("paused");
    wait_for_status(&h.manager, rec.id, DownloadStatus::Paused, DONE).await;
    let before = h.server.state.hls_segment_requests();

    h.server.state.set_hls_delay_ms(0);
    h.manager.resume(rec.id).await.expect("resumed");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;

    assert_eq!(
        file_sha(&done.final_path()),
        hex_sha(&hls_variant_bytes("low"))
    );
    // At least two parts were stored before the pause, so resume asks for at most the rest.
    let after = h.server.state.hls_segment_requests() - before;
    assert!(
        after <= HLS_SEGMENTS as u64 - 2,
        "resume asked for {after} parts"
    );
    h.manager.shutdown().await.expect("shutdown");
}

async fn wait_for_requests(h: &Harness, count: u64) {
    let deadline = tokio::time::Instant::now() + DONE;
    while h.server.state.hls_segment_requests() < count {
        assert!(
            tokio::time::Instant::now() < deadline,
            "no segment requests"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs ffmpeg on PATH or next to the program"]
async fn hls_converts_to_mp4_with_ffmpeg() {
    let h = Harness::start_with(|settings| {
        settings.media.convert_to_mp4 = true;
    })
    .await;
    let url = h.server.url("/hls/low/index.m3u8");
    let rec = h
        .manager
        .add(hls_request(&url, "convert.ts", 2))
        .await
        .expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert!(done
        .final_path()
        .extension()
        .is_some_and(|ext| ext == "mp4"));
    assert!(done.final_path().exists());
    h.manager.shutdown().await.expect("shutdown");
}
