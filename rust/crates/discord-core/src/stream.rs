//! Контракт шлюза и чистого редьюсера Go Live / Stream (STUB-27, RW-40).
//!
//! Замороженный контракт: `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md`,
//! векторные фикстуры: `waves/RW-40/VOICE/stream-opcodes-fixtures.json`.
//!
//! Ключевые решения:
//! - **D-27-1**: `stream_key` в состоянии хранится структурой [`StreamKey`], а на провод уходит каноническая строка;
//! - **D-27-2**: `STREAM_CREATE` для `call` кодирует `guild_id: null` (`EtfValue::Nil`);
//! - **D-27-3**: команды 18…22 отправляются только из `Established` (`SessionNotEstablished`);
//! - **D-27-4**: `token` второго RTC хранится только в [`StreamSecret`] с редактированным `Debug`/`Display`;
//! - **D-27-5**: `viewer_ids` — `Option<Vec<u64>>`: `None` (поля нет) ≠ `Some(vec![])` (0 зрителей);
//! - **D-27-6**: `Debug`/`Display` у [`StreamKey`] не печатают `guild_id` и не печатают собранную строку ключа;
//! - **D-27-7**: неизвестный `STREAM_DELETE.reason` сохраняется как [`StreamDeleteReason::Unknown`], фаза `Ended`.

use std::collections::BTreeMap;
use std::fmt;

use discord_codec::etf::EtfValue;

/// Технический предел длины входной строки `stream_key` (§2.4 контракта).
pub const MAX_KEY_BYTES: usize = 128;

/// Максимальная длина `preferred_region` / `region` в байтах ASCII (§2.2 п. 6).
pub const MAX_REGION_BYTES: usize = 64;

/// Тип стрима: гильдия (`"guild"`) либо звонок/DM (`"call"`).
///
/// Якорь: `A-STREAMTYPE` (`@5084903`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StreamKind {
    /// Стрим в голосовом канале гильдии (`guild:<guild_id>:<channel_id>:<user_id>`).
    Guild,
    /// Стрим в личном/групповом звонке (`call:<channel_id>:<user_id>`).
    Call,
}

/// Алиас типа стрима для совместимости с нотацией `StreamType`.
pub type StreamType = StreamKind;

impl StreamKind {
    /// Литерал провода (`"guild"` | `"call"`).
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Guild => "guild",
            Self::Call => "call",
        }
    }

    /// Точное регистрозависимое сопоставление литерала провода.
    #[must_use]
    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw {
            "guild" => Some(Self::Guild),
            "call" => Some(Self::Call),
            _ => None,
        }
    }
}

impl fmt::Display for StreamKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire())
    }
}

/// Ошибка разбора строки [`StreamKey`] (§3.1 контракта).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamKeyError {
    /// Пустая строка.
    Empty,
    /// Длина строки превышает [`MAX_KEY_BYTES`].
    TooLong {
        /// Фактическая длина в байтах.
        len: usize,
    },
    /// Неизвестный префикс (не `"guild"` и не `"call"`).
    UnknownKind {
        /// Встреченный префикс.
        kind: String,
    },
    /// Неверное число сегментов (`:`-разделителей).
    SegmentCount {
        /// Распознанный вид ключа.
        kind: StreamKind,
        /// Ожидаемое число сегментов (4 для `guild`, 3 для `call`).
        expected: usize,
        /// Фактическое число сегментов.
        found: usize,
    },
    /// Пустой сегмент на указанной позиции.
    EmptySegment {
        /// Индекс сегмента (начиная с 0).
        index: usize,
    },
    /// Сегмент не является канонической снежинкой (не ASCII-цифры, ведущий ноль, `0`, > 20 знаков).
    NotSnowflake {
        /// Индекс сегмента.
        index: usize,
    },
    /// Десятичное число из ≤ 20 цифр не помещается в `u64`.
    Overflow {
        /// Индекс сегмента.
        index: usize,
    },
}

impl fmt::Display for StreamKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("пустой ключ стрима"),
            Self::TooLong { len } => {
                write!(
                    f,
                    "ключ стрима слишком длинный ({len} Б > {MAX_KEY_BYTES} Б)"
                )
            }
            Self::UnknownKind { .. } => f.write_str("неизвестный тип ключа стрима"),
            Self::SegmentCount {
                kind,
                expected,
                found,
            } => write!(
                f,
                "неверное число сегментов для {kind}: ожидалось {expected}, получено {found}"
            ),
            Self::EmptySegment { index } => write!(f, "пустой сегмент ключа на позиции {index}"),
            Self::NotSnowflake { index } => {
                write!(
                    f,
                    "сегмент на позиции {index} не является валидным snowflake"
                )
            }
            Self::Overflow { index } => {
                write!(f, "переполнение u64 в сегменте ключа на позиции {index}")
            }
        }
    }
}

impl std::error::Error for StreamKeyError {}

/// Структурное представление `stream_key` (D-27-1, D-27-6).
///
/// Якоря: `A-KEY-MODULE`, `A-KEY-PARSE`, `A-KEY-BUILD`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamKey {
    /// Тип стрима (`Guild` или `Call`).
    pub kind: StreamKind,
    /// Идентификатор гильдии (`Some` ⇔ `Guild`).
    pub guild_id: Option<u64>,
    /// Идентификатор голосового канала.
    pub channel_id: u64,
    /// Идентификатор владельца (стримера).
    pub owner_id: u64,
}

impl StreamKey {
    /// Разобрать каноническую строку `stream_key` (§2.4).
    pub fn parse(raw: &str) -> Result<Self, StreamKeyError> {
        if raw.is_empty() {
            return Err(StreamKeyError::Empty);
        }
        if raw.len() > MAX_KEY_BYTES {
            return Err(StreamKeyError::TooLong { len: raw.len() });
        }
        let segments: Vec<&str> = raw.split(':').collect();
        let prefix = segments[0];
        let Some(kind) = StreamKind::from_wire(prefix) else {
            return Err(StreamKeyError::UnknownKind {
                kind: prefix.to_string(),
            });
        };
        match kind {
            StreamKind::Guild => {
                if segments.len() != 4 {
                    return Err(StreamKeyError::SegmentCount {
                        kind,
                        expected: 4,
                        found: segments.len(),
                    });
                }
                let guild_id = parse_snowflake_segment(segments[1], 1)?;
                let channel_id = parse_snowflake_segment(segments[2], 2)?;
                let owner_id = parse_snowflake_segment(segments[3], 3)?;
                Ok(Self {
                    kind,
                    guild_id: Some(guild_id),
                    channel_id,
                    owner_id,
                })
            }
            StreamKind::Call => {
                if segments.len() != 3 {
                    return Err(StreamKeyError::SegmentCount {
                        kind,
                        expected: 3,
                        found: segments.len(),
                    });
                }
                let channel_id = parse_snowflake_segment(segments[1], 1)?;
                let owner_id = parse_snowflake_segment(segments[2], 2)?;
                Ok(Self {
                    kind,
                    guild_id: None,
                    channel_id,
                    owner_id,
                })
            }
        }
    }

    /// Проверить и собрать ключ из компонентов (§1.3, T-STREAM-22).
    pub fn build(
        kind: StreamKind,
        guild_id: Option<u64>,
        channel_id: u64,
        owner_id: u64,
    ) -> Result<Self, StreamError> {
        match (kind, guild_id) {
            (StreamKind::Guild, None) => return Err(StreamError::MissingGuildId),
            (StreamKind::Call, Some(_)) => return Err(StreamError::UnexpectedGuildId),
            (StreamKind::Guild, Some(0)) => {
                return Err(StreamError::InvalidId { field: "guild_id" });
            }
            _ => {}
        }
        if channel_id == 0 {
            return Err(StreamError::InvalidId {
                field: "channel_id",
            });
        }
        if owner_id == 0 {
            return Err(StreamError::InvalidId { field: "owner_id" });
        }
        Ok(Self {
            kind,
            guild_id,
            channel_id,
            owner_id,
        })
    }

