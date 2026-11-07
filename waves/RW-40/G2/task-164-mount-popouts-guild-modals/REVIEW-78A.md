# Technical Review: task-164 (`FE-P1-06` Mount `userPopout` & `guildModals` in `app.js`)

- **Карточка**: `RW-40` / `G2` / `task-164` (`FE-P1-06`)
- **Ворктри**: `/home/sanya/orca/workspaces/project/feature-task-164-mount-popouts-guild-modals`
- **Ветка**: `feature/task-164-mount-popouts-guild-modals`
- **Проверяемые файлы**:
  - `rust/gui-spike/ui/app.js` (`debe47e288cf689c700050050eea4b9b`)
  - `tests/unit/gui-spike-features.test.mjs` (`7e42032fbc13ebc6a0a5a6ed78dd04f1`)
- **Вердикт**: **`PASS / Ready for Merge`**

---

## 1. Архитектурная проверка и соответствие ТЗ

1. **Монтирование контроллеров**:
   - `createUserPopoutController` и `createGuildModalsController` корректно импортированы и инициализированы в `app.js` (`boot()`).
   - Оба контроллера включены в возвращаемый объект `currentApp()` с обновлённым JSDoc `@typedef {Object} App`.
   - Реэкспортированы из `app.js` для единой точки интеграции и тестовой изоляции.
2. **Делегирование событий и сохранение виртуализации**:
   - `userPopout.attachToChatAndMembers(...)` использует делегированные обработчики кликов на `#chat-list` и `#members-list`, не создавая лишних DOM-узлов до клика.
   - `guildModals.attachTrigger(...)` привязан к `#guild-rail` / `#guild-list` и `#guild-header` / `.tree-header`.
   - Все модалки монтируются в `document.body` с атрибутом `hidden`, не нарушая структуры окон виртуализации (инварианты 100 000 сообщений и 24 229 участников полностью соблюдены).
3. **Интеграция с уведомлениями**:
   - `notifications.openModal` реализован как фасад к `openGuildModal` (при наличии `opts.guildId`) либо к `syncNotificationsWithGuild(opts?.triggerElement)`.
   - В `createGuildModalsController` передан коллбэк `onOpenNotifications: () => notifications?.openModal?.()`.

---

## 2. Результаты верификации тестов и типов

1. **Unit-тесты**:
   ```sh
   node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/user-popout.test.mjs tests/unit/guild-modals.test.mjs
   ```
   - **Результат**: `206 pass, 0 fail, 0 skipped` (`24 suites`) за `2.35s` (`exit 0`).
2. **TypeScript проверка**:
   ```sh
   npx tsc -p tsconfig.json
   npx tsc -p rust/gui-spike/ui/tsconfig.json
   ```
   - **Результат**: `0 errors` в обоих конфигурациях (`exit 0`).

---

## 3. Мутационное тестирование (5/5 убито)

Проведено ручное мутационное тестирование в `rust/gui-spike/ui/app.js`:

| # | Мутация | Ожидаемый эффект | Фактический результат | Статус |
|---|---|---|---|---|
| **M1** | Удаление вызова `userPopout.attachToChatAndMembers(...)` | Клик по автору в чате не открывает `#user-popout` | Тест 2 упал (`AssertionError: клик по .author-name не открыл #user-popout`) | **УБИТ (1/5)** |
| **M2** | Удаление `guildModals.attachTrigger(guildRailNode)` | `contextmenu` на рейле серверов не открывает `#guild-context-menu` | Тест 3 упал (`AssertionError: contextmenu на #guild-rail не открыл #guild-context-menu`) | **УБИТ (2/5)** |
| **M3** | Замена коллбэка `onOpenNotifications: () => {}` | Клик по пункту «Настройки уведомлений» не вызывает открытие модалки | Тест 4 упал (`AssertionError: false !== true`) | **УБИТ (3/5)** |
| **M4** | Исключение `userPopout` из возвращаемого объекта `boot()` | Отсутствие `userPopout` в `currentApp()`, сбой тестов 1, 2, 5 | Тесты 1, 2, 5 упали (`AssertionError`, `TypeError`) | **УБИТ (4/5)** |
| **M5** | Исключение `guildModals` из возвращаемого объекта `boot()` | Отсутствие `guildModals` в `currentApp()`, сбой тестов 1, 3, 4, 5 | Тесты 1, 3, 4, 5 упали (`AssertionError`, `TypeError`) | **УБИТ (5/5)** |

**Результат мутационного анализа**: 5 из 5 мутантов успешно убиты целевыми тестами (`100% kill rate`).

---

## 4. Контроль целостности файлов

Хеши файлов до и после проведения мутационного тестирования совпадают:
- `rust/gui-spike/ui/app.js`: `debe47e288cf689c700050050eea4b9b`
- `tests/unit/gui-spike-features.test.mjs`: `7e42032fbc13ebc6a0a5a6ed78dd04f1`

---

## 5. Итоговое заключение

Код полностью готов к слиянию в ветку `unstable`. Все требования ТЗ, стандарты A11y/DOM и тестовой политики выполнены на 100%.

- **Статус задачи**: `Ready for Merge`
