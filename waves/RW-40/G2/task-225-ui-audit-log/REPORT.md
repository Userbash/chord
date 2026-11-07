# task-225 / FE-P2-24 — журнал аудита: метки действий и дифф изменений

Роль: G2 Frontend. Ветка: `feature/task-225-ui-audit-log` (от `unstable`, HEAD `6118fda`).
Режим: strict TDD (red → green), targeted-тест своего модуля, без `gate:worker`.

## Сделано
- `tests/unit/audit-log.test.mjs` — 13 тестов в 4 сьютах: точные карты кодов/меток,
  `Object.isFrozen` + бросок при мутации, формат записи с целью/без цели, причина с
  обрезкой пробелов, пустая и пробельная причина отбрасывается, фолбэк «Неизвестный»,
  числовой фолбэк на неизвестном действии, дифф с «нет» на `null`/`undefined` и
  сохранением falsy (`false`, `0`, `""`).
- `rust/gui-spike/ui/features/audit-log.js` — `// @ts-check` + JSDoc, чистый модуль
  без DOM/сети: `AUDIT_ACTION_TYPES` (11 кодов, frozen), `AUDIT_ACTION_LABELS_RU`
  (11 русских меток, frozen), `formatAuditLogEntryRu`, `formatAuditChangeDiffRu`.

## Прогоны
- red: `node --test tests/unit/audit-log.test.mjs` → exit 1, `ERR_MODULE_NOT_FOUND`
  (модуль ещё не создан) — `red.log`.
- green: тот же прогон → exit 0, `tests 13 / pass 13 / fail 0 / skipped 0` — `green.log`.
- Targeted-тесты других модулей и полный гейт не запускались (TOKEN-LEAN, T0).

## Файлы и md5
```
8b9f21a9de832d6628f41669c3a9b996  rust/gui-spike/ui/features/audit-log.js
3543b827526978b20638b601e9c2bce9  tests/unit/audit-log.test.mjs
efc1d6ca4916f7528482b3f6d0bcf7f7  red.log
80f8a0509a4420d7bd1cbda5f3b1ae1a  green.log
```

## Риски и заметки
- Неизвестный `actionType` даёт фолбэк `Действие <код>`; контракт ТЗ этот случай
  не задаёт — поведение зафиксировано тестом, при иной продуктовой трактовке
  правка одна строка в `labelFor`.
- Модуль не подключён к рендеру панели аудита: это отдельная карточка монтирования.
- `git commit` не выполнялся; PR в `unstable` за координатором.
