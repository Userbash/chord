# RW-24 / B05 — review resolution (P7-01)

Дата: 2026-10-21. Ревьюеры: REV-A (locale/plural/format/lib), REV-B (markdown/lib),
оба read-only; авторы кода свой код не ревьюили. Итог после разбора: **P0=0, P1=0, P2=0**,
P3 — backlog.

## 1. Закрыто red-first

| ID | Находка | Проверка | Исправление |
|---|---|---|---|
| A-P1 | `=N` проверялся после `offset` | `red-review-A-i18n.log` → `exact_selector_offset...` fail (`one` вместо `exact3`) | `lib.rs`: exact строится из исходного `v`, offset — только для CLDR-категории; `green-review-A-i18n.log` 6/6 |
| A-P2 | неизвестная валюта форматировалась с группировкой | `red-review-A-format.log` → `1234.50 XXX` fail (`1,234.50 XXX`) | `format.rs`: неизвестная валюта → `{value:.2} {currency}`; `green-review-A-format.log` 3/3 |
| A-P2 | Number AST принимал строку | `red-review-A-i18n.log` → `number_nodes_reject...` fail | `lib.rs`: `Value::Str` → `TranslateError::InvalidArgument`; `green-review-A-i18n.log` 6/6 |
| B-P1 | ordered-list overflow `u64::MAX + 1` | `red-review-B2.log` → panic в `markdown.rs:121` | `markdown.rs`: `saturating_add`; `green-review-B2.log` 11/11 |
| B-P2 | вложенные списки схлопывались | `red-review-B2.log` → `- outer\n- inner` | рекурсивный `parse_list` + indent continuation в renderer; 12/12 green |
| B-P2 | timestamp принимал хвост стиля | `red-review-B2.log` → `<t:...:Rbad>` → `<t:...:R>` | точная проверка ровно одного символа из `tTdDfFsSR`; 12/12 green |
| B-P2 | slash-команда не проверяла charset/length | `red-review-B2.log` | `valid_slash_name`: первый токен 1..=32, `[-_Letter_Number]`; пробелы после команды разрешены контрактом `</cmd arg:123>`; 12/12 green |

Дополнительно усилен oracle: 56 значений plural на локаль (включая дроби, миллионы,
11–14, 1000/10000), второй timestamp (январь, 09:07) для дат/времени. Это поймало
`cs` short-time `HH` вместо `H`; генератор теперь строит time pattern по второму sample.
После усиления — все 36 локалей × 2 timestamp × 4 стиля зелёные.

## 2. Отклонено как не-баг (с доказательством)

| ID | Находка | Решение |
|---|---|---|
| A-P2 | `he` cardinal `many` отсутствует | **Не баг для целевого ICU.** Node 24 / ICU 78.3 / CLDR 48: `new Intl.PluralRules("he").resolvedOptions().pluralCategories` = `one,two,other`; `select(10)=other`, `select(20)=other`. Оракул `cldr-oracle.json` содержит эти значения. Правило `many` в `TZ-W25a.md` устарело (CLDR до 42); `he` отключён в клиенте. Код следует оракулу и реальному `Intl.PluralRules`, а не устаревшему тексту ТЗ. |
| A-P2 | замена NBSP/U+202F на обычный пробел в датах/времени | **Не баг.** `Intl.DateTimeFormat.format()` (именно его использует Discord) нормализует U+00A0/U+202F в U+0020; `formatToParts()` сохраняет их в literal. Оракул сгенерирован через `format()`, все 36×2×4 сравнения проходят. Замена нужна для паритета с `format()`, иначе `en-US` short даёт `2:07 PM` вместо `2:07 PM`. |
| B-P2 | `</bad name:123>` должен быть literal | **Не баг по контракту:** таблица TZ-W25b §4 явно требует `</cmd arg:123>` → тот же текст, то есть пробелы после первого токена допустимы. Проверяются charset/длина именно имени команды; `</bad{name:123>` и 33-символьное имя теперь literal. |

## 3. P3 backlog (не блокирует P7-01)

- `$link` URL/аргумент rich-тега — подтверждённого URL в RE нет (P3-15 backlog).
- Полные переводы 31 локали, `Intl.ListFormat`/`RelativeTimeFormat`/`DurationFormat`,
  `<t:...:R>` UI-таймеры — вне P7-01.
- `format_percent` для локалей с пробелом перед `%` (ru/fr/de) не использует пробел;
  в P7-01 контракт проверяет `en-US`/`ru` number, currency и даты. Это P3.
- Slash-команда с пробелами в аргументах: принята по контракту; более строгая грамматика
  аргументов — P3.
- `he`-правило в `TZ-W25a.md` следует исправить при следующей ревизии ТЗ (код уже
  соответствует ICU 78).

## 4. Артефакты

- `review-A.md`, `review-B.md`, `codex-review-A.log`, `codex-review-B.log`.
- Red/green: `red-review-A-*.log`, `green-review-A-*.log`, `red-review-B2.log`,
  `green-review-B2.log`, `green-review-B05.log`.
- `MUTATION-REPORT-B05.md`, `mutations/results.json`, `mutations/mutation-*.log`.
