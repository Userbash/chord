# Отчёт воркера — RW-29 / W29a (P7-01e Discord-таймстемпы `<t:…>`, headless)

## 1. Границы

- Продукт: `rust/crates/discord-i18n/src/timestamp.rs` (новый),
  `rust/crates/discord-i18n/src/timestamp_data.rs` (генерируется),
  `rust/crates/discord-i18n/src/format.rs` — **только аддитивно**
  (`render_pattern` получил необязательный параметр weekday'ов, добавлена публичная
  `format_date_time_pattern`; поведение `format_date`/`format_time`/скелетонов не менялось),
  `rust/crates/discord-i18n/src/lib.rs` — одна строка `pub mod timestamp;`.
- Тесты: `rust/crates/discord-i18n/tests/timestamp_contract.rs` (14 тестов, t01–t14).
- Оракул/данные: `rust/crates/discord-i18n/tests/data/timestamp-oracle.json`,
  `waves/RW-29/gen-timestamp-data.mjs`, дубликаты в `waves/RW-29/generated/`.
- Артефакты волны: `waves/RW-29/workers/W29a/{red.log,green.log}`,
  `waves/RW-29/run-mutations-w29a.py`, `waves/RW-29/mutations-w29a.log`, этот отчёт.
- Не менялись: `relative.rs`/`plural.rs`/`relative_data.rs`, чужие тесты/модули,
  `Cargo.toml`/`Cargo.lock`, другие крейты, доки (`STATE/STATUS/ROADMAP/TASKS/map`),
  golden (`logs/`, `functions/`, `evidence/` — только чтение), сеть, live Discord,
  `unsafe`, `git commit/push`.
- `cargo` запускался последовательно с `CARGO_TARGET_DIR=/tmp/opencode/target-w29a`;
  workspace/clippy/fmt/npm не запускались (integration-лейн).

## 2. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-i18n/src/timestamp.rs` | `d291d6f50d65629b6e9edfa068d8321b` |
| `rust/crates/discord-i18n/src/timestamp_data.rs` | `c0a9393793a9ca2d85c9877ef0916cdd` |
| `rust/crates/discord-i18n/src/format.rs` | `64d1e3273f575c0262dbda0a7ee31d49` (после `cargo fmt --all`) |
| `rust/crates/discord-i18n/src/lib.rs` | `dee649fed123b4a0e5aaa465c3dc1f34` |
| `rust/crates/discord-i18n/tests/timestamp_contract.rs` | `f09a24473fb92fbfc03d59ddc08d61d4` |
| `rust/crates/discord-i18n/tests/data/timestamp-oracle.json` | `1af293db5e3f06ba22d01c7e25c3e1a5` |
| `waves/RW-29/gen-timestamp-data.mjs` | `11a0d5b5719470acf2b55fb812a11d8f` |
| `waves/RW-29/run-mutations-w29a.py` | `c534be760d7d53ff1b896c314e6a19f7` |
| `waves/RW-29/workers/W29a/red.log` | `fd02ea97dceb4335aa001e261cecdb8a` |
| `waves/RW-29/workers/W29a/green.log` | `a0ca6bd28032093bd1354ec0cbbea52b` |
| `waves/RW-29/mutations-w29a.log` | `c65d7a6686143e4a0652a331811271b9` |

Дубликаты для аудита совпадают побайтово:
`waves/RW-29/generated/timestamp_data.rs` = `c0a939…`, `generated/timestamp-oracle.json` =
`1af293…`.

`lib.rs` — общий файл волны: md5 зафиксирован на момент отчёта (W29b/W29c строку
`pub mod timestamp;` не трогают).

## 3. Сделано

**Генератор `waves/RW-29/gen-timestamp-data.mjs`** (Node 24.21.0, ICU 78.3, offline,
`timeZone: "UTC"`):

- список локалей читается из `locale.rs` (`SUPPORTED_LOCALES` минус `DISABLED_LOCALES`) —
  31 включённая, выключенные `pt-PT/sk/he/ar/id` явно проверяются;
