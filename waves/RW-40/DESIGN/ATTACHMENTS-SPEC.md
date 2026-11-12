# RW-40 / G3-ATTACH-AND-GIFT-SPEC — композер: вложения и подарок 🎁

Статус: дизайн-контракт (документ; продуктовый код не менялся). Дата: 2026-10-30.
Владелец: G3 Design. Исполнитель по UI: G2 Frontend.
Вход:
`waves/RW-40/reference-oracle-REA.md` §1 (STUB-13 вложения), §5 (STUB-12 gift), §8 (остаточные gap);
`waves/RW-40/THEME-SPEC.md` §2 (dark→light), §2.6 (контраст), §4.2 (порядок блоков);
`waves/RW-40/ATTACH-RE.md` §2; `rust/gui-spike/ui/tokens.css`;
`rust/gui-spike/ui/index.html` (композер, строки 121–127); `ui/components/composer.css`;
`ui/components/server-menu.css` (`.dialog-backdrop`, `.dialog`, `.btn`);
`ui/features/actions.js` (реестр `data-action`, focus-trap, disabled-affordance);
`tests/unit/gui-spike-dom.test.mjs`, `tests/unit/gui-spike-features.test.mjs` (замороженные проверки).

Ассеты RE не копируются: переносятся только числа, имена полей и тексты контракта.

## 0. Легенда, правила и границы

| Метка | Значение | Источник |
|-|-|-|
| `[D]` | факт из дампа / live-бандла | `reference-oracle-REA.md` §1, §5 |
| `[T]` | существующий токен проекта | `ui/tokens.css` (значение в скобках — для ревью) |
| `[L]` | значение light-темы | `THEME-SPEC.md` §2 |
| `[A]` | наше решение или пробел | этот документ; подтверждение до реализации |

Правила:

1. Любой цвет и любое значение из шкалы (отступы, радиусы, размеры текста, контролов) — `var(--token)`.
   Допустимые литералы: hairline `1px` для границ, `0`, `100%` и единицы окна в `calc()`, а также
   theme-agnostic значения, уже разрешённые THEME-SPEC §4.3 (scrim `rgb(0 0 0 / 72%)` внутри
   `.dialog-backdrop`, `hsl(var(--avatar-*))`). В новом коде scrim не дублируется: используется
   существующий класс `.dialog-backdrop`.
2. Пиксельных замеров этих двух компонентов в дампе **нет**. `[M]` в документе не используется:
   геометрия чипа и модалки — наше решение `[A]`, контракт сети и лимитов — `[D]`.
3. Ошибки не глотаются: у каждого отказа есть состояние, русский текст и выход
   (повтор/удаление/закрытие). Молчаливого `catch {}` в реализации быть не может.
4. Продуктовый код, чужие модули, `logs/`, `functions/`, `evidence/` не трогаются.
   Документ адресован G2; файловая матрица для G2 — §6.2.

## 1. Кнопка «+» и вход файлов

### 1.1 DOM

Кнопка уже есть в `index.html:122`; меняется только контракт поведения (уходит заглушка
`aria-disabled="true"`).

```html
<div id="composer" class="composer" role="group" aria-label="Поле ввода сообщения"
     data-attach-state="none">
  <!-- 1. Кнопка выбора файлов. Остаётся в DOM всегда (в т.ч. disabled). -->
  <button class="icon-btn composer-attach" type="button"
          data-action="attach"
          aria-label="Прикрепить файл"
          title="Прикрепить файл (до 10)">
    <span aria-hidden="true">＋</span>
  </button>
  <input id="composer-file-input" type="file" multiple hidden
         aria-hidden="true" tabindex="-1">

  <!-- 2. Очередь превью: absolute, над композером, в поток не входит. -->
  <div id="composer-attachments" class="composer-attachments" role="list"
       aria-label="Вложения к сообщению" hidden></div>

  <!-- 3. Счётчик файлов: виден, когда очередь непустая. -->
  <span id="composer-attach-count" class="composer-attach-count"
        data-visible="false">0 / 10</span>

  <textarea id="composer-input" class="composer-input" rows="1"
            placeholder="Написать сообщение…" aria-label="Сообщение"
            aria-describedby="composer-hint composer-attach-hint"></textarea>

  <button class="icon-btn" type="button" data-action="emoji" aria-label="Эмодзи"
          aria-haspopup="dialog" aria-expanded="false">😀</button>

  <!-- 4. Подарок: см. §4. -->
  <button class="icon-btn composer-gift" type="button"
          data-action="composer-gift"
          aria-label="Подарок" aria-haspopup="dialog"
          aria-expanded="false" aria-controls="gift-modal"
          title="Отправить подарок">🎁</button>
</div>

<!-- 5. Один live-region на всю очередь (не по региону на чип). -->
<p id="composer-attach-status" class="visually-hidden"
   role="status" aria-live="polite" aria-atomic="true"></p>
<p id="composer-attach-hint" class="visually-hidden"></p>
```

| Элемент | Контракт | Метка |
|-|-|-|
| `#composer` | контейнер 56px не меняет высоту; очередь — absolute-слой | `[T]` `--layout-composer-h` (56); `[A]` решение |
| `[data-action="attach"]` | открывает нативный диалог выбора файлов (`#composer-file-input.click()`), не сеть | `[A]` |
| `#composer-file-input` | `multiple`, `hidden`, вне tab-порядка; значение сбрасывается после выбора | `[A]` |
| `#composer-attachments` | `role="list"`, `hidden` при пустой очереди; чипы — `role="listitem"` | `[A]` |
| `#composer-attach-count` | текст `«3 / 10»`; при 10 файлах `data-full="true"` | `[D]` лимит 10; `[A]` формат |
| `#composer-attach-status` | единственный `aria-live="polite"`, `aria-atomic="true"`; текст — §3.2 | `[A]` |

Почему очередь absolute, а не строка в потоке: `.composer` — фиксированные 56px, а над ним уже
висят absolute-элементы `.composer-reply` и `.typing-indicator` (`composer.css` §P3-02, P-03-UI).
Строка в потоке сдвинула бы композер и сломала геометрию списка. Поэтому: очередь — absolute над
композером, а два существующих чипа поднимаются на её высоту через runtime-переменную
`--attach-queue-h` (§2.4).

### 1.2 Три входа

| Вход | Контракт | Метка |
|-|-|-|
| Клик по `attach` | открывает picker; отмена picker'а — без изменений и без текста | `[A]` |
| Drag-and-drop на `.composer` | `data-drop="active"` на `#composer`, фон `--accent-soft`; drop добавляет файлы; dragleave/ESC снимает атрибут | `[A]` (совпадает с `DESIGN-MSG.md` §2.1) |
| Paste из clipboard | файлы из `clipboardData.files` попадают в очередь; текст — обычная вставка | `[A]` |

Правила приёма (проверяются до сети):

1. Пустой файл (`size === 0`) → чип в состоянии `error-empty` сразу, без `POST`:
   клиент делает ту же проверку (`0===this.currentSize → ENTITY_EMPTY`) `[D]` §1.6.
2. Файл больше лимита канала → `error-413` сразу, без сети (`currentSize > getMaxFileSize → ENTITY_TOO_LARGE`) `[D]`.
   Лимит канала — tier-based: NONE/T1 — 10 МиБ, T2 — 50 МиБ, T3 — 100 МиБ `[D]` §1.5.
   Пока tier неизвестен, берётся **10 МиБ** (значение сервера для бесплатного guild и константа
   бэкенда B2a `MAX_UPLOAD_FILE_BYTES`) — UI не имеет права пропустить больше, чем примет сервер.
3. Уже 10 файлов → новые не принимаются, статус `limitCount`, кнопка `attach` получает
   `aria-disabled="true"` и `title` с причиной (визуально `--state-disabled-opacity`).
