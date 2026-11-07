# RW-40 / G3 — DM-FRIENDS-SPEC: личные сообщения, групповые DM и друзья (N1-07 / C2)

Статус: **дизайн-контракт, Pass 3 (спецификация G3 Design; закрыты все 7 P1 и 12 P2 из `waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW.md`, а также 1 P1 и 11 P2 из `waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW-PASS2.md`; продуктовый код и тесты не менялись)**.  
Дата: 2026-10-30.  
Владелец: **G3 Design**.  
Потребители: **G2 Frontend** (N1-07 UI, C2 DM-список и интеграция `dm-user` из MB-03 Profile Popover), **G1 Core** (REST-маршруты `/users/@me/channels*`, `/channels/{id}/recipients*`, `/users/@me/relationships*`, `/users/@me/notes/*`, события `CHANNEL_CREATE`, `CHANNEL_DELETE`, `CHANNEL_RECIPIENT_ADD`, `CHANNEL_RECIPIENT_REMOVE`, `RELATIONSHIP_ADD`, `RELATIONSHIP_UPDATE`, `RELATIONSHIP_REMOVE`, расширение `PrivateChannel` и `Relationship`), **G4 QA** (контрактные и UI-инварианты `T-N107-*`).

Нормативные источники:
- `waves/RW-40/DM-FRIENDS-RE.md` — полный RE-разбор 24 REST-маршрутов на 12 путях, схем `PrivateChannel` (`type: 1 | 3`) и `Relationship` (`RelationshipType 0..6`), живого захвата `002-READY.json` (17 `private_channels`, 60 `relationships`, 146 `users`) и 7 Gateway dispatch-событий из бандла `web.4d4b2f75fdea5470.js` (Discord `1.0.162`);
- `rust/gui-spike/ui/tokens.css` — единственный источник CSS custom properties;
- `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/style.css`, `rust/gui-spike/ui/components/{primitives,rail,tree}.css`, `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/features/notifications.js` — существующая DOM-разметка, стили, логика `#guild-rail` / `#channel-tree` и конвенция `guildId === "@me"`;
- `rust/crates/discord-core/src/ready/social.rs`, `rust/crates/discord-core/src/state/{types,presence}.rs` и `rust/crates/discord-core/src/dispatch.rs` — существующий разбор `READY.private_channels`, `READY.relationships`, `Presence` и роутер диспатчей;
- `tests/unit/guild-rail.test.mjs` и `tests/support/gui-dom-harness.mjs` — замороженные тесты рейла серверов и тестовый DOM-шим;
- `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` — матрица индикаторов непрочитанного/упоминаний (`.mention-badge`, `.is-unread`, `.is-muted`) и маршрут `PATCH /users/@me/guilds/@me/settings`;
- `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` — кнопка `button[data-action="dm-user"]` («Написать сообщение») в поповере профиля MB-03;
- `waves/RW-40/DESIGN/G05-REST-SPEC.md` — инварианты D1–D10, конечный автомат сетевых мутаций `idle → pending → ready | error` и посекундный таймер `429` (`retry_after_ms`);
- `waves/RW-40/THEME-SPEC.md` — правила контраста WCAG 2.2 AA.

---

## 0. Легенда, ограничения и архитектурные инварианты

### 0.1 Метки доказательности

| Метка | Значение | Источник |
|---|---|---|
| `[D]` | Доказано декодированным бандлом или живым захватом | `waves/RW-40/DM-FRIENDS-RE.md` (`/home/sanya/re-deep`: `decoded/4d2a64cb5df5fc4e_0`, `d38b37e4d52b4495_0`, `e1affb550dd48702_0`, `9f6dccfb41eb44b6_0`, `a8de0187058c21c2_0`, `002-READY.json`, `003-READY_SUPPLEMENTAL.json`) |
| `[S]` | Статическая схема маршрутов / Rust-типов | `/home/sanya/re-deep/rest-static.json`, `gateway-schema.md`, `rust/crates/discord-core/src/ready/social.rs` |
| `[T]` | Дизайн-токен проекта | `rust/gui-spike/ui/tokens.css` |
| `[F]` | Замороженный контракт существующего кода и тестов (включая расширяемые точки task-122: `settings_store.rs`, `readLastGuildId`, `normalizeSettings`/`getEffectiveLastGuildId`) | `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/features/settings.js`, `rust/gui-spike/src/settings_store.rs`, `tests/unit/guild-rail.test.mjs`, `tests/unit/gui-spike-*.test.mjs` |
| `[A]` | Проектное решение / адаптация DL (явно отделено от `[D]`) | Данный документ и `DM-FRIENDS-RE.md` §0 п.7, §8, §10 |
| `[?]` | Открытый вопрос RE (не подтверждено в дампе) | `DM-FRIENDS-RE.md` §3.2 (`consent_status` 4-е значение), §10 |

### 0.2 Базовые правила спецификации

1. **Строгий Token-only CSS**: каждое свойство `var(--...)` в CSS-блоках этого документа обязано существовать в `rust/gui-spike/ui/tokens.css`. Сырые цвета (`#hex`, `rgb(...)`, `rgba(...)`, `hsl(...)`, `hsla(...)`), псевдокласс `:has()` для управления видимостью шапки/экрана и `!important` в CSS-блоках запрещены (0 вхождений). Запрет распространяется и на **комментарии внутри CSS-блоков**: в нормативном CSS §3.2 нет ни одного `#hex` ни в объявлениях, ни в комментариях; пары контраста описываются именами токенов, а не их значениями.
2. **Контраст WCAG 2.2 AA (`THEME-SPEC.md` §2.2–2.6, порог ≥ `4.5:1` для обычного текста `< 24px` и ≥ `3.0:1` для non-text индикаторов)**:
   - `--warning` (`#fdb833`) используется **только как фон** в паре с `color: var(--text-on-warning)` (`#121214`, контраст `10.76:1`); использовать `--warning` как цвет текста или иконки запрещено.
   - `--danger-fill` (`#cf3a3f`) используется как фон бейджей (`.mention-badge`) и destructive-кнопок в паре с `color: var(--text-on-danger)` (`#ffffff`, контраст `4.85:1`; на hover `--danger-fill-hover` `#b92d33` — `6.02:1`); для текста ошибок на тёмных поверхностях используется `color: var(--danger-text)` (`#ff9a9e`, `7.24:1` на `--surface-3`); для декоративных рамок/точек — `var(--danger)` (`#da3e44`).
   - `--accent` (`#5865f2`) как фон используется только с `color: var(--text-on-accent)` (`#ffffff`, контраст `4.61:1`; на hover `--accent-hover` `#4654c0` — `6.38:1`, на active `--accent-active` `#3c45a5` — `8.12:1`); для акцентного текста на тёмных поверхностях — `color: var(--text-accent)`.
   - `--positive` (`#3d9e60`, `tokens.css:106`) **запрещено** сочетать с белым текстом `--text-on-accent` (`#ffffff`, контраст `3.36:1 < 4.5:1`) или `--text-strong` (`#fbfbfb`, `3.24:1 < 4.5:1`). На заливке `background: var(--positive)` (кнопка-вкладка `.friends-tab--add` с мелким текстом `14px/500`) обязательно используется тёмный токен текста `color: var(--text-on-warning)` (`#121214` из `tokens.css:91`, контраст **`5.58:1` ≥ `4.5:1` WCAG AA**). Для зелёного текста и иконок на тёмных поверхностях `--bg-chat` и `--surface-1..4` (например, `.friends-tab--add[aria-selected="true"]`, `.friend-action-btn--accept:hover`, `#add-friend-feedback[data-kind="success"]`) обязательно используется `color: var(--positive-text)` (`#57d98a`, контраст `9.66:1` на `--bg-chat` и `7.65:1` на `--surface-4`).
   - `--text-faint` (`#81828a`) допускается только на `--bg-app` (`4.89:1`) и `--bg-chat` (`4.54:1`, `tokens.css:86`); внутри карточек и модалок на `--surface-1..4` используются `--text-muted`, `--text-subtle`, `--text` или `--text-strong`.
3. **Сохранность замороженного каркаса `rust/gui-spike/ui/index.html` и явное управление состоянием контейнеров (`[F]`)**:
   - В `index.html` остаются ровно 4 элемента `.screen` (`#screen-chat`, `#screen-emoji`, `#screen-members`, `#screen-settings`), чтобы не нарушать инварианты `gui-spike-dom` и `features/screens.js`.
   - Режим «Личные сообщения и Друзья» (`Home`) связывается с каноническим идентификатором `HOME_GUILD_ID = "@me"` (`data-guild-id="@me"` на `#home-rail-btn`, совместимо с `features/notifications.js:1330,1384`).
   - Атрибуты навигации строго разделены между контейнерами (одно имя на узел):
     - `#channel-tree` несёт атрибут `data-nav-mode="guild" | "home"` (`"guild"` на старте по умолчанию);
     - `main.content` в режиме Home несёт атрибут `data-home-subview="friends" | "requests" | "dm"` (в режиме сервера `data-nav-mode="guild"` атрибут `data-home-subview` с `main.content` удаляется).
   - Скрытие `#chat-header` и `#screen-chat` при открытой панели `#friends-view` (`main.content[data-home-subview="friends"]`) выполняется **без использования `:has()`** двумя синхронными механизмами:
     1. **Явное состояние DOM-узлов из JS**: при переходе в `data-home-subview="friends"` контроллер ставит `hidden` (`aria-hidden="true"`) на `#chat-header`, снимает класс `.active` и ставит `aria-hidden="true"` на `#screen-chat` (благодаря чему правило `.content:has(#screen-chat.active) #chat-header` из `style.css:153` перестаёт матчиться, а `.screen[aria-hidden="true"]` из `style.css:127` скрывает `#screen-chat`), и снимает `hidden` (`aria-hidden="false"`) с `#friends-view`. При переходе к каналу ЛС (`data-home-subview="dm"`) или возврате в гильдию (`data-nav-mode="guild"`) контроллер ставит `hidden` (`aria-hidden="true"`) на `#friends-view`, снимает `hidden` (`aria-hidden="false"`) с `#chat-header` и возвращает `.active` + `aria-hidden="false"` на `#screen-chat` (если `body[data-active-screen="chat"]`).
     2. **Каскад CSS без `:has()`**: в `components/dm-friends.css` правило `main.content[data-home-subview="friends"] #chat-header#chat-header` имеет специфичность `(2, 2, 1)`, что выше `(2, 2, 0)` у `.content:has(#screen-chat.active) #chat-header` в `style.css:153` даже без учёта снятия `.active`.
   - Все динамические контейнеры и Home-узлы (`#friends-view`, `#message-request-banner`, `#group-dm-dialog`, `#stranger-confirm-dialog`, `#friend-context-menu`, `#dm-channel-list`, `#dm-quick-search-btn`, `.dm-nav-list`, `#dm-nav-requests-item`) монтируются идемпотентно из JS-модуля `features/dm-friends.js` (по образцу `#auth-gate` и `#join-server-dialog`), чтобы один код-путь работал и в браузере, и в DOM-шиме тестов (`tests/support/gui-dom-harness.mjs`) (`[F]`). Идемпотентность обязательна для каждого узла: повторный вызов монтажа не создаёт дублей и не теряет уже выставленное состояние (`hidden`, `aria-current`, `tabindex`).
4. **Персистентность режима Home (`[F]`-расширение G-01b, реализуется в задаче task-122)**. Режим навигации `Home ("@me")` обязан переживать перезапуск приложения. Контракт состоит из трёх согласованных слоёв:
   - **Rust `rust/gui-spike/src/settings_store.rs`**: `UiSettings` получает аддитивное поле `home_mode: Option<bool>` (`#[serde(default, rename = "homeMode", skip_serializing_if = "Option::is_none")]`; `None` в файле прежней версии трактуется как `false`). Все пять фильтров замороженного кода, которые сегодня отбрасывают `"@me"`, принимают его наравне со snowflake: два в Rust — `normalize_last_guild_id` (`:281-284`, используется и в `deserialize_last_guild_id`) и `UiSettings::validate` (`:202-211`); три в UI — `readLastGuildId` (`app.js:335-339`), `normalizeSettings` (`features/settings.js:209-212`) и `getEffectiveLastGuildId` (`features/settings.js:281-285`). Дополнительно `normalizeSettings` пробрасывает `homeMode` **только когда в источнике лежит boolean**: `...(typeof source.homeMode === "boolean" ? { homeMode: source.homeMode } : {})` — тот же приём, что уже применён к `lastGuildId` (`features/settings.js:209-212`). Не булево значение и отсутствие ключа **не добавляют** `homeMode` в результат (а не превращают его в `false`). Это условие обязательно из-за замороженного оракула `tests/unit/gui-spike-features.test.mjs` (`[F]`): путь диалога настроек `collectSettings()` (`features/settings.js:375-394`) → `normalizeSettings()` → `settings_save` собирает объект без `homeMode`, и payload обязан остаться ровно из пяти ключей (`density`, `locale`, `theme`, `toggles`, `version`; `lastGuildId` добавляется только при непустом значении). Режим Home при этом не теряется: в режиме Home `getEffectiveLastGuildId` возвращает `"@me"`, диалог сохраняет `lastGuildId: "@me"`, и boot-правило §2.1.1 п.3 (`homeMode === true` **или** `lastGuildId === "@me"`) восстанавливает Home на следующем запуске. `homeMode` отсутствует в файле или равен `false` → режим сервера (в Rust `None` трактуется как `false`). `SettingsStore::save` (`:518-524`) сохраняет оба поля без потери остальных полей `UiSettings`.
   - **Локальное хранилище UI**: активный режим навигации дублируется в `localStorage` по ключу `"discord-lab.homeNavMode"` со значениями `"home"` или `"guild"`; при недоступности `localStorage` (приватный режим, `SecurityError` при записи) используется `sessionStorage` с тем же ключом. Чтение и запись обёрнуты в `try/catch`: отказ хранилища не блокирует навигацию, но и не считается успешным сохранением.
   - **Память контроллера**: `createGuildRail`/контроллер Home держит поле `homeNavMode: "guild" | "home"` — единственный источник истины для текущего рендера; запись в `settings_save` и в `localStorage` выполняется только при `origin === "user"` (программный boot не перезаписывает выбор пользователя).
5. **Подключение нормативных стилей (`[F]`)**: файл `rust/gui-spike/ui/components/dm-friends.css` обязан быть подключён в `rust/gui-spike/ui/index.html` отдельной строкой `<link rel="stylesheet" href="components/dm-friends.css" />` последним в блоке стилей (текущий список — `index.html:9-22`, порядок каскада: `tokens.css` → `primitives.css` → `style.css` → `components/*`). Размещение после `components/tree.css` гарантирует, что правила §3.2 идут позже переопределяемых правил `style.css` и компонентов. Без этого `<link>` правила §3.2 не применяются, и `T-N107-TOKENS-01` обязан падать: оракул проверяет и наличие подключения, и содержимое файла.

---

## 1. Протокольный контракт и модель данных (RE Traceability)

### 1.1 Ключевые факты RE, обязательные к соблюдению

1. **`POST /users/@me/channels` всегда принимает массив `recipients`, а не `recipient_id` (`[D] DM-FRIENDS-RE.md §0 п.2, §2.1, @567585, @568882, @569410`)**:
   - Клиент нормализует вход через `_getRecipients: e => null != e ? Array.isArray(e) ? e : [e] : []` и **всегда** отправляет тело `{"recipients": ["<snowflake>", ...]}` — как для одиночной DM (`recipients.length === 1`), так и для групповой DM (`recipients.length >= 2`).
   - Поле `recipient_id` в `POST /users/@me/channels` **не существует** (негативный оракул `DM-FRIENDS-RE.md` §9). Единственный маршрут в контракте ЛС, принимающий `{"recipient_id": "<snowflake>"}`, — это `POST /users/@me/channels/group-dm-shell` (`[D] @568215`).
   - Для `recipients.length === 1` клиент сначала вызывает `this._openCachedDMChannel(userId)` (`[D] @567585`): если канал `type === 1` с этим единственным получателем (`channel.channel_type == 1 && channel.recipient_ids == [userId]`, предикат поиска в нашем сторе — `[A]`) уже есть в сторе, HTTP POST не вызывается, а сразу открывается существующий канал.
2. **`GET /users/@me/channels` клиентом не вызывается (`[D] DM-FRIENDS-RE.md §0 п.3, §1.3`)**:
   - В бандле `1.0.162` у `USER_CHANNELS` ровно **0** GET-вызовов и **2** POST-вызова (`rest-static.json`).
   - Начальный список ЛС приходит в `READY.d.private_channels` (`[D] §6.1`) и `READY_SUPPLEMENTAL.d.lazy_private_channels` (`[D] §6.3`), новые каналы поступают через Gateway `CHANNEL_CREATE` (`[D] §7.1`), а точечное получение существующего DM с пользователем — через `GET /users/@me/dms/{user_id}` (`[D] @569241`).
3. **Различие формата получателей в `READY` и в REST / `CHANNEL_CREATE` (`[D] DM-FRIENDS-RE.md §2.2, §5.1, §10, @4920048, @8899327`)**:
   - В сжатом `READY.d.private_channels[]` приходят только идентификаторы `recipient_ids: [snowflake]`. В живом захвате `002-READY.json` — 17 объектов, все `type: 1` (`DM`), у всех `recipient_ids.length === 1`, все резолвятся в `READY.d.users[]` из 146 пользователей (`[D]`). Групповых DM (`type: 3`) в живом захвате нет — форма `GROUP_DM` подтверждена кодом `eD.fromServer` (`[D] @8899327`), а примеры wire-объектов `GROUP_DM` помечены `[A]` (`DM-FRIENDS-RE.md` §5.1, §10).
   - В ответе `POST /users/@me/channels` и в диспатче `CHANNEL_CREATE` приходит поле `recipients: User[]` (массив **объектов пользователей**). Оригинальный клиент сортирует их детерминированно `eD.sortRecipients(e.recipients, e.id)` и извлекает идентификаторы `t.map(e => e.id)` (`[D] @8899327`).
   - **Инвариант нормализации DL (`[A]` по `DM-FRIENDS-RE.md` §5.1, подробно в §1.5.1)**: `PrivateChannel` хранит канонический `recipient_ids: Vec<u64>`, а при получении `recipients: User[]` из REST или `CHANNEL_CREATE` парсер/редуктор извлекает `id` каждого пользователя в `recipient_ids` и одновременно делает upsert каждого объекта `User` в `state.users`. В UI список объектов получателей всегда вычисляется как проекция `channel.recipient_ids` через `state.users`.
4. **Различие формата связей в `READY` и в REST / `RELATIONSHIP_*` (`[D] DM-FRIENDS-RE.md §4.1, §5.2, §6.2, §7.3`)**:
   - В `READY.d.relationships[]` приходят 7 полей: `id`, `user_id` (`id === user_id` у 60/60 записей захвата), `type` (`1` × 18 друзей, `2` × 42 заблокированных), `nickname` (`nil` у 60/60), `since` (ISO-8601), `is_spam_request` (`false`), `user_ignored` (`false`).
   - В `GET /users/@me/relationships` (`@699083`) и в диспатчах `RELATIONSHIP_ADD` / `RELATIONSHIP_UPDATE` (`@4931266`) вместо `user_id` приходит вложенный объект `user: User` (потребитель `D(t.user)` кладёт его в `UserStore`, `@9403042`), а также дополнительные поля `note`, `origin_application_id`, `is_stranger_request` и (у `RELATIONSHIP_ADD`) `should_notify`.
   - В `RELATIONSHIP_REMOVE` (`@4931266`, `@9322202`) стор читает минимум `id` и `user_ignored` (`user` опционален).
   - Побочный эффект стора (`[D] @9322065`): при `RELATIONSHIP_UPDATE`, если старый тип связи был `4` (`PENDING_OUTGOING`), а новый стал `1` (`FRIEND`), генерируется внутреннее событие `FRIEND_REQUEST_ACCEPTED` с объектом `user`.

### 1.2 Полный каталог REST-маршрутов DM и друзей (`[D] DM-FRIENDS-RE.md §1.1–§4.6`)

