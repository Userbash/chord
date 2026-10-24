# L-02-UI-SPEC — контракт UI-проекций dispatch → экраны (G2, read-only срез)

Дата: 2026-10-24. Роль: **G2 Frontend (read-only)**. PM: DeepSeek. Карточка: `L-02-UI-SPEC`.
Задача среза: зафиксировать контракт так, чтобы UI-воркер после принятия `L-02-CORE`
стартовал без ожидания и без переизобретения полей.

Статус: **проект контракта на заморозку**. Код, тесты, CSS, `index.html`, harness и Rust
в этом срезе **не менялись**. Live/окно/браузер/сеть/`cargo`/`npm test` не запускались.

Вход (прочитано):

| Документ / файл | Что взято |
|---|---|
| `CONTRACTS/dispatch.md` §3–§10 | конверт `{kind,data}`, 6 форвардимых `kind`, политика секретов, доставка/дедуп, `net_dispatch_stats`, предложение по ядру §6 |
| `waves/RW-40/PM/l02/REPORT.md` | что реально сделано G1, `CORE_DISPATCH_AVAILABLE=false`, `sourceAvailable=false`, 21 тест |
| `rust/gui-spike/src/net.rs` | `DISPATCH_EVENT`, `FORWARDED_DISPATCHES`, `DispatchPayload`, `DispatchStats` (camelCase) |
| `rust/crates/discord-core/src/state/{member_list,presence}.rs`, `dispatch.rs` | нормализованные формы `MemberListOp`/`Presence`, 21 имя типа |
| `fixtures/gateway/by-type/*.json`, `fixtures/gw-proto/facts-frozen.json` | реальные `d` для READY/CHUNK/LIST_UPDATE/PRESENCE; `MESSAGE_REACTION_ADD` = 1 символ, `REMOVE` = 0 |
| `rust/gui-spike/ui/{app.js,model.js}` + `features/{chat,members,screens,reactions,typing-indicator,dom}.js` | текущий DOM, ключи состояния, точки обновления |
| `rust/gui-spike/ui/index.html`, `components/{chat,members}.css` | канонические селекторы, ARIA, высоты/группы |
| `tests/unit/{chat-reactions,channel-switch,typing-ui,gui-spike-features}.test.mjs`, `tests/support/gui-dom-harness.mjs` | стиль оракулов, ограничения шима, `loadApp`/`currentApp` |
| `TEST-POLICY.md`, `AGENTS.md`, `G6-RECHECK-01.md` §L-02 | red-first, запрет ослаблений, ожидаемый тест `dispatch_ui.test.mjs`, G6-D-03 |

---

## 1. Границы среза

### 1.1 Входит

1. Подписка UI на `gateway://dispatch` (Tauri v2 `window.__TAURI__.event.listen`).
2. Проекции 6 форвардимых `kind` в состояние UI и в DOM без reload:
   `READY`, `GUILD_MEMBERS_CHUNK`, `GUILD_MEMBER_LIST_UPDATE`, `PRESENCE_UPDATE`,
   `MESSAGE_REACTION_ADD`, `MESSAGE_REACTION_REMOVE`.
3. Идемпотентность, negative cases (unknown kind / malformed / повтор / секреты),
   счётчики наблюдаемости, отсутствие «молчаливого ожидания» при `sourceAvailable=false`.
4. Один новый тест `tests/unit/dispatch_ui.test.mjs`, red-first, только fake events.

### 1.2 Не входит (и не будет имитироваться)

| Что | Почему / чья карточка |
|---|---|
| `MESSAGE_CREATE` / `MESSAGE_UPDATE` в чат | L-02 их **не** форвардит (`CONTRACTS/dispatch.md` §4); новые сообщения — M-01/M-06 |
| `TYPING_START` → `typing-indicator.start()` | не форвардится; P-03-UI/ядро |
| `MESSAGE_ACK` / read-state / бейджи | M-07; `READY.read_state` в L-02 UI не проецируется |
| Группировка live-участников по ролям/6 групп | MB-01; нужны имена ролей гильдии — отдельный срез |
| Live-данные на экране «Участники» (`#member-list`, виртуализация) | `#member-list` синтетический (`makeMember`), реальных `user.id` нет → MB-01 |
| Живой источник в ядре (`sourceAvailable=true`) | `L-02-CORE`, предложение `CONTRACTS/dispatch.md` §6 |
| Live-проверка «событие дошло до окна» | окно PM; здесь запрещено |

### 1.3 Главный факт, определяющий весь контракт

Конверт — **ровно два поля**: `{ kind, data }` (`net.rs::DispatchPayload`). Поля `s`/`seq`
в нём **нет**, поэтому UI **не может** дедуплицировать «повтор `s`» — это делает backend
в пределах соединения (`duplicates` в статистике, сброс на `Ready`/`Reconnecting`).

Следствие: **идемпотентность UI обязана быть по бизнес-ключу**, а не по `s`. Это не
ослабление: событие может прийти повторно и после reconnect (новое соединение — новый
`s`), и после переигровки на стороне ядра. Контракт ниже задаёт ключ для каждого `kind`.

`[A]`-оговорка: если G1 аддитивно добавит в конверт `s`/`seq`, UI использует его как
**дополнительный** ключ дедупа (см. §6, `normalizeEnvelope` → `seq?`), не ломаясь от
лишнего поля. Мета-тест A3 (§8.1) это фиксирует.

---

## 2. Нормализация payload (обязательна для всех проекций)

`etf_to_json` (`net.rs`) даёт: `Nil → null`, атомы → строки, `Tuple/List → массив`,
`Map → объект`, ключ-целое → строка, `NaN/Inf → null`. Дампы `fixtures/gateway/by-type`
содержат `"nil"` строкой и числа-строки (`"1792391357994"`, иногда отрицательные).
Поэтому UI нормализует **оба** представления и не полагается на тип:

