# RW-15 / P2-04 — research: ленивые списки участников

Источник: `docs/rust-client/PLAN.md:72-73`, `TEST-PLAN.md:63,88`, `FEATURES.md:17,66`,
фикстуры `fixtures/gateway/by-type/*GUILD_MEMBER_LIST_UPDATE.json`, `docs/re/gateway-protocol.*`.
Read-only; код и golden не менялись.

## 1. Подтверждено capture

- 70 файлов `*-GUILD_MEMBER_LIST_UPDATE.json`; union-ключи `d`:
  `groups`, `guild_id`, `id`, `member_count`, `online_count`, `ops`.
- `d.id`: `"everyone"` (52) и строковый `"1659606647"` (18) — **хранить строкой**, не `u64`.
- ops: 3 `SYNC`, 53 `UPDATE`, 14 `INSERT`, 13 `DELETE`.
  - `SYNC`: `{items, op, range}`; `range` — 2 байта `"\u0000c"` = `[0, 99]`, не массив;
  - `INSERT`/`UPDATE`: `{index, item, op}`; `DELETE`: `{index, op}`;
  - `items` — плоский список `group|member`; `group` — `{id}`, `member` — объект с `user`,
    `roles`, `presence` и полями участника.
- `groups[].count` — сырое число, не «загружено»; сумма groups не равна `member_count`;
  `online_count` не равен `online`-группе. Ничего не выводить одно из другого.
- `members` внутри `d` отсутствует во всех 70 — не поддерживать.
- `range [0,99]` — окно плоского списка, включая group-заголовки; 100 items ≠ 100 members.

## 2. Оракулы и дефект старых фикстур

- Чистые положительные ID: `008`, `021`, `034`, `045`, `075`, `079`; они — основные
  state-оракулы.
- `005`/`006` — старый big-int декод: `guild_id=-1467272931325252608`, 98/1 отрицательных
  member-id. `docs/re/gateway-protocol.json.corrections` подтверждает, что корректный id —
  `1014537851418980372`, а отрицательный — `altParserValue`. Исходные файлы **отвергаются**
  на `guild_id`; shape/ops проверяются на копии с исправленным в памяти guild_id, а
  отрицательные member-id обязаны дать диагностику и не попасть в типизированный список.
- `008` — `guild_id=1014537851418980372`, list `1659606647`, 17 items, 2 groups.
- `021` — `guild_id=294064825540280335`, 100 items, 4 groups, counts 112/13, range `[0,99]`.
- `034` — INSERT index 1 (member, list `1659606647`).
- `045` — DELETE 11 + INSERT 17 (`everyone`).
- `075` — DELETE 11 (list `1659606647`).
- `079` — DELETE 99, INSERT 0 member, DELETE 99, INSERT 0 group → 5 groups.

## 3. op37/op14 — не подтверждено

- В коде нет op37: `gateway.rs` знает только 0,1,7,9,10,11; `identify.rs` — op2; `ws.rs` —
  IDENTIFY/RESUME. Исходящие кадры capture замаскированы, payload op37 отсутствует.
- `gateway-protocol.md:248-249` говорит про **op14**-подписки, PLAN/TASKS — про **op37**.
  `[[0,99]]` — требование карточки/R1, а не проверенный payload.
- Решение RW-15: не изобретать wire-формат. Подписка остаётся абстракцией
  `Effect::SubscribeMembers { guild_id, ranges }` (P2-02, 7 эффектов READY уже приняты).
  `T-GW-09` в части wire — отдельный blocked-item до capture/решения владельца; в P2-04
  принимается подтверждённая часть (ops member list). Это то же сужение scope, что P2-06
  (memory-only без REST/SQLite).

## 4. Ограничения

- Presence в P2-04 — минимальный снимок (`status`, `client_status`); activities/полная модель —
  P2-09.
- `GUILD_MEMBERS_CHUNK` — P2-05, не смешивать.
- `Guild.member_count` не обновлять из member list: counts живут в `MemberList`.
- `State.users` не наполнять из member list в этой волне: list-local snapshot, иначе
  кросс-карточное влияние и риск с отрицательными ID.
- Неизвестная гильдия: список всё равно создаётся, но пишется `inconsistencies`; паники нет.
- Stale `s` — общая политика P2-02, no-op.

## 5. Офлайн-проверка

`cargo test --offline -p discord-core member_list` на реальных фикстурах через
`tests/support/ready_fixture.rs`; независимые числа — `jq` из §2. Никакой сети, live Discord,
новых зависимостей и записей в golden.
