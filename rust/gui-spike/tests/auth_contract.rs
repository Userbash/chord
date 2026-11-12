//! Контракт backend-авторизации (RW-39 / NET-C2): `auth_status`/`auth_login_token`/
//! `auth_logout`/`auth_restore` без сети и без живого keyring.
//!
//! Оракулы — внешние, а не текущая реализация:
//! - `Authorization` — сырой токен без `Bearer` (`AUTH-RE.md` H-01, `web.js @1002375`);
//! - проверка токена — `GET /users/@me` (`AUTH-RE.md` A-15), тело — реальная форма
//!   `/users/@me` и тела ошибки Discord `{message, code}` (`rest::user`);
//! - keyring `!Send` (`discord-platform::keyring`) — значит все вызовы идут через
//!   поток-владелец; тест использует тот же механизм с фейковым хранилищем;
//! - токен не попадает ни в JSON состояния, ни в текст ошибки.
//!
//! Тесты не запускают окно, не ходят в сеть и не трогают Secret Service: валидатор и
//! хранилище — фейки; ошибки 401/429/сеть/битый JSON подаются как данные.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chord_core::rest::client::ApiError;
use chord_core::rest::transport::{HttpError, HttpResponse};
use chord_core::Me;
use chord_client::auth::{
    self, AuthBackend, AuthError, AuthState, AuthStateEmitter, AuthStatus, IdentityContext,
    LoginOutcome, LoginTransport, LoginView, SmsChallenge, ThreadedStore, TokenStore,
    TokenValidator, ValidationRequest, CAPTCHA_KEY_HEADER, CAPTCHA_RQDATA_HEADER,
    CAPTCHA_RQTOKEN_HEADER, CAPTCHA_SESSION_ID_HEADER, LOGIN_PATH, MFA_SMS_SEND_PATH,
};
use serde_json::Value;

/// Токен Chord (3 сегмента через точку).
const TOKEN: &str = "chord_mock_token_alpha.mockpart.chord_mock_token_signature_one";
/// Второй токен: проверяем замену значения, а не «первое попавшееся».
const TOKEN2: &str = "chord_mock_token_beta.mockpart.chord_mock_token_signature_two";

/// Пользователь, которого вернул бы `GET /users/@me` (форма 2026: discriminator "0").
fn me() -> Me {
    Me {
        id: "123456789012345678".to_string(),
        username: "tester".to_string(),
        discriminator: "0".to_string(),
        global_name: Some("Тестер".to_string()),
        avatar: None,
        bot: false,
    }
}

/// Тело успешного `/users/@me` — сверяется с независимым парсером ядра.
const ME_BODY: &str = r#"{"id":"123456789012345678","username":"tester","discriminator":"0","global_name":"Тестер","avatar":null}"#;

/// Тело 401 от Discord (`AUTH-RE.md` E-07: `50014 INVALID_TOKEN`).
const UNAUTHORIZED_BODY: &str = r#"{"message":"401: Unauthorized","code":0}"#;

// ---------------------------------------------------------------------------
// Фейки: хранилище и валидатор
// ---------------------------------------------------------------------------

/// Журнал обращений к фейковому хранилищу (живёт вне потока-владельца).
#[derive(Debug, Default)]
struct StoreLog {
    loads: usize,
    stores: Vec<String>,
    clears: usize,
}

/// Фейковое хранилище токена — заменяет `Keyring` (никакого libsecret).
struct FakeStore {
    token: Option<String>,
    log: Arc<Mutex<StoreLog>>,
    load_error: Option<String>,
    store_error: Option<String>,
    clear_error: Option<String>,
}

impl FakeStore {
    fn new(token: Option<&str>, log: Arc<Mutex<StoreLog>>) -> Self {
        Self {
            token: token.map(str::to_string),
            log,
            load_error: None,
            store_error: None,
            clear_error: None,
        }
    }

    fn failing(error: &'static str) -> Self {
        Self {
            token: None,
            log: Arc::new(Mutex::new(StoreLog::default())),
            load_error: Some(error.to_string()),
            store_error: Some(error.to_string()),
            clear_error: Some(error.to_string()),
        }
    }
}

impl TokenStore for FakeStore {
    fn load(&mut self) -> Result<Option<String>, String> {
        self.log.lock().expect("журнал хранилища").loads += 1;
        match &self.load_error {
            Some(error) => Err(error.clone()),
            None => Ok(self.token.clone()),
        }
    }

    fn store(&mut self, token: &str) -> Result<(), String> {
        self.log
            .lock()
            .expect("журнал хранилища")
            .stores
            .push(token.to_string());
        match &self.store_error {
            Some(error) => Err(error.clone()),
            None => {
                self.token = Some(token.to_string());
                Ok(())
            }
        }
    }

    fn clear(&mut self) -> Result<bool, String> {
        self.log.lock().expect("журнал хранилища").clears += 1;
        match &self.clear_error {
            Some(error) => Err(error.clone()),
            None => Ok(self.token.take().is_some()),
        }
    }
}

/// Фейковый валидатор токена: считает вызовы и отдаёт заранее заданный исход.
struct FakeValidator {
    result: Result<Me, AuthError>,
    calls: AtomicUsize,
}

impl FakeValidator {
    fn ok() -> Self {
        Self {
            result: Ok(me()),
            calls: AtomicUsize::new(0),
        }
    }

    fn err(error: AuthError) -> Self {
        Self {
            result: Err(error),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl TokenValidator for FakeValidator {
    fn validate(&self, _request: &ValidationRequest) -> Result<Me, AuthError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

/// Детерминированная identity: без чтения `/proc`, env и `/dev/urandom`.
fn identity() -> IdentityContext {
    IdentityContext::new("ru", "Europe/Moscow")
}

fn backend(store: FakeStore, validator: Arc<FakeValidator>) -> (AuthBackend, Arc<Mutex<StoreLog>>) {
    let log = Arc::clone(&store.log);
    (
        AuthBackend::with_identity(Box::new(store), validator, identity()),
        log,
    )
}

/// JSON состояния — ровно то, что уйдёт во фронтенд.
fn json(state: &AuthState) -> Value {
    serde_json::to_value(state).expect("AuthState сериализуется")
}

#[derive(Default)]
struct FakeEmitter {
    events: Mutex<Vec<Value>>,
}

impl AuthStateEmitter for FakeEmitter {
    fn emit_state(&self, state: &AuthState) -> Result<(), String> {
        self.events
            .lock()
            .expect("журнал событий")
            .push(json(state));
        Ok(())
    }
}

#[test]
fn auth_state_events_are_redacted_and_cover_phases() {
    let emitter = FakeEmitter::default();
    for state in [
        AuthState::idle(false),
        AuthState::validating(),
        AuthState::ready((&me()).into(), true),
        AuthState::error(AuthError::InvalidToken),
    ] {
        auth::emit_auth_state(&emitter, &state).expect("эмитировать состояние");
    }
    let events = emitter.events.lock().expect("журнал событий");
    assert_eq!(events.len(), 4);
    assert_eq!(events[0]["status"], "idle");
    assert_eq!(events[1]["status"], "validating");
    assert_eq!(events[2]["status"], "ready");
    assert_eq!(events[3]["status"], "error");
    let text = serde_json::to_string(&*events).expect("JSON событий");
    assert!(!text.contains(TOKEN));
}

#[test]
fn logout_disconnects_before_clearing_keyring() {
    let validator = Arc::new(FakeValidator::ok());
    let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let backend = AuthBackend::with_identity(
        Box::new(OrderedStore {
            inner: store,
            order: Arc::clone(&order),
        }),
        validator,
        identity(),
    );
    backend.logout_with_disconnect(|| {
        order.lock().expect("порядок").push("disconnect");
        Ok(())
    });
    assert_eq!(*order.lock().expect("порядок"), vec!["disconnect", "clear"]);
}

struct OrderedStore {
    inner: FakeStore,
    order: Arc<Mutex<Vec<&'static str>>>,
}

impl TokenStore for OrderedStore {
    fn load(&mut self) -> Result<Option<String>, String> {
        self.inner.load()
    }
    fn store(&mut self, token: &str) -> Result<(), String> {
        self.inner.store(token)
    }
    fn clear(&mut self) -> Result<bool, String> {
        self.order.lock().expect("порядок").push("clear");
        self.inner.clear()
    }
}

// ---------------------------------------------------------------------------
// auth_status: только keyring, сеть — никогда
// ---------------------------------------------------------------------------

#[test]
fn status_without_stored_token_is_idle_and_never_validates() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, log) = backend(
        FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default()))),
        validator.clone(),
    );

    let state = backend.status();
    assert_eq!(state.status, AuthStatus::Idle);
    assert_eq!(state.user, None);
    assert_eq!(state.error, None);
    assert!(!state.stored, "в keyring токена нет");
    assert_eq!(validator.calls(), 0, "auth_status не ходит в сеть");

    // Повторный статус не дёргает keyring повторно: результат уже известен.
    let again = backend.status();
    assert_eq!(again, state);
    assert_eq!(
        log.lock().expect("журнал").loads,
        1,
        "keyring читается один раз"
    );
}

