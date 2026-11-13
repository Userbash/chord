#![forbid(unsafe_code)]
//! # discord-voice
//!
//! Голосовой стек: WebSocket RTC-сигнализация, UDP-медиа, SRTP (`aead_aes256_gcm_rtpsize`),
//! keepalive-обмен и рукопожатие DAVE/MLS.
//!
//! Спека: `docs/re/voice-deep.md` §2–3 (сессии s2/s3, константы из strace).
//! План: P4-01 (сигнализация) → P4-02 (UDP/keepalive/ротация) → P4.5 (DAVE/MLS).
//!
//! P4-01: headless RTC signaling. Модуль только разбирает и моделирует данные;
//! транспортных и сетевых операций здесь нет. UDP/медиа/DAVE в эту карточку не входят.

pub mod crypto;
pub mod dave;
pub mod dave_v1;
pub mod devices;
pub mod media;
pub mod noise;
pub mod ptt;
pub mod reception;
pub mod reconnect;
pub mod regions;
pub mod rtcp;
pub mod rtp;
pub mod session;
pub mod srtp;
pub mod stats;
pub mod udp;

pub use noise::{
    KrispModelTier, NoiseBand, NoiseCancellationStats, NoiseCancellerState, NoiseSuppressionConfig,
    NoiseSuppressionError, NoiseSuppressionPipeline, SuppressionBackend, VadGateResult,
};
pub use ptt::{
    AudioModeType, HardwareKeybindAction, HardwarePttKeybindHandler, InputDeviceId,
    InputDeviceType, KeybindError, KeybindEventOptions, KeybindFiredEvent, MicCaptureState,
    MicFrameOutput, MicStateMachine, PttAccessStatus, PttActivationState, PttKeyState,
    ScanCodeCombo, ScanCodeEntry,
};
pub use session::VoiceHeartbeatAck as HeartbeatAck;
pub use session::{
    DecryptedRtcpCompound, DecryptedRtpPacket, DiscoveryResponse, Speaking, SpeakingFlags,
    VoiceHeartbeat, VoiceHeartbeatAck, VoiceResume, VoiceSessionDriver, VoiceSessionError,
    VoiceSessionPhase, VoiceSessionRunner, VoiceStateUpdate, decode_discovery_response,
    encode_discovery_request, encode_discovery_request_with_port, select_encryption_mode,
};
pub use srtp::{DaveMlsEngine, DaveStrippedRtpPacket, DaveTransitionOutcome};

use chord_json::{self, JsonValue};
use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

/// Payload type OPUS для медиа-канала (Select Protocol, priority 1000).
pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;

/// Payload type AV1 (video) из списка кодеков Select Protocol.
pub const RTP_AV1_PAYLOAD_TYPE: u8 = 101;

/// Payload type VP8 (video) для медиа-канала (Select Protocol / WebRTC).
pub const RTP_VP8_PAYLOAD_TYPE: u8 = 107;

/// Payload type VP9 (video) для медиа-канала (WebRTC transport options).
pub const RTP_VP9_PAYLOAD_TYPE: u8 = 109;

/// Режим шифрования из Session Description (s2/s3).
pub const MEDIA_ENCRYPTION_MODE: &str = "aead_aes256_gcm_rtpsize";

/// Версия протокола DAVE (MLS) в Session Description.
pub const DAVE_PROTOCOL_VERSION: u8 = 1;

/// Версия secure frames в Session Description.
pub const SECURE_FRAMES_VERSION: u8 = 1;

/// Интервал heartbeat голосового шлюза (op8 Hello) — 13 750 мс.
pub const VOICE_HEARTBEAT_INTERVAL_MS: u64 = 13_750;

/// Магия keepalive-пакета (Little Endian, из strace-дампов s3).
pub const KEEPALIVE_MAGIC: u32 = 0x1337_cafe;

/// Магия ответа на keepalive.
pub const KEEPALIVE_REPLY: u32 = 0x1337_f00d;

/// Наблюдавшиеся op-коды RTC-сигнализации (union сессий 1–2).
pub const OBSERVED_RTC_OPS: [u8; 9] = [0, 1, 2, 3, 4, 6, 8, 15, 16];

/// RTC signaling operation code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceOp {
    Identify,
    SelectProtocol,
    Ready,
    Heartbeat,
    SessionDescription,
    Speaking,
    HeartbeatAck,
    Resume,
    Hello,
    Resumed,
    ClientsConnect,
    Video,
    ClientDisconnect,
    MediaSinkWants,
    VoiceBackendVersion,
    SecureFramesPrepareProtocolTransition,
    SecureFramesExecuteTransition,
    SecureFramesReadyForTransition,
    SecureFramesPrepareEpoch,
    MlsExternalSenderPackage,
    MlsKeyPackage,
    MlsProposals,
    MlsCommitWelcome,
    MlsPrepareCommitTransition,
    MlsWelcome,
    MlsInvalidCommitWelcome,
    NoRoute,
    Unknown(u8),
}

impl VoiceOp {
    #[must_use]
    pub fn from_u8(raw: u8) -> Self {
        match raw {
            0 => Self::Identify,
            1 => Self::SelectProtocol,
            2 => Self::Ready,
            3 => Self::Heartbeat,
            4 => Self::SessionDescription,
            5 => Self::Speaking,
            6 => Self::HeartbeatAck,
            7 => Self::Resume,
            8 => Self::Hello,
            9 => Self::Resumed,
            11 => Self::ClientsConnect,
            12 => Self::Video,
            13 => Self::ClientDisconnect,
            15 => Self::MediaSinkWants,
            16 => Self::VoiceBackendVersion,
            21 => Self::SecureFramesPrepareProtocolTransition,
            22 => Self::SecureFramesExecuteTransition,
            23 => Self::SecureFramesReadyForTransition,
            24 => Self::SecureFramesPrepareEpoch,
            25 => Self::MlsExternalSenderPackage,
            26 => Self::MlsKeyPackage,
            27 => Self::MlsProposals,
            28 => Self::MlsCommitWelcome,
            29 => Self::MlsPrepareCommitTransition,
            30 => Self::MlsWelcome,
            31 => Self::MlsInvalidCommitWelcome,
            32 => Self::NoRoute,
            value => Self::Unknown(value),
        }
    }

    #[must_use]
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Identify => 0,
            Self::SelectProtocol => 1,
            Self::Ready => 2,
            Self::Heartbeat => 3,
            Self::SessionDescription => 4,
            Self::Speaking => 5,
            Self::HeartbeatAck => 6,
            Self::Resume => 7,
            Self::Hello => 8,
            Self::Resumed => 9,
            Self::ClientsConnect => 11,
            Self::Video => 12,
            Self::ClientDisconnect => 13,
            Self::MediaSinkWants => 15,
            Self::VoiceBackendVersion => 16,
            Self::SecureFramesPrepareProtocolTransition => 21,
            Self::SecureFramesExecuteTransition => 22,
            Self::SecureFramesReadyForTransition => 23,
            Self::SecureFramesPrepareEpoch => 24,
            Self::MlsExternalSenderPackage => 25,
            Self::MlsKeyPackage => 26,
            Self::MlsProposals => 27,
            Self::MlsCommitWelcome => 28,
            Self::MlsPrepareCommitTransition => 29,
            Self::MlsWelcome => 30,
            Self::MlsInvalidCommitWelcome => 31,
            Self::NoRoute => 32,
            Self::Unknown(value) => value,
        }
    }

    /// Проверяет, относится ли опкод к протоколу сквозного шифрования DAVE / MLS (опкоды 21..=31).
    #[must_use]
    pub fn is_dave_or_mls(self) -> bool {
        matches!(self.as_u8(), 21..=31)
    }

    /// Возвращает каноническое строковое имя опкода RTC Voice Gateway.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Identify => "IDENTIFY",
            Self::SelectProtocol => "SELECT_PROTOCOL",
            Self::Ready => "READY",
            Self::Heartbeat => "HEARTBEAT",
            Self::SessionDescription => "SESSION_DESCRIPTION",
            Self::Speaking => "SPEAKING",
            Self::HeartbeatAck => "HEARTBEAT_ACK",
            Self::Resume => "RESUME",
            Self::Hello => "HELLO",
            Self::Resumed => "RESUMED",
            Self::ClientsConnect => "CLIENTS_CONNECT",
            Self::Video => "VIDEO",
            Self::ClientDisconnect => "CLIENT_DISCONNECT",
            Self::MediaSinkWants => "MEDIA_SINK_WANTS",
            Self::VoiceBackendVersion => "VOICE_BACKEND_VERSION",
            Self::SecureFramesPrepareProtocolTransition => {
                "SECURE_FRAMES_PREPARE_PROTOCOL_TRANSITION"
            }
            Self::SecureFramesExecuteTransition => "SECURE_FRAMES_EXECUTE_TRANSITION",
            Self::SecureFramesReadyForTransition => "SECURE_FRAMES_READY_FOR_TRANSITION",
            Self::SecureFramesPrepareEpoch => "SECURE_FRAMES_PREPARE_EPOCH",
            Self::MlsExternalSenderPackage => "MLS_EXTERNAL_SENDER_PACKAGE",
            Self::MlsKeyPackage => "MLS_KEY_PACKAGE",
            Self::MlsProposals => "MLS_PROPOSALS",
            Self::MlsCommitWelcome => "MLS_COMMIT_WELCOME",
            Self::MlsPrepareCommitTransition => "MLS_PREPARE_COMMIT_TRANSITION",
            Self::MlsWelcome => "MLS_WELCOME",
            Self::MlsInvalidCommitWelcome => "MLS_INVALID_COMMIT_WELCOME",
            Self::NoRoute => "NO_ROUTE",
            Self::Unknown(_) => "UNKNOWN",
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.name()
    }
}

