# REVIEW-196A — Техническое ревью PR-168 (`task-288` / `PLAT-FIX-01`: вынос `k13..k26` в `safe_storage_contract.rs`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-196A
- **Проверяемая ветка:** `bugfix/task-288-keyring-live-meta-fix`, база/HEAD `06d89e4` (Merge PR-167), коммитов в PR нет
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/bugfix-task-288-keyring-live-meta-fix`
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты, исходники после каждой мутации восстановлены побайтно (MD5 совпадают)
- **Дополнение scope:** координатор согласовал (`ask`, ответ `A:fix-gate-in-scope`) правку `scripts/gate.mjs`: без неё PR молча снимал живое исполнение `k25` (см. §7). Правка — один элемент массива `args` в `keyringLivePhase`.

## 1. Объект ревью и целостность файлов

| Файл | MD5 в `REPORT.md` | MD5 до проверки | MD5 после мутаций и правки gate |
| --- | --- | --- | --- |
| `rust/crates/discord-platform/tests/keyring_contract.rs` | `4e08fb1900a103f974ea97863c60bb7d` | совпал ✅ | `4e08fb1900a103f974ea97863c60bb7d` ✅ |
| `rust/crates/discord-platform/tests/safe_storage_contract.rs` (новый) | `1a761398a6868906e5a077a46f4bdd1e` | совпал ✅ | `1a761398a6868906e5a077a46f4bdd1e` ✅ |
| `scripts/gate.mjs` | — (добавлен по TR-196A) | `3343b35` (git blob) | `0a16056b7009abcbced6fb26d184d64f` |
| `tests/meta/keyring-live.test.mjs` | не изменялся | `1f3cec9bf9695a9073164603e8d04cc5` | `1f3cec9bf9695a9073164603e8d04cc5` ✅, `git diff` пуст |
| `rust/crates/discord-platform/src/keyring.rs` (мутировался и восстановлен) | — | `934bfb9f9cefd76fb67640c07909c68c` | `934bfb9f9cefd76fb67640c07909c68c` ✅ |

Харнесс мутаций (`mutations/mutate-196a.sh`) снимает MD5 до цикла, копирует оригиналы в `/tmp/opencode/mut-backup` и после каждого мутанта восстанавливает `cp` из бэкапа, затем сверяет MD5; при расхождении скрипт выходит с кодом `4`. Лог `mutations/run.log` содержит `restore-ok src=934bfb9f… test=4e08fb19…` после каждой из пяти мутаций и одинаковые MD5 в финальной строке.

## 2. Состав изменений (`git status --porcelain`)

```
 D node_modules
 M package-lock.json
 M rust/crates/discord-platform/tests/keyring_contract.rs
 M scripts/gate.mjs
?? rust/crates/discord-platform/tests/safe_storage_contract.rs
```

- Три `M`/`??` — предмет PR-168: минус 360 строк `k13..k26` в `keyring_contract.rs` (было 731 строка и 26 `#[test]`, стало 376 и 12), новый контракт на 434 строки, второй `--test` в live-фазе.
- `D node_modules`, `M package-lock.json` — не часть PR (симлинк `node_modules` развёрнут в рабочем дереве, lock перегенерирован). Координатору при merge брать только три пути, не `git add -A` (см. `P2-2`).
- `Cargo.toml` не менялся: cargo подхватывает `tests/*.rs` автодискавери, отдельная регистрация таргета не нужна.

## 3. `keyring_contract.rs`: k01..k12 не тронуты

Сверка с независимым оракулом — файл из базы (`git show HEAD:rust/crates/discord-platform/tests/keyring_contract.rs`), а не с текущей реализацией. Диф префикса файла до первой `fn k13` содержит ровно три изменения:

1. четыре строки `//!` в шапку со ссылкой на новый контракт;
2. импорт сузился до `use discord_platform::keyring::{Keyring, KeyringError};` — ровно то, что осталось нужным;
3. удалён хвост `k13..k26`.

Тела `k01..k12`, константы `LIVE_ENV`, `SCHEMA_NAME`, пороги `RSS_LIMIT_KILOBYTES = 4 * 1024`, `LEAK_ITERATIONS = 2000`, `LEAK_SECRET_BYTES = 4096`, хелперы `live_guard`/`live_keyring`/`secret_tool_*`/`rss_kilobytes` побайтно те же. Атрибутов `#[ignore = "live-keyring: …"]` в файле ровно 12 — по одному на каждый `#[test]`.

