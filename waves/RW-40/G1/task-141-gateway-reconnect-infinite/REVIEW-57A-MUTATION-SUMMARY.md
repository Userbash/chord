# REVIEW-57A — Сводка мутационного тестирования (task-141 / BE-RE-06 / F-GW-07)

Файл под проверкой: `rust/crates/discord-core/src/gateway/supervisor.rs` (исходный и итоговый md5 `ffa734a6b168880f14fe5cc7e601f940`).

| # | Мутация | Место в `supervisor.rs` | Команда прогона | Статус | Убившие тесты | Лог |
|---|---|---|---|---|---|---|
| M1 | Изменение порога 4 попыток `RESUME` (`RESUME_RESET_AFTER_FAILURES: 4 -> 3`) | `:42` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests` | **KILLED** (`EXIT=101`, 2 failed) | `resets_session_after_four_failed_resumes_and_switches_to_fresh_identify`, `successful_resumed_resets_failure_streak_and_backoff` | `REVIEW-57A-MUTATION-M1.log` |
| M2 | Отключение сброса `session_id`/`seq` (закомментирован `self.state = None` в `note_attempt`) | `:403` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests` | **KILLED** (`EXIT=101`, 2 failed) | `resets_session_after_four_failed_resumes_and_switches_to_fresh_identify`, `successful_resumed_resets_failure_streak_and_backoff` | `REVIEW-57A-MUTATION-M2.log` |
| M3 | Изменение потолка backoff 60s (`Backoff::default()` `60_000 -> 30_000`) | `:87` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests` | **KILLED** (`EXIT=101`, 1 failed) | `unlimited_reconnect_mode_holds_60s_ceiling_and_stops_immediately_on_4004` | `REVIEW-57A-MUTATION-M3.log` |
| M4 | Пропуск фатального стопа на `4004` (`Some(4004) => CloseAction::Reconnect` в `classify_close`) | `:244` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor::tests` | **KILLED** (`EXIT=101`, 3 failed) | все 3 контрактных теста `gateway::supervisor::tests::*` | `REVIEW-57A-MUTATION-M4.log` |
| M5 | Нарушение совместимости `max_reconnects` (возврат дефолта `max_reconnects: 8` в `SupervisorConfig::new` + отключение проверки `max_reconnects != 0` в `run_until`) | `:151`, `:362` | `--lib gateway::supervisor::tests` + `--test gateway_supervisor supervisor_reconnect_limit_stops` | **KILLED** (`EXIT_LIB=101`, `EXIT_IT=101`) | `unlimited_reconnect_mode_holds_60s_ceiling_and_stops_immediately_on_4004` (`--lib`) + `supervisor_reconnect_limit_stops` (`--test gateway_supervisor`) | `REVIEW-57A-MUTATION-M5.log` |

Итог: **5/5 мутантов убиты**, исходный файл `supervisor.rs` восстановлен побайтно (`ffa734a6b168880f14fe5cc7e601f940`), тестовые бинари пересобраны из чистого дерева.
