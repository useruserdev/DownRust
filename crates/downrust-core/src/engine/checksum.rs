// SPDX-License-Identifier: GPL-3.0-or-later
//! SHA-256 of finished files, and the check that follows a transfer when an expected value is set.
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use std::time::Duration;

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};

use super::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};
use crate::http::request::get_request;
use crate::model::{ChecksumState, DownloadKind, DownloadRecord, EngineEvent, HeaderPair};

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
    let expected = match record.expected_sha256.clone() {
        Some(expected) => expected,
        None => match discover_published(ctx, &record).await {
            Some(found) => found,
            None => return Ok(TaskOutcome::Completed),
        },
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

/// Largest published checksum file that is read.
const MAX_PUBLISHED_BYTES: usize = 1024 * 1024;
const PUBLISHED_TIMEOUT: Duration = Duration::from_secs(10);

/// Looks for a SHA-256 published next to a finished HTTP file. Returns the first hash found.
async fn discover_published(ctx: &TaskContext, record: &DownloadRecord) -> Option<String> {
    if !ctx.settings.auto_checksum || record.kind != DownloadKind::File {
        return None;
    }
    let mut base = reqwest::Url::parse(&record.url).ok()?;
    if !matches!(base.scheme(), "http" | "https") {
        return None;
    }
    base.set_query(None);
    base.set_fragment(None);
    let file_name = base
        .path_segments()?
        .next_back()
        .map(|name| percent_decode_str(name).decode_utf8_lossy().into_owned())?;
    let candidates = [
        format!("{base}.sha256"),
        format!("{base}.sha256sum"),
        base.join("SHA256SUMS").ok()?.to_string(),
        base.join("sha256sums.txt").ok()?.to_string(),
        base.join("SHA256SUMS.txt").ok()?.to_string(),
    ];
    let headers = record.request_headers();
    for candidate in candidates {
        let Some(text) = fetch_published(ctx, &candidate, &headers).await else {
            continue;
        };
        if let Some(hash) = parse_published(&text, &file_name) {
            return Some(hash);
        }
    }
    None
}

/// Reads a small text file with a GET request. `None` for any failure, status other than 200 or size over the limit.
async fn fetch_published(ctx: &TaskContext, url: &str, headers: &[HeaderPair]) -> Option<String> {
    tokio::time::timeout(PUBLISHED_TIMEOUT, async {
        let mut response = get_request(&ctx.client, url, headers, None, None)
            .send()
            .await
            .ok()?;
        if response.status() != reqwest::StatusCode::OK {
            return None;
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            if body.len() + chunk.len() > MAX_PUBLISHED_BYTES {
                return None;
            }
            body.extend_from_slice(&chunk);
        }
        Some(String::from_utf8_lossy(&body).into_owned())
    })
    .await
    .ok()
    .flatten()
}

/// The first token of a line when it is a 64-character hex hash, and the text after it.
fn split_hash(line: &str) -> Option<(&str, &str)> {
    let end = line.find(char::is_whitespace).unwrap_or(line.len());
    let (token, rest) = line.split_at(end);
    (token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())).then_some((token, rest))
}

/// Finds the hash for `file_name` in a published file. A line `<hash> [ *]<name>` must name the file,
/// compared without case. A file that holds only one hash also counts.
pub(crate) fn parse_published(text: &str, file_name: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    for line in &lines {
        if let Some((hash, rest)) = split_hash(line) {
            let name = rest.trim_start_matches([' ', '\t', '*']);
            if name.eq_ignore_ascii_case(file_name) {
                return Some(hash.to_ascii_lowercase());
            }
        }
    }
    match lines.as_slice() {
        [only] => split_hash(only)
            .filter(|(_, rest)| rest.trim().is_empty())
            .map(|(hash, _)| hash.to_ascii_lowercase()),
        _ => None,
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
    use super::{hash_file, parse_published};
    use sha2::{Digest, Sha256};

    const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn one_hash_file_gives_its_hash() {
        assert_eq!(
            parse_published(&format!("{HASH}\n"), "abc.txt"),
            Some(HASH.to_owned())
        );
    }

    #[test]
    fn one_hash_file_with_another_name_is_refused() {
        assert_eq!(
            parse_published(&format!("{HASH}  other.txt\n"), "abc.txt"),
            None
        );
    }

    #[test]
    fn sums_file_matches_the_name_without_case() {
        let zeros = "0".repeat(64);
        let text = format!("{HASH}  Other.bin\n{zeros}  ABC.txt\n");
        assert_eq!(parse_published(&text, "abc.txt"), Some(zeros));
    }

    #[test]
    fn sums_file_accepts_binary_mode_marker() {
        let text = format!("{HASH} *abc.txt\n");
        assert_eq!(parse_published(&text, "abc.txt"), Some(HASH.to_owned()));
    }

    #[test]
    fn sums_file_without_the_name_gives_nothing() {
        let text = format!("{HASH}  other.txt\n{HASH}  more.txt\n");
        assert_eq!(parse_published(&text, "abc.txt"), None);
    }

    #[test]
    fn text_without_a_hash_gives_nothing() {
        assert_eq!(parse_published("<html>not found</html>", "abc.txt"), None);
    }

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