impl fmt::Display for VoiceOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Error from parsing or validating a signaling value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceError(String);

impl VoiceError {
    fn invalid(message: &str) -> Self {
        Self(message.to_owned())
    }
}

impl fmt::Display for VoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for VoiceError {}

/// RTC Hello payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub version: u8,
    pub heartbeat_interval_ms: u64,
}

/// Video or audio stream metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSpec {
    pub kind: String,
    pub ssrc: Option<u64>,
    pub rtx_ssrc: Option<u64>,
    pub rid: Option<String>,
    pub quality: Option<u16>,
    pub active: Option<bool>,
}

/// Identify payload. Its credentials are redacted by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Identify {
    pub server_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub session_id: String,
    pub token: String,
    pub max_dave_protocol_version: u8,
    pub video: bool,
    pub streams: Vec<StreamSpec>,
}

impl fmt::Debug for Identify {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identify")
            .field("server_id", &"<redacted>")
            .field("channel_id", &"<redacted>")
            .field("user_id", &"<redacted>")
            .field("session_id", &"<redacted>")
            .field("token", &"<redacted>")
            .field("max_dave_protocol_version", &self.max_dave_protocol_version)
            .field("video", &self.video)
            .field("streams", &self.streams)
            .finish()
    }
}

impl Identify {
    /// Serialize the outgoing op0 frame (`{"op":0,"d":...}`) without redaction.
    #[must_use]
    pub fn to_json(&self) -> String {
        let streams = self
            .streams
            .iter()
            .map(|stream| {
                let mut fields = vec![("type", JsonValue::String(stream.kind.clone()))];
                if let Some(ssrc) = stream.ssrc {
                    fields.push(("ssrc", json_u64(ssrc)));
                }
                if let Some(rtx_ssrc) = stream.rtx_ssrc {
                    fields.push(("rtx_ssrc", json_u64(rtx_ssrc)));
                }
                if let Some(rid) = &stream.rid {
                    fields.push(("rid", JsonValue::String(rid.clone())));
                }
                if let Some(quality) = stream.quality {
                    fields.push(("quality", json_u64(u64::from(quality))));
                }
                if let Some(active) = stream.active {
                    fields.push(("active", JsonValue::Bool(active)));
                }
                object(fields)
            })
            .collect();
        let payload = object(vec![
            ("server_id", JsonValue::String(self.server_id.clone())),
            ("channel_id", JsonValue::String(self.channel_id.clone())),
            ("user_id", JsonValue::String(self.user_id.clone())),
            ("session_id", JsonValue::String(self.session_id.clone())),
            ("token", JsonValue::String(self.token.clone())),
            (
                "max_dave_protocol_version",
                json_u64(u64::from(self.max_dave_protocol_version)),
            ),
            ("video", JsonValue::Bool(self.video)),
            ("streams", JsonValue::Array(streams)),
        ]);
        chord_json::to_json_string(&object(vec![("op", json_u64(0)), ("d", payload)]))
    }
}

/// Select Protocol codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Codec {
    pub name: String,
    pub kind: String,
    pub priority: u64,
    pub payload_type: u8,
    pub rtx_payload_type: Option<u8>,
    pub encode: Option<bool>,
    pub decode: Option<bool>,
}

impl Codec {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        kind: impl Into<String>,
        priority: u64,
        payload_type: u8,
    ) -> Self {
        Self {
            name: name.into(),
            kind: kind.into(),
            priority,
            payload_type,
            rtx_payload_type: None,
            encode: None,
            decode: None,
        }
    }

    #[must_use]
    pub fn with_rtx(mut self, rtx_payload_type: u8, encode: bool, decode: bool) -> Self {
        self.rtx_payload_type = Some(rtx_payload_type);
        self.encode = Some(encode);
        self.decode = Some(decode);
        self
    }

    #[must_use]
    pub fn default_client_codecs() -> Vec<Self> {
        default_client_codecs()
    }
}

/// Возвращает эталонный набор из 5 кодеков клиента Discord:
/// opus 120 (audio), AV1 101/rtx 102, H265 103/rtx 104, H264 105/rtx 106, VP8 107/rtx 108.
#[must_use]
pub fn default_client_codecs() -> Vec<Codec> {
    vec![
        Codec {
            name: "opus".to_owned(),
            kind: "audio".to_owned(),
            priority: 1000,
            payload_type: RTP_OPUS_PAYLOAD_TYPE,
            rtx_payload_type: None,
            encode: None,
            decode: None,
        },
        Codec {
            name: "AV1".to_owned(),
            kind: "video".to_owned(),
            priority: 1000,
            payload_type: RTP_AV1_PAYLOAD_TYPE,
            rtx_payload_type: Some(102),
            encode: Some(true),
            decode: Some(true),
        },
        Codec {
            name: "H265".to_owned(),
            kind: "video".to_owned(),
            priority: 2000,
            payload_type: 103,
            rtx_payload_type: Some(104),
            encode: Some(true),
            decode: Some(true),
        },
        Codec {
            name: "H264".to_owned(),
            kind: "video".to_owned(),
            priority: 3000,
            payload_type: 105,
            rtx_payload_type: Some(106),
            encode: Some(true),
            decode: Some(true),
        },
        Codec {
            name: "VP8".to_owned(),
            kind: "video".to_owned(),
            priority: 4000,
            payload_type: RTP_VP8_PAYLOAD_TYPE,
            rtx_payload_type: Some(108),
            encode: Some(true),
            decode: Some(true),
        },
    ]
}

/// Outgoing Select Protocol payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectProtocol {
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub mode: String,
    pub codecs: Vec<Codec>,
    pub rtc_connection_id: Option<String>,
    pub experiments: Vec<String>,
}

