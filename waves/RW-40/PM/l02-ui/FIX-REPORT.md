# L-02-UI-FIX — закрытие P2 из REVIEW-L02-UI (red-first)

Дата: 2026-10-24. Воркер: **G2 Frontend**. PM: DeepSeek. Карточка: `L-02-UI-FIX` (P1A).
Вход: `REVIEW.md` (две P2), `PLAN.md`, `REPORT.md`. Статус: **готово к повторному review**.
`cargo`, `npm run gate:worker`, live/окно/браузер/сеть не запускались.

## 1. Что закрыто

### P2-1 — пустой READY оставляет устаревшие плитки в рейле

| Где | Было | Стало |
|---|---|---|
| `dispatch-model.js::projectReady` | `if (guilds.length === 0) return []` + маркер «снапшот применён» отсутствовал | пустой снапшот **всегда** даёт `{type:"guilds",guilds:[]}`, кроме настоящего повтора; `store.source="dispatch"` ставится и для пустого READY |
| `dispatch-model.js::projectReady` (`same`) | повтор определялся только по store; на чистом store первый пустой READY считался duplicate | duplicate требует `store.source === "dispatch"`, т.е. что снапшот уже доходил до view |
| `app.js::rail.applyGuilds` | `items.length === 0` → ранний `return`, рейл не очищался | пустой массив — валидный полный снапшот: `renderEmpty()` (плитки удалены, `#guild-rail .guild-empty` «Нет серверов», `.guild-add` сохранён), `onSelect`/фокус не трогаются |

Покрыты оба реальных сценария: (а) непустой READY → пустой READY; (б) **первый** пустой READY
после boot, когда рейл наполнен `ready()` IPC (именно этот случай прежний фикс пропускал).

### P2-2 — числовой self ID терял точность

| Где | Было | Стало |
|---|---|---|
| `dispatch-model.js` | — | новый экспорт `asSnowflake(value)`: строка — как есть; число — только `Number.isSafeInteger`; unsafe integer → `""` (точность уже потеряна при разборе JSON) |
| `dispatch-model.js::setDispatchSelfUserId` | `asString(value)` (`String()` для любого числа) | `asSnowflake(value)` |
| `dispatch-model.js::projectReady` (fallback `d.user.id`) | `asString` | `asSnowflake` |
| `app.js::invoke("ready")` | `typeof id === "string" \|\| "number"` → `String(userId)` для typing и dispatch | один `asSnowflake(result?.user?.id)`; при `""` значение не применяется (и typing, и dispatch) |

Следствие отклонения: `store.selfUserId` остаётся пустым, `me` у реакций не выставляется
(ложный «свой» счётчик исключён), UI продолжает работать. Это осознанная деградация, а не
стрингование вслепую; комментарий в коде фиксирует причину (AGENTS: не глотать молча).

## 2. RED (до фикса)

Оба прогона — до продуктовых правок, падение на целевых assert, а не на отсутствии модулей.

| Лог | Команда | Результат |
|---|---|---|
| `fix-red.log` | `node --test tests/unit/dispatch_ui.test.mjs` | exit 1, **4 падения**: B16 «пустой READY не применён как полный снапшот», B17 «unsafe integer превращён в строку вслепую», C15 «пустой READY не применён как полный снапшот», C16 «unsafe integer превращён в строку вслепую»; 37 старых зелёные |
| `fix-red2.log` | то же после добавления кейсов «первый пустой READY» | exit 1, **2 падения**: B16b «источник не отмечен», C15b «первый пустой READY проглочен как duplicate»; остальные 41 зелёные |

## 3. GREEN / targeted

