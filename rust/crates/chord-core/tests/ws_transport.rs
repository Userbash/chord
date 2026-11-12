//! Транспорт шлюза Discord: пайп zstd→термы→сессия и WS-клиент (волна RW-04, P1-03 /
//! T-GW-01, T-GW-03) на локальном стабе `127.0.0.1`.
//!
//! **Провенанс оракулов** (независимые артефакты, не подгонка под реализацию):
//! - живой протокол (спека `docs/re/gateway-protocol.md` §3): server→client — **один непрерывный
//!   zstd-кадр**, WS-сообщения = фрагменты этого кадра; client→server — несжатые ETF-термы
//!   (`0x83` + терм). Стаб собирает поток C-энкодером `zstd` (dev-зависимость) с окном ≈1 КиБ
//!   (`CParameter::WindowLog(10)`), `finish()` не вызывается — кадр не завершается, как у шлюза;
//!   после каждого полезного терма идёт «филлер» — валидный терм-строка ~1536 Б, прокручивающий
//!   окно ruzstd (удержание `распаковано − окно`, RW-02), иначе полезный терм задерживается;
//! - HELLO-контракт (интервал 41 250 мс, `d.heartbeat_interval`) — ядро RW-03;
//! - lab-поток `{RE_LAB_ROOT:-/home/sanya/re-deep}/gw-proto/zstd-frames-95974_1389.hexl` —
//!   снимок `capture-run2.jsonl`, 137 кадров, 104 066 Б сжатых (оракул RW-02: распаковка
//!   824 816 Б, ruzstd отдаёт 759 280 Б = распаковка − окно 65 536);
//! - таймлайн `gw-proto/timeline-95974_1389.tsv` — независимое подтверждение, что в потоке
//!   137 событий: op10 × 1, op11 × 23, op0 × 113 с `s` = 1..113.
//!
//! Лабораторные файлы опциональны: нет файла — skip с причиной, а не падение.
//! Все сетевые кейсы — только локальный стаб под `tokio::time::timeout(5 с)`.

use chord_core::codec::ZstdError;
use chord_core::codec::etf::{self, EtfValue};
use chord_core::gateway::{GatewayError, HeartbeatMode, QosPayload, ReconnectReason, Session};
use chord_core::ws::{
    GatewayPipe, GatewayUrl, RunOutcome, TransportError, connect_and_run, encode_heartbeat,
    encode_qos_heartbeat,
};
use futures_util::{SinkExt, StreamExt};
use std::fs;
use std::future::Future;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::error::{Error as WsError, ProtocolError};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
use tokio_tungstenite::{WebSocketStream, accept_async};

/// Тайм-аут любого сетевого кейса (ТЗ RW-04): цикл обязан уложиться.
const CASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Длина строки филлера (ТЗ: ~1536 Б) — больше окна стаба (1 КиБ), поэтому полезные термы
/// выходят из удержания ruzstd.
const FILLER_TEXT_LEN: usize = 1536;

/// Интервал heartbeat стаба (ТЗ, кейс 5): 60 мс — несколько ударов укладываются в 5 с.
const STUB_INTERVAL_MS: i64 = 60;

// ---------------------------------------------------------------------------
// Оракулы и кодирование термов (значения `etf` + известные факты захвата)
// ---------------------------------------------------------------------------

/// Ключ-атом map (как в реальном трафике: `op`/`s`/`d` — атомы).
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

/// Минимальное значение кадра шлюза: `{op: code}`.
fn op_value(code: i64) -> EtfValue {
    map(vec![("op", EtfValue::Int(code))])
}

/// Значение op10 HELLO с заданным интервалом: `{op: 10, d: {heartbeat_interval: interval}}`.
fn hello_value(interval: i64) -> EtfValue {
    map(vec![
        ("op", EtfValue::Int(10)),
        (
            "d",
            map(vec![("heartbeat_interval", EtfValue::Int(interval))]),
        ),
    ])
}

/// Минимальное значение диспатча: `{op: 0, s: seq}`.
fn dispatch_value(seq: i64) -> EtfValue {
    map(vec![("op", EtfValue::Int(0)), ("s", EtfValue::Int(seq))])
}

/// Форма провода: `0x83` + терм.
fn wire(value: &EtfValue) -> Vec<u8> {
    etf::encode(value)
}

/// HELLO с интервалом `interval` (форма провода).
fn hello_bytes(interval: i64) -> Vec<u8> {
    wire(&hello_value(interval))
}

/// Диспатч `s = seq` (форма провода).
fn dispatch_bytes(seq: i64) -> Vec<u8> {
    wire(&dispatch_value(seq))
}

/// ACK (op11) — форма провода.
fn ack_bytes() -> Vec<u8> {
    wire(&op_value(11))
}

/// op7 RECONNECT — форма провода.
fn op7_bytes() -> Vec<u8> {
    wire(&op_value(7))
}

