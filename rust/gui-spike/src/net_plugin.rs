//! Modular Tauri v2 plugin (`net`) encapsulating network and gateway connection commands.
//!
//! Provides `net_connect`, `net_status`, `net_dispatch_stats`, and `net_disconnect`
//! via `tauri::plugin::Builder::new("net")`.

use std::sync::Arc;

use chord_client::net::{DispatchStats, NetService};

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("блокирующая операция не завершилась: {error}"))
}

/// Connects to the Discord Gateway via [`NetService`] on a blocking worker thread.
pub async fn net_connect(net: tauri::State<'_, Arc<NetService>>) -> Result<String, String> {
    let net = Arc::clone(net.inner());
    blocking(move || {
        net.connect()
            .map(|status| status.to_string())
            .map_err(|error| error.to_string())
    })
    .await?
}

/// Returns the current gateway connection status as a string.
#[must_use]
pub fn net_status(net: tauri::State<'_, Arc<NetService>>) -> String {
    net.status().to_string()
}

/// Returns L-02 dispatch counters (received, forwarded to UI, dropped, and source detail).
#[allow(dead_code)]
#[must_use]
pub fn net_dispatch_stats(net: tauri::State<'_, Arc<NetService>>) -> DispatchStats {
    net.dispatch_stats()
}

/// Disconnects from the Discord Gateway via [`NetService`] on a blocking worker thread.
pub async fn net_disconnect(net: tauri::State<'_, Arc<NetService>>) -> Result<String, String> {
    let net = Arc::clone(net.inner());
    blocking(move || {
        net.disconnect()
            .map(|status| status.to_string())
            .map_err(|error| error.to_string())
    })
    .await?
}

/// Initializes the modular Tauri v2 `"net"` plugin.
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("net").build()
}