    /// Собрать каноническую проводную строку (`guild:g:c:u` или `call:c:u`).
    #[must_use]
    pub fn to_wire(&self) -> String {
        match self.kind {
            StreamKind::Guild => format!(
                "guild:{}:{}:{}",
                self.guild_id.unwrap_or(0),
                self.channel_id,
                self.owner_id
            ),
            StreamKind::Call => format!("call:{}:{}", self.channel_id, self.owner_id),
        }
    }

    /// Синоним [`Self::to_wire`] для получения канонической строки ключа.
    #[must_use]
    pub fn canonical(&self) -> String {
        self.to_wire()
    }

    /// Конструктор ключа гильдии.
    #[must_use]
    pub fn guild(guild_id: u64, channel_id: u64, owner_id: u64) -> Self {
        Self {
            kind: StreamKind::Guild,
            guild_id: Some(guild_id),
            channel_id,
            owner_id,
        }
    }

    /// Конструктор ключа звонка.
    #[must_use]
    pub fn call(channel_id: u64, owner_id: u64) -> Self {
        Self {
            kind: StreamKind::Call,
            guild_id: None,
            channel_id,
            owner_id,
        }
    }
}

/// D-27-6: `Debug` не печатает `guild_id` и не печатает собранную строку ключа.
impl fmt::Debug for StreamKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamKey")
            .field("kind", &self.kind)
            .field("guild_id", &"<redacted>")
            .field("channel_id", &self.channel_id)
            .field("owner_id", &self.owner_id)
            .finish()
    }
}

/// D-27-6: `Display` также не печатает `guild_id` и собранную строку ключа.
impl fmt::Display for StreamKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamKey(kind={}, guild_id=<redacted>, channel_id={}, owner_id={})",
            self.kind.as_wire(),
            self.channel_id,
            self.owner_id
        )
    }
}

fn parse_snowflake_segment(seg: &str, index: usize) -> Result<u64, StreamKeyError> {
    if seg.is_empty() {
        return Err(StreamKeyError::EmptySegment { index });
    }
    if !seg.bytes().all(|b| b.is_ascii_digit()) || seg.starts_with('0') || seg.len() > 20 {
        return Err(StreamKeyError::NotSnowflake { index });
    }
    seg.parse::<u64>()
        .map_err(|_| StreamKeyError::Overflow { index })
}

/// Состояние сессии шлюза для проверки D-27-3 (`isSessionEstablished`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewaySessionState {
    /// Сессия устанавливается (до `READY`/`RESUMED`).
    Connecting,
    /// Сессия установлена (`isSessionEstablished() == true`).
    Established,
    /// Переподключение.
    Reconnecting,
    /// Отключено.
    Disconnected,
}

impl GatewaySessionState {
    /// Разрешена ли отправка команд стрима 18…22 (D-27-3).
    #[must_use]
    pub fn is_established(self) -> bool {
        matches!(self, Self::Established)
    }

    /// Разбор строкового обозначения состояния сессии из фикстуры.
    #[must_use]
    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw {
            "established" => Some(Self::Established),
            "connecting" => Some(Self::Connecting),
            "reconnecting" => Some(Self::Reconnecting),
            "disconnected" => Some(Self::Disconnected),
            _ => None,
        }
    }
}

/// Исходящие опкоды Go Live / Stream (`18..=22`, клиент → сервер).
///
/// Якорь: `A-OPCODES` (`@4996751`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StreamOp {
    /// `18` — `STREAM_CREATE`.
    Create = 18,
    /// `19` — `STREAM_DELETE`.
    Delete = 19,
    /// `20` — `STREAM_WATCH`.
    Watch = 20,
    /// `21` — `STREAM_PING`.
    Ping = 21,
    /// `22` — `STREAM_SET_PAUSED`.
    SetPaused = 22,
}

#[allow(non_upper_case_globals)]
impl StreamOp {
    /// Алиас `StreamCreate = 18`.
    pub const StreamCreate: Self = Self::Create;
    /// Алиас `StreamDelete = 19`.
    pub const StreamDelete: Self = Self::Delete;
    /// Алиас `StreamWatch = 20`.
    pub const StreamWatch: Self = Self::Watch;
    /// Алиас `StreamPing = 21`.
    pub const StreamPing: Self = Self::Ping;
    /// Алиас `StreamSetPaused = 22`.
    pub const StreamSetPaused: Self = Self::SetPaused;

    /// Числовой код опкода (`18..=22`).
    #[must_use]
    pub fn code(self) -> i64 {
        self as i64
    }

    /// Преобразование числового кода в [`StreamOp`]; вне `18..=22` возвращает `None`.
    #[must_use]
    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            18 => Some(Self::Create),
            19 => Some(Self::Delete),
            20 => Some(Self::Watch),
            21 => Some(Self::Ping),
            22 => Some(Self::SetPaused),
            _ => None,
        }
    }

    /// Имя опкода в таблице эталонного клиента (`A-OPCODES`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Create => "STREAM_CREATE",
            Self::Delete => "STREAM_DELETE",
            Self::Watch => "STREAM_WATCH",
            Self::Ping => "STREAM_PING",
            Self::SetPaused => "STREAM_SET_PAUSED",
        }
    }
}

/// Исходящая команда Go Live / Stream (`op 18..=22`).
///
/// Якорь: `A-SENDERS` (`@4998992`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamCommand {
    /// `op 18 STREAM_CREATE`: `{type, guild_id, channel_id, preferred_region}`.
    Create {
        /// Тип стрима (`Guild` или `Call`).
        kind: StreamKind,
        /// Идентификатор гильдии (`Some` ⇔ `Guild`, для `Call` на провод кодируется `null` по D-27-2).
        guild_id: Option<u64>,
        /// Идентификатор канала.
        channel_id: u64,
        /// Предпочитаемый голосовой регион либо `None` (`null` на проводе).
        preferred_region: Option<String>,
    },
    /// `op 19 STREAM_DELETE`: `{stream_key}`.
    Delete {
        /// Ключ стрима.
        stream_key: StreamKey,
    },
    /// `op 20 STREAM_WATCH`: `{stream_key}`.
    Watch {
        /// Ключ стрима.
        stream_key: StreamKey,
    },
    /// `op 21 STREAM_PING`: `{stream_key}`.
    Ping {
        /// Ключ стрима.
        stream_key: StreamKey,
    },
    /// `op 22 STREAM_SET_PAUSED`: `{stream_key, paused}`.
    SetPaused {
        /// Ключ стрима.
        stream_key: StreamKey,
        /// Флаг паузы.
        paused: bool,
    },
}

impl StreamCommand {
    /// Проверить инварианты команды перед сериализацией (§2.2, §3.2).
    pub fn validate(&self) -> Result<(), StreamError> {
        match self {
            Self::Create {
                kind,
                guild_id,
                channel_id,
                preferred_region,
            } => {
                match (kind, guild_id) {
                    (StreamKind::Guild, None) => return Err(StreamError::MissingGuildId),
                    (StreamKind::Call, Some(_)) => return Err(StreamError::UnexpectedGuildId),
                    (StreamKind::Guild, Some(0)) => {
                        return Err(StreamError::InvalidId { field: "guild_id" });
                    }
                    _ => {}
                }
                if *channel_id == 0 {
                    return Err(StreamError::InvalidId {
                        field: "channel_id",
                    });
                }
                if let Some(region) = preferred_region
                    && !is_valid_region(region)
                {
                    return Err(StreamError::InvalidRegion);
                }
                Ok(())
            }
            Self::Delete { stream_key }
            | Self::Watch { stream_key }
            | Self::Ping { stream_key }
            | Self::SetPaused { stream_key, .. } => {
                let _ = StreamKey::build(
                    stream_key.kind,
                    stream_key.guild_id,
                    stream_key.channel_id,
                    stream_key.owner_id,
                )?;
                Ok(())
            }
        }
    }

