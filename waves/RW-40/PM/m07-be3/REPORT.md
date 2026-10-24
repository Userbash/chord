# M-07-BE3 — badge-проекция `read_state` (red-first)

Дата: 2026-10-24. Воркер: **G1 Backend**. PM: DeepSeek.
Статус: **готово к ревью**. Окно/live/сеть/keyring/commit и полный `gate:worker` не
использовались; UI/`fixtures/`/`logs/`/`evidence/`/`functions/` не тронуты.

## Контракт (источник)

- `waves/RW-40/PARITY/G6-REPARITY-03.md` §1.1, строка **M-07**: «`data.rs` отдаёт
  ровно 6 полей и **исключает** `badge_count`/`last_acked_id`/`read_state_type`;
  … G1 badge-проекция (аддитивно, оракул READY)».
- `docs/re/gateway-schema.md` §`READY.d.read_state.entries[] (157)`: 9 ключей.
  Присутствие в реальном артефакте: `flags` 155/157, `last_message_id` 155/157,
  `last_pin_timestamp` 155/157, `mention_count` 155/157, `last_viewed` 143/157,
  `badge_count` **2/157** (16), `last_acked_id` **2/157** (bignum),
  `read_state_type` **2/157** (2).
- `fixtures/gateway/by-type/002-READY.json`: те же 157 записей; badge-форма —
  `{id: 384299970632876032, badge_count: 16, last_acked_id: "1294268550826950656",
  read_state_type: 2}` и `{id: 1014537851418980372, badge_count: 0,
  last_acked_id: "1237379809395605509", read_state_type: 4}`. Ни одного `null`
  в артефакте; 14 записей без `last_viewed` (2 badge + 12 без поля).
- `docs/rust-client/CODE-STANDARD.md` §RW-17: «`badge_count=0` — валидное поле
  READY, но не бейдж», предикат `> 0` — политика селектора/UI, не проекции.
- `docs/re/gateway-schema.md` + `rust/crates/discord-core/src/ready/read_state.rs`
  (P2-07, принято): `badge_count: Option<i64>`, `last_acked_id` — bignum,
  `read_state_type: Option<i64>`; семантика 2/4 не документирована RE-оракулом.

## API (аддитивно, без выдуманных имён)

`rust/gui-spike/src/data.rs`, `ReadStateEntry` — было 6 полей, стало 9; новые имена —
точный camelCase ключей READY (`serde rename_all = camelCase`), ничего сверх READY:

```
{ channelId, flags, lastMessageId, lastPinTimestamp, lastViewed, mentionCount,
  badgeCount, lastAckedId, readStateType }
```

| Поле READY | IPC | Тип | Правило |
|---|---|---|---|
| `badge_count` | `badgeCount` | `Option<i64>` | число как есть; `0` — значение, не null; отсутствие → null |
| `last_acked_id` | `lastAckedId` | `Option<String>` | decimal-строка READY: snowflake `1294268550826950656` > JS-safe integer, как `lastMessageId`; отсутствие → null |
| `read_state_type` | `readStateType` | `Option<i64>` | число как есть, без интерпретации 2/4 |

- `mentionCount` **не потерян**: без изменений, 155/157 в оракуле.
- `flags` уже проецировался (155/157) — не менялся, проверен оракулом повторно.
- `null ≠ 0`: отсутствующее/`nil`/`null` → `null`; `badge_count = 0` → `Some(0)`.
- Порядок записей — как в READY (пересортировки нет), проверен отдельным assert
  по id-последовательности.
- Типизированные ошибки: `badge_count`/`read_state_type`/`last_acked_id` не того
  типа → `Parse` с `READY.read_state.entries[i]` (новый `optional_i64`, `optional_str`);
  ошибка не маскируется `null`/пустым списком.

`rust/gui-spike/src/main.rs` **не менялся**: команда `read_state` делегирует в
`data::read_state(&root)`, форма IPC не расширялась на уровне команды.

## RED (до продуктового кода)

`waves/RW-40/PM/m07-be3/red.log` — тесты написаны первыми, полей ещё нет:

```
error[E0560]: struct `ReadStateEntry` has no field named `badge_count`
   --> tests/read_state_contract.rs:111:13
error[E0560]: struct `ReadStateEntry` has no field named `last_acked_id`
error[E0560]: struct `ReadStateEntry` has no field named `read_state_type`
...
error: could not compile `discord-gui-spike` (test "read_state_contract") due to 15 previous errors
error: could not compile `discord-gui-spike` (test "data_contract") due to 8 previous errors
EXIT:101
```

Изменение ожиданий двух существующих тестов — **расширение контракта, не
ослабление**: «ровно шесть полей» → «ровно девять», badge-запись «всё null» →
«три badge-поля из READY». Добавлены (не удалены) проверки `null`, `0` и `Parse`;
ни один assert после red не убран.

## GREEN

| Прогон | Результат | Лог |
|---|---|---|
| `cargo test --test read_state_contract` | **15 passed / 0 failed** (было 12; +3) | `green-targeted-final.log` |
| `cargo test --test data_contract` | **30 passed / 0 failed** (было 30; оракул расширен) | `green-targeted-final.log` |
| `cargo test` (весь crate `gui-spike`) | **285 passed / 0 failed**, EXIT 0 | `green-gui-final.log` |
| `cargo clippy --all-targets -- -D warnings` | EXIT 0 | `clippy.log` |
| `cargo fmt -- --check` | EXIT 0 | `fmt.log` |
| `node --test tests/unit/read-state-ui.test.mjs` (регрессия UI, не менялся) | **8 passed / 0 failed** | `green-read-state-ui.log` |

