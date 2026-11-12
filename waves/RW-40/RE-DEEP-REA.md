# reference-oracle-REA — недостающие контракты по дампу и изолированной сессии

Дата: 2026-10-30. Карточка: reference-oracle-REA. Ветка: `RW-40`.
Продуктовый код не менялся. Затронуты только документы этой волны.

Метки: `[D]` — цитата декодированного статического дампа `reference-oracle/`;
`[S]` — цитата скрипта, снятого из изолированной сессии (Live) или статической схемы;
`[A]` — не подтверждено, остаётся gap.

## 0. Главное

| STUB | Было | Стало | Что дало |
|---|---|---|---|
| STUB-13 вложения | blocked, «endpoint/multipart/лимиты — gap» | **подтверждён полностью** | `POST /channels/{id}/attachments` → `{files:[...]}` → `upload_url` → `PUT` → `uploaded_filename` в `POST /channels/{id}/messages` |
| STUB-14 поиск | только 5 параметров | **все фильтры + пагинация + 202-retry** | таблица `queryKey`, `SearchFetcher`, `limit=25`, `sort_by/sort_order` |
| STUB-03 create server | «endpoint требует RE» | **частично**: маршрут есть, тела запроса в дампе нет | `GUILDS:"/guilds"`, call-site отсутствует → gap |
| STUB-05/07 инвайты | коды `[A]` | **подтверждено** | тела `POST /invites/{code}`, `POST /channels/{id}/invites`, коды ошибок |
| STUB-12 gift 🎁 | «нужен RE» | **подтверждено**: обработчик, модалка, endpoint | `openGiftModal` → `POST /users/@me/entitlements/gift-codes` |
| STUB-27 Go Live | `STREAM_*` — `[A]` | **подтверждено** | op 18/19/20/21/22 + поля всех четырёх пакетов |

Одна поправка к прежним выводам: вложения были помечены `blocked` из-за артефакта
экстрактора, а не из-за отсутствия кода. `rest-static.json` считает call-site по прямой
подстановке `Rsh.*` в вызов (`callSiteCount: 0`), а upload идёт через класс
`MessageAttachmentUploadTarget.getCreateAttachmentURL()`, поэтому связь потерялась.
Код в дампе есть — см. §1.

## 1. STUB-13 — вложения (B2 Attachments Upload)

### 1.1 Маршрут

`[D] 4d2a64cb5df5fc4e_0@269215`

```js
MESSAGE_CREATE_ATTACHMENT_UPLOAD:e=>`/channels/${e}/attachments`,UPDAT…
```

Имя `CHANNEL_MESSAGE_ATTACHMENTS` в клиенте не используется: константа называется
`MESSAGE_CREATE_ATTACHMENT_UPLOAD`. Путь совпадает с заданием.

Цель загрузки `[D] 4d2a64cb5df5fc4e_0@2587202`:

```js
class u{getCreateAttachmentURL(e){return s.Rsh.MESSAGE_CREATE_ATTACHMENT_UPLOAD(e)}
getDeleteUploadURL(e){return s.Rsh.MESSAGE_DELETE_UPLOAD(e)}
getMaxFileSize(e){let t=d.A.getBasicChannel(e);return(0,o.C)(c.o2(t?.guild_id))}
getMaxAttachmentsCount(){return s.XgB}
getMaxTotalAttachmentSize(){return(0,a.jS)({location:"MessageAttachmentUploadTarget"})}
```

### 1.2 Запрос за upload_url

`[D] 4d2a64cb5df5fc4e_0@2573535`

```js
let e=await this.trackTime("getUploadUrlTimeMs",async()=>{
  let e=r.getCreateAttachmentURL(this.channelId);
  return await o.Bo.post({url:e,body:{files:[n]},headers:this.buildOriginalMd5Headers(),
    ...this.createAttachmentUrlRetryOpts(),rejectWithError:!1})});
this.setResponseUrl(e.body.attachments[0].upload_url),
this.setUploadedFilename(e.body.attachments[0].upload_filename)
```

Итог: `POST /channels/{channel_id}/attachments`, тело `{"files":[<метаданные>]}`,
ответ `{"attachments":[{"upload_url":…,"upload_filename":…}]}`.
Больше в ответе ничего не читается.

### 1.3 Структура `files[]`

`[D] 4d2a64cb5df5fc4e_0@2572444`, модуль `122924`:

```js
getUploadPayload:function(e){let t=e.item.file;
  r()(null!=t,"file data must not be empty"),
  {filename:t.name,file_size:t.size,id:s().uniqueId(),is_clip:null!=e.clip,
   title:e.clip?.name,application_id:e.clip?.applicationId,
   clip_created_at:null!=e.clip?(0,l.Uq)(e.clip.createdAt):void 0,
   clip_participant_ids:null!=e.clip?(0,l.gD)(e.clip.users):void 0,
   clip_remote_id:e.clip?.remoteClipId,
   clip_events_timeline:null!=e.clip?(0,l.lz)(e.clip):void 0,
   clip_sync_timestamp:null!=e.clip?(0,l.Ae)(e.clip):void 0,
   original_content_type:e.mimeType}}
```

