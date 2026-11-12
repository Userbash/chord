# REVIEW-66A: Техническое ревью карточки task-149 (`FE-P0-04`)

- **Карточка**: `task-149` (`FE-P0-04`: Монтирование `createReactionsController` в `rust/gui-spike/ui/features/chat.js` и интерактивные реакции `reactions.js`)
- **Компоненты**:
  - `rust/gui-spike/ui/features/chat.js`
  - `rust/gui-spike/ui/features/reactions.js`
  - `tests/unit/reactions-ui.test.mjs`
  - `tests/unit/gui-spike-dom.test.mjs`
- **Ревьюер**: Технический ревьюер (Review 66A)
- **Дата проверки**: 2026-10-30
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-149-mount-reactions-chat`

---

## 1. Резюме и вердикт

- **Вердикт**: **PASS / Ready for Merge**
- **Уровень дефектов**: **P3** (дефектов уровней P0, P1, P2 не обнаружено).
- Все требования ТЗ и спецификации `REACTIONS-EXTENDED-SPEC.md` выполнены в полном объёме:
  - кликабельные пилюли реакций с поддержкой обычных (`type=0`) и супер-реакций / burst (`type=1`, `Shift+Click`), оптимистичным обновлением счётчиков, атрибутов `data-me`, `aria-pressed`, `data-burst` и класса `.active`, с гарантированным откатом при ошибках IPC;
  - всплывающая панель действий (`.message-action-toolbar` / `.msg-hover-toolbar`) с быстрыми реакциями, кнопкой ответа и открытием 9-категорийного пикера;
  - 9-категорийный `#reaction-picker-popover` (`role="dialog"`, `aria-modal="false"`) с изолированными id (`#reaction-picker-*`), сохраняющий инвариант `10` стационарных табов на `#screen-emoji` через lazy-destroy;
  - модалка «Кто поставил реакцию» (`#reaction-users-modal`, `role="dialog"`, `aria-modal="true"`) с вкладками «Обычные» (`type=0`) и «Супер-реакции» (`type=1`) по contextmenu на пилюле;
  - корректная очистка обработчиков и DOM при перерисовке строк и вызове `destroy()`;
  - инварианты DOM чата соблюдены: ровно 2 ребёнка у строки (`.author` + `.text`), `innerHTML` не используется.

---

## 2. Результаты тестов и проверки типов

1. **Unit & Integration тесты**:
   ```sh
   node --test tests/unit/reactions-ui.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs
   ```
   - **Результат**: **144 pass, 0 fail, 0 skip** (18 suites, ~740ms, exit code 0).
   - В том числе:
     - `tests/unit/reactions-ui.test.mjs`: 5/5 pass;
     - `tests/unit/gui-spike-dom.test.mjs`: 39/39 pass (включая интеграционный тест `FE-P0-04 (task-149): монтирование реакций в chat.js без нарушения DOM-инвариантов`);
     - `tests/unit/gui-spike-features.test.mjs`: 100/100 pass.

2. **Статическая типизация**:
   ```sh
   npx tsc -p tsconfig.json
   ```
   - **Результат**: **0 ошибок (exit code 0)**. Чистый typecheck проекта.

---

## 3. Проверка функциональных требований и инвариантов

- [x] **Пилюли реакций (`.reaction`)**:
  - Обычный клик: переключает реакцию (`toggle_reaction` / `onToggleReaction`), обновляет `.reaction-count` и класс `.active`.
  - Клик с зажатым `Shift`: устанавливает супер-реакцию (`burst=true`, `type=1`, атрибуты `data-burst="true"`, `data-reaction-type="1"`, класс `reaction--burst`).
  - Ошибки IPC (`401`, `429`, сетевые): корректный откат состояния в `requestReaction`.
  - Повторный вызов `bindMessageRow` при виртуализации корректно очищает слушатели без утечек и дублирования.
- [x] **Панель действий (`.message-action-toolbar` / `.msg-hover-toolbar`)**:
  - Расширяет существующий `.msg-toolbar` либо монтируется динамически в standalone.
  - Содержит кнопки быстрых реакций (`[data-command="react"]`), кнопку добавления реакции (`[data-command="add-reaction"]`) с `aria-haspopup="dialog"`, `aria-expanded` и `aria-controls`, а также кнопку ответа (`[data-command="reply"]`).
