// SPDX-License-Identifier: GPL-3.0-or-later
//! Native messaging host for the browser extension. It answers `getConfig` with the bridge port
//! and token, read from the saved settings. It also writes the host manifests and registers them.
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use downrust_core::store::Store;
use serde_json::{json, Value};

use crate::{error::AppResult, paths::portable_root, settings::AppSettings};

pub const HOST_NAME: &str = "com.downrust.bridge";
const FIREFOX_EXTENSION_ID: &str = "downrust@downrust.local";
const CHROME_ORIGIN: &str = "chrome-extension://kfjljlbhjjfebfnamdcmchngepfjhkej/";
const APP_DIR_NAME: &str = "com.downrust.desktop";
const DB_FILE_NAME: &str = "downrust.sqlite3";
const MANIFEST_DIR: &str = "native-messaging";
const MAX_MESSAGE: usize = 1024 * 1024;

/// True when the browser started this executable as a native messaging host.
pub fn is_host_invocation<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().any(|arg| {
        let arg = arg.as_ref();
        arg.starts_with("chrome-extension://") || arg == FIREFOX_EXTENSION_ID
    })
}

/// Reads one message: a 4-byte little-endian length, then UTF-8 JSON. Returns `None` at the end
/// of the input.
pub fn read_message<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0u8; 4];
    match reader.read_exact(&mut length) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    }
    let size = u32::from_le_bytes(length) as usize;
    if size > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message is larger than 1 MB",
        ));
    }
    let mut body = vec![0u8; size];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Writes one message with its length prefix.
pub fn write_message<W: Write>(writer: &mut W, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value).map_err(io::Error::other)?;
    if body.len() > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reply is larger than 1 MB",
        ));
    }
    let length = u32::try_from(body.len()).map_err(io::Error::other)?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()
}

/// The reply to one request.
pub fn reply(request: &[u8], settings: &AppSettings, version: &str) -> Value {
    let Ok(parsed) = serde_json::from_slice::<Value>(request) else {
        return json!({"type": "error", "message": "invalid request"});
    };
    match parsed.get("type").and_then(Value::as_str) {
        Some("getConfig") => json!({
            "type": "config",
            "port": settings.browser_port,
            "token": settings.browser_token,
            "enabled": settings.browser_integration,
            "version": version,
        }),
        _ => json!({"type": "error", "message": "unknown request"}),
    }
}

/// Answers requests until the input closes. Settings are loaded for each request.
pub fn serve<R: Read, W: Write>(
    input: &mut R,
    output: &mut W,
    load: impl Fn() -> AppSettings,
    version: &str,
) -> io::Result<()> {
    while let Some(request) = read_message(input)? {
        let response = reply(&request, &load(), version);
        write_message(output, &response)?;
    }
    Ok(())
}

/// Runs the host on stdin and stdout.
pub fn run() -> io::Result<()> {
    let db_path = data_dir().join(DB_FILE_NAME);
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(
        &mut stdin.lock(),
        &mut stdout.lock(),
        move || load_settings(&db_path),
        env!("CARGO_PKG_VERSION"),
    )
}

/// The data folder of the app: next to the executable in portable mode, otherwise the roaming
/// app data folder.
pub fn data_dir() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    if let Some(root) = portable_root(&exe_dir, |path| path.is_file()) {
        return root;
    }
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(APP_DIR_NAME)
}

/// The saved settings, or the defaults when the database or the settings are missing.
fn load_settings(db_path: &Path) -> AppSettings {
    if !db_path.is_file() {
        return AppSettings::default();
    }
    let Ok(store) = Store::open(db_path) else {
        return AppSettings::default();
    };
    store
        .load_settings_json()
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<AppSettings>(&json).ok())
        .unwrap_or_default()
}

/// Writes the Firefox and Chrome manifests into `<data dir>\native-messaging`, rewriting a file
/// only when its content differs, and points the registry keys at them.
pub fn install(data_dir: &Path) -> AppResult<()> {
    let exe = std::env::current_exe()?;
    let exe_text = exe.to_string_lossy().into_owned();
    let dir = data_dir.join(MANIFEST_DIR);
    fs::create_dir_all(&dir)?;
    let firefox = dir.join("firefox.json");
    let chrome = dir.join("chrome.json");
    write_if_changed(
        &firefox,
        &json!({
            "name": HOST_NAME,
            "description": "DownRust",
            "path": exe_text,
            "type": "stdio",
            "allowed_extensions": [FIREFOX_EXTENSION_ID],
        }),
    )?;
    write_if_changed(
        &chrome,
        &json!({
            "name": HOST_NAME,
            "description": "DownRust",
            "path": exe_text,
            "type": "stdio",
            "allowed_origins": [CHROME_ORIGIN],
        }),
    )?;
    register(&firefox, &chrome)
}

