//! # Сессия шлюза Discord: детерминированное ядро (RW-03, T-GW-01…03)
//!
//! Обработка HELLO, планирование heartbeat (интервал 41 250 мс; первый удар — jitter от сида),
//! учёт ACK/RTT, детектор зомби, отслеживание `s`-последовательности и реакции на op7/op9.
//! **Транспорта здесь нет** (сокет/TLS/WS — RW-04): часы виртуальные — `now_ms: u64` передаёт
//! вызывающий, `std::time` не используется, поведение детерминировано и воспроизводимо.
//!
//! Контракт (ТЗ RW-03; проверен реплеем реального таймлайна `95974.1389` в
//! `tests/gateway_session.rs`):
//! 1. op10 HELLO: `d.heartbeat_interval` → [`Hello`]; первый удар — `now + jitter_seed % interval`;
//!    повторный HELLO до первого удара пере-планирует расписание, после первого удара —
//!    игнорируется;
//! 2. op11 ACK: закрывает удар (`rtt = now − момент удара`), следующий удар — `ACK + interval`;
//!    ACK без отправленного удара («лишний» ACK между ударами в захвате) игнорируется;
//! 3. [`Session::on_timer`]: нет ACK за интервал → `Reconnect{Zombie}` (сессия мертва); иначе —
//!    удар по сроку, `d` = последний `s` (до первого диспатча — `null`);
//! 4. op0 DISPATCH: `s` (int) → последняя последовательность; `t`/`d` не анализируются;
//! 5. op7 → `Reconnect{ServerRequest}`, op9 → `Reconnect{InvalidSession}`; сессия мертва;
//! 6. прочие опкоды и значения без целого `op` — без эффектов (счётчик
//!    [`Session::ignored_values`]); мусорные байты → [`GatewayError::Decode`], нарушенная
//!    структура разобранного терма → [`GatewayError::Malformed`];
//! 7. [`Session::next_deadline_ms`]: min(срок удара, дедлайн зомби) либо `None`.
//!
//! Паник на входных данных нет: арифметика — saturating, нулевой интервал HELLO отвергается как
//! `Malformed` (деление по модулю не выполняется), `unsafe` запрещён линтом крейта. Ошибки не
//! глотаются молча: [`Session::on_term`] разбирает **ровно один** терм на кадр (хвост —
//! `Malformed`) и остаётся честным даже у мёртвой сессии; [`Session::on_value`] мёртвой сессии
//! возвращает `Ok` без эффектов — это документированное «молчание» мёртвой сессии.

use crate::state::PresenceStatus;
use discord_codec::etf::{self, EtfValue};
use std::collections::BTreeMap;

pub mod supervisor;

/// Оп-код QoS-heartbeat (`QOS_HEARTBEAT`) из таблицы клиента 1.0.162 (AUTH-RE G-12).
///
/// В [`Op`] кода 40 нет намеренно: сервер его не присылает, а `Op::from_code` — контракт
/// RW-03 (`Op::from_code(40) == Op::Unknown(40)`). Клиентская сторона различает heartbeat по
/// [`HeartbeatMode`].
pub const QOS_HEARTBEAT_OP: i64 = 40;

/// Какой кадр клиент шлёт на удар: боевой op40 или legacy op1 (rw-03/04).
///
/// `Session::new` оставляет [`HeartbeatMode::LegacyOp1`] — обратная совместимость ядра RW-03;
/// боевой путь (супервайзер, транспорт RW-39) включает [`HeartbeatMode::Qos`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeartbeatMode {
    /// Legacy op1 `{op:1, d:seq}` — старый контракт RW-03/04.
    #[default]
    LegacyOp1,
    /// Боевой op40 `{op:40, d:{seq, qos}}` (AUTH-RE G-12).
    Qos,
}

/// QoS-полезная нагрузка op40: `{active, reasons}` (из `heartbeatQOSState` клиента).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QosPayload {
    /// Пользователь активен (клиент шлёт activity-состояние).
    pub active: bool,
    /// Причины активности (в клиенте — отсортированный уникальный список).
    pub reasons: Vec<String>,
}