impl SelectProtocol {
    #[must_use]
    pub fn new(
        protocol: impl Into<String>,
        address: impl Into<String>,
        port: u16,
        mode: impl Into<String>,
        codecs: Vec<Codec>,
    ) -> Self {
        Self {
            protocol: protocol.into(),
            address: address.into(),
            port,
            mode: mode.into(),
            codecs,
            rtc_connection_id: None,
            experiments: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_rtc_connection_id(mut self, rtc_connection_id: impl Into<String>) -> Self {
        self.rtc_connection_id = Some(rtc_connection_id.into());
        self
    }

    #[must_use]
    pub fn with_experiments(mut self, experiments: Vec<String>) -> Self {
        self.experiments = experiments;
        self
    }

    /// Serialize the outgoing op1 frame (`{"op":1,"d":...}`) with rtx_payload_type, encode, decode,
    /// rtc_connection_id, and experiments without redaction.
    #[must_use]
    pub fn to_frame_json(&self) -> String {
        let codecs = self
            .codecs
            .iter()
            .map(|codec| {
                let mut fields = vec![
                    ("name", JsonValue::String(codec.name.clone())),
                    ("type", JsonValue::String(codec.kind.clone())),
                    ("priority", json_u64(codec.priority)),
                    ("payload_type", json_u64(u64::from(codec.payload_type))),
                ];
                if let Some(rtx) = codec.rtx_payload_type {
                    fields.push(("rtx_payload_type", json_u64(u64::from(rtx))));
                }
                if let Some(encode) = codec.encode {
                    fields.push(("encode", JsonValue::Bool(encode)));
                }
                if let Some(decode) = codec.decode {
                    fields.push(("decode", JsonValue::Bool(decode)));
                }
                object(fields)
            })
            .collect();
        let data = object(vec![
            ("address", JsonValue::String(self.address.clone())),
            ("port", json_u64(u64::from(self.port))),
            ("mode", JsonValue::String(self.mode.clone())),
        ]);
        let mut d_fields = vec![
            ("protocol", JsonValue::String(self.protocol.clone())),
            ("data", data),
            // RE capture дублирует address/port/mode на верхнем уровне d.
            ("address", JsonValue::String(self.address.clone())),
            ("port", json_u64(u64::from(self.port))),
            ("mode", JsonValue::String(self.mode.clone())),
            ("codecs", JsonValue::Array(codecs)),
        ];
        if let Some(ref rtc_id) = self.rtc_connection_id {
            d_fields.push(("rtc_connection_id", JsonValue::String(rtc_id.clone())));
        }
        if !self.experiments.is_empty() {
            d_fields.push((
                "experiments",
                JsonValue::Array(
                    self.experiments
                        .iter()
                        .map(|s| JsonValue::String(s.clone()))
                        .collect(),
                ),
            ));
        }
        let payload = object(d_fields);
        chord_json::to_json_string(&object(vec![("op", json_u64(1)), ("d", payload)]))
    }

    /// Serialize the outgoing op1 frame (`{"op":1,"d":...}`) without redaction.
    #[must_use]
    pub fn to_json(&self) -> String {
        self.to_frame_json()
    }
}

/// RTC Ready payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub ssrc: u64,
    pub port: u16,
    pub ip: String,
    pub modes: Vec<String>,
    pub experiments: Vec<String>,
    pub streams: Vec<StreamSpec>,
}

/// RTC Session Description payload.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionDescription {
    pub mode: String,
    pub secret_key: Vec<u8>,
    pub media_session_id: String,
    pub dave_protocol_version: u8,
    pub secure_frames_version: u8,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
}

impl fmt::Debug for SessionDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionDescription")
            .field("mode", &self.mode)
            .field("secret_key", &"<redacted>")
            .field("media_session_id", &"<redacted>")
            .field("dave_protocol_version", &self.dave_protocol_version)
            .field("secure_frames_version", &self.secure_frames_version)
            .field("audio_codec", &self.audio_codec)
            .field("video_codec", &self.video_codec)
            .finish()
    }
}

impl fmt::Display for SessionDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionDescription(mode={}, secret_key=<redacted>, media_session_id=<redacted>)",
            self.mode
        )
    }
}

impl SessionDescription {
    /// Возвращает `true`, если согласован видеокодек VP8.
    #[must_use]
    pub fn is_vp8(&self) -> bool {
        self.video_codec
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case("VP8"))
    }

    /// Возвращает `true`, если согласован видеокодек VP9.
    #[must_use]
    pub fn is_vp9(&self) -> bool {
        self.video_codec
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case("VP9"))
    }
}

/// Parsed payload for a voice signaling frame.
///
/// `Other` хранит исходный payload для диагностики, но `Debug` намеренно скрывает его:
/// неизвестный op может содержать token/secret.
#[derive(Clone, PartialEq)]
pub enum VoicePayload {
    Hello(Hello),
    Identify(Identify),
    SelectProtocol(SelectProtocol),
    Ready(Ready),
    SessionDescription(SessionDescription),
    HeartbeatAck(HeartbeatAck),
    Other { op: VoiceOp, raw: JsonValue },
}

impl fmt::Debug for VoicePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hello(value) => formatter.debug_tuple("Hello").field(value).finish(),
            Self::Identify(value) => formatter.debug_tuple("Identify").field(value).finish(),
            Self::SelectProtocol(value) => formatter
                .debug_tuple("SelectProtocol")
                .field(value)
                .finish(),
            Self::Ready(value) => formatter.debug_tuple("Ready").field(value).finish(),
            Self::SessionDescription(value) => formatter
                .debug_tuple("SessionDescription")
                .field(value)
                .finish(),
            Self::HeartbeatAck(value) => {
                formatter.debug_tuple("HeartbeatAck").field(value).finish()
            }
            Self::Other { op, raw: _ } => formatter
                .debug_struct("Other")
                .field("op", op)
                .field("raw", &"<redacted>")
                .finish(),
        }
    }
}

/// Parsed RTC signaling frame.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceFrame {
    pub op: VoiceOp,
    pub payload: VoicePayload,
    pub seq: Option<u64>,
}

/// Parse an internal RTC payload of the form `{ "op": ..., "d": ... }`.
pub fn parse_frame(input: &str) -> Result<VoiceFrame, VoiceError> {
    let root = chord_json::parse(input).map_err(|_| VoiceError::invalid("invalid voice JSON"))?;
    let raw_op = required_u64(&root, "op")?
        .try_into()
        .map_err(|_| VoiceError::invalid("invalid voice op"))?;
    let op = VoiceOp::from_u8(raw_op);
    let data = root
        .get("d")
        .ok_or_else(|| VoiceError::invalid("missing voice payload"))?;
    let seq = optional_u64(&root, "seq")?;
    let payload = match op {
        VoiceOp::Hello => VoicePayload::Hello(Hello {
            version: required_u64(data, "v")?
                .try_into()
                .map_err(|_| VoiceError::invalid("invalid Hello version"))?,
            heartbeat_interval_ms: required_u64(data, "heartbeat_interval")?,
        }),
        VoiceOp::Identify => VoicePayload::Identify(parse_identify(data)?),
        VoiceOp::SelectProtocol => VoicePayload::SelectProtocol(parse_select_protocol(data)?),
        VoiceOp::Ready => VoicePayload::Ready(parse_ready(data)?),
        VoiceOp::SessionDescription => {
            VoicePayload::SessionDescription(parse_session_description(data)?)
        }
        VoiceOp::HeartbeatAck => VoicePayload::HeartbeatAck(HeartbeatAck {
            t: required_u64(data, "t")?,
        }),
        other => VoicePayload::Other {
            op: other,
            raw: data.clone(),
        },
    };
    Ok(VoiceFrame { op, payload, seq })
}

fn parse_identify(data: &JsonValue) -> Result<Identify, VoiceError> {
    let max_dave_protocol_version = required_u64(data, "max_dave_protocol_version")?
        .try_into()
        .map_err(|_| VoiceError::invalid("invalid DAVE version"))?;
    let video = required_bool(data, "video")?;
    if max_dave_protocol_version != 1 || !video {
        return Err(VoiceError::invalid("unsupported Identify parameters"));
    }
    Ok(Identify {
        server_id: nonempty_string(data, "server_id")?,
        channel_id: nonempty_string(data, "channel_id")?,
        user_id: nonempty_string(data, "user_id")?,
        session_id: nonempty_string(data, "session_id")?,
        token: nonempty_string(data, "token")?,
        max_dave_protocol_version,
        video,
        streams: parse_streams(data.get("streams"))?,
    })
}

