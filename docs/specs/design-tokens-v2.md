# Спецификация: Design Tokens V2, Контрастность WCAG 2.2 (Dark / Light / Midnight) и Адаптивные Брейкпоинты

- **Карточка:** `TASK-631` (`G3 Design Lead` — Design Tokens V2, Dark/Light/Midnight Theme WCAG 2.2 Contrast Audit, Breakpoints Spec & Meta Test Sync)
- **Статус:** `Active Specification (V2)`
- **Канонический CSS-источник:** `rust/gui-spike/ui/tokens.css` (`204` базовых `:root`-токена + `@media` брейкпоинты и режимы `cozy`/`compact`)
- **Каноническая карта алиасов:** `rust/gui-spike/ui/features/token-map.js` (`25` канонических токенов, `7` разрешённых коллизий `RESOLVED_COLLISIONS`)
- **Эталоны обратного инжиниринга (RE Oracles):**
  - `/home/sanya/re-deep/frontend-d03.md` — контракты 12 Mana-компонентов и DOM-токены Discord Desktop (`build 1.0.162`, модуль `657718`, `521489`, `798819`, `306091`, `6095`)
  - `/home/sanya/re-deep/ui/tokens.json` — полный дамп `4795` CSS Custom Properties (`neutral-1..100`, `brand-100..900`, `primary-100..900`, `red`, `green`, `yellow`, `blue`, `plum`)
  - `waves/RW-37/RE/layout.json` — снятые через CDP bounding-box размеры 4-колоночного layout при `1600×900`
  - `waves/RW-40/theme-spec.json` и `waves/RW-40/THEME-SPEC.md` — `57` переопределений Light-темы и `160` пар контраста

---

## 1. Архитектура и Происхождение Токенов (Provenance Model V2)

Все `204` базовых CSS Custom Properties в `rust/gui-spike/ui/tokens.css` разделены на 15 функциональных секций (`§0..§14`) и снабжены строгой меткой происхождения:

| Метка | Источник истины | Назначение |
|---|---|---|
| `[M]` | `/home/sanya/re-deep/frontend-d03.md` §5–§6 | Извлечённые из JS/CSS бандлов Discord Mana токены 12 базовых компонентов (`Modal`, `Button`, `Menu`, `Switch`, `Input`, `Badge`, `Scroller` и др.) |
| `[T]` | `/home/sanya/re-deep/ui/tokens.json` (`4795` переменных) | Палитра `neutral-1..100`, `primary-500..900`, `brand-260..600`, `red`, `green`, `yellow`, `blue`, `opacity-*`, радиусы и тени |
| `[L]` | `waves/RW-37/RE/layout.json` (снимок `1600×900`) | Измеренные геометрические размеры колонок (`72px`, `311px`, `829px`, `256px`), шапки (`48px`), композера (`56px`/`68px`) и строк (`26px`/`48px`/`42px`) |
| `[A]` | Аудит контрастности WCAG 2.2 AA/AAA | Скорректированные значения семантических цветов и пар текст-на-заливке для гарантированного прохождения порогов `4.5:1`, `7.0:1` и `3.0:1` |
| `[F]` | Фокус, интеракции и доступность (WCAG 2.4.7 / 2.4.11) | Кольца клавиатурного фокуса (`--focus-ring`, `--focus-ring-danger`), состояния `hover`/`active`/`selected` и тайминги анимаций |

### 1.1. Синтаксические инварианты `tokens.css`
1. **Только нативные CSS Custom Properties:** `:root` содержит ровно `204` токена; ни один токен не использует запрещённый суффикс `!important`.
2. **Атомарные цветовые форматы:** каждый цветовой токен задаётся либо в нижнем регистре `#rrggbb` (6 hex-символов), либо в современном CSS Color 4 синтаксисе `rgb(r g b / a%)`, либо как `var(--*)` со ссылкой на базовый токен.
3. **Глубина разрешения алиасов:** любая цепочка `var(--*)` разрешается детерминированно не более чем за `8` шагов без циклов и висячих ссылок.
4. **Ассет-гигиена:** файл `tokens.css` и документация спецификации не содержат внешних загрузок шрифтов или удалённых картинок; системный стек шрифтов задаётся через `--font-sans`, `--font-display`, `--font-mono`.

