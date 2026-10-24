# RW-39 / W39b — SPEC: дизайн-система DL и экраны (Phase 1b)

Статус: спецификация для вёрстки (W39c) и фронтенда (W39d). Дата: 2026-10-23.
Автор: W39b (Design/UI-UX). PM: DeepSeek. Волна: RW-39 (foundation P3-01/P3-02).

Документ описывает **числа и правила**, а не готовый CSS: единственный источник значений —
`waves/RW-39/DESIGN/tokens.css`, точная разметка — `DOM-CONTRACT.md`.

## 1. Источники и статусы значений

| Код | Статус | Источник |
|---|---|---|
| `[M]` | измерено | `/home/sanya/re-deep/ui/00-app.png`, `ui8/*.jpg` (пиксельные замеры, dpr 1.25) |
| `[T]` | токен | `/home/sanya/re-deep/ui/tokens.json` (4795 CSS-переменных, тема dark, сборка 1.0.162) |
| `[L]` | RE-контракт | `waves/RW-37/RE/layout.json` (schema `rw37/re-layout-v1`, проверен в RW-37) |
| `[A]` | assumption | выбрано нами; обоснование рядом; не выдаётся за измерение |
| `[F]` | frozen | число заморожено тестами RW-38 (`tests/unit/gui-spike-dom.test.mjs`) |

Правило: **каждое число в этом документе имеет код**. Если значения нет в таблице — его
нельзя придумывать на месте: добавьте токен в `tokens.css` и строку сюда.

На момент написания (2026-10-23) `DESIGN/tokens.json` и `DESIGN/layout.json` от W39a ещё
не существовали: раскладка взята из принятого RE-контракта `waves/RW-37/RE/layout.json`,
цвета — из `ui/tokens.json` и пиксельных замеров. Когда W39a сдаст свои файлы, они
сверяются с этим документом; `tokens.css` самодостаточен и от них не зависит.

Референсы — только чтение (`/home/sanya/re-deep` — golden). В репозиторий не копируются ни
PNG, ни шрифты, ни иконки, ни спрайты: переносятся **значения** (цвета, размеры, отступы).

## 2. Границы и юридическая рамка (кратко)

- Проект неофициальный; бренд в UI — **`DL`** (монограмма в рейле). Название и логотип
  Discord не используются; в UI нет ни логотипов, ни фирменных функций Discord (Nitro,
  Boost). Имена гильдий/каналов/участников приходят из санитизированных фикстур
  (`ITSKIDON PC BOOST` — тестовая гильдия), это данные, а не бренд проекта.
- Шрифты — только **системные** (`system-ui`), см. §4. Шрифты Discord (`gg sans`,
  `ABC Ginto Discord`, `Noto Sans` как поставляемый веб-ассет) **не** подключаются.
- Emoji — **системный** emoji-шрифт ОС, без Twemoji/спрайтов Discord.
- Иконки — **собственные inline-SVG** (проект, лицензия кода MIT OR Apache-2.0), сетка
  24×24, `stroke="currentColor"`, `stroke-width="2"`, `stroke-linecap="round"`. Набор и
  геометрия — §12. Сторонние наборы не подключаются (нет SBOM-проверки, `T-HARN-07`).
- Цвета/размеры из дампа — это измерения, не копирование ассетов. Акцент `#5865f2`
  взят как значение токена темы (`--brand-500`), а не как элемент фирменного стиля.
- Сеть в UI не используется: все экраны работают на `fixtures/` (W39e) или на локальных
  данных спайка. Отправка сообщений отсутствует.

## 3. Цвета (dark theme)

### 3.1 Поверхности (elevation ladder)

| Токен | Значение | Где | Источник |
|---|---|---|---|
| `--bg-app` | `#121214` | рейл, дерево каналов, titlebar | `[M]` (18,18,20) + `[T] --primary-800` |
| `--bg-chat` | `#1a1a1e` | чат, панель участников, шапка чата | `[M]` (26,26,30) + `[T] --primary-730` |
| `--surface-1` | `#202024` | строка пользователя, hover строки | `[M]` (32,32,36) |
| `--surface-2` | `#242429` | модалка/поповер/меню, hover сообщения | `[M]` (36,36,41) + `[T] --modal-background` |
| `--surface-3` | `#28282d` | приподнятая карточка, hover пункта меню | `[T] --background-surface-higher` |
| `--surface-4` | `#2c2d32` | максимум поднятия (dropdown над меню) | `[T] --background-surface-highest` |
| `--surface-sunken` | `#222327` | поле ввода сообщения | `[M]` (34,35,39) + `[T] --chat-background-default` |
| `--surface-input` | `#1e1f22` | input на `--surface-2` (поиск в настройках) | `[A]` blend чёрный 12% |

Правило: поверхности не «изобретаются» — новый слой только из этой лестницы. Границы панелей —
1px `--border-subtle`, не тени (референс: граница панели `#29292d` на стыке чата и участников).

### 3.2 Текст

| Токен | Значение | Применение | Источник |
|---|---|---|---|
| `--text-strong` | `#fbfbfb` | имя автора, заголовок канала, активный пункт | `[M]` (251,251,251) + `[T] --text-strong` |
| `--text` | `#efeff1` | тело сообщений, названия каналов | `[M]` + `[T] --text-default` |
| `--text-muted` | `#96979e` | placeholder, время, подписи, счётчики | `[M]` (150,151,158) + `[T] --text-muted` |
| `--text-subtle` | `#abacb2` | вторичные подписи в настройках | `[T] --text-subtle` |
| `--text-faint` | `#81828a` | заголовки категорий каналов/групп | `[M]` (129,130,138) + `[T] --chat-text-muted` |
| `--text-link` | `#4d96ee` | ссылки, info-иконки | `[M]` + `[T] --text-link` |
| `--text-accent` | `#a5b0ff` | акцентный текст на тёмном | `[A]`: `--accent` как текст не проходит AA |
| `--text-on-accent` | `#ffffff` | текст на `--accent` | `[A]` контраст 4.61 |
| `--text-on-danger` | `#ffffff` | текст на `--danger-fill` | `[A]` контраст 4.85 |
| `--text-inverse` | `#2f3035` | текст на обратных (светлых) поверхностях; на плашке `--warning` — `--text-on-warning` (10.76) | `[T] --text-invert` (D-02, Δ23; на `--warning` 7.57) |

Ограничение: `--text-faint` разрешён **только** на `--bg-app`/`--bg-chat` (4.89/4.54);
на `--surface-2` и выше он даёт 4.04 — там использовать `--text-muted`.

### 3.3 Акцент, опасность, статусы

