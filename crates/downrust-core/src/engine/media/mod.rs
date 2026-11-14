// SPDX-License-Identifier: GPL-3.0-or-later
//! HLS and DASH transfers, and the dispatch of a download by its kind.
pub(crate) mod crypto;
pub(crate) mod dash;
pub(crate) mod fetch;
pub(crate) mod ffmpeg;
pub(crate) mod hls;
pub(crate) mod playlist;
pub(crate) mod state;

use super::checksum::verify_completed;
use super::task::{run_task, TaskContext, TaskOutcome};
use crate::model::DownloadKind;

/// Runs a download of any kind to its end. The checksum check runs after media transfers too.
pub(crate) async fn run_download(ctx: TaskContext) -> TaskOutcome {
    let kind = match ctx.store.get_download(ctx.id) {
        Ok(Some(record)) => record.kind,
        Ok(None) => return TaskOutcome::Failed(format!("download {} not found", ctx.id)),
        Err(err) => return TaskOutcome::Failed(err.to_string()),
    };
    let result = match kind {
        DownloadKind::File => return run_task(ctx).await,
        DownloadKind::Hls => hls::run(&ctx).await,
        DownloadKind::Dash => dash::run(&ctx).await,
    };
    match result {
        Ok(TaskOutcome::Completed) => verify_completed(&ctx).await,
        Ok(outcome) => outcome,
        Err(err) => TaskOutcome::Failed(err.to_string()),
    }
}
