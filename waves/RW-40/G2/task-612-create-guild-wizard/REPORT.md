# REPORT — TASK-612 (G2 Frontend): Визард шаблонов создания серверов (`rust/gui-spike/ui/features/create-guild.js` + `rust/gui-spike/ui/styles/create-guild.css`)

- **batch/карточка:** `RW-40 / TASK-612` (G2 Frontend — Визард шаблонов создания серверов)
- **Ветка:** `feature/task-612-create-guild-wizard`
- **Рабочее дерево:** `/home/sanya/orca/workspaces/project/task-612-create-guild-wizard`
- **Дата:** 2026-11-07
- **Коммитов нет:** `git commit` / `git push` не выполнялись (согласно правилам `AGENTS.md` §6).

---

## 1. Сделано (`PROCESS.md` §4.2)

- **batch/карточка:** `RW-40 / TASK-612`;
- **сделано:**
  1. В `rust/gui-spike/ui/features/create-guild.js` (с полным сохранением обратной совместимости существующих экспортов `STUB-03`) реализован визард выбора шаблонов и настройки создаваемого сервера:
     - `CREATE_GUILD_TEMPLATES` и `VALID_TEMPLATES` — замороженный справочник 4 шаблонов по RE-оракулу Discord: `'Свой шаблон'` (`id: "custom"`), `'Для меня и друзей'` (`id: "friends"`, категория/алиас `"gaming"`), `'Учебная группа'` (`id: "study"`), `'Клуб или сообщество'` (`id: "community"`), а также резолвер `resolveGuildTemplate(templateIdOrLabel)`.
     - `MIN_GUILD_NAME_CHARS = 1`, `MAX_GUILD_NAME_CHARS = 100`, `normalizeGuildName(name)` и `validateGuildName(name)` — валидация названия сервера `1..=100` символов Unicode после обрезки пробелов (`.trim()`) с нормализацией имени перед отправкой в Tauri IPC `create_guild`.
     - `validateGuildIconDataUri(dataUri)`, `setIcon(dataUri)`, `clearIcon()`, `getIcon()` — загрузка, валидация и предпросмотр иконки сервера в формате base64 DataURI (`data:image/(png|jpeg|jpg|gif|webp);base64,...`) с DOM-элементами `#create-guild-icon-input`, `#create-guild-icon-preview` (`<img class="create-guild-icon-img">`) и `#create-guild-icon-clear` и передачей `icon` в `create_guild`.
     - Доступность (A11y): навигация стрелками (`ArrowDown`, `ArrowRight`, `ArrowUp`, `ArrowLeft`, `Home`, `End`) по карточкам шаблонов с циклическим переходом и roving `tabindex` (`"0"` / `"-1"`), закрытие модального окна по `Escape` с возвратом фокуса и `aria-expanded="false"` на триггер, а также фокус-ловушка (`Tab` / `Shift+Tab`) внутри `#create-guild-dialog`.
  2. Создан файл стилей `rust/gui-spike/ui/styles/create-guild.css`, оформленный на 100% через CSS-переменные из `rust/gui-spike/ui/tokens.css` (без сырых hex/rgb/hsl цветов и без `!important`).
  3. Создан модульный тест `tests/unit/create-guild-wizard.test.mjs` (10 тестов в 5 сьютах), синхронизирована граница `1..=100` в `tests/unit/create-guild.test.mjs` и добавлена строка синхронизации в `TDD-PLAN.md` §8.