---

## 2. Каноническая Карта Токенов и Разрешение Коллизий (`token-map.js`)

Для исключения расхождений между внутренними именами `gui-spike` и эталонными переменными Discord Desktop модуль `rust/gui-spike/ui/features/token-map.js` экспортирует `CANONICAL_TOKEN_MAP` (`25` записей) и таблицу разрешённых коллизий `RESOLVED_COLLISIONS` (`7` пар):

| Устаревшее имя (Legacy) | Канонический токен Discord (`token-map.js`) | Внутренний токен `tokens.css` (Dark) | Категория |
|---|---|---|---|
| `--bg-secondary` | `--background-secondary` (`#2b2d31`) | `--surface-1` (`#202024`) | `surface` |
| `--bg-tertiary` | `--background-tertiary` (`#1e1f22`) | `--bg-app` (`#121214`) / `--surface-input` (`#1e1f22`) | `surface` |
| `--text-muted` | `--text-muted-discord` (`#949ba4`) | `--text-muted` (`#96979e`) | `text` |
| `--border-subtle` | `--background-modifier-accent` (`rgba(78, 80, 88, 0.48)`) | `--border-subtle` (`rgb(151 152 159 / 12%)`) | `border` |
| `--brand-hover` | `--brand-560` (`#4752c4`) | `--accent-hover` (`#4654c0`) | `brand` |
| `--danger-fg` | `--status-danger` (`#f23f43`) | `--danger` (`#da3e44`) / `--danger-text` (`#ff9a9e`) | `status` |
| `--surface-overlay` | `--background-floating` (`#111214`) | `--surface-popout` (`#242429`) / `--bg-app` (`#121214`) | `surface` |

Полный перечень `25` ключей `CANONICAL_TOKEN_MAP`:
- **Surface (`6`):** `--background-primary` (`#313338`), `--background-secondary` (`#2b2d31`), `--background-secondary-alt` (`#232428`), `--background-tertiary` (`#1e1f22`), `--background-accent` (`#4e5058`), `--background-floating` (`#111214`).
- **Text (`5`):** `--text-normal` (`#dbdee1`), `--text-muted-discord` (`#949ba4`), `--text-link` (`#00a8fc`), `--header-primary` (`#f2f3f5`), `--header-secondary` (`#b5bac1`).
- **Brand (`3`):** `--brand-500` (`#5865f2`), `--brand-560` (`#4752c4`), `--brand-600` (`#3c45a5`).
- **Status (`4`):** `--status-positive` (`#23a55a`), `--status-warning` (`#f0b232`), `--status-danger` (`#f23f43`), `--status-offline` (`#80848e`).
- **Interactive (`4`):** `--interactive-normal` (`#b5bac1`), `--interactive-hover` (`#dbdee1`), `--interactive-active` (`#ffffff`), `--interactive-muted` (`#4e5058`).
- **Border (`3`):** `--background-modifier-accent` (`rgba(78, 80, 88, 0.48)`), `--background-modifier-hover` (`rgba(78, 80, 88, 0.3)`), `--background-modifier-selected` (`rgba(78, 80, 88, 0.6)`).

---

## 3. Аудит Контрастности WCAG 2.2 AA и AAA по Трём Темам (Dark / Light / Midnight)

Расчёт относительной яркости $L$ и контрастного отношения $CR$ выполняется строго по формуле WCAG 2.2 (sRGB):

$$L = 0.2126 \cdot R_{lin} + 0.7152 \cdot G_{lin} + 0.0722 \cdot B_{lin}, \quad CR = \frac{\max(L_1, L_2) + 0.05}{\min(L_1, L_2) + 0.05}$$

где $C_{lin} = (C_{srgb} / 12.92)$ при $C_{srgb} \le 0.04045$, иначе $((C_{srgb} + 0.055) / 1.055)^{2.4}$.

