# NET-E1 — отчёт: GatewayUrl, op40/heartbeat, супервайзер

Батч/карточка: RW-39 / NET-E1 (Core gateway). PM: DeepSeek.
HEAD: `0239543` (рабочее дерево с параллельными изменениями других воркеров).
Основание: `CORE-AUDIT.md` P0-3; `AUTH-RE.md` G-04…G-20; `ADR-2-auth.md`; `BRIEF-NETE1.md`.
Сеть/окно/GUI/commit — не использовались; все сетевые кейсы — локальные стабы `127.0.0.1`.

## 1. Сделано

### 1.1 `GatewayUrl` (`src/ws.rs`)
- `GATEWAY_ENDPOINT = wss://gateway.discord.gg`, `GATEWAY_QUERY = encoding=etf&v=9&compress=zstd-stream`.
- `GatewayUrl::{remote, from_base, resume, as_str}`, `Default`, `Display`.
- Нормализация: trim; `https→wss`, `http→ws`, схема без префикса → `wss`; хвостовые `/`
  обрезаются; старый query отбрасывается и заменяется каноническим (без дублей); пустой URL,
  чужая схема, пустой хост, пробелы, фрагмент — типизированные ошибки (`GatewayUrlError`),
  не паники.
- `resume_gateway_url` из READY проходит ту же нормализацию (G-16); результат сохраняется
  супервайзером и используется для всех реконнектов.

### 1.2 Heartbeat: op40 + fallback op1 (`src/gateway.rs`, `src/ws.rs`)
- `QOS_HEARTBEAT_OP = 40`; `HeartbeatMode::{Qos, LegacyOp1}`; `QosPayload { active, reasons }`.
- `Session::with_heartbeat_mode`, `heartbeat_mode/set_heartbeat_mode`, `qos/set_qos`.
- Боевой режим (`Qos`): каждый удар — `encode_qos_heartbeat` → `{op:40, d:{seq, qos}}`
  (`seq=0` до первого диспатча; `qos=null` либо `{active:true/false, reasons:[…]}`),
  как `_sendHeartbeat` клиента 1.0.162 (G-12).
- `op1` от сервера в `Qos` — немедленный удар + перезапуск интервала (`now + interval`),
  как `_handleHeartbeatReceive` (G-12/G-13); в `LegacyOp1` op1 по-прежнему игнорируется.
- `op11` ACK закрывает удар/меряет RTT; нет ACK за интервал → `Reconnect{Zombie}` (G-14).
- Fallback: `Session::new` остаётся `LegacyOp1` (контракты RW-03/04/06 не ломаются),
  боевой путь (супервайзер) явно включает `HeartbeatMode::Qos`; тест на op1 есть.

### 1.3 Супервайзер (`src/gateway/supervisor.rs`, новый)
- Цикл connect → HELLO → IDENTIFY/RESUME → dispatch/heartbeat → close.
- `SupervisorConfig::new(gateway, token)` — боевые дефолты RE: capabilities 1734653,
  `compress:false`, online-presence, `qos_token` client-provided.
- `SupervisorConfig::with_super_properties(...)` — properties = super-properties + 4 поля
  идентификации (`client_app_state`, `is_fast_connect:false`, `gateway_connect_reasons`,
  `installation_id?`), без `intents/shard`; присутствие из `presence`.
- `plan_reconnect` (T-GW-07) выбирает RESUME/IDENTIFY: разрыв < 30 с и сессия известна и
  op9 не запретил resume → RESUME, иначе IDENTIFY. `d=true` → RESUME, `d=false` → IDENTIFY.
- op7 → новый коннект (с сохранённой сессией → RESUME). Close-политика:
  `4004 → Stop(Unauthorized)`, `4013/4014 → Stop(InvalidClient)`, прочие/обрыв → reconnect.
- Backoff: 1000 мс, ×2, потолок 60 000, сброс на READY (как `gatewayBackoff` клиента);
  лимит `max_reconnects` → `Stop(ReconnectLimit)`; отмена — `run_until(shutdown)` на любом
  шаге, включая backoff (`SupervisorOutcome::Cancelled`).
