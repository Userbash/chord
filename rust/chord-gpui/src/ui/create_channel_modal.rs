use crate::state::{AppState, ChannelType};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Модальное окно создания канала (Create Channel Modal)
pub struct CreateChannelModal;

impl CreateChannelModal {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_create_channel_open {
            return div().into_any_element();
        }

        let is_text = state.new_channel_type == ChannelType::GuildText;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                this.state.close_create_channel();
                cx.notify();
            }))
            .child(
                div()
                    .v_flex()
                    .w(px(440.))
                    .bg(DISCORD_THEME.bg_primary)
                    .rounded(px(8.))
                    .shadow_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .on_mouse_down(MouseButton::Left, |_e, _window, cx| {
                        cx.stop_propagation();
                    })
                    // Заголовок
                    .child(
                        div()
                            .p(px(20.))
                            .v_flex()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Создать канал"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("в категории ОБЩИЕ КАНАЛЫ"),
                            ),
                    )
                    // Выбор типа канала (Текстовый / Голосовой)
                    .child(
                        div()
                            .px(px(20.))
                            .v_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("ТИП КАНАЛА"),
                            )
                            // Текстовый канал
                            .child(
                                div()
                                    .p(px(10.))
                                    .rounded(px(6.))
                                    .bg(if is_text { DISCORD_THEME.bg_active } else { DISCORD_THEME.bg_secondary })
                                    .border_1()
                                    .border_color(if is_text { DISCORD_THEME.blurple } else { DISCORD_THEME.border_subtle })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("#"),
                                    )
                                    .child(
                                        div()
                                            .v_flex()
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(DISCORD_THEME.text_normal)
                                                    .child("Текстовый"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(DISCORD_THEME.text_muted)
                                                    .child("Публикуйте сообщения, изображения, мемы и мнения"),
                                            ),
                                    )
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.state.new_channel_type = ChannelType::GuildText;
                                        cx.notify();
                                    })),
                            )
                            // Голосовой канал
                            .child(
                                div()
                                    .p(px(10.))
                                    .rounded(px(6.))
                                    .bg(if !is_text { DISCORD_THEME.bg_active } else { DISCORD_THEME.bg_secondary })
                                    .border_1()
                                    .border_color(if !is_text { DISCORD_THEME.blurple } else { DISCORD_THEME.border_subtle })
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .text_size(px(18.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("🔊"),
                                    )
                                    .child(
                                        div()
                                            .v_flex()
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(DISCORD_THEME.text_normal)
                                                    .child("Голосовой"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(DISCORD_THEME.text_muted)
                                                    .child("Общайтесь голосом, видео и демонстрацией экрана"),
                                            ),
                                    )
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.state.new_channel_type = ChannelType::GuildVoice;
                                        cx.notify();
                                    })),
                            ),
                    )
                    // Название канала
                    .child(
                        div()
                            .p(px(20.))
                            .v_flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("НАЗВАНИЕ КАНАЛА"),
                            )
                            .child(
                                div()
                                    .h(px(38.))
                                    .w_full()
                                    .px(px(12.))
                                    .bg(DISCORD_THEME.bg_secondary)
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(DISCORD_THEME.border_subtle)
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(if is_text { "#" } else { "🔊" }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(DISCORD_THEME.text_normal)
                                            .child("новый-канал"),
                                    ),
                            ),
                    )
                    // Футер с кнопками
                    .child(
                        div()
                            .p(px(16.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .px(px(14.))
                                    .py(px(8.))
                                    .rounded(px(4.))
                                    .hover(|s| s.text_color(gpui::white()))
                                    .cursor_pointer()
                                    .text_color(DISCORD_THEME.text_muted)
                                    .text_size(px(13.))
                                    .child("Отмена")
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.state.close_create_channel();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .px(px(20.))
                                    .py(px(8.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.blurple)
                                    .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                                    .cursor_pointer()
                                    .text_color(gpui::white())
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Создать канал")
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.state.create_channel("новый-канал".into());
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
