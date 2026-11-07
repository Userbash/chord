# ТЗ — task-279 (`G2 Frontend L1 / P0`: `FE-FIX-03` — Устранение 10 ошибок `tsc` в 5 модулях `ui/features/*.js`)

## Приоритет, Сложность и Экономия токенов
- **Приоритет:** `P0` | **Сложность:** `L1` (атомарная правка типов JSDoc/JS по точным строкам).
- **Модель:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`).
- **Экономия токенов:** НЕ сканируй репозиторий и НЕ читай посторонние файлы! Открой только 5 указанных ниже файлов на указанных строках.

## Разрешённые файлы для записи (Write-Scope)
- `rust/gui-spike/ui/features/attachment-validator.js` (строка 47)
- `rust/gui-spike/ui/features/inbox.js` (строка 83)
- `rust/gui-spike/ui/features/invite-parser.js` (строки 33–34)
- `rust/gui-spike/ui/features/svg-icons.js` (строки 74–104)
- `rust/gui-spike/ui/features/token-map.js` (строка 75)
- `/home/sanya/orca/project/waves/RW-40/G2/task-279-ui-features-tsc-clean/red.log`
- `/home/sanya/orca/project/waves/RW-40/G2/task-279-ui-features-tsc-clean/green.log`
- `/home/sanya/orca/project/waves/RW-40/G2/task-279-ui-features-tsc-clean/REPORT.md`

## Точный список ошибок `tsc` (оракул Red)
```text
rust/gui-spike/ui/features/attachment-validator.js(47,10): error TS2532: Object is possibly 'undefined'.
rust/gui-spike/ui/features/inbox.js(83,26): error TS7053: Element implicitly has an 'any' type because expression of type 'string' can't be used to index type 'Readonly<{ for_you: "Для вас"; unreads: "Непрочитанные"; mentions: "Упоминания"; friend_requests: "Заявки в друзья"; }>'.
rust/gui-spike/ui/features/invite-parser.js(33,25): error TS2345: Argument of type 'string | undefined' is not assignable to parameter of type 'string'.
rust/gui-spike/ui/features/invite-parser.js(34,18): error TS2345: Argument of type 'string | undefined' is not assignable to parameter of type 'string'.
rust/gui-spike/ui/features/svg-icons.js(74,26): error TS2345: Argument of type 'string | undefined' is not assignable to parameter of type 'string'.
rust/gui-spike/ui/features/svg-icons.js(76,3): error TS2739: Type 'HTMLElement' is missing the following properties from type 'SVGElement': ownerSVGElement, viewportElement
rust/gui-spike/ui/features/svg-icons.js(91,19): error TS2339: Property 'hasAttribute' does not exist on type 'Document | Element'.
rust/gui-spike/ui/features/svg-icons.js(91,55): error TS2339: Property 'hasAttribute' does not exist on type 'Document | Element'.
rust/gui-spike/ui/features/svg-icons.js(104,52): error TS2339: Property 'createElement' does not exist on type 'Document | Element'.
rust/gui-spike/ui/features/token-map.js(75,3): error TS2322: Type 'string | undefined' is not assignable to type 'string | null'.
```

## Шаги выполнения (Strict TDD: Red -> Green)
1. **Red:** Запусти `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "attachment-validator\.js|inbox\.js|invite-parser\.js|svg-icons\.js|token-map\.js"` и сохрани вывод (10 ошибок) в `red.log`.
2. **Green:** Исправь типы/фолбэки в этих 5 файлах без изменения бизнес-логики и без `@ts-ignore`.
3. **Проверка:**
   - `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "attachment-validator\.js|inbox\.js|invite-parser\.js|svg-icons\.js|token-map\.js"` — **0 строк** (`green.log`)!
   - `node --test tests/unit/attachment-validator.test.mjs tests/unit/inbox.test.mjs tests/unit/invite-parser.test.mjs tests/unit/svg-icons.test.mjs tests/unit/token-map.test.mjs` — все тесты `PASS`!
4. Запиши краткий `REPORT.md` с `md5sum` 5 файлов и вызови `orca orchestration worker-done`. Не делай `git commit`.
