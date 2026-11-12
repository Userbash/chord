# DM-FRIENDS-RE — личные сообщения, групповые DM и друзья

Карточка: **RE-N1-07-DM-FRIENDS** (`TASKS.md` N1-07, владельцы G1+G2, приоритет P3).
Дата: 2026-10-30. Источник — read-only разбор `reference-oracle/`:
`decoded/4d2a64cb5df5fc4e_0` (`web.4d4b2f75fdea5470.js`, Discord 1.0.162, 12 410 287 Б,
md5 `934b1289b7ebf17bc0fd09569b23b438`), `decoded/d38b37e4d52b4495_0`
(`07cac0073fc4f6e3.js`), `decoded/e1affb550dd48702_0` (`e51d4232a1a23440.js`),
`gateway/by-type/*`, `gateway-schema.md`, `gateway-protocol.md`, `rest-static.*`.

`logs/`, `functions/`, `evidence/`, `reference-oracle/` не изменялись. Код, тесты и
`cargo` не трогались; изменён только этот документ.

Метки: `[D]` — цитата декодированного бандла/дампа; `[S]` — статическая схема или индекс;
`[A]` — вывод по коду без runtime-подтверждения; `[?]` — не подтверждено.
Смещения — байтовые (`@NNNNN`), потому что минифицированный бандл однострочный.

## 0. Вердикт

1. **REST-контракт снят полностью**: 24 пары «метод–путь» на 12 путях — создание DM,
   групповая DM через shell, добавление/удаление получателей, выход, закрытие,
   message requests, полный цикл друзей (заявка, принятие, блок, ник, игнор),
   очистка pending, заметки.
2. **Поправка к формулировке задания**: `POST /users/@me/channels` **никогда** не получает
   `recipient_id`. Клиент всегда шлёт массив `recipients` — даже для одного получателя
   (§2.1). В контракте ЛС `recipient_id` встречается ровно в одном месте —
   `POST /users/@me/channels/group-dm-shell`.
3. **`GET /users/@me/channels` клиентом не вызывается.** Константа `USER_CHANNELS` одна на
   GET и POST, но call-site только POST; список ЛС приходит в `READY.private_channels`.
   Это не пробел контракта, а факт: HTTP-метод для этого пути в клиенте не реализован.
4. **Перечисления доказаны кодом, а не догадкой**: `ChannelType` 1 = DM, 3 = GROUP_DM
   (плюс `PRIVATE_CHANNEL = {1,3}`); `RelationshipType` 0 = NONE, 1 = FRIEND, 2 = BLOCKED,
   3 = PENDING_INCOMING, 4 = PENDING_OUTGOING, 5 = IMPLICIT, 6 = SUGGESTION.
5. **READY снят с живого захвата**: `private_channels[]` — 17 объектов (все `type:1`, у всех
   ровно один `recipient_ids`), `relationships[]` — 60 объектов (1:18, 2:42), все `user_id`
   резолвятся в `READY.users[]` (146 пользователей).
6. **Dispatch-контракт**: `CHANNEL_CREATE`/`CHANNEL_DELETE` несут сам объект канала,
   `CHANNEL_RECIPIENT_ADD/REMOVE` — `{channel_id,user,nick}`, `RELATIONSHIP_ADD/UPDATE` —
   relationship-объект с `user`, `RELATIONSHIP_REMOVE` — тот же объект без обязательного `user`.
   Ни одного из этих диспатчей в захвате нет (§7.5).
7. Остаточные gap: тело/коды `GET /users/@me/channels` (клиентом не используется),
   точный набор ключей captcha в теле заявки, серверные числовые лимиты (в клиенте их нет),
   факт приёма `PUT` без `type` при принятии заявки.

## 1. Маршруты

### 1.1 Каталог эндпоинтов

Все пути — из `Endpoints` (`Rsh`, модуль `652215`, `[D] @258630`, `@259640`, `@260280`,
`@266725`, `@270359`, `@271460`, `@273990`, `@310495`). Колонка «call-site» — число
подтверждённых вызовов в бандле 1.0.162 (`rest-static.json` + собственный grep).

| # | Метод | Путь | Ключ | call-site | Тело | Ответ |
|---|---|---|---|---|---|---|
| 1 | GET | `/users/@me/channels` | `USER_CHANNELS` | **0** | — | `[A]` массив каналов; клиент не использует |
| 2 | POST | `/users/@me/channels` | `USER_CHANNELS` | 2 | `{recipients:[snowflake]}` | объект канала (DM или GROUP_DM) |
| 3 | POST | `/users/@me/channels/group-dm-shell` | `USER_GROUP_DM_SHELL` | 1 | `{recipient_id}` | объект канала (пустая группа) |
| 4 | POST | `/users/@me/channels/bulk-leave` | `USER_CHANNELS_BULK_LEAVE` | 1 | `{channel_ids:[snowflake]}` | `[A]` тело не читается |
| 5 | GET | `/users/@me/dms/{user_id}` | `DM_CHANNEL` | 1 | — | объект канала |
| 6 | GET | `/users/@me/relationships` | `USER_RELATIONSHIPS()` | 2 | — | массив связей, у каждой `user` |
| 7 | POST | `/users/@me/relationships` | `USER_RELATIONSHIPS()` | 1 | `{username,discriminator,note?}` | `[A]` тело не читается |
| 8 | PUT | `/users/@me/relationships/{user_id}` | `USER_RELATIONSHIP(e)` | 1 | `{type,from_friend_suggestion?,confirm_stranger_request?,captcha?}` | `[A]` тело не читается |
| 9 | DELETE | `/users/@me/relationships/{user_id}` | `USER_RELATIONSHIP(e)` | 1 | — | `[A]` тело не читается |
| 10 | PATCH | `/users/@me/relationships/{user_id}` | `USER_RELATIONSHIP(e)` | 1 | `{nickname}` | `[A]` |
| 11 | PUT | `/users/@me/relationships/{user_id}/ignore` | `IGNORE_USER(e)` | 1 | — | `[A]` |
| 12 | DELETE | `/users/@me/relationships/{user_id}/ignore` | `IGNORE_USER(e)` | 1 | — | `[A]` |
| 13 | DELETE | `/users/@me/relationships` + `?relationship_type=3` | `USER_RELATIONSHIPS()` | 1 | — | `[A]` |
| 14 | DELETE | `/users/@me/relationships` + `?relationship_type=3` | `USER_RELATIONSHIPS()` | 1 | `{filters:[1,2]}` | `[A]` |
| 15 | GET | `/users/@me/notes/{user_id}` | `NOTE(e)` | 1 | — | `{note_user_id,note,…}` |
| 16 | PUT | `/users/@me/notes/{user_id}` | `NOTE(e)` | 1 | `{note}` | `[A]` |
| 17 | PUT | `/channels/{id}/recipients/{user_id}` | `CHANNEL_RECIPIENT(e,t)` | 1 | — | **201** — объект канала, иначе пусто |
| 18 | DELETE | `/channels/{id}/recipients/{user_id}` | `CHANNEL_RECIPIENT(e,t)` | 1 | — | `[A]` |
| 19 | PUT | `/channels/{id}/recipients/@me` | `CHANNEL_RECIPIENT_ME(e)` | 3 | `{consent_status}` | `[A]` |
| 20 | PATCH | `/channels/{id}/recipients/@me` | `CHANNEL_RECIPIENT_ME(e)` | 1 | `{flags}` | `[A]` |
| 21 | DELETE | `/channels/{id}/recipients/@me` | `CHANNEL_RECIPIENT_ME(e)` | 1 | — | `[A]` |
| 22 | PUT | `/channels/recipients/@me/batch-reject` | `CHANNEL_RECIPIENT_REJECT_BATCH()` | 1 | `{channel_ids:[snowflake]}` | `[A]` |
| 23 | PATCH | `/channels/{id}` | `CHANNEL(e)` | 3 | `{name}` \| `{icon}` \| `{owner}` | объект канала `[A]` для `name`/`icon` |
| 24 | DELETE | `/channels/{id}` + `?silent={bool}` | `CHANNEL(e)` | 2 | — | `[A]` |

