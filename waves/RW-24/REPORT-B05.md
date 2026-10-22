# Отчёт B05 / RW-24 — P7-01 (полный i18n)

Обновлено: 2026-10-21. Статус: **готов к приёмке**; review-фиксы закрыты,
targeted/workspace зелёные, gate passed, мутации 20/20.

## 1. Состав и результат

| Карточка | Результат | Тесты |
|---|---|---|
| P7-01 locale registry/fallback | 36 кодов, 31 включён, 5 отключены; exact → case-insensitive → language → en-US; canonical/fallback chain | `locale_contract` — **2/2** |
| P7-01 CLDR cardinal/ordinal | 36 локалей, cardinal/ordinal, `=N`, offset, дроби, отрицательные; независимый Node ICU oracle | `plural_contract` — **2/2** (56 значений × 36 × 2 типа) |
| P7-01 числа/даты/время | группировка 3/2 и NBSP/U+202F, integer/percent/currency, 4 date/time style, skeleton, th +543, timezone | `format_contract` — **3/3** (36 × 2 timestamp × 4 style) |
| P7-01 markdown 23 правила | block/inline, escape, mentions/emoji/timestamp/sound/static route, safe URL, UTF-8/fuzz, nested list, depth 11 | `markdown_contract` — **12/12** |
| Интеграция AST/fallback | `Ast::Date/Time` style, Number/Plural через модули, `InvalidArgument`, disabled locales, реальные ru/en каталоги | `i18n_contract` — **6/6** |
| Старые контракты | 8 lib-тестов сохранены | `discord-i18n` lib — **8/8** |

Итого `discord-i18n`: **33/33**; workspace: **663 passed / 0 failed**; `discord-core`: **530**.
`cargo fmt --all -- --check` — чисто; workspace clippy `-D warnings` — 0.

## 2. Что сделано

- `locale.rs`: реестр 36/31/5, `canonical_locale`, `is_enabled`, `resolve_locale`,
  `fallback_chain`, `locale_language`; отключённые локали не выбираются, fallback на en-US.
- `plural.rs`: CLDR cardinal/ordinal для всех 36 кодов, точные миллионы, дроби, `v/f/t`,
  отрицательные, ordinal en/fr/ro/vi/hu/it/sv/uk/hi; `he` соответствует ICU 78
  (`one/two/other`, без `many`).
- `format.rs`: группировка primary/secondary (hi 3/2), NBSP/U+202F, integer/percent/
  currency (USD/EUR/RUB/GBP/JPY + неизвестная валюта), даты/время UTC по Hinnant,
  th +543, localized month/weekday/dayPeriod/timezone, skeletons.
- `markdown.rs`: 23 правила, безопасные URL, HTML как текст, bounded inline/list depth,
  вложенные списки с indentation, saturating ordered numbers, точные timestamp/slash
  токены, fuzz-lite 2 000 строк + UTF-8 boundary.
- `lib.rs`: удалены дубли, новые модули подключены к AST, `Date/Time` style,
  exact `=N` до offset, fallback current → default → en-US, `InvalidArgument` для
  не-числа, canonical catalog keys.
- `format_data.rs` + `cldr-oracle.json`: 36 локалей, 4 date/time style, месяцы/дни/
  dayPeriod/timezone/era; генератор `gen-format-data.mjs` воспроизводим offline.

## 3. Red / green

| Что | Лог | Результат |
|---|---|---|
| W25a red/green | `red-W25a.log`, `green-W25a.log` | red exit 101; green 7/7 |
| W25b red/green | `red-W25b.log`, `green-W25b.log` | red 0/5 exit 101; green 5/5 |
| Review A red/green | `red-review-A-format.log`, `red-review-A-i18n.log`; `green-review-A-*.log` | 3/3, 6/6 |
| Review B red/green | `red-review-B05.log`, `red-review-B2.log`; `green-review-B05.log`, `green-review-B2.log` | red 3 fail / 8, затем 3 fail / 11; green 12/12 |
| Усиление oracle | второй timestamp / 56 plural values | поймало `cs` `HH` vs `H`; green после генератора |
| fmt / clippy | — | fmt чисто, clippy 0 |
| workspace | `workspace-test-b05.log`, `workspace-clippy-b05.log` | **663 passed / 0 failed**, clippy 0 |
| gate | `gate-worker-b05-final.log`; `.gate/worker-2026-10-21T08-58-08-129Z.json` | **passed** |

## 4. Независимое ревью

- REV-A (locale/plural/format/lib): P1 exact/offset, P2 currency, P2 Number string
  закрыты red-first; P2 `he many` и NBSP — **не баги** для ICU 78/`format()`, подтверждено
  oracle и `resolvedOptions().pluralCategories` (см. `REVIEW-RESOLUTION-B05.md`).
- REV-B (markdown): P1 list overflow, P2 nested lists, P2 timestamp trailing, P2 slash
  charset/length закрыты red-first; `</cmd arg:123>` с пробелом сохранён по контракту.
- Авторы свой код не ревьюили; итог P0/P1/P2=0, P3 — backlog.

## 5. Мутации

**20/20 caught, 0 missed, 0 inconclusive.** Подробности — `MUTATIONS-B05.md`,
`MUTATION-REPORT-B05.md`, `mutations/results.json`. Исходники восстановлены по md5.

## 6. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-i18n/src/locale.rs` | `25a8c4d0cf86d067e0d8b320d17261ac` |
| `rust/crates/discord-i18n/src/plural.rs` | `e34095b967a5a6aece93a6b79a64c726` |
| `rust/crates/discord-i18n/src/format.rs` | `2ce60ebb1223638b562f5ba1108b9c46` |
| `rust/crates/discord-i18n/src/format_data.rs` | `2d30c803b5a27cc7ba5ddc9ad0061ff3` |
| `rust/crates/discord-i18n/src/markdown.rs` | `6251a8dc1ca28027f393cd454f09b3dd` |
| `rust/crates/discord-i18n/src/lib.rs` | `d541856a3cf44a43ab6ccc5a122d9e05` |
| `tests/locale_contract.rs` | `859b387dd043b031d1685a2cc273cc7d` |
| `tests/plural_contract.rs` | `22b11f9fcaa341221705488c83c3c0e3` |
| `tests/format_contract.rs` | `94db0f5ad54a7cfd623644d10caacb60` |
| `tests/markdown_contract.rs` | `4c6959babd1134691668499557e334b6` |
| `tests/i18n_contract.rs` | `6d120f9ec21f6b48df2f53fdc4d5e7cb` |
| `tests/data/cldr-oracle.json` | `e3ee5688307588fba1616fe763625435` |
| `waves/RW-24/gen-format-data.mjs` | `92a301cf047ed123310cdf53158e3cce` |

Полный список — `MD5SUMS.txt`.

## 7. Риски / P3 backlog

- `$link` URL/аргумент — подтверждённого URL в RE нет (P3-15).
- Полные переводы 31 локали, `ListFormat`/`RelativeTimeFormat`/`DurationFormat`,
  `<t:...:R>` UI-таймеры — вне P7-01.
- `format_percent` не добавляет локальный пробел перед `%` для ru/fr/de (P3).
- `he`-правило в `TZ-W25a.md` устарело относительно ICU 78; код и oracle согласованы.
- Slash-команда с пробелами в аргументах принята по контракту; строгая грамматика
  аргументов — P3.
- Внешние зависимости/сеть/live/Discord/SQLite/keyring/GUI не добавлялись и не трогались.
