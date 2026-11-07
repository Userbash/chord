# RW-40 / G3 — PINS-THREADS-PROFILE-SPEC: панели M-10 (pins), M-11 (threads) и MB-03 (profile popover)

Статус: дизайн-контракт (только спецификация; продуктовый код не менялся). Дата: 2026-10-30.
Владелец: G3 Design. Потребители: **G2 Frontend** (карточки STUB-10, STUB-16), **G1 Core** (данные закреплений/веток/профиля).
Связанные задачи:
- `STUB-10`: панель закреплённых сообщений (`M-10`) и панель веток (`M-11`) в хедере чата (`chat-header`).
- `STUB-16`: поповер профиля участника (`MB-03`) при клике на строку участника в списке участников (`#member-panel` / `#member-list`).

---

## 0. Легенда и правила оформления

| Метка | Значение | Источник |
|---|---|---|
| `[T]` | значение дизайн-токена проекта | `rust/gui-spike/ui/tokens.css` = `waves/RW-39/DESIGN/tokens.css` |
| `[D]` | факт дампа (DOM, JSON, класс, ключ i18n, атрибуты) | `ui8/*`, `index.html`, `ru-catalog.json`, `members.js` |
| `[M]` | замер на референсном скриншоте | `waves/RW-37/RE/layout.json`, `00-app.png` / 1.25 |
| `[A]` | assumption — проектное решение, адаптированное под архитектуру DL | этот документ |

### Базовые правила:
1. **Только токены**: размеры, отступы, радиусы, шрифты, цвета и тени задаются исключительно через custom properties из `tokens.css`. Никаких незадекларированных пикселей и "магических чисел".
2. **Без ассетов**: внешние картинки, веб-шрифты и спрайты запрещены. Все иконки — UTF-8 символы либо inline-SVG на системных шрифтах (`tokens.css` §5).
3. **Единый цикл состояний**: каждый компонент обязательно поддерживает 4 состояния: `empty`, `loading`, `error`, `populated`.
4. **Доступность (a11y) и фокус**: семантические роли WAI-ARIA, `aria-expanded`, `aria-busy`, `role="dialog"`, закрытие по `Escape`, возвращение фокуса на инициирующий элемент (`trigger element`).

---

## 1. Архитектурное позиционирование и перекрытие (Z-Index / Layers)

Панели M-10, M-11 и поповер MB-03 являются плавающими оверлеями/поповерами:
- Панели Pins (M-10) и Threads (M-11) позиционируются относительно правого верхнего угла рабочей области чата под хедером (`#chat-header`, высота `--layout-header-h`: 49px).
- Поповер профиля (MB-03) позиционируется как плавающее окно относительно выбранного элемента участника (`.member` в `.member-panel-list` или `#member-list`).
- z-index: `var(--z-popover)` (**100**) в соответствии с `tokens.css` §12.
- Тень: `var(--shadow-popover)` (комбинация `var(--shadow-elevation-high)` и `var(--shadow-elevation-stroke)`).
- Граница: `1px solid var(--border-subtle)`.
- Скругление: `var(--radius-md)` (12px) для карточки профиля, `var(--radius-sm)` (8px) для выпадающих панелей.

---

## 2. Спецификация панели закреплённых сообщений (M-10 / STUB-10)

### 2.1 Инициирующий триггер (Header Action)
- Элемент в хедере: `button.icon-btn[data-action="pins"]` в `.chat-actions`.
- Содержимое кнопки: пиктограмма `📌`.
- Атрибуты доступности:
  - `aria-label="Закреплённые сообщения"`
  - `aria-haspopup="dialog"`
  - `aria-expanded="false"` (переключается в `"true"` при открытии панели)
  - `aria-controls="chat-pins-panel"`

