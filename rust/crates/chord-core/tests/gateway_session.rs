//! Сессия шлюза Discord на виртуальных часах (волна RW-03, P1-03 / T-GW-01…03).
//!
//! **Провенанс оракулов** (независимые артефакты, не подгонка под реализацию):
//! - HELLO-терм (138 Б, `op10`, `heartbeat_interval = 41250`) — первые байты расшифровки
//!   `{RE_LAB_ROOT}/gw-proto/zstd-frames-95974_1389.hexl` (снимок `capture-run2.jsonl`, сокет
//!   `95974.1389`); ведущий `0x83` — маркер варианта провода, `etf::decode` его пропускает.
//! - Таймлайн `{RE_LAB_ROOT}/gw-proto/timeline-95974_1389.tsv` (md5 `23f38750227ae79166748323199054d7`):
//!   137 событий; источники — `capture-run2.jsonl` (ts) и `gw-proto/msgs-run2-95974_1389.jsonl`
//!   (op/s/t); генератор `/home/sanya/re-deep/gw-make-timeline.mjs`.
//! - Симуляция координатора той же семантики: для всех 41 250 сидов зомби не срабатывает
//!   ни разу, число ударов 11–12 (сид 1214 → 12).
//!
//! Тесты используют только публичный API (`chord_core::gateway` + ре-экспорт кодека
//! `chord_core::codec`); лабораторные файлы опциональны: нет файла — skip с причиной, а не
//! падение.

use chord_core::HEARTBEAT_INTERVAL_MS;
use chord_core::codec::ZstdStream;
use chord_core::codec::etf::{self, EtfValue};
use chord_core::gateway::{Effect, GatewayError, Op, ReconnectReason, Session};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// HELLO-терм (138 Б) из оракула захвата; ведущий `0x83` — маркер провода.
const HELLO_HEX: &str = "8374000000046400016474000000026400065f74726163656c000000016d000000325b22676174657761792d7072642d61726d2d75732d65617374312d642d30346d35222c7b226d6963726f73223a302e307d5d6a6400126865617274626561745f696e74657276616c620000a1226400026f70610a640001736400036e696c640001746400036e696c";

/// Дедлайн зомби в юнит-тестах: `удар + 41250` (интервал из реального HELLO).
const ZOMBIE_AFTER_BEAT_MS: u64 = 41_250;

/// Строка hex → байты (пробелы игнорируются).
fn unhex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(compact.len() % 2, 0, "hex нечётной длины: {s}");
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Байты → строка hex (для сверки с оракулом).
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Байты HELLO-терма (полная форма провода, 138 Б).
fn hello_term() -> Vec<u8> {
    let term = unhex(HELLO_HEX);
    assert_eq!(term.len(), 138, "HELLO-терм из оракула — 138 Б");
    term
}

/// Ключ-атом map (как в реальном трафике: `op`/`s`/`t` — атомы).
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

/// Минимальное значение оп-кода: `{op: code}`.
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

/// Lab-корень: `RE_LAB_ROOT` или `/home/sanya/re-deep` (как в тестах RW-02).
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

/// Событие таймлайна: `i \t ts \t op \t s \t t` (`nil`/`-` = nil).
#[derive(Debug, Clone)]
struct TimelineEvent {
    /// Момент события, мс (из `capture-run2.jsonl`).
    ts: u64,
    /// Оп-код шлюза.
    op: i64,
    /// `s`-последовательность диспатча (у остальных — `None`).
    s: Option<u64>,
}

/// Разбор tsv-таймлайна: строки `#` — заголовки, поля через табуляцию.
fn parse_timeline(text: &str) -> Vec<TimelineEvent> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields.len(), 5, "строка таймлайна из 5 полей: {line}");
            TimelineEvent {
                ts: fields[1]
                    .parse()
                    .unwrap_or_else(|e| panic!("ts в строке {line}: {e}")),
                op: fields[2]
                    .parse()
                    .unwrap_or_else(|e| panic!("op в строке {line}: {e}")),
                s: parse_seq(fields[3]),
            }
        })
        .collect()
}

