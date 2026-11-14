// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP handlers and the client side URL builder for `/file/{size}`.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::body::{file_body, BodyPlan};
use crate::content::{etag, full_content, last_modified};
use crate::range::{evaluate, if_range_matches, Requested};
use crate::state::ServerState;
use sha2::{Digest, Sha256};

/// Options for a file URL built by [`file_url`]. The default is a range-capable file with a known length, an ETag and no pacing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOpts {
    /// Honour `Range` requests. `false` answers with `200` and the whole file.
    pub ranges: bool,
    /// Send `Content-Length` on `200` answers. `false` makes the body chunked.
    pub length: bool,
    /// Bytes per second. `0` means no pacing.
    pub rate: u64,
    /// Plain `filename="…"` for `Content-Disposition`. Use ASCII only.
    pub name: Option<String>,
    /// UTF-8 `filename*=UTF-8''…` for `Content-Disposition`. The raw name is percent-encoded.
    pub name_star: Option<String>,
    /// Send an `ETag` header.
    pub etag: bool,
}

impl Default for FileOpts {
    fn default() -> Self {
        Self {
            ranges: true,
            length: true,
            rate: 0,
            name: None,
            name_star: None,
            etag: true,
        }
    }
}

/// Builds the request path `/file/{size}` with the query options that differ from the default.
pub fn file_url(size: u64, opts: &FileOpts) -> String {
    let mut query: Vec<String> = Vec::new();
    if !opts.ranges {
        query.push("ranges=0".to_owned());
    }
    if !opts.length {
        query.push("length=0".to_owned());
    }
    if opts.rate > 0 {
        query.push(format!("rate={}", opts.rate));
    }
    if let Some(name) = &opts.name {
        query.push(format!("name={}", pct_encode(name, is_unreserved)));
    }
    if let Some(name) = &opts.name_star {
        query.push(format!("name_star={}", pct_encode(name, is_unreserved)));
    }
    if !opts.etag {
        query.push("etag=0".to_owned());
    }
    let mut path = format!("/file/{size}");
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query.join("&"));
    }
    path
}

/// Query options of `/file/{size}`. A flag set to `0` turns the feature off. Any other value, or no value, keeps it on.
#[derive(Debug, Deserialize, Default)]
pub(crate) struct FileQuery {
    ranges: Option<String>,
    length: Option<String>,
    rate: Option<u64>,
    name: Option<String>,
    name_star: Option<String>,
    etag: Option<String>,
}

fn flag(value: &Option<String>) -> bool {
    value.as_deref() != Some("0")
}

/// Basic credentials accepted by `/auth/file/{size}`: user `demo`, password `secret`.
pub const DEMO_AUTHORIZATION: &str = "Basic ZGVtbzpzZWNyZXQ=";

/// Serves `/auth/file/{size}` like `/file/{size}`, but only with the demo credentials.
pub(crate) async fn auth_file(
    State(state): State<Arc<ServerState>>,
    Path(size): Path<u64>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
    uri: Uri,
) -> Response<Body> {
    let accepted = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        == Some(DEMO_AUTHORIZATION);
    if !accepted {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"test\"")],
        )
            .into_response();
    }
    file(State(state), Path(size), Query(query), headers, uri).await
}

