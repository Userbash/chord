# Отчёт по карточке task-133 (FE-P0-01): Монтаж `createDmFriendsController` в `app.js` и DOM-хостов в `index.html`

- **Волна / Отдел / Карточка**: `RW-40` / `G2 Frontend Specialist` / `task-133` (`FE-P0-01` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Ветка**: `feature/task-133-mount-dm-friends-app`
- **Статус**: Выполнено (Strict TDD: Red → Green → Mutation Kill)

## 1. Что сделано

1. **Статическая разметка (`rust/gui-spike/ui/index.html`) и тестовый DOM-шим (`tests/support/gui-dom-harness.mjs`)**:
   - Добавлены 5 обязательных хостов согласно `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md`:
     - `#home-rail-btn` (`.brand.home-rail-btn` с `.brand-monogram` и `.guild-mention-badge`) в `#guild-rail`;
     - `#dm-channel-list` (`.channel-list.dm-channel-list[role="list"][hidden]`) в `#channel-tree .tree-scroll`;
     - `#friends-view` (`.friends-view[role="region"][aria-label="Друзья"][hidden]`) в `main.content`;
     - `#group-dm-dialog` (`.dialog-backdrop[role="presentation"][hidden]`) в `body`;
     - `#stranger-confirm-dialog` (`.dialog-backdrop[role="presentation"][hidden]`) в `body`.
   - Сохранены все инварианты `gui-spike-dom.test.mjs` (4 экрана `.screen`, 38 тумблеров `.toggle`, совпадение всех `id` и классов между `index.html` и `gui-dom-harness.mjs`) и `guild-rail.test.mjs` (отсутствие `[data-guild]` и `aria-current="false"` на невыбранных `.guild-item`).

2. **Интеграция `createDmFriendsController` в `rust/gui-spike/ui/app.js`**:
   - Импортированы `createDmFriendsController` и `HOME_GUILD_ID` (`"@me"`) из `./features/dm-friends.js`.
   - Обновлена функция `readLastGuildId(raw)`: поддерживает `lastGuildId === "@me"` и `homeMode === true` для сохранения и восстановления режима Home при старте из `settings_load`.
   - Расширен `createGuildRail`:
     - добавлен метод `selectHome(origin, focus)` и поддержка `focusId === "@me"` в `renderGuilds` / `load`;
     - колбэки `onEnterHome`, `onLeaveHome`, `onReadyPayload` синхронизируют состояние рейла серверов, дерева каналов (`#channel-tree[data-nav-mode="home"|"guild"]`) и центральной области (`#friends-view` ↔ `#screen-chat`);
     - выбор Home (`@me`) пользователем сохраняется через `rememberGuild("@me")` → `settings_save`.
   - При загрузке `ready()` данные `users`, `relationships`, `private_channels` / `privateChannels` и `presences` (включая массивный и объектный форматы) пробрасываются в `createDmFriendsController`.
   - Выбор канала ЛС в `#dm-channel-list` переключает центральную панель на `#screen-chat` и открывает сообщения канала через `chat.openChannel({ id, name })` (`messages_page`).
   - Локальный REST-адаптер `sendDmRest` (с методами `dmFriends.setRestHandler` и `dmFriends.getRestCalls` на `currentApp().dmFriends`) обрабатывает запросы без вызова незарегистрированных в `src/main.rs` IPC-команд (`INT-07` и `tests/meta/ipc-path.test.mjs` 12/12 зелёные).

3. **Тесты (`tests/unit/gui-spike-features.test.mjs`)**:
   - Добавлен сьют `FE-P0-01 / task-133: монтирование Home / ЛС / друзей в app.js и index.html` (12 тестов), проверяющий наличие всех 5 хостов, вход в Home (`@me`), возврат на гильдию, восстановление `lastGuildId: "@me"` и `homeMode: true`, рендер `private_channels` / `relationships` / `presences` из `ready()`, открытие ЛС (`messages_page`), возврат в `#friends-view` через `#dm-nav-friends`, фильтрацию вкладок и поиск друзей, форму «Добавить в друзья», модалки `#group-dm-dialog` и `#stranger-confirm-dialog` (код `80013`) и клавиатурную навигацию без `console.error`/`console.warn`.

## 2. Доказательства TDD (Red → Green → Mutations)

- **Red-прогон** (`/home/sanya/orca/project/waves/RW-40/G2/task-133-mount-dm-friends/red.log`):
  ```text
  node --test tests/unit/gui-spike-features.test.mjs
  ℹ tests 123
  ℹ pass 111
  ℹ fail 12
  ```
  Все 12 новых тестов `FE-P0-01 / task-133` упали до внесения продуктовых правок в `index.html` и `app.js`.

- **Green-прогон** (`/home/sanya/orca/project/waves/RW-40/G2/task-133-mount-dm-friends/green.log`):
  ```text
  node --test tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/dm-friends-ui.test.mjs tests/meta/ipc-path.test.mjs tests/meta/test-policy.test.mjs
  ℹ tests 209
  ℹ suites 28
  ℹ pass 209
  ℹ fail 0
  ```
  Дополнительно с `tests/unit/guild-rail.test.mjs`: `225 pass, 0 fail`.
  Проверка типов `npm run typecheck` (`tsc -p tsconfig.json && tsc -p tsconfig.agent.json`): `0 errors` (exit code 0).

- **Мутационная проверка (3 целевые мутации, все убиты и откачены)**:
  1. Удаление ветки `if (value === HOME_GUILD_ID || bag.homeMode === true) return HOME_GUILD_ID;` в `readLastGuildId` (`app.js`) → **KILLED** (`fail 2`: тесты 4 и 5).
  2. Подмена `privateChannels` на `[]` при монтировании `createDmFriendsController` (`app.js`) → **KILLED** (`fail 4`: тесты 6, 7, 8, 11).
  3. Замена `id="friends-view"` на `id="friends-view-broken"` в `index.html` → **KILLED** (`fail 2`: `gui-spike-dom.test.mjs` и тест 1 `FE-P0-01`).

## 3. Изменённые файлы и MD5

| Файл | MD5 |
|---|---|
| `rust/gui-spike/ui/app.js` | `e83caa7d335328eeb06599d5cd4093e1` |
| `rust/gui-spike/ui/index.html` | `04f3e0d3d532219b8187938172b6fc50` |
| `tests/support/gui-dom-harness.mjs` | `9e3f4f81953ba4e8caf7992b790ef60d` |
| `tests/unit/gui-spike-features.test.mjs` | `116bed757c1bb53e6ac9965dfd680805` |

## 4. Риски и замечания

- Незарегистрированные в `rust/gui-spike/src/main.rs` IPC-команды не вызываются; все проверки `INT-07` (`UI invoke ⊆ generate_handler!`) и `tests/meta/ipc-path.test.mjs` (12/12) зелёные.
- При появлении в Rust-бэкенде выделенных команд REST для управления связями/каналами ЛС достаточно подключить их в адаптер `sendDmRest` в `rust/gui-spike/ui/app.js`.