/// Поле `s` таймлайна: `nil` (актуальный генератор) или `-` (старый) → `None`, иначе число.
fn parse_seq(field: &str) -> Option<u64> {
    if field == "-" || field == "nil" {
        return None;
    }
    Some(field.parse().unwrap_or_else(|e| panic!("s={field}: {e}")))
}

/// Значение события таймлайна для `Session::on_value` (поля строки; `d` HELLO — из константы).
fn value_for(event: &TimelineEvent) -> EtfValue {
    match event.op {
        10 => map(vec![
            ("op", EtfValue::Int(10)),
            (
                "d",
                map(vec![(
                    "heartbeat_interval",
                    EtfValue::Int(i64::try_from(HEARTBEAT_INTERVAL_MS).expect("интервал в i64")),
                )]),
            ),
        ]),
        0 => map(vec![
            ("op", EtfValue::Int(0)),
            (
                "s",
                EtfValue::Int(
                    i64::try_from(event.s.expect("у op0 обязателен s")).expect("s в i64"),
                ),
            ),
        ]),
        other => op_value(other),
    }
}

#[test]
fn hello_term_parses_interval() {
    // HELLO с реального провода: интервал 41 250 и первый удар внутри интервала (jitter от сида).
    let seed = 777u64;
    let mut session = Session::new(seed);
    let effects = session
        .on_term(&hello_term(), 1000)
        .expect("HELLO-терм из захвата обязан разобраться");
    assert!(effects.is_empty(), "HELLO сам по себе не даёт эффектов");
    let hello = session.hello().expect("после op10 HELLO заполнен");
    assert_eq!(hello.heartbeat_interval_ms, 41_250, "интервал из захвата");
    assert_eq!(
        session.next_deadline_ms(),
        Some(1000 + seed % 41_250),
        "первый удар — now + jitter_seed % interval"
    );
}

#[test]
fn first_beat_delay_is_seed_mod_interval() {
    // Крайние и «захватные» сиды: дедлайн строго внутри интервала.
    for seed in [0u64, 1, 1214, 41_249] {
        let mut session = Session::new(seed);
        let now = 5_000u64;
        session.on_term(&hello_term(), now).expect("HELLO");
        let expected = now + seed % 41_250;
        assert_eq!(session.next_deadline_ms(), Some(expected), "сид {seed}");
        assert!(
            expected < now + 41_250,
            "первый удар — строго внутри интервала (сид {seed})"
        );
    }
}

#[test]
fn zero_interval_hello_is_malformed() {
    // Нулевой интервал недопустим: первый удар — `seed % interval` (деление на ноль),
    // поэтому сессия обязана ответить Malformed — без паники и без расписания.
    let seed = 7u64;
    let mut session = Session::new(seed);
    let zero = hello_value(0);
    match session.on_value(&zero, 1000) {
        Err(GatewayError::Malformed { msg }) => assert!(!msg.is_empty(), "сообщение непусто"),
        other => panic!("интервал 0: ожидался Malformed, получено {other:?}"),
    }
    // Отрицательный интервал в u64 не переводится — тоже Malformed.
    match session.on_value(&hello_value(-1), 1001) {
        Err(GatewayError::Malformed { msg }) => assert!(!msg.is_empty(), "сообщение непусто"),
        other => panic!("интервал -1: ожидался Malformed, получено {other:?}"),
    }
    // Через провод тот же контракт: терм с нулевым интервалом — Malformed, а не паника.
    let term = etf::encode(&zero);
    match session.on_term(&term, 1002) {
        Err(GatewayError::Malformed { msg }) => assert!(!msg.is_empty(), "сообщение непусто"),
        other => panic!("терм с интервалом 0: ожидался Malformed, получено {other:?}"),
    }
    // Некорректный HELLO не принят: ни интервала, ни расписания; сессия жива.
    assert!(
        session.hello().is_none(),
        "некорректный HELLO не запоминается"
    );
    assert_eq!(session.next_deadline_ms(), None, "удары не запланированы");
    assert!(!session.is_dead(), "ошибка структуры не убивает сессию");
    // Корректный HELLO после ошибки работает штатно (ошибка не «отравила» сессию).
    session
        .on_term(&hello_term(), 2000)
        .expect("корректный HELLO после ошибки");
    assert_eq!(
        session.hello().map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "интервал корректного HELLO"
    );
    assert_eq!(
        session.next_deadline_ms(),
        Some(2000 + seed % 41_250),
        "расписание построено от корректного HELLO"
    );
}