impl QosPayload {
    /// Неактивное состояние с пустым списком причин.
    pub fn inactive() -> Self {
        Self {
            active: false,
            reasons: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Исходящие клиентские опкоды: op3, op4, op8, op37 (BE-P1-01 / F-GW-02)
// ---------------------------------------------------------------------------
//
// Формы тел — дословно из клиента 1.0.162 (бандл
// `out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`):
//   * op3  `presenceUpdate(status,since,activities,afk)`  @4 998 360;
//   * op4  `voiceStateUpdate({guildId,channelId,selfMute,selfDeaf,selfVideo,flags,…})`
//          → `{guild_id, channel_id, self_mute, self_deaf, self_video, flags}`;
//   * op8  `requestGuildMembers(guild,{query,limit,userIds,presences})`
//          → `{guild_id, query, limit, user_ids, presences}` (плюс необязательный `nonce`);
//   * op37 `updateGuildSubscriptions(subs)` → `{subscriptions:{<guild_id>:<sub>}}`;
//          `<sub>` — `get(guild)` из `SubscriptionManager` @9 112 505:
//          `{typing, threads, activities, members, member_updates, channels}`.
//
// Кодирование — как у клиентского wetf-энкодера (`encoding:{string:"binary",key:"binary",
// array:"list",null:"atom",undefined:"null"}`): ключи и строки — BINARY_EXT, массивы — LIST_EXT,
// `null`/`undefined` — NIL_EXT, булевы значения — атомы `"true"`/`"false"`.
// Исходящие кадры в захвате маскированы (`sha256+len`), поэтому оракул — код клиента и
// байтовая раскладка ETF, а не декод дампа (`gateway-protocol.json` `sentGateway`).

/// Оп-код исходящего op3 PRESENCE_UPDATE (таблица клиента, бандл @4 996 243).
pub const PRESENCE_UPDATE_OP: i64 = 3;
/// Оп-код исходящего op4 VOICE_STATE_UPDATE.
pub const VOICE_STATE_UPDATE_OP: i64 = 4;
/// Оп-код исходящего op8 REQUEST_GUILD_MEMBERS.
pub const REQUEST_GUILD_MEMBERS_OP: i64 = 8;
/// Оп-код исходящего op37 GUILD_SUBSCRIPTIONS_BULK.
pub const GUILD_SUBSCRIPTIONS_BULK_OP: i64 = 37;

/// Максимальный размер одного ETF-кадра OP 37 GUILD_SUBSCRIPTIONS_BULK (14.5 KiB = 15 360 байт по оракулу Discord 1.0.162).
pub const MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES: usize = 15_360;

/// Пакет подписок на гильдии: карта `guild_id -> OutboundGuildSubscription`.
pub type GuildSubscriptionsBulk = BTreeMap<String, OutboundGuildSubscription>;

/// Алиас ошибки кодирования исходящих кадров.
pub type CodecError = OutboundError;

/// Максимум `user_ids` в одном op8-кадре: клиент режет `.chunk(100)` (@4 954 008).
pub const GUILD_MEMBERS_CHUNK_SIZE: usize = 100;

/// Ошибка сборки исходящего кадра — не паника.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundError {
    /// Значение не влезает в wire-лимиты ETF (binary > `u32`, atom > `u16`, контейнер > `u32`).
    Encode {
        /// Текст ошибки энкодера.
        msg: String,
    },
}

impl std::fmt::Display for OutboundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode { msg } => write!(f, "исходящий кадр не закодирован: {msg}"),
        }
    }
}

impl std::error::Error for OutboundError {}

/// Тело op3: `{status, since, activities, afk}`.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboundPresenceUpdate {
    /// Проводной статус (`online`/`idle`/`dnd`/`invisible`).
    pub status: PresenceStatus,
    /// Unix-время смены статуса, мс; `0` — «не менять».
    pub since: u64,
    /// Активности как готовые ETF-термы (полная схема activity — вне этого файла).
    pub activities: Vec<EtfValue>,
    /// AFK-режим.
    pub afk: bool,
}

/// Тело op4: `{guild_id, channel_id, self_mute, self_deaf, self_video, flags}`.
///
/// `guild_id`/`channel_id` — `null` для личного звонка либо выхода из голоса.
/// Условные поля клиента (`preferred_region(s)`, `tracks`) не входят: их форма в захвате
/// не подтверждена, а продукт пока не стримит (см. отчёт task-139).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundVoiceStateUpdate {
    /// Гильдия; `None` → `null`.
    pub guild_id: Option<String>,
    /// Канал; `None` → `null`.
    pub channel_id: Option<String>,
    /// Микрофон выключен.
    pub self_mute: bool,
    /// Наушники выключены.
    pub self_deaf: bool,
    /// Камера включена.
    pub self_video: bool,
    /// Битовая маска клиента (обычно `0`).
    pub flags: u64,
}

/// Тело op8: `{guild_id, query, limit, user_ids, presences, nonce}`.
///
/// Два взаимоисключающих пути клиента: `query` + `limit` либо `user_ids`; отсутствующие поля
/// уходят как `null` (в клиенте `undefined:"null"`). `nonce` веб-клиент не шлёт; поле добавлено
/// по контракту API (эхо в `GUILD_MEMBERS_CHUNK` в захвате не наблюдалось).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundGuildMembersRequest {
    /// Гильдия.
    pub guild_id: String,
    /// Префикс имени; `None` → `null`.
    pub query: Option<String>,
    /// Лимит участников; `None` → `null`.
    pub limit: Option<u64>,
    /// Точечные id участников (не больше [`GUILD_MEMBERS_CHUNK_SIZE`] на кадр).
    pub user_ids: Vec<String>,
    /// Прислать presences вместе с участниками.
    pub presences: bool,
    /// Необязательный nonce для сопоставления чанка; `None` → `null`.
    pub nonce: Option<String>,
}

/// Один объект подписки op37 (`SubscriptionManager.get(guild)` @9 112 505).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundGuildSubscription {
    /// Подписка на typing.
    pub typing: bool,
    /// Подписка на threads.
    pub threads: bool,
    /// Подписка на activities.
    pub activities: bool,
    /// Точечные id участников (ключи `_members`).
    pub members: Vec<String>,
    /// Подписка на member updates.
    pub member_updates: bool,
    /// id каналов (ключи `_channels`).
    pub channels: Vec<String>,
}

/// Байты кадра op3 с ведущим `0x83`; ошибка — [`OutboundError`], не паника.
pub fn encode_presence_update(update: &OutboundPresenceUpdate) -> Result<Vec<u8>, OutboundError> {
    let payload = EtfValue::Map(vec![
        (
            key("status"),
            EtfValue::Str(update.status.as_str().to_string()),
        ),
        (key("since"), int(update.since)),
        (key("activities"), EtfValue::List(update.activities.clone())),
        (key("afk"), bool_atom(update.afk)),
    ]);
    encode_frame(PRESENCE_UPDATE_OP, payload)
}

/// Байты кадра op4 с ведущим `0x83`; ошибка — [`OutboundError`], не паника.
pub fn encode_voice_state_update(
    update: &OutboundVoiceStateUpdate,
) -> Result<Vec<u8>, OutboundError> {
    let payload = EtfValue::Map(vec![
        (key("guild_id"), optional_text(update.guild_id.as_deref())),
        (
            key("channel_id"),
            optional_text(update.channel_id.as_deref()),
        ),
        (key("self_mute"), bool_atom(update.self_mute)),
        (key("self_deaf"), bool_atom(update.self_deaf)),
        (key("self_video"), bool_atom(update.self_video)),
        (key("flags"), int(update.flags)),
    ]);
    encode_frame(VOICE_STATE_UPDATE_OP, payload)
}