#[test]
fn status_reports_stored_token_but_does_not_probe_network() {
    let validator = Arc::new(FakeValidator::ok());
    let store = FakeStore::new(Some(TOKEN), Arc::new(Mutex::new(StoreLog::default())));
    let (backend, _log) = backend(store, validator.clone());

    let state = backend.status();
    assert_eq!(
        state.status,
        AuthStatus::Idle,
        "без auth_restore сессии ещё нет"
    );
    assert!(state.stored, "keyring сообщил о сохранённом токене");
    assert_eq!(validator.calls(), 0, "статус не проверяет токен в сети");

    let text = json(&state).to_string();
    assert!(!text.contains(TOKEN), "токен не уходит во фронтенд: {text}");
}

#[test]
fn status_reports_keyring_failure_as_error() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _log) = backend(FakeStore::failing("нет D-Bus"), validator);

    let state = backend.status();
    assert_eq!(state.status, AuthStatus::Error);
    assert_eq!(state.error.as_ref().map(AuthError::kind), Some("Keyring"));
    assert!(!state.stored);
}

#[test]
fn token_reads_keyring_without_validation_and_redacts_errors() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, log) = backend(
        FakeStore::new(Some(TOKEN), Arc::new(Mutex::new(StoreLog::default()))),
        validator.clone(),
    );

    assert_eq!(
        backend.token().expect("чтение токена"),
        Some(TOKEN.to_string())
    );
    assert_eq!(validator.calls(), 0, "token() не валидирует токен по сети");
    assert_eq!(log.lock().expect("журнал").loads, 1);

    let empty = AuthBackend::with_identity(
        Box::new(FakeStore::new(
            None,
            Arc::new(Mutex::new(StoreLog::default())),
        )),
        validator,
        identity(),
    );
    assert_eq!(empty.token().expect("пустой keyring"), None);

    let failing = AuthBackend::with_identity(
        Box::new(FakeStore::failing("keyring недоступен")),
        Arc::new(FakeValidator::ok()),
        identity(),
    );
    let error = failing.token().expect_err("ошибка keyring возвращается");
    assert_eq!(error.kind(), "Keyring");
    assert!(!format!("{error:?}").contains(TOKEN));
    assert!(!error.to_string().contains(TOKEN));
}

// ---------------------------------------------------------------------------
// auth_login_token: 200 → Ready, keyring.store
// ---------------------------------------------------------------------------

#[test]
fn login_validates_token_and_persists_it() {
    let validator = Arc::new(FakeValidator::ok());
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = AuthBackend::with_identity(Box::new(store), validator.clone(), identity());

    let state = backend.login(TOKEN);
    assert_eq!(state.status, AuthStatus::Ready);
    assert!(state.stored);
    assert_eq!(state.error, None);
    let user = state.user.expect("пользователь в READY");
    assert_eq!(user.id, "123456789012345678");
    assert_eq!(user.username, "tester");
    assert_eq!(user.global_name.as_deref(), Some("Тестер"));
    assert_eq!(validator.calls(), 1);

    let log = log.lock().expect("журнал");
    assert_eq!(
        log.stores,
        vec![TOKEN.to_string()],
        "токен сохранён в keyring"
    );
    assert_eq!(
        backend.status().status,
        AuthStatus::Ready,
        "статус помнит сессию"
    );
}

#[test]
fn login_trims_pasted_token_before_use() {
    let validator = Arc::new(FakeValidator::ok());
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = AuthBackend::with_identity(Box::new(store), validator, identity());

    let state = backend.login(&format!("  {TOKEN}\n"));
    assert_eq!(state.status, AuthStatus::Ready);
    assert_eq!(log.lock().expect("журнал").stores, vec![TOKEN.to_string()]);
}

#[test]
fn login_rejects_empty_token_without_network_or_keyring() {
    let validator = Arc::new(FakeValidator::ok());
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = AuthBackend::with_identity(Box::new(store), validator.clone(), identity());

    for empty in ["", "   ", "\n\t"] {
        let state = backend.login(empty);
        assert_eq!(state.status, AuthStatus::Error);
        assert_eq!(
            state.error.as_ref().map(AuthError::kind),
            Some("EmptyToken")
        );
    }
    assert_eq!(
        validator.calls(),
        0,
        "пустой токен не уходит даже в валидатор"
    );
    assert!(log.lock().expect("журнал").stores.is_empty());
}

// ---------------------------------------------------------------------------
// auth_login_token: негатив 401/429/сеть/битый JSON
// ---------------------------------------------------------------------------

#[test]
fn login_invalid_token_401_keeps_nothing_in_keyring() {
    let validator = Arc::new(FakeValidator::err(AuthError::InvalidToken));
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = AuthBackend::with_identity(Box::new(store), validator, identity());

    let state = backend.login(TOKEN);
    assert_eq!(state.status, AuthStatus::Error);
    assert_eq!(
        state.error.as_ref().map(AuthError::kind),
        Some("InvalidToken")
    );
    assert!(!state.stored);
    assert!(
        log.lock().expect("журнал").stores.is_empty(),
        "401 не сохраняет токен"
    );

    let text = json(&state).to_string();
    assert!(!text.contains(TOKEN), "токен не попадает в ошибку: {text}");
    assert!(
        !format!("{:?}", state.error).contains(TOKEN),
        "Debug ошибки не печатает токен"
    );
}

#[test]
fn login_rate_limited_429_reports_retry_after() {
    let validator = Arc::new(FakeValidator::err(AuthError::RateLimited {
        retry_after_ms: 5_000,
    }));
    let (backend, _log) = backend(
        FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default()))),
        validator,
    );

    let state = backend.login(TOKEN);
    assert_eq!(state.status, AuthStatus::Error);
    assert_eq!(
        state.error.as_ref().map(AuthError::kind),
        Some("RateLimited")
    );
    assert_eq!(
        json(&state)["error"]["retry_after_ms"].as_u64(),
        Some(5_000)
    );
}