| Токен | Значение | Источник | Применение |
|---|---|---|---|
| `--accent` | `#5865f2` | `[M]` баннер + `[T] --brand-500` | primary-кнопка, активный rail-btn, фокус-заливка, mention-подсветка |
| `--accent-hover` | `#4654c0` | `[T] --brand-560` (D-02, Δ4) | hover primary |
| `--accent-active` | `#3c45a5` | `[A]` | pressed primary |
| `--accent-soft` | `rgb(88 101 242 / 16%)` | `[T]` message-highlight | подсветка выбранного/упоминания |
| `--danger` | `#da3e44` | `[T] --status-danger` | иконка/точка статуса (non-text ≥3); текст — `--danger-text` |
| `--danger-text` | `#ff9a9e` | `[A]` | danger-текст на поверхностях ≥ `--surface-2` |
| `--danger-fill` | `#cf3a3f` | `[A]` | destructive-кнопка, белый текст 4.85 (дамп `#da3e44` с белым = 4.42 < AA) |
| `--danger-fill-hover` | `#b92d33` | `[A]` | hover destructive (6.02) |
| `--warning` | `#fdb833` | `[T] --status-warning` | предупреждения, mention-полоса |
| `--positive` | `#3d9e60` | `[T] --status-positive` / `--green-360` | success, online; текст — `--positive-text` |
| `--positive-fact` | `#45a366` | `[M]` статус-точка панели участников (5.52 на `--bg-chat`; на `--surface-4` 4.37 — non-text) | фактический зелёный дампа; ≠ `--positive` (Δ26), только точка/иконка |
| `--positive-text` | `#57d98a` | `[A]` | positive-текст на hover-поверхностях |
| `--info` | `#4d96ee` | `[T] --text-link` | info-плашка |
| `--status-online` | `#3d9e60` | `[T] --status-positive` | точка статуса |
| `--status-idle` | `#fdb833` | `[T] --status-warning` | точка статуса |
| `--status-dnd` | `#da3e44` | `[T] --status-danger` | точка статуса |
| `--status-offline` | `#80848e` | `[A]` | точка статуса |
| `--status-streaming` | `#593695` | `[A]` (в дампе 1.0.162 токена нет) | точка статуса |

### 3.4 Контраст (WCAG 2.2 AA)

Расчёт по относительной яркости sRGB; требование: ≥ 4.5:1 для текста < 24px (или < 19px
bold), ≥ 3:1 для крупного текста, иконок и границ контролов.

| Текст \ Фон | `#121214` | `#1a1a1e` | `#202024` | `#242429` | `#2c2d32` |
|---|---|---|---|---|---|
| `--text` `#efeff1` | 16.29 | 15.11 | 14.14 | 13.45 | 11.96 |
| `--text-strong` `#fbfbfb` | 18.08 | 16.77 | 15.69 | 14.93 | 13.28 |
| `--text-muted` `#96979e` | 6.43 | 5.96 | 5.58 | 5.31 | 4.72 |
| `--text-subtle` `#abacb2` | 8.26 | 7.66 | 7.17 | 6.82 | 6.07 |
| `--text-faint` `#81828a` | 4.89 | 4.54 | 4.25 ✗ | 4.04 ✗ | 3.59 ✗ |
| `--text-link` `#4d96ee` | 6.16 | 5.71 | 5.34 | 5.09 | 4.52 |
| `--text-accent` `#a5b0ff` | 9.14 | 8.47 | 7.93 | 7.54 | 6.71 |
| `--danger-text` `#ff9a9e` | 9.24 | 8.57 | 8.02 | 7.63 | 6.78 |

Пары контролов: `#ffffff` на `--accent` = **4.61**; `#ffffff` на `--danger-fill` = **4.85**
(белый на `--danger` `#da3e44` = 4.42 — поэтому заливка кнопки затемнена, значение дампа
остаётся для иконок/точек); `#121214` на `--warning` = **10.76**; `--text-inverse` `#2f3035`
на `--warning` = **7.57** (белый на `--text-inverse` = 13.17); `--focus-ring` `#a5b0ff` на всех
поверхностях 6.71–9.14 (≥ 3:1 для non-text); статус-точки как non-text: `--danger` 3.93,
`--warning` 9.98, `--positive` 5.17 на `--bg-chat` (≥ 3:1);
`--scrollbar-thumb` `#5f606a` на `--bg-chat` = 2.78, на `--bg-app` = 3.0
(`[A]` скроллбар декоративен и дублируется колесом/клавиатурой; на светлых поверхностях
thumb не используется).

Правила:
1. `--accent` **не используется как цвет текста** (2.98–4.06 — ниже AA) — только заливка/иконка.
2. `--text-faint` не поднимается выше `--bg-chat`.
3. Hover-фон не должен снижать контраст текста ниже AA: hover затемняет/осветляет фон ≤ 6%,
   поэтому пары `--text`/`--text-muted` остаются ≥ 5:1.
4. Информация не передаётся только цветом: статус — точка + `aria-label`, ошибка — иконка +
   текст, unread — точка + жирный шрифт.

## 4. Типографика

Стек: `--font-sans` = `system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue",
Arial, "Noto Sans", "Liberation Sans", sans-serif` + системный emoji-стек.
`--font-mono` — только для кода/ID. Никаких `@font-face`, `url()`, веб-шрифтов.

| Стиль | Размер/интерлиньяж | Вес | Применение | Источник |
|---|---|---|---|---|
| display | 24 / 30 | 600 | заголовок окна настроек | `[A]` |
| title | 20 / 24 | 600 | заголовки секций настроек | `[A]` |
| body | 16 / 22 | 400 | сообщения, названия каналов, заголовок чата | `[M] 22px` `[T] --chat-markup-line-height` |
| body-strong | 16 / 22 | 600 | имя автора, активный канал | `[M]` |
| ui | 14 / 18 | 400/500 | кнопки, таймстемпы, подписи участников | `[M]` |
| small | 12 / 16 | 500/600 | категории, группы участников, счётчики | `[M]` |
| caption | 11 / 14 | 700 | бейджи (`BOT`, роль) | `[A]` |

Правила:
- Категории/группы — `text-transform: uppercase` + `letter-spacing: 0.02em` `[A]`.
- Обрезание — `text-overflow: ellipsis; white-space: nowrap; min-width: 0` (имена каналов,
  участников, автор). Перенос — только в теле сообщения (`overflow-wrap: anywhere`).
- Числа в метриках — `font-variant-numeric: tabular-nums` (уже в спайке).
- Минимальный размер текста в UI — 11px (бейджи), контраст бейджа ≥ 4.5.
- `rem` не используем: `px` фиксирует паритет при системном масштабе; масштаб 200%
  достигается zoom браузера/webview и не ломает сетку (все контейнеры на flex/grid).

## 5. Сетка, спейсинг, shell

### 5.1 Размеры shell

