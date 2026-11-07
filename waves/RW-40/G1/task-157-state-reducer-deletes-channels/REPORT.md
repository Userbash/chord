# task-157 (BE-P1-02 / F-GW-06) — Отчёт воркера G1 Backend

- batch/карточка: `RW-40 / task-157 (BE-P1-02 / F-GW-06)`
- сделано:
  * В `rust/crates/discord-core/src/state/types.rs`:
    - Добавлены варианты в `EventPayload`: `MessageDelete(MessageDeleteDispatch)`, `MessageDeleteBulk(MessageDeleteBulkDispatch)`, `ChannelCreate(ChannelLifecycleDispatch)`, `ChannelUpdate(ChannelLifecycleDispatch)`, `ChannelDelete(ChannelLifecycleDispatch)`, `ThreadCreate(ThreadLifecycleDispatch)`, `ThreadUpdate(ThreadLifecycleDispatch)`, `ThreadDelete(ThreadLifecycleDispatch)`.
    - Добавлены алиасы типов: `MessageDeleteEvent`, `MessageDeleteBulkEvent`, `ChannelEvent`, `ThreadEvent`.
    - Реализовано подключение типизированного разбора 8 событий через `dispatch.parse_typed()` в `Event::from_routed_at` (и конвертация `DispatchParseError` в `EventError`).
  * В `rust/crates/discord-core/src/state/reducer.rs`:
    - `EventPayload::MessageDelete(ev)`: удаление сообщения `state.messages.remove(&ev.id)` (идемпотентно, без паники).
    - `EventPayload::MessageDeleteBulk(ev)`: удаление сообщений `state.messages.remove(id)` для всех id из `ev.ids`.
    - `EventPayload::ChannelCreate(ev)` / `ChannelUpdate(ev)`: добавление/обновление канала в `guild.channels` и индекса `channel_guild` при наличии гильдии в `state.guilds`; при отсутствии гильдии запись в `state.diagnostics.inconsistencies`.
    - `EventPayload::ChannelDelete(ev)`: удаление канала из `guild.channels`, индекса `channel_guild`, очистка сообщений удалённого канала (`state.messages`), а также очистка дочерних веток и их сообщений.
    - `EventPayload::ThreadCreate(ev)` / `ThreadUpdate(ev)`: добавление/обновление `Thread` в `state.threads` и индекса `thread_guild`.
    - `EventPayload::ThreadDelete(ev)`: удаление треда из `state.threads`, индекса `thread_guild` и сообщений треда.
  * Написаны 15 контрактных тестов в `rust/crates/discord-core/tests/state_contract.rs`.
- red:
  * Команда: `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test state_contract`
  * Результат до реализации: `exit code 101` (14 ошибок компиляции из-за отсутствия вариантов в `EventPayload`).
  * Лог сохранён: `/home/sanya/orca/project/waves/RW-40/G1/task-157-state-reducer-deletes-channels/red.log`.
- green:
  * Команда: `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-core --test state_contract --test state_reducer --test state_types --test state_selectors --lib`
  * Результат: `exit code 0` (все 86 тестов: 15 state_contract, 29 lib unit tests, 19 state_reducer, 8 state_selectors, 15 state_types).
  * Лог сохранён: `/home/sanya/orca/project/waves/RW-40/G1/task-157-state-reducer-deletes-channels/green.log`.
- targeted: `rust/crates/discord-core/tests/state_contract.rs` — 15 passed, 0 failed, 0 warnings.
- oracle: RFC / Discord Gateway Protocol v9 + `crates/discord-core/src/dispatch.rs` (task-146 / F-GW-03 / F-GW-04), ETF / JSON представления wire events.
- генерация: не применимо.
- мутации: 5/5 KILLED:
  * Мутант 1 (`apply_channel_delete` не удаляет сообщения канала): убит тестом `c05_channel_delete_removes_channel_and_messages` (left 3 != right 1).
  * Мутант 2 (`apply_channel_upsert` не обновляет индекс `channel_guild`): убит тестом `c03_channel_create_and_update_in_existing_guild` (left None != right Some(100)).
  * Мутант 3 (`MessageDeleteBulk` пропускает цикл удаления): убит тестом `c02_message_delete_bulk_removes_multiple_messages` (left 4 != right 2).
  * Мутант 4 (`apply_channel_upsert` при отсутствующей гильдии не пишет диагностику): убит тестом `c04_channel_create_in_unknown_guild_records_inconsistency`.
  * Мутант 5 (`apply_thread_delete` не удаляет тред из `state.threads`): убит тестом `c07_thread_delete_removes_thread_and_messages` (left Some != right None).
- baseline/restore:
  * Проверен baseline до и после каждого мутанта, полное восстановление подтверждено финальным `git status` и `rustfmt --check`.
- стабильность: 10 повторных прогонов тестов без флейков.
- gate:worker: целевые тесты модуля и `clippy -- -D warnings` чистые; полный batch gate выполняет координатор.
- файлы:
  * `76959f14e9b8e52c19f8cd930124ac64  rust/crates/discord-core/src/state/types.rs`
  * `c792da047c48423aa2ab6d4b1096d178  rust/crates/discord-core/src/state/reducer.rs`
  * `0521a09e586c0c0c37a78d6d9278162b  rust/crates/discord-core/tests/state_contract.rs`
- риски/вопросы: отсутствуют, все 8 событий полностью интегрированы в чистый State reducer с сохранением обратной совместимости.
- следующий шаг: передать на двойное ревью PM + Technical Reviewer (DeepSeek).