- 6 стилей (`t T d D f F`) × 3 момента (`1623488460`, `-1`, `1893456000`) × 31 локаль;
- паттерны выводятся из `Intl.DateTimeFormat.formatToParts` с **явными компонентами**
  (`hour/minute/second`, `year/month/day`, `weekday`), а не из `dateStyle/timeStyle`:
  ширины полей тогда не зависят от даты и паттерн однозначен;
- отдельно генерируется таблица длинных названий дней недели **в контексте даты**
  (финский эссив `lauantaina`, а не самостоятельный `lauantai`);
- встроенная самопроверка: независимая JS-модель `render_pattern` (токены
  `y M d E H h m s a`, нормализация NBSP/NNBSP) сверяется с ICU на всех **558** строках;
  расхождение — исключение, а не тихая запись. Также проверяется, что в паттернах есть
  NNBSP/NBSP (иначе мутация «нормализация» была бы невалидной);
- пишет `src/timestamp_data.rs` (31 запись: 6 паттернов + 7 weekday'ов, сортировка по коду
  локали для `binary_search`), `tests/data/timestamp-oracle.json` и дубликаты в
  `waves/RW-29/generated/`;
- детерминизм: в выходах нет времени генерации; повторный прогон даёт те же md5
  (проверено), версия Node/ICU зафиксирована в заголовке и в `source` оракула.

**Оракул `timestamp-oracle.json`:**

- `cases` — 31 × 6 × 3 = **558** строк `{locale, style, timestamp, expected}`;
- `relative` — `Intl.RelativeTimeFormat(numeric: "auto", style: "long")` для en-US/ru/de/ja,
  дельты ±30 с, ±5 мин, ±3 ч, ±2 сут, ±3 нед, ±5 мес, ±2 года и 0 (всего 60; unit/value
  выбраны ладдером ТЗ, строка — из ICU);
- `ladder` — 24 границы ладдера для en-US (проверка выбора единицы и усечения к нулю);
- `extreme` — `i64::MIN`/`i64::MAX` пары (сатурация вычитания, ICU-строка для относительного
  стиля);
- нормализация: U+202F/U+00A0 → U+0020 и в паттернах, и в оракуле (как `render_pattern`).

**`src/timestamp.rs`:**

- `TimestampStyle` (7 вариантов) и `timestamp_style` — ровно один символ из `tTdDfFR`,
  иначе `None` (`""`, `"tt"`, `"r"`, `"R "`, не-ASCII → `None`);
- `render_timestamp(timestamp_secs, style, locale, now_secs)`: локаль —
  `canonical_locale` + `is_enabled`, иначе `DEFAULT_LOCALE`; календарные стили рендерятся
  через `format_date_time_pattern` (UTC, `year_offset` для тайского календаря);
- `R`: `delta = timestamp_secs.saturating_sub(now_secs)`, ладдер по `|delta|`
  (365/30/7 сут, час, минута, иначе секунда), количество = `delta / размер` (усечение к нулю),
  `format_relative(.., Long, Auto)` — ноль даёт `now`/`сейчас`, ±1/±2 сут — ICU special
  forms (`tomorrow`, `послезавтра`);
- `render_timestamp_tokens`: байтовый скан `<t:`, токен = `<t:` + опц. `-` + цифры (ровно
  `i64`, `checked_mul/add`, `-2^63` поддержан) + опц. `:` + ровно одна буква + `>`;
  пустой стиль = `ShortDateTime`; невалидный токен копируется как есть, вложенность не
  поддерживается; текст без токенов возвращается байт-в-байт; произвольный Unicode безопасен
  (срезы только по ASCII-границам);
- без паник/`unwrap`/`unsafe` в продакшн-пути: все `unwrap`-подобные места —
  `unwrap_or`/`unwrap_or_default`/`unwrap_or_else` в тестах.

