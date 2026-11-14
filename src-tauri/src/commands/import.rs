// SPDX-License-Identifier: GPL-3.0-or-later
//! Command that reads download addresses from a list file, such as an IDM export or a web page.
use downrust_core::util::import_list::{decode_text, parse_download_list};

use crate::error::{AppError, AppResult};

/// Largest list file that is read.
const MAX_LIST_BYTES: u64 = 20 * 1024 * 1024;

#[tauri::command]
pub fn import_download_list(path: String) -> AppResult<Vec<String>> {
    let size = std::fs::metadata(&path)?.len();
    if size > MAX_LIST_BYTES {
        return Err(AppError::message("The list file is larger than 20 MB."));
    }
    let bytes = std::fs::read(&path)?;
    Ok(parse_download_list(&decode_text(&bytes)))
}
