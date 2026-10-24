# RW-40 / C3a — THEME-SPEC: light-тема и плотности cozy/compact

Статус: дизайн-контракт (документы; код не менялся). Дата: 2026-10-24. Владелец: G3 Design.
Карточка: C3a (`waves/RW-40/TASKS.md`, «темы light/cozy»).
Вход: `/home/sanya/re-deep/ui/tokens.json` (4795 переменных), `ui/00-app.png`, `ui/log.json`,
`waves/RW-39/DESIGN/{tokens.css,SPEC.md,REFERENCES.md}`, `waves/RW-37/RE/layout.json`,
`waves/RW-40/DESIGN/DESIGN-MSG.md`, `rust/gui-spike/ui/{tokens.css,index.html,features/settings.js}`.
Машиночитаемая версия: `waves/RW-40/theme-spec.json` (schema `rw40/theme-spec-v1`).
Ассеты RE не копируются и не используются: переносятся только **числа** (цвета, размеры).

## 0. Легенда

| Метка | Значение | Источник |
|-|-|-|
| `[T]` | токен дампа | `/home/sanya/re-deep/ui/tokens.json` (4795 переменных, тёмное состояние, сборка 1.0.162) |
| `[M]` | измерено на пикселях | `/home/sanya/re-deep/ui/00-app.png` (+ `ui8/*`) |
| `[L]` | RE-контракт раскладки | `waves/RW-37/RE/layout.json` |
| `[F]` | заморожено тестами RW-38 | `tests/unit/gui-spike-dom.test.mjs` |
| `[A]` | решение/пробел контракта | этот документ; подтверждение до реализации |

Правила: каждое значение имеет метку и источник. `[A]` не маскирует отсутствие замера:
light-скриншотов в дампе нет, поэтому `[M]` в этой карточке относится только к факту
тёмного экрана, а не к light-значениям. Все light-цвета — либо существующий токен
нейтральной/цветовой рампы дампа (`[T]`, сверено скриптом), либо наше решение (`[A]`).

## 1. Есть ли в дампе light-тема

**Нет.** Это честный пробел, а не «недостаточно посмотрели».

| Проверка | Факт | Метка |
|-|-|-|
| Структура `tokens.json` | плоская карта 4795 вычисленных custom properties одного состояния; нет theme-контейнера, нет пар `dark`/`light` | `[T]` `[M]` (log.json: tokens count 4795) |
| Имена с `light` | 41: `chip-*` (16), `role-*` (6), `premium-*` (4), `lol-text-*` (2), `text-overlay-light`, `icon-overlay-light`, `font-weight-light`, `custom-*-shadow-light`, `bg-gradient-*`. Это компонентные варианты чипов/ролей/градиентов, а не тема | `[T]` |
| Theme-level light-токены | 0: нет ни одного light-набора surface/text/border/status | `[T]` |
| Значения базовых ролей | `--background-base-low` L 13.333%, `--background-base-lowest` L 7.451%, `--text-default` L 94.118%, `--text-strong` L 98.431% — тёмная тема | `[T]` |
| Экран `00-app.png` | тёмный: средняя яркость 37.7/255; rail/tree `(18,18,20)`, chat `(26,26,30)`, composer `(34,35,39)`, акцентная плашка `(88,101,242)` | `[M]` |
| `log.json` | поля `theme` нет; снимок один, в тёмной теме | `[T]`/`[D]` |
| Light-скриншоты | отсутствуют: и `ui/00-app.png`, и `ui/01…07`, и `ui8/*` сняты в тёмной теме | `[M]` |

Следствие: light-палитра **выводится инверсией ролей** на существующей нейтральной рампе
дампа (`neutral-1…neutral-100`, `red/green/yellow/blue/brand-*`), а не измеряется. Всё, что
не удалось выразить существующим токеном, помечено `[A]` и перечислено в §6.

## 2. Таблица dark → light

### 2.1 Поверхности

