# SEARCH-202-NSFW-RE — M-13: серверный поиск, HTTP 202 и NSFW-гейтинг

Карточка: **SEARCH-202-NSFW-RE** (M-13, STUB-14).
Дата: 2026-10-30. Ветка: `RW-40`.
Источник: read-only разбор `/home/sanya/re-deep` + `waves/RW-40/RE-DEEP-REA.md`.
Код, тесты и golden-каталоги (`logs/`, `functions/`, `evidence/`) не менялись.

Метки: `[D]` — цитата декодированного дампа или runtime-артефакта; `[S]` —
статический индекс/схема; `[A]` — вывод без runtime-подтверждения; `[?]` — не найдено.

## 0. Вердикт

1. Поиск сообщений — это **пять маршрутов** под `/api/v9`: два GET (`/guilds/{id}/…`,
   `/channels/{id}/…`) и три POST `/tabs`. Поиск «в канале гильдии» физически идёт
   в guild-эндпоинт с фильтром `channel_id`, а не в channel-эндпоинт.
2. GET-контракт закрыт полностью: 12 фильтровых query-параметров, транспортные
   (`sort_by`, `sort_order`, `offset`), три служебных
   (`include_nsfw`, `search_session_id`, `search_query_id`) и один артефакт ретрая
   (`attempts`). `limit` официальный клиент **не передаёт**; размер страницы — 25.
3. HTTP 202 — это «глубокая индексация идёт». Клиент читает **заголовок**
   `retry-after` в секундах, по умолчанию 5000 мс, делает не более 5 повторов
   (всего ≤ 6 запросов). `documents_indexed` приходит только в теле 200.
4. NSFW-гейтинг на поиске — это один флаг запроса `include_nsfw`, который
   выставляется только при наличии per-guild согласия (`GuildNSFWAgreeStore`).
   Новый `age_gated_features`-флоу (битовая маска, `AGE_GATED_SPACES=1`) отключает
   это согласие для не-взрослых. Отдельного поля `age_gated` в REST/gateway нет.
5. Не подтверждено runtime: сам ответ 202 (в capture только 200), максимальный
   `offset`, серверная фильтрация NSFW по `include_nsfw`, дробный `retry-after`.

## 1. Маршруты и метод

`[D] 4d2a64cb5df5fc4e_0@279632`, `@279679`, `@279736`, `@280974`, `@281025`:

```js
SEARCH_GUILD:e=>`/guilds/${e}/messages/search`,
SEARCH_TABS_GUILD:e=>`/guilds/${e}/messages/search/tabs`,
SEARCH_TABS_DMS:"/users/@me/messages/search/tabs",
SEARCH_CHANNEL:e=>`/channels/${e}/messages/search`,
SEARCH_TABS_CHANNEL:e=>`/channels/${e}/messages/search/tabs`,
```

База — `https://discord.com/api/v9`: `[D] 4d2a64cb5df5fc4e_0@10697304`

```js
return"https:"+window.GLOBAL_ENV.API_ENDPOINT+(e?`/v${window.GLOBAL_ENV.API_VERSION}`:"")
```

и runtime-захват `[D] ui8/20f-search-network.json`:
`https://discord.com/api/v9/guilds/1014537851418980372/messages/search?...`.

| Маршрут | Метод | Когда используется | Доказательство |
|---|---|---|---|
| `/guilds/{guild_id}/messages/search` | GET | гильдия и канал гильдии (тред — после нормализации) | `[D] @279632`; `SearchFetcher.getEndpoint`, `GUILD`/`GUILD_CHANNEL` |
| `/channels/{channel_id}/messages/search` | GET | приватный канал (DM/GDM) | `[D] @280974`; `case CHANNEL` |
| `/guilds/{guild_id}/messages/search/tabs` | POST | вкладки `messages/links/media/files/pins/recent` | `[D] @279679`; класс `DX` |
| `/channels/{channel_id}/messages/search/tabs` | POST | вкладки в приватном канале | `[D] @281025` |
| `/users/@me/messages/search/tabs` | POST | глобальный поиск по ЛС | `[D] @279736`; `case DMS` |

Метод доказан кодом `[D] 82a5af57ee11dcd5_0` (`0a6ed6a6830e5071.js`, модуль `177068`,
класс `SearchFetcher`):

```js
class d extends c{getEndpoint(){switch(this.searchType){case n.I4_.GUILD:…return n.Rsh.SEARCH_GUILD(this.searchId);
  case n.I4_.GUILD_CHANNEL:{…let e=a.A.getChannel(this.searchId),t=e?.getGuildId();…return n.Rsh.SEARCH_GUILD(t)}
  case n.I4_.CHANNEL:…return n.Rsh.SEARCH_CHANNEL(this.searchId);…
makeRequest(e){let{rejectWithError:t}=e,s=this.getEndpoint();
  return null==s?null:h.Bo.get({url:s,query:r.stringify(this.query),oldFormErrors:!0,rejectWithError:t})}
```

```js
class u extends c{getEndpoint(){switch(this.searchType){case n.I4_.DMS:return n.Rsh.SEARCH_TABS_DMS;…
  makeRequest(e){…h.Bo.post({url:s,body:this.payload,oldFormErrors:!0,rejectWithError:t})}
```

Три следствия:

* `SEARCH_TYPE` — `[D] 4d2a…@318856`: `GUILD`, `CHANNEL`, `GUILD_CHANNEL`, `THREAD`,
  `DMS`, плюс нерелевантные `CHAT_SIDEBAR`, `GIF`, `GUILD_DISCOVERY*`, `STICKER*`,
  `EMOJI*`, `SOUNDBOARD`. В `getEndpoint` у GET нет `case THREAD` и `case DMS`
  (оба уходят в `throw Error("Unhandled search type")`); значит контекст треда
  нормализуется в `GUILD`/`GUILD_CHANNEL` до вызова фетчера, иначе поиск падает.
  Точное место нормализации в декодированных чанках не найдено — `[A]`.
* `GUILD_CHANNEL` **не** идёт в `/channels/{id}/messages/search`: он подставляет
  `guild_id` канала и передаёт `channel_id` фильтром. Это подтверждено live-захватом
  `[D] ui8/20i-scope-compare.json`: `…/guilds/1014537851418980372/messages/search?channel_id=1014537851867775060&content=ProBot…`.
* `rest-static.json` для `SEARCH_GUILD`, `SEARCH_CHANNEL`, `SEARCH_TABS_GUILD`,
  `SEARCH_TABS_CHANNEL` даёт `"method":null,"callSiteCount":0`, а для
  `SEARCH_TABS_DMS` — `"method":"GET"` `[S]`. Это ложный отрицательный результат
  экстрактора: call-site спрятан в `SearchFetcher.makeRequest`, метод — в
  `Bo.get`/`Bo.post`. Опираться на `rest-static` по этим пяти ключам нельзя.

Заголовки реального запроса `[D] capture-run2.jsonl` / `ui8/20f-search-network.json`:
`Authorization`, `X-Super-Properties`, `X-Discord-Locale`, `X-Discord-Timezone`,
`X-Debug-Options: bugReporterEnabled`, `Referer`, `Accept-Language`, `User-Agent`.
Обязательных среди них для контракта — только `Authorization`; остальные
диагностические.

## 2. GET-запрос: параметры

### 2.1 Полная таблица

Источник значений — таблица правил токенов `[D] 4d2a64cb5df5fc4e_0@7564745` и
сборщик `Zf` (`function b(e)`) в том же модуле. `queryKey` — имя параметра.

| Параметр | Тип в query | Откуда берётся | Значение / формат |
|---|---|---|---|
| `content` | string | свободный текст (все не-фильтровые токены) | склеенный пробелом, `.trim()` |
| `author_id` | повторяющийся string | `from:` | snowflake; несколько → несколько параметров |
| `mentions` | повторяющийся string | `mentions:` | snowflake |
| `has` | повторяющийся string | `has:` | `link, embed, poll, snapshot, file, video, image, sound, sticker`; допускает `-` |
| `channel_id` | повторяющийся string | `in:` | snowflake канала |
| `link_hostname` | string | `link from:` | произвольный текст |
| `attachment_extension` | string | `file type:` | произвольный текст |
| `attachment_filename` | string | `file name:` | произвольный текст |
| `pinned` | повторяющийся bool | `pinned:` | `true` / `false` |
| `author_type` | повторяющийся string | `author type:` | `user, bot, webhook`; допускает `-` |
| `min_id` | string | `before:` / `after:` / `on:` | snowflake из timestamp (нижняя граница) |
| `max_id` | string | то же | snowflake из timestamp (верхняя граница) |
| `sort_by` | string | UI сортировка | `timestamp` \| `relevance` |
| `sort_order` | string | UI сортировка | `desc` \| `asc` |
| `offset` | number | пагинация | `0, 25, 50, …` (шаг 25) |
| `include_nsfw` | bool | NSFW-гейт | `true` / `false`; только при согласии (§7) |
| `search_session_id` | uuid | сессия поиска | `[D] capture` `d6183157-…` |
| `search_query_id` | uuid | запрос | `[D] capture` `cfb6f5e3-…` |
| `search_everywhere` | bool | «искать везде» | `true`; `[D] effc40ce4f2aa27c_0@1785` |
| `attempts` | number | побочный эффект ретрая | `1…5`, см. §5.4 |

`limit` в GET **не передаётся**. Размер страницы — константа `T_y`: `[D] 4d2a…@318806`

```js
iw=10,iG=25
```

и пагинация `offset: e*Q.T_y` `[D] 4d2a…@7542421`.

### 2.2 Токены → `queryKey`

`[D] 4d2a64cb5df5fc4e_0@7564745`:

```js
[D.LWr.ANSWER_USERNAME_FROM]:{…,queryKey:"author_id"},
[D.LWr.ANSWER_USERNAME_MENTIONS]:{…,queryKey:"mentions"},
[D.LWr.ANSWER_HAS]:{…,queryKey:"has"},
[D.LWr.ANSWER_LINK_FROM]:{…,queryKey:"link_hostname"},
[D.LWr.ANSWER_FILE_TYPE]:{…,queryKey:"attachment_extension"},
[D.LWr.ANSWER_FILE_NAME]:{…,queryKey:"attachment_filename"},
[D.LWr.ANSWER_IN]:{…,queryKey:"channel_id"},
[D.LWr.ANSWER_PINNED]:{…,queryKey:"pinned"},
[D.LWr.ANSWER_AUTHOR_TYPE]:{…,queryKey:"author_type"}
```

`before:`/`on:`/`after:` не имеют `queryKey`: они кладут `start`/`end`, а сборщик
превращает их в `min_id`/`max_id` `[D] @7550300`:

```js
case T.LWr.ANSWER_BEFORE:case T.LWr.ANSWER_ON:case T.LWr.ANSWER_AFTER:
  let a=e.getData("start"),s=e.getData("end");
  a&&(t.min_id=h.default.fromTimestamp(a)),s&&(t.max_id=h.default.fromTimestamp(s));return
```

