# TEST-MATRIX B05 / RW-24 — P7-01

| Требование P7-01 | RE-источник | Тест | Файл | Оракул |
|---|---|---|---|---|
| 36 кодов, 31 enabled, 5 disabled | `i18n-deep.md` §7; `inline/lang-list.json` | `locale_registry_matches_asset_list`, `disabled_locales_rejected` | `tests/locale_contract.rs` | список 36/31/5 в контракте |
| resolve requested → default, отключённые не выбираются | §7 | `resolve_locale_prefers_exact_then_language_then_default` | `tests/locale_contract.rs` | таблица кейсов |
| fallback chain без циклов | §7 | `fallback_chain_is_requested_then_default` | `tests/locale_contract.rs` | — |
| CLDR cardinal 36 локалей | §2 | `cardinal_matches_icu_oracle` (36×21) | `tests/plural_contract.rs` | `cldr-oracle.json` |
| CLDR ordinal 36 локалей | §2 | `ordinal_matches_icu_oracle` (36×21) | `tests/plural_contract.rs` | `cldr-oracle.json` |
| ru one/few/many + дроби | §2 | `ru_categories_cover_1_2_5_11_21_22_101` | `tests/plural_contract.rs` | ICU |
| точные `=N`, offset, `#` | §2, §5 | `exact_selector_offset_and_pound` | `tests/i18n_contract.rs` | реальный ru-каталог |
| числа по локали | §6 | `number_matches_icu_oracle` (36×6) | `tests/format_contract.rs` | `cldr-oracle.json` |
| ru NBSP, fr U+202F, hi 3/2 | §6 | `number_separators_and_grouping` | `tests/format_contract.rs` | ICU |
| integer/percent/currency | §1, §6 | `number_styles` | `tests/format_contract.rs` | ICU/таблица |
| дата 4 стиля × 36 | §5 | `date_matches_icu_oracle` (36×4) | `tests/format_contract.rs` | `cldr-oracle.json` |
| время 4 стиля × 36 | §5 | `time_matches_icu_oracle` (36×4) | `tests/format_contract.rs` | `cldr-oracle.json` |
| th эра +543, ru genitive | §5 | `thai_era_and_ru_month_context` | `tests/format_contract.rs` | ICU |
| skeleton `M/d`, `::MMMMd` | §5 | `skeletons_supported_and_unknown_none` | `tests/format_contract.rs` | ICU |
| 23 markdown-правила | §3 | `markdown_23_rules` (23 asserts) | `tests/markdown_contract.rs` | таблица TZ-W25b §4 |
| безопасные URL / HTML | §3 | `unsafe_urls_dropped_and_html_is_text` | `tests/markdown_contract.rs` | RE regex/sanitizeUrl |
| незакрытые маркеры, fuzz-lite | §3 | `unclosed_and_fuzz_do_not_panic` | `tests/markdown_contract.rs` | — |
| реальные ru/en каталоги | §0, §7 | `real_catalogs_still_parse` | `tests/i18n_contract.rs` | 23 836 / 23 903 |
| date/time AST | §1, §5 | `catalog_date_time_nodes_render` | `tests/i18n_contract.rs` | каталог + ICU |
| fallback requested → en-US | §7 | `missing_key_falls_back_to_default` | `tests/i18n_contract.rs` | — |
| 31 insert / 5 reject / live switch | §7 | `all_enabled_catalogs_insert_and_switch` | `tests/i18n_contract.rs` | список |

## Целевые числа

- `cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --test locale_contract` — ≥5
- `... --test plural_contract` — 2 (но 36×21 внутри)
- `... --test format_contract` — ≥6
- `... --test markdown_contract` — ≥6 (23 правила в таблице)
- `... --test i18n_contract` — ≥5
- `--lib` — 8 старых + адаптированные
- workspace — без регрессий (638 до B05; после — больше)
