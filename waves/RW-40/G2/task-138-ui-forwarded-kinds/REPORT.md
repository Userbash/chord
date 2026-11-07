# task-138 / BE-P1-02-UI — FORWARDED_KINDS: UI принимает все канонические типы ядра

- **batch/карточка:** RW-40 / BE-P1-02-UI (task-138; продолжение находки F-GW-03 и
  отчёта `waves/RW-40/G1/task-136-net-forwarded-dispatches/REPORT.md`, риск №3 —
  «UI-фильтр всё ещё 6 имён»).
- **worktree:** `/home/sanya/orca/workspaces/project/feature-task-138-ui-forwarded-kinds`
  (HEAD `4e24048`).
- **файлы (write-scope):**
  - `rust/gui-spike/ui/features/dispatch-model.js`;
  - `tests/unit/dispatch_ui.test.mjs` — профильный unit-тест модели (dispatch-model.js
    тестируется только здесь, `gui-spike-model.test.mjs` про другое);
  - этот отчёт и логи рядом.

## Сделано

1. **`FORWARDED_KINDS`: 6 → 22.** Порядок — как в `DispatchKind::ALL` / `as_str`
   (21 имя), `MESSAGE_REACTION_REMOVE` в конце (raw в ядре, форвардится по имени
   провода — `CONTRACTS/dispatch.md` §5). Список совпадает с расширенным
   `net::FORWARDED_DISPATCHES` из task-136; в UI он остаётся надмножеством бэкенда.
2. **Проекции отделены от приёма.** Раньше `switch` имел `default: projectReaction`,
   поэтому любое новое имя молча уехало бы в проекцию реакций. Теперь явные ветки
   для 6 kinds с проекцией, а канонические без проекции (CHANNEL_INFO, MESSAGE_CREATE/
   UPDATE/ACK, TYPING_START, VOICE_*, THREAD_LIST_SYNC, READY_SUPPLEMENTAL, …)
   считаются в `skipped`: `ok: true`, изменений нет, без throw и console.
   `ignored` остался только для `kind` вне `FORWARDED_KINDS`.
3. **Конверт принимает массивный `d`.** `SESSIONS_REPLACE.d` — массив
   (`docs/re/gateway-schema.md`: «d — массив, не объект»), а `normalizeEnvelope`
   отбрасывал массивы как `reason: "data"`. Без правки одно из 22 имён падало бы в
   `malformed` ещё до фильтра. Теперь массив — валидный конверт; проекции сами
   требуют объект и по-прежнему дают `malformed`.
4. **Тест-оракул — ядро, а не текущий код.** A2 читает `impl DispatchKind::as_str`
   из `rust/crates/discord-core/src/dispatch.rs` (21 имя), добавляет
   `MESSAGE_REACTION_REMOVE`, проверяет 22 без дублей и требует, чтобы каждый name
   из `net.rs::FORWARDED_DISPATCHES` входил в UI-список (работает и до, и после
   интеграции task-136). A4 гоняет все 22 имени оракула через `applyDispatch` с
   правдоподобным `d`: `malformed 0`, `ignored 0`, у 6 с проекцией — изменения и
   `applied 1`, у остальных — пустые `changes` и `skipped 1`. A5 фиксирует, что имена
   вне ядра (`MESSAGE_DELETE`, `GUILD_CREATE`, `message_create`, …) остаются за
   фильтром.
5. **Обновлены зависимые кейсы:** B13 (malformed-таблица — только 6 kinds с
   проекцией), B14 (неизвестные имена вместо бывших «не форвардимых»), C10
   (`ignored 1` + `skipped 2`), C14 (`ignored 0` + `skipped 1`).
6. **Typecheck приведён к чистому.** В `tests/unit/dispatch_ui.test.mjs` было 16
   ошибок `tsc` на HEAD (JSDoc у `oracleOrder`, `reactionFrame`, `messagesPagePayload`,
   `domDump`, `makeBridge`, литералы `[]` в фикстурах view). Все закрыты в моём файле;
   `npx tsc -p tsconfig.json` и `npx tsc -p rust/gui-spike/ui/tsconfig.json` — exit 0.

## red (до реализации)

