# RW-40 / G3 — TYPING-MARKDOWN-SPEC: индикатор набора (P-03-UI) и форматирование Markdown/mentions/spoilers (M-08)

Статус: дизайн-контракт (спецификация; продуктовый код не менялся). Дата: 2026-10-30.  
Владелец: **G3 Design**. Исполнители: **G2 Frontend** (UI, стили, разметка, рендерер сообщений), **G1 Core** (IPC/события Gateway, парсинг AST).  
Связанные карточки:
- `P-03-UI`: Полоса индикатора набора текста `#chat-typing-indicator` под композером (состояния, a11y, плюрализм, slowmode).
- `M-08`: Форматирование сообщений Markdown, инлайн-код, блоки кода, цитаты, спойлеры (`data-spoiler`), интерактивные пилюли упоминаний (@user, #channel, @role, @everyone/@here) в тёмной и светлой темах.

Нормативные документы:
- `rust/gui-spike/ui/tokens.css` (источник дизайн-токенов DL: поверхности, акценты, типографика, радиусы, тени, motion);
- `waves/RW-40/THEME-SPEC.md` §2 (таблицы dark → light для поверхностей, текста, границ, акцентов, WCAG AA 4.5:1), §4 (CSS-контракт селекторов `:root[data-theme="light"]`);
- `waves/RW-40/PARITY/PARITY-MATRIX.md` (строки P-03 и M-08, DoD, границы, `[S]` и `[L]` evidence);
- `waves/RW-40/RE-DEEP-REA.md` §2, §4, §8 (оракулы форматирования и разметки);
- `AGENTS.md` (правила воркеров, strict TDD, запрет изменения продуктового кода).

---

## 0. Легенда, правила и архитектурные ограничения

| Метка | Значение | Источник |
|---|---|---|
| `[D]` | Факт из дампа / live-бандла / REA | `RE-DEEP-REA.md`, `PARITY-MATRIX.md` (B162, G162, S162) |
| `[T]` | Токен проекта из `ui/tokens.css` | `rust/gui-spike/ui/tokens.css` |
| `[L]` | Значение и правила light-темы | `THEME-SPEC.md` §2, §2.6, §4 |
| `[A]` | Проектное решение / адаптация под архитектуру DL | Данный документ (проверяемо до кода) |
| `[F]` | Зафиксировано тестом | DOM/Feature-контракт |

### Базовые требования спецификации:
1. **Строгая токенизация (`100% token binding`)**:
   - Никаких сырых шестнадцатеричных цветов (`#hex`) или прямых `rgb(...)` в компонентных CSS-правилах (за исключением разрешённого hairline `1px` и токенов альфа-каналов).
   - Все свойства `color`, `background`, `border-color`, `border-radius`, `font-size`, `line-height`, `gap`, `padding` привязываются строго к переменным `var(--token)`.
2. **Отсутствие внешних ассетов**:
   - Никаких внешних веб-шрифтов, картинок или сторонних CDN. Иконки — системные UTF-8 символы или CSS-геометрия (псевдоэлементы `::before`/`::after`). Шрифты — исключительно `var(--font-sans)` и `var(--font-mono)` (`tokens.css` §5).
3. **WAI-ARIA и доступность (a11y)**:
   - Индикатор набора и медленный режим озвучиваются скринридерами без прерывания ввода пользователя (`role="status"`, `aria-live="polite"`, `aria-atomic="true"`).
   - Интерактивные спойлеры имеют клавиатурную доступность (`tabindex="0"`, `role="button"`, активация клавишами `Enter` / `Space`, ARIA-состояния `aria-expanded`).
4. **WCAG 2.2 AA контраст**:
   - Все пары «текст/фон» удовлетворяют контрасту ≥ 4.5:1 для обычного текста и ≥ 3:1 для элементов управления и границ (согласно матрице `THEME-SPEC.md` §2.6).

---

## 1. Компонент P-03-UI — Полоса индикатора набора текста `#chat-typing-indicator`

### 1.1 Расположение и геометрия
Индикатор набора текста располагается **под полем композера** внутри секции чата `.chat-main` (ниже `#composer`), занимая выделенную полосу фиксированной минимальной высоты, либо встраивается компактно под границей композера, не вызывая сдвигов и прыжков списка сообщений (`layout shift`).

- **Контейнер**: `#chat-typing-indicator` (или `.chat-typing-indicator`)
- **Высота полосы**: нормативная общая блочная высота `height: 20px` / `min-height: 20px` при `box-sizing: border-box` (включает контент `var(--line-height-small)` 16px и вертикальные отступы `padding: var(--space-1) var(--chat-pad-x)` по 2px сверху и снизу; гарантирует отсутствие layout shift при появлении/скрытии).
- **Горизонтальные отступы**: `padding: var(--space-1) var(--chat-pad-x)` (`--chat-pad-x` = 16px). Выравнивание по левой границе текстового поля композера.
- **Шрифт**: `font-family: var(--font-sans)`, `font-size: var(--font-size-small)` (12px), `line-height: var(--line-height-small)` (16px), `font-weight: var(--font-weight-normal)` (400).
- **Цвет текста по умолчанию**: `var(--text-muted)` (Dark `#96979e`, Light `#696a73` — WCAG AA ≥ 4.5:1).
- **Плавность появления/исчезновения**: `transition: opacity var(--motion-fast) var(--ease-out)` (`--motion-fast` = 120ms). При `prefers-reduced-motion: reduce` время анимации 0ms.

### 1.2 Анимация трех точек (Typing Dots Animation)
Индикатор набора содержит визуальный индикатор активности — три пульсирующие точки рядом с текстом.
- **Размер точки**: `width: 4px`, `height: 4px`, `border-radius: var(--radius-round)`.
- **Заливка точки**: `background-color: var(--text-muted)`.
- **Отступ между точками**: `gap: var(--space-1)` (2px).
- **Анимация**: Плавное изменение прозрачности `opacity` от `0.3` до `1.0` с циклическим смещением фаз:
  - Точка 1: `animation: typing-bounce 1.4s infinite ease-in-out 0ms;`
  - Точка 2: `animation: typing-bounce 1.4s infinite ease-in-out 200ms;`
  - Точка 3: `animation: typing-bounce 1.4s infinite ease-in-out 400ms;`
  - При `prefers-reduced-motion: reduce`: точки статичны, анимация отключена (`animation: none; opacity: 0.8`).

### 1.3 DOM-структура и WAI-ARIA контракт
```html
<div id="chat-typing-indicator"
     class="chat-typing-indicator"
     role="status"
     aria-live="polite"
     aria-atomic="true"
     data-state="idle"
     hidden>
  <!-- Визуальный индикатор точек (скрыт от screen-reader через aria-hidden) -->
  <span class="typing-dots" aria-hidden="true">
    <span class="typing-dot"></span>
    <span class="typing-dot"></span>
    <span class="typing-dot"></span>
  </span>

  <!-- Текстовый контейнер для имен и плюрализма -->
  <span class="typing-text"></span>

  <!-- Контейнер для обратного отсчета slowmode (активен при data-state="slowmode") -->
  <span class="typing-slowmode" hidden>
    <span class="slowmode-icon" aria-hidden="true">⏱️</span>
    <span class="slowmode-timer"></span>
  </span>
</div>
```

### 1.4 Состояния `data-state` и CSS-селекторы

Компонент управляется дата-атрибутом `data-state` на корневом элементе:

| `data-state` | Описание | Видимость элементов | Цвета и стилизация |
|---|---|---|---|
| `idle` | Нет активных печатающих, slowmode не активен | Элемент имеет атрибут `hidden` (`display: none;`), `opacity: 0` | — |
| `active` | 1 или более пользователей печатают | `.typing-dots` и `.typing-text` видимы, `.typing-slowmode` скрыт | Текст: `var(--text-muted)`; жирные имена: `var(--text-strong)`, `font-weight: var(--font-weight-semibold)` |
| `slowmode` | Включен медленный режим в канале (кулдаун между сообщениями) | `.typing-dots` скрыты; `.typing-slowmode` виден | Фон: `var(--state-hover)`; текст: `var(--text-subtle)`, индикатор/иконка: `var(--warning)`. Контраст относительно фактического фона `var(--state-hover)`: `var(--text-subtle)` (6.1–8.3 на тёмных поверхностях, ≥ 4.5:1 WCAG AA); иконка `⏱️` |
| `error` | Ошибка отправки typing (`401`, `403`, `429`, сетевой сбой) | `.typing-dots` скрыты; текст ошибки в `.typing-text` | Текст: `var(--danger-text)` (Dark `#ff9a9e`, Light `#bd2934`) |

```css
/* Базовые стили индикатора набора */
.chat-typing-indicator {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 20px;
  min-height: 20px;
  padding: var(--space-1) var(--chat-pad-x);
  color: var(--text-muted);
  font-family: var(--font-sans);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  box-sizing: border-box;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: none;
  pointer-events: none;
}

.chat-typing-indicator[hidden] {
  display: none;
}

/* Точки */
.chat-typing-indicator .typing-dots {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  margin-right: var(--space-1);
}

.chat-typing-indicator .typing-dot {
  width: 4px;
  height: 4px;
  border-radius: var(--radius-round);
  background-color: var(--text-muted);
  opacity: 0.4;
  animation: typing-pulse 1.4s infinite ease-in-out;
}

.chat-typing-indicator .typing-dot:nth-child(2) {
  animation-delay: 200ms;
}

.chat-typing-indicator .typing-dot:nth-child(3) {
  animation-delay: 400ms;
}

@keyframes typing-pulse {
  0%, 80%, 100% {
    opacity: 0.3;
    transform: scale(0.8);
  }
  40% {
    opacity: 1;
    transform: scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .chat-typing-indicator .typing-dot {
    animation: none;
    opacity: 0.7;
    transform: none;
  }
}

/* Имена пользователей в тексте индикатора набора */
.chat-typing-indicator .typing-user-name {
  color: var(--text-strong);
  font-weight: var(--font-weight-semibold);
}

/* Состояние: Slowmode */
/* THEME-SPEC.md запрещает использовать базовый --warning как цвет текста на светлом (1.74:1).
 * Для текста используется существующий в tokens.css токен var(--text-subtle)
 * (контраст 6.1–8.3 на тёмных поверхностях, WCAG AA ≥ 4.5:1), а var(--warning) — для иконки/индикатора.
 */
.chat-typing-indicator[data-state="slowmode"] {
  background-color: var(--state-hover);
  color: var(--text-subtle);
  border-radius: var(--radius-xs);
}

:root[data-theme="light"] .chat-typing-indicator[data-state="slowmode"] {
  background-color: var(--state-hover);
  color: var(--text-subtle);
}

.chat-typing-indicator[data-state="slowmode"] .typing-dots {
  display: none;
}

.chat-typing-indicator .typing-slowmode {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

/* Состояние: Ошибка */
.chat-typing-indicator[data-state="error"] {
  color: var(--danger-text);
}

.chat-typing-indicator[data-state="error"] .typing-dots {
  display: none;
}
```

### 1.5 Правила русского плюрализма и шаблоны строк
Логика формирования текста строго соблюдает правила русского языка для перечисления участников набора.  
Сам текущий пользователь (self) исключается из списка печатающих. Имена экранируются для защиты от HTML-инъекций.

| Количество печатающих | Правило формирования текста | Пример вывода в UI | Разметка (`.typing-text`) |
|---|---|---|---|
| **0 участников** | Индикатор переходит в `data-state="idle"` и скрывается (`hidden`) | — | *(пусто)* |
| **1 участник** | `<Имя> печатает…` | **Alice** печатает… | `<strong class="typing-user-name">Alice</strong> печатает…` |
| **2 участника** | `<Имя1> и <Имя2> печатают…` | **Alice** и **Bob** печатают… | `<strong class="typing-user-name">Alice</strong> и <strong class="typing-user-name">Bob</strong> печатают…` |
| **3 участника** | `<Имя1>, <Имя2> и <Имя3> печатают…` | **Alice**, **Bob** и **Charlie** печатают… | `<strong class="typing-user-name">Alice</strong>, <strong class="typing-user-name">Bob</strong> и <strong class="typing-user-name">Charlie</strong> печатают…` |
| **> 3 участников (N)** | `Несколько человек печатают…` | Несколько человек печатают… | `Несколько человек печатают…` |

> [!NOTE]
> Если у пользователя отсутствует `name` (или пустая строка), используется локализованный фоллбек `"Пользователь"`.
> Если одновременно активен медленный режим (slowmode), приоритет отображения следующий:
> - Если таймер slowmode > 0: отображается состояние `slowmode` (блокирует ввод и показывает обратный отсчет).
> - Если таймер slowmode истек (0 с) и есть печатающие: отображается состояние `active`.

### 1.6 Отображение таймера slowmode (Медленный режим)
Таймер кулдауна `POST /channels/{id}/typing` возвращает `TypingCooldowns { messageSendCooldownMs, threadCreateCooldownMs }`.  
Отображение обратного отсчета в `.slowmode-timer`:

1. **Формат времени**:
   - `> 60 сек`: `ММ:СС` (например, `Медленный режим: 01:45`)
   - `≤ 60 сек`: целые секунды с плюрализмом русского языка:
     - 1, 21, 31, 41, 51 секунда: `Медленный режим: следующее сообщение через N секунду` (или кратко: `Медленный режим: 1 с.`)
     - 2..4, 22..24, 32..34, 42..44, 52..54 секунды: `Медленный режим: следующее сообщение через N секунды` (`N с.`)
     - 5..20, 25..30, 35..40, 45..50, 55..60 секунд: `Медленный режим: следующее сообщение через N секунд` (`N с.`)
2. **Шаблон вывода по умолчанию**:
   - `Медленный режим: {time}` (например, `⏱️ Медленный режим: 5 с.`).
3. **Окончание таймера**:
   - При `remainingMs <= 0` элемент `.typing-slowmode` скрывается, `data-state` сбрасывается в `idle` (или `active`, если в этот момент другие участники печатают).

---

## 2. Компонент M-08 — Форматирование Markdown, упоминаний и спойлеров

Рендеринг текста сообщений базируется на безопасном парсинге Markdown в AST с последующей генерацией строго типизированных HTML-элементов. Все неформатированные фрагменты проходят обязательное экранирование (`textContent`/HTML entities).

### 2.1 Инлайн-код (`inline-code`)
- **Синтаксис Markdown**: `` `код` ``
- **DOM-элемент**: `<code class="md-inline-code">код</code>`
- **Токены стилизации**:
  - Шрифт: `font-family: var(--font-mono)`.
  - Размер шрифта: `font-size: var(--font-size-ui)` (14px, токен `tokens.css` §5; 0.875 от базового `var(--font-size-body)` 16px).
  - Фон: `background-color: var(--surface-sunken)` (Dark: `#222327`, Light: `#f3f3f4`).
  - Граница: `border: 1px solid var(--border-subtle)` (Dark: `rgb(151 151 159 / 12%)`, Light: `rgb(0 0 0 / 8%)`).
  - Цвет текста: `color: var(--text-strong)` (Dark: `#fbfbfb`, Light: `#323339`).
  - Внутренние отступы: `padding: var(--space-1) var(--space-2)` (2px 4px).
  - Скругление: `border-radius: var(--radius-xs)` (4px).

```css
.md-inline-code {
  font-family: var(--font-mono);
  font-size: var(--font-size-ui);
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-xs);
  background-color: var(--surface-sunken);
  border: 1px solid var(--border-subtle);
  color: var(--text-strong);
  white-space: pre-wrap;
  word-break: break-word;
}
```

### 2.2 Блоки кода (`code-block`)
- **Синтаксис Markdown**:
  ````markdown
  ```rust
  fn main() {
      println!("Hello, DL!");
  }
  ```
  ````
- **DOM-элемент**:
  ```html
  <div class="md-code-block-wrapper">
    <div class="md-code-header">
      <span class="md-code-lang">rust</span>
      <button type="button" class="btn btn-ghost btn-sm md-code-copy" data-action="copy-code" title="Скопировать код" aria-label="Скопировать код">
        <span class="copy-icon" aria-hidden="true">📋</span>
        <span class="copy-label">Копировать</span>
      </button>
    </div>
    <pre class="md-code-block"><code class="md-code-content" data-lang="rust">...</code></pre>
  </div>
  ```
- **Токены стилизации**:
  - Контейнер обёртки: `background-color: var(--surface-sunken)`, `border: 1px solid var(--border-subtle)`, `border-radius: var(--radius-sm)` (8px).
  - Шапка (язык и кнопка копирования): `height: var(--btn-height-sm)` (28px), `padding: 0 var(--space-3)`, `border-bottom: 1px solid var(--border-subtle)`, `color: var(--text-muted)`.
  - Тело блока (`pre`): `font-family: var(--font-mono)`, `font-size: var(--font-size-ui)` (14px), `line-height: var(--line-height-ui)` (18px), `padding: var(--space-3)`, `overflow-x: auto`.
  - Скроллбар: `scrollbar-width: thin`, `scrollbar-color: var(--scrollbar-thumb) var(--scrollbar-track)`.
  - Цвет текста кода: `color: var(--text)`.

```css
.md-code-block-wrapper {
  margin: var(--space-2) 0;
  border-radius: var(--radius-sm);
  background-color: var(--surface-sunken);
  border: 1px solid var(--border-subtle);
  overflow: hidden;
  max-width: 100%;
}

.md-code-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: var(--btn-height-sm);
  padding: 0 var(--space-3);
  background-color: var(--surface-2);
  border-bottom: 1px solid var(--border-subtle);
}

.md-code-lang {
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: var(--letter-spacing-caps);
}

.md-code-copy {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0 var(--space-2);
  height: calc(var(--btn-height-sm) - var(--space-2));
  font-size: var(--font-size-caption);
  color: var(--text-muted);
  background: transparent;
  border: none;
  cursor: pointer;
  border-radius: var(--radius-xs);
}

.md-code-copy:hover {
  color: var(--text-strong);
  background-color: var(--state-hover);
}

.md-code-block {
  margin: 0;
  padding: var(--space-3);
  overflow-x: auto;
  font-family: var(--font-mono);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  color: var(--text);
  tab-size: 4;
}

.md-code-content {
  font-family: inherit;
  font-size: inherit;
  background: transparent;
  padding: 0;
  border: none;
}
```

### 2.3 Цитаты (`> quote`)
- **Синтаксис Markdown**: `> Текст цитаты`
- **DOM-элемент**: `<blockquote class="md-quote"><div class="md-quote-content">Текст цитаты</div></blockquote>`
- **Токены стилизации**:
  - Левая полоса (border-left): `border-left: 4px solid var(--accent)` (Dark: `#5865f2`, Light: `#5865f2`). В нейтральном варианте цитаты допустим `var(--border-strong)`.
  - Внутренний отступ: `padding: var(--space-1) var(--space-4)`.
  - Внешний отступ: `margin: var(--space-1) 0`.
  - Цвет текста: `color: var(--text-subtle)` (Dark: `#abacb2`, Light: `#5f606a` — контраст WCAG AA ≥ 5.10).
  - Скругление углов: `border-radius: 0 var(--radius-xs) var(--radius-xs) 0`.

```css
.md-quote {
  margin: var(--space-1) 0;
  padding: var(--space-1) var(--space-4);
  border-left: 4px solid var(--accent);
  border-radius: 0 var(--radius-xs) var(--radius-xs) 0;
  background: transparent;
  color: var(--text-subtle);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.md-quote-content {
  overflow-wrap: break-word;
}
```

### 2.4 Спойлеры (`data-spoiler`) и доступность (a11y)
- **Синтаксис Markdown**: `||секретный текст||`
- **DOM-элемент**:
  ```html
  <span class="md-spoiler"
        data-spoiler="hidden"
        role="button"
        tabindex="0"
        aria-expanded="false"
        aria-label="Спойлер, нажмите для показа">
    <span class="md-spoiler-content">секретный текст</span>
  </span>
  ```
- **Поведение и доступность**:
  - По умолчанию спойлер закрыт: `data-spoiler="hidden"`, контент заблюрен / залит маской, текст невидим.
  - При клике мышью или нажатии клавиш `Enter` / `Space` спойлер раскрывается: `data-spoiler="revealed"`, атрибут `aria-expanded="true"`, контент становится читаемым.
  - Повторный клик не скрывает спойлер (паритет с поведением эталона Discord: однажды раскрытый спойлер остается открытым до обновления контекста/канала).
- **Стилизация закрытого спойлера (`hidden`)**:
  - Фон: `var(--surface-3)` (Dark: `#28282d`, Light: `#ebeced`).
  - Текст: полностью скрыт через `color: transparent; text-shadow: none; user-select: none; filter: blur(4px);` либо монолитная заливка фона поверх текста.
  - Курсор: `cursor: pointer`.
  - Фокус клавиатуры: `outline: var(--focus-ring-width) solid var(--focus-ring); outline-offset: var(--focus-ring-offset);`.
- **Стилизация открытого спойлера (`revealed`)**:
  - Фон: `var(--surface-sunken)` (с минимальным затемнением/подложкой).
  - Текст: `color: var(--text)`.
  - Фильтр снят: `filter: none; user-select: text;`.
  - `cursor: text`.

```css
.md-spoiler {
  display: inline-block;
  vertical-align: baseline;
  padding: 0 var(--space-1);
  border-radius: var(--radius-xs);
  cursor: pointer;
  transition: background-color var(--motion-fast) var(--ease-out),
              filter var(--motion-fast) var(--ease-out);
}

.md-spoiler:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

/* Скрытый спойлер */
.md-spoiler[data-spoiler="hidden"] {
  background-color: var(--surface-3);
  color: transparent;
  user-select: none;
}

.md-spoiler[data-spoiler="hidden"] .md-spoiler-content {
  opacity: 0;
  filter: blur(6px);
  display: inline-block;
}

.md-spoiler[data-spoiler="hidden"]:hover {
  background-color: var(--surface-4);
}

/* Раскрытый спойлер */
.md-spoiler[data-spoiler="revealed"] {
  background-color: var(--surface-sunken);
  color: var(--text);
  cursor: text;
}

.md-spoiler[data-spoiler="revealed"] .md-spoiler-content {
  opacity: 1;
  filter: none;
}
```

---

## 3. Пилюли упоминаний (Mentions) в Dark и Light темах

Упоминания в сообщениях рендерятся в виде интерактивных чипов-пилюль («pills»).  
При наведении курсора происходит подсветка, клик инициирует открытие профиля пользователя или переход к каналу.

### 3.1 Категории упоминаний и типы чипов
1. `@пользователь` (`mention-user`): `<span class="mention mention-user" role="button" tabindex="0" data-user-id="{id}">@Имя</span>`
2. `#канал` (`mention-channel`): `<span class="mention mention-channel" role="button" tabindex="0" data-channel-id="{id}">#название</span>`
3. `@роль` (`mention-role`): `<span class="mention mention-role" role="button" tabindex="0" data-role-id="{id}">@Роль</span>`
4. `@everyone` / `@here` (`mention-broadcast`): `<span class="mention mention-broadcast">@everyone</span>`

### 3.2 Токены и цвета: матрица тем Dark vs Light

Контраст выверен строго по правилам `THEME-SPEC.md` (§2.2, §2.3, §2.6).  
Все пилюли имеют радиус скругления `var(--radius-xs)` (4px), внутренние отступы `padding: 0 var(--space-2)` (0 4px), шрифт `font-weight: var(--font-weight-medium)` (500).

| Тип упоминания | Параметр | Dark тема (`:root`) | Light тема (`:root[data-theme="light"]`) | Источник токена и контраст |
|---|---|---|---|---|
| **@пользователь / @роль** (обычное) | Фон | `var(--accent-soft)` | `var(--accent-soft)` | `THEME-SPEC.md` §2.3 (Dark: 16%, Light: 12%) |
| | Текст | `var(--text-accent)` (`#a5b0ff`) | `var(--text-accent)` (`#4654c0`) | Контраст: Dark 6.71:1, Light 5.22:1 (≥ 4.5 AA) |
| | Hover фон | `var(--accent)` | `var(--accent)` | Паритет с эталоном при наведении |
| | Hover текст | `var(--text-on-accent)` (`#ffffff`) | `var(--text-on-accent)` (`#ffffff`) | Контраст 4.61:1 к `--accent` |
| **#канал** | Фон | `var(--accent-soft)` | `var(--accent-soft)` | Единая палитра ссылок на каналы |
| | Текст | `var(--text-accent)` (`#a5b0ff`) | `var(--text-accent)` (`#4654c0`) | WCAG AA ≥ 4.5:1 |
| **@everyone / @here** (broadcast) | Фон | `var(--accent-soft)` | `var(--accent-soft)` | Акцентная плашка |
| | Текст | `var(--text-accent)` (`#a5b0ff`) | `var(--text-accent)` (`#4654c0`) | WCAG AA ≥ 4.5:1 |
| **Упоминание текущего пользователя (`self`)** | Фон строки сообщения | `var(--accent-soft)` | `var(--accent-soft)` | Нормативная подсветка строки существующим токеном `tokens.css` (`[T] --message-highlight-background-default`); полоса: `border-left: 2px solid var(--warning)` |
| | Пилюля в сообщении | Фон: `var(--accent-soft)`, Текст: `var(--text-strong)` | Фон: `var(--accent-soft)`, Текст: `var(--text-subtle)` | Контраст: Dark > 13:1 к `--bg-chat` (≥ 4.5:1 AA), Light ≥ 4.5:1 AA (токен `var(--text-subtle)`) |

```css
/* Базовые стили пилюль упоминаний */
.mention {
  display: inline-flex;
  align-items: center;
  padding: 0 var(--space-2);
  border-radius: var(--radius-xs);
  background-color: var(--accent-soft);
  color: var(--text-accent);
  font-weight: var(--font-weight-medium);
  text-decoration: none;
  cursor: pointer;
  user-select: none;
  transition: background-color var(--motion-fast) var(--ease-out),
              color var(--motion-fast) var(--ease-out);
}

.mention:hover {
  background-color: var(--accent);
  color: var(--text-on-accent);
}

.mention:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

/* Упоминание канала с глифом */
.mention-channel::before {
  content: "#";
  margin-right: 1px;
  opacity: 0.8;
}

/* Упоминание самого себя (Self Mention) */
.mention.mention-self {
  background-color: var(--accent-soft);
  color: var(--text-strong);
}

:root[data-theme="light"] .mention.mention-self {
  background-color: var(--accent-soft);
  color: var(--text-subtle);
}

.mention.mention-self:hover {
  background-color: var(--warning);
  color: var(--text-on-warning);
}

/* Сообщение, в котором упомянули текущего пользователя */
.row[data-mentioned="true"] {
  background-color: var(--accent-soft);
  border-left: 2px solid var(--warning);
}
```

---

## 4. Контракт доступности (WAI-ARIA, Keyboard & Focus Matrix)

| Компонент | Роль / ARIA | Клавиши управления | Обработка событий и фокус |
|---|---|---|---|
| `#chat-typing-indicator` | `role="status"`, `aria-live="polite"`, `aria-atomic="true"` | — (пассивный статус-регион) | Текст обновляется динамически. Скринридер озвучивает изменения без прерывания набора сообщения пользователем. |
| `.md-spoiler` | `role="button"`, `tabindex="0"`, `aria-expanded="false|true"`, `aria-label="Спойлер"` | `Enter`, `Space` | При нажатии переключает `data-spoiler="revealed"` и `aria-expanded="true"`. Фокус остается на элементе спойлера. |
| `.mention` | `role="button"`, `tabindex="0"` | `Enter`, `Space` | Открывает поповер профиля пользователя (`#member-profile-popover`) или переключает активный канал (`channel-switch`). |
| `.md-code-copy` | `type="button"`, `aria-label="Скопировать код"` | `Enter`, `Space` | Копирует содержимое блока в `navigator.clipboard`. При успехе меняет текст на `Скопировано!`, возвращает исходный через 2 сек. |

---

## 5. План тестирования и критерии приёмки (DoD)

### 5.1 Targeted Unit & DOM Tests
Для покрытия UI-компонентов спецификации создаются модульные тесты:
1. `tests/unit/typing-indicator-ui.test.mjs`:
   - Проверка структуры `#chat-typing-indicator` и ARIA-атрибутов (`role="status"`, `aria-live="polite"`).
   - Проверка переключения `data-state="idle|active|slowmode|error"`.
   - Проверка русского плюрализма:
     - 1 участник → `"Alice печатает…"`
     - 2 участника → `"Alice и Bob печатают…"`
     - 3 участника → `"Alice, Bob и Charlie печатают…"`
     - 4 участника → `"Несколько человек печатают…"`
   - Проверка исключения `selfUserId` из списка печатающих.
   - Проверка форматирования таймера slowmode (секунды, ММ:СС).
2. `tests/unit/markdown-mentions-ui.test.mjs`:
   - Проверка парсинга инлайн-кода (наличие класса `.md-inline-code`, отсутствие инъекций тегов).
   - Проверка структуры блока кода `.md-code-block-wrapper` с шапкой языка и кнопкой копирования.
   - Проверка цитат `.md-quote` и границы `--accent`.
   - Проверка спойлеров: дефолтное состояние `data-spoiler="hidden"`, раскрытие по клику и нажатию `Enter`/`Space` в `revealed`.
   - Проверка классов упоминаний `.mention-user`, `.mention-channel`, `.mention-role`, `.mention-broadcast` и подсветки `mention-self`.
3. Сверка соответствия токенов:
   - Прогон скрипта валидации токенов на предмет отсутствия незарегистрированных custom properties.

### 5.2 Чеклист приёмки (Definition of Done)
- [x] Контракт полностью документирован в `waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC.md`.
- [x] Привязка геометрии и цветов строго к `tokens.css` и `THEME-SPEC.md`.
- [x] Описаны все 4 состояния `data-state` индикатора набора с ARIA `role="status"` и `aria-live="polite"`.
- [x] Сформулированы правила русского плюрализма для 1, 2, 3 и >3 участников.
- [x] Зафиксирован таймер slowmode и обратный отсчет.
- [x] Описаны классы и токены Markdown: инлайн-код, блоки кода, цитаты, спойлеры с клавиатурной доступностью.
- [x] Разработана палитра пилюль упоминаний для dark и light тем с гарантией WCAG AA 4.5:1.
- [x] Продуктовый код не изменялся, cargo не запускался, git commit не выполнялся.
