use crate::state::AppState;
use crate::state::models::PresenceStatus;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct StatusPicker;

impl StatusPicker {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_status_picker_open {
            return div().into_any_element();
        }

        let current_status = state.current_user.status;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000020))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.state.is_status_picker_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(60.))
                    .left(px(80.))
                    .w(px(250.))
                    .bg(DISCORD_THEME.bg_tertiary)
                    .rounded(px(8.))
                    .border_1()
                    .border_color(DISCORD_THEME.border_strong)
                    .p(px(8.))
                    .v_flex()
                    .gap(px(4.))
                    // 1. В сети
                    .child(Self::render_status_item(
                        PresenceStatus::Online,
                        DISCORD_THEME.status_online,
                        "В сети",
                        "Виден для всех",
                        current_status == PresenceStatus::Online,
                        cx,
                    ))
                    // 2. Не активен
                    .child(Self::render_status_item(
                        PresenceStatus::Idle,
                        DISCORD_THEME.status_idle,
                        "Не активен",
                        "Отойти от компьютера",
                        current_status == PresenceStatus::Idle,
                        cx,
                    ))
                    // 3. Не беспокоить
                    .child(Self::render_status_item(
                        PresenceStatus::Dnd,
                        DISCORD_THEME.status_dnd,
                        "Не беспокоить",
                        "Не получать уведомления",
                        current_status == PresenceStatus::Dnd,
                        cx,
                    ))
                    // 4. Невидимый
                    .child(Self::render_status_item(
                        PresenceStatus::Offline,
                        DISCORD_THEME.status_offline,
                        "Невидимый",
                        "Вы не кажетесь в сети",
                        current_status == PresenceStatus::Offline,
                        cx,
                    ))
                    // Разделитель
                    .child(div().h(px(1.)).my(px(4.)).bg(DISCORD_THEME.border_subtle))
                    // Кастомный статус
                    .child(
                        div()
                            .h(px(34.))
                            .px(px(8.))
                            .rounded(px(4.))
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(16.)).child("💬"))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Задать статус..."),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_status_item(
        status: PresenceStatus,
        color: Hsla,
        title: &'static str,
        subtitle: &'static str,
        is_selected: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .p(px(6.))
            .rounded(px(4.))
            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(12.)).rounded_full().bg(color))
                    .child(
                        div()
                            .v_flex()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(subtitle),
                            ),
                    ),
            )
            .children(if is_selected {
                Some(
                    div()
                        .text_size(px(14.))
                        .text_color(DISCORD_THEME.blurple)
                        .child("✓"),
                )
            } else {
                None
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.state.set_status(status);
                    cx.notify();
                }),
            )
    }
}
