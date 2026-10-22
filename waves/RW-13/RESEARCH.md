# RW-13 / P2-02 — read-only research входов состояния

Дата: 2026-10-20. Все данные получены из курированных фикстур и `docs/re/`; живой Discord,
сеть и golden не использовались. Источник истины по форме провода — `docs/re/gateway-schema.md`,
`docs/re/gateway-protocol.md`; по уже принятой модели READY — `waves/RW-12/DESIGN-P2-01.md`.

## 1. Фикстуры и контрольные суммы

| Файл | md5 | Форма `d` | Ключей/длина |
|---|---|---|---|
| `fixtures/gateway/by-type/002-READY.json` | `a1a8fd5662be88108dd25ac619a377f2` | map | 40 |
| `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json` | `55983830547abaf8d1b122798fdcf568` | map | 7 |
| `fixtures/gateway/by-type/004-SESSIONS_REPLACE.json` | `e4eb9bb110e5f87ff7633126b7661b1d` | array | 1 |
| `fixtures/gateway/by-type/004-THREAD_LIST_SYNC.json` | `507d8c15a2031e5d93b1995382ade391` | map | 3 |
| `fixtures/gateway/by-type/012-CHANNEL_INFO.json` | `0b5b19541f9dbbab52a3cf947335e452` | map | 2 |
| `fixtures/gateway/by-type/023-MESSAGE_CREATE.json` | `d82d96cada5678de01bc18ab5e967430` | map | 19 |
| `fixtures/gateway/by-type/026-MESSAGE_UPDATE.json` | `7bbf2592c7c41fbbc79940286cd01a85` | map | 19 |

`RESUMED` в `by-type/` отсутствует: в захвате он встречается 3 раза
(`docs/re/gateway-protocol.md` §4), но отдельного курированного кадра нет. Поэтому payload
`RESUMED` в P2-02 не домысливается: принимается отсутствующий/`nil`/map `d`, состояние не
меняется, обновляется только `last_seq`.

## 2. READY — снимок

`jq` по `002-READY.json`:

```text
guilds=7, channels=272, roles=132, emojis=363, stickers=51,
users=146, private_channels=17, relationships=60,
read_state.entries=157, read_state.version=128949,
user_guild_settings.entries=58, user_guild_settings.version=734,
sessions=1, merged_members=[1,1,1,1,1,1,1],
member_count=[157,112,59,73,44749,24228,3]
```

Гильдии (порядок `READY`): `274093052673392641`, `294064825540280335`,
`338001249918255106`, `700482297027231765`, `942355007271997450`,
`1014537851418980372`, `1340572775974305803`. Все `lazy=true`; крупные — `XOS` (44 749) и
`ITSKIDON PC BOOST` (24 228). `private_channels` — только type 1. `users` содержит
собственного пользователя `384299970632876032`.

## 3. READY_SUPPLEMENTAL — снимок участников

`jq`:

```text
guilds=7, merged_members.groups=7, per_group=[17,14,7,2,10,87,1],
merged_raw_total=138, merged_unique_total=137, voice_states=1, merged_presences.guilds=7,
merged_presences.friends=2, lazy_private_channels=0
```

- `merged_members[i]` привязан к сырому индексу `d.guilds[i]` (контракт P2-01).
  В первой группе 17 сырых записей, но `user_id=289091818442719242` встречается дважды;
  принятый P2-01 dedup (last wins) даёт **16** уникальных, всего 137, а не 138. Это
  проверено `jq group_by` и тестом `state_types`; состояние хранит уже разобранные группы.
- `d.guilds[]` содержит только `activity_instances`, `embedded_activities`, `id`,
  `voice_states`; `members`/`presences` внутри гильдии отсутствуют.
- `voice_states` (1 запись) — P2-10; `merged_presences` (2 friends + 7 групп) — P2-09.
  P2-02 их не разбирает и не превращает в состояние.
- `lazy_private_channels` в этой фикстуре пуст; отдельного контракта на слияние ЛС из
  supplemental нет — P2-02 не додумывает.

## 4. SESSIONS_REPLACE

`d` — **массив**, не объект (`gateway-schema.md` §3). Один элемент:

```text
session_id="REDACTED" (8 символов в курированной фикстуре),
status="invisible", client_info={client:"desktop", os:"linux", version:0},
processed_at_timestamp=0
```

Событие заменяет список сессий целиком, а не сливается по id. `Session::Debug` уже
редактирует `session_id` (P2-01).

