//! Драйвер голосовой сессии Discord (`VoiceSessionDriver` / `VoiceSessionRunner`) — карточка task-150 (F-VC-02).
//!
//! Связывает сигнализацию Voice Gateway v8 (`OP 0 IDENTIFY`, `OP 8 HELLO`, `OP 2 READY`,
//! `OP 1 SELECT_PROTOCOL`, `OP 4 SESSION_DESCRIPTION`, `OP 5 SPEAKING`, `OP 3 HEARTBEAT`,
//! `OP 6 HEARTBEAT_ACK`, `OP 7 RESUME`), 74-байтный обмен UDP IP Discovery
//! (`encode_discovery_request` / `decode_discovery_response`) и шифрование медиа-потока
//! `SrtpContext` (`aead_xchacha20_poly1305_rtpsize` с приоритетом над `aead_aes256_gcm_rtpsize`)
//! на сыром 32-байтном `secret_key` из `OP 4 SESSION_DESCRIPTION`.

use crate::media::rtp::{RtpError, RtpHeader, RtpSender, parse_rtp};
use crate::rtcp::{Compound, RtcpError, parse_compound};
use crate::srtp::{EncryptionMode, SrtpContext, SrtpError};
use crate::udp::{DISCOVERY_LEN, DiscoveryPacket, UdpError};
use crate::{
    DAVE_PROTOCOL_VERSION, Hello, Identify, Ready, SelectProtocol, SessionDescription, StreamSpec,
    VoiceError, VoiceFrame, VoiceOp, VoicePayload, VoiceServerUpdate, json_u64, object,
    parse_frame,
};
use discord_json::{JsonNumber, JsonValue};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{BitAnd, BitOr, BitOrAssign};

/// Алиас драйвера голосовой сессии (`VoiceSessionRunner`).
pub type VoiceSessionRunner = VoiceSessionDriver;

/// Битовая маска флагов речи для `OP 5 SPEAKING`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SpeakingFlags(u32);

impl SpeakingFlags {
    /// Тишина (передача голоса не активна).
    pub const NONE: Self = Self(0);
    /// Обычная передача микрофона (`1 << 0`).
    pub const MICROPHONE: Self = Self(1 << 0);
    /// Передача звука демонстрации экрана (`1 << 1`).
    pub const SOUND_SHARE: Self = Self(1 << 1);
    /// Алиас `SOUND_SHARE` (`1 << 1`).
    pub const SOUNDSHARE: Self = Self(1 << 1);
    /// Приоритетный говорящий (`1 << 2`).
    pub const PRIORITY: Self = Self(1 << 2);

    /// Создаёт маску из сырого `u32`.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Возвращает числовое значение битовой маски.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Проверяет, что ни один флаг не взведён.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Проверяет наличие всех битов `other` в текущей маске.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Объединение двух масок флагов.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOr for SpeakingFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for SpeakingFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SpeakingFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

/// Кадр `OP 5 SPEAKING`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Speaking {
    /// Флаги активности речи (`MICROPHONE`, `SOUND_SHARE`, `PRIORITY`).
    pub speaking: SpeakingFlags,
    /// Задержка (в мс/тиках), для исходящих кадров клиента обычно `0`.
    pub delay: u32,
    /// Аудио SSRC говорящего.
    pub ssrc: u32,
    /// Опциональный `user_id` (присутствует во входящих кадрах `OP 5` от сервера).
    pub user_id: Option<String>,
}

impl Speaking {
    /// Сериализует исходящий кадр `OP 5 SPEAKING` (`{"op":5,"d":{"speaking":...,"delay":...,"ssrc":...}}`).
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut fields = vec![
            ("speaking", json_u64(u64::from(self.speaking.bits()))),
            ("delay", json_u64(u64::from(self.delay))),
            ("ssrc", json_u64(u64::from(self.ssrc))),
        ];
        if let Some(user_id) = &self.user_id {
            fields.push(("user_id", JsonValue::String(user_id.clone())));
        }
        discord_json::to_json_string(&object(vec![("op", json_u64(5)), ("d", object(fields))]))
    }
}

/// Исходящий кадр `OP 3 HEARTBEAT` для Voice Gateway v8 (`{"op":3,"d":{"t":...,"seq_ack":...}}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceHeartbeat {
    /// Метка времени/нонс отправки (мс).
    pub t: u64,
    /// Последний полученный порядковый номер `seq` от сервера (`-1`, если ещё не было).
    pub seq_ack: i64,
}

impl VoiceHeartbeat {
    /// Сериализует исходящий кадр `OP 3 HEARTBEAT`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let payload = object(vec![
            ("t", json_u64(self.t)),
            (
                "seq_ack",
                JsonValue::Number(JsonNumber::from_i64(self.seq_ack)),
            ),
        ]);
        discord_json::to_json_string(&object(vec![("op", json_u64(3)), ("d", payload)]))
    }
}

/// Входящий кадр `OP 6 HEARTBEAT_ACK` (`{"op":6,"d":{"t":...}}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceHeartbeatAck {
    /// Эхо-нонс `t`, отправленный в `OP 3 HEARTBEAT`.
    pub t: u64,
}

/// Исходящий кадр `OP 7 RESUME` (`{"op":7,"d":{"server_id":...,"channel_id":...,"session_id":...,"token":...,"seq_ack":...}}`).
#[derive(Clone, PartialEq, Eq)]
pub struct VoiceResume {
    pub server_id: String,
    pub channel_id: String,
    pub session_id: String,
    pub token: String,
    pub seq_ack: i64,
}

impl fmt::Debug for VoiceResume {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceResume")
            .field("server_id", &"<redacted>")
            .field("channel_id", &"<redacted>")
            .field("session_id", &"<redacted>")
            .field("token", &"<redacted>")
            .field("seq_ack", &self.seq_ack)
            .finish()
    }
}

impl VoiceResume {
    /// Сериализует исходящий кадр `OP 7 RESUME`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let payload = object(vec![
            ("server_id", JsonValue::String(self.server_id.clone())),
            ("channel_id", JsonValue::String(self.channel_id.clone())),
            ("session_id", JsonValue::String(self.session_id.clone())),
            ("token", JsonValue::String(self.token.clone())),
            (
                "seq_ack",
                JsonValue::Number(JsonNumber::from_i64(self.seq_ack)),
            ),
        ]);
        discord_json::to_json_string(&object(vec![("op", json_u64(7)), ("d", payload)]))
    }
}

/// Состояние `VOICE_STATE_UPDATE` из основного шлюза Discord.
#[derive(Clone, PartialEq, Eq)]
pub struct VoiceStateUpdate {
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub user_id: String,
    pub session_id: String,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub self_video: bool,
    pub self_stream: bool,
}

impl fmt::Debug for VoiceStateUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceStateUpdate")
            .field("guild_id", &"<redacted>")
            .field("channel_id", &self.channel_id)
            .field("user_id", &"<redacted>")
            .field("session_id", &"<redacted>")
            .field("self_mute", &self.self_mute)
            .field("self_deaf", &self.self_deaf)
            .field("self_video", &self.self_video)
            .field("self_stream", &self.self_stream)
            .finish()
    }
}

impl VoiceStateUpdate {
    /// Создаёт `VoiceStateUpdate` для активного голосового канала.
    #[must_use]
    pub fn new(
        guild_id: impl Into<String>,
        channel_id: impl Into<String>,
        user_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            guild_id: Some(guild_id.into()),
            channel_id: Some(channel_id.into()),
            user_id: user_id.into(),
            session_id: session_id.into(),
            self_mute: false,
            self_deaf: false,
            self_video: false,
            self_stream: false,
        }
    }

    /// Разбирает событие `VOICE_STATE_UPDATE` из JSON.
    pub fn parse(input: &str) -> Result<Self, VoiceError> {
        let root = discord_json::parse(input)
            .map_err(|_| VoiceError::invalid("invalid voice state JSON"))?;
        let data = root.get("d").unwrap_or(&root);
        let user_id = data
            .get("user_id")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| VoiceError::invalid("missing user_id"))?
            .to_owned();
        let session_id = data
            .get("session_id")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| VoiceError::invalid("missing session_id"))?
            .to_owned();
        let guild_id = match data.get("guild_id") {
            Some(JsonValue::Null) | None => None,
            Some(val) => Some(
                val.as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| VoiceError::invalid("invalid guild_id"))?
                    .to_owned(),
            ),
        };
        let channel_id = match data.get("channel_id") {
            Some(JsonValue::Null) | None => None,
            Some(val) => Some(
                val.as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| VoiceError::invalid("invalid channel_id"))?
                    .to_owned(),
            ),
        };
        let self_mute = data
            .get("self_mute")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let self_deaf = data
            .get("self_deaf")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let self_video = data
            .get("self_video")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        let self_stream = data
            .get("self_stream")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false);
        Ok(Self {
            guild_id,
            channel_id,
            user_id,
            session_id,
            self_mute,
            self_deaf,
            self_video,
            self_stream,
        })
    }
}

/// Разобранный 74-байтный ответ UDP IP Discovery (`type = 2`, `length = 70`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryResponse {
    /// Аудио SSRC клиента, отражённый сервером.
    pub ssrc: u32,
    /// Обнаруженный внешний IP-адрес клиента (ASCII).
    pub address: String,
    /// Обнаруженный внешний UDP-порт клиента.
    pub port: u16,
}

/// Фаза жизненного цикла голосовой сессии.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceSessionPhase {
    /// Отправлен `OP 0 IDENTIFY`, ожидание `OP 8 HELLO` и `OP 2 READY`.
    Identifying,
    /// Получен `OP 2 READY`, выбран режим шифрования и сформирован 74B UDP IP Discovery запрос.
    DiscoveringUdp,
    /// Обработан 74B UDP IP Discovery ответ, отправлен `OP 1 SELECT_PROTOCOL`, ожидание `OP 4 SESSION_DESCRIPTION`.
    SelectingProtocol,
    /// Получен `OP 4 SESSION_DESCRIPTION`, инициализирован `SrtpContext` с 32B `secret_key`, медиа-канал активен.
    Established,
    /// Зафиксирован таймаут ожидания `OP 6 HEARTBEAT_ACK`.
    TimedOut,
    /// Сессия закрыта.
    Closed,
}

/// Ошибки драйвера голосовой сессии.
#[derive(Debug, PartialEq, Eq)]
pub enum VoiceSessionError {
    /// Ошибка разбора/валидации сигнализации.
    Signaling(VoiceError),
    /// Ошибка пакета UDP IP Discovery.
    Udp(UdpError),
    /// Ошибка шифрования/расшифровки SRTP/SRTCP.
    Srtp(SrtpError),
    /// Ошибка формирования/разбора заголовка RTP.
    Rtp(RtpError),
    /// Ошибка разбора пакета RTCP.
    Rtcp(RtcpError),
    /// Недопустимая фаза сессии для запрошенной операции.
    InvalidPhase {
        expected: &'static str,
        actual: VoiceSessionPhase,
    },
    /// В `VoiceStateUpdate` отсутствует `channel_id`.
    MissingChannelId,
    /// Несовпадение `guild_id` между `VoiceServerUpdate` и `VoiceStateUpdate`.
    ServerIdMismatch,
    /// Сервер не предложил ни одного поддерживаемого режима шифрования (`aead_xchacha20_poly1305_rtpsize` / `aead_aes256_gcm_rtpsize`).
    UnsupportedEncryptionMode,
    /// Несовпадение SSRC в ответе UDP IP Discovery.
    SsrcMismatch { expected: u32, actual: u32 },
    /// Некорректная длина `secret_key` (ожидалось 32 байта).
    InvalidSecretKeyLength { len: usize },
    /// Контекст SRTP ещё не инициализирован (не получен `OP 4 SESSION_DESCRIPTION`).
    SrtpNotEstablished,
    /// Интервал heartbeat ещё не известен (не получен `OP 8 HELLO`).
    HeartbeatNotConfigured,
    /// Таймаут ожидания `OP 6 HEARTBEAT_ACK` на предыдущий `OP 3 HEARTBEAT`.
    HeartbeatTimeout {
        sent_at_ms: u64,
        now_ms: u64,
        interval_ms: u64,
    },
    /// Получен `OP 6 HEARTBEAT_ACK` с неожиданным нонсом `t`.
    HeartbeatAckMismatch {
        expected_nonce: u64,
        actual_nonce: u64,
    },
}

impl fmt::Display for VoiceSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signaling(err) => write!(f, "ошибка сигнализации: {err}"),
            Self::Udp(err) => write!(f, "ошибка UDP discovery: {err}"),
            Self::Srtp(err) => write!(f, "ошибка SRTP: {err}"),
            Self::Rtp(err) => write!(f, "ошибка RTP: {err}"),
            Self::Rtcp(err) => write!(f, "ошибка RTCP: {err:?}"),
            Self::InvalidPhase { expected, actual } => {
                write!(
                    f,
                    "недопустимая фаза сессии: ожидалась {expected}, текущая {actual:?}"
                )
            }
            Self::MissingChannelId => f.write_str("отсутствует channel_id в состоянии голоса"),
            Self::ServerIdMismatch => {
                f.write_str("несовпадение guild_id между VoiceServerUpdate и VoiceStateUpdate")
            }
            Self::UnsupportedEncryptionMode => {
                f.write_str("нет поддерживаемых режимов шифрования в списке сервера")
            }
            Self::SsrcMismatch { expected, actual } => {
                write!(
                    f,
                    "несовпадение SSRC в UDP discovery: ожидался {expected}, получен {actual}"
                )
            }
            Self::InvalidSecretKeyLength { len } => {
                write!(f, "некорректная длина secret_key: {len} Б (ожидалось 32 Б)")
            }
            Self::SrtpNotEstablished => f.write_str("контекст SRTP ещё не инициализирован"),
            Self::HeartbeatNotConfigured => {
                f.write_str("интервал heartbeat ещё не получен из OP 8 HELLO")
            }
            Self::HeartbeatTimeout {
                sent_at_ms,
                now_ms,
                interval_ms,
            } => write!(
                f,
                "таймаут ожидания HEARTBEAT_ACK: отправлен в {sent_at_ms} мс, сейчас {now_ms} мс, интервал {interval_ms} мс"
            ),
            Self::HeartbeatAckMismatch {
                expected_nonce,
                actual_nonce,
            } => write!(
                f,
                "несовпадение нонса HEARTBEAT_ACK: ожидался {expected_nonce}, получен {actual_nonce}"
            ),
        }
    }
}

