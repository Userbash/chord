# NOTIFICATIONS-RE — настройки уведомлений, per-guild и per-channel overrides

Карточка: **RE-N05-NOTIFICATIONS** (ST-05 + N-05, WSJF 4.5).
Дата: 2026-10-30. Источник: read-only разбор `reference-oracle/`
(`gateway/`, `out/bundles/`, `pretty/`, `rest-static.*`, `gateway-schema.md`).
Код, тесты и `cargo` не трогались.

Метки: `[D]` — цитата артефакта (дамп, бандл, схема); `[S]` — статическая схема/индекс;
`[A]` — вывод по коду без runtime-подтверждения; `[?]` — не подтверждено.
Смещения в минифицированных бандлах — байтовые (`@NNNNN`), потому что файл однострочный.

## 0. Вердикт

1. Read-контракт `user_guild_settings` снят полностью: контейнер, 13 полей записи,
   6 полей channel override, все перечисления и битовые флаги. Значения
   `message_notifications` 0/1/2/3 **доказаны кодом клиента**, а не догадкой — это
   закрывает `[RE]`/`[A]` из `DESIGN/G05-REST-SPEC.md` §0.5 п.7.
2. `USER_GUILD_SETTINGS_UPDATE` — это **одна запись того же вида**, что элемент
   `d.user_guild_settings.entries[]`; клиент заворачивает её в массив и отдаёт в
   `USER_GUILD_SETTINGS_FULL_UPDATE`.
3. Запись идёт двумя маршрутами: гильдия — `PATCH /users/@me/guilds/settings`
   (bulk, тело `{guilds:{<id>:…}}`), DM — `PATCH /users/@me/guilds/@me/settings`
   (тело — сама запись). Одиночный `PATCH /users/@me/guilds/{guild_id}/settings`
   в клиенте для гильдий **не используется**, но маршрут объявлен.
4. Не подтверждено: тело ответа PATCH, коды 200/204/403/429, сам dispatch в capture
   (в дампе шлюза его нет), запись `hide_muted_channels` клиентом.

## 1. Контейнер в READY

`d.user_guild_settings` — map с тремя ключами `[D]`:
`gateway-schema.md:147`, `gateway-schema.md:385`.

| Поле | Тип (ETF) | Факт в capture | Смысл |
|---|---|---|---|
| `entries` | list | 58 записей | по одной на гильдию (DM — запись с `guild_id=null` `[A]`) |
| `partial` | bool | `false` | `true` — сервер прислал неполный набор, кэш не стирать |
| `version` | int | 734 | версия коллекции для кэша/БД |

Размер секции — 193 547 байт, вторая по величине после `guilds`
(`gateway-protocol.md:177,182`). Top-level `d.notification_settings` тоже есть:
`{"flags": 0}` `[D]` (`gateway/by-type/002-READY.json`).

## 2. Запись `entries[]` — 13 полей

Источник схемы: `gateway-schema.md:255-271`; значения — расчёт по
`gateway/by-type/002-READY.json` (`[D]`, 58 записей).

| Поле | Wire | Присутствие | Наблюдённые значения | Смысл |
|---|---|---|---|---|
| `guild_id` | int64 \| null | 58/58 | snowflake; `null` в capture нет | ключ записи; `null` = личные сообщения |
| `message_notifications` | int | 58/58 | `0`×19, `1`×36, `2`×3 | уровень уведомлений гильдии (§3.1) |
| `suppress_everyone` | bool | 58/58 | `false`×58 | подавлять `@everyone`/`@here` |
| `suppress_roles` | bool | 58/58 | `false`×57, `true`×1 | подавлять упоминания ролей |
| `muted` | bool | 58/58 | `true`×22, `false`×36 | сервер заглушён |
| `mute_config` | map \| null | 58/58 | `null`×37, объект×21 | окно mute (§4) |
| `mobile_push` | bool | 58/58 | `true`×57, `false`×1 | push на мобильные |
| `notify_highlights` | int | 58/58 | `0`×56, `1`×2 | «уведомления о хайлайтах» (§3.2) |
| `flags` | int | 58/58 | `0`×52, `16384`×6 | битовая маска гильдии (§3.4) |
| `hide_muted_channels` | bool | 58/58 | `false`×58 | скрывать заглушённые каналы в списке |
| `mute_scheduled_events` | bool | 58/58 | `false`×57, `true`×1 | глушить события сервера |
| `version` | int | 58/58 | 0…734, у каждой свой | версия записи (last-wins кэша) |
| `channel_overrides` | list \| map \| null | 58/58 | 1328 элементов у 52 записей | переопределения каналов (§5) |

