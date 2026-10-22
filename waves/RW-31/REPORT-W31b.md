# REPORT W31b — P2-03 SQLite-store через FFI к системному libsqlite3

Батч: RW-31, карточка P2-03, seat STORE-1 / W31b. Дата: 2026-10-22.
Репозиторий: `discord-lab`, крейт `discord-store`. Решение владельца — **вариант A**
(`waves/RW-31/SQLITE-DECISION.md`). Сеть не использовалась, новых крейтов и правок workspace
`Cargo.toml`/`Cargo.lock` нет, `git commit/push` не выполнялись.

## 1. Границы

- Изменены/созданы только файлы ТЗ:
  - `rust/crates/discord-store/src/lib.rs` — безопасный API `Store` (переписан);
  - `rust/crates/discord-store/src/ffi.rs` — новый, единственное место с `unsafe`;
  - `rust/crates/discord-store/Cargo.toml` — одна строка: `unsafe_code = "forbid"` → `"deny"`;
  - `rust/crates/discord-store/tests/store_contract.rs` — новый контрактный тест s01–s14;
  - `waves/RW-31/sqlite/oracle.py` — независимый оракул (Python 3 stdlib `sqlite3`);
  - `waves/RW-31/run-mutations-w31b.py`, `waves/RW-31/mutations-w31b.log`,
    `waves/RW-31/workers/W31b/{red.log,green.log}`, этот отчёт.
- Не трогал: другие крейты, workspace `Cargo.toml`/`Cargo.lock`, чужие тесты, доки, golden,
  `package.json`. `CORE_TABLES`/`SCHEMA_VERSION` и unit-тест `core_tables_are_set` сохранены.
- Пересечений с W31a нет (`discord-platform` не затрагивался).

## 2. Что сделано

- `src/ffi.rs` — тонкая обёртка `extern "C"` (`#[link(name = "sqlite3")]`, система 3.54):
  `sqlite3_open_v2/close/errmsg/extended_errcode/exec/free/busy_timeout/prepare_v2/bind_text/
  bind_int64/bind_null/step/reset/clear_bindings/column_count/column_type/column_text/
  column_bytes/column_int64/finalize/changes`.
  Безопасные типы `Db`/`Stmt<'a>`/`Step`; `Stmt<'a>` держит `&'a Db` (statement не переживает
  соединение), `Drop` закрывает/финализирует ровно один раз. 26 `// SAFETY:`-комментариев;
  `unsafe` — только в этом файле; в `lib.rs` — `#[allow(unsafe_code)] mod ffi;` и
  `#![deny(unsafe_op_in_unsafe_fn)]`. Вне `ffi.rs` нет `unsafe`, `unwrap`, `expect`, `panic!`.
- `src/lib.rs` — замороженный контракт §1 TZ дословно: `MessageRow`, `ChannelRow`, `StoreError`,
  `Store::{open, open_in_memory, migrate, schema_version, put_channel, channel, put_message,
  messages, delete_message, kv_set, kv_get}`. Модель и ошибка — `Debug/Clone/PartialEq/Eq`
  (нужно тестам), `Display + std::error::Error` для ошибки, `Debug` для `Store` (путь, без handle).
  `Store` — `!Send + !Sync` (проверено компилятором): один поток, один писатель.
- `migrate`: `busy_timeout=5000`, `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`
  (до транзакции — внутри `BEGIN` эти PRAGMA не действуют), затем `BEGIN IMMEDIATE` →
  `CREATE TABLE/INDEX IF NOT EXISTS` → `PRAGMA user_version = 1` → `COMMIT`; при ошибке
  `ROLLBACK`. `user_version > 1` → `StoreError::Migration`. Повторный вызов — no-op.
- Схема v1: `channels(id PK, guild_id, name, kind, position)`,
  `messages(id PK, channel_id → channels(id) ON DELETE CASCADE, author_id, content, timestamp,
  edited_timestamp, flags, deleted)`, `kv(key PK, value)`, индексы
  `(channel_id, timestamp DESC)` и `(channel_id, id DESC)`.
