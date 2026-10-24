# G1-01-UI — search-overlay: новые `SearchError.kind` (G2 Frontend)

- batch/карточка: RW-40 / G1-01-UI (продолжение G1-01; основание — `waves/RW-40/QUALITY/G1-AUDIT.md` §4, §7.1);
- сделано:
  - `rust/gui-spike/ui/features/search-overlay.js`:
    - `SEARCH_ERROR_KINDS` — добавлены `forbidden`, `notFound`, `client`, `server`
      (`SearchError` из `src/search.rs` — закрытый список; без них ошибка считалась
      «команда не зарегистрирована» и включала локальный fallback);
    - `remoteErrorText`: `forbidden` → «нет доступа (403)», `notFound` → «не найдено (404)»,
      `client{status}` → «ошибка запроса (400)» (статус не теряется; без числа — «4xx»),
      `server{status}` → «ошибка сервера (503)» (без числа — «5xx»); хелпер
      `httpStatusCode()` — нечисловой/нулевой статус не подставляется;
    - шапка модуля и docstring `renderRemoteError` уточнены: fallback запрещён для всех
      `SearchError`;
  - `tests/unit/gui-spike-features.test.mjs`: новый кейс «G1-01: forbidden/notFound/client/server —
    видимая ошибка без fallback и свои тексты» (9 подкейсов: 403, 404, 400, 422, 503, 5xx без
    статуса, 401, 429, network). Проверяется: `data-state=error`, текст по каждому kind,
    0 `.search-result`, `matchesCount=0`, `mode="remote"` (не откатился в local), статус не
    оконный, ошибка на видимой поверхности `.content`, все 9 сообщений попарно различны.
- red: `node --test tests/unit/gui-spike-features.test.mjs` → **70 tests / 69 pass / 1 fail**,
  exit 1 (`red.log`); падение именно целевого кейса:
  `403: нет видимой ошибки (найдено 27 из 400 загруженных … · локально: сетевая команда поиска недоступна)`
  — подтверждает описанный дефект (forbidden → fallback). До добавления теста baseline был
  зелёным: 69/69, exit 0.
- green: `node --test tests/unit/gui-spike-features.test.mjs` → **70 / 70**, exit 0
  (`green-features.log`).
- targeted: `node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs
  tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs
  tests/unit/gui-spike-auth.test.mjs` → **216 / 216 pass, 32 suites, 0 fail**, exit 0
  (`green-suite.log`). До карточки те же файлы давали 215 → 215 не ослаблены, +1 новый кейс.
- oracle: `rust/gui-spike/src/search.rs` (`SearchError` + `serde(tag="kind", rename_all="camelCase")`:
  `forbidden`, `notFound`, `client{status}`, `server{status}`), `rust/gui-spike/src/rest_failure.rs`
  (`from_status`: 403/404/其余 4xx/5xx, статус в варианте), `waves/RW-40/QUALITY/G1-AUDIT.md` §7.1.
  Тест описывает ожидаемый UI-контракт по этим enum'ам, а не текущую реализацию JS.
- генерация: не применимо.
- мутации: не применимо (JS-карточка UI; эквивалент red-first проверен снятием kind'ов из
  закрытого списка — именно это дало red).
- baseline/restore: не применимо (мутаций нет).
- стабильность: не требовалось (чистая функция без таймеров/сети); новые кейсы используют
  `whenIdle()` и не зависят от порядка.
- gate:worker: **не запускался** — по ТЗ окно/live/cargo/gate запрещены; воркер в batch
  ограничивается targeted-прогонами. Полный гейт — integration-лейн.
- дополнительные проверки:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0 (`tsc.log`);
  - `node waves/RW-39/PM/w39c/geometry-check.mjs` → «все 31 геометрических значений в допуске»,
    exit 0 (`geometry.log`).
- файлы (+md5 на финальных байтах):
  - `rust/gui-spike/ui/features/search-overlay.js` — `1fd0716585bb6406fbd2b8efcfcb5dba`
  - `tests/unit/gui-spike-features.test.mjs` — `fb150fe58b4326c83fff35bfe3ac6267`
  - `waves/RW-40/PM/g1-01-ui/red.log` — `9c65274fe24d059af534bba4edc0e665`
  - `waves/RW-40/PM/g1-01-ui/green-features.log` — `c77b0c5c976e0abed6eeeb11c1341a6f`
  - `waves/RW-40/PM/g1-01-ui/green-suite.log` — `70b29753d6eed496ed1cfaa0d96c22c1`
  - Оба файла карточки — untracked (`?? rust/gui-spike/ui/`, `?? tests/unit/gui-spike-features.test.mjs`),
    поэтому git-baseline md5 отсутствует: сравнение только с финальными байтами.
- риски/вопросы:
  1. `client`/`server` показывают текст по-русски с исходным числовым статусом; если PM захочет
     иные формулировки — это одна строка в `remoteErrorText`.
  2. Backend гарантирует `status` только для `client`/`server`; для `403/404` единичные варианты
     статуса не несут — текст фиксированный, «не терять status» выполнено для тех kind'ов, где он есть.
  3. Rust, `src/**`, остальной `ui/**` не менялись; `ui/` и тест — untracked, коммит не делался.
- следующий шаг: приёмка Team Lead (review G2), затем доки волны и `waves/db.jsonl` — на стороне
  координатора.
