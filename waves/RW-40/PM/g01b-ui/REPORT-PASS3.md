# REPORT-PASS3 — INT-03-UI-APP-PANELS-MODALS (Воркер G2 Frontend Lane B)

- **Дата:** 2026-10-30.
- **Роль:** Воркер G2 Frontend Lane B (RW-40, карточка INT-03-UI-APP-PANELS-MODALS).
- **Основание:**
  1. `waves/RW-40/PM/g01b-ui/REVIEW-PASS2.md` (P1 — гонка при быстрых последовательных кликах по серверам A -> B в `rememberGuild` в `rust/gui-spike/ui/app.js`; P2 — отсутствие mutation evidence для `lastGuildId`).
  2. `waves/RW-40/PM/stub10-pins-threads/REVIEW.md` (P1 — подключение `createPinsThreadsPanels` в `app.js` и оформление поповеров `.header-dropdown-panel`, `.pins-panel`, `.threads-panel` в `server-menu.css` на токенах `var(--token)`).
  3. `waves/RW-40/PM/stub05-invites-ui/REVIEW.md` (интеграция `createInviteModal` в `app.js`).
- **Назначенные файлы (только они):**
  - `rust/gui-spike/ui/app.js`
  - `rust/gui-spike/ui/components/server-menu.css`
  - `tests/unit/guild-rail.test.mjs`
- **Статус:** red → green, targeted-прогоны (97 тестов), `tsc` (exit 0) и 5 мутаций (5/5 KILLED) успешно завершены. Коммитов нет.

---

## 1. Проблемы и контекст

1. **Гонка A -> B в `rememberGuild` (`app.js`):**
   При быстрых последовательных кликах пользователя по плитке сервера A, а затем B, каждый вызов `rememberGuild` запускал асинхронный `settings_load` без ожидания и без сверки актуальности. Если ответ `settings_load` для A возвращался позже, чем завершался B, устаревший запрос A вызывал `settings_save` и перезаписывал выбор пользователя назад на A.
2. **Отсутствие интеграции панелей и модалок (`app.js`):**
   Контроллеры `createPinsThreadsPanels` и `createInviteModal` существовали изолированно в `features/*.js` без вызова из `app.js` и без экспорта в `currentApp()`.
3. **Отсутствие стилей поповеров (`server-menu.css`):**
   Для классов `.header-dropdown-panel`, `.pins-panel`, `.threads-panel` отсутствовали правила позиционирования (`top: var(--layout-header-h)`, `right: var(--space-4)`, `z-index: var(--z-popover)`), ширин (`360px` для pins, `380px` для threads) и внутренних элементов панелей.

---

## 2. Strict TDD (Red-first)

В `tests/unit/guild-rail.test.mjs` добавлен целевой тест моделирования гонки при быстрых кликах:
- **Тест:** `быстрые клики серверов A -> B с задержкой первого settings_load/settings_save гарантированно сохраняют последний выбор B`.
- **Сценарий:**
  1. Boot приложения с подменой IPC `settings_load`.
  2. Клик по плитке сервера A (`tileA.click()`) — вызов `settings_load` задерживается в deferred promise `firstLoadDeferred`.
  3. Быстрый клик по плитке сервера B (`tileB.click()`) — его `settings_load` и `settings_save` выполняются штатно.
  4. Разрешение отложенного промиса первого клика (`resolveFirstLoad()`) и ожидание стабилизации microtasks (`app.settle(6)`).
  5. Проверка инварианта: в хранилище настроек остаётся `targetB`, старый запрос A не перетёр актуальное состояние.

### Red-прогон
```bash
node --test tests/unit/guild-rail.test.mjs > waves/RW-40/PM/g01b-ui/red-pass3.log 2>&1
```
- **Exit code:** 1.
- **Ошибка:**
  ```text
  AssertionError [ERR_ASSERTION]: гонка при быстрых кликах A -> B: сохранён 294064825540280335, ожидался последний выбор 338001249918255106
  + actual - expected
  + '294064825540280335'
  - '338001249918255106'
  ```
- **Артефакт:** `waves/RW-40/PM/g01b-ui/red-pass3.log`.

---

## 3. Реализация (Green)

