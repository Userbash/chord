# REVIEW-57A — task-141 / BE-RE-06 (`BE-P2-06б`), сброс `session_id`/`seq` после 4 неудачных `RESUME` и неограниченный backoff (F-GW-07)

Дата: 2026-10-30. Ревьюер: Технический ревьюер, Review 57A.
Ветка: `feature/task-141-gateway-reconnect-infinite`, worktree
`/home/user/orca/workspaces/project/feature-task-141-gateway-reconnect-infinite`,
HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Режим: сверка с RE-оракулом + чтение диффа + целевые и интеграционные тесты + 5/5 мутационных прогонов.

## Вердикт

**PASS. Ready for Merge.** P0 = 0, P1 = 0. P2 = 1, P3 = 2.

Срез закрывает находку `F-GW-07` (`waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §3.6, `BE-P2-06б` в `waves/RW-40/PM/BACKLOG-RE-BACKEND.md`):
1. После `RESUME_RESET_AFTER_FAILURES = 4` подряд неудачных попыток `RESUME` супервайзер сбрасывает `self.state = None` (`session_id` и `last_seq` обнуляются) и переводит 5-ю попытку на свежий `IDENTIFY` (`op=2` вместо `op=6`), продолжая серию реконнектов без остановки.
2. Успешный `RESUMED` (как и `READY`) в `Supervisor::absorb` вызывает `backoff.reset()` и сбрасывает счётчики неудач (`reconnects = 0`, `resume_failures = 0`, `disconnected_at = None`), в точном соответствии с `this.gatewayBackoff.succeed()` в `_handleDispatch`.
3. Боевой дефолт `SupervisorConfig::new` переведён на `max_reconnects = 0` (неограниченный режим переподключения с экспоненциальным backoff `1_000 -> 2_000 -> 4_000 -> 8_000 -> 16_000 -> 32_000 -> 60_000` мс), который сразу подхватывается `core_supervisor_config` в `rust/gui-spike/src/net.rs:634`. При этом ненулевой `max_reconnects > 0` сохранён для существующих интеграционных тестов (`tests/gateway_supervisor.rs`) и явных продуктовых политик, а фатальные коды закрытия (`4004`, `4013`, `4014`) останавливают супервайзер немедленно, до ожидания backoff.

## Что проверено

- **Файловый скоуп:** в продуктовом коде изменён ровно один разрешённый файл — `rust/crates/discord-core/src/gateway/supervisor.rs` (md5 `ffa734a6b168880f14fe5cc7e601f940`, совпадает с `REPORT.md` §5). Чужие модули (`gateway.rs`, `resume.rs`, `ws.rs`, `tests/gateway_supervisor.rs`) не затронуты. В `git status` висит не связанная с задачей модификация `package-lock.json` (общая для worktree волны, в коммит карточки включать не нужно).
- **Сверка с независимым оракулом:** проверены байтовые смещения в `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (md5 `934b1289b7ebf17bc0fd09569b23b438`):
  - `@5 013 000` (`_handleDispatch`): и `"READY"===t`, и `"RESUMED"===t` выставляют `this.connectionState = v.A.SESSION_ESTABLISHED` и вызывают `this.gatewayBackoff.succeed()`;
  - `@5 014 326..5 015 050` (`_handleClose`): на `4004===t` выставляет `connectionState = CLOSED` и вызывает `this._reset(e, t, n)` без ретрая; на остальных кодах вызывает `let i = this.gatewayBackoff.fail(() => this._connect())` и проверяет `this.gatewayBackoff.fails > 4 && this._reset(e, t, n)`;
  - `@5 015 390` (`_reset`): `this.sessionId = null, this.seq = 0` — сбрасывает идентификатор сессии и seq (не трогая `this.resumeUrl`), поэтому следующая попытка уходит со свежим `IDENTIFY` (`op=2`).
- **Качество тестов и соблюдение TEST-POLICY:**
  - В `RED.log` / `red.log` (md5 `820d1ef49c87d4b93217e189fc2624ca`) зафиксировано честное падение всех 3 новых контрактных тестов на старой реализации (`EXIT=101`, 0 passed, 3 failed).
  - Ни один существующий тест не удалён, не ослаблен и не помечен `#[ignore]`/`.only`. Все 14 интеграционных тестов в `rust/crates/discord-core/tests/gateway_supervisor.rs` проходят без правок.
  - Обработка ошибок: `saturating_add` для счётчиков (`reconnects`, `resume_failures`, `session_resets`), явное сохранение `last_error` и `last_url_error`.

## Прогоны

Все команды запускались последовательно с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target` после принудительной пересборки `supervisor.rs` из исходников текущей ветки.

| Прогон | Команда | Результат | Лог |
|---|---|---|---|
| Targeted unit (`--lib`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib` | **6 passed, 0 failed** (`EXIT=0`) | `REVIEW-57A-GREEN-LIB.log` |
| Интеграционные тесты супервайзера | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --test gateway_supervisor` | **14 passed, 0 failed** (`EXIT=0`) | `REVIEW-57A-SUPERVISOR-IT.log` |
| Clippy (`discord-core`) | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | **0 предупреждений** (`EXIT=0`) | `REVIEW-57A-CLIPPY.log` |
| Форматирование (`rustfmt`) | `(cd rust && cargo fmt --all -- --check)` | **чисто** (`EXIT=0`) | `REVIEW-57A-FMT.log` |

Контрольные суммы артефактов воркера проверены и совпадают с `REPORT.md` §5:
- `rust/crates/discord-core/src/gateway/supervisor.rs` — `ffa734a6b168880f14fe5cc7e601f940`
- `waves/RW-40/G1/task-141-gateway-reconnect-infinite/red.log` — `820d1ef49c87d4b93217e189fc2624ca`
- `waves/RW-40/G1/task-141-gateway-reconnect-infinite/green-lib.log` — `cfc60e8a33ff494ec8e0b3bea36a8c09`
- `waves/RW-40/G1/task-141-gateway-reconnect-infinite/green.log` — `9fe78a426233cfa8774ac15bacd21e48`
- `waves/RW-40/G1/task-141-gateway-reconnect-infinite/CLIPPY.log` — `ac53ad2cea90c21f47de3ff65ec149b4`
- `waves/RW-40/G1/task-141-gateway-reconnect-infinite/FMT.log` — `fdbdc3d0c7b403251ac5407380220523`

## Мутационные проверки (5/5 убиты)

Подробная сводка — в `REVIEW-57A-MUTATION-SUMMARY.md`, полные выводы — в `REVIEW-57A-MUTATION-M1.log` … `REVIEW-57A-MUTATION-M5.log`. После завершения серии `supervisor.rs` восстановлен из резервной копии (md5 `ffa734a6b168880f14fe5cc7e601f940`), тестовые бинари пересобраны начисто.

| # | Мутация | Место | Результат | Чем поймана |
|---|---|---|---|---|
| M1 | Порог попыток `RESUME`: `RESUME_RESET_AFTER_FAILURES: 4 -> 3` | `supervisor.rs:42` | **KILLED** (`EXIT=101`, 2 failed) | `resets_session_after_four_failed_resumes_and_switches_to_fresh_identify`, `successful_resumed_resets_failure_streak_and_backoff` |
| M2 | Отключение сброса `session_id`/`seq` (убран `self.state = None` в `note_attempt`) | `supervisor.rs:403` | **KILLED** (`EXIT=101`, 2 failed) | `resets_session_after_four_failed_resumes_and_switches_to_fresh_identify`, `successful_resumed_resets_failure_streak_and_backoff` |
| M3 | Потолок backoff 60s изменён: `Backoff::default()` `60_000 -> 30_000` | `supervisor.rs:87` | **KILLED** (`EXIT=101`, 1 failed) | `unlimited_reconnect_mode_holds_60s_ceiling_and_stops_immediately_on_4004` |
| M4 | Пропуск фатального стопа на `4004`: `Some(4004) => CloseAction::Reconnect` в `classify_close` | `supervisor.rs:244` | **KILLED** (`EXIT=101`, 3 failed) | все 3 теста `gateway::supervisor::tests::*` |
| M5 | Нарушение совместимости `max_reconnects`: возврат дефолта `8` в `SupervisorConfig::new` и отключение ветки `max_reconnects != 0` в `run_until` | `supervisor.rs:151,362` | **KILLED** (`EXIT_LIB=101`, `EXIT_IT=101`) | `unlimited_reconnect_mode_holds_60s_ceiling_and_stops_immediately_on_4004` (`--lib`) + `supervisor_reconnect_limit_stops` (`--test gateway_supervisor`) |

## Findings

**P0:** нет.

**P1:** нет.

**P2-1. `disconnected_at` перезаписывается на каждой неудачной попытке (`supervisor.rs:370`), что занижает `downtime_ms` между серией попыток (связь с `F-GW-08` / `BE-P2-06в`).**
В `run_until` строка `self.disconnected_at = Some(Instant::now())` выполняется перед каждым `sleep(delay)`. Из-за этого `next_action()` (`supervisor.rs:410-415`) передаёт в `plan_reconnect` не суммарное время отсутствия сессии с момента разрыва (или последнего `HEARTBEAT_ACK`, как требует `F-GW-08`: `Date.now() - lastHeartbeatAckTime <= 3 min`), а лишь длительность последнего `sleep(delay)`. При боевом backoff первые 4 неудачных `RESUME` занимают `1 + 2 + 4 + 8 = 15 с` и принудительно сбрасывают сессию через `note_attempt`, поэтому для `F-GW-07` это поведение корректно. Однако при реализации соседней карточки `BE-P2-06в` (`F-GW-08`, окно `RESUME` 3 минуты от последнего ACK) момент потери связи/последнего ACK нельзя перезаписывать на каждом неудачном реконнекте.

**P3-1. Двойной линейный проход по `pipe.events()` для поиска `DispatchKind::Resumed` (`supervisor.rs:331-334` и `427-430`).**
Проверка `pipe.events().iter().any(|event| event.kind == Some(DispatchKind::Resumed))` выполняется сначала внутри `self.absorb(&pipe)`, а затем сразу после него в `run_until` для вычисления `established`. Если `absorb` будет возвращать `bool` (`established`), повторный проход по срезу событий исчезнет. Кроме того, если в будущем вызывающий код начнёт дренировать `GatewayPipe::take_events()` на длинных сессиях (как рекомендует докстринг `ws.rs:375`), факт получения `RESUMED` лучше фиксировать отдельным sticky-полем в `GatewayPipe` по аналогии с `last_ready`.

**P3-2. Побочное изменение `package-lock.json` в рабочем дереве.**
В `git status` присутствует незакоммиченное изменение `package-lock.json`, не относящееся к `task-141`. При коммите/интеграции ветки `feature/task-141-gateway-reconnect-infinite` нужно включать только `rust/crates/discord-core/src/gateway/supervisor.rs`.

## Границы доказательств

Проверены модульные и интеграционные тесты супервайзера шлюза (`discord-core --lib` и `--test gateway_supervisor`) на локальном WebSocket-стабе `127.0.0.1` с реальным ETF+zstd-stream кодированием, `clippy -D warnings`, `cargo fmt --check`, а также 5 целевых мутаций в `supervisor.rs`. Живой Discord и соседние части `BE-P2-06` — случайный jitter первого heartbeat (`F-GW-06`, `gateway.rs`) и 3-минутное окно `RESUME` от последнего ACK (`F-GW-08`, `resume.rs`) — не входили в файловый скоуп `task-141` и проверяются в своих карточках.
