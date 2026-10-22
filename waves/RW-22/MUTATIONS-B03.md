# RW-22 / B03 — мутации

Дата: 2026-10-21. Итог: **18/18 caught**. Каждая мутация временно меняла продукт,
targeted-тест падал, затем файл восстанавливался из копии. Логи — `mutations/`.

| ID | Файл | Мутация | Тест | Лог | Итог |
|---|---|---|---|---|---|
| m-json-01 | `discord-json/src/lib.rs` | `depth > MAX_DEPTH` → `>=` | `depth_limit_is_enforced_without_panic` | `m-json-01.log` | caught |
| m-json-02 | `discord-json/src/lib.rs` | duplicate-key guard отключён | `rejects_malformed_and_dangerous_input` | `m-json-02.log` | caught |
| m-json-03 | `discord-json/src/lib.rs` | writer `\n` → `n` | `writer_round_trips_unicode_escapes_and_numbers` | `m-json-03.log` | caught |
| m-i18n-01 | `discord-i18n/src/lib.rs` | `MESSAGE_KEY_LEN` 6 → 5 | `hash_vectors_and_locale_selection` | `m-i18n-01.log` | caught |
| m-i18n-02 | `discord-i18n/src/lib.rs` | ru plural `+1` к числу | `plural_exact_and_fractional_categories_are_cldr_safe` | `m-i18n-02.log` | caught |
| m-i18n-03 | `discord-i18n/src/lib.rs` | разрешён `javascript:` | `markdown_is_safe_and_handles_unclosed_markers` | `m-i18n-03.log` | caught |
| m-i18n-04 | `discord-i18n/src/lib.rs` | fallback default → current | `number_formatting_uses_locale_separators_and_keeps_fraction` | `m-i18n-04.log` | caught |
| m-voice-01 | `discord-voice/src/lib.rs` | снята проверка длины secret key | `secret_key_is_decoded_validated_and_redacted` | `m-voice-01.log` | caught |
| m-voice-02 | `discord-voice/src/lib.rs` | tie-break региона инвертирован | `region_selection_orders_latency_ties_and_manual_choice` | `m-voice-02.log` | caught |
| m-voice-03 | `discord-voice/src/lib.rs` | `Debug` Other печатает raw | `unknown_payload_debug_does_not_leak_secrets` | `m-voice-03.log` | caught |
| m-voice-04 | `discord-voice/src/lib.rs` | не снимается `/?v=9` | `endpoint_with_existing_version_query_is_not_duplicated` | `m-voice-04.log` | caught |
| m-voice-05 | `discord-voice/src/lib.rs` | op8 → `Unknown(8)` | `capture_frames_match_signal_contract` | `m-voice-05.log` | caught |
| m-core-01 | `discord-core/src/rest/resources.rs` | отключена ASCII-цифровая проверка id | `path_renders_only_the_application_id` | `m-core-01.log` | caught |
| m-core-02 | `discord-core/src/rest/resources.rs` | Saved Messages 404 → 200 | `saved_messages_404_is_not_a_success_payload` | `m-core-02.log` | caught |
| m-core-03 | `discord-core/src/rest/resources.rs` | лишний id игнорируется | `path_renders_only_the_application_id` | `m-core-03.log` | caught |
| m-ci-01 | `scripts/ci.sh` | удалена строка cargo-deny | `ci_dry_run_prints_full_plan_without_running_tools`, `scripts_are...` | `m-ci-01.log` | caught |
| m-ci-02 | `scripts/ci.sh` | release.sh убран из плана | `ci_dry_run_prints_full_plan_without_running_tools` | `m-ci-02.log` | caught |
| m-ci-03 | `scripts/release.sh` | убран sha256sum | `release_dry_run_is_offline_and_writes_dist_only` | `m-ci-03.log` | caught |

## Примечание

`m-voice-01b.log` — **не считается**: первая версия теста secret key падала только из-за
отсутствия обязательных DAVE-полей, а не из-за неверной длины ключа. Тест был усилен
полным payload и повторён как `m-voice-01.log`; это пример проверки, что мутация ловит
именно заявленный контракт, а не побочную ошибку.

## Восстановление

После каждой мутации файл копировался обратно из `/tmp/opencode/*.bak`; итоговые md5
зафиксированы в `REPORT-B03.md` §5. Зелёный прогон после восстановления — `targeted-b03-final-*.log`,
`test-core-b03-final.log`, `test-workspace-b03-final.log`.
