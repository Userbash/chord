# TSC-FIX — 16 ошибок `tsc` в `tests/unit/dispatch_ui.test.mjs`

Дата: 2026-10-24. Воркер: **G4 QA writer**. PM: DeepSeek. Карточка: `TSC-FIX` (P1A).
Вход: `PARITY/MERGE-READINESS-05.md` §0 п.4, `PM/l02-ui/FIX-REPORT.md` §5 п.5,
`tests/unit/dispatch_ui.test.mjs`, корневой `tsconfig.json`, `rust/gui-spike/ui/tsconfig.json`.
Статус: **готово к приёмке**.

`cargo`, `npm run gate:worker`, e2e, live/окно/браузер/CDP/сеть/Discord — **не запускались**.
Продукт (`rust/**`, `src/**`, `scripts/**`, `bin/**`) и чужие тесты — **не менялись**.
Единственная запись в коде — `tests/unit/dispatch_ui.test.mjs`; остальное — логи и этот отчёт.

> **Путь.** Корневых `PARITY/`, `PM/`, `QUALITY/` «в смысле ТЗ» нет — файлы карточек
> лежат в `waves/RW-40/{PARITY,PM,QUALITY}` (это же зафиксировано в
> `PARITY/MERGE-READINESS-05.md`, шапка). Поэтому отчёт и логи записаны в
> `waves/RW-40/QUALITY/`, а не в корневой `QUALITY/` (там только QA-M2-02/04, SEC-L02, G-01b).

## 1. Что сделано

Только **типы** (JSDoc) в тесте: 8 × `TS7006` (неявные `any`), 5 × `TS2339`
(свойства на `{}`), 2 × `TS2345` (`push` в `never[]`), 1 × `TS7006` на `source`.
Тестовые утверждения, данные, оракулы и порядок кейсов не тронуты: assert-строк
**262 → 262**, `it(` **43 → 43**, `.only`/`.skip` — **0**.

| # | Строка (red) | Код | Причина | Правка (тип) |
|---|---|---|---|---|
| 1 | 109:22 | TS7006 | `oracleOrder(initial, …)` — `initial` неявный `any` | `@param {string[]} initial` |
| 2 | 109:31 | TS7006 | `oracleOrder(…, ops)` — `ops` неявный `any` | `@param {any[]} ops` + `@returns {string[]}` |
| 3 | 146:24 | TS7006 | `reactionFrame(action, …)` | `@param {string} action` |
| 4–8 | 148,149,150,151 | TS2339 | `input = {}` → тип `{}`; `channelId/messageId/userId/emojiId/emoji` | `@param {{channelId?: any, messageId?: any, userId?: any, emojiId?: any, emoji?: any}} [input]` |
| 9 | 234:49 | TS7006 | `expected` выведен из `any` (`JSON.parse`), `.map((guild) => …)` | инлайн `@type {any}` на параметре |
| 10–11 | 522:8, 522:15 | TS7006 | `listen` — conditional expression без контекстного типа: `event`/`handler` | JSDoc на `const listen` = тип `listen` из `createDispatchBridge` |
| 12 | 579:25 | TS2345 | `calls: []` в объектном литерале → `never[]`, `this.calls.push(any[])` | `calls: /** @type {any[]} */ ([])` |
| 13 | 601:54 | TS7006 | `setSource(source)` в view D5 | `@param {string} source` |
| 14 | 601:82 | TS2345 | `sources: []` → `never[]`, `this.sources.push(source)` | `sources: /** @type {string[]} */ ([])` |
| 15 | 647:30 | TS7006 | `messagesPagePayload(reactions = [])` | `@param {any[]} [reactions]` |
| 16 | 797:18 | TS7006 | `domDump(doc)` | `@param {GuiDom["document"]} doc` + `@returns {string}` |

Осознанно **не** вводились `@returns {any}` на `reactionFrame`/`messagesPagePayload`:
возвращаемый тип остаётся выведенным (аннотация только на входе), проверка не ослаблена.
`domDump` типизирован реальным типом документа шима (`GuiDom["document"]`), а не `any`.

## 2. RED (до правок)

| Лог | Команда | Результат |
|---|---|---|
| `TSC-FIX-red.log` | `npx tsc -p tsconfig.json` | **exit 1, 16 ошибок** — все в `tests/unit/dispatch_ui.test.mjs` |
| `TSC-FIX-test-before.log` | `node --test tests/unit/dispatch_ui.test.mjs` | **exit 0, 43 passed / 0 failed** — тест был зелёным, красным был только `tsc` |

