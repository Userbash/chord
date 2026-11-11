use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct SearchDrawer;

impl SearchDrawer {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_search_open {
            return div().into_any_element();
        }

        let query = state.search_query.clone();
        let results = &state.search_results;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000030))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.state.is_search_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .w(px(420.))
                    .bg(DISCORD_THEME.bg_secondary)
                    .border_l_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .v_flex()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {}) // stop propagation
                    // Заголовок панели поиска
                    .child(
                        div()
                            .h(px(48.))
                            .px(px(16.))
                            .border_b_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Поиск по сообщениям (Ctrl+F)"),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .p(px(4.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(16.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("✕")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.is_search_open = false;
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    )
                    // Поле ввода запроса и фильтры
                    .child(
                        div()
                            .p(px(16.))
                            .v_flex()
                            .gap(px(10.))
                            .child(
                                div()
                                    .h(px(40.))
                                    .px(px(12.))
                                    .rounded(px(6.))
                                    .bg(DISCORD_THEME.bg_primary)
                                    .border_1()
                                    .border_color(DISCORD_THEME.blurple)
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(if query.is_empty() {
                                                DISCORD_THEME.text_muted
                                            } else {
                                                DISCORD_THEME.text_header
                                            })
                                            .child(if query.is_empty() {
                                                "Поиск сообщений или @пользователя...".to_string()
                                            } else {
                                                query
                                            }),
                                    )
                                    .child(div().text_size(px(16.)).child("🔍")),
                            )
                            // Быстрые фильтры (from:, in:, has:)
                            .child(
                                div()
                                    .flex()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .px(px(8.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_tertiary)
                                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                            .cursor_pointer()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("от:пользователя")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.state.set_search_query("Rustery".into());
                                                    cx.notify();
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px(px(8.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_tertiary)
                                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                            .cursor_pointer()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("в:канале")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.state.set_search_query("GPUI".into());
                                                    cx.notify();
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px(px(8.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_tertiary)
                                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                            .cursor_pointer()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("содержит:ссылку")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.state.set_search_query("http".into());
                                                    cx.notify();
                                                }),
                                            ),
                                    ),
                            ),
                    )
                    // Список найденных результатов
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .overflow_hidden()
                            .p(px(16.))
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(format!("РЕЗУЛЬТАТЫ ПОИСКА ({})", results.len())),
                            )
                            .children(results.iter().map(|msg| {
                                let ch_id = msg.channel_id.clone();
                                div()
                                    .p(px(10.))
                                    .rounded(px(6.))
                                    .bg(DISCORD_THEME.bg_primary)
                                    .border_1()
                                    .border_color(DISCORD_THEME.border_subtle)
                                    .hover(|s| s.border_color(DISCORD_THEME.blurple))
                                    .cursor_pointer()
                                    .v_flex()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(DISCORD_THEME.text_header)
                                                    .child(msg.author.username.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(DISCORD_THEME.text_muted)
                                                    .child(msg.timestamp.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child(msg.content.clone()),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.state.select_channel(ch_id.clone());
                                            this.state.is_search_open = false;
                                            cx.notify();
                                        }),
                                    )
                            })),
                    ),
            )
            .into_any_element()
    }
}