Правило границ `[D] 4d2a…@7558245` (`validator j`): для `before:` остаётся только
`end` (→ `max_id`), для `after:` — только `start` (→ `min_id`), для `on:` — обе.
Дата принимает `YYYY-MM-DD`, `YYYY-MM`, `YYYY`, названия месяца/дня/года и
относительные `today`/`yesterday`/`week`/`month`/`year`.

Значения `has` `[D] @7559278`:

```js
{[v.intl.string(v.t.ZNR2fi)]:"link",…:"embed",…:"poll",…:"snapshot",…:"file",
 …:"video",…:"image",…:"sound",…:"sticker"}
```

Значения `author_type` — `user`, `bot`, `webhook` `[D]` там же. UI-снимок
`has`-фильтра показывает те же девять категорий `[D] ui8/20b-search-filters-has.json`:
`изображение, видео, ссылку, файл, вложение, аудио, опрос, стикер, переслать`
(однозначное соответствие русских подписей значениям кодом не проверялось).
Отрицание `[D] 4d2a…@7556693` (`function y`) и `@7559783` (`function $`):

```js
let i=n.getMatch(1),r=i.startsWith("-"),a=t[r?i.slice(1):i];…n.setData(e,r?`-${a}`:a)
```

То есть минус **входит в значение параметра**: `has=-file`, `author_type=-bot`.
Подтверждено только для `has:` и `author type:` (обе функции вызываются лишь из
`z`/`X`). Для `from:`, `mentions:`, `in:` парсер минус не снимает: `-from:user`
попадёт в `content`, а не в `author_id`. Это расходится с формулировкой в
`ATTACH-RE.md` §1 («Отрицание через `-`: `-has:file`, `-from:user`») — верна только
первая половина.

`pinned:` принимает строго `true|false` `[D] 4d2a…@7559783`:

```js
return"true"===t?(e.setData("pinned",!0),!0):"false"===t&&(e.setData("pinned",!1),!0)
```

В GET значение уходит как повторяющийся bool (`pinned=true`); если в запросе есть
оба токена, будет `pinned=true&pinned=false`. В POST-вкладках массив сворачивается:
`[D] effc40ce4f2aa27c_0@474`

```js
if(!Array.isArray(e.pinned))return;let t=e.pinned.some(e=>!0===e);e.pinned=t
```

Сводный пример. Ввод
`from:pharos12510 in:#general has:image before:2026-10-20 pinned:true привет`
даёт:

```
GET /api/v9/guilds/{guild_id}/messages/search
  ?author_id=…&channel_id=…&has=image&max_id=…
  &pinned=true&content=привет
  &sort_by=timestamp&sort_order=desc&offset=0
```

(порядок параметров иллюстративный: клиент сохраняет порядок вставки ключей).

Токены-фильтры вырезаны из `content`; `before:` даёт только `max_id`; `#` в `in:`
отбрасывается регэкспом парсера `W` `[D] 4d2a…@7558341`.

### 2.3 Сериализация

`[D] 82a5af57ee11dcd5_0`: `query:r.stringify(this.query)`, где `r = n(488428)`, а
`488428` — `t.encode=t.stringify=r(998736)` `[D] 25883e62976b3cc0_0@13755`. Модуль
`998736` — полифилл Node `querystring` `[D] 8b501595bbf87517_0@667164`:

```js
Array.isArray(e[i])?e[i].map(function(e){return o+encodeURIComponent(t(e))}).join(r):…
case"boolean":return e?"true":"false";…
```

Правила:

* массив → повтор ключа: `has=image&has=video`, `author_id=1&author_id=2`;
* bool → `true`/`false`;
* `null`/`undefined` → пустое значение `key=`; **объект** → `key=`;
* порядок ключей — порядок вставки (не сортируется), кодирование — `encodeURIComponent`;
* сборщик удаляет `contents` и пустой `content` `[D] 4d2a…@7551206` (`delete t.contents`,
  `t.content||delete t.content`).

### 2.4 Сортировка

`[D] 4d2a…@7542418` (`function R`/`nm`) и enum `BBH` `[D] 4d2a…@319362`:

| UI | enum | `sort_by` | `sort_order` |
|---|---|---|---|
| «По новизне» (default) | `newest` | `timestamp` | `desc` |
| «Самое старое» | `oldest` | `timestamp` | `asc` |
| «Самое актуальное» | `most_relevant` | `relevance` | `desc` |

Три пункта меню подтверждены UI-снимком `[D] ui8/20c-search-sort.json`:
`«По новизне» / «Самое старое» / «Самое актуальное»`.

### 2.5 Сессионные id

`[D] 4d2a…@280974`-область (`function z`, экспорт `VI`):

```js
function z(e,t){if(!(0,I._)(e)||!u.A.getGuild(e.guildId)?.features.has(T.GuildFeatures.DISCOVERABLE))return null;
  let n=t.getSessionId(e),i=t.getQueryId(e);
  return null==n||null==i?null:{search_session_id:n,search_query_id:i}}
```

`I._` — «это guild-контекст» (`GUILD`, `GUILD_CHANNEL`, `THREAD`) `[D] 902008`.
То есть id передаются только для **discoverable** гильдий и только если сессия уже
инициализирована. `search_session_id` живёт на контекст, `search_query_id`
перегенерируется на каждый новый запрос `[D] effc40ce4f2aa27c_0@5395` (`refreshQueryId`).
Live-захват показывает оба параметра `[D] ui8/20f-search-network.json`, т.е.
тестовая гильдия была discoverable.

### 2.6 Ограничение текста

