# FE-P2-01 (task-182): доступный Tooltip вместо сырых `title`

## Отчёт воркера

- **batch/карточка:** RW-40 / FE-P2-01 (`waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` §3), ветка
  `feature/task-182-ui-tooltip-component` от `unstable` (HEAD `b8183f5`).
- **сделано:**
  - `rust/gui-spike/ui/features/tooltip.js` (новый, 381 строка): `TOOLTIP_SHOW_DELAY_MS = 400`,
    замороженный `ICON_TOOLTIPS` (13 подписей), `createTooltipController(doc, options)` с
    `attach / detach / showFor / hide / destroy`, один общий `#discord-tooltip` в `body`,
    миграция `title → data-tooltip`, `aria-describedby`, `data-open`, `data-placement`,
    закрытие по `mouseleave / focusout / click / Escape`;
  - `rust/gui-spike/ui/components/primitives.css` (+31 строка): `.tooltip` / `.tooltip_c36707`
    на токенах `--z-tooltip`, `--tooltip-radius`, `--tooltip-pad-x/y`, `--surface-4`,
    `--text-strong`, `--shadow-popover`; `pointer-events: none`; явное скрытое состояние;
  - `tests/unit/tooltip.test.mjs` (новый, 471 строка, 20 тестов, 6 сьютов).
- **red:** `node --test tests/unit/tooltip.test.mjs` → каркас без реализации: `ℹ tests 20, pass 0,
  fail 20`, exit 1 (`waves/RW-40/G2/task-182-ui-tooltip-component/red.log`).
- **green:** `node --test tests/unit/tooltip.test.mjs` → `ℹ tests 20, pass 20, fail 0`, exit 0;
  `npx tsc -p tsconfig.json` → 0 ошибок (`green.log`).
- **targeted:** `tests/unit/tooltip.test.mjs` — 20/20; смежные фронтенд-контракты
  (9 файлов из `test-impact`) — 400/400; `waves/RW-39/PM/w39c/geometry-check.mjs` — 31/31 в допуске.
- **oracle:** `reference-oracle/frontend-d03.md` §5 п.7 (`tooltip_c36707`,
  `tooltipDisablePointerEvents_c36707`); `reference-oracle/ru-catalog.json` — 12 из 13 подписей
  совпадают буквально; `tokens.css` — значения стилей; ТЗ FE-P2-01 — задержка 400 мс и перечень
  иконок. Подписи в тесте — извлечённый список (как в `gui-spike-features.test.mjs`), тест не
  зависит от наличия лаборатории и не читает константы модуля.
- **генерация:** не применимо (ручных артефактов нет).
- **мутации:** 5/5 KILLED:
  1. `TOOLTIP_SHOW_DELAY_MS 400 → 300` → 2 падения (константа + записанная задержка);
  2. удаление `target.removeAttribute("title")` → 3 падения (миграция `title`);
  3. `TOOLTIP_CLOSE_KEYS.includes(key) → false` → 1 падение (Escape);
  4. удаление `setAttribute("data-open", "true")` → 2 падения;
  5. удаление `pointer-events: none` из CSS → 1 падение.
- **baseline/restore:** после каждой мутации файл восстановлен; md5 до и после совпадают
  (`tooltip.js 3cae3eca51640289a37424c03b7da2bb`, `primitives.css f9a9bc1fbfd3fb5e06dbb94f7237dc65`).
- **стабильность:** гонок и внешних ресурсов нет, таймеры инъецированы; повторный прогон даёт тот же
  результат.
