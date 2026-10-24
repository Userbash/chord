# REPORT W39h3 — Frontend fix (P0/P1/P2 Codex)

Воркер: W39h3. PM: DeepSeek. База: `waves/RW-39/PM/BRIEF-W39h3.md`,
`waves/RW-39/PM/w39h/REVIEW-codex.md`. HEAD: `0239543`.
Red-first, без окна/probe/cargo/gate/сети/commit.

## Итог

| # | Дефект (Codex) | Статус | Файлы |
|---|---|---|---|
| P0 | `linkEmbed` копировал `embed.url` в `href` (`javascript:` исполнялся) | fixed | `ui/features/embeds.js` |
| P1 | `data-action="gift"` без handler'а и без `aria-disabled` → `ACTION_FAILED` | fixed | `ui/features/actions.js`, `ui/index.html` |
| P2 | «По новизне» и «Самое актуальное» сортировали одинаково | fixed | `ui/features/search-overlay.js` |

## P0 — URL в link-embed

**Red** (`red-features.log`, тест «link-embed: в href только http(s)…»):
```
AssertionError: небезопасный URL попал в href: "javascript:alert(document.domain)"
```
**Fix**: `safeHref()` — `new URL()` + белый список `http:`/`https:`. Безопасный адрес → `<a.embed-title>` с `href` и `rel="noreferrer noopener"`; небезопасный/относительный/пустой → `<span.embed-title>` без `href`, `aria-disabled="true"` + `title`-подпись (текст остаётся текстом, не навигацией).
**Green**: тест покрывает `https://…`, `javascript:`, `JavaScript:`, `data:`, `vbscript:` и пустой `url`; `https:` сохранён в `href`, `rel` на месте.

## P1 — кнопка «Подарок»

**Red** (`red-features.log`):
```
AssertionError: gift не помечен aria-disabled   null !== 'true'
```
(и до фикса клик шёл в `неизвестный data-action="gift"` → `reportError(ACTION_FAILED)`).
**Fix**: выбран разрешённый брифом вариант «`aria-disabled` + подпись» (как у `inbox`/`attach`):
- `actions.js`: handler `gift` (no-op с явным комментарием Phase 3) + в init выставляются `aria-disabled="true"` и `title="Подарок (Phase 3): недоступно в спайке"` — работает и в браузере, и в DOM-шиме;
- `index.html`: те же два атрибута статически (только атрибуты gift).
**Green**: клик по gift дважды — `consoleErrors`/`consoleWarns` пусты, `aria-disabled` остаётся `true`, доступное имя есть.
**Доп. регрессия** (dom-тест, `red-dom-gift.log` → green): статика `index.html` и шим совпадают по `aria-disabled`/`title`/`aria-label`. Red снят на временно откатанной разметке, файл восстановлен (md5 `653282056da39a0c84eeae84c7d54eb1`).

## P2 — сортировка «Самое актуальное»

**Red** (`red-features.log`):
```
AssertionError: «Самое актуальное» не подняло закреплённое: первым 99999   false !== true
```
**Fix**: `sorted()` для `recent`: сначала закреплённые (`store.getRow(index).pinned`), внутри групп — по убыванию индекса (свежие сверху). `new` остался чисто по времени (убывание индекса), `old` — по возрастанию.
**Green**: тест грузит окно с закреплённой строкой `#99893` (`99893 % 523 === 0`), проверяет: `new` строго по времени; `recent` первым даёт закреплённое, набор совпадений тот же, незакреплённые — в том же порядке, что и в `new`; `recent[0] !== new[0]`.

## Тесты

- Было: **125**, стало **129** (+4: 3 в `gui-spike-features.test.mjs`, 1 в `gui-spike-dom.test.mjs`); ни один старый тест не изменён/не ослаблен (правки только аддитивные).
- `node --test tests/unit/gui-spike-{dom,model,bench,features}.test.mjs` → **129 pass / 0 fail** (`green-all.log`).
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0 (`tsc.log`).
- `node waves/RW-39/PM/w39c/geometry-check.mjs` → 31/31 ok (`geometry.log`).
- Окно/probe/cargo/gate/сеть/commit не запускались.

## Файлы и md5

```
1aa9e460930b894bfe94b50cff6af0be  rust/gui-spike/ui/features/embeds.js
ccee1717b5ffade8bb75bb6cf655d0df  rust/gui-spike/ui/features/actions.js
6c649aad0a44f10de4859d04194aeb7a  rust/gui-spike/ui/features/search-overlay.js
653282056da39a0c84eeae84c7d54eb1  rust/gui-spike/ui/index.html
5a5e1073aff08fca7e5e3bc3f0fa6df9  tests/unit/gui-spike-features.test.mjs
18e0b6ff2c70705360248a0c2acccc51  tests/unit/gui-spike-dom.test.mjs
```
Логи: `red-features.log`, `red-dom-gift.log`, `green-features.log`, `green-all.log`, `tsc.log`, `geometry.log`.

## Остаток / риски

- P1 закрыт вариантом «disabled-affordance», а не интерактивной заглушкой Phase 3: клик осознанно не меняет состояние (по брифу допустимо). Если PM хочет кликабельный поповер — отдельная карточка.
- `safeHref` отклоняет относительные URL (в демо-данных только absolute `https:`, поведение честное: без навигации).
- «Самое актуальное» трактует «актуальность» как pinned-first + свежесть; иной приоритет (например, активность/ответы) в демо-данных не поддержан.
- `app.js`/шим/общие файлы не трогались; `ui/**` вне списка — read-only, соблюдено.
