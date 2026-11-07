# G3-DM-FRIENDS-SPEC — независимое ревью, Pass 2

**Вердикт: RETURN (P1).** Семь P1 и двенадцать P2 из
`waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW.md` закрыты по существу: `#chat-header`
скрывается явным состоянием без нового `:has()`, контраст CTA на `--positive` стал
`5.58:1`, Home получил `HOME_GUILD_ID = "@me"` и восстановление `.tree-header`,
клавиша `Home` больше не конфликтует с `tests/unit/guild-rail.test.mjs`, таблица
редуктора покрывает все 7 событий, а `recipients[]` нормализуется в
`recipient_ids` + `state.users`. Приёмку блокирует одно новое место: сохранение и
восстановление режима Home через `lastGuildId: "@me"` невозможно на замороженном
коде — значение молча отбрасывается пятью фильтрами (два в Rust, три в UI),
поэтому boot-ветка `settings_load → "@me"` недостижима, а выбор Home не
переживает перезапуск.

Ревью read-only. Прочитаны `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (2094 строки),
`waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW.md`, `waves/RW-40/DM-FRIENDS-RE.md`
(968 строк), `rust/gui-spike/ui/tokens.css`, `index.html`, `style.css`, `app.js`,
`components/primitives.css`, `ui/features/{settings,notifications,join-server,actions}.js`,
`rust/gui-spike/src/settings_store.rs`, `rust/crates/discord-core/src/{ready/social.rs,state/{types,presence,reducer}.rs,dispatch.rs}`,
`tests/unit/guild-rail.test.mjs`, `tests/support/gui-dom-harness.mjs`,
`fixtures/gateway/by-type/002-READY.json`. Изменён только этот файл. HEAD `4e24048`,
ветка `rw40/dm-friends-spec-pass2`.

Классификация: **P0 — 0, P1 — 1, P2 — 11.** Все находки ниже проверены по коду,
RE и живому fixture, а не по пересказу спецификации.

## Сводка по пунктам задания

| Пункт | Результат | Комментарий |
|---|---|---|
| `:has()` для скрытия `#chat-header` | PASS | Явные `hidden` + `aria-hidden` + `data-home-subview`; в §3.2 `:has()` — 0. Запасное правило `(2,2,1)` действительно сильнее `(2,2,0)` из `style.css:153` |
| Контраст WCAG 2.2 AA ≥ 4.5:1 на всех парах | PASS по существу, P2 по числам | `#121214` на `#3d9e60` = **5.58:1** (пересчитано); все 16 пар §3.3 проходят, но 6 значений в таблице посчитаны неверно |
| Явный `guildId="@me"` | PASS | `HOME_GUILD_ID`, `data-guild-id="@me"`, `selectedId()`, запрет `channels("@me")` |
| Сохранение/восстановление `.tree-header` | PASS | §2.1.2 явно скрывает и возвращает `.tree-guild`/`guild-menu`, обновляет имя гильдии |
| Сохранение/восстановление режима Home | **FAIL (P1)** | `lastGuildId: "@me"` не проходит ни один из пяти фильтров замороженного кода |
| Клавиша `Home` vs `guild-rail.test.mjs` | PASS | Сохранена семантика `Home → GUILDS[0]`, `End → GUILDS[last]`, `ArrowUp` с первой → последняя |
| Полный контракт редуктора и модели `recipients` | PASS | Таблица на все 7 событий + `READY`/`READY_SUPPLEMENTAL`; `rawRecipients` убран, имя группы строится по `recipient_ids` |
| 100% `var(--...)`, 0 сырых цветов, 0 `!important` | PASS | 96/96 токенов объявлены и совпадают по базовому значению; `!important` и `:has()` — 0 в правилах |

## Закрытие 7 P1 Pass 1