impl std::error::Error for VoiceSessionError {}

impl From<VoiceError> for VoiceSessionError {
    fn from(err: VoiceError) -> Self {
        Self::Signaling(err)
    }
}

impl From<UdpError> for VoiceSessionError {
    fn from(err: UdpError) -> Self {
        Self::Udp(err)
    }
}

impl From<SrtpError> for VoiceSessionError {
    fn from(err: SrtpError) -> Self {
        Self::Srtp(err)
    }
}

impl From<RtpError> for VoiceSessionError {
    fn from(err: RtpError) -> Self {
        Self::Rtp(err)
    }
}

impl From<RtcpError> for VoiceSessionError {
    fn from(err: RtcpError) -> Self {
        Self::Rtcp(err)
    }
}

/// Расшифрованный входящий RTP-пакет.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedRtpPacket {
    /// Полный расшифрованный RTP-кадр (`[clear header + ext][plaintext payload]`).
    pub raw: Vec<u8>,
    /// Разобранный RTP-заголовок (`sequence`, `timestamp`, `ssrc`, `payload_type`, `marker`).
    pub header: RtpHeader,
    /// Профиль расширения заголовка (например, `0xBEDE`), если присутствует.
    pub extension_profile: Option<u16>,
    /// Байты данных расширения заголовка, если присутствует.
    pub extension_data: Option<Vec<u8>>,
    /// Открытая полезная нагрузка медиа-кадра (Opus / видео).
    pub payload: Vec<u8>,
}

/// Расшифрованный входящий составной пакет SRTCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedRtcpCompound {
    /// Полный расшифрованный RTCP-пакет без SRTP-тега и трейлера.
    pub raw: Vec<u8>,
    /// Разобранные RTCP-пакеты (`SenderReport`, `ReceiverReport`, `Other`).
    pub compound: Compound,
}

/// Кодирует 74-байтный запрос UDP IP Discovery (`type = 1`, `length = 70`, `ssrc` BE, 66 нулевых байт).
#[must_use]
pub fn encode_discovery_request(ssrc: u32) -> [u8; DISCOVERY_LEN] {
    encode_discovery_request_with_port(ssrc, 0)
}

/// Кодирует 74-байтный запрос UDP IP Discovery с указанием локального порта в байтах `72..74`.
#[must_use]
pub fn encode_discovery_request_with_port(ssrc: u32, local_port: u16) -> [u8; DISCOVERY_LEN] {
    DiscoveryPacket::Request {
        ssrc,
        port: local_port,
    }
    .encode()
    .unwrap_or([0u8; DISCOVERY_LEN])
}

/// Декодирует 74-байтный ответ UDP IP Discovery (`type = 2`, `length = 70`, `ssrc`, ASCII IP, `port` BE).
pub fn decode_discovery_response(packet: &[u8]) -> Result<DiscoveryResponse, VoiceSessionError> {
    match DiscoveryPacket::parse(packet)? {
        DiscoveryPacket::Response {
            ssrc,
            address,
            port,
        } => Ok(DiscoveryResponse {
            ssrc,
            address,
            port,
        }),
        DiscoveryPacket::Request { .. } => Err(VoiceSessionError::Udp(UdpError::InvalidType)),
    }
}

/// Выбирает режим шифрования из списка `OP 2 READY` с приоритетом
/// `aead_xchacha20_poly1305_rtpsize` над `aead_aes256_gcm_rtpsize`.
pub fn select_encryption_mode(modes: &[String]) -> Result<EncryptionMode, VoiceSessionError> {
    const PRIORITY_ORDER: [EncryptionMode; 3] = [
        EncryptionMode::XChaCha20Poly1305RtpSize,
        EncryptionMode::Aes256GcmRtpSize,
        EncryptionMode::Aes256Gcm,
    ];
    for preferred in PRIORITY_ORDER {
        let target = preferred.as_engine_name();
        if modes.iter().any(|mode| mode == target) {
            return Ok(preferred);
        }
    }
    Err(VoiceSessionError::UnsupportedEncryptionMode)
}

/// Вычисляет SSRC видео-потока и RTX-потока из базового `audio_ssrc` для сессии Go Live:
/// `video_ssrc = audio_ssrc + 1`, `rtx_ssrc = audio_ssrc + 2`.
#[must_use]
pub const fn compute_stream_ssrcs(audio_ssrc: u32) -> (u32, u32) {
    (audio_ssrc.wrapping_add(1), audio_ssrc.wrapping_add(2))
}

/// Событие `STREAM_SERVER_UPDATE` из основного шлюза Discord для выделенной второй сессии Go Live.
#[derive(Clone, PartialEq, Eq)]
pub struct StreamServerUpdate {
    /// Ключ стрима (например, `"guild:<guild_id>:<channel_id>:<user_id>"`).
    pub stream_key: String,
    /// Точка подключения медиа-шлюза стрима (`host:port`).
    pub endpoint: String,
    /// Токен авторизации сессии стрима.
    pub token: String,
    /// Идентификатор сессии шлюза (если передан или привязан от голосовой сессии).
    pub session_id: Option<String>,
    /// Выделенный `server_id` (`rtc_server_id`) сессии стрима, если указан отдельно от `stream_key`.
    pub server_id: Option<String>,
}

impl fmt::Debug for StreamServerUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamServerUpdate")
            .field("stream_key", &self.stream_key)
            .field("endpoint", &self.endpoint)
            .field("token", &"<redacted>")
            .field("session_id", &"<redacted>")
            .field("server_id", &"<redacted>")
            .finish()
    }
}

impl StreamServerUpdate {
    /// Создаёт `StreamServerUpdate` с обязательными полями `stream_key`, `endpoint`, `token`.
    #[must_use]
    pub fn new(
        stream_key: impl Into<String>,
        endpoint: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            stream_key: stream_key.into(),
            endpoint: endpoint.into(),
            token: token.into(),
            session_id: None,
            server_id: None,
        }
    }

    /// Разбирает событие `STREAM_SERVER_UPDATE` из JSON (с конвертом `{"d":{...}}` или без).
    pub fn parse(input: &str) -> Result<Self, VoiceError> {
        let root = discord_json::parse(input)
            .map_err(|_| VoiceError::invalid("invalid stream server update JSON"))?;
        let data = root.get("d").unwrap_or(&root);
        let stream_key = data
            .get("stream_key")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| VoiceError::invalid("missing stream_key"))?
            .to_owned();
        let endpoint = data
            .get("endpoint")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| VoiceError::invalid("missing stream endpoint"))?
            .to_owned();
        let token = data
            .get("token")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| VoiceError::invalid("missing stream token"))?
            .to_owned();
        let session_id = data
            .get("session_id")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let server_id = data
            .get("server_id")
            .or_else(|| data.get("rtc_server_id"))
            .or_else(|| data.get("guild_id"))
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        Ok(Self {
            stream_key,
            endpoint,
            token,
            session_id,
            server_id,
        })
    }

    /// Разбирает компоненты `stream_key` формата `"guild:<guild_id>:<channel_id>:<user_id>"`
    /// или `"call:<channel_id>:<user_id>"`.
    #[must_use]
    pub fn parse_stream_key(&self) -> Option<(&str, &str, &str)> {
        let parts: Vec<&str> = self.stream_key.split(':').collect();
        match parts.as_slice() {
            ["guild", guild_id, channel_id, user_id]
                if !guild_id.is_empty() && !channel_id.is_empty() && !user_id.is_empty() =>
            {
                Some((guild_id, channel_id, user_id))
            }
            ["call", channel_id, user_id] if !channel_id.is_empty() && !user_id.is_empty() => {
                Some((channel_id, channel_id, user_id))
            }
            _ => None,
        }
    }

    /// URL подключения к голосовому шлюзу стрима (`wss://<endpoint>/?v=9`).
    pub fn websocket_url(&self) -> Result<String, VoiceError> {
        let server_id = self
            .server_id
            .clone()
            .or_else(|| self.parse_stream_key().map(|(g, _, _)| g.to_owned()))
            .unwrap_or_else(|| "0".to_owned());
        let update = VoiceServerUpdate {
            token: self.token.clone(),
            guild_id: server_id,
            endpoint: self.endpoint.clone(),
        };
        update.websocket_url()
    }
}

/// Максимальное разрешение потока для `OP 12 VIDEO` (`{"type":"fixed","width":...,"height":...}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoMaxResolution {
    /// Тип ограничения разрешения (`"fixed"` или `"source"`).
    pub kind: String,
    /// Ширина кадра в пикселях.
    pub width: u32,
    /// Высота кадра в пикселях.
    pub height: u32,
}

impl VideoMaxResolution {
    /// Создаёт фиксированное разрешение `{"type":"fixed","width":...,"height":...}`.
    #[must_use]
    pub fn fixed(width: u32, height: u32) -> Self {
        Self {
            kind: "fixed".to_owned(),
            width,
            height,
        }
    }
}

/// Элемент массива `streams` в `OP 12 VIDEO` для демонстрации экрана Go Live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoStreamDescriptor {
    /// Тип потока (`"screen"` или `"video"`).
    pub kind: String,
    /// Идентификатор simulcast-слоя (`"100"`).
    pub rid: String,
    /// Качество потока (`100`).
    pub quality: u32,
    /// SSRC основного видео-потока (`video_ssrc`).
    pub ssrc: u32,
    /// SSRC потока повторной передачи (`rtx_ssrc`).
    pub rtx_ssrc: u32,
    /// Признак активности потока (`true`).
    pub active: bool,
    /// Максимальный битрейт в бит/с (`max_bitrate`).
    pub max_bitrate: u32,
    /// Максимальная частота кадров (`max_framerate`).
    pub max_framerate: u32,
    /// Максимальное разрешение потока (`max_resolution`).
    pub max_resolution: VideoMaxResolution,
}

/// Кадр `Voice Opcode 12 (Video)` для объявления видео/экранного потока Go Live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceVideo {
    /// Базовый аудио SSRC сессии (`audio_ssrc`).
    pub audio_ssrc: u32,
    /// SSRC видео-потока (`video_ssrc = audio_ssrc + 1`).
    pub video_ssrc: u32,
    /// SSRC RTX-потока (`rtx_ssrc = audio_ssrc + 2`).
    pub rtx_ssrc: u32,
    /// Массив дескрипторов потоков (`streams`).
    pub streams: Vec<VideoStreamDescriptor>,
}

impl VoiceVideo {
    /// Создаёт кадр `OP 12 VIDEO` для демонстрации экрана Go Live, вычисляя
    /// `video_ssrc = audio_ssrc + 1` и `rtx_ssrc = audio_ssrc + 2`.
    #[must_use]
    pub fn screen_share(
        audio_ssrc: u32,
        max_bitrate: u32,
        max_framerate: u32,
        width: u32,
        height: u32,
    ) -> Self {
        let (video_ssrc, rtx_ssrc) = compute_stream_ssrcs(audio_ssrc);
        Self::screen_share_with_ssrcs(
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
            max_bitrate,
            max_framerate,
            width,
            height,
        )
    }

    /// Создаёт кадр `OP 12 VIDEO` с явно переданными `audio_ssrc`, `video_ssrc`, `rtx_ssrc`.
    #[must_use]
    pub fn screen_share_with_ssrcs(
        audio_ssrc: u32,
        video_ssrc: u32,
        rtx_ssrc: u32,
        max_bitrate: u32,
        max_framerate: u32,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
            streams: vec![VideoStreamDescriptor {
                kind: "screen".to_owned(),
                rid: "100".to_owned(),
                quality: 100,
                ssrc: video_ssrc,
                rtx_ssrc,
                active: true,
                max_bitrate,
                max_framerate,
                max_resolution: VideoMaxResolution::fixed(width, height),
            }],
        }
    }

    /// Сериализует кадр `Voice Opcode 12 (Video)` в точный JSON-контракт голосового шлюза.
    #[must_use]
    pub fn to_json(&self) -> String {
        let streams = self
            .streams
            .iter()
            .map(|s| {
                object(vec![
                    ("type", JsonValue::String(s.kind.clone())),
                    ("rid", JsonValue::String(s.rid.clone())),
                    ("quality", json_u64(u64::from(s.quality))),
                    ("ssrc", json_u64(u64::from(s.ssrc))),
                    ("rtx_ssrc", json_u64(u64::from(s.rtx_ssrc))),
                    ("active", JsonValue::Bool(s.active)),
                    ("max_bitrate", json_u64(u64::from(s.max_bitrate))),
                    ("max_framerate", json_u64(u64::from(s.max_framerate))),
                    (
                        "max_resolution",
                        object(vec![
                            ("type", JsonValue::String(s.max_resolution.kind.clone())),
                            ("width", json_u64(u64::from(s.max_resolution.width))),
                            ("height", json_u64(u64::from(s.max_resolution.height))),
                        ]),
                    ),
                ])
            })
            .collect();
        let d = object(vec![
            ("audio_ssrc", json_u64(u64::from(self.audio_ssrc))),
            ("video_ssrc", json_u64(u64::from(self.video_ssrc))),
            ("rtx_ssrc", json_u64(u64::from(self.rtx_ssrc))),
            ("streams", JsonValue::Array(streams)),
        ]);
        discord_json::to_json_string(&object(vec![("op", json_u64(12)), ("d", d)]))
    }

    /// Разбирает кадр `Voice Opcode 12 (Video)` из JSON.
    pub fn parse_json(input: &str) -> Result<Self, VoiceError> {
        let root =
            discord_json::parse(input).map_err(|_| VoiceError::invalid("invalid OP 12 JSON"))?;
        if let Some(op) = root.get("op").and_then(JsonValue::as_u64)
            && op != 12
        {
            return Err(VoiceError::invalid("unexpected opcode for OP 12 Video"));
        }
        let d = root.get("d").unwrap_or(&root);
        let req_u32 = |obj: &JsonValue, key: &str| -> Result<u32, VoiceError> {
            let v = obj
                .get(key)
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| VoiceError::invalid("missing u32 field in OP 12"))?;
            u32::try_from(v).map_err(|_| VoiceError::invalid("u32 overflow in OP 12"))
        };
        let audio_ssrc = req_u32(d, "audio_ssrc")?;
        let video_ssrc = req_u32(d, "video_ssrc")?;
        let rtx_ssrc = req_u32(d, "rtx_ssrc")?;
        let arr = d
            .get("streams")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| VoiceError::invalid("missing streams in OP 12"))?;
        let mut streams = Vec::with_capacity(arr.len());
        for item in arr {
            let kind = item
                .get("type")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| VoiceError::invalid("missing stream type in OP 12"))?
                .to_owned();
            let rid = item
                .get("rid")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| VoiceError::invalid("missing stream rid in OP 12"))?
                .to_owned();
            let quality = req_u32(item, "quality")?;
            let ssrc = req_u32(item, "ssrc")?;
            let stream_rtx = req_u32(item, "rtx_ssrc")?;
            let active = item
                .get("active")
                .and_then(JsonValue::as_bool)
                .ok_or_else(|| VoiceError::invalid("missing stream active in OP 12"))?;
            let max_bitrate = req_u32(item, "max_bitrate")?;
            let max_framerate = req_u32(item, "max_framerate")?;
            let res_obj = item
                .get("max_resolution")
                .ok_or_else(|| VoiceError::invalid("missing max_resolution in OP 12"))?;
            let res_kind = res_obj
                .get("type")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| VoiceError::invalid("missing max_resolution.type in OP 12"))?
                .to_owned();
            let width = req_u32(res_obj, "width")?;
            let height = req_u32(res_obj, "height")?;
            streams.push(VideoStreamDescriptor {
                kind,
                rid,
                quality,
                ssrc,
                rtx_ssrc: stream_rtx,
                active,
                max_bitrate,
                max_framerate,
                max_resolution: VideoMaxResolution {
                    kind: res_kind,
                    width,
                    height,
                },
            });
        }
        Ok(Self {
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
            streams,
        })
    }
}

