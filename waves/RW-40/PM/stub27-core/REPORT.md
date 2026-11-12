# STUB-27-CORE-STREAM-OPCODES — отчёт воркера G1 Core Rust Architecture (RW-40)

Дата: 2026-10-30. Воркер: **G1 Core Rust Architecture**. Карточка: `STUB-27-CORE-STREAM-OPCODES`.
Базовый коммит: `4e24048`.
Контракт: `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md`.
Векторные фикстуры: `waves/RW-40/VOICE/stream-opcodes-fixtures.json` (`rw40/stream-opcodes-fixtures-v1`).

---

## 1. Сделано

1. **Контрактный тест (RED-first)** `rust/crates/discord-core/tests/stream_gateway_contract.rs`:
   - `t_stream_00_anchors_match_contract_and_oracle_bundle` — сверка всех 17 якорей (`A-OPCODES`…`A-REST`) из `stream-opcodes-fixtures.json` с документом `STREAM-GATEWAY-CONTRACT.md` и побайтово (`offset`, `len`, `quote`, размер `12 410 287` Б) с RE-бандлом `reference-oracle/decoded/4d2a64cb5df5fc4e_0`;
   - `t_stream_01_opcode_table_and_gateway_isolation` — таблица опкодов 18…22 (`StreamOp::Create=18` / `StreamOp::StreamCreate`, `Delete=19` / `StreamDelete`, `Watch=20` / `StreamWatch`, `Ping=21` / `StreamPing`, `SetPaused=22` / `StreamSetPaused`), `from_code(23) == None`, неизменность `discord_core::Op::from_code(18..=22) == Op::Unknown(18..=22)`;
   - `t_stream_02_03_04_outbound_commands_match_fixtures` — кодирование всех 9 исходящих векторов `outbound` (`out-18-create-guild`…`out-22-set-paused-false`), проверка D-27-2 (`guild_id: null` / `EtfValue::Nil` для `call`), проверка D-27-3 (отказ `SessionNotEstablished` вне `GatewaySessionState::Established`);
   - `t_stream_05_stream_key_roundtrip_and_canonical_form` — `StreamKey::parse`, `to_wire`, `canonical`, `build`, `guild`, `call`, `MAX_KEY_BYTES = 128`;
   - `t_stream_06_negative_stream_key_vectors` — все 13 негативных векторов `stream_key` из фикстуры (`Empty`, `TooLong`, `UnknownKind`, `SegmentCount`, `EmptySegment`, `NotSnowflake`, `Overflow`) и код `stream.key.invalid`;
   - `t_stream_07_redacted_debug_display_and_snapshot` — отсутствие утечки полной строки ключа, `guild_id` (D-27-6), `token` (D-27-4) и `endpoint` в `Debug`, `Display` и `StreamState::snapshot()`;
   - `t_stream_08_inbound_dispatches_match_fixtures` — разбор всех 21 входящих векторов `inbound` (`STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_UPDATE`, `STREAM_DELETE`, а также игнорирование `VOICE_STATE_UPDATE` как `None`), включая D-27-5 (`viewer_ids: Some(vec![]) != None`) и D-27-7 (`StreamDeleteReason::Unknown`);
   - `t_stream_09_to_16_and_21_reducer_scenarios_replay` — пошаговый реплей всех 14 сценариев `reducer` (`R-01`…`R-13`, включая `R-08` и `R-08b`): фазы (`Connecting`, `Active`, `Paused`, `Reconnecting`, `Ended`, `Failed`), удержание записи, `transport_ready`, эффекты (`OpenStreamRtc`, `CloseStreamRtc`, `ShowSafetyRateLimit`, `NotifyViewerFull`), `stale_sequences`, `unknown_streams`;
   - `t_stream_11_12_13_hypotheses_and_diagnostics` — проверка гипотез `H-PAUSE` (отсутствие `paused` в `STREAM_UPDATE` сохраняет паузу), `H-SERVER-UPDATE` (`orphan_server_updates`), `D-27-7` (`unknown_reasons`);
   - `t_stream_17_18_negative_command_and_inbound_vectors` — все 9 негативных векторов `command` и все 5 негативных векторов `inbound` (жёсткие ошибки `invalid_dispatches` и мягкие `malformed_fields`), проверка `parse_dispatch("STREAM_FOO") == None`;
   - `t_stream_19_20_22_etf_prefixes_idempotency_and_layer_boundary` — отсутствие паник на всех префиксах ETF-байтов каждого исходящего и входящего кадра, идемпотентность `encode()`, round-trip `from_etf_frame`, граница слоёв G5↔G1 (`StreamKey::build`).

2. **Продуктовый модуль** `rust/crates/discord-core/src/stream.rs` (подключён строкой `pub mod stream;` в `rust/crates/discord-core/src/lib.rs`):
   - `StreamKind` / `StreamType` (`Guild`, `Call`), `StreamKey`, `StreamKeyError`, редактированный `Debug`/`Display` (D-27-1, D-27-6);
   - `StreamOp` (`18..=22` с константами `StreamCreate`…`StreamSetPaused`), `GatewaySessionState`, `StreamCommand` (`validate`, `op`, `to_etf`, `encode`, `encode_if_established`, `encode_in_session`, `from_raw_spec`, `from_etf_frame`) (D-27-2, D-27-3);
   - `StreamSecret` (D-27-4), `StreamCreated`, `StreamServerUpdate`, `StreamUpdate`, `StreamDeleted`, `StreamDeleteReason` (D-27-7), `StreamDispatch`, `parse_dispatch`, `parse_dispatch_with_diagnostics` (D-27-5);
   - `StreamPhase` (6 фаз), `Stream`, `StreamState`, `StreamDiagnostics`, `StreamSnapshot`, `StreamSummary`, `StreamEffect`, `StreamAction`, `LocalStreamAction`, чистый редьюсер `apply` с выделенными функциями гипотез `H-VIEWERS` (`merge_viewer_ids`) и `H-PAUSE` (`resolve_paused`).