| Элемент | Значение | Допуск | Источник |
|---|---|---|---|
| Рейл (`#guild-rail`) | **64px** | 64–72 (ТЗ) | `[L] rail_w=64`, `[T] --custom-guild-list-width = 40+12*2` |
| Дерево каналов (`#channel-tree`) | **240px** | ~240 (ТЗ) | `[A]` по RUNBOOK; RE-значение 311 → `--layout-tree-w-wide` при ≥1600px `[L]` |
| Панель участников (`#member-panel`) | **240px** | ~240 (ТЗ) | `[A]`; RE 256 → `--layout-member-panel-w-wide` при ≥1600px `[L]` |
| Titlebar (`.topbar`) | **32px** | — | `[L] topbar_h=32` |
| Шапка чата (`#chat-header`) | **49px** | ~48 (ТЗ) | `[L] header_h=49`, `[T] --custom-channel-header-height` |
| Composer (`#composer`) | **56px** | 56 | `[L] composer_h=56`, `[T] --custom-channel-textarea-text-area-height` |
| Строка пользователя (`#user-bar`) | **56px** | — | `[M]` y 1044…1100 |
| Строка канала | **28px**, шаг **30px** | — | `[L] row_heights.channel`; шаг 28 высота + 2 зазор `[M]` (`--row-channel-pitch`) |
| Строка участника (панель и список) | **42px**, шаг **43px** | — | `[L] row_heights.member`; шаг `[M]` (JPEG ±1); `--row-h-member` (D-04) |
| Строка сообщения: продолжение / начало группы | **26px / 48px** | — | `[L] message_cozy_continuation / message_cozy_group_start`; `--row-h-continuation`, `--row-h-group-start` (D-04) |
| Перенесённая строка сообщения | **+22px** за строку | — | `[L] line_extra`; `--row-line-extra` (D-04) |

Высота строки сообщения — переменная (D-04): `26 + 22×переносы` для продолжения группы,
`48 + 22×переносы` для начала группы, участник `42`. Фиксированного трека в модели нет:
замороженные значения RW-38 — приближение тестов, снимаются в W39d
(`--row-message-track`/`--row-member-track`).

Формула колонок: `rail 64 + tree 240 + chat (1fr, min 0) + member-panel 240` при окне
≥ 1200px. Высота: `titlebar 32 + header 49 + (chat 1fr) + composer 56`.
`chat_w` на референсе 1417 при окне 2048 `[L]` — у нас `flex: 1 1 auto; min-width: 0`.

Адаптив (правила, не «магия»):

| Ширина окна | Поведение |
|---|---|
| ≥ 1600px | tree 311, member panel 256 (`[L]`, измеренные значения) |
| 1200–1599px | tree 240, member panel 240 (базовые токены) |
| 960–1199px | member panel скрывается (`display:none`), tree 240 |
| 720–959px | tree 200 (минимум), member panel скрыт; chat ≥ 520px |
| < 720px | tree скрыт (`#channel-tree[aria-hidden="true"]`), навигация только рейлом; экран не считается поддерживаемым `[A]` |

Переполнение: `html, body { height: 100%; overflow: hidden }`; скроллятся только списки
(`#chat-list`, `#member-list`, `#member-panel-list`, `.emoji-grid`, `.settings`) и дерево
(`.tree-scroll`). Горизонтального скролла в shell нет.

### 5.2 Спейсинг

Шкала `--space-1..13` = 2,4,6,8,12,16,20,24,32,40,48,56,64 `[A]` (шаг 4, как в референсе:
padding строк 4/8, панели 12/16, секции 24).

| Место | Значение |
|---|---|
| Горизонтальный padding контента | 16 (`--chat-pad-x`, `--layout-panel-pad`) |
| Padding строки канала | `0 8px`, высота 28 |
| Padding строки участника (панель) | `4px 8px` (32 аватар + 4+4 + 2) |
| Гаттер сообщения | 64 (16 + аватар 40 + 8) |
| Отступ строки сообщения | `2px 16px` (внутри высоты по D-04) |
| Секция настроек | 24 сверху/снизу, 16 между строками |

### 5.3 Радиусы и тени

Радиусы: `4 / 8 / 12 / 16 / 24 / round` `[T]`. Кнопки и input — 8, меню/поповер — 8,
аватар — round, rail-btn в покое — round, активный — 14 `[A]` (референс «морфит» иконку),
карточка эмодзи — 8, бейдж — 8 (или round для счётчика `[A]`).

Тени: `--shadow-elevation-low` (шапки), `-stroke` (1px контур), `-medium`, `-high`
(поповер/меню/модалка). Меню/поповер = `--shadow-popover`. Внутри одной панели тени не
используются — только границы `--border-subtle`.

### 5.4 Слои и движение

`--z-base 0 < header 10 < popover 100 < modal 200 < tooltip 300 < toast 400`.
Движение: `--motion-fast 120ms` (hover/press), `--motion-base 180ms` (панели, вкладки),
`--ease-out`. При `prefers-reduced-motion: reduce` длительности = 0 (токены), геометрия
не меняется. Анимации только `opacity`/`transform` (без layout-тряски).

## 6. Каталог токенов

Полный список — `tokens.css` (12 групп: поверхности, текст, акцент/статусы, границы/состояния,
типографика, спейсинг, радиусы, тени, раскладка, строки, компоненты, слои/движение).
Обязательные имена из ТЗ присутствуют: `--bg`, `--surface`, `--text`, `--muted` (алиас
`--text-muted`), `--accent`, `--space-*`, `--radius-*`, `--layout-*`.

W39c подключает файл **копией** в `ui/components/tokens.css` (Tauri asset protocol не ходит
за пределы `ui/`): источник истины — этот файл, md5 фиксируется в отчёте волны; локальные
переопределения значений запрещены (ревью сравнивает md5 и diff).

## 7. Компоненты

Формат: анатомия → размеры → состояния → токены → ARIA/поведение. Классы — из
`DOM-CONTRACT.md` §3–§5 (там же точная разметка и состояния).

### 7.1 Rail button (`.rail-btn[data-screen]`)

- Анатомия: `button` 40×40, иконка 24 (системный emoji или inline-SVG), `aria-label`,
  индикатор активности слева (псевдоэлемент 4×32, `--text-strong`).
- Состояния: default (bg transparent, иконка `--text-muted`); hover (`--state-hover`,
  иконка `--text`); active/selected (`--accent-soft`, иконка `--text-strong`, радиус 14,
  индикатор 4×32); focus-visible (ring); disabled (opacity 0.45, `aria-disabled`).
- Поведение: один `active` на приложение; `aria-current="page"` только у активного;
  `title` = «Чат 100k» и т. п. (тест RW-38 сверяет порядок `data-screen` = chat, emoji,
  members, settings `[F]`).
- Токены: `--layout-rail-item`, `--radius-round`, `--radius-selected`, `--state-hover`,
  `--accent-soft`.