Принцип: в тёмной теме elevation = «светлее», в светлой — «темнее + тень/бордер».
`--bg-app` и `--bg-chat` меняются местами по светлоте, а лестница поверхностей идёт
от hover (`--surface-1`) к pressed (`--surface-4`).

| Токен | dark (`tokens.css`) | light | Метка, источник | Применение |
|-|-|-|-|-|
| `--bg-app` | `#121214` | `#f7f7f8` | `[T]` `neutral-3` | рейл, дерево, titlebar |
| `--bg-chat` | `#1a1a1e` | `#ffffff` | `[T]` `neutral-1` | чат, шапка, панель участников |
| `--surface-1` | `#202024` | `#efeff1` | `[T]` `neutral-5` | hover строк, nav настроек |
| `--surface-2` | `#242429` | `#f3f3f4` | `[T]` `neutral-4` | карточка, модалка, пикер, tooltip |
| `--surface-3` | `#28282d` | `#ebeced` | `[T]` `neutral-6` | вложенная поверхность, embed-image, toggle track |
| `--surface-4` | `#2c2d32` | `#e8e8ea` | `[T]` `neutral-7` | pressed, hover rail-кнопок, фон msg-toolbar |
| `--surface-sunken` | `#222327` | `#f3f3f4` | `[T]` `neutral-4` | composer |
| `--surface-input` | `#1e1f22` | `#ffffff` | `[T]` `neutral-1` | поле ввода (на серых поверхностях — белое с бордером) |
| `--row-hover-bg` | `var(--surface-1)` | `#efeff1` | `[T]` `neutral-5`, alias `--surface-1` | hover строки |
| `--row-selected-bg` | `var(--surface-2)` | `#ebeced` | `[T]` `neutral-6`, alias `--surface-3` | выбранная строка |

Теснота шага — нормальная для светлой темы: разделение несут `--border-subtle` и тени.
`--surface-2` **не может быть белым**: `.embed` рисуется на `--bg-chat` без бордера и
потерялся бы (проверено по `components/chat.css`).

### 2.2 Текст

Порядок контраста сохранён как в тёмной теме: `strong > text > subtle > muted > faint`.

| Токен | dark | light | Метка, источник | Применение |
|-|-|-|-|-|
| `--text-strong` | `#fbfbfb` | `#323339` | `[T]` `neutral-69` | имя автора, заголовок |
| `--text` | `#efeff1` | `#3f4048` | `[T]` `neutral-60` | тело сообщений |
| `--text-subtle` | `#abacb2` | `#5f606a` | `[T]` `neutral-45` | вторичные подписи |
| `--text-muted` | `#96979e` | `#696a73` | `[T]` `neutral-42` | время, placeholder, счётчики |
| `--text-faint` | `#81828a` | `#70717a` | `[T]` `neutral-40` | категории/группы; только `--bg-app`/`--bg-chat` |
| `--text-link` | `#4d96ee` | `#0363c1` | `[T]` `blue-460` | ссылки, info-иконки |
| `--text-accent` | `#a5b0ff` | `#4654c0` | `[T]` `brand-560` | акцентный текст |
| `--text-on-accent` | `#ffffff` | `#ffffff` | `[T]` `neutral-1` | текст на `--accent` |
| `--text-on-danger` | `#ffffff` | `#ffffff` | `[T]` `neutral-1` | текст на `--danger-fill` |
| `--text-on-warning` | `#121214` | `#323339` | `[T]` `neutral-69` | текст на `--warning` |
| `--text-inverse` | `#2f3035` | `#ffffff` | `[T]` `neutral-1` | текст на тёмной/цветной заливке |

Ограничения (проверены §2.6): `--text-faint` — только `--bg-app` (4.53) и `--bg-chat` (4.85);
`--text-muted` не применяется на `--surface-4` (4.39) — там `--text-subtle` (5.10) или `--text`.

### 2.3 Акцент

