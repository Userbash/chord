# REVIEW-68A: Техническое ревью карточки task-151 (`FE-P0-05`)

- **Карточка**: `task-151` (`FE-P0-05`: Mount `createAttachmentsController` в `rust/gui-spike/ui/features/composer.js` и `attachments.js`)
- **Компоненты**:
  - `rust/gui-spike/ui/features/composer.js`
  - `rust/gui-spike/ui/features/attachments.js`
  - `tests/unit/attachments-ui.test.mjs`
- **Ревьюер**: Технический ревьюер (Review 68A)
- **Дата проверки**: 2026-10-30
- **Ворктри**: `/home/user/orca/workspaces/project/feature-task-151-mount-attachments-composer` (`HEAD = 4e24048`)

---

## 1. Резюме и вердикт

- **Вердикт**: **PASS / Ready for Merge**
- **Уровень дефектов**: **P3** (замечаний уровня P0, P1, P2 не обнаружено).
- Все функциональные требования и инварианты из `docs/specs/M-11-ATTACHMENTS.md`, `/home/user/orca/project/waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` и `/home/user/orca/project/waves/RW-40/ATTACH-RE.md` полностью соблюдены:
  - Лимиты очереди: не более 10 файлов (`MAX_ATTACHMENT_COUNT = 10`), размер файла `<= 25 МБ` (`MAX_ATTACHMENT_SIZE_BYTES = 26_214_400` байт).
  - Приём файлов через кнопку прикрепления `#composer-attach-btn` (`<input type="file" id="composer-file-input">`), drag-and-drop (`dragenter`/`dragover`/`dragleave`/`drop`) и вставку из буфера обмена (`paste`).
  - Рендеринг полки `#attachment-shelf` непосредственно над полем ввода `#composer-input` с карточками `.attachment-card`, переключателем спойлера (`SPOILER_` префикс и флаг `IS_SPOILER = 8`), полем описания (`alt text` / `description`) и кнопкой удаления (`.attachment-remove-btn`).
  - Вызов `uploadAll()` -> IPC-команда `upload_attachments` при отправке сообщения, передача массива `attachments` в payload.
  - Разрешение отправки сообщения с пустым текстом (`""`), если в очереди есть хотя бы 1 вложение (`queuedCount > 0`), и запрет пустой отправки без текста и без вложений.
  - Корректная очистка очереди после отправки и полное снятие слушателей/очистка DOM при `destroy()`.
- Обратная совместимость с существующим `createComposer` (20 МиБ chip-очередь, 29 тестов `gui-spike-dom.test.mjs` и `composer.test.mjs`) полностью сохранена.

---

## 2. Результаты тестов и статической типизации

1. **Unit-тесты**:
   ```sh
   node --test tests/unit/attachments-ui.test.mjs tests/unit/composer.test.mjs tests/unit/gui-spike-dom.test.mjs
   ```
   - **Результат**: **40 pass, 0 fail, 4 suites** (exit code 0, длительность ~278ms).
   - Все 11 тестов в `tests/unit/attachments-ui.test.mjs` (4 теста `createAttachmentsController` + 7 тестов интеграции в `composer.js`) проходят успешно.
   - Все 29 тестов `tests/unit/gui-spike-dom.test.mjs` и `tests/unit/composer.test.mjs` проходят без регрессий.

2. **Статическая типизация**:
   - `npx tsc -p rust/gui-spike/ui/tsconfig.json`: **0 ошибок** (exit code 0).
   - `// @ts-check` и полная JSDoc-типизация присутствуют в `rust/gui-spike/ui/features/composer.js` и `rust/gui-spike/ui/features/attachments.js`.
   - *(Примечание: 16 ошибок в глобальном `tsconfig.json` вызваны исключительно сторонним файлом `tests/unit/dispatch_ui.test.mjs` из ветки L-02-UI на baseline `4e24048` и зафиксированы в `REVIEW-59A.md`; назначенные файлы task-151 ошибок типизации не содержат).*

---

## 3. Проверка функциональных требований и контракта

- [x] **Кнопка прикрепления и ввод файлов**:
  - Кнопка `#composer-attach-btn` (`[data-action="attach"]`) открывает диалог выбора файлов через `#composer-file-input` (`multiple`).
  - Поддерживаются события drag-and-drop с подсветкой дропзоны (`dragover-active`) и извлечение файлов из `event.dataTransfer`.
  - Поддерживается событие `paste` с фильтрацией элементов `file` из `event.clipboardData`.