Эталонный вид записи (реальный capture, сокращён) `[D]`:

```json
{
  "guild_id": "1014537851418980372",
  "message_notifications": 1,
  "suppress_everyone": "false",
  "suppress_roles": "false",
  "muted": "true",
  "mute_config": {"end_time": "nil", "selected_time_window": -1},
  "mobile_push": "true",
  "notify_highlights": 0,
  "flags": 0,
  "hide_muted_channels": "false",
  "mute_scheduled_events": "false",
  "version": 331,
  "channel_overrides": [ ... ]
}
```

**Про «строковые» булевы.** В JSON-дампе `true`/`false`/`nil` напечатаны строками,
но это текстовый дамп ETF-атомов, а не строки на проводе `[A]`: `gateway-schema.md`
помечает их ETF-типом `str`/`nil`. Rust-декодер даёт нормальные `Bool`/`Null`
(`discord-core/src/ready/value.rs:88-96`, `is_null` — `Nil` или атом `nil`).
Практическое следствие для парсера: **не** проверять `entry.muted` на истинность
у строки (это баг JS-клиента, `DESIGN/G05-REST-SPEC.md` §0.5 п.2), и **не**
считать отсутствие поля за `false` — это отдельное состояние «пришло частичное
обновление» (§7).

## 3. Перечисления

### 3.1 `message_notifications` — 4 значения

Определение `[D]`:
`out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js@313815` (модуль `652215`,
экспорт `orn`):

```js
Z[Z.ALL_MESSAGES=0]="ALL_MESSAGES", Z[Z.ONLY_MENTIONS=1]="ONLY_MENTIONS",
Z[Z.NO_MESSAGES=2]="NO_MESSAGES", Z[Z.NULL=3]="NULL"
```

| Значение | Имя | Где допустимо | Смысл |
|---:|---|---|---|
| 0 | `ALL_MESSAGES` | гильдия и канал | все сообщения |
| 1 | `ONLY_MENTIONS` | гильдия и канал | только упоминания |
| 2 | `NO_MESSAGES` | гильдия и канал | ничего |
| 3 | `NULL` | только канал | «как в гильдии» (наследование) |

В capture на уровне гильдии `3` не встречается ни разу, на уровне канала —
1327 из 1328 `[D]`. `getChannelMessageNotifications` возвращает `NULL`, если
override отсутствует или в нём нет поля
`@9387157` `[D]` — то есть «нет поля» и «3» для чтения эквивалентны, но на запись
клиент шлёт именно `3` (`@1622` в `discord.com_assets_046406f7a5feb3e2.js`).

### 3.2 `notify_highlights` — 3 значения

Определение `[D]`: `@529852` (модуль `652215`, экспорт `guM`):

```js
tO[tO.NULL=0]="NULL", tO[tO.DISABLED=1]="DISABLED", tO[tO.ENABLED=2]="ENABLED"
```

В capture: `0`×56, `1`×2, `2` — нет `[D]`. UI-тумблер «Highlights» пишет
`ENABLED`/`DISABLED` (`discord.com_assets_046406f7a5feb3e2.js@5511, @17620`).

### 3.3 Настройка unread (не на проводе, вычисляется из `flags`)

`UNSET=0`, `ALL_MESSAGES=1`, `ONLY_MENTIONS=2` `[D]` — `@7338127` (модуль `790782`,
экспорт `e`). Хранится не отдельным полем, а битами `UNREADS_*` в `flags`.

### 3.4 `flags` гильдии (`n3`)

`@8352891` (модуль `355097`) `[D]`:

```js
{OPT_IN_CHANNELS_ON:16384, OPT_IN_CHANNELS_OFF:8192,
 UNREADS_ONLY_MENTIONS:4096, UNREADS_ALL_MESSAGES:2048}
```

`16384` в capture = «каналы только по подписке включены» (6 записей), а не mute.

### 3.5 `flags` канала (`vv`)

`@8352746` (модуль `355097`) `[D]`:

