//! P0-01: спайк UI-оболочки Tauri.
//!
//! Минимальное окно + IPC-команды для честного замера памяти процесса и
//! проверяемого UI-снимка. Сети нет; BFF фазы 3 читает только локальные
//! `fixtures/` (`DL_FIXTURES`, по умолчанию — `fixtures/` репозитория).
//! Чистая логика вынесена в `discord_gui_spike::lib`/`discord_gui_spike::data`
//! и покрыта `tests/ipc_contract.rs` + `tests/data_contract.rs`, живой запуск —
//! `tests/launch.rs`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

use discord_gui_spike::attachments::{
    AttachmentError, AttachmentFileSpec, AttachmentTransport, AttachmentUploadSlot,
    DiscordAttachments, UploadBytes, UploadPutCommandArgs, UploadPutInput,
};
use discord_gui_spike::auth::{self, AuthBackend, AuthError, AuthState, LoginView, SmsChallenge};
use discord_gui_spike::data::{
    self, ChannelRow, DataError, MembersPage, MessagesPage, MessagesPageRequest, MessagesPosition,
    ReadStateEntry, ReadyPayload, SearchFilters, SearchPage, SettingsPayload,
};
use discord_gui_spike::guilds::{
    self, CreateGuildError, CreateGuildOutcome, CreateGuildPayload, DiscordGuilds, EnterGuildError,
    LeaveGuildError,
};
use discord_gui_spike::invites::{
    self, AcceptedInvite, CreateInviteParams, CreatedInvite, DiscordInvites, InviteError,
    InvitePreview,
};
use discord_gui_spike::messages::{self, DiscordMessageSender, DiscordReadState, MessageError};
use discord_gui_spike::net::{
    DispatchEmitter, DispatchPayload, DispatchStats, NetService, DISPATCH_EVENT,
};
use discord_gui_spike::notifications::{
    ChannelNotificationOverridePatch, DiscordNotifications, GuildNotificationSettingsPatch,
    NotificationError,
};
use discord_gui_spike::pins_threads::{
    DiscordPinsThreads, PinnedMessage, PinsThreadsError, ThreadSummary,
};
use discord_gui_spike::reactions::{
    toggle_reaction as send_reaction, DiscordReactionApi, ReactionError,
};
use discord_gui_spike::search::{
    DiscordSearch, RemoteSearch, SearchError, SearchParams, SearchResponse,
};
use discord_gui_spike::settings_store::{SettingsError, SettingsStore, UiSettings};
use discord_gui_spike::typing::{self, DiscordTyping, TypingCooldowns, TypingError};
use discord_gui_spike::{
    parse_process_metrics, validate_bench_payload, validate_ui_snapshot, ProcessMetrics,
};
use std::sync::Arc;
use tauri::{Emitter, Manager};

struct TauriAuthEmitter(tauri::AppHandle);

impl auth::AuthStateEmitter for TauriAuthEmitter {
    fn emit_state(&self, state: &AuthState) -> Result<(), String> {
        self.0
            .emit("auth://state", state)
            .map_err(|error| format!("не удалось отправить auth://state: {error}"))
    }
}

fn emit_auth_state(app: &tauri::AppHandle, state: &AuthState) -> Result<(), AuthError> {
    auth::emit_auth_state(&TauriAuthEmitter(app.clone()), state)
        .map_err(|message| AuthError::Internal { message })
}

// ---------------------------------------------------------------------------
// RW-40/L-02: dispatch → UI. Эмиттер вызывается на потоке `gateway-dispatch`
// (очередь `DispatchChannel`), поэтому окно/UI не блокируют шлюз. В событие
// уходит только конверт `{kind, data}`: токены и session-возможности READY
// вырезаны в `net::map_dispatch`. Ошибка эмита возвращается наверх и попадает
// в счётчики `net_dispatch_stats`, а не в лог (payload не печатается).
// ---------------------------------------------------------------------------

struct TauriDispatchEmitter(tauri::AppHandle);

impl DispatchEmitter for TauriDispatchEmitter {
    fn emit(&self, payload: &DispatchPayload) -> Result<(), String> {
        self.0
            .emit(DISPATCH_EVENT, payload)
            .map_err(|error| format!("не удалось отправить {DISPATCH_EVENT}: {error}"))
    }
}

#[tauri::command]
async fn process_metrics() -> Result<ProcessMetrics, String> {
    blocking(|| {
        let status = std::fs::read_to_string("/proc/self/status")
            .map_err(|err| format!("не удалось прочитать /proc/self/status: {err}"))?;
        parse_process_metrics(&status)
    })
    .await?
}

#[tauri::command]
fn spike_config() -> serde_json::Value {
    serde_json::json!({
        "autobench": std::env::var("SPIKE_AUTOBENCH").ok().as_deref() == Some("1"),
    })
}

/// Печатает результат автозамера в stdout — харнесс читает его из app.log.
/// Payload валидируется как JSON с объектами chat/members: иначе перевод строки
/// внутри строки подделал бы маркер `BENCH`/`UI_SNAPSHOT` в логе.
#[tauri::command]
fn report_bench(payload: String) -> Result<(), String> {
    let value = validate_bench_payload(&payload)?;
    println!("BENCH {value}");
    Ok(())
}

#[tauri::command]
fn report_ui_snapshot(payload: String) -> Result<(), String> {
    match validate_ui_snapshot(&payload) {
        Ok(value) => {
            println!("UI_SNAPSHOT {value}");
            Ok(())
        }
        Err(err) => {
            eprintln!("[REPORT_UI_SNAPSHOT_ERROR] {err}\npayload: {payload}");
            Err(err)
        }
    }
}

// ---------------------------------------------------------------------------
// BFF фазы 3 (W39e): только локальные `fixtures/`, без сети.
// Корень — `DL_FIXTURES` или `fixtures/` репозитория; ошибки типизированы
// `DataError` и уходят во фронтенд как `{kind, message, path?/resource?}`.
// ---------------------------------------------------------------------------

/// READY: пользователь, гильдии и счётчики первого экрана.
#[tauri::command]
async fn ready() -> Result<ReadyPayload, DataError> {
    blocking(|| {
        let root = data::fixtures_root()?;
        data::ready(&root)
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Записи `READY.read_state`: `lastViewed` (курсор ack), `flags`,
/// `mentionCount`. Локальная READY-проекция: токен не читается и не логируется.
#[tauri::command]
async fn read_state() -> Result<Vec<ReadStateEntry>, DataError> {
    blocking(|| {
        let root = data::fixtures_root()?;
        data::read_state(&root)
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Дерево каналов из READY: `guildId` — необязательный фильтр, `None` — все
/// каналы и ЛС. Локальная проекция: токен не читается и не логируется.
#[tauri::command]
async fn channels(guild_id: Option<String>) -> Result<Vec<ChannelRow>, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        data::channels(&root, guild_id.as_deref())
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Страница сообщений канала. `cursor` — forward (id последней строки
/// предыдущей страницы), `before` — older (id первой строки окна: вернуть
/// строки старше), `position` — `"start"` (по умолчанию) или `"latest"`.
/// Курсоры взаимоисключающие; семантика — в `data::messages_page_request`.
#[tauri::command]
async fn messages_page(
    channel_id: String,
    cursor: Option<String>,
    before: Option<String>,
    position: Option<String>,
    limit: Option<usize>,
) -> Result<MessagesPage, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        let position = MessagesPosition::parse(position.as_deref())?;
        data::messages_page_request(
            &root,
            &channel_id,
            MessagesPageRequest {
                cursor: cursor.as_deref(),
                before: before.as_deref(),
                position,
                limit,
            },
        )
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Страница участников гильдии; онлайн-участники идут первыми.
#[tauri::command]
async fn members_page(
    guild_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<MembersPage, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        data::members_page(&root, &guild_id, cursor.as_deref(), limit)
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Настройки из READY: все секции или одна по `section`.
#[tauri::command]
async fn settings(section: Option<String>) -> Result<SettingsPayload, DataError> {
    blocking(move || {
        let root = data::fixtures_root()?;
        data::settings(&root, section.as_deref())
    })
    .await
    .map_err(DataError::invalid_arg)?
}

/// Фильтры поиска в том виде, в каком их присылает фронтенд (camelCase).
///
/// Канонический `data::SearchFilters` десериализуется сам (`#[derive(Deserialize)]`,
/// `rename_all = "camelCase"`, отсутствующие ключи — `None`), поэтому зеркала в
/// слое команд нет: обрезка строк, проверка длин/даты и семантика фильтров
/// остаются в `data.rs`.
///
/// Страница поиска по фикстурам: 7 фильтров R-10, `cursor` — id последнего
/// результата предыдущей страницы.
#[tauri::command]
fn search_page(
    query: String,
    filters: Option<SearchFilters>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<SearchPage, DataError> {
    let root = data::fixtures_root()?;
    let filters = filters.unwrap_or_default();
    data::search_page(&root, &query, &filters, cursor.as_deref(), limit)
}

/// Единственная точка чтения токена для сетевых команд (G1-02).
///
/// `spawn_blocking` и оба отказа (незавершившийся blocking-вызов и ошибка
/// keyring) сведены сюда, поэтому новая команда не копирует этот блок.
/// Коды не унифицируются — их задаёт домен: `storage` (SEC-02) для keyring/
/// blocking (отказ хранилища не выдаётся за сеть), `missing` для «токена нет»
/// (`Unauthorized` у сообщений/реакций, `MissingToken` у поиска/guild-команд).
/// Причина keyring в доменную ошибку не копируется: наружу уходит только
/// типизированный `kind`, поэтому токен и текст ошибки хранилища не логируются
/// и не попадают в `Debug`/`Display`.
async fn token_or<E, F>(load: F, storage: fn() -> E, missing: fn() -> E) -> Result<String, E>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
    E: Send + 'static,
{
    blocking(load)
        .await
        .map_err(|_| storage())?
        .map_err(|_| storage())?
        .ok_or_else(missing)
}

/// Серверный поиск сообщений гильдии; токен читается исключительно из backend/keyring.
#[tauri::command]
async fn search_remote(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    params: Option<SearchParams>,
) -> Result<SearchResponse, SearchError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<SearchError, _>(
        move || backend.token(),
        || SearchError::Storage,
        || SearchError::MissingToken,
    )
    .await?;
    let search = DiscordSearch::new(&token)?;
    search.search(&guild_id, &params.unwrap_or_default()).await
}

/// Отправка сообщения: токен читается исключительно из backend/keyring.
#[tauri::command]
async fn send_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    content: String,
    nonce: String,
    attachments: Option<Vec<discord_gui_spike::attachments::UploadedAttachment>>,
) -> Result<discord_gui_spike::messages::MessageReceipt, MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let sender = DiscordMessageSender::new(&token)?;
    if let Some(att) = attachments.filter(|a| !a.is_empty()) {
        sender
            .send_with_attachments(&channel_id, &content, &nonce, &att)
            .await
    } else {
        discord_gui_spike::messages::send_message(&sender, &channel_id, &content, &nonce).await
    }
}

#[tauri::command]
async fn edit_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    content: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    messages::edit_message(&api, &channel_id, &message_id, &content).await
}

#[tauri::command]
async fn delete_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    messages::delete_message(&api, &channel_id, &message_id).await
}

#[tauri::command]
async fn reply_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    content: String,
    reply_to: String,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordMessageSender::new(&token)?;
    messages::reply_message(&api, &channel_id, &content, &reply_to).await
}

// ---------------------------------------------------------------------------
// RW-40/M-07: read state / ack. Bearer — только из backend/keyring; `token`
// в аргументах команды — nullable read-state токен тела (`{token,last_viewed}`),
// не auth. Ничего не логируется.
// ---------------------------------------------------------------------------

#[tauri::command]
async fn ack_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    last_viewed: u64,
    token: Option<String>,
) -> Result<(), MessageError> {
    let backend = Arc::clone(backend.inner());
    let bearer = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let api = DiscordReadState::new(&bearer)?;
    messages::ack_message(&api, &channel_id, &message_id, last_viewed, token).await
}

// ---------------------------------------------------------------------------
// RW-40/G-03a: вход в сервер из preview/lurker. Токен — только из backend/keyring
// (никогда из аргументов фронтенда) и не попадает в логи/ошибки.
// Q-01: одна операция — одна IPC-команда `join_guild` (UI-глагол). Второго
// Tauri-command'а нет; `guilds::enter_guild` — модульный alias той же операции.
// ---------------------------------------------------------------------------

/// Войти в сервер из preview: `PUT /guilds/{id}/members/@me?lurker=true`, тело `{}`.
#[tauri::command]
async fn join_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
) -> Result<(), EnterGuildError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<EnterGuildError, _>(
        move || backend.token(),
        || EnterGuildError::Storage,
        || EnterGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    guilds::join_guild(&api, &guild_id).await
}

/// Создать новый сервер: `POST /guilds` (STUB-03).
#[tauri::command]
async fn create_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    name: String,
    icon: Option<String>,
    template: Option<String>,
) -> Result<CreateGuildOutcome, CreateGuildError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<CreateGuildError, _>(
        move || backend.token(),
        || CreateGuildError::Storage,
        || CreateGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    let payload = CreateGuildPayload {
        name,
        icon,
        channels: None,
        guild_template_code: template,
    };
    guilds::create_guild(&api, payload).await
}

