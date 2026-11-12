# task-136 / BE-P1-02 — FORWARDED_DISPATCHES: все канонические типы ядра

- **batch/карточка:** RW-40 / BE-P1-02 (task-136, находка F-GW-03 из
  `waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §3.3).
- **worktree:** `/home/user/orca/workspaces/project/feature-task-136-net-forwarded-dispatches`
  (HEAD `4e24048`).
- **сделано:**
  - `FORWARDED_DISPATCHES` расширен с 6 до 22 имён: все 21 имя `DispatchKind::ALL`
    (`rust/crates/discord-core/src/dispatch.rs`) плюс `MESSAGE_REACTION_REMOVE`
    (в ядре raw, но форвардится по имени провода по `CONTRACTS/dispatch.md` §5).
  - Фильтр `map_dispatch` не менялся: он по-прежнему сверяет точное имя провода с
    `FORWARDED_DISPATCHES`, поэтому список типов и фильтр не разъезжаются.
  - Секреты READY, дедуп по `s`, `try_send` и счётчики не тронуты.
  - Обновлены doc-комментарии модуля, константы и `DispatchStats::filtered`.
- **red:** `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test net_contract`
  → 24 passed, 3 failed, exit 101. Падают:
  `forwarded_dispatches_cover_every_core_kind_and_the_raw_reaction_remove`
  (`CHANNEL_INFO не форвардится`),
  `every_core_dispatch_kind_reaches_the_kind_data_envelope` (то же),
  `all_core_kinds_reach_the_ui_channel_in_order` (в UI дошли 5 из 21, таймаут).
  Лог: `red-net-contract.log` рядом с отчётом.
- **green:** та же команда после реализации → 27 passed, 0 failed, exit 0.
  Лог: `green-net-contract.log`.
- **targeted:**
  - `net_contract`: 27/27 (лог `green-net-contract.log`).
  - `ipc_contract`: 20/20 (лог `green-ipc-contract.log`).
  - `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings`:
    exit 0 (лог `clippy.log`).
  - `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check`: exit 0.
- **oracle:** канонический список — `DispatchKind::ALL` (21 имя, union семи снимков
  `gw-proto/msgs-*.jsonl`, 1340 op0) и `DispatchKind::from_name`; имена в тестах
  подаются op0-кадром через `route_dispatch`, а не готовым `DispatchEvent`.
  `MESSAGE_REACTION_REMOVE` — `CONTRACTS/dispatch.md` §5. Снятые кадры
  `fixtures/gateway/by-type/*.json` продолжают сверяться поле-в-поле в старых тестах.
- **генерация:** не применимо (нет генераторов/фикстур, только код и тесты).
- **мутации:** не применимо для batch-воркера (мутационный прогон — у QA/integration).
- **baseline/restore:** не применимо (мутаций не было).
- **стабильность:** один прогон; тест UI-канала использует `Condvar`-ожидание без
  sleep-гонок. `ipc_contract` и `clippy` один раз стартовали одновременно и ждали
  file lock; оба завершились exit 0, на результат это не влияет.
- **gate:worker:** не запускался — по процессу полный gate делает integration/QA на batch.
- **файлы:**
  - `rust/gui-spike/src/net.rs` — `3e463b632d9e177392d10046b8b81d24`
  - `rust/gui-spike/tests/net_contract.rs` — `9ff8127759546e4404cff7096306796a`
  - `waves/RW-40/G1/task-136-net-forwarded-dispatches/REPORT.md` — этот файл.
  - Логи: `red-net-contract.log`, `green-net-contract.log`,
    `green-ipc-contract.log`, `clippy.log`, `fmt.log`.
- **риски/вопросы:**
  1. В ТЗ перечислены `MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE/UPDATE/DELETE`,
     `THREAD_CREATE/UPDATE/DELETE`, но этих имён нет в `DispatchKind::ALL`: ядро отдаёт их
     raw (`kind: None`). Карточка BE-P1-02 и находка F-GW-03 требуют ровно 21 тип
     `DispatchKind::ALL`, поэтому эти 8 имён остаются отфильтрованными — тест
     `wire_names_outside_the_core_router_are_filtered` фиксирует это явно. Форвардить их
     можно только расширением core-роутера (вне моего write-scope) либо отдельной карточкой.
  2. `MESSAGE_REACTION_REMOVE` сохранён в списке сверх `DispatchKind::ALL` — иначе
     регрессировал бы контракт §5 и тест `reaction_kinds_use_wire_name_and_forward_data_as_is`.
  3. UI-фильтр `rust/gui-spike/ui/features/dispatch-model.js` (`FORWARDED_KINDS`) всё ещё
     содержит 6 имён; без его расширения новые события дойдут до Tauri-канала, но будут
     отброшены моделью UI. Это отдельная карточка UI-лейна (BE-P1-02, write-scope UI).
  4. `CONTRACTS/dispatch.md` §4 описывает 6 имён и требует правки Team Lead (не мой файл).
  5. В основном чекауте `/home/user/orca/project` файл `net.rs` уже содержит незакоммиченные
     правки SEC-L02-01 (`sink.reset()` в `connect`/`disconnect`); при интеграции возможен
     конфликт в области `NetController`, не в `FORWARDED_DISPATCHES`.
- **следующий шаг:** приёмка Team Lead → интеграция worktree, обновление
  `CONTRACTS/dispatch.md` §4 и карточка UI-лейну на `FORWARDED_KINDS`.