Смежные, но не входящие в контракт DM: `GET /friend-suggestions` (`FRIEND_SUGGESTIONS`),
`DELETE /friend-suggestions/{id}` (`FRIEND_SUGGESTION(e)`), `GET|POST|DELETE /users/@me/invites`
(`FRIEND_INVITES`, `[S]` — в индексе один union из трёх методов), `POST /users/@me/relationships/bulk`
(`USER_BULK_RELATIONSHIPS`, **0** call-site).

### 1.2 Цитаты эндпоинтов

`[D] @258630`:

```js
USER_RELATIONSHIPS:function(){let e=arguments.length>0&&void 0!==arguments[0]?arguments[0]:tk.ME;
  return`/users/${e}/relationships`},
USER_RELATIONSHIP:e=>`/users/@me/relationships/${e}`,
USER_BULK_RELATIONSHIPS:"/users/@me/relationships/bulk",
```

`tk.ME` — это литерал `"@me"` (`[D]` модуль `901123`: `let s="@me"`), поэтому путь
разворачивается в `/users/@me/relationships`. Параметр существует только для чтения чужого
списка (`/users/{id}/relationships`), клиент его не передаёт.

`[D] @259640`:

```js
USER_CHANNELS:"/users/@me/channels",
USER_CHANNELS_BULK_LEAVE:"/users/@me/channels/bulk-leave",
USER_GROUP_DM_SHELL:"/users/@me/channels/group-dm-shell",
```

`[D] @270359`:

```js
CHANNEL_RECIPIENTS:e=>`/channels/${e}/recipients`,
CHANNEL_RECIPIENT:(e,t)=>`/channels/${e}/recipients/${t}`,
CHANNEL_RECIPIENT_ME:e=>`/channels/${e}/recipients/@me`,
CHANNEL_RECIPIENT_REJECT_BATCH:()=>"/channels/recipients/@me/batch-reject",
```

`[D] @260290`, `@266725`, `@273990`, `@310495`:

```js
DM_CHANNEL:e=>`/users/@me/dms/${e}`,
CHANNEL:e=>`/channels/${e}`,
NOTE:e=>`/users/@me/notes/${e}`,
IGNORE_USER:e=>`/users/@me/relationships/${e}/ignore`,
```

### 1.3 `GET /users/@me/channels` — почему нуля вызовов достаточно

Проверено тремя способами:

* `rest-static.json`: у `USER_CHANNELS` `"method":"POST"`, `"callSiteCount":2`,
  `"bodyKeys":["{recipients}"]` — GET-вызова нет.
* grep по всем декодированным бандлам и по второй сборке (`flatpak-hits/`):
  `get({url:*.Rsh.USER_CHANNELS…` — **0 совпадений**, `post({url:…USER_CHANNELS,body:{…}}` — 2.
* Клиент не нуждается в этом вызове: список ЛС приходит в `READY.private_channels`
  (§6.1), новые каналы — диспатчем `CHANNEL_CREATE` (§7.1), точечное открытие —
  `GET /users/@me/dms/{user_id}`.

Для паритета это означает: GET-ручку можно не реализовывать, пока не появится сценарий
«перечитать список». Если реализуем — форма ответа `[A]`: массив объектов канала того же
вида, что `private_channels[]`, но с `recipients` вместо `recipient_ids` (как у POST, §2.2).

## 2. Создание DM и групповой DM

### 2.1 Тело запроса

`[D] @567585` (`openPrivateChannel`):

```js
openPrivateChannel(e){let{recipientIds:t,joinCall:n=!1,joinCallVideo:r=!1,location:a,
  onBeforeTransition:s,navigateToChannel:l=!0}=e,o=this._getRecipients(t);
function d(e){n&&T.A.call(e.id,r,!0,e.isDM()?e.getRecipientId():null)}
if(1===o.length){let[e]=o,t=this._openCachedDMChannel(e,s,l);if(null!=t)return d(t),Promise.resolve(t.id)}
try{let e=await i.Bo.post({url:g.Rsh.USER_CHANNELS,body:{recipients:o},
  context:{location:a},oldFormErrors:!0,retries:3,rejectWithError:(0,i.fT)()});
if(l){s?.();let t=this._openPrivateChannel(e.body);d(t)}return e.body.id}
```

`[D] @568882` (`ensurePrivateChannel`):

```js
ensurePrivateChannel(e){let t=this._getRecipients(e),
  n=await i.Bo.post({url:g.Rsh.USER_CHANNELS,body:{recipients:t},
    oldFormErrors:!0,rejectWithError:(0,i.fT)()}),
  r=(0,A.UE)(n.body);return a.h.dispatch({type:"CHANNEL_CREATE",channel:r}),r.id}
```

Нормализация входа `[D] @569410`: `_getRecipients:e=>null!=e?Array.isArray(e)?e:[e]:[]` —
одиночный id заворачивается в массив. Итоговое тело всегда `{"recipients":["<snowflake>",…]}`,
даже для одного получателя.

Параметры запроса:
`retries: 3`, `oldFormErrors: true`, `rejectWithError: (0,i.fT)()`, `context: {location}`.
`oldFormErrors` — режим разбора ошибок формы, **не** поле тела.

**Что это меняет против задания.** `recipient_id` в этом маршруте не существует. Форма с
одним полем есть только у shell-маршрута (§2.4). Проверка по двум сборкам:
`USER_CHANNELS,body:{recipients:…}` в `decoded/` и `flatpak-hits/` — других вариантов нет.

### 2.2 Ответ

`e.body` используется целиком: `(0,A.UE)(n.body)` → `createChannelRecord` → класс
приватного канала (`[D] @8903690`: `[I.rbe.DM]:class extends eD{}`, `[I.rbe.GROUP_DM]:class extends eD{}`).

Обязательный минимум ответа — то, что читает `eD.fromServer` (`[D] @8899327`):

```js
static fromServer(e){let t=eD.sortRecipients(e.recipients,e.id),n={
  application_id:e.application_id,flags_:e.flags,guild_id:null,icon:e.icon,id:e.id,
  isMessageRequest:e.is_message_request,isMessageRequestTimestamp:e.is_message_request_timestamp,
  isSpam:e.is_spam??!1,lastMessageId:e.last_message_id,lastPinTimestamp:e.last_pin_timestamp,
  name:e.name??"",nicks:ec(e.nicks),ownerId:e.owner_id,rawRecipients:t,
  recipients:t.map(e=>e.id),recipientFlags:e.recipient_flags,
  safetyWarnings:e.safety_warnings,blockedUserWarningDismissed:e.blocked_user_warning_dismissed,
  type:null!=e.type?e.type:I.rbe.DM};return(0,_.pp)(n,eD)}
```

Следствия:
* `recipients` — массив **объектов пользователей** (клиент сортирует их по `id`, затем
  берёт `id`). Одних id недостаточно: `t.map(e=>e.id)` упадёт на строках.
* `type` по умолчанию — `1` (DM), для группы сервер обязан прислать `3`.
* `guild_id` принудительно `null` — приватный канал не может быть гильдейским.
* `recipients` сортируются детерминированно: `sort((a,b)=>(hash(a.id)^n)-(hash(b.id)^n))`,
  `n` — хеш id канала. Порядок получателей в ответе не важен.
* Для GROUP_DM читаются также `name`, `icon`, `owner_id`, `nicks` (map `userId → nick`).

### 2.3 Коды ошибок

`[D] @568148` (`createGroupDmShell`, тот же паттерн у `openPrivateChannel`):

