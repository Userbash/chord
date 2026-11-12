# G3-DM-FRIENDS-SPEC — независимое ревью, Pass 1

**Вердикт: RETURN (P1).** Протокольная часть снята с RE точно: 24 маршрута совпадают
по методу и пути один-в-один, живой READY-оракул сходится (17 ЛС, 60 связей, 146
пользователей), token-only CSS формально чист (95 токенов, 0 пропусков, 0 сырых
цветов, 0 `!important`). Приёмку блокируют семь мест: правило скрытия `#chat-header`
проигрывает существующему селектору `:has()`, белый текст на `--positive` даёт
3.36:1 вместо AA 4.5:1, режим Home нигде не связан с `guildId='@me'` и не
восстанавливает шапку дерева, клавиша `Home` противоречит замороженному тесту,
а контракт ядра не описывает ни модель `recipients`, ни шесть из семи новых
dispatch-переходов.

Ревью read-only. Прочитаны `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (1876 строк),
`waves/RW-40/DM-FRIENDS-RE.md` (968 строк), `rust/gui-spike/ui/tokens.css`,
`rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/style.css`,
`rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/components/{rail,tree,primitives}.css`,
`rust/gui-spike/ui/features/notifications.js`,
`rust/crates/discord-core/src/{ready/social.rs,state/types.rs,state/presence.rs,dispatch.rs}`,
`tests/unit/guild-rail.test.mjs`, `tests/support/gui-dom-harness.mjs`,
`reference-oracle/gateway/by-type/002-READY.json`, `reference-oracle/rest-static.json`.
Изменён только этот файл. HEAD `4e24048`, ветка `rw40/review-dm-friends-spec`.

Классификация: **P0 — 0, P1 — 7, P2 — 12.**

## Сводка по пунктам задания

| Пункт | Результат | Комментарий |
|---|---|---|
| Кнопка Home/Friends в `#guild-rail` | **FAIL (P1)** | Кнопка описана, но не связана с `guildId='@me'`; при возврате в гильдию не восстанавливается шапка дерева |
| Переключение `guildId='@me'` | **FAIL (P1)** | 0 вхождений `@me` как id гильдии; существующая конвенция `notifications.js:1330` не учтена |
| ЛС и Group DM из `READY.private_channels`, типы 1/3 | PASS с оговоркой (P1/P2) | Типы, сортировка и 0 GET есть; в модели нет поля под `recipients`/`rawRecipients` |
| Сортировка по `last_message_id` | PASS с оговоркой (P2) | Правило есть, но без tie-break и с неоднозначной сортировкой закреплённых |
| 0 вызовов `GET /users/@me/channels` | PASS | Явно заявлено и закреплено оракулом `T-N107-DM-GET-01`; `rest-static.json` подтверждает 0 GET |
| Друзья из `READY.relationships`, типы 1–4 | PASS с оговоркой (P2) | Маппинг типов верный; источник `presence` для вкладки «В сети» не назван |
| Вкладки Online / All / Pending / Blocked / Add Friend | PASS с оговоркой (P2) | Все пять есть; нет тест-оракула на фильтры и сортировку |
| `POST /users/@me/channels` `{recipients:[...]}` | PASS | Тело, нормализация и негативный оракул на `recipient_id` совпадают с RE |
| `PUT`/`DELETE /users/@me/relationships/{id}` | PASS | Тело, единый маршрут удаления и `80013` совпадают с RE |
| `POST /users/@me/relationships`, Pomelo `null` | PASS | `parseInt(undefined) → NaN → JSON null` описано верно |
| Код `80013` | PASS | Диалог, `isStranger` и повтор с `confirm_stranger_request: true` |
| CSS-токены, 0 сырых цветов, 0 `!important` | PASS формально; **FAIL по контрасту (P1)** | 95/95 токенов существуют; белый на `--positive` — 3.36:1 |
| WAI-ARIA роли, состояния, клавиатура | **FAIL (P1)** | Набор ролей широкий, но `Home` конфликтует с замороженным тестом; есть пробелы P2 |

## P1 — блокируют приёмку

