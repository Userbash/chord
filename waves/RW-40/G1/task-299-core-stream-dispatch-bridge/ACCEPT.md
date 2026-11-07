# ACCEPT — PR-178 (`task-299` / `CORE-FIX-01`: Мост входящих событий `STREAM_*` в `rust/crates/discord-core/src/dispatch.rs`)

- **Статус PM-приёмки:** `ACCEPTED` (`Ready for Merge`)
- **Ветка:** `bugfix/task-299-core-stream-dispatch-bridge`
- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Модель исполнения:** `Antigravity Flash Low` (`gemini-3.8-flash-low`, `--effort low`, `L1 / P0`)

## Сверка с DoD
1. В `rust/crates/discord-core/src/dispatch.rs` реализованы `StreamDispatchKind` (`ALL`, `from_name`, `as_str`), `RoutedStreamDispatch` (`into_action`), `DispatchKind::STREAM_NAMES`, `DispatchKind::is_stream_name`, методы `DispatchEvent::{stream_kind, is_stream, is_known, parse_stream, parse_stream_with_diagnostics, to_stream_action}`, `route_stream_dispatch` и `route_stream_dispatch_with_diagnostics`.
2. Все 4 интеграционных теста `cargo test -p discord-core --test dispatch_types` (`4/4 PASS` — 21 вектор `inbound`, 5 негативных векторов, 14 сценариев `reducer` из `stream-opcodes-fixtures.json`) и `33/33` lib-теста `discord-core` зелёные.
