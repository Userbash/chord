# REACTIONS-EXTENDED-SPEC — Полная дизайн-спецификация поповера выбора реакций и быстрых реакций сообщения (RW-40 / task-114)

- **Статус**: Нормативная дизайн-спецификация (Design Freeze RW-40)
- **Ревизия**: 4 (2026-10-30). База — ревизия 3, переработанная по DeepSeek Review 38C
  (`waves/RW-40/PM/task114-reactions-spec/REVIEW-PASS3.md`, 2 P1 + 5 P2): кнопка «×» баннера
  ошибки получила описанный обработчик и оракул N-12, контраст кнопки тулбара в состоянии
  `pending` + `aria-pressed="true"` поднят с 4.04:1 до 5.74:1, `aria-controls` триггеров
  синхронизирован с жизненным циклом поповера, префиксные id в §3.2 приведены к норме,
  покрытие `data-pending` в §8.2 исправлено, порядок `Tab` сделан условным. Что именно
  изменено — §0.3 (ревизия 2), §0.4 (ревизия 3) и §0.5 (ревизия 4).
- **Карточка**: `design/task-114-reactions-picker-spec`
- **Роль**: G3 Design
- **Целевая область**: Поповер выбора реакций (`#reaction-picker-popover` / `#screen-emoji` / `.emoji-card`), быстрые реакции сообщения в плавающей панели действий (`.msg-toolbar` / `QUICK_REACTIONS`) и контекстном меню (`R-16`), ряд пилюль реакций в ленте сообщений (`.reactions-row` / `.reaction` / `.reaction-add-btn`), связка с Tauri IPC `toggle_reaction`, REST-маршрутами `PUT` и `DELETE /channels/{id}/messages/{id}/reactions/{emoji}/@me` и событиями Gateway `MESSAGE_REACTION_ADD` / `MESSAGE_REACTION_REMOVE`.
- **Единый источник дизайн-токенов**: `rust/gui-spike/ui/tokens.css` (100% биекция `var(--...)`, `0` сырых `hex`/`rgb`/`hsl` цветов в CSS-правилах, `0` `!important`, контраст текста WCAG 2.2 AA $\ge 4.5:1$, контраст интерактивных границ и non-text индикаторов $\ge 3.0:1$; фактические коэффициенты — §7.2, документированные исключения — §7.2.2). `tokens.css` объявляет только `color-scheme: dark`; light-тема в коде отсутствует и в настоящей спецификации не сертифицируется.
- **Связанные документы и код**:
  - Токены и темизация: `rust/gui-spike/ui/tokens.css`, `waves/RW-40/THEME-SPEC.md`
  - Архитектура сообщений и пикера: `waves/RW-40/DESIGN/DESIGN-MSG.md`, `waves/RW-39/DESIGN/SPEC-P3-02.md`, `waves/RW-39/DESIGN/DOM-CONTRACT.md`, `waves/RW-39/DESIGN/RE-EXTRA.md`
  - RE-дампы эталона Discord (`1.0.9036`): `reference-oracle/ui/01-msg-menu-items.json`, `reference-oracle/ui8/21b-emoji-full.json`, `reference-oracle/ui8/21d-emoji-categories.json`
  - Текущая реализация в спайке: `rust/gui-spike/src/reactions.rs`, `rust/gui-spike/src/main.rs`, `rust/gui-spike/src/data.rs`, `rust/gui-spike/src/net.rs`, `rust/gui-spike/ui/features/reactions.js`, `rust/gui-spike/ui/features/emoji.js`, `rust/gui-spike/ui/features/message-toolbar.js`, `rust/gui-spike/ui/features/rows.js`, `rust/gui-spike/ui/features/chat.js`, `rust/gui-spike/ui/features/dispatch-model.js`, `rust/gui-spike/ui/components/emoji.css`, `rust/gui-spike/ui/components/chat.css`
  - Контрактные тесты: `rust/gui-spike/tests/reactions_contract.rs`, `tests/unit/chat-reactions.test.mjs`, `tests/unit/dispatch_ui.test.mjs`, `tests/unit/gui-spike-dom.test.mjs`

---

## 0. Легенда доказательности и архитектурные инварианты

### 0.1. Маркеры источников (`[D]`, `[S]`, `[T]`, `[M]`, `[L]`, `[F]`, `[A]`)

Каждое нормативное требование в настоящей спецификации снабжено маркером происхождения:

| Маркер | Значение | Первоисточник |
|---|---|---|
| **`[D]`** | Прямой DOM-дамп эталона Discord (`1.0.9036`) | `reference-oracle/ui/01-msg-menu-items.json`, `reference-oracle/ui8/21b-emoji-full.json`, `reference-oracle/ui8/21d-emoji-categories.json` |
| **`[S]`** | CSS-правила из скомпилированных таблиц стилей эталона | `DESIGN-MSG.md`, `SPEC-P3-02.md`, `RE-EXTRA.md` |
| **`[T]`** | Семантические дизайн-токены приложения | `rust/gui-spike/ui/tokens.css`, `waves/RW-40/THEME-SPEC.md` |
| **`[M]`** | Числовой замер геометрии (`getBoundingClientRect` / `computedStyle`) | `21b-emoji-full.json` (`498×440`, `44×44`, `9` колонок), `01-msg-menu-items.json` (`188px` min-width, `32px` item) |
| **`[L]`** | Локализация `ru` эталона и русский UI-контракт `discord-lab` | `21d-emoji-categories.json`, `01-msg-menu-items.json`, `emoji.js`, `reactions.rs` |
| **`[F]`** | Зафиксированный контракт в коде и тестах `rust/gui-spike` | `reactions.rs`, `reactions.js`, `emoji.js`, `message-toolbar.js`, `rows.js`, `chat.js`, `dispatch-model.js` |
| **`[A]`** | Архитектурное решение `discord-lab` (`D-01`..`D-08`) | `DESIGN-MSG.md §0.2`, `DOM-CONTRACT.md §9` |

### 0.2. Жёсткие инварианты реализации

1. **100% биекция с `rust/gui-spike/ui/tokens.css`** `[T]`:
   - Все цвета, фоны, рамки, скругления, отступы, тени, шрифты, размеры иконок, высоты кнопок, `z-index` и длительности переходов в CSS-правилах задаются **исключительно** через переменные `var(--...)`, объявленные в `rust/gui-spike/ui/tokens.css`.
   - Запрещены сырые литералы цветов (`#...`, `rgb(...)`, `rgba(...)`, `hsl(...)`, `hsla(...)`) и модификатор `!important` (`0` вхождений).
   - Запрещено выдумывать несуществующие переменные (такие как `--warning-text`, `--surface-overlay`, `--mention-self-bg`). `--warning-text` введён в `waves/RW-40/THEME-SPEC.md §2.5` как design-target светлой темы, но в `tokens.css` его нет: до появления токена в `tokens.css` использовать его в CSS запрещено. Согласование с THEME-SPEC — отдельная карточка темы, не эта.
   - **Non-text контраст (WCAG 2.2 SC 1.4.11)**: граница интерактивного контрола, которая является единственным признаком контрола или его состояния, обязана иметь $\ge 3.0:1$ к смежному фону. В настоящей спецификации этому подчиняются пилюля `.reaction`, кнопка `.reaction-add-btn`, выбранная ячейка `.emoji-cell[data-reacted="true"]`, фокус-кольцо и индикаторы ошибки. Декоративные границы контейнеров (`.emoji-card`, панель `.msg-toolbar`, разделители `.emoji-head`/`.emoji-preview`/`.emoji-section-header`, скелетон загрузки) под SC 1.4.11 не подпадают: они не идентифицируют контрол, а панель отделена тенью `--shadow-popover`; их фактические коэффициенты всё равно приведены в §7.2.2 без сертификации. Неактивные контролы (`:disabled`, `[aria-disabled="true"]`) исключены из требования самим SC 1.4.11 (Inactive user interface components).
   - Замена `--border-subtle`/`--border`/`--border-strong` на границе контрола **не** решает задачу: 12% / 20% / 44% альфы `#97979f` поверх `--bg-chat` дают 1.20 / 1.38 / 2.20 : 1. Поэтому граница контрола строится на `--accent` (3.77:1 к `--bg-chat`, 3.35:1 к `--surface-2`, 3.18:1 к `--surface-3`).
2. **Инвариант прямых детей строки сообщения `.row`** `[F][A]`:
   - В `rust/gui-spike/ui/features/chat.js` (`baseRow`) элемент `.row` содержит ровно 2 прямых дочерних узла: `.author` и `.text` (`children.length === 2`, проверяется в `tests/unit/gui-spike-dom.test.mjs`).
   - Ряд пилюль реакций `.reactions-row` монтируется **внутрь** контейнера `.text` (который при наличии вложений, реакций или бейджа `(изменено)` получает атрибут `data-stack="true"`), а не третьим прямым ребёнком `.row`.
3. **Защита от XSS и инъекций разметки** `[F][A]`:
   - Все пользовательские и серверные строки (глиф или имя эмодзи `emoji`, счётчик `count`, поисковый запрос, текст ошибки `ReactionError`) вставляются в DOM только через `document.createElement(...)`, `element.textContent = ...` и `element.setAttribute(...)`. Использование `innerHTML`, `outerHTML`, `insertAdjacentHTML` запрещено.
4. **Автономность (Offline-First / 0 внешних ассетов)** `[A]`:
   - Никаких обращений к внешним CDN (`cdn.discordapp.com`, Twemoji CDN, внешним веб-шрифтам) пикер и пилюли реакций не совершают. Стандартные эмодзи рендерятся системным глифом через стек `var(--font-sans)` (`"Apple Color Emoji"`, `"Segoe UI Emoji"`, `"Noto Color Emoji"`), а кастомные серверные эмодзи вида `name:id` без локального растра — текстовым бейджем `:name:` моноширинным стеком `var(--font-mono)`.
5. **Единый источник состояния реакций и дедупликация запросов** `[F]`:
   - Локальное состояние живёт в модуле `rust/gui-spike/ui/features/reactions.js` (экспортируемые функции, без класса-контроллера): `overrides: Map<ReactionKey, Map<string, {count: number, me: boolean}>>` (`reactions.js:19-23`) и `pending: Set<string>` (`:24`), где `ReactionKey` — либо числовой индекс bench-строки, либо `channelReactionKey(channelId, messageId) === "channel:" + channelId + ":" + messageId` (`:43-45`).
   - Ключ in-flight операции: `` `${String(key)}:${emoji}` `` (`reactions.js:166`), то есть `channel:<channelId>:<messageId>:<emoji>` для реального канала и `<index>:<emoji>` для bench-строки. Формула `` `${channelId}:${messageId}:${emoji}` `` (без префикса `channel:`) неверна и в коде не используется.
   - Публичный API ровно два (сигнатуры заморожены тестами `tests/unit/chat-reactions.test.mjs`, `tests/unit/gui-spike-features.test.mjs`):
     - `toggleReaction(key, emoji, base?, options?) → boolean` (`reactions.js:127-148`) — только локальное переключение `overrides`, возвращает новое `me`;
     - `requestReaction(key, emoji, base, options?) → Promise<void>` (`reactions.js:165-209`), где `options = { element?, channelId?, messageId?, onSettled? }`.
     Никаких `message`-объектов, `onOptimistic`, `onError`, `previous` и возврата `{active, list}` в коде нет.
   - Пока ключ находится в `pending`, повторный `requestReaction` с той же парой строка/эмодзи выходит без сети (`:167`). Это единственная защита от дублей; поповер пикера обязан использовать тот же `requestReaction`, а не собственную отправку.
6. **Замороженный контракт важнее текста спецификации** `[F]`: если утверждение расходится с тестом из `tests/unit/` или с `DOM-CONTRACT.md`, побеждает тест. Изменение такого теста — отдельное решение PM с правкой теста и записи в `waves/db.jsonl`, а не «попутная» правка спецификации. Настоящая ревизия приводит текст к тестам; единственное исключение — `.emoji-row`/D-05, см. §2.4.
7. **Новые узлы и состояния обязаны иметь red-first оракул** `[A]`: `#reaction-picker-popover`, баннер, loading/empty, `data-reacted`, `data-state`, `data-pending`, `aria-busy` на пилюле и обработчики `data-action="reaction-picker-close"` / `data-action="dismiss-picker-error"` в коде `ui/` отсутствуют (проверено `grep -rl`; у `data-pending` покрыт только сам атрибут — см. §8.2), поэтому маркер `[F]` к ним неприменим. Для каждого такого узла в §8 указан тест, который пишется **до** кода и падает на текущем HEAD.

### 0.3. Что изменено в ревизии 2
| Блок | Было | Стало |
|---|---|---|
| §7.2 | Палитра Discord из дампа, light-колонка | Пересчёт по `tokens.css` (dark only), non-text пары, документированные исключения |
| §0.2 п.1 | «non-text ≥ 3.0» без реализации | Правило границы контрола через `--accent`, исключения по SC 1.4.11 |
| §4.1 | `--space-2..7` сдвинуты на шаг | Фактические `2/4/6/8/12/16/20px`, бюджет 496 из 498 |
| §7.1 | `.emoji-card` без рабочей раскладки | `grid-template-areas`; строка баннера есть всегда, но при `[hidden]` её высота 0; `flex: 1` только у видимого состояния |
| §3.2.4/§3.2.5 | `kind: "rate_limited"`, поле `message` | serde `ReactionError`: camelCase, без `message`, + `client`/`invalidResponse`, 429-путь с retry-циклом |
| §6.1/§6.3 | `requestReaction(message, emoji, {onOptimistic…})`, дельты ±1 | Фактические сигнатуры, `projectReaction` с абсолютным `count` и `Set<userId>` |
| §2.2 | 5 кнопок внутри `.row` | Одна панель на чат, 15 пунктов, `data-open`/`data-for-index` |
| §2.3/§2.4 | Дубли id, `aria-pressed` на `option`, Режим B | Префикс id на экземпляр, только `aria-selected`, listbox-only |
| §8 | Ссылки на несуществующие проверки | Таблица red-first оракулов с реальными `файл:строка` |

### 0.4. Что изменено в ревизии 3

| Блок | Было (ревизия 2) | Стало (ревизия 3) |
|---|---|---|
| §1.2/§2.3/§5.3 | Кнопка «×» поповера использовала `data-action="emoji-close"`; показ через `hidden`, механизм удаления не задан | `data-action="reaction-picker-close"`; lazy mount при открытии и `remove()` при закрытии; перечень глобальных запросов, которые поповер не задевает |
| §2.1/§2.2 | Примеры триггеров без `aria-controls` и `aria-haspopup`/`aria-expanded`; `aria-label` кастомной пилюли противоречил правилу; у `.reaction-add-btn` несуществующий `data-action` | Оба триггера несут одинаковый набор ARIA-атрибутов; `aria-label` кастомной реакции — сырой `name:id`; `data-action` убран в пользу прямого обработчика |
| §2.1/§2.3 | «`reactionsGroupOf` всегда создаёт группу», «липкий заголовок» | Уточнено: `null` без `.text`; слово «липкий» убрано (правила `sticky` нет) |
| §2.3/§5.1 | `data-category` у вкладок без нормы и оракула; `aria-controls` вкладки не объяснён | `data-category` удалён; компромисс `tab → listbox` записан как явное исключение APG |
| §3.1/§7.1/§7.2.1 | Обещанный hover-цвет счётчика не имел CSS-правила; 14.93 : 1 | Добавлено `.reaction:hover … .reaction-count`; 14.18 : 1 |
| §6.3 | «Защита от эха» арифметикой live-счётчика | Зафиксирован известный дефект двойного счёта, карточка N-11 (§8.4) |
| §7.2.2 | `2.15 : 1` у рамки карточки, `2.46 : 1` у `--state-selected-strong` | `2.42 : 1` (к своей подложке `2.15 : 1`), `≈1.53–1.57 : 1` |
| §8.1/§8.2 | E-04 ссылался на оракул `delete`; закрытие поповера и `aria-busy` не проверялись | Ссылка исправлена; N-03 дополнен destroy-оракулом, добавлен N-10 (`aria-busy`) |

### 0.5. Что изменено в ревизии 4

| Блок | Было (ревизия 3) | Стало (ревизия 4) |
|---|---|---|
| §2.3/§5.3/§8.2 | Кнопка «×» баннера несла `data-action="dismiss-picker-error"` без обработчика и без оракула — мёртвый узел (P1-1) | §5.3 п.1 задаёт прямой обработчик в `openReactionPicker()` рядом с close, §5.3 п.3 — семантику dismiss (баннер скрыт, `data-state` восстановлен, поповер открыт, кулдаун не обходится); оракул N-12 |
| §3.1/§7.1/§7.2.1 | `.msg-toolbar-btn[data-pending]` перебивал цвет выбранной кнопки: `--text-muted` на `#333651` = 4.04:1 (P1-2) | Добавлено правило пересечения `[data-pending="true"][aria-pressed="true"]` → `--text-accent` (**5.74:1**), строка в §7.2.1 и состояние 10 в §3.1 |
| §2.1/§2.2/§5.1 | `aria-controls="reaction-picker-popover"` висел на триггерах в закрытом состоянии, когда узла нет (P2-1) | `aria-controls` выставляется и снимается вместе с `aria-expanded`; записано третьим исключением §5.1 |
| §3.1/§7.1 | §3.1 п.7 обещал `opacity`/`cursor` у `.reaction-add-btn[aria-disabled="true"]`, в CSS правила не было (P2-2) | Селектор добавлен в существующее правило §7.1 |
| §3.2.2/§3.2.3 | Непрефиксные `#emoji-grid`/`#emoji-tabs` в разделе про поповер (P2-3) | `#reaction-picker-grid` / `#reaction-picker-tabs` |
| §8.2 | Ложное «`data-pending` не покрыт тестами» (P2-4) | Указано фактическое покрытие (`chat-reactions.test.mjs:372,376`; `gui-spike-features.test.mjs:1245,1463,1466`); не покрыт только `aria-busy` на пилюле (N-10) |
| §5.2 | «`Tab` проходит 4 стопа» без условий (P2-5) | Формулировка условная: 4 стопа в `ready` без баннера, 5 при видимом баннере, состав в `loading` — отдельный |


---

## 1. Архитектура и точки входа пользовательского сценария

