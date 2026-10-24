# RW-39 / W39g — SPEC P3-02 «чат-вью»: дизайн по референсам

Статус: design gate, **итерация 2** (сверка с RE-контрактом W39f). Дата: 2026-10-23. PM: DeepSeek.
Вход: `HANDOFF.md` §1/§4, `REFERENCES.md` §1–§7, `ref-manifest.json`, `SPEC.md` §7/§9,
`tokens.css`, `layout.json`, `DOM-CONTRACT.md`, **`RE-EXTRA.md` / `re-extra.json`** (D-14),
`DECISIONS.md` D-13/D-14.

Изменения итерации 2 помечены в примечании `← RE-D14`: `[A]` заменён на `[D]` (факт из дампа,
`RE-EXTRA.md`) или `[M]` там, где RE-EXTRA даёт замер. Оставшиеся `[A]` — пробелы, перечисленные
в §13. Ничего не додумано: где контракта нет, стоит `[A]` и gap.

Источник истины по **значениям** — `tokens.css` (токены) и `layout.json` (геометрия).
Эта спека не вводит второй набор чисел: ниже даются ссылки на токены/ключи, а литералы
приводятся только там, где токена ещё нет (тогда рядом стоит код и обоснование).
Ассеты (`/home/sanya/re-deep`) не копируются: переносятся только числа.

## 0. Коды и ключевые числа

Коды: `[M]` измерено · `[D]` факт из дампа (`RE-EXTRA.md`, W39f) · `[T]` токен дампа ·
`[L]` RE-контракт RW-37 · `[A]` assumption · `[F]` frozen RW-38.

Ключевые числа карточки — **единственное место**, где они выписаны литералами; все
разделы ниже ссылаются на эти строки и на токены.

| # | Величина | Значение | Код | Источник | Токен / ключ |
|:-|:-|:-|:-|:-|:-|
| K-01 | высота начала группы сообщений | 48 (CSS `min-height` 44) | `[L]`/`[D]` | RW-37 `row_heights_detail.message_cozy_group_start`; `DOM-CONTRACT` §9.5; RE V-12 | `--row-h-group-start` |
| K-02 | высота продолжения группы | 26 | `[L]` | RW-37 `row_heights.message_cozy`; `DOM-CONTRACT` §9.5 | `--row-h-continuation` |
| K-03 | добавка за перенос строки | +22 | `[L]` | RW-37 `message_extra_line`; `tokens.json --chat-markup-line-height` | `--row-line-extra` |
| K-04 | гаттер сообщения cozy / compact | 64 / 80 | `[L]`/`[D]` | RW-37 `message_indent_cozy/compact`; RE V-12 (`64 = 12+40+12`), V-13 (`padding-inline-start:80`) | `--message-gutter` / `--message-gutter-compact` |
| K-05 | аватар сообщения / участника | 40 / 32 | `[L]`/`[D]` | RW-37 `message_avatar=40`, `member_avatar=32`; RE V-12, M-07; D-13 | `--avatar-lg` / `--avatar-md` |
| K-06 | шаг виртуализации строк | 44 | `[F]` | RW-38 `CHAT_ROW_HEIGHT=44`; **не дизайн-константа** | `--row-message-track` |
| K-07 | синяя полоса «20+ новых» | 34 border-box (контент 32) | `[M]`/`[D]` | D-13; RE V-05 (высота 32, brand-500); R-04 (y 81.6…114.4) | `--unread-banner-h` |
| K-08 | шаг строки участника / высота | 43 / 42 | `[M]` / `[L]` | R-08 медиана 43.2 (`n=16`, JPEG ±1); RW-37 `member=42` | `--row-member-pitch` / `--row-h-member` |
| K-09 | попаут профиля | 268.8 × 408 | `[M]` | R-06 bbox при (22.4, 608.8); D-13 | литерал, §8 |
| K-10 | лимит текста сообщения | 2000 (Nitro 4000) | `[D]` | RE C-04 (`nY=2e3`, `nK=4e3`) | литерал, §4 |
| K-11 | порог счётчика символов | остаток ≤ 200 (`max/10`) | `[D]` | RE C-05 | литерал, §4 |
| K-12 | вложений на сообщение | ≤ 10 | `[D]` | RE C-11 (`XgB=10`) | литерал, §4 |
| K-13 | блок заголовка группы участников | 42 (текстовая строка 24) | `[M]` | D-13; RE M-08 (`padding-block:16px 4px`) | `--row-member-group` |

### 0.1 Решения PM D-13 (приоритет над таблицами ниже)

1. Unread-полоса «20+ новых» — **34** border-box (контент 32), `--unread-banner-h`;
   `.chat-notice`-подсказка остаётся **24** — это разные элементы.
2. Заголовок группы участников — блок **42** (`--row-member-group`), текстовая строка 24.
3. В панели чата тулбара нет; тулбар 48 + поиск 32 — только на экране `members`.
4. Profile-popout — **268.8×408**.
5. Поиск в шапке: контейнер 32, поле 244×28.
6. Аватар сообщения **40**, участника **32**.
7. `--row-message-track` — `[F]` шаг виртуализации, не дизайн-высота.

## 1. Сообщение (`.row`)

