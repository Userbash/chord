//! Таймауты и ретраи REST-запросов (T-REST-03, волна RW-08 / P1-07) на локальном стабе
//! `127.0.0.1` — сеть наружу не ходит.
//!
//! **Провенанс оракулов** (независимые источники; тест не подгоняется под реализацию):
//! - контракт §6.1 ТЗ `waves/RW-08/TZ.md`: типы/сигнатуры заморожены координатором до старта
//!   волны; семантика — «не-идемпотентный — ровно 1 попытка; идемпотентный — до `max_attempts`
//!   (0 → 1) с `sleep(backoff)` между попытками»;
//! - идемпотентность — RFC 9110 §9.2.2: GET/PUT/DELETE идемпотентны, POST/PATCH — нет;
//! - стаб-сервер — §6.2 ТЗ: `127.0.0.1:0`, режимы `Always200`/`Hang`/`HangFirst`/`CloseFirst`,
//!   счётчик запросов после чтения до `\r\n\r\n`, задачи гасятся в `Drop` (`abort`);
//! - тайминги — маленькие и только нижние границы (`elapsed >= attempt_timeout`): верхних
//!   границ нет — медленная машина не должна валить кейс. `CASE_TIMEOUT` — страховка от
//!   зависания: если политика не применит таймаут, кейс упадёт, а не повиснет.
//!
//! Кейсы: N1–N6 — политика на замыканиях без сокетов; S1–S5 — стаб-сервер (клиент теста —
//! минимальный HTTP/1.1 по TCP: `Connection: close`, чтение ответа до EOF).

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chord_core::rest::retry::{
    AttemptError, Retried, RetryError, RetryLimits, attempt_once, execute_with_retries,
    is_idempotent,
};
use chord_core::rest::routes::HttpMethod;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Верхняя страховка любого async-кейса: политика обязана завершиться быстрее.
const CASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Ждёт результат кейса не дольше [`CASE_TIMEOUT`]; таймаут страховки — падение кейса.
async fn within_case<F: Future>(what: &str, future: F) -> F::Output {
    timeout(CASE_TIMEOUT, future)
        .await
        .unwrap_or_else(|_elapsed| panic!("{what}: не уложился в {CASE_TIMEOUT:?}"))
}

// ---------------------------------------------------------------------------
// N1–N6: политика ретраев на замыканиях (без сокетов)
// ---------------------------------------------------------------------------

/// N1: таблица идемпотентности (RFC 9110 §9.2.2).
#[test]
fn is_idempotent_table() {
    assert!(is_idempotent(HttpMethod::Get), "GET идемпотентен");
    assert!(is_idempotent(HttpMethod::Put), "PUT идемпотентен");
    assert!(is_idempotent(HttpMethod::Delete), "DELETE идемпотентен");
    assert!(!is_idempotent(HttpMethod::Post), "POST не идемпотентен");
    assert!(!is_idempotent(HttpMethod::Patch), "PATCH не идемпотентен");
}

/// N2: `attempt_once` навешивает таймаут; мгновенный успех проходит как есть.
#[tokio::test]
async fn attempt_once_maps_timeout() {
    // Зависшее будущее: по истечении attempt_timeout — Timeout (не паника и не зависание).
    let timed_out = within_case(
        "N2: attempt_once(pending, 50 мс)",
        attempt_once(Duration::from_millis(50), || {
            std::future::pending::<Result<u8, AttemptError>>()
        }),
    )
    .await;
    assert_eq!(
        timed_out,
        Err(AttemptError::Timeout),
        "pending + 50 мс → Timeout"
    );

    // Мгновенный успех до таймаута: значение T возвращается без изменений.
    let ok = within_case(
        "N2: attempt_once(Ok(7), 500 мс)",
        attempt_once(Duration::from_millis(500), || async {
            Ok::<u8, AttemptError>(7)
        }),
    )
    .await;
    assert_eq!(ok, Ok(7u8), "мгновенный Ok(7) → Ok(7)");
}

/// N3: не-идемпотентные POST/PATCH — ровно одна попытка, ошибка не ретраится.
#[tokio::test]
async fn non_idempotent_single_attempt() {
    let limits = RetryLimits {
        // Больше одной попытки для POST/PATCH недопустимо: лимит здесь не должен помогать.
        max_attempts: 5,
        attempt_timeout: Duration::from_millis(500),
        backoff: Duration::from_millis(5),
    };
    for method in [HttpMethod::Post, HttpMethod::Patch] {
        let mut calls = 0u32;
        let result = within_case(
            "N3: execute_with_retries(не-идемпотентный)",
            execute_with_retries(method, &limits, || {
                calls += 1;
                async { Err::<i32, AttemptError>(AttemptError::Transport("boom".to_string())) }
            }),
        )
        .await;
        assert_eq!(
            result,
            Err(RetryError::NotRetried {
                error: AttemptError::Transport("boom".to_string())
            }),
            "{method:?}: ошибка не-идемпотентного запроса не ретраится"
        );
        assert_eq!(calls, 1, "{method:?}: замыкание вызвано ровно один раз");
    }
}