/// op1 HEARTBEAT от сервера — форма провода (триггер немедленного удара, G-12).
fn op1_bytes() -> Vec<u8> {
    wire(&op_value(1))
}

/// Сессия в боевом режиме heartbeat: op40 `{seq,qos}` (G-12).
fn qos_session(seed: u64) -> Session {
    Session::with_heartbeat_mode(seed, HeartbeatMode::Qos)
}

/// Разбирает кадр клиента и возвращает `op` (без проверки хвоста — кадры клиента цельные).
fn client_op(data: &[u8]) -> Result<i64, String> {
    let (value, consumed) = etf::decode(data).map_err(|error| format!("кадр клиента: {error}"))?;
    if consumed != data.len() {
        return Err(format!(
            "кадр клиента {} Б: разобран терм на {consumed} Б (хвост)",
            data.len()
        ));
    }
    value
        .get("op")
        .and_then(EtfValue::as_int)
        .ok_or_else(|| format!("кадр клиента без op: {value:?}"))
}

/// Филлер: валидный терм-строка (`0x83` + ETF `Str` 1536 Б) — держит окно ruzstd прокрученным.
fn filler_bytes() -> Vec<u8> {
    wire(&EtfValue::Str("f".repeat(FILLER_TEXT_LEN)))
}

/// Формат `zt`-стаба: **непрерывный** zstd-поток без `finish()` (как у шлюза).
struct StubStream {
    /// C-энкодер zstd поверх `Vec<u8>`; кадр никогда не завершается.
    encoder: zstd::stream::write::Encoder<'static, Vec<u8>>,
    /// Сколько сжатых байт уже отдано потребителю.
    sent: usize,
}

impl StubStream {
    /// Энкодер с окном ≈1 КиБ (`WindowLog(10)`): удержание окна видно и на маленьких порциях.
    fn new() -> Self {
        let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1).expect("zstd-энкодер");
        encoder
            .set_parameter(zstd::zstd_safe::CParameter::WindowLog(10))
            .expect("окно zstd 1 КиБ");
        Self { encoder, sent: 0 }
    }

    /// Дописывает произвольные байты провода и завершает блок (`flush`), отдаёт новые байты.
    fn write_raw(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.encoder.write_all(bytes).expect("запись в zstd");
        self.encoder.flush().expect("flush zstd-блока");
        self.take_fresh()
    }

    /// Терм + филлер одним фрагментом (обычный шаг стаба).
    fn write_frame(&mut self, term: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(term.len() + FILLER_TEXT_LEN + 6);
        bytes.extend_from_slice(term);
        bytes.extend_from_slice(&filler_bytes());
        self.write_raw(&bytes)
    }

    /// Новые сжатые байты (без завершения кадра).
    fn take_fresh(&mut self) -> Vec<u8> {
        let buffer = self.encoder.get_mut();
        let fresh = buffer[self.sent..].to_vec();
        self.sent = buffer.len();
        fresh
    }
}

// ---------------------------------------------------------------------------
// Стаб-сервер на 127.0.0.1 (WS-handshake + сценарий обработчика)
// ---------------------------------------------------------------------------

/// WS-сокет стаба (plain TCP: TLS в тестах не поднимается — только локальный `ws://`).
type StubSocket = WebSocketStream<tokio::net::TcpStream>;

/// Запускает стаб-сервер на `127.0.0.1:0` и возвращает его `ws://`-URL и задачу-сценарий.
async fn spawn_stub<T, F, Fut>(handler: F) -> (String, tokio::task::JoinHandle<Result<T, String>>)
where
    F: FnOnce(StubSocket) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, String>> + Send,
    T: Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("стаб: bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("стаб: local_addr");
    let task = tokio::spawn(async move {
        let (tcp, _) = listener
            .accept()
            .await
            .map_err(|error| format!("accept: {error}"))?;
        let ws = accept_async(tcp)
            .await
            .map_err(|error| format!("WS-handshake: {error}"))?;
        handler(ws).await
    });
    (format!("ws://{addr}"), task)
}

/// Ждёт завершения стаба (не дольше 5 с) и разворачивает результат сценария.
async fn stub_result<T>(task: tokio::task::JoinHandle<Result<T, String>>) -> T {
    timeout(CASE_TIMEOUT, task)
        .await
        .expect("стаб-задача не завершилась за 5 с")
        .expect("стаб-задача упала")
        .expect("сценарий стаба вернул ошибку")
}

/// Ошибка слоя WS/TCP → текст для сценария стаба.
fn io_err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Обрыв клиентом без close-handshake — ожидаемый конец сценария после `Reconnect`.
///
/// По контракту ТЗ RW-04 на `Reconnect` цикл возвращает исход, close-кадром не отвечая:
/// соединение рвётся по `drop`, стаб видит FIN (`ResetWithoutClosingHandshake`) либо в редкой
/// гонке RST (`ConnectionReset`). Прочие ошибки слоя остаются ошибкой сценария (не глушатся).
fn is_expected_drop(error: &WsError) -> bool {
    match error {
        WsError::Protocol(ProtocolError::ResetWithoutClosingHandshake) => true,
        WsError::Io(io_error) => io_error.kind() == ErrorKind::ConnectionReset,
        _ => false,
    }
}

