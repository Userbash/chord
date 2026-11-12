//! Супервайзер шлюза: connect → HELLO → IDENTIFY/RESUME → dispatch/heartbeat → close
//! (волна RW-39, NET-E1; основание — `AUTH-RE.md` G-04…G-20 и `CORE-AUDIT.md` P0-3).
//!
//! **Провенанс оракулов** (не подгонка под реализацию):
//! - `AUTH-RE.md`: G-12 (heartbeat op40 `{seq,qos}`, op1 от сервера — триггер), G-16
//!   (`resume_gateway_url` с обрезанным `/`), G-18 (close 4004 → logout, op7 → reconnect,
//!   op9 `d=true` → RESUME / `d=false` → IDENTIFY), G-04…G-11 (поля IDENTIFY);
//! - `ADR-2-auth.md`: close 4004 → logout; 4013/4014 → прекратить reconnect; heartbeat op40
//!   с fallback op1.
//!
//! Все сетевые кейсы — только локальный стаб `127.0.0.1` под `tokio::time::timeout`.
//! Реального Discord, окна и GUI нет.

use chord_core::codec::etf::{self, EtfValue};
use chord_core::gateway::supervisor::{
    Backoff, CloseAction, StopReason, Supervisor, SupervisorConfig, SupervisorOutcome,
    classify_close,
};
use chord_core::identify::CAPABILITIES_ACTIVE;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::ws::GatewayUrl;
use futures_util::{SinkExt, StreamExt};
use std::future::Future;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::{WebSocketStream, accept_async};

/// Тайм-аут любого сетевого кейса.
const CASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Интервал heartbeat стаба (первый удар отодвинут сидом — рукопожатие успевает).
const STUB_INTERVAL_MS: i64 = 41_250;

/// Сид jitter: первый удар через 1000 мс, стаб успевает прочитать IDENTIFY/RESUME.
const JITTER_SEED: u64 = 1000;

/// session_id из READY (32 символа — формат живого клиента).
const SESSION_ID: &str = "f14ac1f3d408c15027d6d98da172c398";

// ---------------------------------------------------------------------------
// ETF-помощники (формы как в захвате: op/t/s/d — атомы)
// ---------------------------------------------------------------------------

/// Ключ/значение-атом map.
fn atom(name: &str) -> EtfValue {
    EtfValue::Atom(name.to_string())
}

/// Map из пар «ключ-атом → значение».
fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(key), value))
            .collect(),
    )
}

/// Список (форма массивов в захвате).
fn list(items: Vec<EtfValue>) -> EtfValue {
    EtfValue::List(items)
}

/// HELLO с интервалом `interval`.
fn hello_value(interval: i64) -> EtfValue {
    map(vec![
        ("op", EtfValue::Int(10)),
        (
            "d",
            map(vec![("heartbeat_interval", EtfValue::Int(interval))]),
        ),
    ])
}

/// READY (op0, s=1) с session_id и (опционально) resume_gateway_url.
fn ready_value(session_id: &str, resume_gateway_url: Option<&str>) -> EtfValue {
    let mut d = vec![
        ("guilds", list(Vec::new())),
        ("session_id", EtfValue::Str(session_id.to_string())),
    ];
    if let Some(url) = resume_gateway_url {
        d.push(("resume_gateway_url", EtfValue::Str(url.to_string())));
    }
    map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("READY")),
        ("s", EtfValue::Int(1)),
        ("d", map(d)),
    ])
}

/// RESUMED (op0) — ответ сервера на RESUME.
fn resumed_value(seq: i64) -> EtfValue {
    map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("RESUMED")),
        ("s", EtfValue::Int(seq)),
        ("d", map(Vec::new())),
    ])
}

/// PRESENCE_UPDATE (op0) с реальной формой `d` (`user` + `status`), как в захвате.
fn presence_value(seq: i64, user_id: &str) -> EtfValue {
    map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("PRESENCE_UPDATE")),
        ("s", EtfValue::Int(seq)),
        (
            "d",
            map(vec![
                (
                    "user",
                    map(vec![("id", EtfValue::Str(user_id.to_string()))]),
                ),
                ("status", atom("online")),
            ]),
        ),
    ])
}