### Нормативы WCAG 2.2 в проекте:
- **WCAG 2.2 AA (Normal Text, SC 1.4.3):** `CR >= 4.5:1` для текста `< 18pt` (`24px`) или `< 14pt bold` (`18.66px`).
- **WCAG 2.2 AAA (Enhanced Text, SC 1.4.6):** `CR >= 7.0:1` для основного потока сообщений (`--text`, `--text-strong`) на всех базовых поверхностях.
- **WCAG 2.2 AA (Large Text & Non-Text Contrast, SC 1.4.3 / 1.4.11 / 2.4.11):** `CR >= 3.0:1` для крупного текста, статусных точек, бейджей, ползунков, переключателей и кольца клавиатурного фокуса `--focus-ring`.

---

### 3.1. Тема 1: Dark Theme (Каноническая тёмная тема `:root` в `tokens.css`)

#### Поверхности Dark Theme (`8` базовых уровней):
| Токен | Hex | Эталон `ui/tokens.json` | Относительная яркость $L$ | Назначение |
|---|---|---|---|---|
| `--bg-app` | `#121214` | `neutral-92` | `0.00613` | Рейл серверов, подложка окна |
| `--bg-chat` | `#1a1a1e` | `neutral-85` | `0.01045` | Центральная колонка чата и шапка |
| `--surface-input` | `#1e1f22` | `primary-800` | `0.01356` | Поле ввода композера, инпуты, поиск |
| `--surface-1` | `#202024` | `neutral-80` | `0.01451` | Дерево каналов, панель участников |
| `--surface-sunken` | `#222327` | `neutral-79` | `0.01662` | Голосовая панель, карточки настроек |
| `--surface-2` | `#242429` | `neutral-76` (`--modal-background`) | `0.01765` | Модалки, поповеры, карточки вложений |
| `--surface-3` | `#28282d` | `neutral-73` | `0.02113` | Всплывающие меню, чипы, тултипы |
| `--surface-4` | `#2c2d32` | `neutral-70` | `0.02581` | Hover-плитки, верхний уровень карточек |

#### Матрица контраста текста Dark Theme (`CR:1` на всех 8 поверхностях):
| Текстовый токен | Hex | `--bg-app` `#121214` | `--bg-chat` `#1a1a1e` | `--surface-input` `#1e1f22` | `--surface-1` `#202024` | `--surface-sunken` `#222327` | `--surface-2` `#242429` | `--surface-3` `#28282d` | `--surface-4` `#2c2d32` | Уровень WCAG 2.2 |
|---|---|---|---|---|---|---|---|---|---|---|
| `--text-strong` | `#fbfbfb` | `18.08` | `16.77` | `15.93` | `15.69` | `15.17` | `14.93` | `14.18` | `13.28` | **AAA (`>= 7.0:1`)** |
| `--text` | `#efeff1` | `16.29` | `15.11` | `14.35` | `14.14` | `13.67` | `13.45` | `12.77` | `11.96` | **AAA (`>= 7.0:1`)** |
| `--text-subtle` | `#abacb2` | `8.26` | `7.66` | `7.28` | `7.17` | `6.93` | `6.82` | `6.48` | `6.07` | **AA (`>= 4.5:1`)** |
| `--text-muted` | `#96979e` | `6.43` | `5.96` | `5.66` | `5.58` | `5.40` | `5.31` | `5.04` | `4.72` | **AA (`>= 4.5:1`)** |
| `--text-faint` | `#81828a` | `4.89` | `4.54` | `4.31` (запрет) | `4.25` (запрет) | `4.11` (запрет) | `4.04` (запрет) | `3.84` (запрет) | `3.59` (запрет) | **AA только на `--bg-app`/`--bg-chat`** |
| `--text-link` / `--info` | `#4d96ee` | `6.16` | `5.72` | `5.43` | `5.35` | `5.18` | `5.09` | `4.84` | `4.52` | **AA (`>= 4.5:1`)** |
| `--text-accent` | `#a5b0ff` | `9.14` | `8.48` | `8.05` | `7.93` | `7.67` | `7.55` | `7.17` | `6.71` | **AA (`>= 4.5:1`)** |
| `--danger-text` | `#ff9a9e` | `9.24` | `8.57` | `8.14` | `8.02` | `7.75` | `7.63` | `7.25` | `6.78` | **AA (`>= 4.5:1`)** |
| `--warning-text` | `#fdb833` | `10.76` | `9.99` | `9.48` | `9.34` | `9.04` | `8.89` | `8.44` | `7.90` | **AAA (`>= 7.0:1`)** |
| `--positive-text` | `#57d98a` | `10.42` | `9.66` | `9.18` | `9.04` | `8.75` | `8.61` | `8.17` | `7.65` | **AAA (`>= 7.0:1`)** |

