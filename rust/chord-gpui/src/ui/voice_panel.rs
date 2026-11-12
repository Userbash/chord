//! Bottom-left voice connection status bar and audio/mic/deafen control panel.

use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct VoicePanel;

impl VoicePanel {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let voice_session = match &state.voice_session {
            Some(v) => v.clone(),
            None => return div().into_any_element(),
        };

        // Если активен личный звонок и открыт соответствующий диалог,
        // панель внизу скрывается, так как элементы управления отображаются в оверлее звонка
        if matches!(
            state.active_call,
            crate::state::CallStatus::Connected { .. }
        ) {
            if let Some(ch_id) = &state.selected_channel_id {
                if ch_id.starts_with("dm-") {
                    return div().into_any_element();
                }
            }
        }

        div()
            .h(px(52.))
            .flex_none()
            .w_full()
            .px(px(8.))
            .py(px(4.))
            .bg(DISCORD_THEME.bg_secondary_alt)
            .border_b_1()
            .border_color(DISCORD_THEME.border_subtle)
            .v_flex()
            .justify_between()
            .overflow_hidden()
            // 1. Верхняя строка (24px): статус «📶 Голос подключён» + задержка пинга + кнопки управления
            .child(
                div()
                    .h(px(24.))
                    .flex_none()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .overflow_hidden()
                    // Индикатор статуса и задержки
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.status_online)
                                    .child("📶"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.status_online)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child("Голос подключён"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .whitespace_nowrap()
                                    .child(format!("({} мс)", voice_session.ping_ms)),
                            ),
                    )
                    // Кнопки управления (Камера, Демонстрация экрана, Отключение звонка)
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            // Кнопка камеры
                            .child(
                                div()
                                    .size(px(24.))
                                    .rounded(px(4.))
                                    .bg(if state.is_camera_enabled {
                                        DISCORD_THEME.bg_active
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .child(if state.is_camera_enabled {
                                        "📹"
                                    } else {
                                        "📷"
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.toggle_camera();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            // Кнопка стрима экрана
                            .child(
                                div()
                                    .size(px(24.))
                                    .rounded(px(4.))
                                    .bg(if state.is_screen_sharing {
                                        DISCORD_THEME.blurple
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .child("🖥️")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            if this.state.is_screen_sharing {
                                                this.state.stop_screen_sharing();
                                            } else {
                                                this.state.open_screen_share_modal();
                                            }
                                            cx.notify();
                                        }),
                                    ),
                            )
                            // Кнопка отключения звонка (красная телефонная трубка с hover)
                            .child(
                                div()
                                    .id("btn-disconnect-voice")
                                    .size(px(24.))
                                    .rounded(px(4.))
                                    .bg(gpui::rgba(0xDA373C26))
                                    .hover(|s| {
                                        s.bg(DISCORD_THEME.status_dnd).text_color(gpui::white())
                                    })
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.status_dnd)
                                    .child("📞")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.disconnect_voice();
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            // 2. Нижняя строка (18px): «🔊 Канал / Сервер» слева и «RTC Connected» справа
            .child(
                div()
                    .h(px(18.))
                    .flex_none()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .overflow_hidden()
                    .child(
                        div().flex_1().min_w(px(0.)).overflow_hidden().child(
                            div()
                                .text_size(px(11.))
                                .text_color(DISCORD_THEME.text_muted)
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(format!(
                                    "🔊 {} / {}",
                                    voice_session.channel_name, voice_session.guild_name
                                )),
                        ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .pl(px(4.))
                            .text_size(px(10.))
                            .text_color(DISCORD_THEME.text_muted)
                            .whitespace_nowrap()
                            .child("RTC Connected"),
                    ),
            )
            .into_any_element()
    }
}
