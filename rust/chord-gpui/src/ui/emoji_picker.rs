//! Standard and custom emoji picker popover for message composition and reactions.

use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct EmojiPicker;

impl EmojiPicker {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_emoji_picker_open {
            return div().into_any_element();
        }

        let emojis = [
            (
                "ЧАСТО ИСПОЛЬЗУЕМЫЕ",
                vec!["😀", "🔥", "👍", "❤️", "🎉", "😂", "🦀", "🚀"],
            ),
            (
                "СМАЙЛЫ И ЭМОЦИИ",
                vec![
                    "😀", "😃", "😄", "😁", "😆", "😅", "🤣", "😂", "🙂", "🙃", "😉", "😊", "😇",
                    "🥰", "😍", "🤩",
                ],
            ),
            (
                "ПРИРОДА И ЖИВОТНЫЕ",
                vec![
                    "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮", "🐷",
                    "🐸", "🐵", "🦀",
                ],
            ),
            (
                "ЕДА И НАПИТКИ",
                vec![
                    "🍏", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🫐", "🍒", "🍑", "🍍",
                    "🍕", "🍔", "🍟",
                ],
            ),
            (
                "ОБЪЕКТЫ И СИМВОЛЫ",
                vec![
                    "💡", "📱", "💻", "⌨️", "🖥️", "🖱️", "💾", "💿", "📀", "🎥", "🎬", "📻", "🎯",
                    "✨", "⭐", "⚡",
                ],
            ),
        ];

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000020))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.state.is_emoji_picker_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(80.))
                    .right(px(270.))
                    .w(px(430.))
                    .h(px(380.))
                    .bg(DISCORD_THEME.bg_tertiary)
                    .rounded(px(8.))
                    .border_1()
                    .border_color(DISCORD_THEME.border_strong)
                    .p(px(12.))
                    .v_flex()
                    .gap(px(10.))
                    // Поле поиска эмодзи
                    .child(
                        div()
                            .h(px(36.))
                            .px(px(12.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_primary)
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("Поиск эмодзи (Ctrl+E)..."),
                            )
                            .child(div().text_size(px(14.)).child("🔍")),
                    )
                    // Категории и сетка эмодзи
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .overflow_hidden()
                            .gap(px(10.))
                            .children(emojis.into_iter().map(|(category_title, list)| {
                                div()
                                    .v_flex()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(category_title),
                                    )
                                    .child(div().flex().flex_wrap().gap(px(6.)).children(
                                        list.into_iter().map(|emoji| {
                                            let emoji_str = emoji.to_string();
                                            div()
                                                .size(px(34.))
                                                .rounded(px(6.))
                                                .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(20.))
                                                .child(emoji)
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        this.state.insert_emoji(&emoji_str);
                                                        cx.notify();
                                                    }),
                                                )
                                        }),
                                    ))
                            })),
                    ),
            )
            .into_any_element()
    }
}
