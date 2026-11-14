// SPDX-License-Identifier: GPL-3.0-or-later
//! SHA-256 of finished files, and the check that follows a transfer when an expected value is set.
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};
use crate::model::{ChecksumState, EngineEvent};

const BUFFER_SIZE: usize = 64 * 1024;

/// Hex SHA-256 of a whole file, read in 64 KiB pieces.
pub(crate) fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; BUFFER_SIZE];
    loop {
        let read = match file.read(&mut buf) {
            Ok(read) => read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        };
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Runs after a transfer has finished. Without an expected value the outcome stays `Completed`.
/// With one, the final file is hashed and compared; a mismatch fails the task.
pub(crate) async fn verify_completed(ctx: &TaskContext) -> TaskOutcome {
    match check(ctx).await {
        Ok(outcome) => outcome,
        Err(err) => TaskOutcome::Failed(err.to_string()),
    }
}

async fn check(ctx: &TaskContext) -> Result<TaskOutcome> {
    let Some(record) = ctx.store.get_download(ctx.id)? else {
        return Ok(TaskOutcome::Completed);
    };
    let Some(expected) = record.expected_sha256.clone() else {
        return Ok(TaskOutcome::Completed);
    };
    ctx.store
        .set_checksum(ctx.id, Some(&expected), Some(ChecksumState::Verifying))?;
    send_updated(ctx)?;

    let path = record.final_path();
    let hashed = tokio::task::spawn_blocking(move || hash_file(&path))
        .await
        .map_err(|err| Error::Transfer(format!("checksum task stopped: {err}")))?;
    let actual = hashed?;

    if actual == expected {
        ctx.store
            .set_checksum(ctx.id, Some(&expected), Some(ChecksumState::Ok))?;
        send_updated(ctx)?;
        Ok(TaskOutcome::Completed)
    } else {
        ctx.store
            .set_checksum(ctx.id, Some(&expected), Some(ChecksumState::Mismatch))?;
        send_updated(ctx)?;
        Ok(TaskOutcome::Failed(format!(
            "Checksum mismatch: expected {expected}, got {actual}."
        )))
    }
}

fn send_updated(ctx: &TaskContext) -> Result<()> {
    if let Some(record) = ctx.store.get_download(ctx.id)? {
        let _ = ctx.events.send(EngineEvent::Updated(record));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::hash_file;
    use sha2::{Digest, Sha256};

    #[test]
    fn hashes_known_content() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("abc.txt");
        std::fs::write(&path, b"abc").expect("write");
        assert_eq!(
            hash_file(&path).expect("hash"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hashes_content_larger_than_one_buffer() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("big.bin");
        let data = vec![7u8; 200_000];
        std::fs::write(&path, &data).expect("write");
        let expected = hex::encode(Sha256::digest(&data));
        assert_eq!(hash_file(&path).expect("hash"), expected);
    }
}