    /// Опкод команды (`18..=22`).
    #[must_use]
    pub fn op(&self) -> StreamOp {
        match self {
            Self::Create { .. } => StreamOp::Create,
            Self::Delete { .. } => StreamOp::Delete,
            Self::Watch { .. } => StreamOp::Watch,
            Self::Ping { .. } => StreamOp::Ping,
            Self::SetPaused { .. } => StreamOp::SetPaused,
        }
    }

    /// Полезная нагрузка `d` в виде [`EtfValue::Map`] (§2.2).
    #[must_use]
    pub fn to_etf(&self) -> EtfValue {
        match self {
            Self::Create {
                kind,
                guild_id,
                channel_id,
                preferred_region,
            } => {
                // D-27-2: для Call кодируется явный Nil (guild_id: null).
                let guild_val = match (kind, guild_id) {
                    (StreamKind::Guild, Some(id)) => EtfValue::Str(id.to_string()),
                    _ => EtfValue::Nil,
                };
                let region_val = match preferred_region {
                    Some(region) => EtfValue::Str(region.clone()),
                    None => EtfValue::Nil,
                };
                EtfValue::Map(vec![
                    (
                        EtfValue::Str("type".to_string()),
                        EtfValue::Str(kind.as_wire().to_string()),
                    ),
                    (EtfValue::Str("guild_id".to_string()), guild_val),
                    (
                        EtfValue::Str("channel_id".to_string()),
                        EtfValue::Str(channel_id.to_string()),
                    ),
                    (EtfValue::Str("preferred_region".to_string()), region_val),
                ])
            }
            Self::Delete { stream_key }
            | Self::Watch { stream_key }
            | Self::Ping { stream_key } => EtfValue::Map(vec![(
                EtfValue::Str("stream_key".to_string()),
                EtfValue::Str(stream_key.to_wire()),
            )]),
            Self::SetPaused { stream_key, paused } => EtfValue::Map(vec![
                (
                    EtfValue::Str("stream_key".to_string()),
                    EtfValue::Str(stream_key.to_wire()),
                ),
                (
                    EtfValue::Str("paused".to_string()),
                    EtfValue::Atom(if *paused { "true" } else { "false" }.to_string()),
                ),
            ]),
        }
    }

    /// Валидировать и закодировать полный кадр шлюза `{"op": N, "d": {...}}` (§3.2).
    pub fn encode(&self) -> Result<EtfValue, StreamError> {
        self.validate()?;
        Ok(EtfValue::Map(vec![
            (
                EtfValue::Str("op".to_string()),
                EtfValue::Int(self.op().code()),
            ),
            (EtfValue::Str("d".to_string()), self.to_etf()),
        ]))
    }

    /// Кодировать команду только при установленной сессии шлюза (D-27-3).
    pub fn encode_if_established(&self, established: bool) -> Result<EtfValue, StreamError> {
        if !established {
            return Err(StreamError::SessionNotEstablished);
        }
        self.encode()
    }

    /// Кодировать команду с проверкой [`GatewaySessionState`] (D-27-3).
    pub fn encode_in_session(&self, session: GatewaySessionState) -> Result<EtfValue, StreamError> {
        self.encode_if_established(session.is_established())
    }

    /// Собрать и провалидировать команду из нетипизированной спецификации (с проверкой D-27-3).
    pub fn from_raw_spec(
        spec: &EtfValue,
        session: GatewaySessionState,
    ) -> Result<Self, StreamError> {
        if !session.is_established() {
            return Err(StreamError::SessionNotEstablished);
        }
        if spec.as_map().is_none() {
            return Err(StreamError::WrongFieldType {
                field: "command",
                expected: "map",
            });
        }
        let Some(variant) = spec.get("variant").and_then(strict_str) else {
            return Err(StreamError::MissingField { field: "variant" });
        };
        match variant {
            "create" => {
                let raw_type = spec
                    .get("type")
                    .or_else(|| spec.get("kind"))
                    .and_then(strict_str)
                    .ok_or(StreamError::WrongKind {
                        expected: StreamKind::Guild,
                        found: StreamKind::Call,
                    })?;
                let Some(kind) = StreamKind::from_wire(raw_type) else {
                    return Err(StreamError::WrongKind {
                        expected: StreamKind::Guild,
                        found: StreamKind::Call,
                    });
                };
                let guild_id = match spec.get("guildId").or_else(|| spec.get("guild_id")) {
                    None => None,
                    Some(v) if is_null(v) => None,
                    Some(v) => Some(parse_u64_id_Allow_zero(v, "guild_id")?),
                };
                let channel_raw = spec
                    .get("channelId")
                    .or_else(|| spec.get("channel_id"))
                    .ok_or(StreamError::MissingField {
                        field: "channel_id",
                    })?;
                if is_null(channel_raw) {
                    return Err(StreamError::InvalidId {
                        field: "channel_id",
                    });
                }
                let channel_id = parse_u64_id_Allow_zero(channel_raw, "channel_id")?;
                let preferred_region = match spec
                    .get("preferredRegion")
                    .or_else(|| spec.get("preferred_region"))
                {
                    None => None,
                    Some(v) if is_null(v) => None,
                    Some(v) => {
                        let Some(s) = strict_str(v) else {
                            return Err(StreamError::InvalidRegion);
                        };
                        Some(s.to_string())
                    }
                };
                let cmd = Self::Create {
                    kind,
                    guild_id,
                    channel_id,
                    preferred_region,
                };
                cmd.validate()?;
                Ok(cmd)
            }
            "delete" | "watch" | "ping" => {
                let key_raw = spec
                    .get("streamKey")
                    .or_else(|| spec.get("stream_key"))
                    .ok_or(StreamError::MissingField {
                        field: "stream_key",
                    })?;
                let key_str = strict_str(key_raw).ok_or(StreamError::InvalidKey {
                    reason: StreamKeyError::Empty,
                })?;
                let stream_key = StreamKey::parse(key_str)
                    .map_err(|reason| StreamError::InvalidKey { reason })?;
                let cmd = match variant {
                    "delete" => Self::Delete { stream_key },
                    "watch" => Self::Watch { stream_key },
                    _ => Self::Ping { stream_key },
                };
                cmd.validate()?;
                Ok(cmd)
            }
            "set_paused" => {
                let key_raw = spec
                    .get("streamKey")
                    .or_else(|| spec.get("stream_key"))
                    .ok_or(StreamError::MissingField {
                        field: "stream_key",
                    })?;
                let key_str = strict_str(key_raw).ok_or(StreamError::InvalidKey {
                    reason: StreamKeyError::Empty,
                })?;
                let stream_key = StreamKey::parse(key_str)
                    .map_err(|reason| StreamError::InvalidKey { reason })?;
                let paused_raw = spec.get("paused").ok_or(StreamError::InvalidPaused)?;
                let Some(paused) = strict_bool(paused_raw) else {
                    return Err(StreamError::InvalidPaused);
                };
                let cmd = Self::SetPaused { stream_key, paused };
                cmd.validate()?;
                Ok(cmd)
            }
            _ => Err(StreamError::WrongFieldType {
                field: "variant",
                expected: "create|delete|watch|ping|set_paused",
            }),
        }
    }

