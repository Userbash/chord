# Отчёт воркера: TASK-625 (`deep_link` — безопасный обработчик `discord://` / `disclip://`)

## 1. Batch / Карточка
- **Карточка:** `TASK-625` (`task_257cf3bbdf04`, dispatch `ctx_12679580467a`)
- **Отдел:** `G5 Voice / Platform` (`discord-platform`)
- **Ветка:** `feature/task-625-deep-link-handler`
- **Файлы задачи:**
  - `rust/crates/discord-platform/src/deep_link.rs`
  - `rust/crates/discord-platform/tests/deep_link_contract.rs`

## 2. Сделано (сверенный с RE-оракулами `reference-oracle/` контракт)
1. **Расширенные RE-маршруты `discord://` (`reference-oracle/decoded/4d2a64cb5df5fc4e_0` модуль `901123` `BVt`, `reference-oracle/main-arch-work/m-core-5203.txt`):**
   - Добавлены варианты [`Route`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L102-L137) и канонические маппинги [`main_window_path`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L794-L864):
     - `Route::Invite { code }` (`discord://[-]/invite(s)/<code>` -> `/invite/<code>`, граница длины `MAX_INVITE_CODE_LEN = 64`)
     - `Route::GiftCode { code }` (`discord://[-]/gift(s)/<code>`, `discord://app/gifts/<code>` -> `/gifts/<code>`)
     - `Route::UserProfile { user_id }` (`discord://[-]/users/<id|@me>` -> `/users/<id>`, валидация границ Snowflake `1..=u64::MAX`)
     - `Route::Quest { quest_id }` (`discord://[-]/quests/<id>` -> `/quests/<id>`)
     - `Route::LoginHandoff` (`discord://login/handoff`, `discord://[-]/login/handoff` -> `/login/handoff`)
     - `Route::ConnectAuthorize` (`discord://connect/authorize`, `discord://[-]/connect/authorize` -> `/connect/authorize`)
2. **Структурированный разбор целей `/channels/...` с проверкой границ Snowflake (`1..=u64::MAX`):**
   - Реализованы [`ChannelTarget`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L868-L888) (`DirectMessagesHome`, `DirectMessage`, `GuildChannel`, `GuildThread`) и [`parse_channel_target`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L904-L953), отвергающие `0`, переполнение `> u64::MAX`, ведущий `+` и лишние сегменты.
3. **Разбор и защита параметров `/oauth2/authorize` (`reference-oracle/decoded/4d2a64cb5df5fc4e_0` `Kk["/oauth2/authorize"]`):**
   - Реализованы [`OAuth2AuthorizeParams`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L960-L969), [`OAuth2AuthorizeError`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L987-L995) и [`parse_oauth2_authorize_params`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1110-L1185) с маскированием `state` и `nonce` (`<redacted>`) в `Debug` и защитой `redirect_uri` от опасных схем (`javascript:`, `data:`, `file:`, внешний `http://`).
4. **Защита от Path Traversal и инъекций в сегменты путей (`is_safe_segment`):**
   - Все маршруты `channels`, `guilds`, `settings`, `library`, `clips`, `disclip`, `invite`, `gifts`, `users`, `quests` очищаются от `.` / `..`, `\`, а также percent-encoded вариантов (`%2e%2e`, `%2f`, `%5c`, `%00` и управляющих байтов).
5. **Безопасность внешних ссылок и сравнение Origin (`reference-oracle/asar-extract/core/bundle.js` модуль `#1331` `securityUtils`):**
   - Добавлены [`BLOCKED_URL_PROTOCOLS`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L49-L64) (14 опасных протоколов из RE-оракула), [`should_open_external_url`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1192-L1239) (блокирует однобуквенные схемы дисков Windows `c:/...`, `BLOCKED_URL_PROTOCOLS`, `user:pass@host`) и [`check_url_origin_matches`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1281-L1289).
6. **Регистрация системных протоколов (`XDG .desktop`, `mimeapps.list`, Windows Squirrel registry, `beforeReadyProtocolRegistration`):**
   - Реализованы [`ProtocolPrivilegeSpec`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1294-L1301), [`before_ready_protocol_registration`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1306-L1315) (`reference-oracle/main-arch-work/m-app-46312.txt`), [`desktop_protocol_handler_entry`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1349-L1357), [`mimeapps_scheme_entries`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1360-L1379) и [`windows_protocol_registry_commands`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1383-L1415) (`reference-oracle/main-arch-work/m-app-27251.txt`).