Обязательные поля ровно те, что в задании: `filename`, `file_size`, `id`, `is_clip`.
`id` — локальный UUID (`uniqueId()`), не серверный.

Заголовок `[D] 4d2a64cb5df5fc4e_0@2598776`:

```js
return null==n?t:{...t,"X-Discord-Original-MD5":n}
```

### 1.4 PUT на upload_url

`[D] 4d2a64cb5df5fc4e_0@2567497` (`uploadFileToCloud`):

```js
this.item.platform===A.x.REACT_NATIVE
  ? t=null!=(e={type:this.item.mimeType,uri:this.item.uri,name:this.item.filename}).type
      &&"application/json"!==e.type ? e.type : "application/octet-stream"
  : (e=this.item.file,t="application/octet-stream")
await this.uploadFileWithResumption(this.responseUrl,e,t)
```

`[D] 4d2a64cb5df5fc4e_0@2568630` (`startOrResumeUpload`):

```js
if(t>0) … e.headers={...e.headers??{}, "Content-Range":`bytes ${t}-${this.currentSize-1}/${this.currentSize}`}
let n=await this._uploadHttpClient.doUpload(e,{fileByteRange:{start:t}})
if(200!==n.status&&201!==n.status)throw new b("upload",{response:n})
```

Докачка `[D]`: `getResumePosition` → `GET {upload_url}` с `Content-Range: bytes */*`,
ответ `200/201` = файл уже целиком, `308` = нужен `Range`-ответ и продолжение с `t[1]+1`.
Таймаут: `1 час`, `retries: 12` (`retryOpts`), для `createAttachmentUrl` — те же 12.

### 1.5 Лимиты

| Что | Значение | Метка |
|---|---|---|
| Максимум вложений в сообщении | `10` (`n4=10`) | `[D] 4d2a64cb5df5fc4e_0@314301` |
| Базовый лимит на файл | `Math.max(0x1400000, tierLimit)` = не ниже **20 МиБ** | `[D] 4d2a64cb5df5fc4e_0@6011428` |
| Guild tier NONE / TIER_1 | `10 МиБ` (`L.TbF = n7 = 0xa00000`) | `[D] @314301`, `@6804500` |
| Guild tier TIER_2 | `50 МиБ` (`0x3200000`) | `[D] @6805978` |
| Guild tier TIER_3 | `100 МиБ` (`0x6400000`) | `[D] @6804500` |
| Суммарно на сообщение | `500 МБ` (`524288e3`), либо `1 ГиБ` (`0x40000000`) при включённом эксперименте | `[D] 4d2a64cb5df5fc4e_0@9868110` |
| Продуктовые вложения (аналог) | `10` файлов, `1 ГиБ` каждый | `[D] @5347570` |

`[D] 4d2a64cb5df5fc4e_0@9868110`:

```js
function o(e){let{location:t}=e;return(0,a.J6)({location:t}).enabled?0x40000000:524288e3}
```

Проверка количества перед отправкой (`[D] 23e2050b01c07deb_0@…`):

```js
if(h.A.getUploadCount(t.id,i)+z.length>S.XgB){…openUploadError({title:…wOr6hB…})…}
```

### 1.6 Коды 413 / 429

`[D] 4d2a64cb5df5fc4e_0@2572800` (предпроверка до сети):

```js
if(0===this.currentSize)return void this.handleError(y.t02.ENTITY_EMPTY);
let a=r.getMaxFileSize(this.channelId);
if((this.currentSize??0)>a)return void this.handleError(y.t02.ENTITY_TOO_LARGE);
```

`[D]`, после ответа:

```js
catch(t){let e=t?.body?.code??t.status;
  e!==y.t02.ENTITY_TOO_LARGE&&(D.error(`Requesting upload url failed with code …`),…),
  this.handleError(e);return}
```

Коды `[D] 4d2a64cb5df5fc4e_0@322893` и рядом:

| Код | Имя |
|---|---|
| 40005 | `ENTITY_TOO_LARGE` |
| 40006 | `ENTITY_EMPTY` |
| 40007 | `USER_BANNED` |
| 50035 | `INVALID_FORM_BODY` |
| 20016 | `SLOWMODE_RATE_LIMITED` |
| 20022 | `CHANNEL_FOLLOWING_EDIT_RATE_LIMITED` |

429 обрабатывается на уровне отправки сообщения `[D] 4d2a64cb5df5fc4e_0@2361900`:

```js
else if(429===n.status){let e=parseInt(n.headers["retry-after"]);
  isNaN(e)?t(null,n):t({retryAfter:e*p.A.Millis.SECOND})}
```

Отдельного «413» для upload в клиенте нет: сервер отдаёт `40005` в `body.code`,
а клиент дополнительно режет размер локально.

### 1.7 Передача `uploaded_filename` в сообщение

`[D] 4d2a64cb5df5fc4e_0@667084`:

```js
null!=r&&(ec.message.attachments=r.map((e,t)=>(0,ex.OW)(e,t)))
```

Сборщик `OW` `[D] 4d2a64cb5df5fc4e_0@9867091`, модуль `292348`:

```js
function d(e){let t=arguments.length>1&&void 0!==arguments[1]?arguments[1]:null,
  n=arguments.length>2?arguments[2]:void 0,a={id:t?.toString()??e.id};
  null!=e.description&&(a.description=e.description),
  a.filename=n??e.filename,
  a.uploaded_filename=e.uploadedFilename,
  e.spoiler&&(a.is_spoiler=!0),
  "durationSecs"in e&&null!=e.durationSecs&&(a.duration_secs=e.durationSecs),
  "waveform"in e&&null!=e.waveform&&(a.waveform=e.waveform),
  "isThumbnail"in e&&!0===e.isThumbnail&&(a.is_thumbnail=e.isThumbnail),
  …clip…,"item"in e&&…&&(a.original_content_type=e.mimeType),a}
```

Отправка `[D] 4d2a64cb5df5fc4e_0@2362936`:

```js
l.Bo.post({url:A.Rsh.MESSAGES(n),body:o,context:…,oldFormErrors:!0,…N.ZG,signal:u.signal,rejectWithError:!0},d)
```

где `o = {mobile_network_type, ...r, signal_strength?}`, а `r` — это
`{type:"SEND"|"SEND_ANNOUNCEMENT", message:{attachments:[…]}, analyticsLocation}`.
Маршрут сообщений `[D] @268972`: `MESSAGES:e=>\`/channels/${e}/messages\``.

Обратная сторона (scheduled messages) `[D] @7532506` подтверждает имя поля в JSON:
`attachment_uploads[].uploaded_filename → uploadedFilename`.

### 1.8 Полный порядок

1. `POST /channels/{id}/attachments` с `{"files":[{filename,file_size,id,is_clip,…}]}` (+`X-Discord-Original-MD5`).
2. Ответ: `attachments[0].upload_url`, `attachments[0].upload_filename`.
3. `PUT {upload_url}` с сырым телом файла, `Content-Type` = mime файла или `application/octet-stream`; докачка через `Content-Range`, `308`.
4. `POST /channels/{id}/messages` с `message.attachments[].uploaded_filename = upload_filename`.

Этого достаточно, чтобы снять `blocked` с B2 на уровне контракта. Реализация
сжатия/ресайза (`compressAndExtractData`) — отдельная задача, для паритета не нужна.

## 2. STUB-14 — фильтры поиска (M-13)

### 2.1 Маршруты

`[D] @279632`, `[D] @280974`:

```js
SEARCH_GUILD:e=>`/guilds/${e}/messages/search`,
SEARCH_TABS_GUILD:e=>`/guilds/${e}/messages/search/tabs`,
SEARCH_CHANNEL:e=>`/channels/${e}/messages/search`,
SEARCH_TABS_CHANNEL:e=>`/channels/${e}/messages/search/tabs`,
SEARCH_TABS_DMS:"/users/@me/messages/search/tabs",
```

### 2.2 Как строится запрос

`[D] 82a5af57ee11dcd5_0` (`0a6ed6a6830e5071.js`), класс `SearchFetcher`:

```js
makeRequest(e){let{rejectWithError:t}=e,s=this.getEndpoint();
  return null==s?null:h.Bo.get({url:s,query:r.stringify(this.query),oldFormErrors:!0,rejectWithError:t})}
```

GET, не POST. Параметры сериализуются в query-string.

Ответы `[D]` того же файла:

```js
if(200===r.status)e(r);
else if(202===r.status){if(this.query.attempts=(this.query.attempts??0)+1,this.query.attempts>5)return;
  let h=parseInt(r.headers["retry-after"]);
  this.retryDelay=isNaN(h)||0===h?5e3:h*l.A.Millis.SECOND,this.retryLater(e,t,s),t(r)}
```

То есть `202` = «глубокая индексация идёт», максимум 5 повторов, задержка
`retry-after` (по умолчанию 5000 мс).

### 2.3 Точные query-параметры

Источник — таблица фильтров `[D] 4d2a64cb5df5fc4e_0@7564745…7569200`.
`queryKey` — имя REST-параметра, `ANSWER_*` — значение после `from:`/`in:`/`has:` и т. п.

| Фильтр UI | Токен | `queryKey` | Значение |
|---|---|---|---|
| Автор | `from:` | `author_id` | id пользователя, `Set` → массив |
| Упоминания | `mentions:` | `mentions` | id пользователя, массив |
| Тип вложения | `has:` | `has` | `link, embed, poll, snapshot, file, video, image, sound, sticker` |
| Канал | `in:` | `channel_id` | id канала, массив |
| Ссылка с домена | `link from:` | `link_hostname` | строка |
| Расширение файла | `file type:` | `attachment_extension` | строка |
| Имя файла | `file name:` | `attachment_filename` | строка |
| Закреплённые | `pinned:` | `pinned` | `true` / `false` |
| Тип автора | `author type:` | `author_type` | `user, bot, webhook` |
| Дата (до/включительно/после) | `before:`/`on:`/`after:` | `min_id`, `max_id` | snowflake из timestamp |
| Произвольный текст | — | `content` | строка |

`[D] @7564745`:

```js
[D.LWr.ANSWER_USERNAME_FROM]:{follows:[D.LWr.FILTER_FROM],…,queryKey:"author_id"},
[D.LWr.ANSWER_USERNAME_MENTIONS]:{follows:[D.LWr.FILTER_MENTIONS],…,queryKey:"mentions"},
[D.LWr.ANSWER_HAS]:{…,queryKey:"has"},
[D.LWr.ANSWER_LINK_FROM]:{…,queryKey:"link_hostname"},
[D.LWr.ANSWER_FILE_TYPE]:{…,queryKey:"attachment_extension"},
[D.LWr.ANSWER_FILE_NAME]:{…,queryKey:"attachment_filename"},
[D.LWr.ANSWER_IN]:{…,queryKey:"channel_id"},
[D.LWr.ANSWER_PINNED]:{…,queryKey:"pinned"},
[D.LWr.ANSWER_AUTHOR_TYPE]:{…,queryKey:"author_type"}
```

Дата `[D] @7550420`:

```js
case T.LWr.ANSWER_BEFORE:case T.LWr.ANSWER_ON:case T.LWr.ANSWER_AFTER:
  let a=e.getData("start"),s=e.getData("end");
  a&&(t.min_id=h.default.fromTimestamp(a)),s&&(t.max_id=h.default.fromTimestamp(s));return
```

Значения `has` `[D] @7559957`:

```js
{[v.intl.string(v.t.ZNR2fi)]:"link",…:"poll",…:"snapshot",…:"file",
 …:"video",…:"image",…:"sound",…:"sticker"}
```

Значения `author_type` `[D]` там же: `{"user":"user","bot":"bot","webhook":"webhook"}`.
Отрицание через `-`: `-has:file`, `-from:user` (`[D] @7559957`, `$()` с `startsWith("-")`).

### 2.4 Сортировка, лимит, offset

`[D] 4d2a64cb5df5fc4e_0@7542421`, модуль `768570`:

```js
let d="SearchStore",c="SearchHistoryStore_Web",
u={sort_by:"timestamp",sort_order:"desc"},
_={1:{has:["image","video"]},3:{has:["file"]},0:{},2:{has:["link"]},4:{pinned:!0}};
…let A={recent:1,media:1,pins:4,messages:0,links:2,files:3};
```

То есть вкладки поиска — это заранее собранные фильтры, а не отдельные параметры:
`media → has:["image","video"]`, `files → has:["file"]`, `links → has:["link"]`,
`pins → pinned:true`.

Размер страницы `[D] @318812`: `iG=25` (константа `T_y`), используется как
`offset: e*Q.T_y` при перелистывании.

`limit` в реальном запросе не передаётся — смотрим live-захват:

```
[D] capture-run2.jsonl
/guilds/1014537851418980372/messages/search?content=…&sort_by=timestamp&sort_order=desc
  &search_session_id=d6183157-…&search_query_id=cfb6f5e3-…&offset=0
```

`include_nsfw` `[D] @7550145`:

```js
function v(e,t){if(c.A.didAgree(t)){let t=A.default.getCurrentUser();
  null!=t&&(e.include_nsfw=null==t.nsfwAllowed||t.nsfwAllowed)}}
```

### 2.5 Ответ

`[D] b9e153f9dfaafc9f_0@29799`:

```js
let n=e.body,l=n.messages[0];
…result:n,messageCount:n.total_results…
{analyticsId:n.analytics_id,totalResults:n.total_results,channels:n.channels??[],
 messages:n.messages,threads:n.threads??[],members:(n.members??[]).map(…),
 doingHistoricalIndex:n.doing_deep_historical_index,documentsIndexed:n.documents_indexed}
```

`messages` — массив массивов (контекст вокруг совпадения), отсюда `messages[0]`.

### 2.6 Что осталось

Максимальный `offset` в клиенте не ограничен — константы нет ни в дампе, ни в live-бандле.
Серверный предел — `[A]`, нужен отдельный запрос с большим `offset`.

## 3. STUB-03 — создание сервера

`[D] @266302`: `GUILDS:"/guilds"`. Метод и тело в клиенте не встречаются:

* `rest-static.json` по этому маршруту: `"method":null,"callSiteCount":0`.
* В live-бандле (2026-10-30, `web.b2b84438f3c83e84.js`) `Rsh.GUILDS` тоже отсутствует.
* Строка `GUILD_CREATE_MODAL` есть только как метка аналитики `[D] @357134`, не как код модалки.

Значит, `POST /guilds` вызывается из лениво подгружаемого чанка, который не появляется
на экране логина. **Тело запроса — `[A]`.**

Соседние маршруты, которые подтверждены и могут понадобиться для «create/join»:

| Маршрут | Ключ | Метка |
|---|---|---|
| `POST /guilds/{id}/members/@me` | `GUILD_JOIN` | `[D] @261965` |
| `PUT /guilds/{id}/members/@me` (lurker) | — | `[L]` из RW-40 |
| `GET /guilds/{id}/requests` | `GUILD_JOIN_REQUESTS` | `[D] @265970` |
| `POST /guilds/{id}/requests/@me` | `GUILD_MEMBER_REQUEST_TO_JOIN` | `[D] @265910` |
| `GET /guilds/templates/{code}` | `UNRESOLVED_GUILD_TEMPLATE` | `[D] @276289`-область |
| `GET /guilds/{id}/templates` | `GUILD_TEMPLATES` | `[D] @276425` |

