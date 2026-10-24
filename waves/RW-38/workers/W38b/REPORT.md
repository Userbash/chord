# W38b — Rust: lib + IPC-контракт + запуск Tauri (RW-38)

- **batch/карточка**: RW-38 / DoD §2.2 (Rust: `src/lib.rs`, `tests/ipc_contract.rs`, `tests/launch.rs`);
- **сделано**:
  - `src/lib.rs` — чистая логика без Tauri-рантайма: `ProcessMetrics` (`Debug, PartialEq, Serialize`),
    `read_status_field`, `parse_process_metrics` (Result, отсутствие/нечитаемость поля — ошибка),
    `SNAPSHOT_FLOORS` (4/100000/24229/10/38/rendered≥10), `validate_ui_snapshot`,
    `validate_bench_payload` (JSON + объекты chat/members). Тексты ошибок и семантика перенесены
    из `main.rs` без изменений;
  - `src/main.rs` — `use discord_gui_spike::{...}`; `#[tauri::command]`-обёртки `process_metrics`
    (читает `/proc/self/status`), `spike_config`, `report_bench` (`BENCH`), `report_ui_snapshot`
    (`UI_SNAPSHOT`) остались; дублирующий `#[cfg(test)] mod tests` удалён — те же и больше кейсов
    живут в `tests/ipc_contract.rs` (чтобы не было двух копий контракта);
  - `Cargo.toml` — `[lib] name = "discord_gui_spike", path = "src/lib.rs"`; новых зависимостей нет;
  - `tests/ipc_contract.rs` — 20 кейсов: валидный/граничный snapshot, отсутствие каждого из 7 полей,
    типы (`"4"`, `4.5`, `-1`, `null`, `true`, `[]`, `{}`), отрицательные, нули/единицы,
    `rendered<10` (chat и members), битый JSON/массив; bench (chat/members, `null`, `[]`, число,
    строка, отсутствие ключа, битый JSON); `read_status_field` (line-start, нечисловое),
    `parse_process_metrics` (валидный, отсутствие каждого поля, нечисловое, RSS≠HWM, сериализация);
  - `tests/launch.rs` — opt-in `GUI_SPIKE_E2E=1`; без `DISPLAY`/`WAYLAND_DISPLAY` — `eprintln` +
    ранний return (skip-aware). Иначе `Command` на `env!("CARGO_BIN_EXE_discord-gui-spike")` с env
    `SPIKE_EXIT_AFTER_MS=9000`, `SPIKE_AUTOBENCH=0`, `GDK_BACKEND=x11`,
    `WEBKIT_DISABLE_DMABUF_RENDERER=1`, `WEBKIT_DISABLE_COMPOSITING_MODE=1`,
    `LIBGL_ALWAYS_SOFTWARE=1`, `GTK_MODULES=gail:atk-bridge`; stdout/stderr читаются параллельно,
    таймаут 20 с → kill; assert: exit 0, `window-ready`, `UI_SNAPSHOT` с 4/100000/24229/10/38,
    stderr без `Failed to create GBM buffer|Gdk-Message:|panic|segmentation fault`.
- **red**: `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` →
  `error[E0432]: unresolved import discord_gui_spike` (tests/ipc_contract.rs:9), exit 101;
  лог `waves/RW-38/workers/W38b/red.log`;
- **green**: та же команда → `20 passed (ipc_contract)`, `1 passed (launch, skip по env)`,
  exit 0; лог `waves/RW-38/workers/W38b/green.log`;
- **targeted**:
  - `GUI_SPIKE_E2E=1 cargo test ... --test launch -- --nocapture` → 1 passed, 9.11 s (реальный
    запуск окна, exit 0 по `SPIKE_EXIT_AFTER_MS`), `launch-e2e.log`;
  - skip-проверка `env -u DISPLAY -u WAYLAND_DISPLAY GUI_SPIKE_E2E=1 ... --test launch -- --nocapture`
    → `skip: нет DISPLAY/WAYLAND_DISPLAY`, 1 passed, `launch-skip.log`;
  - `cargo clippy --offline --all-targets -- -D warnings` → exit 0 (`clippy.log`);
  - `cargo fmt -- --check` → OK после `cargo fmt` (первый check дал 2 diff, исправлено; `fmt-check.log`
    содержит исходный diff);
- **oracle**: при переносе сверялся исходный `main.rs` (поведение и тексты ошибок 1:1) +
  независимый live-артефакт — реальный запуск бинаря и его `UI_SNAPSHOT`, а не текущая реализация;
  пороги сверены с `waves/RW-38/TZ.md` §4 и `tests/smoke/gui-spike.smoke.test.mjs`;
- **генерация**: не применимо;
- **мутации**: не применимо (карточка воркера; мутанты — у QA-лейна);
- **baseline/restore**: не применимо;
- **стабильность**: launch прогнан дважды (skip и live), таймаут с запасом 20 с при
  `SPIKE_EXIT_AFTER_MS=9000`; гонок нет — тесты не делят состояние, `/proc` только читается;
- **gate:worker**: воркер полный gate не запускает (batch-вердикт за Integration/QA); локально
  прогнаны `cargo test/clippy/fmt` по `rust/gui-spike`;
- **файлы + md5**:
  - `rust/gui-spike/src/lib.rs` — `c1eb0af3e3c25bf5a264339bffc74877`
  - `rust/gui-spike/src/main.rs` — `2763cca93ead65509ece95d861096dab`
  - `rust/gui-spike/Cargo.toml` — `fc2515bc5a3e01159300c6382ccaae25`
  - `rust/gui-spike/tests/ipc_contract.rs` — `6e2fe83e366aa7ec3bfafd114f381085`
  - `rust/gui-spike/tests/launch.rs` — `402fb1178f2942d54b1a905bb2f89966`
  - `ui/*`, `ui/model.js`, бенч/смоук, `waves/*` (кроме этого каталога логов) не менялись;
- **риски/вопросы**:
  - live-тест наследует env родителя; при заданном, но нерабочем `DISPLAY` он упадёт — это
    осознанно (opt-in `GUI_SPIKE_E2E=1`), headless-прогон без дисплея даёт skip;
  - `#[cfg(test)] mod tests` из `main.rs` удалён: покрытие перенесено и расширено в
    `tests/ipc_contract.rs`, но если PM ждёт «unit-тесты внутри бинаря» — вернуть модуль нельзя без
    дублирования контракта (решение на приёмке);
  - `cargo test` собирает бинарь с новым `ui/*` только при пересборке: live smoke PM отдельно
    проверяет свежесть бинаря (вне моих файлов);
- **следующий шаг**: приёмка Team Lead (файловая матрица + red/green + live-лог), затем batch-gate.