```js
}catch(e){throw e?.body?.code===g.t02.QUARANTINED&&(0,u.default)(),e}
```

`QUARANTINED` = **20026** (`[D] @321019`, enum `t02`). То есть единственный код, который
клиент обрабатывает специально, — «аккаунт на карантине»: открывается модалка, ошибка
пробрасывается дальше.

Остальные коды приходят из общего enum (`[D] @321000…332000`, `t02`):

| Код | Имя | Когда |
|---|---|---|
| 20026 | `QUARANTINED` | аккаунт на карантине → модалка |
| 30004 | `TOO_MANY_RECIPIENTS` | группа переполнена |
| 40003 | `RATE_LIMIT_DM_OPEN` | слишком часто открывают ЛС |
| 50003 | `INVALID_ACTION_DM` | действие невозможно (например, бот) |
| 50096 | `INVALID_CANNOT_FRIEND_SELF` | заявка самому себе |
| 10013 | `UNKNOWN_USER` | получатель не найден |
| 50035 | `INVALID_FORM_BODY` | битый `recipients` |
| 340005 | `USER_DM_UPLOAD_ATTACHMENT_LIMITED_ACCESS` | ограничение вложений в ЛС |
| 340006 | `USER_GDM_UPLOAD_ATTACHMENT_LIMITED_ACCESS` | то же для группы |

429 обрабатывается общим HTTP-слоем (`retry-after`), отдельной ветки в DM-коде нет.

### 2.4 Shell группы и добавление получателей

`[D] @568215` (`createGroupDmShell`):

```js
createGroupDmShell(e){let{recipientId:t,location:n,onBeforeTransition:r,navigateToChannel:a=!0}=e;
try{let e=await i.Bo.post({url:g.Rsh.USER_GROUP_DM_SHELL,body:{recipient_id:t},
  context:{location:n},oldFormErrors:!0,retries:3,rejectWithError:(0,i.fT)()});
return a&&(r?.(),this._openPrivateChannel(e.body)),e.body.id}
```

Это единственное место с `recipient_id` в контракте ЛС. Сценарий: создать «пустую» группу
с одним участником, затем наращивать состав через `PUT /channels/{id}/recipients/{user_id}`.

`[D] @570912` (`addRecipient`):

```js
addRecipient(e,t,n,a){return i.Bo.put({url:g.Rsh.CHANNEL_RECIPIENT(e,t),
  context:{location:n},oldFormErrors:!0,rejectWithError:!0}).then(t=>(
  r.O.announce(S.intl.string(S.t.cU0t1F)),a?.(),201===t.status)?this._openPrivateChannel(t.body).id:e)
  .catch(()=>(r.O.announce(S.intl.string(S.t["8GEdej"])),e))}
```

Контракт: **201** — сервер вернул новый канал (shell стал настоящей группой), клиент
открывает его; иначе (200) канал не менялся, возвращается прежний id. Ошибка не
пробрасывается — тост и возврат исходного канала.

`addRecipients` (`[D]` там же) добавляет **последовательно**: первый вызов может вернуть
201, остальные идут уже в новый канал.

## 3. Групповая DM: управление

| Действие | Запрос | Тело / query | Смещение |
|---|---|---|---|
| Переименовать | `PATCH /channels/{id}` | `{name}` | `[D] @571559` |
| Сменить иконку | `PATCH /channels/{id}` | `{icon}` (строка или `null`) | `[D] @571778` |
| Сменить владельца | `PATCH /channels/{id}` | `{owner}` | `[D] @571455` |
| Добавить участника | `PUT /channels/{id}/recipients/{user_id}` | — | `[D] @570912` |
| Убрать участника | `DELETE /channels/{id}/recipients/{user_id}` | — | `[D]` там же |
| Выйти из группы | `DELETE /channels/{id}/recipients/@me` | — | `[D] e1affb…` |
| Выйти из нескольких | `POST /users/@me/channels/bulk-leave` | `{channel_ids}` | `[D] @570274` |
| Закрыть ЛС | `DELETE /channels/{id}` | `?silent={bool}` | `[D] @569905` |

`setIcon` дополнительно шлёт аналитику `CHANNEL_ICON_EDIT_PROGRESSED` с
`{channel_id, channel_type, old_icon_set, new_icon_set, location, status}` — это не поле
запроса.

### 3.1 Закрытие ЛС

`[D] @569942`:

```js
a.h.dispatch({type:"CHANNEL_DELETE",channel:{id:e,guild_id:void 0,parent_id:void 0},silent:n}),
t&&!__OVERLAY__&&(0,_.pX)(g.BVt.FRIENDS),
i.Bo.del({url:g.Rsh.CHANNEL(e),query:{silent:n},oldFormErrors:!0,rejectWithError:(0,i.fT)()})
```

То есть «закрыть ЛС» — это `DELETE /channels/{id}?silent=true|false` с оптимистичным
локальным `CHANNEL_DELETE`. `silent` управляет аналитикой/анимацией, не серверным смыслом.

### 3.2 Message requests (ЛС от незнакомцев)

Модуль `838056` (`[D] decoded/e1affb550dd48702_0`, `e51d4232a1a23440.js`):

```js
async function s(t){await n.Bo.put({url:c.Rsh.CHANNEL_RECIPIENT_ME(t),
  body:{consent_status:u.w.ACCEPTED},rejectWithError:(0,n.fT)()}),
  o.h.dispatch({type:"MESSAGE_REQUEST_ACCEPT_OPTIMISTIC",channelId:t})}
function i(t){return n.Bo.put({url:c.Rsh.CHANNEL_RECIPIENT_ME(t),
  body:{consent_status:u.w.UNSPECIFIED},…})}
function C(t){return n.Bo.put({url:c.Rsh.CHANNEL_RECIPIENT_ME(t),
  body:{consent_status:u.w.PENDING},…})}
function _(t){return n.Bo.del({url:c.Rsh.CHANNEL_RECIPIENT_ME(t),…})}
function h(t){return n.Bo.put({url:c.Rsh.CHANNEL_RECIPIENT_REJECT_BATCH(),
  body:{channel_ids:t},…})}
```

| Значение `consent_status` | Смысл |
|---|---|
| `"ACCEPTED"` | принять запрос → `MESSAGE_REQUEST_ACCEPT_OPTIMISTIC` |
| `"UNSPECIFIED"` | снять выбор (вернуть в неопределённое) |
| `"PENDING"` | оставить в запросах |

Полный enum (`u.w`, модуль `961523`) в дампе не найден — три значения доказаны кодом,
четвёртое (`"REJECTED"`?) — `[?]`. Отказ реализован как `DELETE /recipients/@me`
(выход из канала) и батч-`PUT /channels/recipients/@me/batch-reject` с `{channel_ids}`.

### 3.3 `recipient_flags`

`[D]` чанк `07cac0073fc4f6e3.js` (модуль `253506`):

```js
i=((l={})[l.DISMISSED_IN_GAME_MESSAGE_NUX=1]="DISMISSED_IN_GAME_MESSAGE_NUX",
  l[l.DISMISSED_CURRENT_CHAT_WALLPAPER=2]="DISMISSED_CURRENT_CHAT_WALLPAPER",l)
```

Читается через `(0,er.Lt)(t.recipientFlags??0, flag)` и пишется
`PATCH /channels/{id}/recipients/@me {flags}`. В захвате у всех 17 каналов
`recipient_flags: 0` — оба бита не выставлены.

## 4. Друзья: `/users/@me/relationships`

### 4.1 Список

`[D] @699083` (`fetchRelationships`):

```js
fetchRelationships(){r.Bo.get({url:I.Rsh.USER_RELATIONSHIPS(),oldFormErrors:!0,
  rejectWithError:!0}).then(e=>s.h.dispatch({type:"LOAD_RELATIONSHIPS_SUCCESS",
  relationships:e.body}),()=>s.h.dispatch({type:"LOAD_RELATIONSHIPS_FAILURE"}))}
```

