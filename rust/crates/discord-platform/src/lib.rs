//! # discord-platform
//!
//! Системная интеграция: трей, системные уведомления, буфер обмена, ярлыки.
//! Заменяет наблюдавшиеся IPC-каналы Electron-клиента (`DISCORD_NOTIFICATIONS_*` и др.).
//!
//! Спека: `docs/re/backend-ipc.md` (168 корневых каналов; мост DiscordNative — 35 записей).
//! План: P6 (UI/интеграция); тесты: T-SYS-*.

pub mod autostart;
pub mod dbus;
pub mod deep_link;
pub mod keyring;
pub mod notifications;
pub mod power;
pub mod screencast;
pub mod spellcheck;
pub mod tray;

#[allow(unsafe_code)]
mod ffi_hunspell;
#[allow(unsafe_code)]
mod ffi_secret;

/// Корневых IPC-каналов наследия Electron, наблюдавшихся в волне R7.
pub const LEGACY_ROOT_CHANNELS: usize = 168;

/// Объединённый whitelist renderer→main (send 30 ∪ invoke 14).
pub const LEGACY_IPC_WHITELIST_UNION: usize = 43;

/// Записей в мосте `window.DiscordNative` (expose + fields).
pub const LEGACY_BRIDGE_ENTRIES: usize = 35;

/// Каналов уведомлений (`NOTIFICATIONS_*`), которые заменяются нативно.
pub const NOTIFICATION_CHANNELS: usize = 7;

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_channel_counts_match_re() {
        assert_eq!(super::LEGACY_ROOT_CHANNELS, 168);
        assert_eq!(super::LEGACY_IPC_WHITELIST_UNION, 43);
        assert_eq!(super::LEGACY_BRIDGE_ENTRIES, 35);
        assert_eq!(super::NOTIFICATION_CHANNELS, 7);
    }
}

pub mod rpc_ipc;
