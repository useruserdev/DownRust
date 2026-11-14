// SPDX-License-Identifier: GPL-3.0-or-later
//! Local HTTP bridge that browser extensions use to hand downloads to DownRust.
pub mod auth;
pub mod routes;

use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

use crate::{error::AppResult, state::AppState};

const STOP_TIMEOUT: Duration = Duration::from_secs(3);

/// Owns a running bridge server. Dropping it without calling `stop` leaves the server running.
pub struct BridgeHandle {
    shutdown: oneshot::Sender<()>,
    done: oneshot::Receiver<()>,
}

impl BridgeHandle {
    /// Asks the server to stop and waits until its listener is closed, so the port can be bound again.
    pub async fn stop(self) {
        let _ = self.shutdown.send(());
        if tokio::time::timeout(STOP_TIMEOUT, self.done).await.is_err() {
            tracing::warn!("browser bridge did not stop in time");
        }
    }
}

/// Binds `127.0.0.1:port` and serves the bridge routes until stopped.
pub async fn start(app: AppHandle, port: u16) -> AppResult<BridgeHandle> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let (shutdown, shutdown_rx) = oneshot::channel::<()>();
    let (done_tx, done) = oneshot::channel::<()>();
    let router = routes::router(app);
    tauri::async_runtime::spawn(async move {
        let server = axum::serve(listener, router).with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });
        if let Err(err) = server.await {
            tracing::warn!("browser bridge stopped with error: {err}");
        }
        let _ = done_tx.send(());
    });
    tracing::info!("browser bridge listening on 127.0.0.1:{port}");
    Ok(BridgeHandle { shutdown, done })
}

/// Makes the running bridge match the settings: stops any running server, then starts one on
/// `port` when `enabled` is true.
pub async fn apply(app: &AppHandle, enabled: bool, port: u16) -> AppResult<()> {
    let state = app.state::<AppState>();
    let previous = state.bridge.lock().expect("bridge lock poisoned").take();
    if let Some(handle) = previous {
        handle.stop().await;
    }
    if !enabled {
        return Ok(());
    }
    let handle = start(app.clone(), port).await?;
    *state.bridge.lock().expect("bridge lock poisoned") = Some(handle);
    Ok(())
}

/// True while a bridge server is running.
pub fn is_running(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .bridge
        .lock()
        .expect("bridge lock poisoned")
        .is_some()
}