#### Пары текст-на-заливке и нетекстовые UI-индикаторы Dark Theme:
- `--text-on-accent` (`#ffffff`) на `--accent` (`#5865f2`): **`4.61:1`** (AA Pass); на `--accent-hover` (`#4654c0`): **`6.38:1`**; на `--accent-active` (`#3c45a5`): **`8.12:1`** (AAA Pass).
- `--text-on-danger` (`#ffffff`) на `--danger-fill` (`#cf3a3f`): **`4.85:1`** (AA Pass; исходный `--danger` `#da3e44` даёт `4.42:1 < 4.5:1` и поэтому используется только для иконок/границ `>= 3.0:1`, а для заливки кнопок и бейджей с белым текстом обязателен `--danger-fill` `#cf3a3f`). На `--danger-fill-hover` (`#b92d33`): **`6.02:1`**.
- `--text-on-warning` (`#121214`) на `--warning` (`#fdb833`): **`10.76:1`** (AAA Pass).
- Кольцо фокуса `--focus-ring` (`#a5b0ff`): **`6.71:1 .. 9.14:1`** на всех 8 поверхностях (`>= 3.0:1` SC 2.4.11 Pass).
- Статусы (`--status-online` `#3d9e60` = `4.09..5.58:1`, `--status-idle` `#fdb833` = `7.90..10.76:1`, `--status-dnd` `#da3e44` = `3.11..4.24:1`, `--status-offline` `#80848e` = `3.67..5.00:1`): все проходят порог `>= 3.0:1` на всех поверхностях.
- Палитра аватаров `hsl(h, 45%, 32%)` с `--avatar-fg` (`#ffffff`): минимум **`4.77:1`** (при `hue=60°`), при дефолтном `hue=212°` — **`8.43:1`**, максимум **`12.01:1`** (`hue=240°`) — все `360` оттенков проходят `>= 4.5:1`.

---

### 3.2. Тема 2: Light Theme (`[data-theme="light"]`, `57` переопределений в `waves/RW-40/theme-spec.json`)

Light-тема строится на светлом конце шкалы `neutral-1..7` из `/home/sanya/re-deep/ui/tokens.json` и переопределяет ровно `57` семантических цветовых токенов (геометрия, типографика и радиусы наследуются без изменений).

#### Поверхности Light Theme (`8` уровней):
| Токен | Hex | Эталон `ui/tokens.json` | Назначение |
|---|---|---|---|
| `--bg-app` | `#f7f7f8` | `neutral-3` | Рейл серверов и внешняя подложка |
| `--bg-chat` | `#ffffff` | `neutral-1` (`white-500`) | Основное полотно чата |
| `--surface-input` | `#ffffff` | `neutral-1` | Поле ввода композера и поиска |
| `--surface-1` | `#efeff1` | `neutral-5` | Дерево каналов и панель участников |
| `--surface-sunken` | `#f3f3f4` | `neutral-4` | Голосовая панель и подвал |
| `--surface-2` | `#f3f3f4` | `neutral-4` | Модалки, карточки, вложения |
| `--surface-3` | `#ebeced` | `neutral-6` | Меню и выпадающие списки |
| `--surface-4` | `#e8e8ea` | `neutral-7` | Hover-плитки и чипы |

