// SPDX-License-Identifier: GPL-3.0-or-later
//! Downloads over FTP, FTPS and SFTP. Each attempt opens one connection, resumes from the part
//! file and retries connection errors. A failed login stops at once.
mod address;
mod plain;
mod pump;
mod sftp;

use std::fmt::Display;
use std::fs::File;
use std::future::Future;
use std::path::Path;
use std::time::Duration;

use address::{resolve_login, Address, Login, Protocol};

use crate::engine::attempt::retry_policy;
use crate::engine::file::{finalize, open_part_file};
use crate::engine::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};
use crate::model::{DownloadRecord, EngineEvent, Phase, ProbeInfo};

/// Why an attempt stopped without finishing.
#[derive(Debug)]
pub(crate) enum Failure {
    /// The attempt may be tried again. `progressed` is true when bytes arrived in it.
    Retry { msg: String, progressed: bool },
    /// The download fails with this message.
    Fatal(String),
    /// The user cancelled. The part file and its progress are kept.
    Cancelled,
}

pub(crate) type Step<T> = std::result::Result<T, Failure>;

/// A failure that will not get better by trying again.
fn fatal(err: impl Display) -> Failure {
    Failure::Fatal(err.to_string())
}

/// A failure worth another try.
fn retry(msg: impl Into<String>, progressed: bool) -> Failure {
    Failure::Retry {
        msg: msg.into(),
        progressed,
    }
}

/// Runs `fut` for at most `limit`. A cancel during the wait gives `Failure::Cancelled`, a
/// timeout gives a retry, and an error goes through `map`.
async fn bounded<T, E>(
    ctx: &TaskContext,
    limit: Duration,
    what: &str,
    fut: impl Future<Output = std::result::Result<T, E>>,
    map: impl FnOnce(E) -> Failure,
) -> Step<T> {
    tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => Err(Failure::Cancelled),
        timed = tokio::time::timeout(limit, fut) => match timed {
            Err(_) => Err(retry(format!("{what} timed out"), false)),
            Ok(Err(err)) => Err(map(err)),
            Ok(Ok(value)) => Ok(value),
        },
    }
}

/// Runs a download of this kind to its end. A cancel gives `Cancelled`.
pub(crate) async fn run(ctx: &TaskContext) -> Result<TaskOutcome> {
    let record = ctx
        .store
        .get_download(ctx.id)?
        .ok_or(Error::NotFound(ctx.id))?;
    let address = Address::parse(&record.url)?;
    let login = resolve_login(&address, &record.url, &ctx.settings)?;
    let policy = retry_policy(&ctx.settings);
    let mut failures: u32 = 0;
    loop {
        match attempt(ctx, &address, &login).await {
            Ok(outcome) => return Ok(outcome),
            Err(Failure::Cancelled) => return Ok(TaskOutcome::Cancelled),
            Err(Failure::Fatal(msg)) => return Ok(TaskOutcome::Failed(msg)),
            Err(Failure::Retry { msg, progressed }) => {
                failures = if progressed { 1 } else { failures + 1 };
                if failures > policy.max_retries {
                    return Ok(TaskOutcome::Failed(msg));
                }
                tracing::warn!(id = ctx.id, %msg, failures, "FTP transfer failed, retrying");
                tokio::select! {
                    biased;
                    _ = ctx.cancel.cancelled() => return Ok(TaskOutcome::Cancelled),
                    _ = tokio::time::sleep(policy.delay_for(failures)) => {}
                }
            }
        }
    }
}

/// File name used when the request gives none.
pub(crate) fn initial_name(url: &str) -> String {
    address::initial_file_name(url)
}

/// One attempt: transfer through the protocol, then move the part file to its final name.
async fn attempt(ctx: &TaskContext, address: &Address, login: &Login) -> Step<TaskOutcome> {
    let prev = ctx
        .store
        .get_download(ctx.id)
        .map_err(fatal)?
        .ok_or_else(|| fatal(Error::NotFound(ctx.id)))?;
    ctx.live.set_phase(Phase::Connecting);
    let size = match address.protocol {
        Protocol::Sftp => sftp::download(ctx, &prev, address, login).await?,
        Protocol::Ftp | Protocol::Ftps => plain::download(ctx, &prev, address, login).await?,
    };
    finish(ctx, &prev, size).await?;
    Ok(TaskOutcome::Completed)
}

/// Prepares the part file for a transfer of `size` bytes. Returns the file and the offset the
/// transfer starts at. A part file that is too long, or a size that changed, starts from zero.
fn prepare(ctx: &TaskContext, prev: &DownloadRecord, size: u64) -> Step<(File, u64)> {
    let dir = Path::new(&prev.save_dir);
    std::fs::create_dir_all(dir).map_err(fatal)?;
    let part = prev.part_path();
    let changed = prev.total_size.is_some_and(|known| known != size);
    let saved = std::fs::metadata(&part).map(|meta| meta.len()).ok();
    let offset = match saved {
        Some(len) if len <= size && !changed => len,
        _ => 0,
    };
    if offset == 0 {
        ctx.store.clear_progress(ctx.id).map_err(fatal)?;
    } else {
        ctx.store
            .save_checkpoint(ctx.id, offset, &[])
            .map_err(fatal)?;
    }
    let file = open_part_file(&part, offset == 0).map_err(fatal)?;
    ctx.store
        .update_meta(ctx.id, &probe_for(prev, size))
        .map_err(fatal)?;
    emit_updated(ctx)?;
    ctx.live.set_media_total(Some(size));
    ctx.live.set_media_done(offset);
    ctx.live.set_connections(1);
    ctx.live.set_phase(Phase::Downloading);
    Ok((file, offset))
}

/// The size and file facts the record keeps for this transfer.
fn probe_for(prev: &DownloadRecord, size: u64) -> ProbeInfo {
    ProbeInfo {
        url: prev.url.clone(),
        final_url: prev.url.clone(),
        file_name: prev.file_name.clone(),
        total_size: Some(size),
        resumable: true,
        content_type: None,
        etag: None,
        last_modified: None,
        category: prev.category,
    }
}

fn emit_updated(ctx: &TaskContext) -> Step<()> {
    if let Some(record) = ctx.store.get_download(ctx.id).map_err(fatal)? {
        let _ = ctx.events.send(EngineEvent::Updated(record));
    }
    Ok(())
}

/// Renames the complete part file to its final name and records it.
async fn finish(ctx: &TaskContext, prev: &DownloadRecord, size: u64) -> Step<()> {
    ctx.live.set_phase(Phase::Finalizing);
    let name = finalize(
        &prev.part_path(),
        Path::new(&prev.save_dir),
        &prev.file_name,
    )
    .await
    .map_err(fatal)?;
    ctx.store
        .record_finished_file(ctx.id, &name, size)
        .map_err(fatal)?;
    Ok(())
}