- **gate:worker:** запущен (работа одиночная, не batch) —
  `CARGO_TARGET_DIR=/home/user/orca/project/rust/target npm run gate:worker`, exit 1,
  вердикт `failed` (`gate.log`). Разбор по фазам:
  - `typecheck`, `ui-tsc` — зелёные;
  - `meta` — 96/102: pre-existing (`keyring-live` ждёт 12 тестов, в дереве 13 — `k25_...`) плюс
    `plan-sync`, который требует строку §8 для **девяти** тестовых файлов, включая мой новый
    (`attachments-ui`, `guild-modals`, `members-subscriptions`, `notifications`, `pins-threads`,
    `reactions-ui`, `tooltip`, `user-popout`, `voice_ui`). §8/`PLAN.md` — общий файл Team Lead;
  - `unit` — 1183/1184: `dispatch_ui` A2 (`CHANNEL_CREATE` отбрасывается UI) — pre-existing;
  - `spike-rust` — `ipc_wiring_contract::every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`
    — pre-existing;
  - `workspace-rust` — не компилируется `discord-core` (тест `dispatch_types`: нет методов
    `is_stream`, `parse_stream`, `to_stream_action`) — pre-existing;
  - `smoke` — 19/22: нет собранного `rust/gui-spike/target/debug/discord-gui-spike` и заглушка окна.
  - **Доказательство pre-existing:** те же четыре падения воспроизведены на базе без моих файлов
    (три файла задачи временно убраны, `primitives.css` застэшен): `plan-sync` (8 файлов без моего),
    `dispatch_ui` A2, `ipc_wiring_contract` — падают так же. После проверки файлы возвращены,
    `rust/gui-spike/Cargo.lock` (побочный артефакт cargo) восстановлен из HEAD.
- **файлы + md5:**
  | файл | md5 |
  |---|---|
  | `rust/gui-spike/ui/features/tooltip.js` (новый) | `3cae3eca51640289a37424c03b7da2bb` |
  | `rust/gui-spike/ui/components/primitives.css` | `f9a9bc1fbfd3fb5e06dbb94f7237dc65` |
  | `tests/unit/tooltip.test.mjs` (новый) | `740a5c6d2c85ef6329d2650a783e35e0` |

## Артефакты

| файл | что внутри |
|---|---|
| `red.log` | прогон до реализации: 20 падений, exit 1 |
| `green.log` | targeted 20/20 + `tsc` без ошибок |
| `gate.log` | полный `gate:worker` с разбором фаз |

## Риски и вопросы

1. **Подпись `attach` расходится с каталогом.** В `ru-catalog.json` строки «Прикрепить файл» нет,
   ближайшая точная — «Прикрепить вложения». В коде оставлена подпись из ТЗ («Прикрепить файл»),
   тест фиксирует ровно одно расхождение из 13 и падает, если состав изменится. Решение — за G3/G7
   и Team Lead: либо ТЗ правится на «Прикрепить вложения», либо строка остаётся «нашей» `[A]`.
2. **§8 без строки теста.** `tests/unit/tooltip.test.mjs` отсутствует в `PLAN.md` §8 — общий файл,
   правит Team Lead. До этого `meta`/`plan-sync` останется красным (там уже 8 чужих файлов).
3. **Компонент пока не смонтирован.** `index.html` и `app.js` вне моей области записи, поэтому
   тултип не подключён к иконкам шапки/рейла. Интеграция: `controller.attach(document)` после
   построения shell, `title` у иконок можно оставить — контроллер сам переносит его в `data-tooltip`.
4. **Позиционирование не реализовано.** Координаты (`top`/`left`) не вычисляются: в DOM-шиме нет
   layout. `data-placement` — контракт для слоя, который владеет геометрией иконки; сейчас элемент
   `position: fixed` без вставок.
5. **`package-lock.json` изменён не мной.** В рабочем дереве до начала работы уже был диф
   (добавлены `bin` и `engines` от `package.json`); мои файлы его не касаются, восстановление
   оставляю Team Lead.
6. **`prefers-reduced-motion`/анимация появления** не добавлялись: в макете подсказка появляется
   без перехода, лишнее движение не требуется.

## Следующий шаг

Приёмка Team Lead + PM; после принятия — строка §8, монтаж `attach()` в `app.js`/`index.html`
отдельной карточкой, затем проверка в `testing`.