#### Матрица контраста текста Light Theme (`160` проверенных пар в `theme-spec.json`):
| Текстовый токен | Hex | `--bg-chat` `#ffffff` | `--bg-app` `#f7f7f8` | `--surface-2` `#f3f3f4` | `--surface-1` `#efeff1` | `--surface-3` `#ebeced` | `--surface-4` `#e8e8ea` | Уровень WCAG 2.2 |
|---|---|---|---|---|---|---|---|---|
| `--text-strong` | `#323339` (`neutral-67`) | `12.59` | `11.79` | `11.40` | `11.00` | `10.62` | `10.29` | **AAA (`>= 7.0:1`)** |
| `--text` | `#3f4048` (`neutral-60`) | `10.30` | `9.64` | `9.33` | `9.00` | `8.68` | `8.42` | **AAA (`>= 7.0:1`)** |
| `--text-subtle` | `#5f606a` (`neutral-46`) | `6.24` | `5.84` | `5.65` | `5.45` | `5.26` | `5.10` | **AA (`>= 4.5:1`)** |
| `--text-muted` | `#696a73` (`neutral-43`) | `5.37` | `5.02` | `4.86` | `4.69` | `4.53` | `4.39` (запрет) | **AA на `--bg-app..--surface-3`** |
| `--text-faint` | `#70717a` (`neutral-41`) | `4.85` | `4.54` | `4.39` (запрет) | `4.24` (запрет) | `4.10` (запрет) | `3.97` (запрет) | **AA на `--bg-app`/`--bg-chat`** |
| `--text-link` / `--info` | `#0363c1` (`blue-530`) | `5.90` | `5.52` | `5.34` | `5.16` | `4.98` | `4.82` | **AA (`>= 4.5:1`)** |
| `--text-accent` | `#4654c0` (`brand-560`) | `6.38` | `5.98` | `5.78` | `5.58` | `5.38` | `5.22` | **AA (`>= 4.5:1`)** |
| `--danger-text` | `#bd2934` (`red-560`) | `5.95` | `5.57` | `5.39` | `5.20` | `5.02` | `4.86` | **AA (`>= 4.5:1`)** |
| `--warning-text` | `#945300` (`yellow-660`) | `6.01` | `5.63` | `5.44` | `5.25` | `5.07` | `4.91` | **AA (`>= 4.5:1`)** |
| `--positive-text` | `#01713a` (`green-560`) | `6.13` | `5.74` | `5.55` | `5.35` | `5.17` | `5.01` | **AA (`>= 4.5:1`)** |

#### Нетекстовые и парные токены Light Theme:
- `--focus-ring` (`#4654c0`): **`5.22:1 .. 6.38:1`** на всех 8 поверхностях (`>= 3.0:1` Pass).
- Статусы (`--status-online` `#158246` = `4.00..4.89:1`, `--status-idle` `#b76e00` = `3.15..3.85:1`, `--status-dnd` `#cf3a3f` = `3.97..4.85:1`, `--status-offline` `#70717a` = `3.97..4.85:1`): все проходят `>= 3.0:1` на всех 8 поверхностях.
- Заливки кнопок: `--accent` (`#5865f2`) с `--text-on-accent` (`#ffffff`) = **`4.61:1`**; `--danger-fill` (`#cf3a3f`) с `--text-on-danger` (`#ffffff`) = **`4.85:1`**; `--warning` (`#b76e00`) с `--text-on-warning` (`#ffffff`) = **`3.85:1`** (крупный текст/иконка `>= 3.0:1`).

---

### 3.3. Тема 3: Midnight / OLED (Onyx) Theme (`[data-theme="midnight"]`)

Midnight (OLED / Onyx) использует ультра-тёмные поверхности `neutral-88..100` и `primary-830..900` из `/home/sanya/re-deep/ui/tokens.json` для энергоэффективных OLED-экранов и максимального контраста:

#### Поверхности Midnight (OLED) Theme (`8` уровней):
| Токен | Hex | Эталон `ui/tokens.json` | Относительная яркость $L$ | Назначение |
|---|---|---|---|---|
| `--bg-app` | `#000000` | `neutral-100` (`black-500`) | `0.00000` | Истинно-чёрная OLED подложка и рейл серверов |
| `--surface-input` | `#050506` | `neutral-98` (`primary-860`) | `0.00157` | Поле ввода композера и поиска |
| `--bg-chat` | `#070709` | `neutral-97` | `0.00222` | Центральная колонка чата |
| `--surface-sunken` | `#0a0a0c` | `neutral-96` | `0.00315` | Голосовая панель и подвал |
| `--surface-1` | `#0c0c0e` | `neutral-95` (`primary-830`) | `0.00380` | Дерево каналов и список участников |
| `--surface-2` | `#101013` | `neutral-93` | `0.00523` | Модалки, поповеры, карточки |
| `--surface-3` | `#151518` | `neutral-90` | `0.00744` | Контекстные меню и тултипы |
| `--surface-4` | `#17181b` | `neutral-88` | `0.00877` | Hover-плитки и активные чипы |