| # | Метод и путь | Ключ `Endpoints` (`Rsh`) | Смещение в бандле | Тело запроса / query | Ответ и реакция клиента |
|---:|---|---|---|---|---|
| 1 | `GET /users/@me/channels` | `USER_CHANNELS` | `@259640` | — | **0 call-site** `[D]`. Клиентом не вызывается (список из `READY.private_channels`) |
| 2 | `POST /users/@me/channels` | `USER_CHANNELS` | `@259640`, `@567585`, `@568882` | `{"recipients": ["<snowflake>", ...]}` (`retries: 3`, `oldFormErrors: true`) | HTTP `2xx` (`200/201` `[A]`): объект канала (`DM` `type:1` или `GROUP_DM` `type:3` с `recipients: User[]`) → локальный `CHANNEL_CREATE` и переход в канал `[D]` |
| 3 | `POST /users/@me/channels/group-dm-shell` | `USER_GROUP_DM_SHELL` | `@259640`, `@568215` | `{"recipient_id": "<snowflake>"}` (единственное место с `recipient_id`) | Объект пустой группы (shell) для последующего `PUT /channels/{id}/recipients/{user_id}` `[D]` |
| 4 | `POST /users/@me/channels/bulk-leave` | `USER_CHANNELS_BULK_LEAVE` | `@259640`, `@570274` | `{"channel_ids": ["<snowflake>", ...]}` | Выход из нескольких групп сразу `[D]` |
| 5 | `GET /users/@me/dms/{user_id}` | `DM_CHANNEL` | `@260280`, `@569241` | — | Объект канала DM с данным пользователем `[D]` |
| 6 | `PUT /channels/{id}/recipients/{user_id}` | `CHANNEL_RECIPIENT(e,t)` | `@270359`, `@570912` | — | **201**: сервер вернул новый канал (shell стал группой), клиент открывает `t.body.id`; **200/204**: канал прежний. Live-анонс `cU0t1F` («пользователь добавлен в группу»), при ошибке — `8GEdej` («не удалось добавить пользователя в группу») `[D]` |
| 7 | `DELETE /channels/{id}/recipients/{user_id}` | `CHANNEL_RECIPIENT(e,t)` | `@270359`, `@570912` | — | Удаление участника из `GROUP_DM` (доступно владельцу `owner_id`) `[D]` |
| 8 | `PUT /channels/{id}/recipients/@me` | `CHANNEL_RECIPIENT_ME(e)` | `@270359`, `e1affb…@311` | `{"consent_status": "ACCEPTED" \| "UNSPECIFIED" \| "PENDING"}` | Message request: при `"ACCEPTED"` диспатчит `MESSAGE_REQUEST_ACCEPT_OPTIMISTIC` `[D]` |
| 9 | `PATCH /channels/{id}/recipients/@me` | `CHANNEL_RECIPIENT_ME(e)` | `@270359`, `d38b37…@243430` | `{"flags": <int>}` (`RecipientFlags`: `1` NUX, `2` wallpaper) | Обновление битов `recipient_flags` `[D]` |
| 10 | `DELETE /channels/{id}/recipients/@me` | `CHANNEL_RECIPIENT_ME(e)` | `@270359`, `e1affb…@311` | — | Выход из `GROUP_DM` или отклонение одиночного message request `[D]` |
| 11 | `PUT /channels/recipients/@me/batch-reject` | `CHANNEL_RECIPIENT_REJECT_BATCH()` | `@270359`, `e1affb…@311` | `{"channel_ids": ["<snowflake>", ...]}` | Пакетное отклонение message requests `[D]` |
| 12 | `PATCH /channels/{id}` | `CHANNEL(e)` | `@266725`, `@571455..571778` | `{"name": "..."}` \| `{"icon": "..." \| null}` \| `{"owner": "<snowflake>"}` | Переименование, смена иконки или передача владения `GROUP_DM` `[D]` |
| 13 | `DELETE /channels/{id}?silent={bool}` | `CHANNEL(e)` | `@266725`, `@569652`, `@569942` | query `?silent=true\|false` | Закрытие ЛС; в оригинальном клиенте сопровождается локальным `CHANNEL_DELETE` и редиректом на `FRIENDS` (`@569942`) `[D]` |
| 14 | `GET /users/@me/relationships` | `USER_RELATIONSHIPS()` | `@258630`, `@699083` | — | Массив объектов связей (каждый с вложенным `user: User`) → `LOAD_RELATIONSHIPS_SUCCESS` / `LOAD_RELATIONSHIPS_FAILURE` `[D]` |
| 15 | `POST /users/@me/relationships` | `USER_RELATIONSHIPS()` | `@258630`, `@697487` | `{"username": l, "discriminator": parseInt(o), "note": a, ...captchaPayload}` | Отправка заявки по Discord Tag (`username#1234` или Pomelo `username` → `discriminator: null` при `JSON.stringify(NaN)`) `[D]` |
| 16 | `PUT /users/@me/relationships/{user_id}` | `USER_RELATIONSHIP(e)` | `@258630`, `@697776` | `{"type": a, "from_friend_suggestion": s, "confirm_stranger_request": l, ...captchaPayload}` | Принятие заявки / отправка по ID / блокировка (`type: 2`). В бандле `acceptFriendRequest` не передаёт `type` (`undefined`), шлёт `{"confirm_stranger_request": false}`; для паритета наш адаптер явно шлёт `type: 1` (`[D]`/`[A] §4.3`) |
| 17 | `DELETE /users/@me/relationships/{user_id}` | `USER_RELATIONSHIP(e)` | `@258630`, `@698771` | — | Единый маршрут для `removeFriend`, `cancelFriendRequest`, отклонения входящей заявки и `unblockUser` `[D]` |
| 18 | `PATCH /users/@me/relationships/{user_id}` | `USER_RELATIONSHIP(e)` | `@258630`, `@698967` | `{"nickname": string \| null}` | Установка или сброс личного псевдонима друга `[D]` |
| 19 | `PUT /users/@me/relationships/{user_id}/ignore` | `IGNORE_USER(e)` | `@310495`, `@699907` | — | Игнорировать пользователя → `RELATIONSHIP_IGNORE_USER_SUCCESS` `[D]` |
| 20 | `DELETE /users/@me/relationships/{user_id}/ignore` | `IGNORE_USER(e)` | `@310495`, `@700264` | — | Снять игнор с пользователя `[D]` |
| 21 | `DELETE /users/@me/relationships?relationship_type=3` | `USER_RELATIONSHIPS()` | `@258630`, `@699318` | query `?relationship_type=3` (без тела) | Очистить все входящие заявки (`PENDING_INCOMING`) → `RELATIONSHIP_PENDING_INCOMING_REMOVED` `[D]` |
| 22 | `DELETE /users/@me/relationships?relationship_type=3` | `USER_RELATIONSHIPS()` | `@258630`, `@699593` | query `?relationship_type=3`, тело `{"filters": [1, 2]}` | Очистить входящие заявки из `SPAM=1` и `IGNORED=2` (`clearPendingSpamAndIgnored`) `[D]` |
| 23 | `GET /users/@me/notes/{user_id}` | `NOTE(e)` | `@273990`, `9f6dcc…@493` | — | Ответ `{note_user_id, note, ...}`; **обязательная проверка** `e.note_user_id === userId`, иначе `Error("Invalid response from server")` → `USER_NOTE_UPDATE` `[D]` |
| 24 | `PUT /users/@me/notes/{user_id}` | `NOTE(e)` | `@273990`, `a8de01…@161` | `{"note": string}` | Сохранение личной заметки о пользователе (ошибка `50015 INVALID_NOTE`) `[D]` |

### 1.3 Перечисления, флаги и формулы счётчиков (`[D] DM-FRIENDS-RE.md §3.3, §4.7, §5.3`)

#### 1.3.1 Типы приватных каналов (`ChannelType`, `@11855584`, `@11856265`)
- `1` = `DM` (личная переписка 1-на-1, `guild_id: null`, `recipient_ids.length === 1`).
- `3` = `GROUP_DM` (групповая переписка, `guild_id: null`, дополнительные поля `name`, `icon`, `owner_id`, `nicks`; подтверждено кодом `eD.fromServer` `[D] @8899327`, в живом `002-READY.json` записей с `type == 3` нет `[A]`).
- Множество `PRIVATE_CHANNEL = {1, 3}`.

#### 1.3.2 Битовые флаги канала и получателя
- `ChannelFlags` (`@3250152`): для DM/GROUP_DM значимы `PINNED = 2` (`1 << 1`, закреплённый чат вверху списка ЛС) и `IS_SPAM = 32` (`1 << 5`, канал в папке спама запросов).
- `RecipientFlags` (`d38b37e4d52b4495_0@243430`, модуль `253506`):
  - `DISMISSED_IN_GAME_MESSAGE_NUX = 1`;
  - `DISMISSED_CURRENT_CHAT_WALLPAPER = 2`.

#### 1.3.3 Типы связей (`RelationshipType`, экспорт `eA$`, `@258005`)

| Код | Имя в клиенте (`eA$`) | Смысл в UI и фильтрации |
|---:|---|---|
| `0` | `NONE` | Связи нет |
| `1` | `FRIEND` | Друг (вкладка «В сети» при `presence.status.is_online()` и вкладка «Все») |
| `2` | `BLOCKED` | Заблокированный пользователь (вкладка «Заблокированные») |
| `3` | `PENDING_INCOMING` | Входящая заявка в друзья (вкладка «Ожидание», секция входящих) |
| `4` | `PENDING_OUTGOING` | Исходящая заявка в друзья (вкладка «Ожидание», секция исходящих) |
| `5` | `IMPLICIT` | Неявная связь (игровые/платформенные связи) |
| `6` | `SUGGESTION` | Рекомендация друзей |

*Примечание трассируемости*: в карточке задания значения `3` и `4` упоминаются как `INCOMING_REQUEST` / `OUTGOING_REQUEST` — в коде клиента Discord они именуются `PENDING_INCOMING` (`3`) и `PENDING_OUTGOING` (`4`) (`[D] DM-FRIENDS-RE.md §4.7`).

#### 1.3.4 Секции UI и формулы счётчиков `RelationshipStore` (`[D] @243700, @9318383, @9319095, @7346275`)
- Внутренние константы секций экрана друзей (`@243700`): `ONLINE`, `ALL`, `PENDING`, `PENDING_IGNORED`, `SPAM`, `SUGGESTIONS`, `ADD_FRIEND`, плюс фильтр `BLOCKED` (`type === 2`) для отображения заблокированных пользователей (`[D]`/`[A]`).
- Фильтры очистки входящих (`RelationshipFilters`, `@7346275`): `SPAM = 1`, `IGNORED = 2`.
- Правило `isIgnored(userId)` (`@9319095`): пользователь считается игнорируемым, если его `type !== 2` (`!BLOCKED`) **и** `ignoredSet.has(userId)` (`user_ignored === true`).
- Формулы счётчиков `RelationshipStore` (`@9318383`):
  ```text
  spamCount            = E.size   // входящие заявки (type === 3) с is_spam_request === true
  pendingIgnoredCount  = h.size   // входящие заявки (type === 3) с user_ignored === true
  spamAndIgnoredCount  = Set(E ∪ h).size // входящие заявки (type === 3) с (is_spam_request || user_ignored) — выводится в #friends-spam-count
  pendingCount         = Math.max((incomingSet.size ?? 0) - spamCount - pendingIgnoredCount, 0)
  outgoingCount        = outgoingSet.size   // type === 4
  friendCount          = friendSet.size     // type === 1
  blockedCount         = blockedSet.size    // type === 2
  messageRequestCount  = count(private_channels where is_message_request === true)
  ```
  Бейдж новых заявок на вкладке «Ожидание» (`#friends-pending-badge`) и пункте «Друзья» (`#dm-nav-friends-badge`) равен `pendingCount` (`[D] @9318383`), на пункте «Запросы переписки» (`#dm-nav-requests-badge`) — `messageRequestCount`, а на кнопке `#home-rail-btn` (`.guild-mention-badge`) — `pendingCount + dmMentionCount`. При значении `0` любой из этих четырёх бейджей обязательно получает атрибут `hidden` (`[A]`).

### 1.4 Таблица кодов ошибок REST (`[D] DM-FRIENDS-RE.md §2.3, §4.3, §4.8, @321000..332000, @696700`)

Коды `body.code`, имена констант `t02` и тип реакции клиента доказаны бандлом (`[D]`); точные русские формулировки сообщений для UI DL являются проектной локализацией (`[A]`). Коды карантина для каналов (`20026`) и для связей (`40068`) разнесены по своим контекстам:

| Код `body.code` / HTTP | Константа `t02` в бандле `[D]` | Контекст возникновения `[D]` | Реакция клиента `[D]` и нормативный русский текст UI в DL `[A]` |
|---:|---|---|---|
| `20026` | `QUARANTINED` (`@568148`, `@321019`) | Открытие DM (`POST /users/@me/channels`) или создание `group-dm-shell` с аккаунта на карантине | Вызвать модалку/баннер карантина и пробросить ошибку `[D]`; текст (`role="alert"`): «Действие недоступно: ваш аккаунт ограничен (карантин)» `[A]` |
| `40068` | `USER_QUARANTINED` (`@696700`, `@321000`) | Отправка заявки или мутация `/users/@me/relationships*` с аккаунта на карантине | Закрыть активную модалку и показать экран ограничения аккаунта `[D]`; текст (`role="alert"`): «Отправка запросов в друзья заблокирована из-за ограничений аккаунта» `[A]` |
| `30002` | `TOO_MANY_FRIENDS` | Превышен серверный лимит друзей | Показать ошибку в форме/статусе `[D]`: «Достигнут максимальный лимит друзей» `[A]` |
| `30004` | `TOO_MANY_RECIPIENTS` | Превышен серверный лимит участников группы | Показать ошибку в `#group-dm-error` `[D]`: «В этой группе уже максимальное число участников» `[A]` |
| `30059` | `TOO_MANY_BLOCKED_USERS` | Превышен лимит заблокированных | Показать ошибку `[D]`: «Достигнут лимит заблокированных пользователей» `[A]` |
| `30078` | `TOO_MANY_PENDING_OUTGOING` | Слишком много исходящих заявок | Показать ошибку `[D]`: «Слишком много ожидающих исходящих заявок» `[A]` |
| `40002` (HTTP 403) | `EMAIL_VERIFICATION_REQUIRED` (`@696700`) | Не подтверждена почта аккаунта | Открыть предупреждение о верификации почты `[D]`: «Для отправки заявок необходимо подтвердить адрес электронной почты» `[A]` |
| `40003` | `RATE_LIMIT_DM_OPEN` | Слишком частое открытие новых ЛС | Показать ошибку и заблокировать повтор на время кулдауна `[D]`: «Вы открываете личные сообщения слишком часто. Подождите немного» `[A]` |
| `40007` | `USER_BANNED` | Аккаунт заблокирован | Показать ошибку `[D]`: «Аккаунт заблокирован» `[A]` |
| `10013` | `UNKNOWN_USER` | Получатель не найден | Показать ошибку `[D]`: «Пользователь не найден» `[A]` |
| `50003` | `INVALID_ACTION_DM` | Недопустимое действие в ЛС (например, с ботом) | Показать ошибку `[D]`: «Невозможно выполнить это действие в личных сообщениях» `[A]` |
| `50015` | `INVALID_NOTE` | Некорректная длина/формат заметки (`PUT /users/@me/notes/{id}`) | Показать ошибку под полем заметки `[D]`: «Не удалось сохранить заметку: недопустимый формат или длина» `[A]` |
| `50035` | `INVALID_FORM_BODY` | Некорректное тело запроса (`recipients` / тег) | Показать ошибку валидации формы `[D]`: «Проверьте правильность введённых данных» `[A]` |
| `50096` / `80003` | `INVALID_CANNOT_FRIEND_SELF` / `RELATIONSHIP_INVALID_SELF` | Попытка добавить в друзья самого себя | Показать ошибку в `#add-friend-feedback` `[D]`: «Вы не можете добавить в друзья самого себя» `[A]` |
| `80001` | `RELATIONSHIP_INCOMING_BLOCKED` | Получатель запретил входящие заявки | Показать ошибку `[D]`: «Пользователь не принимает заявки в друзья» `[A]` |
| `80002` | `RELATIONSHIP_INVALUD_USER_BOT` | Заявка боту (опечатка имени константы в самом бандле) | Показать ошибку `[D]`: «Ботов нельзя добавлять в друзья» `[A]` |
| `80004` | `RELATIONSHIP_INVALID_DISCORD_TAG` | Пользователь с таким тегом не найден | Показать ошибку в `#add-friend-feedback` `[D]`: «Не удалось найти пользователя с таким именем. Проверьте регистр и написание» `[A]` |
| `80007` | `RELATIONSHIP_ALREADY_FRIENDS` | Уже в друзьях | Показать ошибку `[D]`: «Вы уже дружите с этим пользователем» `[A]` |
| `80013` | `RELATIONSHIP_INVALID_NO_CONFIRMATION` (`@6673300`) | Принятие заявки от незнакомца без общих серверов | Выставить `is_stranger_request = true`, открыть `#stranger-confirm-dialog` и после подтверждения повторить `PUT /users/@me/relationships/{id}` с `confirm_stranger_request: true` `[D]` |
| `340005` / `340006` / `340007` | `USER_*_LIMITED_ACCESS` | Ограничение доступа к вложениям DM/GDM или заявкам в друзья | Показать текст ограничения аккаунта `[D]`/`[A]` |
| HTTP `429` | Rate limit (`retry_after`, `@696700`) | Превышение частоты запросов | При `errorUxConfig === 0` (`SHOW_ALWAYS`) показать сообщение и запустить посекундный таймер `retry_after_ms` (`G05-REST-SPEC.md` §3.0); для фоновых вызовов (`SHOW_ONLY_IF_ACTION_NEEDED`) — без модалки `[D]` |

### 1.5 Расширение структур Rust, нормализация `recipients` и полный контракт редуктора (`G1 Core`, `DM-FRIENDS-RE.md` §5, §7, §8)

#### 1.5.1 Модели `PrivateChannel` и `Relationship` (`rust/crates/discord-core/src/ready/social.rs`)

Существующие структуры в `rust/crates/discord-core/src/ready/social.rs` расширяются обратно совместимо (новые поля `Option` / `Vec` / `BTreeMap` с дефолтами в `Default` и `parse_*`):

1. **`PrivateChannel` (`social.rs:138-159`)** — сохраняет существующие 10 полей (`id`, `channel_type`, `flags`, `recipient_ids: Vec<u64>`, `last_message_id`, `last_pin_timestamp`, `is_message_request`, `is_message_request_timestamp`, `is_spam`, `recipient_flags`) и дополняется 6 полями из `eD.fromServer` (`[D] @8899327`):
   - `pub name: Option<String>` — название групповой DM (`channel_type == 3`, пустая строка `""` нормализуется в `None` или хранится как `Some`);
   - `pub icon: Option<String>` — хеш иконки группы;
   - `pub owner_id: Option<u64>` — snowflake владельца группы;
   - `pub nicks: BTreeMap<u64, String>` — локальные ники участников в группе (`user_id → nick`);
   - `pub safety_warnings: Vec<String>` — предупреждения безопасности (`null`/отсутствие → пустой `Vec`);
   - `pub blocked_user_warning_dismissed: bool` — скрыто ли предупреждение о заблокированном участнике (дефолт `false`).
2. **Правило нормализации `recipients` ↔ `recipient_ids` + `state.users` (`[D] @4920048, @8899327`, `[A]`)**:
   - Внутри `PrivateChannel` единственным хранимым списком участников является `pub recipient_ids: Vec<u64>` (дублирование объектов `User` внутри `PrivateChannel` не используется, чтобы избежать рассинхрона с `state.users: BTreeMap<u64, User>`).
   - Если входной пакет (`READY.d.private_channels[]`, `READY_SUPPLEMENTAL.d.lazy_private_channels[]`) содержит `recipient_ids: [u64]`, парсер читает их напрямую в `channel.recipient_ids`.
   - Если входной пакет (`POST /users/@me/channels`, `GET /users/@me/dms/{id}`, `PUT /channels/{id}/recipients/{user_id}` со статусом `201`, Gateway `CHANNEL_CREATE`) содержит массив объектов `recipients: [UserMap]`:
     1. каждый элемент `recipients[i]` разбирается через `parse_user` и **upsert'ится в `state.users`** (`state.users.insert(user.id, user)`);
     2. массив `channel.recipient_ids` заполняется идентификаторами `user.id` из `recipients` с детерминированной сортировкой `sort_recipients(ids, channel.id)` (или сохранением порядка без дублей); если в пакете нет `recipients`, читается `recipient_ids`.
   - Для фронтенда (`gui-spike`) селектор/сериализатор канала отдаёт `recipientIds: string[]` и производный массив `recipients: User[]`, вычисленный как `channel.recipient_ids.iter().filter_map(|id| state.users.get(id)).collect()`.
3. **`Relationship` (`social.rs:163-178`)** — сохраняет существующие 7 полей (`id`, `user_id`, `relation_type`, `nickname`, `since`, `is_spam_request`, `user_ignored`) и дополняется 3 полями из `@4931266` / `@9318383`:
   - `pub note: Option<String>` — текст заметки;
   - `pub origin_application_id: Option<u64>` — ID приложения источника связи;
   - `pub is_stranger_request: bool` — флаг заявки от незнакомца (код `80013`, дефолт `false`).
   - **Нормализация `user` ↔ `user_id`**: в `READY` приходит плоский `user_id: u64`; в `GET /users/@me/relationships` и `RELATIONSHIP_ADD` / `RELATIONSHIP_UPDATE` / `RELATIONSHIP_REMOVE` может приходить вложенный объект `user: Map` вместо или вместе с `user_id`. Если `user` присутствует, он разбирается через `parse_user` и upsert'ится в `state.users`, а `relationship.user_id` вычисляется как `raw_user_id.or(parsed_user.id).unwrap_or(id)`. Если ни `user`, ни `user_id` не переданы (негативный кейс `RELATIONSHIP_ADD` / `RELATIONSHIP_REMOVE` без `user`), парсер **не возвращает ошибку**, а полагает `user_id = id` (так как в `READY` `id == user_id` у 60/60 записей, `[D] §6.2, §9`).

#### 1.5.2 Маршрутизация и таблица переходов редуктора для всех 7 событий (`dispatch.rs`, `state/reducer.rs`)

Чтобы не нарушать замороженный инвариант `DispatchKind::ALL.len() == 21` (`F-GW-06`, `T-GW-08` в `rust/crates/discord-core/src/dispatch.rs:67`), 7 событий DM и друзей регистрируются либо через расширение роутера по образцу `StreamDispatchKind` (`dispatch.rs:145-197`, `SocialDispatchKind::ALL` + включение в `DispatchEvent::is_known()`), либо в `EventPayload` редуктора `state/types.rs`.

