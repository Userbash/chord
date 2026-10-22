# TZ W29a — P7-01e Discord-таймстемпы `<t:…>` (headless)

Волна: RW-29. Дата: 2026-10-22. Крейт: `discord-i18n`. Сеть/live не используются.

## 1. Замороженный контракт

```rust
pub enum TimestampStyle { ShortTime, LongTime, ShortDate, LongDate, ShortDateTime, LongDateTime, Relative }

/// Стиль по букве Discord (`t T d D f F R`); None — неизвестный/отсутствующий.
pub fn timestamp_style(code: &str) -> Option<TimestampStyle>;

/// Рендер одного таймстемпа; `now_secs` — «сейчас» для относительного стиля (UTC).
pub fn render_timestamp(timestamp_secs: i64, style: TimestampStyle, locale: &str, now_secs: i64) -> String;

/// Замена всех валидных `<t:…>` в тексте; невалидные токены остаются как есть.
pub fn render_timestamp_tokens(input: &str, locale: &str, now_secs: i64) -> String;
```

Инварианты:

- Синтаксис токена: `<t:` + опциональный `-` + цифры (unix-секунды `i64`) + опционально `:` +
  ровно одна буква из `tTdDfFR` + `>`. Пустой стиль = `ShortDateTime` (Discord `f`).
  Лишние пробелы, вторая буква, мусор, переполнение `i64` → токен не заменяется.
- Стили (рендер в UTC, детерминированно; форматирование через существующие
  `format_date`/`format_time`/`format_date_skeleton`/`format_time_skeleton`, при нехватке —
  собственные CLDR-паттерны в `timestamp_data.rs`):
  - `t` — короткое время (`9:01 AM`), `T` — время с секундами (`9:01:00 AM`);
  - `d` — короткая дата (`06/12/2021`), `D` — длинная (`June 12, 2021`);
  - `f` — короткая дата + короткое время, `F` — длинная дата с weekday + короткое время;
  - `R` — относительное через `relative::format_relative(.., Auto)`.
- `R`: ладдер единиц по `|delta|` (delta = timestamp − now): ≥365 сут → `Year` (delta/365 сут),
  ≥30 сут → `Month` (delta/30 сут), ≥7 сут → `Week` (delta/7 сут), ≥1 сут → `Day`,
  ≥3600 с → `Hour`, ≥60 с → `Minute`, иначе `Second`. Деление целочисленное, округление к нулю;
  ноль → `Auto` («now»/`сейчас`).
- Локаль: `canonical_locale` + `is_enabled`, иначе `DEFAULT_LOCALE` (en-US).
- Время рендерится в UTC (как `format_date`/`format_time`); NBSP/NNBSP (`\u{a0}`, `\u{202f}`)
  нормализуются в обычный пробел — так же, как `render_pattern` в `format.rs`. Оракул
  генерируется с той же нормализацией, иначе сравнение байтовое.
- Разрешено аддитивно расширить `discord-i18n/src/format.rs` (новые публичные функции для
  комбинированных date+time паттернов); существующее поведение `format_date`/`format_time`/
  `format_date_skeleton`/`format_time_skeleton` не меняется.
- Текст без токенов возвращается байт-в-байт; токены обрабатываются слева направо, вложенность
  не поддерживается; `render_timestamp_tokens` не паникует на произвольном Unicode.
- `i64::MIN`/`i64::MAX` в токене и в `now_secs` не паникуют (насыщение при вычитании).

## 2. Данные

`src/timestamp_data.rs` и `tests/data/timestamp-oracle.json` генерируются
`waves/RW-29/gen-timestamp-data.mjs` (Node 24 ICU, `timeZone: "UTC"`):

- 31 включённая локаль × 6 стилей (t/T/d/D/f/F) × 3 момента (2021-06-12T09:01:00Z,
  1969-12-31T23:59:59Z, 2030-01-01T00:00:00Z);
- `R`-оракул: `Intl.RelativeTimeFormat` для дельт из ладдера (±30 с, ±5 мин, ±3 ч, ±2 сут,
  ±3 нед, ±5 мес, ±2 года) на en/ru/de/ja — для сверки выбора единицы и формы.

Ручная правка `*_data.rs` запрещена; таблица должна воспроизводиться генератором побайтово.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| t01 | `timestamp_style` для `t T d D f F R` | все 7, неизвестные → None |
| t02 | en t/T/f/F на 2021-06-12T09:01:00Z | по ICU (UTC) |
| t03 | en d/D | `06/12/2021`, `June 12, 2021` |
| t04 | ru t/T/d/D/f/F | по ICU |
| t05 | R en/ru: +2 сут, −5 мин, 0 | `in 2 days`/`через 2 дня`, `5 minutes ago`/`5 минут назад`, `now`/`сейчас` |
| t06 | R ладдер: 365 сут, 30 сут, 7 сут, 3600 с, 60 с, 59 с | Year/Month/Week/Hour/Minute/Second |
| t07 | `render_timestamp_tokens`: несколько токенов + текст | заменены только валидные |
| t08 | невалидные: `<t:abc>`, `<t:1:q>`, `<t:1:RR>`, `<t:>`, `<t:1` | как есть |
| t09 | локаль `xx`/`he` (выключена) | en-US |
| t10 | `i64::MIN`/`i64::MAX` | без паники, детерминированно |
| t11 | матрица оракула t/T/d/D/f/F | точное совпадение |
| t12 | инварианты данных: 31×6×3 = 558 строк, нет `{`-плейсхолдеров | 558 |

Бюджет: 12–15 тестов; мутации 6–8.

## 4. Команды

```sh
node waves/RW-29/gen-timestamp-data.mjs
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29a cargo test --offline -p discord-i18n --test timestamp_contract
```

Полный gate — один раз в integration-лейне (см. BATCH-TZ §4).

## 5. DoD

red-лог с exit code → green → мутации (продукт восстановлен по md5) → отчёт в
`waves/RW-29/REPORT-W29a.md` → ревью → accepted.
