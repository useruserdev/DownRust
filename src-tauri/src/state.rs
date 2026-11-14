// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared application state managed by Tauri.
use std::sync::{Mutex, RwLock};

use downrust_core::{manager::DownloadManager, store::Store};

use crate::{
    after_queue::AfterQueue, bridge::BridgeHandle, paths::AppPaths, settings::AppSettings,
};

pub struct AppState {
    pub manager: DownloadManager,
    pub store: Store,
    pub settings: RwLock<AppSettings>,
    pub paths: AppPaths,
    pub start_hidden: bool,
    pub bridge: Mutex<Option<BridgeHandle>>,
    pub after_queue: Mutex<AfterQueue>,
}
