use crate::state::AppState;
use crate::theme::DISCORD_THEME;
use crate::ui::AppLayout;
use gpui::*;
use gpui_kit::component::StyledExt;

pub struct ChatView;

impl ChatView {
    /// Порог смещения скролла от верхнего края для инициирования пагинации (offset < 10)
    pub const PAGINATION_SCROLL_OFFSET_THRESHOLD: f32 = 10.0;

    /// Размер порции подгружаемых сообщений при пагинации (limit = 50)
    pub const PAGINATION_LIMIT: u32 = 50;

    /// Проверка, указывает ли смещение скролла на достижение верхнего края ленты (offset < 10)
    #[must_use]
    pub fn should_paginate(offset: f32) -> bool {
        offset < Self::PAGINATION_SCROLL_OFFSET_THRESHOLD
    }

    /// Обработка события скролла ленты сообщений:
    /// если скролл приблизился к верху (offset < 10), выполняет подгрузку старых сообщений
    pub fn handle_scroll(state: &mut AppState, offset: f32) -> bool {
        if !Self::should_paginate(offset) {
            return false;
        }
        let channel_id = match state.selected_channel_id.clone() {
            Some(cid) => cid,
            None => return false,
        };
        Self::paginate_messages(state, &channel_id).is_ok_and(|count| count > 0)
    }

    /// Вызов пагинации текущего активного канала на основе текущего состояния скролла
    pub fn trigger_pagination(state: &mut AppState) -> bool {
        let offset = f32::from(state.chat_scroll_handle.offset().y).abs();
        Self::handle_scroll(state, offset)
    }

    /// Динамическая пагинация сообщений:
    /// запрашивает GET /channels/{id}/messages?before={oldest_id}&limit=50
    /// и сохраняет позицию скролла на исходном сообщении (scroll position preservation)
    pub fn paginate_messages(state: &mut AppState, channel_id: &str) -> Result<usize, String> {
        if state.is_paginating {
            return Err("Пагинация уже выполняется".into());
        }
        if state.has_reached_history_top(channel_id) {
            return Ok(0);
        }

        let messages = match state.messages.get(channel_id) {
            Some(msgs) if !msgs.is_empty() => msgs.clone(),
            _ => return Ok(0),
        };

        let oldest_msg = match messages.first() {
            Some(m) => m.clone(),
            None => return Ok(0),
        };
        let oldest_id = oldest_msg.id.clone();

        state.is_paginating = true;

        let real_channel_id = if channel_id.starts_with("dm-") {
            let recipient = channel_id.trim_start_matches("dm-");
            state
                .dm_channels
                .get(recipient)
                .cloned()
                .unwrap_or_else(|| channel_id.to_string())
        } else {
            channel_id.to_string()
        };

        let previous_top_item = state.chat_scroll_handle.top_item();

        // Запрос порции сообщений: GET /channels/{id}/messages?before={oldest_id}&limit=50
        let token_opt = state
            .active_token
            .clone()
            .or_else(crate::net::load_saved_token);
        let older_messages: Vec<crate::state::Message> = if let Some(token) = token_opt {
            if !token.starts_with("sample-")
                && !token.starts_with("mock-")
                && token.len() >= 50
                && !channel_id.starts_with("ch-")
            {
                match crate::net::fetch_channel_messages_before_blocking(
                    &token,
                    &real_channel_id,
                    &oldest_id,
                    Self::PAGINATION_LIMIT,
                ) {
                    Ok(msgs) => msgs,
                    Err(e) => {
                        state.is_paginating = false;
                        return Err(e);
                    }
                }
            } else {
                Self::generate_mock_older_messages(channel_id, &oldest_id, Self::PAGINATION_LIMIT)
            }
        } else {
            Self::generate_mock_older_messages(channel_id, &oldest_id, Self::PAGINATION_LIMIT)
        };

        if older_messages.is_empty() {
            state.mark_channel_history_exhausted(channel_id);
            state.is_paginating = false;
            return Ok(0);
        }

        let current_feed = state.messages.entry(channel_id.to_string()).or_default();
        let mut prepended = Vec::with_capacity(older_messages.len());
        let mut seen = std::collections::HashSet::new();

        for msg in older_messages {
            if seen.insert(msg.id.clone()) {
                prepended.push(msg);
            }
        }
        let prepended_count = prepended.len();

        let old_items = std::mem::take(current_feed);
        for msg in old_items {
            if seen.insert(msg.id.clone()) {
                prepended.push(msg);
            }
        }
        *current_feed = prepended;

        // Сохранение позиции скролла (Scroll Position Preservation):
        // Исходный верхний элемент (previous_top_item) теперь сместился на prepended_count позиций.
        // Фиксируем скролл на нём, чтобы вьюпорт оставался на том же месте.
        let target_item = previous_top_item + prepended_count;
        state.chat_scroll_handle.scroll_to_top_of_item(target_item);

        state.is_paginating = false;
        Ok(prepended_count)
    }