`setSearchInputText` режет ввод до 512 символов `[D] effc40ce4f2aa27c_0@10631`:

```js
S.A.updateSearchQueryText(e,t.slice(0,512))
```

Серверный лимит длины `content` не подтверждён — `[A]`.

## 3. POST `/tabs`: тело запроса

`[D] effc40ce4f2aa27c_0@3685` (модуль `956467`, `SearchTabFetcher.MS`):

```js
createRequestPayload(e){let{searchQuery:t,searchTabs:s,getLimit:r,pagination:a,trackExactTotalHits:i}=e,
  {include_nsfw:_,channel_id:c,search_session_id:l,search_query_id:h,...d}=t,
  u={include_nsfw:_,channel_ids:c,tabs:{},track_exact_total_hits:i,search_session_id:l,search_query_id:h};
  return s.forEach(e=>{let t=r(e),s=n.B7[e],i=null!=s?n.su[s]:{};
    u.tabs[e]={...n.us,...i,...d,...a,limit:t}}),u}
```

Схема тела:

```json
{
  "include_nsfw": true,
  "channel_ids": "1014537851867775060",
  "track_exact_total_hits": true,
  "search_session_id": "…",
  "search_query_id": "…",
  "tabs": {
    "messages": { "sort_by": "timestamp", "sort_order": "desc",
                  "content": "ProBot", "limit": 25, "offset": 0 },
    "links":    { "has": ["link"], "sort_by": "timestamp", "sort_order": "desc",
                  "limit": 25, "offset": 0 }
  }
}
```

Детали:

* `channel_id` переименовывается в `channel_ids` и **не** остаётся в теле вкладки.
  Тип не нормализуется: если фильтр пришёл массивом, уйдёт массив, если строкой —
  строка `[D]` (деструктуризация без приведения).
* `include_nsfw`, `search_session_id`, `search_query_id` — только верхний уровень.
* Фильтры (`content`, `author_id`, `mentions`, `has`, `pinned`, …) — **внутри каждой
  вкладки**, вместе с `sort_by/sort_order` и `offset`.
* `limit` — на вкладку, из `getLimit(tab)`; для ЛС-вкладки `messages` это `T_y = 25`
  `[D] effc40ce4f2aa27c_0@10082` (`getLimit:()=>p.T_y`).
* Вкладки — пресеты фильтров `[D] 4d2a…@7542418`:

  | Вкладка | Ключ пресета | Доп. фильтр |
  |---|---|---|
  | `messages` | 0 | `{}` |
  | `media` / `recent` | 1 | `{has:["image","video"]}` |
  | `links` | 2 | `{has:["link"]}` |
  | `files` | 3 | `{has:["file"]}` |
  | `pins` | 4 | `{pinned:true}` |

  Базовая сортировка всех вкладок — `us = {sort_by:"timestamp",sort_order:"desc"}`.
* `track_exact_total_hits` — bool, включает точный `total_results` вместо оценки.

Метод — POST `[D] 82a5af57ee11dcd5_0` (класс `DX`), не GET, вопреки
`rest-static.json` для `SEARCH_TABS_DMS`.

Отдельный случай: поиск сообщений в карточке участника (mod view) собирает тело
вручную и **обходит** общий retry-механизм `[D] b9e153f9dfaafc9f_0@25821`:

```js
o=i.useMemo(()=>({tabs:{messages:eq(e,"messages",u),links:eq(e,"links",u),media:eq(e,"media",u)},
  track_exact_total_hits:!0,include_nsfw:!0}),[e,u]);
…
let e=await n.makeRequest({rejectWithError:!1});l=e?.body
```

Здесь `include_nsfw` жёстко `true`, а 202 не ретраится: `makeRequest` вернёт ответ
со статусом 202 и пустым `tabs`, и панель покажет нули.

## 4. Ответ 200

### 4.1 Поля

`[D] b9e153f9dfaafc9f_0@29479` и `@31367`:

```js
{analyticsId:n.analytics_id,totalResults:n.total_results,channels:n.channels??[],
 messages:n.messages,threads:n.threads??[],members:(n.members??[]).map(…),
 doingHistoricalIndex:n.doing_deep_historical_index,documentsIndexed:n.documents_indexed}
```

| Поле | Тип | Присутствие | Смысл |
|---|---|---|---|
| `analytics_id` | string (hex) | всегда (200) | идентификатор запроса для аналитики |
| `messages` | array of array of message | всегда | внешний массив — результаты, внутренний — контекст вокруг совпадения |
| `total_results` | number | всегда | всего совпадений; при `track_exact_total_hits` — точно |
| `doing_deep_historical_index` | bool | всегда | идёт глубокая индексация истории |
| `documents_indexed` | number | **опционально** | сколько документов проиндексировано; в обычном 200 отсутствует |
| `channels` | array | опционально | каналы, упомянутые в результатах |
| `threads` | array | опционально | треды |
| `members` | array | опционально | авторы (трансформируются в member-записи) |
| `cursor` | ? | только в `/tabs` (на вкладку) | курсор вкладки; `null`, если пустой объект |

Runtime-подтверждение пустого ответа `[D] ui8/20g-search-probe.json`:

```json
{"analytics_id":"35a8f4e2e3dc17d18f882706c6f766f1","messages":[],"doing_deep_historical_index":false,"total_results":0}
```

Отсюда: `documents_indexed`, `channels`, `threads`, `members` в 200 **могут
отсутствовать**; дефолты в клиенте — `0` или `[]`. `messages[0]` — группа
контекста, её первый элемент используется как «само совпадение» `[D] b9e153…@29479`
(`let n=e.body.messages[0]`) и `[D] b9e153…@31367` (`A.result.messages.map(e=>{let[t]=e;…})`).
Порядок контекста внутри группы кодом не фиксируется — `[A]`.

