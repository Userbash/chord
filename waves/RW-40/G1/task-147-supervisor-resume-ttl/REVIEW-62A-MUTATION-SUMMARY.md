# REVIEW-62A — Сводка мутационного тестирования (task-147 / F-GW-08 & OP 3/4/8/37 в `GatewaySupervisor`)

Файл под проверкой: `rust/crates/discord-core/src/gateway/supervisor.rs` (исходный и итоговый md5 `5af33f1994907ad686cbff2ade5df8e3`).

| # | Мутация | Место в `supervisor.rs` | Команда прогона | Статус | Убившие тесты | Лог |
|---|---|---|---|---|---|---|
| M1 | Сужение окна `RESUME` со 180 с до старых 30 с (`RESUME_TTL_MS: 180_000 -> 30_000`) | `:80` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor` | **KILLED** (`EXIT=101`, 1 failed) | `resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s` | `REVIEW-62A-MUTATION-M1.log` |
| M2 | Отключение проверки `elapsed_ms > RESUME_TTL_MS` и сброса `reset_session()` в `plan_next_reconnect` | `:522-528` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor` | **KILLED** (`EXIT=101`, 2 failed) | `resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s`, `outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session` | `REVIEW-62A-MUTATION-M2.log` |
| M3 | Сдвиг порога 4 неудачных попыток `RESUME` из `F-GW-07` (`RESUME_RESET_AFTER_FAILURES: 4 -> 3`) | `:71` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor` | **KILLED** (`EXIT=101`, 2 failed) | `resets_session_after_four_failed_resumes_and_switches_to_fresh_identify`, `successful_resumed_resets_failure_streak_and_backoff` | `REVIEW-62A-MUTATION-M3.log` |
| M4 | Пропуск проверки готовности сессии `ensure_session_ready` перед отправкой `OP 3/4/8/37` | `:642-648` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor` | **KILLED** (`EXIT=101`, 1 failed) | `outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session` | `REVIEW-62A-MUTATION-M4.log` |
| M5 | Отключение чанкования `user_ids` по 100 в `send_request_guild_members_chunked` (`OP 8`) | `:722` | `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib gateway::supervisor` | **KILLED** (`EXIT=101`, 1 failed) | `outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session` | `REVIEW-62A-MUTATION-M5.log` |

Итог: **5/5 мутантов убиты**, исходный файл `supervisor.rs` восстановлен побайтно (`5af33f1994907ad686cbff2ade5df8e3`), тестовые бинари пересобраны из чистого дерева.
