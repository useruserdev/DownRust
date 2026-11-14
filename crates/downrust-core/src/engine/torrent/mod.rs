// SPDX-License-Identifier: GPL-3.0-or-later
//! Torrent downloads from magnet links and .torrent files, run by librqbit.
mod session;
mod source;
mod transfer;

use crate::engine::task::{TaskContext, TaskOutcome};
use crate::error::Result;

/// Runs a download of this kind to its end. A cancel gives `Cancelled`.
pub(crate) async fn run(ctx: &TaskContext) -> Result<TaskOutcome> {
    transfer::run(ctx).await
}

/// File name used when the request gives none.
pub(crate) fn initial_name(url: &str) -> String {
    source::initial_name(url)
}