```js
{NEW_FORUM_THREADS_ON:16384, NEW_FORUM_THREADS_OFF:8192, OPT_IN_ENABLED:4096,
 FAVORITED:2048, UNREADS_ALL_MESSAGES:1024, UNREADS_ONLY_MENTIONS:512}
```

В capture: `4096`×1252, `6144` (4096|2048)×1, поле отсутствует у 75 из 1328 `[D]`.
Биты ставятся/снимаются помощниками `Lt`/`br`/`UI`/`iE`/`lA`/`PW`/`mD`
(`@11923745`, модуль `665260`): `iE` снимает перечисленные биты, `UI` ставит,
`mD(e,bit)` = «снять пару UNREADS, поставить bit».

### 3.6 Глобальные (аккаунт) флаги

`USE_NEW_NOTIFICATIONS=16`, `MENTION_ON_ALL_MESSAGES=32` `[D]` — `@6216165`
(модуль `406535`, экспорт `i`). В capture `d.notification_settings.flags=0`,
значит новая модалка уведомлений выключена (§10).

## 4. `mute_config`

Схема `[D]`: `gateway-schema.md:265` (гильдия) и `:281` (канал).

| Поле | Тип | Смысл |
|---|---|---|
| `end_time` | ISO-8601 строка \| null | момент окончания mute; `nil` = бессрочно |
| `selected_time_window` | int (секунды) | выбранное окно; `-1` = «пока не включу» |

Допустимые окна и значения `[D]` — `@8349873` (модуль `355097`, экспорт `Xx`):

| UI | `selected_time_window` |
|---|---:|
| 15 минут | 900 |
| 1 час | 3600 |
| 3 часа | 10800 |
| 8 часов | 28800 |
| 24 часа | 86400 |
| «Пока не включу» | -1 |

Список для дропдауна — `@6225743` (модуль `769591`, `b8()`), порядок и id
(`15-minutes`,`1-hour`,`3-hours`,`8-hours`,`24-hours`,`forever`) `[D]`.
UI формирует `end_time` как `now + selected_time_window` секунд, для `-1` —
`null` (`discord.com_assets_046406f7a5feb3e2.js@11993`) `[D]`.

Семантика чтения `[D]` — `@2364848` (модуль `544743`):

```js
isMuted            = muted && (!cfg || !cfg.end_time || end_time >= now)
isTemporarilyMuted = muted &&  cfg  &&  cfg.end_time  && end_time >= now
```

Следствия:
- `muted=true` + `mute_config=null` — валидное состояние (бессрочный mute):
  из 22 заглушённых гильдий конфиг есть у 21; из 42 заглушённых каналов — у 38 `[D]`.
- Истёкший `end_time` клиент гасит при загрузке (`O.setTimer(...)` → `muted=false`)
  и по таймеру диспатчит `GUILD_MUTE_EXPIRED` / `CHANNEL_MUTE_EXPIRED`
  (`@9384776`, `@9392282`) `[D]`. В capture есть 3 канала с `end_time` 2021 года —
  исторические, для парсера это просто «истекло».
- Формат `end_time` — `2021-02-27T17:14:08.418000+00:00` (микросекунды, смещение)
  `[D]`. Хранить строкой, не терять точность; сравнение — по времени, не по тексту.

## 5. `channel_overrides`

Схема `[D]`: `gateway-schema.md:273-282`; в capture 1328 элементов.

| Поле | Wire | Присутствие | Значения в capture |
|---|---|---|---|
| `channel_id` | int64 | 1328/1328 | snowflake |
| `collapsed` | bool | 1328/1328 | `false`×1279, `true`×49 |
| `flags` | int | **1253/1328** | `4096`×1252, `6144`×1 |
| `message_notifications` | int | 1328/1328 | `3`×1327, `1`×1 |
| `mute_config` | map \| null | 1328/1328 | `null`×1290, объект×38 |
| `muted` | bool | 1328/1328 | `false`×1286, `true`×42 |

Пример `[D]`:

```json
{"channel_id": "1173958353802776576", "collapsed": "false", "flags": 4096,
 "message_notifications": 3, "mute_config": "nil", "muted": "false"}
```

Важные свойства:
1. **Форма.** В READY — массив; клиент принимает и массив, и map по `channel_id`
   (`@9385738`: `e instanceof Array ? keyBy(e,"channel_id") : e`) `[D]`.
   На запись всегда уходит **map** `{ "<channel_id>": {…} }`
   (`@682805`, `@683232`) `[D]`. Парсер обязан поддерживать обе формы.
