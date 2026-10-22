# BATCH RW-32 — P6-07 spellcheck (hunspell FFI) + P6-02b hints `a{sv}`

Дата: 2026-10-22. Team Lead: TL-1. Техлид: TL-2. Статус: **frozen**.

## 1. Состав

| Карточка | Seat | Файлы | Зависит от | Оракул |
|---|---|---|---|---|
| P6-07 Spellcheck (headless) | PLATFORM-2 / W32a | `discord-platform/src/spellcheck.rs`, `src/ffi_hunspell.rs`, `Cargo.toml` (только lint), `tests/spellcheck_contract.rs`, `fixtures/hunspell/{test.aff,test.dic}`, `waves/RW-32/spellcheck/*` | системный `libhunspell 1.7` | libhunspell + эталонные словари; CLI-оракула нет — проверяются семантические инварианты |
| P6-02b D-Bus hints | PLATFORM-1 / W32b | `discord-platform/src/dbus/message.rs` (аддитивно), `src/notifications.rs` (аддитивно), `tests/dbus_contract.rs`/`notifications_contract.rs` (доп. кейсы), `fixtures/dbus/hints.json` | P6-02 `[x]` | libdbus-мок (`python3-dbus`) читает `a{sv}` |

Общие файлы: `discord-platform/src/lib.rs` — W32a добавляет `pub mod spellcheck;` + `mod ffi_hunspell;`,
W32b не трогает; `Cargo.toml` — только W32a (`unsafe_code = "deny"` + `#[allow(unsafe_code)]`
на `ffi_hunspell`). Запрещено: чужие модули/тесты, workspace `Cargo.toml`/`Cargo.lock`, golden,
сеть, live, commit.

## 2. Контракты

`TZ-W32a.md`, `TZ-W32b.md`.

## 3. Бюджеты

- W32a: h01–h12, 11–13 тестов, мутации 6–8.
- W32b: q01–q08, 7–9 тестов, мутации 4–6.

## 4. Гейт и DoD

Свой `CARGO_TARGET_DIR=/tmp/opencode/target-w32<a|b>`, targeted-тесты; полный gate — один раз
в integration-лейне. DoD: red → green → мутации (SKIP=0, `--nocapture`) → отчёты → ревью →
accepted.
