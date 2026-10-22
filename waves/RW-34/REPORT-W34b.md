# REPORT W34b — P6-02d расширенные hints `a{sv}` (headless)

- **batch/карточка:** RW-34 / P6-02d, seat PLATFORM-2. Дата: 2026-10-23.
- **репозиторий:** `discord-lab`, крейт `discord-platform`, поверх принятого P6-02b (RW-32).
- Сеть/live-уведомления не использовались; `unsafe` не добавлялся; новых зависимостей нет;
  `git commit/push` не выполнялись. `dbus/` (mod/message/auth), `power.rs`, `spellcheck*`,
  `keyring*`, `lib.rs`, `Cargo.toml`, golden-каталоги не тронуты.
- Полный batch-гейт не запускался (по AGENTS.md §3 это integration-лейн); здесь targeted
  прогон + мутации.

## 1. Сделано (аддитивно)

`src/notifications.rs`:

- `Hints` расширен четырьмя полями замороженного контракта:
  `image_path: Option<String>`, `sound_name: Option<String>`, `transient: Option<bool>`,
  `suppress_sound: Option<bool>` (существующие `urgency`/`category`/`desktop_entry`/
  `resident` не менялись).
- `Hints::to_dbus()` пишет `a{sv}` в фиксированном порядке:
  `urgency` (`y`), `category` (`s`), `desktop-entry` (`s`), `image-path` (`s`),
  `sound-name` (`s`), `resident` (`b`), `transient` (`b`), `suppress-sound` (`b`).
  Пустые строки (`category`, `desktop_entry`, `image_path`, `sound_name`) пропускаются
  через существующий `non_empty`; `Some(false)` для boolean — запись `b:0`, не пропуск.
- Сигнатура `Notify` (`NOTIFY_SIGNATURE = "susssasa{sv}i"`) не менялась, `notify()` по-прежнему
  `notify_with_hints(..., &Hints::default())`; обновлена только документация модуля/метода.
- Диф ограничен `notifications.rs`: +33/−5, поведение старых полей сохранено (q01–q07 зелёные).

Оракулы (независимы от Rust-кода):

- `waves/RW-34/dbus/mock_hints2.py` — копия мока W32b (`mock_hints.py` не изменён, md5
  оригинала `d983d49076c5197cfd03d841d013376f`); правлена только docstring-шапка, логика
  журнала (`ключ:сигнатура:значение|…` в порядке провода) идентична. Мок на libdbus, поэтому
  тип варианта и ключ видит демаршалинг python3-dbus, а не наш маршалер.
- `fixtures/dbus/hints-ext.json` — 6 наборов, для каждого полные байты
  `Notify(susssasa{sv}i)` от **libgio** (`Gio.DBusMessage.to_blob()`), плюс снимок журнала
  `mock_hints2.py` (что реально демаршалировал libdbus) и общий блок `notify`.
  Наборы: `all_eight`, `image_and_sound`, `bools_false`, `empties_normalized`, `unicode`,
  `sound_and_image_alt`. Генератор — `waves/RW-34/dbus/gen-hints-ext.py` (дополнительный
  артефакт волны, вне перечня карточки: нужен для воспроизводимости; см. §7).

## 2. Red

Тесты e01–e07 добавлены до реализации; прогон на старом `Hints` (4 поля):

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w34b \
  cargo test --offline -p discord-platform --test notifications_contract
# EXIT=101 — 22 ошибки: E0560 «struct Hints has no field named image_path/sound_name/
# transient/suppress_sound» и E0609 в хелперах e07
```

Лог: `waves/RW-34/workers/W34b/red.log`, последняя строка `# EXIT=101`.

## 3. Green

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w34b \
  cargo test --offline -p discord-platform --test notifications_contract
# test result: ok. 25 passed; 0 failed; 0 ignored
# EXIT=0
```

Лог: `waves/RW-34/workers/W34b/green.log`, `# EXIT=0`, предупреждений компилятора нет,
harness-SKIP нет (приватный `dbus-daemon` + `python3-dbus`/`gi` доступны; `n16` — намеренный
live-skip по флагу `DISCORD_LAB_LIVE_NOTIFY`). 25 тестов = 18 прежних (n/q) + 7 новых e01–e07;
поведение прежних hints не сломано. `rustfmt --edition 2024 --check` по обоим файлам и
`cargo fmt --all -- --check` — чисто; `cargo clippy -p discord-platform --test
notifications_contract -- -D warnings` — без замечаний.

## 4. Тесты (7 новых, бюджет 7–9)

| тест | кейс | проверка |
|---|---|---|
| `e01_all_eight_hints_reach_mock_with_freedesktop_types` | e01 | все 8 hints → `hints_len=8`, каждая запись `ключ:сигнатура:значение` с типом `y`/`s`/`b` (сверка как множество, порядок — в e05) |
| `e02_image_path_and_sound_name_are_string_variants` | e02 | `image-path:s:file:///tmp/avatar.png\|sound-name:s:alarm-clock-elapsed` |
| `e03_transient_and_suppress_sound_are_bools_including_false` | e03 | `transient:b:1\|suppress-sound:b:0` и `transient:b:0\|suppress-sound:b:1` — false не пропускается |
| `e04_empty_new_strings_are_skipped` | e04 | пустые `image_path`/`sound_name` (и старые строки) не попадают в dict: 2 записи; только пустые строки → 0 записей |
| `e05_key_order_is_frozen` | e05 | поля заданы в обратном порядке; журнал строго `urgency,category,desktop-entry,image-path,sound-name,resident,transient,suppress-sound` |
| `e06_unicode_and_long_new_hints_are_delivered_without_panic` | e06 | `image-path` 4096 байт UTF-8, `sound-name` 4096 ASCII, Unicode-category — доставлены без искажений |
| `e07_hints_bytes_match_libgio_and_round_trip` | e07 | 6 наборов фикстуры: `Hints::to_dbus()` = эталон (ключи/типы/порядок), `parse(libgio bytes).body[6]` = эталон, сборка нашим `message::build` побайтово равна libgio, round-trip `parse` даёт то же тело; сигнатура `susssasa{sv}i` |