/// op0 с именем, которого нет в 21 типе ядра: raw-событие (`kind: None`).
fn raw_dispatch_value(seq: i64, name: &str) -> EtfValue {
    map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom(name)),
        ("s", EtfValue::Int(seq)),
        ("d", map(Vec::new())),
    ])
}

/// op7 RECONNECT.
fn op7_value() -> EtfValue {
    map(vec![("op", EtfValue::Int(7))])
}

/// op9 INVALID_SESSION с `d`.
fn op9_value(d: EtfValue) -> EtfValue {
    map(vec![("op", EtfValue::Int(9)), ("d", d)])
}

/// Длина строки-филлера: больше окна ruzstd стаба, поэтому полезный терм выходит из удержания.
const FILLER_TEXT_LEN: usize = 1536;

/// **Непрерывный** zstd-поток стаба (как у шлюза): один кадр на соединение, без `finish()`.
struct StubStream {
    /// C-энкодер поверх `Vec<u8>`; кадр никогда не завершается.
    encoder: zstd::stream::write::Encoder<'static, Vec<u8>>,
    /// Сколько сжатых байт уже отдано потребителю.
    sent: usize,
}

impl StubStream {
    /// Энкодер с окном ≈1 КиБ (`WindowLog(10)`).
    fn new() -> Self {
        let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1).expect("zstd-энкодер");
        encoder
            .set_parameter(zstd::zstd_safe::CParameter::WindowLog(10))
            .expect("окно zstd 1 КиБ");
        Self { encoder, sent: 0 }
    }

    /// Термы + филлер одним фрагментом; возвращает свежие сжатые байты.
    fn frame(&mut self, terms: &[EtfValue]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for term in terms {
            bytes.extend_from_slice(&etf::encode(term));
        }
        self.frame_bytes(&bytes)
    }

    /// Произвольные байты потока + филлер одним фрагментом.
    ///
    /// Нужен для честно повреждённого хвоста: валидный терм и мусор уезжают
    /// в одном binary-фрагменте, как это может прийти от шлюза.
    fn frame_bytes(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut payload = bytes.to_vec();
        payload.extend_from_slice(&etf::encode(&EtfValue::Str("f".repeat(FILLER_TEXT_LEN))));
        self.encoder.write_all(&payload).expect("запись в zstd");
        self.encoder.flush().expect("flush zstd-блока");
        let buffer = self.encoder.get_mut();
        let fresh = buffer[self.sent..].to_vec();
        self.sent = buffer.len();
        fresh
    }
}

// ---------------------------------------------------------------------------
// Стаб-сервер и клиентские кадры
// ---------------------------------------------------------------------------

/// WS-сокет стаба (plain TCP: TLS в тестах не поднимается).
type StubSocket = WebSocketStream<tokio::net::TcpStream>;

/// Поднимает стаб на `count` последовательных соединений; каждый обслуживает `handler`.
async fn spawn_multi<T, F, Fut>(
    count: usize,
    handler: F,
) -> (String, tokio::task::JoinHandle<Result<Vec<T>, String>>)
where
    T: Send + 'static,
    F: Fn(usize, StubSocket) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, String>> + Send,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("стаб: bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("стаб: local_addr");
    let task = tokio::spawn(async move {
        let mut results = Vec::with_capacity(count);
        for index in 0..count {
            let (tcp, _) = listener
                .accept()
                .await
                .map_err(|error| format!("accept #{index}: {error}"))?;
            let ws = accept_async(tcp)
                .await
                .map_err(|error| format!("WS-handshake #{index}: {error}"))?;
            results.push(handler(index, ws).await?);
        }
        Ok(results)
    });
    (format!("ws://{addr}"), task)
}

/// Ждёт завершения стаба (не дольше 5 с) и разворачивает результаты.
async fn stub_result<T>(task: tokio::task::JoinHandle<Result<Vec<T>, String>>) -> Vec<T> {
    timeout(CASE_TIMEOUT, task)
        .await
        .expect("стаб-задача не завершилась за 5 с")
        .expect("стаб-задача упала")
        .expect("сценарий стаба вернул ошибку")
}

/// Ошибка слоя WS → текст сценария.
fn io_err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Отправляет термы очередным фрагментом непрерывного zstd-потока.
async fn send_terms(
    ws: &mut StubSocket,
    stream: &mut StubStream,
    terms: &[EtfValue],
) -> Result<(), String> {
    ws.send(Message::binary(stream.frame(terms)))
        .await
        .map_err(io_err)
}

