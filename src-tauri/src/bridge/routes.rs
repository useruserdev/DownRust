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

use crate::{
    bridge::auth,
    events::{ExternalAddPayload, EXTERNAL_ADD_REQUEST},
    state::AppState,
    window,
};

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

pub fn router(app: AppHandle) -> Router {
    Router::new()
        .route("/v1/ping", get(ping))
        .route("/v1/downloads", post(add_download))
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

/// A file name from the page title. Media transfers get the `.mp4` extension.
fn title_file_name(title: &str, kind: Option<DownloadKind>) -> Option<String> {
    let base = sanitize_file_name(title.trim());
    if base.is_empty() {
        return None;
    }
    let media = kind.is_some_and(DownloadKind::is_media);
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
