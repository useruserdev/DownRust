// SPDX-License-Identifier: GPL-3.0-or-later
//! Downloads from video sites with yt-dlp.
mod args;
mod info;
mod progress;
mod tool;

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStderr;

pub use info::{site_info, SiteInfo};
pub use tool::{ensure_tool, find_tool, tool_version, update_tool};

use super::file::finalize;
use super::media::ffmpeg::{find_ffmpeg, hidden_std_command};
use super::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};
use crate::model::{DownloadRecord, Phase, SegmentView};
use crate::settings::{EngineSettings, SiteFormat, SiteSettings};
use args::{download_args, proxy_for, request_args};
use progress::{parse_line, PartTracker};

/// Connections yt-dlp may use for one video.
const MAX_CONNECTIONS: u8 = 8;
const PART_SUFFIXES: [&str; 3] = [".part", ".ytdl", ".temp"];

/// The options JSON stored with a video site download.
#[derive(Deserialize)]
struct SiteOptions {
    format: Option<SiteFormat>,
}

/// Runs a download of this kind to its end. A cancel gives `Cancelled`.
pub(crate) async fn run(ctx: &TaskContext) -> Result<TaskOutcome> {
    run_inner(ctx).await
}

/// File name used when the request gives none.
pub(crate) fn initial_name(_url: &str) -> String {
    "video.mp4".to_owned()
}

async fn run_inner(ctx: &TaskContext) -> Result<TaskOutcome> {
    let record = ctx
        .store
        .get_download(ctx.id)?
        .ok_or(Error::NotFound(ctx.id))?;
    let tool = ensure_tool(&ctx.settings, &ctx.client).await?;
    let ffmpeg = find_ffmpeg(&ctx.settings.media);
    let format = chosen_format(&record, &ctx.settings.site);
    let save_dir = PathBuf::from(&record.save_dir);
    let work = save_dir.join(format!(".{}.site", record.file_name));
    std::fs::create_dir_all(&work)?;

    let connections = record.connections.clamp(1, MAX_CONNECTIONS);
    let request = request_args(
        &ctx.settings.user_agent,
        &record.request_headers(),
        proxy_for(&ctx.settings, &record.url),
    );
    let args = download_args(
        &record.url,
        &work,
        format,
        connections,
        download_rate(&record, &ctx.settings),
        ffmpeg.as_deref(),
        &request,
    );

    let mut command = tokio::process::Command::from(hidden_std_command(&tool));
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Transfer("yt-dlp output is not available".to_owned()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::Transfer("yt-dlp errors are not available".to_owned()))?;
    let error_reader = tokio::spawn(read_errors(stderr));

    ctx.live.set_phase(Phase::Downloading);
    ctx.live.set_connections(u32::from(connections));
    let mut lines = BufReader::new(stdout).lines();
    let mut tracker = PartTracker::default();
    loop {
        tokio::select! {
            _ = ctx.cancel.cancelled() => {
                let _ = child.kill().await;
                return Ok(TaskOutcome::Cancelled);
            }
            line = lines.next_line() => match line? {
                Some(text) => {
                    if text.starts_with("[Merger]") {
                        ctx.live.set_phase(Phase::Finalizing);
                    } else if let Some(sample) = parse_line(&text) {
                        tracker.push(sample);
                        report(ctx, &tracker);
                    }
                }
                None => break,
            },
        }
    }
    let status = child.wait().await?;
    let errors = error_reader.await.unwrap_or_default();
    if !status.success() {
        return Err(Error::Transfer(errors.message()));
    }

    ctx.live.set_phase(Phase::Finalizing);
    let produced = largest_output(&work)?
        .ok_or_else(|| Error::Transfer("yt-dlp finished without a file".to_owned()))?;
    let extension = produced
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("mp4")
        .to_owned();
    let stem = Path::new(&record.file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("video")
        .to_owned();
    let final_name = finalize(&produced, &save_dir, &format!("{stem}.{extension}")).await?;
    let size = std::fs::metadata(save_dir.join(&final_name))?.len();
    ctx.store.record_finished_file(ctx.id, &final_name, size)?;
    if let Err(err) = std::fs::remove_dir_all(&work) {
        tracing::warn!("cannot remove the work folder: {err}");
    }
    Ok(TaskOutcome::Completed)
}

/// The quality of the download: the options JSON when it names one, otherwise the default.
fn chosen_format(record: &DownloadRecord, settings: &SiteSettings) -> SiteFormat {
    record
        .options
        .as_deref()
        .and_then(|text| serde_json::from_str::<SiteOptions>(text).ok())
        .and_then(|options| options.format)
        .unwrap_or(settings.default_format)
}

/// The lower of the download limit and the global limit, `None` when neither is set.
fn download_rate(record: &DownloadRecord, settings: &EngineSettings) -> Option<u64> {
    [record.speed_limit, settings.global_speed_limit]
        .into_iter()
        .flatten()
        .filter(|limit| *limit > 0)
        .min()
}

/// Publishes the bytes of the video so far and the expected total.
fn report(ctx: &TaskContext, tracker: &PartTracker) {
    let done = tracker.done();
    let total = tracker.total();
    ctx.live.set_media_done(done);
    ctx.live.set_media_total(total);
    let views = match total {
        Some(total) if total > 0 => vec![SegmentView {
            start: 0,
            end: total,
            done: done.min(total),
            active: true,
        }],
        _ => Vec::new(),
    };
    ctx.live.set_media_views(views);
}

/// The biggest file of the work folder that is not a partial download.
fn largest_output(dir: &Path) -> Result<Option<PathBuf>> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if PART_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)) {
            continue;
        }
        let size = entry.metadata()?.len();
        let path = entry.path();
        match &best {
            Some((current, _)) if *current >= size => {}
            _ => best = Some((size, path)),
        }
    }
    Ok(best.map(|(_, path)| path))
}

