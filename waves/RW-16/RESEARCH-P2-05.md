# RW-16 / P2-05 — research: `GUILD_MEMBERS_CHUNK`

Дата: 2026-10-20. Источники — только локальные артефакты; сеть и живой Discord не использовались.
Карточка: `PLAN.md` P2-05 «Чанки и догрузка», тест `T-DATA-07`.

## 1. Что подтверждено фикстурами

### `022-GUILD_MEMBERS_CHUNK.json` (чистый кадр)

```text
t=GUILD_MEMBERS_CHUNK, op=0, s=17
d keys: chunk_count, chunk_index, guild_id, members, not_found
guild_id=294064825540280335
chunk_index=0, chunk_count=1
members=4, presences отсутствует, nonce отсутствует
not_found=["707997415642365992"]
```

Участники (все id положительные, `roles` у трёх — `null` → пустой Vec):

| user.id | username | roles |
|---|---|---|
| `159985870458322944` | MEE6 | 2 |
| `465601078914711572` | lgreaper | 0 |
| `566215798759096322` | nomadk.g. | 0 |
| `875330208104853504` | marriaz | 0 |

Поля member: `avatar`, `banner`, `communication_disabled_until`, `deaf`, `flags`,
`joined_at`, `mute`, `nick`, `pending`, `premium_since`, `roles`, `user`.
Поля `user`: `avatar`, `avatar_decoration_data`, `bot`, `collectibles`, `discriminator`,
`display_name`, `display_name_styles`, `global_name`, `id`, `primary_guild`,
`public_flags`, `username`, `vad_colors`.

### `009-GUILD_MEMBERS_CHUNK.json` (legacy-артефакт)

```text
t=GUILD_MEMBERS_CHUNK, op=0, s=7
d keys: chunk_count, chunk_index, guild_id, members, not_found
guild_id=-1467272931325252608  <-- отрицательный, старый big-int декодер
chunk_index=0, chunk_count=1
members=1, presences отсутствует, nonce отсутствует
not_found отсутствует
member.user.id=-13390286217619177728  <-- тоже отрицательный
roles=["-2690948858511381248"]
```

Политика как у `005/006` P2-04: исходный кадр — `InvalidId` на `d.guild_id`; shape
проверяется на копии с исправленным **только** guild_id, отрицательный member-id даёт
`malformed` и не попадает в состояние.

## 2. Что подтверждено документами RE, но не телами фикстур

- `docs/re/gateway-protocol.md:208,252`: в живом capture было **11** `GUILD_MEMBERS_CHUNK`
  (32 участника), но тела не выгружены; в `by-type` попал один кадр (в `summary.json` —
  индекс 22, 3076 байт payload).
- `docs/re/gateway-schema.md:96`: один кадр, `d` 666 байт JSON; `not_found` в схеме не
  раскрыт отдельно.
- `docs/re/gateway-schema.md:531`: `nonce` **не наблюдался** в дампе; `presences` в
  `GUILD_MEMBERS_CHUNK` также не наблюдались.
- `PLAN.md` P2-05: «GUILD_MEMBERS_CHUNK, запросы пресенсов для видимого диапазона».
  Это требование плана, а не подтверждённый wire-контракт.

## 2a. Дополнительные тела из `waves/RW-01/review/oracle-fresh.txt`

Кроме `009/022`, в read-only журнале RW-01 есть ещё два тела GMC (строки 100/112):

| s | guild_id | chunk_count/index | members | not_found |
|---:|---|---|---:|---|
| 78 | `294064825540280335` | 1/0 | 0 | 4 id |
| 84 | `294064825540280335` | 1/0 | 0 | 2 id |

Оба — `members:null`, без `nonce`/`presences`; `not_found` — массив строк. Их id
`1015297857303105647`, `95101520944570368`, `685455441781587991`, `793141031326973962`
встречаются в READY/users или member list, поэтому `not_found` **не** означает «удалить
пользователя» и не является сигналом отсутствия в гильдии. Это только метаданные ответа.

