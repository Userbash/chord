# REPORT RW-28 — P7-01c RelativeTimeFormat (headless)

Волна: RW-28. Дата: 2026-10-22. Карточка: P7-01c (хвост P7-01). Крейт: `discord-i18n`.
Сеть/live/новые зависимости не использовались.

## 1. Сделано

- `relative.rs`: `format_relative` с резолвом локали (`canonical_locale` + `is_enabled` →
  `DEFAULT_LOCALE`), направлением future/past (0 → future), ICU-спецформами `Auto`,
  плюральной категорией по `|value|` и фолбэком `other` → строка en-US → `"{n} {unit}"`.
- `relative_data.rs`: 744 строки (31 включённая локаль × 8 единиц × 3 стиля), генерируются
  `waves/RW-28/gen-relative-data.mjs` из Node 24 ICU; ручная правка запрещена.
- Оракул: `rust/crates/discord-i18n/tests/data/relative-oracle.json` (3392 кейса, внутри
  крейта — после ревью P2-1).
- Точные целочисленные пути после ревью P1-1: `plural::plural_category_integer` (операнды CLDR
  из `u64`, общий `select`), `format::format_integer_unsigned` (группировка по цифрам);
  `format_integer(i64)` переведён на точный путь.
- `relative::RELATIVE_ROW_COUNT` для инварианта 744 (ревью P2-2).
- Решение TL по P0-1 (Auto ±2 = ICU-паритет) — `waves/RW-28/DECISIONS.md`, §1 TZ обновлён.

## 2. Red / green

| Фаза | Команда | Итог |
|---|---|---|
| red | `CARGO_TARGET_DIR=/tmp/opencode/target-w28 cargo test -p discord-i18n --test relative_contract` (модуль `relative` скрыт, финальные байты теста) | `E0432 unresolved import`, `# EXIT=101` — `waves/RW-28/workers/W28a/red.log` |
| green | то же с модулем | **12 passed / 0 failed**, `# EXIT=0` — `waves/RW-28/workers/W28a/green.log` |

md5 теста, на котором сняты логи: `6dddd8fad1d47224bd99f00255abd913`; red — crate-level (см. §5, P3-5).

## 3. Тесты

12 тестов: t01 стили/коды, t02 ru-плюрализация, t03 de/es/ja/zh, t04 ICU-спецформы (±1/0/1 и локальные ±2,
фолбэк на `Always` для единиц без ±2), t05 фолбэк вне спецформ, t06 short/narrow,
t07 все 8 единиц, t08 неизвестная/выключенная локаль, t09 полная матрица оракула + покрытие
(31 локаль / 8 единиц / 3 стиля / 2 режима), t10 ноль и `i64::MIN`, t11 инварианты данных
(744 строки, нет `{`), t12 точность > 2^53 и русская категория `one`.

`discord-i18n` целиком: 8 lib + 3 + 6 + 10 + 2 + 13 + 2 + 12 = 56 тестов, 0 failed.

## 4. Мутации

`waves/RW-28/run-mutations.py` → `waves/RW-28/mutations.log`: **8/8 caught**, `restored_all=True`,
base md5 `19af4fc5c86b3380f70f2733ad84cd2a` (M1 past/future, M2 категория, M3 Auto, M4 Short,
M5 плейсхолдер, M6 группировка, M7 ноль, M8 единица).

## 5. Независимое ревью

Вердикт первого прохода: «доработка». Закрыто red-first/решением TL:

- **P0-1** `Auto` ±2 vs замороженный §1 → TL-решение: ICU-паритет, §1 обновлён, t04 усилен
  проверкой фолбэка на `Always`; данные/оракул не перегенерированы по существу.
- **P1-1** потеря точности `|value| > 2^53` → точные `plural_category_integer` /
  `format_integer_unsigned`, тест t12.