| Токен | dark | light | Метка, источник | Применение |
|-|-|-|-|-|
| `--accent` | `#5865f2` | `#5865f2` | `[T]` `brand-500` | primary, активный пункт; как текст не используется |
| `--accent-hover` | `#4654c0` | `#4654c0` | `[T]` `brand-560` | hover primary |
| `--accent-active` | `#3c45a5` | `#3a48a3` | `[T]` `brand-600` (dark — `[A]`) | pressed primary |
| `--accent-soft` | `rgb(88 101 242 / 16%)` | `rgb(88 101 242 / 12%)` | `[A]`: 16% дампа рассчитаны на тёмный фон, на белом дают слишком плотную заливку | выбранная реакция, mention, dropzone |

`#ffffff` на `--accent` = **4.607** — AA проходит в обеих темах; на `--accent-hover` 6.38.

### 2.4 Границы, состояния, фокус, скроллбар

В тёмной теме бордеры — белый/серый с альфой; в светлой альфа меняется на чёрную,
чтобы «углублять», а не «подсвечивать».

| Токен | dark | light | Метка, источник | Применение |
|-|-|-|-|-|
| `--border-subtle` | `rgb(151 151 159 / 12%)` | `rgb(0 0 0 / 8%)` | `[A]` зеркало альфы | границы панелей |
| `--border` | `rgb(151 151 159 / 20%)` | `rgb(0 0 0 / 14%)` | `[A]` `#dbdbdb` на белом | границы контролов |
| `--border-strong` | `rgb(151 151 159 / 44%)` | `rgb(0 0 0 / 48%)` | `[A]` non-text AA 3.55–3.69 | input, сильные разделители |
| `--divider` | `rgb(255 255 255 / 6%)` | `rgb(0 0 0 / 6%)` | `[A]` инверсия | разделители строк |
| `--state-hover` | `rgb(255 255 255 / 6%)` | `rgb(0 0 0 / 5%)` | `[A]` затемнение вместо осветления | hover |
| `--state-active` | `rgb(255 255 255 / 9%)` | `rgb(0 0 0 / 8%)` | `[A]` | pressed |
| `--state-selected` | `rgb(255 255 255 / 11%)` | `rgb(0 0 0 / 10%)` | `[A]` | выбранный канал |
| `--state-selected-strong` | `rgb(255 255 255 / 14%)` | `rgb(0 0 0 / 13%)` | `[A]` | вкладка/пункт меню |
| `--state-disabled-opacity` | `0.45` | `0.45` | `[A]` от темы не зависит | disabled |
| `--badge-secondary-bg` | `rgb(151 151 159 / 16%)` | `rgb(0 0 0 / 8%)` | `[A]` зеркало | badge |
| `--focus-ring` | `#a5b0ff` | `#4654c0` | `[T]` `brand-560`, 5.22–6.38 | focus-visible |
| `--scrollbar-thumb` | `#5f606a` | `#81828a` | `[T]` `neutral-35`, 3.12–3.82 | скроллбар |

Тени light (`[A]`, зеркало тёмных): `--shadow-elevation-low` —
`0 1px 0 rgb(0 0 0 / 6%), 0 1.5px 0 rgb(0 0 0 / 4%), 0 2px 0 rgb(0 0 0 / 3%)`;
`--shadow-elevation-stroke` — `0 0 0 1px rgb(0 0 0 / 8%)`;
`--shadow-elevation-medium` — `0 4px 4px rgb(0 0 0 / 8%)`;
`--shadow-elevation-high` — `0 8px 16px rgb(0 0 0 / 12%)`;
`--shadow-border` — `0 0 0 1px rgb(0 0 0 / 8%)`;
`--shadow-popover` — `var(--shadow-elevation-high), var(--shadow-elevation-stroke)`.

### 2.5 Статусы и обратная связь