### 1. `.content[data-home-subview="friends"] #chat-header` не скроет шапку: селектор слабее существующего

Спецификация скрывает шапку чата правилом
`waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md:1159-1162`:

```css
.content[data-home-subview="friends"] #chat-header,
.content[data-home-subview="friends"] #screen-chat {
  display: none;
}
```

В `rust/gui-spike/ui/style.css:153-158` уже есть конкурирующие правила:

```css
.content:has(#screen-chat.active) #chat-header { display: flex; }
.content:has(#screen-chat:not(.active)) #chat-header { display: none; }
```

Специфичность первого — `(2,2,0)`: `#screen-chat` внутри `:has()` и `#chat-header`
дают два id, `.content` и `.active` — два класса. У правила спецификации — `(1,2,0)`.
По каскаду `(2,2,0)` побеждает независимо от порядка подключения файлов. Пока
`#screen-chat` остаётся `.active` (а спецификация оставляет
`body[data-active-screen="chat"]`, §0.2), `#chat-header` продолжит рисоваться над
`#friends-view`. При этом сам `#screen-chat` скрывается корректно: `(1,2,0)` с id
бьёт `.screen.active` `(0,2,0)`. То есть дефект асимметричный и визуально
проявится сразу: шапка чата с прежним каналом над списком друзей.

Что нужно: либо поднять специфичность правила до уровня существующего
(`.content[data-home-subview="friends"]:has(#screen-chat.active) #chat-header`),
либо снимать `.active`/`aria-hidden` с `#screen-chat` при входе в Friends и
возвращать при выходе, либо вводить отдельный `body[data-active-screen="friends"]`
и правило с id. `!important` запрещён §0.2, поэтому это единственные варианты.
Добавить проверку каскада в `T-N107-A11Y-01` или отдельный CSS-тест.

### 2. Белый текст на `--positive` — 3.36:1, это ниже AA; §0.2 ошибочно объявляет пару допустимой

`waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md:40` разрешает `--positive` как фон
кнопки «Добавить в друзья» в паре с `--text-on-accent`/`--text-strong`. В CSS это
реализовано в `.friends-tab--add` (`:1243-1246`):

```css
.friends-tab--add {
  background: var(--positive);
  color: var(--text-on-accent);
}
```

В `rust/gui-spike/ui/tokens.css:106` `--positive: #3d9e60`, в `:89`
`--text-on-accent: #ffffff`, в `:81` `--text-strong: #fbfbfb`. Расчёт по WCAG 2.x
(relative luminance) даёт:

- `#ffffff` на `#3d9e60` — **3.36:1**;
- `#fbfbfb` на `#3d9e60` — **3.24:1**.

Нужно 4.5:1: текст вкладки — `--font-size-ui` 14px, `--font-weight-medium` 500, то
есть не «крупный» текст (порог 3:1 к нему не применим). Комментарий в `tokens.css`
про «5.17 на `--bg-chat`» относится к `--positive` как цвету текста/индикатора на
тёмном фоне, а не к белому тексту на зелёной заливке. Сейчас спецификация нарушает
собственное правило §0.2 п.2 и `THEME-SPEC.md` §2.2–2.6, а `T-N107-TOKENS-01`
этого не поймает: он проверяет только имена токенов и отсутствие сырых цветов.

Что нужно: заменить фон CTA на `--accent` + `--text-on-accent` (4.61:1, уже
используется для primary-кнопок) либо добавить в `tokens.css` отдельный тёмный
`--text-on-positive` (подходит `#121214`, 5.58:1) по процедуре §6 `THEME-SPEC.md`.
В `T-N107-TOKENS-01` добавить проверку контраста для пар «фон/текст», а не только
наличие переменных.

### 3. Требование `guildId='@me'` не покрыто: Home остаётся чисто DOM-атрибутом

В задании явно требуется переключение `guildId='@me'`. В спецификации `@me`
встречается только внутри путей `/users/@me/...`; как идентификатор домашней
гильдии он не упомянут ни разу. Режим Home описан только через
`#channel-tree[data-nav-mode="home"]` (`:226-227`, `:236-241`) и
`data-home-subview` на `.content`.

