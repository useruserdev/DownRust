// SPDX-License-Identifier: GPL-3.0-or-later
//! Download engine for DownRust: probing, segmented transfers, persistence and scheduling.
pub(crate) mod auth;
pub(crate) mod engine;
pub mod error;
pub mod http;
pub mod manager;
pub mod media {
    //! ffmpeg lookup for HLS and DASH downloads.
    pub use crate::engine::media::ffmpeg::{detect_ffmpeg, FfmpegInfo};
}
pub mod site {
    //! yt-dlp helpers for video sites.
    pub use crate::engine::site::{
        ensure_tool, find_tool, site_info, tool_version, update_tool, SiteInfo,
    };
}
pub mod model;
pub mod secrets;
pub mod settings;
pub mod store;
pub mod util;

pub use error::{Error, Result};