/// Keeps the last error of yt-dlp, read from its error output.
#[derive(Default)]
struct ErrorLog {
    last_error: Option<String>,
    last_line: Option<String>,
}

impl ErrorLog {
    fn push(&mut self, line: &str) {
        let text = line.trim();
        if text.is_empty() {
            return;
        }
        if let Some(rest) = text.strip_prefix("ERROR: ") {
            self.last_error = Some(rest.to_owned());
        }
        self.last_line = Some(text.to_owned());
    }

    fn message(&self) -> String {
        self.last_error
            .clone()
            .or_else(|| self.last_line.clone())
            .unwrap_or_else(|| "no message".to_owned())
    }
}

async fn read_errors(stderr: ChildStderr) -> ErrorLog {
    let mut log = ErrorLog::default();
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        log.push(&line);
    }
    log
}

/// The error message of a finished yt-dlp run, read from its error text.
fn error_message(stderr: &str) -> String {
    let mut log = ErrorLog::default();
    for line in stderr.lines() {
        log.push(line);
    }
    log.message()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_prefers_the_last_error_line() {
        let text = "WARNING: x\nERROR: [youtube] abc: Video unavailable\nsome trailer\n";
        assert_eq!(error_message(text), "[youtube] abc: Video unavailable");
        assert_eq!(
            error_message("ERROR: first\nERROR: [youtube] abc: Private video\n"),
            "[youtube] abc: Private video"
        );
        assert_eq!(error_message(""), "no message");
    }

    #[test]
    fn largest_file_skips_partial_downloads() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("video.mp4.part"), vec![0; 900]).expect("write");
        std::fs::write(dir.path().join("video.f137.mp4.ytdl"), vec![0; 800]).expect("write");
        std::fs::write(dir.path().join("video.mp4"), vec![0; 100]).expect("write");
        std::fs::write(dir.path().join("video.m4a"), vec![0; 50]).expect("write");
        let found = largest_output(dir.path())
            .expect("lists")
            .expect("some file");
        assert_eq!(
            found.file_name().and_then(|n| n.to_str()),
            Some("video.mp4")
        );
        let empty = tempfile::tempdir().expect("temp dir");
        assert_eq!(largest_output(empty.path()).expect("lists"), None);
    }

    #[test]
    fn initial_name_is_video_mp4() {
        assert_eq!(initial_name("https://youtu.be/x"), "video.mp4");
    }
}
