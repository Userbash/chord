#![forbid(unsafe_code)]
//! Чистая логика GUI-спайка: без Tauri-рантайма, чтобы контракт IPC проверялся
//! обычными тестами (`tests/ipc_contract.rs`), а не только живым окном.
//!
//! Здесь три вещи:
//! - разбор `/proc/self/status` в метрики процесса (RSS — именно VmRSS, не VmHWM);
//! - проверка `UI_SNAPSHOT` по порогам (4 экрана / 100k чата / 24 229 участников /
//!   10 категорий / 38 тумблеров / rendered ≥10);
//! - проверка `BENCH`-снимка как JSON с объектами chat/members.
//!
//! BFF фазы 3 — в модуле [`data`]: `ready`, `read_state`, `channels`,
//! `messages_page`, `members_page`, `settings` читают только локальные
//! `fixtures/` (корень — `DL_FIXTURES`).
//!
//! W39j — пилот [`core_bridge`]: та же страница сообщений, но собранная из
//! `discord-core` (`state::Message`/`history::History`), с тем же курсором.
//!
//! RW-39/NET-C2 — [`auth`]: импорт токена владельца, `GET /users/@me`, keyring в
//! потоке-владельце, типизированные ошибки. Сеть — только в `DiscordValidator`,
//! тесты используют фейки.
//!
//! RW-40/B8a — [`settings_store`]: UI-настройки (тумблеры, плотность, тема,
//! язык) в одном JSON-файле; путь — `DL_SETTINGS_PATH` или `<repo>/.local/`,
//! атомарная запись tmp+rename, типизированные `Io`/`Parse`/`Invalid`.
//!
//! RW-40/G-03a — [`guilds`]: вход в сервер из preview/lurker
//! (`PUT /guilds/{id}/members/@me?lurker=true`, тело `{}`, 204) с типизированными
//! ошибками; fake-транспорт в тестах, реальный — `ApiClient` ядра.
//!
//! RW-40/G-03 — [`invites`]: вход по приглашению — resolve (`GET /invites/{code}`)
//! и accept (`POST /invites/{code}`, ровно три nullable-поля) с типизированными
//! ошибками; `[S]`-расширение каталога маршрутов, fake-транспорт в тестах.
//!
//! Команды Tauri (`#[tauri::command]`) и печать маркеров остаются в `main.rs`.

/// B2a-ATTACH-BACKEND: слоты загрузки вложений `POST /channels/{id}/attachments`.
pub mod attachments;
pub mod auth;
pub mod automod;
pub mod core_bridge;
pub mod data;
pub mod dm_channels;
/// RW-40/PR-14 (task-112): BFF связей, ЛС и Group DM — `GET /users/@me/relationships`,
/// `POST /users/@me/channels` (код `80013`), Pomelo и заметки.
pub mod dm_friends;
pub mod dto;
pub mod expressions;
/// RW-40/task-209: контракт публикаций в форум-канале (`POST /channels/{id}/threads`).
pub mod forum;
pub mod guilds;
pub mod invites;
/// RW-40/task-196: расширенный IPC-диспетчер (`ExtendedIpcCommand`, `ExtendedIpcPlan`).
pub mod ipc_ext;
pub mod messages;
pub mod moderation;
pub mod net;
/// RW-40/N-05: настройки уведомлений гильдий и переопределения каналов (`PATCH /users/@me/guilds/{guild_id}/settings`).
pub mod notifications;
pub mod permissions;
/// RW-40/M-10 & M-11: пины (`GET/PUT/DELETE /channels/{id}/pins`) и ветки канала.
pub mod pins_threads;
pub mod polls;
/// RW-40/BE-P2-10: профиль пользователя (`GET /users/{id}/profile`) с типизированными
/// ошибками, `Retry-After` и разбором ответа.
pub mod profile;
pub mod profile_ext;
pub mod reactions;
pub mod relationships;
/// RW-40/G1-01: единый классификатор REST-ошибок для сетевых модулей.
pub mod rest_failure;
pub mod roles;
pub mod scheduled_events;
pub mod search;
pub mod search_bff;
pub mod settings_store;
/// RW-40/P-03 (D-108): исходящий typing `POST /channels/{id}/typing`.
pub mod typing;
/// RW-40/PR-17 (task-124): устройства ввода/вывода, громкости, режимы
/// (`VoiceActivity`/`PushToTalk`), `self_mute`/`self_deaf` и тест микрофона.
pub mod voice_devices;
/// RW-40/task-294 (`RE-22-02`): диагностика голоса — попап `RTCConnectionPopout`,
/// секции `RTCDebugModal`, окна истории (600 точек / 30 снимков) и
/// `Voice Privacy Code` (30 цифр, 6 групп по 5).
pub mod voice_diagnostics;
pub mod webhooks;

