# REPORT W32b — P6-02b D-Bus hints `a{sv}` (headless)

- **batch/карточка:** RW-32 / P6-02b, seat PLATFORM-1. Дата: 2026-10-22.
- **репозиторий:** `discord-lab`, крейт `discord-platform`. Сеть/live не использовались,
  `unsafe`/новых зависимостей нет, `git commit/push` не выполнялись.
- **gate:worker:** воркер полный gate не запускает — targeted-прогон + мутации; полный
  batch-гейт за Integration/QA.

## 1. Сделано

- `src/notifications.rs` (аддитивно):
  - `Urgency { Low, Normal, Critical }` + `Urgency::code()` → `0/1/2`;
  - `Hints { urgency, category, desktop_entry, resident }` + `Default`;
  - `Hints::to_dbus() -> dbus::Value` — `a{sv}`: `urgency`→`y`, `category`/`desktop-entry`→`s`,
    `resident`→`b`; пустые строки пропускаются; порядок фиксирован —
    urgency, category, desktop-entry, resident;
  - `Notifier::notify_with_hints(&Notification, &Hints)`; `notify()` делегирует с
    `Hints::default()` (пустой dict, 0 записей, не `null`);
  - сигнатура вызова `NOTIFY_SIGNATURE = "susssasa{sv}i"` не изменилась.
- `src/dbus/message.rs` — **не менялся**: существующего `build/parse`/`Value::Dict/Variant`
  достаточно для `a{sv}` (выравнивание 4 у массива, 8 у dict-entry, `v` = signature+значение).
- Оракулы:
  - `waves/RW-32/dbus/mock_hints.py` — **отдельная копия** libdbus-мока (RW-30-мок не тронут):
    журнал `Notify` дополнен полем `hints` = `ключ:сигнатура:значение|…` в порядке провода
    (тип варианта виден как `y`/`s`/`b`), плюс `hints_len`;
  - `waves/RW-32/dbus/gen-hints.py` → `fixtures/dbus/hints.json`: 8 наборов (empty,
    urgency low/normal/critical, category, desktop-entry+resident, resident=false, all);
    для каждого — полные байты `Notify(susssasa{sv}i)`, собранные **libgio**
    (`Gio.DBusMessage.to_blob()`), и снимок журнала libdbus-мока;
  - Rust-тесты: q01–q07 — приватный `dbus-daemon` + `mock_hints.py` (libdbus демаршалит
    hints), q08 — побайтовая сверка с libgio + round-trip собственным парсером.

## 2. Red / green

Red (тесты добавлены, реализации `Hints`/`Urgency`/`notify_with_hints` нет; источник —
`HEAD`-версия `notifications.rs`, после прогона восстановлен по md5):

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w32b \
  cargo test --offline -p discord-platform --test notifications_contract --test dbus_contract
# EXIT=101 — E0432 unresolved imports Hints/Urgency, E0599 no method notify_with_hints
```

Лог: `waves/RW-32/workers/W32b/red.log`, последняя строка `# EXIT=101`.

Green (финальный прогон на текущем дереве):

```
test result: ok. 8 passed; 0 failed    (dbus_contract, вкл. q08)
test result: ok. 18 passed; 0 failed   (notifications_contract, вкл. q01–q07)
# EXIT=0
```

Лог: `waves/RW-32/workers/W32b/green.log`, последняя строка `# EXIT=0`, предупреждений нет.
`rustfmt --edition 2024 --check` по трём файлам — чисто.

## 3. Тесты (8 новых, бюджет 7–9)

`notifications_contract.rs` (7, mock на libdbus):

| тест | кейс | проверка |
|---|---|---|
| `q01_notify_without_hints_sends_empty_dict` | q01 | `notify` → `hints_len=0`, поле hints пустое; `notify_with_hints(Hints::default())` даёт то же |
| `q02_urgency_is_byte_variant_with_contract_values` | q02 | Low/Normal/Critical → `urgency:y:0/1/2` |
| `q03_category_is_string_variant` | q03 | `category:s:email.arrived` |
| `q04_desktop_entry_and_resident_have_string_and_bool_variants` | q04 | `desktop-entry:s:…|resident:b:1` и `b:0` для false |
| `q05_all_hints_are_four_entries_in_frozen_order` | q05 | 4 записи, точный порядок и типы |
| `q06_empty_category_and_desktop_entry_are_skipped` | q06 | пустые строки не попадают в dict (2 записи) |
| `q07_unicode_and_long_values_are_delivered_without_panic` | q07 | Unicode summary + category 4 КиБ + desktop-entry 4 КиБ (UTF-8), без паники |

`dbus_contract.rs` (1):

| тест | кейс | проверка |
|---|---|---|
| `q08_hints_dict_matches_libgio_bytes_and_round_trips` | q08 | 8 наборов: `Hints::to_dbus()` = эталон фикстуры (ключи/типы/порядок), libgio-байты `Notify` = сборка нашим маршалером, round-trip парсером, сигнатура `susssasa{sv}i` |