### 2.2 Разметка и геометрия контейнера
```html
<section id="chat-pins-panel" class="header-dropdown-panel pins-panel"
         role="dialog"
         aria-label="Закреплённые сообщения"
         aria-modal="false"
         data-state="empty"
         tabindex="-1">
  <header class="dropdown-header">
    <h2 class="dropdown-title">Закреплённые сообщения</h2>
    <button class="icon-btn dropdown-close" type="button" data-action="close-pins" aria-label="Закрыть">✕</button>
  </header>
  <div class="dropdown-content pins-content" role="feed" aria-busy="false">
    <!-- Сменяемый контент в зависимости от data-state -->
  </div>
</section>
```

- **Размеры**:
  - Ширина: `360px` (`[A]`, адаптированная ширина панели списка, в пределах допустимой ширины правого лейаута).
  - Максимальная высота: `calc(100vh - var(--layout-header-h) - var(--space-8))` (не превышает видимую область чата).
  - Позиционирование: `position: absolute; top: var(--layout-header-h); right: var(--space-4); z-index: var(--z-popover);`.
  - Фон: `var(--surface-2)` (hover/карточка, `#242429`).

### 2.3 Четыре состояния (State Matrix)

1. **`data-state="loading"`**:
   - `aria-busy="true"` на `.pins-content`.
   - Внутри 3 скелетона `.pin-skeleton` (высота строки сообщения `var(--row-h-continuation)` 26px и `var(--row-h-group-start)` 48px, фон `var(--surface-4)`, пульсирующая прозрачность `var(--state-disabled-opacity)`).
   - Текст для screen reader: `<span class="sr-only" role="status">Загрузка закреплённых сообщений…</span>`.

2. **`data-state="empty"`**:
   - `aria-busy="false"`.
   - Содержимое:
     ```html
     <div class="dropdown-empty-state">
       <span class="empty-icon" aria-hidden="true">📌</span>
       <p class="empty-title">В этом канале нет закреплённых сообщений</p>
       <p class="empty-desc">Сообщения могут быть закреплены через меню действий сообщения.</p>
     </div>
     ```
   - Токены: заголовок `--font-size-ui` (14px) `--text-strong`, описание `--font-size-small` (12px) `--text-muted`.

3. **`data-state="error"`**:
   - `aria-busy="false"`.
   - Содержимое:
     ```html
     <div class="dropdown-error-state" role="alert">
       <p class="error-text">Не удалось загрузить закреплённые сообщения</p>
       <button class="btn btn-secondary btn-retry" type="button" data-action="retry-pins">Повторить попытку</button>
     </div>
     ```
   - Токены: цвет текста `--danger-text`, кнопка высотой `var(--btn-height-sm)` (28px).

4. **`data-state="populated"`**:
   - Список карточек сообщений `.pin-item`:
     - Отступы: `padding: var(--space-3) var(--space-4)` (6px 8px).
     - Разделитель между карточками: `border-bottom: 1px solid var(--divider)`.
     - Шапка карточки: аватар автора `var(--avatar-sm)` (20px), имя автора `var(--font-size-ui)` / `--font-weight-semibold`, дата/время `var(--font-size-caption)` (11px) `--text-muted`.
     - Тело сообщения: `var(--font-size-ui)`, перенос строк `var(--line-height-ui)`.
     - Кнопка перехода к сообщению (Jump to message):
       `<button class="btn-jump" type="button" data-action="jump-to-message" data-message-id="...">Перейти</button>`.
       Высота: `var(--btn-height-sm)` (28px), цвет `--text-link`, hover: underline.

---

## 3. Спецификация панели веток (M-11 / STUB-10)

### 3.1 Инициирующий триггер (Header Action)
- Элемент в хедере: `button.icon-btn[data-action="threads"]` в `.chat-actions`.
- Содержимое: пиктограмма `🧵`.
- Атрибуты:
  - `aria-label="Ветки"`
  - `aria-haspopup="dialog"`
  - `aria-expanded="false"` (переключается в `"true"` при открытии)
  - `aria-controls="chat-threads-panel"`