Страница — 25 результатов `[D] ui8/20i-scope-compare.json` (`panel.items: 25`) и
`[D] ui8/20-search-results.json` (`totalText: "3 302 результата"`).

### 4.2 Ответ `/tabs`

`[D] effc40ce4f2aa27c_0@228` (`fetchTabMessages`):

```js
let{body:s}=e,r=Object.entries(s.tabs);
…{id:n,analyticsId:s.analytics_id,totalResults:r.total_results,cursor:r.cursor,
   messages:r.messages,channels:r.channels??[],threads:r.threads??[],
   members:(r.members??[]).map(…),doingHistoricalIndex:s.doing_deep_historical_index,
   documentsIndexed:s.documents_indexed}
```

То есть `analytics_id`, `doing_deep_historical_index`, `documents_indexed` — верхний
уровень; `total_results`, `messages`, `cursor`, `channels`, `threads`, `members` —
внутри `tabs.<имя>`. Ключи `tabs` — имена вкладок (`messages`, `links`, `media`, …).

## 5. HTTP 202: контракт ретрая

### 5.1 Код

`[D] 82a5af57ee11dcd5_0@212` (`SearchFetcher`, модуль `177068`):

```js
async fetch(e,t,s){if(!this.isCanceled)try{let r=await this.makeRequest({rejectWithError:!1});
  if(null==r||this.isCanceled)return;
  if(200===r.status)e(r);
  else if(202===r.status){if(this.query.attempts=(this.query.attempts??0)+1,this.query.attempts>5)return;
    let h=parseInt(r.headers["retry-after"]);
    this.retryDelay=isNaN(h)||0===h?5e3:h*l.A.Millis.SECOND,this.retryLater(e,t,s),t(r)}}
  catch(e){new i.A("SearchFetcher").error(e),s(e)}}
cancel(){this.isCanceled=!0,null!=this.indexingPollId&&clearTimeout(this.indexingPollId)}
retryLater(e,t,s){null!=this.indexingPollId&&clearTimeout(this.indexingPollId),
  this.indexingPollId=setTimeout(this.fetch.bind(this,e,t,s),this.retryDelay)}
```

### 5.2 Числовой контракт

| Параметр | Значение |
|---|---|
| Успех | только `200`; тело разбирается |
| Индексация | `202`; тело игнорируется |
| Источник задержки | **заголовок** `retry-after`, секунды (`parseInt`) |
| Задержка | `N * 1000` мс |
| Дефолт | `5000` мс, если заголовка нет, он не число или `0` |
| Счётчик | `query.attempts`, инкремент на каждом 202 |
| Лимит | стоп при `attempts > 5` |
| Повторов | 5 (после первого 202) |
| Всего запросов | ≤ 6 |
| Таймер | один; новый `retryLater` отменяет предыдущий |
| Отмена | `cancel()` → `isCanceled`, таймер очищается, ответ игнорируется |
| Ошибка сети/исключение | `Logger("SearchFetcher").error(e)` + failure-callback `s(e)` |
| Финальный 202 | тихий `return`: ни success, ни failure, ни нового таймера |

Важное следствие про `parseInt`: `retry-after: 0.25` даст `0` и уедет в дефолт
**5000 мс**, а не в 250 мс. Дробные значения заголовка контрактом не поддержаны.

### 5.3 Колбэки

`fetch(onSuccess, onIndexing, onFailure)`:

* `200` → `onSuccess(response)`; в action это `SEARCH_MESSAGES_SUCCESS`.
* `202` → `onIndexing(response)`; в action — `SEARCH_MESSAGES_INDEXING`
  `[D] effc40ce4f2aa27c_0@1409` (`()=>{dispatch({type:"SEARCH_MESSAGES_INDEXING",ids})}`).
* ошибка → `SEARCH_MESSAGES_FAILURE`.

`onIndexing` вызывается на **каждом** 202 (до 5 раз), а не только на первом.

### 5.4 Побочный эффект: `attempts` уходит в query

`makeRequest` каждый раз сериализует **тот же** объект `this.query`, в который
`fetch` записал `attempts`. Поэтому повторные GET содержат
`&attempts=1`, `&attempts=2`, … `&attempts=5`. Первый запрос — без `attempts`.
Для POST `/tabs` утечки нет: там сериализуется `this.payload`, собранный один раз
в конструкторе, а `attempts` живёт в `this.query`.

Реакция сервера на `attempts` не проверена — `[A]`.

### 5.5 Другие 202 в клиенте (для контраста)

| Поверхность | Источник задержки | Лимит | Код |
|---|---|---|---|
| Поиск сообщений | заголовок `retry-after`, сек | 5 повторов | `[D] 82a5…@212` |
| `POST /guilds/{id}/members-search` | **тело** `retry_after`, сек | 3 повтора (`i>3` → throw) | `[D] 4d2a…@5277460` |
| Application command index (GET) | фиксированные 5000 мс | 3 (`retries:3-c-1`) | `[D] 4d2a…@2973483` |
| Spotify token `202` | фиксированные 5000 мс | 1 на цикл | `[D] 4d2a…@7749398` |

`members-search` `[D] 4d2a…@5277460`:

```js
if(202===l.status){if(null==l.body.retry_after)throw Error("Indexing response did not include retry_after");
  if(!r)throw Error("Indexing response received but autoRetry is disabled");
  …await new Promise(e=>setTimeout(e,l.body.retry_after*m.A.Millis.SECOND)),N(e,t,n,i+1)}
```

