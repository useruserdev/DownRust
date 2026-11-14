// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP reads for media parts: retries with capped delays, rate limits, cancellation and live
//! byte counts. Byte ranges that the server answers with a full body are cut to size here.
use std::path::PathBuf;
use std::time::Duration;

use crate::engine::retry::RetryPolicy;
use crate::engine::task::TaskContext;
use crate::error::{Error, Result};
use crate::http::request::get_request;
use crate::model::HeaderPair;
use crate::settings::EngineSettings;

use super::ffmpeg::CANCELLED;
use super::playlist::ByteRange;

/// Bytes of a resource and the address it was finally read from (after redirects).
pub(crate) struct Fetched {
    pub bytes: Vec<u8>,
    pub url: String,
}

/// Retry policy taken from the engine settings.
pub(crate) fn retry_policy(settings: &EngineSettings) -> RetryPolicy {
    RetryPolicy {
        max_retries: settings.max_retries,
        base: Duration::from_millis(settings.retry_base_delay_ms),
        max: Duration::from_millis(settings.retry_max_delay_ms),
    }
}

/// Reads a whole resource, or one byte range of it, and retries temporary failures.
pub(crate) async fn fetch(
    ctx: &TaskContext,
    url: &str,
    headers: &[HeaderPair],
    range: Option<ByteRange>,
) -> Result<Fetched> {
    let policy = retry_policy(&ctx.settings);
    let mut attempt = 0_u32;
    loop {
        let mut counted = 0_u64;
        match fetch_once(ctx, url, headers, range, &mut counted).await {
            Ok(fetched) => return Ok(fetched),
            Err(err) => {
                ctx.live.sub_media_bytes(counted);
                if !is_retryable(&err) || attempt >= policy.max_retries {
                    return Err(err);
                }
                attempt += 1;
                tracing::warn!(attempt, "media part failed, retrying: {err}");
                wait_or_cancel(ctx, policy.delay_for(attempt)).await?;
            }
        }
    }
}

/// Network errors, 429 and 5xx answers are worth another try.
fn is_retryable(err: &Error) -> bool {
    match err {
        Error::Http(_) | Error::Timeout(_) => true,
        Error::HttpStatus(code) => *code == 429 || *code >= 500,
        _ => false,
    }
}

/// Sleeps for the delay, or stops when the download is cancelled.
pub(crate) async fn wait_or_cancel(ctx: &TaskContext, delay: Duration) -> Result<()> {
    tokio::select! {
        _ = ctx.cancel.cancelled() => Err(Error::Transfer(CANCELLED.to_owned())),
        _ = tokio::time::sleep(delay) => Ok(()),
    }
}

async fn fetch_once(
    ctx: &TaskContext,
    url: &str,
    headers: &[HeaderPair],
    range: Option<ByteRange>,
    counted: &mut u64,
) -> Result<Fetched> {
    let wanted = range.map(|r| (r.offset, Some(r.offset + r.length.saturating_sub(1))));
    let request = get_request(&ctx.client, url, headers, wanted, None);
    let mut response = request.send().await?;
    let status = response.status().as_u16();
    if status == 429 || status >= 500 || !(200..300).contains(&status) {
        return Err(Error::HttpStatus(status));
    }
    let final_url = response.url().to_string();
    let mut bytes = Vec::new();
    loop {
        let chunk = tokio::select! {
            _ = ctx.cancel.cancelled() => {
                return Err(Error::Transfer(CANCELLED.to_owned()));
            }
            chunk = response.chunk() => chunk?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        let n = chunk.len() as u64;
        ctx.global_limiter.acquire(n).await;
        ctx.limiter.acquire(n).await;
        *counted += n;
        ctx.live.add_media_bytes(n);
        bytes.extend_from_slice(&chunk);
    }
    let bytes = match range {
        // The server ignored the range and sent the whole resource. Cut out the wanted part.
        Some(range) if status == 200 => slice_range(bytes, range)?,
        _ => bytes,
    };
    Ok(Fetched {
        bytes,
        url: final_url,
    })
}

fn slice_range(bytes: Vec<u8>, range: ByteRange) -> Result<Vec<u8>> {
    let start = usize::try_from(range.offset).unwrap_or(usize::MAX);
    let end = start.saturating_add(usize::try_from(range.length).unwrap_or(usize::MAX));
    bytes.get(start..end).map(<[u8]>::to_vec).ok_or_else(|| {
        Error::Protocol("the server sent less data than the byte range needs".to_owned())
    })
}

/// Writes a finished part through a `.part` file and renames it. Runs on a blocking thread.
pub(crate) async fn write_part(path: PathBuf, bytes: Vec<u8>) -> Result<()> {
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
        temp_name.push(".part");
        let temp = path.with_file_name(temp_name);
        std::fs::write(&temp, &bytes)?;
        std::fs::rename(&temp, &path)
    })
    .await
    .map_err(|err| Error::Transfer(format!("file task stopped: {err}")))??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_a_full_body_to_the_range() {
        let body = (0_u8..10).collect::<Vec<u8>>();
        let cut = slice_range(
            body,
            ByteRange {
                length: 3,
                offset: 4,
            },
        )
        .expect("slice");
        assert_eq!(cut, vec![4, 5, 6]);
    }

    #[test]
    fn short_body_is_a_protocol_error() {
        let body = vec![1_u8, 2];
        assert!(slice_range(
            body,
            ByteRange {
                length: 3,
                offset: 0
            }
        )
        .is_err());
    }

    #[test]
    fn only_temporary_failures_are_retried() {
        assert!(is_retryable(&Error::HttpStatus(503)));
        assert!(is_retryable(&Error::HttpStatus(429)));
        assert!(!is_retryable(&Error::HttpStatus(404)));
        assert!(!is_retryable(&Error::Protocol("x".to_owned())));
    }
}
