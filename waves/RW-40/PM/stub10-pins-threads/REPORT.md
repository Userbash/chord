# Отчёт: STUB-10-PINS-THREADS-PANELS-UI (M-10 и M-11)

- **Роль**: Воркер G2 Frontend Lane C
- **Карточка**: STUB-10-PINS-THREADS-PANELS-UI (M-10 и M-11)
- **Волна**: RW-40
- **Дата**: 2026-10-30
- **Спецификация**: `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` (§1, §2, §3, §5, §6)

---

## 1. Что сделано

1. **Создан модуль `rust/gui-spike/ui/features/pins-threads.js`**:
   - Реализован контроллер `createPinsThreadsPanels(options)` с прогрессивным созданием:
     - `#chat-pins-panel` (`role="dialog"`, `aria-label="Закреплённые сообщения"`, `data-state="empty|loading|error|populated"`, `aria-modal="false"`, `tabindex="-1"`, кнопка закрытия `[data-action="close-pins"]`, контейнер `.pins-content[role="feed"]`).
     - `#chat-threads-panel` (`role="dialog"`, `aria-label="Ветки"`, `data-state="empty|loading|error|populated"`, `aria-modal="false"`, `tabindex="-1"`, кнопка закрытия `[data-action="close-threads"]`, табы `[role="tablist"]`, поле поиска `.threads-search-input`, контейнер `#threads-list.threads-content[role="region"]`).
   - Привязка к кнопкам-триггерам `[data-action="pins"]` и `[data-action="threads"]` с установкой и динамическим переключением `aria-expanded` (`true`/`false`), `aria-haspopup="dialog"`, `aria-controls` (`chat-pins-panel` / `chat-threads-panel`).
   - Взаимное исключение: открытие панели пинов автоматически закрывает панель веток и наоборот.
   - Поддержка всех 4 состояний для обеих панелей:
     - `loading`: `aria-busy="true"`, скелетоны (`.pin-skeleton` — 3 шт., `.thread-skeleton` — 4 шт.), screen reader статус.
     - `empty`: `aria-busy="false"`, `.dropdown-empty-state` с иконкой, заголовком и описанием.
     - `error`: `aria-busy="false"`, `.dropdown-error-state[role="alert"]`, кнопка повтора (`retry-pins` / `retry-threads`) с вызовом колбэков.
     - `populated`: `aria-busy="false"`, список элементов (`.pin-item` с кнопкой перехода `[data-action="jump-to-message"]`; `.thread-item` с переходом к ветке по клику и Enter, фильтрация по подстроке поиска и переключение табов Активные/Архив).
   - Управление фокусом и закрытие:
     - Клавиша `Escape`: закрытие активной панели с возвратом фокуса на кнопку-триггер.
     - Клик вне панели (outside click): закрытие с возвратом фокуса на кнопку-триггер.
     - Кнопки закрытия `[data-action="close-pins"]` и `[data-action="close-threads"]` с возвратом фокуса.
   - Метод `destroy()`: корректное снятие всех навешенных слушателей событий без утечек памяти.
   - Полное соблюдение строгой типизации: JSDoc, `// @ts-check`, 0 ошибок `tsc`, **0 `@ts-ignore`**.

2. **Добавлены тесты в `tests/unit/channel-paging.test.mjs`**:
   - В конец файла добавлен describe-блок `STUB-10-PINS-THREADS-PANELS-UI: M-10 (pins) и M-11 (threads) выпадающие панели` (5 тестов).
   - Существующие 14 тестов пагинации не изменялись и не ослаблялись.

3. **Файловая дисциплина**:
   - Изменены **только** разрешённые правилами файлы: `rust/gui-spike/ui/features/pins-threads.js` и `tests/unit/channel-paging.test.mjs`.
   - Запрещённые файлы (`app.js`, `chat.js`, `actions.js`, `composer.js`, `members.js`, `search.js`, `index.html`) не трогались.

---

## 2. RED-прогон

- **Команда**: `node --test tests/unit/channel-paging.test.mjs`
- **Exit Code**: 1
- **Первая строка ошибки**: `Error: createPinsThreadsPanels: not implemented`
- **Результат**: 14 pass, 5 fail (все 5 новых тестов упали на заглушке; 14 существующих тестов прошли)
- **Артефакт**: `waves/RW-40/PM/stub10-pins-threads/red.log`

---

## 3. GREEN-прогон

- **Команда**: `node --test tests/unit/channel-paging.test.mjs`
- **Exit Code**: 0
- **Результат**: 19 pass, 0 fail (14 существующих + 5 новых тестов STUB-10)
- **Длительность**: ~376 мс
- **Артефакт**: `waves/RW-40/PM/stub10-pins-threads/green.log`

---

## 4. Проверка типов (TypeScript)

- **Команда**: `npx tsc -p tsconfig.json && npx tsc -p rust/gui-spike/ui/tsconfig.json`
- **Exit Code**: 0 (0 ошибок)
- **Директивы `@ts-ignore`**: 0

---

## 5. Мутационное тестирование (5 мутаций)

Все 5 мутаций проверены, зафиксированы и пойманы тестами (100% caught):

| № | Инвариант | Внесённая мутация | Результат | Exit Code |
|---|---|---|---|:---:|
| 1 | Взаимное исключение | Отключён вызов `closeThreads()` при вызове `openPins()` | CAUGHT (`AssertionError: 'true' !== 'false'`) | 1 |
| 2 | Количество скелетонов пинов | Изменён цикл со скелетонами: `i < 2` вместо `i < 3` | CAUGHT (`AssertionError: должно быть 3 скелетона, 2 !== 3`) | 1 |
| 3 | Фильтрация поиска веток | Инвертировано условие подстроки: `!includes(query)` | CAUGHT (`AssertionError: assert.ok(items[0]?.textContent.includes("Архитектура Rust"))`) | 1 |
| 4 | Возврат фокуса при закрытии | Отключён вызов `pinsBtn.focus()` в `closePins()` | CAUGHT (`AssertionError: фокус вернулся на кнопку pins`) | 1 |
| 5 | ARIA-привязка `aria-controls` | Заменено значение атрибута `aria-controls` на `"wrong-panel"` | CAUGHT (`AssertionError: 'wrong-panel' !== 'chat-pins-panel'`) | 1 |

- **Артефакт мутаций**: `waves/RW-40/PM/stub10-pins-threads/mutations.log`

---

## 6. Файлы и контрольные суммы (md5)

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/features/pins-threads.js` | `f3e2708a3e8082dc60a999387850659a` |
| `tests/unit/channel-paging.test.mjs` | `e6974ee4a62ea5eef57ac128239e5dce` |
| `waves/RW-40/PM/stub10-pins-threads/red.log` | `f18858e3f03d4b852230af39817fafb5` |
| `waves/RW-40/PM/stub10-pins-threads/green.log` | `f2ee4f5e56698374f096beeef096a895` |
| `waves/RW-40/PM/stub10-pins-threads/mutations.log` | `e49f141bd6f0fcaf6381cf9a17fb5c0b` |
| `waves/RW-40/PM/stub10-pins-threads/REPORT.md` | `818d23bedcfada42897a6cde1de4b5c1` |

---

## 7. Риски и границы

- Модуль полностью автономен и готов к интеграции в `actions.js`/`app.js` на этапе координатора/Team Lead без вмешательства в текущие ветки других воркеров.
- Поддерживает как автономный запуск через переданный `document`/DOM-шим, так и работу в реальном браузере.
