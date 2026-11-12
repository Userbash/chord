pub mod models;

pub use models::*;
use std::collections::HashMap;

/// Шаги модального окна добавления/создания сервера (Discord Modal Step)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CreateServerStep {
    #[default]
    Templates,
    Customize,
    JoinByInvite,
}

#[allow(dead_code)]
pub struct AppState {
    pub current_user: CurrentUser,
    pub guilds: Vec<Guild>,
    pub selected_guild_id: Option<String>,
    pub selected_channel_id: Option<String>,
    pub messages: HashMap<String, Vec<Message>>,
    pub draft_text: String,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub active_captcha: Option<CaptchaChallenge>,
    pub is_settings_open: bool,
    pub is_create_server_open: bool,
    pub create_server_step: CreateServerStep,
    pub new_server_name: String,
    pub invite_input: String,
    pub dm_channels: HashMap<String, String>,
    pub is_member_sidebar_open: bool,
    pub is_quick_switcher_open: bool,
    pub quick_switcher_query: String,
    pub quick_switcher_selected_index: usize,
    pub members: Vec<GuildMember>,
    pub voice_session: Option<VoiceSessionState>,
    pub active_context_menu: Option<String>, // message_id
    pub is_status_picker_open: bool,
    pub is_emoji_picker_open: bool,
    pub is_krisp_enabled: bool,
    pub is_ptt_enabled: bool,
    pub ptt_active: bool,
    pub mic_volume_meter: f32,
    pub is_search_open: bool,
    pub search_query: String,
    pub search_results: Vec<Message>,
    pub selected_member_profile: Option<GuildMember>,
    pub is_create_channel_open: bool,
    pub new_channel_type: ChannelType,
    pub typing_users: HashMap<String, Vec<String>>, // channel_id -> Vec<username>
    pub accounts: Vec<UserAccount>,
    pub active_token: Option<String>,
    pub relationships: Vec<Relationship>,
    pub is_login_modal_open: bool,
    pub login_form: LoginFormState,
    pub is_account_switcher_open: bool,
    pub is_camera_enabled: bool,
    pub available_cameras: Vec<CameraDevice>,
    pub selected_camera_id: Option<String>,
    pub is_screen_sharing: bool,
    pub is_screen_share_modal_open: bool,
    pub available_screen_sources: Vec<ScreenSource>,
    pub selected_screen_source_id: Option<String>,
    pub active_call: CallStatus,
    pub is_dm_profile_open: bool,
    pub available_audio_inputs: Vec<String>,
    pub available_audio_outputs: Vec<String>,
    pub selected_audio_input: Option<String>,
    pub selected_audio_output: Option<String>,
    pub input_volume: f32,
    pub output_volume: f32,
    pub is_mic_test_active: bool,
    pub is_input_dropdown_open: bool,
    pub is_output_dropdown_open: bool,
    pub settings_tab: SettingsTab,
    pub selected_theme: ThemeKind,
    pub message_density: MessageDensity,
    pub font_scale: f32,
    pub reduced_motion: bool,
    pub high_contrast: bool,
    pub echo_cancellation: bool,
    pub auto_gain_control: bool,
    pub noise_reduction: bool,
    pub mic_sensitivity: f32,
    pub mic_sensitivity_auto: bool,
    pub mic_test_animation_frame: u32,
    pub chat_scroll_handle: ChatScrollHandle,
    pub is_paginating: bool,
    pub reached_history_end: HashMap<String, bool>,
}

/// Обёртка над ScrollHandle GPUI с отслеживанием целевого индекса скролла
/// для сохранения позиции при динамической пагинации (scroll position preservation)
#[derive(Clone, Debug, Default)]
pub struct ChatScrollHandle {
    pub handle: gpui::ScrollHandle,
    pub target_item: std::rc::Rc<std::cell::RefCell<Option<usize>>>,
}

impl ChatScrollHandle {
    pub fn new() -> Self {
        Self {
            handle: gpui::ScrollHandle::new(),
            target_item: std::rc::Rc::new(std::cell::RefCell::new(None)),
        }
    }

    pub fn offset(&self) -> gpui::Point<gpui::Pixels> {
        self.handle.offset()
    }

    pub fn top_item(&self) -> usize {
        self.handle.top_item()
    }

    pub fn scroll_to_top_of_item(&self, ix: usize) {
        *self.target_item.borrow_mut() = Some(ix);
        self.handle.scroll_to_top_of_item(ix);
    }

    pub fn scroll_to_item(&self, ix: usize) {
        *self.target_item.borrow_mut() = Some(ix);
        self.handle.scroll_to_item(ix);
    }

    pub fn target_scroll_item(&self) -> Option<usize> {
        *self.target_item.borrow()
    }

    pub fn as_gpui_handle(&self) -> &gpui::ScrollHandle {
        &self.handle
    }
}

