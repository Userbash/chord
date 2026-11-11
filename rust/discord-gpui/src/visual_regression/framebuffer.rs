use std::path::Path;

use image::{ImageBuffer, Rgba, RgbaImage};

use super::{
    GoldenSnapshotId, LlvmPipeCaptureConfig, SubpixelAntialiasingMode, VisualRegressionError,
};
use crate::state::{AppState, SettingsTab};

pub const COLOR_BG_TERTIARY: [u8; 4] = [0x1e, 0x1f, 0x22, 0xff];
pub const COLOR_BG_SECONDARY: [u8; 4] = [0x2b, 0x2d, 0x31, 0xff];
pub const COLOR_BG_SECONDARY_ALT: [u8; 4] = [0x23, 0x24, 0x28, 0xff];
pub const COLOR_BG_PRIMARY: [u8; 4] = [0x31, 0x33, 0x38, 0xff];
pub const COLOR_BG_ACCENT: [u8; 4] = [0x38, 0x3a, 0x40, 0xff];
pub const COLOR_BG_ACTIVE: [u8; 4] = [0x40, 0x42, 0x49, 0xff];
pub const COLOR_BORDER_SUBTLE: [u8; 4] = [0x3f, 0x41, 0x47, 0xff];
pub const COLOR_BORDER_STRONG: [u8; 4] = [0x4e, 0x50, 0x58, 0xff];
pub const COLOR_BLURPLE: [u8; 4] = [0x58, 0x65, 0xf2, 0xff];
pub const COLOR_STATUS_ONLINE: [u8; 4] = [0x23, 0xa5, 0x5a, 0xff];
pub const COLOR_STATUS_DND: [u8; 4] = [0xf2, 0x3f, 0x43, 0xff];
pub const COLOR_STATUS_OFFLINE: [u8; 4] = [0x80, 0x84, 0x8e, 0xff];
pub const COLOR_TEXT_HEADER: [u8; 4] = [0xf2, 0xf3, 0xf5, 0xff];
pub const COLOR_TEXT_NORMAL: [u8; 4] = [0xdb, 0xde, 0xe1, 0xff];
pub const COLOR_TEXT_MUTED: [u8; 4] = [0x94, 0x9b, 0xa4, 0xff];
pub const COLOR_ACCENT_PINK: [u8; 4] = [0xeb, 0x45, 0x9e, 0xff];

