//! Resume/реконнект шлюза: RESUME (op6), политика resume, флаг op9 и непрерывность `s`
//! (волна RW-06, P1-05 / T-GW-05…07).
//!
//! **Провенанс оракулов** (независимые артефакты, не подгонка под реализацию):
//! - `rest-static.md` §4: RESUME `{token, session_id, seq}`; «при INVALID_SESSION (false) →
//!   IDENTIFY»; `_doResume`/`GatewaySocket._connect` бандла веб-клиента.
//! - run2-артефакты `gw-proto/msgs-run2-95974_{8269,8608,10029,13072}.jsonl`: resume #1
//!   (s 95..663, 569 op0, RESUMED при 96), resume #2 (s 664..722, 59 op0, RESUMED при 668),
//!   свежий логин .10029 (s 1..331, есть op7), resume #3 .13072 (первый op0 = RESUMED s=332,
//!   s 332..371, 40 op0); швы серий: 663+1=664 и 331+1=332; исходящие RESUME — 173/176 Б.
//! - стаб-сервер: только `127.0.0.1`, все сетевые кейсы — под `tokio::time::timeout(5 с)`.
//!
//! Lab-файлы опциональны: нет файла — skip с причиной, а не падение.

use chord_core::codec::etf::{self, EtfValue};
use chord_core::gateway::{ReconnectReason, Session};
use chord_core::identify::{CAPABILITIES_ACTIVE, IdentifyParams, IdentifyProperties};
use chord_core::resume::{
    RESUME_MAX_DOWNTIME_MS, ReconnectAction, ResumeError, ResumeParams, ResumeState,
    build_resume_frame, plan_reconnect,
};
use chord_core::ws::{
    GatewayPipe, RunOptions, RunOutcome, connect_and_run_resume, connect_and_run_with,
};
use futures_util::{SinkExt, StreamExt};
use std::fs;
use std::future::Future;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{WebSocketStream, accept_async};

/// Тайм-аут любого сетевого кейса (как в RW-04/RW-05): цикл обязан уложиться.
const CASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Интервал heartbeat стаба: 60 мс.
const STUB_INTERVAL_MS: i64 = 60;

/// Длина строки филлера (как в RW-04/RW-05): больше окна ruzstd стаба.
const FILLER_TEXT_LEN: usize = 1536;

/// session_id из run2-READY (сокет `.1389`, 32 символа) — общий для стабов и лаб-кейса.
const LAB_SESSION_ID: &str = "f14ac1f3d408c15027d6d98da172c398";

// ---------------------------------------------------------------------------
// Оракульные числа из захвата (перепроверены координатором по jsonl)
// ---------------------------------------------------------------------------

/// Артефакты run2, на которых стоит T-GW-05.
const LAB_FILES: [&str; 4] = [
    "msgs-run2-95974_8269.jsonl",
    "msgs-run2-95974_8608.jsonl",
    "msgs-run2-95974_10029.jsonl",
    "msgs-run2-95974_13072.jsonl",
];

// ---------------------------------------------------------------------------
// ETF-помощники (формы как в RW-05)
// ---------------------------------------------------------------------------

/// Ключ/значение-атом map (формы сервера: `op`/`t`/`d`).
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

/// Список (`LIST_EXT` с хвостовым `NIL_EXT` — форма массивов в захвате).
fn list(items: Vec<EtfValue>) -> EtfValue {
    EtfValue::List(items)
}

/// Форма провода: `0x83` + терм.
fn wire(value: &EtfValue) -> Vec<u8> {
    etf::encode(value)
}

/// Разбирает терм так, как это делает пайп: ведущий `0x83` + ровно один терм на весь буфер.
fn decode_wire(bytes: &[u8]) -> EtfValue {
    assert_eq!(
        bytes.first(),
        Some(&etf::VERSION_BYTE),
        "ведущий маркер 0x83"
    );
    let (value, consumed) = etf::decode(bytes).expect("терм разбирается кодеком");
    assert_eq!(consumed, bytes.len(), "терм занимает весь буфер");
    value
}