/// Закрывает соединение кодом `code`.
async fn close_with(ws: &mut StubSocket, code: u16, reason: &str) -> Result<(), String> {
    ws.close(Some(CloseFrame {
        code: CloseCode::from(code),
        reason: reason.into(),
    }))
    .await
    .map_err(io_err)
}

/// Читает следующий бинарный кадр клиента, пропуская heartbeat (op40).
async fn next_client_frame(ws: &mut StubSocket) -> Result<EtfValue, String> {
    loop {
        match timeout(CASE_TIMEOUT, ws.next())
            .await
            .map_err(|_| "клиент молчит дольше 5 с".to_string())?
        {
            Some(Ok(Message::Binary(data))) => {
                let (value, consumed) =
                    etf::decode(&data).map_err(|error| format!("кадр клиента: {error}"))?;
                if consumed != data.len() {
                    return Err(format!(
                        "кадр клиента {} Б: разобран терм на {consumed} Б (хвост)",
                        data.len()
                    ));
                }
                if value.get("op").and_then(EtfValue::as_int) == Some(40) {
                    continue; // heartbeat — не рукопожатие
                }
                return Ok(value);
            }
            Some(Ok(Message::Close(_))) | None => {
                return Err("клиент закрыл соединение до рукопожатия".to_string());
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => return Err(io_err(error)),
        }
    }
}

/// Ждёт от клиента кадр с оп-кодом `expected`.
async fn expect_op(ws: &mut StubSocket, expected: i64) -> Result<EtfValue, String> {
    let value = next_client_frame(ws).await?;
    match value.get("op").and_then(EtfValue::as_int) {
        Some(op) if op == expected => Ok(value),
        other => Err(format!("ждали op{expected}, пришло {other:?}: {value:?}")),
    }
}

/// Дочитывает соединение до обрыва клиентом (после op7/op9 клиент бросает сокет).
async fn drain_until_drop(ws: &mut StubSocket) -> Result<(), String> {
    loop {
        match ws.next().await {
            Some(Ok(_)) => {}
            Some(Err(error)) => {
                // ResetWithoutClosingHandshake — штатный обрыв после Reconnect.
                let text = io_err(error);
                if text.contains("ResetWithoutClosingHandshake")
                    || text.contains("Connection reset")
                {
                    return Ok(());
                }
                return Err(text);
            }
            None => return Ok(()),
        }
    }
}

/// Конфиг супервайзера для стаба: быстрый backoff, малый лимит.
fn test_config(url: &str) -> SupervisorConfig {
    let gateway = GatewayUrl::from_base(url).expect("URL стаба");
    let mut config = SupervisorConfig::new(gateway, "TEST_TOKEN");
    config.jitter_seed = JITTER_SEED;
    config.max_reconnects = 2;
    config.backoff = Backoff::new(1, 2);
    config
}

// ---------------------------------------------------------------------------
// Чистые кейсы: close-политика и backoff
// ---------------------------------------------------------------------------

#[test]
fn close_codes_match_adr_policy() {
    // G-18 + ADR-2: 4004 — logout без ретрая; 4013/4014 — Stop.
    assert_eq!(
        classify_close(Some(4004)),
        CloseAction::Stop(StopReason::Unauthorized),
        "4004 — Unauthorized (logout)"
    );
    assert_eq!(
        classify_close(Some(4013)),
        CloseAction::Stop(StopReason::InvalidClient { code: 4013 }),
        "4013 — Stop"
    );
    assert_eq!(
        classify_close(Some(4014)),
        CloseAction::Stop(StopReason::InvalidClient { code: 4014 }),
        "4014 — Stop"
    );
    for code in [None, Some(1000), Some(4000), Some(4009), Some(1006)] {
        assert_eq!(
            classify_close(code),
            CloseAction::Reconnect,
            "код {code:?} — реконнект через backoff"
        );
    }
}

#[test]
fn backoff_doubles_caps_and_resets() {
    // Клиент: gatewayBackoff = new Backoff(1e3, 6e4) (бандл, `_handleClose`).
    let mut backoff = Backoff::new(1_000, 60_000);
    assert_eq!(backoff.next_delay_ms(), 1_000, "первая задержка — база");
    assert_eq!(backoff.next_delay_ms(), 2_000, "далее ×2");
    assert_eq!(backoff.next_delay_ms(), 4_000);
    for _ in 0..10 {
        backoff.next_delay_ms();
    }
    assert_eq!(backoff.next_delay_ms(), 60_000, "потолок 60 000 мс");
    backoff.reset();
    assert_eq!(
        backoff.next_delay_ms(),
        1_000,
        "успешная сессия сбрасывает backoff"
    );
}

// ---------------------------------------------------------------------------
// Стаб-кейсы: close-политика, IDENTIFY по RE, op7/op9, resume URL, лимит, отмена
// ---------------------------------------------------------------------------

#[tokio::test]
async fn supervisor_stops_on_close_4004() {
    // Один коннект, сервер сразу закрывает 4004 → Stop(Unauthorized), без ретрая.
    let (url, stub) = spawn_multi(1, |_index, mut ws| async move {
        close_with(&mut ws, 4004, "authentication failed").await?;
        Ok(())
    })
    .await;
    let mut supervisor = Supervisor::new(test_config(&url));
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    stub_result(stub).await;
    assert_eq!(
        outcome,
        SupervisorOutcome::Stop(StopReason::Unauthorized),
        "4004 → Unauthorized"
    );
}

#[tokio::test]
async fn supervisor_emits_ready_and_stopped_events() {
    let (url, stub) = spawn_multi(1, |_index, mut ws| async move {
        let mut stream = StubStream::new();
        send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
        expect_op(&mut ws, 2).await?;
        send_terms(&mut ws, &mut stream, &[ready_value(SESSION_ID, None)]).await?;
        close_with(&mut ws, 4004, "done").await?;
        Ok(())
    })
    .await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&events);
    let config = test_config(&url).with_event_handler(move |event| {
        captured.lock().expect("журнал событий").push(event);
    });
    let mut supervisor = Supervisor::new(config);
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    stub_result(stub).await;

    assert_eq!(outcome, SupervisorOutcome::Stop(StopReason::Unauthorized));
    assert_eq!(
        *events.lock().expect("журнал событий"),
        vec![
            chord_core::SupervisorEvent::Ready,
            chord_core::SupervisorEvent::Stopped(StopReason::Unauthorized),
        ]
    );
}