То есть `retry_after` в теле — это **не** контракт поиска сообщений. Для сообщений
клиент смотрит только заголовок.

### 5.6 Что делает HTTP-слой с 429/5xx

`[D] 4d2a64cb5df5fc4e_0@10692774`:

```js
function f(e,t){let n=e?.["retry-after"]??e?.["Retry-After"];
  if("string"==typeof n){let e=parseInt(n,10);if(Number.isFinite(e)&&e>0)return e}
  if(null!=t&&"object"==typeof t){let e=t.retry_after;if("number"==typeof e&&Number.isFinite(e)&&e>0)return e}}
```

Статусы `[D] 4d2a…@10692387`: `E={502,504,507,598,599,522,523,524}` (ретраятся,
только если задан `retries`), `A={429,503}` (per-URL очередь + backoff
`Backoff(1000, 60000)`, минимум `1000*(retryAfter ?? 5)` мс). Поиск `retries` не
задаёт, поэтому 5xx не ретраятся, а 429/503 проходят через общий rate-limit queue.

## 6. `documents_indexed` и `doing_deep_historical_index`

Стор `[D] 4d2a64cb5df5fc4e_0@7543688` (модуль `517381`, `SearchMessageStore`):

```js
class _{isIndexing=!1;isHistoricalIndexing=!1;isFetching=!1;analyticsId=null;error=null;
  messages=null;documentsIndexed=0;totalResults=null;messageIds=new Set;isInitialFetchComplete=!1;cursor=null;
  handleSearchStart(){this.isFetching=!0,this.isIndexing=!1,this.analyticsId=null;this.error=null}
  handleSearchIndexing(){this.isInitialFetchComplete=!0,this.isIndexing=!0,this.isHistoricalIndexing=!0;this.isFetching=!1;this.error=null}
  handleSearchFailure(e){…this.documentsIndexed=0}
  handleSearchSuccess(e,t){let{analyticsId:n,cursor:i,totalResults:r,doingHistoricalIndex:a,documentsIndexed:s}=e;
    …this.isIndexing=!1;this.isHistoricalIndexing=a;…this.documentsIndexed=s;this.cursor=i;…}}
```

Контракт состояний:

* `202` → `isIndexing=true`, `isHistoricalIndexing=true`, `isFetching=false`.
  `documents_indexed` в этот момент **не обновляется** (в 202 его не читают).
* `200` → `isIndexing=false`; `isHistoricalIndexing=doing_deep_historical_index`;
  `documentsIndexed=documents_indexed` (при отсутствии поля — `0`, см. дефолт в
  компоненте `[D] b9e153f9dfaafc9f_0@31367`: `A.result.documents_indexed??0`).
  Значит «индексация ещё идёт» после 200 выражается флагом
  `doing_deep_historical_index=true`, а не статусом.
* `failure` → `documentsIndexed=0`.
* Геттеры: `getIsIndexing`, `getIsHistoricalIndexing`, `getDocumentsIndexed`.

UI-строки (ключи i18n) `[D] en-catalog.json` / `ru-catalog.json`:

| Ключ | EN | RU |
|---|---|---|
| `AXPbZr` | Before searching, we need to index this server. Give us a bit. | Нам нужно проиндексировать этот сервер перед поиском… |
| `Br0xJA` | Before searching, we need to index your messages. Give us a bit. | Перед поиском нам нужно проиндексировать ваши сообщения… |
| `Q0JJjv` | Before searching, we need to index this DM. Give us a bit. | Нам нужно проиндексировать эти ЛС перед поиском… |
| `4Y3O+O` | `{count}` «result(s) indexed so far. We’re still indexing older messages.» | «Найдено {count} результатов. Мы всё ещё индексируем старые сообщения.» |

Выбор строки по типу контекста `[D] 4d2a64cb5df5fc4e_0@7548375` (`function N`): `GUILD`/`GUILD_CHANNEL`/
`THREAD` → `AXPbZr`, `DMS` → `Br0xJA`, `CHANNEL` → `Q0JJjv`. Привязка `{count}` к
`documents_indexed` или к `total_results` в декодированных чанках не найдена —
`[A]`; по смыслу строки это `documents_indexed`.

## 7. NSFW-гейтинг

### 7.1 Где живёт состояние

| Поле | Где | Тип | Наблюдение |
|---|---|---|---|
| `channel.nsfw` | gateway `READY.d.guilds[].channels[]`, REST Channel | bool | `"false"` в capture `[D] gateway/by-type/002-READY.json` |
| `guild.nsfw` | `READY.d.guilds[].properties` | bool | `"false"` |
| `guild.nsfw_level` | там же | int | `0`; enum: `DEFAULT=0`, `EXPLICIT=1`, `SAFE=2`, `AGE_RESTRICTED=3` `[D] 4d2a…@380077` |
| `user.nsfw_allowed` | `READY.d.user` | bool \| null | `true` |
| `user.age_verification_status` | `READY.d.user` | int | enum: `UNVERIFIED=1`, `VERIFIED_TEEN=2`, `VERIFIED_ADULT=3`, `INFERRED_ADULT=4`, `INFERRED_TEEN=5`, `CLIENT_ONLY_PENDING=99999` `[D] 4d2a…@2277765` |
| `regional_feature_config.age_gated_features` | `READY.d` | int (битовая маска) | `687` |
| `regional_feature_config.teen_by_default_settings` | `READY.d` | int (битовая маска) | есть, к поиску не относится |

