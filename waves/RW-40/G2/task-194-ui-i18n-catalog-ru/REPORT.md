# task-194 — UI i18n catalog RU (G2)

Batch: RW-40 / G2. Ветка: `feature/task-194-ui-i18n-catalog-ru` (без `git commit`).

## Сделано
- `tests/unit/i18n.test.mjs` — red-first контракт (11 тестов, 3 сьюта), сверяющий ключи, русские строки, плюральные формы CLDR и аудит DOM против оракулов `/home/sanya/re-deep/i18n-deep.md` и `/home/sanya/re-deep/ru-catalog.json`.
- `rust/gui-spike/ui/i18n.js` — модуль русской локализации UI (`// @ts-check` + JSDoc, ESM):
  - `HASHED_RU_KEYS`: замороженная карта (`Object.freeze`) из 45 канонических 6-символьных `base64(xxh64(id, seed=0))[0..6]` ключей Discord (`ru-catalog.json` / `i18n-deep.md`) по 10+ категориям (`inbox`, `search`, `settings`, `threads`, `polls`, `voice`, `attachments`, `tooltips`, `shortcuts`, `errors`, статусы и плюральные счётчики).
  - `RU_CATALOG` и `DEFAULT_LOCALE = "ru"`: замороженный семантический каталог русских строк UI.
  - `tHashed(hashKey, params = {})` и `t(key, params = {})`: разрешение ключей из `HASHED_RU_KEYS`, `RU_CATALOG` и `RU_PLURAL_PRESETS` (через `Object.prototype.hasOwnProperty.call`), подстановка `{param}` плейсхолдеров и поддержка русских плюральных форм через `selectRussianPluralCategory` / `formatPluralRu` из `./features/plural-format.js` (включая ICU-синтаксис `{count, plural, one {...} few {...} many {...} other {...}}`, точные селекторы `=N` и символ `#`).
  - `auditUntranslatedDomText(rootEl)` и `FORBIDDEN_ENGLISH_LABELS`: обход текстовых узлов и атрибутов `aria-label`, `placeholder`, `title` внутри `rootEl` с дедупликацией обнаруженных захардкоженных английских UI-меток (`Send Message`, `Search`, `Settings`, `Mute`, `Deafen`, `Disconnect`, `Upload File` и др.).
  - `plural`, `formatNumber`, `formatDate` и реэкспорт хелперов из `./features/plural-format.js`.

## Прогоны
- **Red:** `node --test tests/unit/i18n.test.mjs` → `fail 1` (`ERR_MODULE_NOT_FOUND` до создания `rust/gui-spike/ui/i18n.js`) — сохранено в `red.log`.
- **Green (targeted):** `node --test tests/unit/i18n.test.mjs` → `tests 11, suites 3, pass 11, fail 0, skip 0` — сохранено в `green.log`.
- **Typecheck (targeted):** `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess --target ES2023 --module NodeNext --moduleResolution NodeNext --types node rust/gui-spike/ui/i18n.js tests/unit/i18n.test.mjs` — `0 errors`.

## Файлы и md5
```
d12b967602657cbeb9a8362d49d3e623  rust/gui-spike/ui/i18n.js
5919ad3bab048aaa47ca505e5a0fc33d  tests/unit/i18n.test.mjs
```

## Риски / осталось
- Подключение `tHashed` / `auditUntranslatedDomText` во все экраны `app.js` и шаблоны `index.html` выполняется отдельными интеграционными карточками.
- Полный `gate:worker` не запускался в воркере (запускается один раз на batch в integration-лейне).
- `git commit` не выполнялся по правилам пайплайна.