Полная таблица переходов `state/reducer.rs` (закрывает `READY`, `READY_SUPPLEMENTAL` и все 7 событий `DM-FRIENDS-RE.md` §7.1–§7.4):

| Событие (`t`) | Входной payload (`d`) | Операция с `state.private_channels`, `state.relationships`, `state.users` | Идемпотентность и порядок мержа | Поведение при неизвестном `id` / отсутствии `user` | Побочные эффекты и счётчики |
|---|---|---|---|---|---|
| `READY` (`CONNECTION_OPEN`) | `d.private_channels[]`, `d.relationships[]`, `d.users[]` | Полная очистка `private_channels.clear()` и `relationships.clear()` (`[D] @9320846`), затем заполнение из `d`; `d.users[]` и `d.user` заполняют `state.users` | Повторный `READY` полностью заменяет обе коллекции без остатка старых ключей (`T-N107-READY-02`) | Битый элемент без `id` пропускается с записью в `diagnostics.malformed` (`social.rs:346,400`) | Полный пересчёт `friendCount`, `blockedCount`, `outgoingCount`, `spamCount`, `pendingIgnoredCount`, `pendingCount` |
| `READY_SUPPLEMENTAL` | `d.lazy_private_channels[]`, `d.merged_presences.friends[]` | Каждый канал из `lazy_private_channels` проходит upsert в `state.private_channels` по `id` (`[D] @4918565`); `merged_presences.friends` обновляет `state.presences` со скоупом `PresenceScope::Private` | Идемпотентный upsert по `channel.id` и `(user_id, Private)` | Пустой `lazy_private_channels: []` (как в `003-READY_SUPPLEMENTAL.json`) — валидный no-op | Обновляет онлайн-статусы друзей для вкладки «В сети» |
| `CHANNEL_CREATE` | Объект канала (`id`, `type`, `recipients?: User[]`, `recipient_ids?: u64[]`, `name?`, `icon?`, `owner_id?`, `nicks?`, …) | Если `type ∈ {1, 3}`: 1) все `u` из `recipients[]` upsert'ятся в `state.users`; 2) `recipient_ids` собирается из `recipients` (или `recipient_ids`); 3) канал вставляется/обновляется в `state.private_channels` по `id` (`[D] @4924901, @8899327`) | **Строго идемпотентен по `id`**: если канал уже создан локально после `POST /users/@me/channels`, повторный шлюзовой `CHANNEL_CREATE` мержит поля без дублей в `recipient_ids` | Если `recipients` и `recipient_ids` оба отсутствуют — `recipient_ids: Vec::new()`, канал создаётся без паники; если `type ∉ {1, 3}` — ветка `private_channels` не трогается | Пересчёт `messageRequestCount` (если `is_message_request == true`) |
| `CHANNEL_DELETE` | Объект канала (минимум `id`, опционально `guild_id`, `silent`) | Удаляет канал `state.private_channels.remove(&id)` (`[D] @4924901, @569942`) | **Идемпотентен**: повторный `CHANNEL_DELETE` после оптимистичного закрытия ЛС возвращает `None` | Если `id` нет в `state.private_channels` — **безопасный no-op** (0 ошибок, `state` не меняется) | Если удалённый канал был открыт в UI в режиме Home — переход на `#friends-view` (`[D] @569942`) и пересчёт `messageRequestCount` |
| `CHANNEL_RECIPIENT_ADD` | `{ channel_id: u64, user: User, nick: Option<String> }` | 1) `user` upsert'ится в `state.users`; 2) если `state.private_channels.get_mut(&channel_id)` существует (только приватные каналы `type ∈ {1, 3}`, `[D] @9022143`), `user.id` добавляется в `recipient_ids` (если отсутствует), а при `nick = Some(n)` пишется `nicks.insert(user.id, n)` | **Идемпотентен**: повторное событие для того же `(channel_id, user.id)` не дублирует `user.id` в `recipient_ids`, а только обновляет `nicks` | Если `channel_id` неизвестен (нет в `private_channels`) — **no-op** для каналов (`user` всё равно сохраняется в `state.users`); без паники | В UI обновляется заголовок/число участников `GROUP_DM` (`recipient_ids.len() + 1`) |
| `CHANNEL_RECIPIENT_REMOVE` | `{ channel_id: u64, user: User, nick: Option<String> }` | 1) Если `user` валиден — upsert в `state.users`; 2) в `state.private_channels.get_mut(&channel_id)` удаляется `user.id` из `recipient_ids` (`retain(\|&x\| x != user.id)`) и из `nicks.remove(&user.id)`; если `user.id == state.user.id`, канал удаляется из `private_channels` (**`[A]`**: правило выхода `@me` — проектное решение DL, в `DM-FRIENDS-RE.md` §7.2 для `CHANNEL_RECIPIENT_REMOVE` описан только `addRecipient`-эквивалент для приватных каналов; удаление канала подтверждается `[D] @569942` лишь для `CHANNEL_DELETE`) | **Идемпотентен**: повторное удаление отсутствующего `user.id` — no-op | Если `channel_id` неизвестен или `user.id` нет в `recipient_ids` — **безопасный no-op** | В UI обновляется состав `GROUP_DM` (или закрывается чат при выходе `@me`) |
| `RELATIONSHIP_ADD` | `{ id, type, user?: User, user_id?: u64, since?, nickname?, is_spam_request?, is_stranger_request?, user_ignored?, origin_application_id?, note?, should_notify? }` | 1) Если `user` присутствует — upsert в `state.users`; 2) `user_id = d.user_id.or(d.user.id).unwrap_or(d.id)`; 3) `state.relationships.insert(id, rel)` (`[D] @4931266`) | **Идемпотентный upsert по `id`**: перезаписывает запись актуальными полями | Если поле `user` отсутствует — разбор **не падает**, связь создаётся с `user_id = d.user_id.unwrap_or(d.id)` (`DM-FRIENDS-RE.md` §9) | Пересчёт всех счётчиков `RelationshipStore`; при `type == 3 && should_notify && !is_spam_request && !user_ignored` — анонс входящей заявки |
| `RELATIONSHIP_UPDATE` | `{ id, type, user?: User, user_id?: u64, nickname?, since?, is_spam_request?, is_stranger_request?, user_ignored?, origin_application_id?, note? }` | 1) Если `user` присутствует — upsert в `state.users`; 2) сохраняется `prev_type = state.relationships.get(&id).map(\|r\| r.relation_type)`; 3) upsert `state.relationships.insert(id, rel)` (`[D] @4931266, @9322065`) | **Идемпотентный upsert по `id`**: повтор с тем же `type` не генерирует дублирующее уведомление | Если `id` ранее отсутствовал в `state.relationships` — вставляется как новая запись (`prev_type = None`); отсутствие `user` не роняет разбор | Если `prev_type == Some(4)` (`PENDING_OUTGOING`) и новый `type == 1` (`FRIEND`), эмитится событие `FRIEND_REQUEST_ACCEPTED { user_id }` (`[D] @9322065`); пересчёт счётчиков |
| `RELATIONSHIP_REMOVE` | `{ id, user_ignored?: bool, user?: User, type?: i64 }` | Удаляет запись `state.relationships.remove(&id)` (`[D] @4931266, @9322202`) | **Идемпотентен**: повторное удаление того же `id` — no-op | Поле `user` необязательно; если `id` нет в `state.relationships` — **безопасный no-op** | Пересчёт всех счётчиков `RelationshipStore` |

---

## 2. Иерархия DOM и архитектура компонентов

### 2.1 Кнопка «Домой / Друзья» в `#guild-rail`, `guildId='@me'` и возврат в гильдию (`rust/gui-spike/ui/index.html`, `app.js`)

#### 2.1.1 Идентификатор `HOME_GUILD_ID = "@me"` и идемпотентное монтирование (`[F]`/`[A]`)

1. **Канонический идентификатор домашнего режима (`HOME_GUILD_ID = "@me"`)**:
   - Кнопка `#home-rail-btn` несёт атрибут `data-guild-id="@me"` и `data-action="nav-home"`.
   - Селектор `createGuildRail.selectedId()` (`rust/gui-spike/ui/app.js:412-415`) возвращает `"@me"`, если `#home-rail-btn` имеет класс `.is-selected`, либо `data-guild-id` выбранной плитки `button.guild-item.is-selected`.
   - Это напрямую связывает режим Home с уже существующей конвенцией `rust/gui-spike/ui/features/notifications.js:1330,1384` (`const isDm = !guildId || guildId === "@me"`), благодаря чему открытие настроек уведомлений в режиме Home обращается к `PATCH /users/@me/guilds/@me/settings` (без обёртки `guilds`), а не к последнему серверу.
   - При `guildId === "@me"` вызов IPC `invoke("channels", { guildId: "@me" })` **запрещён** (0 вызовов `channels` для `"@me"`); вместо этого `#channel-tree` рендерит приватные каналы из `state.private_channels`.
2. **Идемпотентное монтирование в браузере и тестовом DOM-шиме (`tests/support/gui-dom-harness.mjs`)**:
   - `ensureHomeNodes(root)` из `features/dm-friends.js` монтирует **весь** набор Home-узлов, если их ещё нет: `#dm-quick-search-btn` (в `.tree-header`), `.dm-nav-list` с `#dm-nav-friends` и `#dm-nav-requests-item`, `#dm-channel-list`, `#friends-view` и `#message-request-banner`. Повторный вызов возвращает существующие узлы и не дублирует их; частичный DOM (например, шим с уже созданным `.tree-header`) дополняется недостающими узлами.
   - На старте в `index.html:38` и в `tests/support/gui-dom-harness.mjs:805-808` внутри `#guild-rail` существует `<div class="brand" title="discord-lab" aria-hidden="true">DL</div>`, тогда как `.guild-list` в DOM-шиме до вызова `ensureList()` отсутствует.
   - Монтирование кнопки выполняется идемпотентной функцией `ensureHomeRailButton(rail)`:
     - если `byId("home-rail-btn")` уже существует, возвращается существующий узел (повторные вызовы не создают дублей);
     - иначе находится узел `rail.querySelector(".brand")` и преобразуется/заменяется на `<button id="home-rail-btn" class="brand home-rail-btn" type="button" data-guild-id="@me" data-action="nav-home">` (при отсутствии `.brand` кнопка вставляется перед `.guild-list` или перед стабильным якорем `.rail-sep`). Второй элемент `.brand` никогда не создаётся;
     - **обязательное снятие `aria-hidden` с кнопки**: при превращении `.brand` в `<button id="home-rail-btn">` атрибут `aria-hidden="true"` **обязательно удаляется с самой кнопки** (`removeAttribute("aria-hidden")`), так как фокусируемый контрол с `aria-hidden="true"` нарушает WAI-ARIA. Атрибут `aria-hidden="true"` переносится на дочерние декоративные `<span class="brand-monogram" aria-hidden="true">DL</span>` и `<span id="home-rail-badge" class="guild-mention-badge" aria-hidden="true" hidden>0</span>`.
3. **Начальное состояние (boot-режим), порядок источников и взаимное исключение выбора**:
   - **Порядок разрешения режима на boot** (первый источник, который даёт определённое значение, побеждает):
     1. `settings_load.homeMode === true` **или** `settings_load.lastGuildId === "@me"` → режим Home. `homeMode === false` при числовом `lastGuildId` → режим сервера с восстановлением этой гильдии.
     2. Иначе — `localStorage`/`sessionStorage["discord-lab.homeNavMode"]`: `"home"` → Home, `"guild"` → сервер.
     3. Иначе — режим сервера по умолчанию (`data-nav-mode="guild"`, первая гильдия `GUILDS[0]`).
     Хранилище применяется синхронно до `ready()`, чтобы не мигать гильдейским чатом; после ответа `settings_load` контроллер сверяет значения и **файл настроек важнее хранилища** (расхождение перезаписывает `localStorage`).
   - `readLastGuildId` (`app.js:335-339`) принимает как snowflake (`/^\d{1,20}$/`), так и строку `"@me"`; любое другое значение по-прежнему даёт `""` (гильдия не выдумывается). `settings_load` без поля `homeMode` (файл прежней версии) трактуется как `homeMode: false`.
   - В режиме сервера `createGuildRail` выбирает восстановленную (`lastGuildId`) или первую гильдию `GUILDS[0]` (`.is-selected`, `aria-current="true"`), а `#home-rail-btn` остаётся **невыбранной** (без `.is-selected` и без `aria-current`). Это на 100% сохраняет инварианты `tests/unit/guild-rail.test.mjs:176-178,365-409`.
   - Если источник даёт Home, либо пользователь кликнул/нажал `Enter`/`Space` на `#home-rail-btn`, либо вызван переход `openPrivateChannel`, активируется режим Home (`rail.selectedId === "@me"`, `homeNavMode = "home"`).
   - **Сохранение выбора (`origin === "user"` only)**: при выборе Home контроллер пишет `homeNavMode = "home"`, `localStorage["discord-lab.homeNavMode"] = "home"` и вызывает `settings_save` со всей структурой `UiSettings`: `{ ...normalizeSettings(raw), lastGuildId: "@me", homeMode: true }`. При выборе гильдии — `homeNavMode = "guild"`, `localStorage[...] = "guild"` и `settings_save` с `lastGuildId: "<guildId>"`, `homeMode: false`. Программные переключения на boot (`origin !== "user"`) ничего не сохраняют.
   - В `#guild-rail` действует строгое взаимное исключение: одновременно активен **либо** `#home-rail-btn` (`.is-selected`, `aria-current="page"`), **либо** ровно одна плитка `button.guild-item` (`.is-selected`, `aria-current="true"`).

Ниже показан снимок DOM `#guild-rail` **после переключения в режим Home (`guildId === "@me"`)**:

```html
<aside id="guild-rail" class="rail" role="navigation" aria-label="Серверы и экраны" data-state="ready">
  <!-- Кнопка «Домой / Личные сообщения и Друзья» (активна при guildId === "@me") -->
  <button
    id="home-rail-btn"
    class="brand home-rail-btn is-selected"
    type="button"
    data-guild-id="@me"
    data-action="nav-home"
    data-unread="true"
    data-pending-count="2"
    data-mention-count="0"
    title="Личные сообщения и Друзья"
    aria-label="Личные сообщения и Друзья, 2 входящих запроса в друзья"
    aria-current="page"
    tabindex="0"
  >
    <span class="brand-monogram" aria-hidden="true">DL</span>
    <span id="home-rail-badge" class="guild-mention-badge" aria-hidden="true">2</span>
  </button>

  <!-- Навигация по серверам (собирается createGuildRail в app.js; в режиме Home ни одна плитка не имеет is-selected / aria-current) -->
  <div class="guild-list" role="navigation" aria-label="Серверы">
    <button class="guild-item" type="button" data-guild-id="696738350706458685" aria-label="Сервер ITSKIDON PC BOOST" tabindex="0">IP</button>
    <button class="guild-add" type="button" data-action="add-server" aria-label="Добавить сервер" aria-haspopup="dialog" aria-expanded="false">+</button>
  </div>

  <div class="rail-sep" role="separator" aria-hidden="true"></div>
  <button class="rail-btn active" type="button" data-screen="chat" title="Чат 100k" aria-label="Чат" aria-current="page">💬</button>
  <button class="rail-btn" type="button" data-screen="emoji" title="Эмодзи-пикер" aria-label="Эмодзи">😀</button>
  <button class="rail-btn" type="button" data-screen="members" title="Участники 24k" aria-label="Участники">👥</button>
  <button class="rail-btn" type="button" data-screen="settings" title="Настройки" aria-label="Настройки">⚙️</button>
</aside>
```

#### 2.1.2 Инварианты переключения `Home ("@me")` ↔ `Guild` и восстановления `.tree-header`:
1. **Переход в Home (`select("@me", focus, origin)`)**:
   - Со всех плиток `.guild-item` в `.guild-list` снимаются `.is-selected` и `aria-current="true"`. Roving `tabindex` внутри `.guild-list` сохраняется явно: `tabindex="0"` остаётся у плитки, которая была активна до перехода в Home; если активной плитки не было (первый запуск, пустой `lastGuildId`) — у первой плитки `button.guild-item`. Все остальные плитки получают `tabindex="-1"`. Благодаря этому `Tab` с `#home-rail-btn` попадает в `.guild-list`, а не перепрыгивает сразу на `.rail-btn` (§5.2 п.1).
   - На `#home-rail-btn` выставляются `.is-selected` и `aria-current="page"`, `rail.selectedId` становится `"@me"`, `homeNavMode = "home"`.
   - На `#channel-tree` выставляется `data-nav-mode="home"` и `aria-label="Личные сообщения"`, на `.tree-scroll` — `aria-label="Навигация личных сообщений"`.
   - Внутри `.tree-header` штатные узлы гильдии `<h2 class="tree-guild">` и `<button data-action="guild-menu">` **не удаляются из DOM**, а скрываются атрибутом `hidden`, тогда как кнопка `#dm-quick-search-btn` становится видимой (с неё снимается `hidden`).
   - `channels("@me")` **не вызывается**; в `.tree-scroll` рендерится навигация Home (`#dm-nav-friends`, `#dm-nav-requests`, `#dm-channel-list`). Если до этого не был выбран конкретный DM-канал, открывается подрежим `main.content[data-home-subview="friends"]` (`#friends-view`).
   - **Сохранение режима (`origin === "user"` only)**: `settings_save` получает `{ ...normalizeSettings(raw), lastGuildId: "@me", homeMode: true }`, а `localStorage["discord-lab.homeNavMode"]` (или `sessionStorage` при отказе `localStorage`) получает значение `"home"`. Программный boot в Home (`origin !== "user"`) не перезаписывает файл и хранилище.
2. **Возврат из Home в сервер (`select(guildId, focus, origin)` при `guildId !== "@me"`)**:
   - С `#home-rail-btn` снимаются `.is-selected` и `aria-current="page"`; на выбранной плитке `.guild-item[data-guild-id="${guildId}"]` выставляются `.is-selected`, `aria-current="true"` и `tabindex="0"` (у остальных плиток — `tabindex="-1"`).
   - **Персистентность (`origin === "user"` only)**: `homeNavMode = "guild"`, `localStorage["discord-lab.homeNavMode"] = "guild"`, `settings_save` получает `lastGuildId: "<guildId>"` (строго цифровой snowflake) и `homeMode: false`. Программные восстановления на boot не пишут ни в файл, ни в хранилище.
   - **Восстановление `.tree-header`**: на `#dm-quick-search-btn` ставится `hidden`, а с `<h2 class="tree-guild">` и `<button data-action="guild-menu">` в `.tree-header` снимается `hidden` (если эти узлы отсутствовали, они идемпотентно воссоздаются в `.tree-header`), после чего `.tree-guild.textContent` обновляется именем выбранного сервера `guild.name` (`app.js:441-442`).
   - На `#channel-tree` выставляется `data-nav-mode="guild"` и `aria-label="Каналы сервера"`, на `.tree-scroll` — `aria-label="Список каналов"`. Соседние `#voice-panel` и `#user-bar` остаются в `#channel-tree` без изменений.
   - С `main.content` удаляется атрибут `data-home-subview`; на `#friends-view` ставится `hidden` (`aria-hidden="true"`), с `#chat-header` снимается `hidden` (`aria-hidden="false"`), с `#chat-add-recipients-btn` ставится `hidden`, а на `#screen-chat` (при `body[data-active-screen="chat"]`) восстанавливаются класс `.active` и `aria-hidden="false"`.
   - Вызывается `invoke("channels", { guildId })` и перерисовывается список каналов выбранной гильдии в `.tree-scroll`.
3. **Формула бейджа `#home-rail-badge` и `aria-label` кнопки `#home-rail-btn`**:
   - Число на `#home-rail-badge` (`.guild-mention-badge`) равно `totalBadge = pendingCount + dmMentionCount`, где `pendingCount = RelationshipStore.getPendingCount()` (`[D] @9318383`), а `dmMentionCount` — сумма `mention_count` по всем `private_channels` из `read_state`. При `totalBadge === 0` бейджу ставится атрибут `hidden`.
   - `aria-label` на `#home-rail-btn` формируется динамически по **единственной** формуле (она же используется в §5.1; других вариантов текста в документе нет). Части собираются с русским склонением через `pluralRu(n, one, few, many)`:
     - `pendingPart` = `${pendingCount} ${pluralRu(pendingCount, "входящий запрос", "входящих запроса", "входящих запросов")} в друзья` — только при `pendingCount > 0`;
     - `mentionPart` = `${dmMentionCount} ${pluralRu(dmMentionCount, "непрочитанное упоминание", "непрочитанных упоминания", "непрочитанных упоминаний")}` — только при `dmMentionCount > 0`;
     - `aria-label = parts.length > 0 ? "Личные сообщения и Друзья, " + parts.join(", ") : "Личные сообщения и Друзья"`.
   - Примеры точных строк: `0/0` → `"Личные сообщения и Друзья"`; `2/0` → `"Личные сообщения и Друзья, 2 входящих запроса в друзья"`; `0/2` → `"Личные сообщения и Друзья, 2 непрочитанных упоминания"`; `2/2` → `"Личные сообщения и Друзья, 2 входящих запроса в друзья, 2 непрочитанных упоминания"`. Те же правила склонения применяются к `#dm-nav-friends`, `#dm-nav-requests` и `#friends-tab-pending` (§2.2 п.1).

### 2.2 Дерево личных сообщений в `#channel-tree` при `data-nav-mode="home"`