| Хелпер | Правило |
|---|---|
| `nilish(v)` | `v === null \|\| v === undefined \|\| v === "nil"` |
| `asString(v)` | строка → trim; `nilish` → `""`; число → `String(v)`; иначе `""` |
| `asNumber(v)` | finite number; строка `^[+-]?\d+(\.\d+)?$` → Number; иначе `null` |
| `asBool(v)` | `true`, `"true"`, `1`, `"1"` → `true`; иначе `false` |
| `asArray(v)` | массив → массив; иначе `[]` (не бросать) |
| `asRecord(v)` | непустой объект (не массив) → объект; иначе `{}` |

Идентификаторы — **только строки** (снежинки в дампах бывают `i64`/отрицательные;
`Number()` теряет точность). Ни одно поле `data` не рендерится как JSON, не попадает
в `title`/`aria-*` целиком и не сохраняется (ни `localStorage`, ни `sessionStorage`).

---

## 3. Таблица событий: dispatch → store → DOM/ARIA → идемпотентность

Обозначения: `[L]` наблюдалось в захвате, `[S]` спецификация/эталон, `[A]` не подтверждено.

### 3.1 `READY` `[L]`

Источник: `fixtures/gateway/by-type/002-READY.json` (7 гильдий, 272 канала, `user.id`).
Секреты (`session_id`, `resume_gateway_url`, `qos_token`, `auth_session_id_hash`,
`analytics_token`) вырезаны G1; UI дополнительно их не читает (defense in depth).

**Store** (`store.live`):

```
guilds: Map<guildId, { id, name, memberCount }>
order:  guildId[]                  // порядок payload
selfUserId: string                 // см. правило ниже
onlineCountKnown: boolean          // LIST_UPDATE уже дал точный online_count
source: "dispatch"                 // факт, что данные пришли без reload
```

**`selfUserId` — один источник.** Авторитетный — `ready()` IPC (`App` уже читает его для
`typing.setSelfUserId`). `READY` dispatch заполняет поле **только если оно пустое**
(`store.selfUserId === ""`). Причина: `READY` из фикстуры принадлежит другому аккаунту
(`user.id = 384299970632876032`), а вход в тестах/спайке — `42`; перезапись сломала бы
`me` у реакций. Тест C6 это фиксирует.

Читаемые поля: `d.guilds[].id`, `d.guilds[].properties.name`, `d.guilds[].member_count`,
`d.user.id` (только fallback, см. выше). Всё остальное (включая `channels[]`, `roles[]`,
`merged_members[]`) **игнорируется** — проекция каналов уже принадлежит C-02 (`data::channels`), второй
источник истины не создаём.

**DOM/ARIA**

| Узел | Изменение |
|---|---|
| `#guild-rail .guild-item[data-guild-id]` | upsert по `guildId`: добавить отсутствующие, обновить `aria-label="Сервер <name>"`, `textContent` = монограмма (тот же `guildMonogram`, что у G-01), удалить отсутствующие **только** если rail в состоянии `ready` |
| выбор в рейле | **не менять**: `.is-selected`, `aria-current="page"`, `tabindex` ровер-навигации и `document.activeElement` сохраняются; `onSelect` **не** вызывается (иначе `channels(guildId)` на каждое READY) |
| `.tree-guild` | текст = имя выбранной гильдии, если она есть в снапшоте |
| `#member-online` | `formatNumber(memberCount)` **только** если `onlineCountKnown === false` |
| `#guild-rail` `role="navigation"` | live-регион не добавляем (озвучка списка серверов при каждом reconnect — шум) |

**Идемпотентность:** полный снапшот, ключ — `guildId`. Повторное применение того же
`READY` даёт побайтово тот же набор плиток (тот же порядок, число, имена). Плитки не
пересоздаются, если `id/name` не изменились (иначе теряется фокус).

**Negative:** `guilds` не массив → `malformed`; гильдия без `id`/`name` → пропуск
элемента + `skipped`; `d === null` → `malformed`; `READY` с секретами → значения не
попадают в DOM и в счётчики (`redactedKeys`).

### 3.2 `GUILD_MEMBERS_CHUNK` `[L]`

Источник: `009-GUILD_MEMBERS_CHUNK.json` (`chunk_index/chunk_count/guild_id/members[]`,
1 участник, гильдия `-1467272931325252608`) и `022-…` (4 участника, гильдия
`294064825540280335` — есть в `READY`; удобна для C3).

**Store:** `members: Map<guildId, Map<userId, LiveMember>>`, где

```
LiveMember = { userId, name, nick, bot, roles: string[], status?, activity? }
```

`guildId = asString(d.guild_id)`, `userId = asString(member.user.id)`,
`name = user.global_name || nick || user.username || userId` (порядок фиксирован),
`bot = asBool(user.bot)`.

**DOM:** только панель чата `#member-panel-list` (см. §4.3). Гильдия не выбрана →
только store, DOM не трогаем.

**Идемпотентность:** ключ `chunk = "${guildId}:${chunk_index}/${chunk_count}"` в
bounded-наборе. Повтор чанка → ни одного изменения (`duplicate`). Слияние участников —
по `userId` (не по индексу массива).

**Negative:** `members` не массив → `malformed`; участник без `user.id` → пропуск
одного элемента (`skipped`), остальные применяются; `chunk_count === 0` → `malformed`.

### 3.3 `GUILD_MEMBER_LIST_UPDATE` `[L]`

Источник: `005/006/008/010/011/…-GUILD_MEMBER_LIST_UPDATE.json` (70 кадров в `by-type`,
1048 в union — основной поток участников).

**Store:** плоский список окна, как в ядре (`member_list.rs`: `MemberListOp`):

```
list: Map<guildId, {
  listId, memberCount, onlineCount,
  order: userId[],                       // авторитетный порядок окна
  items: Map<userId, LiveMember>,
  range: {start,end} | null,
  opsApplied: number, complete: boolean
}>
```

Читаемые поля: `d.guild_id`, `d.id` (id списка, `everyone`/role id), `d.member_count`,
`d.online_count`, `d.groups[].id/count`, `d.ops[]`, `item.member.presence.status`.

