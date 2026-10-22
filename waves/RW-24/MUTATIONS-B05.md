# MUTATIONS B05 / RW-24 — P7-01

Статус: **ожидает green W25a/W25b**. QA-MUT — отдельный агент, не автор кода.
Каждая мутация: временная правка продукта → targeted test должен упасть → restore по
md5 → запись `caught`/`missed`. Не считать мутацию, если тест не компилируется по
посторонней причине.

| # | Файл | Мутация | Ожидаемый тест | Статус |
|---:|---|---|---|---|
| 1 | `plural.rs` | ru few `2..=4` → `2..=5` | `ru_categories_cover...` | caught |
| 2 | `plural.rs` | pl many не учитывает 11–14 | `cardinal_matches_icu_oracle` | caught |
| 3 | `plural.rs` | ar zero/two переставлены | `cardinal_matches_icu_oracle` | caught |
| 4 | `plural.rs` | fr million `many` → `other` | `cardinal_matches_icu_oracle` | caught |
| 5 | `plural.rs` | en ordinal 11 → one | `ordinal_matches_icu_oracle` | caught |
| 6 | `plural.rs` | he 0.5 → other | `cardinal_matches_icu_oracle` | caught |
| 7 | `format.rs` | ru group NBSP → space | `number_separators_and_grouping` | caught |
| 8 | `format.rs` | fr decimal `,` → `.` | `number_matches_icu_oracle` | caught |
| 9 | `format.rs` | hi group secondary 2 → 3 | `number_separators_and_grouping` | caught |
| 10 | `format.rs` | ru long month → short | `date_matches_icu_oracle` | caught |
| 11 | `format.rs` | th year offset 543 → 0 | `thai_era_and_ru_month_context` | caught |
| 12 | `format.rs` | time dayPeriod AM/PM swap | `time_matches_icu_oracle` | caught |
| 13 | `markdown.rs` | bold `**` → `*` | `markdown_23_rules` | caught |
| 14 | `markdown.rs` | unsafe URL allowed | `unsafe_urls_dropped...` | caught |
| 15 | `markdown.rs` | list depth 11 → 12 | `unclosed_and_fuzz...` | caught |
| 16 | `markdown.rs` | escape `\*` оставляет `\` | `markdown_23_rules` | caught |
| 17 | `locale.rs` | disabled filter убран | `disabled_locales_rejected` | caught |
| 18 | `locale.rs` | fallback chain теряет default | `fallback_chain...` | caught |
| 19 | `lib.rs` | plural exact selector `=N` отключён | `exact_selector_offset_and_pound` | caught |
| 20 | `lib.rs` | date style игнорируется (medium) | `catalog_date_time_nodes_render` | caught |

Цель: ≥18 caught, 0 missed; 2 inconclusive допустимы только с объяснением.


## Результат прогона (2026-10-21)

Все 20 мутаций применены к текущему продукту, targeted-тесты запущены под
`flock /tmp/opencode/discord-cargo.lock`, исходники восстановлены и сверены по md5.

- caught: **20/20**
- missed: **0**
- inconclusive: **0**
- runner: `run-mutations.py`; результаты — `mutations/results.json`, логи — `mutations/mutation-*.log`
- фактические targeted-имена: `every_oracle_cardinal_and_ordinal_sample_matches`,
  `all_locale_numbers_match_the_independent_intl_oracle`,
  `all_locale_dates_and_times_match_the_independent_intl_oracle`,
  `renders_the_23_frozen_markdown_rules`, `rejects_unsafe_urls_but_keeps_labels`,
  `list_depth_limit_keeps_the_thirteenth_level_literal`,
  `registry_and_disabled_locales_match_the_frozen_inventory`,
  `locale_resolution_uses_available_enabled_exact_then_language_then_default`,
  `exact_selector_offset_and_pound_use_plural_rules`,
  `catalog_date_time_and_number_nodes_render_from_icu_oracle`.