/// Имена ключей map (ключ-атом или ключ-строка).
fn key_names(value: &EtfValue) -> Vec<String> {
    value
        .as_map()
        .expect("значение — map")
        .iter()
        .map(|(key, _)| match key {
            EtfValue::Atom(name) | EtfValue::Str(name) => name.clone(),
            other => panic!("нестроковый ключ map: {other:?}"),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Параметры и синтетические термы
// ---------------------------------------------------------------------------

/// Параметры IDENTIFY с дефолтами нашего клиента (для свежих соединений).
fn test_identify_params() -> IdentifyParams {
    IdentifyParams {
        token: "TEST_TOKEN".to_string(),
        capabilities: CAPABILITIES_ACTIVE,
        properties: IdentifyProperties::for_rw_client(),
        presence: None,
        compress: false,
        client_state_guild_versions: Vec::new(),
        qos_token: vec![0x0A, 0x02, 0x08, 0x00],
    }
}

/// Параметры RESUME: seq=94 — как у `.5532` до resume #1 (первый dispatch `.8269` — s=95).
fn test_resume_params() -> ResumeParams {
    ResumeParams {
        token: "TEST_TOKEN".to_string(),
        session_id: LAB_SESSION_ID.to_string(),
        seq: 94,
    }
}

/// READY (op0, s=1) с session_id — свежий логин.
fn ready_bytes(session_id: &str) -> Vec<u8> {
    wire(&map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("READY")),
        ("s", EtfValue::Int(1)),
        (
            "d",
            map(vec![
                ("guilds", list(Vec::new())),
                ("session_id", EtfValue::Str(session_id.to_string())),
            ]),
        ),
    ]))
}

/// READY без session_id (для негативного кейса `resume_state`).
fn ready_without_session_id_bytes() -> Vec<u8> {
    wire(&map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("READY")),
        ("s", EtfValue::Int(1)),
        ("d", map(vec![("guilds", list(Vec::new()))])),
    ]))
}

/// Диспатч op0 с типом и `s`.
fn dispatch_bytes(t: &str, s: i64) -> Vec<u8> {
    wire(&map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom(t)),
        ("s", EtfValue::Int(s)),
        ("d", map(Vec::new())),
    ]))
}

/// RESUMED (op0) — ответ сервера на RESUME.
fn resumed_bytes(s: i64) -> Vec<u8> {
    dispatch_bytes("RESUMED", s)
}

/// op7 RECONNECT (сервер просит переподключиться).
fn op7_bytes() -> Vec<u8> {
    wire(&map(vec![("op", EtfValue::Int(7))]))
}

/// op9 INVALID_SESSION с `d`.
fn op9_bytes(d: EtfValue) -> Vec<u8> {
    wire(&map(vec![("op", EtfValue::Int(9)), ("d", d)]))
}

/// HELLO с интервалом `interval`.
fn hello_bytes(interval: i64) -> Vec<u8> {
    wire(&map(vec![
        ("op", EtfValue::Int(10)),
        (
            "d",
            map(vec![("heartbeat_interval", EtfValue::Int(interval))]),
        ),
    ]))
}

/// Один zstd-фрагмент с данными термами — **завершённый** кадр (пайп отдаёт все термы целиком;
/// незавершённый кадр удерживал бы хвост окна — это проверяют сокет-кейсы со `StubStream`).
fn zstd_message(terms: &[Vec<u8>]) -> Vec<u8> {
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1).expect("zstd-энкодер");
    encoder
        .set_parameter(zstd::zstd_safe::CParameter::WindowLog(10))
        .expect("окно zstd 1 КиБ");
    for term in terms {
        encoder.write_all(term).expect("терм в zstd");
    }
    encoder.finish().expect("завершение zstd-кадра")
}

/// ACK (op11) — форма провода (стаб отвечает на удары, чтобы клиент не стал зомби).
fn ack_bytes() -> Vec<u8> {
    wire(&map(vec![("op", EtfValue::Int(11))]))
}

// ---------------------------------------------------------------------------
// Юнит-кейсы RESUME (без сокетов)
// ---------------------------------------------------------------------------

#[test]
fn resume_frame_has_evidence_fields() {
    // Кейс 1: op6, d = {token, session_id, seq}; ключи/значения — формы wetf.
    let params = test_resume_params();
    let frame = build_resume_frame(&params).expect("RESUME собирается");
    let value = decode_wire(&frame);

    let mut keys = key_names(&value);
    keys.sort();
    assert_eq!(keys, ["d", "op"], "кадр — {{op, d}}");
    assert_eq!(
        value.get("op").and_then(EtfValue::as_int),
        Some(6),
        "op = RESUME"
    );

    let d = value.get("d").expect("payload под d");
    let mut d_keys = key_names(d);
    d_keys.sort();
    assert_eq!(d_keys, ["seq", "session_id", "token"], "поля RESUME");
    assert_eq!(
        d.get("token").and_then(EtfValue::as_str),
        Some("TEST_TOKEN"),
        "токен без изменений"
    );
    assert_eq!(
        d.get("session_id").and_then(EtfValue::as_str),
        Some(LAB_SESSION_ID),
        "session_id из READY"
    );
    assert_eq!(
        d.get("seq").and_then(EtfValue::as_int),
        Some(94),
        "seq — последний принятый s"
    );
}

