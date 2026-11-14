// SPDX-License-Identifier: GPL-3.0-or-later
//! DASH downloads. ffmpeg reads the manifest and writes one MP4 file. Progress comes from the
//! `out_time_us` lines of ffmpeg's progress output. A DASH download has no resume: it starts from zero.
use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use super::fetch::fetch;
use super::ffmpeg::{find_ffmpeg, hidden_std_command, CANCELLED, NEEDS_FFMPEG};
use super::hls::media_dir;
use crate::engine::file::finalize;
use crate::engine::task::{TaskContext, TaskOutcome};
use crate::error::{Error, Result};
use crate::model::{DownloadRecord, HeaderPair, Phase, SegmentView};

/// Runs a DASH download to its end. A cancel gives `Cancelled`.
pub(crate) async fn run(ctx: &TaskContext) -> Result<TaskOutcome> {
    match run_inner(ctx).await {
        Err(Error::Transfer(message)) if message == CANCELLED => Ok(TaskOutcome::Cancelled),
        other => other,
    }
}

async fn run_inner(ctx: &TaskContext) -> Result<TaskOutcome> {
    let record = ctx
        .store
        .get_download(ctx.id)?
        .ok_or(Error::NotFound(ctx.id))?;
    let program =
        find_ffmpeg(&ctx.settings.media).ok_or_else(|| Error::Transfer(NEEDS_FFMPEG.to_owned()))?;
    let headers = record.request_headers();
    let manifest = fetch(ctx, &record.url, &headers, None).await?;
    let duration_us = mpd_duration(&String::from_utf8_lossy(&manifest.bytes))
        .map(|secs| (secs * 1_000_000.0) as u64);

    let dir = media_dir(&record);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    let out = dir.join("output.mp4");
    tracing::info!(
        id = ctx.id,
        "DASH transfers have no resume and start from zero"
    );

    let args = ffmpeg_args(&record, &headers, &ctx.settings.user_agent, &out);
    let mut command = tokio::process::Command::from(hidden_std_command(&program));
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
        .ok_or_else(|| Error::Transfer("ffmpeg output is not available".to_owned()))?;
    ctx.live.set_phase(Phase::Downloading);
    ctx.live.set_connections(1);
    let mut lines = BufReader::new(stdout).lines();
    loop {
        tokio::select! {
            _ = ctx.cancel.cancelled() => {
                let _ = child.kill().await;
                return Ok(TaskOutcome::Cancelled);
            }
            line = lines.next_line() => match line? {
                Some(text) => {
                    if let Some(us) = progress_time_us(&text) {
                        report(ctx, &out, us, duration_us);
                    }
                }
                None => break,
            },
        }
    }
    let status = child.wait().await?;
    if !status.success() {
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr).await;
        }
        let last = stderr.lines().rev().find(|line| !line.trim().is_empty());
        return Err(Error::Transfer(format!(
            "ffmpeg failed: {}",
            last.unwrap_or("no message")
        )));
    }

    ctx.live.set_phase(Phase::Finalizing);
    let save_dir = PathBuf::from(&record.save_dir);
    let stem = std::path::Path::new(&record.file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("video")
        .to_owned();
    let final_name = finalize(&out, &save_dir, &format!("{stem}.mp4")).await?;
    let size = std::fs::metadata(save_dir.join(&final_name))?.len();
    ctx.store.record_finished_file(ctx.id, &final_name, size)?;
    if let Err(err) = std::fs::remove_dir_all(&dir) {
        tracing::warn!("cannot remove the part folder: {err}");
    }
    Ok(TaskOutcome::Completed)
}

/// Command line of the ffmpeg run: request headers, progress output and the copy into the file.
pub(crate) fn ffmpeg_args(
    record: &DownloadRecord,
    headers: &[HeaderPair],
    user_agent: &str,
    out: &std::path::Path,
) -> Vec<String> {
    let mut args = ffmpeg_header_args(headers, user_agent);
    let fixed = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-progress",
        "pipe:1",
        "-i",
    ];
    args.extend(fixed.iter().map(|arg| (*arg).to_owned()));
    args.push(record.url.clone());
    args.extend(["-c", "copy"].iter().map(|arg| (*arg).to_owned()));
    args.push(out.to_string_lossy().into_owned());
    args
}

/// Turns request headers into ffmpeg options. Cookie and the other headers go in `-headers`,
/// and the user agent and referrer have their own options.
pub(crate) fn ffmpeg_header_args(headers: &[HeaderPair], user_agent: &str) -> Vec<String> {
    let mut lines = String::new();
    let mut user_agent_value = user_agent.to_owned();
    let mut referer = None;
    for header in headers {
        if header.name.eq_ignore_ascii_case("user-agent") {
            user_agent_value = header.value.clone();
        } else if header.name.eq_ignore_ascii_case("referer") {
            referer = Some(header.value.clone());
        } else {
            lines.push_str(&format!("{}: {}\r\n", header.name, header.value));
        }
    }
    let mut args = Vec::new();
    if !lines.is_empty() {
        args.push("-headers".to_owned());
        args.push(lines);
    }
    if let Some(referer) = referer {
        args.push("-referer".to_owned());
        args.push(referer);
    }
    args.push("-user_agent".to_owned());
    args.push(user_agent_value);
    args
}

