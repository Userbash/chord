# Отчёт воркера: task-152 (`FE-P0-06` DOM-адаптер `mountPinsThreadsDom` и `createPinsThreadsController`)

- **Волна / Отдел**: `RW-40` (Отдел G2 Frontend: WebView UI & DOM/A11y)
- **Карточка**: `task-152` (`FE-P0-06` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Спецификации**: `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` (§1–§3, §5–§6), `STUB-10` (`M-10` и `M-11` / `M-12`)
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-152-mount-pins-threads`

---

## 1. Что сделано

1. **Red-first тесты (`tests/unit/pins-threads.test.mjs`)**:
   - Написан набор из 6 поведенческих unit-тестов (`STUB-10` + `FE-P0-06 / task-152`), проверяющих:
     - привязку `mountPinsThreadsDom(rootEl, controllerOrOpts)` к существующим кнопкам шапки канала (`#btn-TogglePins` и `#btn-ToggleThreads`) и их динамическое создание при отсутствии;
     - синхронизацию `aria-expanded` (`"false"` ↔ `"true"`), `aria-haspopup="dialog"`, `aria-controls` и переключение видимости поповера пинов (`#pins-popover`, `role="dialog"`) и боковой панели веток (`#threads-sidebar` / `#thread-view-panel`) со взаимным исключением;
     - рендеринг списка закреплённых сообщений в `#pins-popover` (автор, текст, время, кнопка `'Перейти'` → `onJumpToMessage` / `jumpToMessage`, кнопка `'Открепить'` с двухшаговым подтверждением `.pin-unpin-confirm[role="alertdialog"]` / `confirm-unpin` / `cancel-unpin` → `unpinMessage`) и пустого состояния (`data-state="empty"`, `"В этом канале нет закреплённых сообщений"`);
     - рендеринг вкладок Активные (`#tab-active-threads`) / Архивные (`#tab-archived-threads`) ветки в `#threads-sidebar`, формы создания новой ветки (`#create-thread-form`: валидация названия `1..100` символов, `aria-invalid`, `#thread-form-error[role="alert"]`, выбор `auto_archive_duration` → `createThread`) и переход внутрь ветки (`#thread-view-panel` с кнопкой закрытия `#btn-close-thread-view`);
     - закрытие открытых панелей по клавише `Escape` с возвратом фокуса на кнопку-триггер и полную очистку всех слушателей через возвращаемый `destroy()`.
   - Падение до реализации зафиксировано в `/home/user/orca/project/waves/RW-40/G2/task-152-mount-pins-threads/red.log` (`tests 6, pass 1, fail 5`).

2. **Реализация (`rust/gui-spike/ui/features/pins-threads.js`)**:
   - Сохранены без изменений все существующие экспорты `STUB-10` (`isInside`, `ensurePinsPanel`, `ensureThreadsPanel`, `renderPinsState`, `renderThreadsState`, `createPinsThreadsPanels`).
   - Реализованы и экспортированы:
     - константы `THREAD_NAME_MIN_LEN = 1`, `THREAD_NAME_MAX_LEN = 100`, `THREAD_ARCHIVE_DURATIONS = [60, 1440, 4320, 10080]`, `DEFAULT_THREAD_ARCHIVE_DURATION = 1440`;
     - контроллер состояния `createPinsThreadsController(options)` (управление списками `pins`, `activeThreads`, `archivedThreads`, взаимным исключением `pinsOpen`/`threadsOpen`, двухшаговым подтверждением `requestUnpin`/`cancelUnpin`/`unpinMessage`, созданием ветки `createThread` с валидацией `1..100` символов и `auto_archive_duration`, переходом внутрь ветки `openThread`/`closeThreadView`, подпиской `subscribe` и `destroy`);
     - DOM-адаптер `mountPinsThreadsDom(rootEl, controllerOrOpts)` с полной очисткой слушателей в `destroy()`.

---

## 2. Red-прогон (до реализации)

- **Команда**: `node --test tests/unit/pins-threads.test.mjs`
- **Результат**: `tests 6 | pass 1 | fail 5`
- **Лог**: `/home/user/orca/project/waves/RW-40/G2/task-152-mount-pins-threads/red.log`

---

## 3. Green-прогон и проверка типов (после реализации)

- **Команды**:
  - `node --test tests/unit/pins-threads.test.mjs` → **6 pass, 0 fail** (exit 0)
  - `node --test tests/unit/channel-paging.test.mjs` → **14 pass, 0 fail** (exit 0)
  - `npx tsc -p tsconfig.json` → **0 ошибок** (exit 0)
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json` → **0 ошибок** (exit 0)
  - `node --test tests/meta/test-policy.test.mjs tests/meta/ipc-path.test.mjs` → **29 pass, 0 fail** (exit 0)
- **Лог**: `/home/user/orca/project/waves/RW-40/G2/task-152-mount-pins-threads/green.log`

---

## 4. Мутационная проверка (4 мутации)

Все 4 целевые семантические мутации пойманы тестами (`4/4 KILLED`), после чего файл восстановлен до байта:

| № | Инвариант | Мутация | Статус |
|---|---|---|---|
| 1 | Взаимное исключение панелей в `openThreads()` | Замена `pinsOpen = false` на `void 0` | **KILLED** (`AssertionError: 'true' !== 'false'`) |
| 2 | Подтверждение перед `unpinMessage` | Немедленный вызов `controller.unpinMessage(item.id)` вместо `controller.requestUnpin(item.id)` | **KILLED** (`AssertionError: без подтверждения unpinMessage не должен вызываться`) |
| 3 | Лимит длины названия ветки `1..100` символов | Замена `THREAD_NAME_MAX_LEN = 100` на `200` | **KILLED** (`AssertionError: название > 100 символов не должно вызывать createThread`) |
| 4 | Снятие слушателей при `destroy()` | Ранний `return` в `destroy()` до очистки слушателей | **KILLED** (`AssertionError: после destroy() клик по #btn-TogglePins не открывает поповер`) |

- **Лог мутаций**: `/home/user/orca/project/waves/RW-40/G2/task-152-mount-pins-threads/mutations.log`

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/features/pins-threads.js` | `ee1a0241b88244c4d0212df4597d2460` |
| `tests/unit/pins-threads.test.mjs` | `b866c9be139417f153957ab799c8df2e` |
| `/home/user/orca/project/waves/RW-40/G2/task-152-mount-pins-threads/red.log` | `8838b265a540aaf3ea7fa63e210fa42a` |
| `/home/user/orca/project/waves/RW-40/G2/task-152-mount-pins-threads/green.log` | `97d604dddc71c014c7a0ec2e508e0645` |
| `/home/user/orca/project/waves/RW-40/G2/task-152-mount-pins-threads/mutations.log` | `692b4693c431243c089c51a7d9edd61f` |

---

## 6. Риски и границы

- Чужие файлы (`app.js`, `index.html`, `chat.js`, `composer.js`) не затрагивались.
- Существующий контракт `createPinsThreadsPanels` (M-10 / M-11) полностью сохранён.