GET без query, ответ — **массив**. Ключевое отличие от READY: элемент содержит объект
`user`, а не `user_id`. Потребитель `[D] @9403042`:

```js
function eS(e){let{relationships:t}=e;return t.reduce((e,t)=>D(t.user)||e,!1)}
```

`D(user)` кладёт пользователя в `UserStore` — значит без `user` ответ бесполезен.
Полный набор ключей элемента `[A]`: тот же relationship-объект (§5.2) плюс `user`.

### 4.2 Заявка в друзья

`[D] @697487` (`sendRequest`):

```js
sendRequest(e){let{discordTag:t,context:n,captchaPayload:i,note:a,errorUxConfig:s=0}=e,
  [l,o]=t.split("#");
return r.Bo.post({url:I.Rsh.USER_RELATIONSHIPS(),
  body:{username:l,discriminator:parseInt(o),note:a,...i},context:n,
  oldFormErrors:!0,rejectWithError:(0,r.fT)()}).catch(e=>{N(e,s,t)})}
```

* `discordTag` — строка `"username#1234"`, разбивается по `#`, дискриминатор парсится в
  **число** (`parseInt`), а не строка.
* `note` — необязательное сообщение к заявке.
* `captchaPayload` раскрывается в тело (`...i`). Точные ключи `[A]`; общий HTTP-перехватчик
  для капчи использует заголовки `X-Captcha-Key`, `X-Captcha-Rqtoken`,
  `X-Captcha-Session-Id` (`[D] @1004797`), но здесь payload идёт в тело.
* **Pomelo.** Для аккаунтов с `discriminator === "0"` тег формируется без `#`
  (`[D]` модуль `getUserTag`: `if("0"!==e.discriminator&&e.discriminator!==s.h3J) … username#disc`).
  Тогда `o === undefined`, `parseInt(undefined) === NaN`, и `JSON.stringify` даёт
  `"discriminator":null`. Серверное поведение при `null` — `[A]`.

### 4.3 PUT — заявка по id, принятие, блок

`[D] @697776` (`addRelationship`):

```js
addRelationship(e,t){let{userId:n,context:i,type:a,fromFriendSuggestion:s,
  confirmStrangerRequest:l,captchaPayload:o}=e,d=…,c=_.default.getUser(n);
return r.Bo.put({url:I.Rsh.USER_RELATIONSHIP(n),
  body:{type:a,from_friend_suggestion:s,confirm_stranger_request:l,...o},
  context:i,oldFormErrors:!0,rejectWithError:(0,r.fT)()})
```

| Вызов | `type` | Примечание |
|---|---|---|
| `blockUser` | `2` (`BLOCKED`) | `addRelationship({userId,context,type:BLOCKED})` |
| `acceptFriendRequest` | **не передаётся** | `acceptFriendRequest:e=>C.addRelationship(e, cb)`; вызывающий слой `maybeConfirmFriendRequestAccept` кладёт только `{userId, confirmStrangerRequest, context}` |

Практический вывод: при принятии заявки клиент отправляет
`PUT /users/@me/relationships/{id}` с телом `{"confirm_stranger_request":false}` —
`type` отсутствует, потому что `undefined` выпадает при сериализации. Что сервер делает с
`PUT` без `type` — `[A]`; для паритета безопаснее всегда слать `type: 1`.

`confirm_stranger_request` — подтверждение «незнакомца». Сценарий `[D] @6673300`:

```js
function C(e,t){return e?.body?.code===d.t02.RELATIONSHIP_INVALID_NO_CONFIRMATION
  ?(i.h.dispatch({type:"UPDATE_STRANGER_STATUS",userId:t.userId,isStranger:!0}),
    S({onConfirm:()=>{N({...t,confirmStrangerRequest:!0}),t.onConfirm?.()},…}),!0)
  :(e?.ok&&i.h.dispatch({type:"UPDATE_STRANGER_STATUS",userId:t.userId,isStranger:!1}),!1)}
```

То есть код **80013** = «нужно подтверждение», клиент показывает диалог и повторяет запрос
с `confirm_stranger_request: true`. Повторная попытка идёт уже без диалога
(`maybeConfirmFriendRequestAccept` проверяет `RelationshipStore.isStranger`).

`from_friend_suggestion: true` ставится, когда заявка отправлена из списка рекомендаций.
Источник значения — `[A]` (в дампе только чтение поля на входе).

### 4.4 DELETE и PATCH

`[D] @698771`:

```js
removeRelationship:(e,t,n)=>r.Bo.del({url:I.Rsh.USER_RELATIONSHIP(e),context:t,
  oldFormErrors:!0,rejectWithError:(0,r.fT)()}).then(()=>{n?.()}).catch(()=>{…})
```

Один и тот же вызов для `removeFriend`, `cancelFriendRequest`, `unblockUser`; различие —
только тост/аналитика. Ошибка поглощается тостом «не удалось» (best-effort, осознанно).

`[D] @698967`:

```js
updateRelationship:(e,t)=>r.Bo.patch({url:I.Rsh.USER_RELATIONSHIP(e),
  body:{nickname:t},rejectWithError:(0,r.fT)()})
```

`nickname` — серверный псевдоним друга (виден только отправителю), не глобальное имя.

### 4.5 Игнор и очистка pending

`[D] @699907`, `@700264`:

```js
ignoreUser:(e,t,n)=>r.Bo.put({url:I.Rsh.IGNORE_USER(e),context:{location:t},…})
  .then(()=>{…dispatch({type:"RELATIONSHIP_IGNORE_USER_SUCCESS",userId:e,timestamp:Date.now()})}),
unignoreUser:(e,t,n)=>r.Bo.del({url:I.Rsh.IGNORE_USER(e),context:{location:t},…})
```

Тела нет. Игнор — отдельная ось от `BLOCKED`: `RelationshipStore` хранит `ignored` в
`Set` и считает `isIgnored` как «не BLOCKED и в Set» (`[D] @9319095`).

`[D] @699318`, `@699593`:

```js
clearPendingRelationships:()=>r.Bo.del({url:I.Rsh.USER_RELATIONSHIPS(),
  query:{relationship_type:I.eA$.PENDING_INCOMING},…})
  .then(()=>{dispatch({type:"RELATIONSHIP_PENDING_INCOMING_REMOVED"})}),
clearPendingSpamAndIgnored:()=>r.Bo.del({url:I.Rsh.USER_RELATIONSHIPS(),
  query:{relationship_type:I.eA$.PENDING_INCOMING},body:{filters:[p.w.SPAM,p.w.IGNORED]},…})
```

Оба — `DELETE` с query `relationship_type=3`; второй дополнительно фильтрует тело
`{filters:[1,2]}` (`RelationshipFilters`: `SPAM=1`, `IGNORED=2`, `[D] @7346275`).

### 4.6 Заметки

Чанк `229787` (`[D] decoded/9f6dccfb41eb44b6_0`, модуль `612630`) — чтение:

```js
let{body:e}=await s.Bo.get({url:d.Rsh.NOTE(t),oldFormErrors:!0,rejectWithError:!0});
if(e.note_user_id!==t)throw Error("Invalid response from server");
n.h.dispatch({type:"USER_NOTE_UPDATE",id:t,note:e.note})
```

Запись — модуль `629403` (`[D] decoded/a8de0187058c21c2_0`):

```js
updateNote:(e,t)=>i.Bo.put({url:l.Rsh.NOTE(e),body:{note:t},oldFormErrors:!0,rejectWithError:!0})
```

GET возвращает `{note_user_id, note, …}`; клиент **проверяет** `note_user_id === userId`
и падает на несовпадении — хороший оракул для фейка. Код ошибки `50015 INVALID_NOTE`.

