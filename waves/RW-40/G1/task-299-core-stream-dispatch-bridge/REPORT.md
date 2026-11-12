# REPORT: task-299 (CORE-FIX-01) — Мост входящих событий `STREAM_*` в `rust/crates/discord-core/src/dispatch.rs`

- **Карточка:** `STUB-27-CORE-DISPATCH-INTEGRATION` (`task-299`)
- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Ветка:** `bugfix/task-299-core-stream-dispatch-bridge`
- **Write-scope:** строго ровно 1 файл `rust/crates/discord-core/src/dispatch.rs`

---

## 1. Что сделано

В `rust/crates/discord-core/src/dispatch.rs` реализован мост входящих событий стримов Go Live:
1. `DispatchKind::STREAM_NAMES` (`["STREAM_CREATE", "STREAM_SERVER_UPDATE", "STREAM_UPDATE", "STREAM_DELETE"]`) и метод `DispatchKind::is_stream_name`.
2. Перечисление `StreamDispatchKind` (`StreamCreate`, `StreamServerUpdate`, `StreamUpdate`, `StreamDelete`), константа `ALL`, сопоставление `from_name` и метод `as_str`.
3. Структура `RoutedStreamDispatch` с сохранением `seq` (`s`), типом `kind` и полезной нагрузкой `dispatch: crate::stream::StreamDispatch`, а также методом `into_action` в `crate::stream::StreamAction`.
4. Расширение `DispatchEvent`:
   - `is_known` теперь учитывает `self.kind.is_some() || self.is_extended() || self.is_stream()`;
   - методы `stream_kind`, `is_stream`, `parse_stream`, `parse_stream_with_diagnostics`, `to_stream_action`.
5. Функции верхнего уровня `route_stream_dispatch` и `route_stream_dispatch_with_diagnostics`.

---

## 2. Результаты тестов (TDD Red -> Green)

### Red-прогон
До внесения изменений прогон `cargo test -p discord-core --test dispatch_types` завершился с ошибкой компиляции (21 ошибка: отсутствовали `RoutedStreamDispatch`, `StreamDispatchKind`, `route_stream_dispatch`, `DispatchKind::STREAM_NAMES`, `is_stream` и сопутствующие методы).

### Green-прогон
```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test dispatch_types
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
Все 4 теста набора `dispatch_types.rs` пройдены:
- `t01_canonical_21_dispatch_kinds_preserved_and_disjoint_from_stream_kinds` — ok
- `t02_route_dispatch_bridges_all_21_fixture_inbound_vectors_to_stream_parse_dispatch` — ok
- `t03_negative_and_malformed_stream_dispatches_via_router` — ok
- `t04_end_to_end_reducer_replay_via_dispatch_bridge` — ok

### Unit-тесты библиотеки
```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --lib
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

---

## 3. Изменённые файлы и контрольные суммы

- `rust/crates/discord-core/src/dispatch.rs` (MD5: `afac17737709a87ecd2a51f88af152d6`)

---

## 4. Риски и блокеры
- Рисков не выявлено. Инвариант 21 типа `DispatchKind` сохранён, мост стримов строго изолирован и протестирован на 21 векторном сценарии оракула `stream-opcodes-fixtures.json`.