#### Матрица контраста текста Midnight (OLED) Theme (`CR:1` на всех 8 поверхностях):
| Текстовый токен | Hex | `--bg-app` `#000000` | `--surface-input` `#050506` | `--bg-chat` `#070709` | `--surface-sunken` `#0a0a0c` | `--surface-1` `#0c0c0e` | `--surface-2` `#101013` | `--surface-3` `#151518` | `--surface-4` `#17181b` | Уровень WCAG 2.2 |
|---|---|---|---|---|---|---|---|---|---|---|
| `--text-strong` | `#fbfbfb` | `20.29` | `19.68` | `19.44` | `19.10` | `18.87` | `18.38` | `17.66` | `17.16` | **AAA (`>= 7.0:1`)** |
| `--text` | `#efeff1` | `18.29` | `17.74` | `17.52` | `17.22` | `17.01` | `16.57` | `15.92` | `15.46` | **AAA (`>= 7.0:1`)** |
| `--text-subtle` | `#abacb2` | `9.28` | `9.00` | `8.89` | `8.73` | `8.63` | `8.40` | `8.07` | `7.84` | **AAA (`>= 7.0:1`)** |
| `--text-muted` | `#96979e` | `7.22` | `7.00` | `6.92` | `6.80` | `6.71` | `6.54` | `6.28` | `6.10` | **AA (`>= 4.5:1`)** |
| `--text-faint` | `#81828a` | `5.49` | `5.33` | `5.26` | `5.17` | `5.11` | `4.97` | `4.78` | `4.64` | **AA на ВСЕХ 8 поверхностях (`>= 4.5:1`)** |
| `--text-link` / `--info` | `#4d96ee` | `6.92` | `6.71` | `6.63` | `6.51` | `6.43` | `6.27` | `6.02` | `5.85` | **AA (`>= 4.5:1`)** |
| `--text-accent` | `#a5b0ff` | `10.25` | `9.95` | `9.82` | `9.65` | `9.54` | `9.29` | `8.92` | `8.67` | **AAA (`>= 7.0:1`)** |
| `--danger-text` | `#ff9a9e` | `10.37` | `10.06` | `9.93` | `9.76` | `9.64` | `9.39` | `9.02` | `8.76` | **AAA (`>= 7.0:1`)** |
| `--warning-text` | `#fdb833` | `12.08` | `11.72` | `11.58` | `11.38` | `11.24` | `10.95` | `10.52` | `10.21` | **AAA (`>= 7.0:1`)** |
| `--positive-text` | `#57d98a` | `11.70` | `11.34` | `11.21` | `11.01` | `10.88` | `10.60` | `10.18` | `9.89` | **AAA (`>= 7.0:1`)** |

---

## 4. Контракты 12 Mana-Компонентов (`frontend-d03.md` §5–§6 -> `tokens.css`)