Точная форма `ops[]` (по 70 кадрам `by-type` — 83 op-объекта: 67 с `{index,item}`, 13 с
`{index}`, 3 с `{items,range}`; **в одном кадре бывает 1, 2 или 4 op** — порядок провода
авторитетен, кадр применяется целиком):

```json
{ "op": "SYNC",   "items": [ {"group":{"id":"…"}}, {"member":{…}} ], "range": {"start":0,"end":99} }
{ "op": "INSERT", "index": 17, "item": {"member": {…}} }
{ "op": "UPDATE", "index": 3,  "item": {"member": {…}} }
{ "op": "DELETE", "index": 94 }
```

`range` в живом кадре бывает мусорным атомом (`" c"` в `005-…json`) → принимается только
объект с целыми `start`/`end`, иначе `range = null` (без throw; `range` в L-02 UI не
используется, но нормализуется, чтобы не тащить мусор в store).

**Применение ops** (порядок провода, без сортировки):

| op | Правило |
|---|---|
| `SYNC` | `order` = `userId` из `items[]`; элементы `group` в `order` не входят (снимок заголовков — `d.groups`); `range` = нормализованный `range` |
| `INSERT` | вставка в позицию `index`; если `userId` уже есть — трактуется как `UPDATE` (без дубля), `duplicate`+1 |
| `UPDATE` | замена элемента в позиции `index` (позиция вне окна → `outOfRange`) |
| `DELETE` | удаление по `index` (вне окна → `outOfRange`, ничего не удаляем) |
| неизвестный `op` | кадр целиком `malformed` (частичное применение запрещено: иначе окно теряет порядок) |

**DOM/ARIA**

| Узел | Изменение |
|---|---|
| `#member-panel-list` | `data-source="live"`, `aria-label="Участники сервера"` (нейтрально: в списке есть и offline) |
| `#member-panel-list .member[data-user-id]` | строка на участника, порядок = `order`; `data-status`, `.status-dot[data-status]`, `aria-label="<name>, <statusLabel>"` (+`, <activity>` если есть); высота 42 (`style.height`, D-04) — тот же шаблон §7.2 |
| `#member-online` | `formatNumber(online_count)`; после этого `onlineCountKnown = true` (READY больше не перебивает) |
| группы | в live-режиме `.member-group` **не создаются** (группировка = MB-01); bench-разметка 6 групп не затрагивается, пока live не пришёл |

**Идемпотентность:** `SYNC` естественно идемпотентен; `INSERT` существующего `userId`
не создаёт вторую строку; повторная последовательность `ops` даёт тот же `order`.
Инвариант: в `order` нет дублей `userId`, `order.length === items.size`.

**Negative:** `ops` не массив → `malformed` (счётчики `member_count` тоже не применяем);
`index` не целое/отрицательный → `outOfRange`; группа (`item.group`) без `id` → пропуск
op; `item.member` без `user.id` → пропуск op; `online_count` не число → счётчик не
трогаем (`#member-online` остаётся прежним, без выдуманного 0).

### 3.4 `PRESENCE_UPDATE` `[L]`

Источник: `010/012/013/…-PRESENCE_UPDATE.json` (`guild_id`, `user.id`, `status`,
`client_status`, `activities[]`, `processed_at_timestamp`).

**Store:** `presences: Map<"${guildId}:${userId}", { status, clientStatus, activity, ts }>`.
`activity` = `asString(activities[0].name)` (только имя; assets/url не рендерим).
`ts = asNumber(processed_at_timestamp)` (может быть отрицательным — не отбрасывать).

**DOM/ARIA**

| Узел | Изменение |
|---|---|
| `#member-panel-list .member[data-user-id="X"]` | `data-status`, `.status-dot[data-status]`, пересборка `aria-label` (MEM-18: статус — в доступном имени) |
| строка не в окне | store обновляется, DOM — нет; `applied: "stored"` (не «тихо потеряли») |
| `#user-bar` / self | **не** трогаем (presence себя — L-01/профиль), `selfUserId` только для `me` в реакциях |
| порядок/высота строк | не меняются (никакого re-sort и пересборки списка) |

**Идемпотентность:** last-write-wins по `ts`: событие с `ts < stored.ts` игнорируется
(`stale`), `ts === stored.ts` применяется (идемпотентно), `ts === null` применяется
без сравнения. Повтор кадра при равном `ts` не меняет DOM.

**Negative:** нет `user_id`/`user.id` → `malformed`; `status` вне
`online|idle|dnd|offline` → `data-status="unknown"`, `statusLabel="неизвестно"`,
`unknownStatus`+1 (не подменяем на `offline`); `guild_id` отсутствует → `malformed`
(приватный scope L-02 не поддерживает, не угадываем).

### 3.5 `MESSAGE_REACTION_ADD` / `MESSAGE_REACTION_REMOVE` `[L]`/`[A]`

Свидетельства: `MESSAGE_REACTION_ADD` — 1 событие в union (`facts-frozen.json`),
кадра `by-type` нет; `MESSAGE_REACTION_REMOVE` — **0** в захватах, в `DispatchKind::ALL`
отсутствует, форвардится по имени. Форма — `[A]`.

Ожидаемые поля (`[A]`, принимать оба регистра `snake_case`/`camelCase`):
`channel_id`, `message_id`, `user_id`, `emoji { id, name, animated }`, `guild_id?`,
`message_author_id?`. Обязательны: `channel_id`, `message_id`, `emoji.name|emoji.id`.

**Store:** счётчики живут **там же, где уже живут реакции сообщения** —
`chat.channelWindows.items[].reactions` (`{emoji,count,me}`) — второго источника истины
нет. В bridge-store только `userId`-сетка дедупа и LRU-ключи.

**DOM/ARIA** (только открытый канал, только загруженное окно):

| Узел | Изменение |
|---|---|
| `#chat-list .row[data-channel-message="true"][data-message-id="<id>"]` | строка сообщения; чужая строка/канал → `not-open`/`not-loaded` |
| `.text .reactions-row[role="group"][aria-label="Реакции"]` | создать, если реакций не было (`data-stack="true"` на `.text` тоже выставить) |
| `.reaction[data-emoji="<emoji>"]` | `data-me`, `aria-pressed`, `aria-label="Реакция <emoji>, <count>"`, `.reaction-count` = count |
| count → 0 | пилюля **удаляется** (без «призрака» с нулём); `aria-pressed` не остаётся `true` |

