// SPDX-License-Identifier: GPL-3.0-or-later
//! Downloads from video sites with yt-dlp.
use crate::engine::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};

/// Runs a download of this kind to its end. A cancel gives `Cancelled`.
pub(crate) async fn run(ctx: &TaskContext) -> Result<TaskOutcome> {
    let _ = ctx;
    Err(Error::Transfer(
        "yt-dlp downloads are not available yet".to_owned(),
    ))
}

/// File name used when the request gives none.
pub(crate) fn initial_name(url: &str) -> String {
    let _ = url;
    "download".to_owned()
}
