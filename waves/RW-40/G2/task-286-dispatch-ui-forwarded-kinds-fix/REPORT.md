# Отчёт — task-286 (`G2 Frontend L1 / P0`, `FE-FIX-08`)

**Ветка:** `bugfix/task-286-dispatch-ui-forwarded-kinds-fix` (от `unstable`, HEAD `e2d9ae1`)
**Карточка:** `FE-FIX-08` — синхронизация `FORWARDED_KINDS` с бэкендом.
**Итог:** `node --test tests/unit/dispatch_ui.test.mjs` — 58/58 pass, `npx tsc -p tsconfig.json --noEmit` — 0 errors.

## Причина падения (не та, что в ТЗ)

`A2` падал на `CHANNEL_CREATE форвардится бэкендом, но отбрасывается UI`. Реальный корень:
`net.rs::FORWARDED_DISPATCHES` расширен до **30** имён ещё в task-156 (`51886e5`), а
`dispatch-model.js::FORWARDED_KINDS` остался на **22** (21 `DispatchKind::as_str` +
`MESSAGE_REACTION_REMOVE`). Не хватало 8 имён `ExtendedDispatchKind::NAMES`:
`MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE`, `CHANNEL_UPDATE`,
`CHANNEL_DELETE`, `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`.
Арифметика `22` внутри теста была устаревшим слепком оракула: `A2` одновременно требует
`FORWARDED_KINDS.length === 22` и «список UI — надмножество `FORWARDED_DISPATCHES` из 30 имён»,
то есть был самопротиворечив. Правится сторона UI + арифметика теста; `net.rs` (G1, вне
write-scope) не тронут.

## Что сделано

1. `rust/gui-spike/ui/features/dispatch-model.js` — `FORWARDED_KINDS` расширен до 30 имён в
   порядке `net.rs::FORWARDED_DISPATCHES`; комментарий описывает состав и то, что у 8
   расширенных событий проекций пока нет (принимаются как `skipped`).
2. `tests/unit/dispatch_ui.test.mjs` — проверка не ослаблена, а усилена:
   - новый парсер оракула `extendedWireNames()` читает `dispatch.rs::ExtendedDispatchKind::NAMES`
     (8 имён) и `forwardedOracle()` собирает полный набор 30;
   - `A2`: точное равенство `FORWARDED_KINDS` оракулу (30, без дублей) **и** обратная проверка
     «`FORWARDED_DISPATCHES` из `net.rs` совпадает с оракулом ядра» (раньше сверялось только
     одностороннее вхождение);
   - `A5`: негативный список обновлён (`MESSAGE_DELETE*` теперь легально форвардятся), вместо
     них проверяются `MESSAGE_DELETE_EXTRA`, `CHANNEL_PINS_UPDATE`, `GUILD_CREATE`,
     `message_create`, `typed`;
   - `B14`: из негативного списка убраны `MESSAGE_DELETE`/`MESSAGE_DELETE_BULK` (они в списке
     бэкенда), добавлен `MESSAGE_DELETE_EXTRA`;
   - новый `B14b`: каждое из 8 расширенных событий принимается моделью — `ok=true`,
     `ignored=0`, `skipped=1`, `changes=[]`, `malformed=0`.
3. Записаны `red.log` (две стадии red) и `green.log` (unit + tsc + регрессионный срез).

## Red (до фикса)

- `red.log` 1/2 — исходное падение: `A2 ... AssertionError: CHANNEL_CREATE форвардится
  бэкендом, но отбрасывается UI`, `tests 57 / pass 56 / fail 1`.
- `red.log` 2/2 — после синхронизации оракула в тесте, прод ещё на 22:
  `A2 ... AssertionError: в UI не 30 kind`, `B14b ... MESSAGE_DELETE форвардится бэкендом,
  но отбрасывается UI`, `tests 58 / pass 56 / fail 2`.

## Green (после фикса)

- `green.log`: `node --test tests/unit/dispatch_ui.test.mjs` → `tests 58 / pass 58 / fail 0`,
  `skipped 0`.
- `npx tsc -p tsconfig.json --noEmit` → `0 errors`, exit 0.
- Регрессионный срез из `scripts/test-impact.mjs` (frontend-контракт: `gui-spike-dom`,
  `gui-spike-model`, `gui-spike-bench`, `gui-spike-features`, `gui-spike-auth`,
  `gui-spike-regressions`, `guild-rail`, `voice-ui`, `channel-switch`, `dispatch_ui`) →
  `tests 458 / pass 458 / fail 0`.

## Файлы и md5

| Файл | md5 |
| --- | --- |
| `rust/gui-spike/ui/features/dispatch-model.js` | `9c7439d1db386a89fbd2347a95a207a2` |
| `tests/unit/dispatch_ui.test.mjs` | `ac92c85adfbf146d812921a176a19ced` |

## Риски и замечания

- `scripts/test-impact.mjs` поднял уровень до **T2**, потому что в рабочем дереве уже были
  чужие изменения: удалённый симлинк `node_modules` и правка `package-lock.json` (не мои, не
  трогал). Из-за этого полный `npm test` / `cargo test --workspace` не запускался — только
  адресный срез, который гейт указывает для `dispatch-model.js` и теста.
- `tests/meta/plan-sync.test.mjs` падает до моих правок и независимо от них: в репозитории
  нет `PLAN.md` (`git ls-files PLAN.md` → пусто), поэтому все тестовые файлы «без строки в §8».
  Не чинил: `PLAN.md` §8 — зона Team Lead.
- 8 расширенных событий пока без проекций в UI (`skipped`). Их отображение (`MESSAGE_DELETE*`,
  `CHANNEL_*`, `THREAD_*`) — отдельная задача; здесь закрыт только фильтр, который их выбрасывал.
- `git commit` не делал.