/// Отправляет фрагмент WS-сообщением Binary.
async fn send_binary(ws: &mut StubSocket, bytes: Vec<u8>) -> Result<(), String> {
    ws.send(Message::binary(bytes)).await.map_err(io_err)
}

/// Ждёт от клиента `Pong` с ожидаемым payload (не дольше `limit`); прочие кадры пропускает.
///
/// Отсутствие или чужой payload `Pong` — ошибка сценария (не молчание): мутация «клиент не
/// отвечает на Ping» обязана валить кейс. Дедлайн — чтобы падение было быстрым: стаб при
/// провале закрывает соединение (клиент завершается), а не ждёт `CASE_TIMEOUT`.
async fn expect_pong(ws: &mut StubSocket, expected: &[u8], limit: Duration) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await {
            Ok(Some(Ok(Message::Pong(payload)))) => {
                if payload.as_ref() == expected {
                    return Ok(());
                }
                return Err(format!("Pong с чужим payload: {payload:?}"));
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(error))) => return Err(io_err(error)),
            Ok(None) => return Err("соединение закрыто до Pong".to_string()),
            Err(_elapsed) => {
                return Err(format!("Pong не получен за {} мс", limit.as_millis()));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Lab-файлы (skip-aware)
// ---------------------------------------------------------------------------

/// Lab-корень: `RE_LAB_ROOT` или `/home/sanya/re-deep` (как в тестах RW-02/RW-03).
fn lab_root() -> PathBuf {
    PathBuf::from(
        std::env::var("RE_LAB_ROOT").unwrap_or_else(|_| "/home/sanya/re-deep".to_string()),
    )
}

/// Текст lab-файла; нет файла → `None` (skip-aware), прочие ошибки чтения не глотаются.
fn read_opt(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("lab-файл {}: {error}", path.display()),
    }
}

/// Строка hex → байты (пробелы игнорируются).
fn unhex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(compact.len() % 2, 0, "hex нечётной длины: {s}");
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Последний `s` из op0-строк таймлайна (`i \t ts \t op \t s \t t`) — независимый оракул.
fn timeline_last_dispatch_seq(text: &str) -> Option<u64> {
    let mut last = None;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 5 || fields[2] != "0" {
            continue;
        }
        if let Ok(seq) = fields[3].parse::<u64>() {
            last = Some(seq);
        }
    }
    last
}

// ---------------------------------------------------------------------------
// Юнит-кейсы пайпа (без сокетов: zstd-байты строит энкодер стаба)
// ---------------------------------------------------------------------------

#[test]
fn pipe_hello_from_encoded_stream() {
    // HELLO (реальный интервал 41 250) + филлер → сессия знает интервал, эффектов нет,
    // счётчики приёма растут.
    let mut stub = StubStream::new();
    let first = stub.write_frame(&hello_bytes(41_250));
    let mut pipe = GatewayPipe::new(Session::new(0));
    let effects = pipe
        .push_binary(&first, 0)
        .expect("HELLO из zstd-потока обязан разобраться");
    assert!(effects.is_empty(), "HELLO сам по себе не даёт эффектов");
    assert_eq!(
        pipe.session()
            .hello()
            .map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "интервал из HELLO"
    );
    assert_eq!(pipe.frames_received(), 1, "одно WS-сообщение");
    assert_eq!(
        pipe.bytes_received(),
        first.len() as u64,
        "сжатые байты кадра учтены"
    );
    assert!(
        pipe.terms_parsed() >= 1,
        "HELLO разобран: термов {}",
        pipe.terms_parsed()
    );
    // Вторая порция — только филлеры: эффектов по-прежнему нет, счётчики растут.
    let second = stub.write_frame(&filler_bytes());
    let effects = pipe.push_binary(&second, 1).expect("филлеры");
    assert!(effects.is_empty(), "филлеры не дают эффектов");
    assert_eq!(pipe.frames_received(), 2, "два WS-сообщения");
    assert_eq!(
        pipe.bytes_received(),
        (first.len() + second.len()) as u64,
        "сумма сжатых байт"
    );
    assert!(
        pipe.terms_parsed() > 1,
        "филлеры доразобраны: термов {}",
        pipe.terms_parsed()
    );
    assert_eq!(
        pipe.session()
            .hello()
            .map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "интервал не изменился"
    );
}

