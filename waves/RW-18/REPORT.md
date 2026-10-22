# RW-18 / REPORT — P2-09 memory-only presence

Дата: 2026-10-20. Статус: реализация, P2-фиксы, мутации и gate завершены; раунд 1
независимого ревью закрыт, ожидается раунд 2. Контракт:
`waves/RW-18/DESIGN-P2-09.md`; research: `waves/RW-18/RESEARCH-P2-09.md`;
ТЗ: `waves/RW-18/TZ-P2-09.md`.

## 1. Итог карточки

P2-09 реализует memory-only presence: `PRESENCE_UPDATE`, `READY_SUPPLEMENTAL`,
presence из member-list и best-effort `GUILD_MEMBERS_CHUNK.presences[]`. Сеть, SQLite,
UI, outgoing presence-запросы и voice/typing не затрагиваются. Публичные поля P2-04/P2-05
расширены аддитивно; `State.users` member-list не меняет (P2-04 frozen).

Ключевая семантика:

- presence хранится по `(user_id, PresenceScope)`, scopes — `Guild(u64)` и `Private`;
- агрегат предпочитает лучший видимый `online|idle|dnd` scope; только если такого нет —
  лучший `offline+hidden`;
- offline без hidden удаляет запись; offline с hidden остаётся видимым и сохраняет
  обычные activities;
- activities нормализуются custom > competing > streaming > playing > other, затем
  displayable tie-break и новые раньше старых; остаётся один playing;
- неизвестные/неподтверждённые формы диагностируются, wire не изобретается.

## 2. Сделано по волнам

- **W18a** — `src/state/presence.rs`: типы, парсеры `PRESENCE_UPDATE`, activities
  (`party.size` байтами и list-формой, отрицательные application id строкой),
  `merged_presences`; `State.presences`, `ReadySupplemental.merged_presences`,
  `EventPayload::PresenceUpdate`, route, root re-exports.
- **W18b** — `presence_reducer.rs`: live upsert, offline/all-offline cleanup,
  supplemental friends/guilds, member-list presence, chunk best-effort; `upsert_user`
  доступен reducer-у; `MemberPresence` получил activities/hidden/timestamp; chunk
  `presences[]` маркируется как unconfirmed.
- **W18c** — `presence_selectors.rs`: статусы, scopes, activities,
  primary/custom/by-type, client statuses, mobile/vr, online ids, guild count;
  member-list `008` activities/timestamps проверены на реальной фикстуре.

Итого новых интеграционных тестов: **54** (18 `presence_types` + 18 `presence_reducer`
+ 18 `presence_selectors`) плюс unit-тест helper в `presence.rs`. Тесты скопированы в
`waves/RW-18/tests/`; манифест изменённых файлов — `MD5SUMS.txt` (15 файлов).

## 3. Red → green

| Шаг | Red | Green |
|---|---|---|
| W18a | `red-w18a.exit=101`, compile-red | `green-w18a-first.exit=0`, 16/16 |
| W18b | `red-w18b.exit=101`, compile-red (`chunk.presences`) | `green-w18b.exit=0`, 17/17; неудачная попытка `attempt-w18b-first.exit=101` сохранена отдельно |
| W18c | `red-w18c.exit=101`, no methods | `green-w18c.exit=0`, 14/14 |
| P2-1 | `red-s15.exit=101` (`s15`), старый агрегат давал offline | `green-p2-fixes.exit=0` |
| P2-2 | `red-p2-fixes.exit=101` (`r18`, нет диагностики) | `green-p2-fixes.exit=0` |

Дополнительные P3-тесты: `s15`–`s18`, `a17`–`a18`, `r18`; после восстановления мутаций
целевой набор — **54/54**, exit 0 (`targeted-54.log`).

## 4. Независимое ревью

`waves/reviews/review-p2-09.md`, раунд 1: **P0=0, P1=0, P2=2, P3=14**, вердикт
«не принято». Оба P2 закрыты red-first:

- **P2-1** — агрегат больше не понижает live online из-за более свежего offline+hidden
  scope; `s15` проверяет online guild 10 против offline+hidden guild 20, а
  `presence_in_guild(20)` продолжает возвращать скрытый scope.
- **P2-2** — пустой member-list `status` даёт `ignored` с путём
  `...member.presence.status`; `r18` покрывает; reducer-фильтр оставлен как
  defense-in-depth.

P3 закрыты в этом же цикле: P3-1 (id в ошибке), P3-2 (`guild_id=0` → ignored, `a18`),
P3-3 (без лишней inconsistency), P3-4 (`is_offline`, `s18`), P3-5 (positive VR и
mobile/desktop guards, `s16`), P3-6 (displayable tie-break/list `party.size`, `s17`/`a17`),
P3-7 (Private-scope/client-status selectors), P3-8 (`a15`/`a16`), P3-9 (`r04`/`r10` на
полном READY), P3-10 (новый stale payload в `r15`), P3-11 (doc-comment `r09`),
P3-12 (`MD5SUMS.txt`, переименование attempt-лога, единые формулировки), P3-13
(подтверждение TZ r09 + journal note), P3-14 (убран лишний публичный API
`presence_keys`; helper `has_displayable_content` — `pub(crate)`).

Остаточных P3 по раунду 1 нет. Раунд 2 — после синхронизации docs/journal.

## 5. Мутации

`waves/RW-18/MUTATIONS.md`: **13/13 caught**, exit 101, в каждом логе падает ожидаемый
тест. Покрыты: scope selection, offline cleanup, activity priority, member-list
application, chunk default guild, offline+hidden visibility, multiple-playing, VR/mobile
guards, displayable tie-break, stale payload, `guild_id=0`, пустой member-status.
Исходники восстановлены; md5 до/после совпадают, `md5sum -c MD5SUMS.txt` — все OK.

## 6. Финальные проверки

| Команда | Результат |
|---|---|
| `cargo test --offline -p discord-core --test presence_types --test presence_reducer --test presence_selectors` | **54 passed**, exit 0 (`targeted-54.log`) |
| `cargo test --offline -p discord-core` | **473 passed**, exit 0 (`core-tests-final.log`) |
| `cargo test --offline --workspace` | **515 passed**, exit 0 (`full-tests-final2.log`) |
| `cargo fmt --all -- --check` | exit 0 (`fmt-final.log`) |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0 (`clippy-final.log`) |
| `npm run gate:worker` | **passed**: typecheck, meta 25, unit 169, smoke 20 (`gate-worker.log`, `gate-worker.json`) |

Машина: 16 vCPU; Rust-тесты и gate прогонялись отдельно, gate не покрывает Rust.

## 7. Файлы

Новые: `src/state/presence.rs`, `presence_reducer.rs`, `presence_selectors.rs`;
тесты `tests/presence_{types,reducer,selectors}.rs`. Изменены: `state/types.rs`,
`state/reducer.rs`, `state/member_list.rs`, `state/member_chunk.rs`, `state/mod.rs`,
`src/lib.rs`; контрактные правки в `tests/state_types.rs`, `tests/state_reducer.rs`,
`tests/member_chunk_types.rs`. Все они в `MD5SUMS.txt`.

## 8. Риски / ограничения

- `GUILD_MEMBERS_CHUNK.presences[]` форма не подтверждена: принят только
  `PRESENCE_UPDATE`-подобный элемент; факт маркируется `presences:unconfirmed-shape`;
  wire-запрос не изобретался.
- `hidden_activities` не дедуплицируются (порядок как на проводе).
- Нет TTL/лимита presence; карточка memory-only, live capture не использовался.
- `is_offline` = отсутствие или `PresenceStatus::Offline`; `Invisible`/`Unknown` не
  считаются offline (зафиксировано `s18`).
- GUI/SQLite/сеть/исходящие presence-запросы остаются внешне заблокированы; P2-03 —
  только после разрешения владельца.
