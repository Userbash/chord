# Отчёт по карточке `task-164` (`FE-P1-06`): монтирование контроллеров `createUserPopoutController` и `createGuildModalsController` в `app.js`

- **Batch / Карточка**: `RW-40` / `G2` / `task-164` (`FE-P1-06` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Воркер**: `term_962d5099-572f-47dc-a96f-bfbd78e34313` (`G2 Frontend — WebView UI & DOM/A11y`)
- **Статус**: `DONE` (Strict TDD: Red -> Green -> TypeScript -> Mutation checks)

---

## 1. Сделано

1. **Импорт и экспорт контроллеров в `rust/gui-spike/ui/app.js`**:
   - Импортированы `createUserPopoutController` из `./features/user-popout.js` (PR-43 / FE-P1-04) и `createGuildModalsController` из `./features/guild-modals.js` (PR-44 / FE-P1-05).
   - Выполнен реэкспорт `createUserPopoutController` и `createGuildModalsController` из `rust/gui-spike/ui/app.js` для единой точки интеграции и тестовой изоляции.
   - Обновлён JSDoc `@typedef {Object} App` с добавлением полей `userPopout` и `guildModals`.

2. **Монтирование `createUserPopoutController`**:
   - Создан экземпляр `userPopout = createUserPopoutController({ container: document.body, root: document.body, document, invoke, onError: reportUiError })`.
   - Вызван `userPopout.attachToChatAndMembers({ chatList: document.getElementById('chat-list'), membersList: document.getElementById('members-list') ?? document.getElementById('member-list') })`.
   - Делегированные обработчики клика в `#chat-list` (`.author`, `.author-name`, `.avatar`) и `#members-list` (`.member`, `.member-name`, `.member-label`) активируют карточку профиля без создания статических узлов в контейнерах списков.

3. **Монтирование `createGuildModalsController`**:
   - Создан экземпляр `guildModals = createGuildModalsController({ container: document.body, root: document.body, document, invoke, onError: reportUiError, onOpenNotifications: () => notifications?.openModal?.() })`.
   - В контроллере уведомлений `createNotificationsController` добавлен алиас `openModal: openGuildModal`, а в `boot()` метод `notifications.openModal` связан с синхронизацией гильдии `syncNotificationsWithGuild(triggerElement)`.
   - Контекстное меню привязано через `guildModals.attachTrigger()` к `#guild-rail` / `#guild-list` и `#guild-header` / `.tree-header` при наличии узлов в DOM.

4. **Сохранение всех инвариантов виртуализации и DOM**:
   - Все модалки (`#user-popout`, `#user-profile-modal`, `#guild-context-menu`, `#invite-modal`, `#leave-guild-modal`, `#join-guild-modal`) монтируются строго в `document.body` с атрибутом `hidden`.
   - В `#chat-list` и `#members-list` не добавляются лишние статические узлы до клика, что гарантирует сохранение инвариантов виртуализации 100k сообщений чата и 24k участников в `tests/unit/gui-spike-dom.test.mjs` (100% pass).

5. **Интеграционные тесты монтирования в `tests/unit/gui-spike-features.test.mjs`**:
   - Добавлен тестовый сьют `FE-P1-06 / task-164` из 5 тестов:
     1. Импорт и монтирование контроллеров при старте приложения, возврат в `currentApp()`, наличие скрытых DOM-элементов модалок в `document.body`.
     2. Открытие `#user-popout` по клику на `.author` в `#chat-list` и на `.member` в `#members-list` с корректными данными и закрытие через `closePopout()`.
     3. Открытие `#guild-context-menu` по событию `contextmenu` на триггере рейла `#guild-rail` и закрытие по Escape.
     4. Активация пункта «Настройки уведомлений» в меню сервера вызывает `onOpenNotifications` (`notifications.openModal()`).
     5. Очистка DOM и слушателей при вызове `destroy()`.

---

## 2. Red-прогон (Strict TDD)

- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-164-mount-popouts-guild-modals/red.log`
- **Команда**:
  ```sh
  node --test --test-name-pattern="FE-P1-06 / task-164" tests/unit/gui-spike-features.test.mjs
  ```
- **Результат**: `0 pass, 5 fail` — все 5 интеграционных тестов упали до реализации продуктового кода в `app.js` (отсутствие импорта, отсутствие `userPopout`/`guildModals` в `currentApp()`, отсутствие `#user-popout` и `#guild-context-menu` в DOM).

---

## 3. Green-прогон и проверка типов

- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-164-mount-popouts-guild-modals/green.log`
- **Команды**:
  ```sh
  node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/user-popout.test.mjs tests/unit/guild-modals.test.mjs
  npx tsc -p tsconfig.json
  npx tsc -p rust/gui-spike/ui/tsconfig.json
  ```
- **Результат**:
  - `206 pass, 0 fail, 0 skipped` (`24 suites`) за `2.4s`
  - `npx tsc -p tsconfig.json`: `0 errors` (`exit 0`)
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json`: `0 errors` (`exit 0`)

---

## 4. Мутационная проверка (TEST-POLICY.md)

Выполнены 3 мутации в `rust/gui-spike/ui/app.js` с проверкой падения тестов:
1. **Мутация M1** (закомментирован вызов `userPopout.attachToChatAndMembers(...)`):
   - Клик по автору в чате и участнику в списке не открывает `#user-popout` -> **тест 2 упал (`exit 1`)**.
2. **Мутация M2** (закомментирован вызов `guildModals.attachTrigger(...)`):
   - Правый клик `contextmenu` по `#guild-rail` не открывает `#guild-context-menu` -> **тест 3 упал (`exit 1`)**.
3. **Мутация M3** (`onOpenNotifications: () => {}` вместо вызова `notifications?.openModal?.()`):
   - Клик по «Настройки уведомлений» в меню не триггерит открытие модалки уведомлений -> **тест 4 упал (`exit 1`)**.

---

## 5. Изменённые файлы и MD5

| Файл | MD5 | Назначение изменений |
|---|---|---|
| `rust/gui-spike/ui/app.js` | `debe47e288cf689c700050050eea4b9b` | Импорт, монтирование, реэкспорт `userPopout` и `guildModals`, привязка триггеров и делегированных кликов, подключение `notifications.openModal` |
| `tests/unit/gui-spike-features.test.mjs` | `7e42032fbc13ebc6a0a5a6ed78dd04f1` | 5 интеграционных тестов монтирования `userPopout` и `guildModals` в `app.js` |

---

## 6. Риски и совместимость

- **Виртуализация 100k / 24k**: DOM-инварианты списков `#chat-list` и `#member-list` полностью сохранены; модалки и поповеры смонтированы в `document.body`.
- **Строгий TDD и отсутствие регрессий**: все 206 тестов в 4 целевых тестовых файлах выполняются зелёным за 2.4s.
- **Типобезопасность**: `tsconfig.json` и `rust/gui-spike/ui/tsconfig.json` компилируются без единой ошибки и без `any`-утечек наружу.