4. Суммарный размер очереди больше лимита сообщения → отказ `limitTotal`, без сети.
5. Порядок чипов = порядок выбора. Перетаскивания внутри очереди в MVP нет (`[A]`).

### 1.3 Клавиатура и фокус

| Действие | Поведение |
|-|-|
| Enter/Space на `attach` | открыть picker (нативная кнопка, дополнительный код не нужен) |
| Tab | `attach` → `#composer-input` → `emoji` → `composer-gift` → чипы очереди |
| Tab внутри чипа | по DOM: «Повторить» (если виден) → «Убрать» |
| Delete на кнопке внутри чипа | удаляет чип, фокус уходит на следующий чип или на `attach` |
| Escape при `data-drop="active"` | снимает подсветку, файлы не добавляются |
| Возврат фокуса | после удаления последнего чипа — на `attach`; после ошибки — на «Повторить» |

## 2. Очередь `#composer-attachments`

### 2.1 Полная разметка очереди и чипа

```html
<div id="composer-attachments" class="composer-attachments" role="list"
     aria-label="Вложения к сообщению">
  <div class="attachment-chip" role="listitem"
       data-attachment-id="0f0c2f5e-6c1b-4c39-9e0e-2b8f6d7a1c11"
       data-state="uploading"
       aria-busy="true">
    <!-- превью: изображение или иконка типа -->
    <img class="attachment-thumb" src="blob:…" alt="" aria-hidden="true">
    <div class="attachment-meta">
      <span class="attachment-name" title="договор-2026-финал.pdf">договор-2026-финал.pdf</span>
      <span class="attachment-size">2,4 МиБ</span>
      <span class="attachment-status">Загрузка… 45%</span>
      <span class="attachment-error" role="alert" hidden></span>
    </div>
    <div class="attachment-progress" role="progressbar"
         aria-label="Загрузка «договор-2026-финал.pdf»"
         aria-valuemin="0" aria-valuemax="100" aria-valuenow="45"
         aria-valuetext="45%">
      <div class="attachment-progress-fill" style="width: 45%"></div>
    </div>
    <div class="attachment-actions">
      <button class="attachment-action" type="button"
              data-action="attachment-retry" hidden>Повторить</button>
      <button class="attachment-action attachment-remove" type="button"
              data-action="attachment-remove"
              aria-label="Убрать файл «договор-2026-финал.pdf»"
              title="Убрать файл"><span aria-hidden="true">✕</span></button>
    </div>
  </div>
</div>
```

`data-attachment-id` — локальный UUID из тела `POST …/attachments` (`files[].id`,
`s().uniqueId()` `[D]` §1.3), не серверный id и не `upload_filename`. До отправки запроса
слота id генерируется локально и остаётся неизменным на весь жизненный цикл чипа.

### 2.2 Анатомия чипа

| Часть | Контракт | Метка |
|-|-|-|
| Ширина | `--attach-chip-w` (248px), `max-width: 100%` | `[A]` |
| Высота | `min-height` = thumb 40 + 2×8 = 56 | `[T]` `--attach-chip-thumb` (40), `--space-4` (8); `[A]` |
| Превью | изображение — `blob:`-URL, `object-fit: cover`, `--radius-xs`; прочие файлы — иконка `--icon-size-lg` (24) по MIME-семейству | `[A]`; object URL отзывается при удалении/готовности |
| Имя | одна строка, `text-overflow: ellipsis`, полный текст в `title`; не обрезается в середине | `[A]` |
| Размер | `--font-size-small`, `--text-subtle`; формат — §2.3 | `[A]` |
| Статус | `--font-size-small`, `--text-muted`; при `ready` текст «Готово» `--positive-text` | `[T]` `--positive-text` |
| Ошибка | `.attachment-error`, `role="alert"`, `--danger-text`; заменяет статус, не добавляется вторым рядом | `[T]` `--danger-text` (4.86–5.95 светлая) |
| Прогресс | `role="progressbar"`, `aria-valuenow` 0…100, `aria-valuetext` = `«{n}%»`; высота `--progress-h` (4), заливка `--accent`, трек `var(--state-active)` | `[T]`; `[A]` геометрия |
| Удаление | `[data-action="attachment-remove"]`, `aria-label` с именем файла; доступно во всех состояниях | `[A]` |
| Повтор | `[data-action="attachment-retry"]`, виден только при `error-network` / `error-other` / `error-429` (после отсчёта) | `[A]` |

Чип остаётся в DOM во всех состояниях, включая ошибки. Удаление — единственный способ убрать
файл из очереди (клик по «✕» во время `uploading` = отмена: `AbortController.abort()`).

### 2.3 Формат размера и процента

```
formatBytes(n):
  n < 1024                     → "{n} Б"
  n < 1024^2                   → значение = n / 1024,        единица "КиБ"
  n < 1024^3                   → значение = n / 1024^2,      единица "МиБ"
  иначе                        → значение = n / 1024^3,      единица "ГиБ"
  значение: 1 знак после запятой, ",0" отбрасывается, разделитель — запятая,
  между числом и единицей — неразрывный пробел (U+00A0)
```

Примеры: `0 → «0 Б»`, `999 → «999 Б»`, `1024 → «1 КиБ»`, `1536 → «1,5 КиБ»`,
`20971520 → «20 МиБ»`, `500 · 1024² → «500 МиБ»`.

```
percent:
  p = floor(отправлено_байт / всего_байт × 100), clamp 0…100
  100 выставляется только после PUT 200/201 И наличия upload_filename
  значение монотонно не убывает (докачка продолжает с места обрыва)
  при докачке: отправлено_байт = resume_start + bytes_sent_in_this_run
```

### 2.4 CSS очереди

Новые правила кладутся в `ui/components/composer.css` (секция «вложения»), порядок — после
существующих блоков; `!important` не используется. Единственный inline-style — `width` у
`.attachment-progress-fill` (динамический прогресс); цвет и геометрия остаются в CSS.