Следствие для модели: два ответа одной гильдии с `(chunk_count=1, chunk_index=0, nonce=None)`
— нормальное явление, а не duplicate-ошибка. Ключ `(guild_id, None)` хранит **последний**
такой ответ; члены/пользователи при этом уже аддитивно слиты в `Guild.members`/`State.users`,
а `not_found` предыдущего ответа не обязан сохраняться. Это ограничение зафиксировано в
DESIGN §4/§6, а не выдаётся за корреляцию.

## 3. Что нельзя утверждать без capture

- нельзя утверждать форму `d.presences[]` (`user_id`/`status`/`client_status`/`activities`)
  и то, что сервер вообще присылает presences в chunk;
- нельзя утверждать wire-payload запроса участников (op8/op37/op14) и связь `nonce` с
  диапазоном; отправленные кадры в доступном capture замаскированы;
- нельзя утверждать, что `chunk_count` всегда 1 или что chunk-ы приходят по порядку;
- нельзя утверждать, что `not_found` — полный список отсутствующих участников гильдии.

Поэтому P2-05 реализует **память и декларации**, а не сетевой запрос: `Effect` — только
интерфейс, исполнение — в transport/P2-09, которого в этой волне нет.

## 4. Оракулы

```bash
jq -c '{guild_id:.d.guild_id,chunk_index:.d.chunk_index,chunk_count:.d.chunk_count,
        members:(.d.members|length),not_found:.d.not_found}' \
  fixtures/gateway/by-type/022-GUILD_MEMBERS_CHUNK.json

jq -c '.d.members[] | {id:.user.id,username:.user.username,roles:(.roles//[])}' \
  fixtures/gateway/by-type/022-GUILD_MEMBERS_CHUNK.json

jq -c '{guild_id:.d.guild_id,members:(.d.members|length),
        member_id:.d.members[0].user.id}' \
  fixtures/gateway/by-type/009-GUILD_MEMBERS_CHUNK.json
```

Ожидаемо: `022` — 4 участника, `chunk_count/index=1/0`, `not_found` длины 1; `009` —
отрицательные guild/user id, исходный кадр отвергается.

## 5. Границы P2-05

**Входит:** разбор подтверждённых полей chunk, память по `(guild_id, nonce)`, сборка
`chunk_index/chunk_count`, merge участников/пользователей, `not_found`, диагностика,
селекторы, чистый построитель `Effect::RequestPresence` для видимого диапазона списка P2-04.

**Не входит:** wire-запрос op8/op37/op14, форма `presences[]`, полная presence-модель
(P2-09), `PRESENCE_UPDATE`, SQLite/UI/сеть, live Discord, новые зависимости.

## 6. Риски

0. В capture четыре тела, все `chunk_count/index=1/0`, два из них — `members:null`; multi-chunk
   и `nonce` не наблюдались. Сборка `chunk_count>1` — синтетический контракт.
1. `nonce` отсутствует в обоих фикстурах; тип хранит `Option<String>`, но wire-корреляция
   не доказана. При отсутствии nonce ключ — `(guild_id, None)`.
2. `presences` в chunk не разбираются: при наличии поле помечается `ignored`, чтобы не
   терять данные молча; это осознанный P3 до P2-09.
3. `Guild.members`/`State.users` merge — аддитивный; удаление участников по chunk не
   выполняется, `not_found` только сохраняется.
4. Legacy `009` нельзя использовать как положительный shape-оракул без правки guild_id;
   отрицательный member-id остаётся `malformed`.

## 7. Итог реализации

- Red: compile-red 101, затем behavior-red `r12`/`r13` (101).
- Green: targeted 43/43 (types 19, reducer 13, selectors 11), core 378, workspace 420;
  fmt/clippy чисто.
- Ревью: round 1 — P2-1 (`duplicate-index` для `nonce=None/count>1`), закрыт red-first;
  round 2 — принято, P0/P1/P2=0.
- Gate: `.gate/worker-2026-10-20T13-21-59-680Z.json` passed; `accepted` — `waves/db.jsonl`.
