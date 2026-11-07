# STUB-27-CORE-DISPATCH-INTEGRATION — отчёт воркера G1 Core Integration (RW-40)

Дата: 2026-10-30. Воркер: **G1 Core Integration**. Карточка: `STUB-27-CORE-DISPATCH-INTEGRATION`.
Базовый коммит: `4e24048`.
Контракт: `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` (§0, §2.3, §3.3, §8).
Векторные фикстуры: `waves/RW-40/VOICE/stream-opcodes-fixtures.json` (`rw40/stream-opcodes-fixtures-v1`).

---

## 1. Сделано

1. **Контрактный тест маршрутизации (RED-first)** `rust/crates/discord-core/tests/dispatch_types.rs`:
   - `t01_canonical_21_dispatch_kinds_preserved_and_disjoint_from_stream_kinds` — проверка неизменности инварианта 21 канонического типа из RW-02/RW-07 (`DispatchKind::ALL.len() == 21`, `DISPATCH_TYPE_COUNT_R7 == 21`), сверка 4 серверных типов `StreamDispatchKind::ALL` (`STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_UPDATE`, `STREAM_DELETE`) с якорем `A-ROUTER` (`@4937830`, `len = 576`) и таблицей `opcodes` из `stream-opcodes-fixtures.json`, проверка непересечения множеств `DispatchKind::ALL` и `StreamDispatchKind::ALL`, регистрозависимости и отказа на исходящие/неизвестные имена;
   - `t02_route_dispatch_bridges_all_21_fixture_inbound_vectors_to_stream_parse_dispatch` — маршрутизация всех 21 векторов `inbound` из фикстуры (в обоих представлениях `t`: `Atom` и `Str`) через `route_dispatch`, `DispatchEvent::stream_kind`, `DispatchEvent::is_stream`, `DispatchEvent::is_known`, `DispatchEvent::parse_stream`, `DispatchEvent::to_stream_action` и `route_stream_dispatch` к `crate::stream::parse_dispatch`, а также проверка игнорирования `VOICE_STATE_UPDATE` (`in-voice-state-self-stream-false`) как не-стримового события (`parse_stream() == None`, `route_stream_dispatch() == None`);
   - `t03_negative_and_malformed_stream_dispatches_via_router` — проверка всех 5 негативных векторов `inbound` из фикстуры (жёсткие ошибки `invalid_dispatches` и мягкие `malformed_fields` через `parse_stream_with_diagnostics` и `route_stream_dispatch_with_diagnostics`), инварианта §3.3 (битый `STREAM_*` сохраняет `is_stream() == true` и `is_known() == true`, не притворяясь неизвестным типом), отказа на не-`op0` кадры (`op 18..=22`, `10`, `11`) и неизвестные имена (`STREAM_FOO`, `STREAM_WATCH`, `FUTURE_EVENT_X`, `stream_create`);
   - `t04_end_to_end_reducer_replay_via_dispatch_bridge` — сквозной реплей всех 14 сценариев `reducer` (`R-01`…`R-13`) из `stream-opcodes-fixtures.json` через полные `op0` ETF-кадры: `route_dispatch(&frame)` → `routed.to_stream_action()` → `stream::apply(&mut state, action)`.

2. **Интеграционный мост в диспетчере** `rust/crates/discord-core/src/dispatch.rs`:
   - сохранён неизменным инвариант 21 живого типа захвата R7 в `DispatchKind` (`DispatchKind::ALL: [DispatchKind; 21]`), добавлены `DispatchKind::STREAM_NAMES` и `DispatchKind::is_stream_name`;
   - добавлены `StreamDispatchKind` (`StreamCreate`, `StreamServerUpdate`, `StreamUpdate`, `StreamDelete`, `ALL`, `from_name`, `as_str`) и `RoutedStreamDispatch` (`kind`, `seq`, `dispatch`, `into_action`);
   - на `DispatchEvent` (без изменения четырёх публичных полей `kind`, `name`, `seq`, `data`) добавлены методы `stream_kind`, `is_stream`, `is_known`, `parse_stream`, `parse_stream_with_diagnostics`, `to_stream_action`, `to_stream_action_with_diagnostics`;
   - добавлены функции верхнего уровня `route_stream_dispatch` и `route_stream_dispatch_with_diagnostics`.

