use crate::state::AppState;
use crate::state::models::QuickSwitcherItemKind;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct QuickSwitcher;

impl QuickSwitcher {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_quick_switcher_open {
            return div().into_any_element();
        }

        let query = &state.quick_switcher_query;
        let filtered_items = state.filter_quick_switcher();
        let selected_idx = state.quick_switcher_selected_index;

        let category_title = if query.starts_with('*') {
            "СЕРВЕРЫ"
        } else if query.starts_with('#') {
            "ТЕКСТОВЫЕ КАНАЛЫ"
        } else if query.starts_with('@') {
            "ПОЛЬЗОВАТЕЛИ И ДРУЗЬЯ"
        } else if query.starts_with('!') {
            "ГОЛОСОВЫЕ КАНАЛЫ"
        } else {
            "БЫСТРЫЙ ПЕРЕХОД"
        };

        let mut item_elements = Vec::new();

        if filtered_items.is_empty() {
            item_elements.push(
                div()
                    .py(px(24.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("🔍"),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child(if query.is_empty() {
                                "Нет доступных элементов".to_string()
                            } else {
                                format!("Ничего не найдено по запросу «{}»", query)
                            }),
                    )
                    .into_any_element(),
            );
        } else {
            for (idx, item) in filtered_items.iter().take(12).enumerate() {
                let is_selected = idx == selected_idx;
                let item_clone = item.clone();

                let (badge_text, badge_color) = match item.kind {
                    QuickSwitcherItemKind::Server => ("*", DISCORD_THEME.blurple),
                    QuickSwitcherItemKind::Channel => ("#", DISCORD_THEME.text_muted),
                    QuickSwitcherItemKind::User => ("@", DISCORD_THEME.status_online),
                };

                let mut item_el = div()
                    .h(px(40.))
                    .px(px(12.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover));

                if is_selected {
                    item_el = item_el.bg(DISCORD_THEME.bg_active);
                }

                item_el = item_el
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .w(px(24.))
                                    .h(px(24.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_secondary)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(badge_color)
                                    .child(badge_text),
                            )
                            .child(
                                div().flex().flex_col().child(
                                    div()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(if is_selected {
                                            DISCORD_THEME.interactive_active
                                        } else {
                                            DISCORD_THEME.text_header
                                        })
                                        .child(item.name.clone()),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(item.subtitle.clone()),
                            )
                            .children(if is_selected {
                                Some(
                                    div()
                                        .px(px(6.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(DISCORD_THEME.blurple)
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(DISCORD_THEME.interactive_active)
                                        .child("↵"),
                                )
                            } else {
                                None
                            }),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.state.select_quick_switcher_item(&item_clone);
                            cx.notify();
                        }),
                    );

                item_elements.push(item_el.into_any_element());
            }
        }

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000b3))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(90.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.state.close_quick_switcher();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .w(px(600.))
                    .bg(DISCORD_THEME.bg_tertiary)
                    .rounded(px(10.))
                    .border_1()
                    .border_color(DISCORD_THEME.border_strong)
                    .p(px(16.))
                    .v_flex()
                    .gap(px(12.))
                    .on_mouse_down(MouseButton::Left, |_, _, _| {}) // stop propagation
                    // 1. Поле ввода поиска
                    .child(
                        div()
                            .h(px(52.))
                            .px(px(16.))
                            .rounded(px(8.))
                            .bg(DISCORD_THEME.bg_primary)
                            .border_1()
                            .border_color(DISCORD_THEME.blurple)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        if query.is_empty() {
                                            div()
                                                .text_size(px(16.))
                                                .text_color(DISCORD_THEME.text_muted)
                                                .child("Куда вы хотите перейти? (*сервер, #канал, @пользователь)...")
                                                .into_any_element()
                                        } else {
                                            div()
                                                .flex()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_size(px(16.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(DISCORD_THEME.text_header)
                                                        .child(query.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(16.))
                                                        .text_color(DISCORD_THEME.blurple)
                                                        .child("|"),
                                                )
                                                .into_any_element()
                                        },
                                    ),
                            )
                            .child(
                                if !query.is_empty() {
                                    div()
                                        .cursor_pointer()
                                        .p(px(4.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                        .text_size(px(14.))
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child("✕")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.state.quick_switcher_query.clear();
                                                this.state.quick_switcher_selected_index = 0;
                                                cx.notify();
                                            }),
                                        )
                                        .into_any_element()
                                } else {
                                    div().text_size(px(16.)).child("🔍").into_any_element()
                                },
                            ),
                    )
                    // 2. Быстрые чипы-фильтры префиксов (*, #, @, !)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .bg(if query.starts_with('*') {
                                        DISCORD_THEME.blurple
                                    } else {
                                        DISCORD_THEME.bg_secondary
                                    })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if query.starts_with('*') {
                                        DISCORD_THEME.interactive_active
                                    } else {
                                        DISCORD_THEME.text_header
                                    })
                                    .child("* Серверы")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.quick_switcher_query = "*".to_string();
                                            this.state.quick_switcher_selected_index = 0;
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .bg(if query.starts_with('#') {
                                        DISCORD_THEME.blurple
                                    } else {
                                        DISCORD_THEME.bg_secondary
                                    })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if query.starts_with('#') {
                                        DISCORD_THEME.interactive_active
                                    } else {
                                        DISCORD_THEME.text_header
                                    })
                                    .child("# Каналы")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.quick_switcher_query = "#".to_string();
                                            this.state.quick_switcher_selected_index = 0;
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .bg(if query.starts_with('@') {
                                        DISCORD_THEME.blurple
                                    } else {
                                        DISCORD_THEME.bg_secondary
                                    })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if query.starts_with('@') {
                                        DISCORD_THEME.interactive_active
                                    } else {
                                        DISCORD_THEME.text_header
                                    })
                                    .child("@ Пользователи")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.quick_switcher_query = "@".to_string();
                                            this.state.quick_switcher_selected_index = 0;
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .bg(if query.starts_with('!') {
                                        DISCORD_THEME.blurple
                                    } else {
                                        DISCORD_THEME.bg_secondary
                                    })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if query.starts_with('!') {
                                        DISCORD_THEME.interactive_active
                                    } else {
                                        DISCORD_THEME.text_header
                                    })
                                    .child("! Голосовые")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.quick_switcher_query = "!".to_string();
                                            this.state.quick_switcher_selected_index = 0;
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    )
                    // 3. Список результатов
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .max_h(px(320.))
                            .overflow_hidden()
                            .child(
                                div()
                                    .px(px(4.))
                                    .py(px(2.))
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(category_title),
                            )
                            .children(item_elements),
                    )
                    // 4. Подвал навигации
                    .child(
                        div()
                            .pt(px(10.))
                            .border_t_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("СОВЕТ: * серверы, # каналы, @ пользователи, ! голос"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("↑↓ Навигация • ENTER Выбрать • ESC Закрыть"),
                            ),
                    ),
            )
            .into_any_element()
    }
}
