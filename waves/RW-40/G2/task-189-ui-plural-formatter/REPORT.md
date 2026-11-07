# T-189 — UI plural formatter (G2)

**Статус:** green, targeted-тесты 10/10, tsc по файлам чисто. Без git commit.

## Файлы

| Файл | md5 |
| --- | --- |
| `rust/gui-spike/ui/features/plural-format.js` | `74566041a78642f5b675d54ed08f54b6` |
| `tests/unit/plural-format.test.mjs` | `795f90fb3a3e0cf4ee8fb1e78e1885c9` |

## Контракт

- `selectRussianPluralCategory(count)` → `one` (n%10===1 && n%100!==11), `few`
  (n%10=2..4 && (n%100<12 || n%100>14)), `many` (прочие целые ≥0: 0, 5..20, 25, 111),
  `other` (дробные, отрицательные, NaN/Infinity, не-число/не-строка: `null`, `{}`, `[]`, boolean).
- `formatPluralRu(count, forms)` — счётчик `Math.max(0, Math.trunc(Number(count) || 0))`,
  подставляет все `{count}`; нет формы/шаблон не строка → `""`.
- `RU_PLURAL_PRESETS` — `unreadMessages`, `membersOnline`, `typingUsers`, `attachmentsCount`;
  `formatPresetRu(key, count)`, неизвестный ключ → `""` (hasOwnProperty, без прототипа).

## Прогоны

- Red: `red.log` — модуль отсутствовал, fail 1 (0 pass).
- Green: `green.log` — tests 10, suites 3, pass 10, fail 0.
- `npx tsc -p tsconfig.json --noEmit` — ошибок по этим файлам нет.

## Риски

- `formatPluralRu` без `other` в формах возвращает `""` — вызывающий код должен передавать
  полный набор или брать пресет. `'other'` в категории достигается только через прямой вызов
  `selectRussianPluralCategory` (после нормализации счётчик всегда целый ≥0).