/// Выйти с сервера: `DELETE /users/@me/guilds/{id}` с телом `{lurking}`.
///
/// `lurking` — явный bool от caller'а (UI: `isLurking || isGuest`); команда его не
/// вычисляет. Ответ не читается: успех — 2xx.
#[tauri::command]
async fn leave_guild(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    lurking: bool,
) -> Result<(), LeaveGuildError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<LeaveGuildError, _>(
        move || backend.token(),
        || LeaveGuildError::Storage,
        || LeaveGuildError::MissingToken,
    )
    .await?;
    let api = DiscordGuilds::new(&token)?;
    guilds::leave_guild(&api, &guild_id, lurking).await
}

/// Поставить или снять реакцию; токен читается исключительно из backend/keyring.
#[tauri::command]
async fn toggle_reaction(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
    emoji: String,
    active: bool,
) -> Result<(), ReactionError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<ReactionError, _>(
        move || backend.token(),
        || ReactionError::Storage,
        || ReactionError::Unauthorized,
    )
    .await?;
    let api = DiscordReactionApi::new(&token)?;
    send_reaction(&api, &channel_id, &message_id, &emoji, active).await
}

// ---------------------------------------------------------------------------
// RW-40/P-03 (D-108): исходящий typing. POST без тела, успех 200; ответ —
// опциональные кулдауны slowmode, пустое тело допустимо. TTL/троттлинг делает
// UI, не backend. Bearer — только из backend/keyring, ничего не логируется.
// ---------------------------------------------------------------------------

/// Сообщить серверу, что пользователь печатает; `None` — кулдаунов в ответе нет.
#[tauri::command]
async fn send_typing(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
) -> Result<Option<TypingCooldowns>, TypingError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<TypingError, _>(
        move || backend.token(),
        || TypingError::Storage,
        || TypingError::Unauthorized,
    )
    .await?;
    let api = DiscordTyping::new(&token)?;
    typing::send_typing(&api, &channel_id).await
}

// ---------------------------------------------------------------------------
// RW-40/G-03: вход по приглашению. Bearer — только из backend/keyring (никогда из
// аргументов фронтенда) и не попадает в логи/ошибки. resolve — GET с
// with_counts/with_expiration; accept — POST с тремя nullable-полями тела.
// ---------------------------------------------------------------------------

/// Превью приглашения: `GET /invites/{code}?with_counts=true&with_expiration=true`.
#[tauri::command]
async fn resolve_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
) -> Result<InvitePreview, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
    invites::resolve_invite(&api, &code).await
}

/// Вступить по приглашению: `POST /invites/{code}`.
///
/// Все три поля тела nullable и передаются как пришли (без подстановок):
/// `session_id`, `invite_instance_id`, `received_installation_id`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn accept_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    session_id: Option<String>,
    invite_instance_id: Option<String>,
    received_installation_id: Option<String>,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
) -> Result<AcceptedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
    let effective_session_id = session_id
        .filter(|s| !s.trim().is_empty())
        .or_else(data::current_gateway_session_id);
    if let Some(key) = captcha_key.filter(|s| !s.trim().is_empty()) {
        let solution = invites::CaptchaSolution {
            captcha_key: key,
            captcha_rqtoken: captcha_rqtoken.filter(|s| !s.trim().is_empty()),
            captcha_session_id: captcha_session_id.filter(|s| !s.trim().is_empty()),
        };
        invites::accept_invite_with_captcha(
            &api,
            &code,
            effective_session_id.as_deref(),
            invite_instance_id.as_deref(),
            received_installation_id.as_deref(),
            &solution,
        )
        .await
    } else {
        invites::accept_invite(
            &api,
            &code,
            effective_session_id.as_deref(),
            invite_instance_id.as_deref(),
            received_installation_id.as_deref(),
        )
        .await
    }
}

/// Создать приглашение для канала: `POST /channels/{channel_id}/invites`.
///
/// Пустой вызов (`params = None`) использует канонический дефолт Discord
/// (`max_age = 86400`, `max_uses = 0`, `temporary = false`).
#[tauri::command]
async fn create_channel_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    params: Option<CreateInviteParams>,
) -> Result<CreatedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
    let params = params.unwrap_or_default();
    discord_gui_spike::create_channel_invite(&api, &channel_id, &params).await
}

// ---------------------------------------------------------------------------
// RW-40/M-07: слоты загрузки вложений (`POST /channels/{channel_id}/attachments`).
// Bearer читается только из backend/keyring через `token_or`.
// ---------------------------------------------------------------------------

/// Запросить pre-signed GCS слоты загрузки вложений для канала.
#[tauri::command]
async fn request_upload_slots(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    files: Vec<AttachmentFileSpec>,
) -> Result<Vec<AttachmentUploadSlot>, AttachmentError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<AttachmentError, _>(
        move || backend.token(),
        || AttachmentError::Network,
        || AttachmentError::Unauthorized,
    )
    .await?;
    let api = DiscordAttachments::new(&token)?;
    discord_gui_spike::request_upload_slots(&api, &channel_id, &files).await
}

/// Загрузить байты вложения методом `PUT` на pre-signed `upload_url` из слота.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn attachments_upload_put(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    payload: Option<UploadPutInput>,
    slot_id: Option<String>,
    id: Option<String>,
    channel_id: Option<String>,
    upload_url: Option<String>,
    content_type: Option<String>,
    data: Option<UploadBytes>,
    bytes: Option<UploadBytes>,
) -> Result<(), AttachmentError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<AttachmentError, _>(
        move || backend.token(),
        || AttachmentError::Network,
        || AttachmentError::Unauthorized,
    )
    .await?;
    let api = DiscordAttachments::new(&token)?;
    let args = UploadPutCommandArgs {
        payload,
        slot_id: slot_id.or(id.clone()),
        id,
        channel_id,
        upload_url,
        content_type,
        data,
        bytes,
    };
    execute_attachments_upload_put(&api, args).await
}

async fn execute_attachments_upload_put<S: AttachmentTransport + ?Sized>(
    api: &S,
    args: UploadPutCommandArgs,
) -> Result<(), AttachmentError> {
    let input = UploadPutInput::from_ipc(
        args.payload,
        args.slot_id.or(args.id),
        args.channel_id,
        args.upload_url,
        args.content_type,
        args.data,
        args.bytes,
    )?;
    discord_gui_spike::attachments_upload_put(
        api,
        &input.upload_url,
        &input.content_type,
        input.payload_bytes(),
    )
    .await
}

