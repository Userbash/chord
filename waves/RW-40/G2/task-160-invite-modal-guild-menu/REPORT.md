# Отчёт воркера: task-160 (FE-P1-05: Модалка приглашения на сервер/канал #invite-modal и контекстное меню сервера #guild-context-menu)

- **Волна / Отдел**: `RW-40` (Отдел G2 Frontend: WebView UI & DOM/A11y)
- **Карточка**: `task-160` (`FE-P1-05` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Спецификации и контракты**:
  - `waves/RW-40/DESIGN/INVITES-SPEC.md` (§1 DOM-структура, a11y, clipboard);
  - `waves/RW-40/RE-DEEP-REA.md` §4.3 (`POST /channels/{id}/invites`, опции max_age/max_uses);
  - `rust/gui-spike/src/invites.rs`, `rust/gui-spike/src/notifications.rs`, `rust/gui-spike/src/main.rs` (`create_invite`, `leave_guild`, `resolve_invite`, `accept_invite`, `join_guild`).
- **Ворктри**: `/home/sanya/orca/workspaces/project/feature-task-160-invite-modal-guild-menu`

---

## 1. Что сделано

1. **Strict TDD (Red-first)**:
   - В `tests/unit/guild-modals.test.mjs` разработан комплексный набор из 18 поведенческих unit-тестов, проверяющих:
     - Экспорт констант допустимых значений `VALID_INVITE_MAX_AGES`, `VALID_INVITE_MAX_USES` и вспомогательной функции `normalizeInviteCode`;
     - Контекстное меню `#guild-context-menu` (`role='menu'`, пункты `role='menuitem'` и `role='menuitemcheckbox'`: `invite`, `notifications`, `hide-muted`, `leave-guild`);
     - Открытие контекстного меню через `openContextMenu()` и по событию `contextmenu` на триггере через `attachTrigger()`;
     - Полную клавиатурную навигацию меню: циклическую `ArrowDown`/`ArrowUp`, переход на начало/конец `Home`/`End`, закрытие по `Escape` с возвратом фокуса на триггер;
     - Действия пунктов: вызовы `openInviteModal()`, `onOpenNotifications()`, переключение `aria-checked` тумблера `hide-muted`, открытие `openLeaveModal()`;
     - Модалку создания инвайта `#invite-modal` (`role='dialog'`, `aria-modal='true'`): генерация инвайта по умолчанию (`86400`, `0`, `false`), отображение ссылки в `#invite-link-input`;
     - Кнопку копирования `#invite-copy-btn`: копирование в clipboard и смена текста на «Скопировано!»;
     - Смену параметров срока действия (`#invite-max-age-select`), лимита использований (`#invite-max-uses-select`) и чекбокса временного членства (`#invite-temporary-checkbox`) с автоматическим пересозданием инвайта;
     - Закрытие модалки инвайта по кнопке закрытия и клавише `Escape`;
     - Модалку выхода `#leave-guild-modal` (`role='dialog'`, `aria-modal='true'`): подтверждение с вызовом `leave_guild` (с `lurking: false`) и отмену выхода;
     - Модалку присоединения `#join-guild-modal` (`role='dialog'`, `aria-modal='true'`): валидацию кода/ссылки, вызовы `resolve_invite` и `accept_invite`/`join_guild`;
     - Очистку состояния и снятие обработчиков в `destroy()`.
   - Зафиксировано падение до реализации в `/home/sanya/orca/project/waves/RW-40/G2/task-160-invite-modal-guild-menu/red.log`.

2. **Реализация модуля (`rust/gui-spike/ui/features/guild-modals.js`)**:
   - Модуль снабжён директивой `// @ts-check` и строгими JSDoc-типами;
   - Реализована функция `createGuildModalsController(options)` со всеми требуемыми компонентами и методами;
   - Локальная функция IPC-вызова названа `invoker(...)`, что обеспечивает корректное прохождение статического анализатора INT-07 (проверка литеральных команд в `generate_handler!`);
   - Все 5 вызываемых команд (`create_invite`, `leave_guild`, `resolve_invite`, `accept_invite`, `join_guild`) строго входят в `generate_handler!` спайка.

---

## 2. Red-прогон (до реализации)

- **Команда**: `node --test tests/unit/guild-modals.test.mjs`
- **Результат**: `tests 1 | pass 0 | fail 1` (модуль не найден)
- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-160-invite-modal-guild-menu/red.log`

---

## 3. Green-прогон и проверки типов (после реализации)

- **Команды**:
  - `node --test tests/unit/guild-modals.test.mjs` → **18 pass, 0 fail** (exit 0)
  - `node --test tests/unit/guild-modals.test.mjs tests/unit/gui-spike-dom.test.mjs` → **57 pass, 0 fail** (exit 0)
  - `npx tsc -p tsconfig.json` → **0 ошибок** (exit 0)
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json` → **0 ошибок** (exit 0)
  - `node --test --test-name-pattern="INT-07" tests/unit/gui-spike-features.test.mjs` → **11 pass, 0 fail** (exit 0)
  - `node --test tests/meta/test-policy.test.mjs tests/meta/ipc-path.test.mjs` → **29 pass, 0 fail** (exit 0)
- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-160-invite-modal-guild-menu/green.log`

---

## 4. Мутационная проверка (5/5 мутаций)

Все 5 целевых семантических мутаций пойманы тестами (`5/5 KILLED`), после чего файлы проверены по контрольным суммам:

| № | Инвариант | Мутация | Результат |
|---|---|---|---|
| 1 | Открытие контекстного меню (`openContextMenu`) | Убрано снятие атрибута `hidden` у `#guild-context-menu` | **KILLED** (5 тестов упали: 03, 04, 05, 06, 18) |
| 2 | Копирование инвайта (`copyInviteLink`) | Убрана запись текста в буфер обмена (`clipboard.writeText`) | **KILLED** (тест 12 упал: `0 !== 1`) |
| 3 | Клавиатурная навигация `ArrowDown` | Заменён циклический переход `(index + 1)` на переход к 0 | **KILLED** (тест 04 упал: активный элемент не сменился) |
| 4 | Подтверждение выхода из сервера | Убран вызов `invoker("leave_guild", ...)` | **KILLED** (тест 15 упал: `leave_guild не вызван`) |
| 5 | Валидация кода приглашения | Пропуск проверки `CODE_RE.test(code)` в `normalizeInviteCode` | **KILLED** (тесты 01 и 17 упали на невалидных символах) |

- **Лог мутаций**: `/home/sanya/orca/project/waves/RW-40/G2/task-160-invite-modal-guild-menu/mutations.log`

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/features/guild-modals.js` | `9350dbfd35a539d77457b63150e99be2` |
| `tests/unit/guild-modals.test.mjs` | `323443098b143950935dd53ec185953d` |

---

## 6. Риски и замечания

- Изменения полностью изолированы в назначенных файлах воркера. Общие файлы не затрагивались.
- Локальная функция IPC-вызова названа `invoker(...)`, контракт статического анализатора INT-07 соблюдён.
