# G-03a-UI — «Открыть сервер» в preview приглашения (lurker)

Карточка: **G-03a-UI** (backend G-03a принят D-76; G-03 UI принят D-122).
Воркер: G2 Frontend. Дата: 2026-10-24. Live/окно/сеть/cargo/gate/commit — не запускались.

## Сделано

- `rust/gui-spike/ui/features/join-server.js`
  - `JOIN_COMMANDS.enter = "join_guild"`, `JOIN_TEXT.open/opened`.
  - В preview добавлена кнопка «Открыть сервер» (`data-action="enter-guild"`),
    только при непустом `guildId` из ответа `resolve_invite`; ничего не выдумывается.
  - Новый шаг `enterStep`: `invoke("join_guild", { guildId })`, состояние
    `preview_ready → entering → entered | error`, `aria-busy` на время запроса.
  - Успех (204, тело не читается) → `data-state="entered"`, статус
    «Сервер открыт для просмотра» в общем live-регионе `#server-status`.
    Плитка в рейл **не** добавляется, `accept_invite` не вызывается.
  - Ошибки 401/403/404/429/5xx/сеть/parse/storage — видимый текст в `#join-error`;
    retry повторяет только упавший `join_guild` (resolve/accept не дублируются).
  - 429: `data-retry-after` и disabled только на кнопке «Открыть сервер»;
    после cooldown кнопка снова активна.
  - Один клик = один IPC: guard `entering`, повторные клики в `entering`/`entered`
    игнорируются. В `entered` кнопка disabled до нового preview/закрытия.
  - `busy` (= resolve/accept/enter) для Esc/клика вне: диалог не закрывается в полёте.
- `rust/gui-spike/ui/features/actions.js`
  - Зарегистрирован `data-action="enter-guild"` (реестр §8.1; прямой слушатель
    ставит сам join-server, т.к. кнопка создаётся после init).
  - Esc/клик вне используют `join.busy`, а не только `pending`.
- `rust/gui-spike/ui/components/server-menu.css`
  - Стили `.join-preview-open` (38px, `--btn-*`, focus-ring, disabled) на токенах.
- `tests/unit/join-server.test.mjs` — аддитивный блок из 8 тестов:
  preview→open, 204/entered/статус, 403, 429, 401+сеть, один IPC, отсутствие плитки,
  разделение enter/accept, аргумент только `{guildId}` без токена.

Контракт не менялся: backend `src/**` read-only, bearer остаётся в backend,
nullable-поля accept не подставляются, lurker не трактуется как членство.

## Red

```
node --test tests/unit/join-server.test.mjs
ℹ tests 24
ℹ pass 16
ℹ fail 8
```

Новые тесты падали по ожидаемым причинам: `нет #join-preview [data-action="enter-guild"]`,
`assert.equal(JOIN_COMMANDS.enter, "join_guild")` → `undefined`, отсутствие `join_guild`
в исходнике. Старые 16 тестов оставались зелёными (ослаблений нет).

## Green

```
node --test tests/unit/join-server.test.mjs
ℹ tests 24 / pass 24 / fail 0

node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs \
  tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs \
  tests/unit/gui-spike-auth.test.mjs tests/unit/gui-spike-regressions.test.mjs \
  tests/unit/guild-rail.test.mjs tests/unit/voice-ui.test.mjs \
  tests/unit/channel-switch.test.mjs tests/unit/join-server.test.mjs
ℹ tests 343 / suites 55 / pass 343 / fail 0 / skipped 0
```

tsc / геометрия:

```
npx tsc -p rust/gui-spike/ui/tsconfig.json      → exit 0
node waves/RW-39/PM/w39c/geometry-check.mjs     → все 31 значений в допуске
```

## Файлы (md5)

| Файл | md5 |
|-|-|
| `rust/gui-spike/ui/features/join-server.js` | `940612793f1f82ba8625da4dfd33d438` |
| `rust/gui-spike/ui/features/actions.js` | `908bfe2dbf4202cdf28c389d44b2095f` |
| `rust/gui-spike/ui/components/server-menu.css` | `6b90663a2038b2619c3cd655f160d588` |
| `tests/unit/join-server.test.mjs` | `5bb2673320804f042362273b28dc0b3d` |

`index.html` и `src/**` не менялись.

## Риски / остаток

- Живой 204 lurker (`PUT /guilds/{id}/members/@me?lurker=true`) не проверялся:
  окно/live запрещены; опора — принятый backend G-03a (D-76) и `[L]` 204 ×2 в G6.
- 403 не блокирует кнопку навсегда: ручной повтор разрешён и повторяет только
  `join_guild` (требование 2); авто-повтора нет. Если нужно «без повтора» строго —
  отдельное решение Team Lead.
- Успех enter объявляется в `#server-status` (visually-hidden live-регион, как accept).
  Отдельного видимого тоста не добавляли, чтобы не расширять DOM-контракт §6.
- После `entered` повторное открытие в рамках того же preview заблокировано;
  новое открытие — после нового preview/закрытия диалога.
- Полный `gate:worker`, cargo и mutation — не запускались (запрещены ТЗ на этот шаг).
