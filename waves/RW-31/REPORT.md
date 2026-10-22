# REPORT RW-31 — B12 headless: P6-06 power/idle + P2-03 SQLite-store

Волна: RW-31. Дата: 2026-10-22. Статус: принято после gate и ревью.
Контракты: `BATCH-TZ.md`, `TZ-W31a.md`, `TZ-W31b.md`; решения/RE: `RE-POWER.md`,
`SQLITE-DECISION.md`.

## Состав и результаты

| Карточка | Воркер | Тесты | Мутации | Отчёт |
|---|---|---:|---:|---|
| P6-06 Питание/idle | PLATFORM-1 / W31a | 12 (p11 live-skip) | 7/7 | `REPORT-W31a.md` |
| P2-03 SQLite-store | STORE-1 / W31b | 14 | 7/7 | `REPORT-W31b.md` |

## Что сделано

- **P6-06**: `ScreenSaver::{inhibit,uninhibit,is_active,idle_seconds}` (сессионная шина,
  `UnInhibit` с fallback, `UnknownMethod` → `Ok(None)`), `PowerMonitor` (`PrepareForSleep(b)`,
  общий дедлайн ожидания), `IdlePolicy` (пороги — конфигурация, не RE-факт).
- **P2-03**: SQLite через FFI к системному `libsqlite3` 3.54 (без `rusqlite` и новых крейтов):
  `Store` (channels/messages/kv, upsert, keyset-пагинация, soft delete, миграции
  `user_version=1`), `unsafe` изолирован в `ffi.rs` (26 `SAFETY`-блоков), `unsafe_code = "deny"`.

## Оракулы

- D-Bus: приватный `dbus-daemon` + `mock_power.py` на libdbus; мок журналирует вызовы,
  сигналы `PrepareForSleep`; live-проверка только читает (`DISCORD_LAB_LIVE_POWER=1`).
- SQLite: Python 3 `sqlite3` независимо читает созданный файл (`user_version`, схема, индексы,
  `integrity_check`, FK, порядок/keyset, отсутствие дублей).

## Ревью и закрытие находок

- W31a (независимый агент): «доработка» — P1 якорь мутации после fmt + md5, P2 лог/дедлайн/red.
  Закрыто: якорь устойчив к форматированию, **7/7 SKIP=0**, md5 пересчитаны, red/green на
  финальных байтах, `next_event` получил общий дедлайн, harness не течёт на skip-пути.
- W31b (codex): «доработка» — P2 якорь мутации 05 и md5. Закрыто: стабильный SQL-якорь,
  **7/7 SKIP=0**, md5 пересчитаны, red/green перегенерены.
- Общий фикс процесса: мутационные раннеры запускают тесты с `-- --nocapture`, иначе `SKIP:`
  не виден и «KILLED» мог быть ложным; логи пересняты без перенаправления stdout в тот же файл.

## Гейт

`waves/RW-31/gate-cargo.log`: fmt 0, clippy 0, workspace **1010 passed / 0 failed**;
`waves/RW-31/gate-worker.log` — passed.

## Ограничения

- fd-ингибитор сна (`logind.Inhibit`, SCM_RIGHTS), Wayland/X11 idle, пороги idle из RE — вне
  карточки/в бэклоге.
- SQLite: однопоточный `Store`, WAL только для файла, схема Discord wire-совместимость не
  заявляется; для сборки нужен системный `libsqlite3`.
