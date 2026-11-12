# RW-18 / P2-09 — research: presence

Дата: 2026-10-20. Только чтение: `fixtures/`, `docs/re/`, снимок веб-клиента
`reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` и
`reference-oracle/pretty/mod-595528.pretty.js`. Живой Discord не использовался.

## 1. Подтверждённый capture

### `PRESENCE_UPDATE` (29 событий)

Файлы: `fixtures/gateway/by-type/{010,012,013,014,016,017,018,025,026,036,043,053,058,059,060,061,062,065,069,076,077,078,082,083,085,087,088,090,092}-PRESENCE_UPDATE.json`.

- top-level `d`: `activities` 29, `client_status` 29, `processed_at_timestamp` 29,
  `status` 29, `user` 29; `guild_id` 25; `hidden_activities` 4; `restricted_application` 4.
- `status`: `online` 15, `idle` 13, `dnd` 1. `offline` в capture не встречается.
- `client_status`: `desktop` 17, `mobile` 12, `vr` 7 (ключи не взаимоисключающие).
- `activities`: 6 объектов — type `0` (playing) 3, type `4` (custom) 3.
- поля activity в этих 6: `content_classification`, `created_at`, `id`, `name`,
  `official_game_id`, `type`; optional `timestamps.start`, `state`, `emoji.name`,
  `session_id` (в фикстуре уже `REDACTED`).
- `user`: 12 полных объектов, 17 минимальных `{id}`. У полных есть `username`,
  `global_name`, `discriminator`, `avatar`, `bot`, `public_flags` и др.
- `processed_at_timestamp`: `0` (int) либо десятичная строка (в т.ч. большая).
  `since`/`broadcast` в фикстурах `null`; `hidden_activities`/`restricted_application`
  `null` или `"nil"`.

Ключевые примеры:

| файл | seq | user | guild | status | client | activities |
|---|---:|---|---|---|---|---|
| `010` | 7 | `95101520944570368` | `1014537851418980372` | online | desktop online | type 0 `bg-capture`, start `1792391357883` |
| `012` | 9 | `275258435199827968` | — | idle | desktop idle | null, полный user |
| `018` | 13 | `702520807447855184` | `1014537851418980372` | online | mobile online, vr idle | null, processed `0` |
| `043` | 35 | `531356125954834463` | `1014537851418980372` | dnd | desktop dnd | type 4 Custom Status, emoji 🇷🇺, state |
| `076`/`077`/`078` | 59/60/61 | `572843509363638282` | три разные гильдии | online | mobile online | null, processed `0` |

### `READY_SUPPLEMENTAL.merged_presences`

`fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json`:

- `merged_presences.friends` = 2; элементы имеют `user_id`, `status`, `client_status`,
  `activities`, `hidden_activities`, `processed_at_timestamp`, `restricted_application`
  (нет вложенного `user`).
- `merged_presences.guilds` = 7 массивов, выровнены по `d.guilds[]`:
  `[4, 4, 3, null, 1, 14, null]`; всего 26 guild-снимков.
- Итого 28 scope-записей, 18 уникальных пользователей; 7 пользователей имеют >1 scope.
- `status`: online 15, idle 8, dnd 5; activities: type 0 — 5, type 4 — 4.
- `processed_at_timestamp`: 22 строки и 6 int; friends имеют большие строки, guild-снимки
  в основном `0`.
- Все `user_id` присутствуют в `READY.users` (146 пользователей); вложенного `user` нет.
- `READY` (`002`) **не содержит** `presences`; `d.guilds[].presences` отсутствует.

### `GUILD_MEMBER_LIST_UPDATE.member.presence`

Во всех 79 событиях, у member-элементов 275 полных presence-снимков (не только 209:
`item` в `INSERT`/`UPDATE` тоже учитывается). Ключи: `activities` 275, `client_status` 275,
`processed_at_timestamp` 275, `status` 275, `user` 275, `game` 65 (legacy, не моделируется).

- status: online 92, offline 84, idle 59, dnd 40.
- client keys: `desktop`, `web`, `mobile`, `embedded`, `vr`.
- activities: type 4 — 47, type 0 — 40, type 2 — 4.
- activity fields (91 объектов): `content_classification` 91, `created_at` 91, `id` 91,
  `name` 91, `official_game_id` 91, `type` 91, `state` 63, `session_id` 59,
  `application_id` 35, `assets` 25, `timestamps` 24, `details` 24, `platform` 23,
  `emoji` 20, `flags` 16, `party` 15, `parent_application_id` 12,
  `supported_platforms` 9, `buttons` 5, `status_display_type` 5, `sync_id` 1.