```css
/* Очередь: absolute над композером, высота в поток не входит. */
.composer-attachments {
  position: absolute;
  right: 0;
  bottom: calc(100% + var(--space-1));
  left: 0;
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  max-height: calc(var(--attach-chip-thumb) * 3 + var(--space-2) * 2);
  overflow-y: auto;
  scrollbar-width: thin;
}

.composer-attachments[hidden] {
  display: none;
}

/* Чип: карточка на --surface-2, бордер нужен светлой теме (surface-2 ≈ фон). */
.attachment-chip {
  position: relative;
  display: grid;
  grid-template-columns: var(--attach-chip-thumb) minmax(0, 1fr) auto;
  grid-template-rows: auto auto;
  gap: var(--space-1) var(--space-3);
  width: var(--attach-chip-w);
  max-width: 100%;
  min-height: calc(var(--attach-chip-thumb) + var(--space-4) * 2);
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  color: var(--text);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.attachment-thumb {
  grid-row: 1;
  width: var(--attach-chip-thumb);
  height: var(--attach-chip-thumb);
  border-radius: var(--radius-xs);
  object-fit: cover;
  background: var(--surface-3);
}

.attachment-meta {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.attachment-name {
  overflow: hidden;
  color: var(--text);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attachment-size {
  overflow: hidden;
  color: var(--text-subtle);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attachment-status {
  overflow: hidden;
  color: var(--text-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attachment-chip[data-state="ready"] .attachment-status {
  color: var(--positive-text);
}

.attachment-error {
  max-height: calc(var(--line-height-small) * 2);
  overflow: hidden;
  color: var(--danger-text);
}

.attachment-error[hidden],
.attachment-status[hidden] {
  display: none;
}

/* Прогресс: вторая строка под метаданными, на всю ширину, кроме превью. */
.attachment-progress {
  grid-row: 2;
  grid-column: 2 / -1;
  height: var(--progress-h);
  overflow: hidden;
  border-radius: var(--radius-round);
  background: var(--state-active);
}

.attachment-progress-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width var(--motion-fast) linear;
}

.attachment-chip[data-state="error-network"] .attachment-progress-fill {
  background: var(--danger);
}

.attachment-actions {
  display: flex;
  gap: var(--space-2);
  grid-row: 1;
  grid-column: 3;
  align-items: flex-start;
}

.attachment-action {
  min-height: var(--btn-height-sm);
  padding: 0 var(--space-3);
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-accent);
  font: inherit;
  cursor: pointer;
}

.attachment-action:hover {
  background: var(--state-hover);
}

.attachment-action:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.attachment-remove {
  width: var(--icon-size);
  height: var(--icon-size);
  padding: 0;
  color: var(--text-muted);
}

.attachment-action[hidden] {
  display: none;
}

/* Видимость статуса и прогресса по состояниям (см. §3.1). */
.attachment-chip[data-state="idle"] .attachment-status,
.attachment-chip[data-state^="error"] .attachment-status {
  display: none;
}

.attachment-chip[data-state="idle"] .attachment-progress,
.attachment-chip[data-state="ready"] .attachment-progress,
.attachment-chip[data-state="error-413"] .attachment-progress,
.attachment-chip[data-state="error-empty"] .attachment-progress,
.attachment-chip[data-state="error-429"] .attachment-progress,
.attachment-chip[data-state="error-other"] .attachment-progress {
  display: none;
}

/* В error-network полоса остаётся на последнем значении — она показывает,
 * где оборвалась докачка. */

/* Счётчик файлов в композере. */
.composer-attach-count {
  flex: 0 0 auto;
  order: 4;
  color: var(--text-muted);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.composer-attach-count[data-visible="false"] {
  display: none;
}

.composer-attach-count[data-full="true"] {
  color: var(--danger-text);
}

/* Недоступные кнопки композера: остаются в DOM, но честно выглядят disabled. */
.composer-attach[aria-disabled="true"],
.composer-gift[aria-disabled="true"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

/* Dropzone: подсветка композера при drag-over. */
.composer[data-drop="active"] {
  outline: var(--focus-ring-width) dashed var(--accent);
  outline-offset: calc(-1 * var(--focus-ring-offset));
  background: var(--accent-soft);
}

/* Существующие чипы поднимаются на высоту очереди. */
.composer-reply,
.typing-indicator {
  bottom: calc(100% + var(--attach-queue-h, 0px) + var(--space-2));
}

.composer[data-reply="true"] .typing-indicator {
  bottom: calc(100% + var(--attach-queue-h, 0px) + var(--space-2)
              + var(--line-height-small) + var(--space-2));
}

/* Последнее сообщение не прячется под очередью. Если виртуализация списка
 * игнорирует padding-bottom (проверить на W39d), правило снимается: очередь
 * временная и исчезает после отправки. */
#screen-chat .chat-list {
  padding-bottom: calc(var(--attach-queue-h, 0px) + var(--space-2));
}

@media (prefers-reduced-motion: reduce) {
  .attachment-progress-fill {
    transition: none;
  }
}
```

`--attach-queue-h` — **runtime-переменная, не дизайн-токен**: JS пишет её на `#screen-chat`
(`style.setProperty("--attach-queue-h", h + "px")`) по `ResizeObserver` на
`#composer-attachments`, через `requestAnimationFrame`. При пустой очереди свойство удаляется,
fallback `0px` в CSS. В jsdom замеров нет → переменная не ставится, значение остаётся `0px`;
поэтому тесты не проверяют пиксели, только наличие/отсутствие атрибута и `hidden`.

## 3. Состояния вложений

### 3.1 Машина состояний чипа (`data-state`)

| Состояние | Что происходит | `data-state` | `aria-busy` | Прогресс | Действия | Метка |
|-|-|-|-|-|-|-|
| idle | файл принят, в очередь добавлен, сеть ещё не тронута | `idle` | `false` | скрыт | Убрать | `[A]` |
| preparing | `POST /channels/{id}/attachments` за слотом | `uploading` | `true` | 0% («Подготовка…») | Убрать (отмена) | `[D]` §1.2; `[A]` подпись |
| uploading | `PUT {upload_url}` идёт | `uploading` | `true` | 0…100 | Убрать (abort) | `[D]` §1.4 |
| ready | PUT 200/201, `upload_filename` получен | `ready` | `false` | скрыт | Убрать | `[D]` §1.7 |
| error-empty | файл 0 байт (`40006 ENTITY_EMPTY`) | `error-empty` | `false` | скрыт | Убрать | `[D]` §1.6 |
| error-413 | файл больше лимита (`40005 ENTITY_TOO_LARGE`, локальная проверка или ответ) | `error-413` | `false` | скрыт | Убрать | `[D]` §1.5–1.6 |
| error-429 | сервер ответил 429 + `retry-after` | `error-429` | `false` | скрыт | Убрать; Повторить (после отсчёта) | `[D]` §1.6 |
| error-network | обрыв, таймаут, 5xx | `error-network` | `false` | замирает на последнем значении | Убрать; Повторить | `[A]` |
| error-other | иной код (`50035 INVALID_FORM_BODY`, 403, неизвестный) | `error-other` | `false` | скрыт | Убрать; Повторить | `[D]` §1.6 |

Пять состояний из задачи (`idle` / `uploading` / `ready` / `error-413` / `error-429`) — каноничные;
`error-empty`, `error-network`, `error-other` добавлены потому, что контракт дампа их прямо
предусматривает, и молчаливая потеря файла недопустима.

Правило имени `error-413`: HTTP-кода 413 у upload в клиенте нет — сервер отдаёт
`body.code = 40005 ENTITY_TOO_LARGE`, а клиент режет размер локально `[D]` §1.6. Поэтому состояние
`error-413` срабатывает и на локальную предпроверку, и на HTTP 413, и на `body.code === 40005`.

### 3.2 Тексты (русский, дословно)

```js
export const ATTACH_TEXT = {
  button: "Прикрепить файл",
  buttonTitle: "Прикрепить файл (до 10)",
  countLabel: "Вложения к сообщению",
  statusRegion: "Состояние загрузки вложений",

  preparing: "Подготовка…",
  uploading: (p) => `Загрузка… ${p}%`,
  ready: "Готово",

  limitCount: (max) => `Можно прикрепить не больше ${max} файлов.`,
  limitTotal: (max) => `Суммарный размер вложений больше ${max}.`,
  emptyFile: "Файл пустой — прикрепите другой.",
  tooLarge: (max) => `Файл больше ${max}. Уменьшите размер и попробуйте снова.`,
  rateLimited: (sec) => `Слишком много загрузок. Повторить через ${sec} с.`,
  network: "Не удалось загрузить файл. Проверьте соединение.",
  rejected: "Сервер отклонил файл.",

  retry: "Повторить",
  remove: (name) => `Убрать файл «${name}»`,

  sendBlocked: "Дождитесь окончания загрузки файлов.",
  sendBlockedError: "Уберите файлы с ошибкой или повторите загрузку.",
  dropActive: "Отпустите, чтобы прикрепить",

  started: (name) => `Файл «${name}» добавлен в очередь.`,
  uploaded: (name) => `Файл «${name}» загружен.`,
  failed: (name, reason) => `Файл «${name}»: ${reason}`,
  cancelled: (name) => `Загрузка файла «${name}» отменена.`,
};
```

