# Отчёт RW-14 / P2-06 — memory history и пагинация

## 1. Сделано

- **W14a** — `src/history/types.rs`: `HistoryConfig`, `History`, `ChannelHistory`,
  `WindowDirection`, `HistoryDiagnostics`, `HistoryError`, `LiveOutcome`; bounded window,
  Last/Before/After/Around trim, Live > Page, live defer при прокрутке вверх, Debug без content.
- **W14b** — `src/history/pagination.rs`: `Cursor`, `PageRequest`, `MessagePage`, `PageToken`,
  `PageApply`, `PageError`; `from_effect` (только `FetchMessages`), query-формы, generation,
  stale, атомарный `ChannelMismatch`, dedup/last-wins, счётчики уникальных id.
- **W14c** — `src/history/selectors.rs`: `messages`, `recent`, `message`, `oldest/newest`,
  `has_older/newer`, `range`, `window_around`, `len`/`total_len`/`channel_count`.
- **Координатор** — `history/mod.rs`, корневые re-exports в `lib.rs`, red/green-артефакты,
  тест-коррекции, интеграция, gate.

## 2. Red

- W14a/b: compile-red до появления `discord_core::history` — `red-w14ab.log`, exit **101**;
  после setup-фикса helper остаётся compile-red (модуля нет). Подробно — `TEST-CORRECTIONS.md`.
- W14c: behavior-red отсутствующих селекторов — `red-w14c.log`, exit **101**; green 8/8.
- P2-1: root re-exports — `red-w14-root.log`, exit **101**; green 20/20.
- P3-2: счётчики страницы — `red-w14-counts.log`, exit **101** (было replaced=1 при
  дубликате в странице); после дедупа — green.

## 3. Green

- targeted: `history_types` 7 + `history_pagination` 5 + `history_selectors` 8 = **20/20**
  (`green-w14.log`).
- `cargo test --offline -p discord-core` — **295 passed / 0 failed** (`green-core-final.log`).
- `cargo test --offline --workspace` — **337 passed / 0 failed** (`green-workspace-final.log`).
- `cargo fmt --all -- --check` — exit 0.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — exit 0.
- `npm run gate:worker` — **passed**, `.gate/worker-2026-10-20T11-45-30-914Z.json`
  (`gate.log`).

## 4. Границы scope

Memory-only ядро. REST-JSON/response, сеть, SQLite, перезапуск, отмена/таймауты, tombstones
(`MESSAGE_DELETE` не подтверждён) и UI — вне волны. `MessagePage` — seam для будущего адаптера.
`after`/`around` — локальная модель/контракт, capture подтверждает только формы запросов
`limit`/`before`/`around`, но не серверную семантику. `State.messages` не ограничивается этой
волной.

## 5. Ревью

Независимый ревьюер: раунд 1 — P0/P1=0, P2-1 (root re-exports), P3-1…P3-8. P2-1 закрыт
red-first; P3-2/P3-4 закрыты, `duplicates` ограничен 256, DESIGN уточнён. Повторный раунд —
**«принято»**, P0/P1/P2 = 0; P3-1…P3-7 — backlog (`waves/reviews/review-p2-06.md`).

## 6. Файлы

- `rust/crates/discord-core/src/history/{mod,types,pagination,selectors}.rs`
- `rust/crates/discord-core/src/lib.rs`
- `rust/crates/discord-core/tests/history_{types,pagination,selectors}.rs`
- `waves/RW-14/*`, `waves/reviews/review-p2-06.md`

## 7. Риски

- P3: `live_ids` удаляются при вытеснении; очень поздняя страница теоретически может
  воскресить вытесненный live-id (ограничено окном); нет TTL/лимита на число каналов и на
  `State.messages`; `after` не подтверждён capture; нет tombstones.
- Полный P2-06 (REST response, SQLite, перезапуск) требует отдельной волны/решения владельца.
