# Отчёт по задаче task-174 (BE-P2-14: Desktop Notifications dispatch -> DND check -> D-Bus Notify with actions -> ActionInvoked -> channel deep link + message.id dedup)

## 1. Сводка
- **Отдел/Воркер:** G5 Platform Worker
- **Карточка:** task-174 / BE-P2-14 (`waves/RW-40/PM/BACKLOG-RE-BACKEND.md` §2)
- **Цель:** Маршрутизация десктопных уведомлений сообщений Discord: фильтрация автора, проверка DND, проверка активного сфокусированного канала, фильтрация упоминаний/ЛС, дедупликация `message_id` в ограниченной FIFO-очереди, маскирование токенов/секретов в preview текста, генерация канонического `discord://` deep link для DM/серверов, обработка D-Bus событий `ActionInvoked` и `NotificationClosed`.

## 2. Что сделано
1. В `rust/crates/discord-platform/src/notifications.rs`:
   - Объявлены структуры `IncomingMessageNotification` и enum `NotificationDispatchDecision`.
   - Расширен `NotificationEvent` вариантами `ActionInvoked` и `NotificationClosed` (с сохранением `Action` и `Closed` для обратной совместимости) и добавлен алиас `pub type Event = NotificationEvent`.
   - Реализована функция `redact_secrets` для маскирования чувствительных токенов (`mfa.*`, 3-частных base64 Discord токенов, `Bearer <token>` и `Bearer=<token>`) маркером `<redacted>`.
   - Реализован `NotificationRouter` (с ограниченной FIFO-очередью дедупликации `DEFAULT_DEDUP_CAPACITY = 256`):
     - `evaluate(&mut self, msg: &IncomingMessageNotification) -> NotificationDispatchDecision`:
       * подавляет, если `msg.author_id == self_user_id` (`SuppressedSelfAuthor`);
       * подавляет, если `dnd_enabled == true` (`SuppressedDnd`);
       * подавляет, если `window_focused && active_channel_id == Some(msg.channel_id)` (`SuppressedActiveChannel`);
       * подавляет, если `!msg.mentions_self && !msg.is_dm` (`SuppressedNotMentionOrDm`);
       * подавляет повторные сообщения по `message_id` (`SuppressedDuplicate`);
       * иначе регистрирует `message_id` в ограниченном FIFO, маскирует секреты в теле, собирает `Notification` с действием `("default", "Open")` и вычисляет `deep_link_url` (`discord://-/channels/@me/{channel_id}` для DM или `discord://-/channels/{guild_id}/{channel_id}` для гильдий).
     - `register_sent(&mut self, notification_id: u32, deep_link_url: String)`.
     - `handle_event(&mut self, event: &NotificationEvent) -> Option<String>`: возвращает зарегистрированный URL при вызове действия (`Action` / `ActionInvoked`) и удаляет запись из маппинга при закрытии (`Closed` / `NotificationClosed`).
2. В `rust/crates/discord-platform/tests/notifications_contract.rs`:
   - Добавлены 9 контрактных тестов `r01..r09`, покрывающих все варианты подавления, дедупликацию и FIFO-вытеснение, формирование deep-link для DM и серверов, маскирование токенов MFA/base64/Bearer, а также обработку действий и очистку по закрытию.

## 3. TDD-прогоны
- **Red:** `waves/RW-40/G5/task-174-platform-notifications-router/red.log` (ошибка компиляции из-за отсутствия новых типов и функций).
- **Green:** `waves/RW-40/G5/task-174-platform-notifications-router/green.log` (все 34 теста успешно пройдены: 25 существующих + 9 новых).
- **Clippy:** `cargo clippy --manifest-path rust/Cargo.toml -p discord-platform --test notifications_contract --lib -- -D warnings` — чисто (0 warnings).

## 4. Изменённые файлы и контрольные суммы
- `rust/crates/discord-platform/src/notifications.rs` (`aa6bb85544c51f204d54bfd83db26ea3`)
- `rust/crates/discord-platform/tests/notifications_contract.rs` (`f1d7e0042017de50a3967059d4da3fc8`)

## 5. Риски и совместимость
- Риски отсутствуют: изменения изолированы в write-scope крейта `discord-platform`, 100% обратная совместимость со всеми существующими тестами D-Bus и notifications сохранена. Коммиты в git не выполнялись.
