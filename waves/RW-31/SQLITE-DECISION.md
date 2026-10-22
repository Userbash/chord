# Решение по P2-03 (SQLite-store): вариант без `rusqlite`

Дата: 2026-10-22. Крейт: `discord-store`. Статус: **ждёт решения владельца**.

## Проблема

`P2-03` (персистентность) заблокирован: `rusqlite`/`libsqlite3-sys` нет в офлайн-кэше, а
`Cargo.toml` без разрешения не меняется.

## Проверка возможности (сделана)

Системный SQLite присутствует: `pkg-config sqlite3` → **3.54.0**, `libsqlite3.so` в
`/usr/lib/x86_64-linux-gnu`. Минимальный FFI-прототип (`sqlite3_open`/`sqlite3_exec`/
`sqlite3_libversion`/`sqlite3_close`, линковка `#[link(name = "sqlite3")]`) собран `rustc`
и успешно создал/записал/закрыл БД **без единого нового крейта и без правок `Cargo.toml`**.

## Предлагаемый вариант A — FFI к системному SQLite

- `discord-store/src/ffi.rs` — тонкая обёртка `extern "C"` (`open/close/errmsg/exec/prepare_v2/
  bind_*/step/column_*/finalize`), весь `unsafe` изолирован здесь.
- `discord-store/src/lib.rs` — безопасный API: `Store::open(path)`, `migrate()`, `put_message`,
  `get_messages(channel, before, limit)`, `put_channel`, `kv_get/kv_set`.
- Схема v1 + миграции в транзакции, `PRAGMA journal_mode=WAL`, `foreign_keys=ON`, busy-timeout.
- Тесты: `:memory:` и temp-файл; восстановление после reopen; уникальные индексы; параметры
  биндятся (никакой конкатенации SQL); конкурентный доступ — один писатель.
- Оракул: `sqlite3` CLI (3.54) и/или Python `sqlite3` как независимая проверка схемы/данных.

**Требуется разрешение:**

1. системная библиотека `libsqlite3` (динамическая линковка) — как runtime-зависимость;
2. точечное исключение из `unsafe_code = "forbid"`: в `discord-store/Cargo.toml` —
   `unsafe_code = "deny"` + `#[allow(unsafe_code)]` только на модуль `ffi`, safety-комментарии
   на каждый `unsafe`-блок, `#![deny(unsafe_op_in_unsafe_fn)]`.

**Плюсы:** разблокирует P2-03/P3 (история, кэш) без сети; один системный .so; полный контроль
над SQL. **Минусы:** FFI-код нужно ревьюить отдельно; поведение зависит от версии SQLite;
`unsafe`-исключение — отклонение от текущего правила «везде forbid».

## Вариант B — ждать `rusqlite` (текущий блокер)

Ничего не делаем до появления крейта в офлайн-кэше/разрешения на загрузку. P2-03 остаётся
закрытым, store — каркас.

## Вариант C — файловый store (свой формат)

Без SQLite: append-only лог/JSON с индексами. Разблокирует персистентность, но это не SQLite
из плана и хуже для запросов/миграций. Не рекомендуется, кроме как временный fallback.
