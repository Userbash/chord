# RW-40 / G3 — ATTACHMENTS-RENDER-SPEC: рендер вложений в ленте сообщений (AT-03)

Статус: дизайн-контракт (спецификация; продуктовый код и тесты не изменялись). Дата: 2026-10-30.  
Владелец: **G3 Design** (карточка `G3-AT03-RENDER-SPEC`).  
Потребители: **G2 Frontend** (карточка `AT-03`, WSJF 4.5 в `waves/RW-40/PARITY/GAP-BACKLOG.md`; целевой тест `tests/unit/attachments_render.test.mjs`), **G1 Core / Backend** (`MessageAttachment` в `rust/gui-spike/src/data.rs`, продление ссылок `POST /attachments/refresh-urls`).

Нормативные источники и оракулы:
- `waves/RW-40/ATTACH-RENDER-RE.md` — полный RE-разбор схемы `attachment` по 72 живым объектам из 16 снапшотов каналов, битовой маски `AttachmentFlags`, трёх классификаторов (`Rr`, `FE`, `TX`), пайплайна `renderAttachments()`, резолвера причин скрытия (`spoiler` / `explicit`), декодера `waveform` и правил CDN/refresh;
- `rust/gui-spike/ui/tokens.css` — единственный нормативный источник CSS-токенов (поверхности, текст, акценты, статусы, границы, состояния, типографика, отступы, радиусы, тени, раскладка, высоты строк, размеры компонентов, слои и движение);
- `waves/RW-40/THEME-SPEC.md` (§2, §2.6, §4) — правила контраста WCAG 2.2 AA в тёмной и светлой темах без добавления новых имён переменных сверх `rust/gui-spike/ui/tokens.css`;
- `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` — контракт очереди вложений в композере (предшествующий этап жизненного цикла до попадания вложения в ленту сообщений);
- `waves/RW-40/DESIGN/TYPING-MARKDOWN-SPEC.md` (§2.4) — единый паттерн интерактивных спойлеров (`data-spoiler="hidden|revealed"`, активация по клику, `Enter` и `Space`);
- `rust/gui-spike/ui/components/chat.css` и `rust/gui-spike/ui/features/chat.js` — DOM-инвариант строки сообщения (`.row` содержит ровно два дочерних узла `.author` и `.text`, а все аксессуары рендерятся внутри `.text[data-stack="true"]`).

---

## 0. Легенда, правила токенизации и архитектурные инварианты

| Метка | Значение | Источник |
|---|---|---|
| `[D]` | Подтверждённый факт из дампа / live-бандла / REST-снапшотов | `waves/RW-40/ATTACH-RENDER-RE.md` |
| `[S]` | Следует из кода бандла косвенно (имена пропов, сигнатуры) | `waves/RW-40/ATTACH-RENDER-RE.md` |
| `[T]` | Существующий CSS-токен из `rust/gui-spike/ui/tokens.css` | `rust/gui-spike/ui/tokens.css` |
| `[L]` | Правила светлой темы и матрицы контраста WCAG 2.2 AA | `waves/RW-40/THEME-SPEC.md` |
| `[A]` | Проектное решение / адаптация для `AT-03` (закрытие ленивых чанков `d2`/`d3`/`dJ`) | Настоящая спецификация |
| `[F]` | Инвариант существующих контрактных тестов DOM | `tests/unit/gui-spike-dom.test.mjs` |

### 0.1. Строгая привязка к `rust/gui-spike/ui/tokens.css` (`100% token binding`)

1. **Только существующие токены**: все CSS-правила рендера вложений используют исключительно кастомные свойства `var(--*)`, объявленные в `rust/gui-spike/ui/tokens.css`. Введение новых имён CSS-переменных, а также любых сырых цветовых литералов в CSS-правилах и тексте настоящей спецификации строго запрещено.
2. **Без внешних ассетов**: запрещены внешние растровые изображения для UI-контролов, иконочные веб-шрифты и сторонние ссылки. Шрифты — только системные `var(--font-sans)` и `var(--font-mono)` (`rust/gui-spike/ui/tokens.css` §5). Иконки типов файлов и элементов управления плеером — встроенные векторные/символьные глифы с окраской через `currentColor`.
3. **Защита от XSS и проверка URL (`safeHref`)**:
   - Ни одно строковое поле вложения (`filename`, `title`, `description`, `content_type`) не вставляется через `innerHTML` — используется исключительно `document.createElement(...)` и `textContent`.
   - Ссылки `url` и `proxy_url` перед установкой в `href` или `src` проходят валидацию `safeHref`: допускаются только абсолютные URL со схемами `http:` и `https:` (а для локальных оптимистичных превью в очереди — `blob:` только до отправки). Любые `javascript:`, `data:`, `vbscript:`, `file:` или невалидные строки отвергаются (`safeHref` возвращает `null`), а соответствующий элемент переводится в честное состояние `aria-disabled="true"` или `data-state="error"`.
4. **DOM-инвариант строки сообщения (`DOM-CONTRACT §7.1`)**:
   - Внутри `.row` всегда остаются ровно два прямых дочерних элемента: `.author` и `.text` (`[F]`).
   - Контейнер вложений `.message-attachments` монтируется **внутри** `.text` (при этом на `.text` выставляется `data-stack="true"`, а на `.row` в список токенов атрибута `data-extras` добавляется `"attachments"`), располагаясь после `.text-body` и перед `.embeds` / `.reactions-row`.

---

## 1. Контракт данных `attachment` и правила классификации (`ATTACH-RENDER-RE.md`)

### 1.1. Схема объекта вложения и флагов сообщения

Согласно `waves/RW-40/ATTACH-RENDER-RE.md` (§2.1–§2.5, §3), рендерер сообщения принимает массив `message.attachments` и флаги сообщения `message.flags`:

```ts
interface MessageAttachmentRaw {
  id: string;                      // Обязательное: snowflake идентификатор вложения [D]
  filename: string;                // Обязательное: исходное имя файла с расширением [D]
  size: number;                    // Обязательное: размер в байтах (целое >= 0) [D]
  url: string;                     // Обязательное: прямая ссылка CDN (cdn.discordapp.com) с ?ex=&is=&hm= [D]
  proxy_url?: string;              // Обязательное в REST: прокси-ссылка (media.discordapp.net) [D]
  content_type?: string;           // MIME-тип; может содержать параметры, например "text/plain; charset=utf-8" [D]
  content_scan_version?: number;   // Версия сканера контента (1, 4; -1 = не просканировано) [D]
  width?: number | null;           // Ширина в пикселях (> 0 для image/video; отсутствует у audio/file) [D]
  height?: number | null;          // Высота в пикселях (> 0 для image/video; отсутствует у audio/file) [D]
  placeholder?: string | null;     // Base64-строка blurhash-заглушки [D]
  placeholder_version?: number;    // Версия формата placeholder; рендерится ТОЛЬКО при === 1 [D]
  original_content_type?: string;  // Исходный MIME до серверного перекодирования [D]
  duration_secs?: number | null;   // Длительность медиа в секундах (float, например 36.09559) [D]
  waveform?: string | null;        // Base64-массив байтов амплитуд (0..255) для голосовых сообщений [D]
  flags?: number;                  // Битовая маска AttachmentFlags [D]
  title?: string | null;           // Заголовок клипа или записи [D]
  description?: string | null;     // Пользовательский alt-текст (описание изображения/видео) [D]
  loading_state?: "LOADING" | "LOADED_SUCCESS"; // Клиентское состояние при оптимистичной отправке [D]
}
```

> [!IMPORTANT]
> **Отсутствие поля `ephemeral` иходящего `is_spoiler` во входящей схеме (`[D]` §2.5):**
> 1. У объекта `attachment` **нет** поля `ephemeral`. Эфемерность определяется флагом самого сообщения `EPHEMERAL = 64` (`1 << 6`) и путём `/ephemeral-attachments/` в `url` / `proxy_url`.
> 2. Входящий JSON от сервера передаёт признак спойлера через бит `IS_SPOILER = 8` в поле `attachment.flags` (а также историческим префиксом `SPOILER_` в `filename`). Ключ `is_spoiler` используется только в исходящем `PATCH /channels/{id}/messages/{id}` при редактировании вложений.

### 1.2. Битовая маска `AttachmentFlags` (`attachment.flags`) и флаги сообщения (`message.flags`)

| Константа | Значение | Назначение в рендере (`ATTACH-RENDER-RE.md` §3, §5, §7) | Метка |
|---|---:|---|---|
| `IS_CLIP` | `1` (`1 << 0`) | Помечает видео-клип (бейдж клипа, учёт `title` и `duration_secs`) | `[D]` |
| `IS_THUMBNAIL` | `2` (`1 << 1`) | **Исключается из inline-рендера** сообщения: `attachments.filter(a => !((a.flags ?? 0) & 2))` | `[D]` |
| `IS_REMIX` | `4` (`1 << 2`) | Метка ремикса; сохраняется в `data-attachment-flags`, визуально не меняет карточку | `[D]` |
| `IS_SPOILER` | `8` (`1 << 3`) | Активирует спойлер-оверлей `'СПОЙЛЕР'` (`obscureReason = "spoiler"`) | `[D]` |
| `CONTAINS_EXPLICIT_MEDIA` | `16` (`1 << 4`) | Скрытие по фильтру чувствительного контента (`obscureReason = "explicit_content"`) | `[D]` |
| `IS_ANIMATED` | `32` (`1 << 5`) | Помечает анимированное изображение (`GIF` / `WebP` / `AVIF`, атрибут `data-animated="true"`) | `[D]` |
| `CONTAINS_GORE_CONTENT` | `64` (`1 << 6`) | Скрытие по фильтру жестокого контента (`obscureReason = "gore_content"`) | `[D]` |
| `CONTAINS_SELF_HARM_CONTENT` | `128` (`1 << 7`) | Скрытие по фильтру самоповреждения (`obscureReason = "self_harm_content"`) | `[D]` |
| `HAS_TIMELINE_COMMENTS` | `256` (`1 << 8`) | Признак комментариев таймлайна; в inline-рендере не меняет верстку | `[D]` |
| `MessageFlags.EPHEMERAL` | `64` (`1 << 6`) | Флаг **сообщения**: эфемерное сообщение | `[D]` |
| `MessageFlags.IS_VOICE_MESSAGE` | `8192` (`1 << 13`) | Флаг **сообщения**: переключает рендер аудио-вложения с обычного плеера на плеер голосового сообщения с `waveform` | `[D]` |