Это расходится с уже существующей конвенцией кода: в
`rust/gui-spike/ui/features/notifications.js:1330` и `:1384` проверка
`guildId === "@me"` выбирает DM-настройки и endpoint
`/users/@me/guilds/@me/settings`. Спецификация цитирует `NOTIFICATIONS-SPEC.md`
как источник, но не связывает Home с этой веткой.

Последствия: `createGuildRail.selectedId()` (`app.js:412-416`) читает только
`data-guild-id` плиток, поэтому Home не участвует в выборе; при восстановлении
выбора после смены экрана или повторного READY нет способа отличить Home от
последней реальной гильдии, а настройки уведомлений из Home уйдут в эту гильдию,
а не в `@me`. Это не косметика, а неопределённость состояния между UI и ядром.

Что нужно: ввести `HOME_GUILD_ID = "@me"` в контракт состояния и навигации,
ставить `data-guild-id="@me"` на `#home-rail-btn`, учитывать его в
`selectedId()`/восстановлении выбора, явно запретить `channels("@me")` (вместо
этого — список ЛС) и добавить тест: Home → `guildId === "@me"` → DM-endpoint
настроек. Если решение сознательно другое, нужен тег `[A]`, обоснование и
негативный оракул, а не молчание.

### 4. Возврат из Home в гильдию не восстанавливает `.tree-header`: `select()` теряет `.tree-guild`

§2.2 (`:244-256`) заменяет шапку `#channel-tree` на
`<header class="tree-header dm-tree-header">` с кнопкой поиска. В guild-режиме
существующая шапка содержит `h2.tree-guild` и
`button[data-action="guild-menu"]` (`index.html:55-57`). Спецификация
переключает только `data-nav-mode` и `aria-label` (`:226-227`), но не говорит,
кто возвращает прежнюю шапку и `#voice-panel`/`#user-bar`-соседей.

При этом `createGuildRail.select()` в `app.js:439-445` ищет
`document.querySelector(".tree-guild")` и обновляет имя гильдии. Если шапка
заменена DM-версией, `heading` равен `null`, и после клика по плитке название
сервера в дереве не обновится. `createChannelTree` (`app.js:817-826`) держит
ссылку на `.tree-scroll`, но шапку не перерисовывает вовсе. Кнопка меню сервера
также исчезнет.

Что нужно: описать в §2.1 явный контракт переключения: сохранить/восстановить
разметку `.tree-header` (`.tree-guild`, `guild-menu`) либо рендерить шапку из
`createChannelTree` по режиму; при возврате вызывать `channels(guildId)` и
перерисовывать список; в Home не вызывать `channels()`. Добавить тест
«Home → клик по гильдии → в шапке снова имя гильдии и кнопка меню».

### 5. Клавиша `Home` на плитке гильдии противоречит замороженному тесту

§5.2 п.1 (`:1839`) требует: «`Home` с любой плитки `.guild-item` возвращает фокус
на `#home-rail-btn`». Существующий `app.js:496-505` обрабатывает `Home` иначе —
`move("first", true)`, то есть выбирает первую гильдию и переводит на неё фокус.
Это закреплено тестом `tests/unit/guild-rail.test.mjs:240-241`:

```js
last.dispatchEvent("keydown", { key: "Home" });
assert.equal(api(app).rail.selectedId, GUILDS[0]?.id, "Home не выбрал первую гильдию");
```

Спецификация не помечает это как изменение замороженного поведения и не требует
править тест. Если G2 реализует §5.2 буквально, `guild-rail.test.mjs` упадёт;
если не реализует — нарушится A11Y-контракт `T-N107-A11Y-01`.

Что нужно: либо сохранить текущую семантику (`Home` → первая гильдия, а на
`#home-rail-btn` вести, например, `ArrowUp` с первой плитки), либо явно
зафиксировать смену поведения, обновить `guild-rail.test.mjs` и указать это в
§0.2 как разрешённое изменение `[F]`-контракта.

### 6. Контракт редуктора покрывает только `READY` и `CHANNEL_CREATE`; шесть из семи событий без перехода