#[test]
fn first_beat_has_null_sequence() {
    // До первого диспатча d = null; после удара дедлайн — зомби-таймаут (нет ACK за интервал).
    let mut session = Session::new(10);
    session.on_term(&hello_term(), 1000).expect("HELLO");
    assert_eq!(
        session.on_timer(1010),
        vec![Effect::SendHeartbeat { d: None }],
        "первый удар: s ещё нет → d = null"
    );
    assert_eq!(
        session.next_deadline_ms(),
        Some(1010 + ZOMBIE_AFTER_BEAT_MS),
        "дедлайн зомби = момент удара + интервал"
    );
    assert!(
        session.on_timer(1011).is_empty(),
        "второй удар не раньше ACK"
    );
}

#[test]
fn second_hello_after_first_beat_is_ignored() {
    // До первого удара повторный HELLO пере-планирует расписание (контроль)...
    let seed = 3u64;
    let mut early = Session::new(seed);
    early.on_term(&hello_term(), 1000).expect("HELLO 41 250");
    early
        .on_value(&hello_value(8_000), 1500)
        .expect("повторный HELLO до первого удара");
    assert_eq!(
        early.hello().map(|hello| hello.heartbeat_interval_ms),
        Some(8_000),
        "до первого удара интервал обновляется"
    );
    assert_eq!(
        early.next_deadline_ms(),
        Some(1500 + seed % 8_000),
        "расписание пересчитано от нового HELLO"
    );

    // ...после первого удара — игнорируется целиком: ни интервал, ни расписание не меняются.
    let mut session = Session::new(0);
    session.on_term(&hello_term(), 1000).expect("HELLO 41 250");
    assert_eq!(
        session.on_timer(1000),
        vec![Effect::SendHeartbeat { d: None }],
        "первый удар"
    );
    let deadline_before = session.next_deadline_ms();
    assert_eq!(
        deadline_before,
        Some(1000 + 41_250),
        "дедлайн зомби до подмены"
    );
    assert!(
        session
            .on_value(&hello_value(8_000), 2000)
            .expect("повторный HELLO после удара — не ошибка")
            .is_empty(),
        "повторный HELLO не даёт эффектов"
    );
    assert_eq!(
        session.hello().map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "после первого удара интервал не подменяется"
    );
    assert_eq!(
        session.next_deadline_ms(),
        deadline_before,
        "после первого удара расписание не меняется"
    );
    assert_eq!(session.beats_emitted(), 1, "новых ударов нет");
    // Дальнейший ACK закрывает удар по исходному интервалу 41 250.
    session.on_value(&op_value(11), 1222).expect("ACK");
    assert_eq!(session.rtt_ms(), Some(222), "RTT = 1222 − 1000");
    assert_eq!(
        session.next_deadline_ms(),
        Some(1222 + 41_250),
        "следующий удар — ACK + исходный интервал"
    );
}

#[test]
fn beats_carry_last_sequence() {
    // Удар несёт последний s из диспатчей.
    let mut session = Session::new(5);
    session.on_term(&hello_term(), 1000).expect("HELLO");
    for seq in 1..=3i64 {
        let value = map(vec![
            ("op", EtfValue::Int(0)),
            ("s", EtfValue::Int(seq)),
            ("t", EtfValue::Str("READY".to_string())),
        ]);
        assert!(
            session.on_value(&value, 1001).expect("диспатч").is_empty(),
            "диспатч s={seq} не даёт эффектов"
        );
    }
    assert_eq!(session.last_seq(), Some(3), "последний s");
    assert_eq!(
        session.on_timer(1005),
        vec![Effect::SendHeartbeat { d: Some(3) }],
        "удар несёт s=3"
    );
}

