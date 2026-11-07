# Мутационный прогон task-140 (Review 54A)

Файлы и md5 до/после: `rust/crates/discord-core/src/identify.rs`
`95cae2fdb08951759ae280e35554ce81` / `95cae2fdb08951759ae280e35554ce81`;
`rust/crates/discord-core/src/rest/identity.rs`
`5f99d3e0c75b1745fa89964f00818c42` / `5f99d3e0c75b1745fa89964f00818c42`.

Каждая мутация применялась к одному файлу, затем прогонялся
`cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib`
(логи `REVIEW-54A-MUTATION-M*.log`), после чего файл восстанавливался из копии.

| # | Мутация | Результат | Поймавшие тесты |
|---|---|---|---|
| M1 | `launch_signature := client_launch_id` в `from_super_properties` | killed, 4 passed / 4 failed, exit 101 | `client_identity_drives_identify_properties`, `identify_wire_matches_super_properties_byte_for_byte`, `rw_client_identity_matches_identify_defaults`, `client_identity_builds_rest_and_identify_from_one_set_of_uuids` |
| M2 | убрать `launch_signature` из `properties` OP2 | killed, 7 / 1, exit 101 | `identify_wire_matches_super_properties_byte_for_byte` |
| M3 | `to_identify_properties` игнорирует идентичность и зовёт `for_rw_client()` | killed, 5 / 3, exit 101 | `client_identity_drives_identify_properties`, `identify_wire_matches_super_properties_byte_for_byte`, `client_identity_builds_rest_and_identify_from_one_set_of_uuids` |
| M4 | `rw_client()` без кэша `OnceLock` | killed, 7 / 1, exit 101 | `rw_client_identity_matches_identify_defaults` |
| M5 | `os_arch := app_arch` в `from_super_properties` | killed, 6 / 2, exit 101 | `client_identity_drives_identify_properties`, `identify_wire_matches_super_properties_byte_for_byte` |
| M6 (доп.) | `browser` захардкожен в `"Discord Client"` | killed, 7 / 1, exit 101 | `client_identity_builds_rest_and_identify_from_one_set_of_uuids` |

Итог: требуемые **5/5** (M1–M5) убиты; дополнительный M6 тоже убит. После
всех прогонов md5 обоих файлов совпадают с исходными.