## 5. Мутации (6, бюджет 4–6)

`waves/RW-34/run-mutations-w34b.py` (stdout не перенаправляется в лог; журнал пишет сам
скрипт). Baseline: `exit=0, SKIP=0`. Каждая мутация — один дефект в `src/notifications.rs`,
restore из снимка с проверкой md5.

| ID | мутация | класс | итог | упавшие тесты |
|---|---|---|---|---|
| 01 | `image_path_key_underscore` (`image-path` → `image_path`) | ключ | KILLED | e01, e02, e05, e06, e07 |
| 02 | `transient_variant_byte` (`b` → `y`) | тип варианта | KILLED | e01, e03, e05, e07 |
| 03 | `empty_image_path_not_skipped` (`non_empty` → `as_deref`) | пропуск пустой | KILLED | e04 |
| 04 | `sound_name_before_image_path` | порядок | KILLED | e02, e05, e06, e07 |
| 05 | `suppress_sound_dropped` | наличие записи | KILLED | e01, e03, e05, e07 |
| 06 | `transient_false_skipped` (`Some(false)` → пропуск) | значение false | KILLED | e03, e07 |

`KILLED` — только при `test result: FAILED` и `SKIP=0`; исходные md5 восстановлены
(`0355d73eaa26611bbdc2a9fe38a798e6` до/после каждой мутации). Итог: 6/6 убиты.
Лог: `waves/RW-34/mutations-w34b.log`.

## 6. Файлы (md5)

| файл | md5 |
|---|---|
| `rust/crates/discord-platform/src/notifications.rs` | `0355d73eaa26611bbdc2a9fe38a798e6` |
| `rust/crates/discord-platform/tests/notifications_contract.rs` | `3496ad0662a6a1d5299c1afe440a27d0` |
| `waves/RW-34/dbus/mock_hints2.py` | `014f727bf8e38b97cb6ee109c2cd32dc` |
| `waves/RW-34/dbus/gen-hints-ext.py` | `f0abf041d7b89cd1f3b45ab6f32996f0` |
| `fixtures/dbus/hints-ext.json` | `d199e7aaa31248a3e81f6a4f0155b325` |
| `waves/RW-34/run-mutations-w34b.py` | `be0568de858d20dc1a049fbfae262a78` |
| `waves/RW-34/workers/W34b/red.log` | `5294b708967f5caca99a9ada4b326d5d` |
| `waves/RW-34/workers/W34b/green.log` | `855a2fe43602d392c702360e24addde2` |
| `waves/RW-34/mutations-w34b.log` | `49790fd0ceae0d8ec1d690e14a0b85d5` |
| `waves/RW-34/REPORT-W34b.md` | (этот файл; md5 меняется при правках — уточнит TL) |

Оригинал `waves/RW-32/dbus/mock_hints.py` не изменён:
`d983d49076c5197cfd03d841d013376f`.

## 7. Воспроизведение фикстуры

```sh
cd /home/sanya/orca/project && python3 waves/RW-34/dbus/gen-hints-ext.py
# hints-ext.json: 6 наборов
# md5 fixtures/dbus/hints-ext.json = d199e7aaa31248a3e81f6a4f0155b325
```

`gen-hints-ext.py` — единственный не указанный в ТЗ файл (дополнительный артефакт волны):
байты `Notify` собирает **libgio** (`Gio.DBusMessage.to_blob()`), снимок журнала — приватный
`dbus-daemon` + `mock_hints2.py` (libdbus). Наборы заданы в порядке контракта (urgency,
category, desktop-entry, image-path, sound-name, resident, transient, suppress-sound),
потому что libgio сериализует `a{sv}` в порядке вставки. Проверено: повторный запуск даёт
тот же md5 фикстуры. Сеть не используется, `ensure_ascii=False`. Если TL считает файл
лишним — фикстура остаётся валидной, генератор можно удалить (код воспроизводим по истории
волны).

## 8. Ограничения

- Проверен только targeted-таргет `notifications_contract`; `cargo fmt`/`clippy`/
  `test --workspace`/`gate:worker` — за integration-лейном RW-34.
- `e07` опирается на libgio-байты фикстуры; при смене общего блока `notify`
  (app_name/summary/body/actions/timeout) фикстуру нужно перегенерировать.
- Оракул-мок — копия W32b; формат журнала не менялся, поэтому новые хинты проверяются
  общим механизмом `ключ:сигнатура:значение` (значения с `|`/`:` в hints форматом журнала
  не поддерживаются — в тестах таких значений нет).
- `image-path`/`sound-name` — только передача хинтов; реальное отображение иконки/звука
  зависит от сервера уведомлений (вне этого модуля, как и в P6-02b).
- Live-сервис уведомлений на рабочем столе не затрагивался (n16 flag-gated, только чтение
  возможностей).