- Диагностика без молчаливого проглатывания: `last_error()`, `last_url_error()`,
  `resume_state()`, `resume_url()`, `reconnects()`.

### 1.4 IDENTIFY по RE
`identify.rs` уже соответствовал G-04…G-11 (capabilities 1734653, `compress:false`,
properties+4 поля, без intents/shard) — не менялся. Добавлена связь супервайзера с
`SuperProperties` и контрактный тест на реальном стабе (7 полей, значения, отсутствие
`intents/shard`).

## 2. Red → green

Red-first: сначала написаны тесты (`tests/gateway_supervisor.rs` — 10, `tests/ws_transport.rs`
— +6), прогон упал на отсутствующих API (`GatewayUrl`, `HeartbeatMode`, `QosPayload`,
`encode_qos_heartbeat`, `supervisor`) — `PM/nete1/01-red.log`.

В процессе красными были 3 стаб-кейса супервайзера: стаб слал каждый терм отдельным
завершённым zstd-кадром → `Zstd(TrailingData)`, а тест отмены считал штатный reset ошибкой.
Исправлено в тестовом стабе (непрерывный zstd-поток + ожидаемый обрыв), а не ослаблением
проверок; после этого — зелёные.

Green:
- targeted: `gateway_supervisor` 10/10, `ws_transport` 19/19 — `03-green-targeted.log`;
- полный `discord-core`: **568 passed, 0 failed** — `04-green-full.log`;
- финальный гейт (test + clippy + fmt) — `07-final-gate.log`.

Тесты: было **552** → стало **568** (+16: ws 13→19, supervisor 0→10).

## 3. Гейт

```
cd rust
cargo test --offline -p discord-core                  # TEST_EXIT=0, 568 passed / 0 failed
cargo clippy --offline -p discord-core --all-targets -- -D warnings   # CLIPPY_EXIT=0
cargo fmt -p discord-core -- --check                  # FMT_EXIT=0
```

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/ws.rs` | `7373ccba85efbac2ddd79d73cca3dc3f` |
| `rust/crates/discord-core/src/gateway.rs` | `b5df4e3631a997eba32928fc509336f4` |
| `rust/crates/discord-core/src/gateway/supervisor.rs` (new) | `06b53ec8dbdb1d94af7b8e3df60bd264` |
| `rust/crates/discord-core/src/lib.rs` | `06ca51b79dae5651c4bd942c045d827d` |
| `rust/crates/discord-core/tests/ws_transport.rs` | `4c63de7de7718c1263dd7a84757017c7` |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` (new) | `866fdb1150c899973000fdf901fb51dd` |

Логи: `00-baseline.log`, `01-red.log`, `02-compile.log`, `03-green-targeted.log`,
`04-green-full.log`, `05-clippy.log`, `06-fmt-check.log` (red fmt до форматирования),
`07-final-gate.log`.

## 5. Остаток / риски

1. `qos` в op40 пока всегда `null`: источника active-состояния (presence/activity) в ядре нет;
   API `Session::set_qos` готов, интеграция — за слоем состояния/UI.
2. `Op::from_code(40)` намеренно остаётся `Op::Unknown(40)` (контракт RW-03 и его тест);
   боевой op40 различается `HeartbeatMode`, а не вариантом `Op`. Это осознанное отклонение
   от буквы «Op::QosHeartbeat», чтобы не ломать 552 базовых теста.
3. Backoff/счётчик реконнектов сбрасываются на READY, как у клиента; сервер, который всегда
   отдаёт READY и сразу закрывает соединение, может зациклиться (патология; лимит действует
   для отказов до READY). При необходимости — отдельный порог «сессия жила N мс».
4. `resume_gateway_url` используется как есть после нормализации; живой проверки (окно) не было
   по ТЗ. Все выводы — на стабах и oracle из RE.
5. `identify.rs` не менялся (уже соответствовал RE); если потребуется `installation_id` из
   keyring/store — это отдельная карточка.
