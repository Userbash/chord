# TASK SEARCH-FIX — отчёт (QA-M2-03 / P1A)

- **batch/карточка**: RW-40 / QA-M2-03 (P1); роль G2 Frontend, OpenCode DeepSeek V4.1 Flash.
- **сделано**:
  - `rust/gui-spike/ui/features/search-overlay.js`: `selectedGuildId()` читает
    `data-guild-id`, legacy `dataset.guild` — только если он реально есть (DOM-шим);
    `guildId` в overlay стал ленивым (`getGuildId`), поэтому boot с пустым рейлом больше
    не «замораживает» пустой id. `remoteState`/`canRemote()`/`fetchRemote()` считают
    guild на каждый запрос; снимок id берётся один раз на страницу выдачи, затем уходит
    в `search_remote`. Ошибки `SearchError` (`unauthorized`/`forbidden`/`notFound`/…)
    по-прежнему рисуют видимую ошибку без локального fallback.
  - `rust/gui-spike/ui/app.js`: overlay получает `getGuildId: selectedGuildId` вместо
    снятого на boot `dataset.guild`; импортирован единый резолвер.
  - `tests/unit/search-remote-real-index.test.mjs`: новый regression на реальном
    `ui/index.html` и реальном `app.js`. Шим-заглушки `.guild-item` удаляются **до**
    импорта, поэтому boot видит пустой рейл, как продукт; рейл наполняется только
    `ready()` (7 гильдий из `fixtures/gateway/by-type/002-READY.json`). Проверяется:
    ровно один `search_remote` с snowflake из `data-guild-id`; ноль legacy `data-guild`
    в DOM; `unauthorized`/`forbidden`/`notFound` → ошибка, 0 результатов, `mode=remote`,
    без оконного fallback.
- **red**: `node --test tests/unit/search-remote-real-index.test.mjs` → **exit 1**,
  `pass 0 / fail 2`. Обе проверки падают на старом коде одинаково:
  `AssertionError: search_remote вызван 0 раз(а), нужен ровно один` — следствие
  `dataset.guild` при пустом на boot рейле. Лог: `red.log`.
- **green**: `node --test tests/unit/search-remote-real-index.test.mjs` → **exit 0**,
  `pass 2 / fail 0` (лог: `green.log`).
- **targeted**: `node --test tests/unit/gui-spike-features.test.mjs
  tests/unit/guild-rail.test.mjs tests/unit/gui-spike-dom.test.mjs
  tests/unit/search-remote-real-index.test.mjs` → **exit 0**, `tests 139 / pass 139 /
  fail 0 / skip 0` (лог: `targeted.log`). Чужие тесты не правились.
- **oracle**: `fixtures/gateway/by-type/002-READY.json` (7 гильдий; `id` — snowflake,
  имя — `properties.name`) + реальный `rust/gui-spike/ui/index.html` (статичных
  `data-guild="N"` нет). Контракт `search_remote`: параметры `guildId` + `params`.
- **генерация**: не применимо (ручной фикс + один regression-файл, генератора нет).
- **мутации**: не применимо (не запускались по условию задачи; red-прогон на старом коде
  эквивалентен целевой мутации «вернуть `dataset.guild`» и даёт падение обоих тестов).
- **baseline/restore**: baseline — `HEAD 0239543`, дерево dirty; фикс и тест новые
  (`rust/gui-spike/**` и `tests/unit/search-remote-real-index.test.mjs` вне git).
  RED снят на временно возвращённых старых версиях `app.js`/`search-overlay.js`,
  затем файлы восстановлены из копии; md5 ниже — финальные.
- **стабильность**: два зелёных прогона нового файла подряд (2/2 pass); гонок/ресурсов
  в изменении нет.
- **gate:worker**: не запускался (запрещено условием: no cargo/gate/live; воркер в batch
  делегирует полный гейт integration-лейну). `npx tsc -p tsconfig.json` → exit 1 только
  на **чужих** файлах: `scripts/gate.mjs` и `tests/unit/dispatch_ui.test.mjs`
  (pre-existing, вне scope); в `ui/app.js`, `search-overlay.js` и новом тесте ошибок нет.
- **файлы + md5**:
  - `rust/gui-spike/ui/app.js` — `c79d741461f24fbf9c768d1463c96795`
  - `rust/gui-spike/ui/features/search-overlay.js` — `44b34c0f9642188f54fd96c29afdf7a4`
  - `tests/unit/search-remote-real-index.test.mjs` — `182f6199561e8c6334db68a904526788`
  - артефакты: `waves/RW-40/PM/search-fix/{red,green,targeted,tsc}.log`
- **риски/остаток**:
  - Браузерного E2E сценария 3 всё ещё нет — это QA-M2-02/G4; текущий regression
    гоняет настоящий `app.js`/`index.html`-контракт в DOM-шиме, но **без** статичной
    плитки-подмены, поэтому false-green закрыт на уровне unit.
  - Legacy `data-guild` остаётся только как чтение `dataset.guild` для шима; продукт
    его не создаёт (проверяется тестом). Удалять fallback нельзя без правки шима.
  - Живой Discord/T3, cargo и полный gate не запускались — по условию задачи.
- **следующий шаг**: приёмка Team Lead; затем G4 — браузерный e2e сценария 3 и запись
  QA-M2-03 в `BUGS.md` (R-01/M-13).