## 4. STUB-05 / STUB-07 — инвайты

### 4.1 Принять инвайт

`[D] 4d2a64cb5df5fc4e_0@635659`:

```js
_.Bo.post({url:ei.Rsh.INVITE(A),context:g,oldFormErrors:!0,
  body:{session_id:f,invite_instance_id:c.invite_instance_id,received_installation_id:p},
  rejectWithError:(0,_.fT)()})
```

`A` — нормализованный код, `g = {...context, invite_guild_scheduled_event_id}`.
`session_id` — gateway-сессия клиента, `invite_instance_id` — из резолва инвайта,
`received_installation_id` — только если инвайт пришёл по ссылке установки.

Ответ — объект инвайта (`channel`, `guild`, `inviter`, `new_member`, `flags`, `expires_at`…),
дальше клиент решает, показывать ли верификацию: `[D]` там же
`if(!E&&!f&&null!=_&&c.new_member&&!c.show_verification_form)`.

Ошибки `[D]` там же и в enum `@322893`:

| Код | Имя | Реакция клиента |
|---|---|---|
| 10006 | `UNKNOWN_INVITE` | тост «инвайт недействителен» |
| 10004 | `UNKNOWN_GUILD` | ошибка |
| 10007 | `UNKNOWN_MEMBER` | ошибка |
| 40007 | `USER_BANNED` | запрет входа |
| 50035 | `INVALID_FORM_BODY` | ошибка формы |
| 30001 | `TOO_MANY_USER_GUILDS` | лимит серверов |

Плюс особые: `USER_GUILD_JOIN_LARGE_GUILD_UNDERAGE_DISALLOWED`,
`USER_GUILD_JOIN_AGE_RESTRICTED_IOS_DISALLOWED` (`[D] @635600`-область) — только для
возрастных ограничений, для десктопа не критично.

### 4.2 Резолв инвайта

`[D] 4d2a64cb5df5fc4e_0@5764857`:

```js
let I={inputValue:n?.inputValue,with_counts:!0,with_expiration:!0,
  guild_scheduled_event_id:f,target_channel_id:u,target_message_id:h,
  with_permissions:!0,with_games:!!n?.withGames||void 0,
  with_guild_experiments:!!n?.withGuildExperiments||void 0},
p=o.A.get({url:_.Rsh.INVITE(c),query:I,oldFormErrors:!0,…})
```

### 4.3 Создать инвайт

`[D] 4d2a64cb5df5fc4e_0@632264`:

```js
async createInvite(e){let t=arguments.length>1&&void 0!==arguments[1]?arguments[1]:{},n=arguments.length>2?arguments[2]:void 0;
  try{let i={...t};i.role_ids?.length===0&&delete i.role_ids;
  let{body:r}=await _.Bo.post({url:ei.Rsh.INSTANT_INVITES(e),body:i,context:{location:n},rejectWithError:!0});
```

Маршрут `[D] @270160`: `INSTANT_INVITES:e=>\`/channels/${e}/invites\``.

Поля тела `[S] 4d2a64cb5df5fc4e_0@1933819` (схема Embedded App SDK, тот же контракт):

```js
[ck.e$.CREATE_CHANNEL_INVITE]:{request:e=>({channel_id:e.string().required(),
  max_age:e.number().optional(),max_uses:e.number().optional(),
  temporary:e.boolean().optional(),unique:e.boolean().optional(),
  target_type:…"InviteTargetTypes"…,target_user_id:…,target_application_id:…}),
 response:e=>({code:e.string().required(),type:e.number().optional(),
  channel:{id,name,type}?,guild:{id,name,icon}?,expires_at,max_age,max_uses,uses,temporary})}
```

Ответ: `code` (строка, обязательное), `expires_at`, `uses`, `max_age`, `max_uses`, `temporary`.

### 4.4 Список и отзыв

| Действие | Запрос | Метка |
|---|---|---|
| Список инвайтов канала | `GET /channels/{id}/invites` | `[D] @9009598` |
| Отозвать | `DELETE /invites/{code}` | `[D] @634707` |
| Список инвайтов сервера | `GET /guilds/{id}/invites` | `[D] @263348` |

## 5. STUB-12 — кнопка Gift 🎁

### 5.1 Обработчик

Компонент кнопки — `ChannelPremiumGiftButton`, `[D] 0eb6cf877afa84ff_0@7496`:

```js
{openGiftModal:Y,shouldShowWishlistModal:ee,shouldShowGiftSelectionModal:en}=(0,k.$)({
  giftRecipient:m,analyticsLocations:a,
  analyticsObject:{page:t.isPrivate()?x.liQ.DM_CHANNEL:x.liQ.GUILD_CHANNEL,
    section:x.JJy.CHANNEL_TEXT_AREA,object:S?x.ZSU.GIFTING_PROMOTION_BUTTON:x.ZSU.BUTTON_ICON,
    objectType:x.AnalyticsObjectTypes.GIFT},
  location:S?"gift-promotion-button":"gift-button"});
…
onClick:function(){d(!1),…,Y()}
```