    /// Разобрать закодированный ETF-кадр `{"op": 18..22, "d": {...}}` обратно в [`StreamCommand`].
    pub fn from_etf_frame(frame: &EtfValue) -> Result<Self, StreamError> {
        if frame.as_map().is_none() {
            return Err(StreamError::WrongFieldType {
                field: "frame",
                expected: "map",
            });
        }
        let op_code = frame
            .get("op")
            .and_then(EtfValue::as_int)
            .ok_or(StreamError::MissingField { field: "op" })?;
        let Some(op) = StreamOp::from_code(op_code) else {
            return Err(StreamError::WrongFieldType {
                field: "op",
                expected: "18..=22",
            });
        };
        let d = frame
            .get("d")
            .ok_or(StreamError::MissingField { field: "d" })?;
        if d.as_map().is_none() {
            return Err(StreamError::WrongFieldType {
                field: "d",
                expected: "map",
            });
        }
        let variant = match op {
            StreamOp::Create => "create",
            StreamOp::Delete => "delete",
            StreamOp::Watch => "watch",
            StreamOp::Ping => "ping",
            StreamOp::SetPaused => "set_paused",
        };
        let mut pairs = d.as_map().unwrap().to_vec();
        pairs.push((
            EtfValue::Str("variant".to_string()),
            EtfValue::Str(variant.to_string()),
        ));
        Self::from_raw_spec(&EtfValue::Map(pairs), GatewaySessionState::Established)
    }
}

/// Секретный токен второго RTC-соединения (D-27-4).
///
/// Не печатается в `Debug` и `Display`; доступ к строке — только через [`Self::expose`].
#[derive(Clone, PartialEq, Eq)]
pub struct StreamSecret(String);

impl StreamSecret {
    /// Обернуть строку токена в [`StreamSecret`].
    #[must_use]
    pub fn new(token: String) -> Self {
        Self(token)
    }

    /// Единственный явный способ получить сырое значение токена.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for StreamSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"<redacted>\"")
    }
}

impl fmt::Display for StreamSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Событие `STREAM_CREATE` (сервер → клиент, `op 0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCreated {
    /// Ключ стрима.
    pub stream_key: StreamKey,
    /// Голосовой регион стрима.
    pub region: Option<String>,
    /// Список ID зрителей (`None` ≠ `Some(vec![])`, D-27-5).
    pub viewer_ids: Option<Vec<u64>>,
    /// Идентификатор RTC-сервера стрима.
    pub rtc_server_id: Option<String>,
    /// Идентификатор RTC-канала стрима.
    pub rtc_channel_id: Option<u64>,
    /// Состояние паузы стрима.
    pub paused: Option<bool>,
}

/// Событие `STREAM_SERVER_UPDATE` (сервер → клиент, `op 0`).
#[derive(Clone, PartialEq, Eq)]
pub struct StreamServerUpdate {
    /// Ключ стрима.
    pub stream_key: StreamKey,
    /// Endpoint медиа-сервера второго RTC.
    pub endpoint: String,
    /// Секретный токен второго RTC (D-27-4).
    pub token: StreamSecret,
}

impl fmt::Debug for StreamServerUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamServerUpdate")
            .field("stream_key", &self.stream_key)
            .field("endpoint", &"<redacted>")
            .field("token", &self.token)
            .finish()
    }
}

/// Событие `STREAM_UPDATE` (сервер → клиент, `op 0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamUpdate {
    /// Ключ стрима.
    pub stream_key: StreamKey,
    /// Голосовой регион стрима.
    pub region: Option<String>,
    /// Список ID зрителей (`None` ≠ `Some(vec![])`, D-27-5).
    pub viewer_ids: Option<Vec<u64>>,
    /// Состояние паузы стрима.
    pub paused: Option<bool>,
}

/// Протокольная причина завершения стрима в `STREAM_DELETE.reason` (§5.1).
///
/// Якорь: `A-REASONS` (`@528586`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamDeleteReason {
    /// `"user_requested"` — остановлен пользователем.
    UserRequested,
    /// `"stream_full"` — достигнут лимит зрителей.
    StreamFull,
    /// `"unauthorized"` — нет прав / токен отвергнут.
    Unauthorized,
    /// `"safety_guild_rate_limited"` — сработал предохранитель гильдии.
    SafetyGuildRateLimited,
    /// D-27-7: неизвестная серверная причина (forward-compat).
    Unknown(String),
}

impl StreamDeleteReason {
    /// Разобрать проводное значение `reason` (D-27-7).
    #[must_use]
    pub fn from_wire(raw: &str) -> Self {
        match raw {
            "user_requested" => Self::UserRequested,
            "stream_full" => Self::StreamFull,
            "unauthorized" => Self::Unauthorized,
            "safety_guild_rate_limited" => Self::SafetyGuildRateLimited,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Проводное строковое значение `reason`.
    #[must_use]
    pub fn as_wire(&self) -> &str {
        match self {
            Self::UserRequested => "user_requested",
            Self::StreamFull => "stream_full",
            Self::Unauthorized => "unauthorized",
            Self::SafetyGuildRateLimited => "safety_guild_rate_limited",
            Self::Unknown(raw) => raw.as_str(),
        }
    }

    /// Является ли причина одной из четырёх известных протокольных констант `A-REASONS`.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    /// Проекция в атрибут `data-reason` для UI (§5.3).
    #[must_use]
    pub fn ui_data_reason(&self) -> Option<&'static str> {
        match self {
            Self::StreamFull => Some("viewer-full"),
            Self::Unauthorized => Some("permission-channel"),
            Self::SafetyGuildRateLimited => Some("safety-rate-limited"),
            Self::UserRequested | Self::Unknown(_) => None,
        }
    }
}

/// Событие `STREAM_DELETE` (сервер → клиент, `op 0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamDeleted {
    /// Ключ стрима.
    pub stream_key: StreamKey,
    /// Флаг временной недоступности (`true` → `Reconnecting`).
    pub unavailable: Option<bool>,
    /// Причина удаления/завершения стрима.
    pub reason: Option<StreamDeleteReason>,
}

/// Входящий `op 0`-диспатч семейства `STREAM_*` (§3.3).
///
/// Якорь: `A-ROUTER` (`@4937840`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamDispatch {
    /// `STREAM_CREATE`.
    Created(StreamCreated),
    /// `STREAM_SERVER_UPDATE`.
    ServerUpdate(StreamServerUpdate),
    /// `STREAM_UPDATE`.
    Update(StreamUpdate),
    /// `STREAM_DELETE`.
    Deleted(StreamDeleted),
}

/// Разобрать входящий `op 0`-диспатч по имени `name` (`t`) и полезной нагрузке `data` (`d`).
///
/// Возвращает `None`, если `name` не относится к `STREAM_*`;
/// `Some(Err)` — если событие наше, но нарушает обязательные поля.
#[must_use]
pub fn parse_dispatch(name: &str, data: &EtfValue) -> Option<Result<StreamDispatch, StreamError>> {
    let mut diagnostics = StreamDiagnostics::default();
    parse_dispatch_with_diagnostics(name, data, &mut diagnostics)
}