### 4.7 Перечисления друзей

`RelationshipType` (`[D] @258005`, enum `eA$`):

| Значение | Имя в клиенте | Смысл в UI |
|---|---|---|
| 0 | `NONE` | связи нет |
| 1 | `FRIEND` | друг |
| 2 | `BLOCKED` | заблокирован |
| 3 | `PENDING_INCOMING` | входящая заявка |
| 4 | `PENDING_OUTGOING` | исходящая заявка |
| 5 | `IMPLICIT` | неявная связь (игровые/платформенные связи) |
| 6 | `SUGGESTION` | рекомендация |

В задании 3 и 4 названы `INCOMING_REQUEST`/`OUTGOING_REQUEST` — это те же значения,
в клиенте они называются `PENDING_INCOMING`/`PENDING_OUTGOING`.

Вкладки списка друзей (`[D] @243700`): `ADD_FRIEND`, `ALL`, `ONLINE`, `PENDING`,
`PENDING_IGNORED`, `SPAM`, `SUGGESTIONS` — это UI-секции, а не параметры запроса.

`RelationshipFilters` (`[D] @7346275`): `SPAM=1`, `IGNORED=2`.

`RelationshipStore` (`[D] @9318383`) считает:

```js
T=Math.max((v.get(PENDING_INCOMING)?.size??0)-m-g,0)   // getPendingCount
m=E.size                                                // getSpamCount
g=h.size                                                // getPendingIgnoredCount
```

`getFriendCount/getOutgoingCount` — размеры соответствующих множеств.

### 4.8 Коды ошибок друзей

`[D] @321000…332000`, enum `t02`:

| Код | Имя | Реакция клиента |
|---|---|---|
| 30002 | `TOO_MANY_FRIENDS` | общий текст ошибки |
| 30059 | `TOO_MANY_BLOCKED_USERS` | общий текст |
| 30078 | `TOO_MANY_PENDING_OUTGOING` | общий текст |
| 40002 | `EMAIL_VERIFICATION_REQUIRED` | модалка «подтвердите почту» |
| 40007 | `USER_BANNED` | общий текст |
| 40068 | `USER_QUARANTINED` | закрыть модалку, показать карантин |
| 50035 | `INVALID_FORM_BODY` | ошибка формы |
| 50096 | `INVALID_CANNOT_FRIEND_SELF` | общий текст |
| 80001 | `RELATIONSHIP_INCOMING_BLOCKED` | общий текст |
| 80002 | `RELATIONSHIP_INVALUD_USER_BOT` | общий текст (опечатка в имени — в клиенте) |
| 80003 | `RELATIONSHIP_INVALID_SELF` | общий текст |
| 80004 | `RELATIONSHIP_INVALID_DISCORD_TAG` | «пользователь не найден» |
| 80007 | `RELATIONSHIP_ALREADY_FRIENDS` | общий текст |
| 80013 | `RELATIONSHIP_INVALID_NO_CONFIRMATION` | диалог подтверждения незнакомца (§4.3) |
| 340007 | `USER_FRIEND_REQUEST_LIMITED_ACCESS` | ограничение аккаунта |

Общий обработчик `[D] @696700`:

```js
case 429: 0===t&&g({title:…,body:…,confirmText:…});break;
case 403: if(a===EMAIL_VERIFICATION_REQUIRED){…} break;
default:
  if(a===USER_QUARANTINED)(0,f.Z_)(),(0,o.default)();
  else if((0,u.O)(i,a))break;
  else if(a===RELATIONSHIP_INVALID_NO_CONFIRMATION)break;
  else if(0===t){let e=null!=n?(0,E.vU)(a||0,n):…;g({…})}
```

Важно: `429` показывает модалку только при `errorUxConfig === 0` (`SHOW_ALWAYS`), для
`SHOW_ONLY_IF_ACTION_NEEDED` — молчит.

## 5. Схемы объектов

### 5.1 Канал DM / GROUP_DM

Три источника одного типа: `READY.private_channels[]`, `CHANNEL_CREATE`, ответ
`POST /users/@me/channels`. Разбор — `private channel fromServer` (`[D] @8899327`).

| Поле wire | Тип | Обяз. | Внутреннее | Смысл |
|---|---|---|---|---|
| `id` | snowflake | да | `id` | id канала |
| `type` | int | да (дефолт 1) | `type` | `1` DM, `3` GROUP_DM |
| `recipients` | array пользователей | да для REST/CHANNEL_CREATE | `rawRecipients`, `recipients[]` id | участники, кроме себя |
| `recipient_ids` | array snowflake | да для READY | `recipients[]` id | то же в сжатом READY |
| `name` | string \| null | нет | `name` | имя группы |
| `icon` | string \| null | нет | `icon` | хеш иконки группы |
| `owner_id` | snowflake | нет | `ownerId` | владелец группы |
| `nicks` | map `id→nick` | нет | `nicks` | ники участников в группе |
| `last_message_id` | snowflake \| null | нет | `lastMessageId` | последнее сообщение |
| `last_pin_timestamp` | ISO-строка \| null | нет | `lastPinTimestamp` | последний пин |
| `flags` | int | нет | `flags_` | `ChannelFlags` (§5.3) |
| `is_message_request` | bool | нет | `isMessageRequest` | ЛС из запросов |
| `is_message_request_timestamp` | ISO \| null | нет | `isMessageRequestTimestamp` | когда стал запросом |
| `is_spam` | bool | нет | `isSpam` | помечен спамом |
| `recipient_flags` | int | нет | `recipientFlags` | биты §3.3 |
| `safety_warnings` | array \| null | нет | `safetyWarnings` | предупреждения безопасности |
| `blocked_user_warning_dismissed` | bool | нет | `blockedUserWarningDismissed` | скрыть предупреждение о блокировке |
| `application_id` | snowflake | нет | `application_id` | канал приложения |
| `guild_id` | — | **нет** | всегда `null` | у приватных каналов гильдии нет |

Различие источников (важно для тестов):

* `READY.private_channels[]` — **`recipient_ids`**, без `recipients`. Гидратор READY
  разворачивает их в объекты из `users[]` и **удаляет** `recipient_ids`
  (`[D] @4920048`):

  ```js
  a?.forEach(e=>{let t=e.recipient_ids;null!=t&&(e.recipients=t.map(e=>(
    eT()(null!=eL[e],"Missing user in compressed ready payload"),eL[e]))),delete e.recipient_ids});
  ```

* `CHANNEL_CREATE` и REST-ответ — **`recipients`** (объекты). Если сервер пришлёт
  `recipient_ids` в `CHANNEL_CREATE`, клиент получит пустой список участников.
  Для своего клиента безопаснее принимать оба поля.

В живом READY (`[D] gateway/by-type/002-READY.json`): 17 каналов, все `type:1`,
у всех `recipient_ids.length == 1`, `flags == 0`, `recipient_flags == 0`,
`last_pin_timestamp` есть у 2 из 17, `safety_warnings` — `null` у всех.
Групповых DM в захвате нет — форма GROUP_DM подтверждена кодом, но не данными.

### 5.2 Relationship

Три формы одного объекта:

| Источник | Ключи | Метка |
|---|---|---|
| `READY.relationships[]` | `id`, `user_id`, `type`, `nickname`, `since`, `is_spam_request`, `user_ignored` | `[D]` 60/60 |
| `GET /users/@me/relationships` | те же + `user` (объект), возможно `note`, `origin_application_id`, `is_stranger_request` | `[D]` для `user`, `[A]` для остальных |
| `RELATIONSHIP_ADD` / `UPDATE` | `id`, `type`, `user`, `since`, `nickname`, `is_spam_request`, `is_stranger_request`, `user_ignored`, `origin_application_id`, `note` (+ `should_notify` у ADD) | `[D] @4931266` |