#[test]
fn login_network_failure_is_reported_as_network() {
    let validator = Arc::new(FakeValidator::err(AuthError::Network {
        message: "таймаут после 3 попыток".to_string(),
    }));
    let (backend, _log) = backend(
        FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default()))),
        validator,
    );

    let state = backend.login(TOKEN);
    assert_eq!(state.status, AuthStatus::Error);
    assert_eq!(state.error.as_ref().map(AuthError::kind), Some("Network"));
}

#[test]
fn login_broken_json_maps_to_parse_without_token() {
    // Валидатор разбирает тело настоящим парсером ядра: битый JSON — Parse, тело 401 —
    // InvalidToken. Сети нет, тело подаётся как данные.
    struct BodyValidator {
        body: Vec<u8>,
    }
    impl TokenValidator for BodyValidator {
        fn validate(&self, request: &ValidationRequest) -> Result<Me, AuthError> {
            auth::parse_me_body(&self.body, &request.token)
        }
    }

    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = AuthBackend::with_identity(
        Box::new(store),
        Arc::new(BodyValidator {
            body: b"{\"id\": 123456789012345678, \"username\": ".to_vec(),
        }),
        identity(),
    );
    let state = backend.login(TOKEN);
    assert_eq!(state.status, AuthStatus::Error);
    assert_eq!(state.error.as_ref().map(AuthError::kind), Some("Parse"));
    assert!(log.lock().expect("журнал").stores.is_empty());
    let text = json(&state).to_string();
    assert!(
        !text.contains(TOKEN),
        "битый ответ не тащит токен в ошибку: {text}"
    );

    // То же тело, что приходит при 401: поля `id` нет, есть `message`/`code`.
    let backend = AuthBackend::with_identity(
        Box::new(FakeStore::failing("не вызывается")),
        Arc::new(BodyValidator {
            body: UNAUTHORIZED_BODY.as_bytes().to_vec(),
        }),
        identity(),
    );
    let state = backend.login(TOKEN);
    assert_eq!(
        state.error.as_ref().map(AuthError::kind),
        Some("InvalidToken"),
        "тело 401 без `id` — недействительный токен, а не Parse"
    );

    // Успешное тело разбирается тем же путём (положительный контроль).
    let backend = AuthBackend::with_identity(
        Box::new(FakeStore::new(
            None,
            Arc::new(Mutex::new(StoreLog::default())),
        )),
        Arc::new(BodyValidator {
            body: ME_BODY.as_bytes().to_vec(),
        }),
        identity(),
    );
    assert_eq!(backend.login(TOKEN).status, AuthStatus::Ready);
}

#[test]
fn login_keyring_failure_is_reported_and_not_ready() {
    let validator = Arc::new(FakeValidator::ok());
    let backend = AuthBackend::with_identity(
        Box::new(FakeStore::failing("коллекция заблокирована")),
        validator,
        identity(),
    );

    let state = backend.login(TOKEN);
    assert_eq!(
        state.status,
        AuthStatus::Error,
        "без сохранения сессия не READY"
    );
    assert_eq!(state.error.as_ref().map(AuthError::kind), Some("Keyring"));
    assert!(!state.stored);
}

// ---------------------------------------------------------------------------
// auth_restore: сеть только по явной команде
// ---------------------------------------------------------------------------

#[test]
fn restore_reads_keyring_and_validates_stored_token() {
    let validator = Arc::new(FakeValidator::ok());
    let store = FakeStore::new(Some(TOKEN), Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = AuthBackend::with_identity(Box::new(store), validator.clone(), identity());

    assert_eq!(backend.status().status, AuthStatus::Idle);
    assert_eq!(validator.calls(), 0);

    let state = backend.restore();
    assert_eq!(state.status, AuthStatus::Ready);
    assert_eq!(validator.calls(), 1, "restore проверяет токен");
    assert!(
        log.lock().expect("журнал").stores.is_empty(),
        "восстановление не перезаписывает keyring"
    );
}

#[test]
fn restore_without_stored_token_stays_idle() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _log) = backend(
        FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default()))),
        validator.clone(),
    );

    let state = backend.restore();
    assert_eq!(state.status, AuthStatus::Idle);
    assert!(!state.stored);
    assert_eq!(validator.calls(), 0);
}

#[test]
fn restore_with_revoked_token_reports_invalid_token() {
    let validator = Arc::new(FakeValidator::err(AuthError::InvalidToken));
    let (backend, _log) = backend(
        FakeStore::new(Some(TOKEN), Arc::new(Mutex::new(StoreLog::default()))),
        validator,
    );

    let state = backend.restore();
    assert_eq!(state.status, AuthStatus::Error);
    assert_eq!(
        state.error.as_ref().map(AuthError::kind),
        Some("InvalidToken")
    );
}

// ---------------------------------------------------------------------------
// auth_logout
// ---------------------------------------------------------------------------

#[test]
fn logout_clears_keyring_and_is_idempotent() {
    let validator = Arc::new(FakeValidator::ok());
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = AuthBackend::with_identity(Box::new(store), validator, identity());

    assert_eq!(backend.login(TOKEN).status, AuthStatus::Ready);

    let first = backend.logout();
    assert_eq!(first.status, AuthStatus::Idle, "после logout — экран входа");
    assert_eq!(first.user, None);
    assert!(!first.stored);

    let second = backend.logout();
    assert_eq!(second, first, "повторный logout идемпотентен");
    assert_eq!(
        log.lock().expect("журнал").clears,
        2,
        "clear вызывается каждый раз"
    );
    assert_eq!(backend.status().status, AuthStatus::Idle);
}

#[test]
fn logout_survives_keyring_failure_and_still_wipes_session() {
    // Хранилище уже сломано: сессию всё равно гасим, а причину показываем.
    let validator = Arc::new(FakeValidator::err(AuthError::Keyring {
        message: "нет D-Bus".to_string(),
    }));
    let backend = AuthBackend::with_identity(
        Box::new(FakeStore::failing("нет D-Bus")),
        validator,
        identity(),
    );

    let state = backend.logout();
    assert_eq!(state.status, AuthStatus::Error);
    assert_eq!(state.error.as_ref().map(AuthError::kind), Some("Keyring"));
    assert_eq!(
        state.user, None,
        "сессия погашена несмотря на ошибку keyring"
    );
    assert!(!state.stored);
}

// ---------------------------------------------------------------------------
// Поток-владелец keyring (`Keyring` — `!Send`)
// ---------------------------------------------------------------------------

#[test]
fn threaded_store_owns_store_in_dedicated_thread() {
    let log = Arc::new(Mutex::new(StoreLog::default()));
    let factory_log = Arc::clone(&log);
    let mut store = ThreadedStore::spawn(move || {
        Ok(Box::new(FakeStore::new(Some(TOKEN), factory_log)) as Box<dyn TokenStore>)
    });

    // Все три операции идут через канал в поток-владелец.
    assert_eq!(store.load().expect("load"), Some(TOKEN.to_string()));
    store.store(TOKEN2).expect("store");
    assert_eq!(
        store.load().expect("load после store"),
        Some(TOKEN2.to_string())
    );
    assert!(store.clear().expect("clear"), "запись была");
    assert_eq!(store.load().expect("load после clear"), None);

    let operations = {
        let log = log.lock().expect("журнал");
        (log.loads, log.stores.clone(), log.clears)
    };
    assert_eq!(operations.0, 3);
    assert_eq!(operations.1, vec![TOKEN2.to_string()]);
    assert_eq!(operations.2, 1);
}