---

## 2. RED-прогон (до правок в `src/dispatch.rs`)

Команда:
```bash
(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo test --offline -p discord-core --test dispatch_types) > waves/RW-40/PM/stub27-dispatch/red.log 2>&1
```
- Exit code: `101`
- Первая ошибка: `error[E0432]: unresolved imports discord_core::dispatch::RoutedStreamDispatch, discord_core::dispatch::StreamDispatchKind, discord_core::dispatch::route_stream_dispatch, discord_core::dispatch::route_stream_dispatch_with_diagnostics` (`crates/discord-core/tests/dispatch_types.rs:20:19`)
- Лог: `waves/RW-40/PM/stub27-dispatch/red.log`

---

## 3. GREEN-прогон и проверки `discord-core`

| Команда | Exit code | Результат | Артефакт |
|---|---:|---|---|
| `(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo test --offline -p discord-core --test dispatch_types)` | `0` | **4 passed; 0 failed** (`t01`…`t04`) | — |
| `(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo test --offline -p discord-core)` | `0` | **583 passed; 0 failed** (все 55 тестовых таргетов `discord-core`, включая `dispatch_router`, `dispatch_types`, `stream_gateway_contract`, `state_types`, `state_reducer`) | `waves/RW-40/PM/stub27-dispatch/green.log` |
| `(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo clippy --offline -p discord-core --test dispatch_types -- -D warnings)` | `0` | 0 предупреждений | — |
| `(cd rust && rustfmt --edition 2024 --check crates/discord-core/src/dispatch.rs crates/discord-core/tests/dispatch_types.rs)` | `0` | чисто | — |

---

## 4. Мутационное тестирование (`mutations.log`)

Команда прогона каждого мутанта:
```bash
(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-disp2 cargo test --offline -p discord-core --test dispatch_types)
```
Baseline до мутаций:
- `rust/crates/discord-core/src/dispatch.rs` = `528597e9f5aa4b894982cbd3f5cbda38`
- `rust/crates/discord-core/tests/dispatch_types.rs` = `af859f628901986aa523b01ea1c507b1`
- Результат baseline: `4 passed; 0 failed` (`exit=0`)

Результаты (5/5 KILLED, лог: `waves/RW-40/PM/stub27-dispatch/mutations.log`):