В живом READY `id === user_id` у всех 60 записей, типы только `1` (18) и `2` (42),
`nickname` — `nil` у всех, `is_spam_request` и `user_ignored` — `false` у всех.
`since` — ISO-8601 с микросекундами (`2025-06-21T06:48:57.474000+00:00`).

`RelationshipStore` (`[D] @9318383`) держит по связи: `type`, `nickname`, `since`, `note`,
`originApplicationId`, `isSpamRequest`, `isStrangerRequest`, `userIgnored`; счётчики
pending/spam/ignored считаются из множеств, а не из полей.

### 5.3 `ChannelFlags` (для `flags`)

`[D] @3250152`:

```js
c=Object.freeze({GUILD_FEED_REMOVED:1,PINNED:2,ACTIVE_CHANNELS_REMOVED:4,REQUIRE_TAG:16,
  IS_SPAM:32,IS_GUILD_RESOURCE_CHANNEL:128,CLYDE_AI:256,IS_SCHEDULED_FOR_DELETION:512,
  IS_MEDIA_CHANNEL:1024,SUMMARIES_DISABLED:2048,IS_ROLE_SUBSCRIPTION_TEMPLATE_PREVIEW_CHANNEL:8192,
  IS_BROADCASTING:16384,HIDE_MEDIA_DOWNLOAD_OPTIONS:32768,IS_JOIN_REQUEST_INTERVIEW_CHANNEL:65536,
  OBFUSCATED:131072,IS_MODERATOR_REPORT_CHANNEL:524288,IS_SPOILER_CHANNEL:2097152,
  IS_GAME_INVITES_CHANNEL:4194304,HAS_ONLY_SYSTEM_MESSAGES:8388608})
```

Для DM практически значимы `PINNED` (2) — закреплённый чат в списке ЛС — и `IS_SPAM` (32);
остальные биты относятся к гильдейским каналам.

## 6. Gateway: READY

### 6.1 `private_channels`

`[D] gateway-schema.md:161`, `:208-220`; `gateway-protocol.md:175,184`.

| Поле | Тип (ETF) | Присутствие | Пример |
|---|---|---|---|
| `flags` | int | 17/17 | `0` |
| `id` | int64 (bignum) | 17/17 | `696738350706458685` |
| `is_message_request` | bool | 17/17 | `false` |
| `is_message_request_timestamp` | null | 17/17 | `nil` |
| `is_spam` | bool | 17/17 | `false` |
| `last_message_id` | int64 \| null | 17/17 | `1558137543688720415` |
| `last_pin_timestamp` | string | 2/17 | `2021-03-17T13:34:40+00:00` |
| `recipient_flags` | int | 17/17 | `0` |
| `recipient_ids` | array int64 | 17/17 | `["315473023476170753"]` |
| `safety_warnings` | null | 17/17 | `null` |
| `type` | int | 17/17 | `1` |

Размер секции — 10 644 Б JSON (по `gateway-protocol.md`). У всех 17 записей
`recipient_ids` ссылаются на пользователей из `READY.users[]` (проверено, 0 пропусков).

### 6.2 `relationships`

`[D] gateway-schema.md:136`, `:382`.

| Поле | Тип | Присутствие | Пример |
|---|---|---|---|
| `id` | snowflake | 60/60 | `194374444821643266` |
| `user_id` | snowflake | 60/60 | совпадает с `id` |
| `type` | int | 60/60 | `1` ×18, `2` ×42 |
| `nickname` | string \| nil | 60/60 | `nil` |
| `since` | ISO-строка | 60/60 | `2025-06-21T06:48:57.474000+00:00` |
| `is_spam_request` | bool | 60/60 | `false` |
| `user_ignored` | bool | 60/60 | `false` |

`note`, `origin_application_id`, `is_stranger_request`, `should_notify` в захвате не
встретились (нет ни одной заявки/заметки) — их наличие в объекте доказано только
обработчиком диспатча (`[D] @4931266`).

Top-level `friend_suggestion_count` = `0` (`[D]` там же), `game_relationships` = `null`.

### 6.3 `READY_SUPPLEMENTAL`

`[D] gateway/by-type/003-READY_SUPPLEMENTAL.json`: ключи `disclose`, `game_invites`,
`guilds`, `lazy_private_channels`, `merged_members`, `merged_presences`, `user_activities`.
`lazy_private_channels` в этом захвате — пустой массив (0 записей). Клиент разворачивает
его так же, как `private_channels`, и кладёт в `initialPrivateChannels`
(`[D] @4918565`: `lazyPrivateChannels:i`).

### 6.4 Как клиент собирает состояние

`[D] @4921010` (`READY` → `CONNECTION_OPEN`):

```js
eW({type:"CONNECTION_OPEN",sessionId:e.session_id,…,user:e.user,users:e.users,guilds:a,
  initialPrivateChannels:t,unavailableGuilds:i,readState:e.read_state,
  userGuildSettings:e.user_guild_settings,tutorial:e.tutorial,
  relationships:e.relationships??[],gameRelationships:e.game_relationships,
  friendSuggestionCount:e.friend_suggestion_count,…})
```

`RelationshipStore` (`[D] @9320846`):

```js
CONNECTION_OPEN:function(e){d.clear(),v.clear(),c={},u={},_={},A.clear(),E.clear(),h.clear(),
  R(),f={},p={},e.relationships.forEach(e=>{b(e.id,e.type),
    null!=e.nickname&&(c[e.id]=e.nickname),null!=e.since&&(u[e.id]=e.since),
    null!=e.note&&(_[e.id]=e.note),e.is_spam_request&&E.add(e.id),
    null!=e.origin_application_id&&(f[e.id]=e.origin_application_id),
    e.user_ignored&&(y(e.id),e.type===PENDING_INCOMING&&h.add(e.id))}),L(),P()}
```

То есть READY **заменяет** состояние целиком (clear + заполнение), а не мержит.
`initialPrivateChannels` — тоже полная замена списка ЛС.

## 7. Dispatch-события

### 7.1 `CHANNEL_CREATE` / `CHANNEL_DELETE`

`[D] @4924901`:

```js
eH(["CHANNEL_CREATE","CHANNEL_DELETE"],e=>f.D.loadGuildIds([e.guild_id]),
  (e,t)=>{eW({type:t,channel:(0,ed.UE)(e)})})
```

Payload — **сам объект канала**, без обёртки. Внутри `createChannelRecord` тип
диспетчеризуется по `type`: `1` и `3` → приватный канал (`[D] @8903690`, фабрика
`@8904343`). Для DM/группы `guild_id` отсутствует; `loadGuildIds([undefined])` — no-op.

Клиент также генерирует это событие локально после `POST /users/@me/channels`
(`[D] @568882`, `@569241`). Шлёт ли сервер `CHANNEL_CREATE` создателю той же ЛС — `[A]`,
но в Discord приходит и ему, поэтому стор обязан быть идемпотентным по `id`.

Какие поля сервер шлёт в `CHANNEL_CREATE` для DM — `[A]`; минимум, который читает клиент,
совпадает с REST-ответом (§5.1): `id`, `type`, `recipients`.

### 7.2 `CHANNEL_RECIPIENT_ADD` / `CHANNEL_RECIPIENT_REMOVE`

`[D] @4926913`:

```js
eV(["CHANNEL_RECIPIENT_ADD","CHANNEL_RECIPIENT_REMOVE"],(e,t)=>{
  let n=f.A.getBasicChannel(e.channel_id);
  eW({type:t,channelId:e.channel_id,user:e.user,nick:e.nick,isMember:null!=n})})
```

| Поле wire | Тип | Смысл |
|---|---|---|
| `channel_id` | snowflake | группа |
| `user` | объект пользователя | добавленный/удалённый |
| `nick` | string \| null | ник в группе |