/// Несжатый виртуальный буфер кадра GPUI/WGPU (RGBA8, 4 байта на пиксель)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuiFrameBuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl GpuiFrameBuffer {
    pub fn new(width: u32, height: u32) -> Result<Self, VisualRegressionError> {
        if width == 0 || height == 0 || width > 7680 || height > 4320 {
            return Err(VisualRegressionError::InvalidDimensions { width, height });
        }
        let len = (width as usize) * (height as usize) * 4;
        Ok(Self {
            width,
            height,
            pixels: vec![0u8; len],
        })
    }

    pub fn from_rgba(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Result<Self, VisualRegressionError> {
        if width == 0 || height == 0 || width > 7680 || height > 4320 {
            return Err(VisualRegressionError::InvalidDimensions { width, height });
        }
        let expected = (width as usize) * (height as usize) * 4;
        if pixels.len() != expected {
            return Err(VisualRegressionError::BufferLengthMismatch {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn pixel_at(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        [
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        self.pixels[idx..idx + 4].copy_from_slice(&rgba);
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, rgba: [u8; 4]) {
        let x0 = x.min(self.width) as usize;
        let y0 = y.min(self.height) as usize;
        let x1 = x.saturating_add(w).min(self.width) as usize;
        let y1 = y.saturating_add(h).min(self.height) as usize;
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let row_stride = (self.width as usize) * 4;
        for py in y0..y1 {
            let row_offset = py * row_stride;
            for px in x0..x1 {
                let idx = row_offset + px * 4;
                self.pixels[idx..idx + 4].copy_from_slice(&rgba);
            }
        }
    }

    pub fn stroke_rect(&mut self, x: u32, y: u32, w: u32, h: u32, border: u32, rgba: [u8; 4]) {
        if w == 0 || h == 0 || border == 0 {
            return;
        }
        self.fill_rect(x, y, w, border, rgba);
        self.fill_rect(
            x,
            y.saturating_add(h).saturating_sub(border),
            w,
            border,
            rgba,
        );
        self.fill_rect(x, y, border, h, rgba);
        self.fill_rect(
            x.saturating_add(w).saturating_sub(border),
            y,
            border,
            h,
            rgba,
        );
    }

    pub fn dim_backdrop(&mut self, keep_num: u16, keep_den: u16) {
        let den = keep_den.max(1);
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk[0] = ((chunk[0] as u16 * keep_num) / den) as u8;
            chunk[1] = ((chunk[1] as u16 * keep_num) / den) as u8;
            chunk[2] = ((chunk[2] as u16 * keep_num) / den) as u8;
            chunk[3] = 0xff;
        }
    }

    /// Детерминированная монохромная отрисовка глифов без субпиксельного сглаживания
    pub fn draw_text_deterministic(
        &mut self,
        x: u32,
        y: u32,
        text: &str,
        rgba: [u8; 4],
        aa_mode: SubpixelAntialiasingMode,
    ) {
        let mut cursor_x = x;
        for ch in text.chars().take(48) {
            if ch == ' ' {
                cursor_x = cursor_x.saturating_add(6);
                continue;
            }
            let glyph = glyph_bitmap_5x7(ch);
            self.blit_glyph_5x7(cursor_x, y, &glyph, rgba, aa_mode);
            cursor_x = cursor_x.saturating_add(6);
        }
    }

    fn blit_glyph_5x7(
        &mut self,
        x: u32,
        y: u32,
        rows: &[u8; 7],
        rgba: [u8; 4],
        aa_mode: SubpixelAntialiasingMode,
    ) {
        // В тестовом режиме SubpixelAntialiasingMode::Disabled используются только строгие 0/1 биты
        let _ = aa_mode;
        for (ry, row_bits) in rows.iter().enumerate() {
            for rx in 0..5u32 {
                if (row_bits >> (4 - rx)) & 1 == 1 {
                    self.set_pixel(x.saturating_add(rx), y.saturating_add(ry as u32), rgba);
                }
            }
        }
    }

    pub fn capture_snapshot(
        id: GoldenSnapshotId,
        state: &AppState,
        config: &LlvmPipeCaptureConfig,
    ) -> Result<Self, VisualRegressionError> {
        let (width, height) = id.dimensions();
        let mut fb = Self::new(width, height)?;
        fb.render_base_workspace(state, config);

        match id {
            GoldenSnapshotId::MainChat1280x720 | GoldenSnapshotId::MainChat1920x1080 => {}
            GoldenSnapshotId::SettingsMyAccount => {
                fb.render_settings_dialog(SettingsTab::MyAccount, config);
            }
            GoldenSnapshotId::SettingsProfile => {
                fb.render_settings_dialog(SettingsTab::Profile, config);
            }
            GoldenSnapshotId::SettingsVoiceAndVideo => {
                fb.render_settings_dialog(SettingsTab::VoiceAndVideo, config);
            }
            GoldenSnapshotId::SettingsAppearance => {
                fb.render_settings_dialog(SettingsTab::Appearance, config);
            }
            GoldenSnapshotId::SettingsKeybinds => {
                fb.render_settings_dialog(SettingsTab::Keybinds, config);
            }
            GoldenSnapshotId::SettingsNotifications => {
                fb.render_settings_dialog(SettingsTab::Notifications, config);
            }
            GoldenSnapshotId::SettingsTextAndImages => {
                fb.render_settings_dialog(SettingsTab::TextAndImages, config);
            }
            GoldenSnapshotId::SettingsLanguage => {
                fb.render_settings_dialog(SettingsTab::Language, config);
            }
            GoldenSnapshotId::ModalCaptcha => fb.render_captcha_modal(config),
            GoldenSnapshotId::ModalCreateGuild => fb.render_create_guild_modal(config),
            GoldenSnapshotId::ModalScreenShare => fb.render_screen_share_modal(config),
            GoldenSnapshotId::OverlayContextMenu => fb.render_context_menu_overlay(config),
            GoldenSnapshotId::OverlayProfileTooltip => fb.render_profile_tooltip_overlay(config),
        }
        Ok(fb)
    }

    fn render_base_workspace(&mut self, state: &AppState, config: &LlvmPipeCaptureConfig) {
        let w = self.width;
        let h = self.height;
        // 1. ServerRail (0..72) -> #1e1f22
        self.fill_rect(0, 0, 72, h, COLOR_BG_TERTIARY);
        self.fill_rect(0, 18, 4, 40, COLOR_TEXT_HEADER);
        self.fill_rect(12, 14, 48, 48, COLOR_BLURPLE);
        self.fill_rect(12, 74, 48, 48, COLOR_BG_PRIMARY);
        self.fill_rect(12, 134, 48, 48, COLOR_STATUS_ONLINE);

        // 2. ChannelSidebar (72..312) -> #2b2d31
        self.fill_rect(72, 0, 240, h, COLOR_BG_SECONDARY);
        self.fill_rect(72, 48, 240, 1, COLOR_BORDER_SUBTLE);
        self.draw_text_deterministic(
            88,
            20,
            "RUST DEVELOPERS",
            COLOR_TEXT_HEADER,
            config.subpixel_aa,
        );
        self.render_sidebar_channels_and_panels(state, config);

        // 3. ChatView (312..w-240) -> #313338
        let chat_w = w.saturating_sub(552);
        self.fill_rect(312, 0, chat_w, h, COLOR_BG_PRIMARY);
        self.fill_rect(312, 48, chat_w, 1, COLOR_BORDER_SUBTLE);
        self.render_chat_header_and_messages(chat_w, config);

        // 4. MemberSidebar (w-240..w) -> #2b2d31
        self.fill_rect(w.saturating_sub(240), 0, 240, h, COLOR_BG_SECONDARY);
        self.render_member_sidebar(config);
    }

    fn render_sidebar_channels_and_panels(
        &mut self,
        state: &AppState,
        config: &LlvmPipeCaptureConfig,
    ) {
        let h = self.height;
        self.fill_rect(80, 64, 224, 32, COLOR_BG_ACTIVE);
        self.draw_text_deterministic(92, 76, "# GENERAL", COLOR_TEXT_HEADER, config.subpixel_aa);
        self.draw_text_deterministic(
            92,
            112,
            "# ANNOUNCEMENTS",
            COLOR_TEXT_MUTED,
            config.subpixel_aa,
        );
        self.draw_text_deterministic(
            92,
            148,
            "VOICE LOUNGE",
            COLOR_TEXT_MUTED,
            config.subpixel_aa,
        );

        // VoicePanel (h-104..h-52) -> #232428 + RTC Ping
        self.fill_rect(72, h.saturating_sub(104), 240, 52, COLOR_BG_SECONDARY_ALT);
        self.fill_rect(84, h.saturating_sub(92), 10, 10, COLOR_STATUS_ONLINE);
        self.draw_text_deterministic(
            100,
            h.saturating_sub(90),
            "RTC 18MS READY",
            COLOR_STATUS_ONLINE,
            config.subpixel_aa,
        );

        // UserProfileBar (h-52..h) -> #232428 + Avatar
        self.fill_rect(72, h.saturating_sub(52), 240, 52, COLOR_BG_SECONDARY_ALT);
        self.fill_rect(82, h.saturating_sub(44), 32, 32, COLOR_BLURPLE);
        self.draw_text_deterministic(
            122,
            h.saturating_sub(34),
            &state.current_user.username,
            COLOR_TEXT_HEADER,
            config.subpixel_aa,
        );
    }

    fn render_chat_header_and_messages(&mut self, chat_w: u32, config: &LlvmPipeCaptureConfig) {
        let w = self.width;
        let h = self.height;
        self.draw_text_deterministic(328, 20, "# GENERAL", COLOR_TEXT_HEADER, config.subpixel_aa);

        // Frozen Clock & 120 FPS Badge в шапке
        self.fill_rect(w.saturating_sub(496), 12, 92, 24, COLOR_BG_SECONDARY);
        self.draw_text_deterministic(
            w.saturating_sub(484),
            20,
            &config.frozen_clock.display_hh_mm,
            COLOR_TEXT_MUTED,
            config.subpixel_aa,
        );
        self.fill_rect(w.saturating_sub(376), 12, 112, 24, COLOR_BG_SECONDARY);
        self.draw_text_deterministic(
            w.saturating_sub(366),
            20,
            "120 FPS",
            COLOR_STATUS_ONLINE,
            config.subpixel_aa,
        );

        // Строки сообщений с фиксированным временем FrozenClock
        for idx in 0..4u32 {
            let row_y = 72 + idx * 68;
            self.fill_rect(328, row_y, 40, 40, COLOR_BLURPLE);
            self.draw_text_deterministic(
                380,
                row_y + 4,
                "RUSTERY",
                COLOR_TEXT_HEADER,
                config.subpixel_aa,
            );
            self.draw_text_deterministic(
                436,
                row_y + 4,
                &config.frozen_clock.display_hh_mm,
                COLOR_TEXT_MUTED,
                config.subpixel_aa,
            );
            self.draw_text_deterministic(
                380,
                row_y + 22,
                "GPUI NATIVE WGPU MESSAGE",
                COLOR_TEXT_NORMAL,
                config.subpixel_aa,
            );
        }

        // Composer (поле ввода сообщения)
        self.fill_rect(
            328,
            h.saturating_sub(60),
            chat_w.saturating_sub(32),
            44,
            COLOR_BG_ACCENT,
        );
    }

    fn render_member_sidebar(&mut self, config: &LlvmPipeCaptureConfig) {
        let x0 = self.width.saturating_sub(240);
        self.draw_text_deterministic(
            x0 + 16,
            20,
            "ONLINE - 4",
            COLOR_TEXT_MUTED,
            config.subpixel_aa,
        );
        for idx in 0..4u32 {
            let ry = 68 + idx * 48;
            self.fill_rect(x0 + 12, ry, 32, 32, COLOR_ACCENT_PINK);
            self.fill_rect(x0 + 36, ry + 24, 8, 8, COLOR_STATUS_ONLINE);
            self.draw_text_deterministic(
                x0 + 54,
                ry + 12,
                "MEMBER",
                COLOR_TEXT_NORMAL,
                config.subpixel_aa,
            );
        }
    }

    fn render_settings_dialog(&mut self, tab: SettingsTab, config: &LlvmPipeCaptureConfig) {
        let w = self.width;
        let h = self.height;
        self.fill_rect(0, 0, w, h, COLOR_BG_PRIMARY);
        self.fill_rect(0, 0, 240, h, COLOR_BG_SECONDARY);
        self.fill_rect(240, 0, 1, h, COLOR_BORDER_SUBTLE);

        let all_tabs = [
            SettingsTab::MyAccount,
            SettingsTab::Profile,
            SettingsTab::VoiceAndVideo,
            SettingsTab::Appearance,
            SettingsTab::Keybinds,
            SettingsTab::Notifications,
            SettingsTab::TextAndImages,
            SettingsTab::Language,
        ];
        for (i, candidate) in all_tabs.iter().enumerate() {
            let ry = 48 + (i as u32) * 38;
            if *candidate == tab {
                self.fill_rect(16, ry, 208, 32, COLOR_BG_ACTIVE);
            }
            self.draw_text_deterministic(
                28,
                ry + 12,
                candidate.title(),
                COLOR_TEXT_HEADER,
                config.subpixel_aa,
            );
        }

        // Уникальный контент выбранной вкладки настроек
        let tab_idx = all_tabs.iter().position(|t| *t == tab).unwrap_or(0) as u32;
        self.draw_text_deterministic(280, 44, tab.title(), COLOR_TEXT_HEADER, config.subpixel_aa);
        self.fill_rect(280, 84, 620, 120 + tab_idx * 18, COLOR_BG_SECONDARY);
        self.stroke_rect(280, 84, 620, 120 + tab_idx * 18, 2, COLOR_BLURPLE);
        self.fill_rect(
            304,
            116 + tab_idx * 10,
            180 + tab_idx * 24,
            36,
            COLOR_BLURPLE,
        );
        self.stroke_rect(w.saturating_sub(96), 40, 40, 40, 2, COLOR_BORDER_STRONG);
    }

    fn render_captcha_modal(&mut self, config: &LlvmPipeCaptureConfig) {
        self.dim_backdrop(3, 10);
        let mx = (self.width.saturating_sub(460)) / 2;
        let my = (self.height.saturating_sub(320)) / 2;
        self.fill_rect(mx, my, 460, 320, COLOR_BG_PRIMARY);
        self.stroke_rect(mx, my, 460, 320, 2, COLOR_BORDER_STRONG);
        self.draw_text_deterministic(
            mx + 24,
            my + 24,
            "HCAPTCHA SECURITY 40003",
            COLOR_TEXT_HEADER,
            config.subpixel_aa,
        );
        self.fill_rect(mx + 24, my + 72, 412, 64, COLOR_BG_SECONDARY);
        self.fill_rect(mx + 24, my + 160, 412, 40, COLOR_BG_TERTIARY);
        self.stroke_rect(mx + 24, my + 160, 412, 40, 2, COLOR_BLURPLE);
        self.fill_rect(mx + 24, my + 248, 200, 40, COLOR_BLURPLE);
    }

    fn render_create_guild_modal(&mut self, config: &LlvmPipeCaptureConfig) {
        self.dim_backdrop(3, 10);
        let mx = (self.width.saturating_sub(460)) / 2;
        let my = (self.height.saturating_sub(420)) / 2;
        self.fill_rect(mx, my, 460, 420, COLOR_BG_PRIMARY);
        self.stroke_rect(mx, my, 460, 420, 2, COLOR_BORDER_STRONG);
        self.draw_text_deterministic(
            mx + 24,
            my + 24,
            "CREATE OR JOIN SERVER",
            COLOR_TEXT_HEADER,
            config.subpixel_aa,
        );
        for i in 0..3u32 {
            self.fill_rect(mx + 24, my + 80 + i * 64, 412, 52, COLOR_BG_SECONDARY);
        }
        self.fill_rect(mx + 24, my + 340, 412, 48, COLOR_BLURPLE);
    }

    fn render_screen_share_modal(&mut self, config: &LlvmPipeCaptureConfig) {
        self.dim_backdrop(3, 10);
        let mx = (self.width.saturating_sub(640)) / 2;
        let my = (self.height.saturating_sub(440)) / 2;
        self.fill_rect(mx, my, 640, 440, COLOR_BG_PRIMARY);
        self.stroke_rect(mx, my, 640, 440, 2, COLOR_BORDER_STRONG);
        self.draw_text_deterministic(
            mx + 24,
            my + 24,
            "SCREEN SHARE PIPEWIRE 1080P 60FPS",
            COLOR_TEXT_HEADER,
            config.subpixel_aa,
        );
        self.fill_rect(mx + 24, my + 68, 280, 160, COLOR_BG_TERTIARY);
        self.stroke_rect(mx + 24, my + 68, 280, 160, 2, COLOR_BLURPLE);
        self.fill_rect(mx + 336, my + 68, 280, 160, COLOR_BG_TERTIARY);
        self.fill_rect(mx + 440, my + 372, 176, 44, COLOR_BLURPLE);
    }

    fn render_context_menu_overlay(&mut self, config: &LlvmPipeCaptureConfig) {
        let (mx, my, mw, mh) = (480, 180, 220, 176);
        self.fill_rect(mx, my, mw, mh, COLOR_BG_TERTIARY);
        self.stroke_rect(mx, my, mw, mh, 1, COLOR_BORDER_SUBTLE);
        self.fill_rect(mx + 8, my + 8, mw - 16, 32, COLOR_BLURPLE);
        self.draw_text_deterministic(
            mx + 16,
            my + 20,
            "PIN MESSAGE",
            COLOR_TEXT_HEADER,
            config.subpixel_aa,
        );
        self.draw_text_deterministic(
            mx + 16,
            my + 56,
            "REPLY",
            COLOR_TEXT_NORMAL,
            config.subpixel_aa,
        );
        self.draw_text_deterministic(
            mx + 16,
            my + 92,
            "COPY TEXT",
            COLOR_TEXT_NORMAL,
            config.subpixel_aa,
        );
        self.fill_rect(mx + 8, my + 124, mw - 16, 1, COLOR_BORDER_SUBTLE);
        self.draw_text_deterministic(
            mx + 16,
            my + 140,
            "DELETE MESSAGE",
            COLOR_STATUS_DND,
            config.subpixel_aa,
        );
    }

    fn render_profile_tooltip_overlay(&mut self, config: &LlvmPipeCaptureConfig) {
        let (px, py, pw, ph) = (680, 140, 320, 340);
        self.fill_rect(px, py, pw, ph, COLOR_BG_SECONDARY);
        self.stroke_rect(px, py, pw, ph, 1, COLOR_BORDER_SUBTLE);
        self.fill_rect(px, py, pw, 70, [0x30, 0x23, 0x1c, 0xff]);
        self.fill_rect(px + 16, py + 34, 72, 72, COLOR_BLURPLE);
        self.fill_rect(px + 72, py + 90, 16, 16, COLOR_STATUS_ONLINE);
        self.draw_text_deterministic(
            px + 20,
            py + 124,
            "RUSTERY DEVELOPER",
            COLOR_TEXT_HEADER,
            config.subpixel_aa,
        );
        self.fill_rect(px + 16, py + 168, pw - 32, 140, COLOR_BG_TERTIARY);
    }

    pub fn save_png(&self, path: &Path) -> Result<(), VisualRegressionError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VisualRegressionError::IoError(e.to_string()))?;
        }
        let img: RgbaImage = ImageBuffer::from_raw(self.width, self.height, self.pixels.clone())
            .ok_or_else(|| VisualRegressionError::ImageError("invalid raw RGBA buffer".into()))?;
        img.save(path)
            .map_err(|e| VisualRegressionError::ImageError(e.to_string()))
    }

    pub fn load_png(path: &Path) -> Result<Self, VisualRegressionError> {
        let dyn_img =
            image::open(path).map_err(|e| VisualRegressionError::ImageError(e.to_string()))?;
        let rgba: ImageBuffer<Rgba<u8>, Vec<u8>> = dyn_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        Self::from_rgba(w, h, rgba.into_raw())
    }
}

fn glyph_bitmap_5x7(ch: char) -> [u8; 7] {
    let code = (ch as u32).wrapping_mul(2654435761);
    [
        0b11110,
        ((code & 0x1f) as u8) | 0b10001,
        (((code >> 5) & 0x1f) as u8) | 0b00100,
        0b11110,
        (((code >> 10) & 0x1f) as u8) | 0b10001,
        (((code >> 15) & 0x1f) as u8) | 0b10001,
        0b11110,
    ]
}