#[test]
fn threaded_store_propagates_open_failure() {
    let mut store = ThreadedStore::spawn(|| Err("Secret Service недоступен".to_string()));
    let error = store.load().expect_err("открытие хранилища не удалось");
    assert!(
        error.contains("Secret Service"),
        "причина сохранена: {error}"
    );
    assert!(
        store.store(TOKEN).is_err(),
        "поток мёртв — ошибка, а не зависание"
    );
}

/// Хранилище, которое «висит»: операция не возвращается, пока тест не отпустит её.
/// Модель заблокированной коллекции Secret Service — без D-Bus и без окна.
struct BlockingStore {
    released: Arc<(Mutex<bool>, Condvar)>,
}

impl BlockingStore {
    fn new(released: Arc<(Mutex<bool>, Condvar)>) -> Self {
        Self { released }
    }

    fn wait(&self) {
        let (lock, cvar) = &*self.released;
        let mut released = lock.lock().expect("флаг освобождения");
        while !*released {
            released = cvar.wait(released).expect("ожидание освобождения");
        }
    }
}

impl TokenStore for BlockingStore {
    fn load(&mut self) -> Result<Option<String>, String> {
        self.wait();
        Ok(None)
    }

    fn store(&mut self, _token: &str) -> Result<(), String> {
        self.wait();
        Ok(())
    }

    fn clear(&mut self) -> Result<bool, String> {
        self.wait();
        Ok(false)
    }
}

#[test]
fn threaded_store_times_out_instead_of_waiting_forever() {
    // SEC-01: зависший keyring не должен блокировать вызывающего навсегда.
    use std::time::{Duration, Instant};

    let released = Arc::new((Mutex::new(false), Condvar::new()));
    let factory_released = Arc::clone(&released);
    let mut store = ThreadedStore::spawn(move || {
        Ok(Box::new(BlockingStore::new(factory_released)) as Box<dyn TokenStore>)
    })
    .with_timeout(Duration::from_millis(50));

    let started = Instant::now();
    let error = store
        .load()
        .expect_err("зависший keyring — ошибка, а не ожидание");
    let elapsed = started.elapsed();

    assert!(
        error.contains("не ответил"),
        "причина таймаута видна: {error}"
    );
    assert!(
        !error.contains(TOKEN),
        "в тексте ошибки нет секрета: {error}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "вызов вернулся по таймауту, а не по разблокировке: {elapsed:?}"
    );

    // Отпускаем поток-владелец, иначе его `join` в `Drop` ждал бы вечно.
    {
        let (lock, cvar) = &*released;
        *lock.lock().expect("флаг освобождения") = true;
        cvar.notify_all();
    }
}

// ---------------------------------------------------------------------------
// Маппинг ошибок ядра: 401/429/транспорт/битый JSON, без токена в тексте
// ---------------------------------------------------------------------------

#[test]
fn core_errors_map_to_typed_auth_errors() {
    use chord_core::rest::client::ApiError;
    use chord_core::rest::transport::HttpError;
    use std::time::Duration;

    assert_eq!(
        auth::map_api_error(&ApiError::Unauthorized, TOKEN),
        AuthError::InvalidToken
    );
    assert_eq!(
        auth::map_api_error(
            &ApiError::RateLimited {
                retry_after: Duration::from_millis(1_500),
                attempts: 3,
            },
            TOKEN,
        ),
        AuthError::RateLimited {
            retry_after_ms: 1_500
        }
    );
    let network = auth::map_api_error(
        &ApiError::Transport(HttpError::Connect(format!("отказ, токен {TOKEN}"))),
        TOKEN,
    );
    assert_eq!(network.kind(), "Network");
    assert!(
        !format!("{network}").contains(TOKEN),
        "транспортная ошибка не печатает токен: {network}"
    );

    // Битый JSON и тело ошибки Discord — через настоящий парсер ядра.
    let parse = chord_core::parse_me(b"{\"id\": ").expect_err("битый JSON");
    assert_eq!(auth::map_me_error(&parse, TOKEN).kind(), "Parse");
    let unauthorized = chord_core::parse_me(UNAUTHORIZED_BODY.as_bytes())
        .expect_err("тело 401 не является пользователем");
    assert_eq!(
        auth::map_me_error(&unauthorized, TOKEN),
        AuthError::InvalidToken
    );
}

#[test]
fn identity_context_is_complete_for_auth_headers() {
    let identity = IdentityContext::new("ru", "Europe/Moscow");
    assert_eq!(identity.locale, "ru");
    assert_eq!(identity.timezone, "Europe/Moscow");
    // 22 поля супер-свойств: `X-Super-Properties` не должен быть пустым.
    assert!(!identity.super_properties.to_base64().is_empty());
    assert_eq!(identity.super_properties.client_version, "1.0.162");
    assert_eq!(identity.super_properties.browser_version, "42.11.12");
    assert_eq!(identity.super_properties.client_build_number, 641_004);
    assert_eq!(identity.super_properties.native_build_number, 94_186);
    assert_eq!(
        identity.super_properties.browser_user_agent,
        chord_core::rest::http1::DEFAULT_USER_AGENT
    );

    // Реальная identity читает среду, но не сеть и не keyring, и кэширует UUID на процесс.
    let detected = IdentityContext::detect();
    let detected_again = IdentityContext::detect();
    assert!(!detected.locale.trim().is_empty());
    assert!(!detected.timezone.trim().is_empty());
    assert_eq!(
        detected.super_properties.client_launch_id,
        detected_again.super_properties.client_launch_id,
        "client_launch_id обязан быть единым на весь процесс (защита от антифрода Discord)"
    );
    assert_eq!(
        detected.super_properties.launch_signature,
        detected_again.super_properties.launch_signature,
        "launch_signature обязан быть единым на весь процесс"
    );
    assert_eq!(
        detected.super_properties.client_heartbeat_session_id,
        detected_again.super_properties.client_heartbeat_session_id,
        "client_heartbeat_session_id обязан быть единым на весь процесс"
    );
    assert_eq!(
        identity.super_properties.client_launch_id,
        detected.super_properties.client_launch_id
    );
}

// ---------------------------------------------------------------------------
// Регистрация Tauri-команд
// ---------------------------------------------------------------------------

#[test]
fn invoke_handler_registers_auth_commands() {
    let source = include_str!("../src/main.rs");
    const OPEN: &str = "generate_handler![";
    let start = source
        .find(OPEN)
        .expect("main.rs объявляет invoke_handler!")
        + OPEN.len();
    let rest = &source[start..];
    let block = &rest[..rest.find(']').expect("invoke_handler! без ]")];
    for command in [
        "auth_status",
        "auth_login_token",
        "auth_logout",
        "auth_restore",
    ] {
        assert!(
            block
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .any(|name| name == command),
            "{command} не зарегистрирован в invoke_handler!"
        );
    }
}

// ---------------------------------------------------------------------------
// RW-39 / NET-F1: вход паролем, MFA и SMS (NF-01…NF-06)
// Всё через fake transport: ни окна, ни сети, ни keyring, ни captcha-обхода.
// ---------------------------------------------------------------------------

/// Пароль-маркер: его не должно быть ни в Debug, ни в ошибках, ни в JSON состояния.
const PASSWORD: &str = "P@ssw0rd-netf1-secret";
/// 2FA-код-маркер (тот же инвариант, что у пароля).
const MFA_CODE: &str = "654321";

