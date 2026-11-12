# ACCEPT — INT-06-NOTIFICATIONS-IPC-WIRING (`rust/gui-spike/src/main.rs` & `rust/gui-spike/tests/ipc_wiring_contract.rs`)

- **Задача:** `task-106` (`INT-06-NOTIFICATIONS-IPC-WIRING`, закрытие `F-5` из `MERGE-READINESS-13.md`)
- **Статус задачи:** `Merged to Unstable`
- **Команда:** G1 Rust Backend (`ctx_37626c93d0e9`)
- **Product Review (PM):** **APPROVED** — оба IPC-обработчика уведомлений (`update_guild_notification_settings`, `update_channel_notification_override`) подключены в `main.rs` и соответствуют контракту UI `notifications.js` и спецификации `NOTIFICATIONS-SPEC.md`.
- **Technical Review (DeepSeek):** [REVIEW.md](file:///home/user/orca/project/waves/RW-40/PM/int06-notif-ipc/REVIEW.md) (`ctx_fe712b47c7d7`, `deepseek/deepseek-v4.1-flash`) — **PASS (P0=0, P1=0, P2=3)**.
- **Pull Request:** [PR-02-task-106-notif-ipc.md](file:///home/user/orca/project/waves/RW-40/PR/PR-02-task-106-notif-ipc.md) -> слит в `unstable`, рабочая ветка удалена.
- **Отдельная задача по P2:** создана `bugfix/task-109-ipc-oracle-and-override` в `Backlog -> Ready for Development` для закрытия P2-1..P2-3.