### 3.2 Разметка и геометрия контейнера
```html
<section id="chat-threads-panel" class="header-dropdown-panel threads-panel"
         role="dialog"
         aria-label="Ветки канала"
         aria-modal="false"
         data-state="empty"
         tabindex="-1">
  <header class="dropdown-header">
    <div class="threads-header-tabs" role="tablist">
      <button role="tab" id="tab-active-threads" aria-selected="true" aria-controls="threads-list" tabindex="0">Активные</button>
      <button role="tab" id="tab-archived-threads" aria-selected="false" aria-controls="threads-list" tabindex="-1">Архив</button>
    </div>
    <button class="icon-btn dropdown-close" type="button" data-action="close-threads" aria-label="Закрыть">✕</button>
  </header>
  <div id="threads-list" class="dropdown-content threads-content" role="region" aria-busy="false">
    <!-- Сменяемый контент в зависимости от data-state -->
  </div>
</section>
```

- **Размеры**:
  - Ширина: `380px` (`[A]`, позволяет комфортно разместить название ветки, количество ответов и временную метку).
  - Максимальная высота: `calc(100vh - var(--layout-header-h) - var(--space-8))`.
  - Позиционирование: `position: absolute; top: var(--layout-header-h); right: var(--space-4); z-index: var(--z-popover);`.
  - Фон: `var(--surface-2)`.

### 3.3 Четыре состояния (State Matrix)

1. **`data-state="loading"`**:
   - `aria-busy="true"`.
   - Внутри 4 плашки-скелетона `.thread-skeleton` высотой 36px (`var(--space-2)` зазор, фон `var(--surface-4)`).

2. **`data-state="empty"`**:
   - `aria-busy="false"`.
   - Содержимое:
     ```html
     <div class="dropdown-empty-state">
       <span class="empty-icon" aria-hidden="true">🧵</span>
       <p class="empty-title">В этом канале нет активных веток</p>
       <p class="empty-desc">Создайте ветку из любого сообщения для отдельного обсуждения.</p>
     </div>
     ```
   - Заголовок: `--font-size-ui`, текст `--text-muted`.

3. **`data-state="error"`**:
   - `aria-busy="false"`.
   - Содержимое:
     ```html
     <div class="dropdown-error-state" role="alert">
       <p class="error-text">Не удалось загрузить список веток</p>
       <button class="btn btn-secondary btn-retry" type="button" data-action="retry-threads">Повторить попытку</button>
     </div>
     ```

4. **`data-state="populated"`**:
   - Список элементов `.thread-item`:
     - Строка ветки: высота минимум 44px, padding `var(--space-2) var(--space-4)`.
     - Заголовок ветки: `--font-size-ui`, `--font-weight-medium`, цвет `--text-strong`.
     - Метаданные (строка под заголовком):
       `<span class="thread-replies">💬 {count} ответов</span> · <span class="thread-last-msg">посл. {time}</span>`.
       Размер шрифта: `var(--font-size-small)` (12px), цвет `var(--text-muted)`.
     - Фокусная строка: при клике или Enter/Space на `.thread-item` происходит переход в режим ветки (вызов `router.openThread(threadId)`).

---

## 4. Спецификация поповера профиля участника (MB-03 / STUB-16)

### 4.1 Инициирующий триггер (Trigger Element)
- Строка участника `.member` в правой панели (`#member-panel`) или в полном экране участников (`#member-list`).
- Атрибуты триггера:
  - `role="button"` (либо кнопка внутри строки)
  - `aria-haspopup="dialog"`
  - `aria-expanded="false"` (переключается в `"true"` при клике)
  - `aria-controls="user-profile-popover"`