Красный прогон совпадает с независимым снимком `MERGE-READINESS-05 §0 п.4`
(«exit 1, 16 ошибок, все в `dispatch_ui.test.mjs`»): 8 × TS7006, 5 × TS2339, 2 × TS2345,
1 × TS7006 (см. §1). Продуктовая причина отсутствует — дефект чисто типовой (red-first
здесь = сначала зафиксирован красный `tsc`, затем правка).

## 3. GREEN / targeted

| Проверка | Результат | Лог |
|---|---|---|
| `npx tsc -p tsconfig.json` | **exit 0, 0 ошибок** (16 → 0) | `TSC-FIX-green.log` |
| `node --test tests/unit/dispatch_ui.test.mjs` | **43 passed, 0 failed, 0 skipped** | `TSC-FIX-test-green.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` (sanity, файл не менялся) | exit 0 | — |
| `npm run gate:worker` / `cargo` / e2e / live | **не запускались** — запрещено ТЗ карточки | — |

Проверка, что тесты не ослаблены (независимо от `tsc`):

```
assert.: 262 (HEAD)  vs  262 (now)
it(    :   43 (HEAD) vs   43 (now)
.only/.skip( : 0
```

## 4. Файлы + md5

`rust/gui-spike/**` вне git-базы; `tests/**` — в базе, md5 ниже.

| Файл | md5 было | md5 стало | Статус |
|---|---|---|---|
| `tests/unit/dispatch_ui.test.mjs` | `f2806f3f4088b0527f6a31ff1d7bce2a` | `d30771d65b6373ecbbac6de1f9e61a3d` | изменён (только JSDoc-типы) |
| `tsconfig.json`, `rust/gui-spike/ui/tsconfig.json` | — | — | не тронуты |
| `rust/gui-spike/**`, `src/**`, `scripts/**`, `bin/**`, фикстуры, чужие тесты, доки | — | — | не тронуты |

Артефакты доказательств (md5 на момент отчёта):

| Артефакт | md5 |
|---|---|
| `waves/RW-40/QUALITY/TSC-FIX-red.log` | `b447f10ab58d04afa1eb4bfce738fd0b` |
| `waves/RW-40/QUALITY/TSC-FIX-green.log` | `5f92ec48467ba1498a49596a1e54f56f` |
| `waves/RW-40/QUALITY/TSC-FIX-test-before.log` | `3163b140878dd3f4031e197858fd1b57` |
| `waves/RW-40/QUALITY/TSC-FIX-test-green.log` | `74c55794c3c87af628f5d34b05993521` |
| `waves/RW-40/QUALITY/TSC-FIX.md` (этот отчёт) | `90a6195dc6c6769a888732bdf6608eaa` |

Diff — 9 хунков, все в тесте: добавлены/расширены JSDoc-блоки и 3 инлайн-каста `@type`
(`calls`, `sources`, параметр `guild`); переформатирован литерал view в D5 **без** изменения
поведения. Ни одна строка `assert.*`, ни один кейс не удалён и не изменён.

## 5. Риски / остаток

1. **Гейт в целом остаётся красным** — не из-за `tsc`: `MERGE-READINESS-05 §0 п.4` фиксирует
   второй блокер, `node --test tests/meta/plan-sync.test.mjs` (в `TDD-PLAN.md §8` нет 10
   тестовых файлов). Это **вне scope** `TSC-FIX` и не правилось.
2. **`action` в `reactionFrame` не используется телом** (исторически); параметр сохранён
   как есть, аннотирован `string` — менять сигнатуру хелпера ради типов не стал, чтобы не
   трогать вызовы 10+ кейсов.
3. **`@param {any[]}`** на `ops`/`reactions` намеренно широкий: фикстуры читаются
   `JSON.parse` и не имеют статической схемы; сузить до структур `DispatchPayload` можно
   только вместе с типизацией фикстур (отдельная карточка).
4. **Дерево dirty** (параллельные воркеры RW-40): md5 — снимок на момент отчёта; при
   приёмке сверять повторно.
5. Повторный review/ACCEPT — за PM/Team Lead; воркер приёмку не закрывает.
