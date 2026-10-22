# Отчёт W30a — P7-01d `Intl.DurationFormat` (RW-30)

Batch: RW-30, карточка P7-01d, seat CLIENT-1 / W30a. Дата: 2026-10-22.
Крейт: `discord-i18n`. Сеть/live/commit не использовались, `unsafe` нет.

## 1. Сделано

- Генератор `waves/RW-30/gen-duration-data.mjs` (Node v24.21.0 / ICU): 31 включённая
  локаль × 3 стиля × 8 единиц × значения `[1,2,5,11,21,101]`; шаблоны единиц сняты
  через `Intl.DurationFormat.formatToParts`, категории — `Intl.PluralRules`, группировка —
  `Intl.NumberFormat` (BigInt для `u64::MAX`).
- Разделители `join2` / `join_middle` / `join_last` извлечены из пар и троек единиц;
  гипотеза регулярной склейки проверена на 200 случайных длительностях (1–8 единиц,
  значения 1..10^6) на каждую локаль/стиль: **18 600 проверок, расхождений нет**
  (при расхождении генератор падает).
- Дополнительно к значениям ТЗ добавлены сэмплы категории `many`, недостижимой
  значениями `[1,2,5,11,21,101]`: `fr`, `it`, `pt-BR`, `es-419`, `es-ES` → 1 000 000.
  Для `pl` (и подобных) `other` для целых недостижим — слот заполнен формой `many`;
  lookup её не выбирает. В таблице всегда 6 CLDR-категорий на строку → 744 × 6 = 4464.
- Генератор идемпотентен: два прогона дают те же md5 (проверено).
- Реализация `duration.rs`: порядок years → milliseconds, пропуск нулей, `""` на все
  нули, `plural_category_integer` (точные цифры для значений > 2^53), разделители из
  данных, группировка `format_integer_unsigned`, фолбэк `canonical+enabled → en-US row →
  "{n} {unit}"`, нормализация NNBSP/NBSP → пробел. Без паник/`unwrap`/`unsafe`.
- 13 контрактных тестов d01–d13 + полный оракул 4092 кейса (2232 случайных + 1860 краевых:
  все нули, `u64::MAX`, одиночные единицы 1/1000, 21/101).
- Мутации: 8/8 убиты, исходник восстановлен по md5 (baseline = restored).

## 2. Прогоны

```sh
node waves/RW-30/gen-duration-data.mjs
# rows=744 joins=93 patterns=4464 cases=4092 random=2232 joinChecks=18600 locales=31 node=v24.21.0

cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w30a \
  cargo test --offline -p discord-i18n --test duration_contract
```

- **red**: `waves/RW-30/workers/W30a/red.log`, **EXIT=101**, `1 passed; 12 failed`
  (стаб возвращал `""`; d13 детерминизма на стабе проходил тривиально).
- **green**: `waves/RW-30/workers/W30a/green.log`, **EXIT=0**, `13 passed; 0 failed`.
- **targeted**: только `--test duration_contract`. Полный gate / clippy / fmt / npm не
  запускались — по правилу batch это integration-лейн.

Тесты: d01 en long 1h2m3s; d02 ru long; d03 de long (`und`); d04 en narrow `5m 9s`;
d05 пропуск нулей + все нули; d06 группировка 1000 (en/ru/de) + отсутствие NBSP/NNBSP;
d07 категории ru/pl/en + fr `many` на 10^6; d08 все 8 единиц в порядке; d09 неизвестная/
выключенная локаль → en-US (+ канонизация `RU`); d10 `u64::MAX`/пустая Duration без паник;
d11 весь оракул; d12 инварианты таблицы; d13 детерминизм.

## 3. Мутации (`waves/RW-30/run-mutations-w30a.py` → `mutations-w30a.log`)

| ID | Мутация | Exit | Вердикт |
|---|---|---|---|
| m01 | порядок единиц hours ↔ minutes | 101 | KILLED |
| m02 | `join2` → `join_middle` | 101 | KILLED (ловит d12) |
| m03 | `join_middle` → `join_last` | 101 | KILLED |
| m04 | нулевые единицы не опускаются | 101 | KILLED |
| m05 | категория всегда `other` | 101 | KILLED |
| m06 | фолбэк локали в `ru` вместо en-US | 101 | KILLED |
| m07 | группировка отключена | 101 | KILLED |
| m08 | нормализация пробелов удалена | 101 | KILLED |

