use crate::state::{AppState, ChannelType, CurrentUser, Guild, PresenceStatus};
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use crate::ui::voice_panel::VoicePanel;
use gpui::*;
use gpui_kit::component::StyledExt;

/// Боковая панель каналов и личных сообщений (Discord Channel Sidebar - 240px)
/// Выполнена в точном соответствии со стилем официального клиента Discord
pub struct ChannelSidebar;

impl ChannelSidebar {
    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let selected_guild = state
            .selected_guild_id
            .as_ref()
            .and_then(|gid| state.guilds.iter().find(|g| &g.id == gid));
        let selected_channel_id = state.selected_channel_id.clone();
        let current_user = state.current_user.clone();
        let is_muted = state.is_muted;
        let is_deafened = state.is_deafened;

        div()
            .v_flex()
            .w(px(240.))
            .h_full()
            .bg(DISCORD_THEME.bg_secondary)
            .border_r_1()
            .border_color(DISCORD_THEME.border_subtle)
            .overflow_hidden()
            // 1. Верхний заголовок сервера или кнопка быстрого поиска ЛС (48px)
            .child(Self::render_header(selected_guild, cx))
            // 2. Основная прокручиваемая область каналов или списка друзей/ЛС
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .p(px(8.))
                    .gap(px(2.))
                    .child(if let Some(guild) = selected_guild {
                        Self::render_guild_channels(guild, selected_channel_id.as_deref(), cx)
                            .into_any_element()
                    } else {
                        Self::render_dm_navigation(state, selected_channel_id.as_deref(), cx)
                            .into_any_element()
                    }),
            )
            // 3. Нижний контейнер (VoicePanel + Панель профиля с кнопкой выхода)
            // Использует flex_none, что гарантирует 100% видимость и отсутствие сдвигов
            .child(
                div()
                    .v_flex()
                    .flex_none()
                    .w_full()
                    .bg(DISCORD_THEME.bg_secondary_alt)
                    .children(if state.voice_session.is_some() {
                        Some(VoicePanel::render(state, _window, cx))
                    } else {
                        None
                    })
                    .child(Self::render_user_profile_bar(
                        &current_user,
                        is_muted,
                        is_deafened,
                        cx,
                    )),
            )
    }

    /// Верхний заголовок боковой панели (Сервер или кнопка поиска ЛС)
    fn render_header(
        selected_guild: Option<&Guild>,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .h(px(48.))
            .px(px(16.))
            .border_b_1()
            .border_color(DISCORD_THEME.border_subtle)
            .flex()
            .items_center()
            .justify_between()
            .child(if let Some(guild) = selected_guild {
                // Заголовок сервера с выпадающим шевроном
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_header)
                            .child(guild.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("▼"),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.open_create_channel(ChannelType::GuildText);
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            } else {
                // Кнопка быстрого поиска в Личных сообщениях ("Найти или начать беседу")
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .h(px(28.))
                            .w_full()
                            .px(px(8.))
                            .bg(DISCORD_THEME.bg_tertiary)
                            .rounded(px(4.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("Найти или начать беседу"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("🔍"),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_quick_switcher();
                                    cx.notify();
                                }),
                            ),
                    )
                    .into_any_element()
            })
    }

    /// Навигация по разделу Личные сообщения (Друзья, Nitro, Список диалогов)
    fn render_dm_navigation(
        state: &AppState,
        selected_channel_id: Option<&str>,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let is_friends_active = selected_channel_id == Some("dm-friends");
        let pending_count = state
            .relationships
            .iter()
            .filter(|r| r.relationship_type == crate::state::RelationshipType::IncomingRequest)
            .count();

        div()
            .v_flex()
            .gap(px(2.))
            // 1. Вкладка "Друзья"
            .child(
                div()
                    .id("dm-nav-friends")
                    .h(px(42.))
                    .px(px(12.))
                    .rounded(px(4.))
                    .bg(if is_friends_active {
                        DISCORD_THEME.bg_active
                    } else {
                        gpui::transparent_black()
                    })
                    .hover(|s| {
                        if !is_friends_active {
                            s.bg(DISCORD_THEME.bg_hover)
                        } else {
                            s
                        }
                    })
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(div().text_size(px(18.)).child("👥"))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_friends_active {
                                        DISCORD_THEME.text_header
                                    } else {
                                        DISCORD_THEME.text_muted
                                    })
                                    .child("Друзья"),
                            ),
                    )
                    .children(if pending_count > 0 {
                        Some(
                            div()
                                .min_w(px(18.))
                                .h(px(18.))
                                .px(px(4.))
                                .rounded_full()
                                .bg(DISCORD_THEME.status_dnd)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(10.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(gpui::white())
                                .child(pending_count.to_string()),
                        )
                    } else {
                        None
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.select_channel("dm-friends".into());
                            cx.notify();
                        }),
                    ),
            )
            // 2. Вкладка "Nitro"
            .child(
                div()
                    .id("dm-nav-nitro")
                    .h(px(42.))
                    .px(px(12.))
                    .rounded(px(4.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_size(px(18.)).child("🚀"))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Nitro"),
                    ),
            )
            // 3. Вкладка "Магазин" (Shop - по эталону Video_2026-11-09_23-23-05.mp4)
            .child(
                div()
                    .id("dm-nav-shop")
                    .h(px(42.))
                    .px(px(12.))
                    .rounded(px(4.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_size(px(18.)).child("🏪"))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Магазин"),
                    ),
            )
            // 4. Вкладка "Задания" (Quests - по эталону Discord Linux)
            .child(
                div()
                    .id("dm-nav-quests")
                    .h(px(42.))
                    .px(px(12.))
                    .rounded(px(4.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_size(px(18.)).child("🎯"))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("Задания"),
                    ),
            )
            // 5. Заголовок раздела "ПРЯМЫЕ СООБЩЕНИЯ"
            .child(
                div()
                    .pt(px(16.))
                    .pb(px(4.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(DISCORD_THEME.text_muted)
                            .child("ПРЯМЫЕ СООБЩЕНИЯ"),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.text_muted)
                            .hover(|s| s.text_color(DISCORD_THEME.text_normal))
                            .child("+"),
                    ),
            )
            // 4. Список активных личных диалогов с друзьями
            .children(state.relationships.iter().map(|rel| {
                let channel_id = format!("dm-{}", rel.user_id);
                let is_active = selected_channel_id == Some(channel_id.as_str());
                let cid = channel_id.clone();
                let username = rel
                    .user
                    .global_name
                    .as_deref()
                    .unwrap_or(&rel.user.username);
                let status_color = match rel.status {
                    PresenceStatus::Online => DISCORD_THEME.status_online,
                    PresenceStatus::Idle => DISCORD_THEME.status_idle,
                    PresenceStatus::Dnd => DISCORD_THEME.status_dnd,
                    PresenceStatus::Offline => DISCORD_THEME.status_offline,
                };

                div()
                    .h(px(42.))
                    .px(px(8.))
                    .rounded(px(4.))
                    .bg(if is_active {
                        DISCORD_THEME.bg_active
                    } else {
                        gpui::transparent_black()
                    })
                    .hover(|s| {
                        if !is_active {
                            s.bg(DISCORD_THEME.bg_hover)
                        } else {
                            s
                        }
                    })
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            // Аватарка с индикатором статуса
                            .child(
                                div()
                                    .relative()
                                    .size(px(32.))
                                    .rounded_full()
                                    .bg(DISCORD_THEME.blurple)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(gpui::white())
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(13.))
                                    .child(
                                        rel.user
                                            .username
                                            .chars()
                                            .next()
                                            .map(|c| c.to_string())
                                            .unwrap_or_default(),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom_0()
                                            .right_0()
                                            .size(px(10.))
                                            .rounded_full()
                                            .border_2()
                                            .border_color(DISCORD_THEME.bg_secondary)
                                            .bg(status_color),
                                    ),
                            )
                            // Имя собеседника и активность
                            .child(
                                div()
                                    .v_flex()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_active {
                                                DISCORD_THEME.text_header
                                            } else {
                                                DISCORD_THEME.text_normal
                                            })
                                            .child(username.to_string()),
                                    )
                                    .children(rel.activity.as_ref().map(|act| {
                                        div()
                                            .text_size(px(11.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(act.clone())
                                    })),
                            ),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.state.select_channel(cid.clone());
                            cx.notify();
                        }),
                    )
            }))
    }

    /// Список категорий и каналов сервера
    fn render_guild_channels(
        guild: &Guild,
        selected_channel_id: Option<&str>,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        div()
            .v_flex()
            .gap(px(2.))
            .children(guild.categories.iter().flat_map(|cat| {
                let cat_channels: Vec<_> = guild
                    .channels
                    .iter()
                    .filter(|ch| ch.category_id.as_deref() == Some(&cat.id))
                    .cloned()
                    .collect();

                let mut items = Vec::new();
                // Заголовок категории (Стрелочка + Название + Кнопка добавления канала)
                items.push(
                    div()
                        .pt(px(12.))
                        .pb(px(4.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(DISCORD_THEME.text_muted)
                                .child(format!("▾ {}", cat.name)),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .hover(|s| s.text_color(gpui::white()))
                                .text_color(DISCORD_THEME.text_muted)
                                .text_size(px(14.))
                                .child("+")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _window, cx| {
                                        this.state.open_create_channel(ChannelType::GuildText);
                                        cx.notify();
                                    }),
                                ),
                        )
                        .into_any_element(),
                );

                // Список каналов в категории
                for ch in cat_channels {
                    let is_active = selected_channel_id == Some(&ch.id);
                    let ch_id = ch.id.clone();
                    let ch_name = ch.name.clone();
                    let g_name = guild.name.clone();
                    let is_locked = ch.is_locked;
                    let icon = if is_locked {
                        "🔒"
                    } else {
                        match ch.channel_type {
                            ChannelType::GuildText => "#",
                            ChannelType::GuildVoice => "🔊",
                            ChannelType::GuildAnnouncement => "📢",
                            ChannelType::Dm | ChannelType::GroupDm => "@",
                        }
                    };

                    items.push(
                        div()
                            .id(SharedString::from(format!("channel-{}", ch.id)))
                            .h(px(34.))
                            .px(px(8.))
                            .rounded(px(4.))
                            .bg(if is_active {
                                DISCORD_THEME.bg_active
                            } else {
                                gpui::transparent_black()
                            })
                            .hover(|s| {
                                if !is_active {
                                    s.bg(DISCORD_THEME.bg_hover)
                                } else {
                                    s
                                }
                            })
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child(icon),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(if is_active || ch.unread {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::MEDIUM
                                            })
                                            .text_color(if is_active {
                                                DISCORD_THEME.text_header
                                            } else if ch.unread {
                                                DISCORD_THEME.text_header
                                            } else {
                                                DISCORD_THEME.text_muted
                                            })
                                            .child(ch.name.clone()),
                                    ),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _window, cx| {
                                    if ch.channel_type == ChannelType::GuildVoice {
                                        if let Some(session) = &this.state.voice_session {
                                            if session.channel_id == ch_id {
                                                this.state.disconnect_voice();
                                                cx.notify();
                                                return;
                                            }
                                        }
                                        this.state.connect_voice(
                                            ch_id.clone(),
                                            ch_name.clone(),
                                            g_name.clone(),
                                        );
                                    } else {
                                        this.state.select_channel(ch_id.clone());
                                    }
                                    cx.notify();
                                }),
                            )
                            .into_any_element(),
                    );
                }

                items
            }))
    }

    /// Нижняя панель профиля пользователя с кнопкой выхода и регуляторами (52px)
    fn render_user_profile_bar(
        current_user: &CurrentUser,
        is_muted: bool,
        is_deafened: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let status_color = match current_user.status {
            PresenceStatus::Online => DISCORD_THEME.status_online,
            PresenceStatus::Idle => DISCORD_THEME.status_idle,
            PresenceStatus::Dnd => DISCORD_THEME.status_dnd,
            PresenceStatus::Offline => DISCORD_THEME.status_offline,
        };

        div()
            .h(px(52.))
            .flex_none()
            .w_full()
            .px(px(6.))
            .bg(DISCORD_THEME.bg_secondary_alt)
            .border_t_1()
            .border_color(DISCORD_THEME.border_subtle)
            .flex()
            .items_center()
            .justify_between()
            // Левая часть: Аватарка + Никнейм + Статус (клик открывает менеджер аккаунтов)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .p(px(2.))
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .rounded(px(4.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.state.toggle_account_switcher();
                            cx.notify();
                        }),
                    )
                    // Аватар со значком статуса
                    .child(
                        div()
                            .relative()
                            .size(px(32.))
                            .flex_none()
                            .rounded_full()
                            .bg(DISCORD_THEME.blurple)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(gpui::white())
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(14.))
                            .child(
                                current_user
                                    .username
                                    .chars()
                                    .next()
                                    .map(|c| c.to_string())
                                    .unwrap_or_default(),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .right_0()
                                    .size(px(10.))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(DISCORD_THEME.bg_secondary_alt)
                                    .bg(status_color),
                            ),
                    )
                    // Текстовая информация о пользователе (адаптивная без переноса на 3 строки)
                    .child(
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .overflow_hidden()
                                    .child(
                                        current_user
                                            .global_name
                                            .as_deref()
                                            .unwrap_or(&current_user.username)
                                            .to_string(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .overflow_hidden()
                                    .child(
                                        if current_user.discriminator == "0"
                                            || current_user.discriminator.is_empty()
                                        {
                                            format!("@{}", current_user.username)
                                        } else {
                                            format!("#{}", current_user.discriminator)
                                        },
                                    ),
                            ),
                    ),
            )
            // Правая часть: Кнопки управления (Микрофон, Наушники, Настройки, ВЫХОД)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(1.))
                    // 1. Микрофон (Mute/Unmute)
                    .child(
                        div()
                            .id("btn-mic-toggle")
                            .size(px(28.))
                            .rounded(px(4.))
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.))
                            .child(if is_muted { "🔇" } else { "🎙️" })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_mute();
                                    cx.notify();
                                }),
                            ),
                    )
                    // 2. Наушники (Deafen/Undeafen)
                    .child(
                        div()
                            .id("btn-deaf-toggle")
                            .size(px(28.))
                            .rounded(px(4.))
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.))
                            .child(if is_deafened { "🚫" } else { "🎧" })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.toggle_deaf();
                                    cx.notify();
                                }),
                            ),
                    )
                    // 3. Настройки пользователя
                    .child(
                        div()
                            .id("btn-settings")
                            .size(px(28.))
                            .rounded(px(4.))
                            .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.))
                            .child("⚙️")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.is_settings_open = !this.state.is_settings_open;
                                    cx.notify();
                                }),
                            ),
                    )
                    // 4. КНОПКА «ВЫХОД» (Log Out с открытием окна логина)
                    .child(
                        div()
                            .id("btn-logout")
                            .size(px(28.))
                            .rounded(px(4.))
                            .hover(|s| {
                                s.bg(gpui::rgba(0xDA373C33))
                                    .text_color(DISCORD_THEME.status_dnd)
                            })
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(14.))
                            .text_color(DISCORD_THEME.text_muted)
                            .child("🚪")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.state.logout();
                                    cx.notify();
                                }),
                            ),
                    ),
            )
    }
}
