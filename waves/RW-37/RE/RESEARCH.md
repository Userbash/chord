# RW-37 / W37a — RE-контракт раскладки и пагинации Discord

- Дата: 2026-10-23. Воркер: W37a (REA). Волна: RW-37, карточка-основание: `TZ.md` §5 (P3-01 + оконная модель).
- Источники: **только чтение** `/home/sanya/re-deep` (CDP-захваты `ui/`, `ui2/`, `ui3/`, `ui5/`, `ui6/`, `ui8/`,
  `frontend-d03.*`, `gateway-schema.*`, `gateway-protocol.*`, `rest-d02.*`, `rest-static.*`,
  распакованные бандлы `decoded/`). Сеть не использовалась, Discord не запускался, golden не изменялся.
- Сессия захвата: Discord web/desktop сборка 1.0.162 (Chromium 148 / Electron 42), тема dark, язык ru,
  окно 2048×1106 CSS при `devicePixelRatio=1.25` (скриншоты 2560×1383 px).
- Все числа ниже воспроизводятся скриптом `extract.py` (см. §9); машиночитаемая версия — `layout.json`.

Статусы: **measured** — прямое измерение в артефакте; **css/source-derived** — посчитано по правилу из
бандла/токенов (с указанием правила); **assumption** — вывод по аналогии, в дампе прямого подтверждения нет.

---

## 1. Вьюпорт

| Что | Значение | Источник | Как извлечено | Статус |
|---|---|---|---|---|
| Вьюпорт (CSS px) | **2048 × 1106** | `ui/log.json` → `{name:"start", size:{w:2048,h:1106}, dpr:1.25}` | `Runtime.evaluate` `innerWidth/innerHeight` + `devicePixelRatio` (`ui-trace.mjs:111`) | measured |
| DPR | **1.25** | там же | | measured |
| Скриншоты | **2560 × 1383 px** | IHDR `ui/00-app.png`, `ui6/02-channel.png` | PIL: 2560/1.25 = 2048, 1383/1.25 = 1106.4 | measured |
| Тема | dark, ru | `docs/re/frontend-d03.md` §«Дата/Среда», `ui/tokens.json` | — | measured |

Вьюпорт один и тот же во всех сессиях `ui*`: все полноэкранные снимки 2560×1383 px
(кропы/клипы — `ui8/22-members.jpg` 320×1281, `ui8/20a-*.jpg` 2176×1175 — из них берутся только
относительные размеры), поэтому числа раскладки сопоставимы между `ui/`, `ui6/` и `ui8/`.

---

## 2. Горизонтальная раскладка (5 колонок)

Замер по `ui/00-app.png` (скан строк по цвету панелей) + `getBoundingClientRect` из `ui8/22-members.json`.
Цвета панелей в этом срезе: sidebar (rail + tree) `rgb(18,18,20)`, chat `rgb(26,26,30)`, composer `rgb(34,35,39)`.

```
x=0        64        375                              1792        2048
|--rail----|--tree---|-------------chat--------------|--members--|
   64 px      311 px              1417 px                256 px
```