## 4. Мутации (6, бюджет 4–6)

`waves/RW-32/run-mutations-w32b.py`: baseline `exit=0, SKIP=0`; оба test-таргета
прогоняются **по отдельности** (`cargo test` с несколькими `--test` останавливается на
первом упавшем бинарнике, поэтому mock-оракул и byte-оракул проверяются независимо);
`KILLED` только при `test result: FAILED` и `SKIP=0` (намеренный live-skip n16 исключён);
restore по md5. Лог: `waves/RW-32/mutations-w32b.log`.

| # | мутация | вердикт | кто поймал |
|---|---|---|---|
| 01 | `urgency` вариант `y` → `u` | KILLED | q02, q05, q06, q08 |
| 02 | ключ `urgency` → `urgentcy` | KILLED | q02, q05, q06, q08 |
| 03 | пустые строки не пропускаются | KILLED | q06 |
| 04 | ключ `desktop_entry` (подчёркивание) | KILLED | q04, q05, q07, q08 |
| 05 | запись `resident` удалена | KILLED | q04, q05, q06, q08 |
| 06 | порядок: desktop-entry раньше category | KILLED | q05, q07, q08 |

Итог: 6/6 убиты, все файлы восстановлены (md5 до = после у каждой мутации).

## 5. Файлы + md5

| файл | md5 |
|---|---|
| `rust/crates/discord-platform/src/notifications.rs` | `84a41a3b7bd10563590c3f169f6fc18a` |
| `rust/crates/discord-platform/src/dbus/message.rs` (без изменений) | `268747b9eaf390b6ae00b3ee5f0e13eb` |
| `rust/crates/discord-platform/tests/notifications_contract.rs` | `be62957b819a20a19891beccc596cd1f` |
| `rust/crates/discord-platform/tests/dbus_contract.rs` | `40c1a7753b828da170afd0c19e0311f4` |
| `fixtures/dbus/hints.json` | `04bb74b6313df49ad2fb4a47787a7090` |
| `waves/RW-32/dbus/mock_hints.py` | `d983d49076c5197cfd03d841d013376f` |
| `waves/RW-32/dbus/gen-hints.py` (генератор фикстуры) | `23aa7bb529cf37ccc1a93404c89e468f` |
| `waves/RW-32/run-mutations-w32b.py` | `8dc738e4866aec822e10da9bd961a98e` |
| `waves/RW-32/mutations-w32b.log` | `e876c571eb3f596e07e2792b13797f8b` |
| `waves/RW-32/workers/W32b/red.log` | `a91f007aa6fb5a8c9c993fcd751438af` |
| `waves/RW-32/workers/W32b/green.log` | `243eff1ef7eb3fd2a10a7cc93a21786c` |

`src/lib.rs`, `Cargo.toml`, `spellcheck.rs`, `power.rs`, чужие тесты не трогались
(`git status`: изменения `Cargo.toml`/`lib.rs`/`spellcheck*` — работа параллельного W32a).

## 6. Ограничения и риски

- `hints` — только минимум freedesktop: `urgency`, `category`, `desktop-entry`, `resident`.
  `image-path`, `sound-name`, `suppress-sound`, `x`/`y`, `transient` и прочие не заявляются.
- `unix:abstract=` и unix-fd (`h`) по-прежнему не поддержаны (наследие RW-30, вне карточки).
- `Hints::to_dbus()` — публичный аддитивный метод (нужен q08 для сверки структуры);
  контрактные типы/поля из ТЗ не изменены.
- RW-30-мок `waves/RW-30/dbus/mock_service.py` не изменялся: расширение сделано отдельной
  копией `mock_hints.py`, чтобы не поменять формат журнала принятых тестов W30b.
- `fixtures/dbus/hints.json` содержит `journal`-снимок libdbus; при перегенерации
  (`python3 waves/RW-32/dbus/gen-hints.py`) он обновляется. Без окружения libdbus/libgio
  генератор пишет байты libgio и пропускает журнал (код 0).
- Live-сервис не трогался; n16 остаётся skip-aware под `DISCORD_LAB_LIVE_NOTIFY=1`.

## 7. Следующий шаг

Передать Team Lead: приёмка карточки P6-02b (границы файлов, red-first, независимые
оракулы libdbus + libgio, мутации), затем batch-гейт RW-32 в integration-лейне.

## После независимого ревью (P3-1)

Ревью: принято, P0/P1/P2 = 0. Закрыт P3-1 (утечка приватного `dbus-daemon` на error-path):
оба харнесса (`Harness`, `HintsHarness`) при ошибке старта мока убивают демон и удаляют tempdir
(`abort()`); red/green пересняты на финальных байтах теста (`# EXIT=101` / `# EXIT=0`,
8 + 18 passed), md5 обновлены. P3-2…P3-5 — бэклог волны.