/// N4: идемпотентные GET/PUT ретраятся после ошибок попытки и отдают `attempts`.
#[tokio::test]
async fn idempotent_retries_to_success() {
    let limits = RetryLimits {
        max_attempts: 4,
        attempt_timeout: Duration::from_millis(500),
        backoff: Duration::from_millis(5),
    };

    // GET: транспортная ошибка (обрыв) → повтор → успех.
    let mut calls = 0u32;
    let result = within_case(
        "N4: GET",
        execute_with_retries(HttpMethod::Get, &limits, || {
            calls += 1;
            let attempt_number = calls;
            async move {
                if attempt_number == 1 {
                    Err(AttemptError::Transport("разрыв".to_string()))
                } else {
                    Ok(7i32)
                }
            }
        }),
    )
    .await;
    assert_eq!(
        result,
        Ok(Retried {
            value: 7,
            attempts: 2
        }),
        "GET: транспортная ошибка первой попытки ретраится до успеха"
    );
    assert_eq!(calls, 2, "GET: ровно два вызова замыкания");

    // PUT: таймаут попытки → повтор → успех (PUT идемпотентен по RFC 9110 §9.2.2).
    let mut calls = 0u32;
    let result = within_case(
        "N4: PUT",
        execute_with_retries(HttpMethod::Put, &limits, || {
            calls += 1;
            let attempt_number = calls;
            async move {
                if attempt_number == 1 {
                    Err(AttemptError::Timeout)
                } else {
                    Ok(9i32)
                }
            }
        }),
    )
    .await;
    assert_eq!(
        result,
        Ok(Retried {
            value: 9,
            attempts: 2
        }),
        "PUT: таймаут первой попытки ретраится до успеха"
    );
    assert_eq!(calls, 2, "PUT: ровно два вызова замыкания");
}

/// N5: исчерпание `max_attempts` для идемпотентного; `max_attempts = 0` трактуется как 1.
#[tokio::test]
async fn idempotent_exhausts_and_clamps() {
    // Три попытки — предел: последняя ошибка в Exhausted, вызовов ровно три.
    let limits = RetryLimits {
        max_attempts: 3,
        attempt_timeout: Duration::from_millis(500),
        backoff: Duration::from_millis(5),
    };
    let mut calls = 0u32;
    let result = within_case(
        "N5: GET (max_attempts = 3)",
        execute_with_retries(HttpMethod::Get, &limits, || {
            calls += 1;
            async { Err::<i32, AttemptError>(AttemptError::Timeout) }
        }),
    )
    .await;
    assert_eq!(
        result,
        Err(RetryError::Exhausted {
            attempts: 3,
            last: AttemptError::Timeout
        }),
        "исчерпание лимита: Exhausted с числом попыток и последней ошибкой"
    );
    assert_eq!(calls, 3, "ровно max_attempts вызовов замыкания");

    // max_attempts = 0 → одна попытка (трактовка ТЗ §6.1), а не бесконечный цикл/ноль попыток.
    let limits = RetryLimits {
        max_attempts: 0,
        attempt_timeout: Duration::from_millis(500),
        backoff: Duration::from_millis(5),
    };
    let mut calls = 0u32;
    let result = within_case(
        "N5: GET (max_attempts = 0)",
        execute_with_retries(HttpMethod::Get, &limits, || {
            calls += 1;
            async { Err::<i32, AttemptError>(AttemptError::Timeout) }
        }),
    )
    .await;
    assert_eq!(
        result,
        Err(RetryError::Exhausted {
            attempts: 1,
            last: AttemptError::Timeout
        }),
        "0 трактуется как 1: Exhausted после одной попытки"
    );
    assert_eq!(calls, 1, "ровно один вызов замыкания при max_attempts = 0");
}

/// N6: пауза `backoff` между попытками действительно выдерживается — защита от «тихого»
/// удаления `sleep`; проверяется только нижняя граница (медленная машина кейс не валит).
#[tokio::test]
async fn backoff_pause_between_attempts() {
    const BACKOFF: Duration = Duration::from_millis(60);
    let limits = RetryLimits {
        max_attempts: 3,
        attempt_timeout: Duration::from_millis(500),
        backoff: BACKOFF,
    };
    let mut calls = 0u32;
    let started = Instant::now();
    let result = within_case(
        "N6: GET (пауза перед повтором)",
        execute_with_retries(HttpMethod::Get, &limits, || {
            calls += 1;
            let attempt_number = calls;
            async move {
                if attempt_number == 1 {
                    Err(AttemptError::Transport("разрыв".to_string()))
                } else {
                    Ok(11i32)
                }
            }
        }),
    )
    .await;
    let elapsed = started.elapsed();
    assert_eq!(
        result,
        Ok(Retried {
            value: 11,
            attempts: 2
        }),
        "успех после паузы: retried с двумя попытками"
    );
    assert_eq!(calls, 2, "ровно два вызова замыкания");
    assert!(
        elapsed >= BACKOFF,
        "пауза между попытками не выдержана: прошло {elapsed:?} при backoff {BACKOFF:?}"
    );
}