#[test]
fn resume_frame_size_plausible() {
    // Кейс 2: реальные исходящие RESUME — 173/176 Б (маскированы; размер — ориентир).
    // Наш кадр: токен 59 Б (как у пользователя), session_id 32 Б.
    let params = ResumeParams {
        token: "A".repeat(59),
        session_id: "f".repeat(32),
        seq: 663,
    };
    let size = build_resume_frame(&params)
        .expect("RESUME собирается")
        .len();
    assert!(
        (120..=400).contains(&size),
        "размер RESUME {size} Б вне ориентира 120..=400 (реальные — 173/176 Б)"
    );
}

#[test]
fn resume_debug_redacts_token() {
    // Кейс 3: Debug не должен раскрывать токен.
    let params = test_resume_params();
    let debug = format!("{params:?}");
    assert!(
        !debug.contains("TEST_TOKEN"),
        "Debug не должен содержать токен: {debug}"
    );
    assert!(debug.contains("REDACTED"), "Debug помечает токен: {debug}");
}

#[test]
fn resume_frame_rejects_empty_fields() {
    // Кейс 4: пустые поля — конкретные ошибки без паник (невалидный RESUME не уйдёт на провод).
    let mut params = test_resume_params();
    params.token = String::new();
    assert_eq!(
        build_resume_frame(&params),
        Err(ResumeError::EmptyToken),
        "пустой токен — именно EmptyToken"
    );

    let mut params = test_resume_params();
    params.session_id = String::new();
    assert_eq!(
        build_resume_frame(&params),
        Err(ResumeError::EmptySessionId),
        "пустой session_id — именно EmptySessionId"
    );
}

#[test]
fn plan_reconnect_policy() {
    // Кейс 5 (T-GW-07): 3 с → Resume; 30 с → Identify; op9(false) → Identify.
    let state = ResumeState {
        session_id: LAB_SESSION_ID.to_string(),
        last_seq: 663,
    };
    assert_eq!(
        plan_reconnect(None, 0, None),
        ReconnectAction::Identify,
        "нет состояния сессии — свежий IDENTIFY"
    );
    assert_eq!(
        plan_reconnect(Some(&state), 3_000, None),
        ReconnectAction::Resume,
        "разрыв 3 с — resume"
    );
    assert_eq!(
        plan_reconnect(Some(&state), RESUME_MAX_DOWNTIME_MS, None),
        ReconnectAction::Identify,
        "разрыв 30 с — новый IDENTIFY"
    );
    assert_eq!(
        RESUME_MAX_DOWNTIME_MS, 30_000,
        "порог закреплён контрактом плана (литерал)"
    );
    assert_eq!(
        plan_reconnect(Some(&state), 30_000, None),
        ReconnectAction::Identify,
        "ровно 30 000 мс (литерал) — Identify"
    );
    assert_eq!(
        plan_reconnect(Some(&state), 29_999, None),
        ReconnectAction::Resume,
        "29 999 мс (литерал) — Resume"
    );
    assert_eq!(
        plan_reconnect(Some(&state), 3_000, Some(false)),
        ReconnectAction::Identify,
        "op9 d=false — resume запрещён"
    );
    assert_eq!(
        plan_reconnect(Some(&state), 3_000, Some(true)),
        ReconnectAction::Resume,
        "op9 d=true — resume разрешён"
    );
}