// ---------------------------------------------------------------------------
// RW-40/M-08: закреплённые сообщения и треды канала. Bearer — только из
// backend/keyring через `token_or`, без передачи токена из фронтенда.
// ---------------------------------------------------------------------------

/// Список закреплённых сообщений канала: `GET /channels/{channel_id}/pins`.
#[tauri::command]
async fn list_channel_pins(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    discord_gui_spike::list_channel_pins(&api, &channel_id).await
}

/// Закрепить сообщение в канале: `PUT /channels/{channel_id}/pins/{message_id}`.
#[tauri::command]
async fn pin_channel_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
) -> Result<(), PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    discord_gui_spike::pin_channel_message(&api, &channel_id, &message_id).await
}

/// Открепить сообщение в канале: `DELETE /channels/{channel_id}/pins/{message_id}`.
#[tauri::command]
async fn unpin_channel_message(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    message_id: String,
) -> Result<(), PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    discord_gui_spike::unpin_channel_message(&api, &channel_id, &message_id).await
}

/// Список активных или публичных архивных тредов канала (`archived = false | true`).
#[tauri::command]
async fn list_channel_threads(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    archived: bool,
) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    discord_gui_spike::list_channel_threads(&api, &channel_id, archived).await
}

// ---------------------------------------------------------------------------
// RW-40 / INT-05-IPC-ALIASES: выравнивание имён IPC-команд UI ↔ Rust (F-1).
// Каждая команда делегирует в ту же доменную обёртку и читает токен только
// через `token_or`, сохраняя коды ошибок и форму ответов, ожидаемую в UI.
// ---------------------------------------------------------------------------

/// Опциональный объект `params` для `create_invite`: принимает как camelCase (`maxAge`, `maxUses`),
/// так и snake_case (`max_age`, `max_uses`).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInviteParamsInput {
    #[serde(default, alias = "max_age")]
    max_age: Option<u32>,
    #[serde(default, alias = "max_uses")]
    max_uses: Option<u32>,
    #[serde(default)]
    temporary: Option<bool>,
    #[serde(default)]
    flags: Option<u32>,
}

impl CreateInviteParamsInput {
    fn resolve(
        params: Option<Self>,
        max_age: Option<u32>,
        max_uses: Option<u32>,
        temporary: Option<bool>,
        flags: Option<u32>,
    ) -> CreateInviteParams {
        let defaults = CreateInviteParams::default();
        let nested = params.unwrap_or_default();
        CreateInviteParams {
            max_age: max_age.or(nested.max_age).unwrap_or(defaults.max_age),
            max_uses: max_uses.or(nested.max_uses).unwrap_or(defaults.max_uses),
            temporary: temporary.or(nested.temporary).unwrap_or(defaults.temporary),
            flags: flags.or(nested.flags),
        }
    }
}

/// IPC-алиас `create_channel_invite` под имя вызова `invite-modal.js:370` (`create_invite`).
/// Принимает как плоский payload `{channelId, maxAge, maxUses, temporary, flags}`, так и
/// вложенный `{channelId, params}`.
#[tauri::command]
async fn create_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    params: Option<CreateInviteParamsInput>,
    max_age: Option<u32>,
    max_uses: Option<u32>,
    temporary: Option<bool>,
    flags: Option<u32>,
) -> Result<CreatedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
    let resolved = CreateInviteParamsInput::resolve(params, max_age, max_uses, temporary, flags);
    discord_gui_spike::create_channel_invite(&api, &channel_id, &resolved).await
}

/// Элемент `files[]` для IPC-алиаса `attachments_upload_slots` (`composer.js:725`):
/// принимает как `file_size`/`is_clip` (snake_case из `composer.js`), так и `fileSize`/`isClip`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct UploadSlotSpecInput {
    filename: String,
    #[serde(alias = "fileSize")]
    file_size: u64,
    id: String,
    #[serde(default, alias = "isClip")]
    is_clip: bool,
}

impl From<UploadSlotSpecInput> for AttachmentFileSpec {
    fn from(input: UploadSlotSpecInput) -> Self {
        Self {
            filename: input.filename,
            file_size: input.file_size,
            id: input.id,
            is_clip: input.is_clip,
        }
    }
}

/// Ответ `attachments_upload_slots` в формате, который читает `composer.js:737` (`res.attachments`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct UploadSlotsEnvelope {
    attachments: Vec<AttachmentUploadSlot>,
}

/// IPC-алиас `request_upload_slots` под имя вызова `composer.js:725` (`attachments_upload_slots`).
/// Возвращает конверт `{ attachments }`, который читает `composer.js:737`.
#[tauri::command]
async fn attachments_upload_slots(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    files: Vec<UploadSlotSpecInput>,
) -> Result<UploadSlotsEnvelope, AttachmentError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<AttachmentError, _>(
        move || backend.token(),
        || AttachmentError::Network,
        || AttachmentError::Unauthorized,
    )
    .await?;
    let api = DiscordAttachments::new(&token)?;
    let specs: Vec<AttachmentFileSpec> = files.into_iter().map(Into::into).collect();
    let attachments = discord_gui_spike::request_upload_slots(&api, &channel_id, &specs).await?;
    Ok(UploadSlotsEnvelope { attachments })
}

/// IPC-алиас `list_channel_pins` под имя вызова `app.js:1372` (`pins_list`).
#[tauri::command]
async fn pins_list(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
) -> Result<Vec<PinnedMessage>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    discord_gui_spike::list_channel_pins(&api, &channel_id).await
}

/// IPC-алиас `list_channel_threads` под имя вызова `app.js:1378` (`threads_list`).
/// При отсутствии `archived` в аргументах возвращает активные ветки (`archived = false`).
#[tauri::command]
async fn threads_list(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    channel_id: String,
    archived: Option<bool>,
) -> Result<Vec<ThreadSummary>, PinsThreadsError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<PinsThreadsError, _>(
        move || backend.token(),
        || PinsThreadsError::Storage,
        || PinsThreadsError::MissingToken,
    )
    .await?;
    let api = DiscordPinsThreads::new(&token)?;
    discord_gui_spike::list_channel_threads(&api, &channel_id, archived.unwrap_or(false)).await
}

// ---------------------------------------------------------------------------
// RW-40 / INT-06-NOTIFICATIONS-IPC-WIRING (F-5): Tauri IPC-команды уведомлений
// `update_guild_notification_settings` и `update_channel_notification_override`
// (`notifications.js:1335,1389`). Bearer читается только из backend/keyring
// через `token_or`, без передачи токена из фронтенда.
// ---------------------------------------------------------------------------

/// Входное переопределение канала для `update_channel_notification_override`:
/// принимает как прямое `ChannelNotificationOverridePatch` (`override` / `override_field` /
/// `override_patch` / `payload`), объект `{ override: ... }`, так и конверт
/// `{ channel_overrides: { "<channel_id>": ... } }` из `notifications.js:1389`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(untagged)]
enum ChannelOverrideInput {
    Envelope {
        channel_overrides: std::collections::BTreeMap<String, ChannelNotificationOverridePatch>,
    },
    OverrideField {
        #[serde(rename = "override", alias = "override_field", alias = "overrideField")]
        override_field: ChannelNotificationOverridePatch,
    },
    Direct(ChannelNotificationOverridePatch),
}

impl ChannelOverrideInput {
    fn into_patch(
        self,
        channel_id: &str,
    ) -> Result<ChannelNotificationOverridePatch, NotificationError> {
        match self {
            Self::Envelope {
                mut channel_overrides,
            } => {
                if let Some(patch) = channel_overrides.remove(channel_id) {
                    Ok(patch)
                } else if channel_overrides.is_empty() {
                    Ok(ChannelNotificationOverridePatch::default())
                } else {
                    Err(NotificationError::InvalidChannelId)
                }
            }
            Self::OverrideField { override_field } => Ok(override_field),
            Self::Direct(patch) => Ok(patch),
        }
    }

    fn resolve(
        channel_id: &str,
        r#override: Option<ChannelNotificationOverridePatch>,
        override_field: Option<ChannelNotificationOverridePatch>,
        override_patch: Option<ChannelNotificationOverridePatch>,
        patch: Option<Self>,
        payload: Option<Self>,
    ) -> Result<ChannelNotificationOverridePatch, NotificationError> {
        let from_patch = match patch {
            Some(input) => Some(input.into_patch(channel_id)?),
            None => None,
        };
        let from_payload = match payload {
            Some(input) => Some(input.into_patch(channel_id)?),
            None => None,
        };
        Ok(r#override
            .or(override_field)
            .or(override_patch)
            .or(from_patch)
            .or(from_payload)
            .unwrap_or_default())
    }
}

/// Обновить настройки уведомлений гильдии / DM: `PATCH /users/@me/guilds/{guild_id}/settings`.
#[tauri::command]
async fn update_guild_notification_settings(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    patch: Option<GuildNotificationSettingsPatch>,
    settings: Option<GuildNotificationSettingsPatch>,
    payload: Option<GuildNotificationSettingsPatch>,
) -> Result<serde_json::Value, NotificationError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<NotificationError, _>(
        move || backend.token(),
        || NotificationError::Storage,
        || NotificationError::MissingToken,
    )
    .await?;
    let api = DiscordNotifications::new(&token)?;
    let resolved = patch.or(settings).or(payload).unwrap_or_default();
    discord_gui_spike::update_guild_notification_settings(&api, &token, &guild_id, &resolved).await
}

/// Обновить переопределение уведомлений канала: `PATCH /users/@me/guilds/{guild_id}/settings`
/// с картой `{"channel_overrides": {"<channel_id>": { ... }}}`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn update_channel_notification_override(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    guild_id: String,
    channel_id: String,
    r#override: Option<ChannelNotificationOverridePatch>,
    override_field: Option<ChannelNotificationOverridePatch>,
    override_patch: Option<ChannelNotificationOverridePatch>,
    patch: Option<ChannelOverrideInput>,
    payload: Option<ChannelOverrideInput>,
) -> Result<serde_json::Value, NotificationError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<NotificationError, _>(
        move || backend.token(),
        || NotificationError::Storage,
        || NotificationError::MissingToken,
    )
    .await?;
    let api = DiscordNotifications::new(&token)?;
    let resolved = ChannelOverrideInput::resolve(
        &channel_id,
        r#override,
        override_field,
        override_patch,
        patch,
        payload,
    )?;
    discord_gui_spike::update_channel_notification_override(
        &api,
        &token,
        &guild_id,
        &channel_id,
        &resolved,
    )
    .await
}

