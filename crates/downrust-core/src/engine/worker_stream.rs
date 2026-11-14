// SPDX-License-Identifier: GPL-3.0-or-later
//! One HTTP request of a segment worker. Sends it, checks the reply and streams the body into
//! the segment table and the writer.
use bytes::BytesMut;
use reqwest::header::CONTENT_RANGE;
use reqwest::Response;
use tokio::time::Instant;

use super::segments::UNKNOWN_END;
use super::worker::{WorkerContext, FLUSH_BYTES, FLUSH_EVERY};
use super::writer::WriteCmd;
use crate::http::classify::{classify_status, StatusClass};
use crate::http::request::get_request;
use crate::util::content_range::parse_content_range;

/// Result of one HTTP request of a worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Attempt {
    Done,
    Cancelled,
    ResourceChanged,
    Busy(String),
    Retry { msg: String, progressed: bool },
    Fatal(String),
}

/// Maps a failing status code to the worker's reaction.
pub(crate) fn failure_for_status(code: u16) -> Attempt {
    match classify_status(code) {
        StatusClass::Success => Attempt::Fatal(format!("unexpected HTTP {code}")),
        StatusClass::Busy => Attempt::Busy(format!("the server is busy (HTTP {code})")),
        StatusClass::Retryable => Attempt::Retry {
            msg: format!("server error HTTP {code}"),
            progressed: false,
        },
        StatusClass::Fatal => Attempt::Fatal(format!("server responded with HTTP {code}")),
    }
}

/// True when a `Content-Range` value starts at `pos`.
pub(crate) fn range_starts_at(content_range: Option<&str>, pos: u64) -> bool {
    content_range
        .and_then(parse_content_range)
        .is_some_and(|range| range.start == pos)
}

/// Checks the reply status. Returns the reaction when the reply cannot be used.
fn check_reply(response: &Response, use_range: bool, pos: u64) -> Option<Attempt> {
    let code = response.status().as_u16();
    if !use_range {
        return (code != 200).then(|| failure_for_status(code));
    }
    match code {
        206 => {
            let value = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok());
            (!range_starts_at(value, pos)).then_some(Attempt::ResourceChanged)
        }
        200 | 416 => Some(Attempt::ResourceChanged),
        other => Some(failure_for_status(other)),
    }
}

/// Sends one request for the bytes from `pos` up to `end` and streams the reply.
pub(crate) async fn fetch_once(ctx: &WorkerContext, url: &str, pos: u64, end: u64) -> Attempt {
    let range = ctx
        .use_range
        .then(|| (pos, (end != UNKNOWN_END).then(|| end - 1)));
    let request = get_request(
        &ctx.client,
        url,
        &ctx.headers,
        range,
        ctx.if_range.as_deref(),
    )
    .send();
    let response = tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => return Attempt::Cancelled,
        sent = tokio::time::timeout(ctx.read_timeout, request) => match sent {
            Err(_) => return Attempt::Retry {
                msg: "the server did not answer in time".to_owned(),
                progressed: false,
            },
            Ok(Err(err)) => return Attempt::Retry { msg: err.to_string(), progressed: false },
            Ok(Ok(response)) => response,
        },
    };
    if response.status().as_u16() == 401 {
        return Attempt::Fatal(ctx.unauthorized.clone());
    }
    if let Some(reaction) = check_reply(&response, ctx.use_range, pos) {
        return reaction;
    }
    stream_body(ctx, response, pos).await
}