- [x] **Валидация лимитов**:
  - Максимум 10 файлов в очереди: при превышении добавляются только первые файлы до лимита 10, на остальные выводится информативное сообщение об ошибке.
  - Максимум 25 МБ (`26_214_400` байт): файлы, превышающие размер, отсекаются с сообщением об ошибке без добавления в очередь.
  - Пустые файлы (0 байт) отсекаются.
- [x] **Полка вложений `#attachment-shelf`**:
  - Размещается строго над полем `#composer-input`.
  - Карточки файлов `.attachment-card` отображают имя, форматированный размер, бейдж `СПОЙЛЕР` при включении.
  - Переключатель спойлера `.attachment-spoiler-toggle`: добавляет/убирает префикс `SPOILER_` в имени файла и устанавливает бит `ATTACHMENT_FLAG_IS_SPOILER` (8).
  - Поле ввода описания `.attachment-alt-input`: сохраняет `description` (alt text) для вложения.
  - Кнопка удаления `.attachment-remove-btn`: удаляет файл из очереди и обновляет полку.
- [x] **Отправка и IPC выгрузка**:
  - При отправке (кнопка отправки, submit формы, `Enter` без `Shift`) вызывается `uploadAll(channelId)` -> IPC-команда `upload_attachments`.
  - При ошибке выгрузки очередь вложений и введённый текст сообщения сохраняются (нет потери данных пользователя).
  - Поддерживается отправка сообщения с пустым текстом (`""`), если в очереди есть `>= 1` файл.
  - Пустая отправка (текст пустой и 0 вложений) блокируется.
  - После успешной отправки поле ввода и очередь вложений очищаются (`attachments.clear()`).
- [x] **Жизненный цикл**:
  - Метод `destroy()` отписывает все навешенные DOM-обработчики (`submit`, `keydown`, `click`, `drag*`, `paste`, `change`) и очищает полку.

---

## 4. Мутационное тестирование (5/5 KILLED)

Проведено 5 независимых мутаций в коде `rust/gui-spike/ui/features/attachments.js` и `rust/gui-spike/ui/features/composer.js`. Все 5 мутаций обнаружены и «убиты» тестами (`5/5 KILLED`):

| № | Файл | Описание мутации | Код возврата | Результат тестов |
|---|---|---|---|---|
| **M1** | `attachments.js` | Уменьшение лимита файлов `MAX_ATTACHMENT_COUNT = 10 -> 9` | `exit 1` | **KILLED** (3 fail: тест лимитов и тест кнопки/D&D) |
| **M2** | `attachments.js` | Изменение префикса спойлера `SPOILER_PREFIX = "SPOILER_" -> "HIDDEN_"` | `exit 1` | **KILLED** (5 fail: тесты хелперов, полки, uploadAll и отправки) |
| **M3** | `composer.js` | Запрет отправки пустого текста даже при наличии вложений (`queuedCount > 0`) | `exit 1` | **KILLED** (1 fail: тест разрешения отправки с пустым текстом) |
| **M4** | `composer.js` | Пропуск `uploadAll()` при отправке сообщения (`[]` вместо вызова IPC) | `exit 1` | **KILLED** (3 fail: тесты отправки, пустого текста и обработки ошибки) |
| **M5** | `composer.js` | Пропуск очистки `attachments.clear()` после успешной отправки | `exit 1` | **KILLED** (2 fail: тесты отправки и сохранения очереди при ошибке) |

После проведения мутационного тестирования файлы возвращены в исходное состояние, побайтно сверены контрольные суммы MD5.

---

## 5. Контрольные суммы файлов (MD5)

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/composer.js` | modified | `db914a85335b56913710ac513be37c2e` |
| `rust/gui-spike/ui/features/attachments.js` | added | `324ded5e39847f4e99532dc861d1269a` |
| `tests/unit/attachments-ui.test.mjs` | added | `9380e9230098f0aabf9fca1423b26b5d` |

---

## 6. Заключение

Карточка `task-151` выполнена в соответствии с правилами проекта (`Strict TDD`, `TEST-POLICY.md`, изоляция файлов). Все требования и инварианты покрыты тестами, мутационный барьер 5/5 пройден.
Рекомендация: **Ready for Merge**.