2. **Override ≠ настройка уведомлений.** Запись создаётся и ради `collapsed`,
   и ради `flags`: 32 override не имеют `flags` и уровня, отличного от 3, и у всех
   32 `collapsed=true` — то есть существуют только из-за свёрнутости `[D]` (расчёт).
   Удаление override в UI не удаляет запись, а пишет
   `{message_notifications: 3, muted: false}` (`@1622`) `[D]`.
3. **`flags` может отсутствовать** (75/1328). При чтении клиент подставляет `?? 0`
   (`@9389080`) `[D]`, но в **апдейте** отсутствие поля не должно означать «сбросить
   в 0» — это «не менять».
4. `collapsed` — UI-состояние категории, но живёт в серверных настройках и
   сохраняется тем же PATCH; клиент дебаунсит изменения на 15 с
   (`@8333009`, модуль `931260`: `u = 15*SECOND`, `E()` перезапускает таймер,
   `h()` подмешивает изменённые `collapsed`) `[D]`.

## 6. Разрешение уровня (что показывать и что подавлять)

`resolvedMessageNotifications` `[D]` — `@9387678`:

1. override канала, если `message_notifications != 3`;
2. иначе override родительской категории (`parent_id`), если `!= 3`;
3. иначе уровень гильдии.

Подавление (`allowNoMessages`, `@9388508`) `[D]`: mute гильдии **или** категории
**или** канала, либо resolved `NO_MESSAGES`, либо opt-in каналы включены и канал
не выбран. `allowAllMessages` — обратное условие.

Пресеты (то, что видит пользователь в таблице каналов) `[D]` — `@6231609`
(модуль `24873`):

| Пресет | unread | `message_notifications` |
|---|---|---|
| `all_messages` | `ALL_MESSAGES` | `ALL_MESSAGES` |
| `hybrid` | `ALL_MESSAGES` | `ONLY_MENTIONS` |
| `mentions` | `UNSET` или `ONLY_MENTIONS` | `ONLY_MENTIONS` |
| `nothing` | `UNSET` или `ONLY_MENTIONS` | `NO_MESSAGES` |
| `custom` | любое другое | — |

Запись пресета (канал) `[D]` — `@6229788`…`@6230821`:
`{message_notifications, flags: mD(текущие, один из UNREADS_ALL_MESSAGES|UNREADS_ONLY_MENTIONS)}`
(`mD` сначала снимает оба бита `UNREADS_*`, потом ставит нужный),
сброс в «по умолчанию» — `{message_notifications: 3, flags: PW(текущие)}`
(`@6230633`, снять оба бита `UNREADS_*`).

## 7. `USER_GUILD_SETTINGS_UPDATE`

Диспатч `[D]` — `@4930904` (модуль `595528`, gateway):

```js
eV(["USER_GUILD_SETTINGS_UPDATE"], e => {
  eW({ type: "USER_GUILD_SETTINGS_FULL_UPDATE", userGuildSettings: [e] });
});
```

Следствия:

1. **Payload — ровно одна запись** формата §2 (snake_case, те же поля). Клиент не
   разворачивает и не переименовывает поля, только оборачивает в массив.
2. Внутреннее действие — `USER_GUILD_SETTINGS_FULL_UPDATE` (массив). Стор
   применяет каждую запись как **частичный апдейт** `[D]` — `@9391654`:

   ```js
   USER_GUILD_SETTINGS_FULL_UPDATE: ({userGuildSettings}) => {
     userGuildSettings.forEach(e => { w(e.guild_id, {channel_overrides: {}, ...e}); });
   }
   ```

   `w()` (`@9384248`) мержит `{...шаблон_гильдии, ...текущая_запись, ...пришедшее}`.
   Шаблон зависит от `guild.default_message_notifications` (`ALL_MESSAGES` или
   `ONLY_MENTIONS`) `[D]` — то есть при отсутствии `message_notifications`
   подставляется серверный дефолт гильдии, а не `0`.
