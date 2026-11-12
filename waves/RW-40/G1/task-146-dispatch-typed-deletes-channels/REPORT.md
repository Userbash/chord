# Отчёт воркера — task-146 (F-GW-03 / F-GW-04): типизированный разбор `MESSAGE_DELETE*`, `CHANNEL_*`, `THREAD_*` в `dispatch.rs`

## 1. Batch / карточка

- **Волна:** `RW-40`, отдел `G1` (Backend).
- **Карточка:** `task-146`, находка `F-GW-04` / `F-GW-03` (`waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §3.3, `REVIEW-50A.md` P2-3).
- **Файл воркера:** только `rust/crates/discord-core/src/dispatch.rs`.
- **Ветка / worktree:** `feature/task-146-dispatch-typed-deletes-channels`, HEAD `4e24048`.
- **Независимый оракул:**
  1. Веб-бандл Discord 1.0.162 `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (md5 `934b1289b7ebf17bc0fd09569b23b438`):
     - `@4 923 794`: `eH(["MESSAGE_DELETE"], e => f.D.loadGuildIds([e.guild_id]), e => { eW({type:"MESSAGE_DELETE", guildId:e.guild_id, id:e.id, channelId:e.channel_id}) })`;
     - `@4 923 864`: `eH(["MESSAGE_DELETE_BULK"], e => f.D.loadGuildIds([e.guild_id]), e => { eW({type:"MESSAGE_DELETE_BULK", guildId:e.guild_id, ids:e.ids, channelId:e.channel_id}) })`;
     - `@4 925 216`: `eH(["CHANNEL_CREATE","CHANNEL_DELETE"], e => f.D.loadGuildIds([e.guild_id]), (e, t) => { eW({type:t, channel:(0,ed.UE)(e)}) })`;
     - `@4 925 551`: `eH(["CHANNEL_UPDATE"], e => f.D.loadGuildIds([e.guild_id]), e => { eG.add(e) })`;
     - `@4 925 596`: `eH(["THREAD_CREATE","THREAD_UPDATE","THREAD_DELETE"], e => f.D.loadGuildIds([e.guild_id]), (e, t) => { let{newly_created:n,...i}=e; eW({type:t, isNewlyCreated:n, channel:(0,ed.UE)(i)}) })`;
     - `@8 897 000..8 903 000` (`ed.UE` / `fromServer`): поля `id`, `guild_id`, `type`, `name`, `parent_id`.
  2. `reference-oracle/gateway-protocol.json` (md5 `754fc8fa493d566bf1e46e9eebb82ff6`), `idChecks`: 64-битные Snowflake ID `> 2^53` в ETF `SMALL_BIG_EXT` (`guild.id = 1014537851418980372` / `6e0800143004f5cc5c140e`, `channel.id = 1176785373989445703` / `6e08004720841110c85410`, `message.id = 1561632596763353163`, `1561624484056793151`, `1561379697990959244`) и отрицательные значения старого сломанного парсера (`-1467272931325252608`), которые обязаны отвергаться.

## 2. Что сделано

В `rust/crates/discord-core/src/dispatch.rs` (с полным сохранением `DispatchKind::ALL: [DispatchKind; 21]` и всех 4 публичных полей `DispatchEvent` для обратной совместимости):

1. Добавлен enum `ExtendedDispatchKind` (алиас `LifecycleDispatchKind`) из 8 вариантов (`MessageDelete`, `MessageDeleteBulk`, `ChannelCreate`, `ChannelUpdate`, `ChannelDelete`, `ThreadCreate`, `ThreadUpdate`, `ThreadDelete`) с `ALL`, `NAMES`, `from_name`, `as_str`, `is_message_delete`, `is_channel_lifecycle`, `is_thread_lifecycle`.
2. Добавлены типизированные структуры дельт:
   - `MessageDeleteDispatch` (алиас `MessageDeleteDelta`): `id: u64`, `channel_id: u64`, `guild_id: Option<u64>`;
   - `MessageDeleteBulkDispatch` (алиас `MessageDeleteBulkDelta`): `ids: Vec<u64>`, `channel_id: u64`, `guild_id: Option<u64>`;
   - `ChannelLifecycleDispatch` (алиас `ChannelDispatchDelta`): `id: u64`, `guild_id: Option<u64>`, `kind: Option<i64>` (из `d.type` / `d.kind`), `name: Option<String>`, `parent_id: Option<u64>`;
   - `ThreadLifecycleDispatch` (алиас `ThreadDispatchDelta`): `id: u64`, `guild_id: Option<u64>`, `kind: Option<i64>`, `name: Option<String>`, `parent_id: Option<u64>`, `newly_created: Option<bool>`.
3. Добавлены `TypedDispatch`, `RoutedTypedDispatch` и типизированная ошибка `DispatchParseError` (`InvalidJson`, `InvalidEnvelope`, `MissingField`, `InvalidFieldType`, `InvalidSnowflake`, `EmptyBulkIds`).
4. На `DispatchEvent` добавлены методы `extended_kind`, `is_extended`, `is_known`, `parse_typed`, а на уровне модуля — парсеры конвертов и полезных нагрузок из ETF и JSON (`parse_typed_dispatch_etf`, `parse_typed_dispatch_json`, `route_typed_dispatch`, `route_typed_dispatch_json_value`, `route_typed_dispatch_json`), поддерживающие как шлюзовой формат `{"op":0,"t":...,"s":...,"d":...}`, так и конверт моста `{"kind":...,"data":...}` без прохода через `f64` и без потери точности 64-битных Snowflake ID.

## 3. RED-прогон

До реализации логики парсинга в `dispatch.rs` написаны 5 контрактных тестов в `dispatch::tests` и зафиксирован лог падения:

Команда:
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib dispatch::tests
```

Результат (`red.log` / `RED.log`, `EXIT=101`, 0 passed, 5 failed):
- `canonical_21_dispatch_kinds_preserved_and_disjoint_from_8_extended_kinds` — **FAILED** (`from_name должен находить MESSAGE_DELETE`);
- `message_delete_and_bulk_etf_and_json_preserve_64bit_snowflakes` — **FAILED** (`routed_event.extended_kind() == None`);
- `channel_create_update_delete_etf_and_json_typed_parsing` — **FAILED** (`CHANNEL_CREATE маршрутизируется`);
- `thread_create_update_delete_etf_and_json_typed_parsing` — **FAILED** (`THREAD_CREATE маршрутизируется`);
- `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` — **FAILED** (`MESSAGE_DELETE распознан`).

## 4. GREEN-прогон и проверки

| Проверка | Команда | Результат | Лог |
|---|---|---|---|
| Targeted unit (`dispatch::tests`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib dispatch::tests` | **5 passed, 0 failed** (`EXIT=0`) | `green-lib.log` |
| Все тесты `discord-core` | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` | **53 набора, 582 passed, 0 failed** (`EXIT=0`) | `green.log` / `GREEN.log` |
| Clippy (`discord-core`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | **0 предупреждений** (`EXIT=0`) | `CLIPPY.log` |
| Форматирование | `cd rust && cargo fmt --all -- --check` | **чисто** (`EXIT=0`) | `FMT.log` |

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/dispatch.rs` | `e33cc7d7821be717bfa6d3a5006462e2` |
| `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/red.log` | `05853a123d02944da34f901df29a4418` |
| `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/green-lib.log` | `c118e3077fee27106197f8825c9d4947` |
| `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/green.log` | `93f3011d7e8c893b68ac62b0f520e3d0` |
| `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/CLIPPY.log` | `aa782936f3187b846dc81ba3c97c3356` |
| `waves/RW-40/G1/task-146-dispatch-typed-deletes-channels/FMT.log` | `fdbdc3d0c7b403251ac5407380220523` |

## 6. Риски и совместимость

- **Обратная совместимость `DispatchKind::ALL` (21 вариант):** `DispatchKind` и его первый блок `impl DispatchKind` сохранены без изменений, поэтому исчерпывающие `match` в `state/types.rs`, интеграционный тест `tests/dispatch_router.rs` и JS-оракул `coreWireNames()` в `tests/unit/dispatch_ui.test.mjs` работают без регрессий.
- **Интеграция с `state/types.rs` и `gui-spike/src/net.rs`:** подключение `TypedDispatch` к редьюсеру состояния (`state/types.rs`, `state/reducer.rs` — общие файлы Team Lead) и расширение списка форвардинга `net.rs` находятся вне файлового скоупа `task-146` (`ТОЛЬКО rust/crates/discord-core/src/dispatch.rs`).
