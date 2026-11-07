//! Перечисление доступных источников захвата (мониторы, окна) и выбор бэкенда окружения.
//!
//! Оракул: `/home/sanya/re-deep/backend-ipc.md` (`DISCORD_DESKTOP_CAPTURER_GET_SOURCES`,
//! `DiscordNative.desktopCapture`, фильтр `types: ["screen", "window"]`) и
//! `/home/sanya/re-deep/native-re.md` (`discord_voice.node` -> Wayland Portal / X11 fallback).

use crate::capture::{DesktopBackend, DesktopSource, DesktopSourceType};

/// Фильтр перечисления источников захвата (соответствует `types: ["screen", "window"]`
/// в `DISCORD_DESKTOP_CAPTURER_GET_SOURCES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFilter {
    /// Включать мониторы (`"screen"`).
    pub include_screens: bool,
    /// Включать отдельные окна (`"window"`).
    pub include_windows: bool,
}

impl SourceFilter {
    /// Включает все источники (и мониторы, и окна).
    #[must_use]
    pub const fn all() -> Self {
        Self {
            include_screens: true,
            include_windows: true,
        }
    }

    /// Включает только мониторы (`DesktopSourceType::Screen`).
    #[must_use]
    pub const fn screens_only() -> Self {
        Self {
            include_screens: true,
            include_windows: false,
        }
    }

    /// Включает только окна (`DesktopSourceType::Window`).
    #[must_use]
    pub const fn windows_only() -> Self {
        Self {
            include_screens: false,
            include_windows: true,
        }
    }

    /// Пустой фильтр (не возвращает ни одного источника).
    #[must_use]
    pub const fn none() -> Self {
        Self {
            include_screens: false,
            include_windows: false,
        }
    }

    /// Строит фильтр из среза типов источников [`DesktopSourceType`].
    #[must_use]
    pub fn from_types(types: &[DesktopSourceType]) -> Self {
        let mut filter = Self::none();
        for kind in types {
            match kind {
                DesktopSourceType::Screen => filter.include_screens = true,
                DesktopSourceType::Window => filter.include_windows = true,
            }
        }
        filter
    }

    /// Проверяет, проходит ли источник `source` через фильтр.
    #[must_use]
    pub const fn matches(self, source: &DesktopSource) -> bool {
        match source.source_type {
            DesktopSourceType::Screen => self.include_screens,
            DesktopSourceType::Window => self.include_windows,
        }
    }
}

impl Default for SourceFilter {
    fn default() -> Self {
        Self::all()
    }
}

/// Чистая функция выбора бэкенда захвата экрана по значениям переменных окружения.
///
/// Приоритет совпадает с `discord_voice.node`:
/// 1. Если задан непустой `WAYLAND_DISPLAY` или `XDG_SESSION_TYPE == "wayland"` -> [`DesktopBackend::PipeWirePortal`].
/// 2. Иначе если задан непустой `DISPLAY` или `XDG_SESSION_TYPE == "x11"` -> [`DesktopBackend::X11Shm`].
/// 3. Иначе -> [`DesktopBackend::SyntheticFallback`].
#[must_use]
pub fn detect_backend_from_env(
    xdg_session_type: Option<&str>,
    wayland_display: Option<&str>,
    x11_display: Option<&str>,
) -> DesktopBackend {
    let session = xdg_session_type.map(str::trim).unwrap_or("");
    let wayland = wayland_display.map(str::trim).unwrap_or("");
    let x11 = x11_display.map(str::trim).unwrap_or("");

    if !wayland.is_empty() || session.eq_ignore_ascii_case("wayland") {
        DesktopBackend::PipeWirePortal
    } else if !x11.is_empty() || session.eq_ignore_ascii_case("x11") {
        DesktopBackend::X11Shm
    } else {
        DesktopBackend::SyntheticFallback
    }
}

/// Определяет активный бэкенд захвата из переменных окружения текущего процесса.
#[must_use]
pub fn detect_backend_live() -> DesktopBackend {
    let session = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();
    let x11 = std::env::var("DISPLAY").ok();
    detect_backend_from_env(session.as_deref(), wayland.as_deref(), x11.as_deref())
}

/// Базовый набор источников захвата для заданного бэкенда (когда список окон/мониторов не переопределён явно).
#[must_use]
pub fn default_sources_for_backend(backend: DesktopBackend) -> Vec<DesktopSource> {
    let backend_label = match backend {
        DesktopBackend::PipeWirePortal => "PipeWire / Wayland Portal",
        DesktopBackend::X11Shm => "X11 SHM",
        DesktopBackend::SyntheticFallback => "Виртуальный дисплей",
    };

    vec![
        DesktopSource {
            id: "screen:0".to_string(),
            name: format!("Экран 1 ({backend_label})"),
            source_type: DesktopSourceType::Screen,
            is_primary: true,
        },
        DesktopSource {
            id: "window:discord-lab".to_string(),
            name: "Discord Lab — Главное окно".to_string(),
            source_type: DesktopSourceType::Window,
            is_primary: false,
        },
    ]
}