1. **Скрытие `#chat-header` — закрыто.** §0.2 п.4 (`:49-51`) вводит два синхронных
   механизма: атрибуты `hidden`/`aria-hidden` и `data-home-subview` на
   `main.content` (§2.1.2, §2.3) плюс каскадное правило §3.2 (`:1303-1306`)
   `main.content[data-home-subview="friends"] #chat-header#chat-header` со
   специфичностью `(2,2,1)`. Правило сильнее `.content:has(#screen-chat.active) #chat-header`
   `(2,2,0)`; кроме того `[hidden] { display: none !important; }`
   (`components/primitives.css:64`) гарантирует скрытие независимо от каскада.
   Новых `:has()` в нормативном CSS нет.
2. **Контраст `--positive` — закрыто.** §0.2 п.2 (`:41`) запрещает белый на
   `--positive` и требует `--text-on-warning`; `.friends-tab--add` (`:1386-1392`)
   реализует пару `#121214`/`#3d9e60`. Пересчёт по WCAG 2.x: `#ffffff` на
   `#3d9e60` = 3.36:1, `#fbfbfb` = 3.24:1, `#121214` = **5.58:1**. Шрифт вкладки —
   14px/500, то есть не large text, порог 4.5:1 применим.
3. **`guildId='@me'` — частично закрыто.** Идентификатор введён и связан с
   `notifications.js:1330,1384`; `selectedId()` и запрет `channels("@me")`
   описаны, оракулы `T-N107-HOME-NAV-01`/`T-N107-DM-GET-01` есть. Но
   персистентность режима Home нереализуема — см. P1 ниже.
4. **Восстановление `.tree-header` — закрыто.** §2.1.2 п.2 (`:290-295`) требует
   не удалять `.tree-guild` и `button[data-action="guild-menu"]`, а скрывать их
   `hidden`, идемпотентно воссоздавать при отсутствии и обновлять
   `.tree-guild.textContent`; `createGuildRail.select()` (`app.js:439-442`)
   продолжает находить узел. Соседи `#voice-panel`/`#user-bar` не трогаются.
5. **Клавиша `Home` — закрыто.** §5.2 п.1 (`:2046-2054`) фиксирует прежнюю
   семантику: `Home` на плитке выбирает `GUILDS[0]`, `End` — последнюю,
   `ArrowUp`/`ArrowDown` цикличны; это совпадает с `tests/unit/guild-rail.test.mjs:231-252`.
   Переход на Home — `Tab`/`Enter`/`Space`/`ArrowDown` с `#home-rail-btn`.
6. **Редуктор на 7 событий — закрыто.** §1.5.2 (`:210-220`) даёт таблицу
   «payload → операция с `private_channels`/`relationships`/`users` →
   идемпотентность → неизвестный id» для `READY`, `READY_SUPPLEMENTAL`,
   `CHANNEL_CREATE`, `CHANNEL_DELETE`, `CHANNEL_RECIPIENT_ADD/REMOVE`,
   `RELATIONSHIP_ADD/UPDATE/REMOVE`. `T-N107-DISPATCH-01` перечисляет негативные
   кейсы (`RELATIONSHIP_ADD` без `user`, `CHANNEL_CREATE` без `recipients`,
   удаление неизвестного id). Роутинг через `SocialDispatchKind::ALL` не ломает
   `DispatchKind::ALL.len() == 21` (`dispatch.rs:67`).
7. **Модель `recipients` — закрыто.** §1.5.1 п.2 (`:191-197`) фиксирует
   единственный хранимый список `recipient_ids`, upsert `recipients[]` в
   `state.users`, проекцию для UI и `sort_recipients`; `rawRecipients` в
   документе отсутствует, формула имени `GROUP_DM` (§2.2 п.3, `:476-484`)
   использует `recipient_ids` + `state.users`. Существующая структура
   `social.rs:138-159` расширяется шестью `Option`/`Vec`/`BTreeMap`-полями.

## Закрытие 12 P2 Pass 1