#[test]
fn pipe_buffers_partial_term() {
    // Терм разрезан между двумя WS-сообщениями: до второго push диспатча нет,
    // после — распознан ровно один раз (last_seq = 1) и хвост не переразбирается.
    let mut stub = StubStream::new();
    let term = dispatch_bytes(1);
    let split = term.len() / 2;
    let first = stub.write_raw(&term[..split]);
    let mut pipe = GatewayPipe::new(Session::new(0));
    let effects = pipe.push_binary(&first, 0).expect("первая половина терма");
    assert!(effects.is_empty(), "неполный терм не даёт эффектов");
    assert_eq!(
        pipe.session().last_seq(),
        None,
        "неполный диспатч не распознан"
    );
    assert_eq!(
        pipe.pending_bytes(),
        split,
        "распакованная половина терма ждёт в хвосте пайпа (F-GW-01: окно больше не держит байты)"
    );
    assert_eq!(pipe.terms_parsed(), 0, "термов ещё нет");

    let second = stub.write_frame(&term[split..]);
    let effects = pipe.push_binary(&second, 5).expect("вторая половина терма");
    assert!(effects.is_empty(), "диспатч не даёт эффектов");
    assert_eq!(
        pipe.session().last_seq(),
        Some(1),
        "терм собран из двух push и распознан"
    );
    assert!(
        pipe.terms_parsed() >= 1,
        "разобран хотя бы диспатч: термов {}",
        pipe.terms_parsed()
    );
    let parsed_after_second = pipe.terms_parsed();

    // Третья порция (только филлеры): диспатч не переразбирается, хвост двигается дальше.
    let third = stub.write_frame(&filler_bytes());
    let effects = pipe.push_binary(&third, 9).expect("филлеры");
    assert!(effects.is_empty(), "филлеры не дают эффектов");
    assert_eq!(
        pipe.session().last_seq(),
        Some(1),
        "повторного диспатча нет: s остаётся 1"
    );
    assert!(
        pipe.terms_parsed() > parsed_after_second,
        "разбор продолжился: {} → {}",
        parsed_after_second,
        pipe.terms_parsed()
    );
}

#[test]
fn pipe_missing_magic_is_error() {
    // Первый push без магии zstd-кадра — ошибка терминального декодера (не паника).
    let mut pipe = GatewayPipe::new(Session::new(0));
    let garbage = [0x00, 0x83, 0x74, 0x00, 0x00];
    match pipe.push_binary(&garbage, 0) {
        Err(TransportError::Zstd(ZstdError::MissingMagic)) => {}
        other => panic!("ожидался Zstd(MissingMagic), получено {other:?}"),
    }
    // Счётчики приёма не откатываются: сообщение пришло, ошибка — на декодере.
    assert_eq!(pipe.frames_received(), 1, "кадр учтён");
    assert_eq!(pipe.bytes_received(), garbage.len() as u64, "байты учтены");
    assert_eq!(pipe.terms_parsed(), 0, "термов нет");
    assert_eq!(pipe.session().last_seq(), None, "сессия не тронута");
}

#[test]
fn pipe_desync_tail_is_error() {
    // Распакованный хвост начинается не с маркера терма (`0x83`) — рассинхронизация потока:
    // ошибка кадра с диагностикой, а не молчаливый сброс (защитный инвариант пайпа).
    // Подача > окна ruzstd (1 КиБ), иначе хвост удержан и до проверки не дойдёт.
    let mut stub = StubStream::new();
    let garbage = vec![0x66u8; 8192];
    let first = stub.write_raw(&garbage);
    let mut pipe = GatewayPipe::new(Session::new(0));
    match pipe.push_binary(&first, 0) {
        Err(TransportError::Frame(GatewayError::Decode { msg })) => {
            assert!(
                msg.contains("рассинхронизирован") && msg.contains("0x66"),
                "диагностика рассинхрона с первым байтом: {msg}"
            );
        }
        other => panic!("ожидался Frame(Decode) о рассинхроне, получено {other:?}"),
    }
    assert_eq!(pipe.frames_received(), 1, "кадр учтён");
    assert_eq!(
        pipe.bytes_received(),
        first.len() as u64,
        "сжатые байты учтены"
    );
    assert_eq!(pipe.terms_parsed(), 0, "термов нет");
    assert!(
        pipe.pending_bytes() > 0,
        "рассинхронизированный хвост остался в пайпе"
    );
}

#[test]
fn gateway_url_builds_canonical_endpoint() {
    // G-01/G-02/G-03: база GATEWAY_ENDPOINT + `encoding=etf&v=9&compress=zstd-stream`.
    let default = GatewayUrl::default();
    assert_eq!(
        default.as_str(),
        "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream",
        "канонический URL шлюза"
    );
    // База без схемы нормализуется в wss (как GLOBAL_ENV.GATEWAY_ENDPOINT).
    let bare = GatewayUrl::from_base("gateway.discord.gg").expect("база без схемы");
    assert_eq!(bare, default, "схема по умолчанию — wss");
}

