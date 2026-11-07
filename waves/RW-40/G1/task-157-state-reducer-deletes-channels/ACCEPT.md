# PM Acceptance — task-157 (`BE-P1-02` / `F-GW-06`: State Reducer & Types for `MESSAGE_DELETE*`, `CHANNEL_*`, `THREAD_*`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/crates/discord-core/src/state/types.rs`
  - `rust/crates/discord-core/src/state/reducer.rs`
  - `rust/crates/discord-core/tests/state_contract.rs`
- **Соответствие ТЗ и оракулу:**
  1. Варианты `MessageDelete`, `MessageDeleteBulk`, `ChannelCreate/Update/Delete`, `ThreadCreate/Update/Delete` добавлены в `EventPayload` и подключены к `dispatch.parse_typed()`.
  2. Чистый reducer `apply` обновляет `state.messages`, `guild.channels` (`channel_guild`), `state.threads` (`thread_guild`) и фиксирует несоответствия в `state.diagnostics.inconsistencies`.
  3. 86/86 тестов (`state_contract`, `state_reducer`, `state_types`, `state_selectors`, `lib`) проходят (`0 fail`).
- **Следующий шаг:** Техническое ревью (`Review 74A`) с проверкой `5/5` мутаций.
