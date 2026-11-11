use crate::state::{AppState, CallStatus};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Верхняя панель активного вызова в Личных сообщениях (In-Call Overlay)
/// Полностью повторяет визуальный дизайн из Video_2026-11-07_22-11-28.mp4
pub struct CallOverlay;

impl CallOverlay {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let (recipient_name, is_connected, duration_secs) = match &state.active_call {
            CallStatus::Idle => return div().into_any_element(),
            CallStatus::Ringing { recipient_name, .. } => {
                if state.selected_guild_id.is_some() {
                    return div().into_any_element();
                }
                (recipient_name.clone(), false, 0)
            }
            CallStatus::Connected {
                channel_id: _,
                recipient_name,
                duration_secs,
                ..
            } => {
                if state.selected_guild_id.is_some() {
                    return div().into_any_element();
                }
                (recipient_name.clone(), true, *duration_secs)
            }
        };

        let current_user = &state.current_user;
        let is_muted = state.is_muted;
        let is_deafened = state.is_deafened;
        let is_camera_enabled = state.is_camera_enabled;
        let is_screen_sharing = state.is_screen_sharing;

        div()
            .w_full()
            .bg(gpui::rgba(0x111214ee))
            .border_b_1()
            .border_color(DISCORD_THEME.border_subtle)
            .p(px(16.))
            .v_flex()
            .items_center()
            .gap(px(16.))
            // 1. Верхняя строка: Заголовок и таймер
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(16.)).child("📞"))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child(format!("Звонок: @{}", recipient_name)),
                            ),
                    )
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(2.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .rounded(px(4.))
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_connected {
                                DISCORD_THEME.status_online
                            } else {
                                DISCORD_THEME.text_muted
                            })
                            .child(if is_connected {
                                format!("{:02}:{:02}", duration_secs / 60, duration_secs % 60)
                            } else {
                                "Вызов...".into()
                            }),
                    ),
            )
            // 2. Сетка участников звонка (Две карточки по центру)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(24.))
                    .items_center()
                    .justify_center()
                    // Карточка текущего пользователя (Rustery)
                    .child(
                        div()
                            .w(px(160.))
                            .h(px(120.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .rounded(px(8.))
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .v_flex()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .size(px(52.))
                                    .rounded_full()
                                    .bg(DISCORD_THEME.blurple)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(22.))
                                    .child(
                                        current_user
                                            .username
                                            .chars()
                                            .next()
                                            .map(|c| c.to_string())
                                            .unwrap_or_default(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child(current_user.username.clone()),
                            ),
                    )
                    // Карточка собеседника
                    .child(
                        div()
                            .w(px(160.))
                            .h(px(120.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .rounded(px(8.))
                            .border_1()
                            .border_color(if is_connected {
                                DISCORD_THEME.status_online
                            } else {
                                DISCORD_THEME.border_subtle
                            })
                            .v_flex()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .size(px(52.))
                                    .rounded_full()
                                    .bg(gpui::rgb(0x9333ea))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(22.))
                                    .child(
                                        recipient_name
                                            .chars()
                                            .next()
                                            .map(|c| c.to_string())
                                            .unwrap_or_default(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child(recipient_name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(if is_connected {
                                        DISCORD_THEME.status_online
                                    } else {
                                        DISCORD_THEME.status_idle
                                    })
                                    .child(if is_connected {
                                        "В эфире"
                                    } else {
                                        "Ожидание ответа..."
                                    }),
                            ),
                    ),
            )
            // 3. Панель управления звонком (Камера, Экран, Микрофон, Звук, Завершить)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(10.))
                    .items_center()
                    // 1. Камера
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(if is_camera_enabled {
                                DISCORD_THEME.blurple
                            } else {
                                DISCORD_THEME.bg_secondary
                            })
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .text_color(gpui::white())
                            .child("📷 Камера")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_camera();
                                    cx.notify();
                                }),
                            ),
                    )
                    // 2. Демонстрация экрана
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(if is_screen_sharing {
                                DISCORD_THEME.blurple
                            } else {
                                DISCORD_THEME.bg_secondary
                            })
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .text_color(gpui::white())
                            .child("🖥️ Демонстрация экрана")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.open_screen_share_modal();
                                    cx.notify();
                                }),
                            ),
                    )
                    // 3. Выкл. микрофон
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .text_color(if is_muted {
                                DISCORD_THEME.status_dnd
                            } else {
                                gpui::white()
                            })
                            .child(if is_muted {
                                "🔇 Вкл. микрофон"
                            } else {
                                "🎙️ Выкл. микрофон"
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_mute();
                                    cx.notify();
                                }),
                            ),
                    )
                    // 4. Выкл. звук
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .text_color(if is_deafened {
                                DISCORD_THEME.status_dnd
                            } else {
                                gpui::white()
                            })
                            .child(if is_deafened {
                                "🚫 Вкл. звук"
                            } else {
                                "🎧 Выкл. звук"
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_deaf();
                                    cx.notify();
                                }),
                            ),
                    )
                    // 5. Завершить звонок (Красная кнопка)
                    .child(
                        div()
                            .px(px(16.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.status_dnd)
                            .hover(|s| s.bg(gpui::rgb(0xc02e33)))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("📞 Завершить звонок")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.end_call();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            .into_any_element()
    }
}
