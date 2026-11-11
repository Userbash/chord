//! `SUB-908-1`: Чек-листы приемочного тестирования (Smoke & Sanity Checklists)
//! - Шаг 1.1: Авторизация (пароль, QR-код, капча 40003, сохранение токена, выход)
//! - Шаг 1.2: Чат (текст, эмодзи, вложения до 25 МБ, pin, edit, delete, quote)
//! - Шаг 1.3: Голос (подключение, mute/deafen, переключение устройств, Push-to-Talk, громкость)
//! - Шаг 1.4: Шаринг экрана (экраны, окна, пресеты 720p/1080p, горячее переключение в стриме)

use serde::{Deserialize, Serialize};

use crate::manual_qa::{ManualQaError, mask_secret_tokens};
use crate::state::{
    AppState, CaptchaChallenge, LoginFormState, LoginStep, Message, MessageAttachment, MessageKind,
    MessageReaction, UserSummary, VoiceSessionState,
};
use crate::ui::screen_share_modal::{ScreenShareFps, ScreenShareResolution, ScreenShareTab};

pub const DISCORD_CODE_40003_CAPTCHA_RATE_LIMIT: u32 = 40003;
pub const MAX_MESSAGE_CHARS: usize = 2000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024; // 26_214_400 байт
pub const MAX_VOICE_VOLUME_PERCENT: u32 = 200;

/// Входное описание вложения при проверке чек-листа отправки сообщений
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentPayload {
    pub filename: String,
    pub size_bytes: u64,
}

/// Согласованные параметры видеопотока при проверке чек-листа демонстрации экрана
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedScreenShareParams {
    pub source_id: String,
    pub is_screen: bool,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// Сводный отчет прохождения Smoke & Sanity чек-листов (`SUB-908-1`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmokeSanitySuiteReport {
    pub auth_passed: bool,
    pub chat_passed: bool,
    pub voice_passed: bool,
    pub screen_share_passed: bool,
}

/// Шаг 1.1: Валидатор чек-листа авторизации
#[derive(Debug, Clone, Default)]
pub struct AuthChecklistValidator;

impl AuthChecklistValidator {
    pub const fn new() -> Self {
        Self
    }

    pub fn verify_password_login(
        &self,
        state: &mut AppState,
        email_or_phone: &str,
        password: &str,
    ) -> Result<(), ManualQaError> {
        let email = email_or_phone.trim();
        if email.is_empty() || !email.contains('@') {
            return Err(ManualQaError::InvalidCredentials(
                "email_or_phone must contain '@'",
            ));
        }
        if password.len() < 8 {
            return Err(ManualQaError::InvalidCredentials(
                "password must be at least 8 chars",
            ));
        }
        state.is_login_modal_open = true;
        state.login_form.email_or_phone = email.to_string();
        state.login_form.password = password.to_string();
        state.login_form.step = LoginStep::Mfa {
            ticket: "mfa-ticket-908".into(),
        };
        state.login_form.error_message = None;
        Ok(())
    }

    pub fn verify_qr_code_flow(
        &self,
        state: &mut AppState,
        qr_fingerprint: &str,
    ) -> Result<(), ManualQaError> {
        if !qr_fingerprint.starts_with("discord-remote-auth-") || qr_fingerprint.len() < 24 {
            return Err(ManualQaError::InvalidQrFingerprint(
                qr_fingerprint.to_string(),
            ));
        }
        state.login_form.qr_fingerprint = qr_fingerprint.to_string();
        Ok(())
    }

    pub fn verify_captcha_and_code_40003(
        &self,
        state: &mut AppState,
        discord_error_code: u32,
        sitekey: &str,
        service: &str,
        rqdata: Option<&str>,
    ) -> Result<(), ManualQaError> {
        if discord_error_code != DISCORD_CODE_40003_CAPTCHA_RATE_LIMIT {
            return Err(ManualQaError::UnexpectedCaptchaErrorCode(
                discord_error_code,
            ));
        }
        if sitekey.trim().is_empty() {
            return Err(ManualQaError::InvalidCaptchaSitekey);
        }
        state.active_captcha = Some(CaptchaChallenge {
            sitekey: sitekey.to_string(),
            service: service.to_string(),
            rqdata: rqdata.map(ToOwned::to_owned),
        });
        Ok(())
    }

