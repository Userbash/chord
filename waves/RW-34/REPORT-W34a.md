# REPORT W34a — P6-02c D-Bus abstract-адреса (headless)

Волна: **RW-34**. Batch: `BATCH-TZ.md`. Карточка: **P6-02c** (seat PLATFORM-1 / W34a).
Дата: 2026-10-23. Статус: **готово к приёмке** (red → green → мутации 6/6 → отчёт).

Окружение: Linux, `rustc 1.99.0`, `dbus-daemon 1.16.2` (приватный `--session`),
`python3-dbus` + `gi` (libdbus-мок `waves/RW-30/dbus/mock_service.py`). Abstract-сокеты
в окружении разрешены — стенд поднимается по-настоящему, `SKIP` в прогонах нет.

## 1. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-platform/src/dbus/mod.rs` | `f7761028d173f04b600dddb8adfa0933` |
| `rust/crates/discord-platform/tests/dbus_contract.rs` | `2802321a4df54e275c3d8d10bf93654d` |
| `waves/RW-34/run-mutations-w34a.py` | `9de6875ea78f06aa664ce3ec6a2d7e7e` |
| `waves/RW-34/workers/W34a/red.log` | `505f382d3b5813fde9cd26ed4bed60fa` |
| `waves/RW-34/workers/W34a/green.log` | `0a2c6ad265a44af6789c8ca335f91d82` |
| `waves/RW-34/mutations-w34a.log` | `6a5dfc02f81bd306c66db60c777b74d6` |

Изменены только два файла карточки. `notifications.rs`, `message.rs`, `auth.rs`, `power.rs`,
`lib.rs`, `Cargo.toml`, чужие тесты, golden и доки не трогались; `unsafe` не добавлялся,
новых зависимостей нет. Правки `notifications.rs`/`notifications_contract.rs` и
`fixtures/dbus/hints-ext.json` в рабочем дереве — параллельный воркер W34b.

## 2. Red / green

Команда: `cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w34a cargo test --offline
-p discord-platform --test dbus_contract`.

- Red: **EXIT=101**, 24 ошибки компиляции (`E0432`/`E0599`/`E0277`): нет
  `BusAddress::Abstract`, `abstract_name`, `MAX_ABSTRACT_NAME_BYTES`, `path()` ещё не
  `Option` (`workers/W34a/red.log`). Тесты a01–a07 написаны до кода.
- Green: **15 passed, 0 failed, SKIP=0**, **EXIT=0** (`workers/W34a/green.log`).
  Прогон с `-- --nocapture`: `SKIP:` не печатается, `a04`/`a05`/`a07` реально поднимают
  `dbus-daemon` (a04/a05 — на `unix:abstract=…` + libdbus-мок).
- Регрессия path-ветки: `--test power_contract` — **12 passed, 0 failed** (потребители
  `Connection::connect` через `unix:path` не затронуты).

## 3. Кейсы (TZ §3)

| ID | Тест | Проверка | Результат |
|---|---|---|---|
| a01 | `a01_parse_path_yields_path_and_no_abstract_name` | `unix:path=…` → `Path`, `path()` Some, `abstract_name()` None | ok |
| a02 | `a02_parse_abstract_keeps_raw_bytes_up_to_kernel_limit` | abstract → байты как есть; `guid` не входит в имя; 107 байт — предел, 108 → None | ok |
| a03 | `a03_parse_skips_unsupported_and_invalid_entries` | `tcp:`, пусто, мусор, пустой path/abstract, NUL, `guid`-only, список `;` (первый поддерживаемый), невалидная запись → следующая | ok |
| a04 | `a04_connect_abstract_bus_yields_unique_name` | connect к приватному `unix:abstract=…` daemon, `Hello`, имя `:1.x` | ok |
| a05 | `a05_call_over_abstract_bus_returns_mock_reply` | `GetCapabilities` через abstract-шину → ответ мока + запись в TSV-журнале libdbus-мока | ok |
| a06 | `a06_connect_missing_abstract_returns_error_without_panic` | несуществующее abstract-имя → `DbusError::Io` (ECONNREFUSED), без паники | ok |
| a07 | `a07_path_branch_still_connects_to_path_bus` | штатный `--session` (`unix:path=…`) → Hello, path-ветка цела | ok |

Бюджет 6–8: добавлено 7 тестов; старые кейсы файла (n01–n14, q08) остались зелёными.
Итого в `dbus_contract`: 15 passed.

**Обновление n01 (осознанное, по замороженному контракту).** До карточки n01 утверждал
`BusAddress::parse("unix:abstract=…") == None` и `path() -> &Path`; замороженный контракт
TZ §1 делает abstract поддерживаемым, а `path()` — `Option<&Path>`. Поэтому в n01:
abstract-кейсы вынесены в a01–a03, проверки `unix:path`/`guid`/`from_env` сохранены,
сравнение идёт по `Option`. Это не «подгонка теста под код»: изменён сам контракт, путь
`unix:path` проверяется теми же утверждениями. Skip-сообщение n01 про незаданный
`DBUS_SESSION_BUS_ADDRESS` заменено на `INFO:` — тест не пропускается, выполняется целиком
(нужно для baseline `SKIP=0`).

