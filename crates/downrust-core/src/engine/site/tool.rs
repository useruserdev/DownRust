// SPDX-License-Identifier: GPL-3.0-or-later
//! Finds, downloads and updates the yt-dlp program.
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use super::error_message;
use crate::engine::media::ffmpeg::hidden_std_command;
use crate::error::{Error, Result};
use crate::settings::EngineSettings;

const TOOL_FILE: &str = "yt-dlp.exe";
const DOWNLOAD_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const CHECK_MARKER: &str = "yt-dlp.checked";
const UPDATE_INTERVAL: Duration = Duration::from_secs(7 * 24 * 3600);
const NOT_FOUND: &str = "yt-dlp was not found. Set its path in Settings → Video.";

/// The yt-dlp program in use: the configured path, then the tools folder, then the folder of
/// DownRust, then PATH. `None` when none of them exists.
pub fn find_tool(settings: &EngineSettings) -> Option<PathBuf> {
    find_in(
        &settings.site.ytdlp_path,
        &settings.site.tools_dir,
        std::env::current_exe().ok().as_deref(),
        std::env::var_os("PATH").as_deref(),
    )
}

/// The lookup order of `find_tool`, with the environment passed in for testing.
fn find_in(
    configured: &str,
    tools_dir: &str,
    exe: Option<&Path>,
    path_var: Option<&OsStr>,
) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        return path.is_file().then_some(path);
    }
    let tools_dir = tools_dir.trim();
    if !tools_dir.is_empty() {
        let candidate = Path::new(tools_dir).join(TOOL_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(dir) = exe.and_then(Path::parent) {
        let candidate = dir.join(TOOL_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let paths = path_var?;
    std::env::split_paths(paths)
        .map(|dir| dir.join(TOOL_FILE))
        .find(|candidate| candidate.is_file())
}

/// First line of `yt-dlp --version`, or `None` when the program does not run.
pub fn tool_version(path: &Path) -> Option<String> {
    let output = hidden_std_command(path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
}

/// Returns the yt-dlp program, downloading it when none is found and `auto_download` is on.
/// When `auto_update` is on and the last check is a week old, runs `yt-dlp -U`. A failed
/// update is ignored.
pub async fn ensure_tool(settings: &EngineSettings, client: &reqwest::Client) -> Result<PathBuf> {
    let path = resolve_tool(settings, client).await?;
    if settings.site.auto_update {
        if let Some(marker) = checked_marker(settings) {
            if update_due(&marker) {
                if let Err(err) = run_update(&path).await {
                    tracing::warn!("yt-dlp update did not run: {err}");
                }
                write_marker(&marker);
            }
        }
    }
    Ok(path)
}

/// Returns the yt-dlp program after a self update with `yt-dlp -U`.
pub async fn update_tool(settings: &EngineSettings, client: &reqwest::Client) -> Result<PathBuf> {
    let path = resolve_tool(settings, client).await?;
    run_update(&path).await?;
    if let Some(marker) = checked_marker(settings) {
        write_marker(&marker);
    }
    Ok(path)
}

async fn resolve_tool(settings: &EngineSettings, client: &reqwest::Client) -> Result<PathBuf> {
    if let Some(path) = find_tool(settings) {
        return Ok(path);
    }
    if settings.site.auto_download {
        return download_tool(settings, client).await;
    }
    Err(Error::Transfer(NOT_FOUND.to_owned()))
}

/// Downloads the program into the tools folder and marks it as checked for updates.
async fn download_tool(settings: &EngineSettings, client: &reqwest::Client) -> Result<PathBuf> {
    let dir = settings.site.tools_dir.trim();
    if dir.is_empty() {
        return Err(Error::Transfer(NOT_FOUND.to_owned()));
    }
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(TOOL_FILE);
    let part = dir.join(format!("{TOOL_FILE}.part"));
    let mut response = client.get(DOWNLOAD_URL).send().await?.error_for_status()?;
    {
        let mut file = std::fs::File::create(&part)?;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk)?;
        }
        file.flush()?;
    }
    std::fs::rename(&part, &target)?;
    if let Some(marker) = checked_marker(settings) {
        write_marker(&marker);
    }
    Ok(target)
}

/// Runs `yt-dlp -U`. A failure gives the last line of the error output.
async fn run_update(path: &Path) -> Result<()> {
    let output = tokio::process::Command::from(hidden_std_command(path))
        .arg("-U")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::Transfer(format!(
        "yt-dlp update failed: {}",
        error_message(&stderr)
    )))
}

/// The file that records the last update check. `None` without a tools folder.
fn checked_marker(settings: &EngineSettings) -> Option<PathBuf> {
    let dir = settings.site.tools_dir.trim();
    (!dir.is_empty()).then(|| Path::new(dir).join(CHECK_MARKER))
}

/// True when the marker is missing or older than the update interval.
fn update_due(marker: &Path) -> bool {
    match std::fs::metadata(marker).and_then(|meta| meta.modified()) {
        Ok(modified) => SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age >= UPDATE_INTERVAL),
        Err(_) => true,
    }
}

fn write_marker(marker: &Path) {
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(err) = std::fs::write(marker, b"") {
        tracing::warn!("cannot write the update check marker: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_path_wins_and_must_exist() {
        let dir = tempfile::tempdir().expect("temp dir");
        let program = dir.path().join("my-yt-dlp.exe");
        std::fs::write(&program, b"x").expect("write");
        let text = program.to_string_lossy().into_owned();
        assert_eq!(find_in(&text, "", None, None), Some(program));
        let missing = dir.path().join("nope").to_string_lossy().into_owned();
        assert_eq!(find_in(&missing, "", None, None), None);
    }

    #[test]
    fn tools_folder_comes_before_the_program_folder_and_path() {
        let tools = tempfile::tempdir().expect("temp dir");
        let beside = tempfile::tempdir().expect("temp dir");
        let on_path = tempfile::tempdir().expect("temp dir");
        let exe = beside.path().join("downrust.exe");
        std::fs::write(&exe, b"x").expect("write exe");
        std::fs::write(beside.path().join(TOOL_FILE), b"x").expect("write");
        std::fs::write(on_path.path().join(TOOL_FILE), b"x").expect("write");
        let in_tools = tools.path().join(TOOL_FILE);
        std::fs::write(&in_tools, b"x").expect("write");
        let tools_text = tools.path().to_string_lossy().into_owned();
        let path_var = std::env::join_paths([on_path.path().to_path_buf()]).expect("join");
        assert_eq!(
            find_in("", &tools_text, Some(&exe), Some(&path_var)),
            Some(in_tools)
        );
        assert_eq!(
            find_in("", "", Some(&exe), Some(&path_var)),
            Some(beside.path().join(TOOL_FILE))
        );
        assert_eq!(
            find_in("", "", None, Some(&path_var)),
            Some(on_path.path().join(TOOL_FILE))
        );
        assert_eq!(find_in("  ", "", None, None), None);
    }

    #[test]
    fn update_is_due_when_marker_is_missing_or_old() {
        let dir = tempfile::tempdir().expect("temp dir");
        let marker = dir.path().join(CHECK_MARKER);
        assert!(update_due(&marker));
        std::fs::write(&marker, b"").expect("write");
        assert!(!update_due(&marker));
        let old = SystemTime::now() - Duration::from_secs(8 * 24 * 3600);
        let file = std::fs::File::options()
            .write(true)
            .open(&marker)
            .expect("open");
        file.set_modified(old).expect("set time");
        assert!(update_due(&marker));
    }
}
