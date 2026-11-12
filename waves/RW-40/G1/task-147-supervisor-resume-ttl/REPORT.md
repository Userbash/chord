# Отчёт воркера — task-147 (F-GW-08 Supervisor 3-min Resume TTL & OP 3/4/8/37)

## 1. Batch / карточка

- **Волна:** `RW-40`, отдел `G1` (Backend).
- **Карточка:** `task-147` — находка `F-GW-08` (`waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §3.7, `BE-P2-06в`) + проброс исходящих клиентских опкодов `OP 3` / `OP 4` / `OP 8` / `OP 37` (`F-GW-02` / `BE-P1-01`) в `GatewaySupervisor`.
- **Файл воркера:** `rust/crates/discord-core/src/gateway/supervisor.rs` (+ предварительная синхронизация из `/home/user/orca/project` после PR-23/PR-24/PR-28).
- **Ветка / worktree:** `feature/task-147-supervisor-resume-ttl-and-opcodes`, HEAD `4e24048`.
- **Независимый оракул:** веб-бандл Discord 1.0.162 `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (md5 `934b1289b7ebf17bc0fd09569b23b438`):
  - `_doResumeOrIdentify` (@5 019 509): `null !== this.sessionId && (null == this.lastHeartbeatAckTime || e - this.lastHeartbeatAckTime <= tI) ? this._doResume() : this._doIdentify()`, где `tI = 3 * L.A.Millis.MINUTE` (`180 000` мс);
  - `_doIdentify` (@5 018 140): `this.seq = 0, this.sessionId = null` — при истечении 3-минутного окна с последнего `HEARTBEAT_ACK` (op11) сессия сразу сбрасывается и отправляется свежий `IDENTIFY` (op2);
  - таблица опкодов `e2` (@4 996 243) и методы `GatewaySocket` (`presenceUpdate`, `voiceStateUpdate`, `requestGuildMembers`, `updateGuildSubscriptions` @4 998 360…4 998 666; проверка `this.isSessionEstablished()` в `send` @5 024 800).

## 2. Что сделано

В `rust/crates/discord-core/src/gateway/supervisor.rs`:

1. **Окно `RESUME` 180 000 мс (3 минуты) от последнего `HEARTBEAT_ACK` / успешного события (`F-GW-08`):**
   - Добавлены константы `pub const RESUME_TTL_MS: u64 = 180_000;` и `pub const RESUME_MAX_ACK_ELAPSED_MS: u64 = RESUME_TTL_MS;`.
   - В `Supervisor` (`pub type GatewaySupervisor = Supervisor`) добавлен учёт времени последнего `HEARTBEAT_ACK` (op11) / успешного события сессии (`last_ack_at: Option<Instant>`, `ack_elapsed_override_ms: Option<u64>`, методы `note_heartbeat_ack`, `set_last_ack_elapsed_ms`, `last_ack_elapsed_ms`, `plan_next_reconnect`).
   - При переподключении (`plan_next_reconnect`):
     - если `elapsed <= 180_000` мс и сервер не запретил резюмирование (`op9 d=false`), сохраняются `session_id` и `last_seq` и выбирается `ReconnectAction::Resume` (op6);
     - если `elapsed > 180_000` мс, супервайзер немедленно вызывает `reset_session()` (`session_id`/`seq` сбрасываются в `None`, `session_ready = false`, `session_resets += 1`) и выбирает `ReconnectAction::Identify` (op2).
2. **Формирование и постановка в очередь исходящих клиентских кадров `OP 3` / `OP 4` / `OP 8` / `OP 37` в `GatewaySupervisor`:**
   - Ре-экспортированы константы, структуры и кодировщики из `crate::gateway` и объявлены публичные алиасы:
     - `GatewaySupervisor = Supervisor`;
     - `PresenceUpdate = OutboundPresenceUpdate` (`OP 3`);
     - `VoiceStateUpdateOut = OutboundVoiceStateUpdate` (`OP 4`);
     - `RequestGuildMembers = OutboundGuildMembersRequest` (`OP 8`);
     - `GuildSubscriptionsBulk = BTreeMap<String, OutboundGuildSubscription>` (`OP 37`).
   - Добавлен тип ошибки `SupervisorOutboundError` (`NotReady { op }` / `Encode(OutboundError)`).
   - Реализована проверка готовности сессии `is_session_ready(&self) -> bool` (сессия установила `READY`/`RESUMED`, не инвалидирована `op9 d=false` и не истекла по `RESUME_TTL_MS`) и методы кодирования/постановки в очередь:
     - `send_presence_update` / `queue_presence_update` (`OP 3`);
     - `send_voice_state_update` / `queue_voice_state_update` (`OP 4`);
     - `send_request_guild_members` / `queue_request_guild_members` и `send_request_guild_members_chunked` (`OP 8`, чанкование `user_ids` по `GUILD_MEMBERS_CHUNK_SIZE = 100`);
     - `send_guild_subscriptions_bulk` / `queue_guild_subscriptions_bulk` (`OP 37`);
     - методы управления очередью и диагностики: `pending_outbound_frames`, `outbound_frames`, `pop_outbound_frame`, `take_outbound_frames`, `rejected_outbound`.

## 3. RED-прогон

До реализации продуктовой логики в `supervisor.rs` добавлены 2 контрактных теста (`resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s` и `outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session`):

```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests
```

- Результат: `EXIT=101` (18 ошибок компиляции `E0422`/`E0425`/`E0433`: отсутствуют `RESUME_TTL_MS`, `GatewaySupervisor`, `PresenceUpdate`, `VoiceStateUpdateOut`, `RequestGuildMembers`, `GuildSubscriptionsBulk`, `SupervisorOutboundError`, `set_last_ack_elapsed_ms`, `send_presence_update`, `send_voice_state_update`, `send_request_guild_members`, `send_guild_subscriptions_bulk`).
- Лог: `waves/RW-40/G1/task-147-supervisor-resume-ttl/RED.log` (`red.log`).

## 4. GREEN-прогон и проверки

| Проверка | Команда | Результат | Лог |
|---|---|---|---|
| Targeted unit (`supervisor::tests`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests` | **5 passed, 0 failed** (`EXIT=0`) | `GREEN-LIB.log` |
| Все тесты `discord-core` | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` | **все наборы зелёные, 0 failed** (`EXIT=0`) | `GREEN.log` / `green.log` |
| Clippy (`discord-core`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | **0 предупреждений** (`EXIT=0`) | `CLIPPY.log` |
| Форматирование | `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` | **чисто** (`EXIT=0`) | `FMT.log` |

## 5. Файлы и md5

| Файл | md5 | Примечание |
|---|---|---|
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5af33f1994907ad686cbff2ade5df8e3` | Целевой файл задачи |
| `rust/crates/discord-core/src/gateway.rs` | `958a3708bc00f4caf6126686e7d91f4f` | Синхронизирован из `/home/user/orca/project` (PR-24) |
| `rust/crates/discord-codec/src/zstd.rs` | `6563379e4a71ec6875ad1b678d31975b` | Синхронизирован из `/home/user/orca/project` (PR-23) |
| `rust/crates/discord-codec/Cargo.toml` | `0708cbb594c521dd30e4b6870159d238` | Синхронизирован из `/home/user/orca/project` (PR-23) |
| `rust/Cargo.lock` | `6ac855a6516669d44d67ec5b5b7da334` | Синхронизирован из `/home/user/orca/project` (PR-23) |
| `waves/RW-40/G1/task-147-supervisor-resume-ttl/RED.log` | `bf7e0ab21376f63187a68e57a329688d` | Лог RED-прогона |
| `waves/RW-40/G1/task-147-supervisor-resume-ttl/GREEN-LIB.log` | `080368d770aa579a4199f1537006100f` | Лог targeted GREEN-прогона |
| `waves/RW-40/G1/task-147-supervisor-resume-ttl/GREEN.log` | `9eeef329e02a97a09e9043404681c513` | Полный лог тестов `discord-core` |
| `waves/RW-40/G1/task-147-supervisor-resume-ttl/CLIPPY.log` | `6018217d2f76d1a3e7ab099bff60ff2a` | Лог `cargo clippy` |
| `waves/RW-40/G1/task-147-supervisor-resume-ttl/FMT.log` | `d41d8cd98f00b204e9800998ecf8427e` | Лог `cargo fmt --check` |

## 6. Риски и совместимость

- Все 14 существующих интеграционных тестов в `rust/crates/discord-core/tests/gateway_supervisor.rs` и 3 теста `F-GW-07` в `supervisor::tests` проходят без изменений.
- Константа `RESUME_MAX_DOWNTIME_MS = 30_000` в `rust/crates/discord-core/src/resume.rs` не менялась (вне файлового скоупа задачи); `Supervisor` делегирует в `plan_reconnect` после собственной проверки 3-минутного окна `RESUME_TTL_MS = 180_000` от последнего `HEARTBEAT_ACK`.