| Токен | dark | light | Метка, источник | Применение |
|-|-|-|-|-|
| `--danger` | `#da3e44` | `#da3e44` | `[T]` `red-400`, non-text 3.61–4.42 | иконка/точка |
| `--danger-text` | `#ff9a9e` | `#bd2934` | `[T]` `red-460`, 4.86–5.95 | текст ошибки |
| `--danger-fill` | `#cf3a3f` | `#cf3a3f` | `[A]` белый текст 4.85; `#da3e44` даёт 4.42 | destructive-кнопка |
| `--danger-fill-hover` | `#b92d33` | `#b92d33` | `[A]` белый текст 6.02 | hover destructive |
| `--warning` | `#fdb833` | `#fdb833` | `[T]` `yellow-300`; только заливка | предупреждение, mention-полоса |
| `--warning-text` | — | `#945300` | `[T]` `yellow-460`, 4.91–6.01, **новое имя** | текст предупреждения на светлом |
| `--positive` | `#3d9e60` | `#269153` | `[T]` `green-400`, non-text 3.26–3.99 | success/online |
| `--positive-text` | `#57d98a` | `#01713a` | `[T]` `green-460`, 5.01–6.13 | positive-текст |
| `--info` | `#4d96ee` | `#0363c1` | `[T]` `blue-460`, 4.82–5.90 | info-плашка |
| `--status-online` | `#3d9e60` | `#269153` | `[T]` `green-400` | точка статуса |
| `--status-idle` | `#fdb833` | `#bb7300` | `[T]` `yellow-400`, 3.08–3.77 | точка статуса |
| `--status-dnd` | `#da3e44` | `#da3e44` | `[T]` `red-400` | точка статуса |
| `--status-offline` | `#80848e` | `#80848e` | `[A]` статус-токена в дампе нет | точка статуса |
| `--status-streaming` | `#593695` | `#593695` | `[A]` в дампе 1.0.162 токена нет | точка статуса |

`--warning` на светлом фоне имеет 1.74 к белому, поэтому **не используется как текст/иконка** —
только заливка с `--text-on-warning` (7.24). Для текста есть `--warning-text`.

### 2.6 Контраст (WCAG 2.2 AA), расчёт по sRGB

Текст: требование ≥ 4.5:1 (< 24px). Non-text: ≥ 3:1 (иконки, точки, границы контролов).

| Токен | bg-app | bg-chat | s-1 | s-2 | s-3 | s-4 | sunken | input | min |
|-|-|-|-|-|-|-|-|-|-|
| `--text-strong` `#323339` | 11.76 | 12.59 | 10.96 | 11.35 | 10.64 | 10.29 | 11.35 | 12.59 | 10.29 |
| `--text` `#3f4048` | 9.62 | 10.30 | 8.97 | 9.29 | 8.71 | 8.42 | 9.29 | 10.30 | 8.42 |
| `--text-subtle` `#5f606a` | 5.82 | 6.24 | 5.43 | 5.62 | 5.27 | 5.10 | 5.62 | 6.24 | 5.10 |
| `--text-muted` `#696a73` | 5.02 | 5.37 | 4.68 | 4.84 | 4.54 | 4.39 ✗ | 4.84 | 5.37 | 4.39 |
| `--text-faint` `#70717a` | 4.53 | 4.85 | 4.22 ✗ | 4.37 ✗ | 4.10 ✗ | 3.96 ✗ | 4.37 ✗ | 4.85 | 3.96 |
| `--text-link` `#0363c1` | 5.51 | 5.90 | 5.14 | 5.32 | 4.99 | 4.82 | 5.32 | 5.90 | 4.82 |
| `--text-accent` `#4654c0` | 5.96 | 6.38 | 5.56 | 5.76 | 5.40 | 5.22 | 5.76 | 6.38 | 5.22 |
| `--danger-text` `#bd2934` | 5.56 | 5.95 | 5.18 | 5.36 | 5.03 | 4.86 | 5.36 | 5.95 | 4.86 |
| `--warning-text` `#945300` | 5.61 | 6.01 | 5.23 | 5.42 | 5.08 | 4.91 | 5.42 | 6.01 | 4.91 |
| `--positive-text` `#01713a` | 5.73 | 6.13 | 5.34 | 5.53 | 5.18 | 5.01 | 5.53 | 6.13 | 5.01 |
| `--info` `#0363c1` | 5.51 | 5.90 | 5.14 | 5.32 | 4.99 | 4.82 | 5.32 | 5.90 | 4.82 |

