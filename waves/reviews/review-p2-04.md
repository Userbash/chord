# Независимое ревью RW-15 / P2-04 — итог

Дата: 2026-10-20. Ревьюер: отдельный read-only агент (не автор кода).
Контракт: `waves/RW-15/DESIGN-P2-04.md`, `TZ-P2-04.md`; scope — memory-only
`GUILD_MEMBER_LIST_UPDATE`; wire op37/op14 не подтверждён capture и не добавлялся.

## Вердикт: **принято**

P0 = 0, P1 = 0, **P2 = 0**. P3 — backlog, не блокирует.

## Первый раунд

- P2-1: `INSERT` в позицию существующего member-id не писал `duplicate`, потому что
  проверка исключала `position == index`; после сдвига дубликат оставался.
- Закрыто red-first: `red-w15-p21.log` exit 101 (`duplicates=[]`, items `[42, 42]`) →
  проверка по всем существующим элементам → `green-w15-p21.log` exit 0.
  Регрессия: `member_list_reducer::m15_insert_duplicate_at_same_position_is_diagnostic`.
- P3 (формат unknown-op, in-range UPDATE, same-guild тест, header-кейсы, single malformed,
  пустой `client_status`, out-of-range INSERT) закрыты; см. `TEST-CORRECTIONS.md`.

## Повторный раунд

Проверены:
- P2-1 по существу: `member_list.rs` перед `INSERT` сканирует все элементы; независимый
  пробник даёт `duplicate` для `SYNC [42]` + `INSERT 0 [42]`;
- `unknown-op` → `inconsistencies` с полным префиксом
  `member_list:{guild}:{list}:unknown-op:{op}`;
- пустой `id` → `WrongType { path: "d.id" }`; `m07`;
- single `malformed` для битого item; `m14`;
- пустые `client_status` key/value → `ignored`; `m13`;
- битые groups/overflow count, out-of-range INSERT, успешный in-range UPDATE, два list_id
  одной гильдии (`m16`);
- отсутствие паник/`unsafe` в product-коде member list; cross-state side effects отсутствуют;
- stale-политика P2-02 применяется до разбора;
- 005/006: исходные кадры отвергаются `InvalidId`; shape — на копии с исправленным
  `guild_id`; 98/1 отрицательных member-id → `malformed`.

## Прогоны

| Проверка | Результат |
|---|---|
| targeted member list | **39 passed** (types 14, reducer 17, selectors 8), exit 0 |
| `cargo test --offline -p discord-core` | **334 passed, 0 failed**, exit 0 |
| `cargo test --offline --workspace` | **376 passed, 0 failed**, exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0 |
| `npm run gate:worker` | **passed**, `.gate/worker-2026-10-20T12-31-13-208Z.json` |

## P3 (backlog, не блокирует)

1. ~~`member_list_selectors::s06` называется «two lists of one guild», но применяет 008
   и 021 (разные гильдии); same-guild был только в reducer.~~ Закрыто test-only:
   `s06` переименован, добавлен `s09_two_lists_of_same_guild_are_independent`.
2. ~~Комментарий `member_list_reducer::m02` говорит про одну гильдию, хотя фикстуры разных
   гильдий.~~ Закрыто: `m02` переименован, same-guild — `m16`.
3. ~~`member_list_types::m09` проверяет не полный префикс unknown-op.~~ Закрыто: проверяется
   `member_list:{guild}:{list}:unknown-op:FROBNICATE`.
4. `waves/RW-15/tests-red/` пуст: red был compile-red и задокументирован в
   `TEST-CORRECTIONS.md`; процессное замечание.

P3-1…3 закрыты после приёмки как test-only (`green-w15-p3c.log`): targeted 40/40, core 335,
workspace 377; product md5 не менялся. P3-4 остаётся процессным backlog.

## Scope

Принята подтверждённая capture часть. Wire-формат op37/op14 не изобретался; подписка остаётся
`Effect::SubscribeMembers` (P2-02). `State.users`/`Guild.member_count` member list не меняет;
presence минимальный, activities — P2-09; `GUILD_MEMBERS_CHUNK` — P2-05.