**Идемпотентность:** ключ `"${channelId}:${messageId}:${emoji}:${userId}:${ADD|REMOVE}"`,
LRU 512. `ADD` уже учтённого пользователя → no-op (`duplicate`); `REMOVE` неизвестного
пользователя → no-op + `ignored` (счётчик **не** уходит в минус); `REMOVE` при
`count === 0` → no-op. `me = (userId === store.selfUserId)`; `ADD` себя → `aria-pressed="true"`,
`REMOVE` себя → `false`.

**Negative:** нет `emoji.name` и `emoji.id` → `malformed`; нет `user_id` → событие
применяется как «кто-то», но без `me` и без дедупа (`degraded`+1) — альтернатива
(полный отказ) отклонена: счётчик важнее персональной атрибуции; `channel_id` не равен
открытому → `not-open` (DOM не трогаем); сообщение не в окне → `not-loaded`.

### 3.6 `TYPING_START` и остальные 15 типов `[L]`

Не форвардятся (`CONTRACTS/dispatch.md` §4). Bridge обязан их **не** обрабатывать:
`kind` не в `FORWARDED_KINDS` → `ignored`+1, без throw, без console-шума. Это же
поведение для `MESSAGE_CREATE`, `CHANNEL_INFO`, `READY_SUPPLEMENTAL`, мусора `"X"`.

---

## 4. DOM-контракт: точные селекторы и правила

### 4.1 Существующие якоря (менять нельзя)

> Нотация `>` в §4.3 — только схема вложенности. В рантайме и в тестах используются
> лишь селекторы, которые понимает DOM-шим (DOM-CONTRACT §9.4): `#id`, `.class`, `tag`,
> `tag.class`, `[attr="value"]` (в т.ч. несколько подряд), потомок через пробел, группы.
> Поиск вложенных узлов — `row.querySelector(".status-dot")`/обход `children`, не `>`.

| Селектор | Роль | Файл |
|---|---|---|
| `#guild-rail .guild-item[data-guild-id]` | плитка гильдии (`aria-label="Сервер <name>"`, `.is-selected`, `aria-current="page"`, ровер `tabindex`) | `app.js` |
| `.tree-guild` | имя гильдии в шапке дерева | `app.js` |
| `#channel-tree .channel-group[data-channel-type]`, `.channel-item[data-channel-id]` | дерево каналов (C-02) | `app.js` |
| `#chat-header[data-channel-id]`, `.chat-title-text` | шапка чата | `chat.js` |
| `#chat-list[role="list"]`, `.row[data-channel-message="true"][data-message-id]`, `.text`, `.reactions-row[role="group"]`, `.reaction[data-emoji]`, `.reaction-count` | строки/пилюли | `chat.js` |
| `#member-panel-list[role="list"]`, `.member`, `.status-dot[data-status]`, `.member-group[data-group-key]`, `.member-group-title[data-height="42"]` | панель участников | `members.js` |
| `#member-online` | счётчик в заголовке панели | `index.html`/`members.js` |
| `#net-status` | баннер L-01 (в потоке `.topbar`) | `app.js` |
| `body[data-active-screen]` | канонический экран | `screens.js` |

### 4.2 Новые атрибуты (аддитивные, без новой разметки и CSS)

| Атрибут | Узел | Значения | Зачем |
|---|---|---|---|
| `data-user-id` | `#member-panel-list .member` | строка-снежинка | адресация presence/reaction-обновлений, O(1) |
| `data-source` | `#member-panel-list` | `bench` \| `live` | не смешивать синтетику и live; bench-тесты не ломаются |
| `data-dispatch-source` | `document.body` и `#net-status` | `live` \| `absent` \| `unavailable` | «UI не ждёт молча пустой канал» (`CONTRACTS/dispatch.md` §8) без нового текста/баннера/CSS |
| `data-live-order` | `#member-panel-list` | целое = `order.length` | диагностика/тесты порядка окна без чтения JS-структур |

`index.html` **не меняется** → правки `tests/support/gui-dom-harness.mjs` (зеркало id/классов,
DOM-CONTRACT §9.1) не требуются. Это осознанное ограничение контракта.

### 4.3 Форма live-строки участника

Используется тот же шаблон, что §7.2/DOM-CONTRACT, чтобы не разъехались высоты и CSS:

```
div.member[role="listitem"][data-user-id][data-status][data-index]
  span.avatar.avatar--md[aria-hidden="true"] > span.status-dot[data-status]
  span.member-name > span.member-label(<name>)
  span.member-role(<activity | statusLabel>)
  style.height = 42px
  aria-label = "<name>, <statusLabel>" (+ ", <activity>", если activity есть)
```

`renderMemberRow` (`members.js`) получает аддитивный 4-й аргумент
`{ total?: number, userId?: string, label?: string }`: `total` → `aria-setsize`,
`userId` → `data-user-id`, `label` → готовый `aria-label` (live-адаптер собирает его сам,
чтобы не получить хвостовую запятую и не дублировать статус в `activity`).
При отсутствии аргумента поведение bench-строк не меняется ни на атрибут
(регресс-тесты §8.3 это фиксируют).

### 4.4 Инварианты «без reload» (проверяются в тесте, не декларируются)

1. Число IPC-вызовов `ready`, `channels`, `messages_page` **не меняется** после любого
   набора dispatch-событий (счётчики `app.callsOf(...)` до/после).
2. `#chat-list` не пересобирается: длина `children` и идентичность нетронутых строк
   сохраняются; `#chat-list.scrollTop` не меняется.
3. `document.activeElement` и `body[data-active-screen]` не меняются; фокус не переезжает.
4. Строки, которых событие не касается, не перерисовываются (идентичность узлов).
5. `currentApp().router.current` тот же экран.

### 4.5 ARIA-правила