`tests/meta/keyring-live.test.mjs` подтверждает это же независимо: «ровно 12 #[test] и 12 #[ignore]», «у каждого #[test] сразу следует #[ignore]», «имена и число кейсов не изменились», «пороги security-кейсов не ослаблены», «live_guard различает not-enabled, tool-missing и service-unavailable» — все зелёные (`logs/review-196a-meta-keyring-live.log`).

## 4. `safe_storage_contract.rs`: 14 кейсов перенесены без изменений

Независимая проверка переноса: из базы взят хвост файла от `fn k13_safe_storage_prefix_and_detection` до EOF (354 строки) и сравнён с `safe_storage_contract.rs` от той же сигнатуры до EOF.

```
IDENTICAL: k13..k26 (354 строки) побайтно совпадают с базой
```

Лог: `logs/review-196a-k13-k26-identity.log`. То есть ни один assert, ни одно ожидаемое значение base64, ни одна паника `KEYRING-LIVE:` не переписаны под текущую реализацию.

Что добавлено сверх переноса — только шапка модуля и собственные импорты (`ENCRYPTED_TOKEN_PREFIX`, `Keyring`, `KeyringError`, `SafeStorageMigrationError`, `SafeStorageMigrator`, `extract_safe_storage_ciphertext`, `is_safe_storage_token`). Live-обвязка (`LIVE_ENV`, `SEQ`, `LIVE_LOCK`, `live_enabled`, `unique_key`, `live_lock`, `live_guard`, `has_secret_tool`, `live_keyring`) продублирована из `keyring_contract.rs` с теми же текстами паник; это плата за отдельный тестовый бинарник, см. `P2-3`.

Раскладка кейсов: `k13..k24`, `k26` — офлайн, `k25_migrator_with_live_keyring` — `#[ignore = "live-keyring: …"]`. Дефолтный прогон даёт `13 passed; 0 failed; 1 ignored`, live-набор — `k25` (см. §7).

## 5. Тесты

Все прогоны с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.

| Проверка | Результат | Лог |
| --- | --- | --- |
| `cargo test --offline -p discord-platform --test keyring_contract --test safe_storage_contract` | exit `0`: `0 passed; 0 failed; 12 ignored` + `13 passed; 0 failed; 1 ignored` | `logs/review-196a-cargo.log` |
| `node --test tests/meta/keyring-live.test.mjs` | exit `0`, `tests 13 / pass 13 / fail 0` | `logs/review-196a-meta-keyring-live.log` |
| `npm run test:meta` | exit `0`, `tests 102 / pass 102 / fail 0 / skipped 0` | `logs/review-196a-meta-full.log` |
| `cargo clippy --offline -p discord-platform --tests -- -D warnings` | exit `0`, 0 warning | `logs/review-196a-clippy.log` |
| `rustfmt --edition 2024 --check` по обоим тестовым файлам | exit `0` | `logs/review-196a-fmt.log` |
| `--list --ignored` для args live-фазы (после правки gate) | 12 кейсов `k01..k12` + `k25` | `logs/review-196a-gate-live-list.log` |

Про `rustfmt`: проверял с `--edition 2024` (edition воркспейса). На `--edition 2021` rustfmt переупорядочивает импорты по старому `style_edition` и даёт ложный diff — к PR это отношения не имеет.

## 6. Мутации

Методика: точная замена одной конструкции через `python3` с проверкой, что шаблон встречается в файле ровно один раз (иначе `ABORT`), прогон целевого теста, восстановление из бэкапа `cp`, сверка MD5. Мутировался `rust/crates/discord-platform/src/keyring.rs` — файла `src/safe_storage.rs`, названного в ТЗ, в репозитории нет: вся safeStorage-логика (`ENCRYPTED_TOKEN_PREFIX`, `is_safe_storage_token`, `extract_safe_storage_ciphertext`, `decode_base64`, `SafeStorageMigrator`) живёт в `keyring.rs` (строки 36–395).

| # | Мутация | Файл | Итог | Убивший тест |
| --- | --- | --- | --- | --- |
| M1 | `ENCRYPTED_TOKEN_PREFIX = "dQw4w9WgXcQ:"` → `"dQw4w9WgXcQ"` (потерян `:`) | `keyring.rs` | убит | `k13_safe_storage_prefix_and_detection` (+9), `3 passed; 10 failed` |
| M2 | `is_safe_storage_token`: `raw.trim().starts_with(…)` → `raw.starts_with(…)` | `keyring.rs` | убит | `k13` — `is_safe_storage_token("  dQw4w9WgXcQ:aGVsbG8=  \n")` |
| M3 | `extract_safe_storage_ciphertext`: ветка «нет префикса» отдаёт `EmptyPayload` вместо `InvalidBase64` | `keyring.rs` | убит | `k16_extract_safe_storage_ciphertext_rejects_invalid_base64` (`left: Err(EmptyPayload)` vs `right: Err(InvalidBase64)`) |
| M4 | `migrate_once`: удалён guard `if self.migrated { return Ok(false) }` | `keyring.rs` | убит | `k19_migrator_idempotent_no_reimport` (`left: Ok(true)` vs `right: Ok(false)`) |
| M5 | `RSS_LIMIT_KILOBYTES: u64 = 4 * 1024` → `64 * 1024` (ослабление порога k02) | `keyring_contract.rs` | убит | мета-тест «пороги security-кейсов не ослаблены» |