fn parse_select_protocol(data: &JsonValue) -> Result<SelectProtocol, VoiceError> {
    let protocol = nonempty_string(data, "protocol")?;
    let nested = data
        .get("data")
        .ok_or_else(|| VoiceError::invalid("missing protocol data"))?;
    let codecs_value = data
        .get("codecs")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| VoiceError::invalid("missing codecs"))?;
    let codecs = codecs_value
        .iter()
        .map(|codec| {
            let rtx_payload_type = match codec.get("rtx_payload_type") {
                Some(val) => {
                    let raw = val
                        .as_u64()
                        .ok_or_else(|| VoiceError::invalid("invalid rtx_payload_type"))?;
                    Some(
                        raw.try_into()
                            .map_err(|_| VoiceError::invalid("invalid rtx_payload_type range"))?,
                    )
                }
                None => None,
            };
            let encode = codec.get("encode").and_then(JsonValue::as_bool);
            let decode = codec.get("decode").and_then(JsonValue::as_bool);
            Ok(Codec {
                name: nonempty_string(codec, "name")?,
                kind: nonempty_string(codec, "type")?,
                priority: required_u64(codec, "priority")?,
                payload_type: required_u64(codec, "payload_type")?
                    .try_into()
                    .map_err(|_| VoiceError::invalid("invalid codec payload type"))?,
                rtx_payload_type,
                encode,
                decode,
            })
        })
        .collect::<Result<Vec<_>, VoiceError>>()?;
    if !codecs
        .iter()
        .any(|codec| codec.name == "opus" && codec.payload_type == 120 && codec.priority == 1000)
        || !codecs
            .iter()
            .any(|codec| codec.name == "AV1" && codec.payload_type == 101)
    {
        return Err(VoiceError::invalid("required codecs missing"));
    }
    let rtc_connection_id = optional_string(data, "rtc_connection_id")?;
    let experiments = match data.get("experiments") {
        Some(JsonValue::Array(items)) => {
            let mut list = Vec::with_capacity(items.len());
            for item in items {
                list.push(
                    item.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| VoiceError::invalid("invalid experiment item"))?,
                );
            }
            list
        }
        Some(_) => return Err(VoiceError::invalid("experiments must be an array")),
        None => Vec::new(),
    };
    Ok(SelectProtocol {
        protocol,
        address: nonempty_string(nested, "address")?,
        port: required_u64(nested, "port")?
            .try_into()
            .map_err(|_| VoiceError::invalid("invalid protocol port"))?,
        mode: nonempty_string(nested, "mode")?,
        codecs,
        rtc_connection_id,
        experiments,
    })
}

fn parse_ready(data: &JsonValue) -> Result<Ready, VoiceError> {
    Ok(Ready {
        ssrc: required_u64(data, "ssrc")?,
        port: required_u64(data, "port")?
            .try_into()
            .map_err(|_| VoiceError::invalid("invalid Ready port"))?,
        ip: nonempty_string(data, "ip")?,
        modes: required_strings(data, "modes")?,
        experiments: required_strings(data, "experiments")?,
        streams: parse_streams(data.get("streams"))?,
    })
}

fn parse_session_description(data: &JsonValue) -> Result<SessionDescription, VoiceError> {
    let secret_key = match data.get("secret_key") {
        Some(JsonValue::Array(items)) => {
            let mut bytes = Vec::with_capacity(items.len());
            for item in items {
                let raw = item
                    .as_u64()
                    .ok_or_else(|| VoiceError::invalid("invalid secret_key byte"))?;
                let byte =
                    u8::try_from(raw).map_err(|_| VoiceError::invalid("secret_key byte > 255"))?;
                bytes.push(byte);
            }
            bytes
        }
        _ => decode_base64(&nonempty_string(data, "secret_key")?)?,
    };
    if secret_key.len() != 32 {
        return Err(VoiceError::invalid("secret_key must decode to 32 bytes"));
    }
    let media_session_id = optional_string(data, "media_session_id")?
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".to_owned());
    let secure_frames_version = match optional_u64(data, "secure_frames_version")? {
        Some(raw) => raw
            .try_into()
            .map_err(|_| VoiceError::invalid("invalid secure frames version"))?,
        None => SECURE_FRAMES_VERSION,
    };
    Ok(SessionDescription {
        mode: nonempty_string(data, "mode")?,
        secret_key,
        media_session_id,
        dave_protocol_version: required_u64(data, "dave_protocol_version")?
            .try_into()
            .map_err(|_| VoiceError::invalid("invalid DAVE version"))?,
        secure_frames_version,
        audio_codec: optional_string(data, "audio_codec")?,
        video_codec: optional_string(data, "video_codec")?,
    })
}

fn parse_streams(value: Option<&JsonValue>) -> Result<Vec<StreamSpec>, VoiceError> {
    let values = value
        .and_then(JsonValue::as_array)
        .ok_or_else(|| VoiceError::invalid("missing streams"))?;
    values
        .iter()
        .map(|stream| {
            Ok(StreamSpec {
                kind: stream
                    .get("type")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .to_owned(),
                ssrc: optional_u64(stream, "ssrc")?,
                rtx_ssrc: optional_u64(stream, "rtx_ssrc")?,
                rid: optional_string(stream, "rid")?,
                quality: optional_u64(stream, "quality")?
                    .map(TryInto::try_into)
                    .transpose()
                    .map_err(|_| VoiceError::invalid("invalid stream quality"))?,
                active: optional_bool(stream, "active")?,
            })
        })
        .collect()
}

fn required_u64(value: &JsonValue, key: &str) -> Result<u64, VoiceError> {
    value
        .get(key)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| VoiceError::invalid("missing or invalid integer field"))
}
fn optional_u64(value: &JsonValue, key: &str) -> Result<Option<u64>, VoiceError> {
    value
        .get(key)
        .map(|field| {
            field
                .as_u64()
                .ok_or_else(|| VoiceError::invalid("invalid integer field"))
        })
        .transpose()
}
fn required_bool(value: &JsonValue, key: &str) -> Result<bool, VoiceError> {
    value
        .get(key)
        .and_then(JsonValue::as_bool)
        .ok_or_else(|| VoiceError::invalid("missing or invalid boolean field"))
}
fn optional_bool(value: &JsonValue, key: &str) -> Result<Option<bool>, VoiceError> {
    value
        .get(key)
        .map(|field| {
            field
                .as_bool()
                .ok_or_else(|| VoiceError::invalid("invalid boolean field"))
        })
        .transpose()
}
fn nonempty_string(value: &JsonValue, key: &str) -> Result<String, VoiceError> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| VoiceError::invalid("missing or empty string field"))
}
fn optional_string(value: &JsonValue, key: &str) -> Result<Option<String>, VoiceError> {
    value
        .get(key)
        .map(|field| {
            field
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| VoiceError::invalid("invalid string field"))
        })
        .transpose()
}
fn required_strings(value: &JsonValue, key: &str) -> Result<Vec<String>, VoiceError> {
    value
        .get(key)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| VoiceError::invalid("missing string array"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| VoiceError::invalid("invalid string array item"))
        })
        .collect()
}