use serde::Serialize;

/// Метрики процесса, которые команда `process_metrics` отдаёт фронтенду.
#[derive(Debug, PartialEq, Serialize)]
pub struct ProcessMetrics {
    /// Resident set size, КиБ (из /proc/self/status).
    pub rss_kib: u64,
    /// Пиковый RSS, КиБ.
    pub hwm_kib: u64,
    /// Число потоков процесса.
    pub threads: u64,
}

/// Читает целое значение поля status-файла по началу строки.
/// `None` — поля нет или значение не число (никакого молчаливого 0).
pub fn read_status_field(status: &str, key: &str) -> Option<u64> {
    let line = status.lines().find(|line| line.starts_with(key))?;
    let value = line.split_whitespace().nth(1)?;
    value.parse().ok()
}

/// Разбор status-текста в метрики. Вынесено из команды, чтобы тестировать
/// маппинг полей без чтения живого `/proc` (R9: RSS — именно VmRSS, не VmHWM).
/// Отсутствие/нечитаемость поля — ошибка, а не молчаливый 0 (иначе метрика врёт).
pub fn parse_process_metrics(status: &str) -> Result<ProcessMetrics, String> {
    let field = |key: &str| {
        read_status_field(status, key)
            .ok_or_else(|| format!("в /proc/self/status нет читаемого поля {key}"))
    };
    Ok(ProcessMetrics {
        rss_kib: field("VmRSS:")?,
        hwm_kib: field("VmHWM:")?,
        threads: field("Threads:")?,
    })
}

/// Минимальные значения доказывают, что спайк действительно отрисовал обещанное:
/// 4 экрана, 100k чата, 24 229 участников, 10 категорий, 38 тумблеров.
/// Rendered-строки ≥10 отсекают регрессию, когда виртуализация рисует только overscan.
pub const SNAPSHOT_FLOORS: [(&str, u64); 7] = [
    ("screenCount", 4),
    ("chatRows", 100_000),
    ("memberRows", 24_229),
    ("emojiCategories", 10),
    ("settingsToggles", 38),
    ("chatRenderedRows", 10),
    ("memberRenderedRows", 10),
];

/// Проверка UI-снимка перед печатью маркера `UI_SNAPSHOT`.
pub fn validate_ui_snapshot(payload: &str) -> Result<serde_json::Value, String> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|err| format!("некорректный UI-снимок: {err}"))?;
    for (key, min) in SNAPSHOT_FLOORS {
        let actual = value
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("UI-снимок не содержит целое неотрицательное поле {key}"))?;
        if actual < min {
            return Err(format!("UI-снимок: {key}={actual} меньше ожидаемого {min}"));
        }
    }
    Ok(value)
}

/// Проверка BENCH-снимка: JSON с объектами chat/members. Иначе перевод строки
/// внутри строки подделал бы маркер `BENCH` в логе.
pub fn validate_bench_payload(payload: &str) -> Result<serde_json::Value, String> {
    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|err| format!("некорректный BENCH-снимок: {err}"))?;
    for key in ["chat", "members"] {
        if !value.get(key).is_some_and(serde_json::Value::is_object) {
            return Err(format!("BENCH-снимок не содержит объект {key}"));
        }
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// RW-40 / INT-01-RUST-WIRING: публичные IPC-обёртки для вложений (B2a),
// создания приглашений канала (STUB-05) и пинов/веток (M-10/M-11).
// ---------------------------------------------------------------------------

/// Канонический реестр IPC-команд INT-01-RUST-WIRING.
pub const IPC_WIRING_COMMANDS: [&str; 6] = [
    "request_upload_slots",
    "create_channel_invite",
    "list_channel_pins",
    "pin_channel_message",
    "unpin_channel_message",
    "list_channel_threads",
];

/// Публичная IPC-обёртка запроса слотов загрузки вложений (`POST /channels/{id}/attachments`).
pub async fn request_upload_slots<S: attachments::AttachmentTransport + ?Sized>(
    sender: &S,
    channel_id: &str,
    files: &[attachments::AttachmentFileSpec],
) -> Result<Vec<attachments::AttachmentUploadSlot>, attachments::AttachmentError> {
    attachments::request_upload_slots(sender, channel_id, files).await
}

/// Публичная IPC-обёртка загрузки байтов вложения по `upload_url` (`PUT {upload_url}`).
pub async fn attachments_upload_put<S: attachments::AttachmentTransport + ?Sized>(
    sender: &S,
    upload_url: &str,
    content_type: &str,
    data: &[u8],
) -> Result<(), attachments::AttachmentError> {
    attachments::upload_attachment_bytes(sender, upload_url, content_type, data).await
}

/// Публичная IPC-обёртка выполнения `attachments_upload_put` из аргументов Tauri-команды [`attachments::UploadPutCommandArgs`].
pub async fn attachments_upload_put_command<S: attachments::AttachmentTransport + ?Sized>(
    sender: &S,
    args: attachments::UploadPutCommandArgs,
) -> Result<(), attachments::AttachmentError> {
    attachments::upload_put_command(sender, args).await
}

/// Публичная IPC-обёртка создания приглашения на канал (`POST /channels/{id}/invites`).
pub async fn create_channel_invite<T: invites::InviteTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    params: &invites::CreateInviteParams,
) -> Result<invites::CreatedInvite, invites::InviteError> {
    invites::create_channel_invite(transport, channel_id, params).await
}