### 7.2 Guild item (`.guild-item`)

- Анатомия: `button` 40×40, круг, внутри — 2-буквенная монограмма (цвет детерминирован от
  id) либо inline-SVG; `aria-label="Сервер …"`, `aria-current="true"` у выбранного.
- Состояния: default (bg `--surface-1`), hover (bg `--surface-3`, radius 14), selected
  (`--accent` bg, белая монограмма, radius 14, индикатор 4×32), focus-visible.
- 3 элемента-заглушки серверов `[A]` (fixtures READY: 7 гильдий `[L]`); остальные — в
  выпадающем списке `data-action="guild-menu"` (popover §7.14).
- Токены: `--layout-guild-item`, `--radius-round`, `--radius-selected`.

### 7.3 Channel item (`.channel-item`)

- Анатомия: `button` высотой 28, padding `0 8px`, gap 6, иконка 20 (`#` / динамик) +
  `.channel-name` 16/20 (20 = 28 − 2×4 `[L]`), `border-radius: 4`; вертикальный зазор
  между строками 2px → шаг 30 `[M]` (`--row-channel-pitch`).
- Состояния: default (`--text-faint`, иконка `--text-faint`); hover (`--state-hover`,
  текст `--text`); selected (`--state-selected` = `#2c2c30` `[M]`, текст `--text-strong`,
  `aria-current="page"`); unread (`--text-strong` + точка 8px слева `--text-strong`);
  muted (`--text-faint`, иконка muted); focus-visible.
- Категория-заголовок: 12/16, 600, uppercase, `--text-faint`, высота 24, chevron 12.
- Токены: `--row-channel`, `--state-hover`, `--state-selected`.

### 7.4 Chat header (`#chat-header`)

- Анатомия (слева→справа): toggle дерева (иконка 20), `#` 24 `--text-faint`, название
  канала 16/22 600 `--text-strong`, разделитель `|` (1×20 `--border`), затем actions:
  threads/pins/members (icon-btn 32×32) и поиск.
- Размер: 49px, padding `0 16px`, bg `--bg-chat`, border-bottom `--border-subtle`,
  `z-index: var(--z-header)`; поиск 32 высотой, radius round, bg `--surface-input`,
  иконка 16, placeholder `--text-muted`, ширина `clamp(144px, 20vw, 244px)` `[A]`.
- Состояния: sticky (не скроллится), hover/focus на кнопках, `aria-expanded` у toggle.
- Токены: `--layout-header-h`, `--search-height`, `--border-subtle`.

### 7.5 Message row (`.row`, cozy/compact, group start/continuation)

- Высота — переменная (D-04): `--row-h-group-start` 48 у начала группы, `--row-h-continuation`
  26 у продолжения, `+ --row-line-extra` 22 за каждый перенос; внутри — сетка 2×22:
  `grid-template-columns: var(--message-gutter) 1fr; grid-template-rows: 22px 22px`.
- **Group start** (`data-group="start"`): аватар 40 (абсолютно, `left:16px; top:2px`,
  radius round), строка 1 — имя 16/22 600 `--text-strong` + бейдж `BOT` + время 12/16
  `--text-muted`; строка 2 — текст 16/22 `--text` с `padding-inline-start: 64px`.
- **Continuation** (`data-group="continuation"`): `.author` скрыт (`display:none`),
  текст занимает обе строки, вертикально центрирован (11px сверху/снизу), отступ 64.
- **Compact density** (`:root[data-density="compact"]`): гаттер 80 `[L]`, автор всегда
  скрыт, отступы 11/11; высота считается по D-04 (одна линия 22 + отступы), фиксированной
  константой не задаётся.
- Состояния: default; hover `--row-hover-bg` (`#202024` `[M]`); selected/context
  `--row-selected-bg` + левая полоса 2px `--accent`; mention — фон `--accent-soft`,
  левая полоса 2px `--warning`, время/иконка `--warning`; placeholder (загрузка) —
  bg `--surface-1`, opacity 0.45, **без детей** `[F]`.
- Контент: markdown-минимум (жирный, курсив, код, ссылка `--text-link`, упоминание
  `--text-accent` на `--accent-soft`); вложения — карточка `--surface-2`, radius 8.
- ARIA: `role="listitem"`, `aria-label="<автор>, <время>: <текст>"`; строки не фокусируемы,
  фокус — на `#chat-list` (roving не требуется). Группировка озвучивается `aria-label`
  (автор повторяется в каждой строке — приемлемо для спайка).
- Токены: `--row-h-continuation`, `--row-h-group-start`, `--row-line-extra`,
  `--message-gutter`, `--line-height-message`, `--avatar-lg`, `--row-hover-bg`.

### 7.6 Member row (`.member`) и заголовок группы

- Список (панель и виртуализация): высота **42** `[L]` — `--row-h-member` (D-04), 3 ребёнка:
  `.avatar` (32, status-dot 10 в правом нижнем углу, ring 2px `--bg-chat`), `.member-name`
  (14/18 500 `--text`; badges 16), `.member-role` (12/16 `--text-muted`, ellipsis).
- Шаг строки в панели — **43** `[L]`/`[M]` (`--row-member-pitch`), padding `4px 8px`, hover `--state-hover`,
  selected `--state-selected`; имя может быть окрашено ролью — только из палитры
  `--text`/`--text-accent`/`--positive-text` (контраст ≥ 4.5).
- Заголовок группы (`.members-group-title`): 12/16 600 uppercase `--text-faint`,
  блок **42** `[M]` (`--row-member-group`: 16px pad-top + строка 24 + 2px), счётчик в тексте
  («В СЕТИ — 735»); в панели это обычный элемент, в виртуализованном списке — **нет**
  (ограничение RW-38, см. `DOM-CONTRACT.md` §6.4 и §7.2).
- ARIA: `role="listitem"`, `aria-label="<имя>, <статус>, <роль>"`; статус — не только цвет
  (текст в aria-label). Клик/Enter — профиль (popover §7.14).
- Токены: `--row-h-member`, `--row-member-pitch`, `--avatar-md`, `--status-*`.

### 7.7 Composer (`#composer`)

- Анатомия: контейнер 56px, padding `0 16px 8px`(внутри 44px поле): `+` (attach, 24),
  `textarea` (1 строка, `min-height: 44`, max 5 строк → но контейнер 56 фиксирован:
  при росте textarea поле скроллится `[A]`), справа emoji 24 и gift 24.
- Стиль поля: bg `--surface-sunken`, radius 8, padding `11px 16px`, placeholder
  `--text-muted`, focus — ring `--focus-ring` (без смены bg).
- Состояния: default; focus-within (ring); disabled/readonly — `--text-muted`,
  `aria-disabled="true"`, отправка недоступна; ошибка — border `--danger`, текст
  `--danger-text` (не в Phase 2).
