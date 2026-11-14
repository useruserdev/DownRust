// SPDX-License-Identifier: GPL-3.0-or-later
//! Command line arguments and `downrust://` links: parsing, the actions they start and the
//! magnet link registration.
use downrust_core::model::AddRequest;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;

use crate::{
    error::{AppError, AppResult},
    events::{ExternalAddPayload, OpenAddPayload, EXTERNAL_ADD_REQUEST, OPEN_ADD_DIALOG},
    state::AppState,
    window,
};

const ADDRESS_PREFIXES: [&str; 6] = [
    "http://", "https://", "ftp://", "ftps://", "sftp://", "magnet:",
];
const DEEP_LINK_SCHEME: &str = "downrust";
const MAGNET_SCHEME: &str = "magnet";

/// One address to add. `silent` adds it at once; otherwise the Add dialog opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddArgs {
    pub url: String,
    pub name: Option<String>,
    pub dir: Option<String>,
    pub paused: bool,
    pub silent: bool,
}

/// Everything the arguments ask for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Command {
    pub new_download: bool,
    pub pause_all: bool,
    pub resume_all: bool,
    pub adds: Vec<AddArgs>,
}

impl Command {
    pub fn is_empty(&self) -> bool {
        !self.new_download && !self.pause_all && !self.resume_all && self.adds.is_empty()
    }
}

/// Reads the arguments that follow the program name. `is_file` tells whether a path exists;
/// it is used only for `.torrent` files. Unknown flags and `--minimized` are ignored.
pub fn parse<S: AsRef<str>>(args: &[S], is_file: impl Fn(&str) -> bool) -> Command {
    let mut command = Command::default();
    let mut index = 0;
    while index < args.len() {
        let token = args[index].as_ref();
        index += 1;
        match token {
            "--new" => command.new_download = true,
            "--pause-all" => command.pause_all = true,
            "--resume-all" => command.resume_all = true,
            "add" => {
                if let Some(add) = parse_add(args, &mut index) {
                    command.adds.push(add);
                }
            }
            other => {
                if let Some(add) = parse_bare(other, &is_file).or_else(|| parse_link(other)) {
                    command.adds.push(add);
                }
            }
        }
    }
    command
}

/// Reads the options of `add` up to the next command word.
fn parse_add<S: AsRef<str>>(args: &[S], index: &mut usize) -> Option<AddArgs> {
    let mut add = AddArgs {
        url: String::new(),
        name: None,
        dir: None,
        paused: false,
        silent: false,
    };
    while let Some(token) = args.get(*index).map(AsRef::as_ref) {
        if matches!(token, "add" | "--new" | "--pause-all" | "--resume-all") {
            break;
        }
        *index += 1;
        match token {
            "--name" => add.name = next_value(args, index),
            "--dir" => add.dir = next_value(args, index),
            "--paused" => add.paused = true,
            "--silent" => add.silent = true,
            flag if flag.starts_with("--") => {}
            address => {
                if add.url.is_empty() {
                    add.url = address.to_string();
                }
            }
        }
    }
    (!add.url.is_empty()).then_some(add)
}

fn next_value<S: AsRef<str>>(args: &[S], index: &mut usize) -> Option<String> {
    let value = args.get(*index)?.as_ref().to_string();
    *index += 1;
    Some(value)
}

/// A bare address, or a `.torrent` file that exists.
fn parse_bare(token: &str, is_file: &impl Fn(&str) -> bool) -> Option<AddArgs> {
    let lower = token.to_ascii_lowercase();
    let is_address = ADDRESS_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix));
    let is_torrent = lower.ends_with(".torrent") && is_file(token);
    (is_address || is_torrent).then(|| AddArgs {
        url: token.to_string(),
        name: None,
        dir: None,
        paused: false,
        silent: false,
    })
}

/// A `downrust://add?url=...&name=...` link.
fn parse_link(token: &str) -> Option<AddArgs> {
    let link = url::Url::parse(token).ok()?;
    if link.scheme() != DEEP_LINK_SCHEME || link.host_str() != Some("add") {
        return None;
    }
    let mut target = None;
    let mut name = None;
    for (key, value) in link.query_pairs() {
        match key.as_ref() {
            "url" => target = Some(value.into_owned()),
            "name" => name = Some(value.into_owned()),
            _ => {}
        }
    }
    let url = target.filter(|value| !value.is_empty())?;
    Some(AddArgs {
        url,
        name,
        dir: None,
        paused: false,
        silent: false,
    })
}

