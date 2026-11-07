# task-244 — UI attachment helper (G2)

## Итог
Чистый JS-хелпер вложений: лимиты, категория MIME, размер по-русски, валидация пачки.
Strict TDD: red — `ERR_MODULE_NOT_FOUND`, green — 14/14 pass.

## Файлы и md5
| Файл | md5 |
| --- | --- |
| `rust/gui-spike/ui/features/attachment-helper.js` | `7b6ac88b63e1ddb93ef4b0296ae4c673` |
| `tests/unit/attachment-helper.test.mjs` | `f0150ebd5bee69d52595d4d94f904abf` |

## Контракт
- `MAX_ATTACHMENTS_PER_MESSAGE = 10`
- `MAX_STANDARD_FILE_BYTES = 25 * 1024 * 1024` (`26214400`)
- `ATTACHMENT_CATEGORIES` — `Object.freeze({ IMAGE, VIDEO, AUDIO, DOCUMENT })`
- `classifyAttachmentMime(mimeType)` — trim + lowercase, префиксы `image/` `video/` `audio/`, иначе `document`
- `formatAttachmentSizeRu(bytes)` — `< 1024` → `"N Б"`, `< 1 МиБ` → `"X.Y КБ"`, иначе `"X.YY МБ"`; отрицательные и `NaN` → `"0 Б"`
- `validateAttachmentsBatch(files, maxBytes = MAX_STANDARD_FILE_BYTES)` — `"Нет файлов"` → `"Максимум 10 файлов"` → `"Файл превышает лимит размера"` (`size <= 0 || size > maxBytes`), иначе `{ valid: true, errorRu: null, totalBytes }`

## Прогоны
- red: `node --test tests/unit/attachment-helper.test.mjs` → exit 1, `ERR_MODULE_NOT_FOUND` (модуля ещё нет) — `red.log`
- green: тот же прогон → exit 0, `tests 14 / pass 14 / fail 0` — `green.log`

## Границы
`maxBytes` проверяется на каждом файле; порядок проверок фиксирован тестом (count раньше size).
DOM/сеть не затронуты, `gate:worker` и git-коммит по ТЗ не запускались.

## Риски
- Формат размера: `1048575` даёт `"1024.0 КБ"` по контракту ТЗ — не округление до МБ.
- `size` приводится через `Number(...) || 0`, поэтому мусорный размер трактуется как `0` (ошибка «превышает лимит»), а не как pass.
