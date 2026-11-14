// SPDX-License-Identifier: GPL-3.0-or-later
//! FTP downloads against an in-process libunftp server: exact bytes, pause and resume from the
//! saved offset, anonymous and user/password logins, and a refused password.
mod common;

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use common::{request, test_settings, wait_downloaded, wait_for_status, MIB};
use downrust_core::manager::DownloadManager;
use downrust_core::model::{AddRequest, DownloadStatus, EngineEvent};
use downrust_core::store::Store;
use libunftp::ServerBuilder;
use tempfile::TempDir;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use unftp_core::auth::{AuthenticationError, Authenticator, Credentials, Principal};
use unftp_sbe_fs::Filesystem;

const LIMIT: Duration = Duration::from_secs(60);
const USER: &str = "alice";
const PASSWORD: &str = "s3cret-pass";

/// Accepts every anonymous login. With an account set, only that user and password pass.
#[derive(Debug)]
struct Accounts {
    account: Option<(String, String)>,
}

#[async_trait]
impl Authenticator for Accounts {
    async fn authenticate(
        &self,
        username: &str,
        creds: &Credentials,
    ) -> Result<Principal, AuthenticationError> {
        match &self.account {
            None => Ok(Principal {
                username: username.to_owned(),
            }),
            Some((user, pass))
                if username == user && creds.password.as_deref() == Some(pass.as_str()) =>
            {
                Ok(Principal {
                    username: username.to_owned(),
                })
            }
            Some(_) => Err(AuthenticationError::BadPassword),
        }
    }
}

/// Deterministic content of `size` bytes.
fn content(size: u64) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

/// Starts an FTP server on 127.0.0.1 that serves `file_name` from a temporary folder.
/// Returns its address and the folder, which must stay alive for the test.
async fn start_server(
    file_name: &str,
    data: &[u8],
    account: Option<(&str, &str)>,
) -> (SocketAddr, TempDir) {
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("free port")
        .local_addr()
        .expect("local address")
        .port();
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::write(root.path().join(file_name), data).expect("file written");
    let folder = root.path().to_path_buf();
    let auth = Accounts {
        account: account.map(|(user, pass)| (user.to_owned(), pass.to_owned())),
    };
    let server = ServerBuilder::with_authenticator(
        Box::new(move || Filesystem::new(folder.clone()).expect("served folder")),
        Arc::new(auth),
    )
    .build()
    .expect("server builds");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tokio::spawn(async move {
        let _ = server.listen(addr.to_string()).await;
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(addr).is_err() {
        assert!(Instant::now() < deadline, "FTP server did not start");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    (addr, root)
}

/// A manager with its own store and download folder.
async fn start_manager() -> (
    DownloadManager,
    Store,
    TempDir,
    UnboundedReceiver<EngineEvent>,
) {
    let dir = tempfile::tempdir().expect("download folder");
    let store = Store::open_in_memory().expect("store opens");
    let (tx, events) = unbounded_channel();
    let manager = DownloadManager::start(store.clone(), test_settings(dir.path()), tx)
        .await
        .expect("manager starts");
    (manager, store, dir, events)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn anonymous_download_has_exact_bytes() {
    let size = 3 * MIB + 17;
    let data = content(size);
    let (addr, _root) = start_server("data.bin", &data, None).await;
    let (manager, _store, dir, _events) = start_manager().await;

    let url = format!("ftp://{addr}/data.bin");
    let rec = manager
        .add(request(&url, "data.bin", 1))
        .await
        .expect("added");
    let done = wait_for_status(&manager, rec.id, DownloadStatus::Completed, LIMIT).await;
    assert_eq!(done.downloaded, size);
    assert_eq!(
        std::fs::read(dir.path().join("data.bin")).expect("file exists"),
        data
    );
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn user_and_password_in_address_log_in() {
    let size = MIB + 5;
    let data = content(size);
    let (addr, _root) = start_server("account.bin", &data, Some((USER, PASSWORD))).await;
    let (manager, _store, dir, _events) = start_manager().await;

    let url = format!("ftp://{USER}:{PASSWORD}@{addr}/account.bin");
    let rec = manager
        .add(request(&url, "account.bin", 1))
        .await
        .expect("added");
    wait_for_status(&manager, rec.id, DownloadStatus::Completed, LIMIT).await;
    assert_eq!(
        std::fs::read(dir.path().join("account.bin")).expect("file exists"),
        data
    );
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn wrong_password_fails_with_clear_message() {
    let data = content(64 * 1024);
    let (addr, _root) = start_server("secret.bin", &data, Some((USER, PASSWORD))).await;
    let (manager, _store, _dir, _events) = start_manager().await;

    let url = format!("ftp://{USER}:wrong-pass@{addr}/secret.bin");
    let rec = manager
        .add(request(&url, "secret.bin", 1))
        .await
        .expect("added");
    let failed = wait_for_status(&manager, rec.id, DownloadStatus::Error, LIMIT).await;
    let message = failed.error.expect("error message kept");
    assert!(
        message.contains("rejected the login"),
        "unexpected message: {message}"
    );
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn paused_download_resumes_from_saved_offset() {
    let size = 2 * MIB;
    let data = content(size);
    let (addr, _root) = start_server("big.bin", &data, None).await;
    let (manager, store, dir, _events) = start_manager().await;

    let url = format!("ftp://{addr}/big.bin");
    let rec = manager
        .add(AddRequest {
            url,
            file_name: Some("big.bin".to_owned()),
            connections: Some(1),
            speed_limit: Some(512 * 1024),
            start: true,
            ..AddRequest::default()
        })
        .await
        .expect("added");
    wait_downloaded(&manager, rec.id, 256 * 1024, LIMIT).await;
    manager.pause(rec.id).await.expect("pause accepted");
    wait_for_status(&manager, rec.id, DownloadStatus::Paused, LIMIT).await;

    let part = dir.path().join("big.bin.downrust");
    let part_len = std::fs::metadata(&part).expect("part file exists").len();
    assert!(
        part_len > 0 && part_len < size,
        "part file has {part_len} bytes"
    );
    let saved = store
        .get_download(rec.id)
        .expect("query works")
        .expect("row exists")
        .downloaded;
    assert_eq!(saved, part_len, "saved progress must match the part file");

    manager.resume(rec.id).await.expect("resume accepted");
    let done = wait_for_status(&manager, rec.id, DownloadStatus::Completed, LIMIT).await;
    assert_eq!(done.downloaded, size);
    assert_eq!(
        std::fs::read(dir.path().join("big.bin")).expect("file exists"),
        data
    );
    assert!(!Path::new(&part).exists());
    manager.shutdown().await.expect("shutdown");
}
