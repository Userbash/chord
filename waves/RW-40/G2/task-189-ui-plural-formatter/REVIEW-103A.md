# REVIEW-103A — T-189 UI plural formatter (G2)

**Вердикт:** APPROVED. `P0=0`, `P1=0`, мутации `5/5` убиты. Без git commit.

## Проверено
- `rust/gui-spike/ui/features/plural-format.js` (94 строки) против контракта в `REPORT.md`:
  категория CLDR для ru (`one` — mod10=1 и mod100≠11; `few` — mod10 2..4 и mod100 вне 12..14;
  `many` — остальные целые ≥0; `other` — дробные, отрицательные, NaN/Infinity, не-число/не-строка).
- `formatPluralRu` нормализует счётчик `Math.max(0, Math.trunc(Number(count) || 0))`, подставляет
  все `{count}`, возвращает `""` при отсутствии формы или нестроковом шаблоне.
- Пресеты четырёх UI-строк заморожены (`Object.freeze`), `formatPresetRu` ходит через
  `hasOwnProperty` — ключи прототипа не утекают.

## Прогоны
- Targeted: `node --test tests/unit/plural-format.test.mjs` → tests 10, pass 10, fail 0.
- md5 до и после мутаций: `74566041a78642f5b675d54ed08f54b6` — совпадает с `REPORT.md`,
  рабочее дерево вернулось в исходное состояние.

## Мутации (все убиты)
| # | Мутация | fail |
| --- | --- | --- |
| m1 | убран `mod100 !== 11` в ветке `one` | 4 |
| m2 | убрано исключение `mod100 12..14` в ветке `few` | 1 |
| m3 | убрана проверка `n < 0` в категории | 1 |
| m4 | убран clamp `Math.max(0, n)` в нормализации | 1 |
| m5 | `split/join` → `replace` (только первое `{count}`) | 1 |

## Замечания (не блокируют)
- P3: `formatPluralRu(Infinity, { other: '{count} …' })` даст строку с `Infinity` — нормализация
  не отсекает нефинитные значения. На практике счётчики целые; поведение стоит зафиксировать
  тестом, если появится источник `Infinity`.