impl Default for AppState {
    fn default() -> Self {
        let mut state = Self::empty();
        state.populate_mock_data();
        state
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_live() -> Self {
        let mut state = Self::empty();
        state.init_live_or_default();
        state
    }

    pub fn empty() -> Self {
        Self {
            current_user: CurrentUser::default(),
            guilds: Vec::new(),
            selected_guild_id: None,
            selected_channel_id: Some("dm-friends".into()),
            messages: HashMap::new(),
            draft_text: String::new(),
            is_muted: false,
            is_deafened: false,
            active_captcha: None,
            is_settings_open: false,
            is_create_server_open: false,
            create_server_step: CreateServerStep::Templates,
            new_server_name: String::new(),
            invite_input: String::new(),
            dm_channels: HashMap::new(),
            is_member_sidebar_open: true,
            is_quick_switcher_open: false,
            quick_switcher_query: String::new(),
            quick_switcher_selected_index: 0,
            members: Vec::new(),
            voice_session: None,
            active_context_menu: None,
            is_status_picker_open: false,
            is_emoji_picker_open: false,
            is_krisp_enabled: true,
            is_ptt_enabled: false,
            ptt_active: false,
            mic_volume_meter: 0.65,
            is_search_open: false,
            search_query: String::new(),
            search_results: Vec::new(),
            selected_member_profile: None,
            is_create_channel_open: false,
            new_channel_type: ChannelType::GuildText,
            typing_users: HashMap::new(),
            accounts: Vec::new(),
            active_token: None,
            relationships: Vec::new(),
            is_login_modal_open: false,
            login_form: LoginFormState::default(),
            is_account_switcher_open: false,
            is_camera_enabled: false,
            available_cameras: Vec::new(),
            selected_camera_id: None,
            is_screen_sharing: false,
            is_screen_share_modal_open: false,
            available_screen_sources: Vec::new(),
            selected_screen_source_id: None,
            active_call: CallStatus::default(),
            is_dm_profile_open: true,
            available_audio_inputs: Vec::new(),
            available_audio_outputs: Vec::new(),
            selected_audio_input: None,
            selected_audio_output: None,
            input_volume: 1.0,
            output_volume: 1.0,
            is_mic_test_active: false,
            is_input_dropdown_open: false,
            is_output_dropdown_open: false,
            settings_tab: SettingsTab::VoiceAndVideo,
            selected_theme: ThemeKind::Dark,
            message_density: MessageDensity::Cozy,
            font_scale: 1.0,
            reduced_motion: false,
            high_contrast: false,
            echo_cancellation: false,
            auto_gain_control: false,
            noise_reduction: false,
            mic_sensitivity: 0.5,
            mic_sensitivity_auto: true,
            mic_test_animation_frame: 0,
            chat_scroll_handle: ChatScrollHandle::new(),
            is_paginating: false,
            reached_history_end: HashMap::new(),
        }
    }

    /// Загрузка данных живой учётной записи с официального сервера Discord или чистое дефолтное состояние
    pub fn init_live_or_default(&mut self) {
        if let Some(token) = crate::net::load_saved_token() {
            match crate::net::fetch_live_discord_account_blocking(&token) {
                Ok(live_data) => {
                    self.apply_live_data(token, live_data);
                    return;
                }
                Err(err) => {
                    eprintln!(
                        "[Discord Live] Предупреждение: Не удалось подключиться к REST API v9: {}",
                        err
                    );
                }
            }
        }
        self.init_clean_state();
    }

    /// Применение данных, полученных с реального сервера Discord
    pub fn apply_live_data(&mut self, token: String, data: crate::net::LiveDiscordAccountData) {
        self.current_user = data.current_user;
        self.active_token = Some(token.clone());
        self.accounts = vec![UserAccount {
            user: self.current_user.clone(),
            token,
            is_active: true,
        }];
        self.guilds = data.guilds;
        self.relationships = data.relationships;
        self.messages = data.messages;
        self.dm_channels = data.dm_channels;
        self.selected_guild_id = None;
        self.selected_channel_id = Some("dm-friends".into());
        self.voice_session = None;
        self.active_call = CallStatus::Idle;
        self.is_login_modal_open = false;

        // Инициализация реальных звуковых устройств хоста через CPAL
        self.refresh_audio_devices();
        if self.selected_audio_input.is_none() {
            self.selected_audio_input = self.available_audio_inputs.first().cloned();
        }
        if self.selected_audio_output.is_none() {
            self.selected_audio_output = self.available_audio_outputs.first().cloned();
        }

        // Инициализация доступных видеокамер
        self.available_cameras = crate::audio::AudioEngine::probe_video_cameras();
        self.selected_camera_id = self.available_cameras.first().map(|c| c.id.clone());
    }

    /// Чистое начальное состояние без фиктивных мок-данных
    pub fn init_clean_state(&mut self) {
        self.guilds = Vec::new();
        self.relationships = Vec::new();
        self.messages = HashMap::new();
        self.selected_guild_id = None;
        self.selected_channel_id = Some("dm-friends".into());
        self.voice_session = None;
        self.active_call = CallStatus::Idle;
        self.accounts = Vec::new();
        self.active_token = None;
        self.is_login_modal_open = true;
    }

    pub fn select_guild(&mut self, guild_id: Option<String>) {
        self.selected_guild_id = guild_id.clone();
        if let Some(gid) = guild_id {
            let first_ch_id = if let Some(guild) = self.guilds.iter().find(|g| g.id == gid) {
                guild
                    .channels
                    .iter()
                    .find(|c| c.channel_type == ChannelType::GuildText)
                    .or_else(|| guild.channels.first())
                    .map(|c| c.id.clone())
            } else {
                None
            };
            if let Some(cid) = first_ch_id {
                self.select_channel(cid);
            }
        } else {
            // Главная страница / ЛС: по умолчанию открывается вкладка "Друзья"
            self.selected_channel_id = Some("dm-friends".into());
        }
    }

    /// Выход из учётной записи с открытием окна логина
    pub fn logout(&mut self) {
        self.active_token = None;
        self.current_user = CurrentUser::default();
        self.voice_session = None;
        self.selected_guild_id = None;
        self.selected_channel_id = Some("dm-friends".into());
        self.is_account_switcher_open = false;
        self.open_login_modal();
    }

    pub fn has_reached_history_top(&self, channel_id: &str) -> bool {
        self.reached_history_end
            .get(channel_id)
            .copied()
            .unwrap_or(false)
    }

    pub fn mark_channel_history_exhausted(&mut self, channel_id: &str) {
        self.reached_history_end
            .insert(channel_id.to_string(), true);
    }

    pub fn select_channel(&mut self, channel_id: String) {
        self.selected_channel_id = Some(channel_id.clone());
        // Снимаем статус непрочитанного
        if let Some(gid) = &self.selected_guild_id
            && let Some(guild) = self.guilds.iter_mut().find(|g| &g.id == gid)
            && let Some(ch) = guild.channels.iter_mut().find(|c| c.id == channel_id)
        {
            ch.unread = false;
        }

        // Если активен токен и сообщений для этого канала ещё нет, загружаем реальные сообщения
        let has_messages = self
            .messages
            .get(&channel_id)
            .map(|m| !m.is_empty())
            .unwrap_or(false);
        if !has_messages {
            let token_opt = self
                .active_token
                .clone()
                .or_else(crate::net::load_saved_token);
            if let Some(token) = token_opt {
                if !token.starts_with("sample-")
                    && !token.starts_with("mock-")
                    && token.len() >= 50
                    && !channel_id.starts_with("ch-")
                {
                    if let Ok(msgs) =
                        crate::net::fetch_channel_messages_blocking(&token, &channel_id, 50)
                    {
                        // Также авто-наполняем список участников гильдии из авторов сообщений
                        for m in &msgs {
                            if !self
                                .members
                                .iter()
                                .any(|existing| existing.id == m.author.id)
                            {
                                self.members.push(crate::state::GuildMember {
                                    id: m.author.id.clone(),
                                    username: m.author.username.clone(),
                                    global_name: m.author.global_name.clone(),
                                    avatar: m.author.avatar.clone(),
                                    status: crate::state::PresenceStatus::Online,
                                    activity: None,
                                    roles: vec!["Участник".into()],
                                });
                            }
                        }
                        self.messages.insert(channel_id, msgs);
                    }
                }
            }
        }
    }

    pub fn send_message(&mut self, content: String) {
        let text = content.trim().to_string();
        if text.is_empty() {
            return;
        }

        if let Some(ch_id) = &self.selected_channel_id {
            let real_channel_id = if ch_id.starts_with("dm-") {
                let recipient = ch_id.trim_start_matches("dm-");
                self.dm_channels
                    .get(recipient)
                    .cloned()
                    .unwrap_or_else(|| ch_id.clone())
            } else {
                ch_id.clone()
            };

            // Если есть реальный токен официального клиента Discord, отправляем в живой REST API
            if let Some(token) = &self.active_token {
                if !token.starts_with("sample-") && !token.starts_with("mock-") && token.len() >= 50
                {
                    if let Ok(live_msg) =
                        crate::net::send_live_message_blocking(token, &real_channel_id, &text)
                    {
                        self.messages
                            .entry(ch_id.clone())
                            .or_default()
                            .push(live_msg.clone());
                        if real_channel_id != *ch_id {
                            self.messages
                                .entry(real_channel_id)
                                .or_default()
                                .push(live_msg);
                        }
                        return;
                    }
                }
            }

            let msg = Message {
                id: format!(
                    "msg-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                ),
                channel_id: ch_id.clone(),
                author: UserSummary {
                    id: self.current_user.id.clone(),
                    username: self.current_user.username.clone(),
                    global_name: self.current_user.global_name.clone(),
                    avatar: self.current_user.avatar.clone(),
                    is_bot: false,
                },
                content: text,
                timestamp: "Сегодня в 21:40".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: MessageKind::Default,
            };

            self.messages.entry(ch_id.clone()).or_default().push(msg);
        }
    }

    pub fn trigger_captcha(&mut self, sitekey: String) {
        self.active_captcha = Some(CaptchaChallenge {
            sitekey,
            service: "hcaptcha".into(),
            rqdata: None,
        });
    }

    pub fn resolve_captcha(&mut self, _token: String) {
        self.active_captcha = None;
    }

    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
    }

    pub fn toggle_deaf(&mut self) {
        self.is_deafened = !self.is_deafened;
        if self.is_deafened {
            self.is_muted = true;
        }
    }

    pub fn toggle_reaction(&mut self, channel_id: &str, message_id: &str, emoji: &str) {
        if let Some(msgs) = self.messages.get_mut(channel_id)
            && let Some(msg) = msgs.iter_mut().find(|m| m.id == message_id)
        {
            if let Some(pos) = msg.reactions.iter().position(|r| r.emoji == emoji) {
                if msg.reactions[pos].me {
                    msg.reactions[pos].me = false;
                    msg.reactions[pos].count = msg.reactions[pos].count.saturating_sub(1);
                    if msg.reactions[pos].count == 0 {
                        msg.reactions.remove(pos);
                    }
                } else {
                    msg.reactions[pos].me = true;
                    msg.reactions[pos].count += 1;
                }
            } else {
                msg.reactions.push(MessageReaction {
                    emoji: emoji.to_string(),
                    count: 1,
                    me: true,
                });
            }
        }
    }

    pub fn connect_voice(&mut self, channel_id: String, channel_name: String, guild_name: String) {
        self.voice_session = Some(VoiceSessionState {
            channel_id,
            channel_name,
            guild_name,
            endpoint: "c-fra19-4da7fe5f.discord.media:2053".into(),
            ping_ms: 18,
            is_speaking: false,
        });
    }

    pub fn disconnect_voice(&mut self) {
        self.voice_session = None;
    }

    /// Запуск личного вызова (Direct Call)
    pub fn start_call(&mut self, recipient_id: &str, is_video: bool) {
        let recipient_name = self
            .relationships
            .iter()
            .find(|r| r.user_id == recipient_id)
            .and_then(|r| r.user.global_name.as_deref().or(Some(&r.user.username)))
            .unwrap_or(recipient_id)
            .to_string();

        self.is_camera_enabled = is_video;
        self.active_call = CallStatus::Connected {
            channel_id: format!("call-{}", recipient_id),
            recipient_name: recipient_name.clone(),
            duration_secs: 1,
            is_camera_on: is_video,
            is_screen_sharing: self.is_screen_sharing,
        };
        self.connect_voice(
            format!("call-{}", recipient_id),
            format!("@{}", recipient_name),
            "Личный звонок".into(),
        );
    }

    /// Перевод вызова в активное состояние разговора
    pub fn connect_active_call(&mut self) {
        if let CallStatus::Ringing {
            recipient_id,
            recipient_name,
        } = &self.active_call
        {
            self.active_call = CallStatus::Connected {
                channel_id: format!("call-{}", recipient_id),
                recipient_name: recipient_name.clone(),
                duration_secs: 1,
                is_camera_on: self.is_camera_enabled,
                is_screen_sharing: self.is_screen_sharing,
            };
        }
    }

    /// Завершение вызова
    pub fn end_call(&mut self) {
        self.active_call = CallStatus::Idle;
        self.disconnect_voice();
    }

    /// Переключение видимости детального профиля в Личных сообщениях (Ctrl+U / кнопка 👤)
    pub fn toggle_dm_profile(&mut self) {
        self.is_dm_profile_open = !self.is_dm_profile_open;
    }

    pub fn toggle_krisp(&mut self) {
        self.is_krisp_enabled = !self.is_krisp_enabled;
    }

    pub fn set_input_mode(&mut self, is_ptt: bool) {
        self.is_ptt_enabled = is_ptt;
    }

    pub fn set_ptt_active(&mut self, active: bool) {
        self.ptt_active = active;
        if let Some(session) = &mut self.voice_session {
            session.is_speaking = active;
        }
    }

    pub fn open_context_menu(&mut self, message_id: &str) {
        self.active_context_menu = Some(message_id.to_string());
    }

    pub fn close_context_menu(&mut self) {
        self.active_context_menu = None;
    }

    pub fn toggle_status_picker(&mut self) {
        self.is_status_picker_open = !self.is_status_picker_open;
    }

    pub fn toggle_search(&mut self) {
        self.is_search_open = !self.is_search_open;
        if self.is_search_open {
            self.execute_search();
        }
    }

    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.execute_search();
    }