/// Один зафиксированный вызов транспорта: путь, тело и заголовки как есть.
#[derive(Clone, Debug)]
struct LoginCall {
    path: String,
    body: Vec<u8>,
    headers: Vec<(String, String)>,
}

/// Фейковый транспорт `/auth/*`: очередь ответов + журнал вызовов.
struct FakeLogin {
    responses: Mutex<VecDeque<Result<HttpResponse, ApiError>>>,
    calls: Arc<Mutex<Vec<LoginCall>>>,
}

impl FakeLogin {
    fn new(
        responses: Vec<Result<HttpResponse, ApiError>>,
    ) -> (Arc<Self>, Arc<Mutex<Vec<LoginCall>>>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        (
            Arc::new(Self {
                responses: Mutex::new(responses.into()),
                calls: Arc::clone(&calls),
            }),
            calls,
        )
    }
}

impl LoginTransport for FakeLogin {
    fn post_json(
        &self,
        path: &str,
        body: &[u8],
        headers: &[(&str, &str)],
    ) -> Result<HttpResponse, ApiError> {
        self.calls
            .lock()
            .expect("журнал транспорта")
            .push(LoginCall {
                path: path.to_string(),
                body: body.to_vec(),
                headers: headers
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                    .collect(),
            });
        self.responses
            .lock()
            .expect("очередь ответов")
            .pop_front()
            .expect("fake transport: ответ не задан")
    }
}

/// HTTP-ответ как данные (без сети).
fn http(status: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: body.as_bytes().to_vec(),
    }
}

/// 429 с `Retry-After` в секундах — как отдаёт Discord (RFC 6585 §4).
fn http_429(seconds: &str) -> HttpResponse {
    HttpResponse {
        status: 429,
        headers: vec![("Retry-After".to_string(), seconds.to_string())],
        body: b"{}".to_vec(),
    }
}

/// Backend + журнал хранилища + журнал транспорта (см. `login_backend`).
type LoginFixture = (
    AuthBackend,
    Arc<Mutex<StoreLog>>,
    Arc<Mutex<Vec<LoginCall>>>,
);

/// Backend с фейковым транспортом и фейковым хранилищем; журнал хранилища — снаружи.
fn login_backend(
    responses: Vec<Result<HttpResponse, ApiError>>,
    validator: Arc<FakeValidator>,
) -> LoginFixture {
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let store_log = Arc::clone(&store.log);
    let (transport, calls) = FakeLogin::new(responses);
    let backend = AuthBackend::with_identity(Box::new(store), validator, identity())
        .with_login_transport(transport);
    (backend, store_log, calls)
}

fn body_json(call: &LoginCall) -> Value {
    serde_json::from_slice(&call.body).expect("тело запроса — JSON")
}

fn contains_header(call: &LoginCall, name: &str, value: &str) -> bool {
    call.headers
        .iter()
        .any(|(key, actual)| key.eq_ignore_ascii_case(name) && actual == value)
}

// --- успех: 200 + token → keyring + Ready, токена нет в ответе UI -------------

#[test]
fn login_password_success_stores_token_and_hides_it_from_view() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, store_log, calls) = login_backend(
        vec![Ok(http(200, &format!(r#"{{"token":"{TOKEN}"}}"#)))],
        validator.clone(),
    );

    let outcome = backend.login_password(
        "user@example.test",
        PASSWORD.to_string(),
        None,
        None,
        None,
        None,
    );
    assert_eq!(outcome, LoginOutcome::Token(TOKEN.to_string()));
    assert_eq!(validator.calls(), 1, "токен проверяется GET /users/@me");
    assert_eq!(
        store_log.lock().expect("журнал").stores,
        vec![TOKEN.to_string()],
        "токен лёг в keyring"
    );
    assert_eq!(backend.status().status, AuthStatus::Ready);

    let view = LoginView::from_outcome(outcome, &backend.status()).expect("готовое состояние");
    let json = serde_json::to_string(&view).expect("LoginView сериализуется");
    assert!(!json.contains(TOKEN), "токен не уходит во фронтенд: {json}");
    assert!(json.contains("tester"), "UI получает пользователя: {json}");

    let calls = calls.lock().expect("журнал транспорта");
    assert_eq!(calls.len(), 1, "ровно один POST логина");
    assert_eq!(calls[0].path, LOGIN_PATH);
    let body = body_json(&calls[0]);
    assert_eq!(body["login"], "user@example.test");
    assert_eq!(
        body["password"], PASSWORD,
        "пароль реально уходит в /auth/login"
    );
    assert_eq!(body["undelete"], false);
    assert!(body.get("login_source").is_some(), "поле A-02 присутствует");
    assert!(
        body.get("gift_code_sku_id").is_some(),
        "поле A-02 присутствует"
    );
    assert!(
        contains_header(&calls[0], "authorization", ""),
        "A-03: явный пустой authorization, без унаследованного токена"
    );
    assert!(
        !format!("{backend:?}").contains(PASSWORD),
        "backend не хранит и не печатает пароль"
    );
}

// --- 401: неверный логин/пароль, без секретов и без повторов -----------------

#[test]
fn login_password_401_is_invalid_credentials_without_secrets() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, store_log, calls) = login_backend(
        vec![Ok(http(401, r#"{"message":"401: Unauthorized","code":0}"#))],
        validator.clone(),
    );

    let outcome = backend.login_password(
        "user@example.test",
        PASSWORD.to_string(),
        None,
        None,
        None,
        None,
    );
    let LoginOutcome::Error(error) = &outcome else {
        panic!("401 обязан быть ошибкой: {outcome:?}");
    };
    assert_eq!(error.kind(), "InvalidCredentials");
    for text in [
        format!("{outcome:?}"),
        format!("{error}"),
        format!("{error:?}"),
    ] {
        assert!(!text.contains(PASSWORD), "пароль утёк: {text}");
    }
    assert_eq!(validator.calls(), 0, "неверный вход не проверяет токен");
    assert!(store_log.lock().expect("журнал").stores.is_empty());
    assert_eq!(
        calls.lock().expect("журнал транспорта").len(),
        1,
        "401 не повторяется"
    );
}

// --- 429: retry_after и никакой «долбёжки» -----------------------------------

#[test]
fn login_password_429_reports_retry_after_without_retry_loop() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _store_log, calls) = login_backend(vec![Ok(http_429("3.5"))], validator);

    let outcome = backend.login_password("u", PASSWORD.to_string(), None, None, None, None);
    assert_eq!(
        outcome,
        LoginOutcome::Error(AuthError::RateLimited {
            retry_after_ms: 3_500
        })
    );
    assert_eq!(
        calls.lock().expect("журнал транспорта").len(),
        1,
        "ADR-3: логин не повторяется автоматически"
    );
}

// --- captcha: контекст для штатного виджета, без solver ----------------------

#[test]
fn login_password_captcha_returns_widget_context() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, store_log, calls) = login_backend(
        vec![Ok(http(
            400,
            r#"{"captcha_key":"site-key-123","captcha_service":"hcaptcha","captcha_session_id":"session-1","captcha_rqdata":"rqdata-1","captcha_rqtoken":"rq-1","should_serve_invisible":true,"user_flow":"login"}"#,
        ))],
        validator.clone(),
    );

    let outcome = backend.login_password("u", PASSWORD.to_string(), None, None, None, None);
    assert_eq!(
        outcome,
        LoginOutcome::Captcha {
            sitekey: "site-key-123".to_string(),
            service: "hcaptcha".to_string(),
            captcha_session_id: Some("session-1".to_string()),
            captcha_rqdata: Some("rqdata-1".to_string()),
            captcha_rqtoken: Some("rq-1".to_string()),
            should_serve_invisible: true,
            user_flow: Some("login".to_string()),
        }
    );
    assert_eq!(validator.calls(), 0, "captcha — не успешный вход");
    assert!(store_log.lock().expect("журнал").stores.is_empty());
    assert_eq!(
        calls.lock().expect("журнал транспорта").len(),
        1,
        "captcha не решается и не повторяется автоматически"
    );
    let text = format!("{outcome:?}");
    assert!(
        !text.contains("rq-1"),
        "rqtoken не печатается в Debug: {text}"
    );
    assert!(
        !text.contains("session-1"),
        "session id не печатается в Debug: {text}"
    );
    assert!(
        !text.contains("rqdata-1"),
        "rqdata не печатается в Debug: {text}"
    );

    let view = LoginView::from_outcome(outcome, &backend.status()).expect("captcha view");
    let json = serde_json::to_string(&view).expect("captcha view сериализуется");
    for field in [
        "site-key-123",
        "hcaptcha",
        "session-1",
        "rqdata-1",
        "rq-1",
        "should_serve_invisible",
        "user_flow",
    ] {
        assert!(json.contains(field), "captcha view теряет {field}: {json}");
    }
}

