# task-268 (FE-INT-11) — Unified Channel & Composer Helpers Bridge

Воркер G2 Frontend Integration, token-lean. HEAD базы: `b196aea` (`unstable`).
Ветка: `feature/task-268-ui-chat-helpers-bridge`.

## Что сделано

Строгий TDD: сначала тест (red на `ERR_MODULE_NOT_FOUND`), затем реализация.

| Файл | Что внутри | md5 |
| --- | --- | --- |
| `rust/gui-spike/ui/features/chat-helpers-bridge.js` | Фасад `buildChannelHeaderBadgesModel(options)` + `formatChannelTopicTooltip(topic, maxChars = 160)`. Переиспользует `formatUnreadCountBadge`, `formatThreadStatusSummaryRu` из `thread-badge.js` и `resolveBoostTierFromCount`, `BOOST_TIER_THRESHOLDS` из `guild-boost.js`. Своего дублирования логики нет. | `972ee85338329e2cb98ff3d909bd67cc` |
| `tests/unit/chat-helpers-bridge.test.mjs` | 11 тестов: форма/заморозка модели, дефолты, бейдж непрочитанных (`0`/`5`/`99`/`150`), все четыре срока автоархивации + неизвестный срок, флаги `locked`/`archived`, уровни бустов `0/1/2/6/7/13/14/500`, проценты прогресса, тултип темы (не-строка, пустая, короткая, длинная, `maxChars=1` и `2`). | `e3daee06110c901214ff91d6978fe814` |

## Решения по контракту

- `threadDurationLabel` берётся из `formatThreadStatusSummaryRu({ autoArchiveDuration, archived, locked }).archiveLabel`. Так неизвестный срок даёт `24 часа` (нормализация внутри `thread-badge.js`), а не `undefined`.
- `boostProgressPercent` — прогресс до **следующего** уровня: база `0 -> TIER_1(2)`, `1 -> TIER_2(7)`, `2 -> TIER_3(14)`, `Math.round(count / порог * 100)`, зажим `0..100`. На уровне 3 всегда `100`. Примеры: `1 -> 50`, `2 -> 29`, `6 -> 86`, `13 -> 93`.
- `formatChannelTopicTooltip`: не-строка и пустая после `trim()` → `""`; `trim().length <= maxChars` → `trim()`; иначе `trim().slice(0, Math.max(0, maxChars - 1)) + "…"`. Считается уже обрезанная тема, поэтому пробелы по краям не съедают лимит. `maxChars=1` даёт `"…"`.
- Модель возвращается через `Object.freeze` с ровно шестью полями контракта.

## Red

```
node --test tests/unit/chat-helpers-bridge.test.mjs > waves/RW-40/G2/task-268-.../red.log 2>&1
exit=1
code: 'ERR_MODULE_NOT_FOUND'
url: 'file://.../rust/gui-spike/ui/features/chat-helpers-bridge.js'
ℹ pass 0  ℹ fail 1
```

## Green

```
node --test tests/unit/chat-helpers-bridge.test.mjs > .../green.log 2>&1
exit=0
ℹ tests 11  ℹ pass 11  ℹ fail 0
```

`npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → exit `0` (0 ошибок).

Дополнительно: прямой `tsc` по модулю (`--ignoreConfig --allowJs --checkJs --strict ...`) → 0 ошибок.
Один TS2322 в `thread-badge.js:75` всплывает только при `--noUncheckedIndexedAccess` и существует
до этой задачи (файл не трогался, tsconfig UI проверяет лишь `app.js`), поэтому в scope не правится.

Проверка соседних модулей на регрессию: `node --test tests/unit/guild-boost.test.mjs tests/unit/threads-drawer.test.mjs` → 19/19 pass.

## Риски и что осталось

- В `tsconfig.json` UI `include` — только `app.js`, поэтому новые feature-модули не попадают в проектный
  `tsc`, пока их не импортирует `app.js`. Это отдельное решение Team Lead / G2, вне Write-Scope карточки.
- Модуль пока не подключён к рендеру заголовка: монтирование — задача другого слайса, здесь только чистый фасад.
- Нет `git commit` (по ТЗ); гейт `gate:worker` (T1) выполняется в integration-лейне.

## Артефакты

- `waves/RW-40/G2/task-268-ui-chat-helpers-bridge/red.log`
- `waves/RW-40/G2/task-268-ui-chat-helpers-bridge/green.log`
- `waves/RW-40/G2/task-268-ui-chat-helpers-bridge/REPORT.md`
