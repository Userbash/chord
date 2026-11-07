# task-142 (BE-RE-03 / F-GW-02): подключение кодировщиков исходящих опкодов 3/4/8/37 к живому транспорту `ws.rs`

- **batch/карточка:** RW-40 / G1, `task-142` (продолжение BE-RE-03 / F-GW-02, карточка бэклога BE-P1-01 в `waves/RW-40/PM/BACKLOG-RE-BACKEND.md:35`).
- **worktree / ветка:** `/home/sanya/orca/workspaces/project/feature-task-142-ws-outbound-wiring`, ветка `feature/task-142-ws-outbound-wiring`, HEAD `4e24048`.
- **write-scope:** `rust/crates/discord-core/src/ws.rs` (+ предварительная синхронизация файлов PR-23 `task-137` и PR-24 `task-139` из `/home/sanya/orca/project`).

## Сделано

1. **Синхронизация с PR-23 (`task-137`) и PR-24 (`task-139`):**
   - В рабочее дерево скопированы `rust/crates/discord-codec/src/zstd.rs`, `rust/crates/discord-codec/Cargo.toml`, `rust/Cargo.lock`, `rust/crates/discord-core/src/gateway.rs` (а также обновлённые в PR-23 интеграционные тесты `rust/crates/discord-codec/tests/zstd_stream.rs`, `rust/crates/discord-core/tests/dispatch_router.rs`, `rust/crates/discord-core/tests/ws_transport.rs`, чтобы полный прогон `cargo test -p discord-core` проверял актуальный безбуферный поток `libzstd` на 824 816 Б / 137 термов).
2. **Обновление документации `ws.rs` (строки 51–57, закрытие F-01 из Review 53A):**
   - Устаревший раздел «Известное ограничение (RW-02: удержание окна ruzstd)» (про 759 280 Б, 44 терма и `s = 36`) заменён на описание потоковой распаковки `libzstd` (`ZSTD_decompressStream`, F-GW-01 / `task-137`): все 824 816 Б и 137 полных термов (`s = 1..113` для 113 диспатчей `op0`, 23 `op11` ACK и 1 `op10` HELLO) отдаются пайпу сразу без удержания 64 КиБ окна.
3. **Подключение исходящих опкодов OP 3 / OP 4 / OP 8 / OP 37 к транспорту `ws.rs`:**
   - Ре-экспортированы типы и кодировщики из `crate::gateway` и объявлены публичные алиасы типов:
     - `PresenceUpdate = OutboundPresenceUpdate` (OP 3 `PRESENCE_UPDATE`);
     - `VoiceStateUpdateOut = OutboundVoiceStateUpdate` (OP 4 `VOICE_STATE_UPDATE`);
     - `RequestGuildMembers = OutboundGuildMembersRequest` (OP 8 `REQUEST_GUILD_MEMBERS`);
     - `GuildSubscriptionsBulk = BTreeMap<String, OutboundGuildSubscription>` (OP 37 `GUILD_SUBSCRIPTIONS_BULK`).
   - В `TransportError` добавлен вариант `Outbound(OutboundError)` с реализацией `Display` и `Error::source` (без паник и без глотания ошибок кодирования/записи).
   - Добавлена функция `encode_request_guild_members_frames(request)`:
     - если `request.user_ids` непуст — режет список через `chunk_user_ids` по `GUILD_MEMBERS_CHUNK_SIZE` (100) и кодирует отдельный бинарный ETF-кадр OP 8 на каждый чанк;
     - если `request.user_ids` пуст и задан `request.query` — кодирует один кадр OP 8;
     - если пусты и `user_ids`, и `query` — возвращает пустой вектор (`0` кадров).
   - Реализованы асинхронные методы отправки бинарных ETF-кадров (`Message::binary`) на `GatewayPipe` (с проверкой `ensure_alive()` на живость сессии и учётом счётчиков `frames_sent()` / `bytes_sent()`) и одноимённые свободные функции модуля `ws`:
     - `send_presence_update` (OP 3);
     - `send_voice_state_update` (OP 4);
     - `send_request_guild_members` (OP 8, чанкование по 100 `user_ids`, возвращает число отправленных кадров);
     - `send_guild_subscriptions_bulk` (OP 37).
   - В `GatewayPipe` и цикл `connect_and_run_resume` добавлена очередь исходящих кадров (`queue_presence_update`, `queue_voice_state_update`, `queue_request_guild_members`, `queue_guild_subscriptions_bulk`, `pending_outbound_frames()`), которая дренируется функцией `maybe_flush_outbound` строго после получения `HELLO` (и отправки `IDENTIFY`/`RESUME`, если они сконфигурированы) — ни один клиентский кадр не уходит до завершения рукопожатия.

## Проверки (Evidence)