### 1.3. Три классификатора и маршрутизация рендера (`[D]` §4.1–§4.5, §5)

Рендерер `renderAttachments(message, options)` выполняет фильтрацию и классификацию строго в следующем порядке:

1. **Гейт сообщения**: если `message.attachments` пуст, либо сообщение является системным/опросом (`isPoll`), контейнер `.message-attachments` не создаётся (`return null`).
2. **Фильтр миниатюр**: все элементы с битом `(attachment.flags ?? 0) & 2` (`IS_THUMBNAIL`) отбрасываются из inline-выдачи (`[D] 48550@223939`). Если после фильтрации список пуст — `return null`.
3. **Классификация первого сегмента MIME (`firstMimeSegment`)**:
   - Поскольку `content_type` может содержать параметры (`"text/plain; charset=utf-8"`), сравнение выполняется по подстроке до первого `/` или `;`:
     `const baseMime = (contentType ?? "").split(";")[0].trim().toLowerCase(); const [mimeTop] = baseMime.split("/");` (`[D] @6056280`).
4. **Классификация inline-медиа по расширению имени файла (`classifyInlineMedia`, функция `Rr` `[D] @5918776`)**:
   - Регулярное выражение изображения (`o.u`): `/\.(png|jpe?g|jfif|webp|gif|heic|heif|dng|avif)$/i`.
   - Регулярное выражение видео (`o.AE`): `/\.(mp4|webm|mov|qt)$/i`.
   - **Важное правило оракула `[D]` §4.1, §4.4, §4.5**:
     - Вложение попадает в медиа-сетку `.attachments-mosaic` как `"IMAGE"`, только если его `filename` матчит `/\.(png|jpe?g|jfif|webp|gif|heic|heif|dng|avif)$/i` (при этом `image/svg+xml` с именем `.svg` и `image/vnd.adobe.photoshop` с `.psd` **не** входят в этот список и рендерятся как файловые карточки `webcode` / `photoshop`).
     - Вложение попадает в медиа-сетку как `"VIDEO"`, если его `filename` матчит `/\.(mp4|webm|mov|qt)$/i` и при этом имеется валидный `proxy_url` (или `url` в локальном фоллбеке). Если обе стороны видео превышают `6016 × 3384` (в любой ориентации: `(w > 6016 && h > 3384) || (w > 3384 && h > 6016)`), выставляется `forceExternal = true` (`[D] @5929320`).
5. **Классификация аудио и голосовых сообщений (`AUDIO` / `VOICE_MESSAGE`)**:
   - Если вложение не попало в `IMAGE` / `VIDEO`, проверяется расширение аудио `/\.(mp3|ogg|opus|wav|aiff|flac)$/i` (`klass === "audio"`, `[D] @9704193`) либо первый сегмент `mimeTop === "audio"`:
     - При `(message.flags & 8192) !== 0` (`IS_VOICE_MESSAGE`): рендерится компонент **Голосовое сообщение** (`.attachment-voice-card` с `waveform`).
     - При `(message.flags & 8192) === 0`: рендерится компонент **Аудио-вложение** (`.attachment-audio-card` с метаданными файла и линейным прогресс-баром).
6. **Классификация файловых карточек (`classifyFile` / `TX`, `[D] @9704193`)**:
   - Все остальные вложения (включая `image/png` без расширения в `filename`, `.svg`, `.psd`, `.pdf`, архивы, код, документы, таблицы и бинарные файлы) рендерятся как **Файловое вложение** (`.attachment-file-card`) с иконкой класса `klass` по таблице §4.1.

### 1.4. Продление CDN-подписей (`refresh-urls`, `[D]` §8.1)

Ссылки `url` и `proxy_url` содержат query-параметры подписи `?ex=<hex>&is=<hex>&hm=<hex>`.
- Параметр `ex` — Unix-время истечения в шестнадцатеричном формате (`parseInt(ex, 16)` секунд).
- Если `exSeconds <= Math.floor(Date.now() / 1000) + 3600` (до истечения остался 1 час или меньше, и путь входит в `/attachments/` или `/ephemeral-attachments/`), клиент запрашивает обновление через `POST /attachments/refresh-urls` с телом `{ attachment_urls: [url] }` и заменяет адрес на `refreshed_urls[0].refreshed`.

---

## 2. Изображения (`IMAGE`): одиночные, сетка Mosaic (`1..4+`), пропорции, Skeleton, Spoiler, Lightbox

### 2.1. Контейнер `.message-attachments` и сетка `.attachments-mosaic`

Все визуальные медиа-вложения (`IMAGE` и `VIDEO`) одного сообщения группируются в контейнер `.attachments-mosaic`, за которым в потоке `.message-attachments` следуют карточки аудио (`.attachment-audio-card`, `.attachment-voice-card`) и файлов (`.attachment-file-card`).

| Количество медиа (`N`) | Атрибут `data-mosaic-count` | Атрибут `data-layout` | Раскладка сетки (`.attachments-mosaic`) | Размеры и ограничения (`[T]` / `[A]`) |
|---:|---|---|---|---|
| `1` | `"1"` | `"single"` | 1 колонка (`grid-template-columns: minmax(0, 1fr)`) | Контейнер `max-width: var(--modal-w-sm)` (`442px`), `max-height: calc(var(--emoji-picker-h) - var(--space-10))` (`400px`); сохраняет `aspect-ratio` исходного `width / height` |
| `2` | `"2"` | `"mosaic-2"` | 2 равные колонки (`grid-template-columns: repeat(2, minmax(0, 1fr))`), `gap: var(--space-2)` (`4px`) | Общая ширина до `var(--modal-w-md)` (`602px`); высота ряда до `calc(var(--layout-tree-w) + var(--space-8))` (`264px`), ячейки `aspect-ratio: 1 / 1` или `4 / 3` с `object-fit: cover` |
| `3` | `"3"` | `"mosaic-3"` | 2 колонки, 2 строки: элемент #1 занимает левую колонку на 2 строки (`grid-row: 1 / span 2`), элементы #2 и #3 — правую колонку друг над другом, `gap: var(--space-2)` (`4px`) | Общая ширина до `var(--modal-w-md)` (`602px`), высота сетки до `calc(var(--emoji-picker-h) - var(--space-10))` (`400px`) |
| `4` | `"4"` | `"mosaic-4"` | Сетка `2 × 2` (`grid-template-columns: repeat(2, minmax(0, 1fr))`, `grid-template-rows: repeat(2, minmax(0, 1fr))`), `gap: var(--space-2)` (`4px`) | Общая ширина до `var(--modal-w-md)` (`602px`); каждая ячейка с `aspect-ratio: 16 / 10` и `object-fit: cover` |
| `5..10` (`4+`) | `"{N}"` | `"mosaic-many"` | Сетка до 3 колонок (`grid-template-columns: repeat(3, minmax(0, 1fr))`), `gap: var(--space-2)` (`4px`): при `N = 5` верхний ряд из 2 крупных ячеек, нижний из 3; при `N > 4` в компактном превью на 4-й ячейке отображается оверлей-бейдж `+{N - 4}` (`data-overflow-extra="{N - 4}"`) либо рендерятся все `N` ячеек с атрибутом `data-mosaic-index` | Общая ширина до `var(--modal-w-md)` (`602px`), `border-radius: var(--radius-sm)` (`8px`), `overflow: hidden` |

### 2.2. Ограничение `max-width` / `max-height` и сохранение `aspect-ratio` по `width` / `height`

1. **Вычисление пропорций (`aspect-ratio`)**:
   - Если у вложения заданы валидные `width > 0` и `height > 0` (`[D]` §2.1, §4.3), на элемент `<figure class="attachment-media-item">` устанавливаются дата-атрибуты `data-width="{width}"`, `data-height="{height}"`, а на внутренний `<img>` / `<video>` — HTML-атрибуты `width="{width}"` и `height="{height}"`.
   - Для одиночного вложения (`data-layout="single"`) вычисляется целевой размер рамки с сохранением пропорций (`ratio = width / height`) в пределах `MAX_W = 442` (`var(--modal-w-sm)`), `MAX_H = 400` (`calc(var(--emoji-picker-h) - var(--space-10))`), `MIN_W = 150`, `MIN_H = 110` (`[D]` §9.3):
     ```js
     function computeSingleMediaBox(width, height, maxW = 442, maxH = 400) {
       if (!width || !height || width <= 0 || height <= 0) {
         return { width: maxW, height: Math.round((maxW * 9) / 16), aspectRatio: "16 / 9" };
       }
       const scale = Math.min(1, maxW / width, maxH / height);
       const displayW = Math.max(150, Math.round(width * scale));
       const displayH = Math.max(110, Math.round(height * scale));
       return { width: displayW, height: displayH, aspectRatio: `${width} / ${height}` };
     }
     ```
   - Вычисленное соотношение `aspect-ratio` резервирует место в потоке до окончания сетевой загрузки, полностью исключая скачки верстки (`CLS = 0`).
2. **Фоллбек при отсутствии размеров**:
   - Если `width` или `height` отсутствуют, равны `0` или отрицательны, контейнер получает дефолтное соотношение сторон `16 / 9` (`data-aspect-fallback="true"`).

### 2.3. Состояние загрузки Skeleton и Blurhash (`placeholder_version === 1`)

- **Начальное состояние (`data-state="loading"`)**:
  - На `<figure class="attachment-media-item">` устанавливаются `data-state="loading"` и `aria-busy="true"`.
  - Внутри рендерится слой `.attachment-skeleton` с фоном `var(--surface-3)` и плавной анимацией прозрачности (`@media (prefers-reduced-motion: reduce)` отключает анимацию).
  - **Правило Blurhash (`[D]` §8.2 `@841846`)**: если `attachment.placeholder_version === 1` и строка `attachment.placeholder` непустая, на `.attachment-skeleton` выставляется `data-placeholder="blurhash"` и `data-placeholder-version="1"`. При любом другом значении `placeholder_version` (например, `2`, `0`, `null`, `undefined`) поле `placeholder` **игнорируется** и остаётся чистый skeleton-фон `var(--surface-3)`.
- **Завершение загрузки (`data-state="loaded"`)**:
  - По событию `load` у `<img>` атрибут `data-state` меняется на `"loaded"`, `aria-busy` снимается (`"false"`), слой `.attachment-skeleton` скрывается (`hidden`).