| Элемент | Значение | Источник | Как извлечено | Статус |
|---|---|---|---|---|
| Guild rail | **64 px** | `ui/tokens.json`: `--custom-guild-list-width: calc(40px + 12px*2)`, `--custom-guild-list-padding: 12px`, `--guildbar-avatar-size: 40px`; `ui2/log.json` (`guild-rect`: `w=40`, центр `x=32` ⇒ left = 12) | формула токена + rect иконки гильдии | measured |
| Channel tree | **311 px** (в этой сессии) | `ui/00-app.png` / `ui6/02-channel.png`: граница панелей на `x=375.2`; rail 64 ⇒ tree = 311.2 | скан 6 строк: первый x, где ≥60 px подряд chat-цвета после ≥35 px sidebar-цвета | measured |
| — проверка формулой | 375 − 64 − 1 = **310** (+1 px border = 311) | `decoded/3eefa7dae1e2c234_0`: `.sidebarList__5e434{width:calc(var(--custom-guild-sidebar-width) - var(--custom-guild-list-width) - 1px)}` | подстановка измеренной суммы 375 | css-derived |
| Channel tree, дефолт сборки | **203 px** | `decoded/…`: `:root{--custom-guild-sidebar-width:268px}`; `ui/tokens.json` (live) = `268px` | 268 − 64 − 1 | css-derived |
| Чат | **1417 px** | `ui8/22-members.json` `rect.x0=1792` − 375 | вычитание | measured |
| Member list | **256 × 1025 px** @ `(1792, 82)` | `ui8/22-members.json` → `rect:{x0:1792,y0:82,w:256,h:1025}`; кроп `ui8/22-members.jpg` = 320×1281 px = 256×1024.8 CSS | `getBoundingClientRect()` (`ui-members6.mjs`); сверка с кропом | measured |
| Member list, токен | **256 px** live (в бандле также 264 и 268) | `ui/tokens.json`: `--custom-member-list-width: 256px`; `decoded/…`: в разных слоях `256px/264px/268px` | прямое чтение | measured |
| Header канала | **49 px** (полоса y 32…81) | `ui/tokens.json`: `--custom-channel-header-height: 49px`; `ui/00-app.png` столбец x=1200 | смена цвета по столбцу + токен | measured |
| Titlebar | **32 px** (y 0…32) | `ui/tokens.json`: `--custom-app-top-bar-height: 32px`; тот же столбец | то же | measured |
| Composer (textarea) | **56 px**, ширина ≈ **1401 px**, верх `y=1044` | `--custom-channel-textarea-text-area-height: 56px`; PNG: бокс `(34,35,39)` x 381.6…1784.8, y 1044…1100 | цветовой скан + токен | measured |
| Отступ composer от краёв чата | ≈ 6–8 px с каждой стороны | PNG: 381.6 − 375 = 6.6; 1792 − 1784.8 = 7.2 | вычитание | measured (точное правило padding — assumption) |

**Важно про ширину сайдбара.** В захваченной сессии суммарная ширина сайдбара (rail + tree) = **375 px**,
а не дефолтные 268 px из `:root`. Три независимых сигнала сходятся на 375:

1. измеренная граница панелей — `375.2` (два разных скриншота: `ui/00-app.png`, `ui6/02-channel.png`);
2. CSS-формула `.sidebarList__5e434` даёт ровно 310 при подстановке 375 (совпадает с измеренными 311 с border);
3. в бандле есть JS-фолбэк того же значения:
   `function K(){let e=document.body.style.getPropertyValue("--custom-guild-sidebar-width").slice(0,-2);return parseInt(""!==e?e:"375")}`
   (`decoded/3eefa7dae1e2c234_0`).

Причина (persisted user setting на `document.body` vs. новый дефолт сборки) в артефактах **не аттестована** —
`assumption`. Для нашего клиента безопасно брать **дефолт 268 (tree 203)** и разрешать ресайз до 375+;
контракт `layout.json` содержит оба значения (`channel_tree_w` = измеренное, `channel_tree_w_default` = токен).

---

## 3. Вертикальные размеры строк

### 3.1 Сообщение (cozy / compact)

Источник правила — CSS из распакованного бандла `decoded/3eefa7dae1e2c234_0`:

```css
.wrapper_c19a55{position:relative;min-height:1.375rem;padding-inline-end:var(--custom-message-margin-horizontal)}
.cozy_c19a55.wrapper_c19a55{padding-top:var(--custom-message-spacing-vertical-container-cozy);
                            padding-bottom:var(--custom-message-spacing-vertical-container-cozy);
                            padding-inline-start:var(--custom-message-margin-left-content-cozy)}
.compact_c19a55.wrapper_c19a55{padding-top:var(--custom-message-padding-vertical-container-compact);
                               padding-bottom:var(--custom-message-padding-vertical-container-compact);
                               padding-inline-start:var(--custom-message-margin-compact-indent)}
.header_c19a55{display:block;line-height:1.375rem;min-height:1.375rem}
.avatar_c19a55{height:var(--chat-avatar-size);width:var(--chat-avatar-size);
               inset-inline-start:var(--custom-message-margin-horizontal);
               margin-top:calc(4px - var(--custom-message-spacing-vertical-container-cozy))}
```