#[test]
fn ack_measures_rtt_and_reschedules() {
    // ACK закрывает удар: RTT = now − удар, следующий удар — через интервал от ACK.
    let mut session = Session::new(0);
    session.on_term(&hello_term(), 1000).expect("HELLO");
    assert_eq!(
        session.on_timer(1000),
        vec![Effect::SendHeartbeat { d: None }],
        "сид 0 → удар в момент HELLO"
    );
    assert!(
        session
            .on_value(&op_value(11), 1222)
            .expect("ACK")
            .is_empty(),
        "ACK не даёт эффектов"
    );
    assert_eq!(session.rtt_ms(), Some(222), "RTT = 1222 − 1000");
    assert_eq!(
        session.next_deadline_ms(),
        Some(1222 + 41_250),
        "после ACK следующий удар — через интервал"
    );
}

#[test]
fn missed_ack_is_zombie() {
    // Нет ACK за интервал → Reconnect{Zombie}, сессия мертва и молчит.
    let mut session = Session::new(0);
    session.on_term(&hello_term(), 1000).expect("HELLO");
    assert_eq!(
        session.on_timer(1000),
        vec![Effect::SendHeartbeat { d: None }],
        "удар отправлен"
    );
    assert_eq!(
        session.on_timer(1000 + ZOMBIE_AFTER_BEAT_MS),
        vec![Effect::Reconnect {
            reason: ReconnectReason::Zombie
        }],
        "на границе интервала без ACK — зомби"
    );
    assert!(session.is_dead(), "сессия мертва");
    assert_eq!(
        session.next_deadline_ms(),
        None,
        "у мёртвой сессии дедлайнов нет"
    );
    assert!(
        session.on_timer(1000 + 2 * ZOMBIE_AFTER_BEAT_MS).is_empty(),
        "мёртвая сессия молчит"
    );
    assert!(
        session
            .on_value(&op_value(11), 1000 + 2 * ZOMBIE_AFTER_BEAT_MS)
            .expect("мёртвая сессия не ошибается")
            .is_empty(),
        "мёртвая сессия не даёт эффектов"
    );
}

#[test]
fn dead_session_is_silent_for_dispatches_and_acks() {
    // Мёртвая сессия (op7) молчит в on_value: `s` не двигается, RTT не меняется,
    // повторный op7 не даёт эффектов, счётчик проигнорированных не растёт.
    let mut session = Session::new(0);
    session.on_term(&hello_term(), 1000).expect("HELLO");
    session
        .on_value(&dispatch_value(7), 1001)
        .expect("диспатч s=7");
    assert_eq!(
        session.on_timer(1005),
        vec![Effect::SendHeartbeat { d: Some(7) }],
        "удар с s=7"
    );
    session.on_value(&op_value(11), 1100).expect("ACK");
    assert_eq!(session.rtt_ms(), Some(95), "RTT = 1100 − 1005");
    assert_eq!(
        session.on_value(&op_value(7), 1200).expect("op7"),
        vec![Effect::Reconnect {
            reason: ReconnectReason::ServerRequest
        }],
        "op7 убивает сессию"
    );
    assert!(session.is_dead(), "сессия мертва");
    let ignored_before = session.ignored_values();
    let beats_before = session.beats_emitted();

    // op0: значение принимается без ошибки, но `last_seq` не обновляется.
    assert!(
        session
            .on_value(&dispatch_value(99), 1300)
            .expect("мёртвая сессия не ошибается")
            .is_empty(),
        "диспатч у мёртвой сессии — без эффектов"
    );
    assert_eq!(session.last_seq(), Some(7), "last_seq замер после смерти");
    // op11: RTT остаётся последним измеренным.
    assert!(
        session
            .on_value(&op_value(11), 1400)
            .expect("мёртвая сессия не ошибается")
            .is_empty(),
        "ACK у мёртвой сессии — без эффектов"
    );
    assert_eq!(session.rtt_ms(), Some(95), "rtt_ms замер после смерти");
    // Повторный op7 — без новых эффектов.
    assert!(
        session
            .on_value(&op_value(7), 1500)
            .expect("повторный op7 не ошибка")
            .is_empty(),
        "повторный op7 молчит"
    );
    // Неизвестный опкод — тоже молчание (и не считается проигнорированным).
    assert!(
        session
            .on_value(&op_value(999), 1600)
            .expect("op999 не ошибка")
            .is_empty(),
        "неизвестный опкод у мёртвой сессии молчит"
    );
    assert_eq!(
        session.ignored_values(),
        ignored_before,
        "мёртвая сессия не считает проигнорированные значения"
    );
    assert_eq!(session.beats_emitted(), beats_before, "удары не эмитятся");
    assert_eq!(session.next_deadline_ms(), None, "дедлайнов нет");
}