| Где | Что показывается | Метка |
|-|-|-|
| `.attachment-status` в `uploading` | `Загрузка… 45%` | `[A]` |
| `.attachment-status` в `preparing` | `Подготовка…` | `[A]` |
| `.attachment-status` в `ready` | `Готово` | `[A]` |
| `.attachment-error` в `error-413` | `Файл больше 10 МиБ. Уменьшите размер и попробуйте снова.` | `[D]` лимит; `[A]` текст |
| `.attachment-error` в `error-429` | `Слишком много загрузок. Повторить через 12 с.` (число убывает) | `[D]` `retry-after`; `[A]` текст |
| `.attachment-error` в `error-empty` | `Файл пустой — прикрепите другой.` | `[D]` код 40006; `[A]` текст |
| `.attachment-error` в `error-network` | `Не удалось загрузить файл. Проверьте соединение.` | `[A]` |
| `.attachment-error` в `error-other` | `Сервер отклонил файл.` | `[A]` |
| `#composer-attach-status` | старт/финиш/ошибка по одному разу на событие, без прогресса | `[A]` |
| `#composer-attach-hint` | `Дождитесь окончания загрузки файлов.` пока есть `uploading` | `[A]` |
| `title` кнопки при 10 файлах | `Прикрепить файл (до 10)` + причина отказа в live-region | `[A]` |

Лимит в тексте `tooLarge` берётся из вычисленного значения, а не хардкодится:
`getMaxFileSize(channel)` — tier NONE/T1 `10 МиБ`, T2 `50 МиБ`, T3 `100 МиБ` `[D]` §1.5.
Значение по умолчанию — **10 МиБ**: это же число зафиксировано в бэкенде B2a
(`MAX_UPLOAD_FILE_BYTES`, `PM/b2a-attach/REPORT.md`), и UI не должен его превышать, иначе
сервер вернёт `40005` на файле, который локально прошёл проверку.
Отдельный факт дампа `Math.max(20 МиБ, tierLimit)` `[D]` §1.5 относится к другой ветке лимита
(буст-аккаунты); как дефолт он не используется — иначе UI обещает больше, чем примет сервер.
Если данных о tier нет — показывается «Файл больше допустимого размера.
Уменьшите размер и попробуйте снова.» `[A]`.

Прогресс **не** анонсируется в live-region: проценты в `aria-valuenow` читает AT при фокусе на
чипе. Иначе скринридер захлёбывается. Анонсы — только `started`, `uploaded`, `failed`, `cancelled`.

### 3.3 413 и 429: поведение

**413 / `40005`**

1. Локальная предпроверка до `POST` (размер > лимита) — состояние `error-413`, сети нет.
2. Ответ `POST` с `body.code === 40005` или HTTP 413 — то же состояние, текст с лимитом.
3. «Повторить» не показывается: файл не уменьшится от повтора. Единственный выход — «Убрать».
4. Если сервер отверг файл, который локально прошёл (лимит сервера ниже клиентского), текст
   всё равно называет лимит, но в лог пишется `limit_source: "server"` для диагностики `[A]`.
5. `error-empty` (`40006`) ведёт себя так же: только «Убрать».

**429**

1. `retry-after` читается из заголовка (секунды), как в клиенте: `parseInt(headers["retry-after"])`
   `[D]` §1.6. Нет заголовка или `NaN` → 5 с `[A]`.
2. Пока идёт отсчёт: «Повторить» `disabled`, `aria-disabled="true"`, текст
   `Слишком много загрузок. Повторить через N с.`; отсчёт обновляется раз в секунду.
3. По окончании отсчёта кнопка включается; повтор идёт с того же шага (слот или PUT), не с нуля.
4. Отсчёт — `setInterval`, очищается при удалении чипа и при уходе со экрана; утечек нет.
5. Другие чипы в очереди не блокируются глобально: 429 относится к конкретному запросу `[A]`.

### 3.4 Агрегат композера и отправка

`#composer[data-attach-state]`:

| Значение | Условие | Влияние |
|-|-|-|
| `none` | очередь пуста | `#composer-attachments[hidden]`, счётчик скрыт |
| `queued` | есть чипы в `idle` (ещё не начали) | отправка заблокирована |
| `uploading` | есть чип в фазе `preparing` или `uploading` | отправка заблокирована, hint `sendBlocked` |
| `ready` | все чипы `ready` | отправка разрешена |
| `error` | есть чип в любом `error-*` | отправка заблокирована, hint `sendBlockedError` |

Отправка сообщения: тело `POST /channels/{id}/messages` содержит
`message.attachments[].uploaded_filename = upload_filename` `[D]` §1.7. Чипы без
`uploaded_filename` в тело не попадают. После успешной отправки очередь очищается, object URL
отзываются.

Правило «error блокирует отправку»: файл с ошибкой нельзя тихо выбросить. Пользователь либо
убирает чип, либо повторяет загрузку. Это отличие от «отправить что получилось» — осознанное `[A]`.

### 3.5 Порядок, параллельность, повторы

1. Слот: один `POST /channels/{id}/attachments` на файл, тело `{"files":[{filename,file_size,id,
   is_clip:false}]}` — ровно четыре ключа `[D]` §1.3. Поле `original_content_type` из дампа
   в бэкенд B2a (`AttachmentFileSpec`) не входит, лишние ключи его валидация запрещает — поэтому
   UI его не отправляет; MIME уезжает только в `PUT` (§3.5 п. 7).
   Заголовок `X-Discord-Original-MD5` — только если MD5 уже посчитан локально; иначе не
   отправляется (`null → без заголовка`) `[D]`.
2. Одновременно не больше **2** конвейеров (слот + PUT) `[A]`: 10 файлов не должны забивать
   канал и очередь сообщений.
3. Автоповтор слота и PUT — **2** попытки с задержкой 1 с и 2 с; дальше `error-network` +
   «Повторить» `[A]`. Клиент ретраит 12 раз с таймаутом 1 час `[D]` §1.4 — для UI это
   неприемлемо долго, поэтому лимит снижен осознанно; расхождение зафиксировано.
4. Докачка после обрыва (`error-network` → «Повторить»): сначала
   `GET {upload_url}` с `Content-Range: bytes */*`; `200/201` → файл уже целиком, чип сразу
   `ready`; `308` → позиция из `Range`, продолжение с `range.end + 1` `[D]` §1.4.
5. Таймаут одного PUT — 60 с `[A]` (не 1 час, как в клиенте: UI должен уметь сказать «не вышло»).
6. `AbortController` на каждый чип; удаление чипа отзывает PUT, `object URL`, отсчёты и rAF.
7. MIME для PUT: `file.type || "application/octet-stream"` `[D]` §1.4.
8. Сжатие, ресайз, генерация waveform/thumbnail контента — вне карточки `[D]` §8.
9. Числа 10 файлов и 10 МиБ — общие с бэкендом B2a (`MAX_ATTACHMENTS_PER_MESSAGE`,
   `MAX_UPLOAD_FILE_BYTES`, `PM/b2a-attach/REPORT.md`). В JS-модуле они живут одной константой
   (`ATTACH_LIMITS`), а не разбросаны по коду; при изменении лимита на бэкенде UI обязан
   получить новое значение из того же источника, а не «поправить текст».

## 4. Кнопка 🎁 и модалка `#gift-modal`

### 4.1 Кнопка

```html
<button class="icon-btn composer-gift" type="button"
        data-action="composer-gift"
        aria-label="Подарок"
        aria-haspopup="dialog"
        aria-expanded="false"
        aria-controls="gift-modal"
        title="Отправить подарок">🎁</button>
```

