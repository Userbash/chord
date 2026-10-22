# REPORT RW-33 — P1-10 keyring через libsecret (headless)

Волна: RW-33. Дата: 2026-10-22. Статус: принято после gate и ревью.
Контракт: `TZ-W33a.md`; `BATCH-TZ.md`.

## Результат

| Карточка | Воркер | Тесты | Мутации |
|---|---|---:|---:|
| P1-10 Keyring | SEC-1 / W33a | 12 (live + skip-режим) | 7/7 |

## Что сделано

- `Keyring::{connect, store, lookup, clear}` поверх FFI к системному `libsecret 0.21`
  (не-variadic `secret_service_*_sync`, `secret_schema_newv`), default collection, схема
  `com.discordlab.Token`, атрибут `key`, флаг `SECRET_SCHEMA_DONT_MATCH_NAME` (иначе
  `secret-tool`-записи не находятся).
- `SecretValue` → `g_strndup` → `String` → `g_free`, `GError` освобождается всегда;
  `Drop` без двойного unref; `!Send + !Sync`; ручной `Debug` без секрета и указателей.
- `unsafe` только в `ffi_secret.rs` с `// SAFETY:`; в продакшн-пути нет `panic/unwrap/expect`.

## Оракулы

- Реальный `org.freedesktop.secrets` (ksecretd) на сессионной шине; независимый CLI-оракул
  `secret-tool` (store/lookup/clear) в обе стороны.
- Live-кейсы по флагу `DISCORD_LAB_KEYRING_LIVE=1`; без флага — skip с причиной, unit-кейсы
  (валидация, Debug-redaction, double clear) выполняются всегда.
- Секреты синтетические, ключи уникальны (`pid`), после тестов коллекция очищается.

## Гейт

`waves/RW-33/gate-cargo.log`: fmt 0, clippy 0, workspace **1043 passed / 0 failed**;
`waves/RW-33/gate-worker.log` — passed.

## Ограничения

- `*_sync` на заблокированной коллекции может ждать prompt — live только по флагу.
- Wire-совместимость с крейтом `keyring`/`secret-service` не заявляется; формат хранения —
  собственный (schema + attribute).
- B07 остаётся заблокированным только по GUI-крейтам; keyring и SQLite закрыты.