| Правило | Деталь |
|---|---|
| Статус — не только цветом | статус в `aria-label` строки (MEM-18) и в `data-status` |
| Live-регионы | `#member-online` уже `aria-live="polite"` — счётчик озвучивается; реакции и статусы **не** добавляют live-регион (шум при 29+ событиях/сек) |
| Роли | `#member-panel-list` остаётся `role="list"`, строки — `role="listitem"`; `aria-setsize/posinset` для live = `order.length`/`index+1` (не `24 229`) |
| Пилюля | `button.reaction`, `aria-pressed`, непустой `aria-label` (MSG-23…26), `data-emoji` |
| Скрытые узлы | никаких `aria-hidden` на обновляемых строках; placeholder-строки не получают `data-user-id` |

---

## 5. Negative cases (сводная таблица)

| # | Случай | Ожидание |
|---|---|---|
| N1 | `kind` не из 6 (`TYPING_START`, `MESSAGE_CREATE`, `"X"`, пустая строка) | `ignored`+1, DOM без изменений, `console.error` = 0, подписка жива |
| N2 | `data === null`/массив/строка для любого `kind` | `malformed`+1, DOM без изменений, исключения нет |
| N3 | обязательное поле отсутствует (`user.id`, `ops`, `guilds`, `emoji`) | `malformed`/`skipped`, частичные данные не применяются (кроме явно описанных «пропуск элемента») |
| N4 | повтор кадра (в т.ч. `READY` после reconnect) | тот же DOM; `duplicate`/no-op; дублей плиток/строк/пилюль нет |
| N5 | `INSERT`/`ADD` уже известного ключа | no-op (`duplicate`), второго элемента нет |
| N6 | `DELETE`/`REMOVE` без предшествующего `ADD`, `index` вне окна | no-op + `outOfRange`/`ignored`; счётчики не отрицательные |
| N7 | секреты в `data` (регресс `net.rs`): `session_id`, `token`, `*_token`, `resume_gateway_url`, `auth_session_id_hash`, `client_secret`, `password` | значения **не** попадают в `textContent`, атрибуты, `title`, счётчики, снапшот; `redactedKeys`+1; UI продолжает работать |
| N8 | гигантские/отрицательные id (`"-13390286217619177728"`) | остаются строками, без `Number`, без `NaN` в атрибутах |
| N9 | `"nil"`/`null`/`0`/`"false"` в разных местах | `nilish`/`asBool` дают одинаковый результат для дампа и для живого кадра |
| N10 | burst: 100 `PRESENCE_UPDATE` подряд | 100 применений к store, ≤100 записей в DOM, последний статус победил, ошибок нет |
| N11 | событие при `body[data-active-screen]="settings"` | чат/панель обновляются в фоне, экран не переключается, фокус не крадётся |
| N12 | нет transport (`window.__TAURI__.event` отсутствует) | `body[data-dispatch-source="absent"]`, никаких throw; `currentApp().dispatch.handle` всё равно работает (тестовый путь) |
| N13 | исключение внутри DOM-применения одного события | `failed`+1, `lastErrorKind` (без payload), следующее событие обрабатывается |

---

## 6. Архитектура UI-слоя (3 новых файла + 4 узкие правки)

```
ui/features/dispatch-model.js   (новый, чистый: без DOM/глобалов)
    DISPATCH_EVENT, FORWARDED_KINDS
    nilish/asString/asNumber/asBool/asArray/asRecord
    normalizeEnvelope(value) -> {ok, kind, data, seq?} | {ok:false, reason}
    createDispatchStore()
    applyDispatch(store, envelope) -> { changes: Change[], counters }
    projectReady / projectMembersChunk / projectMemberListUpdate
    projectPresence / projectReaction            (экспорт для fixture-тестов)

ui/features/dispatch-view.js    (новый, DOM: только §4-селекторы)
    createDispatchView({ body, memberPanelList, memberOnline, treeHeading,
                         rail, chat, selectedGuildId? })
      .apply(changes) -> { applied, ignored, failed }
      .selectGuild(guildId)          // вызывается из rail.onSelect
      .stats

ui/features/dispatch-bridge.js  (новый: транспорт + счётчики + изоляция ошибок)
    resolveDispatchListen(win = window)        // __TAURI__.event.listen | null
    createDispatchBridge({ listen, view, now, dedupeSize })
      .start() .stop() .handle(envelope) .stats .source
```

`Change` — единственный шов model↔view, DOM в модели нет:

| `type` | Поля |
|---|---|
| `guilds` | `guilds: [{id,name,memberCount}]`, `selfUserId` |
| `members` | `guildId`, `order: string[]`, `rows: [{userId,name,status,statusLabel,activity}]`, `onlineCount?`, `memberCount?`, `listId` |
| `presence` | `guildId`, `userId`, `status`, `statusLabel`, `activity` |
| `reaction` | `channelId`, `messageId`, `emoji`, `count`, `me`, `removed` |

Внутренние API, которые контракт требует от существующих модулей (узко и аддитивно):

| Модуль | Добавить | Зачем |
|---|---|---|
| `chat.js` | `applyReaction({channelId,messageId,emoji,count,me,removed}) -> "applied"\|"not-open"\|"not-loaded"`; приватный `reactionsGroupOf(row)` (вынести существующий блок из `renderChannelMessage`, поведение bench не меняет) | live-пилюли тем же шаблоном, без дублирования разметки |
| `members.js` | 4-й аргумент `renderMemberRow` (`{total?,userId?,label?}`) + экспорт `STATUS_LABELS` | live-строка без второй копии шаблона §7.2 |
| `reactions.js` | `adjustReactionOverride(key, emoji, delta, me)` (аддитивно) | не дать optimistic-override перетереть live-счётчик на следующем `refreshRow` |
| `app.js` | `rail.applyGuilds(items)` (upsert без смены выбора/фокуса/`onSelect`); wiring bridge в `boot()`; `dispatch` в объекте `App`; `dispatch` в `report_ui_snapshot`; `onSelect` → `view.selectGuild` | одна точка подписки, наблюдаемость |