Токены (`ui/tokens.json`, live): `--chat-markup-line-height: 1.375rem` (**22 px**),
`--chat-avatar-size: 40px`, `--custom-message-margin-horizontal: 12px`,
`--custom-message-margin-left-content-cozy: calc(40px+12px+12px)` (**64 px**),
`--custom-message-margin-compact-indent: 5rem` (**80 px**),
`--custom-message-spacing-vertical-container-cozy: 0.125rem` (**2 px**),
`--custom-message-padding-vertical-container-compact: 0.125rem` (**2 px**).

| Строка | Высота | Как посчитано | Статус |
|---|---|---|---|
| Сообщение cozy, продолжение группы (1 строка текста) | **26 px** | 22 (line-height) + 2 + 2 (padding) | css-derived |
| Сообщение cozy, начало группы (header + 1 строка) | **48 px** | 22 (header) + 22 (контент) + 4 | css-derived |
| Сообщение cozy, каждая доп. строка | +22 px | line-height 22 | css-derived |
| Сообщение compact (timestamp + автор + текст в одну строку) | **26 px** | 22 + 2 + 2 | css-derived |
| Контент сообщения (проверка) | **66 px = 3 × 22** | `ui/log.json` → `msg-rect` (`[id^="message-content-"]`, `h=66`) | measured |
| Аватар сообщения | **40 × 40**, left = chat_left + 12 | PNG: аватар y 609.6…648.8 CSS (40 px), x 387…427 | measured |
| Текст сообщения (cozy) | начинается на **chat_left + 64** | PNG: минимальный x текста = **439.2** = 375.2 + 64.0 | measured |

Следствие для виртуализации: строка сообщения **не фиксированной высоты** (26 / 48 / +22 за строку
переноса). Текущий спайк (`rust/gui-spike/ui/style.css:130`, `.row{height:44px}`) использует фиксированные
44 px — это допустимая аппроксимация только для «среднего» сообщения; контракт требует хранить высоту
на строку (или оценивать по числу строк текста) — см. §7.

### 3.2 Участник и канал

| Строка | Высота | Источник / как | Статус |
|---|---|---|---|
| Участник (member list) | **42 px** | CSS: `.memberInner__5d473{padding:4px 0}` + `.avatar__91a9d{height:32px;width:32px;margin-inline-end:12px}` + `.container__91a9d{padding:1px 0}` ⇒ 32+8+2 = 42. Замер по `ui8/22-members.jpg`: медиана расстояний между верхами аватаров ≈ **42.2** (без групп-заголовков) | measured + css |
| Заголовок группы участников | ≈ 16 + 4 + текст (~22) | `.membersGroup_c8ffbb{padding-block:16px 4px}` (`--space-lg`/`--space-xxs`) | css-derived |
| Канал (text/voice row) | **28 px**, ширина **296 px** | `ui6/log.json` → `text-channel`: `h=28`, `w=296` (`a.link__2ea32`). CSS: `.link__2ea32{padding-top:4px;padding-bottom:4px}` + `.name__2ea32{line-height:var(--channels-name-line-height)}` = 20 px ⇒ 28 | measured |
| Отступ строки канала | left ≈ tree_left + 6, right ≈ tree_right − 9 | `ui6` rect: центр `x=218`, `w=296` ⇒ 70…366 при tree 64…375 | measured (точные padding — assumption) |

---

## 4. Пагинация истории сообщений

### 4.1 Что реально ходило в сеть (CDP Network)

Из `capture-ready.jsonl`, `capture-run2.jsonl`, `capture-cold.jsonl` (каждый URL учтён дважды: request+response):

