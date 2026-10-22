# B01 — batch TZ: P2-09 accepted + P2-10 voice + P2-11 typing

Дата: 2026-10-21. Процесс: `PROCESS.md` §3.13–§3.15, роли: `docs/rust-client/TEAM.md`.
Цель: закрыть P2-09 приёмкой и реализовать P2-10/P2-11 **двумя активными кодовыми
воркерами** (CORE-1 voice, CORE-2 typing), не шестью.

## 1. Состав

| Карточка | Lane | Владелец | Контракт | Статус |
| --- | --- | --- | --- | --- |
| P2-09 presence | accept | REV-1 + TL-1 | `RW-18/DESIGN-P2-09.md` | **accepted 2026-10-21**, P0/P1/P2=0, P3=8 |
| P2-10 voice | W19 | CORE-1 | `RW-19/DESIGN-P2-10.md` | **integrated green 15/15**, gate/review pending |
| P2-11 typing | W20 | CORE-2 | `RW-20/DESIGN-P2-11.md` | **integrated green 15/15**, gate/review pending |
| P2-08 settings | B02/W21 | CORE-3 | `RW-21/DESIGN-P2-08.md` | freeze ready, не в B01 |

## 2. File matrix (никто не пересекается)

| Lane | Новые/свои файлы | Shared (только TL-2) |
| --- | --- | --- |
| W19 voice | `state/voice/{mod,types,parser,reducer,selectors}.rs`; `tests/voice_state_*.rs` | `state/mod.rs`, `state/types.rs`, `state/reducer.rs`, `src/lib.rs` |
| W20 typing | `state/typing/{mod,types,parser,reducer,selectors}.rs`; `tests/typing_*.rs`; `tests/data/typing-run2.jsonl` | те же |
| P2-09 accept | `waves/reviews/*`, журнал | TL-1 |
| QA | `waves/B01/*` mutation/gate | — |

W19/W20 **не** добавляют `pub mod` в `state/mod.rs` и не трогают `EventPayload`; это I0/I1.

## 3. Подволны (2 active workers)

| Подволна | W19 (CORE-1) | W20 (CORE-2) | Ворота |
| --- | --- | --- | --- |
| a | types + parser + `voice_state_types` | types + parser + `typing_types` | compile-red, затем I0, parser green |
| b | reducer + `voice_state_reducer` | reducer + `typing_reducer` | behavior-red, затем I1, reducer green |
| c | selectors + `voice_state_selectors` | selectors + `typing_selectors` | behavior-red, green |
| QA | mutations voice | mutations typing | 5–8 каждая, restore md5 |

Правило: следующий подволна стартует после green предыдущей пары; review/QA предыдущего
batch идёт параллельно и не считается code WIP.

## 4. Integration lane (TL-2)

### I0 — store scaffolding (после файлов W19/W20)

- `state/mod.rs`: `pub mod voice; pub mod typing;` + re-exports типов/функций.
- `state/types.rs`: `State` поля
  - `voice_states: BTreeMap<VoiceKey, VoiceState>`,
  - `voice_channel_start_times: BTreeMap<VoiceChannelKey, i64>`,
  - `voice_channel_statuses: BTreeMap<VoiceChannelKey, String>`,
  - `typing: BTreeMap<TypingKey, TypingEntry>`;
  - `Debug` — длины; `from_ready` — пустые карты;
  - `ReadySupplemental.voice_states: Vec<VoiceState>`;
  - `parse_ready_supplemental` — `parse_voice_states(...)`.
- I0 не добавляет `EventPayload`/routes.

### I1 — единый API-патч (после reducer-файлов W19/W20)

- `EventPayload`: `VoiceStateUpdate`, `VoiceChannelStartTime`, `VoiceChannelStatus`,
  `TypingStart`.
- `Event::from_routed` — парсеры voice/typing; `Event::from_frame_at(frame, now_ms)` —
  typing с `received_at_ms=Some(now)`.
- `apply`/`apply_dispatch` arms; `apply_frame_at`.
- `MESSAGE_CREATE` → `typing::reducer::remove_for_message` (author id до move).
- `merge_supplemental` → `voice::reducer::apply_snapshot`.
- `src/lib.rs` re-exports: voice/typing types, `TYPING_TTL_MS`.
- `state/mod.rs` re-exports.
- **Один** I1-патч по shared-файлам; I0 — только каркас store.

## 5. Тесты и gate

- W19: 15 targeted (`voice_state_types/reducer/selectors`).
- W20: 15 targeted (`typing_types/reducer/selectors`).
- P2-09: 54 targeted уже зелёные, accepted.
- Ожидаемо после B01: workspace `515 + 30 = 545`, core `473 + 30 = 503` (если I1 не добавит
  unit-тестов; уточнить по факту).
- **Факт integration (2026-10-21):** targeted 30/30, core **503**, workspace **545**,
  fmt/clippy exit 0; см. `INTEGRATION.md`, `logs/`.
- Мутации: **16/16 caught** (8 voice + 8 typing); два выживших закрыты red-first тестами
  (V3 zero timer, V5 guild filter) — `MUTATIONS.md`.
- Shared-тесты после типизации: `state_reducer` r10 и `state_types` t08 переведены на
  `VOICE_SERVER_UPDATE`; `ReadySupplemental` получил `voice_states` — см. `INTEGRATION.md` §2.
- Один batch gate в integration-лейне, cargo **последовательно**:
  ```bash
  cargo fmt --all -- --check
  cargo clippy --offline --workspace --all-targets -- -D warnings
  cargo test --offline --workspace
  npm run gate:worker
  ```
- Gate artifact — `.gate/worker-<timestamp>.json`; e2e-повтор — как в `PROCESS.md`.
- После gate — два независимых ревьюера (P2-10, P2-11) параллельно; P0/P1/P2 — red-first,
  повторный gate; P3 — backlog.

## 6. Критерии accepted B01

- P2-09: review round 2, P0/P1/P2=0.
- P2-10/P2-11: 15/15 + 15/15, mutations caught, gate 0, review P0/P1/P2=0.
- Нет сетевых/новых зависимостей/live/SQLite/UI; shared-файлы менял только TL-2.
- Доки/журнал — один раз после accepted; `waves/db.jsonl` append-only.
- `.ua/knowledge-graph.json` пересобрать после accepted (или явно оставить stale-ноту).

## 7. Статус B01 на 2026-10-21

- I0/I1 применены, targeted/core/workspace/fmt/clippy зелёные; evidence — `INTEGRATION.md`,
  `logs/targeted-final.log`, `logs/core-503.log`, `logs/workspace-545.log`.
- Мутации 16/16; evidence — `MUTATIONS.md`, `mutations/`.
- Осталось: **один batch `gate:worker`**, два независимых review, приёмка TL-1.
- P2-09 уже accepted; B01 не меняет его P3-список.
- Incident: `waves/RW-20/` внешне исчез и восстановлен; fixture md5 совпал, код/тесты не
  пострадали (см. W20 report §7).