    /// Генерация исторической страницы архивных сообщений для mock/тестового режима
    fn generate_mock_older_messages(
        channel_id: &str,
        oldest_id: &str,
        limit: u32,
    ) -> Vec<crate::state::Message> {
        let count = limit.clamp(1, 50);
        let mut result = Vec::with_capacity(count as usize);
        for i in (1..=count).rev() {
            result.push(crate::state::Message {
                id: format!("{}-older-{}", oldest_id, i),
                channel_id: channel_id.to_string(),
                author: crate::state::UserSummary {
                    id: "u-pharos".into(),
                    username: "ShadowByte".into(),
                    global_name: Some("ShadowByte @pharos12510".into()),
                    avatar: None,
                    is_bot: false,
                },
                content: format!("Архивное сообщение #{i} (пагинация before={oldest_id})"),
                timestamp: "Вчера в 14:00".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: crate::state::MessageKind::Default,
            });
        }
        result
    }

    pub fn render(
        state: &AppState,
        _window: &mut Window,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let selected_channel_id = state.selected_channel_id.clone();
        let selected_guild = state
            .selected_guild_id
            .as_ref()
            .and_then(|gid| state.guilds.iter().find(|g| &g.id == gid));

        let current_channel = selected_guild.and_then(|g| {
            selected_channel_id
                .as_ref()
                .and_then(|cid| g.channels.iter().find(|c| &c.id == cid))
        });

        let (channel_name, channel_topic, is_dm) = if let Some(cid) = selected_channel_id.as_deref()
        {
            if cid.starts_with("dm-") {
                let user_id = cid.trim_start_matches("dm-");
                let friend = state.relationships.iter().find(|r| r.user_id == user_id);
                let name = friend
                    .and_then(|f| f.user.global_name.as_deref())
                    .or_else(|| friend.map(|f| f.user.username.as_str()))
                    .unwrap_or("Личный диалог");
                (
                    name.to_string(),
                    format!("Начало вашей истории личных сообщений с @{}.", name),
                    true,
                )
            } else if selected_guild.is_none() {
                let friend = state.relationships.first();
                let name = friend
                    .and_then(|f| f.user.global_name.as_deref())
                    .or_else(|| friend.map(|f| f.user.username.as_str()))
                    .unwrap_or("Личный диалог");
                (
                    name.to_string(),
                    format!("Начало вашей истории личных сообщений с @{}.", name),
                    true,
                )
            } else if let Some(ch) = current_channel {
                (
                    ch.name.clone(),
                    ch.topic
                        .clone()
                        .unwrap_or_else(|| "Добро пожаловать в начало канала!".into()),
                    false,
                )
            } else {
                (
                    "общий-чат".into(),
                    "Добро пожаловать в начало канала!".into(),
                    false,
                )
            }
        } else {
            (
                "общий-чат".into(),
                "Добро пожаловать в начало канала!".into(),
                false,
            )
        };

        let messages = selected_channel_id
            .as_ref()
            .and_then(|cid| {
                state.messages.get(cid).cloned().or_else(|| {
                    if cid.starts_with("dm-") {
                        let recipient = cid.trim_start_matches("dm-");
                        state
                            .dm_channels
                            .get(recipient)
                            .and_then(|real_id| state.messages.get(real_id).cloned())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();

        div()
            .v_flex()
            .flex_1()
            .h_full()
            .bg(DISCORD_THEME.bg_primary)
            .justify_between()
            // 1. Верхняя панель заголовка канала
            .child(
                div()
                    .h(px(48.))
                    .px(px(16.))
                    .border_b_1()
                    .border_color(DISCORD_THEME.border_subtle)
                    .flex()
                    .items_center()
                    .justify_between()
                    // Левая часть: Имя канала и тема
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .flex_1()
                            .min_w(px(0.))
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(20.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(if is_dm { "@" } else { "#" }),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_header)
                                    .whitespace_nowrap()
                                    .child(channel_name.clone()),
                            )
                            .child(
                                div()
                                    .w(px(1.))
                                    .h(px(16.))
                                    .mx(px(4.))
                                    .flex_none()
                                    .bg(DISCORD_THEME.border_subtle),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .text_size(px(13.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .overflow_hidden()
                                    .child(channel_topic),
                            ),
                    )
                    // Правая часть: Действия (Звонок аудио, Звонок видео, Закреплённые, Профиль, Поиск)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            // Кнопка аудиозвонка (📞)
                            .child(
                                div()
                                    .id("btn-start-audio-call")
                                    .cursor_pointer()
                                    .size(px(28.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(16.))
                                    .text_color(
                                        if state.active_call.is_in_call()
                                            && !state.active_call.is_video()
                                        {
                                            DISCORD_THEME.status_online
                                        } else {
                                            DISCORD_THEME.text_muted
                                        },
                                    )
                                    .child("📞")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if let Some(cid) =
                                                this.state.selected_channel_id.clone()
                                            {
                                                let user_id =
                                                    cid.trim_start_matches("dm-").to_string();
                                                this.state.start_call(&user_id, false);
                                                cx.notify();
                                            }
                                        }),
                                    ),
                            )
                            // Кнопка видеозвонка (📹)
                            .child(
                                div()
                                    .id("btn-start-video-call")
                                    .cursor_pointer()
                                    .size(px(28.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(16.))
                                    .text_color(
                                        if state.active_call.is_in_call()
                                            && state.active_call.is_video()
                                        {
                                            DISCORD_THEME.status_online
                                        } else {
                                            DISCORD_THEME.text_muted
                                        },
                                    )
                                    .child("📹")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if let Some(cid) =
                                                this.state.selected_channel_id.clone()
                                            {
                                                let user_id =
                                                    cid.trim_start_matches("dm-").to_string();
                                                this.state.start_call(&user_id, true);
                                                cx.notify();
                                            }
                                        }),
                                    ),
                            )
                            // Кнопка закреплённых сообщений (📌)
                            .child(
                                div()
                                    .cursor_pointer()
                                    .size(px(28.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(16.))
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child("📌"),
                            )
                            // Кнопка списка участников (на сервере) или профиля пользователя (в DM)
                            .child(
                                div()
                                    .id("btn-toggle-sidebar")
                                    .cursor_pointer()
                                    .size(px(28.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(16.))
                                    .text_color(if is_dm {
                                        if state.is_dm_profile_open {
                                            DISCORD_THEME.text_header
                                        } else {
                                            DISCORD_THEME.text_muted
                                        }
                                    } else {
                                        if state.is_member_sidebar_open {
                                            DISCORD_THEME.text_header
                                        } else {
                                            DISCORD_THEME.text_muted
                                        }
                                    })
                                    .child(if is_dm { "👤" } else { "👥" })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if this.state.selected_guild_id.is_none() {
                                                this.state.toggle_dm_profile();
                                            } else {
                                                this.state.is_member_sidebar_open =
                                                    !this.state.is_member_sidebar_open;
                                            }
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("btn-search-quick-switcher")
                                    .cursor_pointer()
                                    .h(px(24.))
                                    .px(px(8.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("Поиск (Ctrl+K) 🔍"),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.toggle_quick_switcher();
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(
                                div()
                                    .id("btn-search-messages")
                                    .cursor_pointer()
                                    .h(px(24.))
                                    .px(px(8.))
                                    .rounded(px(4.))
                                    .bg(DISCORD_THEME.bg_tertiary)
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(DISCORD_THEME.text_muted)
                                            .child("Поиск (Ctrl+F) 🔎"),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.state.toggle_search();
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    )
                    // Индикатор набора текста (Typing Indicator)
                    .child({
                        let typing = selected_channel_id
                            .as_ref()
                            .and_then(|cid| state.typing_users.get(cid))
                            .filter(|users| !users.is_empty());

                        if let Some(users) = typing {
                            let text = if users.len() == 1 {
                                format!("{} печатает...", users[0])
                            } else if users.len() == 2 {
                                format!("{} и {} печатают...", users[0], users[1])
                            } else {
                                "Несколько человек печатают...".to_string()
                            };

                            div()
                                .h(px(20.))
                                .px(px(16.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().size(px(6.)).rounded_full().bg(DISCORD_THEME.blurple))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child(text),
                                )
                                .into_any_element()
                        } else {
                            div().h(px(20.)).into_any_element()
                        }
                    }),
            )
            // 2. Интерактивный оверлей звонка (видео/аудио конференция в стиле Video_2026-11-07_22-11-28.mp4)
            .child(crate::ui::call_overlay::CallOverlay::render(
                state, _window, cx,
            ))
            // 3. Лента сообщений
            .child({
                let active_ch_id = selected_channel_id.clone().unwrap_or_default();
                div()
                    .id("chat-message-list")
                    .v_flex()
                    .flex_1()
                    .p(px(16.))
                    .gap(px(16.))
                    .overflow_y_scroll()
                    .track_scroll(&state.chat_scroll_handle.handle)
                    .on_scroll(cx.listener(|this, _event: &ScrollEvent, _window, cx| {
                        let offset = f32::from(this.state.chat_scroll_handle.offset().y).abs();
                        let top_item = this.state.chat_scroll_handle.top_item();
                        if offset < Self::PAGINATION_SCROLL_OFFSET_THRESHOLD || top_item < 10 {
                            if ChatView::trigger_pagination(&mut this.state) {
                                cx.notify();
                            }
                        }
                    }))
                    .children(if state.has_reached_history_top(&active_ch_id) {
                        Some(
                            div()
                                .id("chat-history-top-banner")
                                .py(px(12.))
                                .v_flex()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(px(22.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(DISCORD_THEME.text_header)
                                        .child(if is_dm {
                                            format!("Начало истории с @{}!", channel_name)
                                        } else {
                                            format!("Добро пожаловать в #{}!", channel_name)
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child("Это самое начало истории сообщений."),
                                )
                                .into_any_element(),
                        )
                    } else if state.is_paginating {
                        Some(
                            div()
                                .id("chat-pagination-spinner")
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child("Загрузка более старых сообщений... ⏳"),
                                )
                                .into_any_element(),
                        )
                    } else {
                        None
                    })
                    .children(messages.into_iter().map(|msg| {
                        let msg_id = msg.id.clone();
                        let msg_menu_id = msg.id.clone();
                        let is_pinned = msg.is_pinned;
                        let channel_id_for_msg = active_ch_id.clone();
                        let author_display_name = msg
                            .author
                            .global_name
                            .as_deref()
                            .unwrap_or(&msg.author.username)
                            .to_string();

                        let row_group = SharedString::from(format!("msg-row-{}", msg_id));
                        let is_menu_open = state.active_context_menu.as_deref() == Some(&msg_id);

                        // Специальный рендеринг системных сообщений о звонках (по эталону официального клиента)
                        match msg.message_kind {
                            crate::state::MessageKind::CallStarted => {
                                div()
                                    .id(format!("message-row-{}", msg_id))
                                    .group(row_group.clone())
                                    .relative()
                                    .flex()
                                    .items_center()
                                    .gap(px(16.))
                                    .py(px(4.))
                                    .px(px(8.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    // Зеленая иконка телефонной трубки (начало звонка)
                                    .child(
                                        div()
                                            .size(px(40.))
                                            .rounded_full()
                                            .bg(gpui::rgba(0x23A55A22))
                                            .border_1()
                                            .border_color(DISCORD_THEME.status_online)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(18.))
                                            .child("📞"),
                                    )
                                    .child(
                                        div()
                                            .v_flex()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(
                                                        div()
                                                            .text_size(px(14.))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(DISCORD_THEME.text_header)
                                                            .child(
                                                                msg.author
                                                                    .global_name
                                                                    .unwrap_or(msg.author.username),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(DISCORD_THEME.text_muted)
                                                            .child(msg.timestamp),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .text_color(DISCORD_THEME.status_online)
                                                    .child(msg.content),
                                            ),
                                    )
                                    .child(Self::render_floating_action_bar(
                                        &msg_menu_id,
                                        &channel_id_for_msg,
                                        &author_display_name,
                                        row_group,
                                        is_menu_open,
                                        cx,
                                    ))
                                    .into_any_element()
                            }
                            crate::state::MessageKind::CallMissed => {
                                div()
                                    .id(format!("message-row-{}", msg_id))
                                    .group(row_group.clone())
                                    .relative()
                                    .flex()
                                    .items_center()
                                    .gap(px(16.))
                                    .py(px(4.))
                                    .px(px(8.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    // Красная иконка пропущенного звонка
                                    .child(
                                        div()
                                            .size(px(40.))
                                            .rounded_full()
                                            .bg(gpui::rgba(0xF23F4322))
                                            .border_1()
                                            .border_color(DISCORD_THEME.status_dnd)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(18.))
                                            .child("📞❌"),
                                    )
                                    .child(
                                        div()
                                            .v_flex()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(
                                                        div()
                                                            .text_size(px(14.))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(DISCORD_THEME.text_header)
                                                            .child(
                                                                msg.author
                                                                    .global_name
                                                                    .unwrap_or(msg.author.username),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(DISCORD_THEME.text_muted)
                                                            .child(msg.timestamp),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .text_color(DISCORD_THEME.status_dnd)
                                                    .child(msg.content),
                                            ),
                                    )
                                    .child(Self::render_floating_action_bar(
                                        &msg_menu_id,
                                        &channel_id_for_msg,
                                        &author_display_name,
                                        row_group,
                                        is_menu_open,
                                        cx,
                                    ))
                                    .into_any_element()
                            }
                            crate::state::MessageKind::Default => {
                                div()
                                    .id(format!("message-row-{}", msg_id))
                                    .group(row_group.clone())
                                    .relative()
                                    .flex()
                                    .gap(px(16.))
                                    .p(px(6.))
                                    .rounded(px(4.))
                                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |this, _, _, cx| {
                                            this.state.open_context_menu(&msg_id);
                                            cx.notify();
                                        }),
                                    )
                                    // Аватар автора
                                    .child(
                                        div()
                                            .size(px(40.))
                                            .rounded_full()
                                            .bg(DISCORD_THEME.blurple)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(gpui::white())
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(16.))
                                            .child(
                                                msg.author
                                                    .username
                                                    .chars()
                                                    .next()
                                                    .map(|c| c.to_string())
                                                    .unwrap_or_default(),
                                            ),
                                    )
                                    // Тело сообщения
                                    .child(
                                        div()
                                            .v_flex()
                                            .flex_1()
                                            .gap(px(4.))
                                            // Заголовок сообщения (Имя + Время + Иконка пина)
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(
                                                        div()
                                                            .text_size(px(15.))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(DISCORD_THEME.text_header)
                                                            .child(author_display_name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(DISCORD_THEME.text_muted)
                                                            .child(msg.timestamp),
                                                    )
                                                    .children(if is_pinned {
                                                        Some(div().text_size(px(12.)).child("📌"))
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            // Текст сообщения с поддержкой Markdown и Codeblock
                                            .child(Self::render_formatted_content(&msg.content))
                                            // Реакции
                                            .child(div().flex().gap(px(6.)).pt(px(4.)).children(
                                                msg.reactions.into_iter().map(|reaction| {
                                                    div()
                                                        .px(px(6.))
                                                        .py(px(2.))
                                                        .rounded(px(4.))
                                                        .bg(if reaction.me {
                                                            DISCORD_THEME.mention_bg
                                                        } else {
                                                            DISCORD_THEME.bg_secondary
                                                        })
                                                        .border_1()
                                                        .border_color(if reaction.me {
                                                            DISCORD_THEME.blurple
                                                        } else {
                                                            DISCORD_THEME.border_subtle
                                                        })
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.))
                                                        .child(
                                                            div()
                                                                .text_size(px(14.))
                                                                .child(reaction.emoji),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(12.))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(if reaction.me {
                                                                    DISCORD_THEME.blurple
                                                                } else {
                                                                    DISCORD_THEME.text_muted
                                                                })
                                                                .child(reaction.count.to_string()),
                                                        )
                                                }),
                                            )),
                                    )
                                    // Плавающий бар быстрых действий над сообщением (Floating Action Bar on Hover)
                                    .child(Self::render_floating_action_bar(
                                        &msg_menu_id,
                                        &channel_id_for_msg,
                                        &author_display_name,
                                        row_group,
                                        is_menu_open,
                                        cx,
                                    ))
                                    .into_any_element()
                            }
                        }
                    }))
            })
            // 3. Нижнее поле ввода сообщения
            .child(
                div()
                    .p(px(16.))
                    .child(
                        div()
                            .h(px(44.))
                            .px(px(16.))
                            .rounded(px(8.))
                            .bg(DISCORD_THEME.bg_accent)
                            .flex()
                            .items_center()
                            .justify_between()
                            // Левая кнопка прикрепления файла (+)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .child(
                                        div()
                                            .size(px(24.))
                                            .rounded_full()
                                            .bg(DISCORD_THEME.text_muted)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(DISCORD_THEME.text_header))
                                            .text_color(DISCORD_THEME.bg_primary)
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(14.))
                                            .child("+"),
                                    )
                                    // Поле ввода текста / плейсхолдер
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .text_color(if state.draft_text.is_empty() {
                                                        DISCORD_THEME.text_muted
                                                    } else {
                                                        DISCORD_THEME.text_normal
                                                    })
                                                    .child(if state.draft_text.is_empty() {
                                                        if is_dm {
                                                            format!(
                                                                "Написать в @{}...",
                                                                channel_name
                                                            )
                                                        } else {
                                                            format!(
                                                                "Написать в #{}...",
                                                                channel_name
                                                            )
                                                        }
                                                    } else {
                                                        state.draft_text.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .text_color(DISCORD_THEME.blurple)
                                                    .child(" |"),
                                            ),
                                    ),
                            )
                            // Правые кнопки (Подарок, GIF, Эмодзи, Отправить)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_size(px(18.))
                                            .hover(|s| s.text_size(px(20.)))
                                            .child("🎁"),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .text_size(px(18.))
                                            .hover(|s| s.text_size(px(20.)))
                                            .child("👾"),
                                    )
                                    .child(
                                        div()
                                            .id("btn-emoji-picker")
                                            .cursor_pointer()
                                            .text_size(px(18.))
                                            .hover(|s| s.text_size(px(20.)))
                                            .child("😊")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    this.state.toggle_emoji_picker();
                                                    cx.notify();
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("btn-quick-send")
                                            .px(px(10.))
                                            .py(px(4.))
                                            .rounded(px(4.))
                                            .bg(DISCORD_THEME.blurple)
                                            .hover(|s| s.bg(DISCORD_THEME.blurple_hover))
                                            .cursor_pointer()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(gpui::white())
                                            .child("Отправить ↵")
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, _, _, cx| {
                                                    let trimmed = this.state.draft_text.trim();
                                                    if !trimmed.is_empty() {
                                                        let text = std::mem::take(
                                                            &mut this.state.draft_text,
                                                        );
                                                        this.state.send_message(text);
                                                        cx.notify();
                                                    }
                                                }),
                                            ),
                                    ),
                            ),
                    )
                    // Индикатор набора текста (Typing Indicator)
                    .child({
                        let typing = selected_channel_id
                            .as_ref()
                            .and_then(|cid| state.typing_users.get(cid))
                            .filter(|users| !users.is_empty());

                        if let Some(users) = typing {
                            let text = if users.len() == 1 {
                                format!("{} печатает...", users[0])
                            } else if users.len() == 2 {
                                format!("{} и {} печатают...", users[0], users[1])
                            } else {
                                "Несколько человек печатают...".to_string()
                            };

                            div()
                                .h(px(20.))
                                .px(px(16.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(div().size(px(6.)).rounded_full().bg(DISCORD_THEME.blurple))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(DISCORD_THEME.text_muted)
                                        .child(text),
                                )
                                .into_any_element()
                        } else {
                            div().h(px(20.)).into_any_element()
                        }
                    }),
            )
    }

    pub fn render_floating_action_bar(
        msg_id: &str,
        channel_id: &str,
        author_display_name: &str,
        row_group: SharedString,
        is_menu_open: bool,
        cx: &mut Context<AppLayout>,
    ) -> impl IntoElement {
        let cid = channel_id.to_string();
        let mid = msg_id.to_string();
        let reply_author = author_display_name.to_string();

        let mut bar = div()
            .id(format!("action-bar-{}", mid))
            .absolute()
            .top(px(-14.))
            .right(px(16.));

        if !is_menu_open {
            bar = bar.invisible().group_hover(row_group, |s| s.visible());
        }

        bar.flex()
            .items_center()
            .gap(px(2.))
            .px(px(4.))
            .py(px(2.))
            .h(px(28.))
            .rounded(px(6.))
            .bg(DISCORD_THEME.bg_secondary)
            .border_1()
            .border_color(DISCORD_THEME.border_subtle)
            .shadow_md()
            // Быстрая реакция 👍 (Quick emoji reaction)
            .child(
                div()
                    .id(format!("btn-quick-react-thumbs-{}", mid))
                    .cursor_pointer()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .text_size(px(13.))
                    .child("👍")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let cid = cid.clone();
                            let mid = mid.clone();
                            move |this, _, _, cx| {
                                this.state.toggle_reaction(&cid, &mid, "👍");
                                cx.notify();
                            }
                        }),
                    ),
            )
            // Быстрая реакция ❤️ (Quick emoji reaction)
            .child(
                div()
                    .id(format!("btn-quick-react-heart-{}", mid))
                    .cursor_pointer()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .text_size(px(13.))
                    .child("❤️")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let cid = cid.clone();
                            let mid = mid.clone();
                            move |this, _, _, cx| {
                                this.state.toggle_reaction(&cid, &mid, "❤️");
                                cx.notify();
                            }
                        }),
                    ),
            )
            // Быстрая реакция 😂 (Quick emoji reaction)
            .child(
                div()
                    .id(format!("btn-quick-react-laugh-{}", mid))
                    .cursor_pointer()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .text_size(px(13.))
                    .child("😂")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let cid = cid.clone();
                            let mid = mid.clone();
                            move |this, _, _, cx| {
                                this.state.toggle_reaction(&cid, &mid, "😂");
                                cx.notify();
                            }
                        }),
                    ),
            )
            // Кнопка добавления реакции 😀 (btn-react-{mid})
            .child(
                div()
                    .id(format!("btn-react-{}", mid))
                    .cursor_pointer()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .text_size(px(13.))
                    .child("😀")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let cid = cid.clone();
                            let mid = mid.clone();
                            move |this, _, _, cx| {
                                this.state.toggle_reaction(&cid, &mid, "👍");
                                cx.notify();
                            }
                        }),
                    ),
            )
            // Кнопка быстрого ответа 💬 (Reply button)
            .child(
                div()
                    .id(format!("btn-reply-{}", mid))
                    .cursor_pointer()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .text_size(px(13.))
                    .child("💬")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let reply_author = reply_author.clone();
                            move |this, _, _, cx| {
                                this.state.draft_text = format!("@{}: ", reply_author);
                                cx.notify();
                            }
                        }),
                    ),
            )
            // Кнопка закрепления сообщения 📌 (Pin button)
            .child(
                div()
                    .id(format!("btn-pin-{}", mid))
                    .cursor_pointer()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .text_size(px(13.))
                    .child("📌")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let cid = cid.clone();
                            let mid = mid.clone();
                            move |this, _, _, cx| {
                                this.state.pin_message(&cid, &mid);
                                cx.notify();
                            }
                        }),
                    ),
            )
            // Кнопка открытия контекстного меню ⋯ (More button)
            .child(
                div()
                    .id(format!("btn-more-{}", mid))
                    .cursor_pointer()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded(px(3.))
                    .hover(|s| s.bg(DISCORD_THEME.bg_hover))
                    .text_size(px(14.))
                    .text_color(DISCORD_THEME.text_muted)
                    .child("⋯")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let mid = mid.clone();
                            move |this, _, _, cx| {
                                this.state.open_context_menu(&mid);
                                cx.notify();
                            }
                        }),
                    ),
            )
    }

    pub fn render_formatted_content(content: &str) -> impl IntoElement {
        let mut root = div().v_flex().gap(px(4.));

        if content.contains("```") {
            let parts: Vec<&str> = content.split("```").collect();
            for (idx, part) in parts.iter().enumerate() {
                if idx % 2 == 1 {
                    let lines: Vec<&str> = part.lines().collect();
                    let (lang, code_text) =
                        if !lines.is_empty() && !lines[0].contains(' ') && !lines[0].is_empty() {
                            (lines[0], lines[1..].join("\n"))
                        } else {
                            ("text", part.to_string())
                        };

                    root = root.child(
                        div()
                            .p(px(8.))
                            .rounded(px(6.))
                            .bg(DISCORD_THEME.bg_tertiary)
                            .border_1()
                            .border_color(DISCORD_THEME.border_strong)
                            .v_flex()
                            .gap(px(4.))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(DISCORD_THEME.text_muted)
                                    .child(lang.to_uppercase()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(gpui::rgb(0x73daca))
                                    .child(code_text),
                            ),
                    );
                } else if !part.trim().is_empty() {
                    root = root.child(Self::render_plain_lines(part));
                }
            }
        } else {
            root = root.child(Self::render_plain_lines(content));
        }

        root
    }

    fn render_plain_lines(text: &str) -> impl IntoElement {
        let mut block = div().v_flex().gap(px(2.));
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("> ") {
                block = block.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            div()
                                .w(px(4.))
                                .rounded(px(2.))
                                .bg(DISCORD_THEME.border_subtle),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(DISCORD_THEME.text_muted)
                                .child(trimmed.trim_start_matches("> ").to_string()),
                        ),
                );
            } else {
                block = block.child(
                    div()
                        .text_size(px(14.))
                        .text_color(DISCORD_THEME.text_normal)
                        .child(line.to_string()),
                );
            }
        }
        block
    }
}