Гильдия считается NSFW, если `nsfw_level ∈ {EXPLICIT, AGE_RESTRICTED}`
`[D] 4d2a64cb5df5fc4e_0@8912066` (`function m` в модуле `260509`), либо, при старом эксперименте,
`owner_configured_content_level ∈ {1,3}`. Канал — по флагу `channel.nsfw`.

### 7.2 `include_nsfw` в запросе

`[D] 4d2a…@7550064` (модуль фильтров):

```js
function v(e,t){if(c.A.didAgree(t)){let t=A.default.getCurrentUser();
  null!=t&&(e.include_nsfw=null==t.nsfwAllowed||t.nsfwAllowed)}}
```

Правила:

1. Параметр добавляется **только** если `GuildNSFWAgreeStore.didAgree(guild_id)`.
2. Значение `true`, если `user.nsfw_allowed` равен `null` (неизвестно) или `true`;
   `false` — только при явном `nsfw_allowed === false`.
3. Для приватных каналов (`CHANNEL`, guild_id = null) параметр не добавляется
   (`mt(context)` возвращает null).
4. Для `/tabs` правило то же: `fetchTabMessages` вызывает `L5(g, guildId)` до
   сборки payload `[D] effc40ce4f2aa27c_0@606`.
5. Жёсткое исключение — mod view (карточка участника): и GET-поиск его сообщений
   (`eq(t,e,{include_nsfw:!0})`), и tabs-payload
   (`track_exact_total_hits:!0,include_nsfw:!0`) ставят `true` вручную
   `[D] b9e153f9dfaafc9f_0@22896` и `@25821`. Согласие гильдии там не проверяется.

Live-подтверждение обратной стороны: ни в одном захваченном GET-запросе
(`capture-run2.jsonl` — 11 шт., `ui8/20f-search-network.json`, `ui8/20i-scope-compare.json`)
параметра `include_nsfw` нет — согласия не было `[D]`.

### 7.3 Согласие и `age_gated_features`

`GuildNSFWAgreeStore` `[D] 4d2a64cb5df5fc4e_0@9090635` (модуль `517019`):

```js
let l="GuildNSFWAgreeStore",o={};
class d extends i.Ay.Store{…initialize(){o=r.w.get(l)??o}
  didAgree(e){return!(null==e||(0,s.r)())&&(o[e]||!1)}}
…{GUILD_NSFW_AGREE:function(e){let{guildId:t}=e;o[t]=!0,r.w.set(l,o)}}
```

* Согласие — map `guild_id → true` в localStorage (через `r.w`, с fallback в память)
  под ключом `GuildNSFWAgreeStore`, ставится действием `GUILD_NSFW_AGREE`
  (модалка age gate) `[D] 4d2a…@9090635`.
* `s` здесь — модуль `597306` `[D] 4d2a…@2806159`:

  ```js
  function s(){let e=(0,r.p9)();return(0,a.d6)(i.t.AGE_GATED_SPACES)&&e}
  ```

* `p9()` = **не взрослый**: `p9:()=>T`, `function T(){return!C()}`, а `C()` истинно
  для `VERIFIED_ADULT`/`INFERRED_ADULT` `[D] 4d2a…@2787350`.

Итоговая формула гейта:

```
didAgree(guild) = guild != null
               && !(isFeatureAgeGated(AGE_GATED_SPACES) && user.age_verification_status ∉ {3,4})
               && storedAgreement[guild] === true
```

То есть при включённом `AGE_GATED_SPACES` не-взрослый **не может** разблокировать
NSFW-контент старым per-guild согласием: `didAgree` принудительно `false`,
`include_nsfw` не отправляется. Взрослый по-прежнему проходит через сохранённое
согласие.

### 7.4 `age_gated_features` — битовая маска

`[D] 4d2a64cb5df5fc4e_0@7343467` (модуль `923495`):

```js
class l{gatedFeatures;constructor(e){this.gatedFeatures=e}
  isFeatureAgeGated(e){return(this.gatedFeatures&e)!=0}
  hasAgeGatedFeatures(){return(this.gatedFeatures??0)!==0}}
…static fromConnectionOpen(e){return new d(new s(e.teen_by_default_settings),
  new l(e.age_gated_features),new o(!0===e.should_collect_app_store_signal))}
```

Enum фич `[D] 4d2a…@11850986`:

```js
NONE=0, AGE_GATED_SPACES=1, SENSITIVE_CONTENT_SHOW_SETTING=2, DM_PRIVACY_SETTINGS=4,
MESSAGE_REQUEST_RESTRICTIONS_TOGGLE=8, LARGE_SERVER_ACCESS=16, COMMANDS_TOGGLE=32,
REACTIVE_CHECK=64, STAGE_SPEAKING=128, JOIN_DISCOVERABLE_SERVER=256, NOTICE=512
```

Наблюдённое `687 = 1|2|4|8|32|128|512` — включены `AGE_GATED_SPACES`,
`SENSITIVE_CONTENT_SHOW_SETTING`, `DM_PRIVACY_SETTINGS`,
`MESSAGE_REQUEST_RESTRICTIONS_TOGGLE`, `COMMANDS_TOGGLE`, `STAGE_SPEAKING`, `NOTICE`;
выключены `LARGE_SERVER_ACCESS`, `REACTIVE_CHECK`, `JOIN_DISCOVERABLE_SERVER`.

Для поиска важен ровно бит `AGE_GATED_SPACES=1` — он переключает формулу §7.3.
Остальные биты — соседние age-gated фичи (настройки приватности, стейджи, команды).

### 7.5 Чего в контракте нет

