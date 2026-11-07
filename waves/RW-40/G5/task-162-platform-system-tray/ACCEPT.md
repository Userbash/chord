# PM Acceptance — task-162 (`BE-P1-09`: System Tray, Unread Badge & 6-Item Menu in `discord-platform`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/crates/discord-platform/src/lib.rs`
  - `rust/crates/discord-platform/src/tray.rs`
  - `rust/crates/discord-platform/tests/tray_contract.rs`
- **Соответствие ТЗ и оракулу:**
  1. 5 состояний иконок `TrayIconState` (`Normal`, `Unread`, `Speaking`, `Muted`, `Deafened`) с приоритетами из `RE/main-arch.json` §9 (`m-core-1275`).
  2. 6-пунктовое меню `TrayMenu` (`Open`, `Mute`, `Deafen`, `Voice Settings`, `Check for Updates`, `Quit`), бейдж `format_badge_count` (`0..=99`, `'99+'`) для `com.canonical.Unity.LauncherEntry`, `MinimizeToTrayPolicy` и headless-контроллер `TrayController`.
  3. 110/110 тестов `discord-platform` (включая 10 контрактных тестов `tray_contract`) проходят без предупреждений (`clippy 0`).
- **Следующий шаг:** Техническое ревью (`Review 75A`) с проверкой `5/5` мутаций.