`timestamp_data.rs` подключён как `#[path = "timestamp_data.rs"] mod timestamp_data;`
внутри `timestamp.rs`: `lib.rs` по ТЗ получает только `pub mod timestamp;`, второй строки
`mod timestamp_data;` там быть не может.

**`src/format.rs` (аддитивно):** `render_pattern` принимает
`weekdays_long: Option<&[&'static str; 7]>` (все прежние вызовы передают `None` — поведение
идентично), добавлена `pub fn format_date_time_pattern(timestamp_secs, locale, pattern,
weekdays_long)`.

## 4. Red / green

Команда (из `rust/`):

```sh
CARGO_TARGET_DIR=/tmp/opencode/target-w29a cargo test --offline -p discord-i18n --test timestamp_contract
```

- **Red** (`workers/W29a/red.log`, `# EXIT=101`):
  `error[E0432]: unresolved import discord_i18n::timestamp` — модуль ещё не реализован.
- **Green** (`workers/W29a/green.log`, `# EXIT=0`): `14 passed; 0 failed`.

Тесты (t01–t14):

| ID | Что проверяет |
|---|---|
| t01 | 7 кодов стилей, неизвестные/многосимвольные → `None` |
| t02 | en-US `t/T/f/F` на 2021-06-12T09:01:00Z (ICU) |
| t03 | en-US `d` = `06/12/2021`, `D` = `June 12, 2021` |
| t04 | ru `t/T/d/D/f/F` (включая `12 июня 2021 г.`) |
| t05 | R en/ru: +2 сут, −5 мин, 0 (`in 2 days`, `5 minutes ago`, `now`, `послезавтра`, `5 минут назад`, `сейчас`) |
| t06 | ладдер: 0/59/60/3599/3600/7199/86399/1 сут/2 сут/7 сут−1/7 сут/3 нед/30 сут−1/30 сут/5 мес/365 сут−1/365 сут/2 года и отрицательные |
| t07 | несколько токенов + текст, пустой стиль = `f`, текст без токенов байт-в-байт (NBSP/NNBSP/Unicode) |
| t08 | невалидные токены без изменений (17 вариантов) + смешанный текст |
| t09 | `xx`/`he`/`ar`/`id`/`pt-PT`/`sk`/`""` → en-US |
| t10 | `i64::MIN/MAX` в дате и в `now`: без паники, детерминированно, экстремумы из оракула |
| t11 | матрица оракула 558 строк — точное совпадение |
| t12 | инварианты данных: 558 строк, 31×6=186 паттернов, уникальность, нет `{}`, нет NBSP/NNBSP |
| t13 | все 31 включённые локали рендерят непусто все 7 стилей; 5 выключенных = en-US |
| t14 | R-оракул 60 кейсов + 24 границы ладдера |

Дополнительно (targeted, без workspace-гейта) прогнаны соседние контракты крейта после
правки `format.rs`: `format_contract` — 3 passed, `relative_contract` — 12 passed.

## 5. Мутации

`python3 waves/RW-29/run-mutations-w29a.py` → `waves/RW-29/mutations-w29a.log`:
8 мутаций, **выжило 0**, продукт восстановлен по md5 (проверяется скриптом).

| ID | Мутация | Убита тестами |
|---|---|---|
| m01 | стиль `t` → паттерн `T` | t02, t04, t07, t08, t11 |
| m02 | ладдер: неделя 7 → 8 сут | t06, t14 |
| m03 | дефолт пустого стиля `f` → `d` | t07 |
| m04 | парсер: потерян `-` | t10 |
| m05 | локаль-фолбэк без `is_enabled` | t09, t13 |
| m06 | отключена нормализация NBSP/NNBSP | t02, t04, t07, t08, t10, t13 |
| m07 | `Auto` → `Always` в R | t05, t06, t14 |
| m08 | ладдер: месяц 30 → 31 сут | t06, t14 |

## 6. Неоднозначности и риски