    pub fn verify_token_persistence(
        &self,
        state: &mut AppState,
        token: &str,
    ) -> Result<String, ManualQaError> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(ManualQaError::EmptyToken);
        }
        state.active_token = Some(trimmed.to_string());
        state.is_login_modal_open = false;
        Ok(mask_secret_tokens(trimmed))
    }

    pub fn verify_logout(&self, state: &mut AppState) -> Result<(), ManualQaError> {
        state.active_token = None;
        state.active_captcha = None;
        state.login_form = LoginFormState::default();
        state.is_login_modal_open = true;
        Ok(())
    }
}

/// Шаг 1.2: Валидатор чек-листа чата
#[derive(Debug, Clone, Default)]
pub struct ChatChecklistValidator;

impl ChatChecklistValidator {
    pub const fn new() -> Self {
        Self
    }

    pub fn verify_send_message(
        &self,
        state: &mut AppState,
        channel_id: &str,
        content: &str,
        attachments: &[AttachmentPayload],
    ) -> Result<String, ManualQaError> {
        self.validate_payload(content, attachments)?;
        let list = state.messages.entry(channel_id.to_string()).or_default();
        let next_id = format!("msg-{}", list.len() + 1);
        let mapped_attachments = attachments
            .iter()
            .enumerate()
            .map(|(idx, att)| MessageAttachment {
                id: format!("att-{next_id}-{idx}"),
                filename: att.filename.clone(),
                url: format!(
                    "https://cdn.discordapp.com/attachments/{channel_id}/{}",
                    att.filename
                ),
                size_bytes: att.size_bytes,
            })
            .collect();

        list.push(Message {
            id: next_id.clone(),
            channel_id: channel_id.to_string(),
            author: UserSummary {
                id: state.current_user.id.clone(),
                username: state.current_user.username.clone(),
                global_name: state.current_user.global_name.clone(),
                avatar: state.current_user.avatar.clone(),
                is_bot: false,
            },
            content: content.to_string(),
            timestamp: "12:00".into(),
            edited_timestamp: None,
            attachments: mapped_attachments,
            reactions: Vec::new(),
            is_pinned: false,
            message_kind: MessageKind::Default,
        });
        Ok(next_id)
    }

    fn validate_payload(
        &self,
        content: &str,
        attachments: &[AttachmentPayload],
    ) -> Result<(), ManualQaError> {
        if content.trim().is_empty() && attachments.is_empty() {
            return Err(ManualQaError::EmptyMessagePayload);
        }
        let char_count = content.chars().count();
        if char_count > MAX_MESSAGE_CHARS {
            return Err(ManualQaError::MessageTooLong {
                char_count,
                max_chars: MAX_MESSAGE_CHARS,
            });
        }
        if attachments.len() > MAX_ATTACHMENTS_COUNT {
            return Err(ManualQaError::TooManyAttachments {
                count: attachments.len(),
                max_count: MAX_ATTACHMENTS_COUNT,
            });
        }
        for att in attachments {
            if att.size_bytes > MAX_ATTACHMENT_BYTES {
                return Err(ManualQaError::PayloadTooLarge413 {
                    size_bytes: att.size_bytes,
                    max_bytes: MAX_ATTACHMENT_BYTES,
                });
            }
        }
        Ok(())
    }

    pub fn verify_emoji_reaction(
        &self,
        state: &mut AppState,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<(), ManualQaError> {
        let msg = find_message_mut(state, channel_id, message_id)?;
        if let Some(existing) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
            existing.count += 1;
            existing.me = true;
        } else {
            msg.reactions.push(MessageReaction {
                emoji: emoji.to_string(),
                count: 1,
                me: true,
            });
        }
        Ok(())
    }

    pub fn verify_pin_message(
        &self,
        state: &mut AppState,
        channel_id: &str,
        message_id: &str,
        pinned: bool,
    ) -> Result<(), ManualQaError> {
        let msg = find_message_mut(state, channel_id, message_id)?;
        msg.is_pinned = pinned;
        Ok(())
    }

