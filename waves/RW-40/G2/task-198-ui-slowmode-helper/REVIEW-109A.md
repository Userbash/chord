# REVIEW-109A — T-198 UI slowmode helper (token-lean)

Вердикт: **APPROVED**. P0=0, P1=0. Мутации: **5/5 killed**.

## Проверено
- `rust/gui-spike/ui/features/slowmode.js` (92 строки): чистые функции, без DOM/сети, `// @ts-check` + JSDoc, все экспорты задокументированы.
- `node --test tests/unit/slowmode.test.mjs`: `tests 14, pass 14, fail 0`.
- Контракт совпадает с `REPORT.md` §Контракт: потолок 21600, границы 0/60/3600, `ceil` для остатка, `MM:SS`/`HH:MM:SS`, `00:00` для нечисловых.

## Мутации (каждая: правка → тест → откат)
| # | Мутация | Итог |
|---|---------|------|
| 1 | `MAX_SLOWMODE_SECONDS 21600 → 21599` | killed |
| 2 | `Math.ceil → Math.floor` в остатке | killed |
| 3 | `hours > 0 → hours > 1` в отсчёте | killed |
| 4 | `canBypass → return 1` | killed |
| 5 | `seconds <= 0 → seconds < 0` в метке | killed |

После отката md5 = `1a12cfbcb84b9654e7b71629df1173ce`, совпадает с `REPORT.md`. Рабочее дерево не изменено, commit не делался.

## Замечания (P2, не блокируют)
- `!lastMessageSentMs` в `computeSlowmodeRemainingSec` считает `0` отсутствием сообщения — для Unix-эпохи некорректно, но Discord шлёт `null`, риск нулевой.
- Red-тест, исправленный до green (см. `REPORT.md` §Риски), подтверждён независимой мутационной проверкой: реализация не подогнана под ошибочное ожидание.
- По условиям token-lean `gate:worker`, полный workspace и typecheck не запускались — нужен прогон в integration-лейне.