#[tokio::test]
async fn supervisor_stops_on_close_4013_and_4014() {
    // 4013/4014 — Stop без бесконечного reconnect (ADR-2).
    for code in [4013u16, 4014] {
        let (url, stub) = spawn_multi(1, move |_index, mut ws| async move {
            close_with(&mut ws, code, "invalid intents").await?;
            Ok(())
        })
        .await;
        let mut supervisor = Supervisor::new(test_config(&url));
        let outcome = timeout(CASE_TIMEOUT, supervisor.run())
            .await
            .expect("супервайзер не остановился за 5 с");
        stub_result(stub).await;
        assert_eq!(
            outcome,
            SupervisorOutcome::Stop(StopReason::InvalidClient { code }),
            "close {code} → Stop"
        );
    }
}

#[tokio::test]
async fn supervisor_identify_matches_re_capture() {
    // G-04…G-11: ровно {token, capabilities, properties, presence, compress, client_state,
    // qos_token}; capabilities 1734653, compress false, 4 поля identify в properties,
    // никаких intents/shard.
    let (url, stub) = spawn_multi(1, |_index, mut ws| async move {
        let mut stream = StubStream::new();
        send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
        let identify = expect_op(&mut ws, 2).await?;
        close_with(&mut ws, 4004, "done").await?;
        Ok(identify)
    })
    .await;
    // G-06: properties = super-properties + 4 поля идентификации (пересечение — из SP).
    let properties = SuperProperties {
        os: "Linux-RE".to_string(),
        browser: "Discord Client".to_string(),
        client_version: "1.0.162".to_string(),
        client_build_number: 641_004,
        browser_user_agent: "Mozilla/5.0 RE-TEST".to_string(),
        client_app_state: "unfocused".to_string(),
        ..SuperProperties::default()
    };
    let mut config = SupervisorConfig::with_super_properties(
        GatewayUrl::from_base(&url).expect("URL стаба"),
        "TEST_TOKEN",
        &properties,
    );
    config.jitter_seed = JITTER_SEED;
    config.max_reconnects = 2;
    config.backoff = Backoff::new(1, 2);
    let mut supervisor = Supervisor::new(config);
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    let observed = stub_result(stub).await;
    assert_eq!(
        outcome,
        SupervisorOutcome::Stop(StopReason::Unauthorized),
        "после IDENTIFY стаб закрыл 4004"
    );
    let identify = observed.into_iter().next().expect("IDENTIFY");
    let d = identify.get("d").expect("payload под d");
    let mut keys: Vec<String> = d
        .as_map()
        .expect("IDENTIFY — map")
        .iter()
        .map(|(key, _)| match key {
            EtfValue::Atom(name) | EtfValue::Str(name) => name.clone(),
            other => panic!("нестроковый ключ IDENTIFY: {other:?}"),
        })
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "capabilities",
            "client_state",
            "compress",
            "presence",
            "properties",
            "qos_token",
            "token"
        ],
        "G-04: ровно семь полей, без intents/shard"
    );
    assert_eq!(
        d.get("capabilities").and_then(EtfValue::as_int),
        Some(i64::try_from(CAPABILITIES_ACTIVE).expect("capabilities в i64")),
        "G-05: capabilities = 1734653"
    );
    assert_eq!(
        d.get("compress").and_then(EtfValue::as_atom),
        Some("false"),
        "G-10: compress = false"
    );
    assert_eq!(
        d.get("token").and_then(EtfValue::as_str),
        Some("TEST_TOKEN"),
        "токен из конфига"
    );

    let presence = d.get("presence").expect("G-07: presence");
    assert_eq!(
        presence.get("status").and_then(EtfValue::as_str),
        Some("online"),
        "presence.status = online"
    );
    assert!(presence.get("activities").is_some(), "presence.activities");
    assert!(presence.get("afk").is_some(), "presence.afk");

    let properties = d.get("properties").expect("properties");
    assert_eq!(
        properties
            .get("client_app_state")
            .and_then(EtfValue::as_str),
        Some("unfocused"),
        "G-06: client_app_state из SP"
    );
    assert_eq!(
        properties.get("os").and_then(EtfValue::as_str),
        Some("Linux-RE"),
        "G-06: поля SP переносятся в IDENTIFY"
    );
    assert_eq!(
        properties
            .get("browser_user_agent")
            .and_then(EtfValue::as_str),
        Some("Mozilla/5.0 RE-TEST"),
        "G-06: browser_user_agent из SP"
    );
    assert_eq!(
        properties
            .get("client_build_number")
            .and_then(EtfValue::as_int),
        Some(641_004),
        "G-06: client_build_number из SP"
    );
    assert_eq!(
        properties
            .get("is_fast_connect")
            .and_then(EtfValue::as_atom),
        Some("false"),
        "G-06: is_fast_connect=false"
    );
    assert_eq!(
        properties
            .get("gateway_connect_reasons")
            .and_then(EtfValue::as_str),
        Some("COLD_START"),
        "G-06: gateway_connect_reasons"
    );
    assert!(
        properties.get("installation_id").is_none(),
        "G-06: installation_id только при наличии значения"
    );
    assert!(
        properties.get("intents").is_none(),
        "G-09: intents в пользовательском IDENTIFY отсутствует"
    );
    let client_state = d.get("client_state").expect("client_state");
    assert!(
        client_state.get("guild_versions").is_some(),
        "G-08: client_state.guild_versions"
    );
    assert!(
        !d.as_map().expect("map").iter().any(|(key, _)| {
            matches!(key, EtfValue::Atom(name) | EtfValue::Str(name) if name == "shard")
        }),
        "G-09: shard отсутствует"
    );
}