- Поведение: Enter — новая строка не отправляется (спайк без сети); Ctrl+E открывает
  экран «Эмодзи»; Esc — закрывает поповер/возвращает в чат; вставка эмодзи вставляет
  символ в позицию курсора.
- ARIA: `role="group"` + `aria-label="Поле ввода сообщения"`; `textarea` c
  `aria-label="Сообщение"` и `aria-describedby="composer-hint"`; подсказка
  `.chat-notice` (24px, 12/16 `--text-muted`) — «Отправка недоступна в спайке».
  Отдельно от подсказки: синяя unread-полоса «20+ новых сообщений» — **34**
  border-box (`--unread-banner-h`, контент 32, D-13), не путать с `.chat-notice`.
- Токены: `--layout-composer-h`, `--surface-sunken`, `--composer-radius`.

### 7.8 Emoji tab (`.tab`) и сетка (`.emoji-cell`)

- Пикер: карточка `--emoji-picker-w` 498 × `--emoji-picker-h` 440 `[T]`, radius 8,
  `--shadow-popover`, bg `--surface-2`; шапка 48 (`--emoji-section-h`): поиск-pill 32
  + close 20; тело: rail 48 слева, сетка справа.
- Вкладка-категория: `button` 40×40 в rail 48, иконка 20, `role="tab"`,
  `aria-selected`, `aria-controls="emoji-grid"`; выбранная — bg `--state-selected-strong`,
  иконка `--text-strong`, слева полоса 2px `--accent`; hover `--state-hover`.
- Ячейка: 44×44 `[M]`, radius 8, emoji 26px (системный шрифт), hover `--state-hover`,
  focus-visible ring, active `--state-active`, selected `--accent-soft` + ring 1px
  `--accent`. Заблокированная (locked) — opacity 0.4 + иконка замка 12 `[A]`
  (в фикстурах спайка нет locked — только стиль).
- Сетка: 9 колонок `[M]` (`--emoji-cols`), gap 4, padding 12; заголовок секции 32
  (12/16 600 uppercase `--text-faint`).
- ARIA: `role="grid"` → `role="row"` → `role="gridcell"`; `aria-rowcount/colcount`;
  структурно `#emoji-grid` = `grid` + `row`-обёртки (`display: contents`), ячейки — `gridcell`
  (D-05; закрывает структурную часть OBS-01); roving tabindex: одна ячейка `tabindex="0"`,
  остальные `-1`; стрелки перемещают фокус, Enter/Space вставляют эмодзи, Home/End — к краю
  строки, PageUp/PageDown — на 3 строки.
- Токены: `--emoji-*`, `--state-*`.

### 7.9 Settings toggle (`.toggle` + `input[role=switch]`)

- Анатомия (label): текст 14/18 `--text` + подсказка 12/16 `--text-muted` слева,
  контрол справа: трек 40×24, thumb 18 (сдвиг 18px), `input` визуально скрыт
  (`opacity:0; position:absolute`) но фокусируем `[F]` (тест кликает именно `input`).
- Состояния: default (трек `rgb(0 0 0 / 12%)`, border `--border`), hover (border
  `--border-strong`), checked (трек `--accent`, thumb белый), checked+hover
  (`--accent-hover`), focus-visible (ring на треке через `:focus-visible + .toggle-track`),
  disabled (opacity 0.45).
- ARIA: `role="switch"`, `aria-checked` синхронизирован с `checked` (W39d — на `click`
  и `change`), доступное имя — текст label.
- Строка настройки: label 14/18 `--text`, значение 14/18 `--text-muted`, контрол справа;
  высота 48 `[A]`, разделитель `--divider`; группа — `h3` 20/24 600 (3 группы `[F]`).
- Токены: `--switch-*`, `--state-*`.

### 7.10 Search input (`.search-field` / `.search-input`)

- Анатомия: контейнер 32 (`--search-height`), radius round, bg `--surface-input`,
  иконка 16 `--text-muted`, input 14/18 `--text`, placeholder `--text-muted`,
  кнопка очистки 16 (появляется при вводе, `aria-label="Очистить поиск"`).
- Состояния: default (border transparent), hover (bg `--surface-3`), focus-within
  (ring 2px `--focus-ring`, bg `--surface-2`), disabled (opacity 0.45), invalid
  (border `--danger` + `aria-invalid="true"`).
- ARIA: `input[type=search]` c `aria-label`; контейнер `role="search"`; результат поиска
  анонсируется `aria-live="polite"` (счётчик).
- Токены: `--search-height`, `--surface-input`.

### 7.11 Icon button (`.icon-btn`)

- 32×32 (в шапке) / 24×24 (в composer), radius 8/4, иконка 20/24 `currentColor`,
  hover `--state-hover`, active `--state-active`, focus ring, disabled 0.45.
- Обязателен `aria-label` (иконка `aria-hidden="true"`), `type="button"`.

### 7.12 Badge (`.badge`)

- Высота 16, padding `0 4px`, radius 8, 11/14 700 uppercase; варианты:
  `--bot` (bg `--accent`, текст белый), `--role` (bg `--badge-secondary-bg` = `#97979f` 16%,
  текст `--text-subtle`), `--count` (round, min-width 16, bg `--danger-fill`, белый).
- Текст бейджа дублируется в `aria-label` строки (не полагаться на визуальный бейдж).

### 7.13 Avatar (`.avatar` + `.status-dot`)

- Размеры 16/20/32/40 (`--avatar-*`); фон — детерминированный `hsl(var(--avatar-hue)
  var(--avatar-s) var(--avatar-l))`, hue = hash от id (0…359), белые инициалы
  (`--avatar-fg`) при контрасте ≥ 4.5 (светлота 32% гарантирует это для любого hue) `[A]`.
  Инициалы 1–2 символа, 600.
- Статус-дот 10 (32px аватар) / 12 (40px), ring 2px цвета родительской поверхности;
  `aria-label="В сети|Не беспокоить|Неактивен|Не в сети"` на обёртке.
- Изображения профиля в Phase 2 не грузятся (нет сети) — только инициалы/цвет.

### 7.14 Menu / popover (`.menu`, `.popover`)

- Меню: bg `--surface-2`, radius 8, padding 8, border 1px `--border`, `--shadow-popover`,
  min-width 188, max-height 320 со скроллом; пункт 32, padding `0 8px`, radius 4,
  hover `--state-hover`, selected `--state-selected-strong`, destructive — текст
  `--danger-text`; разделитель 1px `--border-subtle`.
- Поповер (профиль/меню сервера): ширина 300 `[T] --custom-user-profile-popout-width`,
  bg `--surface-2`, radius 8, шапка 60 с баннером `--accent`, аватар 80 с ring 6
  цвета поверхности, имя 20/24 600, тег 14/18 `--text-muted`.