/// Serves `/file/{size}`: deterministic content with optional ranges, pacing and fail injection.
/// An absolute request address (as a proxy receives it) is counted as a proxied request.
pub(crate) async fn file(
    State(state): State<Arc<ServerState>>,
    Path(size): Path<u64>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
    uri: Uri,
) -> Response<Body> {
    state.count_request();
    if uri.authority().is_some() {
        state.count_proxied();
    }
    let Some(guard) = state.try_acquire() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "1")],
        )
            .into_response();
    };
    let version = state.version();
    let tag = flag(&query.etag).then(|| etag(version, size));
    let modified = last_modified(version);
    let ranges_on = flag(&query.ranges);
    let length_on = flag(&query.length);

    let header_value = |name: HeaderName| headers.get(name).and_then(|v| v.to_str().ok());
    let if_range_ok = header_value(header::IF_RANGE)
        .is_none_or(|value| if_range_matches(value, tag.as_deref(), &modified));
    let requested = if ranges_on && if_range_ok {
        evaluate(header_value(header::RANGE), size)
    } else {
        Requested::Full
    };

    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::LAST_MODIFIED, modified.as_str());
    if let Some(tag) = &tag {
        builder = builder.header(header::ETAG, tag.as_str());
    }
    if ranges_on {
        builder = builder.header(header::ACCEPT_RANGES, "bytes");
    }
    if let Some(value) = disposition(query.name.as_deref(), query.name_star.as_deref()) {
        builder = builder.header(header::CONTENT_DISPOSITION, value);
    }

    let (status, start, len) = match requested {
        Requested::Unsatisfiable => {
            return match builder
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{size}"))
                .header(header::CONTENT_LENGTH, "0")
                .body(Body::empty())
            {
                Ok(response) => response,
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            };
        }
        Requested::Partial { start, end } => {
            state.count_range();
            builder = builder
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
                .header(header::CONTENT_LENGTH, (end - start + 1).to_string());
            (StatusCode::PARTIAL_CONTENT, start, end - start + 1)
        }
        Requested::Full => {
            if length_on {
                builder = builder.header(header::CONTENT_LENGTH, size.to_string());
            }
            (StatusCode::OK, 0, size)
        }
    };

    let plan = BodyPlan {
        version,
        size,
        start,
        len,
        rate: query.rate.unwrap_or(0),
        fail_at: state.take_failure(),
        state: Arc::clone(&state),
        _guard: guard,
    };
    match builder.status(status).body(file_body(plan)) {
        Ok(response) => response,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Query of `/redirect`.
#[derive(Debug, Deserialize)]
pub(crate) struct RedirectQuery {
    to: String,
}

/// Answers `302 Found` with `Location` set to the `to` query value.
pub(crate) async fn redirect(Query(query): Query<RedirectQuery>) -> Response<Body> {
    match HeaderValue::from_str(&query.to) {
        Ok(location) => (StatusCode::FOUND, [(header::LOCATION, location)]).into_response(),
        Err(_) => StatusCode::BAD_REQUEST.into_response(),
    }
}

/// Builds the `Content-Disposition` value from the plain and the UTF-8 name.
fn disposition(name: Option<&str>, name_star: Option<&str>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(name) = name {
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        parts.push(format!("filename=\"{escaped}\""));
    }
    if let Some(name) = name_star {
        parts.push(format!(
            "filename*=UTF-8''{}",
            pct_encode(name, is_attr_char)
        ));
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("attachment; {}", parts.join("; ")))
    }
}

/// RFC 3986 unreserved characters.
fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// RFC 5987 `attr-char` characters.
fn is_attr_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte)
}

/// Percent-encodes every byte that `keep` rejects.
fn pct_encode(input: &str, keep: fn(u8) -> bool) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        if keep(byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Size of the files served under `/files/{name}`.
pub const NAMED_FILE_SIZE: u64 = 256 * 1024;

/// Hex SHA-256 of the content of every `/files/{name}` file.
pub fn named_file_sha256() -> String {
    format!("{:x}", Sha256::digest(full_content(1, NAMED_FILE_SIZE)))
}

/// Files for the checksum tests under `/files/{name}`. All have the same content.
/// `side-*` names have a `.sha256` sidecar with the right hash, `bad-*` names a wrong one.
/// `sums-known.bin` is listed in `/files/SHA256SUMS`, and `none-*` names have no checksum.
pub(crate) async fn named_file(Path(path): Path<String>) -> Response<Body> {
    if path == "SHA256SUMS" {
        let body = format!(
            "{}  sums-known.bin\n{}  other.bin\n",
            named_file_sha256(),
            "0".repeat(64)
        );
        return (StatusCode::OK, body).into_response();
    }
    if let Some(base) = path.strip_suffix(".sha256") {
        let hash = if base.starts_with("side-") {
            named_file_sha256()
        } else if base.starts_with("bad-") {
            "0".repeat(64)
        } else {
            return StatusCode::NOT_FOUND.into_response();
        };
        return (StatusCode::OK, format!("{hash}\n")).into_response();
    }
    (StatusCode::OK, full_content(1, NAMED_FILE_SIZE)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_has_no_query() {
        assert_eq!(file_url(1000, &FileOpts::default()), "/file/1000");
    }

    #[test]
    fn url_lists_only_changed_options() {
        let opts = FileOpts {
            ranges: false,
            length: false,
            rate: 4096,
            name: Some("my file.pdf".to_owned()),
            name_star: Some("résumé.pdf".to_owned()),
            etag: false,
        };
        assert_eq!(
            file_url(8, &opts),
            "/file/8?ranges=0&length=0&rate=4096&name=my%20file.pdf&name_star=r%C3%A9sum%C3%A9.pdf&etag=0"
        );
    }

    #[test]
    fn disposition_forms() {
        assert_eq!(disposition(None, None), None);
        assert_eq!(
            disposition(Some("report.pdf"), None).as_deref(),
            Some("attachment; filename=\"report.pdf\"")
        );
        assert_eq!(
            disposition(None, Some("résumé.pdf")).as_deref(),
            Some("attachment; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf")
        );
    }
}