/// Публичная IPC-обёртка загрузки закреплённых сообщений канала (`GET /channels/{id}/pins`).
pub async fn list_channel_pins<T: pins_threads::PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
) -> Result<Vec<pins_threads::PinnedMessage>, pins_threads::PinsThreadsError> {
    pins_threads::list_channel_pins(transport, channel_id).await
}

/// Публичная IPC-обёртка закрепления сообщения в канале (`PUT /channels/{id}/pins/{message_id}`).
pub async fn pin_channel_message<T: pins_threads::PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    message_id: &str,
) -> Result<(), pins_threads::PinsThreadsError> {
    pins_threads::pin_channel_message(transport, channel_id, message_id).await
}

/// Публичная IPC-обёртка открепления сообщения в канале (`DELETE /channels/{id}/pins/{message_id}`).
pub async fn unpin_channel_message<T: pins_threads::PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    message_id: &str,
) -> Result<(), pins_threads::PinsThreadsError> {
    pins_threads::unpin_channel_message(transport, channel_id, message_id).await
}

/// Публичная IPC-обёртка загрузки веток канала (`GET /channels/{id}/threads/active` или `/threads/archived/public`).
pub async fn list_channel_threads<T: pins_threads::PinsThreadsTransport + ?Sized>(
    transport: &T,
    channel_id: &str,
    archived: bool,
) -> Result<Vec<pins_threads::ThreadSummary>, pins_threads::PinsThreadsError> {
    pins_threads::list_channel_threads(transport, channel_id, archived).await
}

/// Публичная обёртка каскадного резолвера уведомлений (`N-05`).
#[must_use]
pub fn resolve_effective_notifications(
    guild: &notifications::GuildNotificationPolicyInput,
    parent_category: Option<&notifications::ChannelNotificationOverride>,
    channel: Option<&notifications::ChannelNotificationOverride>,
    now_unix: i64,
) -> notifications::EffectiveNotificationPolicy {
    notifications::resolve_effective_notifications(guild, parent_category, channel, now_unix)
}

/// Публичная IPC-обёртка обновления настроек уведомлений гильдии (`PATCH /users/@me/guilds/{guild_id}/settings`).
pub async fn update_guild_notification_settings<
    T: notifications::NotificationTransport + ?Sized,
>(
    transport: &T,
    token: &str,
    guild_id: &str,
    patch: &notifications::GuildNotificationSettingsPatch,
) -> Result<serde_json::Value, notifications::NotificationError> {
    notifications::update_guild_notification_settings(transport, token, guild_id, patch).await
}

/// Публичная IPC-обёртка обновления переопределения уведомлений канала (`PATCH /users/@me/guilds/{guild_id}/settings`).
pub async fn update_channel_notification_override<
    T: notifications::NotificationTransport + ?Sized,
>(
    transport: &T,
    token: &str,
    guild_id: &str,
    channel_id: &str,
    override_patch: &notifications::ChannelNotificationOverridePatch,
) -> Result<serde_json::Value, notifications::NotificationError> {
    notifications::update_channel_notification_override(
        transport,
        token,
        guild_id,
        channel_id,
        override_patch,
    )
    .await
}
pub mod bff_domains_router;
pub mod onboarding;
pub mod slash_commands;
pub mod soundboard;
pub mod stage_instances;
pub mod voice_stage;