**Итого `5/5` (100%).** После цикла: `keyring.rs` = `934bfb9f9cefd76fb67640c07909c68c`, `keyring_contract.rs` = `4e08fb1900a103f974ea97863c60bb7d` — совпадают с исходными. Логи по мутантам: `mutations/M1.log … M5.log`, сводка `mutations/run.log`, харнесс `mutations/mutate-196a.sh`.

M1 показывает, что оракул у `k13` — литерал `"dQw4w9WgXcQ:"` из формата Discord, а не константа из проверяемого модуля: тест падает и на изменении самой константы (иначе `assert_eq!(ENCRYPTED_TOKEN_PREFIX, ENCRYPTED_TOKEN_PREFIX)` был бы тавтологией). M5 закрывает именно то, что мета-тест обязан ловить: ослабление порогов `k02` не проходит как «зелёная» правка контракта.

## 7. Найденный P1 и правка `scripts/gate.mjs` (scope расширен координатором)

До PR единственный live-кейс миграции `k25_migrator_with_live_keyring` лежал в `keyring_contract.rs`, и фаза `keyring-live` (`scripts/gate.mjs:299`) запускала его в release/qualify:

```
args: ["test", "--offline", "-p", "discord-platform", "--test", "keyring_contract", "--", "--ignored"]
```

После переноса в `safe_storage_contract.rs` фаза гоняла только `keyring_contract`, а `k25` не запускался нигде: в `cargo test --workspace` он `ignored`, в live-фазе — вне выбранного таргета. То есть live-покрытие `SafeStorageMigrator::migrate_keyring` против реального `org.freedesktop.secrets` пропадало молча, без единого красного теста — ровно тот класс «отключения проверки», который `TEST-POLICY.md` запрещает.

Проверка факта (лог `logs/review-196a-gate-live-list.log`):

```
### keyringLivePhase args -- --list --ignored (после PR-168 + TR-196A)
12 tests, 0 benchmarks            # keyring_contract: k01..k12
k25_migrator_with_live_keyring: test
1 test, 0 benchmarks              # safe_storage_contract

### было до TR-196A: только --test keyring_contract
12 tests, 0 benchmarks            # k25 отсутствует
```

Правка (одобрена координатором, `A:fix-gate-in-scope`):

```
args: [
  "test", "--offline", "-p", "discord-platform",
  "--test", "keyring_contract",
  "--test", "safe_storage_contract",
  "--", "--ignored",
],
```

Плюс комментарий в блоке фазы, чтобы следующий автор не отрезал второй таргет. `node --check scripts/gate.mjs` — синтаксис чистый; `node --test tests/meta/keyring-live.test.mjs` — `13/13`; `npm run test:meta` — `102/102`; регексы T2 в мета-тесте (`--test",\s*"keyring_contract`, `"--",\s*"--ignored"`, `failOnSkips`, `env`, `timeoutMs`) после правки совпадают, ни одна проверка не ослаблена.

## 8. Замечания

Блокирующих нет. Ниже — `P2`, все на отдельные карточки.

### P2-1. Второй `--test` в live-фазе ничем не запинен
`tests/meta/keyring-live.test.mjs` проверяет `"--test",\s*"keyring_contract"` и не требует `safe_storage_contract`; удаление второго таргета мета-тест переживёт, и `k25` снова выпадет из live-фазы без красного прогона. Файл мета-теста по ТЗ трогать запрещено, поэтому пин не добавлен. **Рекомендация:** в карточке на `tests/meta/keyring-live.test.mjs` добавить в T2 проверку обоих таргетов как точного списка (`assert.deepEqual` по `--test`-парам) — тогда `M5`-подобное удаление ловится тестом, а не ревьюером.

### P2-2. Шум в рабочем дереве, не входящий в PR
`D node_modules`, `M package-lock.json` — расхождение симлинка и lock, к задаче не относятся. **Рекомендация:** merge только по трём путям (`tests/keyring_contract.rs`, `tests/safe_storage_contract.rs`, `scripts/gate.mjs`), lock чинить отдельно.