7. **Верификация клипов `disclip://` (`DISCORD_UUID` в MP4 + HTTP Range `200`/`206`/`416`/`404`) и `CustomProtocolHandler`:**
   - Реализованы [`DISCORD_CLIP_UUID_HEX`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L40), [`DISCORD_CLIP_UUID_BYTES`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L43-L45), [`DiscordMp4ErrorReason`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1420-L1427), [`verify_discord_mp4_bytes`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1456-L1536), [`verify_discord_mp4_file`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1540-L1568), [`serve_disclip_request`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1637-L1716) (`reference-oracle/main-arch-work/m-core-7635.txt`) и [`CustomProtocolHandler`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs#L1721-L1818).

## 3. Red-прогон (Strict TDD)
- **Команда:** `CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-platform --test deep_link_contract`
- **Лог:** [`waves/RW-40/G5/task-625-deep-link-handler/red.log`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/waves/RW-40/G5/task-625-deep-link-handler/red.log)
- **Результат:** ожидаемое падение компиляции (`EXIT_CODE=101`, 9 ошибок `E0432`/`E0599` на новых символах контракта `d18..d25` до реализации в `deep_link.rs`).

## 4. Green-прогон и Targeted-проверки (`T0`)
- **Команда:** `CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-platform`
- **Лог:** [`waves/RW-40/G5/task-625-deep-link-handler/green.log`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/waves/RW-40/G5/task-625-deep-link-handler/green.log)
- **Результат:** `25 passed; 0 failed` в `deep_link_contract` (`d01..d25`) и все `197 passed; 0 failed` по всему крейту `discord-platform`.
- **Форматирование и линтер:**
  - `rustfmt --edition 2024 --check rust/crates/discord-platform/src/deep_link.rs rust/crates/discord-platform/tests/deep_link_contract.rs` — `0` замечаний.
  - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo clippy --manifest-path rust/Cargo.toml -p discord-platform --all-targets -- -D warnings` — `0` предупреждений и ошибок.

## 5. Мутационное тестирование (`5/5 = 100%`)
- **Лог:** [`waves/RW-40/G5/task-625-deep-link-handler/mutations.log`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/waves/RW-40/G5/task-625-deep-link-handler/mutations.log)
- `M1` (`MAX_INVITE_CODE_LEN: 64 -> 63`) — **KILLED** (`d18_extended_re_routes_invite_gift_user_quest_handoff_connect`)
- `M2` (`is_safe_segment`: отключение проверки `segment == ".."`) — **KILLED** (`d21_path_traversal_and_percent_encoded_injection_blocked`)
- `M3` (`OAuth2AuthorizeParams::Debug`: вывод `state` без `<redacted>`) — **KILLED** (`d20_oauth2_authorize_query_params_and_redirect_uri_security`)
- `M4` (`DISCORD_CLIP_UUID_BYTES`: искажение первого байта `0xa1 -> 0xa0`) — **KILLED** (`d24_disclip_mp4_verification_and_range_serving_contract`)
- `M5` (`CustomProtocolHandler::handle_open_url`: пропуск инкремента `blocked_count` при ошибке) — **KILLED** (`d25_custom_protocol_handler_state_machine_and_metrics`)

## 6. Изменённые файлы и MD5
- `bf568e50cfc408fc26ecb3676f928cdf` — [`rust/crates/discord-platform/src/deep_link.rs`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/src/deep_link.rs)
- `1ec80b4f376d4e4d084e0df741e41f85` — [`rust/crates/discord-platform/tests/deep_link_contract.rs`](file:///home/user/orca/workspaces/project/task-625-deep-link-handler/rust/crates/discord-platform/tests/deep_link_contract.rs)

## 7. Риски и примечания
- Обратная совместимость со всеми существующими вызовами (`parse_deep_link`, `extract_deep_link`, `extract_url_from_args`, `route`, `main_window_path`, `DeepLinkQueue`, `classify_webview_navigation`, тесты `d01..d17`) полностью сохранена.
- Ни один файл за пределами назначенного скоупа не затронут.
