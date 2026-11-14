// SPDX-License-Identifier: GPL-3.0-or-later
//! Engine settings with their clamping rules and folder resolution.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::model::Category;

mod media;
mod network;
mod scan;
mod schedule;
mod site;
mod speed_schedule;
mod torrent;

pub use media::{MediaQuality, MediaSettings};
pub use network::{
    host_matches, route_for, sanitized_logins, valid_proxy_url, ProxyMode, ProxyRule,
    ProxyRuleAction, ProxySettings, Route, SiteLogin,
};
pub use scan::{ScanSettings, DEFAULT_SCAN_ARGS};
pub use schedule::{in_window, is_open_now, Schedule};
pub use site::SiteSettings;
pub use speed_schedule::SpeedSchedule;
pub use torrent::TorrentSettings;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
pub const MAX_CONNECTIONS: u8 = 32;
pub const MAX_CONCURRENT_DOWNLOADS: u32 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EngineSettings {
    pub max_concurrent_downloads: u32,   // default 3, clamp 1..=10
    pub default_connections: u8,         // default 8, clamp 1..=32
    pub global_speed_limit: Option<u64>, // None; Some(0) -> None; Some(x) -> max(x, 1024)
    pub min_segment_size: u64,           // 1 MiB, clamp 64 KiB..=64 MiB
    pub max_retries: u32,                // 10, clamp 0..=100
    pub retry_base_delay_ms: u64,        // 1000, clamp 10..=60000
    pub retry_max_delay_ms: u64,         // 30000, clamp base..=600000
    pub connect_timeout_secs: u64,       // 20, clamp 1..=120
    pub read_timeout_secs: u64,          // 30, clamp 2..=600
    pub checkpoint_interval_ms: u64,     // 3000, clamp 100..=60000
    pub progress_interval_ms: u64,       // 500, clamp 50..=5000
    pub user_agent: String,              // DEFAULT_USER_AGENT; empty -> default
    pub use_system_proxy: bool,          // true
    pub preallocate: bool,               // true
    pub resume_on_startup: bool,         // false
    pub default_dir: String,             // "" -> filled by the app
    pub auto_sort_by_category: bool,     // true
    pub category_dirs: BTreeMap<Category, String>,
    #[serde(default)]
    pub schedule: Schedule,
    #[serde(default)]
    pub proxy: ProxySettings,
    #[serde(default)]
    pub site_logins: Vec<SiteLogin>,
    #[serde(default)]
    pub scan: ScanSettings,
    #[serde(default)]
    pub media: MediaSettings,
    #[serde(default)]
    pub site: SiteSettings,
    #[serde(default)]
    pub torrent: TorrentSettings,
    #[serde(default)]
    pub speed_schedule: SpeedSchedule,
    /// Looks for a published SHA-256 next to a finished file and checks the file against it.
    #[serde(default)]
    pub auto_checksum: bool,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 3,
            default_connections: 8,
            global_speed_limit: None,
            min_segment_size: 1024 * 1024,
            max_retries: 10,
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 30_000,
            connect_timeout_secs: 20,
            read_timeout_secs: 30,
            checkpoint_interval_ms: 3000,
            progress_interval_ms: 500,
            user_agent: DEFAULT_USER_AGENT.to_owned(),
            use_system_proxy: true,
            preallocate: true,
            resume_on_startup: false,
            default_dir: String::new(),
            auto_sort_by_category: true,
            category_dirs: BTreeMap::new(),
            schedule: Schedule::default(),
            proxy: ProxySettings::default(),
            site_logins: Vec::new(),
            scan: ScanSettings::default(),
            media: MediaSettings::default(),
            site: SiteSettings::default(),
            torrent: TorrentSettings::default(),
            speed_schedule: SpeedSchedule::default(),
            auto_checksum: true,
        }
    }
}

impl EngineSettings {
    /// Applies every clamping rule of the settings.
    pub fn sanitized(mut self) -> Self {
        self.max_concurrent_downloads = self
            .max_concurrent_downloads
            .clamp(1, MAX_CONCURRENT_DOWNLOADS);
        self.default_connections = self.default_connections.clamp(1, MAX_CONNECTIONS);
        self.global_speed_limit = match self.global_speed_limit {
            None | Some(0) => None,
            Some(limit) => Some(limit.max(1024)),
        };
        self.min_segment_size = self.min_segment_size.clamp(64 * 1024, 64 * 1024 * 1024);
        self.max_retries = self.max_retries.clamp(0, 100);
        self.retry_base_delay_ms = self.retry_base_delay_ms.clamp(10, 60_000);
        self.retry_max_delay_ms = self
            .retry_max_delay_ms
            .clamp(self.retry_base_delay_ms, 600_000);
        self.connect_timeout_secs = self.connect_timeout_secs.clamp(1, 120);
        self.read_timeout_secs = self.read_timeout_secs.clamp(2, 600);
        self.checkpoint_interval_ms = self.checkpoint_interval_ms.clamp(100, 60_000);
        self.progress_interval_ms = self.progress_interval_ms.clamp(50, 5_000);
        if self.user_agent.is_empty() {
            self.user_agent = DEFAULT_USER_AGENT.to_owned();
        }
        self.schedule = self.schedule.sanitized();
        // A saved `useSystemProxy: false` from older versions means no proxy at all.
        if !self.use_system_proxy && self.proxy.mode == ProxyMode::System {
            self.proxy.mode = ProxyMode::Direct;
        }
        self.proxy = self.proxy.sanitized();
        self.use_system_proxy = self.proxy.mode == ProxyMode::System;
        self.site_logins = sanitized_logins(self.site_logins);
        self.scan = self.scan.sanitized();
        self.media = self.media.sanitized();
        self
    }

    /// Fills empty folder settings from the downloads root.
    /// An empty `default_dir` becomes the root. Each category with a folder name gets
    /// `root/<folder>` when its entry is missing or empty. `Other` gets the root when its entry is missing.
    pub fn fill_missing_dirs(&mut self, downloads_root: &Path) {
        let root = downloads_root.to_string_lossy().into_owned();
        if self.default_dir.is_empty() {
            self.default_dir = root.clone();
        }
        for category in Category::ALL {
            match category.folder_name() {
                Some(folder) => {
                    let entry = self.category_dirs.entry(category).or_default();
                    if entry.is_empty() {
                        *entry = downloads_root.join(folder).to_string_lossy().into_owned();
                    }
                }
                None => {
                    self.category_dirs
                        .entry(category)
                        .or_insert_with(|| root.clone());
                }
            }
        }
    }

    /// With auto sort on and a non-empty `category_dirs` entry, returns that folder.
    /// Otherwise returns `default_dir`.
    pub fn dir_for(&self, category: Category) -> PathBuf {
        if self.auto_sort_by_category {
            if let Some(dir) = self.category_dirs.get(&category) {
                if !dir.is_empty() {
                    return PathBuf::from(dir);
                }
            }
        }
        PathBuf::from(&self.default_dir)
    }
}