| Сценарий | URL-шаблон | Наблюдено | Статус |
|---|---|---|---|
| Первичная загрузка канала | `GET /api/v9/channels/{id}/messages?limit=12` | **10 различных каналов** в трёх захватах (в одном снимке `run2` — 8, `docs/re/rest-d02.md` §3) | measured |
| «Загрузить ещё» (скролл вверх) | `GET …/messages?before=<snowflake>&limit=24` | **3 различных URL** (2 канала), `limit=24` | measured |
| Прыжок к сообщению | `GET …/messages?limit=30&around=<snowflake>` | **1 URL** | measured |
| Закреплённые | `GET …/messages/pins?limit=25` | 1 URL (2 события req+resp) | measured |
| Поиск по гильдии | `GET /guilds/{id}/messages/search?…&offset=0` | `offset` в query | measured (`docs/re/rest-d02.md` §3) |

`docs/re/rest-d02.md` §3 подтверждает тот же набор: `?limit=12` ×8, `?before=<snowflake>&limit=24` ×2,
`?limit=30&around=<snowflake>` ×1, и формулирует правило «`limit` + один из курсоров `before`/`after`/`around`».

### 4.2 Формулы клиента (из бандла)

| Правило | Значение | Источник | Статус |
|---|---|---|---|
| Размер чанка «загрузить ещё» | `limit = min(30, 2 × config("scrollManager.loadMore"))` | `decoded/3eefa7dae1e2c234_0`: `fetchMessages({channelId, before, after, limit:Math.min(eh.EMb, 2*(0,lZ.h)("scrollManager.loadMore")), truncate:true})` | source |
| Потолок чанка | **30** (`EMb` = `nF` = 30) | там же: модуль 652215 `let nx=100,nk=200,nF=30,nB=30,nV=25,nH=120,nj=60`; export map `EMb:()=>nF` | source |
| Прыжок (`around`) | **30** (`d7Q` = `nB` = 30) | там же: `fetchMessages({channelId, limit:D.d7Q, jump:{...}})`; экспорт `d7Q:()=>nB` | source + measured (совпало с `limit=30&around`) |
| Первичная загрузка | remote-config `MessageManager.initialFetch` (дефолт в коде отсутствует) | `fetchMessages({channelId, limit:(0,R.h)("MessageManager.initialFetch"), ...})` | source; значение **12** — measured |
| Preload ветки | **100** | `fetchMessages({channelId:r,limit:100})` (thread `load`) | source |
| Максимум `GET /messages` | **100** | следствие предыдущего (клиент сам просит 100) + документированный лимит Discord; 4xx-пробы `limit>100` в дампе **нет** | source + `assumption` (серверный потолок прямой пробой не подтверждён) |
| Сообщений в памяти на канал | **≤ 120**, при превышении обрезается до **60** | `decoded/…`: `truncate(e,t){return this.length<=E.UNo?this:…truncateBottom(E.rLJ)}`; `UNo`=`nH`=**120**, `rLJ`=`nj`=**60** | source |
| Обрезка при догрузке | `truncate:true` в `loadMore` | тот же вызов | source |

**Ответ на вопрос «сколько сообщений реально в памяти клиента на старте»:** при открытии канала — ровно
**12** (один запрос), после двух подгрузок — до **60** (`12 + 24 + 24`), жёсткий потолок коллекции канала —
**120**, после чего она срезается до 60. Для сравнения: GUI-спайк RW-36 держит 100 000 строк в `PageStore`
на один чат — это на три порядка больше реального клиента.

### 4.3 Размеры страниц для нашей оконной модели

Ориентиры из реального клиента (все — measured/source выше): страница 12–24, потолок чанка 30, потолок
истории 120, окно участников 100. Отсюда рекомендация для P3-01 (§7): `pageSize = 100`, `maxPages = 2`
(200 строк на список) — с запасом покрывает и историю (120), и окно участников (100), и остаётся в
бюджете памяти ADR-1.

