// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type shared by the download engine.
use crate::model::DownloadId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid address: {0}")]
    InvalidUrl(String),
    #[error("only http and https addresses are supported (got {0})")]
    UnsupportedScheme(String),
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server responded with HTTP {0}")]
    HttpStatus(u16),
    #[error("unexpected server response: {0}")]
    Protocol(String),
    #[error("timed out: {0}")]
    Timeout(String),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("data format error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("download {0} not found")]
    NotFound(DownloadId),
    #[error("{0}")]
    InvalidInput(String),
    #[error("the download manager has stopped")]
    ManagerStopped,
    #[error("{0}")]
    Transfer(String),
    #[error("password store error: {0}")]
    Secrets(String),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
