// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type returned by every command and setup step.

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Core(#[from] downrust_core::Error),
    #[error("window system error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("settings format error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

impl AppError {
    /// Wraps a plugin or other error that has no dedicated variant.
    pub fn message(err: impl std::fmt::Display) -> Self {
        Self::Message(err.to_string())
    }
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