/// Байты кадра op8 с ведущим `0x83`; ошибка — [`OutboundError`], не паника.
pub fn encode_request_guild_members(
    request: &OutboundGuildMembersRequest,
) -> Result<Vec<u8>, OutboundError> {
    let payload = EtfValue::Map(vec![
        (key("guild_id"), EtfValue::Str(request.guild_id.clone())),
        (key("query"), optional_text(request.query.as_deref())),
        (key("limit"), request.limit.map_or(EtfValue::Nil, int)),
        (
            key("user_ids"),
            if request.user_ids.is_empty() {
                EtfValue::Nil
            } else {
                text_list(&request.user_ids)
            },
        ),
        (key("presences"), bool_atom(request.presences)),
        (key("nonce"), optional_text(request.nonce.as_deref())),
    ]);
    encode_frame(REQUEST_GUILD_MEMBERS_OP, payload)
}

/// Байты кадра op37 с ведущим `0x83`; ошибка — [`OutboundError`], не паника.
pub fn encode_guild_subscriptions(
    subscriptions: &BTreeMap<String, OutboundGuildSubscription>,
) -> Result<Vec<u8>, OutboundError> {
    let guilds = subscriptions
        .iter()
        .map(|(guild_id, subscription)| {
            (
                EtfValue::Str(guild_id.clone()),
                EtfValue::Map(vec![
                    (key("typing"), bool_atom(subscription.typing)),
                    (key("threads"), bool_atom(subscription.threads)),
                    (key("activities"), bool_atom(subscription.activities)),
                    (key("members"), text_list(&subscription.members)),
                    (
                        key("member_updates"),
                        bool_atom(subscription.member_updates),
                    ),
                    (key("channels"), text_list(&subscription.channels)),
                ]),
            )
        })
        .collect();
    let payload = EtfValue::Map(vec![(key("subscriptions"), EtfValue::Map(guilds))]);
    encode_frame(GUILD_SUBSCRIPTIONS_BULK_OP, payload)
}

/// Кодирует одиночный ETF-кадр OP 37 для переданного набора подписок (обратная совместимость).
pub fn encode_guild_subscriptions_bulk(
    subscriptions: &GuildSubscriptionsBulk,
) -> Result<Vec<u8>, OutboundError> {
    encode_guild_subscriptions(subscriptions)
}

/// Разбивает большой словарь подписок гильдий на последовательность ETF-кадров OP 37
/// размером не более [`MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES`] (15 360 байт) каждый.
///
/// Если `subscriptions` пуст, возвращает пустой вектор кадров.
pub fn encode_guild_subscriptions_bulk_chunked(
    subscriptions: &GuildSubscriptionsBulk,
) -> Result<Vec<Vec<u8>>, CodecError> {
    if subscriptions.is_empty() {
        return Ok(Vec::new());
    }

    let mut frames = Vec::new();
    let mut current_chunk: GuildSubscriptionsBulk = BTreeMap::new();

    for (guild_id, sub) in subscriptions {
        current_chunk.insert(guild_id.clone(), sub.clone());
        let frame = encode_guild_subscriptions(&current_chunk)?;
        if frame.len() > MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES {
            if current_chunk.len() > 1 {
                current_chunk.remove(guild_id);
                let chunk_frame = encode_guild_subscriptions(&current_chunk)?;
                frames.push(chunk_frame);
                current_chunk.clear();
                current_chunk.insert(guild_id.clone(), sub.clone());
            } else {
                frames.push(frame);
                current_chunk.clear();
            }
        }
    }

    if !current_chunk.is_empty() {
        let frame = encode_guild_subscriptions(&current_chunk)?;
        frames.push(frame);
    }

    Ok(frames)
}

/// Режет `user_ids` на чанки по [`GUILD_MEMBERS_CHUNK_SIZE`] (клиент: `.chunk(100)`).
///
/// Пустой список даёт пустой набор чанков — op8 не отправляется.
pub fn chunk_user_ids(user_ids: &[String]) -> Vec<&[String]> {
    user_ids.chunks(GUILD_MEMBERS_CHUNK_SIZE).collect()
}

/// Кадр клиента: `{op: <code>, d: <payload>}` с ведущим `0x83`.
fn encode_frame(op: i64, payload: EtfValue) -> Result<Vec<u8>, OutboundError> {
    let frame = EtfValue::Map(vec![(key("op"), EtfValue::Int(op)), (key("d"), payload)]);
    etf::encode_checked(&frame).map_err(|error| OutboundError::Encode {
        msg: error.to_string(),
    })
}

/// Ключ map в форме клиента (`key:"binary"`).
fn key(name: &str) -> EtfValue {
    EtfValue::Str(name.to_string())
}

/// Неотрицательное целое; величина сверх `i64` насыщается (провод не паникует).
fn int(value: u64) -> EtfValue {
    EtfValue::Int(i64::try_from(value).unwrap_or(i64::MAX))
}

/// Булево значение — атом `"true"`/`"false"` (как в захвате).
fn bool_atom(value: bool) -> EtfValue {
    EtfValue::Atom(if value { "true" } else { "false" }.to_string())
}

/// Строка либо `NIL_EXT` (клиент: `null`/`undefined` → null).
fn optional_text(value: Option<&str>) -> EtfValue {
    value.map_or(EtfValue::Nil, |text| EtfValue::Str(text.to_string()))
}

/// Массив binary-строк.
fn text_list(items: &[String]) -> EtfValue {
    EtfValue::List(items.iter().cloned().map(EtfValue::Str).collect())
}

