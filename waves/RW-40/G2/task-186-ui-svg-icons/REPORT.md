# task-186: SVG-иконки Discord UI

## Что сделано
- `rust/gui-spike/ui/features/svg-icons.js`: `ICON_SIZE=20`, frozen `SVG_ICONS` (20 имён),
  `createIconSvg(doc, name, options)` (null для неизвестного имени), идемпотентный
  `upgradeGlyphIcons(root)` (пропускает готовые svg и узлы с `data-icon-rendered="svg"`).
- `tests/unit/svg-icons.test.mjs`: 14 тестов на карту, атрибуты, options, null-ветки,
  идемпотентность, несколько узлов, неизвестные иконки и пустой корень.

## Прогоны
- RED: `red.log` — модуль отсутствовал, все тесты падали.
- GREEN: `ℹ tests 14 ℹ pass 14 ℹ fail 0 ` (0 падений).
  Первый зелёный прогон выявил 2 падения: svg сам несёт `data-icon` и повторно
  заворачивался; исправлено пропуском `tagName === "svg"` и уточнением выборки в тесте.

## md5
- rust/gui-spike/ui/features/svg-icons.js  `7fb502e19f6495d77cc53542c023abb5`
- tests/unit/svg-icons.test.mjs             `8981e27b313b8ec6e385e888d4c9fa87`
