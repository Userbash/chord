# Отчёт RW-13 / P2-02 — state/reducer/selectors

## 1. Сделано

- **W13a** — `src/state/types.rs`: `State`, `Event`, `EventPayload`, `EventError`,
  `StateDiagnostics`, `Message`, `MessagePatch`, `Thread`, `ThreadListSync`, `ChannelInfo`,
  `ReadySupplemental`, `Effect`/`Effects`/`MemberRange`; разбор READY/SUPPLEMENTAL/RESUMED,
  SESSIONS_REPLACE, THREAD_LIST_SYNC, CHANNEL_INFO, MESSAGE_CREATE/UPDATE; redaction `Debug`.
- **W13b** — `src/state/reducer.rs`: чистый `apply`/`apply_dispatch`/`apply_frame`, stale-
  политика (включая malformed), READY-reset, слияние supplemental/members, upsert-ы,
  `SubscribeMembers`; без I/O.
- **W13c** — `src/state/selectors.rs`: guild/channel/private/user/relationship/message/thread/
  member/read-state выборки, детерминированная сортировка, счётчики.
- **Координатор** — `state/mod.rs`, корневые re-exports в `lib.rs`, выделение
  `ready/guild.rs::parse_channel_value` без изменения P2-01; тестовый `state_fixture.rs`.

## 2. Red

- Исходный: `cargo test --offline -p discord-core --test state_reducer` → compile-error
  `unresolved import discord_core::state`, exit **101** (`red.log`, `red.exit`).
- После ревью: `red-review-findings.log` — compile-red отсутствующих корневых re-exports,
  exit **101**; `red-review-findings-runtime.log` — `15 passed; 4 failed` (r14–r17),
  exit **101** (`red-review-findings-runtime.exit`).
- P1/P2 findings закрыты red-first; `r18` — guard (проходил на старом коде), `r12` —
  property-проверка после ревью, отдельного red нет. Честно отражено в
  `TEST-CORRECTIONS.md`.

## 3. Green

- `cargo test --offline -p discord-core --test state_types --test state_reducer
  --test state_selectors` → **42 passed / 0 failed** (15+19+8), `green-final-state.log`.
- `cargo test --offline -p discord-core` → **275 passed / 0 failed**.
- `cargo test --offline --workspace` → **317 passed / 0 failed** (`green-workspace.log`).
- `cargo fmt --all -- --check` → exit **0**.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` → exit **0**.
- `npm run gate:worker` → **passed**; финальный отчёт
  `.gate/worker-2026-10-20T10-58-32-180Z.json` (`gate-after-docs.log`).

## 4. Ревью

- Первый раунд: **не принято** — P1 stale malformed + P2-1…P2-5; все закрыты.
- Повторный раунд: **принято**, P0/P1/P2 = 0; P3-1…P3-6 — backlog.
- Отчёт: `waves/reviews/review-p2-02.md`; независимый агент, не автор кода.

## 5. Файлы

- `rust/crates/discord-core/src/state/{mod,types,reducer,selectors}.rs`
- `rust/crates/discord-core/src/lib.rs`
- `rust/crates/discord-core/src/ready/guild.rs`
- `rust/crates/discord-core/tests/{state_types,state_reducer,state_selectors}.rs`
- `rust/crates/discord-core/tests/support/state_fixture.rs`
- `waves/RW-13/*`, `waves/reviews/review-p2-02.md`
- Доки: `waves/STATE.md`, `waves/TASK-TREE-RUST.md`, `docs/rust-client/{STATUS,ROADMAP}.md`,
  `docs/map.md`, `TASKS.md`, `README.md`, `waves/db.jsonl`.

## 6. Риски/вопросы

- P3: неполное покрытие отдельных веток, синтетические thread-id, `r19` не вызывает
  `apply_dispatch`/`apply_frame`, пустая supplemental-группа с невалидным raw-индексом.
- SQLite (`P2-03`) и GUI/keyring остаются заблокированными решениями владельца; P2-06
  делается в памяти.
- Коммит/push не выполнялись: рабочее дерево dirty, это делает координатор.
