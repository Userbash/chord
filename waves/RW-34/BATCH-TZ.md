# BATCH RW-34 — D-Bus abstract-адреса + расширенные hints (headless)

Дата: 2026-10-23. Team Lead: TL-1. Статус: **frozen**.

## 1. Состав

| Карточка | Seat | Файлы | Оракул |
|---|---|---|---|
| P6-02c abstract-адреса | PLATFORM-1 / W34a | `discord-platform/src/dbus/{mod.rs}`, `tests/dbus_contract.rs` | приватный `dbus-daemon --address=unix:abstract=…` + libdbus-мок |
| P6-02d hints ext | PLATFORM-2 / W34b | `discord-platform/src/notifications.rs`, `tests/notifications_contract.rs`, `waves/RW-34/dbus/mock_hints2.py`, `fixtures/dbus/hints-ext.json` | libgio/libdbus-мок |

Общие/запрещённые: `src/lib.rs`, `Cargo.toml`, `power.rs`, `spellcheck*`, `keyring*`, чужие тесты,
golden, сеть, live, commit. Каждый воркер — свой `CARGO_TARGET_DIR=/tmp/opencode/target-w34<a|b>`.

## 2. Контракты

`TZ-W34a.md`, `TZ-W34b.md`.

## 3. Бюджеты

- W34a: 6–8 тестов, мутации 4–6.
- W34b: 7–9 тестов, мутации 4–6.

## 4. DoD

red → green → мутации (`--nocapture`, SKIP=0) → отчёты → ревью → accepted → gate → push.