---

## 5. READY и ленивая догрузка

### 5.1 Сколько объектов приходит в READY

Источник: `gateway-schema.md` §3 (разбор одного READY, seq=1; JSON 584 149 Б, распаковано 577 124 Б, 39 полей
+ `_trace`) и `gateway-schema.json` (`events[t=READY]`).

| Коллекция | Кол-во | Примечание |
|---|---|---|
| `guilds[]` | **7** | у каждой `data_mode:"full"`, `lazy:true` (7/7), `large:false` (в другом снимке `large:true`) |
| `guilds[].channels[]` | **272** | все каналы всех 7 гильдий сразу |
| `guilds[].roles[]` | **132** | |
| `users[]` | **146** | только «известные» пользователи (ЛС, пресенсы), **не** участники гильдий |
| `private_channels[]` | **17** | ЛС/группы |
| `merged_members[][]` | **7** (по 1 на гильдию) | демонстрационно: полный список участников не приходит |
| `read_state.entries[]` | **157** | |
| `user_guild_settings.entries[]` | **58** | |
| `…channel_overrides[]` | **1 328** | |
| `relationships[]` | **60** | |
| `sessions[]` | **1** | |

Ключевое: **в READY нет `presences`, а внутри `guilds[]` нет `members`** (`threads:null`, `members` ушли в
`merged_members` по 1 записи на гильдию) — `gateway-schema.md` §3 «Чего в READY нет». Т.е. полный список
участников **никогда** не материализуется на старте.

Второй снимок (`gateway-protocol.md` §6, гильдия ITSKIDON `1014537851418980372`): `member_count` **24 229**
(в разборе схемы — 24 227), каналов **83** (text 47, voice 20, category 12, forum 4), ролей 34, эмодзи 46,
`voice_states:0`, `members:0`, `sizeBytes` 65 489.

### 5.2 Как клиент догружает участников

| Механизм | Наблюдение | Источник | Статус |
|---|---|---|---|
| Окно списка участников | **100 строк** | `gateway-schema.json`: `GUILD_MEMBER_LIST_UPDATE.ops[0] = {op:"SYNC", range:"0x0063"}`; `range` — 2 байта `[0, 99]`; в SYNC **100** элементов items (2 группы + 98 участников) | measured |
| Инкрементальные правки окна | 79 событий: SYNC×1, UPDATE×57, DELETE×21, INSERT×21; `index` 1…99 | `gateway-schema.md` §4.1 | measured |
| Счётчики | `member_count` 24 227…24 228, `online_count` 657…679; группы: `online` + роль | там же | measured |
| Второй снимок | `GUILD_MEMBER_LIST_UPDATE` — **1 048** событий за сессию («ленивый список ~24 тыс.») | `gateway-protocol.md` §8 | measured |
| `GUILD_MEMBERS_CHUNK` | 1 событие, `chunk_index=0`, `chunk_count=1`, **1 участник**, 666 Б | `gateway-schema.json`; в другом снимке — 11 событий / 32 участника (`gateway-protocol.md`) | measured |
| Запрос окна (op 14 `GUILD_SUBSCRIPTIONS` с `ranges:[[0,99]]`) | **не захвачен**: исходящие WS-кадры в дампе только хешированы (`wssent-bin`: `len`+`sha256`, 340 кадров) | `capture-run2.jsonl` | `assumption` (форма payload — стандартный op14; подтверждён только ответ сервера) |
| op 8 `REQUEST_GUILD_MEMBERS` | в дампе не наблюдался; `GUILD_MEMBERS_CHUNK` пришёл на 1 участника (запрос точечный) | — | `assumption` |

Практический вывод: список участников — это **окно 100 строк**, которое сервер сам синхронизирует
(`SYNC`/`INSERT`/`UPDATE`/`DELETE` по индексам), а клиент лишь подписывается на диапазон. Ровно та же
модель, что нужна нашему `PageStore` (`ensureRange` + `getRow(index)`), только страница = 100.