fn decode_base64(input: &str) -> Result<Vec<u8>, VoiceError> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(VoiceError::invalid("invalid base64 key"));
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, chunk) in bytes.chunks(4).enumerate() {
        let last = index + 1 == bytes.len() / 4;
        if chunk.len() != 4 {
            return Err(VoiceError::invalid("invalid base64 key"));
        }
        let a =
            base64_value(chunk[0]).ok_or_else(|| VoiceError::invalid("invalid base64 key"))? as u32;
        let b =
            base64_value(chunk[1]).ok_or_else(|| VoiceError::invalid("invalid base64 key"))? as u32;
        let c_pad = chunk[2] == b'=';
        let d_pad = chunk[3] == b'=';
        if c_pad && (!d_pad || !last) || d_pad && !last {
            return Err(VoiceError::invalid("invalid base64 key"));
        }
        let c = if c_pad {
            0
        } else {
            base64_value(chunk[2]).ok_or_else(|| VoiceError::invalid("invalid base64 key"))? as u32
        };
        let d = if d_pad {
            0
        } else {
            base64_value(chunk[3]).ok_or_else(|| VoiceError::invalid("invalid base64 key"))? as u32
        };
        if (c_pad && b & 0x0f != 0) || (d_pad && !c_pad && c & 0x03 != 0) {
            return Err(VoiceError::invalid("invalid base64 key"));
        }
        let bits = (a << 18) | (b << 12) | (c << 6) | d;
        output.push((bits >> 16) as u8);
        if !c_pad {
            output.push((bits >> 8) as u8);
        }
        if !d_pad {
            output.push(bits as u8);
        }
    }
    Ok(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Gateway `VOICE_SERVER_UPDATE`, with token redacted from `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct VoiceServerUpdate {
    pub token: String,
    pub guild_id: String,
    pub endpoint: String,
}

impl fmt::Debug for VoiceServerUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceServerUpdate")
            .field("token", &"<redacted>")
            .field("guild_id", &"<redacted>")
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

impl VoiceServerUpdate {
    pub fn parse(input: &str) -> Result<Self, VoiceError> {
        let root = chord_json::parse(input)
            .map_err(|_| VoiceError::invalid("invalid voice server JSON"))?;
        let data = root.get("d").unwrap_or(&root);
        let update = Self {
            token: nonempty_string(data, "token")?,
            guild_id: nonempty_string(data, "guild_id")?,
            endpoint: nonempty_string(data, "endpoint")?,
        };
        update.validate_endpoint()?;
        Ok(update)
    }

    pub fn websocket_url(&self) -> Result<String, VoiceError> {
        self.validate_endpoint()?;
        let host = self
            .endpoint
            .strip_prefix("wss://")
            .unwrap_or(&self.endpoint);
        let host = host
            .strip_suffix("/?v=9")
            .unwrap_or(host)
            .trim_end_matches('/');
        Ok(format!("wss://{host}/?v=9"))
    }

    fn validate_endpoint(&self) -> Result<(), VoiceError> {
        let endpoint = &self.endpoint;
        if endpoint.chars().any(char::is_whitespace) || endpoint.chars().any(char::is_control) {
            return Err(VoiceError::invalid("invalid voice endpoint"));
        }
        let host = if let Some(host) = endpoint.strip_prefix("wss://") {
            host.strip_suffix("/?v=9")
                .unwrap_or(host)
                .trim_end_matches('/')
        } else {
            if endpoint.contains("://") {
                return Err(VoiceError::invalid("unsupported voice endpoint scheme"));
            }
            endpoint
        };
        if host.is_empty()
            || host.contains(['/', '@', '?', '#'])
            || host.starts_with(':')
            || host.ends_with(':')
        {
            return Err(VoiceError::invalid("invalid voice endpoint host"));
        }
        Ok(())
    }
}

/// Choose a region by measured latency, geographic order, then lexical order.
pub fn select_region(
    samples: &[(String, u32)],
    geo: &[String],
    manual: Option<&str>,
) -> Result<String, VoiceError> {
    if let Some(region) = manual {
        if geo.iter().any(|item| item == region) || samples.iter().any(|(item, _)| item == region) {
            return Ok(region.to_owned());
        }
        return Err(VoiceError::invalid("manual region is unavailable"));
    }
    if samples.is_empty() {
        return geo
            .first()
            .cloned()
            .ok_or_else(|| VoiceError::invalid("no voice regions available"));
    }
    let mut best: BTreeMap<&str, u32> = BTreeMap::new();
    for (region, latency) in samples {
        best.entry(region)
            .and_modify(|current| *current = (*current).min(*latency))
            .or_insert(*latency);
    }
    best.into_iter()
        .min_by(|(left, left_latency), (right, right_latency)| {
            left_latency
                .cmp(right_latency)
                .then_with(|| {
                    geo.iter()
                        .position(|item| item == left)
                        .unwrap_or(usize::MAX)
                        .cmp(
                            &geo.iter()
                                .position(|item| item == right)
                                .unwrap_or(usize::MAX),
                        )
                })
                .then_with(|| left.cmp(right))
        })
        .map(|(region, _)| region.to_owned())
        .ok_or_else(|| VoiceError::invalid("no voice regions available"))
}

fn json_u64(value: u64) -> JsonValue {
    JsonValue::Number(chord_json::JsonNumber::from_u64(value))
}

fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

// ============================================================================
// V1: Конечный автомат звонка `VoiceCallStateMachine`
// ============================================================================

/// Окно коалесценции быстрых переключений `self_mute` / `self_deaf` в мс (`120ms`).
pub const STATE_COALESCE_MS: u64 = 120;

/// Окно коалесценции `STATE_COALESCE = 120ms` (`Duration::from_millis(120)`).
pub const STATE_COALESCE: Duration = Duration::from_millis(STATE_COALESCE_MS);

/// Таймаут установки/подтверждения голосовой сессии до `op:4 SessionDescription` (`15s`).
pub const VOICE_CALL_NEGOTIATION_TIMEOUT_MS: u64 = 15_000;

/// Состояния конечного автомата голосового звонка (этап V1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceCallState {
    /// Нет активного голосового звонка.
    Idle,
    /// Отправлен Gateway `op:4`, ожидается прохождение барьера `VOICE_STATE_UPDATE` + `VOICE_SERVER_UPDATE`.
    Signaling,
    /// Барьер шлюза пройден (`session_id` + `endpoint` + `token`), идёт рукопожатие RTC WS / UDP до `op:4 SessionDescription`.
    NegotiatingTransport,
    /// Получен `op:4 SessionDescription` для актуальной `negotiation_revision`; сессия подтверждена (`session_confirmed = true`).
    Connected,
    /// Отправлен запрос на выход из голосового канала (`channel_id: null`), ожидается подтверждение `VOICE_STATE_UPDATE`.
    Departing,
}

/// Действие очистки при таймауте переговоров звонка (`15s` без `op:4 SessionDescription`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceCleanupAction {
    /// Таймаут не наступил или сессия уже подтверждена.
    None,
    /// Необходимо отправить в основной Gateway `op:4` (`channel_id: null`), чтобы сбросить фантомную сессию.
    SendGatewayLeaveOp4,
}

/// Конечный автомат голосового звонка (этап V1, эталон Serein `crates/discord-gateway/src/voice.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCallStateMachine {
    user_id: String,
    state: VoiceCallState,
    guild_id: Option<String>,
    channel_id: Option<String>,
    session_id: Option<String>,
    endpoint: Option<String>,
    token: Option<String>,
    rtc_server_id: Option<String>,
    attempt_id: u64,
    negotiation_revision: u64,
    last_barrier_emitted_revision: Option<u64>,
    session_confirmed: bool,
    negotiation_started_at_ms: Option<u64>,
    muted: bool,
    deafened: bool,
    sent_muted: bool,
    sent_deafened: bool,
    last_state_sent_ms: Option<u64>,
    state_deadline_ms: Option<u64>,
    last_cleanup_op4_json: Option<String>,
}

impl VoiceCallStateMachine {
    /// Создаёт новый конечный автомат звонка в состоянии `Idle` для пользователя `user_id`.
    #[must_use]
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            state: VoiceCallState::Idle,
            guild_id: None,
            channel_id: None,
            session_id: None,
            endpoint: None,
            token: None,
            rtc_server_id: None,
            attempt_id: 0,
            negotiation_revision: 0,
            last_barrier_emitted_revision: None,
            session_confirmed: false,
            negotiation_started_at_ms: None,
            muted: false,
            deafened: false,
            sent_muted: false,
            sent_deafened: false,
            last_state_sent_ms: None,
            state_deadline_ms: None,
            last_cleanup_op4_json: None,
        }
    }

    /// Текущее состояние автомата звонка.
    #[must_use]
    pub const fn state(&self) -> VoiceCallState {
        self.state
    }

    /// Монотонно возрастающая ревизия переговоров (`negotiation_revision`).
    #[must_use]
    pub const fn negotiation_revision(&self) -> u64 {
        self.negotiation_revision
    }

    /// Признак подтверждения сессии (`true` **только** после получения `op:4 SessionDescription`).
    #[must_use]
    pub const fn session_confirmed(&self) -> bool {
        self.session_confirmed
    }

    /// Идентификатор текущей попытки подключения.
    #[must_use]
    pub const fn attempt_id(&self) -> u64 {
        self.attempt_id
    }

    /// Целевой `guild_id` активного/устанавливаемого звонка.
    #[must_use]
    pub fn guild_id(&self) -> Option<&str> {
        self.guild_id.as_deref()
    }

    /// Целевой `channel_id` активного/устанавливаемого звонка.
    #[must_use]
    pub fn channel_id(&self) -> Option<&str> {
        self.channel_id.as_deref()
    }

    /// Текущий `session_id` из `VOICE_STATE_UPDATE`.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Текущий `endpoint` из `VOICE_SERVER_UPDATE`.
    #[must_use]
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    /// Текущий `token` из `VOICE_SERVER_UPDATE`.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Вычисленный `server_id` для RTC `Identify` (`guild_id` либо `channel_id` для DM-звонков).
    #[must_use]
    pub fn rtc_server_id(&self) -> Option<&str> {
        self.rtc_server_id
            .as_deref()
            .or(self.guild_id.as_deref())
            .or(self.channel_id.as_deref())
    }

    /// Желаемое состояние `self_mute`.
    #[must_use]
    pub const fn muted(&self) -> bool {
        self.muted
    }

    /// Желаемое состояние `self_deaf`.
    #[must_use]
    pub const fn deafened(&self) -> bool {
        self.deafened
    }

    /// Дедлайн отложенной отправки коалесцированного `op:4` (в миллисекундах).
    #[must_use]
    pub const fn state_deadline_ms(&self) -> Option<u64> {
        self.state_deadline_ms
    }

    /// Забирает подготовленный JSON-кадр Gateway `op:4` для очистки после таймаута `check_timeout`.
    pub fn take_last_cleanup_op4_json(&mut self) -> Option<String> {
        self.last_cleanup_op4_json.take()
    }

    /// Инициирует вход в голосовой канал: переводит автомат в `Signaling`, сбрасывает барьер
    /// и возвращает JSON-строку Gateway `op:4 Voice State Update`.
    pub fn start_join(
        &mut self,
        guild_id: Option<String>,
        channel_id: String,
        attempt_id: u64,
        muted: bool,
        deafened: bool,
        now_ms: u64,
    ) -> Result<String, VoiceError> {
        if channel_id.trim().is_empty() {
            return Err(VoiceError::invalid("channel_id must not be empty"));
        }
        if let Some(ref gid) = guild_id
            && gid.trim().is_empty()
        {
            return Err(VoiceError::invalid(
                "guild_id must not be empty when present",
            ));
        }
        let effective_muted = muted || deafened;
        self.state = VoiceCallState::Signaling;
        self.guild_id = guild_id;
        self.channel_id = Some(channel_id.clone());
        self.session_id = None;
        self.endpoint = None;
        self.token = None;
        self.rtc_server_id = None;
        self.attempt_id = attempt_id;
        self.last_barrier_emitted_revision = None;
        self.session_confirmed = false;
        self.negotiation_started_at_ms = Some(now_ms);
        self.muted = effective_muted;
        self.deafened = deafened;
        self.sent_muted = effective_muted;
        self.sent_deafened = deafened;
        self.last_state_sent_ms = Some(now_ms);
        self.state_deadline_ms = None;
        self.last_cleanup_op4_json = None;

        Ok(encode_gateway_voice_state_op4(
            self.guild_id.as_deref(),
            Some(&channel_id),
            effective_muted,
            deafened,
        ))
    }

    /// Обрабатывает событие `VOICE_STATE_UPDATE` из основного шлюза Discord.
    ///
    /// Возвращает `Ok(Some(revision))`, если оба условия барьера (`VOICE_STATE_UPDATE` + `VOICE_SERVER_UPDATE`)
    /// выполнены и ревизия `negotiation_revision` обновилась.
    pub fn on_voice_state_update(
        &mut self,
        update: &VoiceStateUpdate,
    ) -> Result<Option<u64>, VoiceError> {
        if update.user_id != self.user_id {
            return Ok(None);
        }

        if self.state == VoiceCallState::Departing {
            if update.channel_id.is_none() {
                self.reset_to_idle();
            }
            return Ok(None);
        }

        if self.state == VoiceCallState::Idle {
            return Ok(None);
        }

        // Если сервер отключил пользователя от канала (channel_id == None) для нашей гильдии
        if update.channel_id.is_none() && update.guild_id == self.guild_id {
            self.reset_to_idle();
            return Ok(None);
        }

        if update.channel_id != self.channel_id || update.guild_id != self.guild_id {
            return Ok(None);
        }

        if update.session_id.trim().is_empty() {
            return Err(VoiceError::invalid(
                "VOICE_STATE_UPDATE session_id is empty",
            ));
        }

        if self.session_id.as_deref() != Some(update.session_id.as_str()) {
            self.session_id = Some(update.session_id.clone());
            self.negotiation_revision = self.negotiation_revision.saturating_add(1);
            self.session_confirmed = false;
            if self.state == VoiceCallState::Connected {
                self.state = VoiceCallState::NegotiatingTransport;
            }
        }

        Ok(self.try_complete_barrier())
    }

    /// Обрабатывает событие `VOICE_SERVER_UPDATE` из основного шлюза Discord.
    ///
    /// Возвращает `Ok(Some(revision))`, если оба условия барьера (`VOICE_STATE_UPDATE` + `VOICE_SERVER_UPDATE`)
    /// выполнены и ревизия `negotiation_revision` обновилась.
    pub fn on_voice_server_update(
        &mut self,
        update: &VoiceServerUpdate,
    ) -> Result<Option<u64>, VoiceError> {
        if matches!(self.state, VoiceCallState::Idle | VoiceCallState::Departing) {
            return Ok(None);
        }

        if let Some(expected_guild) = self.guild_id.as_deref()
            && expected_guild != update.guild_id
        {
            return Ok(None);
        }

        let _ = update.websocket_url()?;

        let changed = self.endpoint.as_deref() != Some(update.endpoint.as_str())
            || self.token.as_deref() != Some(update.token.as_str());
        if changed {
            self.endpoint = Some(update.endpoint.clone());
            self.token = Some(update.token.clone());
            self.rtc_server_id = Some(update.guild_id.clone());
            self.negotiation_revision = self.negotiation_revision.saturating_add(1);
            self.session_confirmed = false;
            if self.state == VoiceCallState::Connected {
                self.state = VoiceCallState::NegotiatingTransport;
            }
        }

        Ok(self.try_complete_barrier())
    }

    /// Подтверждает голосовую сессию строго после получения `op:4 SessionDescription`
    /// для актуальной `negotiation_revision`.
    pub fn on_session_description_op4(
        &mut self,
        revision: u64,
        desc: &SessionDescription,
    ) -> Result<(), VoiceError> {
        if !matches!(
            self.state,
            VoiceCallState::NegotiatingTransport | VoiceCallState::Connected
        ) {
            return Err(VoiceError::invalid(
                "op:4 SessionDescription rejected before VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE barrier",
            ));
        }
        if self.session_id.is_none() || self.endpoint.is_none() || self.token.is_none() {
            return Err(VoiceError::invalid(
                "voice barrier incomplete when confirming op:4 SessionDescription",
            ));
        }
        if revision == 0 || revision != self.negotiation_revision {
            return Err(VoiceError::invalid(
                "stale or mismatched negotiation_revision for op:4 SessionDescription",
            ));
        }
        if desc.secret_key.len() != 32 || desc.mode.trim().is_empty() {
            return Err(VoiceError::invalid("invalid SessionDescription in op:4"));
        }

        self.session_confirmed = true;
        self.state = VoiceCallState::Connected;
        self.negotiation_started_at_ms = None;
        Ok(())
    }

    /// Обновляет желаемое состояние `self_mute` / `self_deaf` с коалесценцией `STATE_COALESCE = 120ms`.
    ///
    /// Если с момента последней отправки прошло `>= 120ms`, немедленно возвращает JSON Gateway `op:4`.
    /// Иначе планирует `state_deadline_ms` и возвращает `None` до вызова `flush_state`.
    pub fn set_mute_deaf(&mut self, muted: bool, deafened: bool, now_ms: u64) -> Option<String> {
        let effective_muted = muted || deafened;
        self.muted = effective_muted;
        self.deafened = deafened;

        if matches!(self.state, VoiceCallState::Idle | VoiceCallState::Departing)
            || self.channel_id.is_none()
        {
            return None;
        }

        if (self.muted, self.deafened) == (self.sent_muted, self.sent_deafened) {
            self.state_deadline_ms = None;
            return None;
        }

        if let Some(last_sent) = self.last_state_sent_ms {
            let elapsed = now_ms.saturating_sub(last_sent);
            if elapsed < STATE_COALESCE_MS {
                let deadline = last_sent.saturating_add(STATE_COALESCE_MS);
                if self.state_deadline_ms.is_none_or(|d| deadline < d) {
                    self.state_deadline_ms = Some(deadline);
                }
                return None;
            }
        }

        self.commit_state_send(now_ms)
    }

    /// Отправляет отложенное коалесцированное обновление `self_mute` / `self_deaf`, если наступил `state_deadline_ms`.
    pub fn flush_state(&mut self, now_ms: u64) -> Option<String> {
        if self
            .state_deadline_ms
            .is_none_or(|deadline| now_ms < deadline)
        {
            return None;
        }
        if matches!(self.state, VoiceCallState::Idle | VoiceCallState::Departing)
            || self.channel_id.is_none()
        {
            self.state_deadline_ms = None;
            return None;
        }
        if (self.muted, self.deafened) == (self.sent_muted, self.sent_deafened) {
            self.state_deadline_ms = None;
            return None;
        }
        self.commit_state_send(now_ms)
    }

    /// Откатывает признак отправленного состояния при ошибке записи в Gateway-сокет,
    /// чтобы следующий `flush_state` повторил отправку.
    pub fn state_send_failed(&mut self, now_ms: u64) {
        self.sent_muted = !self.muted;
        self.sent_deafened = !self.deafened;
        self.state_deadline_ms = Some(now_ms);
    }

    /// Проверяет 15-секундный таймаут переговоров (`VOICE_CALL_NEGOTIATION_TIMEOUT_MS`).
    ///
    /// Если сессия не получила `op:4 SessionDescription` за 15 секунд, переводит автомат в `Idle`,
    /// подготавливает Gateway `op:4` (`channel_id: null`) и возвращает `VoiceCleanupAction::SendGatewayLeaveOp4`.
    pub fn check_timeout(&mut self, now_ms: u64) -> VoiceCleanupAction {
        if self.session_confirmed
            || !matches!(
                self.state,
                VoiceCallState::Signaling | VoiceCallState::NegotiatingTransport
            )
        {
            return VoiceCleanupAction::None;
        }

        let Some(started_ms) = self.negotiation_started_at_ms else {
            return VoiceCleanupAction::None;
        };
        if now_ms.saturating_sub(started_ms) < VOICE_CALL_NEGOTIATION_TIMEOUT_MS {
            return VoiceCleanupAction::None;
        }

        let cleanup_json =
            encode_gateway_voice_state_op4(self.guild_id.as_deref(), None, false, false);
        self.reset_to_idle();
        self.last_cleanup_op4_json = Some(cleanup_json);
        VoiceCleanupAction::SendGatewayLeaveOp4
    }

    /// Инициирует штатный выход из голосового канала (`Connected | Signaling | NegotiatingTransport -> Departing`).
    pub fn leave(&mut self, now_ms: u64) -> Option<String> {
        if matches!(self.state, VoiceCallState::Idle | VoiceCallState::Departing) {
            return None;
        }
        let leave_json = encode_gateway_voice_state_op4(self.guild_id.as_deref(), None, true, true);
        self.state = VoiceCallState::Departing;
        self.session_confirmed = false;
        self.channel_id = None;
        self.endpoint = None;
        self.token = None;
        self.negotiation_started_at_ms = Some(now_ms);
        self.state_deadline_ms = None;
        Some(leave_json)
    }

    fn try_complete_barrier(&mut self) -> Option<u64> {
        if self.session_id.is_some() && self.endpoint.is_some() && self.token.is_some() {
            if self.state == VoiceCallState::Signaling {
                self.state = VoiceCallState::NegotiatingTransport;
            }
            if self.last_barrier_emitted_revision != Some(self.negotiation_revision) {
                self.last_barrier_emitted_revision = Some(self.negotiation_revision);
                return Some(self.negotiation_revision);
            }
        }
        None
    }

    fn commit_state_send(&mut self, now_ms: u64) -> Option<String> {
        let channel_id = self.channel_id.as_deref()?;
        self.state_deadline_ms = None;
        self.last_state_sent_ms = Some(now_ms);
        self.sent_muted = self.muted;
        self.sent_deafened = self.deafened;
        Some(encode_gateway_voice_state_op4(
            self.guild_id.as_deref(),
            Some(channel_id),
            self.muted,
            self.deafened,
        ))
    }

    fn reset_to_idle(&mut self) {
        self.state = VoiceCallState::Idle;
        self.guild_id = None;
        self.channel_id = None;
        self.session_id = None;
        self.endpoint = None;
        self.token = None;
        self.rtc_server_id = None;
        self.session_confirmed = false;
        self.negotiation_started_at_ms = None;
        self.state_deadline_ms = None;
    }
}

