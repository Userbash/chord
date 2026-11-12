//! Помощники для тестов состояния (RW-13 / P2-02).
//!
//! Строят только `EtfValue` в форме провода: ключи — атомы, строки — `Str`, целые — `Int`,
//! `nil`/`true`/`false` — атомы. Курированные фикстуры читаются через
//! [`super::ready_fixture`], поэтому оракул остаётся реальным артефактом, а не копией
//! реализации.

use chord_codec::etf::EtfValue;

/// Атом (`nil`, `true`, `false`, `invisible`, …).
pub fn atom(value: &str) -> EtfValue {
    EtfValue::Atom(value.to_string())
}

/// Строка провода (включая десятичные snowflake-иды).
pub fn text(value: &str) -> EtfValue {
    EtfValue::Str(value.to_string())
}

/// Целое.
pub fn int(value: i64) -> EtfValue {
    EtfValue::Int(value)
}

/// Map с ключами-атомами в заданном порядке.
pub fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(key), value))
            .collect(),
    )
}

/// Список.
pub fn list(values: Vec<EtfValue>) -> EtfValue {
    EtfValue::List(values)
}

/// Полный op0-кадр с `t`, `s`, `d`.
pub fn frame(name: &str, seq: u64, data: EtfValue) -> EtfValue {
    map(vec![
        ("op", int(0)),
        ("t", atom(name)),
        ("s", int(i64::try_from(seq).expect("seq в i64"))),
        ("d", data),
    ])
}

/// Прочитать реальную курированную фикстуру.
pub fn load(name: &str) -> EtfValue {
    super::ready_fixture::load(name).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// Минимальный валидный READY: `user.id=100`, `session_id=session-secret`, без гильдий.
pub fn minimal_ready_frame() -> EtfValue {
    frame(
        "READY",
        1,
        map(vec![
            ("user", map(vec![("id", text("100"))])),
            ("guilds", list(Vec::new())),
            ("session_id", text("session-secret")),
        ]),
    )
}

/// Канал гильдии с минимально обязательными полями (`id`, `type`).
pub fn channel(id: &str, name: &str) -> EtfValue {
    map(vec![
        ("id", text(id)),
        ("type", int(0)),
        ("name", text(name)),
        ("position", int(0)),
    ])
}

/// `CHANNEL_INFO` с одним/несколькими каналами.
pub fn channel_info_frame(seq: u64, guild_id: &str, channels: Vec<EtfValue>) -> EtfValue {
    frame(
        "CHANNEL_INFO",
        seq,
        map(vec![
            ("guild_id", text(guild_id)),
            ("channels", list(channels)),
        ]),
    )
}

/// Тред с полями из реальной фикстуры `004-THREAD_LIST_SYNC.json`.
pub fn thread(id: &str, guild_id: &str, parent_id: &str, name: &str) -> EtfValue {
    map(vec![
        ("id", text(id)),
        ("guild_id", text(guild_id)),
        ("parent_id", text(parent_id)),
        ("name", text(name)),
        ("type", int(11)),
        ("owner_id", text("900")),
        ("last_message_id", text("901")),
        ("message_count", int(5)),
        ("member_count", int(0)),
        ("total_message_sent", int(10)),
        ("rate_limit_per_user", int(0)),
        ("flags", int(2)),
        ("member_ids_preview", list(Vec::new())),
        (
            "thread_metadata",
            map(vec![
                ("archived", atom("false")),
                ("locked", atom("false")),
                ("invitable", atom("true")),
                ("auto_archive_duration", int(60)),
                (
                    "archive_timestamp",
                    text("2024-05-30T14:17:04.195000+00:00"),
                ),
                ("create_timestamp", text("2024-05-30T14:08:38.531000+00:00")),
            ]),
        ),
    ])
}

/// `THREAD_LIST_SYNC` с валидными id.
pub fn thread_sync_frame(seq: u64, guild_id: &str, threads: Vec<EtfValue>) -> EtfValue {
    frame(
        "THREAD_LIST_SYNC",
        seq,
        map(vec![
            ("guild_id", text(guild_id)),
            ("most_recent_messages", atom("nil")),
            ("threads", list(threads)),
        ]),
    )
}

/// `MESSAGE_CREATE` с минимальным набором полей.
pub fn message_create_frame(seq: u64, id: &str, channel_id: &str, content: &str) -> EtfValue {
    frame(
        "MESSAGE_CREATE",
        seq,
        map(vec![
            ("id", text(id)),
            ("channel_id", text(channel_id)),
            ("content", text(content)),
            ("type", int(0)),
            ("flags", int(0)),
            ("pinned", atom("false")),
            ("tts", atom("false")),
            ("mention_everyone", atom("false")),
        ]),
    )
}
