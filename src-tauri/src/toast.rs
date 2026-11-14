// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows toast notifications through one notifier that lives as long as the app.
//! Creating a new notifier for every toast leaves a few system handles behind each time,
//! so the notifier is created once and reused.
use std::path::Path;
use std::sync::OnceLock;

use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier};

/// The PowerShell app id, used when the program runs from the build folder: such a build has no
/// Start menu shortcut, and Windows only shows toasts for registered app ids.
const UNREGISTERED_APP_ID: &str =
    r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

static NOTIFIER: OnceLock<Option<ToastNotifier>> = OnceLock::new();

/// Shows a toast with a title and a body. `identifier` is the app identifier from the Tauri config.
pub fn show(identifier: &str, title: &str, body: &str) -> windows::core::Result<()> {
    let Some(notifier) = NOTIFIER.get_or_init(|| {
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id(identifier)))
            .map_err(|err| tracing::warn!("toast notifier unavailable: {err}"))
            .ok()
    }) else {
        return Ok(());
    };
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(toast_xml(title, body)))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    notifier.Show(&toast)
}

fn app_id(identifier: &str) -> String {
    let from_build_folder = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .is_some_and(|dir| dir.ends_with("target/debug") || dir.ends_with("target/release"));
    if from_build_folder {
        UNREGISTERED_APP_ID.to_owned()
    } else {
        identifier.to_owned()
    }
}

fn toast_xml(title: &str, body: &str) -> String {
    format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        escape(title),
        escape(body)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escapes_file_names() {
        assert_eq!(
            toast_xml("Done", "a<b>&\"c\".zip"),
            r#"<toast><visual><binding template="ToastGeneric"><text>Done</text><text>a&lt;b&gt;&amp;&quot;c&quot;.zip</text></binding></visual></toast>"#
        );
    }
}
