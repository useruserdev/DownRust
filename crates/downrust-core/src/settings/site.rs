// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings for video sites downloaded with yt-dlp.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SiteSettings {}