#[test]
fn gateway_url_normalizes_resume_url() {
    // G-16: хвостовой `/` обрезается, параметры приводит клиент (`new URL(l)` + append).
    let with_slash = GatewayUrl::resume("wss://gateway-us-east1-d.discord.gg/")
        .expect("resume-URL с хвостовым слэшем");
    assert_eq!(
        with_slash.as_str(),
        "wss://gateway-us-east1-d.discord.gg/?encoding=etf&v=9&compress=zstd-stream",
        "хвостовой слэш не удваивается"
    );

    // Параметры из READY не доверяем: канонический набор один, без дублей.
    let with_query = GatewayUrl::resume(
        "wss://gateway-us-east1-d.discord.gg?encoding=json&v=6&compress=zlib-stream&x=1",
    )
    .expect("resume-URL с query");
    assert_eq!(
        with_query.as_str(),
        "wss://gateway-us-east1-d.discord.gg/?encoding=etf&v=9&compress=zstd-stream",
        "query заменяется каноническим"
    );

    // `https://` в READY — схема WS-транспорта: wss.
    let https = GatewayUrl::resume("https://gateway.discord.gg/").expect("https → wss");
    assert_eq!(
        https.as_str(),
        "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream",
        "https маппится в wss"
    );

    // Локальный стаб: ws сохраняется (тесты без TLS), порт/путь не теряются.
    let local = GatewayUrl::from_base("ws://127.0.0.1:8080/").expect("ws стаба");
    assert_eq!(
        local.as_str(),
        "ws://127.0.0.1:8080/?encoding=etf&v=9&compress=zstd-stream",
        "ws и порт сохранены"
    );
}

#[test]
fn gateway_url_rejects_bad_input() {
    // Ошибки — значения, не паники: пустой URL, чужая схема, пустой хост, пробелы, фрагмент.
    for raw in [
        "",
        "   ",
        "ftp://gateway.discord.gg",
        "wss://",
        "wss:///",
        "wss://a b",
    ] {
        assert!(
            GatewayUrl::from_base(raw).is_err(),
            "URL {raw:?} обязан быть отвергнут"
        );
    }
    assert!(
        GatewayUrl::resume("wss://gateway.discord.gg/#frag").is_err(),
        "фрагмент — не часть адреса шлюза"
    );
}

#[test]
fn qos_heartbeat_encode_shape() {
    // G-12: op40, d = {seq, qos}; qos = null до первого active-состояния.
    let bytes = encode_qos_heartbeat(0, None);
    assert_eq!(bytes[0], 0x83, "ведущий маркер провода");
    let (value, consumed) = etf::decode(&bytes).expect("op40 разбирается кодеком");
    assert_eq!(consumed, bytes.len(), "терм занимает весь буфер");
    assert_eq!(
        value.get("op").and_then(EtfValue::as_int),
        Some(40),
        "op = QOS_HEARTBEAT"
    );
    let d = value.get("d").expect("payload под d");
    assert_eq!(
        d.get("seq").and_then(EtfValue::as_int),
        Some(0),
        "seq = 0 до первого диспатча (как this.seq у клиента)"
    );
    assert_eq!(d.get("qos"), Some(&EtfValue::Nil), "qos = null");

    // Активное состояние: {active: true, reasons: [...]}.
    let qos = QosPayload {
        active: true,
        reasons: vec!["COLD_START".to_string(), "USER".to_string()],
    };
    let bytes = encode_qos_heartbeat(7, Some(&qos));
    let (value, _) = etf::decode(&bytes).expect("op40 с qos");
    let d = value.get("d").expect("payload под d");
    assert_eq!(d.get("seq").and_then(EtfValue::as_int), Some(7), "seq = 7");
    let payload = d.get("qos").expect("qos");
    assert_eq!(
        payload.get("active").and_then(EtfValue::as_atom),
        Some("true"),
        "active — ETF-атом true"
    );
    let reasons = payload
        .get("reasons")
        .and_then(EtfValue::as_list)
        .expect("reasons — список");
    assert_eq!(reasons.len(), 2, "обе причины сохранены");
    assert!(
        bytes.len() <= 128,
        "кадр op40 ≤ 128 Б, получено {}",
        bytes.len()
    );
}

#[test]
fn heartbeat_encode_roundtrip() {
    // Кадр удара — `{op: 1, d}` с ведущим `0x83`, компактный, разбирается кодеком.
    let with_seq = encode_heartbeat(Some(7));
    assert_eq!(with_seq[0], 0x83, "ведущий маркер провода");
    assert!(
        with_seq.len() <= 64,
        "терм heartbeat ≤ 64 Б, получено {}",
        with_seq.len()
    );
    let (value, consumed) = etf::decode(&with_seq).expect("encode_heartbeat → etf::decode");
    assert_eq!(consumed, with_seq.len(), "терм занимает весь буфер");
    assert_eq!(
        value.get("op").and_then(EtfValue::as_int),
        Some(1),
        "op = HEARTBEAT"
    );
    assert_eq!(
        value.get("d").and_then(EtfValue::as_int),
        Some(7),
        "d = последний s"
    );

    // d = null (до первого диспатча): NIL_EXT, а не атом "nil".
    let without_seq = encode_heartbeat(None);
    assert_eq!(without_seq[0], 0x83, "ведущий маркер провода");
    assert!(
        without_seq.len() <= 64,
        "терм heartbeat без d ≤ 64 Б, получено {}",
        without_seq.len()
    );
    let (value, consumed) = etf::decode(&without_seq).expect("heartbeat с d = null");
    assert_eq!(consumed, without_seq.len(), "терм занимает весь буфер");
    assert_eq!(
        value.get("op").and_then(EtfValue::as_int),
        Some(1),
        "op = HEARTBEAT"
    );
    assert_eq!(value.get("d"), Some(&EtfValue::Nil), "d = null");
}