## 4. Мутации

`waves/RW-34/run-mutations-w34a.py`, targeted с `-- --nocapture`, baseline —
`exit=0, SKIP=0, test result=ok`. **6/6 KILLED**, md5 восстановлены (до/после совпадают,
тестовый файл тоже под защитой md5):

| # | Мутация | Убита тестами |
|---|---|---|
| 01 | `from_path`: снята проверка NUL (`unix:path=/tmp/\0bus` → Path) | a03 |
| 02 | `from_abstract_name`: разрешено пустое имя (`unix:abstract=` → `Abstract([])`) | a03 |
| 03 | предел имени off-by-one (`>= 107` вместо `> 107`) | a02 |
| 04 | имя abstract берётся из всей записи, а не из поля — заглатывает `,guid=…` | a02, a04, a05 |
| 05 | список `;` не перебирается (`take(1)`) — `tcp:…;unix:abstract=…` → None | a03 |
| 06 | `connect_addr` идёт в усечённое имя (`&name[..1]`) — connection refused | a04, a05 |

Вердикт `KILLED` выставляется только при `exit != 0` и строке `test result: FAILED`;
`INCONCLUSIVE` — если были `SKIP:`; baseline со `SKIP:` останавливает прогон. Лог пишет сам
скрипт (`mutations-w34a.log`), stdout в файл лога не перенаправляется.

## 5. Решения и ограничения

- **Контракт.** `pub enum BusAddress { Path(PathBuf), Abstract(Vec<u8>) }`;
  `parse`, `from_env`, `path() -> Option<&Path>`, `abstract_name() -> Option<&[u8]>`;
  `Connection::connect(&BusAddress)` — сигнатура сохранена. Ровно один из `path()`/
  `abstract_name()` даёт `Some`.
- **Разбор.** Записи `;` перебираются по порядку, берётся первый поддерживаемый; запись с
  чужой схемой (`tcp:`), пустым значением, NUL внутри значения или именем > 107 байт
  пропускается (для одиночного адреса это `None`). `guid` и прочие ключи того же адреса
  игнорируются; имя abstract обрезается по `,`/`;`, байты не декодируются.
- **Транспорт.** `Path` — `UnixStream::connect`; `Abstract` — стабильные
  `std::os::linux::net::SocketAddrExt::from_abstract_name` +
  `UnixStream::connect_addr` (без `SocketAncillary` и без `unsafe`). Ошибка построения
  адреса → `DbusError::Address`, отказ соединения → `DbusError::Io`.
- **Платформа.** Abstract-сокеты — только Linux: на других ОС `parse` по-прежнему даёт
  `Abstract` (адрес валиден по спеке), а `connect` возвращает `DbusError::Address`
  (`#[cfg(not(target_os = "linux"))]`).
- **Не поддержано (документировано в модуле):** percent-encoding (`%2F`) не декодируется,
  `tcp:`/`nonce-tcp:`/`launchd:`/`autolaunch:` → `None`; аутентификация/Hello/вызовы не
  менялись.
- **Стенд.** Приватный `dbus-daemon --session --address=unix:abstract=discord-lab-<pid>-<n>`
  + libdbus-мок; каждый тест — свой демон/мок/tempdir (pid + наносекунды + счётчик), всё
  снимается в `Drop`. Оракул — журнал мока (TSV) и уникальное имя от демона, не наш код.
  Если `dbus-daemon`/`python3-dbus` недоступны или abstract-сокеты запрещены — тест печатает
  `SKIP: причина` и не падает.
- **Совместимость.** `notifications`/`power` используют `Notifier::connect`/`Inhibitor::connect`
  без изменений; проверено `power_contract` (12/12). `notifications_contract` в этом прогоне
  не собирался: файл в работе у W34b (независимая причина, к dbus-адресам не относится).

## 6. Проверки

- `rustfmt --edition 2024` по двум файлам карточки — чисто (`cargo fmt -p discord-platform`
  не запускался целиком, чтобы не форматировать файлы W34b).
- `cargo clippy --offline -p discord-platform --test dbus_contract -- -D warnings` — 0
  предупреждений.
- `cargo test --offline -p discord-platform --test dbus_contract` — 15 passed, SKIP=0.
- `cargo test --offline -p discord-platform --test power_contract` — 12 passed (регрессия
  path-ветки и потребителей).
- Полный `gate:worker`/workspace не запускался: batch-воркер делает только targeted,
  общий гейт — один раз в integration-лейне (BATCH-TZ §4, `PROCESS.md`).