/// Оп-код шлюза: известные из захвата плюс «всё прочее» числом.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// op0 — DISPATCH (несёт `s`-последовательность).
    Dispatch,
    /// op1 — HEARTBEAT (запрос сервера; в `Qos`-режиме — немедленный удар, G-12).
    Heartbeat,
    /// op7 — RECONNECT.
    Reconnect,
    /// op9 — INVALID_SESSION.
    InvalidSession,
    /// op10 — HELLO.
    Hello,
    /// op11 — HEARTBEAT_ACK.
    HeartbeatAck,
    /// Любой иной оп-код (op2/op40/…): без эффектов.
    Unknown(i64),
}

impl Op {
    /// Оп-код числа → вариант; неизвестные коды сохраняются.
    pub fn from_code(code: i64) -> Self {
        match code {
            0 => Self::Dispatch,
            1 => Self::Heartbeat,
            7 => Self::Reconnect,
            9 => Self::InvalidSession,
            10 => Self::Hello,
            11 => Self::HeartbeatAck,
            other => Self::Unknown(other),
        }
    }
}

/// Полезная нагрузка HELLO (op10): интервал heartbeat из `d.heartbeat_interval`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello {
    /// Интервал heartbeat в миллисекундах (в захвате — 41 250).
    pub heartbeat_interval_ms: u64,
}

/// Причина реконнекта.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectReason {
    /// Нет ACK за интервал — сервер «зомби».
    Zombie,
    /// Сервер прислал op7 RECONNECT.
    ServerRequest,
    /// Сервер прислал op9 INVALID_SESSION.
    InvalidSession,
}

/// Эффект, который сессия просит выполнить снаружи (транспорт — вне этой волны).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Отправить heartbeat; `d` — последняя `s`-последовательность (до первого диспатча — `null`).
    SendHeartbeat {
        /// Последний `s` из op0 либо `None` (до первого диспатча).
        d: Option<u64>,
    },
    /// Переподключиться с указанной причиной.
    Reconnect {
        /// Почему нужен реконнект.
        reason: ReconnectReason,
    },
}

/// Ошибки сессии.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayError {
    /// Байты не разбираются как ETF-терм.
    Decode {
        /// Текст ошибки декодера.
        msg: String,
    },
    /// Терм разобран, но его структура нарушена (например, op10 без `heartbeat_interval`).
    Malformed {
        /// Описание нарушения.
        msg: String,
    },
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode { msg } => write!(f, "терм не декодирован: {msg}"),
            Self::Malformed { msg } => write!(f, "нарушена структура терма: {msg}"),
        }
    }
}

impl std::error::Error for GatewayError {}

/// Состояние одной сессии шлюза.
pub struct Session {
    /// Сид детерминированного jitter-а первого удара.
    jitter_seed: u64,
    /// HELLO, если уже пришёл (единственный источник интервала).
    hello: Option<Hello>,
    /// Срок ближайшего запланированного удара (ждём прихода виртуальных часов).
    next_beat_ms: Option<u64>,
    /// Момент отправки удара, ожидающего ACK (неответный удар).
    pending_beat_ms: Option<u64>,
    /// Последняя `s`-последовательность из op0.
    last_seq: Option<u64>,
    /// Последний измеренный RTT (ACK минус удар).
    rtt_ms: Option<u64>,
    /// Сколько heartbeat-ударов отправлено.
    beats_emitted: u64,
    /// Сколько значений проигнорировано (неизвестные/необрабатываемые опкоды).
    ignored_values: u64,
    /// Мёртва ли сессия (зомби либо оп7/оп9).
    dead: bool,
    /// Форма heartbeat: op40 (боевая) либо op1 (legacy).
    heartbeat_mode: HeartbeatMode,
    /// Текущая QoS-полезная нагрузка для op40 (`None` до первого active-состояния).
    qos: Option<QosPayload>,
}

impl Session {
    /// Новая сессия с заданным сидом jitter-а.
    pub fn new(jitter_seed: u64) -> Self {
        Self {
            jitter_seed,
            hello: None,
            next_beat_ms: None,
            pending_beat_ms: None,
            last_seq: None,
            rtt_ms: None,
            beats_emitted: 0,
            ignored_values: 0,
            dead: false,
            heartbeat_mode: HeartbeatMode::LegacyOp1,
            qos: None,
        }
    }

    /// Новая сессия с явной формой heartbeat (боевая — [`HeartbeatMode::Qos`]).
    pub fn with_heartbeat_mode(jitter_seed: u64, heartbeat_mode: HeartbeatMode) -> Self {
        let mut session = Self::new(jitter_seed);
        session.heartbeat_mode = heartbeat_mode;
        session
    }

    /// Форма heartbeat этой сессии.
    pub fn heartbeat_mode(&self) -> HeartbeatMode {
        self.heartbeat_mode
    }

    /// Переключает форму heartbeat (например, fallback на op1 без пересоздания сессии).
    pub fn set_heartbeat_mode(&mut self, heartbeat_mode: HeartbeatMode) {
        self.heartbeat_mode = heartbeat_mode;
    }

    /// Текущая QoS-нагрузка op40 (`None` — `qos:null`, как у клиента до active-состояния).
    pub fn qos(&self) -> Option<&QosPayload> {
        self.qos.as_ref()
    }

    /// Заменяет QoS-нагрузку для следующих ударов.
    pub fn set_qos(&mut self, qos: Option<QosPayload>) {
        self.qos = qos;
    }