Кнопка `[D]` там же: `aria-haspopup:"dialog"`, `isActive:!1`, иконка `GiftIcon`.
Поверхность для аналитики — `CHANNEL_TEXT_AREA`, локации `gift-button` / `gift-promotion-button`.

### 5.2 Модалка

`openGiftModal` — хук `[D] 0eb6cf877afa84ff_0@23109`:

```js
openGiftModal:i.useCallback(()=>{
  null!=p?(0,l.A)({isGift:!0,giftRecipient:n??void 0,subscriptionTier:p,…}):
  h? _?s({giftRecipient:n,…}):(0,C.A)({giftRecipient:n??void 0,…}):
  j&&null!=n?(0,C.A)({giftRecipient:n,…}):
  k?(0,C.A)({…,giftingOrigin:g.vQ.GUILD_CHANNEL}):
  _&&null!=n?s({giftRecipient:n,…}):
  (0,l.A)({isGift:!0,giftRecipient:n??void 0,initialPlanId:null,…})},…)
```

Два варианта модалки:

* `l.A` (модуль `582128`) — модалка подарочной подписки Nitro, реализация в ленивом чанке,
  **в дампе отсутствует** (`582128` не имеет определения).
* `990820` + `4ff3d45110245dd5_0` — «Gift selection»: `openModalLazy(…).then(n.bind(n,806677))`,
  `modalKey: "gift-selection-modal-${userId}"`, `giftingOrigin`.

Модуль `806677` (тело модалки) в дампе отсутствует: `grep` по `reference-oracle/decoded`
не находит `806677(e,t,n)`, в live-бандле есть только ссылка `n.bind(n,806677)`.
Разметка модалки — `[A]`.

### 5.3 Endpoint

`[D] 4d2a64cb5df5fc4e_0@613587`:

```js
async createGiftCode(e){let t=arguments.length>1&&void 0!==arguments[1]?arguments[1]:null,
  n=arguments.length>2?arguments[2]:null;
  r.h.dispatch({type:"GIFT_CODE_CREATE_START",skuId:e,subscriptionPlanId:t});
  try{let a=await i.Bo.post({url:h.Rsh.USER_GIFT_CODE_CREATE,
    body:{sku_id:e,subscription_plan_id:t,gift_style:n},oldFormErrors:!0,rejectWithError:!0});
```

Маршрут `[D] @290560`: `USER_GIFT_CODE_CREATE:"/users/@me/entitlements/gift-codes"`.

Полный набор gift-маршрутов `[D] @290386…@290799`:

| Ключ | Путь | Метод |
|---|---|---|
| `USER_GIFT_CODE_CREATE` | `/users/@me/entitlements/gift-codes` | POST |
| `USER_GIFT_CODES` | `/users/@me/entitlements/gift-codes` | GET |
| `USER_GIFT_CODE_REVOKE` | `/users/@me/entitlements/gift-codes/{code}` | DELETE |
| `GIFT_CODE_RESOLVE` | `/entitlements/gift-codes/{code}` | GET |
| `GIFT_CODE_REDEEM` | `/entitlements/gift-codes/{code}/redeem` | POST |
| `USER_GIFT_CODE_DELIVERIES` | `/users/@me/gift-code-deliveries` | — |
| `GIFT_INTENTS_DISMISS` | `/users/@me/gift-intents/dismiss` | POST |

Стиль подарка `[S]` (`web.b2b84438f3c83e84.js`):

```js
var R=((i={})[i.DEFAULT=0]="DEFAULT",i[i.CUSTOM_STYLE=1]="CUSTOM_STYLE",
  i[i.CUSTOM_MESSAGE_EMOJI_SOUNDBOARD=2]="CUSTOM_MESSAGE_EMOJI_SOUNDBOARD",i);
function L(e,t){return a.Fr||a.v1?0:null!=e||t?2:1}
… return `${e}:${t??""}:${n??""}`   // skuId:planId:giftStyle
```

Современный путь (checkout) добавляет к платежу `[D] 3e8824f7eca8914f_0@230732`:

```js
ew.gift_style=eN,ew.reward_sku_ids=eL?eM:[];
ek===R.tB.CUSTOM_MESSAGE_EMOJI_SOUNDBOARD&&(…,ew.recipient_id=eO.id,ew.custom_message=eb,
  ew.emoji_id=ej?.id,ew.emoji_name=ej?.id==null?ej?.surrogates:void 0,ew.sound_id=eR?.soundId)
```

### 5.4 Вывод по STUB-12

Кнопка не «просто disabled»: у неё есть обработчик, модалка и два сетевых пути.
Минимальный паритет: `POST /users/@me/entitlements/gift-codes` с `{sku_id, subscription_plan_id, gift_style}`.
Разметка модалки и выбор SKU — отдельная задача, здесь остаётся `[A]`.

## 6. STUB-27 — Go Live / Stream

### 6.1 Opcodes

`[D] 4d2a64cb5df5fc4e_0@4996751`:

```js
s[s.STREAM_CREATE=18]="STREAM_CREATE",s[s.STREAM_DELETE=19]="STREAM_DELETE",
s[s.STREAM_WATCH=20]="STREAM_WATCH",s[s.STREAM_PING=21]="STREAM_PING",
s[s.STREAM_SET_PAUSED=22]="STREAM_SET_PAUSED"
```

### 6.2 Исходящие пакеты

`[D] 4d2a64cb5df5fc4e_0@4998992`:

```js
streamCreate(e,t,n){let i=arguments.length>3&&void 0!==arguments[3]?arguments[3]:null;
  this.send(e2.STREAM_CREATE,{type:e,guild_id:t,channel_id:n,preferred_region:i})}
streamWatch(e){this.send(e2.STREAM_WATCH,{stream_key:e})}
streamPing(e){this.send(e2.STREAM_PING,{stream_key:e})}
streamDelete(e){this.send(e2.STREAM_DELETE,{stream_key:e})}
streamSetPaused(e,t){this.send(e2.STREAM_SET_PAUSED,{stream_key:e,paused:t})}
```

Вызов из редьюсера `[D] @4954991`:

```js
STREAM_START:function(e){let{streamType:t,guildId:n,channelId:i}=e;if(B.isSessionEstablished()){
  let e=null!=n?f.A.getChannel(i)?.rtcRegion:h.A.getCall(i)?.region;
  B.streamCreate(t,n,i,e??T.A.getPreferredRegion())}return!1}
```

`preferred_region` берётся из `channel.rtcRegion` для гильдии или `call.region` для звонка,
иначе `T.A.getPreferredRegion()`; `type` — `GUILD` или `CALL` (`R.U4.GUILD`/`R.U4.CALL`, `[D] @710570`).

### 6.3 Входящие пакеты

`[D] 4d2a64cb5df5fc4e_0@4937840`:

```js
eV(["STREAM_CREATE"],e=>{eW({type:"STREAM_CREATE",streamKey:e.stream_key,region:e.region,
  viewerIds:e.viewer_ids,rtcServerId:e.rtc_server_id,rtcChannelId:e.rtc_channel_id,paused:e.paused})}),
eV(["STREAM_SERVER_UPDATE"],e=>{…endpoint:e.endpoint,token:e.token…}),
eV(["STREAM_UPDATE"],e=>{…streamKey,region,viewerIds,paused…}),
eV(["STREAM_DELETE"],e=>{…streamKey,unavailable:e.unavailable,reason:e.reason…}),
```

Сводка:

| Op | Направление | Поля |
|---|---|---|
| 18 `STREAM_CREATE` | client→server | `type`, `guild_id`, `channel_id`, `preferred_region` |
| 19 `STREAM_DELETE` | client→server | `stream_key` |
| 20 `STREAM_WATCH` | client→server | `stream_key` |
| 21 `STREAM_PING` | client→server | `stream_key` |
| 22 `STREAM_SET_PAUSED` | client→server | `stream_key`, `paused` |
| `STREAM_CREATE` | server→client | `stream_key`, `region`, `viewer_ids`, `rtc_server_id`, `rtc_channel_id`, `paused` |
| `STREAM_SERVER_UPDATE` | server→client | `stream_key`, `endpoint`, `token` |
| `STREAM_UPDATE` | server→client | `stream_key`, `region`, `viewer_ids`, `paused` |
| `STREAM_DELETE` | server→client | `stream_key`, `unavailable`, `reason` |

REST-часть Go Live `[D] @292006`:

```js
STREAM_PREVIEW:e=>`/streams/${e}/preview`,
STREAM_NOTIFY:e=>`/streams/${e}/notify`,
STREAM:e=>`/streams/${e}/stream`,
```

## 7. Изолированная runtime-сессия

### 7.1 Как запускалось

Инструментов REA MCP в этой сессии нет: каталог Code Mode содержит только `github`,
`basic-memory`, `understand`, `opencode` (`search` по `analyze_javascript_application`,
`list_browser_targets`, `export_web_scripts` и остальным возвращает пусто).
Поэтому эквивалент шагов 2–3 сделан напрямую по CDP: `/json/list`, `Debugger.scriptParsed`,
`Debugger.getScriptSource`, `Runtime.evaluate` — через встроенный в Node 26 `WebSocket`.

Запуск по инструкции `--user-data-dir=/tmp/discord-rea-isolated` не изолирует:
Discord сам выставляет `userData` и проигнорировал флаг, увидел работающий основной клиент
и вышел с `Quitting secondary instance.` (`/tmp/discord-rea-isolated.log`), каталог остался пустым.
Рабочий вариант — отдельный `HOME`:

```bash
HOME=/tmp/discord-rea-home XDG_CONFIG_HOME=/tmp/discord-rea-home/.config \
/home/user/.config/discord/app-1.0.167/Discord --remote-debugging-port=9229 --no-sandbox
```

Порт поднялся, целевая страница — `https://discord.com/login`, UA
`discord/1.0.167 Chrome/152.0.7977.167 Electron/44.9.3`.
Основной профиль `~/.config/discord` не использовался: его процессы (6 шт.) продолжали работать,
сессия и токены не менялись. После снятия данных процесс остановлен, порт 9229 закрыт.