/// Разобрать входящий `op 0`-диспатч с записью мягких ошибок и неизвестных полей в [`StreamDiagnostics`].
pub fn parse_dispatch_with_diagnostics(
    name: &str,
    data: &EtfValue,
    diagnostics: &mut StreamDiagnostics,
) -> Option<Result<StreamDispatch, StreamError>> {
    if !matches!(
        name,
        "STREAM_CREATE" | "STREAM_SERVER_UPDATE" | "STREAM_UPDATE" | "STREAM_DELETE"
    ) {
        return None;
    }

    let Some(pairs) = data.as_map() else {
        diagnostics.invalid_dispatches += 1;
        return Some(Err(StreamError::WrongFieldType {
            field: "d",
            expected: "map",
        }));
    };

    let stream_key = match parse_required_stream_key(data) {
        Ok(key) => key,
        Err(err) => {
            diagnostics.invalid_dispatches += 1;
            return Some(Err(err));
        }
    };

    match name {
        "STREAM_CREATE" => {
            record_ignored_fields(
                pairs,
                &[
                    "stream_key",
                    "region",
                    "viewer_ids",
                    "rtc_server_id",
                    "rtc_channel_id",
                    "paused",
                ],
                diagnostics,
            );
            let region = parse_soft_region(data, "region", diagnostics);
            let viewer_ids = parse_soft_viewer_ids(data, diagnostics);
            let rtc_server_id = parse_soft_snowflake_string(data, "rtc_server_id", diagnostics);
            let rtc_channel_id = parse_soft_snowflake_u64(data, "rtc_channel_id", diagnostics);
            let paused = parse_soft_bool(data, "paused", diagnostics);
            Some(Ok(StreamDispatch::Created(StreamCreated {
                stream_key,
                region,
                viewer_ids,
                rtc_server_id,
                rtc_channel_id,
                paused,
            })))
        }
        "STREAM_SERVER_UPDATE" => {
            record_ignored_fields(pairs, &["stream_key", "endpoint", "token"], diagnostics);
            let endpoint = match parse_required_non_empty_str(data, "endpoint") {
                Ok(s) => s,
                Err(err) => {
                    diagnostics.invalid_dispatches += 1;
                    return Some(Err(err));
                }
            };
            let token = match parse_required_non_empty_str(data, "token") {
                Ok(s) => s,
                Err(err) => {
                    diagnostics.invalid_dispatches += 1;
                    return Some(Err(err));
                }
            };
            Some(Ok(StreamDispatch::ServerUpdate(StreamServerUpdate {
                stream_key,
                endpoint,
                token: StreamSecret::new(token),
            })))
        }
        "STREAM_UPDATE" => {
            record_ignored_fields(
                pairs,
                &["stream_key", "region", "viewer_ids", "paused"],
                diagnostics,
            );
            let region = parse_soft_region(data, "region", diagnostics);
            let viewer_ids = parse_soft_viewer_ids(data, diagnostics);
            let paused = parse_soft_bool(data, "paused", diagnostics);
            Some(Ok(StreamDispatch::Update(StreamUpdate {
                stream_key,
                region,
                viewer_ids,
                paused,
            })))
        }
        "STREAM_DELETE" => {
            record_ignored_fields(pairs, &["stream_key", "unavailable", "reason"], diagnostics);
            let unavailable = parse_soft_bool(data, "unavailable", diagnostics);
            let reason = parse_soft_delete_reason(data, diagnostics);
            Some(Ok(StreamDispatch::Deleted(StreamDeleted {
                stream_key,
                unavailable,
                reason,
            })))
        }
        _ => None,
    }
}

/// Шесть фаз жизненного цикла стрима (§3.4).
///
/// Якорь: `A-PHASES` (`@528441`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StreamPhase {
    /// `"CONNECTING"` — локальный `WatchRequested`, ждём `STREAM_CREATE`.
    Connecting,
    /// `"ACTIVE"` — стрим активен (`paused == false`).
    Active,
    /// `"PAUSED"` — стрим на паузе (`paused == true`).
    Paused,
    /// `"RECONNECTING"` — получено `STREAM_DELETE{unavailable: true}`.
    Reconnecting,
    /// `"ENDED"` — стрим штатно завершён.
    Ended,
    /// `"FAILED"` — отказ авторизации/прав (`reason == "unauthorized"`).
    Failed,
}

impl StreamPhase {
    /// Константа фазы в формате эталонного клиента (`"CONNECTING"`…`"FAILED"`).
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Connecting => "CONNECTING",
            Self::Active => "ACTIVE",
            Self::Paused => "PAUSED",
            Self::Reconnecting => "RECONNECTING",
            Self::Ended => "ENDED",
            Self::Failed => "FAILED",
        }
    }

    /// Нижний регистр фазы для сценариев редьюсера и IPC (`"connecting"`…`"failed"`).
    #[must_use]
    pub fn as_lowercase(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Reconnecting => "reconnecting",
            Self::Ended => "ended",
            Self::Failed => "failed",
        }
    }

    /// Разбор фазы по строке `"CONNECTING"`…`"FAILED"`.
    #[must_use]
    pub fn from_wire(raw: &str) -> Option<Self> {
        match raw {
            "CONNECTING" => Some(Self::Connecting),
            "ACTIVE" => Some(Self::Active),
            "PAUSED" => Some(Self::Paused),
            "RECONNECTING" => Some(Self::Reconnecting),
            "ENDED" => Some(Self::Ended),
            "FAILED" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Запись одного стрима в [`StreamState`] (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stream {
    /// Структурный ключ стрима (D-27-1).
    pub key: StreamKey,
    /// Текущая фаза жизненного цикла.
    pub phase: StreamPhase,
    /// Голосовой регион.
    pub region: Option<String>,
    /// Список зрителей (`None` — не присылался, `Some([])` — 0 зрителей, D-27-5).
    pub viewer_ids: Option<Vec<u64>>,
    /// Идентификатор RTC-сервера.
    pub rtc_server_id: Option<String>,
    /// Идентификатор RTC-канала.
    pub rtc_channel_id: Option<u64>,
    /// Текущий флаг паузы.
    pub paused: bool,
    /// Причина последнего `STREAM_DELETE`.
    pub delete_reason: Option<StreamDeleteReason>,
    /// Готовность транспорта второго RTC (`true` после `STREAM_SERVER_UPDATE`).
    pub transport_ready: bool,
}

/// Счётчики и списки диагностики [`StreamState`] (§3.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamDiagnostics {
    /// Отброшенные немонотонные `s` (`seq <= last_seq`).
    pub stale_sequences: Vec<u64>,
    /// Число отброшенных битых `STREAM_*` диспатчей.
    pub invalid_dispatches: u64,
    /// Имена мягких полей с неверным типом/значением.
    pub malformed_fields: Vec<String>,
    /// Число `STREAM_DELETE` по неизвестному ключу.
    pub unknown_streams: u64,
    /// Число `STREAM_SERVER_UPDATE` без существующей записи стрима.
    pub orphan_server_updates: u64,
    /// Встреченные неизвестные `STREAM_DELETE.reason` (D-27-7).
    pub unknown_reasons: Vec<String>,
    /// Игнорированные неизвестные поля в `d`.
    pub ignored_fields: Vec<String>,
}

/// Полное состояние подсистемы Go Live / Stream (§3.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamState {
    streams: BTreeMap<StreamKey, Stream>,
    last_seq: Option<u64>,
    /// Диагностика разбора и редьюсера.
    pub diagnostics: StreamDiagnostics,
}

impl StreamState {
    /// Создать пустое состояние стримов.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Получить запись стрима по ключу.
    #[must_use]
    pub fn get(&self, key: &StreamKey) -> Option<&Stream> {
        self.streams.get(key)
    }

    /// Все текущие записи стримов.
    #[must_use]
    pub fn streams(&self) -> &BTreeMap<StreamKey, Stream> {
        &self.streams
    }

    /// Последний принятый `seq` (`s`).
    #[must_use]
    pub fn last_seq(&self) -> Option<u64> {
        self.last_seq
    }

    /// Безопасная проекция наружу (IPC/UI): без `token`, `endpoint` и строки ключа (§3.4).
    #[must_use]
    pub fn snapshot(&self) -> StreamSnapshot {
        let streams = self
            .streams
            .values()
            .map(|stream| StreamSummary {
                key: stream.key.clone(),
                phase: stream.phase,
                phase_wire: stream.phase.as_wire(),
                region: stream.region.clone(),
                viewer_count: stream.viewer_ids.as_ref().map(Vec::len),
                transport_ready: stream.transport_ready,
            })
            .collect();
        StreamSnapshot { streams }
    }
}

/// Проекция состояния стримов для IPC/UI без секретов и проводных ключей (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSnapshot {
    /// Сводки по всем известным стримам.
    pub streams: Vec<StreamSummary>,
}

