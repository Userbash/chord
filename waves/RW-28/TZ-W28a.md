# TZ W28a — P7-01c RelativeTimeFormat (headless)

Волна: RW-28. Дата: 2026-10-22. Карточка: P7-01c (хвост P7-01, отдельного ID в PLAN нет).
Крейт: `discord-i18n`. Зависимостей нет. Сеть/live не используются.

## 1. Замороженный контракт

```rust
pub enum RelativeUnit { Second, Minute, Hour, Day, Week, Month, Quarter, Year }
pub enum RelativeStyle { Long, Short, Narrow }
pub enum RelativeNumeric { Always, Auto }

pub fn format_relative(
    locale: &str,
    value: i64,
    unit: RelativeUnit,
    style: RelativeStyle,
    numeric: RelativeNumeric,
) -> String;
```

- Резолв локали: `canonical_locale` + `is_enabled`, иначе `DEFAULT_LOCALE` (en-US).
- Направление: `value >= 0` → future, `value < 0` → past; ноль в `Always` — future
  (`in 0 seconds`), в `Auto` — специальная форма «now».
- Плюральная категория выбирается по `|value|` через `plural::plural_category_integer(.., Cardinal)`;
  при отсутствии категории в строке — фолбэк `other`.
- `Auto` использует специальные формы ICU, включая локальные −2/+2 (например, рус. «позавчера»,
  нем. «vorgestern»); при их отсутствии в данных — обычные паттерны `Always`.
  *(Правка TL-1 2026-10-22 после ревью RW-28: прежняя формулировка «только −1/0/1» расходилась
  с ICU-оракулом; поведение приведено к ICU, см. `waves/RW-28/DECISIONS.md`.)*
- Отсутствующая строка таблицы → строка en-US; полное отсутствие → `"{n} {unit}"`
  (детерминированно, без паники).
- `i64::MIN` не паникует (`unsigned_abs`).
- Число подставляется точной целочисленной группировкой `format::format_integer_unsigned`
  (без f64-round-trip; для значений > 2^53 цифры и категория сохраняются).
- Выход не содержит `{0}` и `{{`/`}}` после подстановки.

## 2. Данные

`src/relative_data.rs` генерируется `waves/RW-28/gen-relative-data.mjs` из Node 24 ICU
(`Intl.RelativeTimeFormat`, `Intl.PluralRules`): 31 включённая локаль × 8 единиц ×
3 стиля. Ручная правка запрещена. Спецформы `Auto` — из `formatToParts` без integer-части.

Оракул: `tests/data/relative-oracle.json` (генерируется тем же скриптом независимо от
Rust-таблицы): матрица значений для всех локалей + расширенная выборка en/ru.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| t01 | en long always, ±2 day, 1000 day | `2 days ago`, `in 2 days`, `1,000 days ago` |
| t02 | ru long always, -5/-2/-1/1/2/5 day | `5 дней назад`, `2 дня назад`, `1 день назад`, `через 1 день`, `через 2 дня`, `через 5 дней` |
| t03 | de/es/ja/zh базовые | по ICU |
| t04 | auto −1/0/1 и локальные ±2 day (en/ru/de/ja); фолбэк `Always` для единиц без ±2 | yesterday/today/tomorrow, вчера/сегодня/завтра, позавчера/послезавтра, vorgestern, 明後日 |
| t05 | auto при 2 day | как always |
| t06 | short/narrow (en/ru, hour/minute) | по ICU |
| t07 | все 8 единиц en | по ICU |
| t08 | неизвестная/выключенная локаль (`xx`, `he`) | en-US |
| t09 | оракул-матрица | точное совпадение |
| t10 | `i64::MIN`, 0 past/future | без паники, `0 seconds ago` / `in 0 seconds` |
| t11 | инварианты данных | нет `{0}`/`{{` в шаблонах, ровно 744 строки |

Бюджет: 11–14 тестов; мутации 6–8.

## 4. Команды

```sh
node waves/RW-28/gen-relative-data.mjs
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w28 cargo test --offline -p discord-i18n --test relative_contract
cargo fmt --all -- --check && cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline --workspace && cd .. && npm run gate:worker
```

## 5. DoD

red-лог с exit code → green → мутации (продукт восстановлен по md5) → batch gate →
независимое ревью (P0/P1/P2=0) → отчёт `waves/RW-28/REPORT.md` → accepted в `db.jsonl`,
`STATE.md` и доки один раз.
