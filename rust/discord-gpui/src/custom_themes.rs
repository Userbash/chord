//! Пользовательские темы оформления и динамическая CSS палитра (`TASK-985`).
//! Поддерживает 4 встроенные темы Discord (`Dark`, `Light`, `Midnight`, `Amoled`) и до 8
//! пользовательских тем, горячее переключение палитры без перезагрузки интерфейса,
//! валидацию контрастности по стандарту WCAG 2.2 AA/AAA и санитизацию CSS-токенов.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub const THEME_PICKER_CONTAINER: &str = "Theme Picker";
pub const THEME_PICKER_DIMENSIONS: &str = "100%";
pub const THEME_PICKER_PADDING: &str = "16px";
pub const THEME_PICKER_SPACING: &str = "12px";

pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const WCAG_AA_NORMAL_MIN_RATIO: f64 = 4.5;
pub const WCAG_AAA_NORMAL_MIN_RATIO: f64 = 7.0;
pub const WCAG_AA_NON_TEXT_MIN_RATIO: f64 = 3.0;
pub const MAX_INSTALLED_CUSTOM_THEMES: usize = 8;
pub const EMERGENCY_RESET_SHORTCUT: &str = "Ctrl+Shift+F12";

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, PartialEq)]
pub enum CustomThemeError {
    UnknownTheme(String),
    InvalidCssPropertyName(String),
    ForbiddenCssConstruct(String),
    InvalidColorFormat(String),
    WcagContrastViolation(Vec<String>),
    MaxCustomThemesExceeded(usize),
    HttpSyncError {
        status: u16,
        retry_after_ms: Option<u64>,
    },
    AccessDenied(&'static str),
    BoundaryViolation(&'static str),
}

impl fmt::Display for CustomThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CustomThemeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinThemeKind {
    Dark,
    Light,
    Midnight,
    Amoled,
}

impl BuiltinThemeKind {
    #[must_use]
    pub const fn as_id(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Midnight => "midnight",
            Self::Amoled => "amoled",
        }
    }

    #[must_use]
    pub const fn color_scheme(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark | Self::Midnight | Self::Amoled => "dark",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreviewState {
    DefaultDark,
    HighlightTheme,
    Applied,
    SkeletonLoading,
}

impl ThemePreviewState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefaultDark => "dark",
            Self::HighlightTheme => "highlight theme",
            Self::Applied => "applied",
            Self::SkeletonLoading => "skeleton",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemePalette {
    pub id: String,
    pub label: String,
    pub color_scheme: String,
    pub tokens: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WcagAuditReport {
    pub passed: bool,
    pub audited_pairs: usize,
    pub failed_pairs: Vec<String>,
    pub min_text_ratio: f64,
    pub min_non_text_ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSwitchResult {
    pub active_theme_id: String,
    pub preview_state: ThemePreviewState,
    pub reloaded: bool,
    pub injected_css: String,
}

fn parse_hex_byte(pair: &str) -> Option<u8> {
    u8::from_str_radix(pair, 16).ok()
}

pub fn parse_css_color(raw: &str) -> Result<RgbaColor, CustomThemeError> {
    let trimmed = raw.trim().to_ascii_lowercase();
    let Some(hex) = trimmed.strip_prefix('#') else {
        return Err(CustomThemeError::InvalidColorFormat(raw.to_string()));
    };
    if hex.len() != 6 && hex.len() != 8 {
        return Err(CustomThemeError::InvalidColorFormat(raw.to_string()));
    }
    let r = parse_hex_byte(&hex[0..2])
        .ok_or_else(|| CustomThemeError::InvalidColorFormat(raw.to_string()))?;
    let g = parse_hex_byte(&hex[2..4])
        .ok_or_else(|| CustomThemeError::InvalidColorFormat(raw.to_string()))?;
    let b = parse_hex_byte(&hex[4..6])
        .ok_or_else(|| CustomThemeError::InvalidColorFormat(raw.to_string()))?;
    let a = if hex.len() == 8 {
        let a_u8 = parse_hex_byte(&hex[6..8])
            .ok_or_else(|| CustomThemeError::InvalidColorFormat(raw.to_string()))?;
        f64::from(a_u8) / 255.0
    } else {
        1.0
    };
    Ok(RgbaColor { r, g, b, a })
}

#[must_use]
pub fn srgb_channel_luminance(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[must_use]
pub fn relative_luminance(color: RgbaColor) -> f64 {
    0.2126 * srgb_channel_luminance(color.r)
        + 0.7152 * srgb_channel_luminance(color.g)
        + 0.0722 * srgb_channel_luminance(color.b)
}

#[must_use]
pub fn contrast_ratio(fg: RgbaColor, bg: RgbaColor) -> f64 {
    let alpha = fg.a.clamp(0.0, 1.0);
    let comp = RgbaColor {
        r: (f64::from(fg.r) * alpha + f64::from(bg.r) * (1.0 - alpha)).round() as u8,
        g: (f64::from(fg.g) * alpha + f64::from(bg.g) * (1.0 - alpha)).round() as u8,
        b: (f64::from(fg.b) * alpha + f64::from(bg.b) * (1.0 - alpha)).round() as u8,
        a: 1.0,
    };
    let l1 = relative_luminance(comp);
    let l2 = relative_luminance(bg);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

pub fn sanitize_css_token_entry(
    name: &str,
    value: &str,
) -> Result<(String, String), CustomThemeError> {
    let valid_name = name.starts_with("--")
        && name.len() > 2
        && name[2..]
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid_name {
        return Err(CustomThemeError::InvalidCssPropertyName(name.to_string()));
    }
    let lower = value.trim().to_ascii_lowercase();
    if lower.contains("url(") {
        return Err(CustomThemeError::ForbiddenCssConstruct(value.to_string()));
    }
    if lower.contains("@import")
        || lower.contains("@font-face")
        || lower.contains("expression(")
        || lower.contains("javascript:")
        || lower.contains("!important")
    {
        return Err(CustomThemeError::ForbiddenCssConstruct(value.to_string()));
    }
    let _ = parse_css_color(&lower)?;
    Ok((name.to_string(), lower))
}

fn build_palette_map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (k, v) in pairs {
        map.insert((*k).to_string(), (*v).to_string());
    }
    map
}

fn dark_base_tokens() -> BTreeMap<String, String> {
    build_palette_map(&[
        ("--bg-app", "#121214"),
        ("--bg-chat", "#1a1a1e"),
        ("--surface-1", "#202024"),
        ("--surface-2", "#242429"),
        ("--text-strong", "#fbfbfb"),
        ("--text", "#efeff1"),
        ("--text-subtle", "#abacb2"),
        ("--text-muted", "#96979e"),
        ("--text-link", "#4d96ee"),
        ("--text-accent", "#a5b0ff"),
        ("--danger-text", "#ff9a9e"),
        ("--positive-text", "#57d98a"),
        ("--text-on-accent", "#ffffff"),
        ("--accent", "#5865f2"),
        ("--focus-ring", "#a5b0ff"),
        ("--status-online", "#3d9e60"),
    ])
}

fn light_base_tokens() -> BTreeMap<String, String> {
    build_palette_map(&[
        ("--bg-app", "#f7f7f8"),
        ("--bg-chat", "#ffffff"),
        ("--surface-1", "#efeff1"),
        ("--surface-2", "#f3f3f4"),
        ("--text-strong", "#111214"),
        ("--text", "#232428"),
        ("--text-subtle", "#4e5058"),
        ("--text-muted", "#5c5e66"),
        ("--text-link", "#0058b0"),
        ("--text-accent", "#3c45a5"),
        ("--danger-text", "#a1282f"),
        ("--positive-text", "#136f3e"),
        ("--text-on-accent", "#ffffff"),
        ("--accent", "#5865f2"),
        ("--focus-ring", "#4654c0"),
        ("--status-online", "#237c48"),
    ])
}

#[must_use]
pub fn builtin_palette(kind: BuiltinThemeKind) -> ThemePalette {
    let mut tokens = match kind {
        BuiltinThemeKind::Light => light_base_tokens(),
        BuiltinThemeKind::Dark | BuiltinThemeKind::Midnight | BuiltinThemeKind::Amoled => {
            dark_base_tokens()
        }
    };
    if kind == BuiltinThemeKind::Midnight {
        for (k, v) in [
            ("--bg-app", "#0b0d14"),
            ("--bg-chat", "#111420"),
            ("--surface-1", "#161a29"),
            ("--surface-2", "#1b2032"),
            ("--text-strong", "#f2f3ff"),
            ("--text", "#d8ddf1"),
            ("--text-muted", "#9ea8c7"),
            ("--focus-ring", "#a8b8ff"),
        ] {
            tokens.insert(k.to_string(), v.to_string());
        }
    } else if kind == BuiltinThemeKind::Amoled {
        for (k, v) in [
            ("--bg-app", "#000000"),
            ("--bg-chat", "#000000"),
            ("--surface-1", "#070709"),
            ("--surface-2", "#0c0c0e"),
            ("--text-strong", "#ffffff"),
        ] {
            tokens.insert(k.to_string(), v.to_string());
        }
    }
    ThemePalette {
        id: kind.as_id().to_string(),
        label: kind.as_id().to_string(),
        color_scheme: kind.color_scheme().to_string(),
        tokens,
    }
}

fn audit_surface_pair(
    tokens: &BTreeMap<String, String>,
    s_key: &str,
    bg: RgbaColor,
    failed: &mut Vec<String>,
) -> Result<(usize, f64, f64), CustomThemeError> {
    let texts = [
        "--text-strong",
        "--text",
        "--text-subtle",
        "--text-muted",
        "--text-link",
        "--text-accent",
        "--danger-text",
        "--positive-text",
    ];
    let mut count = 0usize;
    let mut min_txt = f64::INFINITY;
    let mut min_ui = f64::INFINITY;
    for t_key in texts {
        let fg = parse_css_color(tokens.get(t_key).map(String::as_str).unwrap_or(""))?;
        let ratio = contrast_ratio(fg, bg);
        count += 1;
        min_txt = min_txt.min(ratio);
        let req = if t_key == "--text-strong" || t_key == "--text" {
            WCAG_AAA_NORMAL_MIN_RATIO
        } else {
            WCAG_AA_NORMAL_MIN_RATIO
        };
        if ratio + 1e-9 < req {
            failed.push(format!("{t_key} on {s_key}: {ratio:.2} < {req:.1}"));
        }
    }
    for ui_key in ["--focus-ring", "--status-online"] {
        let fg = parse_css_color(tokens.get(ui_key).map(String::as_str).unwrap_or(""))?;
        let ratio = contrast_ratio(fg, bg);
        count += 1;
        min_ui = min_ui.min(ratio);
        if ratio + 1e-9 < WCAG_AA_NON_TEXT_MIN_RATIO {
            failed.push(format!("{ui_key} on {s_key}: {ratio:.2}"));
        }
    }
    Ok((count, min_txt, min_ui))
}

pub fn audit_palette_wcag_aa(
    tokens: &BTreeMap<String, String>,
) -> Result<WcagAuditReport, CustomThemeError> {
    let mut failed = Vec::new();
    let mut audited = 0usize;
    let mut min_txt = f64::INFINITY;
    let mut min_ui = f64::INFINITY;

    for s_key in ["--bg-app", "--bg-chat", "--surface-1", "--surface-2"] {
        let bg = parse_css_color(tokens.get(s_key).map(String::as_str).unwrap_or(""))?;
        let (cnt, mt, mu) = audit_surface_pair(tokens, s_key, bg, &mut failed)?;
        audited += cnt;
        min_txt = min_txt.min(mt);
        min_ui = min_ui.min(mu);
    }
    Ok(WcagAuditReport {
        passed: failed.is_empty() && audited > 0,
        audited_pairs: audited,
        failed_pairs: failed,
        min_text_ratio: min_txt,
        min_non_text_ratio: min_ui,
    })
}

pub struct CustomThemeEngine {
    pub active_theme_id: String,
    pub preview_state: ThemePreviewState,
    pub custom_themes: BTreeMap<String, ThemePalette>,
}

impl Default for CustomThemeEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomThemeEngine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_theme_id: "dark".to_string(),
            preview_state: ThemePreviewState::DefaultDark,
            custom_themes: BTreeMap::new(),
        }
    }

    pub fn hover_preview(&mut self, theme_id: &str) -> Result<ThemePreviewState, CustomThemeError> {
        let _ = self.resolve_palette(theme_id)?;
        self.preview_state = ThemePreviewState::HighlightTheme;
        Ok(self.preview_state)
    }

    pub fn register_custom_theme(&mut self, palette: ThemePalette) -> Result<(), CustomThemeError> {
        if !self.custom_themes.contains_key(&palette.id)
            && self.custom_themes.len() >= MAX_INSTALLED_CUSTOM_THEMES
        {
            return Err(CustomThemeError::MaxCustomThemesExceeded(
                MAX_INSTALLED_CUSTOM_THEMES,
            ));
        }
        let mut clean_tokens = BTreeMap::new();
        for (k, v) in &palette.tokens {
            let (ck, cv) = sanitize_css_token_entry(k, v)?;
            clean_tokens.insert(ck, cv);
        }
        let report = audit_palette_wcag_aa(&clean_tokens)?;
        if !report.passed {
            return Err(CustomThemeError::WcagContrastViolation(report.failed_pairs));
        }
        self.custom_themes.insert(
            palette.id.clone(),
            ThemePalette {
                tokens: clean_tokens,
                ..palette
            },
        );
        Ok(())
    }

    pub fn resolve_palette(&self, theme_id: &str) -> Result<ThemePalette, CustomThemeError> {
        match theme_id {
            "dark" => Ok(builtin_palette(BuiltinThemeKind::Dark)),
            "light" => Ok(builtin_palette(BuiltinThemeKind::Light)),
            "midnight" => Ok(builtin_palette(BuiltinThemeKind::Midnight)),
            "amoled" => Ok(builtin_palette(BuiltinThemeKind::Amoled)),
            other => self
                .custom_themes
                .get(other)
                .cloned()
                .ok_or_else(|| CustomThemeError::UnknownTheme(other.to_string())),
        }
    }

    pub fn apply_theme(&mut self, theme_id: &str) -> Result<ThemeSwitchResult, CustomThemeError> {
        let palette = self.resolve_palette(theme_id)?;
        let report = audit_palette_wcag_aa(&palette.tokens)?;
        if !report.passed {
            return Err(CustomThemeError::WcagContrastViolation(report.failed_pairs));
        }
        self.active_theme_id = palette.id.clone();
        self.preview_state = ThemePreviewState::Applied;
        let mut css = format!(":root[data-theme=\"{}\"] {{\n", palette.id);
        for (k, v) in &palette.tokens {
            css.push_str(&format!("  {k}: {v};\n"));
        }
        css.push('}');
        Ok(ThemeSwitchResult {
            active_theme_id: self.active_theme_id.clone(),
            preview_state: self.preview_state,
            reloaded: false,
            injected_css: css,
        })
    }

    pub fn emergency_reset(&mut self) -> Result<ThemeSwitchResult, CustomThemeError> {
        let mut res = self.apply_theme("dark")?;
        self.preview_state = ThemePreviewState::DefaultDark;
        res.preview_state = ThemePreviewState::DefaultDark;
        Ok(res)
    }
}

pub fn validate_theme_sync_bounds(
    snowflake_id: u64,
    message_chars: usize,
    attachments: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
    has_capability: bool,
) -> Result<&'static str, CustomThemeError> {
    if !has_capability {
        return Err(CustomThemeError::AccessDenied(
            "CUSTOM_THEME_CAPABILITY_REQUIRED",
        ));
    }
    if message_chars > MAX_MESSAGE_CHARS
        || attachments > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(CustomThemeError::BoundaryViolation("LIMIT_EXCEEDED"));
    }
    let _ = snowflake_id;
    Ok("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_985_1_builtin_palettes_and_picker_tokens() {
        assert_eq!(THEME_PICKER_CONTAINER, "Theme Picker");
        assert_eq!(THEME_PICKER_DIMENSIONS, "100%");
        assert_eq!(THEME_PICKER_PADDING, "16px");
        assert_eq!(THEME_PICKER_SPACING, "12px");
        assert_eq!(TOKEN_BG, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");

        let amoled = builtin_palette(BuiltinThemeKind::Amoled);
        assert_eq!(
            amoled.tokens.get("--bg-app").map(String::as_str),
            Some("#000000")
        );
        assert_eq!(
            amoled.tokens.get("--bg-chat").map(String::as_str),
            Some("#000000")
        );
    }

    #[test]
    fn test_sub_985_2_hot_theme_switch_without_reload() {
        let mut engine = CustomThemeEngine::new();
        assert_eq!(engine.active_theme_id, "dark");
        assert_eq!(engine.preview_state.as_str(), "dark");

        let hovered = engine.hover_preview("midnight");
        assert_eq!(hovered, Ok(ThemePreviewState::HighlightTheme));
        assert_eq!(engine.preview_state.as_str(), "highlight theme");

        for id in ["midnight", "light", "amoled", "dark"] {
            let switched = engine.apply_theme(id);
            assert!(switched.is_ok());
            if let Ok(res) = switched {
                assert!(!res.reloaded);
                assert_eq!(res.active_theme_id, id);
                assert_eq!(res.preview_state.as_str(), "applied");
                assert!(res.injected_css.contains("--bg-app:"));
            }
        }

        let reset = engine.emergency_reset();
        assert!(reset.is_ok());
        if let Ok(res) = reset {
            assert!(!res.reloaded);
            assert_eq!(res.active_theme_id, "dark");
            assert_eq!(res.preview_state.as_str(), "dark");
        }
    }

    #[test]
    fn test_sub_985_3_wcag_aa_contrast_and_css_sanitizer() {
        for kind in [
            BuiltinThemeKind::Dark,
            BuiltinThemeKind::Light,
            BuiltinThemeKind::Midnight,
            BuiltinThemeKind::Amoled,
        ] {
            let pal = builtin_palette(kind);
            let report = audit_palette_wcag_aa(&pal.tokens);
            assert!(report.is_ok());
            if let Ok(rep) = report {
                assert!(
                    rep.passed,
                    "Theme {:?} failed: {:?}",
                    kind, rep.failed_pairs
                );
                assert!(rep.min_text_ratio >= 4.5);
                assert!(rep.min_non_text_ratio >= 3.0);
            }
        }

        let mut bad_pal = builtin_palette(BuiltinThemeKind::Dark);
        // #5c5e66 на #121214 даёт ~2.85:1 (< 4.5 AA, но > 1.5), убивая мутацию порога WCAG
        bad_pal
            .tokens
            .insert("--text-muted".to_string(), "#5c5e66".to_string());
        let bad_rep = audit_palette_wcag_aa(&bad_pal.tokens);
        assert!(bad_rep.is_ok());
        if let Ok(rep) = bad_rep {
            assert!(!rep.passed);
        }

        assert_eq!(
            sanitize_css_token_entry("--bg-app", "url(https://evil.example/a.png)"),
            Err(CustomThemeError::ForbiddenCssConstruct(
                "url(https://evil.example/a.png)".to_string()
            ))
        );
        assert_eq!(
            sanitize_css_token_entry("--bg-app", "#121214 !important"),
            Err(CustomThemeError::ForbiddenCssConstruct(
                "#121214 !important".to_string()
            ))
        );
        assert_eq!(
            sanitize_css_token_entry("body", "#121214"),
            Err(CustomThemeError::InvalidCssPropertyName("body".to_string()))
        );
    }

    #[test]
    fn test_sub_985_4_all_nine_dimensions_contract() {
        let mut engine = CustomThemeEngine::new();
        for idx in 1..=8 {
            let mut pal = builtin_palette(BuiltinThemeKind::Dark);
            pal.id = format!("custom-{idx}");
            assert!(engine.register_custom_theme(pal).is_ok());
        }
        let mut ninth = builtin_palette(BuiltinThemeKind::Dark);
        ninth.id = "custom-9".to_string();
        assert_eq!(
            engine.register_custom_theme(ninth),
            Err(CustomThemeError::MaxCustomThemesExceeded(8))
        );

        assert_eq!(
            validate_theme_sync_bounds(0, 2_000, 10, 25 * 1024 * 1024, 9_975, 200, true),
            Ok("ok")
        );
        assert_eq!(
            validate_theme_sync_bounds(u64::MAX, 2_001, 10, 1024, 0, 100, true),
            Err(CustomThemeError::BoundaryViolation("LIMIT_EXCEEDED"))
        );
        assert_eq!(
            validate_theme_sync_bounds(1, 10, 1, 100, 0, 100, false),
            Err(CustomThemeError::AccessDenied(
                "CUSTOM_THEME_CAPABILITY_REQUIRED"
            ))
        );
    }
}
