# M-07-BE2 — READY `read_state` для UI (red-first)

Дата: 2026-10-24. Воркер: **G1 Backend**. PM: DeepSeek.
Статус: **готово к приёмке**. Окно/live/сеть/keyring/commit не использовались; UI не тронут.

## Контракт (источник)

- `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.5 (M-07):
  `READY.read_state.entries[]` = `{id, flags, last_message_id, last_pin_timestamp,
  last_viewed, mention_count}`; `last_viewed` — **счётчик** read-state, не snowflake
  (`046-MESSAGE_ACK.json`: `last_viewed: 4310` при `message_id: 1561348877573947515`).
- Реальный артефакт `fixtures/gateway/by-type/002-READY.json`: **157** записей
  read_state, из них 143 — `last_viewed` присутствует (3 значения `4310`,
  индексы 76/79/104), 12 — без `last_viewed`, 2 — форма `read_state_type`
  (`{badge_count, id, last_acked_id, read_state_type}`, без `flags`). Замечание
  G6 «3 записи в фикстуре» к текущему артефакту не относится (см. «Остаток»).
- Новый REST-endpoint не добавлялся: только READY-проекция.

## API

`rust/gui-spike/src/data.rs`:

- `pub struct ReadStateEntry { channel_id, flags: Option<u64>,
  last_message_id: Option<String>, last_pin_timestamp: Option<String>,
  last_viewed: Option<u64>, mention_count: Option<u64> }`, serde `camelCase`
  → IPC **ровно** `{channelId, flags, lastMessageId, lastPinTimestamp,
  lastViewed, mentionCount}` (никаких `badge_count`/`last_acked_id`/лишних полей).
- `pub fn read_state(root: &Path) -> Result<Vec<ReadStateEntry>, DataError>`:
  читает `gateway/by-type/002-READY.json`; порядок записей — как в READY
  (UI сопоставляет по `channelId`, пересортировки нет); сеть/токен не нужны.
- Отсутствующее/`null`/санитизированное `nil` поле → `null`, **не** выдуманный 0.
  `flags`/`mention_count` у badge-записей честно `null`.
- Ошибки: нет READY → `NotFound` (через `read_json`/`map_io`); битый JSON,
  нет `d`/`read_state`, `entries` не массив, запись без строкового `id`,
  поле не того типа (`last_viewed: "не число"`) → `Parse` с индексом
  `READY.read_state.entries[i]`. Пустой `entries: []` — валидный пустой ответ;
  ошибка **никогда** не маскируется пустым списком.

`rust/gui-spike/src/main.rs`:

- `#[tauri::command] fn read_state() -> Result<Vec<ReadStateEntry>, DataError>`:
  `data::fixtures_root()` + `data::read_state(&root)`; токен не читается,
  keyring/AuthBackend не задействованы, логирования нет.
- Команда добавлена в `invoke_handler!` и в список wiring-теста; `TOKEN_COMMANDS`
  не менялся (read_state — локальная).

`rust/gui-spike/src/lib.rs`: только строка doc-списка BFF-команд (`read_state`).

## RED (до продуктового кода)

`waves/RW-40/PM/m07-be2/red.log`: тесты написаны первыми, API ещё нет —

```
error[E0432]: unresolved import `discord_gui_spike::data::ReadStateEntry`
error[E0425]: cannot find function `read_state` in module `data`
...
error: could not compile `discord-gui-spike` (test "read_state_contract") due to 12 previous errors
EXIT:101
error: could not compile `discord-gui-spike` (test "data_contract") due to 1 previous error
EXIT:101
```

Ни один assert после red не ослаблен.

## GREEN

`waves/RW-40/PM/m07-be2/green-gui.log`, `green-core.log`, `clippy.log`, `fmt.log`.

- `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` —
  **251 passed / 0 failed** (EXIT 0). База 238 → +13:
  `read_state_contract` 12/12 (новый) + `data_contract` 30/30 (+1, было 29).