Полный `gate:worker`/`test --workspace`/e2e/live не запускались (запрет ТЗ).

## Тесты

`rust/gui-spike/tests/read_state_contract.rs` (15):

1. `projects_three_entries_with_last_viewed_4310_and_nulls` — три записи, структура
   целиком (новые поля `None` у обычных записей), `last_viewed = 4310`.
2. `serializes_exactly_nine_camel_case_fields_typed_like_ready` — ровно 9 ключей на
   запись; `lastViewed`/`flags`/`mentionCount` — числа; `badgeCount`/`lastAckedId`/
   `readStateType` = `null` у обычной записи; `nil` → `null`, не 0.
3. `empty_entries_is_a_valid_empty_projection`.
4–9. Ошибки: нет READY → `NotFound`; битый JSON, нет `read_state`, `entries` не
   массив, запись без `id` (индекс в тексте), `last_viewed: "не число"` → `Parse`.
10. **`badge_form_projects_badge_fields_and_missing_common_fields_as_null`** (новый
    по смыслу) — реальная badge-форма: `badgeCount = 16`, `readStateType = 2`,
    `lastAckedId = "1294268550826950656"` (строка), общие поля `null`; JSON-типы.
11. **`badge_count_zero_is_a_value_not_null`** (новый) — `badge_count = 0` →
    `Some(0)`, `read_state_type = 4`.
12. **`malformed_badge_fields_are_parse_with_entry_index`** (новый) — три кейса
    (строка в `badge_count`, bool в `read_state_type`, число в `last_acked_id`) →
    `Parse` + `entries[1]`.
13. **`explicit_null_badge_fields_are_none_not_zero`** (новый) — явный `null` в
    трёх badge-полях → `None`.
14. `entry_without_flags_is_null_not_invented_zero`.
15. `tauri_read_state_command_is_local_registered_and_silent` (wiring `main.rs`).

`rust/gui-spike/tests/data_contract.rs::read_state_matches_ready_oracle` (оракул —
сам READY, не реализация): поле-в-поле 9 ключей, порядок `channelId` = порядок
READY; независимые проверки: badge-записей ровно **2**, `badgeCount` = `[16, 0]`,
`readStateType` = `[2, 4]`, `lastAckedId` = две decimal-строки; badge-поля либо все
три, либо все `null`; `mentionCount` 155/157; `flags` 155/157; `last_viewed = 4310`
присутствует, есть записи без `last_viewed`.

## Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/data.rs` | `8013271de7a6444b2b9dba3301a165d4` |
| `rust/gui-spike/tests/read_state_contract.rs` | `fde688f33614636c2699c55ebba945e9` |
| `rust/gui-spike/tests/data_contract.rs` | `e31c912740393ec3bae6c48847ca1ef0` |

Не менялись (md5 на момент отчёта): `rust/gui-spike/src/main.rs`
`336863b5b1ffca169b851a2095798f01`, `rust/gui-spike/ui/features/chat.js`
`2e918f3cff5dc968ca3117480e306ce4`, `ui/features/actions.js`
`908bfe2dbf4202cdf28c389d44b2095f`, `tests/unit/read-state-ui.test.mjs`
`f9d6e653c31e5c8c1ab60ce04352f7d2`, `CONTRACTS/read-state.md`
`fb7ed47afc3494e667ad21d873a04ed7`.

Logs: `waves/RW-40/PM/m07-be3/{red.log, green-targeted-final.log,
green-gui-final.log, clippy.log, fmt.log, green-read-state-ui.log}`.

## Остаток / риски

- **`CONTRACTS/read-state.md` устарел (не правил — вне файлового мандата ТЗ).**
  §1 перечисляет 6 полей, §5 называет `badgeCount`/`lastAckedId` «вне контракта».
  После этой карточки это неверно: нужен аддитивный апдейт §1 (9 полей, типы,
  `badgeCount = 0` — значение) и §5 (badge спроецирован, семантика 2/4 — нет).
  Решение PM/TL: правка контракта или отдельная карточка G1/G2.
- **Doc-строка `main.rs:116-117`** перечисляет только `lastViewed/flags/mentionCount`
  — не ложна, но неполна; `main.rs` вне мандата, правится владельцем команды.
- **`read_state_type` 2/4 отдаётся сырым числом**: RE-оракула семантики нет
  (`badge_count = 0` при type 4 — гипотеза «guild-level», не доказательство).
  UI не должен трактовать тип без отдельного RE/карточки G-08.
- **G2/G-08 не закрыты**: UI ещё не читает `badgeCount`/`readStateType`; unread
  без упоминаний и badge на плитке гильдии остаются вне работы до G2. Предикат
  «показывать бейдж» — `badgeCount > 0`, не «поле есть» (CODE-STANDARD RW-17).
- **`badgeCount` отрицательный** (теоретически) проецируется как есть; клампинг
  `>= 0` — политика селектора (`StateReadStateSelectors::badge_count`), не проекции.
- **`lastAckedId` — строка**, а не число IPC: если G2 ожидает число, это надо
  зафиксировать в контракте; число потеряло бы точность в JS (> 2^53).
- Живой паритет badge (реальный READY с 4+ записями, MESSAGE_ACK на badge) не
  проверялся: окно/live не использовались. Полный гейт — за integration-лейном.
- `data.rs` и `main.rs` в дереве уже разошлись с md5 отчёта `m07-be2`
  (`a99647c1…`/`6e5b0adb…` → текущие) — дрейф до меня, правки других карточек.
