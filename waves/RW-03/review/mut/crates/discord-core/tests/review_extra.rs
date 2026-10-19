//! Ревью RW-03: независимые крайние проверки контракта (файл существует ТОЛЬКО в копии
//! `/tmp/opencode/rw-03/review/mut/`, в репозиторий не попадает). Проверяем заявленные в отчёте
//! решения: нулевой/отрицательный интервал → Malformed; op0 без `s` → Malformed; `op` не-int →
//! молчание; мёртвая сессия молчит на op7, но on_term всё равно декодирует; saturating-время.
use discord_core::codec::etf::{self, EtfValue};
use discord_core::gateway::{GatewayError, Session};

fn atom(s: &str) -> EtfValue {
    EtfValue::Atom(s.to_string())
}

fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(key), value))
            .collect(),
    )
}

fn hello_with_interval(value: EtfValue) -> EtfValue {
    map(vec![
        ("op", EtfValue::Int(10)),
        ("d", map(vec![("heartbeat_interval", value)])),
    ])
}

#[test]
fn zero_and_negative_and_broken_interval_are_malformed() {
    let mut session = Session::new(7);
    let zero = hello_with_interval(EtfValue::Int(0));
    match session.on_value(&zero, 10) {
        Err(GatewayError::Malformed { .. }) => {}
        other => panic!("нулевой интервал: ожидался Malformed, получено {other:?}"),
    }
    assert!(session.hello().is_none());
    assert_eq!(session.next_deadline_ms(), None);

    let negative = hello_with_interval(EtfValue::Int(-1));
    match session.on_value(&negative, 10) {
        Err(GatewayError::Malformed { .. }) => {}
        other => panic!("отрицательный интервал: {other:?}"),
    }
    let no_d = map(vec![("op", EtfValue::Int(10))]);
    match session.on_value(&no_d, 10) {
        Err(GatewayError::Malformed { .. }) => {}
        other => panic!("op10 без d: {other:?}"),
    }
    let str_interval = hello_with_interval(EtfValue::Str("x".to_string()));
    match session.on_value(&str_interval, 10) {
        Err(GatewayError::Malformed { .. }) => {}
        other => panic!("строковый интервал: {other:?}"),
    }
    // Через байты провода — то же самое.
    match session.on_term(&etf::encode(&zero), 10) {
        Err(GatewayError::Malformed { .. }) => {}
        other => panic!("on_term с нулевым интервалом: {other:?}"),
    }
}

#[test]
fn dispatch_without_seq_is_malformed() {
    let mut session = Session::new(0);
    session
        .on_term(&etf::encode(&hello_with_interval(EtfValue::Int(41250))), 100)
        .expect("HELLO");
    let no_s = map(vec![("op", EtfValue::Int(0))]);
    match session.on_value(&no_s, 101) {
        Err(GatewayError::Malformed { .. }) => {}
        other => panic!("op0 без s: {other:?}"),
    }
    assert_eq!(session.last_seq(), None);
}

#[test]
fn op_as_string_is_ignored_not_panic() {
    let mut session = Session::new(0);
    let value = map(vec![("op", EtfValue::Str("10".to_string()))]);
    assert!(
        session.on_value(&value, 5).expect("без ошибки").is_empty(),
        "op-строка не даёт эффектов"
    );
    assert_eq!(session.ignored_values(), 1);
    assert!(session.hello().is_none(), "HELLO не распознан как строка");
}

#[test]
fn dead_session_ignores_second_op7_but_still_decodes_terms() {
    let mut session = Session::new(0);
    session
        .on_term(&etf::encode(&hello_with_interval(EtfValue::Int(41250))), 0)
        .expect("HELLO");
    let effects = session
        .on_value(&map(vec![("op", EtfValue::Int(7))]), 1)
        .expect("op7");
    assert_eq!(effects.len(), 1, "первый op7 — реконнект");
    assert!(session.is_dead());
    assert!(
        session
            .on_value(&map(vec![("op", EtfValue::Int(7))]), 2)
            .expect("повторный op7")
            .is_empty(),
        "мёртвая сессия молчит на повторный op7"
    );
    match session.on_term(&[0x83, 0x80], 3) {
        Err(GatewayError::Decode { .. }) => {}
        other => panic!("мусор у мёртвой сессии обязан быть Decode: {other:?}"),
    }
}

#[test]
fn multi_term_frame_is_malformed() {
    let mut session = Session::new(0);
    let mut buf = etf::encode(&hello_with_interval(EtfValue::Int(41250)));
    buf.extend(etf::encode(&map(vec![("op", EtfValue::Int(11))])));
    match session.on_term(&buf, 10) {
        Err(GatewayError::Malformed { .. }) => {}
        other => panic!("два терма в кадре: {other:?}"),
    }
}

#[test]
fn extreme_timestamps_saturate_without_panic() {
    let term = etf::encode(&hello_with_interval(EtfValue::Int(41250)));
    let mut session = Session::new(u64::MAX);
    session.on_term(&term, u64::MAX - 10).expect("HELLO");
    let deadline = session.next_deadline_ms().expect("дедлайн");
    assert!(deadline <= u64::MAX, "дедлайн переполнился: {deadline}");
    let _ = session.on_timer(u64::MAX);
    let _ = session.on_timer(u64::MAX);
    let _ = session.on_value(&map(vec![("op", EtfValue::Int(11))]), u64::MAX);

    // Время «назад»: RTT не должен паниковать (saturating).
    let mut back = Session::new(0);
    back.on_term(&term, 1000).expect("HELLO2");
    let _ = back.on_timer(1000);
    let _ = back
        .on_value(&map(vec![("op", EtfValue::Int(11))]), 500)
        .expect("ACK назад");
    println!("RTT при времени назад: {:?}", back.rtt_ms());
}