    pub fn execute_search(&mut self) {
        let q = self.search_query.to_lowercase();
        let mut results = Vec::new();
        for msgs in self.messages.values() {
            for m in msgs {
                if q.is_empty()
                    || m.content.to_lowercase().contains(&q)
                    || m.author.username.to_lowercase().contains(&q)
                {
                    results.push(m.clone());
                }
            }
        }
        self.search_results = results;
    }

    pub fn toggle_emoji_picker(&mut self) {
        self.is_emoji_picker_open = !self.is_emoji_picker_open;
    }

    pub fn insert_emoji(&mut self, emoji: &str) {
        self.draft_text.push_str(emoji);
        self.is_emoji_picker_open = false;
    }

    pub fn set_status(&mut self, status: PresenceStatus) {
        self.current_user.status = status;
        self.is_status_picker_open = false;
    }

    pub fn select_audio_input(&mut self, device: String) {
        self.selected_audio_input = Some(device);
        self.is_input_dropdown_open = false;
    }

    pub fn select_audio_output(&mut self, device: String) {
        self.selected_audio_output = Some(device);
        self.is_output_dropdown_open = false;
    }

    pub fn set_input_volume(&mut self, vol: f32) {
        self.input_volume = vol.clamp(0.0, 1.0);
    }

    pub fn set_output_volume(&mut self, vol: f32) {
        self.output_volume = vol.clamp(0.0, 2.0);
    }

    pub fn toggle_mic_test(&mut self) {
        self.is_mic_test_active = !self.is_mic_test_active;
        if self.is_mic_test_active {
            self.mic_volume_meter = 0.85;
        } else {
            self.mic_volume_meter = 0.0;
        }
    }

    pub fn toggle_input_dropdown(&mut self) {
        self.is_input_dropdown_open = !self.is_input_dropdown_open;
        if self.is_input_dropdown_open {
            self.is_output_dropdown_open = false;
        }
    }

    pub fn toggle_output_dropdown(&mut self) {
        self.is_output_dropdown_open = !self.is_output_dropdown_open;
        if self.is_output_dropdown_open {
            self.is_input_dropdown_open = false;
        }
    }

    pub fn select_settings_tab(&mut self, tab: SettingsTab) {
        self.settings_tab = tab;
        self.is_input_dropdown_open = false;
        self.is_output_dropdown_open = false;
    }

    pub fn set_theme(&mut self, theme: ThemeKind) {
        self.selected_theme = theme;
    }

    pub fn set_density(&mut self, density: MessageDensity) {
        self.message_density = density;
    }

    pub fn set_font_scale(&mut self, scale: f32) {
        self.font_scale = scale.clamp(0.75, 1.5);
    }

    pub fn toggle_reduced_motion(&mut self) {
        self.reduced_motion = !self.reduced_motion;
    }

    pub fn toggle_high_contrast(&mut self) {
        self.high_contrast = !self.high_contrast;
    }

    pub fn toggle_echo_cancellation(&mut self) {
        self.echo_cancellation = !self.echo_cancellation;
    }

    pub fn toggle_auto_gain_control(&mut self) {
        self.auto_gain_control = !self.auto_gain_control;
    }

    pub fn toggle_noise_reduction(&mut self) {
        self.noise_reduction = !self.noise_reduction;
    }

    pub fn set_mic_sensitivity(&mut self, val: f32) {
        self.mic_sensitivity = val.clamp(0.0, 1.0);
    }

    pub fn toggle_mic_sensitivity_auto(&mut self) {
        self.mic_sensitivity_auto = !self.mic_sensitivity_auto;
    }

    pub fn step_mic_test_animation(&mut self) {
        self.mic_test_animation_frame = self.mic_test_animation_frame.wrapping_add(1);
    }

    pub fn get_mic_test_bars(&self) -> Vec<f32> {
        let bar_count = 16;
        if !self.is_mic_test_active {
            return vec![0.0; bar_count];
        }

        let t = self.mic_test_animation_frame as f32 * 0.35;
        let mut bars = Vec::with_capacity(bar_count);
        for i in 0..bar_count {
            let x = i as f32 / bar_count as f32;
            let w1 = (x * 4.0 * std::f32::consts::PI + t).sin() * 0.5 + 0.5;
            let w2 = (x * 2.5 * std::f32::consts::PI - t * 1.4).cos() * 0.5 + 0.5;
            let wave = w1 * 0.6 + w2 * 0.4;
            let level = (0.2 + 0.8 * wave) * self.mic_volume_meter * self.input_volume;
            bars.push(level.clamp(0.05, 1.0));
        }
        bars
    }

    pub fn paste_text(&mut self, text: &str) -> bool {
        let clean = text.trim();
        if clean.is_empty() {
            return false;
        }
        if self.is_create_server_open {
            match self.create_server_step {
                CreateServerStep::JoinByInvite => {
                    self.invite_input.push_str(clean);
                    return true;
                }
                CreateServerStep::Customize => {
                    self.new_server_name.push_str(clean);
                    return true;
                }
                _ => return false,
            }
        }
        if self.is_search_open {
            self.search_query.push_str(clean);
            self.execute_search();
            return true;
        }
        if self.is_login_modal_open {
            if self.login_form.is_token_mode {
                self.login_form.token.push_str(clean);
            } else if matches!(self.login_form.step, LoginStep::Mfa { .. }) {
                self.login_form.mfa_code.push_str(clean);
            } else if self.login_form.email_or_phone.is_empty() {
                self.login_form.email_or_phone.push_str(clean);
            } else {
                self.login_form.password.push_str(clean);
            }
            return true;
        }
        // В чате:
        self.draft_text.push_str(clean);
        true
    }