- ARIA: `role="menu"`/`role="menuitem"` (или `dialog` для профиля), `aria-haspopup`,
  `aria-expanded` на опенере; фокус-ловушка для профиля, Esc закрывает, клик вне —
  закрывает; первый пункт получает фокус.
- Токены: `--menu-*`, `--modal-*` (для модалки), `--shadow-popover`.

### 7.15 Tooltip

- bg `--bg-app`, текст 14/18 `--text`, radius 8, padding `6px 8px`, `--shadow-border`,
  задержка 400ms, `role="tooltip"`, `aria-describedby` на опенере; не перекрывает
  курсор (`pointer-events:none`); при reduced-motion без fade.

### 7.16 Empty state / error state

- `.empty-state`: центр, иконка 48 `--text-faint`, заголовок 16/22 600 `--text`,
  подсказка 14/18 `--text-muted`, опциональная secondary-кнопка; `role="status"`.
- `.error-state`: иконка `--danger`, заголовок `--text`, текст `--danger-text`,
  кнопка «Повторить» (secondary).
- Тексты (RU): чат — «Сообщений пока нет» / «Канал пуст»; поиск — «Ничего не найдено»;
  эмодзи — «Эмодзи не найдены»; участники — «Нет участников по фильтру»;
  настройки — «Раздел не найден».

### 7.17 Scrollbar

- Тонкий 8px, thumb `--scrollbar-thumb`, track transparent, thumb скрыт без hover
  (opacity 0 → 1 за 120ms) `[A]`; в Firefox — `scrollbar-width: thin`.
  Скроллбар не единственный способ прокрутки (колесо, клавиатура, PageUp/Down).

## 8. Состояния (сводная матрица)

| Компонент | default | hover | active/pressed | focus-visible | selected | disabled |
|---|---|---|---|---|---|---|
| rail-btn | transparent / muted | `--state-hover` | `--state-active` | ring | `--accent-soft` + indicator | opacity .45 |
| guild-item | `--surface-1` | `--surface-3`, r14 | `--state-active` | ring | `--accent`, r14 | opacity .45 |
| channel-item | transparent | `--state-hover` | `--state-active` | ring | `--state-selected` | opacity .45 |
| message row | transparent | `--row-hover-bg` | — | ring на `#chat-list` | `--row-selected-bg` | — |
| member row | transparent | `--state-hover` | `--state-active` | ring | `--state-selected` | opacity .45 |
| composer | `--surface-sunken` | — | — | focus-within ring | — | `aria-disabled` |
| tab (emoji) | transparent | `--state-hover` | `--state-active` | ring | `--state-selected-strong` + полоса | opacity .4 (locked) |
| emoji cell | transparent | `--state-hover` | `--state-active` | ring | `--accent-soft` + ring | opacity .4 |
| switch | трек 12% чёрного | border strong | — | ring | `--accent` | opacity .45 |
| icon-btn | transparent | `--state-hover` | `--state-active` | ring | `--state-selected` | opacity .45 |
| menu item | transparent | `--state-hover` | `--state-active` | ring | `--state-selected-strong` | opacity .45 |
| search | `--surface-input` | `--surface-3` | — | ring | — | opacity .45 |

Правила: focus-visible виден **всегда** (не удалять `outline` без замены); disabled не
убирает элемент из DOM (иначе ломается `aria`), а получает `aria-disabled`; selected
дублируется `aria-current`/`aria-selected`; hover не обязателен для тач-сценария.

Пустые состояния: список пуст → `.empty-state` **оверлеем в родителе списка** (`.chat-main`,
`#screen-members`), но не ребёнком `#chat-list` (там допустим только sizer `[F]`); sizer
остаётся, `#chat-list.children.length === 1`. Загрузка → skeleton-строки
`data-state="placeholder"` (без детей) + `aria-busy="true"` на списке. Ошибка → `.error-state`
оверлеем там же и `aria-live="assertive"` на контейнере ошибки.

## 9. Экраны

Shell общий для всех экранов: titlebar 32 + `#guild-rail` 64 + `#channel-tree` 240 +
контент + `#member-panel` 240 (только chat). Переключение — `.rail-btn[data-screen]`,
одновременно: `.screen.active`, `aria-hidden` на неактивных, `body[data-active-screen]`
(`data-screen` остаётся только на `.rail-btn` — см. `DOM-CONTRACT.md` §2).
Фокус после переключения — на заголовок экрана (`tabindex="-1"` у `.screen-title`? нет:
`#chat-header h1` / `.screen` c `tabindex="-1"` `[A]`).

### 9.1 Чат (`#screen-chat`)

Структура: `#chat-header` (49) → `.chat-main` (`#chat-list` 1fr + `.chat-notice` 24 +
`#composer` 56) → `#member-panel` (240, `role="complementary"`).

Поведение:
- Старт: список прокручен в конец (RW-38) `[F]`; окно — переменная высота D-04
  (26/48/+22), overscan 2–8.
- Скролл: колесо/тач над `#chat-list`; клавиши Home/End/PageUp/PageDown при фокусе на
  списке; `role="list"`, `aria-label="Сообщения"`, `tabindex="0"`.
- Hover: подсветка строки; клик по строке — выделение (для контекстного меню в Phase 3).
- Поиск в шапке фильтрует **загруженные** строки (≤ 500, PageStore) без сети; счётчик
  `aria-live="polite"` («12 из 240»); пустой результат → `.empty-state`.
- Панель участников: заголовок «Участники — 735» `[L] online_count 663…735` и превью
  до 50 строк; строки 42; заголовки групп «В СЕТИ — N»; «показать всех» →
  экран members.
- Композер: см. §7.7; отправка выключена, `Enter` не создаёт сообщений.

### 9.2 Эмодзи (`#screen-emoji`)

Структура: центрированная карточка 498×440 (max `calc(100vw - 48px)`), шапка с поиском и
close, rail 10 вкладок (`role="tablist"`, `aria-orientation="vertical"`), сетка
`role="grid"`, футер-предпросмотр 48 (эмодзи 24 + имя 14/18 `--text-muted`).

Поведение:
- 10 категорий `[F]` (`EMOJI_CATEGORIES = 10`): по умолчанию первая; клик по вкладке
  перерисовывает сетку (≤ 120 ячеек `[F]`), `aria-selected` ровно у одной.
- Поиск: фильтр по текущей категории; пустой запрос возвращает полный набор; повторный
  рендер не дублирует ячейки `[F]`; пусто → `.empty-state`.
- Клик по ячейке вставляет эмодзи в композер (если открыт из чата) и закрывает пикер;
  Esc — закрыть; Ctrl+E — открыть/закрыть.
