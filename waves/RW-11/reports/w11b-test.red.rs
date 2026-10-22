//! Запрет телеметрии `science`/`beaker`/`track` (T-SEC-04, волна RW-11 / P1-11).
//!
//! **Провенанс оракула** (независимый источник; тест не подгоняется под реализацию):
//! - `docs/re/rest-d02.md` §3/§7: `POST /api/v9/science` — 88 запросов (все XHR с
//!   `Authorization`), `/api/v9/beaker` — 1 запрос той же схемы, `/api/v9/track` в захвате
//!   **не наблюдался**; в API-таблицу они осознанно не включены — политика клиента:
//!   телеметрия по умолчанию не отправляется;
//! - контракт §2 ТЗ `waves/RW-11/TZ.md`: блокируется ровно **первый сегмент** пути
//!   (`science`/`beaker`/`track`); query (`?`) и fragment (`#`) отбрасываются; сравнение
//!   точное и регистрозависимое (пути Discord — lowercase); `guard_target` возвращает
//!   исходный срез без копирования/нормализации.

use discord_core::rest::telemetry::{
    TELEMETRY_SEGMENTS, TelemetryError, guard_target, is_telemetry_path,
};

/// Кейс 1: блокируются ровно первые сегменты `science`/`beaker`/`track`.
///
/// Список — из ТЗ §2; производные кейсы (`#fragment`, хвост сегментов) проверяют тот же
/// контракт «первый сегмент между `/` и `/`».
#[test]
fn blocks_exact_first_segments() {
    // Константа заморожена оракулом: ровно три наблюдаемых/политикой закрытых сегмента.
    assert_eq!(TELEMETRY_SEGMENTS, ["science", "beaker", "track"]);

    for target in [
        "/science",
        "/science?x=1",
        "/science/events",
        "/beaker",
        "/track",
        "/beaker?x=1",
        // Производные того же контракта: fragment, хвост и завершающий слэш.
        "/track/events/1",
        "/science#frag",
        "/beaker/",
    ] {
        assert!(
            is_telemetry_path(target),
            "{target:?}: первый сегмент телеметрии обязан блокироваться"
        );
    }
}

/// Кейс 2: похожие пути и чужие сегменты не блокируются.
#[test]
fn allows_lookalikes_and_other_paths() {
    for target in [
        "/scientific",
        "/users/@me/science",
        "/tracks",
        "/",
        "science",
        "/science%2Ffoo",
        // Производные: query/fragment не влияют на разбор первого сегмента.
        "/users/@me/science?x=/science",
        "/scientific?x=1",
        "/tracks#science",
        // Регистрозависимость: Discord-пути lowercase, `/SCIENCE` — не тот же сегмент.
        "/SCIENCE",
    ] {
        assert!(
            !is_telemetry_path(target),
            "{target:?}: не телеметрия — блокировать нельзя"
        );
    }
}

/// Кейс 3: `guard_target` отказывает телеметрии и пропускает остальное исходным срезом.
#[test]
fn guard_target_blocks_and_passes_through() {
    for target in ["/science", "/science/events", "/beaker?x=1", "/track#frag"] {
        assert_eq!(
            guard_target(target),
            Err(TelemetryError::Blocked),
            "{target:?}: guard_target обязан вернуть Blocked"
        );
    }

    for target in ["/users/@me", "/scientific", "/", "science"] {
        let guarded = guard_target(target)
            .unwrap_or_else(|error| panic!("{target:?}: пропуск ожидался, получено {error:?}"));
        assert_eq!(guarded, target, "{target:?}: содержимое среза не меняется");
        assert_eq!(
            guarded.as_ptr(),
            target.as_ptr(),
            "{target:?}: возвращается указатель на исходный срез, а не копия"
        );
    }

    // Явная проверка на владеемой строке: срез живёт в исходном буфере.
    let owned = String::from("/users/@me/science");
    let guarded = guard_target(&owned).expect("/users/@me/science: пропуск ожидался");
    assert_eq!(guarded, owned.as_str());
    assert_eq!(guarded.as_ptr(), owned.as_ptr(), "исходный буфер без копии");
}

/// Кейс 4: пустой target — не телеметрия, `guard_target` возвращает `Ok("")`.
#[test]
fn empty_target_is_allowed() {
    assert!(!is_telemetry_path(""), "пустой target — не телеметрия");
    assert_eq!(guard_target(""), Ok(""), "guard_target(\"\") → Ok(\"\")");
}