/// Кадр `Voice Opcode 15 (Media Sink Wants)` (`{"op":15,"d":{"any":100,...}}`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaSinkWants {
    /// Запрошенное качество по умолчанию (`"any": 100`).
    pub any: Option<u32>,
    /// Поле `"pixelCounts"` (если присутствует).
    pub pixel_counts: Option<u32>,
    /// Индивидуальные запросы качества по SSRC (`"<ssrc>": quality`).
    pub ssrc_qualities: BTreeMap<u32, u32>,
}

impl MediaSinkWants {
    /// Создаёт `MediaSinkWants` с полем `"any"`.
    #[must_use]
    pub fn any(quality: u32) -> Self {
        Self {
            any: Some(quality),
            pixel_counts: None,
            ssrc_qualities: BTreeMap::new(),
        }
    }

    /// Добавляет целевое качество для конкретного `ssrc`.
    #[must_use]
    pub fn with_ssrc(mut self, ssrc: u32, quality: u32) -> Self {
        self.ssrc_qualities.insert(ssrc, quality);
        self
    }

    /// Сериализует кадр `OP 15 MEDIA_SINK_WANTS` в JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut entries: BTreeMap<String, JsonValue> = BTreeMap::new();
        if let Some(any) = self.any {
            entries.insert("any".to_owned(), json_u64(u64::from(any)));
        }
        if let Some(pc) = self.pixel_counts {
            entries.insert("pixelCounts".to_owned(), json_u64(u64::from(pc)));
        }
        for (&ssrc, &quality) in &self.ssrc_qualities {
            entries.insert(ssrc.to_string(), json_u64(u64::from(quality)));
        }
        discord_json::to_json_string(&object(vec![
            ("op", json_u64(15)),
            ("d", JsonValue::Object(entries)),
        ]))
    }

    /// Разбирает `MediaSinkWants` из JSON-значения поля `d`.
    pub fn from_json_value(data: &JsonValue) -> Result<Self, VoiceError> {
        let entries = data
            .as_object()
            .ok_or_else(|| VoiceError::invalid("MediaSinkWants d must be an object"))?;
        let mut wants = Self::default();
        for (key, val) in entries {
            if key == "any" {
                if let Some(v) = val.as_u64().and_then(|n| u32::try_from(n).ok()) {
                    wants.any = Some(v);
                }
            } else if key == "pixelCounts" {
                if let Some(v) = val.as_u64().and_then(|n| u32::try_from(n).ok()) {
                    wants.pixel_counts = Some(v);
                }
            } else if let Ok(ssrc) = key.parse::<u32>()
                && let Some(v) = val.as_u64().and_then(|n| u32::try_from(n).ok())
            {
                wants.ssrc_qualities.insert(ssrc, v);
            }
        }
        Ok(wants)
    }

    /// Разбирает кадр `OP 15 MEDIA_SINK_WANTS` из JSON-строки.
    pub fn parse_json(input: &str) -> Result<Self, VoiceError> {
        let root =
            discord_json::parse(input).map_err(|_| VoiceError::invalid("invalid OP 15 JSON"))?;
        if let Some(op) = root.get("op").and_then(JsonValue::as_u64)
            && op != 15
        {
            return Err(VoiceError::invalid(
                "unexpected opcode for OP 15 MediaSinkWants",
            ));
        }
        let d = root.get("d").unwrap_or(&root);
        Self::from_json_value(d)
    }
}

/// Диспетчеризованное событие DAVE / MLS Voice Gateway (`Op 21..=31`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveMlsEvent {
    /// `Op 21`: Prepare Transition (JSON, сервер → клиент).
    PrepareTransition(crate::dave::DavePrepareTransition),
    /// `Op 22`: Execute Transition (JSON, сервер → клиент).
    ExecuteTransition(crate::dave::DaveExecuteTransition),
    /// `Op 23`: Transition Ready (JSON, клиент → сервер).
    TransitionReady(crate::dave::DaveReadyForTransition),
    /// `Op 24`: Prepare Epoch (JSON, сервер → клиент).
    PrepareEpoch(crate::dave::DavePrepareEpoch),
    /// `Op 25`: External Sender Package (Binary, сервер → клиент).
    ExternalSenderPackage { seq: u16, payload: Vec<u8> },
    /// `Op 26`: Key Package (Binary, клиент → сервер).
    KeyPackage { payload: Vec<u8> },
    /// `Op 27`: Proposals (Binary, сервер → клиент).
    Proposals { seq: u16, payload: Vec<u8> },
    /// `Op 28`: Commit Welcome (Binary, клиент → сервер).
    CommitWelcome { payload: Vec<u8> },
    /// `Op 29`: Announce Commit Transition (Binary, сервер → клиент).
    AnnounceCommit {
        seq: u16,
        transition_id: u16,
        commit: Vec<u8>,
    },
    /// `Op 30`: Welcome (Binary, сервер → клиент).
    Welcome {
        seq: u16,
        transition_id: u16,
        welcome: Vec<u8>,
    },
    /// `Op 31`: Invalid Commit Welcome (JSON, клиент ↔ сервер).
    InvalidCommitWelcome(crate::dave::DaveInvalidCommitWelcome),
}

/// Драйвер голосовой сессии Discord (`VoiceSessionDriver`).
pub struct VoiceSessionDriver {
    server_id: String,
    channel_id: String,
    user_id: String,
    session_id: String,
    token: String,
    endpoint: String,
    stream_key: Option<String>,
    go_live_session: Option<Box<VoiceSessionDriver>>,
    media_sink_wants: Option<MediaSinkWants>,
    phase: VoiceSessionPhase,
    hello_version: Option<u8>,
    heartbeat_interval_ms: Option<u64>,
    last_seq_ack: Option<u64>,
    last_heartbeat_sent_ms: Option<u64>,
    last_heartbeat_ack_ms: Option<u64>,
    pending_heartbeat: Option<(u64, u64)>,
    last_rtt_ms: Option<u64>,
    ssrc: Option<u32>,
    video_ssrc: Option<u32>,
    rtx_ssrc: Option<u32>,
    server_udp_ip: Option<String>,
    server_udp_port: Option<u16>,
    external_ip: Option<String>,
    external_port: Option<u16>,
    selected_mode: Option<EncryptionMode>,
    dave_protocol_version: Option<u8>,
    secure_frames_version: Option<u8>,
    media_session_id: Option<String>,
    audio_codec: Option<String>,
    video_codec: Option<String>,
    experiments: Vec<String>,
    speaking_flags: SpeakingFlags,
    srtp: Option<SrtpContext>,
    audio_sender: Option<RtpSender>,
    video_sequence: u16,
    video_timestamp: u32,
    outgoing_ws: Vec<String>,
    outgoing_binary_ws: Vec<Vec<u8>>,
    outgoing_udp: Vec<Vec<u8>>,
    dave_events: Vec<DaveMlsEvent>,
    dave_epoch: Option<u64>,
    dave_pending_transition: Option<(u16, u8)>,
    dave_last_executed_transition: Option<u16>,
    dave_external_sender: Option<Vec<u8>>,
    unhandled_opcodes: BTreeMap<u8, u64>,
}

impl fmt::Debug for VoiceSessionDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceSessionDriver")
            .field("server_id", &"<redacted>")
            .field("channel_id", &self.channel_id)
            .field("user_id", &"<redacted>")
            .field("session_id", &"<redacted>")
            .field("token", &"<redacted>")
            .field("endpoint", &self.endpoint)
            .field("phase", &self.phase)
            .field("hello_version", &self.hello_version)
            .field("heartbeat_interval_ms", &self.heartbeat_interval_ms)
            .field("last_heartbeat_ack_ms", &self.last_heartbeat_ack_ms)
            .field("ssrc", &self.ssrc)
            .field("selected_mode", &self.selected_mode)
            .field("dave_protocol_version", &self.dave_protocol_version)
            .field("secure_frames_version", &self.secure_frames_version)
            .field("media_session_id", &"<redacted>")
            .field("audio_codec", &self.audio_codec)
            .field("video_codec", &self.video_codec)
            .field("experiments", &self.experiments)
            .field("speaking_flags", &self.speaking_flags)
            .field("unhandled_opcodes", &self.unhandled_opcodes)
            .finish()
    }
}

impl VoiceSessionDriver {
    /// Создаёт драйвер голосовой сессии из `VoiceServerUpdate` и `VoiceStateUpdate`
    /// и сразу формирует исходящий кадр `OP 0 IDENTIFY`.
    pub fn new(
        server: VoiceServerUpdate,
        state: VoiceStateUpdate,
    ) -> Result<Self, VoiceSessionError> {
        let _ = server.websocket_url()?;
        let channel_id = state
            .channel_id
            .filter(|id| !id.is_empty())
            .ok_or(VoiceSessionError::MissingChannelId)?;
        if let Some(state_guild_id) = &state.guild_id
            && state_guild_id != &server.guild_id
        {
            return Err(VoiceSessionError::ServerIdMismatch);
        }

        let mut driver = Self {
            server_id: server.guild_id,
            channel_id,
            user_id: state.user_id,
            session_id: state.session_id,
            token: server.token,
            endpoint: server.endpoint,
            stream_key: None,
            go_live_session: None,
            media_sink_wants: None,
            phase: VoiceSessionPhase::Identifying,
            hello_version: None,
            heartbeat_interval_ms: None,
            last_seq_ack: None,
            last_heartbeat_sent_ms: None,
            last_heartbeat_ack_ms: None,
            pending_heartbeat: None,
            last_rtt_ms: None,
            ssrc: None,
            video_ssrc: None,
            rtx_ssrc: None,
            server_udp_ip: None,
            server_udp_port: None,
            external_ip: None,
            external_port: None,
            selected_mode: None,
            dave_protocol_version: None,
            secure_frames_version: None,
            media_session_id: None,
            audio_codec: None,
            video_codec: None,
            experiments: Vec::new(),
            speaking_flags: SpeakingFlags::NONE,
            srtp: None,
            audio_sender: None,
            video_sequence: 0,
            video_timestamp: 0,
            outgoing_ws: Vec::with_capacity(2),
            outgoing_binary_ws: Vec::new(),
            outgoing_udp: Vec::new(),
            dave_events: Vec::new(),
            dave_epoch: None,
            dave_pending_transition: None,
            dave_last_executed_transition: None,
            dave_external_sender: None,
            unhandled_opcodes: BTreeMap::new(),
        };
        let identify_json = driver.identify_json();
        driver.outgoing_ws.push(identify_json);
        Ok(driver)
    }

    /// Запускает сессию и возвращает драйвер вместе со структурой `OP 0 IDENTIFY`.
    pub fn start(
        server: VoiceServerUpdate,
        state: VoiceStateUpdate,
    ) -> Result<(Self, Identify), VoiceSessionError> {
        let driver = Self::new(server, state)?;
        let identify = driver.build_identify();
        Ok((driver, identify))
    }

    /// URL подключения к голосовому шлюзу (`wss://<endpoint>/?v=9`).
    pub fn websocket_url(&self) -> Result<String, VoiceSessionError> {
        let update = VoiceServerUpdate {
            token: self.token.clone(),
            guild_id: self.server_id.clone(),
            endpoint: self.endpoint.clone(),
        };
        Ok(update.websocket_url()?)
    }

    /// Текущая фаза голосовой сессии.
    #[must_use]
    pub const fn phase(&self) -> VoiceSessionPhase {
        self.phase
    }

    /// Собирает структуру `OP 0 IDENTIFY` для текущей сессии.
    #[must_use]
    pub fn build_identify(&self) -> Identify {
        Identify {
            server_id: self.server_id.clone(),
            channel_id: self.channel_id.clone(),
            user_id: self.user_id.clone(),
            session_id: self.session_id.clone(),
            token: self.token.clone(),
            max_dave_protocol_version: DAVE_PROTOCOL_VERSION,
            video: true,
            streams: vec![StreamSpec {
                kind: "video".to_owned(),
                ssrc: None,
                rtx_ssrc: None,
                rid: Some("100".to_owned()),
                quality: Some(100),
                active: None,
            }],
        }
    }

    /// Возвращает JSON-строку `OP 0 IDENTIFY`.
    #[must_use]
    pub fn identify_json(&self) -> String {
        self.build_identify().to_json()
    }

    /// Собирает кадр `OP 7 RESUME` для восстановления сигнального соединения.
    #[must_use]
    pub fn build_resume(&self) -> VoiceResume {
        VoiceResume {
            server_id: self.server_id.clone(),
            channel_id: self.channel_id.clone(),
            session_id: self.session_id.clone(),
            token: self.token.clone(),
            seq_ack: self.last_seq_ack.map_or(-1, |seq| seq as i64),
        }
    }