#[test]
fn pipe_resume_state_after_ready() {
    // Кейс 6: READY с session_id и s=1 → состояние resume; без READY/без session_id — None.
    let mut pipe = GatewayPipe::new(Session::new(0));
    pipe.push_binary(&zstd_message(&[ready_bytes(LAB_SESSION_ID)]), 0)
        .expect("READY принят");
    assert_eq!(
        pipe.resume_state(),
        Some(ResumeState {
            session_id: LAB_SESSION_ID.to_string(),
            last_seq: 1,
        }),
        "сессия резюмируема после READY"
    );

    let empty = GatewayPipe::new(Session::new(0));
    assert_eq!(
        empty.resume_state(),
        None,
        "без READY — нечего резюмировать"
    );

    let mut no_id = GatewayPipe::new(Session::new(0));
    no_id
        .push_binary(&zstd_message(&[ready_without_session_id_bytes()]), 0)
        .expect("READY без session_id");
    assert_eq!(
        no_id.resume_state(),
        None,
        "без session_id — resume невозможен"
    );
}

#[test]
fn pipe_invalid_session_flag() {
    // Кейс 7: флаг `d` последнего op9 (атом true/false либо int 1/0).
    for (d, expected) in [
        (atom("false"), Some(false)),
        (atom("true"), Some(true)),
        (EtfValue::Int(0), Some(false)),
    ] {
        let mut pipe = GatewayPipe::new(Session::new(0));
        pipe.push_binary(&zstd_message(&[op9_bytes(d.clone())]), 0)
            .expect("op9 принят");
        assert_eq!(pipe.last_invalid_session_resumable(), expected, "d = {d:?}");
    }

    let fresh = GatewayPipe::new(Session::new(0));
    assert_eq!(
        fresh.last_invalid_session_resumable(),
        None,
        "op9 не приходил — флага нет"
    );
}

// ---------------------------------------------------------------------------
// Lab-кейс: непрерывность `s` по run2-артефактам (skip-aware)
// ---------------------------------------------------------------------------

/// Lab-корень: `RE_LAB_ROOT` или `/home/sanya/re-deep` (как в тестах RW-02/RW-04/RW-05).
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

/// Числовое поле верхнего уровня из jsonl-строки захвата (`"key":123`).
///
/// Формат артефактов фиксирован (см. шапку): `{"idx":N,"op":N,"t":"…","s":N|"nil",…}` —
/// первый `"key":` в строке всегда верхнего уровня, поэтому мини-парсер честен для этих файлов.
fn json_int_field(line: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{key}\":");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    if rest.starts_with('"') {
        return None; // "nil" или строка
    }
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Строковое поле верхнего уровня из jsonl-строки захвата (`"key":"value"`).
fn json_str_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

/// Диспатч op0 из артефакта: `s` и тип `t`.
struct LabDispatch {
    /// Последовательность `s`.
    s: i64,
    /// Тип диспатча (`READY`/`RESUMED`/…).
    t: String,
}

/// Все op0-диспатчи файла в порядке появления.
fn lab_dispatches(text: &str) -> Vec<LabDispatch> {
    text.lines()
        .filter_map(|line| {
            if json_int_field(line, "op") != Some(0) {
                return None;
            }
            Some(LabDispatch {
                s: json_int_field(line, "s")?,
                t: json_str_field(line, "t").unwrap_or_default(),
            })
        })
        .collect()
}

