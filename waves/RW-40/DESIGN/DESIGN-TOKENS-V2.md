# Отчёт G3 Design Lead: Design Tokens V2, Аудит Контрастности WCAG 2.2 (Dark / Light / Midnight) и Синхронизация Брейкпоинтов

- **Карточка:** `TASK-631` (`task_f0b756c0ba40`, dispatch `ctx_1534ef8c38a6`)
- **Отдел / Роль:** `G3 Design (UX/UI & Specs)`
- **Ветка:** `design/task-631-design-tokens-v2` (от `unstable`)
- **Дата:** `2026-11-07`
- **Статус:** `Ready for Product & Technical Review`

---

## 1. Резюме Выполненной Работы

1. **Проведён полный аудит дизайн-токенов V2 (`204` канонических `:root`-переменных в `rust/gui-spike/ui/tokens.css`):**
   - Проверены все 15 функциональных секций (`§0..§14`), метки происхождения `[M]` (`/home/sanya/re-deep/frontend-d03.md`), `[T]` (`/home/sanya/re-deep/ui/tokens.json`, `4795` переменных), `[L]` (`waves/RW-37/RE/layout.json`), `[A]` (WCAG 2.2 AA/AAA) и `[F]` (фокус/интеракции).
   - Подтверждено отсутствие циклов `var(--*)` (максимальная глубина разрешения `<= 2` шага при лимите `8`), отсутствие `!important` и внешних ресурсов.
2. **Выполнен математический аудит контрастности WCAG 2.2 AA/AAA в трёх цветовых темах:**
   - **Dark Theme (`tokens.css`):** проверены `10` текстовых токенов × `8` поверхностей (`--bg-app` `#121214` .. `--surface-4` `#2c2d32`), `6` пар текст-на-заливке кнопок/бейджей (`--accent`, `--danger-fill`, `--warning`), `10` нетекстовых UI-токенов (`--focus-ring`, `--status-*`, `--scrollbar-thumb`), `360` оттенков палитры аватаров `hsl(0..359, 45%, 32%)` и `16` композиций полупрозрачных состояний `--state-hover`/`--state-active`/`--state-selected`.
   - **Light Theme (`waves/RW-40/theme-spec.json`):** проверены все `57` переопределений и `160` нормативных пар контраста (`--bg-chat` `#ffffff`, `--bg-app` `#f7f7f8`, `--surface-1..4` `#efeff1..#e8e8ea`).
   - **Midnight / OLED Theme (`neutral-88..100` из `ui/tokens.json`):** проверены `8` ультра-тёмных поверхностей (`--bg-app` `#000000` .. `--surface-4` `#17181b`), где все `10` текстовых токенов (включая `--text-faint` `#81828a`, `4.64..5.49:1`) проходят порог WCAG 2.2 AA (`>= 4.5:1`), а `6` токенов (`--text-strong`, `--text`, `--text-subtle`, `--text-accent`, `--danger-text`, `--positive-text`) проходят порог WCAG 2.2 AAA (`>= 7.0:1`).
3. **Синхронизированы каноническая карта `token-map.js`, брейкпоинты и мета-тесты:**
   - Оформлена спецификация `docs/specs/design-tokens-v2.md`.
   - Создан мета-тест `tests/meta/design-tokens-contrast.test.mjs` (`15` тестовых кейсов в `4` наборах) и зарегистрирован в `TDD-PLAN.md` §8.

---

## 2. Таблица Разрешённых Коллизий Имен Токенов (`RESOLVED_COLLISIONS` в `token-map.js`)