#[test]
fn op7_and_op9_reconnect() {
    // op7 — сервер просит реконнект; op9 — сессия невалидна (в захвате волны R7 op9 не
    // встречался — контракт из спеки протокола).
    let mut session = Session::new(0);
    session.on_term(&hello_term(), 100).expect("HELLO");
    assert_eq!(
        session.on_value(&op_value(7), 200).expect("op7"),
        vec![Effect::Reconnect {
            reason: ReconnectReason::ServerRequest
        }],
        "op7 → ServerRequest"
    );
    assert!(session.is_dead(), "после op7 сессия мертва");

    let mut session = Session::new(0);
    session.on_term(&hello_term(), 100).expect("HELLO");
    assert_eq!(
        session.on_value(&op_value(9), 200).expect("op9"),
        vec![Effect::Reconnect {
            reason: ReconnectReason::InvalidSession
        }],
        "op9 → InvalidSession"
    );
    assert!(session.is_dead(), "после op9 сессия мертва");
}

#[test]
fn unknown_ops_and_garbage_do_not_panic() {
    // Таблица оп-кодов из спеки (`docs/re/gateway-protocol.md` §4).
    assert_eq!(Op::from_code(0), Op::Dispatch);
    assert_eq!(Op::from_code(1), Op::Heartbeat);
    assert_eq!(Op::from_code(7), Op::Reconnect);
    assert_eq!(Op::from_code(9), Op::InvalidSession);
    assert_eq!(Op::from_code(10), Op::Hello);
    assert_eq!(Op::from_code(11), Op::HeartbeatAck);
    assert_eq!(Op::from_code(40), Op::Unknown(40));

    let mut session = Session::new(0);
    session.on_term(&hello_term(), 1000).expect("HELLO");
    // Известные, но не обрабатываемые в этой волне опкоды и неизвестные коды — без эффектов.
    let unknown = [1i64, 2, 3, 4, 5, 6, 8, 12, 40, 999, -1, i64::MAX];
    for code in unknown {
        let effects = session
            .on_value(&op_value(code), 1001)
            .unwrap_or_else(|e| panic!("опкод {code}: {e}"));
        assert!(effects.is_empty(), "опкод {code} без эффектов");
    }
    assert_eq!(
        session.ignored_values(),
        unknown.len() as u64,
        "счётчик проигнорированных значений"
    );
    assert!(!session.is_dead(), "мусор не убивает сессию");

    // Обрезанный HELLO: любой строгий префикс 138-байтового терма — Decode, без паник.
    let hello = hello_term();
    for cut in 0..hello.len() {
        match session.on_term(&hello[..cut], 1002) {
            Err(GatewayError::Decode { msg }) => {
                assert!(!msg.is_empty(), "обрезание {cut}: сообщение непусто");
            }
            other => panic!("обрезание {cut}: ожидался Decode, получено {other:?}"),
        }
    }
    // Пустой буфер — тоже Decode.
    match session.on_term(&[], 1003) {
        Err(GatewayError::Decode { msg }) => {
            assert!(!msg.is_empty(), "пустой буфер: сообщение непусто")
        }
        other => panic!("пустой буфер: ожидался Decode, получено {other:?}"),
    }
    // ~100 мусорных термов: недопустимый тег (0x80…0xbf вне таблицы тегов ETF) + случайный хвост.
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for i in 0..100usize {
        let mut buf = vec![0x83u8, 0x80 | (i as u8 & 0x3f)];
        let tail = usize::try_from(next() % 8).expect("хвост < 8");
        for _ in 0..tail {
            buf.push((next() >> 24) as u8);
        }
        match session.on_term(&buf, 1004) {
            Err(GatewayError::Decode { msg }) => {
                assert!(!msg.is_empty(), "мусор {i}: сообщение непусто");
            }
            other => panic!("мусор {i}: ожидался Decode, получено {other:?}"),
        }
    }
    // Структурные нарушения известного опкода и лишние байты после терма — Malformed (не Decode):
    // «один терм на кадр», ошибки не глотаются.
    let no_interval = etf::encode(&op_value(10));
    match session.on_term(&no_interval, 1005) {
        Err(GatewayError::Malformed { msg }) => {
            assert!(
                !msg.is_empty(),
                "op10 без d.heartbeat_interval: сообщение непусто"
            );
        }
        other => panic!("op10 без heartbeat_interval: ожидался Malformed, получено {other:?}"),
    }
    let mut with_tail = hello;
    with_tail.push(0x00);
    match session.on_term(&with_tail, 1006) {
        Err(GatewayError::Malformed { msg }) => {
            assert!(!msg.is_empty(), "лишний байт: сообщение непусто");
        }
        other => panic!("HELLO + лишний байт: ожидался Malformed, получено {other:?}"),
    }
    assert!(!session.is_dead(), "мусор и ошибки не убивают сессию");
}