| № | Компонент (`frontend-d03.md`) | Селектор / Атрибут | Канонические токены `tokens.css` | Значения |
|---|---|---|---|---|
| 1 | **Button** | `data-mana-component="button"`, `.btn` | `--btn-height-xs`, `--btn-height-sm`, `--btn-height-md`, `--btn-height-lg`, `--btn-radius-pill` | `24px`, `32px`, `38px`, `44px`, `var(--radius-round)` (`2147483647px`) |
| 2 | **TextInput** | `data-mana-component="text-input"`, `.input` | `--input-height-sm`, `--input-height-md`, `--input-height-lg`, `--input-pad-x`, `--input-radius`, `--surface-input`, `--danger-border` | `32px`, `40px`, `44px`, `12px`, `8px`, `#1e1f22`, `#f8787c` |
| 3 | **Menu (Mana)** | `.menu_c1e9c4`, `[role="menu"]` | `--menu-pad`, `--menu-border`, `--menu-min-w`, `--menu-max-w`, `--menu-item-h`, `--menu-radius` | `8px`, `1px`, `188px`, `320px`, `32px`, `8px` |
| 4 | **ContextMenu** | `role="menu"`, `message-*` | Наследует `--menu-pad` (`8px`), `--menu-border` (`1px`), `--z-menu` (`400`) | Слой поповера `z-index: 400` |
| 5 | **Modal** | `data-mana-component="layer-modal"`, `role="dialog"` | `--modal-w-sm`, `--modal-w-md`, `--modal-w-lg`, `--modal-pad-y`, `--modal-pad-x`, `--modal-radius`, `--surface-modal` | `442px`, `602px`, `800px`, `16px`, `20px`, `12px`, `var(--surface-2)` (`#242429`) |
| 6 | **Popover** | `data-mana-component="popover"`, `.popout` | `--popout-w-emoji`, `--popout-h-emoji`, `--popout-w-Reaction`, `--popout-w-profile`, `--popout-w-pins`, `--popout-w-search` | `424px`, `440px`, `336px`, `300px`, `420px`, `418px` |
| 7 | **Tooltip** | `.tooltip_c36707`, `[role="tooltip"]` | `--tooltip-pad-x`, `--tooltip-pad-y`, `--tooltip-max-w`, `--tooltip-radius`, `--surface-tooltip` | `10px`, `6px`, `240px`, `6px`, `var(--surface-3)` (`#28282d`) |
| 8 | **Switch** | `input[type="checkbox"][role="switch"]` | `--switch-w`, `--switch-h`, `--switch-thumb`, `--switch-pad`, `--accent` (`#5865f2`), `--text-on-accent` (`#ffffff`) | `40px`, `24px`, `18px`, `3px` |
| 9 | **Slider** | `[role="slider"]`, `.slider__4c059` | `--slider-track-h`, `--slider-thumb-w`, `--slider-thumb-h` | `6px`, `12px`, `24px` (диапазоны `0..100` и `0..200`) |
| 10 | **Avatar** | `.avatar_fdd9da`, `.avatarWrapper_d8a370` | `--avatar-xs`, `--avatar-sm`, `--avatar-md`, `--avatar-lg`, `--avatar-xl`, `--status-dot-sm`, `--status-dot-md`, `--status-ring` | `16px`, `24px`, `32px`, `40px`, `80px`, `8px`, `10px`, `2px` |
| 11 | **Badge** | `.numberBadge__463b7` | `--badge-min-w`, `--badge-h`, `--badge-pad-x`, `--badge-radius`, `--badge-bg` (`var(--danger-fill)`), `--badge-fg` (`var(--text-on-danger)`) | `16px`, `16px`, `4px`, `var(--radius-round)`, `#cf3a3f`, `#ffffff` (`4.85:1`) |
| 12 | **Scroller** | `.scrollerBase_d125d2`, `.scroller_affa7e` | `--scrollbar-thin-w`, `--scrollbar-auto-w`, `--scrollbar-thumb`, `--scrollbar-thumb-hover`, `--scrollbar-track` | `6px`, `12px`, `#5f616a`, `#81828a`, `rgb(0 0 0 / 0%)` |

---

## 5. Спецификация Адаптивных Брейкпоинтов и Геометрии Shell (`tokens.css` ↔ `style.css` ↔ `design-breakpoints.test.mjs`)

Адаптивная 4-колоночная сетка `.shell` (`guild-rail` | `channel-tree` | `chat-pane` | `member-pane`) управляется через CSS Custom Properties в `rust/gui-spike/ui/tokens.css`, медиа-запросы в `rust/gui-spike/ui/style.css` и верифицируется мета-тестом `tests/meta/design-breakpoints.test.mjs` на матрице из **12 контрольных ширин**:
`2048, 1600, 1599, 1440, 1280, 1200, 1199, 960, 959, 720, 719, 600`.

