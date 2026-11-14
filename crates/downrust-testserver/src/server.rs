// SPDX-License-Identifier: GPL-3.0-or-later
//! Server lifecycle: binds the socket, runs the router on its own thread and shuts it down on drop.

use std::net::{Ipv4Addr, SocketAddr, TcpListener as StdListener};
use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tokio::sync::oneshot;

use crate::error::{Error, Result};
use crate::handlers::{auth_file, file, named_file, redirect};
use crate::hls::hls;
use crate::state::ServerState;

/// A running test server bound to `127.0.0.1`. It stops when dropped.
///
/// The server runs on its own thread with its own runtime, so it keeps
/// serving while the caller's runtime is shut down or replaced.
#[derive(Debug)]
pub struct TestServer {
    /// Address the server listens on.
    pub addr: SocketAddr,
    /// Shared state: version, failure injection, concurrency limit and counters.
    pub state: Arc<ServerState>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl TestServer {
    /// Starts a server on an ephemeral port.
    pub fn start() -> Result<Self> {
        Self::start_on(0)
    }

    /// Starts a server on the given port. Port `0` picks a free port.
    pub fn start_on(port: u16) -> Result<Self> {
        let requested = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let listener = StdListener::bind(requested).map_err(|source| Error::Bind {
            addr: requested.to_string(),
            source,
        })?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let state = Arc::new(ServerState::default());
        let app = router(Arc::clone(&state));
        let (shutdown, stop) = oneshot::channel();
        std::thread::Builder::new()
            .name("downrust-testserver".to_owned())
            .spawn(move || serve(listener, app, stop))
            .map_err(Error::Spawn)?;
        Ok(Self {
            addr,
            state,
            shutdown: Some(shutdown),
        })
    }

    /// Full URL for a path, for example `url("/file/1000")`.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

/// Routes of the test server.
pub(crate) fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/file/{size}", get(file))
        .route("/auth/file/{size}", get(auth_file))
        .route("/redirect", get(redirect))
        .route("/hls/{*path}", get(hls))
        .route("/files/{*path}", get(named_file))
        .with_state(state)
}

/// Runs the server until `stop` fires. In-flight bodies finish first.
fn serve(listener: StdListener, app: Router, stop: oneshot::Receiver<()>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("downrust-testserver: cannot build runtime: {err}");
            return;
        }
    };
    runtime.block_on(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("downrust-testserver: cannot register listener: {err}");
                return;
            }
        };
        let graceful = async {
            let _ = stop.await;
        };
        if let Err(err) = axum::serve(listener, app)
            .with_graceful_shutdown(graceful)
            .await
        {
            eprintln!("downrust-testserver: server stopped with error: {err}");
        }
    });
}
