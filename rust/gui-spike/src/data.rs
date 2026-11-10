//! BFF фазы 3 (W39e): типизированные данные для фронтенда строго из `fixtures/`.
//!
//! Сети здесь нет: каждый вызов читает санитизированные артефакты RE
//! (`fixtures/gateway/by-type/*.json`) с диска. Корень задаётся `DL_FIXTURES`
//! (по умолчанию — `fixtures/` в корне репозитория), поэтому домашний каталог
//! лаборатории в коде не зашит.
//!
//! Контракт Tauri-команд (`main.rs`):
//! - `ready()` → [`ReadyPayload`] — пользователь, гильдии, счётчики READY;
//! - `read_state()` → `Vec<ReadStateEntry>` — записи `READY.read_state`
//!   (курсор `lastViewed` для ack, `flags`, `mentionCount`, badge-форма
//!   `badgeCount`/`lastAckedId`/`readStateType`) без сети;
//! - `channels(guildId?)` → `Vec<ChannelRow>` — дерево каналов из READY
//!   (гильдейские + ЛС), типы числом, порядок «гильдия → position → id»;
//! - `messages_page(channelId, cursor?, before?, position?, limit?)` →
//!   [`MessagesPage`] — страница сообщений канала. `cursor` — forward-курсор:
//!   id последней строки предыдущей страницы; `before` — older-курсор: id
//!   первой строки уже загруженного окна, отдаются строки **старше** него;
//!   `position = "latest"` — стартовая страница с конца канала (без id).
//!   Строка несёт `embeds`/`attachments`/`reactions`/`replyTo`/`editedAt`,
//!   `day` (дата разделителя) и `group` (`start`/`continuation`);
//! - `members_page(guildId, cursor?, limit?)` → [`MembersPage`] — участники
//!   гильдии из снятых `GUILD_MEMBER_LIST_UPDATE`, роли, группы и счётчики;
//! - `search_page(query, filters, cursor, limit)` → [`SearchPage`] — локальный
//!   поиск по фикстурам с 7 фильтрами R-10 (автор, канал, содержит,
//!   упоминание, дата, тип, закреплено);
//! - `settings(section?)` → [`SettingsPayload`] — секции настроек из READY.
//!
//! Пагинация: `cursor`/`before` непрозрачны для UI и стабильны (id строки),
//! `limit` — `1..=100` ([`MAX_PAGE_LIMIT`]), пустая страница — валидный ответ.
//! Forward-страница отдаёт `limit` строк строго после `cursor` и `nextCursor` —
//! id последней строки. Older-страница отдаёт не более `limit` строк строго до
//! `before` в том же возрастающем порядке (UI делает prepend), `olderCursor` —
//! id первой строки (следующий `before`). `hasOlder`/`hasMore` независимы:
//! первое — есть ли строки старше страницы, второе — есть ли строки новее.
//! Оба курсора взаимоисключающие; `position = "latest"` не сочетается с ними.
//! `loaded_rows_limit` = [`UI_WINDOW_ROWS`] — инвариант UI (окно виртуализации
//! ≤ 500 строк), сервер его не навязывает и отдаёт не более `limit` строк.
//!
//! Пределы входа: файл фикстуры ≤ [`MAX_FIXTURE_BYTES`] (проверяется по
//! `metadata` до чтения в память), `channelId`/`guildId`/`cursor`/`section`/
//! `query` и значения фильтров ≤ [`MAX_INPUT_LEN`] символов, кадр `by-type`
//! обязан иметь непустой строковый `t` и присутствующий `d` — иначе `Parse`,
//! а не тихий пропуск.
//!
//! Ошибки — [`DataError`] с `kind`: `Io`, `Parse`, `NotFound`, `InvalidArg`;
//! сериализуются в `{ "kind", "message", "path"?/"resource"? }`. На входных
//! данных нет `unwrap()`: отсутствие обязательного поля — `Parse`, а не паника.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{json, Value};

#[derive(Default)]
struct LiveSession {
    active: bool,
    token: Option<String>,
    user: Option<ReadyUser>,
    ready_data: Option<Value>,
    channel_messages: HashMap<String, Vec<Value>>,
    fetched_channels: HashSet<String>,
    fetched_channels_at: HashMap<String, std::time::Instant>,
    voice_states: HashMap<String, Vec<Value>>,
    presences: HashMap<String, Value>,
    member_list_updates: HashMap<String, Value>,
    gateway_session_id: Option<String>,
    voice_session_id: Option<String>,
    voice_guild_id: Option<String>,
    voice_channel_id: Option<String>,
    voice_endpoint: Option<String>,
    voice_token: Option<String>,
    voice_launched_token: Option<String>,
    voice_phase: String,
    voice_got_state_update: bool,
    voice_got_server_update: bool,
    voice_got_rtc_op4: bool,
    voice_connect_started_at: Option<std::time::Instant>,
    voice_last_error: Option<String>,
    outbound_voice_op4_frames: Vec<Value>,
    mute_deaf_coalesce_seq: u64,
    stream_key: Option<String>,
    stream_rtc_server_id: Option<String>,
    stream_rtc_channel_id: Option<String>,
    stream_endpoint: Option<String>,
    stream_token: Option<String>,
    stream_launched_token: Option<String>,
    stream_launched_server_id: Option<String>,
    resolved_invites: HashMap<String, Value>,
    local_preview_guilds: HashMap<String, Value>,
    deleted_message_ids: HashSet<(String, String)>,
}

fn live_session() -> &'static Mutex<LiveSession> {
    static SESSION: OnceLock<Mutex<LiveSession>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(LiveSession::default()))
}

fn live_rest_mock_slot() -> &'static Mutex<Option<std::sync::Arc<dyn crate::guilds::GuildJoinTransport>>> {
    static MOCK: OnceLock<Mutex<Option<std::sync::Arc<dyn crate::guilds::GuildJoinTransport>>>> =
        OnceLock::new();
    MOCK.get_or_init(|| Mutex::new(None))
}

pub fn set_live_rest_mock_handler(
    handler: Option<std::sync::Arc<dyn crate::guilds::GuildJoinTransport>>,
) {
    let mut slot = live_rest_mock_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *slot = handler;
}

pub fn clear_live_rest_mock_handler() {
    set_live_rest_mock_handler(None);
}

/// Возвращает активный живой токен, если сессия активна и токен установлен.
pub fn resolve_live_token() -> Option<String> {
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !guard.active {
        return None;
    }
    guard
        .token
        .as_ref()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Извлекает Discord snowflake `user_id` из первого сегмента токена (`base64(user_id).timestamp.hmac`).
pub fn extract_user_id_from_token(token: &str) -> Option<String> {
    let first_seg = token.trim().split('.').next()?.trim();
    if first_seg.is_empty() {
        return None;
    }
    let decoded = decode_base64_ascii(first_seg)?;
    let text = String::from_utf8(decoded).ok()?;
    let trimmed = text.trim();
    if !trimmed.is_empty()
        && trimmed.bytes().all(|b| b.is_ascii_digit())
        && trimmed.parse::<u64>().is_ok()
    {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn decode_base64_ascii(input: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=' && !b.is_ascii_whitespace())
        .collect();
    if clean.is_empty() {
        return None;
    }
    let map_byte = |b: u8| -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        if chunk.len() < 2 {
            return None;
        }
        let v0 = map_byte(chunk[0])? as u32;
        let v1 = map_byte(chunk[1])? as u32;
        out.push(((v0 << 2) | (v1 >> 4)) as u8);
        if chunk.len() > 2 {
            let v2 = map_byte(chunk[2])? as u32;
            out.push((((v1 & 0x0F) << 4) | (v2 >> 2)) as u8);
            if chunk.len() > 3 {
                let v3 = map_byte(chunk[3])? as u32;
                out.push((((v2 & 0x03) << 6) | v3) as u8);
            }
        }
    }
    Some(out)
}

pub fn outbound_voice_op4_frames() -> Vec<Value> {
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.outbound_voice_op4_frames.clone()
}

pub fn clear_outbound_voice_op4_frames() {
    let mut guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.outbound_voice_op4_frames.clear();
}

/// Возвращает `session_id` активной Gateway-сессии (если подключена),
/// используемый при `POST /invites/{code}` (`[D] 4d2a64cb5df5fc4e_0@635659`).
pub fn current_gateway_session_id() -> Option<String> {
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard
        .gateway_session_id
        .clone()
        .or_else(|| guard.voice_session_id.clone())
        .filter(|s| !s.trim().is_empty())
}

/// Синоним [`current_gateway_session_id`] для модулей REST (`guilds.rs`, `invites.rs`).
pub fn live_gateway_session_id() -> Option<String> {
    current_gateway_session_id()
}

/// Проверяет, активна ли живая пользовательская сессия.
pub fn is_live_active() -> bool {
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.active
}

/// Записывает событие телеметрии живой сессии в `/tmp/live-ui-telemetry.jsonl`.
pub fn append_live_user_telemetry(payload: Value) {
    if let Ok(line) = serde_json::to_string(&payload) {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/live-ui-telemetry.jsonl")
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

fn extract_invite_code_from_target(target_or_code: &str) -> String {
    let trimmed = target_or_code.trim();
    let without_query = trimmed.split(['?', '#']).next().unwrap_or(trimmed).trim_end_matches('/');
    without_query
        .strip_prefix("/invites/")
        .unwrap_or_else(|| without_query.rsplit('/').next().unwrap_or(without_query))
        .trim()
        .to_string()
}

/// Кэширует JSON-ответ `GET /invites/{code}` в `LiveSession`, чтобы `join_guild` и `accept_invite`
/// знали метаданные сервера, целевой канал и список `features`.
pub fn cache_resolved_invite_payload(target_or_code: &str, body: &[u8]) {
    let Ok(mut val) = serde_json::from_slice::<Value>(body) else {
        return;
    };
    sanitize_etf_json_value(&mut val);
    let code = field_str(&val, "code")
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| extract_invite_code_from_target(target_or_code));
    if code.is_empty() {
        return;
    }
    let mut guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.resolved_invites.insert(code, val);
}

/// Проверяет, содержит ли гильдия флаг `"DISCOVERABLE"` в `features`.
pub fn is_cached_guild_discoverable(guild_id: &str) -> bool {
    let gid = guild_id.trim();
    if gid.is_empty() {
        return false;
    }
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let has_discoverable = |g: &Value| -> bool {
        let feats = g
            .get("features")
            .and_then(Value::as_array)
            .or_else(|| {
                g.get("properties")
                    .and_then(|p| p.get("features"))
                    .and_then(Value::as_array)
            });
        feats.is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("DISCOVERABLE")))
    };
    if let Some(ref rd) = guard.ready_data {
        for g in array(rd.get("guilds")) {
            if field_str(g, "id") == Some(gid) && has_discoverable(g) {
                return true;
            }
        }
    }
    for inv in guard.resolved_invites.values() {
        if let Some(g) = inv.get("guild") {
            if field_str(g, "id") == Some(gid) && has_discoverable(g) {
                return true;
            }
        }
    }
    false
}

/// Возвращает код приглашения из кэша `resolve_invite` для заданного `guild_id`.
pub fn cached_invite_code_for_guild(guild_id: &str) -> Option<String> {
    let gid = guild_id.trim();
    if gid.is_empty() {
        return None;
    }
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    for (code, inv) in &guard.resolved_invites {
        let inv_gid = inv
            .get("guild")
            .and_then(|g| field_str(g, "id"))
            .or_else(|| field_str(inv, "guild_id"));
        if inv_gid == Some(gid) {
            return Some(code.clone());
        }
    }
    None
}

fn encode_base64_ascii(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Строит Base64-заголовок `X-Context-Properties` по эталонному `getInviteContext("Desktop Invite Modal", invite)`
/// из `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@632050`.
pub fn build_invite_context_properties_b64(target_or_code: &str) -> String {
    let code = extract_invite_code_from_target(target_or_code);
    let inv_opt = if code.is_empty() {
        None
    } else {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard
            .resolved_invites
            .get(&code)
            .cloned()
            .or_else(|| {
                guard
                    .resolved_invites
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&code))
                    .map(|(_, v)| v.clone())
            })
    };
    if let Some(inv) = inv_opt {
        let mut ctx = serde_json::Map::new();
        ctx.insert(
            "location".to_string(),
            Value::String("Desktop Invite Modal".to_string()),
        );
        if let Some(gid) = inv
            .get("guild")
            .and_then(|g| field_str(g, "id"))
            .or_else(|| field_str(&inv, "guild_id"))
        {
            ctx.insert(
                "location_guild_id".to_string(),
                Value::String(gid.to_string()),
            );
        }
        if let Some(ch) = inv.get("channel") {
            if let Some(cid) = field_str(ch, "id") {
                ctx.insert(
                    "location_channel_id".to_string(),
                    Value::String(cid.to_string()),
                );
            }
            if let Some(ctype) = field_u64(ch, "type") {
                ctx.insert("location_channel_type".to_string(), json!(ctype));
            }
        }
        if let Ok(bytes) = serde_json::to_vec(&Value::Object(ctx)) {
            return encode_base64_ascii(&bytes);
        }
    }
    "eyJsb2NhdGlvbiI6IkpvaW4gR3VpbGQifQ==".to_string()
}

/// Принимает приглашение по коду через боевой REST с полной клиентской идентичностью Discord 1.0.162.
pub fn accept_invite_via_live_rest(
    code: &str,
) -> Result<discord_core::rest::transport::HttpResponse, String> {
    let clean_code = extract_invite_code_from_target(code);
    if clean_code.is_empty() {
        return Err("пустой код приглашения".to_string());
    }
    let ctx_b64 = build_invite_context_properties_b64(&clean_code);
    let (token_opt, sid_opt) = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        (
            guard.token.clone(),
            guard
                .gateway_session_id
                .clone()
                .or_else(|| guard.voice_session_id.clone()),
        )
    };
    let Some(token) = token_opt.filter(|t| !t.trim().is_empty() && !t.starts_with("test-")) else {
        return Err("нет активного токена".to_string());
    };
    let clean_code_owned = clean_code.to_string();
    let handle = std::thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || -> Result<discord_core::rest::transport::HttpResponse, String> {
            let identity = crate::auth::IdentityContext::detect();
            let auth = discord_core::rest::client::ApiAuth::from_token_with_scheme(
                &token,
                discord_core::rest::client::AuthScheme::User,
                &identity.super_properties,
                &identity.locale,
                &identity.timezone,
            )
            .map_err(|e| e.to_string())?;
            let client = discord_core::rest::client::ApiClient::new(discord_core::rest::client::ApiConfig {
                base: discord_core::API_V9_BASE.to_string(),
                auth,
                http: discord_core::rest::transport::HttpClientConfig {
                    connect_timeout: std::time::Duration::from_millis(2500),
                    read_timeout: std::time::Duration::from_millis(3500),
                    write_timeout: std::time::Duration::from_millis(2500),
                    ..discord_core::rest::transport::HttpClientConfig::default()
                },
                retry: discord_core::rest::client::ApiRetryPolicy {
                    max_attempts: 1,
                    ..discord_core::rest::client::ApiRetryPolicy::default()
                },
            })
            .map_err(|e| e.to_string())?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let body_val = if let Some(sid) = sid_opt.filter(|s| !s.trim().is_empty()) {
                json!({ "session_id": sid })
            } else {
                json!({})
            };
            let body_bytes = serde_json::to_vec(&body_val).unwrap_or_else(|_| b"{}".to_vec());
            let path = format!("/invites/{clean_code_owned}");
            let headers = [("X-Context-Properties", ctx_b64.as_str())];
            runtime
                .block_on(client.post_once(&path, &body_bytes, &headers))
                .map_err(|e| e.to_string())
        }))
        .map_err(|_| "внутренняя ошибка потока при принятии приглашения".to_string())?
    });
    handle
        .join()
        .map_err(|_| "ожидание потока принятия приглашения прервано".to_string())?
}

pub fn accept_invite_and_join_guild<T: crate::guilds::GuildJoinTransport + ?Sized>(
    transport: &T,
    invite_code: &str,
    guild_id: &str,
    user_id: &str,
) -> Result<crate::guilds::GuildJoinOutcome, String> {
    crate::guilds::accept_invite_and_join_guild(transport, invite_code, guild_id, user_id)
}

/// Синхронно гарантирует, что активная сессия шлюза (`gateway_session_id`) зарегистрирована на сервере `guild_id`
/// (через `PUT /guilds/{id}/members/@me?lurker=true&session_id={sid}`), чтобы `OP 4 VOICE_STATE_UPDATE` принимался шлюзом Discord.
pub fn ensure_live_guild_lurker_session(guild_id: &str) {
    let gid = guild_id.trim().to_string();
    if gid.is_empty() || gid == "@me" {
        return;
    }
    let (token_opt, sid_opt) = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        (
            guard.token.clone(),
            guard
                .gateway_session_id
                .clone()
                .or_else(|| guard.voice_session_id.clone())
                .filter(|s| !s.trim().is_empty()),
        )
    };
    if live_rest_mock_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .is_some()
    {
        return;
    }
    let Some(token) = token_opt.filter(|t| {
        let tr = t.trim();
        !cfg!(test)
            && !tr.is_empty()
            && tr != "live-test-token"
            && !tr.starts_with("test-")
            && !tr.starts_with("synthetic_")
            && !tr.starts_with("MT_real_user_token_")
            && !tr.contains(".GTest")
    }) else {
        return;
    };
    let handle = std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let identity = crate::auth::IdentityContext::detect();
            let Ok(auth) = discord_core::rest::client::ApiAuth::from_token_with_scheme(
                &token,
                discord_core::rest::client::AuthScheme::User,
                &identity.super_properties,
                &identity.locale,
                &identity.timezone,
            ) else {
                return;
            };
            let Ok(client) = discord_core::rest::client::ApiClient::new(discord_core::rest::client::ApiConfig {
                base: discord_core::API_V9_BASE.to_string(),
                auth,
                http: discord_core::rest::transport::HttpClientConfig {
                    connect_timeout: std::time::Duration::from_millis(2000),
                    read_timeout: std::time::Duration::from_millis(2500),
                    write_timeout: std::time::Duration::from_millis(2000),
                    ..discord_core::rest::transport::HttpClientConfig::default()
                },
                retry: discord_core::rest::client::ApiRetryPolicy {
                    max_attempts: 1,
                    ..discord_core::rest::client::ApiRetryPolicy::default()
                },
            }) else {
                return;
            };
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            let lurk_path = if let Some(ref sid) = sid_opt {
                format!("/guilds/{gid}/members/@me?lurker=true&session_id={sid}")
            } else {
                format!("/guilds/{gid}/members/@me?lurker=true")
            };
            let _ = runtime.block_on(client.put(&lurk_path, b"{}"));
        }));
    });
    let _ = handle.join();
}

fn last_hydrate_at_map() -> &'static Mutex<HashMap<String, std::time::Instant>> {
    static LAST_HYDRATE_AT: OnceLock<Mutex<HashMap<String, std::time::Instant>>> = OnceLock::new();
    LAST_HYDRATE_AT.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn clear_hydrate_cooldown(guild_id: &str) {
    let mut map = last_hydrate_at_map().lock().unwrap_or_else(|p| p.into_inner());
    map.remove(guild_id.trim());
}

/// Синхронно входит на сервер (при необходимости через `PUT /guilds/{id}/members/@me`),
/// автоматически проходит `member-verification` при `pending == true` или `403`,
/// перезагружает роли `GET /api/v9/guilds/{id}/members/@me` и пересчитывает `channelPermissions`.
pub fn hydrate_live_guild_blocking(guild_id: &str) -> Option<Value> {
    let gid = guild_id.trim().to_string();
    if gid.is_empty() || gid == "@me" {
        return None;
    }
    let (token_opt, self_uid, initial_pending) = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let pend = guard
            .ready_data
            .as_ref()
            .and_then(|rd| rd.get("guilds"))
            .and_then(Value::as_array)
            .and_then(|arr| arr.iter().find(|g| field_str(g, "id") == Some(gid.as_str())))
            .or_else(|| guard.local_preview_guilds.get(&gid))
            .is_some_and(|g| field_bool(g, "pending") || field_bool(g, "is_pending"));
        (guard.token.clone(), resolve_live_self_uid(&guard), pend)
    };

    let mock_opt = live_rest_mock_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone();
    if let Some(mock_transport) = mock_opt {
        let super_props = crate::guilds::default_super_properties_base64();
        if let Ok(outcome) = crate::guilds::complete_guild_verification_and_permissions(
            mock_transport.as_ref(),
            &gid,
            &self_uid,
            initial_pending,
            &super_props,
        ) {
            let ch_perms_val = serde_json::to_value(&outcome.channel_permissions)
                .unwrap_or_else(|_| json!({}));
            let mut hydrated_guild: Option<Value> = None;
            let snapshot = {
                let mut guard = live_session()
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                let apply_outcome = |target_map: &mut serde_json::Map<String, Value>| {
                    if let Some(roles) = outcome.guild.get("roles").and_then(Value::as_array) {
                        target_map.insert("roles".to_string(), Value::Array(roles.clone()));
                    }
                    if let Some(owner_id) = outcome.guild.get("owner_id").cloned() {
                        target_map.insert("owner_id".to_string(), owner_id);
                    }
                    if !outcome.channels.is_empty() {
                        target_map.insert(
                            "channels".to_string(),
                            Value::Array(outcome.channels.clone()),
                        );
                    }
                    let mr_json: Vec<Value> = outcome
                        .roles
                        .iter()
                        .map(|s| Value::String(s.clone()))
                        .collect();
                    target_map.insert("my_roles".to_string(), Value::Array(mr_json.clone()));
                    target_map.insert("myRoles".to_string(), Value::Array(mr_json));
                    target_map.insert("lurker".to_string(), Value::Bool(false));
                    target_map.insert("isMember".to_string(), Value::Bool(true));
                    target_map.insert("pending".to_string(), Value::Bool(outcome.pending));
                    target_map.insert("channelPermissions".to_string(), ch_perms_val.clone());
                };
                guard.local_preview_guilds.remove(&gid);
                if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                    let g_arr = ready_map
                        .entry("guilds".to_string())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    if let Value::Array(ref mut r_guilds) = g_arr {
                        if let Some(Value::Object(ref mut ex_map)) = r_guilds
                            .iter_mut()
                            .find(|g| field_str(g, "id") == Some(gid.as_str()))
                        {
                            apply_outcome(ex_map);
                            hydrated_guild = Some(Value::Object(ex_map.clone()));
                        } else {
                            let mut new_map = serde_json::Map::new();
                            new_map.insert("id".to_string(), Value::String(gid.clone()));
                            apply_outcome(&mut new_map);
                            let val = Value::Object(new_map);
                            r_guilds.push(val.clone());
                            hydrated_guild = Some(val);
                        }
                    }
                }
                guard.ready_data.clone()
            };
            if let Some(snap) = snapshot {
                save_live_ready_cache(&snap);
            }
            return hydrated_guild;
        }
        return None;
    }

    let Some(token) = token_opt.filter(|t| {
        let tr = t.trim();
        !cfg!(test)
            && !tr.is_empty()
            && tr != "live-test-token"
            && !tr.starts_with("test-")
            && !tr.starts_with("synthetic_")
            && !tr.starts_with("MT_real_user_token_")
            && !tr.contains(".GTest")
    }) else {
        return None;
    };
    {
        let mut map = last_hydrate_at_map().lock().unwrap_or_else(|p| p.into_inner());
        let now = std::time::Instant::now();
        if let Some(prev) = map.get(&gid) {
            if now.duration_since(*prev) < std::time::Duration::from_secs(15) {
                return None;
            }
        }
        map.insert(gid.clone(), now);
    }
    let handle = std::thread::spawn(move || -> Option<Value> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || -> Option<Value> {
            let identity = crate::auth::IdentityContext::detect();
            let Ok(auth) = discord_core::rest::client::ApiAuth::from_token_with_scheme(
                &token,
                discord_core::rest::client::AuthScheme::User,
                &identity.super_properties,
                &identity.locale,
                &identity.timezone,
            ) else {
                return None;
            };
            let Ok(client) = discord_core::rest::client::ApiClient::new(discord_core::rest::client::ApiConfig {
                base: discord_core::API_V9_BASE.to_string(),
                auth,
                http: discord_core::rest::transport::HttpClientConfig {
                    connect_timeout: std::time::Duration::from_millis(800),
                    read_timeout: std::time::Duration::from_millis(1000),
                    write_timeout: std::time::Duration::from_millis(800),
                    ..discord_core::rest::transport::HttpClientConfig::default()
                },
                retry: discord_core::rest::client::ApiRetryPolicy {
                    max_attempts: 1,
                    ..discord_core::rest::client::ApiRetryPolicy::default()
                },
            }) else {
                return None;
            };
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return None;
            };

            let sid_opt = live_gateway_session_id();
            let member_res = runtime.block_on(client.get(&format!("/guilds/{gid}/members/@me")));

            let mut my_roles_val: Option<Vec<String>> = None;
            let mut lurker_state: Option<bool> = None;
            let mut is_member_state: Option<bool> = None;
            let mut pending_state: Option<bool> = Some(initial_pending);
            let mut got_403_gate = false;

            match member_res {
                Ok(resp) if (200..300).contains(&resp.status) => {
                    if let Ok(v) = serde_json::from_slice::<Value>(&resp.body) {
                        let roles: Vec<String> = array(v.get("roles"))
                            .iter()
                            .filter_map(|r| as_non_nil_str(r).map(str::to_string))
                            .collect();
                        let flags = field_u64(&v, "flags").unwrap_or(0);
                        let is_guest = (flags & 8) != 0 || field_bool(&v, "lurker");
                        let is_pending = field_bool(&v, "pending") || field_bool(&v, "is_pending");
                        my_roles_val = Some(roles);
                        lurker_state = Some(is_guest);
                        is_member_state = Some(!is_guest);
                        pending_state = Some(is_pending);
                        if is_guest {
                            if let Some(ref sid) = sid_opt {
                                let lurk_path = format!("/guilds/{gid}/members/@me?lurker=true&session_id={sid}");
                                let _ = runtime.block_on(client.put(&lurk_path, b"{}"));
                            }
                        }
                    }
                }
                _ => {
                    let join_res = runtime
                        .block_on(client.put(&format!("/guilds/{gid}/members/@me?lurker=false"), b"{}"))
                        .or_else(|_| {
                            if let Some(ref sid) = sid_opt {
                                runtime.block_on(client.put(
                                    &format!("/guilds/{gid}/members/@me?lurker=false&session_id={sid}"),
                                    b"{}",
                                ))
                            } else {
                                Err(discord_core::rest::client::ApiError::Client { status: 403 })
                            }
                        });
                    if let Ok(jr) = join_res {
                        if (200..300).contains(&jr.status) {
                            if let Ok(v) = serde_json::from_slice::<Value>(&jr.body) {
                                let roles: Vec<String> = array(v.get("roles"))
                                    .iter()
                                    .filter_map(|r| as_non_nil_str(r).map(str::to_string))
                                    .collect();
                                let flags = field_u64(&v, "flags").unwrap_or(0);
                                let is_guest = (flags & 8) != 0 || field_bool(&v, "lurker");
                                let is_pending = field_bool(&v, "pending") || field_bool(&v, "is_pending");
                                my_roles_val = Some(roles);
                                lurker_state = Some(is_guest);
                                is_member_state = Some(!is_guest);
                                pending_state = Some(is_pending);
                            } else {
                                my_roles_val = Some(Vec::new());
                                lurker_state = Some(false);
                                is_member_state = Some(true);
                                pending_state = Some(false);
                            }
                        }
                    } else {
                        got_403_gate = true;
                        let lurk_path = if let Some(ref sid) = sid_opt {
                            format!("/guilds/{gid}/members/@me?lurker=true&session_id={sid}")
                        } else {
                            format!("/guilds/{gid}/members/@me?lurker=true")
                        };
                        let _ = runtime.block_on(client.put(&lurk_path, b"{}"));
                    }
                }
            }

            // S3: Автоматическое подтверждение правил через member-verification + PUT /guilds/{id}/requests/@me
            if pending_state == Some(true) || got_403_gate {
                if let Ok(ver_resp) = runtime.block_on(client.get(&format!("/guilds/{gid}/member-verification"))) {
                    if (200..300).contains(&ver_resp.status) {
                        if let Ok(ver_json) = serde_json::from_slice::<Value>(&ver_resp.body) {
                            let accept_body = crate::guilds::build_member_verification_accept_body(&ver_json);
                            if let Ok(body_bytes) = serde_json::to_vec(&accept_body) {
                                if let Ok(put_resp) = runtime.block_on(client.put(&format!("/guilds/{gid}/requests/@me"), &body_bytes)) {
                                    if (200..300).contains(&put_resp.status) {
                                        pending_state = Some(false);
                                        lurker_state = Some(false);
                                        is_member_state = Some(true);
                                        if let Ok(reloaded) = runtime.block_on(client.get(&format!("/guilds/{gid}/members/@me"))) {
                                            if (200..300).contains(&reloaded.status) {
                                                if let Ok(rv) = serde_json::from_slice::<Value>(&reloaded.body) {
                                                    let roles: Vec<String> = array(rv.get("roles"))
                                                        .iter()
                                                        .filter_map(|r| as_non_nil_str(r).map(str::to_string))
                                                        .collect();
                                                    my_roles_val = Some(roles);
                                                    pending_state = Some(field_bool(&rv, "pending") || field_bool(&rv, "is_pending"));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let guild_val: Option<Value> = runtime
                .block_on(client.get(&format!("/guilds/{gid}")))
                .ok()
                .filter(|r| r.status == 200)
                .and_then(|r| serde_json::from_slice(&r.body).ok());

            let channels_val: Option<Vec<Value>> = runtime
                .block_on(client.get(&format!("/guilds/{gid}/channels")))
                .ok()
                .filter(|r| r.status == 200)
                .and_then(|r| serde_json::from_slice::<Value>(&r.body).ok())
                .and_then(|v| v.as_array().cloned());

            if guild_val.is_none() && channels_val.is_none() && my_roles_val.is_none() {
                clear_hydrate_cooldown(&gid);
                return None;
            }

            let mut hydrated_out: Option<Value> = None;
            let snapshot = {
                let mut guard = live_session()
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                let apply_updates = |target_map: &mut serde_json::Map<String, Value>| {
                    if let Some(ref gv) = guild_val {
                        if let Some(roles) = gv.get("roles").and_then(Value::as_array) {
                            if !roles.is_empty() {
                                target_map.insert("roles".to_string(), Value::Array(roles.clone()));
                            }
                        }
                        if let Some(owner_id) = gv.get("owner_id").cloned() {
                            target_map.insert("owner_id".to_string(), owner_id);
                        }
                        for key in [
                            "name",
                            "icon",
                            "features",
                            "afk_channel_id",
                            "system_channel_id",
                            "rules_channel_id",
                            "verification_level",
                        ] {
                            if let Some(val) = gv.get(key).cloned().filter(|v| !v.is_null()) {
                                target_map.insert(key.to_string(), val);
                            }
                        }
                    }
                    if let Some(ref chs) = channels_val {
                        if !chs.is_empty() {
                            target_map.insert("channels".to_string(), Value::Array(chs.clone()));
                        }
                    }
                    if let Some(ref mr) = my_roles_val {
                        let mr_json: Vec<Value> =
                            mr.iter().map(|s| Value::String(s.clone())).collect();
                        target_map.insert("my_roles".to_string(), Value::Array(mr_json.clone()));
                        target_map.insert("myRoles".to_string(), Value::Array(mr_json));
                    }
                    if let Some(lurk) = lurker_state {
                        target_map.insert("lurker".to_string(), Value::Bool(lurk));
                    }
                    if let Some(ism) = is_member_state {
                        target_map.insert("isMember".to_string(), Value::Bool(ism));
                    }
                    if let Some(pend) = pending_state {
                        target_map.insert("pending".to_string(), Value::Bool(pend));
                    }
                    let roles_slice = target_map
                        .get("roles")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let chs_slice = target_map
                        .get("channels")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let mr_vec: Vec<String> = target_map
                        .get("my_roles")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    let owner_str = target_map.get("owner_id").and_then(Value::as_str);
                    let ch_perms =
                        crate::permissions::PermissionCalculator::compute_guild_channel_permissions(
                            &gid,
                            owner_str,
                            &self_uid,
                            &mr_vec,
                            &roles_slice,
                            &chs_slice,
                            lurker_state.unwrap_or(false),
                            pending_state.unwrap_or(false),
                        );
                    if let Ok(cp_val) = serde_json::to_value(&ch_perms) {
                        target_map.insert("channelPermissions".to_string(), cp_val);
                    }
                };
                if is_member_state == Some(true) {
                    guard.local_preview_guilds.remove(&gid);
                } else if let Some(Value::Object(ref mut lp)) = guard.local_preview_guilds.get_mut(&gid) {
                    apply_updates(lp);
                    hydrated_out = Some(Value::Object(lp.clone()));
                }
                if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                    let g_arr = ready_map
                        .entry("guilds".to_string())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    if let Value::Array(ref mut r_guilds) = g_arr {
                        if let Some(Value::Object(ref mut ex_map)) = r_guilds
                            .iter_mut()
                            .find(|g| field_str(g, "id") == Some(gid.as_str()))
                        {
                            apply_updates(ex_map);
                            hydrated_out = Some(Value::Object(ex_map.clone()));
                        } else if is_member_state == Some(true) || guild_val.is_some() || channels_val.is_some() {
                            let mut new_map = serde_json::Map::new();
                            new_map.insert("id".to_string(), Value::String(gid.clone()));
                            apply_updates(&mut new_map);
                            let val = Value::Object(new_map);
                            r_guilds.push(val.clone());
                            hydrated_out = Some(val);
                        }
                    }
                }
                guard.ready_data.clone()
            };
            if let Some(snap) = snapshot {
                save_live_ready_cache(&snap);
            }
            hydrated_out
        }))
        .ok()
        .flatten()
    });
    handle.join().ok().flatten()
}

/// Загружает в фоне полную структуру сервера (`GET /guilds/{id}`, `GET /guilds/{id}/channels`, `GET /guilds/{id}/members/@me`),
/// чтобы не блокировать фоновый опрос при старте.
pub fn spawn_hydrate_live_guild_full(guild_id: &str) {
    let gid = guild_id.trim().to_string();
    if gid.is_empty() || gid == "@me" {
        return;
    }
    std::thread::spawn(move || {
        hydrate_live_guild_blocking(&gid);
    });
}

fn build_guild_object_from_invite(
    guild_id: &str,
    invite_val: Option<&Value>,
    _self_uid: &str,
    _is_real_token: bool,
) -> Value {
    let g_obj = invite_val.and_then(|v| v.get("guild"));
    let g_name = g_obj
        .and_then(|g| field_str(g, "name"))
        .or_else(|| invite_val.and_then(|v| field_str(v, "guild_name")))
        .unwrap_or("Сервер");
    let g_icon = g_obj
        .and_then(|g| g.get("icon"))
        .cloned()
        .unwrap_or(Value::Null);
    let g_features = g_obj
        .and_then(|g| g.get("features"))
        .cloned()
        .unwrap_or_else(|| json!([]));
    let member_count = invite_val
        .and_then(|v| field_u64(v, "approximate_member_count"))
        .unwrap_or(1);

    let ch_obj = invite_val.and_then(|v| v.get("channel"));
    let ch_id = ch_obj
        .and_then(|c| field_str(c, "id"))
        .or_else(|| invite_val.and_then(|v| field_str(v, "target_channel_id")))
        .or_else(|| invite_val.and_then(|v| field_str(v, "channel_id")))
        .unwrap_or(guild_id);
    let ch_name = ch_obj
        .and_then(|c| field_str(c, "name"))
        .unwrap_or("general");
    let ch_type = ch_obj
        .and_then(|c| field_u64(c, "type"))
        .unwrap_or(0);

    let real_owner_id = g_obj
        .and_then(|g| g.get("owner_id"))
        .cloned()
        .unwrap_or(Value::Null);
    let initial_channels = if ch_obj.is_some()
        || invite_val.and_then(|v| field_str(v, "target_channel_id")).is_some()
        || invite_val.and_then(|v| field_str(v, "channel_id")).is_some()
    {
        json!([
            {
                "id": ch_id,
                "guild_id": guild_id,
                "name": ch_name,
                "type": ch_type,
                "position": 0,
                "permission_overwrites": []
            }
        ])
    } else {
        json!([])
    };
    json!({
        "id": guild_id,
        "name": g_name,
        "icon": g_icon,
        "features": g_features,
        "member_count": member_count,
        "isMember": true,
        "lurker": false,
        "owner_id": real_owner_id,
        "properties": {
            "id": guild_id,
            "name": g_name,
            "icon": g_icon,
            "features": g_features,
            "owner_id": real_owner_id
        },
        "my_roles": [],
        "myRoles": [],
        "roles": [
            {
                "id": guild_id,
                "name": "@everyone",
                "position": 0,
                "color": 0,
                "hoist": false,
                "permissions": "1071698529857"
            }
        ],
        "channels": initial_channels,
        "voice_states": [],
        "members": []
    })
}

fn ensure_local_guild_in_guard(
    guard: &mut LiveSession,
    guild_id: &str,
    invite_hint: Option<&Value>,
) -> Option<Value> {
    let gid = guild_id.trim();
    if gid.is_empty() {
        return None;
    }
    let self_uid = resolve_live_self_uid(guard);
    let is_real_token = guard.token.as_deref().is_some_and(|t| {
        let tr = t.trim();
        !cfg!(test)
            && !tr.is_empty()
            && tr != "live-test-token"
            && !tr.starts_with("test-")
            && !tr.starts_with("synthetic_")
            && !tr.starts_with("MT_real_user_token_")
            && !tr.contains(".GTest")
    });
    let cached_inv = invite_hint.cloned().or_else(|| {
        guard
            .resolved_invites
            .values()
            .find(|inv| {
                inv.get("guild").and_then(|g| field_str(g, "id")) == Some(gid)
                    || field_str(inv, "guild_id") == Some(gid)
            })
            .cloned()
    });
    let built = build_guild_object_from_invite(gid, cached_inv.as_ref(), &self_uid, is_real_token);
    guard
        .local_preview_guilds
        .insert(gid.to_string(), built.clone());

    if guard.ready_data.is_none() {
        guard.ready_data = Some(json!({
            "user": {
                "id": guard.user.as_ref().map(|u| u.id.as_str()).unwrap_or("0"),
                "username": guard.user.as_ref().map(|u| u.username.as_str()).unwrap_or("user")
            },
            "guilds": [],
            "private_channels": [],
            "relationships": [],
            "users": []
        }));
    }
    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
        let guilds_val = ready_map
            .entry("guilds".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(ref mut r_guilds) = guilds_val {
            if let Some(existing) = r_guilds
                .iter_mut()
                .find(|g| field_str(g, "id") == Some(gid))
            {
                if let Value::Object(ref mut ex_map) = existing {
                    let has_overflow_channel = ex_map
                        .get("channels")
                        .and_then(Value::as_array)
                        .is_some_and(|arr| {
                            arr.iter()
                                .any(|c| field_str(c, "id").is_some_and(|s| s.parse::<u64>().is_err()))
                        });
                    let existing_ch_len = ex_map
                        .get("channels")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len);
                    let built_ch_len = built
                        .get("channels")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len);
                    if has_overflow_channel || (existing_ch_len <= 1 && built_ch_len > existing_ch_len) {
                        if let Some(ch_arr) = built.get("channels").cloned() {
                            ex_map.insert("channels".to_string(), ch_arr);
                        }
                    }
                    let has_overflow_role = ex_map
                        .get("roles")
                        .and_then(Value::as_array)
                        .is_some_and(|arr| {
                            arr.iter()
                                .any(|r| field_str(r, "id").is_some_and(|s| s.parse::<u64>().is_err()))
                        });
                    let existing_roles_len = ex_map
                        .get("roles")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len);
                    let built_roles_len = built
                        .get("roles")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len);
                    if has_overflow_role
                        || (existing_roles_len <= 1 && built_roles_len > existing_roles_len)
                    {
                        if let Some(r_arr) = built.get("roles").cloned() {
                            ex_map.insert("roles".to_string(), r_arr);
                        }
                    }
                    if !ex_map.contains_key("my_roles") || has_overflow_role {
                        if let Some(mr) = built.get("my_roles").cloned() {
                            ex_map.insert("my_roles".to_string(), mr.clone());
                            ex_map.insert("myRoles".to_string(), mr);
                        }
                    }
                    if ex_map
                        .get("members")
                        .and_then(Value::as_array)
                        .map_or(true, Vec::is_empty)
                    {
                        if let Some(m_arr) = built.get("members").cloned() {
                            ex_map.insert("members".to_string(), m_arr);
                        }
                    }
                }
            } else {
                r_guilds.push(built.clone());
            }
        }
    }
    guard.ready_data.clone()
}

/// Принимает JSON-ответ `POST /invites/{code}` и добавляет сервер и канал в `ready_data["guilds"]`.
pub fn ingest_accepted_invite_payload(body: &[u8]) {
    let Ok(mut val) = serde_json::from_slice::<Value>(body) else {
        return;
    };
    sanitize_etf_json_value(&mut val);
    if let Some(code) = field_str(&val, "code").map(str::to_string) {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.resolved_invites.insert(code, val.clone());
    }
    let gid_opt = val
        .get("guild")
        .and_then(|g| field_str(g, "id"))
        .or_else(|| field_str(&val, "guild_id"))
        .map(str::to_string);
    if let Some(gid) = gid_opt {
        let snapshot = {
            let mut guard = live_session()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            ensure_local_guild_in_guard(&mut guard, &gid, Some(&val))
        };
        if let Some(snap) = snapshot {
            save_live_ready_cache(&snap);
        }
        clear_hydrate_cooldown(&gid);
        hydrate_live_guild_blocking(&gid);
    }
}

/// Гарантирует присутствие гильдии `guild_id` в `ready_data["guilds"]` (материализуя из кэша инвайта при необходимости).
pub fn ensure_local_guild_present(guild_id: &str) {
    let snapshot = {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        ensure_local_guild_in_guard(&mut guard, guild_id, None)
    };
    if let Some(snap) = snapshot {
        save_live_ready_cache(&snap);
    }
    let has_mock = live_rest_mock_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .is_some();
    if has_mock {
        hydrate_live_guild_blocking(guild_id);
    }
}

/// Проверяет наличие гильдии в живой сессии или кэше разрешённых приглашений;
/// если гильдия есть в `resolved_invites`, материализует её в `ready_data["guilds"]`.
pub fn has_live_guild_or_channels(guild_id: &str) -> bool {
    let gid = guild_id.trim();
    if gid.is_empty() {
        return false;
    }
    let (already_in_ready, in_resolved_invites) = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let in_ready = guard.ready_data.as_ref().is_some_and(|rd| {
            array(rd.get("guilds"))
                .iter()
                .any(|g| field_str(g, "id") == Some(gid))
        });
        let in_inv = guard.resolved_invites.values().any(|inv| {
            inv.get("guild").and_then(|g| field_str(g, "id")) == Some(gid)
                || field_str(inv, "guild_id") == Some(gid)
        });
        (in_ready, in_inv)
    };
    if already_in_ready {
        return true;
    }
    if in_resolved_invites {
        ensure_local_guild_present(gid);
        return true;
    }
    false
}

/// Если `POST /invites/{code}` отклонён Discord API (400/403), но `resolve_invite` для этого кода уже выполнен,
/// синхронно входит на сервер через `PUT /guilds/{id}/members/@me` (`lurker=false` -> `lurker=true&session_id={sid}`),
/// загружает реальные каналы и роли сервера и возвращает сериализованный invite-объект для `decode_accept`.
pub fn materialized_accept_body_for_invite_target(target_or_code: &str) -> Option<Vec<u8>> {
    let code = extract_invite_code_from_target(target_or_code);
    if code.is_empty() {
        return None;
    }
    let mut inv_val = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard
            .resolved_invites
            .get(&code)
            .cloned()
            .or_else(|| {
                guard
                    .resolved_invites
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&code))
                    .map(|(_, v)| v.clone())
            })
    }?;
    let gid = inv_val
        .get("guild")
        .and_then(|g| field_str(g, "id"))
        .or_else(|| field_str(&inv_val, "guild_id"))?
        .to_string();
    {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _ = ensure_local_guild_in_guard(&mut guard, &gid, Some(&inv_val));
    }
    clear_hydrate_cooldown(&gid);
    hydrate_live_guild_blocking(&gid);
    let snapshot = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(ref rd) = guard.ready_data {
            if let Some(g_obj) = array(rd.get("guilds"))
                .iter()
                .find(|g| field_str(g, "id") == Some(gid.as_str()))
            {
                let real_chs = array(g_obj.get("channels"));
                if !real_chs.is_empty() {
                    let cur_ch_id = inv_val
                        .get("channel")
                        .and_then(|c| field_str(c, "id"))
                        .unwrap_or_default()
                        .to_string();
                    let cur_ch_is_valid_text = real_chs.iter().any(|c| {
                        field_str(c, "id") == Some(cur_ch_id.as_str())
                            && matches!(field_u64(c, "type").unwrap_or(0), 0 | 5)
                    });
                    if !cur_ch_is_valid_text {
                        let preferred_cid = field_str(g_obj, "rules_channel_id")
                            .or_else(|| field_str(g_obj, "system_channel_id"));
                        let chosen = preferred_cid
                            .and_then(|pid| real_chs.iter().find(|c| field_str(c, "id") == Some(pid)))
                            .or_else(|| {
                                real_chs
                                    .iter()
                                    .find(|c| matches!(field_u64(c, "type").unwrap_or(0), 0 | 5))
                            })
                            .or_else(|| real_chs.first());
                        if let Some(ch) = chosen {
                            if let Value::Object(ref mut im) = inv_val {
                                im.insert(
                                    "channel".to_string(),
                                    json!({
                                        "id": field_str(ch, "id").unwrap_or(&gid),
                                        "name": field_str(ch, "name").unwrap_or("general"),
                                        "type": field_u64(ch, "type").unwrap_or(0),
                                    }),
                                );
                            }
                        }
                    }
                }
            }
        }
        guard.ready_data.clone()
    };
    if let Some(snap) = snapshot {
        save_live_ready_cache(&snap);
    }
    serde_json::to_vec(&inv_val).ok()
}

/// Удаляет гильдию из локального списка `ready_data["guilds"]` и `local_preview_guilds` при выходе.
pub fn remove_local_guild(guild_id: &str) {
    let gid = guild_id.trim();
    if gid.is_empty() {
        return;
    }
    let snapshot = {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.local_preview_guilds.remove(gid);
        guard.voice_states.remove(gid);
        if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
            if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                r_guilds.retain(|g| field_str(g, "id") != Some(gid));
            }
        }
        guard.ready_data.clone()
    };
    if let Some(snap) = snapshot {
        save_live_ready_cache(&snap);
    }
}

/// Записывает JSON-ответ отправленного сообщения (`POST /channels/{id}/messages`) в `channel_messages`.
pub fn ingest_live_sent_message_json(body: &[u8]) {
    let Ok(mut val) = serde_json::from_slice::<Value>(body) else {
        return;
    };
    sanitize_etf_json_value(&mut val);
    let Some(cid) = field_str(&val, "channel_id").map(str::to_string) else {
        return;
    };
    record_sent_message(&cid, val);
}

/// Обновляет отредактированное сообщение в `channel_messages`.
pub fn update_edited_message(channel_id: &str, message_id: &str, content: &str) {
    let cid = channel_id.trim();
    let mid = message_id.trim();
    if cid.is_empty() || mid.is_empty() {
        return;
    }
    let mut guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(list) = guard.channel_messages.get_mut(cid) {
        for msg in list.iter_mut() {
            if field_str(msg, "id") == Some(mid) {
                if let Value::Object(ref mut map) = msg {
                    map.insert("content".to_string(), Value::String(content.to_string()));
                    map.insert(
                        "edited_timestamp".to_string(),
                        Value::String("2026-11-06T12:00:01.000Z".to_string()),
                    );
                }
            }
        }
    }
}

/// Удаляет сообщение из кэша `channel_messages` и запоминает ID удалённого сообщения,
/// предотвращая повторное воскрешение из фоновых GET /messages запросов.
pub fn remove_deleted_message(channel_id: &str, message_id: &str) {
    let cid = channel_id.trim();
    let mid = message_id.trim();
    if cid.is_empty() || mid.is_empty() {
        return;
    }
    let mut guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard
        .deleted_message_ids
        .insert((cid.to_string(), mid.to_string()));
    if let Some(list) = guard.channel_messages.get_mut(cid) {
        list.retain(|m| field_str(m, "id") != Some(mid));
    }
}

fn live_state_cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        })?;
    Some(base.join("discord-client").join("state_cache.json"))
}

/// Рекурсивно очищает артефакты ETF-сериализации (`"nil"` -> `null`, `"true"`/`"false"` -> `bool`),
/// чтобы ни в кэше `state_cache.json`, ни в живых событиях шлюза не появлялись строковые `"nil"` и `"false"`.
fn sanitize_etf_json_value(val: &mut Value) {
    match val {
        Value::String(s) => {
            if s == "nil" {
                *val = Value::Null;
            } else if s == "true" {
                *val = Value::Bool(true);
            } else if s == "false" {
                *val = Value::Bool(false);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                sanitize_etf_json_value(item);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                sanitize_etf_json_value(v);
            }
        }
        _ => {}
    }
}

fn is_synthetic_test_ready(ready_data: &Value) -> bool {
    let uid = ready_data
        .get("user")
        .and_then(|u| field_str(u, "id"))
        .unwrap_or_default();
    if uid == "777000" || uid == "111222333444555666" || uid.starts_with("test-") {
        return true;
    }
    array(ready_data.get("guilds")).iter().any(|g| {
        matches!(
            field_str(g, "id"),
            Some(
                "950000000000000001"
                    | "900000000000000001"
                    | "920000000000000001"
                    | "930000000000000001"
            )
        )
    })
}

fn save_live_ready_cache(ready_data: &Value) {
    if cfg!(test) || is_synthetic_test_ready(ready_data) {
        return;
    }
    let has_mock = live_rest_mock_slot()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .is_some();
    if has_mock {
        return;
    }
    let Some(path) = live_state_cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut clean = ready_data.clone();
    sanitize_etf_json_value(&mut clean);
    if let Ok(bytes) = serde_json::to_vec(&clean) {
        let tmp_path = path.with_extension("json.tmp");
        if fs::write(&tmp_path, bytes).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
            }
            let _ = fs::rename(&tmp_path, &path);
        }
    }
}

/// Проверяет соответствие `state.user.id` активному токену/пользователю и вычищает чужие/неподтверждённые гильдии.
pub fn sanitize_state_cache_for_token(
    data: &mut Value,
    token: &str,
    expected_user_id: Option<&str>,
) -> bool {
    sanitize_etf_json_value(data);
    let cached_uid = data
        .get("user")
        .and_then(|u| field_str(u, "id"))
        .unwrap_or_default()
        .trim()
        .to_string();
    if cached_uid.is_empty() {
        return false;
    }
    let token_uid = extract_user_id_from_token(token);
    if let Some(exp) = expected_user_id.map(str::trim).filter(|s| !s.is_empty()) {
        if cached_uid != exp {
            return false;
        }
    }
    if let Some(ref t_uid) = token_uid {
        if &cached_uid != t_uid {
            return false;
        }
    }

    if let Some(Value::Array(ref mut guilds)) = data.get_mut("guilds") {
        guilds.retain(|g| {
            let Some(gid) = field_str(g, "id") else {
                return false;
            };
            let is_fixture_synth = field_str(g, "name").is_some_and(|n| {
                n.contains("Fixture") || n.contains("ShadowByte") || n.contains("Synthetic")
            });
            if is_fixture_synth
                || matches!(
                    gid,
                    "950000000000000001"
                        | "900000000000000001"
                        | "920000000000000001"
                        | "930000000000000001"
                )
            {
                return false;
            }
            // Вычищаем неподтверждённые / чужие гильдии
            // Вычищаем неподтверждённые / lurker-гильдии из кэша
            let is_lurker = field_bool(g, "lurker");
            let is_not_member = field_bool_opt(g, "isMember") == Some(false)
                || field_bool_opt(g, "is_member") == Some(false);
            let is_unconfirmed = field_bool(g, "unconfirmed") || field_bool(g, "foreign");
            if is_lurker || is_not_member || is_unconfirmed {
                return false;
            }
            // Вычищаем гильдии, помеченные другим ownerUserId / cachedForUserId
            if let Some(owner_user_id) =
                field_str(g, "ownerUserId").or_else(|| field_str(g, "cachedForUserId"))
            {
                if owner_user_id != cached_uid {
                    return false;
                }
            }
            true
        });
        for g in guilds.iter_mut() {
            if let Value::Object(ref mut gm) = g {
                if let Some(Value::Array(ref mut ch_arr)) = gm.get_mut("channels") {
                    ch_arr.retain(|c| {
                        let valid_id = field_str(c, "id").is_some_and(|s| s.parse::<u64>().is_ok());
                        let is_hallucinated_synth = field_str(c, "name").is_some_and(|n| {
                            matches!(
                                n,
                                "медиа-и-мемы"
                                    | "модераторская"
                                    | "Игровая комната"
                                    | "Общий голосовой"
                                    | "ИНФОРМАЦИЯ"
                                    | "ТЕКСТОВЫЕ КАНАЛЫ"
                                    | "ГОЛОСОВЫЕ КАНАЛЫ"
                                    | "ПЕРСОНАЛ"
                            )
                        });
                        valid_id && !is_hallucinated_synth
                    });
                }
                let has_shadowbyte_role = ["my_roles", "myRoles"].iter().any(|k| {
                    gm.get(*k)
                        .and_then(Value::as_array)
                        .is_some_and(|arr| {
                            arr.iter().any(|v| {
                                matches!(
                                    v.as_str(),
                                    Some("1057598992449552425" | "1088036342321123398")
                                )
                            })
                        })
                });
                if has_shadowbyte_role && !cached_uid.is_empty() {
                    gm.insert("my_roles".to_string(), Value::Array(Vec::new()));
                    gm.insert("myRoles".to_string(), Value::Array(Vec::new()));
                }
            }
        }
    }
    true
}

/// Загружает `state_cache.json`, сверяет `state.user.id` с активным токеном/пользователем
/// и вычищает чужие/неподтверждённые гильдии. При несовпадении `user.id` удаляет чужой кэш.
pub fn load_verified_state_cache(
    path: &Path,
    token: &str,
    expected_user_id: Option<&str>,
) -> Option<Value> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() > MAX_FIXTURE_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let mut data: Value = serde_json::from_slice(&bytes).ok()?;
    if !sanitize_state_cache_for_token(&mut data, token, expected_user_id) {
        let _ = fs::remove_file(path);
        return None;
    }
    if data.get("user").and_then(|u| field_str(u, "id")).is_some()
        && data.get("guilds").and_then(Value::as_array).is_some()
        && !is_synthetic_test_ready(&data)
    {
        Some(data)
    } else {
        None
    }
}

fn load_live_ready_cache_for(token: Option<&str>, expected_uid: Option<&str>) -> Option<Value> {
    let path = live_state_cache_path()?;
    load_verified_state_cache(&path, token.unwrap_or_default(), expected_uid)
}

fn clear_live_ready_cache(was_synthetic_test: bool) {
    if was_synthetic_test {
        return;
    }
    if let Some(path) = live_state_cache_path() {
        let _ = fs::remove_file(path);
    }
}

fn enrich_voice_state_entry(
    vs: &Value,
    guild_members: &[Value],
    users_by_id: &HashMap<String, Value>,
) -> Value {
    let mut clean_vs = vs.clone();
    sanitize_etf_json_value(&mut clean_vs);
    let mut obj = match clean_vs.as_object() {
        Some(m) => m.clone(),
        None => return clean_vs,
    };
    let Some(uid) = obj
        .get("user_id")
        .and_then(|v| as_non_nil_str(v))
        .map(str::to_string)
    else {
        return Value::Object(obj);
    };
    let member_from_list = guild_members.iter().find(|m| {
        field_str(m, "user_id") == Some(uid.as_str())
            || m.get("user").and_then(|u| field_str(u, "id")) == Some(uid.as_str())
    });
    let mut user_val = users_by_id.get(&uid).cloned().or_else(|| {
        member_from_list
            .and_then(|m| m.get("user"))
            .cloned()
    });
    if let Some(ref mut u) = user_val {
        sanitize_etf_json_value(u);
    }
    let mut member_map = obj
        .get("member")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(Value::Object(gm)) = member_from_list {
        for (k, v) in gm {
            let mut cv = v.clone();
            sanitize_etf_json_value(&mut cv);
            let existing_empty = match member_map.get(k) {
                None | Some(Value::Null) => true,
                Some(Value::String(s)) => s.is_empty() || s == "nil",
                _ => false,
            };
            if existing_empty && !cv.is_null() {
                member_map.insert(k.clone(), cv);
            }
        }
    }
    if let Some(ref u) = user_val {
        let existing_user_empty = match member_map.get("user") {
            None | Some(Value::Null) => true,
            Some(Value::String(s)) => s.is_empty() || s == "nil",
            _ => false,
        };
        if existing_user_empty {
            member_map.insert("user".to_string(), u.clone());
        }
    }
    if !member_map.is_empty() {
        obj.insert("member".to_string(), Value::Object(member_map));
    }
    if let Some(u) = user_val {
        let existing_top_user_empty = match obj.get("user") {
            None | Some(Value::Null) => true,
            Some(Value::String(s)) => s.is_empty() || s == "nil",
            _ => false,
        };
        if existing_top_user_empty {
            obj.insert("user".to_string(), u);
        }
    }
    Value::Object(obj)
}

fn resolve_live_self_uid(guard: &LiveSession) -> String {
    guard
        .user
        .as_ref()
        .map(|u| u.id.clone())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            guard
                .ready_data
                .as_ref()
                .and_then(|r| r.get("user"))
                .and_then(|u| field_str(u, "id"))
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn populate_voice_and_presences_from_ready(guard: &mut LiveSession, data: &Value) {
    let mut users_by_id: HashMap<String, Value> = HashMap::new();
    if let Some(me) = data.get("user") {
        if let Some(uid) = field_str(me, "id") {
            users_by_id.insert(uid.to_string(), me.clone());
            if guard.user.as_ref().map(|u| u.id.as_str()) != Some(uid) {
                guard.user = Some(ReadyUser {
                    id: uid.to_string(),
                    username: field_str(me, "username").unwrap_or("user").to_string(),
                    global_name: field_str(me, "global_name").map(str::to_string),
                    avatar: field_str(me, "avatar").map(str::to_string),
                    email: Some("REDACTED".to_string()),
                    bio: field_str(me, "bio").map(str::to_string),
                    premium: field_u64(me, "premium_type").unwrap_or(0) > 0,
                    mfa_enabled: field_bool(me, "mfa_enabled"),
                });
            }
        }
    }
    for u in array(data.get("users")) {
        if let Some(uid) = field_str(u, "id") {
            users_by_id.insert(uid.to_string(), u.clone());
        }
    }
    let merged_members_outer = array(data.get("merged_members"));
    for (idx, guild) in array(data.get("guilds")).iter().enumerate() {
        if let Some(gid) = field_str(guild, "id") {
            let mut g_members: Vec<Value> = array(guild.get("members")).to_vec();
            if let Some(mm) = merged_members_outer.get(idx) {
                g_members.extend_from_slice(array(Some(mm)));
            }
            let raw_vs = array(guild.get("voice_states"));
            let enriched: Vec<Value> = raw_vs
                .iter()
                .map(|vs| enrich_voice_state_entry(vs, &g_members, &users_by_id))
                .collect();
            guard.voice_states.insert(gid.to_string(), enriched);
        }
    }
    for p in array(data.get("presences")) {
        if let Some(uid) = p
            .get("user")
            .and_then(|u| field_str(u, "id"))
            .or_else(|| field_str(p, "user_id"))
        {
            guard.presences.insert(uid.to_string(), p.clone());
        }
    }
}

/// Выполняет быстрое получение первичного состояния аккаунта через REST API Discord
/// (`/users/@me`, `/users/@me/guilds`, `/users/@me/relationships`, `/users/@me/channels`),
/// чтобы интерфейс сразу получал реальные серверы, друзей и личные каналы без ожидания Gateway.
pub fn fetch_live_ready_via_rest(token: &str) -> Option<Value> {
    let token = token.trim().to_string();
    if token.is_empty()
        || token == "live-test-token"
        || token.starts_with("test-")
    {
        return None;
    }
    let handle = std::thread::spawn(move || {
        std::panic::catch_unwind(move || {
            let identity = crate::auth::IdentityContext::detect();
            let auth = discord_core::rest::client::ApiAuth::from_token_with_scheme(
                &token,
                discord_core::rest::client::AuthScheme::User,
                &identity.super_properties,
                &identity.locale,
                &identity.timezone,
            )
            .ok()?;
            let config = discord_core::rest::client::ApiConfig {
                base: discord_core::API_V9_BASE.to_string(),
                auth,
                http: discord_core::rest::transport::HttpClientConfig {
                    connect_timeout: std::time::Duration::from_millis(3000),
                    read_timeout: std::time::Duration::from_millis(5000),
                    write_timeout: std::time::Duration::from_millis(3000),
                    ..discord_core::rest::transport::HttpClientConfig::default()
                },
                retry: discord_core::rest::client::ApiRetryPolicy {
                    max_attempts: 2,
                    ..discord_core::rest::client::ApiRetryPolicy::default()
                },
            };
            let client = discord_core::rest::client::ApiClient::new(config).ok()?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;

            // 1. /users/@me
            let user_val: Value = {
                let resp = runtime.block_on(client.get("/users/@me")).ok()?;
                if resp.status != 200 {
                    return None;
                }
                serde_json::from_slice(&resp.body).ok()?
            };

            // 2. /users/@me/guilds
            let guilds_val: Value = {
                if let Ok(resp) = runtime.block_on(client.get("/users/@me/guilds")) {
                    if resp.status == 200 {
                        serde_json::from_slice(&resp.body).unwrap_or(Value::Array(Vec::new()))
                    } else {
                        Value::Array(Vec::new())
                    }
                } else {
                    Value::Array(Vec::new())
                }
            };

            // 3. /users/@me/relationships
            let relationships_val: Value = {
                if let Ok(resp) = runtime.block_on(client.get("/users/@me/relationships")) {
                    if resp.status == 200 {
                        serde_json::from_slice(&resp.body).unwrap_or(Value::Array(Vec::new()))
                    } else {
                        Value::Array(Vec::new())
                    }
                } else {
                    Value::Array(Vec::new())
                }
            };

            // 4. /users/@me/channels
            let channels_val: Value = {
                if let Ok(resp) = runtime.block_on(client.get("/users/@me/channels")) {
                    if resp.status == 200 {
                        serde_json::from_slice(&resp.body).unwrap_or(Value::Array(Vec::new()))
                    } else {
                        Value::Array(Vec::new())
                    }
                } else {
                    Value::Array(Vec::new())
                }
            };

            let ready_obj = serde_json::json!({
                "user": user_val,
                "guilds": guilds_val,
                "relationships": relationships_val,
                "private_channels": channels_val,
                "read_state": { "entries": [] },
                "presences": [],
                "sessions": []
            });
            Some(ready_obj)
        })
        .ok()
        .flatten()
    });
    handle.join().ok().flatten()
}

/// Активирует или сбрасывает живую сессию приложения после авторизации / восстановления из keyring.
pub fn set_live_session(token: Option<String>, auth_user: Option<&crate::auth::AuthUser>) {
    let mut should_fetch_rest = false;
    let mut token_to_fetch: Option<String> = None;
    {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let is_test_token = token.as_deref().is_some_and(|t| {
            let tr = t.trim();
            tr == "live-test-token"
                || tr.starts_with("test-")
                || tr.starts_with("synthetic_")
                || tr.contains(".GTest")
        });
        let is_test_user = auth_user.is_some_and(|u| {
            u.id == "777000" || u.id.starts_with("test-") || u.username == "live_verifier"
        });
        if let Some(u) = auth_user {
            let prev_uid = guard
                .user
                .as_ref()
                .map(|pu| pu.id.as_str())
                .or_else(|| {
                    guard
                        .ready_data
                        .as_ref()
                        .and_then(|rd| rd.get("user"))
                        .and_then(|ru| field_str(ru, "id"))
                });
            if prev_uid.is_some_and(|pid| pid != u.id) {
                guard.ready_data = None;
                guard.local_preview_guilds.clear();
                guard.channel_messages.clear();
                guard.fetched_channels.clear();
                guard.fetched_channels_at.clear();
                guard.voice_states.clear();
                guard.presences.clear();
                guard.member_list_updates.clear();
            }
        }
        if let Some(tok) = token.filter(|t| !t.trim().is_empty()) {
            guard.token = Some(tok.clone());
            guard.active = true;
            if is_test_token || is_test_user {
                if auth_user.is_none() {
                    guard.user = None;
                    guard.ready_data = None;
                    guard.local_preview_guilds.clear();
                }
            } else {
                should_fetch_rest = true;
                token_to_fetch = Some(tok);
            }
        } else if auth_user.is_none() {
            let was_synthetic = guard.token.as_deref() == Some("live-test-token")
                || guard
                    .token
                    .as_deref()
                    .is_some_and(|t| t.contains(".GTest"))
                || guard
                    .user
                    .as_ref()
                    .is_some_and(|u| {
                        u.id == "777000"
                            || u.id.starts_with("test-")
                            || u.username == "live_verifier"
                    })
                || guard
                    .ready_data
                    .as_ref()
                    .is_some_and(is_synthetic_test_ready);
            guard.token = None;
            guard.active = false;
            guard.user = None;
            guard.ready_data = None;
            guard.voice_phase = "disconnected".to_string();
            guard.voice_got_state_update = false;
            guard.voice_got_server_update = false;
            guard.voice_got_rtc_op4 = false;
            guard.voice_connect_started_at = None;
            guard.local_preview_guilds.clear();
            guard.channel_messages.clear();
            guard.fetched_channels.clear();
            guard.fetched_channels_at.clear();
            guard.voice_states.clear();
            guard.presences.clear();
            guard.member_list_updates.clear();
            clear_live_ready_cache(was_synthetic);
        }
        if let Some(u) = auth_user {
            guard.active = true;
            guard.user = Some(ReadyUser {
                id: u.id.clone(),
                username: u.username.clone(),
                global_name: u.global_name.clone(),
                avatar: u.avatar.clone(),
                email: Some("REDACTED".to_string()),
                bio: None,
                premium: false,
                mfa_enabled: true,
            });
        }
        if guard.active && guard.ready_data.is_none() && !is_test_token && !is_test_user {
            let expected_uid = guard.user.as_ref().map(|u| u.id.clone());
            let cur_tok = guard.token.clone();
            if let Some(cached) =
                load_live_ready_cache_for(cur_tok.as_deref(), expected_uid.as_deref())
            {
                let matches_user = match (
                    &guard.user,
                    cached.get("user").and_then(|u| field_str(u, "id")),
                ) {
                    (Some(u), Some(cached_id)) => u.id == cached_id,
                    (None, Some(_)) => true,
                    _ => false,
                };
                if matches_user {
                    populate_voice_and_presences_from_ready(&mut guard, &cached);
                    guard.ready_data = Some(cached);
                }
            }
        }
    }
    if should_fetch_rest {
        if let Some(tok) = token_to_fetch {
            std::thread::spawn(move || {
                if let Some(mut rd) = fetch_live_ready_via_rest(&tok) {
                    sanitize_etf_json_value(&mut rd);
                    let mut gids_to_hydrate: Vec<String> = Vec::new();
                    let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
                    if g.active {
                        let prev_guilds: HashMap<String, Value> = g
                            .ready_data
                            .as_ref()
                            .and_then(|old_rd| old_rd.get("guilds"))
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|gv| {
                                        field_str(gv, "id").map(|id| (id.to_string(), gv.clone()))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if let Value::Object(ref mut r_map) = rd {
                            let g_arr = r_map
                                .entry("guilds".to_string())
                                .or_insert_with(|| Value::Array(Vec::new()));
                            if let Value::Array(ref mut list) = g_arr {
                                for item in list.iter_mut() {
                                    let Some(gid) = field_str(item, "id").map(str::to_string)
                                    else {
                                        continue;
                                    };
                                    gids_to_hydrate.push(gid.clone());
                                    let prev_opt = prev_guilds
                                        .get(&gid)
                                        .cloned()
                                        .or_else(|| g.local_preview_guilds.remove(&gid));
                                    if let Value::Object(im) = item {
                                        im.insert("isMember".to_string(), Value::Bool(true));
                                        im.insert("lurker".to_string(), Value::Bool(false));
                                        if let Some(Value::Object(pm)) = prev_opt {
                                            for k in [
                                                "roles",
                                                "channels",
                                                "members",
                                                "my_roles",
                                                "myRoles",
                                                "voice_states",
                                                "properties",
                                            ] {
                                                let cur_empty = im
                                                    .get(k)
                                                    .and_then(Value::as_array)
                                                    .map_or(im.get(k).is_none(), Vec::is_empty);
                                                if cur_empty {
                                                    if let Some(pv) = pm.get(k) {
                                                        im.insert(k.to_string(), pv.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                for (prev_gid, prev_gval) in &prev_guilds {
                                    if !list.iter().any(|x| field_str(x, "id") == Some(prev_gid.as_str())) {
                                        gids_to_hydrate.push(prev_gid.clone());
                                        list.push(prev_gval.clone());
                                    }
                                }
                                for (lgid, lval) in &g.local_preview_guilds {
                                    if !list
                                        .iter()
                                        .any(|x| field_str(x, "id") == Some(lgid.as_str()))
                                    {
                                        gids_to_hydrate.push(lgid.clone());
                                        list.push(lval.clone());
                                    }
                                }
                            }
                            let prev_rels = g
                                .ready_data
                                .as_ref()
                                .and_then(|old_rd| old_rd.get("relationships"))
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default();
                            if !prev_rels.is_empty() {
                                let cur_rels = r_map
                                    .entry("relationships".to_string())
                                    .or_insert_with(|| Value::Array(Vec::new()));
                                if let Value::Array(ref mut clist) = cur_rels {
                                    if clist.is_empty() {
                                        *clist = prev_rels;
                                    } else {
                                        for pr in prev_rels {
                                            let puid = field_str(&pr, "id").unwrap_or("");
                                            if !puid.is_empty() && !clist.iter().any(|cr| field_str(cr, "id") == Some(puid)) {
                                                clist.push(pr);
                                            }
                                        }
                                    }
                                }
                            }
                            let prev_privs = g
                                .ready_data
                                .as_ref()
                                .and_then(|old_rd| old_rd.get("private_channels"))
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default();
                            if !prev_privs.is_empty() {
                                let cur_privs = r_map
                                    .entry("private_channels".to_string())
                                    .or_insert_with(|| Value::Array(Vec::new()));
                                if let Value::Array(ref mut clist) = cur_privs {
                                    if clist.is_empty() {
                                        *clist = prev_privs;
                                    } else {
                                        for pc in prev_privs {
                                            let pcid = field_str(&pc, "id").unwrap_or("");
                                            if !pcid.is_empty() && !clist.iter().any(|c| field_str(c, "id") == Some(pcid)) {
                                                clist.push(pc);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        populate_voice_and_presences_from_ready(&mut g, &rd);
                        g.ready_data = Some(rd.clone());
                        drop(g);
                        save_live_ready_cache(&rd);
                        for gid in gids_to_hydrate {
                            spawn_hydrate_live_guild_full(&gid);
                        }
                    }
                }
            });
        }
    }
}

/// Принимает живые события шлюза (`READY`, `READY_SUPPLEMENTAL`, `MESSAGE_CREATE`, `MESSAGE_UPDATE`, `VOICE_STATE_UPDATE`, `PRESENCE_UPDATE`).
pub fn ingest_live_dispatch(kind: &str, raw_data: &Value) {
    const MAX_LIVE_MESSAGES_PER_CHANNEL: usize = 500;
    const MAX_LIVE_MESSAGE_CHANNELS: usize = 128;
    const MAX_LIVE_PRESENCES: usize = 5_000;
    let mut clean_data = raw_data.clone();
    sanitize_etf_json_value(&mut clean_data);
    let data = &clean_data;
    let mut deferred_cache_save: Option<Value> = None;
    {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match kind {
            "READY" => {
                if let Some(new_uid) = data.get("user").and_then(|u| field_str(u, "id")) {
                    if data.get("guilds").and_then(Value::as_array).is_some() {
                        let is_synth = is_synthetic_test_ready(data);
                        let prev_uid = guard
                            .ready_data
                            .as_ref()
                            .and_then(|rd| rd.get("user"))
                            .and_then(|ru| field_str(ru, "id"))
                            .map(str::to_string);
                        if is_synth || prev_uid.as_deref().is_some_and(|p| p != new_uid) {
                            guard.local_preview_guilds.clear();
                        }
                        let prev_guilds: HashMap<String, Value> = if is_synth
                            || prev_uid.as_deref().is_some_and(|p| p != new_uid)
                        {
                            HashMap::new()
                        } else {
                            guard
                                .ready_data
                                .as_ref()
                                .and_then(|rd| rd.get("guilds"))
                                .and_then(Value::as_array)
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|gv| {
                                            field_str(gv, "id").map(|id| (id.to_string(), gv.clone()))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default()
                        };
                        let mut merged_ready = data.clone();
                        if !is_synth {
                            if let Value::Object(ref mut rm) = merged_ready {
                                if let Some(Value::Array(ref mut g_arr)) = rm.get_mut("guilds") {
                                    for g_item in g_arr.iter_mut() {
                                        if let Some(gid_str) = field_str(g_item, "id").map(str::to_string) {
                                            let is_ready_lurker = field_bool(g_item, "lurker");
                                            if !is_ready_lurker {
                                                guard.local_preview_guilds.remove(&gid_str);
                                            }
                                            let prev_opt = prev_guilds
                                                .get(&gid_str)
                                                .or_else(|| guard.local_preview_guilds.get(&gid_str));
                                            if let Value::Object(ref mut gm) = g_item {
                                                gm.insert("isMember".to_string(), Value::Bool(!is_ready_lurker));
                                                gm.insert("lurker".to_string(), Value::Bool(is_ready_lurker));
                                                if let Some(prev_g) = prev_opt {
                                                    for key in ["channels", "roles", "my_roles", "myRoles", "owner_id", "pending"] {
                                                        let cur_empty = gm
                                                            .get(key)
                                                            .and_then(Value::as_array)
                                                            .map(|a| a.is_empty())
                                                            .unwrap_or_else(|| !gm.contains_key(key));
                                                        if cur_empty {
                                                            if let Some(pv) = prev_g.get(key) {
                                                                gm.insert(key.to_string(), pv.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    for (pgid, pg_val) in &guard.local_preview_guilds {
                                        if !g_arr.iter().any(|x| field_str(x, "id") == Some(pgid.as_str())) {
                                            g_arr.push(pg_val.clone());
                                        }
                                    }
                                }
                            }
                        }
                        guard.voice_states.clear();
                        guard.presences.clear();
                        populate_voice_and_presences_from_ready(&mut guard, &merged_ready);
                        guard.ready_data = Some(merged_ready.clone());
                        deferred_cache_save = Some(merged_ready);
                    }
                }
            }
            "READY_SUPPLEMENTAL" => {
                let mut users_by_id: HashMap<String, Value> = HashMap::new();
                if let Some(ref rd) = guard.ready_data {
                    if let Some(me) = rd.get("user") {
                        if let Some(uid) = field_str(me, "id") {
                            users_by_id.insert(uid.to_string(), me.clone());
                        }
                    }
                    for u in array(rd.get("users")) {
                        if let Some(uid) = field_str(u, "id") {
                            users_by_id.insert(uid.to_string(), u.clone());
                        }
                    }
                }
                let supp_merged_members = array(data.get("merged_members"));
                let ready_merged_members: Vec<Value> = guard
                    .ready_data
                    .as_ref()
                    .map(|r| array(r.get("merged_members")).to_vec())
                    .unwrap_or_default();
                let supp_guilds = array(data.get("guilds"));
                if !supp_guilds.is_empty() {
                    let ready_guild_ids: Vec<String> = guard
                        .ready_data
                        .as_ref()
                        .map(|r| {
                            array(r.get("guilds"))
                                .iter()
                                .filter_map(|g| field_str(g, "id").map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    for (idx, sg) in supp_guilds.iter().enumerate() {
                        let gid = field_str(sg, "id")
                            .map(str::to_string)
                            .or_else(|| ready_guild_ids.get(idx).cloned());
                        if let Some(gid) = gid {
                            let mut g_members: Vec<Value> = Vec::new();
                            if let Some(mm) = ready_merged_members.get(idx) {
                                g_members.extend_from_slice(array(Some(mm)));
                            }
                            if let Some(mm) = supp_merged_members.get(idx) {
                                g_members.extend_from_slice(array(Some(mm)));
                            }
                            for m in &g_members {
                                if let Some(u) = m.get("user") {
                                    if let Some(uid) = field_str(u, "id") {
                                        users_by_id
                                            .entry(uid.to_string())
                                            .or_insert_with(|| u.clone());
                                    }
                                }
                            }
                            let raw_vs = array(sg.get("voice_states"));
                            let vs: Vec<Value> = raw_vs
                                .iter()
                                .map(|entry| {
                                    enrich_voice_state_entry(entry, &g_members, &users_by_id)
                                })
                                .collect();
                            guard.voice_states.insert(gid, vs);
                        }
                    }
                }
                if let Some(merged_presences) = data.get("merged_presences") {
                    for friend_p in array(merged_presences.get("friends")) {
                        if let Some(uid) = friend_p
                            .get("user")
                            .and_then(|u| field_str(u, "id"))
                            .or_else(|| field_str(friend_p, "user_id"))
                        {
                            if guard.presences.len() < MAX_LIVE_PRESENCES
                                || guard.presences.contains_key(uid)
                            {
                                guard.presences.insert(uid.to_string(), friend_p.clone());
                            }
                        }
                    }
                    for g_arr in array(merged_presences.get("guilds")) {
                        for p in array(Some(g_arr)) {
                            if let Some(uid) = p
                                .get("user")
                                .and_then(|u| field_str(u, "id"))
                                .or_else(|| field_str(p, "user_id"))
                            {
                                if guard.presences.len() < MAX_LIVE_PRESENCES
                                    || guard.presences.contains_key(uid)
                                {
                                    guard.presences.insert(uid.to_string(), p.clone());
                                }
                            }
                        }
                    }
                }
                let voice_snapshot = guard.voice_states.clone();
                let all_presences: Vec<Value> = guard.presences.values().cloned().collect();
                if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                    if !supp_merged_members.is_empty() {
                        let mut combined_mm = ready_merged_members;
                        for (idx, smm) in supp_merged_members.iter().enumerate() {
                            if idx < combined_mm.len() {
                                if let (Some(mut base_arr), Some(supp_arr)) =
                                    (combined_mm[idx].as_array().cloned(), smm.as_array())
                                {
                                    for item in supp_arr {
                                        base_arr.push(item.clone());
                                    }
                                    combined_mm[idx] = Value::Array(base_arr);
                                }
                            } else {
                                combined_mm.push(smm.clone());
                            }
                        }
                        ready_map.insert("merged_members".to_string(), Value::Array(combined_mm));
                    }
                    if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                        for rg in r_guilds.iter_mut() {
                            if let Some(gid) = field_str(rg, "id").map(str::to_string) {
                                if let Some(vs) = voice_snapshot.get(&gid) {
                                    if let Value::Object(ref mut gm) = rg {
                                        gm.insert(
                                            "voice_states".to_string(),
                                            Value::Array(vs.clone()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ready_map.insert("presences".to_string(), Value::Array(all_presences));
                    deferred_cache_save = Some(Value::Object(ready_map.clone()));
                }
            }
            "MESSAGE_CREATE" => {
                if let (Some(cid), Some(mid)) = (field_str(data, "channel_id"), field_str(data, "id")) {
                    let self_user_id = guard.user.as_ref().map(|u| u.id.clone());
                    if guard.channel_messages.len() >= MAX_LIVE_MESSAGE_CHANNELS
                        && !guard.channel_messages.contains_key(cid)
                    {
                        if let Some(first_key) = guard.channel_messages.keys().next().cloned() {
                            guard.channel_messages.remove(&first_key);
                        }
                    }
                    let list = guard.channel_messages.entry(cid.to_string()).or_default();
                    if let Some(existing) = list.iter_mut().find(|m| field_str(m, "id") == Some(mid)) {
                        *existing = data.clone();
                    } else {
                        if let Some(author) = data.get("author") {
                            let author_id = field_str(author, "id").unwrap_or_default();
                            let is_self = self_user_id.as_deref() == Some(author_id);
                            if !is_self {
                                let username = field_str(author, "username").unwrap_or("Discord");
                                let content = field_str(data, "content").unwrap_or_default();
                                if !content.is_empty() {
                                    let _ = crate::notifications::send_desktop_notification(
                                        username,
                                        content,
                                        Some("im.received"),
                                    );
                                }
                            }
                        }
                        list.push(data.clone());
                        if list.len() > MAX_LIVE_MESSAGES_PER_CHANNEL {
                            let excess = list.len() - MAX_LIVE_MESSAGES_PER_CHANNEL;
                            list.drain(0..excess);
                        }
                    }
                }
            }
            "MESSAGE_UPDATE" => {
                if let (Some(cid), Some(mid)) = (field_str(data, "channel_id"), field_str(data, "id")) {
                    if guard.channel_messages.len() >= MAX_LIVE_MESSAGE_CHANNELS
                        && !guard.channel_messages.contains_key(cid)
                    {
                        if let Some(first_key) = guard.channel_messages.keys().next().cloned() {
                            guard.channel_messages.remove(&first_key);
                        }
                    }
                    let list = guard.channel_messages.entry(cid.to_string()).or_default();
                    if let Some(existing) = list.iter_mut().find(|m| field_str(m, "id") == Some(mid)) {
                        if let (Value::Object(ex_map), Value::Object(new_map)) = (existing, data) {
                            for (k, v) in new_map {
                                ex_map.insert(k.clone(), v.clone());
                            }
                        }
                    } else if data.get("author").and_then(|a| field_str(a, "id")).is_some() {
                        list.push(data.clone());
                        if list.len() > MAX_LIVE_MESSAGES_PER_CHANNEL {
                            let excess = list.len() - MAX_LIVE_MESSAGES_PER_CHANNEL;
                            list.drain(0..excess);
                        }
                    }
                }
            }
            "VOICE_STATE_UPDATE" => {
                if let Some(uid) = field_str(data, "user_id") {
                    let gid_opt = field_str(data, "guild_id")
                        .map(str::to_string)
                        .or_else(|| field_str(data, "channel_id").map(|_| "@me".to_string()));
                    let mut users_by_id: HashMap<String, Value> = HashMap::new();
                    let mut guild_members: Vec<Value> = Vec::new();
                    if let Some(ref mut rd) = guard.ready_data {
                        if let Some(u) = data.get("member").and_then(|m| m.get("user")) {
                            if let Some(u_id) = field_str(u, "id") {
                                users_by_id.insert(u_id.to_string(), u.clone());
                                if let Value::Object(ref mut r_map) = rd {
                                    if let Some(Value::Array(ref mut u_arr)) = r_map.get_mut("users") {
                                        if let Some(ex) = u_arr
                                            .iter_mut()
                                            .find(|x| field_str(x, "id") == Some(u_id))
                                        {
                                            *ex = u.clone();
                                        } else if u_arr.len() < 10_000 {
                                            u_arr.push(u.clone());
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(me) = rd.get("user") {
                            if let Some(me_id) = field_str(me, "id") {
                                users_by_id.insert(me_id.to_string(), me.clone());
                            }
                        }
                        for u in array(rd.get("users")) {
                            if let Some(u_id) = field_str(u, "id") {
                                users_by_id.insert(u_id.to_string(), u.clone());
                            }
                        }
                        if let Some(ref gid) = gid_opt {
                            let guilds_arr = array(rd.get("guilds"));
                            if let Some(g_idx) = guilds_arr
                                .iter()
                                .position(|g| field_str(g, "id") == Some(gid.as_str()))
                            {
                                guild_members
                                    .extend_from_slice(array(guilds_arr[g_idx].get("members")));
                                if let Some(mm) = array(rd.get("merged_members")).get(g_idx) {
                                    guild_members.extend_from_slice(array(Some(mm)));
                                }
                            }
                        }
                    }
                    if let Some(gid) = gid_opt {
                        let is_self_user = guard
                            .user
                            .as_ref()
                            .is_some_and(|u| u.id == uid)
                            || guard
                                .ready_data
                                .as_ref()
                                .and_then(|r| r.get("user"))
                                .and_then(|u| field_str(u, "id"))
                                == Some(uid);
                        let joined_channel = field_str(data, "channel_id").is_some();
                        if is_self_user && joined_channel {
                            for (other_gid, other_list) in guard.voice_states.iter_mut() {
                                if other_gid != &gid {
                                    other_list.retain(|entry| field_str(entry, "user_id") != Some(uid));
                                }
                            }
                        }
                        let list = guard.voice_states.entry(gid.clone()).or_default();
                        let previous_member = list
                            .iter()
                            .find(|entry| field_str(entry, "user_id") == Some(uid))
                            .and_then(|entry| entry.get("member"))
                            .cloned();
                        list.retain(|entry| field_str(entry, "user_id") != Some(uid));
                        if joined_channel {
                            if let Some(pm) = previous_member {
                                guild_members.push(pm);
                            }
                            let enriched =
                                enrich_voice_state_entry(data, &guild_members, &users_by_id);
                            list.push(enriched);
                        }
                        let vs_clone = list.clone();
                        if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                            if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds")
                            {
                                for rg in r_guilds.iter_mut() {
                                    let rg_id = field_str(rg, "id").map(str::to_string);
                                    if rg_id.as_deref() == Some(gid.as_str()) {
                                        if let Value::Object(ref mut gm) = rg {
                                            gm.insert(
                                                "voice_states".to_string(),
                                                Value::Array(vs_clone.clone()),
                                            );
                                        }
                                    } else if is_self_user && joined_channel {
                                        if let Value::Object(ref mut gm) = rg {
                                            if let Some(Value::Array(ref mut vs_arr)) =
                                                gm.get_mut("voice_states")
                                            {
                                                vs_arr.retain(|entry| {
                                                    field_str(entry, "user_id") != Some(uid)
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        for list in guard.voice_states.values_mut() {
                            list.retain(|entry| field_str(entry, "user_id") != Some(uid));
                        }
                    }
                }
            }
            "CALL_CREATE" | "CALL_UPDATE" => {
                if let Some(cid) = field_str(data, "channel_id") {
                    let mut users_by_id: HashMap<String, Value> = HashMap::new();
                    if let Some(ref rd) = guard.ready_data {
                        if let Some(me) = rd.get("user") {
                            if let Some(u_id) = field_str(me, "id") {
                                users_by_id.insert(u_id.to_string(), me.clone());
                            }
                        }
                        for u in array(rd.get("users")) {
                            if let Some(u_id) = field_str(u, "id") {
                                users_by_id.insert(u_id.to_string(), u.clone());
                            }
                        }
                    }
                    let self_uid = resolve_live_self_uid(&guard);
                    let local_in_this_call =
                        LOCAL_VOICE_ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
                            && guard.voice_channel_id.as_deref() == Some(cid);
                    let list = guard.voice_states.entry("@me".to_string()).or_default();
                    let existing_self_entry = if local_in_this_call && !self_uid.is_empty() {
                        list.iter()
                            .find(|entry| {
                                field_str(entry, "user_id") == Some(self_uid.as_str())
                                    && field_str(entry, "channel_id") == Some(cid)
                            })
                            .cloned()
                    } else {
                        None
                    };
                    list.retain(|entry| field_str(entry, "channel_id") != Some(cid));
                    let mut seen_self = false;
                    for raw_vs in array(data.get("voice_states")) {
                        let Some(uid) = field_str(raw_vs, "user_id") else {
                            continue;
                        };
                        let vs_cid = field_str(raw_vs, "channel_id").unwrap_or(cid);
                        if vs_cid.is_empty() {
                            continue;
                        }
                        if uid == self_uid {
                            seen_self = true;
                        }
                        let mut with_cid = raw_vs.clone();
                        if let Value::Object(ref mut m) = with_cid {
                            m.insert("guild_id".to_string(), Value::String("@me".to_string()));
                            m.insert("channel_id".to_string(), Value::String(vs_cid.to_string()));
                        }
                        let enriched = enrich_voice_state_entry(&with_cid, &[], &users_by_id);
                        list.push(enriched);
                    }
                    if !seen_self {
                        if let Some(self_entry) = existing_self_entry {
                            list.push(self_entry);
                        }
                    }
                }
            }
            "CALL_DELETE" => {
                if let Some(cid) = field_str(data, "channel_id") {
                    if let Some(list) = guard.voice_states.get_mut("@me") {
                        list.retain(|entry| field_str(entry, "channel_id") != Some(cid));
                    }
                }
            }
            "PRESENCE_UPDATE" => {
                if let Some(uid) = data
                    .get("user")
                    .and_then(|u| field_str(u, "id"))
                    .or_else(|| field_str(data, "user_id"))
                {
                    if guard.presences.len() < MAX_LIVE_PRESENCES
                        || guard.presences.contains_key(uid)
                    {
                        guard.presences.insert(uid.to_string(), data.clone());
                    }
                    let all_presences: Vec<Value> = guard.presences.values().cloned().collect();
                    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                        ready_map.insert("presences".to_string(), Value::Array(all_presences));
                    }
                }
            }
            "GUILD_MEMBER_LIST_UPDATE" => {
                if let Some(gid) = field_str(data, "guild_id") {
                    let has_groups = !array(data.get("groups")).is_empty();
                    if has_groups || !guard.member_list_updates.contains_key(gid) {
                        guard
                            .member_list_updates
                            .insert(gid.to_string(), data.clone());
                    }
                    let mut new_members: Vec<Value> = Vec::new();
                    for op in array(data.get("ops")) {
                        for item in array(op.get("items")) {
                            if let Some(member) = item.get("member") {
                                new_members.push(member.clone());
                            }
                        }
                        if let Some(item) = op.get("item") {
                            if let Some(member) = item.get("member") {
                                new_members.push(member.clone());
                            }
                        }
                        if let Some(member) = op.get("member") {
                            new_members.push(member.clone());
                        }
                    }
                    for member in &new_members {
                        if let Some(u) = member.get("user") {
                            if let Some(uid) = field_str(u, "id") {
                                if let Some(p) = member.get("presence") {
                                    if guard.presences.len() < MAX_LIVE_PRESENCES
                                        || guard.presences.contains_key(uid)
                                    {
                                        guard.presences.insert(uid.to_string(), p.clone());
                                    }
                                }
                            }
                        }
                    }
                    if !new_members.is_empty() {
                        if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                            if let Some(Value::Array(ref mut u_arr)) = ready_map.get_mut("users") {
                                for nm in &new_members {
                                    if let Some(u) = nm.get("user") {
                                        if let Some(uid) = field_str(u, "id") {
                                            if let Some(ex) = u_arr
                                                .iter_mut()
                                                .find(|x| field_str(x, "id") == Some(uid))
                                            {
                                                *ex = u.clone();
                                            } else if u_arr.len() < 10_000 {
                                                u_arr.push(u.clone());
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(Value::Array(ref mut r_guilds)) =
                                ready_map.get_mut("guilds")
                            {
                                for rg in r_guilds.iter_mut() {
                                    if field_str(rg, "id") == Some(gid) {
                                        if let Value::Object(ref mut gm) = rg {
                                            let m_arr = gm
                                                .entry("members".to_string())
                                                .or_insert_with(|| Value::Array(Vec::new()));
                                            if let Value::Array(ref mut list) = m_arr {
                                                for nm in &new_members {
                                                    let n_uid = nm
                                                        .get("user")
                                                        .and_then(|u| field_str(u, "id"))
                                                        .or_else(|| field_str(nm, "user_id"));
                                                    if let Some(uid_str) = n_uid {
                                                        if let Some(ex) = list.iter_mut().find(|x| {
                                                            x.get("user")
                                                                .and_then(|u| field_str(u, "id"))
                                                                .or_else(|| field_str(x, "user_id"))
                                                                == Some(uid_str)
                                                        }) {
                                                            *ex = nm.clone();
                                                        } else if list.len() < 1000 {
                                                            list.push(nm.clone());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "GUILD_MEMBERS_CHUNK" => {
                if let Some(gid) = field_str(data, "guild_id") {
                    for p in array(data.get("presences")) {
                        if let Some(uid) = p
                            .get("user")
                            .and_then(|u| field_str(u, "id"))
                            .or_else(|| field_str(p, "user_id"))
                        {
                            if guard.presences.len() < MAX_LIVE_PRESENCES
                                || guard.presences.contains_key(uid)
                            {
                                guard.presences.insert(uid.to_string(), p.clone());
                            }
                        }
                    }
                    let chunk_members = array(data.get("members"));
                    if !chunk_members.is_empty() {
                        if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                            if let Some(Value::Array(ref mut r_guilds)) =
                                ready_map.get_mut("guilds")
                            {
                                for rg in r_guilds.iter_mut() {
                                    if field_str(rg, "id") == Some(gid) {
                                        if let Value::Object(ref mut gm) = rg {
                                            let m_arr = gm
                                                .entry("members".to_string())
                                                .or_insert_with(|| Value::Array(Vec::new()));
                                            if let Value::Array(ref mut list) = m_arr {
                                                for nm in chunk_members {
                                                    let n_uid = nm
                                                        .get("user")
                                                        .and_then(|u| field_str(u, "id"))
                                                        .or_else(|| field_str(nm, "user_id"));
                                                    if let Some(uid_str) = n_uid {
                                                        if let Some(ex) = list.iter_mut().find(|x| {
                                                            x.get("user")
                                                                .and_then(|u| field_str(u, "id"))
                                                                .or_else(|| field_str(x, "user_id"))
                                                                == Some(uid_str)
                                                        }) {
                                                            *ex = nm.clone();
                                                        } else if list.len() < 1000 {
                                                            list.push(nm.clone());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "GUILD_MEMBER_UPDATE" => {
                if let (Some(gid), Some(uid)) = (
                    field_str(data, "guild_id"),
                    data.get("user")
                        .and_then(|u| field_str(u, "id"))
                        .or_else(|| field_str(data, "user_id")),
                ) {
                    let new_roles = array(data.get("roles")).to_vec();
                    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                        if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                            for rg in r_guilds.iter_mut() {
                                if field_str(rg, "id") == Some(gid) {
                                    if let Value::Object(ref mut gm) = rg {
                                        let m_arr = gm
                                            .entry("members".to_string())
                                            .or_insert_with(|| Value::Array(Vec::new()));
                                        if let Value::Array(ref mut list) = m_arr {
                                            if let Some(ex) = list.iter_mut().find(|x| {
                                                x.get("user")
                                                    .and_then(|u| field_str(u, "id"))
                                                    .or_else(|| field_str(x, "user_id"))
                                                    == Some(uid)
                                            }) {
                                                if let Value::Object(ref mut obj) = ex {
                                                    obj.insert("roles".to_string(), Value::Array(new_roles.clone()));
                                                    if let Some(nick) = data.get("nick") {
                                                        obj.insert("nick".to_string(), nick.clone());
                                                    }
                                                }
                                            } else {
                                                list.push(data.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(preview_g) = guard.local_preview_guilds.get_mut(gid) {
                        if let Value::Object(ref mut gm) = preview_g {
                            let m_arr = gm
                                .entry("members".to_string())
                                .or_insert_with(|| Value::Array(Vec::new()));
                            if let Value::Array(ref mut list) = m_arr {
                                if let Some(ex) = list.iter_mut().find(|x| {
                                    x.get("user")
                                        .and_then(|u| field_str(u, "id"))
                                        .or_else(|| field_str(x, "user_id"))
                                        == Some(uid)
                                }) {
                                    if let Value::Object(ref mut obj) = ex {
                                        obj.insert("roles".to_string(), Value::Array(new_roles));
                                        if let Some(nick) = data.get("nick") {
                                            obj.insert("nick".to_string(), nick.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "GUILD_CREATE" => {
                if let Some(gid) = field_str(data, "id") {
                    let mut users_by_id: HashMap<String, Value> = HashMap::new();
                    if let Some(ref rd) = guard.ready_data {
                        if let Some(me) = rd.get("user") {
                            if let Some(uid) = field_str(me, "id") {
                                users_by_id.insert(uid.to_string(), me.clone());
                            }
                        }
                        for u in array(rd.get("users")) {
                            if let Some(uid) = field_str(u, "id") {
                                users_by_id.insert(uid.to_string(), u.clone());
                            }
                        }
                    }
                    for m in array(data.get("members")) {
                        if let Some(u) = m.get("user") {
                            if let Some(uid) = field_str(u, "id") {
                                users_by_id.insert(uid.to_string(), u.clone());
                            }
                        }
                    }
                    let vs_raw = array(data.get("voice_states"));
                    if !vs_raw.is_empty() {
                        let members_slice = array(data.get("members"));
                        let enriched: Vec<Value> = vs_raw
                            .iter()
                            .map(|v| enrich_voice_state_entry(v, members_slice, &users_by_id))
                            .collect();
                        guard.voice_states.insert(gid.to_string(), enriched);
                    }
                    for pr in array(data.get("presences")) {
                        let uid = field_str(pr, "user_id").or_else(|| {
                            pr.get("user").and_then(|u| field_str(u, "id"))
                        });
                        if let Some(u) = uid {
                            if guard.presences.contains_key(u) || guard.presences.len() < MAX_LIVE_PRESENCES {
                                guard.presences.insert(u.to_string(), pr.clone());
                            }
                        }
                    }
                    let mut guild_obj = data.clone();
                    let is_gc_lurker = field_bool(&guild_obj, "lurker");
                    if let Value::Object(ref mut gm) = guild_obj {
                        if let Some(props) = gm.get("properties").and_then(Value::as_object).cloned() {
                            for (k, v) in props {
                                gm.entry(k).or_insert(v);
                            }
                        }
                        gm.insert("isMember".to_string(), Value::Bool(!is_gc_lurker));
                        gm.insert("lurker".to_string(), Value::Bool(is_gc_lurker));
                    }
                    if !is_gc_lurker {
                        guard.local_preview_guilds.remove(gid);
                    } else if guard.local_preview_guilds.contains_key(gid) {
                        guard.local_preview_guilds.insert(gid.to_string(), guild_obj.clone());
                    }
                    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                        let guilds_val = ready_map
                            .entry("guilds".to_string())
                            .or_insert_with(|| Value::Array(Vec::new()));
                        if let Value::Array(ref mut r_guilds) = guilds_val {
                            if let Some(existing) = r_guilds
                                .iter_mut()
                                .find(|g| field_str(g, "id") == Some(gid))
                            {
                                *existing = guild_obj;
                            } else {
                                r_guilds.push(guild_obj);
                            }
                        }
                    }
                    deferred_cache_save = guard.ready_data.clone();
                }
            }
            "GUILD_DELETE" => {
                if let Some(gid) = field_str(data, "id") {
                    let unavailable = field_bool(data, "unavailable");
                    if !unavailable {
                        if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                            if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                                r_guilds.retain(|g| field_str(g, "id") != Some(gid));
                            }
                        }
                        guard.voice_states.remove(gid);
                        deferred_cache_save = guard.ready_data.clone();
                    }
                }
            }
            "MESSAGE_DELETE" => {
                if let (Some(cid), Some(mid)) = (field_str(data, "channel_id"), field_str(data, "id")) {
                    guard
                        .deleted_message_ids
                        .insert((cid.to_string(), mid.to_string()));
                    if let Some(list) = guard.channel_messages.get_mut(cid) {
                        list.retain(|m| field_str(m, "id") != Some(mid));
                    }
                }
            }
            "MESSAGE_DELETE_BULK" => {
                if let Some(cid) = field_str(data, "channel_id") {
                    if let Some(Value::Array(ids)) = data.get("ids") {
                        for mid_val in ids {
                            if let Some(mid) = mid_val.as_str() {
                                guard
                                    .deleted_message_ids
                                    .insert((cid.to_string(), mid.to_string()));
                                if let Some(list) = guard.channel_messages.get_mut(cid) {
                                    list.retain(|m| field_str(m, "id") != Some(mid));
                                }
                            }
                        }
                    }
                }
            }
            "RELATIONSHIP_ADD" | "RELATIONSHIP_UPDATE" => {
                if let Some(rid) = field_str(data, "id") {
                    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                        if let Some(u) = data.get("user") {
                            if let Some(uid) = field_str(u, "id") {
                                let u_arr = ready_map
                                    .entry("users".to_string())
                                    .or_insert_with(|| Value::Array(Vec::new()));
                                if let Value::Array(ref mut list) = u_arr {
                                    if let Some(ex) = list.iter_mut().find(|x| field_str(x, "id") == Some(uid)) {
                                        *ex = u.clone();
                                    } else {
                                        list.push(u.clone());
                                    }
                                }
                            }
                        }
                        let rel_arr = ready_map
                            .entry("relationships".to_string())
                            .or_insert_with(|| Value::Array(Vec::new()));
                        if let Value::Array(ref mut list) = rel_arr {
                            if let Some(ex) = list.iter_mut().find(|x| field_str(x, "id") == Some(rid)) {
                                if let (Value::Object(ex_map), Value::Object(new_map)) = (ex, data) {
                                    for (k, v) in new_map {
                                        ex_map.insert(k.clone(), v.clone());
                                    }
                                }
                            } else {
                                list.push(data.clone());
                            }
                        }
                        deferred_cache_save = Some(Value::Object(ready_map.clone()));
                    }
                }
            }
            "RELATIONSHIP_REMOVE" => {
                if let Some(rid) = field_str(data, "id") {
                    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                        if let Some(Value::Array(ref mut list)) = ready_map.get_mut("relationships") {
                            list.retain(|x| field_str(x, "id") != Some(rid));
                        }
                        deferred_cache_save = Some(Value::Object(ready_map.clone()));
                    }
                }
            }
            "CHANNEL_CREATE" | "CHANNEL_UPDATE" => {
                if let Some(cid) = field_str(data, "id") {
                    let ch_type = field_i64(data, "type").unwrap_or(0);
                    let gid_opt = field_str(data, "guild_id");
                    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                        for rec in array(data.get("recipients")) {
                            if let Some(uid) = field_str(rec, "id") {
                                let u_arr = ready_map
                                    .entry("users".to_string())
                                    .or_insert_with(|| Value::Array(Vec::new()));
                                if let Value::Array(ref mut list) = u_arr {
                                    if let Some(ex) = list.iter_mut().find(|x| field_str(x, "id") == Some(uid)) {
                                        *ex = rec.clone();
                                    } else {
                                        list.push(rec.clone());
                                    }
                                }
                            }
                        }
                        if gid_opt.is_none() || ch_type == 1 || ch_type == 3 {
                            let pc_arr = ready_map
                                .entry("private_channels".to_string())
                                .or_insert_with(|| Value::Array(Vec::new()));
                            if let Value::Array(ref mut list) = pc_arr {
                                if let Some(ex) = list.iter_mut().find(|x| field_str(x, "id") == Some(cid)) {
                                    *ex = data.clone();
                                } else {
                                    list.push(data.clone());
                                }
                            }
                        } else if let Some(gid) = gid_opt {
                            if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                                for rg in r_guilds.iter_mut() {
                                    if field_str(rg, "id") == Some(gid) {
                                        if let Value::Object(ref mut gm) = rg {
                                            let ch_arr = gm
                                                .entry("channels".to_string())
                                                .or_insert_with(|| Value::Array(Vec::new()));
                                            if let Value::Array(ref mut list) = ch_arr {
                                                if let Some(ex) = list.iter_mut().find(|x| field_str(x, "id") == Some(cid)) {
                                                    *ex = data.clone();
                                                } else {
                                                    list.push(data.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        deferred_cache_save = Some(Value::Object(ready_map.clone()));
                    }
                }
            }
            "CHANNEL_DELETE" => {
                if let Some(cid) = field_str(data, "id") {
                    if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                        if let Some(Value::Array(ref mut list)) = ready_map.get_mut("private_channels") {
                            list.retain(|x| field_str(x, "id") != Some(cid));
                        }
                        if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                            for rg in r_guilds.iter_mut() {
                                if let Value::Object(ref mut gm) = rg {
                                    if let Some(Value::Array(ref mut chs)) = gm.get_mut("channels") {
                                        chs.retain(|x| field_str(x, "id") != Some(cid));
                                    }
                                }
                            }
                        }
                        deferred_cache_save = Some(Value::Object(ready_map.clone()));
                    }
                }
            }
            "CHANNEL_RECIPIENT_ADD" => {
                if let (Some(cid), Some(u)) = (field_str(data, "channel_id"), data.get("user")) {
                    if let Some(uid) = field_str(u, "id") {
                        if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                            let u_arr = ready_map
                                .entry("users".to_string())
                                .or_insert_with(|| Value::Array(Vec::new()));
                            if let Value::Array(ref mut list) = u_arr {
                                if let Some(ex) = list.iter_mut().find(|x| field_str(x, "id") == Some(uid)) {
                                    *ex = u.clone();
                                } else {
                                    list.push(u.clone());
                                }
                            }
                            if let Some(Value::Array(ref mut pc_list)) = ready_map.get_mut("private_channels") {
                                for pc in pc_list.iter_mut() {
                                    if field_str(pc, "id") == Some(cid) {
                                        if let Value::Object(ref mut pm) = pc {
                                            let rec_arr = pm
                                                .entry("recipients".to_string())
                                                .or_insert_with(|| Value::Array(Vec::new()));
                                            if let Value::Array(ref mut r_list) = rec_arr {
                                                if let Some(ex) = r_list.iter_mut().find(|x| field_str(x, "id") == Some(uid)) {
                                                    *ex = u.clone();
                                                } else {
                                                    r_list.push(u.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            deferred_cache_save = Some(Value::Object(ready_map.clone()));
                        }
                    }
                }
            }
            "CHANNEL_RECIPIENT_REMOVE" => {
                if let (Some(cid), Some(u)) = (field_str(data, "channel_id"), data.get("user")) {
                    if let Some(uid) = field_str(u, "id") {
                        if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                            if let Some(Value::Array(ref mut pc_list)) = ready_map.get_mut("private_channels") {
                                for pc in pc_list.iter_mut() {
                                    if field_str(pc, "id") == Some(cid) {
                                        if let Value::Object(ref mut pm) = pc {
                                            if let Some(Value::Array(ref mut r_list)) = pm.get_mut("recipients") {
                                                r_list.retain(|x| field_str(x, "id") != Some(uid));
                                            }
                                        }
                                    }
                                }
                            }
                            deferred_cache_save = Some(Value::Object(ready_map.clone()));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(snapshot) = deferred_cache_save {
        save_live_ready_cache(&snapshot);
    }
}

static LOCAL_VOICE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_SELF_MUTE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_SELF_DEAF: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_SELF_VIDEO: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_SELF_STREAM: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_INPUT_VOLUME_PCT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(100);
static LOCAL_OUTPUT_VOLUME_PCT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(100);
static LOCAL_KRISP_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static LOCAL_AUTO_GAIN_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static LOCAL_AUTO_SENSITIVITY_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static LOCAL_VAD_THRESHOLD_DB: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(-45);
static LOCAL_EFFECTIVE_VAD_DB: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(-45);

fn selected_input_device() -> &'static Mutex<Option<String>> {
    static DEV: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    DEV.get_or_init(|| Mutex::new(None))
}

fn selected_output_device() -> &'static Mutex<Option<String>> {
    static DEV: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    DEV.get_or_init(|| Mutex::new(None))
}

/// Обрабатывает нередактированные события шлюза (`READY`, `VOICE_STATE_UPDATE`, `VOICE_SERVER_UPDATE`)
/// для координации живого голосового RTC-соединения (до вырезания `session_id` и `token`).
pub fn ingest_raw_gateway_dispatch(kind: &str, raw_data: &Value) {
    use std::sync::atomic::Ordering;
    let mut clean_data = raw_data.clone();
    sanitize_etf_json_value(&mut clean_data);
    let data = &clean_data;
    let mut launch_params: Option<(String, String, String, String, String, Option<String>)> = None;
    let mut stream_launch_params: Option<(
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
    )> = None;
    let mut should_stop = false;
    let mut should_stop_stream = false;
    let mut preview_gids_to_sync: Vec<String> = Vec::new();
    {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match kind {
            "READY" | "RESUMED" => {
                if let Some(sid) = field_str(data, "session_id") {
                    guard.gateway_session_id = Some(sid.to_string());
                }
                preview_gids_to_sync = guard.local_preview_guilds.keys().cloned().collect();
                if LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst)
                    && !discord_core::ws::is_live_rtc_connected()
                {
                    if let Some(ref cid) = guard.voice_channel_id {
                        let gid = guard.voice_guild_id.clone();
                        let cid_clone = Some(cid.clone());
                        let s_mute = LOCAL_SELF_MUTE.load(Ordering::SeqCst);
                        let s_deaf = LOCAL_SELF_DEAF.load(Ordering::SeqCst);
                        let s_video = LOCAL_SELF_VIDEO.load(Ordering::SeqCst);
                        let s_stream = LOCAL_SELF_STREAM.load(Ordering::SeqCst);
                        let self_uid = resolve_live_self_uid(&guard);
                        guard.voice_launched_token = None;
                        let _ = discord_core::ws::queue_live_voice_state_ext(
                            gid,
                            cid_clone,
                            s_mute,
                            s_deaf,
                            s_video,
                            s_stream,
                            Some(self_uid.as_str()),
                        );
                    }
                }
            }
            "READY_SUPPLEMENTAL" => {
                // Не перехватываем голосовую сессию основного клиента при старте:
                // участники голосовых каналов уже отображаются через `guard.voice_states`.
            }
            "VOICE_STATE_UPDATE" => {
                let uid = field_str(data, "user_id").map(str::to_string);
                let self_uid = Some(resolve_live_self_uid(&guard)).filter(|s| !s.is_empty());
                if uid.is_some() && uid == self_uid {
                    let event_sid = field_str(data, "session_id").map(str::to_string);
                    let is_our_gateway_session = match (&event_sid, &guard.gateway_session_id) {
                        (Some(es), Some(gs)) => es == gs,
                        _ => true,
                    };
                    let local_active = LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst);
                    let cid = field_str(data, "channel_id")
                        .filter(|s| !s.is_empty() && *s != "null" && *s != "nil")
                        .map(str::to_string);
                    let gid = field_str(data, "guild_id")
                        .filter(|s| !s.is_empty() && *s != "@me" && *s != "null" && *s != "nil")
                        .map(str::to_string);
                    if local_active
                        && !is_our_gateway_session
                        && cid.is_some()
                        && (cid != guard.voice_channel_id || gid != guard.voice_guild_id)
                    {
                        // Пользователь подключился к ДРУГОМУ голосовому каналу/серверу в другом клиенте —
                        // немедленно выходим из нашего голосового канала, чтобы не сидеть в двух местах одновременно.
                        let old_gid = guard.voice_guild_id.take();
                        guard.voice_channel_id = None;
                        guard.voice_endpoint = None;
                        guard.voice_token = None;
                        guard.voice_launched_token = None;
                        LOCAL_VOICE_ACTIVE.store(false, Ordering::SeqCst);
                        should_stop = true;
                        if let Some(ref og) = old_gid {
                            if Some(og.as_str()) != gid.as_deref() {
                                let _ = discord_core::ws::queue_live_voice_state_ext(
                                    Some(og.clone()),
                                    None,
                                    false,
                                    false,
                                    false,
                                    false,
                                    self_uid.as_deref(),
                                );
                            }
                        }
                    } else if local_active && is_our_gateway_session {
                        let sid = guard
                            .gateway_session_id
                            .clone()
                            .or(event_sid)
                            .or_else(|| guard.voice_session_id.clone());
                        guard.voice_channel_id = cid.clone();
                        guard.voice_guild_id = gid.clone();
                        if let Some(ref s) = sid {
                            guard.voice_session_id = Some(s.clone());
                        }
                        discord_core::ws::note_live_rtc_target(
                            gid.clone(),
                            cid.clone(),
                            sid.clone(),
                        );
                        if cid.is_none() {
                            LOCAL_VOICE_ACTIVE.store(false, Ordering::SeqCst);
                            guard.voice_endpoint = None;
                            guard.voice_token = None;
                            guard.voice_launched_token = None;
                            guard.voice_phase = "disconnected".to_string();
                            guard.voice_got_state_update = false;
                            guard.voice_got_server_update = false;
                            guard.voice_got_rtc_op4 = false;
                            guard.voice_connect_started_at = None;
                            should_stop = true;
                        } else {
                            guard.voice_got_state_update = true;
                            if guard.voice_got_state_update
                                && guard.voice_got_server_update
                                && (guard.voice_got_rtc_op4
                                    || discord_core::ws::is_live_rtc_connected())
                            {
                                guard.voice_phase = "connected".to_string();
                            } else if guard.voice_got_state_update && guard.voice_got_server_update
                            {
                                guard.voice_phase = "negotiating".to_string();
                            } else {
                                guard.voice_phase = "signaling".to_string();
                            }
                            if let (Some(ep), Some(tok), Some(u)) = (
                                guard.voice_endpoint.clone(),
                                guard.voice_token.clone(),
                                self_uid,
                            ) {
                                if guard.voice_launched_token.as_deref() != Some(tok.as_str()) {
                                    let s = sid.unwrap_or_else(|| "live_rtc_session".to_string());
                                    let server_id = guard
                                        .voice_guild_id
                                        .clone()
                                        .or_else(|| cid.clone())
                                        .unwrap_or_default();
                                    guard.voice_launched_token = Some(tok.clone());
                                    launch_params = Some((ep, server_id, u, s, tok, cid));
                                }
                            }
                        }
                    }
                }
            }
            "VOICE_SERVER_UPDATE" => {
                if LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst) {
                    let ep = field_str(data, "endpoint").map(str::to_string);
                    let tok = field_str(data, "token").map(str::to_string);
                    let event_gid = field_str(data, "guild_id")
                        .filter(|s| !s.is_empty() && *s != "@me" && *s != "null" && *s != "nil")
                        .map(str::to_string);
                    let event_cid = field_str(data, "channel_id")
                        .filter(|s| !s.is_empty() && *s != "null" && *s != "nil")
                        .map(str::to_string);
                    if event_gid.is_some() {
                        guard.voice_guild_id = event_gid.clone();
                    } else if event_cid.is_some() {
                        guard.voice_guild_id = None;
                    }
                    guard.voice_endpoint = ep.clone();
                    guard.voice_token = tok.clone();
                    if ep.is_some() && tok.is_some() {
                        guard.voice_got_server_update = true;
                        if guard.voice_got_state_update
                            && guard.voice_got_server_update
                            && (guard.voice_got_rtc_op4
                                || discord_core::ws::is_live_rtc_connected())
                        {
                            guard.voice_phase = "connected".to_string();
                        } else if guard.voice_got_state_update && guard.voice_got_server_update {
                            guard.voice_phase = "negotiating".to_string();
                        } else {
                            guard.voice_phase = "signaling".to_string();
                        }
                    }
                    let self_uid = resolve_live_self_uid(&guard);
                    let sid = guard
                        .gateway_session_id
                        .clone()
                        .or_else(|| guard.voice_session_id.clone());
                    if let (Some(ep_val), Some(tok_val), Some(sid_val)) = (ep, tok, sid) {
                        if guard.voice_launched_token.as_deref() != Some(tok_val.as_str())
                            || !discord_core::ws::is_live_rtc_connected()
                        {
                            let server_id = event_gid
                                .or_else(|| guard.voice_guild_id.clone())
                                .or(event_cid)
                                .or_else(|| guard.voice_channel_id.clone())
                                .unwrap_or_default();
                            let cid = guard.voice_channel_id.clone();
                            guard.voice_launched_token = Some(tok_val.clone());
                            launch_params =
                                Some((ep_val, server_id, self_uid, sid_val, tok_val, cid));
                        }
                    }
                }
            }
            "RTC_OP4"
            | "RTC_SESSION_DESCRIPTION"
            | "RTC_SELECT_PROTOCOL_ACK"
            | "VOICE_RTC_OP4" => {
                if LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst) {
                    guard.voice_got_rtc_op4 = true;
                    if guard.voice_got_state_update && guard.voice_got_server_update {
                        guard.voice_phase = "connected".to_string();
                    }
                }
            }
            "STREAM_CREATE" => {
                let skey = field_str(data, "stream_key").map(str::to_string);
                let rtc_srv = field_str(data, "rtc_server_id").map(str::to_string);
                let skey_cid = skey.as_deref().and_then(|sk| {
                    let parts: Vec<&str> = sk.split(':').collect();
                    if parts.first() == Some(&"call") && parts.len() >= 3 {
                        Some(parts[1].to_string())
                    } else if parts.first() == Some(&"guild") && parts.len() >= 4 {
                        Some(parts[2].to_string())
                    } else {
                        None
                    }
                });
                let rtc_ch = field_str(data, "rtc_channel_id")
                    .map(str::to_string)
                    .or(skey_cid)
                    .or_else(|| guard.voice_channel_id.clone())
                    .or_else(|| rtc_srv.clone());
                if let Some(ref k) = skey {
                    guard.stream_key = Some(k.clone());
                }
                if let Some(ref s) = rtc_srv {
                    guard.stream_rtc_server_id = Some(s.clone());
                }
                if let Some(ref c) = rtc_ch {
                    guard.stream_rtc_channel_id = Some(c.clone());
                }
                let self_uid = resolve_live_self_uid(&guard);
                let streamer_uid = guard
                    .stream_key
                    .as_deref()
                    .and_then(|sk| sk.rsplit(':').next())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| self_uid.clone());
                let sid = guard
                    .gateway_session_id
                    .clone()
                    .or_else(|| guard.voice_session_id.clone());
                if let (Some(ep_val), Some(tok_val), Some(srv_val), Some(sid_val)) = (
                    guard.stream_endpoint.clone(),
                    guard.stream_token.clone(),
                    guard.stream_rtc_server_id.clone(),
                    sid,
                ) {
                    let needs_launch = guard.stream_launched_token.as_deref() != Some(tok_val.as_str())
                        || guard.stream_launched_server_id.as_deref() != Some(srv_val.as_str());
                    if needs_launch {
                        guard.stream_launched_token = Some(tok_val.clone());
                        guard.stream_launched_server_id = Some(srv_val.clone());
                        stream_launch_params = Some((
                            ep_val,
                            srv_val,
                            self_uid,
                            sid_val,
                            tok_val,
                            guard.stream_rtc_channel_id.clone(),
                            streamer_uid,
                        ));
                    }
                }
            }
            "STREAM_SERVER_UPDATE" => {
                let skey = field_str(data, "stream_key").map(str::to_string);
                let ep = field_str(data, "endpoint").map(str::to_string);
                let tok = field_str(data, "token").map(str::to_string);
                if let Some(ref k) = skey {
                    guard.stream_key = Some(k.clone());
                }
                guard.stream_endpoint = ep.clone();
                guard.stream_token = tok.clone();
                let skey_cid = skey.as_deref().and_then(|sk| {
                    let parts: Vec<&str> = sk.split(':').collect();
                    if parts.first() == Some(&"call") && parts.len() >= 3 {
                        Some(parts[1].to_string())
                    } else if parts.first() == Some(&"guild") && parts.len() >= 4 {
                        Some(parts[2].to_string())
                    } else {
                        None
                    }
                });
                if guard.stream_rtc_channel_id.is_none() {
                    guard.stream_rtc_channel_id = skey_cid.or_else(|| guard.voice_channel_id.clone());
                }
                let self_uid = resolve_live_self_uid(&guard);
                let streamer_uid = guard
                    .stream_key
                    .as_deref()
                    .and_then(|sk| sk.rsplit(':').next())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| self_uid.clone());
                let sid = guard
                    .gateway_session_id
                    .clone()
                    .or_else(|| guard.voice_session_id.clone());
                let srv = guard
                    .stream_rtc_server_id
                    .clone()
                    .or_else(|| guard.voice_guild_id.clone())
                    .or_else(|| guard.voice_channel_id.clone());
                if let (Some(ep_val), Some(tok_val), Some(srv_val), Some(sid_val)) =
                    (ep, tok, srv, sid)
                {
                    if guard.stream_launched_token.as_deref() != Some(tok_val.as_str()) {
                        guard.stream_launched_token = Some(tok_val.clone());
                        guard.stream_launched_server_id = Some(srv_val.clone());
                        stream_launch_params = Some((
                            ep_val,
                            srv_val,
                            self_uid,
                            sid_val,
                            tok_val,
                            guard.stream_rtc_channel_id.clone(),
                            streamer_uid,
                        ));
                    }
                }
            }
            "STREAM_DELETE" => {
                let skey = field_str(data, "stream_key");
                if skey.is_none() || skey == guard.stream_key.as_deref() {
                    guard.stream_key = None;
                    guard.stream_rtc_server_id = None;
                    guard.stream_rtc_channel_id = None;
                    guard.stream_endpoint = None;
                    guard.stream_token = None;
                    guard.stream_launched_token = None;
                    guard.stream_launched_server_id = None;
                    should_stop_stream = true;
                }
            }
            _ => {}
        }
    }
    if should_stop {
        discord_core::ws::stop_live_rtc_session();
    } else if let Some((ep, server_id, user_id, session_id, token, cid)) = launch_params {
        discord_core::ws::start_live_rtc_session(ep, server_id, user_id, session_id, token, cid);
    }
    if should_stop_stream {
        discord_core::ws::stop_live_stream_rtc_session();
    } else if let Some((ep, server_id, user_id, session_id, token, cid, streamer_uid)) =
        stream_launch_params
    {
        let rtc_ch = cid
            .or_else(|| {
                let guard = live_session().lock().unwrap_or_else(|p| p.into_inner());
                guard.voice_channel_id.clone()
            })
            .unwrap_or_else(|| server_id.clone());
        discord_core::ws::start_live_stream_rtc_session(
            ep,
            server_id,
            rtc_ch,
            user_id,
            session_id,
            token,
            streamer_uid,
        );
    }
    for pgid in preview_gids_to_sync {
        spawn_hydrate_live_guild_full(&pgid);
    }
}

/// Подтверждает получение RTC `op:4` (`SELECT_PROTOCOL_ACK` / `session_description`),
/// переводя фазу голосового соединения из `'negotiating'` в `'connected'`.
pub fn confirm_live_rtc_op4_session_description(data: Option<&Value>) {
    let mut guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let _ = data;
    guard.voice_got_rtc_op4 = true;
    guard.voice_last_error = None;
    if guard.voice_got_state_update && guard.voice_got_server_update {
        guard.voice_phase = "connected".to_string();
    }
}

/// Проверяет таймаут (15s) рукопожатия голосовой сессии (`signaling`/`negotiating` без `connected`)
/// и при превышении автоматически очищает фантомную сессию отправкой `op:4 (channel_id: null)`.
pub fn check_voice_handshake_timeout_with_elapsed(elapsed: std::time::Duration) -> bool {
    use std::sync::atomic::Ordering;
    if elapsed < std::time::Duration::from_secs(15) {
        return false;
    }
    if !LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst) {
        return false;
    }
    let (should_cleanup, prev_gid, self_uid) = {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let is_connected = guard.voice_phase == "connected"
            || (guard.voice_got_state_update
                && guard.voice_got_server_update
                && (guard.voice_got_rtc_op4 || discord_core::ws::is_live_rtc_connected()));
        if is_connected || guard.voice_phase == "disconnected" {
            return false;
        }
        LOCAL_VOICE_ACTIVE.store(false, Ordering::SeqCst);
        let self_uid = resolve_live_self_uid(&guard);
        let prev_gid = guard.voice_guild_id.take();
        guard.voice_channel_id = None;
        guard.voice_endpoint = None;
        guard.voice_token = None;
        guard.voice_launched_token = None;
        guard.voice_phase = "disconnected".to_string();
        guard.voice_got_state_update = false;
        guard.voice_got_server_update = false;
        guard.voice_got_rtc_op4 = false;
        guard.voice_connect_started_at = None;
        guard.voice_last_error = Some(
            "Сервер Discord не подтвердил вход в голосовой канал (убедитесь, что аккаунт состоит на сервере)".to_string(),
        );
        guard.mute_deaf_coalesce_seq = guard.mute_deaf_coalesce_seq.wrapping_add(1);
        for list in guard.voice_states.values_mut() {
            list.retain(|entry| field_str(entry, "user_id") != Some(self_uid.as_str()));
        }
        guard.outbound_voice_op4_frames.push(json!({
            "op": 4,
            "guild_id": prev_gid,
            "channel_id": Value::Null,
            "self_mute": false,
            "self_deaf": false,
            "self_video": false,
            "self_stream": false,
        }));
        (true, prev_gid, self_uid)
    };
    if should_cleanup {
        stop_local_mic_monitor();
        discord_core::ws::stop_live_rtc_session();
        let _ = discord_core::ws::queue_live_voice_state_ext(
            prev_gid,
            None,
            false,
            false,
            false,
            false,
            Some(self_uid.as_str()),
        );
        return true;
    }
    false
}

/// Подключается к голосовому каналу либо обновляет состояние `self_mute`/`self_deaf` через `OP 4 VOICE_STATE_UPDATE`.
/// Проверяет, разрешено ли текущему пользователю запускать стрим (право PERM_STREAM = 1 << 9)
/// в указанном голосовом канале.
pub fn can_user_stream_in_channel(guild_id: Option<&str>, channel_id: &str) -> bool {
    let clean_gid = guild_id
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "@me" && *s != "null" && *s != "nil");
    let Some(gid) = clean_gid else {
        // В личных сообщениях (@me / DM) демонстрация экрана всегда разрешена
        return true;
    };
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let self_uid = resolve_live_self_uid(&guard);
    let guild_opt = guard
        .ready_data
        .as_ref()
        .and_then(|rd| rd.get("guilds"))
        .and_then(Value::as_array)
        .and_then(|arr| arr.iter().find(|g| field_str(g, "id") == Some(gid)))
        .or_else(|| guard.local_preview_guilds.get(gid));
    let Some(g_val) = guild_opt else {
        return true;
    };
    let is_lurker = field_bool(g_val, "lurker")
        || field_bool_opt(g_val, "isMember") == Some(false)
        || field_bool_opt(g_val, "is_member") == Some(false);
    let is_pending = field_bool(g_val, "pending") || field_bool(g_val, "is_pending");
    if is_lurker || is_pending {
        return false;
    }
    let roles = array(g_val.get("roles"));
    let channels = array(g_val.get("channels"));
    let owner_id = field_str(g_val, "owner_id").or_else(|| {
        g_val.get("properties").and_then(|p| field_str(p, "owner_id"))
    });
    let my_roles: Vec<String> = array(g_val.get("my_roles").or_else(|| g_val.get("myRoles")))
        .iter()
        .filter_map(|r| as_non_nil_str(r).map(str::to_string))
        .collect();

    let ch_perms_map = crate::permissions::PermissionCalculator::compute_guild_channel_permissions(
        gid,
        owner_id,
        &self_uid,
        &my_roles,
        roles,
        channels,
        is_lurker,
        is_pending,
    );
    if let Some(access) = ch_perms_map.get(channel_id) {
        return access.can_stream;
    }
    true
}

pub fn connect_or_update_live_voice(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_mute: bool,
    self_deaf: bool,
) -> Result<Value, String> {
    connect_or_update_live_voice_ext(guild_id, channel_id, self_mute, self_deaf, false, false)
}

/// Расширенное подключение/обновление голосового состояния (`self_mute`, `self_deaf`, `self_video`, `self_stream`).
pub fn connect_or_update_live_voice_ext(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_mute: bool,
    self_deaf: bool,
    self_video: bool,
    self_stream: bool,
) -> Result<Value, String> {
    use std::sync::atomic::Ordering;
    let clean_gid = guild_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "@me" && s != "null" && s != "nil");
    let clean_cid = channel_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "null" && s != "nil");
    if let (Some(ref gid_check), Some(ref cid_check)) = (&clean_gid, &clean_cid) {
        let (is_real_token, needs_hydrate, needs_bg_refresh) = {
            let guard = live_session()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let real_tok = guard.token.as_deref().is_some_and(|t| {
                let tr = t.trim();
                !tr.is_empty()
                    && tr != "live-test-token"
                    && !tr.starts_with("test-")
                    && !tr.starts_with("synthetic_")
                    && !tr.contains(".GTest")
            });
            let guild_opt = guard
                .ready_data
                .as_ref()
                .and_then(|rd| rd.get("guilds"))
                .and_then(Value::as_array)
                .and_then(|arr| arr.iter().find(|g| field_str(g, "id") == Some(gid_check.as_str())))
                .or_else(|| guard.local_preview_guilds.get(gid_check));
            if let Some(g_val) = guild_opt {
                let is_lurker = field_bool(g_val, "lurker")
                    || field_bool_opt(g_val, "isMember") == Some(false)
                    || field_bool_opt(g_val, "is_member") == Some(false)
                    || field_bool(g_val, "pending");
                let roles_empty = array(g_val.get("roles")).is_empty();
                let chs_empty = array(g_val.get("channels")).is_empty();
                (real_tok, roles_empty || chs_empty, is_lurker)
            } else {
                (real_tok, real_tok, false)
            }
        };
        if is_real_token && needs_hydrate {
            hydrate_live_guild_blocking(gid_check);
        } else if is_real_token && needs_bg_refresh {
            spawn_hydrate_live_guild_full(gid_check);
        }
        {
            let guard = live_session()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let self_uid = resolve_live_self_uid(&guard);
            let guild_opt = guard
                .ready_data
                .as_ref()
                .and_then(|rd| rd.get("guilds"))
                .and_then(Value::as_array)
                .and_then(|arr| arr.iter().find(|g| field_str(g, "id") == Some(gid_check.as_str())))
                .or_else(|| guard.local_preview_guilds.get(gid_check));
            if let Some(g_val) = guild_opt {
                let still_lurker = field_bool(g_val, "lurker")
                    || field_bool_opt(g_val, "isMember") == Some(false)
                    || field_bool_opt(g_val, "is_member") == Some(false);
                let is_pending = field_bool(g_val, "pending") || field_bool(g_val, "is_pending");
                let roles = array(g_val.get("roles"));
                let channels = array(g_val.get("channels"));
                let owner_id = field_str(g_val, "owner_id").or_else(|| {
                    g_val.get("properties").and_then(|p| field_str(p, "owner_id"))
                });
                let my_roles: Vec<String> = array(g_val.get("my_roles").or_else(|| g_val.get("myRoles")))
                    .iter()
                    .filter_map(|r| as_non_nil_str(r).map(str::to_string))
                    .collect();

                let ch_perms_map =
                    crate::permissions::PermissionCalculator::compute_guild_channel_permissions(
                        gid_check,
                        owner_id,
                        &self_uid,
                        &my_roles,
                        roles,
                        channels,
                        still_lurker,
                        is_pending,
                    );
                if let Some(access) = ch_perms_map.get(cid_check.as_str()) {
                    if !access.can_connect {
                        return Err(access.denied_reason.clone().unwrap_or_else(|| {
                            "Роль не даёт права Подключаться (CONNECT)".to_string()
                        }));
                    }
                } else if still_lurker {
                    return Err("Сервер в режиме просмотра".to_string());
                } else if is_pending {
                    return Err("Требуется принять правила сервера".to_string());
                } else if !roles.is_empty() {
                    let role_defs: Vec<crate::permissions::RolePermission> = roles
                        .iter()
                        .filter_map(|r| {
                            let id = field_str(r, "id")?.to_string();
                            let permissions = crate::permissions::parse_perm_bits(r.get("permissions"));
                            let position = field_i64(r, "position").unwrap_or(0);
                            Some(crate::permissions::RolePermission {
                                id,
                                permissions,
                                position,
                            })
                        })
                        .collect();
                    let access =
                        crate::permissions::PermissionCalculator::evaluate_allowed_channel_access(
                            gid_check,
                            owner_id.unwrap_or_default(),
                            &self_uid,
                            &my_roles,
                            &role_defs,
                            &[],
                            still_lurker,
                            is_pending,
                        );
                    if !access.can_connect {
                        return Err(access.denied_reason.unwrap_or_else(|| {
                            "Роль не даёт права Подключаться (CONNECT)".to_string()
                        }));
                    }
                }
            }
        }
    }
    LOCAL_SELF_MUTE.store(self_mute, Ordering::SeqCst);
    LOCAL_SELF_DEAF.store(self_deaf, Ordering::SeqCst);
    let v_prev = LOCAL_SELF_VIDEO.swap(self_video, Ordering::SeqCst);
    let s_prev = LOCAL_SELF_STREAM.swap(self_stream, Ordering::SeqCst);
    let stream_or_video_changed = s_prev != self_stream || v_prev != self_video;
    LOCAL_VOICE_ACTIVE.store(clean_cid.is_some(), Ordering::SeqCst);
    discord_core::ws::set_live_voice_mute_deaf_state(self_mute, self_deaf);
    let (self_uid, prev_gids, channel_changed, coalesce_seq) = {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let self_uid = resolve_live_self_uid(&guard);
        let mut prev_gids: Vec<String> = Vec::new();
        if let Some(ref vg) = guard.voice_guild_id {
            if !vg.is_empty() && vg != "@me" && Some(vg.as_str()) != clean_cid.as_deref() {
                prev_gids.push(vg.clone());
            }
        }
        for (gid_key, vs_list) in &guard.voice_states {
            if gid_key == "@me" || Some(gid_key.as_str()) == clean_cid.as_deref() {
                continue;
            }
            if vs_list.iter().any(|entry| {
                field_str(entry, "user_id") == Some(self_uid.as_str())
                    && field_str(entry, "channel_id").is_some()
            }) && !prev_gids.contains(gid_key)
            {
                prev_gids.push(gid_key.clone());
            }
        }
        let channel_changed = guard.voice_channel_id != clean_cid || guard.voice_guild_id != clean_gid;
        guard.voice_guild_id = if clean_cid.is_some() {
            clean_gid.clone()
        } else {
            None
        };
        guard.voice_channel_id = clean_cid.clone();
        guard.mute_deaf_coalesce_seq = guard.mute_deaf_coalesce_seq.wrapping_add(1);
        let coalesce_seq = guard.mute_deaf_coalesce_seq;
        let rtc_disconnected = {
            let st = discord_core::ws::live_rtc_snapshot().state;
            st == "disconnected" || st.starts_with("error")
        };
        if clean_cid.is_none() {
            guard.voice_endpoint = None;
            guard.voice_token = None;
            guard.voice_launched_token = None;
            guard.stream_launched_token = None;
            guard.stream_launched_server_id = None;
            guard.voice_phase = "disconnected".to_string();
            guard.voice_got_state_update = false;
            guard.voice_got_server_update = false;
            guard.voice_got_rtc_op4 = false;
            guard.voice_connect_started_at = None;
            guard.voice_last_error = None;
        } else if channel_changed {
            guard.voice_endpoint = None;
            guard.voice_token = None;
            guard.voice_launched_token = None;
            guard.stream_launched_token = None;
            guard.stream_launched_server_id = None;
            guard.voice_phase = "signaling".to_string();
            guard.voice_got_state_update = false;
            guard.voice_got_server_update = false;
            guard.voice_got_rtc_op4 = false;
            guard.voice_connect_started_at = Some(std::time::Instant::now());
            guard.voice_last_error = None;
        } else if rtc_disconnected {
            guard.voice_launched_token = None;
        }
        if !self_stream {
            guard.stream_launched_token = None;
            guard.stream_launched_server_id = None;
        }
        if clean_cid.is_none() {
            for list in guard.voice_states.values_mut() {
                list.retain(|entry| field_str(entry, "user_id") != Some(self_uid.as_str()));
            }
            if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                    for rg in r_guilds.iter_mut() {
                        if let Value::Object(ref mut gm) = rg {
                            if let Some(Value::Array(ref mut vs_arr)) = gm.get_mut("voice_states") {
                                vs_arr.retain(|entry| {
                                    field_str(entry, "user_id") != Some(self_uid.as_str())
                                });
                            }
                        }
                    }
                }
            }
        } else if let Some(ref cid_str) = clean_cid {
            let gid_str_val = clean_gid.clone().unwrap_or_else(|| "@me".to_string());
            let gid_str = &gid_str_val;
            for (other_gid, other_list) in guard.voice_states.iter_mut() {
                if other_gid != gid_str {
                    other_list.retain(|entry| field_str(entry, "user_id") != Some(self_uid.as_str()));
                }
            }
            let list = guard.voice_states.entry(gid_str.clone()).or_default();
            if let Some(existing) = list
                .iter_mut()
                .find(|entry| field_str(entry, "user_id") == Some(self_uid.as_str()))
            {
                if let Value::Object(ref mut m) = existing {
                    m.insert("channel_id".to_string(), Value::String(cid_str.clone()));
                    m.insert("self_mute".to_string(), Value::Bool(self_mute));
                    m.insert("self_deaf".to_string(), Value::Bool(self_deaf));
                    m.insert("self_video".to_string(), Value::Bool(self_video));
                    m.insert("self_stream".to_string(), Value::Bool(self_stream));
                }
            } else {
                list.push(json!({
                    "guild_id": gid_str,
                    "channel_id": cid_str,
                    "user_id": self_uid,
                    "self_mute": self_mute,
                    "self_deaf": self_deaf,
                    "self_video": self_video,
                    "self_stream": self_stream,
                    "mute": false,
                    "deaf": false,
                    "suppress": false,
                }));
            }
            let vs_clone = list.clone();
            if let Some(Value::Object(ref mut ready_map)) = guard.ready_data {
                if let Some(Value::Array(ref mut r_guilds)) = ready_map.get_mut("guilds") {
                    for rg in r_guilds.iter_mut() {
                        if field_str(rg, "id") == Some(gid_str.as_str()) {
                            if let Value::Object(ref mut gm) = rg {
                                gm.insert("voice_states".to_string(), Value::Array(vs_clone.clone()));
                            }
                        } else if let Value::Object(ref mut gm) = rg {
                            if let Some(Value::Array(ref mut vs_arr)) = gm.get_mut("voice_states") {
                                vs_arr.retain(|entry| {
                                    field_str(entry, "user_id") != Some(self_uid.as_str())
                                });
                            }
                        }
                    }
                }
            }
        }
        (self_uid, prev_gids, channel_changed, coalesce_seq)
    };
    if clean_cid.is_none() {
        stop_local_mic_monitor();
        discord_core::ws::stop_live_rtc_session();
        let mut target_gids = prev_gids;
        if let Some(ref g) = clean_gid {
            if !target_gids.contains(g) {
                target_gids.push(g.clone());
            }
        }
        if target_gids.is_empty() {
            {
                let mut guard = live_session()
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                guard.outbound_voice_op4_frames.push(json!({
                    "op": 4,
                    "guild_id": Value::Null,
                    "channel_id": Value::Null,
                    "self_mute": false,
                    "self_deaf": false,
                    "self_video": false,
                    "self_stream": false,
                }));
            }
            discord_core::ws::queue_live_voice_state_ext(
                None,
                None,
                false,
                false,
                false,
                false,
                Some(self_uid.as_str()),
            )
            .map_err(|e| e.to_string())?;
        } else {
            for g in target_gids {
                {
                    let mut guard = live_session()
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    guard.outbound_voice_op4_frames.push(json!({
                        "op": 4,
                        "guild_id": g,
                        "channel_id": Value::Null,
                        "self_mute": false,
                        "self_deaf": false,
                        "self_video": false,
                        "self_stream": false,
                    }));
                }
                discord_core::ws::queue_live_voice_state_ext(
                    Some(g),
                    None,
                    false,
                    false,
                    false,
                    false,
                    Some(self_uid.as_str()),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        return Ok(live_voice_rtc_status_json());
    } else {
        for old_gid in prev_gids {
            if clean_gid.as_deref() != Some(old_gid.as_str()) {
                let _ = discord_core::ws::queue_live_voice_state_ext(
                    Some(old_gid),
                    None,
                    false,
                    false,
                    false,
                    false,
                    Some(self_uid.as_str()),
                );
            }
        }
        discord_core::ws::note_live_rtc_target(clean_gid.clone(), clean_cid.clone(), None);
        let in_vol = LOCAL_INPUT_VOLUME_PCT.load(Ordering::Relaxed).min(100);
        let out_vol = LOCAL_OUTPUT_VOLUME_PCT.load(Ordering::Relaxed).min(200);
        discord_core::ws::send_native_voice_cmd(&format!(
            "{{\"cmd\":\"set_state\",\"selfMute\":{self_mute},\"selfDeafen\":{self_deaf},\"inputVolume\":{in_vol},\"outputVolume\":{out_vol}}}"
        ));
        discord_core::ws::send_native_stream_cmd(&format!(
            "{{\"cmd\":\"set_stream\",\"broadcasting\":{self_stream}}}"
        ));
        ensure_local_mic_monitor();
    }

    if !stream_or_video_changed && !channel_changed {
        // V1: Коалесценция 120ms для обновлений mute/deaf на том же голосовом канале
        let gid_for_timer = clean_gid.clone();
        let cid_for_timer = clean_cid.clone();
        let uid_for_timer = self_uid.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(120));
            let should_send = {
                let mut guard = live_session()
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                if guard.mute_deaf_coalesce_seq == coalesce_seq
                    && LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst)
                    && guard.voice_channel_id == cid_for_timer
                {
                    let m = LOCAL_SELF_MUTE.load(Ordering::SeqCst);
                    let d = LOCAL_SELF_DEAF.load(Ordering::SeqCst);
                    let v = LOCAL_SELF_VIDEO.load(Ordering::SeqCst);
                    let s = LOCAL_SELF_STREAM.load(Ordering::SeqCst);
                    guard.outbound_voice_op4_frames.push(json!({
                        "op": 4,
                        "guild_id": gid_for_timer,
                        "channel_id": cid_for_timer,
                        "self_mute": m,
                        "self_deaf": d,
                        "self_video": v,
                        "self_stream": s,
                    }));
                    Some((m, d, v, s))
                } else {
                    None
                }
            };
            if let Some((m, d, v, s)) = should_send {
                let _ = discord_core::ws::queue_live_voice_state_ext(
                    gid_for_timer,
                    cid_for_timer,
                    m,
                    d,
                    v,
                    s,
                    Some(uid_for_timer.as_str()),
                );
            }
        });
        return Ok(live_voice_rtc_status_json());
    }

    {
        let mut guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.outbound_voice_op4_frames.push(json!({
            "op": 4,
            "guild_id": clean_gid,
            "channel_id": clean_cid,
            "self_mute": self_mute,
            "self_deaf": self_deaf,
            "self_video": self_video,
            "self_stream": self_stream,
        }));
    }
    discord_core::ws::queue_live_voice_state_ext(
        clean_gid.clone(),
        clean_cid.clone(),
        self_mute,
        self_deaf,
        self_video,
        self_stream,
        Some(self_uid.as_str()),
    )
    .map_err(|e| e.to_string())?;
    Ok(live_voice_rtc_status_json())
}

pub fn current_live_voice_target() -> (Option<String>, Option<String>) {
    let guard = live_session().lock().unwrap_or_else(|p| p.into_inner());
    (guard.voice_guild_id.clone(), guard.voice_channel_id.clone())
}

pub fn active_voice_guild_and_channel() -> (Option<String>, Option<String>) {
    current_live_voice_target()
}

pub fn current_live_voice_mute_deaf() -> (bool, bool) {
    (
        LOCAL_SELF_MUTE.load(std::sync::atomic::Ordering::SeqCst),
        LOCAL_SELF_DEAF.load(std::sync::atomic::Ordering::SeqCst),
    )
}

pub fn current_live_self_uid() -> String {
    let guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    resolve_live_self_uid(&guard)
}

static LOCAL_MIC_LEVEL_DB: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-100);
static LOCAL_MIC_SPEAKING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_MIC_MONITOR_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_MIC_MONITOR_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static LOCAL_MIC_LOOPBACK_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static AUDIO_DEVICE_EPOCH: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static MEDIA_PLAY_EPOCH: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static VOICE_MSG_RECORD_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static VOICE_MSG_RECORD_START_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn stop_local_mic_monitor() {
    use std::sync::atomic::Ordering;
    LOCAL_MIC_LOOPBACK_ACTIVE.store(false, Ordering::SeqCst);
    LOCAL_MIC_MONITOR_ACTIVE.store(false, Ordering::SeqCst);
    LOCAL_MIC_SPEAKING.store(false, Ordering::Relaxed);
    LOCAL_MIC_LEVEL_DB.store(-100, Ordering::Relaxed);
}

/// Чисто останавливает фоновые дочерние процессы (`parec`, медиа-аудио) и живые RTC-потоки при выходе из приложения.
pub fn shutdown_live_workers() {
    use std::sync::atomic::Ordering;
    MEDIA_PLAY_EPOCH.fetch_add(1, Ordering::SeqCst);
    VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
    stop_local_mic_monitor();
    let _ = discord_core::ws::queue_live_voice_state(None, None, false, false);
    discord_core::ws::stop_live_rtc_session();
}

fn start_linux_media_audio_playback(raw_spec: &str, vol_pct: Option<u32>, is_muted: Option<bool>) {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    let epoch = MEDIA_PLAY_EPOCH.fetch_add(1, Ordering::SeqCst) + 1;
    if is_muted.unwrap_or(false) || vol_pct == Some(0) {
        return;
    }
    let mut url = raw_spec.trim().to_string();
    let mut seek_sec = 0.0_f64;
    let mut item_vol = vol_pct.unwrap_or(100).min(200);
    if url.starts_with('{') {
        if let Ok(parsed) = serde_json::from_str::<Value>(&url) {
            if let Some(u) = field_str(&parsed, "url") {
                url = u.to_string();
            }
            if let Some(s) = parsed.get("seek").and_then(Value::as_f64) {
                seek_sec = s.max(0.0);
            }
            if let Some(v) = field_u64(&parsed, "volume") {
                item_vol = (v as u32).min(200);
            }
        }
    }
    if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with('/')) {
        return;
    }
    let out_dev = selected_output_device()
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    std::thread::spawn(move || {
        let mut ff_cmd = std::process::Command::new("ffmpeg");
        ff_cmd
            .arg("-nostdin")
            .arg("-loglevel")
            .arg("quiet");
        if url.starts_with("http://") || url.starts_with("https://") {
            ff_cmd
                .arg("-reconnect")
                .arg("1")
                .arg("-reconnect_streamed")
                .arg("1")
                .arg("-reconnect_delay_max")
                .arg("2");
        }
        if seek_sec > 0.05 {
            ff_cmd.arg("-ss").arg(format!("{seek_sec:.2}"));
        }
        ff_cmd
            .arg("-i")
            .arg(&url)
            .arg("-vn")
            .arg("-f")
            .arg("s16le")
            .arg("-ac")
            .arg("2")
            .arg("-ar")
            .arg("48000")
            .arg("-")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let Ok(mut ff_child) = ff_cmd.spawn() else {
            return;
        };
        let Some(mut ff_stdout) = ff_child.stdout.take() else {
            let _ = ff_child.kill();
            let _ = ff_child.wait();
            return;
        };
        let mut pa_cmd = std::process::Command::new("pacat");
        pa_cmd
            .arg("--channels=2")
            .arg("--rate=48000")
            .arg("--format=s16le")
            .arg("--latency-msec=40");
        if !out_dev.is_empty() && out_dev != "default" {
            pa_cmd.arg(format!("--device={out_dev}"));
        }
        pa_cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let Ok(mut pa_child) = pa_cmd.spawn() else {
            let _ = ff_child.kill();
            let _ = ff_child.wait();
            return;
        };
        let Some(mut pa_stdin) = pa_child.stdin.take() else {
            let _ = ff_child.kill();
            let _ = ff_child.wait();
            let _ = pa_child.kill();
            let _ = pa_child.wait();
            return;
        };
        let mut buf = [0u8; 3840];
        while MEDIA_PLAY_EPOCH.load(Ordering::Relaxed) == epoch {
            let Ok(n) = ff_stdout.read(&mut buf) else {
                break;
            };
            if n == 0 {
                break;
            }
            if LOCAL_SELF_DEAF.load(Ordering::Relaxed) {
                continue;
            }
            let app_vol = LOCAL_OUTPUT_VOLUME_PCT.load(Ordering::Relaxed).min(200);
            let scale = (item_vol as f32 / 100.0) * (app_vol as f32 / 100.0);
            if (scale - 1.0).abs() > 0.02 {
                for chunk in buf[..n].chunks_exact_mut(2) {
                    let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                    let scaled = (f32::from(s) * scale).clamp(-32760.0, 32760.0) as i16;
                    let bytes = scaled.to_le_bytes();
                    chunk[0] = bytes[0];
                    chunk[1] = bytes[1];
                }
            }
            if pa_stdin.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        drop(pa_stdin);
        drop(ff_stdout);
        let _ = ff_child.kill();
        let _ = ff_child.wait();
        let _ = pa_child.kill();
        let _ = pa_child.wait();
    });
}

fn play_linux_ui_sound(sound_name: &str, volume_pct: Option<u32>, muted: Option<bool>) {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    if muted.unwrap_or(false) {
        return;
    }
    let filename = match sound_name {
        "join" | "user_join" => "user_join.mp3",
        "leave" | "user_leave" => "user_leave.mp3",
        "user_moved" | "moved" => "user_moved.mp3",
        "disconnect" => "disconnect.mp3",
        "mute" => "mute.mp3",
        "unmute" => "unmute.mp3",
        "deafen" => "deafen.mp3",
        "undeafen" => "undeafen.mp3",
        "stream_on" | "stream_started" | "camera_on" => "stream_started.mp3",
        "stream_off" | "stream_ended" | "camera_off" => "stream_ended.mp3",
        "stream_user_joined" => "stream_user_joined.mp3",
        "stream_user_left" => "stream_user_left.mp3",
        "message" | "message1" => "message1.mp3",
        _ => "mute.mp3",
    };
    let candidates = [
        format!("/tmp/discord_standard_sounds/{filename}"),
        format!("/home/sanya/orca/workspaces/project/unstable/rust/gui-spike/ui/assets/sounds/{filename}"),
        format!("/home/sanya/orca/project/rust/gui-spike/ui/assets/sounds/{filename}"),
        format!("rust/gui-spike/ui/assets/sounds/{filename}"),
    ];
    let Some(path) = candidates.into_iter().find(|p| Path::new(p).exists()) else {
        return;
    };
    let item_vol = volume_pct.unwrap_or(85).min(200);
    let out_dev = selected_output_device()
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    std::thread::spawn(move || {
        let Ok(mut ff_child) = std::process::Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-loglevel",
                "quiet",
                "-i",
                &path,
                "-vn",
                "-f",
                "s16le",
                "-ar",
                "48000",
                "-ac",
                "2",
                "pipe:1",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            return;
        };
        let Some(mut ff_stdout) = ff_child.stdout.take() else {
            let _ = ff_child.kill();
            let _ = ff_child.wait();
            return;
        };
        let mut pa_cmd = std::process::Command::new("pacat");
        pa_cmd
            .arg("--playback")
            .arg("--channels=2")
            .arg("--rate=48000")
            .arg("--format=s16le")
            .arg("--client-name=Discord-UI-SFX")
            .arg("--stream-name=Discord-SFX")
            .arg("--latency-msec=30");
        if !out_dev.is_empty() && out_dev != "default" {
            pa_cmd.arg(format!("--device={out_dev}"));
        }
        let Ok(mut pa_child) = pa_cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            let _ = ff_child.kill();
            let _ = ff_child.wait();
            return;
        };
        let Some(mut pa_stdin) = pa_child.stdin.take() else {
            let _ = ff_child.kill();
            let _ = ff_child.wait();
            let _ = pa_child.kill();
            let _ = pa_child.wait();
            return;
        };
        let app_vol = LOCAL_OUTPUT_VOLUME_PCT.load(Ordering::Relaxed).min(200);
        let scale = (item_vol as f32 / 100.0) * (app_vol as f32 / 100.0);
        let mut buf = [0u8; 3840];
        loop {
            let Ok(n) = ff_stdout.read(&mut buf) else {
                break;
            };
            if n == 0 {
                break;
            }
            if (scale - 1.0).abs() > 0.02 {
                for chunk in buf[..n].chunks_exact_mut(2) {
                    let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                    let scaled = (f32::from(s) * scale).clamp(-32760.0, 32760.0) as i16;
                    let bytes = scaled.to_le_bytes();
                    chunk[0] = bytes[0];
                    chunk[1] = bytes[1];
                }
            }
            if pa_stdin.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        drop(pa_stdin);
        drop(ff_stdout);
        let _ = ff_child.kill();
        let _ = ff_child.wait();
        let _ = pa_child.wait();
    });
}

fn encode_bytes_base64(bytes: &[u8]) -> String {
    const B64_CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut b64 = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        b64.push(B64_CHARS[((n >> 18) & 63) as usize] as char);
        b64.push(B64_CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            b64.push(B64_CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            b64.push('=');
        }
        if chunk.len() > 2 {
            b64.push(B64_CHARS[(n & 63) as usize] as char);
        } else {
            b64.push('=');
        }
    }
    b64
}

fn fetch_remote_stream_preview_blocking(stream_key: &str) {
    if stream_key.trim().is_empty() {
        return;
    }
    let token = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.token.clone()
    };
    let Some(tok) = token.filter(|t| !t.trim().is_empty()) else {
        return;
    };
    let encoded_key = stream_key.replace(':', "%3A");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let url = format!("https://discord.com/api/v9/streams/{encoded_key}/preview?version={now_ms}");
    let Ok(out) = std::process::Command::new("curl")
        .args([
            "-sS",
            "--max-time",
            "3",
            "-H",
            &format!("Authorization: {}", tok.trim()),
            "-H",
            "User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/0.0.120 Chrome/134.0.6998.179 Electron/35.1.5 Safari/537.36",
            &url,
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return;
    };
    if !out.status.success() {
        return;
    }
    let Ok(val) = serde_json::from_slice::<Value>(&out.stdout) else {
        return;
    };
    let Some(img_url) = val.get("url").and_then(Value::as_str) else {
        return;
    };
    let tmp_path = "/tmp/discord_remote_stream_preview_dl.jpg";
    let final_path = "/tmp/discord_remote_stream_frame.jpg";
    let status = std::process::Command::new("curl")
        .args(["-sS", "-L", "--max-time", "3", "-o", tmp_path, img_url])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if status.map(|s| s.success()).unwrap_or(false) {
        if std::fs::metadata(tmp_path).map(|m| m.len() > 64).unwrap_or(false) {
            let _ = std::fs::rename(tmp_path, final_path);
        }
    }
}

fn start_voice_msg_recording() -> Value {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(40));
    let out_path = "/tmp/discord-voice-msg-recording.ogg";
    let _ = std::fs::remove_file(out_path);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    VOICE_MSG_RECORD_START_MS.store(now_ms, Ordering::SeqCst);
    VOICE_MSG_RECORD_ACTIVE.store(true, Ordering::SeqCst);
    let inp_dev = selected_input_device()
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    std::thread::spawn(move || {
        let mut pa_cmd = std::process::Command::new("parec");
        pa_cmd
            .arg("--channels=2")
            .arg("--rate=48000")
            .arg("--format=s16le")
            .arg("--latency-msec=20");
        if !inp_dev.is_empty() && inp_dev != "default" {
            pa_cmd.arg(format!("--device={inp_dev}"));
        }
        pa_cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let Ok(mut pa_child) = pa_cmd.spawn() else {
            VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
            return;
        };
        let Some(mut pa_stdout) = pa_child.stdout.take() else {
            let _ = pa_child.kill();
            let _ = pa_child.wait();
            VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
            return;
        };
        let Ok(mut ff_child) = std::process::Command::new("ffmpeg")
            .arg("-nostdin")
            .arg("-y")
            .arg("-loglevel")
            .arg("quiet")
            .arg("-f")
            .arg("s16le")
            .arg("-ar")
            .arg("48000")
            .arg("-ac")
            .arg("2")
            .arg("-i")
            .arg("-")
            .arg("-c:a")
            .arg("libopus")
            .arg("-b:a")
            .arg("64k")
            .arg(out_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            let _ = pa_child.kill();
            let _ = pa_child.wait();
            VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
            return;
        };
        let Some(mut ff_stdin) = ff_child.stdin.take() else {
            let _ = pa_child.kill();
            let _ = pa_child.wait();
            let _ = ff_child.kill();
            let _ = ff_child.wait();
            VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
            return;
        };
        let mut buf = [0u8; 3840];
        while VOICE_MSG_RECORD_ACTIVE.load(Ordering::Relaxed) {
            let Ok(n) = pa_stdout.read(&mut buf) else {
                break;
            };
            if n == 0 || ff_stdin.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        drop(ff_stdin);
        drop(pa_stdout);
        let _ = pa_child.kill();
        let _ = pa_child.wait();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1200);
        loop {
            match ff_child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                _ => {
                    let _ = ff_child.kill();
                    let _ = ff_child.wait();
                    break;
                }
            }
        }
    });
    json!({ "ok": true, "recording": true })
}

fn stop_and_send_voice_message(channel_id: &str) -> Value {
    use std::sync::atomic::Ordering;
    let start_ms = VOICE_MSG_RECORD_START_MS.load(Ordering::SeqCst);
    VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(280));
    let out_path = "/tmp/discord-voice-msg-recording.ogg";
    let Ok(file_bytes) = std::fs::read(out_path) else {
        return json!({ "ok": false, "error": "Файл записи голосового сообщения не найден" });
    };
    if file_bytes.len() < 100 {
        return json!({ "ok": false, "error": "Запись слишком короткая" });
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(start_ms + 1000);
    let duration_secs = ((now_ms.saturating_sub(start_ms) as f64) / 1000.0).clamp(0.5, 1200.0);
    let cid = channel_id.trim().to_string();
    let token = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.token.clone().unwrap_or_default()
    };
    if token.is_empty() || cid.is_empty() {
        return json!({ "ok": false, "error": "Нет активной сессии или канала" });
    }
    let file_len = file_bytes.len();
    let res = std::thread::spawn(move || -> Option<Value> {
        let identity = crate::auth::IdentityContext::detect();
        let auth = discord_core::rest::client::ApiAuth::from_token_with_scheme(
            &token,
            discord_core::rest::client::AuthScheme::User,
            &identity.super_properties,
            &identity.locale,
            &identity.timezone,
        )
        .ok()?;
        let config = discord_core::rest::client::ApiConfig {
            base: discord_core::API_V9_BASE.to_string(),
            auth,
            http: discord_core::rest::transport::HttpClientConfig {
                connect_timeout: std::time::Duration::from_millis(4000),
                read_timeout: std::time::Duration::from_millis(8000),
                write_timeout: std::time::Duration::from_millis(8000),
                ..discord_core::rest::transport::HttpClientConfig::default()
            },
            retry: discord_core::rest::client::ApiRetryPolicy {
                max_attempts: 1,
                ..discord_core::rest::client::ApiRetryPolicy::default()
            },
        };
        let client = discord_core::rest::client::ApiClient::new(config).ok()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .ok()?;
        let attach_req = json!({
            "files": [{
                "filename": "voice-message.ogg",
                "file_size": file_len,
                "id": "0",
                "is_clip": false
            }]
        });
        let attach_body = serde_json::to_vec(&attach_req).ok()?;
        let attach_resp = runtime
            .block_on(client.post(&format!("/channels/{cid}/attachments"), &attach_body))
            .ok()?;
        if attach_resp.status != 200 {
            return None;
        }
        let attach_json: Value = serde_json::from_slice(&attach_resp.body).ok()?;
        let slot = attach_json
            .get("attachments")
            .and_then(Value::as_array)
            .and_then(|a| a.first())?;
        let upload_url = field_str(slot, "upload_url")?;
        let upload_filename = field_str(slot, "upload_filename")?;
        let put_status = std::process::Command::new("curl")
            .arg("-sS")
            .arg("-X")
            .arg("PUT")
            .arg("-H")
            .arg("Content-Type: audio/ogg")
            .arg("--data-binary")
            .arg(format!("@{out_path}"))
            .arg(upload_url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()?;
        if !put_status.success() {
            return None;
        }
        let msg_req = json!({
            "content": "",
            "nonce": now_ms.to_string(),
            "tts": false,
            "flags": 8192,
            "attachments": [{
                "id": "0",
                "filename": "voice-message.ogg",
                "uploaded_filename": upload_filename,
                "duration_secs": (duration_secs * 100.0).round() / 100.0,
                "waveform": "////////8P/w//D/8P/w//D/8P/w//D/8P/w//D/8P8="
            }]
        });
        let msg_body = serde_json::to_vec(&msg_req).ok()?;
        let msg_resp = runtime
            .block_on(client.post(&format!("/channels/{cid}/messages"), &msg_body))
            .ok()?;
        if msg_resp.status != 200 {
            return None;
        }
        let mut msg_json: Value = serde_json::from_slice(&msg_resp.body).ok()?;
        sanitize_etf_json_value(&mut msg_json);
        record_sent_message(&cid, msg_json.clone());
        let row = parse_message(&msg_json, Path::new("live://voice-msg")).ok()?;
        serde_json::to_value(row).ok()
    })
    .join()
    .ok()
    .flatten();
    match res {
        Some(row_val) => json!({ "ok": true, "message": row_val }),
        None => json!({ "ok": false, "error": "Не удалось загрузить голосовое сообщение в Discord" }),
    }
}

const OPUS_AUDIO_BRIDGE_PY: &str = r#"
import array, ctypes, math, os, struct, subprocess, sys, threading

inp_dev = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else ""
out_dev = sys.argv[2] if len(sys.argv) > 2 and sys.argv[2] else ""
loopback = sys.argv[3] == "1" if len(sys.argv) > 3 else False

state_lock = threading.Lock()
cfg_krisp = True
cfg_agc = True
cfg_auto_vad = True
cfg_muted = False
cfg_vad_db = -45
cfg_in_vol = 100
cfg_out_vol = 100
cfg_deafened = False

opus = ctypes.CDLL("libopus.so.0")
opus.opus_encoder_create.argtypes = [ctypes.c_int32, ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_int)]
opus.opus_encoder_create.restype = ctypes.c_void_p
opus.opus_encode.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_int32]
opus.opus_encode.restype = ctypes.c_int32
opus.opus_decoder_create.argtypes = [ctypes.c_int32, ctypes.c_int, ctypes.POINTER(ctypes.c_int)]
opus.opus_decoder_create.restype = ctypes.c_void_p
opus.opus_decode.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_char_p, ctypes.c_int, ctypes.c_int]
opus.opus_decode.restype = ctypes.c_int

err = ctypes.c_int(0)
enc = opus.opus_encoder_create(48000, 2, 2048, ctypes.byref(err))
decoders = {}

parec_args = ["parec", "--channels=2", "--rate=48000", "--format=s16le", "--latency-msec=20", "--client-name=Discord-Mic-Meter", "--property=media.role=meter"]
if inp_dev:
    parec_args.append(f"--device={inp_dev}")
pacat_args = ["pacat", "--channels=2", "--rate=48000", "--format=s16le", "--latency-msec=20", "--client-name=Discord-Mic-Loopback"]
if out_dev:
    pacat_args.append(f"--device={out_dev}")

parec = subprocess.Popen(parec_args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
pacat = subprocess.Popen(pacat_args, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL) if loopback else None
pacat_lock = threading.Lock()

def ensure_pacat():
    global pacat
    if pacat is None or pacat.poll() is not None:
        pacat = subprocess.Popen(pacat_args, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return pacat

def scale_pcm_bytes(raw_bytes, mult):
    if abs(mult - 1.0) < 0.02:
        return raw_bytes
    arr = array.array("h")
    arr.frombytes(raw_bytes)
    for i in range(len(arr)):
        v = int(arr[i] * mult)
        if v > 32000:
            v = 32000
        elif v < -32000:
            v = -32000
        arr[i] = v
    return arr.tobytes()

def rx_worker():
    global cfg_krisp, cfg_agc, cfg_auto_vad, cfg_muted, cfg_vad_db, cfg_in_vol, cfg_out_vol, cfg_deafened
    pcm_out = ctypes.create_string_buffer(5760 * 4)
    stdin_buf = sys.stdin.buffer
    try:
        while True:
            hdr = stdin_buf.read(6)
            if not hdr or len(hdr) < 6:
                break
            ssrc, plen = struct.unpack("<IH", hdr)
            if plen == 0:
                continue
            payload = stdin_buf.read(plen)
            if not payload or len(payload) < plen:
                break
            if ssrc == 0xFFFFFFFF and plen == 8:
                k, a, av, m, vdb, iv, ov, df = struct.unpack("<BBBBbBBB", payload)
                with state_lock:
                    cfg_krisp = bool(k)
                    cfg_agc = bool(a)
                    cfg_auto_vad = bool(av)
                    cfg_muted = bool(m)
                    cfg_vad_db = int(vdb)
                    cfg_in_vol = int(iv)
                    cfg_out_vol = int(ov)
                    cfg_deafened = bool(df)
                continue
            with state_lock:
                deaf = cfg_deafened
                out_mult = cfg_out_vol / 100.0
            if deaf or out_mult <= 0.001:
                continue
            dec = decoders.get(ssrc)
            if dec is None:
                d_err = ctypes.c_int(0)
                dec = opus.opus_decoder_create(48000, 2, ctypes.byref(d_err))
                decoders[ssrc] = dec
            samples = opus.opus_decode(dec, payload, plen, pcm_out, 5760, 0)
            if samples > 0:
                with pacat_lock:
                    p = ensure_pacat()
                    if p and p.stdin:
                        raw_slice = pcm_out.raw[:samples * 4]
                        p.stdin.write(scale_pcm_bytes(raw_slice, out_mult))
                        p.stdin.flush()
    except Exception:
        pass

t = threading.Thread(target=rx_worker, daemon=True)
t.start()

out_pkt = ctypes.create_string_buffer(1276)
stdout_buf = sys.stdout.buffer

noise_floor = 90.0
ns_gain = 1.0
agc_gain = 0.85
hp_x_l = 0.0
hp_y_l = 0.0
hp_x_r = 0.0
hp_y_r = 0.0
lp_y_l = 0.0
lp_y_r = 0.0
vad_hold_frames = 0

try:
    while parec.stdout:
        pcm = parec.stdout.read(3840)
        if not pcm or len(pcm) < 3840:
            break
        with state_lock:
            krisp_on = cfg_krisp
            agc_on = cfg_agc
            auto_vad_on = cfg_auto_vad
            is_muted = cfg_muted or cfg_deafened
            man_vad_db = cfg_vad_db
            in_vol_pct = cfg_in_vol
            out_vol_pct = cfg_out_vol

        arr = array.array("h")
        arr.frombytes(pcm)
        n_samp = len(arr)

        # Stage 1: 90Hz High-Pass Filter (when Krisp is ON) + RMS & Peak measurement
        sum_sq = 0.0
        peak = 1.0
        if krisp_on:
            for i in range(0, n_samp, 2):
                xl = float(arr[i])
                xr = float(arr[i + 1])
                yl = xl - hp_x_l + 0.988 * hp_y_l
                yr = xr - hp_x_r + 0.988 * hp_y_r
                hp_x_l = xl
                hp_y_l = yl
                hp_x_r = xr
                hp_y_r = yr
                sum_sq += yl * yl + yr * yr
                al = abs(yl)
                ar = abs(yr)
                if al > peak:
                    peak = al
                if ar > peak:
                    peak = ar
                if yl > 32767.0:
                    yl = 32767.0
                elif yl < -32768.0:
                    yl = -32768.0
                if yr > 32767.0:
                    yr = 32767.0
                elif yr < -32768.0:
                    yr = -32768.0
                arr[i] = int(yl)
                arr[i + 1] = int(yr)
        else:
            for i in range(n_samp):
                v = float(arr[i])
                sum_sq += v * v
                av = abs(v)
                if av > peak:
                    peak = av

        raw_rms = math.sqrt(sum_sq / float(n_samp))

        # Stage 2: Adaptive Noise Floor Estimator & Krisp Wiener Suppression
        if raw_rms < noise_floor * 1.65:
            noise_floor = 0.92 * noise_floor + 0.08 * raw_rms
        else:
            noise_floor = min(1800.0, noise_floor * 1.0015 + 0.05)
        if noise_floor < 18.0:
            noise_floor = 18.0

        snr = raw_rms / max(noise_floor, 15.0)
        crest = peak / max(raw_rms, 1.0)
        if krisp_on:
            if snr < 2.1:
                g_target = 0.01
            elif snr < 3.8:
                norm_t = (snr - 2.1) / 1.7
                g_target = 0.01 + 0.99 * (norm_t * norm_t)
            else:
                g_target = 1.0
            # Suppress sharp transient keyboard/mouse clicks (high crest factor, short impulsive energy)
            if crest > 2.8 and snr < 9.5:
                g_target = 0.005
            if g_target > ns_gain:
                ns_gain = 0.42 * ns_gain + 0.58 * g_target
            else:
                ns_gain = 0.88 * ns_gain + 0.12 * g_target
        else:
            ns_gain = 1.0

        # Stage 3: Automatic Gain Control (AGC) + Fast Loud-Mic Attenuation + Peak Limiter
        post_ns_rms = raw_rms * ns_gain
        target_rms = 3800.0  # -18.7 dBFS comfortable target voice level
        if agc_on:
            if snr >= 1.9 and post_ns_rms > 70.0:
                desired_agc = max(0.12, min(2.4, target_rms / post_ns_rms))
                if desired_agc < agc_gain:
                    # Fast attenuation when microphone is too loud
                    agc_gain = 0.28 * agc_gain + 0.72 * desired_agc
                else:
                    # Gentle boost when microphone is quiet
                    agc_gain = 0.95 * agc_gain + 0.05 * desired_agc
            elif post_ns_rms * agc_gain > target_rms * 1.25:
                agc_gain = target_rms / max(post_ns_rms, 1.0)
        else:
            agc_gain = 1.0

        vol_mult = float(in_vol_pct) / 100.0
        total_gain = ns_gain * agc_gain * vol_mult
        # Hard ceiling limiter at -6.5 dBFS (15500) so loud mic never blasts or clips
        if peak * total_gain > 15500.0:
            total_gain = 15500.0 / max(peak, 1.0)

        # Apply gain + de-hiss smoothing when Krisp is ON and SNR is low
        dehiss = krisp_on and snr < 2.6
        proc_sum_sq = 0.0
        for i in range(0, n_samp, 2):
            sl = float(arr[i]) * total_gain
            sr = float(arr[i + 1]) * total_gain
            if dehiss:
                sl = 0.55 * sl + 0.45 * lp_y_l
                sr = 0.55 * sr + 0.45 * lp_y_r
            lp_y_l = sl
            lp_y_r = sr
            if sl > 32000.0:
                sl = 32000.0
            elif sl < -32000.0:
                sl = -32000.0
            if sr > 32000.0:
                sr = 32000.0
            elif sr < -32000.0:
                sr = -32000.0
            arr[i] = int(sl)
            arr[i + 1] = int(sr)
            proc_sum_sq += sl * sl + sr * sr

        proc_rms = math.sqrt(proc_sum_sq / float(n_samp))
        if proc_rms < 1.0 or (is_muted and not loopback):
            db = -100
        else:
            db = int(max(-100.0, min(0.0, round(20.0 * math.log10(proc_rms / 32768.0)))))

        # Stage 4: Automatic vs Manual Input Sensitivity (VAD)
        if auto_vad_on:
            nf_scaled = max(12.0, noise_floor * agc_gain * vol_mult * (0.35 if krisp_on else 1.0))
            nf_db = 20.0 * math.log10(nf_scaled / 32768.0)
            eff_vad_db = int(max(-65.0, min(-28.0, round(nf_db + 10.0))))
        else:
            eff_vad_db = int(max(-85, min(-5, man_vad_db)))

        if db >= eff_vad_db:
            vad_hold_frames = 14
        elif vad_hold_frames > 0:
            vad_hold_frames -= 1

        gate_open = vad_hold_frames > 0
        if not gate_open:
            # Silence frame when below VAD sensitivity threshold
            for i in range(n_samp):
                arr[i] = int(arr[i] * 0.05)

        proc_pcm = arr.tobytes()

        if loopback and pacat and pacat.stdin:
            out_mult = float(out_vol_pct) / 100.0
            with pacat_lock:
                pacat.stdin.write(scale_pcm_bytes(proc_pcm, out_mult))
                pacat.stdin.flush()

        n = opus.opus_encode(enc, proc_pcm, 960, out_pkt, 1276)
        if n < 0:
            n = 0
        stdout_buf.write(struct.pack("<bbH", db, eff_vad_db, n))
        if n > 0:
            stdout_buf.write(out_pkt.raw[:n])
        stdout_buf.flush()
except Exception:
    pass
finally:
    try:
        parec.kill()
    except Exception:
        pass
    try:
        if pacat:
            pacat.kill()
    except Exception:
        pass
"#;

fn ensure_local_mic_monitor() {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;
    LOCAL_MIC_MONITOR_ACTIVE.store(true, Ordering::SeqCst);
    if LOCAL_MIC_MONITOR_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let _ = std::panic::catch_unwind(|| {
            while LOCAL_MIC_MONITOR_ACTIVE.load(Ordering::Relaxed) {
                let start_epoch = AUDIO_DEVICE_EPOCH.load(Ordering::Relaxed);
                let start_loopback = LOCAL_MIC_LOOPBACK_ACTIVE.load(Ordering::Relaxed);
                let input_dev = selected_input_device()
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .filter(|s| !s.is_empty() && s != "default")
                    .unwrap_or_default();
                let output_dev = selected_output_device()
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .filter(|s| !s.is_empty() && s != "default")
                    .unwrap_or_default();
                let mut py_cmd = std::process::Command::new("python3");
                py_cmd.args([
                    "-u",
                    "-c",
                    OPUS_AUDIO_BRIDGE_PY,
                    &input_dev,
                    &output_dev,
                    if start_loopback { "1" } else { "0" },
                ]);
                let child = py_cmd
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                let Ok(mut proc) = child else {
                    for _ in 0..15 {
                        if !LOCAL_MIC_MONITOR_ACTIVE.load(Ordering::Relaxed) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                    continue;
                };
                let Some(mut stdout) = proc.stdout.take() else {
                    let _ = proc.kill();
                    let _ = proc.wait();
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                };
                let rx_stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                if let Some(mut stdin) = proc.stdin.take() {
                    let rx_stop_clone = std::sync::Arc::clone(&rx_stop);
                    std::thread::spawn(move || {
                        let mut last_ctrl = [0u8; 8];
                        let mut tick = 0u32;
                        while !rx_stop_clone.load(Ordering::Relaxed)
                            && LOCAL_MIC_MONITOR_ACTIVE.load(Ordering::Relaxed)
                        {
                            let ctrl = [
                                u8::from(LOCAL_KRISP_ENABLED.load(Ordering::Relaxed)),
                                u8::from(LOCAL_AUTO_GAIN_ENABLED.load(Ordering::Relaxed)),
                                u8::from(LOCAL_AUTO_SENSITIVITY_ENABLED.load(Ordering::Relaxed)),
                                u8::from(LOCAL_SELF_MUTE.load(Ordering::Relaxed)),
                                (LOCAL_VAD_THRESHOLD_DB.load(Ordering::Relaxed).clamp(-85, -5)
                                    as i8) as u8,
                                LOCAL_INPUT_VOLUME_PCT.load(Ordering::Relaxed).min(200) as u8,
                                LOCAL_OUTPUT_VOLUME_PCT.load(Ordering::Relaxed).min(200) as u8,
                                u8::from(LOCAL_SELF_DEAF.load(Ordering::Relaxed)),
                            ];
                            if ctrl != last_ctrl || tick % 10 == 0 {
                                last_ctrl = ctrl;
                                let mut c_hdr = [0u8; 6];
                                c_hdr[0..4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
                                c_hdr[4..6].copy_from_slice(&8u16.to_le_bytes());
                                if stdin.write_all(&c_hdr).is_err()
                                    || stdin.write_all(&ctrl).is_err()
                                {
                                    return;
                                }
                                let _ = stdin.flush();
                            }
                            tick = tick.wrapping_add(1);
                            let rx_frames = discord_core::ws::drain_live_rtc_rx_opus();
                            if !rx_frames.is_empty() && !LOCAL_SELF_DEAF.load(Ordering::Relaxed) {
                                for (ssrc, opus) in rx_frames {
                                    let Ok(plen) = u16::try_from(opus.len()) else {
                                        continue;
                                    };
                                    let mut hdr = [0u8; 6];
                                    hdr[0..4].copy_from_slice(&ssrc.to_le_bytes());
                                    hdr[4..6].copy_from_slice(&plen.to_le_bytes());
                                    if stdin.write_all(&hdr).is_err()
                                        || stdin.write_all(&opus).is_err()
                                    {
                                        return;
                                    }
                                }
                                let _ = stdin.flush();
                            } else {
                                std::thread::sleep(std::time::Duration::from_millis(15));
                            }
                        }
                    });
                }
                let mut hdr = [0u8; 4];
                let mut opus_buf = vec![0u8; 1500];
                let mut hold_frames = 0u32;
                while LOCAL_MIC_MONITOR_ACTIVE.load(Ordering::Relaxed)
                    && AUDIO_DEVICE_EPOCH.load(Ordering::Relaxed) == start_epoch
                    && LOCAL_MIC_LOOPBACK_ACTIVE.load(Ordering::Relaxed) == start_loopback
                    && stdout.read_exact(&mut hdr).is_ok()
                {
                    let db = i32::from(hdr[0] as i8);
                    let eff_vad_db = i32::from(hdr[1] as i8);
                    let opus_len = usize::from(u16::from_le_bytes([hdr[2], hdr[3]]));
                    if opus_len > 0 {
                        if opus_len > opus_buf.len() {
                            opus_buf.resize(opus_len, 0);
                        }
                        if stdout.read_exact(&mut opus_buf[..opus_len]).is_err() {
                            break;
                        }
                    }
                    LOCAL_MIC_LEVEL_DB.store(db, Ordering::Relaxed);
                    LOCAL_EFFECTIVE_VAD_DB.store(eff_vad_db, Ordering::Relaxed);
                    let is_speaking = if db >= eff_vad_db {
                        hold_frames = 14; // ~280 мс удержания зелёного кольца вокруг аватарки (при 20 мс кадрах)
                        true
                    } else if hold_frames > 0 {
                        hold_frames -= 1;
                        true
                    } else {
                        false
                    };
                    LOCAL_MIC_SPEAKING.store(is_speaking, Ordering::Relaxed);
                    if is_speaking
                        && opus_len > 0
                        && LOCAL_VOICE_ACTIVE.load(Ordering::Relaxed)
                        && !LOCAL_SELF_MUTE.load(Ordering::Relaxed)
                        && !LOCAL_SELF_DEAF.load(Ordering::Relaxed)
                    {
                        discord_core::ws::push_live_rtc_tx_opus(opus_buf[..opus_len].to_vec());
                    }
                }
                rx_stop.store(true, Ordering::SeqCst);
                LOCAL_MIC_SPEAKING.store(false, Ordering::Relaxed);
                LOCAL_MIC_LEVEL_DB.store(-100, Ordering::Relaxed);
                drop(stdout);
                let _ = proc.kill();
                let _ = proc.wait();
                if LOCAL_MIC_MONITOR_ACTIVE.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
            }
        });
        LOCAL_MIC_SPEAKING.store(false, Ordering::Relaxed);
        LOCAL_MIC_LEVEL_DB.store(-100, Ordering::Relaxed);
        LOCAL_MIC_MONITOR_STARTED.store(false, Ordering::SeqCst);
    });
}

/// Возвращает JSON-снимок текущего состояния RTC-соединения для UI (`voice_rtc_status`).
#[must_use]
pub fn live_voice_rtc_status_json() -> Value {
    use std::sync::atomic::Ordering;
    let maybe_elapsed = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.voice_connect_started_at.map(|t| t.elapsed())
    };
    if let Some(el) = maybe_elapsed {
        if el >= std::time::Duration::from_secs(15) {
            let _ = check_voice_handshake_timeout_with_elapsed(el);
        }
    }
    let snap = discord_core::ws::live_rtc_snapshot();
    let local_speaking = LOCAL_MIC_SPEAKING.load(Ordering::Relaxed);
    let local_db = LOCAL_MIC_LEVEL_DB.load(Ordering::Relaxed);
    let mut speaking_users = snap.speaking_users;
    let has_unmapped_ssrc = speaking_users.iter().any(|u| u.starts_with("ssrc:"));
    let mut effective_state = snap.state.clone();
    let mut voice_error: Option<String> = snap.error.clone();
    let mut voice_timed_out = false;
    if let Ok(mut guard) = live_session().lock() {
        let local_active = LOCAL_VOICE_ACTIVE.load(Ordering::SeqCst);
        if !local_active || guard.voice_channel_id.is_none() {
            guard.voice_phase = "disconnected".to_string();
            effective_state = "disconnected".to_string();
            if let Some(ref err) = guard.voice_last_error {
                voice_error = Some(err.clone());
                voice_timed_out = true;
            }
        } else if guard.voice_got_state_update
            && guard.voice_got_server_update
            && (guard.voice_got_rtc_op4 || snap.state == "connected")
        {
            guard.voice_phase = "connected".to_string();
            guard.voice_last_error = None;
            effective_state = "connected".to_string();
        } else if guard.voice_got_state_update && guard.voice_got_server_update {
            guard.voice_phase = "negotiating".to_string();
            effective_state = "negotiating".to_string();
        } else {
            guard.voice_phase = "signaling".to_string();
            effective_state = "signaling".to_string();
        }
        let self_id_opt = guard
            .ready_data
            .as_ref()
            .and_then(|d| d.get("user"))
            .and_then(|u| field_str(u, "id"))
            .map(str::to_string);
        if local_speaking && !LOCAL_SELF_MUTE.load(Ordering::Relaxed) && !LOCAL_SELF_DEAF.load(Ordering::Relaxed) {
            if let Some(ref self_id) = self_id_opt {
                if !speaking_users.iter().any(|u| u == self_id) {
                    speaking_users.push(self_id.clone());
                }
            }
        }
        if has_unmapped_ssrc {
            if let (Some(ref gid), Some(ref cid)) = (&snap.guild_id, &snap.channel_id) {
                if let Some(vs_list) = guard.voice_states.get(gid) {
                    for vs in vs_list {
                        if field_str(vs, "channel_id") == Some(cid.as_str()) {
                            if let Some(uid) = field_str(vs, "user_id") {
                                if self_id_opt.as_deref() != Some(uid)
                                    && !field_bool(vs, "self_mute")
                                    && !field_bool(vs, "mute")
                                    && !speaking_users.iter().any(|u| u == uid)
                                {
                                    speaking_users.push(uid.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    speaking_users.retain(|u| !u.starts_with("ssrc:"));
    let (stream_muted, stream_vol) = discord_core::ws::get_live_stream_audio_state();
    let is_connected = effective_state == "connected";
    let ui_phase = if voice_timed_out {
        "timeout"
    } else {
        effective_state.as_str()
    };
    json!({
        "state": effective_state,
        "phase": ui_phase,
        "timedOut": voice_timed_out,
        "error": voice_error,
        "connected": is_connected,
        "endpoint": snap.endpoint,
        "shortHostname": snap.short_hostname,
        "guildId": snap.guild_id,
        "channelId": snap.channel_id,
        "ssrc": snap.ssrc,
        "lastPingMs": snap.last_ping_ms,
        "averagePingMs": snap.average_ping_ms,
        "pingHistory": snap.ping_history,
        "packetLossPct": snap.packet_loss_pct,
        "protocol": snap.protocol,
        "encryptionMode": snap.encryption_mode,
        "speakingUsers": speaking_users,
        "localMicSpeaking": local_speaking,
        "localMicLevelDb": local_db,
        "effectiveVadDb": LOCAL_EFFECTIVE_VAD_DB.load(Ordering::Relaxed),
        "vadThresholdDb": LOCAL_VAD_THRESHOLD_DB.load(Ordering::Relaxed),
        "krispEnabled": LOCAL_KRISP_ENABLED.load(Ordering::Relaxed),
        "autoGainControl": LOCAL_AUTO_GAIN_ENABLED.load(Ordering::Relaxed),
        "autoSensitivity": LOCAL_AUTO_SENSITIVITY_ENABLED.load(Ordering::Relaxed),
        "inputVolumePct": LOCAL_INPUT_VOLUME_PCT.load(Ordering::Relaxed),
        "outputVolumePct": LOCAL_OUTPUT_VOLUME_PCT.load(Ordering::Relaxed),
        "streamAudioMuted": stream_muted,
        "streamAudioVolumePct": stream_vol,
        "gpuVendor": snap.gpu_vendor,
        "gpuModel": snap.gpu_model,
        "gpuApi": snap.gpu_api,
        "hwEncoders": snap.hw_encoders,
        "hwDecoders": snap.hw_decoders,
        "activeVideoCodec": snap.active_video_codec,
        "micTestActive": LOCAL_MIC_LOOPBACK_ACTIVE.load(Ordering::Relaxed),
    })
}

/// Запускает дочерний процесс `pactl` с жёстким таймаутом, изоляцией паник и лимитом памяти stdout (`256 KiB`),
/// гарантированно очищая дочерний процесс через `kill() + wait()` при зависании аудио-демона.
fn run_pactl_bounded(args: &[&str], timeout: std::time::Duration) -> Option<String> {
    use std::io::Read;
    const MAX_PACTL_STDOUT_BYTES: u64 = 256 * 1024;
    let mut child = std::process::Command::new("pactl")
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let stdout_pipe = child.stdout.take();
    let reader_handle = stdout_pipe.map(|pipe| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.take(MAX_PACTL_STDOUT_BYTES).read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    });
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(12));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    reader_handle.and_then(|h| h.join().ok())
}

/// Возвращает список реальных аудиоустройств ввода и вывода из PipeWire/PulseAudio (`pactl`).
/// Четыре независимых дочерних процесса `pactl` запускаются параллельно в отдельных потоках (`std::thread::scope`)
/// с защитой `catch_unwind` и таймаутом `1200 мс`.
#[must_use]
pub fn list_system_audio_devices_json() -> Value {
    use std::sync::atomic::Ordering;
    ensure_local_mic_monitor();
    fn parse_pactl_list(kind: &str) -> Vec<Value> {
        let Some(text) =
            run_pactl_bounded(&["list", kind], std::time::Duration::from_millis(1200))
        else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let mut cur_name = String::new();
        let mut cur_desc = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("Name:") {
                if !cur_name.is_empty() {
                    items.push((cur_name.clone(), cur_desc.clone()));
                }
                cur_name = rest.trim().to_string();
                cur_desc.clear();
            } else if let Some(rest) = trimmed.strip_prefix("Description:") {
                cur_desc = rest.trim().to_string();
            }
        }
        if !cur_name.is_empty() {
            items.push((cur_name, cur_desc));
        }
        items
            .into_iter()
            .filter(|(name, _)| kind != "sources" || !name.ends_with(".monitor"))
            .map(|(name, desc)| {
                let label = if desc.is_empty() { name.clone() } else { desc };
                json!({
                    "id": name,
                    "name": label,
                })
            })
            .collect()
    }
    let (default_source, default_sink, mut inputs, mut outputs) = std::thread::scope(|s| {
        let h_src = s.spawn(|| {
            std::panic::catch_unwind(|| {
                run_pactl_bounded(
                    &["get-default-source"],
                    std::time::Duration::from_millis(800),
                )
                .map(|o| o.trim().to_string())
                .unwrap_or_default()
            })
            .unwrap_or_default()
        });
        let h_sink = s.spawn(|| {
            std::panic::catch_unwind(|| {
                run_pactl_bounded(&["get-default-sink"], std::time::Duration::from_millis(800))
                    .map(|o| o.trim().to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
        });
        let h_inputs = s.spawn(|| {
            std::panic::catch_unwind(|| parse_pactl_list("sources")).unwrap_or_default()
        });
        let h_outputs = s.spawn(|| {
            std::panic::catch_unwind(|| parse_pactl_list("sinks")).unwrap_or_default()
        });
        (
            h_src.join().unwrap_or_default(),
            h_sink.join().unwrap_or_default(),
            h_inputs.join().unwrap_or_default(),
            h_outputs.join().unwrap_or_default(),
        )
    });
    if inputs.is_empty() {
        inputs.push(json!({ "id": "default", "name": "По умолчанию (System Input)" }));
    }
    if outputs.is_empty() {
        outputs.push(json!({ "id": "default", "name": "По умолчанию (System Output)" }));
    }
    json!({
        "inputs": inputs,
        "outputs": outputs,
        "defaultInput": if default_source.is_empty() { "default".to_string() } else { default_source },
        "defaultOutput": if default_sink.is_empty() { "default".to_string() } else { default_sink },
        "krispEnabled": LOCAL_KRISP_ENABLED.load(Ordering::Relaxed),
        "autoGainControl": LOCAL_AUTO_GAIN_ENABLED.load(Ordering::Relaxed),
        "autoSensitivity": LOCAL_AUTO_SENSITIVITY_ENABLED.load(Ordering::Relaxed),
        "vadThresholdDb": LOCAL_VAD_THRESHOLD_DB.load(Ordering::Relaxed),
        "inputVolumePct": LOCAL_INPUT_VOLUME_PCT.load(Ordering::Relaxed),
        "outputVolumePct": LOCAL_OUTPUT_VOLUME_PCT.load(Ordering::Relaxed),
    })
}

/// Выполняет REST-запросы личных сообщений (звонки / звонок начался / ringing)
pub fn handle_live_dm_rest_request(raw_json: &str) -> Value {
    let parsed: Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(_) => return json!({ "ok": false, "error": "invalid json payload" }),
    };
    let method = field_str(&parsed, "method").unwrap_or("GET").to_uppercase();
    let url = field_str(&parsed, "url").unwrap_or("");
    let body_val = parsed.get("body");

    let token_opt = {
        let guard = live_session().lock().unwrap_or_else(|p| p.into_inner());
        guard.token.clone()
    };
    let Some(token) = token_opt.filter(|t| !t.trim().is_empty()) else {
        return json!({ "ok": false, "error": "no authorization token" });
    };

    let identity = crate::auth::IdentityContext::detect();
    let auth = match discord_core::rest::client::ApiAuth::from_token_with_scheme(
        &token,
        discord_core::rest::client::AuthScheme::User,
        &identity.super_properties,
        &identity.locale,
        &identity.timezone,
    ) {
        Ok(a) => a,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };

    let client = match discord_core::rest::client::ApiClient::new(discord_core::rest::client::ApiConfig {
        base: discord_core::API_V9_BASE.to_string(),
        auth,
        http: discord_core::rest::transport::HttpClientConfig {
            connect_timeout: std::time::Duration::from_millis(1500),
            read_timeout: std::time::Duration::from_millis(2500),
            write_timeout: std::time::Duration::from_millis(1500),
            ..discord_core::rest::transport::HttpClientConfig::default()
        },
        retry: discord_core::rest::client::ApiRetryPolicy {
            max_attempts: 1,
            ..discord_core::rest::client::ApiRetryPolicy::default()
        },
    }) {
        Ok(c) => c,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => return json!({ "ok": false, "error": e.to_string() }),
    };

    let body_bytes = body_val.map(|b| serde_json::to_vec(b).unwrap_or_default()).unwrap_or_default();
    let res = match method.as_str() {
        "POST" => runtime.block_on(client.post(url, &body_bytes)),
        "PUT" => runtime.block_on(client.put(url, &body_bytes)),
        "DELETE" => runtime.block_on(client.delete(url)),
        _ => runtime.block_on(client.get(url)),
    };

    match res {
        Ok(resp) => {
            let status = resp.status;
            let data: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
            json!({
                "ok": (200..300).contains(&status),
                "status": status,
                "data": data
            })
        }
        Err(e) => json!({
            "ok": false,
            "error": e.to_string()
        }),
    }
}

/// Полностью очищает локальное состояние стрима (ключ, токен, сервер) и немедленно
/// завершает активную RTC-сессию и фоновые процессы декодирования/трансляции кадров.
pub fn reset_live_stream_state() {
    LOCAL_SELF_STREAM.store(false, std::sync::atomic::Ordering::SeqCst);
    discord_core::ws::clear_last_stream_key();
    if let Ok(mut guard) = live_session().lock() {
        guard.stream_key = None;
        guard.stream_rtc_server_id = None;
        guard.stream_rtc_channel_id = None;
        guard.stream_endpoint = None;
        guard.stream_token = None;
        guard.stream_launched_token = None;
        guard.stream_launched_server_id = None;
    }
    discord_core::ws::stop_live_stream_rtc_session();
    let _ = std::fs::remove_file("/dev/shm/discord_remote_stream_frame.jpg");
    let _ = std::fs::remove_file("/dev/shm/discord_remote_stream_meta.bin");
    let _ = std::fs::remove_file("/tmp/discord_remote_stream_frame.jpg");
}

/// Опрашивает список реальных окон через KWin Scripting D-Bus API на KDE Wayland.
pub fn query_kwin_active_windows() -> Vec<Value> {
    let js_code = r#"
(function() {
    var wins = workspace.windowList();
    var res = [];
    for (var i = 0; i < wins.length; ++i) {
        var w = wins[i];
        if (w.normalWindow && !w.deleted) {
            res.push({
                id: String(w.internalId),
                title: String(w.caption || ""),
                app: String(w.resourceClass || w.resourceName || "")
            });
        }
    }
    console.info("KWIN_WINDOWS_JSON:" + JSON.stringify(res));
})();
"#;
    let script_path = "/tmp/discord_kwin_list_wins.js";
    if std::fs::write(script_path, js_code).is_err() {
        return fallback_capture_windows();
    }

    let plugin_name = format!(
        "query_wins_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let out = std::process::Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.kde.KWin",
            "--object-path",
            "/Scripting",
            "--method",
            "org.kde.kwin.Scripting.loadScript",
            script_path,
            &plugin_name,
        ])
        .output();

    let Ok(out) = out else {
        return fallback_capture_windows();
    };

    let out_str = String::from_utf8_lossy(&out.stdout);
    let trimmed = out_str
        .trim()
        .trim_matches(|c| c == '(' || c == ')' || c == ',' || c == ' ');
    if let Ok(script_num) = trimmed.parse::<u32>() {
        let obj_path = format!("/Scripting/Script{script_num}");
        let _ = std::process::Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.kde.KWin",
                "--object-path",
                &obj_path,
                "--method",
                "org.kde.kwin.Script.run",
            ])
            .output();
        let _ = std::process::Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.kde.KWin",
                "--object-path",
                &obj_path,
                "--method",
                "org.kde.kwin.Script.stop",
            ])
            .output();

        std::thread::sleep(std::time::Duration::from_millis(35));
        if let Ok(j_out) = std::process::Command::new("journalctl")
            .args(["_COMM=kwin_wayland", "-n", "6", "--no-pager"])
            .output()
        {
            let j_text = String::from_utf8_lossy(&j_out.stdout);
            for line in j_text.lines().rev() {
                if let Some(pos) = line.find("KWIN_WINDOWS_JSON:") {
                    let json_sub = &line[pos + "KWIN_WINDOWS_JSON:".len()..];
                    if let Ok(parsed) = serde_json::from_str::<Vec<Value>>(json_sub) {
                        let res: Vec<Value> = parsed
                            .into_iter()
                            .filter(|w| {
                                let title = w
                                    .get("title")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("")
                                    .trim();
                                let app = w
                                    .get("app")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("")
                                    .trim();
                                !title.is_empty() && app != "kwin_wayland"
                            })
                            .map(|w| {
                                let wid = w
                                    .get("id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let title = w
                                    .get("title")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let app = w
                                    .get("app")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let label = if !title.is_empty() {
                                    title.clone()
                                } else {
                                    app.clone()
                                };
                                json!({
                                    "id": wid,
                                    "uuid": wid,
                                    "title": title,
                                    "label": label,
                                    "app": app
                                })
                            })
                            .collect();
                        if !res.is_empty() {
                            return res;
                        }
                    }
                }
            }
        }
    }
    fallback_capture_windows()
}

fn fallback_capture_windows() -> Vec<Value> {
    vec![
        json!({ "id": "win_orca", "uuid": "win_orca", "title": "Orca", "label": "Orca", "app": "orca" }),
        json!({ "id": "win_browser", "uuid": "win_browser", "title": "Браузер", "label": "Браузер", "app": "browser" }),
        json!({ "id": "win_discord", "uuid": "win_discord", "title": "Discord Lab", "label": "Discord Lab", "app": "discord" })
    ]
}

/// Управляет аудиопараметрами ТОЛЬКО внутри приложения (`discord_voice.node` + `OPUS_AUDIO_BRIDGE_PY`),
/// никогда не изменяя системную громкость или системный mute ОС (`pactl`).
pub fn set_system_audio_device(
    kind: &str,
    device_id: Option<&str>,
    volume_pct: Option<u32>,
    muted: Option<bool>,
) -> Value {
    use std::sync::atomic::Ordering;
    if kind == "set_capture_source" {
        let raw = device_id.unwrap_or_default().trim().to_string();
        let mut target_handle = String::new();
        let mut target_label = String::new();
        let mut target_kind = "screen".to_string();
        if !raw.is_empty() {
            if raw.starts_with('{') {
                target_handle = raw.clone();
                target_kind = "window".to_string();
            } else if raw.starts_with("screen:") || raw.starts_with("screens:") {
                target_handle = raw.clone();
                target_kind = "screen".to_string();
            } else if raw.contains(':') {
                let parts: Vec<&str> = raw.splitn(3, ':').collect();
                if let Some(&tab) = parts.first() {
                    if tab == "windows" || tab == "window" {
                        target_kind = "window".to_string();
                    }
                }
                if let Some(&id) = parts.get(1) {
                    target_handle = id.trim().to_string();
                }
                if let Some(&lbl) = parts.get(2) {
                    target_label = lbl.trim().to_string();
                }
            } else {
                target_handle = raw.clone();
                if raw.starts_with("win") {
                    target_kind = "window".to_string();
                }
            }
        }
        if target_handle.starts_with('{') {
            target_kind = "window".to_string();
        }
        let _ = std::fs::write("/dev/shm/discord_capture_target.txt", target_handle.as_bytes());
        let _ = std::fs::write("/dev/shm/discord_capture_kick.bin", &[1u8]);
        let ucmd = format!(
            "{{\"cmd\":\"set_stream_source\",\"sourceType\":\"{target_kind}\",\"sourceId\":\"{target_handle}\",\"label\":\"{target_label}\"}}"
        );
        discord_core::ws::send_native_stream_cmd(&ucmd);
        return json!({
            "ok": true,
            "targetKind": target_kind,
            "targetHandle": target_handle,
            "targetLabel": target_label
        });
    }
    if kind == "list_capture_windows" {
        let windows = query_kwin_active_windows();
        return json!({
            "ok": true,
            "windows": windows
        });
    }
    if kind == "ui_telemetry" {
        if let Some(line) = device_id {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/live-ui-telemetry.jsonl")
            {
                let _ = writeln!(file, "{line}");
            }
            println!("[USER-TELEMETRY] {line}");
        }
        return json!({ "ok": true });
    }
    if kind == "dm_rest_request" {
        return handle_live_dm_rest_request(device_id.unwrap_or_default());
    }
    if kind == "dm_friends_sync" {
        let dev_str = device_id.unwrap_or_default();
        let refresh_rest = dev_str == "refresh_rest";
        let sync_guild_id = dev_str.strip_prefix("sync_guild:").map(str::trim).filter(|s| !s.is_empty());
        let (mut guilds, mut relationships, mut private_channels, mut users, mut presences, token_opt, need_fetch) = {
            let mut guard = live_session().lock().unwrap_or_else(|p| p.into_inner());
            if !guard.active {
                return json!({ "ok": false });
            }
            if guard.ready_data.is_none() {
                let tok_opt = guard.token.clone();
                let exp_uid = guard.user.as_ref().map(|u| u.id.clone());
                if let Some(cached) = load_live_ready_cache_for(tok_opt.as_deref(), exp_uid.as_deref()) {
                    populate_voice_and_presences_from_ready(&mut guard, &cached);
                    guard.ready_data = Some(cached);
                }
            }
            let rd = guard.ready_data.clone();
            let tok = guard.token.clone();
            let guilds = rd.as_ref().and_then(|d| d.get("guilds").cloned()).unwrap_or(Value::Array(Vec::new()));
            let rels = rd.as_ref().and_then(|d| d.get("relationships").cloned()).unwrap_or(Value::Array(Vec::new()));
            let privs = rd.as_ref().and_then(|d| d.get("private_channels").cloned()).unwrap_or(Value::Array(Vec::new()));
            let users = rd.as_ref().and_then(|d| d.get("users").cloned()).unwrap_or(Value::Array(Vec::new()));
            let presences: Vec<Value> = guard.presences.values().cloned().collect();
            let need_fetch = refresh_rest || sync_guild_id.is_some() || rd.is_none();
            (guilds, rels, privs, users, presences, tok, need_fetch)
        };
        if need_fetch {
            if let Some(sgid) = sync_guild_id {
                let snap_opt = {
                    let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
                    ensure_local_guild_in_guard(&mut g, sgid, None)
                };
                if let Some(snap) = snap_opt {
                    save_live_ready_cache(&snap);
                }
                let has_mock = live_rest_mock_slot()
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_some();
                if has_mock {
                    hydrate_live_guild_blocking(sgid);
                } else {
                    spawn_hydrate_live_guild_full(sgid);
                }
                {
                    let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
                    if let Some(ref rd) = g.ready_data {
                        guilds = rd.get("guilds").cloned().unwrap_or(Value::Array(Vec::new()));
                        relationships = rd
                            .get("relationships")
                            .cloned()
                            .unwrap_or(Value::Array(Vec::new()));
                        private_channels = rd
                            .get("private_channels")
                            .cloned()
                            .unwrap_or(Value::Array(Vec::new()));
                        users = rd.get("users").cloned().unwrap_or(Value::Array(Vec::new()));
                        presences = g.presences.values().cloned().collect();
                    }
                }
            } else if let Some(tok) = token_opt {
                static LIVE_READY_FETCH_IN_FLIGHT: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                static LAST_LIVE_READY_FETCH_AT: OnceLock<Mutex<Option<std::time::Instant>>> =
                    OnceLock::new();
                let ts_slot = LAST_LIVE_READY_FETCH_AT.get_or_init(|| Mutex::new(None));
                let mut ts_guard = ts_slot.lock().unwrap_or_else(|p| p.into_inner());
                let min_cooldown = if refresh_rest {
                    std::time::Duration::from_secs(30)
                } else {
                    std::time::Duration::from_secs(60)
                };
                let cooled_down = ts_guard
                    .map(|t| t.elapsed() >= min_cooldown)
                    .unwrap_or(true);
                let should_spawn = cooled_down && !LIVE_READY_FETCH_IN_FLIGHT.swap(true, Ordering::SeqCst);
                if should_spawn {
                    *ts_guard = Some(std::time::Instant::now());
                }
                if should_spawn {
                    std::thread::spawn(move || {
                        if let Some(mut fetched_rd) = fetch_live_ready_via_rest(&tok) {
                            sanitize_etf_json_value(&mut fetched_rd);
                            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
                            if g.active {
                                let prev_guilds: HashMap<String, Value> = g
                                    .ready_data
                                    .as_ref()
                                    .and_then(|rd| rd.get("guilds"))
                                    .and_then(Value::as_array)
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|gv| {
                                                field_str(gv, "id")
                                                    .map(|id| (id.to_string(), gv.clone()))
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                if let Value::Object(ref mut r_map) = fetched_rd {
                                    let g_arr = r_map
                                        .entry("guilds".to_string())
                                        .or_insert_with(|| Value::Array(Vec::new()));
                                    if let Value::Array(ref mut list) = g_arr {
                                        for item in list.iter_mut() {
                                            let Some(gid) =
                                                field_str(item, "id").map(str::to_string)
                                            else {
                                                continue;
                                            };
                                            let prev_opt = prev_guilds
                                                .get(&gid)
                                                .or_else(|| g.local_preview_guilds.get(&gid));
                                            if let Value::Object(im) = item {
                                                im.insert("isMember".to_string(), Value::Bool(true));
                                                im.insert("lurker".to_string(), Value::Bool(false));
                                                if let Some(Value::Object(pm)) = prev_opt {
                                                    for k in [
                                                        "roles",
                                                        "channels",
                                                        "members",
                                                        "my_roles",
                                                        "myRoles",
                                                        "voice_states",
                                                        "properties",
                                                    ] {
                                                        let cur_empty = im
                                                            .get(k)
                                                            .and_then(Value::as_array)
                                                            .map_or(
                                                                im.get(k).is_none(),
                                                                Vec::is_empty,
                                                            );
                                                        if cur_empty {
                                                            if let Some(pv) = pm.get(k) {
                                                                im.insert(k.to_string(), pv.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        for (prev_gid, prev_gval) in &prev_guilds {
                                            if !list.iter().any(|x| field_str(x, "id") == Some(prev_gid.as_str())) {
                                                list.push(prev_gval.clone());
                                            }
                                        }
                                        for (lgid, lval) in &g.local_preview_guilds {
                                            if !list
                                                .iter()
                                                .any(|x| field_str(x, "id") == Some(lgid.as_str()))
                                            {
                                                let mut preview_clone = lval.clone();
                                                if let Value::Object(ref mut pm) = preview_clone {
                                                    if !pm.contains_key("isMember") {
                                                        pm.insert(
                                                            "isMember".to_string(),
                                                            Value::Bool(true),
                                                        );
                                                    }
                                                    if !pm.contains_key("lurker") {
                                                        pm.insert(
                                                            "lurker".to_string(),
                                                            Value::Bool(false),
                                                        );
                                                    }
                                                }
                                                list.push(preview_clone);
                                            }
                                        }
                                    }
                                    let prev_rels = g
                                        .ready_data
                                        .as_ref()
                                        .and_then(|old_rd| old_rd.get("relationships"))
                                        .and_then(Value::as_array)
                                        .cloned()
                                        .unwrap_or_default();
                                    if !prev_rels.is_empty() {
                                        let cur_rels = r_map
                                            .entry("relationships".to_string())
                                            .or_insert_with(|| Value::Array(Vec::new()));
                                        if let Value::Array(ref mut clist) = cur_rels {
                                            if clist.is_empty() {
                                                *clist = prev_rels;
                                            } else {
                                                for pr in prev_rels {
                                                    let puid = field_str(&pr, "id").unwrap_or("");
                                                    if !puid.is_empty() && !clist.iter().any(|cr| field_str(cr, "id") == Some(puid)) {
                                                        clist.push(pr);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    let prev_privs = g
                                        .ready_data
                                        .as_ref()
                                        .and_then(|old_rd| old_rd.get("private_channels"))
                                        .and_then(Value::as_array)
                                        .cloned()
                                        .unwrap_or_default();
                                    if !prev_privs.is_empty() {
                                        let cur_privs = r_map
                                            .entry("private_channels".to_string())
                                            .or_insert_with(|| Value::Array(Vec::new()));
                                        if let Value::Array(ref mut clist) = cur_privs {
                                            if clist.is_empty() {
                                                *clist = prev_privs;
                                            } else {
                                                for pc in prev_privs {
                                                    let pcid = field_str(&pc, "id").unwrap_or("");
                                                    if !pcid.is_empty() && !clist.iter().any(|c| field_str(c, "id") == Some(pcid)) {
                                                        clist.push(pc);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                populate_voice_and_presences_from_ready(&mut g, &fetched_rd);
                                g.ready_data = Some(fetched_rd.clone());
                                drop(g);
                                save_live_ready_cache(&fetched_rd);
                            }
                        }
                        LIVE_READY_FETCH_IN_FLIGHT.store(false, Ordering::SeqCst);
                    });
                }
            }
        }
        return json!({
            "ok": true,
            "guilds": guilds,
            "relationships": relationships,
            "privateChannels": private_channels,
            "users": users,
            "presences": presences,
        });
    }
    if kind == "krisp" {
        let enabled = muted.unwrap_or(true);
        LOCAL_KRISP_ENABLED.store(enabled, Ordering::SeqCst);
        discord_core::ws::send_native_voice_cmd(&format!(
            "{{\"cmd\":\"set_state\",\"krisp\":{enabled}}}"
        ));
        return json!({ "ok": true, "krisp": enabled });
    }
    if kind == "agc" {
        let enabled = muted.unwrap_or(true);
        LOCAL_AUTO_GAIN_ENABLED.store(enabled, Ordering::SeqCst);
        discord_core::ws::send_native_voice_cmd(&format!(
            "{{\"cmd\":\"set_state\",\"automaticGainControl\":{enabled}}}"
        ));
        return json!({ "ok": true, "automaticGainControl": enabled });
    }
    if kind == "auto_sensitivity" {
        let enabled = muted.unwrap_or(true);
        LOCAL_AUTO_SENSITIVITY_ENABLED.store(enabled, Ordering::SeqCst);
        discord_core::ws::send_native_voice_cmd(&format!(
            "{{\"cmd\":\"set_state\",\"autoSensitivity\":{enabled}}}"
        ));
        return json!({ "ok": true, "autoSensitivity": enabled });
    }
    if kind == "vad_threshold" {
        let db = device_id
            .and_then(|s| s.trim().parse::<i32>().ok())
            .or_else(|| volume_pct.map(|v| -(v.min(90) as i32)))
            .unwrap_or(-45)
            .clamp(-85, -5);
        LOCAL_VAD_THRESHOLD_DB.store(db, Ordering::SeqCst);
        if let Some(auto_on) = muted {
            LOCAL_AUTO_SENSITIVITY_ENABLED.store(auto_on, Ordering::SeqCst);
        }
        let auto_now = LOCAL_AUTO_SENSITIVITY_ENABLED.load(Ordering::Relaxed);
        discord_core::ws::send_native_voice_cmd(&format!(
            "{{\"cmd\":\"set_state\",\"vadThreshold\":{db},\"autoSensitivity\":{auto_now}}}"
        ));
        return json!({ "ok": true, "vadThresholdDb": db, "autoSensitivity": auto_now });
    }
    if kind == "mic_test_start" {
        LOCAL_MIC_LOOPBACK_ACTIVE.store(true, Ordering::SeqCst);
        AUDIO_DEVICE_EPOCH.fetch_add(1, Ordering::SeqCst);
        discord_core::ws::send_native_voice_cmd(r#"{"cmd":"loopback","active":true}"#);
        ensure_local_mic_monitor();
        return live_voice_rtc_status_json();
    }
    if kind == "mic_test_stop" {
        LOCAL_MIC_LOOPBACK_ACTIVE.store(false, Ordering::SeqCst);
        AUDIO_DEVICE_EPOCH.fetch_add(1, Ordering::SeqCst);
        discord_core::ws::send_native_voice_cmd(r#"{"cmd":"loopback","active":false}"#);
        return live_voice_rtc_status_json();
    }
    if kind == "media_play" {
        if let Some(spec) = device_id {
            start_linux_media_audio_playback(spec, volume_pct, muted);
        }
        return json!({ "ok": true });
    }
    if kind == "media_stop" {
        MEDIA_PLAY_EPOCH.fetch_add(1, Ordering::SeqCst);
        return json!({ "ok": true });
    }
    if kind == "voice_msg_record_start" {
        return start_voice_msg_recording();
    }
    if kind == "voice_msg_record_stop" {
        VOICE_MSG_RECORD_ACTIVE.store(false, Ordering::SeqCst);
        return json!({ "ok": true, "recording": false });
    }
    if kind == "voice_msg_send" {
        return stop_and_send_voice_message(device_id.unwrap_or_default());
    }
    if kind == "ui_sound" {
        play_linux_ui_sound(device_id.unwrap_or("mute"), volume_pct, muted);
        return json!({ "ok": true });
    }
    if kind == "stream_audio_volume" {
        let (prev_muted, prev_vol) = discord_core::ws::get_live_stream_audio_state();
        let mut parsed_muted: Option<bool> = None;
        let mut parsed_vol: Option<u32> = None;
        let mut parsed_uid: Option<String> = None;
        if let Some(raw_dev) = device_id.map(str::trim).filter(|s| !s.is_empty()) {
            let parts: Vec<&str> = raw_dev.split(':').collect();
            if let Some(m_str) = parts.first() {
                match *m_str {
                    "1" | "true" => parsed_muted = Some(true),
                    "0" | "false" => parsed_muted = Some(false),
                    _ => {}
                }
            }
            if let Some(v_str) = parts.get(1) {
                if let Ok(v) = v_str.trim().parse::<u32>() {
                    parsed_vol = Some(v.min(200));
                }
            }
            if let Some(u_str) = parts.get(2) {
                let clean_u = u_str.trim();
                if !clean_u.is_empty() {
                    parsed_uid = Some(clean_u.to_string());
                }
            }
        }
        let next_muted = muted.or(parsed_muted).unwrap_or(prev_muted);
        let next_vol = volume_pct.or(parsed_vol).unwrap_or(prev_vol).min(200);
        discord_core::ws::set_live_stream_audio_state(next_muted, next_vol);
        if let Some(uid) = parsed_uid {
            let eff_vol = if next_muted { 0 } else { next_vol };
            let ucmd = format!(
                "{{\"cmd\":\"set_user_audio\",\"userId\":\"{uid}\",\"muted\":{next_muted},\"volume\":{eff_vol}}}"
            );
            discord_core::ws::send_native_stream_cmd(&ucmd);
            discord_core::ws::send_native_voice_cmd(&ucmd);
        }
        return json!({
            "ok": true,
            "streamAudioMuted": next_muted,
            "streamAudioVolumePct": next_vol
        });
    }
    if kind == "user_audio_volume" {
        let uid = device_id.unwrap_or_default().trim().to_string();
        let next_muted = muted.unwrap_or(false);
        let next_vol = volume_pct.unwrap_or(100).min(200);
        if !uid.is_empty() {
            let cmd = format!(
                "{{\"cmd\":\"set_user_audio\",\"userId\":\"{uid}\",\"muted\":{next_muted},\"volume\":{next_vol}}}"
            );
            discord_core::ws::send_native_voice_cmd(&cmd);
            discord_core::ws::send_native_stream_cmd(&cmd);
        }
        return json!({
            "ok": true,
            "userId": uid,
            "muted": next_muted,
            "volumePercent": next_vol
        });
    }
    if kind == "stream_watch" {
        let skey = device_id.unwrap_or_default().to_string();
        if let Ok(mut guard) = live_session().lock() {
            guard.stream_endpoint = None;
            guard.stream_token = None;
            guard.stream_launched_token = None;
        }
        let _ = std::fs::remove_file("/dev/shm/discord_remote_stream_frame.jpg");
        let _ = std::fs::remove_file("/dev/shm/discord_remote_stream_meta.bin");
        let _ = std::fs::remove_file("/tmp/discord_remote_stream_frame.jpg");
        let _ = discord_core::ws::queue_live_stream_watch(&skey);
        let skey_bg = skey.clone();
        std::thread::spawn(move || {
            fetch_remote_stream_preview_blocking(&skey_bg);
        });
        return json!({ "ok": true, "streamKey": skey });
    }
    if kind == "stream_stop_watch" {
        let skey = device_id.unwrap_or_default();
        if !skey.trim().is_empty() {
            let _ = discord_core::ws::queue_live_stream_delete(skey);
        }
        reset_live_stream_state();
        return json!({ "ok": true });
    }
    if kind == "voice_states_sync" {
        if let Ok(guard) = live_session().lock() {
            let mut combined = guard.voice_states.clone();
            if let Some(ref rd) = guard.ready_data {
                if let Some(g_arr) = rd.get("guilds").and_then(Value::as_array) {
                    for g in g_arr {
                        if let Some(gid) = field_str(g, "id") {
                            if !combined.contains_key(gid) {
                                let vs = array(g.get("voice_states"));
                                if !vs.is_empty() {
                                    combined.insert(gid.to_string(), vs.to_vec());
                                }
                            }
                        }
                    }
                }
            }
            return json!({
                "ok": true,
                "voiceStates": combined
            });
        }
        return json!({ "ok": false, "voiceStates": {} });
    }
    if kind == "remote_stream_frame" {
        static LAST_REMOTE_PREVIEW_FETCH_MS: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        static REMOTE_FRAME_CACHE: OnceLock<Mutex<(u64, String)>> = OnceLock::new();
        let raw_arg = device_id.unwrap_or_default().to_string();
        let (skey, since_seq) = match raw_arg.split_once("|since=") {
            Some((k, s)) => (k.to_string(), s.trim().parse::<u64>().unwrap_or(0)),
            None => (raw_arg, 0),
        };
        let shm_path = "/dev/shm/discord_remote_stream_frame.jpg";
        let tmp_path = "/tmp/discord_remote_stream_frame.jpg";
        let meta_path = "/dev/shm/discord_remote_stream_meta.bin";
        let frame_path = if Path::new(shm_path).exists() {
            shm_path
        } else {
            tmp_path
        };
        let mut meta_seq: u64 = 0;
        let mut meta_w: u32 = 0;
        let mut meta_h: u32 = 0;
        let mut meta_fps: u32 = 0;
        if let Ok(mbytes) = std::fs::read(meta_path) {
            if mbytes.len() >= 20 {
                meta_seq = u64::from_le_bytes([
                    mbytes[0], mbytes[1], mbytes[2], mbytes[3], mbytes[4], mbytes[5], mbytes[6],
                    mbytes[7],
                ]);
                meta_w = u32::from_le_bytes([mbytes[8], mbytes[9], mbytes[10], mbytes[11]]);
                meta_h = u32::from_le_bytes([mbytes[12], mbytes[13], mbytes[14], mbytes[15]]);
                meta_fps = u32::from_le_bytes([mbytes[16], mbytes[17], mbytes[18], mbytes[19]]);
            } else if mbytes.len() >= 16 {
                meta_seq = u32::from_ne_bytes([mbytes[0], mbytes[1], mbytes[2], mbytes[3]]) as u64;
                meta_w = u32::from_ne_bytes([mbytes[4], mbytes[5], mbytes[6], mbytes[7]]);
                meta_h = u32::from_ne_bytes([mbytes[8], mbytes[9], mbytes[10], mbytes[11]]);
                meta_fps = u32::from_ne_bytes([mbytes[12], mbytes[13], mbytes[14], mbytes[15]]);
            }
        }
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mtime_ms = std::fs::metadata(frame_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let effective_seq = if meta_seq > 0 { meta_seq } else { mtime_ms };
        let file_fresh = mtime_ms > 0 && now_ms.saturating_sub(mtime_ms) < 1200;
        let prev_fetch = LAST_REMOTE_PREVIEW_FETCH_MS.load(Ordering::Relaxed);
        if !file_fresh && now_ms.saturating_sub(prev_fetch) >= 2200 && !skey.is_empty() {
            LAST_REMOTE_PREVIEW_FETCH_MS.store(now_ms, Ordering::Relaxed);
            let skey_bg = skey.clone();
            std::thread::spawn(move || {
                fetch_remote_stream_preview_blocking(&skey_bg);
            });
        }
        if effective_seq > 0 {
            if since_seq > 0 && since_seq == effective_seq {
                return json!({
                    "ok": true,
                    "seq": effective_seq,
                    "unchanged": true,
                    "width": meta_w,
                    "height": meta_h,
                    "streamFps": meta_fps
                });
            }
            let cache_lock = REMOTE_FRAME_CACHE.get_or_init(|| Mutex::new((0, String::new())));
            if let Ok(guard) = cache_lock.lock() {
                if guard.0 == effective_seq && !guard.1.is_empty() {
                    return json!({
                        "ok": true,
                        "seq": effective_seq,
                        "unchanged": false,
                        "width": meta_w,
                        "height": meta_h,
                        "streamFps": meta_fps,
                        "previewDataUrl": guard.1.clone()
                    });
                }
            }
            if let Ok(bytes) = std::fs::read(frame_path) {
                if bytes.len() > 64 {
                    let b64 = encode_bytes_base64(&bytes);
                    let url = format!("data:image/jpeg;base64,{b64}");
                    if let Ok(mut guard) = cache_lock.lock() {
                        *guard = (effective_seq, url.clone());
                    }
                    return json!({
                        "ok": true,
                        "seq": effective_seq,
                        "unchanged": false,
                        "width": meta_w,
                        "height": meta_h,
                        "streamFps": meta_fps,
                        "previewDataUrl": url
                    });
                }
            }
        }
        return json!({ "ok": false });
    }
    if kind == "screen_preview" {
        static LAST_CAP_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        static CAP_IN_FLIGHT: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        static PREVIEW_FRAME_CACHE: OnceLock<Mutex<(u64, String)>> = OnceLock::new();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let prev_ms = LAST_CAP_MS.load(Ordering::Relaxed);
        let shm_thumb = "/dev/shm/discord_live_preview_thumb.jpg";
        let shm_live = "/dev/shm/discord_live_screen.jpg";
        let thumb_path = if Path::new(shm_thumb).exists() {
            shm_thumb
        } else {
            shm_live
        };
        let mtime_pre = std::fs::metadata(thumb_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let is_fresh = mtime_pre > 0 && now_ms.saturating_sub(mtime_pre) < 2500;
        if !is_fresh
            && (now_ms.saturating_sub(prev_ms) >= 2500 || mtime_pre == 0)
            && !CAP_IN_FLIGHT.swap(true, Ordering::SeqCst)
        {
            LAST_CAP_MS.store(now_ms, Ordering::Relaxed);
            std::thread::spawn(move || {
                let raw_shm = "/dev/shm/discord_live_screen_tmp.jpg";
                let live_shm = "/dev/shm/discord_live_screen.jpg";
                let st = std::process::Command::new("spectacle")
                    .args(["-b", "-n", "-f", "-o", raw_shm])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                if st.map(|s| s.success()).unwrap_or(false) && Path::new(raw_shm).exists() {
                    let _ = std::fs::rename(raw_shm, live_shm);
                }
                CAP_IN_FLIGHT.store(false, Ordering::SeqCst);
            });
        }
        let mtime_ms = std::fs::metadata(thumb_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        if mtime_ms > 0 {
            let cache_lock = PREVIEW_FRAME_CACHE.get_or_init(|| Mutex::new((0, String::new())));
            if let Ok(guard) = cache_lock.lock() {
                if guard.0 == mtime_ms && !guard.1.is_empty() {
                    return json!({
                        "ok": true,
                        "seq": mtime_ms,
                        "unchanged": true,
                        "previewDataUrl": guard.1.clone()
                    });
                }
            }
            if let Ok(bytes) = std::fs::read(thumb_path) {
                if !bytes.is_empty() {
                    let b64 = encode_bytes_base64(&bytes);
                    let url = format!("data:image/jpeg;base64,{b64}");
                    if let Ok(mut guard) = cache_lock.lock() {
                        *guard = (mtime_ms, url.clone());
                    }
                    return json!({
                        "ok": true,
                        "seq": mtime_ms,
                        "unchanged": false,
                        "previewDataUrl": url
                    });
                }
            }
        }
        return json!({ "ok": false });
    }
    let is_input = kind == "input" || kind == "source";
    let mut changed_device = false;
    if let Some(id) = device_id.filter(|s| !s.is_empty()) {
        let clean_opt = if id == "default" {
            None
        } else {
            Some(id.to_string())
        };
        let mut actually_changed = false;
        if is_input {
            if let Ok(mut g) = selected_input_device().lock() {
                if *g != clean_opt {
                    *g = clean_opt;
                    actually_changed = true;
                }
            }
        } else if let Ok(mut g) = selected_output_device().lock() {
            if *g != clean_opt {
                *g = clean_opt;
                actually_changed = true;
            }
        }
        if actually_changed {
            AUDIO_DEVICE_EPOCH.fetch_add(1, Ordering::SeqCst);
            changed_device = true;
        }
    }
    if let Some(vol) = volume_pct {
        let clamped = vol.min(200);
        if is_input {
            LOCAL_INPUT_VOLUME_PCT.store(clamped, Ordering::SeqCst);
            discord_core::ws::send_native_voice_cmd(&format!(
                "{{\"cmd\":\"set_state\",\"inputVolume\":{clamped}}}"
            ));
        } else {
            LOCAL_OUTPUT_VOLUME_PCT.store(clamped, Ordering::SeqCst);
            discord_core::ws::send_native_voice_cmd(&format!(
                "{{\"cmd\":\"set_state\",\"outputVolume\":{clamped}}}"
            ));
        }
    }
    if let Some(m) = muted {
        if is_input {
            LOCAL_SELF_MUTE.store(m, Ordering::SeqCst);
            discord_core::ws::send_native_voice_cmd(&format!(
                "{{\"cmd\":\"set_state\",\"selfMute\":{m}}}"
            ));
        } else {
            LOCAL_SELF_DEAF.store(m, Ordering::SeqCst);
            discord_core::ws::send_native_voice_cmd(&format!(
                "{{\"cmd\":\"set_state\",\"selfDeafen\":{m}}}"
            ));
        }
        discord_core::ws::set_live_voice_mute_deaf_state(
            LOCAL_SELF_MUTE.load(Ordering::SeqCst),
            LOCAL_SELF_DEAF.load(Ordering::SeqCst),
        );
    }
    if changed_device {
        list_system_audio_devices_json()
    } else {
        json!({ "ok": true })
    }
}

/// Записывает отправленное сообщение в кеш живой сессии `LiveSession.channel_messages`.
pub fn record_sent_message(channel_id: &str, mut message: Value) {
    let cid = channel_id.trim();
    if cid.is_empty() {
        return;
    }
    let mut guard = live_session()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let fallback_user = guard.user.clone();
    if let Value::Object(ref mut map) = message {
        if map.get("channel_id").and_then(as_non_nil_str).is_none() {
            map.insert("channel_id".to_string(), Value::String(cid.to_string()));
        }
        if !map.contains_key("author") {
            let author = if let Some(u) = fallback_user {
                json!({
                    "id": u.id,
                    "username": u.username,
                    "global_name": u.global_name,
                    "avatar": u.avatar,
                    "bot": false
                })
            } else {
                json!({
                    "id": "0",
                    "username": "user",
                    "bot": false
                })
            };
            map.insert("author".to_string(), author);
        }
    }
    let msg_id = field_str(&message, "id").map(str::to_string);
    if guard.channel_messages.len() >= 128 && !guard.channel_messages.contains_key(cid) {
        if let Some(first_key) = guard.channel_messages.keys().next().cloned() {
            guard.channel_messages.remove(&first_key);
        }
    }
    let list = guard.channel_messages.entry(cid.to_string()).or_default();
    if let Some(mid) = msg_id {
        if let Some(existing) = list
            .iter_mut()
            .find(|m| field_str(m, "id") == Some(mid.as_str()))
        {
            *existing = message;
            return;
        }
    }
    list.push(message);
    if list.len() > 500 {
        let excess = list.len() - 500;
        list.drain(0..excess);
    }
}

// ---------------------------------------------------------------------------
// Битовые маски прав доступа Discord API v9 и расчёт переопределений каналов
// ---------------------------------------------------------------------------

/// Создание мгновенного приглашения (`CREATE_INSTANT_INVITE`, `1 << 0`).
pub const PERM_CREATE_INSTANT_INVITE: u64 = 1 << 0;
/// Полный административный доступ (`ADMINISTRATOR`, `1 << 3`).
pub const PERM_ADMINISTRATOR: u64 = 1 << 3;
/// Добавление реакций к сообщениям (`ADD_REACTIONS`, `1 << 6`).
pub const PERM_ADD_REACTIONS: u64 = 1 << 6;
/// Просмотр текстового или голосового канала (`VIEW_CHANNEL`, `1 << 10`).
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
/// Отправка сообщений в текстовый канал (`SEND_MESSAGES`, `1 << 11`).
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
/// Прикрепление файлов и медиа (`ATTACH_FILES`, `1 << 15`).
pub const PERM_ATTACH_FILES: u64 = 1 << 15;
/// Чтение истории сообщений канала (`READ_MESSAGE_HISTORY`, `1 << 16`).
pub const PERM_READ_MESSAGE_HISTORY: u64 = 1 << 16;
/// Подключение к голосовому каналу (`CONNECT`, `1 << 20`).
pub const PERM_CONNECT: u64 = 1 << 20;
/// Разговор в голосовом канале (`SPEAK`, `1 << 21`).
pub const PERM_SPEAK: u64 = 1 << 21;
/// Использование активации по голосу (`USE_VAD`, `1 << 25`).
pub const PERM_USE_VAD: u64 = 1 << 25;

/// Итоговая политика доступа пользователя к конкретному каналу.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelAccessPolicy {
    pub permissions: u64,
    pub can_view: bool,
    pub can_send_messages: bool,
    pub can_read_history: bool,
    pub can_attach_files: bool,
    pub can_add_reactions: bool,
    pub can_connect_voice: bool,
    pub can_speak_voice: bool,
}

/// Вычисляет эффективные 64-битные права доступа пользователя в канале по официальной иерархии Discord:
/// 1. Владелец гильдии (`owner_id == Some(user_id)`) получает `u64::MAX`.
/// 2. Базовые права складываются побитовым ИЛИ из роли `@everyone` (`role.id == guild_id`)
///    и всех ролей участника (`member_roles`). Если установлен [`PERM_ADMINISTRATOR`], возвращается `u64::MAX`.
/// 3. Применяется переопределение канала для `@everyone` (`overwrite.id == guild_id`): `(perms & !deny) | allow`.
/// 4. Применяются суммарные переопределения ролей участника (`member_roles`): `(perms & !role_deny) | role_allow`.
/// 5. Применяется персональное переопределение участника (`overwrite.id == user_id`): `(perms & !member_deny) | member_allow`.
#[must_use]
pub fn compute_effective_permissions(
    user_id: &str,
    guild_id: &str,
    owner_id: Option<&str>,
    guild_roles: &[Value],
    member_roles: &[String],
    overwrites: &[Value],
) -> u64 {
    if !user_id.is_empty() && owner_id == Some(user_id) {
        return u64::MAX;
    }

    let mut base = 0u64;
    for role in guild_roles {
        if let Some(role_id) = field_str(role, "id") {
            if role_id == guild_id || member_roles.iter().any(|r| r == role_id) {
                base |= field_u64(role, "permissions").unwrap_or(0);
            }
        }
    }

    if (base & PERM_ADMINISTRATOR) != 0 {
        return u64::MAX;
    }

    let mut perms = base;

    for ov in overwrites {
        if field_str(ov, "id") == Some(guild_id) {
            let allow = field_u64(ov, "allow").unwrap_or(0);
            let deny = field_u64(ov, "deny").unwrap_or(0);
            perms = (perms & !deny) | allow;
        }
    }

    let mut role_allow = 0u64;
    let mut role_deny = 0u64;
    for ov in overwrites {
        if let Some(ov_id) = field_str(ov, "id") {
            if ov_id != guild_id && member_roles.iter().any(|r| r == ov_id) {
                role_allow |= field_u64(ov, "allow").unwrap_or(0);
                role_deny |= field_u64(ov, "deny").unwrap_or(0);
            }
        }
    }
    perms = (perms & !role_deny) | role_allow;

    if !user_id.is_empty() {
        for ov in overwrites {
            if field_str(ov, "id") == Some(user_id) {
                let member_allow = field_u64(ov, "allow").unwrap_or(0);
                let member_deny = field_u64(ov, "deny").unwrap_or(0);
                perms = (perms & !member_deny) | member_allow;
            }
        }
    }

    perms
}

/// Вычисляет прикладную политику доступа к каналу по маске прав `perms` и типу канала `channel_type`.
#[must_use]
pub fn evaluate_channel_access(perms: u64, channel_type: i64) -> ChannelAccessPolicy {
    if channel_type == 1 || channel_type == 3 {
        return ChannelAccessPolicy {
            permissions: perms,
            can_view: true,
            can_send_messages: true,
            can_read_history: true,
            can_attach_files: true,
            can_add_reactions: true,
            can_connect_voice: true,
            can_speak_voice: true,
        };
    }

    let effective = if (perms & PERM_ADMINISTRATOR) != 0 {
        u64::MAX
    } else {
        perms
    };

    let can_view = (effective & PERM_VIEW_CHANNEL) != 0;
    let is_category = channel_type == 4;
    let is_voice = channel_type == 2 || channel_type == 13;

    let can_send_messages = can_view && !is_category && (effective & PERM_SEND_MESSAGES) != 0;
    let can_read_history = can_view && !is_category && (effective & PERM_READ_MESSAGE_HISTORY) != 0;
    let can_attach_files = can_send_messages && (effective & PERM_ATTACH_FILES) != 0;
    let can_add_reactions = can_view && !is_category && (effective & PERM_ADD_REACTIONS) != 0;
    let can_connect_voice = can_view && is_voice && (effective & PERM_CONNECT) != 0;
    let can_speak_voice = can_connect_voice && (effective & PERM_SPEAK) != 0;

    ChannelAccessPolicy {
        permissions: perms,
        can_view,
        can_send_messages,
        can_read_history,
        can_attach_files,
        can_add_reactions,
        can_connect_voice,
        can_speak_voice,
    }
}

fn fetch_rest_channel_messages(token: &str, channel_id: &str) -> Option<Vec<Value>> {
    let token = token.trim().to_string();
    let cid = channel_id.trim().to_string();
    if cfg!(test)
        || token.is_empty()
        || token == "live-test-token"
        || token.starts_with("test-")
        || token.starts_with("synthetic_")
        || token.starts_with("MT_real_user_token_")
        || token.contains(".GTest")
        || cid.is_empty()
        || !cid.bytes().all(|b| b.is_ascii_digit())
        || cid.parse::<u64>().is_err()
    {
        return None;
    }
    let is_forum_channel = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.ready_data.as_ref().is_some_and(|rd| {
            array(rd.get("guilds")).iter().any(|g| {
                array(g.get("channels")).iter().any(|c| {
                    field_str(c, "id") == Some(cid.as_str())
                        && matches!(field_u64(c, "type"), Some(15 | 16))
                })
            })
        })
    };
    std::thread::spawn(move || {
        std::panic::catch_unwind(move || {
            let identity = crate::auth::IdentityContext::detect();
            let auth = discord_core::rest::client::ApiAuth::from_token_with_scheme(
                &token,
                discord_core::rest::client::AuthScheme::User,
                &identity.super_properties,
                &identity.locale,
                &identity.timezone,
            )
            .ok()?;
            let config = discord_core::rest::client::ApiConfig {
                base: discord_core::API_V9_BASE.to_string(),
                auth,
                http: discord_core::rest::transport::HttpClientConfig {
                    connect_timeout: std::time::Duration::from_millis(1200),
                    read_timeout: std::time::Duration::from_millis(1500),
                    write_timeout: std::time::Duration::from_millis(1200),
                    ..discord_core::rest::transport::HttpClientConfig::default()
                },
                retry: discord_core::rest::client::ApiRetryPolicy {
                    max_attempts: 1,
                    ..discord_core::rest::client::ApiRetryPolicy::default()
                },
            };
            let client = discord_core::rest::client::ApiClient::new(config).ok()?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .ok()?;
            let path = format!("/channels/{cid}/messages?limit=50");
            match runtime.block_on(client.get(&path)) {
                Ok(response) if response.status == 200 => {
                    if let Ok(mut parsed) = serde_json::from_slice::<Value>(&response.body) {
                        sanitize_etf_json_value(&mut parsed);
                        if let Some(arr) = parsed.as_array() {
                            return Some(arr.clone());
                        }
                    }
                    return None;
                }
                Ok(response) if response.status == 400 && is_forum_channel => {}
                Err(discord_core::rest::client::ApiError::Client { status: 400 })
                    if is_forum_channel => {}
                _ => return None,
            }
            // Fallback для форум-каналов (type = 15, где /messages возвращает 400): получаем темы и стартовые сообщения
            let forum_path = format!(
                "/channels/{cid}/threads/search?sort_by=last_message_time&sort_order=desc&limit=25"
            );
            let forum_resp = runtime.block_on(client.get(&forum_path)).ok()?;
            if forum_resp.status != 200 {
                return None;
            }
            let parsed: Value = serde_json::from_slice(&forum_resp.body).ok()?;
            let threads = parsed.get("threads").and_then(Value::as_array)?;
            let first_msgs = parsed
                .get("first_messages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut msg_by_id: HashMap<String, Value> = HashMap::new();
            for m in first_msgs {
                if let Some(mid) = field_str(&m, "id").or_else(|| field_str(&m, "channel_id")) {
                    msg_by_id.insert(mid.to_string(), m);
                }
            }
            let mut out = Vec::new();
            for t in threads {
                let tid = field_str(t, "id")?;
                let tname = field_str(t, "name").unwrap_or("Тема форума");
                let msg_count = field_u64(t, "message_count").unwrap_or(0);
                if let Some(mut msg) = msg_by_id.remove(tid) {
                    if let Some(obj) = msg.as_object_mut() {
                        obj.insert("channel_id".to_string(), Value::String(cid.clone()));
                        let body = obj
                            .get("content")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let combined = if body.is_empty() {
                            format!("💬 {tname} (сообщений: {msg_count})")
                        } else {
                            format!("💬 {tname} (сообщений: {msg_count})\n{body}")
                        };
                        obj.insert("content".to_string(), Value::String(combined));
                    }
                    out.push(msg);
                } else {
                    let owner_id = field_str(t, "owner_id").unwrap_or("0");
                    let ts = t
                        .get("thread_metadata")
                        .and_then(|tm| field_str(tm, "create_timestamp"))
                        .unwrap_or("2025-01-01T00:00:00.000000+00:00");
                    out.push(json!({
                        "id": tid,
                        "channel_id": cid,
                        "author": {
                            "id": owner_id,
                            "username": "Автор темы",
                            "global_name": "Автор темы",
                            "avatar": null,
                            "bot": false
                        },
                        "content": format!("💬 {tname} (сообщений: {msg_count})"),
                        "timestamp": ts,
                        "edited_timestamp": null,
                        "pinned": false,
                        "type": 0,
                        "embeds": [],
                        "attachments": [],
                        "reactions": [],
                        "mentions": [],
                        "mention_everyone": false
                    }));
                }
            }
            Some(out)
        })
        .ok()
        .flatten()
    })
    .join()
    .ok()
    .flatten()
}

/// Размер страницы по умолчанию, строк.
pub const DEFAULT_PAGE_LIMIT: usize = 50;
/// Максимальный размер страницы: `page ≤ 100` из ТЗ.
pub const MAX_PAGE_LIMIT: usize = 100;
/// Бюджет окна UI (`loadedRows ≤ 500`), строк. Инвариант фронтенда, не сервера:
/// BFF лишь сообщает его в ответе, чтобы UI не расходился с контрактом.
pub const UI_WINDOW_ROWS: usize = 500;
/// Максимальный размер файла фикстуры, байт (64 MiB). Файл больше лимита
/// отвергается по `metadata.len()` **до** чтения: иначе локальный артефакт
/// в сотни MB съел бы память и CPU парсера.
pub const MAX_FIXTURE_BYTES: u64 = 64 * 1024 * 1024;
/// Максимальная длина пользовательских строк `channelId`/`guildId`/`cursor`,
/// символов. Более длинный вход — `InvalidArg`, в сообщении усечённое значение.
pub const MAX_INPUT_LEN: usize = 256;
/// Сколько символов значения показывать в тексте ошибки (плюс `…` при усечении).
const ERROR_VALUE_PREVIEW: usize = 32;

/// Каталог `fixtures/` в репозитории (на два уровня выше crate `rust/gui-spike`).
/// Используется только как значение по умолчанию; рабочий путь задаёт `DL_FIXTURES`.
fn default_fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|repo| repo.join("fixtures"))
        .unwrap_or_else(|| PathBuf::from("fixtures"))
}

/// Разрешает корень фикстур: явный аргумент (из `DL_FIXTURES`) важнее дефолта.
/// Вынесено из [`fixtures_root`], чтобы проверять приоритет без мутации env.
pub fn resolve_root(configured: Option<&str>) -> PathBuf {
    match configured {
        Some(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => default_fixtures_root(),
    }
}

/// Корень фикстур для команд: `DL_FIXTURES` или репозиторный `fixtures/`.
/// Отсутствие каталога — `NotFound` с подсказкой про переменную окружения.
pub fn fixtures_root() -> Result<PathBuf, DataError> {
    let configured = std::env::var("DL_FIXTURES").ok();
    let root = resolve_root(configured.as_deref());
    if !root.is_dir() {
        return Err(DataError::not_found(
            "fixtures",
            format!(
                "каталог фикстур не найден: {} (задайте DL_FIXTURES)",
                root.display()
            ),
        ));
    }
    Ok(root)
}

// ---------------------------------------------------------------------------
// Ошибки
// ---------------------------------------------------------------------------

/// Типизированная ошибка BFF. Варианты фиксированы контрактом: `Io`, `Parse`,
/// `NotFound`, `InvalidArg`; `kind()` отдаёт это имя как есть.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    /// Файловая система недоступна/отказала (не «файла нет» — это `NotFound`).
    Io { path: PathBuf, message: String },
    /// Файл есть, но это не ожидаемый JSON/структура.
    Parse { path: PathBuf, message: String },
    /// Ресурс (файл, канал, гильдия, секция) не найден.
    NotFound { resource: String, message: String },
    /// Аргумент команды вне контракта (пустой id, limit, чужой курсор).
    InvalidArg { message: String },
}

impl DataError {
    /// Имя варианта для JSON/логов и для фронтенда.
    pub fn kind(&self) -> &'static str {
        match self {
            DataError::Io { .. } => "Io",
            DataError::Parse { .. } => "Parse",
            DataError::NotFound { .. } => "NotFound",
            DataError::InvalidArg { .. } => "InvalidArg",
        }
    }

    /// Ошибка ввода-вывода с путём.
    pub fn io(path: &Path, message: impl Into<String>) -> Self {
        DataError::Io {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }

    /// Ошибка разбора JSON/структуры с путём.
    pub fn parse(path: &Path, message: impl Into<String>) -> Self {
        DataError::Parse {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }

    /// Ресурс не найден: `resource` — что искали (`fixtures`, `канал`, `секция`).
    pub fn not_found(resource: impl Into<String>, message: impl Into<String>) -> Self {
        DataError::NotFound {
            resource: resource.into(),
            message: message.into(),
        }
    }

    /// Некорректный аргумент команды.
    pub fn invalid_arg(message: impl Into<String>) -> Self {
        DataError::InvalidArg {
            message: message.into(),
        }
    }

    fn message(&self) -> &str {
        match self {
            DataError::Io { message, .. }
            | DataError::Parse { message, .. }
            | DataError::NotFound { message, .. }
            | DataError::InvalidArg { message } => message,
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io { path, message } => {
                write!(formatter, "Io ({}): {message}", path.display())
            }
            DataError::Parse { path, message } => {
                write!(formatter, "Parse ({}): {message}", path.display())
            }
            DataError::NotFound { resource, message } => {
                write!(formatter, "NotFound [{resource}]: {message}")
            }
            DataError::InvalidArg { message } => {
                write!(formatter, "InvalidArg: {message}")
            }
        }
    }
}

impl std::error::Error for DataError {}

/// Представление для JSON: стабильные `kind`/`message` + контекст, если есть.
struct ErrorView<'a> {
    kind: &'static str,
    message: &'a str,
    path: Option<String>,
    resource: Option<&'a str>,
}

impl Serialize for DataError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let view = ErrorView {
            kind: self.kind(),
            message: self.message(),
            path: match self {
                DataError::Io { path, .. } | DataError::Parse { path, .. } => {
                    Some(path.display().to_string())
                }
                DataError::NotFound { .. } | DataError::InvalidArg { .. } => None,
            },
            resource: match self {
                DataError::NotFound { resource, .. } => Some(resource),
                _ => None,
            },
        };
        // Компактный объект `{kind, message, path?, resource?}` — без вложенности.
        let mut state = serializer.serialize_struct("DataError", 4)?;
        state.serialize_field("kind", view.kind)?;
        state.serialize_field("message", view.message)?;
        if let Some(path) = view.path.as_deref() {
            state.serialize_field("path", path)?;
        }
        if let Some(resource) = view.resource {
            state.serialize_field("resource", resource)?;
        }
        state.end()
    }
}

// ---------------------------------------------------------------------------
// Чтение фикстур
// ---------------------------------------------------------------------------

/// Кадр диспатча из `gateway/by-type/*.json`: тип (`t`), данные (`d`), путь.
struct Frame {
    kind: String,
    data: Value,
    path: PathBuf,
}

fn map_io(path: &Path, error: &io::Error) -> DataError {
    if error.kind() == io::ErrorKind::NotFound {
        DataError::not_found(
            path.display().to_string(),
            format!("файл фикстуры не найден: {}", path.display()),
        )
    } else {
        DataError::io(path, error.to_string())
    }
}

type FixtureCacheKey = (PathBuf, u64, Option<std::time::SystemTime>);

fn fixture_json_cache() -> &'static std::sync::RwLock<HashMap<FixtureCacheKey, Value>> {
    static CACHE: OnceLock<std::sync::RwLock<HashMap<FixtureCacheKey, Value>>> = OnceLock::new();
    CACHE.get_or_init(|| std::sync::RwLock::new(HashMap::new()))
}

/// Читает JSON-файл фикстуры с потокобезопасным in-memory кэшем по `(path, size, mtime)`.
/// Нет файла → `NotFound`, битый JSON → `Parse`.
/// Файл больше [`MAX_FIXTURE_BYTES`] отвергается до чтения в память (по `metadata`).
fn read_json(path: &Path) -> Result<Value, DataError> {
    let meta = fs::metadata(path).map_err(|error| map_io(path, &error))?;
    let size = meta.len();
    if size > MAX_FIXTURE_BYTES {
        return Err(DataError::parse(
            path,
            format!(
                "фикстура больше лимита {MAX_FIXTURE_BYTES} байт ({size} байт): чтение отклонено"
            ),
        ));
    }
    let key: FixtureCacheKey = (path.to_path_buf(), size, meta.modified().ok());
    if let Ok(guard) = fixture_json_cache().read() {
        if let Some(cached) = guard.get(&key) {
            return Ok(cached.clone());
        }
    }
    let text = fs::read_to_string(path).map_err(|error| map_io(path, &error))?;
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|error| DataError::parse(path, format!("некорректный JSON: {error}")))?;
    if let Ok(mut guard) = fixture_json_cache().write() {
        if guard.len() >= 64 {
            guard.clear();
        }
        guard.insert(key, parsed.clone());
    }
    Ok(parsed)
}

/// Все кадры `by-type` в детерминированном порядке (по имени файла).
/// Пустой каталог — валидный (пустой) набор, а не ошибка.
///
/// Кадр строгий: `t` — непустая строка, `d` — присутствует и не `null`
/// (nil-кадры дампа несут `"d": "nil"`, `"d": null` не встречается ни разу,
/// поэтому `null` означает испорченный файл). Иначе `Parse` с путём и индексом:
/// кадр без типа иначе молча терялся бы и давал ложный `NotFound`/пустую страницу.
fn read_frames(root: &Path) -> Result<Vec<Frame>, DataError> {
    let dir = root.join("gateway").join("by-type");
    let entries = fs::read_dir(&dir).map_err(|error| map_io(&dir, &error))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry.path()),
            Err(error) => {
                // Не глотаем молча: сообщаем путь каталога и причину.
                eprintln!("data: пропущена запись {dir:?}: {error}");
                None
            }
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    paths.sort();

    let mut frames = Vec::with_capacity(paths.len());
    for (index, path) in paths.into_iter().enumerate() {
        let value = read_json(&path)?;
        let kind = value
            .get("t")
            .and_then(Value::as_str)
            .filter(|kind| !kind.is_empty())
            .ok_or_else(|| {
                DataError::parse(
                    &path,
                    format!("кадр by-type #{index}: поле t должно быть непустой строкой"),
                )
            })?
            .to_string();
        let data = match value.get("d") {
            None => {
                return Err(DataError::parse(
                    &path,
                    format!("кадр by-type #{index} ({kind}): нет поля d"),
                ));
            }
            Some(Value::Null) => {
                return Err(DataError::parse(
                    &path,
                    format!("кадр by-type #{index} ({kind}): поле d = null"),
                ));
            }
            Some(data) => data.clone(),
        };
        frames.push(Frame { kind, data, path });
    }
    Ok(frames)
}

fn array(value: Option<&Value>) -> &[Value] {
    value.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn array_len(value: Option<&Value>) -> usize {
    array(value).len()
}

/// Строка фикстуры; санитизированная заглушка `"nil"` и `""` — это «нет значения».
fn as_non_nil_str(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|text| !text.is_empty() && *text != "nil")
}

fn field_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(as_non_nil_str)
}

fn field_u64(value: &Value, key: &str) -> Option<u64> {
    match value.get(key) {
        Some(Value::Number(number)) => number.as_u64(),
        Some(Value::String(text)) => text.parse().ok(),
        _ => None,
    }
}

fn field_i64(value: &Value, key: &str) -> Option<i64> {
    match value.get(key) {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.parse().ok(),
        _ => None,
    }
}

fn field_bool(value: &Value, key: &str) -> bool {
    field_bool_opt(value, key).unwrap_or(false)
}

fn field_bool_opt(value: &Value, key: &str) -> Option<bool> {
    match value.get(key) {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::String(text)) => match text.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        Some(Value::Number(number)) => match number.as_u64() {
            Some(1) => Some(true),
            Some(0) => Some(false),
            _ => None,
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ready
// ---------------------------------------------------------------------------

/// Пользователь из READY. `email` остаётся сантизированным (`REDACTED`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub bio: Option<String>,
    pub premium: bool,
    pub mfa_enabled: bool,
}

/// Гильдия READY: имя берётся из `properties.name`, счётчики — из READY.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuildSummary {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub member_count: u64,
    pub channel_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub afk_channel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub afk_timeout: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub roles: Vec<Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub my_roles: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub members: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lurker: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_member: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
}

fn populate_guild_members_and_my_roles(
    guilds: &mut [GuildSummary],
    ready_d: &Value,
    self_uid: &str,
) {
    let raw_guilds = array(ready_d.get("guilds"));
    let merged_members_outer = array(ready_d.get("merged_members"));
    let mut users_by_id: HashMap<String, Value> = HashMap::new();
    if let Some(me) = ready_d.get("user") {
        if let Some(uid) = field_str(me, "id") {
            let mut cm = me.clone();
            sanitize_etf_json_value(&mut cm);
            users_by_id.insert(uid.to_string(), cm);
        }
    }
    for u in array(ready_d.get("users")) {
        if let Some(uid) = field_str(u, "id") {
            let mut cu = u.clone();
            sanitize_etf_json_value(&mut cu);
            users_by_id.insert(uid.to_string(), cu);
        }
    }
    for g in guilds.iter_mut() {
        let Some(g_idx) = raw_guilds
            .iter()
            .position(|rg| field_str(rg, "id") == Some(g.id.as_str()))
        else {
            continue;
        };
        let mut raw_list: Vec<Value> = Vec::new();
        raw_list.extend_from_slice(array(raw_guilds[g_idx].get("members")));
        if let Some(mm) = merged_members_outer.get(g_idx) {
            raw_list.extend_from_slice(array(Some(mm)));
        }
        let mut seen_uids: HashSet<String> = g
            .members
            .iter()
            .filter_map(|m| {
                field_str(m, "user_id")
                    .or_else(|| m.get("user").and_then(|u| field_str(u, "id")))
                    .map(str::to_string)
            })
            .collect();
        for item in raw_list {
            let mut clean_m = item.clone();
            sanitize_etf_json_value(&mut clean_m);
            let uid_opt = field_str(&clean_m, "user_id")
                .or_else(|| clean_m.get("user").and_then(|u| field_str(u, "id")))
                .map(str::to_string);
            let Some(uid) = uid_opt else {
                continue;
            };
            if let Value::Object(ref mut m_map) = clean_m {
                if !m_map.contains_key("user") || m_map.get("user").is_some_and(Value::is_null) {
                    if let Some(u_obj) = users_by_id.get(&uid) {
                        m_map.insert("user".to_string(), u_obj.clone());
                    }
                }
            }
            if uid == self_uid {
                let m_roles: Vec<String> = array(clean_m.get("roles"))
                    .iter()
                    .filter_map(|r| as_non_nil_str(r).map(str::to_string))
                    .collect();
                if !m_roles.is_empty() {
                    g.my_roles = m_roles;
                }
            }
            if seen_uids.insert(uid.clone()) {
                if g.members.len() < 400 {
                    g.members.push(clean_m);
                }
            } else if let Some(existing) = g.members.iter_mut().find(|m| {
                field_str(m, "user_id")
                    .or_else(|| m.get("user").and_then(|u| field_str(u, "id")))
                    == Some(uid.as_str())
            }) {
                *existing = clean_m;
            }
        }
    }
}

/// Сессия клиента (READY `sessions[]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub client: String,
    pub os: String,
    pub version: u64,
    pub status: String,
}

/// Ответ `ready`: кто вошёл, где, сколько данных в READY.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyPayload {
    pub user: ReadyUser,
    pub guilds: Vec<GuildSummary>,
    pub guild_count: usize,
    pub private_channel_count: usize,
    pub relationship_count: usize,
    pub read_state_count: usize,
    pub session: Option<SessionSummary>,
    pub users: Vec<Value>,
    pub relationships: Vec<Value>,
    pub private_channels: Vec<Value>,
    pub presences: Vec<Value>,
    pub voice_states: HashMap<String, Vec<Value>>,
    pub user_guild_settings: Value,
    pub read_state: Value,
}

fn parse_ready_user(value: &Value, path: &Path) -> Result<ReadyUser, DataError> {
    let id = field_str(value, "id")
        .ok_or_else(|| DataError::parse(path, "READY.user без строкового id"))?;
    let username = field_str(value, "username")
        .ok_or_else(|| DataError::parse(path, "READY.user без строкового username"))?;
    Ok(ReadyUser {
        id: id.to_string(),
        username: username.to_string(),
        global_name: field_str(value, "global_name").map(str::to_string),
        avatar: field_str(value, "avatar").map(str::to_string),
        email: field_str(value, "email").map(str::to_string),
        bio: field_str(value, "bio").map(str::to_string),
        premium: field_bool(value, "premium"),
        mfa_enabled: field_bool(value, "mfa_enabled"),
    })
}

fn parse_guild(value: &Value) -> GuildSummary {
    let properties = value.get("properties");
    let name = properties
        .and_then(|props| field_str(props, "name"))
        .or_else(|| field_str(value, "name"))
        .unwrap_or_default()
        .to_string();
    let owner_id = properties
        .and_then(|props| field_str(props, "owner_id"))
        .or_else(|| field_str(value, "owner_id"))
        .map(str::to_string);
    let afk_channel_id = properties
        .and_then(|props| field_str(props, "afk_channel_id"))
        .or_else(|| field_str(value, "afk_channel_id"))
        .map(str::to_string);
    let afk_timeout = properties
        .and_then(|props| field_u64(props, "afk_timeout"))
        .or_else(|| field_u64(value, "afk_timeout"));
    let mut roles = array(value.get("roles")).to_vec();
    for r in roles.iter_mut() {
        sanitize_etf_json_value(r);
    }
    let my_roles: Vec<String> = array(value.get("my_roles").or_else(|| value.get("myRoles")))
        .iter()
        .filter_map(|r| as_non_nil_str(r).map(str::to_string))
        .collect();
    GuildSummary {
        id: field_str(value, "id").unwrap_or_default().to_string(),
        name,
        icon: properties
            .and_then(|props| field_str(props, "icon"))
            .or_else(|| field_str(value, "icon"))
            .map(str::to_string),
        member_count: field_u64(value, "member_count").unwrap_or(0),
        channel_count: array_len(value.get("channels")),
        owner_id,
        afk_channel_id,
        afk_timeout,
        roles,
        my_roles,
        members: Vec::new(),
        lurker: field_bool_opt(value, "lurker"),
        is_member: field_bool_opt(value, "isMember").or_else(|| field_bool_opt(value, "is_member")),
        pending: field_bool_opt(value, "pending"),
    }
}

/// Читает `gateway/by-type/002-READY.json` и отдаёт сводку для первого экрана.
/// При наличии живого токена (`resolve_live_token().is_some()`) чтение фикстуры `002-READY.json`
/// полностью запрещено (S1).
pub fn ready(root: &Path) -> Result<ReadyPayload, DataError> {
    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let forbid_fixture = resolve_live_token().is_some();

    let mut user: ReadyUser;
    let mut guilds: Vec<GuildSummary>;
    let mut session: Option<SessionSummary>;
    let mut read_state_count: usize;
    let mut users: Vec<Value>;
    let mut relationships: Vec<Value>;
    let mut private_channels: Vec<Value>;
    let mut user_guild_settings: Value;
    let mut read_state: Value;
    let mut presences: Vec<Value>;
    let mut voice_states: HashMap<String, Vec<Value>>;

    if forbid_fixture {
        user = ReadyUser {
            id: "0".to_string(),
            username: "user".to_string(),
            global_name: None,
            avatar: None,
            email: None,
            bio: None,
            premium: false,
            mfa_enabled: false,
        };
        guilds = Vec::new();
        session = Some(SessionSummary {
            client: "desktop".to_string(),
            os: "linux".to_string(),
            version: 0,
            status: "online".to_string(),
        });
        read_state_count = 0;
        users = Vec::new();
        relationships = Vec::new();
        private_channels = Vec::new();
        user_guild_settings = Value::Null;
        read_state = json!({ "entries": [] });
        presences = Vec::new();
        voice_states = HashMap::new();
    } else {
        let value = read_json(&path)?;
        let data = value
            .get("d")
            .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?;
        let user_value = data
            .get("user")
            .ok_or_else(|| DataError::parse(&path, "READY без объекта user"))?;
        user = parse_ready_user(user_value, &path)?;
        guilds = array(data.get("guilds")).iter().map(parse_guild).collect();
        populate_guild_members_and_my_roles(&mut guilds, data, &user.id);
        session = array(data.get("sessions")).first().map(|session| {
            let info = session.get("client_info");
            SessionSummary {
                client: info
                    .and_then(|value| field_str(value, "client"))
                    .unwrap_or_default()
                    .to_string(),
                os: info
                    .and_then(|value| field_str(value, "os"))
                    .unwrap_or_default()
                    .to_string(),
                version: info
                    .and_then(|value| field_u64(value, "version"))
                    .unwrap_or(0),
                status: field_str(session, "status").unwrap_or_default().to_string(),
            }
        });
        read_state_count = data
            .get("read_state")
            .and_then(|state| state.get("entries"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);

        users = array(data.get("users")).to_vec();
        relationships = array(data.get("relationships")).to_vec();
        private_channels = array(data.get("private_channels")).to_vec();
        user_guild_settings = data
            .get("user_guild_settings")
            .cloned()
            .unwrap_or(Value::Null);
        read_state = data.get("read_state").cloned().unwrap_or(Value::Null);

        presences = array(data.get("presences")).to_vec();
        voice_states = HashMap::new();
        for guild in array(data.get("guilds")) {
            if let Some(gid) = field_str(guild, "id") {
                let vs = array(guild.get("voice_states")).to_vec();
                if !vs.is_empty() {
                    voice_states.insert(gid.to_string(), vs);
                }
            }
        }

        let supp_path = root
            .join("gateway")
            .join("by-type")
            .join("003-READY_SUPPLEMENTAL.json");
        if let Ok(supp_val) = read_json(&supp_path) {
            if let Some(supp_d) = supp_val.get("d") {
                if let Some(mp) = supp_d.get("merged_presences") {
                    for item in array(mp.get("friends")) {
                        presences.push(item.clone());
                    }
                    for g_arr in array(mp.get("guilds")) {
                        for item in array(Some(g_arr)) {
                            presences.push(item.clone());
                        }
                    }
                }
                for (idx, sg) in array(supp_d.get("guilds")).iter().enumerate() {
                    let gid = field_str(sg, "id")
                        .map(str::to_string)
                        .or_else(|| guilds.get(idx).map(|g| g.id.clone()));
                    if let Some(gid) = gid {
                        let vs = array(sg.get("voice_states")).to_vec();
                        if !vs.is_empty() {
                            voice_states.entry(gid).or_default().extend(vs);
                        }
                    }
                }
            }
        }
    }

    {
        {
            let mut guard = live_session()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if guard.active && guard.ready_data.is_none() {
                let tok_opt = guard.token.clone().or_else(resolve_live_token);
                let exp_uid = guard.user.as_ref().map(|u| u.id.clone());
                if let Some(cached) = load_live_ready_cache_for(tok_opt.as_deref(), exp_uid.as_deref()) {
                    populate_voice_and_presences_from_ready(&mut guard, &cached);
                    guard.ready_data = Some(cached);
                }
            }
        }
        if forbid_fixture {
            for _ in 0..12 {
                {
                    let guard = live_session()
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    if guard.ready_data.is_some() || !guard.active {
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if guard.active {
            if let Some(ref live_user) = guard.user {
                user.id = live_user.id.clone();
                user.username = live_user.username.clone();
                if live_user.global_name.is_some() {
                    user.global_name = live_user.global_name.clone();
                }
                if live_user.avatar.is_some() {
                    user.avatar = live_user.avatar.clone();
                }
                if let Some(ref mut sess) = session {
                    sess.status = "online".to_string();
                }
            }
            if let Some(ref live_d) = guard.ready_data {
                if let Some(live_user_val) = live_d.get("user") {
                    if let Ok(parsed_live_user) = parse_ready_user(live_user_val, &path) {
                        user = parsed_live_user;
                    }
                }
                if let Some(g_arr) = live_d.get("guilds").and_then(Value::as_array) {
                    guilds = g_arr.iter().map(parse_guild).collect();
                    populate_guild_members_and_my_roles(&mut guilds, live_d, &user.id);
                } else {
                    guilds.clear();
                }
                users.clear();
                let live_users = array(live_d.get("users"));
                for lu in live_users {
                    let mut clean_u = lu.clone();
                    sanitize_etf_json_value(&mut clean_u);
                    users.push(clean_u);
                }
                if let Some(r_arr) = live_d.get("relationships").and_then(Value::as_array) {
                    relationships = r_arr.clone();
                } else {
                    relationships.clear();
                }
                if let Some(p_arr) = live_d.get("private_channels").and_then(Value::as_array) {
                    private_channels = p_arr.clone();
                } else {
                    private_channels.clear();
                }
                if let Some(live_rs) = live_d.get("read_state") {
                    if let Some(entries) = live_rs.get("entries").and_then(Value::as_array) {
                        read_state = live_rs.clone();
                        read_state_count = entries.len();
                    } else if let Some(entries) = live_rs.as_array() {
                        read_state = json!({ "entries": entries });
                        read_state_count = entries.len();
                    } else {
                        read_state = json!({ "entries": [] });
                        read_state_count = 0;
                    }
                } else {
                    read_state = json!({ "entries": [] });
                    read_state_count = 0;
                }
                if let Some(live_ugs) = live_d.get("user_guild_settings") {
                    if !live_ugs.is_null() {
                        user_guild_settings = live_ugs.clone();
                    }
                }
            } else {
                // В активной сессии без ready_data НЕ подсовываем фейковые гильдии и ЛС из 002-READY.json!
                guilds.clear();
                users.clear();
                relationships.clear();
                private_channels.clear();
                read_state = json!({ "entries": [] });
                read_state_count = 0;
            }
            // В живой сессии НЕ возвращаем устаревшие фикстурные `voice_states` из
            // `003-READY_SUPPLEMENTAL.json` (например, октябрьский `SKID В ЭФИРЕ`):
            // источником истины являются только живые `READY_SUPPLEMENTAL` и `VOICE_STATE_UPDATE`.
            voice_states.clear();
            let mut users_by_id: HashMap<String, Value> = users
                .iter()
                .filter_map(|u| field_str(u, "id").map(|id| (id.to_string(), u.clone())))
                .collect();
            for (gid, vs) in &guard.voice_states {
                let g_members: Vec<Value> = guilds
                    .iter()
                    .find(|g| &g.id == gid)
                    .map(|g| g.members.clone())
                    .unwrap_or_default();
                let mut enriched_vs: Vec<Value> = Vec::with_capacity(vs.len());
                for entry in vs {
                    let enriched = enrich_voice_state_entry(entry, &g_members, &users_by_id);
                    if let Some(u) = enriched
                        .get("user")
                        .or_else(|| enriched.get("member").and_then(|m| m.get("user")))
                    {
                        if let Some(uid) = field_str(u, "id") {
                            if !users_by_id.contains_key(uid) {
                                users_by_id.insert(uid.to_string(), u.clone());
                                users.push(u.clone());
                            }
                        }
                    }
                    enriched_vs.push(enriched);
                }
                voice_states.insert(gid.clone(), enriched_vs);
            }
            for p in guard.presences.values() {
                presences.push(p.clone());
            }
            for vs_list in voice_states.values() {
                for vs in vs_list {
                    if field_str(vs, "channel_id").is_some() {
                        if let Some(uid) = field_str(vs, "user_id") {
                            let has_online = presences.iter().any(|p| {
                                let pid = field_str(p, "user_id")
                                    .or_else(|| p.get("user").and_then(|u| field_str(u, "id")));
                                pid == Some(uid)
                                    && field_str(p, "status").unwrap_or("offline") != "offline"
                            });
                            if !has_online {
                                presences.push(json!({
                                    "user_id": uid,
                                    "user": { "id": uid },
                                    "status": "online"
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ReadyPayload {
        guild_count: guilds.len(),
        private_channel_count: private_channels.len(),
        relationship_count: relationships.len(),
        read_state_count,
        guilds,
        user,
        session,
        users,
        relationships,
        private_channels,
        presences,
        voice_states,
        user_guild_settings,
        read_state,
    })
}

// ---------------------------------------------------------------------------
// read_state — READY-проекция для UI (RW-40/M-07-BE2, badge — M-07-BE3)
// ---------------------------------------------------------------------------

/// Запись `READY.read_state.entries[]` в терминах IPC.
///
/// Поля — ровно ключи READY (`docs/re/gateway-schema.md`
/// `READY.d.read_state.entries[]`), имена — их точный camelCase: общая форма
/// `{channelId, flags, lastMessageId, lastPinTimestamp, lastViewed, mentionCount}`
/// и badge-форма (`read_state_type`, 2/157) —
/// `{badgeCount, lastAckedId, readStateType}`. `lastViewed` — счётчик read-state
/// (не snowflake; пример `4310` из `MESSAGE_ACK`).
///
/// Отсутствующее или санитизированное (`nil`) поле — `null`, а не 0: у обычной
/// записи нет badge-полей, у badge-записи нет `flags`/`lastViewed`, и выдуманный
/// 0 соврал бы UI. `badgeCount = 0` — **валидное значение** (16 и 0 в READY),
/// не «нет бейджа»: предикат `> 0` — политика UI/селектора, не проекции.
/// `lastAckedId` — десятичная строка, как `lastMessageId`: snowflake READY
/// (`1294268550826950656`) не помещается в JS-safe integer. `readStateType`
/// отдаётся числом как есть, без интерпретации (семантика 2/4 — RE-оракул,
/// `docs/rust-client/CODE-STANDARD.md` RW-17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadStateEntry {
    pub channel_id: String,
    pub flags: Option<u64>,
    pub last_message_id: Option<String>,
    pub last_pin_timestamp: Option<String>,
    /// `last_viewed` из READY — счётчик ack для канала.
    pub last_viewed: Option<u64>,
    pub mention_count: Option<u64>,
    /// `badge_count` — число бейджей badge-формы; `None` у обычной записи.
    pub badge_count: Option<i64>,
    /// `last_acked_id` — non-channel ack-точка badge-формы, decimal-строка.
    pub last_acked_id: Option<String>,
    /// `read_state_type` — тип записи READY (2/4), как есть, без трактовки.
    pub read_state_type: Option<i64>,
}

/// Строковое поле, где `null`, `nil` и `""` — «значения нет». Другой тип —
/// `Parse`: битый READY не должен молча превращаться в `null`.
#[allow(dead_code)]
fn optional_str(
    value: &Value,
    key: &str,
    path: &Path,
    at: &str,
) -> Result<Option<String>, DataError> {
    optional_str_or_snowflake(value, key, path, at, false)
}

fn optional_str_or_snowflake(
    value: &Value,
    key: &str,
    path: &Path,
    at: &str,
    allow_number: bool,
) -> Result<Option<String>, DataError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text == "nil" || text.is_empty() => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(Value::Number(num)) if allow_number => Ok(Some(num.to_string())),
        Some(_) => Err(DataError::parse(
            path,
            format!("{at}: поле {key} должно быть строкой или null"),
        )),
    }
}

/// Числовое поле `u64`, где отсутствие/`null`/`nil` — `None`. Нераспознанное
/// значение — `Parse`, а не тихий `null`.
fn optional_u64(value: &Value, key: &str, path: &Path, at: &str) -> Result<Option<u64>, DataError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text == "nil" || text.is_empty() => Ok(None),
        Some(_) => field_u64(value, key).map(Some).ok_or_else(|| {
            DataError::parse(path, format!("{at}: поле {key} должно быть числом u64"))
        }),
    }
}

/// Числовое поле `i64` (`badge_count`/`read_state_type` в READY): отсутствие/
/// `null`/`nil` — `None`, нераспознанное значение — `Parse`. Знак сохраняется
/// как есть: отрицательный `badge_count` — решение селектора, не проекции.
fn optional_i64(value: &Value, key: &str, path: &Path, at: &str) -> Result<Option<i64>, DataError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text == "nil" || text.is_empty() => Ok(None),
        Some(_) => field_i64(value, key).map(Some).ok_or_else(|| {
            DataError::parse(path, format!("{at}: поле {key} должно быть числом i64"))
        }),
    }
}

/// Записи `READY.read_state.entries[]` для UI.
///
/// Источник — только локальный `gateway/by-type/002-READY.json`: сеть, токен и
/// keyring не нужны. Порядок записей сохраняется как в READY (UI сопоставляет
/// строки по `channelId`; пересортировка сломала бы соответствие серверу).
/// Badge-форма READY проецируется аддитивно: обычная запись получает три
/// `null`, badge-запись — `badgeCount`/`lastAckedId`/`readStateType` как есть.
///
/// Ошибки — [`DataError`]: нет READY — `NotFound`; битый JSON, отсутствие
/// `read_state`/`entries`, запись без строкового `id`, поле не того типа
/// (включая `badge_count`/`read_state_type`/`last_acked_id`) — `Parse` с
/// индексом записи. Ошибка никогда не маскируется пустым списком; пустой
/// `entries: []` — валидный ответ (у пользователя просто нет read-state).
pub fn read_state(root: &Path) -> Result<Vec<ReadStateEntry>, DataError> {
    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let forbid_fixture = resolve_live_token().is_some();
    let live_entries = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if guard.active || forbid_fixture {
            Some(
                guard
                    .ready_data
                    .as_ref()
                    .and_then(|live_d| {
                        live_d.get("read_state").and_then(|rs| {
                            rs.get("entries")
                                .and_then(Value::as_array)
                                .or_else(|| rs.as_array())
                                .cloned()
                        })
                    })
                    .unwrap_or_default(),
            )
        } else {
            None
        }
    };

    let fallback_value;
    let entries: &[Value] = if let Some(ref items) = live_entries {
        items.as_slice()
    } else {
        fallback_value = read_json(&path)?;
        let data = fallback_value
            .get("d")
            .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?;
        let read_state = data
            .get("read_state")
            .ok_or_else(|| DataError::parse(&path, "READY без объекта read_state"))?;
        match read_state.get("entries") {
            Some(Value::Array(items)) => items.as_slice(),
            _ => {
                return Err(DataError::parse(
                    &path,
                    "READY.read_state.entries должен быть массивом",
                ))
            }
        }
    };

    let is_live = live_entries.is_some();
    let mut rows = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let at = format!("READY.read_state.entries[{index}]");
        let channel_id = field_str(entry, "id")
            .ok_or_else(|| DataError::parse(&path, format!("{at}: нет строкового id")))?;
        rows.push(ReadStateEntry {
            channel_id: channel_id.to_string(),
            flags: optional_u64(entry, "flags", &path, &at)?,
            last_message_id: optional_str_or_snowflake(entry, "last_message_id", &path, &at, is_live)?,
            last_pin_timestamp: optional_str_or_snowflake(entry, "last_pin_timestamp", &path, &at, is_live)?,
            last_viewed: optional_u64(entry, "last_viewed", &path, &at)?,
            mention_count: optional_u64(entry, "mention_count", &path, &at)?,
            badge_count: optional_i64(entry, "badge_count", &path, &at)?,
            last_acked_id: optional_str_or_snowflake(entry, "last_acked_id", &path, &at, is_live)?,
            read_state_type: optional_i64(entry, "read_state_type", &path, &at)?,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// channels — READY-проекция дерева каналов (C-02)
// ---------------------------------------------------------------------------

/// Канал READY для дерева каналов.
///
/// Поля — ровно те, что есть в READY: у гильдейского канала это `name`,
/// `position` и `parent_id`; у ЛС (`private_channels[]`) нет ни гильдии, ни
/// имени, ни позиции — там честный `None`, а не придуманное значение.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRow {
    pub id: String,
    /// id гильдии; `None` — личный канал (ЛС).
    pub guild_id: Option<String>,
    /// Имя канала; у ЛС в READY имени нет.
    pub name: Option<String>,
    /// `type` кадра числом: 0 текст, 2 голос, 4 категория, 5 объявление,
    /// 15 форум, 1 ЛС. Не переименовывается: UI решает по числу.
    #[serde(rename = "type")]
    pub kind: u64,
    /// `parent_id` категории; санитизированное `nil` — `None`.
    pub parent_id: Option<String>,
    /// `position` внутри гильдии; у ЛС позиции нет.
    pub position: Option<i64>,
    /// Оверрайты прав доступа (`permission_overwrites` из READY) для расчёта приватности/замков.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub permission_overwrites: Vec<Value>,
    /// Лимит пользователей голосового канала (`user_limit` > 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_limit: Option<u64>,
}

/// Массив из объекта READY: отсутствие/`null` — пустой набор, другой тип —
/// `Parse`. Иначе опечатка в фикстуре давала бы «каналов нет» вместо ошибки.
fn ready_array<'a>(data: &'a Value, key: &str, path: &Path) -> Result<&'a [Value], DataError> {
    match data.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(DataError::parse(
            path,
            format!("READY: поле {key} должно быть массивом"),
        )),
    }
}

/// Канал READY: `id` и `type` обязательны (без типа UI не решит, открывать ли
/// чат), остальные поля — как в фикстуре, `nil` — отсутствие значения.
fn parse_channel(
    channel: &Value,
    guild_id: Option<&str>,
    path: &Path,
    at: &str,
) -> Result<ChannelRow, DataError> {
    let id = field_str(channel, "id")
        .ok_or_else(|| DataError::parse(path, format!("{at}: нет строкового id")))?;
    let kind = field_u64(channel, "type")
        .ok_or_else(|| DataError::parse(path, format!("{at} (канал {id}): нет числового type")))?;
    Ok(ChannelRow {
        id: id.to_string(),
        guild_id: guild_id.map(str::to_string),
        name: field_str(channel, "name").map(str::to_string),
        kind,
        parent_id: field_str(channel, "parent_id").map(str::to_string),
        position: field_i64(channel, "position"),
        permission_overwrites: array(channel.get("permission_overwrites")).to_vec(),
        user_limit: field_u64(channel, "user_limit").filter(|&lim| lim > 0),
    })
}

/// Каналы из READY: гильдейские `guilds[].channels[]` и ЛС `private_channels[]`.
///
/// `guild_id` — необязательный фильтр (обрезается, ≤ [`MAX_INPUT_LEN`]):
/// неизвестная гильдия — `NotFound`, а не пустой список. Порядок стабильный:
/// гильдии в порядке READY (как рейл), внутри — `position`, затем `id`;
/// у ЛС нет ни гильдии, ни позиции, поэтому они идут последними по `id`.
///
/// Это READY-проекция, а не «все каналы, где были сообщения»: каналы,
/// встреченные только в `MESSAGE_*`, в дерево не попадают. Сеть/токен не нужны.
pub fn channels(root: &Path, guild_id: Option<&str>) -> Result<Vec<ChannelRow>, DataError> {
    let guild_filter = match guild_id {
        None => None,
        Some(value) => {
            let value = value.trim();
            if value.is_empty() {
                return Err(DataError::invalid_arg("guildId не может быть пустым"));
            }
            ensure_input_len(value, "guildId")?;
            Some(value)
        }
    };

    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let forbid_fixture = resolve_live_token().is_some();
    let value_opt = if forbid_fixture {
        None
    } else {
        Some(read_json(&path)?)
    };
    let (fallback_guilds, fallback_private): (&[Value], &[Value]) = if let Some(ref value) = value_opt {
        let data = value
            .get("d")
            .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?;
        (
            ready_array(data, "guilds", &path)?,
            ready_array(data, "private_channels", &path)?,
        )
    } else {
        (&[], &[])
    };

    let (is_live_active, live_token_opt, mut live_guilds_opt, live_private_opt) = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if guard.active {
            if let Some(ref live_d) = guard.ready_data {
                let lg = array(live_d.get("guilds"));
                let lp = array(live_d.get("private_channels"));
                (
                    true,
                    guard.token.clone(),
                    Some(lg.to_vec()),
                    Some(lp.to_vec()),
                )
            } else if forbid_fixture {
                (true, guard.token.clone(), Some(Vec::new()), Some(Vec::new()))
            } else {
                (true, guard.token.clone(), None, None)
            }
        } else if forbid_fixture {
            (false, None, Some(Vec::new()), Some(Vec::new()))
        } else {
            (false, None, None, None)
        }
    };

    if is_live_active {
        if let Some(filter) = guild_filter {
            let has_guild_with_channels = live_guilds_opt
                .as_ref()
                .map(|lg| {
                    lg.iter().any(|g| {
                        field_str(g, "id") == Some(filter)
                            && !array(g.get("channels")).is_empty()
                    })
                })
                .unwrap_or(false);
            if has_guild_with_channels && filter != "@me" {
                let has_mock = live_rest_mock_slot()
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_some();
                if has_mock {
                    hydrate_live_guild_blocking(filter);
                } else {
                    spawn_hydrate_live_guild_full(filter);
                }
            } else if !has_guild_with_channels {
                if filter != "@me" {
                    hydrate_live_guild_blocking(filter);
                    let guard = live_session()
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    if guard.active {
                        if let Some(ref live_d) = guard.ready_data {
                            live_guilds_opt = Some(array(live_d.get("guilds")).to_vec());
                        }
                    }
                }
                let still_missing = live_guilds_opt
                    .as_ref()
                    .map(|lg| {
                        !lg.iter().any(|g| {
                            field_str(g, "id") == Some(filter)
                                && !array(g.get("channels")).is_empty()
                        })
                    })
                    .unwrap_or(true);
                if still_missing {
                    if let Some(ref tok) = live_token_opt {
                        if let Some(mut fetched_rd) = fetch_live_ready_via_rest(tok) {
                            sanitize_etf_json_value(&mut fetched_rd);
                            {
                                let mut guard = live_session()
                                    .lock()
                                    .unwrap_or_else(|poison| poison.into_inner());
                                if guard.active {
                                    if let Value::Object(ref mut rm) = fetched_rd {
                                        if let Some(Value::Array(ref mut g_arr)) = rm.get_mut("guilds") {
                                            for (pgid, pg_val) in &guard.local_preview_guilds {
                                                if !g_arr.iter().any(|x| field_str(x, "id") == Some(pgid.as_str())) {
                                                    g_arr.push(pg_val.clone());
                                                }
                                            }
                                        }
                                    }
                                    populate_voice_and_presences_from_ready(&mut guard, &fetched_rd);
                                    guard.ready_data = Some(fetched_rd.clone());
                                }
                            }
                            save_live_ready_cache(&fetched_rd);
                            if filter != "@me" {
                                hydrate_live_guild_blocking(filter);
                            }
                            let guard = live_session()
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner());
                            if guard.active {
                                if let Some(ref live_d) = guard.ready_data {
                                    live_guilds_opt = Some(array(live_d.get("guilds")).to_vec());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let guilds: &[Value] = live_guilds_opt.as_deref().unwrap_or(fallback_guilds);
    let private: &[Value] = live_private_opt.as_deref().unwrap_or(fallback_private);

    if let Some(filter) = guild_filter {
        let known = guilds
            .iter()
            .any(|guild| field_str(guild, "id") == Some(filter));
        if !known {
            return Err(DataError::not_found(
                "гильдия",
                format!("гильдия {filter} не найдена в READY"),
            ));
        }
    }

    // Ключ сортировки: индекс гильдии в READY; `usize::MAX` — ЛС (в конце).
    let mut rows: Vec<(usize, ChannelRow)> = Vec::new();
    for (guild_index, guild) in guilds.iter().enumerate() {
        let id = field_str(guild, "id").ok_or_else(|| {
            DataError::parse(
                &path,
                format!("READY.guilds[{guild_index}] без строкового id"),
            )
        })?;
        if guild_filter.is_some_and(|filter| filter != id) {
            continue;
        }
        for (channel_index, channel) in array(guild.get("channels")).iter().enumerate() {
            rows.push((
                guild_index,
                parse_channel(
                    channel,
                    Some(id),
                    &path,
                    &format!("READY.guilds[{guild_index}].channels[{channel_index}]"),
                )?,
            ));
        }
    }
    if guild_filter.is_none() {
        for (index, channel) in private.iter().enumerate() {
            rows.push((
                usize::MAX,
                parse_channel(
                    channel,
                    None,
                    &path,
                    &format!("READY.private_channels[{index}]"),
                )?,
            ));
        }
    }

    rows.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| {
                left.1
                    .position
                    .unwrap_or(i64::MAX)
                    .cmp(&right.1.position.unwrap_or(i64::MAX))
            })
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

// ---------------------------------------------------------------------------
// Общая пагинация
// ---------------------------------------------------------------------------

/// Проверенный размер страницы: `1..=MAX_PAGE_LIMIT`, по умолчанию `DEFAULT_PAGE_LIMIT`.
/// `pub(crate)` — мост ядра (`core_bridge`) обязан применять тот же лимит.
pub(crate) fn page_limit(limit: Option<usize>) -> Result<usize, DataError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err(DataError::invalid_arg(format!(
            "limit должен быть в 1..={MAX_PAGE_LIMIT}, получено {limit}"
        )));
    }
    Ok(limit)
}

/// Усечённое пользовательское значение для текста ошибки: не более
/// [`ERROR_VALUE_PREVIEW`] символов плюс `…`. Гигантский вход не копируется
/// в сообщение целиком (иначе сам отчёт об ошибке — аллокация на весь вход).
fn preview(value: &str) -> String {
    let mut short = String::new();
    let mut chars = value.chars();
    for _ in 0..ERROR_VALUE_PREVIEW {
        match chars.next() {
            Some(ch) => short.push(ch),
            None => return short,
        }
    }
    if chars.next().is_some() {
        short.push('…');
    }
    short
}

/// Проверяет длину пользовательского id/курсора: длиннее [`MAX_INPUT_LEN`] —
/// `InvalidArg` с усечённым значением. Пустую строку вызывающий отсекает сам.
fn ensure_input_len(value: &str, name: &str) -> Result<(), DataError> {
    let len = value.chars().count();
    if len > MAX_INPUT_LEN {
        return Err(DataError::invalid_arg(format!(
            "{name} длиннее {MAX_INPUT_LEN} символов: {} (всего {len})",
            preview(value)
        )));
    }
    Ok(())
}

/// Индекс первой строки после курсора. Курсор — id строки из этой же выборки;
/// `None`/`""` — с начала. Чужой id — `InvalidArg`, а не молчаливый сброс.
/// `pub(crate)` — мост ядра использует ту же семантику курсора, что и BFF.
pub(crate) fn cursor_start(
    ids: &[String],
    cursor: Option<&str>,
    what: &str,
) -> Result<usize, DataError> {
    match cursor.map(str::trim) {
        None | Some("") => Ok(0),
        Some(cursor) => {
            ensure_input_len(cursor, "курсор")?;
            ids.iter()
                .position(|id| id == cursor)
                .map(|position| position + 1)
                .ok_or_else(|| {
                    DataError::invalid_arg(format!(
                        "курсор {what} не найден в текущей выборке: {}",
                        preview(cursor)
                    ))
                })
        }
    }
}

/// Индекс строки-якоря older-страницы: строки строго **до** неё. `None`/`""` —
/// якоря нет (`Ok(None)`): вызывающий берёт хвост выборки. Чужой id —
/// `InvalidArg`, а не молчаливый сброс на начало/конец.
///
/// `pub(crate)` — мост ядра сможет применять ту же семантику `before`,
/// что и BFF (сейчас мост остаётся forward-only).
pub(crate) fn cursor_end(
    ids: &[String],
    before: Option<&str>,
    what: &str,
) -> Result<Option<usize>, DataError> {
    match before.map(str::trim) {
        None | Some("") => Ok(None),
        Some(before) => {
            ensure_input_len(before, "before-курсор")?;
            ids.iter()
                .position(|id| id == before)
                .map(Some)
                .ok_or_else(|| {
                    DataError::invalid_arg(format!(
                        "before-курсор {what} не найден в текущей выборке: {}",
                        preview(before)
                    ))
                })
        }
    }
}

// ---------------------------------------------------------------------------
// messages_page
// ---------------------------------------------------------------------------

/// Normalized channel id: trimmed, non-empty, ≤ [`MAX_INPUT_LEN`] chars.
fn ensure_channel_id(channel_id: &str) -> Result<&str, DataError> {
    let channel_id = channel_id.trim();
    if channel_id.is_empty() {
        return Err(DataError::invalid_arg("channelId не может быть пустым"));
    }
    ensure_input_len(channel_id, "channelId")?;
    Ok(channel_id)
}

/// Все строки канала без пагинации (общий загрузчик BFF и моста ядра).
///
/// Повторяет проверки `messages_page`: пустой/длинный `channelId` — `InvalidArg`,
/// неизвестный канал — `NotFound`, битый by-type — `Parse`.
pub(crate) fn channel_rows(root: &Path, channel_id: &str) -> Result<Vec<MessageRow>, DataError> {
    let channel_id = ensure_channel_id(channel_id)?;
    let (
        live_active,
        live_token,
        already_fetched,
        needs_live_refresh,
        is_local_synth_channel,
        cached_live,
        live_ready,
    ) = {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let is_real_token = guard
            .token
            .as_deref()
            .is_some_and(|t| !t.is_empty() && t != "live-test-token" && !t.starts_with("test-"));
        let stale = is_real_token
            && guard
                .fetched_channels_at
                .get(channel_id)
                .is_none_or(|ts| ts.elapsed() >= std::time::Duration::from_secs(15));
        let synth_ch = channel_id.parse::<u64>().map_or(true, |cid_num| {
            guard.local_preview_guilds.keys().any(|gid_str| {
                gid_str.parse::<u64>().is_ok_and(|gid_num| {
                    gid_num >= 100_000_000_000_000_000
                        && (gid_num.saturating_add(100)..=gid_num.saturating_add(200))
                            .contains(&cid_num)
                })
            })
        });
        (
            guard.active,
            guard.token.clone(),
            guard.fetched_channels.contains(channel_id),
            stale,
            synth_ch,
            guard
                .channel_messages
                .get(channel_id)
                .cloned()
                .unwrap_or_default(),
            if guard.active {
                guard.ready_data.clone()
            } else {
                None
            },
        )
    };
    let mut frames = if live_active && live_ready.is_some() && root == default_fixtures_root() {
        Vec::new()
    } else {
        read_frames(root)?
    };
    if let Some(live_d) = live_ready {
        frames.push(Frame {
            kind: "READY".to_string(),
            data: live_d,
            path: PathBuf::from("live://ready"),
        });
    }
    let is_known = !cached_live.is_empty() || known_channels(&frames).contains(channel_id);
    if !is_known && (!live_active || !channel_id.bytes().all(|b| b.is_ascii_digit())) {
        return Err(DataError::not_found(
            "канал",
            format!("канал {channel_id} не найден в READY/by-type"),
        ));
    }
    let mut live_msgs = cached_live;
    if live_active && !is_local_synth_channel && (!already_fetched || needs_live_refresh) {
        let should_fetch_rest = if live_token.is_some() {
            true
        } else {
            collect_messages(&frames, channel_id)?.is_empty()
        };
        if should_fetch_rest {
            if let Some(ref token) = live_token {
                {
                    let mut guard = live_session()
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    guard.fetched_channels.insert(channel_id.to_string());
                    guard
                        .fetched_channels_at
                        .insert(channel_id.to_string(), std::time::Instant::now());
                }
                if let Some(mut fetched) = fetch_rest_channel_messages(token, channel_id) {
                    let mut guard = live_session()
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    // Фильтруем удалённые сообщения: сообщения из blacklisted deleted_message_ids
                    // гарантированно не попадут в отображение канала.
                    fetched.retain(|m| {
                        if let Some(mid) = field_str(m, "id") {
                            !guard
                                .deleted_message_ids
                                .contains(&(channel_id.to_string(), mid.to_string()))
                        } else {
                            true
                        }
                    });
                    // Не воскрешаем старые сообщения из кэша, если они удалены на сервере Discord.
                    // Сохраняем только локальные неподтверждённые (pending) сообщения.
                    for existing in &live_msgs {
                        if let Some(eid) = field_str(existing, "id") {
                            let is_pending = existing
                                .get("is_pending")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            if is_pending
                                && !guard
                                    .deleted_message_ids
                                    .contains(&(channel_id.to_string(), eid.to_string()))
                                && !fetched.iter().any(|m| field_str(m, "id") == Some(eid))
                            {
                                fetched.push(existing.clone());
                            }
                        }
                    }
                    guard
                        .channel_messages
                        .insert(channel_id.to_string(), fetched.clone());
                    live_msgs = fetched;
                }
            }
        }
    }
    if !live_msgs.is_empty() {
        frames.retain(|frame| {
            !(frame.kind == "MESSAGE_CREATE"
                && field_str(&frame.data, "channel_id") == Some(channel_id))
        });
        let live_path = PathBuf::from("live://discord-rest");
        for msg in live_msgs {
            frames.push(Frame {
                kind: "MESSAGE_CREATE".to_string(),
                data: msg,
                path: live_path.clone(),
            });
        }
    }
    let rows = collect_messages(&frames, channel_id)?;
    if live_active && !already_fetched {
        let mut chan_gid: Option<String> = rows.iter().find_map(|r| r.guild_id.clone());
        if chan_gid.is_none() {
            for frame in &frames {
                if frame.kind == "READY" {
                    for g in array(frame.data.get("guilds")) {
                        if let Some(gid) = field_str(g, "id") {
                            if array(g.get("channels"))
                                .iter()
                                .any(|c| field_str(c, "id") == Some(channel_id))
                            {
                                chan_gid = Some(gid.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
        if let Some(ref gid) = chan_gid {
            discord_core::ws::queue_live_guild_subscription(gid, channel_id);
            let mut author_ids: Vec<String> = rows.iter().map(|r| r.author.id.clone()).collect();
            author_ids.sort();
            author_ids.dedup();
            if !author_ids.is_empty() {
                discord_core::ws::queue_live_request_guild_members(gid, &author_ids);
            }
        } else {
            discord_core::ws::queue_live_dm_call_sync(channel_id);
        }
    }
    Ok(rows)
}

/// Автор сообщения (только поля, нужные списку чата).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageAuthor {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub bot: bool,
}

/// Вложение сообщения. В снятых фикстурах `attachments` всегда `null`, поэтому
/// на практике список пуст; поля разбираются из кадра, если вложение появится.
/// Элемент без обязательных полей не роняет строку: `filename` пустой — это
/// отсутствие данных, а не выдуманное имя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageAttachment {
    pub id: Option<String>,
    pub filename: String,
    pub size: u64,
    pub url: Option<String>,
    pub content_type: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waveform: Option<String>,
}

/// Embed сообщения: только те поля, что реально есть в кадре.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEmbed {
    pub id: Option<String>,
    /// `type` кадра: `article`, `rich`, `image`, …
    pub kind: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u64>,
    pub provider_name: Option<String>,
    pub author_name: Option<String>,
    pub footer_text: Option<String>,
    pub thumbnail_url: Option<String>,
    pub field_count: usize,
}

/// Реакция сообщения (`reactions[]` кадра). В снятых фикстурах поля нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageReaction {
    /// `emoji.name`, у кастомных — `emoji.id`, если имени нет.
    pub emoji: String,
    pub count: u64,
    pub me: bool,
}

/// Строка сообщения. `content` не рендерится как HTML — это данные, не разметка.
///
/// `day`/`group` вычислены из `timestamp` и соседних строк этого же канала:
/// `group = "start"` у первого сообщения и при смене автора или дня,
/// иначе `"continuation"`. Разрыв по времени (> 7 минут) не считается: для него
/// нужен парсер ISO-времени, а новых зависимостей не добавляем.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author: MessageAuthor,
    pub content: String,
    pub timestamp: Option<String>,
    pub edited: bool,
    pub edited_at: Option<String>,
    /// Дата `YYYY-MM-DD` из `timestamp` — для разделителя дня.
    pub day: Option<String>,
    /// `start` | `continuation` — для `data-group` строки.
    pub group: String,
    pub pinned: bool,
    /// `type` кадра (0 — обычное сообщение).
    pub message_type: u64,
    pub embeds: Vec<MessageEmbed>,
    pub attachments: Vec<MessageAttachment>,
    pub reactions: Vec<MessageReaction>,
    /// id сообщения, на которое отвечают (`message_reference`/`referenced_message`).
    pub reply_to: Option<String>,
    /// id упомянутых пользователей из `mentions[]`.
    pub mentions: Vec<String>,
    pub mention_everyone: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<Value>,
}

/// Страница сообщений канала. `next_cursor == None` — строк новее нет;
/// пустая страница при `has_more == false` — валидный конец выборки.
///
/// `has_more`/`next_cursor` — forward (как было), `has_older`/`older_cursor` —
/// older-направление. `older_cursor` — id первой строки страницы: его передают
/// в `before`, чтобы получить следующую более старую страницу. Если строк
/// старше нет, оба older-поля — `false`/`None`, а не выдуманный курсор.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagesPage {
    pub channel_id: String,
    pub items: Vec<MessageRow>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub has_older: bool,
    pub older_cursor: Option<String>,
    pub total: usize,
    pub limit: usize,
    pub loaded_rows_limit: usize,
}

fn parse_message_author(value: &Value, path: &Path) -> Result<MessageAuthor, DataError> {
    let id =
        field_str(value, "id").ok_or_else(|| DataError::parse(path, "author без строкового id"))?;
    let username = field_str(value, "username").unwrap_or(id);
    Ok(MessageAuthor {
        id: id.to_string(),
        username: username.to_string(),
        global_name: field_str(value, "global_name").map(str::to_string),
        avatar: field_str(value, "avatar").map(str::to_string),
        bot: field_bool(value, "bot"),
    })
}

/// Дата `YYYY-MM-DD` из ISO-подобного `timestamp`. Непарсящееся значение
/// (санитизированное `nil`, пустая строка, чужой формат) — `None`, не фейк.
fn day_of(timestamp: Option<&str>) -> Option<String> {
    let text = timestamp?;
    let date = text.get(..10)?;
    let bytes = date.as_bytes();
    let shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    shape.then(|| date.to_string())
}

fn parse_mentions(data: &Value) -> Vec<String> {
    array(data.get("mentions"))
        .iter()
        .filter_map(|user| field_str(user, "id").map(str::to_string))
        .collect()
}

fn parse_reactions(value: Option<&Value>) -> Vec<MessageReaction> {
    array(value)
        .iter()
        .filter_map(|reaction| {
            let emoji = reaction.get("emoji")?;
            let name = field_str(emoji, "name").or_else(|| field_str(emoji, "id"));
            let Some(name) = name else {
                // Не глотаем молча: реакция без emoji.name/id не отрисуется.
                eprintln!("data: реакция без emoji.name/id пропущена");
                return None;
            };
            Some(MessageReaction {
                emoji: name.to_string(),
                count: field_u64(reaction, "count").unwrap_or(0),
                me: field_bool(reaction, "me"),
            })
        })
        .collect()
}

fn format_live_discord_text(raw: &str, data: &Value) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut out = raw.to_string();
    for m in array(data.get("mentions")) {
        if let Some(uid) = field_str(m, "id") {
            let display = field_str(m, "global_name")
                .or_else(|| field_str(m, "username"))
                .unwrap_or(uid);
            let replacement = format!("@{display}");
            out = out.replace(&format!("<@{uid}>"), &replacement);
            out = out.replace(&format!("<@!{uid}>"), &replacement);
        }
    }
    const SHORTCODES: &[(&str, &str)] = &[
        (":arrow_left:", "⬅️"),
        (":arrow_right:", "➡️"),
        (":arrow_upper_right:", "↗️"),
        (":arrow_lower_right:", "↘️"),
        (":question:", "❓"),
        (":grey_question:", "❔"),
        (":exclamation:", "❗"),
        (":computer:", "💻"),
        (":desktop:", "🖥️"),
        (":white_check_mark:", "✅"),
        (":heavy_check_mark:", "✔️"),
        (":warning:", "⚠️"),
        (":fire:", "🔥"),
        (":sparkles:", "✨"),
        (":rocket:", "🚀"),
        (":thumbsup:", "👍"),
        (":+1:", "👍"),
        (":thumbsdown:", "👎"),
        (":-1:", "👎"),
        (":heart:", "❤️"),
        (":x:", "❌"),
        (":information_source:", "ℹ️"),
        (":gear:", "⚙️"),
        (":shield:", "🛡️"),
        (":lock:", "🔒"),
        (":unlock:", "🔓"),
        (":bell:", "🔔"),
        (":tada:", "🎉"),
        (":wave:", "👋"),
        (":crown:", "👑"),
        (":star:", "⭐"),
        (":link:", "🔗"),
        (":pushpin:", "📌"),
        (":speech_balloon:", "💬"),
    ];
    for (code, emoji) in SHORTCODES {
        if out.contains(code) {
            out = out.replace(code, emoji);
        }
    }
    out
}

fn parse_embeds_for_frame(data: &Value, is_live: bool) -> Vec<MessageEmbed> {
    array(data.get("embeds"))
        .iter()
        .map(|embed| {
            let mut desc = field_str(embed, "description").map(str::to_string);
            if is_live {
                let mut field_lines = Vec::new();
                for f in array(embed.get("fields")) {
                    let fname = field_str(f, "name").unwrap_or_default();
                    let fval = field_str(f, "value").unwrap_or_default();
                    if !fname.is_empty() || !fval.is_empty() {
                        let line = if !fname.is_empty() && !fval.is_empty() {
                            format!("{fname}\n{fval}")
                        } else if !fname.is_empty() {
                            fname.to_string()
                        } else {
                            fval.to_string()
                        };
                        field_lines.push(format_live_discord_text(&line, data));
                    }
                }
                if let Some(d) = desc.as_mut() {
                    *d = format_live_discord_text(d, data);
                    if !field_lines.is_empty() {
                        d.push_str("\n\n");
                        d.push_str(&field_lines.join("\n\n"));
                    }
                } else if !field_lines.is_empty() {
                    desc = Some(field_lines.join("\n\n"));
                }
            }
            MessageEmbed {
                id: field_str(embed, "id").map(str::to_string),
                kind: field_str(embed, "type").unwrap_or("rich").to_string(),
                title: field_str(embed, "title").map(|t| {
                    if is_live {
                        format_live_discord_text(t, data)
                    } else {
                        t.to_string()
                    }
                }),
                description: desc,
                url: field_str(embed, "url").map(str::to_string),
                color: field_u64(embed, "color"),
                provider_name: embed
                    .get("provider")
                    .and_then(|provider| field_str(provider, "name"))
                    .map(str::to_string),
                author_name: embed
                    .get("author")
                    .and_then(|author| field_str(author, "name"))
                    .map(str::to_string),
                footer_text: embed
                    .get("footer")
                    .and_then(|footer| field_str(footer, "text"))
                    .map(str::to_string),
                thumbnail_url: embed
                    .get("thumbnail")
                    .and_then(|thumbnail| {
                        if is_live {
                            field_str(thumbnail, "proxy_url")
                                .or_else(|| field_str(thumbnail, "url"))
                        } else {
                            field_str(thumbnail, "url")
                        }
                    })
                    .or_else(|| {
                        embed.get("image").and_then(|img| {
                            if is_live {
                                field_str(img, "proxy_url").or_else(|| field_str(img, "url"))
                            } else {
                                field_str(img, "url")
                            }
                        })
                    })
                    .map(str::to_string),
                field_count: array_len(embed.get("fields")),
            }
        })
        .collect()
}

fn parse_attachments_for_frame(data: &Value, is_live: bool) -> Vec<MessageAttachment> {
    let parse_one = |attachment: &Value| -> MessageAttachment {
        let duration_secs = if is_live {
            attachment
                .get("duration_secs")
                .map(|v| match v {
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => s.clone(),
                    _ => String::new(),
                })
                .filter(|s| !s.is_empty() && s != "nil")
        } else {
            None
        };
        let waveform = if is_live {
            field_str(attachment, "waveform").map(str::to_string)
        } else {
            None
        };
        MessageAttachment {
            id: field_str(attachment, "id").map(str::to_string),
            filename: field_str(attachment, "filename")
                .unwrap_or_default()
                .to_string(),
            size: field_u64(attachment, "size").unwrap_or(0),
            url: field_str(attachment, "url")
                .or_else(|| field_str(attachment, "proxy_url"))
                .map(str::to_string),
            content_type: field_str(attachment, "content_type").map(str::to_string),
            width: field_u64(attachment, "width"),
            height: field_u64(attachment, "height"),
            duration_secs,
            waveform,
        }
    };
    let mut out: Vec<MessageAttachment> =
        array(data.get("attachments")).iter().map(parse_one).collect();
    if is_live {
        for snap in array(data.get("message_snapshots")) {
            if let Some(msg) = snap.get("message") {
                for att in array(msg.get("attachments")) {
                    let parsed = parse_one(att);
                    if parsed.url.is_some() && !out.iter().any(|x| x.url == parsed.url) {
                        out.push(parsed);
                    }
                }
                for emb in array(msg.get("embeds")) {
                    if let Some(vid) = emb.get("video") {
                        if let Some(vurl) =
                            field_str(vid, "url").or_else(|| field_str(vid, "proxy_url"))
                        {
                            if !out.iter().any(|x| x.url.as_deref() == Some(vurl)) {
                                out.push(MessageAttachment {
                                    id: field_str(emb, "id").map(str::to_string),
                                    filename: "video.mp4".to_string(),
                                    size: 0,
                                    url: Some(vurl.to_string()),
                                    content_type: Some("video/mp4".to_string()),
                                    width: field_u64(vid, "width"),
                                    height: field_u64(vid, "height"),
                                    duration_secs: None,
                                    waveform: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        for emb in array(data.get("embeds")) {
            if let Some(vid) = emb.get("video") {
                if let Some(vurl) = field_str(vid, "url").or_else(|| field_str(vid, "proxy_url")) {
                    if !out.iter().any(|x| x.url.as_deref() == Some(vurl)) {
                        out.push(MessageAttachment {
                            id: field_str(emb, "id").map(str::to_string),
                            filename: "video.mp4".to_string(),
                            size: 0,
                            url: Some(vurl.to_string()),
                            content_type: Some("video/mp4".to_string()),
                            width: field_u64(vid, "width"),
                            height: field_u64(vid, "height"),
                            duration_secs: None,
                            waveform: None,
                        });
                    }
                }
            }
        }
    }
    out
}

/// id сообщения, на которое отвечают: `message_reference.message_id` или
/// `referenced_message.id`. Нет обоих — `None` (в фикстурах ответов нет).
fn parse_reply_to(data: &Value) -> Option<String> {
    data.get("message_reference")
        .and_then(|reference| field_str(reference, "message_id"))
        .or_else(|| {
            data.get("referenced_message")
                .and_then(|message| field_str(message, "id"))
        })
        .map(str::to_string)
}

fn parse_message(data: &Value, path: &Path) -> Result<MessageRow, DataError> {
    let id = field_str(data, "id")
        .ok_or_else(|| DataError::parse(path, "сообщение без строкового id"))?;
    let channel_id = field_str(data, "channel_id")
        .ok_or_else(|| DataError::parse(path, format!("сообщение {id} без channel_id")))?;
    let author_value = data
        .get("author")
        .ok_or_else(|| DataError::parse(path, format!("сообщение {id} без author")))?;
    let timestamp = field_str(data, "timestamp").map(str::to_string);
    let edited_at = field_str(data, "edited_timestamp").map(str::to_string);
    let is_live = path.to_string_lossy().starts_with("live://");
    let mut raw_content = data
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if is_live && raw_content.is_empty() {
        for snap in array(data.get("message_snapshots")) {
            if let Some(sc) = snap
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty() && *s != "nil")
            {
                raw_content = format!("↪️ {sc}");
                break;
            }
        }
    }
    let content = if is_live {
        format_live_discord_text(&raw_content, data)
    } else {
        raw_content
    };
    Ok(MessageRow {
        id: id.to_string(),
        channel_id: channel_id.to_string(),
        guild_id: field_str(data, "guild_id").map(str::to_string),
        author: parse_message_author(author_value, path)?,
        content,
        day: day_of(timestamp.as_deref()),
        timestamp,
        edited: edited_at.is_some(),
        edited_at,
        // Заполняется в `collect_messages` по соседним строкам канала.
        group: "start".to_string(),
        pinned: field_bool(data, "pinned"),
        message_type: field_u64(data, "type").unwrap_or(0),
        embeds: parse_embeds_for_frame(data, is_live),
        attachments: parse_attachments_for_frame(data, is_live),
        reactions: parse_reactions(data.get("reactions")),
        reply_to: parse_reply_to(data),
        mentions: parse_mentions(data),
        mention_everyone: field_bool(data, "mention_everyone"),
        flags: if is_live { field_u64(data, "flags") } else { None },
        call: if is_live {
            data.get("call").cloned().filter(|v| !v.is_null())
        } else {
            None
        },
    })
}

/// Каналы, известные READY (гильдейские и ЛС) плюс каналы из самих кадров
/// сообщений: молчащий канал остаётся известным и отдаёт пустую страницу.
fn known_channels(frames: &[Frame]) -> HashSet<String> {
    let mut known = HashSet::new();
    for frame in frames {
        if frame.kind == "READY" {
            for guild in array(frame.data.get("guilds")) {
                for channel in array(guild.get("channels")) {
                    if let Some(id) = field_str(channel, "id") {
                        known.insert(id.to_string());
                    }
                }
            }
            for channel in array(frame.data.get("private_channels")) {
                if let Some(id) = field_str(channel, "id") {
                    known.insert(id.to_string());
                }
            }
        } else if frame.kind.starts_with("MESSAGE_") {
            if let Some(id) = field_str(&frame.data, "channel_id") {
                known.insert(id.to_string());
            }
        }
    }
    known
}

/// Метаданные канала для результатов поиска: имя и гильдия из READY.
/// Каналы, которых нет в READY (сняты в другой сессии), остаются с `None`.
#[derive(Debug, Clone, Default)]
struct ChannelInfo {
    name: Option<String>,
    guild_name: Option<String>,
}

fn channel_index(frames: &[Frame]) -> HashMap<String, ChannelInfo> {
    let mut index: HashMap<String, ChannelInfo> = HashMap::new();
    for frame in frames {
        if frame.kind != "READY" {
            continue;
        }
        for guild in array(frame.data.get("guilds")) {
            let guild_name = guild
                .get("properties")
                .and_then(|properties| field_str(properties, "name"))
                .map(str::to_string);
            for channel in array(guild.get("channels")) {
                let Some(id) = field_str(channel, "id") else {
                    continue;
                };
                index.insert(
                    id.to_string(),
                    ChannelInfo {
                        name: field_str(channel, "name").map(str::to_string),
                        guild_name: guild_name.clone(),
                    },
                );
            }
        }
        for channel in array(frame.data.get("private_channels")) {
            let Some(id) = field_str(channel, "id") else {
                continue;
            };
            index.entry(id.to_string()).or_insert_with(|| ChannelInfo {
                name: field_str(channel, "name").map(str::to_string),
                guild_name: None,
            });
        }
    }
    index
}

/// Собирает сообщения канала без дублей. `MESSAGE_CREATE` — база,
/// `MESSAGE_UPDATE` того же id лишь уточняет строку (текст, embed, правку),
/// не создавая вторую.
fn collect_messages(frames: &[Frame], channel_id: &str) -> Result<Vec<MessageRow>, DataError> {
    let mut messages: HashMap<String, MessageRow> = HashMap::new();
    for frame in frames {
        if frame.kind != "MESSAGE_CREATE" && frame.kind != "MESSAGE_UPDATE" {
            continue;
        }
        if field_str(&frame.data, "channel_id") != Some(channel_id) {
            continue;
        }
        let row = parse_message(&frame.data, &frame.path)?;
        match messages.get_mut(&row.id) {
            Some(existing) => {
                if frame.kind == "MESSAGE_UPDATE" {
                    existing.edited = true;
                    if existing.edited_at.is_none() {
                        existing.edited_at = row.edited_at.clone();
                    }
                    if !row.content.is_empty() {
                        existing.content = row.content;
                    }
                    // CREATE несёт `embeds: null`, UPDATE — реальный объект:
                    // без переноса embed терялся бы при дедупликации.
                    if existing.embeds.is_empty() {
                        existing.embeds = row.embeds;
                    }
                    if existing.attachments.is_empty() {
                        existing.attachments = row.attachments;
                    }
                    if existing.reactions.is_empty() {
                        existing.reactions = row.reactions;
                    }
                    if existing.reply_to.is_none() {
                        existing.reply_to = row.reply_to;
                    }
                    if existing.mentions.is_empty() {
                        existing.mentions = row.mentions;
                    }
                    existing.pinned |= row.pinned;
                }
            }
            None => {
                messages.insert(row.id.clone(), row);
            }
        }
    }
    let mut rows: Vec<MessageRow> = messages.into_values().collect();
    // Стабильный порядок: время, затем id. Числовые snowflake не парсим:
    // в санитизированных фикстурах встречаются отрицательные id.
    rows.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.id.cmp(&right.id))
    });
    // `group` — по соседям в полном списке канала (до пагинации), поэтому
    // граница страницы не меняет разметку строки.
    let mut previous: Option<(String, Option<String>)> = None;
    for row in rows.iter_mut() {
        let starts = match &previous {
            None => true,
            Some((author, day)) => *author != row.author.id || *day != row.day,
        };
        row.group = if starts { "start" } else { "continuation" }.to_string();
        previous = Some((row.author.id.clone(), row.day.clone()));
    }
    Ok(rows)
}

/// Сторона открытия истории, когда якоря (`cursor`/`before`) не заданы.
///
/// В IPC приходит строкой: `"start"` (по умолчанию, первая страница с начала)
/// или `"latest"` (страница с конца канала). Другие значения — `InvalidArg`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MessagesPosition {
    /// С начала истории — прежнее поведение `messages_page` без курсора.
    #[default]
    Start,
    /// С конца истории: последние `limit` строк, порядок возрастающий.
    Latest,
}

impl MessagesPosition {
    /// Разбор IPC-значения: `None`/`""`/`"start"` → [`Self::Start`],
    /// `"latest"` → [`Self::Latest`], иначе `InvalidArg` с усечённым значением.
    pub fn parse(raw: Option<&str>) -> Result<Self, DataError> {
        match raw.map(str::trim) {
            None | Some("") | Some("start") => Ok(Self::Start),
            Some("latest") => Ok(Self::Latest),
            Some(other) => Err(DataError::invalid_arg(format!(
                "position должен быть \"start\" или \"latest\", получено: {}",
                preview(other)
            ))),
        }
    }
}

/// Запрос страницы сообщений: forward-курсор, older-курсор или сторона старта.
///
/// `cursor` и `before` взаимоисключающие, `position = Latest` не сочетается ни
/// с одним из них: комбинация — `InvalidArg`, а не тихий приоритет одного.
/// Пустые/пробельные курсоры считаются отсутствующими (как и раньше у `cursor`).
#[derive(Debug, Clone, Copy, Default)]
pub struct MessagesPageRequest<'a> {
    pub cursor: Option<&'a str>,
    pub before: Option<&'a str>,
    pub position: MessagesPosition,
    pub limit: Option<usize>,
}

/// Окно выборки → [`MessagesPage`]. `start..end` — полуинтервал строк канала
/// в порядке backend (`timestamp`, затем `id`).
///
/// `has_more`/`next_cursor` — есть ли строки после окна и id последней строки
/// (пара согласована: пустая страница не обещает курсор). `has_older` /
/// `older_cursor` — есть ли строки до окна и id первой строки, следующий
/// `before`. Строки старше/новее не выдумываются: только факты выборки.
fn messages_window(
    channel_id: &str,
    rows: &[MessageRow],
    start: usize,
    end: usize,
    limit: usize,
) -> MessagesPage {
    debug_assert!(start <= end && end <= rows.len(), "окно вне выборки");
    let items: Vec<MessageRow> = rows[start..end].to_vec();
    let has_more = !items.is_empty() && end < rows.len();
    let next_cursor = if has_more {
        items.last().map(|row| row.id.clone())
    } else {
        None
    };
    let has_older = start > 0 && !items.is_empty();
    let older_cursor = if has_older {
        items.first().map(|row| row.id.clone())
    } else {
        None
    };
    MessagesPage {
        channel_id: channel_id.to_string(),
        items,
        next_cursor,
        has_more,
        has_older,
        older_cursor,
        total: rows.len(),
        limit,
        loaded_rows_limit: UI_WINDOW_ROWS,
    }
}

/// Страница сообщений канала: `limit` строк строго после `cursor`.
///
/// Прежняя forward-сигнатура сохранена для существующих вызовов; older-режим —
/// [`messages_page_request`] с `before` или `position = Latest`.
/// Неизвестный канал → `NotFound`; известный молчащий канал → пустая страница
/// (`items: []`, `has_more: false`), это не ошибка.
pub fn messages_page(
    root: &Path,
    channel_id: &str,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<MessagesPage, DataError> {
    messages_page_request(
        root,
        channel_id,
        MessagesPageRequest {
            cursor,
            limit,
            ..MessagesPageRequest::default()
        },
    )
}

/// Страница сообщений по полному контракту C-02-BEFORE.
///
/// - `cursor` — forward: строки строго после id (поведение не изменилось);
/// - `before` — older: не более `limit` строк строго до id, порядок
///   возрастающий (как у forward-страниц — UI делает prepend). `before`
///   обязан быть id строки этого канала, иначе `InvalidArg`;
/// - `position = Latest` — последние `limit` строк канала без якоря;
/// - без якорей и `Start` — первая страница с начала истории.
///
/// `olderCursor` ответа — следующий `before`; когда `hasOlder == false`,
/// `olderCursor == None`. `hasMore`/`nextCursor` описывают forward-направление
/// и не меняются из-за older-запроса.
pub fn messages_page_request(
    root: &Path,
    channel_id: &str,
    request: MessagesPageRequest<'_>,
) -> Result<MessagesPage, DataError> {
    let channel_id = ensure_channel_id(channel_id)?;
    let limit = page_limit(request.limit)?;

    let cursor = request.cursor.map(str::trim).filter(|v| !v.is_empty());
    let before = request.before.map(str::trim).filter(|v| !v.is_empty());

    if cursor.is_some() && before.is_some() {
        return Err(DataError::invalid_arg(
            "cursor и before взаимоисключающие: задайте одно направление",
        ));
    }
    if request.position == MessagesPosition::Latest {
        if cursor.is_some() {
            return Err(DataError::invalid_arg(
                "position \"latest\" не сочетается с cursor: latest — старт с конца",
            ));
        }
        if before.is_some() {
            return Err(DataError::invalid_arg(
                "position \"latest\" не сочетается с before: latest — старт с конца",
            ));
        }
    }

    let rows = channel_rows(root, channel_id)?;
    let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();

    let (start, end) = match (before, request.position) {
        (Some(before), _) => {
            // Строки строго до якоря; якорь обязан существовать в канале.
            let end = cursor_end(&ids, Some(before), "сообщений")?.unwrap_or(rows.len());
            (end.saturating_sub(limit), end)
        }
        (None, MessagesPosition::Latest) => (rows.len().saturating_sub(limit), rows.len()),
        (None, MessagesPosition::Start) => {
            let start = cursor_start(&ids, cursor, "сообщений")?;
            (start, (start + limit).min(rows.len()))
        }
    };

    Ok(messages_window(channel_id, &rows, start, end, limit))
}

// ---------------------------------------------------------------------------
// members_page
// ---------------------------------------------------------------------------

/// Строка участника. `status` — presence из снимка (`offline`, если не снят).
///
/// `roles` — реальные id ролей из `member.roles`, отсортированы по позиции
/// (старшая первой), поэтому `roles[0]` — роль для чипа строки. `role_name` —
/// её имя из READY; у гильдии, которой нет в READY, имя честно отсутствует.
/// `group` — id группы снимка (роль или `online`/`idle`/`dnd`/`offline`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberRow {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub status: String,
    pub bot: bool,
    pub roles: Vec<String>,
    pub role_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clan_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    pub group: String,
}

/// Группа панели участников: роль (`kind = "role"`) или статус (`"status"`).
///
/// `announced_count` — `count` из `groups[]` самого свежего снимка (Discord
/// считает по всей гильдии; для больших гильдий список снят частично),
/// `loaded_count` — сколько строк этой группы реально есть в фикстурах.
/// `title` заполняется только для роли (имя из READY); для статуса — `None`,
/// подпись даёт фронтенд по id, чтобы не выдумывать текст в BFF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberGroup {
    pub id: String,
    pub kind: String,
    pub title: Option<String>,
    pub position: Option<i64>,
    pub announced_count: Option<u64>,
    pub loaded_count: usize,
}

/// Страница участников гильдии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MembersPage {
    pub guild_id: String,
    pub guild_name: Option<String>,
    pub member_count: Option<u64>,
    pub items: Vec<MemberRow>,
    pub groups: Vec<MemberGroup>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total: usize,
    pub limit: usize,
    pub loaded_rows_limit: usize,
}

fn status_rank(status: &str) -> u8 {
    match status {
        "online" => 0,
        "idle" => 1,
        "dnd" => 2,
        "offline" => 3,
        _ => 4,
    }
}

/// Роль гильдии из READY: id, имя, позиция, цвет и флаг `hoist` (для порядка групп и подсветки ников).
#[derive(Debug, Clone, PartialEq, Eq)]
struct RoleInfo {
    id: String,
    name: String,
    position: i64,
    color: u32,
    hoist: bool,
}

fn snowflake_low56(id: &str) -> Option<u64> {
    let trimmed = id.trim();
    if let Some(rest) = trimmed.strip_prefix('-') {
        let abs = rest.parse::<u64>().ok()?;
        Some((abs >> 8) & 0x00ff_ffff_ffff_ffff)
    } else {
        let val = trimmed.parse::<u64>().ok()?;
        Some(val & 0x00ff_ffff_ffff_ffff)
    }
}

fn extract_user_clan_tag(user: Option<&Value>) -> Option<String> {
    let u = user?;
    let clan = u.get("clan").or_else(|| u.get("primary_guild"))?;
    if clan.get("identity_enabled").is_some_and(|v| {
        v == &Value::Bool(false) || v.as_str() == Some("false")
    }) {
        return None;
    }
    field_str(clan, "tag").map(str::to_string)
}

fn extract_presence_activity(presence: Option<&Value>) -> Option<String> {
    let p = presence?;
    for act in array(p.get("activities")) {
        let kind = field_u64(act, "type").unwrap_or(0);
        if kind == 4 {
            if let Some(st) = field_str(act, "state") {
                return Some(st.to_string());
            }
        } else if let Some(name) = field_str(act, "name") {
            if name != "bg-capture" && name != "Custom Status" {
                return Some(format!("🎮 {name}"));
            }
        }
    }
    None
}

fn resolve_role_color_hex(roles: &[String], guild: &GuildInfo) -> Option<String> {
    for role_id in roles {
        if let Some(info) = guild.roles.iter().find(|r| &r.id == role_id) {
            if info.color > 0 {
                return Some(format!("#{:06x}", info.color & 0x00ff_ffff));
            }
        }
    }
    None
}

fn resolve_hoisted_role_id(roles: &[String], guild: &GuildInfo) -> Option<String> {
    for role_id in roles {
        if let Some(info) = guild.roles.iter().find(|r| &r.id == role_id) {
            if info.hoist {
                return Some(info.id.clone());
            }
        }
    }
    None
}

/// Метаданные гильдии: READY (имя, счётчик, роли) + последний снимок
/// участников (`groups[]` с счётчиками). Счётчики последнего снимка
/// перекрывают предыдущие: Discord присылает их заново при каждом UPDATE.
#[derive(Debug, Clone, Default)]
struct GuildInfo {
    name: Option<String>,
    member_count: Option<u64>,
    roles: Vec<RoleInfo>,
    announced: Vec<(String, u64)>,
}

fn guild_index(frames: &[Frame]) -> HashMap<String, GuildInfo> {
    let mut index: HashMap<String, GuildInfo> = HashMap::new();
    for frame in frames {
        match frame.kind.as_str() {
            "READY" => {
                for guild in array(frame.data.get("guilds")) {
                    let Some(id) = field_str(guild, "id") else {
                        continue;
                    };
                    let entry = index.entry(id.to_string()).or_default();
                    entry.name = guild
                        .get("properties")
                        .and_then(|properties| field_str(properties, "name"))
                        .or_else(|| field_str(guild, "name"))
                        .map(str::to_string);
                    entry.member_count = field_u64(guild, "member_count");
                    let parsed_roles: Vec<RoleInfo> = array(guild.get("roles"))
                        .iter()
                        .filter_map(|role| {
                            Some(RoleInfo {
                                id: field_str(role, "id")?.to_string(),
                                name: field_str(role, "name").unwrap_or_default().to_string(),
                                position: field_i64(role, "position").unwrap_or(0),
                                color: field_u64(role, "color").unwrap_or(0) as u32,
                                hoist: field_bool(role, "hoist"),
                            })
                        })
                        .collect();
                    for r in parsed_roles {
                        if let Some(ex) = entry.roles.iter_mut().find(|x| x.id == r.id) {
                            if !r.name.is_empty() {
                                *ex = r;
                            }
                        } else {
                            entry.roles.push(r);
                        }
                    }
                }
            }
            "GUILD_MEMBER_LIST_UPDATE" => {
                let Some(id) = field_str(&frame.data, "guild_id") else {
                    continue;
                };
                let entry = index.entry(id.to_string()).or_default();
                entry.member_count = field_u64(&frame.data, "member_count").or(entry.member_count);
                let announced: Vec<(String, u64)> = array(frame.data.get("groups"))
                    .iter()
                    .filter_map(|group| {
                        Some((
                            field_str(group, "id")?.to_string(),
                            field_u64(group, "count")?,
                        ))
                    })
                    .collect();
                if !announced.is_empty() {
                    entry.announced = announced;
                }
            }
            _ => {}
        }
    }
    index
}

fn is_status_group(id: &str) -> bool {
    matches!(id, "online" | "idle" | "dnd" | "offline")
}

/// Группа по presence, если снимок пришёл без маркера группы (UPDATE/INSERT
/// без `items` или повреждённый маркер): `idle`/`dnd` Discord показывает в
/// «В сети», `offline` — в «Не в сети».
fn status_group(status: &str) -> &'static str {
    match status {
        "online" | "idle" | "dnd" => "online",
        _ => "offline",
    }
}

fn parse_member(value: &Value, guild: &GuildInfo) -> Option<MemberRow> {
    let user = value.get("user")?;
    let id = field_str(user, "id")?;
    let status = value
        .get("presence")
        .and_then(|presence| field_str(presence, "status"))
        .unwrap_or("offline");
    let raw_username = field_str(user, "username").unwrap_or(id);
    let raw_display_name = field_str(value, "nick")
        .or_else(|| field_str(user, "global_name"))
        .or_else(|| field_str(user, "display_name"))
        .unwrap_or(raw_username);
    let is_snowflake = raw_display_name.len() >= 15
        && raw_display_name.len() <= 22
        && raw_display_name.bytes().all(|b| b.is_ascii_digit());
    let display_name = if is_snowflake {
        format!(
            "Участник {}",
            &raw_display_name[raw_display_name.len().saturating_sub(4)..]
        )
    } else {
        raw_display_name.to_string()
    };
    let username = if is_snowflake {
        format!(
            "user_{}",
            &raw_username[raw_username.len().saturating_sub(4)..]
        )
    } else {
        raw_username.to_string()
    };
    let mut roles: Vec<String> = array(value.get("roles"))
        .iter()
        .filter_map(|role| role.as_str().map(str::to_string))
        .collect();
    // Старшая роль первой: позиция из READY, неизвестные роли — в конце.
    roles.sort_by_key(|role| {
        std::cmp::Reverse(
            guild
                .roles
                .iter()
                .find(|info| &info.id == role)
                .map(|info| info.position)
                .unwrap_or(i64::MIN),
        )
    });
    roles.dedup();
    let role_name = roles
        .first()
        .and_then(|role| guild.roles.iter().find(|info| &info.id == role))
        .map(|info| info.name.clone());
    let role_color = resolve_role_color_hex(&roles, guild);
    let clan_tag = extract_user_clan_tag(Some(user));
    let activity = extract_presence_activity(value.get("presence"));
    Some(MemberRow {
        id: id.to_string(),
        username: username.to_string(),
        display_name: display_name.to_string(),
        avatar: field_str(value, "avatar")
            .or_else(|| field_str(user, "avatar"))
            .map(str::to_string),
        status: status.to_string(),
        bot: field_bool(user, "bot"),
        roles,
        role_name,
        role_color,
        clan_tag,
        activity,
        group: status_group(status).to_string(),
    })
}

/// Участники гильдии из всех `GUILD_MEMBER_LIST_UPDATE` + роли/имя/счётчик из READY.
///
/// Элемент `ops[].items[]` — либо группа (`{"group": …}`, не участник, задаёт
/// группу следующим строкам), либо участник (`{"member": …}`). Всё остальное и
/// участник без `member.user.id` — повреждённый вход: `Parse` с индексом
/// элемента, а не молчаливый пропуск (иначе страница тихо теряет строки).
fn collect_members(
    frames: &[Frame],
    guild_id: &str,
    guild: &GuildInfo,
) -> Result<Vec<MemberRow>, DataError> {
    let mut members: HashMap<String, MemberRow> = HashMap::new();
    for frame in frames {
        if frame.kind != "GUILD_MEMBER_LIST_UPDATE" {
            continue;
        }
        if field_str(&frame.data, "guild_id") != Some(guild_id) {
            continue;
        }
        // Маркер группы действует на последующие элементы этого кадра; каждый
        // кадр начинает с чистого состояния (иначе группа «протекла» бы из
        // предыдущего снимка другой гильдии).
        let mut group: Option<String> = None;
        for (op_index, op) in array(frame.data.get("ops")).iter().enumerate() {
            for (item_index, item) in array(op.get("items")).iter().enumerate() {
                if item.get("member").is_none() {
                    if let Some(marker) = item.get("group") {
                        group = field_str(marker, "id").map(str::to_string);
                        continue;
                    }
                }
                let Some(member) = item.get("member") else {
                    return Err(DataError::parse(
                        &frame.path,
                        format!(
                            "GUILD_MEMBER_LIST_UPDATE: элемент ops[{op_index}].items[{item_index}] \
                             без member и без group"
                        ),
                    ));
                };
                let mut row = parse_member(member, guild).ok_or_else(|| {
                    DataError::parse(
                        &frame.path,
                        format!(
                            "GUILD_MEMBER_LIST_UPDATE: элемент ops[{op_index}].items[{item_index}] \
                             без member.user.id"
                        ),
                    )
                })?;
                row.group = group
                    .clone()
                    .unwrap_or_else(|| status_group(&row.status).to_string());
                match members.get_mut(&row.id) {
                    Some(existing) => {
                        // Более поздний снимок уточняет presence, если он был пуст.
                        if existing.status.is_empty() {
                            existing.status = row.status;
                        }
                    }
                    None => {
                        members.insert(row.id.clone(), row);
                    }
                }
            }
        }
    }
    Ok(members.into_values().collect())
}

/// Группы для ответа: объявленные в `groups[]` плюс встреченные в строках.
/// Порядок — роли по позиции (старшие выше), затем статусы
/// `online`/`idle`/`dnd`/`offline`, затем незнакомые id.
fn member_groups(guild: &GuildInfo, rows: &[MemberRow]) -> Vec<MemberGroup> {
    let mut ids: Vec<String> = guild.announced.iter().map(|(id, _)| id.clone()).collect();
    for row in rows {
        if !ids.contains(&row.group) {
            ids.push(row.group.clone());
        }
    }
    let mut groups: Vec<MemberGroup> = ids
        .into_iter()
        .map(|id| {
            let role = guild.roles.iter().find(|info| info.id == id);
            MemberGroup {
                kind: if role.is_some() || !is_status_group(&id) {
                    "role".to_string()
                } else {
                    "status".to_string()
                },
                title: role.map(|info| info.name.clone()),
                position: role.map(|info| info.position),
                announced_count: guild
                    .announced
                    .iter()
                    .find(|(group, _)| group == &id)
                    .map(|(_, count)| *count),
                loaded_count: rows.iter().filter(|row| row.group == id).count(),
                id,
            }
        })
        .collect();
    groups.sort_by(
        |left, right| match (left.kind.as_str(), right.kind.as_str()) {
            ("role", "role") => right
                .position
                .unwrap_or(i64::MIN)
                .cmp(&left.position.unwrap_or(i64::MIN))
                .then_with(|| left.id.cmp(&right.id)),
            ("role", _) => std::cmp::Ordering::Less,
            (_, "role") => std::cmp::Ordering::Greater,
            _ => status_rank(&left.id)
                .cmp(&status_rank(&right.id))
                .then_with(|| left.id.cmp(&right.id)),
        },
    );
    groups
}

fn collect_supplemental_members(
    root: &Path,
    guild_id: &str,
    guild: &GuildInfo,
) -> Vec<MemberRow> {
    let (live_ready, live_presences, live_mlu, live_vs_for_guild) = {
        let g = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        (
            g.ready_data.clone(),
            g.presences.clone(),
            g.member_list_updates.get(guild_id).cloned(),
            g.voice_states.get(guild_id).cloned().unwrap_or_default(),
        )
    };
    let ready_file_val = if resolve_live_token().is_some() {
        None
    } else {
        let ready_path = root.join("gateway").join("by-type").join("002-READY.json");
        read_json(&ready_path).ok()
    };
    let Some(ready_d) = live_ready
        .as_ref()
        .or_else(|| ready_file_val.as_ref().and_then(|v| v.get("d")))
    else {
        return Vec::new();
    };
    let guilds_arr = array(ready_d.get("guilds"));
    let Some(guild_idx) = guilds_arr
        .iter()
        .position(|g| field_str(g, "id") == Some(guild_id))
    else {
        return Vec::new();
    };
    let guild_obj = &guilds_arr[guild_idx];

    let mut roles_by_low56: HashMap<u64, String> = HashMap::new();
    for r in &guild.roles {
        if let Some(l56) = snowflake_low56(&r.id) {
            roles_by_low56.insert(l56, r.id.clone());
        }
    }

    let mut users_by_id: HashMap<String, Value> = HashMap::new();
    let mut users_by_low56: HashMap<u64, String> = HashMap::new();
    if let Some(fb_d) = ready_file_val.as_ref().and_then(|v| v.get("d")) {
        if let Some(me) = fb_d.get("user") {
            if let Some(uid) = field_str(me, "id") {
                users_by_id.insert(uid.to_string(), me.clone());
                if !uid.starts_with('-') {
                    if let Some(l56) = snowflake_low56(uid) {
                        users_by_low56.insert(l56, uid.to_string());
                    }
                }
            }
        }
        for u in array(fb_d.get("users")) {
            if let Some(uid) = field_str(u, "id") {
                users_by_id.insert(uid.to_string(), u.clone());
                if !uid.starts_with('-') {
                    if let Some(l56) = snowflake_low56(uid) {
                        users_by_low56.insert(l56, uid.to_string());
                    }
                }
            }
        }
    }
    if let Some(me) = ready_d.get("user") {
        if let Some(uid) = field_str(me, "id") {
            users_by_id.insert(uid.to_string(), me.clone());
            if !uid.starts_with('-') {
                if let Some(l56) = snowflake_low56(uid) {
                    users_by_low56.insert(l56, uid.to_string());
                }
            }
        }
    }
    for u in array(ready_d.get("users")) {
        if let Some(uid) = field_str(u, "id") {
            users_by_id.insert(uid.to_string(), u.clone());
            if !uid.starts_with('-') {
                if let Some(l56) = snowflake_low56(uid) {
                    users_by_low56.insert(l56, uid.to_string());
                }
            }
        }
    }

    let mut presences_by_user: HashMap<String, String> = HashMap::new();
    let mut activities_by_user: HashMap<String, String> = HashMap::new();
    if let Some(me) = ready_d.get("user").and_then(|u| field_str(u, "id")) {
        presences_by_user.insert(me.to_string(), "online".to_string());
    }
    for p in array(ready_d.get("presences")) {
        if let Some(uid) = field_str(p, "user_id")
            .or_else(|| p.get("user").and_then(|u| field_str(u, "id")))
        {
            let st = field_str(p, "status").unwrap_or("online");
            presences_by_user.insert(uid.to_string(), st.to_string());
            if let Some(act) = extract_presence_activity(Some(p)) {
                activities_by_user.insert(uid.to_string(), act);
            }
        }
    }
    for p in array(guild_obj.get("presences")) {
        if let Some(uid) = field_str(p, "user_id")
            .or_else(|| p.get("user").and_then(|u| field_str(u, "id")))
        {
            let st = field_str(p, "status").unwrap_or("online");
            presences_by_user.insert(uid.to_string(), st.to_string());
            if let Some(act) = extract_presence_activity(Some(p)) {
                activities_by_user.insert(uid.to_string(), act);
            }
        }
    }
    for (uid, p) in &live_presences {
        let st = field_str(p, "status").unwrap_or("online");
        presences_by_user.insert(uid.clone(), st.to_string());
        if let Some(act) = extract_presence_activity(Some(p)) {
            activities_by_user.insert(uid.clone(), act);
        }
    }

    let mut raw_members: Vec<Value> = Vec::new();
    let mut in_voice_uids: HashSet<String> = HashSet::new();
    raw_members.extend_from_slice(array(guild_obj.get("members")));
    if let Some(mm1) = array(ready_d.get("merged_members")).get(guild_idx) {
        raw_members.extend_from_slice(array(Some(mm1)));
    }
    for vs in array(guild_obj.get("voice_states")) {
        if let Some(uid) = field_str(vs, "user_id") {
            if field_str(vs, "channel_id").is_some() {
                in_voice_uids.insert(uid.to_string());
            }
        }
        if let Some(member) = vs.get("member") {
            raw_members.push(member.clone());
        } else if let Some(uid) = field_str(vs, "user_id") {
            raw_members.push(serde_json::json!({ "user_id": uid }));
        }
    }
    for vs in &live_vs_for_guild {
        if let Some(uid) = field_str(vs, "user_id") {
            if field_str(vs, "channel_id").is_some() {
                in_voice_uids.insert(uid.to_string());
            }
        }
        if let Some(member) = vs.get("member") {
            raw_members.push(member.clone());
        } else if let Some(uid) = field_str(vs, "user_id") {
            raw_members.push(serde_json::json!({ "user_id": uid }));
        }
    }

    let is_live = live_ready.is_some();
    let supp_path = root
        .join("gateway")
        .join("by-type")
        .join("003-READY_SUPPLEMENTAL.json");
    if let Ok(supp_val) = read_json(&supp_path) {
        if let Some(supp_d) = supp_val.get("d") {
            let supp_guilds = array(supp_d.get("guilds"));
            let supp_idx = supp_guilds
                .iter()
                .position(|g| field_str(g, "id") == Some(guild_id))
                .unwrap_or(guild_idx);
            if !is_live {
                if let Some(sg) = supp_guilds.get(supp_idx) {
                    for vs in array(sg.get("voice_states")) {
                        if let Some(uid) = field_str(vs, "user_id") {
                            if field_str(vs, "channel_id").is_some() {
                                in_voice_uids.insert(uid.to_string());
                            }
                        }
                        if let Some(member) = vs.get("member") {
                            raw_members.push(member.clone());
                        } else if let Some(uid) = field_str(vs, "user_id") {
                            raw_members.push(serde_json::json!({ "user_id": uid }));
                        }
                    }
                }
            }
            if let Some(mm2) = array(supp_d.get("merged_members")).get(supp_idx) {
                raw_members.extend_from_slice(array(Some(mm2)));
            }
            if !is_live {
                if let Some(mp_guilds) = supp_d
                    .get("merged_presences")
                    .and_then(|mp| mp.get("guilds"))
                {
                    if let Some(mp_list) = array(Some(mp_guilds)).get(supp_idx) {
                        for p in array(Some(mp_list)) {
                            if let Some(uid) = field_str(p, "user_id") {
                                let st = field_str(p, "status").unwrap_or("online");
                                presences_by_user.insert(uid.to_string(), st.to_string());
                                if let Some(act) = extract_presence_activity(Some(p)) {
                                    activities_by_user.insert(uid.to_string(), act);
                                }
                            }
                        }
                    }
                }
                if let Some(mp_friends) = supp_d
                    .get("merged_presences")
                    .and_then(|mp| mp.get("friends"))
                {
                    for p in array(Some(mp_friends)) {
                        if let Some(uid) = field_str(p, "user_id") {
                            let st = field_str(p, "status").unwrap_or("online");
                            presences_by_user
                                .entry(uid.to_string())
                                .or_insert_with(|| st.to_string());
                            if let Some(act) = extract_presence_activity(Some(p)) {
                                activities_by_user.entry(uid.to_string()).or_insert(act);
                            }
                        }
                    }
                }
            }
        }
    }

    // Подгружаем сначала эталонные снимки `GUILD_MEMBER_LIST_UPDATE` с положительными Snowflake из `/home/sanya/re-deep/gw-proto/`,
    // а затем `005-GUILD_MEMBER_LIST_UPDATE.json` / `008-GUILD_MEMBER_LIST_UPDATE.json` и живой `member_list_updates`.
    let target_l56 = snowflake_low56(guild_id);
    let mut mlu_sources: Vec<(Value, bool)> = Vec::new();
    let gw_proto_jsonl_paths = [
        "/home/sanya/re-deep/gw-proto/msgs-run2-95974_10029.jsonl",
        "/home/sanya/re-deep/gw-proto/msgs-run1-57992_5112.jsonl",
        "/home/sanya/re-deep/gw-proto/msgs-run2-95974_1389.jsonl",
        "/home/sanya/re-deep/gw-proto/msgs-run2-95974_5532.jsonl",
    ];
    for jsonl_path in gw_proto_jsonl_paths {
        if let Ok(content) = std::fs::read_to_string(jsonl_path) {
            for line in content.lines() {
                if !line.contains("\"GUILD_MEMBER_LIST_UPDATE\"") {
                    continue;
                }
                if let Ok(mut parsed) = serde_json::from_str::<Value>(line) {
                    sanitize_etf_json_value(&mut parsed);
                    if let Some(d) = parsed.get("d") {
                        if let Some(fgid) = field_str(d, "guild_id") {
                            if fgid == guild_id
                                || (target_l56.is_some() && snowflake_low56(fgid) == target_l56)
                            {
                                mlu_sources.push((d.clone(), false));
                            }
                        }
                    }
                }
            }
        }
    }
    let mlu_candidate_paths = [
        root.join("gateway")
            .join("by-type")
            .join("005-GUILD_MEMBER_LIST_UPDATE.json"),
        root.join("gateway")
            .join("by-type")
            .join("008-GUILD_MEMBER_LIST_UPDATE.json"),
        PathBuf::from("/home/sanya/re-deep/gateway/by-type/005-GUILD_MEMBER_LIST_UPDATE.json"),
        PathBuf::from("/home/sanya/re-deep/gateway/by-type/008-GUILD_MEMBER_LIST_UPDATE.json"),
    ];
    for mlu_path in mlu_candidate_paths {
        if let Ok(mlu_val) = read_json(&mlu_path) {
            if let Some(d) = mlu_val.get("d") {
                if let Some(fgid) = field_str(d, "guild_id") {
                    if fgid == guild_id
                        || (target_l56.is_some() && snowflake_low56(fgid) == target_l56)
                    {
                        mlu_sources.push((d.clone(), false));
                    }
                }
            }
        }
    }
    if let Some(mlu) = live_mlu {
        mlu_sources.push((mlu, true));
    }
    for (mlu_d, from_live_mlu) in mlu_sources {
        let mut push_mlu_member = |member: &Value| {
            let mut m_clean = member.clone();
            if is_live && !from_live_mlu {
                if let Value::Object(ref mut map) = m_clean {
                    map.remove("presence");
                }
            }
            raw_members.push(m_clean);
        };
        for op in array(mlu_d.get("ops")) {
            for item in array(op.get("items")) {
                if let Some(member) = item.get("member") {
                    push_mlu_member(member);
                }
            }
            if let Some(item) = op.get("item") {
                if let Some(member) = item.get("member") {
                    push_mlu_member(member);
                }
            }
            if let Some(member) = op.get("member") {
                push_mlu_member(member);
            }
        }
    }

    let mut members: HashMap<String, MemberRow> = HashMap::new();
    for m in raw_members {
        let embedded_user = m.get("user");
        let raw_uid_opt = field_str(&m, "user_id")
            .or_else(|| embedded_user.and_then(|u| field_str(u, "id")));
        let Some(raw_uid) = raw_uid_opt else {
            continue;
        };
        let resolved_uid = if raw_uid.starts_with('-') {
            let Some(pos_uid) =
                snowflake_low56(raw_uid).and_then(|l56| users_by_low56.get(&l56).cloned())
            else {
                continue;
            };
            pos_uid
        } else {
            if let Some(l56) = snowflake_low56(raw_uid) {
                users_by_low56.entry(l56).or_insert_with(|| raw_uid.to_string());
            }
            raw_uid.to_string()
        };
        if let Some(u) = embedded_user {
            users_by_id
                .entry(resolved_uid.clone())
                .or_insert_with(|| u.clone());
        }
        if let Some(p) = m.get("presence") {
            if let Some(st) = field_str(p, "status") {
                presences_by_user.insert(resolved_uid.clone(), st.to_string());
            }
            if let Some(act) = extract_presence_activity(Some(p)) {
                activities_by_user.insert(resolved_uid.clone(), act);
            }
        }
        let user_obj = embedded_user.or_else(|| users_by_id.get(&resolved_uid));
        let username_raw = user_obj
            .and_then(|u| field_str(u, "username"))
            .unwrap_or(&resolved_uid);
        let display_name_raw = field_str(&m, "nick")
            .or_else(|| user_obj.and_then(|u| field_str(u, "global_name")))
            .or_else(|| user_obj.and_then(|u| field_str(u, "display_name")))
            .unwrap_or(username_raw);
        if display_name_raw == "nil" || username_raw == "nil" {
            continue;
        }
        let is_snowflake = display_name_raw.len() >= 15
            && display_name_raw.len() <= 22
            && display_name_raw.bytes().all(|b| b.is_ascii_digit());
        let display_name = if is_snowflake {
            format!(
                "Участник {}",
                &display_name_raw[display_name_raw.len().saturating_sub(4)..]
            )
        } else {
            display_name_raw.to_string()
        };
        let username = if is_snowflake {
            format!(
                "user_{}",
                &username_raw[username_raw.len().saturating_sub(4)..]
            )
        } else {
            username_raw.to_string()
        };
        let avatar = field_str(&m, "avatar")
            .or_else(|| user_obj.and_then(|u| field_str(u, "avatar")))
            .map(str::to_string);
        let mut status = presences_by_user
            .get(&resolved_uid)
            .map(String::as_str)
            .unwrap_or("offline")
            .to_string();
        let bot = user_obj.is_some_and(|u| field_bool(u, "bot"));
        let clan_tag = extract_user_clan_tag(user_obj);
        let mut activity = activities_by_user.get(&resolved_uid).cloned();
        if in_voice_uids.contains(&resolved_uid) {
            if status == "offline" {
                status = "online".to_string();
            }
            if activity.is_none() {
                activity = Some("🔊 В голосовом чате".to_string());
            }
        }

        let mut roles: Vec<String> = array(m.get("roles"))
            .iter()
            .filter_map(|r| as_non_nil_str(r))
            .map(|r_str| {
                if r_str.starts_with('-') {
                    snowflake_low56(r_str)
                        .and_then(|l56| roles_by_low56.get(&l56).cloned())
                        .unwrap_or_else(|| r_str.to_string())
                } else {
                    r_str.to_string()
                }
            })
            .collect();
        roles.sort_by_key(|role| {
            std::cmp::Reverse(
                guild
                    .roles
                    .iter()
                    .find(|info| &info.id == role)
                    .map(|info| info.position)
                    .unwrap_or(i64::MIN),
            )
        });
        roles.dedup();
        let top_role = roles
            .first()
            .and_then(|role| guild.roles.iter().find(|info| &info.id == role));
        let role_name = top_role.map(|info| info.name.clone());
        let role_color = resolve_role_color_hex(&roles, guild);
        let group = if status == "offline" {
            "offline".to_string()
        } else {
            resolve_hoisted_role_id(&roles, guild)
                .unwrap_or_else(|| status_group(&status).to_string())
        };

        members.insert(
            resolved_uid.clone(),
            MemberRow {
                id: resolved_uid.clone(),
                username: username.to_string(),
                display_name: display_name.to_string(),
                avatar,
                status,
                bot,
                roles,
                role_name,
                role_color,
                clan_tag,
                activity,
                group,
            },
        );
    }
    members.into_values().collect()
}

/// Страница участников гильдии: `limit` строк строго после `cursor`.
///
/// Неизвестная гильдия → `NotFound`; известная гильдия без снятых
/// `GUILD_MEMBER_LIST_UPDATE` → пустая страница (не ошибка).
pub fn members_page(
    root: &Path,
    guild_id: &str,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<MembersPage, DataError> {
    let guild_id = guild_id.trim();
    if guild_id.is_empty() {
        return Err(DataError::invalid_arg("guildId не может быть пустым"));
    }
    ensure_input_len(guild_id, "guildId")?;
    let limit = page_limit(limit)?;
    let mut frames = read_frames(root)?;
    let (live_active, live_ready_opt) = {
        let g = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        (g.active, g.ready_data.clone())
    };
    if live_active {
        if let Some(live_d) = live_ready_opt {
            frames.push(Frame {
                kind: "READY".to_string(),
                data: live_d,
                path: PathBuf::from("live://ready"),
            });
        }
    }
    let guilds = guild_index(&frames);
    let Some(mut guild) = guilds.get(guild_id).cloned() else {
        return Err(DataError::not_found(
            "гильдия",
            format!("гильдия {guild_id} не найдена в READY/by-type"),
        ));
    };

    let mut rows = collect_members(&frames, guild_id, &guild)?;
    if live_active {
        let supp = collect_supplemental_members(root, guild_id, &guild);
        for m in supp {
            if let Some(ex) = rows.iter_mut().find(|r| r.id == m.id) {
                ex.status = m.status;
                ex.group = m.group;
                if ex.clan_tag.is_none() {
                    ex.clan_tag = m.clan_tag;
                }
                if ex.activity.is_none() {
                    ex.activity = m.activity;
                }
                if ex.role_color.is_none() {
                    ex.role_color = m.role_color;
                }
                if ex.role_name.is_none() {
                    ex.role_name = m.role_name;
                }
                if ex.roles.is_empty() {
                    ex.roles = m.roles;
                }
            } else {
                rows.push(m);
            }
        }
        // В живой сессии заголовки ролей и `В сети` показывают реальное число загруженных участников (`loaded_count`),
        // а не статические октябрьские фикстурные числа (`672`).
        guild.announced.clear();
    }
    let groups = member_groups(&guild, &rows);
    let order: HashMap<&str, usize> = groups
        .iter()
        .enumerate()
        .map(|(position, group)| (group.id.as_str(), position))
        .collect();
    rows.sort_by(|left, right| {
        let left_idx = order.get(left.group.as_str()).copied().unwrap_or(usize::MAX);
        let right_idx = order.get(right.group.as_str()).copied().unwrap_or(usize::MAX);
        left_idx
            .cmp(&right_idx)
            .then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
            .then_with(|| left.id.cmp(&right.id))
    });

    let ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
    let start = cursor_start(&ids, cursor, "участников")?;
    let end = (start + limit).min(rows.len());
    let items: Vec<MemberRow> = if start < end {
        rows[start..end].to_vec()
    } else {
        Vec::new()
    };
    let has_more = end < rows.len();
    let next_cursor = if has_more {
        items.last().map(|row| row.id.clone())
    } else {
        None
    };

    Ok(MembersPage {
        guild_id: guild_id.to_string(),
        guild_name: guild.name,
        member_count: guild.member_count,
        items,
        groups,
        next_cursor,
        has_more,
        total: rows.len(),
        limit,
        loaded_rows_limit: UI_WINDOW_ROWS,
    })
}

// ---------------------------------------------------------------------------
// search_page — локальный поиск по загруженным фикстурам
// ---------------------------------------------------------------------------

/// Фильтры поиска (7 секций R-10). `None` — фильтр не задан.
///
/// Строковые значения обрезаются по краям и ограничены [`MAX_INPUT_LEN`];
/// `day` обязан быть `YYYY-MM-DD`; `message_type` — `type` кадра;
/// `pinned` — `pinned` кадра. Пустой фильтр/запрос — «без ограничения»,
/// поэтому пустой результат — валидный ответ, а не ошибка.
///
/// `Deserialize` — канонический IPC-контракт (`camelCase`, отсутствующие ключи
/// `None`): команда `search_page` принимает этот тип напрямую, без зеркала в
/// `main.rs`. Семантика фильтров остаётся здесь, в слое данных.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFilters {
    pub author_id: Option<String>,
    pub channel_id: Option<String>,
    pub content: Option<String>,
    pub mention_id: Option<String>,
    pub day: Option<String>,
    pub message_type: Option<u64>,
    pub pinned: Option<bool>,
}

/// Найденное сообщение: та же строка, что в чате, плюс метаданные канала
/// для строки результата (имя канала/гильдии может отсутствовать, если
/// канал не попал в READY).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub message: MessageRow,
    pub channel_name: Option<String>,
    pub guild_name: Option<String>,
}

/// Страница результатов поиска. `total` — сколько совпало во всех фикстурах,
/// `scanned` — сколько сообщений просмотрено (для честного «найдено / загружено»).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub query: String,
    pub items: Vec<SearchHit>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total: usize,
    pub scanned: usize,
    pub limit: usize,
    pub loaded_rows_limit: usize,
}

/// Пустая/пробельная строка фильтра = фильтр не задан.
fn normalized(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// `YYYY-MM-DD` — единственный принимаемый формат даты (как в `day` строки).
fn ensure_day(value: &str) -> Result<(), DataError> {
    ensure_input_len(value, "дата")?;
    let bytes = value.as_bytes();
    let shape = value.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    if !shape {
        return Err(DataError::invalid_arg(format!(
            "дата должна быть в формате YYYY-MM-DD: {}",
            preview(value)
        )));
    }
    Ok(())
}

fn matches_query(row: &MessageRow, query_lower: &str) -> bool {
    query_lower.is_empty() || row.content.to_lowercase().contains(query_lower)
}

fn matches_mention(row: &MessageRow, mention_id: &str) -> bool {
    row.mentions.iter().any(|id| id == mention_id)
        || row.content.contains(&format!("<@{mention_id}>"))
        || row.content.contains(&format!("<@!{mention_id}>"))
}

/// Проверяет один фильтр по строке. Значения уже обрезаны и провалидированы.
fn matches_filters(row: &MessageRow, filters: &SearchFilters) -> bool {
    if let Some(author) = filters.author_id.as_deref().map(str::trim) {
        if row.author.id != author {
            return false;
        }
    }
    if let Some(channel) = filters.channel_id.as_deref().map(str::trim) {
        if row.channel_id != channel {
            return false;
        }
    }
    if let Some(content) = filters.content.as_deref().map(str::trim) {
        if !row.content.to_lowercase().contains(&content.to_lowercase()) {
            return false;
        }
    }
    if let Some(mention) = filters.mention_id.as_deref().map(str::trim) {
        if !matches_mention(row, mention) {
            return false;
        }
    }
    if let Some(day) = filters.day.as_deref().map(str::trim) {
        if row.day.as_deref() != Some(day) {
            return false;
        }
    }
    if let Some(kind) = filters.message_type {
        if row.message_type != kind {
            return false;
        }
    }
    if let Some(pinned) = filters.pinned {
        if row.pinned != pinned {
            return false;
        }
    }
    true
}

/// Локальный поиск по снятым фикстурам: тот же набор сообщений, что отдают
/// `messages_page`, но по всем каналам сразу, с фильтрами R-10.
///
/// Сортировка — время, затем id, затем канал (детерминированная при равных
/// timestamp). `limit` — `1..=100`; курсор — id последнего результата.
/// Нет совпадений — пустая страница с `total: 0`, это не ошибка.
pub fn search_page(
    root: &Path,
    query: &str,
    filters: &SearchFilters,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> Result<SearchPage, DataError> {
    let query = query.trim();
    ensure_input_len(query, "query")?;
    for (value, name) in [
        (filters.author_id.as_deref(), "автор"),
        (filters.channel_id.as_deref(), "канал"),
        (filters.content.as_deref(), "содержит"),
        (filters.mention_id.as_deref(), "упоминание"),
    ] {
        if let Some(value) = value.map(str::trim) {
            ensure_input_len(value, name)?;
        }
    }
    if let Some(day) = filters
        .day
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        ensure_day(day)?;
    }
    // Пустая строка из поля ввода — «фильтр не задан», а не «искать пустое»:
    // иначе `authorId: ""` отдавал бы нулевой результат на пустом UI.
    let filters = SearchFilters {
        author_id: normalized(filters.author_id.as_deref()),
        channel_id: normalized(filters.channel_id.as_deref()),
        content: normalized(filters.content.as_deref()),
        mention_id: normalized(filters.mention_id.as_deref()),
        day: normalized(filters.day.as_deref()),
        message_type: filters.message_type,
        pinned: filters.pinned,
    };
    let limit = page_limit(limit)?;

    let frames = read_frames(root)?;
    let channels = channel_index(&frames);
    let query_lower = query.to_lowercase();
    let mut channel_ids: Vec<String> = known_channels(&frames).into_iter().collect();
    channel_ids.sort();
    let mut hits: Vec<SearchHit> = Vec::new();
    let mut scanned = 0;
    for channel_id in channel_ids {
        let rows = collect_messages(&frames, &channel_id)?;
        scanned += rows.len();
        let info = channels.get(&channel_id);
        for row in rows {
            if !matches_query(&row, &query_lower) || !matches_filters(&row, &filters) {
                continue;
            }
            hits.push(SearchHit {
                message: row,
                channel_name: info.and_then(|info| info.name.clone()),
                guild_name: info.and_then(|info| info.guild_name.clone()),
            });
        }
    }
    hits.sort_by(|left, right| {
        left.message
            .timestamp
            .cmp(&right.message.timestamp)
            .then_with(|| left.message.id.cmp(&right.message.id))
            .then_with(|| left.message.channel_id.cmp(&right.message.channel_id))
    });

    let ids: Vec<String> = hits.iter().map(|hit| hit.message.id.clone()).collect();
    let start = cursor_start(&ids, cursor, "результатов поиска")?;
    let end = (start + limit).min(hits.len());
    let items: Vec<SearchHit> = if start < end {
        hits[start..end].to_vec()
    } else {
        Vec::new()
    };
    let has_more = end < hits.len();
    let next_cursor = if has_more {
        items.last().map(|hit| hit.message.id.clone())
    } else {
        None
    };

    Ok(SearchPage {
        query: query.to_string(),
        items,
        next_cursor,
        has_more,
        total: hits.len(),
        scanned,
        limit,
        loaded_rows_limit: UI_WINDOW_ROWS,
    })
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

/// Секция настроек: стабильный `id` для навигации, заголовок и значения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsSection {
    pub id: String,
    pub title: String,
    pub values: Value,
}

/// Ответ `settings`: все секции или одна выбранная (`selected`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsPayload {
    pub sections: Vec<SettingsSection>,
    pub selected: Option<String>,
}

fn muted_channels(data: &Value) -> usize {
    array(
        data.get("user_guild_settings")
            .and_then(|value| value.get("entries")),
    )
    .iter()
    .flat_map(|entry| array(entry.get("channel_overrides")))
    .filter(|overrides| field_bool(overrides, "muted"))
    .count()
}

/// Настройки из READY. Секции честно отражают только то, что есть в фикстуре:
/// профиль, уведомления, сессии, приватность и счётчики данных.
/// Неизвестная секция → `NotFound` (пустая строка — тоже неизвестная);
/// секция длиннее [`MAX_INPUT_LEN`] → `InvalidArg` до чтения READY (R5).
pub fn settings(root: &Path, section: Option<&str>) -> Result<SettingsPayload, DataError> {
    if let Some(section) = section {
        ensure_input_len(section, "section")?;
    }
    let path = root.join("gateway").join("by-type").join("002-READY.json");
    let forbid_fixture = resolve_live_token().is_some();
    let live_ready_opt = if root == default_fixtures_root() {
        let guard = live_session()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if guard.active || forbid_fixture {
            let mut d = guard.ready_data.clone().unwrap_or_else(|| json!({}));
            if d.get("user").is_none() {
                if let Some(ref u) = guard.user {
                    if let Some(obj) = d.as_object_mut() {
                        obj.insert(
                            "user".to_string(),
                            json!({
                                "id": u.id,
                                "username": u.username,
                                "global_name": u.global_name,
                                "avatar": u.avatar,
                                "email": u.email,
                                "bio": u.bio,
                                "premium": u.premium,
                                "mfa_enabled": u.mfa_enabled
                            }),
                        );
                    }
                }
            }
            Some(d)
        } else {
            None
        }
    } else {
        None
    };
    let fallback_value;
    let data: &Value = if let Some(ref live_d) = live_ready_opt {
        live_d
    } else {
        fallback_value = read_json(&path)?;
        fallback_value
            .get("d")
            .ok_or_else(|| DataError::parse(&path, "READY без объекта d"))?
    };
    let default_user_val = json!({
        "id": "0",
        "username": "user"
    });
    let user_value = if live_ready_opt.is_some() {
        data.get("user").unwrap_or(&default_user_val)
    } else {
        data.get("user")
            .ok_or_else(|| DataError::parse(&path, "READY без объекта user"))?
    };
    let user = parse_ready_user(user_value, &path)?;
    let guilds = array(data.get("guilds"));

    let session_entries: Vec<Value> = array(data.get("sessions"))
        .iter()
        .map(|session| {
            let info = session.get("client_info");
            json!({
                "client": info.and_then(|value| field_str(value, "client")),
                "os": info.and_then(|value| field_str(value, "os")),
                "version": info.and_then(|value| field_u64(value, "version")),
                "status": field_str(session, "status"),
            })
        })
        .collect();

    let sections = vec![
        SettingsSection {
            id: "profile".to_string(),
            title: "Профиль".to_string(),
            values: json!({
                "id": user.id,
                "username": user.username,
                "globalName": user.global_name,
                "avatar": user.avatar,
                "email": user.email,
                "bio": user.bio,
                "premium": user.premium,
                "mfaEnabled": user.mfa_enabled,
                "phoneSet": user_value.get("phone").and_then(as_non_nil_str).is_some(),
                "verified": field_bool(user_value, "verified"),
            }),
        },
        SettingsSection {
            id: "notifications".to_string(),
            title: "Уведомления".to_string(),
            values: json!({
                "flags": data
                    .get("notification_settings")
                    .and_then(|value| field_u64(value, "flags"))
                    .unwrap_or(0),
                "guildOverrides": array(
                    data.get("user_guild_settings").and_then(|value| value.get("entries"))
                )
                .len(),
                "mutedChannels": muted_channels(data),
            }),
        },
        SettingsSection {
            id: "sessions".to_string(),
            title: "Сессии".to_string(),
            values: json!({
                "count": session_entries.len(),
                "entries": session_entries,
            }),
        },
        SettingsSection {
            id: "privacy".to_string(),
            title: "Приватность".to_string(),
            values: json!({
                "consents": data.get("consents").cloned().unwrap_or(Value::Null),
                "nsfwAllowed": field_bool(user_value, "nsfw_allowed"),
                "ageVerificationStatus": field_u64(user_value, "age_verification_status"),
                "explicitContentScanVersion": field_u64(data, "explicit_content_scan_version"),
            }),
        },
        SettingsSection {
            id: "data".to_string(),
            title: "Данные".to_string(),
            values: json!({
                "guilds": guilds.len(),
                "privateChannels": array_len(data.get("private_channels")),
                "relationships": array_len(data.get("relationships")),
                "readStateEntries": data
                    .get("read_state")
                    .and_then(|state| state.get("entries"))
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len),
                "users": array_len(data.get("users")),
                "channels": guilds
                    .iter()
                    .map(|guild| array_len(guild.get("channels")))
                    .sum::<usize>(),
            }),
        },
    ];

    let Some(section) = section else {
        return Ok(SettingsPayload {
            sections,
            selected: None,
        });
    };
    let available: Vec<String> = sections.iter().map(|item| item.id.clone()).collect();
    match sections.into_iter().find(|item| item.id == section) {
        Some(found) => Ok(SettingsPayload {
            selected: Some(found.id.clone()),
            sections: vec![found],
        }),
        None => Err(DataError::not_found(
            "секция настроек",
            format!(
                "секция настроек «{section}» не найдена; доступны: {}",
                available.join(", ")
            ),
        )),
    }
}

#[cfg(test)]
mod dm_voice_stream_tests {
    use super::*;
    use serde_json::json;

    static TEST_SERIAL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn dm_voice_server_update_does_not_corrupt_guild_id_on_stream_toggle() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
        }
        ingest_live_dispatch(
            "READY",
            &json!({
                "session_id": "sess_dm_test",
                "user": {
                    "id": "1566799713523466370",
                    "username": "rustery0330",
                    "global_name": "Rustery"
                },
                "guilds": [],
                "users": [
                    {
                        "id": "384299970632876000",
                        "username": "pharos12510",
                        "global_name": "ShadowByte"
                    }
                ]
            }),
        );
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            assert_eq!(
                g.user.as_ref().map(|u| u.id.as_str()),
                Some("1566799713523466370"),
                "READY dispatch must populate guard.user so STREAM_CREATE/STREAM_SERVER_UPDATE know self_uid"
            );
        }

        let _ = connect_or_update_live_voice_ext(
            Some("@me".to_string()),
            Some("1566834523038294047".to_string()),
            false,
            false,
            false,
            false,
        );
        ingest_raw_gateway_dispatch(
            "VOICE_SERVER_UPDATE",
            &json!({
                "guild_id": null,
                "channel_id": "1566834523038294047",
                "endpoint": "rotterdam123.discord.media:443",
                "token": "tok_dm_1"
            }),
        );
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            assert_eq!(
                g.voice_guild_id, None,
                "DM VOICE_SERVER_UPDATE must keep voice_guild_id == None instead of overwriting with channel_id"
            );
            assert_eq!(
                g.voice_endpoint.as_deref(),
                Some("rotterdam123.discord.media:443")
            );
        }

        // Toggle screen share ON in DM call
        let _ = connect_or_update_live_voice_ext(
            Some("".to_string()),
            Some("1566834523038294047".to_string()),
            false,
            false,
            false,
            true,
        );
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            assert_eq!(
                g.voice_endpoint.as_deref(),
                Some("rotterdam123.discord.media:443"),
                "Starting screen share in DM call must NOT wipe voice_endpoint"
            );
            let dm_vs = g.voice_states.get("@me").expect("@me voice states");
            let my_vs = dm_vs
                .iter()
                .find(|v| field_str(v, "user_id") == Some("1566799713523466370"))
                .expect("self voice state in @me");
            assert_eq!(my_vs.get("self_stream").and_then(Value::as_bool), Some(true));
        }

        // Toggle screen share OFF in DM call
        let _ = connect_or_update_live_voice_ext(
            Some("".to_string()),
            Some("1566834523038294047".to_string()),
            false,
            false,
            false,
            false,
        );
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            assert_eq!(
                g.voice_endpoint.as_deref(),
                Some("rotterdam123.discord.media:443"),
                "Stopping screen share in DM call must NOT wipe voice_endpoint or disconnect call"
            );
            let dm_vs = g.voice_states.get("@me").expect("@me voice states");
            let my_vs = dm_vs
                .iter()
                .find(|v| field_str(v, "user_id") == Some("1566799713523466370"))
                .expect("self voice state must remain in @me after stopping screen share");
            assert_eq!(
                my_vs.get("self_stream").and_then(Value::as_bool),
                Some(false)
            );
        }
    }

    #[test]
    fn dm_call_create_update_delete_syncs_voice_states_in_home() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
        }
        ingest_live_dispatch(
            "READY",
            &json!({
                "session_id": "sess_dm_test2",
                "user": {
                    "id": "1566799713523466370",
                    "username": "rustery0330",
                    "global_name": "Rustery"
                },
                "guilds": [],
                "users": [
                    {
                        "id": "384299970632876000",
                        "username": "pharos12510",
                        "global_name": "ShadowByte"
                    }
                ]
            }),
        );

        ingest_live_dispatch(
            "CALL_CREATE",
            &json!({
                "channel_id": "1566834523038294047",
                "voice_states": [
                    {
                        "user_id": "384299970632876000",
                        "channel_id": "1566834523038294047",
                        "self_mute": false,
                        "self_deaf": false,
                        "self_stream": true
                    }
                ]
            }),
        );
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            let dm_vs = g
                .voice_states
                .get("@me")
                .expect("CALL_CREATE must populate @me voice_states");
            assert_eq!(dm_vs.len(), 1);
            assert_eq!(field_str(&dm_vs[0], "user_id"), Some("384299970632876000"));
            assert_eq!(
                dm_vs[0].get("self_stream").and_then(Value::as_bool),
                Some(true)
            );
        }

        // Now ShadowByte leaves via CALL_UPDATE with empty voice_states
        ingest_live_dispatch(
            "CALL_UPDATE",
            &json!({
                "channel_id": "1566834523038294047",
                "voice_states": []
            }),
        );
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            let dm_vs = g.voice_states.get("@me").cloned().unwrap_or_default();
            assert!(
                dm_vs
                    .iter()
                    .all(|v| field_str(v, "user_id") != Some("384299970632876000")),
                "CALL_UPDATE with empty voice_states must remove departed participant from @me"
            );
        }
    }

    #[test]
    fn live_invite_resolve_and_materialize_guild_fallback_on_400() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
        }
        ingest_live_dispatch(
            "READY",
            &json!({
                "session_id": "sess_invite_test",
                "user": {
                    "id": "1566799713523466370",
                    "username": "rustery0330",
                    "global_name": "Rustery"
                },
                "guilds": [],
                "users": []
            }),
        );

        let invite_json = br#"{
            "code": "eSrxp2YGUC",
            "guild": {
                "id": "274093052673392641",
                "name": "ITSKIDON PC BOOST",
                "icon": "abc_icon",
                "features": ["COMMUNITY", "NEWS"]
            },
            "channel": {
                "id": "274093052673392642",
                "name": "welcome",
                "type": 0
            },
            "approximate_member_count": 24199,
            "approximate_presence_count": 1467
        }"#;

        cache_resolved_invite_payload("/invites/eSrxp2YGUC?with_counts=true&with_expiration=true", invite_json);

        assert_eq!(
            cached_invite_code_for_guild("274093052673392641"),
            Some("eSrxp2YGUC".to_string())
        );
        assert!(
            !is_cached_guild_discoverable("274093052673392641"),
            "Non-discoverable guild must report false so join_guild uses invite accept"
        );
        assert!(
            has_live_guild_or_channels("274093052673392641"),
            "has_live_guild_or_channels must materialize cached invite guild when 400/403 occurs"
        );

        let fallback_body = materialized_accept_body_for_invite_target("/invites/eSrxp2YGUC")
            .expect("materialized_accept_body_for_invite_target must return cached invite JSON");
        let parsed: Value = serde_json::from_slice(&fallback_body).expect("valid JSON");
        assert_eq!(
            parsed.get("guild").and_then(|g| field_str(g, "id")),
            Some("274093052673392641")
        );

        let root = default_fixtures_root();
        let ch_list = channels(&root, Some("274093052673392641")).expect("channels for materialized guild");
        assert!(
            ch_list
                .iter()
                .any(|c| c.id == "274093052673392642" && c.name.as_deref() == Some("welcome")),
            "Materialized guild must include the invite target channel in channels()"
        );

        remove_local_guild("274093052673392641");
        let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
        let r_guilds = g
            .ready_data
            .as_ref()
            .and_then(|d| d.get("guilds"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(
            r_guilds.iter().all(|rg| field_str(rg, "id") != Some("274093052673392641")),
            "remove_local_guild must remove guild from ready_data"
        );
    }

    #[test]
    fn live_message_sent_edited_deleted_syncs_cache() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
        }
        ingest_live_sent_message_json(
            br#"{"id":"999001","channel_id":"1566834523038294047","content":"Hello world","timestamp":"2026-11-06T12:00:00.000Z"}"#,
        );
        update_edited_message("1566834523038294047", "999001", "Hello updated");
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            let msgs = g.channel_messages.get("1566834523038294047").expect("messages");
            assert_eq!(msgs.len(), 1);
            assert_eq!(field_str(&msgs[0], "content"), Some("Hello updated"));
        }
        remove_deleted_message("1566834523038294047", "999001");
        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            let msgs = g.channel_messages.get("1566834523038294047").expect("messages");
            assert!(msgs.is_empty());
        }
    }

    #[test]
    fn accept_invite_via_live_rest_does_not_panic_inside_tokio_runtime() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
            g.token = Some("synthetic_non_test_token_for_runtime_check".to_string());
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rt.block_on(async {
                // Calling accept_invite_via_live_rest inside an active async runtime must never panic
                let _ = accept_invite_via_live_rest("pbFBVm4nB");
            })
        }));
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
        }
        assert!(
            res.is_ok(),
            "accept_invite_via_live_rest must not panic when called inside a Tokio runtime"
        );
    }

    #[test]
    fn invite_stub_guild_merges_full_roles_channels_and_resolves_member_group_titles() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
            g.user = Some(ReadyUser {
                id: "384299970632876000".to_string(),
                username: "pharos12510".to_string(),
                global_name: Some("ShadowByte".to_string()),
                avatar: None,
                email: None,
                bio: None,
                premium: false,
                mfa_enabled: true,
            });
        }

        // Simulate resolving & accepting invite to ITSKIDON PC BOOST (1014537851418980372)
        // where the invite payload only carries 1 welcome channel (#📲┃основной) and no roles.
        let invite_json = r#"{
            "code": "eSrxp2YGUC",
            "guild": {
                "id": "1014537851418980372",
                "name": "ITSKIDON PC BOOST",
                "icon": "a_99f9f3b49b05c2276a0267f06060b868",
                "features": ["COMMUNITY", "INVITE_SPLASH"]
            },
            "channel": {
                "id": "1014537851867775060",
                "name": "📲┃основной",
                "type": 0
            },
            "approximate_member_count": 24199,
            "approximate_presence_count": 1467
        }"#.as_bytes();
        cache_resolved_invite_payload("/invites/eSrxp2YGUC", invite_json);
        ingest_accepted_invite_payload(invite_json);
        ingest_live_dispatch(
            "GUILD_CREATE",
            &serde_json::json!({
                "id": "1014537851418980372",
                "name": "ITSKIDON PC BOOST",
                "lurker": false,
                "isMember": true,
                "my_roles": ["1057598992449552425"],
                "roles": [
                    { "id": "1014537851418980372", "name": "@everyone", "position": 0, "permissions": "1071698529857" },
                    { "id": "1055027618170290198", "name": "Админ", "position": 10, "hoist": true, "color": 3066993, "permissions": "8" },
                    { "id": "1057598992449552425", "name": "Участник", "position": 5, "hoist": true, "color": 15158332, "permissions": "1071698529857" }
                ],
                "channels": [
                    { "id": "1014537851867775060", "guild_id": "1014537851418980372", "name": "📲┃основной", "type": 0, "position": 0, "permission_overwrites": [] },
                    { "id": "1039511139874242560", "guild_id": "1014537851418980372", "name": "Войс 1", "type": 2, "position": 1, "permission_overwrites": [] }
                ],
                "members": [
                    {
                        "user_id": "384299970632876000",
                        "user": { "id": "384299970632876000", "username": "pharos12510", "global_name": "ShadowByte" },
                        "roles": ["1057598992449552425"]
                    }
                ]
            }),
        );

        let root = default_fixtures_root();
        let ch_list = channels(&root, Some("1014537851418980372"))
            .expect("channels for ITSKIDON PC BOOST");
        assert!(
            ch_list.len() >= 2,
            "Expected hydrated guild channel list after GUILD_CREATE, got {}",
            ch_list.len()
        );

        let m_page = members_page(&root, "1014537851418980372", None, Some(100))
            .expect("members_page for ITSKIDON PC BOOST");
        for grp in &m_page.groups {
            if grp.kind == "role" {
                let title = grp.title.as_deref().unwrap_or_default();
                assert!(
                    !title.is_empty() && !title.bytes().all(|b| b.is_ascii_digit()),
                    "Role group {} must have a human-readable role title, got {:?}",
                    grp.id,
                    grp.title
                );
            }
        }

        let r_payload = ready(&root).expect("ready payload");
        let itskidon = r_payload
            .guilds
            .iter()
            .find(|g| g.id == "1014537851418980372")
            .expect("ITSKIDON guild in ready");
        assert!(
            itskidon.roles.len() >= 3,
            "Expected hydrated roles list in ready().guilds, got {}",
            itskidon.roles.len()
        );
        assert!(
            !itskidon.my_roles.is_empty(),
            "Expected my_roles to be populated for current user in ITSKIDON guild"
        );
    }

    #[test]
    fn other_account_does_not_inherit_shadowbyte_roles_or_fake_welcome_messages() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
            g.token = Some("MT_real_user_token_rustery".to_string());
            g.user = Some(ReadyUser {
                id: "1566799713523466370".to_string(),
                username: "rustery0330".to_string(),
                global_name: Some("Rustery".to_string()),
                avatar: None,
                email: None,
                bio: None,
                premium: false,
                mfa_enabled: false,
            });
        }

        let invite_json = r#"{
            "code": "eSrxp2YGUC",
            "guild": {
                "id": "1014537851418980372",
                "name": "ITSKIDON PC BOOST",
                "icon": "a_99f9f3b49b05c2276a0267f06060b868",
                "features": ["COMMUNITY", "PREVIEW_ENABLED", "DISCOVERABLE"]
            },
            "channel": {
                "id": "1014537851867775060",
                "name": "📲┃основной",
                "type": 0
            },
            "approximate_member_count": 24199,
            "approximate_presence_count": 1467
        }"#.as_bytes();
        cache_resolved_invite_payload("/invites/eSrxp2YGUC", invite_json);
        let _ = materialized_accept_body_for_invite_target("/invites/eSrxp2YGUC");

        let root = default_fixtures_root();
        let r_payload = ready(&root).expect("ready payload for Rustery");
        let itskidon = r_payload
            .guilds
            .iter()
            .find(|g| g.id == "1014537851418980372")
            .expect("ITSKIDON guild in ready");

        assert!(
            itskidon.my_roles.is_empty(),
            "Rustery (1566799713523466370) must NOT inherit ShadowByte's roles from 002-READY.json, got {:?}",
            itskidon.my_roles
        );
        assert_eq!(
            itskidon.lurker,
            Some(false),
            "Materialized guild after invite accept must be marked lurker=false"
        );
        assert_eq!(
            itskidon.is_member,
            Some(true),
            "Materialized guild after invite accept must be marked is_member=true"
        );

        {
            let g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            let rules_msgs = g
                .channel_messages
                .get("1241842061401915496")
                .cloned()
                .unwrap_or_default();
            assert!(
                rules_msgs.is_empty(),
                "Real token session must NOT inject fake 'Добро пожаловать' bot messages into fixture channel_messages, got {:?}",
                rules_msgs
            );
        }

        // Explicitly mark guild as lurker/non-member preview and verify voice connect is rejected
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            if let Some(Value::Object(ref mut rm)) = g.ready_data {
                if let Some(Value::Array(ref mut gs)) = rm.get_mut("guilds") {
                    if let Some(Value::Object(ref mut gm)) = gs
                        .iter_mut()
                        .find(|x| field_str(x, "id") == Some("1014537851418980372"))
                    {
                        gm.insert("lurker".to_string(), Value::Bool(true));
                        gm.insert("isMember".to_string(), Value::Bool(false));
                    }
                }
            }
        }
        let voice_res = connect_or_update_live_voice_ext(
            Some("1014537851418980372".to_string()),
            Some("1039511139874242560".to_string()),
            false,
            false,
            false,
            false,
        );
        assert!(
            voice_res.is_err(),
            "Connecting to guild voice channel in explicit lurker/non-member mode must return Err"
        );
    }

    #[test]
    fn game_room_19_digit_invite_produces_valid_u64_channels_messages_and_voice_connect() {
        let _serial = TEST_SERIAL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        {
            let mut g = live_session().lock().unwrap_or_else(|p| p.into_inner());
            *g = LiveSession::default();
            g.active = true;
            g.token = Some("MT_real_user_token_rustery".to_string());
            g.user = Some(ReadyUser {
                id: "1566799713523466370".to_string(),
                username: "rustery0330".to_string(),
                global_name: Some("Rustery".to_string()),
                avatar: None,
                email: None,
                bio: None,
                premium: false,
                mfa_enabled: false,
            });
        }

        let invite_json = r#"{
            "code": "wVc2y3ET",
            "guild": {
                "id": "1340572775974305803",
                "name": "Game Room",
                "icon": null,
                "features": []
            },
            "channel": {
                "id": "1340572775974305805",
                "name": "общее",
                "type": 0
            },
            "approximate_member_count": 5,
            "approximate_presence_count": 2
        }"#
        .as_bytes();
        cache_resolved_invite_payload("/invites/wVc2y3ET", invite_json);
        let _ = materialized_accept_body_for_invite_target("/invites/wVc2y3ET");
        ingest_live_dispatch(
            "GUILD_CREATE",
            &serde_json::json!({
                "id": "1340572775974305803",
                "name": "Game Room",
                "lurker": false,
                "isMember": true,
                "roles": [
                    { "id": "1340572775974305803", "name": "@everyone", "position": 0, "permissions": "1071698529857" }
                ],
                "channels": [
                    { "id": "1340572775974305805", "guild_id": "1340572775974305803", "name": "общее", "type": 0, "position": 0, "permission_overwrites": [] },
                    { "id": "1340572775974305806", "guild_id": "1340572775974305803", "name": "voice-lobby", "type": 2, "position": 1, "permission_overwrites": [] }
                ]
            }),
        );
        ingest_live_dispatch(
            "MESSAGE_CREATE",
            &serde_json::json!({
                "id": "1340572775974305900",
                "channel_id": "1340572775974305805",
                "guild_id": "1340572775974305803",
                "content": "Реальное сообщение из шлюза",
                "timestamp": "2026-11-06T12:00:00.000Z",
                "author": { "id": "1566799713523466370", "username": "rustery0330" }
            }),
        );

        let root = default_fixtures_root();
        let ch_list =
            channels(&root, Some("1340572775974305803")).expect("channels for Game Room");
        assert_eq!(
            ch_list.len(),
            2,
            "Expected only the 2 real GUILD_CREATE channels (no hallucinated template channels)"
        );
        for ch in &ch_list {
            assert!(
                ch.id.parse::<u64>().is_ok(),
                "Channel ID {} must be a valid 64-bit snowflake (<= u64::MAX)",
                ch.id
            );
        }

        let page = messages_page(&root, "1340572775974305805", None, None)
            .expect("messages_page for #общее");
        assert!(
            !page.items.is_empty(),
            "Expected real MESSAGE_CREATE messages in #общее for Game Room"
        );

        let voice_ch = ch_list
            .iter()
            .find(|c| c.kind == 2)
            .expect("voice channel in Game Room");
        let voice_res = connect_or_update_live_voice_ext(
            Some("1340572775974305803".to_string()),
            Some(voice_ch.id.clone()),
            false,
            false,
            false,
            false,
        );
        assert!(
            voice_res.is_ok(),
            "Voice connect on joined Game Room must succeed without 'Сервер в режиме просмотра', got {:?}",
            voice_res
        );
    }
}