// ---------------------------------------------------------------------------
// Стаб-кейсы: реальные сокеты 127.0.0.1 (T-GW-01: HELLO → heartbeat → ACK)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ws_hello_and_heartbeats_acked() {
    // Стаб: HELLO (интервал 60 мс) → отвечает ACK на удары; после трёх ударов закрывает
    // соединение. Клиент: ≥2 удара op40, RTT измерен по ACK, исход — Closed.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let mut beats = 0u64;
        while let Some(message) = ws.next().await {
            match message.map_err(io_err)? {
                Message::Binary(data) => {
                    if client_op(&data)? != 40 {
                        return Err(format!("от клиента ждали op40, пришло {:?}", data));
                    }
                    beats += 1;
                    send_binary(&mut ws, stream.write_frame(&ack_bytes())).await?;
                    if beats >= 3 {
                        ws.close(None).await.map_err(io_err)?;
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Ok(beats)
    })
    .await;
    let mut pipe = GatewayPipe::new(qos_session(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("цикл транспорта не уложился в 5 с");
    let beats = stub_result(stub).await;
    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "стаб закрыл соединение без кода"
    );
    assert!(beats >= 2, "стаб принял {beats} heartbeat (ожидалось ≥ 2)");
    assert!(
        pipe.session().beats_emitted() >= 2,
        "сессия отправила {} ударов",
        pipe.session().beats_emitted()
    );
    let rtt = pipe.session().rtt_ms();
    assert!(rtt.is_some(), "RTT измерен по ACK: {rtt:?}");
    assert!(
        rtt.unwrap_or(u64::MAX) <= 1000,
        "RTT локального сокета ≤ 1000 мс, получено {rtt:?}"
    );
}

#[tokio::test]
async fn ws_ping_gets_pong() {
    // WS Ping — не кадр шлюза, но транспорт обязан ответить Pong с тем же payload (RFC 6455
    // §5.5.2). После ответа стаб чисто закрывает соединение → Closed.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(41_250))).await?;
        ws.send(Message::Ping(b"probe".to_vec().into()))
            .await
            .map_err(io_err)?;
        if let Err(error) = expect_pong(&mut ws, b"probe", Duration::from_millis(700)).await {
            // Клиенту нужно закрытие, иначе он будет ждать CASE_TIMEOUT; ошибку не глотаем.
            let _ = ws.close(None).await;
            return Err(error);
        }
        ws.close(None).await.map_err(io_err)?;
        Ok(())
    })
    .await;
    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("Ping/Pong не обработан за 5 с");
    stub_result(stub).await;
    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "после Pong стаб закрыл соединение без кода"
    );
    assert_eq!(
        pipe.session()
            .hello()
            .map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "HELLO обработан до Ping"
    );
    assert_eq!(
        pipe.ignored_messages(),
        0,
        "Ping — обработанный служебный кадр, не счётчик игнора"
    );
}

#[tokio::test]
async fn ws_text_messages_counted() {
    // Text шлюз не присылает (спека §3: server→client — бинарь), но молча его пропускать нельзя:
    // цикл обязан учесть сообщение в ignored_messages (наблюдаемость), не ломая соединение.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(41_250))).await?;
        ws.send(Message::text("probe")).await.map_err(io_err)?;
        ws.close(None).await.map_err(io_err)?;
        Ok(())
    })
    .await;
    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("Text/Close не обработаны за 5 с");
    stub_result(stub).await;
    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "Text не мешает чистому закрытию"
    );
    assert_eq!(
        pipe.ignored_messages(),
        1,
        "Text-сообщение учтено счётчиком"
    );
    assert_eq!(
        pipe.session()
            .hello()
            .map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "HELLO обработан до Text"
    );
}