/// The time in microseconds from one `out_time_us=` line of the progress output.
pub(crate) fn progress_time_us(line: &str) -> Option<u64> {
    line.strip_prefix("out_time_us=")?.trim().parse().ok()
}

/// Publishes the bytes written so far and, when the duration is known, the time position.
fn report(ctx: &TaskContext, out: &std::path::Path, out_us: u64, duration_us: Option<u64>) {
    let bytes = std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0);
    ctx.live.set_media_done(bytes);
    let views = match duration_us {
        Some(total) if total > 0 => vec![SegmentView {
            start: 0,
            end: total,
            done: out_us.min(total),
            active: true,
        }],
        _ => Vec::new(),
    };
    ctx.live.set_media_views(views);
}

/// Duration in seconds of a `mediaPresentationDuration` attribute, when the manifest has one.
pub(crate) fn mpd_duration(manifest: &str) -> Option<f64> {
    let marker = "mediaPresentationDuration=\"";
    let start = manifest.find(marker)? + marker.len();
    let rest = &manifest[start..];
    let end = rest.find('"')?;
    iso_duration_secs(&rest[..end])
}

/// Parses an ISO 8601 duration such as `PT1H2M3.5S` or `P1DT2H`. Years count as 365 days and
/// months as 30 days.
pub(crate) fn iso_duration_secs(text: &str) -> Option<f64> {
    let body = text.trim().strip_prefix('P')?;
    let (date, time) = match body.split_once('T') {
        Some((date, time)) => (date, time),
        None => (body, ""),
    };
    let date_total = sum_units(
        date,
        &[
            ('Y', 365.0 * 86_400.0),
            ('M', 30.0 * 86_400.0),
            ('D', 86_400.0),
        ],
    )?;
    let time_total = sum_units(time, &[('H', 3_600.0), ('M', 60.0), ('S', 1.0)])?;
    Some(date_total + time_total)
}

/// Sums `number unit` pairs. A number without a unit at the end is an error.
fn sum_units(text: &str, units: &[(char, f64)]) -> Option<f64> {
    let mut total = 0.0;
    let mut number = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
            continue;
        }
        let (_, factor) = units.iter().find(|(unit, _)| *unit == ch)?;
        let value: f64 = number.parse().ok()?;
        total += value * factor;
        number.clear();
    }
    if number.is_empty() {
        Some(total)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_durations_are_parsed() {
        assert_eq!(iso_duration_secs("PT1H2M3.5S"), Some(3723.5));
        assert_eq!(iso_duration_secs("PT0S"), Some(0.0));
        assert_eq!(iso_duration_secs("P1DT2H"), Some(93_600.0));
        assert_eq!(iso_duration_secs("PT30M"), Some(1800.0));
        assert_eq!(iso_duration_secs("P1M"), Some(2_592_000.0));
        assert_eq!(iso_duration_secs("1H"), None);
        assert_eq!(iso_duration_secs("PT5"), None);
        assert_eq!(iso_duration_secs("PT5X"), None);
    }

    #[test]
    fn manifest_duration_is_read_from_the_attribute() {
        let mpd =
            "<MPD type=\"static\" mediaPresentationDuration=\"PT10M5S\" minBufferTime=\"PT2S\">";
        assert_eq!(mpd_duration(mpd), Some(605.0));
        assert_eq!(mpd_duration("<MPD type=\"dynamic\">"), None);
    }

    #[test]
    fn progress_lines_give_the_time() {
        assert_eq!(progress_time_us("out_time_us=1500000"), Some(1_500_000));
        assert_eq!(progress_time_us("out_time_us=N/A"), None);
        assert_eq!(progress_time_us("progress=continue"), None);
    }

    #[test]
    fn headers_become_ffmpeg_options() {
        let headers = [
            HeaderPair {
                name: "Cookie".into(),
                value: "a=1".into(),
            },
            HeaderPair {
                name: "User-Agent".into(),
                value: "Browser/1".into(),
            },
            HeaderPair {
                name: "Referer".into(),
                value: "https://page.test/".into(),
            },
        ];
        let args = ffmpeg_header_args(&headers, "fallback");
        assert_eq!(
            args,
            vec![
                "-headers".to_owned(),
                "Cookie: a=1\r\n".to_owned(),
                "-referer".to_owned(),
                "https://page.test/".to_owned(),
                "-user_agent".to_owned(),
                "Browser/1".to_owned(),
            ]
        );
        let plain = ffmpeg_header_args(&[], "fallback");
        assert_eq!(plain, vec!["-user_agent".to_owned(), "fallback".to_owned()]);
    }
}
