use crate::state::{AppState, PresenceStatus, Relationship};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Правая боковая панель профиля пользователя в Личных сообщениях (DM User Profile Sidebar - 320px)
/// Выполнена в точном соответствии с референсами Video_2026-11-07_22-11-28.mp4 и Video_2026-11-09_23-23-05.mp4
pub struct UserProfileSidebar;

impl UserProfileSidebar {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        _cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_dm_profile_open {
            return div().into_any_element();
        }

        // Находим текущего собеседника по selected_channel_id ("dm-{user_id}")
        let target_user_id = match state.selected_channel_id.as_deref() {
            Some(cid) if cid.starts_with("dm-") => cid.trim_start_matches("dm-"),
            _ => "u-demo-1", // fallback на первого друга
        };

        let relationship = state
            .relationships
            .iter()
            .find(|r| r.user_id == target_user_id)
            .or_else(|| state.relationships.first());

        let rel = match relationship {
            Some(r) => r,
            None => return div().into_any_element(),
        };

        div()
            .v_flex()
            .w(px(320.))
            .h_full()
            .bg(DISCORD_THEME.bg_secondary)
            .border_l_1()
            .border_color(DISCORD_THEME.border_subtle)
            .overflow_hidden()
            // 1. Градиентный верхний баннер
            .child(Self::render_banner(rel))
            // 2. Аватарка поверх баннера + статус
            .child(Self::render_avatar_header(rel))
            // 3. Основная карточка информации о пользователе
            .child(Self::render_profile_card(rel))
            .into_any_element()
    }

    /// Градиентный баннер вверху профиля
    fn render_banner(rel: &Relationship) -> impl IntoElement {
        let banner_color = rel.banner_color.as_deref().unwrap_or("#9333ea");

        let bg_color = match banner_color {
            "#e91e63" => gpui::rgb(0xe91e63),
            "#3b82f6" => gpui::rgb(0x3b82f6),
            _ => gpui::rgb(0x9333ea), // фиолетовый дефолт
        };

        div()
            .h(px(120.))
            .w_full()
            .bg(bg_color)
            .relative()
            .flex()
            .justify_end()
            .p(px(8.))
            // Кнопка действий "•••"
            .child(
                div()
                    .size(px(28.))
                    .rounded_full()
                    .bg(gpui::rgba(0x00000044))
                    .hover(|s| s.bg(gpui::rgba(0x00000088)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(gpui::white())
                    .text_size(px(14.))
                    .child("•••"),
            )
    }

    /// Блок аватарки с наложением на баннер и индикатором статуса
    fn render_avatar_header(rel: &Relationship) -> impl IntoElement {
        let status_color = match rel.status {
            PresenceStatus::Online => DISCORD_THEME.status_online,
            PresenceStatus::Idle => DISCORD_THEME.status_idle,
            PresenceStatus::Dnd => DISCORD_THEME.status_dnd,
            PresenceStatus::Offline => DISCORD_THEME.status_offline,
        };

        div().px(px(16.)).relative().child(
            div()
                .mt(px(-40.))
                .relative()
                .size(px(80.))
                .rounded_full()
                .border_4()
                .border_color(DISCORD_THEME.bg_secondary)
                .bg(DISCORD_THEME.blurple)
                .flex()
                .items_center()
                .justify_center()
                .text_color(gpui::white())
                .font_weight(FontWeight::BOLD)
                .text_size(px(32.))
                .child(
                    rel.user
                        .username
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_default(),
                )
                // Большой индикатор статуса
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .right_0()
                        .size(px(22.))
                        .rounded_full()
                        .border_4()
                        .border_color(DISCORD_THEME.bg_secondary)
                        .bg(status_color),
                ),
        )
    }

    /// Карточка профиля с метаданными
    fn render_profile_card(rel: &Relationship) -> impl IntoElement {
        let display_name = rel
            .user
            .global_name
            .as_deref()
            .unwrap_or(&rel.user.username);
        let user_tag = rel.user_tag.as_deref().unwrap_or("@unknown");
        let member_since = rel.member_since.as_deref().unwrap_or("26 нояб. 2017 г.");
        let mutual_guilds = rel.mutual_guilds_count;

        div()
            .p(px(16.))
            .v_flex()
            .gap(px(12.))
            // Внутренний контейнер карточки
            .child(
                div()
                    .p(px(12.))
                    .bg(DISCORD_THEME.bg_tertiary)
                    .rounded(px(8.))
                    .v_flex()
                    .gap(px(12.))
                    // Имя и тег
                    .child(
                        div()
                            .v_flex()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child(display_name.to_string()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(user_tag.to_string()),
                            ),
                    )
                    // Активность
                    .children(rel.activity.as_ref().map(|act| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().size(px(8.)).rounded_full().bg(DISCORD_THEME.blurple))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_normal)
                                    .child(act.clone()),
                            )
                    }))
                    .child(div().h(px(1.)).bg(DISCORD_THEME.border_subtle))
                    // В ЧИСЛЕ УЧАСТНИКОВ С
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("В ЧИСЛЕ УЧАСТНИКОВ С"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_normal)
                                    .child(member_since.to_string()),
                            ),
                    )
                    // ОБЩИЕ СЕРВЕРЫ
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(format!("ОБЩИЕ СЕРВЕРЫ — {}", mutual_guilds)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(if mutual_guilds > 0 {
                                        DISCORD_THEME.text_normal
                                    } else {
                                        DISCORD_THEME.text_muted
                                    })
                                    .child(if mutual_guilds > 0 {
                                        "Rust Developers Community"
                                    } else {
                                        "Нет общих серверов"
                                    }),
                            ),
                    )
                    // ЗАМЕТКА
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("ЗАМЕТКА"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(
                                        rel.note
                                            .as_deref()
                                            .unwrap_or("Нажмите, чтобы добавить заметку")
                                            .to_string(),
                                    ),
                            ),
                    )
                    // Кнопка "Полный профиль"
                    .child(
                        div()
                            .h(px(34.))
                            .w_full()
                            .mt(px(4.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(DISCORD_THEME.text_normal)
                            .child("Полный профиль"),
                    ),
            )
    }
}
