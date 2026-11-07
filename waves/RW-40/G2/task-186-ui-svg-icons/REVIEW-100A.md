# REVIEW-100A — task-186 SVG-иконки

**Вердикт: APPROVED. P0=0, P1=0, мутации 5/5.**

## Проверено
- `svg-icons.js` (118 строк) прочитан целиком: `ICON_SIZE=20`, frozen `SVG_ICONS` (20 имён), `createIconSvg`
  (null для неизвестного имени и без документа), `upgradeGlyphIcons` (пропуск готового `svg` и маркера
  `data-icon-rendered`, `replaceChildren` с fallback, счётчик только реальных замен).
- Отчёт `REPORT.md` совпадает с кодом и с наблюдаемым поведением.

## Прогон
- `node --test tests/unit/svg-icons.test.mjs`: `tests 14 / pass 14 / fail 0 / skipped 0`.
- md5 до и после мутаций: `7fb502e19f6495d77cc53542c023abb5` (восстановлен).

## Мутации (все убиты, rc=1)
| # | Мутация | Упало |
|---|---|---|
| M1 | `ICON_SIZE = 20` → `24` | 3 |
| M2 | дефолтный класс `"ui-icon"` → `"icon"` | 2 |
| M3 | `aria-hidden` `"true"` → `"false"` | 1 |
| M4 | убран пропуск готового `svg` в `upgradeGlyphIcons` | 1 |
| M5 | `data-icon` на `svg` → константа `"x"` | 2 |

## Замечания
Блокирующих нет. Коммит не делался.
