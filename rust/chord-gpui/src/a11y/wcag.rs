//! Автоматизированный аудит доступности по стандарту WCAG 2.2 (Подзадача 902.3).
//! Включает проверку коэффициентов контрастности (AA >= 4.5:1, AAA >= 7.0:1, Focus >= 3.0:1)
//! и проверку отсутствия «клавиатурных ловушек» (No Keyboard Trap, SC 2.1.2) во всех модалках.

use crate::a11y::AppDriver;
use crate::state::{CaptchaChallenge, ChannelType, GuildMember, PresenceStatus};
use crate::theme::discord_dark::DiscordThemeColors;
use gpui::Hsla;

/// Отчёт аудита контрастности WCAG 2.2 (AA >= 4.5:1, AAA >= 7.0:1, Focus/UI >= 3.0:1).
#[derive(Debug, Clone, PartialEq)]
pub struct Wcag22ContrastReport {
    pub passed_aa: bool,
    pub passed_aaa_primary: bool,
    pub passed_focus_appearance: bool,
    pub audited_pairs: usize,
    pub aa_min_ratio: f32,
    pub aaa_min_ratio: f32,
    pub violations: Vec<String>,
}

/// Отчёт проверки отсутствия клавиатурных ловушек (Focus Trap Audit) во всех модальных окнах.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTrapAuditReport {
    pub passed: bool,
    pub audited_modals: Vec<String>,
    pub trapped_modals: Vec<String>,
    pub focus_leak_modals: Vec<String>,
}

pub struct Wcag22Auditor;

impl Wcag22Auditor {
    /// Полный аудит цветовой палитры темы на соответствие WCAG 2.2 Level AA и AAA.
    pub fn audit_theme_contrast(theme: &DiscordThemeColors) -> Wcag22ContrastReport {
        let mut violations = Vec::new();
        let aaa_pairs = Self::aaa_primary_pairs(theme);
        let aa_pairs = Self::aa_normal_pairs(theme);
        let focus_pairs = Self::focus_indicator_pairs(theme);

        let mut aaa_min = f32::MAX;
        for (label, fg, bg) in &aaa_pairs {
            let ratio = theme.contrast_ratio(*fg, *bg);
            aaa_min = aaa_min.min(ratio);
            if ratio < 7.0 {
                violations.push(format!("{label}: AAA ratio {ratio:.2}:1 < 7.0:1"));
            }
        }

        let mut aa_min = aaa_min;
        for (label, fg, bg) in &aa_pairs {
            let ratio = theme.contrast_ratio(*fg, *bg);
            aa_min = aa_min.min(ratio);
            if ratio < 4.5 {
                violations.push(format!("{label}: AA ratio {ratio:.2}:1 < 4.5:1"));
            }
        }

        let mut focus_ok = true;
        for (label, fg, bg) in &focus_pairs {
            let ratio = theme.contrast_ratio(*fg, *bg);
            if ratio < 3.0 {
                focus_ok = false;
                violations.push(format!("{label}: Focus ratio {ratio:.2}:1 < 3.0:1"));
            }
        }

        let total = aaa_pairs.len() + aa_pairs.len() + focus_pairs.len();
        Wcag22ContrastReport {
            passed_aa: aa_min >= 4.5,
            passed_aaa_primary: aaa_min >= 7.0,
            passed_focus_appearance: focus_ok,
            audited_pairs: total,
            aa_min_ratio: aa_min,
            aaa_min_ratio: aaa_min,
            violations,
        }
    }

    fn aaa_primary_pairs(theme: &DiscordThemeColors) -> Vec<(&'static str, Hsla, Hsla)> {
        vec![
            (
                "text_header on bg_primary",
                theme.text_header,
                theme.bg_primary,
            ),
            (
                "text_header on bg_secondary",
                theme.text_header,
                theme.bg_secondary,
            ),
            (
                "text_header on bg_tertiary",
                theme.text_header,
                theme.bg_tertiary,
            ),
            ("text_header on bg_modal", theme.text_header, theme.bg_modal),
            (
                "text_normal on bg_primary",
                theme.text_normal,
                theme.bg_primary,
            ),
            (
                "text_normal on bg_secondary",
                theme.text_normal,
                theme.bg_secondary,
            ),
            (
                "text_normal on bg_tertiary",
                theme.text_normal,
                theme.bg_tertiary,
            ),
            ("text_code on bg_primary", theme.text_code, theme.bg_primary),
        ]
    }

    fn aa_normal_pairs(theme: &DiscordThemeColors) -> Vec<(&'static str, Hsla, Hsla)> {
        let mut pairs = Self::aa_text_surface_pairs(theme);
        pairs.extend(Self::aa_semantic_and_button_pairs(theme));
        pairs
    }

    fn aa_text_surface_pairs(theme: &DiscordThemeColors) -> Vec<(&'static str, Hsla, Hsla)> {
        vec![
            (
                "text_muted on bg_primary",
                theme.text_muted,
                theme.bg_primary,
            ),
            (
                "text_muted on bg_secondary",
                theme.text_muted,
                theme.bg_secondary,
            ),
            (
                "text_muted on bg_tertiary",
                theme.text_muted,
                theme.bg_tertiary,
            ),
            (
                "text_muted_aa on bg_accent",
                theme.text_muted_aa,
                theme.bg_accent,
            ),
            ("text_link on bg_primary", theme.text_link, theme.bg_primary),
            (
                "text_link on bg_secondary",
                theme.text_link,
                theme.bg_secondary,
            ),
        ]
    }

