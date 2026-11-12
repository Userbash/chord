# RW-24 / B05 — P7-01 «Полный I18N»: контракт и файловая матрица

Статус: **заморожен до кода** (Team Lead, 2026-10-21). Источник: `docs/re/i18n-deep.md`
§1–3, §5–7; `docs/rust-client/PLAN.md` P7-01; `docs/rust-client/TEST-PLAN.md`
T-I18N-04…08, T-MSG-01; реальные каталоги `reference-oracle/{ru,en}-catalog.json`
(только чтение).

## 1. DoD

1. Реестр **36** кодов, **31** включённая локаль, 5 отключённых; `resolve_locale` и
   `fallback_chain` дают `запрошенная → en-US`, не выбирают отключённые.
2. CLDR cardinal **и** ordinal для всех 36 кодов (включая отключённые как чистую функцию),
   точные селекторы `=N` и `=N.M`, `offset`, `#` с локальным числом; ветка `other`
   обязательна, отсутствие нужной ветви — ошибка.
3. Числа: группировка/десятичный разделитель по локали, integer/percent/currency,
   без потери дробной части; `ru` — `U+00A0`, `fr` — `U+202F` (по данным ICU).
4. Даты/время: стили `short/medium/long/full`, UTC, 36 локалей; реальные CLDR-паттерны
   (порядок, разделители, месяцы в контексте даты, недели, dayPeriod, эра `th` +543).
   Скелеты `M/d`, `::MMMMd`, `yyyy-MM-dd`, `HH:mm` — без паник; неизвестный — `None`.
5. Markdown сообщений: **23 правила** из `docs/re/i18n-deep.md` §3, безопасные ссылки,
   escape, лимиты глубины/списка, отсутствие паник на обрезанном входе.
6. `I18n` реально использует новые модули: `Date/Time/Number/Plural`, fallback
   `requested → en-US`; реальные ru/en каталоги по-прежнему парсятся (23 836 / 23 903).
7. Targeted-тесты красные до кода; после кода зелёные; мутации по плану; затем один
   batch gate и независимое ревью. Только после этого — accepted.

## 2. Файловая матрица (непересекающаяся)

| Владелец | Пишет | Не трогает |
|---|---|---|
| **W25a** (locale/plural/format) | `src/locale.rs`, `src/plural.rs`, `src/format.rs`, `tests/locale_contract.rs`, `tests/plural_contract.rs`, `tests/format_contract.rs` | `src/lib.rs`, `src/markdown.rs`, `src/format_data.rs`, `tests/data/*` |
| **W25b** (markdown) | `src/markdown.rs`, `tests/markdown_contract.rs` | `src/lib.rs`, `src/locale.rs`, `src/plural.rs`, `src/format.rs`, `src/format_data.rs`, `tests/data/*` |
| **TL / integration** | `src/lib.rs`, `src/format_data.rs`, `waves/RW-24/*`, `tests/i18n_contract.rs`, доки | продуктовые файлы W25a/W25b |
| **QA-MUT** | временные мутации + `waves/RW-24/MUTATIONS-B05.md` | всё остальное; restore по md5 |

`src/format_data.rs` и `tests/data/cldr-oracle.json` — **сгенерированные оракулы**,
не редактировать воркерам. Источник: Node `Intl` (offline ICU, `timeZone: "UTC"`),
генератор — `waves/RW-24/gen-format-data.mjs`; фикстура — `tests/data/cldr-oracle.json`.

## 3. Замороженный API

### 3.1 `locale.rs`

```rust
pub const TOTAL_LOCALES: usize;             // 36
pub const ENABLED_LOCALES: usize;           // 31
pub const SUPPORTED_LOCALES: [&str; 36];
pub const DEFAULT_LOCALE: &str;             // "en-US"
pub const DISABLED_LOCALES: [&str; 5];

pub fn is_supported(code: &str) -> bool;
pub fn is_enabled(code: &str) -> bool;
pub fn canonical_locale(code: &str) -> Option<&'static str>;
pub fn locale_language(code: &str) -> &str;       // "pt-BR" -> "pt"
pub fn resolve_locale(requested: &str, available: &[&str]) -> String;
pub fn fallback_chain(requested: &str, available: &[&str]) -> Vec<String>;
```

`resolve_locale`: точное совпадение → case-insensitive → базовая язык-часть →
`DEFAULT_LOCALE`; отключённые и отсутствующие в `available` не выбираются. Пустой
`requested` → default. `fallback_chain`: `[resolved, default]` без дублей; для
`resolved == default` — один элемент.

