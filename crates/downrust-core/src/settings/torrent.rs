// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings for torrent downloads.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TorrentSettings {
    /// Port for incoming peers. 0 lets the system pick a free port.
    pub listen_port: u16,
    pub enable_dht: bool,
    pub enable_upnp: bool,
    /// Keep sharing a finished torrent until the app exits.
    pub seed_after_finish: bool,
    /// Upload limit in bytes per second. None means unlimited.
    pub upload_limit: Option<u64>,
}

impl Default for TorrentSettings {
    fn default() -> Self {
        Self {
            listen_port: 0,
            enable_dht: true,
            enable_upnp: true,
            seed_after_finish: false,
            upload_limit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_set() {
        let settings = TorrentSettings::default();
        assert_eq!(settings.listen_port, 0);
        assert!(settings.enable_dht);
        assert!(settings.enable_upnp);
        assert!(!settings.seed_after_finish);
        assert_eq!(settings.upload_limit, None);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let settings: TorrentSettings =
            serde_json::from_str(r#"{"listenPort":6881}"#).expect("settings parse");
        assert_eq!(settings.listen_port, 6881);
        assert!(settings.enable_dht);
        assert_eq!(settings.upload_limit, None);
    }
}
