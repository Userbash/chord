# Приёмка PM: task-174 (BE-P2-14) — NotificationRouter: фильтрация DND/активного канала/упоминаний/ЛС, дедупликация по message_id, маскирование токенов и переход по клику

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-174-platform-notifications-router`
- **Пулл-реквест:** `PR-59` (`feature/task-174-platform-notifications-router -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **`IncomingMessageNotification`, `NotificationDispatchDecision` и `NotificationRouter` в `notifications.rs`**:
   - Фильтрация своих сообщений (`SuppressedSelfAuthor`), DND (`SuppressedDnd`), активного канала в сфокусированном окне (`SuppressedActiveChannel`), неупоминаний вне ЛС (`SuppressedNotMentionOrDm`), дубликатов по `message_id` в FIFO-очереди на 256 записей (`SuppressedDuplicate`).
   - Маскирование токенов (`mfa.*`, 3-частных base64-токенов, `Bearer`) маркером `<redacted>`.
   - Генерация действия `("default", "Open")` и маршрута `discord://-/channels/@me/{channel_id}` (ЛС) / `discord://-/channels/{guild_id}/{channel_id}` (сервер).
   - Обработка `ActionInvoked` -> возврат `deep_link_url` и очистка по `NotificationClosed`.
2. **Контрактные тесты в `notifications_contract.rs`**:
   - Все 34 теста (25 существующих + 9 новых `r01..r09`) проходят (`34/34 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 88A)** (`5/5` мутаций).
