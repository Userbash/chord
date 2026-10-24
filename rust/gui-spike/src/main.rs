//! P0-01: спайк UI-оболочки Tauri.
//!
//! Минимальное окно + IPC-команды для честного замера памяти процесса и
//! проверяемого UI-снимка. Сети нет; BFF фазы 3 читает только локальные
//! `fixtures/` (`DL_FIXTURES`, по умолчанию — `fixtures/` репозитория).
//! Чистая логика вынесена в `discord_gui_spike::lib`/`discord_gui_spike::data`
//! и покрыта `tests/ipc_contract.rs` + `tests/data_contract.rs`, живой запуск —
//! `tests/launch.rs`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use discord_gui_spike::auth::{self, AuthBackend, AuthError, AuthState, LoginView, SmsChallenge};
use discord_gui_spike::data::{
    self, ChannelRow, DataError, MembersPage, MessagesPage, MessagesPageRequest, MessagesPosition,
    ReadStateEntry, ReadyPayload, SearchFilters, SearchPage, SettingsPayload,
};
use discord_gui_spike::guilds::{self, DiscordGuilds, EnterGuildError, LeaveGuildError};
use discord_gui_spike::invites::{
    self, AcceptedInvite, DiscordInvites, InviteError, InvitePreview,
};
use discord_gui_spike::messages::{self, DiscordMessageSender, DiscordReadState, MessageError};
use discord_gui_spike::net::{
    DispatchEmitter, DispatchPayload, DispatchStats, NetService, DISPATCH_EVENT,
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
fn process_metrics() -> Result<ProcessMetrics, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|err| format!("не удалось прочитать /proc/self/status: {err}"))?;
    parse_process_metrics(&status)
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
    let value = validate_ui_snapshot(&payload)?;
    println!("UI_SNAPSHOT {value}");
    Ok(())
}

// ---------------------------------------------------------------------------
// BFF фазы 3 (W39e): только локальные `fixtures/`, без сети.
// Корень — `DL_FIXTURES` или `fixtures/` репозитория; ошибки типизированы
// `DataError` и уходят во фронтенд как `{kind, message, path?/resource?}`.
// ---------------------------------------------------------------------------

/// READY: пользователь, гильдии и счётчики первого экрана.
#[tauri::command]
fn ready() -> Result<ReadyPayload, DataError> {
    let root = data::fixtures_root()?;
    data::ready(&root)
}

/// Записи `READY.read_state`: `lastViewed` (курсор ack), `flags`,
/// `mentionCount`. Локальная READY-проекция: токен не читается и не логируется.
#[tauri::command]
fn read_state() -> Result<Vec<ReadStateEntry>, DataError> {
    let root = data::fixtures_root()?;
    data::read_state(&root)
}

/// Дерево каналов из READY: `guildId` — необязательный фильтр, `None` — все
/// каналы и ЛС. Локальная проекция: токен не читается и не логируется.
#[tauri::command]
fn channels(guild_id: Option<String>) -> Result<Vec<ChannelRow>, DataError> {
    let root = data::fixtures_root()?;
    data::channels(&root, guild_id.as_deref())
}

/// Страница сообщений канала. `cursor` — forward (id последней строки
/// предыдущей страницы), `before` — older (id первой строки окна: вернуть
/// строки старше), `position` — `"start"` (по умолчанию) или `"latest"`.
/// Курсоры взаимоисключающие; семантика — в `data::messages_page_request`.
#[tauri::command]
fn messages_page(
    channel_id: String,
    cursor: Option<String>,
    before: Option<String>,
    position: Option<String>,
    limit: Option<usize>,
) -> Result<MessagesPage, DataError> {
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
}

/// Страница участников гильдии; онлайн-участники идут первыми.
#[tauri::command]
fn members_page(
    guild_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<MembersPage, DataError> {
    let root = data::fixtures_root()?;
    data::members_page(&root, &guild_id, cursor.as_deref(), limit)
}

/// Настройки из READY: все секции или одна по `section`.
#[tauri::command]
fn settings(section: Option<String>) -> Result<SettingsPayload, DataError> {
    let root = data::fixtures_root()?;
    data::settings(&root, section.as_deref())
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
) -> Result<discord_gui_spike::messages::MessageReceipt, MessageError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<MessageError, _>(
        move || backend.token(),
        || MessageError::Storage,
        || MessageError::Unauthorized,
    )
    .await?;
    let sender = DiscordMessageSender::new(&token)?;
    discord_gui_spike::messages::send_message(&sender, &channel_id, &content, &nonce).await
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
async fn accept_invite(
    backend: tauri::State<'_, Arc<AuthBackend>>,
    code: String,
    session_id: Option<String>,
    invite_instance_id: Option<String>,
    received_installation_id: Option<String>,
) -> Result<AcceptedInvite, InviteError> {
    let backend = Arc::clone(backend.inner());
    let token = token_or::<InviteError, _>(
        move || backend.token(),
        || InviteError::Storage,
        || InviteError::MissingToken,
    )
    .await?;
    let api = DiscordInvites::new(&token)?;
    invites::accept_invite(
        &api,
        &code,
        session_id.as_deref(),
        invite_instance_id.as_deref(),
        received_installation_id.as_deref(),
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
    let state = blocking(move || backend.login(&token))
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
    let state = blocking(move || backend.restore())
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
        backend.logout_with_disconnect(|| {
            net.disconnect()
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
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
        LoginView::from_outcome(outcome, &backend.status())
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
        LoginView::from_outcome(outcome, &backend.status())
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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Уникальный заголовок для GUI-пробы: чужой/старый экземпляр не должен
            // приниматься за запущенный этим тестом.
            if let Ok(title) = std::env::var("SPIKE_WINDOW_TITLE") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_title(&title);
                }
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
            if let Ok(ms) = std::env::var("SPIKE_EXIT_AFTER_MS") {
                if let Ok(ms) = ms.parse::<u64>() {
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(ms));
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
            resolve_invite,
            accept_invite,
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
            settings_save
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

    const TOKEN_COMMANDS: [&str; 12] = [
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
}