Когда активен режим «Домой» (`#channel-tree[data-nav-mode="home"]`, `guildId === "@me"`), в `.tree-header` узлы `.tree-guild` и `[data-action="guild-menu"]` скрыты атрибутом `hidden` и показана кнопка быстрого поиска `#dm-quick-search-btn` (открывает `#quick-switcher`), а `.tree-scroll` содержит пункт навигации «Друзья» (`#dm-nav-friends`), пункт «Запросы переписки» (`#dm-nav-requests`, видимый при `messageRequestCount > 0`) и секцию «ЛИЧНЫЕ СООБЩЕНИЯ» с кнопкой `+` создания групповой DM.

**Контракт монтирования и начального состояния (`[F]`, §0.2 п.3).** Все узлы этого раздела (`#dm-quick-search-btn`, `.dm-nav-list`, `#dm-nav-friends`, `#dm-nav-requests-item`, `#dm-channel-list`) создаёт `features/dm-friends.js` идемпотентно: если узел уже есть в `index.html` или DOM-шиме, он переиспользуется, иначе создаётся в описанной позиции (кнопка поиска — в `.tree-header` вместо скрытых `.tree-guild`/`guild-menu`; `.dm-nav-list` — в `.tree-scroll` после `p.channel-note` и перед секцией `#dm-channel-list`; `#dm-nav-requests-item` — вторым `<li>` списка). До первого перехода в Home монтирование не должно показывать Home-разметку поверх гильдейского дерева, поэтому:
- `#dm-quick-search-btn`, `.dm-nav-list` и `#dm-nav-requests-item` создаются с атрибутом `hidden`; `hidden` снимается только при `#channel-tree[data-nav-mode="home"]` (а с `#dm-nav-requests-item` — дополнительно только при `messageRequestCount > 0`);
- `#dm-channel-list` создаётся пустым (`<ul>` без `<li>`), чтобы до загрузки `state.private_channels` не показывать демонстрационные строки из макета;
- повторный монтаж не сбрасывает уже выставленные `hidden`, `aria-current` и `tabindex`.

Ниже показан снимок DOM `#channel-tree` в режиме Home (демонстрационные строки `#dm-channel-list` — пример рендера, а не начальное состояние):

```html
<aside
  id="channel-tree"
  class="tree"
  role="navigation"
  aria-label="Личные сообщения"
  data-nav-mode="home"
  data-state="ready"
  aria-busy="false"
>
  <header class="tree-header dm-tree-header">
    <h2 class="tree-guild" hidden>ITSKIDON PC BOOST</h2>
    <button class="icon-btn" type="button" data-action="guild-menu" aria-label="Меню сервера" aria-haspopup="menu" aria-expanded="false" hidden>…</button>
    <button
      id="dm-quick-search-btn"
      class="dm-search-trigger"
      type="button"
      data-action="open-quick-switcher"
      aria-label="Найти или начать беседу"
      aria-haspopup="dialog"
    >
      <span class="dm-search-trigger-icon" aria-hidden="true">🔍</span>
      <span class="dm-search-trigger-text">Найти или начать беседу</span>
    </button>
  </header>

  <nav class="tree-scroll dm-tree-scroll" aria-label="Навигация личных сообщений">
    <p class="channel-note visually-hidden" role="status" aria-live="polite">Открыт раздел «Друзья»</p>

    <!-- Верхние навигационные пункты Home -->
    <ul class="dm-nav-list" role="list">
      <li>
        <button
          id="dm-nav-friends"
          class="channel-item dm-nav-item is-selected"
          type="button"
          data-action="open-friends-view"
          aria-label="Друзья, 2 входящих запроса в друзья"
          aria-current="page"
          tabindex="0"
        >
          <span class="channel-icon" aria-hidden="true">👥</span>
          <span class="channel-name">Друзья</span>
          <span id="dm-nav-friends-badge" class="mention-badge" aria-hidden="true">2</span>
        </button>
      </li>
      <!-- Показывается при messageRequestCount > 0 (при 0 на #dm-nav-requests-item и #dm-nav-requests-badge ставится hidden) -->
      <li id="dm-nav-requests-item">
        <button
          id="dm-nav-requests"
          class="channel-item dm-nav-item"
          type="button"
          data-action="open-message-requests"
          aria-label="Запросы переписки, 1 новый запрос"
          tabindex="-1"
        >
          <span class="channel-icon" aria-hidden="true">📨</span>
          <span class="channel-name">Запросы переписки</span>
          <span id="dm-nav-requests-badge" class="mention-badge" aria-hidden="true">1</span>
        </button>
      </li>
    </ul>

    <!-- Секция «ЛИЧНЫЕ СООБЩЕНИЯ» (или «ЗАПРОСЫ ПЕРЕПИСКИ» при выборе #dm-nav-requests) -->
    <section class="channel-group dm-channel-group" aria-labelledby="dm-group-heading">
      <div class="dm-group-header">
        <h3 id="dm-group-heading" class="channel-group-title dm-group-title">ЛИЧНЫЕ СООБЩЕНИЯ</h3>
        <button
          id="create-group-dm-btn"
          class="icon-btn dm-create-btn"
          type="button"
          data-action="open-create-group-dm"
          title="Создать групповой чат"
          aria-label="Создать личное сообщение или групповой чат"
          aria-haspopup="dialog"
          aria-expanded="false"
        >＋</button>
      </div>

      <ul id="dm-channel-list" class="channel-list dm-channel-list" role="list">
        <!-- 1. Одиночная DM (type = 1), закреплённая (flags & 2), с непрочитанным упоминанием -->
        <li class="dm-channel-row" data-channel-id="696738350706458685" data-channel-type="1" data-pinned="true">
          <button
            class="channel-item dm-channel-item is-unread has-mention"
            type="button"
            data-channel-id="696738350706458685"
            data-recipient-id="315473023476170753"
            data-openable="true"
            data-unread="true"
            data-mention-count="3"
            aria-label="Личное сообщение: AlexVance, В сети, закреплён, непрочитано, 3 упоминания"
            tabindex="-1"
          >
            <span class="avatar avatar--sm dm-avatar" style="--avatar-hue: 165" aria-hidden="true">
              AV
              <span class="status-dot" data-status="online"></span>
            </span>
            <span class="dm-item-meta">
              <span class="channel-name dm-item-name">AlexVance</span>
              <span class="dm-item-subtext">Играет в Rust</span>
            </span>
            <span class="dm-pin-indicator" title="Закреплённый чат" aria-hidden="true">📌</span>
            <span class="mention-badge" aria-hidden="true">3</span>
          </button>
          <button
            class="icon-btn dm-close-btn"
            type="button"
            data-action="close-dm"
            data-channel-id="696738350706458685"
            aria-label="Закрыть переписку с AlexVance"
            tabindex="-1"
          >✕</button>
        </li>

        <!-- 2. Групповая DM (type = 3) с кастомным именем и числом участников -->
        <li class="dm-channel-row" data-channel-id="712345678901234567" data-channel-type="3" data-owner-id="194374444821643266">
          <button
            class="channel-item dm-channel-item"
            type="button"
            data-channel-id="712345678901234567"
            data-openable="true"
            data-unread="false"
            aria-label="Групповой чат: Команда релиза, 4 участника"
            tabindex="-1"
          >
            <span class="avatar avatar--sm dm-avatar dm-avatar--group" style="--avatar-hue: 235" aria-hidden="true">КР</span>
            <span class="dm-item-meta">
              <span class="channel-name dm-item-name">Команда релиза</span>
              <span class="dm-item-subtext">4 участника</span>
            </span>
          </button>
          <button
            class="icon-btn dm-close-btn"
            type="button"
            data-action="leave-group-dm"
            data-channel-id="712345678901234567"
            aria-label="Покинуть группу «Команда релиза»"
            tabindex="-1"
          >✕</button>
        </li>
      </ul>
    </section>
  </nav>

  <!-- Штатные #voice-panel и #user-bar остаются на месте без изменений -->
</aside>
```

**Правила фильтрации «Запросов переписки», скрытия бейджей, сортировки и именования строк `#dm-channel-list` (`[D] @8899327, e1affb…@311`, `[A]`):**
1. **Раздел «Запросы переписки» (`#dm-nav-requests`, `data-action="open-message-requests"`) и скрытие бейджей при `0` (`[D]`/`[A]`)**:
   - Счётчик `messageRequestCount` равен числу каналов в `state.private_channels` с `is_message_request === true`.
   - **Правило `0 → hidden` для всех бейджей навигации**:
     - `#home-rail-badge`: скрывается атрибутом `hidden` при `pendingCount + dmMentionCount === 0`;
     - `#dm-nav-friends-badge`: показывает `pendingCount`; при `pendingCount === 0` получает атрибут `hidden`, а `aria-label` на `#dm-nav-friends` равен `"Друзья"`. При `pendingCount > 0` — `"Друзья, " + pendingCount + " " + pluralRu(pendingCount, "входящий запрос", "входящих запроса", "входящих запросов") + " в друзья"` (та же формула и склонение, что у `pendingPart` в §2.1.2 п.3);
     - `#dm-nav-requests-badge` и `<li id="dm-nav-requests-item">`: показывают `messageRequestCount`; при `messageRequestCount === 0` оба получают атрибут `hidden`, а `aria-label` на `#dm-nav-requests` равен `"Запросы переписки"`. При `messageRequestCount > 0` — `"Запросы переписки, " + messageRequestCount + " " + pluralRu(messageRequestCount, "новый запрос", "новых запроса", "новых запросов")`;
     - `#friends-pending-badge`: показывает `pendingCount`; при `pendingCount === 0` получает атрибут `hidden`, а `aria-label` на `#friends-tab-pending` равен `"Ожидание"`. При `pendingCount > 0` — `"Ожидание, " + pendingCount + " " + pluralRu(pendingCount, "входящий запрос", "входящих запроса", "входящих запросов") + " в друзья"`.
   - **Поведение при клике на `#dm-nav-requests`**:
     - Кнопка `#dm-nav-requests` получает `.is-selected` и `aria-current="page"`, заголовок секции `#dm-group-heading` переключается на `"ЗАПРОСЫ ПЕРЕПИСКИ"`, а список `#dm-channel-list` фильтруется по предикату `channel.is_message_request === true` (в обычном режиме `#dm-nav-friends` или при выборе обычного DM заголовок `#dm-group-heading` равен `"ЛИЧНЫЕ СООБЩЕНИЯ"`, а `#dm-channel-list` фильтруется по `!channel.is_message_request`).
     - При выборе конкретного канала-запроса (`is_message_request === true`) он открывается в `#screen-chat` (`main.content[data-home-subview="dm"]`), где над `#composer` отображается баннер запроса переписки (`#message-request-banner`) с кнопками:
       - «Принять» (`data-action="accept-message-request"`) → вызывает `PUT /channels/{id}/recipients/@me` с `{"consent_status": "ACCEPTED"}` и диспатчит `MESSAGE_REQUEST_ACCEPT_OPTIMISTIC` (`is_message_request = false`, канал переходит в основной список `ЛИЧНЫЕ СООБЩЕНИЯ`, `[D] e1affb…@311`);
       - «Отклонить» (`data-action="reject-message-request"`) → вызывает `DELETE /channels/{id}/recipients/@me` (`[D] e1affb…@311`), удаляя канал из `private_channels`.
2. **Детерминированная сортировка `#dm-channel-list` (`[A]`, проверяется `T-N107-SORT-01`)**:
   - Все отображаемые в `#dm-channel-list` приватные каналы (`type: 1 | 3`) сначала делятся на две корзины по биту закрепления `ChannelFlags.PINNED = 2` (`[D] @3250152`):
     1. закреплённые: `(channel.flags & 2) !== 0`;
     2. обычные: `(channel.flags & 2) === 0`.
   - Внутри **каждой** корзины (как среди закреплённых между собой, так и среди незакреплённых между собой) применяется единый строгий порядок по двум ключам (`[A]`):
     1. **Первичный ключ**: `BigInt(channel.last_message_id ?? 0)` **по убыванию** (каналы с сообщениями всегда располагаются выше каналов с `last_message_id === null`).
     2. **Вторичный ключ (tie-break при равных `last_message_id`, включая оба `null`)**: `BigInt(channel.id)` **по убыванию**.
3. **Отображаемое имя и подпись канала (строго через `channel.recipient_ids` и `state.users`, `§1.5.1`)**:
   - Для `type === 1` (`DM`): пусть `uid = channel.recipient_ids[0]`, `user = state.users.get(uid)`, `rel = state.relationships.get(uid)`. Отображаемое имя: `rel?.nickname || user?.global_name || user?.display_name || user?.username || String(uid)`.
   - Для `type === 3` (`GROUP_DM`): если `channel.name` непустое (`channel.name.trim() !== ""`) — `channel.name`; иначе — список имён получателей через запятую, вычисленный по `channel.recipient_ids` (отсортированным через `sortRecipients(channel.recipient_ids, channel.id)` по `[D] @8899327`):
     ```js
     channel.recipient_ids
       .map((uid) => channel.nicks?.[uid] || state.users.get(uid)?.global_name || state.users.get(uid)?.display_name || state.users.get(uid)?.username || String(uid))
       .join(", ")
     ```
     Подпись `.dm-item-subtext` для `GROUP_DM` выводит `${channel.recipient_ids.length + 1} участника(-ов)` (получатели из `recipient_ids` + текущий пользователь `@me`).

#### 2.2.1 Баннер запроса переписки (`#message-request-banner`, `[D] e1affb…@311`)

Баннер — интерактивная поверхность над `#composer` в `#screen-chat`, которая показывается только когда открыт приватный канал с `is_message_request === true` в режиме Home (`main.content[data-home-subview="dm"]`). Он монтируется идемпотентно из `features/dm-friends.js` **с начальным `hidden` и `aria-hidden="true"`** и не появляется до первого запроса переписки.

```html
<div
  id="message-request-banner"
  class="message-request-banner"
  role="note"
  aria-labelledby="message-request-banner-text"
  hidden
  aria-hidden="true"
>
  <p id="message-request-banner-text" class="message-request-banner-text">
    Это запрос переписки. Примите его, чтобы отправлять сообщения.
  </p>
  <div class="message-request-banner-actions">
    <button
      class="group-dm-btn group-dm-btn--secondary"
      type="button"
      data-action="reject-message-request"
      aria-label="Отклонить запрос переписки"
    >Отклонить</button>
    <button
      class="group-dm-btn group-dm-btn--primary"
      type="button"
      data-action="accept-message-request"
      aria-label="Принять запрос переписки"
    >Принять</button>
  </div>
</div>
```

**Контракт показа, ARIA и состояний:**
1. **Видимость**: `hidden` снимается (`aria-hidden="false"`) тогда и только тогда, когда одновременно `main.content[data-home-subview="dm"]`, открытый канал имеет `is_message_request === true` и `#message-request-banner` уже смонтирован. При переходе в `#friends-view`, в канал без запроса или в гильдию баннеру ставится `hidden` и `aria-hidden="true"`.
2. **ARIA**: `role="note"` (не `status`: внутри есть фокусируемые кнопки), имя — `aria-labelledby="message-request-banner-text"`; `#message-request-banner-text` — обычный `<p>` без `aria-live` (баннер не анонсируется повторно на каждый рендер). Обе кнопки — нативные `<button>` с явным `aria-label`; порядок табуляции — «Отклонить», затем «Принять»; при `data-state="pending"` на баннере обе кнопки получают `disabled` и `aria-busy="true"`.
3. **Действия**: «Принять» (`data-action="accept-message-request"`) → `PUT /channels/{id}/recipients/@me` с `{"consent_status": "ACCEPTED"}` и `MESSAGE_REQUEST_ACCEPT_OPTIMISTIC` (`is_message_request = false`); «Отклонить» (`data-action="reject-message-request"`) → `DELETE /channels/{id}/recipients/@me` ([D] e1affb…@311). В обоих случаях по `2xx` баннер скрывается (`hidden`), фокус переводится на `#composer` (`#message-input`), а `messageRequestCount` пересчитывается.
4. **CSS**: класс `.message-request-banner` и его потомки описаны в §3.2 (блок 9); видимость управляется только атрибутом `hidden` (глобальное `[hidden] { display: none !important; }` из `components/primitives.css:64` плюс явное правило §3.2), без `:has()` и без inline-стилей.

### 2.3 Панель «Друзья» в `.content` (`#friends-view`)

Когда в `#channel-tree[data-nav-mode="home"]` выбран пункт «Друзья» (`#dm-nav-friends`), на `main.content` выставляется атрибут `data-home-subview="friends"`, на `#chat-header` и `#screen-chat` выставляется `aria-hidden="true"` (с `#screen-chat` также снимается `.active`, на `#chat-header` ставится `hidden`), а секция `#friends-view` отображается внутри `main.content`.

**Начальное состояние панели (`[F]`, закрывает риск показа поверх гильдейского чата).** `features/dm-friends.js` монтирует `#friends-view` **с `hidden` и `aria-hidden="true"`**: boot по умолчанию — режим сервера (§2.1.1 п.3), поэтому `.friends-view { display: flex }` из §3.2 не должен срабатывать до первого перехода в Home. Контроллер снимает `hidden` (`aria-hidden="false"`) только в обработчике `open-friends-view`/`select("@me")`; при возврате в гильдию и при открытии канала ЛС панель снова получает `hidden` и `aria-hidden="true"`. Снимок ниже показывает панель **после** снятия `hidden`; в разметке атрибуты сохранены, чтобы монтаж «как в макете» не открывал панель сам по себе.

