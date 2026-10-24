# BRIEF W39e — backend BFF на `fixtures/` (Phase 3)

Владелец: W39e (Backend). PM: DeepSeek. Запуск: после приёмки E3 (верстка+фронтенд).
Вход: `fixtures/` (санитизировано), RE-контракты (README в `fixtures/`, `docs/re/frontend-d03.md`),
`rust/gui-spike/src/*.rs` (текущие команды), `TZ.md` §RUNBOOK «Backend».

## Файлы (только эти)

- `rust/gui-spike/src/data.rs` (новый), `src/lib.rs`, `src/main.rs`
- `rust/gui-spike/tests/data_contract.rs` (новый)
- `rust/gui-spike/Cargo.toml` — только если нужна зависимость; **сначала спросить PM** (нет сети)
- логи: `waves/RW-39/PM/w39e/`

`ui/**` — read-only (W39c/W39d).

## Требования

1. Tauri-команды: `ready`, `messages_page`, `members_page`, `settings`. Источник — `fixtures/`,
   **без сети**; путь к fixtures конфигурируем (env/аргумент), не хардкод домашней директории.
2. Типизированные ошибки (`thiserror` без новых крейтов, если возможно): `Io`, `Parse`,
   `NotFound`, `InvalidArg`; никаких `unwrap()` на входе данных.
3. Пагинация: стабильный курсор, лимиты (`page ≤ 100`, `≤ 500` строк в окне UI), пустая страница
   = валидный ответ (не ошибка).
4. Контрактные тесты на реальных файлах `fixtures/` + негативные кейсы (битый JSON, пустой каталог,
   отсутствующий id). Skip-aware: нет fixtures → skip с причиной, но в CI-режиме fail.

## Проверка

```bash
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
```

## Отчёт

`waves/RW-39/PM/w39e/REPORT.md`: команда → вход → выход, покрытие веток, файлы+md5,
что осталось (live Discord не трогаем). Без commit/push, без сети.