| Свойство | Контракт | Метка |
|-|-|-|
| `data-action` | `composer-gift` (канонический; старый `gift` удаляется, см. §7) | `[A]` |
| `aria-haspopup` / `aria-expanded` / `aria-controls` | `dialog`, `false`/`true`, `gift-modal` — как у компонента `ChannelPremiumGiftButton` (`aria-haspopup:"dialog"`) | `[D]` §5.1 |
| Видимость | кнопка остаётся в DOM всегда; при недоступности — `aria-disabled="true"` и `title="Подарки недоступны в этом канале"` | `[A]` (правило «disabled не удаляет» — `composer.css` CMP-07) |
| Клик при `aria-disabled="true"` | ничего не открывает, `console.error`/`ACTION_FAILED` не появляется | `[A]` |
| Повторный клик при открытой модалке | модалка не закрывается и не пересоздаётся; фокус переводится в карточку | `[A]` |
| Локация аналитики | `gift-button` (повышающий баннер — отдельная карточка) | `[D]` §5.1 |
| Фокус | `:focus-visible` → `--focus-ring`; после закрытия модалки фокус возвращается сюда | `[T]`; `[A]` |

### 4.2 DOM модалки

Хост — `#gift-modal` (`.dialog-backdrop`, `hidden` по умолчанию), карточка — `.dialog`
с `role="dialog"`, как в `join-server.js`/`actions.js`. Ширина — `--modal-w-md` (602) `[T]`.

```html
<div id="gift-modal" class="dialog-backdrop" hidden>
  <div class="dialog gift-modal-card" role="dialog" aria-modal="true"
       aria-labelledby="gift-modal-title" aria-describedby="gift-modal-desc"
       data-state="closed">

    <header class="gift-head">
      <h2 id="gift-modal-title" class="dialog-title">Подарить Nitro</h2>
      <button class="icon-btn gift-close" type="button"
              data-action="gift-close" aria-label="Закрыть">✕</button>
    </header>

    <p id="gift-modal-desc" class="dialog-text">
      Подарок придёт выбранному получателю. Оплата спишется сразу.
    </p>

    <div class="gift-recipient">
      <span class="gift-label">Получатель</span>
      <span id="gift-recipient-name" class="gift-recipient-name">Получатель не выбран</span>
    </div>

    <fieldset class="gift-plans" id="gift-plans">
      <legend class="gift-label">Что подарить</legend>
      <label class="gift-plan">
        <input type="radio" name="gift-plan" value="1" checked>
        <span class="gift-plan-name">1 месяц</span>
        <span class="gift-plan-price"></span>
      </label>
      <!-- остальные планы генерируются из данных; цен нет до загрузки -->
    </fieldset>

    <label class="gift-custom">
      <span class="gift-label">Сообщение к подарку (необязательно)</span>
      <textarea id="gift-custom-message" class="gift-custom-input" rows="3"
                maxlength="200"
                placeholder="Напишите что-нибудь приятное…"></textarea>
      <span id="gift-custom-count" class="gift-counter">0 / 200</span>
    </label>

    <p id="gift-error" class="dialog-error" role="alert" aria-live="assertive" hidden></p>

    <div id="gift-success" class="gift-success" hidden>
      <p class="gift-success-title">Подарок отправлен</p>
      <p id="gift-success-code" class="gift-success-code"></p>
      <button class="gift-copy" type="button"
              data-action="gift-copy-code">Скопировать код</button>
    </div>

    <div class="dialog-actions">
      <button class="btn btn-secondary" type="button" data-action="gift-close">Отмена</button>
      <button id="gift-submit" class="btn btn-primary" type="button"
              data-action="gift-submit">Подарить</button>
    </div>
  </div>
</div>
```

| Элемент | Контракт | Метка |
|-|-|-|
| `#gift-modal` | `hidden` при закрытии; клик по фону и Escape закрывают (кроме `submitting`) | `[A]` |
| `.gift-modal-card` | `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `aria-describedby`, `data-state` | `[A]` |
| `#gift-plans` | `fieldset` + `legend`; планы — `input[type=radio]`, roving-порядок нативный | `[A]` |
| `#gift-custom-message` | `maxlength="200"`; счётчик `0 / 200`; при пустом — `gift_style = 1` (см. §4.4) | `[A]`; стили `[S]` §5.3 |
| `#gift-error` | `role="alert"`, `aria-live="assertive"`, один регион на модалку | `[A]` |
| `#gift-submit` | `type="button"` (не `submit`: формы нет), double-click защищён `data-state="submitting"` | `[A]` |
| Фокус-трап | Tab/Shift+Tab внутри карточки, `trapTab` по образцу `actions.js:891` | `[A]` |
| Возврат фокуса | на `[data-action="composer-gift"]` | `[A]` |

### 4.3 Состояния модалки (`data-state` на `.gift-modal-card`)

| Состояние | Условие | Что видно | Действия |
|-|-|-|-|
| `loading` | модалка открыта, планы ещё не пришли | текст `loading`, `aria-busy="true"`, submit disabled, крестик активен | Закрыть |
| `ready` | планы есть, получатель известен | планы, сообщение, submit enabled | Подарить, Закрыть |
| `submitting` | запрос `POST /users/@me/entitlements/gift-codes` в полёте | submit «Отправляем…» disabled, закрытие disabled | нет |
| `success` | 2xx, код подарка получен | `#gift-success`, submit скрыт | Скопировать код, Готово |
| `error-429` | 429 + `retry-after` | `#gift-error` с отсчётом, submit disabled до нуля | Закрыть |
| `error-403` | 401/403 | `#gift-error`: «Подарки недоступны для этого аккаунта.» | Закрыть |
| `error-400` | `50035 INVALID_FORM_BODY` / 400 | `#gift-error`: «Не удалось создать подарок. Проверьте данные.» | Подарить снова |
| `error-network` | обрыв/таймаут/5xx | `#gift-error`: «Нет связи с сервером. Подарок не отправлен.» | Подарить снова |
| `closed` | хост `hidden`, `data-state="closed"` | ничего; при следующем открытии — `loading` или `ready` | — |

Правила: при `submitting` карточка не закрывается ни Escape, ни фоном, ни крестиком —
иначе пользователь не узнает, ушёл подарок или нет. После `success` модалка остаётся открытой
до «Готово», потому что код подарка нужно скопировать.

### 4.4 Сетевой контракт модалки

| Шаг | Запрос | Тело | Метка |
|-|-|-|-|
| Создать подарок | `POST /users/@me/entitlements/gift-codes` | `{sku_id, subscription_plan_id, gift_style}` | `[D]` §5.3 |
| `gift_style` | `0` DEFAULT, `1` CUSTOM_STYLE, `2` CUSTOM_MESSAGE_EMOJI_SOUNDBOARD `[S]`; расчёт в UI — `style = 2` при непустом сообщении, иначе `1`; `0` — только при включённых флагах (`Fr`/`v1`), которых у нас нет | `[S]` §5.3; `[A]` маппинг без флагов |
| Ответ | код подарка; поля ответа в RE не зафиксированы | `[A]` |
| 429 | `retry-after` в секундах, как общий обработчик | `[D]` §1.6 |
| Список SKU/планов | endpoint в RE не найден; планы приходят в модалку параметром | `[A]` gap |

Обработка ответа: если в теле есть `code` — он показывается и копируется; если поле
называется иначе, `success` показывает только «Подарок отправлен», а тело пишется в
диагностический лог `[A]`. Выдумывать имя поля нельзя — это остаточный gap §8.

Цена: **не хардкодится**. Пока планы не пришли, `.gift-plan-price` пуст и submit disabled;
валюта и формат — из данных планов `[A]`.

### 4.5 Тексты (русский, дословно)