/// Сводка одного стрима для IPC/UI (§3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSummary {
    /// Структурный ключ с редактированным `Debug` (D-27-6).
    pub key: StreamKey,
    /// Фаза стрима.
    pub phase: StreamPhase,
    /// Проводная строка фазы (`"CONNECTING"`…`"FAILED"`).
    pub phase_wire: &'static str,
    /// Голосовой регион.
    pub region: Option<String>,
    /// Число зрителей (`None` ⇔ `viewer_ids` отсутствовал, D-27-5).
    pub viewer_count: Option<usize>,
    /// Готовность второго RTC-транспорта.
    pub transport_ready: bool,
}

/// Побочный эффект редьюсера [`apply`] (§3.4).
#[derive(Clone, PartialEq, Eq)]
pub enum StreamEffect {
    /// Открыть/переподключить второй RTC-транспорт стрима (D-27-4).
    OpenStreamRtc {
        /// Ключ стрима.
        key: StreamKey,
        /// Endpoint медиа-сервера.
        endpoint: String,
        /// Секретный токен второго RTC.
        token: StreamSecret,
    },
    /// Закрыть второй RTC-транспорт стрима.
    CloseStreamRtc {
        /// Ключ стрима.
        key: StreamKey,
    },
    /// Показать уведомление о срабатывании safety rate limit гильдии.
    ShowSafetyRateLimit {
        /// Идентификатор гильдии.
        guild_id: u64,
    },
    /// Уведомить пользователя о переполнении зрителей (`stream_full`).
    NotifyViewerFull {
        /// Ключ стрима.
        key: StreamKey,
    },
}

/// `Debug` для [`StreamEffect`] скрывает `endpoint` и `token` (§2.6, T-STREAM-07).
impl fmt::Debug for StreamEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenStreamRtc { key, token, .. } => f
                .debug_struct("OpenStreamRtc")
                .field("key", key)
                .field("endpoint", &"<redacted>")
                .field("token", token)
                .finish(),
            Self::CloseStreamRtc { key } => {
                f.debug_struct("CloseStreamRtc").field("key", key).finish()
            }
            Self::ShowSafetyRateLimit { guild_id } => f
                .debug_struct("ShowSafetyRateLimit")
                .field("guild_id", guild_id)
                .finish(),
            Self::NotifyViewerFull { key } => f
                .debug_struct("NotifyViewerFull")
                .field("key", key)
                .finish(),
        }
    }
}

/// Типизированная ошибка шлюза и команд Go Live / Stream (§3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// Ключ стрима не соответствует грамматике §2.4.
    InvalidKey {
        /// Детальная причина ошибки ключа.
        reason: StreamKeyError,
    },
    /// Отсутствует обязательное поле.
    MissingField {
        /// Имя поля.
        field: &'static str,
    },
    /// Поле имеет неверный тип.
    WrongFieldType {
        /// Имя поля.
        field: &'static str,
        /// Ожидаемый тип.
        expected: &'static str,
    },
    /// Идентификатор равен `0` или не является валидной снежинкой.
    InvalidId {
        /// Имя поля идентификатора.
        field: &'static str,
    },
    /// Невалидный `preferred_region` (пустой, не ASCII, пробелы/control или > 64 Б).
    InvalidRegion,
    /// Поле `paused` не является строгим boolean.
    InvalidPaused,
    /// Неверный тип стрима (`type` не `"guild"`/`"call"`).
    WrongKind {
        /// Ожидаемый тип.
        expected: StreamKind,
        /// Обнаруженный тип.
        found: StreamKind,
    },
    /// Для `type: "guild"` отсутствует `guild_id`.
    MissingGuildId,
    /// Для `type: "call"` передан ненулевой `guild_id`.
    UnexpectedGuildId,
    /// Попытка отправить команду 18…22 вне `Established` (D-27-3).
    SessionNotEstablished,
}

impl StreamError {
    /// Стабильный машинный код ошибки для IPC/UI (§3.5).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidKey { .. } => "stream.key.invalid",
            Self::MissingField { .. } => "stream.field.missing",
            Self::WrongFieldType { .. } => "stream.field.type",
            Self::InvalidId { .. } => "stream.field.id",
            Self::InvalidRegion => "stream.field.region",
            Self::InvalidPaused => "stream.field.paused",
            Self::WrongKind { .. } => "stream.command.kind",
            Self::MissingGuildId => "stream.command.guild_id.missing",
            Self::UnexpectedGuildId => "stream.command.guild_id.unexpected",
            Self::SessionNotEstablished => "stream.session.not_established",
        }
    }
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey { reason } => write!(f, "некорректный ключ стрима: {reason}"),
            Self::MissingField { field } => write!(f, "отсутствует обязательное поле `{field}`"),
            Self::WrongFieldType { field, expected } => {
                write!(f, "поле `{field}` должно иметь тип {expected}")
            }
            Self::InvalidId { field } => write!(f, "некорректный идентификатор в поле `{field}`"),
            Self::InvalidRegion => f.write_str(
                "некорректный регион стрима (ожидается непустой ASCII до 64 байт без пробелов)",
            ),
            Self::InvalidPaused => f.write_str("поле `paused` должно быть строгим boolean"),
            Self::WrongKind { expected, found } => {
                write!(
                    f,
                    "неверный тип стрима: ожидался {expected}, получен {found}"
                )
            }
            Self::MissingGuildId => {
                f.write_str("для стрима типа `guild` обязателен идентификатор `guild_id`")
            }
            Self::UnexpectedGuildId => {
                f.write_str("для стрима типа `call` поле `guild_id` должно быть null")
            }
            Self::SessionNotEstablished => {
                f.write_str("сессия шлюза не установлена: отправка команды стрима отклонена")
            }
        }
    }
}

impl std::error::Error for StreamError {}

/// Входное действие чистого редьюсера [`apply`] (§4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamAction {
    /// Входящее серверное событие `STREAM_*` с номером последовательности `s`.
    Dispatch {
        /// Номер последовательности `s` из `op 0`.
        seq: Option<u64>,
        /// Разобранное событие `STREAM_*`.
        event: StreamDispatch,
    },
    /// Локальное намерение пользователя/клиента.
    Local(LocalStreamAction),
    /// Новый `READY`: полный сброс runtime-состояния стримов и закрытие открытых RTC.
    Ready,
}

/// Локальные интенты пользователя/клиента (§4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalStreamAction {
    /// Запрошен старт собственного стрима (без optimistic-записи, `A-REDUCER-START`).
    StartRequested {
        /// Тип стрима.
        kind: StreamKind,
        /// Гильдия (`Some` для `Guild`).
        guild_id: Option<u64>,
        /// Канал.
        channel_id: u64,
    },
    /// Запрошен просмотр чужого стрима (`Connecting`, `A-REDUCER-WATCH`).
    WatchRequested {
        /// Ключ стрима.
        key: StreamKey,
    },
    /// Запрошена остановка стрима (фаза не меняется до прихода `STREAM_DELETE`, `A-STOP-HELPER`).
    StopRequested {
        /// Ключ стрима.
        key: StreamKey,
    },
    /// Запрошено переключение паузы (фаза не меняется до `STREAM_UPDATE`, `A-PAUSE-UI`).
    SetPausedRequested {
        /// Ключ стрима.
        key: StreamKey,
        /// Желаемое значение паузы.
        paused: bool,
    },
    /// Принудительное закрытие и удаление записи стрима (`STREAM_CLOSE`).
    Close {
        /// Ключ стрима.
        key: StreamKey,
    },
}

