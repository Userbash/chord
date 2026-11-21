//! Modular Tauri v2 plugin (`settings`) encapsulating local UI settings IPC commands.
//!
//! Provides `settings_load` and `settings_save` via `tauri::plugin::Builder::new("settings")`.

use chord_client::settings_store::{SettingsError, SettingsStore, UiSettings};

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("блокирующая операция не завершилась: {error}"))
}

/// Loads UI settings from the local store, returning defaults if the file is absent.
///
/// Runs file I/O on a blocking worker thread so the UI and async runtime remain unblocked.
pub async fn settings_load() -> Result<UiSettings, SettingsError> {
    blocking(|| SettingsStore::from_env().load())
        .await
        .map_err(|message| SettingsError::invalid("internal", message))?
}

/// Validates and atomically persists UI settings to the local store (max 256 KiB).
///
/// Runs file I/O on a blocking worker thread via `tmp + rename`.
pub async fn settings_save(settings: UiSettings) -> Result<(), SettingsError> {
    blocking(move || SettingsStore::from_env().save(&settings))
        .await
        .map_err(|message| SettingsError::invalid("internal", message))?
}

/// Initializes the modular Tauri v2 `"settings"` plugin.
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("settings").build()
}