// ---------------------------------------------------------------------------
// Стаб-сервер 127.0.0.1:0 (ТЗ §6.2)
// ---------------------------------------------------------------------------

/// Ответ стаба: `HTTP/1.1 200 OK`, `Content-Length: 2`, тело `ok`, закрытие.
const STUB_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";

/// Режим ответа стаба (ТЗ §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Отвечать `200 OK` с телом `ok` на каждое соединение.
    Always200,
    /// Не отвечать и не закрывать: попытка клиента обязана истечь по `attempt_timeout`.
    Hang,
    /// Первое соединение — `Hang`, дальше — `Always200`.
    HangFirst,
    /// Первое соединение — закрыть без ответа, дальше — `Always200`.
    CloseFirst,
}

/// Стаб-сервер на `127.0.0.1:0`; задачи гасятся в [`Drop`] (утечек нет).
struct StubServer {
    /// База клиента теста: `http://127.0.0.1:<эфемерный порт>`.
    base: String,
    /// Сколько запросов прочитано целиком (до `\r\n\r\n`).
    seen: Arc<AtomicUsize>,
    /// Задачи обработчиков соединений (для `abort` в `Drop`).
    handlers: Arc<Mutex<Vec<JoinHandle<()>>>>,
    /// Задача accept-цикла (для `abort` в `Drop`).
    accept: JoinHandle<()>,
}

impl StubServer {
    /// Поднимает стаб-сервер; слушающий сокет — только loopback (`127.0.0.1:0`).
    async fn start(mode: Mode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("стаб: bind 127.0.0.1:0");
        let addr = listener.local_addr().expect("стаб: local_addr");
        let seen = Arc::new(AtomicUsize::new(0));
        // Обработчики складываются в общий список: отмена accept-задачи не снимает задачи,
        // которые она успела породить (режим Hang держит сокет до отмены).
        let handlers = Arc::new(Mutex::new(Vec::new()));
        let accept = tokio::spawn(accept_loop(
            listener,
            mode,
            Arc::clone(&seen),
            Arc::clone(&handlers),
        ));
        Self {
            base: format!("http://{addr}"),
            seen,
            handlers,
            accept,
        }
    }

    /// Сколько запросов стаб прочитал целиком (до `\r\n\r\n`).
    fn requests(&self) -> usize {
        self.seen.load(Ordering::SeqCst)
    }
}

impl Drop for StubServer {
    /// Гасит accept-цикл и все задачи обработчиков (ТЗ §6.2: утечек нет).
    fn drop(&mut self) {
        self.accept.abort();
        let mut handlers = self
            .handlers
            .lock()
            .expect("стаб: мьютекс задач не отравлен");
        for handler in handlers.drain(..) {
            handler.abort();
        }
    }
}

/// Accept-цикл: по задаче на соединение — `Hang` первого не блокирует следующие попытки.
async fn accept_loop(
    listener: TcpListener,
    mode: Mode,
    seen: Arc<AtomicUsize>,
    handlers: Arc<Mutex<Vec<JoinHandle<()>>>>,
) {
    let mut accepted = 0u32;
    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                accepted += 1;
                let handler = tokio::spawn(serve_connection(socket, mode, accepted, seen.clone()));
                handlers
                    .lock()
                    .expect("стаб: мьютекс задач не отравлен")
                    .push(handler);
            }
            Err(error) => {
                // Среда деградировала: без accept клиент получит отказ соединения — кейс
                // упадёт явно (eprintln фиксирует причину), а не «зависнет молча».
                eprintln!("стаб: accept: {error}");
                return;
            }
        }
    }
}

/// Одно соединение: читает запрос до `\r\n\r\n`, учитывает его и отвечает по режиму.
async fn serve_connection(
    mut socket: TcpStream,
    mode: Mode,
    accepted: u32,
    seen: Arc<AtomicUsize>,
) {
    let mut request = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match socket.read(&mut chunk).await {
            // Клиент ушёл до конца запроса — штатное следствие таймаута попытки: счётчик не растёт.
            Ok(0) => return,
            Ok(read) => {
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            // Обрыв до конца запроса — тоже штатное следствие таймаута попытки.
            Err(_) => return,
        }
    }
    seen.fetch_add(1, Ordering::SeqCst);
    match mode {
        Mode::Always200 => write_response(&mut socket).await,
        Mode::Hang => hang(socket).await,
        Mode::HangFirst if accepted == 1 => hang(socket).await,
        Mode::HangFirst => write_response(&mut socket).await,
        // Закрытие без ответа: клиент прочитает пустой ответ (`Transport`).
        Mode::CloseFirst if accepted == 1 => {}
        Mode::CloseFirst => write_response(&mut socket).await,
    }
}