#[test]
fn lab_resume_seq_continuity() {
    // Кейс 12 (T-GW-05): швы и непрерывность `s` по четырём артефактам run2.
    let mut texts = Vec::new();
    for name in LAB_FILES {
        let path = lab_root().join("gw-proto").join(name);
        let Some(text) = read_opt(&path) else {
            eprintln!("skip: нет lab-файла {}", path.display());
            return;
        };
        texts.push(text);
    }
    let d = |i: usize| lab_dispatches(&texts[i]);
    let (resume1, resume2, fresh, resume3) = (d(0), d(1), d(2), d(3));

    // Числа артефактов (оракул).
    assert_eq!(resume1.len(), 569, "resume #1: op0");
    assert_eq!(resume1.first().map(|item| item.s), Some(95));
    assert_eq!(resume1.last().map(|item| item.s), Some(663));
    assert_eq!(resume2.len(), 59, "resume #2: op0");
    assert_eq!(resume2.first().map(|item| item.s), Some(664));
    assert_eq!(resume2.last().map(|item| item.s), Some(722));
    assert_eq!(fresh.len(), 331, "свежий логин .10029: op0");
    assert_eq!(fresh.last().map(|item| item.s), Some(331));
    assert_eq!(resume3.len(), 40, "resume #3: op0");
    assert_eq!(resume3.first().map(|item| item.s), Some(332));
    assert_eq!(resume3.last().map(|item| item.s), Some(371));
    assert_eq!(
        resume3.first().map(|item| item.t.as_str()),
        Some("RESUMED"),
        "resume #3 начинается с RESUMED"
    );

    // Швы серий: ни одного потерянного `s` между сокетами.
    assert_eq!(
        resume1.last().map(|item| item.s + 1),
        resume2.first().map(|item| item.s),
        "шов 663 → 664"
    );
    assert_eq!(
        fresh.last().map(|item| item.s + 1),
        resume3.first().map(|item| item.s),
        "шов 331 → 332"
    );

    // Непрерывность внутри каждого сегмента.
    for (name, dispatches) in [
        ("8269", &resume1),
        ("8608", &resume2),
        ("10029", &fresh),
        ("13072", &resume3),
    ] {
        for pair in dispatches.windows(2) {
            assert_eq!(pair[1].s, pair[0].s + 1, "{name}: разрыв s");
        }
    }

    // Реплей сегментов в сессию: `s` доходит до конца без потерь.
    for (name, dispatches, last) in [("8608", &resume2, 722), ("13072", &resume3, 371)] {
        let mut session = Session::new(0);
        for (now, dispatch) in dispatches.iter().enumerate() {
            let term = map(vec![
                ("op", EtfValue::Int(0)),
                ("t", atom(&dispatch.t)),
                ("s", EtfValue::Int(dispatch.s)),
                ("d", map(Vec::new())),
            ]);
            session
                .on_value(&term, now as u64)
                .unwrap_or_else(|error| panic!("{name}: терм s={}: {error}", dispatch.s));
        }
        assert_eq!(session.last_seq(), Some(last), "{name}: последний s");
    }

    // Реплей .10029 целым непрерывным zstd-кадром через пайп: сессия из READY
    // резюмируема (session_id + последний s), политика даёт Resume при разрыве 3 с.
    let mut terms = vec![ready_bytes(LAB_SESSION_ID)];
    for dispatch in fresh.iter().skip(1) {
        terms.push(wire(&map(vec![
            ("op", EtfValue::Int(0)),
            ("t", atom(&dispatch.t)),
            ("s", EtfValue::Int(dispatch.s)),
            ("d", map(Vec::new())),
        ])));
    }
    let mut pipe = GatewayPipe::new(Session::new(0));
    pipe.push_binary(&zstd_message(&terms), 0)
        .expect("реплей .10029 через пайп");
    assert_eq!(
        pipe.session().last_seq(),
        Some(331),
        "пайп дочитал серию .10029 до конца"
    );
    let state = pipe.resume_state().expect("сессия резюмируема");
    assert_eq!(state.session_id, LAB_SESSION_ID);
    assert_eq!(state.last_seq, 331);
    assert_eq!(
        plan_reconnect(Some(&state), 3_000, None),
        ReconnectAction::Resume,
        "разрыв 3 с — резюмируемся"
    );
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

/// Стаб на **два последовательных соединения**: `h1` обслуживает первое, затем accept второго
/// и `h2` — второе (сценарий реконнекта T-GW-06).
async fn spawn_stub2<T1, T2, F1, F2, Fut1, Fut2>(
    h1: F1,
    h2: F2,
) -> (String, tokio::task::JoinHandle<Result<(T1, T2), String>>)
where
    F1: FnOnce(StubSocket) -> Fut1 + Send + 'static,
    Fut1: Future<Output = Result<T1, String>> + Send,
    T1: Send + 'static,
    F2: FnOnce(StubSocket) -> Fut2 + Send + 'static,
    Fut2: Future<Output = Result<T2, String>> + Send,
    T2: Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("стаб: bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("стаб: local_addr");
    let task = tokio::spawn(async move {
        let (tcp1, _) = listener
            .accept()
            .await
            .map_err(|error| format!("accept #1: {error}"))?;
        let ws1 = accept_async(tcp1)
            .await
            .map_err(|error| format!("WS-handshake #1: {error}"))?;
        let first = h1(ws1).await?;
        let (tcp2, _) = listener
            .accept()
            .await
            .map_err(|error| format!("accept #2: {error}"))?;
        let ws2 = accept_async(tcp2)
            .await
            .map_err(|error| format!("WS-handshake #2: {error}"))?;
        let second = h2(ws2).await?;
        Ok((first, second))
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

/// Отправляет фрагмент WS-сообщением Binary.
async fn send_binary(ws: &mut StubSocket, bytes: Vec<u8>) -> Result<(), String> {
    ws.send(Message::binary(bytes)).await.map_err(io_err)
}

/// Формат `zt`-стаба: **непрерывный** zstd-поток без `finish()` (как у шлюза).
struct StubStream {
    /// C-энкодер zstd поверх `Vec<u8>`; кадр никогда не завершается.
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

    /// Терм + филлер одним фрагментом (обычный шаг стаба).
    fn write_frame(&mut self, term: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(term.len() + FILLER_TEXT_LEN + 6);
        bytes.extend_from_slice(term);
        bytes.extend_from_slice(&{ wire(&EtfValue::Str("f".repeat(FILLER_TEXT_LEN))) });
        self.write_raw(&bytes)
    }

    /// Дописывает произвольные байты провода и завершает блок (`flush`), отдаёт новые байты.
    fn write_raw(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.encoder.write_all(bytes).expect("запись в zstd");
        self.encoder.flush().expect("flush zstd-блока");
        self.take_fresh()
    }

    /// Новые сжатые байты (без завершения кадра).
    fn take_fresh(&mut self) -> Vec<u8> {
        let buffer = self.encoder.get_mut();
        let fresh = buffer[self.sent..].to_vec();
        self.sent = buffer.len();
        fresh
    }
}

/// Разбор кадра клиента: ровно один терм на весь буфер.
fn decode_client_frame(data: &[u8]) -> Result<EtfValue, String> {
    let (value, consumed) = etf::decode(data).map_err(|error| format!("кадр клиента: {error}"))?;
    if consumed != data.len() {
        return Err(format!(
            "кадр клиента {} Б: разобран терм на {consumed} Б (хвост)",
            data.len()
        ));
    }
    Ok(value)
}

/// Поля op6, наблюдаемые стабом.
type ObservedResume = (String, String, i64);

/// Ждёт от клиента ровно один op6 и возвращает его поля `(token, session_id, seq)`.
async fn expect_resume(ws: &mut StubSocket) -> Result<ObservedResume, String> {
    while let Some(message) = ws.next().await {
        match message.map_err(io_err)? {
            Message::Binary(data) => {
                let value = decode_client_frame(&data)?;
                match value.get("op").and_then(EtfValue::as_int) {
                    Some(6) => {
                        let d = value.get("d").cloned().expect("d RESUME");
                        let token = d
                            .get("token")
                            .and_then(EtfValue::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let session_id = d
                            .get("session_id")
                            .and_then(EtfValue::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let seq = d.get("seq").and_then(EtfValue::as_int).unwrap_or(-1);
                        return Ok((token, session_id, seq));
                    }
                    Some(1) => {} // heartbeat
                    other => return Err(format!("ждали op6, пришло {other:?}; терм: {value:?}")),
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    Err("клиент закрылся, не прислав op6".to_string())
}

// ---------------------------------------------------------------------------
// Стаб-кейсы: реальные сокеты 127.0.0.1
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ws_resume_sent_once_after_hello() {
    // Кейс 8: HELLO → ровно один op6 (не op2) с полями; стаб отвечает RESUMED s=95 (как `.8269`).
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let observed = expect_resume(&mut ws).await?;
        send_binary(&mut ws, stream.write_frame(&resumed_bytes(95))).await?;
        ws.close(None).await.map_err(io_err)?;
        Ok(observed)
    })
    .await;

    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(
        CASE_TIMEOUT,
        connect_and_run_resume(
            &url,
            &mut pipe,
            RunOptions::none(),
            Some(test_resume_params()),
        ),
    )
    .await
    .expect("цикл транспорта не уложился в 5 с");
    let observed = stub_result(stub).await;

    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "стаб закрыл соединение без кода"
    );
    assert_eq!(
        observed,
        ("TEST_TOKEN".to_string(), LAB_SESSION_ID.to_string(), 94),
        "поля op6"
    );
    assert_eq!(
        pipe.session().last_seq(),
        Some(95),
        "RESUMED продолжил серию: 94 → 95"
    );
}

#[tokio::test]
async fn ws_resume_suppresses_identify() {
    // Кейс 9: при resume опция identify игнорируется — op2 НЕ отправляется.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let observed = expect_resume(&mut ws).await?;
        send_binary(&mut ws, stream.write_frame(&resumed_bytes(95))).await?;
        // Ограниченное окно наблюдения после RESUMED: op2 быть не должно, удары — ACKаем,
        // иначе клиент станет зомби и оборвёт соединение раньше проверки.
        let mut identifies = 0u64;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match timeout(remaining, ws.next()).await {
                Ok(Some(Ok(Message::Binary(data)))) => {
                    match decode_client_frame(&data)?
                        .get("op")
                        .and_then(EtfValue::as_int)
                    {
                        Some(2) => identifies += 1,
                        Some(1) => send_binary(&mut ws, stream.write_frame(&ack_bytes())).await?,
                        _ => {}
                    }
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(error))) => return Err(format!("сокет наблюдения: {error}")),
                Ok(None) => break,
                Err(_) => break,
            }
        }
        ws.close(None).await.map_err(io_err)?;
        Ok((observed, identifies))
    })
    .await;

    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(
        CASE_TIMEOUT,
        connect_and_run_resume(
            &url,
            &mut pipe,
            RunOptions {
                identify: Some(test_identify_params()),
                on_dispatch: None,
            },
            Some(test_resume_params()),
        ),
    )
    .await
    .expect("цикл транспорта не уложился в 5 с");
    let (observed, identifies) = stub_result(stub).await;

    assert_eq!(outcome, Ok(RunOutcome::Closed { code: None }));
    assert_eq!(
        observed,
        ("TEST_TOKEN".to_string(), LAB_SESSION_ID.to_string(), 94),
        "op6 ушёл с полями resume"
    );
    assert_eq!(identifies, 0, "op2 при resume не отправляется");
}

#[tokio::test]
async fn ws_op7_then_resume_keeps_seq() {
    // Кейс 10 (T-GW-06): op7 → новый сокет → RESUME(seq=2) → RESUMED s=3 → s=4.
    // События не теряются: серия s непрерывна через оба соединения.
    let (url, stub) = spawn_stub2(
        |mut ws| async move {
            let mut stream = StubStream::new();
            send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
            // Свежее соединение: ждём op2.
            let mut identifies = 0u64;
            while let Some(message) = ws.next().await {
                match message.map_err(io_err)? {
                    Message::Binary(data) => {
                        let value = decode_client_frame(&data)?;
                        if value.get("op").and_then(EtfValue::as_int) == Some(2) {
                            identifies += 1;
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            if identifies != 1 {
                return Err(format!("op2 на первом сокете: {identifies}"));
            }
            send_binary(&mut ws, stream.write_frame(&ready_bytes(LAB_SESSION_ID))).await?;
            send_binary(
                &mut ws,
                stream.write_frame(&dispatch_bytes("MESSAGE_CREATE", 2)),
            )
            .await?;
            send_binary(&mut ws, stream.write_frame(&op7_bytes())).await?;
            // Ждём, пока клиент обработает op7 и закроет соединение сам (иначе reset
            // обгонит доставку кадров и клиент вернёт ошибку WS вместо Reconnect).
            let drained = timeout(CASE_TIMEOUT, async {
                while let Some(Ok(_)) = ws.next().await {}
            })
            .await;
            if drained.is_err() {
                return Err("клиент не закрыл соединение после op7 за 5 с".to_string());
            }
            Ok(identifies)
        },
        |mut ws| async move {
            let mut stream = StubStream::new();
            send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
            let observed = expect_resume(&mut ws).await?;
            send_binary(&mut ws, stream.write_frame(&resumed_bytes(3))).await?;
            send_binary(
                &mut ws,
                stream.write_frame(&dispatch_bytes("MESSAGE_UPDATE", 4)),
            )
            .await?;
            ws.close(None).await.map_err(io_err)?;
            // Не роняем сокет сразу: даём клиенту вычитать close-кадр (иначе reset).
            let _ = timeout(CASE_TIMEOUT, async {
                while let Some(Ok(_)) = ws.next().await {}
            })
            .await;
            Ok(observed)
        },
    )
    .await;

    let mut pipe1 = GatewayPipe::new(Session::new(0));
    let outcome1 = timeout(
        CASE_TIMEOUT,
        connect_and_run_with(
            &url,
            &mut pipe1,
            RunOptions {
                identify: Some(test_identify_params()),
                on_dispatch: None,
            },
        ),
    )
    .await
    .expect("первое соединение не уложилось в 5 с");
    assert_eq!(
        outcome1,
        Ok(RunOutcome::Reconnect {
            reason: ReconnectReason::ServerRequest
        }),
        "op7 → Reconnect{{ServerRequest}}"
    );
    assert_eq!(pipe1.session().last_seq(), Some(2), "событие s=2 принято");
    let state = pipe1.resume_state().expect("сессия резюмируема");
    assert_eq!(state.session_id, LAB_SESSION_ID);
    assert_eq!(state.last_seq, 2);

    let mut pipe2 = GatewayPipe::new(Session::new(0));
    let outcome2 = timeout(
        CASE_TIMEOUT,
        connect_and_run_resume(
            &url,
            &mut pipe2,
            RunOptions::none(),
            Some(ResumeParams {
                token: "TEST_TOKEN".to_string(),
                session_id: state.session_id.clone(),
                seq: state.last_seq,
            }),
        ),
    )
    .await
    .expect("второе соединение не уложилось в 5 с");
    let (first_ok, observed) = stub_result(stub).await;

    assert_eq!(first_ok, 1, "первый сокет: один op2");
    assert_eq!(
        observed,
        ("TEST_TOKEN".to_string(), LAB_SESSION_ID.to_string(), 2),
        "RESUME продолжил с seq=2"
    );
    assert_eq!(
        outcome2,
        Ok(RunOutcome::Closed { code: None }),
        "второй сокет закрыт стабом"
    );
    assert_eq!(
        pipe2.session().last_seq(),
        Some(4),
        "серия продолжилась: 2 → 3 (RESUMED) → 4"
    );
}

#[tokio::test]
async fn ws_invalid_session_false() {
    // Кейс 11: op9 d=false → Reconnect{InvalidSession}; флаг запрещает resume — план даёт IDENTIFY.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let observed = expect_resume(&mut ws).await?;
        send_binary(&mut ws, stream.write_frame(&op9_bytes(atom("false")))).await?;
        ws.close(None).await.map_err(io_err)?;
        Ok(observed)
    })
    .await;

    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(
        CASE_TIMEOUT,
        connect_and_run_resume(
            &url,
            &mut pipe,
            RunOptions::none(),
            Some(test_resume_params()),
        ),
    )
    .await
    .expect("цикл транспорта не уложился в 5 с");
    let observed = stub_result(stub).await;

    assert_eq!(
        outcome,
        Ok(RunOutcome::Reconnect {
            reason: ReconnectReason::InvalidSession
        }),
        "op9 → Reconnect{{InvalidSession}}"
    );
    assert_eq!(
        observed,
        ("TEST_TOKEN".to_string(), LAB_SESSION_ID.to_string(), 94),
        "op6 ушёл до op9"
    );
    assert_eq!(
        pipe.last_invalid_session_resumable(),
        Some(false),
        "флаг последнего op9"
    );
    let state = ResumeState {
        session_id: LAB_SESSION_ID.to_string(),
        last_seq: 94,
    };
    assert_eq!(
        plan_reconnect(Some(&state), 3_000, pipe.last_invalid_session_resumable()),
        ReconnectAction::Identify,
        "d=false: следующий коннект — IDENTIFY"
    );
}

#[tokio::test]
async fn ws_resume_not_before_hello() {
    // Кейс 13 (нит R10 мутационного ревью): на resume-соединении до HELLO клиент молчит —
    // op6 уходит только после оп10 (аналог `ws_identify_only_after_hello` для RESUME).
    let (url, stub) = spawn_stub(|mut ws| async move {
        let quiet = timeout(Duration::from_millis(300), ws.next())
            .await
            .is_err();
        if !quiet {
            return Ok((quiet, None));
        }
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let observed = expect_resume(&mut ws).await?;
        send_binary(&mut ws, stream.write_frame(&resumed_bytes(95))).await?;
        ws.close(None).await.map_err(io_err)?;
        Ok((quiet, Some(observed)))
    })
    .await;

    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(
        CASE_TIMEOUT,
        connect_and_run_resume(
            &url,
            &mut pipe,
            RunOptions::none(),
            Some(test_resume_params()),
        ),
    )
    .await
    .expect("цикл транспорта не уложился в 5 с");
    let (quiet, observed) = stub_result(stub).await;

    assert_eq!(outcome, Ok(RunOutcome::Closed { code: None }));
    assert!(quiet, "до HELLO клиент не отправил ни одного кадра");
    assert_eq!(
        observed.expect("op6 после HELLO"),
        ("TEST_TOKEN".to_string(), LAB_SESSION_ID.to_string(), 94),
        "op6 ушёл после HELLO с полями resume"
    );
    assert_eq!(pipe.session().last_seq(), Some(95));
}