Колонки: `bg-app #f7f7f8`, `bg-chat #ffffff`, `s-1 #efeff1`, `s-2 #f3f3f4`,
`s-3 #ebeced`, `s-4 #e8e8ea`, `sunken #f3f3f4`, `input #ffffff`.

Non-text (минимум по всем поверхностям): `--accent` 3.77, `--accent-hover` 5.22,
`--accent-active` 6.51, `--danger` 3.61, `--positive` 3.26, `--status-online` 3.26,
`--status-idle` 3.08, `--status-dnd` 3.61, `--status-offline` 3.06, `--status-streaming` 7.19,
`--border-strong` 3.55, `--focus-ring` 5.22, `--scrollbar-thumb` 3.12.

Пары «текст на заливке»: `#ffffff` на `--accent` **4.607**; `#ffffff` на `--danger-fill`
**4.850**; `--text-on-warning` на `--warning` **7.242**; `#ffffff` на `--danger-fill-hover`
**6.024**.

Правила: `--accent` и `--warning` не используются как цвет текста; `--text-faint` не поднимается
выше `--bg-chat`; `--text-muted` не применяется на `--surface-4`; hover/selected-фоны меняют
фон ≤ 10%, поэтому контраст текста на них остаётся ≥ 4.5 (проверено: `--text-muted` на
`rgb(0 0 0 / 5%)` поверх белого = 5.10).

## 3. Плотности: cozy и compact

Атрибут `data-density` уже пишется `features/settings.js` (`SETTINGS_DENSITIES =
["cozy","compact"]`, дефолт `cozy`). Плотность меняет **только список сообщений**:
дерево 28, участники 42, шапка 49, composer 56 — одинаковы в обоих режимах.

### 3.1 Значения

| Токен | cozy | compact | Метка, источник | Что означает |
|-|-|-|-|-|
| `--message-gutter` | `64px` | `80px` | `[L]` `message_indent_cozy` / `message_indent_compact` | отступ текста от левого края; cozy = 16 pad + 40 avatar + 8 gap, compact = 5rem |
| `--row-h-group-start` | `48px` | `26px` | cozy `[L]` `row_heights_detail.message_cozy_group_start`; compact `[A]` (в оракуле только `row_heights.message_compact = 26`) | высота первой строки группы |
| `--row-h-continuation` | `26px` | `26px` | `[L]` `row_heights.message_cozy` / `row_heights.message_compact` | высота продолжения группы |
| `--row-line-extra` | `22px` | `22px` | `[L]` `line_height` | прибавка за каждый перенос строки |
| `--line-height-message` | `22px` | `22px` | `[L]` `line_height` | межстрочный интервал тела |
| `--row-message-track` | `44px` | `44px` | `[F]` RW-38 | трек виртуализации; **не меняется** до D-04/W39d |

Арифметика высот (совпадает с `layout.json` provenance — css-derived):

- cozy group start `48 = 2 (pad-top) + 22 (строка имени) + 22 (тело) + 2 (pad-bottom)`;
- cozy continuation `26 = 22 + 2 + 2`;
- compact любая строка `26 = 22 + 2 + 2` (аватара и отдельной строки имени нет);
- перенос текста в обоих режимах: `+22` за строку (`--row-line-extra`);
- аватар 40 (`--avatar-lg`) есть только в cozy; в compact он скрыт, а gutter растёт до 80.

### 3.2 Что именно меняется

| Область | cozy | compact | Метка |
|-|-|-|-|
| Аватар 40 в строке | показан | скрыт | `[A]` (поведение, не токен) |
| Имя/бейдж/время | отдельной строкой над телом | в одну строку с телом | `[A]` |
| Отступ текста | 64 | 80 | `[L]` |
| Первая строка группы | 48 | 26 (без группового зазора) | `[L]` / `[A]` |
| Продолжение группы | 26 | 26 | `[L]` |
| Перенос строки | +22 | +22 | `[L]` |
| Hover/selected фон | вся строка | вся строка | `[A]` (геометрия заливки не измерена) |
| Трек виртуализации | 44 | 44 | `[F]` |
| Каналы/участники/шапка/composer | 28 / 42 / 49 / 56 | те же | `[L]` |

