# REPORT — task-288 (`G5 Platform L1 / P0`, `PLAT-FIX-01`)

- **Карточка:** `task-288` — вынос `k13..k26` из `rust/crates/discord-platform/tests/keyring_contract.rs`
  в новый контракт `rust/crates/discord-platform/tests/safe_storage_contract.rs`.
- **Ветка / worktree:** `bugfix/task-288-keyring-live-meta-fix`,
  `/home/sanya/orca/workspaces/project/bugfix-task-288-keyring-live-meta-fix`
- **База:** `06d89e4` (Merge PR-167, task-287), HEAD не менялся — коммитов нет.
- **Исполнитель:** G5 Voice / Platform, `deepseek/deepseek-v4.1-flash`.

## 1. Причина и объём

`tests/meta/keyring-live.test.mjs` требует, чтобы `keyring_contract.rs` содержал ровно 12
`#[test]`-функций `k01…k12`, у каждой `#[ignore = "live-keyring: …"]`, и чтобы cargo печатал
`0 passed; 0 failed; 12 ignored`. После дописывания `k13..k26` в тот же файл (`k25` — тоже
`#[ignore]`, `k13..k24` и `k26` — обычные тесты) мета-тест краснел на 5 проверках, а
`npm run test:meta` падал, потому что `tests/meta/**` входит в набор.

Правка: safeStorage-кейсы (`k13..k26`) и их live-обвязка перенесены в отдельный контрактный
бинарник. Ни один тест не удалён, не переименован и не ослаблен: тела всех 26 кейсов побайтно
прежние, атрибуты `#[ignore]` сохранены, пороги `RSS_LIMIT_KILOBYTES`/`LEAK_ITERATIONS`/
`LEAK_SECRET_BYTES` в `k02` не тронуты.

## 2. Red-прогон (`red.log`)

```
node --test tests/meta/keyring-live.test.mjs   → EXIT=1, 5 падений
  ✖ ровно 12 #[test] и 12 #[ignore] с причиной live-keyring
  ✖ у каждого #[test] сразу следует #[ignore = "live-keyring: …"]
  ✖ имена и число кейсов не изменились
  ✖ в дефолтном прогоне 0 passed и 12 ignored
  ✖ --list --ignored перечисляет ровно 12 кейсов k01…k12
```

Фактический вывод cargo до правки: `test result: ok. 13 passed; 0 failed; 13 ignored`,
`--list --ignored` отдавал 13 имён (лишний `k25_migrator_with_live_keyring`).
Полный лог: `/home/sanya/orca/project/waves/RW-40/G5/task-288-keyring-live-meta-fix/red.log`.

## 3. Green-прогон (`green.log`)

| # | Команда | Результат |
|---|---------|-----------|
| 1 | `cargo test --offline -p discord-platform --test keyring_contract` | `ok. 0 passed; 0 failed; 12 ignored` , EXIT=0 |
| 1 | `cargo test --offline -p discord-platform --test safe_storage_contract` | `ok. 13 passed; 0 failed; 1 ignored` (`k25` live), EXIT=0 |
| 2 | `node --test tests/meta/keyring-live.test.mjs` | `tests 13 / pass 13 / fail 0 / skipped 0`, EXIT=0 |
| 3 | `npm run test:meta` | `tests 102 / pass 102 / fail 0 / skipped 0`, EXIT=0 |

Все три запуска с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
Дополнительно: `cargo clippy -p discord-platform --tests` — 0 warning/0 error;
`cargo fmt --check -p discord-platform` — чисто по обоим файлам задачи
(`notifications_contract.rs` в diff остаётся, это предсуществующее состояние базы, файл не трогался).
Полные логи: `green.log`.

## 4. Изменённые файлы и md5

| Файл | Строк | md5 |
|------|-------|-----|
| `rust/crates/discord-platform/tests/keyring_contract.rs` | 376 (было 731) | `4e08fb1900a103f974ea97863c60bb7d` |
| `rust/crates/discord-platform/tests/safe_storage_contract.rs` (новый) | 434 | `1a761398a6868906e5a077a46f4bdd1e` |

`tests/meta/keyring-live.test.mjs` **не изменялся** (требование ТЗ).
`git status` в worktree: `M keyring_contract.rs`, `?? safe_storage_contract.rs`; строки
`D node_modules` и `M package-lock.json` были в worktree до начала работы и к задаче не относятся.

### Что осталось в `keyring_contract.rs`

- Импорт сузился до `use discord_platform::keyring::{Keyring, KeyringError};`.
- Оставлены `LIVE_ENV`, `SCHEMA_NAME`, пороги `k02`, `SEQ`, `LIVE_LOCK`, `live_enabled`,
  `unique_key`, `live_lock`, `live_guard`, `live_keyring`, `Entry`, `has_secret_tool`,
  `secret_tool_lookup`, `secret_tool_store`, `secret_tool_clear`, `rss_kilobytes` и ровно `k01..k12`.
- Добавлен абзац в шапку файла со ссылкой на новый контракт — иначе следующий автор снова
  допишет safeStorage-кейсы сюда.

### Что уехало в `safe_storage_contract.rs`

- `k13_safe_storage_prefix_and_detection` … `k26_migration_failure_paths_and_state_do_not_leak_secrets`
  (14 функций, тела без изменений).
- Импорты `SafeStorageMigrator`, `SafeStorageMigrationError`, `ENCRYPTED_TOKEN_PREFIX`,
  `extract_safe_storage_ciphertext`, `is_safe_storage_token`, `KeyringError`, `Keyring`.
- Live-обвязка для `k25`: `LIVE_ENV`, `SEQ`, `LIVE_LOCK`, `live_enabled`, `unique_key`, `live_lock`,
  `live_guard`, `has_secret_tool`, `live_keyring` — те же паники с префиксом `KEYRING-LIVE:`.

## 5. Риски

- `k25` остаётся `#[ignore]`: в дефолтном прогоне `safe_storage_contract` печатает
  `13 passed; 1 ignored` — это ожидаемо, live-исполнение по-прежнему только через
  `cargo test … -- --ignored` в фазе `keyring-live` гейта (там `--test keyring_contract`).
  Если захотят гонять live-миграцию, фазу `keyring-live` в `scripts/gate.mjs` придётся расширить
  на второй `--test safe_storage_contract` — это правка гейта, вне разрешённых файлов, не делал.
- `scripts/test-impact.mjs` может не знать про новый тестовый файл: маппинг не менял.

## 6. Статус

Готово к приёмке PM + Tech Review. `git commit/push` не выполнялся (запрещено воркеру).
