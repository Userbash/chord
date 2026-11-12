# REVIEW-64A: Техническое ревью карточки task-152 (`FE-P0-06`)

- **Карточка**: `task-152` (`FE-P0-06`: DOM-адаптер `mountPinsThreadsDom` и контроллер `createPinsThreadsController`)
- **Компоненты**: `rust/gui-spike/ui/features/pins-threads.js`, `tests/unit/pins-threads.test.mjs`
- **Ревьюер**: Технический ревьюер (Review 64A)
- **Дата проверки**: 2026-10-30
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-152-mount-pins-threads`

---

## 1. Резюме и вердикт

- **Вердикт**: **PASS / Ready for Merge**
- **Уровень дефектов**: **P3** (замечаний уровня P0, P1, P2 не обнаружено).
- Все функциональные и a11y-требования выполнены в точном соответствии со спецификациями `PINS-THREADS-PROFILE-SPEC.md` (§1–§3, §5–§6) и `STUB-10` (`M-10`, `M-11`).
- Обратная совместимость с ранее реализованными экспортами `STUB-10` полностью сохранена.

---

## 2. Результаты верификации тестов и типов

1. **Targeted unit-тесты**:
   ```sh
   node --test tests/unit/pins-threads.test.mjs tests/unit/gui-spike-features.test.mjs
   ```
   - **Результат**: **106 pass, 0 fail** (15 suites, duration ~730ms, exit code 0).
   - Все 6 тестов `pins-threads.test.mjs` проходят успешно.
   - Регрессий в `gui-spike-features.test.mjs` (100 тестов) нет.

2. **Проверка статической типизации**:
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json`: **0 ошибок** (exit code 0).
   - `npx tsc --noEmit rust/gui-spike/ui/features/pins-threads.js --checkJs ...`: **0 ошибок** (exit code 0).
   - `// @ts-check` и подробный JSDoc присутствуют во всех экспортируемых функциях и типах.
   *(Примечание: ошибки в глобальном `tsconfig.json` вызваны не связанным сторонним файлом `tests/unit/dispatch_ui.test.mjs` из ветки L-02-UI на baseline main; к коду task-152 замечаний по типам нет).*

---

## 3. Проверка функциональных требований и инвариантов

- [x] **Кнопки шапки (`#btn-TogglePins`, `#btn-ToggleThreads`)**:
  - Привязываются к существующим в DOM элементам или динамически создаются в контейнере шапки при их отсутствии.
  - Поддерживают атрибуты `aria-haspopup="dialog"`, `aria-controls`, `aria-expanded` ("false" ↔ "true").
- [x] **Поповер пинов (`#pins-popover`)**:
  - Атрибут `role="dialog"`.
  - Взаимное исключение с боковой панелью веток (открытие одного автоматически закрывает другое).
  - Рендеринг списка закреплённых сообщений с автором, текстом, временем и действиями.
  - Кнопка перехода к сообщению (`jumpToMessage` / `onJumpToMessage`).
  - Двухшаговое подтверждение открепления: вызов `requestUnpin`, показ `.pin-unpin-confirm[role="alertdialog"]` с отменой (`cancelUnpin`) и подтверждением (`confirmUnpin` -> `unpinMessage`).
  - Корректный переход в пустое состояние (`data-state="empty"`).
- [x] **Боковая панель веток (`#threads-sidebar`) и просмотр ветки (`#thread-view-panel`)**:
  - Переключение вкладок «Активные» (`#tab-active-threads`) и «Архивные» (`#tab-archived-threads`).
  - Валидация названия новой ветки `1..100` символов (`aria-invalid`, `#thread-form-error[role="alert"]`).
  - Выбор длительности архивации (`auto_archive_duration` из `[60, 1440, 4320, 10080]`) и передача в `createThread`.
  - Открытие карточки ветки внутри `#thread-view-panel` и закрытие кнопкой `#btn-close-thread-view`.
- [x] **Клавиатура и a11y**:
  - Закрытие панелей по нажатию клавиши `Escape` с возвратом фокуса на соответствующую кнопку-триггер (`#btn-TogglePins` / `#btn-ToggleThreads`).
- [x] **Жизненный цикл и очистка**:
  - Метод `destroy()` отписывается от событий контроллера, очищает слушатели событий и предотвращает утечки памяти.

---

## 4. Мутационное тестирование (5/5 KILLED)

Проведено 5 независимых семантических мутаций в файле `rust/gui-spike/ui/features/pins-threads.js`. Все 5 мутаций обнаружены и «убиты» тестами (`5/5 KILLED`):

| № | Описание мутации | Изменение в коде | Статус | Сообщение теста |
|---|---|---|---|---|
| **M1** | Нарушение взаимного исключения в `openThreads()` | Замена `pinsOpen = false` на `pinsOpen = true` | **KILLED** | `AssertionError: 'true' !== 'false'` |
| **M2** | Обход подтверждения открепления сообщения | Немедленный вызов `controller.unpinMessage(item.id)` вместо `controller.requestUnpin` | **KILLED** | `AssertionError: без подтверждения unpinMessage не должен вызываться` |
| **M3** | Ослабление валидации длины названия ветки | Изменение `THREAD_NAME_MAX_LEN = 100` на `200` | **KILLED** | `AssertionError: название > 100 символов не должно вызывать createThread` |
| **M4** | Отключение очистки слушателей в `destroy()` | Пропуск вызова зарегистрированных `cleanups` в `destroy()` | **KILLED** | `AssertionError: после destroy() клик по #btn-TogglePins не открывает поповер` |
| **M5** | Отключение обработчика клика по кнопке `#btn-TogglePins` | Пропуск регистрации `listen(btnPins, "click", ...)` | **KILLED** | `AssertionError: Expected values to be strictly equal: 'false' !== 'true'` |

После завершения мутационного тестирования файл `rust/gui-spike/ui/features/pins-threads.js` восстановлен в исходное состояние, контрольная сумма MD5 проверена.

---

## 5. Контрольные суммы файлов (MD5)

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/features/pins-threads.js` | `ee1a0241b88244c4d0212df4597d2460` |
| `tests/unit/pins-threads.test.mjs` | `b866c9be139417f153957ab799c8df2e` |

---

## 6. Итоговый статус

Задача **task-152** полностью готова к интеграции и слиянию в основную ветку.