Почему compact не «сжимает» трек: высота строки в виртуализаторе — предмет D-04
(prefix-sum по 26/48/+22), реализация в W39d; до этого CSS не переопределяет
`height/top` строки (`components/chat.css`), поэтому плотность сейчас влияет на gutter
и контент, но не на шаг списка. Это осознанное ограничение, а не забытый пункт.

### 3.3 CSS-блоки плотности

```css
:root[data-density="cozy"] {
  --message-gutter: 64px;
  --row-h-group-start: 48px;
  --row-h-continuation: 26px;
  --row-line-extra: 22px;
  --line-height-message: 22px;
}

:root[data-density="compact"] {
  --message-gutter: 80px;
  --row-h-group-start: 26px;
  --row-h-continuation: 26px;
  --row-line-extra: 22px;
  --line-height-message: 22px;
}
```

## 4. Применение

### 4.1 Атрибуты

| Что | Значение |
|-|-|
| Элемент | `<html>` (`document.documentElement`), не `body` |
| Тема | `data-theme="dark"` (дефолт) / `data-theme="light"` |
| Плотность | `data-density="cozy"` (дефолт) / `data-density="compact"` |
| Разметка | `<html lang="ru" data-theme="dark" data-density="cozy">` |
| Писатель | `ui/features/settings.js` → `applySettings()` (уже реализовано в B8b; белые списки `dark/light`, `cozy/compact`) |
| `color-scheme` | `dark` в `:root`, `light` в `:root[data-theme="light"]` |

### 4.2 CSS-контракт

```css
:root { color-scheme: dark; /* существующая база, не меняется */ }

:root[data-theme="light"] {
  color-scheme: light;
  /* 57 переопределений из §2 (свойства ниже) */
}

:root[data-theme="light"] { --bg-app: #f7f7f8; --bg-chat: #ffffff; /* … */ }
```

Специфичность: `:root[data-theme="light"]` (0,2,0) выигрывает у `:root` (0,1,0) и у
`html` (0,0,1), поэтому блок можно добавлять после базового без `!important`.
Порядок в `tokens.css`: `:root` → `:root[data-theme="light"]` → блоки `data-density`.

Полный список переопределяемых переменных (57): `--bg-app`, `--bg-chat`, `--surface-1…4`,
`--surface-sunken`, `--surface-input`, `--row-hover-bg`, `--row-selected-bg`,
`--text-strong`, `--text`, `--text-subtle`, `--text-muted`, `--text-faint`, `--text-link`,
`--text-accent`, `--text-on-accent`, `--text-on-danger`, `--text-on-warning`, `--text-inverse`,
`--accent`, `--accent-hover`, `--accent-active`, `--accent-soft`, `--danger`, `--danger-text`,
`--danger-fill`, `--danger-fill-hover`, `--warning`, `--warning-text`, `--positive`,
`--positive-text`, `--info`, `--status-online`, `--status-idle`, `--status-dnd`,
`--status-offline`, `--status-streaming`, `--border-subtle`, `--border`, `--border-strong`,
`--divider`, `--state-hover`, `--state-active`, `--state-selected`, `--state-selected-strong`,
`--state-disabled-opacity`, `--badge-secondary-bg`, `--focus-ring`, `--scrollbar-thumb`,
`--shadow-elevation-low`, `--shadow-elevation-stroke`, `--shadow-elevation-medium`,
`--shadow-elevation-high`, `--shadow-border`, `--shadow-popover`.

Не меняются: типографика, спейсинг, радиусы, размеры компонентов, раскладка, аватары
(белые инициалы на `hsl(h 45% 32%)` проходят AA в обеих темах), движение, z-слои.

### 4.3 Предусловия для Frontend (вне C3a)

