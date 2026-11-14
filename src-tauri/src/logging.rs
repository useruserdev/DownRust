// SPDX-License-Identifier: GPL-3.0-or-later
//! File logging with a size cap.
use std::{
    fs::{self, OpenOptions},
    path::Path,
    sync::Mutex,
};

use tracing::Level;

use crate::error::AppResult;

const LOG_FILE: &str = "downrust.log";
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Starts the global tracing subscriber. Call once.
pub fn init(log_dir: &Path) -> AppResult<()> {
    let path = log_dir.join(LOG_FILE);
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
        fs::File::create(&path)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    tracing_subscriber::fmt()
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_max_level(Level::INFO)
        .init();
    Ok(())
}