`start()` — только после `auth ready` (как `net.start()`), `stop()` — на logout
(как `net.stop()`/`typingSender.reset()`). `listen` возвращает `Promise<UnlistenFn>`;
`stop()` обязан дождаться и вызвать unlisten, повторный `start()` не создаёт вторую
подписку.

Счётчики (`bridge.stats`, снапшот `dispatch`): `source` (`live|absent|unavailable`),
`received`, `applied`, `ignored`, `malformed`, `duplicate`, `stale`, `outOfRange`,
`skipped`, `degraded`, `unknownStatus`, `notLoaded`, `notOpen`, `failed`,
`redactedKeys`, `lastErrorKind`. **Ни payload, ни текста сообщений, ни секретов.**

---

## 7. File ownership

### 7.1 Меняет G2 (L-02-UI)

| Файл | Тип | Объём/границы |
|---|---|---|
| `rust/gui-spike/ui/features/dispatch-model.js` | новый | весь файл |
| `rust/gui-spike/ui/features/dispatch-view.js` | новый | весь файл |
| `rust/gui-spike/ui/features/dispatch-bridge.js` | новый | весь файл |
| `rust/gui-spike/ui/features/chat.js` | правка | только вынос `reactionsGroupOf` + `applyReaction` в возвращаемом объекте; строки bench/канала не меняют вывод |
| `rust/gui-spike/ui/features/members.js` | правка | 4-й аргумент `renderMemberRow` (`{total?,userId?,label?}`; без него bench-строка не меняется), экспорт `STATUS_LABELS` |
| `rust/gui-spike/ui/features/reactions.js` | правка | только экспорт `adjustReactionOverride` (10–15 строк, без изменения `toggleReaction`/`requestReaction`) |
| `rust/gui-spike/ui/app.js` | правка | `rail.applyGuilds`, wiring bridge, `dispatch` в `App`/снапшоте, `onSelect`-хук |
| `tests/unit/dispatch_ui.test.mjs` | новый | весь файл |

### 7.2 Меняет G1 (не трогать в UI-срезе)

`rust/gui-spike/src/net.rs`, `rust/gui-spike/src/main.rs`, `rust/gui-spike/tests/net_contract.rs`,
`rust/crates/discord-core/**` (хук `CONTRACTS/dispatch.md` §6), `CONTRACTS/dispatch.md`
(владелец — G1/PM).

### 7.3 Заморожено (никто в этой карточке)

`rust/gui-spike/ui/index.html` (новых id нет), `tests/support/gui-dom-harness.mjs`,
`ui/style.css`, `ui/tokens.css`, `ui/components/*.css` (CSS — G4),
`ui/model.js`, `ui/features/{screens,typing-indicator,rows,virtual-list,errors,dom}.js`,
`ui/features/composer.js`, `ui/features/search*.js`, `ui/features/voice.js`,
`fixtures/**`, `logs/`, `functions/`, `evidence/` (golden, только чтение),
`tests/unit/*` других карточек, `waves/db.jsonl`, `waves/STATE.md` (Team Lead).

### 7.4 Конфликтные файлы — обязательная проверка до старта

`app.js` (P-03-UI, G-01, G-03, C-02), `chat.js` (C-02-PAGE, M-06, M-07),
`members.js` (P-01/P-02, MB-01), `reactions.js` (M-06). Перед стартом PM подтверждает,
что ни одна открытая карточка не владеет этими файлами. Если `app.js` занят —
fallback-порядок: (1) отложить `rail.applyGuilds` (READY обновляет только
`.tree-guild` + `#member-online`), (2) при занятом `chat.js` — только участники/presence,
реакции отдельным под-срезом.

---

## 8. Test plan — `tests/unit/dispatch_ui.test.mjs`

Правила: только fake events, без live/сети/браузера/`cargo`; оракулы — фикстуры и
**исходник** `net.rs` (как `typing-ui.test.mjs` читает `types.rs`); `loadApp` из harness;
`currentApp().dispatch.handle(envelope)` — тот же путь, что колбэк `listen`.
Модульное состояние dispatch живёт в `boot()`, а не в модуле → утечки между тестами нет
(в отличие от `reactions.js`, где нужен `resetReactions`).

### 8.1 Группа A — контракт (мета, Rust как оракул)

| # | Кейс | Утверждение |
|---|---|---|
| A1 | `DISPATCH_EVENT` | значение в `dispatch-model.js` === строка `net.rs::DISPATCH_EVENT` |
| A2 | `FORWARDED_KINDS` | множество === `FORWARDED_DISPATCHES` в `net.rs`, ровно 6, порядок не важен |
| A3 | конверт | `DispatchPayload` в `net.rs` содержит только `kind`/`data` (фиксирует §1.3); `normalizeEnvelope` принимает лишние поля и отдаёт `seq` только если оно есть |

### 8.2 Группа B — чистые проекции (без DOM)

| # | Кейс | Оракул |
|---|---|---|
| B1 | READY: 7 гильдий, имена, `member_count`, `selfUserId` | `002-READY.json` независимо от UI |
| B2 | READY: `d` из дампа (`"nil"`) и он же с `null` дают **равную** проекцию | оба варианта |
| B3 | READY дважды → `deepEqual` store и `changes` пустые | идемпотентность |
| B4 | CHUNK: участники по `user.id`, `name` по приоритету `global_name→nick→username→id` | `009/022-…json` |
| B5 | CHUNK дважды → `duplicate`, store равен | ключ чанка |
| B6 | LIST_UPDATE `SYNC`: `order`/counts равны независимому пересчёту `ops[0].items` | `005-…json` |
| B7 | LIST_UPDATE `INSERT/UPDATE/DELETE`: независимая модель в тесте (массив) vs `store.order`; кадр с 2 и 4 op применяется целиком и по порядку | серия кадров (`005/079/086-…json`) |
| B7a | `SYNC` с мусорным `range` (`" c"`) → `range=null`, остальное применилось | `005-…json` |
| B8 | дубль `INSERT` userId → нет дубля, `duplicate`+1 | N5 |
| B9 | `DELETE` вне окна → `outOfRange`, порядок не меняется | N6 |
| B10 | PRESENCE: LWW (старый `ts` → `stale`), равный `ts` применяется, `ts`-строка/отрицательный | `010-…json` + синтетика |
| B11 | REACTION ADD/REMOVE: ±1, clamp 0, `me` по `selfUserId`, повтор ADD → no-op | `[A]`-форма |
| B12 | REACTION REMOVE без ADD → нет отрицательного счёта | N6 |
| B13 | таблица malformed: `null`/`[]`/`"x"`/`{}` по каждому kind → `{applied:0, malformed:1}` | N2/N3 |
| B14 | unknown kind (`TYPING_START`, `MESSAGE_CREATE`, `"X"`) → `ignored`, DOM-изменений 0 | N1 |
| B15 | секреты в `d`: рекурсивный обход store/changes/`JSON.stringify` не находит значений | N7 |