- `application_id`/`parent_application_id` — строки, часто **отрицательные**
  (например `-1236281867793082368`), поэтому это не `u64`.
- `party.size` — **двухбайтовая строка** (`\x01\x03` = current 1, max 3;
  `\x03\x00` max 0), не список.
- `assets`: `large_image`/`large_text`/`small_text`; `emoji`: `name` (иногда без id);
  `buttons`: список строк; `supported_platforms`: список строк.

### `GUILD_MEMBERS_CHUNK`

`009`/`022` не содержат ключа `presences`; форма `presences[]` не подтверждена.
Веб-клиент отправляет `GUILD_MEMBERS_CHUNK` в тот же presence-batch, что и
`PRESENCE_UPDATE` (`mod-595528.pretty.js:282-283`), но полезная нагрузка элемента
офлайн не наблюдалась.

## 2. Официальный веб-клиент (read-only oracle)

### Разбор gateway

`pretty/mod-595528.pretty.js`:

- `eX(e,t)` → `{user, status, clientStatus, activities, hiddenActivities, guildId,
  processedAtTimestamp}`; `guild_id` передаётся из события, а `PRESENCE_UPDATE`
  обрабатывается через `eV(["PRESENCE_UPDATE"], e => eq(eX(e, e.guild_id)))`.
- `READY_SUPPLEMENTAL` раскладывает `merged_presences.guilds` по индексу гильдии,
  friends — отдельно; `null`-группа даёт `[]`.
- `GUILD_MEMBER_LIST_UPDATE` для SYNC/INSERT/UPDATE с `member.presence` отправляет
  полный presence в store; DELETE — нет.
- `GUILD_MEMBERS_CHUNK` попадает в batch `PRESENCE_UPDATES`.

### PresenceStore (`out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`, module 290863)

- Внутренняя карта `presencesForGuilds[userId][guildId]`; `ME` — приватный scope.
- Live update: offline без hidden activities и без существующей записи игнорируется;
  offline записывается в scope; агрегат пересобирается. Если все scope offline и
  hidden пусты — пользователь удаляется.
- Агрегат: выбрать scope с максимальным `processedAtTimestamp`; при равенстве — с
  большим числом activities. Если выбранный offline, но не все — статус не понижается.
- `getStatus(user, guild?)` по умолчанию `offline`; `getActivities` — `[]`;
  `getPrimaryActivity` — первый не-HANG_STATUS; `isMobileOnline` — mobile online,
  desktop/vr не online; `isVROnline` — vr online.
- Сортировка activities (`C`/`O`): приоритет custom 4, competing 3, streaming 2,
  playing 1, остальные 0; затем «displayable» (не custom и есть details/assets/party/state);
  затем `created_at` по убыванию. Несколько playing сворачиваются в один (самый свежий).
- `hiddenActivities` дедуплицируются по `application_id:party.id` (в этой волне
  сохраняем без дедупа, чтобы не изобретать порядок).
- `session_id`, `secrets`, `sync_id` не нужны для модели присутствия и не сохраняются.

## 3. Границы и неизвестное

- `GUILD_MEMBERS_CHUNK.presences[]` — форма не подтверждена. Принимаем только элементы,
  совпадающие по форме с `PRESENCE_UPDATE`; иначе — диагностика/пропуск. Wire-запрос
  presence не изобретается (`Effect::RequestPresence` остаётся интерфейсом).
- `restricted_application`, `content_classification`, `official_game_id`, `game` —
  не моделируются; это не ошибка.
- Нет TTL/лимита: presence — память процесса; offline убирает запись.
- `invisible` в capture не встречается, но статус сохраняется как известный variant.
- `offline` в `PRESENCE_UPDATE` не наблюдался; поведение берём из официального store.

## 4. Оракулы для тестов

- `010` → user `95101520944570368`, guild `1014537851418980372`, online, desktop online,
  1 activity `bg-capture`, start `1792391357883`, processed `1792391357994`.
- `043` → dnd, custom type 4, name `Custom Status`, state `f1an#9271 | f_1an`, emoji name 🇷🇺.
- `018` → mobile online + vr idle, processed 0.
- `003` → 28 scope, 18 users, friends 2, guild counts `[4,4,3,null,1,14,null]`.
- `008` (после READY+003) → 34 scope, 24 users; member `95101520944570368` activity
  `Wife's Bedroom`, start `1792390727877`.
- `069`/`097` → реальные `buttons`, `party.size` bytes, отрицательные application id.