#[test]
fn login_password_with_captcha_answer_sends_captcha_headers() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _store_log, calls) = login_backend(
        vec![Ok(http(200, &format!(r#"{{"token":"{TOKEN}"}}"#)))],
        validator,
    );

    let outcome = backend.login_password(
        "u",
        PASSWORD.to_string(),
        Some("captcha-answer"),
        Some("rq-1"),
        Some("session-1"),
        Some("rqdata-1"),
    );
    assert_eq!(outcome, LoginOutcome::Token(TOKEN.to_string()));

    let calls = calls.lock().expect("журнал транспорта");
    assert!(contains_header(
        &calls[0],
        CAPTCHA_KEY_HEADER,
        "captcha-answer"
    ));
    assert!(contains_header(&calls[0], CAPTCHA_RQTOKEN_HEADER, "rq-1"));
    assert!(contains_header(
        &calls[0],
        CAPTCHA_SESSION_ID_HEADER,
        "session-1"
    ));
    assert!(contains_header(
        &calls[0],
        CAPTCHA_RQDATA_HEADER,
        "rqdata-1"
    ));
}

// --- MFA: состояние, ticket и методы -----------------------------------------

#[test]
fn login_password_mfa_returns_ticket_and_methods() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, store_log, calls) = login_backend(
        vec![Ok(http(
            200,
            r#"{"mfa":true,"ticket":"ticket-1","totp":true,"sms":true,"login_instance_id":"iid-1"}"#,
        ))],
        validator.clone(),
    );

    let outcome = backend.login_password("u", PASSWORD.to_string(), None, None, None, None);
    assert_eq!(
        outcome,
        LoginOutcome::Mfa {
            ticket: "ticket-1".to_string(),
            methods: vec!["totp".to_string(), "sms".to_string()],
            login_instance_id: Some("iid-1".to_string()),
        }
    );
    assert_eq!(validator.calls(), 0, "MFA — промежуточный шаг, не Ready");
    assert!(store_log.lock().expect("журнал").stores.is_empty());
    assert_eq!(calls.lock().expect("журнал транспорта").len(), 1);
    assert!(
        !format!("{outcome:?}").contains("ticket-1"),
        "ticket — секрет сессии, Debug его маскирует"
    );
}

// --- submit_mfa: успех, неверный код, неизвестный метод ----------------------

#[test]
fn submit_mfa_success_stores_token_and_sends_code_once() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, store_log, calls) = login_backend(
        vec![Ok(http(200, &format!(r#"{{"token":"{TOKEN}"}}"#)))],
        validator.clone(),
    );

    let outcome = backend.submit_mfa("totp", MFA_CODE.to_string(), "ticket-1", Some("iid-1"));
    assert_eq!(outcome, LoginOutcome::Token(TOKEN.to_string()));
    assert_eq!(validator.calls(), 1);
    assert_eq!(
        store_log.lock().expect("журнал").stores,
        vec![TOKEN.to_string()]
    );

    let calls = calls.lock().expect("журнал транспорта");
    assert_eq!(calls.len(), 1, "ровно один POST MFA");
    assert_eq!(calls[0].path, "/auth/mfa/totp");
    let body = body_json(&calls[0]);
    assert_eq!(body["code"], MFA_CODE);
    assert_eq!(body["ticket"], "ticket-1");
    assert_eq!(body["login_instance_id"], "iid-1");
    assert!(contains_header(&calls[0], "authorization", ""));
    assert!(!format!("{backend:?}").contains(MFA_CODE));
}

#[test]
fn submit_mfa_wrong_code_is_rejected_without_echoing_code() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, store_log, calls) = login_backend(
        vec![Ok(http(
            401,
            r#"{"message":"Invalid two-factor code","code":60008}"#,
        ))],
        validator,
    );

    let outcome = backend.submit_mfa("totp", MFA_CODE.to_string(), "ticket-1", None);
    let LoginOutcome::Error(error) = &outcome else {
        panic!("неверный код обязан быть ошибкой: {outcome:?}");
    };
    assert_eq!(
        error.kind(),
        "MfaRejected",
        "60008 MFA_INVALID_CODE — отказ кода: {error:?}"
    );
    for text in [format!("{outcome:?}"), format!("{error}")] {
        assert!(!text.contains(MFA_CODE), "2FA-код утёк в текст: {text}");
    }
    assert!(store_log.lock().expect("журнал").stores.is_empty());
    assert_eq!(
        calls.lock().expect("журнал транспорта").len(),
        1,
        "неверный код не повторяется"
    );
}

#[test]
fn submit_mfa_rejects_unknown_method_before_network() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _store_log, calls) = login_backend(vec![], validator);

    let outcome = backend.submit_mfa("../logout", MFA_CODE.to_string(), "ticket-1", None);
    let LoginOutcome::Error(error) = &outcome else {
        panic!("метод вне whitelist обязан быть ошибкой");
    };
    assert_eq!(error.kind(), "Internal");
    assert!(!format!("{error}").contains(MFA_CODE));
    assert!(
        calls.lock().expect("журнал транспорта").is_empty(),
        "неизвестный метод не уходит в путь /auth/mfa/…"
    );
}

// --- request_sms -------------------------------------------------------------

#[test]
fn request_sms_sends_ticket_and_returns_masked_phone() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _store_log, calls) =
        login_backend(vec![Ok(http(200, r#"{"phone":"+1 *** 1234"}"#))], validator);

    let challenge = backend.request_sms("ticket-1").expect("SMS отправлена");
    assert_eq!(
        challenge,
        SmsChallenge {
            phone: Some("+1 *** 1234".to_string())
        }
    );

    let calls = calls.lock().expect("журнал транспорта");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].path, MFA_SMS_SEND_PATH);
    let body = body_json(&calls[0]);
    assert_eq!(body["ticket"], "ticket-1");
    assert_eq!(
        body.as_object().map(serde_json::Map::len),
        Some(1),
        "тело — только ticket: {body}"
    );
}

#[test]
fn request_sms_429_is_rate_limited_without_retry() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _store_log, calls) = login_backend(vec![Ok(http_429("2"))], validator);

    let error = backend.request_sms("ticket-1").expect_err("429 — ошибка");
    assert_eq!(
        error,
        AuthError::RateLimited {
            retry_after_ms: 2_000
        }
    );
    assert_eq!(calls.lock().expect("журнал транспорта").len(), 1);
}

