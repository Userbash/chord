# task-193 — UI attachment validator (G2)

Ветка: `feature/task-193-ui-attachment-validator` (не коммичено).

## Сделано
- `tests/unit/attachment-validator.test.mjs` — 21 тест: константы (`10`, `26214400`, `SPOILER_`),
  `formatFileSizeRu` (clamp, Б/КБ/МБ), `classifyAttachmentKind` (extension + MIME),
  `isSpoilerFilename` / `toggleSpoilerFilename` (basename, каталог сохраняется),
  `validateAttachmentBatch` (`TOO_MANY_FILES`, `FILE_TOO_LARGE`, `BATCH_TOO_LARGE`, границы ровно в лимит).
- `rust/gui-spike/ui/features/attachment-validator.js` — `// @ts-check` + JSDoc, чистые функции, без DOM.
- Red: `red.log` (`'test failed'`, модуль отсутствовал). Green: `green.log` — 21 pass, 0 fail, 0 skip.
- Полный `gate:worker` не запускался (token-lean режим ТЗ), targeted-тест зелёный.

## Файлы и md5
```
a155f98341dd6b28cac9b9f5a0a0ef24  rust/gui-spike/ui/features/attachment-validator.js
f356b59796f106be5ad346fb3195142a  tests/unit/attachment-validator.test.mjs
```

## Замечания
- `totalBytes` возвращается всегда (и при ошибке), считается по всем переданным файлам.
- Порядок проверок: количество → размер файла → сумма батча.
- Неотрицательный `size` и `NaN` в размере трактуются как `FILE_TOO_LARGE`.

## Осталось
- Приёмка Team Lead / PM / техревью; подключение валидатора к composer — отдельная карточка.
