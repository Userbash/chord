# REVIEW-61A — task-146 (F-GW-03 / F-GW-04): типизированный разбор `MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE/UPDATE/DELETE`, `THREAD_CREATE/UPDATE/DELETE` в `dispatch.rs`

Дата: 2026-10-30. Ревьюер: Технический ревьюер, Review 61A.
Ветка: `feature/task-146-dispatch-typed-deletes-channels`, worktree
`/home/user/orca/workspaces/project/feature-task-146-dispatch-typed-deletes-channels`,
HEAD `4e24048`.
Режим: сверка с отчётом воркера (`REPORT.md`), RE-аудитом (`RE-GAP-BACKEND-PROTOCOL.md` §3.3, `REVIEW-50A.md` P2-3) и артефактами `reference-oracle/` + аудит диффа `rust/crates/discord-core/src/dispatch.rs` + целевые/интеграционные тесты + 5/5 мутационных прогонов.

## Вердикт

**PASS. Ready for Merge.** P0 = 0, P1 = 0. P2 = 2 (follow-up в общих файлах вне write-scope этой карточки), P3 = 3.

Срез закрывает задачу `task-146` в пределах выделенного файла `rust/crates/discord-core/src/dispatch.rs`:
1. Сохранены без изменений `DispatchKind` (`DispatchKind::ALL: [DispatchKind; 21]`), функция `route_dispatch` и все 4 публичных поля `DispatchEvent` (`kind`, `name`, `seq`, `data`), что гарантирует 100%-ную обратную совместимость с существующими исчерпывающими `match` в `state/types.rs`, тестами `tests/dispatch_router.rs` и JS-оракулом `coreWireNames()`.
2. Добавлено перечисление `ExtendedDispatchKind` (алиас `LifecycleDispatchKind`) из 8 типов (`MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE`, `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`) с константами `ALL`, `NAMES` и предикатами категорий (`is_message_delete`, `is_channel_lifecycle`, `is_thread_lifecycle`).
3. Реализованы типизированные структуры дельт (`MessageDeleteDispatch`, `MessageDeleteBulkDispatch`, `ChannelLifecycleDispatch`, `ThreadLifecycleDispatch`), объединённый enum `TypedDispatch`, обёртка `RoutedTypedDispatch` и типизированная ошибка `DispatchParseError` с реализацией `Display` и `std::error::Error`.
4. Реализован разбор всех 8 типов как из ETF (`parse_typed_dispatch_etf`, `route_typed_dispatch`, `DispatchEvent::parse_typed`), так и из JSON (`parse_typed_dispatch_json`, `route_typed_dispatch_json_value`, `route_typed_dispatch_json`) с поддержкой шлюзового конверта `{"op":0,"t":...,"s":...,"d":...}` и конверта моста `{"kind":...,"data":...}`.
5. Snowflake ID (`> 2^53`) разбираются в точные `u64` как из строковых/атомных представлений (`"1561632596763353163"`), так и из целочисленных (`EtfValue::Int` с проверкой `*number >= 0`, `JsonNumber::as_u64()` по лексической записи без прохода через `f64`). Отрицательные `i64` старого парсера (`-1467272931325252608`), числа с плавающей точкой и переполнения `> u64::MAX` детерминированно отвергаются с `DispatchParseError::InvalidSnowflake`.

---

## 1. Проверка прогонов и артефактов воркера

Все команды выполнены последовательно с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:

| Проверка | Команда | Результат | Лог ревью |
|---|---|---|---|
| Unit-тесты `dispatch` | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib dispatch` | **5 passed, 0 failed** (`EXIT=0`) | `REVIEW-61A-GREEN-LIB.log` |
| Интеграционный тест роутера (`dispatch_router`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --test dispatch_router` | **12 passed, 0 failed** (`EXIT=0`) | `REVIEW-61A-DISPATCH-ROUTER-IT.log` |
| Clippy (`discord-core --all-targets`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | **0 предупреждений** (`EXIT=0`) | `REVIEW-61A-CLIPPY.log` |
| Форматирование (`rustfmt`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo fmt --manifest-path rust/Cargo.toml -p discord-core -- --check` | **чисто** (`EXIT=0`) | `REVIEW-61A-FMT.log` |

Контрольные суммы всех файлов и логов воркера перепроверены и побайтно совпадают с `REPORT.md` §5:
- `rust/crates/discord-core/src/dispatch.rs` — `e33cc7d7821be717bfa6d3a5006462e2`
- `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/red.log` — `05853a123d02944da34f901df29a4418` (0 passed, 5 failed, `EXIT=101`)
- `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/green-lib.log` — `c118e3077fee27106197f8825c9d4947`
- `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/green.log` — `93f3011d7e8c893b68ac62b0f520e3d0`
- `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/CLIPPY.log` — `aa782936f3187b846dc81ba3c97c3356`
- `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/FMT.log` — `fdbdc3d0c7b403251ac5407380220523`

---

## 2. Сверка с RE-оракулом и контрактом

1. **Веб-бандл Discord 1.0.162 (`reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`, md5 `934b1289b7ebf17bc0fd09569b23b438`):**
   - `MESSAGE_DELETE` (`@4923794`): `id`, `channel_id`, опциональный `guild_id` (для DM отсутствует или `null`) — полностью соответствует `MessageDeleteDispatch`.
   - `MESSAGE_DELETE_BULK` (`@4923864`): `ids` (непустой массив Snowflake в порядке провода), `channel_id`, опциональный `guild_id` — соответствует `MessageDeleteBulkDispatch`.
   - `CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE` (`@4925216`, `@4925551`, `ed.UE` `@8897000..8903000`): обязательный `id`, опциональные `guild_id`, `type` (или `kind`), `name`, `parent_id` — соответствует `ChannelLifecycleDispatch`.
   - `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE` (`@4925596`): выделение `newly_created: Option<bool>` плюс поля канала/ветки (`id`, `guild_id`, `type`/`kind`, `name`, `parent_id`) — соответствует `ThreadLifecycleDispatch`.
2. **Точность 64-битных Snowflake ID (`> 2^53`):**
   - В `etf_parse_snowflake` (`dispatch.rs:570-579`) `EtfValue::Int(number)` принимает только неотрицательные `i64` (декодированные из `SMALL_BIG_EXT` в `discord_codec::etf`), а `EtfValue::Str` / `EtfValue::Atom` проверяют `text.bytes().all(|b| b.is_ascii_digit())` и парсят напрямую в `u64` (вплоть до `u64::MAX`).
   - В `json_parse_snowflake` (`dispatch.rs:864-873`) `JsonValue::Number` вызывает `JsonNumber::as_u64()`, который в `discord-json` (`lib.rs:150-155`) разбирает исходное лексическое представление числа через `str::parse::<u64>()` без конвертации в `f64` и отвергает любые числа с `-`, `.`, `e`/`E`. Строковые Snowflake (`JsonValue::String`) разбираются тем же целочисленным парсером.
3. **Обратная совместимость 21 варианта `DispatchKind::ALL`:**
   - `DispatchKind::ALL` содержит ровно 21 канонический вариант снимка R7; `DispatchKind::from_name` возвращает `None` для всех 8 новых имён, а `ExtendedDispatchKind::from_name` возвращает `Some(...)` только для 8 новых имён и `None` для 21 канонического типа. Пересечений между множествами нет (`is_disjoint`).

---

## 3. Мутационные проверки (5/5 убиты)

Сводка сохранена в `REVIEW-61A-MUTATION-SUMMARY.md`, полные логи — в `REVIEW-61A-MUTATION-M1.log` … `REVIEW-61A-MUTATION-M5.log`. После серии мутаций `rust/crates/discord-core/src/dispatch.rs` восстановлен побайтно (`md5 = e33cc7d7821be717bfa6d3a5006462e2`) и перепроверен чистым прогоном тестов.

| # | Мутация | Место | Результат | Поймавшие тесты |
|---|---|---|---|---|
| **M1** | Пропуск числового JSON Snowflake через `f64` (`number.as_u64().map(\|n\| (n as f64) as u64)`) — потеря младших бит для ID `> 2^53` | `dispatch.rs:866` | **KILLED** (`EXIT=101`, 2 passed / 3 failed) | `message_delete_and_bulk_etf_and_json_preserve_64bit_snowflakes`, `channel_create_update_delete_etf_and_json_typed_parsing`, `thread_create_update_delete_etf_and_json_typed_parsing` |
| **M2** | Удаление `"MESSAGE_DELETE_BULK"` из `ExtendedDispatchKind::from_name` (`=> None`) | `dispatch.rs:181` | **KILLED** (`EXIT=101`, 2 passed / 3 failed) | `canonical_21_dispatch_kinds_preserved_and_disjoint_from_8_extended_kinds`, `message_delete_and_bulk_etf_and_json_preserve_64bit_snowflakes`, `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` |
| **M3** | Принятие отрицательных `i64` в `etf_parse_snowflake` через каст `Some(*number as u64)` | `dispatch.rs:572` | **KILLED** (`EXIT=101`, 4 passed / 1 failed) | `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` |
| **M4** | Игнорирование `newly_created` в `parse_thread_lifecycle_etf` (`let newly_created = None;`) | `dispatch.rs:799` | **KILLED** (`EXIT=101`, 4 passed / 1 failed) | `thread_create_update_delete_etf_and_json_typed_parsing` |
| **M5** | Удаление проверки `if items.is_empty()` (`EmptyBulkIds`) в `parse_message_delete_bulk_json` | `dispatch.rs:1033-1035` | **KILLED** (`EXIT=101`, 4 passed / 1 failed) | `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` |

---

## 4. Findings

**P0:** нет.

**P1:** нет.

**P2-1. Подключение `TypedDispatch` к `state/types.rs`, `state/reducer.rs`, `ws.rs` и `lib.rs` (integration-лейн Team Lead).**
В рамках файлового скоупа `task-146` (разрешён только `rust/crates/discord-core/src/dispatch.rs`) общие файлы `rust/crates/discord-core/src/lib.rs`, `state/types.rs`, `state/reducer.rs` и `ws.rs` не трогались. На этапе интеграции Team Lead потребуется:
- ре-экспортировать `ExtendedDispatchKind`, `TypedDispatch`, `RoutedTypedDispatch`, `DispatchParseError`, `route_typed_dispatch*` в `lib.rs` (если они нужны внешним крейтам на верхнем уровне `discord_core`);
- подключить `DispatchEvent::parse_typed()` в `state/types.rs` / `state/reducer.rs` для мутации `State::messages`, `State::guilds` и `State::private_channels`;
- в `ws.rs:285` при необходимости учитывать `event.is_known()` вместо `event.kind.is_none()` для счётчика `unknown_dispatches`.

**P2-2. Форвардинг 8 событий через мост `gui-spike/src/net.rs` и UI-модель (`F-GW-03`).**
Как отмечено в `REVIEW-50A.md` (P2-3), `FORWARDED_DISPATCHES` в `rust/gui-spike/src/net.rs` сейчас пропускает 21 тип `DispatchKind::ALL` + `MESSAGE_REACTION_REMOVE`. Чтобы типизированные дельты удалений и каналов/веток доходили до UI, в отдельной карточке `net.rs` / UI нужно добавить `ExtendedDispatchKind::NAMES` в белый список моста.

**P3-1. Асимметрия негативного теста `EmptyBulkIds` между JSON и ETF.**
В `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` (`dispatch.rs:1682-1689`) пустой массив `d.ids` (`DispatchParseError::EmptyBulkIds`) проверяется только на JSON-пути (`route_typed_dispatch_json`), тогда как зеркальная ветка в `parse_message_delete_bulk_etf` (`dispatch.rs:749-751`) не имеет отдельного негативного ассерта в ETF. Реализация в обоих парсерах корректна; при следующем касании `dispatch::tests` стоит добавить 1 ассерт `route_typed_dispatch` с `ids: EtfValue::List(vec![])`.

**P3-2. Зарезервированный вариант `DispatchParseError::InvalidEnvelope`.**
Вариант `DispatchParseError::InvalidEnvelope { reason }` (`dispatch.rs:405-408`) объявлен в публичном перечислении и поддержан в `Display`, но внутри `dispatch.rs` сейчас не конструируется: не-op0 и не-объектные верхнеуровневые конверты возвращают `None` (в соответствии с контрактом `route_dispatch`), а не-объектный `d` возвращает `DispatchParseError::InvalidFieldType { field: "d", expected: "map", .. }`. Оставить для внешних потребителей или использовать в `route_typed_dispatch_json` при строгом режиме — на усмотрение Team Lead.

**P3-3. Побочная модификация `package-lock.json` в worktree.**
В `git status --short` присутствует стандартное для worktree расхождение `M package-lock.json`. В коммит/мерж карточки `task-146` должен входить только `rust/crates/discord-core/src/dispatch.rs` (`md5 = e33cc7d7821be717bfa6d3a5006462e2`).
