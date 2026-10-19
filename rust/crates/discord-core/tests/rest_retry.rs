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

use discord_core::rest::retry::{
    AttemptError, Retried, RetryError, RetryLimits, attempt_once, execute_with_retries,
    is_idempotent,
};
use discord_core::rest::routes::HttpMethod;
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