// --- пустые входы, сеть и маскировка Debug ----------------------------------

#[test]
fn empty_credentials_never_reach_transport() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _store_log, calls) = login_backend(vec![], validator);

    assert_eq!(
        backend.login_password("   ", PASSWORD.to_string(), None, None, None, None),
        LoginOutcome::Error(AuthError::EmptyCredentials)
    );
    assert_eq!(
        backend.login_password("u", String::new(), None, None, None, None),
        LoginOutcome::Error(AuthError::EmptyCredentials)
    );
    assert!(calls.lock().expect("журнал транспорта").is_empty());
}

#[test]
fn login_network_failure_is_network_and_not_retried() {
    let validator = Arc::new(FakeValidator::ok());
    let (backend, _store_log, calls) = login_backend(
        vec![Err(ApiError::Transport(HttpError::Connect(
            "connect refused".to_string(),
        )))],
        validator,
    );

    let outcome = backend.login_password("u", PASSWORD.to_string(), None, None, None, None);
    let LoginOutcome::Error(error) = &outcome else {
        panic!("сеть — ошибка");
    };
    assert_eq!(error.kind(), "Network");
    assert_eq!(calls.lock().expect("журнал транспорта").len(), 1);
}

#[test]
fn login_outcome_debug_redacts_token_ticket_and_captcha_rqtoken() {
    let token = format!("{:?}", LoginOutcome::Token(TOKEN.to_string()));
    assert!(!token.contains(TOKEN), "токен в Debug: {token}");
    assert!(token.contains("<redacted>"), "ожидалась маска: {token}");

    let mfa = format!(
        "{:?}",
        LoginOutcome::Mfa {
            ticket: "ticket-secret".to_string(),
            methods: vec!["totp".to_string()],
            login_instance_id: None,
        }
    );
    assert!(!mfa.contains("ticket-secret"), "ticket в Debug: {mfa}");

    let captcha = format!(
        "{:?}",
        LoginOutcome::Captcha {
            sitekey: "site".to_string(),
            service: "hcaptcha".to_string(),
            captcha_session_id: Some("session-secret".to_string()),
            captcha_rqdata: Some("rqdata-secret".to_string()),
            captcha_rqtoken: Some("rq-secret".to_string()),
            should_serve_invisible: false,
            user_flow: Some("login".to_string()),
        }
    );
    assert!(!captcha.contains("rq-secret"), "rqtoken в Debug: {captcha}");
}

// --- регистрация Tauri-команд -----------------------------------------------

#[test]
fn invoke_handler_registers_password_login_commands() {
    let source = include_str!("../src/main.rs");
    const OPEN: &str = "generate_handler![";
    let start = source
        .find(OPEN)
        .expect("main.rs объявляет invoke_handler!")
        + OPEN.len();
    let rest = &source[start..];
    let block = &rest[..rest.find(']').expect("invoke_handler! без ]")];
    for command in ["auth_login_password", "auth_mfa", "auth_sms"] {
        assert!(
            block
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .any(|name| name == command),
            "{command} не зарегистрирован в invoke_handler!"
        );
    }
}

// ---------------------------------------------------------------------------
// PLAT-01 / G1-03: мьютекс состояния не удерживается через сеть и keyring I/O
// ---------------------------------------------------------------------------

/// Валидатор с «замком»: сообщает о входе в `validate` и висит, пока тест не
/// отпустит его. Модель медленного `/users/@me` без сети и без sleep-гонок.
struct GatedValidator {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    result: Result<Me, AuthError>,
}

impl GatedValidator {
    fn new(result: Result<Me, AuthError>) -> (Arc<Self>, mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let validator = Arc::new(Self {
            entered: entered_tx,
            release: Mutex::new(release_rx),
            result,
        });
        (validator, entered_rx, release_tx)
    }
}

impl TokenValidator for GatedValidator {
    fn validate(&self, _request: &ValidationRequest) -> Result<Me, AuthError> {
        let _ = self.entered.send(());
        // Таймаут — страховка от зависшего теста, а не часть контракта.
        let released = self
            .release
            .lock()
            .expect("замок валидатора")
            .recv_timeout(Duration::from_secs(10));
        assert!(released.is_ok(), "тест обязан отпустить валидатор");
        self.result.clone()
    }
}

/// Backend с любым fake-валидатором (тип стирается до `dyn TokenValidator`).
fn backend_any(
    store: FakeStore,
    validator: Arc<dyn TokenValidator>,
) -> (AuthBackend, Arc<Mutex<StoreLog>>) {
    let log = Arc::clone(&store.log);
    (
        AuthBackend::with_identity(Box::new(store), validator, identity()),
        log,
    )
}

#[test]
fn blocked_validation_does_not_block_status_or_token() {
    // Во время `login` (сеть висит) IPC `status`/`token` обязаны отвечать, а не
    // стоять на мьютексе состояния (PLAT-01).
    let (validator, entered_rx, release_tx) = GatedValidator::new(Ok(me()));
    let (backend, _log) = backend_any(
        FakeStore::new(Some(TOKEN), Arc::new(Mutex::new(StoreLog::default()))),
        validator,
    );
    let backend = Arc::new(backend);

    let login_backend = Arc::clone(&backend);
    let login = thread::spawn(move || login_backend.login(TOKEN2));
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("валидатор должен войти в validate");

    let observer_backend = Arc::clone(&backend);
    let (observed_tx, observed_rx) = mpsc::channel();
    let observer = thread::spawn(move || {
        let status_started = Instant::now();
        let state = observer_backend.status();
        let status_elapsed = status_started.elapsed();
        let token_started = Instant::now();
        let token = observer_backend.token();
        let token_elapsed = token_started.elapsed();
        let _ = observed_tx.send((state, status_elapsed, token, token_elapsed));
    });

    let observed = observed_rx.recv_timeout(Duration::from_secs(2));
    // Отпускаем валидатор в любом исходе: red-прогон не должен висеть.
    let _ = release_tx.send(());
    let (state, status_elapsed, token, token_elapsed) =
        observed.expect("status()/token() ждут мьютекс, удерживаемый сетевым login");
    observer.join().expect("наблюдатель");

    assert!(
        status_elapsed < Duration::from_secs(1),
        "status() не должен ждать сетевой login: {status_elapsed:?}"
    );
    assert!(
        token_elapsed < Duration::from_secs(1),
        "token() не должен ждать сетевой login: {token_elapsed:?}"
    );
    assert_eq!(
        state.status,
        AuthStatus::Validating,
        "статус честно показывает идущую проверку"
    );
    assert_eq!(token.expect("keyring"), Some(TOKEN.to_string()));
    assert_eq!(
        login.join().expect("поток login").status,
        AuthStatus::Ready,
        "после разблокировки проверка завершается"
    );
}

