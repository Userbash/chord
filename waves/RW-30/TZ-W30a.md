# TZ W30a — P7-01d `Intl.DurationFormat` (headless)

Волна: RW-30. Дата: 2026-10-22. Крейт: `discord-i18n`. Сеть/live не используются.

## 1. Замороженный контракт

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Duration {
    pub years: u64,
    pub months: u64,
    pub weeks: u64,
    pub days: u64,
    pub hours: u64,
    pub minutes: u64,
    pub seconds: u64,
    pub milliseconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationStyle { Long, Short, Narrow }

pub fn format_duration(locale: &str, duration: &Duration, style: DurationStyle) -> String;
```

Инварианты (проверены на Node 24.21 / ICU 78):

- Порядок единиц фиксирован: years → months → weeks → days → hours → minutes → seconds →
  milliseconds; нулевые единицы **опускаются**; все нули → пустая строка `""`.
- Значение каждой единицы форматируется с группировкой ICU (`1,000 days`, `1 000 дней`).
- Плюральная форма — `plural::plural_category_integer(locale, value, Cardinal)`; при
  отсутствии категории в данных — `other`.
- Разделители берутся из данных локали/стиля: `join2` (ровно два элемента), `join_middle`
  (между первыми элементами при ≥3), `join_last` (перед последним при ≥3). Например,
  `de long`: middle `", "`, last `" und "`; `en-US narrow`: оба `" "`; `ru long`: оба `" "`.
- Локаль: `canonical_locale` + `is_enabled`, иначе `DEFAULT_LOCALE` (en-US); отсутствующая
  строка → en-US, полное отсутствие → детерминированная сборка `"{n} {unit}"`.
- `u64::MAX`, пустой `Duration`, одна единица — без паник; `unsafe` нет.

## 2. Данные и оракул

`src/duration_data.rs` и `tests/data/duration-oracle.json` генерируются
`waves/RW-30/gen-duration-data.mjs` (Node 24, `timeZone` не нужен):

- паттерны: 31 включённая локаль × 3 стиля × 8 единиц × значения `[1, 2, 5, 11, 21, 101]`
  (покрытие CLDR-категорий; проверка — `Intl.PluralRules` в генераторе);
- разделители `join2/join_middle/join_last`: вывод `Intl.DurationFormat` на парах и тройках
  единиц (генератор сам извлекает и **проверяет гипотезу регулярной склейки** на 200 случайных
  наборах единиц и значений на локаль/стиль; при расхождении — падение генератора);
- оракул: 2000+ случайных длительностей (детерминированный seed) + краевые (одна единица,
  все нули, `u64::MAX`, 1000/21/101) на все 31 локаль и 3 стиля, плюс нормализация
  NBSP/NNBSP → пробел (как в `render_pattern`).

Ручная правка `*_data.rs` запрещена; повторный запуск генератора даёт те же md5.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| d01 | en long 1h2m3s | `1 hour, 2 minutes, 3 seconds` |
| d02 | ru long 1 час + 2 минуты + 3 секунды | по ICU |
| d03 | de long 3 единицы | `…, … und …` |
| d04 | en narrow 5m9s | `5m 9s` |
| d05 | нулевые единицы опускаются; все нули → `""` | по ICU |
| d06 | группировка 1000 days (en/ru/de) | `1,000` / `1 000` / `1.000` |
| d07 | плюральные категории (1/2/5/21/101) | по ICU |
| d08 | все 8 единиц в правильном порядке | по ICU |
| d09 | неизвестная/выключенная локаль (`xx`, `he`) | en-US |
| d10 | `u64::MAX`, пустая Duration | без паники |
| d11 | полный оракул (2000+ кейсов) | точное совпадение |
| d12 | инварианты данных: 31×3×8×6 паттернов, разделители непустые | счётчики |
| d13 | `format_duration` идемпотентен/детерминирован | 2 вызова равны |

Бюджет: 12–15 тестов; мутации 6–8 (порядок единиц, join2/join_middle/join_last, пропуск нуля,
плюральная категория, локаль-фолбэк, группировка).

## 4. Команды

```sh
node waves/RW-30/gen-duration-data.mjs
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w30a cargo test --offline -p discord-i18n --test duration_contract
```

## 5. DoD

red-лог с exit code → green → мутации (restore по md5) → отчёт `waves/RW-30/REPORT-W30a.md`.