```js
export const GIFT_TEXT = {
  button: "Подарок",
  buttonTitle: "Отправить подарок",
  unavailable: "Подарки недоступны в этом канале",

  title: "Подарить Nitro",
  desc: "Подарок придёт выбранному получателю. Оплата спишется сразу.",
  recipientLabel: "Получатель",
  recipientEmpty: "Получатель не выбран",
  plansLabel: "Что подарить",
  customLabel: "Сообщение к подарку (необязательно)",
  customPlaceholder: "Напишите что-нибудь приятное…",
  customCounter: (n, max) => `${n} / ${max}`,
  loading: "Загружаем варианты подарка…",

  submit: "Подарить",
  submitting: "Отправляем…",
  close: "Закрыть",
  cancel: "Отмена",

  rateLimited: (sec) => `Слишком много попыток. Повторить через ${sec} с.`,
  forbidden: "Подарки недоступны для этого аккаунта.",
  badRequest: "Не удалось создать подарок. Проверьте данные.",
  network: "Нет связи с сервером. Подарок не отправлен.",

  successTitle: "Подарок отправлен",
  successCode: (code) => `Код подарка: ${code}`,
  copyCode: "Скопировать код",
  copied: "Код скопирован.",
  copyFailed: "Не удалось скопировать код.",
  done: "Готово",
};
```

### 4.6 CSS модалки

Кладётся в новый `ui/components/gift.css` (не в `composer.css`: модалка не живёт в композере).
Общие классы `.dialog-backdrop`, `.dialog`, `.dialog-title`, `.dialog-text`, `.dialog-error`,
`.dialog-actions`, `.btn` и `.dialog-actions .btn-primary` уже есть в `server-menu.css`
и переиспользуются как есть. Кнопка «Скопировать код» стоит вне `.dialog-actions`, поэтому
получает собственный класс `.gift-copy` (§4.6) — не полагается на `.btn`, который объявлен
только внутри `.dialog-actions`.

```css
.gift-modal-card {
  width: min(var(--modal-w-md), calc(100vw - var(--space-11)));
  max-height: calc(100vh - var(--space-11) * 2);
  overflow-y: auto;
  background: var(--surface-2);
}

.gift-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.gift-close {
  flex: 0 0 auto;
}

.gift-label {
  color: var(--text-subtle);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-medium);
  letter-spacing: var(--letter-spacing-normal);
}

.gift-recipient {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.gift-recipient-name {
  overflow: hidden;
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.gift-plans {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 0;
  border: 0;
}

.gift-plan {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  min-height: var(--btn-height-lg);
  padding: 0 var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: transparent;
  cursor: pointer;
}

.gift-plan:hover {
  background: var(--state-hover);
}

.gift-plan:focus-within {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.gift-plan:has(input:checked) {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.gift-plan-name {
  color: var(--text);
  font-size: var(--font-size-ui);
}

.gift-plan-price {
  margin-left: auto;
  color: var(--text-subtle);
  font-size: var(--font-size-ui);
  font-variant-numeric: tabular-nums;
}

.gift-custom {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.gift-custom-input {
  min-height: var(--space-12);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-input);
  color: var(--text);
  font: inherit;
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
  resize: vertical;
}

.gift-custom-input::placeholder {
  color: var(--text-muted);
}

.gift-counter {
  align-self: flex-end;
  color: var(--text-muted);
  font-size: var(--font-size-caption);
  font-variant-numeric: tabular-nums;
}

.gift-success {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  border-radius: var(--radius-sm);
  background: var(--surface-1);
}

.gift-success[hidden] {
  display: none;
}

.gift-success-title {
  margin: 0;
  color: var(--positive-text);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-semibold);
}

.gift-success-code {
  margin: 0;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: var(--font-size-ui);
  overflow-wrap: anywhere;
}

/* Кнопка копирования живёт вне .dialog-actions, поэтому свои правила. */
.gift-copy {
  align-self: flex-start;
  min-height: var(--btn-height);
  padding: 0 var(--btn-pad-x);
  border: 1px solid var(--border-strong);
  border-radius: var(--btn-radius);
  background: transparent;
  color: var(--text-accent);
  font: inherit;
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
}

.gift-copy:hover {
  background: var(--state-hover);
}

.gift-copy:focus-visible {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

.gift-modal-card[data-state="submitting"] .gift-close,
.gift-modal-card[data-state="submitting"] #gift-submit,
.gift-modal-card[data-state="submitting"] [data-action="gift-close"] {
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}
```

`.dialog-actions .btn-primary` уже задан в `server-menu.css` (accent-токены) — новый стиль не
нужен. Белый текст на `--accent` — 4.607, AA проходит в обеих темах (`THEME-SPEC.md` §2.3).
Готовый `.btn-danger` не используется: подарок — не destructive-действие.

## 5. Таблица CSS-токенов

### 5.1 Существующие токены (менять значения не нужно)

| Токен | dark | light | Метка | Где используется |
|-|-|-|-|-|
| `--surface-1` | `#202024` | `#efeff1` | `[T]` `[L]` | фон блока получателя и success |
| `--surface-2` | `#242429` | `#f3f3f4` | `[T]` `[L]` | фон чипа и карточки модалки |
| `--surface-3` | `#28282d` | `#ebeced` | `[T]` `[L]` | фон превью-заглушки |
| `--surface-input` | `#1e1f22` | `#ffffff` | `[T]` `[L]` | textarea сообщения к подарку |
| `--accent` | `#5865f2` | `#5865f2` | `[T]` `[L]` | заливка прогресса, выбранный план, primary |
| `--accent-hover` | `#4654c0` | `#4654c0` | `[T]` `[L]` | hover primary |
| `--accent-active` | `#3c45a5` | `#3a48a3` | `[T]` `[L]` | pressed primary |
| `--accent-soft` | `rgb(88 101 242 / 16%)` | `rgb(88 101 242 / 12%)` | `[T]` `[L]` | выбранный план, dropzone |
| `--danger` | `#da3e44` | `#da3e44` | `[T]` `[L]` | заливка прогресса при `error-network` |
| `--danger-text` | `#ff9a9e` | `#bd2934` | `[T]` `[L]` | текст ошибок, счётчик на лимите |
| `--positive-text` | `#57d98a` | `#01713a` | `[T]` `[L]` | «Готово», заголовок успеха |
| `--text` | `#efeff1` | `#3f4048` | `[T]` `[L]` | имя файла, текст модалки |
| `--text-strong` | `#fbfbfb` | `#323339` | `[T]` `[L]` | получатель, заголовок |
| `--text-subtle` | `#abacb2` | `#5f606a` | `[T]` `[L]` | размер файла, label, цена |
| `--text-muted` | `#96979e` | `#696a73` | `[T]` `[L]` | статус, счётчики, placeholder |
| `--text-accent` | `#a5b0ff` | `#4654c0` | `[T]` `[L]` | «Повторить», secondary-кнопка |
| `--text-on-accent` | `#ffffff` | `#ffffff` | `[T]` `[L]` | текст на primary |
| `--border-subtle` | `rgb(151 151 159 / 12%)` | `rgb(0 0 0 / 8%)` | `[T]` `[L]` | бордер чипа, блок получателя |
| `--border` | `rgb(151 151 159 / 20%)` | `rgb(0 0 0 / 14%)` | `[T]` `[L]` | бордер плана |
| `--border-strong` | `rgb(151 151 159 / 44%)` | `rgb(0 0 0 / 48%)` | `[T]` `[L]` | бордер textarea |
| `--state-hover` | `rgb(255 255 255 / 6%)` | `rgb(0 0 0 / 5%)` | `[T]` `[L]` | hover кнопок и планов |
| `--state-active` | `rgb(255 255 255 / 9%)` | `rgb(0 0 0 / 8%)` | `[T]` `[L]` | трек прогресса |
| `--state-disabled-opacity` | `0.45` | `0.45` | `[T]` `[L]` | submitting/disabled |
| `--focus-ring` | `#a5b0ff` | `#4654c0` | `[T]` `[L]` | `:focus-visible` всех контролов |
| `--focus-ring-width` | `2px` | `2px` | `[T]` | ring |
| `--focus-ring-offset` | `2px` | `2px` | `[T]` | ring |
| `--shadow-popover` | `0 8px 16px rgb(0 0 0 / 24%), 0 0 0 1px rgb(0 0 0 / 16%)` | `0 8px 16px rgb(0 0 0 / 12%), 0 0 0 1px rgb(0 0 0 / 8%)` | `[T]` `[L]` | карточка модалки (через `.dialog`) |
| `--font-size-caption` | `11px` | `11px` | `[T]` | счётчик файлов, счётчик символов подарка |
| `--font-size-small` | `12px` | `12px` | `[T]` | метаданные чипа, label |
| `--font-size-ui` | `14px` | `14px` | `[T]` | текст модалки, кнопки |
| `--line-height-small` | `16px` | `16px` | `[T]` | чип |
| `--line-height-ui` | `18px` | `18px` | `[T]` | модалка |
| `--space-1…6` | `2…16px` | те же | `[T]` | паддинги и зазоры |
| `--space-11` | `48px` | `48px` | `[T]` | поля модалки от краёв окна |
| `--space-12` | `56px` | `56px` | `[T]` | min-height textarea |
| `--radius-xs` | `4px` | `4px` | `[T]` | превью, кнопка чипа |
| `--radius-sm` | `8px` | `8px` | `[T]` | чип, карточка, поля |
| `--radius-round` | `2147483647px` | то же | `[T]` | трек прогресса |
| `--btn-height` | `32px` | `32px` | `[T]` | «Скопировать код» |
| `--btn-height-sm` | `28px` | `28px` | `[T]` | кнопки чипа |
| `--btn-height-lg` | `38px` | `38px` | `[T]` | строка плана |
| `--btn-pad-x` | `16px` | `16px` | `[T]` | `.btn` |
| `--btn-radius` | `8px` | `8px` | `[T]` | `.btn` |
| `--icon-size` | `20px` | `20px` | `[T]` | крестик чипа |
| `--icon-size-lg` | `24px` | `24px` | `[T]` | иконка типа файла, «+» |
| `--modal-w-md` | `602px` | `602px` | `[T]` | ширина модалки |
| `--modal-pad-x` / `--modal-pad-y` | `24px` / `16px` | те же | `[T]` | паддинги карточки |
| `--font-mono` | системный моно | то же | `[T]` | код подарка |
| `--motion-fast` | `120ms` (0 при reduced motion) | то же | `[T]` | анимация прогресса |
| `--z-modal` | `200` | `200` | `[T]` | `.dialog-backdrop` |
| `--layout-composer-h` | `56px` | `56px` | `[T]` | высота композера не меняется |

