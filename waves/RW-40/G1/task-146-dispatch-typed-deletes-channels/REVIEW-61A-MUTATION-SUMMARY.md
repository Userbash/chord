# Сводка мутационного тестирования — REVIEW-61A (`task-146`, `rust/crates/discord-core/src/dispatch.rs`)

Исходный и восстановленный `md5` файла `rust/crates/discord-core/src/dispatch.rs`:
`e33cc7d7821be717bfa6d3a5006462e2`

Команда прогона каждого мутанта:
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib dispatch
```

| Мутант | Строки `dispatch.rs` | Суть мутации | Код возврата | Результат | Поймавшие тесты | Лог |
|---|---|---|---|---|---|---|
| **M1** | `866` (`json_parse_snowflake`) | Пропуск числового Snowflake через IEEE-754 `f64` (`number.as_u64().map(\|n\| (n as f64) as u64)`), вызывающий потерю младших бит для ID `> 2^53` | `101` | **KILLED** (2 passed, 3 failed) | `message_delete_and_bulk_etf_and_json_preserve_64bit_snowflakes`, `channel_create_update_delete_etf_and_json_typed_parsing`, `thread_create_update_delete_etf_and_json_typed_parsing` | `REVIEW-61A-MUTATION-M1.log` |
| **M2** | `181` (`ExtendedDispatchKind::from_name`) | Удаление сопоставления `"MESSAGE_DELETE_BULK" => Some(Self::MessageDeleteBulk)` (`=> None`) | `101` | **KILLED** (2 passed, 3 failed) | `canonical_21_dispatch_kinds_preserved_and_disjoint_from_8_extended_kinds`, `message_delete_and_bulk_etf_and_json_preserve_64bit_snowflakes`, `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` | `REVIEW-61A-MUTATION-M2.log` |
| **M3** | `572` (`etf_parse_snowflake`) | Снятие фильтра `*number >= 0` и каст отрицательного `i64` в `u64` (`Some(*number as u64)`), пропускающий битые отрицательные ID старого парсера (`-1467272931325252608`) | `101` | **KILLED** (4 passed, 1 failed) | `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` | `REVIEW-61A-MUTATION-M3.log` |
| **M4** | `799` (`parse_thread_lifecycle_etf`) | Игнорирование поля `d.newly_created` в ETF-парсере веток (`let newly_created = None;`) | `101` | **KILLED** (4 passed, 1 failed) | `thread_create_update_delete_etf_and_json_typed_parsing` | `REVIEW-61A-MUTATION-M4.log` |
| **M5** | `1033-1035` (`parse_message_delete_bulk_json`) | Удаление проверки непустоты массива `d.ids` (`if items.is_empty() { return Err(DispatchParseError::EmptyBulkIds { .. }); }`) | `101` | **KILLED** (4 passed, 1 failed) | `rejects_invalid_snowflakes_missing_fields_and_non_dispatch_envelopes` | `REVIEW-61A-MUTATION-M5.log` |

Итог: **5 / 5 мутантов убиты**, исходный файл восстановлен побайтно (`md5 = e33cc7d7821be717bfa6d3a5006462e2`).