1. **`rust/gui-spike/ui/app.js` — защита токеном поколения `latestGuildId`:**
   - Введена переменная `let latestGuildId = ""` внутри области видимости рейла.
   - В `rememberGuild(id)` при старте выставляется `latestGuildId = id`.
   - После завершения `await invoke("settings_load")` выполняется строгая проверка актуальности:
     ```javascript
     if (latestGuildId !== id) return;
     ```
     Если пользователь уже переключился на другую гильдию, устаревший запрос немедленно прерывается без отправки `settings_save` и без затирания актуальных настроек.
   - Ошибки `settings_save` в `catch` также фильтруются `if (latestGuildId === id)` во избежание показа ложных ошибок для отменённых выборов.

2. **`rust/gui-spike/ui/app.js` — подключение `pinsThreads` и `inviteModal`:**
   - Импортированы `createPinsThreadsPanels` из `./features/pins-threads.js` и `createInviteModal` из `./features/invite-modal.js`.
   - Обновлён `App` typedef с сигнатурами `pinsThreads` и `inviteModal`.
   - Создан контроллер `inviteModal = createInviteModal({ invoke })`.
   - Реализовано прогрессивное подключение `createPinsThreadsPanels` по спецификации PINS-THREADS-SPEC §1–§3:
     - Контроллер и DOM-узлы `#chat-pins-panel` и `#chat-threads-panel` создаются по требованию (lazy getter `pinsThreads`), что гарантирует чистоту DOM на этапе boot (критично для контрактных проверок отсутствия статических узлов).
     - Добавлены клик-слушатели на кнопки хедера `[data-action="pins"]` и `[data-action="threads"]`, которые открывают соответствующие панели при первом клике.
   - Контроллеры экспортированы в возвращаемом объекте приложения `currentApp()`.

3. **`rust/gui-spike/ui/components/server-menu.css` — поповер-стили панелей:**
   - Добавлен раздел `/* 5. Панели M-10 (pins) и M-11 (threads) в хедере чата */`.
   - `.header-dropdown-panel`: абсолютное позиционирование `position: absolute; top: var(--layout-header-h); right: var(--space-4); z-index: var(--z-popover); max-height: calc(100vh - var(--layout-header-h) - var(--space-8));` с фоном `var(--surface-2)`, скруглением `var(--radius-sm)`, рамкой `var(--border-subtle)` и тенью `var(--shadow-popover)`.
   - `.pins-panel`: ширина `360px`.
   - `.threads-panel`: ширина `380px`.
   - Оформлены элементы `.dropdown-header`, `.dropdown-title`, `.dropdown-close`, `.dropdown-content`, `.dropdown-empty-state`, `.dropdown-error-state`, `.pin-item`, `.thread-item`, табы веток и поисковое поле строго на дизайн-токенах из `tokens.css`.

4. **`tests/unit/guild-rail.test.mjs` — интеграционные контракты:**
   - Добавлен тест `STUB-10 / STUB-05: currentApp() экспортирует рабочие контроллеры pinsThreads и inviteModal`.
   - Добавлен тест `PINS-THREADS-SPEC §1–§3: server-menu.css содержит поповер-правила .header-dropdown-panel, .pins-panel, .threads-panel`.

---

## 4. Green-прогон и Targeted-тесты

### Green-прогон целевого набора
```bash
node --test tests/unit/guild-rail.test.mjs > waves/RW-40/PM/g01b-ui/green-pass3.log 2>&1
```
- **Exit code:** 0.
- **Результат:** 15 passed, 0 failed.
- **Артефакт:** `waves/RW-40/PM/g01b-ui/green-pass3.log`.

### Полный targeted-набор смежных модулей
```bash
node --test tests/unit/guild-rail.test.mjs tests/unit/server-menu.test.mjs tests/unit/join-server.test.mjs tests/unit/channel-paging.test.mjs
```
- **Результат:** 97 passed, 0 failed, exit 0:
  - `guild-rail.test.mjs`: 15 passed (рейл, восстанавление `lastGuildId`, гонка A->B, экспорт панелей, CSS-поповеры).
  - `server-menu.test.mjs`: 46 passed (меню сервера, a11y, roving tabindex, диалоги, leave, геометрия).
  - `join-server.test.mjs`: 35 passed (нормализация ссылок, диалог join, lurker preview, модалка invite STUB-05/G-06).
  - `channel-paging.test.mjs`: 19 passed (постраничный скролл, older messages, панели пинов и веток STUB-10).

