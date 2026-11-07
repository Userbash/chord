# REPORT — G-01b-UI + C-07-FIX (отдел G2 Frontend Lane B)

- **Дата:** 2026-10-30.
- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748` (не менялся, коммитов нет).
- **Вход:** `PM/g01b-contract/ACCEPT.md` (`lastGuildId` в `UiSettings`, ACCEPTED) и
  `waves/RW-40/PARITY/MERGE-READINESS-05.md` §1 п.C-07 (`quick-switcher.js:67,144,150`).
- **Статус:** red → green, targeted-прогон и `tsc` зелёные. Полный `gate:worker` не
  запускался (см. §6).

## 1. Что сделано

**G-01b-UI (`rust/gui-spike/ui/app.js`)**

- `readLastGuildId()` — разбор `lastGuildId` из ответа `settings_load`: только
  строка-снежинка (как `is_snowflake` в `settings_store.rs`), иначе пусто; гильдия
  не выдумывается.
- Восстановление на boot: рейл получает `resolvePreferred()` и в `load()` ждёт его
  перед `renderGuilds(items, preferred)`. `renderGuilds` уже выбирает
  `items.find(id === focusId) ?? items[0]` — сохранённая гильдия, если её нет в
  READY, откатывается на первую.
- Второго `settings_load` на старте нет: ответ перехватывается в единственной точке
  IPC (`invoke`), где `settings.js` и так читает файл для тумблеров. Промис
  `settingsLoaded` закрывается и при ошибке чтения (`finally`) — рейл не ждёт вечно.
- Сохранение: `onChoose(id)` вызывается только при выборе пользователем (клик по
  плитке, стрелки/Home/End), не на boot/READY. `rememberGuild()` делает
  read-modify-write: свежий `settings_load` → `{ ...normalizeSettings(raw), lastGuildId }`
  → `settings_save`. Целая структура обязательна: `UiSettings` строгая
  (`deny_unknown_fields`), заготовка из одного поля затёрла бы тумблеры/плотность/
  тему/язык. Ошибка записи видна через `surface(error)`, а не молчит.
- `invoke(cmd, args)` в JSDoc: `args` помечен опциональным (`[args]`) — вызов
  `settings_load` без аргументов теперь типизирован честно.

**C-07-FIX (`rust/gui-spike/ui/features/quick-switcher.js`)**

- `guildIdOf(node)` — `data-guild-id`, затем `dataset.guildId` (второе — для DOM-шима,
  где `setAttribute` не выводит `data-*` в `dataset`). Legacy `dataset.guild` не
  читается ни в списке результатов, ни в `choose()`.

**Тесты (red-first)**

- `tests/unit/guild-rail.test.mjs`, новый `describe "G-01b"`: (а) boot выбирает
  гильдию с `id == lastGuildId`, `channels({guildId})` — для неё, `settings_save` на
  boot нет; (б) `lastGuildId` вне READY и отсутствие поля → первая гильдия; (в) клик
  по плитке → ровно один `settings_save` с обновлённым `lastGuildId` **и** сохранёнными
  полями файла (version/toggles/density/theme/locale).
- `tests/unit/gui-spike-regressions.test.mjs`, новый `describe "C-07"`: в готовом рейле
  нет `[data-guild]`; свитчер находит гильдию по имени, выбор в нём помечает нужную
  плитку (`data-guild-id`), заголовок дерева меняется, свитчер закрывается.

## 2. Red

```bash
node --test tests/unit/guild-rail.test.mjs tests/unit/gui-spike-regressions.test.mjs
# exit=1, 3 падения — полный лог: waves/RW-40/PM/g01b-ui/red.log
```

- `G-01b boot: выбирается гильдия с id == lastGuildId` — `actual '274093052673392641'`,
  `expected '700482297027231765'` (выбрана первая гильдия, а не сохранённая).
- `G-01b клик по плитке сохраняет lastGuildId` — `0 !== 1` (`settings_save` не вызван).
- `C-07 выбор гильдии в switcher` — `7 !== 1`: старый `dataset.guild` в шиме пуст у
  всех плиток, поэтому «выбраны» все; цель гильдии терялась (`target = ""`).

Третий G-01b-тест (откат на первую гильдию) на старом коде проходил — это и есть
текущее поведение, его и закрепили.

## 3. Green

```bash
node --test tests/unit/guild-rail.test.mjs tests/unit/gui-spike-regressions.test.mjs tests/unit/dispatch_ui.test.mjs
# tests 57, pass 57, fail 0, skipped 0, unit_exit=0
npx tsc -p tsconfig.json
# tsc_exit=0
node --test 'tests/unit/**/*.test.mjs'     # контроль вне карточки
# tests 713, pass 713, fail 0, unit_all_exit=0
```

Полный вывод — `waves/RW-40/PM/g01b-ui/green.log`. Контрольный прогон всех unit нужен
был из-за контракта `gui-spike-features.test.mjs`: «`settings_load` на старте ровно
один раз» — первая версия с отдельным чтением ломала его (2 вызова), поэтому чтение
перехватывается в `invoke`. Другие ui-тесты не задеты.

## 4. Файлы и md5

| Файл | MD5 | Роль |
|---|---|---|
| `rust/gui-spike/ui/app.js` | `5f081fdd05224d68ac4e7d8b9a803c8c` | G-01b: восстановление и сохранение `lastGuildId` |
| `rust/gui-spike/ui/features/quick-switcher.js` | `ba8aff457122cbd67b8edefec2df7479` | C-07: `data-guild-id` вместо `data-guild` |
| `tests/unit/guild-rail.test.mjs` | `939ab3ab60297a4d132f12a997053b0e` | контракт G-01b (3 теста) |
| `tests/unit/gui-spike-regressions.test.mjs` | `b0d21cbca0f3695087d0b5cabb1e6b4b` | регресс C-07 |
| `waves/RW-40/PM/g01b-ui/red.log` | `ce4192a55a2ff3f931e0111a008ae60b` | red-прогон |
| `waves/RW-40/PM/g01b-ui/green.log` | `5cac1c69ad5f68a3d3548f9f1d70a2c7` | green-прогон + контроль |
| `waves/RW-40/PM/g01b-ui/REPORT.md` | — | этот отчёт |

`git diff --stat`: 4 файла, +303 / −22. `chat.js`, `settings.js`, `index.html`,
`src/**`, golden-каталоги не менялись.

## 5. Границы

- Трогал только 4 файла карточки + `waves/RW-40/PM/g01b-ui/`.
- `chat.js` (G2 Lane A) не открывался на запись; `cargo` не запускался; сети, Discord,
  live, браузера и e2e не было.
- `index.html` и CSS не менялись: плитки по-прежнему создаются в `app.js`
  (`data-guild-id` там был с G-01).

## 6. Риски и долги

1. **`settings.js` затирает `lastGuildId` при своём сохранении.** `persist()` отправляет
   `collectSettings()` → `normalizeSettings()`, а тот по контракту отбрасывает
   неизвестные ключи, включая `lastGuildId`. После клика по гильдии выбор сохранён, но
   следующее переключение тумблера (debounce 200 мс) запишет файл без поля — рейл на
   следующем boot откроется на первой гильдии. Лечится одной строкой в `settings.js`
   (`...collectSettings(), lastGuildId` из общего состояния), но этот файл вне матрицы
   карточки — нужен владелец `settings.js` или отдельная карточка. **P2, требует решения
   PM/TL до merge.**
2. **`gate:worker` не запускался:** внутри `scripts/gate.mjs` есть Rust-фаза (`cargo`),
   а карточка cargo запрещает. Full gate — на integration-лейне.
3. **Сохранение на клик — read-modify-write:** между `settings_load` и `settings_save`
   возможен параллельный `settings_save` от `settings.js`; последний писатель выигрывает.
   Поле `lastGuildId` при этом не теряется (читается свежий файл), но при одновременном
   клике и переключении тумблера возможна потеря одного из изменений. Отдельного
   локирования в UI-слое нет — архитектурно это тот же P2, что п.1.
4. **Выбор в quick-switcher не пишет `lastGuildId`** (свитчер меняет только DOM
   выделения и не вызывает `channels`) — поведение не менялось, вне контракта карточки.
5. **`lastGuildId` переживает logout:** после выхода и повторного входа рейл восстановит
   прошлый выбор. Это ожидаемо для файла настроек, но если аккаунт другой, гильдии в
   READY не будет и сработает откат на первую.
6. **`preferredGuildId` читается один раз** на boot; READY-upsert (`applyGuilds`) выбор
   не перетирает (там `paintSelection`), но и не восстанавливает сохранённый, если
   первый `ready()` пришёл пустым.
