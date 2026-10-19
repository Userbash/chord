# D-02 — REST-клиент Discord: методы, тела, auth-поток (живые данные)

Источник: два CDP-захвата, снятые на живом Discord 1.0.162 (Linux, Electron 42.11.12).
Задача воркера: закрыть D-02 по **живым** данным и сверить со статической картой (`rest-static.md`, 904 записи).

## 0. Итог в числах

| Метрика | Значение |
|---|---|
| Событий `req` в снимке | 6654 |
| Уникальных requestId | 6651 (дублей: 3) |
| API-запросов (только /api/v9, `api` + `api-auth`) | 133 |
| Уникальных API-эндпоинтов | **47** |
| Из них с телом запроса (postData) | **6** |
| Из них с телом ответа | **0** |
| Захваченных тел ответов всего | 200 (все — `/assets/*.js`) |
| Запросов с заголовком Authorization | 221 |
| Отфильтровано как шум | 6521 из 6654 (98.0 %) |
| Сверка со статикой | 44/47 эндпоинтов подтверждены статикой (метод+путь) |

## 0.1 Индекс доказательств (для выборочной проверки)

| Утверждение | Где смотреть (requestId / команда) |
|---|---|
| Супер-свойства (base64 JSON, 22 поля) | `run2|95974.2323, run2|95974.3486, run2|95974.3741` |
| Authorization в нижнем регистре (3×) | `run2|95974.3434, run2|95974.8036, ready|57992.7364` |
| Тело `{token,last_viewed}` у ack | `run2|95974.3831, run2|95974.4057, run2|95974.8164` |
| Тело `{}` у `PUT guilds/{id}/members/@me` | `ready|57992.6989, run2|95974.7409` |
| Тело `{allow_triggers}` | `run2|95974.3423, run2|95974.7586, ready|57992.7308` |
| Тело `{message_ids:[…]}` | `run2|95974.8279` |
| Обрезанное тело settings-proto (4001 симв.) | `run2|95974.8275` |
| Телеметрия `{token, events}` | `run2|95974.2417 (science), run2|95974.8268 (beaker)` |
| 304 (условный GET) | `run2|95974.3428, run2|95974.7594, ready|57992.7316` |
| 404 (негативные пробы) | `run2|95974.2324, run2|95974.7477, ready|57992.7075 / run2|95974.3431, run2|95974.7590, ready|57992.7312 / run2|95974.3801, run2|95974.7989` |
| 429 rate-limit | `run2|95974.8271` |
| Голосовой `op:0` Identify c token+session_id (текстовый WS) | `run2|95974.8158, ready|57992.7568` |
| Основной шлюз (бинарные zstd/ETF кадры → READY нечитаем) | `run2|95974.1389, run2|95974.5532, ready|57992.5112` |
| Тела ответов = только /assets/*.js | `bash rest-d02-verify.sh → раздел 8 (body non-asset=0)` |
| Все числа отчёта | `bash rest-d02-verify.sh → rest-d02.verify.txt` |

## 1. Методика и воспроизводимость

```
Входные файлы (только чтение):
  d02-snapshot/capture-run2.snap.jsonl
    md5=abfe14b76cec1e559c3b45522bc44b7a  bytes=4137676  lines=10170
  d02-snapshot/capture-ready.snap.jsonl
    md5=b43636da09a4a2c0f08d36b1ce66a46a  bytes=2479139  lines=5104
  снимок сделан: 2026-10-19T11:51:47+05:00
  capture-run2.jsonl — ЖИВОЙ файл, дозаписывается во время анализа (см. U-10),
  поэтому все числа привязаны к снимку в d02-snapshot/, а не к имени файла.
После снимка: node rest-d02.mjs && node rest-d02-report.mjs
```

- Разбор: `rest-d02.mjs` — построчный JSONL-разбор, join `req`↔`res` по паре `(файл, requestId)`.
- Нормализация пути: сегмент из 16–21 цифры → `{id}`, UUID → `{uuid}`, прочие числа → `{num}`; query отброшен в отдельный словарь.
- Классификация: `api` (в таблицу), `telemetry-*`, `static-assets`, `cdn-media`, `data-url`, `cloudflare-challenge`, `web-navigation`, `third-party` (в раздел «Шум»).
- Независимый контроль: `bash rest-d02-verify.sh` — те же числа считаются `jq`+`sort`+`uniq`+`md5sum`+`join` **вне кода разбора**; полный вывод в `rest-d02.verify.txt`. Расхождений с `rest-d02.json` нет.
- Оракул «числа не из реализации»: md5 снимка + независимый подсчёт строк/reqId/кодов в jq. Тела запросов цитируются как есть (санитизированные), requestId указан для каждого примера.

Ограничение: `hb`-события снимка показывают `bodiesTaken:200` — это потолок скрипта захвата, а не свойство клиента.

## 2. Таблица эндпоинтов (/api/v9)

Все запросы ушли на `https://discord.com/api/v9/*`. `count` — число событий `req` в снимке.
`статус` — коды из `res` того же requestId (пусто = ответ не попал в снимок).

| # | Метод | Путь | Вызовов | Коды ответов | Тело запроса (ключи) | auth-ключ | static |
|---|---|---|---|---|---|---|---|
| 1 | GET | `/channels/{id}/messages` | 11 | 200×11 | — | Authorization | ✔ |
| 2 | GET | `/applications/public` | 7 | 200×7 | — | Authorization | ✔ |
| 3 | GET | `/games/autocomplete` | 6 | 200×6 | — | Authorization | ✔ |
| 4 | POST | `/channels/{id}/messages/{id}/ack` | 5 | 200×5 | `token, last_viewed` | Authorization | ✔ |
| 5 | GET | `/quests/@me` | 5 | 200×5 | — | Authorization | ✔ |
| 6 | GET | `/users/{id}/profile` | 4 | 200×4 | — | Authorization | ✔ |
| 7 | GET | `/users/@me/billing/subscriptions` | 4 | 200×4 | — | Authorization | ✔ |
| 8 | GET | `/content-inventory/users/@me` | 3 | 200×3 | — | Authorization | ✔ |
| 9 | GET | `/games/detectable/exclusions` | 3 | 304×3 | — | Authorization | ✔ |
| 10 | GET | `/partner-sdk/storefront-config` | 3 | 200×3 | — | Authorization | ✔ |
| 11 | GET | `/promotions` | 3 | 200×3 | — | Authorization | ✔ |
| 12 | GET | `/quests/decision` | 3 | 200×3 | — | Authorization | ✔ |
| 13 | GET | `/quests/get-decisions` | 3 | 200×3 | — | Authorization | ✔ |
| 14 | GET | `/users/@me` | 3 | 200×3 | — | authorization | ✔ |
| 15 | GET | `/users/@me/achievements/unread-state` | 3 | 200×3 | — | Authorization | ✔ |
| 16 | GET | `/users/@me/activities/statistics/applications` | 3 | 200×3 | — | Authorization | ✔ |
| 17 | GET | `/users/@me/applications/{id}/entitlements` | 3 | 200×3 | — | Authorization | ✔ |
| 18 | GET | `/users/@me/billing/checkout-recovery` | 3 | 200×3 | — | Authorization | ✔ |
| 19 | POST | `/users/@me/billing/user-offer` | 3 | 204×3 | `allow_triggers` | Authorization | ✔ |
| 20 | GET | `/users/@me/collectibles-marketing` | 3 | 200×3 | — | Authorization | ✔ |
| 21 | GET | `/users/@me/entitlements` | 3 | 200×3 | — | Authorization | ✔ |
| 22 | GET | `/users/@me/library` | 3 | 200×3 | — | Authorization | ✔ |
| 23 | GET | `/users/@me/mfa/webauthn/credentials` | 3 | 200×3 | — | Authorization | ✔ |
| 24 | GET | `/users/@me/referrals/eligibility` | 3 | 404×3 | — | Authorization | ✔ |
| 25 | GET | `/users/@me/saved-messages` | 3 | 404×3 | — | Authorization | ✔ |
| 26 | GET | `/users/@me/settings-proto/{num}` | 3 | 200×3 | — | Authorization | ✔ |
| 27 | GET | `/users/@me/virtual-currency/balance` | 3 | 200×3 | — | Authorization | ✔ |
| 28 | GET | `/widget-configs/developer` | 3 | 200×3 | — | Authorization | ✔ |
| 29 | GET | `/widget-configs/featured` | 3 | 200×3 | — | Authorization | ✔ |
| 30 | GET | `/activities/shelf` | 2 | 200×2 | — | Authorization | ✔ |
| 31 | GET | `/guilds/{id}/application-command-index` | 2 | 200×2 | — | Authorization | метод? |
| 32 | PUT | `/guilds/{id}/members/@me` | 2 | 204×2 | `(empty-object)` | Authorization | ✔ |
| 33 | GET | `/soundboard-default-sounds` | 2 | 200×2 | — | Authorization | ✔ |
| 34 | GET | `/users/{id}/application-identities` | 2 | 200×2 | — | Authorization | ✔ |
| 35 | GET | `/users/@me/application-command-index` | 2 | 200×2 | — | Authorization | ✔ |
| 36 | GET | `/users/@me/notes/{id}` | 2 | 404×2 | — | Authorization | ✔ |
| 37 | POST | `/beaker` | 1 | 204×1 | `token, events` | Authorization | нет |
| 38 | PATCH | `/channels/{id}/explicit-media` | 1 | 204×1 | `message_ids` | Authorization | ✔ |
| 39 | PUT | `/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` | 1 | 204×1 | — | Authorization | метод? |
| 40 | GET | `/channels/{id}/messages/pins` | 1 | 200×1 | — | Authorization | ✔ |
| 41 | GET | `/channels/{id}/threads/search` | 1 | 200×1 | — | Authorization | ✔ |
| 42 | GET | `/collectibles-products/{id}` | 1 | 200×1 | — | Authorization | ✔ |
| 43 | GET | `/games` | 1 | 200×1 | — | Authorization | ✔ |
| 44 | GET | `/storefront/products/skus` | 1 | 200×1 | — | Authorization | ✔ |
| 45 | GET | `/users/@me/billing/payment-sources` | 1 | 200×1 | — | Authorization | ✔ |
| 46 | PATCH | `/users/@me/settings-proto/{num}` | 1 | 200×1 | `(non-json)` | Authorization | ✔ |
| 47 | GET | `/users/@me/survey` | 1 | 429×1 | — | Authorization | ✔ |

Итого 133 запросов на 47 уникальных маршрутов. Кодов ответов: 200, 204, 304, 404, 429.

Распределение кодов:

| Код | Кол-во | Комментарий |
|---|---|---|
| 200 | 113 | ок |
| 204 | 8 | успех без тела (ack, user-offer, beaker, explicit-media) |
| 304 | 3 | условный GET (кэш `games/detectable/exclusions`) |
| 404 | 8 | негативные пробы фич (`referrals/eligibility`, `saved-messages`, `notes/{id}`) |
| 429 | 1 | rate-limit на `users/@me/survey` (единственный) |

Детали по не-200:

| Эндпоинт | Код | requestId |
|---|---|---|
| `GET /games/detectable/exclusions` | 304 (3×) | run2|95974.3428 |
| `POST /users/@me/billing/user-offer` | 204 (3×) | run2|95974.3423 |
| `GET /users/@me/referrals/eligibility` | 404 (3×) | run2|95974.2324 |
| `GET /users/@me/saved-messages` | 404 (3×) | run2|95974.3431 |
| `PUT /guilds/{id}/members/@me` | 204 (2×) | run2|95974.7409 |
| `GET /users/@me/notes/{id}` | 404 (2×) | run2|95974.3801 |
| `POST /beaker` | 204 (1×) | run2|95974.8268 |
| `PATCH /channels/{id}/explicit-media` | 204 (1×) | run2|95974.8279 |
| `PUT /channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` | 204 (1×) | run2|95974.8046 |
| `GET /users/@me/survey` | 429 (1×) | run2|95974.8271 |

## 3. Query-параметры

Всего 45 уникальных имён параметров. Частые:

| Параметр | Раз | Где встречается |
|---|---|---|
| `application_ids` | 58 | `GET /api/v9/applications/public`×58 |
| `visible_guild_ids` | 21 | `GET /api/v9/quests/decision`×21 |
| `limit` | 16 | `GET /api/v9/channels/{id}/messages`×11, `GET /api/v9/users/@me/billing/subscriptions`×3 |
| `platform` | 6 | `GET /api/v9/promotions`×3, `GET /api/v9/users/@me/collectibles-marketing`×3 |
| `placement` | 6 | `GET /api/v9/quests/decision`×3, `GET /api/v9/quests/get-decisions`×3 |
| `client_heartbeat_session_id` | 6 | `GET /api/v9/quests/decision`×3, `GET /api/v9/quests/get-decisions`×3 |
| `client_ad_session_id` | 6 | `GET /api/v9/quests/decision`×3, `GET /api/v9/quests/get-decisions`×3 |
| `q` | 6 | `GET /api/v9/games/autocomplete`×6 |
| `filter_group` | 6 | `GET /api/v9/games/autocomplete`×6 |
| `locale` | 5 | `GET /api/v9/promotions`×3, `GET /api/v9/collectibles-products/{id}`×1 |
| `country_code` | 4 | `GET /api/v9/users/@me/library`×3, `GET /api/v9/storefront/products/skus`×1 |
| `type` | 4 | `GET /api/v9/users/{id}/profile`×3, `PUT /api/v9/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me`×1 |
| `with_mutual_guilds` | 4 | `GET /api/v9/users/{id}/profile`×4 |
| `with_mutual_friends` | 4 | `GET /api/v9/users/{id}/profile`×4 |
| `with_mutual_friends_count` | 4 | `GET /api/v9/users/{id}/profile`×4 |
| `with_sku` | 3 | `GET /api/v9/users/@me/entitlements`×3 |
| `with_application` | 3 | `GET /api/v9/users/@me/entitlements`×3 |
| `entitlement_type` | 3 | `GET /api/v9/users/@me/entitlements`×3 |
| `exclude_ended` | 3 | `GET /api/v9/users/@me/entitlements`×3 |
| `exclude_consumed` | 3 | `GET /api/v9/users/@me/applications/{id}/entitlements`×3 |
| `bookmark_ids` | 3 | `GET /api/v9/users/@me/saved-messages`×3 |
| `include_inactive` | 3 | `GET /api/v9/users/@me/billing/subscriptions`×3 |
| `exclude_unpaid_statuses` | 3 | `GET /api/v9/users/@me/billing/subscriptions`×3 |
| `subscription_type` | 3 | `GET /api/v9/users/@me/billing/subscriptions`×3 |

Показательные примеры (реальные строки из снимка; полные URL — в `rest-d02.json` → `endpoints[].requestIds`):

- `GET /applications/public?application_ids=…` ×7 — батч приложений; в одном запросе до 15 повторов `application_ids` (58 вхождений параметра суммарно).
- `GET /users/{id}/profile?type=sidebar&with_mutual_guilds=true&with_mutual_friends=true&with_mutual_friends_count=false` — вариант для сайдбара; `type=account_popout` с `false` — вариант для попапа. Параметр `type` переключает форму ответа.
- `GET /channels/{id}/messages` — пагинация: `?limit=12` ×8, `?before=<snowflake>&limit=24` ×2, `?limit=30&around=<snowflake>` ×1. То есть `limit` + один из курсоров `before`/`after`/`around`.
- `GET /quests/decision?placement=1&client_heartbeat_session_id=<uuid>&client_ad_session_id=<uuid>&visible_guild_ids=<snowflake>×N` — 7 значений `visible_guild_ids` в одном запросе.
- `GET /games/autocomplete?q=<URL-encoded>&filter_group=default` — 6 запросов, поисковые строки введены пользователем (кириллица); один и тот же `q` повторяется дважды → дебаунс/повтор при вводе.

## 4. Тела запросов (postData)

Тело захвачено у **6** маршрутов (всего 13 запросов, все — с `Content-Type: application/json`). Ниже — ключи верхнего уровня JSON + санитизированный пример ≤200 символов и ссылка на requestId.

Всего `postData` в снимке: 120 (88 — `/science`, 13 — прочие API, остальное — Cloudflare jsd). Длины: min 2, median 834, max 4001 — то есть **скрипт захвата обрезает тело на 4000 символах**: 12 запросов упёрлись в потолок.

Усечённые (важно для схем):

| requestId | Маршрут | Длина |
|---|---|---|
| `run2|95974.2116` | `POST /cdn-cgi/challenge-platform/h/b/jsd/oneshot/d2d3316b729e/0.6136024352790642:1792386030:xXSD0q7OtCBFmyu7mgupgOBkHgWGMGnre-kB2YvXedE/a4cdb9214a6c3675` | 4001 |
| `run2|95974.2417` | `POST /api/v9/science` | 4001 |
| `run2|95974.3413` | `POST /api/v9/science` | 4001 |
| `run2|95974.4044` | `POST /api/v9/science` | 4001 |
| `run2|95974.7478` | `POST /api/v9/science` | 4001 |
| `run2|95974.7951` | `POST /api/v9/science` | 4001 |
| `run2|95974.7990` | `POST /api/v9/science` | 4001 |
| `run2|95974.8047` | `POST /api/v9/science` | 4001 |
| `run2|95974.8163` | `POST /api/v9/science` | 4001 |
| `run2|95974.8275` | `PATCH /api/v9/users/@me/settings-proto/2` | 4001 |
| `ready|57992.7285` | `POST /api/v9/science` | 4001 |
| `ready|57992.7574` | `POST /api/v9/science` | 4001 |

### POST `/channels/{id}/messages/{id}/ack` ×5

- Ключи верхнего уровня: `token`, `last_viewed`
- Пример (requestId `ready|57992.7577`):

```json
{"token":null,"last_viewed":4310}
```

### POST `/users/@me/billing/user-offer` ×3

- Ключи верхнего уровня: `allow_triggers`
- Пример (requestId `ready|57992.7308`):

```json
{"allow_triggers":true}
```

### PUT `/guilds/{id}/members/@me` ×2

- Ключи верхнего уровня: `(empty-object)`
- Пример (requestId `ready|57992.6989`):

```json
{}
```

### POST `/beaker` ×1

- Ключи верхнего уровня: `token`, `events`
- Пример (requestId `run2|95974.8268`):

```json
{"token":"REDACTED","events":[{"type":"client_telemetry","properties":{"client_track_timestamp":1792392393947,"rpc_success_count":26,"rpc_failure_count":0,"first_seen_event_sequence_number":0,"last_se…
```

### PATCH `/channels/{id}/explicit-media` ×1

- Ключи верхнего уровня: `message_ids`
- Пример (requestId `run2|95974.8279`):

```json
{"message_ids":["{id}"]}
```

### PATCH `/users/@me/settings-proto/{num}` ×1

- Ключи верхнего уровня: `(non-json)`
- Пример (requestId `run2|95974.8275`):

```json
{"settings":"REDACTED…
```

Наблюдения по телам:

- `POST /science` и `POST /beaker` — одна и та же схема телеметрии: `{token, events:[{type, properties:{...}}]}`. Пример события: `libdiscore_loaded`, `app_web_perf_startup_metrics`, `client_telemetry` (requestId `run2|95974.2417`, `run2|95974.8268`).
- `POST /channels/{id}/messages/{id}/ack` — `{token, last_viewed}`; `token` бывает `null`, `last_viewed` — номер сообщения (requestId `ready|57992.7577`).
- `PUT /guilds/{id}/members/@me?lurker=true` — тело **пустой JSON-объект** `{}` (requestId `ready|57992.6989`, `run2|95974.7409`). Единственный случай «тело есть, полей нет»; без учёта этого случая эндпоинт выглядел бы как «без тела».
- `PATCH /channels/{id}/explicit-media` — `{message_ids:[snowflake]}` (requestId `run2|95974.8279`).
- `PATCH /users/@me/settings-proto/{num}` — тело `{"settings":"<base64 protobuf>"}`, причём в снимке оно **обрезано** на 4001 символе, поэтому не парсится как JSON и помечено `(non-json)`. Внутренний слой — protobuf (поля вида `flag_ru`, `face_with_monocle`, `blue_circle` — это карта эмодзи-настроек), но целиком он недоступен из-за лимита захвата (requestId `run2|95974.8275`).

## 5. Тела ответов

**Важный пробел:** в снимке 200 событий `body`, и все они — статика `/assets/*.js` (script-чанки webpack, host `discord.com`). Ни одного тела ответа REST-эндпоинта.

Доказательство (независимая проверка, не из кода отчёта):

```bash
jq -r 'select(.ev=="body") | .url' d02-snapshot/capture-ready.snap.jsonl | wc -l        # 200
jq -r 'select(.ev=="body") | .url' d02-snapshot/capture-ready.snap.jsonl \
  | grep -vc '/assets/'                                                              # 0
jq -r 'select(.ev=="body") | (.body|tostring|length)' d02-snapshot/capture-ready.snap.jsonl \
  | sort -n | awk 'NR==1{m=$1} {a[NR]=$1} END{print NR, m, $1, a[int(NR/2)]}'            # 200 141 20001 520
```

Размеры тел: min 141, median 520, max **20 001** (потолок скрипта — 20 000 символов + перевод строки), суммарно 367 961 символ. 200 из 200 ответов — код 200.

Следствия для D-02:

- Схемы ответов `READY`, `/gateway`, `/users/@me`, `/channels/*/messages`, `/science`, `/experiments` **из этого захвата не извлекаются**.
- WebSocket-кадры в снимке: 494 `wsrecv` (471 бинарный + 23 текстовых), 19 `wssent`, 120 `wssent-bin`, 6 `wsc`. Бинарные кадры — base64 от zstd-сжатого ETF (`firstHex` начинается с `28b52ffd` — магия zstd), т.е. **тело READY в снимке нечитаемо**; текстовые кадры относятся к голосовому сокету (см. §6.5).
- `POST /science` позволяет реконструировать схему телеметрии **со стороны запроса** (см. §4) — это единственная «богатая» схема, доступная в этом снимке.
- `GET /users/@me` (×3) ответов не имеет: кэш/304 не помогли, тело не перехвачено.

## 6. Auth-поток

### 6.1 Что наблюдается живьём

| Признак | Значение |
|---|---|
| Запросов с заголовком `Authorization` | 221 |
| ─ из них ключ `Authorization` | 218 |
| ─ из них ключ `authorization` (нижний регистр) | 3 — все 3 на `GET /api/v9/users/@me` |
| С `X-Super-Properties` | 221 |
| С `X-Discord-Locale` | 221 |
| С `X-Discord-Timezone` | 221 |
| С `X-Debug-Options` | 221 |
| С `Cookie` | 0 (см. U-05) |
| `POST /api/v9/science` с Authorization | 88 из 88 — телеметрия авторизована |
| Запросов к `/api/v9/auth/*` | 0 (см. U-02) |
| Запросов к `GET /api/v9/gateway` | 0 (см. U-03) |

Ключевое: **все 133 API-запроса из 133** идут с одним и тем же набором заголовков — `Authorization` + `X-Super-Properties` + `X-Discord-Locale` + `X-Discord-Timezone` + `X-Debug-Options` + `Accept-Language` (`requestsWithFullAuthHeaderSet = 133`). Это ровно та «шапка», которую статика приписывает `prepareRequest` (rest-static §4, модуль 926456). 13 запросов дополнительно несут `Content-Type: application/json` — ровно те, у которых есть тело.

Полный список эндпоинтов с auth-ключом — в `rest-d02.json` → `auth.endpointsWithAuth` (47 маршрутов).

### 6.2 Супер-свойства (base64 JSON) — раскодированный пример

requestId `run2|95974.2323` (`GET /api/v9/channels/{id}/messages?limit=12`), заголовок присутствует в 221 запросах (все авторизованные — 133 API + 88 science):

```json
{
  "os": "Linux",
  "browser": "Discord Client",
  "release_channel": "stable",
  "client_version": "1.0.162",
  "os_version": "7.2.10-1-cachyos",
  "os_arch": "x64",
  "app_arch": "x64",
  "system_locale": "ru",
  "has_client_mods": false,
  "client_launch_id": "ab3d725a-82e4-4d66-b77c-64a8463fb04c",
  "browser_user_agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36",
  "browser_version": "42.11.12",
  "window_manager": "KDE,unknown",
  "distro": "\"Arch Linux\"",
  "runtime_environment": "native",
  "display_server": "wayland",
  "client_build_number": 641004,
  "native_build_number": 94186,
  "client_event_source": null,
  "launch_signature": "da02ad21-144b-43b5-9618-a18392f1c066",
  "client_heartbeat_session_id": "f615427c-47a2-49ed-9482-d4b192dd781e",
  "client_app_state": "focused"
}
```

Разбор полей (22 ключа):

| Поле | Значение (пример) | Смысл |
|---|---|---|
| `os` | `Linux` | ОС клиента |
| `browser` | `Discord Client` | имя клиента |
| `release_channel` | `stable` | канал обновлений |
| `client_version` | `1.0.162` | версия Discord |
| `os_version` | `7.2.10-1-cachyos` | версия ядра |
| `os_arch` | `x64` | архитектура ОС |
| `app_arch` | `x64` | архитектура приложения |
| `system_locale` | `ru` | локаль системы |
| `has_client_mods` | `false` | признак модов клиента (Vencord/BetterDiscord) |
| `client_launch_id` | `ab3d725a-82e4-4d66-b77c-64a8463fb04c` | UUID запуска клиента |
| `browser_user_agent` | `Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36` | полный UA |
| `browser_version` | `42.11.12` | версия Chromium/Electron |
| `window_manager` | `KDE,unknown` | оконный менеджер (KDE,unknown) |
| `distro` | `"Arch Linux"` | дистрибутив (значение в кавычках — `"Arch Linux"`) |
| `runtime_environment` | `native` | native (Electron) vs web |
| `display_server` | `wayland` | wayland/x11 |
| `client_build_number` | `641004` | номер сборки клиента |
| `native_build_number` | `94186` | номер нативной сборки |
| `client_event_source` | `null` | источник события (null) |
| `launch_signature` | `da02ad21-144b-43b5-9618-a18392f1c066` | UUID подписи запуска |
| `client_heartbeat_session_id` | `f615427c-47a2-49ed-9482-d4b192dd781e` | UUID сессии телеметрии (связывает запросы) |
| `client_app_state` | `focused` | состояние окна (focused) |

Единственный заголовок, который статика обещает, но которого **нет** в живых данных, — `X-Fingerprint`/`X-Installation-ID`/`X-Routing-Key`/`x-client-trace-id` (см. U-08). Наблюдались только `X-Context-Properties` (base64, 8 запросов на 3 маршрутах: `quests/decision`×3, `quests/get-decisions`×3, `guilds/{id}/members/@me`×2) и `x-science-test`/`X-Failed-Requests` (только на `/science`).

### 6.3 Где задаётся версия v9

- Живьём: **все 133 API-запроса** ушли на `https://discord.com/api/v9/...` (ни одного v6/v8/v10). Проверка: `jq -r 'select(.ev=="req")|.url' … | grep -o '/api/v[0-9]*' | sort | uniq -c`.
- Статически: `"https:" + window.GLOBAL_ENV.API_ENDPOINT + "/v" + window.GLOBAL_ENV.API_VERSION` (rest-static §2, модуль 636537).
- То есть живой снимок подтверждает `API_VERSION = 9`, но сам источник константы (`GLOBAL_ENV` в `web.js`) в телах этого снимка отсутствует — `web.js` в захваченных `body` нет (проверено: 0 тел с `web.` в URL).

### 6.4 Чего в auth-потоке нет

- Нет `POST /auth/login`, `/auth/mfa/*`, `/auth/register`, `/auth/logout`, `GET /auth/sessions` — сессия уже была; токен читается из `TokenStore` (localStorage, rest-static §4). Ни один из этих маршрутов живьём не подтверждён.
- Нет обмена на `POST /auth/handoff/exchange`, нет `GET /oauth2/*`.
- Нет `GET /api/v9/gateway` — URL шлюза берётся из `GLOBAL_ENV.GATEWAY_ENDPOINT` (см. U-03); живьём виден только сам сокет: `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` (в снимке 6 `wsc`, 19 `wssent`, 120 `wssent-bin`, 494 `wsrecv`).

### 6.5 Второй авторизационный путь: голосовой шлюз (наблюдается живьём)

Основной шлюз отдаёт кадры бинарно (zstd+ETF), поэтому `IDENTIFY`/`RESUME` там нечитаемы. А вот голосовой сокет — **текстовый JSON**, и в нём виден отдельный вход с теми же кредами:

- Сокет: `wss://c-fra19-4da7fe5f.discord.media:2053/?v=9` (requestId `run2|95974.8158`), второй — `ready|57992.7568`.
- `op:0` (Identify, отправлен клиентом) — тело целиком:

```json
{"op":0,"d":{"server_id":"{guild_id}","channel_id":"{channel_id}","user_id":"{user_id}","session_id":"REDACTED","token":"REDACTED","max_dave_protocol_version":1,"video":true,"streams":[{"type":"video","rid":"100","quality":100}]}}
```

- `op:1` (Select Protocol): `{protocol:"udp", data:{address, port, mode:"aead_aes256_gcm_rtpsize"}, codecs:[{name:"opus",type:"audio",payload_type:120},{name:"AV1",type:"video"}]}` — клиент отдаёт выбранный режим шифрования и кодеки.
- `op:2` (Ready, пришёл от сервера): `{streams:[{ssrc,rtx_ssrc,rid:"100"}], ssrc, port, modes:["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"]}`.
- `op:4` (Session Description): `{video_codec:"AV1", secure_frames_version:1, secret_key:"REDACTED", mode:"aead_aes256_gcm_rtpsize", media_session_id:"REDACTED", dave_protocol_version:1, audio_codec:"opus"}`.
- `op:8` (Hello): `{v:8, heartbeat_interval:13750}` — **протокол голосового шлюза v8, тогда как REST — v9**; интервал heartbeat 13 750 мс против типового 41 250 мс у основного шлюза.
- `op:16` (версии): `{voice:"0.22.1+denbeigh.remove-voice-curator-74a3bd0c5", rtc_worker:"1.6.98"}`.
- `op:3` (heartbeat, seq_ack) и `op:6` (ack) — по 7 раз каждый; `op:15` `{any:100}` (media sink wants).

Итог: REST-вызовов `/voice/*` действительно нет (U-09), но **второй путь авторизации** (voice) подтверждён живьём: `token` + `session_id` уходят в `op:0` голосового шлюза. Регионы RTC и UDP-адрес сервера в отчёте не раскрываются (это сетевые координаты пользователя).

## 7. Шум (не API) — что отфильтровано

Отфильтровано **6521** запросов (98.0 % всех `req`). В основную таблицу они не попали.

| Группа | Запросов | Домены | Типы |
|---|---|---|---|
| `static-assets` | 6025 | discord.com×6025 | Script×4264, Fetch×5, Stylesheet×1619, Font×45, Image×78, Media×11, XHR×3 |
| `cdn-media` | 334 | cdn.discordapp.com×308, media.discordapp.net×4, images-ext-1.discordapp.net×22 | Image×314, Other×6, XHR×9, Fetch×5 |
| `telemetry-science` | 88 | discord.com×88 | XHR×88 |
| `data-url` | 40 | (inline-data)×40 | Image×40 |
| `third-party` | 25 | latency.discord.media×3, scope×18, i.scdn.co×2, start×2 | XHR×3, Fetch×20, Image×2 |
| `cloudflare-challenge` | 4 | discord.com×4 | Script×2, XHR×1, Other×1 |
| `discord-other-subdomain` | 3 | status.discord.com×3 | XHR×3 |
| `web-navigation` | 2 | discord.com×2 | Document×2 |

Разбор по подгруппам:

- **`static-assets` (6025)** — бандлы клиента: `Script` 4264, `Stylesheet` 1619, `Image` 78, `Font` 45, `Media` 11, `XHR` 3, `Fetch` 5. Это прогрев/перезагрузка `web.js` + лениво подгружаемые чанки (`/assets/<num>.<hash>.js`, `*.module.wasm`).
- **`cdn-media` (334)** — `cdn.discordapp.com` 308 (`/media/v1/collectibles-shop/{id}/static` ×44, `/avatars/{id}/<hash>.webp`, `/icons/{id}/…`, `/clan-badges/{id}/…`, `/detectables/games-v1.json` ×6, `non-games-v1.json` ×6), `images-ext-1.discordapp.net` 22, `media.discordapp.net` 4.
- **`telemetry-science` (88)** — `POST /api/v9/science`, все XHR, все с Authorization. Именно сюда уходит аналитика (`libdiscore_loaded`, `app_web_perf_startup_metrics`, `fetch_user_offer_started`, `client_telemetry`). `/api/v9/beaker` — 1 запрос той же схемы.
- **`data-url` (40)** — `data:image/png;base64,…` (инлайн PNG, не сеть).
- **`third-party` (25)** — `sentry-ipc://scope/sentry_key` ×18 и `sentry-ipc://start/sentry_key` ×2 (IPC-канал Electron→main для Sentry, не HTTP), `latency.discord.media/rtc?v=2` ×3 (RTC-латентность), `i.scdn.co` ×2 (картинки Spotify).
- **`cloudflare-challenge` (4)** — `/cdn-cgi/challenge-platform/...` (jsd-скрипты Cloudflare; это единственный след анти-бот защиты в снимке).
- **`discord-other-subdomain` (3)** — `status.discord.com/api/v2/scheduled-maintenances/upcoming.json` (v2, не v9 — намеренно не в основной таблице).
- **`web-navigation` (2)** — переходы `GET /channels/{guild}` (Document), а не XHR.

Внутри шума по объёму:

| Путь | Запросов |
|---|---|
| `/api/v9/science` | 88 |
| `/media/v1/collectibles-shop/{id}/static` | 44 |
| `(data: URL, инлайн PNG — не сеть)` | 40 |
| `/sentry_key` | 20 |
| `/avatars/{id}/e234430751874dcfca60cdd3a53a2673.webp` | 14 |
| `/avatars/{id}/a_49f72f47b64c72f2f130cb9fb2808cc1.webp` | 6 |
| `/detectables/games-v1.json` | 6 |
| `/detectables/non-games-v1.json` | 6 |
| `/media/v1/collectibles-shop/{id}/animated` | 6 |
| `/assets/web.4d4b2f75fdea5470.js` | 5 |
| `/icons/{id}/81805511d8bc5fb4680727e99f041773.webp` | 5 |
| `/icons/{id}/b4b4c3850182f7e0dbbaa116df2ed3f2.webp` | 5 |

## 8. Сверка со статикой и пробелы

### 8.1 Сверка

- Живьём: **47** уникальных маршрутов / **133** вызовов.
- Статика: 648 REST-записей, из них с определённым методом 626; у 10 записей путь содержит артефакт извлечения (`${null!=e?` и т.п.).

| Итог сверки | Маршрутов |
|---|---|
| Подтверждено статикой (путь + метод) | **44** |
| Путь есть, метод в статике не определён (`null`) — живой вызов уточняет | 2 |
| Конфликт метода (статика и живое расходятся) | 0 |
| Нет в статике (новое знание от живого захвата) | 1 |

#### Метод, не определённый в статике, но видимый живьём

| Метод (live) | Путь | Вызовов | Ключ статики |
|---|---|---|---|
| **GET** | `/guilds/{id}/application-command-index` | 2 | APPLICATION_COMMAND_INDEX_GUILD |
| **PUT** | `/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` | 1 | REACTION |

#### Маршруты, которых нет в статической карте

| Метод | Путь | Вызовов | Комментарий |
|---|---|---|---|
| POST | `/beaker` | 1 | телеметрия второго канала (та же схема `{token, events}`, что у `/science`), в статике отсутствует |

#### Маршруты, найденные в статике только после нормализации пути

| Метод (live) | Путь (live) | Путь в статике (как есть) | requestId |
|---|---|---|---|
| GET | `/content-inventory/users/@me` | `/content-inventory/users/@me${null!=e?` | run2|95974.3434 |

Оговорка по сопоставлению: статический плейсхолдер `{arg}` сопоставлялся с value-подобным сегментом (`{id}`, `@me`, эмодзи в %-кодировке). Маршруты, где `@me` совпал с `{arg}` (например `/users/@me`), отдельно проверены по точной записи статики (`GET|PATCH /users/@me [ME]`).

### 8.2 Важные маршруты, которых в захвате НЕ было

| Маршрут | Почему ожидался | Статус в захвате |
|---|---|---|
| `/auth/login` | основной вход (POST) | ❌ не подтверждено |
| `/auth/register` | регистрация (POST) | ❌ не подтверждено |
| `/auth/logout` | выход из сессии (POST) | ❌ не подтверждено |
| `/auth/mfa/*` | второй фактор (POST) | ❌ не подтверждено |
| `/auth/sessions` | список сессий (GET), logout others | ❌ не подтверждено |
| `/auth/location-metadata` | регион/согласия/captcha при входе (GET) | ❌ не подтверждено |
| `/experiments` | флаги фич; при наличии токена — в READY (GET) | ❌ не подтверждено |
| `/apex/experiments` | флаги поверхности web-приложения (GET) | ❌ не подтверждено |
| `/gateway` | REST-получение URL шлюза (GET) | ❌ не подтверждено |
| `/users/@me/settings` | настройки пользователя (GET/PATCH) | ❌ не подтверждено |
| `/oauth2/authorize` | OAuth2-диалог | ❌ не подтверждено |
| `/voice/regions` | голосовые регионы | ❌ не подтверждено |
| `/read-states/ack-bulk` | пакетная отметка о прочтении | ❌ не подтверждено |
| `/track` | телеметрия-трекинг (POST) | ❌ не подтверждено |

Отдельно: `POST /api/v9/science` наблюдался **88 раз** (все с Authorization), но отнесён к телеметрии и в таблицу API не включён — это осознанное разграничение «API vs телеметрия» (§7). `POST /api/v9/track` не наблюдался ни разу.

### 8.3 Пробелы (unknowns)

| # | Что | Детали | Статус |
|---|---|---|---|
| U-01 | Тела ответов REST | В обоих захватах ни одного `body` события для /api/*. Захвачено 200 тел, все — /assets/*.js (md5-проверяемо), лимит 20 000 символов. | не подтверждено |
| U-02 | /api/v9/auth/* (вход/регистрация/MFA/сброс пароля) | Ни одного запроса к /auth/* ни в одном захвате: сессия уже была активна (токен из TokenStore). Все схемы — только из статики (rest-static §4). | не подтверждено |
| U-03 | GET /api/v9/gateway (REST-получение URL шлюза) | Отсутствует. Клиент подключается к wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream напрямую. | не подтверждено (в захвате не вызывался) |
| U-04 | GET /experiments и /apex/experiments | В захвате отсутствуют — согласуется со статикой: ExperimentsStore пропускает запрос при наличии токена (эксперименты приходят в READY). | не подтверждено (условная ветка) |
| U-05 | Cookie __dcfduid/__sdcfduid/__cfruid | Ни один `req` не содержит заголовка Cookie (CDP не отдаёт cookie-заголовки в requestWillBeSent). Тип/время жизни cookie по живому трафику не проверены. | не подтверждено |
| U-06 | Формат значения Authorization (Bearer vs сырой токен) | Значения отредактированы санитайзером (Authorization: REDACTED) в обоих файлах, включая .orig. Заявление «сырой токен без Bearer» остаётся статическим (rest-static §4). | не подтверждено |
| U-07 | Различие Authorization / authorization | 3 запроса GET /users/@me пришли с ключом в нижнем регистре. Нельзя отличить реальную разницу в коде клиента от нормализации заголовков CDP/HTTP2. | не подтверждено (артефакт?) |
| U-08 | X-Fingerprint / X-Installation-ID / X-Routing-Key / x-client-trace-id | Заявлены статикой в prepareRequest, но в живых API-запросах отсутствуют (есть только X-Super-Properties, X-Discord-Locale, X-Discord-Timezone, X-Debug-Options, Accept-Language). | расхождение статика↔живое |
| U-09 | Голосовой RTC / voice REST | REST-вызовов /voice/* в захвате нет: голос идёт через отдельный текстовый WS (v=8) — см. §6.5, где op:0 Identify с token+session_id подтверждён живьём. Голосовые кадры (UDP/RTP) в CDP не видны. | частично подтверждено (WS, не REST) |
| U-10 | capture-run2.jsonl дозаписывается живой сессией | Файл растёт во время анализа (mtime 11:51:37, размер 4 105 738 → 4 136 152 байт, req 4390 → 4395). Анализ привязан к снимку d02-snapshot/ (md5 в provenance). | ограничение методики |

## 9. Приложение: как воспроизвести

```bash
# 1. заморозить снимок (файл живого захвата дозаписывается)
mkdir -p /home/sanya/re-deep/d02-snapshot && cd /home/sanya/re-deep
cp capture-run2.jsonl d02-snapshot/capture-run2.snap.jsonl
cp capture-ready.jsonl d02-snapshot/capture-ready.snap.jsonl
md5sum d02-snapshot/*.jsonl   # сверить с rest-d02.json → provenance

# 2. разбор + отчёт
node rest-d02.mjs && node rest-d02-report.mjs
# 3. независимая проверка ключевых чисел (jq/awk/join, вне кода разбора)
bash rest-d02-verify.sh | tee rest-d02.verify.txt
```

Ожидаемый итог проверки (из `rest-d02.verify.txt`):

```
req=6654 (4395+2259), уникальных requestId=6651 (3 повтора из-за redirect/retry)
API v9: 133 запроса, 47 уникальных маршрутов; /api/v2: 3 (status.discord.com)
Authorization: 218 Authorization + 3 authorization = 221
X-Super-Properties / X-Discord-Timezone / X-Discord-Locale / X-Debug-Options / Accept-Language = 221 каждый
science: 88 запросов, все с Authorization, все 204
body: 200, не-ассет 0, min 141 / median 520 / max 20001 / sum 367961 (9 упёрлись в потолок 20 000)
postData: 120, обрезка на 4000 (12 упёрлись); API (не science) с телом: 13
шум: assets=6025 cloudflare=4 data=40 web-nav=2 cdn+thirdparty=362
коды API: 200×113, 204×8, 304×3, 404×8, 429×1
```

Файлы артефактов D-02:

| Файл | Что внутри |
|---|---|
| `rest-d02.md` | этот отчёт |
| `rest-d02.json` | машиночитаемый результат: `endpoints[]`, `queryParams`, `auth`, `noise`, `crossRef`, `unknowns` |
| `rest-d02.stats.json` | плоские счётчики и списки строк-эндпоинтов |
| `rest-d02.crossref.json` | детальная сверка живых маршрутов со статикой (по каждому маршруту) |
| `rest-d02.verify.txt` | вывод независимой проверки (`rest-d02-verify.sh`) |
| `rest-d02.mjs`, `rest-d02-report.mjs`, `rest-d02-verify.sh` | код разбора, генератор отчёта, верификатор |
| `d02-snapshot/` | замороженный снимок входных данных + md5 |
