# RW-16 / P2-05 — отчёт

## 1. Сделано

Реализован memory-only `GUILD_MEMBERS_CHUNK`:

- разбор подтверждённых полей `guild_id`, `chunk_index`, `chunk_count`, `members`,
  `not_found`; optional `nonce`; `presences[]` не изобретается (P2-09), при наличии —
  `ignored`;
- сборка по `(guild_id, Option<nonce>)`, `chunk_count` first-wins, duplicate index last-wins,
  out-of-order, `is_complete`;
- `duplicate-index` диагностируется для `nonce=Some` и `nonce=None/count>1`; единственное
  исключение — независимый повтор `nonce=None/count=1` (capture s=78/s=84), закрыто P2-1
  red-first;
- аддитивный merge в `Guild.members` и `State.users`, `not_found` без удаления,
  `Guild.member_count`/`member_lists`/messages не меняются;
- диагностика: malformed members, ignored not_found/nonce/members/presences, duplicates
  member/index, inconsistencies unknown guild/count;
- селекторы сборок/участников/not_found и чистый
  `member_list_presence_effect(...) -> Option<Effect::RequestPresence>` для видимого
  диапазона P2-04;
- legacy `009`: исходный кадр `InvalidId`, corrected copy с отрицательным member-id —
  `malformed`; wire-запрос участников не добавлялся.

## 2. Red

- `waves/RW-16/red-w16.log`, exit **101**, compile-red (26 ошибок отсутствующих API).
- `red-w16-r12.log`, exit **101** — повтор `nonce=None/count=1` ошибочно давал
  `duplicate-index`.
- `red-w16-r13.log`, exit **101** — P2-1 ревью: `nonce=None/count=2` не давал
  `duplicate-index`.
- Поправки тестовой обвязки: `s08` (индекс flat-list / группы) и clippy-allow тестового
  конструктора; подробности — `TEST-CORRECTIONS.md`.

## 3. Green

- targeted: `member_chunk_types` 19 + `member_chunk_reducer` 13 + `member_chunk_selectors` 11
  = **43/43** (`green-w16-final.log`).
- `cargo test --offline -p discord-core` — **378 passed / 0 failed** (`green-core-w16.log`).
- `cargo test --offline --workspace` — **420 passed / 0 failed**
  (`green-workspace-w16.log`).
- `cargo fmt --all -- --check` — exit 0 (`fmt-w16.log`).
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — exit 0
  (`clippy-w16.log`).
- `npm run gate:worker` — **passed**, `.gate/worker-2026-10-20T13-21-59-680Z.json`
  (`gate-post-journal.log`, exit 0).

## 4. Файлы

- `src/state/member_chunk.rs`
- `src/state/member_chunk_selectors.rs`
- `src/state/types.rs` (поле/вариант/Debug/route)
- `src/state/reducer.rs` (ветка `GuildMembersChunk`)
- `src/state/mod.rs`, `src/lib.rs` (re-exports)
- `tests/member_chunk_{types,reducer,selectors}.rs`
- `waves/RW-16/` (RESEARCH/DESIGN/TZ/TEST-CORRECTIONS/тесты/логи)

## 5. Ревью

Первое независимое read-only ревью (`ses_ea69ef126ffe1uhFnPcOM0Xrer`): **не принято**,
P0=0, P1=0, P2=1 — `duplicate-index` подавлялся для любого `nonce=None`, включая
`chunk_count>1`. P2-1 закрыт red-first (`r13`, exit 101), правило сужено до
`nonce.is_some() || count > 1`; DESIGN §4.4/§6.4 и TZ синхронизированы. P3-1/2 (устаревшие
формулировки, misleading green log) закрыты; P3-3 частично закрыт тестами `c17`–`c19`,
`r12`, `r13`; P3-4 gate — passed; P3-5 — DESIGN/TZ обновлены.

Повторное ревью: **принято**, P0=0, P1=0, P2=0; P3 — только трассируемость
(`waves/reviews/review-p2-05.md`).


## 7. Мутации

Проведены 3 мутации `member_chunk.rs`; каждая роняла targeted-тесты (exit 101):

| Мутация | Поймавший тест |
|---|---|
| `is_complete()` → всегда `true` | `r03` |
| duplicate-условие → только `nonce.is_some()` | `r13` |
| merge → push без замены существующего member | `r10` |

Продукт восстановлен из резервной копии, md5 совпадает с `SOURCE-MD5SUMS.txt`; после
восстановления targeted — **43/43** (`green-w16-post-mutation.log`). Детали:
`mutations/README.md`, `mutation-complete.*`, `mutation-duplicate.*`, `mutation-merge.*`.

## 6. Риски / P3

- `nonce` не подтверждён capture; wire-корреляция и op8/op37/op14 отсутствуют намеренно.
- `presences[]` в chunk не разбирается до P2-09; поле помечается `ignored`.
- Merge аддитивный: `not_found`/повторный chunk не удаляют ранее слитых участников.
- `Guild.member_count` — сырое значение READY, не пересчитывается.
- Cap/prune сборок не введён; повтор `nonce=None/count=1` заменяет снимок chunk-а
  (участники/пользователи уже слиты).
- P3-3: нет отдельного regression на `nonce=Some/count>1` duplicate; ветки покрыты `r04`
  (`Some/count1`) и `r13` (`None/count2`), независимый probe ревьюера — ok.
- P3-2: red-лог `r13` снят до двух last-wins assert-ов; причина падения та же, сдвиг строк
  отмечен здесь.
