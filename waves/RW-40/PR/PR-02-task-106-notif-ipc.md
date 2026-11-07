# PR #02 — `feature/task-106-notif-ipc` (`rw40/review-int06-notif-ipc`) -> `unstable`

- **Задача:** `task-106` — Подключение Tauri IPC-команд уведомлений `update_guild_notification_settings` и `update_channel_notification_override` (`F-5`)
- **Роль исполнителя:** Разработчик Backend (`G1 Rust Backend`, `ctx_37626c93d0e9`)
- **Целевая ветка:** `unstable` (`/home/sanya/orca/workspaces/project/unstable`)
- **Локальные проверки и CI воркера:**
  - `RED`: `cargo test -p discord-gui-spike --test ipc_wiring_contract` -> `EXIT_CODE=101` ([red.log](file:///home/sanya/orca/project/waves/RW-40/PM/int06-notif-ipc/red.log))
  - `GREEN`: `ipc_wiring_contract` `8/8`, `lib` `8/8`, `main` `12/12`, `notifications_contract` `4/4`, `clippy` `0 warnings`, `rustfmt` `exit 0`, `ipc-path.test.mjs` `12/12`, `test-policy.test.mjs` `17/17` ([green.log](file:///home/sanya/orca/project/waves/RW-40/PM/int06-notif-ipc/green.log))
  - `MUTATIONS`: `5/5 KILLED`, baseline восстановлен ([mutations.log](file:///home/sanya/orca/project/waves/RW-40/PM/int06-notif-ipc/mutations.log))
- **Проверка Продуктового Менеджера (Product Review):** **APPROVED** (`Ready for Technical Review`)
- **Проверка Технического Ревьюера (`deepseek/deepseek-v4.1-flash`):** **APPROVED (`PASS, P0=0, P1=0, P2=3`)** ([REVIEW.md](file:///home/sanya/orca/project/waves/RW-40/PM/int06-notif-ipc/REVIEW.md))
- **Статус задачи:** `Merged to Unstable` (рабочая ветка удалена после слияния в `unstable`; замечания P2-1..P2-3 вынесены в отдельную задачу `bugfix/task-109-ipc-oracle-and-override`).
