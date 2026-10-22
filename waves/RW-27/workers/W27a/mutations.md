# Мутации — RW-27 / W27a (P7-01b ListFormat)

Правки только в `rust/crates/discord-i18n/src/list.rs`; после каждой мутации файл
восстановлен из копии, md5 сверен. Targeted-команда:

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w27a \
  cargo test --offline -p discord-i18n --test list_contract
```

- Исходный md5 `list.rs`: `38a3d1d5435ef29a9d061295ab74acb3`
- Все прогоны восстановления: `restored_ok=True`, итоговый md5 совпадает.
- Неэквивалентных поймано: **6/6**. Эквивалентных задокументировано: **2** (M3, M4);
  они ненаблюдаемы на замороженных данных, поэтому в знаменатель не входят.
- Авторитетный повторный прогон (TL-2, 2026-10-22, после регенерации таблицы):
  `waves/RW-27/mutations/w27a-final.log` — `caught=6/8 restored_all=True`.

| # | Мутация | exit | Упавшие тесты | md5 мутанта | Restore |
|---|---|---|---|---|---|
| M1 | `pair` → `start` (2 элемента) | 101 | `en_us_long_conjunction_scales_from_empty_to_five`, `pair_separator_depends_on_type_and_style`, `oracle_fixture_matches_all_1674_cases` | `fcee08a3958750b73dd1cc6ebc16a0bd` | ok |
| M2 | `start` ↔ `end` (3+ элемента) | 101 | 7 тестов (включая oracle) | `741ac7c2691ddaf03e159ccc8a251e10` | ok |
| M3 | `middle` → `start` (внутренний цикл) | 0 | — (эквивалент: во всех 279 строках `LIST_PATTERNS` `middle == start`) | `377f3f74daf5c82c3ec5ffe9e7cb842c` | ok |
| M3b | `middle` → `pair` (внутренний цикл) | 101 | `middle_separator_repeats_for_inner_items`, `en_us_long_conjunction_scales_from_empty_to_five`, `ru_long_conjunction_and_disjunction`, `oracle_fixture_matches_all_1674_cases` | `ba3d77a67a532e49e2c0d0c35b5f2636` | ok |
| M4 | убрать fallback-паттерн `en-US` (`.or_else(...)`) | 0 | — (эквивалент: таблица полная 31×3×3, для выключенной/неизвестной локали `resolved` уже `en-US`) | `de1c85dc5e53b34cafc5bbcdaad27bb7` | ok |
| M5 | 3+ ветка без цикла `middle` | 101 | 6 тестов (включая oracle) | `d70f56495e5ad21643cd64e8dad6adf4` | ok |
| M6 | пустой список → `" "` | 101 | `empty_and_single_item_lists_are_locale_independent`, `en_us_long_conjunction_scales_from_empty_to_five`, `oracle_fixture_matches_all_1674_cases` | `c9fed473b55c2cff2d0357ad6512b875` | ok |
| M7 | `end` → `middle` (3+ элемента) | 101 | 7 тестов (включая oracle) | `1c7eba7b5cd733f72c9ab862cf759f9b` | ok |

## Почему M3 и M4 эквивалентны

- **M3**: генератор выдал `middle == start` для всех 31 включённой локали × 3 стиля ×
  3 типа (279 строк `LIST_PATTERNS`), поэтому подмена слота поведенчески ненаблюдаема.
  Сам внутренний цикл проверяется M3b (`middle → pair`) — поймана.
- **M4**: после регенерации таблица содержит строку для каждой включённой локали, включая
  `pt-BR`; `resolved` всегда попадает в `LIST_PATTERNS`, поэтому `.or_else(en-US)` и
  ветка `join(", ")` не исполняются ни на одном входе. Это defensive-код, а не пробел
  тестов. Прежняя редакция отчёта (30 локалей, M4 «поймана») относилась к таблице до
  регенерации и заменена этим прогоном.