§1.5 (`:180-183`) добавляет семь `DispatchKind`, но в `state/reducer.rs`
описывает ровно два правила: полная замена на `READY` и идемпотентная вставка
`CHANNEL_CREATE` с мержем `recipients`. Для `CHANNEL_DELETE`,
`CHANNEL_RECIPIENT_ADD`, `CHANNEL_RECIPIENT_REMOVE`, `RELATIONSHIP_ADD`,
`RELATIONSHIP_UPDATE`, `RELATIONSHIP_REMOVE` нет ни удаления по id, ни upsert
связи, ни обновления `recipient_ids`/`nicks`, ни мержа вложенного `user` в
`state.users`, ни пересчёта `pendingCount`/`friendCount`.

RE §7.4 задаёт внутренние действия для каждого события, а §7.2 прямо говорит,
что стор применяет `addRecipient` только к приватным каналам. Без этих правил
G1 будет вынужден изобретать поведение: что делать с неизвестным id, что
приоритетнее — локальный `CHANNEL_CREATE` или шлюзовой, удалять ли связь при
`RELATIONSHIP_REMOVE` без `user`, обновлять ли `users` из `RELATIONSHIP_ADD`.
`T-N107-DISPATCH-01` проверяет только идемпотентность, переход `4 → 1` и
удаление без `user`, но не закрывает остальные шесть событий.

Что нужно: таблица «событие → операция с `private_channels`/`relationships`/
`users` → идемпотентность → поведение при неизвестном id» на все семь событий,
включая `CHANNEL_DELETE` и recipient-события. Оракулы — на каждую строку.

### 7. Модель `PrivateChannel` не вмещает `recipients`, хотя §1.1.3 и §2.2 на неё опираются

§1.1.3 (`:62-63`) требует принимать оба представления: `recipient_ids` из READY
и `recipients: User[]` из REST/`CHANNEL_CREATE`, «попутно обновляя `users`».
§2.2 (`:383`) для имени группы использует
`rawRecipients.map(u => channel.nicks[u.id] || u.global_name || u.username)`.
Но §1.5 (`:169-175`) добавляет в `PrivateChannel` только `name`, `icon`,
`owner_id`, `nicks`, `safety_warnings`,
`blocked_user_warning_dismissed` — поля под `recipients`/`rawRecipients` нет,
и правило «сложить `recipients` в `recipient_ids` + `state.users`» не
сформулировано. Существующая структура
(`rust/crates/discord-core/src/ready/social.rs:138-159`) хранит только
`recipient_ids: Vec<u64>`.

Итог: G1 может реализовать парсер двумя несовместимыми способами (потерять
объекты получателей или завести неописанное поле), а G2 — получить
несуществующий `rawRecipients`. Для GROUP_DM это ломает и имя, и подпись
«N участников».

Что нужно: одно из двух — добавить `pub recipients: Vec<User>` (или
`raw_recipients`) в модель и в тест сериализации, либо явно записать
нормализацию: `recipients[]` → `recipient_ids[]` + upsert в `state.users`,
а `rawRecipients` в UI считать производным от `recipient_ids` и `state.users`.
Формулу имени переписать на существующее поле.

## P2 — не блокируют, но должны быть закрыты до заморозки

1. **Осиротевшие имена и рассинхрон атрибутов.** §0.2 (`:44-45`) упоминает
   `.dm-tree-list` и `#dm-recipient-menu`, которых больше нет ни в разметке, ни
   в CSS; фактический контейнер — `#dm-channel-list`/`.dm-channel-list`, меню —
   `#friend-context-menu` (§2.5, `:869`). Там же сказано, что `.content`
   получает `data-nav-mode="home"`, а CSS и §2.3 используют
   `data-home-subview`. Оставить одно имя на каждый узел.

