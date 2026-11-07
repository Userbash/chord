# task-255 — UI панель бустов сервера (чистые помощники)

Batch/карточка: RW-40 / task-255. Ветка: feature/task-255-ui-guild-boost, HEAD 850fab9.

## Сделано
- `rust/gui-spike/ui/features/guild-boost.js` (`// @ts-check`): `BOOST_TIER_THRESHOLDS`, `BOOST_TIER_PERKS_RU`, `resolveBoostTierFromCount`, `formatBoostProgressSummaryRu` — всё по ТЗ, объекты заморожены.
- `tests/unit/guild-boost.test.mjs`: 8 тестов — пороги, перки, границы 2/7/14, негативные, дробные, `NaN`/`undefined`/`"7"`/`"abc"`, форма сводки и остаток.

## Red
`node --test tests/unit/guild-boost.test.mjs` -> exit 1 (модуль ещё не существует, ERR_MODULE_NOT_FOUND). Лог: `red.log`.

## Green
`node --test tests/unit/guild-boost.test.mjs` -> exit 0, `pass 8 / fail 0 / skipped 0`. Лог: `green.log`.

## Файлы и md5
```
bb3064fac22bf960e3c271553fa3706a  rust/gui-spike/ui/features/guild-boost.js
6fbecd4202057a6bdabef99445074fd5  tests/unit/guild-boost.test.mjs
```

## Риски
- `gate:worker`, `tsc`, `cargo` и `git commit` не запускались по условию ТЗ; проверка типов для `// @ts-check` на этом файле не выполнялась.
- Интеграция в DOM/панель (рендер, CSS, i18n-ключи) не входит в scope — только чистые функции.