#[tokio::test]
async fn supervisor_op7_resumes_and_uses_resume_url() {
    // G-16/G-18: READY.resume_gateway_url (с хвостовым `/`) → op7 → второй сокет → op6.
    let (url_b, task_b) = spawn_multi(1, |_index, mut ws| async move {
        let mut stream = StubStream::new();
        send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
        let resume = expect_op(&mut ws, 6).await?;
        send_terms(&mut ws, &mut stream, &[resumed_value(2)]).await?;
        close_with(&mut ws, 4004, "done").await?;
        Ok(resume)
    })
    .await;
    let resume_url = format!("{url_b}/");
    let (url_a, task_a) = spawn_multi(1, move |_index, mut ws| {
        let resume_url = resume_url.clone();
        async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            expect_op(&mut ws, 2).await?;
            send_terms(
                &mut ws,
                &mut stream,
                &[ready_value(SESSION_ID, Some(resume_url.as_str()))],
            )
            .await?;
            send_terms(&mut ws, &mut stream, &[op7_value()]).await?;
            drain_until_drop(&mut ws).await?;
            Ok(())
        }
    })
    .await;

    let mut supervisor = Supervisor::new(test_config(&url_a));
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    stub_result(task_a).await;
    let observed = stub_result(task_b).await;
    assert_eq!(
        outcome,
        SupervisorOutcome::Stop(StopReason::Unauthorized),
        "второй сокет закрыт 4004"
    );
    let expected_resume_url = format!("{url_b}/?encoding=etf&v=9&compress=zstd-stream");
    assert_eq!(
        supervisor.resume_url().map(GatewayUrl::as_str),
        Some(expected_resume_url.as_str()),
        "resume URL сохранён и нормализован (без хвостового слэша)"
    );
    let resume = observed.into_iter().next().expect("RESUME");
    let d = resume.get("d").expect("payload RESUME");
    assert_eq!(
        d.get("session_id").and_then(EtfValue::as_str),
        Some(SESSION_ID),
        "RESUME несёт session_id из READY"
    );
    assert_eq!(
        d.get("seq").and_then(EtfValue::as_int),
        Some(1),
        "RESUME несёт последний s"
    );
    assert_eq!(
        d.get("token").and_then(EtfValue::as_str),
        Some("TEST_TOKEN")
    );
}