### 8.3 Группа C — интеграция через реальный `app.js`

| # | Кейс | Утверждение |
|---|---|---|
| C1 | READY → рейл | `.guild-item[data-guild-id]` count = `guilds.length`, `aria-label` = `Сервер <name>`; выбор/фокус/`aria-current` сохранены; `channels`/`ready` IPC = 0 новых |
| C2 | LIST_UPDATE → панель | `#member-panel-list[data-source="live"]`, `aria-label="Участники сервера"`, строки `[data-user-id]`, порядок = `order`, высота 42, `#member-online` = `formatNumber(online_count)` |
| C3 | CHUNK после LIST_UPDATE | имена появились у тех же строк (идентичность узлов сохранена), число строк не выросло |
| C4 | PRESENCE online→idle→online | `data-status` строки и `.status-dot`, `aria-label` пересобран; `#chat-list.scrollTop`/фокус/активный экран не изменились; строка — тот же узел |
| C5 | REACTION ADD (чужой) | пилюля создана в `.text .reactions-row`, `.reaction-count="1"`, `aria-pressed="false"`, `aria-label="Реакция <e>, 1"`, `data-stack="true"` |
| C6 | REACTION ADD (self) | `aria-pressed="true"`, `data-me="true"` |
| C7 | REACTION REMOVE до 0 | пилюля удалена, `aria-pressed` не остался `true`, счётчик не отрицательный |
| C8 | REACTION для неоткрытого канала/незагруженного сообщения | `#chat-list` без изменений, `not-open`/`not-loaded`, других IPC нет |
| C9 | N7 секреты | обход `textContent` всех узлов + всех атрибутов + `report_ui_snapshot` не содержит значений секретов |
| C10 | N1/N2 | ни одного изменения DOM, `console.error` пуст, `stats.ignored/malformed` выросли |
| C11 | N10 burst 100 presence | 100 применений, DOM-строки не пересозданы, финальный статус верный |
| C12 | N11 событие на экране settings | `body[data-active-screen]="settings"` не изменился, DOM обновлён в фоне |
| C13 | N12 нет transport | `body[data-dispatch-source="absent"]`, `#net-status[data-dispatch-source="absent"]`, throw нет |
| C14 | снапшот | `report_ui_snapshot.dispatch` присутствует в payload (`source`, счётчики) и не содержит payload/секретов; живые счётчики после событий читаются из `currentApp().dispatch.stats` (снапшот публикуется один раз на boot) |

**Состав кадров в интеграционных кейсах.** Фикстуры L-02 сняты с разных аккаунтов/прогонов:
`READY` — 7 гильдий, среди них `1014537851418980372` (ITSKIDON); `GUILD_MEMBERS_CHUNK` и
`GUILD_MEMBER_LIST_UPDATE` — гильдия `-1467272931325252608` (в `READY` отсутствует);
`PRESENCE_UPDATE` — гильдия `1014537851418980372`. Поэтому тест **компонует конверт из
`[L]`-кадров**, переклеивая только `guild_id` (и, где нужно, `user.id` из того же кадра),
чтобы события относились к одной выбранной гильдии; поля не выдумываются, файлы фикстур
не меняются. Для C2–C4/C11: в копию `READY.d` добавляется гильдия
`-1467272931325252608` (`member_count` из `LIST_UPDATE`), затем она выбирается
(`rail.applyGuilds` + `view.selectGuild`) до подачи событий.

### 8.4 Группа D — транспорт (юнит)

| # | Кейс |
|---|---|
| D1 | `createDispatchBridge({listen: fake})`: `start()` → один вызов с `gateway://dispatch`, колбэк применяет конверт |
| D2 | `stop()` вызывает unlisten; повторный `start()` не создаёт вторую подписку |
| D3 | ошибка в `view.apply` → `failed`+1, `lastErrorKind`, следующее событие обрабатывается (N13) |
| D4 | `resolveDispatchListen` без `window.__TAURI__.event` → `null`; с функцией → она же |
| D5 | `listen` упал (`Promise.reject`) → `source="unavailable"`, ошибка учтена, без unhandled rejection |

### 8.5 Red-first и команды

RED (до продуктового кода), ожидаемая причина падения — отсутствующие модули/свойства,
а не assert:

```
node --test tests/unit/dispatch_ui.test.mjs
# ожидаемо: ERR_MODULE_NOT_FOUND dispatch-model.js (A1…B15, D*)
#           + TypeError: currentApp().dispatch is undefined (C1…C14)
# exit code 1, лог: waves/RW-40/<batch>/dispatch-ui-red.log
```

GREEN/targeted:

```
node --test tests/unit/dispatch_ui.test.mjs
node --test tests/unit/chat-reactions.test.mjs tests/unit/channel-switch.test.mjs \
             tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-dom.test.mjs
npx tsc -p rust/gui-spike/ui/tsconfig.json
npm run gate:worker            # один раз на batch, integration-лейн
```

Для C1–C14 используется `loadApp({ invokeHandler })` с теми же ответами, что в
`chat-reactions`/`channel-switch` (`auth_status`, `ready`, `channels`, `messages_page`,
`read_state`, `net_status`), затем открывается канал `1163408591315812392` и
выполняется `currentApp().dispatch.handle(...)`.

