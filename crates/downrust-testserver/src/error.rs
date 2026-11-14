// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the test server crate.

use std::io;

/// Errors returned by the test server and its command line entry point.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The listening socket could not be bound.
    #[error("failed to bind {addr}: {source}")]
    Bind {
        /// Address that was requested.
        addr: String,
        /// Underlying socket error.
        #[source]
        source: io::Error,
    },
    /// The thread that runs the server could not be started.
    #[error("failed to start the server thread: {0}")]
    Spawn(#[source] io::Error),
    /// A socket or other input/output operation failed.
    #[error("input/output error: {0}")]
    Io(#[from] io::Error),
    /// The command line arguments are not valid.
    #[error("{0}")]
    Usage(String),
}

/// Result alias used across the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