- [x] **9-категорийный `#reaction-picker-popover`**:
  - `role="dialog"`, `aria-modal="false"`, `data-state="ready"|"empty"`.
  - Префиксные id узлов предотвращают конфликт со стационарным экраном `#screen-emoji`.
  - Переключение 9 вкладок категорий (`role="tab"`) и поиск по эмодзи.
  - Переключатель супер-реакции `[data-action="toggle-burst"]` с атрибутом `aria-pressed`.
  - Закрытие по кнопке `✕`, выбору эмодзи и клавише `Escape` с возвратом фокуса.
  - При закрытии размонтируется (lazy destroy), сохраняя инвариант `document.querySelectorAll(".tab").length === 10`.
- [x] **Модалка «Кто поставил реакцию» (`#reaction-users-modal`)**:
  - `role="dialog"`, `aria-modal="true"`.
  - Вкладки `Обычные` (`data-reaction-type="0"`) и `Супер-реакции` (`data-reaction-type="1"`).
  - Вызов `get_reaction_users` / `onFetchReactionUsers` с соответствующим `type` (0 или 1).
  - Рендеринг списка пользователей (`#reaction-users-list` с `role="listitem"`).
  - Закрытие по Escape и кнопке `✕` с возвратом фокуса на триггер (пилюлю реакции).
- [x] **Очистка ресурсов (`destroy()` / `dispose()`)**:
  - Полное удаление активных поповеров и модалок из DOM, снятие всех навешенных слушателей со строк.
  - Инвариант `row.children.length === 2` (`.author` + `.text`) строго соблюдён.

---

## 4. Верификация мутационного тестирования (5/5 KILLED)

Проверено 5 независимых мутантов в кодовой базе карточки:

| Мутант | Файл | Описание мутации | Результат |
|---|---|---|---|
| **M1** | `reactions.js` | Отключение синхронизации пилюли `syncPillDom(pill, next, burst)` при клике | **KILLED** (`AssertionError: счётчик должен оптимистично увеличиться до 2`, exit code 1) |
| **M2** | `reactions.js` | Подмена `type: burst ? REACTION_TYPE_BURST : REACTION_TYPE_NORMAL` на константу `REACTION_TYPE_NORMAL` | **KILLED** (`AssertionError: 0 !== 1`, exit code 1) |
| **M3** | `reactions.js` | Отключение размонтирования поповера `activePopover.remove()` и `existing.remove()` в `closePicker` | **KILLED** (нарушение lazy destroy, таймаут/падение тестов DOM, exit code 1) |
| **M4** | `reactions.js` | Поломка переключения вкладки `Супер-реакции` (подмена `currentType = REACTION_TYPE_BURST` на `NORMAL`) | **KILLED** (`AssertionError: 'false' !== 'true'`, exit code 1) |
| **M5** | `chat.js` | Отключение монтирования `createReactionsController` в `createChatScreen` | **KILLED** (3 теста упали: отсутствие `.message-action-toolbar` и модалки `#reaction-users-modal`, exit code 1) |

Все мутации были проверены независимо, зафиксированы падения тестов, после чего файлы возвращены в исходное состояние с проверкой контрольных сумм.

---

## 5. Контрольные суммы проверенных файлов

```
503c86c840d55f5a96e8f0a5000ba69b  rust/gui-spike/ui/features/chat.js
aa45fe323a0844ed4f675ffe42b0307d  rust/gui-spike/ui/features/reactions.js
8b814efb01b8305c0b35523e4608734e  tests/unit/reactions-ui.test.mjs
018c247adbce6eec1f903c966833b905  tests/unit/gui-spike-dom.test.mjs
```

---

## 6. Заключение

Карточка `task-149` (`FE-P0-04`) полностью готова к слиянию (**Ready for Merge**). Нарушений контрактов, дефектов a11y, несовместимостей DOM-инвариантов и регрессий в смежных компонентах чата не обнаружено.
