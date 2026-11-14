// SPDX-License-Identifier: GPL-3.0-or-later
//! Virus scan of a finished file. An external program runs on the file, and its exit code
//! decides the result.
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use super::task::TaskOutcome;
use crate::model::{DownloadId, EngineEvent, ScanState};
use crate::settings::ScanSettings;
use crate::store::Store;

const SCAN_TIMEOUT: Duration = Duration::from_secs(600);
const THREAT_EXIT_CODE: i32 = 2;
const PLACEHOLDER: &str = "{file}";
/// Message of the failed download when the scan finds a threat.
pub(crate) const THREAT_MESSAGE: &str = "Windows Security found a threat in this file.";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// What the scan program reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScanVerdict {
    Clean,
    Threat,
    Failed(String),
}

/// Exit code 0 is clean, 2 is a threat, and anything else counts as a failed scan.
pub(crate) fn verdict_for_exit(code: Option<i32>) -> ScanVerdict {
    match code {
        Some(0) => ScanVerdict::Clean,
        Some(THREAT_EXIT_CODE) => ScanVerdict::Threat,
        Some(other) => ScanVerdict::Failed(format!("the scan program exited with code {other}")),
        None => ScanVerdict::Failed("the scan program was stopped by the system".to_owned()),
    }
}

/// Splits an argument text the way a command line does: spaces separate arguments and
/// double quotes group them. The quote characters are removed.
pub(crate) fn split_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    for ch in text.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    out.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        out.push(current);
    }
    out
}

/// The arguments with `{file}` replaced by the path of the finished file.
pub(crate) fn build_args(settings: &ScanSettings, file: &Path) -> Vec<String> {
    let file_text = file.to_string_lossy();
    split_args(&settings.args)
        .into_iter()
        .map(|arg| arg.replace(PLACEHOLDER, &file_text))
        .collect()
}

/// Runs the program without a console window and reports its verdict.
pub(crate) async fn run_program(program: &str, args: &[String]) -> ScanVerdict {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => return ScanVerdict::Failed(format!("cannot start the scan program: {err}")),
    };
    match tokio::time::timeout(SCAN_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => verdict_for_exit(status.code()),
        Ok(Err(err)) => ScanVerdict::Failed(format!("the scan program failed: {err}")),
        Err(_) => {
            let _ = child.kill().await;
            ScanVerdict::Failed("the scan program did not finish in ten minutes".to_owned())
        }
    }
}

/// Scans a completed download. Returns `Completed` unless the scan found a threat, which
/// returns `Failed` with the threat message. A scan that cannot run leaves the download completed.
pub(crate) async fn scan_completed(
    store: &Store,
    events: &UnboundedSender<EngineEvent>,
    id: DownloadId,
    settings: &ScanSettings,
) -> TaskOutcome {
    if !settings.enabled {
        return TaskOutcome::Completed;
    }
    let record = match store.get_download(id) {
        Ok(Some(record)) => record,
        _ => {
            tracing::warn!(id, "virus scan skipped: the download is gone");
            return TaskOutcome::Completed;
        }
    };
    set_state(store, events, id, ScanState::Scanning);
    let args = build_args(settings, &record.final_path());
    let verdict = run_program(&settings.resolved_program(), &args).await;
    match verdict {
        ScanVerdict::Clean => {
            set_state(store, events, id, ScanState::Clean);
            TaskOutcome::Completed
        }
        ScanVerdict::Threat => {
            set_state(store, events, id, ScanState::Threat);
            TaskOutcome::Failed(THREAT_MESSAGE.to_owned())
        }
        ScanVerdict::Failed(reason) => {
            tracing::warn!(id, "virus scan failed: {reason}");
            set_state(store, events, id, ScanState::Failed);
            TaskOutcome::Completed
        }
    }
}