1. `components/primitives.css`: убрать `html { color-scheme: dark }` — сейчас перекрывается
   специфичностью, но дубль должен уйти.
2. `index.html`: `<meta name="color-scheme" content="dark">` → `content="dark light"`.
3. Компоненты уже используют только `var(--token)`; цветовых литералов нет, кроме
   theme-agnostic scrim `rgb(0 0 0 / 62%)` (quick-switcher) и `rgb(0 0 0 / 72%)` (auth)
   и `hsl(var(--avatar-*))` — они корректны в обеих темах.
4. Новое имя `--warning-text` добавить в dark-базу со значением `#fdb833` (там оно
   не используется как текст, но имя должно существовать, чтобы `var()` не был пустым).

### 4.4 Риски

| Риск | Метка | Решение |
|-|-|-|
| FOUC: тема применяется после async `settings_load` | `[A]` | вне C3a; вариант — pre-paint атрибут/инлайн-скрипт, требует решения PM |
| `theme=system` (`prefers-color-scheme`) | `[A]` gap | в белом списке только `dark/light`; новое значение = миграция настроек |
| Плотность не меняет шаг виртуализации до W39d | `[A]`/`[F]` | задокументировано в §3.2, не «зеленить» подгонкой тестов |
| `--text-faint` на `--bg-app` = 4.53 (запас 0.03) | `[A]` | не использовать на серых поверхностях |
| Тема не проверена на живом Discord | `[A]` | окно/live запрещены карточкой; проверка — QA по §2.6 |

## 5. Проверка и воспроизводимость

| Артефакт | Что делает |
|-|-|
| `PM/c3a/validate.py` | JSON-схема; отсутствие внешних ссылок/шрифтовых подключений; существование имён в `tokens.css`; сверка `[T]`-значений с дампом; `[L]` с `layout.json`; пересчёт 160 пар контраста |
| `PM/c3a/contrast.py` → `contrast.log` | матрицы контраста из `theme-spec.json` (единственный источник значений) |
| `PM/c3a/scan-tokens.log` | скан дампа: 4795 токенов, 41 имя с `light`, 0 theme-level |
| `PM/c3a/validate.log` | вывод валидатора (324 проверки) |
| `PM/c3a/REPORT.md` | отчёт: [T] vs [A], пробелы, md5 |

Проверки карточки: JSON валиден; поиск внешних ссылок и шрифтовых подключений по
`THEME-SPEC.md` и `theme-spec.json` пуст; каждое значение либо совпадает с существующим
токеном дампа, либо помечено `[A]`; ни один ассет не скопирован.

## 6. Пробелы и решения

| # | Пробел/решение | Метка |
|-|-|-|
| G-01 | Light-темы и light-скриншотов в дампе нет; палитра выведена инверсией ролей | `[T]`/`[A]` |
| G-02 | Light-значения не измерены пиксельно; `[M]` только для факта тёмного экрана | `[M]` |
| G-03 | `--warning-text` — новое имя (нет в `tokens.css` и дампе) | `[A]` |
| G-04 | compact group start 26 не измерен: в `layout.json` только `message_compact=26` | `[A]` |
| G-05 | D-04 (26/48/+22) не реализован в виртуализаторе; трек 44 заморожен | `[F]`/`[A]` |
| G-06 | Нет `theme=system`; `prefers-color-scheme` не используется | `[A]` |
| G-07 | FOUC при загрузке настроек | `[A]` |
| G-08 | Заливка hover/selected в compact не измерена (ширина до gutter или от gutter) | `[A]` |
| G-09 | `--status-streaming` отсутствует в дампе (обе темы) | `[A]` |
| G-10 | Тени light — расчётные, не измеренные | `[A]` |

## 7. Что не делалось

Код не менялся: `rust/gui-spike/ui/**`, `src/**`, тесты, `package.json` не тронуты.
Окно, сеть, cargo, коммиты не запускались. Ассеты (PNG/JPG/шрифты/иконки) не копировались
и не используются; шрифты — только системные, иконки — существующие inline-SVG проекта.