1. **`f` в ТЗ против клиента Discord.** ТЗ §1 фиксирует `f` = «короткая дата + короткое
   время», `F` = «длинная дата с weekday + короткое время». В клиенте Discord `f`
   исторически показывает длинную дату (`June 12, 2021 9:01 AM`). Выбрана формулировка ТЗ:
   `f` = паттерн `d` + время `t` (`06/12/2021, 9:01 AM` в en-US), `F` = ICU combined
   `{weekday long, year, month long, day, hour, minute}`. Это зафиксировано оракулом;
   wire-совместимость с клиентом не заявляется.
2. **`t`/`T` собраны из явных компонентов, а не `timeStyle`.** Для en-GB это даёт `9:01`
   вместо `09:01`. Причина: `timeStyle: "short"` у части локалей (zh-TW) использует
   «гибкие» day periods (凌晨/上午/中午/下午/晚上), а `format_data.rs` хранит только две формы
   `a`; явные компоненты дают стабильные `AM/PM`-формы, воспроизводимые существующим
   `render_pattern`. Все 558 строк сверены с ICU именно в этой постановке.
3. **`R` и `Auto`.** ТЗ §3 t05 ожидает ru `+2 сут` = `через 2 дня`, но §1 требует
   `format_relative(.., Auto)`, а ICU Auto даёт `послезавтра`. Приоритет у §1 и оракула
   (t05, t14); противоречие зафиксировано комментарием в тесте.
4. **UTC-рендер.** Все стили рендерятся в UTC (как `format_date`/`format_time`). Клиент
   Discord показывает локальную зону пользователя; перенос на локальную зону — отдельная
   задача, в этом контракте не заявлен.
5. **Нормализация пробелов.** Оракул и рендер нормализуют U+202F/U+00A0 в обычный пробел.
   Паттерны в `timestamp_data.rs` намеренно хранят «сырые» NNBSP из ICU (ru/uk — перед
   `г.`), поэтому нормализация в `format.rs` реально нагружена (m06 её убивает).
6. **Финский weekday.** В контексте даты ICU использует эссив (`lauantaina`); таблица
   `weekdays_long` в `timestamp_data.rs` сгенерирована именно из контекста даты, а не из
   самостоятельной формы (`format_data.rs` хранит `lauantai`).
7. **Экстремальные даты вне диапазона `Date`.** Для `i64::MIN/MAX` в календарных стилях
   ICU-оракула нет (JS `Date` не представляет такие годы); тест t10 проверяет отсутствие
   паники и детерминизм, а для относительного стиля — точную строку из оракула.
8. **Воспроизводимость.** Данные зависят от версии ICU: побайтовое воспроизведение
   гарантируется на Node 24.21.0 / ICU 78.3 (зафиксировано в заголовке `timestamp_data.rs`
   и в `source` оракула). Временных меток в выходах нет, повторный прогон md5 не меняет.
9. **`#[path]` для `timestamp_data`.** Ограничение ТЗ «в `lib.rs` только одна строка»
   вынуждает подключать сгенерированный модуль из `timestamp.rs`; это отмечено
   комментарием в коде.
10. **Форматирование крейта до integration-лейна.** Мои файлы (`timestamp.rs`,
    `timestamp_data.rs`, `timestamp_contract.rs` и добавленный блок в `format.rs`)
    проходят `rustfmt --check --edition 2024` чисто. `cargo fmt --all -- --check`
    в текущем дереве показывает три **чужих/доволновых** диффа: `src/format.rs:159`
    (`format_integer`, строка существует с RW-24 и мной не менялась), `src/relative.rs`
    и `tests/relative_contract.rs` (не мои файлы). Я их не переформатировал — это решает
    TL на integration-лейне.

## 7. Команды проверки

```sh
node waves/RW-29/gen-timestamp-data.mjs        # 2× подряд → одинаковые md5
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29a \
  cargo test --offline -p discord-i18n --test timestamp_contract   # 14 passed
python3 waves/RW-29/run-mutations-w29a.py      # 8/8 killed, restore ok
```
