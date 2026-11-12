# TZ-W25a — locale / CLDR plural / number-date-time (P7-01)

Ты — **W25a**, кодовый воркер. Пиши только файлы из §1. `src/lib.rs` — интеграционный,
не трогай. Сначала red-тесты, затем минимальный green. Оракул — `tests/data/cldr-oracle.json`
(сгенерирован Node `Intl`, offline ICU, UTC) и `src/format_data.rs` (сгенерированные
CLDR-паттерны/имена). Не редактируй эти два файла и не подгоняй тесты под код.

## 1. Файлы

Пишешь:
- `rust/crates/discord-i18n/src/locale.rs`
- `rust/crates/discord-i18n/src/plural.rs`
- `rust/crates/discord-i18n/src/format.rs`
- `rust/crates/discord-i18n/tests/locale_contract.rs`
- `rust/crates/discord-i18n/tests/plural_contract.rs`
- `rust/crates/discord-i18n/tests/format_contract.rs`

Читаешь, но не меняешь:
- `rust/crates/discord-i18n/src/format_data.rs`
- `rust/crates/discord-i18n/tests/data/cldr-oracle.json`
- `docs/re/i18n-deep.md` §2, §5–7
- `waves/RW-24/WAVE-PLAN.md`

Не трогаешь: `src/lib.rs`, `src/markdown.rs`, `Cargo.toml`, `waves/*`, любые чужие
тесты/доки. Зависимостей не добавлять.

## 2. API (заморожен)

См. `WAVE-PLAN.md` §3.1–3.3. Сигнатуры должны совпасть буквально; добавлять публичные
хелперы можно, но не менять существующие.

`format_data.rs` уже содержит `pub(crate) struct LocaleFormatData` с полями:
`date_patterns[4]`, `time_patterns[4]`, `months_short[12]`, `months_long[12]`,
`weekdays_short[7]`, `weekdays_long[7]`, `day_periods[2]`, `timezone_short`,
`timezone_long`, `era`, `year_offset`, `group`, `decimal`, `group_primary`,
`group_secondary`. `format_data::data(locale)` возвращает en-US для неизвестной.

## 3. Плурализация

`plural_category` работает для всех 36 кодов (включая отключённые). Округляй/абсолють
вход; `v` — число видимых дробных цифр (для `1.5` v=1; для `2.0` v=0). Для f64,
полученных из `Value::Float`, достаточно `to_string()` без хвостовых нулей.

Cardinal (язык — часть до `-`, lower-case):
- `ja ko zh th vi id` — always `other`.
- `en de nl sv` — `one` iff `i == 1 && v == 0`.
- `da` — `one` iff `n == 1 || (t != 0 && (i == 0 || i == 1))`.
- `no fi bg el hu tr` — `one` iff `n == 1`.
- `es` — `one` iff `n == 1`; `many` iff `v == 0 && i != 0 && i % 1_000_000 == 0`;
  иначе `other`.
- `pt-PT` — `one` iff `i == 1 && v == 0`; `many` для точных миллионов; иначе `other`.
- `pt-BR` — `one` iff `i == 0 || i == 1`; `many` для точных миллионов; иначе `other`.
- `fr` — `one` iff `i == 0 || i == 1`; `many` для точных миллионов; иначе `other`.
- `it` — `one` iff `i == 1 && v == 0`; `many` для точных миллионов; иначе `other`.
- `hi` — `one` iff `i == 0 || n == 1`; иначе `other`.
- `ru uk pl` — `one` iff `v == 0 && i % 10 == 1 && i % 100 != 11`; `few` iff
  `v == 0 && 2..=4` последняя цифра и `i % 100` не 12..=14; `many` iff `v == 0` и
  (`i % 10 == 0 || 5..=9` или `i % 100` 11..=14); иначе `other`.
- `cs sk` — `one` iff `i == 1 && v == 0`; `few` iff `i` 2..=4 && v==0; `many` iff
  `v != 0`; иначе `other`.
- `hr` — `one` iff (`i%10==1 && i%100!=11`) или (`f%10==1 && f%100!=11`); `few` iff
  (`i%10` 2..=4 && `i%100` не 12..=14) или (`f%10` 2..=4 && `f%100` не 12..=14);
  иначе `other`.
- `ro` — `one` iff `i == 1 && v == 0`; `few` iff `v != 0 || n == 0 || n%100` 2..=19;
  иначе `other`.
- `lt` — `one` iff `n%10==1 && n%100` не 11..=19; `few` iff `n%10` 2..=9 && `n%100`
  не 11..=19; `many` iff `f != 0`; иначе `other`.
- `ar` — `zero` n=0; `one` n=1; `two` n=2; `few` n%100 3..=10; `many` n%100 11..=99;
  иначе `other`.
- `he` — `one` iff (`i==1 && v==0`) || (`i==0 && v!=0`); `two` iff `i==2 && v==0`;
  иначе `other`. **Уточнение после oracle:** Node 24 / ICU 78.3 / CLDR 48 больше не
  возвращает `many` для `he` (`pluralCategories = one,two,other`; 10/20 → `other`).
  Прежнее правило `many` было устаревшим; источник истины — `cldr-oracle.json`.
- fallback — как `en` (one/other).

