// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP routes of the browser bridge.
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use downrust_core::model::{DownloadKind, HeaderPair};
use downrust_core::util::filename::sanitize_file_name;
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::{
    bridge::auth,
    events::{
        BatchLink, ExternalAddPayload, ExternalBatchPayload, EXTERNAL_ADD_REQUEST,
        EXTERNAL_BATCH_REQUEST,
    },
    state::AppState,
    window,
};

/// Most links one batch request may carry.
const MAX_BATCH_LINKS: usize = 5000;

/// Address schemes a batch link may use.
const BATCH_SCHEMES: [&str; 6] = ["http", "https", "ftp", "ftps", "sftp", "magnet"];

/// Body of `POST /v1/downloads`. Only `url` is required.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddBody {
    url: String,
    file_name: Option<String>,
    referrer: Option<String>,
    cookies: Option<String>,
    user_agent: Option<String>,
    kind: Option<String>,
    title: Option<String>,
}

/// Body of `POST /v1/batch`, sent by the link grabber.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchBody {
    links: Vec<BatchLink>,
    referrer: Option<String>,
    user_agent: Option<String>,
}

pub fn router(app: AppHandle) -> Router {
    Router::new()
        .route("/v1/ping", get(ping))
        .route("/v1/downloads", post(add_download))
        .route("/v1/batch", post(add_batch))
        .with_state(app)
}

async fn ping(State(app): State<AppHandle>, headers: HeaderMap) -> Response {
    if let Err(status) = guard(&app, &headers, false) {
        return status.into_response();
    }
    Json(json!({"name": "DownRust", "version": env!("CARGO_PKG_VERSION")})).into_response()
}

