# RW-40 / G3 — NOTIFICATIONS-SPEC: настройки уведомлений сервера и каналов (N-05 / ST-05)

Статус: **дизайн-контракт (спецификация G3 Design; продуктовый код и тесты не менялись)**.  
Дата: 2026-10-30.  
Владелец: **G3 Design**.  
Потребители: **G2 Frontend** (N-05 UI, G-05 п.3–5, ST-05 UI, M-07 индикаторы), **G1 Core** (`user_guild_settings` в READY/state/IPC, `USER_GUILD_SETTINGS_UPDATE`, REST PATCH), **G5 Platform** (D-Bus / desktop notifications гейтинг).

Нормативные источники:
- `waves/RW-40/NOTIFICATIONS-RE.md` — полный RE-разбор `user_guild_settings`, `channel_overrides`, `mute_config`, битовых флагов `flags`, `USER_GUILD_SETTINGS_UPDATE`, маршрутов `PATCH /users/@me/guilds/settings` и `PATCH /users/@me/guilds/@me/settings`, а также UI-контракта модалки из бандла `discord.com_assets_046406f7a5feb3e2.js`;
- `waves/RW-40/DESIGN/G05-REST-SPEC.md` — контракт меню сервера `#server-menu` (пункты #3 «Заглушить сервер», #4 «Параметры уведомлений», #5 «Скрыть заглушённые каналы», дефекты D1–D10, типизированные ошибки и 429 countdown);
- `waves/RW-40/DESIGN/ST-01-SPEC.md` — навигация настроек пользователя `#screen-settings` (раздел 5 `notifications_panel` «Уведомления»);
- `waves/RW-40/THEME-SPEC.md` — правила контраста WCAG 2.2 AA (запрет `--warning` и `--accent` в качестве цвета текста, разделение non-text и text-токенов);
- `rust/gui-spike/ui/tokens.css` — единственный источник CSS custom properties;
- `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/components/{tree,rail,chat,settings,server-menu,primitives}.css` — существующая DOM-разметка и компонентные стили спайка;
- `rust/crates/discord-core/src/ready/read_state.rs` — существующие Rust-структуры `ReadState`, `UserGuildSettings`, `UserGuildSettingsEntry`, `ChannelOverride`, `MuteConfig`.

---

## 0. Легенда, ограничения и архитектурные границы

### 0.1 Метки доказательности

| Метка | Значение | Источник |
|---|---|---|
| `[D]` | Факт дампа / бандла / схемы | `waves/RW-40/NOTIFICATIONS-RE.md` (`/home/sanya/re-deep`: `002-READY.json`, `gateway-schema.md`, `web.4d4b2f75fdea5470.js`, `046406f7a5feb3e2.js`, `mod-652215.pretty.js`) |
| `[S]` | Статическая схема маршрутов / типов | `rest-static.json`, `rust/crates/discord-core/src/ready/read_state.rs` |
| `[T]` | Дизайн-токен проекта | `rust/gui-spike/ui/tokens.css` |
| `[M]` | Измерено на референсе | `waves/RW-40/DESIGN/SERVER-MENU.md`, `waves/RW-37/RE/layout.json` |
| `[F]` | Зафиксировано существующими тестами | `tests/unit/gui-spike-*.test.mjs` |
| `[A]` | Проектное решение / адаптация DL | Данный документ (явно отделено от `[D]`) |
| `[?]` | Открытый вопрос RE (не подтверждено live-ответом) | `waves/RW-40/NOTIFICATIONS-RE.md` §9.4, §13 |

### 0.2 Базовые правила спецификации

1. **Строгая привязка к `rust/gui-spike/ui/tokens.css`**: каждое свойство `var(--...)` в этой спецификации обязано существовать в `rust/gui-spike/ui/tokens.css`. Сырые цветовые литералы (`#hex`, `rgb(...)`, `hsl(...)`) и `!important` в компонентных правилах запрещены.
2. **Контраст WCAG 2.2 AA (`THEME-SPEC.md` §2.2–2.6)**:
   - `--warning` используется **только как фон** в паре с `color: var(--text-on-warning)` (контраст 10.76 в dark, 7.24 в light); использовать `--warning` как цвет текста или иконки запрещено.
   - `--danger-fill` используется как фон бейджей и destructive-кнопок в паре с `color: var(--text-on-danger)` (контраст 4.85); для текста ошибок на тёмных поверхностях используется `color: var(--danger-text)`, для декоративных точек/рамок — `var(--danger)`.
   - `--accent` как фон используется только с `color: var(--text-on-accent)` (4.61); для акцентного текста на поверхностях используется `color: var(--text-accent)`.
   - `--text-faint` допускается только на `--bg-app` и `--bg-chat`; на `--surface-2`, `--surface-3`, `--surface-4` используются `--text-muted`, `--text-subtle`, `--text` или `--text-strong`.
3. **Никаких внешних ассетов**: шрифты — системные (`var(--font-sans)`, `var(--font-mono)`), иконки — системные UTF-8 глифы (`🔔`, `🔕`, `✕`, `▾`, `▸`, `#`, `🔊`, `📢`) или CSS-геометрия на токенах (`tokens.css` §5, §11).
4. **Честная работа с сетью и ошибками**: никаких оптимистичных мутаций до получения ответа `2xx` от IPC/REST (или до завершения локального сохранения для чисто локальных настроек). Ошибки `401`, `403`, `429` (с посекундным обратным отсчётом `retry_after_ms`), `5xx` и `network`/`timeout` обрабатываются по единому контракту `G05-REST-SPEC.md` §3.0.

### 0.3 Три контура настроек уведомлений (не смешивать)

По результатам `NOTIFICATIONS-RE.md` (§1–§2, §8, §10) настройки уведомлений в Discord разделены на три независимых слоя хранения:

| Контур | Источник чтения | Маршрут / событие записи | Состав полей | Где в UI |
|---|---|---|---|---|
| **1. Per-guild & per-channel (`user_guild_settings`)** | `READY.d.user_guild_settings.entries[]` `[D]` | Gateway: `USER_GUILD_SETTINGS_UPDATE` → `USER_GUILD_SETTINGS_FULL_UPDATE` `[D]`. REST: `PATCH /users/@me/guilds/settings` (bulk для гильдий) и `PATCH /users/@me/guilds/@me/settings` (для DM) `[D]` | 13 полей гильдии (`muted`, `mute_config`, `message_notifications`, `suppress_everyone`, `suppress_roles`, `notify_highlights`, `mute_scheduled_events`, `mobile_push`, `hide_muted_channels`, `flags`, `version`, `channel_overrides[]`) + 6 полей канала | `#server-menu` (п.3–5), модалка `#guild-notifications-dialog`, поповер канала `#channel-notif-menu`, индикаторы `#guild-rail` и `#channel-tree` |
| **2. Глобальные флаги аккаунта (`notification_settings`)** | `READY.d.notification_settings.flags` `[D]` | Gateway: `NOTIFICATION_SETTINGS_UPDATE`. REST: `PATCH /users/@me/notification-settings` (`{flags}`) `[D]` | Битовая маска: `USE_NEW_NOTIFICATIONS = 16`, `MENTION_ON_ALL_MESSAGES = 32` (`@6216165`) `[D]` | `#screen-settings` → панель `notifications` |
| **3. Локальный стор клиента (`NotificationSettingsStore`)** | Локальное хранилище (`persistKey = "notifications"`, `@9225345`) `[D]`; в спайке — `UiSettings` (`settings_load` / `settings_save`) `[A]` | Локальный IPC `settings_save` (на сервер не отправляется) `[D]` | `desktopType` (`NEVER`\|`ALL`), `disableAllSounds`, `disabledSounds[]`, `ttsType` (`NEVER`\|`ALL_CHANNELS`\|`SELECTED_CHANNEL`), `disableUnreadBadge`, `taskbarFlash`, `notifyMessagesInSelectedChannel`, `screenDowntimeReminder` `[D]` | `#screen-settings` → панель `notifications` (ST-05) |

---

## 1. Модель данных, перечисления и каскад разрешения (RE Traceability)

### 1.1 Контейнер `d.user_guild_settings` и запись `entries[]` (13 полей)

В `READY` контейнер `d.user_guild_settings` имеет три поля (`[D] NOTIFICATIONS-RE.md §1`, `gateway-schema.md:147,385`, `read_state.rs:49-58`):
- `entries`: список записей `UserGuildSettingsEntry` (в capture `58` записей, 193 547 байт);
- `partial`: `bool` (`false` в capture — полный слепок, старый кэш заменяется; при `true` — дозапись поверх кэша без очистки, `@2134328`);
- `version`: `u64` (`734` в capture — версия коллекции).

Каждая запись `entries[]` содержит ровно **13 полей** (`[D] NOTIFICATIONS-RE.md §2`, `gateway-schema.md:255-271`, `read_state.rs:60-89`):

| # | Поле | Тип в Rust / DTO | Факт в capture (`58` зап.) | Назначение и инварианты разбора |
|---:|---|---|---|---|
| 1 | `guild_id` | `Option<String>` (в Rust `Option<u64>`) | `58/58` snowflake (`null` нет) | ID сервера; `null` (или `"@me"` в REST) означает настройки ЛС (DM) `[D]` |
| 2 | `message_notifications` | `i64` (`0 \| 1 \| 2`) | `0`×19, `1`×36, `2`×3 | Базовый уровень уведомлений гильдии (§1.2). Значение `3` на уровне гильдии не встречается `[D]` |
| 3 | `suppress_everyone` | `bool` | `false`×58 | Подавлять `@everyone` и `@here` на сервере `[D]` |
| 4 | `suppress_roles` | `bool` | `false`×57, `true`×1 | Подавлять упоминания всех `@ролей` на сервере `[D]` |
| 5 | `muted` | `bool` | `true`×22, `false`×36 | Флаг приглушения сервера (интерпретируется только вместе с `mute_config`, см. §1.3) `[D]` |
| 6 | `mute_config` | `Option<MuteConfig>` | `null`×37, объект×21 | Окно таймера приглушения: `{ end_time, selected_time_window }` (§1.3) `[D]` |
| 7 | `mobile_push` | `bool` | `true`×57, `false`×1 | Отправлять мобильные push-уведомления `[D]` |
| 8 | `notify_highlights` | `i64` (`0 \| 1 \| 2`) | `0`×56, `1`×2 | Уведомления о хайлайтах (`0=NULL`, `1=DISABLED`, `2=ENABLED`, §1.2) `[D]` |
| 9 | `flags` | `i64` | `0`×52, `16384`×6 | Битовая маска гильдии (`UNREADS_*`, `OPT_IN_CHANNELS_*`, §1.2) `[D]` |
| 10 | `hide_muted_channels` | `bool` | `false`×58 | Скрывать заглушённые каналы в `#channel-tree` `[D]` |
| 11 | `mute_scheduled_events` | `bool` | `false`×57, `true`×1 | Заглушить уведомления о запланированных событиях сервера `[D]` |
| 12 | `version` | `u64` | `0…734` | Версия записи для кэша/БД (`last-wins`) `[D]` |
| 13 | `channel_overrides` | `Vec<ChannelOverride>` / `Map<String, ChannelOverride>` | `1328` эл. у `52` записей (макс. `185` в одной) | Переопределения каналов и категорий (§1.4) `[D]` |

> **Инвариант строгого разбора булевых и `nil` (закрытие D8 / `G05-REST-SPEC.md` §0.5 п.2):**  
> В JSON-дампе ETF-атомы сериализованы строками `"true"`, `"false"`, `"nil"`. На уровне JS/IPC парсер обязан принимать булево `true`/`false` и явно разбирать строковые атомы (`val === true || val === "true"` → `true`; `val === false || val === "false"` → `false`; `"nil"` / `null` → `null`). Неизвестная строка — диагностика/ошибка разбора, а **не** `Boolean("false") === true`. Отсутствие поля в частичном апдейте (`undefined`) означает «не менять текущее поле», а не `false` (`[D] NOTIFICATIONS-RE.md §2, §7`).

### 1.2 Перечисления и битовые флаги (`[D] NOTIFICATIONS-RE.md §3`)

#### 1.2.1 Уровень уведомлений `message_notifications` (`orn`, модуль `652215`, `@313815`)
Закрывает вопрос `[RE]`/`[A]` из `G05-REST-SPEC.md` §0.5 п.7 — числовые коды доказаны бандлом клиента:

| Код | Константа в бандле | Допустимая область | Русская подпись в UI | i18n-ключ (`[D] §10`) | Поведение при чтении и записи |
|---:|---|---|---|---|---|
| `0` | `ALL_MESSAGES` | Гильдия, категория, канал | Все сообщения | `n/bTaY` | Уведомлять о каждом новом сообщении (если не заглушено). При `memberCount >= 2500` на уровне гильдии показывать предупреждение `Dh5p5j` `[D]` |
| `1` | `ONLY_MENTIONS` | Гильдия, категория, канал | Только @упоминания | `L2hmYy` / `6fQPhu` | Уведомлять только при `@user`, а также `@role` и `@everyone`/`@here` (если они не подавлены флагами `suppress_*`) `[D]` |
| `2` | `NO_MESSAGES` | Гильдия, категория, канал | Ничего | `CtVGyQ` | Не создавать звуковых/desktop-уведомлений (прямые `@упоминания` по-прежнему увеличивают бейдж `mention_count`) `[D]` |
| `3` | `NULL` | **Только канал / категория** | По умолчанию сервера (категории) | — | Наследование от родительской категории или гильдии (§1.5). При чтении отсутствие override или поля эквивалентно `3` (`@9387157`); при сбросе override на запись уходит явно `3` (`@1622`) `[D]` |

*Политика валидации (`NOTIFICATIONS-RE.md §11`)*: неизвестное числовое значение (`∉ {0,1,2,3}` или `3` на уровне гильдии) логируется в `diagnostics.ignored` и при отображении фоллбэчится на `guild.default_message_notifications` (`0` или `1`), а не молча превращается в `0`.

#### 1.2.2 Уведомления о хайлайтах `notify_highlights` (`guM`, модуль `652215`, `@529852`)

| Код | Константа | Факт в capture | Отображение в тумблере UI (`@5511, @17620`) |
|---:|---|---|---|
| `0` | `NULL` | `56/58` | Состояние по умолчанию (в тумблере трактуется как включено, пока сервер сам не заглушён) `[D]` |
| `1` | `DISABLED` | `2/58` | Выключено (`aria-checked="false"`). При выключении тумблера UI отправляет `notify_highlights: 1` `[D]` |
| `2` | `ENABLED` | `0/58` | Включено (`aria-checked="true"`). При включении тумблера UI отправляет `notify_highlights: 2` `[D]` |

#### 1.2.3 Битовые маски `flags` гильдии и канала (`модуль 355097`, `@8352891`, `@8352746`)

**Флаги гильдии (`GuildNotificationFlags`, `i64`):**

| Бит | Маска | Константа | Назначение |
|---:|---:|---|---|
| `11` | `2048` | `UNREADS_ALL_MESSAGES` | Считать непрочитанными все сообщения в гильдии `[D]` |
| `12` | `4096` | `UNREADS_ONLY_MENTIONS` | Считать непрочитанными только упоминания `[D]` |
| `13` | `8192` | `OPT_IN_CHANNELS_OFF` | Режим «Каналы по подписке» явно выключен `[D]` |
| `14` | `16384` | `OPT_IN_CHANNELS_ON` | Режим «Каналы по подписке» включён (`6/58` гильдий в capture) `[D]` |

**Флаги канала (`ChannelNotificationFlags`, `Option<i64>`):**

| Бит | Маска | Константа | Назначение |
|---:|---:|---|---|
| `9` | `512` | `UNREADS_ONLY_MENTIONS` | Индикатор непрочитанного в канале только от упоминаний `[D]` |
| `10` | `1024` | `UNREADS_ALL_MESSAGES` | Индикатор непрочитанного в канале от всех сообщений `[D]` |
| `11` | `2048` | `FAVORITED` | Канал добавлен в избранное (`1/1328` в комбинации `6144 = 4096 \| 2048`) `[D]` |
| `12` | `4096` | `OPT_IN_ENABLED` | Канал выбран в списке каналов сервера (`1252/1328` в capture) `[D]` |
| `13` | `8192` | `NEW_FORUM_THREADS_OFF` | Уведомления о новых ветках форума выключены `[D]` |
| `14` | `16384` | `NEW_FORUM_THREADS_ON` | Уведомления о новых ветках форума включены `[D]` |

**Помощники модификации битов (`@11923745`, модуль `665260`) `[D]`:**
- Для гильдии пара `GUILD_UNREAD_MASK = 2048 | 4096` (`6144`); для канала пара `CHANNEL_UNREAD_MASK = 512 | 1024` (`1536`).
- Установка бита unread (`mD(flags, bit)`): `(flags & ~UNREAD_MASK) | bit`.
- Сброс битов unread (`PW(flags)`): `flags & ~UNREAD_MASK`.
- При изменении `flags` запрещено затирать старшие биты (`OPT_IN_ENABLED = 4096`, `FAVORITED = 2048`, `NEW_FORUM_THREADS_*`).

#### 1.2.4 Четыре пресета таблицы/старой модалки (`модуль 24873`, `@6231609`) `[D]`
При глобальном флаге аккаунта `USE_NEW_NOTIFICATIONS = 0` (в нашем capture `d.notification_settings.flags = 0`) клиент поддерживает 4 комбинированных пресета `(unread, message_notifications)`:
1. `all_messages`: `unread = ALL_MESSAGES`, `message_notifications = 0` (`ALL_MESSAGES`);
2. `hybrid`: `unread = ALL_MESSAGES`, `message_notifications = 1` (`ONLY_MENTIONS`) — точка непрочитанного горит на все сообщения, но звуковое/desktop-уведомление приходит только на `@упоминания`;
3. `mentions`: `unread ∈ {UNSET, ONLY_MENTIONS}`, `message_notifications = 1` (`ONLY_MENTIONS`);
4. `nothing`: `unread ∈ {UNSET, ONLY_MENTIONS}`, `message_notifications = 2` (`NO_MESSAGES`).

### 1.3 Таймер приглушения (`mute_config`) и 6 пресетов длительности (`[D] NOTIFICATIONS-RE.md §4`)

Объект `mute_config` одинаков для гильдии и для канала (`gateway-schema.md:265,281`, `read_state.rs:108-115`):
```json
{
  "end_time": "2026-10-30T20:00:00.000Z",
  "selected_time_window": 3600
}
```

#### 1.3.1 Шесть канонических пресетов (`@8349873` экспорт `Xx`, `@6225743` `b8()`) `[D]`

| Порядок | ID опции в бандле | `selected_time_window` (с) | Русская подпись в UI | Формирование `end_time` при записи (`@11993`) |
|---:|---|---:|---|---|
| 1 | `15-minutes` | `900` | На 15 минут | `new Date(nowMs + 900_000).toISOString()` |
| 2 | `1-hour` | `3600` | На 1 час | `new Date(nowMs + 3_600_000).toISOString()` |
| 3 | `3-hours` | `10800` | На 3 часа | `new Date(nowMs + 10_800_000).toISOString()` |
| 4 | `8-hours` | `28800` | На 8 часов | `new Date(nowMs + 28_800_000).toISOString()` |
| 5 | `24-hours` | `86400` | На 24 часа | `new Date(nowMs + 86_400_000).toISOString()` |
| 6 | `forever` | `-1` | Пока не включу (`r3LawO`) | `null` |

#### 1.3.2 Формула вычисления эффективного mute и истечения таймера (`@2364848`, модуль `544743`) `[D]`

```js
const endMs = cfg?.end_time && cfg.end_time !== "nil" ? Date.parse(cfg.end_time) : null;
const isMuted = Boolean(entry.muted) && (endMs === null || (!Number.isNaN(endMs) && endMs >= nowMs));
const isTemporarilyMuted = Boolean(entry.muted) && endMs !== null && !Number.isNaN(endMs) && endMs >= nowMs;
```

Инварианты:
1. **Бессрочный mute без `mute_config`**: `muted: true` + `mute_config: null` (или `end_time: null`) — **валидный бессрочный mute** (`isMuted = true`, `isTemporarilyMuted = false`). В capture из 22 заглушённых гильдий `mute_config` есть у 21, у 1 — `null`; из 42 заглушённых каналов `mute_config` есть у 38, у 4 — `null` `[D]`.
2. **Истёкший `end_time`**: если `endMs < nowMs` (в capture есть 3 канала с историческим `end_time` 2021 года), запись считается **не заглушённой** (`isMuted = false`). При инициализации стора и получении апдейта для каждого активного `isTemporarilyMuted` ставится таймер на `max(0, endMs - nowMs)`, по срабатыванию которого локально сбрасывается `muted = false` (аналог `GUILD_MUTE_EXPIRED` / `CHANNEL_MUTE_EXPIRED` в эталоне, `@9384776`, `@9392282`) `[D]`.
3. **Хранение `end_time`**: сервер присылает ISO-8601 с микросекундами и смещением (`2021-02-27T17:14:08.418000+00:00`), клиент при записи отправляет `toISOString()` (миллисекунды `Z`). Хранить исходную строку без потери точности; сравнивать только через парсинг времени в эпоху, а не лексикографически (`[D] NOTIFICATIONS-RE.md §4, §13`).

### 1.4 Переопределения каналов (`channel_overrides`) (`[D] NOTIFICATIONS-RE.md §5`)

Схема элемента (`gateway-schema.md:273-282`, `read_state.rs:91-106`):
- `channel_id`: `String` / `u64` (обязателен; битый id → skip + `diagnostics.malformed`);
- `collapsed`: `bool` (`false`×1279, `true`×49 в capture — состояние свёрнутости категории в дереве);
- `flags`: `Option<i64>` (присутствует у `1253/1328`, отсутствует у `75/1328`; при чтении для вычисления битов `flags ?? 0`, при частичном апдейте отсутствие поля не затирает существующий `flags`);
- `message_notifications`: `i64` (`3`×1327, `1`×1 в capture);
- `mute_config`: `Option<MuteConfig>` (`null`×1290, объект×38 в capture);
- `muted`: `bool` (`false`×1286, `true`×42 в capture).

**Три ключевых правила работы с `channel_overrides` (`[D] §5`):**
1. **Двойная форма (массив vs словарь)**: в `READY` `channel_overrides` приходит массивом, но в `USER_GUILD_SETTINGS_UPDATE` и локальных экшенах может быть словарём `{ "<channel_id>": { ... } }` (`@9385738`). Парсер обязан принимать обе формы; на запись в REST всегда отправляется словарь `{ "<channel_id>": { ... } }` (`@682805`, `@683232`).
2. **Фильтр видимых строк в таблице настроек уведомлений**: поскольку запись в `channel_overrides` создаётся также ради `collapsed=true` (32 записи в capture) или `OPT_IN_ENABLED=4096` (1252 записи), в UI-таблицу «Переопределения каналов» выводятся **только** те каналы/категории, у которых есть реальное переопределение уведомлений:
   ```js
   const hasNotificationOverride =
     isChannelMuted(override, nowMs) ||
     (override.message_notifications !== undefined && override.message_notifications !== 3) ||
     ((override.flags ?? 0) & (512 | 1024)) !== 0;
   ```
3. **Сброс (удаление) override в UI (`@1622`)**: нажатие кнопки удаления строки в таблице не удаляет саму запись `channel_id` на сервере (чтобы не сломать `collapsed` и `OPT_IN_ENABLED`), а отправляет PATCH:
   ```json
   {
     "channel_overrides": {
       "<channel_id>": {
         "message_notifications": 3,
         "muted": false,
         "flags": "<currentFlags & ~(512 | 1024)>"
       }
     }
   }
   ```

### 1.5 Каскад разрешения уровня и подавления упоминаний (`[D] NOTIFICATIONS-RE.md §6`)

Для любого канала `channel` (с родительской категорией `channel.parent_id`) в гильдии `guild`:

1. **Разрешённый уровень уведомлений (`resolvedMessageNotifications`, `@9387678`)**:
   - Шаг 1: если у `channel.id` есть `channel_override` и `override.message_notifications ∈ {0, 1, 2}` (`!= 3`) → вернуть его;
   - Шаг 2: иначе если у `channel.parent_id` есть `category_override` и `category_override.message_notifications ∈ {0, 1, 2}` (`!= 3`) → вернуть его;
   - Шаг 3: иначе вернуть `guild_entry.message_notifications` (если записи гильдии нет — `guild.default_message_notifications`, по умолчанию `1` `ONLY_MENTIONS`).

2. **Полное подавление (`allowNoMessages`, `@9388508`)**:
   Канал подавляет звуковые и desktop-уведомления (включая уведомления от `@user`, `@role` и `@everyone`/`@here`), если выполнено хотя бы одно условие (`allowNoMessages === true`):
   - `isGuildMuted(guild_entry, nowMs) === true`;
   - `isCategoryMuted(category_override, nowMs) === true`;
   - `isChannelMuted(channel_override, nowMs) === true`;
   - `resolvedMessageNotifications === 2` (`NO_MESSAGES`);
   - на гильдии включён `OPT_IN_CHANNELS_ON` (`(guild_entry.flags & 16384) !== 0`), а у канала и его категории не выставлен `OPT_IN_ENABLED` (`((channel_override?.flags ?? 0) & 4096) === 0`).

3. **Матрица срабатывания упоминаний (`@everyone`, `@here`, `@role`, `@user`) (`[D] NOTIFICATIONS-RE.md §3.1, §6`)**:

| Тип события в сообщении | Условие подавления | Звук / Desktop-уведомление (G5) | Бейдж упоминаний `.mention-badge` (`mention_count`) | Точка непрочитанного `.is-unread` |
|---|---|---|---|---|
| Прямое упоминание `@user` (текущий пользователь) | Звук/desktop подавляется при `allowNoMessages` (`isGuildMuted \|\| isCategoryMuted \|\| isChannelMuted \|\| resolvedMessageNotifications === 2` или opt-in подавление); бейдж не подавляется | Да, если `!allowNoMessages` (`!isMuted && resolvedMessageNotifications !== 2`) | **Всегда увеличивает счётчик** (даже при `muted` и `NO_MESSAGES`) `[D]` | Да (при `muted` виден `.mention-badge`) |
| Упоминание роли `<@&role_id>` пользователя | Полностью подавляется при `guild_entry.suppress_roles === true`; звук/desktop также подавляется при `allowNoMessages` (`isMuted \|\| resolvedMessageNotifications === 2`) | Да, если `!suppress_roles && !allowNoMessages` (`!suppress_roles && !isMuted && resolvedMessageNotifications !== 2`) | Увеличивает, только если `!suppress_roles` (в т.ч. при `NO_MESSAGES`) `[D]` | Да, если `!suppress_roles && !isChannelMuted` |
| `@everyone` или `@here` | Полностью подавляется при `guild_entry.suppress_everyone === true`; звук/desktop также подавляется при `allowNoMessages` (`isMuted \|\| resolvedMessageNotifications === 2`) | Да, если `!suppress_everyone && !allowNoMessages` (`!suppress_everyone && !isMuted && resolvedMessageNotifications !== 2`) | Увеличивает, только если `!suppress_everyone` (в т.ч. при `NO_MESSAGES`) `[D]` | Да, если `!suppress_everyone && !isChannelMuted` |
| Обычное сообщение без упоминаний | Подавляется при `allowNoMessages` или `resolvedMessageNotifications !== 0` (`ALL_MESSAGES`) | Только при `resolvedMessageNotifications === 0 && !allowNoMessages` (`!isMuted`) | Нет (`0`) | Да, если `!isChannelMuted && !isCategoryMuted && resolvedUnread === ALL_MESSAGES` |

### 1.6 Gateway `USER_GUILD_SETTINGS_UPDATE` и REST-маршруты записи (`[D] NOTIFICATIONS-RE.md §7, §9`)

#### 1.6.1 Gateway Dispatch: `USER_GUILD_SETTINGS_UPDATE` (`@4930904`, `@9391654`)
- Payload события — **одна запись** формата `UserGuildSettingsEntry` (`snake_case`).
- Клиент оборачивает её в массив `[entry]` и применяет как частичное обновление поверх существующей записи `guild_id` (`w()` на `@9384248`):
  - Если для `guild_id` записи ещё не было, базовый шаблон берёт `message_notifications = guild.default_message_notifications`.
  - **Защитный инвариант редьюсера (`NOTIFICATIONS-RE.md §7 п.3`)**: если в пришедшем payload ключ `channel_overrides` отсутствует, существующая карта `channel_overrides` **сохраняется без изменений**; если ключ присутствует (массивом или map) — нормализуется по `channel_id` и обновляет overrides.
  - Если `incoming.version < current.version` (при `incoming.version > 0`), устаревший апдейт не перезаписывает более свежие поля (`[D] §12 п.5`).

#### 1.6.2 REST-маршруты сохранения (`saveUserGuildSettings`, модуль `931260`, `@8333335`)

| Цель | Метод и путь | Ключ `Endpoints` | Формат тела запроса | Источник |
|---|---|---|---|---|
| Гильдия (`guildId != null && guildId !== "@me"`) | `PATCH /users/@me/guilds/settings` | `USER_GUILD_SETTINGS_BULK` | `{ "guilds": { "<guild_id>": { ...partial } } }` | `mod-652215.pretty.js:94`, `rest-static.json:4428`, `@8333335` `[D]` |
| ЛС / DM (`guildId == null \|\| guildId === "@me"`) | `PATCH /users/@me/guilds/@me/settings` | `USER_GUILD_SETTINGS("@me")` | `{ ...partial }` | `mod-652215.pretty.js:94`, `rest-static.json:4442`, `@8333335` `[D]` |
| Одиночная гильдия (резервный маршрут) | `PATCH /users/@me/guilds/{guild_id}/settings` | `USER_GUILD_SETTINGS(id)` | `{ ...partial }` | Объявлен в `Endpoints`, но штатно клиентом для гильдий не вызывается `[D]` |

#### 1.6.3 Точные тела запросов по каждому UI-действию (`[D] NOTIFICATIONS-RE.md §9.3, §10`)

| UI-действие | Внутренний payload `<partial>` (внутри `guilds[guildId]` для гильдии) | Ref в бандле |
|---|---|---|
| Включить/выключить mute сервера тумблером | `{ "muted": true \| false }` | `046406f7a5feb3e2.js@11993` `[D]` |
| Выбрать окно длительности mute сервера | `{ "muted": true, "mute_config": { "selected_time_window": W, "end_time": ISO_OR_NULL } }` | `046406f7a5feb3e2.js@11993` `[D]` |
| Снять mute сервера из меню/модалки | `{ "muted": false, "mute_config": null }` | `046406f7a5feb3e2.js@11993` `[D]` |
| Сменить уровень уведомлений сервера (0 / 1 / 2) | `{ "message_notifications": 0 \| 1 \| 2 }` (при пресете старой модалки `+{ "flags": mD(flags, UNREADS_*) }`) | `@682193`, `@15696`, `@12220` `[D]` |
| Переключить `suppress_everyone` | `{ "suppress_everyone": true \| false }` | `046406f7a5feb3e2.js@5511` `[D]` |
| Переключить `suppress_roles` | `{ "suppress_roles": true \| false }` | `046406f7a5feb3e2.js@5511` `[D]` |
| Переключить `notify_highlights` | `{ "notify_highlights": 2 \| 1 }` (`2` = вкл., `1` = выкл.) | `046406f7a5feb3e2.js@5511, @17620` `[D]` |
| Переключить `mute_scheduled_events` | `{ "mute_scheduled_events": true \| false }` | `046406f7a5feb3e2.js@5511` `[D]` |
| Переключить `mobile_push` | `{ "mobile_push": true \| false }` | `046406f7a5feb3e2.js@5511` `[D]` |
| Добавить канал в таблицу overrides | `{ "channel_overrides": { "<ch_id>": { "muted": false, "message_notifications": resolvedLevel, "flags": mD(chFlags, resolvedUnreadBit) } } }` | `@682805`, `@18911` `[D]` |
| Выбрать радио уровня в строке канала (0 / 1 / 2) | `{ "channel_overrides": { "<ch_id>": { "message_notifications": 0 \| 1 \| 2, "muted": currentMuted } } }` | `@682805`, `@1228` `[D]` |
| Переключить mute канала (чекбокс или пресет) | `{ "channel_overrides": { "<ch_id>": { "message_notifications": currentOr3, "muted": true \| false, "mute_config": cfgOrNull } } }` | `@682805`, `@1228` `[D]` |
| Удалить (сбросить) override канала | `{ "channel_overrides": { "<ch_id>": { "message_notifications": 3, "muted": false, "flags": PW(chFlags) } } }` | `046406f7a5feb3e2.js@1622`, `@6230633` `[D]` |
| Переключить «Скрыть заглушённые каналы» | Локальный стор (`GUILD_TOGGLE_COLLAPSE_MUTED`, `@9393306`) — **по сети не отправляется** `[D]` | `@9393306` `[D]` |

---

## 2. DOM-иерархия и структура поверхностей UI

Вся функциональность N-05 размещается в 5 согласованных точках существующего DOM (`rust/gui-spike/ui/index.html`) без добавления новых корневых `.screen` (инвариант 4 экранов `DOM-CONTRACT` сохранён):

1. **Меню сервера `#server-menu`** (`index.html:202`) — пункты #3, #4, #5 и их вложенные поповеры-подменю `#guild-mute-submenu` и `#guild-notif-submenu`.
2. **Модальное окно настроек уведомлений сервера `#guild-notifications-dialog`** — монтируется в `document.body` рядом с `#join-server-dialog` и `#leave-server-dialog` (`index.html:206-207`) одним код-путём (совместимо с DOM-шимом тестов).
3. **Поповер быстрых настроек уведомлений канала `#channel-notif-menu`** — вызывается кнопкой колокольчика в `#chat-header` (`index.html:107`) или контекстным действием строки канала в `#channel-tree`.
4. **Индикаторы в `#guild-rail` (`index.html:37`), `#channel-tree` (`index.html:54`) и `#screen-chat` (`index.html:117`)** — бейджи упоминаний, точки непрочитанного, иконки приглушения и фильтрация скрытых каналов.
5. **Панель глобальных и локальных настроек уведомлений `#screen-settings` (`pane="notifications"`, `index.html:176`)** — глобальные переключатели ST-05.

### 2.1 Пункты #3, #4, #5 в `#server-menu` и подменю

В `#server-menu` (`features/actions.js:48-59`) пункты #3, #4 и #5 образуют третью группу меню (между вторым и третьим разделителями `.menu-sep`):

```html
<!-- Внутри #server-menu[role="menu"] -->
<!-- Пункт #3: Заглушить сервер -->
<button class="menu-item has-subtitle" type="button" role="menuitem"
        data-menu-index="2" data-action="guild-mute"
        aria-haspopup="menu" aria-expanded="false"
        aria-controls="guild-mute-submenu" tabindex="-1">
  <span class="menu-item-text">
    <span class="menu-item-label">Заглушить сервер</span>
    <!-- Видим только когда isMuted === true -->
    <span class="menu-item-subtitle" data-role="mute-until">До 20:00</span>
  </span>
  <span class="menu-item-chevron" aria-hidden="true">▸</span>
</button>

<!-- Пункт #4: Параметры уведомлений -->
<button class="menu-item has-subtitle" type="button" role="menuitem"
        data-menu-index="3" data-action="guild-notifications"
        aria-haspopup="menu" aria-expanded="false"
        aria-controls="guild-notif-submenu" tabindex="-1">
  <span class="menu-item-text">
    <span class="menu-item-label">Параметры уведомлений</span>
    <!-- Только из реальной проекции user_guild_settings; без данных — hidden (G05-REST-SPEC §2.2 #4) -->
    <span class="menu-item-subtitle" data-role="notif-level">Только @упоминания</span>
  </span>
  <span class="menu-item-chevron" aria-hidden="true">▸</span>
</button>

<!-- Пункт #5: Скрыть заглушённые каналы -->
<button class="menu-item" type="button" role="menuitemcheckbox"
        data-menu-index="4" data-action="guild-hide-muted"
        aria-checked="false" tabindex="-1">
  <span class="menu-item-text">
    <span class="menu-item-label">Скрыть заглушённые каналы</span>
  </span>
  <span class="menu-item-check" aria-hidden="true"></span>
</button>
<div class="menu-sep" role="separator"></div>
```

#### Подменю `#guild-mute-submenu` (6 пресетов таймера + снятие mute)

Монтируется внутри `#server-menu` (или рядом в `body` с `z-index: var(--z-popover)`) и открывается при активации/фокусе `→` на пункте #3:

```html
<div id="guild-mute-submenu" class="menu server-submenu" role="menu"
     aria-label="Заглушить сервер" hidden>
  <!-- Кнопка быстрого снятия рендерится первой только при isMuted === true -->
  <button class="menu-item" type="button" role="menuitem"
          data-action="guild-unmute" tabindex="-1" hidden>
    <span class="menu-item-text">
      <span class="menu-item-label">Включить уведомления сервера</span>
    </span>
  </button>
  <div class="menu-sep" role="separator" data-role="unmute-sep" hidden></div>

  <!-- 6 канонических пресетов из @6225743 (b8) и @8349873 (Xx) -->
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-mute-preset" data-window="900" data-preset-id="15-minutes"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 15 минут</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-mute-preset" data-window="3600" data-preset-id="1-hour"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 1 час</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-mute-preset" data-window="10800" data-preset-id="3-hours"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 3 часа</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-mute-preset" data-window="28800" data-preset-id="8-hours"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 8 часов</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-mute-preset" data-window="86400" data-preset-id="24-hours"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 24 часа</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-mute-preset" data-window="-1" data-preset-id="forever"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Пока не включу</span></span>
  </button>
</div>
```

#### Подменю `#guild-notif-submenu` (быстрый выбор уровня + вход в модалку)

```html
<div id="guild-notif-submenu" class="menu server-submenu" role="menu"
     aria-label="Параметры уведомлений" hidden>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-notif-level" data-level="0"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Все сообщения</span></span>
    <span class="menu-item-radio" aria-hidden="true"></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-notif-level" data-level="1"
          aria-checked="true" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Только @упоминания</span></span>
    <span class="menu-item-radio" aria-hidden="true"></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="guild-notif-level" data-level="2"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Ничего</span></span>
    <span class="menu-item-radio" aria-hidden="true"></span>
  </button>
  <div class="menu-sep" role="separator"></div>
  <button class="menu-item" type="button" role="menuitem"
          data-action="open-guild-notification-modal"
          aria-haspopup="dialog" tabindex="-1">
    <span class="menu-item-text">
      <span class="menu-item-label">Настройки уведомлений сервера…</span>
    </span>
  </button>
</div>
```

### 2.2 Модальное окно настроек уведомлений сервера `#guild-notifications-dialog`

Структура строго следует контракту модалки `NotificationSettingsModal` из `discord.com_assets_046406f7a5feb3e2.js` (`[D] NOTIFICATIONS-RE.md §10`):

```html
<div id="guild-notifications-dialog" class="dialog-backdrop" role="presentation" hidden>
  <div class="dialog notif-modal" role="dialog" aria-modal="true"
       aria-labelledby="guild-notif-title"
       aria-describedby="guild-notif-subtitle"
       data-state="ready" data-guild-id="1014537851418980372">

    <!-- 1. Шапка модалки -->
    <header class="notif-modal-header">
      <div class="notif-modal-title-group">
        <h2 id="guild-notif-title" class="dialog-title">Параметры уведомлений</h2>
        <p id="guild-notif-subtitle" class="dialog-hint">ITSKIDON PC BOOST</p>
      </div>
      <button class="icon-btn dialog-close" type="button"
              data-action="guild-notif-close" aria-label="Закрыть">
        <span aria-hidden="true">✕</span>
      </button>
    </header>

    <!-- 2. Секция приглушения сервера (mute + 6 окон длительности) -->
    <section class="notif-section" role="group" aria-labelledby="guild-notif-mute-heading">
      <h3 id="guild-notif-mute-heading" class="visually-hidden">Приглушение сервера</h3>
      <label class="toggle" for="guild-notif-mute-switch">
        <span class="toggle-label">
          <span>Заглушить сервер</span>
          <span class="toggle-hint">Отключает звуковые и всплывающие уведомления со всего сервера</span>
        </span>
        <input id="guild-notif-mute-switch" class="toggle-input" type="checkbox"
               role="switch" data-setting="muted" aria-checked="false" />
        <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
      </label>

      <!-- Показывается только когда muted === true (или выбирается длительность) -->
      <div id="guild-notif-mute-window-row" class="settings-field notif-mute-window" hidden>
        <label id="guild-notif-window-label" class="settings-field-label"
               for="guild-notif-window-select">Длительность заглушения</label>
        <select id="guild-notif-window-select" class="settings-select"
                data-setting="selected_time_window"
                aria-labelledby="guild-notif-window-label">
          <option value="900">На 15 минут</option>
          <option value="3600">На 1 час</option>
          <option value="10800">На 3 часа</option>
          <option value="28800">На 8 часов</option>
          <option value="86400">На 24 часа</option>
          <option value="-1">Пока не включу</option>
        </select>
        <span id="guild-notif-mute-expiry" class="settings-slider-value"
              data-role="mute-expiry-text">Бессрочно</span>
      </div>
    </section>

    <!-- 3. Радиогруппа базового уровня уведомлений сервера (0 / 1 / 2) -->
    <section class="notif-section" role="radiogroup"
             aria-labelledby="guild-notif-level-heading">
      <h3 id="guild-notif-level-heading" class="notif-section-title">
        Настройки уведомлений сервера
      </h3>
      <div class="notif-level-list">
        <button class="notif-level-card" type="button" role="radio"
                data-action="modal-guild-level" data-level="0"
                aria-checked="false" tabindex="-1">
          <span class="notif-radio-indicator" aria-hidden="true"></span>
          <span class="notif-level-body">
            <span class="notif-level-title">Все сообщения</span>
            <span class="notif-level-desc">Уведомления о каждом сообщении в незаглушённых каналах</span>
          </span>
        </button>
        <button class="notif-level-card" type="button" role="radio"
                data-action="modal-guild-level" data-level="1"
                aria-checked="true" tabindex="0">
          <span class="notif-radio-indicator" aria-hidden="true"></span>
          <span class="notif-level-body">
            <span class="notif-level-title">Только @упоминания</span>
            <span class="notif-level-desc">Уведомления только при упоминании вас или вашей роли</span>
          </span>
        </button>
        <button class="notif-level-card" type="button" role="radio"
                data-action="modal-guild-level" data-level="2"
                aria-checked="false" tabindex="-1">
          <span class="notif-radio-indicator" aria-hidden="true"></span>
          <span class="notif-level-body">
            <span class="notif-level-title">Ничего</span>
            <span class="notif-level-desc">Никаких звуковых или всплывающих уведомлений</span>
          </span>
        </button>
      </div>
      <!-- Предупреждение Dh5p5j: показывается при выборе level=0 на сервере с memberCount >= 2500 ([D] §10) -->
      <p id="guild-notif-large-warning" class="notif-large-guild-banner" role="note" hidden>
        На этом сервере много участников — включение «Всех сообщений» может привести к частому потоку уведомлений.
      </p>
    </section>

    <!-- 4. Тумблеры подавления упоминаний и дополнительных каналов доставки -->
    <section class="notif-section" role="group"
             aria-labelledby="guild-notif-suppress-heading">
      <h3 id="guild-notif-suppress-heading" class="notif-section-title">
        Подавление упоминаний и события
      </h3>

      <label class="toggle" for="guild-notif-suppress-everyone">
        <span class="toggle-label">
          <span>Подавлять @everyone и @here</span>
          <span class="toggle-hint">Отключает уведомления и бейджи от упоминаний @everyone и @here</span>
        </span>
        <input id="guild-notif-suppress-everyone" class="toggle-input" type="checkbox"
               role="switch" data-setting="suppress_everyone" aria-checked="false" />
        <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
      </label>

      <label class="toggle" for="guild-notif-suppress-roles">
        <span class="toggle-label">
          <span>Подавлять все @роли</span>
          <span class="toggle-hint">Отключает уведомления и бейджи от упоминаний любых ролей</span>
        </span>
        <input id="guild-notif-suppress-roles" class="toggle-input" type="checkbox"
               role="switch" data-setting="suppress_roles" aria-checked="false" />
        <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
      </label>

      <label class="toggle" for="guild-notif-highlights">
        <span class="toggle-label">
          <span>Уведомления о хайлайтах</span>
          <span class="toggle-hint">Сводки активных обсуждений с участием друзей на сервере</span>
        </span>
        <input id="guild-notif-highlights" class="toggle-input" type="checkbox"
               role="switch" data-setting="notify_highlights" aria-checked="true" />
        <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
      </label>

      <label class="toggle" for="guild-notif-scheduled-events">
        <span class="toggle-label">
          <span>Заглушить новые события</span>
          <span class="toggle-hint">Отключает уведомления о старте запланированных мероприятий</span>
        </span>
        <input id="guild-notif-scheduled-events" class="toggle-input" type="checkbox"
               role="switch" data-setting="mute_scheduled_events" aria-checked="false" />
        <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
      </label>

      <label class="toggle" for="guild-notif-mobile-push">
        <span class="toggle-label">
          <span>Мобильные push-уведомления</span>
          <span class="toggle-hint">Отправлять уведомления на мобильные устройства, когда вы не за ПК</span>
        </span>
        <input id="guild-notif-mobile-push" class="toggle-input" type="checkbox"
               role="switch" data-setting="mobile_push" aria-checked="true" />
        <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
      </label>
    </section>

    <!-- 5. Таблица переопределений каналов и категорий (channel_overrides) -->
    <section class="notif-section" role="group"
             aria-labelledby="guild-notif-overrides-heading">
      <h3 id="guild-notif-overrides-heading" class="notif-section-title">
        Переопределения уведомлений каналов
      </h3>

      <!-- Селектор добавления канала или категории в таблицу -->
      <div class="notif-override-toolbar">
        <label class="visually-hidden" for="notif-override-channel-select">
          Выберите канал или категорию для переопределения
        </label>
        <select id="notif-override-channel-select" class="settings-select"
                aria-label="Выберите канал или категорию для переопределения">
          <option value="">Выберите канал или категорию…</option>
          <!-- Опции формируются динамически из списка каналов гильдии -->
        </select>
        <button class="btn btn-secondary" type="button"
                data-action="notif-override-add">Добавить</button>
      </div>

      <!-- Таблица с 5 каноническими колонками из [D] NOTIFICATIONS-RE.md §10:
           1) Канал (uShwWv), 2) Все (hZrr6k), 3) Упоминания (JzbSEY),
           4) Ничего (y59NJm), 5) Mute (pGn/bJ) -->
      <div class="notif-override-table-wrap">
        <table class="notif-override-table" aria-labelledby="guild-notif-overrides-heading">
          <thead>
            <tr>
              <th scope="col" class="col-channel">Канал или категория</th>
              <th scope="col" class="col-radio">Все</th>
              <th scope="col" class="col-radio">Упоминания</th>
              <th scope="col" class="col-radio">Ничего</th>
              <th scope="col" class="col-mute">Mute</th>
            </tr>
          </thead>
          <tbody id="notif-override-tbody">
            <!-- Пример строки переопределения канала -->
            <tr class="notif-override-row" data-channel-id="1173958353802776576">
              <th scope="row" class="notif-override-cell-channel">
                <span class="channel-icon" aria-hidden="true">#</span>
                <span class="notif-override-channel-meta">
                  <span class="notif-override-channel-name">объявления</span>
                  <span class="notif-override-category-name">НОВОСТИ</span>
                </span>
              </th>
              <td class="notif-override-cell-radio">
                <button class="notif-cell-radio" type="button" role="radio"
                        data-action="override-set-level"
                        data-channel-id="1173958353802776576" data-level="0"
                        aria-checked="false" tabindex="-1"
                        aria-label="объявления: Все сообщения"></button>
              </td>
              <td class="notif-override-cell-radio">
                <button class="notif-cell-radio" type="button" role="radio"
                        data-action="override-set-level"
                        data-channel-id="1173958353802776576" data-level="1"
                        aria-checked="true" tabindex="0"
                        aria-label="объявления: Только @упоминания"></button>
              </td>
              <td class="notif-override-cell-radio">
                <button class="notif-cell-radio" type="button" role="radio"
                        data-action="override-set-level"
                        data-channel-id="1173958353802776576" data-level="2"
                        aria-checked="false" tabindex="-1"
                        aria-label="объявления: Ничего"></button>
              </td>
              <td class="notif-override-cell-mute">
                <button class="notif-cell-checkbox" type="button" role="checkbox"
                        data-action="override-toggle-mute"
                        data-channel-id="1173958353802776576"
                        aria-checked="false" tabindex="0"
                        aria-label="Заглушить канал объявления"></button>
                <button class="icon-btn notif-override-remove" type="button"
                        data-action="override-remove"
                        data-channel-id="1173958353802776576" tabindex="0"
                        aria-label="Сбросить настройки канала объявления"
                        title="Сбросить переопределение">
                  <span aria-hidden="true">✕</span>
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <p id="notif-override-empty" class="dialog-hint notif-override-empty" hidden>
          Нет переопределённых каналов — для всех каналов действуют правила сервера.
        </p>
      </div>
    </section>

    <!-- 6. Ошибка сохранения и действия диалога -->
    <p id="guild-notif-error" class="dialog-error" role="alert" hidden></p>
    <footer class="dialog-actions">
      <button class="btn btn-primary" type="button" data-action="guild-notif-close">
        Готово
      </button>
    </footer>
  </div>
</div>
```

### 2.3 Поповер уведомлений канала `#channel-notif-menu`

Для быстрой настройки отдельного канала без открытия большой модалки (или по клику на кнопку колокольчика в `#chat-header` / кнопку настройки на строке `.channel-item`) используется компактный поповер `#channel-notif-menu`:

```html
<div id="channel-notif-menu" class="menu server-menu channel-notif-menu" role="menu"
     aria-label="Уведомления канала" data-channel-id="" hidden>
  <!-- Приглушение канала: при isChannelMuted === false раскрывает 6 пресетов,
       при isChannelMuted === true позволяет сразу снять mute -->
  <button class="menu-item" type="button" role="menuitem"
          data-action="channel-unmute" tabindex="-1" hidden>
    <span class="menu-item-text">
      <span class="menu-item-label">Включить уведомления канала</span>
      <span class="menu-item-subtitle" data-role="channel-mute-until"></span>
    </span>
  </button>

  <div class="menu-group-heading" role="presentation">Заглушить канал</div>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-mute-preset" data-window="900"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 15 минут</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-mute-preset" data-window="3600"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 1 час</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-mute-preset" data-window="10800"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 3 часа</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-mute-preset" data-window="28800"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 8 часов</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-mute-preset" data-window="86400"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">На 24 часа</span></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-mute-preset" data-window="-1"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Пока не включу</span></span>
  </button>

  <div class="menu-sep" role="separator"></div>
  <div class="menu-group-heading" role="presentation">Уровень уведомлений</div>

  <!-- 3 = NULL («По умолчанию категории/сервера») -->
  <button class="menu-item has-subtitle" type="button" role="menuitemradio"
          data-action="channel-notif-level" data-level="3"
          aria-checked="true" tabindex="-1">
    <span class="menu-item-text">
      <span class="menu-item-label">По умолчанию</span>
      <span class="menu-item-subtitle" data-role="channel-inherited-label">Только @упоминания</span>
    </span>
    <span class="menu-item-radio" aria-hidden="true"></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-notif-level" data-level="0"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Все сообщения</span></span>
    <span class="menu-item-radio" aria-hidden="true"></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-notif-level" data-level="1"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Только @упоминания</span></span>
    <span class="menu-item-radio" aria-hidden="true"></span>
  </button>
  <button class="menu-item" type="button" role="menuitemradio"
          data-action="channel-notif-level" data-level="2"
          aria-checked="false" tabindex="-1">
    <span class="menu-item-text"><span class="menu-item-label">Ничего</span></span>
    <span class="menu-item-radio" aria-hidden="true"></span>
  </button>
</div>
```

### 2.4 DOM-разметка индикаторов в `#channel-tree` и `#guild-rail`

#### 2.4.1 Строка канала в `#channel-tree` (`index.html:64-67`, `tree.css:92-154`)

```html
<!-- Обычный непрочитанный канал с 3 упоминаниями -->
<li>
  <button class="channel-item is-unread has-mention" type="button"
          data-channel-id="1173958353802776576" data-channel-type="0"
          data-openable="true" data-unread="true" data-muted="false"
          data-mention-count="3" data-notif-level="3"
          aria-label="объявления, непрочитано, 3 упоминания">
    <span class="channel-icon" aria-hidden="true">#</span>
    <span class="channel-name">объявления</span>
    <span class="mention-badge" aria-hidden="true">3</span>
  </button>
</li>

<!-- Заглушённый канал без упоминаний (скрывается при #channel-tree[data-hide-muted="true"]) -->
<li>
  <button class="channel-item is-muted" type="button"
          data-channel-id="1173958353802776577" data-channel-type="0"
          data-openable="true" data-unread="false" data-muted="true"
          data-mention-count="0" data-notif-level="2"
          aria-label="флудилка, канал заглушён">
    <span class="channel-icon" aria-hidden="true">#</span>
    <span class="channel-name">флудилка</span>
    <span class="channel-mute-icon" aria-hidden="true" title="Канал заглушён">🔕</span>
  </button>
</li>
```

#### 2.4.2 Плитка гильдии в `#guild-rail` (`rail.css:54-85`)

```html
<button class="guild-item" type="button"
        data-guild-id="1014537851418980372"
        data-unread="true" data-muted="false" data-mention-count="5"
        aria-label="Сервер ITSKIDON PC BOOST, непрочитано, 5 упоминаний">
  <span class="guild-monogram" aria-hidden="true">IP</span>
  <span class="guild-mention-badge" aria-hidden="true">5</span>
</button>
```

### 2.5 Раздел «Уведомления» в `#screen-settings` (`pane="notifications"`, ST-05)

В `#screen-settings` (`features/settings.js:751-759`) панель `notifications` объединяет подтверждённые ключи `NotificationSettingsStore` (`[D] NOTIFICATIONS-RE.md §8`, `@9225345`), глобальные флаги аккаунта (`@6216165`) и сводку `user_guild_settings` из `data.rs:2207-2220`:

```html
<section class="settings-pane" data-pane="notifications" role="region" aria-label="Уведомления">
  <h3>Уведомления</h3>
  <h4 class="settings-block-title">Уведомления на рабочем столе и в трее</h4>

  <label class="toggle" for="setting-notif-desktop">
    <span class="toggle-label">
      <span>Включить уведомления на рабочем столе</span>
      <span class="toggle-hint">Системные всплывающие уведомления (desktopType: ALL / NEVER)</span>
    </span>
    <input id="setting-notif-desktop" class="toggle-input" type="checkbox"
           role="switch" data-setting-id="notifications.desktop_enabled" aria-checked="true" />
    <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
  </label>

  <label class="toggle" for="setting-notif-unread-badge">
    <span class="toggle-label">
      <span>Включить значок непрочитанных сообщений</span>
      <span class="toggle-hint">Показывать красный индикатор на иконке приложения в панели задач</span>
    </span>
    <input id="setting-notif-unread-badge" class="toggle-input" type="checkbox"
           role="switch" data-setting-id="notifications.unread_badge" aria-checked="true" />
    <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
  </label>

  <label class="toggle" for="setting-notif-taskbar-flash">
    <span class="toggle-label">
      <span>Мигать на панели задач</span>
      <span class="toggle-hint">Привлекать внимание при получении нового уведомления (taskbarFlash)</span>
    </span>
    <input id="setting-notif-taskbar-flash" class="toggle-input" type="checkbox"
           role="switch" data-setting-id="notifications.taskbar_flash" aria-checked="true" />
    <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
  </label>

  <label class="toggle" for="setting-notif-selected-channel">
    <span class="toggle-label">
      <span>Уведомлять о сообщениях в открытом канале</span>
      <span class="toggle-hint">Звук для сообщений в текущем активном канале (notifyMessagesInSelectedChannel)</span>
    </span>
    <input id="setting-notif-selected-channel" class="toggle-input" type="checkbox"
           role="switch" data-setting-id="notifications.selected_channel" aria-checked="false" />
    <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
  </label>

  <h4 class="settings-block-title">Звуки и озвучивание текста (TTS)</h4>

  <label class="toggle" for="setting-notif-disable-all-sounds">
    <span class="toggle-label">
      <span>Отключить все звуки уведомлений</span>
      <span class="toggle-hint">Глобальное отключение звуковых эффектов (disableAllSounds)</span>
    </span>
    <input id="setting-notif-disable-all-sounds" class="toggle-input" type="checkbox"
           role="switch" data-setting-id="notifications.disable_all_sounds" aria-checked="false" />
    <span class="toggle-track" aria-hidden="true"><span class="toggle-thumb"></span></span>
  </label>

  <div class="settings-field">
    <label id="setting-notif-tts-label" class="settings-field-label"
           for="setting-notif-tts-select">Уведомления Text-to-Speech</label>
    <select id="setting-notif-tts-select" class="settings-select"
            data-setting-id="notifications.tts_type"
            aria-labelledby="setting-notif-tts-label">
      <option value="ALL_CHANNELS">Для всех каналов</option>
      <option value="SELECTED_CHANNEL">Для текущего выбранного канала</option>
      <option value="NEVER">Никогда</option>
    </select>
    <span class="settings-slider-value" aria-hidden="true"></span>
  </div>
</section>
```

---

## 3. Спецификация CSS и привязка к токенам (`rust/gui-spike/ui/tokens.css`)

### 3.1 Реестр используемых токенов `tokens.css`

Каждая переменная ниже проверена на точное присутствие в `rust/gui-spike/ui/tokens.css`:

| Категория | Токены (`var(--...)`) |
|---|---|
| Поверхности | `--bg-app`, `--bg-chat`, `--surface-1`, `--surface-2`, `--surface-3`, `--surface-4`, `--surface-input` |
| Текст | `--text-strong`, `--text`, `--text-subtle`, `--text-muted`, `--text-faint`, `--text-accent`, `--text-on-accent`, `--text-on-danger`, `--text-on-warning` |
| Акцент и статусы | `--accent`, `--accent-hover`, `--accent-soft`, `--danger`, `--danger-text`, `--danger-fill`, `--danger-fill-hover`, `--warning` |
| Границы и состояния | `--border-subtle`, `--border`, `--border-strong`, `--divider`, `--state-hover`, `--state-active`, `--state-selected`, `--state-disabled-opacity`, `--focus-ring`, `--focus-ring-width`, `--focus-ring-offset` |
| Типографика | `--font-sans`, `--font-size-caption`, `--font-size-small`, `--font-size-ui`, `--font-size-body`, `--font-size-title`, `--line-height-caption`, `--line-height-small`, `--line-height-ui`, `--line-height-body`, `--line-height-title`, `--font-weight-normal`, `--font-weight-medium`, `--font-weight-semibold`, `--font-weight-bold`, `--letter-spacing-caps` |
| Отступы и размеры | `--space-0`…`--space-9`, `--space-11`, `--radius-xs`, `--radius-sm`, `--radius-md`, `--radius-round`, `--btn-height-sm`, `--btn-height`, `--btn-height-lg`, `--btn-pad-x`, `--btn-radius`, `--input-height`, `--switch-w`, `--switch-h`, `--switch-thumb`, `--badge-h`, `--badge-pad-x`, `--icon-size-sm`, `--icon-size`, `--row-channel`, `--row-menu-item`, `--menu-pad`, `--menu-border`, `--menu-radius`, `--modal-w-md`, `--modal-pad-x`, `--modal-pad-y`, `--unread-banner-h` |
| Слои, тени, движение | `--z-popover`, `--z-modal`, `--shadow-popover`, `--motion-fast`, `--motion-base`, `--ease-out`, `--ease-in-out` |

### 3.2 Нормативные CSS-правила компонентов уведомлений

```css
/* ==========================================================================
 * 1. Модалка настроек уведомлений сервера (#guild-notifications-dialog)
 * ========================================================================== */
.dialog.notif-modal {
  width: min(var(--modal-w-md), calc(100vw - var(--space-11)));
  gap: var(--space-5);
}

.notif-modal-header {
  position: relative;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
  padding-bottom: var(--space-4);
  border-bottom: var(--menu-border) solid var(--divider);
}

.notif-modal-title-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.notif-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding-bottom: var(--space-4);
  border-bottom: var(--menu-border) solid var(--divider);
}

.notif-section-title {
  margin: 0;
  color: var(--text-subtle);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-caps);
  line-height: var(--line-height-small);
  text-transform: uppercase;
}

.notif-mute-window {
  padding-top: var(--space-2);
}

/* Радио-карточки базового уровня уведомлений гильдии (0 / 1 / 2) */
.notif-level-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.notif-level-card {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  width: 100%;
  min-height: var(--space-11);
  padding: var(--space-3) var(--space-5);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-3);
  color: var(--text);
  text-align: left;
  cursor: pointer;
  transition:
    background var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out);
}

.notif-level-card:hover {
  background: var(--surface-4);
}

.notif-level-card[aria-checked="true"] {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--text-strong);
}

.notif-level-card:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.notif-radio-indicator {
  flex: 0 0 auto;
  width: var(--icon-size);
  height: var(--icon-size);
  border: var(--focus-ring-width) solid var(--border-strong);
  border-radius: var(--radius-round);
  background: transparent;
}

.notif-level-card[aria-checked="true"] .notif-radio-indicator {
  border-color: var(--accent);
  background: var(--accent);
  box-shadow: inset 0 0 0 var(--space-2) var(--surface-2);
}

.notif-level-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.notif-level-title {
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
}

.notif-level-desc {
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

/* Предупреждение Dh5p5j для серверов >= 2500 участников:
 * заливка --warning + текст --text-on-warning (THEME-SPEC §2.5: контраст 10.76 / 7.24) */
.notif-large-guild-banner {
  margin: var(--space-2) 0 0;
  padding: var(--space-3) var(--space-5);
  border-radius: var(--radius-sm);
  background: var(--warning);
  color: var(--text-on-warning);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-small);
}

/* ==========================================================================
 * 2. Таблица переопределений каналов (.notif-override-table)
 * ========================================================================== */
.notif-override-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.notif-override-toolbar .btn {
  flex: 0 0 auto;
  height: var(--input-height);
  padding: 0 var(--btn-pad-x);
  border-radius: var(--radius-xs);
}

.notif-override-table-wrap {
  max-height: calc(var(--space-11) * 5);
  overflow-y: auto;
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.notif-override-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.notif-override-table thead th {
  position: sticky;
  top: var(--space-0);
  z-index: var(--z-header);
  padding: var(--space-3) var(--space-4);
  border-bottom: var(--menu-border) solid var(--divider);
  background: var(--surface-2);
  color: var(--text-subtle);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-caps);
  line-height: var(--line-height-caption);
  text-transform: uppercase;
}

.notif-override-table .col-channel {
  text-align: left;
}

.notif-override-table .col-radio,
.notif-override-table .col-mute {
  width: var(--space-13);
  text-align: center;
}

.notif-override-row {
  border-bottom: var(--menu-border) solid var(--divider);
}

.notif-override-row:hover {
  background: var(--state-hover);
}

.notif-override-cell-channel {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  font-weight: var(--font-weight-normal);
  text-align: left;
}

.notif-override-channel-meta {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.notif-override-channel-name {
  overflow: hidden;
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.notif-override-category-name {
  overflow: hidden;
  color: var(--text-muted);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.notif-override-cell-radio,
.notif-override-cell-mute {
  padding: var(--space-2);
  text-align: center;
  vertical-align: middle;
}

.notif-cell-radio,
.notif-cell-checkbox {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--icon-size);
  height: var(--icon-size);
  padding: var(--space-0);
  border: var(--focus-ring-width) solid var(--border-strong);
  background: transparent;
  cursor: pointer;
}

.notif-cell-radio {
  border-radius: var(--radius-round);
}

.notif-cell-checkbox {
  border-radius: var(--radius-xs);
}

.notif-cell-radio[aria-checked="true"] {
  border-color: var(--accent);
  background: var(--accent);
  box-shadow: inset 0 0 0 var(--space-1) var(--surface-1);
}

.notif-cell-checkbox[aria-checked="true"] {
  border-color: var(--danger);
  background: var(--danger-fill);
  color: var(--text-on-danger);
}

.notif-override-remove {
  width: var(--btn-height-sm);
  height: var(--btn-height-sm);
  margin-left: var(--space-2);
  font-size: var(--icon-size-sm);
}

/* ==========================================================================
 * 3. Подменю в #server-menu и радио-индикатор пункта меню
 * ========================================================================== */
.server-submenu {
  position: fixed;
  z-index: var(--z-popover);
}

.menu-group-heading {
  padding: var(--space-2) var(--menu-pad) var(--space-1);
  color: var(--text-muted);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-caps);
  line-height: var(--line-height-caption);
  text-transform: uppercase;
}

.menu-item-chevron {
  flex: 0 0 auto;
  color: var(--text-subtle);
  font-size: var(--font-size-small);
}

.menu-item-radio {
  display: inline-flex;
  flex: 0 0 auto;
  width: var(--icon-size);
  height: var(--icon-size);
  border: var(--focus-ring-width) solid var(--border-strong);
  border-radius: var(--radius-round);
}

.menu-item[aria-checked="true"] .menu-item-radio {
  border-color: var(--accent);
  background: var(--accent);
  box-shadow: inset 0 0 0 var(--space-1) var(--surface-3);
}

/* ==========================================================================
 * 4. Индикаторы непрочитанного, упоминаний и mute в #channel-tree и #guild-rail
 * ========================================================================== */
/* Бейдж числа упоминаний в строке канала: --danger-fill + --text-on-danger (4.85:1) */
.mention-badge {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  min-width: var(--badge-h);
  height: var(--badge-h);
  margin-left: auto;
  padding: 0 var(--badge-pad-x);
  border-radius: var(--radius-round);
  background: var(--danger-fill);
  color: var(--text-on-danger);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-bold);
  line-height: 1;
}

/* Если у канала есть .mention-badge, белая точка ::after не дублируется */
.channel-item.has-mention::after,
.channel-item.is-muted::after {
  display: none;
}

.channel-mute-icon {
  flex: 0 0 auto;
  margin-left: auto;
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: 1;
}

/* Фильтрация при включённом «Скрыть заглушённые каналы» (п.5 #server-menu):
 * скрывает заглушённые каналы без упоминаний, если они не являются текущим открытым каналом */
#channel-tree[data-hide-muted="true"] .channel-item.is-muted:not([aria-current="page"]):not(.has-mention) {
  display: none;
}

/* Индикаторы плитки гильдии в #guild-rail */
.guild-item {
  position: relative;
}

/* Левая белая пилюля непрочитанного сервера (только если сервер не заглушён и не выбран) */
.guild-item[data-unread="true"]:not([data-muted="true"]):not(.is-selected)::before {
  position: absolute;
  top: 50%;
  left: calc(-1 * var(--space-4));
  width: var(--space-2);
  height: var(--space-4);
  border-radius: var(--radius-round);
  background: var(--text-strong);
  transform: translateY(-50%);
  content: "";
}

/* Бейдж упоминаний на плитке гильдии */
.guild-mention-badge {
  position: absolute;
  right: calc(-1 * var(--space-1));
  bottom: calc(-1 * var(--space-1));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: var(--badge-h);
  height: var(--badge-h);
  padding: 0 var(--badge-pad-x);
  border: var(--focus-ring-width) solid var(--bg-app);
  border-radius: var(--radius-round);
  background: var(--danger-fill);
  color: var(--text-on-danger);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-bold);
  line-height: 1;
}

.guild-mention-badge[hidden],
.mention-badge[hidden] {
  display: none;
}
```

---

## 4. Машина состояний и визуальная матрица индикаторов

### 4.1 Жизненный цикл мутации настройки (`idle → pending → ready | error`)

Каждое изменение серверной настройки уведомлений (в `#server-menu`, `#guild-notifications-dialog`, `#channel-notif-menu`) проходит через детерминированный конечный автомат (`G05-REST-SPEC.md` §3.0):

| Состояние (`data-state`) | Поведение UI и блокировки | Переход при `2xx` / Gateway | Переход при ошибке |
|---|---|---|---|
| `idle` / `ready` | Контролы активны, отражают текущий снимок `UserGuildSettingsEntry` из стора | При клике/Enter/Space → `pending` | — |
| `pending` | Повторная отправка того же контрола заблокирована (`aria-busy="true"`, `disabled` на кнопке/селекте). **Никаких оптимистичных переключений `aria-checked` до ответа!** | Обновить стор, перерисовать `aria-checked`, subtitle, индикаторы `#channel-tree`/`#guild-rail`, объявить в `#server-status` (`role="status"`), перейти в `ready` | Откатить/сохранить прежнее состояние, показать типизированную ошибку в `#guild-notif-error` (или `#server-status`), перейти в `error` |
| `error` | Отображается текст отказа (`--danger-text`); при `429` контрол остаётся `disabled` на время посекундного таймера `retry_after_ms` (`Повторите через {n} с.`) | При истечении `429` до `0` разблокируется ровно один ручной повтор (`ready`) | При `401` (`unauthorized` / `missingToken`) — закрыть меню/модалку и открыть `#auth-gate` |

### 4.2 Матрица состояний строки канала (`.channel-item`) и плитки гильдии (`.guild-item`)

| Состояние данных (`read_state` + `user_guild_settings`) | Классы и атрибуты `.channel-item` | Цвет текста `.channel-name` | Правый индикатор канала | Плитка `.guild-item` в `#guild-rail` |
|---|---|---|---|---|
| Прочитано, не заглушено | `data-unread="false" data-muted="false"` | `var(--text-muted)` (при hover `var(--text)`) | нет | Без пилюли и без бейджа |
| Выбранный канал (`aria-current="page"`) | `aria-current="page"` | `var(--text-strong)` на фоне `var(--state-selected)` | `.mention-badge`, если `mention_count > 0` | Активная плитка `.is-selected` (`var(--accent)`) |
| Непрочитано (`last_message_id > last_acked_id`), `mention_count = 0`, `!isMuted`, `resolvedUnread = ALL_MESSAGES` | `.is-unread`, `data-unread="true"` | `var(--text-strong)`, `var(--font-weight-semibold)` | Круглая точка `::after` (`--space-2` × `--space-2`, `var(--text-strong)`) | Левая пилюля `::before` (`data-unread="true"`), если `!isGuildMuted` |
| Есть упоминания (`mention_count > 0`, не подавлены `suppress_*`), `!isMuted` | `.is-unread.has-mention`, `data-mention-count="{N}"` | `var(--text-strong)`, `var(--font-weight-semibold)` | Бейдж `.mention-badge` (`{N}`, при `N > 99` → `99+`) на `--danger-fill` | Бейдж `.guild-mention-badge` с суммой упоминаний по гильдии |
| Заглушено (`isChannelMuted \|\| isCategoryMuted`), `mention_count = 0` | `.is-muted`, `data-muted="true"` | `var(--text-muted)` с `opacity: var(--state-disabled-opacity)` (`0.45`) | Иконка `🔕` `.channel-mute-icon`; точка `::after` скрыта | Если `isGuildMuted` — левая пилюля `::before` скрыта (`data-muted="true"`) |
| Заглушено (`isChannelMuted`), но есть прямое упоминание (`mention_count > 0`) | `.is-muted.has-mention`, `data-muted="true" data-mention-count="{N}"` | `var(--text-muted)` с `opacity: var(--state-disabled-opacity)` | Бейдж `.mention-badge` (`{N}`) остаётся видимым (канал **не скрывается** при `hide_muted_channels=true`) | Бейдж `.guild-mention-badge` остаётся видимым даже при `data-muted="true"` |

---

## 5. Клавиатурная навигация и WAI-ARIA контракт

### 5.1 Таблица ARIA-ролей и атрибутов

| Узел DOM | Роль (`role`) | Обязательные ARIA-атрибуты | Доступное имя и динамика |
|---|---|---|---|
| `#server-menu` | `menu` | `aria-label="Меню сервера"` | Открывается кнопкой `button[data-action="guild-menu"]` (`aria-haspopup="menu"`, `aria-expanded="true\|false"`) |
| Пункты #3 и #4 в `#server-menu` | `menuitem` | `aria-haspopup="menu"`, `aria-expanded="true\|false"`, `aria-controls="guild-mute-submenu \| guild-notif-submenu"` | Вторая строка `.menu-item-subtitle` входит в вычисляемое имя кнопки |
| Пункт #5 в `#server-menu` | `menuitemcheckbox` | `aria-checked="true\|false"` | Синхронен с `#channel-tree[data-hide-muted]` и визуальным `.menu-item-check` |
| `#guild-mute-submenu`, `#guild-notif-submenu`, `#channel-notif-menu` | `menu` | `aria-label` | Вложенные меню и поповер канала; пресеты и уровни имеют `role="menuitemradio"` и `aria-checked="true\|false"`. Триггер `#channel-notif-menu` в `#chat-header` (`button[data-action="channel-notif-menu"]`) несёт `aria-haspopup="menu"`, `aria-expanded="true\|false"`, `aria-controls="channel-notif-menu"` |
| `.dialog.notif-modal` в `#guild-notifications-dialog` | `dialog` | `aria-modal="true"`, `aria-labelledby="guild-notif-title"`, `aria-describedby="guild-notif-subtitle"` | Захватывает фокус (focus trap) на время открытия |
| Тумблеры в `.notif-modal` и `#screen-settings` | `switch` | `aria-checked="true\|false"` на `<input type="checkbox" class="toggle-input" role="switch">` | Связаны через `<label class="toggle" for="...">`; `aria-checked` обновляется только после подтверждения изменения |
| Секция выбора уровня гильдии | `radiogroup` | `aria-labelledby="guild-notif-level-heading"` | Три карточки `.notif-level-card[role="radio"]` с `aria-checked="true\|false"` и roving `tabindex` |
| Таблица `.notif-override-table` | Нативная `<table>` (`table`) | `aria-labelledby="guild-notif-overrides-heading"` | Заголовки колонок `<th scope="col">`, название канала `<th scope="row">`, три радиокнопки уровня `.notif-cell-radio[role="radio"]` в строке с roving `tabindex` (`"0"` на выбранной `[aria-checked="true"]`, `"-1"` на остальных), чекбокс mute `.notif-cell-checkbox[role="checkbox"]` (`tabindex="0"`, `aria-checked="true\|false"`), кнопка сброса `.notif-override-remove` (`tabindex="0"`, `aria-label="Сбросить настройки канала {name}"`) |
| `.channel-item` в `#channel-tree` | Нативная `<button>` | `aria-current="page"` (у активного), `aria-label` | `aria-label` включает статус: `"{name}, непрочитано, {N} упоминаний"` или `"{name}, канал заглушён"`. Декоративные `.mention-badge` и `.channel-mute-icon` имеют `aria-hidden="true"`, чтобы число не зачитывалось дважды |
| `#guild-notif-error` | `alert` | `role="alert"` | Озвучивает ошибку сохранения немедленно при снятии `hidden` |
| `#server-status` | `status` | `role="status"`, `aria-live="polite"` | Озвучивает успешное применение настройки (например: `«Сервер заглушён на 1 час»`, `«Уровень уведомлений: Только @упоминания»`) |

### 5.2 Клавиатурная модель и управление фокусом (Инварианты D1, D2, D3 из `G05-REST-SPEC.md`)

1. **Roving `tabindex` в меню и подменю (D2)**:
   - В открытом `#server-menu`, `#guild-mute-submenu`, `#guild-notif-submenu` или `#channel-notif-menu` ровно один активный (enabled) пункт имеет `tabindex="0"`, все остальные — `tabindex="-1"`.
   - Клавиши `ArrowDown` / `ArrowUp` перемещают фокус по циклу только среди enabled-пунктов (`[aria-disabled="true"]` пропускаются или при фокусе гарантируют no-op по D1) и синхронно переносят `tabindex="0"`.
   - Клавиши `Home` / `End` переводят фокус на первый / последний enabled-пункт.
2. **Вход и выход из подменю (`ArrowRight` / `ArrowLeft` / `Escape`)**:
   - На пунктах #3 (`guild-mute`) и #4 (`guild-notifications`) нажатие `ArrowRight`, `Enter` или `Space` раскрывает соответствующее подменю (`aria-expanded="true"`, снимает `hidden`) и переводит фокус на первый пункт внутри подменю.
   - Внутри открытого подменю нажатие `ArrowLeft` или `Escape` (`event.stopPropagation()`) закрывает **только подменю**, возвращает `aria-expanded="false"` и возвращает фокус на родительский пункт #3 или #4 в `#server-menu` (само `#server-menu` остаётся открытым).
   - Нажатие `Escape` в корневом `#server-menu` (при закрытых подменю) закрывает `#server-menu`, возвращает фокус на `button[data-action="guild-menu"]` и вызывает `event.stopPropagation()`, чтобы глобальный обработчик `body` **не переключал** активный экран (`body[data-active-screen]` не меняется — инвариант D3).
   - Нажатие `Tab` в меню закрывает меню и подменю, возвращает фокус на opener без отмены штатного перехода.
3. **Открытие и закрытие поповера канала `#channel-notif-menu` с его триггера**:
   - На кнопке-триггере колокольчика в шапке чата `button[data-action="channel-notif-menu"]` (`#chat-header`, имеет `aria-haspopup="menu"`, `aria-expanded="false"`, `aria-controls="channel-notif-menu"`) нажатие `Enter`, `Space`, `ArrowDown` или `ContextMenu` / `Shift+F10`, а на строке канала `.channel-item` в `#channel-tree` — нажатие `ContextMenu` или `Shift+F10`, открывает `#channel-notif-menu` для соответствующего `channel_id`: запоминает элемент-инициатор (`opener`), снимает `hidden`, выставляет `aria-expanded="true"` на триггере и переводит фокус на первый enabled-пункт внутри `#channel-notif-menu` (`tabindex="0"` по правилу roving `tabindex`).
   - Внутри открытого `#channel-notif-menu` клавиши `ArrowDown` / `ArrowUp` / `Home` / `End` перемещают фокус по roving `tabindex`, `Enter` или `Space` активирует текущий пункт (`channel-unmute`, `channel-mute-preset`, `channel-notif-level`), а нажатие `Escape` вызывает `event.stopPropagation()`, закрывает `#channel-notif-menu` (`hidden`, `aria-expanded="false"` на триггере) и возвращает фокус обратно на сохранённый `opener` (`button[data-action="channel-notif-menu"]` или `.channel-item`).
4. **Модальное окно `#guild-notifications-dialog`, таблица `.notif-override-table` и checkbox/switch-контролы**:
   - При открытии из `#guild-notif-submenu` меню и подменю закрываются, запоминается исходный триггер (`opener = button[data-action="guild-menu"]`), а первичный фокус в модалке ставится на тумблер `#guild-notif-mute-switch`.
   - Внутри модалки действует циклический focus trap по `Tab` / `Shift+Tab`.
   - В радиогруппе уровня гильдии `.notif-level-list` (`role="radiogroup"`) стрелки `ArrowDown`/`ArrowRight` и `ArrowUp`/`ArrowLeft` перемещают фокус и выбирают радио-карточку с переносом `tabindex="0"`.
   - **Фокусирование и навигация по radio-контролам строк `.notif-override-table`**: в каждой строке `.notif-override-row` три кнопки `.notif-cell-radio[role="radio"]` (`data-level="0"`, `"1"`, `"2"`) связаны roving `tabindex` в пределах строки — активная радиокнопка (`aria-checked="true"`, либо первая при отсутствии явной отметки) имеет `tabindex="0"`, а две остальные — `tabindex="-1"`. Клавиша `Tab` переводит фокус в строку на одну активную радиокнопку (`tabindex="0"`), затем на чекбокс `.notif-cell-checkbox[role="checkbox"]` (`tabindex="0"`) и кнопку сброса `.notif-override-remove` (`tabindex="0"`). Когда фокус находится на `.notif-cell-radio[role="radio"]`, клавиши `ArrowLeft` / `ArrowRight` (и `ArrowUp` / `ArrowDown`) циклически перемещают фокус между тремя радиокнопками этой же строки (`0 ↔ 1 ↔ 2`), синхронно переносят `tabindex="0"` (остальным двум ставится `tabindex="-1"`) и запускают выбор уровня (`override-set-level`), а `Space` / `Enter` активируют текущую сфокусированную радиокнопку.
   - **Активация checkbox- и switch-контролов клавишей `Space`**: на чекбоксе mute строки `.notif-cell-checkbox[role="checkbox"]`, пункте `menuitemcheckbox` (`button[data-action="guild-hide-muted"]`) и тумблерах `.toggle-input[role="switch"]` нажатие `Space` (а для кнопок с `role="checkbox"` / `role="menuitemcheckbox"` также `Enter`) вызывает `event.preventDefault()` (предотвращая прокрутку диалога/списка) и инициирует переключение настройки (`override-toggle-mute` / соответствующий ключ `data-setting`) по жизненному циклу §4.1 (`idle → pending → ready | error`, без оптимистичного переключения `aria-checked` до ответа).
   - Нажатие `Escape` (если модалка не в состоянии `data-state="pending"`) или кнопки «Закрыть» / «Готово» закрывает модалку, вызывает `event.stopPropagation()` и возвращает фокус на `opener`.

---

## 6. Трассируемость к `NOTIFICATIONS-RE.md` и оракулам тестирования

| Требование спецификации | Раздел `NOTIFICATIONS-RE.md` / Артефакт | Метка | Проверяемый инвариант для тестов (G2 / G1 / G4) |
|---|---|---|---|
| Контейнер `user_guild_settings{entries,partial,version}` и 13 полей записи | §1, §2 (`gateway-schema.md:147,255-271`, `002-READY.json`) | `[D]` | 58 записей в capture, 1328 override у 52 записей (макс. 185); строгий парсинг `"true"`/`"false"`/`"nil"` |
| Коды `message_notifications` `0/1/2/3` (`ALL_MESSAGES`, `ONLY_MENTIONS`, `NO_MESSAGES`, `NULL`) | §3.1 (`web.4d4b2f75fdea5470.js@313815`, экспорт `orn`) | `[D]` | Распределение в capture: гильдии `{0:19, 1:36, 2:3}`, каналы `{3:1327, 1:1}`; `3` допустим только для канала |
| Коды `notify_highlights` `0/1/2` (`NULL`, `DISABLED`, `ENABLED`) | §3.2 (`@529852`, экспорт `guM`; UI `@5511, @17620`) | `[D]` | Распределение `{0:56, 1:2}`; тумблер UI отправляет `2` при включении и `1` при выключении |
| Биты `flags` гильдии (`2048`, `4096`, `8192`, `16384`) и канала (`512`, `1024`, `2048`, `4096`, `8192`, `16384`) | §3.4, §3.5 (`@8352891`, `@8352746`, помощники `mD`/`PW` `@11923745`) | `[D]` | Гильдии `{0:52, 16384:6}`; каналы `{4096:1252, 6144:1, None:75}`. Изменение битов `UNREADS_*` сохраняет `4096` и `2048` |
| 6 пресетов `mute_config.selected_time_window`: `900`, `3600`, `10800`, `28800`, `86400`, `-1` | §4 (`@8349873` `Xx`, `@6225743` `b8()`, `@11993`) | `[D]` | Порядок и значения 6 опций совпадают с `b8()`; для `-1` пишется `end_time: null`, для `>0` — `now + window` |
| Семантика `isMuted` и истечение `end_time` | §4 (`@2364848`, `@9384776`, `@9392282`) | `[D]` | `muted=true` с `mute_config=null` остаётся `isMuted=true`; `end_time < now` даёт `isMuted=false` (3 канала 2021 г. в capture) |
| Каскад `resolvedMessageNotifications`: канал (`!=3`) → категория (`!=3`) → гильдия | §6 (`@9387678`, `@9388508`) | `[D]` | При `channel.message_notifications=3` берётся уровень `parent_id`, а если и там `3`/нет — уровень гильдии |
| Форма `channel_overrides` (list и map на чтение, map на запись) и сброс override | §5, §10 (`@9385738`, `@682805`, `@1622`) | `[D]` | Удаление override в UI шлёт `{message_notifications: 3, muted: false}` и не стирает `collapsed` |
| Обработка `USER_GUILD_SETTINGS_UPDATE` | §7 (`@4930904`, `@9391654`, `@9384248`) | `[D]`/`[A]` | Одиночная запись мержится в стор; отсутствие `channel_overrides` в частичном апдейте не очищает существующие overrides |
| Маршруты записи REST (`PATCH /users/@me/guilds/settings` bulk и `PATCH /users/@me/guilds/@me/settings`) | §9.1, §9.2 (`mod-652215.pretty.js:94`, `@8333335`, `@8334021`) | `[D]` | Для гильдии тело `{guilds: {[id]: partial}}`, для `@me` — `partial` |
| 5 колонок таблицы `channel_overrides` и предупреждение `memberCount >= 2500` | §10 (`046406f7a5feb3e2.js@18911`, ключи `uShwWv`, `hZrr6k`, `JzbSEY`, `y59NJm`, `pGn/bJ`, `Dh5p5j`) | `[D]` | Порядок колонок: Канал, Все, Упоминания, Ничего, Mute; баннер `Dh5p5j` показывается при `level === 0 && memberCount >= 2500` |
| Локальный `NotificationSettingsStore` (`persistKey="notifications"`) vs серверный `user_guild_settings` | §8 (`@9225345`, модуль `803224`) | `[D]` | Звуки, `desktopType`, `ttsType`, `disableUnreadBadge`, `taskbarFlash` хранятся локально и не отправляются в `user_guild_settings` |
