# W27b — мутации (deep links 8, autostart 6)

Цель: каждая мутация должна быть поймана targeted-тестами; продукт восстановлен
побайтно (md5 совпадает с базой). Команда каждого прогона:

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w27b cargo test --offline \
  -p discord-platform --test deep_link_contract --test autostart_contract
```

База md5: `deep_link.rs` = `56bc6ea26b34e7200d1de99469b69e73`, `autostart.rs` = `954222c0a6397ca4c8b2a9c7e169b61e`.

| # | Файл | Мутация | Exit | Вердикт | Поймавшие тесты |
|---|---|---|---|---|---|
| D1 | `deep_link.rs` | схема сравнивается регистрозависимо | 101 | caught | `d09_scheme_and_host_case_insensitive_path_case_sensitive` |
| D2 | `deep_link.rs` | host сравнивается регистрозависимо | 101 | caught | `d09_scheme_and_host_case_insensitive_path_case_sensitive` |
| D3 | `deep_link.rs` | граница длины: >= вместо > | 101 | caught | `d06_rejects_empty_scheme_length_and_controls` |
| D4 | `deep_link.rs` | control-символы в raw пропущены | 101 | caught | `d06_rejects_empty_scheme_length_and_controls` |
| D5 | `deep_link.rs` | extract_deep_link сканирует argv с индекса 0 | 101 | caught | `d07_extract_deep_link_matches_re` |
| D6 | `deep_link.rs` | extract_deep_link игнорирует `--` раньше `--url` | 101 | caught | `d07_extract_deep_link_matches_re` |
| D7 | `deep_link.rs` | Debug печатает реальный токен | 101 | caught | `d03_login_one_time_token_is_redacted_in_debug` |
| D8 | `deep_link.rs` | Channels(path) не канонизируется для host=channels | 101 | caught | `d05_channels_routes` |
| A1 | `autostart.rs` | hidden=true не выключает автозапуск | 101 | caught | `a02_hidden_disables_autostart_via_gnome_key` |
| A2 | `autostart.rs` | кавычка в Exec не экранируется | 101 | caught | `a03_exec_escapes_spaces_quotes_and_field_codes` |
| A3 | `autostart.rs` | `%` не удваивается (field code) | 101 | caught | `a03_exec_escapes_spaces_quotes_and_field_codes` |
| A4 | `autostart.rs` | пустой аргумент не превращается в "" | 101 | caught | `a04_exec_escapes_shell_reserved_and_empty_args` |
| A5 | `autostart.rs` | control-символ в Name пропущен | 101 | caught | `a05_rejects_empty_and_control_fields` |
| A6 | `autostart.rs` | autostart_path без подкаталога autostart/ | 101 | caught | `a06_autostart_path_is_xdg_autostart_file` |

Итог: **14 caught, 0 missed, 0 inconclusive**; все файлы восстановлены по md5.

## Повторный прогон TL-2 (2026-10-22, финальный код)

После интеграционных фиксов TL-2 (кавычки для XDG reserved-символов в `Exec`,
redaction `query`/`fragment` в `Debug` для `DeepLink`) мутации прогнаны заново:
**14 caught, 0 missed, 0 inconclusive**, база md5 обновлена
(`deep_link.rs` `56bc6ea26b34e7200d1de99469b69e73`, `autostart.rs`
`954222c0a6397ca4c8b2a9c7e169b61e`). Лог: `waves/RW-27/mutations/w27b-final.log`.
