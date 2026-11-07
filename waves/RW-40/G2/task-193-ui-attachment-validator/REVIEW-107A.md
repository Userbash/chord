# REVIEW-107A — task-193 UI attachment validator

**Verdict: APPROVED.** P0=0, P1=0, mutations 5/5 killed.

## Проверено
- `rust/gui-spike/ui/features/attachment-validator.js` (152 строки, `// @ts-check`, чистые функции без DOM).
- `tests/unit/attachment-validator.test.mjs` — targeted-прогон: 21 pass, 0 fail, 0 skip, 0 todo (~41 ms).
- Контракт совпадает с REPORT.md: лимиты `10` / `26214400`, порядок проверок count → file → total, `totalBytes` возвращается всегда.

## Мутации (5/5 killed)
| # | Мутация | Результат |
|---|---------|-----------|
| M1 | `MAX_ATTACHMENTS_PER_MESSAGE` 10 → 11 | killed |
| M2 | `MAX_FILE_SIZE_BYTES` +1 байт | killed |
| M3 | `formatFileSizeRu` КБ: `toFixed(1)` → `toFixed(0)` | killed |
| M4 | `classifyAttachmentKind`: image по расширению → `'file'` | killed |
| M5 | `totalBytes > LIMIT` → `>=` | killed |

## Целостность
- Исходник восстановлен после мутаций, md5 `a155f98341dd6b28cac9b9f5a0a0ef24` — совпадает с REPORT.md.
- Полный `gate:worker` не запускался (token-lean режим ТЗ). Git-коммитов нет.

## Осталось
- Подключение валидатора к composer — отдельная карточка; приёмка Team Lead/PM.