Восстановление: `md5(duration.rs)` до и после — `376dbd907d673eb6d796ce41185588fe`.

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-i18n/src/duration.rs` | `376dbd907d673eb6d796ce41185588fe` |
| `rust/crates/discord-i18n/src/duration_data.rs` (генерат) | `fa31488bd628008721c76ac2c10d9a5f` |
| `rust/crates/discord-i18n/src/lib.rs` (+2 строки модулей) | `64ca6fa9c4b127e91ef58b3dc0cd49dd` |
| `rust/crates/discord-i18n/tests/duration_contract.rs` | `b7488fcb79dbcf281ba8c68f1066af4a` |
| `rust/crates/discord-i18n/tests/data/duration-oracle.json` | `928859fe73f5e8438753ceba49dc0028` |
| `waves/RW-30/gen-duration-data.mjs` | `731d01dd65c15253e3d220ff5ecdb5e1` |
| `waves/RW-30/run-mutations-w30a.py` | `c719b423049dc7fa2efb7a9511b319c6` |
| `waves/RW-30/generated/duration_data.rs` | `fa31488bd628008721c76ac2c10d9a5f` |
| `waves/RW-30/generated/duration-oracle.json` | `928859fe73f5e8438753ceba49dc0028` |
| `waves/RW-30/workers/W30a/red.log` | `7ecf552a6f0c58947068cdf1dd3971db` |
| `waves/RW-30/workers/W30a/green.log` | `f066dd7c5e0cbed1d96531d94127b01e` |
| `waves/RW-30/mutations-w30a.log` | `29110d8ace4147bdab271c50bcabf923` |

`lib.rs`: добавлены ровно `pub mod duration;` и `mod duration_data;` (сгенерированный
соседний модуль обязан быть объявлен; в крейте так же объявлены `format_data`/`list_data`/
`relative_data`). Остальные строки не переформатированы.

## 5. Риски и принятые решения

1. **ICU-паритет.** Оракул снят с Node v24.21.0 (полный ICU 78). `Intl.DurationFormat`
   в этой сборке принимает `display: "auto"` (нули опускаются) — генератор фиксирует это
   явно. При смене ICU данные нужно перегенерировать; тесты сверяют контракт с
   замороженным артефактом, а не с рантаймом.
2. **Группировка 4 цифр.** `Intl.DurationFormat` для `bg/es-ES/hu/it/pl` не группирует
   4-значные числа (`1000 дней`), а `Intl.NumberFormat` группирует. `format_integer_unsigned`
   уже содержит этот carve-out; генератор сверяет группировку ICU с `NumberFormat` на
   10^4…10^8 (включая индийскую схему `hi` 3/2) и падает при расхождении.
3. **Нормализация пробелов.** По ТЗ `\u{202f}`/`\u{a0}` → обычный пробел: `format_duration`
   нормализует весь вывод. Это отличается от `format_relative`/`format_date`, которые
   сохраняют NBSP из `format_integer_unsigned`; при выводе рядом длительности и
   relative-времени пробелы могут выглядеть по-разному. Wire-совместимость с клиентом
   этой карточкой не заявляется (headless-форматирование, как и P7-01b/c).
4. **Диапазон оракула.** Temporal-диапазон `Intl.DurationFormat` ограничен ~2^53 секунд,
   поэтому случайные значения — 1..10^6 (группировка/категории покрыты), а `u64::MAX`
   собран синтетически: ICU-шаблон + точная группировка BigInt + категория по точному
   остатку `u64::MAX mod 10^6` (правила CLDR для целых зависят от mod 10/100/1000/10^6;
   сверено с `PluralRules` на f64-представлении, где категория совпадает).
5. **Локаль.** Как в `relative.rs`, принимаются только точные канонические коды:
   `de-DE` → en-US (языкового фолбэка `de` нет). Это минимальная детерминированная
   семантика; если понадобится language-fallback, это отдельное изменение контракта.
6. **`other` для целых.** В `pl` (и ряде славянских) `other` недостижим для целых
   значений; слот заполнен другой формой, но lookup выбирает только реальные категории.
   Дробных значений в `Duration` (все поля `u64`) нет, так что это не наблюдаемо.
7. **Не запускалось:** workspace/clippy/fmt/npm, live Discord, сеть. Targeted-тесты —
   только `duration_contract`; полный gate — в integration-лейне.

## 6. После независимого ревью (P3)

Ревью: **принято**, P0/P1/P2 = 0. Закрыто/учтено:

- **P3-1** md5 в §4 и мутационном логе приведены к байтам после `cargo fmt` (`376dbd90…`,
  тест `b7488fcb…`), лог мутаций перегенерирован.
- **P3-2** red/green перегенерены на финальных байтах теста: red — `E0432` на импорте
  `format_duration` в `duration_contract.rs`, `# EXIT=101`; green — `13 passed`, `# EXIT=0`.
- **P3-3** мутационный харнесс теперь проверяет зелёный baseline и засчитывает `KILLED` только
  при `test result: … FAILED` (ненулевой exit без падения теста → `INCONCLUSIVE`).
- **P3-4** независимый подсчёт паттернов оставлен на оракул (`tests/data`), константа в модуле
  документирована как производная от строк; d13 оставлен как требование ТЗ на детерминизм.
- **P3-5** d09 дополнен выключенной локалью `ar`; d06 — группировкой `10_000_000` (en/ru).
- **P3-6** capacity-нит и all-zero кейс оракула — в бэклоге `waves/STATE.md` §7; `BATCH-TZ`
  про «1 строку» уточнён в отчёте (фактически 2 строки: модуль + сгенерированный сосед).