`node --test tests/unit/dispatch_ui.test.mjs` → exit 1, лог `red-dispatch-ui.log`:

- `A2:` «в UI не 22 kind» — `6 !== 22`;
- `A4:` `CHANNEL_INFO отброшен фильтром FORWARDED_KINDS` (то же для остальных 15
  новых имён);
- `C10:` `ignored 3 !== 1` — TYPING_START/MESSAGE_CREATE считались отброшенными;
- `C14:` `TYPING_START отброшен фильтром вместо skipped`;
- остальные 41 кейс зелёные (проекции 6 kinds не затронуты).

## green

- `node --test tests/unit/dispatch_ui.test.mjs` → 45 passed, 0 failed, exit 0
  (`green-dispatch-ui.log`).
- `node --test 'tests/unit/**/*.test.mjs'` → 704 passed, 0 failed, 0 skipped, exit 0
  (`green-unit-all.log`).
- `npx tsc -p tsconfig.json` → exit 0 (`tsc-root.log`, файл пуст).
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0 (`tsc-ui.log`, файл пуст).

## targeted / gate

- Targeted — весь профильный файл `tests/unit/dispatch_ui.test.mjs` (45/45).
- Широкий прогон unit — 704/704.
- `gate:worker` целиком не запускался: по процессу RW-40 полный гейт делает
  integration/QA на batch; мои изменения не выходят за UI-модель и её тест.
- `tests/meta/plan-sync.test.mjs` падает и на HEAD без моих правок (в §8 не описаны
  `tests/e2e/m2-*.e2e.test.mjs` и др.) — сверено `git stash`, это чужой участок
  `PLAN.md`/§8, к карточке не относится. Лог: `meta-all.log` (101/102).

## Файлы + md5

| файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/dispatch-model.js` | `b4de6d0e36ab2fc590617ad5fe33d826` |
| `tests/unit/dispatch_ui.test.mjs` | `5167317dee6088678d3e2990d99aa973` |

Логи: `red-dispatch-ui.log`, `green-dispatch-ui.log`, `green-unit-all.log`,
`tsc-root.log`, `tsc-ui.log`, `meta-all.log`.

## Риски / вопросы

1. **`CONTRACTS/dispatch.md` §4 по-прежнему описывает 6 имён** и `waves/RW-40/PM/
   l02-ui/PLAN.md` §3.6 говорит «остальные 15 не форвардятся, `ignored`». Обещание
   кода теперь другое: 22 имени приняты, без проекции — `skipped`. Доки — не мой
   write-scope, нужна правка Team Lead.
2. **Семантика `skipped` расширена**: теперь это и «kind принят, проекции нет», и
   «элемент payload отброшен» (участник без `user.id`, групповой элемент). `ignored`
   сузился до «kind вне списка». `dispatch-bridge.js` не менялся: `skipped` уже в
   `MODEL_COUNTERS`, снапшот `dispatch.stats` показывает рост без правок транспорта.
3. **`normalizeEnvelope` теперь принимает массив как `data`** (иначе SESSIONS_REPLACE
   — `malformed`). Примитивы (`"x"`, `7`, `null`) по-прежнему `reason: "data"`.
   Это изменение конверт-контракта, его стоит зафиксировать в `CONTRACTS/dispatch.md`
   §3 и в A3 PLAN.
4. **В этом worktree `rust/gui-spike/src/net.rs` ещё старый (6 имён)** — task-136
   живёт в отдельном worktree. Фильтр бэкенда независим: пока он не расширен, новые
   события не дойдут до UI физически; после интеграции task-136 список совпадёт.
   Тест A2 сформулирован так, чтобы зеленеть в обоих состояниях (UI ⊇ net.rs).
5. **Проекции для новых типов не делались** — карточка про приём, не про отрисовку
   MESSAGE_CREATE/TYPING_START/VOICE_*. Сообщения, typing, голос и треды пока просто
   доходят до модели и считаются в `skipped`; отрисовка — отдельные карточки.
6. **16 ошибок `tsc` в тесте были на HEAD**; я их закрыл в своём файле. Если Team
   Lead не хочет расширять дифф теста типами — это отделяемый кусок (JSDoc-правки),
   логика тестов от него не зависит.