1. Осиротевшие имена убраны: `.dm-tree-list` и `#dm-recipient-menu` в документе
   не встречаются; атрибуты разделены (`#channel-tree[data-nav-mode]`,
   `main.content[data-home-subview]`).
2. Монтирование `#home-rail-btn` описано идемпотентно (`:235-240`), включая
   снятие `aria-hidden` с кнопки и перенос на декоративных потомков.
3. Boot-режим определён (`:241-244`): по умолчанию guild, Home — только по
   `lastGuildId === "@me"` или действию пользователя; взаимное исключение
   `.is-selected`/`aria-current` задано.
4. Сортировка: §2.2 п.2 (`:469-475`) — две корзины по `PINNED`, внутри
   `last_message_id` ↓ и `channel.id` ↓; §4.1.2 (`:1976-1992`) — `Intl.Collator`
   с tie-break по id и трёхключевой порядок для `pending`.
5. «Запросы переписки»: §2.2 п.1 (`:457-468`) задаёт фильтр `is_message_request`,
   заголовок секции, `#message-request-banner` и правило `0 → hidden` для всех
   четырёх бейджей.
6. ARIA: `#friends-tabpanel` без `tabindex`, динамический `aria-labelledby`,
   опции combobox без `tabindex`, динамический `aria-expanded`, счётчики в
   `aria-label`, focus trap с `inert`/`Tab`/`Shift+Tab` (§5.1–§5.2).
7. Все 5 пунктов `#friend-context-menu` описаны (§2.5, `:1002-1017`), включая
   `note_user_id`, `PATCH nickname`, ignore и оба варианта очистки pending.
8. Метки доказательности поправлены: `200/201` — `[A]`, локальная проверка
   `4` цифр — `[A]`, кэш-предикат — `[A]`, отсутствие GROUP_DM в захвате — `[A]`,
   коды `20026`/`40068` разнесены по контекстам, русские строки помечены `[A]`.
9. Источник presence назван (§4.1.1), `streaming` больше не статус, а
   производное `data-status` из `activities[].type === 1`. Тип и ссылки на код
   при этом неверны — см. P2.1.
10. Матрица тестов дополнена `T-N107-TABS-01`, `T-N107-SORT-01`,
    `T-N107-HOME-NAV-01`, проверкой контраста в `T-N107-TOKENS-01`.
11. Диапазон `Relationship` исправлен: `social.rs:163-178` (проверено).
12. `#chat-add-recipients-btn` добавлена в §2.4 (`:833-847`) с `hidden` вне
    режима DM; место монтирования (`#chat-header .chat-actions`) существует и в
    `index.html`, и в DOM-шиме.

## P1 — блокирует приёмку

### 1. `lastGuildId: "@me"` не сохраняется и не читается: пять фильтров замороженного кода отбрасывают значение

Спецификация дважды опирается на `lastGuildId === "@me"`:

- §2.1.1 (`:242`): «На старте после `ready()` (если `settings_load` не вернул
  `lastGuildId === "@me"`)…»;
- §2.1.2 (`:243`): «`settings_save` сохраняет `lastGuildId: "@me"` с сохранением
  всех остальных полей `UiSettings`».

Ни один из этих путей не работает на текущем коде:

1. `rust/crates/discord-core` не участвует; запись идёт через
   `rust/gui-spike/src/settings_store.rs`.
2. `SettingsStore::save` (`settings_store.rs:518-524`) до валидации вызывает
   `normalize_last_guild_id`, а тот (`:281-284`) пропускает только snowflake
   (`is_snowflake`: непустая десятичная строка, влезающая в `u64`). `"@me"`
   превращается в `None` — **без ошибки и без видимого последствия**, файл
   записывается без `lastGuildId`.
3. `UiSettings::validate` (`:202-211`) для нецифрового значения вернул бы
   `Invalid("lastGuildId")`, но до неё нормализованное `None` уже не доходит.
