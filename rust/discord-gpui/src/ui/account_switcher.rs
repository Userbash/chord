use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Меню переключения учетных записей (Account Switcher Popover)
/// Вызывается кликом по иконке пользователя в нижнем футере
pub struct AccountSwitcher;

impl AccountSwitcher {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_account_switcher_open {
            return div().into_any_element();
        }

        let accounts = &state.accounts;

        div()
            .absolute()
            .bottom(px(60.))
            .left(px(80.))
            .w(px(280.))
            .bg(DISCORD_THEME.bg_secondary)
            .rounded(px(8.))
            .shadow_lg()
            .border_1()
            .border_color(DISCORD_THEME.border_subtle)
            .p(px(8.))
            .v_flex()
            .gap(px(4.))
            // Заголовок
            .child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(DISCORD_THEME.text_muted)
                    .child("ПЕРЕКЛЮЧЕНИЕ УЧЁТНЫХ ЗАПИСЕЙ"),
            )
            // Список сохраненных аккаунтов
            .children(accounts.iter().map(|acc| {
                let token = acc.token.clone();
                let username = acc.user.username.clone();
                let is_active = acc.is_active;

                div()
                    .h(px(40.))
                    .px(px(8.))
                    .rounded(px(6.))
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
                            .child(
                                div()
                                    .size(px(24.))
                                    .rounded_full()
                                    .bg(DISCORD_THEME.blurple)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .text_size(px(12.))
                                    .child(
                                        username
                                            .chars()
                                            .next()
                                            .map(|c| c.to_string())
                                            .unwrap_or_default(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(if is_active {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if is_active {
                                        DISCORD_THEME.text_header
                                    } else {
                                        DISCORD_THEME.text_normal
                                    })
                                    .child(username),
                            ),
                    )
                    .child(if is_active {
                        div()
                            .text_size(px(12.))
                            .text_color(DISCORD_THEME.status_online)
                            .child("✓")
                    } else {
                        div()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let token = token.clone();
                            move |this, _, _window, cx| {
                                this.state.switch_account(&token);
                                cx.notify();
                            }
                        }),
                    )
            }))
            .child(div().h(px(1.)).mx(px(4.)).bg(DISCORD_THEME.border_subtle))
            // Кнопка добавить аккаунт (вход)
            .child(
                div()
                    .h(px(36.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(14.)).child("➕"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(DISCORD_THEME.text_normal)
                            .child("Добавить учётную запись"),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.open_login_modal();
                            cx.notify();
                        }),
                    ),
            )
            // Кнопка Выйти из аккаунта
            .child(
                div()
                    .h(px(36.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .hover(|s| s.bg(gpui::rgba(0xDA373C22)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(14.)).child("🚪"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(DISCORD_THEME.status_dnd)
                            .child("Выйти из учётной записи"),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            this.state.logout();
                            cx.notify();
                        }),
                    ),
            )
            .into_any_element()
    }
}