    /// Байты провода (с ведущим `0x83`) → эффекты.
    ///
    /// Кадр обязан содержать **ровно один** терм: хвост после разобранного терма — `Malformed`
    /// (ошибки не глотаются). Мусорные байты — `Decode`; паники на входе нет.
    pub fn on_term(&mut self, term: &[u8], now_ms: u64) -> Result<Vec<Effect>, GatewayError> {
        let (value, consumed) = etf::decode(term).map_err(|error| GatewayError::Decode {
            msg: error.to_string(),
        })?;
        if consumed != term.len() {
            return Err(GatewayError::Malformed {
                msg: format!(
                    "кадр {} Б: разобран один терм на {consumed} Б (лишний хвост)",
                    term.len()
                ),
            });
        }
        self.on_value(&value, now_ms)
    }

    /// Разобранный терм → эффекты.
    pub fn on_value(&mut self, value: &EtfValue, now_ms: u64) -> Result<Vec<Effect>, GatewayError> {
        if self.dead {
            // Мёртвая сессия молчит: транспорт уже переподключается.
            return Ok(Vec::new());
        }
        let Some(code) = value.get("op").and_then(EtfValue::as_int) else {
            // Значение без целого `op` — не кадр шлюза: считаем и молчим (паники нет).
            self.ignored_values += 1;
            return Ok(Vec::new());
        };
        match Op::from_code(code) {
            Op::Hello => self.on_hello(value, now_ms),
            Op::HeartbeatAck => {
                self.on_ack(now_ms);
                Ok(Vec::new())
            }
            Op::Dispatch => self.on_dispatch(value),
            Op::Reconnect => Ok(vec![self.kill(ReconnectReason::ServerRequest)]),
            Op::InvalidSession => Ok(vec![self.kill(ReconnectReason::InvalidSession)]),
            Op::Heartbeat => match self.heartbeat_mode {
                // G-12: op1 от сервера — триггер немедленного heartbeat (боевой режим).
                HeartbeatMode::Qos => Ok(self.on_server_heartbeat(now_ms)),
                // Legacy RW-03: op1 сервер не присылает — значение игнорируется как раньше.
                HeartbeatMode::LegacyOp1 => {
                    self.ignored_values += 1;
                    Ok(Vec::new())
                }
            },
            Op::Unknown(_) => {
                // Сервер op40 не присылает; неизвестные коды не обрабатываются (RW-03).
                self.ignored_values += 1;
                Ok(Vec::new())
            }
        }
    }

    /// Тик виртуальных часов.
    ///
    /// Порядок проверок: мёртвая сессия → дедлайн зомби → срок удара. Зомби важнее удара:
    /// сервер, не ответивший на прошлый удар, не должен получать новый.
    pub fn on_timer(&mut self, now_ms: u64) -> Vec<Effect> {
        if self.dead {
            return Vec::new();
        }
        if self
            .zombie_deadline_ms()
            .is_some_and(|deadline| now_ms >= deadline)
        {
            self.dead = true;
            self.pending_beat_ms = None;
            self.next_beat_ms = None;
            return vec![Effect::Reconnect {
                reason: ReconnectReason::Zombie,
            }];
        }
        if self.next_beat_ms.is_some_and(|due| now_ms >= due) {
            self.beats_emitted += 1;
            self.pending_beat_ms = Some(now_ms);
            self.next_beat_ms = None;
            return vec![Effect::SendHeartbeat { d: self.last_seq }];
        }
        Vec::new()
    }

    /// Ближайший дедлайн (удар или зомби).
    pub fn next_deadline_ms(&self) -> Option<u64> {
        if self.dead {
            return None;
        }
        [self.next_beat_ms, self.zombie_deadline_ms()]
            .into_iter()
            .flatten()
            .min()
    }

    /// HELLO, если уже пришёл.
    pub fn hello(&self) -> Option<&Hello> {
        self.hello.as_ref()
    }

    /// Последняя `s`-последовательность из op0.
    pub fn last_seq(&self) -> Option<u64> {
        self.last_seq
    }

    /// Последний измеренный RTT (ACK минус удар).
    pub fn rtt_ms(&self) -> Option<u64> {
        self.rtt_ms
    }

    /// Сколько heartbeat-ударов отправлено.
    pub fn beats_emitted(&self) -> u64 {
        self.beats_emitted
    }

    /// Мёртва ли сессия (зомби либо оп7/оп9).
    pub fn is_dead(&self) -> bool {
        self.dead
    }

    /// Сколько значений проигнорировано (неизвестные/необрабатываемые опкоды).
    pub fn ignored_values(&self) -> u64 {
        self.ignored_values
    }

    /// Интервал heartbeat из принятого HELLO (до HELLO — `None`).
    fn interval_ms(&self) -> Option<u64> {
        self.hello.map(|hello| hello.heartbeat_interval_ms)
    }

    /// Дедлайн зомби: момент удара + интервал, пока ACK не пришёл; до первого удара — `None`.
    ///
    /// Инвариант: неответный удар появляется только после HELLO, поэтому интервал здесь всегда
    /// есть — второе `?` недостижимо, но не паникует.
    fn zombie_deadline_ms(&self) -> Option<u64> {
        let sent = self.pending_beat_ms?;
        Some(sent.saturating_add(self.interval_ms()?))
    }

    /// op10 HELLO: запомнить интервал и запланировать первый удар.
    fn on_hello(&mut self, value: &EtfValue, now_ms: u64) -> Result<Vec<Effect>, GatewayError> {
        // Повторный HELLO после первого удара игнорируется: расписание уже принято.
        if self.beats_emitted > 0 {
            return Ok(Vec::new());
        }
        let interval = value
            .get("d")
            .and_then(|d| d.get("heartbeat_interval"))
            .and_then(EtfValue::as_int)
            .and_then(|raw| u64::try_from(raw).ok())
            .filter(|interval| *interval > 0)
            .ok_or_else(|| GatewayError::Malformed {
                msg: "op10 HELLO: d.heartbeat_interval — положительное целое".to_string(),
            })?;
        self.hello = Some(Hello {
            heartbeat_interval_ms: interval,
        });
        // Первый удар — детерминированный jitter: seed % interval (замена random из клиента).
        self.next_beat_ms = Some(now_ms.saturating_add(self.jitter_seed % interval));
        Ok(Vec::new())
    }