/// Runs the parsed command: shows the window, opens the Add dialog, pauses or resumes all
/// downloads, or adds the addresses.
pub fn apply(app: &AppHandle, command: Command) {
    if command.is_empty() {
        window::show_main(app);
        return;
    }
    if command.new_download {
        window::show_main(app);
        if let Err(err) = app.emit(OPEN_ADD_DIALOG, OpenAddPayload { url: None }) {
            tracing::warn!("open add dialog event failed: {err}");
        }
    }
    if command.pause_all || command.resume_all {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let manager = app.state::<AppState>().manager.clone();
            if command.pause_all {
                if let Err(err) = manager.pause_all().await {
                    tracing::warn!("pause all from the command line failed: {err}");
                }
            }
            if command.resume_all {
                if let Err(err) = manager.resume_all().await {
                    tracing::warn!("resume all from the command line failed: {err}");
                }
            }
        });
    }
    for add in command.adds {
        if add.silent {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let manager = app.state::<AppState>().manager.clone();
                let request = AddRequest {
                    url: add.url,
                    file_name: add.name,
                    save_dir: add.dir,
                    connections: None,
                    speed_limit: None,
                    start: !add.paused,
                    referrer: None,
                    headers: Vec::new(),
                    probe: None,
                    expected_sha256: None,
                    kind: None,
                    mirrors: Vec::new(),
                    options: None,
                };
                if let Err(err) = manager.add(request).await {
                    tracing::warn!("command line add failed: {err}");
                }
            });
        } else {
            window::show_main(app);
            let payload = ExternalAddPayload {
                url: add.url,
                file_name: add.name,
                referrer: None,
                headers: Vec::new(),
                kind: None,
            };
            if let Err(err) = app.emit(EXTERNAL_ADD_REQUEST, payload) {
                tracing::warn!("external add event failed: {err}");
            }
        }
    }
}

/// Registers the `downrust` scheme and, when the setting is on, the `magnet` scheme.
pub fn register_schemes(app: &AppHandle, handle_magnet_links: bool) {
    let links = app.deep_link();
    if let Err(err) = links.register_all() {
        tracing::warn!("deep link registration failed: {err}");
    }
    if handle_magnet_links {
        if let Err(err) = links.register(MAGNET_SCHEME) {
            tracing::warn!("magnet link registration failed: {err}");
        }
    }
}

/// Turns the magnet link handler on or off for DownRust.
#[tauri::command]
pub fn set_magnet_handler(app: AppHandle, enabled: bool) -> AppResult<()> {
    let links = app.deep_link();
    if enabled {
        links.register(MAGNET_SCHEME).map_err(AppError::message)?;
    } else if let Err(err) = links.unregister(MAGNET_SCHEME) {
        tracing::warn!("magnet link removal failed: {err}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_files(_: &str) -> bool {
        false
    }

    fn parsed(args: &[&str]) -> Command {
        parse(args, no_files)
    }

    #[test]
    fn flags_set_their_actions() {
        let command = parsed(&["--new", "--pause-all", "--resume-all", "--minimized"]);
        assert!(command.new_download);
        assert!(command.pause_all);
        assert!(command.resume_all);
        assert!(command.adds.is_empty());
    }

    #[test]
    fn unknown_flags_are_ignored() {
        let command = parsed(&["--whatever", "--minimized"]);
        assert!(command.is_empty());
    }

    #[test]
    fn add_command_reads_every_option() {
        let command = parsed(&[
            "add",
            "https://example.com/a.zip",
            "--name",
            "b.zip",
            "--dir",
            "C:/Files",
            "--paused",
            "--silent",
        ]);
        assert_eq!(
            command.adds,
            vec![AddArgs {
                url: "https://example.com/a.zip".into(),
                name: Some("b.zip".into()),
                dir: Some("C:/Files".into()),
                paused: true,
                silent: true,
            }]
        );
    }

    #[test]
    fn add_command_without_address_adds_nothing() {
        assert!(parsed(&["add", "--silent"]).adds.is_empty());
    }

    #[test]
    fn add_options_stop_at_the_next_command_word() {
        let command = parsed(&["add", "ftp://host/file", "--new"]);
        assert!(command.new_download);
        assert_eq!(command.adds[0].url, "ftp://host/file");
        assert!(!command.adds[0].silent);
    }

    #[test]
    fn bare_addresses_open_the_dialog() {
        for address in [
            "https://example.com/f.iso",
            "http://example.com/f.iso",
            "ftps://host/file",
            "sftp://host/file",
            "magnet:?xt=urn:btih:abc",
        ] {
            let command = parsed(&[address]);
            assert_eq!(command.adds.len(), 1, "{address}");
            assert!(!command.adds[0].silent);
            assert_eq!(command.adds[0].url, address);
        }
    }

    #[test]
    fn torrent_file_only_counts_when_it_exists() {
        let path = "C:/Downloads/movie.torrent";
        assert!(parse(&[path], no_files).adds.is_empty());
        let command = parse(&[path], |candidate| candidate == path);
        assert_eq!(command.adds[0].url, path);
    }

    #[test]
    fn deep_link_reads_the_percent_encoded_url_and_name() {
        let command =
            parsed(&["downrust://add?url=https%3A%2F%2Fa.example%2Fc.zip%3Fx%3D1&name=d.zip"]);
        assert_eq!(command.adds.len(), 1);
        assert_eq!(command.adds[0].url, "https://a.example/c.zip?x=1");
        assert_eq!(command.adds[0].name.as_deref(), Some("d.zip"));
    }

    #[test]
    fn deep_link_without_url_or_with_other_action_is_ignored() {
        assert!(parsed(&["downrust://add?name=x"]).adds.is_empty());
        assert!(parsed(&["downrust://open?url=https%3A%2F%2Fa"])
            .adds
            .is_empty());
    }

    #[test]
    fn several_adds_keep_their_order() {
        let command = parsed(&[
            "https://a.example/1",
            "add",
            "https://a.example/2",
            "--silent",
        ]);
        let urls: Vec<&str> = command.adds.iter().map(|add| add.url.as_str()).collect();
        assert_eq!(urls, ["https://a.example/1", "https://a.example/2"]);
        assert!(command.adds[1].silent);
    }
}