- **Ошибка загрузки (`data-state="error"`)**:
  - По событию `error` у `<img>` или при `safeHref(url) === null` выставляется `data-state="error"`, `aria-busy="false"`, скрывается битый `<img>` и отображается блок `.attachment-media-error` с иконкой и русским текстом `"Не удалось загрузить изображение."` (`sAfT1K` `[D]` §8.3) цветом `var(--danger-text)`.

### 2.4. Spoiler-оверлей `'СПОЙЛЕР'` и предупреждения о контенте (`obscureReason`)

1. **Определение причины скрытия (`getObscureReason`, `[D]` §7.1 `e0f9541dbaca2408_0@296`)**:
   - Приоритет причин скрытия строго соответствует RE-оракулу:
     1. Флаги чувствительного контента (`(flags & 16) !== 0` → `"explicit_content"`, `(flags & 64) !== 0` → `"gore_content"`, `(flags & 128) !== 0` → `"self_harm_content"`);
     2. Неполный скан для потенциально чувствительного медиа (`content_scan_version === -1` или `< validContentScanVersion` при включённом фильтре → `"potential_explicit_content"`);
     3. Спойлер вложения или канала: `((flags ?? 0) & 8) !== 0` (`IS_SPOILER`), либо `filename.startsWith("SPOILER_")`, либо `channel.isSpoiler === true` → `"spoiler"`;
     4. Иначе — `null` (вложение открыто сразу: `data-spoiler="none"`).
2. **Визуализация и поведение скрытого спойлера (`data-spoiler="hidden"`)**:
   - На `<figure class="attachment-media-item">` устанавливаются `data-spoiler="hidden"` и `data-obscure-reason="{reason}"`.
   - Медиа-контент (`<img>` или `<video>`) размывается через CSS (`filter: blur(var(--space-7)); transform: scale(1.08); pointer-events: none; user-select: none;`) и помечается `aria-hidden="true"`, чтобы до раскрытия скринридер не зачитывал скрытое изображение, а кнопку открытия lightbox нельзя было случайно нажать (`tabindex="-1"`).
   - Поверх медиа монтируется интерактивный оверлей:
     ```html
     <button type="button"
             class="attachment-spoiler-overlay"
             data-action="attachment-spoiler-reveal"
             aria-expanded="false"
             aria-label="Спойлер: изображение скрыто. Нажмите, чтобы показать">
       <span class="attachment-spoiler-pill">СПОЙЛЕР</span>
       <span class="attachment-spoiler-hint">Нажмите, чтобы узнать больше</span>
     </button>
     ```
   - При `reason === "spoiler"` текст пилюли `.attachment-spoiler-pill` равен **`СПОЙЛЕР`** (`XpfDH+` `[D]` §7.1, §7.4). При `reason` из группы `explicit_content | gore_content | self_harm_content | potential_explicit_content` текст пилюли — **`Предупреждение о контенте`** (`SEgHFh` `[D]` §7.1), а подсказка `.attachment-spoiler-hint` — **`Нажмите, чтобы узнать больше`** (`515vjG` `[D]` §6, §7.4).
3. **Раскрытие спойлера по клику / `Enter` / `Space`**:
   - При клике мышью или нажатии клавиш `Enter` / `Space` на `.attachment-spoiler-overlay`:
     - `data-spoiler` на `<figure>` переключается с `"hidden"` на `"revealed"`;
     - на кнопке-оверлее выставляется `aria-expanded="true"` и атрибут `hidden` (оверлей скрывается);
     - с медиа-элемента снимается размытие и `aria-hidden`, а кнопка превью `.attachment-media-trigger` получает `tabindex="0"` и **принимает фокус клавиатуры** (`trigger.focus()`).
     - **Критическое правило двухшагового клика**: клик, раскрывающий спойлер, **не открывает** lightbox и не запускает видео. Lightbox открывается только следующим кликом / `Enter` по уже раскрытому изображению.

### 2.5. DOM-структура изображения и Lightbox-превью (`#attachment-lightbox`)

```html
<!-- Контейнер всех вложений внутри .text[data-stack="true"] -->
<div class="message-attachments" role="group" aria-label="Вложения сообщения">
  <!-- Сетка медиа-вложений (IMAGE / VIDEO) -->
  <div class="attachments-mosaic" data-mosaic-count="1" data-layout="single">
    <figure class="attachment-media-item"
            data-attachment-id="1561379697533911060"
            data-media-type="IMAGE"
            data-state="loaded"
            data-spoiler="hidden"
            data-obscure-reason="spoiler"
            data-animated="false"
            data-width="1280"
            data-height="720">
      <!-- Кнопка открытия в Lightbox (активна после раскрытия спойлера) -->
      <button type="button"
              class="attachment-media-trigger"
              data-action="attachment-lightbox-open"
              data-attachment-id="1561379697533911060"
              tabindex="-1"
              aria-hidden="true"
              aria-haspopup="dialog"
              aria-label="Открыть изображение «screenshot.png» в полном размере">
        <img class="attachment-image"
             src="https://media.discordapp.net/attachments/1058/1561/screenshot.png?format=webp"
             alt="Скриншот окна настроек"
             width="1280"
             height="720"
             loading="lazy">
      </button>

      <!-- Skeleton-подложка (видна при data-state="loading") -->
      <div class="attachment-skeleton" data-placeholder="blurhash" data-placeholder-version="1" hidden aria-hidden="true"></div>

      <!-- Блок ошибки загрузки (виден при data-state="error") -->
      <div class="attachment-media-error" role="alert" hidden>
        <span class="attachment-error-icon" aria-hidden="true">⚠</span>
        <span class="attachment-error-text">Не удалось загрузить изображение.</span>
      </div>

      <!-- Бейдж GIF (виден при data-animated="true" и раскрытом спойлере) -->
      <span class="attachment-gif-badge" hidden aria-hidden="true">GIF</span>

      <!-- Бейдж ALT (рендерится только при наличии непустого attachment.description [D] §7.4) -->
      <button type="button"
              class="attachment-alt-badge"
              data-action="attachment-alt-toggle"
              aria-expanded="false"
              aria-label="Показать описание изображения (ALT)"
              title="ПОДСКАЗКА: Скриншот окна настроек"
              hidden>ALT</button>

      <!-- Оверлей спойлера (виден при data-spoiler="hidden") -->
      <button type="button"
              class="attachment-spoiler-overlay"
              data-action="attachment-spoiler-reveal"
              aria-expanded="false"
              aria-label="Спойлер: изображение скрыто. Нажмите, чтобы показать">
        <span class="attachment-spoiler-pill">СПОЙЛЕР</span>
        <span class="attachment-spoiler-hint">Нажмите, чтобы узнать больше</span>
      </button>
    </figure>
  </div>
</div>
```

#### Модальное окно Lightbox (`#attachment-lightbox`)

При активации `.attachment-media-trigger` (`data-action="attachment-lightbox-open"`) открывается полноэкранный диалог `#attachment-lightbox`:

```html
<div id="attachment-lightbox" class="dialog-backdrop attachment-lightbox" role="presentation" hidden>
  <div class="dialog attachment-lightbox-dialog"
       role="dialog"
       aria-modal="true"
       aria-labelledby="attachment-lightbox-title"
       aria-describedby="attachment-lightbox-meta">
    <header class="attachment-lightbox-header">
      <div class="attachment-lightbox-info">
        <span id="attachment-lightbox-title" class="attachment-lightbox-filename">screenshot.png</span>
        <span id="attachment-lightbox-meta" class="attachment-lightbox-details">1280 × 720 · 420 КБ</span>
      </div>
      <div class="attachment-lightbox-actions">
        <a id="attachment-lightbox-original"
           class="btn attachment-lightbox-link"
           href="https://cdn.discordapp.com/attachments/1058/1561/screenshot.png"
           target="_blank"
           rel="noreferrer noopener"
           aria-label="Открыть оригинал «screenshot.png» в браузере">Открыть оригинал</a>
        <button type="button"
                id="attachment-lightbox-close"
                class="icon-btn attachment-lightbox-close"
                data-action="attachment-lightbox-close"
                aria-label="Закрыть просмотр изображения">
          <span aria-hidden="true">✕</span>
        </button>
      </div>
    </header>

    <div class="attachment-lightbox-stage">
      <button type="button"
              id="attachment-lightbox-prev"
              class="icon-btn attachment-lightbox-nav"
              data-action="attachment-lightbox-prev"
              aria-label="Предыдущее изображение"
              hidden>
        <span aria-hidden="true">◀</span>
      </button>
      <img id="attachment-lightbox-img"
           class="attachment-lightbox-img"
           src="https://cdn.discordapp.com/attachments/1058/1561/screenshot.png"
           alt="Скриншот окна настроек">
      <button type="button"
              id="attachment-lightbox-next"
              class="icon-btn attachment-lightbox-nav"
              data-action="attachment-lightbox-next"
              aria-label="Следующее изображение"
              hidden>
        <span aria-hidden="true">▶</span>
      </button>
    </div>
  </div>
</div>
```

---

## 3. Видео-вложения (`VIDEO`): обложка/постер, кнопка воспроизведения, длительность, элементы управления

### 3.1. Правила постера и ограничений (`ATTACH-RENDER-RE.md` §6, §8.1, §9.3)

1. **URL постера (обложки)**:
   - Строится из `attachment.proxy_url` путём добавления query-параметра `format=webp` (`urlObj.searchParams.set("format", "webp")`, `[D] 5c14801@696`).
   - Если `proxy_url` отсутствует, постер не запрашивается по сети, а подложкой выступает поверхность `var(--surface-3)` (либо blurhash при `placeholder_version === 1`).
2. **Минимальные и максимальные габариты плеера**:
   - Минимальная ширина `min-width: 150px`, минимальная высота `min-height: 110px` (`[D]` §9.3 `static minWidth=150, static minHeight=110`).
   - Максимальная ширина одиночного видео `var(--modal-w-sm)` (`442px`), максимальная высота `calc(var(--emoji-picker-h) - var(--space-10))` (`400px`) с сохранением `aspect-ratio: width / height`.
