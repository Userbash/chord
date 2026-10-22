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
pub mod media;
pub mod reception;
pub mod reconnect;
pub mod regions;
pub mod rtcp;
pub mod srtp;
pub mod stats;
pub mod udp;

use discord_json::{self, JsonValue};
use std::collections::BTreeMap;
use std::fmt;

/// Payload type OPUS для медиа-канала (Select Protocol, priority 1000).
pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;

/// Payload type AV1 (video) из списка кодеков Select Protocol.
pub const RTP_AV1_PAYLOAD_TYPE: u8 = 101;

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
    HeartbeatAck,
    Hello,
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
            6 => Self::HeartbeatAck,
            8 => Self::Hello,
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
            Self::HeartbeatAck => 6,
            Self::Hello => 8,
            Self::Unknown(value) => value,
        }
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
        discord_json::to_json_string(&object(vec![("op", json_u64(0)), ("d", payload)]))
    }
}

/// Select Protocol codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Codec {
    pub name: String,
    pub kind: String,
    pub priority: u64,
    pub payload_type: u8,
}

/// Outgoing Select Protocol payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectProtocol {
    pub protocol: String,
    pub address: String,
    pub port: u16,
    pub mode: String,
    pub codecs: Vec<Codec>,
}

impl SelectProtocol {
    /// Serialize the outgoing op1 frame (`{"op":1,"d":...}`) without redaction.
    #[must_use]
    pub fn to_json(&self) -> String {
        let codecs = self
            .codecs
            .iter()
            .map(|codec| {
                object(vec![
                    ("name", JsonValue::String(codec.name.clone())),
                    ("type", JsonValue::String(codec.kind.clone())),
                    ("priority", json_u64(codec.priority)),
                    ("payload_type", json_u64(u64::from(codec.payload_type))),
                ])
            })
            .collect();
        let data = object(vec![
            ("address", JsonValue::String(self.address.clone())),
            ("port", json_u64(u64::from(self.port))),
            ("mode", JsonValue::String(self.mode.clone())),
        ]);
        let payload = object(vec![
            ("protocol", JsonValue::String(self.protocol.clone())),
            ("data", data),
            // RE capture дублирует address/port/mode на верхнем уровне d.
            ("address", JsonValue::String(self.address.clone())),
            ("port", json_u64(u64::from(self.port))),
            ("mode", JsonValue::String(self.mode.clone())),
            ("codecs", JsonValue::Array(codecs)),
        ]);
        discord_json::to_json_string(&object(vec![("op", json_u64(1)), ("d", payload)]))
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
    let root = discord_json::parse(input).map_err(|_| VoiceError::invalid("invalid voice JSON"))?;
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
            Ok(Codec {
                name: nonempty_string(codec, "name")?,
                kind: nonempty_string(codec, "type")?,
                priority: required_u64(codec, "priority")?,
                payload_type: required_u64(codec, "payload_type")?
                    .try_into()
                    .map_err(|_| VoiceError::invalid("invalid codec payload type"))?,
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
    Ok(SelectProtocol {
        protocol,
        address: nonempty_string(nested, "address")?,
        port: required_u64(nested, "port")?
            .try_into()
            .map_err(|_| VoiceError::invalid("invalid protocol port"))?,
        mode: nonempty_string(nested, "mode")?,
        codecs,
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
    let secret_key = decode_base64(&nonempty_string(data, "secret_key")?)?;
    if secret_key.len() != 32 {
        return Err(VoiceError::invalid("secret_key must decode to 32 bytes"));
    }
    Ok(SessionDescription {
        mode: nonempty_string(data, "mode")?,
        secret_key,
        media_session_id: nonempty_string(data, "media_session_id")?,
        dave_protocol_version: required_u64(data, "dave_protocol_version")?
            .try_into()
            .map_err(|_| VoiceError::invalid("invalid DAVE version"))?,
        secure_frames_version: required_u64(data, "secure_frames_version")?
            .try_into()
            .map_err(|_| VoiceError::invalid("invalid secure frames version"))?,
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
        let root = discord_json::parse(input)
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
    JsonValue::Number(discord_json::JsonNumber::from_u64(value))
}

fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn voice_constants_match_re() {
        assert_eq!(super::RTP_OPUS_PAYLOAD_TYPE, 120);
        assert_eq!(super::VOICE_HEARTBEAT_INTERVAL_MS, 13_750);
        assert_eq!(super::KEEPALIVE_MAGIC, 0x1337_cafe);
        assert_ne!(super::KEEPALIVE_MAGIC, super::KEEPALIVE_REPLY);
        assert_eq!(super::OBSERVED_RTC_OPS.len(), 9);
    }
}