- **P2-1** оракул вне крейта → `tests/data/`, `include_str!("data/…")`.
- **P2-2** t11 без счётчика строк → `RELATIVE_ROW_COUNT == 744`, t09 проверяет покрытие.
- **P2-3** логи без exit code/устаревшие → red/green перегенерированы на финальных байтах,
  мутации сохранены логом.
- **P3** `Node %s` в шапке генератора исправлен; doc про точные коды локалей добавлен.

Второй проход: код принят, P0/P1 = 0; закрыто:

- **P2-1** base md5 мутаций в §4 приведён к фактическому (`19af4fc5…`);
- **P2-2** §6 теперь указывает на актуальный общий гейт `waves/RW-29/gate-cargo.log`
  (953 passed, 12 relative-тестов) и `waves/RW-29/gate-worker.log`;
- **P3-1** строка t04 в TZ §3 приведена к §1;
- **P3-2** путь оракула в doc-комментарии теста — `tests/data/relative-oracle.json`;
- **P3-3** doc `format_integer_unsigned` исправлен (функция теперь используется и
  `format_integer`, и relative-путём);
- **P3-4** doc `format_relative` дополнен правилом точных кодов локалей; запись про ASCII-минус
  добавлена в бэклог `waves/STATE.md` §7;
- **P3-5** red-лог помечен как crate-level: `relative` скрыт, но `timestamp.rs` (RW-29) уже
  импортирует `format_relative`, поэтому сборка падает на lib до компиляции контрактного теста;
  exit-код и md5 финального теста в логе зафиксированы;
- **P3-6** t04 проверяет фолбэк `Always` для всех единиц без ±2 (year/quarter/month/week/hour/
  minute/second), t10 утверждает точные цифры `i64::MIN`, t09 сверяет точный набор 31 локали;
- **P3-7** in-place мутации и фиксированный target-dir — бэклог (§7 STATE), не блокер.

## 6. Гейт

- `cargo fmt --all -- --check` — 0; `cargo clippy --offline --workspace --all-targets -- -D warnings` — 0;
  `cargo test --offline --workspace` — `waves/RW-29/gate-cargo.log`: **953 passed / 0 failed**, 12 relative-тестов
  (включая t12), `FMT_EXIT=0`, `CLIPPY_EXIT=0`, `TEST_EXIT=0`.
- `npm run gate:worker` — passed, `waves/RW-29/gate-worker.log` (итоговый прогон после правок).

## 7. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-i18n/src/relative.rs` | `19af4fc5c86b3380f70f2733ad84cd2a` |
| `rust/crates/discord-i18n/src/relative_data.rs` | `cbf859cad0002c1dc5171a80c9775716` |
| `rust/crates/discord-i18n/src/plural.rs` | `3279808646d4d3ecee897bd634da0b0b` |
| `rust/crates/discord-i18n/src/format.rs` | `fb0ce887feb6c2d155bae3074f1d1e63` |
| `rust/crates/discord-i18n/src/lib.rs` | `dee649fed123b4a0e5aaa465c3dc1f34` |
| `rust/crates/discord-i18n/tests/relative_contract.rs` | `6dddd8fad1d47224bd99f00255abd913` |
| `rust/crates/discord-i18n/tests/data/relative-oracle.json` | `158028e633ceeeb08fdf334e3cd9d888` |
| `waves/RW-28/gen-relative-data.mjs` | `15f23274a93ec3b976d75610c186e5af` |

## 8. Риски и ограничения

- Wire/UI-совместимость с Discord не заявляется: контракт — ICU-семантика Node 24, не
  конкретная сборка клиента.
- `Auto` ±2 присутствует только там, где ICU его отдаёт; для остальных единиц — `Always`
  (покрыто t04).
- Точные пути исправляют > 2^53, но `format_number_with` (f64) для дробных значений по-прежнему
  ограничен f64 — вне карточки.
- Бэклог: ASCII-минус вместо U+2212 в `format_number_with` для `sv-SE/fi/no/hr/lt` (5 из 186
  проверок ревью), вынесен в `waves/STATE.md` §7.