3. **Защита от сверхбольших разрешений (`forceExternal`, `[D] @5929320`)**:
   - Если `width` и `height` заданы и превышают `6016 × 3384` в любой ориентации (`(width > 6016 && height > 3384) || (width > 3384 && height > 6016)`), на контейнере выставляется `data-force-external="true"`. Встроенный `<video>` не создаётся; вместо него отображается постер с внешней ссылкой «Открыть видео во внешнем плеере» (`href={safeUrl}`).
4. **Формат длительности (`formatDuration`, `[D] @5935271`)**:
   - Длительность `duration_secs` (число с плавающей точкой) приводится к целым секундам побитовым усечением `const totalSec = 0 | durationSecs;`:
     ```js
     export function formatDuration(durationSecs) {
       if (typeof durationSecs !== "number" || !Number.isFinite(durationSecs) || durationSecs < 0) {
         return "--:--";
       }
       const t = 0 | durationSecs;
       const s = t % 60;
       const m = (t - s) / 60;
       return `${m}:${String(s).padStart(2, "0")}`;
     }
     ```
   - Примеры по RE-оракулу: `36.0955924987793` → `"0:36"`, `8.119976` → `"0:08"`, `125.9` → `"2:05"`, `null` / `undefined` / `NaN` → `"--:--"`.

### 3.2. DOM-структура видео-вложения (`.attachment-video-player`)

```html
<figure class="attachment-media-item attachment-video-item"
        data-attachment-id="1560312729041113179"
        data-media-type="VIDEO"
        data-player-state="idle"
        data-spoiler="none"
        data-muted="false"
        data-width="720"
        data-height="1280">
  <div class="attachment-video-stage">
    <video class="attachment-video"
           src="https://cdn.discordapp.com/attachments/1058/1560/vkclips_20261015062453.mp4"
           poster="https://media.discordapp.net/attachments/1058/1560/vkclips_20261015062453.mp4?format=webp"
           preload="metadata"
           playsinline
           width="720"
           height="1280"
           aria-label="Видео «vkclips_20261015062453.mp4»"></video>

    <!-- Бейдж длительности/клипа поверх постера (скрывается при воспроизведении) -->
    <div class="attachment-video-badges">
      <span class="attachment-video-title" hidden></span>
      <span class="attachment-video-duration" aria-label="Длительность 0:36">0:36</span>
    </div>

    <!-- Центральная кнопка воспроизведения поверх постера -->
    <button type="button"
            class="attachment-video-play-overlay"
            data-action="video-toggle-play"
            aria-label="Воспроизвести видео «vkclips_20261015062453.mp4»">
      <span class="attachment-play-glyph" aria-hidden="true">▶</span>
    </button>
  </div>

  <!-- Нижняя панель управления видео -->
  <div class="attachment-video-controls" role="group" aria-label="Управление воспроизведением видео">
    <button type="button"
            class="icon-btn attachment-ctrl-btn"
            data-action="video-toggle-play"
            aria-pressed="false"
            aria-label="Воспроизвести">
      <span aria-hidden="true">▶</span>
    </button>

    <span class="attachment-ctrl-time" aria-live="off">
      <span class="attachment-time-current">0:00</span>
      <span aria-hidden="true"> / </span>
      <span class="attachment-time-total">0:36</span>
    </span>

    <input type="range"
           class="attachment-ctrl-seek"
           role="slider"
           data-action="video-seek"
           min="0"
           max="36"
           step="1"
           value="0"
           aria-label="Перемотка видео «vkclips_20261015062453.mp4»"
           aria-valuemin="0"
           aria-valuemax="36"
           aria-valuenow="0"
           aria-valuetext="0:00 из 0:36">

    <button type="button"
            class="icon-btn attachment-ctrl-btn"
            data-action="video-toggle-mute"
            aria-pressed="false"
            aria-label="Выключить звук">
      <span aria-hidden="true">🔊</span>
    </button>

    <input type="range"
           class="attachment-ctrl-volume"
           role="slider"
           data-action="video-volume"
           min="0"
           max="100"
           step="1"
           value="100"
           aria-label="Громкость видео"
           aria-valuemin="0"
           aria-valuemax="100"
           aria-valuenow="100"
           aria-valuetext="100%">

    <button type="button"
            class="icon-btn attachment-ctrl-btn"
            data-action="video-fullscreen"
            aria-label="На весь экран">
      <span aria-hidden="true">⛶</span>
    </button>
  </div>
</figure>
```

### 3.3. Состояния видео-плеера (`data-player-state`)

| `data-player-state` | Поведение и видимость контролов |
|---|---|
| `"idle"` | Видео на паузе в позиции `0:00`, виден `poster`, центральная кнопка `.attachment-video-play-overlay` и бейдж `.attachment-video-duration` видимы |
| `"playing"` | Воспроизведение активно; `.attachment-video-play-overlay` скрыта (`hidden`); кнопка Play/Pause в панели имеет `aria-pressed="true"`, `aria-label="Пауза"`, глиф `⏸`; `.attachment-time-current` и `aria-valuenow` слайдера синхронно обновляются |
| `"paused"` | Воспроизведение приостановлено пользователем; центральная кнопка `.attachment-video-play-overlay` снова видна; кнопка в панели имеет `aria-pressed="false"`, `aria-label="Воспроизвести"`, глиф `▶` |
| `"ended"` | Достигнут конец ролика; состояние переходит в `"ended"`, иконка центральной кнопки меняется на `↺` (`aria-label="Повторить воспроизведение"`) |
| `"error"` | Сетевой сбой или неподдерживаемый кодек (`mediaError`); контролы блокируются (`aria-disabled="true"`), выводится плашка с `role="alert"` и текстом `"Не удалось воспроизвести видео."` цветом `var(--danger-text)` |

---

## 4. Аудио-вложения и голосовые сообщения (`AUDIO` vs `VOICE_MESSAGE`)

Согласно `ATTACH-RENDER-RE.md` (§5 `renderAudioComponent: f ? dJ.SX : dJ.Nj`, §9.1, §9.2), выбор между обычным аудио-плеером и голосовым сообщением определяется флагом **сообщения** `IS_VOICE_MESSAGE = 8192` (`(message.flags & 8192) !== 0`).

### 4.1. Обычное аудио-вложение (`.attachment-audio-card`, `message.flags & 8192 === 0`)

1. **Метаданные (`renderMetadata`, `[D] @5954877`)**:
   - В верхней части карточки выводятся иконка аудио, имя файла `.attachment-audio-name` и форматированный размер `.attachment-audio-size`.
   - По RE-контракту `hideDownloadButton: true` (`[D]` §6, §9.2), кнопка скачивания в метаданных аудио-плеера скрыта (`hidden`), чтобы не дублировать системное действие.
2. **Элементы управления**:
   - Кнопка Play/Pause (`data-action="audio-toggle-play"`, `aria-pressed="false|true"`).
   - Линейный прогресс-бар перемотки `.attachment-audio-seek` (`input[type="range"][role="slider"]`, `min="0"`, `max="{durationSecs}"`).
   - Таймер текущего времени и общей длительности `{current} / {duration}` (`0:00 / 3:14` или `0:00 / --:--` до загрузки метаданных).
   - Кнопка Mute/Unmute (`autoMute: false` `[D]` §6 — звук по умолчанию включён).

```html
<div class="attachment-audio-card"
     role="group"
     aria-label="Аудиофайл «track.mp3»"
     data-attachment-id="1560312729041113200"
     data-media-type="AUDIO"
     data-player-state="idle"
     data-spoiler="none">
  <div class="attachment-audio-metadata">
    <span class="attachment-audio-icon" aria-hidden="true">🎵</span>
    <span class="attachment-audio-name" title="track.mp3">track.mp3</span>
    <span class="attachment-audio-size">4,2 МБ</span>
  </div>

  <div class="attachment-audio-controls">
    <button type="button"
            class="icon-btn attachment-audio-play"
            data-action="audio-toggle-play"
            aria-pressed="false"
            aria-label="Воспроизвести «track.mp3»">
      <span aria-hidden="true">▶</span>
    </button>

    <span class="attachment-ctrl-time">
      <span class="attachment-time-current">0:00</span>
      <span aria-hidden="true"> / </span>
      <span class="attachment-time-total">2:45</span>
    </span>

    <input type="range"
           class="attachment-ctrl-seek attachment-audio-seek"
           role="slider"
           data-action="audio-seek"
           min="0"
           max="165"
           step="1"
           value="0"
           aria-label="Перемотка аудио «track.mp3»"
           aria-valuemin="0"
           aria-valuemax="165"
           aria-valuenow="0"
           aria-valuetext="0:00 из 2:45">

    <button type="button"
            class="icon-btn attachment-ctrl-btn"
            data-action="audio-toggle-mute"
            aria-pressed="false"
            aria-label="Выключить звук">
      <span aria-hidden="true">🔊</span>
    </button>
  </div>
</div>
```

### 4.2. Голосовое сообщение (`.attachment-voice-card`, `message.flags & 8192 !== 0`)

1. **Декодирование и ресемплинг `waveform` (`[D]` §9.1 `@6133343`)**:
   - Поле `attachment.waveform` представляет собой base64-строку, где каждый декодированный байт `0..255` кодирует амплитуду `byte / 255` в диапазоне `0..1`:
     ```js
     export function decodeWaveform(base64Str, targetWidthPx = 192) {
       const barCount = Math.max(1, Math.floor((targetWidthPx + 4) / 6));
       if (typeof base64Str !== "string" || base64Str.length === 0) {
         return new Array(barCount).fill(0);
       }
       let rawBytes;
       try {
         const binary = atob(base64Str);
         rawBytes = Array.from(binary, (ch) => ch.charCodeAt(0) / 255);
       } catch {
         return new Array(barCount).fill(0);
       }
       if (rawBytes.length === 0) return new Array(barCount).fill(0);
       const bars = new Array(barCount).fill(0);
       if (rawBytes.length <= barCount) {
         for (let i = 0; i < rawBytes.length; i++) bars[i] = rawBytes[i];
         return bars;
       }
       // Лишние сэмплы усредняются по корзинам [D] §9.1
       const bucketSize = rawBytes.length / barCount;
       for (let i = 0; i < barCount; i++) {
         const start = Math.floor(i * bucketSize);
         const end = Math.max(start + 1, Math.floor((i + 1) * bucketSize));
         let sum = 0;
         for (let j = start; j < end && j < rawBytes.length; j++) sum += rawBytes[j];
         bars[i] = sum / (end - start);
       }
       return bars;
     }
     ```
   - При ширине контейнера волны `192px` формула `Math.floor((192 + 4) / 6)` даёт **32 столбика** (ширина столбика `var(--space-2)` = `4px`, зазор `var(--space-1)` = `2px`, шаг `6px`).
   - Высота каждого столбика `.attachment-waveform-bar` квантуется на 8 дискретных уровней (`data-level="1".."8"`), где уровень `1` соответствует минимальной высоте `var(--space-1)` (`2px`), а уровень `8` — полной высоте `var(--layout-group-title-h)` (`24px`). Столбики с индексом `i / barCount < progressRatio` получают атрибут `data-played="true"` (заливка `var(--accent)`), остальные — `data-played="false"` (заливка `var(--text-muted)`).