2. **Монтирование `#home-rail-btn` не определено для тестового DOM-шима.**
   §2.1 предлагает «монтируется … перед `.guild-list`» (`:191`), но
   `.guild-list` создаётся только в `createGuildRail.ensureList()`
   (`app.js:398-409`) после READY; в DOM-шиме (`tests/support/gui-dom-harness.mjs:805-822`)
   `.guild-list` нет вовсе, плитки добавляются прямо в рейл. На старте якоря
   не существует. Зафиксировать идемпотентное монтирование
   (`if (!byId("home-rail-btn"))`) относительно стабильного `.rail-sep` или
   заменить `.brand`, а не добавлять второй `.brand`. Если `.brand` превращается
   в кнопку, отдельно потребовать снять с него `aria-hidden="true"`
   (`index.html:38`): фокусируемый элемент с `aria-hidden` — нарушение.

3. **Начальное состояние противоречиво.** В примере разметки
   `#home-rail-btn` уже `.is-selected` с `aria-current="page"`, а
   `#channel-tree` — `data-nav-mode="home"` (`:198`, `:240`). При этом
   `app.js:renderGuilds` после READY выбирает первую гильдию, то есть на старте
   будет два выбранных элемента рейла и два `aria-current="page"`. Определить
   boot-режим: либо guild-режим без Home, либо Home с запретом авто-выбора
   первой гильдии.

4. **Сортировка ЛС и друзей недоопределена.** Для ЛС (`:380`) не сказано, как
   сортируются закреплённые между собой; `BigInt(last_message_id ?? id)` — `[A]`,
   а не `[D]`, и смешивает два ключа без tie-break. Для друзей (`:1796-1799`)
   `localeCompare("ru")` зависит от ICU и не даёт детерминированного порядка при
   равных именах — нужен вторичный ключ по `user.id` и `Intl.Collator("ru")`.
   Перенести сортировку в оракул `T-N107-SORT-01`.

5. **Раздел «Запросы переписки» описан только кнопкой.** `#dm-nav-requests`
   (`:279-290`) имеет `data-action="open-message-requests"`, но ни в §2, ни в §4
   нет панели/фильтра, который он открывает, и нет правила счётчика/скрытия при
   нуле. Аналогично нет правил `hidden` для `#dm-nav-friends-badge`,
   `#dm-nav-requests-badge`, `#friends-pending-badge`; определено только скрытие
   бейджа Home (`:228`). Добавить `#message-requests-view` (или явно
   «тот же `#dm-channel-list` с фильтром `is_message_request === true`») и
   правило «0 → `hidden`».

6. **ARIA-детали, которые сломают APG-паттерны.** `#friends-tabpanel` имеет
   `tabindex="0"` (`:492`), хотя внутри есть фокусируемый `#friends-search-input`;
   по APG `tabindex="0"` на панели нужен только без фокусируемых потомков.
   `aria-labelledby="friends-tab-online"` жёстко указывает на первую вкладку —
   при переключении его нужно обновлять (в таблице §5.1 написано
   `friends-tab-*`, в разметке — конкретный id). `.group-dm-friend-option`
   (`:803`) имеет `tabindex="0"` при combobox с `aria-activedescendant`; в этом
   паттерне фокус остаётся в `#group-dm-search`, а опции не входят в tab-порядок.
   `aria-expanded="true"` у combobox статичен, хотя список фильтруется. У
   `#dm-nav-friends`/`#dm-nav-requests` бейджи `aria-hidden`, а число не входит
   в доступное имя; у `#home-rail-btn` `aria-label` содержит только «2 входящих
   запроса», тогда как правило бейджа (`:228`) суммирует ещё и упоминания.
   Focus trap диалогов описан словами «циклический», но в §5.2 нет обработки
   `Tab`/`Shift+Tab` и `inert`/`aria-hidden` фона. Свести в один пункт контракта.

7. **Пункты контекстного меню без поведения.** `#friend-context-menu` (`:869-876`)
   содержит `friend-edit-nick`, `friend-edit-note`, `friend-toggle-ignore`, но
   §4.2 описывает только принятие/отклонение/отмену/разблокировку и открытие ЛС.
   Нет ни UI-редактора ника/заметки, ни привязки к
   `PATCH /users/@me/relationships/{id}` / `GET|PUT /users/@me/notes/{id}` /
   `PUT|DELETE .../ignore`, ни проверки `note_user_id`. Также неясно, вызывает
   ли `#friends-clear-pending-btn` в режиме «Спам и игнор» обычный
   `DELETE ...?relationship_type=3` или вариант с телом `{"filters":[1,2]}`.
   Подпись «Спам и игнор (N)» показывает только `spamCount`, без
   `pendingIgnoredCount` (`:134`).