3. **Форма `channel_overrides` в апдейте.** Клиент всегда передаёт ключ в `w()`
   (`channel_overrides: s`), а `j(undefined)` возвращает `{}` — то есть апдейт без
   ключа **очищает** набор `[D]` (`@9385738`). Сервер по смыслу FULL-события шлёт
   запись целиком, но для нашего редьюсера безопаснее трактовать отсутствие ключа
   как «не менять»: иначе частичный апдейт сотрёт 1328 override.
4. `CONNECTION_OPEN` дополнительно нормализует `channel_overrides`,
   если ключа нет: `"channel_overrides" in e || (e.channel_overrides = {})`
   (`@9392282`) `[D]`.
5. `version`/`partial` — кэш `[D]` (`@2134328`): при `CONNECTION_OPEN` и
   `partial=false` таблица настроек стирается и пишется заново с версией
   коллекции; при апдейте пишется одна запись с `version = max(version входящих)`
   и обновляется `user_guild_settings_version`. Для нашей БД: хранить `version`
   на запись и `partial` на коллекцию.

Отдельных server-side событий для одного канала нет: `USER_GUILD_SETTINGS_CHANNEL_UPDATE`
и `_CHANNEL_UPDATE_BULK` — локальные действия клиента (`@9391984`, `@9392142`),
по шлюзу приходит только полная запись гильдии.

## 8. Глобальные настройки — не путать с per-guild

| Что | Где | Форма | Ref |
|---|---|---|---|
| `NOTIFICATION_SETTINGS_UPDATE` | gateway dispatch | `{flags, declarative_settings_proto}` → `{settings:{flags, declarativeSettings}}` | `@4947419` `[D]` |
| `PATCH /users/@me/notification-settings` | REST | тело `{flags}` | `pretty/mod-652215.pretty.js:97`; `@684227` (`setAccountFlag`) `[D]` |
| `d.notification_settings` в READY | READY | `{flags}` | `gateway/by-type/002-READY.json` `[D]` |
| `GET /users/@me/notification-settings/muted-games` | REST | список заглушённых игр | `pretty/mod-652215.pretty.js:67` `[D]` |

Локальные (не серверные) «звуки/тип/бейдж» — `NotificationSettingsStore`,
`persistKey="notifications"` `[D]` — `@9225345` (модуль `803224`):

```js
{desktopType, disableAllSounds, disabledSounds:[], ttsType,
 disableUnreadBadge, taskbarFlash, notifyMessagesInSelectedChannel,
 screenDowntimeReminder}
```

`desktopType`: `NEVER|ALL`; `ttsType`: `NEVER|ALL_CHANNELS|SELECTED_CHANNEL`;
`permission state`: `NEVER|ENABLED|BLOCKED` `[D]` (`@243859`, `@243899`, `@243967`
в модуле `652215`).
Эти значения живут только в локальном сторе и **не** входят в `user_guild_settings`
— для ST-05 «звуки» надо тянуть их из локального слоя, а не из READY.

Отдельная серверная проекция тех же настроек — protobuf
`discord_protos.discord_notifications.v1.GuildNotificationSettings`
`[D]` (`@11054423`): поля 1…10 (`suppress_everyone`, `message_notifications`,
`mobile_push`, `mute`, `channel_overrides`, `suppress_roles`, `version`,
`hide_muted_channels`, `mute_scheduled_events`, `notify_highlights`) и
`UserNotificationSettings{user_id, email_settings, flags, guilds:map, version, data}`.
Это путь declarative-settings/снапшотов; для нашего клиента основной — ETF/JSON из §2/§7.

## 9. REST

### 9.1 Маршруты

| Метод | Путь | Ключ `Endpoints` | Тело | Ref |
|---|---|---|---|---|
| PATCH | `/users/@me/guilds/{guild_id}/settings` | `USER_GUILD_SETTINGS` | запись §2 (частичная) | `pretty/mod-652215.pretty.js:94`; `rest-static.md:659`; `rest-static.json:4442` `[D]` |
| PATCH | `/users/@me/guilds/settings` | `USER_GUILD_SETTINGS_BULK` | `{"guilds": {"<guild_id>": {…}}}` | `pretty/mod-652215.pretty.js:94`; `rest-static.md:656`; `rest-static.json:4428` `[D]` |

Точный текст объявления `[D]` (`pretty/mod-652215.pretty.js:94`):

```js
USER_GUILD_SETTINGS_BULK:"/users/@me/guilds/settings",
USER_GUILD_SETTINGS:e=>`/users/@me/guilds/${e}/settings`
```

