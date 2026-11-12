# RW-22 / B03 — тест-матрица

Дата: 2026-10-21. Карточки: P3-15, P4-01, P7-04, P8-03. Foundation: `discord-json`.

## 1. Итоговые числа

| Проверка | Результат |
|---|---:|
| `discord-json` | **11 passed / 0 failed** |
| `discord-i18n` | **8 passed / 0 failed** |
| `discord-voice` | **12 passed / 0 failed** (1 unit + 11 integration) |
| `discord-core` `rest_resources` | **6 passed / 0 failed** |
| `discord-conformance` `ci_contract` | **4 passed / 0 failed** |
| `discord-core` (весь крейт) | **530 passed / 0 failed** |
| workspace | **618 passed / 0 failed** |
| fmt / clippy | чисто, `-D warnings` |

## 2. P3-15 — i18n runtime

| Контракт | Тест | Независимый оракул |
|---|---|---|
| `xxh64`/`message_key` | `hash_vectors_and_locale_selection` | 6 векторов из RE: `ru`, `en-US`, `AKA`, `OK`, `PS4`, `PS5`; пустая строка |
| 36/31/5 локалей, exact/case/base/disabled | `locale_math_is_consistent`, `hash_vectors_and_locale_selection` | `docs/re/i18n-deep.md` §7 / `inline/lang-list.json` |
| Каталог/AST/fallback/plural/tags | `catalog_translate_plural_and_rich_tags`, `plural_exact_and_fractional_categories_are_cldr_safe`, `number_formatting_uses_locale_separators_and_keeps_fraction` | синтетические AST + CLDR-категории; `=0`, `=2`, offset, ru/en, `#` |
| Реальные ru/en каталоги | `real_catalogs_match_independent_inventory` | `reference-oracle/{ru,en}-catalog.json`: 23 836 / 23 903, `hPBScv`, `323362` с `$link` |
| Markdown/XSS/panic | `markdown_is_safe_and_handles_unclosed_markers` | `javascript:`, смешанные ссылки, вложенные скобки, 10 000 truncation-префиксов |

## 3. P4-01 — headless RTC signaling

| Контракт | Тест | Независимый оракул |
|---|---|---|
| 9 op, mapping, unknown | `all_observed_ops_round_trip_and_unknown_is_preserved` | `OBSERVED_RTC_OPS` + `rtc1/2` |
| Реальные RTC-кадры | `capture_frames_match_signal_contract` | `rtc1-frames.jsonl`, `rtc2-frames.jsonl`: Hello 8/13 750, Ready 7142/19 334, streams 7143/7144/rid 100, Session Description 32-byte key/DAVE/secure |
| Identify | `outgoing_identify_and_select_protocol_are_wire_serializable`, `identify...` | capture-поля + wire JSON |
| Select Protocol | `select_protocol_codecs_match_capture_contract` | udp, opus PT120 priority1000, AV1 PT101; верхнеуровневые `address`/`port`/`mode` |
| Secret redaction | `secret_key_is_decoded_validated_and_redacted`, `unknown_payload_debug_does_not_leak_secrets` | 32-byte base64, invalid length/alphabet, `Debug` без secret/token/raw |
| Endpoint | `voice_server_update_builds_valid_websocket_url_without_leaking_token`, `endpoint_with_existing_version_query_is_not_duplicated` | bare host, `wss://`, `/?v=9` без дубля; reject `http/ws/path/query/@` |
| Region | `region_selection_orders_latency_ties_and_manual_choice` | latency, geo tie, lexical, manual, empty |
| Panic safety | `every_prefix_of_frame_is_panic_free` | 10 000 префиксов |

## 4. P7-04 — read-only REST surface

| Контракт | Тест | Независимый оракул |
|---|---|---|
| 8 kind → 8 точных путей | `catalog_matches_d02_and_get_routes`, `catalog_and_paths_are_deterministic` | `docs/re/rest-d02.json` |
| GET-only | `catalog_matches_d02_and_get_routes` | `routes::ROUTES` + D-02 |
| D-02 пути/статусы | `catalog_matches_independent_d02_json_oracle` | `/api/v9...`, `GET`, `statuses` |
| application id | `path_renders_only_the_application_id` | missing/empty/non-digit/too-long/extra id |
| Saved Messages 404 | `saved_messages_404_is_not_a_success_payload` | `[404]`, не success payload |
| Ошибки без утечки входа | `errors_are_stable_and_do_not_disclose_input` | `Display` не содержит id целиком |

Уточнение к `WAVE-PLAN` §6: `TZ-W22c` для этих восьми D-02 маршрутов не подтверждает query,
поэтому `ResourceQuery` в P7-04 не вводится; путь остаётся относительным, без `/api/v9` и
без query, как в `routes::ROUTES`. Это осознанная граница карточки, а не пропуск.

## 5. P8-03 — CI/release

| Контракт | Тест | Проверка |
|---|---|---|
| dry-run план | `ci_dry_run_prints_full_plan_without_running_tools` | fmt/clippy/test/deny/bench/release, без curl/wget |
| release offline | `release_dry_run_is_offline_and_writes_dist_only` | `--offline`, `dist`, `sha256sum`, без scp |
| syntax/no network clients | `scripts_are_syntactically_valid_and_do_not_use_network_clients` | `bash -n`, запрещённые команды |
| workflow = local contract | `workflow_uses_the_same_ci_contract` | `scripts/ci.sh`, `scripts/release.sh`, без `DISCORD_TOKEN` |
| реальный прогон | `scripts/ci.sh` | workspace 618, clippy, bench, release, `ci: OK` |
| артефакт | `scripts/release.sh` | `rust/dist/discord-rs` 445 040 B, `SHA256SUMS` |

## 6. Red-first

Основные red-логи: `red-w22a.log`, `red-w22b.log`, `red-w22c.log` (compile/behavior-red,
exit 101). Дополнительные red-фиксы: `red-w22a-fixes.log`, `red-w22b-review-fixes.log`,
`red-w22b-stream.log`; green: `green-w22a*.log`, `green-w22b*.log`, `green-w22c*.log`.
Red-тест fallback был усилен: раньше оба каталога содержали ключ и fallback не проверялся.