/// H-VIEWERS (§4.4): `viewer_ids` в `STREAM_CREATE`/`STREAM_UPDATE` — полный список, не дельта;
/// `Some` заменяет прежний список целиком, `None` в `UPDATE` сохраняет прежний.
fn merge_viewer_ids(previous: Option<Vec<u64>>, incoming: Option<Vec<u64>>) -> Option<Vec<u64>> {
    match incoming {
        Some(list) => Some(list),
        None => previous,
    }
}

/// H-PAUSE (§4.4): `paused` в событиях — состояние этого стрима; отсутствие поля в `UPDATE`
/// не сбрасывает паузу.
fn resolve_paused(previous: bool, incoming: Option<bool>) -> bool {
    incoming.unwrap_or(previous)
}

/// Чистый редьюсер фаз и эффектов Go Live / Stream (§4.1–§4.5).
///
/// Якоря: `A-REDUCER-CREATEUP` (`@8966402`), `A-REDUCER-DELETE` (`@8972115`).
pub fn apply(state: &mut StreamState, action: StreamAction) -> Vec<StreamEffect> {
    match action {
        StreamAction::Ready => {
            let previous = std::mem::take(&mut state.streams);
            state.last_seq = None;
            previous
                .into_iter()
                .filter_map(|(key, stream)| {
                    stream
                        .transport_ready
                        .then_some(StreamEffect::CloseStreamRtc { key })
                })
                .collect()
        }
        StreamAction::Local(local) => match local {
            LocalStreamAction::StartRequested { .. } => {
                // Запись создаётся только по серверному STREAM_CREATE (§4.2).
                Vec::new()
            }
            LocalStreamAction::WatchRequested { key } => {
                state.streams.insert(
                    key.clone(),
                    Stream {
                        key,
                        phase: StreamPhase::Connecting,
                        region: None,
                        viewer_ids: None,
                        rtc_server_id: None,
                        rtc_channel_id: None,
                        paused: false,
                        delete_reason: None,
                        transport_ready: false,
                    },
                );
                Vec::new()
            }
            LocalStreamAction::StopRequested { .. }
            | LocalStreamAction::SetPausedRequested { .. } => {
                // Optimistic-переходы запрещены (§4.2).
                Vec::new()
            }
            LocalStreamAction::Close { key } => {
                if let Some(removed) = state.streams.remove(&key)
                    && removed.transport_ready
                {
                    return vec![StreamEffect::CloseStreamRtc { key }];
                }
                Vec::new()
            }
        },
        StreamAction::Dispatch { seq, event } => {
            if let Some(s) = seq {
                if let Some(last) = state.last_seq
                    && s <= last
                {
                    state.diagnostics.stale_sequences.push(s);
                    return Vec::new();
                }
                state.last_seq = Some(s);
            }

            match event {
                StreamDispatch::Created(created) => {
                    let paused = resolve_paused(false, created.paused);
                    let phase = if paused {
                        StreamPhase::Paused
                    } else {
                        StreamPhase::Active
                    };
                    state.streams.insert(
                        created.stream_key.clone(),
                        Stream {
                            key: created.stream_key,
                            phase,
                            region: created.region,
                            viewer_ids: merge_viewer_ids(None, created.viewer_ids),
                            rtc_server_id: created.rtc_server_id,
                            rtc_channel_id: created.rtc_channel_id,
                            paused,
                            delete_reason: None,
                            transport_ready: false,
                        },
                    );
                    Vec::new()
                }
                StreamDispatch::Update(update) => {
                    if let Some(entry) = state.streams.get_mut(&update.stream_key) {
                        let paused = resolve_paused(entry.paused, update.paused);
                        entry.paused = paused;
                        entry.phase = if paused {
                            StreamPhase::Paused
                        } else {
                            StreamPhase::Active
                        };
                        if let Some(region) = update.region {
                            entry.region = Some(region);
                        }
                        entry.viewer_ids =
                            merge_viewer_ids(entry.viewer_ids.take(), update.viewer_ids);
                    } else {
                        let paused = resolve_paused(false, update.paused);
                        let phase = if paused {
                            StreamPhase::Paused
                        } else {
                            StreamPhase::Active
                        };
                        state.streams.insert(
                            update.stream_key.clone(),
                            Stream {
                                key: update.stream_key,
                                phase,
                                region: update.region,
                                viewer_ids: merge_viewer_ids(None, update.viewer_ids),
                                rtc_server_id: None,
                                rtc_channel_id: None,
                                paused,
                                delete_reason: None,
                                transport_ready: false,
                            },
                        );
                    }
                    Vec::new()
                }
                StreamDispatch::ServerUpdate(server_update) => {
                    // H-SERVER-UPDATE (§4.4): STREAM_SERVER_UPDATE не меняет фазу, а только
                    // помечает transport_ready и открывает второй RTC.
                    if let Some(entry) = state.streams.get_mut(&server_update.stream_key) {
                        entry.transport_ready = true;
                    } else {
                        state.diagnostics.orphan_server_updates += 1;
                    }
                    vec![StreamEffect::OpenStreamRtc {
                        key: server_update.stream_key,
                        endpoint: server_update.endpoint,
                        token: server_update.token,
                    }]
                }
                StreamDispatch::Deleted(deleted) => {
                    if let Some(StreamDeleteReason::Unknown(ref raw)) = deleted.reason {
                        state.diagnostics.unknown_reasons.push(raw.clone());
                    }

                    let Some(entry) = state.streams.get_mut(&deleted.stream_key) else {
                        state.diagnostics.unknown_streams += 1;
                        if deleted.reason == Some(StreamDeleteReason::StreamFull) {
                            return vec![StreamEffect::NotifyViewerFull {
                                key: deleted.stream_key,
                            }];
                        }
                        return Vec::new();
                    };

                    let prev_phase = entry.phase;
                    let should_close_rtc = entry.transport_ready
                        || matches!(prev_phase, StreamPhase::Active | StreamPhase::Paused);
                    entry.transport_ready = false;
                    entry.delete_reason = deleted.reason.clone();

                    let mut effects = Vec::new();
                    if should_close_rtc {
                        effects.push(StreamEffect::CloseStreamRtc {
                            key: deleted.stream_key.clone(),
                        });
                    }

                    // Приоритет правил A-REDUCER-DELETE (§4.1):
                    // unavailable -> Unauthorized -> SafetyGuildRateLimited -> StreamFull -> (Failed + UserRequested) -> Ended.
                    if deleted.unavailable == Some(true) {
                        entry.phase = StreamPhase::Reconnecting;
                    } else if deleted.reason == Some(StreamDeleteReason::Unauthorized) {
                        entry.phase = StreamPhase::Failed;
                    } else if deleted.reason == Some(StreamDeleteReason::SafetyGuildRateLimited) {
                        entry.phase = StreamPhase::Ended;
                        if let Some(guild_id) = deleted.stream_key.guild_id {
                            effects.push(StreamEffect::ShowSafetyRateLimit { guild_id });
                        }
                    } else if deleted.reason == Some(StreamDeleteReason::StreamFull) {
                        entry.phase = StreamPhase::Ended;
                        effects.push(StreamEffect::NotifyViewerFull {
                            key: deleted.stream_key,
                        });
                    } else if prev_phase == StreamPhase::Failed
                        && deleted.reason == Some(StreamDeleteReason::UserRequested)
                    {
                        entry.phase = StreamPhase::Failed;
                    } else {
                        // H-DELETE-END (§4.4): Ended не удаляет запись сам; запись живёт до Close/нового CREATE.
                        entry.phase = StreamPhase::Ended;
                    }

                    effects
                }
            }
        }
    }
}