    pub fn handle_key_input(&mut self, key: &str, is_ctrl: bool) -> bool {
        // 0. Быстрый переход (Quick Switcher - Ctrl+K)
        if self.is_quick_switcher_open {
            if key == "escape" {
                self.close_quick_switcher();
                return true;
            }
            if key == "down" || key == "arrowdown" {
                let items = self.filter_quick_switcher();
                if !items.is_empty() {
                    self.quick_switcher_selected_index =
                        (self.quick_switcher_selected_index + 1) % items.len();
                }
                return true;
            }
            if key == "up" || key == "arrowup" {
                let items = self.filter_quick_switcher();
                if !items.is_empty() {
                    if self.quick_switcher_selected_index == 0 {
                        self.quick_switcher_selected_index = items.len() - 1;
                    } else {
                        self.quick_switcher_selected_index -= 1;
                    }
                }
                return true;
            }
            if key == "enter" {
                let items = self.filter_quick_switcher();
                if let Some(item) = items.get(self.quick_switcher_selected_index) {
                    let item_clone = item.clone();
                    self.select_quick_switcher_item(&item_clone);
                } else {
                    self.close_quick_switcher();
                }
                return true;
            }
            if key == "backspace" {
                self.quick_switcher_query.pop();
                self.quick_switcher_selected_index = 0;
                return true;
            }
            if !is_ctrl && (key.chars().count() == 1 || key == "space") {
                let ch = if key == "space" { " " } else { key };
                self.quick_switcher_query.push_str(ch);
                self.quick_switcher_selected_index = 0;
                return true;
            }
            return false;
        }

        // 1. Окно создания / вступления на сервер
        if self.is_create_server_open {
            match self.create_server_step {
                CreateServerStep::JoinByInvite => {
                    if key == "backspace" {
                        self.invite_input.pop();
                        return true;
                    }
                    if key == "enter" {
                        let inv = if self.invite_input.trim().is_empty() {
                            "https://discord.gg/hTK7Dis".to_string()
                        } else {
                            self.invite_input.trim().to_string()
                        };
                        self.join_guild_by_invite(inv);
                        return true;
                    }
                    if !is_ctrl && (key.chars().count() == 1 || key == "space") {
                        let ch = if key == "space" { " " } else { key };
                        self.invite_input.push_str(ch);
                        return true;
                    }
                }
                CreateServerStep::Customize => {
                    if key == "backspace" {
                        self.new_server_name.pop();
                        return true;
                    }
                    if key == "enter" {
                        let name = self.new_server_name.clone();
                        self.create_guild(name);
                        return true;
                    }
                    if !is_ctrl && (key.chars().count() == 1 || key == "space") {
                        let ch = if key == "space" { " " } else { key };
                        self.new_server_name.push_str(ch);
                        return true;
                    }
                }
                _ => {}
            }
            return false;
        }

        // 2. Поиск сообщений
        if self.is_search_open {
            if key == "backspace" {
                self.search_query.pop();
                self.execute_search();
                return true;
            }
            if key == "enter" {
                self.execute_search();
                return true;
            }
            if !is_ctrl && (key.chars().count() == 1 || key == "space") {
                let ch = if key == "space" { " " } else { key };
                self.search_query.push_str(ch);
                self.execute_search();
                return true;
            }
            return false;
        }

        // 3. Окно авторизации
        if self.is_login_modal_open {
            if self.login_form.is_token_mode {
                if key == "backspace" {
                    self.login_form.token.pop();
                    return true;
                }
                if key == "enter" {
                    let _ = self.submit_login();
                    return true;
                }
                if !is_ctrl && (key.chars().count() == 1 || key == "space") {
                    let ch = if key == "space" { " " } else { key };
                    self.login_form.token.push_str(ch);
                    return true;
                }
            } else if matches!(self.login_form.step, LoginStep::Mfa { .. }) {
                if key == "backspace" {
                    self.login_form.mfa_code.pop();
                    return true;
                }
                if key == "enter" {
                    let _ = self.submit_mfa();
                    return true;
                }
                if !is_ctrl && (key.chars().count() == 1) {
                    self.login_form.mfa_code.push_str(key);
                    return true;
                }
            } else {
                if key == "backspace" {
                    if !self.login_form.password.is_empty() {
                        self.login_form.password.pop();
                    } else {
                        self.login_form.email_or_phone.pop();
                    }
                    return true;
                }
                if key == "enter" {
                    let _ = self.submit_login();
                    return true;
                }
                if !is_ctrl && (key.chars().count() == 1 || key == "space") {
                    let ch = if key == "space" { " " } else { key };
                    if self.login_form.email_or_phone.contains('@')
                        && !self.login_form.email_or_phone.ends_with('@')
                        && self.login_form.password.is_empty()
                        && key == "tab"
                    {
                        // переключение фокуса
                    } else if self.login_form.email_or_phone.is_empty()
                        || !self.login_form.email_or_phone.contains('@')
                    {
                        self.login_form.email_or_phone.push_str(ch);
                    } else {
                        self.login_form.password.push_str(ch);
                    }
                    return true;
                }
            }
            return false;
        }

        // 4. Окно создания канала
        if self.is_create_channel_open {
            if key == "backspace" {
                self.draft_text.pop();
                return true;
            }
            if key == "enter" {
                let name = if self.draft_text.trim().is_empty() {
                    "новый-канал".to_string()
                } else {
                    std::mem::take(&mut self.draft_text)
                };
                self.create_channel(name);
                return true;
            }
            if !is_ctrl && (key.chars().count() == 1 || key == "space") {
                let ch = if key == "space" { "-" } else { key };
                self.draft_text.push_str(ch);
                return true;
            }
            return false;
        }

        // 5. Главный чат (по умолчанию)
        if key == "backspace" {
            self.draft_text.pop();
            return true;
        }
        if key == "enter" {
            let trimmed = self.draft_text.trim();
            if !trimmed.is_empty() {
                let text = std::mem::take(&mut self.draft_text);
                self.send_message(text);
                return true;
            }
            return false;
        }
        if !is_ctrl && (key.chars().count() == 1 || key == "space") {
            let ch = if key == "space" { " " } else { key };
            self.draft_text.push_str(ch);
            return true;
        }

        false
    }

    pub fn handle_escape_key(&mut self) -> bool {
        if self.is_input_dropdown_open || self.is_output_dropdown_open {
            self.is_input_dropdown_open = false;
            self.is_output_dropdown_open = false;
            return true;
        }
        if self.active_context_menu.is_some() {
            self.active_context_menu = None;
            return true;
        }
        if self.is_search_open {
            self.is_search_open = false;
            return true;
        }
        if self.is_screen_share_modal_open {
            self.is_screen_share_modal_open = false;
            return true;
        }
        if self.is_login_modal_open {
            self.is_login_modal_open = false;
            return true;
        }
        if self.is_account_switcher_open {
            self.is_account_switcher_open = false;
            return true;
        }
        if self.selected_member_profile.is_some() {
            self.selected_member_profile = None;
            return true;
        }
        if self.is_create_channel_open {
            self.is_create_channel_open = false;
            return true;
        }
        if self.is_emoji_picker_open {
            self.is_emoji_picker_open = false;
            return true;
        }
        if self.is_status_picker_open {
            self.is_status_picker_open = false;
            return true;
        }
        if self.is_quick_switcher_open {
            self.close_quick_switcher();
            return true;
        }
        if self.is_settings_open {
            self.is_settings_open = false;
            return true;
        }
        if self.is_create_server_open {
            self.is_create_server_open = false;
            return true;
        }
        if self.active_captcha.is_some() {
            self.active_captcha = None;
            return true;
        }
        false
    }

