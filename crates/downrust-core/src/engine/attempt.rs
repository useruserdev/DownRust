// SPDX-License-Identifier: GPL-3.0-or-later
//! One attempt of a download: probe, plan, transfer through the coordinator, then finish the file.
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::coordinator::{checkpoint, run_coordinator, CoordEnd, Job};
use super::file::{finalize, open_part_file, preallocate};
use super::plan::{decide, TransferPlan};
use super::retry::RetryPolicy;
use super::segments::SegmentTable;
use super::task::TaskContext;
use super::worker::AddressPool;
use super::writer::{spawn_writer, WriterHandle};
use crate::auth::{login_for, request_host, unauthorized_message, with_site_login};
use crate::error::{Error, Result};
use crate::http::classify::{classify_status, StatusClass};
use crate::http::probe::probe;
use crate::model::{DownloadRecord, EngineEvent, HeaderPair, Phase, ProbeInfo};
use crate::settings::EngineSettings;

/// How one attempt ended, when it did not fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttemptEnd {
    Completed,
    Cancelled,
    Restart(String),
}

/// Runs one attempt. `force_single` turns off segmented transfer.
pub(crate) async fn run_attempt(ctx: &TaskContext, force_single: bool) -> Result<AttemptEnd> {
    let prev = ctx
        .store
        .get_download(ctx.id)?
        .ok_or(Error::NotFound(ctx.id))?;
    ctx.live.set_phase(Phase::Connecting);
    let probe_headers = with_site_login(&ctx.settings, &prev.url, &prev.request_headers());
    let Some(probed) = probe_with_retry(ctx, &prev.url, &probe_headers)
        .await
        .map_err(|err| refused_message(err, &ctx.settings, &prev.url))?
    else {
        return Ok(AttemptEnd::Cancelled);
    };
    let saved = ctx.store.load_segments(ctx.id)?;
    let part = prev.part_path();
    let plan = decide(
        &prev,
        &probed,
        &saved,
        part.exists(),
        ctx.settings.preallocate,
        force_single,
    );
    if let Some(note) = &plan.note {
        tracing::warn!(id = ctx.id, "{note}");
    }
    let mirrors = verified_mirrors(ctx, &prev, &probed).await;
    ctx.store.update_meta(ctx.id, &probed)?;
    if plan.fresh {
        ctx.store.clear_progress(ctx.id)?;
    }
    emit_updated(ctx)?;
    let dir = Path::new(&prev.save_dir);
    std::fs::create_dir_all(dir)?;
    let file = open_part_file(&part, plan.fresh)?;
    if let Some(size) = plan.preallocate {
        preallocate(&file, size)?;
    }
    let TransferPlan {
        segmented,
        total,
        slots,
        ..
    } = plan;
    let table = Arc::new(SegmentTable::new(slots));
    ctx.live.attach(Arc::clone(&table), total);
    let writer = spawn_writer(file, Arc::clone(&table));
    ctx.live.set_phase(Phase::Downloading);
    let headers = with_site_login(&ctx.settings, &probed.final_url, &prev.request_headers());
    let job = build_job(ctx, &probed, headers, segmented, mirrors, &writer, &table);
    let end = run_coordinator(ctx, job).await;
    if segmented && matches!(end, CoordEnd::Failed(_) | CoordEnd::Cancelled) {
        if let Err(err) = checkpoint(ctx, &writer.tx).await {
            tracing::warn!(id = ctx.id, error = %err, "final checkpoint failed");
        }
    }
    let WriterHandle { tx, join } = writer;
    drop(tx);
    let file = match join.await {
        Ok(Ok(file)) => file,
        Ok(Err(err)) => return Err(Error::Io(err)),
        Err(err) => return Err(Error::Transfer(format!("disk writer stopped: {err}"))),
    };
    match end {
        CoordEnd::Failed(msg) => Err(Error::Transfer(msg)),
        CoordEnd::Restart(reason) => Ok(AttemptEnd::Restart(reason)),
        CoordEnd::Cancelled => Ok(AttemptEnd::Cancelled),
        CoordEnd::Finished => {
            ctx.live.set_phase(Phase::Finalizing);
            let size = total.unwrap_or_else(|| table.fetched_total());
            file.set_len(size)?;
            drop(file);
            let name = finalize(&part, dir, &prev.file_name).await?;
            ctx.store.record_finished_file(ctx.id, &name, size)?;
            Ok(AttemptEnd::Completed)
        }
    }
}