## 5. THREAD_LIST_SYNC

`d` = `{guild_id, most_recent_messages, threads}`; в фикстуре `threads=1`,
`most_recent_messages=null`. Элемент треда:

```text
id, guild_id, parent_id, name, type=11, owner_id, last_message_id,
message_count=5, member_count=0, total_message_sent=10,
rate_limit_per_user=0, flags=2, member_ids_preview=null,
thread_metadata={archive_timestamp, archived:"false", auto_archive_duration=60,
                 create_timestamp, invitable:"true", locked:"false"}
```

**Важно:** в курированной фикстуре `guild_id`, `id`, `parent_id`, `owner_id` и
`last_message_id` — отрицательные строки (санитайзинг). Контракт P2-01 считает такие
идентификаторы невалидными. Поэтому реальная фикстура используется для проверки
диагностики/отказа, а валидный путь — на синтетическом событии с теми же значениями полей,
но положительными id. Неподтверждённое поведение (удаление отсутствующих тредов,
`most_recent_messages`) не реализуется: P2-02 делает upsert тредов, а `most_recent_messages`
оставляет P3-07.

## 6. CHANNEL_INFO

`d` = `{guild_id, channels}`; в курированной фикстуре 75 каналов, но у каждого элемента
оставлен **только `id`**, и все id отрицательные. Это не позволяет проверить поля канала.
Валидный путь проверяется на объекте канала из `READY`:

```text
id=751425447614480414, type=0, name="___hidden___", parent_id=null,
position=24, flags=131072, nsfw=false, topic=null, last_message_id=null,
permission_overwrites=[{id:274093052673392641,type:0,allow:"0",deny:"1024"}],
rate_limit_per_user=0, bitrate=null, user_limit=null, rtc_region=null,
video_quality_mode=null, default_auto_archive_duration=null
```

P2-02 принимает `CHANNEL_INFO` как upsert каналов; при неизвестной гильдии создаётся
минимальная заглушка гильдии с диагностикой, чтобы канал не терялся. Это внутреннее
решение, не утверждение о Discord API.

## 7. MESSAGE_CREATE / MESSAGE_UPDATE

В захвате только эти два типа; `MESSAGE_DELETE`/`MESSAGE_DELETE_BULK` в 21 наблюдаемом
типе **нет** — P2-02 их не выдумывает. У `023` и `026` одинаковый набор 19 ключей.

`023-MESSAGE_CREATE`:

```text
id=1561627527296979046, channel_id=1163408591315812392,
guild_id=1014537851418980372, author.id=268478587651358721 (bot=true),
content length=208, timestamp=2026-10-19T06:30:16.144000+00:00,
edited_timestamp=null, type=0, flags=0, pinned=false, tts=false,
mention_everyone=false, mentions=null, mention_roles=null,
attachments=null, embeds=null
```

`026-MESSAGE_UPDATE` — тот же id/канал/автор, `content` длиной 208 (тот же текст, не
пустой), `embeds` — массив из 1 элемента, `mentions=null`, `edited_timestamp=null`.

`docs/re/gateway-schema.md` §5 фиксирует: `mentions` — массив объектов пользователей либо
`null`; `attachments` в дампе всегда `null`; `embeds` — `null` или массив. P2-02 хранит
счётчики вложений/embeds, id упомянутых пользователей/ролей и флаги, но не декодирует
embed-контент (P3-02) и не вложения (P3-04).

## 8. Что P2-02 явно не делает

- Не исполняет эффекты: сеть/REST/op37 — следующие карточки.
- Не читает SQLite (P2-03 нет зависимости), не пишет на диск.
- Не трогает UI/Tauri.
- Не разбирает `PRESENCE_UPDATE`, `GUILD_MEMBER_LIST_UPDATE`, voice, typing, reactions,
  settings-proto — отдельные карточки.
- Не вводит новых зависимостей.
- Не строит догадок о `MESSAGE_DELETE`/`GUILD_CREATE`, которых нет в наблюдаемых 21 типах.

## 9. Вывод для дизайна

P2-02 — чистый детерминированный reducer над уже принятыми типами READY. Достаточно:
`BTreeMap`-индексов, upsert-слияния снимков, диагностик, деклараций эффектов и селекторов.
Оракулы T-DATA-04 — числа `jq` выше и реальные `MESSAGE_CREATE`/`MESSAGE_UPDATE`; для
санитайзенных id — синтетика с теми же значениями полей.