### 4.2 Геометрия и разметка карточки профиля
```html
<aside id="user-profile-popover" class="user-profile-popover"
       role="dialog"
       aria-label="Профиль пользователя"
       aria-modal="false"
       data-state="populated"
       tabindex="-1">
  <!-- 1. Верхний баннер / акцентный фон -->
  <div class="profile-banner" style="background-color: hsl(var(--avatar-hue, 212) 45% 25%);"></div>

  <!-- 2. Секция аватара со статусом -->
  <div class="profile-avatar-wrap">
    <div class="avatar avatar--xl profile-avatar" style="--avatar-hue: 212;">
      <span class="avatar-monogram">SB</span>
      <span class="status-dot status-dot--lg" data-status="online" aria-label="В сети"></span>
    </div>
  </div>

  <!-- 3. Основные данные пользователя -->
  <div class="profile-body">
    <div class="profile-header-info">
      <h3 class="profile-display-name">ShadowByte</h3>
      <div class="profile-tag">shadowbyte#0</div>
      <div class="profile-custom-status" data-has-status="true">🎮 Играет в Retro Lab</div>
    </div>

    <div class="profile-divider"></div>

    <!-- 4. Роли на сервере -->
    <div class="profile-section">
      <h4 class="profile-section-title">РОЛИ</h4>
      <div class="profile-badges" role="list">
        <span class="badge badge--role" role="listitem">Разработчик</span>
        <span class="badge badge--role" role="listitem">Тестировщик</span>
      </div>
    </div>

    <div class="profile-divider"></div>

    <!-- 5. Действия пользователя -->
    <div class="profile-actions">
      <button class="btn btn-secondary btn-sm" type="button" data-action="dm-user">Написать сообщение</button>
      <button class="btn btn-ghost btn-sm" type="button" data-action="mention-user">Упомянуть</button>
    </div>
  </div>
</aside>
```

- **Размеры и позиционирование**:
  - Ширина: `300px` (`[M]`, близко к замеру Discord `300–320px`, идеально согласуется с панелью 240px + смещение влево).
  - Баннер: высота `60px` (`var(--space-12)` + `var(--space-2)`).
  - Аватар: `var(--avatar-lg)` (40px) или специальный `avatar--xl` (56px) с отрицательным margin-top `-28px` для наплыва на баннер.
  - Толщина обводки аватара: `4px solid var(--surface-1)` (отсекает баннер от аватара).
  - Радиус поповера: `var(--radius-md)` (12px).
  - Фон тела: `var(--surface-1)` (`#202024`).
  - Тень: `var(--shadow-popover)`.
  - Позиционирование: `position: fixed; z-index: var(--z-popover);`. Вычисляется в JS динамически по координатам строки участника с защитой от вылета за правый и нижний края viewport (edge flipping).

### 4.3 Четыре состояния (State Matrix)

1. **`data-state="loading"`**:
   - `aria-busy="true"`.
   - Баннер `var(--surface-3)`, аватар-заглушка с мерцанием, скелетоны для имени (две полосы 14px и 12px в `.profile-body`), скрытый блок ролей.

2. **`data-state="empty"`** (пользователь не найден / удалён):
   - `aria-busy="false"`.
   - Текст: "Пользователь не найден или покинул сервер".
   - Иконка статуса offline.

3. **`data-state="error"`**:
   - `aria-busy="false"`.
   - Содержимое: "Не удалось загрузить данные профиля", кнопка "Повторить".

4. **`data-state="populated"`**:
   - Полноценная карточка с актуальным статусом, ролями и доступными кнопками действий.

---

## 5. Клавиатурная навигация, фокус и правила закрытия (Keyboard Navigation & Focus Management)

Для всех трёх компонентов (M-10 Pins, M-11 Threads, MB-03 Profile Popover) действуют строгие инварианты WAI-ARIA и фокуса:

### 5.1 Открытие компонента
1. При срабатывании триггера (клик мышью, нажатие `Enter` или `Space` на триггере):
   - Триггеру выставляется атрибут `aria-expanded="true"`.
   - Панель / поповер удаляет атрибут `hidden` (или добавляется в DOM) и получает класс видимости.
   - Фокус **перемещается внутрь панели**:
     - Для M-10 Pins: на кнопку закрытия `dropdown-close` или на первый интерактивный элемент списка (`btn-jump`).
     - Для M-11 Threads: на активный таб `[role="tab"][aria-selected="true"]`.
     - Для MB-03 Profile: на сам контейнер поповера (`tabindex="-1"`) либо на первую кнопку действий (`dm-user`).

### 5.2 Закрытие компонента (Dismissal & Escape)
1. Компонент закрывается в следующих случаях:
   - Нажатие клавиши **`Escape`** в любой точке внутри открытой панели/поповера.
   - Клик по кнопке закрытия (`.dropdown-close` с `data-action="close-*"`).
   - Клик вне области компонента (Outside click / pointerdown outside), если клик не пришёлся на сам инициирующий триггер.
   - Переключение активного канала или экрана (`body[data-active-screen]`).
2. **Возврат фокуса (Focus Return)**:
   - **Обязательный инвариант**: при любом закрытии по `Escape` или клику вне панели фокус **немедленно возвращается** на инициирующий триггерный элемент (кнопка в хедере или строка участника).
   - Триггеру возвращается `aria-expanded="false"`.
   - Пользователь не теряет контекст клавиатурной навигации.

### 5.3 Навигация стрелками внутри списков
- Вкладки веток (M-11): roving tabindex (стрелки `ArrowLeft`/`ArrowRight` перемещают фокус между вкладками "Активные" и "Архив").
- Списки сообщений (M-10) и веток (M-11): стрелки `ArrowUp`/`ArrowDown` циклически перемещают фокус между интерактивными карточками/кнопками перехода.

---

## 6. Привязка к токенам `tokens.css` (Design Tokens Mapping)

| Компонент / Элемент | CSS свойство | Токен из `tokens.css` | Значение токена |
|---|---|---|---|
| Поверхность панелей / поповеров | `background-color` | `--surface-2` / `--surface-1` | `#242429` / `#202024` |
| Граница панелей | `border` | `1px solid var(--border-subtle)` | `1px solid rgb(151 151 159 / 12%)` |
| Тень всплывающих окон | `box-shadow` | `--shadow-popover` | `var(--shadow-elevation-high), var(--shadow-elevation-stroke)` |
| Слой наложения | `z-index` | `--z-popover` | `100` |
| Заголовки блоков | `font-size`, `color` | `--font-size-ui`, `--text-strong` | `14px`, `#fbfbfb` |
| Описания и пустое состояние | `font-size`, `color` | `--font-size-small`, `--text-muted` | `12px`, `#96979e` |
| Ошибки загрузки | `color` | `--danger-text` | `#ff9a9e` |
| Кнопки действий (высота) | `height`, `min-height` | `--btn-height-sm` | `28px` |
| Скругление карточки профиля | `border-radius` | `--radius-md` | `12px` |
| Скругление выпадающих панелей | `border-radius` | `--radius-sm` | `8px` |
| Фокусный контур | `outline` | `var(--focus-ring-width) solid var(--focus-ring)` | `2px solid #a5b0ff` |
| Фокусное смещение | `outline-offset` | `var(--focus-ring-offset)` | `2px` |

---

## 7. Файловая матрица реализации (для G2)

1. `rust/gui-spike/ui/components/header.css` — добавление селекторов `.header-dropdown-panel`, `.pins-panel`, `.threads-panel`.
2. `rust/gui-spike/ui/components/members.css` — добавление стилей карточки `.user-profile-popover`, `.profile-banner`, `.profile-avatar`.
3. `rust/gui-spike/ui/features/actions.js` — реализация обработчиков `pins`, `threads`, `profile`, закрытия по `Escape` и возврата фокуса.
4. Продуктовый Rust-код и Cargo-манифесты не затрагиваются.