### 5.2 Новые токены (3, тема-агностичные)

Добавляются в `:root` в `ui/tokens.css`; light-переопределений не требуют, поэтому список
57 имён из `THEME-SPEC.md` §4.2 не меняется.

| Токен | Значение | Метка | Зачем |
|-|-|-|-|
| `--attach-chip-w` | `248px` | `[A]` | ширина чипа вложения |
| `--attach-chip-thumb` | `40px` | `[A]` | превью/иконка в чипе (= `--avatar-lg`, но отдельное имя: семантика другая) |
| `--progress-h` | `4px` | `[A]` | высота полосы прогресса |

Не-токен: `--attach-queue-h` — runtime-переменная на `#screen-chat`, пишется JS, fallback `0px`.
В `tokens.css` её нет.

### 5.3 Контраст и светлая тема

| Пара | Значение | Требование | Метка |
|-|-|-|-|
| `--text` на `--surface-2` | 9.29 (light) | ≥ 4.5 | `[L]` THEME-SPEC §2.6 |
| `--text-subtle` на `--surface-2` | 5.62 | ≥ 4.5 | `[L]` |
| `--text-muted` на `--surface-2` | 4.84 | ≥ 4.5 | `[L]` |
| `--danger-text` на `--surface-2` | 5.36 | ≥ 4.5 | `[L]` |
| `--positive-text` на `--surface-2` | 5.53 | ≥ 4.5 | `[L]` |
| `--text-accent` на `--surface-2` | 5.76 | ≥ 4.5 | `[L]` |
| `--accent` vs `--surface-2` (прогресс) | 3.77 min | ≥ 3 | `[L]` non-text |
| `--danger` vs `--surface-2` | 3.61 | ≥ 3 | `[L]` |
| `--text-on-accent` на `--accent` | 4.607 | ≥ 4.5 | `[T]` `[L]` |
| `--text-muted` нельзя на `--surface-4` | 4.39 | — | `[L]` — в чипе `--surface-4` не используется |

Правила: цвет — не единственный носитель состояния (есть текст статуса и `aria-valuenow`);
`--text-faint` в чипе и модалке не применяется; `--warning` как текст не используется.

Светлая тема реализуется карточкой C3b (`:root[data-theme="light"]`). До неё G2 пишет только
`var()`: при включении C3b чип и модалка получают светлые значения без правок в `gift.css` и
`composer.css`. Проверять светлую тему на этих компонентах до C3b нечем — это `[A]`-зависимость,
а не забытая проверка.

## 6. Критерии приёмки для G2

### 6.1 Таблица AC

| ID | Критерий | Как проверяется |
|-|-|-|
| AC-01 | `[data-action="attach"]` в обычном состоянии не имеет `aria-disabled` и открывает `#composer-file-input.click()`; при 10 файлах получает `aria-disabled="true"` | тест: клик → шпион на `input.click`; атрибут после 10-го чипа |
| AC-02 | Выбор 3 файлов даёт 3 чипа в `#composer-attachments`, `role="listitem"`, порядок выбора сохранён | DOM-тест на фейковом `FileList` |
| AC-03 | `#composer-attachments` имеет `hidden` при пустой очереди и не имеет при непустой | тест атрибута |
| AC-04 | `#composer` сохраняет высоту 56px: очередь — `position: absolute`, `data-attach-state` не меняет `--layout-composer-h` | стилевой тест: `.composer-attachments { position: absolute }`, `height`/`--layout-composer-h` у `.composer` не переопределены; геометрия списка — `geometry-check` |
| AC-05 | Чип в `uploading` содержит `role="progressbar"` с `aria-valuenow` 0…100 и `aria-valuetext` «{n}%» | тест: после тика прогресса атрибуты обновлены |
| AC-06 | Прогресс монотонно не убывает, 100 — только после PUT 200/201 и наличия `upload_filename` | fake-адаптер: 60% → обрыв → 60%, не 0 |
| AC-07 | `error-413` показывается без сети, если файл больше лимита; «Повторить» отсутствует | тест: `POST` не вызван, текст с лимитом |
| AC-08 | `error-413` также по HTTP 413 и по `body.code === 40005` | fake-ответы обоих видов |
| AC-09 | `error-429` читает `retry-after`, показывает отсчёт, «Повторить» disabled до нуля, затем enabled | fake-ответ с `retry-after: 3`, фейковые таймеры |
| AC-10 | Отсутствие `retry-after` даёт 5 с, а не `NaN` | fake-ответ без заголовка |
| AC-11 | `error-empty` для файла 0 байт, `POST` не вызывается | тест |
| AC-12 | «Убрать» прерывает PUT (`AbortController`), отзывает object URL и удаляет чип из DOM | тест: `abort` вызван, `URL.revokeObjectURL` вызван |
| AC-13 | Отправка блокируется при `queued`/`uploading`/`error`, разрешена при `ready`; hint-текст меняется | тест `data-attach-state` и `#composer-attach-hint` |
| AC-14 | Тело сообщения содержит `attachments[].uploaded_filename` для каждого `ready`-чипа и не содержит чипов без него | тест собранного payload |
| AC-15 | В `ready`-чипе размер и имя отформатированы по §2.3 (`2,4 МиБ`, неразрывный пробел), имя обрезано ellipsis с полным `title` | unit-тест `formatBytes` + DOM |
| AC-16 | `#composer-attach-status` — единственный live-region очереди; прогресс в него не пишется | тест: 3 тика прогресса → 0 записей, 1 старт → 1 запись |
| AC-17 | `[data-action="composer-gift"]` имеет `aria-haspopup="dialog"`, `aria-controls="gift-modal"`, `aria-expanded` переключается | DOM-тест |
| AC-18 | Открытие модалки ставит фокус на первый план (`checked` radio) или на заголовок; Tab не выходит за карточку; Escape закрывает и возвращает фокус на кнопку | тест фокуса |
| AC-19 | `.gift-modal-card[data-state]` проходит все состояния §4.3; при `submitting` Escape/фон/крестик не закрывают | тест переходов |
| AC-20 | `POST /users/@me/entitlements/gift-codes` получает `{sku_id, subscription_plan_id, gift_style}`; `gift_style = 2` при непустом сообщении и `1` при пустом (флаги `Fr`/`v1` недоступны → `0` не отправляется) | тест тела запроса и функции `resolveGiftStyle` |
| AC-21 | Повторный клик «Подарить» не даёт второго запроса | тест: 2 клика → 1 вызов |
| AC-22 | `429` в модалке: отсчёт, submit disabled до нуля, затем повтор | фейковые таймеры |
| AC-23 | Ни один `data-action` из документа не даёт `console.error`/`ACTION_FAILED` при клике в любом состоянии | тест на `consoleErrors`/`consoleWarns` (как в `gui-spike-features.test.mjs:1611`) |
| AC-24 | В новых CSS нет `!important`, нет hex/rgb-литералов цвета, нет px-литералов размера (кроме hairline `1px`); все значения — `var()` | grep по `composer.css`/`gift.css` + review |
| AC-25 | `prefers-reduced-motion: reduce` отключает анимацию прогресса | тест медиазапроса/ревью CSS |
| AC-26 | Светлая тема не ломается: новые правила не содержат цветовых литералов, зависят только от `var()`; проверка значений — после C3b | review + `THEME-SPEC` §2.6 (ручной пересчёт пар) |
| AC-27 | `npx tsc -p tsconfig.json` — 0 ошибок; JSDoc и `// @ts-check` в новых модулях | гейт |
| AC-28 | Целевые тесты зелёные, `npm run gate:worker` пройден; red-логи приложены | гейт |
| AC-29 | Лимиты UI совпадают с бэкендом B2a: 10 файлов и 10 МиБ по умолчанию, числа лежат в одной константе `ATTACH_LIMITS`, текст ошибки берёт значение из неё | unit-тест константы + текст `tooLarge` |