/// Держит соединение без ответа и без FIN до отмены задачи в [`StubServer::drop`].
///
/// Иначе клиент получил бы пустой ответ (`Transport`) вместо истечения `attempt_timeout`.
async fn hang(socket: TcpStream) {
    let _held = socket;
    std::future::pending::<()>().await;
}

/// Пишет ответ `Always200`; ошибка записи — штатная гонка (клиент уже ушёл по таймауту).
async fn write_response(socket: &mut TcpStream) {
    let _ignored = socket.write_all(STUB_RESPONSE).await;
}

// ---------------------------------------------------------------------------
// Минимальный HTTP/1.1-клиент теста
// ---------------------------------------------------------------------------

/// Запрос к стабу: `Connection: close`, ответ читается до EOF.
///
/// Ошибки соединения/записи/чтения и пустой ответ — [`AttemptError::Transport`] (в том числе
/// кейс S5: стаб закрыл соединение без ответа); истечение `attempt_timeout` навешивает
/// [`attempt_once`] — уже [`AttemptError::Timeout`].
async fn http_request(
    base: &str,
    method: HttpMethod,
    path: &str,
) -> Result<(u16, String), AttemptError> {
    let addr = base.strip_prefix("http://").expect("стаб: база http://");
    let mut stream = TcpStream::connect(addr)
        .await
        .map_err(|error| AttemptError::Transport(format!("connect {addr}: {error}")))?;
    let head = format!(
        "{} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n",
        method.as_str()
    );
    stream
        .write_all(head.as_bytes())
        .await
        .map_err(|error| AttemptError::Transport(format!("write: {error}")))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .await
        .map_err(|error| AttemptError::Transport(format!("read: {error}")))?;
    parse_response(&raw)
}

/// Разбор ответа стаба: код из `HTTP/1.1 <код> …`, тело после `\r\n\r\n`.
fn parse_response(raw: &[u8]) -> Result<(u16, String), AttemptError> {
    if raw.is_empty() {
        return Err(AttemptError::Transport(
            "пустой ответ: соединение закрыто без ответа".to_string(),
        ));
    }
    let text = std::str::from_utf8(raw)
        .map_err(|error| AttemptError::Transport(format!("ответ не UTF-8: {error}")))?;
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| AttemptError::Transport("ответ без пустой строки заголовков".to_string()))?;
    let status_line = head.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| {
            AttemptError::Transport(format!("нераспознанный статус: {status_line:?}"))
        })?;
    Ok((status, body.to_string()))
}

// ---------------------------------------------------------------------------
// S1–S5: стаб-кейсы (попытки проходят через реальный TCP 127.0.0.1)
// ---------------------------------------------------------------------------

/// S1: стаб `Always200` — успех с первой попытки, ровно один запрос.
#[tokio::test]
async fn stub_get_success_first_try() {
    let stub = StubServer::start(Mode::Always200).await;
    let limits = RetryLimits {
        max_attempts: 3,
        attempt_timeout: Duration::from_millis(500),
        backoff: Duration::from_millis(10),
    };
    let result = within_case(
        "S1: GET",
        execute_with_retries(HttpMethod::Get, &limits, || {
            http_request(&stub.base, HttpMethod::Get, "/probe")
        }),
    )
    .await;
    assert_eq!(
        result,
        Ok(Retried {
            value: (200u16, "ok".to_string()),
            attempts: 1
        }),
        "первая попытка успешна: статус 200 и тело ok"
    );
    assert_eq!(stub.requests(), 1, "сервер увидел ровно один запрос");
}

/// S2: стаб `HangFirst` — таймаут первой попытки, повтор побеждает; есть полный таймаут.
#[tokio::test]
async fn stub_timeout_then_retry_wins() {
    let stub = StubServer::start(Mode::HangFirst).await;
    let limits = RetryLimits {
        max_attempts: 3,
        attempt_timeout: Duration::from_millis(150),
        backoff: Duration::from_millis(20),
    };
    let started = Instant::now();
    let result = within_case(
        "S2: GET",
        execute_with_retries(HttpMethod::Get, &limits, || {
            http_request(&stub.base, HttpMethod::Get, "/probe")
        }),
    )
    .await;
    let elapsed = started.elapsed();
    assert_eq!(
        result,
        Ok(Retried {
            value: (200u16, "ok".to_string()),
            attempts: 2
        }),
        "повтор после таймаута первой попытки успешен"
    );
    assert_eq!(stub.requests(), 2, "сервер увидел два запроса");
    // Нижняя граница: первая попытка отдала attempt_timeout целиком (верхних границ нет).
    assert!(
        elapsed >= Duration::from_millis(150),
        "полный attempt_timeout перед повтором: прошло {elapsed:?}"
    );
}