```html
<section
  id="friends-view"
  class="friends-view"
  role="region"
  aria-label="Друзья"
  data-active-tab="online"
  data-state="ready"
  hidden
  aria-hidden="true"
>
  <!-- 1. Верхняя панель (49px, --layout-header-h): заголовок и вкладки фильтров -->
  <header class="friends-header">
    <div class="friends-header-brand">
      <span class="friends-header-icon" aria-hidden="true">👥</span>
      <h1 class="friends-header-title">Друзья</h1>
    </div>
    <div class="friends-header-divider" role="separator" aria-hidden="true"></div>

    <div
      id="friends-tablist"
      class="friends-tablist"
      role="tablist"
      aria-orientation="horizontal"
      aria-label="Фильтры списка друзей"
    >
      <button
        id="friends-tab-online"
        class="friends-tab"
        type="button"
        role="tab"
        data-tab="online"
        aria-selected="true"
        aria-controls="friends-tabpanel"
        tabindex="0"
      >В сети</button>

      <button
        id="friends-tab-all"
        class="friends-tab"
        type="button"
        role="tab"
        data-tab="all"
        aria-selected="false"
        aria-controls="friends-tabpanel"
        tabindex="-1"
      >Все</button>

      <button
        id="friends-tab-pending"
        class="friends-tab"
        type="button"
        role="tab"
        data-tab="pending"
        aria-label="Ожидание, 2 входящих запроса в друзья"
        aria-selected="false"
        aria-controls="friends-tabpanel"
        tabindex="-1"
      >
        <span>Ожидание</span>
        <span id="friends-pending-badge" class="mention-badge" aria-hidden="true">2</span>
      </button>

      <button
        id="friends-tab-blocked"
        class="friends-tab"
        type="button"
        role="tab"
        data-tab="blocked"
        aria-selected="false"
        aria-controls="friends-tabpanel"
        tabindex="-1"
      >Заблокированные</button>

      <button
        id="friends-tab-add"
        class="friends-tab friends-tab--add"
        type="button"
        role="tab"
        data-tab="add_friend"
        aria-selected="false"
        aria-controls="friends-add-panel"
        tabindex="-1"
      >Добавить в друзья</button>
    </div>

    <div class="friends-header-actions">
      <button
        class="icon-btn"
        type="button"
        data-action="open-create-group-dm"
        title="Новый групповой чат"
        aria-label="Новый групповой чат"
        aria-haspopup="dialog"
      >💬</button>
    </div>
  </header>

  <!-- 2. Рабочая область панели друзей -->
  <div class="friends-body">
    <!-- 2A. Панель списков (для вкладок online / all / pending / blocked).
         По APG tabpanel НЕ имеет tabindex="0", так как внутри есть фокусируемый #friends-search-input.
         Атрибут aria-labelledby динамически обновляется на id активной вкладки (friends-tab-online | all | pending | blocked). -->
    <div
      id="friends-tabpanel"
      class="friends-tabpanel"
      role="tabpanel"
      aria-labelledby="friends-tab-online"
    >
      <div class="friends-toolbar">
        <search class="search-field friends-search-field" role="search">
          <input
            id="friends-search-input"
            class="search-input"
            type="search"
            placeholder="Поиск"
            aria-label="Поиск по списку друзей"
            autocomplete="off"
          />
        </search>
        <!-- Дополнительный фильтр Спам/Игнорируемые и кнопка массовой очистки (только во вкладке pending) -->
        <div id="friends-pending-controls" class="friends-pending-controls" hidden>
          <button
            id="friends-toggle-spam"
            class="friends-subfilter-btn"
            type="button"
            data-action="toggle-pending-spam"
            aria-pressed="false"
          >Спам и игнор (<span id="friends-spam-count">0</span>)</button>
          <button
            id="friends-clear-pending-btn"
            class="friends-subfilter-btn friends-subfilter-btn--danger"
            type="button"
            data-action="clear-pending-requests"
          >Очистить входящие</button>
        </div>
      </div>

      <!-- Счётчик секции (например, «В СЕТИ — 12», «ВСЕ ДРУЗЬЯ — 18», «ОЖИДАНИЕ — 3») -->
      <h2 id="friends-section-heading" class="friends-section-heading" aria-live="polite">
        В СЕТИ — <span id="friends-section-count">12</span>
      </h2>

      <!-- Список карточек связей -->
      <ul id="friends-list" class="friends-list" role="list" aria-labelledby="friends-section-heading">
        <!-- Вариант А: Друг (RelationshipType = 1, вкладки online / all) -->
        <li
          class="friend-row"
          data-user-id="194374444821643266"
          data-relationship-type="1"
          data-status="online"
        >
          <div class="friend-row-main" data-action="open-dm" data-user-id="194374444821643266" tabindex="0" role="button" aria-label="AlexVance (Ник: Алекс), В сети. Нажмите, чтобы открыть переписку">
            <span class="avatar avatar--md" style="--avatar-hue: 165" aria-hidden="true">
              AV
              <span class="status-dot" data-status="online"></span>
            </span>
            <div class="friend-meta">
              <div class="friend-name-line">
                <span class="friend-display-name">Алекс</span>
                <span class="friend-username-tag">alexvance</span>
              </div>
              <span class="friend-status-text">В сети</span>
            </div>
          </div>
          <div class="friend-actions" role="group" aria-label="Действия с AlexVance">
            <button
              class="icon-btn friend-action-btn"
              type="button"
              data-action="open-dm"
              data-user-id="194374444821643266"
              title="Написать сообщение"
              aria-label="Написать сообщение AlexVance"
            >💬</button>
            <button
              class="icon-btn friend-action-btn"
              type="button"
              data-action="friend-more-menu"
              data-user-id="194374444821643266"
              title="Ещё"
              aria-label="Ещё действия для AlexVance"
              aria-haspopup="menu"
              aria-expanded="false"
            >⋮</button>
          </div>
        </li>

        <!-- Вариант Б: Входящая заявка в друзья (RelationshipType = 3, вкладка pending) -->
        <li
          class="friend-row"
          data-user-id="315473023476170753"
          data-relationship-type="3"
          data-stranger="true"
        >
          <div class="friend-row-main">
            <span class="avatar avatar--md" style="--avatar-hue: 42" aria-hidden="true">NK</span>
            <div class="friend-meta">
              <div class="friend-name-line">
                <span class="friend-display-name">NovaKnight</span>
                <span class="friend-username-tag">novaknight#4821</span>
              </div>
              <span class="friend-status-text">Входящий запрос в друзья</span>
            </div>
          </div>
          <div class="friend-actions" role="group" aria-label="Запрос в друзья от NovaKnight">
            <button
              class="icon-btn friend-action-btn friend-action-btn--accept"
              type="button"
              data-action="accept-friend"
              data-user-id="315473023476170753"
              title="Принять"
              aria-label="Принять запрос в друзья от NovaKnight"
            >✓</button>
            <button
              class="icon-btn friend-action-btn friend-action-btn--danger"
              type="button"
              data-action="decline-friend"
              data-user-id="315473023476170753"
              title="Игнорировать / Отклонить"
              aria-label="Отклонить запрос в друзья от NovaKnight"
            >✕</button>
          </div>
        </li>

        <!-- Вариант В: Исходящая заявка в друзья (RelationshipType = 4, вкладка pending) -->
        <li
          class="friend-row"
          data-user-id="418829102938475610"
          data-relationship-type="4"
        >
          <div class="friend-row-main">
            <span class="avatar avatar--md" style="--avatar-hue: 280" aria-hidden="true">ZR</span>
            <div class="friend-meta">
              <div class="friend-name-line">
                <span class="friend-display-name">ZeroRay</span>
                <span class="friend-username-tag">zeroray</span>
              </div>
              <span class="friend-status-text">Исходящий запрос в друзья</span>
            </div>
          </div>
          <div class="friend-actions" role="group" aria-label="Исходящий запрос к ZeroRay">
            <button
              class="icon-btn friend-action-btn friend-action-btn--danger"
              type="button"
              data-action="cancel-friend-request"
              data-user-id="418829102938475610"
              title="Отменить"
              aria-label="Отменить запрос в друзья к ZeroRay"
            >✕</button>
          </div>
        </li>

        <!-- Вариант Г: Заблокированный пользователь (RelationshipType = 2, вкладка blocked) -->
        <li
          class="friend-row"
          data-user-id="519920192837465123"
          data-relationship-type="2"
        >
          <div class="friend-row-main">
            <span class="avatar avatar--md" style="--avatar-hue: 12" aria-hidden="true">SP</span>
            <div class="friend-meta">
              <div class="friend-name-line">
                <span class="friend-display-name">SpamBot99</span>
                <span class="friend-username-tag">spambot99#0001</span>
              </div>
              <span class="friend-status-text">Заблокирован</span>
            </div>
          </div>
          <div class="friend-actions" role="group" aria-label="Действия с заблокированным SpamBot99">
            <button
              class="icon-btn friend-action-btn friend-action-btn--danger"
              type="button"
              data-action="unblock-user"
              data-user-id="519920192837465123"
              title="Разблокировать"
              aria-label="Разблокировать SpamBot99"
            >🔓</button>
          </div>
        </li>
      </ul>

      <!-- Пустое состояние вкладки -->
      <div id="friends-empty-state" class="friends-empty-state" role="status" hidden>
        <p id="friends-empty-text" class="friends-empty-text">Сейчас никого из друзей нет в сети</p>
      </div>
    </div>

    <!-- 2B. Панель «Добавить в друзья» (вкладка add_friend, POST /users/@me/relationships) -->
    <div
      id="friends-add-panel"
      class="friends-add-panel"
      role="tabpanel"
      aria-labelledby="friends-tab-add"
      hidden
    >
      <h2 class="friends-add-title">ДОБАВИТЬ В ДРУЗЬЯ</h2>
      <p id="friends-add-desc" class="friends-add-desc">
        Вы можете добавить друзей по их имени пользователя Discord (например, <code>username</code> или <code>username#1234</code>).
      </p>

      <form id="add-friend-form" class="add-friend-form" novalidate>
        <div class="add-friend-input-wrap" data-state="idle">
          <input
            id="add-friend-input"
            class="add-friend-input"
            type="text"
            placeholder="Вы можете добавить друзей по их имени пользователя Discord."
            aria-label="Имя пользователя Discord"
            aria-describedby="friends-add-desc add-friend-feedback"
            autocomplete="off"
            required
          />
          <button
            id="add-friend-submit"
            class="add-friend-submit"
            type="submit"
            disabled
            aria-disabled="true"
          >Отправить запрос дружбы</button>
        </div>
        <p id="add-friend-feedback" class="add-friend-feedback" role="status" aria-live="polite" hidden></p>
      </form>
    </div>
  </div>
</section>
```

**Правило разбора и сериализации `add-friend-form` (`[D] DM-FRIENDS-RE.md §4.2, @697487` + локальная предвалидация `[A]`):**
- Входная строка `discordTag = input.value.trim()` разбивается `const [username, rawDisc] = discordTag.split("#")` (`[D] @697487`).
- В оригинальном бандле `sendRequest` напрямую передаёт `discriminator: parseInt(o)` (`[D] @697487`). В UI DL до сетевого вызова применяется дополнительная локальная предвалидация (`[A]`): если символ `#` присутствует, но суффикс `rawDisc` пуст или не состоит из 4 цифр (`!/^\d{4}$/.test(rawDisc)`), форма показывает локальную ошибку в `#add-friend-feedback` без отправки HTTP-запроса.
- Для классического тега `shadowbyte#1234` `Number.parseInt(rawDisc, 10)` даёт число `1234` (`[D]`).
- Для Pomelo-аккаунтов (ввод без `#`, `rawDisc === undefined`) `parseInt(undefined)` даёт `NaN`, что при `JSON.stringify` сериализуется в `"discriminator": null` (`[D] §4.2`). В REST уходит:
  ```json
  { "username": "shadowbyte", "discriminator": null }
  ```
  или для классического тега `shadowbyte#1234`:
  ```json
  { "username": "shadowbyte", "discriminator": 1234 }
  ```

### 2.4 Модальное окно создания групповой DM (`#group-dm-dialog`) и кнопка в `#chat-header`

Открывается тремя триггерами `button[data-action="open-create-group-dm"]`:
1. кнопкой `#create-group-dm-btn` в заголовке секции `ЛИЧНЫЕ СООБЩЕНИЯ` дерева `#channel-tree` (§2.2);
2. кнопкой в `.friends-header-actions` шапки `#friends-view` (§2.3);
3. кнопкой `#chat-add-recipients-btn` («Добавить друзей в ЛС»), которую `features/dm-friends.js` идемпотентно монтирует в `#chat-header .chat-actions` (`rust/gui-spike/ui/index.html:105-113`) перед кнопкой `[data-action="members"]`:
   ```html
   <button
     id="chat-add-recipients-btn"
     class="icon-btn"
     type="button"
     data-action="open-create-group-dm"
     title="Добавить друзей в ЛС"
     aria-label="Добавить друзей в ЛС"
     aria-haspopup="dialog"
     aria-expanded="false"
     hidden
   >👤＋</button>
   ```
   Инвариант видимости `#chat-add-recipients-btn`: кнопка видима (`hidden` снят) **только** когда открыт приватный канал (`type: 1 | 3`) в режиме `guildId === "@me"` (`main.content[data-home-subview="dm"]`); при возврате в любой гильдейский канал (`guildId !== "@me"`) на `#chat-add-recipients-btn` обязательно ставится атрибут `hidden`.

> **Примечание о лимите получателей (`[A]` vs `[D] DM-FRIENDS-RE.md §0 п.7, §2.3, §10`):**  
> В самом JS-бандле `1.0.162` функции `openPrivateChannel` (`@567585`) и `addRecipient` (`@570912`) не содержат захардкоженной константы лимита, а превышение на стороне сервера возвращает код `30004 TOO_MANY_RECIPIENTS` (`[D]`). В продуктовом UI модалки выбора друзей действует клиентский потолок `MAX_GROUP_DM_RECIPIENTS = 9` выбираемых друзей (до **10 участников** в группе включая `@me`, либо до `9 - currentRecipients.length` при добавлении в существующую группу) (`[A]`). При выборе 9 друзей чекбоксы остальных невыбранных строк получают `aria-disabled="true"`, а при ответе сервера `30004` выводится явная ошибка в `#group-dm-error`.

```html
<div
  id="group-dm-dialog"
  class="dialog-backdrop"
  role="presentation"
  hidden
>
  <div
    class="group-dm-modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="group-dm-title"
    aria-describedby="group-dm-subtitle"
    data-state="idle"
  >
    <header class="group-dm-header">
      <div class="group-dm-title-wrap">
        <h2 id="group-dm-title" class="group-dm-title">Выберите друзей</h2>
        <p id="group-dm-subtitle" class="group-dm-subtitle" aria-live="polite">
          Вы можете добавить ещё <span id="group-dm-remaining">9</span> друзей.
        </p>
      </div>
      <button
        class="icon-btn"
        type="button"
        data-action="close-group-dm-modal"
        aria-label="Закрыть окно создания чата"
      >✕</button>
    </header>

    <!-- Поле фильтрации и чипы выбранных получателей -->
    <div class="group-dm-tagbox" role="group" aria-label="Выбранные получатели и фильтр">
      <ul id="group-dm-chips" class="group-dm-chips" role="list" aria-label="Выбранные друзья">
        <li class="group-dm-chip" data-user-id="194374444821643266">
          <span class="group-dm-chip-name">AlexVance</span>
          <button
            class="group-dm-chip-remove"
            type="button"
            data-action="remove-group-dm-recipient"
            data-user-id="194374444821643266"
            aria-label="Убрать AlexVance из списка получателей"
          >✕</button>
        </li>
      </ul>
      <!-- aria-expanded динамический: "true" при наличии видимых опций в #group-dm-friend-list, "false" при пустом результате фильтрации -->
      <input
        id="group-dm-search"
        class="group-dm-search"
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-autocomplete="list"
        aria-controls="group-dm-friend-list"
        aria-activedescendant="group-dm-opt-194374444821643266"
        aria-label="Введите имя пользователя друга"
        placeholder="Введите имя пользователя друга"
        autocomplete="off"
      />
    </div>

    <!-- Список друзей (RelationshipType === 1) с множественным выбором.
         По APG combobox + aria-activedescendant опции role="option" НЕ имеют tabindex="0" (фокус остаётся в #group-dm-search). -->
    <div
      id="group-dm-friend-list"
      class="group-dm-friend-list"
      role="listbox"
      aria-multiselectable="true"
      aria-labelledby="group-dm-title"
    >
      <div
        id="group-dm-opt-194374444821643266"
        class="group-dm-friend-option"
        role="option"
        data-user-id="194374444821643266"
        aria-selected="true"
      >
        <span class="avatar avatar--sm" style="--avatar-hue: 165" aria-hidden="true">
          AV
          <span class="status-dot" data-status="online"></span>
        </span>
        <span class="group-dm-friend-meta">
          <span class="group-dm-friend-name">AlexVance</span>
          <span class="group-dm-friend-tag">alexvance</span>
        </span>
        <span class="group-dm-checkbox" aria-hidden="true">✓</span>
      </div>
    </div>

    <p id="group-dm-error" class="group-dm-error" role="alert" hidden></p>

    <footer class="group-dm-footer">
      <button
        class="group-dm-btn group-dm-btn--secondary"
        type="button"
        data-action="close-group-dm-modal"
      >Отмена</button>
      <button
        id="group-dm-submit"
        class="group-dm-btn group-dm-btn--primary"
        type="button"
        data-action="submit-group-dm"
      >Создать групповой чат</button>
    </footer>
  </div>
</div>
```

**Два сценария отправки `#group-dm-submit` (`[D] @567585, @568215, @570912`):**
1. **Создание нового чата из `#channel-tree` или `#friends-view`**:
   - Если выбран `1` друг (`recipients.length === 1`), подпись кнопки — «Создать ЛС»; при отправке вызывается `openPrivateChannel({ recipientIds: [id] })` → кэш или `POST /users/@me/channels` с `{"recipients": [id]}`.
   - Если выбрано `2..9` друзей (`recipients.length >= 2`), подпись кнопки — «Создать групповой чат»; вызывается `POST /users/@me/channels` с `{"recipients": [id1, id2, ...]}`.
2. **Добавление участников в существующий приватный канал (`channelId`, открыто через `#chat-add-recipients-btn`)**:
   - Если текущий канал `type === 3` (`GROUP_DM`), для каждого выбранного `userId` последовательно вызывается `PUT /channels/{channelId}/recipients/{userId}` (`addRecipient`, `[D] @570912`).
   - Если текущий канал `type === 1` (`DM`) или создаётся через пустой shell `POST /users/@me/channels/group-dm-shell` (`{"recipient_id": firstUserId}`, `[D] @568215`), первый `PUT /channels/{id}/recipients/{user_id}` возвращает статус **201** с новым объектом группового канала, и клиент переключается на `t.body.id`.

### 2.5 Диалог подтверждения заявки от незнакомца (`#stranger-confirm-dialog`, код `80013`) и контекстное меню друга (`#friend-context-menu`)

По `[D] DM-FRIENDS-RE.md §4.3 (@6673300)`, если при `PUT /users/@me/relationships/{user_id}` сервер отвечает кодом `80013` (`RELATIONSHIP_INVALID_NO_CONFIRMATION`), клиент диспатчит `UPDATE_STRANGER_STATUS { userId, isStranger: true }` и открывает диалог подтверждения:

```html
<div id="stranger-confirm-dialog" class="dialog-backdrop" role="presentation" hidden>
  <div
    class="group-dm-modal stranger-confirm-modal"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="stranger-confirm-title"
    aria-describedby="stranger-confirm-desc"
  >
    <h2 id="stranger-confirm-title" class="group-dm-title">Принять запрос в друзья?</h2>
    <p id="stranger-confirm-desc" class="group-dm-subtitle">
      У вас нет общих серверов или друзей с этим пользователем. Вы уверены, что хотите принять запрос?
    </p>
    <footer class="group-dm-footer">
      <button class="group-dm-btn group-dm-btn--secondary" type="button" data-action="cancel-stranger-confirm">Отмена</button>
      <button class="group-dm-btn group-dm-btn--primary" type="button" data-action="accept-stranger-confirm">Принять запрос</button>
    </footer>
  </div>
</div>

<!-- Контекстное меню друга / участника группы (открывается кнопкой ⋮ [data-action="friend-more-menu"] или ПКМ / Shift+F10) -->
<div id="friend-context-menu" class="menu friend-context-menu" role="menu" aria-label="Действия с пользователем" hidden>
  <button class="menu-item" type="button" role="menuitem" data-action="friend-edit-nick" tabindex="0">Изменить никнейм друга</button>
  <button class="menu-item" type="button" role="menuitem" data-action="friend-edit-note" tabindex="-1">Изменить заметку</button>
  <button class="menu-item" type="button" role="menuitem" data-action="friend-toggle-ignore" tabindex="-1">Игнорировать</button>
  <div class="rail-sep" role="separator" aria-hidden="true"></div>
  <button class="menu-item menu-item--danger" type="button" role="menuitem" data-action="friend-remove" tabindex="-1">Удалить из друзей</button>
  <button class="menu-item menu-item--danger" type="button" role="menuitem" data-action="friend-block" tabindex="-1">Заблокировать</button>
</div>
```

**Полный контракт действий всех 5 пунктов `#friend-context-menu` (`[D] DM-FRIENDS-RE.md §4.3–§4.6`):**
1. **`data-action="friend-edit-nick"` («Изменить никнейм друга»)**:
   - Открывает мини-диалог/инлайн-форму редактирования псевдонима (`input[name="nickname"]`, предзаполненную текущим `relationship.nickname ?? ""`).
   - При сохранении вызывает `PATCH /users/@me/relationships/{user_id}` с телом `{"nickname": value.trim() || null}` (`updateRelationship`, `[D] @698967`). По ответу `2xx` (и/или `RELATIONSHIP_UPDATE`) обновляет `relationship.nickname` в сторе и перерисовывает карточку друга и строку DM.
2. **`data-action="friend-edit-note"` («Изменить заметку»)**:
   - При открытии редактора заметки запрашивает актуальную заметку через `GET /users/@me/notes/{user_id}` (`[D] 9f6dcc…@493`).
   - **Обязательная проверка ответа (`T-N107-NOTE-01`)**: клиент обязан проверить `response.body.note_user_id === userId` (`[D] 9f6dcc…@493`). Если `note_user_id !== userId`, выбрасывается `Error("Invalid response from server")`, стор **не мутируется** (`USER_NOTE_UPDATE` не диспатчится), и пользователю выводится ошибка. При совпадении диспатчится `USER_NOTE_UPDATE { id: userId, note: response.body.note }`.
   - При сохранении заметки вызывается `PUT /users/@me/notes/{user_id}` с телом `{"note": value}` (`updateNote`, `[D] a8de01…@161`). При ошибке `50015` (`INVALID_NOTE`) текст ошибки выводится под полем заметки и редактор остаётся открытым.
3. **`data-action="friend-toggle-ignore"` («Игнорировать» / «Снять игнор»)**:
   - Перед открытием меню текст пункта синхронизируется с `isIgnored(userId)` (`relationship.relation_type !== 2 && relationship.user_ignored === true`, `[D] @9319095`):
     - если `!isIgnored(userId)` — подпись «Игнорировать»; активация вызывает `PUT /users/@me/relationships/{user_id}/ignore` (без тела, `[D] @699907`) и по `2xx` диспатчит `RELATIONSHIP_IGNORE_USER_SUCCESS { userId }` (`user_ignored = true`);
     - если `isIgnored(userId)` — подпись «Снять игнор»; активация вызывает `DELETE /users/@me/relationships/{user_id}/ignore` (без тела, `[D] @700264`) и по `2xx` снимает `user_ignored = false`.
4. **`data-action="friend-remove"` («Удалить из друзей»)**:
   - Вызывает `DELETE /users/@me/relationships/{user_id}` (`removeRelationship`, `[D] @698771`). До ответа `2xx` строка остаётся в `data-state="pending"`; после `2xx` (или `RELATIONSHIP_REMOVE`) запись удаляется из `state.relationships`.
5. **`data-action="friend-block"` («Заблокировать»)**:
   - Вызывает `PUT /users/@me/relationships/{user_id}` с телом `{"type": 2}` (`RelationshipType.BLOCKED = 2`, `addRelationship`, `[D] @697776`). По `2xx` (или `RELATIONSHIP_UPDATE` / `RELATIONSHIP_ADD`) связь переходит в `relation_type = 2` (вкладка «Заблокированные»).

---

## 3. Дизайн-токены и нормативный CSS (`components/dm-friends.css`)

### 3.1 Полная таблица привязки к `rust/gui-spike/ui/tokens.css`

Каждый из 96 токенов в нормативном CSS-блоке §3.2 проверен на побайтовое совпадение имени и значения с `rust/gui-spike/ui/tokens.css` (1:1 соответствие между §3.1 и §3.2):