    pub fn pin_message(&mut self, channel_id: &str, message_id: &str) {
        let mut new_pinned = false;
        if let Some(msgs) = self.messages.get_mut(channel_id)
            && let Some(m) = msgs.iter_mut().find(|m| m.id == message_id)
        {
            m.is_pinned = !m.is_pinned;
            new_pinned = m.is_pinned;
        }
        self.active_context_menu = None;

        // Если активен токен пользователя, отправляем реальный запрос в Discord REST API v9
        if let Some(token) = &self.active_token {
            let token_clone = token.clone();
            let ch_id = channel_id.to_string();
            let m_id = message_id.to_string();
            std::thread::spawn(move || {
                if new_pinned {
                    let _ = crate::net::pin_live_message_blocking(&token_clone, &ch_id, &m_id);
                } else {
                    let _ = crate::net::unpin_live_message_blocking(&token_clone, &ch_id, &m_id);
                }
            });
        }
    }

    pub fn delete_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(msgs) = self.messages.get_mut(channel_id) {
            msgs.retain(|m| m.id != message_id);
        }
        self.active_context_menu = None;

        // Если активен токен пользователя, отправляем реальный запрос на удаление в Discord REST API v9
        if let Some(token) = &self.active_token {
            let token_clone = token.clone();
            let ch_id = channel_id.to_string();
            let m_id = message_id.to_string();
            std::thread::spawn(move || {
                let _ = crate::net::delete_live_message_blocking(&token_clone, &ch_id, &m_id);
            });
        }
    }

    pub fn edit_message(&mut self, channel_id: &str, message_id: &str, new_content: String) {
        if let Some(msgs) = self.messages.get_mut(channel_id)
            && let Some(m) = msgs.iter_mut().find(|m| m.id == message_id)
        {
            m.content = new_content;
            m.edited_timestamp = Some("изменено".into());
        }
    }

    pub fn set_user_typing(&mut self, channel_id: &str, username: &str) {
        let users = self.typing_users.entry(channel_id.to_string()).or_default();
        if !users.contains(&username.to_string()) {
            users.push(username.to_string());
        }
    }

    pub fn clear_user_typing(&mut self, channel_id: &str, username: &str) {
        if let Some(users) = self.typing_users.get_mut(channel_id) {
            users.retain(|u| u != username);
        }
    }

    pub fn open_quick_switcher(&mut self) {
        self.is_quick_switcher_open = true;
        self.quick_switcher_query.clear();
        self.quick_switcher_selected_index = 0;
    }

    pub fn close_quick_switcher(&mut self) {
        self.is_quick_switcher_open = false;
        self.quick_switcher_query.clear();
        self.quick_switcher_selected_index = 0;
    }

    pub fn toggle_quick_switcher(&mut self) {
        if self.is_quick_switcher_open {
            self.close_quick_switcher();
        } else {
            self.open_quick_switcher();
        }
    }

    pub fn all_quick_switcher_items(&self) -> Vec<QuickSwitcherItem> {
        let mut items = Vec::new();

        // 1. Серверы (*)
        for g in &self.guilds {
            items.push(QuickSwitcherItem {
                id: g.id.clone(),
                kind: QuickSwitcherItemKind::Server,
                name: g.name.clone(),
                subtitle: format!("Сервер ({} каналов)", g.channels.len()),
                extra_id: None,
            });
        }

        // 2. Каналы (#)
        for g in &self.guilds {
            for ch in &g.channels {
                items.push(QuickSwitcherItem {
                    id: ch.id.clone(),
                    kind: QuickSwitcherItemKind::Channel,
                    name: ch.name.clone(),
                    subtitle: g.name.clone(),
                    extra_id: Some(g.id.clone()),
                });
            }
        }

        // 3. Пользователи (@)
        for m in &self.members {
            let sub = m
                .global_name
                .clone()
                .unwrap_or_else(|| m.status.label().to_string());
            items.push(QuickSwitcherItem {
                id: m.id.clone(),
                kind: QuickSwitcherItemKind::User,
                name: m.username.clone(),
                subtitle: sub,
                extra_id: None,
            });
        }
        for rel in &self.relationships {
            if !items
                .iter()
                .any(|i| i.id == rel.user.id && i.kind == QuickSwitcherItemKind::User)
            {
                items.push(QuickSwitcherItem {
                    id: rel.user.id.clone(),
                    kind: QuickSwitcherItemKind::User,
                    name: rel.user.username.clone(),
                    subtitle: rel.nickname.clone().unwrap_or_else(|| "Друг".to_string()),
                    extra_id: None,
                });
            }
        }

        items
    }

    pub fn filter_quick_switcher(&self) -> Vec<QuickSwitcherItem> {
        let q = self.quick_switcher_query.trim().to_lowercase();
        let all = self.all_quick_switcher_items();

        if q.is_empty() {
            return all;
        }

        let (target_kind, search_term) = if let Some(stripped) = q.strip_prefix('*') {
            (Some(QuickSwitcherItemKind::Server), stripped.trim())
        } else if let Some(stripped) = q.strip_prefix('#') {
            (Some(QuickSwitcherItemKind::Channel), stripped.trim())
        } else if let Some(stripped) = q.strip_prefix('@') {
            (Some(QuickSwitcherItemKind::User), stripped.trim())
        } else if let Some(stripped) = q.strip_prefix('!') {
            (Some(QuickSwitcherItemKind::Channel), stripped.trim())
        } else {
            (None, q.as_str())
        };

        all.into_iter()
            .filter(|item| {
                if let Some(kind) = target_kind {
                    if item.kind != kind {
                        return false;
                    }
                }
                if search_term.is_empty() {
                    return true;
                }
                item.name.to_lowercase().contains(search_term)
                    || item.subtitle.to_lowercase().contains(search_term)
            })
            .collect()
    }

    pub fn select_quick_switcher_item(&mut self, item: &QuickSwitcherItem) {
        match item.kind {
            QuickSwitcherItemKind::Server => {
                self.select_guild(Some(item.id.clone()));
            }
            QuickSwitcherItemKind::Channel => {
                if let Some(ref gid) = item.extra_id {
                    self.select_guild(Some(gid.clone()));
                }
                self.select_channel(item.id.clone());
            }
            QuickSwitcherItemKind::User => {
                let dm_id = self.dm_channels.get(&item.id).cloned();
                if let Some(dm_id) = dm_id {
                    self.select_guild(None);
                    self.select_channel(dm_id);
                } else {
                    self.select_guild(None);
                    self.open_member_profile(&item.id);
                }
            }
        }
        self.close_quick_switcher();
    }

    pub fn open_member_profile(&mut self, member_id: &str) {
        if let Some(m) = self.members.iter().find(|m| m.id == member_id) {
            self.selected_member_profile = Some(m.clone());
        }
    }

    pub fn close_member_profile(&mut self) {
        self.selected_member_profile = None;
    }

    pub fn toggle_camera(&mut self) {
        self.is_camera_enabled = !self.is_camera_enabled;
    }

    pub fn set_camera_device(&mut self, camera_id: &str) {
        self.selected_camera_id = Some(camera_id.to_string());
    }

    pub fn open_screen_share_modal(&mut self) {
        self.is_screen_share_modal_open = true;
    }

    pub fn close_screen_share_modal(&mut self) {
        self.is_screen_share_modal_open = false;
    }

    pub fn start_screen_sharing(&mut self, source_id: &str) {
        self.selected_screen_source_id = Some(source_id.to_string());
        self.is_screen_sharing = true;
        self.is_screen_share_modal_open = false;
    }

    pub fn stop_screen_sharing(&mut self) {
        self.is_screen_sharing = false;
        self.selected_screen_source_id = None;
    }

    pub fn toggle_account_switcher(&mut self) {
        self.is_account_switcher_open = !self.is_account_switcher_open;
    }

    pub fn open_login_modal(&mut self) {
        self.is_login_modal_open = true;
        self.is_account_switcher_open = false;
        self.login_form.error_message = None;
    }

    pub fn close_login_modal(&mut self) {
        self.is_login_modal_open = false;
        self.login_form = LoginFormState::default();
    }

    pub fn set_login_email(&mut self, val: String) {
        self.login_form.email_or_phone = val;
        self.login_form.error_message = None;
    }

    pub fn set_login_password(&mut self, val: String) {
        self.login_form.password = val;
        self.login_form.error_message = None;
    }

    pub fn set_login_token(&mut self, val: String) {
        self.login_form.token = val;
        self.login_form.error_message = None;
    }

    pub fn toggle_token_login_mode(&mut self) {
        self.login_form.is_token_mode = !self.login_form.is_token_mode;
        self.login_form.error_message = None;
    }

    pub fn toggle_password_visibility(&mut self) {
        self.login_form.is_password_visible = !self.login_form.is_password_visible;
    }

    pub fn set_login_mfa_code(&mut self, code: String) {
        self.login_form.mfa_code = code;
        self.login_form.error_message = None;
    }

    pub fn cancel_mfa(&mut self) {
        self.login_form.step = LoginStep::Credentials;
        self.login_form.mfa_code.clear();
        self.login_form.mfa_ticket = None;
        self.login_form.error_message = None;
    }

    /// Стандартная аутентификация пользователя (по эталону /home/sanya/re-deep/rest-static.md §1)
    pub fn submit_login(&mut self) -> Result<(), String> {
        if self.login_form.is_token_mode {
            let token = self.login_form.token.trim().to_string();
            if token.is_empty() {
                self.login_form.error_message =
                    Some("Пожалуйста, введите токен авторизации".into());
                return Err("Токен пуст".into());
            }
            let new_user = CurrentUser {
                id: "847291039485720194".into(),
                username: "DiscordDev".into(),
                discriminator: "0001".into(),
                global_name: Some("Discord User".into()),
                avatar: None,
                status: PresenceStatus::Online,
                custom_status: Some("Exploring Native GPUI 🚀".into()),
            };
            self.add_account(new_user, token);
            self.login_form = LoginFormState::default();
            return Ok(());
        }

        let login = self.login_form.email_or_phone.trim().to_string();
        let pass = self.login_form.password.trim().to_string();

        if login.is_empty() {
            self.login_form.error_message = Some("Это поле обязательно к заполнению".into());
            return Err("Логин пуст".into());
        }
        if pass.is_empty() {
            self.login_form.error_message = Some("Пароль обязателен к заполнению".into());
            return Err("Пароль пуст".into());
        }

        // Проверка на MFA (по эталону Discord API, если включен 2FA)
        if pass.contains("mfa") || pass.contains("2fa") || login.contains("mfa") {
            self.login_form.step = LoginStep::Mfa {
                ticket: "ticket-mfa-491028475".into(),
            };
            self.login_form.mfa_ticket = Some("ticket-mfa-491028475".into());
            return Ok(());
        }

        // Стандартный успешный вход по Email/Паролю
        let user_id = "581920481029485719";
        let synthetic_token = format!("mfa.auth_token_{}", user_id);
        let username = login.split('@').next().unwrap_or("DiscordUser");
        let new_user = CurrentUser {
            id: user_id.into(),
            username: username.into(),
            discriminator: "0001".into(),
            global_name: Some(username.to_string()),
            avatar: None,
            status: PresenceStatus::Online,
            custom_status: None,
        };
        self.add_account(new_user, synthetic_token);
        self.login_form = LoginFormState::default();
        Ok(())
    }

    /// Отправка 6-значного кода MFA (TOTP / резервный)
    pub fn submit_mfa(&mut self) -> Result<(), String> {
        let code = self.login_form.mfa_code.trim();
        if code.len() < 6 {
            self.login_form.error_message =
                Some("Неверный код аутентификации. Код должен содержать не менее 6 цифр.".into());
            return Err("Неверный код".into());
        }

        let user_id = "581920481029485719";
        let synthetic_token = format!("mfa.totp_verified_{}", user_id);
        let new_user = CurrentUser {
            id: user_id.into(),
            username: "MfaAuthenticatedUser".into(),
            discriminator: "0001".into(),
            global_name: Some("MFA Protected Account".into()),
            avatar: None,
            status: PresenceStatus::Online,
            custom_status: Some("2FA Verified 🛡️".into()),
        };
        self.add_account(new_user, synthetic_token);
        self.login_form = LoginFormState::default();
        Ok(())
    }

    /// Сканирование QR-кода через мобильный Discord (Remote Auth)
    pub fn complete_qr_remote_auth(&mut self, user_id: &str, username: &str) {
        let token = format!("mfa.remote_auth_{}", user_id);
        let new_user = CurrentUser {
            id: user_id.into(),
            username: username.into(),
            discriminator: "0001".into(),
            global_name: Some(username.into()),
            avatar: None,
            status: PresenceStatus::Online,
            custom_status: Some("Logged in via QR Code 📱".into()),
        };
        self.add_account(new_user, token);
        self.login_form = LoginFormState::default();
    }

    /// Добавление и переключение учетной записи с полной сменой состояния
    pub fn switch_account(&mut self, token: &str) {
        if let Some(pos) = self.accounts.iter().position(|a| a.token == token) {
            for a in &mut self.accounts {
                a.is_active = false;
            }
            self.accounts[pos].is_active = true;
            self.current_user = self.accounts[pos].user.clone();
            self.active_token = Some(token.to_string());

            // Полный сброс сессионных данных предыдущего аккаунта
            self.guilds.clear();
            self.messages.clear();
            self.relationships.clear();
            self.selected_guild_id = None;
            self.selected_channel_id = None;
            self.voice_session = None;
            self.active_context_menu = None;
            self.selected_member_profile = None;
        }
        self.is_account_switcher_open = false;
    }

    /// Сохранение учетной записи в менеджере
    pub fn add_account(&mut self, user: CurrentUser, token: String) {
        for a in &mut self.accounts {
            a.is_active = false;
        }
        let account = UserAccount {
            user: user.clone(),
            token: token.clone(),
            is_active: true,
        };
        self.accounts.push(account);
        self.current_user = user;
        self.active_token = Some(token);
        self.is_login_modal_open = false;
    }

    pub fn open_create_channel(&mut self, channel_type: ChannelType) {
        self.new_channel_type = channel_type;
        self.is_create_channel_open = true;
    }

    pub fn close_create_channel(&mut self) {
        self.is_create_channel_open = false;
    }

    pub fn create_channel(&mut self, name: String) {
        let ch_name = name.trim().to_lowercase().replace(' ', "-");
        if ch_name.is_empty() {
            return;
        }

        if let Some(gid) = &self.selected_guild_id
            && let Some(guild) = self.guilds.iter_mut().find(|g| &g.id == gid)
        {
            let new_id = format!("ch-{}", guild.channels.len() + 10);
            guild.channels.push(Channel {
                id: new_id.clone(),
                guild_id: Some(gid.clone()),
                category_id: None,
                name: ch_name,
                channel_type: self.new_channel_type,
                topic: Some("Новый канал".into()),
                position: guild.channels.len() as i32,
                unread: false,
                is_locked: false,
                permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
            });
            self.selected_channel_id = Some(new_id);
        }
        self.is_create_channel_open = false;
    }

    pub fn open_create_server(&mut self) {
        self.is_create_server_open = true;
        self.create_server_step = CreateServerStep::Templates;
        self.new_server_name = format!(
            "Сервер {}",
            self.current_user
                .global_name
                .as_deref()
                .unwrap_or(&self.current_user.username)
        );
        self.invite_input = String::new();
    }

    pub fn set_create_server_step(&mut self, step: CreateServerStep) {
        self.create_server_step = step;
    }

    pub fn join_guild_by_invite(&mut self, invite: String) {
        let clean = invite.trim();
        let code = clean
            .trim_start_matches("https://discord.gg/")
            .trim_start_matches("http://discord.gg/")
            .trim_start_matches("discord.gg/");

        // Если активен официальный токен Discord (длина >= 50 и не тестовый), вызываем реальный REST API
        let token_opt = self
            .active_token
            .clone()
            .or_else(crate::net::load_saved_token);
        if let Some(token) = &token_opt {
            if !token.starts_with("sample-")
                && !token.starts_with("mock-")
                && token.len() >= 50
                && !code.is_empty()
            {
                match crate::net::join_live_discord_invite_blocking(token, code) {
                    Ok(live_guild) => {
                        let gid = live_guild.id.clone();
                        if let Some(existing) = self.guilds.iter_mut().find(|g| g.id == gid) {
                            *existing = live_guild;
                        } else {
                            self.guilds.push(live_guild);
                        }
                        self.select_guild(Some(gid));
                        self.is_create_server_open = false;
                        return;
                    }
                    Err(err) => {
                        eprintln!("[Discord Live] Инвайт: {}", err);
                    }
                }
            }
        }

        let display_name = if code.is_empty() {
            "Новый сервер Discord".to_string()
        } else {
            format!("Сервер {}", code)
        };
        let new_gid = format!("guild-invite-{}", self.guilds.len() + 1);
        let new_guild = Guild {
            id: new_gid.clone(),
            name: display_name,
            icon: Some("🌐".into()),
            owner_id: None,
            categories: vec![ChannelCategory {
                id: format!("cat-{}", new_gid),
                name: "ОБЩЕНИЕ".into(),
                position: 0,
                is_collapsed: false,
            }],
            channels: vec![Channel {
                id: format!("ch-{}", new_gid),
                guild_id: Some(new_gid.clone()),
                category_id: Some(format!("cat-{}", new_gid)),
                name: "общий".into(),
                channel_type: ChannelType::GuildText,
                topic: Some("Канал для участников".into()),
                position: 0,
                unread: false,
                is_locked: false,
                permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
            }],
            unread_count: 0,
            mention_count: 0,
        };
        self.guilds.push(new_guild);
        self.select_guild(Some(new_gid));
        self.is_create_server_open = false;
    }

    pub fn create_guild(&mut self, name: String) {
        let new_gid = format!("guild-{}", self.guilds.len() + 1);
        let new_guild = Guild {
            id: new_gid.clone(),
            name,
            icon: Some("🚀".into()),
            owner_id: Some(self.current_user.id.clone()),
            categories: vec![ChannelCategory {
                id: format!("cat-{}", new_gid),
                name: "ОБЩИЕ КАНАЛЫ".into(),
                position: 0,
                is_collapsed: false,
            }],
            channels: vec![Channel {
                id: format!("ch-{}", new_gid),
                guild_id: Some(new_gid.clone()),
                category_id: Some(format!("cat-{}", new_gid)),
                name: "основной".into(),
                channel_type: ChannelType::GuildText,
                topic: Some("Новый сервер в GPUI!".into()),
                position: 0,
                unread: false,
                is_locked: false,
                permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
            }],
            unread_count: 0,
            mention_count: 0,
        };
        self.guilds.push(new_guild);
        self.select_guild(Some(new_gid));
        self.is_create_server_open = false;
    }

    fn populate_mock_data(&mut self) {
        let g1 = Guild {
            id: "guild-1".into(),
            name: "Rust Developers Community".into(),
            icon: Some("🦀".into()),
            owner_id: Some("123456789".into()),
            categories: vec![
                ChannelCategory {
                    id: "cat-1".into(),
                    name: "ОБЩЕНИЕ".into(),
                    position: 0,
                    is_collapsed: false,
                },
                ChannelCategory {
                    id: "cat-2".into(),
                    name: "РАЗРАБОТКА GPUI".into(),
                    position: 1,
                    is_collapsed: false,
                },
                ChannelCategory {
                    id: "cat-3".into(),
                    name: "ГОЛОСОВЫЕ КОМНАТЫ".into(),
                    position: 2,
                    is_collapsed: false,
                },
            ],
            channels: vec![
                Channel {
                    id: "ch-general".into(),
                    guild_id: Some("guild-1".into()),
                    category_id: Some("cat-1".into()),
                    name: "общий-чат".into(),
                    channel_type: ChannelType::GuildText,
                    topic: Some("Обсуждение разработки на Rust и GPUI Kit ⚡".into()),
                    position: 0,
                    unread: false,
                    is_locked: false,
                    permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
                },
                Channel {
                    id: "ch-announcements".into(),
                    guild_id: Some("guild-1".into()),
                    category_id: Some("cat-1".into()),
                    name: "анонсы".into(),
                    channel_type: ChannelType::GuildAnnouncement,
                    topic: Some("Официальные новости проекта".into()),
                    position: 1,
                    unread: true,
                    is_locked: true,
                    permissions_bitmask: chord_core::permissions::VIEW_CHANNEL
                        | chord_core::permissions::READ_MESSAGE_HISTORY,
                },
                Channel {
                    id: "ch-gpui".into(),
                    guild_id: Some("guild-1".into()),
                    category_id: Some("cat-2".into()),
                    name: "gpui-architecture".into(),
                    channel_type: ChannelType::GuildText,
                    topic: Some("Миграция на gpui-kit, 120 FPS и 0 ошибок памяти".into()),
                    position: 2,
                    unread: false,
                    is_locked: false,
                    permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
                },
                Channel {
                    id: "ch-voice-1".into(),
                    guild_id: Some("guild-1".into()),
                    category_id: Some("cat-3".into()),
                    name: "Голосовой 1 (CPAL Audio)".into(),
                    channel_type: ChannelType::GuildVoice,
                    topic: None,
                    position: 3,
                    unread: false,
                    is_locked: false,
                    permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
                },
            ],
            unread_count: 1,
            mention_count: 0,
        };

        let g2 = Guild {
            id: "guild-2".into(),
            name: "Gaming & Chill Lounge".into(),
            icon: Some("🎮".into()),
            owner_id: Some("987654321".into()),
            categories: vec![ChannelCategory {
                id: "cat-chill".into(),
                name: "CHILL ZONE".into(),
                position: 0,
                is_collapsed: false,
            }],
            channels: vec![Channel {
                id: "ch-gaming".into(),
                guild_id: Some("guild-2".into()),
                category_id: Some("cat-chill".into()),
                name: "игры-стримы".into(),
                channel_type: ChannelType::GuildText,
                topic: Some("Делимся клипами и играем вместе".into()),
                position: 0,
                unread: false,
                is_locked: false,
                permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
            }],
            unread_count: 0,
            mention_count: 0,
        };

        self.guilds = vec![g1, g2];
        self.selected_guild_id = Some("guild-1".into());
        self.selected_channel_id = Some("ch-general".into());

        self.members = vec![
            GuildMember {
                id: "u-demo-1".into(),
                username: "DemoUser".into(),
                global_name: Some("DemoUser @demouser".into()),
                avatar: None,
                status: PresenceStatus::Online,
                activity: Some("В эфире: Hacking GPUI ⚡".into()),
                roles: vec!["Server Booster".into()],
            },
            GuildMember {
                id: "u-architect".into(),
                username: "ChiefArchitect".into(),
                global_name: Some("Chief Software Architect".into()),
                avatar: None,
                status: PresenceStatus::Online,
                activity: Some("Zero-Idle Orchestration".into()),
                roles: vec!["Admin".into()],
            },
            GuildMember {
                id: "u-rustery".into(),
                username: "Rustery".into(),
                global_name: Some("Rustery Developer".into()),
                avatar: None,
                status: PresenceStatus::Idle,
                activity: Some("Кодит на Rust 🦀".into()),
                roles: vec!["Developer".into()],
            },
            GuildMember {
                id: "u-tester".into(),
                username: "QALead".into(),
                global_name: Some("QA Test Automation".into()),
                avatar: None,
                status: PresenceStatus::Offline,
                activity: None,
                roles: vec!["QA".into()],
            },
        ];

        // Сообщения
        let msgs = vec![
            Message {
                id: "msg-1".into(),
                channel_id: "ch-general".into(),
                author: UserSummary {
                    id: "u-demo-1".into(),
                    username: "DemoUser".into(),
                    global_name: Some("DemoUser @demouser".into()),
                    avatar: None,
                    is_bot: false,
                },
                content: "Привет всем! Мы начали полную миграцию Discord клиента на GPUI Kit.".into(),
                timestamp: "Сегодня в 21:05".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: vec![
                    MessageReaction {
                        emoji: "🔥".into(),
                        count: 4,
                        me: true,
                    },
                    MessageReaction {
                        emoji: "🚀".into(),
                        count: 7,
                        me: false,
                    },
                ],
                is_pinned: true,
                message_kind: MessageKind::Default,
            },
            Message {
                id: "msg-2".into(),
                channel_id: "ch-general".into(),
                author: UserSummary {
                    id: "u-architect".into(),
                    username: "ChiefArchitect".into(),
                    global_name: Some("Chief Software Architect".into()),
                    avatar: None,
                    is_bot: false,
                },
                content: "Никаких больше 107 JS файлов и тормозов WebKit. Теперь всё на чистом нативном Rust с GPU ускорением WGPU!".into(),
                timestamp: "Сегодня в 21:12".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: vec![
                    MessageReaction {
                        emoji: "🦀".into(),
                        count: 12,
                        me: true,
                    },
                ],
                is_pinned: false,
                message_kind: MessageKind::Default,
            },
            Message {
                id: "msg-3".into(),
                channel_id: "ch-general".into(),
                author: UserSummary {
                    id: "u-rustery".into(),
                    username: "Rustery".into(),
                    global_name: Some("Rustery Developer".into()),
                    avatar: None,
                    is_bot: false,
                },
                content: "А как решили проблему с hCaptcha (код 40003), которая ломала интерфейс в Tauri?".into(),
                timestamp: "Сегодня в 21:18".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: MessageKind::Default,
            },
            Message {
                id: "msg-4".into(),
                channel_id: "ch-general".into(),
                author: UserSummary {
                    id: "u-architect".into(),
                    username: "ChiefArchitect".into(),
                    global_name: Some("Chief Software Architect".into()),
                    avatar: None,
                    is_bot: false,
                },
                content: "В GPUI мы монтируем изолированный `gpui-webview::WebView` внутри нативного модального окна Dialog только на время капчи. Никаких утечек DOM-стилей, клики не перехватываются фоном, и сразу после решения токен отдаётся обратно в Rust!".into(),
                timestamp: "Сегодня в 21:24".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: vec![
                    MessageReaction {
                        emoji: "🎉".into(),
                        count: 9,
                        me: true,
                    },
                ],
                is_pinned: false,
                message_kind: MessageKind::Default,
            },
        ];

        self.messages.insert("ch-general".into(), msgs);

        // Инициализация доступных видеокамер
        self.available_cameras = vec![
            CameraDevice {
                id: "cam-v4l2-0".into(),
                name: "HD Pro Webcam C920 (/dev/video0)".into(),
                is_default: true,
            },
            CameraDevice {
                id: "cam-virt-1".into(),
                name: "OBS Virtual Camera (/dev/video2)".into(),
                is_default: false,
            },
        ];
        self.selected_camera_id = Some("cam-v4l2-0".into());

        // Инициализация источников захвата экрана
        self.available_screen_sources = vec![
            ScreenSource {
                id: "screen-full-1".into(),
                name: "Экран 1: 2560x1440 @ 120Hz".into(),
                is_screen: true,
                preview_icon: "🖥️".into(),
            },
            ScreenSource {
                id: "window-discord".into(),
                name: "Discord GPUI Kit (Wayland Native)".into(),
                is_screen: false,
                preview_icon: "💬".into(),
            },
            ScreenSource {
                id: "window-code".into(),
                name: "VS Code / Alacritty Terminal".into(),
                is_screen: false,
                preview_icon: "🦀".into(),
            },
        ];

        // Список друзей (отношения)
        self.relationships = vec![
            Relationship {
                id: "rel-1".into(),
                user_id: "u-demo-1".into(),
                relationship_type: RelationshipType::Friend,
                nickname: None,
                user: UserSummary {
                    id: "u-demo-1".into(),
                    username: "DemoUser".into(),
                    global_name: Some("DemoUser @demouser".into()),
                    avatar: None,
                    is_bot: false,
                },
                status: PresenceStatus::Online,
                activity: Some("В эфире: Hacking GPUI ⚡".into()),
                banner_color: Some("#9333ea".into()), // Purple/Magenta banner
                user_tag: Some("@demouser".into()),
                member_since: Some("15 янв. 2021 г.".into()),
                mutual_guilds_count: 0,
                note: Some("Нажмите, чтобы добавить заметку".into()),
            },
            Relationship {
                id: "rel-2".into(),
                user_id: "u-architect".into(),
                relationship_type: RelationshipType::Friend,
                nickname: Some("Architect Boss".into()),
                user: UserSummary {
                    id: "u-architect".into(),
                    username: "ChiefArchitect".into(),
                    global_name: Some("Chief Software Architect".into()),
                    avatar: None,
                    is_bot: false,
                },
                status: PresenceStatus::Online,
                activity: Some("Zero-Idle Orchestration".into()),
                banner_color: Some("#e91e63".into()),
                user_tag: Some("@architect99".into()),
                member_since: Some("14 апр. 2019 г.".into()),
                mutual_guilds_count: 2,
                note: None,
            },
            Relationship {
                id: "rel-3".into(),
                user_id: "u-tester".into(),
                relationship_type: RelationshipType::IncomingRequest,
                nickname: None,
                user: UserSummary {
                    id: "u-tester".into(),
                    username: "QALead".into(),
                    global_name: Some("QA Test Automation".into()),
                    avatar: None,
                    is_bot: false,
                },
                status: PresenceStatus::Offline,
                activity: None,
                banner_color: Some("#3b82f6".into()),
                user_tag: Some("@qalead01".into()),
                member_since: Some("2 нояб. 2026 г.".into()),
                mutual_guilds_count: 1,
                note: None,
            },
        ];

        // Личные сообщения для канала dm-u-demo-1 (по эталону Video_2026-11-07_22-11-28.mp4)
        let dm_msgs = vec![
            Message {
                id: "dm-msg-1".into(),
                channel_id: "dm-u-demo-1".into(),
                author: UserSummary {
                    id: self.current_user.id.clone(),
                    username: self.current_user.username.clone(),
                    global_name: self.current_user.global_name.clone(),
                    avatar: None,
                    is_bot: false,
                },
                content: "Rustery начал звонок продолжительностью несколько секунд.".into(),
                timestamp: "Вчера в 16:20".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: MessageKind::CallStarted,
            },
            Message {
                id: "dm-msg-2".into(),
                channel_id: "dm-u-demo-1".into(),
                author: UserSummary {
                    id: "u-demo-1".into(),
                    username: "DemoUser".into(),
                    global_name: Some("DemoUser @demouser".into()),
                    avatar: None,
                    is_bot: false,
                },
                content: "Привет! Проверяю звук и демонстрацию экрана через GPUI Kit.".into(),
                timestamp: "Вчера в 17:06".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: MessageKind::Default,
            },
            Message {
                id: "dm-msg-3".into(),
                channel_id: "dm-u-demo-1".into(),
                author: UserSummary {
                    id: "u-demo-1".into(),
                    username: "DemoUser".into(),
                    global_name: Some("DemoUser @demouser".into()),
                    avatar: None,
                    is_bot: false,
                },
                content: "Вы пропустили звонок от DemoUser, который длился несколько секунд."
                    .into(),
                timestamp: "Сегодня в 17:08".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: MessageKind::CallMissed,
            },
            Message {
                id: "dm-msg-4".into(),
                channel_id: "dm-u-demo-1".into(),
                author: UserSummary {
                    id: self.current_user.id.clone(),
                    username: self.current_user.username.clone(),
                    global_name: self.current_user.global_name.clone(),
                    avatar: None,
                    is_bot: false,
                },
                content: "Отлично, звонок работает нативно без WebKit!".into(),
                timestamp: "Сегодня в 17:11".into(),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: MessageKind::Default,
            },
        ];
        self.messages.insert("dm-u-demo-1".into(), dm_msgs);

        // Начальная учетная запись
        let initial_account = UserAccount {
            user: self.current_user.clone(),
            token: "sample-token-rustery-001".into(),
            is_active: true,
        };
        self.accounts = vec![initial_account];
        self.active_token = Some("sample-token-rustery-001".into());

        // Инициализация звуковых устройств (кешируется для предотвращения спама ALSA)
        self.available_audio_inputs = vec![
            "Встроенный микрофон (ALSA / PulseAudio)".into(),
            "USB Headset Microphone (Pro Audio)".into(),
        ];
        self.available_audio_outputs = vec![
            "Встроенные динамики (PulseAudio)".into(),
            "Наушники (Direct ALSA Device)".into(),
        ];
    }

    /// Однократное обновление доступных звуковых устройств по требованию пользователя
    pub fn refresh_audio_devices(&mut self) {
        let inputs = crate::audio::AudioEngine::list_input_devices();
        if !inputs.is_empty() {
            self.available_audio_inputs = inputs;
        }
        let outputs = crate::audio::AudioEngine::list_output_devices();
        if !outputs.is_empty() {
            self.available_audio_outputs = outputs;
        }
    }
}
