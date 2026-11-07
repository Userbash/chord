# Отчёт воркера: RW-40 / AT-03 — Рендер карточек вложений в сообщениях чата

- **Волна / карточка**: `RW-40` / `AT-03` (`at03-render-ui`)
- **Задача Orca**: `task_47db40c562c7` (`ctx_ff01e13d26c2`)
- **Терминал воркера**: `term_80855b80-1392-4272-8675-6f5e552a18d0`
- **Статус**: Выполнено (Strict TDD: RED → GREEN → 5/5 мутаций убиты, `ui-tsc` и целевые тесты `0` ошибок)

---

## 1. Что сделано

По принятой спецификации `waves/RW-40/DESIGN/ATTACHMENTS-RENDER-SPEC.md` и RE-досье `waves/RW-40/ATTACH-RENDER-RE.md` реализован рендер карточек вложений (`IMAGE`, `VIDEO`, `AUDIO`, `VOICE_MESSAGE`, `FILE`) в сообщениях чата:

1. **Константы и классификаторы (`rust/gui-spike/ui/features/chat.js`)**:
   - Экспортированы `AttachmentFlags` (`IS_CLIP = 1`, `IS_THUMBNAIL = 2`, `IS_REMIX = 4`, `IS_SPOILER = 8`, `CONTAINS_EXPLICIT_MEDIA = 16`, `IS_ANIMATED = 32`, `CONTAINS_GORE_CONTENT = 64`, `CONTAINS_SELF_HARM_CONTENT = 128`, `HAS_TIMELINE_COMMENTS = 256`) и `MessageFlags` (`EPHEMERAL = 64`, `IS_VOICE_MESSAGE = 8192`).
   - Реализованы три классификатора:
     1. `classifyInlineMedia(attachment)` (`Rr` `[D] @5918776`) — классификация inline-медиа в ленте строго по расширению `filename` (`/\.(png|jpe?g|jfif|webp|gif|heic|heif|dng|avif)$/i` → `"IMAGE"`, `/\.(mp4|webm|mov|qt)$/i` при наличии `proxy_url`/`url` → `"VIDEO"`, иначе `"INVALID"`);
     2. `classifyByMime(contentType)` (`FE` `[D] @6056280`) и `firstMimeSegment(contentType)` — по первому сегменту `content_type` до `/` и `;`;
     3. `classifyFile(filename, contentType)` (`TX` `[D] @9704193`) — 15 упорядоченных правил (`photoshop`, `webcode` для `image/svg+xml`, `image`, `video`, `acrobat`, `ae`, `sketch`, `ai`, `archive`, `code`, `document`, `spreadsheet`, `webcode`, `audio`, фоллбек `"unknown"`).
2. **Форматирование, декодеры и безопасность (`rust/gui-spike/ui/features/chat.js`)**:
   - `formatDuration(durationSecs)` (`[D] @5935271`) — усечение `0 | durationSecs` в формат `m:ss` (`36.09559` → `"0:36"`, `8.119976` → `"0:08"`, невалидные → `"--:--"`);
   - `formatAttachmentSize(bytes)` — русские единицы `Б` / `КБ` / `МБ` / `ГБ` по шкале `1024` с запятой (`512 → "512 Б"`, `2563 → "2,5 КБ"`, `26921 → "26,3 КБ"`, `9256495 → "8,8 МБ"`);
   - `decodeWaveform(base64Str, targetWidthPx = 192)` (`[D] @6133343`) — декодирование base64-массива байтов амплитуд (`charCodeAt(0) / 255`) и ресемплинг в `Math.floor((targetWidthPx + 4) / 6)` (`32` при `192px`) столбиков;
   - `computeSingleMediaBox(width, height)` — сохранение `aspect-ratio` в пределах `442×400` (`min 150×110`) и фоллбек `16 / 9`;
   - `getObscureReason(attachment, options)` (`[D] e0f9541@296`) — приоритет `explicit_content` (`16`) → `gore_content` (`64`) → `self_harm_content` (`128`) → `potential_explicit_content` (`content_scan_version === -1`) → `spoiler` (`IS_SPOILER = 8` / `SPOILER_` / `channel.isSpoiler`);
   - `safeAttachmentHref(url)` — допуск только `http:` / `https:` (и `blob:` для локальных превью), блокировка `javascript:` / `data:` с переводом элемента в `aria-disabled="true"` / `data-state="error"`, создание DOM исключительно через `createElement` + `textContent`;
   - `isAttachmentUrlExpiring(url)` + `options.refreshUrls` — отслеживание `ex` в query-строке CDN и обновление истёкших ссылок.
3. **DOM-рендер `.message-attachments`, Lightbox и сохранение инварианта `.row` (`DOM-CONTRACT §7.1`)**:
   - Фильтрация `IS_THUMBNAIL = 2` (`(flags & 2) !== 0`), пустых массивов и опросов (`isPoll`);
   - Сетка `.attachments-mosaic` (`data-mosaic-count="1..N"`, `data-layout="single|mosaic-2|mosaic-3|mosaic-4|mosaic-many"`), Skeleton (`data-placeholder="blurhash"` строго при `placeholder_version === 1`), бейджи `GIF`, `ALT`, `REMIX` (`IS_REMIX = 4`), `+{N - 4}`;
   - Двухшаговое раскрытие спойлера `.attachment-spoiler-overlay` (клик / `Enter` / `Space` меняет `data-spoiler="hidden"` → `"revealed"` и переводит фокус на внутренний триггер, не открывая lightbox);
   - Модальное окно `#attachment-lightbox` (`role="dialog"`, `aria-modal="true"`, навигация `ArrowLeft`/`ArrowRight`, циклический `Tab` focus-trap, закрытие по `Escape` с возвратом фокуса на триггер);
   - Видео-плеер `.attachment-video-item` с постером `proxy_url?format=webp`, контролами и защитой `data-force-external="true"` для разрешений `> 6016×3384`;
   - Разделение `.attachment-audio-card` (`(message.flags & 8192) === 0`) и `.attachment-voice-card` (`MessageFlags.IS_VOICE_MESSAGE = 8192`) с 32 столбиками `.attachment-waveform-bar` (`data-played="true|false"`) и переключателем скорости `1x` / `1.5x` / `2x`;
   - Карточка файла `.attachment-file-card` с иконкой `data-file-klass`, безопасными ссылками и спойлер-оверлеем;
   - В `renderChatRow` и `renderChannelMessage` сохранён инвариант **ровно 2 прямых дочерних элементов `.row` (`.author`, `.text`)** (`DOM-CONTRACT §7.1`), `.text-body` остаётся первым дочерним узлом `.text`, а `.message-attachments` монтируется внутри `.text[data-stack="true"]` перед `.reactions-row` / `.embeds` с добавлением `"attachments"` в `data-extras` на `.row`.
