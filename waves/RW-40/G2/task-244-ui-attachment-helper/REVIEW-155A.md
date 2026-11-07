# task-244 — Technical Review (155A)

**Verdict: APPROVED**

## Проверено
- `rust/gui-spike/ui/features/attachment-helper.js` (71 строка) — чистые хелперы без DOM и сети.
- `tests/unit/attachment-helper.test.mjs` — 14 тестов, оракул — числа и строки контракта ТЗ.
- соответствие контракту из `REPORT.md`: `10` файлов, `26214400` байт, `Object.freeze` категорий, порядок проверок `Нет файлов` → `Максимум 10 файлов` → `Файл превышает лимит размера`.

## Targeted-прогон
`node --test tests/unit/attachment-helper.test.mjs` → exit 0, `tests 14 / pass 14 / fail 0`.
Константы совпадают с Discord: `MAX_ATTACHMENTS_PER_MESSAGE = 10`, `MAX_STANDARD_FILE_BYTES = 25 * 1024 * 1024 = 26214400`.

## Мутационное тестирование 5/5 (killed)

| # | Мутация | Файл/место | Результат |
| --- | --- | --- | --- |
| m1 | `MAX_ATTACHMENTS_PER_MESSAGE` `10` → `11` | константа лимита | убита, 3 теста упали |
| m2 | `(safeBytes / 1024).toFixed(1)` → `.toFixed(2)` | `formatAttachmentSizeRu`, КБ | убита, 1 тест упал |
| m3 | `startsWith('audio/')` → `startsWith('sound/')` | `classifyAttachmentMime` | убита, 1 тест упал |
| m4 | `size > maxBytes` → `size >= maxBytes` | `validateAttachmentsBatch` | убита, 2 теста упали |
| m5 | убран клэмп `Math.max(0, ...)` | `formatAttachmentSizeRu`, байты | убита, 1 тест упал |

Каждая мутация: применение → прогон targeted-теста (exit 1, `ℹ fail ≥ 1`) → откат.
Логи: `/tmp/opencode/m1-max-attachments.log` … `m5-negative-clamp.log` (внешняя сессия ревьюера, воспроизводится той же последовательностью правок).

## Восстановление файла
После отката: `cmp` с оригиналом — байт-в-байт совпадает; `md5 = 7b6ac88b63e1ddb93ef4b0296ae4c673` — совпадает с `REPORT.md`.
Повторный targeted-прогон после восстановления: exit 0, `tests 14 / pass 14 / fail 0`.

## Замечания
Блокирующих нет (P0 = 0, P1 = 0). Наблюдение из `REPORT.md` подтверждено тестом: `1048575` даёт `"1024.0 КБ"`, а не `"1.00 МБ"` — это поведение контракта, не дефект.
`gate:worker` и git-коммит по ТЗ ревью не запускались.