4. `readLastGuildId` в `app.js:335-339` при загрузке принимает только
   `/^\d{1,20}$/`, иначе `""`; `preferredGuildId`/boot-выбор гильдии получают
   пустую строку, а не `"@me"`.
5. Диалог настроек сохраняет через `normalizeSettings`
   (`features/settings.js:209-212`) и `getEffectiveLastGuildId` (`:281-285`) —
   оба тоже требуют digits. При сохранении любых настроек из Home в файл уйдёт
   `currentLastGuildId` (последняя числовая гильдия) либо поле будет опущено.

Итог: `settings_save` завершается успешно, но выбор Home теряется; после
перезапуска boot всегда выбирает гильдию. Заявленный оракул
`T-N107-HOME-NAV-01` проверяет только вызовы/атрибуты, поэтому пройдёт зелёным
на сломанном поведении. Это не косметика: пользователь каждый раз возвращается
в сервер, а «домашний» режим существует только до перезагрузки.

Что нужно (одно из двух, с явной записью в §0.2 как `[F]`-изменение):

- **A. Отдельное поле схемы.** Добавить в `UiSettings` (Rust + JS) булево
  `homeMode`/`navMode` с дефолтом `false`/`"guild"`, сохранять его в
  `rememberGuild`, а `lastGuildId` оставить цифровым. Тогда boot: `navMode ===
  "home"` → Home, иначе восстановленная гильдия. Нужны тесты Rust-сериализации,
  `normalize_last_guild_id` не меняется.
- **B. Расширить G-01b до `"@me"`.** Ослабить `is_snowflake`-фильтр в
  `normalize_last_guild_id`, `validate`, `readLastGuildId`, `normalizeSettings`,
  `getEffectiveLastGuildId`; добавить `"@me"` в допустимые значения и тесты на
  round-trip save → load.

В любом варианте `T-N107-HOME-NAV-01` должен проверять полный цикл
`settings_save → settings_load → boot`, а не только аргументы IPC; иначе дефект
повторится.

## P2 — не блокируют, но должны быть закрыты до заморозки

1. **Тип и ключ `state.presences` описаны неверно (§4.1.1, `:1968-1974`).**
   В коде это `BTreeMap<PresenceKey, Presence>`
   (`state/types.rs:118`), где `PresenceKey = (u64, PresenceScope)`
   (`presence.rs:256-264`), а не `BTreeMap<Snowflake, Presence>`. Псевдокод
   `state.presences.get(id)` в §4.1.1 и таблице §4.1.3 недействителен: нужен
   `(id, PresenceScope::Private)`. Ссылки на строки тоже устарели: `is_online` —
   `presence.rs:65-69`, `ActivityType` — `:78-82`/`:107`, `Presence`/`activities`
   — `:268-278` (в спеке `:14-16`, `:20-34`, `:36-50`). Это единственный P2,
   который лучше закрыть вместе с P1, потому что от него зависит вкладка «В сети».
2. **Арифметика таблицы контраста §3.3 (`:1941-1958`).** Шесть значений не
   сходятся с формулой WCAG (проверено независимым скриптом): `.dm-group-title`
   `4.74` → фактически **4.89**; `.friend-username-tag`/`.friend-status-text`
   `5.98` → **5.96**; `.dm-item-subtext` `6.24` → **6.43**;
   `.friend-action-btn--accept:hover` `8.10` → **7.65**; `.add-friend-feedback`
   `8.36/7.36` → **8.57/7.63**. Все пары остаются ≥ 4.5:1, но таблица — источник
   оракула G4; числа нужно пересчитать, иначе тест унаследует ложные значения.
   Отдельно: минимальная пара документа — `--text-faint` на `--bg-chat`
   4.54:1, запас всего 0.04; это стоит отметить в комментарии CSS.