| Мутант | Описание мутации в `src/dispatch.rs` | md5 мутанта | Статус / Exit | Убит тестами | Restore md5 |
|---|---|---|---|---|---|
| **M1** | Размытие непересечения 21 канонического типа R7 и `STREAM_*`: `DispatchKind::from_name` принимает `"STREAM_CREATE"` как `Some(Self::Ready)` | `4d729ad502a72f6d4cb8e90615b5b20e` | **KILLED** (`2 passed; 2 failed`, `exit=101`) | `t01_canonical_21_dispatch_kinds_preserved_and_disjoint_from_stream_kinds`, `t02_route_dispatch_bridges_all_21_fixture_inbound_vectors_to_stream_parse_dispatch` | `528597e9f5aa4b894982cbd3f5cbda38` (совпал) |
| **M2** | Потеря маршрута `"STREAM_DELETE"` в `StreamDispatchKind::from_name` (`=> None`) | `e206e4075cd55c9f1cfce28b16e7349c` | **KILLED** (`1 passed; 3 failed`, `exit=101`) | `t01_canonical_21_dispatch_kinds_preserved_and_disjoint_from_stream_kinds`, `t02_route_dispatch_bridges_all_21_fixture_inbound_vectors_to_stream_parse_dispatch`, `t03_negative_and_malformed_stream_dispatches_via_router` | `528597e9f5aa4b894982cbd3f5cbda38` (совпал) |
| **M3** | Отключение проверки `op == 0` в `route_dispatch` (пропуск не-`op0` кадров `op 18..=22`, `10`, `11`) | `26cd6b2705101f61e4ca56cafba79af1` | **KILLED** (`3 passed; 1 failed`, `exit=101`) | `t03_negative_and_malformed_stream_dispatches_via_router` | `528597e9f5aa4b894982cbd3f5cbda38` (совпал) |
| **M4** | Нарушение инварианта §3.3 в `DispatchEvent::is_known` (`self.kind.is_some()` без `|| self.is_stream()`) | `029aafd6547db53d7ea72d929cb48d59` | **KILLED** (`2 passed; 2 failed`, `exit=101`) | `t02_route_dispatch_bridges_all_21_fixture_inbound_vectors_to_stream_parse_dispatch`, `t03_negative_and_malformed_stream_dispatches_via_router` | `528597e9f5aa4b894982cbd3f5cbda38` (совпал) |
| **M5** | Игнорирование диагностических счётчиков `StreamDiagnostics` в `parse_stream_with_diagnostics` (вызов `parse_dispatch` вместо `parse_dispatch_with_diagnostics`) | `7b4c8a0af1168fbecf6544c335cc7131` | **KILLED** (`3 passed; 1 failed`, `exit=101`) | `t03_negative_and_malformed_stream_dispatches_via_router` | `528597e9f5aa4b894982cbd3f5cbda38` (совпал) |

Post-restore targeted: `4 passed; 0 failed` (`exit=0`), финальные `md5` `src/dispatch.rs` (`528597e9f5aa4b894982cbd3f5cbda38`) и `tests/dispatch_types.rs` (`af859f628901986aa523b01ea1c507b1`) идентичны baseline.

---

## 5. Файлы и контрольные суммы (md5)

| Файл | Статус | md5 |
|---|---|---|
| `rust/crates/discord-core/src/dispatch.rs` | modified | `528597e9f5aa4b894982cbd3f5cbda38` |
| `rust/crates/discord-core/tests/dispatch_types.rs` | new | `af859f628901986aa523b01ea1c507b1` |
| `waves/RW-40/PM/stub27-dispatch/red.log` | new | `ec946f80e3818d6a067d6b0c4ec9b100` |
| `waves/RW-40/PM/stub27-dispatch/green.log` | new | `87225e8df584fb1a4bd154272ece446a` |
| `waves/RW-40/PM/stub27-dispatch/mutations.log` | new | `c7ddeb75c0dcc766c3b26de2af1fa900` |

Замороженные на ревью файлы `stub27-core` не изменялись (md5 неизменны):
- `rust/crates/discord-core/src/stream.rs`: `e4e0450559bcf947261ca7bc91aea91f`
- `rust/crates/discord-core/tests/stream_gateway_contract.rs`: `bfc82f9d2dacc7bccb1e1475ddb5c7c0`

---

## 6. Риски и границы карточки

1. **Совместимость с RW-02/RW-07 (`dispatch_router.rs`) и `state/types.rs`**: `DispatchKind::ALL` (21 живой тип захвата R7) и поля структуры `DispatchEvent` сохранены без изменений, поэтому все существующие тесты роутера и редьюсеров проходят без правок чужих файлов.
2. **Запреты соблюдены**: изменялись только `rust/crates/discord-core/src/dispatch.rs` и `rust/crates/discord-core/tests/dispatch_types.rs` (+ артефакты `waves/RW-40/PM/stub27-dispatch/*`); `src/stream.rs`, `tests/stream_gateway_contract.rs`, `rust/gui-spike/**` не затрагивались; `git commit` не выполнялся.
