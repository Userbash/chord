# Отчёт по карточке `task-148` (`FE-P0-03`): монтирование `createNotificationsController` в `app.js`

- **Batch / Карточка**: `RW-40` / `G2` / `task-148` (`FE-P0-03` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Воркер**: `term_746391ae-6bbf-4ce0-82ea-75e9fc260cb2` (`G2 Frontend — WebView UI & DOM/A11y`)
- **Статус**: `DONE` (Strict TDD: Red -> Green -> TypeScript -> Mutation checks)

---

## 1. Сделано

1. **Экспорт и инициализация `createNotificationsController` в `rust/gui-spike/ui/app.js`**:
   - Реализован и экспортирован `createNotificationsController(options)` в `rust/gui-spike/ui/app.js` поверх базового контроллера `createNotificationSettingsController` и чистых функций `computeGuildRailIndicatorState`, `computeChannelIndicatorState`, `applyChannelIndicatorToDom`, `resolveEffectiveNotifications`, `evaluateMuteState`, `buildMuteConfig`, `mergeGuildSettingsUpdate`, `normalizeChannelOverrides`, `MUTE_TIMER_OPTIONS`, `MESSAGE_NOTIFICATION_LEVELS` из `rust/gui-spike/ui/features/notifications.js`.
   - При старте приложения (`boot()`) создаётся экземпляр `createNotificationsController({ document, invoke })`, экспортируемый в `currentApp().notifications`.
   - Вызов `ready()` при загрузке гильдий передаёт полный payload (`guilds`, `read_state`, `user_guild_settings`) в `notifications.hydrateReady(payload)`, а загрузка дерева каналов (`channels({ guildId })`) регистрирует каналы через `notifications.registerGuildChannels(guildId, rows, true)`.
   - Сохранён инвариант `P2-3` / `SET-06/07/08` (ровно 38 `.toggle` / `.toggle-input` в `#settings`): тумблеры внутри `#guild-notifications-dialog` используют изолированные классы `.notif-toggle` / `.notif-toggle-input`.

2. **Отрисовка бейджей непрочитанных сообщений (`.guild-unread-pill`) и счётчиков упоминаний (`1..99` и `'99+'` с `aria-label`) в `#guild-list` и `#channel-list`**:
   - В `rust/gui-spike/ui/index.html` и `tests/support/gui-dom-harness.mjs` синхронно добавлены обёртки `#guild-list` (внутри `#guild-rail`) и `#channel-list` (вокруг `#channel-tree`) с полным сохранением всех существующих DOM ID, классов и инвариантов `gui-spike-dom.test.mjs`.
   - В `#guild-list` (`renderGuildIndicators()`) для каждой плитки сервера `.guild-item[data-guild-id]` агрегируются `unread` и `mentionCount` по каналам гильдии с учётом каскада `resolveEffectiveNotifications`:
     - при наличии непрочитанных сообщений (и отсутствии mute сервера) рендерится индикатор `.guild-unread-pill` и атрибут `data-unread="true"`;
     - при `mentionCount > 0` рендерится `.guild-mention-badge` с текстом `1..99` или `'99+'` (при `mentionCount > 99`) и русским `aria-label` со склонением (`"1 упоминание"`, `"3 упоминания"`, `"5 упоминаний"`, `"142 упоминания"`).
   - В `#channel-list` (`renderChannelIndicators()`) для каждой строки канала `.channel-item[data-channel-id]` применяется `computeChannelIndicatorState` + `applyChannelIndicatorToDom`, рендерится `.guild-unread-pill.channel-unread-pill` для непрочитанных незаглушённых каналов и `.mention-badge` с текстом `1..99` / `'99+'` и `aria-label`.

3. **Автоматический вызов `ackChannel(channelId, lastMessageId)` (`ack_channel_message`) при переключении активного текстового канала**:
   - В `chat.openChannel` при открытии реального текстового канала с известным `read_state` / `last_message_id` автоматически вызывается `notifications.ackChannel(channelId, lastMessageId)`.
   - `ackChannel(channelId, lastMessageId)` отправляет IPC-команду `ack_channel_message` (зарегистрированную в `rust/gui-spike/src/main.rs` в `tauri::generate_handler!`, сохраняя инвариант `INT-07: UI invoke ⊆ generate_handler!`), сбрасывает `unread: false` и `mentionCount: 0` для канала и мгновенно пересчитывает индикаторы канала и гильдии.

4. **Контекстное меню гильдии (`#guild-notif-context-menu`) и очистка при `destroy()`**:
   - Создано контекстное меню уведомлений сервера `#guild-notif-context-menu` (`role="menu"`, открывается через `contextmenu` на плитке `.guild-item` или `notifications.openGuildContextMenu({ guildId })`):
     - пресеты заглушения сервера (`role="menuitemradio"`, `data-action="guild-mute-preset"`): `15m` (`900`), `1h` (`3600`), `3h` (`10800`), `8h` (`28800`), `24h` (`86400`), `indefinitely` (`-1`) и кнопка снятия заглушения (`data-action="guild-unmute"`);
     - переключатели уровня уведомлений сервера (`role="menuitemradio"`, `data-action="guild-notif-level"`): `0` (`All` / «Все сообщения»), `1` (`Mentions` / «Только @упоминания»), `2` (`Nothing` / «Ничего»);
     - пункт открытия модалки настроек уведомлений сервера (`data-action="guild-open-notif-modal"` -> `#guild-notif-modal`);
     - все мутации вызывают `update_guild_notification_settings` с каноническим PATCH-пейлоадом `/users/@me/guilds/settings` и обновляют индикаторы `data-muted` / `.guild-unread-pill` на плитке сервера;
     - метод `destroy()` удаляет DOM-узлы `#guild-notif-context-menu`, `#guild-notifications-dialog`, `#channel-notif-menu`, очищает внутренние коллекции и блокирует последующие вызовы.

---

## 2. Red-прогон (Strict TDD)

- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-148-mount-notifications-app/red.log`
- **Команда**:
  ```sh
  node --test tests/unit/notifications.test.mjs tests/unit/gui-spike-features.test.mjs
  ```
- **Результат до написания продуктового кода**: `157 pass, 5 fail` — упали все 5 новых контрактных тестов `FE-P0-03 / task-148` (отсутствие экспорта `createNotificationsController` в `app.js`, отсутствие `#guild-list` и `#channel-list`, отсутствие `.guild-unread-pill` и `ackChannel`, отсутствие `#guild-notif-context-menu`).

---

## 3. Green-прогон и проверка типов

- **Лог**: `/home/sanya/orca/project/waves/RW-40/G2/task-148-mount-notifications-app/green.log`
- **Команды**:
  ```sh
  node --test tests/unit/notifications.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-dom.test.mjs
  npx tsc -p tsconfig.json
  npx tsc -p rust/gui-spike/ui/tsconfig.json
  ```
- **Результат**:
  - `183 pass, 0 fail, 0 skipped` (`24 suites`)
  - Дополнительный прогон смежных тестов (`tests/unit/guild-rail.test.mjs`, `tests/unit/read-state-ui.test.mjs`, `tests/unit/server-menu.test.mjs`, `tests/unit/channel-switch.test.mjs`): `67 pass, 0 fail`
  - `npx tsc -p tsconfig.json`: `0 errors` (`exit 0`)
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json`: `0 errors` (`exit 0`)

---

## 4. Мутационная проверка (TEST-POLICY.md)

Проведены 3 независимые мутации продуктового кода в `rust/gui-spike/ui/app.js` (с последующим откатом):
1. **Мутация M1** (`if (effectiveGuildUnread)` -> `if (false && effectiveGuildUnread)` в `renderGuildIndicators`): отключена отрисовка `.guild-unread-pill` в `#guild-list` -> **тест упал (`M1_EXIT=1`)**.
2. **Мутация M2** (`if (channelId && notificationsRef?.shouldAutoAck(channelId))` -> `if (false && ...)` в `chat.openChannel`): отключён автоматический вызов `ackChannel` при переключении текстового канала -> **тест упал (`M2_EXIT=1`)**.
3. **Мутация M3** (`await wrappedInvoke("update_guild_notification_settings", ...)` -> `if (false) await wrappedInvoke(...)` в `applyGuildSettingsPatch`): отключена отправка IPC-команды `update_guild_notification_settings` из контекстного меню гильдии -> **тест упал (`M3_EXIT=1`)**.

---

## 5. Изменённые файлы и MD5

| Файл | MD5 | Назначение изменений |
|---|---|---|
| `rust/gui-spike/ui/app.js` | `a62ca7d9f3cb2e35d222ef6102be4583` | Реализация и экспорт `createNotificationsController`, монтирование в `boot()`, рендер `.guild-unread-pill` и бейджей упоминаний в `#guild-list` и `#channel-list`, авто-`ackChannel` при переключении канала, контекстное меню гильдии `#guild-notif-context-menu` и `destroy()` |
| `rust/gui-spike/ui/index.html` | `a7d0d608367b42d370dcf7a02619bcec` | Добавлены контейнеры `#guild-list` (внутри `#guild-rail`) и `#channel-list` (вокруг `#channel-tree`) |
| `tests/support/gui-dom-harness.mjs` | `9d3d03a039e4f634af7292bc4f917411` | Синхронизирована разметка `#guild-list` и `#channel-list` с `ui/index.html`, добавлен обработчик `ack_channel_message` в `GuiDom.invoke` |
| `tests/unit/gui-spike-features.test.mjs` | `9ef17ae00ef203e41eb96d5180143fbd` | Добавлен сьют из 4 интеграционных тестов `FE-P0-03 / task-148` для `app.js` |
| `tests/unit/notifications.test.mjs` | `c916642324aec74f4c957c520477e749` | Добавлен контрактный тест `createNotificationsController` (`#guild-list`/`#channel-list`, `ackChannel`, `#guild-notif-context-menu`, `destroy()`) |

---

## 6. Риски и совместимость

- **Инвариант `INT-07: UI invoke ⊆ generate_handler!`**: полностью соблюдён (`ack_channel_message`, `update_guild_notification_settings`, `update_channel_notification_override` входят в `tauri::generate_handler!` в `rust/gui-spike/src/main.rs`).
- **Инвариант `P2-3` / `SET-06/07/08` (38 тумблеров в `#settings`)**: полностью соблюдён.
- **Совместимость с `FE-P0-01 / task-133` (`createDmFriendsController`)**: оба контроллера работают совместно в `boot()`, хук `onReadyPayload` в `createGuildRail` гидрирует и `notifications`, и `dmFriends`.