Ordinal:
- `en` — one/two/few по последней цифре 1/2/3 при `n%100` не 11/12/13; иначе other.
- `fr ro vi` — one iff n==1; иначе other.
- `hu` — one iff n==1 || n==5; иначе other.
- `it` — many iff n ∈ {8,11,80,800}; иначе other.
- `sv` — one iff `n%10` 1..=2 && `n%100` не 11/12; иначе other.
- `uk` — few iff `n%10==3 && n%100!=13`; иначе other.
- `hi` — one n=1; two n=2/3; few n=4; many n=6; иначе other.
- остальные — other.

Тест `plural_contract.rs` читает JSON через `discord_json::parse`, обходит все 36
локалей и все значения из `plural.cardinal`/`plural.ordinal`, сравнивает
`as_str()`. Отдельно: `ru` 1/2/5/11/21/22/101; отрицательные (`-1` → one для ru);
`1.5` → other для ru; `da` 1.5 → one; `he` 0.5 → one; `ar` 0/1/2/3/11;
`hi` ordinal 1/2/4/6.

## 4. Числа

`format_number`:
- `value` округлить/обрезать до max 3 дробных; убрать хвостовые нули, но оставить
  минимум; `-0` → `0`; `NaN/inf` → `0` (без паники).
- grouping: если `group` пуст — без разделителей; иначе группы справа: последняя
  `group_primary`, далее `group_secondary` (для `hi` 3/2).
- `format_integer` — max 0; `format_number_with` — уважает options; `minimum_fraction_digits`
  дополняет нулями; `use_grouping=false` — без групп.
- `format_percent(ratio)` — `ratio*100`, max 0, добавить `%` (для тестов достаточно
  `"25%"`/`"25 %"` по oracle; проверь `Intl.NumberFormat(locale,{style:"percent"})`).
- `format_currency(value, locale, currency)` — поддержать USD/EUR/RUB/GBP/JPY;
  символ/позиция по локали (en `$1,234.50`, ru `1 234,50 ₽`, de `1.234,50 €`,
  fr `1 234,50 €`, ja `¥1,235`); неизвестная валюта — `"1234.50 XXX"` без паники.
- `format_number_named`: `integer` → max 0; `percent` → percent; `currency` → USD;
  иначе decimal (сохраняет `[2,"x"]` → `1.5`).

Тест `format_contract.rs`: все 36 локалей × значения `0,1,1.5,1234,1234567,1234567.89`
против `number` oracle; `ru` NBSP; `fr` U+202F; `hi` 12,34,56,789; `format_integer`;
`format_percent(0.25,"en-US") == "25%"`; currency для 5 локалей; no panic NaN/inf.

## 5. Даты/время

UTC; timestamp — Unix **секунды**. `timestamp_secs.div_euclid(86_400)` дни,
`rem_euclid` секунды. Гражданская дата — алгоритм Howard Hinnant (без зависимостей).
Неделя: 1970-01-01 = Thursday, индекс 0=Sunday.

Паттерны — из `format_data`; токены:
`y yy M MM MMM MMMM d dd EEE EEEE G H HH h hh m mm s ss a z zzzz`.
Литералы в паттерне — в одинарных кавычках, `''` — literal quote. Неизвестный токен
в сгенерированных паттернах не должен встречаться; в skeleton — вернуть `None`.
`G` — `era` (для `th`), `year_offset` прибавлять к году (th +543). `a` — `day_periods`
(0=AM, 1=PM); для 24h пусто. `z`/`zzzz` — timezone_short/long. `h` — 12h (0→12).
`format_date_skeleton`/`format_time_skeleton`: принять `M/d`, `::MMMMd`,
`yyyy-MM-dd`, `HH:mm`, `MMMMd`; `::` отбросить; иначе `None`. Для skeleton с `M/d`
без ведущих нулей; `MMMMd` — «April 5» (без запятой). Не паниковать на мусоре.

Тест `format_contract.rs`: все 36 × 4 date styles и 4 time styles против oracle
`dates`/`times` (timestamp из oracle); `th` full содержит `พ.ศ.` и 2567; `ru` long
`5 апреля 2024 г.`; `fr`/`ru` number separators; skeleton `M/d`, `::MMMMd`;
неизвестный skeleton → `None`; отрицательный timestamp не паникует.

## 6. Red/green

1. Сначала создай тесты; запусти:
   `flock /tmp/opencode/discord-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --test plural_contract`
   (и locale/format). Зафиксируй падение в `waves/RW-24/red-W25a.log` (команда + вывод).
2. Затем код; targeted зелёные. Сохрани `green-W25a.log`.
3. `cargo fmt --all -- --check` и `cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-i18n --all-targets -- -D warnings`
   (под flock, если параллельно другие cargo). Не меняй чужие файлы.
4. В отчёте: что сделано, red/green, md5 своих файлов, риски. Не запускай полный
   workspace/gate — это integration lane.

## 7. Запреты

- Никаких сетевых вызовов, новых зависимостей, `src/lib.rs`, `format_data.rs`,
  `tests/data/*`, `Cargo.lock`.
- Не «зеленить» тесты правкой ожиданий; oracle — источник истины.
- Не трогать `logs/`, `functions/`, `evidence/`, `reference-oracle/`.
- Не коммитить.
