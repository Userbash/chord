# Техническое ревью (Review 75A) — task-162: Системный трей Linux, иконки состояний, бейдж непрочитанных и 6-пунктовое меню в discord-platform

- **Карточка/Компонент:** `task-162` (`BE-P1-09`) / `discord-platform`
- **Ворктри:** `/home/user/orca/workspaces/project/feature-task-162-platform-system-tray`
- **Проверенные файлы:**
  - `rust/crates/discord-platform/src/lib.rs` (md5: `1d0487d3d8de1676cddd51a70db6dd88`)
  - `rust/crates/discord-platform/src/tray.rs` (md5: `5cb1b4a4af3f5d2ae22d74325b6ae9df`)
  - `rust/crates/discord-platform/tests/tray_contract.rs` (md5: `65f3d5adf4cf525b0e34ec621eb6bcab`)
- **Статус ревью:** `PASS`
- **Вердикт:** `Ready for Merge`

---

## 1. Соответствие архитектуре и оракулам

1. **Имена и состояния иконок:**
   - Строго соответствуют оракулу `RE/main-arch.json` §9 (`TRAY_ICON_NAMES`): `Normal` -> `"tray"` / `"discord-tray"`, `Unread` -> `"tray-unread"` / `"discord-tray-unread"`, `Speaking` (`IN_CALL`) -> `"tray-speaking"` / `"discord-tray-speaking"`, `Muted` -> `"tray-muted"` / `"discord-tray-muted"`, `Deafened` -> `"tray-deafened"` / `"discord-tray-deafened"`.
   - Иерархия приоритетов в `compute_tray_icon_state` соответствует модулю `#1275` (`m-core-1275`): `Deafened` > `Muted` > `Speaking` при звонке; вне звонка `Unread` при `unread_count > 0`, иначе `Normal`.
2. **Структура контекстного меню:**
   - 6 пунктов в точном порядке и типах: `Open` (Normal), `Mute` (Checkbox), `Deafen` (Checkbox), `Voice Settings` (Normal), `Check for Updates` (Normal), `Quit` (Normal).
   - Обработка `Quit` переводит приложение в `CloseDisposition::QuitProcess`.
3. **Бейдж непрочитанных:**
   - `format_badge_count` реализует контракт `0 -> None`, `1..=99 -> String(n)`, `>99 -> "99+"`.
   - Формирование сигнала D-Bus `com.canonical.Unity.LauncherEntry.Update` корректно типизировано (`i32` и `bool` variants).
4. **Отказоустойчивость (DoD 3):**
   - Контроллер `TrayController` корректно функционирует в headless-режиме (`backend: None`) без паник и возвращает строгие типизированные ошибки `TrayError` при сбоях бэкенда.

---

## 2. Результаты тестов и проверок качества

- **Targeted & Suite Tests:**
  `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-platform`
  - `tray_contract.rs`: 10 passed; 0 failed (100% pass)
  - Полный сьют `discord-platform`: 110 passed; 0 failed; 8 ignored (интеграционные keyring, требующие live freedesktop daemon).
- **Clippy:**
  `cargo clippy --manifest-path rust/Cargo.toml -p discord-platform --all-targets -- -D warnings`
  - Чисто (0 warnings / 0 errors).
- **Rustfmt:**
  `cargo fmt --manifest-path rust/Cargo.toml -p discord-platform -- --check`
  - Чисто (0 formatting diffs).

---

## 3. Верификация 5/5 мутаций (Mutation Testing)

Все 5 ручных мутаций были применены в `rust/crates/discord-platform/src/tray.rs` и проверены на падение тестов `tray_contract`:

| # | Описание мутации | Изменение в коде | Убивающий тест | Статус |
|---|------------------|-------------------|----------------|--------|
| 1 | Инверсия приоритета mute над deafen | `if self_mute { Muted } else if self_deaf { Deafened }` | `test_compute_tray_icon_state_priority`, `test_tray_controller_voice_and_unread_events` | **KILLED** |
| 2 | Инверсия порога бейджа >99 | `1..=99 => Some("99+"), _ => Some(count)` | `test_format_badge_count_oracle` | **KILLED** |
| 3 | Нарушение порядка пунктов контекстного меню | Перестановка пунктов меню (`Mute` перед `Open`) | `test_tray_menu_structure_and_order` | **KILLED** |
| 4 | Инверсия флага `is_quitting` в политике закрытия | `if !is_quitting { QuitProcess }` | `test_minimize_to_tray_policy`, `test_tray_controller_lifecycle_and_actions` | **KILLED** |
| 5 | Замена безопасного `as_mut()` на `.expect("panic")` при headless/отсутствии бэкенда | `self.backend.as_mut().expect(...)` в `set_unread_count` | `test_tray_controller_backend_unavailable_resilience` | **KILLED** |

**Результат мутационного анализа:** 5/5 мутантов убиты (100% kill rate).
Все файлы полностью восстановлены и их контрольные суммы md5 проверены (`md5sum` совпадает с исходным состоянием до байта).

---

## 4. Итоговое заключение

Код карточки `task-162` полностью соответствует DoD, согласован с эталонным дампом Discord, обладает стопроцентным покрытием мутаций и готов к слиянию в ветку `unstable`.

- **Решение:** `PASS`
- **Статус карточки:** `Ready for Merge`