#[test]
fn heartbeat_term_roundtrips() {
    // Кадр heartbeat (op1) кодируется и разбирается кодеком; длина — компактная.
    let with_seq = map(vec![("op", EtfValue::Int(1)), ("d", EtfValue::Int(42))]);
    let bytes = etf::encode(&with_seq);
    assert!(
        bytes.len() <= 64,
        "кадр heartbeat ≤ 64 Б, получено {}",
        bytes.len()
    );
    let (decoded, consumed) = etf::decode(&bytes).expect("энкод → декод");
    assert_eq!(consumed, bytes.len(), "потреблён весь буфер");
    assert_eq!(decoded.get("op").and_then(EtfValue::as_int), Some(1), "op");
    assert_eq!(decoded.get("d").and_then(EtfValue::as_int), Some(42), "d");

    // d = null (первый удар до диспатча): roundtrip до NIL_EXT.
    let no_seq = map(vec![("op", EtfValue::Int(1)), ("d", EtfValue::Nil)]);
    let bytes = etf::encode(&no_seq);
    assert!(
        bytes.len() <= 64,
        "кадр heartbeat без d ≤ 64 Б, получено {}",
        bytes.len()
    );
    let (decoded, _) = etf::decode(&bytes).expect("энкод → декод (d = null)");
    assert_eq!(decoded.get("d"), Some(&EtfValue::Nil), "d = null");

    // Сессия принимает такой кадр без эффектов и ошибок (op1 в этой волне не обрабатывается).
    let mut session = Session::new(0);
    assert!(
        session
            .on_term(&bytes, 10)
            .expect("op1 не ошибка")
            .is_empty(),
        "op1 без эффектов"
    );
}

#[test]
fn lab_hello_via_zstd_pipeline() {
    // Пайплайн RW-02: первые 3 кадра потока → первые 138 Б расшифровки = HELLO-терм оракула.
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
    assert!(frames.len() >= 3, "в снимке потока ≥ 3 кадров");
    let mut stream = ZstdStream::new();
    let mut out = Vec::new();
    for (i, frame) in frames.iter().take(3).enumerate() {
        out.extend(
            stream
                .push(frame)
                .unwrap_or_else(|e| panic!("кадр {i}: {e}")),
        );
    }
    assert!(
        out.len() >= 138,
        "после трёх кадров выдано {} Б (< 138)",
        out.len()
    );
    assert_eq!(
        hex(&out[..138]),
        HELLO_HEX,
        "первые 138 Б расшифровки — HELLO из захвата"
    );
    // Сессия принимает HELLO прямо из пайплайна (форма провода с ведущим 0x83).
    let mut session = Session::new(1214);
    assert!(
        session
            .on_term(&out[..138], 1792391312488)
            .expect("HELLO из пайплайна")
            .is_empty(),
        "HELLO без эффектов"
    );
    assert_eq!(
        session.hello().map(|hello| hello.heartbeat_interval_ms),
        Some(41_250),
        "интервал из HELLO"
    );
}