| Проверка | Результат | Лог |
|---|---|---|
| `node --test tests/unit/dispatch_ui.test.mjs` | **43 passed, 0 failed** (было 37; +B16, B16b, B17, C15, C15b, C16) | `fix-green.log` |
| targeted UI (guild-rail, chat-reactions, channel-switch, typing-ui, read-state-ui, gui-spike-{features,dom,regressions,bench,auth}, join-server, voice-ui) | **347 passed, 0 failed** | `fix-targeted-ui.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | чисто (exit 0) | `fix-tsc.log` |
| `npm run gate:worker` / `cargo` / live | **не запускались** — по ТЗ карточки (integration-лейн / окно PM) | — |

Новые кейсы и их оракулы:

| # | Кейс | Утверждение |
|---|---|---|
| B16 | непустой READY → пустой READY | 1 `guilds`-change с `[]`; store/order очищены; повтор пустого → `duplicate`, changes `[]` |
| B16b | первый пустой READY на чистом store | change с `[]` (не duplicate), `store.source="dispatch"` |
| B17 | self ID | `String(unsafe) !== snowflake` (оракул потери точности); unsafe number → `""`; safe `42` → `"42"`; string snowflake без потерь; READY-fallback не стринговал unsafe |
| C15 | непустой → пустой READY в рейле | 0 плиток `[data-guild-id]`, `.guild-empty` «Нет серверов», `.guild-add` на месте, 0 новых `ready/channels/messages_page` |
| C15b | первый пустой READY после boot | плитки от `ready()` IPC удалены; повтор пустого → `duplicate`, рейл не пересобран |
| C16 | `ready()` IPC self ID | unsafe number → `store.selfUserId === ""`, `me` реакции `false`; string snowflake сохранён точно |

## 4. Файлы + md5

`rust/gui-spike/**` вне git-базы; md5 — слепок изменения.

| Файл | md5 | Статус |
|---|---|---|
| `rust/gui-spike/ui/features/dispatch-model.js` | `37dacc61d7daf74c144681637dded1f5` | изменён |
| `rust/gui-spike/ui/app.js` | `1ac5c36976a064120529c132e61cfb54` | изменён |
| `tests/unit/dispatch_ui.test.mjs` | `f2806f3f4088b0527f6a31ff1d7bce2a` | изменён |
| `rust/gui-spike/ui/features/dispatch-view.js` | `fd1842ac442dae108f80b3872f163f05` | не тронут (md5 = REPORT.md) |
| `rust/gui-spike/ui/features/dispatch-bridge.js` | `1cad6f1324b2fe508866d29097792443` | не тронут |
| `rust/gui-spike/ui/features/chat.js` | `2e918f3cff5dc968ca3117480e306ce4` | не тронут |
| `rust/gui-spike/ui/features/members.js` | `98801ba746103791f06234d08d2a5ae7` | не тронут |
| `rust/gui-spike/ui/features/reactions.js` | `ef8f63e02c586bd2c9f43016a92b6b40` | не тронут |
| `tests/support/gui-dom-harness.mjs` | `8e26df0623a00504fc92e3ed7b01ead8` | не тронут |
| `index.html`, CSS, Rust, чужие тесты, фикстуры | — | не тронуты |

В `dispatch_ui.test.mjs` нет `.only`/`.skip`/ослаблений; новые assert строже прежних.

## 5. Остаток / что осознанно не входило

1. **Прочие ID в событиях** (`member.user.id`, `presence.user.id`, `reaction.user_id`)
   по-прежнему проходят через `asString` (PLAN §2). P2 REVIEW касался только self ID из
   `ready()`; расширение strict-правила на все ID — отдельное решение контракта (может
   отбросить валидные кадры с числовыми id, если ядро когда-нибудь так отдаст).
2. **Отклонённый unsafe self ID** не имеет отдельного счётчика: имена 15 счётчиков bridge
   заморожены (§9.1), поэтому наблюдаемое последствие — пустой `selfUserId` и `me=false`.
   Если нужен счётчик/баннер — расширение контракта PM.
3. **Пустой READY не очищает live-панель участников** (в P2 речь только о рейле). Панель
   принадлежит выбранной гильдии и обновляется её LIST_UPDATE/CHUNK; полный сброс панели —
   вопрос MB-01/отдельной карточки.
4. **`store.source="dispatch"`** теперь ставится и на пустом READY: это маркер «снапшот
   доходил до view» (нужен для идемпотентности). Потребителей, ветвящихся по этому полю,
   в UI нет; bridge-`source` (`live/absent/unavailable`) не менялся.
5. **`npx tsc -p tsconfig.json` (корневой)** по-прежнему показывает 16 прежних ошибок в
   `tests/unit/dispatch_ui.test.mjs` (неявные `any` в старых хелперах `oracleOrder`/
   `reactionFrame`/`domDump`); новых строк в списке нет, UI-`tsconfig` чистый.
6. **Повторный Codex review** — следующий шаг; приёмка за Team Lead/PM.