2. **DOM-структура голосового сообщения**:

```html
<div class="attachment-voice-card"
     role="group"
     aria-label="Голосовое сообщение, длительность 0:08"
     data-attachment-id="1359580913469751459"
     data-media-type="VOICE_MESSAGE"
     data-player-state="idle">
  <button type="button"
          class="icon-btn attachment-voice-play"
          data-action="voice-msg-toggle-play"
          aria-pressed="false"
          aria-label="Воспроизвести голосовое сообщение">
    <span aria-hidden="true">▶</span>
  </button>

  <div class="attachment-waveform-wrap">
    <!-- Визуальные столбики амплитуд (скрыты от скринридера, управление через range-слайдер) -->
    <div class="attachment-waveform-bars" aria-hidden="true" data-bar-count="32">
      <span class="attachment-waveform-bar" data-level="2" data-played="false"></span>
      <span class="attachment-waveform-bar" data-level="5" data-played="false"></span>
      <span class="attachment-waveform-bar" data-level="8" data-played="false"></span>
      <!-- ... всего barCount столбиков ... -->
    </div>
    <input type="range"
           class="attachment-waveform-slider"
           role="slider"
           data-action="voice-msg-seek"
           min="0"
           max="8"
           step="1"
           value="0"
           aria-label="Перемотка голосового сообщения"
           aria-valuemin="0"
           aria-valuemax="8"
           aria-valuenow="0"
           aria-valuetext="0:00 из 0:08">
  </div>

  <span class="attachment-voice-duration" aria-label="Длительность 0:08">0:08</span>

  <button type="button"
          class="attachment-voice-speed"
          data-action="voice-msg-speed"
          data-speed="1"
          aria-label="Скорость воспроизведения: 1x">1x</button>
</div>
```

---

## 5. Файловые вложения (`FILE`): классификация `klass`, форматирование размера и карточка файла

### 5.1. Таблица классификации типов файлов `classifyFile(filename, contentType)` (`TX`, `[D] @9704193`)

Функция `classifyFile` проверяет правила строго сверху вниз (первое совпадение побеждает). Сравнение `reType` выполняется по `content_type`, а `reName` — по `filename.toLowerCase()`:

| Порядок | Условие (`reType` / `reName`) | Класс (`klass`) | Символ иконки (`aria-hidden="true"`) | Русская метка типа (для `aria-label`) | Метка |
|---:|---|---|---|---|---|
| 1 | `reType: /^image\/vnd\.adobe\.photoshop/i` | `"photoshop"` | `🎨` | Файл Photoshop | `[D]` |
| 2 | `reType: /^image\/svg\+xml/i` | `"webcode"` | `🌐` | Векторная/веб-разметка | `[D]` |
| 3 | `reType: /^image\//i` | `"image"` | `🖼` | Изображение | `[D]` |
| 4 | `reType: /^video\//i` | `"video"` | `🎬` | Видеофайл | `[D]` |
| 5 | `reName: /\.pdf$/i` | `"acrobat"` | `📕` | Документ PDF | `[D]` |
| 6 | `reName: /\.ae$/i` | `"ae"` | `🎞` | Проект After Effects | `[D]` |
| 7 | `reName: /\.sketch$/i` | `"sketch"` | `💎` | Файл Sketch | `[D]` |
| 8 | `reName: /\.ai$/i` | `"ai"` | `✒` | Файл Adobe Illustrator | `[D]` |
| 9 | `reName: /\.(?:rar|zip|7z|tar|tar\.gz)$/i` | `"archive"` | `🗜` | Архив | `[D]` |
| 10 | `reName: /\.(?:c\+\+|cpp|cc|c|h|hpp|mm|m|json|js|ts|rb|py|java|rs|go|sh|ml|actionscript)$/i` | `"code"` | `💻` | Исходный код | `[D]` |
| 11 | `reName: /\.(?:txt|rtf|doc|docx|md|pages|ppt|pptx|pptm|key|log)$/i` | `"document"` | `📄` | Текстовый документ | `[D]` |
| 12 | `reName: /\.(?:xls|xlsx|numbers|csv)$/i` | `"spreadsheet"` | `📊` | Электронная таблица | `[D]` |
| 13 | `reName: /\.(?:html|xhtml|htm|xml|xsd|css|styl)$/i` | `"webcode"` | `🌐` | Веб-документ | `[D]` |
| 14 | `reName: /\.(?:mp3|ogg|opus|wav|aiff|flac)$/i` | `"audio"` | `🎵` | Аудиофайл | `[D]` |
| 15 | *(фоллбек по умолчанию)* | `"unknown"` | `📎` | Файл | `[D]` |

### 5.2. Форматирование размера файла в Б / КБ / МБ (`formatAttachmentSize`)

В карточке файла и заголовках вложений размер `size` (целое число байт) форматируется на русском языке с использованием единиц **`Б`**, **`КБ`**, **`МБ`**, **`ГБ`** (разделитель дробной части — запятая, между числом и единицей — обычный или неразрывный пробел; дробная часть `.0` отбрасывается):

```js
/**
 * Форматирование размера вложения в Б / КБ / МБ / ГБ (десятичная шкала 1000 / двоичная 1024).
 * По умолчанию используется каноническая шкала 1024 (1 КБ = 1024 Б, 1 МБ = 1048576 Б)
 * с округлением до 1 знака после запятой (для КБ при < 10 КБ — 1 знак, при >= 10 КБ — целое
 * или 1 знак, ",0" отбрасывается).
 * @param {number} bytes
 * @returns {string}
 */
export function formatAttachmentSize(bytes) {
  if (typeof bytes !== "number" || !Number.isFinite(bytes) || bytes <= 0) {
    return "0 Б";
  }
  const n = Math.floor(bytes);
  if (n < 1024) {
    return `${n} Б`;
  }
  if (n < 1024 * 1024) {
    const kb = Math.round((n / 1024) * 10) / 10;
    const text = Number.isInteger(kb) ? String(kb) : String(kb).replace(".", ",");
    return `${text} КБ`;
  }
  if (n < 1024 * 1024 * 1024) {
    const mb = Math.round((n / (1024 * 1024)) * 10) / 10;
    const text = Number.isInteger(mb) ? String(mb) : String(mb).replace(".", ",");
    return `${text} МБ`;
  }
  const gb = Math.round((n / (1024 * 1024 * 1024)) * 10) / 10;
  const text = Number.isInteger(gb) ? String(gb) : String(gb).replace(".", ",");
  return `${text} ГБ`;
}
```

| Вход `size` (байт) | Выход `formatAttachmentSize(size)` | Источник примера |
|---:|---|---|
| `0` | `"0 Б"` | Граничное значение |
| `512` | `"512 Б"` | `< 1024` |
| `1024` | `"1 КБ"` | Ровно 1 КБ (без `,0`) |
| `2563` | `"2,5 КБ"` | Живой объект `vbs.bat` из `ATTACH-RENDER-RE.md` §2.1 |
| `26921` | `"26,3 КБ"` | Живой объект `voice-message.ogg` из `ATTACH-RENDER-RE.md` §2.1 |
| `1048576` | `"1 МБ"` | Ровно 1 МБ (без `,0`) |
| `9256495` | `"8,8 МБ"` | Живой объект `vkclips_20261015062453.mp4` из `ATTACH-RENDER-RE.md` §2.1 |

### 5.3. DOM-структура файлового вложения (`.attachment-file-card`)

```html
<div class="attachment-file-card"
     role="group"
     aria-label="Файл «vbs.bat», 2,5 КБ"
     data-attachment-id="1434205937689952266"
     data-media-type="FILE"
     data-file-klass="unknown"
     data-spoiler="none">
  <div class="attachment-file-icon" data-file-klass="unknown" aria-hidden="true">📎</div>

  <div class="attachment-file-meta">
    <a class="attachment-file-name"
       href="https://cdn.discordapp.com/attachments/1058/1434/vbs.bat"
       target="_blank"
       rel="noreferrer noopener"
       data-action="attachment-open"
       title="vbs.bat">vbs.bat</a>
    <span class="attachment-file-size">2,5 КБ</span>
  </div>

  <a class="icon-btn attachment-file-download"
     href="https://cdn.discordapp.com/attachments/1058/1434/vbs.bat"
     download="vbs.bat"
     target="_blank"
     rel="noreferrer noopener"
     data-action="attachment-download"
     aria-label="Скачать файл «vbs.bat» (2,5 КБ)"
     title="Скачать">
    <span aria-hidden="true">⬇</span>
  </a>

  <!-- Оверлей спойлера для файла (виден при data-spoiler="hidden", ключ 3Gc2XP [D] §7.4) -->
  <button type="button"
          class="attachment-spoiler-overlay"
          data-action="attachment-spoiler-reveal"
          aria-expanded="false"
          aria-label="Файл скрыт под спойлером. Нажмите, чтобы показать"
          hidden>
    <span class="attachment-spoiler-pill">СПОЙЛЕР</span>
    <span class="attachment-spoiler-hint">Файл скрыт под спойлером</span>
  </button>
</div>
```

> [!NOTE]
> **Спойлер на карточке файла (`data-spoiler="hidden"`):**
> Когда файл помечен спойлером (`(flags & 8) !== 0` или `filename.startsWith("SPOILER_")`), имя файла и размер размываются (`filter: blur(var(--space-4))`), ссылки `.attachment-file-name` и `.attachment-file-download` получают `tabindex="-1"` и `aria-hidden="true"`, а поверх карточки отображается кнопка `.attachment-spoiler-overlay` с подписью `Файл скрыт под спойлером` (`3Gc2XP` `[D]` §7.4). Клик или `Enter`/`Space` раскрывает карточку (`data-spoiler="revealed"`), возвращает ссылкам `tabindex="0"` и переводит фокус на `.attachment-file-name`.

