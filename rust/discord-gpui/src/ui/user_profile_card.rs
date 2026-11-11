use crate::state::{AppState, PresenceStatus};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Карточка пользователя (User Profile Popover)
/// Реализует окно профиля по эталону /home/sanya/re-deep/ui/06-profile-popout.html
pub struct UserProfileCard;

impl UserProfileCard {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let member = match &state.selected_member_profile {
            Some(m) => m,
            None => return div().into_any_element(),
        };

        let username = member.username.clone();
        let display_name = member
            .global_name
            .clone()
            .unwrap_or_else(|| username.clone());
        let status = member.status;
        let activity = member.activity.clone();
        let roles = member.roles.clone();

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.state.close_member_profile();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .v_flex()
                    .w(px(320.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .rounded(px(10.))
                    .shadow_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .on_mouse_down(MouseButton::Left, |_e, _window, cx| {
                        // Остановка всплытия клика внутри модалки
                        cx.stop_propagation();
                    })
                    // 1. Верхний баннер профиля (акцентный цвет Discord)
                    .child(
                        div()
                            .relative()
                            .h(px(70.))
                            .w_full()
                            .bg(gpui::rgb(0x30231c))
                            // Круглая аватарка с абсолютным позиционированием
                            .child(
                                div()
                                    .absolute()
                                    .top(px(24.))
                                    .left(px(16.))
                                    .size(px(72.))
                                    .rounded_full()
                                    .border_4()
                                    .border_color(DISCORD_THEME.bg_secondary)
                                    .bg(DISCORD_THEME.blurple)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(28.))
                                    .child(
                                        username
                                            .chars()
                                            .next()
                                            .map(|c| c.to_string())
                                            .unwrap_or_default(),
                                    )
                                    // Индикатор статуса присутствия
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .right_0()
                                            .size(px(16.))
                                            .rounded_full()
                                            .border_2()
                                            .border_color(DISCORD_THEME.bg_secondary)
                                            .bg(match status {
                                                PresenceStatus::Online => {
                                                    DISCORD_THEME.status_online
                                                }
                                                PresenceStatus::Idle => DISCORD_THEME.status_idle,
                                                PresenceStatus::Dnd => DISCORD_THEME.status_dnd,
                                                PresenceStatus::Offline => {
                                                    DISCORD_THEME.status_offline
                                                }
                                            }),
                                    ),
                            ),
                    )
                    // 2. Тело карточки с отступом для накладываемой аватарки
                    .child(
                        div()
                            .v_flex()
                            .pt(px(36.))
                            .px(px(16.))
                            .pb(px(16.))
                            .gap(px(10.))
                            // Имя и тег
                            .child(
                                div()
                                    .v_flex()
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_header)
                                            .child(display_name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(format!("@{}", username)),
                                    ),
                            )
                            // Значки пользователя (Badges)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .px(px(6.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_active)
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child("🏆 Rust GPUI Pioneer"),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_active)
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child("⚡ Early Adopter"),
                                    ),
                            )
                            // Разделитель
                            .child(div().h(px(1.)).w_full().bg(DISCORD_THEME.border_subtle))
                            // Пользовательский статус или активность
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("АКТИВНОСТЬ"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child(activity.unwrap_or_else(|| {
                                                "Играет в GPUI Rust Engine".into()
                                            })),
                                    ),
                            )
                            // Роли на сервере
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(format!("РОЛИ — {}", roles.len())),
                                    )
                                    .child(div().flex().flex_wrap().gap(px(4.)).children(
                                        roles.into_iter().map(|role| {
                                            div()
                                                .px(px(6.))
                                                .py(px(2.))
                                                .rounded(px(4.))
                                                .bg(DISCORD_THEME.bg_primary)
                                                .text_size(px(11.))
                                                .text_color(DISCORD_THEME.text_normal)
                                                .border_1()
                                                .border_color(DISCORD_THEME.border_subtle)
                                                .child(format!("• {}", role))
                                        }),
                                    )),
                            )
                            // Кнопка быстрого действия: Отправить сообщение
                            .child(
                                div()
                                    .h(px(34.))
                                    .w_full()
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.blurple)
                                    .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("💬 Написать сообщение")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, cx| {
                                            this.state.close_member_profile();
                                            this.state.select_guild(None); // Переход в ЛС
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