    /// Забирает накопленные исходящие JSON-кадры для отправки в WebSocket.
    pub fn take_outgoing_ws(&mut self) -> Vec<String> {
        std::mem::take(&mut self.outgoing_ws)
    }

    /// Забирает накопленные исходящие UDP-пакеты (74B discovery, SRTP, SRTCP).
    pub fn take_outgoing_udp(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outgoing_udp)
    }

    /// Обрабатывает входящий JSON-кадр голосового шлюза (`OP 8 HELLO`, `OP 2 READY`,
    /// `OP 4 SESSION_DESCRIPTION`, `OP 6 HEARTBEAT_ACK` и др.).
    pub fn handle_ws_frame(&mut self, raw_json: &str) -> Result<VoiceFrame, VoiceSessionError> {
        let frame = parse_frame(raw_json)?;
        if let Some(seq) = frame.seq {
            self.last_seq_ack = Some(seq);
        }
        match &frame.payload {
            VoicePayload::Hello(hello) => {
                self.handle_hello(hello.clone())?;
            }
            VoicePayload::Ready(ready) => {
                let _ = self.handle_ready(ready.clone())?;
            }
            VoicePayload::SessionDescription(desc) => {
                self.handle_session_description(desc.clone())?;
            }
            VoicePayload::Other {
                op: VoiceOp::HeartbeatAck,
                raw,
            } => {
                let t = raw
                    .get("t")
                    .and_then(JsonValue::as_u64)
                    .or_else(|| raw.as_u64())
                    .ok_or_else(|| VoiceError::invalid("missing heartbeat_ack t"))?;
                let _ = self.handle_heartbeat_ack(VoiceHeartbeatAck { t }, t)?;
            }
            // `parse_frame` разбирает `OP 6` в типизированный вариант, поэтому без этой
            // ветки HEARTBEAT_ACK молча падал в `_` и попадал в счётчик неопознанных.
            VoicePayload::HeartbeatAck(ack) => {
                let _ = self.handle_heartbeat_ack(*ack, ack.t)?;
            }
            VoicePayload::Other {
                op: VoiceOp::MediaSinkWants,
                raw,
            } => {
                if let Ok(wants) = MediaSinkWants::from_json_value(raw) {
                    self.media_sink_wants = Some(wants);
                }
            }
            _ => {
                *self.unhandled_opcodes.entry(frame.op.as_u8()).or_insert(0) += 1;
            }
        }
        Ok(frame)
    }

    /// Счётчик необработанных опкодов голосового шлюза: `op -> число кадров`.
    ///
    /// Учитываются DAVE/MLS (`21..=31`) и любые неизвестные значения; кадры,
    /// разобранные в `OP 8 HELLO`, `OP 2 READY`, `OP 4 SESSION_DESCRIPTION`
    /// и `OP 6 HEARTBEAT_ACK`, сюда не попадают.
    #[must_use]
    pub fn unhandled_opcode_counts(&self) -> &BTreeMap<u8, u64> {
        &self.unhandled_opcodes
    }

    /// Суммарное число необработанных кадров по всем опкодам.
    #[must_use]
    pub fn unhandled_opcode_total(&self) -> u64 {
        self.unhandled_opcodes.values().sum()
    }

    /// Обрабатывает `OP 8 HELLO` и сохраняет `heartbeat_interval_ms`.
    pub fn handle_hello(&mut self, hello: Hello) -> Result<(), VoiceSessionError> {
        if hello.heartbeat_interval_ms == 0 {
            return Err(VoiceError::invalid("heartbeat_interval must be > 0").into());
        }
        self.hello_version = Some(hello.version);
        self.heartbeat_interval_ms = Some(hello.heartbeat_interval_ms);
        Ok(())
    }

    /// Обрабатывает `OP 2 READY`: выбирает режим шифрования (`aead_xchacha20_poly1305_rtpsize`
    /// с приоритетом над `aead_aes256_gcm_rtpsize`) и возвращает 74-байтный UDP IP Discovery запрос.
    pub fn handle_ready(&mut self, ready: Ready) -> Result<[u8; DISCOVERY_LEN], VoiceSessionError> {
        let ssrc =
            u32::try_from(ready.ssrc).map_err(|_| VoiceError::invalid("Ready SSRC exceeds u32"))?;
        if ready.port == 0 {
            return Err(VoiceSessionError::Udp(UdpError::InvalidPort));
        }
        let mode = select_encryption_mode(&ready.modes)?;
        let video_stream = ready.streams.iter().find(|s| s.kind == "video");
        let video_ssrc = video_stream
            .and_then(|s| s.ssrc)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or_else(|| ssrc.wrapping_add(1));
        let rtx_ssrc = video_stream
            .and_then(|s| s.rtx_ssrc)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or_else(|| ssrc.wrapping_add(2));

        self.ssrc = Some(ssrc);
        self.video_ssrc = Some(video_ssrc);
        self.rtx_ssrc = Some(rtx_ssrc);
        self.server_udp_ip = Some(ready.ip);
        self.server_udp_port = Some(ready.port);
        self.selected_mode = Some(mode);
        self.experiments = ready.experiments;
        self.phase = VoiceSessionPhase::DiscoveringUdp;

        let request = encode_discovery_request(ssrc);
        self.outgoing_udp.push(request.to_vec());
        Ok(request)
    }

    /// Формирует 74-байтный UDP IP Discovery запрос для текущего `ssrc`.
    pub fn encode_discovery_request(&self) -> Result<[u8; DISCOVERY_LEN], VoiceSessionError> {
        let ssrc = self.ssrc.ok_or(VoiceSessionError::InvalidPhase {
            expected: "DiscoveringUdp",
            actual: self.phase,
        })?;
        Ok(encode_discovery_request(ssrc))
    }

    /// Обрабатывает 74-байтный UDP IP Discovery ответ и автоматически формирует `OP 1 SELECT_PROTOCOL`.
    pub fn handle_discovery_response(
        &mut self,
        packet: &[u8],
    ) -> Result<SelectProtocol, VoiceSessionError> {
        let expected_ssrc = self.ssrc.ok_or(VoiceSessionError::InvalidPhase {
            expected: "DiscoveringUdp",
            actual: self.phase,
        })?;
        let mode = self.selected_mode.ok_or(VoiceSessionError::InvalidPhase {
            expected: "DiscoveringUdp",
            actual: self.phase,
        })?;
        let response = decode_discovery_response(packet)?;
        if response.ssrc != expected_ssrc {
            return Err(VoiceSessionError::SsrcMismatch {
                expected: expected_ssrc,
                actual: response.ssrc,
            });
        }
        self.external_ip = Some(response.address.clone());
        self.external_port = Some(response.port);
        self.phase = VoiceSessionPhase::SelectingProtocol;

        let select = SelectProtocol {
            protocol: "udp".to_owned(),
            address: response.address,
            port: response.port,
            mode: mode.as_engine_name().to_owned(),
            codecs: crate::default_client_codecs(),
            rtc_connection_id: None,
            experiments: Vec::new(),
        };
        self.outgoing_ws.push(select.to_json());
        Ok(select)
    }

    /// Обрабатывает `OP 4 SESSION_DESCRIPTION` и инициализирует `SrtpContext` с сырым 32B `secret_key`.
    pub fn handle_session_description(
        &mut self,
        desc: SessionDescription,
    ) -> Result<(), VoiceSessionError> {
        let secret_key: [u8; 32] = desc.secret_key.as_slice().try_into().map_err(|_| {
            VoiceSessionError::InvalidSecretKeyLength {
                len: desc.secret_key.len(),
            }
        })?;
        let mode = EncryptionMode::from_engine_name(&desc.mode)
            .ok_or(VoiceSessionError::UnsupportedEncryptionMode)?;

        let srtp = SrtpContext::from_secret_key(&secret_key, mode);
        let ssrc = self.ssrc.unwrap_or(0);
        self.srtp = Some(srtp);
        self.audio_sender = Some(RtpSender::new(ssrc));
        self.video_sequence = 0;
        self.video_timestamp = 0;
        self.selected_mode = Some(mode);
        self.dave_protocol_version = Some(desc.dave_protocol_version);
        self.secure_frames_version = Some(desc.secure_frames_version);
        self.media_session_id = Some(desc.media_session_id);
        self.audio_codec = desc.audio_codec;
        self.video_codec = desc.video_codec;
        self.phase = VoiceSessionPhase::Established;
        Ok(())
    }

    /// Шифрует исходящий аудио-кадр Opus с авто-инкрементом `sequence` (+1), `timestamp` (+960)
    /// и добавлением 16B AEAD-тега и 4B LE nonce-трейлера `0x8000_0000 | n`.
    pub fn encrypt_audio_frame(
        &mut self,
        opus_payload: &[u8],
        ext: Option<&[u8]>,
    ) -> Result<Vec<u8>, VoiceSessionError> {
        let sender = self
            .audio_sender
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let srtp = self
            .srtp
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let clear_rtp = sender.next_packet(opus_payload, ext)?;
        let encrypted = srtp.encrypt_rtp(&clear_rtp)?;
        self.outgoing_udp.push(encrypted.clone());
        Ok(encrypted)
    }

    /// Фиксирует паузу DTX (`elapsed_frames` кадров тишины): сдвигает `timestamp` на `960 * elapsed_frames`
    /// и взводит RTP `marker` для следующего речевого пакета.
    pub fn audio_silence(&mut self, elapsed_frames: u32) {
        if let Some(sender) = &mut self.audio_sender {
            sender.silence(elapsed_frames);
        }
    }

    /// Шифрует исходящий видео-кадр с авто-инкрементом `video_sequence` (+1),
    /// `video_timestamp` (`+timestamp_delta`) и общим 4B LE nonce-трейлером `SrtpContext`.
    pub fn encrypt_video_frame(
        &mut self,
        payload: &[u8],
        payload_type: u8,
        timestamp_delta: u32,
        marker: bool,
        ext: Option<&[u8]>,
    ) -> Result<Vec<u8>, VoiceSessionError> {
        let video_ssrc = self
            .video_ssrc
            .or_else(|| self.ssrc.map(|s| s.wrapping_add(1)))
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let srtp = self
            .srtp
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let header = RtpHeader {
            marker,
            payload_type,
            sequence: self.video_sequence,
            timestamp: self.video_timestamp,
            ssrc: video_ssrc,
        };
        let mut clear_rtp = header.encode(ext)?;
        clear_rtp.extend_from_slice(payload);
        let encrypted = srtp.encrypt_rtp(&clear_rtp)?;
        self.video_sequence = self.video_sequence.wrapping_add(1);
        self.video_timestamp = self.video_timestamp.wrapping_add(timestamp_delta);
        self.outgoing_udp.push(encrypted.clone());
        Ok(encrypted)
    }

    /// Шифрует произвольный сформированный открытый RTP-пакет через `SrtpContext`.
    pub fn encrypt_rtp_packet(&mut self, clear_rtp: &[u8]) -> Result<Vec<u8>, VoiceSessionError> {
        let srtp = self
            .srtp
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let encrypted = srtp.encrypt_rtp(clear_rtp)?;
        self.outgoing_udp.push(encrypted.clone());
        Ok(encrypted)
    }

    /// Расшифровывает входящий SRTP-пакет и разбирает RTP-заголовок, расширение и открытую нагрузку.
    pub fn decrypt_rtp_packet(
        &mut self,
        srtp_packet: &[u8],
    ) -> Result<DecryptedRtpPacket, VoiceSessionError> {
        let srtp = self
            .srtp
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let raw = srtp.decrypt_rtp(srtp_packet)?;
        let parsed = parse_rtp(&raw)?;
        let header = parsed.header;
        let extension_profile = parsed.extension.as_ref().map(|ext| ext.profile);
        let extension_data = parsed.extension.as_ref().map(|ext| ext.data.to_vec());
        let payload = parsed.payload.to_vec();
        Ok(DecryptedRtpPacket {
            raw,
            header,
            extension_profile,
            extension_data,
            payload,
        })
    }

    /// Шифрует исходящий пакет RTCP в формат SRTCP (`[clear 8B][ciphertext][tag 16B][counter 4B LE]`).
    pub fn encrypt_rtcp_packet(&mut self, clear_rtcp: &[u8]) -> Result<Vec<u8>, VoiceSessionError> {
        let srtp = self
            .srtp
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let encrypted = srtp.encrypt_srtcp(clear_rtcp)?;
        self.outgoing_udp.push(encrypted.clone());
        Ok(encrypted)
    }

    /// Расшифровывает входящий пакет SRTCP и разбирает составной RTCP-пакет (`Compound`).
    pub fn decrypt_rtcp_packet(
        &mut self,
        srtcp_packet: &[u8],
    ) -> Result<DecryptedRtcpCompound, VoiceSessionError> {
        let srtp = self
            .srtp
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        let raw = srtp.decrypt_srtcp(srtcp_packet)?;
        let compound = parse_compound(&raw)?;
        Ok(DecryptedRtcpCompound { raw, compound })
    }

    /// Генерирует и ставит в очередь кадр `OP 5 SPEAKING` (`SpeakingFlags::MICROPHONE` / `SOUND_SHARE`).
    pub fn send_speaking(
        &mut self,
        flags: SpeakingFlags,
        delay: u32,
    ) -> Result<String, VoiceSessionError> {
        let ssrc = self.ssrc.ok_or(VoiceSessionError::InvalidPhase {
            expected: "Ready or Established",
            actual: self.phase,
        })?;
        self.speaking_flags = flags;
        let speaking = Speaking {
            speaking: flags,
            delay,
            ssrc,
            user_id: None,
        };
        let json = speaking.to_json();
        self.outgoing_ws.push(json.clone());
        Ok(json)
    }

    /// Проверяет таймер `OP 3 HEARTBEAT` на момент `now_ms`:
    /// - если предыдущий heartbeat не получил `OP 6 HEARTBEAT_ACK` за `heartbeat_interval_ms`,
    ///   переводит сессию в `VoiceSessionPhase::TimedOut` и возвращает `Err(HeartbeatTimeout)`;
    /// - если наступило время следующего удара, формирует `OP 3 HEARTBEAT` и возвращает `Ok(Some(json))`;
    /// - иначе возвращает `Ok(None)`.
    pub fn poll_heartbeat(&mut self, now_ms: u64) -> Result<Option<String>, VoiceSessionError> {
        let interval_ms = self
            .heartbeat_interval_ms
            .ok_or(VoiceSessionError::HeartbeatNotConfigured)?;
        if let Some((_nonce, sent_at_ms)) = self.pending_heartbeat {
            if now_ms.saturating_sub(sent_at_ms) >= interval_ms {
                self.phase = VoiceSessionPhase::TimedOut;
                return Err(VoiceSessionError::HeartbeatTimeout {
                    sent_at_ms,
                    now_ms,
                    interval_ms,
                });
            }
            return Ok(None);
        }
        if let Some(last_sent_ms) = self.last_heartbeat_sent_ms
            && now_ms.saturating_sub(last_sent_ms) < interval_ms
        {
            return Ok(None);
        }
        self.send_heartbeat(now_ms).map(Some)
    }

    /// Принудительно отправляет `OP 3 HEARTBEAT` в момент `now_ms` (если предыдущий не просрочен).
    pub fn send_heartbeat(&mut self, now_ms: u64) -> Result<String, VoiceSessionError> {
        let interval_ms = self
            .heartbeat_interval_ms
            .ok_or(VoiceSessionError::HeartbeatNotConfigured)?;
        if let Some((_nonce, sent_at_ms)) = self.pending_heartbeat
            && now_ms.saturating_sub(sent_at_ms) >= interval_ms
        {
            self.phase = VoiceSessionPhase::TimedOut;
            return Err(VoiceSessionError::HeartbeatTimeout {
                sent_at_ms,
                now_ms,
                interval_ms,
            });
        }
        let hb = VoiceHeartbeat {
            t: now_ms,
            seq_ack: self.last_seq_ack.map_or(-1, |seq| seq as i64),
        };
        let json = hb.to_json();
        self.pending_heartbeat = Some((now_ms, now_ms));
        self.last_heartbeat_sent_ms = Some(now_ms);
        self.outgoing_ws.push(json.clone());
        Ok(json)
    }

    /// Обрабатывает входящий `OP 6 HEARTBEAT_ACK` в момент `now_ms`, сверяя эхо-нонс `t` и вычисляя RTT.
    pub fn handle_heartbeat_ack(
        &mut self,
        ack: VoiceHeartbeatAck,
        now_ms: u64,
    ) -> Result<u64, VoiceSessionError> {
        let _ = self
            .heartbeat_interval_ms
            .ok_or(VoiceSessionError::HeartbeatNotConfigured)?;
        if let Some((expected_nonce, _sent_at_ms)) = self.pending_heartbeat
            && ack.t != expected_nonce
        {
            return Err(VoiceSessionError::HeartbeatAckMismatch {
                expected_nonce,
                actual_nonce: ack.t,
            });
        }
        self.pending_heartbeat = None;
        let rtt_ms = now_ms.saturating_sub(ack.t);
        self.last_rtt_ms = Some(rtt_ms);
        self.last_heartbeat_ack_ms = Some(now_ms);
        Ok(rtt_ms)
    }

    /// Интервал heartbeat из `OP 8 HELLO` (мс).
    #[must_use]
    pub const fn heartbeat_interval_ms(&self) -> Option<u64> {
        self.heartbeat_interval_ms
    }

    /// Аудио SSRC из `OP 2 READY`.
    #[must_use]
    pub const fn ssrc(&self) -> Option<u32> {
        self.ssrc
    }

    /// Видео SSRC из `OP 2 READY`.
    #[must_use]
    pub const fn video_ssrc(&self) -> Option<u32> {
        self.video_ssrc
    }

    /// RTX SSRC из `OP 2 READY`.
    #[must_use]
    pub const fn rtx_ssrc(&self) -> Option<u32> {
        self.rtx_ssrc
    }

    /// UDP-адрес и порт медиа-сервера из `OP 2 READY`.
    #[must_use]
    pub fn server_endpoint_udp(&self) -> Option<(&str, u16)> {
        Some((self.server_udp_ip.as_deref()?, self.server_udp_port?))
    }

    /// Обнаруженный внешний IP и UDP-порт клиента из ответа 74B IP Discovery.
    #[must_use]
    pub fn discovered_endpoint_udp(&self) -> Option<(&str, u16)> {
        Some((self.external_ip.as_deref()?, self.external_port?))
    }

    /// Выбранный или подтверждённый режим шифрования SRTP.
    #[must_use]
    pub const fn selected_mode(&self) -> Option<EncryptionMode> {
        self.selected_mode
    }

    /// Версия протокола DAVE из `OP 4 SESSION_DESCRIPTION`.
    #[must_use]
    pub const fn dave_protocol_version(&self) -> Option<u8> {
        self.dave_protocol_version
    }

    /// Текущие флаги речи клиента (`SpeakingFlags`).
    #[must_use]
    pub const fn speaking_flags(&self) -> SpeakingFlags {
        self.speaking_flags
    }

    /// Следующий `sequence` исходящего аудио-кадра.
    #[must_use]
    pub fn audio_sequence(&self) -> u16 {
        self.audio_sender.as_ref().map_or(0, RtpSender::sequence)
    }

    /// Следующий `timestamp` исходящего аудио-кадра.
    #[must_use]
    pub fn audio_timestamp(&self) -> u32 {
        self.audio_sender.as_ref().map_or(0, RtpSender::timestamp)
    }

    /// Следующий `sequence` исходящего видео-кадра.
    #[must_use]
    pub const fn video_sequence(&self) -> u16 {
        self.video_sequence
    }

    /// Следующий `timestamp` исходящего видео-кадра.
    #[must_use]
    pub const fn video_timestamp(&self) -> u32 {
        self.video_timestamp
    }

    /// Текущий сквозной счётчик пакетов `SrtpContext`.
    #[must_use]
    pub fn srtp_counter(&self) -> Option<u64> {
        self.srtp.as_ref().map(SrtpContext::counter)
    }

    /// Последний измеренный RTT по паре `HEARTBEAT` / `HEARTBEAT_ACK` (мс).
    #[must_use]
    pub const fn last_rtt_ms(&self) -> Option<u64> {
        self.last_rtt_ms
    }

    /// Последний полученный `seq` от голосового шлюза.
    #[must_use]
    pub const fn last_seq_ack(&self) -> Option<u64> {
        self.last_seq_ack
    }

    /// Создаёт отдельный драйвер второй сессии Go Live (`STREAM_SERVER_UPDATE`).
    pub fn new_go_live(
        update: StreamServerUpdate,
        fallback_server_id: &str,
        fallback_channel_id: &str,
        fallback_user_id: &str,
        fallback_session_id: &str,
    ) -> Result<Self, VoiceSessionError> {
        let parsed_key = update.parse_stream_key();
        let server_id = update
            .server_id
            .clone()
            .or_else(|| parsed_key.map(|(g, _, _)| g.to_owned()))
            .unwrap_or_else(|| fallback_server_id.to_owned());
        let channel_id = parsed_key
            .map(|(_, c, _)| c.to_owned())
            .unwrap_or_else(|| fallback_channel_id.to_owned());
        let user_id = parsed_key
            .map(|(_, _, u)| u.to_owned())
            .unwrap_or_else(|| fallback_user_id.to_owned());
        let session_id = update
            .session_id
            .clone()
            .unwrap_or_else(|| fallback_session_id.to_owned());

        let server = VoiceServerUpdate {
            token: update.token,
            guild_id: server_id.clone(),
            endpoint: update.endpoint,
        };
        let mut state = VoiceStateUpdate::new(server_id, channel_id, user_id, session_id);
        state.self_video = true;
        state.self_stream = true;

        let mut driver = Self::new(server, state)?;
        driver.stream_key = Some(update.stream_key);
        Ok(driver)
    }

    /// Запускает выделенную параллельную сессию стрима Go Live для `STREAM_SERVER_UPDATE`
    /// внутри текущего голосового драйвера и возвращает мутабельную ссылку на неё.
    pub fn start_go_live_session(
        &mut self,
        update: StreamServerUpdate,
    ) -> Result<&mut VoiceSessionDriver, VoiceSessionError> {
        let go_live = Self::new_go_live(
            update,
            &self.server_id,
            &self.channel_id,
            &self.user_id,
            &self.session_id,
        )?;
        self.go_live_session = Some(Box::new(go_live));
        Ok(self.go_live_session.as_deref_mut().expect("just inserted"))
    }

    /// Ссылка на выделенную параллельную сессию Go Live, если она запущена.
    #[must_use]
    pub fn go_live_session(&self) -> Option<&VoiceSessionDriver> {
        self.go_live_session.as_deref()
    }

    /// Мутабельная ссылка на выделенную параллельную сессию Go Live, если она запущена.
    pub fn go_live_session_mut(&mut self) -> Option<&mut VoiceSessionDriver> {
        self.go_live_session.as_deref_mut()
    }

    /// Закрывает выделенную параллельную сессию Go Live, не затрагивая основную голосовую сессию.
    pub fn close_go_live_session(&mut self) -> bool {
        self.go_live_session.take().is_some()
    }

    /// Признак того, что данный экземпляр драйвера обслуживает выделенную сессию Go Live (`STREAM_SERVER_UPDATE`).
    #[must_use]
    pub const fn is_go_live(&self) -> bool {
        self.stream_key.is_some()
    }

    /// Ключ стрима `stream_key`, если сессия является выделенной сессией Go Live.
    #[must_use]
    pub fn stream_key(&self) -> Option<&str> {
        self.stream_key.as_deref()
    }

    /// Формирует и ставит в очередь `Voice Opcode 12 (Video)` с массивом
    /// `streams: [{ type: "screen", rid: "100", quality: 100, ssrc: video_ssrc, rtx_ssrc, active: true, max_bitrate, max_framerate, max_resolution }]`.
    pub fn send_video_op12(
        &mut self,
        max_bitrate: u32,
        max_framerate: u32,
        width: u32,
        height: u32,
    ) -> Result<String, VoiceSessionError> {
        let audio_ssrc = self.ssrc.ok_or(VoiceSessionError::InvalidPhase {
            expected: "Ready or Established",
            actual: self.phase,
        })?;
        let (default_video, default_rtx) = compute_stream_ssrcs(audio_ssrc);
        let video_ssrc = self.video_ssrc.unwrap_or(default_video);
        let rtx_ssrc = self.rtx_ssrc.unwrap_or(default_rtx);
        self.video_ssrc = Some(video_ssrc);
        self.rtx_ssrc = Some(rtx_ssrc);

        let video = VoiceVideo::screen_share_with_ssrcs(
            audio_ssrc,
            video_ssrc,
            rtx_ssrc,
            max_bitrate,
            max_framerate,
            width,
            height,
        );
        let json = video.to_json();
        self.outgoing_ws.push(json.clone());
        Ok(json)
    }

    /// Формирует и ставит в очередь кадр `Voice Opcode 15 (Media Sink Wants)`.
    pub fn send_media_sink_wants(&mut self, wants: MediaSinkWants) -> String {
        let json = wants.to_json();
        self.media_sink_wants = Some(wants);
        self.outgoing_ws.push(json.clone());
        json
    }

    /// Последнее зафиксированное состояние `Voice Opcode 15 (Media Sink Wants)`.
    #[must_use]
    pub const fn media_sink_wants(&self) -> Option<&MediaSinkWants> {
        self.media_sink_wants.as_ref()
    }

    /// Включает второй слой сквозного шифрования медиа-кадров DAVE MLS E2EE поверх `SrtpContext` (`_rtpsize`).
    pub fn enable_dave_e2ee(
        &mut self,
        epoch: u64,
        key: &[u8; 32],
    ) -> Result<(), VoiceSessionError> {
        let srtp = self
            .srtp
            .as_mut()
            .ok_or(VoiceSessionError::SrtpNotEstablished)?;
        srtp.set_dave_cipher(Some(crate::srtp::DaveMediaCipher::with_epoch(epoch, key)));
        self.dave_epoch = Some(epoch);
        Ok(())
    }

    /// Отключает второй слой сквозного шифрования медиа-кадров DAVE MLS E2EE.
    pub fn disable_dave_e2ee(&mut self) {
        if let Some(srtp) = &mut self.srtp {
            srtp.set_dave_cipher(None);
        }
    }

    /// Проверяет, активен ли второй слой шифрования медиа-кадров DAVE MLS E2EE поверх SRTP.
    #[must_use]
    pub fn is_dave_e2ee_enabled(&self) -> bool {
        self.srtp
            .as_ref()
            .and_then(SrtpContext::dave_cipher)
            .is_some()
    }

    /// Диспетчеризует JSON-опкоды DAVE / MLS Voice Gateway (`Op 21, 22, 23, 24, 31`).
    pub fn handle_dave_json_frame(
        &mut self,
        raw_json: &str,
    ) -> Result<DaveMlsEvent, VoiceSessionError> {
        let root =
            discord_json::parse(raw_json).map_err(|_| VoiceError::invalid("invalid DAVE JSON"))?;
        if let Some(seq) = root.get("seq").and_then(JsonValue::as_u64) {
            self.last_seq_ack = Some(seq);
        }
        let op = root
            .get("op")
            .and_then(JsonValue::as_u64)
            .and_then(|v| u8::try_from(v).ok())
            .ok_or_else(|| VoiceError::invalid("missing DAVE JSON opcode"))?;
        let event = match op {
            crate::dave::OP_PREPARE_TRANSITION => {
                let msg = crate::dave::DavePrepareTransition::parse_json(raw_json)?;
                self.dave_pending_transition = Some((msg.transition_id, msg.protocol_version));
                self.dave_protocol_version = Some(msg.protocol_version);
                if msg.transition_id != 0 {
                    let _ = self.send_transition_ready(msg.transition_id);
                }
                DaveMlsEvent::PrepareTransition(msg)
            }
            crate::dave::OP_EXECUTE_TRANSITION => {
                let msg = crate::dave::DaveExecuteTransition::parse_json(raw_json)?;
                self.dave_last_executed_transition = Some(msg.transition_id);
                DaveMlsEvent::ExecuteTransition(msg)
            }
            crate::dave::OP_READY_FOR_TRANSITION => {
                let msg = crate::dave::DaveReadyForTransition::parse_json(raw_json)?;
                DaveMlsEvent::TransitionReady(msg)
            }
            crate::dave::OP_PREPARE_EPOCH => {
                let msg = crate::dave::DavePrepareEpoch::parse_json(raw_json)?;
                self.dave_epoch = Some(msg.epoch);
                self.dave_protocol_version = Some(msg.protocol_version);
                DaveMlsEvent::PrepareEpoch(msg)
            }
            crate::dave::OP_MLS_INVALID_COMMIT_WELCOME => {
                let msg = crate::dave::DaveInvalidCommitWelcome::parse_json(raw_json)?;
                DaveMlsEvent::InvalidCommitWelcome(msg)
            }
            _ => return Err(VoiceError::invalid("unsupported DAVE JSON opcode").into()),
        };
        self.dave_events.push(event.clone());
        Ok(event)
    }

    /// Диспетчеризует бинарные MLS-опкоды Voice Gateway (`Op 25..=30`).
    ///
    /// Поддерживает как серверный формат `[seq_be: u16][opcode: u8][...]`, так и клиентский `[opcode: u8][...]`.
    pub fn handle_mls_binary_frame(
        &mut self,
        bytes: &[u8],
    ) -> Result<DaveMlsEvent, VoiceSessionError> {
        if bytes.is_empty() {
            return Err(VoiceError::invalid("empty MLS binary frame").into());
        }
        let event = if crate::dave::is_mls_client_binary_opcode(bytes[0]) {
            let opcode = bytes[0];
            let payload = bytes[1..].to_vec();
            if payload.is_empty() {
                return Err(VoiceError::invalid("empty MLS client binary payload").into());
            }
            match opcode {
                crate::dave::OP_MLS_KEY_PACKAGE => DaveMlsEvent::KeyPackage { payload },
                crate::dave::OP_MLS_COMMIT_WELCOME => DaveMlsEvent::CommitWelcome { payload },
                _ => unreachable!(),
            }
        } else {
            if bytes.len() < 3 {
                return Err(VoiceError::invalid("truncated MLS server frame").into());
            }
            let opcode = bytes[2];
            match opcode {
                crate::dave::OP_MLS_EXTERNAL_SENDER => {
                    let frame = crate::dave::decode_server_binary(bytes)?;
                    let seq = frame.seq.unwrap_or(0);
                    self.last_seq_ack = Some(u64::from(seq));
                    self.dave_external_sender = Some(frame.payload.clone());
                    DaveMlsEvent::ExternalSenderPackage {
                        seq,
                        payload: frame.payload,
                    }
                }
                crate::dave::OP_MLS_PROPOSALS => {
                    let frame = crate::dave::decode_server_binary(bytes)?;
                    let seq = frame.seq.unwrap_or(0);
                    self.last_seq_ack = Some(u64::from(seq));
                    DaveMlsEvent::Proposals {
                        seq,
                        payload: frame.payload,
                    }
                }
                crate::dave::OP_MLS_PREPARE_COMMIT_TRANSITION => {
                    let (seq, _, transition_id, commit) =
                        crate::dave::decode_server_transition_binary(bytes)?;
                    self.last_seq_ack = Some(u64::from(seq));
                    self.dave_pending_transition =
                        Some((transition_id, self.dave_protocol_version.unwrap_or(1)));
                    if transition_id != 0 {
                        let _ = self.send_transition_ready(transition_id);
                    }
                    DaveMlsEvent::AnnounceCommit {
                        seq,
                        transition_id,
                        commit,
                    }
                }
                crate::dave::OP_MLS_WELCOME => {
                    let (seq, _, transition_id, welcome) =
                        crate::dave::decode_server_transition_binary(bytes)?;
                    self.last_seq_ack = Some(u64::from(seq));
                    self.dave_pending_transition =
                        Some((transition_id, self.dave_protocol_version.unwrap_or(1)));
                    if transition_id != 0 {
                        let _ = self.send_transition_ready(transition_id);
                    }
                    DaveMlsEvent::Welcome {
                        seq,
                        transition_id,
                        welcome,
                    }
                }
                crate::dave::OP_MLS_KEY_PACKAGE => {
                    let frame = crate::dave::decode_server_binary(bytes)?;
                    if let Some(seq) = frame.seq {
                        self.last_seq_ack = Some(u64::from(seq));
                    }
                    DaveMlsEvent::KeyPackage {
                        payload: frame.payload,
                    }
                }
                crate::dave::OP_MLS_COMMIT_WELCOME => {
                    let frame = crate::dave::decode_server_binary(bytes)?;
                    if let Some(seq) = frame.seq {
                        self.last_seq_ack = Some(u64::from(seq));
                    }
                    DaveMlsEvent::CommitWelcome {
                        payload: frame.payload,
                    }
                }
                _ => return Err(VoiceError::invalid("MLS opcode outside 25..=30").into()),
            }
        };
        self.dave_events.push(event.clone());
        Ok(event)
    }

    /// Формирует и ставит в очередь `Op 23 TransitionReady` (`{"op":23,"d":{"transition_id":...}}`).
    pub fn send_transition_ready(&mut self, transition_id: u16) -> String {
        let json = crate::dave::DaveReadyForTransition { transition_id }.to_json();
        self.outgoing_ws.push(json.clone());
        json
    }

    /// Формирует и ставит в очередь бинарный клиентский кадр `Op 26 KeyPackage` (`[26][payload]`).
    pub fn send_mls_key_package(&mut self, payload: &[u8]) -> Result<Vec<u8>, VoiceSessionError> {
        let wire = crate::dave::encode_client_binary(crate::dave::OP_MLS_KEY_PACKAGE, payload)?;
        self.outgoing_binary_ws.push(wire.clone());
        Ok(wire)
    }

    /// Формирует и ставит в очередь бинарный клиентский кадр `Op 28 CommitWelcome` (`[28][payload]`).
    pub fn send_mls_commit_welcome(
        &mut self,
        payload: &[u8],
    ) -> Result<Vec<u8>, VoiceSessionError> {
        let wire = crate::dave::encode_client_binary(crate::dave::OP_MLS_COMMIT_WELCOME, payload)?;
        self.outgoing_binary_ws.push(wire.clone());
        Ok(wire)
    }

    /// Формирует и ставит в очередь JSON-кадр `Op 31 InvalidCommitWelcome` (`{"op":31,"d":{"transition_id":...}}`).
    pub fn send_invalid_commit_welcome(&mut self, transition_id: u16) -> String {
        let json = crate::dave::DaveInvalidCommitWelcome { transition_id }.to_json();
        self.outgoing_ws.push(json.clone());
        json
    }

    /// Забирает накопленные исходящие бинарные MLS-кадры (`Op 26`, `Op 28`) для отправки в WebSocket.
    pub fn take_outgoing_binary_ws(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outgoing_binary_ws)
    }

    /// История диспетчеризованных событий DAVE / MLS (`Op 21..=31`).
    #[must_use]
    pub fn dave_events(&self) -> &[DaveMlsEvent] {
        &self.dave_events
    }

    /// Текущая подготовленная или активная эпоха DAVE MLS (`Op 24` / `enable_dave_e2ee`).
    #[must_use]
    pub const fn dave_epoch(&self) -> Option<u64> {
        self.dave_epoch
    }

    /// Последний подготовленный переход DAVE `(transition_id, protocol_version)`.
    #[must_use]
    pub const fn dave_pending_transition(&self) -> Option<(u16, u8)> {
        self.dave_pending_transition
    }

    /// Последний выполненный переход DAVE `transition_id` (`Op 22`).
    #[must_use]
    pub const fn dave_last_executed_transition(&self) -> Option<u16> {
        self.dave_last_executed_transition
    }

    /// Пакет внешнего отправителя MLS (`Op 25 ExternalSenderPackage`), если получен.
    #[must_use]
    pub fn dave_external_sender(&self) -> Option<&[u8]> {
        self.dave_external_sender.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RTP_AV1_PAYLOAD_TYPE;
    use crate::crypto::aes::Aes256Key;
    use crate::crypto::chacha::XChaCha20Poly1305;
    use crate::crypto::gcm::Gcm;
    use crate::crypto::{GCM_NONCE_SIZE, XCHACHA_NONCE_SIZE};
    use crate::media::rtp::FRAME_TICKS;
    use crate::media::rtp::OPUS_PAYLOAD_TYPE;
    use crate::rtcp::RtcpPacket;

    const RAW_SECRET_KEY: [u8; 32] = [
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d,
        0x2e, 0x2f,
    ];

    fn sample_updates() -> (VoiceServerUpdate, VoiceStateUpdate) {
        let server = VoiceServerUpdate::parse(
            r#"{"token":"voice-secret-token","guild_id":"1014537851418980372","endpoint":"c-fra19-4da7fe5f.discord.media:2053"}"#,
        )
        .expect("valid VoiceServerUpdate");
        let state = VoiceStateUpdate::parse(
            r#"{"guild_id":"1014537851418980372","channel_id":"1176785373989445703","user_id":"384299970632876032","session_id":"gw-session-secret-id","self_mute":false,"self_deaf":false,"self_video":true}"#,
        )
        .expect("valid VoiceStateUpdate");
        (server, state)
    }

    #[test]
    fn start_from_server_and_state_updates_builds_op0_identify_and_redacts_secrets() {
        let (server, state) = sample_updates();
        let (mut driver, identify) =
            VoiceSessionRunner::start(server, state).expect("driver start");

        assert_eq!(driver.phase(), VoiceSessionPhase::Identifying);
        assert_eq!(
            driver.websocket_url().expect("ws url"),
            "wss://c-fra19-4da7fe5f.discord.media:2053/?v=9"
        );
        assert_eq!(identify.server_id, "1014537851418980372");
        assert_eq!(identify.channel_id, "1176785373989445703");
        assert_eq!(identify.user_id, "384299970632876032");
        assert_eq!(identify.session_id, "gw-session-secret-id");
        assert_eq!(identify.token, "voice-secret-token");
        assert_eq!(identify.max_dave_protocol_version, 1);
        assert!(identify.video);

        let outgoing = driver.take_outgoing_ws();
        assert_eq!(outgoing.len(), 1);
        let parsed = parse_frame(&outgoing[0]).expect("OP 0 IDENTIFY must parse");
        assert_eq!(parsed.op, VoiceOp::Identify);
        assert_eq!(parsed.payload, VoicePayload::Identify(identify));

        let debug_str = format!("{driver:?}");
        assert!(!debug_str.contains("voice-secret-token"));
        assert!(!debug_str.contains("gw-session-secret-id"));
    }

    #[test]
    fn hello_and_ready_select_xchacha_over_aes_and_encode_74b_udp_discovery_request() {
        let (server, state) = sample_updates();
        let mut driver = VoiceSessionDriver::new(server, state).expect("driver init");
        let _ = driver.take_outgoing_ws();

        // OP 8 HELLO из реального захвата rtc1-frames.jsonl
        driver
            .handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
            .expect("handle OP 8 HELLO");
        assert_eq!(driver.heartbeat_interval_ms(), Some(13_750));

        // OP 2 READY с обоими режимами: aead_xchacha20_poly1305_rtpsize должен иметь приоритет над aead_aes256_gcm_rtpsize
        driver
            .handle_ws_frame(
                r#"{"op":2,"d":{"streams":[{"type":"video","ssrc":9209,"rtx_ssrc":9210,"rid":"100","quality":100,"active":false}],"ssrc":9208,"port":19329,"modes":["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":["fixed_keyframe_interval"]}}"#,
            )
            .expect("handle OP 2 READY");

        assert_eq!(driver.phase(), VoiceSessionPhase::DiscoveringUdp);
        assert_eq!(driver.ssrc(), Some(9208));
        assert_eq!(driver.video_ssrc(), Some(9209));
        assert_eq!(driver.rtx_ssrc(), Some(9210));
        assert_eq!(driver.server_endpoint_udp(), Some(("104.29.146.60", 19329)));
        assert_eq!(
            driver.selected_mode(),
            Some(EncryptionMode::XChaCha20Poly1305RtpSize),
            "aead_xchacha20_poly1305_rtpsize должен выбираться с приоритетом над aead_aes256_gcm_rtpsize"
        );

        // Проверка fallback на aead_aes256_gcm_rtpsize, когда xchacha не предложен
        assert_eq!(
            select_encryption_mode(&["aead_aes256_gcm_rtpsize".to_owned()]),
            Ok(EncryptionMode::Aes256GcmRtpSize)
        );
        assert_eq!(
            select_encryption_mode(&["xsalsa20_poly1305".to_owned()]),
            Err(VoiceSessionError::UnsupportedEncryptionMode)
        );

        // Проверка 74-байтного UDP IP Discovery запроса (оракул strace: [00 01][00 46][00 00 23 f8][66 нулей])
        let discovery_req = driver.encode_discovery_request().expect("discovery req");
        assert_eq!(discovery_req.len(), 74);
        assert_eq!(&discovery_req[0..2], &[0x00, 0x01]);
        assert_eq!(&discovery_req[2..4], &[0x00, 0x46]);
        assert_eq!(&discovery_req[4..8], &9208u32.to_be_bytes());
        assert!(discovery_req[8..74].iter().all(|&b| b == 0));

        let udp_out = driver.take_outgoing_udp();
        assert_eq!(udp_out, vec![discovery_req.to_vec()]);
    }

    #[test]
    fn discovery_response_decodes_74b_and_automatically_builds_op1_select_protocol() {
        let (server, state) = sample_updates();
        let mut driver = VoiceSessionDriver::new(server, state).expect("driver init");
        driver
            .handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
            .expect("OP 8");
        driver
            .handle_ws_frame(
                r#"{"op":2,"d":{"streams":[{"type":"video","ssrc":9209,"rtx_ssrc":9210,"rid":"100","quality":100,"active":false}],"ssrc":9208,"port":19329,"modes":["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":["fixed_keyframe_interval"]}}"#,
            )
            .expect("OP 2");
        let _ = driver.take_outgoing_ws();

        // Формируем эталонный 74-байтный ответ UDP IP Discovery из strace s3 (95.164.91.98:35251, ssrc=9208)
        let mut resp_bytes = [0u8; 74];
        resp_bytes[0..2].copy_from_slice(&2u16.to_be_bytes());
        resp_bytes[2..4].copy_from_slice(&70u16.to_be_bytes());
        resp_bytes[4..8].copy_from_slice(&9208u32.to_be_bytes());
        let ip_ascii = b"95.164.91.98";
        resp_bytes[8..8 + ip_ascii.len()].copy_from_slice(ip_ascii);
        resp_bytes[72..74].copy_from_slice(&35251u16.to_be_bytes());

        let decoded = decode_discovery_response(&resp_bytes).expect("decode 74B response");
        assert_eq!(
            decoded,
            DiscoveryResponse {
                ssrc: 9208,
                address: "95.164.91.98".to_owned(),
                port: 35251,
            }
        );

        let select = driver
            .handle_discovery_response(&resp_bytes)
            .expect("handle discovery response -> OP 1 SELECT_PROTOCOL");
        assert_eq!(driver.phase(), VoiceSessionPhase::SelectingProtocol);
        assert_eq!(
            driver.discovered_endpoint_udp(),
            Some(("95.164.91.98", 35251))
        );
        assert_eq!(select.protocol, "udp");
        assert_eq!(select.address, "95.164.91.98");
        assert_eq!(select.port, 35251);
        assert_eq!(select.mode, "aead_xchacha20_poly1305_rtpsize");

        let ws_out = driver.take_outgoing_ws();
        assert_eq!(ws_out.len(), 1);
        let parsed = parse_frame(&ws_out[0]).expect("OP 1 SELECT_PROTOCOL frame parses");
        assert_eq!(parsed.op, VoiceOp::SelectProtocol);
        assert_eq!(parsed.payload, VoicePayload::SelectProtocol(select));
    }

    #[test]
    fn op4_session_description_initializes_live_srtp_and_encrypts_decrypts_media_frames() {
        for mode_name in ["aead_xchacha20_poly1305_rtpsize", "aead_aes256_gcm_rtpsize"] {
            let (server, state) = sample_updates();
            let mut driver = VoiceSessionDriver::new(server, state).expect("driver init");
            driver
                .handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
                .expect("OP 8");
            driver
                .handle_ws_frame(&format!(
                    r#"{{"op":2,"d":{{"streams":[{{"type":"video","ssrc":9209,"rtx_ssrc":9210,"rid":"100","quality":100,"active":false}}],"ssrc":9208,"port":19329,"modes":["{mode_name}"],"ip":"104.29.146.60","experiments":[]}}}}"#
                ))
                .expect("OP 2");

            let resp_bytes = DiscoveryPacket::Response {
                ssrc: 9208,
                address: "95.164.91.98".to_owned(),
                port: 35251,
            }
            .encode()
            .expect("encode response");
            driver.handle_discovery_response(&resp_bytes).expect("OP 1");

            // OP 4 SESSION_DESCRIPTION с 32-байтным массивом secret_key: [u8; 32]
            let key_json_array = RAW_SECRET_KEY
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let op4_json = format!(
                r#"{{"op":4,"d":{{"mode":"{mode_name}","secret_key":[{key_json_array}],"dave_protocol_version":1,"secure_frames_version":1,"media_session_id":"media-sess-1","audio_codec":"opus","video_codec":"AV1"}}}}"#
            );
            driver.handle_ws_frame(&op4_json).expect("OP 4");

            assert_eq!(driver.phase(), VoiceSessionPhase::Established);
            assert_eq!(driver.dave_protocol_version(), Some(1));
            assert_eq!(driver.srtp_counter(), Some(0));

            // 1. Шифруем два аудио-кадра Opus подряд: проверяем авто-инкремент sequence (0 -> 1),
            // timestamp (0 -> 960), 4B LE nonce trailer (0x80000000 -> 0x80000001) и побайтовое совпадение с оракулом AEAD
            let opus_frame_0 = b"\xf8\xff\xfe\x11\x22\x33";
            let ext_0 = [0x10, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
            let srtp_0 = driver
                .encrypt_audio_frame(opus_frame_0, Some(&ext_0))
                .expect("encrypt audio 0");
            assert_eq!(driver.audio_sequence(), 1);
            assert_eq!(driver.audio_timestamp(), FRAME_TICKS);
            assert_eq!(driver.srtp_counter(), Some(1));

            // Сборка независимого оракула для первого аудио-кадра
            let expected_header_0 = RtpHeader {
                marker: true,
                payload_type: OPUS_PAYLOAD_TYPE,
                sequence: 0,
                timestamp: 0,
                ssrc: 9208,
            }
            .encode(Some(&ext_0))
            .expect("header 0");
            let expected_trailer_0 = 0x8000_0000u32.to_le_bytes();
            let expected_ct_tag_0 = if mode_name == "aead_xchacha20_poly1305_rtpsize" {
                let mut nonce = [0u8; XCHACHA_NONCE_SIZE];
                nonce[..4].copy_from_slice(&expected_trailer_0);
                XChaCha20Poly1305::new(&RAW_SECRET_KEY)
                    .encrypt(&nonce, &expected_header_0, opus_frame_0)
                    .expect("oracle xchacha")
            } else {
                let mut nonce = [0u8; GCM_NONCE_SIZE];
                nonce[..4].copy_from_slice(&expected_trailer_0);
                Gcm::new(&Aes256Key::from_array(RAW_SECRET_KEY))
                    .encrypt(&nonce, &expected_header_0, opus_frame_0)
                    .expect("oracle gcm")
            };
            let mut oracle_wire_0 = Vec::new();
            oracle_wire_0.extend_from_slice(&expected_header_0);
            oracle_wire_0.extend_from_slice(&expected_ct_tag_0);
            oracle_wire_0.extend_from_slice(&expected_trailer_0);
            assert_eq!(srtp_0, oracle_wire_0);

            let opus_frame_1 = b"\xf8\xff\xfe\x44\x55\x66\x77";
            let srtp_1 = driver
                .encrypt_audio_frame(opus_frame_1, None)
                .expect("encrypt audio 1");
            assert_eq!(driver.audio_sequence(), 2);
            assert_eq!(driver.audio_timestamp(), 2 * FRAME_TICKS);
            assert_eq!(driver.srtp_counter(), Some(2));
            assert_eq!(
                &srtp_1[srtp_1.len() - 4..],
                &0x8000_0001u32.to_le_bytes(),
                "второй пакет получает трейлер 0x80000001"
            );

            // 2. Шифруем видео-кадр AV1 (PT 101) — сквозной счётчик nonce становится 0x80000002
            let video_payload = b"\x12\x00\x0a\x0b\x0c\x0d";
            let srtp_video = driver
                .encrypt_video_frame(video_payload, RTP_AV1_PAYLOAD_TYPE, 3_000, true, None)
                .expect("encrypt video frame");
            assert_eq!(driver.video_sequence(), 1);
            assert_eq!(driver.video_timestamp(), 3_000);
            assert_eq!(driver.srtp_counter(), Some(3));
            assert_eq!(
                &srtp_video[srtp_video.len() - 4..],
                &0x8000_0002u32.to_le_bytes()
            );

            // 3. Шифруем произвольный открытый RTP-пакет через encrypt_rtp_packet
            let custom_rtp = RtpHeader {
                marker: false,
                payload_type: OPUS_PAYLOAD_TYPE,
                sequence: 77,
                timestamp: 96_000,
                ssrc: 9208,
            }
            .encode(None)
            .map(|mut h| {
                h.extend_from_slice(b"custom-opus");
                h
            })
            .expect("custom rtp");
            let encrypted_custom = driver
                .encrypt_rtp_packet(&custom_rtp)
                .expect("encrypt_rtp_packet");
            assert_eq!(
                &encrypted_custom[encrypted_custom.len() - 4..],
                &0x8000_0003u32.to_le_bytes()
            );

            // 4. Расшифровка входящих SRTP и SRTCP пакетов
            let decoded_0 = driver.decrypt_rtp_packet(&srtp_0).expect("decrypt srtp_0");
            assert_eq!(decoded_0.header.sequence, 0);
            assert_eq!(decoded_0.header.timestamp, 0);
            assert_eq!(decoded_0.header.ssrc, 9208);
            assert!(decoded_0.header.marker);
            assert_eq!(decoded_0.extension_profile, Some(0xBEDE));
            assert_eq!(decoded_0.extension_data.as_deref(), Some(&ext_0[..]));
            assert_eq!(decoded_0.payload, opus_frame_0);

            let decoded_1 = driver.decrypt_rtp_packet(&srtp_1).expect("decrypt srtp_1");
            assert_eq!(decoded_1.header.sequence, 1);
            assert_eq!(decoded_1.header.timestamp, FRAME_TICKS);
            assert!(!decoded_1.header.marker);
            assert_eq!(decoded_1.payload, opus_frame_1);

            let decoded_video = driver
                .decrypt_rtp_packet(&srtp_video)
                .expect("decrypt srtp_video");
            assert_eq!(decoded_video.header.ssrc, 9209);
            assert_eq!(decoded_video.header.payload_type, RTP_AV1_PAYLOAD_TYPE);
            assert_eq!(decoded_video.payload, video_payload);

            // SRTCP Receiver Report round-trip
            let clear_rtcp = crate::udp::empty_receiver_report(9208);
            let encrypted_rtcp = driver
                .encrypt_rtcp_packet(&clear_rtcp)
                .expect("encrypt_rtcp_packet");
            assert_eq!(encrypted_rtcp.len(), 28);
            let decoded_rtcp = driver
                .decrypt_rtcp_packet(&encrypted_rtcp)
                .expect("decrypt_rtcp_packet");
            assert_eq!(decoded_rtcp.raw, clear_rtcp);
            assert_eq!(decoded_rtcp.compound.packets.len(), 1);
            assert!(matches!(
                decoded_rtcp.compound.packets[0],
                RtcpPacket::ReceiverReport {
                    sender_ssrc: 9208,
                    ..
                }
            ));
        }
    }

    #[test]
    fn speaking_op5_and_heartbeat_op3_op6_ack_and_timeout_detection() {
        let (server, state) = sample_updates();
        let mut driver = VoiceSessionDriver::new(server, state).expect("driver init");
        driver
            .handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
            .expect("OP 8");
        driver
            .handle_ws_frame(
                r#"{"op":2,"d":{"streams":[],"ssrc":7142,"port":19334,"modes":["aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.147.33","experiments":[]}}"#,
            )
            .expect("OP 2");
        driver
            .handle_ws_frame(r#"{"seq":5,"op":15,"d":{"any":100}}"#)
            .expect("OP 15 with seq=5");
        assert_eq!(driver.last_seq_ack(), Some(5));
        let _ = driver.take_outgoing_ws();

        // 1. Генерация OP 5 SPEAKING (SpeakingFlags::MICROPHONE и SpeakingFlags::SOUND_SHARE)
        let speaking_mic = driver
            .send_speaking(SpeakingFlags::MICROPHONE, 0)
            .expect("send speaking mic");
        assert_eq!(driver.speaking_flags(), SpeakingFlags::MICROPHONE);
        let mic_json = discord_json::parse(&speaking_mic).expect("valid speaking JSON");
        assert_eq!(mic_json.get("op").and_then(JsonValue::as_u64), Some(5));
        assert_eq!(
            mic_json
                .get("d")
                .and_then(|d| d.get("speaking"))
                .and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            mic_json
                .get("d")
                .and_then(|d| d.get("ssrc"))
                .and_then(JsonValue::as_u64),
            Some(7142)
        );

        let combined_flags = SpeakingFlags::MICROPHONE | SpeakingFlags::SOUND_SHARE;
        let speaking_both = driver
            .send_speaking(combined_flags, 0)
            .expect("send speaking mic + sound_share");
        assert!(driver.speaking_flags().contains(SpeakingFlags::MICROPHONE));
        assert!(driver.speaking_flags().contains(SpeakingFlags::SOUND_SHARE));
        let both_json = discord_json::parse(&speaking_both).expect("valid speaking JSON");
        assert_eq!(
            both_json
                .get("d")
                .and_then(|d| d.get("speaking"))
                .and_then(JsonValue::as_u64),
            Some(3)
        );

        // 2. Периодический OP 3 HEARTBEAT и проверка OP 6 HEARTBEAT_ACK
        let hb_1 = driver
            .poll_heartbeat(1_000)
            .expect("first heartbeat")
            .expect("should emit OP 3");
        let hb_1_json = discord_json::parse(&hb_1).expect("parse OP 3");
        assert_eq!(hb_1_json.get("op").and_then(JsonValue::as_u64), Some(3));
        assert_eq!(
            hb_1_json
                .get("d")
                .and_then(|d| d.get("t"))
                .and_then(JsonValue::as_u64),
            Some(1_000)
        );
        assert_eq!(
            hb_1_json
                .get("d")
                .and_then(|d| d.get("seq_ack"))
                .and_then(JsonValue::as_i64),
            Some(5)
        );

        // До истечения интервала (13 750 мс) новый heartbeat не отправляется
        assert_eq!(driver.poll_heartbeat(5_000), Ok(None));

        // Приходит OP 6 HEARTBEAT_ACK с неверным нонсом -> ошибка HeartbeatAckMismatch
        assert_eq!(
            driver.handle_heartbeat_ack(VoiceHeartbeatAck { t: 999 }, 1_045),
            Err(VoiceSessionError::HeartbeatAckMismatch {
                expected_nonce: 1_000,
                actual_nonce: 999,
            })
        );

        // Приходит валидный OP 6 HEARTBEAT_ACK -> RTT = 45 мс
        let rtt = driver
            .handle_heartbeat_ack(VoiceHeartbeatAck { t: 1_000 }, 1_045)
            .expect("valid ack");
        assert_eq!(rtt, 45);
        assert_eq!(driver.last_rtt_ms(), Some(45));

        // Следующий удар ровно через 13 750 мс (в 14 750 мс)
        let hb_2 = driver
            .poll_heartbeat(14_750)
            .expect("second heartbeat")
            .expect("should emit second OP 3");
        let hb_2_json = discord_json::parse(&hb_2).expect("parse second OP 3");
        assert_eq!(
            hb_2_json
                .get("d")
                .and_then(|d| d.get("t"))
                .and_then(JsonValue::as_u64),
            Some(14_750)
        );

        // 3. Детекция таймаута, если OP 6 HEARTBEAT_ACK не пришёл за интервал (14_750 + 13_750 = 28_500 мс)
        let timeout_err = driver
            .poll_heartbeat(28_500)
            .expect_err("must detect heartbeat timeout");
        assert_eq!(
            timeout_err,
            VoiceSessionError::HeartbeatTimeout {
                sent_at_ms: 14_750,
                now_ms: 28_500,
                interval_ms: 13_750,
            }
        );
        assert_eq!(driver.phase(), VoiceSessionPhase::TimedOut);
    }

    #[test]
    fn go_live_dual_session_computes_stream_ssrcs_and_emits_op12_video_and_op15_sink_wants() {
        let (server, state) = sample_updates();
        let mut primary = VoiceSessionDriver::new(server, state).expect("primary init");
        let _ = primary.take_outgoing_ws();

        let stream_update = StreamServerUpdate::parse(
            r#"{"stream_key":"guild:1014537851418980372:1176785373989445703:384299970632876032","endpoint":"c-fra19-stream.discord.media:2053","token":"stream-secret-token","session_id":"gw-session-secret-id","server_id":"999888777666555444"}"#,
        )
        .expect("parse StreamServerUpdate");
        let debug_update = format!("{stream_update:?}");
        assert!(!debug_update.contains("stream-secret-token"));
        assert!(!debug_update.contains("gw-session-secret-id"));

        let go_live = primary
            .start_go_live_session(stream_update)
            .expect("start_go_live_session");
        assert!(go_live.is_go_live());
        assert_eq!(
            go_live.stream_key(),
            Some("guild:1014537851418980372:1176785373989445703:384299970632876032")
        );
        assert_eq!(
            go_live.websocket_url().expect("stream ws url"),
            "wss://c-fra19-stream.discord.media:2053/?v=9"
        );

        let stream_identify_frames = go_live.take_outgoing_ws();
        assert_eq!(stream_identify_frames.len(), 1);
        let parsed_id = parse_frame(&stream_identify_frames[0]).expect("parse stream OP 0");
        assert_eq!(parsed_id.op, VoiceOp::Identify);

        // Входящий OP 8 HELLO и OP 2 READY (без массива streams — SSRC вычисляются как audio+1 и audio+2)
        go_live
            .handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
            .expect("stream OP 8");
        go_live
            .handle_ws_frame(
                r#"{"op":2,"d":{"streams":[],"ssrc":40000,"port":19400,"modes":["aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":[]}}"#,
            )
            .expect("stream OP 2");

        assert_eq!(compute_stream_ssrcs(40_000), (40_001, 40_002));
        assert_eq!(go_live.ssrc(), Some(40_000));
        assert_eq!(go_live.video_ssrc(), Some(40_001));
        assert_eq!(go_live.rtx_ssrc(), Some(40_002));

        // Генерация точного JSON-контракта Voice Opcode 12 (Video)
        let op12_json = go_live
            .send_video_op12(6_000_000, 60, 1920, 1080)
            .expect("send_video_op12");
        let op12_val = discord_json::parse(&op12_json).expect("valid op12 json");
        assert_eq!(op12_val.get("op").and_then(JsonValue::as_u64), Some(12));
        let d = op12_val.get("d").expect("d object");
        assert_eq!(
            d.get("audio_ssrc").and_then(JsonValue::as_u64),
            Some(40_000)
        );
        assert_eq!(
            d.get("video_ssrc").and_then(JsonValue::as_u64),
            Some(40_001)
        );
        assert_eq!(d.get("rtx_ssrc").and_then(JsonValue::as_u64), Some(40_002));
        let streams = d
            .get("streams")
            .and_then(JsonValue::as_array)
            .expect("streams array");
        assert_eq!(streams.len(), 1);
        let s0 = &streams[0];
        assert_eq!(s0.get("type").and_then(JsonValue::as_str), Some("screen"));
        assert_eq!(s0.get("rid").and_then(JsonValue::as_str), Some("100"));
        assert_eq!(s0.get("quality").and_then(JsonValue::as_u64), Some(100));
        assert_eq!(s0.get("ssrc").and_then(JsonValue::as_u64), Some(40_001));
        assert_eq!(s0.get("rtx_ssrc").and_then(JsonValue::as_u64), Some(40_002));
        assert_eq!(s0.get("active").and_then(JsonValue::as_bool), Some(true));
        assert_eq!(
            s0.get("max_bitrate").and_then(JsonValue::as_u64),
            Some(6_000_000)
        );
        assert_eq!(
            s0.get("max_framerate").and_then(JsonValue::as_u64),
            Some(60)
        );
        let max_res = s0.get("max_resolution").expect("max_resolution");
        assert_eq!(
            max_res.get("type").and_then(JsonValue::as_str),
            Some("fixed")
        );
        assert_eq!(max_res.get("width").and_then(JsonValue::as_u64), Some(1920));
        assert_eq!(
            max_res.get("height").and_then(JsonValue::as_u64),
            Some(1080)
        );

        let parsed_op12 = VoiceVideo::parse_json(&op12_json).expect("roundtrip parse VoiceVideo");
        assert_eq!(parsed_op12.video_ssrc, 40_001);
        assert_eq!(parsed_op12.rtx_ssrc, 40_002);
        assert_eq!(parsed_op12.streams[0].kind, "screen");

        // Формирование и обработка Voice Opcode 15 (Media Sink Wants)
        let wants = MediaSinkWants::any(100).with_ssrc(40_001, 100);
        let op15_out = go_live.send_media_sink_wants(wants.clone());
        let parsed_wants =
            MediaSinkWants::parse_json(&op15_out).expect("parse outgoing OP 15 JSON");
        assert_eq!(parsed_wants, wants);

        go_live
            .handle_ws_frame(r#"{"seq":9,"op":15,"d":{"any":100,"40001":75}}"#)
            .expect("handle incoming OP 15");
        let last_wants = go_live
            .media_sink_wants()
            .expect("recorded incoming MediaSinkWants");
        assert_eq!(last_wants.any, Some(100));
        assert_eq!(last_wants.ssrc_qualities.get(&40_001), Some(&75));

        assert!(primary.go_live_session().is_some());
        assert!(primary.close_go_live_session());
        assert!(primary.go_live_session().is_none());
    }

    #[test]
    fn session_two_layer_dave_e2ee_over_srtp_and_mls_opcodes_21_to_31_dispatch() {
        let (server_a, state_a) = sample_updates();
        let (server_b, state_b) = sample_updates();
        let mut sender = VoiceSessionDriver::new(server_a, state_a).expect("sender");
        let mut receiver = VoiceSessionDriver::new(server_b, state_b).expect("receiver");

        let key_json_array = RAW_SECRET_KEY
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let op4_json = format!(
            r#"{{"op":4,"d":{{"mode":"aead_xchacha20_poly1305_rtpsize","secret_key":[{key_json_array}],"dave_protocol_version":1,"secure_frames_version":1,"media_session_id":"media-sess-1","audio_codec":"opus","video_codec":"H264"}}}}"#
        );

        for drv in [&mut sender, &mut receiver] {
            drv.handle_ws_frame(r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#)
                .expect("OP 8");
            drv.handle_ws_frame(
                r#"{"op":2,"d":{"streams":[],"ssrc":9208,"port":19329,"modes":["aead_xchacha20_poly1305_rtpsize"],"ip":"104.29.146.60","experiments":[]}}"#,
            )
            .expect("OP 2");
            drv.handle_ws_frame(&op4_json).expect("OP 4");
            let _ = drv.take_outgoing_ws();
        }

        // 1. Включаем второй слой шифрования DAVE E2EE поверх SRTP (_rtpsize)
        let dave_epoch_key = [0xABu8; 32];
        sender
            .enable_dave_e2ee(1, &dave_epoch_key)
            .expect("enable sender DAVE");
        receiver
            .enable_dave_e2ee(1, &dave_epoch_key)
            .expect("enable receiver DAVE");
        assert!(sender.is_dave_e2ee_enabled());

        let ext_bytes = [0x90, 0x04, 0x00, 0x00];
        let audio_plain = b"\xf8\xff\xfe\xaa\xbb\xcc\xdd";
        let encrypted_audio = sender
            .encrypt_audio_frame(audio_plain, Some(&ext_bytes))
            .expect("encrypt audio with DAVE+SRTP");

        let decrypted_audio = receiver
            .decrypt_rtp_packet(&encrypted_audio)
            .expect("decrypt audio with SRTP+DAVE");
        assert_eq!(decrypted_audio.extension_profile, Some(0xBEDE));
        assert_eq!(
            decrypted_audio.extension_data.as_deref(),
            Some(&ext_bytes[..]),
            "RTP extension header bytes must be preserved in _rtpsize mode"
        );
        assert_eq!(decrypted_audio.payload, audio_plain);

        // 2. Диспетчеризация всех опкодов DAVE / MLS Voice Gateway (Op 21..=31)
        // Op 21: PrepareTransition -> автоматически отправляет Op 23 TransitionReady для transition_id != 0
        let ev21 = sender
            .handle_dave_json_frame(r#"{"op":21,"d":{"transition_id":7,"protocol_version":1}}"#)
            .expect("Op 21");
        assert_eq!(
            ev21,
            DaveMlsEvent::PrepareTransition(crate::dave::DavePrepareTransition {
                transition_id: 7,
                protocol_version: 1,
            })
        );
        assert_eq!(sender.dave_pending_transition(), Some((7, 1)));
        let auto_op23 = sender.take_outgoing_ws();
        assert_eq!(auto_op23, vec![r#"{"d":{"transition_id":7},"op":23}"#]);

        // Op 22: ExecuteTransition
        let ev22 = sender
            .handle_dave_json_frame(r#"{"op":22,"d":{"transition_id":7}}"#)
            .expect("Op 22");
        assert_eq!(
            ev22,
            DaveMlsEvent::ExecuteTransition(crate::dave::DaveExecuteTransition {
                transition_id: 7
            })
        );
        assert_eq!(sender.dave_last_executed_transition(), Some(7));

        // Op 23: TransitionReady
        let op23_str = sender.send_transition_ready(8);
        let ev23 = sender.handle_dave_json_frame(&op23_str).expect("Op 23");
        assert_eq!(
            ev23,
            DaveMlsEvent::TransitionReady(crate::dave::DaveReadyForTransition { transition_id: 8 })
        );
        let _ = sender.take_outgoing_ws();

        // Op 24: PrepareEpoch
        let ev24 = sender
            .handle_dave_json_frame(r#"{"op":24,"d":{"epoch":1,"protocol_version":1}}"#)
            .expect("Op 24");
        assert_eq!(
            ev24,
            DaveMlsEvent::PrepareEpoch(crate::dave::DavePrepareEpoch {
                epoch: 1,
                protocol_version: 1,
            })
        );
        assert_eq!(sender.dave_epoch(), Some(1));

        // Op 25: ExternalSenderPackage (binary server -> client: [seq_be: u16][25][payload])
        let ev25 = sender
            .handle_mls_binary_frame(&[0x00, 0x0A, 25, 0xDE, 0xAD, 0xBE, 0xEF])
            .expect("Op 25");
        assert_eq!(
            ev25,
            DaveMlsEvent::ExternalSenderPackage {
                seq: 10,
                payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
            }
        );
        assert_eq!(sender.last_seq_ack(), Some(10));
        assert_eq!(
            sender.dave_external_sender(),
            Some(&[0xDE, 0xAD, 0xBE, 0xEF][..])
        );

        // Op 26: KeyPackage (binary client -> server: [26][payload])
        let kp_wire = sender
            .send_mls_key_package(&[0x01, 0x02, 0x03])
            .expect("send Op 26");
        assert_eq!(kp_wire, vec![26, 0x01, 0x02, 0x03]);
        assert_eq!(sender.take_outgoing_binary_ws(), vec![kp_wire]);

        // Op 27: Proposals (binary server -> client: [seq_be: u16][27][payload])
        let ev27 = sender
            .handle_mls_binary_frame(&[0x00, 0x0B, 27, 0x00, 0xAA, 0xBB])
            .expect("Op 27");
        assert_eq!(
            ev27,
            DaveMlsEvent::Proposals {
                seq: 11,
                payload: vec![0x00, 0xAA, 0xBB],
            }
        );
        assert_eq!(sender.last_seq_ack(), Some(11));

        // Op 28: CommitWelcome (binary client -> server: [28][payload])
        let cw_wire = sender
            .send_mls_commit_welcome(&[0xCC, 0xDD])
            .expect("send Op 28");
        assert_eq!(cw_wire, vec![28, 0xCC, 0xDD]);
        assert_eq!(sender.take_outgoing_binary_ws(), vec![cw_wire]);

        // Op 29: AnnounceCommit (binary server -> client: [seq_be: u16][29][transition_id_be: u16][commit])
        let ev29 = sender
            .handle_mls_binary_frame(&[0x00, 0x0C, 29, 0x00, 0x09, 0x11, 0x22, 0x33])
            .expect("Op 29");
        assert_eq!(
            ev29,
            DaveMlsEvent::AnnounceCommit {
                seq: 12,
                transition_id: 9,
                commit: vec![0x11, 0x22, 0x33],
            }
        );
        assert_eq!(sender.last_seq_ack(), Some(12));
        assert_eq!(
            sender.take_outgoing_ws(),
            vec![r#"{"d":{"transition_id":9},"op":23}"#]
        );

        // Op 30: Welcome (binary server -> client: [seq_be: u16][30][transition_id_be: u16][welcome])
        let ev30 = sender
            .handle_mls_binary_frame(&[0x00, 0x0D, 30, 0x00, 0x0A, 0x44, 0x55])
            .expect("Op 30");
        assert_eq!(
            ev30,
            DaveMlsEvent::Welcome {
                seq: 13,
                transition_id: 10,
                welcome: vec![0x44, 0x55],
            }
        );
        assert_eq!(sender.last_seq_ack(), Some(13));
        assert_eq!(
            sender.take_outgoing_ws(),
            vec![r#"{"d":{"transition_id":10},"op":23}"#]
        );

        // Op 31: InvalidCommitWelcome
        let op31_str = sender.send_invalid_commit_welcome(10);
        let ev31 = sender.handle_dave_json_frame(&op31_str).expect("Op 31");
        assert_eq!(
            ev31,
            DaveMlsEvent::InvalidCommitWelcome(crate::dave::DaveInvalidCommitWelcome {
                transition_id: 10
            })
        );
    }
}
