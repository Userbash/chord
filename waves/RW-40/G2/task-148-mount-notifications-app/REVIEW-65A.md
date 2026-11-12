# Техническое ревью (Review 65A): карточка `task-148` (`FE-P0-03`)

- **Карточка**: `task-148` (`FE-P0-03` из `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md`)
- **Компонент**: `rust/gui-spike/ui/app.js` (монтирование `createNotificationsController`), `rust/gui-spike/ui/index.html`, `tests/support/gui-dom-harness.mjs`
- **Ревьюер**: `term_29d48c1c-0fe9-478d-a56f-b99127b084ad` (Technical Reviewer 65A)
- **Воркер**: `term_746391ae-6bbf-4ce0-82ea-75e9fc260cb2`
- **Отчёт воркера**: `/home/user/orca/project/waves/RW-40/G2/task-148-mount-notifications-app/REPORT.md`
- **Вердикт**: **PASS / Ready for Merge** (Дефекты: P0=0, P1=0, P2=0, P3=0)

---

## 1. Проверка контрольных сумм файлов (MD5)

Все 5 файлов соответствуют отчёту воркера и актуальному рабочему дереву:

| Файл | MD5 | Статус |
|---|---|---|
| `rust/gui-spike/ui/app.js` | `a62ca7d9f3cb2e35d222ef6102be4583` | Совпадает |
| `rust/gui-spike/ui/index.html` | `a7d0d608367b42d370dcf7a02619bcec` | Совпадает |
| `tests/support/gui-dom-harness.mjs` | `9d3d03a039e4f634af7292bc4f917411` | Совпадает |
| `tests/unit/gui-spike-features.test.mjs` | `9ef17ae00ef203e41eb96d5180143fbd` | Совпадает |
| `tests/unit/notifications.test.mjs` | `c916642324aec74f4c957c520477e749` | Совпадает |

---

## 2. Результаты прогона тестов и проверка типов

1. **Целевой сьют тестов**:
   ```sh
   node --test tests/unit/notifications.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/dm-friends-ui.test.mjs
   ```
   - **Результат**: `202 pass, 0 fail, 0 skipped` (28 suites), время выполнения ~2.18s.
   - Все 4 новых интеграционных теста карточки `FE-P0-03` в `gui-spike-features.test.mjs` и контрактный тест в `notifications.test.mjs` проходят успешно.

2. **Статическая проверка типов TypeScript**:
   ```sh
   npx tsc -p tsconfig.json
   ```
   - **Результат**: `0 errors` (код выхода `0`).

---

## 3. Проверка инвариантов архитектуры и спецификаций

- **Инвариант INT-07 (`UI invoke ⊆ generate_handler!`)**:
  - `ackChannel` отправляет IPC-команду `ack_channel_message` (вызываемую через нормализованный вызов), `update_guild_notification_settings` и `update_channel_notification_override` зарегистрированы в `main.rs`.
  - Тест `INT-07: UI invoke ⊆ generate_handler!` проходит успешно (`11 pass, 0 fail`).
- **Инварианты DOM ID и WAI-ARIA**:
  - Новые элементы `#guild-list` и `#channel-list` добавлены синхронно в `rust/gui-spike/ui/index.html` и `tests/support/gui-dom-harness.mjs` без нарушения существующих селекторов и структуры.
  - Инвариант `P2-3` / `SET-06/07/08` (ровно 38 `.toggle` в `#settings`): тумблеры модалки уведомлений используют изолированные классы `.notif-toggle` / `.notif-toggle-input`.
- **Совместимость с DM/Friends (`task-133` / `FE-P0-01`)**:
  - Совместная работа `createDmFriendsController` и `createNotificationsController` в `boot()` проверена сьютом `tests/unit/dm-friends-ui.test.mjs` (`39 pass, 0 fail`).
- **Бейджи и индикаторы**:
  - Пилюля `.guild-unread-pill` отображается для непрочитанных незаглушённых гильдий/каналов и скрывается при заглушении или после прочтения (`ackChannel`).
  - Бейджи упоминаний корректно форматируют счётчики `1..99` и `'99+'` с русским склонением `aria-label` (`"1 упоминание"`, `"3 упоминания"`, `"5 упоминаний"`, `"142 упоминания"`).
- **Контекстное меню гильдии `#guild-notif-context-menu`**:
  - Реализованы пресеты Mute (`15m`, `1h`, `3h`, `8h`, `24h`, `indefinitely`), снятие mute (`guild-unmute`), уровни уведомлений `All`/`Mentions`/`Nothing` и пункт вызова параметров уведомлений.
  - Метод `destroy()` корректно удаляет созданные элементы DOM и очищает коллекции.

---

## 4. Мутационная проверка (Mutation Testing 5/5)

Проведено 5 независимых мутаций в `rust/gui-spike/ui/app.js`. Все 5 мутантов убиты тестами (`exit 1`):

1. **M1 (renderGuildIndicators: отключение логики effectiveGuildUnread)**:
   - Замена: `const effectiveGuildUnread = state.unread && !state.isMuted;` -> `const effectiveGuildUnread = false && state.unread && !state.isMuted;`
   - Итог: `M1_EXIT=1` (тест падает: отсутствие `.guild-unread-pill` на гильдии).
2. **M2 (renderGuildIndicators: обнуление текста бейджа упоминаний)**:
   - Замена: `badgeEl.textContent = state.badgeText;` -> `badgeEl.textContent = "";`
   - Итог: `M2_EXIT=1` (тест падает: неверный текст бейджа упоминаний гильдии).
3. **M3 (renderChannelIndicators: повреждение класса пилюли канала)**:
   - Замена: `pillEl.className = "guild-unread-pill channel-unread-pill";` -> `pillEl.className = "channel-unread-pill-broken";`
   - Итог: `M3_EXIT=1` (тест падает: отсутствие `.guild-unread-pill` на канале).
4. **M4 (ackChannel: отключение вызова IPC команды `ack_channel_message`)**:
   - Замена: `return wrappedInvoke(ACK_CHANNEL_IPC_CMD, {` -> `return null; if (false) wrappedInvoke(ACK_CHANNEL_IPC_CMD, {`
   - Итог: `M4_EXIT=1` (тест падает: отсутствие вызова `ack_channel_message`).
5. **M5 (openGuildContextMenu: блокировка снятия атрибута hidden у контекстного меню гильдии)**:
   - Замена: `guildCtxMenu.removeAttribute("hidden");` -> `// guildCtxMenu.removeAttribute("hidden");`
   - Итог: `M5_EXIT=1` (тест падает: меню остаётся скрытым при правом клике).

После выполнения мутаций файл `rust/gui-spike/ui/app.js` восстановлен в исходное состояние (MD5: `a62ca7d9f3cb2e35d222ef6102be4583`).

---

## 5. Заключение

Карточка `task-148` выполнена строго по TDD, контрактным спецификациям и правилам проекта. Код готов к слиянию в `unstable`.