8. **Метки доказательности местами завышены.** Локальная проверка длины
   дискриминатора `!== 4` (`:714`) в RE §4.2 отсутствует — `sendRequest` только
   парсит и отправляет; это `[A]`, а не `[D]`. Статус `200/201` для
   `POST /users/@me/channels` (`:75`) в RE не зафиксирован — `[A]`. Точные
   русские строки ошибок в §1.4 (`:145-164`) — адаптация, RE даёт только
   код → реакцию; строки нужно помечать `[A]`. Проверка кэша
   `type === 1 && recipient_ids[0] === userId` (`:56`) выведена из
   `_openCachedDMChannel`, но её тело в RE не приведено — `[A]`. Форма GROUP_DM
   подтверждена только кодом, в захвате групп нет (RE §5.1, §10) — это стоит
   сказать явно. Строка `20026 / 40068` (`:145`) склеивает два разных контекста
   (создание ЛС и заявка в друзья) — разнести.

9. **Источник `presence` для вкладки «В сети» не назван.** §4.1 (`:1796`)
   фильтрует по `presence.status ∈ {online, idle, dnd, streaming}`, но
   `streaming` не является вариантом `PresenceStatus`
   (`rust/crates/discord-core/src/state/presence.rs:20-34`: `Online`, `Idle`,
   `Dnd`, `Offline`, `Invisible`, `Unknown`); стриминг в Discord — активность, а
   не статус. В состоянии уже есть `presences` и `MergedPresences`
   (`state/types.rs:118`, `:425`). Нужно указать путь к данным и правило
   «стриминг → `data-status="streaming"`» (из `activities[].type === 1`) либо
   убрать `streaming` из множества статусов.

10. **Матрица тестов не покрывает вкладки, сортировку и контраст.**
    `T-N107-*` (`:1864-1876`) проверяет READY, POST, отсутствие GET, shell,
    модалку, Pomelo, `80013`, DELETE, заметки, dispatch и A11Y, но не проверяет
    пять вкладок и их фильтры (§4.1), порядок ЛС по `last_message_id`/`pinned`,
    скрытие бейджей при нуле и контраст пар токенов. `T-N107-TOKENS-01`
    ограничен именами и сырыми цветами, поэтому текущее нарушение AA пройдёт
    зелёным. Добавить `T-N107-TABS-01`, `T-N107-SORT-01` и проверку контраста
    (минимум для `--positive`/`--accent` CTA).

11. **Мелкая ошибка ссылки на код.** §1.5 (`:176-178`) указывает
    `social.rs:162-178` для `Relationship`; фактически структура начинается на
    `social.rs:163` (строка 162 — комментарий/пустая). Поправить диапазон.

12. **Кнопка «Добавить друзей в ЛС» в `#chat-header` не существует.** §2.4
    (`:726`) открывает модалку этой кнопкой, но в `index.html:102-114` такой
    кнопки нет и разметка для неё в спецификации не добавлена. Либо добавить
    `button[data-action="open-create-group-dm"]` в `#chat-header` с
    `aria-label`, либо убрать сценарий.

## Проверенные пункты (совпадает с RE и кодом)

- **Каталог REST.** Все 24 строки §1.2 совпадают с RE §1.1 по методу и пути;
  отдельно сверены `POST /users/@me/channels` (`{recipients}` всегда массив,
  `recipient_id` только у `group-dm-shell`), `GET /users/@me/dms/{id}`,
  `PUT|DELETE|PATCH /users/@me/relationships/{id}`, `PUT|DELETE .../ignore`,
  `DELETE /users/@me/relationships?relationship_type=3` (с телом
  `{"filters":[1,2]}` для спама/игнора), заметки с обязательной проверкой
  `note_user_id`. `rest-static.json` подтверждает: `USER_CHANNELS` — только POST,
  2 call-site, тело `{recipients}`; GET-вызовов 0.