/// Keeps the mirrors whose file matches the main file: the same size, the same ETag when both have
/// one, and support for ranges. Other mirrors are logged and dropped.
async fn verified_mirrors(
    ctx: &TaskContext,
    prev: &DownloadRecord,
    main: &ProbeInfo,
) -> Vec<String> {
    let timeout =
        Duration::from_secs(ctx.settings.connect_timeout_secs + ctx.settings.read_timeout_secs);
    let mut kept: Vec<String> = Vec::new();
    for address in &prev.mirrors {
        if !is_http_address(address) {
            tracing::warn!(id = ctx.id, mirror = %address, "mirror dropped: not an http(s) address");
            continue;
        }
        let headers = with_site_login(&ctx.settings, address, &prev.request_headers());
        let info = match probe(&ctx.client, address, &headers, timeout).await {
            Ok(info) => info,
            Err(err) => {
                tracing::warn!(id = ctx.id, mirror = %address, error = %err, "mirror dropped: probe failed");
                continue;
            }
        };
        let same_size = main.total_size.is_some() && info.total_size == main.total_size;
        let same_etag = match (&main.etag, &info.etag) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        };
        if !same_size || !same_etag || !info.resumable {
            tracing::warn!(
                id = ctx.id,
                mirror = %address,
                "mirror dropped: size, ETag or range support differs"
            );
            continue;
        }
        if info.final_url != main.final_url && !kept.contains(&info.final_url) {
            kept.push(info.final_url);
        }
    }
    kept
}

fn is_http_address(address: &str) -> bool {
    let lower = address.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn build_job(
    ctx: &TaskContext,
    probed: &ProbeInfo,
    headers: Vec<HeaderPair>,
    segmented: bool,
    mirrors: Vec<String>,
    writer: &WriterHandle,
    table: &Arc<SegmentTable>,
) -> Job {
    let has_login = login_for(&ctx.settings, &probed.final_url).is_some();
    Job {
        addresses: Arc::new(AddressPool::new(probed.final_url.clone(), mirrors)),
        headers,
        unauthorized: unauthorized_message(has_login, &request_host(&probed.final_url)),
        segmented,
        use_range: segmented,
        if_range: probed.etag.clone().or_else(|| probed.last_modified.clone()),
        min_split: if segmented {
            ctx.settings.min_segment_size
        } else {
            u64::MAX
        },
        retry: retry_policy(&ctx.settings),
        read_timeout: Duration::from_secs(ctx.settings.read_timeout_secs),
        table: Arc::clone(table),
        write_tx: writer.tx.clone(),
    }
}

fn emit_updated(ctx: &TaskContext) -> Result<()> {
    if let Some(record) = ctx.store.get_download(ctx.id)? {
        let _ = ctx.events.send(EngineEvent::Updated(record));
    }
    Ok(())
}

/// Probes the address, retrying transient failures. `Ok(None)` means the task was cancelled.
async fn probe_with_retry(
    ctx: &TaskContext,
    url: &str,
    headers: &[HeaderPair],
) -> Result<Option<ProbeInfo>> {
    let policy = retry_policy(&ctx.settings);
    let timeout =
        Duration::from_secs(ctx.settings.connect_timeout_secs + ctx.settings.read_timeout_secs);
    let mut attempt: u32 = 0;
    loop {
        let result = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => return Ok(None),
            found = probe(&ctx.client, url, headers, timeout) => found,
        };
        match result {
            Ok(info) => return Ok(Some(info)),
            Err(err) if is_transient(&err) => {
                attempt += 1;
                if attempt > policy.max_retries {
                    return Err(err);
                }
                tokio::select! {
                    biased;
                    _ = ctx.cancel.cancelled() => return Ok(None),
                    _ = tokio::time::sleep(policy.delay_for(attempt)) => {}
                }
            }
            Err(err) => return Err(err),
        }
    }
}

/// Turns a `401` reply of the probe into the message that tells the user what to do.
fn refused_message(err: Error, settings: &EngineSettings, address: &str) -> Error {
    match err {
        Error::HttpStatus(401) => Error::Transfer(unauthorized_message(
            login_for(settings, address).is_some(),
            &request_host(address),
        )),
        other => other,
    }
}

/// Errors worth another try: busy or server-side status codes, timeouts and network errors.
pub(crate) fn is_transient(err: &Error) -> bool {
    match err {
        Error::HttpStatus(code) => !matches!(
            classify_status(*code),
            StatusClass::Success | StatusClass::Fatal
        ),
        Error::Timeout(_) | Error::Http(_) => true,
        _ => false,
    }
}

/// Retry policy from the engine settings.
pub(crate) fn retry_policy(settings: &EngineSettings) -> RetryPolicy {
    RetryPolicy {
        max_retries: settings.max_retries,
        base: Duration::from_millis(settings.retry_base_delay_ms),
        max: Duration::from_millis(settings.retry_max_delay_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_errors_are_retried() {
        assert!(is_transient(&Error::HttpStatus(503)));
        assert!(is_transient(&Error::HttpStatus(500)));
        assert!(is_transient(&Error::Timeout("slow".to_owned())));
        assert!(!is_transient(&Error::HttpStatus(404)));
        assert!(!is_transient(&Error::Protocol("bad".to_owned())));
    }

    #[test]
    fn retry_policy_copies_settings() {
        let settings = EngineSettings {
            max_retries: 7,
            retry_base_delay_ms: 50,
            retry_max_delay_ms: 400,
            ..EngineSettings::default()
        };
        let policy = retry_policy(&settings);
        assert_eq!(policy.max_retries, 7);
        assert_eq!(policy.delay_for(1), Duration::from_millis(50));
        assert_eq!(policy.delay_for(4), Duration::from_millis(400));
    }
}