3. **Выдуманный snowflake в §5.2 (`:2049`).** «`GUILDS[0]`, `100000000000000001`»
   не совпадает с fixture: первый id — `274093052673392641`
   (`fixtures/gateway/by-type/002-READY.json`). Оракул клавиатуры должен брать
   id из payload, а не из литерала.
4. **Расхождение формул `aria-label` и битая ссылка (§5.1, `:2030`).** §5.1
   ссылается на несуществующий §2.1.4 (в документе только §2.1.1–§2.1.2) и даёт
   `"Домой — Личные сообщения и друзья"`, тогда как §2.1.2 п.3 и разметка §2.1.1
   используют `"Личные сообщения и Друзья, …"`. Аналогично `#dm-nav-friends` в
   разметке — «2 входящих запроса», в §5.1 — «N ожидающих запросов», у
   `#dm-nav-requests` — «1 новый запрос» против «N непрочитанных». Нужна одна
   строка на элемент; иначе G4 не сможет зафиксировать точное доступное имя.
5. **Начальное состояние `#friends-view` не задано.** Разметка §2.3 (`:490-498`)
   не содержит `hidden`, а boot по §2.1.1 — guild-режим. Если `features/dm-friends.js`
   смонтирует панель «как в разметке», `.friends-view { display: flex }` покажет
   её поверх гильдейского чата до первого перехода. Явно потребовать монтаж с
   `hidden`/`aria-hidden="true"` (и аналогично для `#message-request-banner`).
6. **Roving `tabindex` при выбранном Home.** §2.1.2 п.1 (`:285`) говорит, что
   `tabindex="0"` остаётся на «первой/последней активной» плитке, но не
   определяет, на какой именно. Если все плитки получат `-1` (как в текущем
   `select()` для не-цели), `Tab` с `#home-rail-btn` перепрыгнет `.guild-list`
   сразу на `.rail-btn`, что противоречит §5.2 п.1. Зафиксировать: активная
   плитка, иначе первая, держит `tabindex="0"`.
7. **Владение монтированием Home-дерева.** §0.2 п.4 (`:52`) перечисляет
   `#friends-view`, `#group-dm-dialog`, `#stranger-confirm-dialog`,
   `#friend-context-menu`, `#dm-channel-list`, но не `#dm-quick-search-btn`,
   `.dm-nav-list`, `#dm-nav-requests-item`. §2.1.2 предполагает, что кнопка уже
   существует («с неё снимается `hidden`»). Добавить их в идемпотентный монтаж
   `features/dm-friends.js`, иначе в браузере и в DOM-шиме кнопка поиска не
   появится.
8. **`components/dm-friends.css` нигде не подключён.** В `index.html:9-22` —
   фиксированный список `<link>`; спецификация вводит файл (§3.2, `:1126`), но
   не требует добавить `<link rel="stylesheet" href="components/dm-friends.css">`.
   Без него §3.2 не применяется (спасает только глобальный `[hidden]`), а
   `T-N107-TOKENS-01`, читающий файл, этого не заметит.
9. **Оракул `T-N107-TOKENS-01` противоречит артефакту.** В нормативном CSS-блоке
   есть комментарий с сырыми hex: `:1386` — `--positive (#3d9e60)` и
   `--text-on-warning (#121214)`. Оракул «0 сырых `#hex` … в CSS-блоках» в
   буквальной формулировке упадёт на собственном примере. Либо убрать hex из
   комментария, либо явно записать «комментарии исключаются» в §3.3/§6.
10. **`#message-request-banner` описан только прозой (`:466-468`).** Нет
    разметки, класса в §3.2 и правила показа/скрытия, хотя это интерактивная
    поверхность с двумя кнопками. Добавить фрагмент DOM + ARIA (`role="note"` или
    `status`) и стили, либо явно отдать его в §2.2 как часть `#dm-channel-list`.