| Токен `tokens.css` | Значение в `tokens.css` | Где используется в компонентах DM и Друзей |
|---|---|---|
| `--bg-app` | `#121214` | Фон `#guild-rail`, `#channel-tree`, рамка `.guild-mention-badge` |
| `--bg-chat` | `#1a1a1e` | Фон `#friends-view`, рамка `.status-dot` |
| `--surface-1` | `#202024` | Фон `.group-dm-friend-list`, hover строки `.friend-row` |
| `--surface-2` | `#242429` | Фон `.group-dm-modal`, `.dm-search-trigger`, вторичных кнопок |
| `--surface-3` | `#28282d` | Фон `.friend-context-menu`, `.friend-action-btn` |
| `--surface-4` | `#2c2d32` | Hover `.friend-action-btn`, чипов `.group-dm-chip` |
| `--surface-input` | `#1e1f22` | Фон `.add-friend-input-wrap`, `.group-dm-tagbox` |
| `--text-strong` | `#fbfbfb` | Заголовки, имена друзей, активные вкладки, непрочитанные ЛС |
| `--text` | `#efeff1` | Основной текст строк, полей ввода и кнопок |
| `--text-subtle` | `#abacb2` | Вторичные подписи, неактивные вкладки при hover |
| `--text-muted` | `#96979e` | Статусы друзей, подписи участников группы, пустые состояния |
| `--text-faint` | `#81828a` | Заголовок `ЛИЧНЫЕ СООБЩЕНИЯ` **только на `--bg-app`** (`4.89:1`) и `В СЕТИ — N` **только на `--bg-chat`** (`4.54:1`, запас 0.04 над порогом; `tokens.css:86`) |
| `--text-on-accent` | `#ffffff` | Текст на `--accent`, `--accent-hover`, `--accent-active` |
| `--text-on-warning` | `#121214` | Тёмный текст на заливке `--positive` (`#3d9e60`, контраст `5.58:1` ≥ `4.5:1` AA, `tokens.css:91`) в кнопке `.friends-tab--add` |
| `--text-on-danger` | `#ffffff` | Текст на `--danger-fill` (`#cf3a3f`, `4.85:1`) и `--danger-fill-hover` (`#b92d33`, `6.02:1`) |
| `--accent` | `#5865f2` | Выбранная кнопка `#home-rail-btn`, `.group-dm-btn--primary`, `.add-friend-submit`, выбранный чекбокс |
| `--accent-hover` | `#4654c0` | Hover primary-кнопок |
| `--accent-active` | `#3c45a5` | Active primary-кнопок |
| `--accent-soft` | `rgb(88 101 242 / 16%)` | Выделенная опция в `.group-dm-friend-option[aria-selected="true"]` и `.friends-subfilter-btn[aria-pressed="true"]` |
| `--positive` | `#3d9e60` | Фон вкладки `.friends-tab--add` (с текстом `--text-on-warning`, `5.58:1`) и рамка успешной отправки |
| `--positive-text` | `#57d98a` | Текст активной вкладки `.friends-tab--add[aria-selected="true"]` на `--bg-chat` (`9.66:1`), `#add-friend-feedback[data-kind="success"]` и иконка принятия `✓` на `--surface-4` (`7.65:1`) |
| `--danger` | `#da3e44` | Рамка `.add-friend-input-wrap[data-state="error"]` |
| `--danger-text` | `#ff9a9e` | Текст ошибок `#add-friend-feedback[data-kind="error"]`, `#group-dm-error`, `.menu-item--danger` |
| `--danger-fill` | `#cf3a3f` | Фон `.mention-badge` и destructive-кнопок при hover |
| `--danger-fill-hover` | `#b92d33` | Active destructive-кнопок |
| `--border-subtle` | `rgb(151 151 159 / 12%)` | Рамки `.add-friend-input-wrap`, `.group-dm-tagbox` |
| `--border` | `rgb(151 151 159 / 20%)` | Разделители шапки `.friends-header-divider` |
| `--border-strong` | `rgb(151 151 159 / 44%)` | Рамка чекбокса `.group-dm-checkbox` |
| `--divider` | `rgb(255 255 255 / 6%)` | Разделители строк `.friend-row` и секций модалки |
| `--state-hover` | `rgb(255 255 255 / 6%)` | Hover строк ЛС и вкладок |
| `--state-active` | `rgb(255 255 255 / 9%)` | Active строк и кнопок |
| `--state-selected` | `rgb(255 255 255 / 11%)` | Выбранная строка `.dm-channel-item[aria-current="page"]` |
| `--state-selected-strong` | `rgb(255 255 255 / 14%)` | Активная вкладка `.friends-tab[aria-selected="true"]` |
| `--state-disabled-opacity` | `0.45` | Disabled-контролы и заглушённые каналы |
| `--focus-ring` | `#a5b0ff` | Контур `:focus-visible` |
| `--focus-ring-width` | `2px` | Толщина контура `:focus-visible` и обводки бейджа |
| `--focus-ring-offset` | `2px` | Отступ контура `:focus-visible` |
| `--font-mono` | `ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", "DejaVu Sans Mono", monospace` | Примеры тегов `code` в описании добавления друзей |
| `--font-size-caption` | `11px` | Подписи `.dm-item-subtext`, бейджи |
| `--font-size-small` | `12px` | Заголовки секций, подзаголовки |
| `--font-size-ui` | `14px` | Вкладки, кнопки, строки списков |
| `--font-size-body` | `16px` | Имена друзей и заголовки панелей |
| `--font-size-title` | `20px` | Заголовок модалки `#group-dm-title` |
| `--line-height-caption` | `14px` | Межстрочный интервал `.dm-item-subtext` |
| `--line-height-small` | `16px` | Межстрочный интервал заголовков секций |
| `--line-height-ui` | `18px` | Межстрочный интервал UI |
| `--line-height-body` | `22px` | Межстрочный интервал `.friend-display-name` |
| `--line-height-title` | `24px` | Межстрочный интервал `#group-dm-title` |
| `--font-weight-normal` | `400` | Обычный текст |
| `--font-weight-medium` | `500` | Вкладки и кнопки |
| `--font-weight-semibold` | `600` | Имена, заголовки секций |
| `--font-weight-bold` | `700` | Числовые бейджи |
| `--letter-spacing-caps` | `0.02em` | Верхний регистр заголовков секций |
| `--space-0` | `0` | Сброс отступов |
| `--space-1` | `2px` | Микро-зазоры |
| `--space-2` | `4px` | Внутренние отступы чипов и пилюль |
| `--space-3` | `6px` | Зазоры в строках |
| `--space-4` | `8px` | Базовый шаг отступов |
| `--space-5` | `12px` | Горизонтальные отступы карточек |
| `--space-6` | `16px` | Отступы секций |
| `--space-7` | `20px` | Высота индикатора выбранного элемента рейла |
| `--space-8` | `24px` | Отступы панелей |
| `--space-10` | `40px` | Высота строки опции в модалке |
| `--space-11` | `48px` | Высота строки `.add-friend-input-wrap` |
| `--radius-xs` | `4px` | Скругление строк и чипов |
| `--radius-sm` | `8px` | Скругление кнопок, полей, карточек |
| `--radius-md` | `12px` | Скругление модалки `.group-dm-modal` |
| `--radius-round` | `2147483647px` | Круглые кнопки действий `.friend-action-btn` и бейджи |
| `--shadow-elevation-low` | `0 1px 0 rgb(0 0 0 / 20%), 0 1.5px 0 rgb(0 0 0 / 4%), 0 2px 0 rgb(0 0 0 / 4%)` | Нижняя тень `.friends-header` |
| `--shadow-popover` | `var(--shadow-elevation-high), var(--shadow-elevation-stroke)` | Тень `.group-dm-modal` и `.friend-context-menu` |
| `--layout-header-h` | `49px` | Высота `.friends-header` |
| `--layout-panel-pad` | `16px` | Горизонтальный отступ `.friends-header` и `.friends-tabpanel` |
| `--layout-content-max-w` | `720px` | Максимальная ширина формы `.friends-add-panel` |
| `--row-h-member` | `42px` | Высота строки `.dm-channel-item` (с аватаром и подписью статуса) |
| `--row-h-group-start` | `48px` | Минимальная высота строки `.friend-row` |
| `--row-menu-item` | `32px` | Высота пункта `.menu-item` |
| `--icon-size-sm` | `16px` | Размер малых иконок и чекбокса |
| `--icon-size` | `20px` | Размер стандартных иконок |
| `--btn-height-sm` | `28px` | Высота малых кнопок и `.dm-search-trigger` |
| `--btn-height` | `32px` | Высота вкладок `.friends-tab` и кнопок |
| `--btn-height-lg` | `38px` | Высота кнопок футера модалки |
| `--btn-pad-x` | `16px` | Горизонтальный отступ кнопок |
| `--btn-radius` | `var(--radius-sm)` | Радиус кнопок |
| `--badge-h` | `16px` | Высота `.mention-badge` |
| `--badge-pad-x` | `4px` | Горизонтальный отступ `.mention-badge` |
| `--menu-min-w` | `188px` | Минимальная ширина `.friend-context-menu` |
| `--menu-pad` | `8px` | Внутренний отступ `.friend-context-menu` |
| `--menu-border` | `1px` | Толщина рамок и разделителей |
| `--menu-radius` | `var(--radius-sm)` | Радиус `.friend-context-menu` |
| `--modal-w-sm` | `442px` | Ширина `.group-dm-modal` |
| `--modal-pad-x` | `24px` | Горизонтальный отступ `.group-dm-modal` |
| `--modal-pad-y` | `16px` | Вертикальный отступ `.group-dm-modal` |
| `--z-popover` | `100` | Слой `.friend-context-menu` |
| `--z-modal` | `200` | Слой `#group-dm-dialog` и `#stranger-confirm-dialog` |
| `--motion-fast` | `120ms` | Длительность hover/focus переходов |
| `--ease-out` | `cubic-bezier(0.2, 0.8, 0.2, 1)` | Кривая анимации |

### 3.2 Нормативные правила CSS (`ui/components/dm-friends.css`)

```css
/* =============================================================================
 * dm-friends.css — Личные сообщения (DM / Group DM) и Друзья (N1-07 / C2).
 * Строго token-only: все var(--...) из rust/gui-spike/ui/tokens.css.
 * Сырых цветовых литералов, флагов приоритета и селекторов-предков здесь нет.
 * ========================================================================== */

/* 1. Кнопка «Домой / Друзья» в #guild-rail */
.home-rail-btn {
  position: relative;
  border: var(--space-0);
  cursor: pointer;
  transition:
    background var(--motion-fast) var(--ease-out),
    border-radius var(--motion-fast) var(--ease-out);
}

.home-rail-btn:not(.is-selected) {
  background: var(--surface-2);
  color: var(--text);
}

.home-rail-btn:not(.is-selected):hover {
  background: var(--surface-4);
  border-radius: var(--radius-md);
}

.home-rail-btn.is-selected {
  background: var(--accent);
  color: var(--text-on-accent);
}

.home-rail-btn.is-selected::before {
  position: absolute;
  top: 50%;
  left: calc(-1 * var(--space-4));
  width: var(--space-2);
  height: var(--space-7);
  border-radius: var(--radius-round);
  background: var(--text-strong);
  transform: translateY(-50%);
  content: "";
}

.home-rail-btn:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.home-rail-btn .guild-mention-badge {
  min-width: var(--badge-h);
  height: var(--badge-h);
  padding: var(--space-0) var(--badge-pad-x);
  border: var(--focus-ring-width) solid var(--bg-app);
  font-weight: var(--font-weight-bold);
}

/* 2. Шапка и список ЛС в #channel-tree[data-nav-mode="home"] */
.dm-search-trigger {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  height: var(--btn-height-sm);
  padding: var(--space-0) var(--space-4);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-xs);
  background: var(--surface-input);
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  text-align: left;
}

.dm-search-trigger:hover {
  color: var(--text);
}

.dm-search-trigger-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dm-nav-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding-bottom: var(--space-3);
  border-bottom: var(--menu-border) solid var(--divider);
}

.dm-group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-2) var(--space-1);
}

/* --text-faint используется строго на фоне --bg-app (#channel-tree) */
.dm-group-title {
  margin: var(--space-0);
  color: var(--text-faint);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-caps);
  line-height: var(--line-height-small);
  text-transform: uppercase;
}

.dm-create-btn {
  width: var(--icon-size);
  height: var(--icon-size);
  font-size: var(--font-size-ui);
}

.dm-channel-row {
  position: relative;
  display: flex;
  align-items: center;
}

.dm-channel-item {
  height: var(--row-h-member);
  padding-right: var(--space-8);
}

.dm-channel-item:active {
  background: var(--state-active);
}

.dm-channel-item[aria-current="page"] {
  background: var(--state-selected);
  color: var(--text-strong);
}

.dm-item-meta {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.dm-item-name {
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.dm-item-subtext {
  overflow: hidden;
  color: var(--text-muted);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dm-close-btn {
  position: absolute;
  right: var(--space-2);
  width: var(--icon-size);
  height: var(--icon-size);
  opacity: 0;
  font-size: var(--font-size-small);
  transition: opacity var(--motion-fast) var(--ease-out);
}

.dm-channel-row:hover .dm-close-btn,
.dm-channel-row:focus-within .dm-close-btn {
  opacity: 1;
}

/* 3. Переключение между #friends-view и #screen-chat в main.content:
 * Селектор (2, 2, 1) перекрывает правило style.css:153 (2, 2, 0) без селектора-предка
 * и без флага приоритета, в дополнение к снятию .active с #screen-chat и hidden на #chat-header. */
main.content[data-home-subview="friends"] #chat-header#chat-header,
main.content[data-home-subview="friends"] #screen-chat#screen-chat {
  display: none;
}

.friends-view {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: var(--bg-chat);
  font-weight: var(--font-weight-normal);
}

.friends-view[hidden] {
  display: none;
}

/* 4. Шапка и вкладки панели «Друзья» */
.friends-header {
  display: flex;
  flex: 0 0 var(--layout-header-h);
  align-items: center;
  gap: var(--space-4);
  height: var(--layout-header-h);
  padding: var(--space-0) var(--layout-panel-pad);
  box-shadow: var(--shadow-elevation-low);
}

.friends-header-brand {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  color: var(--text-strong);
}

.friends-header-title {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-body);
}

.friends-header-divider {
  width: var(--menu-border);
  height: var(--space-6);
  background: var(--border);
}

.friends-tablist {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
  overflow-x: auto;
}

.friends-tab {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-2);
  height: var(--btn-height-sm);
  padding: var(--space-0) var(--space-4);
  border: var(--space-0);
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-subtle);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
  transition:
    background var(--motion-fast) var(--ease-out),
    color var(--motion-fast) var(--ease-out);
}

.friends-tab:hover {
  background: var(--state-hover);
  color: var(--text-strong);
}

.friends-tab[aria-selected="true"] {
  background: var(--state-selected-strong);
  color: var(--text-strong);
}

/* Кнопка «Добавить в друзья»: фон --positive + текст --text-on-warning = 5.58:1 >= 4.5:1 AA */
.friends-tab--add {
  background: var(--positive);
  color: var(--text-on-warning);
}

.friends-tab--add:hover {
  background: var(--positive);
  color: var(--text-on-warning);
}

.friends-tab--add[aria-selected="true"] {
  background: transparent;
  color: var(--positive-text);
}

/* 5. Тело и список друзей */
.friends-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.friends-tabpanel {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  padding: var(--space-6) var(--layout-panel-pad);
}

.friends-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  margin-bottom: var(--space-5);
}

.friends-pending-controls {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.friends-subfilter-btn {
  height: var(--btn-height);
  padding: var(--space-0) var(--space-5);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--btn-radius);
  background: var(--surface-2);
  color: var(--text);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-medium);
}

.friends-subfilter-btn[aria-pressed="true"] {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--text-strong);
}

.friends-subfilter-btn--danger {
  color: var(--danger-text);
}

/* --text-faint строго на фоне --bg-chat: 4.54:1, запас 0.04 над порогом AA */
.friends-section-heading {
  margin-bottom: var(--space-3);
  color: var(--text-faint);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-caps);
  line-height: var(--line-height-small);
  text-transform: uppercase;
}

.friends-list {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow-y: auto;
}

.friend-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  min-height: var(--row-h-group-start);
  padding: var(--space-3) var(--space-4);
  border-top: var(--menu-border) solid var(--divider);
  border-radius: var(--radius-sm);
  transition: background var(--motion-fast) var(--ease-out);
}

.friend-row:hover {
  background: var(--surface-1);
}

.friend-row-main {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--space-4);
  min-width: 0;
  cursor: pointer;
}

.friend-meta {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.friend-name-line {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.friend-display-name {
  overflow: hidden;
  color: var(--text-strong);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-body);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.friend-username-tag {
  overflow: hidden;
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.friend-status-text {
  overflow: hidden;
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.friend-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-3);
}

.friend-action-btn {
  width: var(--btn-height);
  height: var(--btn-height);
  border-radius: var(--radius-round);
  background: var(--surface-3);
  color: var(--text);
}

.friend-action-btn:hover {
  background: var(--surface-4);
  color: var(--text-strong);
}

.friend-action-btn--accept:hover {
  color: var(--positive-text);
}

.friend-action-btn--danger:hover {
  background: var(--danger-fill);
  color: var(--text-on-danger);
}

.friend-action-btn--danger:active {
  background: var(--danger-fill-hover);
  color: var(--text-on-danger);
}

.friends-empty-state {
  display: grid;
  flex: 1;
  place-items: center;
  padding: var(--space-8);
  color: var(--text-muted);
  text-align: center;
}

/* 6. Панель «Добавить в друзья» (add_friend) */
.friends-add-panel {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-4);
  max-width: var(--layout-content-max-w);
  padding: var(--space-6) var(--layout-panel-pad);
}

.friends-add-title {
  color: var(--text-strong);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-caps);
  line-height: var(--line-height-body);
  text-transform: uppercase;
}

.friends-add-desc {
  color: var(--text-muted);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.friends-add-desc code {
  font-family: var(--font-mono);
  color: var(--text);
}

.add-friend-input-wrap {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  height: var(--space-11);
  padding: var(--space-0) var(--space-5);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-input);
}

.add-friend-input-wrap:focus-within {
  border-color: var(--focus-ring);
}

.add-friend-input-wrap[data-state="success"] {
  border-color: var(--positive);
}

.add-friend-input-wrap[data-state="error"] {
  border-color: var(--danger);
}

.add-friend-input {
  flex: 1;
  min-width: 0;
  border: var(--space-0);
  background: transparent;
  color: var(--text);
  font-size: var(--font-size-ui);
  outline: none;
}

.add-friend-input::placeholder {
  color: var(--text-muted);
}

.add-friend-submit {
  flex: 0 0 auto;
  height: var(--btn-height);
  padding: var(--space-0) var(--btn-pad-x);
  border: var(--space-0);
  border-radius: var(--btn-radius);
  background: var(--accent);
  color: var(--text-on-accent);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  transition: background var(--motion-fast) var(--ease-out);
}

.add-friend-submit:hover:not(:disabled) {
  background: var(--accent-hover);
}

.add-friend-submit:active:not(:disabled) {
  background: var(--accent-active);
}

.add-friend-submit:disabled {
  cursor: not-allowed;
  opacity: var(--state-disabled-opacity);
}

.add-friend-feedback {
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.add-friend-feedback[data-kind="success"] {
  color: var(--positive-text);
}

.add-friend-feedback[data-kind="error"] {
  color: var(--danger-text);
}

/* 7. Модальное окно создания групповой DM (#group-dm-dialog) */
#group-dm-dialog,
#stranger-confirm-dialog {
  z-index: var(--z-modal);
}

.group-dm-modal {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  width: var(--modal-w-sm);
  max-width: 100%;
  padding: var(--modal-pad-y) var(--modal-pad-x);
  border-radius: var(--radius-md);
  background: var(--surface-2);
  box-shadow: var(--shadow-popover);
  color: var(--text);
}

.group-dm-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.group-dm-title {
  color: var(--text-strong);
  font-size: var(--font-size-title);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-title);
}

.group-dm-subtitle {
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.group-dm-tagbox {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-2);
  min-height: var(--space-10);
  padding: var(--space-2) var(--space-3);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-input);
}

.group-dm-chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.group-dm-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--btn-height-sm);
  padding: var(--space-0) var(--space-3);
  border-radius: var(--radius-xs);
  background: var(--surface-4);
  color: var(--text-strong);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-medium);
}

.group-dm-chip-remove {
  padding: var(--space-0);
  border: var(--space-0);
  background: transparent;
  color: var(--text-subtle);
  font-size: var(--font-size-small);
}

.group-dm-search {
  flex: 1;
  min-width: calc(var(--space-10) * 3);
  border: var(--space-0);
  background: transparent;
  color: var(--text);
  font-size: var(--font-size-ui);
  outline: none;
}

.group-dm-friend-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  max-height: calc(var(--space-10) * 6);
  padding: var(--space-2);
  overflow-y: auto;
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.group-dm-friend-option {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: var(--space-10);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-xs);
  cursor: pointer;
}

.group-dm-friend-option:hover {
  background: var(--state-hover);
}

.group-dm-friend-option[aria-selected="true"] {
  background: var(--accent-soft);
}

.group-dm-friend-option[aria-disabled="true"] {
  cursor: not-allowed;
  opacity: var(--state-disabled-opacity);
}

.group-dm-friend-meta {
  display: flex;
  flex: 1;
  align-items: baseline;
  gap: var(--space-2);
  min-width: 0;
}

.group-dm-friend-name {
  overflow: hidden;
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-dm-friend-tag {
  overflow: hidden;
  color: var(--text-muted);
  font-size: var(--font-size-small);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-dm-checkbox {
  display: inline-grid;
  flex: 0 0 auto;
  place-items: center;
  width: var(--icon-size-sm);
  height: var(--icon-size-sm);
  border: var(--focus-ring-width) solid var(--border-strong);
  border-radius: var(--radius-xs);
  color: transparent;
  font-size: var(--font-size-caption);
}

.group-dm-friend-option[aria-selected="true"] .group-dm-checkbox {
  border-color: var(--accent);
  background: var(--accent);
  color: var(--text-on-accent);
}

.group-dm-error {
  color: var(--danger-text);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.group-dm-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-4);
  padding-top: var(--space-3);
  border-top: var(--menu-border) solid var(--divider);
}

.group-dm-btn {
  height: var(--btn-height-lg);
  padding: var(--space-0) var(--btn-pad-x);
  border: var(--space-0);
  border-radius: var(--btn-radius);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
}

.group-dm-btn--secondary {
  background: var(--surface-3);
  color: var(--text);
}

.group-dm-btn--secondary:hover {
  background: var(--surface-4);
}

.group-dm-btn--primary {
  background: var(--accent);
  color: var(--text-on-accent);
}

.group-dm-btn--primary:hover:not(:disabled) {
  background: var(--accent-hover);
}

.group-dm-btn--primary:disabled {
  cursor: not-allowed;
  opacity: var(--state-disabled-opacity);
}

/* 8. Контекстное меню друга (#friend-context-menu) */
.friend-context-menu {
  position: fixed;
  z-index: var(--z-popover);
  min-width: var(--menu-min-w);
  padding: var(--menu-pad);
  border-radius: var(--menu-radius);
  background: var(--surface-3);
  box-shadow: var(--shadow-popover);
}

.friend-context-menu .menu-item {
  display: flex;
  align-items: center;
  width: 100%;
  height: var(--row-menu-item);
  padding: var(--space-0) var(--space-4);
  border: var(--space-0);
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text);
  font-size: var(--font-size-ui);
  text-align: left;
}

.friend-context-menu .menu-item:hover {
  background: var(--state-hover);
  color: var(--text-strong);
}

.friend-context-menu .menu-item--danger {
  color: var(--danger-text);
}

.friend-context-menu .menu-item--danger:hover {
  background: var(--danger-fill);
  color: var(--text-on-danger);
}

/* 9. Баннер запроса переписки (#message-request-banner) */
.message-request-banner {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-4) var(--space-6);
  border-top: var(--menu-border) solid var(--border);
  background: var(--surface-2);
  color: var(--text);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.message-request-banner[hidden] {
  display: none;
}

.message-request-banner-text {
  flex: 1;
  min-width: 0;
  color: var(--text-subtle);
}

.message-request-banner-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: var(--space-3);
}
```

