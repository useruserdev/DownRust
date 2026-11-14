// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP level tests. They use a raw TCP client so the crate needs no extra dependencies.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use downrust_testserver::{file_url, full_content, FileOpts, TestServer};

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// True when the connection ended before the body was complete.
    cut: bool,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

fn request_text(path: &str, extra: &[(&str, &str)]) -> String {
    let mut text = format!("GET {path} HTTP/1.1\r\nHost: test\r\nConnection: close\r\n");
    for (name, value) in extra {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str("\r\n");
    text
}

fn get(server: &TestServer, path: &str, extra: &[(&str, &str)]) -> Reply {
    let mut stream = TcpStream::connect(server.addr).expect("connect to test server");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("set read timeout");
    stream
        .write_all(request_text(path, extra).as_bytes())
        .expect("send request");
    let mut raw = Vec::new();
    let read_failed = stream.read_to_end(&mut raw).is_err();
    parse(&raw, read_failed)
}

fn parse(raw: &[u8], read_failed: bool) -> Reply {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response head");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().expect("status line");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status code");
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect();
    let rest = &raw[split + 4..];
    let chunked = headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.contains("chunked"));
    let (body, cut) = if chunked {
        match dechunk(rest) {
            Some(body) => (body, read_failed),
            None => (rest.to_vec(), true),
        }
    } else {
        let declared = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, v)| v.parse::<usize>().ok());
        let short = declared.is_some_and(|n| rest.len() < n);
        (rest.to_vec(), read_failed || short)
    };
    Reply {
        status,
        headers,
        body,
        cut,
    }
}

fn dechunk(mut data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = data.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(&data[..line_end]).ok()?.trim();
        let size = usize::from_str_radix(size_text, 16).ok()?;
        data = &data[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        if data.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
}

#[test]
fn range_zero_to_nine_returns_partial_content() {
    let server = TestServer::start().expect("start");
    let reply = get(
        &server,
        &file_url(1000, &FileOpts::default()),
        &[("Range", "bytes=0-9")],
    );
    assert_eq!(reply.status, 206);
    assert_eq!(reply.header("Content-Range"), Some("bytes 0-9/1000"));
    assert_eq!(reply.header("Content-Length"), Some("10"));
    assert_eq!(reply.body, full_content(1, 1000)[..10]);
    assert_eq!(server.state.stats().range_requests, 1);
}

#[test]
fn middle_and_suffix_ranges_match_full_content() {
    let server = TestServer::start().expect("start");
    let expected = full_content(1, 1000);
    let mid = get(&server, "/file/1000", &[("Range", "bytes=500-599")]);
    assert_eq!(mid.status, 206);
    assert_eq!(mid.body, expected[500..600]);
    let tail = get(&server, "/file/1000", &[("Range", "bytes=-10")]);
    assert_eq!(tail.header("Content-Range"), Some("bytes 990-999/1000"));
    assert_eq!(tail.body, expected[990..]);
}

#[test]
fn full_get_returns_whole_file() {
    let server = TestServer::start().expect("start");
    let reply = get(&server, "/file/1000", &[]);
    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("Content-Length"), Some("1000"));
    assert_eq!(reply.header("Accept-Ranges"), Some("bytes"));
    assert_eq!(reply.body, full_content(1, 1000));
}

#[test]
fn ranges_disabled_ignores_range_header() {
    let server = TestServer::start().expect("start");
    let opts = FileOpts {
        ranges: false,
        ..FileOpts::default()
    };
    let reply = get(&server, &file_url(1000, &opts), &[("Range", "bytes=0-9")]);
    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("Content-Range"), None);
    assert_eq!(reply.body, full_content(1, 1000));
    assert_eq!(server.state.stats().range_requests, 0);
}

#[test]
fn range_outside_file_returns_416() {
    let server = TestServer::start().expect("start");
    let reply = get(&server, "/file/1000", &[("Range", "bytes=1000-")]);
    assert_eq!(reply.status, 416);
    assert_eq!(reply.header("Content-Range"), Some("bytes */1000"));
    assert!(reply.body.is_empty());
}

#[test]
fn unknown_length_uses_chunked_body() {
    let server = TestServer::start().expect("start");
    let opts = FileOpts {
        length: false,
        ..FileOpts::default()
    };
    let reply = get(&server, &file_url(5000, &opts), &[]);
    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("Content-Length"), None);
    assert!(!reply.cut);
    assert_eq!(reply.body, full_content(1, 5000));
}

#[test]
fn content_is_deterministic_and_follows_version() {
    let first = TestServer::start().expect("start");
    let second = TestServer::start().expect("start");
    let a = get(&first, "/file/4096", &[]);
    let b = get(&second, "/file/4096", &[]);
    assert_eq!(a.body, b.body);
    assert_eq!(a.header("ETag"), Some("\"v1-4096\""));

    first.state.set_version(2);
    let c = get(&first, "/file/4096", &[]);
    assert_eq!(c.header("ETag"), Some("\"v2-4096\""));
    assert_ne!(c.body, a.body);
    assert_eq!(c.body, full_content(2, 4096));
}

#[test]
fn if_range_mismatch_serves_full_file() {
    let server = TestServer::start().expect("start");
    let reply = get(
        &server,
        "/file/1000",
        &[("Range", "bytes=0-9"), ("If-Range", "\"v9-1000\"")],
    );
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body.len(), 1000);
}

#[test]
fn fail_injection_cuts_the_stream() {
    let server = TestServer::start().expect("start");
    server.state.fail_next(1, 4096);
    let reply = get(&server, "/file/100000", &[]);
    assert_eq!(reply.status, 200);
    assert!(reply.cut, "stream must end early");
    assert_eq!(reply.body.len(), 4096);
    assert_eq!(reply.body, full_content(1, 100_000)[..4096]);
    let next = get(&server, "/file/100", &[]);
    assert!(!next.cut);
    assert_eq!(next.body.len(), 100);
}

#[test]
fn redirect_answers_302_with_location() {
    let server = TestServer::start().expect("start");
    let reply = get(&server, "/redirect?to=%2Ffile%2F10", &[]);
    assert_eq!(reply.status, 302);
    assert_eq!(reply.header("Location"), Some("/file/10"));
}

#[test]
fn concurrency_limit_answers_503() {
    let server = TestServer::start().expect("start");
    server.state.set_max_concurrent(1);
    let mut slow = TcpStream::connect(server.addr).expect("connect");
    slow.write_all(request_text("/file/100000?rate=1000", &[]).as_bytes())
        .expect("send slow request");
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.state.stats().active < 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let reply = get(&server, "/file/10", &[]);
    assert_eq!(reply.status, 503);
    assert_eq!(reply.header("Retry-After"), Some("1"));
    drop(slow);
}

#[test]
fn unknown_path_returns_404() {
    let server = TestServer::start().expect("start");
    let reply = get(&server, "/nothing-here", &[]);
    assert_eq!(reply.status, 404);
}

#[test]
fn content_disposition_carries_both_names() {
    let server = TestServer::start().expect("start");
    let opts = FileOpts {
        name: Some("report.pdf".to_owned()),
        name_star: Some("résumé.pdf".to_owned()),
        ..FileOpts::default()
    };
    let reply = get(&server, &file_url(10, &opts), &[]);
    assert_eq!(
        reply.header("Content-Disposition"),
        Some("attachment; filename=\"report.pdf\"; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf")
    );
}