4. **Стили (`rust/gui-spike/ui/components/chat.css`)**:
   - Добавлены правила секции AT-03 со 100% привязкой к существующим токенам `rust/gui-spike/ui/tokens.css` (0 сырых цветов `#hex`/`rgb`/`hsl`, 0 `!important`, поддержка `@media (prefers-reduced-motion: reduce)` и `:focus-visible`).

---

## 2. Доказательства TDD (RED → GREEN → Mutations)

### 2.1. RED-прогон (`waves/RW-40/PM/at03-render-ui/red.log`)
Команда:
```sh
node --test tests/unit/attachments-render.test.mjs > waves/RW-40/PM/at03-render-ui/red.log 2>&1
```
Результат: **exit code 1** — `tests 11 | pass 0 | fail 11` (поведенческие падения `AssertionError [ERR_ASSERTION]` по каждому из пунктов `T-AT03-01..T-AT03-12`, не `ERR_MODULE_NOT_FOUND`).

### 2.2. GREEN-прогон и Typecheck (`waves/RW-40/PM/at03-render-ui/green.log`)
Команды:
```sh
npx tsc -p rust/gui-spike/ui/tsconfig.json
npx tsc -p tsconfig.json
node --test tests/unit/attachments-render.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs tests/unit/channel-paging.test.mjs tests/meta/test-policy.test.mjs
```
Результат:
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → `exit=0`
- `npx tsc -p tsconfig.json` → `exit=0`
- Целевые и смежные unit/meta-тесты → `tests 238 | suites 35 | pass 238 | fail 0 | skipped 0` (`exit=0`).

### 2.3. Мутационное тестирование (`waves/RW-40/PM/at03-render-ui/mutations.log`)
Команда запуска каждого мутационного прогона (с последующим восстановлением базового файла `rust/gui-spike/ui/features/chat.js` и сверкой `md5sum`):
```sh
node --test tests/unit/attachments-render.test.mjs >> waves/RW-40/PM/at03-render-ui/mutations.log 2>&1
```
Проведено **5 мутаций** в `rust/gui-spike/ui/features/chat.js` (результат **5/5 убиты**, базовый MD5 `0e3dc4984bf7a5db8f400e3c288f873f` восстановлен и сверен):
1. **M1**: Отключение фильтра `IS_THUMBNAIL = 2` в `renderAttachments` → **KILLED** (`T-AT03-01`).
2. **M2**: Подмена классификатора `classifyInlineMedia` (`Rr`) с расширения `filename` на первый сегмент MIME → **KILLED** (`T-AT03-02`).
3. **M3**: Игнорирование `IS_SPOILER = 8` и префикса `SPOILER_` в `getObscureReason` → **KILLED** (`T-AT03-05`).
4. **M4**: Игнорирование `MessageFlags.IS_VOICE_MESSAGE = 8192` при маршрутизации `AUDIO` vs `VOICE_MESSAGE` → **KILLED** (`T-AT03-08 & T-AT03-09`).
5. **M5**: Удаление параметра `?format=webp` в `buildVideoPosterUrl` → **KILLED** (`T-AT03-07`).

---

## 3. Изменённые и созданные файлы + MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/chat.js` | modified | `0e3dc4984bf7a5db8f400e3c288f873f` |
| `rust/gui-spike/ui/components/chat.css` | modified | `96b789ab7b8459008efa1f429dfe3863` |
| `tests/unit/attachments-render.test.mjs` | added | `8b4678d19c6aa4ed9a6b7aa8ad787615` |
| `waves/RW-40/PM/at03-render-ui/red.log` | added | `545dcbe0c8f759e8fb494026f1eeb0e8` |
| `waves/RW-40/PM/at03-render-ui/green.log` | added | `0a38fa67fb9aedfbb17126a9418a04cc` |
| `waves/RW-40/PM/at03-render-ui/mutations.log` | added | `beec144a9ca86dfb666f91c0982ceeb5` |
| `waves/RW-40/PM/at03-render-ui/REPORT.md` | added | *(текущий отчёт)* |

---

## 4. Риски и открытые вопросы

- **Рисков нарушения DOM-контракта `.row` нет**: инвариант `DOM-CONTRACT §7.1` (`.row` имеет ровно 2 прямых дочерних узла `.author` и `.text`, `.text-body` — первый дочерний узел `.text`) проверен как в `tests/unit/attachments-render.test.mjs`, так и в существующих контрактных сюитах `tests/unit/gui-spike-dom.test.mjs` и `tests/unit/gui-spike-features.test.mjs`.
- **Чистота политики тестов**: 0 `@ts-ignore`, 0 `.only` / `.skip`, проверено `tests/meta/test-policy.test.mjs`.
