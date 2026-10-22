# BATCH RW-30 — P7-01d DurationFormat + P6-02 D-Bus-уведомления (headless)

Дата: 2026-10-22. Team Lead: TL-1. Tech Lead: TL-2. Статус: **frozen**.

## 1. Состав

| Карточка | Seat | Файлы (только эти) | Зависит от | Оракул |
|---|---|---|---|---|
| P7-01d `DurationFormat` | CLIENT-1 / W30a | `discord-i18n/src/{duration.rs,duration_data.rs,lib.rs(1 строка)}`, `discord-i18n/tests/duration_contract.rs`, `discord-i18n/tests/data/duration-oracle.json`, `waves/RW-30/gen-duration-data.mjs` | P7-01 `[x]`, P7-01c `[x]` | Node 24 ICU: `Intl.DurationFormat`, `Intl.PluralRules` |
| P6-02 D-Bus-уведомления | PLATFORM-1 / W30b | `discord-platform/src/dbus/{mod.rs,auth.rs,message.rs}`, `discord-platform/src/notifications.rs`, `discord-platform/src/lib.rs` (только `pub mod dbus;` + `pub mod notifications;`), `discord-platform/tests/{dbus_contract.rs,notifications_contract.rs}`, `waves/RW-30/dbus/{mock_service.py,gen-golden.py}`, `fixtures/dbus/*` | P6-01 `[ ]` (трей — UI; уведомления headless) | libdbus (`python3-dbus`) как независимый сервер/клиент на приватном `dbus-daemon` |

Общие файлы: `src/lib.rs` каждого крейта — по одной строке модулей, без переформатирования.
Запрещено: чужие модули/тесты, `Cargo.toml`/`Cargo.lock`, другие крейты, доки
`STATE/STATUS/ROADMAP/TASKS/map`, golden, сеть, live Discord, `git commit/push`, `unsafe`.

## 2. Контракты

Полные сигнатуры — `TZ-W30a.md`, `TZ-W30b.md`. Доказательная база — `RE-NOTIFICATIONS.md`.

## 3. Кейсы и бюджеты

- W30a: d01–d13, 12–15 тестов, мутации 6–8.
- W30b: n01–n16, 14–18 тестов, мутации 8–10.

## 4. Команды и DoD

Воркеры: свой `CARGO_TARGET_DIR=/tmp/opencode/target-w30<a|b>`, только targeted-тесты своих
файлов. Полный gate — один раз в integration-лейне: `cargo fmt --all -- --check`,
`cargo clippy --offline --workspace --all-targets -- -D warnings`,
`cargo test --offline --workspace`, `npm run gate:worker`.

DoD: red-лог с exit code → green → мутации (restore по md5) → batch gate → независимое ревью
(codex + субагент) → отчёт → accepted в `db.jsonl`, `STATE.md`, доки один раз.

## 5. Параллелизм

Не более двух фоновых воркеров; крейты разные (`i18n` vs `platform`), файлы не пересекаются.
W30b не трогает `discord-core`/`voice`; D-Bus — только std (`UnixStream`) и системный
`dbus-daemon` в тестах (skip-aware, если его нет).
