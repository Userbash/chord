# PM Acceptance — task-156 (`BE-P1-01` / `F-GW-05`: OP 37 15,360 B Chunking & 30 Forwarded Dispatches / BFF Gateway Helpers)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/crates/discord-core/src/gateway.rs`
  - `rust/gui-spike/src/net.rs`
  - `rust/gui-spike/tests/net_contract.rs`
- **Соответствие ТЗ и оракулу:**
  1. Константа `MAX_GUILD_SUBSCRIPTIONS_FRAME_BYTES = 15_360` (14.5 KiB) и чанкованный энкодер `encode_guild_subscriptions_bulk_chunked` реализованы в `gateway.rs`.
  2. Список `FORWARDED_DISPATCHES` в `rust/gui-spike/src/net.rs` расширен до 30 событий (добавлены 8 событий `MESSAGE_DELETE*`, `CHANNEL_*`, `THREAD_*` из PR-32), добавлены типизированные структуры и валидаторы BFF-команд `OP 3/4/8/37`.
  3. Все 12 unit-тестов `discord-core::gateway` и 32 контрактных теста `net_contract` проходят (`0 fail`).
- **Следующий шаг:** Техническое ревью (`Review 71A`) с проверкой `5/5` мутаций.