| № | Устаревшее имя (Legacy) | Канонический токен Discord (`CANONICAL_TOKEN_MAP`) | Значение в Discord | Внутренний токен `tokens.css` | Причина переименования |
|---|---|---|---|---|---|
| 1 | `--bg-secondary` | `--background-secondary` | `#2b2d31` | `--surface-1` (`#202024`) | Выравнивание с 3-уровневой шкалой поверхностей Discord Desktop |
| 2 | `--bg-tertiary` | `--background-tertiary` | `#1e1f22` | `--bg-app` (`#121214`) / `--surface-input` (`#1e1f22`) | Исключение неоднозначности между фоном рейла и полем ввода |
| 3 | `--text-muted` | `--text-muted-discord` | `#949ba4` | `--text-muted` (`#96979e`) | Разделение сырого эталона Discord и выверенного под AA `--surface-4` (`4.72:1`) токена |
| 4 | `--border-subtle` | `--background-modifier-accent` | `rgba(78, 80, 88, 0.48)` | `--border-subtle` (`rgb(151 152 159 / 12%)`) | Приведение разделителей к единому модификатору поверхности |
| 5 | `--brand-hover` | `--brand-560` | `#4752c4` | `--accent-hover` (`#4654c0`) | Привязка к номерной шкале `brand-500/560/600` из `ui/tokens.json` |
| 6 | `--danger-fg` | `--status-danger` | `#f23f43` | `--danger` (`#da3e44`) / `--danger-text` (`#ff9a9e`) | Разделение иконочного `--danger` (`>= 3.0:1`), заливки `--danger-fill` (`#cf3a3f`, `4.85:1`) и текста `--danger-text` (`#ff9a9e`, `>= 6.78:1`) |
| 7 | `--surface-overlay` | `--background-floating` | `#111214` | `--surface-popout` (`#242429`) | Унификация плавающих слоёв поповеров и контекстных меню |

---

## 3. Сводные Результаты Аудита Контрастности WCAG 2.2

### 3.1. Dark Theme (`rust/gui-spike/ui/tokens.css`)
| Группа проверок | Количество пар | Минимальный контраст | Норматив WCAG 2.2 | Статус |
|---|---|---|---|---|
| `--text-strong` (`#fbfbfb`) и `--text` (`#efeff1`) на `8` поверхностях | `16` | `11.96:1` (`--text` на `--surface-4` `#2c2d32`) | AAA `>= 7.0:1` | **PASS** |
| Универсальные текстовые токены (`9` шт.) на `8` поверхностях | `72` | `4.52:1` (`--text-link` `#4d96ee` на `--surface-4` `#2c2d32`) | AA `>= 4.5:1` | **PASS** |
| `--text-faint` (`#81828a`) на `--bg-app` и `--bg-chat` | `2` | `4.54:1` (`--bg-chat` `#1a1a1e`), `4.89:1` (`--bg-app` `#121214`) | AA `>= 4.5:1` (запрещён на `--surface-1..4`) | **PASS** |
| Кнопки и бейджи (текст на акцентной/семантической заливке) | `6` | `4.61:1` (`#ffffff` на `--accent` `#5865f2`), `4.85:1` (`#ffffff` на `--danger-fill` `#cf3a3f`) | AA `>= 4.5:1` | **PASS** |
| Нетекстовые UI-индикаторы, статусы и `--focus-ring` (`10` × `8`) | `80` | `3.11:1` (`--danger` `#da3e44` на `--surface-4` `#2c2d32`) | AA Non-Text `>= 3.0:1` | **PASS** |
| Аватары `hsl(0..359, 45%, 32%)` с `--avatar-fg` (`#ffffff`) | `360` | `4.77:1` (`hue=60°`), `8.43:1` (`hue=212°`) | AA `>= 4.5:1` | **PASS** |
| Композитинг состояний `--state-hover`/`--state-active`/`--state-selected` | `31` | `7.98:1` (`--text`), `4.52:1` (`--text-subtle` / `--text-muted`) | AA `>= 4.5:1` | **PASS** |

### 3.2. Light Theme (`waves/RW-40/theme-spec.json`)
| Группа проверок | Количество пар | Минимальный контраст | Норматив WCAG 2.2 | Статус |
|---|---|---|---|---|
| Текстовые токены (`--text-strong` `#323339` .. `--positive-text` `#01713a`) | `70` | `4.53:1` (`--text-muted` на `--surface-3` `#ebeced`, `--text-faint` на `--bg-app` `#f7f7f8`) | AA `>= 4.5:1` | **PASS** |
| Пары текст-на-заливке (`--text-on-accent`, `--text-on-danger`, `--badge-fg`) | `10` | `4.61:1` (`#ffffff` на `#5865f2`) | AA `>= 4.5:1` | **PASS** |
| Нетекстовые UI-токены (`--focus-ring` `#4654c0`, `--status-*`, `--scrollbar-thumb`) | `80` | `3.15:1` (`--warning` `#b76e00` на `--surface-4` `#e8e8ea`) | AA Non-Text `>= 3.0:1` | **PASS** |
| **Итого по матрице `theme-spec.json`** | **`160`** | **`>= норматива по всем 160 парам`** | **WCAG 2.2 AA** | **PASS (160/160)** |