### 3.3 Механическая проверка контраста WCAG 2.2 AA и специфичности CSS

Все пары «текст / фон» в §3.2 проверены по формуле относительной яркости WCAG 2.2 (порог `>= 4.5:1` для мелкого текста `11px`–`16px`):

| Компонент / селектор | Токен текста | Токен фона | Значения (`tokens.css`) | Контраст | Статус WCAG 2.2 AA |
|---|---|---|---|---|---|
| `.friends-tab--add` (невыбранная вкладка «Добавить в друзья», `14px/500`) | `--text-on-warning` | `--positive` | `#121214` на `#3d9e60` | **`5.58:1`** | PASS (`>= 4.5:1`) |
| `.friends-tab--add[aria-selected="true"]` (выбранная вкладка «Добавить в друзья») | `--positive-text` | `--bg-chat` | `#57d98a` на `#1a1a1e` | **`9.66:1`** | PASS (`>= 4.5:1`) |
| `.home-rail-btn.is-selected`, `.add-friend-submit`, `.group-dm-btn--primary` | `--text-on-accent` | `--accent` | `#ffffff` на `#5865f2` | **`4.61:1`** | PASS (`>= 4.5:1`) |
| `.add-friend-submit:hover`, `.group-dm-btn--primary:hover` | `--text-on-accent` | `--accent-hover` | `#ffffff` на `#4654c0` | **`6.38:1`** | PASS (`>= 4.5:1`) |
| `.add-friend-submit:active` | `--text-on-accent` | `--accent-active` | `#ffffff` на `#3c45a5` | **`8.12:1`** | PASS (`>= 4.5:1`) |
| `.friend-action-btn--danger:hover`, `.menu-item--danger:hover`, `.mention-badge` | `--text-on-danger` | `--danger-fill` | `#ffffff` на `#cf3a3f` | **`4.85:1`** | PASS (`>= 4.5:1`) |
| `.friend-action-btn--danger:active` | `--text-on-danger` | `--danger-fill-hover` | `#ffffff` на `#b92d33` | **`6.02:1`** | PASS (`>= 4.5:1`) |
| `.dm-group-title` (`ЛИЧНЫЕ СООБЩЕНИЯ`) | `--text-faint` | `--bg-app` | `#81828a` на `#121214` | **`4.89:1`** | PASS (`>= 4.5:1`) |
| `.friends-section-heading` (`В СЕТИ — N`) | `--text-faint` | `--bg-chat` | `#81828a` на `#1a1a1e` | **`4.54:1`** | PASS (`>= 4.5:1`; минимальная пара документа, запас `0.04`) |
| `.friend-status-text`, `.friend-username-tag`, `.friends-empty-state` | `--text-muted` | `--bg-chat` | `#96979e` на `#1a1a1e` | **`5.96:1`** | PASS (`>= 4.5:1`) |
| `.dm-item-subtext` | `--text-muted` | `--bg-app` | `#96979e` на `#121214` | **`6.43:1`** | PASS (`>= 4.5:1`) |
| `.message-request-banner-text` | `--text-subtle` | `--surface-2` | `#abacb2` на `#242429` | **`6.82:1`** | PASS (`>= 4.5:1`) |
| `.home-rail-btn:not(.is-selected)`, `.friends-subfilter-btn` | `--text` | `--surface-2` | `#efeff1` на `#242429` | **`13.45:1`** | PASS (`>= 4.5:1`) |
| `.friend-action-btn`, `.group-dm-btn--secondary`, `.friend-context-menu .menu-item` | `--text` | `--surface-3` | `#efeff1` на `#28282d` | **`12.77:1`** | PASS (`>= 4.5:1`) |
| `.friend-action-btn:hover`, `.group-dm-chip` | `--text-strong` | `--surface-4` | `#fbfbfb` на `#2c2d32` | **`13.28:1`** | PASS (`>= 4.5:1`) |
| `.friend-action-btn--accept:hover` | `--positive-text` | `--surface-4` | `#57d98a` на `#2c2d32` | **`7.65:1`** | PASS (`>= 4.5:1`) |
| `.add-friend-feedback[data-kind="error"]`, `.group-dm-error` | `--danger-text` | `--bg-chat` / `--surface-2` | `#ff9a9e` на `#1a1a1e` / `#242429` | **`8.57:1` / `7.63:1`** | PASS (`>= 4.5:1`) |

Минимальная пара всего документа — `--text-faint` на `--bg-chat` (`#81828a` на `#1a1a1e`) = **`4.54:1`**: запас над порогом `4.5:1` всего `0.04`, поэтому `--text-faint` на `--bg-chat` допустим только для `.friends-section-heading` и не переносится на другие поверхности. `--text-faint` на `--surface-1..4` запрещён (§0.2 п.2). Все 17 значений таблицы пересчитаны независимым скриптом по формуле WCAG 2.2 (`(L1+0.05)/(L2+0.05)`); числа §3.1 и §3.3 совпадают.

---

## 4. Матрицы состояний и пользовательских сценариев

### 4.1 Матрица вкладок фильтрации `#friends-view`, источник `Presence` и детерминированная сортировка

#### 4.1.1 Источник `Presence` в `AppState` (`[D]` / `[R]`)
- Объекты `Relationship` (`rust/crates/discord-core/src/ready/social.rs:115-161`) и `ReadyUser` **не содержат** поля `presence`.
- Источником присутствия для вкладок `#friends-view` и списка `#dm-channel-list` служит таблица `state.presences: BTreeMap<PresenceKey, Presence>` (`rust/crates/discord-core/src/state/types.rs:118`), где ключ — составной `PresenceKey = (u64, PresenceScope)` (`rust/crates/discord-core/src/state/presence.rs:256-264`), а не одиночный `Snowflake`. Для друзей и приватных каналов используется скоуп `PresenceScope::Private`, поэтому чтение всегда идёт по ключу `(user_id, PresenceScope::Private)`, а не по `user_id`. Для UI строится проекция `presencesByUser: Map<u64, Presence>` только из записей с `PresenceScope::Private`; гильдейские записи `PresenceScope::Guild(guild_id)` в неё не попадают:
  ```js
  // UI (JS): проекция BTreeMap<(u64, PresenceScope), Presence> по Private-scope
  const presence = presencesByUser.get(userId);
  ```
  ```rust
  // Rust: прямое чтение из BTreeMap
  let presence = state.presences.get(&(user_id, PresenceScope::Private));
  ```
  Плоский вызов `state.presences.get(id)` недействителен и в спецификации не используется.
  - начальное заполнение при старте сессии приходит из `READY_SUPPLEMENTAL.d.merged_presences.friends` (с `PresenceScope::Private`, `[D] DM-FRIENDS-RE.md §6.3`);
  - инкрементальные обновления приходят через Gateway-событие `PRESENCE_UPDATE` (`state/reducer.rs`);
  - если для `(user_id, PresenceScope::Private)` нет записи в `state.presences`, его статус считается `PresenceStatus::Offline` (`data-status="offline"`).
- **Правило определения `isOnline` и `data-status` для индикатора `.status-dot`**:
  - Друг включается во вкладку `online` («В сети») тогда и только тогда, когда `presence.status.is_online()` возвращает `true` (`rust/crates/discord-core/src/state/presence.rs:65-69`), т.е. `presence.status ∈ {"online", "idle", "dnd"}`.
  - Фиолетовый индикатор стриминга `.status-dot[data-status="streaming"]` (`--status-streaming` в `tokens.css:113`) является **производным UI-состоянием**: он выставляется, если `presence.status.is_online()` **и** в `presence.activities` есть хотя бы одна активность с `activity.activity_type == ActivityType::Streaming` (`type === 1`; поле объявлено как `pub activity_type: ActivityType` — `rust/crates/discord-core/src/state/presence.rs:201`, объявление `ActivityType` — `:78-82`, разбор `type` — `:107`). Поля `kind` в структуре `Activity` нет. Структура `Presence` и поле `activities: Vec<Activity>` объявлены в `rust/crates/discord-core/src/state/presence.rs:268-278`. В остальных случаях `data-status` равен строковому значению `presence.status` (`"online" | "idle" | "dnd" | "offline"`).

#### 4.1.2 Детерминированный компаратор сортировки `#friends-list`
- Для исключения нестабильного порядка при совпадении отображаемых имён и различиях `localeCompare` между рантаймами используется единый инстанс `Intl.Collator`:
  ```js
  const FRIEND_NAME_COLLATOR = new Intl.Collator("ru", {
    usage: "sort",
    sensitivity: "base",
    numeric: true,
  });
  ```
- Ключ имени: `displayKey(rel) = (rel.nickname || user?.global_name || user?.username || rel.id).trim()`.
- **Двухключевой компаратор алфавитных вкладок (`online`, `all`, `blocked`)**:
  1. Первичный ключ: `const cmp = FRIEND_NAME_COLLATOR.compare(displayKey(a), displayKey(b))` (`cmp !== 0 → return cmp`).
  2. Вторичный ключ (tie-breaker при полном совпадении имён): возрастание Snowflake `BigInt(a.id) < BigInt(b.id) ? -1 : BigInt(a.id) > BigInt(b.id) ? 1 : 0`.
- **Трёхключевой компаратор вкладки `pending` («Ожидание»)**:
  1. Первичный ключ: входящие (`relation_type === 3`) всегда перед исходящими (`relation_type === 4`).
  2. Вторичный ключ: убывание времени создания заявки `Date.parse(b.since ?? "") || 0` минус `Date.parse(a.since ?? "") || 0`.
  3. Третичный ключ (tie-breaker при одинаковом или пустом `since`): убывание Snowflake `BigInt(b.id) > BigInt(a.id) ? 1 : -1`.

#### 4.1.3 Сводная таблица вкладок `#friends-view`

| Вкладка (`data-tab`) | Фильтр по `state.relationships` и `state.presences` | Сортировка строк | Текст пустого состояния (`#friends-empty-text`) |
|---|---|---|---|
| `online` («В сети») | `relation_type === 1` (`FRIEND`) **и** `!user_ignored` **и** `state.presences.get((id, PresenceScope::Private))?.status ∈ {"online", "idle", "dnd"}` (`PresenceStatus::is_online()`) | `FRIEND_NAME_COLLATOR` по `nickname \|\| global_name \|\| username`, tie-breaker: возрастание `BigInt(id)` | «Сейчас никого из друзей нет в сети» |
| `all` («Все») | `relation_type === 1` (`FRIEND`) | `FRIEND_NAME_COLLATOR` по `nickname \|\| global_name \|\| username`, tie-breaker: возрастание `BigInt(id)` | «У вас пока нет друзей. Добавьте кого-нибудь!» |
| `pending` («Ожидание») | По умолчанию: `(relation_type === 3 && !is_spam_request && !user_ignored) \|\| relation_type === 4`. При включённом `#friends-toggle-spam` (`aria-pressed="true"`): `relation_type === 3 && (is_spam_request \|\| user_ignored)` (`[D] @9318383`) | 1) `type === 3` перед `type === 4`; 2) убывание `Date.parse(since ?? "") \|\| 0`; 3) убывание `BigInt(id)` | «Нет ожидающих запросов в друзья» |
| `blocked` («Заблокированные») | `relation_type === 2` (`BLOCKED`) | `FRIEND_NAME_COLLATOR` по `nickname \|\| global_name \|\| username`, tie-breaker: возрастание `BigInt(id)` | «У вас нет заблокированных пользователей» |
| `add_friend` («Добавить в друзья») | Скрывает `#friends-tabpanel`, показывает `#friends-add-panel` | — | — |

### 4.2 Жизненный цикл мутаций отношений, заметок и ЛС (`idle → pending → ready | error`)

1. **Принятие / отклонение / отмена заявки / удаление из друзей / блокировка и разблокировка (`PUT` или `DELETE /users/@me/relationships/{user_id}`)**:
   - При нажатии на кнопку `.friend-action-btn` или соответствующий пункт `#friend-context-menu` строке `.friend-row` ставится `data-state="pending"`, кнопки внутри `.friend-actions` получают `disabled` и `aria-busy="true"`.
   - **Никаких оптимистичных удалений или смен вкладки до ответа `2xx`**, кроме явно доказанного в бандле оптимистичного закрытия ЛС (`DELETE /channels/{id}?silent=false`, `[D] @569942`) и принятия message request (`MESSAGE_REQUEST_ACCEPT_OPTIMISTIC`, `[D] e1affb…@311`).
   - При `200/204` стор обновляется (или идемпотентно применяет `RELATIONSHIP_ADD` / `RELATIONSHIP_UPDATE` / `RELATIONSHIP_REMOVE`), пересчитываются счётчики `pendingCount`, `spamAndIgnoredCount`, `friendCount`, и в `#server-status` (`role="status"`, `aria-live="polite"`) объявляется результат («Запрос в друзья от {name} принят», «Пользователь {name} удалён из друзей», «Пользователь {name} заблокирован»).
   - При ошибке `80013` (`RELATIONSHIP_INVALID_NO_CONFIRMATION`) открывается `#stranger-confirm-dialog`; подтверждение повторно вызывает `PUT /users/@me/relationships/{user_id}` с `{"type": 1, "confirm_stranger_request": true}` (`[D] @6673300`).
2. **Массовая очистка входящих заявок (`#friends-clear-pending-btn`, `DELETE /users/@me/relationships?relationship_type=3`)**:
   - При обычном режиме вкладки `pending` (`#friends-toggle-spam[aria-pressed="false"]`) отправляется `DELETE /users/@me/relationships?relationship_type=3` без тела (`[D] @699593`).
   - При активном режиме спама/игнора (`#friends-toggle-spam[aria-pressed="true"]`) отправляется `DELETE /users/@me/relationships?relationship_type=3` с телом `{"filters": [1, 2]}` (`[D] @699593`: `SPAM = 1`, `IGNORED = 2`).
3. **Ник друга (`PATCH /users/@me/relationships/{user_id}`), заметка (`GET`/`PUT /users/@me/notes/{user_id}`) и игнор (`PUT`/`DELETE /users/@me/relationships/{user_id}/ignore`)**:
   - Пункты `#friend-context-menu` `friend-edit-nick` и `friend-edit-note` выполняют запросы по контракту §2.5; для `GET /users/@me/notes/{user_id}` обязательна проверка `body.note_user_id === userId` (`[D] a8de01…@161`), при несовпадении стор не мутируется.
   - Пункт `friend-toggle-ignore` вызывает `PUT` или `DELETE /users/@me/relationships/{user_id}/ignore` (`[D] @698261, @698537`) и после `2xx` обновляет `user_ignored` и пересчитывает `pendingCount` / `spamAndIgnoredCount`.
4. **Интеграция с кнопкой «Написать сообщение» (`data-action="dm-user"`) в MB-03 Profile Popover и `.friend-row`**:
   - Нажатие `dm-user` или клик по `.friend-row-main` (`data-action="open-dm"`) вызывает `openPrivateChannel({ recipientIds: [userId] })` (`[D] @567585`):
     1. **Локальный поиск существующего 1-on-1 DM в кэше (`[A]`)**: если в `state.private_channels` уже есть канал с `channel_type === 1 && recipient_ids.length === 1 && recipient_ids[0] === userId`, клиент **без сетевого вызова** переключает `#guild-rail` на `#home-rail-btn` (`selectGuild("@me")`), переводит `main.content` в `data-home-subview="dm"`, выделяет этот канал в `#dm-channel-list` и открывает его в `#screen-chat`.
     2. Если канала нет в кэше, отправляется `POST /users/@me/channels` с телом `{"recipients": [userId]}` (`[D] @568882`); полученный канал нормализуется и проходит через `CHANNEL_CREATE` (идемпотентно по `id`), добавляется в `#dm-channel-list` и открывается в `#screen-chat`.

---

## 5. WAI-ARIA и клавиатурная навигация (Полный контракт)

### 5.1 Таблица ARIA-ролей и атрибутов

| Узел DOM | Роль (`role`) | Обязательные ARIA-атрибуты | Доступное имя и динамика |
|---|---|---|---|
| `#home-rail-btn` | Нативная `<button>` | `data-guild-id="@me"`, `aria-current="page"` (когда активен режим Home), `aria-label` | Динамический `aria-label` по единственной формуле §2.1.2 п.3: `"Личные сообщения и Друзья"` при `pendingCount + dmMentionCount === 0`, иначе `"Личные сообщения и Друзья, " + [pendingPart, mentionPart].join(", ")` (только ненулевые части, склонение `pluralRu`). Примеры: `"Личные сообщения и Друзья, 2 входящих запроса в друзья"`, `"Личные сообщения и Друзья, 1 непрочитанное упоминание"`; дочерний `#home-rail-badge` имеет `aria-hidden="true"` и `hidden` при `0` |
| `#channel-tree[data-nav-mode="home"]` | Нативный `<aside>` (`navigation`) | `aria-label="Личные сообщения"`, `data-state`, `aria-busy` | Меняет `aria-label` между `"Каналы сервера"` (`data-nav-mode="guild"`) и `"Личные сообщения"` (`data-nav-mode="home"`) |
| `.dm-channel-item` и `#dm-nav-friends`, `#dm-nav-requests` | Нативная `<button>` | `aria-current="page"` (у активного пункта), `tabindex="0 \| -1"` | Динамический `aria-label` по формулам §2.2 п.1: `#dm-nav-friends` — `"Друзья"` или `"Друзья, " + pendingCount + " " + pluralRu(...) + " в друзья"`; `#dm-nav-requests` — `"Запросы переписки"` или `"Запросы переписки, " + messageRequestCount + " " + pluralRu(messageRequestCount, "новый запрос", "новых запроса", "новых запросов")`; у `.dm-channel-item` включает имя, статус получателя и число упоминаний |
| `.dm-close-btn` | Нативная `<button>` | `aria-label="Закрыть переписку с {name}"` (или `"Покинуть группу «{name}»"` для `type === 3`), `tabindex="-1"` | Сосед `.dm-channel-item` внутри `<li class="dm-channel-row">` (не вложен внутрь `<button>`, чтобы соблюдать валидность HTML и ARIA) |
| `#friends-tablist` | `tablist` | `aria-orientation="horizontal"`, `aria-label="Фильтры списка друзей"` | Содержит 5 вкладок `button.friends-tab[role="tab"]` |
| `.friends-tab` | `tab` | `aria-selected="true \| false"`, `aria-controls="friends-tabpanel \| friends-add-panel"`, `tabindex="0 \| -1"` | Roving `tabindex`: `"0"` только у выбранной вкладки; у `#friends-tab-pending` при `pendingCount > 0` обновляется `aria-label` по формуле §2.2 п.1 (`"Ожидание, " + pendingCount + " " + pluralRu(...) + " в друзья"`), при `0` — `"Ожидание"` |
| `#friends-tabpanel`, `#friends-add-panel` | `tabpanel` | `aria-labelledby="friends-tab-*"` (динамически совпадает с `id` активной вкладки `online \| all \| pending \| blocked` или `friends-tab-add`) | **Без `tabindex="0"`** на самом `tabpanel` (так как внутри находятся фокусируемые контролы поиска и списка); неактивная панель скрыта атрибутом `hidden` |
| `.friend-row-main[role="button"]` | `button` | `tabindex="0 \| -1"`, `aria-label` | Позволяет открыть DM с другом по `Enter` или `Space`; участвует в roving `tabindex` списка `#friends-list` |
| `.group-dm-modal` в `#group-dm-dialog` | `dialog` | `aria-modal="true"`, `aria-labelledby="group-dm-title"`, `aria-describedby="group-dm-subtitle"` | Циклический focus trap внутри модалки (`Tab` / `Shift+Tab` + `inert` и `aria-hidden="true"` на `.shell`); `#group-dm-subtitle` имеет `aria-live="polite"` |
| `#group-dm-search` | `combobox` | `aria-expanded="true \| false"`, `aria-autocomplete="list"`, `aria-controls="group-dm-friend-list"`, `aria-activedescendant` | Реализует паттерн **combobox + `aria-activedescendant`**: реальный DOM-фокус остаётся в `#group-dm-search`; `aria-expanded="true"` при `options.length > 0` и `"false"` при пустом списке |
| `#group-dm-friend-list` | `listbox` | `aria-multiselectable="true"`, `aria-labelledby="group-dm-title"` | Опции `.group-dm-friend-option[role="option"]` **не имеют `tabindex`** и не получают DOM-фокус; несут `id="group-dm-opt-{userId}"`, `aria-selected="true \| false"` и `aria-disabled="true"` при достижении лимита |
| `.stranger-confirm-modal` | `alertdialog` | `aria-modal="true"`, `aria-labelledby="stranger-confirm-title"`, `aria-describedby="stranger-confirm-desc"` | Открывается при коде `80013`; циклический focus trap; первичный фокус — на кнопке «Отмена» (`data-action="cancel-stranger-confirm"`) |
| `#friend-context-menu` | `menu` | `aria-label="Действия с пользователем"` | Пункты имеют `role="menuitem"` и roving `tabindex` (`0` у активного, `-1` у остальных) |
| `#message-request-banner` | `note` | `aria-labelledby="message-request-banner-text"`, `hidden` + `aria-hidden="true"` в начальном состоянии | Показывается только в `main.content[data-home-subview="dm"]` при `is_message_request === true`; содержит две нативные `<button>` («Отклонить», затем «Принять») с явными `aria-label`; при `data-state="pending"` обе `disabled` и `aria-busy="true"` (§2.2.1) |