### 5.1. Таблица 5 адаптивных брейкпоинтов Shell
| Диапазон ширины (`vw`) | Контрольные точки матрицы (`px`) | Медиа-запрос | `--rail-w` | `--tree-w` | `--chat-min-w` | `--member-w` | Видимость колонок (`grid-template-columns`) |
|---|---|---|---|---|---|---|---|
| **`>= 1600px`** (Широкий десктоп, эталон `layout.json` `1600×900`) | `2048`, `1600` | Базовый `:root` | `72px` | `311px` | `520px` (`1fr`, `829px` при `1600px`) | `256px` | Все 4 колонки: `72px 311px minmax(520px, 1fr) 256px` |
| **`1200px – 1599px`** (Стандартный ноутбук) | `1599`, `1440`, `1280`, `1200` | `@media (max-width: 1599px)` | `72px` | `240px` | `520px` (`1fr`) | `240px` | Все 4 колонки (компактные боковые): `72px 240px minmax(520px, 1fr) 240px` |
| **`960px – 1199px`** (Узкий десктоп / планшет альбомный) | `1199`, `960` | `@media (max-width: 1199px)` | `72px` | `240px` | `520px` (`1fr`, при `960px` чат = `648px >= 520px`) | `0px` (`.member-pane` скрыт `display: none`) | 3 колонки (`shell.members-open` открывает `.member-pane` как overlay drawer `240px`) |
| **`720px – 959px`** (Планшет портретный) | `959`, `720` | `@media (max-width: 959px)` | `72px` | `200px` | `400px` (`1fr`, при `720px` чат = `448px >= 400px`) | `0px` (скрыт) | 3 колонки с узким деревом `200px` |
| **`< 720px`** (Мобильный / Rail-only) | `719`, `600` | `@media (max-width: 719px)` | `64px` | `0px` (`.channel-tree` скрыт `display: none`) | `320px` (`1fr`) | `0px` (скрыт) | 2 колонки (`64px 1fr`); `.shell.tree-open` открывает `.channel-tree` как overlay drawer `240px` |

### 5.2. Переменная высота элементов списков (D-04) и режимы плотности (`cozy` / `compact`)
- **Каналы (`channel-item`):** `--channel-row-h: 26px` (обычный текстовый/голосовой канал, `layout.json` `h=26px`), `--channel-row-sub-h: 48px` (канал с подзаголовком активности/статуса, `layout.json` `h=48px`, прирост ровно `+22px`).
- **Участники (`member-row`):** `--member-row-h: 42px` (`layout.json` `232×42px`), аватар `--avatar-md: 32px`.
- **Плотность сообщений (`data-density`):**
  - `cozy` (по умолчанию): `--msg-pad-x: 64px`, `--msg-pad-y: 2px`, `--msg-group-gap: 16px`, `--msg-avatar-size: 40px` (`--avatar-lg`).
  - `compact` (`:root[data-density="compact"]`): `--msg-pad-x: 80px`, `--msg-pad-y: 1px`, `--msg-group-gap: 4px`, `--msg-avatar-size: 0px` (аватары скрыты, таймштамп в левом поле `80px`).

---

## 6. Трассировка с Автоматическими Гейтами и Мета-Тестами

| Тестовый файл | Проверяемые инварианты |
|---|---|
| `tests/meta/design-tokens-contrast.test.mjs` | Полный синтаксический разбор `204` токенов `tokens.css`, глубина `var(--*) <= 8`, WCAG 2.2 AA (`>= 4.5:1`, `>= 3.0:1`) и AAA (`>= 7.0:1`) по Dark, Light (`57` токенов, `160` пар) и Midnight (`8` поверхностей × `10` текстовых токенов), сверка с `/home/sanya/re-deep/frontend-d03.md`, `token-map.js` (`7` коллизий) и брейкпоинтами |
| `tests/meta/design-breakpoints.test.mjs` | Синхронность 5 брейкпоинтов (`1600`, `1200`, `960`, `720`, `<720`), матрицы 12 ширин (`2048..600`), переменной высоты строк D-04 (`26px`, `48px`, `42px`) и инвариантов `min-width: 0` / `overflow-wrap: anywhere` |
| `tests/unit/token-map.test.mjs` | Юнит-тесты `CANONICAL_TOKEN_MAP` (`25` токенов), `RESOLVED_COLLISIONS` (`7` коллизий), `auditCssFile` и `auditAllCss` |
