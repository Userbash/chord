# REVIEW-62A — task-147 (`BE-P2-06в` / `F-GW-08`): 3-минутное окно `RESUME` от последнего `HEARTBEAT_ACK` и исходящие хелперы `OP 3` / `OP 4` / `OP 8` / `OP 37` в `GatewaySupervisor`

Дата: 2026-10-30. Ревьюер: Технический ревьюер, Review 62A.
Ветка: `feature/task-147-supervisor-resume-ttl-and-opcodes`, worktree
`/home/user/orca/workspaces/project/feature-task-147-supervisor-resume-ttl-and-opcodes`,
HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Режим: сверка с RE-оракулом + чтение диффа + целевые и интеграционные тесты + 5/5 мутационных прогонов.

## Вердикт

**PASS. Ready for Merge.** P0 = 0, P1 = 0. P2 = 1, P3 = 2.

Срез закрывает находку `F-GW-08` (`waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §3.7, `BE-P2-06в` в `waves/RW-40/PM/BACKLOG-RE-BACKEND.md`) и пробрасывает клиентские опкоды `OP 3` / `OP 4` / `OP 8` / `OP 37` (`F-GW-02` / `BE-P1-01`) в публичный интерфейс `GatewaySupervisor` (`rust/crates/discord-core/src/gateway/supervisor.rs`):
1. Введена константа `RESUME_TTL_MS: u64 = 180_000` (и синоним `RESUME_MAX_ACK_ELAPSED_MS = RESUME_TTL_MS`), задающая 3-минутное окно жизни сессии для `RESUME` с момента последнего `HEARTBEAT_ACK` (`op11`) / успешного события сессии.
2. В `Supervisor::plan_next_reconnect` при `elapsed <= 180_000` мс (и отсутствии запрета `op9 d=false`) сохраняются `session_id` и `last_seq` и возвращается `ReconnectAction::Resume` (`op=6`). При `elapsed > 180_000` мс супервайзер немедленно вызывает `reset_session()` (`state = None`, `session_ready = false`, `invalid_session = None`, `resume_failures = 0`, очистка `outbound_queue`, `session_resets += 1`) и переходит на свежий `IDENTIFY` (`op=2`).
3. Полностью сохранён контракт `F-GW-07` (`RESUME_RESET_AFTER_FAILURES = 4`, неограниченный реконнект `max_reconnects = 0` с потолком 60 с и немедленной остановкой на `4004`/`4013`/`4014`). При этом исправлено замечание `P2-1` из `REVIEW-57A.md`: неудачные попытки переподключения больше не перезаписывают таймштамп сессии перед каждым `sleep(delay)` — `last_ack_at` обновляется в `absorb` только при реальном `READY`, `RESUMED`, `HEARTBEAT_ACK` или продвижении `seq`.
4. В `GatewaySupervisor` добавлены проверка готовности сессии `is_session_ready()`, ошибка `SupervisorOutboundError` (`NotReady { op }` / `Encode(OutboundError)`) и методы формирования/постановки в очередь клиентских кадров `OP 3` (`send_presence_update`), `OP 4` (`send_voice_state_update`), `OP 8` (`send_request_guild_members`, `send_request_guild_members_chunked` с разбивкой `user_ids` по `GUILD_MEMBERS_CHUNK_SIZE = 100`) и `OP 37` (`send_guild_subscriptions_bulk`).

## Что проверено

- **Файловый скоуп:** относительно `/home/user/orca/project` изменён ровно один целевой файл карточки — `rust/crates/discord-core/src/gateway/supervisor.rs` (md5 `5af33f1994907ad686cbff2ade5df8e3`, совпадает с `REPORT.md` §5). Остальные модифицированные файлы в `git status` (`gateway.rs`, `zstd.rs`, `discord-codec/Cargo.toml`, `rust/Cargo.lock`, `zstd_stream.rs`, `dispatch_router.rs`, `ws_transport.rs`) побайтно идентичны уже влитым в `/home/user/orca/project` PR-23/PR-24/PR-28.
- **Сверка с независимым оракулом:** проверены байтовые смещения в веб-бандле Discord 1.0.162 `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (md5 `934b1289b7ebf17bc0fd09569b23b438`):
  - `@5 019 509` (`_doResumeOrIdentify`): `let e = Date.now(); null !== this.sessionId && (null == this.lastHeartbeatAckTime || e - this.lastHeartbeatAckTime <= tI) ? this._doResume() : this._doIdentify()`, где `tI = 3 * L.A.Millis.MINUTE` (`180 000` мс);
  - `@5 018 140` (`_doIdentify`): `this.seq = 0, this.sessionId = null` при переходе к свежему `IDENTIFY`;
  - `@4 996 243` (таблица `e2`) и `@4 998 360..4 999 000` (`class e3`): `PRESENCE_UPDATE = 3`, `VOICE_STATE_UPDATE = 4`, `REQUEST_GUILD_MEMBERS = 8` (чанкование `userIds` по 100 на `@4 954 130`), `GUILD_SUBSCRIPTIONS_BULK = 37`, а также гейт `isSessionEstablished()` перед отправкой.
- **Качество тестов и соблюдение TEST-POLICY:**
  - В `RED.log` / `red.log` (md5 `bf7e0ab21376f63187a68e57a329688d`) зафиксировано падение новых контрактных тестов до реализации (`EXIT=101`, 18 ошибок компиляции).
  - Граничные значения `180_000` мс (`<= RESUME_TTL_MS` → `op=6` с прежними `session_id` и `seq=7`) и `180_001` мс (`> RESUME_TTL_MS` → сброс `state = None` и `op=2`) проверены на реальном локальном WebSocket+ETF+zstd стабе, а кадр `OP 3` сверен побайтно с независимым wetf-оракулом (`8374000002...`).
  - Существующие 3 unit-теста `F-GW-07` и 14 интеграционных тестов `tests/gateway_supervisor.rs` не ослаблены и проходят без изменений.

## Прогоны

Все команды запускались последовательно с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:

| Прогон | Команда | Результат | Лог |
|---|---|---|---|
| Targeted unit (`gateway::supervisor`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor` | **5 passed, 0 failed** (`EXIT=0`) | `REVIEW-62A-GREEN-LIB.log` |
| Интеграционные тесты супервайзера | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --test gateway_supervisor` | **14 passed, 0 failed** (`EXIT=0`) | `REVIEW-62A-SUPERVISOR-IT.log` |
| Clippy (`discord-core --all-targets`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | **0 предупреждений** (`EXIT=0`) | `REVIEW-62A-CLIPPY.log` |
| Форматирование (`rustfmt`) | `cargo fmt --manifest-path rust/Cargo.toml -p discord-core -- --check` | **чисто** (`EXIT=0`) | `REVIEW-62A-FMT.log` |

Контрольные суммы артефактов воркера проверены и совпадают с `REPORT.md` §5:
- `rust/crates/discord-core/src/gateway/supervisor.rs` — `5af33f1994907ad686cbff2ade5df8e3`
- `waves/RW-40/G1/task-147-supervisor-resume-ttl/RED.log` — `bf7e0ab21376f63187a68e57a329688d`
- `waves/RW-40/G1/task-147-supervisor-resume-ttl/GREEN-LIB.log` — `080368d770aa579a4199f1537006100f`
- `waves/RW-40/G1/task-147-supervisor-resume-ttl/GREEN.log` — `9eeef329e02a97a09e9043404681c513`
- `waves/RW-40/G1/task-147-supervisor-resume-ttl/CLIPPY.log` — `6018217d2f76d1a3e7ab099bff60ff2a`
- `waves/RW-40/G1/task-147-supervisor-resume-ttl/FMT.log` — `d41d8cd98f00b204e9800998ecf8427e`

## Мутационные проверки (5/5 убиты)

Подробная сводка — в `REVIEW-62A-MUTATION-SUMMARY.md`, полные выводы — в `REVIEW-62A-MUTATION-M1.log` … `REVIEW-62A-MUTATION-M5.log`. После серии мутаций `supervisor.rs` восстановлен побайтно (md5 `5af33f1994907ad686cbff2ade5df8e3`), тестовый бинарь пересобран начисто.

| # | Мутация | Место | Результат | Чем поймана |
|---|---|---|---|---|
| M1 | Сужение окна `RESUME` до старых 30 с (`RESUME_TTL_MS: 180_000 -> 30_000`) | `supervisor.rs:80` | **KILLED** (`EXIT=101`, 1 failed) | `resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s` |
| M2 | Отключение проверки `elapsed_ms > RESUME_TTL_MS` и `reset_session()` в `plan_next_reconnect` | `supervisor.rs:522-528` | **KILLED** (`EXIT=101`, 2 failed) | `resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s`, `outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session` |
| M3 | Сдвиг порога 4 попыток `RESUME` из `F-GW-07` (`RESUME_RESET_AFTER_FAILURES: 4 -> 3`) | `supervisor.rs:71` | **KILLED** (`EXIT=101`, 2 failed) | `resets_session_after_four_failed_resumes_and_switches_to_fresh_identify`, `successful_resumed_resets_failure_streak_and_backoff` |
| M4 | Отключение гейта готовности сессии `ensure_session_ready` для `OP 3/4/8/37` | `supervisor.rs:642-648` | **KILLED** (`EXIT=101`, 1 failed) | `outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session` |
| M5 | Отключение чанкования `user_ids` по 100 в `send_request_guild_members_chunked` (`OP 8`) | `supervisor.rs:722` | **KILLED** (`EXIT=101`, 1 failed) | `outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session` |

## Findings

**P0:** нет.

**P1:** нет.

**P2-1. Локальная очередь `Supervisor::outbound_queue` не связана напрямую с `OutboundChannel` в `ws::connect_and_run_resume` (`supervisor.rs:346, 412-420`).**
Методы `send_presence_update` / `send_voice_state_update` / `send_request_guild_members` / `send_guild_subscriptions_bulk` валидируют готовность сессии, кодируют ETF-кадры и складывают их в `self.outbound_queue`, предоставляя `pop_outbound_frame` и `take_outbound_frames`. Внутри `Supervisor::run_until` (`supervisor.rs:412-420`) `RunOptions` пока передаётся без `outbound`-канала из параллельной карточки `task-142` (`ws.rs`), поскольку `ws.rs` не входил в файловый скоуп `task-147`. На интеграционном шаге волны `Supervisor` стоит связать с `GatewayOutboundSender` / `RunOptions::with_outbound`, чтобы накопленные кадры автоматически дренировались в живой сокет.

**P3-1. `ack_elapsed_override_ms` не сбрасывается в `absorb` и `reset_session` (`supervisor.rs:480-487, 592-597`).**
Метод `note_heartbeat_ack()` очищает `self.ack_elapsed_override_ms = None`, однако `absorb()` при обновлении `self.last_ack_at = Some(Instant::now())` и `reset_session()` оставляют `ack_elapsed_override_ms` нетронутым. Это сделано намеренно, чтобы в тесте `resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s` можно было задать `set_last_ack_elapsed_ms(...)` один раз перед `supervisor.run()`, но в многошаговых сценариях после `reset_session()` переопределение останется активным, пока вызывающий явно не сбросит его через `note_heartbeat_ack()`.

**P3-2. Синхронизированные файлы из `/home/user/orca/project` и `package-lock.json` в `git status`.**
В рабочем дереве ветки присутствуют синхронизированные из основной ветки файлы PR-23/PR-24/PR-28 и фоновое изменение `package-lock.json`. При формировании коммита карточки `task-147` нужно включать только `rust/crates/discord-core/src/gateway/supervisor.rs`.

## Границы доказательств

Проверены модульные и интеграционные тесты супервайзера (`discord-core --lib gateway::supervisor` и `--test gateway_supervisor`) на локальном WebSocket-стабе `127.0.0.1` с реальным ETF+zstd-stream кодированием, `clippy --all-targets -- -D warnings`, `cargo fmt --check`, а также 5 независимых мутаций в `supervisor.rs`. Константа `RESUME_MAX_DOWNTIME_MS = 30_000` в `rust/crates/discord-core/src/resume.rs` и прямой проброс `OutboundChannel` в `rust/crates/discord-core/src/ws.rs` находились вне файлового скоупа `task-147`.