    pub fn verify_edit_message(
        &self,
        state: &mut AppState,
        channel_id: &str,
        message_id: &str,
        new_content: &str,
    ) -> Result<(), ManualQaError> {
        self.validate_payload(new_content, &[])?;
        let msg = find_message_mut(state, channel_id, message_id)?;
        msg.content = new_content.to_string();
        msg.edited_timestamp = Some("12:01 (изменено)".into());
        Ok(())
    }

    pub fn verify_quote_reply(
        &self,
        state: &mut AppState,
        channel_id: &str,
        target_message_id: &str,
        reply_text: &str,
    ) -> Result<String, ManualQaError> {
        let quoted_body = {
            let target = find_message_mut(state, channel_id, target_message_id)?;
            target.content.clone()
        };
        let combined = format!("> {quoted_body}\n{reply_text}");
        self.verify_send_message(state, channel_id, &combined, &[])
    }

    pub fn verify_delete_message(
        &self,
        state: &mut AppState,
        channel_id: &str,
        message_id: &str,
    ) -> Result<(), ManualQaError> {
        let list = state
            .messages
            .get_mut(channel_id)
            .ok_or_else(|| ManualQaError::ChannelNotFound(channel_id.to_string()))?;
        let before = list.len();
        list.retain(|m| m.id != message_id);
        if list.len() == before {
            return Err(ManualQaError::MessageNotFound(message_id.to_string()));
        }
        Ok(())
    }
}

fn find_message_mut<'a>(
    state: &'a mut AppState,
    channel_id: &str,
    message_id: &str,
) -> Result<&'a mut Message, ManualQaError> {
    let list = state
        .messages
        .get_mut(channel_id)
        .ok_or_else(|| ManualQaError::ChannelNotFound(channel_id.to_string()))?;
    list.iter_mut()
        .find(|m| m.id == message_id)
        .ok_or_else(|| ManualQaError::MessageNotFound(message_id.to_string()))
}

/// Шаг 1.3: Валидатор чек-листа голоса
#[derive(Debug, Clone, Default)]
pub struct VoiceChecklistValidator;

impl VoiceChecklistValidator {
    pub const fn new() -> Self {
        Self
    }

    pub fn verify_voice_connect(
        &self,
        state: &mut AppState,
        channel_id: &str,
        channel_name: &str,
        guild_name: &str,
        ping_ms: u32,
    ) -> Result<(), ManualQaError> {
        state.voice_session = Some(VoiceSessionState {
            channel_id: channel_id.to_string(),
            channel_name: channel_name.to_string(),
            guild_name: guild_name.to_string(),
            endpoint: "finland1024.discord.media:443".into(),
            ping_ms,
            is_speaking: false,
        });
        Ok(())
    }

    pub fn verify_mute_deafen_transition(
        &self,
        state: &mut AppState,
        muted: bool,
        deafened: bool,
    ) -> Result<(), ManualQaError> {
        if state.voice_session.is_none() {
            return Err(ManualQaError::VoiceSessionNotConnected);
        }
        state.is_deafened = deafened;
        state.is_muted = if deafened { true } else { muted };
        Ok(())
    }

    pub fn verify_audio_device_switch(
        &self,
        state: &mut AppState,
        input_device: &str,
        output_device: &str,
    ) -> Result<(), ManualQaError> {
        if !state
            .available_audio_inputs
            .iter()
            .any(|d| d == input_device)
        {
            return Err(ManualQaError::AudioDeviceUnavailable(
                input_device.to_string(),
            ));
        }
        if !state
            .available_audio_outputs
            .iter()
            .any(|d| d == output_device)
        {
            return Err(ManualQaError::AudioDeviceUnavailable(
                output_device.to_string(),
            ));
        }
        state.selected_audio_input = Some(input_device.to_string());
        state.selected_audio_output = Some(output_device.to_string());
        Ok(())
    }

    pub fn verify_audio_device_hotplug_fallback(
        &self,
        state: &mut AppState,
        removed_device: &str,
    ) -> Result<(), ManualQaError> {
        state.available_audio_inputs.retain(|d| d != removed_device);
        state.selected_audio_input = Some("Default".into());
        Ok(())
    }