#[tokio::test]
async fn ws_zombie_without_ack() {
    // Стаб молчит на удары (ACK не шлёт). Честный таймлайн: первый удар — сразу после HELLO
    // (jitter 0), дедлайн зомби = удар + интервал, поэтому при интервале 300 мс детект обязан
    // уложиться в 500 мс (запас 200 мс на планировщик). Систематическая задержка таймера ×2
    // (детект ≈600 мс) этим порогом ловится.
    const ZOMBIE_INTERVAL_MS: i64 = 300;
    const ZOMBIE_LIMIT: Duration = Duration::from_millis(500);
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(
            &mut ws,
            stream.write_frame(&hello_bytes(ZOMBIE_INTERVAL_MS)),
        )
        .await?;
        let mut beats = 0u64;
        loop {
            match ws.next().await {
                Some(Ok(Message::Binary(_))) => beats += 1,
                Some(Ok(Message::Close(_))) | None => break,
                // После Reconnect клиент по контракту ТЗ бросает соединение без close-кадра.
                Some(Err(error)) if is_expected_drop(&error) => break,
                Some(Err(error)) => return Err(io_err(error)),
                Some(Ok(_)) => {}
            }
        }
        Ok(beats)
    })
    .await;
    let mut pipe = GatewayPipe::new(qos_session(0));
    let started = Instant::now();
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("зомби-детектор не уложился в 5 с");
    let detected = started.elapsed();
    let beats = stub_result(stub).await;
    assert_eq!(
        outcome,
        Ok(RunOutcome::Reconnect {
            reason: ReconnectReason::Zombie
        }),
        "без ACK — зомби"
    );
    assert!(
        detected <= ZOMBIE_LIMIT,
        "детект зомби ≤ {ZOMBIE_LIMIT:?} (пунктуальность таймера), занял {detected:?}"
    );
    assert!(pipe.session().is_dead(), "сессия мертва");
    assert_eq!(beats, 1, "второго удара сервер не получил");
}

#[tokio::test]
async fn ws_legacy_heartbeat_op1_fallback() {
    // Fallback (feature/флаг): legacy-сессия шлёт op1 — старый контракт RW-03/04 сохранён.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        while let Some(message) = ws.next().await {
            match message.map_err(io_err)? {
                Message::Binary(data) => {
                    if client_op(&data)? != 1 {
                        return Err(format!("legacy: ждали op1, пришло {:?}", data));
                    }
                    ws.close(None).await.map_err(io_err)?;
                    return Ok(());
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Err("legacy: клиент не прислал op1".to_string())
    })
    .await;
    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("legacy-heartbeat не уложился в 5 с");
    stub_result(stub).await;
    assert_eq!(outcome, Ok(RunOutcome::Closed { code: None }));
    assert_eq!(pipe.session().beats_emitted(), 1, "один op1-удар");
}

#[tokio::test]
async fn ws_server_op1_triggers_immediate_qos_heartbeat() {
    // G-12: op1 от сервера — триггер немедленного op40, не дожидаясь интервала.
    // Первый плановый удар отодвинут сидом (59 000 < 60 000), поэтому кадр не спутать.
    const LONG_INTERVAL_MS: i64 = 60_000;
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(LONG_INTERVAL_MS))).await?;
        // Даём клиенту время получить HELLO и убедиться, что планового удара нет.
        tokio::time::sleep(Duration::from_millis(100)).await;
        send_binary(&mut ws, stream.write_frame(&op1_bytes())).await?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(700);
        loop {
            match tokio::time::timeout_at(deadline, ws.next()).await {
                Ok(Some(Ok(Message::Binary(data)))) => {
                    if client_op(&data)? == 40 {
                        ws.close(None).await.map_err(io_err)?;
                        return Ok(());
                    }
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(error))) => return Err(io_err(error)),
                Ok(None) => return Err("соединение закрыто без op40".to_string()),
                Err(_) => return Err("op40 после op1 не пришёл за 700 мс".to_string()),
            }
        }
    })
    .await;
    let mut pipe = GatewayPipe::new(qos_session(59_000));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("op1-триггер не уложился в 5 с");
    stub_result(stub).await;
    assert_eq!(outcome, Ok(RunOutcome::Closed { code: None }));
    assert_eq!(
        pipe.session().beats_emitted(),
        1,
        "ровно один немедленный удар"
    );
    assert_eq!(
        pipe.session()
            .hello()
            .map(|hello| hello.heartbeat_interval_ms),
        Some(LONG_INTERVAL_MS as u64),
        "интервал из HELLO"
    );
}

#[tokio::test]
async fn ws_server_reconnect_op7() {
    // op7 от сервера → Reconnect{ServerRequest} без паники; соединение держится до ответа клиента.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(41_250))).await?;
        send_binary(&mut ws, stream.write_frame(&op7_bytes())).await?;
        loop {
            match ws.next().await {
                Some(Ok(Message::Close(_))) | None => break,
                // Reconnect: клиент бросает соединение без close-кадра (контракт ТЗ).
                Some(Err(error)) if is_expected_drop(&error) => break,
                Some(Err(error)) => return Err(io_err(error)),
                Some(Ok(_)) => {}
            }
        }
        Ok(())
    })
    .await;
    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("op7 не обработан за 5 с");
    stub_result(stub).await;
    assert_eq!(
        outcome,
        Ok(RunOutcome::Reconnect {
            reason: ReconnectReason::ServerRequest
        }),
        "op7 → ServerRequest"
    );
    assert!(pipe.session().is_dead(), "сессия мертва");
}

