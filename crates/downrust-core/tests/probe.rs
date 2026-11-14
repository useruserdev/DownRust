// SPDX-License-Identifier: GPL-3.0-or-later
//! Address probing against the local test server.
mod common;

use std::time::Duration;

use downrust_core::http::client::build_client;
use downrust_core::http::probe::probe;
use downrust_core::model::Category;
use downrust_core::Error;
use downrust_testserver::{etag, file_url, FileOpts, TestServer};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

const TIMEOUT: Duration = Duration::from_secs(5);
const SIZE: u64 = 100_000;

fn client() -> reqwest::Client {
    let settings = common::test_settings(&std::env::temp_dir());
    build_client(&settings).expect("client builds")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn range_reply_gives_size_and_resumable() {
    let server = TestServer::start().expect("server starts");
    let url = server.url(&file_url(SIZE, &FileOpts::default()));
    let info = probe(&client(), &url, &[], TIMEOUT)
        .await
        .expect("probe succeeds");
    assert_eq!(info.total_size, Some(SIZE));
    assert!(info.resumable);
    assert_eq!(info.etag, Some(etag(1, SIZE)));
    assert_eq!(info.final_url, url);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disabled_ranges_are_not_resumable() {
    let server = TestServer::start().expect("server starts");
    let opts = FileOpts {
        ranges: false,
        ..FileOpts::default()
    };
    let url = server.url(&file_url(SIZE, &opts));
    let info = probe(&client(), &url, &[], TIMEOUT)
        .await
        .expect("probe succeeds");
    assert_eq!(info.total_size, Some(SIZE));
    assert!(!info.resumable);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_length_without_ranges_has_no_size() {
    let server = TestServer::start().expect("server starts");
    let opts = FileOpts {
        ranges: false,
        length: false,
        ..FileOpts::default()
    };
    let url = server.url(&file_url(SIZE, &opts));
    let info = probe(&client(), &url, &[], TIMEOUT)
        .await
        .expect("probe succeeds");
    assert_eq!(info.total_size, None);
    assert!(!info.resumable);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plain_content_disposition_gives_file_name() {
    let server = TestServer::start().expect("server starts");
    let opts = FileOpts {
        name: Some("report.pdf".to_owned()),
        ..FileOpts::default()
    };
    let url = server.url(&file_url(SIZE, &opts));
    let info = probe(&client(), &url, &[], TIMEOUT)
        .await
        .expect("probe succeeds");
    assert_eq!(info.file_name, "report.pdf");
    assert_eq!(info.category, Category::Documents);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn utf8_filename_star_is_decoded() {
    let server = TestServer::start().expect("server starts");
    let opts = FileOpts {
        name_star: Some("résumé.pdf".to_owned()),
        ..FileOpts::default()
    };
    let url = server.url(&file_url(SIZE, &opts));
    let info = probe(&client(), &url, &[], TIMEOUT)
        .await
        .expect("probe succeeds");
    assert_eq!(info.file_name, "résumé.pdf");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn redirect_is_followed() {
    let server = TestServer::start().expect("server starts");
    let opts = FileOpts {
        name: Some("report.pdf".to_owned()),
        ..FileOpts::default()
    };
    let target = file_url(SIZE, &opts);
    let encoded = utf8_percent_encode(&target, NON_ALPHANUMERIC);
    let url = server.url(&format!("/redirect?to={encoded}"));
    let info = probe(&client(), &url, &[], TIMEOUT)
        .await
        .expect("probe succeeds");
    assert!(info.final_url.contains("/file/"), "{}", info.final_url);
    assert_ne!(info.final_url, url);
    assert_eq!(info.total_size, Some(SIZE));
    assert!(info.resumable);
    assert_eq!(info.file_name, "report.pdf");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn not_found_is_an_http_error() {
    let server = TestServer::start().expect("server starts");
    let url = server.url("/missing/file.bin");
    let result = probe(&client(), &url, &[], TIMEOUT).await;
    assert!(
        matches!(result, Err(Error::HttpStatus(404))),
        "unexpected result: {result:?}"
    );
}