- `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` —
  **574 passed / 0 failed** (EXIT 0), не менялось.
- `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets
  -- -D warnings` — EXIT 0.
- `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` — EXIT 0.

## Тесты

`rust/gui-spike/tests/read_state_contract.rs` (новый, 12):

1. **3 записи** из синтетического READY: полная с `last_viewed = 4310`
   (+`mention_count = 7`), без `last_viewed`, с `nil` в
   `last_message_id`/`last_pin_timestamp`/`last_viewed`; проверка структуры
   целиком, `flags`/`mention_count` как есть.
2. JSON-контракт: ровно шесть camelCase-ключей на запись, `lastViewed`/`flags`/
   `mentionCount` — JSON-числа (`is_u64`), `nil` → `null`, а не `0`.
3. Пустой `entries: []` → `Ok([])` (ошибки нет).
4–9. Ошибки: нет READY → `NotFound`; битый JSON, READY без `read_state`,
   `entries` не массив, запись без `id` (в тексте индекс `entries[1]`),
   `last_viewed: "не число"` → `Parse`; каждое — `Err`, не пустой список.
10. Badge-запись реальной формы (`badge_count`/`last_acked_id`/
    `read_state_type`) → все пять необязательных полей `null`, лишние поля не
    подмешиваются.
11. Source-wiring: тело `fn read_state` делегирует в `data::read_state(&root)`,
    нет `token_or`/`backend.token()`/`AuthBackend`/`keyring`/`println!`/`log::`,
    имя зарегистрировано в `invoke_handler!`.

`rust/gui-spike/tests/data_contract.rs` (аддитивно, +1):
`read_state_matches_ready_oracle` — оракул сам READY: совпадение количества и
порядка записей, поле-в-поле JSON (включая `null` у badge-записей и записей без
`last_viewed`), наличие `last_viewed = 4310` и хотя бы одного `null`.

## Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/data.rs` | `6de4f5726fbf2589c2cb7218f8097208` |
| `rust/gui-spike/src/main.rs` | `6e5b0adb6113383b2b010d1be823ff82` |
| `rust/gui-spike/src/lib.rs` | `44a22d107369236face59fe6ffb60e83` |
| `rust/gui-spike/tests/read_state_contract.rs` (new) | `8718a639750412301d4eaff170fec077` |
| `rust/gui-spike/tests/data_contract.rs` | `a2694e42ebfde099265b70ebad469cee` |

Записи в `fixtures/`, `logs/`, `evidence/`, `functions/`, `UI`/`ui/` — нет
(golden не трогались).

## Остаток / риски

- **G2 (M-07 UI)**: источник `lastViewed` готов — `invoke('read_state')`, затем
  `ack_message { channelId, messageId, lastViewed, token: null }`; триггер ack
  (открытие/скролл/фокус) — политика `[A]`, контракт в `CONTRACTS/read-state.md`.
- **157 vs «3 записи»**: G6 §3.5 говорит о 3 записях, реальный READY содержит
  157, включая 2 гильдейских badge-записи (`read_state_type` 2/4) без `flags`.
  Проекция сохраняет их (порядок и `channelId` не теряются), недостающие поля —
  `null`. UI должен либо игнорировать записи с `flags == null`, либо G2 закажет
  отдельную проекцию badge (`badgeCount`/`lastAckedId`) — сейчас вне контракта.
- `read_state_type`/`badge_count`/`last_acked_id` намеренно не отдаются
  («без выдуманных полей»); при необходимости — отдельное ТЗ с оракулом.
- `flags`/`mentionCount` nullable: у реальных записей они есть всегда, у badge —
  отсутствуют; выдумывать 0 запрещено. Если PM зафиксирует `flags: u64`
  обязательным, потребуется решение по badge-записям (фильтр или отдельный
  тип), это изменение контракта.