---

## 6. CSS-спецификация (только токены `rust/gui-spike/ui/tokens.css`)

Все правила ниже размещаются в `rust/gui-spike/ui/components/chat.css` (секция рендера вложений AT-03). Никаких сырых цветов и никаких новых переменных нет.

```css
/* =============================================================================
 * AT-03: Рендер вложений в ленте сообщений (IMAGE / VIDEO / AUDIO / VOICE / FILE)
 * Все значения строго привязаны к существующим токенам rust/gui-spike/ui/tokens.css.
 * ========================================================================== */

.message-attachments {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-top: var(--space-2);
  max-width: 100%;
}

/* -----------------------------------------------------------------------------
 * 1. Сетка Mosaic и одиночные медиа-контейнеры
 * -------------------------------------------------------------------------- */
.attachments-mosaic {
  display: grid;
  gap: var(--space-2);
  width: 100%;
  max-width: var(--modal-w-md);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.attachments-mosaic[data-layout="single"] {
  grid-template-columns: minmax(0, 1fr);
  max-width: var(--modal-w-sm);
}

.attachments-mosaic[data-layout="mosaic-2"],
.attachments-mosaic[data-layout="mosaic-3"],
.attachments-mosaic[data-layout="mosaic-4"] {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.attachments-mosaic[data-layout="mosaic-3"] .attachment-media-item:first-child {
  grid-row: 1 / span 2;
}

.attachments-mosaic[data-layout="mosaic-many"] {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.attachment-media-item {
  position: relative;
  margin: var(--space-0);
  min-width: calc(var(--layout-tree-min-w) - var(--space-11));
  min-height: calc(var(--layout-composer-h) + var(--space-12) - var(--space-1));
  max-height: calc(var(--emoji-picker-h) - var(--space-10));
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  overflow: hidden;
}

/* В многоячеечной сетке карточки заполняют свою ячейку */
.attachments-mosaic:not([data-layout="single"]) .attachment-media-item {
  min-width: var(--space-0);
  aspect-ratio: 4 / 3;
}

.attachment-media-trigger {
  display: block;
  width: 100%;
  height: 100%;
  padding: var(--space-0);
  border: var(--space-0);
  background: transparent;
  cursor: zoom-in;
}

.attachment-image,
.attachment-video {
  display: block;
  width: 100%;
  height: 100%;
  max-height: calc(var(--emoji-picker-h) - var(--space-10));
  object-fit: cover;
}

.attachments-mosaic[data-layout="single"] .attachment-image,
.attachments-mosaic[data-layout="single"] .attachment-video {
  object-fit: contain;
  background: var(--surface-sunken);
}

/* -----------------------------------------------------------------------------
 * 2. Skeleton-загрузка, ошибки и бейджи (GIF / ALT / +N)
 * -------------------------------------------------------------------------- */
.attachment-skeleton {
  position: absolute;
  inset: var(--space-0);
  background: var(--surface-3);
  transition: opacity var(--motion-base) var(--ease-in-out);
}

.attachment-skeleton[hidden],
.attachment-media-error[hidden],
.attachment-spoiler-overlay[hidden],
.attachment-alt-badge[hidden],
.attachment-gif-badge[hidden],
.attachment-video-play-overlay[hidden] {
  display: none;
}

.attachment-media-error {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-6);
  min-height: calc(var(--layout-composer-h) * 2);
  background: var(--surface-2);
  color: var(--danger-text);
  font-size: var(--font-size-ui);
  line-height: var(--line-height-ui);
}

.attachment-gif-badge,
.attachment-alt-badge,
.attachment-mosaic-overflow {
  position: absolute;
  z-index: var(--z-header);
  padding: var(--space-1) var(--space-3);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--radius-xs);
  background: var(--surface-4);
  color: var(--text-strong);
  font-family: var(--font-sans);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-caption);
  letter-spacing: var(--letter-spacing-caps);
  text-transform: uppercase;
}

.attachment-gif-badge {
  top: var(--space-4);
  left: var(--space-4);
  pointer-events: none;
}

.attachment-alt-badge {
  bottom: var(--space-4);
  left: var(--space-4);
  cursor: pointer;
}

.attachment-alt-badge:hover {
  background: var(--surface-3);
  border-color: var(--border-strong);
}

/* -----------------------------------------------------------------------------
 * 3. Spoiler-оверлей ('СПОЙЛЕР' / предупреждение о контенте)
 * -------------------------------------------------------------------------- */
.attachment-media-item[data-spoiler="hidden"] .attachment-image,
.attachment-media-item[data-spoiler="hidden"] .attachment-video,
.attachment-media-item[data-spoiler="hidden"] .attachment-video-controls,
.attachment-media-item[data-spoiler="hidden"] .attachment-video-badges {
  filter: blur(var(--space-7));
  transform: scale(1.08);
  pointer-events: none;
  user-select: none;
}

.attachment-file-card[data-spoiler="hidden"] .attachment-file-meta,
.attachment-file-card[data-spoiler="hidden"] .attachment-file-icon,
.attachment-file-card[data-spoiler="hidden"] .attachment-file-download {
  filter: blur(var(--space-4));
  pointer-events: none;
  user-select: none;
}

.attachment-spoiler-overlay {
  position: absolute;
  inset: var(--space-0);
  z-index: var(--z-header);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-4);
  border: var(--space-0);
  background: var(--surface-3);
  color: var(--text-strong);
  font-family: var(--font-sans);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.attachment-spoiler-overlay:hover {
  background: var(--surface-4);
}

.attachment-spoiler-pill {
  padding: var(--space-2) var(--space-5);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--radius-round);
  background: var(--surface-1);
  color: var(--text-strong);
  font-size: var(--font-size-small);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-small);
  letter-spacing: var(--letter-spacing-caps);
  text-transform: uppercase;
  box-shadow: var(--shadow-elevation-medium);
}

.attachment-spoiler-hint {
  color: var(--text-subtle);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
}

/* -----------------------------------------------------------------------------
 * 4. Видео-вложения: постер, центральная кнопка Play, длительность, контролы
 * -------------------------------------------------------------------------- */
.attachment-video-stage {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  background: var(--surface-sunken);
}

.attachment-video-badges {
  position: absolute;
  top: var(--space-4);
  right: var(--space-4);
  display: flex;
  gap: var(--space-2);
  pointer-events: none;
}

.attachment-video-duration,
.attachment-video-title {
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-xs);
  background: var(--surface-1);
  color: var(--text-strong);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-caption);
  font-variant-numeric: tabular-nums;
}

.attachment-video-play-overlay {
  position: absolute;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--space-11);
  height: var(--space-11);
  border: var(--space-0);
  border-radius: var(--radius-round);
  background: var(--accent);
  color: var(--text-on-accent);
  font-size: var(--font-size-title);
  line-height: 1;
  box-shadow: var(--shadow-elevation-high);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.attachment-video-play-overlay:hover {
  background: var(--accent-hover);
}

.attachment-video-play-overlay:active {
  background: var(--accent-active);
}

.attachment-video-controls {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: var(--btn-height-lg);
  padding: var(--space-2) var(--space-4);
  border-top: var(--menu-border) solid var(--border-subtle);
  background: var(--surface-1);
  color: var(--text);
}

.attachment-ctrl-time {
  flex: 0 0 auto;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.attachment-ctrl-seek {
  flex: 1 1 auto;
  min-width: var(--space-13);
  height: var(--slider-track-h);
  accent-color: var(--accent);
  cursor: pointer;
}

.attachment-ctrl-volume {
  width: var(--space-13);
  height: var(--slider-track-h);
  accent-color: var(--accent);
  cursor: pointer;
}

/* -----------------------------------------------------------------------------
 * 5. Аудио-карточка (.attachment-audio-card) и Голосовое сообщение (.attachment-voice-card)
 * -------------------------------------------------------------------------- */
.attachment-audio-card,
.attachment-voice-card {
  position: relative;
  display: flex;
  width: 100%;
  max-width: var(--modal-w-sm);
  padding: var(--space-4);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  color: var(--text);
  box-sizing: border-box;
}

.attachment-audio-card {
  flex-direction: column;
  gap: var(--space-3);
}

.attachment-audio-metadata {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: var(--space-0);
}

.attachment-audio-icon {
  flex: 0 0 auto;
  font-size: var(--icon-size);
  line-height: 1;
}

.attachment-audio-name {
  flex: 1 1 auto;
  overflow: hidden;
  color: var(--text-strong);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attachment-audio-size {
  flex: 0 0 auto;
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-variant-numeric: tabular-nums;
}

.attachment-audio-controls {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.attachment-audio-play,
.attachment-voice-play {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--btn-height);
  height: var(--btn-height);
  border: var(--space-0);
  border-radius: var(--radius-round);
  background: var(--accent);
  color: var(--text-on-accent);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.attachment-voice-play {
  width: var(--avatar-lg);
  height: var(--avatar-lg);
}

.attachment-audio-play:hover,
.attachment-voice-play:hover {
  background: var(--accent-hover);
}

.attachment-voice-card {
  flex-direction: row;
  align-items: center;
  gap: var(--space-4);
}

.attachment-waveform-wrap {
  position: relative;
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  height: var(--layout-group-title-h);
  min-width: var(--space-13);
}

.attachment-waveform-bars {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.attachment-waveform-bar {
  flex: 0 0 var(--space-2);
  width: var(--space-2);
  min-height: var(--space-1);
  border-radius: var(--radius-round);
  background: var(--text-muted);
  transition: background var(--motion-fast) var(--ease-out);
}

.attachment-waveform-bar[data-played="true"] {
  background: var(--accent);
}

.attachment-waveform-bar[data-level="1"] { height: var(--space-1); }
.attachment-waveform-bar[data-level="2"] { height: var(--space-2); }
.attachment-waveform-bar[data-level="3"] { height: var(--space-3); }
.attachment-waveform-bar[data-level="4"] { height: var(--space-4); }
.attachment-waveform-bar[data-level="5"] { height: var(--space-5); }
.attachment-waveform-bar[data-level="6"] { height: var(--space-6); }
.attachment-waveform-bar[data-level="7"] { height: var(--space-7); }
.attachment-waveform-bar[data-level="8"] { height: var(--layout-group-title-h); }

.attachment-waveform-slider {
  position: absolute;
  inset: var(--space-0);
  width: 100%;
  height: 100%;
  margin: var(--space-0);
  opacity: 0;
  cursor: pointer;
}

.attachment-voice-duration {
  flex: 0 0 auto;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-variant-numeric: tabular-nums;
}

.attachment-voice-speed {
  flex: 0 0 auto;
  min-height: var(--btn-height-sm);
  padding: var(--space-0) var(--space-3);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--radius-xs);
  background: var(--surface-3);
  color: var(--text-strong);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  cursor: pointer;
}

.attachment-voice-speed:hover {
  background: var(--surface-4);
}

/* -----------------------------------------------------------------------------
 * 6. Файловые вложения (.attachment-file-card)
 * -------------------------------------------------------------------------- */
.attachment-file-card {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-4);
  width: 100%;
  max-width: var(--modal-w-sm);
  min-height: var(--layout-composer-h);
  padding: var(--space-4);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--surface-2);
  color: var(--text);
  box-sizing: border-box;
  overflow: hidden;
}

.attachment-file-icon {
  flex: 0 0 var(--avatar-lg);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--avatar-lg);
  height: var(--avatar-lg);
  border-radius: var(--radius-xs);
  background: var(--surface-3);
  color: var(--text-accent);
  font-size: var(--icon-size-lg);
  line-height: 1;
}

.attachment-file-meta {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: var(--space-0);
}

.attachment-file-name {
  overflow: hidden;
  color: var(--text-link);
  font-size: var(--font-size-ui);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-ui);
  text-decoration: none;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attachment-file-name:hover:not([aria-disabled="true"]) {
  text-decoration: underline;
}

.attachment-file-name[aria-disabled="true"] {
  color: var(--text-muted);
  opacity: var(--state-disabled-opacity);
  cursor: not-allowed;
}

.attachment-file-size {
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-variant-numeric: tabular-nums;
}

.attachment-file-download {
  flex: 0 0 var(--btn-height);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--btn-height);
  height: var(--btn-height);
  border: var(--menu-border) solid var(--border-subtle);
  border-radius: var(--btn-radius);
  background: var(--surface-3);
  color: var(--text-subtle);
  text-decoration: none;
  cursor: pointer;
  transition:
    background var(--motion-fast) var(--ease-out),
    color var(--motion-fast) var(--ease-out);
}

.attachment-file-download:hover:not([aria-disabled="true"]) {
  background: var(--surface-4);
  color: var(--text-strong);
}

.attachment-file-download[aria-disabled="true"] {
  opacity: var(--state-disabled-opacity);
  pointer-events: none;
}

/* -----------------------------------------------------------------------------
 * 7. Lightbox-модалка (#attachment-lightbox)
 * -------------------------------------------------------------------------- */
.attachment-lightbox-dialog {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  width: 100%;
  max-width: var(--modal-w-lg);
  padding: var(--modal-pad-y) var(--modal-pad-x);
  border: var(--menu-border) solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-2);
  box-shadow: var(--shadow-popover);
}

.attachment-lightbox-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.attachment-lightbox-info {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: var(--space-0);
}

.attachment-lightbox-filename {
  overflow: hidden;
  color: var(--text-strong);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-body);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attachment-lightbox-details {
  color: var(--text-muted);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
}

.attachment-lightbox-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.attachment-lightbox-stage {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  min-height: calc(var(--layout-tree-min-w) + var(--space-10));
  padding: var(--space-4);
  border-radius: var(--radius-sm);
  background: var(--bg-app);
}

.attachment-lightbox-img {
  display: block;
  max-width: 100%;
  max-height: calc(var(--emoji-picker-h) + var(--space-13));
  border-radius: var(--radius-xs);
  object-fit: contain;
}

/* -----------------------------------------------------------------------------
 * 8. Клавиатурный фокус (WCAG 2.2 AA non-text >= 3:1) и Reduced Motion
 * -------------------------------------------------------------------------- */
.attachment-media-trigger:focus-visible,
.attachment-spoiler-overlay:focus-visible,
.attachment-alt-badge:focus-visible,
.attachment-video-play-overlay:focus-visible,
.attachment-ctrl-seek:focus-visible,
.attachment-ctrl-volume:focus-visible,
.attachment-audio-play:focus-visible,
.attachment-voice-play:focus-visible,
.attachment-voice-speed:focus-visible,
.attachment-file-name:focus-visible,
.attachment-file-download:focus-visible,
.attachment-waveform-wrap:focus-within {
  outline: var(--focus-ring-width) solid var(--focus-ring);
  outline-offset: var(--focus-ring-offset);
}

@media (prefers-reduced-motion: reduce) {
  .attachment-skeleton,
  .attachment-spoiler-overlay,
  .attachment-video-play-overlay,
  .attachment-audio-play,
  .attachment-voice-play,
  .attachment-waveform-bar,
  .attachment-file-download {
    transition: none;
  }
}
```

