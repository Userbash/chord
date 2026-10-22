//! Контракт политики reconnect/resume (P4-12, RW-29/W29c).
//!
//! Оракул: `docs/re/gateway-protocol.md` §4 — один `op7 RECONNECT` (после `s=331`) и
//! три эпизода `RESUME → RESUMED` с продолжением `s` (95, 664, 332); `op9
//! INVALID_SESSION` в захвате не встречался, поэтому ветка INVALID_SESSION описана
//! протоколом, а не измерением. Точные клиентские задержки Discord в захвате не
//! измерены (jitter не наблюдался): бэкофф детерминированный, значения —
//! `TZ-W29c.md` §1. Таймер применяется вне модуля, здесь проверяются только значения.

use discord_voice::reconnect::{
    BACKOFF_MS, INVALID_SESSION_DELAY_MS, ReconnectAction, ReconnectPlan, ReconnectPolicy,
};

fn plan(action: ReconnectAction, delay_ms: u64, attempt: u32) -> ReconnectPlan {
    ReconnectPlan {
        action,
        delay_ms,
        attempt,
    }
}

fn resume(delay_ms: u64, attempt: u32) -> ReconnectPlan {
    plan(ReconnectAction::Resume, delay_ms, attempt)
}

fn identify(delay_ms: u64, attempt: u32) -> ReconnectPlan {
    plan(ReconnectAction::Identify, delay_ms, attempt)
}

fn give_up(attempt: u32) -> ReconnectPlan {
    plan(ReconnectAction::GiveUp, 0, attempt)
}

#[test]
fn r01_first_resumable_disconnect_resumes_after_one_second() {
    let mut policy = ReconnectPolicy::new(5);
    assert_eq!(policy.attempts(), 0);
    assert_eq!(policy.on_disconnect(true), resume(1_000, 1));
    assert_eq!(policy.attempts(), 1);
}

#[test]
fn r02_six_resumable_disconnects_follow_the_backoff_table() {
    // §4: после op7 клиент закрыл сокет .10029 и открыл .13072 с RESUME → RESUMED
    // s=332 — сессия и `s` продолжаются, поэтому resumable-ветка основная.
    let mut policy = ReconnectPolicy::new(10);
    let mut delays = Vec::new();
    for _ in 0..6 {
        let step = policy.on_disconnect(true);
        assert_eq!(step.action, ReconnectAction::Resume);
        delays.push(step.delay_ms);
    }
    assert_eq!(delays, BACKOFF_MS.to_vec());
    assert_eq!(policy.attempts(), 6);
}

#[test]
fn r03_after_max_attempts_policy_gives_up_without_delay() {
    let mut policy = ReconnectPolicy::new(3);
    for _ in 0..3 {
        assert_eq!(policy.on_disconnect(true).action, ReconnectAction::Resume);
    }
    assert_eq!(policy.attempts(), 3);
    assert_eq!(policy.on_disconnect(true), give_up(3));
    // GiveUp не наращивает счётчик и повторяется идемпотентно.
    assert_eq!(policy.attempts(), 3);
    assert_eq!(policy.on_disconnect(true), give_up(3));
}

#[test]
fn r04_invalid_session_identifies_after_one_second() {
    // op9 INVALID_SESSION: RESUME отклонён — нужен свежий IDENTIFY, задержка фиксированная.
    let mut policy = ReconnectPolicy::new(4);
    assert_eq!(policy.on_disconnect(true), resume(1_000, 1));
    assert_eq!(
        policy.on_invalid_session(),
        identify(INVALID_SESSION_DELAY_MS, 2)
    );
    assert_eq!(policy.attempts(), 2);
    // Счётчик общий с on_disconnect: следующий разрыв берёт бэкофф по attempt=2.
    assert_eq!(policy.on_disconnect(true), resume(4_000, 3));
}

#[test]
fn r05_on_established_resets_attempt_and_backoff() {
    let mut policy = ReconnectPolicy::new(5);
    for _ in 0..3 {
        let _ = policy.on_disconnect(true);
    }
    assert_eq!(policy.attempts(), 3);
    policy.on_established();
    assert_eq!(policy.attempts(), 0);
    assert_eq!(policy.on_disconnect(true), resume(1_000, 1));
}

#[test]
fn r06_non_resumable_disconnect_identifies_with_backoff() {
    let mut policy = ReconnectPolicy::new(4);
    assert_eq!(policy.on_disconnect(false), identify(1_000, 1));
    assert_eq!(policy.on_disconnect(false), identify(2_000, 2));
    assert_eq!(policy.attempts(), 2);
}

#[test]
fn r07_zero_max_attempts_is_one_and_long_series_saturates() {
    let mut single = ReconnectPolicy::new(0);
    assert_eq!(single.on_disconnect(true), resume(1_000, 1));
    assert_eq!(single.on_disconnect(true), give_up(1));
    assert_eq!(single.on_invalid_session(), give_up(1));

    // Индекс бэкоффа насыщается на последнем элементе, счётчик не переполняется.
    let mut long = ReconnectPolicy::new(u32::MAX);
    let mut last = None;
    for _ in 0..1_000 {
        last = Some(long.on_disconnect(true));
    }
    assert_eq!(long.attempts(), 1_000);
    assert_eq!(last, Some(resume(30_000, 1_000)));
}

#[test]
fn r08_policy_is_reusable_after_give_up() {
    let mut policy = ReconnectPolicy::new(2);
    let _ = policy.on_disconnect(true);
    let _ = policy.on_disconnect(true);
    assert_eq!(policy.on_disconnect(true), give_up(2));
    policy.on_established();
    assert_eq!(policy.attempts(), 0);
    assert_eq!(policy.on_disconnect(true), resume(1_000, 1));
}

#[test]
fn r09_invalid_session_respects_the_same_attempt_limit() {
    let mut policy = ReconnectPolicy::new(2);
    assert_eq!(policy.on_invalid_session(), identify(1_000, 1));
    assert_eq!(policy.on_invalid_session(), identify(1_000, 2));
    assert_eq!(policy.on_invalid_session(), give_up(2));
    assert_eq!(policy.attempts(), 2);
}

#[test]
fn r10_backoff_table_is_the_frozen_contract() {
    assert_eq!(
        BACKOFF_MS,
        [1_000, 2_000, 4_000, 8_000, 16_000, 30_000],
        "таблица из TZ-W29c.md §1"
    );
    assert_eq!(INVALID_SESSION_DELAY_MS, 1_000);
    assert_eq!(BACKOFF_MS.len(), 6);
    assert!(
        BACKOFF_MS.windows(2).all(|pair| pair[0] < pair[1]),
        "бэкофф монотонно растёт до потолка"
    );
}
