// SPDX-License-Identifier: GPL-3.0-or-later
//! Mirrors: one file taken from several addresses, with a mirror of the wrong size ignored.
mod common;

use std::time::Duration;

use common::{request, wait_for_status, Harness, MIB};
use downrust_core::model::{AddRequest, DownloadStatus};
use downrust_testserver::{file_url, full_content, FileOpts, TestServer};

const DONE: Duration = Duration::from_secs(60);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mirrors_share_the_connections_and_wrong_size_mirror_is_ignored() {
    let h = Harness::start().await;
    let size = 4 * MIB;
    let url = h.server.url(&file_url(size, &FileOpts::default()));
    let good = TestServer::start().expect("good mirror");
    let wrong = TestServer::start().expect("wrong mirror");
    let good_url = good.url(&file_url(size, &FileOpts::default()));
    let wrong_url = wrong.url(&file_url(size + 1, &FileOpts::default()));
    let add = AddRequest {
        mirrors: vec![good_url, wrong_url],
        ..request(&url, "mirror.bin", 4)
    };
    let rec = h.manager.add(add).await.expect("added");
    let done = wait_for_status(&h.manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(done.error, None);
    let bytes = std::fs::read(h.dir.path().join("mirror.bin")).expect("file");
    assert_eq!(bytes, full_content(1, size));
    assert!(
        good.state.stats().range_requests > 0,
        "the good mirror served no ranges"
    );
    assert!(
        h.server.state.stats().range_requests > 0,
        "the main address served no ranges"
    );
    // The wrong-size mirror gets one probe request and nothing else.
    assert!(
        wrong.state.stats().bytes_sent < 1024,
        "the wrong-size mirror must not serve segments"
    );
    h.manager.shutdown().await.expect("shutdown");
}