Классы/ARIA — `DOM-CONTRACT.md` §4.3, §7.1. Высота строки **переменная** (D-04):
фиксированная константа 44 в вёрстке запрещена, она остаётся только шагом
виртуализации (K-06).

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| MSG-01 | плотность cozy, гаттер | `--message-gutter` | `[L]` | RW-37 `message_indent_cozy`; K-04 | `:root` без `data-density` |
| MSG-02 | плотность compact, гаттер | `--message-gutter-compact` | `[L]`/`[D]` | RW-37 `message_indent_compact`; RE V-13 (`padding-inline-start:5rem`) | `data-density="compact"` на `<html>` (не на `body`) ← RE-D14 |
| MSG-03 | compact: автор и отступы | автор inline, `padding-block: 2px`, аватар `1em` inline (`top: .18em`) | `[D]` | RE V-13 (CSS `compact_c19a55`) | в compact `.author` остаётся в DOM (2 ребёнка `[F]`); время — в gutter ← RE-D14 |
| MSG-04 | начало группы | `--row-h-group-start` | `[L]` | K-01; `layout.json rows.message_group_start` | `data-group="start"`; CSS `min-height: 44` (RE V-12) не отменяет расчёт 48 ← RE-D14 |
| MSG-05 | продолжение группы | `--row-h-continuation` | `[L]` | K-02; `layout.json rows.message_continuation` | `data-group="continuation"` |
| MSG-06 | перенос строки | `--row-line-extra` | `[L]`/`[D]` | K-03; RE V-12 (`min-height: 22`, `padding-block: 2`) | `height = base + extra·(lines−1)` ← RE-D14 |
| MSG-07 | строка текста | `--line-height-message` | `[L]`/`[T]` | `layout.json rows.message_line`; RE V-12 (`1.375rem`) | сетка 2×22 |
| MSG-08 | шаг виртуализации | `--row-message-track` | `[F]` | K-06 | **не** высота строки; менять только с тестами |
| MSG-09 | аватар | `--avatar-lg`, `inset-inline-start: 12px` | `[L]`/`[D]` | K-05; RE V-12 (avatar 40, `inset-inline-start:12`) | контент `margin-inline-start: 64` = 12+40+12; прежние 16 заменены на 12 ← RE-D14 |
| MSG-10 | гаттер контента | `--message-gutter` | `[L]`/`[D]` | K-04; RE V-12 (`calc(40+12+12)`) | `grid-template-columns: var(--message-gutter) 1fr` ← RE-D14 |
| MSG-11 | имя автора | `--font-size-body` / `--line-height-body`, 600 | `[M]`/`[D]` | `SPEC.md` §7.5; RE V-10 (`h2.header`) | цвет `--text-strong`; заголовок `h2` внутри `<article>` ← RE-D14 |
| MSG-12 | время | `--font-size-small` / `--line-height-small`; формат `DD.MM.YYYY, HH:mm` | `[M]`/`[D]` | R-01 (12 px); RE V-11 (пример «12.10.2026, 12:35», `<i> — </i>`, скрытая полная дата) | цвет `--text-muted`; `<time datetime>` ← RE-D14 |
| MSG-13 | бейдж `BOT` | `--badge-h`, `--badge-pad-x`, `--badge-radius` | `[T]`/`[A]` | `SPEC.md` §7.12 | bg `--accent`, текст `--text-on-accent` |
| MSG-14 | текст сообщения | `--font-size-body` / `--line-height-body` | `[M]`/`[D]` | R-02 (`--text` #efeff1); RE V-09 (`#message-content-<id>`) | ссылки `--text-link`, упоминание `--text-accent` на `--accent-soft` ← RE-D14 |
| MSG-15 | `(изменено)` | `--font-size-small`, `--text-muted` | `[A]` | RE-EXTRA не содержит факта об edited-маркере | суффикс после текста, в `aria-label` строки |
| MSG-16 | hover строки | `--row-hover-bg` | `[M]` | R-01/R-02 (`#202024`); `tokens.css` §4 | hover-панель действий появляется вместе с фоном |
| MSG-17 | selected / меню-контекст | `--row-selected-bg` + полоса 2 px `--accent` | `[A]` | `SPEC.md` §7.5 | полоса слева, `padding` не сдвигается |
| MSG-18 | упоминание (mention) | фон `--accent-soft`, полоса 2 px `--warning` | `[A]` | `SPEC.md` §7.5; `--warning` `[T]` | текст упоминания `--text-accent` |
| MSG-19 | reply-quote: полоса | 2 px `--accent`, radius 4 (слева) | `[A]` | reply в дампе не снят (RE G-07, §13) | `--radius-xs` |
| MSG-20 | reply-quote: аватар | `--avatar-xs` | `[A]` | выбор по размеру строки 22 | inline перед ником |
| MSG-21 | reply-quote: ник / текст | `--font-size-small` / `--line-height-small` | `[A]` | типографическая шкала `tokens.css` §5 | ник `--text-subtle`, текст `--text-muted`, одна строка + ellipsis |
| MSG-22 | reply-quote: отступ | `--space-4` сверху, `--space-2` между строкой и текстом | `[A]` | шаг 4 из `tokens.css` §6 | высота quote 22, входит в `+22` (MSG-06) |
| MSG-23 | reactions-row: пилюля | высота 24, radius `--radius-sm`, padding `0 8px` | `[A]` | реакции в дампе не сняты (RE §7) | emoji 16, счётчик `--font-size-small` |
| MSG-24 | reactions: default | bg `--state-hover`, border `--border-subtle` | `[A]` | `tokens.css` §4 | `--border` |
| MSG-25 | reactions: своя реакция (me) | bg `--accent-soft`, border `--accent` | `[A]` | `SPEC.md` §7.5 (mention-логика) | `aria-pressed="true"` |
| MSG-26 | reactions: шаг | `--space-2` между пилюлями, `--space-2` до текста | `[A]` | шаг 4 | ряд переносится, не скроллится |
| MSG-27 | hover-панель: состав | 4 реакции + 11 действий = 15 пунктов | `[M]`/`[D]` | R-16 `01-msg-menu-items.json` (X-01); RE V-14 (кнопки действий — отдельный контейнер) | тексты пунктов — §13, G-02 ← RE-D14 |
| MSG-28 | hover-панель: геометрия | пункт `--row-menu-item`, контейнер `--menu-min-w`, padding `--menu-pad`, radius `--menu-radius`, `--shadow-popover` | `[A]` | `SPEC.md` §7.14; RE геометрии меню не даёт (RE G-01) | 32 — assumption, не измерение |
| MSG-29 | hover-панель: позиция | `top: 4px; right: 16px` (внутри строки), z `--z-popover` | `[A]` | R-16 PNG: панель прижата к правому верхнему углу строки | не перекрывает текст: строка получает `padding-inline-end` |
| MSG-30 | hover-панель: кнопка | 32×32, иконка `--icon-size`, radius 4 | `[A]` | `SPEC.md` §7.11 | «ещё» — 15-й пункт списком |
| MSG-31 | группировка | новое сообщение = `start`, если сменился автор или пауза > 7 мин | `[A]` | RE-EXTRA правила группировки не даёт (V-09 — только DOM-структура) | иначе `continuation`; влияет на K-01/K-02 |
| MSG-32 | ARIA строки | `role="listitem"` + `aria-label`, `aria-setsize`/`aria-posinset`; в дампе вложенный `role="article"` | `[F]`/`[D]` | `DOM-CONTRACT` §5.3, §7.1; RE V-10 | `[F]`-структура (2 ребёнка) приоритетна: `<article>` не добавляем, вопрос в P3-03 ← RE-D14 |
| MSG-33 | клик по аватару | открыть профиль-попаут (§8) | `[D]` | RE V-15 (`.avatar…clickable`, `pointer-events` по классу) | курсор/`pointer-events` только при `clickable` ← RE-D14 |
| MSG-34 | id строки | `chat-messages-<channel>-<id>`, в результатах поиска `search-result-<id>` | `[D]`/`[A]` | RE V-16 (снят только `search-result-`); схема канального id — `[A]` | использовать для `aria-labelledby`/перехода ← RE-D14 |
| MSG-35 | контейнер скролла | `scroller`/`scrollerInner` + виртуализация окна | `[D]`/`[F]` | RE V-20; `DOM-CONTRACT` §6.1 | подтверждает `[F]`-модель окна ← RE-D14 |

## 1.1 Чат-вью: факты RE-EXTRA (V-01…V-20)

| id | элемент | значение/правило | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| VW-01 | шапка чата | иконка типа канала + имя; справа 4 icon-btn (ветки, колокол, закреплённые, участники) + поиск | `[M]` | RE V-01 (PNG, кроп y 32…85) | наша шапка (SPEC §7.4) даёт 3 кнопки + поиск → добавить «колокол» ← RE-D14 |
| VW-02 | поиск в шапке | поле 244×28, placeholder «Искать «{guild}»» | `[D]` | RE V-02 (`20a-searchbar.json` `inputs[0].aria`) | D-13: контейнер-pill 32, поле внутри 244×28 ← RE-D14 |
| VW-03 | топик канала | в снятом канале отсутствует | `[M]`/`[A]` | RE V-03 | вариант «имя + тема» не подтверждён, не рисуем ← RE-D14 |
| VW-04 | unread-divider (линия) | высота 0, `border-top: thin solid --border-subtle`, по центру | `[D]` | RE V-04 (CSS `__908e2`) | цвет линии в дампе — токен notification; у нас `[A]` → `--danger` ← RE-D14 |
| VW-05 | плашка «Новые сообщения» | высота 32, `--accent`, текст `--text-on-accent`, radius `0 0 8px 8px`, padding-inline 16, `top: 0`, z 3 | `[D]`/`[M]` | RE V-05 (CSS `newMessagesBar`); D-13 | border-box 34 → `--unread-banner-h` ← RE-D14 |
| VW-06 | плашка «Перейти к последним» | `bottom: 28`, padding-inline 12, padding-bottom 8 | `[D]` | RE V-06 (`jumpToPresentBar`) | плавающая, над композером ← RE-D14 |
| VW-07 | тексты | «Новые сообщения», «Перейти к последним сообщениям» | `[D]` | RE V-07 (`ru-catalog` `q7hm3m`, `Sj83P7`) | в R-04 текст был «20+ новых сообщений» `[M]` — берём R-04 для счётчика ← RE-D14 |
| VW-08 | divider: контент | 12 px semibold, line-height 13, padding `2px 4px`, radius 8, фон чата; endCap 13, caps 10 bold uppercase | `[D]` | RE V-08 | один компонент для даты и непрочитанного ← RE-D14 |
| VW-09 | cozy DOM | `li.container > .searchResult > .message > .wrapper(cozy,zalgo)[role=article] > .contents > #message-content-<id> + #message-accessories-<id>` | `[D]` | RE V-09 (`ui8/20-search-results.html`) | в спайке структура `[F]` проще; accessories — отдельный контейнер ← RE-D14 |
| VW-10 | ARIA сообщения | `role="article"`, `aria-labelledby="message-content-<id> message-accessories-<id>"`, `h2.header` c `aria-labelledby="message-username-<id>"`, `time#message-timestamp-<id>[datetime]` | `[D]` | RE V-10 | внедрение упирается в `[F]` (MSG-32) ← RE-D14 |
| VW-11 | формат времени | `DD.MM.YYYY, HH:mm`, перед ним `<i> — </i>`, рядом скрытая полная дата | `[D]` | RE V-11 | см. MSG-12 ← RE-D14 |
| VW-12 | cozy-метрики | аватар 40 при `inset-inline-start: 12`; контент 64; wrapper `padding-inline-start: 64`, `padding-block: 2`, `min-height: 22`; header `margin-inline-start: -72`; groupStart `min-height: 44` | `[D]`/`[T]` | RE V-12 (CSS `c19a55`/`5126c`) | наши 48/26 — расчёт по D-04, 44/22 — CSS-минимумы ← RE-D14 |
| VW-13 | compact-метрики | аватар `1em` inline (`top: .18em`), `padding-inline-start: 80`, `padding-block: 2`, groupStart `min-height: 22`, время в gutter | `[D]`/`[T]` | RE V-13 (CSS `compact_c19a55`) | время в gutter тонируется `[A]` → `--text-faint` ← RE-D14 |
| VW-14 | accessories и кнопки | embeds/вложения в `#message-accessories-<id>.container`; кнопки действий `.buttonsContainer` (в результатах — «Перейти») | `[D]` | RE V-14 | см. §2 EMB-13, §6 SRCH-19 ← RE-D14 |
| VW-15 | клик по аватару | профиль | `[D]` | RE V-15 | см. MSG-33 ← RE-D14 |
| VW-16 | id строки | `search-result-<id>` в результатах; канальный `chat-messages-<channel>-<id>` не снят | `[D]`/`[A]` | RE V-16 | см. MSG-34 ← RE-D14 |
| VW-17 | notice-полоса | `.channelNotice`: padding `6px 12px`, `border-bottom: 1px solid --border-subtle` | `[D]` | RE V-17 (CSS `e2d7b8`) | это **не** 24 px из SPEC; блок 24 остаётся нашим решением (D-13) ← RE-D14 |
| VW-18 | пруф unread/new-bar | ни unread-divider, ни new-messages bar в снятом канале нет; контракт только из CSS | `[M]`/`[D]` | RE V-18 | пиксельной сверки не будет, проверяем по контракту ← RE-D14 |
| VW-19 | единство divider | дата — без `isUnread`, непрочитанное — с `isUnread` (цвет линии/текста) | `[D]` | RE V-19 | один компонент, два состояния ← RE-D14 |
| VW-20 | виртуализация | `messageListItem` + `scroller` в DOM (только окно) | `[D]`/`[F]` | RE V-20 | совпадает с окном `[F]` ← RE-D14 |

## 2. Embeds и вложения

Геометрия самих карточек в дампе **не снята** (RE G-01/G-07): числа раздела `[A]`, кроме
цвета/радиуса/паддингов из токенов и контейнера accessories (VW-14). Обоснование выбора:
колонка текста 22 px, карточка не должна выходить за `1fr` и не уже 320 px на узком shell.

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| EMB-01 | link-embed: полоса слева | 4 px `--accent`, radius `--radius-xs` | `[A]` | выбор по `SPEC.md` §7.5 | единственный акцентный маркер карточки; RE-EXTRA геометрии не даёт |
| EMB-02 | link-embed: поверхность | `--surface-2`, radius `--radius-sm` | `[A]`/`[T]` | `--surface-2` `[M]` (`#242429`) | hover не меняет bg |
| EMB-03 | link-embed: паддинги | `--space-4` / `--space-5` (8 / 12) | `[A]` | шаг 4, `tokens.css` §6 | между полосой и контентом `--space-5` |
| EMB-04 | link-embed: макс. ширина | 520 | `[A]` | не измерено; 520 = 22 px × 23.6, читаемая ширина | на узком shell `max-width: 100%` |
| EMB-05 | link-embed: заголовок / описание / сайт | `--font-size-body` 600 `--text-strong` / `--font-size-ui` `--text` / `--font-size-small` `--text-muted` | `[A]` | типографическая шкала `tokens.css` §5 | 3 строки максимум у описания + ellipsis |
| EMB-06 | link-embed: провайдер-строка | `--font-size-small` `--text-subtle` | `[A]` | `--text-subtle` `[T]` | над заголовком |
| EMB-07 | image-embed: картинка | `max-width: 400`; `max-height: 300`; radius `--radius-sm` | `[A]` | не измерено; пропорции 4:3, чтобы не ломать строку 22 | картинка не грузится (нет сети) → заглушка `--surface-3` с иконкой 48 |
| EMB-08 | image-embed: подпись | `--font-size-small` `--text-muted` | `[A]` | шкала | под картинкой, `--space-2` |
| EMB-09 | вложение-файл | карточка `--surface-2`, radius `--radius-sm`, padding `--space-6`, иконка 40 `--text-muted` | `[A]`/`[T]` | `SPEC.md` §7.5; RE C-11 (≤ 10 файлов) | 40 = `--avatar-lg` по размеру ← RE-D14 |
| EMB-10 | вложение: имя / размер | `--font-size-body` `--text-link` / `--font-size-small` `--text-muted` | `[A]` | шкала | имя в одну строку + ellipsis |
| EMB-11 | несколько вложений | сетка 2 колонки, gap `--space-2`, максимум 10 | `[D]`/`[A]` | RE C-11 (лимит 10 — `[D]`); сетка — `[A]` | > 4 файлов → «и ещё N» строкой ← RE-D14 |
| EMB-12 | отступ блока | `--space-2` сверху, входит в `+22` (MSG-06) | `[A]` | K-03 | embed занимает целые строки 22 |
| EMB-13 | контейнер accessories | `#message-accessories-<id>`, отдельный от `#message-content-<id>` | `[D]` | RE V-14; VW-09 | id-схема обязательна для `aria-labelledby` (VW-10) ← RE-D14 |

## 3. Day divider, unread divider, notice

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| DIV-01 | плашка «новые сообщения»: высота | `--unread-banner-h` (34 border-box, контент 32) | `[M]`/`[D]` | K-07; D-13; RE V-05 | `layout.json screens.chat.blocks.chat-unread-divider` ← RE-D14 |
| DIV-02 | плашка: фон | `--accent` | `[M]`/`[D]` | R-04 `#5865f2`; RE V-05 (brand-500) | текст `--text-on-accent` ← RE-D14 |
| DIV-03 | плашка: текст и форма | «20+ новых сообщений», radius `0 0 8px 8px`, padding-inline 16, `top: 0`, z 3 | `[M]`/`[D]` | R-04 (текст), RE V-05/V-08 (форма) | `role="separator"` + `aria-label` ← RE-D14 |
| DIV-04 | неразрывная линия непрочитанного | `border-top: thin solid --border-subtle`, высота 0, по центру | `[D]` | RE V-04 | цвет в дампе — notification-токен; у нас `[A]` → `--danger` (см. VW-04) ← RE-D14 |
| DIV-05 | day divider: блок | 24 | `[A]` | RE V-08 даёт контент, не блок; 24 = `--row-group-title` | блок выведен из контента 13 + padding 2×2 + запас ← RE-D14 |
| DIV-06 | day divider: линия | 1 px `--divider` слева и справа от подписи | `[A]` | `--divider` `[A]` 6 % белого | `flex`, `gap: --space-4` |
| DIV-07 | divider: контент | 12/13 semibold, padding `2px 4px`, radius 8, фон чата, caps 10 bold uppercase | `[D]` | RE V-08 (`content`, `endCap`) | заменяет прежние 12/16 ← RE-D14 |
| DIV-08 | notice-полоса: блок | 24 | `[A]` (решение D-13) | `SPEC.md` §7.7, `DOM-CONTRACT` §4.3; RE V-17 (внутренние 6/12) | **не** равно blue-плашке 34: это разные элементы (D-13) ← RE-D14 |
| DIV-09 | notice: стиль | padding `6px 12px`, `border-bottom: 1px solid --border-subtle`, текст `--font-size-small` `--text-muted` | `[D]` | RE V-17 (CSS `e2d7b8`); `SPEC.md` §7.7 (`role="note"`) | вне `#chat-list` (инвариант §6.1) ← RE-D14 |
| DIV-10 | «Перейти к последним сообщениям» | плашка `bottom: 28`, padding-inline 12, padding-bottom 8, текст `--font-size-ui` | `[D]`/`[A]` | RE V-06/V-07 | кнопка `--btn-height-sm`, `--shadow-popover` ← RE-D14 |
| DIV-11 | один компонент divider | дата — без `isUnread`; непрочитанное — с `isUnread` | `[D]` | RE V-19 | реализовать одним `Divider` с флагом ← RE-D14 |
| DIV-12 | пруф | в снятом канале нет ни плашки, ни divider — только CSS | `[M]` | RE V-18 | сверка по контракту, не по скриншоту ← RE-D14 |

## 4. Композер (`#composer`)

Поведение из RE-EXTRA (`[D]`) — но в спайке отправка выключена (нет сети, `[F]`):
W39h реализует клавиши/счётчик/лимиты как UI-контракт, фактическая отправка не выполняется.

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| CMP-01 | высота контейнера | `--layout-composer-h` | `[L]`/`[T]`/`[D]` | RW-37 `composer_h=56`; RE C-14 (height 56) | `layout.json chat-composer.h`; замер PNG 57 с рамкой `[M]` ← RE-D14 |
| CMP-02 | паддинги | `0 16px 8px`, зазор 6 до края | `[M]` | `layout.json chat-composer.pad`; `cross_check_rw37.new` | 8 и 6 — `[M]` |
| CMP-03 | поле | `--surface-sunken`, radius `--composer-radius`; редактор: `1rem`/`1.375rem`, padding-inline `0 10px` | `[M]`/`[T]`/`[D]` | R-01/R-02 (`#222327`); RE C-14 (CSS) | padding редактора 10 — `[D]`, паддинг контейнера 11/16 — `[M]` ← RE-D14 |
| CMP-04 | поле: рост | `max-height: 50vh` + скролл | `[D]` | RE C-14 (`max-height` токен) | заменяет «max 5 строк» `[A]`; в спайке контейнер 56 фиксирован → до D-04 не растягиваем ← RE-D14 |
| CMP-05 | placeholder (empty) | шаблон «Написать {канал}», `aria-label` с каналом; ЛС — «Сообщение для @name» | `[D]` | RE C-02 (`8lzR/R`), C-03 (`0ZQw/X`) | цвет `--text-muted`; в спайке канал подставляется из фикстуры ← RE-D14 |
| CMP-06 | состояние `typing` | счётчик остатка при `remaining ≤ 200` (`max/10`); тексты «Осталось символов: N» / «Достигнут лимит длины сообщения» / «Сообщение слишком длинное» | `[D]` | RE C-05, C-06 (i18n `qH8uFW`, `tU6YQ7`, `YSRIqa`, `c2Jqed`) | счётчик `--font-size-small` `--text-muted`, при лимите `--danger-text`; прежний порог 1800 неверен ← RE-D14 |
| CMP-07 | состояние `disabled` | в референсе композер disabled: текст «У вас недостаточно прав, чтобы отправлять сообщения на этом канале.», кнопок attach/emoji/gift нет | `[M]`/`[D]` | RE C-01 (PNG-кроп); `SPEC.md` §7.7 | у нас остаётся `aria-disabled="true"` + `--state-disabled-opacity` у кнопок; поле в DOM ← RE-D14 |
| CMP-08 | ошибка (вне Phase 2) | border `--danger`, текст `--danger-text` | `[A]` | `SPEC.md` §7.7 | только стиль, без логики |
| CMP-09 | порядок кнопок | слева attach; справа ряд: **gift → gif → sticker → emoji → apps → submit**, gap `--space-8`, максимум 6 | `[D]` | RE C-09 (JSX композера), C-16 (`showAllButtons`) | в P3-02 реальны emoji (пикер) и gift; отсутствующие не рисуем, допустима замена одной expression-кнопкой (C-16) ← RE-D14 |
| CMP-10 | кнопка: размеры и состояния | 32×32 (в ряду), иконка 20, radius 8; hover `--state-hover`, active `--state-active`, focus ring | `[D]`/`[T]` | RE C-13 (`emojiButton` radius 8, hover-токен, иконка 20); `SPEC.md` §7.11 | hover в дампе — `interactive-background-selected`, у нас `--state-hover` ← RE-D14 |
| CMP-11 | attach affordance | aria «Отправить файл»; файлы из буфера и drag&drop; шорткат `Ctrl+Shift+U` | `[D]` | RE C-10 | в спайке `disabled` + тултип «Недоступно в спайке» `[A]`; paste/drag не реализуем до IO ← RE-D14 |
| CMP-12 | emoji affordance | открывает пикер (Ctrl+E), слой со `inset-inline-start: 86px` | `[D]`/`[F]` | RE C-13, K-02; `SPEC.md` §9.2 | у нас пикер = отдельный экран (совместимость RW-38), позиция 86 — `[D]` для будущего слоя ← RE-D14 |
| CMP-13 | лимит текста | 2000; Nitro 4000; вставка > 4000 предлагает отправить файлом | `[D]` | RE C-04, C-12 (`uploadPromptCharacterCount`) | при 2000 ввод блокируется, счётчик `--danger-text` `[A]`; K-10 ← RE-D14 |
| CMP-14 | focus | ring `--focus-ring` 2 px, offset `--focus-ring-offset`, bg не меняется | `[A]` | `tokens.css` §4 | `focus-within` на контейнере |
| CMP-15 | подсказка для SR | `#composer-hint` (`visually-hidden`) | `[F]` | `DOM-CONTRACT` §4.3 | `aria-describedby` у textarea |
| CMP-16 | отправка с клавиатуры | `Enter` — отправить; `Shift+Enter` — перенос; `Ctrl+Enter` — отправить при выключенном enter-to-submit; `Enter` игнорируется внутри незакрытого блока кода | `[D]` | RE C-07 (`handleKeyPress`/`handleKeyDown`) | в спайке отправка выключена `[F]` → Enter = перенос, `Ctrl+Enter` — no-op; W39h реализует только перенос и счётчик ← RE-D14 |
| CMP-17 | `↑` в пустом поле | редактировать последнее сообщение (`FOCUS_MESSAGES {atEnd:true}`) | `[D]` | RE C-08 | в спайке — фокус на последнюю строку списка `[A]`, редактирование вне Phase 2 ← RE-D14 |
| CMP-18 | вложения | ≤ 10 файлов на сообщение; счётчик вложений | `[D]` | RE C-11 (схема `max(10)`) | K-12; UI-чипы файлов `[A]` (стиль — EMB-09) ← RE-D14 |
| CMP-19 | отдельная кнопка submit | существует в конфиге, скрыта экспериментом `isSubmitButtonEnabled` | `[D]` | RE C-15 | в P3-02 не рисуем: отправка выключена ← RE-D14 |
| CMP-20 | notice «Отправка недоступна» | в референсе полосы нет — наше решение | `[A]` | RE C-17 | 24 px блок (DIV-08), текст `--text-muted` ← RE-D14 |

## 5. Участники: 6 групп и строки

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| MEM-01 | панель | `--layout-member-panel-w` (base) / `--layout-member-panel-w-wide` (≥1600); 256 `[L]` | `[L]`/`[D]` | RW-37 `member_list_w=256`; RE M-10 (`width: 256`, `border-inline-start: 1px solid --border-subtle`) | base 240 — `[A]` (адаптив) ← RE-D14 |
| MEM-02 | число групп | 6 | `[M]` | D-02 `groupCount=6`; RE M-05 (в срезе видны 2: «Server Booster — 2», «В сети — 735») | состав — MEM-04 ← RE-D14 |
| MEM-03 | видимых строк в панели | 35 (34 участника + заголовки) | `[M]`/`[D]` | D-02 `rowCount=35`; RE M-06 | поиска в панели нет (`search: []` `[D]`) ← RE-D14 |
| MEM-04 | состав и порядок групп | hoisted-роли по убыванию позиции → «В сети» → «Не в сети»; в срезе 6 = 4 hoisted-роли + online + offline | `[D]` | RE M-01 (`ep()`: `getSortedRoles().filter(hoist)` + ONLINE + OFFLINE), M-02 (`hoistRoleId`, иначе `online`/`offline`) | заменяет прежнюю догадку «3 роли + 3 статуса»; offline-счётчика в дампе нет (RE G-03) ← RE-D14 |
| MEM-05 | заголовок группы: блок | `--row-member-group` (42 = 16 pad-top + 24 строка + 2) | `[M]` | K-13; D-13; RE M-08 (`padding-block: 16px 4px`) | заменяет `[A]` 24 для блока; текстовая строка 24 — `--row-group-title` ← RE-D14 |
| MEM-06 | заголовок группы: текст | «{label} — {count}» с NBSP перед тире; видимый текст `aria-hidden`, доступное имя — строка с count; ellipsis | `[D]`/`[M]` | RE M-04 (`"Server Booster\u00a0— 2"`), M-08; `layout.json regions.members.group_header` (12/16) | padding-inline 12/4; иконка роли 4 px (M-12) ← RE-D14 |
| MEM-07 | строка: высота и отступы | `--row-h-member`; `margin-inline-start: 8`, `padding-block: 4`, `max-width: calc(100% - 16px)` | `[L]`/`[D]` | K-08; RE M-07 | панель и виртуализация ← RE-D14 |
| MEM-08 | строка: шаг в панели | `--row-member-pitch` | `[M]` | K-08 (43.2 медиана, n=16, JPEG ±1) | `padding: 4px 8px` |
| MEM-09 | строка: аватар / дот | `--avatar-md`; дот 10 + ring 2 px `--bg-chat` | `[L]`/`[D]` | RW-37 `member_avatar=32`; RE M-07 | дот: `--status-online` / `--status-idle` / `--status-dnd` / `--status-offline` ← RE-D14 |
| MEM-10 | строка: имя / подпись | 14/18 `--text`; статус-текст 12/16 `--text-muted` | `[D]` | RE M-07 (CSS + `classTokens`) | заменяет прежнюю смесь `[M]` 16/20 и `[A]` 14/18: дамп даёт 14/18 ← RE-D14 |
| MEM-11 | role-chip | 16 h, pad `0 4px`, radius 4, `--font-size-small` | `[M]` | `layout.json regions.members.role_chip`; `--badge-h`, `--badge-secondary-bg` | в строке максимум 1 чип, остальные — в профиле |
| MEM-12 | счётчики | «{label} — {count}»; count = размер секции на момент рендера (735 online), не всего сервера | `[D]`/`[L]` | RE M-04/M-14; RW-37 `member_count_alt=24229`, `online_count=663` | общее число участников в дампе не снималось `[A]` ← RE-D14 |
| MEM-13 | тулбар экрана | 48, поиск `--search-height` 32 | `[A]`/`[M]` | `SPEC.md` §9.3; D-13; RE M-06 (в панели тулбара нет) | только `#screen-members`, в панели чата нет ← RE-D14 |
| MEM-14 | поиск участников | `--search-height`, radius round, `--surface-input` | `[A]` | `SPEC.md` §7.10 | фильтр по загруженным (≤500), пусто → `.empty-state` |
| MEM-15 | hover / selected / offline | hover `--state-hover`, selected `--state-selected`; offline-строка `opacity: .3`, на hover `1` | `[D]`/`[A]` | `tokens.css` §4; RE M-09 | selected дублируется `aria-current="true"` ← RE-D14 |
| MEM-16 | панель чата: лимит | первые ≤ 50 строк, без виртуализации | `[F]`/`[L]` | `DOM-CONTRACT` §6.3/§6.4 | отдельный `#member-panel-list` |
| MEM-17 | группы в виртуализованном списке | отсутствуют | `[F]` | `DOM-CONTRACT` §7.2, §6.4 | RE M-13 подтверждает фильтрацию пустых секций, но не отменяет `[F]` ← RE-D14 |
| MEM-18 | анонс статуса | `aria-label="<имя>, <статус>, <роль>"` | `[A]`/`[D]` | `SPEC.md` §7.6; RE M-04 (aria-hidden у видимого текста) | статус не только цветом ← RE-D14 |
| MEM-19 | пустые секции | не рисуются (`userIds.length > 0`) | `[D]` | RE M-13 | касается и панели, и экрана ← RE-D14 |
| MEM-20 | иконки в заголовке | иконка роли `margin-inline-end: 4px` (если есть icon/emoji); у бустеров — wheel 4 px + hover-цвет | `[D]` | RE M-08, M-12 | в спайке — inline-SVG 16 `[A]` ← RE-D14 |
| MEM-21 | поверхность панели | `border-inline-start: 1px solid --border-subtle`, фон панели, список `padding: 0 0 10px` | `[D]` | RE M-10 | ширина 256 — `--layout-member-panel-w-wide` ← RE-D14 |
| MEM-22 | сортировка внутри секции | по `(nick ?? globalName).toLowerCase()`, вставка в отсортированную позицию | `[D]`/`[A]` | RE M-03 | в срезе не видна (RE G-04): проверять тестом, не скриншотом ← RE-D14 |

## 6. Search overlay

Оверлей — панель справа от списка сообщений (R-18). Ширина в дампе не логировалась:
`layout.json assumptions` фиксирует только происхождение `x` панели → ширина `[A]`.

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| SRCH-01 | панель: ширина | 418 | `[A]` | R-18 PNG; `layout.json assumptions` («x панели = 1792 − 419») | на shell уже 900 px → `min(418px, 40vw)` |
| SRCH-02 | панель: поверхность | `--surface-2`, border-left 1 px `--border-subtle`, `--shadow-popover` | `[A]` | `SPEC.md` §7.14 (popover) | не модалка: чат остаётся доступен |
| SRCH-03 | шапка панели | 48, input `--search-height` 32, close 20 | `[A]` | `SPEC.md` §7.10; `layout.json chat-header-search` | `role="search"` |
| SRCH-04 | число секций фильтров | 7 | `[M]` | R-10 (`labels`); `REFERENCES.md` §1 | От, В, Содержит, Упоминания, Дата, Тип автора, Закреплено |
| SRCH-05 | секция: заголовок | `--font-size-small` / `--line-height-small`, 600, uppercase, `--text-faint` | `[A]`/`[M]` | R-10 (визуально 12 px uppercase); `--letter-spacing-caps` | высота 16, отступ `--space-4` до контрола |
| SRCH-06 | секция: контрол | 32, bg `--surface-input`, radius `--radius-sm` | `[A]` | `SPEC.md` §7.10 (`--search-height`) | 5 из 7 секций — input `[M]` |
| SRCH-07 | inputs / buttons | 5 input, 4 button | `[M]` | R-10 (`inputs=5`, `buttons=4`) | кнопки: сброс, закрыть, применить, сортировка |
| SRCH-08 | «Содержит»: опции | 9 | `[M]` | R-10 `20b-search-filters-has.json` (`options`) | тексты 9 опций вне scope W39g — §13, G-05 |
| SRCH-09 | «Закреплено» | toggle/чекбокс, `--switch-w`×`--switch-h` | `[A]` | `SPEC.md` §7.9 (switch) | 7-я секция |
| SRCH-10 | сортировка: пункты | 3 | `[M]` | R-11: По новизне (`aria-checked=true`), Самое старое, Самое актуальное | `role="menu"` `[M]` |
| SRCH-11 | сортировка: меню | пункт `--row-menu-item`, контейнер `--menu-min-w`, padding `--menu-pad`, `--shadow-popover` | `[T]`/`[A]` | `SPEC.md` §7.14; геометрия в дампе не снята | выбранный — `--state-selected-strong` + галочка 16 |
| SRCH-12 | сортировка: триггер | icon-btn 32 + подпись `--font-size-ui` `--text-subtle` | `[A]` | R-11 (в PNG — кнопка с текстом) | `aria-haspopup="menu"`, `aria-expanded` |
| SRCH-13 | результаты: панель | заголовок с числом, список 1fr | `[M]`/`[A]` | R-18 («3 302 результата», «49 887») | `04b-search-results.json` пуст → данные только с PNG (§13) |
| SRCH-14 | результаты: элемент | `--font-size-ui`, 2 строки, `--row-h-continuation` + 22 за вторую | `[A]`/`[D]` | K-02/K-03; RE V-14 (`.buttonsContainer`, кнопка «Перейти») | автор/канал/время `--text-muted`, фрагмент `--text` ← RE-D14 |
| SRCH-15 | результаты: срез | 6 пунктов на срез | `[M]` | `docs/re/frontend-d03.md` §1.2 (C-01); `REFERENCES.md` §5 | «показать ещё» — кнопка `--btn-height-sm` |
| SRCH-16 | переход к сообщению | закрыть оверлей, `scrollToIndex`, подсветка `--accent-soft` | `[A]`/`[D]` | RE V-14/V-16 (кнопка «Перейти», id `search-result-<id>`); длительность — `[A]` | после подсветки фокус на `#chat-list`, `aria-live` анонс ← RE-D14 |
| SRCH-17 | пустой результат | `.empty-state` «Ничего не найдено» | `[F]`/`[A]` | `SPEC.md` §7.16 | `role="status"` |
| SRCH-18 | поиск в шапке чата | контейнер-pill 32, поле 244×28 | `[D]`/`[M]`/`[A]` | D-13; RE V-02 (244×28); `layout.json chat-header-search` | оба значения валидны: 32 — контейнер `[A]`, 244×28 — поле `[D]` ← RE-D14 |
| SRCH-19 | id результата | `id="search-result-<id>"`, `data-list-item-id="NO_LIST___<id>"` | `[D]` | RE V-16 | нужен для перехода и тестов ← RE-D14 |

## 7. QuickSwitcher

Состав — `[M]`, геометрия — `[A]` (RE-EXTRA геометрии не даёт, RE G-01).
Экран не входит в Phase 2 (`SPEC.md` §9 не содержит его) → спека на будущее, W39h не блокирует.

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| QS-01 | результаты | 8 | `[M]` | R-17 `03-quickswitcher-results.json` (X-02, md5 `2e5b49…`) | тексты 8 строк — §13, G-03 |
| QS-02 | ввод | contenteditable, не `<input>` | `[M]` | R-17 (X-02, contenteditable в DOM) | у нас допустим `input[type=text]` `[A]` для доступности |
| QS-03 | вызов | `Ctrl+K` (mod+K), «Вкл./выкл. быстрый переключатель» | `[D]` | RE K-01 (реестр `QUICKSWITCHER_SHOW`, i18n `yYsRlD`) | заменяет `[A]` ← RE-D14 |
| QS-04 | карточка | ширина `--modal-w-md`, top 20 vh, radius `--radius-sm`, `--shadow-popover` | `[T]`/`[A]` | `--modal-w-md` = 602 `[T]`; позиция `[A]` | `role="dialog" aria-modal="true"` |
| QS-05 | поле ввода | 48, `--font-size-title`, bg transparent, border-bottom 1 px `--border-subtle` | `[A]` | размер выбран под 24 px шрифт (`--font-size-title`) | placeholder «Куда перейти?» |
| QS-06 | строка результата | `--row-menu-item` 32, padding `0 8px`, radius 4 | `[T]`/`[A]` | `SPEC.md` §7.14 (пункт меню) | иконка 20 + имя + тип (`--text-muted`) |
| QS-07 | активная строка | `--state-selected-strong`, `aria-selected="true"` | `[A]` | `tokens.css` §4 | ровно одна активная |
| QS-08 | клавиши | ↑/↓ — активная строка, Enter — переход, Esc — закрыть, Tab — вне карточки | `[A]`/`[D]` | RE K-04 (Esc закрывает слой/пикер) | `role="combobox"` + `aria-activedescendant`; порядок Tab не подтверждён (RE K-18) ← RE-D14 |
| QS-09 | список | max-height 320, скролл `--scrollbar-thumb` / `--scrollbar-size` | `[A]` | `SPEC.md` §7.14 (max-height меню) | 8 результатов помещаются без скролла при 32 px |

## 8. Profile-popout

| id | элемент | значение | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| PROF-01 | размер | 268.8 × 408 | `[M]` | K-09; R-06 bbox (22.4, 608.8); D-13 | 300 из токена — не размер (D-13) ← RE-D14 |
| PROF-02 | позиция | над строкой участника, `right: 16px`; не выходит за `#member-panel` | `[A]` | R-06: попаут у правого края, y 608.8 при панели от 82 | при нехватке места — зеркалится влево |
| PROF-03 | фон / нижние панели | `--surface-4` / `--surface-2` | `[M]`/`[T]` | R-06 `#2c2d32` = `--surface-4`, панели `#242429` = `--surface-2` | радиус `--radius-sm`, `--shadow-popover` |
| PROF-04 | баннер | 60, bg `--accent` (или детерминированный hue) | `[A]` | `SPEC.md` §7.14 (шапка 60); в R-06 баннер не читается | 60 — assumption |
| PROF-05 | аватар | 80, ring 6 px цвета `--surface-4`, `--avatar-md`-стили инициалов | `[A]`/`[M]` | `SPEC.md` §7.14 (80, ring 6); R-06 (`[M]` только bbox) | статус-дот 16 в правом нижнем углу `[A]` |
| PROF-06 | имя / тег | `--font-size-title` 600 `--text-strong` / `--font-size-ui` `--text-muted` | `[A]` | `SPEC.md` §7.14 (20/24, 14/18) | ellipsis у обоих |
| PROF-07 | секции | «О себе» (2 строки max), «Роли» (чипы), «Активность» (1 строка) | `[A]` | R-06 `06-profile-text.json` = `{"text": null}` → содержимое из фикстур W39e | секции сворачиваются, если данных нет |
| PROF-08 | роли | чипы `--badge-h`, `--badge-secondary-bg`, `--text-subtle` | `[T]` | `SPEC.md` §7.12 (модификатор role у `.badge`) | max 4 чипа + «+N» |
| PROF-09 | нижняя панель действий | 44, кнопки 32 (`--btn-height`), gap `--space-2` | `[A]` | R-06 (нижняя зона `#242429` ≈ 44) | кнопки «Сообщение», «Ещё» |
| PROF-10 | открытие | клик по аватару сообщения (MSG-33) или по строке участника | `[D]`/`[A]` | RE V-15; `SPEC.md` §9.3 | `aria-haspopup="dialog"` у опенера ← RE-D14 |
| PROF-11 | ARIA/поведение | `role="dialog"`, `aria-label="Профиль <имя>"`, фокус-ловушка, Esc, клик вне | `[A]`/`[D]` | `SPEC.md` §7.14; RE K-04 (Esc) | порядок Tab/фокус-ловушка в дампе не подтверждены (RE K-18) ← RE-D14 |
| PROF-12 | данные | фикстуры (имя, статус, роли, активность), без сети; изображения — инициалы | `[F]`/`[L]` | `REFERENCES.md` §6 (F-01); `SPEC.md` §9.3 | роли из `members_page` (W39i) |

## 9. Эмодзи-пикер (сверка SPEC §7.8/§9.2)

| id | элемент | значение/правило | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| EMO-01 | карточка | `--emoji-picker-w` × `--emoji-picker-h`, radius 8 | `[T]`/`[D]` | RE E-01 (min-width 498, height 440) | совпадает с SPEC §7.8 ← RE-D14 |
| EMO-02 | контент-вкладки | 3: «Гифки», «Стикеры», «Эмодзи» (`aria-selected=true` у последней) | `[D]` | RE E-02 | в спайке вкладки = 10 категорий `[F]`; расхождение зафиксировано, менять не даём ← RE-D14 |
| EMO-03 | поиск | placeholder «Найдите идеальный эмодзи», aria «Поиск» | `[D]` | RE E-03 | `--search-height`, `--surface-input` ← RE-D14 |
| EMO-04 | порядок 10 категорий рейла | top guild emoji · recent · {имя сервера} · people · nature · food · activity · travel · objects · symbols | `[D]` | RE E-04 (`21d-emoji-categories.json`) | именно этот порядок; `flags` в рейл не попал (RE G-05) ← RE-D14 |
| EMO-05 | aria-label vs тултип | aria = «сырой» англ. id категории; локализованное имя — только в `title` («Люди», «Природа», «Еда и напитки», «Виды деятельности», «Путешествия», «Объекты», «Символы», recent — «Часто используемые», top — «Лучшие эмодзи сервера {guild}») | `[D]` | RE E-05 (i18n `GX594D`, `egIBDH`, `ldm9aY`, `O783tR`, `w33hIP`, `gWm7Mk`, `QXMYAb`, `vvaizu`, `5TvaSm`, `W6Wi1X`) | отличается от SPEC §7.8 (aria = локализованное имя) → у нас оставляем локализованный `aria-label` для AT, `title` — тоже локализованный `[A]` ← RE-D14 |
| EMO-06 | enum категорий | recent, favorites, top guild emoji, custom, people, nature, food, activity, travel, objects, symbols, flags, premium emoji, soundmoji, suggested | `[D]` | RE E-06 (модуль `732139`) | в спайке 10 категорий `[F]`; остальные — вне Phase 2 ← RE-D14 |
| EMO-07 | глифы вкладок | Clock, Trophy, GameController, Heart, Flag, Star подтверждены; people/nature/food/travel/objects минифицированы | `[D]` | RE E-07 (RE G-06) | переносим только 6 подтверждённых inline-SVG, остальные — нейтральные `[A]` ← RE-D14 |
| EMO-08 | рейл | иконка гильдии 32, юникод 24, padding 4, высота пункта 48, разделитель 1 px с отступами 12 | `[T]` | RE E-08 | `--emoji-rail-w` 48 ← RE-D14 |
| EMO-09 | сетка | 9 колонок (`rowSize=9`), ряды фиксированной длины | `[D]`/`[M]` | RE E-09 (модуль `818645`); `--emoji-cols` 9 | совпадает с `layout.json rows.emoji_cell` ← RE-D14 |
| EMO-10 | объёмы | 50 эмодзи + 17 locked (гильдия), 42 (ЛС); `:fire` → 10/11; секция «Доступно с подпиской Nitro» | `[D]` | RE E-10 | в спайке ≤ 120 ячеек `[F]` ← RE-D14 |
| EMO-11 | нормализация поиска | срезать ведущее/хвостовое `:`, `toLowerCase`, удалить пробелы и `_`, экранировать regex, искать без регистра по name/uniqueName/всем именам; в кандидате `_` тоже удаляется | `[D]` | RE E-11 (`Y`, `searchWithoutFetchingLatest`) | реализовать ровно так; тест на `:fire` / `fire` / `:FIRE:` ← RE-D14 |
| EMO-12 | порядок результатов | unlocked → locked, затем frecency-score, затем имя; `count:0` = без ограничения | `[D]` | RE E-12 (`getSearchResultsOrder`) | locked — премиум, стиль `--state-disabled-opacity` `[A]` ← RE-D14 |
| EMO-13 | «Недавние» | часто используемые (frecency); для реакций — отдельный список | `[D]` | RE E-13 (`K`, `$`) | в спайке хранится в памяти сессии `[A]` ← RE-D14 |
| EMO-14 | тон кожи | переключатель, aria «Выбрать цвет кожи эмодзи (Пусто)» | `[D]` | RE E-14 | вне Phase 2, только aria-контракт ← RE-D14 |
| EMO-15 | секции сетки | «Лучшие эмодзи сервера {guild}», «Часто используемые»; серверные секции сворачиваемые (`EXPAND_OR_COLLAPSE_EMOJIS`) | `[D]` | RE E-15 | заголовок секции 32 (`--emoji-section-h`) ← RE-D14 |
| EMO-16 | область поиска | поиск идёт по всему набору (unicode + custom), не по текущей категории; категория — только листание | `[D]` | RE E-16 | **изменение против SPEC §9.2** (там фильтр по категории) → W39h: искать глобально ← RE-D14 |

## 10. Настройки (сверка SPEC §9.4)

Раздел добавлен по D-14: навигация SPEC §9.4 («6 разделов `[A]`») расходится с дампом.
P3-02 — чат-вью, но число разделов и состав «Голоса» нужны W39h для `features/settings.js`.

| id | элемент | значение/правило | код | источник | примечание |
|:-|:-|:-|:-|:-|:-|
| SET-01 | число разделов навигации | 18 + «Выйти» | `[D]` | RE S-01 (`23-settings-categories.json`, глубина 1, 63 строки) | заменяет `[A]` «6 разделов» из SPEC §9.4; порядок: Учётная запись · Данные и конфиденциальность · Разрешения для сообщений · Уведомления · Nitro · Буст сервера · Подписки · Склад подарков · Выставление счетов · Голос и видео · Внешний вид · Специальные возможности · Система · Язык и время · Зарегистрированные игры · Конфиденциальность активности · Подключённые приложения · Разработчик ← RE-D14 |
| SET-02 | подпункты «Учётной записи» | 4: Информация об учётной записи · Пароль и безопасность · Статус учётной записи · Семейный центр | `[D]` | RE S-02 | вложенный уровень, `--settings-nav-item-h` ← RE-D14 |
| SET-03 | поиск в навигации | placeholder «Поиск» | `[D]` | RE S-03 | 32 px, `--surface-input` ← RE-D14 |
| SET-04 | «Голос и видео»: порядок блоков | Голос (Микрофон, Динамик, проверка) · Профиль ввода · Чувствительность ввода · Шумоподавление · Эхо/рация/АРУ · Камера · Стриминг · Звуки · Связанные настройки · Расширенные | `[D]` | RE S-04 (`23-settings-voice.json` `headings`) | 3 группы `h3` `[F]` не отменяются: блоки внутри групп ← RE-D14 |
| SET-05 | тумблеров в «Голос и видео» | 38 = 12 функциональных + 22 звуковых (4 видимых + 18 под «Показать ещё 18 звуков») + 4 расширенных | `[D]`/`[F]` | RE S-05; RW-38 `switchCount=38` | совпадает с `[F]` 38 — состав теперь известен ← RE-D14 |
| SET-06 | 12 функциональных | авто-чувствительность вкл, эхоподавление вкл, рация выкл, АРУ вкл, расширенная активация вкл, предложение устройств вкл, предупреждение об отсутствии звука вкл, предупреждение о смене канала вкл, QoS выкл, предпросмотр видео вкл, предпросмотр стрима вкл, приглушение стрима вкл | `[D]` | RE S-06 | стартовые `checked` берём из дампа ← RE-D14 |
| SET-07 | 22 звуковых | «Откл. звук», «Включить звук в динамиках», «Заглушить», «Вкл. микрофон», «Камера включена/выключена», «Голосовая связь отключена», «Активация/Деактивация рации», «Пользователь присоединился/вышел/перемещён», «Исходящий звонок», «Стрим начался/остановлен», «Зритель присоединился/вышел», «Начало/Завершение активности», «Присоединение к активности», «Покинул активность», «Приглашение выступить» — все вкл | `[D]` | RE S-07 (`switches[12..33]`) | 18 из них скрыты за «Показать ещё 18 звуков» ← RE-D14 |
| SET-08 | 4 расширенных | OpenH264 вкл, оверлей стрима выкл, запись звука для диагностики выкл, журнал отладки вкл; отдельный тумблер «Показывать настройки диагностики» | `[D]` | RE S-08 (`switches[34..37]`) | последний пункт — не в счёт 38 (отдельный) ← RE-D14 |
| SET-09 | селекты | 4: Микрофон «Система: Easy Effects Source», Динамик «Система: Easy Effects Sink», Камера «Нет видеоустройств», Шумоподавление «Krisp» | `[D]` | RE S-09 | высота `--input-height` 40 (`[A]`); в спайке значения из фикстуры ← RE-D14 |
| SET-10 | ползунки | 4: микрофон 0–100 = 100; динамик 0–200 = 100; приглушение стрима 1–100 = 50 (подпись 49 %); громкость звуковой панели 0–100 = 26.11 (подпись 26 %) | `[D]` | RE S-10 | `--slider-track-h` 8, `--slider-thumb` 16; подпись «N %» — округление вниз ← RE-D14 |
| SET-11 | радиогруппа «Профиль ввода» | 3: «Изоляция голоса», «Студия», «Пользовательский» | `[D]` | RE S-11 | `role="radiogroup"`, `aria-checked` ← RE-D14 |
| SET-12 | кнопки вкладки | 6: «Проверка микрофона»/«Прекратить проверку», «Проверить видео», «Загрузить журналы», «Показать папку», «Сброс» | `[D]` | RE S-12 | `--btn-height` 32; «Сброс» — destructive-стиль `--danger-text` `[A]` ← RE-D14 |
| SET-13 | «Связанные настройки» | «Уведомления» (звуки сообщений и звонков) + «Звуковая панель» (громкость, звук присоединения, селект «Выберите сервер»/«Все серверы», «Выбрать звук»/«Изменить звук») | `[D]` | RE S-13 | ссылки-переходы внутри модалки ← RE-D14 |
| SET-14 | «Внешний вид» | «Отображение сообщений» с вариантами «Компактное»/«Уютное» (кнопка «Включить уютный режим») | `[D]` | RE S-14 (i18n `ZEoGMd`, `7iegX4`, `T7G4Y0`, `CTpwD7`) | связь с `data-density` (§1) ← RE-D14 |
| SET-15 | «Уведомления» | вкладка не открывалась: состав/тумблеры неизвестны | `[A]` | RE S-15, RE G-02 | не рисуем сверх `[F]`-набора; gap G-22 ← RE-D14 |

## 11. Клавиатура и фокус (18 шорткатов)

Реестр — RE-EXTRA §5 (`K-01…K-18`), `mod` = Ctrl (Linux/Windows). Колонка «P3-02»:
`да` — реализуется/проверяется в чат-вью, `нет` — вне Phase 2, `частично` — только контракт.

| id | клавиши | действие | код | источник | P3-02 |
|:-|:-|:-|:-|:-|:-|
| KEY-01 | `Ctrl+K` | QuickSwitcher | `[D]` | RE K-01 | нет (экран вне Phase 2), контракт §7 ← RE-D14 |
| KEY-02 | `Ctrl+E` / `Ctrl+G` / `Ctrl+S` / `Ctrl+P` / `Ctrl+I` | эмодзи / GIF / стикеры / закреплённые / inbox | `[D]` | RE K-02 | да (`Ctrl+E` — пикер, `[F]`), остальные — нет ← RE-D14 |
| KEY-03 | `Ctrl+U`, `Ctrl+F`, `Ctrl+Shift+F` | список участников (toggle), фокус в поиск с префиллом канала / без | `[D]` | RE K-03 | да: `Ctrl+U` — панель, `Ctrl+F` — search overlay ← RE-D14 |
| KEY-04 | `Escape` | закрыть модалку/слой/пикер; в контексте сообщения — фокус в поле | `[D]` | RE K-04 (i18n `rrYBEu`) | да: закрывает QS/пикер/профиль/оверлей ← RE-D14 |
| KEY-05 | `Tab` и любой печатный символ вне полей | фокус в композер (`TEXTAREA_FOCUS = any-character`, `[data-can-focus="true"]`) | `[D]` | RE K-05 (модуль `82038`) | да, но с оговоркой: не перехватывать символ при фокусе в поле ← RE-D14 |
| KEY-06 | `Enter` (без Shift/Alt) | отправка (`wasEnterPressed`) | `[D]` | RE K-06, C-07 | контракт `[D]`, в спайке отправка выключена `[F]` (CMP-16) ← RE-D14 |
| KEY-07 | `↑` в пустом композере | редактировать последнее сообщение | `[D]` | RE K-07 | частично: фокус на последнюю строку (CMP-17) ← RE-D14 |
| KEY-08 | `Ctrl+Alt+↑/↓`, `Ctrl+Shift+[`/`]`, `Ctrl+Tab`/`Ctrl+Shift+Tab`, `Alt+↑/↓` | переход по каналам/серверам | `[D]` | RE K-08 | нет (дерево — отдельный контракт) ← RE-D14 |
| KEY-09 | `PageUp`/`PageDown`, `Shift+PageUp`, `Shift+Esc` | скролл чата; к первому непрочитанному; пометить прочитанным / прыжок к настоящему | `[D]` | RE K-09 (реестр `552049`, `shift+pageup`, `shift+esc`) | да: `PageUp/Down` на `#chat-list` `[F]`; `Shift+Esc` — сброс плашки ← RE-D14 |
| KEY-10 | `Ctrl+Plus` / `Ctrl+Minus` / `Ctrl+0` | зум ± / сброс | `[D]` | RE K-10 | нет (webview масштаб) ← RE-D14 |
| KEY-11 | `Ctrl+Shift+U`, `Ctrl+Shift+T`, `Ctrl+Shift+Alt+N`, `Ctrl+T`/`Ctrl+W` | отправить файл; группа ЛС; ветка; вкладка канала | `[D]` | RE K-11 | `Ctrl+Shift+U` — attach (disabled в спайке), остальное нет ← RE-D14 |
| KEY-12 | `↑/↓`, `Space`/`Enter`, `Esc` | drag&drop: переместить / отпустить / отмена | `[D]` | RE K-12 | нет (нет drag&drop до IO) ← RE-D14 |
| KEY-13 | `→/↓/Space`, `←/↑`, `Esc` | карусели-модалки | `[D]` | RE K-13 | нет (каруселей в Phase 2 нет) ← RE-D14 |
| KEY-14 | `E`, `Backspace`, `P`, `+`, `R`, `F`, `S`, `Ctrl+C`, `Alt+Enter` | действия над сообщением в фокусе: изменить, удалить, закрепить, реакция, ответ, переслать, зачитать, копировать, пометить непрочитанным | `[D]` | RE K-14 (реестр `MESSAGE`, i18n `fsBWmS`,`xwMqD7`,`CvQ18w`,`lfIHs4`,`5IEsGx`,`zSyDdA`,`yGLjXF`,`JrGD7E`,`RpE9k7`) | нет в Phase 2 (нет записи), контракт для меню действий `features/actions.js` ← RE-D14 |
| KEY-15 | — | диалог «Комбинации клавиш»: 6 групп в порядке Навигация · Перетащить мышью · Чат · Голос и видео · Прочее · Сообщение | `[D]` | RE K-15 (`eg()`, i18n `yGE+jg`,`69j6+4`,`/VQax8`,`bI8F5u`,`0Bg9La`,`BAB0yK`) | нет (диалога нет в Phase 2) ← RE-D14 |
| KEY-16 | `Shift+F10`, `Ctrl+Shift+L` | контекстное меню элемента; копировать ссылку на канал | `[D]` | RE K-16 | частично: `Shift+F10` открывает меню сообщения в hover-панели `[A]` ← RE-D14 |
| KEY-17 | `Ctrl+Alt+I` | devtools (в web — no-op) | `[D]` | RE K-17 | нет ← RE-D14 |
| KEY-18 | — | порядок Tab-обхода и focus-trap в поповерах кодом дампа не подтверждены | `[A]` | RE K-18 | остаётся `[A]`: фокус-ловушка — наше решение (PROF-11) ← RE-D14 |

## 12. Состояния (сводно)

| компонент | default | hover | active/pressed | focus-visible | selected | disabled/пусто |
|:-|:-|:-|:-|:-|:-|:-|
| строка сообщения | transparent | `--row-hover-bg` | — | ring на `#chat-list` | `--row-selected-bg` + полоса `--accent` | `data-state="placeholder"` без детей `[F]` |
| hover-панель | скрыта | видна (вместе с hover строки) | `--state-active` | ring | — | скрыта при `aria-disabled` строки |
| reaction pill | `--state-hover` + `--border-subtle` | `--border` | `--state-active` | ring | `--accent-soft` + `--accent` (`aria-pressed`) | нет данных → скрыта |
| reply-quote | полоса `--accent` | вся строка `--row-hover-bg` | — | — | — | автор удалён → «Сообщение удалено» `--text-muted` |
| embed link | `--surface-2` | `--surface-3` | — | ring на ссылке | — | нет превью → только URL |
| embed image | `--surface-3` заглушка | — | — | — | — | без картинки → иконка 48 |
| day divider | линия `--border-subtle` + подпись 12/13 | — | — | — | — | — |
| new messages bar | `--accent` (34 border-box) | — | — | ring на кнопке скролла | — | скрыта, когда непрочитанных нет |
| unread-line divider | `border-top` тонкий, цвет `[A]` `--danger` | — | — | — | `isUnread` | — |
| jump-to-present | `--surface-2` + `--shadow-popover` | `--surface-3` | `--state-active` | ring | — | скрыта в конце списка |
| notice | `--surface-2` + border-bottom | — | — | — | — | скрыт, если нет текста |
| composer | `--surface-sunken` | — | — | ring (`focus-within`) | — | `aria-disabled`, `--text-muted` |
| composer typing | текст + счётчик (≤ 200 остатка) | — | — | ring | — | лимит 2000 → `--danger-text` |
| member row | transparent | `--state-hover` | `--state-active` | ring | `--state-selected` | offline `opacity: .3` (hover → 1) |
| member group header | `--text-faint`, блок 42 | — | — | — | — | пустая секция не рендерится `[D]` |
| emoji tab/cell | transparent | `--state-hover` | `--state-active` | ring | `--state-selected-strong` / `--accent-soft` | locked — `opacity` `[A]` |
| settings toggle (38) | трек 12 % чёрного | `--border-strong` | — | ring | `--accent` | `--state-disabled-opacity` |
| search section | `--surface-input` | `--surface-3` | — | ring | — | `aria-invalid` + `--danger` |
| sort menu item | transparent | `--state-hover` | `--state-active` | ring | `--state-selected-strong` + галочка | — |
| qs row | transparent | `--state-hover` | — | — | `--state-selected-strong` (`aria-selected`) | «Ничего не найдено» |
| profile popout | `--surface-4` | — | — | ring | — | секции без данных скрыты |

Правила (из `SPEC.md` §8, не ослабляются): focus-visible виден всегда; disabled не удаляет
элемент из DOM; selected дублируется `aria-*`; hover не обязателен для тач-сценария;
пустое состояние — оверлей в родителе списка, не ребёнок `#chat-list`/`#member-list` `[F]`.

## 13. Чего нет в дампе

| # | Пробел | Что есть в дампе | Как обойдено | Код |
|:-|:-|:-|:-|:-|
| G-01 | геометрия меню сообщения | только 15 текстов (X-01), без rect; RE G-01 | пункт 32, контейнер `--menu-min-w`, pad `--menu-pad` | `[A]` |
| G-02 | тексты пунктов меню сообщения | счёт 4 реакции + 11 действий (X-01 вне scope чтения W39g) | пункты не выписаны в спеке | `[A]` |
| G-03 | геометрия QuickSwitcher | 8 строк текста (X-02), без rect; RE G-01 | `--modal-w-md`, строки `--row-menu-item`, top 20 vh | `[A]` |
| G-04 | данные результатов поиска | `04b-search-results.json` = `{}`; числа «3 302»/«49 887» только с PNG | список — по виду PNG, данные из фикстур | `[M]`/`[A]` |
| G-05 | 9 опций «Содержит» | счёт известен (R-10), названия — вне scope чтения | UI рисует опции из данных, без хардкода | `[A]` |
| G-06 | текст профиля | `06-profile-text.json` `{"text": null}` | содержимое из фикстур W39e | `[A]` |
| G-07 | reply-quote | в статичных PNG и в RE-EXTRA не встречается (RE G-07) | полоса 2 px `--accent`, quote 22, аватар `--avatar-xs` | `[A]` |
| G-08 | reactions-row | не сняты; RE-EXTRA реакций не даёт | пилюля 24, `--state-hover`/`--accent-soft` | `[A]` |
| G-09 | геометрия embeds/вложений | только контейнер `#message-accessories` (V-14) и лимит 10 (C-11) | §2 целиком `[A]`, кроме токенов и контейнера | `[A]` |
| G-10 | блок day divider | контент 12/13 и caps есть (V-08), общей высоты нет | блок 24 `[A]` из контента + padding | `[A]` |
| G-11 | notice vs blue-плашка | SPEC/DOM 24; PNG 34 — **два разных элемента** | D-13: notice 24, плашка 34 (`--unread-banner-h`) | `[M]`/`[D]` |
| G-12 | компакт-режим: только CSS | RE V-13 даёт метрики, пиксельного замера нет | гаттер 80 `[L]`/`[D]`, `padding-block: 2` `[D]` | `[D]` |
| G-13 | светлая тема | не снята | вне P3-02 | `[A]` |
| G-14 | hover/focus/active | статичные PNG; отдельные состояния в RE только для кнопки эмодзи | токены `[T]`/`[A]` | `[A]` |
| G-15 | заголовки/счётчики групп | порядок групп из кода (M-01), в срезе 2 группы | правило M-01/M-02, счётчики — из данных | `[D]` |
| G-16 | блок заголовка группы | 42 (D-13, M-08), текст 24 | `--row-member-group` | `[M]`/`[D]` |
| G-17 | тулбар экрана участников | в панели нет (M-06), на экране — наше решение | тулбар 48 только `#screen-members` | `[M]`/`[A]` |
| G-18 | ширина панели поиска | origin кропа не логировался | 418 `[A]`, `min(418px, 40vw)` | `[A]` |
| G-19 | лимит текста | **закрыт**: 2000 / Nitro 4000 / порог 200 / >4000 → файл (C-04, C-05, C-12) | K-10, K-11, CMP-13 | `[D]` |
| G-20 | шрифты/иконки/аватары | намеренно не дампятся (RE G-09) | `system-ui` + inline-SVG, инициалы | `[T]` |
| G-21 | категория `flags` | 10 из 11 юникод-категорий в рейле (RE G-05) | не добавлять без решения PM | `[D]` |
| G-22 | «Внешний вид»/«Уведомления»: тумблеры | вкладки не открывались (RE S-15, G-02) | только подписи из i18n; сверх `[F]` не рисуем | `[A]` |
| G-23 | offline-группа и её счётчик | панель не докручена (RE G-03) | заголовок/счётчик из данных, правило M-01 | `[A]` |
| G-24 | сортировка внутри role-группы | не видна в срезе (RE G-04) | правило M-03, тест вместо скриншота | `[D]` |
| G-25 | композер в пикселях | снят только disabled (C-01, RE G-01) | порядок/коверы — по коду `[D]`, не по PNG | `[M]`/`[D]` |
| G-26 | unread/new-bar в пикселях | в канале их нет (RE V-18, G-07) | контракт из CSS `[D]`, пиксельной сверки не будет | `[D]` |
| G-27 | id канальной строки | снят только `search-result-<id>` (V-16) | `chat-messages-<ch>-<id>` — `[A]` | `[A]` |
| G-28 | глифы 5 категорий эмодзи | people/nature/food/travel/objects минифицированы (RE G-06) | 6 подтверждённых SVG + нейтральные `[A]` | `[A]` |

Открытые расхождения: `d-chat-notice` и `d-members-toolbar` **закрыты** D-13
(два элемента / тулбар только на экране), `d-members-group-h` **закрыт** D-13 (блок 42).
Новые расхождения, требующие фиксации в `SPEC.md` (не в P3-02):
шапка чата 3 → 4 icon-btn (VW-01), поиск эмодзи глобальный, а не по категории (EMO-16),
навигация настроек 6 `[A]` → 18 `[D]` (SET-01), aria-label категорий (EMO-05),
время `DD.MM.YYYY, HH:mm` вместо `HH:mm` (MSG-12).