#[tokio::test]
async fn supervisor_op9_true_resumes() {
    // G-18: op9 d=true → RESUME (сессия жива).
    let (url, stub) = spawn_multi(2, |index, mut ws| async move {
        let mut stream = StubStream::new();
        send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
        if index == 0 {
            let first = expect_op(&mut ws, 2).await?;
            send_terms(&mut ws, &mut stream, &[ready_value(SESSION_ID, None)]).await?;
            send_terms(&mut ws, &mut stream, &[op9_value(atom("true"))]).await?;
            drain_until_drop(&mut ws).await?;
            Ok(first)
        } else {
            let resume = expect_op(&mut ws, 6).await?;
            close_with(&mut ws, 4004, "done").await?;
            Ok(resume)
        }
    })
    .await;
    let mut supervisor = Supervisor::new(test_config(&url));
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    let observed = stub_result(stub).await;
    assert_eq!(
        outcome,
        SupervisorOutcome::Stop(StopReason::Unauthorized),
        "после RESUME второй сокет закрыт 4004"
    );
    assert_eq!(
        observed[0].get("op").and_then(EtfValue::as_int),
        Some(2),
        "первый коннект — IDENTIFY"
    );
    assert_eq!(
        observed[1].get("op").and_then(EtfValue::as_int),
        Some(6),
        "d=true → RESUME"
    );
}

#[tokio::test]
async fn supervisor_op9_false_identifies() {
    // G-18: op9 d=false → свежий IDENTIFY (resume запрещён).
    let (url, stub) = spawn_multi(2, |index, mut ws| async move {
        let mut stream = StubStream::new();
        send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
        if index == 0 {
            let first = expect_op(&mut ws, 2).await?;
            send_terms(&mut ws, &mut stream, &[ready_value(SESSION_ID, None)]).await?;
            send_terms(&mut ws, &mut stream, &[op9_value(atom("false"))]).await?;
            drain_until_drop(&mut ws).await?;
            Ok(first)
        } else {
            let second = expect_op(&mut ws, 2).await?;
            close_with(&mut ws, 4004, "done").await?;
            Ok(second)
        }
    })
    .await;
    let mut supervisor = Supervisor::new(test_config(&url));
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    let observed = stub_result(stub).await;
    assert_eq!(
        outcome,
        SupervisorOutcome::Stop(StopReason::Unauthorized),
        "второй коннект закрыт 4004"
    );
    assert_eq!(
        observed[1].get("op").and_then(EtfValue::as_int),
        Some(2),
        "d=false → IDENTIFY"
    );
}