---

## 2. RED-прогон (до написания `src/stream.rs`)

Команда:
```bash
(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo test --offline -p discord-core --test stream_gateway_contract)
```
- Exit code: `101`
- Первая ошибка: `error[E0432]: unresolved import discord_core::stream` (`crates/discord-core/tests/stream_gateway_contract.rs:14:19`)
- Лог: `waves/RW-40/PM/stub27-core/red.log`

---

## 3. GREEN-прогон и targeted-проверки

| Команда | Exit code | Результат | Артефакт |
|---|---:|---|---|
| `(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo test --offline -p discord-core --test stream_gateway_contract)` | `0` | **11 passed; 0 failed** (все T-STREAM-00…22) | `waves/RW-40/PM/stub27-core/green.log` |
| `(cd rust && CARGO_TARGET_DIR=/tmp/target-g1-core cargo clippy --offline -p discord-core --test stream_gateway_contract -- -D warnings)` | `0` | 0 предупреждений | — |
| `(cd rust && rustfmt --edition 2024 --check crates/discord-core/src/stream.rs crates/discord-core/src/lib.rs crates/discord-core/tests/stream_gateway_contract.rs)` | `0` | чисто | — |

---

## 4. Мутационное тестирование (5 мутаций из §7.2 контракта)

Лог всех 5 мутаций и финального восстановления: `waves/RW-40/PM/stub27-core/mutations.log`.

| # | Мутация в `src/stream.rs` | Инвариант | Статус | Упавший тест | Exit code |
|---:|---|---|---|---|---:|
| M1 | `unavailable == Some(true)` ставит `StreamPhase::Ended` вместо `Reconnecting` | §4.1, T-STREAM-09 | **CAUGHT** | `t_stream_09_to_16_and_21_reducer_scenarios_replay` (`R-05-unavailable-reconnect`) | `101` |
| M2 | `resolve_paused` возвращает `incoming.unwrap_or(false)` вместо `incoming.unwrap_or(previous)` | H-PAUSE, T-STREAM-12 | **CAUGHT** | `t_stream_11_12_13_hypotheses_and_diagnostics` | `101` |
| M3 | `parse_soft_viewer_ids` нормализует пустой массив `[]` в `None` | D-27-5, T-STREAM-08 | **CAUGHT** | `t_stream_08_inbound_dispatches_match_fixtures` (`in-create-empty-viewers`) | `101` |
| M4 | `StreamSecret::fmt` (`Debug`) печатает сырой токен `&self.0` вместо `"<redacted>"` | D-27-4, T-STREAM-07 | **CAUGHT** | `t_stream_07_redacted_debug_display_and_snapshot` | `101` |
| M5 | `parse_snowflake_segment` разрешает ведущий ноль (`seg == "0"` вместо `seg.starts_with('0')`) | §2.4, T-STREAM-06 | **CAUGHT** | `t_stream_06_negative_stream_key_vectors` (`neg-key-leading-zero`) | `101` |
| Restore | Откат M5 к эталонному `src/stream.rs` и повторный прогон | Все T-STREAM-00…22 | **GREEN** | **11 passed; 0 failed** | `0` |

---

## 5. Файлы и контрольные суммы (md5)

| Файл | Статус | md5 |
|---|---|---|
| `rust/crates/discord-core/src/stream.rs` | new | `e4e0450559bcf947261ca7bc91aea91f` |
| `rust/crates/discord-core/src/lib.rs` | modified (`+1` строка `pub mod stream;`) | `e847d7c368fe2ff7e3615dd745ac65a9` |
| `rust/crates/discord-core/tests/stream_gateway_contract.rs` | new | `bfc82f9d2dacc7bccb1e1475ddb5c7c0` |
| `waves/RW-40/PM/stub27-core/red.log` | new | `969dccd1a094f8832f2509282d0aa334` |
| `waves/RW-40/PM/stub27-core/green.log` | new | `0ff196e8fb3cf344d08f87db42100339` |
| `waves/RW-40/PM/stub27-core/mutations.log` | new | `48c497617ee50d118b652d6f3379b8fb` |

---

## 6. Риски и границы карточки

1. **Интеграция в общие файлы (`dispatch.rs`, `state/types.rs`, `state/mod.rs`)**: по матрице §8 `STREAM-GATEWAY-CONTRACT.md` и правилам `AGENTS.md` общие файлы не менялись. Подключение `StreamDispatch` к `DispatchKind` (21→25) выполняется на integration-шаге Team Lead.
2. **Гипотезы до live-захвата (`H-VIEWERS`, `H-PAUSE`, `H-SERVER-UPDATE`, `H-DELETE-END`)**: выделены именованными комментариями и изолированными функциями в `src/stream.rs` для однострочной коррекции после live-сессии.
3. **Запреты соблюдены**: `rust/gui-spike/**`, `ui/**`, `logs/`, `functions/`, `evidence/`, `gateway.rs` не затрагивались; `git commit` не выполнялся; все запуски `cargo` выполнялись исключительно с `CARGO_TARGET_DIR=/tmp/target-g1-core --offline`.
