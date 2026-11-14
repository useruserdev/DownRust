// SPDX-License-Identifier: GPL-3.0-or-later
//! The one librqbit session shared by all torrent downloads of the process.
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use librqbit::{DhtSessionConfig, ListenerOptions, Session, SessionOptions};
use tokio::sync::OnceCell;

use crate::error::{Error, Result};
use crate::settings::TorrentSettings;

static SESSION: OnceCell<Arc<Session>> = OnceCell::const_new();

/// Returns the shared session, starting it on the first call. The settings of the first call
/// stay in effect until the app restarts.
pub(crate) async fn shared(
    settings: &TorrentSettings,
    default_dir: PathBuf,
) -> Result<Arc<Session>> {
    SESSION
        .get_or_try_init(|| async {
            Session::new_with_opts(default_dir, session_options(settings))
                .await
                .map_err(|err| Error::Transfer(format!("torrent engine does not start: {err}")))
        })
        .await
        .cloned()
}

/// Session options from the torrent settings. Persistence and fast resume stay off.
pub(crate) fn session_options(settings: &TorrentSettings) -> SessionOptions {
    let dht = settings.enable_dht.then_some(DhtSessionConfig {
        bootstrap_addrs: None,
        port: None,
        persistence: None,
    });
    SessionOptions {
        dht,
        fastresume: false,
        persistence: None,
        listen: Some(ListenerOptions {
            listen_addr: SocketAddr::from((Ipv4Addr::UNSPECIFIED, settings.listen_port)),
            enable_upnp_port_forwarding: settings.enable_upnp,
            ipv4_only: true,
            ..ListenerOptions::default()
        }),
        ..SessionOptions::default()
    }
}
