# Отчёт воркера — task-141 (BE-RE-06 / F-GW-07): сброс сессии после 4 неудачных RESUME и неограниченный reconnect

## 1. Batch / карточка

- **Волна:** `RW-40`, отдел `G1` (Backend).
- **Карточка:** `task-141` / `BE-RE-06` (`BE-P2-06б` в `waves/RW-40/PM/BACKLOG-RE-BACKEND.md`), находка `F-GW-07` (`waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` §3.6).
- **Файл воркера:** только `rust/crates/discord-core/src/gateway/supervisor.rs`.
- **Ветка / worktree:** `feature/task-141-gateway-reconnect-infinite`, HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748`.
- **Независимый оракул:** веб-бандл Discord 1.0.162 `/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (md5 `934b1289b7ebf17bc0fd09569b23b438`):
  - `_handleClose` (@5 014 326): останавливает переподключение только на фатальном коде `4004===t` (`connectionState = CLOSED`, `_reset`); на остальных кодах вызывает `this.gatewayBackoff.fail(() => this._connect())` и при `this.gatewayBackoff.fails > 4` вызывает `this._reset(e, t, n)`;
  - `_reset` (@5 015 390): `this.sessionId = null; this.seq = 0;` — следующая попытка идёт со свежим `IDENTIFY` (op=2), а не `RESUME` (op=6);
  - `gatewayBackoff = new Backoff(1e3, 6e4)` (`rest-static.md` §2): экспоненциальный backoff 1 000 мс ×2 с потолком 60 000 мс без жёсткого лимита на 8 попыток;
  - `_handleDispatch` (@5 013 000): как `READY`, так и `RESUMED` вызывают `this.gatewayBackoff.succeed()`, сбрасывая счётчик неудач и задержку backoff.

## 2. Что сделано

В `rust/crates/discord-core/src/gateway/supervisor.rs`:

1. Добавлена константа `pub const RESUME_RESET_AFTER_FAILURES: u32 = 4;` — порог подряд идущих неудачных попыток `RESUME`, после которого сессия сбрасывается.
2. В `SupervisorConfig::new` дефолт `max_reconnects` изменён с `8` на `0` (неограниченный режим переподключения в бою). Ненулевое значение `max_reconnects > 0` сохранено как явный конечный лимит для существующих тестов (`test_config` в `tests/gateway_supervisor.rs` задаёт `max_reconnects = 2`) и продуктовых политик.
3. В `Supervisor` добавлены поля `resume_failures: u32` и `session_resets: u32` с публичными геттерами `resume_failures(&self) -> u32` и `session_resets(&self) -> u32`, а также метод `note_attempt(&mut self, action: ReconnectAction, established: bool)`:
   - при успешном установлении сессии (`READY` или `RESUMED`) либо попытке со свежим `IDENTIFY` счётчик `resume_failures` сбрасывается в `0`;
   - при неудачной попытке `RESUME` (соединение закрылось или оборвалось без `RESUMED`) `resume_failures` увеличивается на 1;
   - при достижении `RESUME_RESET_AFTER_FAILURES` (4 неудачи подряд) выполняется клиентский `_reset`: `self.state = None` (`session_id` и `last_seq` обнулены), `self.invalid_session = None`, `self.resume_failures = 0`, `self.session_resets += 1`, а серия переподключений продолжается со свежим `IDENTIFY` (op=2) и текущим экспоненциальным backoff (вплоть до потолка 60 с).
4. В `Supervisor::absorb` сброс `backoff.reset()`, `reconnects = 0`, `invalid_session = None` и `disconnected_at = None` теперь выполняется как при `READY`, так и при `RESUMED` (`DispatchKind::Resumed`), в точном соответствии с `this.gatewayBackoff.succeed()` в `_handleDispatch`.
5. Фатальные коды закрытия (`4004` → `StopReason::Unauthorized`, `4013`/`4014` → `StopReason::InvalidClient`) по-прежнему проверяются в `classify_close` до шага backoff и немедленно останавливают супервайзер в любом режиме (включая `max_reconnects = 0` и потолок backoff 60 с).

## 3. RED-прогон

До внесения продуктовых правок в `supervisor.rs` добавлен модуль `#[cfg(test)] mod tests` с тремя контрактными тестами по оракулу и выполнен прогон:

Команда:
```sh
cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests
```

Результат (`red.log` / `RED.log`, `EXIT=101`, 0 passed, 3 failed):
- `resets_session_after_four_failed_resumes_and_switches_to_fresh_identify` — **FAILED**: после 4 неудачных `RESUME` старый код на 5-й попытке снова отправил `op=6` вместо `op=2` (`left: [Some(2), Some(6), Some(6), Some(6), Some(6), Some(6)]`, `right: [Some(2), Some(6), Some(6), Some(6), Some(6), Some(2)]`);
- `successful_resumed_resets_failure_streak_and_backoff` — **FAILED**: старый код остановился на 8-м реконнекте (`left: Stop(ReconnectLimit { reconnects: 8 })`, `right: Stop(Unauthorized)`);
- `unlimited_reconnect_mode_holds_60s_ceiling_and_stops_immediately_on_4004` — **FAILED**: старый код с дефолтным `SupervisorConfig::new` остановился на `Stop(ReconnectLimit { reconnects: 8 })` вместо прохождения 12 попыток до фатального `4004`.

## 4. GREEN-прогон и проверки

| Проверка | Команда | Результат | Лог |
|---|---|---|---|
| Targeted unit (`supervisor::tests`) | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests` | **3 passed, 0 failed** (`EXIT=0`) | `green-lib.log` |
| Все тесты `discord-core` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` | **53 набора, 580 passed, 0 failed** (`EXIT=0`) | `green.log` / `GREEN.log` |
| Clippy (`discord-core`) | `cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | **0 предупреждений** (`EXIT=0`) | `CLIPPY.log` |
| Форматирование | `cd rust && cargo fmt --all -- --check` | **чисто** (`EXIT=0`) | `FMT.log` |

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `ffa734a6b168880f14fe5cc7e601f940` |
| `waves/RW-40/G1/task-141-gateway-reconnect-infinite/red.log` | `820d1ef49c87d4b93217e189fc2624ca` |
| `waves/RW-40/G1/task-141-gateway-reconnect-infinite/green-lib.log` | `cfc60e8a33ff494ec8e0b3bea36a8c09` |
| `waves/RW-40/G1/task-141-gateway-reconnect-infinite/green.log` | `9fe78a426233cfa8774ac15bacd21e48` |
| `waves/RW-40/G1/task-141-gateway-reconnect-infinite/CLIPPY.log` | `ac53ad2cea90c21f47de3ff65ec149b4` |
| `waves/RW-40/G1/task-141-gateway-reconnect-infinite/FMT.log` | `fdbdc3d0c7b403251ac5407380220523` |

## 6. Риски и совместимость

- **Обратная совместимость тестов:** все 14 интеграционных тестов в `rust/crates/discord-core/tests/gateway_supervisor.rs` проходят без изменений; явный конечный лимит `max_reconnects > 0` (`max_reconnects = 2` в `test_config`) продолжает возвращать `Stop(StopReason::ReconnectLimit { reconnects })`.
- **Соседние находки BE-P2-06:** случайный jitter первого heartbeat (`F-GW-06`, `gateway.rs`) и 3-минутное окно `RESUME` от последнего ACK (`F-GW-08`, `resume.rs`) находятся в других модулях вне файлового скоупа `task-141` (`ТОЛЬКО supervisor.rs`).