> Про `op37` из ТЗ: в дампе такого op нет — каталог ограничен `op0` (dispatch), `op10` (HELLO), `op11` (ACK),
> а клиентские кадры сохранены только как `len`+`sha256` (`gateway-schema.md` §2, `gateway-protocol.md` §8).
> Механизм догрузки участников — **op14** (подписка на диапазоны) + **op8** (запрос участников, ответ —
> `GUILD_MEMBERS_CHUNK`); ленивая синхронизация списка идёт входящими `GUILD_MEMBER_LIST_UPDATE`. В контракте
> зафиксирован op14; `op37` считаем вероятной опечаткой в ТЗ (прямого подтверждения ни за, ни против в дампе нет).

---

## 6. Сводка: контракт одной таблицей

| Параметр | Значение | Статус |
|---|---|---|
| viewport | 2048 × 1106 CSS @ dpr 1.25 | measured |
| rail | 64 px (иконка 40 + padding 12×2) | measured |
| channel tree | 311 px (сессия) / 203 px (дефолт токена) | measured / css-derived |
| member list | 256 × 1025 px | measured |
| chat | 1417 px | measured |
| header / titlebar | 49 / 32 px | measured |
| composer | 56 px высота, ~1401 px ширина | measured |
| сообщение cozy (продолжение / начало группы) | 26 / 48 px (+22 за строку) | css-derived (+ measured 66 = 3×22) |
| сообщение compact | 26 px | css-derived |
| участник | 42 px (аватар 32) | measured + css |
| канал | 28 px (ширина строки 296) | measured |
| indent текста сообщения (cozy) | 64 px от левого края чата | measured (439.2 = 375.2 + 64) |
| `GET /messages` max limit | 100 | source + assumption |
| первичная страница | 12 (config) | measured |
| страница «ещё» | min(30, 2×config) → 24 | source + measured |
| прыжок (`around`) | 30 | source + measured |
| pins | 25 | measured |
| история в памяти на канал | ≤ 120, трим до 60 | source |
| окно участников | 100 (SYNC range [0,99]) | measured |
| READY: guilds/channels/roles/users | 7 / 272 / 132 / 146 | measured |

---

## 7. Что это значит для P3-01 / оконной модели

1. **pageSize = 100** (совпадает с окном участников и покрывает потолок истории 120 за 2 страницы),
   `maxPages = 2` ⇒ 200 строк на список. Для чата 100k и участников 24 229 это даёт ≤ 200 материализованных
   объектов вместо 124 229 (ADR-1) — при сохранении `rendered ≥ 10`.
2. **Высота строки не константа.** Cozy: 26 px для продолжения группы и 48 px для начала, +22 px на каждую
   строку переноса. Фиксированные 44 px в спайке (`style.css:130`) дают дрейф скролла; для точной
   виртуализации нужна либо карта высот по индексу, либо оценка `26 + 22*lines` (+22 если group-start).
3. **Список участников — окно 100 с серверной синхронизацией по индексам.** Индексы в
   `GUILD_MEMBER_LIST_UPDATE.ops[].index` абсолютные (0…99 в окне) — `getRow(index)`/`ensureRange` ложатся
   на эту модель один-в-один; LRU-эвикция не должна выкидывать текущий диапазон (совпадает с инвариантом TZ §4).
4. **Ширины для вёрстки:** rail 64, tree 268 (дефолт) / до 375 (ресайз), member list 256, header 49,
   composer 56. Текущий спайк: rail 72, строка 44, участник 40, аватар 28 — расходится с реальным клиентом
   (rail +8; строка 44 против 26/48, т.е. −18/+4; участник −2; аватар −12). Правки — в карточке P3-01, не в этой волне.
5. **Память:** реальный клиент ограничивает историю канала 120 сообщениями и окно участников 100 строками —
   наш бюджет `pageSize*maxPages` можно обосновывать этим, а не только лимитом 300 МБ.

