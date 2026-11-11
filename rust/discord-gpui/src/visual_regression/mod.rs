#![allow(dead_code)]

pub mod framebuffer;
pub mod masks;
pub mod report;
pub mod ssim;

use serde::{Deserialize, Serialize};

use crate::state::SettingsTab;

pub use framebuffer::GpuiFrameBuffer;
pub use masks::{DynamicMaskSet, MaskCategory, MaskRegion};
pub use report::{SnapshotAuditEntry, VisualDiffReport};
pub use ssim::{SsimComparator, SsimResult};

/// Ошибки подсистемы визуальной регрессии GPUI (SSIM + FrameBuffer + JSON Masks)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualRegressionError {
    InvalidDimensions {
        width: u32,
        height: u32,
    },
    BufferLengthMismatch {
        expected: usize,
        actual: usize,
    },
    DimensionMismatch {
        expected: (u32, u32),
        actual: (u32, u32),
    },
    MaskOutOfBounds {
        mask_id: String,
        rect: (u32, u32, u32, u32),
        frame: (u32, u32),
    },
    JsonError(String),
    ImageError(String),
    IoError(String),
}

impl std::fmt::Display for VisualRegressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for VisualRegressionError {}

/// Режим субпиксельного сглаживания шрифтов (принудительно отключается в тестах)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubpixelAntialiasingMode {
    Disabled,
    Standard,
}

/// Зафиксированные системные часы (Frozen Clock) для детерминированного рендеринга
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenClock {
    pub iso_timestamp: String,
    pub display_hh_mm: String,
    pub unix_epoch_secs: u64,
}

impl FrozenClock {
    pub fn deterministic_default() -> Self {
        Self {
            iso_timestamp: "2026-11-11T12:00:00Z".into(),
            display_hh_mm: "12:00".into(),
            unix_epoch_secs: 1_794_398_400,
        }
    }
}

/// Конфигурация программного рендерера `llvmpipe` и детерминированного захвата FrameBuffer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlvmPipeCaptureConfig {
    pub software_gl: bool,
    pub wgpu_backend: String,
    pub mesa_driver: String,
    pub subpixel_aa: SubpixelAntialiasingMode,
    pub frozen_clock: FrozenClock,
}

impl LlvmPipeCaptureConfig {
    pub fn deterministic() -> Self {
        Self {
            software_gl: true,
            wgpu_backend: "gl".into(),
            mesa_driver: "llvmpipe".into(),
            subpixel_aa: SubpixelAntialiasingMode::Disabled,
            frozen_clock: FrozenClock::deterministic_default(),
        }
    }

    pub fn env_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "LIBGL_ALWAYS_SOFTWARE",
                if self.software_gl { "1" } else { "0" }.into(),
            ),
            ("WGPU_BACKEND", self.wgpu_backend.clone()),
            ("MESA_LOADER_DRIVER_OVERRIDE", self.mesa_driver.clone()),
            (
                "GPUI_SUBPIXEL_AA",
                match self.subpixel_aa {
                    SubpixelAntialiasingMode::Disabled => "0".into(),
                    SubpixelAntialiasingMode::Standard => "1".into(),
                },
            ),
            (
                "DISCORD_GPUI_FROZEN_CLOCK",
                self.frozen_clock.iso_timestamp.clone(),
            ),
        ]
    }
}

/// Идентификаторы 15 базовых золотых эталонов (Golden Snapshots)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoldenSnapshotId {
    MainChat1280x720,
    MainChat1920x1080,
    SettingsMyAccount,
    SettingsProfile,
    SettingsVoiceAndVideo,
    SettingsAppearance,
    SettingsKeybinds,
    SettingsNotifications,
    SettingsTextAndImages,
    SettingsLanguage,
    ModalCaptcha,
    ModalCreateGuild,
    ModalScreenShare,
    OverlayContextMenu,
    OverlayProfileTooltip,
}

impl GoldenSnapshotId {
    pub const ALL: [Self; 15] = [
        Self::MainChat1280x720,
        Self::MainChat1920x1080,
        Self::SettingsMyAccount,
        Self::SettingsProfile,
        Self::SettingsVoiceAndVideo,
        Self::SettingsAppearance,
        Self::SettingsKeybinds,
        Self::SettingsNotifications,
        Self::SettingsTextAndImages,
        Self::SettingsLanguage,
        Self::ModalCaptcha,
        Self::ModalCreateGuild,
        Self::ModalScreenShare,
        Self::OverlayContextMenu,
        Self::OverlayProfileTooltip,
    ];

    pub fn slug(&self) -> &'static str {
        match self {
            Self::MainChat1280x720 => "main_chat_1280x720",
            Self::MainChat1920x1080 => "main_chat_1920x1080",
            Self::SettingsMyAccount => "settings_01_my_account",
            Self::SettingsProfile => "settings_02_profile",
            Self::SettingsVoiceAndVideo => "settings_03_voice_and_video",
            Self::SettingsAppearance => "settings_04_appearance",
            Self::SettingsKeybinds => "settings_05_keybinds",
            Self::SettingsNotifications => "settings_06_notifications",
            Self::SettingsTextAndImages => "settings_07_text_and_images",
            Self::SettingsLanguage => "settings_08_language",
            Self::ModalCaptcha => "modal_captcha",
            Self::ModalCreateGuild => "modal_create_guild",
            Self::ModalScreenShare => "modal_screen_share",
            Self::OverlayContextMenu => "overlay_context_menu",
            Self::OverlayProfileTooltip => "overlay_profile_tooltip",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::MainChat1280x720 => "Главное окно чата Discord Dark (1280x720 HD)",
            Self::MainChat1920x1080 => "Главное окно чата Discord Dark (1920x1080 FHD)",
            Self::SettingsMyAccount => "SettingsDialog — Моя учётная запись",
            Self::SettingsProfile => "SettingsDialog — Профиль пользователя",
            Self::SettingsVoiceAndVideo => "SettingsDialog — Голос и видео (CPAL)",
            Self::SettingsAppearance => "SettingsDialog — Внешний вид",
            Self::SettingsKeybinds => "SettingsDialog — Горячие клавиши",
            Self::SettingsNotifications => "SettingsDialog — Уведомления",
            Self::SettingsTextAndImages => "SettingsDialog — Текст и изображения",
            Self::SettingsLanguage => "SettingsDialog — Язык интерфейса",
            Self::ModalCaptcha => "CaptchaModal (hCaptcha 40003)",
            Self::ModalCreateGuild => "CreateGuildModal (Создание и вступление на сервер)",
            Self::ModalScreenShare => "ScreenShareModal (Демонстрация экрана PipeWire)",
            Self::OverlayContextMenu => "ContextMenu (Контекстное меню ПКМ)",
            Self::OverlayProfileTooltip => "UserProfileCard (Всплывающий тултип профиля)",
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::MainChat1920x1080 => (1920, 1080),
            _ => (1280, 720),
        }
    }

    pub fn for_settings_tab(tab: SettingsTab) -> Self {
        match tab {
            SettingsTab::MyAccount => Self::SettingsMyAccount,
            SettingsTab::Profile => Self::SettingsProfile,
            SettingsTab::VoiceAndVideo => Self::SettingsVoiceAndVideo,
            SettingsTab::Appearance => Self::SettingsAppearance,
            SettingsTab::Keybinds => Self::SettingsKeybinds,
            SettingsTab::Notifications => Self::SettingsNotifications,
            SettingsTab::TextAndImages => Self::SettingsTextAndImages,
            SettingsTab::Language => Self::SettingsLanguage,
        }
    }
}