// ---------------------------------------------------------------------------
// RW-39/NET-C2: авторизация. Вся логика — в `discord_gui_spike::auth`, здесь
// только Tauri-обёртки. `Keyring` — `!Send` и может ждать D-Bus, поэтому каждая
// команда уходит в `spawn_blocking`: UI-поток и async-runtime не блокируются.
// ---------------------------------------------------------------------------

/// Блокирующая операция — вне async-runtime (keyring, файл настроек).
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("блокирующая операция не завершилась: {error}"))
}

/// Статус: только keyring, без сети. Сессию восстанавливает `auth_restore`.
#[tauri::command]
async fn auth_status(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    let state = blocking(move || backend.status())
        .await
        .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

/// Импорт токена владельца: `GET /users/@me` → keyring. Токен не логируется
/// и не возвращается во фронтенд (только `AuthState`).
#[tauri::command]
async fn auth_login_token(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    token: String,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    emit_auth_state(&app, &AuthState::validating())?;
    let state = blocking(move || {
        let st = backend.login(&token);
        let tok = backend.token().ok().flatten();
        data::set_live_session(tok, st.user.as_ref());
        st
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

/// Явное восстановление из keyring — единственный путь к сети при старте.
#[tauri::command]
async fn auth_restore(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    let state = blocking(move || {
        let st = backend.restore();
        let tok = backend.token().ok().flatten();
        data::set_live_session(tok, st.user.as_ref());
        st
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

/// Выход: keyring.clear + сброс сессии; повторный вызов идемпотентен.
#[tauri::command]
async fn auth_logout(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    net: tauri::State<'_, Arc<NetService>>,
) -> Result<AuthState, AuthError> {
    let backend = Arc::clone(backend.inner());
    let net = Arc::clone(net.inner());
    let state = blocking(move || {
        let st = backend.logout_with_disconnect(|| {
            net.disconnect()
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        data::set_live_session(None, None);
        st
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state)?;
    Ok(state)
}

// ---------------------------------------------------------------------------
// RW-39/NET-F1: вход по логину/паролю (ADR-3). Пароль и 2FA-код приходят как
// `String` (Tauri), живут только в памяти запроса и не возвращаются. Ошибки —
// типизированные, без секретов; captcha/MFA/Ready — `LoginView` без токена.
// ---------------------------------------------------------------------------

/// Вход паролем: `POST /auth/login`; исход — MFA / captcha / Ready (токен в keyring).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn auth_login_password(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    login: String,
    password: String,
    captcha_key: Option<String>,
    captcha_rqtoken: Option<String>,
    captcha_session_id: Option<String>,
    captcha_rqdata: Option<String>,
) -> Result<LoginView, AuthError> {
    emit_auth_state(&app, &AuthState::validating())?;
    let backend = Arc::clone(backend.inner());
    let state_backend = Arc::clone(&backend);
    let view = blocking(move || {
        let outcome = backend.login_password(
            &login,
            password,
            captcha_key.as_deref(),
            captcha_rqtoken.as_deref(),
            captcha_session_id.as_deref(),
            captcha_rqdata.as_deref(),
        );
        let st = backend.status();
        if st.user.is_some() {
            let tok = backend.token().ok().flatten();
            data::set_live_session(tok, st.user.as_ref());
        }
        LoginView::from_outcome(outcome, &st)
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state_backend.status())?;
    view
}

/// Код 2FA: `POST /auth/mfa/{method}` (method — whitelist в `auth.rs`).
#[tauri::command]
async fn auth_mfa(
    app: tauri::AppHandle,
    backend: tauri::State<'_, Arc<AuthBackend>>,
    method: String,
    code: String,
    ticket: String,
    login_instance_id: Option<String>,
) -> Result<LoginView, AuthError> {
    emit_auth_state(&app, &AuthState::validating())?;
    let backend = Arc::clone(backend.inner());
    let state_backend = Arc::clone(&backend);
    let view = blocking(move || {
        let outcome = backend.submit_mfa(&method, code, &ticket, login_instance_id.as_deref());
        let st = backend.status();
        if st.user.is_some() {
            let tok = backend.token().ok().flatten();
            data::set_live_session(tok, st.user.as_ref());
        }
        LoginView::from_outcome(outcome, &st)
    })
    .await
    .map_err(|message| AuthError::Internal { message })?;
    emit_auth_state(&app, &state_backend.status())?;
    view
}

/// Переотправка SMS-кода: `POST /auth/mfa/sms/send {ticket}` → маскированный `phone`.
#[tauri::command]
async fn auth_sms(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    ticket: String,
) -> Result<SmsChallenge, AuthError> {
    let backend = Arc::clone(backend.inner());
    let challenge = blocking(move || backend.request_sms(&ticket))
        .await
        .map_err(|message| AuthError::Internal { message })?;
    challenge
}

#[tauri::command]
async fn net_connect(net: tauri::State<'_, Arc<NetService>>) -> Result<String, String> {
    let net = Arc::clone(net.inner());
    blocking(move || {
        net.connect()
            .map(|status| status.to_string())
            .map_err(|error| error.to_string())
    })
    .await?
}

#[tauri::command]
fn net_status(net: tauri::State<'_, Arc<NetService>>) -> String {
    net.status().to_string()
}

/// Счётчики L-02: что пришло, что ушло в UI, что отброшено и почему.
///
/// `sourceAvailable` = true: источник — супервайзер ядра (hook `RunOptions.on_dispatch`);
/// причина/механизм видны в `sourceDetail`, чтобы UI не ждал пустой канал молча.
#[tauri::command]
fn net_dispatch_stats(net: tauri::State<'_, Arc<NetService>>) -> DispatchStats {
    net.dispatch_stats()
}

#[tauri::command]
async fn net_disconnect(net: tauri::State<'_, Arc<NetService>>) -> Result<String, String> {
    let net = Arc::clone(net.inner());
    blocking(move || {
        net.disconnect()
            .map(|status| status.to_string())
            .map_err(|error| error.to_string())
    })
    .await?
}

// ---------------------------------------------------------------------------
// RW-40/B8a: UI-настройки на диск. Только локальный файл — ни сети, ни
// токенов. Файл маленький, но I/O уводим в `spawn_blocking`: UI-поток не ждёт
// диск, как и у команд авторизации.
// ---------------------------------------------------------------------------

/// Настройки с диска; нет файла → дефолты отгруженного UI (cozy/dark/ru).
#[tauri::command]
async fn settings_load() -> Result<UiSettings, SettingsError> {
    blocking(|| SettingsStore::from_env().load())
        .await
        .map_err(|message| SettingsError::invalid("internal", message))?
}

/// Сохранение: валидация значений, лимит 256 KiB, атомарная запись tmp+rename.
#[tauri::command]
async fn settings_save(settings: UiSettings) -> Result<(), SettingsError> {
    blocking(move || SettingsStore::from_env().save(&settings))
        .await
        .map_err(|message| SettingsError::invalid("internal", message))?
}

/// Единый Tauri IPC-роутер для доменов BFF (`bff_domains_router`):
/// `slash_commands`, `onboarding`, `soundboard`, `stage_instances` и их
/// последующие расширения. Валидирует действие и отдаёт метод, маршрут и тело
/// запроса; токен и сеть сюда не попадают — отправку делает сетевой слой.
#[tauri::command]
fn invoke_bff_domain(
    action: discord_gui_spike::bff_domains_router::BffDomainAction,
) -> Result<discord_gui_spike::bff_domains_router::BffResolvedAction, String> {
    discord_gui_spike::bff_domains_router::resolve_bff_domain_action(action)
}

#[tauri::command]
async fn voice_connect(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_mute: Option<bool>,
    self_deaf: Option<bool>,
    self_video: Option<bool>,
    self_stream: Option<bool>,
) -> Result<serde_json::Value, String> {
    println!(
        "[IPC-TRACE] voice_connect guild_id={:?} channel_id={:?} self_mute={:?} self_deaf={:?} self_video={:?} self_stream={:?}",
        guild_id, channel_id, self_mute, self_deaf, self_video, self_stream
    );
    blocking(move || {
        data::connect_or_update_live_voice_ext(
            guild_id,
            channel_id,
            self_mute.unwrap_or(false),
            self_deaf.unwrap_or(false),
            self_video.unwrap_or(false),
            self_stream.unwrap_or(false),
        )
    })
    .await?
}

#[tauri::command]
async fn voice_disconnect(guild_id: Option<String>) -> Result<serde_json::Value, String> {
    println!("[IPC-TRACE] voice_disconnect guild_id={guild_id:?}");
    blocking(move || data::connect_or_update_live_voice(guild_id, None, false, false)).await?
}

#[tauri::command]
async fn voice_rtc_status() -> Result<serde_json::Value, String> {
    blocking(data::live_voice_rtc_status_json).await
}

#[tauri::command]
async fn voice_audio_devices() -> Result<serde_json::Value, String> {
    blocking(data::list_system_audio_devices_json).await
}

#[tauri::command]
async fn voice_set_audio_device(
    app: tauri::AppHandle,
    kind: String,
    device_id: Option<String>,
    volume_pct: Option<u32>,
    volume_percent: Option<u32>,
    muted: Option<bool>,
) -> Result<serde_json::Value, String> {
    if kind == "app_quit" {
        data::shutdown_live_workers();
        std::process::exit(0);
    }
    if kind == "app_hide_to_tray" {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        return Ok(serde_json::json!({ "ok": true, "hidden": true }));
    }
    if kind == "ui_telemetry_log" {
        if let Some(payload) = device_id {
            println!("[UI-TELEMETRY] {payload}");
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/discord-gui-spike-telemetry.jsonl")
                .and_then(|mut f| {
                    use std::io::Write;
                    writeln!(f, "{payload}")
                });
        }
        return Ok(serde_json::json!({ "ok": true }));
    }
    let vol = volume_pct.or(volume_percent);
    blocking(move || {
        data::set_system_audio_device(&kind, device_id.as_deref(), vol, muted)
    })
    .await
}

#[tauri::command]
async fn voice_desktop_sources() -> Result<serde_json::Value, String> {
    blocking(|| {
        let capturer = discord_media::ScreenCapturer::new();
        let sources = capturer.enumerate_sources();
        let json_sources: Vec<_> = sources
            .into_iter()
            .map(|s| {
                let st = match s.source_type {
                    discord_media::DesktopSourceType::Screen => "screen",
                    discord_media::DesktopSourceType::Window => "window",
                };
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "sourceType": st,
                    "source_type": st,
                    "isPrimary": s.is_primary,
                    "is_primary": s.is_primary,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "backend": capturer.backend().name(),
            "sources": json_sources
        }))
    })
    .await?
}

#[tauri::command]
async fn voice_stream_start(
    spec: discord_gui_spike::voice_stage::ScreenShareSpec,
) -> Result<serde_json::Value, String> {
    spec.validate()?;
    let op18 = discord_gui_spike::voice_stage::build_gateway_stream_create_op18(&spec)?;
    let stream_key = spec.to_stream_key("me");
    println!("[IPC-TRACE] voice_stream_start: stream_key={stream_key}");
    Ok(serde_json::json!({
        "ok": true,
        "streamKey": stream_key,
        "payload": op18
    }))
}

#[tauri::command]
async fn voice_stream_stop(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    let key = stream_key
        .or(streamKey)
        .ok_or_else(|| "отсутствует обязательный параметр stream_key / streamKey".to_string())?;
    let op19 = discord_gui_spike::voice_stage::build_gateway_stream_delete_op19(&key)?;
    println!("[IPC-TRACE] voice_stream_stop: stream_key={key}");
    Ok(serde_json::json!({
        "ok": true,
        "streamKey": key,
        "payload": op19
    }))
}

#[tauri::command]
async fn voice_stream_watch(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    let key = stream_key
        .or(streamKey)
        .ok_or_else(|| "отсутствует обязательный параметр stream_key / streamKey".to_string())?;
    let op20 = discord_gui_spike::voice_stage::build_gateway_stream_watch_op20(&key)?;
    println!("[IPC-TRACE] voice_stream_watch: stream_key={key}");
    Ok(serde_json::json!({
        "ok": true,
        "streamKey": key,
        "payload": op20
    }))
}

#[tauri::command]
async fn voice_stream_set_paused(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
    paused: bool,
) -> Result<serde_json::Value, String> {
    let key = stream_key
        .or(streamKey)
        .ok_or_else(|| "отсутствует обязательный параметр stream_key / streamKey".to_string())?;
    let op22 = discord_gui_spike::voice_stage::build_gateway_stream_set_paused_op22(&key, paused)?;
    println!("[IPC-TRACE] voice_stream_set_paused: stream_key={key} paused={paused}");
    Ok(serde_json::json!({
        "ok": true,
        "streamKey": key,
        "paused": paused,
        "payload": op22
    }))
}

const SYSTEM_TRAY_PY: &str = r#"
import os, sys, signal
import gi
gi.require_version("Gtk", "3.0")
gi.require_version("AppIndicator3", "0.1")
from gi.repository import Gtk, GLib, AppIndicator3

ppid = os.getppid()
icon_path = "/usr/share/icons/hicolor/256x256/apps/discord.png"
icon_name = icon_path if os.path.exists(icon_path) else "discord"

ind = AppIndicator3.Indicator.new(
    "discord-client-tray",
    icon_name,
    AppIndicator3.IndicatorCategory.APPLICATION_STATUS,
)
ind.set_status(AppIndicator3.IndicatorStatus.ACTIVE)
ind.set_title("Discord")

menu = Gtk.Menu()

item_open = Gtk.ImageMenuItem.new_with_label("Открыть Discord")
try:
    img = Gtk.Image.new_from_icon_name("discord", Gtk.IconSize.MENU)
    item_open.set_image(img)
    item_open.set_always_show_image(True)
except Exception:
    pass
item_open.connect("activate", lambda _: (sys.stdout.write("show\n"), sys.stdout.flush()))
menu.append(item_open)

menu.append(Gtk.SeparatorMenuItem())

item_upd = Gtk.MenuItem.new_with_label("Проверяется наличие обновлений...")
item_upd.set_sensitive(False)
menu.append(item_upd)

menu.append(Gtk.SeparatorMenuItem())

item_cred = Gtk.MenuItem.new_with_label("Благодарности")
item_cred.connect("activate", lambda _: (sys.stdout.write("credits\n"), sys.stdout.flush()))
menu.append(item_cred)

menu.append(Gtk.SeparatorMenuItem())

item_quit = Gtk.MenuItem.new_with_label("Выйти из Discord          Ctrl+Q")
item_quit.connect("activate", lambda _: (sys.stdout.write("quit\n"), sys.stdout.flush(), Gtk.main_quit()))
menu.append(item_quit)

menu.show_all()
ind.set_menu(menu)

def check_parent():
    if os.getppid() != ppid:
        Gtk.main_quit()
        return False
    return True

GLib.timeout_add(1000, check_parent)
signal.signal(signal.SIGTERM, lambda *_: Gtk.main_quit())
Gtk.main()
"#;

fn spawn_system_tray_indicator(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let child = std::process::Command::new("python3")
            .args(["-u", "-c", SYSTEM_TRAY_PY])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        let Ok(mut proc) = child else {
            return;
        };
        if let Some(stdout) = proc.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let cmd = line.trim();
                if cmd == "show" {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                } else if cmd == "credits" {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        let _ = window.eval(
                            "if (window.__OPEN_DISCORD_CREDITS__) window.__OPEN_DISCORD_CREDITS__();",
                        );
                    }
                } else if cmd == "quit" {
                    let _ = proc.kill();
                    let _ = proc.wait();
                    data::shutdown_live_workers();
                    std::process::exit(0);
                }
            }
        }
        let _ = proc.kill();
        let _ = proc.wait();
    });
}

fn main() {
    let is_headless_ci = std::env::var_os("SPIKE_EXIT_AFTER_MS").is_some()
        || std::env::var_os("SPIKE_AUTH_DISABLED").is_some();
    if is_headless_ci && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    tauri::Builder::default()
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if std::env::var_os("SPIKE_EXIT_AFTER_MS").is_none()
                    && std::env::var_os("SPIKE_AUTH_DISABLED").is_none()
                {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    data::shutdown_live_workers();
                }
            }
            tauri::WindowEvent::Destroyed => {
                data::shutdown_live_workers();
            }
            _ => {}
        })
        .setup(|app| {
            // Уникальный заголовок для GUI-пробы: чужой/старый экземпляр не должен
            // приниматься за запущенный этим тестом.
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(title) = std::env::var("SPIKE_WINDOW_TITLE") {
                    let _ = window.set_title(&title);
                }
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            // Маркер для харнесса холодного старта: окно создано.
            // Тестовый хук: live smoke/AT-SPI проверяет каркас без входа.
            // В обычном запуске переменная не задана — гейт входа работает.
            if std::env::var("SPIKE_AUTH_DISABLED").is_ok() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.eval("window.__SPIKE_AUTH_DISABLED__ = true;");
                }
            }
            println!("discord-gui-spike: window-ready");
            // Keyring подключается лениво (в потоке-владельце): окно стартует и без
            // Secret Service; статус сам сообщит `Keyring`-ошибку.
            app.manage(Arc::new(AuthBackend::keyring()));
            let auth = app.state::<Arc<AuthBackend>>().inner().clone();
            // L-02: конверт уходит в `gateway://dispatch` со своего потока;
            // источник — супервайзер ядра (hook `RunOptions.on_dispatch`), флаг и
            // механизм видны в `net_dispatch_stats` (`sourceAvailable: true`).
            let dispatch = Arc::new(TauriDispatchEmitter(app.handle().clone()));
            app.manage(Arc::new(NetService::with_emitter(auth, dispatch)));
            if std::env::var_os("SPIKE_EXIT_AFTER_MS").is_none()
                && std::env::var_os("SPIKE_AUTH_DISABLED").is_none()
            {
                spawn_system_tray_indicator(app.handle().clone());
            }
            if let Ok(ms) = std::env::var("SPIKE_EXIT_AFTER_MS") {
                if let Ok(ms) = ms.parse::<u64>() {
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(ms));
                        data::shutdown_live_workers();
                        std::process::exit(0);
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            process_metrics,
            spike_config,
            report_bench,
            report_ui_snapshot,
            ready,
            read_state,
            channels,
            messages_page,
            members_page,
            settings,
            search_page,
            search_remote,
            send_message,
            edit_message,
            delete_message,
            reply_message,
            ack_message,
            toggle_reaction,
            send_typing,
            join_guild,
            create_guild,
            resolve_invite,
            accept_invite,
            create_channel_invite,
            create_invite,
            request_upload_slots,
            attachments_upload_slots,
            attachments_upload_put,
            list_channel_pins,
            pins_list,
            pin_channel_message,
            unpin_channel_message,
            list_channel_threads,
            threads_list,
            update_guild_notification_settings,
            update_channel_notification_override,
            leave_guild,
            auth_status,
            auth_login_token,
            auth_restore,
            auth_logout,
            auth_login_password,
            auth_mfa,
            auth_sms,
            net_connect,
            net_status,
            net_dispatch_stats,
            net_disconnect,
            settings_load,
            settings_save,
            invoke_bff_domain,
            voice_connect,
            voice_disconnect,
            voice_rtc_status,
            voice_audio_devices,
            voice_set_audio_device,
            voice_desktop_sources,
            voice_stream_start,
            voice_stream_stop,
            voice_stream_watch,
            voice_stream_set_paused
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить Tauri-спайк");
}

// ---------------------------------------------------------------------------
// W39i2: wiring BFF-команд. Регистрация в `invoke_handler!` не видна в рантайме
// (Tauri не отдаёт список команд), а окно для этой проверки не нужно: контракт
// регистрации — сам исходник бинаря.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod wiring_tests {
    use super::*;
    use discord_gui_spike::bff_domains_router::{
        BffDomainAction, BffResolvedAction, UpdateGuildOnboardingRequest,
    };

    /// Тело списка команд внутри `invoke_handler!`.
    fn handler_block(source: &str) -> &str {
        const OPEN: &str = "generate_handler![";
        let start = source
            .find(OPEN)
            .expect("main.rs должен объявлять invoke_handler!")
            + OPEN.len();
        let rest = &source[start..];
        let end = rest.find(']').expect("invoke_handler! без закрывающей ]");
        &rest[..end]
    }

    /// Точное совпадение имени, а не подстрока: `search_page` не должен
    /// «находиться» внутри `search_page_v2` и наоборот.
    fn lists_command(block: &str, command: &str) -> bool {
        block
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|name| name == command)
    }

    #[test]
    fn invoke_handler_registers_all_bff_commands() {
        let block = handler_block(include_str!("main.rs"));
        for command in [
            "ready",
            "read_state",
            "channels",
            "messages_page",
            "members_page",
            "settings",
            "search_page",
            "search_remote",
            "net_dispatch_stats",
        ] {
            assert!(
                lists_command(block, command),
                "{command} не зарегистрирован в invoke_handler!"
            );
        }
    }

    /// Исходник до тестового модуля: литералы теста не дают ложного «нашлось».
    fn head_source() -> &'static str {
        include_str!("main.rs")
            .split("mod wiring_tests")
            .next()
            .expect("голова main.rs")
    }

    /// L-02: конверт уходит ровно в `gateway://dispatch`, ошибка эмита
    /// возвращается наверх (счётчик `emitFailed`), payload не логируется.
    #[test]
    fn dispatch_emitter_targets_gateway_event_without_logging() {
        let head = head_source();
        let start = head
            .find("impl DispatchEmitter for TauriDispatchEmitter")
            .expect("main.rs должен реализовать DispatchEmitter");
        let body = &head[start..];
        // Закрывающая скобка impl — `}` в начале строки (не `{error}` в format!).
        let body = &body[..body.find("\n}\n").expect("конец impl") + 2];
        assert!(
            body.contains("emit(DISPATCH_EVENT, payload)"),
            "конверт должен уходить в событие контракта: {body}"
        );
        assert!(
            body.contains("map_err("),
            "ошибка emit возвращается, а не глотается: {body}"
        );
        for forbidden in ["println!", "eprintln!", "log::", "payload.data"] {
            assert!(
                !body.contains(forbidden),
                "эмиттер не логирует payload ({forbidden}): {body}"
            );
        }
    }

    /// L-02: сервис создаётся с эмиттером — канал готов, источник в ядре
    /// подключён (`net::CORE_DISPATCH_AVAILABLE` → `sourceAvailable: true`).
    #[test]
    fn net_service_is_constructed_with_dispatch_emitter() {
        let head = head_source();
        assert!(head.contains("let dispatch = Arc::new(TauriDispatchEmitter("));
        assert!(head.contains("NetService::with_emitter(auth, dispatch)"));
        assert!(head.contains("net_dispatch_stats"));
    }

    /// Статистика L-02 — read-only команда без аргументов от фронтенда: токен
    /// в UI не читается и в аргументы не попадает.
    #[test]
    fn dispatch_stats_command_is_read_only_without_token() {
        let head = head_source();
        let start = head
            .find("fn net_dispatch_stats(")
            .expect("main.rs должен объявлять net_dispatch_stats");
        let body = &head[start..];
        let body = &body[..body.find("\n}\n").expect("конец команды") + 2];
        assert!(body.contains("net.dispatch_stats()"));
        for forbidden in ["token", "println!", "eprintln!", "log::"] {
            assert!(
                !body.contains(forbidden),
                "net_dispatch_stats не должен содержать {forbidden}: {body}"
            );
        }
    }

    /// C-02-BE: `channels` — локальная READY-проекция: без токена/keyring и без
    /// логирования. Тело команды извлекается из исходника (окно не нужно).
    #[test]
    fn channels_command_is_local_and_carries_no_token() {
        let source = include_str!("main.rs");
        let start = source
            .find("fn channels(guild_id: Option<String>)")
            .expect("main.rs должен объявлять команду channels");
        let rest = &source[start..];
        let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(
            body.contains("data::channels(&root, guild_id.as_deref())"),
            "команда обязана делегировать в data::channels: {body}"
        );
        for forbidden in [
            "token_or",
            "backend.token()",
            "AuthBackend",
            "println!",
            "eprintln!",
            "log::",
        ] {
            assert!(
                !body.contains(forbidden),
                "channels не должен содержать {forbidden}: {body}"
            );
        }
    }

    fn command_body<'a>(source: &'a str, name: &str) -> &'a str {
        let marker = format!("async fn {name}(");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("main.rs должен объявлять команду {name}"));
        let rest = &source[start..];
        let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
        &rest[..end]
    }

    const TOKEN_COMMANDS: [&str; 25] = [
        "search_remote",
        "send_message",
        "edit_message",
        "delete_message",
        "reply_message",
        "ack_message",
        "toggle_reaction",
        "send_typing",
        "join_guild",
        "leave_guild",
        "resolve_invite",
        "accept_invite",
        "create_channel_invite",
        "create_invite",
        "request_upload_slots",
        "attachments_upload_slots",
        "attachments_upload_put",
        "list_channel_pins",
        "pins_list",
        "pin_channel_message",
        "unpin_channel_message",
        "list_channel_threads",
        "threads_list",
        "update_guild_notification_settings",
        "update_channel_notification_override",
    ];

    /// G1-02: копий блока «blocking + два map_err + ok_or» больше нет — у каждой
    /// сетевой команды токен читает один helper `token_or`, коды задаёт домен.
    #[test]
    fn every_network_command_reads_token_through_one_helper() {
        let source = include_str!("main.rs");
        // `include_str!("main.rs")` видит и сам тест, поэтому считаем строки
        // объявления, а не подстроки: литерал в тесте не начинается с `async fn`.
        let declarations = source
            .lines()
            .filter(|line| line.starts_with("async fn token_or"))
            .count();
        assert_eq!(
            declarations, 1,
            "helper чтения токена должен быть объявлен ровно один раз"
        );
        for command in TOKEN_COMMANDS {
            let body = command_body(source, command);
            assert!(
                body.contains("token_or::<"),
                "{command}: токен читается только helper'ом token_or"
            );
            assert_eq!(
                body.matches("backend.token()").count(),
                1,
                "{command}: ровно одно чтение backend.token()"
            );
            assert!(
                !body.contains(".map_err(|_| "),
                "{command}: локальный маппинг ошибок токена запрещён"
            );
            assert!(
                !body.contains("println!") && !body.contains("eprintln!"),
                "{command}: токен и текст keyring-ошибки не логируются"
            );
        }
    }

    /// serde-тег `kind` — контракт с фронтендом; оракул не зависит от имени
    /// Rust-варианта.
    fn error_kind<T: serde::Serialize>(value: &T) -> String {
        let json = serde_json::to_value(value).expect("ошибка сериализуется");
        json.get("kind")
            .and_then(|kind| kind.as_str())
            .unwrap_or_else(|| panic!("нет строкового kind: {json}"))
            .to_string()
    }

    /// G1-02/SEC-02: один helper — типизированные коды каждого домена.
    /// keyring/blocking-сбой → `storage` (аддитивный kind), а не «сеть»:
    /// причина отказа хранилища не маскируется и не утекает. «Токена нет» —
    /// прежний `Unauthorized` у сообщений/реакций и `MissingToken` у search/guild.
    #[test]
    fn token_or_preserves_domain_error_codes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let keyring = || {
            Err(AuthError::Keyring {
                message: "Secret Service недоступен".to_string(),
            })
        };
        let no_token = || Ok(None);

        let error = runtime
            .block_on(token_or::<MessageError, _>(
                keyring,
                || MessageError::Storage,
                || MessageError::Unauthorized,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        let rendered = format!("{error}/{error:?}");
        assert!(
            !rendered.contains("Secret Service"),
            "текст keyring-ошибки не должен попадать в доменную ошибку: {rendered}"
        );
        assert_eq!(
            runtime.block_on(token_or::<MessageError, _>(
                no_token,
                || MessageError::Storage,
                || MessageError::Unauthorized,
            )),
            Err(MessageError::Unauthorized)
        );

        let error = runtime
            .block_on(token_or::<ReactionError, _>(
                keyring,
                || ReactionError::Storage,
                || ReactionError::Unauthorized,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");

        let error = runtime
            .block_on(token_or::<SearchError, _>(
                keyring,
                || SearchError::Storage,
                || SearchError::MissingToken,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        assert_eq!(
            runtime.block_on(token_or::<SearchError, _>(
                no_token,
                || SearchError::Storage,
                || SearchError::MissingToken,
            )),
            Err(SearchError::MissingToken)
        );

        let error = runtime
            .block_on(token_or::<EnterGuildError, _>(
                keyring,
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        assert_eq!(
            runtime.block_on(token_or::<EnterGuildError, _>(
                no_token,
                || EnterGuildError::Storage,
                || EnterGuildError::MissingToken,
            )),
            Err(EnterGuildError::MissingToken)
        );
        // Out-of-scope регрессия: «сеть» остаётся сетью (классификатор REST не
        // переиспользует `Storage`), поэтому код `Network` не подменяется.
        assert_eq!(error_kind(&EnterGuildError::Network), "network");

        let error = runtime
            .block_on(token_or::<LeaveGuildError, _>(
                keyring,
                || LeaveGuildError::Storage,
                || LeaveGuildError::MissingToken,
            ))
            .expect_err("keyring");
        assert_eq!(error_kind(&error), "storage", "{error}");
        assert_eq!(
            runtime.block_on(token_or::<LeaveGuildError, _>(
                no_token,
                || LeaveGuildError::Storage,
                || LeaveGuildError::MissingToken,
            )),
            Err(LeaveGuildError::MissingToken)
        );

        assert_eq!(
            runtime.block_on(token_or::<ReactionError, _>(
                || Ok(Some("secret-token".to_string())),
                || ReactionError::Storage,
                || ReactionError::Unauthorized,
            )),
            Ok("secret-token".to_string())
        );
    }

    /// Фикстур нет — skip с причиной (не падение): живые артефакты тест не
    /// подменяет, временных каталогов не создаёт.
    fn fixtures_or_skip() -> Option<std::path::PathBuf> {
        match data::fixtures_root() {
            Ok(root) => Some(root),
            Err(err) => {
                eprintln!("skip: фикстуры недоступны: {err}");
                None
            }
        }
    }

    #[test]
    fn filters_payload_is_camel_case_and_maps_to_data_type() {
        // Фильтры приходят объектом из JS: имена — camelCase, как аргументы
        // Tauri v2 (`channelId`, `guildId`), отсутствующие ключи — `None`.
        // Канонический `data::SearchFilters` сам десериализуется: зеркала нет.
        let filters: SearchFilters = serde_json::from_value(serde_json::json!({
            "authorId": "100",
            "channelId": "200",
            "content": "call of duty",
            "mentionId": "300",
            "day": "2026-10-19",
            "messageType": 0,
            "pinned": false
        }))
        .expect("фильтры из IPC");
        assert_eq!(
            filters,
            SearchFilters {
                author_id: Some("100".to_string()),
                channel_id: Some("200".to_string()),
                content: Some("call of duty".to_string()),
                mention_id: Some("300".to_string()),
                day: Some("2026-10-19".to_string()),
                message_type: Some(0),
                pinned: Some(false),
            }
        );

        let empty: SearchFilters = serde_json::from_value(serde_json::json!({}))
            .expect("пустой объект фильтров — не ошибка");
        assert_eq!(empty, SearchFilters::default());
    }

    #[test]
    fn search_page_command_passes_query_filters_and_cursor() {
        let Some(root) = fixtures_or_skip() else {
            return;
        };

        // query обрезается в слое данных: команда не должна добавлять свою логику.
        let expected =
            data::search_page(&root, "call of duty", &SearchFilters::default(), None, None)
                .expect("поиск по фикстурам");
        let actual = search_page("  call of duty  ".to_string(), None, None, None)
            .expect("команда search_page");
        assert_eq!(actual, expected);
        assert_eq!(actual.total, 1, "ровно одно совпадение в фикстурах");

        // Фильтр из IPC доезжает до слоя данных (pinned=false → все 13 строк).
        let filters: SearchFilters = serde_json::from_value(serde_json::json!({
            "pinned": false
        }))
        .expect("фильтр pinned");
        let expected_filtered = data::search_page(
            &root,
            "",
            &SearchFilters {
                pinned: Some(false),
                ..SearchFilters::default()
            },
            None,
            Some(5),
        )
        .expect("фильтрованная выборка");
        let actual_filtered =
            search_page(String::new(), Some(filters), None, Some(5)).expect("команда с фильтром");
        assert_eq!(actual_filtered, expected_filtered);

        // Курсор следующей страницы — id последнего результата, без пересечений.
        let cursor = actual_filtered
            .next_cursor
            .clone()
            .expect("при limit 5 и total 13 есть следующая страница");
        let second = search_page(String::new(), None, Some(cursor.clone()), Some(5))
            .expect("вторая страница");
        let expected_second =
            data::search_page(&root, "", &SearchFilters::default(), Some(&cursor), Some(5))
                .expect("вторая страница слоя данных");
        assert_eq!(second, expected_second);
        assert!(
            second.items.iter().all(|hit| !actual_filtered
                .items
                .iter()
                .any(|prev| prev.message.id == hit.message.id)),
            "страницы не должны пересекаться"
        );
    }

    type PutRecord = (String, String, Vec<u8>);

    #[derive(Clone, Default)]
    struct FakePutTransport {
        status: u16,
        puts: std::sync::Arc<std::sync::Mutex<Vec<PutRecord>>>,
    }

    impl FakePutTransport {
        fn answering(status: u16) -> Self {
            Self {
                status,
                puts: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn puts(&self) -> Vec<PutRecord> {
            self.puts.lock().expect("puts lock").clone()
        }
    }

    impl AttachmentTransport for FakePutTransport {
        fn post<'a>(
            &'a self,
            _target: &'a str,
            _body: &'a [u8],
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            discord_core::rest::transport::HttpResponse,
                            AttachmentError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async move { Err(AttachmentError::Network) })
        }

        fn put<'a>(
            &'a self,
            upload_url: &'a str,
            content_type: &'a str,
            body: &'a [u8],
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            discord_core::rest::transport::HttpResponse,
                            AttachmentError,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            self.puts.lock().expect("puts lock").push((
                upload_url.to_string(),
                content_type.to_string(),
                body.to_vec(),
            ));
            let status = self.status;
            Box::pin(async move {
                Ok(discord_core::rest::transport::HttpResponse {
                    status,
                    headers: Vec::new(),
                    body: Vec::new(),
                })
            })
        }
    }

    /// INT-04-ATTACH-PUT-BACKEND-PASS2: проверяет Tauri command extraction на точном плоском
    /// `putPayload` из `composer.js:744-755` (`{channelId, id, uploadUrl, uploadFilename, fileSize,
    /// contentType, bytes: {'0': 137, '1': 80}, data: {'0': 137, '1': 80}}`) и вызов команды
    /// с fake-транспортом.
    #[test]
    fn attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes() {
        let head = head_source();
        let body = command_body(head, "attachments_upload_put");
        let sig_end = body
            .find(')')
            .expect("конец сигнатуры attachments_upload_put");
        let sig = &body[..sig_end];

        let mut declared_keys = Vec::new();
        for raw_arg in sig.split('(').nth(1).expect("список параметров").split(',')
        {
            let trimmed = raw_arg.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((name_part, type_part)) = trimmed.split_once(':') else {
                continue;
            };
            if type_part.contains("tauri::State") {
                continue;
            }
            let snake = name_part.trim();
            let mut camel = String::with_capacity(snake.len());
            let mut upper = false;
            for ch in snake.chars() {
                if ch == '_' {
                    upper = true;
                } else if upper {
                    camel.push(ch.to_ascii_uppercase());
                    upper = false;
                } else {
                    camel.push(ch);
                }
            }
            declared_keys.push((snake.to_string(), camel));
        }

        let expected: Vec<(String, String)> =
            discord_gui_spike::attachments::UPLOAD_PUT_COMMAND_PARAM_KEYS
                .iter()
                .map(|(s, c)| ((*s).to_string(), (*c).to_string()))
                .collect();
        assert_eq!(
            declared_keys, expected,
            "параметры #[tauri::command] async fn attachments_upload_put должны включать `id: Option<String>`"
        );

        let upload_url =
            "https://discord-attachments-uploads-prd.storage.googleapis.com/upload-1?sig=abc123";
        let composer_put_payload = serde_json::json!({
            "channelId": "123456789012345678",
            "id": "slot-1",
            "uploadUrl": upload_url,
            "uploadFilename": "1/image.png",
            "fileSize": 2,
            "contentType": "image/png",
            "bytes": {"0": 137, "1": 80},
            "data": {"0": 137, "1": 80}
        });

        let args = UploadPutCommandArgs::from_tauri_ipc(&composer_put_payload)
            .expect("Tauri command extraction для плоского putPayload");
        let args_serde: UploadPutCommandArgs = serde_json::from_value(composer_put_payload)
            .expect("serde-десериализация UploadPutCommandArgs");
        assert_eq!(args, args_serde);
        assert_eq!(args.payload, None);
        assert_eq!(args.slot_id, None);
        assert_eq!(args.id.as_deref(), Some("slot-1"));
        assert_eq!(args.data, Some(UploadBytes(vec![137, 80])));
        assert_eq!(args.bytes, Some(UploadBytes(vec![137, 80])));

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        let transport = FakePutTransport::answering(200);
        assert_eq!(
            runtime.block_on(execute_attachments_upload_put(&transport, args)),
            Ok(())
        );
        assert_eq!(
            transport.puts(),
            vec![(
                upload_url.to_string(),
                "image/png".to_string(),
                vec![137, 80]
            )]
        );

        // Проверка `slot_id.or(id)`: пустой/пробельный `id` в плоском putPayload обязан
        // отклоняться командой с `InvalidArg` до обращения к транспорту:
        let bad_id_payload = serde_json::json!({
            "channelId": "123456789012345678",
            "id": "   ",
            "uploadUrl": upload_url,
            "uploadFilename": "1/image.png",
            "fileSize": 2,
            "contentType": "image/png",
            "bytes": {"0": 137, "1": 80},
            "data": {"0": 137, "1": 80}
        });
        let bad_args = UploadPutCommandArgs::from_tauri_ipc(&bad_id_payload)
            .expect("Tauri command extraction для bad_id_payload");
        let guard_transport = FakePutTransport::answering(200);
        assert_eq!(
            runtime.block_on(execute_attachments_upload_put(&guard_transport, bad_args)),
            Err(AttachmentError::InvalidArg)
        );
        assert!(guard_transport.puts().is_empty());
    }

    /// INT-05-IPC-ALIASES: все 4 F-1 IPC-алиаса (`attachments_upload_slots`, `create_invite`,
    /// `pins_list`, `threads_list`) зарегистрированы в `generate_handler!` и делегируют
    /// в те же доменные обёртки с сохранением кодов ошибок и формы ответов UI.
    #[test]
    fn invoke_handler_registers_f1_ui_ipc_aliases_and_wires_payload_contracts() {
        let head = head_source();
        let block = handler_block(head);
        for command in [
            "attachments_upload_slots",
            "create_invite",
            "pins_list",
            "threads_list",
        ] {
            assert!(
                lists_command(block, command),
                "{command} не зарегистрирован в invoke_handler!"
            );
        }

        let slots_body = command_body(head, "attachments_upload_slots");
        assert!(slots_body.contains("UploadSlotsEnvelope { attachments }"));
        assert!(slots_body
            .contains("discord_gui_spike::request_upload_slots(&api, &channel_id, &specs)"));

        let invite_body = command_body(head, "create_invite");
        assert!(invite_body.contains(
            "CreateInviteParamsInput::resolve(params, max_age, max_uses, temporary, flags)"
        ));
        assert!(invite_body
            .contains("discord_gui_spike::create_channel_invite(&api, &channel_id, &resolved)"));

        let pins_body = command_body(head, "pins_list");
        assert!(pins_body.contains("discord_gui_spike::list_channel_pins(&api, &channel_id)"));

        let threads_body = command_body(head, "threads_list");
        assert!(threads_body.contains(
            "discord_gui_spike::list_channel_threads(&api, &channel_id, archived.unwrap_or(false))"
        ));

        // Поведенческая проверка десериализации/резолва плоского и вложенного payload (camelCase + snake_case):
        let flat_resolved =
            CreateInviteParamsInput::resolve(None, Some(3600), Some(10), Some(true), Some(1));
        assert_eq!(
            flat_resolved,
            CreateInviteParams {
                max_age: 3600,
                max_uses: 10,
                temporary: true,
                flags: Some(1),
            }
        );

        let nested_camel: CreateInviteParamsInput = serde_json::from_value(serde_json::json!({
            "maxAge": 1800,
            "maxUses": 5,
            "temporary": true,
            "flags": 1
        }))
        .expect("camelCase params");
        assert_eq!(
            CreateInviteParamsInput::resolve(Some(nested_camel), None, None, None, None),
            CreateInviteParams {
                max_age: 1800,
                max_uses: 5,
                temporary: true,
                flags: Some(1),
            }
        );

        let nested_snake: CreateInviteParamsInput = serde_json::from_value(serde_json::json!({
            "max_age": 43200,
            "max_uses": 25,
            "temporary": false
        }))
        .expect("snake_case params");
        assert_eq!(
            CreateInviteParamsInput::resolve(Some(nested_snake), None, None, None, None),
            CreateInviteParams {
                max_age: 43200,
                max_uses: 25,
                temporary: false,
                flags: None,
            }
        );

        assert_eq!(
            CreateInviteParamsInput::resolve(None, None, None, None, None),
            CreateInviteParams::default()
        );

        // Поведенческая проверка `UploadSlotSpecInput` (snake_case из composer.js:725 и camelCase) + `UploadSlotsEnvelope`:
        let snake_spec: UploadSlotSpecInput = serde_json::from_value(serde_json::json!({
            "filename": "avatar.png",
            "file_size": 5000,
            "id": "loc-1",
            "is_clip": false
        }))
        .expect("snake_case UploadSlotSpecInput");
        assert_eq!(
            AttachmentFileSpec::from(snake_spec),
            AttachmentFileSpec {
                filename: "avatar.png".to_string(),
                file_size: 5000,
                id: "loc-1".to_string(),
                is_clip: false,
            }
        );

        let camel_spec: UploadSlotSpecInput = serde_json::from_value(serde_json::json!({
            "filename": "clip.mp4",
            "fileSize": 8192,
            "id": "loc-2",
            "isClip": true
        }))
        .expect("camelCase UploadSlotSpecInput");
        assert_eq!(
            AttachmentFileSpec::from(camel_spec),
            AttachmentFileSpec {
                filename: "clip.mp4".to_string(),
                file_size: 8192,
                id: "loc-2".to_string(),
                is_clip: true,
            }
        );

        let envelope = UploadSlotsEnvelope {
            attachments: vec![AttachmentUploadSlot {
                id: "loc-1".to_string(),
                upload_url: "https://discord.upload/slot-1".to_string(),
                upload_filename: "uploaded-avatar.png".to_string(),
            }],
        };
        assert_eq!(
            serde_json::to_value(&envelope).expect("envelope JSON"),
            serde_json::json!({
                "attachments": [
                    {
                        "id": "loc-1",
                        "upload_url": "https://discord.upload/slot-1",
                        "upload_filename": "uploaded-avatar.png"
                    }
                ]
            })
        );
    }

    #[test]
    fn invoke_handler_registers_notifications_ipc_commands_and_resolves_channel_override_input() {
        let head = head_source();
        let block = handler_block(head);
        for command in [
            "update_guild_notification_settings",
            "update_channel_notification_override",
        ] {
            assert!(
                lists_command(block, command),
                "{command} не зарегистрирован в invoke_handler!"
            );
        }

        // Разбор конверта `{ channel_overrides: { [channelId]: overridePatch } }` из notifications.js:1389:
        let envelope_input: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "channel_overrides": {
                "ch-announce": {
                    "message_notifications": 2,
                    "muted": false
                }
            }
        }))
        .expect("envelope ChannelOverrideInput");
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                Some(envelope_input),
                None
            ),
            Ok(ChannelNotificationOverridePatch::new()
                .with_message_notifications(2)
                .with_muted(false))
        );

        // P2-2: если конверт channel_overrides содержит чужой канал и не содержит запрошенный channel_id,
        // into_patch / resolve НЕ должны молча брать чужой канал, а обязаны вернуть Err(NotificationError::InvalidChannelId):
        let foreign_envelope: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "channel_overrides": {
                "ch-foreign": {
                    "message_notifications": 0,
                    "muted": true
                }
            }
        }))
        .expect("foreign envelope ChannelOverrideInput");
        assert_eq!(
            foreign_envelope.clone().into_patch("ch-announce"),
            Err(NotificationError::InvalidChannelId)
        );
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                Some(foreign_envelope),
                None
            ),
            Err(NotificationError::InvalidChannelId)
        );

        // Пустой конверт `{ channel_overrides: {} }` возвращает пустой патч:
        let empty_envelope: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "channel_overrides": {}
        }))
        .expect("empty envelope ChannelOverrideInput");
        assert_eq!(
            empty_envelope.into_patch("ch-announce"),
            Ok(ChannelNotificationOverridePatch::default())
        );

        // P2-3: вызов только с полем `override` / `override_field` без дублирования `patch`:
        let override_only = ChannelNotificationOverridePatch::new()
            .with_message_notifications(0)
            .with_muted(true);
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                Some(override_only.clone()),
                None,
                None,
                None,
                None
            ),
            Ok(override_only.clone())
        );
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                Some(override_only.clone()),
                None,
                None,
                None
            ),
            Ok(override_only.clone())
        );
        let override_json_input: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "override": {
                "message_notifications": 0,
                "muted": true
            }
        }))
        .expect("override field JSON ChannelOverrideInput");
        assert_eq!(
            override_json_input.clone().into_patch("ch-announce"),
            Ok(override_only.clone())
        );
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                None,
                Some(override_json_input)
            ),
            Ok(override_only)
        );

        // Разбор прямого payload `{ message_notifications: 1, muted: true }`:
        let direct_input: ChannelOverrideInput = serde_json::from_value(serde_json::json!({
            "message_notifications": 1,
            "muted": true
        }))
        .expect("direct ChannelOverrideInput");
        assert_eq!(
            ChannelOverrideInput::resolve(
                "ch-announce",
                None,
                None,
                None,
                None,
                Some(direct_input)
            ),
            Ok(ChannelNotificationOverridePatch::new()
                .with_message_notifications(1)
                .with_muted(true))
        );
    }

    /// BE-INT-16: единый IPC-роутер BFF зарегистрирован в `invoke_handler!` и
    /// делегирует в `bff_domains_router::resolve_bff_domain_action`.
    ///
    /// Имя теста сохраняет карточку (`seven_domains` — целевой список доменов при
    /// расширении роутера). Фактический контракт `bff_domains_router` на базе
    /// `unstable 648ba83` — 4 домена, поэтому валидный/невалидный вызов проверяется
    /// на `UpdateGuildOnboarding` (решение координатора по task-293).
    #[test]
    fn invoke_handler_registers_invoke_bff_domain_for_seven_domains() {
        assert!(
            lists_command(handler_block(head_source()), "invoke_bff_domain"),
            "invoke_bff_domain не зарегистрирован в invoke_handler!"
        );

        // Валидный guild_id: роутер отдаёт метод, маршрут и тело запроса.
        assert_eq!(
            invoke_bff_domain(BffDomainAction::UpdateGuildOnboarding {
                guild_id: "123456789012345678".to_string(),
                request: UpdateGuildOnboardingRequest {
                    enabled: true,
                    description: None,
                    welcome_channels: Vec::new(),
                },
            }),
            Ok(BffResolvedAction {
                method: "PATCH",
                route: "/api/v9/guilds/123456789012345678/welcome-screen".to_string(),
                body: serde_json::json!({
                    "enabled": true,
                    "description": null,
                    "welcome_channels": []
                }),
            })
        );

        // Невалидный Snowflake (ведущий '0'): ошибка поднимается как Err, а не паника.
        let invalid = invoke_bff_domain(BffDomainAction::UpdateGuildOnboarding {
            guild_id: "0".to_string(),
            request: UpdateGuildOnboardingRequest {
                enabled: true,
                description: None,
                welcome_channels: Vec::new(),
            },
        });
        assert!(
            invalid.is_err(),
            "guild_id=\"0\" должен давать Err: {invalid:?}"
        );
    }
}
