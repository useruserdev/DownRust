// SPDX-License-Identifier: GPL-3.0-or-later
//! Manual proxy routing: a download through the proxy completes, and a direct rule skips it.
mod common;

use std::time::Duration;

use common::{expected_sha, request, sha256_file, test_settings, wait_for_status, MIB};
use downrust_core::manager::DownloadManager;
use downrust_core::model::DownloadStatus;
use downrust_core::settings::{ProxyMode, ProxyRule, ProxyRuleAction, ProxySettings};
use downrust_core::store::Store;
use downrust_testserver::{file_url, FileOpts, TestServer};
use tokio::sync::mpsc::unbounded_channel;

const DONE: Duration = Duration::from_secs(60);

/// Starts a manager whose manual proxy is the test server itself.
async fn start_through_proxy(
    server: &TestServer,
    dir: &std::path::Path,
    rules: Vec<ProxyRule>,
) -> DownloadManager {
    let mut settings = test_settings(dir);
    settings.proxy = ProxySettings {
        mode: ProxyMode::Manual,
        url: format!("http://{}", server.addr),
        username: String::new(),
        rules,
    };
    let store = Store::open_in_memory().expect("store opens");
    let (tx, _events) = unbounded_channel();
    DownloadManager::start(store, settings, tx)
        .await
        .expect("manager starts")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn download_through_manual_proxy_completes() {
    let server = TestServer::start().expect("test server starts");
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = start_through_proxy(&server, dir.path(), Vec::new()).await;
    let size = 2 * MIB;
    let url = server.url(&file_url(size, &FileOpts::default()));
    let rec = manager
        .add(request(&url, "proxied.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    assert!(server.state.stats().proxied_requests > 0);
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn direct_rule_bypasses_the_proxy() {
    let server = TestServer::start().expect("test server starts");
    let dir = tempfile::tempdir().expect("temp dir");
    let rules = vec![ProxyRule {
        host: "127.0.0.1".to_owned(),
        action: ProxyRuleAction::Direct,
    }];
    let manager = start_through_proxy(&server, dir.path(), rules).await;
    let size = 2 * MIB;
    let url = server.url(&file_url(size, &FileOpts::default()));
    let rec = manager
        .add(request(&url, "direct.bin", 2))
        .await
        .expect("added");
    let done = wait_for_status(&manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    assert_eq!(server.state.stats().proxied_requests, 0);
    manager.shutdown().await.expect("shutdown");
}
