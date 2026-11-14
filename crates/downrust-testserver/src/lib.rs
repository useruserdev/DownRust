// SPDX-License-Identifier: GPL-3.0-or-later
//! Local HTTP server used by the DownRust tests.
//!
//! Start a server with [`TestServer::start`], build request paths with
//! [`file_url`], and read or change its behaviour through [`ServerState`].
//! Expected bytes come from [`full_content`] or [`fill`].

mod body;
mod content;
mod error;
mod handlers;
mod hls;
mod range;
mod server;
mod state;

pub use content::{etag, fill, full_content, last_modified};
pub use error::{Error, Result};
pub use handlers::{file_url, FileOpts, DEMO_AUTHORIZATION};
pub use hls::{
    enc_segment_plain, hls_variant_bytes, HLS_BYTE_RANGE_BYTES, HLS_ENC_SEGMENTS, HLS_KEY,
    HLS_SEGMENTS,
};
pub use server::TestServer;
pub use state::{ServerState, Stats};