    /// op11 HEARTBEAT_ACK: закрыть неответный удар и перенести расписание от ACK.
    fn on_ack(&mut self, now_ms: u64) {
        // ACK без отправленного удара — «лишний» ACK захвата (парные суммы ≈ интервал): игнорируем.
        let (Some(sent), Some(interval)) = (self.pending_beat_ms, self.interval_ms()) else {
            return;
        };
        self.pending_beat_ms = None;
        self.rtt_ms = Some(now_ms.saturating_sub(sent));
        // Следующий удар — через интервал от ACK (а не от расписания прошлого удара).
        self.next_beat_ms = Some(now_ms.saturating_add(interval));
    }

    /// op1 от сервера в боевом режиме: немедленный удар и перезапуск интервала (G-12/G-13).
    ///
    /// Клиент (`_handleHeartbeatReceive`) шлёт heartbeat сразу и пересоздаёт `setInterval`,
    /// поэтому следующий плановый удар — `now + interval`. Отправленный удар становится
    /// ожидающим ACK (дедлайн зомби — `now + interval`).
    fn on_server_heartbeat(&mut self, now_ms: u64) -> Vec<Effect> {
        self.beats_emitted = self.beats_emitted.saturating_add(1);
        if let Some(interval) = self.interval_ms() {
            self.pending_beat_ms = Some(now_ms);
            self.next_beat_ms = Some(now_ms.saturating_add(interval));
        }
        vec![Effect::SendHeartbeat { d: self.last_seq }]
    }

    /// op0 DISPATCH: обновить `s`-последовательность (идёт в `d` следующего удара).
    fn on_dispatch(&mut self, value: &EtfValue) -> Result<Vec<Effect>, GatewayError> {
        let seq = value
            .get("s")
            .and_then(EtfValue::as_int)
            .and_then(|raw| u64::try_from(raw).ok())
            .ok_or_else(|| GatewayError::Malformed {
                msg: "op0 DISPATCH: s — неотрицательное целое".to_string(),
            })?;
        self.last_seq = Some(seq);
        Ok(Vec::new())
    }

    /// Помечает сессию мёртвой (зомби уже случился/сервер распорядился) и снимает расписание.
    fn kill(&mut self, reason: ReconnectReason) -> Effect {
        self.dead = true;
        self.pending_beat_ms = None;
        self.next_beat_ms = None;
        Effect::Reconnect { reason }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Op, OutboundGuildMembersRequest, OutboundGuildSubscription, OutboundPresenceUpdate,
        OutboundVoiceStateUpdate, chunk_user_ids, encode_guild_subscriptions,
        encode_presence_update, encode_request_guild_members, encode_voice_state_update,
    };
    use crate::state::PresenceStatus;
    use discord_codec::etf::{self, EtfValue};
    use std::collections::BTreeMap;

    #[test]
    fn op_codes_match_protocol_table() {
        assert_eq!(Op::from_code(0), Op::Dispatch);
        assert_eq!(Op::from_code(1), Op::Heartbeat);
        assert_eq!(Op::from_code(11), Op::HeartbeatAck);
        assert_eq!(Op::from_code(-7), Op::Unknown(-7));
    }

    /// Кадр обязан быть ровно одним ETF-термом с ведущим version byte `0x83`.
    fn decode_frame(bytes: &[u8]) -> EtfValue {
        assert_eq!(bytes.first(), Some(&0x83), "ведущий version byte");
        let (value, consumed) = etf::decode(bytes).expect("кадр разбирается");
        assert_eq!(consumed, bytes.len(), "ровно один терм без хвоста");
        value
    }

    /// Ключи map-терма; ключ-атом — нарушение контракта клиента (`key:"binary"`).
    fn map_keys(value: &EtfValue) -> Vec<String> {
        value
            .as_map()
            .expect("ожидался map")
            .iter()
            .map(|(key, _)| match key {
                EtfValue::Str(name) => name.clone(),
                other => panic!("ключ не binary-строка: {other:?}"),
            })
            .collect()
    }

    fn text(value: Option<&EtfValue>) -> Option<&str> {
        value.and_then(EtfValue::as_str)
    }

    #[test]
    fn presence_update_frame_bytes_match_wetf_layout() {
        // Независимый байтовый оракул: MAP(2){op:3, d:MAP(4){status, since, activities, afk}}.
        // Ключи и строки — BINARY_EXT (0x6d), bool — ATOM_EXT (0x64), числа — SMALL_INTEGER_EXT.
        let frame = encode_presence_update(&OutboundPresenceUpdate {
            status: PresenceStatus::Online,
            since: 0,
            activities: Vec::new(),
            afk: false,
        })
        .expect("op3 кодируется");
        let expected = concat!(
            "83",
            "7400000002",
            "6d000000026f70",
            "6103",
            "6d0000000164",
            "7400000004",
            "6d00000006737461747573",
            "6d000000066f6e6c696e65",
            "6d0000000573696e6365",
            "6100",
            "6d0000000a61637469766974696573",
            "6c000000006a",
            "6d0000000361666b",
            "64000566616c7365",
        );
        assert_eq!(crate::codec::etf::encode(&decode_frame(&frame)), frame);
        assert_eq!(to_hex(&frame), expected);
    }