#[tokio::test]
async fn supervisor_reconnect_limit_stops() {
    // Нет слушателя: connect падает; backoff 1→2 мс; после max_reconnects=2 — Stop.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind для свободного порта");
    let addr = listener.local_addr().expect("local_addr");
    drop(listener);
    let url = format!("ws://{addr}");
    let mut supervisor = Supervisor::new(test_config(&url));
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    assert_eq!(
        outcome,
        SupervisorOutcome::Stop(StopReason::ReconnectLimit { reconnects: 2 }),
        "лимит попыток исчерпан"
    );
    assert_eq!(supervisor.reconnects(), 2, "счётчик реконнектов");
    assert!(
        supervisor.last_error().is_some(),
        "ошибка соединения не проглочена: доступна вызывающему"
    );
}

#[tokio::test]
async fn supervisor_cancels() {
    // Отмена: стаб молчит (HELLO нет), shutdown-фьючерс срабатывает → Cancelled.
    let (url, stub) = spawn_multi(1, |_index, mut ws| async move {
        drain_until_drop(&mut ws).await?;
        Ok(())
    })
    .await;
    let mut supervisor = Supervisor::new(test_config(&url));
    let outcome = timeout(
        CASE_TIMEOUT,
        supervisor.run_until(tokio::time::sleep(Duration::from_millis(100))),
    )
    .await
    .expect("отмена не сработала за 5 с");
    stub_result(stub).await;
    assert_eq!(outcome, SupervisorOutcome::Cancelled, "отмена супервайзера");
}

// ---------------------------------------------------------------------------
// L-02-CORE-FINAL: hook dispatch-кадра (CONTRACTS/dispatch.md §6)
// ---------------------------------------------------------------------------

/// Хук отдаёт ровно те op0, что разобрал пайп: имена, `s`, `d` без переписывания.
///
/// Оракул — кадры стаба (форма как в захвате), а не текущая реализация: ожидаемые
/// `name`/`seq`/`d` выписаны явно. Raw-тип (не из 21) тоже доходит — пайп его видел.
#[tokio::test]
async fn supervisor_forwards_real_op0_frames_to_dispatch_handler() {
    let (url, stub) = spawn_multi(1, |_index, mut ws| async move {
        let mut stream = StubStream::new();
        send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
        expect_op(&mut ws, 2).await?;
        send_terms(
            &mut ws,
            &mut stream,
            &[
                ready_value(SESSION_ID, None),
                presence_value(2, "80351110224678912"),
                raw_dispatch_value(3, "MESSAGE_REACTION_REMOVE"),
            ],
        )
        .await?;
        close_with(&mut ws, 4004, "done").await?;
        Ok(())
    })
    .await;

    let dispatches = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&dispatches);
    let config = test_config(&url).with_dispatch_handler(move |event| {
        captured
            .lock()
            .expect("журнал диспатчей")
            .push(event.clone());
    });
    let mut supervisor = Supervisor::new(config);
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    stub_result(stub).await;
    assert_eq!(outcome, SupervisorOutcome::Stop(StopReason::Unauthorized));

    let events = dispatches.lock().expect("журнал диспатчей").clone();
    let names: Vec<&str> = events.iter().map(|event| event.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["READY", "PRESENCE_UPDATE", "MESSAGE_REACTION_REMOVE"],
        "хук видит ровно op0 пайпа, включая raw"
    );
    assert_eq!(events[0].kind, Some(chord_core::DispatchKind::Ready));
    assert_eq!(events[0].seq, Some(1));

    let presence = &events[1];
    assert_eq!(
        presence.kind,
        Some(chord_core::DispatchKind::PresenceUpdate)
    );
    assert_eq!(presence.seq, Some(2));
    assert_eq!(
        presence.data,
        map(vec![
            (
                "user",
                map(vec![("id", EtfValue::Str("80351110224678912".to_string()))])
            ),
            ("status", atom("online")),
        ]),
        "`d` отдаётся целиком: поля не добавляются и не переименовываются"
    );

    let raw = &events[2];
    assert_eq!(raw.kind, None, "неизвестный тип — raw, а не выдуманный вид");
    assert_eq!(raw.seq, Some(3));
}