Пользователь может поставить, переключить (toggle) или снять реакцию на сообщении через четыре синхронизированные точки входа:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                     ТОЧКИ ВХОДА РЕАКЦИЙ НА СООБЩЕНИИ                         │
├──────────────────┬───────────────────┬──────────────────┬────────────────────┤
│ 1. Пилюли под    │ 2. Быстрые        │ 3. Контекстное   │ 4. Поповер выбора  │
│ сообщением       │ реакции тулбара   │ меню сообщения   │ реакций (Picker)   │
│ .reactions-row   │ .msg-toolbar      │ #msg-context-menu│ #reaction-picker-  │
│  ├─ button       │  ├─ 4× button     │  ├─ 4× быстрых   │    popover /       │
│  │  .reaction    │  │  [data-command │  │  реакции [D]  │ #screen-emoji      │
│  └─ button       │  │   ="react"]    │  └─ пункт #5     │  ├─ Поиск эмодзи   │
│     .reaction-   │  └─ button        │     "Добавить    │  ├─ 10 категорий   │
│     add-btn (+)  │     [data-command │      реакцию"    │  ├─ Сетка 9 колонок│
│                  │     ="add-        │     [data-command│  ├─ Баннер 429/403 │
│                  │      reaction"]   │     ="add-       │  └─ Превью внизу   │
│                  │                   │      reaction"]  │                    │
└────────┬─────────┴─────────┬─────────┴────────┬─────────┴─────────┬──────────┘
         │                   │                  │                   │
         └───────────────────┴─────────┬────────┴───────────────────┘
                                       ▼
             requestReaction(key, emoji, base, { channelId, messageId, element, onSettled })
                                       │
         ┌─────────────────────────────┴─────────────────────────────┐
         ▼                                                           ▼
  [Оптимистичный патч UI]                               [Tauri IPC invoke]
  toggleReaction(key, emoji, base)                      invoke("toggle_reaction", {
  overrides.get(key).set(emoji, next)                     channelId, messageId,
  pending.add(`${String(key)}:${emoji}`)                   emoji, active
  onSettled() → перерендер .reactions-row                 })
                                                                     │
                                                                     ▼
                                                        Rust: DiscordReactionApi::toggle
                                                        active=true  → PUT    .../@me (body "{}")
                                                        active=false → DELETE .../@me
                                                        (429/5xx: до 3 попыток внутри ApiClient)
                                                                     │
                                       ┌─────────────────────────────┴──────────────────────┐
                                       ▼                                                    ▼
                              [200 OK / 204 No Content]                           [Ошибка ReactionError]
                              pending.delete(...)                                 Откат overrides к before
                              onSettled()                                         (reactions.js:191-195),
                              Синхронизация с эхо-событиями                       reportReactionError(UIError)
                              Gateway MESSAGE_REACTION_ADD /                       по `kind` (camelCase)
                              MESSAGE_REACTION_REMOVE
```

### 1.1. Контракт быстрых реакций (`QUICK_REACTIONS`): согласование `[F]` и `[D]`

В репозитории и эталонных дампах зафиксированы два представления быстрых реакций, которые спецификация объединяет в единый детерминированный контракт:

1. **Базовый набор быстрых реакций в `gui-spike` (`[F]` `rust/gui-spike/ui/features/rows.js:49`)**:
   ```js
   export const QUICK_REACTIONS = Object.freeze(["👍", "❤️", "😂", "😮"]);
   ```
   Эти 4 кнопки рендерятся первыми в плавающей панели действий `.msg-toolbar` (`rust/gui-spike/ui/features/message-toolbar.js`) при наведении или фокусе на строку сообщения `.row`, а также используются как дефолтный фолбэк быстрых реакций.
2. **Эталонный дамп контекстного меню сообщения (`[D]` `reference-oracle/ui/01-msg-menu-items.json`, инвариант `R-16`)**:
   В дампе Discord `01-msg-menu-items.json` (`15` пунктов `role="menuitem"`) первые 4 позиции занимают наиболее часто используемые эмодзи пользователя (`frecency`), за которыми следует 5-й пункт `Добавить реакцию`:
   - `#0`: `id="message-actions-quickreact-0"`, текст `:thumbsup: :+1: :thumbup: :thumbs_up:` (глиф `👍`, шорткод `:thumbsup:`)
   - `#1`: `id="message-actions-quickreact-1"`, текст `:fire: :flame:` (глиф `🔥`, шорткод `:fire:`)
   - `#2`: `id="message-actions-quickreact-2"`, текст `:regional_indicator_f:` (глиф `🇫`, шорткод `:regional_indicator_f:`)
   - `#3`: `id="message-actions-quickreact-3"`, текст `:Linux:` (кастомный/именованный эмодзи `:Linux:`)
   - `#4`: `id="message-actions-reactions"`, текст `Добавить реакцию` (`hasSubmenu: true`, открывает пикер реакций)
3. **Правило разрешения набора быстрых реакций**:
   - По умолчанию `.msg-toolbar` использует массив `QUICK_REACTIONS = ["👍", "❤️", "😂", "😮"]` (`[F]`, проверяется контрактным тестом `tests/unit/chat-reactions.test.mjs`).
   - Функция `createMessageToolbar(options)` (`message-toolbar.js:59`) принимает `{ list, container, renderRow, indexLabel, authorOf, getComposer, textOf?, messageIdOf?, reactionTargetOf?, onEdit?, onDelete?, onError? }`. Опции `quickReactions`, `onReact`, `onOpenPicker` **не существуют**: набор быстрых реакций жёстко берётся из `QUICK_REACTIONS` (`message-toolbar.js:83`), а клик вызывает `requestReaction(...)` внутри панели (`:204-226`), а не колбэк наружу. Подмена top-4 из `frecency` возможна только правкой `message-toolbar.js` (отдельная карточка), в текущем контракте — фиксированные 4 глифа.
   - В каталоге `EMOJI_CATEGORIES` (`rust/gui-spike/ui/features/emoji.js:22-275`) категории называются иначе, чем в дампе: `"Часто используемые"`, `"Смайлы"`, `"Люди и жесты"`, `"Еда и напитки"`, `"Природа"`, `"Транспорт"`, `"Предметы"`, `"Символы"`, `"Флаги"`, `"Прочее"`. Первая категория содержит `👍`, `❤️`, `😂`, `😮` и `🔥`, но не `🇫`. Поля `keywords` и шорткодов (`thumbsup`, `fire`, `flame`) в модели данных нет (`{ glyph, name }`, `emoji.js:17`): поиск идёт по нормализованному русскому `name` и по глифу (`searchAllEmojis`, `:300-317`), поэтому запрос `thumbsup` не найдёт `👍`. Поиск по шорткодам в текущем контракте не заявляется; чтобы его вернуть, нужен источник шорткодов — отдельная карточка данных.

### 1.2. Два режима размещения пикера реакций (`Popover` vs `Screen`)

Компонент `.emoji-card` (`createEmojiConsole` в `rust/gui-spike/ui/features/emoji.js:339-500`) поддерживает два режима монтирования с идентичной внутренней DOM-сеткой и клавиатурной логикой:
1. **Контекстный поповер реакции на сообщение (`#reaction-picker-popover`)** `[A]` (в коде отсутствует):
   - Открывается кликом по кнопке `[data-command="add-reaction"]` в `.msg-toolbar`, кликом по кнопке `.reaction-add-btn` (`+`) в `.reactions-row`, выбором пункта «Добавить реакцию» в контекстном меню сообщения или нажатием клавиши `+` (`Shift+=`) на активной строке сообщения `.row` (`KEY-14`).
   - **Сейчас триггер не открывает пикер**: `message-toolbar.js:204-207` на `command === "react" || command === "add-reaction"` вызывает `requestReaction(..., emoji || "👍")`, то есть пункт «Добавить реакцию» ставит `👍`. Переключение на открытие поповера — отдельная карточка G2 с правкой `message-toolbar.js` и red-first тестом (§8, N-01); до неё поведение остаётся прежним.
   - Позиционируется с `position: fixed; z-index: var(--z-popover)` (`100`) рядом с кнопкой-триггером сообщения (`anchorElement`).
   - Хранит ссылку на целевое сообщение (`data-target-channel-id`, `data-target-message-id`) и подсвечивает эмодзи, которые текущий пользователь уже поставил на это сообщение (`data-reacted="true"`).
   - Выбор эмодзи вызывает `requestReaction(channelReactionKey(channelId, messageId), emoji.glyph, base, { channelId, messageId, onSettled })` и по умолчанию закрывает поповер с возвратом фокуса на триггер сообщения (при удержании `Shift` во время клика или `Shift+Enter` — переключает реакцию без закрытия поповера для пакетной простановки нескольких реакций).
2. **Стационарный экран обзора (`#screen-emoji`)** `[F]`:
   - Существующий в `rust/gui-spike/ui/index.html:137-155` экран `data-screen-tab="emoji"`, использующий `.emoji-card`, `#emoji-search`, `#emoji-tabs`, `#emoji-grid` и `.emoji-preview`. Инициализируется `initEmojiConsole({ onSelect })` (`app.js:1490-1493`), выбор вставляет глиф в композер.
   - Два экземпляра **не сосуществуют одновременно**: у стационарного экрана id без префикса, у поповера — с префиксом `reaction-picker-` (§2.3). Это требуется потому, что `initEmojiConsole` ищет узлы через `byId("emoji-tabs")`/`byId("emoji-grid")`/`byId("emoji-search")` (`emoji.js:507-510`) и привяжется к первому совпадению.
   - **Механизм — lazy mount** `[A]`: пока пикер закрыт, узла `#reaction-picker-popover` в документе **нет**. `openReactionPicker()` создаёт и монтирует его, `closeReactionPicker()` вызывает `remove()` и снимает слушатели (§5.3). Скрытие корня через `hidden`/`display: none` запрещено: скрытый узел продолжает давать второй набор `.tab` и второй `.emoji-card`, которые считают глобальные запросы (`actions.js:1109`, `app.js:1625`, `gui-spike-features.test.mjs:377`) и замороженный обработчик `[data-action="emoji-close"]` (`actions.js:905`).

---

## 2. Полная DOM-структура компонентов

### 2.1. Ряд пилюль реакций в сообщении (`.reactions-row` и `.reaction`)

Ряд реакций монтируется **последним ребёнком** `.text[data-stack="true"]` (после `.msg-content`, вложений, метки «изменено» и day-divider) `[F]` (`chat.js:2525-2548`). Решение о создании принимает вызывающий код: `reactionsGroupOf(row)` (`chat.js:232-244`) создаёт группу только при наличии `.text` в строке и возвращает её, а при отсутствии `.text` возвращает `null`; вызывающий обязан проверить результат (`chat.js:2529-2530` вызывает функцию лишь при `reactions.length > 0`, а `chat.js:3716-3717` — после проверки `null`). Узел `.reactions-row` **не создаётся**, если реакций нет и нет активной ошибки постановки реакции; при ошибке он создаётся отдельно (§3.2.4).

```html
<div class="row"
      role="listitem"
     tabindex="-1"
     data-message-id="1200000000000000001"
     data-channel-id="1100000000000000001"
     data-group-start="true">
  <span class="author" data-role="mod">alice</span>
  <div class="text" data-stack="true">
    <span class="msg-content">Привет, проверяем реакции!</span>

    <!-- Контейнер реакций: создаётся только при наличии ≥1 реакции или баннера ошибки -->
    <div class="reactions-row"
         role="group"
         aria-label="Реакции">

      <!-- Выбранная текущим пользователем реакция (me = true) -->
      <button type="button"
              class="reaction"
              data-emoji="👍"
              data-me="true"
              aria-pressed="true"
              aria-label="Реакция 👍, 3">
        <span class="reaction-emoji" aria-hidden="true">👍</span>
        <span class="reaction-count">3</span>
      </button>

      <!-- Чужая реакция в состоянии ожидания ответа сервера (pending) -->
      <button type="button"
              class="reaction"
              data-emoji="🔥"
              data-me="false"
              data-pending="true"
              aria-pressed="false"
              aria-busy="true"
              aria-label="Реакция 🔥, 1">
        <span class="reaction-emoji" aria-hidden="true">🔥</span>
        <span class="reaction-count">1</span>
      </button>

      <!-- Кастомная серверная реакция (name:id без внешних CDN) -->
      <button type="button"
              class="reaction"
              data-emoji="party:987654321098765432"
              data-custom-emoji="true"
              data-me="false"
              aria-pressed="false"
              aria-label="Реакция party:987654321098765432, 2">
        <span class="reaction-emoji reaction-emoji-custom" aria-hidden="true">:party:</span>
        <span class="reaction-count">2</span>
      </button>

      <!-- Опциональная кнопка быстрого вызова пикера в конце ряда пилюль -->
      <!-- aria-controls появляется вместе с aria-expanded="true" при открытии:
           до монтирования поповера узла #reaction-picker-popover не существует,
           и IDREF висел бы в пустоту (§5.1, исключение 3). -->
      <button type="button"
              class="reaction-add-btn"
              data-command="add-reaction"
              aria-haspopup="dialog"
              aria-expanded="false"
              aria-label="Добавить реакцию"
              title="Добавить реакцию">
        <span class="reaction-add-glyph" aria-hidden="true">+</span>
      </button>
    </div>

    <!-- Inline-статус ошибки 429 / 403 / сети на уровне конкретного сообщения (скрыт в норме).
         role="alert" озвучивает СТАТИЧНЫЙ текст один раз; счётчик секунд — отдельный узел
         с aria-live="off", чтобы AT не прерывался каждую секунду (см. §3.2.4). -->
    <div class="reaction-error-status"
         role="alert"
         aria-live="assertive"
         data-error-kind="rateLimited"
         hidden>
      <span class="reaction-error-text">Не удалось поставить реакцию: превышен лимит запросов.</span>
      <span class="reaction-error-timer" aria-live="off">Повтор через 2 с</span>
    </div>
  </div>
</div>
```

#### Нормативные правила атрибутов `.reactions-row` и `.reaction` `[F]`:
- Контейнер `.reactions-row` обязан иметь `role="group"` и `aria-label="Реакции"` (`rust/gui-spike/ui/features/chat.js:237-241`).
- Каждая пилюля `button.reaction` обязана иметь:
  - `type="button"`, `class="reaction"`;
  - `data-emoji="<raw_emoji>"` (для стандартного эмодзи — глиф, например `"👍"`; для кастомного — канонический идентификатор `"name:id"` или `"id"`, совпадающий с тем, что передаётся в `toggle_reaction`);
  - `data-me="true"` при `reaction.me === true` и `data-me="false"` при `reaction.me === false`;
  - `aria-pressed="true"` при `reaction.me === true` и `aria-pressed="false"` при `reaction.me === false`;
  - `aria-label="Реакция ${emojiLabel}, ${count}"` (например, `"Реакция 🔥, 3"` — проверяется в `tests/unit/chat-reactions.test.mjs:236-239`). В коде `emojiLabel` — это сырое `reaction.emoji` (`chat.js:214`), поэтому у кастомной реакции получается `"Реакция party:987654321098765432, 2"`, а не `:party:`;
  - при нахождении ключа `${String(key)}:${emoji}` в множестве `pending`: `data-pending="true"` (ставит `reactions.js:177`) и `aria-busy="true"` (**новое** требование `[A]`: код сегодня выставляет только `data-pending`, `aria-busy` на пилюле нет — red-first тест N-10, §8);
  - ровно два дочерних `<span>`:
    1. `<span class="reaction-emoji" aria-hidden="true">${displayEmoji}</span>` (скрыт от скринридера через `aria-hidden="true"`, так как имя эмодзи и количество уже озвучиваются через `aria-label` родительской кнопки — предотвращает двойное чтение);
    2. `<span class="reaction-count">${String(reaction.count)}</span>` (целое число $\ge 1$; когда счётчик падает до `0`, пилюля полностью удаляется из `.reactions-row`).
- Кнопка `.reaction-add-btn` открывает пикер через прямой обработчик, который навешивается при создании строки (`chat.js`), а не через `[data-action]`: `actions.js:925-928` биндит `[data-action]` только для статической разметки на старте, динамически созданная пилюля в этот `NodeList` не попадает. Поэтому у кнопки нет `data-action`; `data-command="add-reaction"` — только признак для тестов и делегированного обработчика ряда, а не глобальный action.
- `aria-haspopup="dialog"` и `aria-expanded="true|false"` присутствуют на кнопке постоянно. `aria-controls="reaction-picker-popover"` выставляется **только** одновременно с `aria-expanded="true"` и снимается при закрытии: до открытия узла `#reaction-picker-popover` в документе нет (§1.2, lazy mount), и постоянно висящий IDREF не разрешался бы (§5.1, исключение 3).

---

### 2.2. Плавающая панель быстрых реакций и действий (`.msg-toolbar`)

**Одна панель на весь чат** `[F]` (`message-toolbar.js:56-108`, `chat.css:351-368`). Панель создаётся один раз и монтируется в контейнер `.chat-main` (не в `#chat-list` и не в `.row`), поэтому виртуализация не получает 15 узлов на строку; тесты фиксируют `all(app, ".msg-toolbar").length === 1` (`tests/unit/gui-spike-features.test.mjs:1158-1159`) и `#chat-list .msg-toolbar === 0` (`tests/unit/gui-spike-dom.test.mjs:804`). При наведении (`mouseover`) или фокусе (`focusin`) на строку панель «переезжает»: JS выставляет inline `top` по `style.top` строки минус `scrollTop` плюс `4px` и фиксированный `right: 16px` (`message-toolbar.js:68-70,158-171`). Скрытие — `display: none` до `data-open="true"` (`chat.css:356,366-368`), а не `visibility`/`opacity`.

Состав — **15 кнопок** (R-16): 4 быстрые реакции (`QUICK_REACTIONS`, `rows.js:49`) + 11 пунктов `MESSAGE_ACTIONS` (`rows.js:158-171`). Разделителя-элемента в DOM нет; визуальную группировку даёт `gap`.

```html
<!-- Один экземпляр на чат; живёт в .chat-main рядом с #chat-list -->
<div class="msg-toolbar"
     role="toolbar"
     aria-label="Действия с сообщением 12"
     data-open="false"
     data-for-index="12"
     style="position: absolute; top: 4px; right: 16px">

  <!-- 1–4. Быстрые реакции: глиф — textContent, подпись — aria-label -->
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="react" data-emoji="👍"
          aria-pressed="false" aria-label="Реакция 👍">👍</button>
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="react" data-emoji="❤️"
          aria-pressed="false" aria-label="Реакция ❤️">❤️</button>
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="react" data-emoji="😂"
          aria-pressed="false" aria-label="Реакция 😂">😂</button>
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="react" data-emoji="😮"
          aria-pressed="false" aria-label="Реакция 😮">😮</button>

  <!-- 5–15. MESSAGE_ACTIONS в порядке rows.js:158-171; текст подписи видим.
       aria-controls у add-reaction отсутствует, пока поповер не смонтирован;
       он выставляется вместе с aria-expanded="true" (§2.1, §5.1 исключение 3). -->
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="add-reaction"
          aria-haspopup="dialog"
          aria-expanded="false"
          aria-label="Добавить реакцию">Добавить реакцию</button>
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="reply" aria-label="Ответить">Ответить</button>
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="edit" aria-label="Изменить сообщение">Изменить сообщение</button>
  <button type="button" class="icon-btn msg-toolbar-btn"
          data-command="delete" aria-label="Удалить сообщение">Удалить сообщение</button>
  <!-- Остальные 7 пунктов без command: aria-disabled="true" + title=UNAVAILABLE_HINT
       («Закрепить сообщение», «Переслать», «Создать ветку», «Копировать текст»,
        «Копировать ссылку», «Пожаловаться», «Ещё») -->
</div>
```

#### Нормативные правила `.msg-toolbar` `[F]`:
- `class="msg-toolbar"`, `role="toolbar"`, `data-open="false"|"true"`, `data-for-index="<index>"` при открытии.
- `aria-label` панели обновляется при каждом открытии: `"Действия с сообщением " + indexLabel(index)` (`message-toolbar.js:168`), а не фиксированное «Быстрые действия с сообщением».
- Ровно 15 кнопок (`bar.children.length === 15`, `message-toolbar.js:332-334`), из них 4 с `data-emoji` (`data-command="react"`, `aria-label="Реакция ${emoji}"`). Ни `title`, ни `data-me` у кнопок нет — состояние передаётся только `aria-pressed` (`message-toolbar.js:143-152`).
- Триггер `[data-command="add-reaction"]` несёт `aria-haspopup="dialog"`, `aria-expanded="true|false"` и `aria-controls="reaction-picker-popover"`, причём `aria-controls` присутствует только при `aria-expanded="true"` — ровно тот же набор и то же правило жизненного цикла, что у `.reaction-add-btn` (§2.1, §5.1 исключение 3).
- Пункт с `data-command === null` в `MESSAGE_ACTIONS` рендерится как `aria-disabled="true"` + `title="Недоступно в спайке (Phase 3)"` (`message-toolbar.js:100-104`, `rows.js:174`); клик по нему — no-op (`:314-315`).
- Обработчик `[data-command="add-reaction"]` **сейчас ставит `👍`**, а не открывает пикер (`message-toolbar.js:204-207`). Требование «открыть поповер» — новая карточка (N-01 в §8); до её приёмки CSS-блок §7.1 не должен делать панель всегда видимой.
- При закрытии поповера по `Escape` фокус возвращается на кнопку, открывшую поповер, либо на строку `.row[data-message-id=...]`, если триггер размонтирован (§5.3).

---

### 2.3. Поповер выбора реакций (`#reaction-picker-popover` / `.emoji-card`)

Поповер использует единую карточку `.emoji-card` (`498×440px` по замеру `[M]` `21b-emoji-full.json` и токенам `--emoji-picker-w`, `--emoji-picker-h` из `tokens.css`). **Все id поповера имеют префикс `reaction-picker-`**: `#screen-emoji` уже владеет `#emoji-search`, `#emoji-tabs`, `#emoji-grid` (`index.html:137-155`), и второй экземпляр с теми же id сломал бы `byId()` в `initEmojiConsole` (`emoji.js:507-510`). Два экземпляра одновременно не монтируются: корень поповера создаётся при открытии и удаляется при закрытии (§1.2, §5.3), поэтому в закрытом состоянии `.emoji-card` в документе ровно один — стационарный.

**Глобальные запросы, которые поповер не задевает** `[F]`. Это не рекомендации, а замороженные места, где второй экземпляр ломает поведение:

| Запрос | Где | Почему важен |
|---|---|---|
| `[data-action="emoji-close"]` | `rust/gui-spike/ui/features/actions.js:905` — `"emoji-close": () => router.switchScreen("chat")`; заморожен `tests/unit/gui-spike-features.test.mjs:853-864` | Кнопка «×» поповера обязана нести **свой** `data-action="reaction-picker-close"`. С `emoji-close` она увела бы на экран чата вместо закрытия, а хелпер `action(app, "emoji-close")` взял бы первый узел в документе |
| `[data-action]` | `actions.js:925-928` — прямой `addEventListener` на каждый узел **при инициализации** | Лениво созданная кнопка закрытия не попадает в этот `NodeList`: обработчик `reaction-picker-close` навешивается в `openReactionPicker()` (либо делегируется на корень поповера) |
| `document.querySelector(".emoji-card")` | `actions.js:1109` — верхний слой при `router.current === "emoji"` | Второй `.emoji-card` делает результат зависимым от порядка узлов; пока открыт стационарный экран, поповер в DOM отсутствует |
| `document.querySelectorAll(".tab")` | `rust/gui-spike/ui/app.js:1625` — бенч-смоук берёт `[1]`; `tests/unit/gui-spike-features.test.mjs:377` требует ровно `EMOJI_CATEGORIES` (10) | Любой смонтированный — даже `hidden` — экземпляр даёт 20 вкладок и валит замороженный E-07 |

Требование: после `closeReactionPicker()` в документе нет ни `#reaction-picker-popover`, ни его `.tab`/`.emoji-cell`; проверка — N-03 (§8.2).

```html
<section id="reaction-picker-popover"
         class="emoji-card reaction-picker-popover"
         role="dialog"
         aria-modal="false"
         aria-label="Выбор реакции"
         data-state="ready"
         data-target-channel-id="1100000000000000001"
         data-target-message-id="1200000000000000001">

  <!-- 1. Шапка: поиск по названию и кнопка закрытия -->
  <header class="emoji-head">
    <search class="search-field" role="search" aria-label="Поиск эмодзи">
      <svg class="icon" aria-hidden="true"><use href="#i-search"/></svg>
      <input id="reaction-picker-search"
             class="search-input"
             type="search"
             placeholder="Найдите идеальный эмодзи"
             aria-label="Поиск эмодзи"
             aria-controls="reaction-picker-grid"
             autocomplete="off"
             spellcheck="false" />
    </search>
    <button type="button"
            class="icon-btn emoji-close-btn"
            data-action="reaction-picker-close"
            aria-label="Закрыть пикер эмодзи"
            title="Закрыть (Esc)">
      <svg class="icon" aria-hidden="true"><use href="#i-close"/></svg>
    </button>
  </header>

  <!-- 2. Опциональный баннер ошибки, скрыт в состоянии ready.
       role="alert" несёт СТАТИЧНЫЙ текст (озвучивается один раз); счётчик секунд —
       отдельный узел с aria-live="off", иначе AT прерывается каждую секунду. -->
  <div id="reaction-picker-banner"
       class="reaction-picker-banner"
       role="alert"
       aria-live="assertive"
       data-error-kind=""
       hidden>
    <span class="reaction-picker-banner-icon" aria-hidden="true">!</span>
    <span class="reaction-picker-banner-text">Не удалось поставить реакцию.</span>
    <span class="reaction-picker-banner-timer" aria-live="off"></span>
    <!-- data-action здесь — только маркер (тот же приём, что у лениво созданной
         .guild-add, app.js:555-566). Глобальный биндер actions.js:925-928 узел не
         увидит: поповер монтируется после initActions. Обработчик навешивается
         напрямую в openReactionPicker() вместе с close — §5.3 п.1 и п.3. -->
    <button type="button"
            class="reaction-picker-banner-dismiss"
            data-action="dismiss-picker-error"
            aria-label="Скрыть уведомление об ошибке">×</button>
  </div>

  <!-- 3. Основное тело: левый вертикальный рейл 10 категорий (48px) + правая область сетки -->
  <div class="emoji-body">

    <!-- Вертикальный рейл категорий: 10 вкладок, порядок и названия — EMOJI_CATEGORIES
         (emoji.js:22-275). aria-controls обязателен: замороженный тест
         tests/unit/gui-spike-features.test.mjs:386 требует ровно "emoji-grid"
         (у стационарного экрана id без префикса). -->
    <div id="reaction-picker-tabs"
         class="emoji-tabs"
         role="tablist"
         aria-label="Категории эмодзи"
         aria-orientation="vertical">
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-0"
              aria-controls="reaction-picker-grid"
              aria-selected="true" tabindex="0"
              aria-label="Часто используемые" title="Часто используемые">🕘</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-1"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Смайлы" title="Смайлы">😀</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-2"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Люди и жесты" title="Люди и жесты">🧑</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-3"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Еда и напитки" title="Еда и напитки">🍔</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-4"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Природа" title="Природа">🌿</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-5"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Транспорт" title="Транспорт">🚗</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-6"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Предметы" title="Предметы">💡</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-7"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Символы" title="Символы">❤️</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-8"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Флаги" title="Флаги">🏳️</button>
      <button type="button" class="tab" role="tab" id="reaction-picker-tab-9"
              aria-controls="reaction-picker-grid"
              aria-selected="false" tabindex="-1"
              aria-label="Прочее" title="Прочее">🎮</button>
    </div>

    <!-- Правая колонка. role="tabpanel" — панель активной вкладки: APG требует
         tablist → tabpanel, а сам #…-grid остаётся role="listbox" (заморожено). -->
    <div class="emoji-grid-viewport"
         role="tabpanel"
         aria-labelledby="reaction-picker-tab-0"
         tabindex="-1">
      <!-- Опциональный заголовок активной категории (не sticky: заголовок
           прокручивается вместе с содержимым, правила position: sticky нет) -->
      <div id="reaction-picker-section-header" class="emoji-section-header" aria-hidden="true">
        <span class="emoji-section-title">Часто используемые</span>
        <span class="emoji-section-count">12</span>
      </div>

      <!-- Сетка эмодзи: 9 колонок по 44×44px, плоский listbox (D-07, §2.4).
           aria-label — замороженный "Эмодзи" (DOM-CONTRACT.md:347). -->
      <div id="reaction-picker-grid"
           class="emoji-grid"
           role="listbox"
           aria-label="Эмодзи"
           data-cols="9">

        <!-- Активная ячейка: фокус roving tabindex="0"; "моя" реакция — только data-reacted
             (aria-pressed у role="option" недопустим, см. §5.1). title === textContent. -->
        <button type="button"
                class="emoji-cell"
                role="option"
                tabindex="0"
                aria-selected="true"
                data-emoji="👍"
                data-reacted="true"
                aria-label="палец вверх, выбрана"
                title="👍">👍</button>

        <!-- Обычная ячейка (tabindex="-1", реакция ещё не выбрана) -->
        <button type="button"
                class="emoji-cell"
                role="option"
                tabindex="-1"
                aria-selected="false"
                data-emoji="🔥"
                data-reacted="false"
                aria-label="огонь"
                title="🔥">🔥</button>

        <!-- ... остальные ячейки категории или результатов поиска ... -->
      </div>

      <!-- Состояние загрузки (data-state="loading", показывается при ожидании каталога) -->
      <div id="reaction-picker-loading"
           class="emoji-loading-state"
           role="status"
           aria-live="polite"
           hidden>
        <div class="emoji-skeleton-grid" aria-hidden="true">
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
          <span class="emoji-skeleton-cell"></span>
        </div>
        <span class="emoji-loading-label">Загрузка эмодзи…</span>
      </div>

      <!-- Состояние пустого поиска; видимо только при data-state="empty" -->
      <div id="reaction-picker-empty"
           class="emoji-empty-state"
           role="status"
           aria-live="polite"
           hidden>
        <span class="emoji-empty-glyph" aria-hidden="true">🔎</span>
        <strong class="emoji-empty-title">Ничего не найдено</strong>
        <span class="emoji-empty-hint">Попробуйте другой запрос</span>
      </div>
    </div>
  </div>

  <!-- 4. Нижняя панель предпросмотра активного эмодзи (48px) -->
  <footer class="emoji-preview" aria-live="polite">
    <span class="emoji-preview-glyph" aria-hidden="true">👍</span>
    <span class="emoji-preview-name">палец вверх</span>
    <span class="emoji-preview-category muted">Часто используемые</span>
  </footer>
</section>
```

Загрузочное состояние имеет префиксный id `#reaction-picker-loading` (тот же приём, что выше).

**Исключение APG для `aria-controls` вкладки** `[A]`: вкладка `.tab` ссылается на `#…-grid` (`role="listbox"`), а не на `.emoji-grid-viewport[role="tabpanel"]`. APG ожидает `tab → tabpanel`; замороженный тест `tests/unit/gui-spike-features.test.mjs:386` требует ровно `aria-controls === "emoji-grid"`. Компромисс принят сознательно: `tabpanel` получает `aria-labelledby` активной вкладки и остаётся обёрткой, а вкладка управляет фактическим содержимым панели — listbox'ом. Это исключение, а не ошибка разметки; менять его можно только вместе с замороженным тестом.

### 2.4. Согласование WAI-ARIA паттернов: только `role="listbox"` (`D-07` / `[F]`)

Замороженный контракт — плоский `listbox` (D-07): `#emoji-grid[role="listbox"]` с прямыми детьми `button.emoji-cell[role="option"]`, без строк-обёрток. Это проверяют `tests/unit/gui-spike-features.test.mjs:372-403` (`.emoji-row` = 0, `role="listbox"`, `role="option"`), `tests/unit/gui-spike-dom.test.mjs:494-498` и `DOM-CONTRACT.md:347`, а `index.html:148-150` прямо фиксирует, что `aria-colcount`/`aria-rowcount` вернутся только вместе с `role="grid"`.

Поэтому **Режим B (`role="grid"` + `.emoji-row[role="row"]` + `gridRole: "grid"`) из спецификации удалён**:
- опции `gridRole` у `createEmojiConsole(options)` нет (`emoji.js:329-341` принимает `tabs/grid/search/previewGlyph/previewName/onSelect`), то есть режим был нереализуем;
- `.emoji-row { display: contents }` противоречит шапке `emoji.css:4-5` и выбрасывает `role="row"` из a11y-дерева в части браузеров. `DOM-CONTRACT.md §9.2 (D-05)` требует обратного — эта строка **считается устаревшей**: правку `DOM-CONTRACT.md` делает PM отдельной карточкой, спецификация её не подменяет.

**Что остаётся и чем платим:**
- Визуальная раскладка двумерная (`9` колонок по `44×44px`, зазор `4px`), а роль — одномерный `listbox`. Двумерные стрелки (`ArrowDown` $= +cols$, `ArrowUp` $= -cols$) — осознанное расширение: APG не запрещает дополнительные клавиши в `listbox`, а без них визуальный порядок и фокус расходятся. `aria-colcount`/`aria-rowcount` не выставляются (это атрибуты `grid`).
- Шаг по вертикали берётся из фактического числа колонок: `Number(grid.dataset.cols) || EMOJI_COLS` (`EMOJI_COLS = 9`, `emoji.js:14`). Рендер выставляет `grid.dataset.cols` по числу треков `gridTemplateColumns` в браузере; в DOM-шиме `getComputedStyle` нет, поэтому остаётся фолбэк `9` и замороженный тест `gui-spike-features.test.mjs:405-426` не меняется. Без этого при `auto-fill` на окне $<540$px `ArrowDown` прыгал бы на 9 при 6 видимых.

---

## 3. Матрица состояний (State Machine & Visual States)

### 3.1. Состояния пилюли `.reaction` и кнопки быстрой реакции `.msg-toolbar-btn`

Все значения — dark-палитра `tokens.css`. Заливки с альфой указаны вместе с композитом на своей подложке (пилюля живёт на `--bg-chat #1a1a1e`, кнопка тулбара — на `--surface-4 #2c2d32`). Контраст границы измеряется к **внешнему** фону (смежная поверхность под контролом) — это та пара, которую требует SC 1.4.11; коэффициенты — §7.2.

| Состояние | DOM-селектор и атрибуты | Визуальное оформление (100% токены `tokens.css`) | Поведение при взаимодействии |
|---|---|---|---|
| **1. Чужая реакция (`idle`, `me = false`)** | `.reaction[data-me="false"][aria-pressed="false"]` | Фон: `var(--state-hover)` (`rgb(255 255 255 / 6%)` → `#28282c`); рамка: `var(--menu-border)` solid `var(--accent)` (`#5865f2`, **3.77:1** к `--bg-chat`); скругление: `var(--radius-sm)` (`8px`); счётчик `.reaction-count`: `var(--text)` (`#efeff1`, **12.79:1** на `#28282c`). | Клик вызывает `requestReaction(key, emoji, base, { channelId, messageId, element, onSettled })` с `active = true` (`PUT .../@me`), оптимистично переводя пилюлю в `me = true`, `count + 1`. |
| **2. Наведение (`hover`, `me = false`)** | `.reaction[data-me="false"]:hover:not(:disabled):not([aria-disabled="true"])` | Фон: `var(--surface-3)` (`#28282d`); рамка: `var(--accent)` (**3.18:1** к заливке, **3.77:1** к чату); счётчик: `var(--text-strong)` (`#fbfbfb`, **14.18:1** на `--surface-3`). Переход `var(--motion-fast) var(--ease-out)`. | Курсор `pointer`. Тултип-превью не дублируется: `aria-label` уже содержит эмодзи и счётчик. |
| **3. Нажатие (`:active`)** | `.reaction:active:not(:disabled):not([aria-disabled="true"])` | Фон: `var(--state-active)` (`rgb(255 255 255 / 9%)` → `#2f2f32`); рамка остаётся `var(--accent)` (**3.77:1** к чату). | Курсор `pointer`. Визуальный отклик нажатия. |
| **4. Выбранная реакция (`selected`, `me = true`)** | `.reaction[data-me="true"]`, `.reaction[aria-pressed="true"]`, `.msg-toolbar-btn[data-command="react"][aria-pressed="true"]` | Фон: `var(--accent-soft)` (`rgb(88 101 242 / 16%)` → `#242640` на чате, `#333651` на `--surface-4`); рамка: `var(--accent)` (**3.77:1** к чату); счётчик `.reaction-count`: `var(--text-accent)` (`#a5b0ff`, **7.19:1** на `#242640`), вес `var(--font-weight-semibold)` (`600`). | Клик вызывает `requestReaction(...)` с `active = false` (`DELETE .../@me`), оптимистично уменьшая `count - 1` (пилюля удаляется при `count === 0`). |
| **5. Наведение на выбранную (`selected:hover`)** | `.reaction[data-me="true"]:hover:not(:disabled):not([aria-disabled="true"])` | Фон и рамка — как в п.4 (рамка `var(--accent)`); меняется **только** цвет счётчика на `var(--text-strong)` (`#fbfbfb`, **14.23:1**). `var(--accent-hover)` (`#4654c0`, 2.72:1 к чату) на границе контрола **не используется** — он не проходит 3:1. | Курсор `pointer`. |
| **6. Ожидание ответа (`pending` / `in-flight`)** | `.reaction[data-pending="true"][aria-busy="true"]`, `.msg-toolbar-btn[data-pending="true"]` | Оптимистичное состояние (п.1/п.4) уже отображено **без изменения прозрачности**: `opacity: 1`; счётчик на время запроса — `var(--text-muted)` (`#96979e`, **5.05:1** на `#28282c`); `cursor: progress`. `opacity: var(--state-disabled-opacity)` для pending **не применяется**: 0.45 уронило бы и текст, и границу ниже AA. У `.reaction` фон и в этом состоянии остаётся `var(--accent-soft)` → `#242640`, поэтому `--text-muted` даёт 5.06:1 и AA проходит; у кнопки тулбара фон композитится в `#333651`, и то же `--text-muted` дало бы 4.04:1 — для неё действует п.10. | Повторные клики по тому же `${String(key)}:${emoji}` отсекаются `pending.has(requestKey)` (`reactions.js:166-167`); второй IPC не уходит. |
| **7. Заблокировано (`403` / кулдаун `429`)** | `.reaction[aria-disabled="true"]`, `.reaction:disabled`, `.reaction-add-btn[aria-disabled="true"]` | `opacity: var(--state-disabled-opacity)` (`0.45`), `cursor: not-allowed`; фон и рамка **не меняются** (остаются как в п.1/п.4, §7.1). Контраст не нормируется: SC 1.4.11/1.4.3 исключают неактивные компоненты. | Обработчик обязан проверять `aria-disabled` и выходить **до** сети (`pointer-events` не используется, чтобы не отнимать фокус и тултип); §7.1 п.5. |
| **8. Фокус с клавиатуры (`:focus-visible`)** | `.reaction:focus-visible`, `.reaction-add-btn:focus-visible`, `.msg-toolbar-btn:focus-visible` | `outline: var(--focus-ring-width) solid var(--focus-ring)` (`2px`, `#a5b0ff`), `outline-offset: var(--focus-ring-offset)` (`2px`). Контраст к `--bg-chat` — **8.47:1**, к `--surface-2` — **7.54:1**. | Активация по `Enter` или `Space`. |
| **9. Выбранная кнопка тулбара** | `.msg-toolbar-btn[aria-pressed="true"]`, `.msg-toolbar-btn[aria-expanded="true"]` | Фон: `var(--accent-soft)` → `#333651` на `--surface-4`; рамка `var(--accent)`; иконка `var(--text-accent)` (**6.71:1** к `--surface-4`, **5.74:1** к `#333651`). | Повторный клик снимает реакцию (toggle). |
| **10. Ожидание ответа на выбранной кнопке тулбара (`pending` + `pressed`)** | `.msg-toolbar-btn[data-pending="true"][aria-pressed="true"]`, `.msg-toolbar-btn[data-pending="true"][data-me="true"]` | Фон и рамка — как в п.9 (`var(--accent-soft)` → `#333651`, рамка `var(--accent)`); цвет иконки остаётся `var(--text-accent)` — **5.74:1**, а не `var(--text-muted)` с его 4.04:1 на `#333651`. `opacity: 1`, `cursor: progress`. Правило §7.1 стоит после правила `pending` и перебивает его по специфичности: (0,3,0) против (0,2,0). | Клики отсекаются `pending` (`reactions.js:166-167`); состояние достижимо всегда, когда быстрая реакция ставится из тулбара: `data-pending` ставится синхронно (`reactions.js:176-177`) до `await invoke`, а `syncPressed` тут же выставляет `aria-pressed="true"` из оптимистичного `overrides` (`message-toolbar.js:224-225`, `reactions.js:147`). |


---

### 3.2. Состояния поповера выбора реакций (`#reaction-picker-popover` / `.emoji-card`)

Поповер управляется атрибутом `data-state` на корневом элементе `.emoji-card`:

```
                  ┌──────────────────────┐
                  │  data-state="loading"│ (aria-busy="true", скелетон 9 ячеек)
                  └──────────┬───────────┘
                             │ каталог инициализирован
                             ▼
                  ┌──────────────────────┐
        ┌────────►│  data-state="ready"  │◄───────────────────────┐
        │         └────┬───────────┬─────┘                        │
        │              │           │                              │
  очистка поиска       │           │ запрос не дал совпадений     │ таймер retry_after_ms истёк
  или найдены эмодзи   │           │ (visible.length === 0)       │ или закрыт баннер ошибки
        │              │           ▼                              │
        │              │  ┌──────────────────────┐                │
        │              └──┤  data-state="empty"  │                │
        │                 └──────────────────────┘                │
        │                          │                              │
        │             ошибка PUT / DELETE .../@me                 │
        │                          ▼                              │
        │         ┌────────────────────────────────┐              │
        └─────────┤ data-state="error-429"         ├──────────────┘
                  │ data-state="error-403"         │
                  │ data-state="error"             │
                  └────────────────────────────────┘
```

#### 3.2.1. Состояние 1: Загрузка (`data-state="loading"`)
- **Когда возникает**: при первичной асинхронной подгрузке серверных эмодзи гильдии (`custom`) либо при открытии пикера до готовности данных канала.
- **DOM и ARIA**:
  - На `#reaction-picker-popover`: `data-state="loading"`, `aria-busy="true"`.
  - Узел `#reaction-picker-loading` видим (`hidden = false`), содержит 9 скелетон-ячеек `.emoji-skeleton-cell` размером `var(--emoji-cell)` (`44×44px`) со скруглением `var(--radius-sm)` (`8px`) и фоном `var(--surface-3)`, и подпись `.emoji-loading-label` («Загрузка эмодзи…», цвет `var(--text-muted)`, 5.31:1 на `--surface-2`).
  - Скелетон — декоративный (`aria-hidden="true"`), его контраст к карточке (1.05:1) не нормируется: он не передаёт состояние, состояние озвучивает подпись. При `prefers-reduced-motion` анимация не добавляется (токены движения уже обнуляются, `tokens.css:349-355`).
  - Поле поиска `#reaction-picker-search` остаётся доступным для фокуса, чтобы не терять `openerElement` и обрабатывать `Escape`.

#### 3.2.2. Состояние 2: Штатный режим и выбранная реакция (`data-state="ready"`)
- **Когда возникает**: в сетке `#reaction-picker-grid` есть $\ge 1$ эмодзи (`visible.length > 0`).
- **Отображение уже выбранных реакций сообщения** `[A]` (пикер в коде отсутствует):
  - При открытии поповера для сообщения пикер вызывает `reactionsFor(channelReactionKey(channelId, messageId), base)` (`reactions.js:103-116`), а не `reactionsFor(targetMessage)` — сигнатуры с объектом сообщения нет.
  - Для каждого эмодзи с `me === true` ячейка `.emoji-cell` получает `data-reacted="true"` и суффикс `", выбрана"` в `aria-label`. `aria-pressed` на ячейку **не выставляется**: у `role="option"` его нет, состояние выбора несёт `aria-selected` (§5.1).
  - Визуально ячейка `.emoji-cell[data-reacted="true"]` получает фон `var(--surface-3)` и рамку `var(--menu-border) solid var(--accent)`; рамка даёт **3.18:1** к заливке и **3.35:1** к `--surface-2` карточки (§7.2.2). Заливка `var(--accent-soft)` на `--surface-2` давала бы только 2.86:1 к рамке и не используется.
  - При наведении или перемещении `roving tabindex="0"` футер `.emoji-preview` синхронно обновляет `.emoji-preview-glyph` (`emoji.glyph`) и `.emoji-preview-name` (`emoji.name` — человекочитаемое имя, как в `emoji.js:380-381`; формы `:${name}:` в модели нет).
  - `.emoji-preview-category` в текущем коде не заполняется: `createEmojiConsole` принимает только `previewGlyph`/`previewName` (`emoji.js:333-336`), поэтому категория в футере — новая опция, отдельный пункт карточки N-03 (§8).

#### 3.2.3. Состояние 3: Пустой поиск эмодзи (`data-state="empty"`)
- **Нормализация поискового запроса (`normalizeEmojiQuery` в `rust/gui-spike/ui/features/emoji.js:283-290`)** `[F]`:
  ```js
  export function normalizeEmojiQuery(raw) {
    return String(raw)
      .trim()
      .replace(/^:+/, "")
      .replace(/:+$/, "")
      .replace(/[\s_]+/g, "")
      .toLowerCase();
  }
  ```
  - Двоеточия по краям (`:fire:` $\to$ `fire`), пробелы и подчёркивания (`thumbs_up` $\to$ `thumbsup`) отбрасываются; регистр приводится к нижнему.
  - При непустом запросе поиск `searchAllEmojis(query)` (`:300-317`) выполняется **сразу по всем 10 категориям** с дедупликацией по паре `glyph + "\u0000" + name`, игнорируя фильтр текущей вкладки `#reaction-picker-tabs`.
  - Совпадение проверяется по конкатенации нормализованного `name` и самого `glyph` (`:310`); списка `keywords` в модели нет.
- **Поведение при `visible.length === 0`** `[A]`:
  - Корневой элемент получает `data-state="empty"`.
  - Сетка `#reaction-picker-grid` очищается (`replaceChildren()`, `emoji.js:411`), узел `#reaction-picker-empty` (у стационарного экрана — свой, без префикса) становится видимым (`hidden = false`) и выводит:
    - Иконку `.emoji-empty-glyph`: `🔎` (`font-size: var(--font-size-display)`, `24px`);
    - Заголовок `.emoji-empty-title`: `Ничего не найдено` (`color: var(--text-strong)`, 14.93:1 на `--surface-2`; `font-size: var(--font-size-ui)`, `font-weight: var(--font-weight-semibold)`);
    - Подсказку `.emoji-empty-hint`: `Попробуйте другой запрос` (`color: var(--text-muted)`, 5.31:1; `font-size: var(--font-size-small)`).
  - Нижний футер `.emoji-preview` синхронно выставляет (`emoji.js:365-370`):
    - `.emoji-preview-glyph.textContent = "🙂"`;
    - `.emoji-preview-name.textContent = "Ничего не найдено"`.
  - Сброс поля `#…-search` (кнопкой очистки или клавишей `Backspace`) немедленно возвращает `data-state="ready"` и восстанавливает сетку выбранной категории с `activeIndex = 0`.

#### 3.2.4. Состояние 4: Ошибка `429 Too Many Requests` (`data-state="error-429"` / `kind: "rateLimited"`)
- **Откуда берётся задержка** `[F]`:
  - `reactions.rs` **не содержит** `parse_retry_after_ms` (такая функция есть в `invites.rs`/`notifications.rs`, оттуда и попала в прошлую редакцию). Разбор `Retry-After`/тела живёт в `discord-core/src/rest/rate_limit.rs:141-179` (приоритет: `retry-after` → тело `retry_after` → `x-ratelimit-reset-after`; отсутствие всех трёх → `Duration::ZERO`).
  - `ApiClient::execute` (`discord-core/src/rest/client.rs:399-430`) на `429` **сам ретраит** до `ApiRetryPolicy::max_attempts = 3` (`client.rs:210-219`) с паузой `min(retry_after, 30s)` (`rate_limit.rs:210-225`), причём `PUT` и `DELETE` идемпотентны, поэтому ретраятся и `5xx`. Ошибка доезжает до UI только после исчерпания попыток.
  - Два пути значения:
    1. `ApiError::RateLimited { retry_after, .. }` → `RestFailure::from_api_error` (`rest_failure.rs:85-91`) → `ReactionError::RateLimited { retry_after_ms }` с **реальным** значением из ответа;
    2. «сырой» статус через `RestFailure::from_status` (`rest_failure.rs:65-76`) даёт `retry_after_ms: 0` — тело и заголовки на этом пути не разбираются.
  - Сериализация (serde, `reactions.rs:18-19`, `#[serde(tag = "kind", rename_all = "camelCase")]`): тег `kind` в **camelCase**, поля не переименовываются (`retry_after_ms` остаётся snake_case), поля `message` **нет** (`Display` в JSON не попадает):
    ```json
    { "kind": "rateLimited", "retry_after_ms": 1500 }
    ```
    Пример `{"retry_after_ms": 1500}` достижим только по пути 1; при `retry_after_ms: 0` (путь 2, а также 429 без `Retry-After` и без тела) значение `0` — валидный вход.
- **Реакция UI при `kind === "rateLimited"`** `[A]`:
  1. **Немедленный откат оптимистичного патча**: `requestReaction` восстанавливает `before` в `overrides` (`reactions.js:191-195`) — пилюля возвращается к прежним `count`/`me`.
  2. **Кулдаун**:
     - `remainingSec = Math.max(1, Math.ceil(retry_after_ms / 1000))` — floor в **1 секунду** обязателен: при `retry_after_ms = 0` баннер всё равно живёт 1 с, «мгновенного» повтора нет. Это правило фиксируется здесь, а не выводится из `Math.max` в коде.
     - Если открыт поповер, он переходит в `data-state="error-429"`, баннер `#reaction-picker-banner[data-error-kind="rateLimited"]` показывается (`hidden = false`) со **статичным** текстом `Не удалось поставить реакцию: превышен лимит запросов.`; счётчик `«Повтор через N с»` живёт в `.reaction-picker-banner-timer[aria-live="off"]` и обновляется раз в `1000` мс. `role="alert"` переписывать каждую секунду нельзя — AT будет прерываться (§3.2.4.1).
     - Если реакция ставилась без пикера (клик по пилюле или кнопке тулбара), та же пара «статичный alert + `aria-live="off"` счётчик» выводится в `.reaction-error-status`.
     - На время кулдауна (`Date.now() < rateLimitUntilMs`) кнопки постановки получают `aria-disabled="true"` и **no-op обработчик** (§7.1 п.5). По истечении таймера баннер скрывается, кнопки разблокируются, один раз озвучивается `role="status" aria-live="polite"`: `Можно повторить.` Ручное закрытие баннера кнопкой «×» кулдаун **не** прерывает: баннер скрывается сразу, `data-state` возвращается к состоянию, предшествовавшему ошибке, а `aria-disabled` снимается только по таймеру (§5.3 п.3).
  3. **Цветовая семантика баннера 429** (строго по `tokens.css`):
     - Поверхность `var(--surface-3)` (`#28282d`), нижняя граница `var(--menu-border) solid var(--warning)` (`#fdb833`, **8.44:1** к поверхности — non-text индикатор проходит), бейдж иконки — фон `var(--warning)`, текст `var(--text-on-warning)` (`#121214`, **10.76:1**), основной текст `var(--text-strong)` (**14.18:1** к `--surface-3`).
- **3.2.4.1. Почему счётчик не в `role="alert"`**: живой регион `assertive` с текстом, меняющимся раз в секунду, прерывает скринридер на каждом тике. Поэтому alert содержит только статичную формулировку один раз за инцидент, а число секунд — `aria-live="off"`; окончание кулдауна объявляет отдельный `polite`-статус.

#### 3.2.5. Состояние 5: Ошибка `403 Forbidden` и прочие ошибки (`data-state="error-403"` / `data-state="error"`)

**Формат ошибки IPC** `[F]`: `#[tauri::command] toggle_reaction` возвращает `Result<(), ReactionError>` (`main.rs:391-409`), `ReactionError` сериализуется `#[serde(tag = "kind", rename_all = "camelCase")]` (`reactions.rs:18-19`). Unit-варианты дают `{"kind":"forbidden"}`, структурные — с полями (`{"kind":"server","status":500}`, `{"kind":"client","status":400}`, `{"kind":"rateLimited","retry_after_ms":0}`). Поля **не** переименовываются, поля `message` **нет**: `Display` (`reactions.rs:44-61`) используется только для логов и в IPC не попадает.

Маппинг десяти вариантов:

| Вариант `ReactionError` → `kind` | HTTP-статус / условие | `Display` из `reactions.rs:44-61` (только для логов) | Текст в UI (`.reaction-picker-banner-text` / `.reaction-error-text`) | Поведение UI после отката оптимистичного состояния |
|---|---|---|---|---|
| `rateLimited` | `429` после исчерпания автоповторов `ApiClient` | `ограничение частоты; повтор через {retry_after_ms} мс` | статичный alert: `Не удалось поставить реакцию: превышен лимит запросов.` + `aria-live="off"`: `Повтор через {N} с` | Откат пилюли; кулдаун `max(1, ceil(retry_after_ms/1000))` с; кнопки `aria-disabled` до конца кулдауна. |
| `forbidden` | `403 Forbidden` | `нет доступа к каналу` | `Недостаточно прав для добавления реакции в этом канале.` | Откат пилюли; `.reaction-add-btn` и ячейки пикера для канала → `aria-disabled="true"`; текст `var(--danger-text)` (7.63:1 на `--surface-2`), рамка `var(--danger)` (3.50:1). |
| `unauthorized` | `401 Unauthorized` или отсутствие токена в `AuthBackend` | `сессия истекла` | `Сессия истекла — войдите заново.` | Откат пилюли; текст `var(--danger-text)`. |
| `notFound` | `404 Not Found` | `сообщение или канал не найдены` | `Сообщение или канал не найдены.` | Откат пилюли; пикер закрывается, если удалено целевое сообщение. |
| `client` | прочие `4xx` (кроме 401/403/404/429), например `400` | `запрос отклонён (HTTP {status})` | `Не удалось поставить реакцию (HTTP {status}).` | Откат пилюли; повторный клик разрешён. |
| `server` | `5xx` (после автоповторов: `PUT`/`DELETE` идемпотентны) | `ошибка сервера Discord (HTTP {status})` | `Discord ответил ошибкой (HTTP {status}). Попробуйте позже.` | Откат пилюли; повторный клик разрешён. |
| `network` | сокет / DNS / таймаут / невалидная база | `сеть недоступна` | `Сеть недоступна. Проверьте подключение.` | Откат пилюли; повторный клик разрешён. |
| `storage` | ошибка чтения OS keyring | `хранилище токена недоступно` | `Нет доступа к хранилищу токена. Войдите заново.` | Откат пилюли; текст `var(--danger-text)`. |
| `invalidArg` | нечисловой `channel_id`/`message_id`, пустой эмодзи, `> 256` символов, пустой токен в `DiscordReactionApi::new` | `некорректные параметры реакции` | `Некорректный эмодзи или идентификатор сообщения.` | Запрос отсекается **до** сети (`reactions.rs:169-178`); пилюля не мутируется. |
| `invalidResponse` | `2xx`, но ответ не разобран | `некорректный ответ реакции` | `Не удалось разобрать ответ Discord. Повторите попытку.` | Откат пилюли; повторный клик разрешён. |

**Обязательное требование к UI-матчеру** `[F]`: сравнивать `kind` в camelCase. Текущий `reactions.js:196-203` проверяет `details?.kind === "Unauthorized"` и `"RateLimited"` (PascalCase) — такие ветки **никогда не срабатывают** на serde-выхлопе; исправление матчера входит в карточку N-02 (§8) и проверяется red-first тестом на форму `{"kind":"unauthorized"}`. Отсутствие токена в `main.rs:401-405` даёт `Storage` (ошибка хранилища) либо `Unauthorized` (нет токена) — оба варианта unit-ные, без текста.

---

## 4. Адаптивные размеры, геометрия и позиционирование

### 4.1. Точная геометрия компонентов (100% токены `rust/gui-spike/ui/tokens.css`)

| Компонент / узел | CSS-свойство | Токен из `tokens.css` | Фактическое значение | Источник |
|---|---|---|---|---|
| **Поповер пикера (`.emoji-card`)** | `width` | `var(--emoji-picker-w)` | `498px` | `[T]` `tokens.css:308`, `[M]` `21b-emoji-full.json` |
| | `height` | `var(--emoji-picker-h)` | `440px` | `[T]` `tokens.css:309` |
| | `border-radius` | `var(--radius-md)` | `12px` | `[T]` `tokens.css:194` |
| | `box-shadow` | `var(--shadow-popover)` | `0 8px 16px rgb(0 0 0/24%), 0 0 0 1px rgb(0 0 0/16%)` (high **+** stroke) | `[T]` `tokens.css:207-209` |
| | `z-index` | `var(--z-popover)` | `100` | `[T]` `tokens.css:338` |
| **Шапка пикера (`.emoji-head`)** | `padding` | `var(--space-5) var(--space-6)` | `12px 16px` | `[T]` `tokens.css:179-180` |
| | `gap` | `var(--space-5)` | `12px` | `[T]` `tokens.css:179` |
| **Поле поиска (`.search-field`)** | `min-height` | `var(--search-height)` | `32px` | `[T]` `tokens.css:293` |
| | `border-radius` | `var(--search-radius)` | `var(--radius-round)` = пилюля, **не** 8px | `[T]` `tokens.css:294` |
| **Рейл категорий (`.emoji-tabs`)** | ширина колонки (`.emoji-body`) | `var(--emoji-rail-w)` | `48px` | `[T]` `tokens.css:310` |
| | `padding` | `var(--space-4)` | `8px` | `[T]` `tokens.css:178` |
| | `gap` | `var(--space-2)` | `4px` | `[T]` `tokens.css:176` |
| **Кнопка категории (`.emoji-tabs .tab`)** | `width × height` | `var(--btn-height) × var(--btn-height)` | `32px × 32px` | `[T]` `tokens.css:288` |
| | `border-radius` | `var(--radius-sm)` | `8px` | `[T]` `tokens.css:193` |
| **Заголовок секции (`.emoji-section-header`)** | `height` | `var(--emoji-section-h)` | `32px` | `[T]` `tokens.css:314` |
| **Сетка эмодзи (`#emoji-grid`)** | `grid-template-columns` | `repeat(var(--emoji-cols), var(--emoji-cell))` | `9 × 44px` = `396px` | `[T]` `tokens.css:311-312` |
| | `grid-auto-rows` | `var(--emoji-cell)` | `44px` | `[T]` `tokens.css:311` |
| | `gap` | `var(--emoji-gap)` | `4px` (`8 × 4px = 32px`) | `[T]` `tokens.css:313` |
| | `padding` | `var(--space-3)` | `6px` — **изменено** (было `--space-5` = 12px, бюджет не сходился) | `[T]` `tokens.css:177` |
| | `scrollbar` | `var(--scrollbar-size)` | `8px` (резерв под классический скроллбар) | `[T]` `tokens.css:138` |
| **Ячейка эмодзи (`.emoji-cell`)** | `width × height` | `var(--emoji-cell) × var(--emoji-cell)` | `44px × 44px` | `[T]` `tokens.css:311`, `[M]` |
| | `border-radius` | `var(--radius-sm)` | `8px` | `[T]` `tokens.css:193` |
| | `font-size` | `var(--font-size-title)` | `20px` | `[T]` `tokens.css:153` |
| **Футер превью (`.emoji-preview`)** | `min-height` | `var(--space-11)` | `48px` | `[T]` `tokens.css:185` |
| | `padding` | `var(--space-5) var(--space-6)` | `12px 16px` | `[T]` `tokens.css:179-180` |
| **Ряд пилюль (`.reactions-row`)** | `gap` | `var(--space-3)` | `6px` | `[T]` `tokens.css:177` |
| | `margin-top` | `var(--space-2)` | `4px` | `[T]` `tokens.css:176` |
| **Пилюля реакции (`.reaction`)** | `min-height` | `var(--layout-group-title-h)` | `24px` | `[T]` `tokens.css:226` |
| | `padding` | `var(--space-1) var(--space-4)` | `2px 8px` | `[T]` `tokens.css:175,178` |
| | `gap` | `var(--space-3)` | `6px` | `[T]` `tokens.css:177` |
| | `border-radius` | `var(--radius-sm)` | `8px` | `[T]` `tokens.css:193` |
| | `font-size` / `line-height` | `var(--font-size-small)` / `var(--line-height-small)` | `12px` / `16px` | `[T]` `tokens.css:150,157` |
| **Плавающий тулбар (`.msg-toolbar`)** | `padding` | `var(--menu-pad)` | `8px` | `[T]` `tokens.css:317` |
| | `gap` | `var(--space-1)` | `2px` | `[T]` `tokens.css:175` |
| | `border-radius` | `var(--menu-radius)` (= `--radius-sm`) | `8px` | `[T]` `tokens.css:319` |
| **Кнопка тулбара (`.msg-toolbar-btn`)** | `min-width × height` | `var(--row-menu-item) × var(--row-menu-item)` | `32px × 32px` (ширина `auto`, чтобы влезла текстовая подпись) | `[T]` `tokens.css:261` |

#### Проверка горизонтального бюджета ширины пикера (`498px`):
$$\text{Width} = \underbrace{48}_{\text{--emoji-rail-w}} + \underbrace{6 + 6}_{2 \times \text{--space-3 padding}} + \underbrace{9 \times 44}_{396\ \text{cells}} + \underbrace{8 \times 4}_{32\ \text{gaps}} + \underbrace{8}_{\text{--scrollbar-size}} = 496\text{px} \le 498\text{px}.$$

Резерв `2px`; горизонтальный скролл исключён (`overflow-x: hidden; overflow-y: auto`), девятая колонка не обрезается. Проверка сходится только при `padding: var(--space-3)`:
- `--space-5` (12px): `48 + 24 + 396 + 32 + 8 = 508` → обрезка 10px (дефект ревизии 1);
- `--space-4` (8px): `48 + 16 + 396 + 32 + 8 = 500` → обрезка 2px;
- `--space-3` (6px): `496` → запас 2px. ✔

Дополнительно: комментарий `tokens.css:312` («498 − 24 (padding) − 48 (rail) = 426 ≈ 9*44 + 8*4») арифметически неточен — при 12px padding контент даёт `426`, а трек требует `428`. Спецификация фиксирует `6px`, чтобы трек помещался; правка комментария в `tokens.css` — вне write-scope этой карточки.

### 4.2. Алгоритм позиционирования поповера (`Anchor & Collision Detection`)

Когда `#reaction-picker-popover` открывается от кнопки-триггера `anchorEl` (`[data-command="add-reaction"]` в `.msg-toolbar`, `.reaction-add-btn` в `.reactions-row` или строки `.row`), его координаты `(left, top)` вычисляются по следующей формуле:

1. Пусть `anchorRect = anchorEl.getBoundingClientRect()`, `pickerW = Math.min(498, window.innerWidth - 24)`, `pickerH = Math.min(440, window.innerHeight - 48)`, `margin = 12` (`var(--space-5)` = 12px, **не 8**), `titlebarH = 32` (`var(--layout-titlebar-h)`, `tokens.css:220`).
2. **По вертикали (предпочтение: под триггером, при нехватке места — над триггером)**:
   - Если `anchorRect.bottom + margin + pickerH <= window.innerHeight - margin`, то `top = anchorRect.bottom + margin`, `data-placement="bottom-end"`.
   - Иначе если `anchorRect.top - margin - pickerH >= titlebarH + margin`, то `top = anchorRect.top - margin - pickerH`, `data-placement="top-end"`.
   - Иначе зажимаем в пределах окна: `top = Math.max(titlebarH + margin, Math.min(anchorRect.bottom + margin, window.innerHeight - pickerH - margin))`.
3. **По горизонтали (выравнивание по правому краю триггера с зажимом в окно)**:
   - Базовая координата: `idealLeft = anchorRect.right - pickerW`.
   - Зажим: `left = Math.max(margin, Math.min(idealLeft, window.innerWidth - pickerW - margin))`.

### 4.3. Адаптивное поведение по брейкпоинтам окна

| Брейкпоинт viewport | Поведение `.emoji-card` / `#reaction-picker-popover` | Поведение `.msg-toolbar` и `.reactions-row` |
|---|---|---|
| **Desktop Wide ($\ge 1200\text{px}$)** | Фиксированный размер `498px × 440px` (`var(--emoji-picker-w) × var(--emoji-picker-h)`), ровно `9` колонок по `44px`. | Полный `.msg-toolbar` (15 пунктов). Пилюли `.reactions-row` переносятся через `flex-wrap: wrap` (`chat.css:185`). |
| **Compact Desktop ($720\text{px} \dots 1199\text{px}$)** | Фиксированный размер `498px × 440px`; collision detection прижимает поповер с отступом `var(--space-5)` (`12px`) от правого/левого края окна. | Полный `.msg-toolbar`. |
| **Narrow / Small Window ($< 540\text{px}$ по ширине или $< 500\text{px}$ по высоте)** | `width: min(var(--emoji-picker-w), calc(100vw - var(--space-8)))`; `height: min(var(--emoji-picker-h), calc(100vh - var(--layout-titlebar-h) - var(--space-8)))`. Сетка `#emoji-grid` переключается на `grid-template-columns: repeat(auto-fill, minmax(var(--emoji-cell), 1fr))`, размер ячейки не ниже `44×44px`. Фактическое число колонок рендер пишет в `grid.dataset.cols` (`getComputedStyle(grid).gridTemplateColumns`), и шаг `ArrowDown`/`ArrowUp` берётся оттуда; в DOM-шиме — фолбэк `EMOJI_COLS = 9`. | Все 15 пунктов сохраняются; при ширине строки $< 320\text{px}$ скрывать пункты **нельзя** — тест `gui-spike-features.test.mjs:1173` требует ровно 15 кнопок, а правило «нет мёртвых кнопок». Вместо скрытия панель переносится: `flex-wrap: wrap` и `max-width: 100%`. |

---

## 5. WAI-ARIA, клавиатурная навигация (`roving tabindex`) и возврат фокуса

### 5.1. Полная карта WAI-ARIA ролей и атрибутов

| Элемент | Роль (`role`) | Обязательные ARIA-атрибуты | Назначение |
|---|---|---|---|
| `#reaction-picker-popover`, стационарная `.emoji-card` | `role="dialog"` | `aria-modal="false"`, `aria-label="Выбор реакции"` (поповер) / `"Выбор эмодзи"` (стационарный, `index.html:139`), `aria-busy="true\|false"` | Немодальный диалог. `role="region"` на стационарной вкладке **не используется**: `index.html:139` уже содержит `role="dialog"` — замороженный атрибут, смена — решение PM. |
| `.search-field` | `role="search"` | `aria-label="Поиск эмодзи"` | Landmark поиска внутри пикера. |
| `#reaction-picker-search` / `#emoji-search` | (нативный `searchbox`) | `aria-label="Поиск эмодзи"`, `aria-controls="reaction-picker-grid"` (у стационарного — `"emoji-grid"`) | Поле фильтрации эмодзи по названию/глифу. |
| `#reaction-picker-tabs` / `#emoji-tabs` | `role="tablist"` | `aria-label="Категории эмодзи"`, `aria-orientation="vertical"` | Вертикальный список из 10 категорий. |
| `.emoji-tabs > .tab` | `role="tab"` | `id="<instance>-tab-<i>"`, `aria-selected="true\|false"`, `tabindex="0\|-1"`, `aria-controls="<instance>-grid"`, `aria-label="<Категория>"`, `title="<Категория>"` | Вкладка категории с `roving tabindex`. `aria-controls` обязателен: замороженный тест `gui-spike-features.test.mjs:386`. |
| `.emoji-grid-viewport` | `role="tabpanel"` | `aria-labelledby="<instance>-tab-<active>"`, `tabindex="-1"` | Панель активной вкладки (APG: `tablist` → `tabpanel`). Сам грид остаётся `listbox`, поэтому панель — обёртка. |
| `#reaction-picker-grid` / `#emoji-grid` | `role="listbox"` | `aria-label="Эмодзи"` (заморожено `DOM-CONTRACT.md:347`, `gui-spike-dom.test.mjs:496-498`), `data-cols="<n>"` | Одномерный контейнер ячеек (§2.4). `role="grid"`/`aria-colcount` не используются. |
| `.emoji-cell` | `role="option"` | `aria-selected="true\|false"`, `tabindex="0\|-1"`, `aria-label="<человекочитаемое имя>[, выбрана]"`, `title="<глиф>"`, `data-reacted="true\|false"` | Фокусируемая ячейка с `roving tabindex`. `aria-pressed` **запрещён**: у `role="option"` его нет, состояние выбора несёт `aria-selected`; «моя реакция» — только `data-reacted` + суффикс в `aria-label`. |
| `#reaction-picker-loading` / `#…-empty` | `role="status"` | `aria-live="polite"` | Озвучивание загрузки и пустого результата. |
| `#reaction-picker-banner` / `.reaction-error-status` | `role="alert"` | `aria-live="assertive"` — **только для статичного текста**; счётчик секунд — `aria-live="off"` | Однократное озвучивание ошибки `429` / `403` / сети (§3.2.4.1). |
| `.reaction-picker-banner-dismiss` | (нативный `button`) | `aria-label="Скрыть уведомление об ошибке"`, `data-action="dismiss-picker-error"` (только маркер) | Скрывает **только баннер**: поповер остаётся открыт, `data-active-screen` не меняется, кулдаун не обходится (§5.3 п.3, оракул N-12). |
| `.emoji-preview` | (нативный `footer`) | `aria-live="polite"` | Озвучивание имени активного эмодзи. |
| `.msg-toolbar` | `role="toolbar"` | `aria-label="Действия с сообщением <N>"`, `data-open`, `data-for-index` | Одна панель на чат (§2.2). |
| `[data-command="add-reaction"]`, `.reaction-add-btn` | (нативный `button`) | `aria-haspopup="dialog"`, `aria-expanded="true\|false"`, `aria-controls="reaction-picker-popover"` — **только при `aria-expanded="true"`**, `aria-label="Добавить реакцию"` | Два триггера одного поповера обязаны иметь **одинаковый** набор атрибутов. `aria-controls` живёт ровно столько, сколько смонтирован поповер: в закрытом состоянии IDREF не висит (§5.1, исключение 3). |
| `.reactions-row` | `role="group"` | `aria-label="Реакции"` | Группа пилюль реакций под сообщением. |
| `button.reaction` | (нативный `button`) | `aria-pressed="true\|false"`, `aria-busy="true\|false"` (при `data-pending`), `aria-label="Реакция <emoji>, <count>"` | Кнопка-переключатель конкретной реакции. |

> **Запрет ловушки доступности**: Ни один родительский контейнер фокусируемых кнопок (`.msg-toolbar`, `.reactions-row`, `#emoji-tabs`, `#emoji-grid`, `#reaction-picker-popover`) **никогда** не должен иметь `aria-hidden="true"`, пока он видим. Атрибут `aria-hidden="true"` разрешён только на декоративных листовых иконках (`.reaction-emoji`, `.emoji-preview-glyph`, `<svg class="icon">`).

**Три записанных исключения** `[A]`:
1. `aria-controls` вкладки указывает на `#…-grid[role="listbox"]`, а не на `.emoji-grid-viewport[role="tabpanel"]` — обоснование и замороженный тест в §2.3.
2. Кнопка закрытия поповера несёт `data-action="reaction-picker-close"`, а не `emoji-close`: второй action занят переходом на экран чата (`actions.js:905`) и заморожен тестом `gui-spike-features.test.mjs:853-864`. Обработчик навешивается при создании узла (§5.3), потому что `actions.js:925-928` биндит `[data-action]` только на старте.
3. `aria-controls="reaction-picker-popover"` на обоих триггерах существует **только** при `aria-expanded="true"`, то есть пока поповер смонтирован. APG описывает `aria-controls` как постоянный атрибут, но `#reaction-picker-popover` создаётся лениво и в закрытом состоянии его в документе нет (§1.2, §5.3); постоянный IDREF указывал бы в пустоту. Атрибут выставляется и снимается в одном месте — `openReactionPicker()` / `closeReactionPicker()`. Замороженный `aria-controls="emoji-grid"` у вкладок стационарного экрана (`gui-spike-features.test.mjs:386`) этим исключением не затрагивается: там узел существует всегда.

### 5.2. Матрица клавиатурной навигации и `roving tabindex` (`rust/gui-spike/ui/features/emoji.js`)

Внутри `#emoji-tabs` и `#emoji-grid` действует строгий `roving tabindex`: в каждый момент ровно одна вкладка и ровно одна ячейка (при `visible.length > 0`) имеют `tabindex="0"` и `aria-selected="true"`, остальные — `tabindex="-1"` и `aria-selected="false"`. Реализация — `updateTabState` (`emoji.js:352-359`) и `setActiveOption` (`:365-385`), ячейки перерисовываются на каждый `render()` (`:387-414`).

Состав фокусируемых узлов зависит от состояния, поэтому число стопов `Tab` не фиксировано:

| Состояние поповера | Стопы `Tab` по порядку DOM |
|---|---|
| `ready` / `empty`, баннер скрыт | 4: `#reaction-picker-search` → `.emoji-close-btn` → активная вкладка `.tab[tabindex="0"]` → активная ячейка `.emoji-cell[tabindex="0"]` (в `empty` ячеек нет — 3 стопа) |
| `error-429` / `error-403` / `error`, баннер видим | 5: `#reaction-picker-search` → `.emoji-close-btn` → **`.reaction-picker-banner-dismiss`** → активная вкладка → активная ячейка (в `error` без сетки — 4) |
| `loading` | 3: `#reaction-picker-search` → `.emoji-close-btn` → активная вкладка; сетки и `.emoji-preview` в tab-порядке нет, поле поиска остаётся фокусируемым (§3.2.1) |

`.reaction-picker-banner-dismiss` стоит в DOM после шапки и до `.emoji-body`, поэтому при видимом баннере он попадает между кнопкой закрытия и вкладками; при `hidden` на баннере кнопка из tab-порядка исчезает вместе с ним.

| Фокус находится в | Клавиша | Действие (`event.preventDefault()`) | Формула индекса `[F]` |
|---|---|---|---|
| **В любой точке пикера** | `Escape` | `event.preventDefault()`; `event.stopPropagation()` (чтобы не сбросить открытый канал/экран чата — правило `D3`); закрывает поповер и возвращает фокус на триггер. | — |
| **Поле `#…-search`** | `ArrowDown` | Переводит фокус на активную ячейку грида (`index = 0`). В текущем коде стрелка на поле не обрабатывается — обработчик только `input` (`emoji.js:491`); поведение — новое, карточка N-03. | `0` |
| **Рейл `#emoji-tabs`** | `ArrowDown` или `ArrowRight` | Переключает категорию на следующую и переводит на неё фокус. | `activeCategory + 1`, клампится в `activateTab` (`emoji.js:434-437`) |
| | `ArrowUp` или `ArrowLeft` | Переключает на предыдущую. | `activeCategory - 1`, клампится там же |
| | `Home` | Первая категория (`Часто используемые`). | `0` |
| | `End` | Последняя категория (`Прочее`, idx `9`). | `EMOJI_CATEGORIES.length - 1` |
| **Сетка `#emoji-grid`** (`cols = Number(dataset.cols) \|\| EMOJI_COLS = 9`) | `ArrowRight` | Фокус и превью на следующую ячейку (переход на следующую строку). | `activeOption + 1` (`:475`) |
| | `ArrowLeft` | На предыдущую. | `activeOption - 1` (`:476`) |
| | `ArrowDown` | Ровно на ряд вниз. | `activeOption + cols` (`:477`; в коде константа `EMOJI_COLS`) |
| | `ArrowUp` | Ровно на ряд вверх. | `activeOption - cols` (`:478`) |
| | `Home` | Первая ячейка. | `0` |
| | `End` | Последняя ячейка. | `visible.length - 1` |
| | `Enter` или `Space` | Вызывает выбор `selectOption(activeOption)` (`emoji.js:421-429`) → `onSelect(glyph, item)`; в поповере это `requestReaction(channelReactionKey(...), glyph, base, {...})` + закрытие с возвратом фокуса. Сейчас `onSelect` стационарного экрана вставляет глиф в композер (`app.js:1490-1493`). | — |
| | `Shift + Enter` | То же без закрытия поповера, `data-reacted` на ячейке переключается. Новое поведение, карточка N-03. | — |

> Клавиши `Enter`/`Space` в гриде уже реализованы (`emoji.js:481-485`), но вызывают `selectOption`, а не сетевой `toggle_reaction` — в поповере к ним добавляется вызов `requestReaction` (N-03).

### 5.3. Протокол открытия, закрытия по `Escape` и возврата фокуса на триггер сообщения

1. **При открытии пикера (`openReactionPicker(triggerEl, message)`)** `[A]` — реализация отсутствует:
   - Если `#reaction-picker-popover` уже в документе — повторное открытие переиспользует его, второй экземпляр не создаётся.
   - Сохраняется ссылка `lastTriggerEl = triggerEl || document.activeElement` и `lastMessageId = message.id`.
   - **Lazy mount**: узел поповера создаётся здесь (`document.createElement`, разметка §2.3 на префиксных id) и монтируется в `document.body`; до этого момента `document.querySelector("#reaction-picker-popover") === null`. На кнопку `[data-action="reaction-picker-close"]` навешивается обработчик закрытия: `actions.js:925-928` биндит `[data-action]` только при инициализации и лениво созданный узел не увидит.
   - На `.reaction-picker-banner-dismiss` навешивается **прямой** обработчик закрытия баннера (п.3). `data-action="dismiss-picker-error"` остаётся только маркером — тот же приём, что у лениво созданной плитки `.guild-add` (`app.js:555-566`: `data-action` + прямой слушатель). В карту `handlers` (`actions.js:828-921`) значение **не** добавляется: статического узла с таким action нет, и запись там была бы мёртвой.
   - На обоих триггерах (`[data-command="add-reaction"]`, `.reaction-add-btn`) вместе с `aria-expanded="true"` выставляется `aria-controls="reaction-picker-popover"`; в закрытом состоянии атрибута нет (§5.1 исключение 3).
   - Поповер сразу видим (без `hidden`), позиционируется относительно `triggerEl` (§4.2), и фокус переводится в поле `#reaction-picker-search` (либо на первую ячейку `.emoji-cell[tabindex="0"]`, если передана опция `focusGrid: true`).
2. **При закрытии пикера (`closeReactionPicker({ restoreFocus = true })`)** `[A]`:
   - Вызывается по `Escape` (с обязательным `event.stopPropagation()`), клику по `[data-action="reaction-picker-close"]`, `pointerdown` вне `#reaction-picker-popover` или после одиночного выбора эмодзи.
   - На кнопке-триггере выставляется `aria-expanded="false"` и снимается `aria-controls` (узла больше нет, §5.1 исключение 3).
   - **Lazy destroy**: узел `#reaction-picker-popover` удаляется из документа (`remove()`), слушатели (`keydown`, `pointerdown`, `input`, клик по `reaction-picker-close`, клик по `.reaction-picker-banner-dismiss`) снимаются вместе с ним. Скрытие корня через `hidden = true` или `display: none` запрещено: скрытый узел сохраняет второй набор `.tab`/`.emoji-cell` и второй `.emoji-card` (§2.3). После закрытия `document.querySelector("#reaction-picker-popover") === null`, `document.querySelectorAll(".tab").length` возвращается к 10 (стационарный экран).
   - Если `restoreFocus === true`, фокус возвращается по цепочке:
     1. `lastTriggerEl`, если `document.contains(lastTriggerEl)` и элемент видим → `lastTriggerEl.focus()`.
     2. Иначе строка `.row[data-message-id="${lastMessageId}"]` → `rowEl.focus()` либо её кнопка `[data-command="add-reaction"]`.
     3. Иначе — явно переданный `openerElement` (`[data-screen-tab="emoji"]` / `#composer-input`).
   - В текущем коде такой цепочки нет: `emoji.js:365-385` (`setActiveOption`) управляет только превью и фокусом ячеек; `Escape` в пикере не обрабатывается. Поэтому возврат фокуса — red-first требование карточки N-03, а не `[F]`.

3. **Закрытие баннера ошибки (`.reaction-picker-banner-dismiss`)** `[A]` — реализация отсутствует, red-first оракул N-12 (§8.2):
   - Обработчик навешивается в `openReactionPicker()` (п.1) и делает ровно три вещи: `banner.hidden = true`; восстанавливает `data-state`; при необходимости переводит фокус. Поповер **не** закрывается, `aria-expanded` у триггера остаётся `"true"`, `aria-controls` остаётся, `data-active-screen` не меняется.
   - **`data-state`**: восстанавливается состояние, предшествовавшее ошибке (`lastNonErrorState`, по умолчанию `ready`). Для типового пути `ready → error-429` после dismiss снова `ready`; если к моменту ошибки сетка была пуста (`empty`), возвращается `empty`, чтобы не нарушать определение `ready` из §3.2.2 (`visible.length > 0`).
   - **Кулдаун не обходится**: `rateLimitUntilMs`, `aria-disabled` и таймер `.reaction-picker-banner-timer` продолжают жить. Скрытый баннер перестаёт озвучиваться (он `hidden`), счётчик тикает молча (`aria-live="off"`), по истечении кулдауна кнопки разблокируются и один раз объявляется `Можно повторить.` (§3.2.4). Для `forbidden` dismiss тоже не снимает `aria-disabled`: это состояние прав, а не таймера (§3.2.5).
   - **Фокус**: кнопка dismiss уходит из документа вместе с баннером, поэтому если `document.activeElement` был внутри `#reaction-picker-banner`, фокус явно переводится на `#reaction-picker-search`; иначе фокус не трогается.
   - **Сеть**: dismiss не вызывает `requestReaction`/`invoke` и не меняет `pending`; счётчик IPC-вызовов остаётся прежним.
   - **Ошибка не «исчезает»**: dismiss скрывает баннер только в текущем инциденте. Пока причина активна (`aria-disabled` на кнопках и ячейках по §3.2.4–§3.2.5), повторное открытие поповера снова показывает баннер с тем же `data-error-kind` и актуальным текстом — иначе `forbidden`-состояние оставило бы пользователя с недоступными кнопками без объяснения.
   - `Escape` баннер не закрывает — он закрывает весь поповер (п.2). Dismiss — единственный способ скрыть ошибку, не закрывая пикер.

---

## 6. Связка с Tauri IPC `toggle_reaction`, REST `PUT`/`DELETE` и Gateway

### 6.1. Полная цепочка вызовов: от клика в UI до HTTP-запроса Discord API v9

```
UI (reactions.js / emoji.js / message-toolbar.js / chat.js)
  │
  │ 1. Клик по .reaction, .msg-toolbar-btn[data-command="react"] или .emoji-cell
  ▼
requestReaction(key, emoji, base, { channelId, messageId, element, onSettled })
  │   key = channelReactionKey(channelId, messageId) | index bench-строки
  │
  ├─► Дедупликация: requestKey = `${String(key)}:${emoji}`  (reactions.js:166)
  │   Если pending.has(requestKey) → выход без сети (Promise<void>)
  │
  ├─► Оптимистичное переключение (reactions.js:168-175):
  │   const before = reactionsFor(key, base).find(r => r.emoji === emoji)
  │   const active = toggleReaction(key, emoji, base, { allowUnknown: true }) → boolean
  │   • me === true  → active = false, count = max(0, count - 1)
  │   • me === false → active = true,  count = count + 1, me = true
  │   element.setAttribute("data-pending", "true")
  │   вызывающий сам перерисовывает строку (chat.js:2543 paintPill / message-toolbar.js:224 renderRow)
  │
  ├─► Вызов Tauri IPC:
  │   invoke("toggle_reaction", {
  │     channelId: options.channelId ?? "news",
  │     messageId: options.messageId ?? String(key),
  │     emoji: String(emoji),
  │     active: Boolean(active)
  │   })
  ▼
Rust Tauri Command (`rust/gui-spike/src/main.rs:392-409`)
  #[tauri::command]
  async fn toggle_reaction(
      backend: tauri::State<'_, Arc<AuthBackend>>,
      channel_id: String,
      message_id: String,
      emoji: String,
      active: bool,
  ) -> Result<(), ReactionError>
  │
  ├─► Чтение токена сессии из AuthBackend (OS Keyring) через token_or
  │   • ошибка чтения keyring → Err(ReactionError::Storage)
  │   • токена нет            → Err(ReactionError::Unauthorized)
  ▼
Rust HTTP Client (`rust/gui-spike/src/reactions.rs:118-142,157-167`)
  DiscordReactionApi::new(&token) → DiscordReactionApi::toggle(&channel_id, &message_id, &emoji, active)
  │
  ├─► Валидация аргументов (`validate`, reactions.rs:169-178):
  │   1. is_numeric_id(channel_id): непустая строка только из ASCII-цифр ('0'..'9')
  │   2. is_numeric_id(message_id): то же
  │   3. emoji.is_empty() == false и emoji.chars().count() <= 256 (MAX_EMOJI_CHARS)
  │      ВНИМАНИЕ: trim() в Rust НЕ выполняется — строка из пробелов проходит
  │      validate и уходит в сеть как %20%20%20. UI обязан сам передавать
  │      emoji.trim(); правка Rust — отдельная карточка (N-04).
  │   4. percent_encode_emoji(emoji): все байты UTF-8, кроме RFC 3986
  │      unreserved (A-Z, a-z, 0-9, '-', '.', '_', '~'), кодируются как %XX
  │
  ├─► Формирование маршрута:
  │   {base_url}/channels/{channel_id}/messages/{message_id}/reactions/{encoded_emoji}/@me
  │   base_url = API_V9_BASE = "https://discord.com/api/v9" (discord-core/src/lib.rs:76)
  │
  ├─► Выбор HTTP-метода по флагу `active`:
  │   • active == true  → Method::PUT, body b"{}" (2 байта), Content-Type: application/json
  │   • active == false → Method::DELETE, тело пустое
  │   Оба метода идемпотентны → ApiClient ретраит 429 и 5xx до max_attempts = 3
  ▼
Обработка HTTP-ответа (`decode_response`, reactions.rs:200-202; политика — rest_failure.rs:65-76):
  • 2xx                    → Ok(())
  • 401                    → Err(ReactionError::Unauthorized)   (unit, без текста)
  • 403                    → Err(ReactionError::Forbidden)
  • 404                    → Err(ReactionError::NotFound)
  • 429                    → Err(ReactionError::RateLimited { retry_after_ms })
  • прочие 4xx             → Err(ReactionError::Client { status })
  • 5xx                    → Err(ReactionError::Server { status })
```

### 6.2. Правила кодирования `{emoji}` в URL маршрута `/reactions/{emoji}/@me` `[F]`

Функция `percent_encode_emoji` (`rust/gui-spike/src/reactions.rs:185-198`, проверена в `rust/gui-spike/tests/reactions_contract.rs:86-91`) кодирует параметр `{emoji}` строго по байтам UTF-8:

| Тип эмодзи | Входное значение `emoji` в JS / IPC | Закодированный сегмент `{emoji}` в HTTP URL | Пример полного пути REST |
|---|---|---|---|
| **Стандартный Unicode (одиночный глиф)** | `"👍"` | `%F0%9F%91%8D` | `/api/v9/channels/110/messages/120/reactions/%F0%9F%91%8D/@me` |
| **Стандартный Unicode (`👋`)** | `"👋"` | `%F0%9F%91%8B` | `/api/v9/channels/42/messages/100500/reactions/%F0%9F%91%8B/@me` |
| **Unicode с модификатором (`❤️`, U+2764 U+FE0F)** | `"❤️"` | `%E2%9D%A4%EF%B8%8F` | `/api/v9/channels/110/messages/120/reactions/%E2%9D%A4%EF%B8%8F/@me` |
| **Кастомный эмодзи сервера (`name:id`)** | `"party:123456789012345678"` | `party%3A123456789012345678` | `/api/v9/channels/110/messages/120/reactions/party%3A123456789012345678/@me` |
| **Невалидный / слишком длинный ($>256$ chars)** | `"a".repeat(257)` или `""` | Отклоняется до сети | `Err(ReactionError::InvalidArg)` (`0` HTTP-запросов) |

### 6.3. Синхронизация с Gateway-событиями `MESSAGE_REACTION_ADD` и `MESSAGE_REACTION_REMOVE` `[F]`

В `rust/gui-spike/src/net.rs:113-120` события `MESSAGE_REACTION_ADD` и `MESSAGE_REACTION_REMOVE` включены в белый список `FORWARDED_DISPATCHES` и доставляются во фронтенд через Tauri-событие `gateway://dispatch` (`net.rs:249-253`):

1. **Проекция события (`projectReaction` в `rust/gui-spike/ui/features/dispatch-model.js:776-824`)**:
   - Из `payload.d` извлекаются `channelId = d.channel_id`, `messageId = d.message_id`, `userId = d.user_id` и `emoji` (`d.emoji.name || d.emoji.id`, `emojiOf`).
   - `store.reactions` — `Map<"channel:message:emoji", { users: Set<userId>, anon: number }>`: ADD кладёт `userId` в `Set`, REMOVE удаляет. Повторный ADD того же `userId` считается дубликатом и не меняет счётчик (`counters.duplicate`), REMOVE без ADD — no-op (`counters.ignored`).
   - **Счётчик абсолютный, не дельта**: возвращается `{ type: "reaction", channelId, messageId, emoji, count: users.size + anon, me, removed }` (`:815-823`), где `me = userId === store.selfUserId`. Объекта `{userId, delta}` в модели нет.
2. **Применение к открытому каналу (`applyReaction` в `rust/gui-spike/ui/features/chat.js:3676-3700`)**:
   - Базовая линия «живого» счётчика — `liveReactionCounts: Map<"key:emoji", number>` (`:3669`).
   - Видимый счётчик: `displayCount = max(0, currentCount - previousLive + liveCount)` (`:3693-3696`), где `currentCount` — то, что видно сейчас (база `messages_page` + `overrides`).
   - `me`: `nextMe = change.me ? !change.removed : currentMe` (`:3699`) — ADD себя включает отметку, REMOVE себя снимает, чужие события отметку не трогают.
   - Изменение записывается в `overrides` через `adjustReactionOverride(key, emoji, displayCount - currentCount, nextMe, item.reactions)` (`:3700`, реализация `reactions.js:225-251`): патч несёт **абсолютный** `count`, поэтому повторный `refreshRow` (скролл/ack) не откатывает live-состояние к старой базе.
   - **Защиты от эха собственной оптимистичной реакции в коде нет** — это известный дефект, а не свойство. Guard `chat.js:3694` (`liveCount === previousLive && !change.removed`) отсекает только повторные идентичные события. Своё эхо-ADD после оптимистичного `+1` даёт `liveCount = previousLive + 1`, поэтому строка `displayCount = currentCount - previousLive + liveCount` (`:3696`) удваивает счётчик: база `0` → optimistic `1` → эхо `count=1` → `1 - 0 + 1 = 2`. `currentCount` уже включает optimistic override (`reactionsFor` на `:3688`). Спека **не** описывает несуществующую защиту: до исправления UI не должен полагаться на дедупликацию эха, а правка вынесена в карточку N-11 (§8.4) с red-first тестом «одно своё событие не меняет видимый счётчик повторно».
   - При `displayCount <= 0` пилюля удаляется, а пустая `.reactions-row` убирается из DOM (`chat.js:3710-3714`).

---

## 7. Нормативный CSS-контракт (100% `tokens.css`) и матрица контраста WCAG 2.2 AA

### 7.1. Полный CSS-код компонентов (`0` hex/rgb/hsl, `0` `!important`)

#### 7.1.0. Файл, порядок каскада и конфликты с текущими стилями

Блок ниже — **единственный нормативный источник** для затронутых селекторов. Он кладётся в конец существующих файлов: разделы 1–2 (пилюли и тулбар) — в `rust/gui-spike/ui/components/chat.css`, разделы 3–4 (карточка, рейл, сетка, превью, адаптив) — в `rust/gui-spike/ui/components/emoji.css`. Порядок каскада: `tokens.css` → `primitives.css` → `components/*.css`; внутри файла новые правила идут **ниже** старых и перекрывают их. Отдельный файл с `@import` не заводится: гейт токенизации проверяет оба существующих файла (§8.3).

Таблица конфликтов (что именно перекрывается и чем):

| Файл:строка | Селектор | Сейчас | Норматив (§7.1) |
|---|---|---|---|
| `emoji.css:26` | `.emoji-card` | `border-radius: var(--radius-lg)` (16px) | `var(--radius-md)` (12px) |
| `emoji.css:19-20` | `.emoji-card` | `display: flex; flex-direction: column` | `display: grid` + `grid-template-areas` |
| `emoji.css:36` | `.emoji-head` | `padding: var(--space-4) var(--space-5)` (8/12) | `var(--space-5) var(--space-6)` (12/16) |
| `emoji.css:34` | `.emoji-head` | `gap: var(--space-2)` (4px) | `var(--space-5)` (12px) |
| `emoji.css:50` | `.emoji-tabs` | `gap: var(--space-1)` (2px) | `var(--space-2)` (4px) |
| `emoji.css:52` | `.emoji-tabs` | `padding: var(--space-2) 0` (4/0) | `var(--space-4)` (8px) |
| `emoji.css:66-71` | `.tab` | `width: 100%; height: var(--emoji-section-h)`, `radius-xs` | `--btn-height × --btn-height` (32×32), `radius-sm` |
| `emoji.css:105` | `.emoji-grid` | `padding: var(--space-4)` (8px) | `var(--space-3)` (6px) — бюджет 496 |
| `emoji.css:126` | `.emoji-cell` | `border-radius: var(--radius-xs)` (4px) | `var(--radius-sm)` (8px) |
| `emoji.css:128` | `.emoji-cell` | `font-size: var(--icon-size-lg)` (24px) | `var(--font-size-title)` (20px) |
| `emoji.css:136` | `.emoji-cell[aria-selected]` | `background: var(--state-selected)` | `var(--state-hover)` |
| `emoji.css:150` | `.emoji-preview` | `height: var(--space-11)` | `min-height: var(--space-11)` |
| `emoji.css:112-117` | `.emoji-row` | `display: grid` | **удаляется**: плоский listbox, строк нет (§2.4) |
| `chat.css:186` | `.reactions-row` | `gap: var(--space-2)` (4px) | `var(--space-3)` (6px) |
| `chat.css:187` | `.reactions-row` | `padding-top: var(--space-1)` | `margin-top: var(--space-2)` |
| `chat.css:192` | `.reaction` | `gap: var(--space-2)` | `var(--space-3)` |
| `chat.css:194` | `.reaction` | `height: var(--layout-group-title-h)` | `min-height: ...` |
| `chat.css:195` | `.reaction` | `padding: 0 var(--space-4)` | `var(--space-1) var(--space-4)` |
| `chat.css:196` | `.reaction` | `border: 1px solid var(--border-subtle)` | `var(--menu-border) solid var(--accent)` |
| `chat.css:359` | `.msg-toolbar` | `padding: var(--menu-pad)` (8px) | `var(--menu-pad)` — **без изменений** |
| `chat.css:361` | `.msg-toolbar` | `background: var(--surface-4, var(--surface-2))` | `var(--surface-4)` (без fallback-маскировки) |
| `chat.css:363` | `.msg-toolbar` | `z-index: var(--z-popover)` | `var(--z-popover)` — **без изменений** |
| `chat.css:371-372` | `.msg-toolbar-btn` | `width: var(--row-menu-item)` (32×32) | `min-width: var(--row-menu-item)`, `height: var(--row-menu-item)`, `width: auto` — текст подписи («Изменить сообщение») не обрезается |

Все правила ниже используют **исключительно** CSS-переменные из `rust/gui-spike/ui/tokens.css`:

```css
/* ==========================================================================
   1. РЯД ПИЛЮЛЬ РЕАКЦИЙ (.reactions-row, .reaction, .reaction-add-btn)
   ========================================================================== */

.reactions-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
  margin-top: var(--space-2);
}

.reaction {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  min-height: var(--layout-group-title-h);
  padding: var(--space-1) var(--space-4);
  border-radius: var(--radius-sm);
  border: var(--menu-border) solid var(--accent);
  background: var(--state-hover);
  color: var(--text);
  font-family: var(--font-sans);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  user-select: none;
  transition:
    background-color var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out),
    color var(--motion-fast) var(--ease-out);
}

.reaction:hover:not(:disabled):not([aria-disabled="true"]) {
  background: var(--surface-3);
  border-color: var(--accent);
  color: var(--text-strong);
}

/* Счётчик не наследует цвет пилюли: у `.reaction-count` собственный color.
   Без этого правила обещание §3.1 п.2 (`--text-strong` на hover) не выполняется. */
.reaction:hover:not(:disabled):not([aria-disabled="true"]) .reaction-count {
  color: var(--text-strong);
}

.reaction:active:not(:disabled):not([aria-disabled="true"]) {
  background: var(--state-active);
}

.reaction[data-me="true"],
.reaction[aria-pressed="true"] {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--text-strong);
}

.reaction[data-me="true"] .reaction-count,
.reaction[aria-pressed="true"] .reaction-count {
  color: var(--text-accent);
  font-weight: var(--font-weight-semibold);
}

/* selected:hover: ни заливка, ни граница не меняются — только счётчик.
   --accent-hover (2.72:1) и --state-selected-strong (≈1.56:1) на границе запрещены. */
.reaction[data-me="true"]:hover:not(:disabled):not([aria-disabled="true"]),
.reaction[aria-pressed="true"]:hover:not(:disabled):not([aria-disabled="true"]) {
  background: var(--accent-soft);
  border-color: var(--accent);
}

.reaction[data-me="true"]:hover:not(:disabled):not([aria-disabled="true"]) .reaction-count,
.reaction[aria-pressed="true"]:hover:not(:disabled):not([aria-disabled="true"]) .reaction-count {
  color: var(--text-strong);
}

/* pending: без opacity — иначе текст и граница падают ниже AA */
.reaction[data-pending="true"],
.reaction[aria-busy="true"] {
  cursor: progress;
}

.reaction[data-pending="true"] .reaction-count,
.reaction[aria-busy="true"] .reaction-count {
  color: var(--text-muted);
}

.reaction:disabled,
.reaction[aria-disabled="true"],
.reaction-add-btn[aria-disabled="true"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

.reaction:focus-visible,
.reaction-add-btn:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.reaction-emoji {
  font-size: var(--font-size-ui);
  line-height: var(--line-height-small);
}

.reaction-emoji-custom {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-muted);
}

.reaction-count {
  font-variant-numeric: tabular-nums;
  color: var(--text);
}

.reaction-add-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: var(--layout-group-title-h);
  min-height: var(--layout-group-title-h);
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-sm);
  border: var(--menu-border) solid var(--accent);
  background: transparent;
  color: var(--text-muted);
  font-family: var(--font-sans);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-small);
  cursor: pointer;
  transition:
    background-color var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out),
    color var(--motion-fast) var(--ease-out);
}

.reaction-add-btn:hover:not(:disabled):not([aria-disabled="true"]),
.reaction-add-btn[aria-expanded="true"] {
  background: var(--state-hover);
  border-color: var(--accent);
  color: var(--text-strong);
}

/* Inline-статус ошибки реакции под сообщением */
.reaction-error-status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-4);
  margin-top: var(--space-2);
  padding: var(--space-2) var(--space-5);
  border-radius: var(--radius-xs);
  background: var(--surface-2);
  border: var(--menu-border) solid var(--danger);
  color: var(--danger-text);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.reaction-error-status[data-error-kind="rateLimited"] {
  border-color: var(--warning);
  color: var(--text-strong);
}

/* Счётчик секунд: не живой регион (aria-live="off" в разметке) */
.reaction-error-timer,
.reaction-picker-banner-timer {
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ==========================================================================
   2. ПЛАВАЮЩАЯ ПАНЕЛЬ ДЕЙСТВИЙ СООБЩЕНИЯ (.msg-toolbar)
   Одна панель на чат: display:none по умолчанию, [data-open="true"] показывает.
   top/right выставляет JS inline (message-toolbar.js:68-70,158-171).
   ========================================================================== */

.msg-toolbar {
  position: absolute;
  display: none;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-1);
  padding: var(--menu-pad);
  max-width: 100%;
  border-radius: var(--menu-radius);
  background: var(--surface-4);
  border: var(--menu-border) solid var(--border-strong);
  box-shadow: var(--shadow-popover);
  z-index: var(--z-popover);
}

.msg-toolbar[data-open="true"] {
  display: flex;
}

.msg-toolbar-btn {
  min-width: var(--row-menu-item);
  width: auto;
  height: var(--row-menu-item);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
  padding: var(--space-0) var(--space-1);
  border-radius: var(--radius-xs);
  border: var(--menu-border) solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  cursor: pointer;
  transition:
    background-color var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out),
    color var(--motion-fast) var(--ease-out);
}

.msg-toolbar-btn:hover:not(:disabled):not([aria-disabled="true"]) {
  background: var(--state-hover);
  color: var(--text-strong);
}

.msg-toolbar-btn[data-me="true"],
.msg-toolbar-btn[aria-pressed="true"],
.msg-toolbar-btn[aria-expanded="true"] {
  background: var(--accent-soft);
  border-color: var(--accent);
  color: var(--text-accent);
}

.msg-toolbar-btn:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.msg-toolbar-btn[aria-disabled="true"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

/* pending на кнопке тулбара: без opacity — прозрачность уронила бы иконку ниже AA */
.msg-toolbar-btn[data-pending="true"] {
  cursor: progress;
  color: var(--text-muted);
}

/* pending + выбранная: --text-muted на композите --accent-soft поверх --surface-4
   даёт 4.04:1 — ниже AA. Цвет состояния pressed сохраняется: 5.74:1.
   Специфичность (0,3,0) перебивает правило pending выше. */
.msg-toolbar-btn[data-pending="true"][aria-pressed="true"],
.msg-toolbar-btn[data-pending="true"][data-me="true"] {
  color: var(--text-accent);
}

/* ==========================================================================
   3. ПОПОВЕР ВЫБОРА РЕАКЦИЙ (.reaction-picker-popover / .emoji-card)
   ========================================================================== */

.emoji-card {
  width: var(--emoji-picker-w);
  height: var(--emoji-picker-h);
  max-width: calc(100vw - var(--space-8));
  max-height: calc(100vh - var(--layout-titlebar-h) - var(--space-8));
  background: var(--surface-2);
  border: var(--menu-border) solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-popover);
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  /* Строка баннера существует всегда, но при [hidden] её высота = 0. */
  grid-template-rows: auto auto minmax(0, 1fr) auto;
  grid-template-areas:
    "head"
    "banner"
    "body"
    "preview";
  overflow: hidden;
  color: var(--text);
}

.emoji-card > .emoji-head { grid-area: head; }
.emoji-card > .reaction-picker-banner { grid-area: banner; }
.emoji-card > .emoji-body { grid-area: body; }
.emoji-card > .emoji-preview { grid-area: preview; }

.reaction-picker-popover {
  position: fixed;
  z-index: var(--z-popover);
}

.emoji-head {
  display: flex;
  align-items: center;
  gap: var(--space-5);
  padding: var(--space-5) var(--space-6);
  background: var(--surface-1);
  border-bottom: var(--menu-border) solid var(--divider);
}

/* Контейнер поиска: геометрию и цвета даёт существующий .search-input
   (primitives.css:309-329), здесь только раскладка строки. */
.emoji-head .search-field {
  flex: 1;
  min-height: var(--search-height);
  min-width: var(--space-0);
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.emoji-head .search-field:focus-within {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
  border-radius: var(--search-radius);
}

/* Баннер ошибки 429 / 403 / сети внутри пикера */
.reaction-picker-banner {
  display: flex;
  align-items: center;
  gap: var(--space-5);
  padding: var(--space-4) var(--space-6);
  background: var(--surface-3);
  border-bottom: var(--menu-border) solid var(--danger);
  color: var(--danger-text);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.reaction-picker-banner[data-error-kind="rateLimited"] {
  border-bottom-color: var(--warning);
  color: var(--text-strong);
}

.reaction-picker-banner-icon {
  width: var(--icon-size-sm);
  height: var(--icon-size-sm);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-round);
  background: var(--danger-fill);
  color: var(--text-on-danger);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-bold);
  flex-shrink: 0;
}

.reaction-picker-banner[data-error-kind="rateLimited"] .reaction-picker-banner-icon {
  background: var(--warning);
  color: var(--text-on-warning);
}

.reaction-picker-banner-text {
  flex: 1;
  min-width: var(--space-0);
}

.reaction-picker-banner-dismiss {
  width: var(--icon-size);
  height: var(--icon-size);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: var(--space-0);
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}

.reaction-picker-banner-dismiss:hover {
  background: var(--state-hover);
  color: var(--text-strong);
}

/* Тело пикера: рейл категорий + область сетки */
.emoji-body {
  display: grid;
  grid-template-columns: var(--emoji-rail-w) 1fr;
  min-height: var(--space-0);
  overflow: hidden;
}

.emoji-tabs {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-4);
  background: var(--surface-1);
  border-right: var(--menu-border) solid var(--divider);
  overflow-y: auto;
}

.emoji-tabs .tab {
  width: var(--btn-height);
  height: var(--btn-height);
  padding: var(--space-0);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-sm);
  border: var(--menu-border) solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--font-size-body);
  cursor: pointer;
  flex-shrink: 0;
  transition:
    background-color var(--motion-fast) var(--ease-out),
    color var(--motion-fast) var(--ease-out);
}

.emoji-tabs .tab:hover {
  background: var(--state-hover);
  color: var(--text);
}

.emoji-tabs .tab[aria-selected="true"] {
  background: var(--state-selected);
  color: var(--text-strong);
}

.emoji-tabs .tab:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.emoji-grid-viewport {
  display: flex;
  flex-direction: column;
  min-height: var(--space-0);
  min-width: var(--space-0);
  overflow: hidden;
}

.emoji-grid-viewport:focus {
  outline: none;
}

.emoji-section-header {
  height: var(--emoji-section-h);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-0) var(--space-6);
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--font-weight-bold);
  letter-spacing: var(--letter-spacing-caps);
  text-transform: uppercase;
  flex-shrink: 0;
}

.emoji-grid {
  flex: 1;
  min-height: var(--space-0);
  display: grid;
  grid-template-columns: repeat(var(--emoji-cols), var(--emoji-cell));
  grid-auto-rows: var(--emoji-cell);
  gap: var(--emoji-gap);
  padding: var(--space-3);
  overflow-y: auto;
  overflow-x: hidden;
  align-content: start;
  scrollbar-color: var(--scrollbar-thumb) var(--scrollbar-track);
}

/* В состояниях loading/empty сетка не участвует в раскладке: иначе она
   делит высоту 50/50 с плашкой состояния (оба flex:1). */
.emoji-card[data-state="loading"] .emoji-grid,
.emoji-card[data-state="empty"] .emoji-grid {
  display: none;
}

.emoji-cell {
  width: var(--emoji-cell);
  height: var(--emoji-cell);
  border-radius: var(--radius-sm);
  border: var(--menu-border) solid transparent;
  background: transparent;
  color: var(--text-strong);
  font-size: var(--font-size-title);
  line-height: var(--line-height-title);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  user-select: none;
  transition:
    background-color var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out);
}

.emoji-cell:hover:not(:disabled):not([aria-disabled="true"]),
.emoji-cell[aria-selected="true"] {
  background: var(--state-hover);
}

/* «Моя» реакция: заливка --surface-3 + рамка --accent (3.18:1 к заливке,
   3.35:1 к карточке). aria-pressed у role="option" не используется. */
.emoji-cell[data-reacted="true"] {
  background: var(--surface-3);
  border-color: var(--accent);
}

.emoji-cell[data-reacted="true"]:hover:not(:disabled):not([aria-disabled="true"]) {
  background: var(--surface-3);
  box-shadow: var(--shadow-elevation-low);
}

.emoji-cell:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: calc(-1 * var(--focus-ring-width));
}

/* Пустое состояние поиска и состояние загрузки */
.emoji-empty-state,
.emoji-loading-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  padding: var(--space-9);
  text-align: center;
  color: var(--text-muted);
}

.emoji-empty-glyph {
  font-size: var(--font-size-display);
  line-height: var(--line-height-display);
}

.emoji-empty-title {
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  font-weight: var(--font-weight-semibold);
}

.emoji-empty-hint,
.emoji-loading-label {
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.emoji-skeleton-grid {
  display: grid;
  grid-template-columns: repeat(var(--emoji-cols), var(--emoji-cell));
  gap: var(--emoji-gap);
}

.emoji-skeleton-cell {
  width: var(--emoji-cell);
  height: var(--emoji-cell);
  border-radius: var(--radius-sm);
  background: var(--surface-3);
}

/* Нижний футер предпросмотра */
.emoji-preview {
  min-height: var(--space-11);
  padding: var(--space-5) var(--space-6);
  background: var(--surface-1);
  border-top: var(--menu-border) solid var(--divider);
  display: flex;
  align-items: center;
  gap: var(--space-5);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.emoji-preview-glyph {
  font-size: var(--font-size-title);
  line-height: var(--line-height-title);
}

.emoji-preview-name {
  font-family: var(--font-mono);
  font-weight: var(--font-weight-semibold);
  color: var(--text-strong);
}

.emoji-preview-category {
  margin-left: auto;
  font-size: var(--font-size-small);
  color: var(--text-muted);
}

/* ==========================================================================
   4. АДАПТИВНОЕ ПЕРЕСТРОЕНИЕ НА УЗКИХ ОКНАХ (< 540px)
   ========================================================================== */

@media (max-width: 540px) {
  .emoji-card {
    width: min(var(--emoji-picker-w), calc(100vw - var(--space-8)));
    max-height: min(var(--emoji-picker-h), calc(100vh - var(--layout-titlebar-h) - var(--space-8)));
  }

  .emoji-grid {
    grid-template-columns: repeat(auto-fill, minmax(var(--emoji-cell), 1fr));
  }
}

/* 5. JS-КОНТРАКТ ОБРАБОТЧИКОВ (не CSS, но нормативно)
   aria-disabled НЕ блокирует клик сам по себе. Каждый обработчик пилюли,
   .reaction-add-btn и ячейки начинается с проверки:
     if (el.getAttribute("aria-disabled") === "true") return;
   pointer-events: none для этого не используется — он отнимает фокус и title. */
```

---

### 7.2. Матрица верификации контраста WCAG 2.2 AA ($\ge 4.5:1$ текст, $\ge 3.0:1$ non-text)

Пересчитано по фактическим значениям `rust/gui-spike/ui/tokens.css` (dark; файл объявляет `color-scheme: dark`, `tokens.css:53`, и **не содержит light-блока**). Метод: WCAG 2.2 relative luminance, альфа-слои композитятся поверх базовой поверхности (`--state-hover` 6%, `--accent-soft` 16%, `--border-*` 12/20/44%, `--divider` 6%). Light-колонки нет: `THEME-SPEC.md §2` — design-target, в коде не реализован, сертифицировать его нечем.

#### 7.2.1. Текст и иконки (SC 1.4.3, $\ge 4.5:1$)

| Элемент | Токен текста / иконки | Поверхность (композит) | Факт | Норма | Итог |
|---|---|---|---|---|---|
| Счётчик обычной пилюли `.reaction-count` | `--text` `#efeff1` | `--bg-chat #1a1a1e` + `--state-hover` → `#28282c` | **12.79 : 1** | ≥ 4.5 | PASS |
| Счётчик обычной пилюли, `:hover` | `--text-strong` `#fbfbfb` | `--surface-3 #28282d` | **14.18 : 1** | ≥ 4.5 | PASS |
| Счётчик выбранной пилюли `.reaction[data-me="true"] .reaction-count` | `--text-accent` `#a5b0ff` | `--bg-chat` + `--accent-soft` → `#242640` | **7.19 : 1** | ≥ 4.5 | PASS |
| Счётчик выбранной, `:hover` | `--text-strong` `#fbfbfb` | `#242640` | **14.23 : 1** | ≥ 4.5 | PASS |
| Счётчик в `pending` | `--text-muted` `#96979e` | `#28282c` | **5.05 : 1** | ≥ 4.5 | PASS |
| `.emoji-preview-name` | `--text-strong` `#fbfbfb` | `--surface-1 #202024` | **15.69 : 1** | ≥ 4.5 | PASS |
| `.emoji-empty-title` | `--text-strong` `#fbfbfb` | `--surface-2 #242429` | **14.93 : 1** | ≥ 4.5 | PASS |
| `.emoji-section-header`, `.emoji-empty-hint`, `.emoji-loading-label` | `--text-muted` `#96979e` | `--surface-2 #242429` | **5.31 : 1** | ≥ 4.5 | PASS |
| Иконки вкладок `.emoji-tabs .tab` | `--text-muted` `#96979e` | `--surface-1 #202024` | **5.58 : 1** | ≥ 4.5 | PASS |
| Активная вкладка `.tab[aria-selected="true"]` | `--text-strong` `#fbfbfb` | `--surface-1` + `--state-selected` (11%) → `#39393c` | **11.12 : 1** | ≥ 4.5 | PASS |
| Плейсхолдер `.search-input::placeholder` | `--text-muted` `#96979e` (primitives.css:322-324) | `--surface-input #1e1f22` | **5.66 : 1** | ≥ 4.5 | PASS |
| Текст ошибки в `.reaction-error-status` | `--danger-text` `#ff9a9e` | `--surface-2 #242429` | **7.63 : 1** | ≥ 4.5 | PASS |
| Текст ошибки в `.reaction-picker-banner` | `--danger-text` `#ff9a9e` | `--surface-3 #28282d` | **7.24 : 1** | ≥ 4.5 | PASS |
| Основной текст баннера 429 | `--text-strong` `#fbfbfb` | `--surface-3 #28282d` | **14.18 : 1** | ≥ 4.5 | PASS |
| Бейдж `!` баннера 429 | `--text-on-warning` `#121214` | `--warning #fdb833` | **10.76 : 1** | ≥ 4.5 | PASS |
| Бейдж `!` баннера ошибки | `--text-on-danger` `#ffffff` | `--danger-fill #cf3a3f` | **4.85 : 1** | ≥ 4.5 | PASS (минимум) |
| Кнопки `.msg-toolbar-btn` (idle) | `--text-muted` `#96979e` | `--surface-4 #2c2d32` | **4.72 : 1** | ≥ 4.5 | PASS |
| Кнопка тулбара, нажатая реакция | `--text-accent` `#a5b0ff` | `--surface-4` + `--accent-soft` → `#333651` | **5.74 : 1** | ≥ 4.5 | PASS |
| Кнопка тулбара, `pending` + нажатая (`[data-pending="true"][aria-pressed="true"]`) | `--text-accent` `#a5b0ff` (правило пересечения §7.1) | `--surface-4` + `--accent-soft` → `#333651` | **5.74 : 1** | ≥ 4.5 | PASS |
| Кнопка тулбара, `pending` без нажатия | `--text-muted` `#96979e` | `--surface-4 #2c2d32` | **4.72 : 1** | ≥ 4.5 | PASS |

#### 7.2.2. Non-text (SC 1.4.11, $\ge 3.0:1$) и документированные исключения

| Элемент / пара | Цвет | Смежный фон | Факт | Норма | Итог |
|---|---|---|---|---|---|
| **Рамка невыбранной пилюли `.reaction`** | `--accent #5865f2` | `--bg-chat #1a1a1e` | **3.77 : 1** | ≥ 3.0 | PASS |
| Рамка пилюли к собственной заливке | `--accent #5865f2` | `--state-hover` → `#28282c` | **3.19 : 1** | ≥ 3.0 | PASS |
| Рамка пилюли в `:hover` | `--accent #5865f2` | `--surface-3 #28282d` | **3.18 : 1** | ≥ 3.0 | PASS |
| Рамка выбранной пилюли к заливке | `--accent #5865f2` | `--accent-soft` → `#242640` | **3.20 : 1** | ≥ 3.0 | PASS |
| Рамка `.reaction-add-btn` | `--accent #5865f2` | `--bg-chat` | **3.77 : 1** | ≥ 3.0 | PASS |
| Рамка `.emoji-cell[data-reacted="true"]` | `--accent #5865f2` | `--surface-3 #28282d` | **3.18 : 1** | ≥ 3.0 | PASS |
| Та же рамка к карточке | `--accent #5865f2` | `--surface-2 #242429` | **3.35 : 1** | ≥ 3.0 | PASS |
| Фокус-кольцо `--focus-ring` | `#a5b0ff` | `--bg-chat` / `--surface-2` / `--surface-3` / `--surface-4` | **8.47 / 7.54 / 7.16 / 6.71 : 1** | ≥ 3.0 | PASS |
| Рамка `.reaction-error-status` | `--danger #da3e44` | `--surface-2 #242429` | **3.50 : 1** | ≥ 3.0 | PASS |
| Нижняя граница баннера ошибки | `--danger #da3e44` | `--surface-3 #28282d` | **3.32 : 1** | ≥ 3.0 | PASS |
| Нижняя граница баннера 429 | `--warning #fdb833` | `--surface-3 #28282d` | **8.44 : 1** | ≥ 3.0 | PASS |
| *Исключение:* рамка панели `.msg-toolbar` | `--border-strong` 44% → `#5b5c62` | `--bg-chat` (панель `#2c2d32` на чате) | 2.60 : 1 | не применяется | контейнер |
| *Исключение:* рамка `.emoji-card` | `--border-strong` 44% → `#57575d` | `--bg-chat` | 2.42 : 1 (к своей подложке `--surface-2` — 2.15 : 1) | не применяется | контейнер |
| *Исключение:* разделители `.emoji-head` / `.emoji-preview` / `.emoji-section-header` | `--divider` 6% → `#2d2d31` | `--surface-1` | 1.18 : 1 | не применяется | декоративный |
| *Исключение:* скелетон `.emoji-skeleton-cell` | `--surface-3 #28282d` | `--surface-2 #242429` | 1.05 : 1 | не применяется | декоративный |

**Обоснование исключений (SC 1.4.11, формулировка «visual information required to identify user interface components and states»):**
1. **Контейнеры** (`.msg-toolbar`, `.emoji-card`) не являются контролами: их содержимое — кнопки и ячейки, каждая из которых имеет собственный проходящий индикатор (§7.2.2 выше); отделение панели от подложки обеспечивает `--shadow-popover` (`0 8px 16px` + 1px stroke), а не цвет рамки. Требование 3:1 к рамке панели не вытекает из SC.
2. **Разделители и скелетон** — декоративные линии и заглушки, они не передают состояние и скрыты от AT (`aria-hidden="true"`); состояние загрузки озвучивает подпись с 5.31:1.
3. **Неактивные контролы** (`:disabled`, `[aria-disabled="true"]`, `opacity: 0.45`) прямо исключены примечанием SC 1.4.11 («Inactive user interface components»).

**Чего делать нельзя:** «чинить» любую из этих пар подстановкой `--border-strong`. 44% альфы `#97979f` дают максимум **2.20:1** (`--bg-chat`), `--border` — 1.38:1, `--border-subtle` — 1.20:1. Именно поэтому граница контрола построена на `--accent`, а не на нейтральном бордере. Проверка воспроизводится любым contrast-checker'ом на паре `#5865f2` / `#1a1a1e` = 3.77.

**Если PM захочет нейтральную границу вместо акцентной:** непрозрачный `#4e5058` на `--bg-chat` действительно даёт 3.12:1, но такого токена в `tokens.css` нет — `--border-strong` это `rgb(151 151 159 / 44%)`, а не `#4e5058`. Ввести `--border-control: #4e5058` можно только правкой `tokens.css` (вне write-scope этой карточки); до этого действует `--accent`.

**Что НЕ сертифицируется:** light-тема (нет в `tokens.css`), hover-состояние `--accent-hover` на границе (2.72:1 — запрещено к использованию, §7.1), `--state-selected-strong` на границе выбранной пилюли (14% белого над тёмной подложкой даёт всего ≈1.53–1.57:1 — запрещено).

---

## 8. Верификация: существующие оракулы и red-first тесты для новых узлов

### 8.1. Уже заморожено (проверять, не ломать)

| # | Что проверяется | Оракул (файл:строка) |
|---|---|---|
| E-01 | `.msg-toolbar` ровно один на чат, `data-open="false"` без hover, `data-for-index`, 15 кнопок, 4 быстрые реакции, `aria-label` у каждой | `tests/unit/gui-spike-features.test.mjs:1155-1210` |
| E-02 | Один IPC на пару строка/эмодзи, оптимистичное `aria-pressed`, откат при 401 | `tests/unit/gui-spike-features.test.mjs:1212-1260` |
| E-03 | `.msg-toolbar` не ребёнок `#chat-list` | `tests/unit/gui-spike-dom.test.mjs:804` |
| E-04 | Пункты панели ищутся по `aria-label`: «Изменить сообщение», «Удалить сообщение», `[data-command="reply"]` | `tests/unit/gui-spike-features.test.mjs:1269,1328,1392`, `tests/unit/channel-switch.test.mjs:536,556,577` |
| E-05 | Пилюля: `aria-label="Реакция 🔥, 3"` | `tests/unit/chat-reactions.test.mjs:236-239` |
| E-06 | 4 кнопки `[data-command="react"]` в реальном канале, реальные id в аргументах IPC | `tests/unit/chat-reactions.test.mjs:385-410` |
| E-07 | Плоский `listbox`, `.emoji-row` = 0, 10 вкладок, `aria-controls="emoji-grid"`, `cell.title === cell.textContent`, уникальные `aria-label`, ровно один `tabindex="0"` | `tests/unit/gui-spike-features.test.mjs:372-403` |
| E-08 | Стрелки/`End` двигают `aria-selected` и roving | `tests/unit/gui-spike-features.test.mjs:405-426` |
| E-09 | `#emoji-grid` — `role="listbox"` с непустым `aria-label`; `cell.title === cell.textContent` после смены вкладки | `tests/unit/gui-spike-dom.test.mjs:490,494-498` |
| E-10 | Глобальный поиск, нормализация `:`/`_`/регистр, дедупликация | `tests/unit/gui-spike-features.test.mjs:429-470` |
| E-11 | `.row` — ровно 2 ребёнка, `.reactions-row` внутри `.text[data-stack]` | `tests/unit/gui-spike-dom.test.mjs:795-809` |
| E-12 | `kind` в serde = `"storage"`, пустой токен → `InvalidArg` | `rust/gui-spike/tests/reactions_contract.rs:93-125` |
| E-13 | Регистрация команды `toggle_reaction`, чтение токена через `backend.token()` | `rust/gui-spike/tests/reactions_contract.rs:127-137` |

### 8.2. Red-first тесты для новых узлов (пишутся до кода, падают на HEAD)

Ни один из узлов `#reaction-picker-popover`, `.reaction-add-btn`, баннера, loading/empty, `data-state`, `data-reacted` и `aria-busy` на пилюле не покрыт тестами сегодня (проверено `grep -rl`). Исключение — сам атрибут `data-pending`: он уже проверяется в `tests/unit/chat-reactions.test.mjs:372,376` и `tests/unit/gui-spike-features.test.mjs:1245,1463,1466`; не покрыт именно `aria-busy` на пилюле — это N-10. Каждая карточка начинается с падающего теста; ниже — точное место и утверждение.

| ID | Карточка / поведение | Файл теста | Утверждение (red на HEAD) |
|---|---|---|---|
| N-01 | Триггер `[data-command="add-reaction"]` открывает поповер вместо постановки 👍 | `tests/unit/chat-reactions.test.mjs` (новый `it`) | после `click()` на `.msg-toolbar [data-command="add-reaction"]`: `byId(app,"reaction-picker-popover")` не `null`, `hidden !== true`, `app.callsOf("toggle_reaction").length === 0`, у триггера `aria-expanded === "true"` |
| N-02 | UI-матчер ошибок сравнивает camelCase `kind` | `tests/unit/chat-reactions.test.mjs` (новый `it`) | `invoke("toggle_reaction")` реджектится объектом `{kind:"rateLimited",retry_after_ms:1500}` → `.reaction-error-status[data-error-kind="rateLimited"]` виден; `{kind:"unauthorized"}` → текст «Сессия истекла» |
| N-03 | Поповер: префиксные id, открытие/закрытие/`Escape`/возврат фокуса, lazy destroy, `Shift+Enter`, `data-reacted` | `tests/unit/chat-reactions.test.mjs` (новый `it`) | в поповере `byId("reaction-picker-grid")` не `null`, `byId("emoji-grid")` указывает на стационарный экран; `Escape` закрывает и возвращает `document.activeElement` на триггер; клик по `[data-action="reaction-picker-close"]` закрывает поповер и **не** меняет `data-active-screen` (обработчик `emoji-close` не вызывается); после любого закрытия `byId("reaction-picker-popover") === null` и `all(app, ".tab").length === EMOJI_CATEGORIES`; у триггера после открытия `aria-controls === "reaction-picker-popover"`, после закрытия атрибут снят (§5.1 исключение 3); `Shift+Enter` не закрывает и ставит `data-reacted="true"` |
| N-04 | `emoji.trim()` до IPC (Rust не тримит) | `tests/unit/chat-reactions.test.mjs` (новый `it`) | клик по ячейке с `data-emoji=" 👍 "` даёт `callsOf("toggle_reaction")[0].args.emoji === "👍"` |
| N-05 | Баннер 429: статичный alert + `aria-live="off"` счётчик, floor 1 с при `retry_after_ms = 0` | `tests/unit/chat-reactions.test.mjs` (новый `it`) | `role="alert"` текст не меняется между тиками; `.reaction-picker-banner-timer` имеет `aria-live === "off"`; при `retry_after_ms:0` счётчик показывает «1 с» |
| N-06 | `data-state` переходы `loading → ready → empty` | `tests/unit/gui-spike-dom.test.mjs` (новый `it`) | `.emoji-card` имеет `data-state="ready"`; после `search.value = "zzz"` + `input` → `data-state="empty"`, `#reaction-picker-empty` без `hidden`, грид `display:none` (нет 50/50) |
| N-07 | Панель тулбара не «всегда видима» после применения CSS §7.1 | `tests/unit/gui-spike-features.test.mjs` (расширить E-01) | `getComputedStyle(bar).display === "none"` при `data-open="false"` и `"flex"` при `"true"` (в шиме — проверка `data-open`, в браузерном прогоне — `display`) |
| N-08 | `aria-disabled` = no-op (без сети) | `tests/unit/chat-reactions.test.mjs` (новый `it`) | клик по пилюле с `aria-disabled="true"` не увеличивает `callsOf("toggle_reaction").length` |
| N-09 | Токенизация нового CSS | `waves/RW-40` гейт (команда ниже) | `0` совпадений `#hex\|rgba?\(\|hsla?\(\|!important`; `comm -23 used declared` пуст |
| N-10 | Пилюля в `pending` несёт `aria-busy="true"` (§2.1) | `tests/unit/chat-reactions.test.mjs` (новый `it`) | после клика до `settle`: у пилюли `data-pending="true"` **и** `aria-busy="true"`; после ответа `aria-busy` снят или `"false"` |
| N-12 | Кнопка «×» баннера ошибки закрывает только баннер и не обходит кулдаун (§5.3 п.3) | `tests/unit/chat-reactions.test.mjs` (новый `it`) | после 429 баннер `hidden === false`, корень `data-state === "error-429"`; `click()` по `.reaction-picker-banner-dismiss` → `banner.hidden === true`, `data-state === "ready"`, `byId("reaction-picker-popover")` не `null`, `document.body.getAttribute("data-active-screen")` не изменился, `callsOf("toggle_reaction").length` не вырос, `document.activeElement === byId("reaction-picker-search")`; до истечения `rateLimitUntilMs` кнопки остаются `aria-disabled="true"`, после истечения — снят и один раз объявлено «Можно повторить.» |

### 8.3. Команды гейта

```sh
# 1) нормативный CSS: 0 сырых цветов и 0 !important
awk '/^```css$/{f=1;next} /^```$/{if(f){f=0}} f' waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md > /tmp/spec-css.css
grep -nE '#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(|!important' /tmp/spec-css.css   # ожидается пусто

# 2) все var(--x) объявлены в tokens.css
grep -oE 'var\(--[a-zA-Z0-9_-]+' /tmp/spec-css.css | sed 's/var(//' | sort -u > /tmp/used.txt
grep -oE '^\s*--[a-zA-Z0-9_-]+\s*:' rust/gui-spike/ui/tokens.css | sed 's/^ *//;s/ *:$//' | sort -u > /tmp/declared.txt
comm -23 /tmp/used.txt /tmp/declared.txt                                    # ожидается пусто

# 3) существующие оракулы
cargo test -p gui-spike --test reactions_contract
node --test tests/unit/chat-reactions.test.mjs tests/unit/gui-spike-dom.test.mjs \
  tests/unit/gui-spike-features.test.mjs tests/unit/channel-switch.test.mjs \
  tests/unit/gui-spike-regressions.test.mjs
```

### 8.4. Что осталось вне этой карточки (нужны отдельные решения)

| ID | Что | Почему отдельно |
|---|---|---|
| N-01 | Открытие поповера по триггеру + `message-toolbar.js` | Меняет замороженное поведение (`message-toolbar.js:204-207`), нужен red-first тест и правка кода G2 |
| N-02 | Матчер `kind` в `reactions.js:196-203` (PascalCase → camelCase) | Правка продуктового кода, не документа |
| N-03 | `createEmojiConsole`: `cols`, `previewCategory`, `Escape`, `focusGrid` | Расширение API консоли; тесты E-07/E-08 должны остаться зелёными |
| N-04 | `trim()` в `reactions.rs::validate` | Правка Rust; сейчас UI обязан тримить сам |
| N-05 | Синхронизация `DOM-CONTRACT.md §9.2` (D-05 `.emoji-row`) и `THEME-SPEC.md §2.5` (`--warning-text`) | Чужие документы, правит PM/Team Lead |
| N-06 | Light-тема | В `tokens.css` нет ни одного light-блока; сертифицировать нечего |
| N-11 | Двойной счётчик от эха собственной реакции (`chat.js:3694-3696`) | Продуктовый дефект live-счётчика: `displayCount` складывает optimistic override с абсолютным live-счётом. Правка `chat.js`/`dispatch-model.js` с red-first тестом «один собственный ADD не даёт +2» |