    fn aa_semantic_and_button_pairs(theme: &DiscordThemeColors) -> Vec<(&'static str, Hsla, Hsla)> {
        vec![
            (
                "text_danger on bg_primary",
                theme.text_danger,
                theme.bg_primary,
            ),
            (
                "text_positive on bg_primary",
                theme.text_positive,
                theme.bg_primary,
            ),
            (
                "text_warning on bg_primary",
                theme.text_warning,
                theme.bg_primary,
            ),
            (
                "btn_primary_text on btn_primary_bg",
                theme.btn_primary_text,
                theme.btn_primary_bg,
            ),
            (
                "btn_danger_text on btn_danger_bg",
                theme.btn_danger_text,
                theme.btn_danger_bg,
            ),
            (
                "btn_success_text on btn_success_bg",
                theme.btn_success_text,
                theme.btn_success_bg,
            ),
        ]
    }

    fn focus_indicator_pairs(theme: &DiscordThemeColors) -> Vec<(&'static str, Hsla, Hsla)> {
        vec![
            (
                "focus_ring on bg_primary",
                theme.border_interactive_focus,
                theme.bg_primary,
            ),
            (
                "focus_ring on bg_secondary",
                theme.border_interactive_focus,
                theme.bg_secondary,
            ),
            (
                "focus_ring on bg_tertiary",
                theme.border_interactive_focus,
                theme.bg_tertiary,
            ),
        ]
    }

    /// Проверка всех модальных окон на удержание фокуса (Focus Containment) и выход по `Escape`.
    pub fn audit_all_modals_focus_traps(driver: &mut AppDriver) -> FocusTrapAuditReport {
        let modal_ids = [
            "modal-login",
            "modal-settings",
            "modal-quick-switcher",
            "modal-create-server",
            "modal-screen-share",
            "modal-captcha",
            "modal-emoji-picker",
            "modal-status-picker",
            "modal-account-switcher",
            "modal-context-menu",
            "modal-member-profile",
            "modal-create-channel",
            "modal-search",
        ];

        let mut audited = Vec::with_capacity(modal_ids.len());
        let mut trapped = Vec::new();
        let mut leaks = Vec::new();

        for modal_id in modal_ids {
            Self::open_modal_by_id(driver, modal_id);
            audited.push(modal_id.to_string());

            if !Self::verify_modal_focus_containment(driver, modal_id) {
                leaks.push(modal_id.to_string());
            }

            let escaped = driver.press_hotkey("Escape").unwrap_or(false);
            if !escaped || driver.tree().active_modal_scope.is_some() {
                trapped.push(modal_id.to_string());
            }
        }

        FocusTrapAuditReport {
            passed: trapped.is_empty() && leaks.is_empty(),
            audited_modals: audited,
            trapped_modals: trapped,
            focus_leak_modals: leaks,
        }
    }

    fn verify_modal_focus_containment(driver: &mut AppDriver, expected_scope: &str) -> bool {
        let first = match driver.press_tab(false) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let second = match driver.press_tab(false) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let back = match driver.press_tab(true) {
            Ok(h) => h,
            Err(_) => return false,
        };
        if back.id != first.id {
            return false;
        }
        let tree = driver.tree();
        for id in [&first.id, &second.id] {
            let scoped_ok = tree
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .and_then(|n| n.modal_scope.as_deref())
                == Some(expected_scope);
            if !scoped_ok {
                return false;
            }
        }
        true
    }

    fn open_modal_by_id(driver: &mut AppDriver, modal_id: &str) {
        driver.focused_id = None;
        match modal_id {
            "modal-login" => driver.state.open_login_modal(),
            "modal-settings" => driver.state.is_settings_open = true,
            "modal-quick-switcher" => driver.state.open_quick_switcher(),
            "modal-create-server" => driver.state.open_create_server(),
            "modal-screen-share" => driver.state.open_screen_share_modal(),
            "modal-captcha" => {
                driver.state.active_captcha = Some(CaptchaChallenge {
                    sitekey: "sitekey-test".into(),
                    service: "hcaptcha".into(),
                    rqdata: None,
                });
            }
            "modal-emoji-picker" => driver.state.is_emoji_picker_open = true,
            "modal-status-picker" => driver.state.is_status_picker_open = true,
            "modal-account-switcher" => driver.state.is_account_switcher_open = true,
            "modal-context-menu" => driver.state.active_context_menu = Some("msg-1".into()),
            "modal-member-profile" => {
                driver.state.selected_member_profile = Some(GuildMember {
                    id: "u-1".into(),
                    username: "Member".into(),
                    global_name: None,
                    avatar: None,
                    status: PresenceStatus::Online,
                    activity: None,
                    roles: Vec::new(),
                });
            }
            "modal-create-channel" => driver.state.open_create_channel(ChannelType::GuildText),
            "modal-search" => driver.state.is_search_open = true,
            _ => {}
        }
    }
}
