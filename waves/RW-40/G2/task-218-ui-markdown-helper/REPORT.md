# T-218 (FE-P2-21) — Markdown Spoiler / Codeblock / Timestamp Helper

Ветка: `feature/task-218-ui-markdown-helper`. База: `unstable`. Коммитов нет.

## Что сделано
- `rust/gui-spike/ui/features/markdown-helper.js` — новый чистый модуль (`// @ts-check`, JSDoc):
  - `TIMESTAMP_STYLES` — замороженный `['t','T','d','D','f','F','R']`;
  - `parseDiscordTimestampTag(tag)` — строгий разбор `<t:UNIX>` / `<t:UNIX:STYLE>`
    (UNIX — целое, включая отрицательные и `0`; дефолт стиля `f`); невалидное → `null`;
  - `extractSpoilerSpans(text)` — непустые `||...||` с `trim()`, сканер пропускает
    inline-код (серии бэктиков любой длины), поэтому `` `||x||` `` и ``` ``||x||`` ```
    спойлерами не считаются;
  - `formatRelativeTimestampRu(unix, now)` — «только что» / «через несколько секунд»,
    `X мин.` / `X ч.` / `X дн.` с округлением вниз.
- `tests/unit/markdown-helper.test.mjs` — 9 тестов: валидные/невалидные теги, границы
  `0`/отрицательные/`1e9`/`12.5`, спойлеры в бэктиках, все диапазоны относительного времени,
  проверка `// @ts-check` + `@module` в исходнике.

## TDD
- Red: `node --test tests/unit/markdown-helper.test.mjs` → exit 1 (модуль отсутствует), `red.log`.
- Green: тот же прогон → exit 0, `tests 9 / pass 9 / fail 0`, `green.log`.

## Артефакты / md5
- `markdown-helper.js` — `e28a346b7190efd9ba6f07f58ea2c117`
- `markdown-helper.test.mjs` — `e24ada3176fd228a1111d86f44900778`

## Риски / не сделано
- Полный `gate:worker`, `tsc`, `cargo`, e2e не запускались (token-lean режим) — нужен прогон в integration-лейне.
- Codeblock (```` ``` ````) отдельно не парсится: задача требовала только независимость спойлеров от inline-кода.
- Модуль не подключён к рантайму рендера (только экспорт), монтирование — вне write-scope.