/// Reads the body until the segment is complete, the connection ends or a failure happens.
async fn stream_body(ctx: &WorkerContext, mut response: Response, pos: u64) -> Attempt {
    let mut buf = BytesMut::with_capacity(FLUSH_BYTES);
    let mut buf_off = pos;
    let mut last_flush = Instant::now();
    let mut progressed = false;
    loop {
        let next = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return finish(ctx, &mut buf, &mut buf_off, Attempt::Cancelled).await;
            }
            read = tokio::time::timeout(ctx.read_timeout, response.chunk()) => read,
        };
        let chunk = match next {
            Err(_) => {
                let retry = Attempt::Retry {
                    msg: "timed out while receiving data".to_owned(),
                    progressed,
                };
                return finish(ctx, &mut buf, &mut buf_off, retry).await;
            }
            Ok(Err(err)) => {
                let retry = Attempt::Retry {
                    msg: err.to_string(),
                    progressed,
                };
                return finish(ctx, &mut buf, &mut buf_off, retry).await;
            }
            Ok(Ok(None)) => return end_of_body(ctx, &mut buf, &mut buf_off, progressed).await,
            Ok(Ok(Some(chunk))) => chunk,
        };
        let n = chunk.len() as u64;
        let paced = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => false,
            () = pace(ctx, n) => true,
        };
        if !paced {
            return finish(ctx, &mut buf, &mut buf_off, Attempt::Cancelled).await;
        }
        let accepted = ctx.table.accept(ctx.slot, n);
        let take = accepted.take.min(n) as usize;
        if take > 0 {
            buf.extend_from_slice(&chunk[..take]);
            progressed = true;
        }
        let due = buf.len() >= FLUSH_BYTES || last_flush.elapsed() >= FLUSH_EVERY;
        if due || accepted.reached_end {
            if let Err(msg) = flush(ctx, &mut buf, &mut buf_off).await {
                return Attempt::Fatal(msg);
            }
            last_flush = Instant::now();
        }
        if accepted.reached_end {
            return Attempt::Done;
        }
    }
}

/// Handles the end of the body. The segment is complete, or the connection closed early.
async fn end_of_body(
    ctx: &WorkerContext,
    buf: &mut BytesMut,
    buf_off: &mut u64,
    progressed: bool,
) -> Attempt {
    if let Err(msg) = flush(ctx, buf, buf_off).await {
        return Attempt::Fatal(msg);
    }
    let (pos, end) = ctx.table.position(ctx.slot);
    if end == UNKNOWN_END {
        ctx.table.finish_unknown(ctx.slot);
        return Attempt::Done;
    }
    if pos >= end {
        Attempt::Done
    } else {
        Attempt::Retry {
            msg: "connection closed early".to_owned(),
            progressed,
        }
    }
}

/// Writes what is buffered, then returns `outcome`. A failed write replaces it with `Fatal`.
async fn finish(
    ctx: &WorkerContext,
    buf: &mut BytesMut,
    buf_off: &mut u64,
    outcome: Attempt,
) -> Attempt {
    match flush(ctx, buf, buf_off).await {
        Ok(()) => outcome,
        Err(msg) => Attempt::Fatal(msg),
    }
}

/// Waits until the global and the per-download limiter allow `n` bytes.
async fn pace(ctx: &WorkerContext, n: u64) {
    ctx.global_limiter.acquire(n).await;
    ctx.limiter.acquire(n).await;
}

/// Sends the buffered bytes to the writer. The buffer is empty afterwards.
async fn flush(ctx: &WorkerContext, buf: &mut BytesMut, buf_off: &mut u64) -> Result<(), String> {
    if buf.is_empty() {
        return Ok(());
    }
    let len = buf.len() as u64;
    let data = buf.split().freeze();
    ctx.write_tx
        .send(WriteCmd::Data {
            slot: ctx.slot,
            offset: *buf_off,
            data,
        })
        .await
        .map_err(|_| "disk writer stopped".to_owned())?;
    *buf_off += len;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_map_to_reactions() {
        assert_eq!(
            failure_for_status(404),
            Attempt::Fatal("server responded with HTTP 404".to_owned())
        );
        assert!(matches!(failure_for_status(429), Attempt::Busy(_)));
        assert!(matches!(failure_for_status(503), Attempt::Busy(_)));
        assert!(matches!(
            failure_for_status(500),
            Attempt::Retry {
                progressed: false,
                ..
            }
        ));
        assert!(matches!(failure_for_status(408), Attempt::Retry { .. }));
        assert!(matches!(failure_for_status(200), Attempt::Fatal(_)));
    }

    #[test]
    fn content_range_must_start_at_position() {
        assert!(range_starts_at(Some("bytes 100-199/500"), 100));
        assert!(!range_starts_at(Some("bytes 0-99/500"), 100));
        assert!(!range_starts_at(Some("garbage"), 0));
        assert!(!range_starts_at(None, 0));
    }
}
