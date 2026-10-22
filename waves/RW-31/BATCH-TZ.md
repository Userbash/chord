# BATCH RW-31 — P6-06 Питание и idle (headless)

Дата: 2026-10-22. Team Lead: TL-1. Tech Lead: TL-2. Статус: **frozen**.

## 1. Состав

| Карточка | Seat | Файлы | Зависит от | Оракул |
|---|---|---|---|---|
| P6-06 Питание/idle | PLATFORM-1 / W31a | `discord-platform/src/power.rs`, `src/lib.rs` (+1 строка), `tests/power_contract.rs`, `waves/RW-31/power/mock_power.py`, `fixtures/dbus/power.json` | P6-02 `[x]` (D-Bus-клиент) | приватный `dbus-daemon` + мок на `python3-dbus`/libdbus |
| P2-03 SQLite-store | STORE-1 / W31b | `discord-store/src/{lib.rs,ffi.rs}`, `discord-store/Cargo.toml` (только lint `unsafe_code`), `discord-store/tests/store_contract.rs`, `waves/RW-31/sqlite/*.py` | решение владельца A `[x]` | Python 3 `sqlite3` (stdlib) + системный `libsqlite3` 3.54 |

Запрещено: чужие модули/тесты, workspace `Cargo.toml`/`Cargo.lock`, другие крейты, доки, golden,
сеть, live Discord, `git commit/push`. `discord-platform/src/lib.rs` — только `pub mod power;`;
`discord-store/Cargo.toml` — только `unsafe_code = "deny"` (исключение согласовано владельцем);
`unsafe` допустим только в `discord-store/src/ffi.rs` с `// SAFETY:` на каждый блок.

## 2. Контракт

`TZ-W31a.md` §1; доказательная база — `RE-POWER.md`.

## 3. Кейсы и бюджет

- W31a: p01–p12, 10–12 тестов, мутации 5–7.
- W31b: s01–s13, 12–15 тестов, мутации 6–8.

## 4. Команды и DoD

Воркер: `CARGO_TARGET_DIR=/tmp/opencode/target-w31a`, только targeted
`cargo test --offline -p discord-platform --test power_contract`. Полный gate — один раз в
integration-лейне (fmt/clippy/workspace/npm). DoD: red → green → мутации → отчёт
`waves/RW-31/REPORT-W31a.md` → ревью → accepted.

## 5. Примечание

P2-03 (SQLite через FFI) включён в волну: владелец подтвердил вариант A
(`waves/RW-31/SQLITE-DECISION.md`). Точные пороги idle→status в RE не подтверждены и вынесены
в `IdlePolicy` как конфигурация.