/// S3: стаб `Hang`, POST — не-идемпотентный не повторяется: один запрос, один таймаут.
#[tokio::test]
async fn stub_timeout_post_not_retried() {
    let stub = StubServer::start(Mode::Hang).await;
    let limits = RetryLimits {
        max_attempts: 3,
        attempt_timeout: Duration::from_millis(150),
        backoff: Duration::from_millis(20),
    };
    let started = Instant::now();
    let result = within_case(
        "S3: POST",
        execute_with_retries(HttpMethod::Post, &limits, || {
            http_request(&stub.base, HttpMethod::Post, "/probe")
        }),
    )
    .await;
    let elapsed = started.elapsed();
    assert_eq!(
        result,
        Err(RetryError::NotRetried {
            error: AttemptError::Timeout
        }),
        "POST: одна попытка, истёкшая по таймауту, не ретраится"
    );
    assert_eq!(stub.requests(), 1, "сервер увидел ровно один запрос");
    assert!(
        elapsed >= Duration::from_millis(150),
        "попытка отдала полный attempt_timeout: прошло {elapsed:?}"
    );
}

/// S4: стаб `Hang`, GET — исчерпание лимита: три попытки и три запроса.
#[tokio::test]
async fn stub_all_timeout_exhausts() {
    let stub = StubServer::start(Mode::Hang).await;
    let limits = RetryLimits {
        max_attempts: 3,
        attempt_timeout: Duration::from_millis(80),
        backoff: Duration::from_millis(10),
    };
    let result = within_case(
        "S4: GET",
        execute_with_retries(HttpMethod::Get, &limits, || {
            http_request(&stub.base, HttpMethod::Get, "/probe")
        }),
    )
    .await;
    assert_eq!(
        result,
        Err(RetryError::Exhausted {
            attempts: 3,
            last: AttemptError::Timeout
        }),
        "все попытки истекли: Exhausted"
    );
    assert_eq!(stub.requests(), 3, "сервер увидел три запроса");
}

/// S5: стаб `CloseFirst` — пустой ответ = `Transport`, идемпотентный GET ретраится и побеждает.
#[tokio::test]
async fn stub_close_is_transport_then_retry() {
    let stub = StubServer::start(Mode::CloseFirst).await;
    let limits = RetryLimits {
        max_attempts: 3,
        attempt_timeout: Duration::from_millis(500),
        backoff: Duration::from_millis(10),
    };
    let result = within_case(
        "S5: GET",
        execute_with_retries(HttpMethod::Get, &limits, || {
            http_request(&stub.base, HttpMethod::Get, "/probe")
        }),
    )
    .await;
    assert_eq!(
        result,
        Ok(Retried {
            value: (200u16, "ok".to_string()),
            attempts: 2
        }),
        "транспортная ошибка (пустой ответ) ретраится для идемпотентных"
    );
    assert_eq!(stub.requests(), 2, "сервер увидел два запроса");
}

// ---------------------------------------------------------------------------
// P1–P5: Task-170 (BE-P2-01 & BE-P3-04) — REST Retry Policy & Client Parity
// ---------------------------------------------------------------------------

use chord_core::rest::rate_limit::{
    DEFAULT_RETRY_AFTER, MAX_RETRY_AFTER, RETRYABLE_SERVER_STATUSES, RateLimitInfo, RestFailure,
    is_retryable_server_status, parse_retry_after_or_default, rate_limit_info,
    retry_delay_client_parity, should_retry_client_parity,
};

