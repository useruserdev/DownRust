// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings for torrent downloads.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TorrentSettings {}