### P2-3. Live-обвязка продублирована в двух тестовых бинарниках
`LIVE_ENV`, `SEQ`, `LIVE_LOCK`, `live_enabled`, `unique_key`, `live_lock`, `live_guard`, `has_secret_tool`, `live_keyring` существуют теперь в `keyring_contract.rs` и `safe_storage_contract.rs` почти дословно. Правка в одном файле не доедет до второго (мета-тест проверяет только первый). **Рекомендация:** вынести обвязку в `tests/common/keyring_live/mod.rs` и подключать через `mod common;` — отдельная карточка рефакторинга тестов.

### P2-4. `scripts/test-impact.mjs` не знает про контракты `discord-platform`
В селекторе нет ни одного правила с `discord-platform` (`grep -n "discord-platform\|keyring" scripts/test-impact.mjs` → пусто), поэтому новый файл `safe_storage_contract.rs` не попадёт в targeted-набор при изменении `crates/discord-platform/src/keyring.rs`. Для этой карточки не блокер (T1-гейт гоняет workspace), но targeted-экономия на платформе не работает. **Рекомендация:** карточка на правила `discord-platform` в селекторе.

### P2-5. Формулировка ТЗ про `src/safe_storage.rs`
ТЗ TR-196A называет файл `rust/crates/discord-platform/src/safe_storage.rs`, которого в репозитории нет; safeStorage-код — часть `src/keyring.rs`. Мутации выполнены по фактическому расположению (§6). **Рекомендация:** поправить шаблон ТЗ, чтобы следующий ревьюер не искал несуществующий файл.

## 9. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| `keyring_contract.rs` — ровно `k01..k12`, `0 passed; 0 failed; 12 ignored` | ✅ |
| `safe_storage_contract.rs` — все `k13..k26`, `13 passed; 0 failed; 1 ignored` | ✅ |
| Перенос без потери и ослабления тестов | ✅ 354 строки `k13..k26` побайтно совпадают с базой |
| `tests/meta/keyring-live.test.mjs` не изменялся | ✅ MD5 `1f3cec9b…`, `git diff` пуст |
| `node --test tests/meta/keyring-live.test.mjs` | ✅ `13/13` |
| `npm run test:meta` | ✅ `102/102` |
| Мутации | ✅ `5/5`, исходники восстановлены по MD5 |
| `clippy`/`rustfmt` по затронутым файлам | ✅ exit `0` |
| Live-покрытие `k25` не потеряно | ✅ второй `--test` в `keyringLivePhase` (scope расширен координатором) |
| `git commit/push` не делался | ✅ |
| `P0 = 0`, `P1 = 0` | ✅ |

## 10. Артефакты

```
waves/RW-40/G5/task-288-keyring-live-meta-fix/REVIEW-196A.md                        — этот отчёт
waves/RW-40/G5/task-288-keyring-live-meta-fix/logs/review-196a-cargo.log            — два контракта cargo
waves/RW-40/G5/task-288-keyring-live-meta-fix/logs/review-196a-meta-keyring-live.log — 13/13
waves/RW-40/G5/task-288-keyring-live-meta-fix/logs/review-196a-meta-full.log        — 102/102
waves/RW-40/G5/task-288-keyring-live-meta-fix/logs/review-196a-clippy.log           — clippy -D warnings
waves/RW-40/G5/task-288-keyring-live-meta-fix/logs/review-196a-fmt.log              — rustfmt --edition 2024
waves/RW-40/G5/task-288-keyring-live-meta-fix/logs/review-196a-gate-live-list.log   — live-набор до/после правки gate
waves/RW-40/G5/task-288-keyring-live-meta-fix/logs/review-196a-k13-k26-identity.log — побайтовая сверка переноса
waves/RW-40/G5/task-288-keyring-live-meta-fix/mutations/mutate-196a.sh              — харнесс с восстановлением
waves/RW-40/G5/task-288-keyring-live-meta-fix/mutations/run.log                     — сводка 5/5 и MD5
waves/RW-40/G5/task-288-keyring-live-meta-fix/mutations/M1.log … M5.log             — логи мутантов
```

**Вердикт: APPROVED.** `P0 = 0`, `P1 = 0`, мутации `5/5`, `tests/meta/keyring-live.test.mjs` не изменялся. Следующий статус — `Ready for Merge`; merge трёх путей (`keyring_contract.rs`, `safe_storage_contract.rs`, `scripts/gate.mjs`) и запись в `waves/db.jsonl` выполняет координатор.