/// Ждёт, пока хук увидит `expected` диспатчей, не закрывая соединение.
///
/// Оракул живой доставки: стаб не шлёт close, пока хук не отдал кадр, — значит
/// событие ушло обработчику **во время** соединения, а не накопленным итогом.
async fn wait_for_dispatches(
    events: &Arc<Mutex<Vec<chord_core::dispatch::DispatchEvent>>>,
    expected: usize,
) -> Result<(), String> {
    for _ in 0..300 {
        if events.lock().expect("журнал диспатчей").len() >= expected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Err(format!(
        "хук не отдал {expected} диспатчей за 3 с незакрытого соединения"
    ))
}

/// Доставка идёт по кадру, а не пачкой после close (CONTRACTS/dispatch.md §6–§7).
#[tokio::test]
async fn dispatch_handler_receives_frames_while_connection_is_open() {
    let dispatches = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&dispatches);
    let (url, stub) = spawn_multi(1, move |_index, mut ws| {
        let observed = Arc::clone(&observed);
        async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            expect_op(&mut ws, 2).await?;
            send_terms(&mut ws, &mut stream, &[ready_value(SESSION_ID, None)]).await?;
            // Соединение открыто: без живой доставки этот шаг упадёт по тайм-ауту.
            wait_for_dispatches(&observed, 1).await?;
            send_terms(
                &mut ws,
                &mut stream,
                &[presence_value(2, "80351110224678912")],
            )
            .await?;
            wait_for_dispatches(&observed, 2).await?;
            close_with(&mut ws, 4004, "done").await?;
            Ok(())
        }
    })
    .await;

    let captured = Arc::clone(&dispatches);
    let config = test_config(&url).with_dispatch_handler(move |event| {
        captured
            .lock()
            .expect("журнал диспатчей")
            .push(event.clone());
    });
    let mut supervisor = Supervisor::new(config);
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    stub_result(stub).await;

    assert_eq!(outcome, SupervisorOutcome::Stop(StopReason::Unauthorized));
    let events = dispatches.lock().expect("журнал диспатчей").clone();
    assert_eq!(
        events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>(),
        vec!["READY", "PRESENCE_UPDATE"],
        "кадры дошли до хука до закрытия соединения"
    );
    assert_eq!(events[1].seq, Some(2));
}

/// Ошибка разбора в том же фрагменте не съедает уже разобранный op0 (FIX L-02 P1).
///
/// Оракул — форма кадра стаба: первый терм фрагмента валиден (READY), затем идёт
/// `0x83 0xff` — неизвестный тег ETF, то есть честная `TransportError::Frame`
/// **после** того, как пайп положил READY в `events`. Хук обязан увидеть READY до
/// возврата ошибки: иначе попытка закрывается, а кадр теряется.
#[tokio::test]
async fn dispatch_handler_gets_op0_parsed_before_frame_error_in_same_fragment() {
    let (url, stub) = spawn_multi(1, |_index, mut ws| async move {
        let mut stream = StubStream::new();
        send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
        expect_op(&mut ws, 2).await?;
        let mut payload = etf::encode(&ready_value(SESSION_ID, None));
        payload.extend_from_slice(&[etf::VERSION_BYTE, 0xff]);
        ws.send(Message::binary(stream.frame_bytes(&payload)))
            .await
            .map_err(io_err)?;
        drain_until_drop(&mut ws).await
    })
    .await;

    let dispatches = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&dispatches);
    let config = test_config(&url).with_dispatch_handler(move |event| {
        captured
            .lock()
            .expect("журнал диспатчей")
            .push(event.clone());
    });
    let mut supervisor = Supervisor::new(config);
    let outcome = timeout(CASE_TIMEOUT, supervisor.run())
        .await
        .expect("супервайзер не остановился за 5 с");
    stub_result(stub).await;

    // READY разобран до ошибки; стаб соединение не закрывал — попытка упала на Frame
    // и супервайзер ушёл в reconnect-политику (лимит из `test_config`).
    assert!(
        matches!(
            outcome,
            SupervisorOutcome::Stop(StopReason::ReconnectLimit { .. })
        ),
        "после ошибки разбора супервайзер продолжает серию: {outcome:?}"
    );
    let events = dispatches.lock().expect("журнал диспатчей").clone();
    assert_eq!(
        events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>(),
        vec!["READY"],
        "op0, разобранный до ошибки фрагмента, доставлен хуку, а не потерян"
    );
    assert_eq!(events[0].seq, Some(1));
}