fn encode_gateway_voice_state_op4(
    guild_id: Option<&str>,
    channel_id: Option<&str>,
    self_mute: bool,
    self_deaf: bool,
) -> String {
    let guild_val = guild_id.map_or(JsonValue::Null, |id| JsonValue::String(id.to_owned()));
    let channel_val = channel_id.map_or(JsonValue::Null, |id| JsonValue::String(id.to_owned()));
    let d = object(vec![
        ("guild_id", guild_val),
        ("channel_id", channel_val),
        ("self_mute", JsonValue::Bool(self_mute)),
        ("self_deaf", JsonValue::Bool(self_deaf)),
    ]);
    chord_json::to_json_string(&object(vec![("op", json_u64(4)), ("d", d)]))
}

// ============================================================================
// V2: Трекер `SeqAckTracker` (JSON `seq` + 2-байтный BE заголовок бинарных DAVE-фреймов)
// ============================================================================

/// Трекер порядкового номера `seq_ack: Option<u16>` голосового WebSocket v8 (этап V2).
///
/// Обновляет `seq_ack` как из текстовых JSON-фреймов (верхнеуровневое поле `"seq"`),
/// так и из 2-байтного Big-Endian заголовка входящих бинарных DAVE-фреймов (`op: 25, 27, 29, 30`),
/// и формирует кадры `op:3 Heartbeat` и `op:7 Resume`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeqAckTracker {
    seq_ack: Option<u16>,
}

