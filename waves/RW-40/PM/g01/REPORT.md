# G-01 — рейл серверов из `ready()` (READY fixture)

Дата: 2026-10-24. Воркер: G2 Frontend (subagent PM DeepSeek). Batch: RW-40 / G-01.
Источник истины: `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.1, `NETWORK-CONTRACTS.md` §1.3,
`fixtures/gateway/by-type/002-READY.json` (7 гильдий), `rust/gui-spike/src/data.rs`
(`ReadyPayload`/`GuildSummary`), `waves/RW-40/DESIGN/SERVER-MENU.md` §2.1.

## 1. Сделано

- `invoke("ready")` — только после `auth ready` (`onReady`), без нового endpoint'а
  (`main.rs: ready` уже есть; REST-ручки списка гильдий в клиенте нет).
- Рейл данных: `#guild-rail` → контейнер `.guild-list[role="navigation"][aria-label="Серверы"]`,
  плитки `button.guild-item[data-guild-id=<snowflake>]`, `aria-label="Сервер <name>"`,
  `aria-current="true"` и `.is-selected` ровно у выбранной, монограмма 2 буквы.
- Поля только из payload: `id`, `name`; `icon` (хеш CDN) в URL **не** склеивается,
  `<img>`/`src` в рейле нет.
- Состояния: `data-state=loading` (3 skeleton-плитки, `aria-busy=true`) → `ready`;
  `0` гильдий → `empty` + `.guild-empty` («Нет серверов»); ошибка `ready()` → `.error-state`
  (`role=alert`, `data-error-code=IPC_FAILED`) с кнопкой `.error-state-retry` («Повторить»),
  повторный вызов отдаёт список. Ответ без `guilds[]`/гильдия без `id|name` — тоже error,
  имена не выдумываются.
- Выбор: клик по плитке, `↑/↓` (с заворотом), `Home/End`; roving `tabindex`; фокус остаётся
  в рейле; заголовок `.tree-guild` следует за выбранным сервером.
- `ui/index.html`: удалены статичные заглушки `data-guild="1|2|3"` (осталась только точка
  вставки перед `.rail-sep`; новых `id`/классов в html нет — контракт `gui-spike-dom`
  «html ⊆ шим-DOM» не нарушен).
- `ui/features/actions.js` (leave-flow G-05/G-04) читает `data-guild-id` (fallback на
  legacy `data-guild` шима), после выхода заголовок дерева берётся у новой выбранной плитки.
- `ui/components/rail.css`: `.guild-list`, `.guild-skeleton` (+pulse), `.guild-empty`,
  локальный `.error-state` внутри рейла (в `primitives.css` он `position:absolute; inset:0`
  для контента — в 64px-рейле переопределён на статический поток).

Не тронуто: `src/**`, `features/search-overlay.js`, `features/errors.js`, `tests/support/**`,
`tests/unit/gui-spike-*.test.mjs`. Окно/live/cargo/gate/сеть/commit не запускались.

## 2. Red (до кода)

```
node --test tests/unit/guild-rail.test.mjs
# ✖ 7 гильдий … / ✖ нет заглушки data-guild="1" / ✖ клик / ✖ клавиатура /
# ✖ empty-state / ✖ error+retry / ✖ loading skeleton
# ℹ tests 8  pass 1  fail 7
```

Падение первых прогонов зафиксировано в `red-first-run-tail.log` (срез вывода).
Полный воспроизводимый red-лог — `red-mutation.log`: копия `app.js` с выключенным
`void rail.load()` (`GUI_DOM_APP=/tmp/opencode/g01-red-ui/app.js node --test …`):
`ℹ tests 8  pass 2  fail 6` (2 проходят guard-кейсы: «заглушки нет» и «до входа ready()
не вызывается» — они и должны проходить без фичи).

## 3. Green

```
node --test tests/unit/guild-rail.test.mjs
ℹ tests 8  pass 8  fail 0      (green.log)

node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs \
  tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs \
  tests/unit/gui-spike-auth.test.mjs tests/unit/gui-spike-regressions.test.mjs
ℹ tests 253  pass 253  fail 0   (не ослаблены)

npx tsc -p rust/gui-spike/ui/tsconfig.json        → exit 0
node waves/RW-39/PM/w39c/geometry-check.mjs       → «все 31 геометрических значений в допуске» (geometry.log)
```

Проверки нового теста (8 кейсов): 7 гильдий и их `id`/`name` из фикстуры; `data-guild-id`,
`aria-label`, `aria-current`, монограмма 2 буквы; отсутствие заглушек `data-guild`;
клик меняет выбранного + `rail.selectedId` + заголовок дерева; `↑/↓/Home/End` и фокус;
empty-state; error+retry (ровно 1 `ready()` до retry, 2 после; `consoleErrors` пуст);
loading skeleton до ответа; до `auth ready` `ready()` не вызывается.

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/index.html` | `d84df49a62c8f8f1436914dacf6ed43f` |
| `rust/gui-spike/ui/app.js` | `a43621b2879f141ccaae5e9888c1c9d8` |
| `rust/gui-spike/ui/features/actions.js` | `1d792ba63d8c75344fe20e2c7e9ffd2f` |
| `rust/gui-spike/ui/components/rail.css` | `53b626f02dc153f85f7db1be328f59eb` |
| `tests/unit/guild-rail.test.mjs` (new) | `714f00e19f0c413c27447c85e65eca9d` |

## 5. Остаток / риски

1. **`search-overlay` и guildId (вне scope, read-only).** `features/search-overlay.js`
   фиксирует `guildId` в момент сборки (`options.guildId ?? selectedGuildId()`, где
   `selectedGuildId()` читает `dataset.guild`). Рейл теперь data-driven и грузится после
   входа, поэтому в живом приложении remote-search получит `""` до отдельного контракта
   (нужно либо передавать геттер, либо обновлять overlay после `ready`). Тесты 253 зелёные:
   шим по-прежнему строит 3 статичные плитки, а рейл в этих тестах не грузится (нет auth ready).
2. **Шим `tests/support/gui-dom-harness.mjs`** всё ещё содержит 3 статичные `data-guild`
   плитки и не отвечает на `ready` (файл вне матрицы G-01). Новый тест подаёт `ready` через
   `invokeHandler`. При миграции шима на READY fixture — убрать legacy-fallback `data-guild`
   в `actions.js`.
3. **G-01b (restore `lastGuildId`) — вне scope**, как и в §3.1: `UiSettings` без поля.
   Logout не сбрасывает рейл (старый список остаётся под модальным гейтом) — отдельная карточка.
4. **Визуальные константы** монограммы/скелетона: правило «инициалы 2 слов, иначе 2 первые
   буквы» выбрано по `[T]`-подсказке «как в существующем `.guild-item`»; в дизайн-доке
   зафиксировано только «2 буквы». Иконки — G-11 (`iconUrl` только от backend).
5. **Live-окно N0 не запускалось** (запрет): READY снят на 1.0.162, renderer 1.0.166 не снимался.