---

## 8. Спорное / assumption (полный список)

| # | Утверждение | Почему assumption |
|---|---|---|
| A1 | `GET /messages` серверный максимум = 100 | клиент сам просит `limit=100` (preload ветки), но пробы `limit=101` в дампе нет |
| A2 | Форма op14-подписки `{guild_id, channels, ranges:[[0,99]]}` | исходящие WS-кадры в CDP только хешированы (`wssent-bin`), payload не сохранён |
| A3 | op8 `REQUEST_GUILD_MEMBERS` использовался для chunk на 1 участника | события chunk есть, запроса нет |
| A4 | Причина sidebar = 375 (persisted user setting vs новый дефолт) | `body.style` в артефактах не сохранён; сходятся только измерения + JS-фолбэк 375 |
| A5 | Точные горизонтальные padding'и строки канала (6/9) и composer (6–8) | вычислены из центров/границ rect'ов с округлением; CSS-правило не выделено |
| A6 | `MessageManager.initialFetch = 12` — это значение remote-config, а не константа | дефолт конфига живёт на сервере; наблюдено 12 во всех 10 различных URL этой сессии |
| A7 | Размер страницы поиска (offset-пагинация) | наблюдён только `offset=0`; размер страницы серверный |
| A8 | `member_count` 24 227 vs 24 229 | два разных снимка (разные сессии), оба фактические |
| A9 | Высота заголовка группы участников ≈ 42 | посчитана по padding'ам токенов, отдельно не измерялась |
| A10 | `chunk` участников ограничен 100 за событие | следует из протокола Discord (op8 `limit ≤ 100`); в дампе chunk'и по 1 участнику |

---

## 9. Воспроизведение

```bash
cd /home/sanya/orca/project
python3 waves/RW-37/RE/extract.py > waves/RW-37/RE/extract.log 2>&1   # exit 0
```

Скрипт читает только `/home/sanya/re-deep` и печатает для каждого числа `value / src / how`. Полный вывод —
`extract.log`, машинночитаемая копия — `layout.json` (и `/tmp/opencode/rw37-re.json` при прогоне).

---

## 10. Артефакты-источники (что именно читалось)

| Артефакт | Что взято |
|---|---|
| `ui/log.json` | viewport 2048×1106, dpr 1.25, `msg-rect` (h=66), `guild-rect` |
| `ui/tokens.json` | 4795 CSS-переменных темы dark: все `--custom-*`, `--chat-*`, `--space-*` |
| `ui/00-app.png` (2560×1383) | границы панелей, аватар 40, текст на 439.2, header/titlebar, composer |
| `ui2/log.json` | `guild-rect` (иконка 40, left 12) |
| `ui6/log.json` | `text-channel` rect: 296×28 |
| `ui6/02-channel.png` | вторая проверка границы панелей (375.2) |
| `ui8/22-members.json` | `membersWrap` rect 1792,82,256,1025; 35 строк, 2 группы |
| `ui8/22-members.jpg` | кроп панели 256×1024.8 CSS → шаг строк участников ≈ 42 |
| `decoded/3eefa7dae1e2c234_0` | CSS сообщений/каналов/участников, константы 100/200/30/30/25/120/60, `truncate`, `loadMore`, `K()` |
| `gateway-schema.{md,json}` | READY (7/272/132/146/17/157/58/1328/60), `GUILD_MEMBER_LIST_UPDATE` SYNC range [0,99], 100 items, chunk |
| `gateway-protocol.md` | второй снимок READY (24 229, 83 канала), 1048 MLU, 11 chunk / 32 участника |
| `capture-ready.jsonl`, `capture-run2.jsonl`, `capture-cold.jsonl` | реальные URL пагинации (`limit=12/24/30`, pins 25), `wssent-bin` (только хеши) |
| `docs/re/frontend-d03.md`, `docs/re/rest-d02.md` | нормативные выдержки: пагинация, участники (735 онлайн), токены |