fn set_state(
    store: &Store,
    events: &UnboundedSender<EngineEvent>,
    id: DownloadId,
    state: ScanState,
) {
    if let Err(err) = store.set_scan_state(id, Some(state)) {
        tracing::warn!(id, "cannot save the scan state: {err}");
    }
    if let Ok(Some(record)) = store.get_download(id) {
        let _ = events.send(EngineEvent::Updated(record));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_map_to_verdicts() {
        assert_eq!(verdict_for_exit(Some(0)), ScanVerdict::Clean);
        assert_eq!(verdict_for_exit(Some(2)), ScanVerdict::Threat);
        assert!(matches!(verdict_for_exit(Some(3)), ScanVerdict::Failed(_)));
        assert!(matches!(verdict_for_exit(None), ScanVerdict::Failed(_)));
    }

    #[test]
    fn quotes_group_arguments() {
        assert_eq!(
            split_args(r#"-Scan -File "{file}" -DisableRemediation"#),
            vec!["-Scan", "-File", "{file}", "-DisableRemediation"]
        );
        assert_eq!(split_args(r#"a "b c" d"#), vec!["a", "b c", "d"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn file_placeholder_is_replaced_inside_one_argument() {
        let settings = ScanSettings {
            args: r#"-File "{file}" -Flag"#.to_owned(),
            ..ScanSettings::default()
        };
        let args = build_args(&settings, Path::new(r"C:\Downloads\my file.zip"));
        assert_eq!(args, vec!["-File", r"C:\Downloads\my file.zip", "-Flag"]);
    }

    #[cfg(windows)]
    mod windows {
        use super::*;
        use crate::model::{Category, DownloadStatus, NewDownload};
        use tokio::sync::mpsc::unbounded_channel;

        fn store_with_row(dir: &Path) -> (Store, DownloadId) {
            let store = Store::open_in_memory().expect("store");
            let record = store
                .insert_download(&NewDownload {
                    url: "https://example.com/a.bin".to_owned(),
                    final_url: None,
                    referrer: None,
                    file_name: "a.bin".to_owned(),
                    save_dir: dir.display().to_string(),
                    category: Category::Other,
                    status: DownloadStatus::Completed,
                    total_size: Some(4),
                    resumable: false,
                    connections: 1,
                    speed_limit: None,
                    etag: None,
                    last_modified: None,
                    content_type: None,
                    headers: Vec::new(),
                    queue_position: 1,
                    created_at: 0,
                    expected_sha256: None,
                    kind: crate::model::DownloadKind::File,
                    mirrors: Vec::new(),
                    options: None,
                })
                .expect("row");
            (store, record.id)
        }

        fn cmd_settings(args: &str) -> ScanSettings {
            ScanSettings {
                enabled: true,
                program: "cmd.exe".to_owned(),
                args: args.to_owned(),
            }
        }

        #[tokio::test]
        async fn clean_exit_keeps_the_download_completed() {
            let dir = tempfile::tempdir().expect("dir");
            let (store, id) = store_with_row(dir.path());
            let (events, _rx) = unbounded_channel();
            let outcome = scan_completed(&store, &events, id, &cmd_settings("/c exit 0")).await;
            assert_eq!(outcome, TaskOutcome::Completed);
            let row = store.get_download(id).expect("query").expect("row");
            assert_eq!(row.scan, Some(ScanState::Clean));
        }

        #[tokio::test]
        async fn threat_exit_fails_the_download() {
            let dir = tempfile::tempdir().expect("dir");
            let (store, id) = store_with_row(dir.path());
            let (events, _rx) = unbounded_channel();
            let outcome = scan_completed(&store, &events, id, &cmd_settings("/c exit 2")).await;
            assert_eq!(outcome, TaskOutcome::Failed(THREAT_MESSAGE.to_owned()));
            let row = store.get_download(id).expect("query").expect("row");
            assert_eq!(row.scan, Some(ScanState::Threat));
        }

        #[tokio::test]
        async fn missing_program_leaves_the_download_completed() {
            let dir = tempfile::tempdir().expect("dir");
            let (store, id) = store_with_row(dir.path());
            let (events, _rx) = unbounded_channel();
            let settings = ScanSettings {
                enabled: true,
                program: "does-not-exist.exe".to_owned(),
                args: String::new(),
            };
            let outcome = scan_completed(&store, &events, id, &settings).await;
            assert_eq!(outcome, TaskOutcome::Completed);
            let row = store.get_download(id).expect("query").expect("row");
            assert_eq!(row.scan, Some(ScanState::Failed));
            assert_eq!(row.status, DownloadStatus::Completed);
        }

        #[tokio::test]
        async fn disabled_scan_does_nothing() {
            let dir = tempfile::tempdir().expect("dir");
            let (store, id) = store_with_row(dir.path());
            let (events, _rx) = unbounded_channel();
            let settings = ScanSettings {
                enabled: false,
                ..cmd_settings("/c exit 2")
            };
            let outcome = scan_completed(&store, &events, id, &settings).await;
            assert_eq!(outcome, TaskOutcome::Completed);
            let row = store.get_download(id).expect("query").expect("row");
            assert_eq!(row.scan, None);
        }
    }
}