### 3.3. Midnight / OLED Theme (`neutral-88..100`)
| Группа проверок | Количество пар | Минимальный контраст | Норматив WCAG 2.2 | Статус |
|---|---|---|---|---|
| Все `10` текстовых токенов (включая `--text-faint` `#81828a`) на `8` поверхностях | `80` | `4.64:1` (`--text-faint` на `--surface-4` `#17181b`) | AA `>= 4.5:1` | **PASS (80/80)** |
| Повышенный контраст AAA (`6` токенов × `8` поверхностей) | `48` | `7.84:1` (`--text-subtle` на `--surface-4` `#17181b`), `20.29:1` (`--text-strong` на `#000000`) | AAA `>= 7.0:1` | **PASS (48/48)** |

---

## 4. Синхронизация Брейкпоинтов (`tokens.css` ↔ `style.css` ↔ `design-breakpoints.test.mjs` ↔ `design-tokens-v2.md`)

Проверена полная идентичность 5 адаптивных брейкпоинтов и 12 тестовых ширин (`2048, 1600, 1599, 1440, 1280, 1200, 1199, 960, 959, 720, 719, 600`):
- **`>= 1600px`:** `--rail-w: 72px`, `--tree-w: 311px`, `--chat-min-w: 520px`, `--member-w: 256px` (все 4 колонки видимы, эталон `waves/RW-37/RE/layout.json`).
- **`1200–1599px` (`@media (max-width: 1599px)`):** `--tree-w: 240px`, `--member-w: 240px` (4 колонки).
- **`960–1199px` (`@media (max-width: 1199px)`):** `--member-w: 0px`, `.member-pane { display: none }` (3 колонки, чат `>= 520px`, drawer `.shell.members-open`).
- **`720–959px` (`@media (max-width: 959px)`):** `--tree-w: 200px`, `--chat-min-w: 400px` (3 колонки).
- **`< 720px` (`@media (max-width: 719px)`):** `--rail-w: 64px`, `--tree-w: 0px`, `.channel-tree { display: none }` (2 колонки, drawer `.shell.tree-open`).
- **Переменная высота элементов (D-04):** `--channel-row-h: 26px`, `--channel-row-sub-h: 48px` (`+22px`), `--member-row-h: 42px`.

---

## 5. Отчёт по Строгому TDD (RED → GREEN → 5/5 Мутаций)

### 5.1. RED-прогон (до создания `docs/specs/design-tokens-v2.md` и `waves/RW-40/DESIGN/DESIGN-TOKENS-V2.md`)
Команда: `node --test tests/meta/design-tokens-contrast.test.mjs`
```text
▶ TASK-631: 4. Сверка с RE-оракулом frontend-d03.md, спекой design-tokens-v2.md и брейкпоинтами
  ✔ эталон /home/sanya/re-deep/frontend-d03.md согласован с размерами и цветами 12 Mana-компонентов в tokens.css (0.36713ms)
  ✖ docs/specs/design-tokens-v2.md и waves/RW-40/DESIGN/DESIGN-TOKENS-V2.md существуют и содержат полный контракт V2 (0.581671ms)
    AssertionError [ERR_ASSERTION]: Отсутствует спецификация docs/specs/design-tokens-v2.md (false !== true)
  ✖ брейкпоинты в tokens.css, style.css, design-breakpoints.test.mjs и design-tokens-v2.md полностью синхронны (0.1447ms)
    Error: ENOENT: no such file or directory, open '.../docs/specs/design-tokens-v2.md'
ℹ tests 15 | pass 13 | fail 2
```

### 5.2. GREEN-прогон (после создания `docs/specs/design-tokens-v2.md` и `waves/RW-40/DESIGN/DESIGN-TOKENS-V2.md`)
Команда: `node --test tests/meta/design-tokens-contrast.test.mjs`
```text
✔ TASK-631: 1. Валидация синтаксиса и цветовых форматов rust/gui-spike/ui/tokens.css
✔ TASK-631: 2. Аудит контрастности WCAG 2.2 AA/AAA — Dark Theme (tokens.css)
✔ TASK-631: 3. Аудит контрастности WCAG 2.2 — Light Theme и Midnight (OLED) Theme
✔ TASK-631: 4. Сверка с RE-оракулом frontend-d03.md, спекой design-tokens-v2.md и брейкпоинтами
ℹ tests 15 | suites 4 | pass 15 | fail 0
```