### 6.1. Реестр использованных CSS-токенов (сверка с `rust/gui-spike/ui/tokens.css`)

Каждый токен в таблице ниже проверен по `rust/gui-spike/ui/tokens.css`:

| Категория | Токены из `rust/gui-spike/ui/tokens.css` |
|---|---|
| **Поверхности (§1)** | `var(--bg-app)`, `var(--surface-1)`, `var(--surface-2)`, `var(--surface-3)`, `var(--surface-4)`, `var(--surface-sunken)` |
| **Текст (§2)** | `var(--text-strong)`, `var(--text)`, `var(--text-subtle)`, `var(--text-muted)`, `var(--text-link)`, `var(--text-accent)`, `var(--text-on-accent)` |
| **Акцент и статусы (§3)** | `var(--accent)`, `var(--accent-hover)`, `var(--accent-active)`, `var(--danger-text)` |
| **Границы, состояния, фокус (§4)** | `var(--border-subtle)`, `var(--border)`, `var(--border-strong)`, `var(--state-disabled-opacity)`, `var(--focus-ring)`, `var(--focus-ring-width)`, `var(--focus-ring-offset)` |
| **Типографика (§5)** | `var(--font-sans)`, `var(--font-mono)`, `var(--font-size-caption)`, `var(--font-size-small)`, `var(--font-size-ui)`, `var(--font-size-body)`, `var(--font-size-title)`, `var(--line-height-caption)`, `var(--line-height-small)`, `var(--line-height-ui)`, `var(--line-height-body)`, `var(--font-weight-medium)`, `var(--font-weight-semibold)`, `var(--font-weight-bold)`, `var(--letter-spacing-caps)` |
| **Отступы и радиусы (§6, §7)** | `var(--space-0)`, `var(--space-1)`, `var(--space-2)`, `var(--space-3)`, `var(--space-4)`, `var(--space-5)`, `var(--space-6)`, `var(--space-7)`, `var(--space-10)`, `var(--space-11)`, `var(--space-12)`, `var(--space-13)`, `var(--radius-xs)`, `var(--radius-sm)`, `var(--radius-md)`, `var(--radius-round)` |
| **Тени и слои (§8, §12)** | `var(--shadow-elevation-medium)`, `var(--shadow-elevation-high)`, `var(--shadow-popover)`, `var(--z-header)`, `var(--motion-fast)`, `var(--motion-base)`, `var(--ease-out)`, `var(--ease-in-out)` |
| **Раскладка и компоненты (§9, §11)** | `var(--layout-tree-min-w)`, `var(--layout-composer-h)`, `var(--layout-group-title-h)`, `var(--avatar-lg)`, `var(--icon-size)`, `var(--icon-size-lg)`, `var(--btn-height-sm)`, `var(--btn-height)`, `var(--btn-height-lg)`, `var(--btn-radius)`, `var(--slider-track-h)`, `var(--emoji-picker-h)`, `var(--menu-border)`, `var(--modal-w-sm)`, `var(--modal-w-md)`, `var(--modal-w-lg)`, `var(--modal-pad-x)`, `var(--modal-pad-y)` |

---

## 7. Полная WAI-ARIA разметка и клавиатурная навигация

### 7.1. Матрица WAI-ARIA ролей и атрибутов

| Компонент / Селектор | Тег / `role` | `aria-label` / `aria-labelledby` | Динамические ARIA и `data-*` атрибуты |
|---|---|---|---|
| `.message-attachments` | `div`, `role="group"` | `aria-label="Вложения сообщения"` | Рендерится только при наличии видимых вложений (после отсева `IS_THUMBNAIL = 2`) |
| `.attachments-mosaic` | `div` | — | `data-mosaic-count="{1..N}"`, `data-layout="single\|mosaic-2\|mosaic-3\|mosaic-4\|mosaic-many"` |
| `.attachment-media-item` | `figure` | — | `data-attachment-id`, `data-media-type="IMAGE\|VIDEO"`, `data-state="loading\|loaded\|error"`, `data-spoiler="none\|hidden\|revealed"`, `aria-busy="true"` при `loading` |
| `.attachment-spoiler-overlay` | `button[type="button"]` | Для изображений: `"Спойлер: изображение скрыто. Нажмите, чтобы показать"`; для видео: `"Спойлер: видео скрыто. Нажмите, чтобы показать"`; для файлов: `"Файл скрыт под спойлером. Нажмите, чтобы показать"` | `aria-expanded="false"` (при `hidden`) → `"true"` и `hidden` после раскрытия |
| `.attachment-media-trigger` | `button[type="button"]` | `"Открыть изображение «{filename}» в полном размере"` | `aria-haspopup="dialog"`, `tabindex="-1"` + `aria-hidden="true"` пока спойлер скрыт; `tabindex="0"` когда спойлер раскрыт |
| `.attachment-alt-badge` | `button[type="button"]` | `"Показать описание изображения (ALT)"` | `aria-expanded="false\|true"`, рендерится только при непустом `attachment.description` |
| `#attachment-lightbox` | `div.dialog-backdrop` + `.dialog[role="dialog"]` | `aria-modal="true"`, `aria-labelledby="attachment-lightbox-title"`, `aria-describedby="attachment-lightbox-meta"` | `hidden` в закрытом состоянии; кнопки навигации `#attachment-lightbox-prev` / `#attachment-lightbox-next` видимы при `mosaicCount > 1` |
| `.attachment-video-controls` | `div`, `role="group"` | `aria-label="Управление воспроизведением видео"` | Содержит кнопки `video-toggle-play` (`aria-pressed`), `video-toggle-mute` (`aria-pressed`), `video-fullscreen` и слайдеры `role="slider"` |
| `.attachment-ctrl-seek` / `.attachment-waveform-slider` | `input[type="range"]`, `role="slider"` | `"Перемотка видео «{filename}»"` / `"Перемотка аудио «{filename}»"` / `"Перемотка голосового сообщения"` | `aria-valuemin="0"`, `aria-valuemax="{durationSecs}"`, `aria-valuenow="{currentSecs}"`, `aria-valuetext="{cur_mm:ss} из {tot_mm:ss}"` |
| `.attachment-audio-card` | `div`, `role="group"` | `aria-label="Аудиофайл «{filename}»"` | `data-player-state="idle\|playing\|paused\|ended\|error"` |
| `.attachment-voice-card` | `div`, `role="group"` | `aria-label="Голосовое сообщение, длительность {mm:ss}"` | `data-player-state="idle\|playing\|paused\|ended\|error"`, столбики `.attachment-waveform-bars` имеют `aria-hidden="true"` |
| `.attachment-file-card` | `div`, `role="group"` | `aria-label="Файл «{filename}», {formattedSize}"` | `data-file-klass="{klass}"`, `data-spoiler="none\|hidden\|revealed"` |
| `.attachment-file-download` | `a` или `button` | `aria-label="Скачать файл «{filename}» ({formattedSize})"` | `download="{filename}"`, `rel="noreferrer noopener"`, `aria-disabled="true"` при невалидном URL |