    pub fn verify_push_to_talk(
        &self,
        state: &mut AppState,
        ptt_enabled: bool,
        key_pressed: bool,
    ) -> Result<(), ManualQaError> {
        let session = state
            .voice_session
            .as_mut()
            .ok_or(ManualQaError::VoiceSessionNotConnected)?;
        state.is_ptt_enabled = ptt_enabled;
        if ptt_enabled && key_pressed {
            state.is_deafened = false;
            state.is_muted = false;
        }
        state.ptt_active = ptt_enabled && key_pressed && !state.is_muted;
        session.is_speaking = state.ptt_active;
        Ok(())
    }

    pub fn verify_volume_bounds(
        &self,
        state: &mut AppState,
        input_percent: u32,
        output_percent: u32,
    ) -> Result<(), ManualQaError> {
        if input_percent > MAX_VOICE_VOLUME_PERCENT {
            return Err(ManualQaError::VolumeOutOfBounds {
                requested_percent: input_percent,
                max_percent: MAX_VOICE_VOLUME_PERCENT,
            });
        }
        if output_percent > MAX_VOICE_VOLUME_PERCENT {
            return Err(ManualQaError::VolumeOutOfBounds {
                requested_percent: output_percent,
                max_percent: MAX_VOICE_VOLUME_PERCENT,
            });
        }
        state.input_volume = (input_percent as f32) / 100.0;
        state.output_volume = (output_percent as f32) / 100.0;
        Ok(())
    }
}

/// Шаг 1.4: Валидатор чек-листа демонстрации экрана
#[derive(Debug, Clone, Default)]
pub struct ScreenShareChecklistValidator;

impl ScreenShareChecklistValidator {
    pub const fn new() -> Self {
        Self
    }

    pub fn verify_start_stream(
        &self,
        state: &mut AppState,
        tab: ScreenShareTab,
        source_id: &str,
        resolution: ScreenShareResolution,
        fps: ScreenShareFps,
    ) -> Result<NegotiatedScreenShareParams, ManualQaError> {
        if state.voice_session.is_none() {
            return Err(ManualQaError::VoiceSessionNotConnected);
        }
        self.apply_stream_selection(state, tab, source_id, resolution, fps)
    }

    pub fn verify_in_stream_source_and_preset_switch(
        &self,
        state: &mut AppState,
        tab: ScreenShareTab,
        new_source_id: &str,
        resolution: ScreenShareResolution,
        fps: ScreenShareFps,
    ) -> Result<NegotiatedScreenShareParams, ManualQaError> {
        if !state.is_screen_sharing {
            return Err(ManualQaError::ScreenShareNotActive);
        }
        self.apply_stream_selection(state, tab, new_source_id, resolution, fps)
    }

    pub fn verify_stop_stream(&self, state: &mut AppState) -> Result<(), ManualQaError> {
        if !state.is_screen_sharing {
            return Err(ManualQaError::ScreenShareNotActive);
        }
        state.is_screen_sharing = false;
        state.selected_screen_source_id = None;
        state.is_screen_share_modal_open = false;
        Ok(())
    }

    fn apply_stream_selection(
        &self,
        state: &mut AppState,
        tab: ScreenShareTab,
        source_id: &str,
        resolution: ScreenShareResolution,
        fps: ScreenShareFps,
    ) -> Result<NegotiatedScreenShareParams, ManualQaError> {
        let source = state
            .available_screen_sources
            .iter()
            .find(|s| s.id == source_id && tab.matches_source(s))
            .ok_or_else(|| ManualQaError::ScreenSourceNotFound(source_id.to_string()))?;
        let is_screen = source.is_screen;
        let (width, height) = resolution.dimensions();
        state.selected_screen_source_id = Some(source_id.to_string());
        state.is_screen_sharing = true;
        state.is_screen_share_modal_open = false;
        Ok(NegotiatedScreenShareParams {
            source_id: source_id.to_string(),
            is_screen,
            width,
            height,
            fps: fps.value(),
        })
    }
}