impl SeqAckTracker {
    /// Создаёт новый трекер `SeqAckTracker` с `seq_ack = None`.
    #[must_use]
    pub const fn new() -> Self {
        Self { seq_ack: None }
    }

    /// Возвращает последнее подтверждённое значение `seq_ack`.
    #[must_use]
    pub const fn seq_ack(&self) -> Option<u16> {
        self.seq_ack
    }

    /// Извлекает верхнеуровневое поле `"seq"` из текстового JSON-кадра RTC WebSocket v8
    /// и обновляет `seq_ack`.
    pub fn update_from_json(&mut self, raw: &str) -> Result<Option<u16>, VoiceError> {
        let root =
            chord_json::parse(raw).map_err(|_| VoiceError::invalid("invalid voice JSON frame"))?;
        let Some(seq_field) = root.get("seq") else {
            return Ok(None);
        };
        if matches!(seq_field, JsonValue::Null) {
            return Ok(None);
        }
        let seq_u64 = seq_field
            .as_u64()
            .ok_or_else(|| VoiceError::invalid("invalid integer seq field"))?;
        let seq_u16 = u16::try_from(seq_u64)
            .map_err(|_| VoiceError::invalid("seq field exceeds u16::MAX"))?;
        self.seq_ack = Some(seq_u16);
        Ok(Some(seq_u16))
    }

    /// Разбирает входящий бинарный кадр DAVE (`[seq_hi, seq_lo, opcode, payload...]`)
    /// для серверных опкодов `25, 27, 29, 30` и обновляет `seq_ack`.
    pub fn update_from_binary<'a>(
        &mut self,
        frame: &'a [u8],
    ) -> Result<(u16, u8, &'a [u8]), VoiceError> {
        if frame.len() < 3 {
            return Err(VoiceError::invalid(
                "binary DAVE frame shorter than 3-byte header",
            ));
        }
        let seq = u16::from_be_bytes([frame[0], frame[1]]);
        let op = frame[2];
        if !matches!(
            op,
            dave::OP_MLS_EXTERNAL_SENDER
                | dave::OP_MLS_PROPOSALS
                | dave::OP_MLS_PREPARE_COMMIT_TRANSITION
                | dave::OP_MLS_WELCOME
        ) {
            return Err(VoiceError::invalid(
                "unsupported server binary DAVE opcode for seq_ack",
            ));
        }
        self.seq_ack = Some(seq);
        Ok((seq, op, &frame[3..]))
    }

    /// Формирует структуру `VoiceHeartbeat` (`op:3`) с текущим `seq_ack` (либо `-1`, если `None`).
    #[must_use]
    pub fn build_heartbeat(&self, nonce: u64) -> VoiceHeartbeat {
        VoiceHeartbeat {
            t: nonce,
            seq_ack: self.seq_ack.map_or(-1, i64::from),
        }
    }

    /// Формирует JSON-строку `op:3 Heartbeat` (`{"op":3,"d":{"t":nonce,"seq_ack":...}}`).
    #[must_use]
    pub fn build_heartbeat_json(&self, nonce: u64) -> String {
        self.build_heartbeat(nonce).to_json()
    }

    /// Формирует структуру `VoiceResume` (`op:7`) с текущим `seq_ack` (либо `-1`, если `None`).
    #[must_use]
    pub fn build_resume(
        &self,
        server_id: &str,
        channel_id: &str,
        session_id: &str,
        token: &str,
    ) -> VoiceResume {
        VoiceResume {
            server_id: server_id.to_owned(),
            channel_id: channel_id.to_owned(),
            session_id: session_id.to_owned(),
            token: token.to_owned(),
            seq_ack: self.seq_ack.map_or(-1, i64::from),
        }
    }

    /// Формирует JSON-строку `op:7 Resume` (`{"op":7,"d":{"server_id":...,"channel_id":...,"session_id":...,"token":...,"seq_ack":...}}`).
    #[must_use]
    pub fn build_resume_json(
        &self,
        server_id: &str,
        channel_id: &str,
        session_id: &str,
        token: &str,
    ) -> String {
        self.build_resume(server_id, channel_id, session_id, token)
            .to_json()
    }
}