### 6.2 Файловая матрица G2

| Файл | Что делает G2 | Статус |
|-|-|-|
| `rust/gui-spike/ui/components/composer.css` | блок «вложения» из §2.4 | правка |
| `rust/gui-spike/ui/components/gift.css` | новый файл из §4.6 | создание |
| `rust/gui-spike/ui/features/attachments.js` | очередь, состояния, сеть, форматтеры | создание |
| `rust/gui-spike/ui/features/gift-modal.js` | модалка, состояния, запрос | создание |
| `rust/gui-spike/ui/index.html` | `#composer-attachments`, `#composer-attach-count`, `#composer-file-input`, `#composer-attach-status`, `#gift-modal`, подписи | правка (блок композера и хоста модалки) |
| `rust/gui-spike/ui/features/actions.js` | регистрация `composer-gift`, `gift-close`, `gift-submit`, `gift-copy-code`, `attachment-remove`, `attachment-retry`; снять заглушки `attach`/`gift` | правка (координация с TL: файл общий) |
| `rust/gui-spike/ui/tokens.css` | 3 токена §5.2 | правка (общий файл — координация с TL) |
| `rust/gui-spike/ui/app.js` | проводка модулей (минимальная) | правка (общий файл — координация с TL) |
| `rust/gui-spike/src/attachments.rs`, `rust/gui-spike/tests/attachments_contract.rs` | не трогать: контракт слота уже реализован B2a | **чтение** (владелец — G1/B2a) |
| `tests/unit/attachments-ui.test.mjs` | новые тесты AC-01…AC-16 | создание |
| `tests/unit/gift-modal.test.mjs` | новые тесты AC-17…AC-22 | создание |
| `tests/unit/gui-spike-dom.test.mjs`, `tests/unit/gui-spike-features.test.mjs` | правка ожиданий контракта — §7 | правка по согласованию с TL |

Запрещено в этой карточке: сеть в unit-тестах (только fake-адаптер), правки `logs/`,
`functions/`, `evidence/`, чужих фич, `package.json`, ретраи «зеленения» через `skip`/`.only`.

## 7. Влияние на замороженные проверки

Контракт меняется: кнопки `attach` и `gift` больше не заглушки Phase 3. Это осознанная смена
контракта этой карточкой, а не подгонка тестов под код. Порядок для G2: сначала правится
ожидание теста (red), затем код (green), в отчёте — ссылка на этот раздел.
Блок `gui-spike-features.test.mjs:1581-1613` целиком описывает контракт «заглушек» и правится
одним куском.

| Файл:строка | Сейчас | Станет | Почему |
|-|-|-|-|
| `tests/unit/gui-spike-dom.test.mjs:283-288` | `data-action="gift"`, `aria-disabled="true"` | `data-action="composer-gift"`, `aria-haspopup="dialog"`, `aria-controls="gift-modal"`, без `aria-disabled` | §4.1 |
| `tests/unit/gui-spike-features.test.mjs:1611` | `action(app,"gift")` — заглушка, `title` содержит «Phase 3» | `action(app,"composer-gift")` открывает `#gift-modal`, `consoleErrors` пуст | §4.3 |
| `tests/unit/gui-spike-features.test.mjs:1601-1603` | `attach` — `aria-disabled="true"` + `title` | `attach` активна, `title="Прикрепить файл (до 10)"` | §1.1 |
| `ui/features/actions.js:783-794` | блоки, ставящие `aria-disabled` на `attach` и `gift` | удаляются; вместо них — регистрация действий (`composer-gift` вместо `gift` в реестре, `actions.js:686`) | §1.1, §4.1 |

Остальные проверки (эмодзи, inbox, bell, настройки) не затрагиваются.

## 8. Пробелы и что вне карточки

| # | Пробел | Метка | Что нужно |
|-|-|-|-|
| G-01 | Поля ответа `POST /users/@me/entitlements/gift-codes` (имя поля с кодом) не подтверждены | `[A]` | live-захват одного подарка |
| G-02 | Endpoint списка SKU/планов для модалки не найден | `[A]` | RE ленивого чанка `806677` |
| G-03 | Цены и валюта планов | `[A]` | из API планов; не хардкодить |
| G-04 | Пиксельная геометрия чипа и модалки в дампе отсутствует | `[A]` | замер после реализации на живом окне (окно согласует координатор) |
| G-05 | `gift_style = 1` (CUSTOM_STYLE) и кастомная графика подарка | `[A]` | отдельная карточка |
| G-06 | Сжатие/ресайз изображений, waveform, thumbnail-контент | вне карточки | `reference-oracle-REA.md` §8 |
| G-07 | Перетаскивание чипов внутри очереди | `[A]` | не в MVP |
| G-08 | Spoiler-флаг вложения (`is_spoiler`) | `[D]` есть в теле сообщения | отдельная карточка |
| G-09 | Светлая тема не реализована (C3b queued) | `[A]` | C3b; до неё проверка только по §5.3 |

## 9. Что не делалось

Продуктовый код не менялся: `rust/gui-spike/ui/**`, `src/**`, тесты, `package.json` не тронуты.
Окно, сеть, cargo, npm-гейты и коммиты не запускались. Ассеты RE не копировались: в документе
только имена полей, числа лимитов и тексты, написанные заново. Замороженные проверки §7 не
правились — это задача G2 вместе с реализацией.