`isMember` — локальное вычисление (есть ли канал в сторе), не поле провода.

Стор private channels (`[D] @9022143`) применяет событие только к приватным
каналам: `!!t?.isPrivate()&&($(t.addRecipient(e.user.id,e.nick,n)),!0)`.

### 7.3 `RELATIONSHIP_ADD` / `RELATIONSHIP_UPDATE` / `RELATIONSHIP_REMOVE`

`[D] @4931266`:

```js
eV(["RELATIONSHIP_ADD"],e=>{eW({type:"RELATIONSHIP_ADD",relationship:{
  id:e.id,type:e.type,user:e.user,since:e.since,nickname:e.nickname,
  isSpamRequest:e.is_spam_request||!1,isStrangerRequest:e.is_stranger_request||!1,
  userIgnored:e.user_ignored||!1,originApplicationId:e.origin_application_id,note:e.note},
  shouldNotify:!0===e.should_notify})}),
eV(["RELATIONSHIP_REMOVE"],e=>{eW({type:"RELATIONSHIP_REMOVE",relationship:e})}),
eV(["RELATIONSHIP_UPDATE"],e=>{eW({type:"RELATIONSHIP_UPDATE",relationship:{
  id:e.id,type:e.type,user:e.user,nickname:e.nickname,since:e.since,
  isSpamRequest:e.is_spam_request||!1,isStrangerRequest:e.is_stranger_request||!1,
  userIgnored:e.user_ignored||!1,originApplicationId:e.origin_application_id,note:e.note}})})
```

`RELATIONSHIP_REMOVE` передаётся как есть; стор читает `id`, `userIgnored`
(`[D] @9322202`), поэтому на проводе обязательны минимум `id` и `user_ignored`,
а `user` — желателен для UI.

Побочный эффект стора: переход `PENDING_OUTGOING → FRIEND` порождает
`FRIEND_REQUEST_ACCEPTED` с объектом пользователя (`[D] @9322065`):

```js
e.relationship.type===FRIEND&&t===PENDING_OUTGOING&&r.h.dispatch({
  type:"FRIEND_REQUEST_ACCEPTED",user:e.relationship.user})
```

### 7.4 Сводная таблица диспатчей

| Событие | Направление | Поля payload | Внутреннее действие |
|---|---|---|---|
| `CHANNEL_CREATE` | server→client | объект канала (`id`, `type`, `recipients`, …) | `CHANNEL_CREATE {channel}` |
| `CHANNEL_DELETE` | server→client | объект канала (минимум `id`) | `CHANNEL_DELETE {channel}` |
| `CHANNEL_RECIPIENT_ADD` | server→client | `channel_id`, `user`, `nick` | добавить участника в группу |
| `CHANNEL_RECIPIENT_REMOVE` | server→client | `channel_id`, `user`, `nick` | убрать участника |
| `RELATIONSHIP_ADD` | server→client | relationship + `should_notify` | `RELATIONSHIP_ADD {relationship,shouldNotify}` |
| `RELATIONSHIP_UPDATE` | server→client | relationship | `RELATIONSHIP_UPDATE {relationship}` |
| `RELATIONSHIP_REMOVE` | server→client | `id`, `user_ignored`, (`user`, `type`) | `RELATIONSHIP_REMOVE {relationship}` |

### 7.5 Чего в захвате нет

`gateway/summary.json` — 124 кадра, 12 типов событий: `READY`, `READY_SUPPLEMENTAL`,
`SESSIONS_REPLACE`, `THREAD_LIST_SYNC`, `GUILD_MEMBER_LIST_UPDATE`, `GUILD_MEMBERS_CHUNK`,
`PRESENCE_UPDATE`, `MESSAGE_CREATE`, `MESSAGE_UPDATE`, `MESSAGE_ACK`, `HELLO`,
`HEARTBEAT_ACK`. Ни `CHANNEL_CREATE`, ни `CHANNEL_RECIPIENT_*`, ни `RELATIONSHIP_*` в окне
захвата (2026-10-19, ~592 с) не произошло — контракт этих событий подтверждён только
обработчиками клиента, а не данными. Это единственная содержательная дыра в разделе.

## 8. Что это значит для N1-07 (без правок кода)

Состояние репозитория на `HEAD 4e24048`:

* READY-разбор ЛС и связей уже есть: `rust/crates/discord-core/src/ready/social.rs`
  (`PrivateChannel` — 10 полей, `Relationship` — 7 полей), редуктор
  `state/types.rs` (`relationships: BTreeMap<u64, Relationship>`) и `ready/snapshot.rs`.
  Это покрывает §6.1–6.2.
* REST-каталог `rust/crates/discord-core/src/rest/routes.rs` содержит 47 маршрутов D-02
  плюс три `[S]`-расширения (`TYPING_PATH`, `INVITE_PATH`). **Ни одного** маршрута из §1.1
  там нет: `grep relationships|@me/channels|recipients` по `routes.rs` — пусто.
* `DispatchKind` (`rust/crates/discord-core/src/dispatch.rs`) знает 21 вариант, но
  `CHANNEL_CREATE`, `CHANNEL_DELETE`, `CHANNEL_RECIPIENT_*`, `RELATIONSHIP_*` в нём
  отсутствуют — §7 целиком не реализован.

Минимальный объём N1-07:

1. **REST**: 6 маршрутов для «открыть DM / создать группу / список друзей / заявка /
   принять-блок / удалить», остальное — по мере UI. Регистрировать через `EXTENSIONS`
   (как `INVITE_PATH`), не размывая frozen 47.
2. **Dispatch**: 7 новых `DispatchKind` (по одному на событие §7.4) + разбор payload.
3. **Состояние**: идемпотентная вставка канала по `id` (локальный и шлюзовой
   `CHANNEL_CREATE` дублируются), замена списка связей на `READY`, инкремент счётчиков
   pending/spam/ignored по §5.2.
4. **Модель**: `PrivateChannel` не хватает `name`, `icon`, `owner_id`, `nicks`,
   `safety_warnings`, `blocked_user_warning_dismissed` — без них группа не отрисуется.
   `Relationship` не хватает `note`, `origin_application_id`, `is_stranger_request`.

## 9. Оракулы для тестов

Живые данные (read-only, `reference-oracle/gateway/by-type/002-READY.json`):

```bash
# 17 ЛС, все type=1
jq -c '[.d.private_channels[]|.type]|group_by(.)|map({type:.[0],n:length})' \
  reference-oracle/gateway/by-type/002-READY.json
# => [{"type":1,"n":17}]

# 60 связей: 18 друзей, 42 блока
jq -c '[.d.relationships[]|.type]|group_by(.)|map({type:.[0],n:length})' \
  reference-oracle/gateway/by-type/002-READY.json
# => [{"type":1,"n":18},{"type":2,"n":42}]

# id == user_id у всех 60
jq '[.d.relationships[]|select(.id!=.user_id)]|length' \
  reference-oracle/gateway/by-type/002-READY.json
# => 0

# у всех ЛС ровно один получатель
jq '[.d.private_channels[]|select((.recipient_ids|length)!=1)]|length' \
  reference-oracle/gateway/by-type/002-READY.json
# => 0

# все получатели и все друзья есть в users[]
jq '(.d.users|map(.id)|unique) as $u
  | ([.d.relationships[].user_id,.d.private_channels[].recipient_ids[]]
     | map(select(. as $x | ($u|index($x))|not)) | length)' \
  reference-oracle/gateway/by-type/002-READY.json
# => 0
```

Статический индекс маршрутов:

```bash
jq -r '.endpoints[]
  | select(.key=="USER_CHANNELS" or .key=="USER_RELATIONSHIPS"
        or .key=="USER_RELATIONSHIP" or .key=="CHANNEL_RECIPIENT_ME")
  | [.key,.method,.path,.callSiteCount,(.bodyKeys|join("|"))] | @tsv' \
  reference-oracle/rest-static.json
```