#[test]
fn lab_timeline_facts() {
    // Независимые факты таймлайна (оракул), без участия реализации.
    let path = lab_root().join("gw-proto/timeline-95974_1389.tsv");
    let Some(text) = read_opt(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    let events = parse_timeline(&text);
    assert_eq!(events.len(), 137, "событий в таймлайне");
    assert_eq!(events[0].op, 10, "первое событие — HELLO");
    assert_eq!(
        events.iter().filter(|event| event.op == 11).count(),
        23,
        "op11 (ACK) в таймлайне"
    );
    let dispatches: Vec<&TimelineEvent> = events.iter().filter(|event| event.op == 0).collect();
    assert_eq!(dispatches.len(), 113, "op0 (DISPATCH) в таймлайне");
    for (index, event) in dispatches.iter().enumerate() {
        assert_eq!(
            event.s,
            Some(index as u64 + 1),
            "s монотонен 1..113 (позиция {index})"
        );
    }
    let hello_ts = events[0].ts;
    let acks: Vec<u64> = events
        .iter()
        .filter(|event| event.op == 11)
        .map(|event| event.ts)
        .collect();
    assert_eq!(acks[0] - hello_ts, 1_214, "first_ack_delta");
    assert_eq!(acks[acks.len() - 1] - hello_ts, 658_927, "last_ack_delta");
    let deltas: Vec<u64> = acks.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(
        deltas.iter().all(|delta| *delta <= 41_263),
        "максимальная дельта между ACK — 41 263 мс"
    );
    assert!(
        deltas
            .iter()
            .filter(|delta| (41_050..=41_300).contains(*delta))
            .count()
            >= 9,
        "каденция 41 250 мс видна ≥ 9 раз"
    );
    // Стартовое состояние сессии для реплея (тест 12): до HELLO ничего не запланировано.
    let fresh = Session::new(1214);
    assert_eq!(fresh.next_deadline_ms(), None, "до HELLO дедлайнов нет");
    assert_eq!(fresh.beats_emitted(), 0, "до HELLO ударов нет");
}

#[test]
fn lab_replay_no_zombie() {
    // Реплей реального таймлайна через сессию: зомби не срабатывает, ударов 11–12.
    let path = lab_root().join("gw-proto/timeline-95974_1389.tsv");
    let Some(text) = read_opt(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    let events = parse_timeline(&text);
    // Сид 1214 = пауза HELLO → первый ACK в захвате: первый удар попадает на реальную отправку.
    let mut session = Session::new(1214);
    let mut beats_from_effects = 0u64;
    for event in &events {
        for effect in session.on_timer(event.ts) {
            match effect {
                Effect::SendHeartbeat { .. } => beats_from_effects += 1,
                Effect::Reconnect { reason } => {
                    panic!("реплей захвата не должен рекконектиться: {reason:?}");
                }
            }
        }
        let effects = session
            .on_value(&value_for(event), event.ts)
            .unwrap_or_else(|e| panic!("событие op{} (ts {}): {e}", event.op, event.ts));
        assert!(
            effects.is_empty(),
            "событие op{} не даёт эффектов",
            event.op
        );
    }
    assert!(!session.is_dead(), "ACK приходят в пределах интервала");
    assert_eq!(
        beats_from_effects,
        session.beats_emitted(),
        "эффекты SendHeartbeat и счётчик ударов совпадают"
    );
    assert!(
        (11..=12).contains(&session.beats_emitted()),
        "ударов {} — ожидалось 11..=12",
        session.beats_emitted()
    );
    assert_eq!(session.last_seq(), Some(113), "последний диспатч s=113");
    assert!(session.rtt_ms().is_some(), "хотя бы один ACK измерен");
}
