# Отчёт — task-279 (`G2 Frontend L1`, `FE-FIX-03`)

Устранение 10 ошибок `tsc` в 5 модулях `rust/gui-spike/ui/features/*.js`.

- **HEAD:** `355be21` (ветка `bugfix/task-279-ui-features-tsc-clean` от `unstable`)
- **Окружение:** node `v26.13.0`, `tsc` `7.0.2`, `tsconfig.json` (`strict`, `noUncheckedIndexedAccess`, `checkJs`, `allowJs`)
- **Коммит:** не делался (запрещено ТЗ; слияние — за координатором)

## Red

`npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "attachment-validator\.js|inbox\.js|invite-parser\.js|svg-icons\.js|token-map\.js"` дал ровно **10 строк** (`red.log`, 10/10 совпали с оракулом ТЗ):

| Файл:строка | Код | Причина |
| --- | --- | --- |
| `attachment-validator.js:47` | TS2532 | `split(';')[0]` даёт `string \| undefined` при `noUncheckedIndexedAccess` |
| `inbox.js:83` | TS7053 | `INBOX_TAB_LABELS` выведен как `Readonly<{for_you, unreads, ...}>`, индексация строкой `tab` запрещена |
| `invite-parser.js:33` | TS2345 | `match[1]` — `string \| undefined` в `Array.prototype.includes` |
| `invite-parser.js:34` | TS2345 | тот же `match[1]` в `codes.push` |
| `svg-icons.js:74` | TS2345 | `SVG_ICONS[name]` — `string \| undefined` в `setAttribute` (проверка `hasOwnProperty` не сужает индекс) |
| `svg-icons.js:76` | TS2739 | `doc.createElement("svg")` в lib.dom `tsc 7` типизирован как `HTMLElement`, а функция объявляет `SVGElement` |
| `svg-icons.js:91` | TS2339 ×2 | `root.hasAttribute` на `Document \| Element` — у `Document` метода нет |
| `svg-icons.js:104` | TS2339 | `root.createElement` на `Document \| Element` — та же причина |
| `token-map.js:75` | TS2322 | `CANONICAL_TOKEN_MAP[localToken]` — `string \| undefined` в возврат `string \| null` |

## Green

Правки — только типы и статические фолбэки; ветвления бизнес-логики, тексты и публичные сигнатуры не менялись, `@ts-ignore`/`@ts-expect-error` не добавлялись.

1. `attachment-validator.js:46-48` — базовый MIME вынесен в `const base = ...split(';')[0] ?? ''` (тот же приём, что уже был в `extensionOf`), затем `base.trim().toLowerCase()`.
2. `inbox.js:19-23` — `INBOX_TAB_LABELS` размечен `@type {Readonly<Record<string, string>>}`: ключи вкладок — строки, а не литеральный объект.
3. `inbox.js:86` — `button.textContent = INBOX_TAB_LABELS[tab] ?? tab`; фолбэк недостижим, так как `tab` приходит из `INBOX_TABS`, чьи значения совпадают с ключами подписей.
4. `invite-parser.js:33` — условие стало `typeof code === 'string' && !codes.includes(code)`: группа 1 у `INVITE_PATTERN` жадная `[A-Za-z0-9-]{2,32}`, поэтому при совпадении она всегда есть, поведение прежнее.
5. `svg-icons.js:58-59` — `const pathData = SVG_ICONS[name]; if (pathData === undefined) return null;` (после `hasOwnProperty` ветка недостижима), `setAttribute("d", pathData)`.
6. `svg-icons.js:65-66` — создание корня: `/** @type {SVGElement} */ (/** @type {unknown} */ (doc.createElement("svg")))` с комментарием про расхождение lib.dom `tsc 7`; в рантайме возвращается настоящий `SVGSVGElement`.
7. `svg-icons.js:94` — `"hasAttribute" in root && typeof root.hasAttribute === "function" && root.hasAttribute("data-icon")`: `in` сужает объединение до `Element`, проверка `typeof` сохранена как была.
8. `svg-icons.js:107` — `host.ownerDocument ?? ("createElement" in root ? /** @type {Document} */ (root) : null)`; отсутствие `createElement`-функции по-прежнему отсекается внутри `createIconSvg` (`typeof doc.createElement !== "function"` → `null`).
9. `token-map.js:75` — `return CANONICAL_TOKEN_MAP[localToken] ?? null`; `hasOwn` выше гарантирует попадание, `null` — контракт функции для неизвестного токена.

### Проверки

```
$ npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E 'attachment-validator\.js|inbox\.js|invite-parser\.js|svg-icons\.js|token-map\.js' | wc -l
0                       # green.log пуст (0 строк)

$ node --test tests/unit/attachment-validator.test.mjs tests/unit/inbox.test.mjs \
    tests/unit/invite-parser.test.mjs tests/unit/svg-icons.test.mjs tests/unit/token-map.test.mjs
ℹ tests 83   ℹ pass 83   ℹ fail 0   ℹ skipped 0
```

Дополнительно: `diff` полного списка ошибок до/после правки вне 5 файлов — `NO_NEW_ERRORS`, новых ошибок в других модулях не появилось.

## Остаток вне Write-Scope

`tsc -p tsconfig.json` по-прежнему даёт **139** ошибок в чужих файлах (`tests/unit/*`): `profile-modal.test.mjs` (29), `svg-icons.test.mjs` (27), `appearance-settings.test.mjs` (22), `channel-tree.test.mjs` (17), `locale-switcher.test.mjs` (9), `inbox.test.mjs` (6), `embed-formatter.test.mjs` (6) и мелочь по остальным. Эти файлы не трогал — они в карточке `task-280-unit-tsc-fix-batch1` и следующих `FE-FIX-*`.

`git status` до начала работы уже показывал ` D node_modules` и ` M package-lock.json` — фоновые изменения worktree, мной не вносились.

## md5sum (5 файлов)

```
dec89713e4893572f6dba02f8dd2a4c9  rust/gui-spike/ui/features/attachment-validator.js
e1d84a89972594d8ed7c1b7b383ac567  rust/gui-spike/ui/features/inbox.js
1835d4d322d9be4772998494390a7e7b  rust/gui-spike/ui/features/invite-parser.js
4790e8f94cda67280aa4b1befc095d24  rust/gui-spike/ui/features/svg-icons.js
1781779a33804706b07d550199549fc8  rust/gui-spike/ui/features/token-map.js
```

## Риски

- `inbox.js`: фолбэк `?? tab` покажет технический ключ вкладки, если подпись когда-нибудь удалят из `INBOX_TAB_LABELS`. Ветка недостижима при текущих данных (`INBOX_TABS` и ключи словаря совпадают), но дрейф не будет заметен тестом.
- `svg-icons.js`: `hasOwnProperty` + `pathData === undefined` — двойная защита; вторая ветка не исполняется и не покрыта тестом (индекс уже проверен выше).
- Двойной каст `unknown → SVGElement` для `createElement("svg")` маскирует реальный тип элемента. Причина — lib.dom `tsc 7.0.2` больше не отдаёт `SVGSVGElement` из этой перегрузки; в браузере и в тестах `tagName === "SVG"` подтверждается (`svg-icons.test.mjs`).
- `"createElement" in root` в `upgradeGlyphIcons` заменяет прежнюю проверку `typeof`. Если в `root` окажется не-функция `createElement`, вызов не произойдёт: `createIconSvg` вернёт `null` на первой проверке. Для `hasAttribute` проверка `typeof` сохранена полностью.