### 9.2 Как клиент выбирает маршрут

`saveUserGuildSettings(guildId, settings)` `[D]` — `@8333335` (модуль `931260`):

```js
null == e || e === "@me"
  ? await patch({url: Rsh.USER_GUILD_SETTINGS("@me"), body: t, rejectWithError: false})
  : await bulk({ [e]: t });   // → PATCH /users/@me/guilds/settings, body {guilds:…}
```

В `bulk` (`h()`, `@8334021`) дополнительно:
- подмешиваются изменённые `collapsed` категорий (снимок раз в 15 с),
- удаляется псевдоключ `"@me"` (`delete e[o.YYv]`),
- тело — `{guilds: e}`, ответ возвращается как `.body`.

**Вывод для реализации:** одиночный маршрут нужен для DM-настроек (`@me`), а
изменение гильдии нативнее делать bulk-запросом одним телом с одной гильдией.
Маршрут `PATCH /users/@me/guilds/{guild_id}/settings` объявлен и валиден, но
клиент им для гильдий не пользуется — при расхождении поведения это первое место
для проверки.

### 9.3 Что пишется в теле

Ключи, которые клиент реально отправляет (все — частичные) `[D]`:

| Ключ | Где формируется |
|---|---|
| `message_notifications` | `@682193`, `@682805`, `@6229788` |
| `muted`, `mute_config{selected_time_window,end_time}` | `discord.com_assets_046406f7a5feb3e2.js@11993` |
| `flags` (гильдия/канал) | `@6229788`, `@6231609`, `@682193` |
| `suppress_everyone`, `suppress_roles` | `discord.com_assets_046406f7a5feb3e2.js@5511` |
| `notify_highlights` | там же |
| `mute_scheduled_events`, `mobile_push` | там же |
| `channel_overrides{ "<id>": {message_notifications,muted,mute_config,flags,collapsed} }` | `@682805`, `@683232`, `@8334021` |
| `hide_muted_channels` | **не отправляется** (`GUILD_TOGGLE_COLLAPSE_MUTED` меняет только локальный стор, `@9393306`) `[D]` |

### 9.4 Чего нет в доказательствах

- Ответ PATCH: `rest-static` фиксирует только call-site, тело ответа не снято;
  bulk-ветка читает `.body`, значит ответ непустой `[A]`, форма `[?]`.
- Коды: 200/204/400/403/429 не наблюдались (`confirmedByLiveCapture: 0`,
  `rest-static.json`, статистика `[S]`).
- Ошибки не пробрасываются: `rejectWithError:false` в обоих вызовах `[D]` —
  неудачная запись молча оставляет локальное состояние «изменённым».
  Для нас это довод писать свою обработку ошибок, а не копировать поведение.
- Лимиты/`retry_after` — неизвестны `[?]`.

## 10. UI-контракт (для ST-05/N-05, отдел G2)

Собран из `out/bundles/discord.com_assets_046406f7a5feb3e2.js` (22 716 Б,
модалка «Уведомления» сервера) `[D]`.

Две реализации, выбор по `USE_NEW_NOTIFICATIONS` (бит 16 аккаунта):
`eT` → `F.os("NotificationSettingsModal")` (`@22553`) `[D]`. В нашем capture
флаг 0 → работает вариант с 4 пресетами (`e_`, старая модалка), но в бандле
лежит и новая (`eI`).

| Элемент | Действие | Тело запроса |
|---|---|---|
| Тумблер mute сервера | `updateGuildNotificationSettings(id,{muted})` | `{muted}` |
| Дропдаун длительности (только при mute) | `updateGuildNotificationSettings(id,{muted:true,mute_config})` | `{muted:true,mute_config:{selected_time_window,end_time}}` |
| Селектор уровня (новая модалка, 3 опции) | `updateGuildNotificationSettings(id,{[field]:value})` | `{message_notifications}` — **без** `flags` |
| Селектор пресета (старая модалка, 4 опции) | `updateGuildNotificationSettings(id,{message_notifications,flags})` | `{message_notifications, flags: md(flags, UNREADS_*)}` |
| Тумблеры `suppress_everyone`, `suppress_roles`, `mute_scheduled_events`, `mobile_push` | `updateGuildNotificationSettings(id,{[field]:bool})` | `{<field>}` |
| Тумблер «Highlights» (в новой модалке инвертирован) | `updateGuildNotificationSettings(id,{notify_highlights})` | `2` = ENABLED, `1` = DISABLED |
| Добавить override канала | `updateChannelOverrideSettings` | `{channel_overrides:{"<id>":{muted:false,message_notifications:resolved,flags:mD(current,resolveUnread)}}}` |
| Радио уровня в строке override | там же | `{channel_overrides:{"<id>":{message_notifications:N,muted:t??false}}}` |
| Чекбокс mute в строке | там же | `{channel_overrides:{"<id>":{message_notifications:текущий??3, muted:bool}}}` |
| Удалить override | там же | `{channel_overrides:{"<id>":{message_notifications:3,muted:false}}}` |

