# Независимое ревью RW-14 / P2-06

- Дата: 2026-10-20.
- Предмет: memory-only history/pagination core (`discord-core::history`).
- Ревьюер: отдельный read-only агент, не автор кода (`ses_ea6df72b2ffexNaYvIwX2qsdRd`).
- Контракт: `waves/RW-14/{RESEARCH,DESIGN-P2-06,TZ-P2-06}.md`.
- Итог: **принято**, P0 = 0, P1 = 0, P2 = 0. P3 = 7 — backlog.

## 1. Закрытие findings

Первое ревью: P0/P1 = 0, **P2-1** — отсутствовали корневые re-exports публичного API
(`src/lib.rs` содержал только `pub mod history;`). P3-1…P3-8 — неблокирующие.

P2-1 закрыт red-first: `h07_crate_root_reexports`, red `red-w14-root.log` exit 101
(unresolved imports), после `pub use history::{...}` — green 20/20. Внешний compile-probe
подтвердил все 11 имён DESIGN §2 из `discord_core` и `discord_core::history`.

Дополнительно закрыты:
- P3-2 — страница дедуплицируется до счётчиков; `inserted/replaced` считают уникальные id
  (`red-w14-counts.log`, exit 101 → green);
- `duplicates` ограничен 256 id;
- `expect` в `apply_page` заменён на безопасную ветку;
- DESIGN уточнён: `has_newer=Some(false)` только для действительно нового live, путь
  `state::Effect`.

## 2. Проверенный контракт

| Пункт | Код | Доказательство |
|---|---|---|
| bounded window, trim Latest/After/Before/Around | `types.rs:217-264` | h05/h09 + внешний probe, `Before [1..5] cap=3 → [1,2,3]`, `Around(3) → [2,3,4]` |
| Around tie — меньший id | `types.rs:256-262`, `selectors.rs:116-120` | h09, s05 + probe |
| Live > Page, page/page last-wins | `pagination.rs:294-304`, `types.rs:162-169` | h10 + probe |
| dedup/counters, cap 256 | `pagination.rs:256-270,315-325` | h11; probe 301 повтор → diagnostics len 256 |
| PageToken generation/stale | `pagination.rs:209-218,236-243,311` | h11; повтор токена → `StaleToken`, данные не меняются |
| атомарный ChannelMismatch | `pagination.rs:245-254` | h08; probe: messages/bounds/duplicates не изменились |
| tri-state bounds | `pagination.rs:305-310` | s02 |
| from_effect/query | `pagination.rs:55-68,87-94` | h07 |
| селекторы, Debug без content | `selectors.rs:14-121`, `types.rs:106-124` | s01–s08, h06/s07 |
| has_newer | `types.rs:182-188`, `DESIGN:73` | h10 |

## 3. P0/P1/P2

Не найдено: паник/`unwrap`/`expect`/`unsafe` в `src/history/*`; утечек окна; ошибок trim;
потери live в текущем окне; обхода stale/cross-channel. Теоретический wrap generation и
`usize`-счётчики — P3, практически недостижимы.

## 4. Прогоны

| Команда | Результат |
|---|---|
| targeted history tests | **20/20** (7+5+8), exit 0 |
| `cargo test --offline --workspace` | **337 passed / 0 failed**, exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0 |
| `npm run gate:worker` | passed, `.gate/worker-2026-10-20T11-45-30-914Z.json` |

## 5. P3 (не блокирует)

1. Нет committed-тестов на Before-trim (W14a case 5) и Around-deferred live (case 6); код и
   внешний probe корректны.
2. Page/page last-wins и повтор успешного токена покрыты h10/h11 частично, не всеми
   сценариями TZ W14b case 4.
3. `h07` импортирует `History`/`HistoryConfig`/`HistoryError`/`LiveOutcome` из `history`, а не
   из корня; внешний probe подтвердил корень.
4. Cap 256 не покрыт committed-тестом; probe 301 повтор подтвердил.
5. Теоретические overflow generation/`usize`-счётчиков.
6. `apply_live` возвращает `Inserted` и для сообщения, сразу вытесненного окном; контракт
   допускает insert-then-trim, но имя не означает «осталось в окне».
7. Поздняя страница может вернуть вытесненный live-id; `live_ids` живут только в окне —
   соответствует DESIGN, отмечено в REPORT §7.

## 6. Вердикт

**Принято.** Блокирующих P0/P1/P2 нет. P3 — backlog; scope волны — memory-only, REST
response/JSON/SQLite/перезапуск/таймауты явно вне её.