Ожидаемо: `USER_CHANNELS POST /users/@me/channels 2 {recipients}`,
`USER_RELATIONSHIPS DELETE|GET|POST /users/{arg}/relationships 5 {username,discriminator,note}|{filters}`,
`USER_RELATIONSHIP DELETE|PATCH|PUT /users/@me/relationships/{arg} 3 {type,from_friend_suggestion,confirm_stranger_request}|{nickname}`,
`CHANNEL_RECIPIENT_ME DELETE|PATCH|PUT /channels/{arg}/recipients/@me 5 {flags}|{consent_status}|{channel_ids}`.

Негативные проверки (контракт «чего быть не должно»):

* в теле `POST /users/@me/channels` нет `recipient_id` — только `recipients`;
* `GET /users/@me/channels` не вызывается клиентом (0 call-site);
* при принятии заявки `type` может отсутствовать — клиент не шлёт его (см. §4.3);
* `RELATIONSHIP_ADD` без `user` не должен ронять разбор — только UI-часть.

## 10. Остаточные gap

| Пункт | Статус | Что нужно |
|---|---|---|
| `GET /users/@me/channels` — форма ответа и коды | `[A]` | живой вызов или решение не реализовывать |
| Точные ключи captcha в теле заявки | `[A]` | захват окна «Добавить в друзья» с капчей |
| `PUT` без `type` при принятии заявки | `[A]` | проверить на живом аккаунте; безопасный вариант — всегда `type:1` |
| Полный enum `consent_status` | `[?]` | найдены `ACCEPTED`, `UNSPECIFIED`, `PENDING`; остальное — ленивый чанк |
| Серверные лимиты: получателей в группе, друзей, блокировок | `[A]` | только коды (`30004`, `30002`, `30059`), чисел в клиенте нет |
| `CHANNEL_CREATE`/`RELATIONSHIP_*` в проводе | `[A]` | живой захват: открыть ЛС, добавить в друзья, принять заявку |
| GROUP_DM в READY | `[A]` | нет ни одной группы в захвате; форма выведена из `fromServer` |
| `should_notify` — поведение UI | `[A]` | поле читается, но обработчика в бандле не найдено |

## 11. Артефакты

| Файл | Размер | md5 |
|---|---|---|
| `reference-oracle/decoded/4d2a64cb5df5fc4e_0` (`web.4d4b2f75fdea5470.js`) | 12 410 287 | `934b1289b7ebf17bc0fd09569b23b438` |
| `reference-oracle/decoded/d38b37e4d52b4495_0` (`07cac0073fc4f6e3.js`) | 243 622 | `6d955e8002cd78eff7a020e0e91d6c37` |
| `reference-oracle/decoded/e1affb550dd48702_0` (`e51d4232a1a23440.js`) | 1 005 | `db8743b8759449f5f9490be90c7158a9` |
| `reference-oracle/decoded/9f6dccfb41eb44b6_0` (чанк `229787`, заметки) | 716 | `d1d33c3c0b1070c5790b0307cdbcd826` |
| `reference-oracle/decoded/a8de0187058c21c2_0` (модуль `629403`, `updateNote`) | 152 376 | `7e479cd685aadd7998ab0e5a2cbdf54d` |
| `reference-oracle/gateway/by-type/002-READY.json` | 579 438 | `5e1e4c2c9ee5c5a0b1495335dde555f5` |
| `reference-oracle/gateway/by-type/003-READY_SUPPLEMENTAL.json` | 50 869 | `991264101aed0cb088ef077abb3ab696` |
| `reference-oracle/gateway/ready-stats.json` | 1 221 | `972065245b4f215b76314fbd7e89b573` |
| `reference-oracle/gateway-schema.md` | — | `1b6615d8d6524e9cd1339775a484a0bb` |
| `reference-oracle/gateway-protocol.md` | — | `a766e52a585faf5eba2bf2dd2cde8abf` |
| `reference-oracle/rest-static.json` | — | `8e0ead46addbb2f54f143343d0de2108` |

Бандл 1.0.162 — тот же, что в `reference-oracle-REA` §10; живой бандл 2026-10-30
(`web.b2b84438f3c83e84.js`) для этого контракта не привлекался: экран логина не грузит
ленивые чанки друзей, а статический дамп содержит все call-sites.

## 12. Индекс ссылок

Смещения — в `decoded/4d2a64cb5df5fc4e_0`, если не указано иное.

| Что | Смещение |
|---|---|
| `RelationshipType` (0–6) | `@258005` |
| `USER_RELATIONSHIPS` / `USER_RELATIONSHIP` / `USER_BULK_RELATIONSHIPS` | `@258630` |
| `USER_CHANNELS` / bulk-leave / group-dm-shell | `@259640` |
| `DM_CHANNEL` | `@260280` |
| `CHANNEL` | `@266725` |
| `CHANNEL_RECIPIENT*` | `@270359` |
| `FRIEND_SUGGESTIONS` / `FRIEND_SUGGESTION` | `@271460` |
| `FRIEND_INVITES` | `@272930` |
| `NOTE` | `@273990` |
| `IGNORE_USER` | `@310495` |
| `openPrivateChannel` (POST `/users/@me/channels`) | `@567585` |
| `createGroupDmShell` | `@568215` |
| `ensurePrivateChannel` | `@568882` |
| `getDMChannel` (`GET /users/@me/dms/{id}`) | `@569241` |
| `_getRecipients` | `@569410` |
| `closePrivateChannel` (`DELETE /channels/{id}?silent`) | `@569652` |
| `bulkLeaveGroupDMs` | `@570274` |
| `addRecipient` / `removeRecipient` / `setDMOwner` / `setName` / `setIcon` | `@570912`–`@571778` |
| `sendRequest` | `@697487` |
| `addRelationship` | `@697776` |
| `removeRelationship` | `@698771` |
| `updateRelationship` | `@698967` |
| `fetchRelationships` | `@699083` |
| `clearPendingRelationships` | `@699318` |
| `clearPendingSpamAndIgnored` | `@699593` |
| `ignoreUser` / `unignoreUser` | `@699907` / `@700264` |
| `RelationshipFilters` (`SPAM=1`, `IGNORED=2`) | `@7346275` |
| READY: разворот `recipient_ids` | `@4920048` |
| READY → `CONNECTION_OPEN` (`initialPrivateChannels`, `relationships`) | `@4921010` |
| `CHANNEL_CREATE`/`CHANNEL_DELETE` handler | `@4924901` |
| `CHANNEL_RECIPIENT_ADD/REMOVE` handler | `@4926913` |
| `RELATIONSHIP_ADD/UPDATE/REMOVE` handler | `@4931266` |
| `private channel fromServer` | `@8899327` |
| карта классов каналов (`DM`/`GROUP_DM` → приватный) | `@8903690`, фабрика `@8904343` |
| `RelationshipStore` | `@9318383` |
| `RelationshipStore.CONNECTION_OPEN` | `@9320846` |
| `ChannelType` (`DM=1`, `GROUP_DM=3`) | `@11855584` |
| `PRIVATE_CHANNEL = {1,3}` | `@11856265` |
| `ChannelFlags` | `@3250152` |
| `RecipientFlags` (`1`, `2`) | `decoded/d38b37e4d52b4495_0` `@243430` |
| `consent_status` (`ACCEPTED`/`UNSPECIFIED`/`PENDING`) | `decoded/e1affb550dd48702_0` `@311` |
| заметки: GET + проверка `note_user_id` | `decoded/9f6dccfb41eb44b6_0` `@493` |
| заметки: `updateNote` | `decoded/a8de0187058c21c2_0` `@161` |