### 3.2 `plural.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralCategory { Zero, One, Two, Few, Many, Other }
impl PluralCategory { pub fn as_str(self) -> &'static str; }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralType { Cardinal, Ordinal }

pub fn plural_category(locale: &str, value: f64, kind: PluralType) -> PluralCategory;
pub fn cardinal(locale: &str, value: f64) -> PluralCategory;
pub fn ordinal(locale: &str, value: f64) -> PluralCategory;
```

CLDR-правила по языкам; `abs`, дробная часть (`v != 0`) в `other` там, где CLDR так
говорит; `ru/uk/pl/cs/sk/hr/ro/lt/ar/he` — свои ветви; `ja/zh/ko/th/vi/id` — `other`;
`fr/es/pt/it` — `many` для точных миллионов. Ordinal: `en`, `fr`, `hu`, `it`, `sv`,
`uk`, `ro`, `vi`, `hi` (остальные — `other`). Оракул — `tests/data/cldr-oracle.json`.

### 3.3 `format.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle { Short, Medium, Long, Full }
impl DateStyle { pub fn parse(s: &str) -> Option<Self>; }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberStyle { Decimal, Integer, Percent, Currency }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberOptions {
    pub style: NumberStyle,
    pub minimum_fraction_digits: u8,
    pub maximum_fraction_digits: u8,
    pub use_grouping: bool,
}
impl Default for NumberOptions { /* decimal, 0/3, grouping */ }

pub fn format_number(value: f64, locale: &str) -> String;
pub fn format_number_with(value: f64, locale: &str, options: NumberOptions) -> String;
pub fn format_integer(value: i64, locale: &str) -> String;
pub fn format_percent(ratio: f64, locale: &str) -> String;   // 0.25 -> "25 %" по ICU
pub fn format_currency(value: f64, locale: &str, currency: &str) -> String;
pub fn format_date(timestamp_secs: i64, locale: &str, style: DateStyle) -> String;
pub fn format_time(timestamp_secs: i64, locale: &str, style: DateStyle) -> String;
pub fn format_date_skeleton(timestamp_secs: i64, locale: &str, skeleton: &str) -> Option<String>;
pub fn format_time_skeleton(timestamp_secs: i64, locale: &str, skeleton: &str) -> Option<String>;
pub fn format_number_named(value: f64, locale: &str, name: &str) -> String;
```

`format_number` — decimal, max 3 fraction digits, grouping; `format_integer` — max 0;
`format_number_named`: `integer` → max 0, `percent` → percent, `currency` → USD,
иначе decimal (сохраняет текущее поведение `[2,"x"]`). UTC; timestamp — Unix seconds.
`format_*_skeleton` принимает `M/d`, `::MMMMd`, `yyyy-MM-dd`, `HH:mm`, `MMMMd`;
`::` отбрасывается; неизвестный токен → `None` (без паники). `format_date/time`
неизвестной/отключённой локали → данные `en-US` (не ошибка).

### 3.4 `markdown.rs`

```rust
pub enum MarkdownNode { /* см. TZ-W25b */ }
pub fn parse_markdown(input: &str) -> MarkdownNode;
pub fn render_markdown(node: &MarkdownNode) -> String;
```

23 правила и точные ожидания — в `TZ-W25b.md` §4. `render_markdown` — нормализованный
Discord-markdown, не HTML; небезопасные URL отбрасываются; HTML не исполняется.

## 4. Тест-план (T-I18N-04…08, T-MSG-01)

| ID | Файл | Что проверяет |
|---|---|---|
| T-I18N-04 | `plural_contract.rs` | все 36 локалей × 21 значение × cardinal/ordinal против `cldr-oracle.json`; `ru` 1/2/5/11/21/22/101; `=N`, offset, `#` — в `i18n_contract.rs` |
| T-I18N-05 | `markdown_contract.rs` | 23 правила таблицей + unsafe URL + обрезанный вход + лимит списка/глубины |
| T-I18N-06 | `format_contract.rs` | числа/даты/время всех 36 локалей против oracle; skeleton; `th` эра; `fr` U+202F; `hi` группировка 3/2 |
| T-I18N-07 | `locale_contract.rs` | 36/31/5, resolve/fallback, все 31 insert, 5 reject, переключение на лету |
| T-I18N-08 | `i18n_contract.rs` | реальные ru/en каталоги, plural-аргументы «N участников», date/time AST, fallback, `$link` control остаётся P3 |

Воркер обязан приложить **red-лог** (targeted test падает по ожидаемой причине) и
**green-лог**. Тесты не подгонять под код: ожидания берутся из oracle/таблицы.

## 5. Команды (сериализовать cargo)

```sh
flock /tmp/opencode/discord-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --test plural_contract
flock /tmp/opencode/discord-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --test format_contract
flock /tmp/opencode/discord-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --test locale_contract
flock /tmp/opencode/discord-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --test markdown_contract
flock /tmp/opencode/discord-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --lib
```

## 6. Мутации (после green, QA-MUT)

Минимум 12: `ru` few 2–4 → 2–5; `pl` many; `ar` zero/two; `fr` million many;
ordinal `en` 11→one; number group separator ru NBSP → space; fr decimal `,` → `.`;
date ru month long → short; `th` year offset 543 → 0; markdown bold `**` → `*`;
unsafe URL `javascript:` allow; list depth 11 → 12; escape `\*` → literal `*` removal.
Каждая мутация — red, restore по md5, лог в `MUTATIONS-B05.md`.

## 7. Интеграция (TL)

1. `lib.rs`: `pub mod locale/plural/format/markdown`, re-export constants/functions;
   убрать дубли; `Ast::Date(String,String)`, `Ast::Time(String,String)`, parse style.
2. `render_ast`: Date/Time/Number/Plural через новые модули; `Value::Int` timestamp;
   `TranslateError::InvalidArgument` для не-числа.
3. `translate`: fallback chain requested → default; missing key — ошибка; реальные
   каталоги не ломать.
4. `tests/i18n_contract.rs`: T-I18N-08; старые 8 lib-тестов сохранить/адаптировать.
5. После полного функционала — один `npm run gate:worker`, review A/B, mutation,
   `waves/db.jsonl`, `STATE.md`, `TASKS.md`, `STATUS.md`, `ROADMAP.md`, `map.md`,
   `CODE-STANDARD.md`.

## 8. P3 backlog (не блокирует)

- `$link` URL control/аргумент (P3-15) — отдельно.
- Полные переводы 31 локали: в offline-кэше только ru/en; продукт поддерживает
  загрузку каталогов, но не выдумывает переводы.
- `Intl.ListFormat`, `RelativeTimeFormat`, `DurationFormat` — P7-04/следующие.
- `<t:...:R>` обновление по таймеру — UI-слой, не i18n.
