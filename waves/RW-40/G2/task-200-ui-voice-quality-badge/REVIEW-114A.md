# REVIEW-114A — TASK-200 UI voice quality badge (G2)

Вердикт: **APPROVED**. P0 = 0, P1 = 0, мутации 5/5 убиты.

## Что проверено
- `rust/gui-spike/ui/features/voice-quality.js` (69 строк): три чистых форматтера,
  `// @ts-check`, JSDoc на всех экспортах, типы `ConnectionQuality`/бар-бакеты 0..3.
- Логика полос соответствует заявленному контракту: `100 && <=2` → good, иначе
  `250 && <=10` → average, иначе poor. `loss <= 2` при 100 мс даёт average, не poor.
- Защита от мусорного входа: `null/undefined/NaN/Infinity/<0` пинг → unknown;
  битрейт клампится в 8..512 кбит/с, не-число уходит в нижний кламп.

## Прогоны
- targeted: `node --test tests/unit/voice-quality.test.mjs` — tests 10, pass 10, fail 0.
- md5 до прогона и после восстановления: `6a5ba080de177bc9c09c6fb9f6670d17` (совпадает с REPORT.md).

## Мутации (single-run script, авто-restore, diff = byte-identical)
| # | Мутация | Результат |
|---|---------|-----------|
| M1 | `loss <= 2` → `loss <= 20` | KILLED |
| M2 | `loss <= 10` → `loss <= 100` | KILLED |
| M3 | убран кламп битрейта (`raw` без `Math.min/max`) | KILLED |
| M4 | бакет `1440p` → `1080p` | KILLED |
| M5 | бакет `60 FPS` → `30 FPS` | KILLED |

## Замечания (не блокирующие)
- P2: бейдж и качество не смонтированы в DOM — вне write-scope задачи, зафиксировано в REPORT.md.
- Полный `gate:worker` не запускался по условиям token-lean ТЗ; перед merge нужен T1-прогон.