Честная оговорка: первая попытка (без `HOME`) успела стартовать в дефолтном пути и выйти
по проверке single-instance — вход не выполнялся, токены не трогались.

### 7.2 Что снято

* `Debugger.scriptParsed`: **984** скрипта, из них **748** — чанки `/assets/`.
* `Debugger.getScriptSource`: **771** файл, **26 983 652** байта →
  `/tmp/discord-rea-isolated/exported-scripts/` (+ `_manifest.json`, `_script-parsed.json`).
* Live-бандл: `https://discord.com/assets/web.b2b84438f3c83e84.js`,
  12 640 328 байт, `md5 d0044dacefb35f0a798eb48272a0396b`.

Сверка с дампом от 2026-10-19: из 748 runtime-чанков **383** есть в `decoded-index.json`,
**365 отсутствуют**. То есть живой клиент новее дампа (в дампе `web.4d4b2f75fdea5470.js`).
Новые чанки лежат в `exported-scripts/` и пригодны для дальнейшего RE.

### 7.3 Чего runtime не дал

Экран логина не грузит ленивые чанки авторизованных сценариев. Поэтому:

* тело `POST /guilds` (STUB-03) — не получено;
* тело модалки gift (`806677`) — не получено, в live-бандле только `n.bind(n,806677)`.

Карта `chunkId → hash` для этих чанков в `web.*.js` не найдена: объект в `.u=e=>…`
не содержит `806677` и не содержит хешей уже загруженных чанков (например `703819`),
значит карта живёт в отдельном runtime-файле. Прямая выборка по URL не удалась.

## 8. Остаточные gap

| Пункт | Статус | Что нужно |
|---|---|---|
| `POST /guilds` тело и коды | `[A]` | live-захват создания сервера под аккаунтом |
| Gift-модалка `806677`, выбор SKU | `[A]` | тот же live-захват, клик по 🎁 |
| Максимальный `offset` поиска | `[A]` | запрос с большим `offset` |
| Сжатие/ресайз перед upload | не нужно для контракта | — |

## 9. Что это меняет в планах

* **B2 (вложения)**: `blocked` снимается. Контракт полный, можно писать backend и fake-тесты
  на `POST /channels/{id}/attachments` + `PUT upload_url` + `uploaded_filename`.
* **B7a (поиск)**: доступны все 7 фильтров, а не 3. `channel_id`, `author_id`, `mentions`,
  `has`, `pinned`, `author_type`, `min_id`/`max_id` — всё подтверждено.
  Добавить обработку `202 + retry-after` и `total_results`.
* **G6 (gift)**: карточку можно перевести из «unknown» в «endpoint известен».
* **G5 (stream)**: `STREAM_*` больше не `[A]` — opcodes и поля зафиксированы.
* **STUB-03**: остаётся `[A]`, но теперь известно, что это именно ленивый чанк, а не
  отсутствующая функция.

## 10. Артефакты

| Файл | Размер | md5 | sha256 (начало) |
|---|---|---|---|
| `reference-oracle/decoded/4d2a64cb5df5fc4e_0` | 12 410 287 | `934b1289b7ebf17bc0fd09569b23b438` | `667a771c61ff709e…` |
| `reference-oracle/decoded/d8b0ffc3255e8113_0` | 587 591 | `7cb334d2c7b50689f0c99c01c305663d` | `643c57c2ebe42a2e…` |
| `reference-oracle/decoded/153cd69d0210c61b_0` | 8 018 | `465ebb0035df36346702b1bf024e854a` | `6b3f089b90814fa3…` |
| `reference-oracle/decoded/23e2050b01c07deb_0` | 3 654 | `3ec5ba1ced9f25db137c7f41b00d3195` | `f7e83c8b8d09cd44…` |
| `reference-oracle/decoded/82a5af57ee11dcd5_0` | 2 443 | — | — (`0a6ed6a6830e5071.js`, `SearchFetcher`) |
| `exported-scripts/web.b2b84438f3c83e84.js` | 12 640 328 | `d0044dacefb35f0a798eb48272a0396b` | `60aeb22429afacde…` |
| `exported-scripts/_manifest.json` | — | — | 771 запись |

`d8b0ffc3255e8113_0` (`30bab6af3517c471.js`) по этим пяти контрактам ничего не добавил:
в нём нет ни `messages/search`, ни `author_id`, ни `upload_url`. Он про настройки
activity-sharing. `153cd69d0210c61b_0` (`912f788c752afd37.js`) дал независимое
подтверждение схемы `upload_url` → `PUT` → `upload_filename` на виджетах профиля:

```js
let{upload_url:t,upload_filename:n}=(await i.Bo.post({url:u.Rsh.USER_PROFILE_WIDGET_ASSET_UPLOAD,
  body:{filename:e.name,file_size:e.size},rejectWithError:!0})).body,
l=await fetch(t,{method:"PUT",body:e,headers:{"Content-Type":""!==e.type?e.type:"application/octet-stream"}});
```