fn test_header(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

/// P1: Константа RETRYABLE_SERVER_STATUSES и предикат is_retryable_server_status.
/// Оракул: Discord web bundle @10 692 340 (E = Set([502, 504, 507, 598, 599, 522, 523, 524])).
#[test]
fn test_retryable_server_statuses_parity() {
    assert_eq!(
        RETRYABLE_SERVER_STATUSES,
        [502, 504, 507, 522, 523, 524, 598, 599],
        "RETRYABLE_SERVER_STATUSES обязан содержать ровно 8 кодов из оракула E"
    );

    for &status in &RETRYABLE_SERVER_STATUSES {
        assert!(
            is_retryable_server_status(status),
            "статус {status} обязан классифицироваться как повторяемый"
        );
    }

    // Неповторяемые серверные и клиентские статусы
    for non_retryable in [
        200, 204, 400, 401, 403, 404, 413, 429, 500, 501, 503, 505, 520, 521, 525,
    ] {
        assert!(
            !is_retryable_server_status(non_retryable),
            "статус {non_retryable} НЕ должен быть в RETRYABLE_SERVER_STATUSES"
        );
    }
}

/// P2: parse_retry_after_or_default для 429 и 503:
/// - заголовок retry-after (целые/дробные секунды)
/// - тело retry_after (float)
/// - дефолт 5 с при отсутствии
/// - cap 60 с
/// - для прочих кодов (500, 502, 200, 404) -> None
#[test]
fn test_parse_retry_after_or_default() {
    assert_eq!(DEFAULT_RETRY_AFTER, Duration::from_secs(5));
    assert_eq!(MAX_RETRY_AFTER, Duration::from_secs(60));

    // 429: заголовок retry-after
    let h1 = [test_header("retry-after", "3")];
    assert_eq!(
        parse_retry_after_or_default(429, &h1, b""),
        Some(Duration::from_secs(3)),
        "429 с retry-after: 3 -> 3s"
    );

    // 429: заголовок Retry-After (case-insensitive)
    let h2 = [test_header("Retry-After", "10")];
    assert_eq!(
        parse_retry_after_or_default(429, &h2, b""),
        Some(Duration::from_secs(10)),
        "429 с Retry-After: 10 -> 10s"
    );

    // 429: тело retry_after (float)
    let body = br#"{"message":"rate limited","retry_after":2.5}"#;
    assert_eq!(
        parse_retry_after_or_default(429, &[], body),
        Some(Duration::from_millis(2500)),
        "429 с JSON retry_after: 2.5 -> 2500ms"
    );

    // 429: приоритет заголовка над телом
    let h_prio = [test_header("retry-after", "4")];
    let b_prio = br#"{"retry_after":1.5}"#;
    assert_eq!(
        parse_retry_after_or_default(429, &h_prio, b_prio),
        Some(Duration::from_secs(4)),
        "заголовок имеет приоритет над телом"
    );

    // 429: дефолт 5с при отсутствии заголовка и тела
    assert_eq!(
        parse_retry_after_or_default(429, &[], b""),
        Some(Duration::from_secs(5)),
        "429 без retry-after -> дефолт 5s"
    );

    // 429: cap 60с
    let h_cap = [test_header("retry-after", "120")];
    assert_eq!(
        parse_retry_after_or_default(429, &h_cap, b""),
        Some(Duration::from_secs(60)),
        "429 с retry-after: 120 -> cap 60s"
    );

    // 503: заголовок retry-after (DoD: 503 с retry-after: 3 ждёт 3 с)
    let h_503 = [test_header("retry-after", "3")];
    assert_eq!(
        parse_retry_after_or_default(503, &h_503, b""),
        Some(Duration::from_secs(3)),
        "503 с retry-after: 3 -> 3s"
    );

    // 503: дефолт 5с
    assert_eq!(
        parse_retry_after_or_default(503, &[], b""),
        Some(Duration::from_secs(5)),
        "503 без retry-after -> дефолт 5s"
    );

    // 503: тело retry_after
    assert_eq!(
        parse_retry_after_or_default(503, &[], br#"{"retry_after":0.75}"#),
        Some(Duration::from_millis(750)),
        "503 с телом retry_after -> 750ms"
    );

    // Битое значение заголовка -> fallback на дефолт 5с
    let h_bad = [test_header("retry-after", "invalid")];
    assert_eq!(
        parse_retry_after_or_default(429, &h_bad, b""),
        Some(Duration::from_secs(5)),
        "битое значение -> дефолт 5s"
    );

    // Прочие статусы (500, 502, 200, 404) -> None
    for st in [200, 400, 401, 404, 500, 501, 502, 504] {
        assert_eq!(
            parse_retry_after_or_default(st, &h1, b""),
            None,
            "статус {st} не должен разбирать retry-after"
        );
    }
}

/// P3: should_retry_client_parity:
/// - 429: всегда true для любого метода и explicit_retries
/// - 503: всегда true для любого метода и explicit_retries
/// - RETRYABLE_SERVER_STATUSES: true если метод идемпотентен ИЛИ explicit_retries > 0;
///   false если не-идемпотентный и explicit_retries == 0.
/// - 500/501/505: всегда false
/// - 401/4xx: всегда false
#[test]
fn test_should_retry_client_parity() {
    let failure_429 = RestFailure::RateLimited(RateLimitInfo::new(Duration::from_secs(1)));
    let failure_503 = RestFailure::Server { status: 503 };
    let failure_500 = RestFailure::Server { status: 500 };
    let failure_501 = RestFailure::Server { status: 501 };
    let failure_505 = RestFailure::Server { status: 505 };
    let failure_401 = RestFailure::Unauthorized;
    let failure_404 = RestFailure::Client { status: 404 };

    let all_methods = [
        HttpMethod::Get,
        HttpMethod::Put,
        HttpMethod::Delete,
        HttpMethod::Post,
        HttpMethod::Patch,
    ];

    // 429: всегда true
    for method in all_methods {
        assert!(
            should_retry_client_parity(method, &failure_429, 0),
            "429 с retries=0"
        );
        assert!(
            should_retry_client_parity(method, &failure_429, 2),
            "429 с retries=2"
        );
    }

    // 503: всегда true
    for method in all_methods {
        assert!(
            should_retry_client_parity(method, &failure_503, 0),
            "503 с retries=0"
        );
        assert!(
            should_retry_client_parity(method, &failure_503, 3),
            "503 с retries=3"
        );
    }

    // RETRYABLE_SERVER_STATUSES (502, 504, 507, 522, 523, 524, 598, 599)
    for &status in &RETRYABLE_SERVER_STATUSES {
        let failure_server = RestFailure::Server { status };

        // Идемпотентные методы (GET, PUT, DELETE) ретраятся даже с explicit_retries = 0
        for method in [HttpMethod::Get, HttpMethod::Put, HttpMethod::Delete] {
            assert!(
                should_retry_client_parity(method, &failure_server, 0),
                "{status}: идемпотентный метод {method:?} ретраится при retries=0"
            );
        }

        // Не-идемпотентные (POST, PATCH) НЕ ретраятся при explicit_retries = 0
        for method in [HttpMethod::Post, HttpMethod::Patch] {
            assert!(
                !should_retry_client_parity(method, &failure_server, 0),
                "{status}: не-идемпотентный метод {method:?} НЕ ретраится при retries=0"
            );
        }

        // Не-идемпотентные (POST, PATCH) ретраятся при explicit_retries > 0
        for method in [HttpMethod::Post, HttpMethod::Patch] {
            assert!(
                should_retry_client_parity(method, &failure_server, 1),
                "{status}: не-идемпотентный метод {method:?} ретраится при retries=1"
            );
            assert!(
                should_retry_client_parity(method, &failure_server, 3),
                "{status}: не-идемпотентный метод {method:?} ретраится при retries=3"
            );
        }
    }

    // 500, 501, 505: никогда не повторяются (DoD: 500 не повторяется)
    for server_fail in [&failure_500, &failure_501, &failure_505] {
        for method in all_methods {
            assert!(
                !should_retry_client_parity(method, server_fail, 0),
                "500/501/505 не повторяются при retries=0"
            );
            assert!(
                !should_retry_client_parity(method, server_fail, 5),
                "500/501/505 не повторяются даже при retries=5"
            );
        }
    }

    // 401 и 4xx: никогда не повторяются
    for client_fail in [&failure_401, &failure_404] {
        for method in all_methods {
            assert!(!should_retry_client_parity(method, client_fail, 0));
            assert!(!should_retry_client_parity(method, client_fail, 3));
        }
    }
}

/// P4: retry_delay_client_parity:
/// - 429 и 503 используют parse_retry_after_or_default (дефолт 5 с, cap 60 с; 503 с retry-after: 3 -> 3 с)
/// - повторяемые серверные статусы используют 1с базовая * 2^(attempt-1) capped at 60с
/// - 500/501/505 возвращают None
/// - 401/4xx возвращают None
#[test]
fn test_retry_delay_client_parity() {
    let failure_429 = RestFailure::RateLimited(RateLimitInfo::new(Duration::from_millis(250)));
    let failure_503 = RestFailure::Server { status: 503 };
    let failure_500 = RestFailure::Server { status: 500 };
    let failure_502 = RestFailure::Server { status: 502 };
    let failure_401 = RestFailure::Unauthorized;
    let failure_404 = RestFailure::Client { status: 404 };

    // 429 с заголовком retry-after: 3 -> 3с
    let h3 = [test_header("retry-after", "3")];
    assert_eq!(
        retry_delay_client_parity(1, &failure_429, &h3, b""),
        Some(Duration::from_secs(3))
    );

    // 429 без заголовков, но с существующим info.retry_after = 250ms -> 250ms
    assert_eq!(
        retry_delay_client_parity(1, &failure_429, &[], b""),
        Some(Duration::from_millis(250))
    );

    // 429 с нулевым info и без заголовков -> дефолт 5с
    let failure_429_zero = RestFailure::RateLimited(RateLimitInfo::default());
    assert_eq!(
        retry_delay_client_parity(1, &failure_429_zero, &[], b""),
        Some(Duration::from_secs(5))
    );

    // 503 с заголовком retry-after: 3 -> 3с
    assert_eq!(
        retry_delay_client_parity(1, &failure_503, &h3, b""),
        Some(Duration::from_secs(3)),
        "503 с retry-after: 3 ждёт 3 с"
    );

    // 503 без заголовков -> дефолт 5с
    assert_eq!(
        retry_delay_client_parity(1, &failure_503, &[], b""),
        Some(Duration::from_secs(5)),
        "503 без retry-after ждёт дефолтные 5 с"
    );

    // 503 с заголовком retry-after: 90 -> cap 60с
    let h90 = [test_header("retry-after", "90")];
    assert_eq!(
        retry_delay_client_parity(1, &failure_503, &h90, b""),
        Some(Duration::from_secs(60)),
        "503 с retry-after: 90 ограничивается 60 с"
    );

    // Повторяемый серверный статус 502: экспоненциальный бэкофф с базой 1с
    assert_eq!(
        retry_delay_client_parity(1, &failure_502, &[], b""),
        Some(Duration::from_secs(1)),
        "attempt 1 -> 1s"
    );
    assert_eq!(
        retry_delay_client_parity(2, &failure_502, &[], b""),
        Some(Duration::from_secs(2)),
        "attempt 2 -> 2s"
    );
    assert_eq!(
        retry_delay_client_parity(3, &failure_502, &[], b""),
        Some(Duration::from_secs(4)),
        "attempt 3 -> 4s"
    );
    assert_eq!(
        retry_delay_client_parity(4, &failure_502, &[], b""),
        Some(Duration::from_secs(8)),
        "attempt 4 -> 8s"
    );
    assert_eq!(
        retry_delay_client_parity(5, &failure_502, &[], b""),
        Some(Duration::from_secs(16)),
        "attempt 5 -> 16s"
    );
    assert_eq!(
        retry_delay_client_parity(6, &failure_502, &[], b""),
        Some(Duration::from_secs(32)),
        "attempt 6 -> 32s"
    );
    assert_eq!(
        retry_delay_client_parity(7, &failure_502, &[], b""),
        Some(Duration::from_secs(60)),
        "attempt 7 -> cap 60s"
    );
    assert_eq!(
        retry_delay_client_parity(u32::MAX, &failure_502, &[], b""),
        Some(Duration::from_secs(60)),
        "attempt u32::MAX -> cap 60s без паники/переполнения"
    );

    // Неповторяемый 500 -> None
    assert_eq!(
        retry_delay_client_parity(1, &failure_500, &[], b""),
        None,
        "500 -> None"
    );
    assert_eq!(
        retry_delay_client_parity(2, &failure_500, &h3, b""),
        None,
        "500 даже с заголовком retry-after -> None"
    );

    // Клиентские ошибки и 401 -> None
    assert_eq!(retry_delay_client_parity(1, &failure_401, &[], b""), None);
    assert_eq!(retry_delay_client_parity(1, &failure_404, &[], b""), None);
}

/// P5: RateLimitInfo limit и scope (BE-P3-04).
/// Разбор x-ratelimit-limit (u32) и x-ratelimit-scope (user/global/shared).
#[test]
fn test_rate_limit_info_limit_and_scope() {
    let headers = [
        test_header("retry-after", "2"),
        test_header("x-ratelimit-limit", "10"),
        test_header("x-ratelimit-scope", "user"),
    ];
    let info = rate_limit_info(429, &headers, b"").expect("429");
    assert_eq!(info.retry_after, Duration::from_secs(2));
    assert_eq!(info.limit, Some(10));
    assert_eq!(info.scope.as_deref(), Some("user"));

    // scope global и shared
    let headers_global = [
        test_header("x-ratelimit-scope", "global"),
        test_header("x-ratelimit-limit", "50"),
    ];
    let info_global = rate_limit_info(429, &headers_global, b"").expect("429");
    assert_eq!(info_global.scope.as_deref(), Some("global"));
    assert_eq!(info_global.limit, Some(50));

    let headers_shared = [test_header("X-RateLimit-Scope", "shared")];
    let info_shared = rate_limit_info(429, &headers_shared, b"").expect("429");
    assert_eq!(info_shared.scope.as_deref(), Some("shared"));
    assert_eq!(info_shared.limit, None);

    // Отсутствие заголовков -> None
    let info_empty = rate_limit_info(429, &[], b"").expect("429");
    assert_eq!(info_empty.limit, None);
    assert_eq!(info_empty.scope, None);

    // Битое значение limit -> None; пустой scope -> None
    let headers_invalid = [
        test_header("x-ratelimit-limit", "not_a_number"),
        test_header("x-ratelimit-scope", "   "),
    ];
    let info_invalid = rate_limit_info(429, &headers_invalid, b"").expect("429");
    assert_eq!(info_invalid.limit, None);
    assert_eq!(info_invalid.scope, None);

    // Билдер методы RateLimitInfo
    let builder_info = RateLimitInfo::new(Duration::from_secs(1))
        .with_limit(25)
        .with_scope("user");
    assert_eq!(builder_info.retry_after, Duration::from_secs(1));
    assert_eq!(builder_info.limit, Some(25));
    assert_eq!(builder_info.scope.as_deref(), Some("user"));
}
