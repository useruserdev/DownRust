// SPDX-License-Identifier: GPL-3.0-or-later
//! Resolves and creates the folders the application uses.
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

/// File names that switch the application to portable mode when placed next to the executable.
const PORTABLE_MARKERS: [&str; 2] = ["portable", "DownRust.portable"];

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub db_path: PathBuf,
    pub downloads_dir: PathBuf,
    pub portable: bool,
}

impl AppPaths {
    pub fn resolve(app: &AppHandle) -> AppResult<Self> {
        let resolver = app.path();
        let (data_dir, log_dir, portable) =
            match portable_root(&executable_dir()?, |path| path.is_file()) {
                Some(root) => {
                    let logs = root.join("logs");
                    (root, logs, true)
                }
                None => (resolver.app_data_dir()?, resolver.app_log_dir()?, false),
            };
        let downloads_dir = match resolver.download_dir() {
            Ok(dir) => dir,
            Err(_) => resolver.home_dir()?.join("Downloads"),
        }
        .join("DownRust");
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&log_dir)?;
        std::fs::create_dir_all(&downloads_dir)?;
        let db_path = data_dir.join("downrust.sqlite3");
        Ok(Self {
            data_dir,
            log_dir,
            db_path,
            downloads_dir,
            portable,
        })
    }
}

/// Folder that holds the running executable.
fn executable_dir() -> AppResult<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::message("the executable has no folder"))
}

/// Returns the data folder for portable mode when one of the marker files exists next to the
/// executable. `marker_exists` tells whether a path is an existing file.
pub fn portable_root(exe_dir: &Path, marker_exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    PORTABLE_MARKERS
        .iter()
        .any(|name| marker_exists(&exe_dir.join(name)))
        .then(|| exe_dir.join("data"))
}

/// Converts a path to text for the frontend.
pub fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    fn named(marker: &'static str) -> impl Fn(&Path) -> bool {
        move |path: &Path| path.file_name() == Some(OsStr::new(marker))
    }

    #[test]
    fn portable_file_enables_portable_mode() {
        let exe = Path::new("C:/Apps/DownRust");
        assert_eq!(
            portable_root(exe, named("portable")),
            Some(exe.join("data"))
        );
    }

    #[test]
    fn named_marker_enables_portable_mode() {
        let exe = Path::new("C:/Apps/DownRust");
        assert_eq!(
            portable_root(exe, named("DownRust.portable")),
            Some(exe.join("data"))
        );
    }

    #[test]
    fn no_marker_keeps_the_installed_layout() {
        let exe = Path::new("C:/Apps/DownRust");
        assert_eq!(portable_root(exe, |_| false), None);
    }

    #[test]
    fn similar_names_do_not_enable_portable_mode() {
        let exe = Path::new("C:/Apps/DownRust");
        assert_eq!(portable_root(exe, named("portable.txt")), None);
    }
}