- `messages`: `ORDER BY timestamp DESC, id DESC`, `deleted = 0`, `limit==0` → пусто,
  `limit>1000` → 1000, `before` — keyset по `(timestamp, id)` строго меньше курсора
  (неизвестный курсор → пустая страница). Все значения — только `?n` + `sqlite3_bind_*`,
  конкатенации SQL со значениями нет.
- `put_*` — upsert `ON CONFLICT(id) DO UPDATE`; `put_message` к несуществующему каналу →
  `StoreError::Constraint` (FK, extended code 787); `delete_message` — soft delete
  (`UPDATE … SET deleted = 1`), `true` если строка существует, `false` если её нет.

## 3. Red

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w31b \
  cargo test --offline -p discord-store --test store_contract
# EXIT=101
# error[E0432]: unresolved imports `discord_store::ChannelRow`, `discord_store::MessageRow`,
#               `discord_store::Store`, `discord_store::StoreError`
```

Лог: `waves/RW-31/workers/W31b/red.log` (последняя строка `# EXIT=101`). Red снят до
реализации: тесты ссылаются на контракт, которого в крейте ещё нет.

## 4. Green

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w31b \
  cargo test --offline -p discord-store --test store_contract -- --nocapture
# EXIT=0; 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Лог: `waves/RW-31/workers/W31b/green.log` (последняя строка `# EXIT=0`); `--nocapture` добавлен,
чтобы в логе был виден ответ Python-оракула. Предупреждений компилятора нет. SKIP — нет.

## 5. Тесты (`tests/store_contract.rs`, 14 из бюджета 12–15)

| ID | Тест | Что проверяет |
|---|---|---|
| s01 | `migrate_sets_schema_version_and_is_idempotent` | `user_version` 0→1, три `migrate` подряд |
| s02 | `channel_upsert_roundtrip` | upsert канала, `None`-поля, Unicode дословно |
| s03 | `message_upsert_without_duplicates` | upsert по id, обновление content/flags/edited |
| s04 | `messages_order_limit_zero_and_clamp` | DESC, `limit=0`, `limit>1000` → 1000 (1005 строк) |
| s05 | `keyset_pagination_is_strict_and_gapless` | страницы по 3, дубли timestamp, 4 страницы, без пропусков |
| s06 | `message_without_channel_is_constraint` | FK → `Constraint`, store жив после ошибки |
| s07 | `soft_delete_hides_message` | soft delete: true / true (уже удалено) / false (нет строки) |
| s08 | `kv_set_get_overwrite` | `None`, set, overwrite |
| s09 | `reopen_file_keeps_data_and_version` | файл, drop, reopen: данные + версия |
| s10 | `sql_injection_is_stored_literally` | `'; DROP TABLE …` в id/content/канале; keyset-ветка с враждебным id |
| s11 | `missing_directory_is_open_error` | `StoreError::Open`, без паники |
| s12 | `python_oracle_matches_file` | Python-оракул файла (см. §6) |
| s13 | `nul_rejected_and_large_string_kept` | NUL в content/id → `Sqlite`, строка ~0,9 млн символов целиком |
| s14 | `unknown_cursor_yields_empty_page` | неизвестный курсор/чужой канал → пустая страница |

Skip-aware: s12 печатает `SKIP: python3 с модулем sqlite3 недоступен…` и завершается успешно,
если `python3`/`sqlite3` нет (проверено прогоном бинарника с `PATH=/nonexistent`: 1 passed,
SKIP в выводе). Отсутствие `libsqlite3` проявилось бы на этапе линковки (это не runtime-skip);
фактически система даёт 3.54.0, `pkg-config --modversion sqlite3` → 3.54.0.

## 6. Независимый оракул `waves/RW-31/sqlite/oracle.py`

