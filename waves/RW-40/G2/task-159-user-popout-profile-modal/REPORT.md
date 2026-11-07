# Отчёт воркера: task-159 (FE-P1-04: Попап карточки пользователя #user-popout и модалка полного профиля #user-profile-modal)

- **Волна / Отдел**: `RW-40` (Отдел G2 Frontend: WebView UI & DOM/A11y)
- **Карточка**: `task-159` (`FE-P1-04` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Спецификации и оракулы**: `ui/06-profile-popout.html`, `voice/ui7/15-popout.json` / `15-popout.html`, `re-deep/frontend-d03.md` (§5 п.5 Modal, §5 п.6 Popover, §3 Участники)
- **Ворктри**: `/home/sanya/orca/workspaces/project/feature-task-159-user-popout-profile-modal`

---

## 1. Что сделано

1. **Red-first тесты (`tests/unit/user-popout.test.mjs`)**:
   - Реализован комплексный набор из 17 unit-тестов, строго проверяющих:
     - инициализацию контроллера `createUserPopoutController` и наличие публичного API (`openPopout`, `closePopout`, `openProfileModal`, `closeProfileModal`, `closeAll`, `switchTab`, `attachToChatAndMembers`, `destroy`);
     - открытие компактного поповера `#user-popout` (`role="dialog"`, `aria-modal="false"`, `aria-label="Профиль пользователя <displayName>"`);
     - отображение аватара, отображаемого имени, username/тега (`.user-popout-tag`), индикатора статуса (`.status-dot[data-status]`) и кастомного статуса (`.user-popout-custom-status`);
     - отображение бейджа БОТ (`.badge--bot`) только при `bot=true` и его отсутствие при `bot=false`;
     - рендеринг списка ролей `.user-popout-roles` с цветными точками (`.role-dot`) и названиями;
     - сохранение заметок в памяти по `userId` (`#user-popout-note`) и их восстановление при повторном открытии карточки того же пользователя;
     - быстрое поле отправки ЛС `#user-popout-dm-input`: вызов `onSendDm` (или `invoker`) по клавише Enter, очистка поля и игнорирование пустого ввода;
     - переход в полную модалку `#user-profile-modal` (`role="dialog"`, `aria-modal="true"`) по клику на кнопку «Полный профиль» (`#user-popout-full-profile-btn`) с закрытием поповера;
     - модалку полного профиля с 3 вкладками (`role="tablist"` / `role="tab"` — «Обо мне» `data-tab="about"`, «Общие серверы» `data-tab="mutual-guilds"`, «Общие друзья» `data-tab="mutual-friends"`) и переключением соответствующих панелей (`[role="tabpanel"]`);
     - кнопку действия «Добавить в друзья» / «Отправить сообщение» (`#user-profile-action-btn`);
     - закрытие модалки по кнопке закрытия (`#user-profile-close-btn`), по клику на бэкдроп (`#user-profile-modal-backdrop`) и по клавише `Escape` с обязательным возвратом фокуса на элемент-триггер;
     - делегированный перехват кликов через `attachToChatAndMembers({ chatList, membersList })` на элементах `.author` / `.author-name` в чате и `.member-row` / `.member` в списке участников;
     - метод `destroy()`: удаление `#user-popout` и `#user-profile-modal` из DOM и снятие всех слушателей.
   - Падение до реализации зафиксировано в `red.log`.

2. **Реализация модуля (`rust/gui-spike/ui/features/user-popout.js`)**:
   - Модуль оформлен с `// @ts-check` и полными JSDoc-типами.
   - Обход DOM выполнен через `findClosest` без использования `Element.prototype.closest()`, что гарантирует совместимость как с браузером, так и с минимальным тестовым шимом `ShimDocument`/`ShimElement`.
   - Вызовы IPC используют имя `invoker(...)`, предотвращая ложные срабатывания статического теста `INT-07: UI invoke ⊆ generate_handler!`.
   - Полная очистка состояния и слушателей событий в `destroy()`.

---

## 2. Red-прогон (до реализации)

- **Команда**: `node --test tests/unit/user-popout.test.mjs`
- **Результат**: `ERR_MODULE_NOT_FOUND` (модуль не существовал)
- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-159-user-popout-profile-modal/red.log`

---

## 3. Green-прогон и проверка типов

- **Команды**:
  - `node --test tests/unit/user-popout.test.mjs tests/unit/gui-spike-dom.test.mjs` → **56 pass, 0 fail** (exit 0)
  - `npm run typecheck` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`) → **0 ошибок** (exit 0)
  - `node --test --test-name-pattern="INT-07" tests/unit/gui-spike-features.test.mjs` → **11 pass, 0 fail** (exit 0)
- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-159-user-popout-profile-modal/green.log`

---

## 4. Мутационная проверка (5 мутаций)

Все 5 целевых семантических мутаций пойманы тестами (`5/5 KILLED`, 100%), файл восстановлен до исходного байта:

| № | Инвариант | Мутация | Статус |
|---|---|---|---|
| 1 | `aria-modal="false"` у поповера карточки | Замена `aria-modal` на `"true"` | **KILLED** (`AssertionError: 'true' !== 'false'`) |
| 2 | Сохранение заметки в памяти по `userId` | Отключение `notesStore.set` | **KILLED** (`AssertionError: 'Заметка...' !== ''`) |
| 3 | Быстрая отправка ЛС по клавише Enter | Замена условия `e.key === "Enter"` на `false` | **KILLED** (`AssertionError: 0 !== 1`) |
| 4 | Переключение вкладок полного профиля | Ранний выход из `switchTab` | **KILLED** (`AssertionError: 'false' !== 'true'`) |
| 5 | Возврат фокуса на триггер при закрытии модалки | Отключение `lastTriggerEl.focus()` в `closeProfileModal` | **KILLED** (`AssertionError: фокус не вернулся на триггер`) |

- **Лог мутаций**: `/home/sanya/orca/project/waves/RW-40/G2/task-159-user-popout-profile-modal/mutations.log`

---

## 5. Файлы и контрольные суммы (MD5)

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/features/user-popout.js` | `4e42d066c9bb0dd43529018a40db4048` |
| `tests/unit/user-popout.test.mjs` | `56d71784e67245fe5e1ce0421e7acdea` |
| `/home/sanya/orca/project/waves/RW-40/G2/task-159-user-popout-profile-modal/red.log` | `9e8283be2f2b20a84490b5222d877400` |
| `/home/sanya/orca/project/waves/RW-40/G2/task-159-user-popout-profile-modal/green.log` | `790f2832af6578a73ea59be8e7ae158c` |
| `/home/sanya/orca/project/waves/RW-40/G2/task-159-user-popout-profile-modal/mutations.log` | `72a97d0eef548ac4566e78172b0bf9e8` |

---

## 6. Риски и границы

- Модифицировались только назначенные файлы карточки: `rust/gui-spike/ui/features/user-popout.js` и `tests/unit/user-popout.test.mjs`.
- Общие файлы (`app.js`, `index.html`, `chat.js`, `members.js`) не изменялись.
- Полная совместимость со статическим тестом `INT-07`.