- **Коды ошибок.** Таблица §1.4 покрывает все коды RE §2.3, §4.6, §4.8, включая
  `20026`, `30002`, `30004`, `30059`, `30078`, `40002` (403), `40003`, `40007`,
  `40068`, `10013`, `50003`, `50015`, `50035`, `50096`, `80001`–`80004`,
  `80007`, `80013`, `340005`–`340007` и `429` с `errorUxConfig === 0`.
- **Enum'ы и флаги.** `ChannelType` 1/3, `PRIVATE_CHANNEL={1,3}`,
  `RelationshipType` 0–6, `ChannelFlags.PINNED=2`/`IS_SPAM=32`,
  `RecipientFlags` 1/2, `RelationshipFilters` 1/2 и формула
  `pendingCount = max(incoming − spam − ignored, 0)` совпадают с RE §3.3,
  §4.7, §5.3.
- **Живой READY-оракул.** `jq` по
  `reference-oracle/gateway/by-type/002-READY.json`: 17 каналов, все `type=1`,
  60 связей (18 типа 1, 42 типа 2), 146 пользователей, 0 записей с
  `recipient_ids.len() != 1`, 0 записей с `id != user_id`. Числа в §1.1.3,
  §1.1.4 и `T-N107-READY-01` совпадают с данными, а не с пересказом.
- **Token-only CSS.** В блоке §3.2 95 уникальных `var(--...)`; все 95 объявлены
  в `rust/gui-spike/ui/tokens.css`; таблица §3.1 содержит те же 95 имён и те же
  базовые значения (многострочные `--font-mono` и
  `--shadow-elevation-low` сверены отдельно; `--motion-fast` в `@media
  (prefers-reduced-motion)` переопределяется на `0ms`, базовое значение
  `120ms`). Сырых `#hex`/`rgb()`/`hsl()` — 0, `!important` — 0, скобки
  сбалансированы.
- **Каркас.** В `index.html` остаются ровно 4 `.screen` (`#screen-chat`,
  `#screen-emoji`, `#screen-members`, `#screen-settings`); `.brand`,
  `.rail-sep`, `#voice-panel`, `#user-bar`, `#chat-header`, `#screen-chat`
  существуют. `createGuildRail` действительно не трогает `.brand`
  (`app.js:398-409`), а `#channel-tree`/`.tree-scroll` — стабильные якоря.
- **ARIA-каркас.** Роли `tablist/tab/tabpanel`, `dialog/alertdialog`,
  `menu/menuitem`, `listbox/option`, `combobox`, roving `tabindex`, возврат
  фокуса на `opener`, `stopPropagation()` по `Escape` и `aria-hidden` на
  декоративных бейджах описаны; это достаточная база для `T-N107-A11Y-01`
  после правок P2.

## Что проверить в Pass 2

1. Пересчитать специфичность CSS после правки §3.2 и показать, что
   `#chat-header` скрыт в Friends при `#screen-chat.active`.
2. Пересчитать контраст всех пар «фон/текст» из §3.2, минимум для
   `.friends-tab--add`, `.group-dm-btn--primary`, `.add-friend-submit`,
   `.friend-action-btn--danger`, `.home-rail-btn.is-selected`.
3. Проверить, что Home ставит `guildId='@me'`, не вызывает `channels("@me")`,
   восстанавливает `.tree-guild`/`guild-menu` и проходит `guild-rail.test.mjs`
   либо тест явно изменён с записью в отчёте.
4. Проверить, что §1.5 содержит переходы на все семь dispatch-событий и поле
   под `recipients`/`rawRecipients` (или нормализацию в `recipient_ids`+`users`).
5. Прогнать `T-N107-*` на фикстурах `002-READY.json` и негативных кейсах
   (`RELATIONSHIP_ADD` без `user`, `CHANNEL_CREATE` без `recipients`,
   `CHANNEL_DELETE` с неизвестным id).

Проверка выполнена только чтением назначенных исходников; спецификация, код,
тесты и golden-каталоги не изменялись. Единственный записанный файл — этот отчёт.