Колонки таблицы override (порядок важен для вёрстки) `[D]`: канал, «Все сообщения»,
«Только @упоминания», «Ничего», «Mute». Заголовки — i18n-ключи `uShwWv`, `hZrr6k`,
`JzbSEY`, `y59NJm`, `pGn/bJ`; тултип mute — `8wbTQ6`/`sWmtI6`.

Тексты уровней — i18n-ключи (не хардкодить): `n/bTaY` (все), `L2hmYy`
(только упоминания, с подстановкой), `CtVGyQ` (ничего), `lprV7V` (заголовок
селектора), `Ztu2Yo` (длительность), `r3LawO` («пока не включу»). Для описания
высокого member-count у «всех сообщений» — `Dh5p5j` при `memberCount >= 2500` (`X._4N`).

## 11. Что это значит для кода (без правок)

Уже есть `[D]` (проверено чтением файла):

| Факт | Файл |
|---|---|
| `UserGuildSettings{entries,partial,version}` и `UserGuildSettingsEntry` со всеми 13 полями | `rust/crates/discord-core/src/ready/read_state.rs:49-89` |
| `ChannelOverride{channel_id,collapsed,flags:Option<i64>,message_notifications,mute_config,muted}` | там же `:91-106` |
| `MuteConfig{end_time:Option<String>,selected_time_window:Option<i64>}` | там же `:108-115` |
| Разбор с диагностикой (`ignored`/`malformed`), `channel_overrides` только списком | там же `:262-360` |
| Проекция в snapshot и селектор `user_guild_settings_entry` | `ready/snapshot.rs:246-284`, `state/selectors.rs:115` |

Чего нет:
1. События `USER_GUILD_SETTINGS_UPDATE` в `state` (grep по `rust/` не находит типа) —
   live-обновления не применяются, есть только снимок READY.
2. Записи (REST) и DTO для IPC: `gui-spike/src/data.rs:2207` отдаёт секцию
   «notifications» из другого ключа, `user_guild_settings` в `ready()` не проецируется
   (совпадает с `DESIGN/G05-REST-SPEC.md` §0.5 п.6).
3. Разбора формы-map у `channel_overrides` (сейчас только список `[D]`), хотя
   клиент допускает map.

Рекомендации по типам (не код, а требование к контракту):
- `message_notifications` и `notify_highlights` — enum с `#[non_exhaustive]`-политикой
  «неизвестное значение = диагностика, не 0»; `3` валиден только для канала.
- `muted` без `mute_config` — валидное состояние; отдельный признак «истекло»
  вычислять по времени, а не хранить.
- `flags` — `Option<i64>` для канала (75/1328 без поля), `i64` для гильдии.
- `end_time` — строка как пришла (микросекунды), без парсинга в наносы при хранении.

## 12. Оракулы для тестов

Независимые проверки, которые не переизобретают реализацию:

1. **Счётчики capture:** 58 записей, 1328 override, 52 записи с непустым набором,
   максимум 185 override в одной записи `[D]` (расчёт по
   `gateway/by-type/002-READY.json`).
2. **Распределения:** `message_notifications` гильдии {0:19, 1:36, 2:3};
   канала {3:1327, 1:1}; `notify_highlights` {0:56, 1:2}; `flags` гильдии
   {0:52, 16384:6}; канала {4096:1252, 6144:1, отсутствует:75} `[D]`.
3. **Enum-паритет:** 0/1/2/3 и 0/1/2 сверить с `@313815` и `@529852` `[D]`,
   а не с собственной таблицей.