- Клавиатура сетки: стрелки/Home/End/PageUp/PageDown, Enter/Space — выбор.
- Отклонение `[A]`: в референсе 3 контент-вкладки (Гифки/Стикеры/Эмодзи) + 10 категорий;
  в спайке вкладки = 10 категорий (совместимость RW-38). Гифки/стикеры — вне Phase 2.

### 9.3 Участники (`#screen-members`)

Структура: `.members-toolbar` 48 (поиск 32 + счётчик `aria-live`) → `#member-list`
(1fr, `role="list"`, `tabindex="0"`), строки 42 `[L]` (`--row-h-member`, D-04), виртуализация.

Поведение:
- Окно строк — 42 (`--row-h-member`, D-04); старт с 0; overscan 2–8; sizer = `total × 42`.
- Поиск по **загруженным** страницам (≤ 500 строк), счётчик показывает найденное;
  пусто → `.empty-state`.
- Клик/Enter по строке — поповер профиля (§7.14) с данными фикстуры (имя, статус, роли,
  активность); без сети.
- Групповые заголовки в виртуализованном списке отсутствуют (ограничение §7.6):
  статус и роль видны в строке; агрегаты — в тулбаре.

### 9.4 Настройки (`#screen-settings`)

Структура: оверлей-панель (min 960×600, центр, radius 8, bg `--surface-2`,
`--shadow-elevation-high`): шапка 56 с `avatar 32 + имя + «Редактировать профиль»` и
close 20; ниже `#settings-nav` 232 (поиск 32 + список разделов 34) и `#settings`
(контент, max-width 720, padding 24/16, скролл).

Поведение:
- Навигация: 6 разделов `[A]` (Учётная запись, Голос и видео, Внешний вид, Уведомления,
  Язык и время, О программе) — кнопки, `aria-current="true"` у активного; контент
  переключается без перезагрузки; `Ctrl+,` открывает, `Esc` закрывает (возврат в чат).
- Контент Phase 2 — 3 группы `[F]`: «Голос и видео», «Интерфейс», «Уведомления»,
  всего 38 переключателей `[F]`; строка 48: название + подсказка + switch справа.
  Навигация не скрывает группы: выбор раздела подсвечивает пункт и скроллит к
  соответствующей группе (все 3 группы всегда в DOM — иначе ломается счётчик `h3`).
- Значения меняются только локально (в памяти), без записи в файлы и сеть; при перезапуске
  сбрасываются (осознанно) `[A]`.
- Поиск по разделам фильтрует nav; пусто → `.empty-state`.
- «О программе»: версия, лицензия MIT OR Apache-2.0, дисклеймер о неофициальности,
  отсутствие телеметрии — текстом (без логотипов).

## 10. Доступность

- **Landmarks**: `.topbar[role=banner]`, `#guild-rail[role=navigation]`,
  `#channel-tree[role=navigation]`, `main[role=main]` (неявно), `#member-panel
  [role=complementary]`, каждый `.screen[role=region]` c `aria-label` и `aria-hidden`.
- **Заголовки**: один `h1` на экран; `#chat-header h1` — имя канала; `.screen-title`
  (titlebar) — `role="heading" aria-level="1"` с текстом экрана `[F]` (текст менять нельзя).
- **Списки**: `role="list"` + `role="listitem"`; виртуализация не меняет роли — в DOM
  только видимое окно, `aria-setsize`/`aria-posinset` на строках `[A]` (рекомендуется).
- **Tabs/grid**: `role="tablist"/"tab"` c `aria-selected`, `aria-controls`;
  `role="grid"/"row"/"gridcell"` c `aria-rowcount/colcount`, roving tabindex.
- **Focus**: `:focus-visible { outline: 2px solid var(--focus-ring); outline-offset: 2px }`;
  фокус не теряется при смене экрана (переводится на заголовок/первый интерактив);
  ловушка фокуса — в модалке настроек и поповере профиля; `Escape` закрывает.
- **Клавиатура** (полная карта):

| Клавиши | Действие |
|---|---|
| `Tab` / `Shift+Tab` | порядок: rail → tree → header → список → composer → member panel |
| `Alt+1…4` | экраны chat/emoji/members/settings `[A]` |
| `Ctrl+E` | эмодзи-пикер (toggle) |
| `Ctrl+,` | настройки |
| `Ctrl+F`, `/` | фокус в поиск сообщений |
| `Escape` | закрыть поповер/модалку/пикер; иначе вернуться в чат |
| `↑/↓` | навигация в меню/сетке/списке (где применимо) |
| `Home/End/PageUp/PageDown` | края/страницы списка и сетки |
| `Enter/Space` | активация кнопки/строки/ячейки |

- **Live regions**: `.metrics[aria-live=polite]` (существующий), счётчики поиска и
  количество участников — `aria-live="polite"`, ошибки — `assertive`.
- **Контраст**: §3.4; текст ≥ 4.5, контролы/иконки ≥ 3; проверка — ручная по таблице
  (automatable: axe в e2e — P3-05 `[A]`).
- **Размер цели**: интерактив ≥ 24×24 (rail 40, icon-btn 32, ячейка 44, строка 28 —
  строка канала 28 по высоте, но зона клика — вся ширина × 28 ≥ 24) `[A]`.
- **Reduced motion**: токены обнуляют длительности; никаких авто-анимаций длиннее 200ms;
  skeleton-мерцание отключено.
- **Масштаб**: 200% zoom — flex/grid-раскладка, tree/panel скрываются по §5.1,
  горизонтальный скролл только в таблицах настроек (none в Phase 2).
- **Скринридер**: виртуализованный список не «прыгает» — при скролле фокус остаётся на
  контейнере; строки не пересоздаются при озвучивании (W39d переиспользует узлы `[F]`).

## 11. Интеграция с W39c/W39d

1. W39c копирует `tokens.css` → `ui/components/tokens.css` (md5 фиксируется).
2. `ui/style.css` = shell + раскладка; `ui/components/*.css` = компоненты §7
   (по одному файлу: `rail.css`, `tree.css`, `chat.css`, `emoji.css`, `members.css`,
   `settings.css`, `primitives.css`). Порядок подключения: tokens → primitives → shell →
   экраны.
3. W39d не пишет стили инлайном, кроме `top/height` виртуализации `[F]`.
4. Классы/ARIA — строго по `DOM-CONTRACT.md`; расхождение = правка контракта, а не кода.

## 12. Иконки (собственные inline-SVG)

Сетка 24×24, `viewBox="0 0 24 24"`, `fill="none"`, `stroke="currentColor"`,
`stroke-width="2"`, `stroke-linecap="round"`, `stroke-linejoin="round"`,
`aria-hidden="true"`, размер 20 (16 для мелких). Лицензия — код проекта
(MIT OR Apache-2.0), автор — проект DL; сторонние наборы не используются.