#[test]
fn logout_during_blocked_validation_wins_and_token_is_not_persisted() {
    // Пока `/users/@me` висит, пользователь выходит. Logout не должен ждать сеть,
    // а поздний ответ login не должен воскресить сессию или записать токен.
    let (validator, entered_rx, release_tx) = GatedValidator::new(Ok(me()));
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = Arc::new(AuthBackend::with_identity(
        Box::new(store),
        validator,
        identity(),
    ));

    let login_backend = Arc::clone(&backend);
    let login = thread::spawn(move || login_backend.login(TOKEN));
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("валидатор должен войти в validate");

    let logout_backend = Arc::clone(&backend);
    let (logout_tx, logout_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = logout_tx.send(logout_backend.logout());
    });
    let logout_state = logout_rx.recv_timeout(Duration::from_secs(2));
    let _ = release_tx.send(());
    let logout_state = logout_state.expect("logout ждёт сетевую валидацию под мьютексом");
    let login_state = login.join().expect("поток login");

    assert_eq!(logout_state.status, AuthStatus::Idle);
    assert!(!logout_state.stored);
    assert_eq!(
        login_state.status,
        AuthStatus::Idle,
        "поздний login не перезаписывает logout"
    );
    assert!(
        log.lock().expect("журнал").stores.is_empty(),
        "токен отменённого login не попадает в keyring"
    );
    assert_eq!(backend.status().status, AuthStatus::Idle);
    assert!(backend.token().expect("keyring").is_none());
}

/// Хранилище с задержкой: первый `load` сообщает о входе и висит до release
/// (последующие чтения проходят сразу). Модель медленного Secret Service без D-Bus.
struct GatedStore {
    token: Mutex<Option<String>>,
    armed: std::sync::atomic::AtomicBool,
    entered: mpsc::Sender<&'static str>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl GatedStore {
    fn new(token: Option<&str>) -> (Self, mpsc::Receiver<&'static str>, mpsc::Sender<()>) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        (
            Self {
                token: Mutex::new(token.map(str::to_string)),
                armed: std::sync::atomic::AtomicBool::new(true),
                entered: entered_tx,
                release: Mutex::new(release_rx),
            },
            entered_rx,
            release_tx,
        )
    }

    fn wait(&self, operation: &'static str) {
        if !self.armed.swap(false, Ordering::SeqCst) {
            return;
        }
        let _ = self.entered.send(operation);
        let released = self
            .release
            .lock()
            .expect("замок хранилища")
            .recv_timeout(Duration::from_secs(10));
        assert!(released.is_ok(), "тест обязан отпустить хранилище");
    }
}

impl TokenStore for GatedStore {
    fn load(&mut self) -> Result<Option<String>, String> {
        self.wait("load");
        Ok(self.token.lock().expect("токен хранилища").clone())
    }

    fn store(&mut self, token: &str) -> Result<(), String> {
        *self.token.lock().expect("токен хранилища") = Some(token.to_string());
        Ok(())
    }

    fn clear(&mut self) -> Result<bool, String> {
        Ok(self.token.lock().expect("токен хранилища").take().is_some())
    }
}

#[test]
fn status_read_does_not_resurrect_state_after_logout() {
    // `status` читает keyring вне мьютекса состояния и коммита, но его поздний
    // результат не должен вернуть `stored=true` после logout: итог детерминирован.
    let (store, entered_rx, release_tx) = GatedStore::new(Some(TOKEN));
    let backend = Arc::new(AuthBackend::with_identity(
        Box::new(store),
        Arc::new(FakeValidator::ok()),
        identity(),
    ));

    let status_backend = Arc::clone(&backend);
    let status = thread::spawn(move || status_backend.status());
    assert_eq!(
        entered_rx.recv_timeout(Duration::from_secs(5)),
        Ok("load"),
        "status читает keyring"
    );

    let logout_backend = Arc::clone(&backend);
    let logout = thread::spawn(move || logout_backend.logout());
    release_tx.send(()).expect("отпустить keyring.load");
    let status_state = status.join().expect("поток status");
    let logout_state = logout.join().expect("поток logout");

    assert_eq!(status_state.status, AuthStatus::Idle);
    assert_eq!(logout_state.status, AuthStatus::Idle);
    assert!(!logout_state.stored);

    let final_state = backend.status();
    assert_eq!(final_state.status, AuthStatus::Idle);
    assert!(
        !final_state.stored,
        "поздний status не воскрешает stored после logout"
    );
    assert!(backend.token().expect("keyring").is_none());
}

/// Валидатор на два вызова с независимыми «замками»: моделирует два
/// параллельных login, каждый из которых висит в своей сетевой проверке.
struct TwoGateValidator {
    calls: AtomicUsize,
    first_entered: mpsc::Sender<()>,
    first_release: Mutex<mpsc::Receiver<()>>,
    second_entered: mpsc::Sender<()>,
    second_release: Mutex<mpsc::Receiver<()>>,
}

impl TokenValidator for TwoGateValidator {
    fn validate(&self, _request: &ValidationRequest) -> Result<Me, AuthError> {
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        let (entered, release) = if index == 0 {
            (&self.first_entered, &self.first_release)
        } else {
            (&self.second_entered, &self.second_release)
        };
        let _ = entered.send(());
        let released = release
            .lock()
            .expect("замок валидатора")
            .recv_timeout(Duration::from_secs(10));
        assert!(released.is_ok(), "тест обязан отпустить валидатор");
        Ok(me())
    }
}

#[test]
fn concurrent_logins_commit_only_the_newest_generation() {
    // Два параллельных login на одном backend: коммитится только начавшийся
    // позже; устаревшая проверка не перезаписывает сессию и keyring.
    let (first_entered_tx, first_entered_rx) = mpsc::channel();
    let (first_release_tx, first_release_rx) = mpsc::channel();
    let (second_entered_tx, second_entered_rx) = mpsc::channel();
    let (second_release_tx, second_release_rx) = mpsc::channel();
    let validator = Arc::new(TwoGateValidator {
        calls: AtomicUsize::new(0),
        first_entered: first_entered_tx,
        first_release: Mutex::new(first_release_rx),
        second_entered: second_entered_tx,
        second_release: Mutex::new(second_release_rx),
    });
    let store = FakeStore::new(None, Arc::new(Mutex::new(StoreLog::default())));
    let log = Arc::clone(&store.log);
    let backend = Arc::new(AuthBackend::with_identity(
        Box::new(store),
        validator,
        identity(),
    ));

    let login_a = {
        let backend = Arc::clone(&backend);
        let handle = thread::spawn(move || backend.login(TOKEN));
        first_entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("login A вошёл в validate");
        handle
    };
    // Второй login стартует позже и получает новое поколение.
    let login_b = {
        let backend = Arc::clone(&backend);
        let handle = thread::spawn(move || backend.login(TOKEN2));
        let entered = second_entered_rx.recv_timeout(Duration::from_secs(5));
        if entered.is_err() {
            // Red-прогон: второй login не дошёл до сети — отпускаем первый,
            // чтобы тестовый процесс не ждал страховочный таймаут.
            let _ = first_release_tx.send(());
        }
        entered.expect("login B вошёл в validate");
        handle
    };

    second_release_tx.send(()).expect("отпустить login B");
    let state_b = login_b.join().expect("поток login B");
    // Устаревшая проверка завершается позже победителя и не должна коммититься.
    first_release_tx.send(()).expect("отпустить login A");
    let state_a = login_a.join().expect("поток login A");

    assert_eq!(state_b.status, AuthStatus::Ready);
    assert_eq!(
        log.lock().expect("журнал").stores,
        vec![TOKEN2.to_string()],
        "сохраняется только токен победившего login"
    );
    assert_eq!(
        backend.token().expect("keyring"),
        Some(TOKEN2.to_string()),
        "устаревший login не перезаписал keyring"
    );
    assert_eq!(
        state_a.status,
        AuthStatus::Ready,
        "устаревшая операция видит актуальное состояние победителя"
    );
    assert_eq!(
        backend.status().status,
        AuthStatus::Ready,
        "итоговое состояние — сессия победителя"
    );
}