### 5.2 Клавиатурная модель для всех интерактивных узлов

1. **Рейл `#guild-rail`, кнопка `#home-rail-btn` и совместимость с `tests/unit/guild-rail.test.mjs`**:
   - Кнопка `#home-rail-btn` расположена в `.brand` **вне** `<ul class="guild-list">` и является самостоятельным таб-стопом (`tabindex="0"`).
   - **Клавиатурная навигация внутри `.guild-list` (`button.guild-item`) остаётся на 100% неизменной** относительно контракта `tests/unit/guild-rail.test.mjs:223-248`:
     - `Home` на любой плитке `.guild-item` выбирает и фокусирует **первую гильдию** (`GUILDS[0]`; в fixture `fixtures/gateway/by-type/002-READY.json` это `274093052673392641` — оракул берёт id из `d.guilds[0].id` payload, а не из литерала), а **не** `#home-rail-btn`;
     - `End` на любой плитке `.guild-item` выбирает и фокусирует **последнюю гильдию** (`GUILDS[GUILDS.length - 1]`; в fixture — `1340572775974305803`, `d.guilds[6].id`);
     - `ArrowUp` на первой плитке `.guild-item` циклически переходит на **последнюю гильдию** (`GUILDS[GUILDS.length - 1]`), а `ArrowDown` на последней — на первую (`GUILDS[0]`).
     - Roving `tabindex` внутри `.guild-list` не зависит от режима: `tabindex="0"` у выбранной плитки, а в режиме Home (плитки не выбраны) — у плитки, активной до перехода, иначе у первой `.guild-item` (§2.1.2 п.1).
   - **Навигация на `#home-rail-btn` и обратно**:
     - Переход между `#home-rail-btn` и активной плиткой в `.guild-list` осуществляется клавишами `Tab` (из `#home-rail-btn` в `.guild-list`) и `Shift+Tab` (из `.guild-list` на `#home-rail-btn`).
     - Когда фокус уже находится на `#home-rail-btn`, нажатие `Enter` или `Space` активирует режим «Домой» (`selectGuild("@me")`, `data-nav-mode="home"`), а нажатие `ArrowDown` переводит фокус на текущую активную (или первую) плитку `.guild-item` в `.guild-list`.
2. **Дерево ЛС (`#channel-tree[data-nav-mode="home"]`) и кнопки закрытия `.dm-close-btn`**:
   - Пункты `#dm-nav-friends`, `#dm-nav-requests` (когда он не `hidden`) и все кнопки `.dm-channel-item` образуют единый список с roving `tabindex` (`tabindex="0"` на текущем выбранном пункте, `"-1"` на остальных).
   - Клавиши `ArrowDown` / `ArrowUp` / `Home` / `End` перемещают фокус между видимыми пунктами навигации Home и синхронно переносят `tabindex="0"`.
   - Когда фокус находится на `.dm-channel-item`, нажатие `ArrowRight` или `Delete` переводит фокус на соседнюю кнопку `.dm-close-btn` (`tabindex="0"` на время фокуса внутри `.dm-channel-row`), а `ArrowLeft` возвращает фокус обратно на `.dm-channel-item`. Нажатие `Enter` или `Space` на `.dm-close-btn` закрывает ЛС (`DELETE /channels/{id}?silent=false`) и переводит фокус на соседний канал в `#dm-channel-list` (либо на `#dm-nav-friends`, если список опустел).
3. **Вкладки фильтров `#friends-tablist` (`role="tablist"`)**:
   - Roving `tabindex`: `ArrowRight` / `ArrowLeft` циклически перемещают фокус между 5 вкладками (`online`, `all`, `pending`, `blocked`, `add_friend`), `Home` — на первую («В сети»), `End` — на последнюю («Добавить в друзья»).
   - При перемещении стрелками или нажатии `Enter` / `Space` вкладка активируется (`aria-selected="true"`), у `#friends-tabpanel` синхронно обновляется `aria-labelledby="friends-tab-{activeTab}"`, а при переходе на `add_friend` фокус по `Tab` сразу попадает в поле `#add-friend-input` (так как сама панель `tabpanel` не имеет `tabindex="0"`).
4. **Строки списка друзей `.friend-row` и контекстное меню `#friend-context-menu`**:
   - Внутри `#friends-list` действует roving `tabindex` по `.friend-row-main`: `tabindex="0"` только у активной строки (по умолчанию первой), `"-1"` у остальных. Клавиши `ArrowDown` / `ArrowUp` / `Home` / `End` перемещают фокус между строками `.friend-row-main`.
   - Нажатие `Enter` или `Space` на `.friend-row-main` друга (`type === 1`) открывает ЛС с ним.
   - Нажатие `ContextMenu` / `Shift+F10` на `.friend-row` или `Enter` / `Space` / `ArrowDown` на кнопке `button[data-action="friend-more-menu"]` открывает `#friend-context-menu` (`aria-expanded="true"` на триггере, фокус на первый enabled `menuitem` с `tabindex="0"`).
   - Внутри `#friend-context-menu` действуют `ArrowDown` / `ArrowUp` / `Home` / `End` (roving `tabindex`), `Enter` / `Space` (активация пункта), `Tab` (закрытие меню) и `Escape` (`event.stopPropagation()`, закрытие меню и возврат фокуса на кнопку-триггер `opener`).
5. **Модалка создания групповой DM (`#group-dm-dialog`) и диалог `#stranger-confirm-dialog`**:
   - При открытии `#group-dm-dialog` (или `#stranger-confirm-dialog`) запоминается `opener` (`#create-group-dm-btn`, `#chat-add-recipients-btn` или кнопка принятия заявки), на `.shell` выставляются `inert` и `aria-hidden="true"`, а внутри модалки включается циклический focus trap (`Tab` на последнем фокусируемом элементе модалки переводит фокус на первый; `Shift+Tab` на первом — на последний).
   - В `#group-dm-dialog` первичный фокус ставится в `#group-dm-search`. Опции `.group-dm-friend-option` **не имеют `tabindex`**; клавиши `ArrowDown` / `ArrowUp` в `#group-dm-search` перемещают визуальный курсор по опциям `#group-dm-friend-list`, обновляя `aria-activedescendant="group-dm-opt-{userId}"` на `#group-dm-search`, а нажатие `Enter` переключает выбор активной опции (`aria-selected="true|false"`), добавляя или удаляя чип в `#group-dm-chips`.
   - Нажатие `Backspace` в пустом `#group-dm-search` удаляет последний выбранный чип `.group-dm-chip`.
   - Нажатие `Escape` в `#group-dm-dialog` или `#stranger-confirm-dialog` вызывает `event.stopPropagation()` (инвариант D3: не меняет `body[data-active-screen]`), снимает `inert` и `aria-hidden` с `.shell`, закрывает модалку и возвращает фокус на сохранённый `opener`.

---

## 6. Матрица трассируемости и оракулы тестирования (G1 / G2 / G4)

| ID проверки | Требование спецификации | Раздел `DM-FRIENDS-RE.md` / Спека | Метка | Проверяемый инвариант (оракул теста) |
|---|---|---|---|---|
| `T-N107-READY-01` | Разбор `READY.d.private_channels` и `READY.d.relationships` | §5.1, §5.2, §6.1, §6.2, §9 (`002-READY.json`) | `[D]` | На `002-READY.json`: `private_channels.len() == 17` (все `type == 1`, `recipient_ids.len() == 1`), `relationships.len() == 60` (`type 1` × 18, `type 2` × 42, у всех `id == user_id`), 0 потерянных ссылок в `users[]` (146 записей) |
| `T-N107-READY-02` | Полная замена коллекций при `READY` (`CONNECTION_OPEN`) и гидратация `READY_SUPPLEMENTAL` | §6.3, §6.4 (`@4921010`, `@9320846`) | `[D]` | Повторный `READY` очищает старые `private_channels` и `relationships` целиком, а не мержит; `READY_SUPPLEMENTAL.merged_presences.friends` заполняет `state.presences` (`PresenceScope::Private`) |
| `T-N107-HOME-NAV-01` | Переключение `#home-rail-btn` (`guildId = "@me"`) ↔ гильдия, целостность `.tree-header` и **полный цикл персистентности режима Home** | §0.2 п.3–4, §2.1, §2.2, §5.2 п.1 | `[F]`/`[A]` | 1) Клик по `#home-rail-btn` выставляет `state.activeGuildId = "@me"`, `#channel-tree[data-nav-mode="home"]`, `main.content[data-home-subview="friends"]`, снимает `.active` с `#screen-chat` (`aria-hidden="true"`), ставит `hidden` на `#chat-header` и **не вызывает** `channels("@me")`; 2) Возврат на сервер восстанавливает `.tree-guild` и `button[data-action="guild-menu"]`, снимает `hidden` с `#chat-header` и возвращает `.active` на `#screen-chat`; 3) `tests/unit/guild-rail.test.mjs` (`Home` → `GUILDS[0]`, `ArrowUp` → последняя) проходит на 100%; 4) **Round-trip save → load → boot**: выбор Home вызывает `settings_save` с `{ lastGuildId: "@me", homeMode: true }` (остальные поля `UiSettings` сохранены) и пишет `localStorage["discord-lab.homeNavMode"] = "home"`; повторный запуск с этим файлом (`settings_load`) активирует Home **до** `ready()` без мигания гильдейского чата (когда `localStorage`/`sessionStorage` доступны; при их отказе `settings_load` асинхронен и допускается один кадр гильдейского чата до его ответа, после которого Home включается); выбор гильдии сохраняет `{ lastGuildId: "<digits>", homeMode: false }` и на следующем boot открывает эту гильдию; файл прежней версии без `homeMode` трактуется как `homeMode: false`; 5) `readLastGuildId`, `normalizeSettings`, `getEffectiveLastGuildId` и Rust `normalize_last_guild_id`/`validate` принимают `"@me"` и не превращают его в `None`/`""`; 6) **Сохранение из диалога настроек** (`collectSettings()` → `normalizeSettings()` → `settings_save`): при загрузке без `lastGuildId` и без `homeMode` payload содержит ровно пять ключей (`density`, `locale`, `theme`, `toggles`, `version`) — замороженный оракул `tests/unit/gui-spike-features.test.mjs` остаётся зелёным; при активном Home тот же путь сохраняет эти пять ключей плюс `lastGuildId: "@me"` и **не добавляет** `homeMode`; ключ `homeMode` появляется в payload только тогда, когда он был boolean в источнике; после такого сохранения Home восстанавливается по `lastGuildId === "@me"` (§2.1.1 п.3) |
| `T-N107-TABS-01` | Фильтрация 5 вкладок `#friends-view`, тогл спама и пустые состояния | §2.3, §4.1 | `[D]`/`[A]` | Вкладка `online` включает только `type === 1 && !user_ignored && status.is_online()`; `all` — все `type === 1`; `pending` — `(type === 3 && !spam && !ignored) \|\| type === 4`, а при `#friends-toggle-spam[aria-pressed="true"]` — `type === 3 && (spam \|\| ignored)`; `blocked` — `type === 2`; `add_friend` скрывает `#friends-tabpanel` и показывает `#friends-add-panel` |
| `T-N107-SORT-01` | Детерминированная сортировка `#dm-channel-list` и `#friends-list` | §2.2 п.2, §4.1.2 | `[D]`/`[A]` | 1) `#dm-channel-list`: сортировка по убыванию `BigInt(last_message_id ?? "0")`, при равенстве — по убыванию `BigInt(channel.id)`; имя 1-on-1 DM берётся из `state.relationships[id].nickname \|\| state.users[id].global_name \|\| state.users[id].username`; 2) `#friends-list`: `Intl.Collator("ru", { sensitivity: "base", numeric: true })` с tie-break по возрастанию `BigInt(user_id)`; для `pending` — `type === 3` перед `type === 4`, затем по убыванию `Date.parse(since)` и `BigInt(id)` |
| `T-N107-DM-POST-01` | Тело `POST /users/@me/channels` для 1 и N получателей и кэш 1-on-1 DM | §0 п.2, §2.1, §9 (`@567585`, `@568882`, `@569410`) | `[D]`/`[A]` | В теле запроса **всегда** передаётся `{"recipients": ["<id>", ...]}` (`[D]`) и **никогда** нет ключа `recipient_id`; при повторном открытии существующего DM `type === 1 && recipient_ids[0] === userId` (`[A]`) берётся канал из `state.private_channels` без HTTP POST |
| `T-N107-DM-GET-01` | Отсутствие вызовов `GET /users/@me/channels` и `channels("@me")` | §0 п.3, §1.3, §9 (`rest-static.json`) | `[D]` | Клиент при старте и навигации в Home (`guildId === "@me"`) **не вызывает** ни `GET /users/@me/channels`, ни `TauriBridge.channels("@me")` (0 вызовов) |
| `T-N107-GDM-SHELL-01` | Создание группы через shell и добавление участников (`#chat-add-recipients-btn`) | §2.4 (`@568215`, `@570912`) | `[D]` | `POST /users/@me/channels/group-dm-shell` шлёт `{"recipient_id": id}`; `PUT /channels/{id}/recipients/{user_id}` при статусе `201` переключает активный канал на `body.id` и апсертит новую группу `type: 3`, при `200` оставляет прежний `id` |
| `T-N107-GDM-MODAL-01` | Модалка создания Group DM и обработка `30004` | §2.1, §2.3 (`@321000`) | `[D]`/`[A]` | Выбор до 9 друзей (10 участников с `@me`) `[A]`; при ответе сервера `code: 30004` (`TOO_MANY_RECIPIENTS`) `[D]` выводится ошибка в `#group-dm-error` и модалка остаётся открытой |
| `T-N107-FRIEND-ADD-01` | Отправка заявки по тегу и Pomelo (`POST /users/@me/relationships`) и коды ошибок | §1.4, §4.2 (`@697487`) | `[D]`/`[A]` | Ввод `"user#1234"` отправляет `{"username": "user", "discriminator": 1234}` (число!); ввод `"user"` без `#` отправляет `{"username": "user", "discriminator": null}`; не-4-значный тег после `#` отклоняется локально `[A]`; коды `80000`–`80009`, `20026` (Антиспам/Капча), `40068` (Верификация аккаунта) отображают точные русские сообщения из §1.4 |
| `T-N107-FRIEND-PUT-01` | Принятие заявки, блок и код `80013` (`PUT /users/@me/relationships/{id}`) | §4.3 (`@697776`, `@6673300`) | `[D]`/`[A]` | Блокировка шлёт `type: 2`; при ответе `code: 80013` (`RELATIONSHIP_INVALID_NO_CONFIRMATION`) открывается `#stranger-confirm-dialog`, подтверждение повторяет `PUT` с `confirm_stranger_request: true` |
| `T-N107-FRIEND-DEL-01` | Удаление, отмена, разблокировка, игнор и очистка pending | §2.3, §2.5, §4.4, §4.5 (`@698261`, `@698771`, `@699593`) | `[D]` | Одиночное удаление/отмена/разблокировка вызывает `DELETE /users/@me/relationships/{id}`; игнор вызывает `PUT`/`DELETE /users/@me/relationships/{id}/ignore`; массовая очистка входящих вызывает `DELETE /users/@me/relationships?relationship_type=3` (без тела в обычном режиме и с телом `{"filters": [1, 2]}` при активном `#friends-toggle-spam`) |
| `T-N107-NOTE-01` | Чтение и запись заметок (`/users/@me/notes/{id}`) и ника (`PATCH`) | §2.5, §4.6 (`9f6dcc…@493`, `a8de01…@161`) | `[D]` | При `GET /users/@me/notes/{id}` проверяется `body.note_user_id === userId`; при несовпадении выбрасывается ошибка, стор не мутируется; `PATCH /users/@me/relationships/{id}` обновляет `nickname` |
| `T-N107-DISPATCH-01` | Полная таблица переходов 7 событий `CHANNEL_*` и `RELATIONSHIP_*` + негативные кейсы | §1.5, §7.1–§7.4 (`@4924901`, `@4926913`, `@4931266`, `@9322065`) | `[D]` | 1) Все 7 событий (`CHANNEL_CREATE`, `CHANNEL_DELETE`, `CHANNEL_RECIPIENT_ADD`, `CHANNEL_RECIPIENT_REMOVE`, `RELATIONSHIP_ADD`, `RELATIONSHIP_UPDATE`, `RELATIONSHIP_REMOVE`) нормализуют `recipients[]` / `user` в `state.users` и обновляют `recipient_ids` / `state.relationships`; 2) `CHANNEL_DELETE` активного DM переводит UI в `#friends-view`; 3) `RELATIONSHIP_UPDATE` при переходе `4 → 1` порождает уведомление; 4) **Негативные кейсы**: `RELATIONSHIP_ADD` без `user` сохраняет связь с `user_id = d.id` без паники; `CHANNEL_CREATE` с `type: 1` без `recipients` создаёт `recipient_ids: []` без паники; `CHANNEL_DELETE` и `RELATIONSHIP_REMOVE` с неизвестным `id` являются no-op |
| `T-N107-A11Y-01` | Роли WAI-ARIA, roving `tabindex`, combobox, focus trap модалок и начальное скрытие Home-панелей | §5.1, §5.2 | `[F]`/`[A]` | 1) В `#friends-tablist`, `#dm-channel-list`, `#friends-list` и `#friend-context-menu` действует roving `tabindex`; 2) `#friends-tabpanel` и `.group-dm-friend-option` **не имеют** `tabindex="0"`; 3) В `#group-dm-search` работает `aria-activedescendant` и динамический `aria-expanded`; 4) Все 4 бейджа скрыты (`hidden`) при `count === 0`; 5) В модалках работает циклический `Tab`/`Shift+Tab` focus trap + `inert` на `.shell`, а `Escape` вызывает `stopPropagation()`, возвращает фокус на `opener` и не меняет `body[data-active-screen]`; 6) После монтажа `#friends-view` и `#message-request-banner` имеют `hidden` + `aria-hidden="true"` и не видны в guild-режиме; 7) `#message-request-banner` имеет `role="note"`, `aria-labelledby="message-request-banner-text"`, ровно две нативные кнопки с явными `aria-label` в порядке «Отклонить» → «Принять»; 8) В режиме Home ровно одна плитка `.guild-item` держит `tabindex="0"` (активная до перехода, иначе первая), остальные — `-1`, поэтому `Tab` с `#home-rail-btn` попадает в `.guild-list` |
| `T-N107-TOKENS-01` | Строгое соблюдение `rust/gui-spike/ui/tokens.css`, подключение `components/dm-friends.css` и контраста WCAG 2.2 AA | §0.2 п.1, п.5, §3.1, §3.2, §3.3 | `[T]` | 1) 100% переменных `var(--...)` в CSS (§3.2) объявлены в `tokens.css` и совпадают 1:1 с таблицей §3.1 (96 токенов); 2) 0 сырых `#hex`, `rgb()`, `hsl()`, `!important` и `:has()` в CSS-блоках — **включая комментарии** (проверка идёт по всему тексту блока, не только по объявлениям); 3) В `index.html` присутствует `<link rel="stylesheet" href="components/dm-friends.css" />`, поэтому правила §3.2 реально применяются; 4) Контраст всех текстовых пар `>= 4.5:1` (включая `.friends-tab--add`: `--text-on-warning` на `--positive` = `5.58:1`, и `.message-request-banner-text`: `--text-subtle` на `--surface-2` = `6.82:1`), минимальная пара — `4.54:1` |