### Type Check
```bash
npx tsc -p tsconfig.json                      # exit 0, 0 errors
npx tsc -p rust/gui-spike/ui/tsconfig.json    # exit 0, 0 errors
```

---

## 5. Мутационное тестирование (Mutation Proof)

Проведено мутационное тестирование с проверкой восстановления baseline MD5 после каждого мутанта:
- **Baseline MD5:**
  - `rust/gui-spike/ui/app.js`: `4d46b018bbca23fe08be08bae1ac1a32`
  - `rust/gui-spike/ui/components/server-menu.css`: `08c5c3cb71d59df18e90a72787f0fe8a`
  - `tests/unit/guild-rail.test.mjs`: `a1907d0e8820f346743b56b63fdfaaa6`

| ID | Цель / Описание | Мутант MD5 | Результат | Убивший тест |
|---|---|---|---|---|
| **M1** | `app.js`: Снятие проверки `if (latestGuildId !== id) return;` после `settings_load` | `85537d38eb89961e46ffc85b5dbf6abd` | **KILLED** (exit 1) | `быстрые клики серверов A -> B с задержкой первого settings_load/settings_save гарантированно сохраняют последний выбор B` |
| **M2** | `app.js`: Инверсия проверки `if (latestGuildId === id) return;` | `fe32eb1321649df9d18c54e1ffa94be4` | **KILLED** (exit 1) | `клик по плитке сохраняет lastGuildId, не теряя остальные настройки` |
| **M3** | `app.js`: Искажение payload `lastGuildId: ""` вместо `id` | `28abd0266e7236661c8606dccd844f7f` | **KILLED** (exit 1) | `клик по плитке сохраняет lastGuildId, не теряя остальные настройки` |
| **M4** | `app.js`: Потеря экспорта `pinsThreads: null` из `currentApp()` | `1dda9a6a00bf575bc734160c817abe1a` | **KILLED** (exit 1) | `STUB-10 / STUB-05: currentApp() экспортирует рабочие контроллеры pinsThreads и inviteModal` |
| **M5** | `server-menu.css`: Нарушение `z-index: 10` вместо `var(--z-popover)` | `2f32a9b0b3e4a7c2c2949bb4d5979596` | **KILLED** (exit 1) | `PINS-THREADS-SPEC §1–§3: server-menu.css содержит поповер-правила .header-dropdown-panel, .pins-panel, .threads-panel` |

Все 5 мутантов уничтожены (**5/5 KILLED**, survivors: 0). После каждого мутанта выполнено восстановление и сверен MD5 (`match: OK`).
Подробный протокол: `waves/RW-40/PM/g01b-ui/mutations-pass3.log`.

---

## 6. Итоговые контрольные суммы (MD5)

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/app.js` | модифицирован | `4d46b018bbca23fe08be08bae1ac1a32` |
| `rust/gui-spike/ui/components/server-menu.css` | модифицирован | `08c5c3cb71d59df18e90a72787f0fe8a` |
| `tests/unit/guild-rail.test.mjs` | модифицирован | `a1907d0e8820f346743b56b63fdfaaa6` |
| `waves/RW-40/PM/g01b-ui/red-pass3.log` | создан | `f09c5b4b6a16c54405a669c3b9eb5837` |
| `waves/RW-40/PM/g01b-ui/green-pass3.log` | создан | `3ad6aab2f2448c7b52bbc3167b9e80a9` |
| `waves/RW-40/PM/g01b-ui/mutations-pass3.log` | создан | `b2253f01d65a940eeee55b67b95ae89d` |

---

## 7. Риски и выводы

- **Архитектурные риски:** отсутствуют. Изменения локализованы строго в трёх назначенных файлах. Чужие модули и тесты не затрагивались.
- **Интеграция:** взаимные исключения и progressive mounting панелей протестированы как автономно (`channel-paging.test.mjs`), так и на уровне приложения (`guild-rail.test.mjs`).
- Карточка **INT-03-UI-APP-PANELS-MODALS (G-01b-UI Pass 3)** полностью готова к приёмке.