* Отдельного поля `age_gated` (bool) в Channel/Guild/User/поисковом ответе нет:
  поиск по дампу даёт только `age_gated_features`, `AGE_GATED_SPACES`,
  `ACTIVITY_AGE_GATED`, `INVALID_ACTIVITY_LAUNCH_AGE_GATED`,
  `USER_GUILD_JOIN_DISCOVERABLE_SERVER_AGE_GATED`. `requires_age_gate` существует
  только у embedded activities `[D] 4d2a…@2634436`, к поиску не применяется.
* Серверная фильтрация по `include_nsfw` (что именно вырезается из `messages`)
  runtime-подтверждения не имеет — `[A]`. Код клиента лишь отправляет флаг.
* Скрытие/блюр NSFW-результатов в панели поиска кодом не найдено: в чанках
  результатов поиска (`26d4575da0486f0d_0`, `dfb980946807c409_0`) слова `nsfw` нет.
  Гейт работает на входе (флаг) и на уровне рендера сообщений/канала, не в поиске.
* Аналог правила для RPC (не поиск): сообщения канала отдаются в payload только при
  `!channel.isNSFW() || user.nsfwAllowed === true` `[D] 4d2a…@7402509`. Это
  подтверждает смысл `nsfw_allowed`, но не является контрактом поиска.

## 8. Сверка с текущей реализацией M-13

Факты по коду (read-only, для приёмки):

| Пункт | Официальный клиент | `rust/gui-spike/src/search.rs` |
|---|---|---|
| `limit` в GET | не отправляется | отправляется, 1..=25 |
| `include_nsfw` | отправляется по §7.2 | отсутствует |
| `search_session_id` / `search_query_id` | для discoverable | отсутствуют |
| `link_hostname` / `attachment_extension` / `attachment_filename` | есть | отсутствуют |
| `search_everywhere` | есть (флаг) | отсутствует |
| `202` | 5 повторов по `retry-after` | не обрабатывается |
| `documents_indexed` | в ответе, опционально | нет в `SearchResponse` |
| `analytics_id`, `channels`, `threads`, `members`, `cursor` | есть | нет в `SearchResponse` |
| `author_id`/`channel_id` массивы | повтор ключа | склейка через запятую (`StringOrVec::into_joined`) |

Склейка массивов через запятую — расхождение с официальным клиентом: он отправляет
повтор ключа (`channel_id=a&channel_id=b`). Принимает ли сервер `channel_id=a,b` —
`[A]`; в коде клиента такого формата нет.

## 9. Остаточные gap

| Пункт | Статус | Что нужно |
|---|---|---|
| Ответ `202` вживую (статус + заголовок `retry-after`) | `[A]` | live-поиск в большой гильдии без индекса |
| Дробный `retry-after` (`0.25`) | `[A]` | тот же live-захват |
| Максимальный `offset` | `[A]` | запрос с большим `offset` |
| Серверная фильтрация NSFW по `include_nsfw` | `[A]` | NSFW-гильдия + согласие/без |
| `attempts` в query на повторе | `[D]` код, `[A]` реакция сервера | live или лог |
| Привязка `{count}` в `4Y3O+O` к `documents_indexed` | `[A]` | чанк с компонентом прогресса |
| Порядок контекста в `messages[i][]` | `[A]` | разбор реального ответа |
| Приём `limit` сервером | `[A]` | запрос с `limit=10` |
| Приём `channel_id=a,b` сервером | `[A]` | запрос со списком |

## 10. Артефакты

| Файл | Размер | md5 |
|---|---|---|
| `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` | 12 410 287 | `934b1289b7ebf17bc0fd09569b23b438` |
| `/home/sanya/re-deep/decoded/82a5af57ee11dcd5_0` (`0a6ed6a6830e5071.js`, `SearchFetcher`) | 2 443 | `8594228760b7d6c62f568202b65aa340` |
| `/home/sanya/re-deep/decoded/effc40ce4f2aa27c_0` (`256796`, search actions) | 21 483 | `39ac7ffa2f4da42f15c652f48f465dcf` |
| `/home/sanya/re-deep/decoded/b9e153f9dfaafc9f_0` (mod view search) | 113 417 | `c01b858e65d6f7f5f42a2e49142eee72` |
| `/home/sanya/re-deep/decoded/25883e62976b3cc0_0` (`488428` querystring) | — | `b14528da6bab80d3a808ebc910723aba` |
| `/home/sanya/re-deep/decoded/8b501595bbf87517_0` (`998736` stringify) | — | `1231ae0ce2d0563a7a3ec213acf0d02b` |
| `/home/sanya/re-deep/ui8/20f-search-network.json` | — | `ac2227b23ac2502d4ead25077c112698` |
| `/home/sanya/re-deep/ui8/20g-search-probe.json` | — | `76787e641d2c43f1f9de55de5e856371` |
| `/home/sanya/re-deep/ui8/20i-scope-compare.json` | — | `49e87d7d13484ec1177ebfa253475bf4` |
| `/home/sanya/re-deep/gateway/by-type/002-READY.json` | — | `5e1e4c2c9ee5c5a0b1495335dde555f5` |

Смещения — байтовые от начала однострочного decoded-файла. Для файлов без размера
(`25883e…`, `8b501595…`) смещения указывают позицию в декодированном тексте, а не
полный размер артефакта.

Связанные документы: `RE-DEEP-REA.md` §2 (базовая таблица фильтров),
`ATTACH-RE.md` §1 (сводка), `PARITY/PARITY-MATRIX.md` (строка M-13),
`rust/gui-spike/src/search.rs` + `rust/gui-spike/tests/search_contract.rs` (текущий код).