- **red:** `node --test tests/unit/create-guild-wizard.test.mjs` → `1 fail` (`SyntaxError: The requested module '../../rust/gui-spike/ui/features/create-guild.js' does not provide an export named 'CREATE_GUILD_TEMPLATES'`), `exit code = 1` (лог: `waves/RW-40/G2/task-612-create-guild-wizard/red.log`);
- **green:** `node --test tests/unit/create-guild-wizard.test.mjs` → `10/10 pass, 0 fail, 0 skipped`, `exit code = 0` (лог: `waves/RW-40/G2/task-612-create-guild-wizard/green.log`);
- **targeted:**
  - `node --test tests/meta/plan-sync.test.mjs tests/unit/create-guild-wizard.test.mjs tests/unit/create-guild.test.mjs tests/unit/join-server.test.mjs` → `48/48 pass, 0 fail, 0 skipped` (`1081.51ms`, `exit code = 0`);
  - `node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs tests/meta/waves-sync.test.mjs` → `174/174 pass, 0 fail, 0 skipped` (`5484.46ms`, `exit code = 0`);
  - `node waves/RW-39/PM/w39c/geometry-check.mjs` → `31/31 ok` (`exit code = 0`);
  - `npm run typecheck` → `0 errors` (`exit code = 0`);
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `0 errors` (`exit code = 0`);
- **oracle:**
  - `/home/sanya/re-deep/frontend-d03.md` §5.5 (`CreateGuildModal`: `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, focus trap модуль `617986`, `--modal-width-small: 442px`);
  - `/home/sanya/re-deep/decoded/a72f8c0298aeb52e_0` (модуль `819638`: шаги визарда `guild-templates` и `customize-guild`);
  - `/home/sanya/re-deep/decoded/9164065a2404f2fe_0` и `/home/sanya/re-deep/ru-catalog.json` (`H3+6BK`: `'Свой шаблон'`, `uE7zcu`: `'Для меня и друзей'`, `fYwSi2`: `'Учебная группа'`, `h9Q1lG`: `'Клуб или сообщество'`);
  - `/home/sanya/re-deep/decoded/496925fc1facf986_0` (модуль `356343`: загрузчик значка сервера base64 DataURI);
  - `rust/gui-spike/src/guilds.rs` (`create_guild`: валидация `1..=100` символов после `trim`, поля `name`, `icon`, `template`);
- **генерация:** не применимо (статические модули UI и таблица стилей);
- **мутации:** `5/5 KILLED` (`0 survivors`, `0 INCONCLUSIVE`, лог: `waves/RW-40/G2/task-612-create-guild-wizard/mutations.log`):
  - `M1_name_min_length_boundary` (`MIN_GUILD_NAME_CHARS = 1` → `MIN_GUILD_NAME_CHARS = 2`): **KILLED** (`exit=1`) — тест `проверяет границы 1..=100 символов после обрезки пробелов и отклоняет невалидные значения`;
  - `M2_name_trim_omitted` (`normalizeGuildName`: `name.trim()` → `name`): **KILLED** (`exit=1`) — тест `обрезает пробелы перед отправкой в create_guild и принимает имя из 1 символа в форме`;
  - `M3_icon_data_uri_validation_bypass` (`validateGuildIconDataUri`: `if (!match) return false` → `if (!match) return true`): **KILLED** (`exit=1`) — тест `валидирует формат base64 DataURI для изображений (png, jpeg, jpg, gif, webp)`;
  - `M4_a11y_arrow_down_direction` (`ArrowDown`: `setActiveTemplateCard(cards, idx + 1, true)` → `setActiveTemplateCard(cards, idx, true)`): **KILLED** (`exit=1`) — тест `поддерживает навигацию стрелками (ArrowDown/ArrowUp/Home/End) по шаблонам с roving tabindex`;
  - `M5_a11y_escape_close_omitted` (`Escape` обработчик без вызова `close()`): **KILLED** (`exit=1`) — тест `закрывает диалог по клавише Escape и возвращает фокус и aria-expanded='false' на триггер`;
- **baseline/restore:**
  - До и после всех 5 мутаций проверены контрольные суммы:
    - `rust/gui-spike/ui/features/create-guild.js`: `8c625b60085954bf0f251ff7803cfb76` (`match: true`)
    - `tests/unit/create-guild-wizard.test.mjs`: `0b224e07095b6a72021ea5bdb979b0e3` (`match: true`)
- **стабильность:** не требовалось (детерминированные синхронные DOM unit-тесты без таймеров и сети);
- **gate:worker:** в соответствии с `PROCESS.md` §3.14 / §5 воркер сдаёт targeted red/green и typecheck; полный batch-gate запускается на integration-лейне;
- **файлы (`md5sum`):**
  ```text
  8c625b60085954bf0f251ff7803cfb76  rust/gui-spike/ui/features/create-guild.js
  131bffc91b56f2f153e0edf95718cd66  rust/gui-spike/ui/styles/create-guild.css
  0b224e07095b6a72021ea5bdb979b0e3  tests/unit/create-guild-wizard.test.mjs
  72986a9607ce5f243ab888e24749e9e5  tests/unit/create-guild.test.mjs
  a790f8ee34959b0b06cfd6ac23c7d1ad  TDD-PLAN.md
  ```
- **риски/вопросы:** отсутствуют; все существующие экспорты `rust/gui-spike/ui/features/create-guild.js` сохранены, смежные сьюты (`create-guild.test.mjs`, `join-server.test.mjs`, `gui-spike-dom.test.mjs`, `gui-spike-features.test.mjs`, `plan-sync.test.mjs`, `waves-sync.test.mjs`) полностью зелёные;
- **следующий шаг:** передать задачу `TASK-612` на Product Review (PM) и Technical Review (DeepSeek).
