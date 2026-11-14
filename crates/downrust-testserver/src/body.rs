// SPDX-License-Identifier: GPL-3.0-or-later
//! Streamed response bodies for file requests: paced, optionally cut, and counted.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use bytes::Bytes;
use futures_util::stream::{self, Stream};

use crate::content::fill;
use crate::state::{ActiveGuard, ServerState};

/// Size of one body chunk in bytes.
const CHUNK: u64 = 16 * 1024;

/// Everything needed to produce a file body.
pub(crate) struct BodyPlan {
    pub version: u64,
    pub size: u64,
    /// First file byte to send.
    pub start: u64,
    /// Number of bytes to send.
    pub len: u64,
    /// Bytes per second. `0` means no pacing.
    pub rate: u64,
    /// Body byte count at which the stream fails, if any.
    pub fail_at: Option<u64>,
    pub state: Arc<ServerState>,
    pub _guard: ActiveGuard,
}

/// Progress of one body stream.
struct Cursor {
    plan: BodyPlan,
    sent: u64,
    failed: bool,
}

/// Builds the response body for a file range.
pub(crate) fn file_body(plan: BodyPlan) -> Body {
    Body::from_stream(chunks(plan))
}

fn chunks(plan: BodyPlan) -> impl Stream<Item = Result<Bytes, io::Error>> {
    let cursor = Cursor {
        plan,
        sent: 0,
        failed: false,
    };
    stream::unfold(cursor, |mut cur| async move {
        if cur.failed || cur.sent >= cur.plan.len {
            return None;
        }
        if cur.plan.fail_at.is_some_and(|at| cur.sent >= at) {
            cur.failed = true;
            // Let the connection task flush the head and the bytes already produced before the error.
            tokio::task::yield_now().await;
            return Some((Err(io::Error::other("injected")), cur));
        }
        let mut n = CHUNK.min(cur.plan.len - cur.sent);
        if let Some(at) = cur.plan.fail_at {
            n = n.min(at - cur.sent);
        }
        if cur.plan.rate > 0 {
            let pause = n as f64 / cur.plan.rate as f64;
            tokio::time::sleep(Duration::from_secs_f64(pause)).await;
        }
        let mut buf = vec![0u8; n as usize];
        fill(
            cur.plan.version,
            cur.plan.size,
            cur.plan.start + cur.sent,
            &mut buf,
        );
        cur.sent += n;
        cur.plan.state.add_bytes(n);
        Some((Ok(Bytes::from(buf)), cur))
    })
}
