# Отчёт RW-15 / P2-04 — memory-only member list

## 1. Сделано

- **W15a** — `src/state/member_list.rs`: `MemberListId` (raw string), `MemberListRange`,
  `MemberListGroup`, `MemberPresence`, `MemberListEntry`, `MemberListOp`, `MemberListUpdate`,
  `MemberList`; разбор `GUILD_MEMBER_LIST_UPDATE` (SYNC/INSERT/UPDATE/DELETE), groups/counts,
  range как 2 байта, member payload с `user.id`, минимальный presence.
- **W15b** — reducer: список по `(guild_id, raw list_id)`, ops по порядку, сдвиги
  INSERT/DELETE, UPDATE без сдвига, SYNC-reset, snapshot groups, counts, duplicate/
  out-of-range диагностика, неизвестная гильдия, no cross-state side effects.
- **W15c** — `member_list_selectors.rs`: list/items/member_at/index_of/group/range/len/
  is_loaded/count.
- **Координатор** — аддитивные правки `state/types.rs`, `state/reducer.rs`, `state/mod.rs`,
  `ready/social.rs` (`parse_member_fields`), root re-exports, тесты, red/green/gate.

## 2. Scope и op37

Реализована подтверждённая capture часть P2-04. Wire-формат op37/op14 **не подтверждён**:
исходящие кадры замаскированы, docs противоречат (`op14` vs `op37`), `[[0,99]]` — требование
карточки, а не payload. Поэтому wire не изобретался; интерфейсом подписки остаётся
`Effect::SubscribeMembers` (P2-02, READY уже даёт 7 эффектов с диапазоном 0..=99). Это
blocked-item для отдельной волны/решения владельца, как memory-only P2-06.

`005/006` — legacy big-int: исходный отрицательный `guild_id` отвергается; shape проверяется
на копии с исправленным в памяти guild_id, отрицательные member-id дают `malformed`.

## 3. Red

- `cargo test --offline -p discord-core member_list` → `red-w15.log`, **exit 101**:
  compile-red, модуля `state::member_list` ещё не было. Поведенческого red до API нет;
  это зафиксировано в `TEST-CORRECTIONS.md`.
- Поправки тестовой обвязки после первого green (helper `list`, порядок `s08`, ожидания
  сдвига/диагностик) — там же; они не меняют контракт.

## 4. Green

- targeted: `member_list_types` 14 + `member_list_reducer` 17 + `member_list_selectors` 9 =
  **40/40** (`green-w15-p3c.log`).
- `cargo test --offline -p discord-core` — **335 passed / 0 failed** (`green-core-p3c.log`).
- `cargo test --offline --workspace` — **377 passed / 0 failed**
  (`green-workspace-p3c.log`).
- `cargo fmt --all -- --check` — exit 0.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — exit 0
  (`clippy-final.log`).
- `npm run gate:worker` — **passed**, `.gate/worker-2026-10-20T12-43-25-346Z.json`
  (`gate-final.log`).

## 5. Ревью

Независимый read-only ревьюер: `ses_ea6c7d4fbffez3zm8hH72BOCiS`. Первый раунд: P0=0,P1=0,
P2=1 (duplicate при INSERT в позицию существующего id); исправлено red-first
(`red-w15-p21.log` → `green-w15-p21.log`), P3 закрыты; повторное ревью — **«принято»**,
P0/P1/P2=0, P3=4 backlog (`waves/reviews/review-p2-04.md`). P3-1/2/3 (same-guild selectors,
m02 naming, full unknown-op prefix) закрыты test-only после приёмки: `green-w15-p3c.log`,
targeted 40/40, core 335, workspace 377; product md5 не менялся.

## 6. Файлы

- `rust/crates/discord-core/src/state/member_list.rs`
- `rust/crates/discord-core/src/state/member_list_selectors.rs`
- `rust/crates/discord-core/src/state/{types,reducer,mod}.rs`
- `rust/crates/discord-core/src/ready/social.rs`
- `rust/crates/discord-core/src/lib.rs`
- `rust/crates/discord-core/tests/member_list_{types,reducer,selectors}.rs`
- `waves/RW-15/*`, `waves/reviews/review-p2-04.md`

## 7. P2-1/P3 после ревью

- P2-1: `INSERT` теперь проверяет все существующие элементы до вставки; regression
  `m15_insert_duplicate_at_same_position_is_diagnostic`.
- P3: unknown-op диагностика в `inconsistencies`; in-range UPDATE; same-guild два списка;
  битые groups/count; header `id`/`guild_id`; single malformed; пустой `client_status`;
  out-of-range INSERT. Все — в `green-w15-p3b.log`.

## 8. Риски

- `groups[].count`/`member_count`/`online_count` — сырые, не связаны инвариантом; UI не должен
  выводить одно из другого.
- `MemberPresence` минимален (status/client_status); activities — P2-09.
- `State.users` не пополняется member list; list-local snapshot, merge — отдельное решение.
- Нет TTL/лимита на число списков; окно задаётся сервером (~100), память ограничена окном.
- op37/op14 wire, chunk-догрузка (P2-05), удаление/переподписка — вне волны.
- 005/006 нельзя использовать как источник положительных ID; только shape/ops.
