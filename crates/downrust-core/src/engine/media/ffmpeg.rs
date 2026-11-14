// SPDX-License-Identifier: GPL-3.0-or-later
//! Finds ffmpeg, reads its version, and runs it for remuxing and DASH downloads.
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::settings::MediaSettings;

/// Process creation flag that keeps a console window from opening on Windows.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A usable ffmpeg program.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegInfo {
    pub path: String,
    pub version: String,
}

/// Finds ffmpeg and reads its version line. `None` when no program runs.
pub fn detect_ffmpeg(settings: &MediaSettings) -> Option<FfmpegInfo> {
    let path = find_ffmpeg(settings)?;
    let version = ffmpeg_version(&path)?;
    Some(FfmpegInfo {
        path: path.to_string_lossy().into_owned(),
        version,
    })
}

/// The configured path when set, otherwise `ffmpeg` next to the program, otherwise the first
/// match on PATH. A configured path that is not a file gives `None`.
pub(crate) fn find_ffmpeg(settings: &MediaSettings) -> Option<PathBuf> {
    find_in(
        &settings.ffmpeg_path,
        std::env::current_exe().ok().as_deref(),
        std::env::var_os("PATH").as_deref(),
    )
}

/// The lookup order of `find_ffmpeg`, with the environment passed in for testing.
fn find_in(
    configured: &str,
    exe: Option<&Path>,
    path_var: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        return path.is_file().then_some(path);
    }
    let name = program_name();
    if let Some(dir) = exe.and_then(Path::parent) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let paths = path_var?;
    std::env::split_paths(paths)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn program_name() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

/// First line of `ffmpeg -version`, or `None` when the program does not run.
pub(crate) fn ffmpeg_version(path: &Path) -> Option<String> {
    let output = hidden_std_command(path).arg("-version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_owned())
}

/// A command that runs without a console window on Windows.
pub(crate) fn hidden_std_command(program: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Message used when a run stops because the user cancelled it.
pub(crate) const CANCELLED: &str = "cancelled";

/// Runs ffmpeg to the end. A cancel kills the process.
pub(crate) async fn run_to_end(
    program: &Path,
    args: &[String],
    cancel: &CancellationToken,
) -> Result<()> {
    let mut command = tokio::process::Command::from(hidden_std_command(program));
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.spawn()?;
    let output = tokio::select! {
        _ = cancel.cancelled() => return Err(Error::Transfer(CANCELLED.to_owned())),
        output = child.wait_with_output() => output?,
    };
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last = stderr.lines().rev().find(|line| !line.trim().is_empty());
        Err(Error::Transfer(format!(
            "ffmpeg failed: {}",
            last.unwrap_or("no message")
        )))
    }
}

/// Copies the streams of a TS file into an MP4 file without re-encoding.
pub(crate) async fn remux_to_mp4(
    program: &Path,
    input: &Path,
    output: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    let args = [
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
        "-y".to_owned(),
        "-i".to_owned(),
        input.to_string_lossy().into_owned(),
        "-c".to_owned(),
        "copy".to_owned(),
        "-bsf:a".to_owned(),
        "aac_adtstoasc".to_owned(),
        output.to_string_lossy().into_owned(),
    ];
    run_to_end(program, &args, cancel).await
}

/// Combines the video of one file with the audio of another into an MP4 file, without re-encoding.
pub(crate) async fn mux_to_mp4(
    program: &Path,
    video: &Path,
    audio: &Path,
    output: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    let args = [
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
        "-y".to_owned(),
        "-i".to_owned(),
        video.to_string_lossy().into_owned(),
        "-i".to_owned(),
        audio.to_string_lossy().into_owned(),
        "-map".to_owned(),
        "0:v:0".to_owned(),
        "-map".to_owned(),
        "1:a:0".to_owned(),
        "-c".to_owned(),
        "copy".to_owned(),
        "-bsf:a".to_owned(),
        "aac_adtstoasc".to_owned(),
        output.to_string_lossy().into_owned(),
    ];
    run_to_end(program, &args, cancel).await
}

/// Message used when a stream needs ffmpeg and none is available.
pub(crate) const NEEDS_FFMPEG: &str = "This stream needs ffmpeg. Set its path in Settings → Video.";

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn configured_path_wins_and_must_exist() {
        let dir = tempfile::tempdir().expect("temp dir");
        let program = dir.path().join("my-ffmpeg.exe");
        std::fs::write(&program, b"x").expect("write");
        let text = program.to_string_lossy().into_owned();
        assert_eq!(find_in(&text, None, None), Some(program));
        let missing = dir.path().join("nope").to_string_lossy().into_owned();
        assert_eq!(find_in(&missing, None, None), None);
    }

    #[test]
    fn program_next_to_the_exe_is_used_before_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let exe = dir.path().join("downrust.exe");
        std::fs::write(&exe, b"x").expect("write exe");
        let beside = dir.path().join(program_name());
        std::fs::write(&beside, b"x").expect("write ffmpeg");
        assert_eq!(find_in("", Some(&exe), None), Some(beside));
    }

    #[test]
    fn path_is_searched_when_nothing_is_configured() {
        let dir = tempfile::tempdir().expect("temp dir");
        let found = dir.path().join(program_name());
        std::fs::write(&found, b"x").expect("write");
        let path_var: OsString =
            std::env::join_paths([PathBuf::from("/none"), dir.path().to_path_buf()])
                .expect("join paths");
        assert_eq!(find_in("  ", None, Some(&path_var)), Some(found));
        assert_eq!(find_in("", None, None), None);
    }

    #[test]
    fn missing_version_output_gives_none() {
        assert_eq!(
            ffmpeg_version(Path::new("/definitely/not/here/ffmpeg")),
            None
        );
    }
}