// ============================================================================
// V4: Трекер надёжности UDP (`UdpReliabilityTracker`, keepalive 5s, 10 ошибок -> реконнект, ротация +60.08s)
// ============================================================================

/// Порог последовательных ошибок отправки UDP-пакетов до принудительного реконнекта (`10`).
pub const UDP_MAX_CONSECUTIVE_FAILURES: u8 = 10;

/// Счётчик последовательных сбоев отправки по UDP-сокету (эталон Serein `UdpFailures`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UdpFailures {
    consecutive: u8,
}

impl UdpFailures {
    /// Создаёт новый счётчик с 0 ошибок.
    #[must_use]
    pub const fn new() -> Self {
        Self { consecutive: 0 }
    }

    /// Регистрирует ошибку отправки UDP-пакета. Возвращает `true`, если достигнут порог `10` ошибок подряд.
    pub fn record_failure(&mut self) -> bool {
        self.consecutive = self.consecutive.saturating_add(1);
        self.consecutive >= UDP_MAX_CONSECUTIVE_FAILURES
    }

    /// Сбрасывает счётчик последовательных ошибок после успешной отправки.
    pub fn record_success(&mut self) {
        self.consecutive = 0;
    }

    /// Текущее число последовательных ошибок отправки.
    #[must_use]
    pub const fn consecutive_failures(&self) -> u8 {
        self.consecutive
    }

    /// Возвращает `true`, если накоплено `>= 10` последовательных ошибок и требуется реконнект.
    #[must_use]
    pub const fn should_reconnect(&self) -> bool {
        self.consecutive >= UDP_MAX_CONSECUTIVE_FAILURES
    }
}

/// Статус надёжности UDP-транспорта после попытки отправки пакета.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpReliabilityStatus {
    /// Пакет успешно отправлен, счётчик ошибок равен 0.
    Healthy,
    /// Одиночный/кратковременный сбой (`< 10` ошибок подряд).
    TransientFailure { consecutive_failures: u8 },
    /// Достигнут порог `10` последовательных ошибок — требуется переподключение транспорта.
    ReconnectRequired { consecutive_failures: u8 },
}

/// Комплексный трекер надёжности UDP-транспорта (этап V4):
/// - генерация keepalive каждые `5s` (`0x1337CAFE` + 4B LE sequence);
/// - отслеживание `UdpFailures = 10` ошибок подряд -> реконнект;
/// - расписание ротации локального UDP-порта каждые `+60.08s` (`60_080ms`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpReliabilityTracker {
    last_keepalive_ms: u64,
    keepalive_sequence: u32,
    failures: UdpFailures,
    rotation: udp::RotationSchedule,
    rotations_completed: u32,
}

impl UdpReliabilityTracker {
    /// Создаёт новый трекер надёжности UDP, отсчитывая таймеры keepalive и ротации от `started_at_ms`.
    #[must_use]
    pub const fn new(started_at_ms: u64) -> Self {
        Self {
            last_keepalive_ms: started_at_ms,
            keepalive_sequence: 0,
            failures: UdpFailures::new(),
            rotation: udp::RotationSchedule::new(started_at_ms),
            rotations_completed: 0,
        }
    }

    /// Проверяет, наступил ли 5-секундный интервал keepalive (`5_000ms`), и возвращает 8-байтный пакет.
    pub fn poll_keepalive(&mut self, now_ms: u64) -> Option<[u8; udp::KEEPALIVE_LEN]> {
        if now_ms.saturating_sub(self.last_keepalive_ms) < udp::KEEPALIVE_INTERVAL_MS {
            return None;
        }
        self.last_keepalive_ms = now_ms;
        self.keepalive_sequence = self.keepalive_sequence.wrapping_add(1);
        Some(udp::keepalive_request(self.keepalive_sequence))
    }

    /// Учитывает результат отправки UDP-пакета (`true` — успех, `false` — ошибка сокета).
    pub fn record_send_result(&mut self, succeeded: bool) -> UdpReliabilityStatus {
        if succeeded {
            self.failures.record_success();
            UdpReliabilityStatus::Healthy
        } else if self.failures.record_failure() {
            UdpReliabilityStatus::ReconnectRequired {
                consecutive_failures: self.failures.consecutive_failures(),
            }
        } else {
            UdpReliabilityStatus::TransientFailure {
                consecutive_failures: self.failures.consecutive_failures(),
            }
        }
    }

    /// Проверяет, наступило ли окно ротации UDP-порта (`+60.08s` = `60_080ms`),
    /// и возвращает пару пакетов `Discovery (74B) + Empty RR (8B)` для нового сокета.
    pub fn poll_port_rotation(&mut self, now_ms: u64, ssrc: u32) -> Option<udp::RotationAction> {
        if self.rotation.due(now_ms) {
            self.rotations_completed = self.rotations_completed.saturating_add(1);
            Some(self.rotation.action(ssrc))
        } else {
            None
        }
    }

    /// Текущее число последовательных ошибок отправки UDP.
    #[must_use]
    pub const fn consecutive_failures(&self) -> u8 {
        self.failures.consecutive_failures()
    }

    /// Требуется ли переподключение транспорта из-за `>= 10` ошибок подряд.
    #[must_use]
    pub const fn should_reconnect(&self) -> bool {
        self.failures.should_reconnect()
    }

    /// Количество завершённых ротаций локального UDP-порта.
    #[must_use]
    pub const fn rotations_completed(&self) -> u32 {
        self.rotations_completed
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn voice_constants_match_re() {
        assert_eq!(super::RTP_OPUS_PAYLOAD_TYPE, 120);
        assert_eq!(super::RTP_AV1_PAYLOAD_TYPE, 101);
        assert_eq!(super::RTP_VP8_PAYLOAD_TYPE, 107);
        assert_eq!(super::RTP_VP9_PAYLOAD_TYPE, 109);
        assert_eq!(super::VOICE_HEARTBEAT_INTERVAL_MS, 13_750);
        assert_eq!(super::KEEPALIVE_MAGIC, 0x1337_cafe);
        assert_ne!(super::KEEPALIVE_MAGIC, super::KEEPALIVE_REPLY);
        assert_eq!(super::OBSERVED_RTC_OPS.len(), 9);
    }

    #[test]
    fn session_description_video_codec_detection() {
        let sd_vp8 = super::SessionDescription {
            mode: "aead_aes256_gcm_rtpsize".into(),
            secret_key: vec![0; 32],
            media_session_id: "test".into(),
            dave_protocol_version: 0,
            secure_frames_version: 0,
            audio_codec: Some("opus".into()),
            video_codec: Some("VP8".into()),
        };
        assert!(sd_vp8.is_vp8());
        assert!(!sd_vp8.is_vp9());

        let sd_vp9 = super::SessionDescription {
            mode: "aead_aes256_gcm_rtpsize".into(),
            secret_key: vec![0; 32],
            media_session_id: "test".into(),
            dave_protocol_version: 0,
            secure_frames_version: 0,
            audio_codec: Some("opus".into()),
            video_codec: Some("vp9".into()),
        };
        assert!(!sd_vp9.is_vp8());
        assert!(sd_vp9.is_vp9());

        let sd_h264 = super::SessionDescription {
            mode: "aead_aes256_gcm_rtpsize".into(),
            secret_key: vec![0; 32],
            media_session_id: "test".into(),
            dave_protocol_version: 0,
            secure_frames_version: 0,
            audio_codec: Some("opus".into()),
            video_codec: Some("H264".into()),
        };
        assert!(!sd_h264.is_vp8());
        assert!(!sd_h264.is_vp9());
    }
}