Читает **файл** БД через stdlib `sqlite3` (Python 3.54-libsqlite3) и печатает `ключ=значение`:
`user_version`, `journal_mode`, `integrity_check`, `foreign_key_violations`, `table`, `column`
(из `PRAGMA table_info`), `index`/`index_column` (из `sqlite_master`+`PRAGMA index_info`),
`message` (content в hex), `duplicate_ids`, `channel`, `kv`. Дополнительно оракул **сам**
перепроверяет семантику: `order_matches` — SQL `ORDER BY timestamp DESC, id DESC` против
сортировки кортежей в Python; `keyset_matches`/`keyset_size` — SQL-запрос «строго старше
курсора» против фильтра `(timestamp, id) < cursor` в Python. Тест s12 сверяет: версия 1,
`journal_mode=wal`, `integrity_check=ok`, FK-нарушений 0, дублей 0, состав таблиц, оба индекса
и их колонки, типы `channels.guild_id`/`messages.timestamp`, три сообщения (порядок, NULL
`edited_timestamp`, `flags=64`, `deleted=0`, Unicode-контент), `kv`, имя канала; `order_matches=1`,
`keyset_matches=1`, `keyset_size=2`.

## 7. Мутации (7, `run-mutations-w31b.py` → `mutations-w31b.log`)

Baseline: `exit=0`, `SKIP=0`, `test result: ok`. Каждая мутация — точечная правка `src/lib.rs`,
восстановление по снимку с проверкой md5 (`b1aa67ff5017fca6dbeda02838949aec` до/после).

| # | Мутация | Убита тестами |
|---|---|---|
| 01 | `foreign_keys_off` (`PRAGMA foreign_keys=OFF`) | s06 |
| 02 | `order_by_asc` (DESC → ASC, все вхождения) | s04, s05 |
| 03 | `limit_clamp_removed` (`limit.min(1000)` → `limit`) | s04 |
| 04 | `upsert_to_insert` (убран `ON CONFLICT … DO UPDATE`) | s03 |
| 05 | `soft_delete_flag_cleared` (`deleted = 1` → `0`) | s07 |
| 06 | `bind_to_concatenation` (channel_id/id в `format!` вместо биндов) | s10 |
| 07 | `user_version_two` (`PRAGMA user_version = 2`) | s01, s09, s10, s12 |

Все 7 — `KILLED` (exit 101, `test result: FAILED`, `SKIP=0`), все файлы восстановлены (md5 OK).
Скрипт запускается с `-- --nocapture`, поэтому строка `SKIP:` была бы видна и baseline стал бы
`INCONCLUSIVE`/остановкой.

## 8. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-store/src/lib.rs` | `b9016340be1146f8dca889a22b95c7e7` |
| `rust/crates/discord-store/src/ffi.rs` | `cb95e6833354bf60099dd50a894555dc` |
| `rust/crates/discord-store/Cargo.toml` | `159bae8f32ffb6dc88e4d98367b5663c` |
| `rust/crates/discord-store/tests/store_contract.rs` | `5a5c671a5751b2e71d038673f2eecfe2` |
| `waves/RW-31/sqlite/oracle.py` | `118d35335e65cfe6b48c3766ae9fc2e2` |
| `waves/RW-31/run-mutations-w31b.py` | `838e319fd398b88a78f40c5ad24e24d2` |
| `waves/RW-31/mutations-w31b.log` | `eef6d73312ed4449cbcaa4e8b45b37f7` |
| `waves/RW-31/workers/W31b/red.log` | `94d0fac40201f375704f52058a927454` |
| `waves/RW-31/workers/W31b/green.log` | `62369d736a1539782d27aafa7d6b3fcd` |

`Cargo.toml`: единственное изменение — `unsafe_code = "forbid"` → `"deny"` (согласовано
владельцем, `SQLITE-DECISION.md` §A).

## 9. Ограничения и риски