fn is_null(value: &EtfValue) -> bool {
    matches!(value, EtfValue::Nil) || matches!(value, EtfValue::Atom(name) if name == "nil")
}

fn strict_str(value: &EtfValue) -> Option<&str> {
    match value {
        EtfValue::Str(s) => Some(s.as_str()),
        EtfValue::Atom(s) if !matches!(s.as_str(), "nil" | "true" | "false") => Some(s.as_str()),
        _ => None,
    }
}

fn strict_bool(value: &EtfValue) -> Option<bool> {
    match value {
        EtfValue::Atom(name) if name == "true" => Some(true),
        EtfValue::Atom(name) if name == "false" => Some(false),
        _ => None,
    }
}

fn is_valid_region(region: &str) -> bool {
    !region.is_empty()
        && region.len() <= MAX_REGION_BYTES
        && region
            .bytes()
            .all(|b| b.is_ascii() && !b.is_ascii_control() && !b.is_ascii_whitespace())
}

#[allow(non_snake_case)]
fn parse_u64_id_Allow_zero(value: &EtfValue, field: &'static str) -> Result<u64, StreamError> {
    match value {
        EtfValue::Int(n) if *n >= 0 => {
            u64::try_from(*n).map_err(|_| StreamError::InvalidId { field })
        }
        EtfValue::Str(s) | EtfValue::Atom(s) => {
            if s == "0" {
                return Ok(0);
            }
            if s.is_empty()
                || s.starts_with('0')
                || s.len() > 20
                || !s.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(StreamError::InvalidId { field });
            }
            s.parse::<u64>()
                .map_err(|_| StreamError::InvalidId { field })
        }
        _ => Err(StreamError::InvalidId { field }),
    }
}

fn parse_positive_snowflake_etf(value: &EtfValue) -> Option<u64> {
    match value {
        EtfValue::Int(n) if *n > 0 => u64::try_from(*n).ok(),
        EtfValue::Str(s) | EtfValue::Atom(s) => {
            parse_snowflake_segment(s, 0).ok().filter(|id| *id > 0)
        }
        _ => None,
    }
}

fn parse_required_stream_key(map: &EtfValue) -> Result<StreamKey, StreamError> {
    let Some(raw) = map.get("stream_key") else {
        return Err(StreamError::MissingField {
            field: "stream_key",
        });
    };
    if is_null(raw) {
        return Err(StreamError::MissingField {
            field: "stream_key",
        });
    }
    let Some(key_str) = strict_str(raw) else {
        return Err(StreamError::InvalidKey {
            reason: StreamKeyError::Empty,
        });
    };
    StreamKey::parse(key_str).map_err(|reason| StreamError::InvalidKey { reason })
}

fn parse_required_non_empty_str(
    map: &EtfValue,
    field: &'static str,
) -> Result<String, StreamError> {
    let Some(raw) = map.get(field) else {
        return Err(StreamError::MissingField { field });
    };
    if is_null(raw) {
        return Err(StreamError::MissingField { field });
    }
    let Some(text) = strict_str(raw) else {
        return Err(StreamError::WrongFieldType {
            field,
            expected: "str",
        });
    };
    if text.is_empty() {
        return Err(StreamError::MissingField { field });
    }
    Ok(text.to_string())
}

fn parse_soft_region(
    map: &EtfValue,
    field: &'static str,
    diagnostics: &mut StreamDiagnostics,
) -> Option<String> {
    let raw = map.get(field)?;
    if is_null(raw) {
        return None;
    }
    match strict_str(raw) {
        Some(s) if is_valid_region(s) => Some(s.to_string()),
        _ => {
            diagnostics.malformed_fields.push(field.to_string());
            None
        }
    }
}

fn parse_soft_viewer_ids(map: &EtfValue, diagnostics: &mut StreamDiagnostics) -> Option<Vec<u64>> {
    let raw = map.get("viewer_ids")?;
    if is_null(raw) {
        return None;
    }
    let items = match raw {
        EtfValue::List(items) | EtfValue::Tuple(items) => items,
        _ => {
            diagnostics.malformed_fields.push("viewer_ids".to_string());
            return None;
        }
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if let Some(id) = parse_positive_snowflake_etf(item) {
            out.push(id);
        } else {
            diagnostics.malformed_fields.push("viewer_ids".to_string());
        }
    }
    Some(out)
}

fn parse_soft_snowflake_string(
    map: &EtfValue,
    field: &'static str,
    diagnostics: &mut StreamDiagnostics,
) -> Option<String> {
    let raw = map.get(field)?;
    if is_null(raw) {
        return None;
    }
    if let Some(id) = parse_positive_snowflake_etf(raw) {
        Some(id.to_string())
    } else {
        diagnostics.malformed_fields.push(field.to_string());
        None
    }
}

fn parse_soft_snowflake_u64(
    map: &EtfValue,
    field: &'static str,
    diagnostics: &mut StreamDiagnostics,
) -> Option<u64> {
    let raw = map.get(field)?;
    if is_null(raw) {
        return None;
    }
    if let Some(id) = parse_positive_snowflake_etf(raw) {
        Some(id)
    } else {
        diagnostics.malformed_fields.push(field.to_string());
        None
    }
}

fn parse_soft_bool(
    map: &EtfValue,
    field: &'static str,
    diagnostics: &mut StreamDiagnostics,
) -> Option<bool> {
    let raw = map.get(field)?;
    if is_null(raw) {
        return None;
    }
    if let Some(b) = strict_bool(raw) {
        Some(b)
    } else {
        diagnostics.malformed_fields.push(field.to_string());
        None
    }
}

fn parse_soft_delete_reason(
    map: &EtfValue,
    diagnostics: &mut StreamDiagnostics,
) -> Option<StreamDeleteReason> {
    let raw = map.get("reason")?;
    if is_null(raw) {
        return None;
    }
    match strict_str(raw) {
        Some(s) if !s.is_empty() => Some(StreamDeleteReason::from_wire(s)),
        _ => {
            diagnostics.malformed_fields.push("reason".to_string());
            None
        }
    }
}

fn record_ignored_fields(
    pairs: &[(EtfValue, EtfValue)],
    allowed: &[&str],
    diagnostics: &mut StreamDiagnostics,
) {
    for (k, _) in pairs {
        if let EtfValue::Str(name) | EtfValue::Atom(name) = k
            && !allowed.contains(&name.as_str())
        {
            diagnostics.ignored_fields.push(name.clone());
        }
    }
}

/// Подмодуль ключа (§3.1).
pub mod key {
    pub use super::{MAX_KEY_BYTES, StreamKey, StreamKeyError, StreamKind, StreamType};
}

/// Подмодуль команд и опкодов (§3.2).
pub mod command {
    pub use super::{GatewaySessionState, StreamCommand, StreamOp};
}

/// Подмодуль входящих диспатчей и секретов (§3.3).
pub mod event {
    pub use super::{
        StreamCreated, StreamDeleteReason, StreamDeleted, StreamDispatch, StreamSecret,
        StreamServerUpdate, StreamUpdate, parse_dispatch, parse_dispatch_with_diagnostics,
    };
}

/// Подмодуль фаз, состояния и редьюсера (§3.4, §4).
pub mod state {
    pub use super::{
        LocalStreamAction, Stream, StreamAction, StreamDiagnostics, StreamEffect, StreamPhase,
        StreamSnapshot, StreamState, StreamSummary, apply,
    };
}

/// Подмодуль ошибок (§3.5).
pub mod error {
    pub use super::StreamError;
}