- **RED-прогон:**
  ```
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib
  → exit 101 (E0432 / E0599: отсутствуют типы и методы send_presence_update, send_voice_state_update, send_request_guild_members, send_guild_subscriptions_bulk, queue_*, frames_sent, bytes_sent)
  ```
  Лог: `waves/RW-40/G1/task-142-ws-outbound-wiring/RED.log`.

- **GREEN-прогон (targeted `--lib`):**
  ```
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib
  → exit 0 (14 passed; 0 failed — включая 5 новых тестов в ws::tests)
  ```
  Лог: `waves/RW-40/G1/task-142-ws-outbound-wiring/GREEN-LIB.log`.

- **GREEN-прогон (весь крейт `discord-core`):**
  ```
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core
  → exit 0 (588 passed; 0 failed; 0 ignored)
  ```
  Лог: `waves/RW-40/G1/task-142-ws-outbound-wiring/GREEN.log`.

- **Clippy и форматирование:**
  ```
  CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings
  → exit 0
  cargo fmt --manifest-path rust/Cargo.toml --all -- --check
  → exit 0
  ```
  Логи: `waves/RW-40/G1/task-142-ws-outbound-wiring/CLIPPY.log`, `waves/RW-40/G1/task-142-ws-outbound-wiring/FMT.log`.

## Новые тесты в `ws::tests`

1. `send_presence_update_sends_binary_etf_op3_frame` — проверяет отправку бинарного WS-кадра OP 3 через `GatewayPipe::send_presence_update` и `ws::send_presence_update`, сверяет первый кадр побайтно с независимым hex-оракулом wetf-раскладки (`837400000002...`), второй кадр — со статусом `dnd`, `since`, custom activity и `afk: true`, проверяет счётчики `frames_sent` и `bytes_sent`.
2. `send_voice_state_update_sends_binary_etf_op4_join_and_leave` — проверяет отправку бинарных кадров OP 4 для входа в голосовой канал гильдии и выхода (`guild_id: None`, `channel_id: None` → `EtfValue::Nil`), порядок ключей и атомы булевых флагов.
3. `send_request_guild_members_chunks_user_ids_by_100_and_supports_query` — проверяет OP 8 на граничных размерах списка `user_ids`: `0` (без `query` → 0 кадров), `100` (1 кадр), `101` (2 кадра: 100 + 1 с сохранением `guild_id`, `presences`, `nonce`), `205` (3 кадра: 100 + 100 + 5), а также поисковый путь (`query: Some("monolit")`, `limit: Some(0)`, `user_ids: []` → 1 кадр).
4. `send_guild_subscriptions_bulk_sends_binary_etf_op37_frame` — проверяет отправку бинарного кадра OP 37 с картой подписок двух гильдий и сверяет десериализованный ETF-терм со схемой `SubscriptionManager.get`.
5. `outbound_guards_dead_session_and_flushes_queued_frames_after_hello` — проверяет отказ отправки на мёртвой сессии (`op7`), проброс ошибки записи при закрытом сокете (`TransportError::Ws`) и автоматическую отправку очереди кадров (`op3`, `op4`, `2×op8`, `op37`) в `connect_and_run` строго после `HELLO` (0 кадров до `HELLO`).

## Файлы и контрольные суммы (md5)

- `rust/crates/discord-core/src/ws.rs` — `9976b056a9ff18559867c1ad110a290f` (основной файл задачи)
- Синхронизированные файлы PR-23 / PR-24 из `/home/sanya/orca/project`:
  - `rust/crates/discord-core/src/gateway.rs` — `958a3708bc00f4caf6126686e7d91f4f`
  - `rust/crates/discord-codec/src/zstd.rs` — `6563379e4a71ec6875ad1b678d31975b`
  - `rust/crates/discord-codec/Cargo.toml` — `0708cbb594c521dd30e4b6870159d238`
  - `rust/crates/discord-codec/tests/zstd_stream.rs` — `bb8799618a270753621c0bdd0bdd4508`
  - `rust/crates/discord-core/tests/dispatch_router.rs` — `8236b56cd3ba488f82226ed205b3b395`
  - `rust/crates/discord-core/tests/ws_transport.rs` — `9fd81c876440fd6c2b60a1df7e37d44c`
  - `rust/Cargo.lock` — `6ac855a6516669d44d67ec5b5b7da334`

## Риски и следующие шаги

1. Ре-экспорт новых функций и типов `ws::*` в корневой `rust/crates/discord-core/src/lib.rs` не выполнялся (`lib.rs` — общий файл под управлением Team Lead); все символы доступны публично через `discord_core::ws::*`.
2. Чанкование больших карт подписок OP 37 по порогу 15 360 Б и проброс команд из `supervisor.rs` / Tauri IPC относятся к следующим карточкам `BE-P1-01`.