- **Однопоточный `Store`**: одно соединение, один писатель; `Store` — `!Send + !Sync`
  (компилятор), пула соединений и конкурентного доступа нет. Конкурентная запись из нескольких
  потоков/процессов не заявляется (busy_timeout лишь сглаживает блокировки).
- **WAL не для `:memory:`**: для `:memory:` SQLite возвращает `journal_mode=memory` — это
  допустимый фолбэк (TZ §1), WAL проверен на файле (s12).
- **NUL-строки**: значения с внутренним NUL не сохраняются (C-строка) — `StoreError::Sqlite`
  (код 21, `SQLITE_MISUSE`), без паники и без обрезания. Путь БД с NUL → `StoreError::Open`.
- **`flags: u64`** хранится в `INTEGER` (i64): значения > `i64::MAX` дают `Constraint`
  (не тихая потеря точности). Отрицательный `flags` в БД (чужая схема) → `Sqlite`/`CORRUPT`.
- **Wire-совместимость с внутренней схемой Discord не заявляется**: таблицы `channels/messages/kv`
  — наши, версия схемы 1; RE-оракул использован только для имён/типов (F-DATA-02/F-DATA-06),
  выдуманных полей Discord нет. `guilds/roles/users/emoji` из `CORE_TABLES` остаются каркасом
  (не входят в v1) — это следующий шаг P2/P3.
- **Поведение зависит от версии системного SQLite** (сейчас 3.54): сборка/линковка требует
  `libsqlite3.so`; на машине без неё тест не слинкуется (в CI нужен пакет `libsqlite3-dev`).
- **Решение по семантике, которого нет в TZ**: повторный `delete_message` существующей
  (уже помеченной) строки возвращает `true` — трактовка «`true`, если строка была помечена
  (или уже удалена — `false` при отсутствии)»; неизвестный курсор в `messages` → пустая
  страница (продолжать не с чего). Обе точки закреплены тестами s07/s14 — если TL решит иначе,
  меняются тест + реализация одновременно.
- `put_message` — upsert **полного снимка** `MessageRow`, включая `deleted`: повторная запись
  с `deleted: false` снимает soft-delete (tombstone не «залипает»). Если для кэша истории нужно
  обратное (удаление необратимо), это отдельное решение TL — правка одной строки SQL
  (`deleted = excluded.deleted` в `ON CONFLICT`) + тест.
- `unsafe` в `ffi.rs` требует отдельного ревью (проверка парности open/close, prepare/finalize,
  `SQLITE_TRANSIENT`, `sqlite3_free` для `exec`-сообщения). Всё это — `// SAFETY:`-блоки,
  ревьюеру смотреть `ffi.rs` целиком.
- Полный `gate:worker` (fmt/clippy/workspace/npm) **не запускал** — по ТЗ он один раз в
  integration-лейне. Учитывая `deny(unsafe_code)` на уровне крейта, clippy может потребовать
  `#[allow(unsafe_code)]` дополнительно на конкретные элементы, если линт поднимется из
  workspace-настроек — сейчас предупреждений нет.

## 10. Передача

- next: TL-2 (integration) — полный `gate:worker` (fmt/clippy/`test --workspace`), затем
  независимое ревью `ffi.rs` (безопасность) и контракта.
- batch gate: не запускал — у integration/QA.

## После ревью codex (2026-10-22)

Вердикт: «доработка» (P0/P1 по коду нет): шаблон мутации 05 не совпадал с форматированием
`lib.rs`, md5 в отчёте устарели. Закрыто:

- якорь мутации 05 заменён на стабильную подстроку SQL; прогон переснят: **7/7 KILLED,
  SKIP=0**, restore по md5;
- md5 в отчёте пересчитаны по финальным файлам;
- red/green перегенерены на финальных байтах (`E0599` на отсутствующем `Store::messages`,
  `# EXIT=101` / `# EXIT=0`, 14 passed);
- лог мутаций переснят без перенаправления stdout в тот же файл (ровно один `## Итог`).
