# Отчёт воркера: task-162 (BE-P1-09: Системный трей Linux, иконки состояний, бейдж непрочитанных и 6-пунктовое меню приложения в discord-platform)

- **batch/карточка:** `RW-40 / BE-P1-09` (task-162)
- **сделано:**
  1. Реализован модуль `rust/crates/discord-platform/src/tray.rs` и экспортирован в `rust/crates/discord-platform/src/lib.rs`.
  2. Определены 5 эталонных состояний иконки трея `TrayIconState` (`Normal`, `Unread`, `Speaking` / `InCall`, `Muted`, `Deafened`) с сопоставлением системных имен `discord-tray*` и ассетов Discord Desktop `tray*` из `RE/main-arch.json` §9.
  3. Реализована чистая функция вычисления состояния `compute_tray_icon_state(in_call, self_mute, self_deaf, unread_count)` со строгой иерархией приоритетов Discord Desktop (`m-core-1275`).
  4. Спроектировано 6-пунктовое контекстное меню `TrayMenu` / `TrayMenuItem` в требуемом порядке:
     - 1. Open («Открыть Discord»)
     - 2. Mute (чекбокс «Откл. микрофон»)
     - 3. Deafen (чекбокс «Откл. звук»)
     - 4. Voice Settings («Настройки голоса и видео»)
     - 5. Check for Updates («Проверить обновления»)
     - 6. Quit («Выйти из Discord»)
  5. Реализована модель бейджа `BadgeState` и `format_badge_count(count: u32)` (0 -> `None` / скрыт, 1..=99 -> число, >99 -> `'99+'`), генерация D-Bus payload для `com.canonical.Unity.LauncherEntry.Update` со свойствами `count` (`Value::I32`) и `count-visible` (`Value::Bool`).
  6. Реализована политика закрытия окна в трей `MinimizeToTrayPolicy` (`should_hide_on_close(minimize_to_tray_enabled, is_quitting) -> CloseDisposition { HideToTray | QuitProcess }`).
  7. Разработан контроллер `TrayController` с абстракцией `TrayBackend` и тестовой реализацией `MockTrayBackend` для работы в headless-режиме без паданий при отсутствии системного трея (DoD 3).
  8. Написан набор контрактных тестов в `rust/crates/discord-platform/tests/tray_contract.rs` (10 тестов, 100% pass).
- **red:** `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-platform --test tray_contract` → `error[E0432]: unresolved import discord_platform::tray`, exit code 101.
- **green:** `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-platform --test tray_contract` → 10 passed; 0 failed; finished in 0.00s, exit code 0.
- **targeted:**
  - `rust/crates/discord-platform/tests/tray_contract.rs` — 10 тестов passed (0 failed).
  - Полный сьют `discord-platform`: `cargo test -p discord-platform` — 110 тестов passed, 0 failed.
  - `cargo clippy -p discord-platform --all-targets -- -D warnings` — чисто (0 warnings).
  - `cargo fmt -p discord-platform -- --check` — чисто.
- **oracle:**
  - `RE/main-arch.json` §9: `TRAY_ICON_NAMES` (`DEFAULT: "tray"`, `UNREAD: "tray-unread"`, `SPEAKING: "tray-speaking"`, `MUTED: "tray-muted"`, `DEAFENED: "tray-deafened"`), GUID по каналу, структура меню и кликов (`atomTray.on("click", options.onTrayClicked)`).
  - `RE/backend-ipc.json`: `DISCORD_APP_SET_BADGE_COUNT`, `DISCORD_APP_DOCK_*`.
  - Модуль `#1275` (`/home/sanya/re-deep/main-arch-work/m-core-1275.txt`): порядок приоритетов состояний и структура `setContextMenu`.
  - Спецификация D-Bus Canonical Unity: `com.canonical.Unity.LauncherEntry` сигнал `Update` с `sa{sv}` (`count`, `count-visible`).
- **генерация:** не применимо.
- **мутации:**
  1. Мутация 1: изменение приоритета `self_deaf` над `self_mute` -> убивается тестом `test_compute_tray_icon_state_priority`.
  2. Мутация 2: инверсия `count > 99` в `format_badge_count` -> убивается тестом `test_format_badge_count_oracle`.
  3. Мутация 3: нарушение порядка или количества пунктов в `TrayMenu` -> убивается тестом `test_tray_menu_structure_and_order`.
  4. Мутация 4: инверсия `is_quitting` в `MinimizeToTrayPolicy` -> убивается тестом `test_minimize_to_tray_policy`.
  5. Мутация 5: паника при отсутствии бэкенда (`None`) -> убивается тестом `test_tray_controller_backend_unavailable_resilience`.
  - Итог: 5/5 мутаций убиваются тестами контракта.
- **baseline/restore:** чисто, посторонние файлы не затронуты.
- **стабильность:** повторный прогон `cargo test -p discord-platform` 5 раз подряд — детерминированный green, 0 flaky.
- **gate:worker:** все целевые проверки и линтеры зеленые.
- **файлы:**
  - `rust/crates/discord-platform/src/lib.rs` (1d0487d3d8de1676cddd51a70db6dd88)
  - `rust/crates/discord-platform/src/tray.rs` (5cb1b4a4af3f5d2ae22d74325b6ae9df)
  - `rust/crates/discord-platform/tests/tray_contract.rs` (65f3d5adf4cf525b0e34ec621eb6bcab)
  - `/home/sanya/orca/project/waves/RW-40/G5/task-162-platform-system-tray/REPORT.md`
- **риски:** нет. Код полностью изолирован в `discord-platform`, абстрагирован через трейт `TrayBackend` и не требует реального оконного сервера для работы и тестов.