### 8.6 DoD среза UI

1. 6 `kind` дают изменения в DOM без reload; инварианты §4.4 проверены.
2. §5 N1–N13 покрыты (минимум по одному assert на случай).
3. Ни одного нового IPC-вызова на dispatch, ни одного `console.error`.
4. `body[data-dispatch-source]` честен: `absent`/`unavailable` при `sourceAvailable=false`.
5. Регресс-тесты зелёные без правки их ожиданий; `tsc` чистый; `gate:worker` зелёный.
6. Секреты: ни значения, ни ключи не попадают в DOM/снапшот/счётчики.
7. CSS/`index.html`/harness не менялись.

---

## 9. Риски и что заморозить

| # | Риск | Оценка | Митигация в контракте |
|---|---|---|---|
| R1 | **Нет `s` в конверте** → UI не видит повтор кадра; после RESUME возможна переигровка | высокая | идемпотентность по бизнес-ключам (§3), `seq?` опционален (§1.3, A3), тесты B3/B5/B8/B11 |
| R2 | **Форма `MESSAGE_REACTION_ADD/REMOVE` — `[A]`** (1 событие в union, 0 REMOVE) | высокая | толерантный парсинг snake/camel, обязательный минимум полей, `degraded`/`malformed` вместо падения; live-проверка только в окне PM |
| R3 | **`L-02-CORE` изменит контракт**: имена полей статистики, `sourceDetail`, добавит `s`/`seq`, поменяет набор `kind` | средняя | заморозка §9.1; UI не ветвится по тексту `sourceDetail`; мета-тесты A1–A3 краснеют при дрейфе, а не «тихо» живут |
| R4 | **Разные представления nil/чисел** в дампе и live (`"nil"`, `"true"`, число-строка) | средняя | §2 + B2 (один и тот же вывод для двух вариантов) |
| R5 | **Конфликт файлов** (`app.js`, `chat.js`, `members.js`, `reactions.js`) | средняя | §7.4: подтверждение PM до старта + fallback-порядок; правки узкие, bench-путь не меняется |
| R6 | **Live-панель теряет 6 групп** (визуальная регрессия) | низкая | группы = MB-01; live-режим помечен `data-source="live"`, bench-разметка не тронута |
| R7 | **Optimistic override в `reactions.js` перетирает live-счётчик** при следующем `refreshRow` | средняя | `adjustReactionOverride` (§6), тест C5/C7 c предварительным `toggleReaction` |
| R8 | **PRESENCE без имени** (в `PRESENCE_UPDATE` только `user.id`) | низкая | имя берём из CHUNK/LIST_UPDATE; presence имя не выдумывает |
| R9 | **Burst/нагрузка**: 24k участников, 29+ presence/сек | низкая | O(1) по `data-user-id`, без re-sort и пересборки панели; C11 |
| R10 | **`net_dispatch_stats` ещё не источник истины** (`sourceAvailable=false`) | — | UI показывает `absent/unavailable`, ничего не имитирует |

### 9.1 Что заморозить до старта UI (freeze list)

1. Имя события `gateway://dispatch` и конверт `{kind,data}` (+ опциональный `seq`).
2. Набор 6 `kind` и правило «остальное → `ignored`».
3. Имена счётчиков `net_dispatch_stats` (camelCase) — читаем только `sourceAvailable`.
4. Читаемый минимум полей `d` (§3.1–§3.5) — расширять можно, переименовывать нельзя.
5. Новые DOM-атрибуты §4.2 и правило «`index.html` не меняется».
6. Ключи идемпотентности (§3) и имена счётчиков bridge (§6).
7. Инварианты §4.4 (no reload) как приёмочные.
8. Границы §1.2 (что L-02 UI не делает) — иначе срез расползётся в M-01/MB-01/P-03.

### 9.2 Открытые вопросы к PM (нужны до кода)

1. **R5:** кто владеет `app.js`/`chat.js`/`members.js` на момент старта UI-среза?
   (Если заняты — применить fallback §7.4, а не «договариваться правкой».)
2. **R7:** разрешён ли аддитивный экспорт `adjustReactionOverride` в `reactions.js`
   (файл M-06)? Если нет — принять известное ограничение и завести follow-up.
3. **R2:** после первого live-захвата `MESSAGE_REACTION_*` — добавлять типизированный
   парсинг в ядро (G1) или оставлять raw и жить на толерантном UI?
4. Порог для `degraded`-событий реакции (без `user_id`): применять счётчик или
   отбрасывать целиком. Предложение — применять (`§3.5`), т.к. счётчик важнее атрибуции.
5. Нужен ли видимый индикатор «живые обновления недоступны» (текст/бейдж) — это новый
   CSS/дизайн (G4), поэтому сейчас только `data-dispatch-source` + снапшот.

---

## 10. Сводка для ответа PM

- **Файл контракта:** `waves/RW-40/PM/l02-ui/PLAN.md` (этот документ).
- **Mapping:** 6 `kind` → 4 группы состояния (guilds / members+list / presence / reactions)
  → 6 адресуемых DOM-якорей; ключи идемпотентности — `guildId`, `userId`, `chunk`,
  `channelId:messageId:emoji:userId:action`.
- **Ownership:** G2 — 3 новых модуля + `chat.js`/`members.js`/`reactions.js`/`app.js`
  (узко) + `tests/unit/dispatch_ui.test.mjs`; G1 — `net.rs`/`main.rs`/`net_contract.rs`/
  `discord-core`; `index.html`, harness, CSS, фикстуры — заморожены.
- **Test cases:** 3 мета (A1–A3), 16 проекций (B1–B15 + B7a), 14 интеграционных (C1–C14),
  5 транспортных (D1–D5); red-first по `ERR_MODULE_NOT_FOUND`.
- **Risks:** отсутствие `s` в конверте (R1), `[A]`-форма реакций (R2), возможный дрейф
  контракта в `L-02-CORE` (R3), конфликт файлов (R5), override-конфликт (R7) —
  все закрыты правилами и тестами, а не обещаниями.