| Иконка | Назначение | path (d) |
|---|---|---|
| `hash` | текстовый канал | `M9 3 7 21M17 3l-2 18M3.5 9h17M3 15h17` |
| `volume` | голосовой канал | `M4 9v6h4l5 4V5L8 9H4z` + `M16.5 8.5a5 5 0 0 1 0 7` |
| `mic` | микрофон | `M12 3a3 3 0 0 1 3 3v6a3 3 0 0 1-6 0V6a3 3 0 0 1 3-3z` + `M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6` |
| `headphones` | звук | `M4 14v-2a8 8 0 0 1 16 0v2M4 14a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H3v-6zM20 14a2 2 0 0 0-2 2v2a2 2 0 0 0 2 2h1v-6z` |
| `gear` | настройки | `M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z` + `M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z` |
| `search` | поиск | `M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM16.2 16.2 21 21` |
| `users` | участники | `M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2` + `M9 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8z` + `M22 21v-2a4 4 0 0 0-3-3.9M16 3.1A4 4 0 0 1 19 7a4 4 0 0 1-3 3.9` |
| `pin` | закреплённые | `M9 3h6l-1 6 3 3v2H7v-2l3-3-1-6zM12 14v7` |
| `thread` | ветки | `M4 5h16v10H9l-5 5V5z` |
| `chevron-down` | раскрытие | `m6 9 6 6 6-6` |
| `plus` | добавить | `M12 5v14M5 12h14` |
| `attach` | вложение | `M21 11.5 12.5 20a5 5 0 0 1-7-7l8-8a3.5 3.5 0 0 1 5 5l-8 8a2 2 0 0 1-3-3l7-7` |
| `smile` | эмодзи | `M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z` + `M8.5 14a4.5 4.5 0 0 0 7 0M9 9.5v.01M15 9.5v.01` |
| `gift` | подарок | `M20 12v8H4v-8M2 8h20v4H2zM12 8v12` + `M12 8S8 8 8 5.5 12 3 12 8zM12 8s4 0 4-2.5S12 3 12 8z` |
| `close` | закрыть | `M6 6l12 12M18 6 6 18` |
| `check` | выбрано | `m4 12 5 5L20 6` |
| `warning` | предупреждение | `M12 3 2 21h20L12 3zM12 9v6M12 18v.01` |
| `info` | инфо | `M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5v.01` |
| `more` | контекст | `M5 12h.01M12 12h.01M19 12h.01` |
| `image` | изображение | `M3 5h18v14H3zM3 16l5-5 4 4 3-3 6 6` + `M9 10a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z` |

Правила: иконка никогда не единственный носитель смысла (рядом `aria-label`), размер
шрифта/иконки не смешивать в одном контроле, цвет — только `currentColor`.

## 13. Отклонения от референса и assumption-лист

| # | Отклонение / допущение | Причина | Статус |
|---|---|---|---|
| A-01 | Строка сообщения — переменная высота **26 / 48 / +22** (D-04), константного трека нет | RE-оракул `layout.json`: continuation 26, group_start 48, line_extra 22; переменная высота требует prefix-sum | ревизия D-04: реализация и обновление ожиданий — W39d |
| A-02 | Строка участника **42** и в панели, и в виртуализации (D-04) | RE: `member_row = 42`; прежний трек 40 — приближение RW-38 | ревизия D-04: W39d |
| A-03 | Дерево **240** (RE 311), панель **240** (RE 256) | ТЗ/RUNBOOK «~240»; RE-значения включаются на ≥1600px | `[A]` |
| A-04 | Вкладки эмодзи = 10 категорий, а не 3 контент-вкладки | RW-38 `EMOJI_CATEGORIES=10`; Гифки/Стикеры вне Phase 2 | `[F]` |
| A-05 | Нет групповых заголовков внутри виртуализованного списка участников | RW-38: `sizer.children.length === rows.length` | `[F]` |
| A-06 | Нет разделителей дней внутри `#chat-list` | тот же инвариант sizer | `[F]` |
| A-07 | 38 переключателей в 3 группах (не 63 раздела/38 в «Голос и видео») | RW-38 фиксирует числа и заголовки групп | `[F]` |
| A-08 | Настройки — оверлей 960×600, не 1398×961 | окно спайка меньше; max-width контента 720 | `[A]` |
| A-09 | Отправка сообщений и сеть отсутствуют | правила волны (без сети/live) | осознанно |
| A-10 | Профиль — данные фикстуры, без груза изображений | нет сети; аватары — инициалы | `[A]` |
| A-11 | Название канала `ITSKIDON PC BOOST` в фикстурах | санитизированные данные RE | данные, не бренд |
| A-12 | `--text-faint` ограничен тёмными фонами | контраст 4.04 на `--surface-2` < AA | осознанно |

## 14. Чек-лист визуальной сверки (Phase 2)

1. Рейл 64, иконки 40, активная — радиус 14 + индикатор.
2. Дерево 240 (на ≥1600 — 311), строка 28, selected `#2c2c30`, категории 12px uppercase.
3. Шапка чата 49, заголовок 16/22 600, поиск-pill 32 справа.
4. Composer 56, поле `#222327`, radius 8, `+` слева, emoji/gift справа.
5. Панель участников 240 (≥1600 — 256), строки 42, группы uppercase.
6. Чат: фон `#1a1a1e`, гаттер 64, аватар 40, время 12px `--text-muted`,
   hover `#202024`; высота строки 48 (начало) / 26 (продолжение) + 22 за перенос (D-04);
   на ≥10 строках нет «белых полос» и прыжков при скролле.
7. Эмодзи: карточка 498×440, rail 48, ячейки 44, gap 4, 9 колонок, футер-предпросмотр.
8. Настройки: оверлей, nav 232, строки 48, switch 40×24, 38 переключателей.
9. Все шрифты — системные; emoji — системные; иконки — свои SVG; бренд `DL`;
   ни одного логотипа/названия Discord в UI.
10. Контраст по §3.4; focus-visible виден на всех интерактивных элементах;
    reduced-motion отключает анимации; Tab-порядок по §10.

## 15. Открытые вопросы к PM (до W39c)

1. **Харнесс RW-38**: новый shell (`#guild-rail`, `#channel-tree`, `#chat-header`,
   `#composer`, `#member-panel`, `#user-bar`) требует синхронной правки
   `tests/support/gui-dom-harness.mjs` (подробно — `DOM-CONTRACT.md` §9). Файл вне матрицы
   W39b/W39c: нужно решение — разрешить аддитивную правку или принять план B.
2. **240 vs 311/256**: принять дефолт 240 (ТЗ/RUNBOOK) или RE-значения 311/256?
   В токенах поддержаны оба (переключение на ≥1600px).
3. **D-04 принят** (2026-10-23): высота строки переменная (26/48/+22), участник 42;
   реализация в виртуализаторе и обновление ожиданий RW-38 — W39d, мутация
   «вернуть константу» — QA-1.