### 7.2. Клавиатурная навигация и управление фокусом

1. **Последовательность обхода `Tab` / `Shift+Tab` внутри `.message-attachments`**:
   - **Если вложение закрыто спойлером (`data-spoiler="hidden"`)**:
     - В tab-порядке находится **только** кнопка `.attachment-spoiler-overlay` (`tabindex="0"`). Внутренние триггеры, ссылки и контролы плеера имеют `tabindex="-1"` и `aria-hidden="true"`, чтобы пользователь клавиатуры не проваливался внутрь скрытого спойлера до его явного раскрытия.
   - **Раскрытие спойлера с клавиатуры (`Enter` или `Space`)**:
     - Нажатие `Enter` или `Space` на `.attachment-spoiler-overlay` вызывает `event.preventDefault()`, переводит карточку в `data-spoiler="revealed"`, скрывает `.attachment-spoiler-overlay` (`hidden`) и **немедленно переносит фокус** на основной интерактивный элемент вложения:
       - для изображения — на `.attachment-media-trigger`;
       - для видео — на кнопку воспроизведения `.attachment-video-play-overlay`;
       - для аудио / голосового сообщения — на кнопку `.attachment-audio-play` / `.attachment-voice-play`;
       - для файла — на ссылку `.attachment-file-name`.
   - **Внутри раскрытого вложения**:
     - Изображение: `.attachment-media-trigger` → `.attachment-alt-badge` (если есть `description`).
     - Видео: `.attachment-video-play-overlay` (если `idle`/`paused`) → `video-toggle-play` → `.attachment-ctrl-seek` → `video-toggle-mute` → `.attachment-ctrl-volume` → `video-fullscreen`.
     - Аудио: `.attachment-audio-play` → `.attachment-audio-seek` → `audio-toggle-mute`.
     - Голосовое сообщение: `.attachment-voice-play` → `.attachment-waveform-slider` → `.attachment-voice-speed`.
     - Файл: `.attachment-file-name` → `.attachment-file-download`.
2. **Клавиатурное управление слайдерами перемотки и громкости (`role="slider"`)**:
   - `ArrowRight` / `ArrowUp`: сдвиг позиции вперёд на `+1` с (для громкости `+5%` или `+1%`);
   - `ArrowLeft` / `ArrowDown`: сдвиг позиции назад на `-1` с (для громкости `-5%` или `-1%`);
   - `Home`: переход в начало (`0:00` / `0%`);
   - `End`: переход в конец (`durationSecs` / `100%`).
   - При каждом изменении синхронно обновляются `input.value`, `aria-valuenow`, `aria-valuetext` и текстовый счётчик `.attachment-time-current` (а для голосового сообщения — атрибут `data-played="true|false"` на столбиках `.attachment-waveform-bar`).
3. **Клавиатурный контракт модального окна Lightbox (`#attachment-lightbox`)**:
   - При открытии lightbox запоминается ссылка на элемент-триггер (`opener = activeTrigger`), а фокус переводится на кнопку закрытия `#attachment-lightbox-close`.
   - Внутри открытого lightbox действует циклический `focus-trap` (`Tab` и `Shift+Tab` перемещают фокус только между `#attachment-lightbox-original`, `#attachment-lightbox-close`, `#attachment-lightbox-prev` и `#attachment-lightbox-next`).
   - Нажатие `ArrowLeft` / `ArrowRight` в открытом lightbox (когда в сообщении несколько изображений в `.attachments-mosaic`) переключает текущее изображение на предыдущее / следующее с обновлением `#attachment-lightbox-img`, `#attachment-lightbox-title` и `#attachment-lightbox-meta`.
   - Нажатие `Escape` (с `event.stopPropagation()`, чтобы не закрыть родительские панели) или клик по подложке `.dialog-backdrop` закрывает lightbox (`hidden`) и **возвращает фокус** на исходный `.attachment-media-trigger` в сообщении.

---

## 8. Матрица верификации контракта (`AT-03` → `tests/unit/attachments_render.test.mjs`)

| ID проверки | Проверяемое требование спецификации | Ожидаемый результат (оракул) | Источник / Метка |
|---|---|---|---|
| **T-AT03-01** | Фильтрация `IS_THUMBNAIL = 2` и пустых массивов | Вложения с `(flags & 2) !== 0` исключаются из `.message-attachments`; если после фильтрации список пуст, контейнер не рендерится | `[D]` `48550@223939` |
| **T-AT03-02** | Классификация inline-медиа по расширению `filename` vs MIME | `cat.png` (`image/png`) → `IMAGE` в `.attachments-mosaic`; `cat` без расширения (`image/png`) и `logo.svg` (`image/svg+xml`) → не попадают в сетку изображений, а рендерятся как `.attachment-file-card` (`klass="image"` и `klass="webcode"`) | `[D]` `@5918776`, `@9704193` |
| **T-AT03-03** | Сетка Mosaic (`1`, `2`, `3`, `4`, `5+`) и сохранение `aspect-ratio` | Для `1..5` изображений выставляются `data-mosaic-count="1..5"` и `data-layout="single\|mosaic-2\|mosaic-3\|mosaic-4\|mosaic-many"`; `width` и `height` из вложения переносятся в атрибуты и вычисляют `aspect-ratio` без `layout shift` | `[A]` `[D]` §2.1, §4.3 |
| **T-AT03-04** | Состояние загрузки Skeleton и правило `placeholder_version === 1` | До загрузки `data-state="loading"`, `aria-busy="true"`, виден `.attachment-skeleton`; blurhash активируется (`data-placeholder="blurhash"`) строго при `placeholder_version === 1` и игнорируется при `placeholder_version !== 1` | `[D]` `@841846` |
| **T-AT03-05** | Spoiler-оверлей `'СПОЙЛЕР'` и двухшаговое раскрытие по клику / `Enter` / `Space` | При `(flags & 8) !== 0` или `SPOILER_` рендерится `data-spoiler="hidden"` и кнопка `.attachment-spoiler-overlay` с пилюлей `'СПОЙЛЕР'` и `aria-expanded="false"`; первый клик или `Enter`/`Space` меняет состояние на `data-spoiler="revealed"`, скрывает оверлей и фокусирует триггер, **не открывая** lightbox | `[D]` `e0f9541@296`, `[A]` |
| **T-AT03-06** | Открытие и навигация Lightbox (`#attachment-lightbox`) | Клик или `Enter` по раскрытому `.attachment-media-trigger` открывает `#attachment-lightbox` (`role="dialog"`, `aria-modal="true"`), `ArrowRight`/`ArrowLeft` листает снимки сетки, `Escape` закрывает модалку и возвращает фокус на триггер | `[A]` |
| **T-AT03-07** | Видео-вложение: постер `proxy_url?format=webp`, длительность `m:ss` и `forceExternal` | Постер видео получает `format=webp`; длительность `36.09559` форматируется как `"0:36"`; при `width > 6016 && height > 3384` выставляется `data-force-external="true"` | `[D]` `5c14801@696`, `@5929320`, `@5935271` |
| **T-AT03-08** | Разделение обычного аудио и голосового сообщения по `message.flags & 8192` | При `message.flags = 0` рендерится `.attachment-audio-card` с именем файла, размером и линейным seek-баром; при `message.flags = 8192` (`IS_VOICE_MESSAGE`) рендерится `.attachment-voice-card` с `.attachment-waveform-bars` и кнопкой скорости `1x` | `[D]` `48550@223939`, `@5954877` |
| **T-AT03-09** | Декодирование `waveform` из base64 | Строка base64 декодируется в массив `0..1` (`charCodeAt(i) / 255`), число столбиков равно `Math.floor((width + 4) / 6)`, при перемотке столбики до текущей позиции получают `data-played="true"` | `[D]` `@6133343` |
| **T-AT03-10** | Карточка файла, таблица `klass` и форматирование размеров в Б / КБ / МБ | `classifyFile` возвращает точный `klass` из 14 классов (`acrobat`, `archive`, `code`, `document`, `spreadsheet`, `webcode`, `unknown` и др.); `formatAttachmentSize` форматирует `512 → "512 Б"`, `2563 → "2,5 КБ"`, `9256495 → "8,8 МБ"` | `[D]` `@9704193`, `@3543521` |
| **T-AT03-11** | Защита `safeHref` и обработка первого сегмента `content_type` | `content_type: "text/plain; charset=utf-8"` корректно парсится по первому сегменту; ссылки `javascript:alert(1)` блокируются (`aria-disabled="true"`), имена файлов экранируются через `textContent` | `[D]` `@6056280`, `[F]` |
| **T-AT03-12** | Строгая токенизация (`100% token binding`) | Все CSS-переменные в спецификации присутствуют в `rust/gui-spike/ui/tokens.css`, отсутствуют сырые цветовые литералы | `[T]` `tokens.css` |