#[tokio::test]
async fn ws_dispatch_updates_heartbeat_seq() {
    // Диспатчи s=1..3 до удара → удар несёт d = 3; стаб видит это в несжатом ETF-кадре клиента.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        for seq in 1..=3i64 {
            send_binary(&mut ws, stream.write_frame(&dispatch_bytes(seq))).await?;
        }
        let mut seen: Vec<u64> = Vec::new();
        'beats: while let Some(message) = ws.next().await {
            match message.map_err(io_err)? {
                Message::Binary(data) => {
                    let (value, _) =
                        etf::decode(&data).map_err(|error| format!("heartbeat: {error}"))?;
                    if value.get("op").and_then(EtfValue::as_int) != Some(40) {
                        return Err(format!("от клиента ждали op40, пришло {value:?}"));
                    }
                    let d = value
                        .get("d")
                        .and_then(|d| d.get("seq"))
                        .and_then(EtfValue::as_int)
                        .and_then(|raw| u64::try_from(raw).ok())
                        .ok_or_else(|| format!("op40 без d.seq: {value:?}"))?;
                    seen.push(d);
                    send_binary(&mut ws, stream.write_frame(&ack_bytes())).await?;
                    if d == 3 {
                        break 'beats;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        ws.close(None).await.map_err(io_err)?;
        Ok(seen)
    })
    .await;
    let mut pipe = GatewayPipe::new(qos_session(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("цикл транспорта не уложился в 5 с");
    let seen = stub_result(stub).await;
    assert!(
        seen.contains(&3),
        "стаб не увидел op40 с d.seq = 3: {seen:?}"
    );
    assert_eq!(pipe.session().last_seq(), Some(3), "диспатчи s=1..3");
    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "после стаб-закрытия — Closed"
    );
}

#[tokio::test]
async fn ws_closed_after_stub_close() {
    // Чистое закрытие стабом с кодом из кадра (CloseCode::Normal): клиент обязан вернуть
    // именно 1000 — деградация кода закрытия должна быть видна тесту, а не тонуть в допуске.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(41_250))).await?;
        ws.close(Some(CloseFrame {
            code: CloseCode::Normal,
            reason: "bye".into(),
        }))
        .await
        .map_err(io_err)?;
        Ok(())
    })
    .await;
    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("закрытие не обработано за 5 с");
    stub_result(stub).await;
    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: Some(1000) }),
        "стаб прислал CloseCode::Normal — клиент отдаёт код 1000 из close-кадра"
    );
}

// ---------------------------------------------------------------------------
// Lab-кейс: реальный снимок потока шлюза (skip-aware)
// ---------------------------------------------------------------------------

#[test]
fn lab_capture_stream_feeds_pipe() {
    // 137 кадров снимка — фрагменты ОДНОГО zstd-кадра: пайп разбирает термы без ошибок.
    let path = lab_root().join("gw-proto/zstd-frames-95974_1389.hexl");
    let Some(text) = read_opt(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    let frames: Vec<Vec<u8>> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(unhex)
        .collect();
    assert_eq!(frames.len(), 137, "кадров в снимке потока");
    let compressed: u64 = frames.iter().map(|frame| frame.len() as u64).sum();
    assert_eq!(compressed, 104_066, "сжатых байт в снимке");

    let mut pipe = GatewayPipe::new(Session::new(0));
    for (index, frame) in frames.iter().enumerate() {
        let effects = pipe
            .push_binary(frame, index as u64)
            .unwrap_or_else(|error| panic!("кадр {index}: {error}"));
        assert!(
            effects.is_empty(),
            "кадр {index}: поток шлюза не даёт эффектов"
        );
    }
    assert_eq!(pipe.frames_received(), 137, "все кадры поданы");
    assert_eq!(pipe.bytes_received(), compressed, "поданы все сжатые байты");
    assert_eq!(
        pipe.session()
            .hello()
            .map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "HELLO — в первой порции потока"
    );

    // F-GW-01: незавершённый кадр больше не удерживает окно — пайп получает все 824 816 Б,
    // то есть все 137 термов (s = 1..113), и хвоста не остаётся.
    assert_eq!(
        pipe.terms_parsed(),
        137,
        "полных термов в выданных 824 816 Б"
    );
    assert_eq!(
        pipe.session().last_seq(),
        Some(113),
        "последний доставленный диспатч — s=113"
    );
    assert_eq!(
        pipe.pending_bytes(),
        0,
        "хвоста нет: поток разобран целиком"
    );

    // Независимый оракул: таймлайн того же сокета знает все 113 диспатчей — пайп доставляет
    // их все, ничего не теряя на границе окна.
    let timeline_path = lab_root().join("gw-proto/timeline-95974_1389.tsv");
    match read_opt(&timeline_path) {
        Some(timeline) => assert_eq!(
            timeline_last_dispatch_seq(&timeline),
            Some(113),
            "в полном потоке 113 диспатчей (s = 1..113) — пайп видит их все"
        ),
        None => eprintln!("skip-деталь: нет lab-файла {}", timeline_path.display()),
    }
}