11. **Непомеченное `[A]` в `CHANNEL_RECIPIENT_REMOVE`.** §1.5.2 (`:217`) удаляет
    канал, если `user.id == state.user.id`; в `DM-FRIENDS-RE.md §7.2` такого
    правила нет (там только `addRecipient` для приватных каналов). Пометить
    `[A]` или сослаться на RE, иначе метка `[D]` в шапке таблицы завышена.

## Механические проверки (независимо от текста спецификации)

- **Токены.** В §3.2 — 96 уникальных `var(--...)`; все 96 объявлены в
  `tokens.css`, все 96 строк таблицы §3.1 совпадают по имени и базовому значению
  из `:root`. `--motion-fast` дополнительно переопределяется на `0ms` в
  `@media (prefers-reduced-motion)`, как и указано в §3.1.
- **Запреты.** Сырых `#hex`/`rgb()`/`hsl()` в объявлениях — 0 (два вхождения —
  только в комментариях, см. P2.9); `!important` — 0; `:has()` — 0; скобки
  сбалансированы, 110 правил, 0 вложенных ошибок.
- **Специфичность.** `main.content[data-home-subview="friends"] #chat-header#chat-header`
  = `(2,2,1)`; `.content:has(#screen-chat.active) #chat-header` = `(2,2,0)`;
  `.screen.active` = `(0,2,0)`. Заявленный вывод верен. `[hidden]` из
  `primitives.css:64` даёт независимую страховку.
- **Контраст.** Пересчитаны все пары §3.2; минимум — `--text-faint` на
  `--bg-chat` 4.54:1, CTA — 5.58:1, `--text-on-accent` на `--accent` 4.61:1,
  white на `--danger-fill` 4.85:1 (hover 6.02:1), `--positive-text` на
  `--surface-4` 7.65:1, `--danger-text` на `--surface-2` 7.63:1. Порог 4.5:1
  выполняется везде; non-text индикаторы (`--positive` как рамка на
  `--surface-input` 4.91:1, `--danger` 3.73:1) — ≥ 3:1.
- **READY-оракул.** По `fixtures/gateway/by-type/002-READY.json`: 17
  `private_channels`, все `type=1`, у всех `recipient_ids.length=1`, 60 связей
  (18 типа 1, 42 типа 2), 146 пользователей, `id == user_id` у 60/60 — числа
  §1.1.3, §1.1.4 и `T-N107-READY-01` совпадают с данными.
- **`guild-rail.test.mjs`.** `Home` с плитки → `GUILDS[0]`, `End` → `GUILDS[6]`,
  `ArrowUp` с первой → последняя; §5.2 сохраняет это. `#home-rail-btn` — не
  `.guild-item`, поэтому `tiles()` и проверки `aria-current` не меняются;
  `data-guild-id` не попадает под селектор `[data-guild]`.

## Что проверить в Pass 3

1. Выбрать схему персистентности Home (P1 A или B), обновить `settings_store.rs`,
   `app.js:335-339`, `features/settings.js:209-212,281-285` и добавить round-trip
   оракул в `T-N107-HOME-NAV-01`.
2. Исправить тип/ключ `state.presences` и ссылки на `presence.rs`
   (`is_online` — 65-69, `Presence` — 268-278), переписать `get(id)` на
   `get(&(id, PresenceScope::Private))`.
3. Пересчитать таблицу §3.3 и убрать hex из комментария CSS (или оговорить
   исключение в оракуле).
4. Свести к одной формуле `aria-label` для `#home-rail-btn`,
   `#dm-nav-friends`, `#dm-nav-requests`; починить ссылку §2.1.4.
5. Задать начальный `hidden` для `#friends-view`, roving `tabindex` при Home,
   монтаж `#dm-quick-search-btn`/`.dm-nav-list` и `<link>` на
   `components/dm-friends.css`.

Проверка выполнена чтением назначенных исходников и fixture; спецификация, код,
тесты и golden-каталоги не изменялись. Единственный записанный файл — этот отчёт.
