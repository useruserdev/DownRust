// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP Basic logins for sites: no login, a correct login and a refused login.
mod common;

use std::time::{Duration, Instant};

use common::{expected_sha, request, sha256_file, test_settings, wait_for_status, MIB};
use downrust_core::manager::DownloadManager;
use downrust_core::model::{DownloadId, DownloadStatus};
use downrust_core::secrets;
use downrust_core::settings::SiteLogin;
use downrust_core::store::Store;
use downrust_testserver::TestServer;
use tokio::sync::mpsc::unbounded_channel;

const DONE: Duration = Duration::from_secs(60);
const NO_LOGIN_MESSAGE: &str = "This site needs a login. Add it in Settings → Sites.";

async fn start(dir: &std::path::Path, logins: Vec<SiteLogin>) -> DownloadManager {
    let mut settings = test_settings(dir);
    settings.site_logins = logins;
    let store = Store::open_in_memory().expect("store opens");
    let (tx, _events) = unbounded_channel();
    DownloadManager::start(store, settings, tx)
        .await
        .expect("manager starts")
}

/// Polls until the download is in error and returns the error text.
async fn wait_for_error(manager: &DownloadManager, id: DownloadId) -> String {
    let deadline = Instant::now() + DONE;
    loop {
        let record = manager.get(id).await.expect("download exists");
        if record.status == DownloadStatus::Error {
            return record.error.unwrap_or_default();
        }
        assert!(Instant::now() < deadline, "download {id} never failed");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn protected_file_without_login_reports_the_missing_login() {
    let server = TestServer::start().expect("test server starts");
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = start(dir.path(), Vec::new()).await;
    let url = server.url(&format!("/auth/file/{}", MIB));
    let rec = manager
        .add(request(&url, "locked.bin", 2))
        .await
        .expect("added");
    assert_eq!(wait_for_error(&manager, rec.id).await, NO_LOGIN_MESSAGE);
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn correct_login_completes_with_matching_hash() {
    secrets::use_memory_backend();
    secrets::set(&secrets::site_user("127.0.0.1"), Some("secret")).expect("store password");
    let server = TestServer::start().expect("test server starts");
    let dir = tempfile::tempdir().expect("temp dir");
    let logins = vec![SiteLogin {
        host: "127.0.0.1".to_owned(),
        username: "demo".to_owned(),
    }];
    let manager = start(dir.path(), logins).await;
    let size = 2 * MIB;
    let url = server.url(&format!("/auth/file/{size}"));
    let rec = manager
        .add(request(&url, "allowed.bin", 4))
        .await
        .expect("added");
    let done = wait_for_status(&manager, rec.id, DownloadStatus::Completed, DONE).await;
    assert_eq!(sha256_file(&done.final_path()), expected_sha(1, size));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn refused_login_reports_the_rejection() {
    // A wildcard pattern that matches 127.0.0.1 gets its own stored password.
    secrets::use_memory_backend();
    secrets::set(&secrets::site_user("*.0.0.1"), Some("wrong")).expect("store password");
    let server = TestServer::start().expect("test server starts");
    let dir = tempfile::tempdir().expect("temp dir");
    let logins = vec![SiteLogin {
        host: "*.0.0.1".to_owned(),
        username: "demo".to_owned(),
    }];
    let manager = start(dir.path(), logins).await;
    let url = server.url(&format!("/auth/file/{}", MIB));
    let rec = manager
        .add(request(&url, "refused.bin", 2))
        .await
        .expect("added");
    assert_eq!(
        wait_for_error(&manager, rec.id).await,
        "The site rejected the login for 127.0.0.1."
    );
    manager.shutdown().await.expect("shutdown");
}