    #[test]
    fn presence_update_frame_passes_activity_terms_and_afk() {
        let activity = EtfValue::Map(vec![
            (EtfValue::Str("type".to_string()), EtfValue::Int(4)),
            (
                EtfValue::Str("name".to_string()),
                EtfValue::Str("Custom Status".to_string()),
            ),
        ]);
        let frame = encode_presence_update(&OutboundPresenceUpdate {
            status: PresenceStatus::Idle,
            since: 1_700_000,
            activities: vec![activity.clone()],
            afk: true,
        })
        .expect("op3 кодируется");
        let value = decode_frame(&frame);
        assert_eq!(map_keys(&value), ["op", "d"]);
        assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(3));
        let d = value.get("d").expect("d");
        assert_eq!(map_keys(d), ["status", "since", "activities", "afk"]);
        assert_eq!(text(d.get("status")), Some("idle"));
        assert_eq!(d.get("since").and_then(EtfValue::as_int), Some(1_700_000));
        assert_eq!(
            d.get("activities").and_then(EtfValue::as_list),
            Some([activity].as_slice())
        );
        assert_eq!(d.get("afk").and_then(EtfValue::as_atom), Some("true"));
    }

    #[test]
    fn voice_state_update_frame_matches_client_shape() {
        let frame = encode_voice_state_update(&OutboundVoiceStateUpdate {
            guild_id: Some("1014537851418980372".to_string()),
            channel_id: Some("1176785373989445703".to_string()),
            self_mute: true,
            self_deaf: false,
            self_video: true,
            flags: 2,
        })
        .expect("op4 кодируется");
        let value = decode_frame(&frame);
        assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(4));
        let d = value.get("d").expect("d");
        assert_eq!(
            map_keys(d),
            [
                "guild_id",
                "channel_id",
                "self_mute",
                "self_deaf",
                "self_video",
                "flags",
            ]
        );
        assert_eq!(text(d.get("guild_id")), Some("1014537851418980372"));
        assert_eq!(text(d.get("channel_id")), Some("1176785373989445703"));
        assert_eq!(d.get("self_mute").and_then(EtfValue::as_atom), Some("true"));
        assert_eq!(
            d.get("self_deaf").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(
            d.get("self_video").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(d.get("flags").and_then(EtfValue::as_int), Some(2));

        // Выход из голоса/личный звонок: оба id — null (NIL_EXT), не пустая строка.
        let dm = encode_voice_state_update(&OutboundVoiceStateUpdate {
            guild_id: None,
            channel_id: None,
            self_mute: false,
            self_deaf: false,
            self_video: false,
            flags: 0,
        })
        .expect("op4 кодируется");
        let dm = decode_frame(&dm);
        let d = dm.get("d").expect("d");
        assert_eq!(d.get("guild_id"), Some(&EtfValue::Nil));
        assert_eq!(d.get("channel_id"), Some(&EtfValue::Nil));
    }

    #[test]
    fn guild_members_request_covers_query_and_user_ids_paths() {
        // Путь query: user_ids/limit/nonce отсутствуют → NIL_EXT (клиент: undefined:"null").
        let query_frame = encode_request_guild_members(&OutboundGuildMembersRequest {
            guild_id: "1014537851418980372".to_string(),
            query: Some("monolit".to_string()),
            limit: Some(0),
            user_ids: Vec::new(),
            presences: true,
            nonce: None,
        })
        .expect("op8 кодируется");
        let value = decode_frame(&query_frame);
        assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(8));
        let d = value.get("d").expect("d");
        assert_eq!(
            map_keys(d),
            [
                "guild_id",
                "query",
                "limit",
                "user_ids",
                "presences",
                "nonce",
            ]
        );
        assert_eq!(text(d.get("guild_id")), Some("1014537851418980372"));
        assert_eq!(text(d.get("query")), Some("monolit"));
        assert_eq!(d.get("limit").and_then(EtfValue::as_int), Some(0));
        assert_eq!(d.get("user_ids"), Some(&EtfValue::Nil));
        assert_eq!(d.get("presences").and_then(EtfValue::as_atom), Some("true"));
        assert_eq!(d.get("nonce"), Some(&EtfValue::Nil));

        // Путь user_ids: query/limit отсутствуют, nonce эхом уходит в чанк.
        let ids_frame = encode_request_guild_members(&OutboundGuildMembersRequest {
            guild_id: "1014537851418980372".to_string(),
            query: None,
            limit: None,
            user_ids: vec![
                "384299970632876032".to_string(),
                "1176785373989445703".to_string(),
            ],
            presences: false,
            nonce: Some("chunk-1".to_string()),
        })
        .expect("op8 кодируется");
        let ids = decode_frame(&ids_frame);
        let d = ids.get("d").expect("d");
        assert_eq!(d.get("query"), Some(&EtfValue::Nil));
        assert_eq!(d.get("limit"), Some(&EtfValue::Nil));
        assert_eq!(
            d.get("user_ids").and_then(EtfValue::as_list),
            Some(
                [
                    EtfValue::Str("384299970632876032".to_string()),
                    EtfValue::Str("1176785373989445703".to_string()),
                ]
                .as_slice()
            )
        );
        assert_eq!(
            d.get("presences").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(text(d.get("nonce")), Some("chunk-1"));
    }

    #[test]
    fn user_ids_chunk_at_one_hundred() {
        let ids = |count: usize| -> Vec<String> { (0..count).map(|i| i.to_string()).collect() };

        assert!(
            chunk_user_ids(&ids(0)).is_empty(),
            "пустой запрос не шлёт чанк"
        );

        let hundred = ids(100);
        let chunks = chunk_user_ids(&hundred);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], hundred.as_slice());

        let hundred_one = ids(101);
        let chunks = chunk_user_ids(&hundred_one);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 100);
        assert_eq!(chunks[1], [100.to_string()].as_slice());
    }

    #[test]
    fn guild_subscriptions_frame_matches_client_shape() {
        let mut subscriptions = BTreeMap::new();
        subscriptions.insert(
            "1014537851418980372".to_string(),
            OutboundGuildSubscription {
                typing: true,
                threads: false,
                activities: true,
                members: vec!["384299970632876032".to_string()],
                member_updates: false,
                channels: vec![
                    "1176785373989445703".to_string(),
                    "1275727867497680918".to_string(),
                ],
            },
        );
        let frame = encode_guild_subscriptions(&subscriptions).expect("op37 кодируется");
        let value = decode_frame(&frame);
        assert_eq!(value.get("op").and_then(EtfValue::as_int), Some(37));
        let d = value.get("d").expect("d");
        assert_eq!(map_keys(d), ["subscriptions"]);
        let guilds = d.get("subscriptions").expect("subscriptions");
        assert_eq!(map_keys(guilds), ["1014537851418980372"]);
        let sub = guilds.get("1014537851418980372").expect("подписка гильдии");
        assert_eq!(
            map_keys(sub),
            [
                "typing",
                "threads",
                "activities",
                "members",
                "member_updates",
                "channels",
            ]
        );
        assert_eq!(sub.get("typing").and_then(EtfValue::as_atom), Some("true"));
        assert_eq!(
            sub.get("threads").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(
            sub.get("activities").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(
            sub.get("member_updates").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(
            sub.get("members").and_then(EtfValue::as_list),
            Some([EtfValue::Str("384299970632876032".to_string())].as_slice())
        );
        assert_eq!(
            sub.get("channels").and_then(EtfValue::as_list),
            Some(
                [
                    EtfValue::Str("1176785373989445703".to_string()),
                    EtfValue::Str("1275727867497680918".to_string()),
                ]
                .as_slice()
            )
        );
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn max_guild_subscriptions_frame_bytes_constant_matches_oracle() {
        assert_eq!(super::MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES, 15_360);
    }

    #[test]
    fn encode_guild_subscriptions_bulk_backward_compatible_with_single_frame() {
        let mut bulk: super::GuildSubscriptionsBulk = std::collections::BTreeMap::new();
        bulk.insert(
            "1014537851418980372".to_string(),
            OutboundGuildSubscription {
                typing: true,
                threads: false,
                activities: true,
                members: vec!["384299970632876032".to_string()],
                member_updates: false,
                channels: vec!["1176785373989445703".to_string()],
            },
        );
        let frame_bulk = super::encode_guild_subscriptions_bulk(&bulk).expect("bulk op37 кодируется");
        let frame_orig = encode_guild_subscriptions(&bulk).expect("orig op37 кодируется");
        assert_eq!(frame_bulk, frame_orig);
    }

    #[test]
    fn encode_guild_subscriptions_bulk_chunked_empty_yields_empty_frames() {
        let bulk: super::GuildSubscriptionsBulk = std::collections::BTreeMap::new();
        let chunks = super::encode_guild_subscriptions_bulk_chunked(&bulk).expect("пустой bulk");
        assert!(chunks.is_empty(), "пустой bulk не должен порождать кадров");
    }

    #[test]
    fn encode_guild_subscriptions_bulk_chunked_splits_oversized_payloads_into_frames_le_15360() {
        let mut bulk: super::GuildSubscriptionsBulk = std::collections::BTreeMap::new();
        // Генерируем 120 гильдий, каждая содержит по 10 каналов и 5 участников,
        // чтобы суммарный размер одиночного ETF превысил 15 360 байт (около 35-40 КБ).
        for i in 0..120 {
            let guild_id = format!("1014537851418980{:03}", i);
            let channels: Vec<String> = (0..10).map(|c| format!("1176785373989445{:02}{:02}", i, c)).collect();
            let members: Vec<String> = (0..5).map(|m| format!("3842999706328760{:02}{:02}", i, m)).collect();
            bulk.insert(
                guild_id,
                OutboundGuildSubscription {
                    typing: true,
                    threads: false,
                    activities: true,
                    members,
                    member_updates: false,
                    channels,
                },
            );
        }

        let single_frame = super::encode_guild_subscriptions_bulk(&bulk).expect("одиночный кадр");
        assert!(
            single_frame.len() > super::MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES,
            "тестовый payload обязан быть больше 15 360 байт (получено {} байт)",
            single_frame.len()
        );

        let chunks = super::encode_guild_subscriptions_bulk_chunked(&bulk)
            .expect("чанкованное кодирование должно пройти успешно");
        assert!(
            chunks.len() >= 2,
            "ожидалось как минимум 2 чанка, получено {}",
            chunks.len()
        );

        let mut collected_guilds = std::collections::BTreeSet::new();
        for (idx, frame) in chunks.iter().enumerate() {
            assert!(
                frame.len() <= super::MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES,
                "чанк #{} имеет размер {} байт, превышающий лимит {}",
                idx,
                frame.len(),
                super::MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES
            );
            assert_eq!(frame[0], 0x83, "каждый чанк должен начинаться с 0x83 wetf-маркера");
            let value = decode_frame(frame);
            assert_eq!(
                value.get("op").and_then(EtfValue::as_int),
                Some(37),
                "op должен быть 37 GUILD_SUBSCRIPTIONS_BULK"
            );
            let d = value.get("d").expect("d");
            let guilds_map = d.get("subscriptions").expect("subscriptions");
            for key in map_keys(guilds_map) {
                assert!(
                    collected_guilds.insert(key.clone()),
                    "дублирование гильдии {} между чанками",
                    key
                );
            }
        }

        assert_eq!(
            collected_guilds.len(),
            bulk.len(),
            "все гильдии обязаны присутствовать в объединении чанков без потерь"
        );
        for expected_guild in bulk.keys() {
            assert!(
                collected_guilds.contains(expected_guild),
                "гильдия {} потеряна при чанковании",
                expected_guild
            );
        }
    }
}
