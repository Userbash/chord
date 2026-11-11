# Отчёт воркера: TASK-904 — Автоматизация визуальной регрессии GPUI на базе SSIM (>0.99) и маскирования

## 1. Batch / Карточка
- **Карточка:** `TASK-904` (Отдел `G4 QA`, приоритет `P1`, Orca Task ID `task_c08f9e27580f`, Dispatch `ctx_7bca3c80c9cb`).
- **Назначение:** Автоматизация визуальной регрессии GPUI на базе SSIM (`>0.99`, порог по умолчанию `0.995`): детерминированное снятие RGBA-пикселей из виртуального `FrameBuffer` GPUI/WGPU (`llvmpipe`, `FrozenClock`, отключённое субпиксельное сглаживание шрифтов), расчёт перцептивного индекса структурного сходства SSIM с JSON-масками динамических зон (`clock`, `fps_counter`, `avatar`, `ping_indicator`), генерация интерактивного HTML-отчёта (`Slider` / `Onion Skin` / `Red Diff Overlay`) и эталоны `Golden Snapshots` (15 снимков: главный чат `1280x720` и `1920x1080`, 8 вкладок `SettingsDialog`, 3 модальных окна, контекстное меню ПКМ и тултип профиля).

## 2. Что сделано
1. **SUB-904-1 — Детерминированный генератор скриншотов из FrameBuffer (`framebuffer.rs`, `mod.rs`):**
   - Реализована структура `GpuiFrameBuffer` (несжатый буфер `RGBA8`, 4 байта на пиксель, экспорт/импорт `PNG` через `image 0.25`).
   - Реализованы `LlvmPipeCaptureConfig` (`LIBGL_ALWAYS_SOFTWARE=1`, `WGPU_BACKEND=gl`, `MESA_LOADER_DRIVER_OVERRIDE=llvmpipe`, `GPUI_SUBPIXEL_AA=0`), `SubpixelAntialiasingMode::Disabled` (монохромная битовая растеризация глифов 5x7 без субпиксельных ореолов) и `FrozenClock` (`2026-11-11T12:00:00Z` / `12:00`, epoch `1794398400`).
2. **SUB-904-2 — Алгоритм SSIM с JSON-масками исключений и HTML Visual Diff (`ssim.rs`, `masks.rs`, `report.rs`):**
   - Реализован `SsimComparator` по формуле Wang et al. (2004) по блокам `8x8` в канале яркости BT.601 ($Y = 0.299R + 0.587G + 0.114B$, $C_1 = (0.01 \times 255)^2$, $C_2 = (0.03 \times 255)^2$) с порогом допуска `0.995` (и минимальным контрактом `> 0.99`).
   - Реализован `DynamicMaskSet` с JSON-сериализацией (`dynamic_masks_1280x720.json`, `dynamic_masks_1920x1080.json`) для 4 обязательных категорий динамических зон (`Clock`, `FpsCounter`, `Avatar`, `PingIndicator`). Маскированные пиксели исключаются из расчёта SSIM, обеспечивая `0` ложных срабатываний при изменении часов, FPS, аватарок или пинга.
   - При расхождении незамаскированных пикселей формируется карта диффа с подсветкой красными пикселями (`#ff2244`) и интерактивный HTML-отчёт `visual_diff_report.html` с режимами `Slider`, `Onion Skin` и `Red Diff Overlay`.
3. **SUB-904-3 — Набор из 15 золотых эталонов (`tests/goldens/*.png` + `golden_manifest.json`):**
   - Главный экран чата Discord Dark (`#1e1f22` / `#2b2d31` / `#313338`): `main_chat_1280x720.png`, `main_chat_1920x1080.png`.
   - Все 8 вкладок `SettingsDialog`: `settings_01_my_account.png`, `settings_02_profile.png`, `settings_03_voice_and_video.png`, `settings_04_appearance.png`, `settings_05_keybinds.png`, `settings_06_notifications.png`, `settings_07_text_and_images.png`, `settings_08_language.png`.
   - Модальные окна и оверлеи: `modal_captcha.png`, `modal_create_guild.png`, `modal_screen_share.png`, `overlay_context_menu.png`, `overlay_profile_tooltip.png`.

## 3. Red-прогон (Strict TDD)
- Команда: `cargo test --manifest-path rust/discord-gpui/Cargo.toml --test visual_regression_contract -- --nocapture`
- Результат RED: `test result: FAILED. 45 passed; 8 failed; 0 ignored; finished in 0.08s` (все 8 целевых контрактов `test_sub_904_*` упали на заглушках до реализации).

## 4. Green-прогон и Targeted Gate (T0)
- Форматирование: `rustfmt --edition 2024 rust/discord-gpui/src/visual_regression/*.rs rust/discord-gpui/tests/visual_regression_contract.rs` — чисто (`0` предупреждений в `visual_regression_contract`).
- Команда: `cargo test --manifest-path rust/discord-gpui/Cargo.toml --test visual_regression_contract test_sub_904 -- --nocapture`
- Результат GREEN: `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 45 filtered out; finished in 1.86s`.
- Мутационный гейт (`test_sub_904_mutation_gate_5_of_5_killed`): `5/5 (100%)` мутаций убито (отказ масок, порча темы `ServerRail`, коллизия вкладок `SettingsDialog`, исчезновение `CaptchaModal`, рассинхрон `FrozenClock`).

## 5. Файлы и контрольные суммы MD5
| Файл | Строк | MD5 |
|---|---:|---|
| `rust/discord-gpui/src/visual_regression/mod.rs` | 211 | `46255b166f3cabc7d1fad23214c8d8e0` |
| `rust/discord-gpui/src/visual_regression/framebuffer.rs` | 557 | `d9a77d455990597760c2f4d02f2f0186` |
| `rust/discord-gpui/src/visual_regression/masks.rs` | 209 | `070e9fa9285655c7cb304e3aa7e2e787` |
| `rust/discord-gpui/src/visual_regression/ssim.rs` | 200 | `78d7075d7c7ee241f9883b9c4bfe35b3` |
| `rust/discord-gpui/src/visual_regression/report.rs` | 171 | `a2d53991d34028fa8fdcd4d1927a9801` |
| `rust/discord-gpui/tests/visual_regression_contract.rs` | 440 | `ff8166b9ae7ac67298c2bc09c96f7ece` |
| `rust/discord-gpui/tests/goldens/golden_manifest.json` | — | `31832cd8f1a3246d03bad240b9cc2f95` |
| `rust/discord-gpui/tests/goldens/dynamic_masks_1280x720.json` | — | `8f8526e7ae7a246f3e1ace3fad39913a` |
| `rust/discord-gpui/tests/goldens/dynamic_masks_1920x1080.json` | — | `9ff7d0f425bf048d9583d61837deeb00` |
| `rust/discord-gpui/tests/goldens/visual_diff_report.html` | — | `dc837a8efc2dc0ab2344938b96b62e0f` |

## 6. Риски и ограничения
- Все файлы изолированы в `rust/discord-gpui/src/visual_regression/`, `rust/discord-gpui/tests/visual_regression_contract.rs` и `rust/discord-gpui/tests/goldens/`; пересечений с соседними воркерами (`TASK-901`, `TASK-902`, `TASK-903`) нет.