### 5.3. Мутационное тестирование (`5/5` мутаций убито)
| Мутация | Целевой инвариант | Внесённое искажение | Результат `node --test tests/meta/design-tokens-contrast.test.mjs` | Статус |
|---|---|---|---|---|
| **M1** | Dark Theme WCAG 2.2 AAA (`--text` `>= 7.0:1`) | Временная подмена `--text: #efeff1` → `--text: #555555` в `tokens.css` | Упал тест `все основные текстовые токены проходят WCAG 2.2 AA` и `--text-strong и --text проходят повышенный порог WCAG 2.2 AAA` | **KILLED** |
| **M2** | Light Theme WCAG 2.2 AA (`160` пар) | Временная подмена `--text-muted` `#696a73` → `#999999` в проверке Light-темы | Упал тест `Light Theme (57 переопределений): все текстовые, парные и нетекстовые токены проходят матрицу WCAG 2.2 AA` | **KILLED** |
| **M3** | Midnight Theme OLED (`--bg-app: #000000`) | Временная подмена `MIDNIGHT_SURFACES["--bg-app"]` `#000000` → `#606060` | Упал тест `Midnight (OLED) Theme: все 10 текстовых токенов проходят >= 4.5:1 на всех 8 поверхностях` | **KILLED** |
| **M4** | RE-оракул `frontend-d03.md` (`--modal-w-sm: 442px`) | Временная подмена `--modal-w-sm: 442px` → `400px` в `tokens.css` | Упал тест `эталон /home/sanya/re-deep/frontend-d03.md согласован с размерами и цветами 12 Mana-компонентов` | **KILLED** |
| **M5** | Синхронность брейкпоинтов (`311px` / `1600px`) | Временное удаление `311px` из `docs/specs/design-tokens-v2.md` | Упал тест `брейкпоинты в tokens.css, style.css, design-breakpoints.test.mjs и design-tokens-v2.md полностью синхронны` | **KILLED** |

Все файлы после мутационных проверок восстановлены побайтово.

---

## 6. Изменённые Файлы и Контрольные Суммы (MD5)

| Файл | MD5 | Назначение |
|---|---|---|
| `docs/specs/design-tokens-v2.md` | `db37738ea771802fa2109a31a122a164` | Спецификация Design Tokens V2, WCAG 2.2 (Dark/Light/Midnight), 12 Mana-компонентов и 5 брейкпоинтов |
| `tests/meta/design-tokens-contrast.test.mjs` | `3ba955c293733006e588c378eb95baa5` | Мета-тест синтаксиса токенов, контрастности WCAG 2.2 AA/AAA и синхронности брейкпоинтов |
| `TDD-PLAN.md` | `b19edb0896300613884348bf19eecb3c` | Регистрация `tests/meta/design-tokens-contrast.test.mjs` в §8 |
| `rust/gui-spike/ui/tokens.css` (read-only oracle) | `21d570a6bf9c4b5d77b00d3d9b3d1d08` | Канонические `204` `:root`-токена и `@media` брейкпоинты |
| `waves/RW-40/theme-spec.json` (read-only oracle) | `4bce1692512bd288667a032de1bd2f5c` | `57` переопределений и `160` пар контраста Light-темы |

---

## 7. Риски и Ограничения
- **Ограничение `--text-faint` в Dark/Light темах:** токен `--text-faint` (`#81828a` в Dark, `#70717a` в Light) проходит порог `>= 4.5:1` только на `--bg-app` и `--bg-chat`; на приподнятых поверхностях `--surface-1..4` для мелких подписей необходимо использовать `--text-muted` или `--text-subtle`. В теме Midnight (`#000000..#17181b`) `--text-faint` проходит `>= 4.5:1` на всех 8 поверхностях.
- **Заливка опасных кнопок и бейджей:** токен `--danger` (`#da3e44`) имеет контраст `4.42:1` с `#ffffff` (`< 4.5:1`), поэтому для текста на красном фоне кнопок и бейджей обязательно применение `--danger-fill` (`#cf3a3f`, контраст `4.85:1`).
