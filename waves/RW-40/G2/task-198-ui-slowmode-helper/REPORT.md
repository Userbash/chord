# RW-40 / G2 — task-198: UI slowmode helper

Карточка: T-198 (хелперы медленного режима). Режим: token-lean.

## Сделано
1. `tests/unit/slowmode.test.mjs` — red-фаза: модуль отсутствует (`exit=1`, `pass 0 / fail 1`), лог `red.log`.
2. `rust/gui-spike/ui/features/slowmode.js` — `// @ts-check` + JSDoc; чистые функции без DOM и сети.
3. Green: `node --test tests/unit/slowmode.test.mjs` → `tests 14, pass 14, fail 0` (`green.log`).

## Контракт
- `MAX_SLOWMODE_SECONDS = 21600` (6 ч).
- `formatSlowmodeIntervalRu`: кламп 0..=MAX; `0 -> «Медленный режим выключен»`, `<60 -> N сек.`, `<3600 -> floor/60 мин.`, иначе `floor/3600 ч.`.
- `computeSlowmodeRemainingSec(rate, lastSentMs, nowMs = Date.now(), canBypass = false)`: 0 при bypass, `rate<=0`, отсутствии `lastSentMs` и на истёкшем лимите; иначе `ceil((last+rate*1000-now)/1000)`.
- `formatSlowmodeCountdownMMSS`: `MM:SS`, от часа — `HH:MM:SS`; нечисловые/отрицательные → `00:00`.

## Файлы и md5
- `tests/unit/slowmode.test.mjs` — `3b3961b64533e5733754321fdef919a5`
- `rust/gui-spike/ui/features/slowmode.js` — `1a12cfbcb84b9654e7b71629df1173ce`

## Риски
- Один тест red-фазы был логически неверен (`rate=60` при последнем сообщении 5 с назад даёт 55 с, а не 0) — исправлен до green, реализация не подгонялась.
- `gate:worker`, полный workspace и commit не запускались по условиям token-lean.