async fn add_download(State(app): State<AppHandle>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(status) = guard(&app, &headers, true) {
        return status.into_response();
    }
    let request: AddBody = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(err) => {
            tracing::warn!("bridge request rejected: {err}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    let kind = match request.kind.as_deref() {
        None => None,
        Some(text) => match DownloadKind::parse(text) {
            Some(kind) => Some(kind),
            None => {
                tracing::warn!("bridge request rejected: unknown kind {text}");
                return StatusCode::BAD_REQUEST.into_response();
            }
        },
    };
    let kind = kind.or_else(|| DownloadKind::from_url(&request.url));
    let file_name = request.file_name.or_else(|| {
        request
            .title
            .as_deref()
            .and_then(|title| title_file_name(title, kind))
    });
    let payload = ExternalAddPayload {
        url: request.url,
        file_name,
        referrer: request.referrer,
        headers: header_pairs(request.cookies, request.user_agent),
        kind,
    };
    if let Err(err) = app.emit(EXTERNAL_ADD_REQUEST, payload) {
        tracing::warn!("external add event failed: {err}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    window::show_main(&app);
    StatusCode::ACCEPTED.into_response()
}

async fn add_batch(State(app): State<AppHandle>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(status) = guard(&app, &headers, true) {
        return status.into_response();
    }
    let request: BatchBody = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(err) => {
            tracing::warn!("bridge batch rejected: {err}");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if let Err(status) = check_batch(&request.links) {
        tracing::warn!("bridge batch rejected: {status}");
        return status.into_response();
    }
    let referrer = request.referrer.filter(|value| !value.is_empty());
    let headers = batch_headers(referrer.as_deref(), request.user_agent);
    let payload = ExternalBatchPayload {
        links: request.links,
        referrer,
        headers,
    };
    if let Err(err) = app.emit(EXTERNAL_BATCH_REQUEST, payload) {
        tracing::warn!("external batch event failed: {err}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    window::show_main(&app);
    StatusCode::ACCEPTED.into_response()
}

/// A batch holds 1 to `MAX_BATCH_LINKS` links. Each link must parse as a URL with an allowed scheme.
fn check_batch(links: &[BatchLink]) -> Result<(), StatusCode> {
    if links.is_empty() || links.len() > MAX_BATCH_LINKS {
        return Err(StatusCode::BAD_REQUEST);
    }
    for link in links {
        let parsed = Url::parse(&link.url).map_err(|_| StatusCode::BAD_REQUEST)?;
        if !BATCH_SCHEMES.contains(&parsed.scheme()) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

/// The page address and the user agent travel as `Referer` and `User-Agent` headers.
fn batch_headers(referrer: Option<&str>, user_agent: Option<String>) -> Vec<HeaderPair> {
    let mut pairs = Vec::new();
    if let Some(value) = referrer {
        pairs.push(HeaderPair {
            name: "Referer".to_owned(),
            value: value.to_owned(),
        });
    }
    if let Some(value) = user_agent.filter(|value| !value.is_empty()) {
        pairs.push(HeaderPair {
            name: "User-Agent".to_owned(),
            value,
        });
    }
    pairs
}

/// Runs the origin, token and (for POST) content type checks in that order.
fn guard(app: &AppHandle, headers: &HeaderMap, needs_json: bool) -> Result<(), StatusCode> {
    let origin = header_text(headers, header::ORIGIN.as_str());
    auth::check_origin(origin)?;
    let expected = app
        .state::<AppState>()
        .settings
        .read()
        .expect("settings lock poisoned")
        .browser_token
        .clone();
    auth::check_token(header_text(headers, auth::TOKEN_HEADER), &expected)?;
    if needs_json {
        auth::check_json(header_text(headers, header::CONTENT_TYPE.as_str()))?;
    }
    Ok(())
}

fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// A file name from the page title. Media transfers and video pages get the `.mp4` extension.
fn title_file_name(title: &str, kind: Option<DownloadKind>) -> Option<String> {
    let base = sanitize_file_name(title.trim());
    if base.is_empty() {
        return None;
    }
    let media = kind.is_some_and(|kind| kind.is_media() || kind == DownloadKind::Site);
    if media && !base.to_ascii_lowercase().ends_with(".mp4") {
        Some(format!("{base}.mp4"))
    } else {
        Some(base)
    }
}

/// Cookies and the user agent travel as ordinary request headers.
fn header_pairs(cookies: Option<String>, user_agent: Option<String>) -> Vec<HeaderPair> {
    let mut pairs = Vec::new();
    if let Some(value) = cookies.filter(|value| !value.is_empty()) {
        pairs.push(HeaderPair {
            name: "Cookie".to_owned(),
            value,
        });
    }
    if let Some(value) = user_agent.filter(|value| !value.is_empty()) {
        pairs.push(HeaderPair {
            name: "User-Agent".to_owned(),
            value,
        });
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(url: &str) -> BatchLink {
        BatchLink {
            url: url.to_owned(),
            text: String::new(),
        }
    }

    #[test]
    fn batch_accepts_allowed_schemes() {
        let links = vec![
            link("https://a.test/file.zip"),
            link("http://a.test/x"),
            link("ftp://a.test/pub/x.iso"),
            link("sftp://a.test/x"),
            link("magnet:?xt=urn:btih:abc"),
        ];
        assert_eq!(check_batch(&links), Ok(()));
    }

    #[test]
    fn batch_rejects_empty_and_oversized_lists() {
        assert_eq!(check_batch(&[]), Err(StatusCode::BAD_REQUEST));
        let full = vec![link("https://a.test/x"); MAX_BATCH_LINKS];
        assert_eq!(check_batch(&full), Ok(()));
        let too_many = vec![link("https://a.test/x"); MAX_BATCH_LINKS + 1];
        assert_eq!(check_batch(&too_many), Err(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn batch_rejects_unparseable_and_foreign_schemes() {
        assert_eq!(
            check_batch(&[link("not a url")]),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            check_batch(&[link("javascript:alert(1)")]),
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            check_batch(&[link("file:///etc/passwd")]),
            Err(StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn batch_headers_carry_referer_and_agent() {
        let pairs = batch_headers(Some("https://a.test/page"), Some("UA/1".to_owned()));
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].name, "Referer");
        assert_eq!(pairs[1].name, "User-Agent");
        assert!(batch_headers(None, Some(String::new())).is_empty());
    }

    #[test]
    fn site_pages_get_mp4_extension() {
        assert_eq!(
            title_file_name("Clip title", Some(DownloadKind::Site)).as_deref(),
            Some("Clip title.mp4")
        );
        assert_eq!(
            title_file_name("Clip.MP4", Some(DownloadKind::Site)).as_deref(),
            Some("Clip.MP4")
        );
        assert_eq!(
            title_file_name("Plain", Some(DownloadKind::File)).as_deref(),
            Some("Plain")
        );
    }
}