fn write_if_changed(path: &Path, value: &Value) -> AppResult<()> {
    let text = serde_json::to_string_pretty(value)?;
    if fs::read_to_string(path).is_ok_and(|current| current == text) {
        return Ok(());
    }
    fs::write(path, text)?;
    Ok(())
}

/// Sets the default value of the host keys for Firefox, Chrome and Edge.
#[cfg(windows)]
fn register(firefox: &Path, chrome: &Path) -> AppResult<()> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let entries = [
        (
            format!(r"Software\Mozilla\NativeMessagingHosts\{HOST_NAME}"),
            firefox,
        ),
        (
            format!(r"Software\Google\Chrome\NativeMessagingHosts\{HOST_NAME}"),
            chrome,
        ),
        (
            format!(r"Software\Microsoft\Edge\NativeMessagingHosts\{HOST_NAME}"),
            chrome,
        ),
    ];
    for (key_path, manifest) in entries {
        let (key, _) = current_user.create_subkey(&key_path)?;
        let manifest_text: String = manifest.to_string_lossy().into_owned();
        key.set_value("", &manifest_text)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn register(_firefox: &Path, _chrome: &Path) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn frame(value: &Value) -> Vec<u8> {
        let mut out = Vec::new();
        write_message(&mut out, value).expect("frame");
        out
    }

    fn settings() -> AppSettings {
        AppSettings {
            browser_port: 7777,
            browser_token: "token-value".into(),
            browser_integration: true,
            ..AppSettings::default()
        }
    }

    #[test]
    fn frame_has_little_endian_length_prefix() {
        let bytes = frame(&json!({"a": 1}));
        let length = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        assert_eq!(length, bytes.len() - 4);
        assert_eq!(&bytes[4..], br#"{"a":1}"#);
    }

    #[test]
    fn messages_round_trip_and_end_at_eof() {
        let mut bytes = frame(&json!({"type": "getConfig"}));
        bytes.extend(frame(&json!({"type": "other"})));
        let mut input = Cursor::new(bytes);
        let first = read_message(&mut input).expect("first").expect("some");
        let second = read_message(&mut input).expect("second").expect("some");
        assert_eq!(first, br#"{"type":"getConfig"}"#);
        assert_eq!(second, br#"{"type":"other"}"#);
        assert!(read_message(&mut input).expect("eof").is_none());
    }

    #[test]
    fn oversized_length_is_rejected() {
        let mut input = Cursor::new(((MAX_MESSAGE + 1) as u32).to_le_bytes().to_vec());
        assert!(read_message(&mut input).is_err());
    }

    #[test]
    fn get_config_replies_with_port_token_and_state() {
        let reply = reply(br#"{"type":"getConfig"}"#, &settings(), "1.2.3");
        assert_eq!(reply["type"], "config");
        assert_eq!(reply["port"], 7777);
        assert_eq!(reply["token"], "token-value");
        assert_eq!(reply["enabled"], true);
        assert_eq!(reply["version"], "1.2.3");
    }

    #[test]
    fn unknown_or_broken_requests_get_an_error() {
        let unknown = reply(br#"{"type":"nope"}"#, &settings(), "1");
        assert_eq!(
            unknown,
            json!({"type": "error", "message": "unknown request"})
        );
        let broken = reply(b"not json", &settings(), "1");
        assert_eq!(broken["type"], "error");
    }

    #[test]
    fn serve_answers_each_request_then_stops() {
        let mut input_bytes = frame(&json!({"type": "getConfig"}));
        input_bytes.extend(frame(&json!({"type": "getConfig"})));
        let mut input = Cursor::new(input_bytes);
        let mut output = Vec::new();
        serve(&mut input, &mut output, settings, "1").expect("serve");
        let mut replies = Cursor::new(output);
        let first = read_message(&mut replies).expect("first").expect("some");
        let second = read_message(&mut replies).expect("second").expect("some");
        assert_eq!(
            serde_json::from_slice::<Value>(&first).expect("json")["type"],
            "config"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&second).expect("json")["type"],
            "config"
        );
    }

    #[test]
    fn host_is_detected_from_the_browser_arguments() {
        assert!(is_host_invocation(&[
            "chrome-extension://kfjljlbhjjfebfnamdcmchngepfjhkej/"
        ]));
        assert!(is_host_invocation(&["downrust@downrust.local"]));
        assert!(!is_host_invocation(&["--new", "https://example.com/a"]));
    }
}