4. **Mute:** `muted` без `mute_config` остаётся mute; `end_time` в прошлом — не mute
   после нормализации; 22/21 и 42/38 `[D]`.
5. **Слияние апдейта:** запись без `message_notifications` получает
   `guild.default_message_notifications`; при наличии ключа `channel_overrides`
   набор заменяется целиком, при отсутствии — не трогается (наша политика, §7 п.3);
   версия применяется кэшем/БД, в памяти клиент апдейт не гардит — политика
   «старая версия не воскрешает поля» фиксируется как осознанное расхождение.
6. **REST-форма:** тело bulk = `{guilds:{id:…}}`, тело DM = запись; `channel_overrides`
   на запись — map, на чтение — список или map.
7. **Строгий разбор:** `muted:"false"` как строка → ошибка/диагностика, не «истина»;
   отсутствие поля ≠ `false` в частичном апдейте.

## 13. Открытые вопросы

| Вопрос | Метка | Что нужно |
|---|---|---|
| Тело и коды ответа PATCH (single/bulk) | `[?]` | live-запрос или capture ответа |
| `USER_GUILD_SETTINGS_UPDATE` в capture | `[?]` | окно с изменением настроек на живом клиенте |
| Кто пишет `hide_muted_channels` | `[A]` | в бандле только локальный тумблер; возможно, пишет другая сборка/веб |
| Поведение при `NULL` (3) на уровне гильдии | `[A]` | сервер, вероятно, отвергает; в capture не встречается |
| `flags` канала: полный список старших битов | `[?]` | в бандле 6 битов, остальные не объявлены |
| Формат `end_time` при записи (микросекунды vs миллисекунды) | `[A]` | клиент шлёт `toISOString()` (мс); сервер вернул мкс — нормализовать при чтении |

## 14. Индекс ссылок

Бандлы и схемы (`reference-oracle/`):

| Артефакт | Что внутри |
|---|---|
| `gateway-schema.md:147,255-271,273-282,385` | схема READY, поля записи и override |
| `gateway-protocol.md:177,182` | счётчики и размеры секций READY |
| `gateway/by-type/002-READY.json` | реальный payload (58 записей, 1328 override) |
| `rest-static.md:656,659`; `rest-static.json:4428,4442` | объявление PATCH-маршрутов |
| `pretty/mod-652215.pretty.js:67,94,97` | `GAME_NOTIFICATION_SETTINGS`, `USER_GUILD_SETTINGS(_BULK)`, `ACCOUNT_NOTIFICATION_SETTINGS` |
| `out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js@313815` | enum `message_notifications` |
| `…@529852` | enum `notify_highlights` |
| `…@7338127` | enum unread (UNSET/ALL/ONLY) |
| `…@8352746`, `…@8352891`, `…@8349873` | биты канала, биты гильдии, окна mute |
| `…@2364848` | семантика mute и таймер |
| `…@9383822` | дефолты записи в сторе |
| `…@9384248`, `…@9385738` | слияние записи, нормализация `channel_overrides` |
| `…@9387157`, `…@9387678`, `…@9388508` | чтение override, разрешение уровня, подавление |
| `…@9391654`, `…@9392282` | обработчики `FULL_UPDATE` и `CONNECTION_OPEN` |
| `…@4930904`, `…@4947419` | gateway-диспатчи `USER_GUILD_SETTINGS_UPDATE`, `NOTIFICATION_SETTINGS_UPDATE` |
| `…@2134328` | версии и `partial` в кэше |
| `…@8333335`, `…@8334021` | выбор маршрута REST и bulk-тело |
| `…@682193`, `…@682805`, `…@683232` | экшены настроек и override |
| `…@6231500`, `…@6231609`, `…@6229788` | пресеты и их запись |
| `…@6216165` | флаги аккаунта (16/32) |
| `…@6225743` | список окон mute |
| `…@9225345` | локальный `NotificationSettingsStore` |
| `…@11054423` | protobuf `GuildNotificationSettings` |
| `out/bundles/discord.com_assets_046406f7a5feb3e2.js@1228,1622,5511,11993,12220,15696,18911,22553` | UI: строка override, удаление, тумблеры, mute-дропдаун, селектор пресета, селектор уровня, таблица override, выбор модалки |

Смещения — в байтах от начала файла; бандл однострочный, поэтому `@N` точнее номера
строки. Для `pretty/` и схем указаны номера строк.
