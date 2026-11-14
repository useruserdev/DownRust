// SPDX-License-Identifier: GPL-3.0-or-later
//! Copies the bytes of a remote reader into the part file. Pacing, live progress, checkpoints and
//! cancel are handled here for every protocol.
use std::fs::File;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::Instant;

use super::{fatal, retry, Failure, Step};
use crate::engine::file::write_all_at;
use crate::engine::task::TaskContext;

/// Largest piece read from the server in one step.
const CHUNK: usize = 64 * 1024;

/// Reads `reader` from `start` to its end and writes it into `file` at the same offsets.
/// Returns the offset reached. A cancel saves the progress and gives `Failure::Cancelled`.
pub(super) async fn copy_to_part<R>(
    ctx: &TaskContext,
    file: &File,
    start: u64,
    reader: &mut R,
) -> Step<u64>
where
    R: AsyncRead + Unpin,
{
    let read_limit = Duration::from_secs(ctx.settings.read_timeout_secs);
    let period = Duration::from_millis(ctx.settings.checkpoint_interval_ms);
    let mut buf = vec![0u8; CHUNK];
    let mut offset = start;
    let mut last_save = Instant::now();
    loop {
        let read = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                save(ctx, offset)?;
                return Err(Failure::Cancelled);
            }
            read = tokio::time::timeout(read_limit, reader.read(&mut buf)) => read,
        };
        let n = match read {
            Err(_) => return Err(retry("the server stopped sending data", offset > start)),
            Ok(Err(err)) => return Err(retry(err.to_string(), offset > start)),
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => n,
        };
        tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                save(ctx, offset)?;
                return Err(Failure::Cancelled);
            }
            () = pace(ctx, n as u64) => {}
        }
        write_all_at(file, &buf[..n], offset).map_err(fatal)?;
        offset += n as u64;
        ctx.live.add_media_bytes(n as u64);
        if last_save.elapsed() >= period {
            save(ctx, offset)?;
            last_save = Instant::now();
        }
    }
    save(ctx, offset)?;
    Ok(offset)
}

/// Waits until the global and the per-download limiter allow `n` bytes.
async fn pace(ctx: &TaskContext, n: u64) {
    ctx.global_limiter.acquire(n).await;
    ctx.limiter.acquire(n).await;
}

/// Stores the offset reached as the downloaded size.
fn save(ctx: &TaskContext, offset: u64) -> Step<()> {
    ctx.store
        .save_checkpoint(ctx.id, offset, &[])
        .map_err(fatal)
}
