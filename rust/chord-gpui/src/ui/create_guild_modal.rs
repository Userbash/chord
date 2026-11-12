use crate::state::{AppState, CreateServerStep};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Модальное окно добавления/создания сервера (Discord Create / Join Server Modal)
/// Точная копия поведения официального клиента Discord
pub struct CreateGuildModal;

impl CreateGuildModal {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        if !state.is_create_server_open {
            return div().into_any_element();
        }

        div()
            .id("modal-create-guild-overlay")
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000b3))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.state.is_create_server_open = false;
                    cx.notify();
                }),
            )
            // Контейнер модального окна
            .child(
                div()
                    .w(px(460.))
                    .bg(DISCORD_THEME.bg_primary)
                    .rounded(px(16.))
                    .border_1()
                    .border_color(DISCORD_THEME.border_strong)
                    .overflow_hidden()
                    .v_flex()
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(match state.create_server_step {
                        CreateServerStep::Templates => {
                            Self::render_templates_step(state, cx).into_any_element()
                        }
                        CreateServerStep::Customize => {
                            Self::render_customize_step(state, cx).into_any_element()
                        }
                        CreateServerStep::JoinByInvite => {
                            Self::render_join_step(state, cx).into_any_element()
                        }
                    }),
            )
            .into_any_element()
    }

    /// Шаг 1: Выбор шаблона сервера или переход к вступлению по приглашению
    fn render_templates_step(_state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        div()
            .p(px(24.))
            .v_flex()
            .gap(px(16.))
            // 1. Верхний заголовок и крестик закрытия
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Создайте свой сервер"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(
                                        "Ваш сервер — это место, где вы можете общаться с друзьями и сообществом. Создайте свой собственный и начните общаться.",
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(16.))
                            .text_color(DISCORD_THEME.text_muted)
                            .hover(|s| s.text_color(DISCORD_THEME.text_header))
                            .child("✕")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.is_create_server_open = false;
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            // 2. Список шаблонов и быстрого присоединения
            .child(
                div()
                    .v_flex()
                    .gap(px(8.))
                    // Быстрое вступление по ссылке / коду приглашения
                    .child(
                        div()
                            .id("btn-quick-join-by-link")
                            .h(px(48.))
                            .px(px(16.))
                            .rounded(px(8.))
                            .bg(DISCORD_THEME.bg_secondary)
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(div().text_size(px(20.)).child("🔗"))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.status_online)
                                            .child("Присоединиться по ссылке или коду"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("→"),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.set_create_server_step(CreateServerStep::JoinByInvite);
                                    cx.notify();
                                }),
                            ),
                    )
                    // Шаблон 1: Свой шаблон
                    .child(Self::render_template_row(
                        "✨",
                        "Свой шаблон",
                        "Сервер сообщества",
                        cx,
                    ))
                    // Шаблон 2: Для меня и друзей
                    .child(Self::render_template_row(
                        "👥",
                        "Для меня и друзей",
                        "Сервер для друзей",
                        cx,
                    ))
                    // Шаблон 3: Клуб или сообщество
                    .child(Self::render_template_row(
                        "🎮",
                        "Игры и стримы",
                        "Игровое сообщество",
                        cx,
                    )),
            )
            // 3. Нижний баннер с кнопкой «Присоединиться к серверу»
            .child(
                div()
                    .mt(px(8.))
                    .p(px(16.))
                    .bg(DISCORD_THEME.bg_secondary_alt)
                    .rounded(px(8.))
                    .v_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child("У вас уже есть приглашение?"),
                    )
                    .child(
                        div()
                            .id("btn-open-join-server")
                            .w_full()
                            .h(px(38.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_hover)
                            .hover(|s| s.bg(DISCORD_THEME.bg_active))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("Присоединиться к серверу")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.set_create_server_step(CreateServerStep::JoinByInvite);
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }

    /// Шаг 2: Персонализация (Название и подтверждение создания)
    fn render_customize_step(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        let has_custom = !state.new_server_name.is_empty();
        let name_display = if has_custom {
            state.new_server_name.clone()
        } else {
            format!("Сервер {}", state.current_user.username)
        };

        div()
            .p(px(24.))
            .v_flex()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Персонализируйте свой сервер"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(
                                        "Задайте новому серверу имя и значок. Вы всегда сможете изменить их позже.",
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(16.))
                            .text_color(DISCORD_THEME.text_muted)
                            .hover(|s| s.text_color(DISCORD_THEME.text_header))
                            .child("✕")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.is_create_server_open = false;
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            // Поле ввода названия сервера
            .child(
                div()
                    .v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("НАЗВАНИЕ СЕРВЕРА"),
                    )
                    .child(
                        div()
                            .h(px(40.))
                            .px(px(12.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_tertiary)
                            .border_1()
                            .border_color(DISCORD_THEME.border_subtle)
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(if has_custom {
                                        DISCORD_THEME.text_header
                                    } else {
                                        DISCORD_THEME.text_muted
                                    })
                                    .child(name_display),
                            )
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(DISCORD_THEME.blurple)
                                    .child(" |"),
                            ),
                    ),
            )
            // Футер с кнопками Назад и Создать
            .child(
                div()
                    .pt(px(12.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .cursor_pointer()
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .px(px(14.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("Назад")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.set_create_server_step(CreateServerStep::Templates);
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-confirm-create-guild")
                            .px(px(24.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.blurple)
                            .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("Создать")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    let name = this.state.new_server_name.clone();
                                    this.state.create_guild(name);
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }

    /// Шаг 3: Вступление на сервер по коду или ссылке приглашения
    fn render_join_step(state: &AppState, cx: &mut Context<AppLayout>) -> impl IntoElement {
        let has_input = !state.invite_input.is_empty();
        let placeholder_text = "Например: https://discord.gg/hTK7Dis или rust".to_string();

        div()
            .p(px(24.))
            .v_flex()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .child("Присоединиться к серверу"),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(
                                        "Введите ссылку или код приглашения, чтобы присоединиться к существующему серверу.",
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(16.))
                            .text_color(DISCORD_THEME.text_muted)
                            .hover(|s| s.text_color(DISCORD_THEME.text_header))
                            .child("✕")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.is_create_server_open = false;
                                    cx.notify();
                                }),
                            ),
                    ),
            )
            // Поле ссылки приглашения
            .child(
                div()
                    .v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("ССЫЛКА ДЛЯ ПРИГЛАШЕНИЯ"),
                    )
                    .child(
                        div()
                            .id("input-invite-field")
                            .h(px(42.))
                            .px(px(12.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.bg_tertiary)
                            .border_1()
                            .border_color(DISCORD_THEME.blurple)
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .flex()
                                    .items_center()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .overflow_hidden()
                                            .text_color(if has_input {
                                                DISCORD_THEME.text_header
                                            } else {
                                                DISCORD_THEME.text_muted
                                            })
                                            .child(if has_input {
                                                state.invite_input.clone()
                                            } else {
                                                placeholder_text
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(DISCORD_THEME.blurple)
                                            .child("|"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .children(if has_input {
                                        Some(
                                            div()
                                                .cursor_pointer()
                                                .px(px(6.))
                                                .py(px(2.))
                                                .rounded(px(4.))
                                                .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                                .text_size(px(12.))
                                                .text_color(DISCORD_THEME.text_muted)
                                                .child("✕")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.state.invite_input.clear();
                                                        cx.notify();
                                                    }),
                                                ),
                                        )
                                    } else {
                                        None
                                    })
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .px(px(8.))
                                            .py(px(4.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.bg_secondary)
                                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui::white())
                                            .child("📋 Вставить")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    if let Some(item) = cx.read_from_clipboard() {
                                                        if let Some(text) = item.text() {
                                                            this.state.invite_input = text.trim().to_string();
                                                            cx.notify();
                                                        }
                                                    }
                                                }),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("Примеры:"),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_secondary)
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(11.))
                                    .text_color(gpui::white())
                                    .child("rust")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.invite_input = "rust".to_string();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_secondary)
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(11.))
                                    .text_color(gpui::white())
                                    .child("midjourney")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.invite_input = "midjourney".to_string();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .cursor_pointer()
                                    .px(px(6.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_secondary)
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .text_size(px(11.))
                                    .text_color(gpui::white())
                                    .child("📋 Вставить")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            if let Some(item) = cx.read_from_clipboard() {
                                                if let Some(text) = item.text() {
                                                    this.state.invite_input = text.trim().to_string();
                                                    cx.notify();
                                                }
                                            }
                                        }),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Приглашения выглядят так: hTK7Dis или https://discord.gg/hTK7Dis"),
                    ),
            )
            // Футер с кнопками Назад и Вступить
            .child(
                div()
                    .pt(px(12.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .cursor_pointer()
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .px(px(14.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("Назад")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.set_create_server_step(CreateServerStep::Templates);
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-confirm-join-guild")
                            .px(px(24.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .bg(DISCORD_THEME.blurple)
                            .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("Присоединиться к серверу")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    let inv = if this.state.invite_input.trim().is_empty() {
                                        "https://discord.gg/hTK7Dis".to_string()
                                    } else {
                                        this.state.invite_input.trim().to_string()
                                    };
                                    this.state.join_guild_by_invite(inv);
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }

    /// Строка шаблона в списке
    fn render_template_row(
        emoji: &'static str,
        title: &'static str,
        new_name: &'static str,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let name_str = new_name.to_string();
        div()
            .h(px(52.))
            .px(px(16.))
            .rounded(px(8.))
            .bg(DISCORD_THEME.bg_secondary)
            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_size(px(20.)).child(emoji))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child(title),
                    ),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child("›"),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.state.new_server_name = name_str.clone();
                    this.state
                        .set_create_server_step(CreateServerStep::Customize);
                    cx.notify();
                }),
            )
    }
}
