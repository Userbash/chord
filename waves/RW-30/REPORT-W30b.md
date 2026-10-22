# REPORT W30b — P6-02 D-Bus-уведомления (headless, без новых зависимостей)

Батч: RW-30, карточка P6-02, seat PLATFORM-1. Дата: 2026-10-22.
Репозиторий: `discord-lab`, крейт `discord-platform`. Сеть не использовалась, `unsafe` нет,
новых зависимостей нет (только `std`).

## 1. Что сделано

- `dbus::{BusAddress, Value, DbusError, Connection, Signal, MessageType}` — клиент D-Bus на
  `std::os::unix::net::UnixStream`: AUTH EXTERNAL, `Hello`, method call/reply/error,
  `AddMatch`, сигналы с очередью (сигналы, пришедшие во время `call`, не теряются).
- `dbus::message` — wire-формат: заголовок `yyyyuua(yv)`, поля в порядке вызова, little-endian,
  выравнивание от начала сообщения, `a(ss)`/`a{sv}`/вложенные контейнеры, разбор с защитой от
  обрезанных/битых пакетов (`Protocol`, без паник).
- `dbus::auth` — uid без `unsafe` (чтение `/proc/self/status`), `AUTH EXTERNAL <hex>`,
  ожидание `OK <guid>`, `BEGIN`.
- `notifications::{Notification, Notifier, NotificationEvent}` — `Notify`,
  `CloseNotification`, `GetCapabilities`, `GetServerInformation`, `listen_actions`,
  `next_event` (`ActionInvoked`/`NotificationClosed`, чужие сигналы пропускаются).
- Тесты: `tests/dbus_contract.rs` (wire/адрес/auth, оракул — libgio) и
  `tests/notifications_contract.rs` (приватный `dbus-daemon` + `mock_service.py` на libdbus).
- Оракулы: `waves/RW-30/dbus/mock_service.py` (libdbus-сервер, журнал вызовов TSV),
  `waves/RW-30/dbus/gen-golden.py` → `fixtures/dbus/{wire,notifications}.json`.

## 2. Оракулы и доказательная база

- `fixtures/dbus/wire.json` — 5 сообщений (method call `Notify`, signal `ActionInvoked`,
  method return `as`, error reply, signal `a(ss)`), байты собраны **libgio**
  (`Gio.DBusMessage.to_blob()`, GLib 2.90.1). Rust-тест разбирает эти байты и собирает
  сообщение обратно из разобранных полей: **побайтовое совпадение** с оракулом (n03), плюс
  проверка границы заголовок/тело и нулевого padding (n04).
- `fixtures/dbus/notifications.json` — ответы настоящего libdbus-сервера на приватном
  `dbus-daemon` (`mock_service.py`): сигнатуры `Notify`/`CloseNotification`/
  `GetCapabilities`/`GetServerInformation` и формы ответов. Используется в n07b для
  сверки `NOTIFY_SIGNATURE` и форм ответов.
- Мок-сервис умеет: отвечать на 4 метода, эмитить `ActionInvoked`/`NotificationClosed`,
  эмитить «чужой» сигнал (другой интерфейс + `ActionInvoked` с телом `s`) и падать
  D-Bus-ошибкой (`--fail-notify`) — всё это проверяется conтрактными тестами.

## 3. Red / green

Red (до реализации, модулей `dbus`/`notifications` нет):

```
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w30b \
  cargo test --offline -p discord-platform --test dbus_contract --test notifications_contract
# EXIT=101  (E0432 unresolved import `discord_platform::dbus`/`notifications`)
```

Лог: `waves/RW-30/workers/W30b/red.log`, последняя строка `# EXIT=101`.

Green (финальный прогон, предписанный `CARGO_TARGET_DIR`):

```
test result: ok. 7 passed; 0 failed  (dbus_contract)
test result: ok. 11 passed; 0 failed (notifications_contract)
# EXIT=0
```

Лог: `waves/RW-30/workers/W30b/green.log`, последняя строка `# EXIT=0`. Предупреждений нет.

## 4. Тесты (18, кейсы n01–n16)

`dbus_contract.rs` (7):

| тест | кейс | проверка |
|---|---|---|
| `n01_bus_address_parse_accepts_path_and_rejects_abstract` | n01 | `unix:path`, `abstract`→None, мусор→None, список через `;`, `guid=…`, `from_env` |
| `n02b_external_auth_line_uses_hex_uid_from_proc_status` | n02 | uid из `/proc/self/status` = `id -u`; `AUTH EXTERNAL 31303030` для 1000; разбор `Uid:` |
| `n03_marshal_round_trip_matches_libgio_bytes` | n03/n04 | разбор 5 golden-сообщений libgio → сборка → побайтовое совпадение |
| `n04_header_body_boundary_is_aligned_and_padded_with_zeroes` | n04 | тело 8-aligned, padding нулевой, `body_len` сходится; `a(ss)` из golden |
| `n05_parse_recovers_header_fields_and_body` | n05 | path/iface/member/destination/signature/reply_serial/error_name и тело |
| `n14_parse_rejects_truncated_and_corrupt_messages_without_panic` | n14 | 9 обрезаний, big-endian, неизвестный тип, лишний байт, битая `body_len`, мусор |
| `n07b_notifications_signatures_match_libdbus_golden` | n07 | `NOTIFY_SIGNATURE`, `u`/`as`/`ssss`/`u`, объявленные capabilities |

`notifications_contract.rs` (11):

| тест | кейс | проверка |
|---|---|---|
| `n02_auth_external_and_hello_yield_unique_name` | n02/n06 | приватный демон: `OK`, `Hello` → `:1.x` |
| `n07_capabilities_and_server_information_come_from_mock` | n07 | список из мока + `(имя, вендор, версия, спека)` |
| `n08_notify_returns_server_id_and_mock_sees_fields` | n08 | id из ответа; журнал: app_name/summary/body/hints_len=0/timeout |
| `n09_notify_actions_reach_mock_as_pairs` | n09 | пары id/label доходят (`open:Open\|reply:Reply`) |
| `n10_close_notification_reaches_mock` | n10 | `CloseNotification(id)` в журнале |
| `n11_error_reply_maps_to_remote_error` | n11 | `DbusError::Remote{name,message}` из ERROR-ответа мока |
| `n12_action_and_close_signals_become_events` | n12 | `ActionInvoked`/`NotificationClosed` → `NotificationEvent` |
| `n13_foreign_signals_are_skipped` | n13 | чужой интерфейс и `ActionInvoked(s)` пропущены, валидный — принят |
| `n15_repeated_close_and_closed_session_survive` | n15 | повторный `close` → Ok (2 записи в журнале); после kill демона — ошибка без паники |
| `n08b_notify_requires_app_name_and_summary` | n08 | пустые `app_name`/`summary` → `Protocol`, на шину не уходит |
| `n16_live_service_is_flag_gated_and_read_only` | n16 | без `DISCORD_LAB_LIVE_NOTIFY=1` — SKIP; с флагом только чтение capabilities |

Skip-кейсы (печатают `SKIP: <причина>`, тест зелёный):
- `n16` — флаг live не выставлен (в прогоне: `SKIP: live-уведомления выключены`);
- все тесты с демоном — при отсутствии `dbus-daemon`/`python3-dbus`/`gi` или если мок не
  поднялся за 10 с (`SKIP: нет dbus-daemon`, `SKIP: нет python3 с модулями dbus и gi`);
- проверка `from_env` внутри `n01` — если `DBUS_SESSION_BUS_ADDRESS` не задан.

Live-уведомление на рабочем столе **не отправлялось** (даже `n16` только читает
capabilities).

## 5. Мутации (10/10 убиты)

Команда: `CARGO_TARGET_DIR=/tmp/opencode/target-w30b python3 waves/RW-30/run-mutations-w30b.py`
(каждая мутация — один точечный дефект, затем прогон обоих тестов и restore по md5).
Лог: `waves/RW-30/mutations-w30b.log` (итог: «все мутации убиты, все файлы восстановлены»).

| # | мутация | убита тестами |
|---|---|---|
| 01 | auth: hex числа вместо hex десятичной записи uid | n02b |
| 02 | выравнивание struct/dict-entry 8 → 4 | n03, n04, n05 |
| 03 | ответ не сопоставляется по `reply_serial` | n02, n07, n08, n08b, n09, n10, n11, n12, n13, n15 |
| 04 | ERROR-ответ как успех | n11 |
| 05 | `actions`: label/id перепутаны | n09 |
| 06 | нет `AddMatch` на `NotificationClosed` | n12 |
| 07 | событие принимает чужое тело | n13 |
| 08 | serial не растёт (0) | все вызовы (10 тестов) |
| 09 | снят guard пустого `summary` | n08b |
| 10 | `hints` не пустой | n08 |

## 6. Файлы и md5

```
608918a56769fbd19d582c944cf0c848  rust/crates/discord-platform/src/dbus/mod.rs
ca96abc04abeac0c0b723d72b44389e9  rust/crates/discord-platform/src/dbus/auth.rs
c13dd0b3d8392e59fa52b6b08d61cdf9  rust/crates/discord-platform/src/dbus/message.rs
10d14433568f81cef00ee1526b2ae77d  rust/crates/discord-platform/src/notifications.rs
d66be18537144106751a900a681d2c0f  rust/crates/discord-platform/src/lib.rs   (только +2 строки модулей)
73a94d383f6b0a34fd9be7b89b6c56a9  rust/crates/discord-platform/tests/dbus_contract.rs
2fde3c5a7b29752198a4ff1fd4c2c8dd  rust/crates/discord-platform/tests/notifications_contract.rs
cf818d22ba71c168d243cd6c3b82fb73  waves/RW-30/dbus/mock_service.py
31dd93d6f04d78eea3b187d80abfb875  waves/RW-30/dbus/gen-golden.py
61e3324ad66823306caa422392f9d461  fixtures/dbus/wire.json
b3edcfd0c54af9dfcc202fbcf2fddf40  fixtures/dbus/notifications.json
0e00848e3236a374b7bfa0cb90691c05  waves/RW-30/run-mutations-w30b.py
651946d739d02bc1c270c05b306d4854  waves/RW-30/workers/W30b/red.log
bceb724a42bca0d691ad8ddba7562661  waves/RW-30/workers/W30b/green.log
f72d166dbad0b1870bd556333e9760e1  waves/RW-30/mutations-w30b.log
```

`src/lib.rs` не переформатировался: добавлены только `pub mod dbus;` и
`pub mod notifications;` (строки `pub mod autostart;`/`pub mod deep_link;` уже были в рабочем
дереве до карточки).

## 7. Отклонения от буквы ТЗ (обоснованы оракулом)

1. **`actions` на проводе — `as`, не `a(ss)`.** Спека freedesktop и сигнатура
   `susssasa{sv}i` описывают `actions` как плоский список парами id/label; libdbus-мок
   отвергает `a(ss)` (несовпадение сигнатуры). Модель остаётся `Vec<(String, String)>`
   («`a(ss)` по смыслу»), тест n09 фиксирует, что пары доходят до сервера.
2. **Идентичность EXTERNAL — hex от десятичной записи uid** (`1000` → `31303030`), как в
   libdbus; hex числа (`3e8`) демон отвергает (`REJECTED EXTERNAL`). Зафиксировано n02b/n02.
3. **Тело ответа/сигнала — последовательность типов без struct-обёртки**: `GetServerInformation`
   возвращает `ssss`, сигналы — `us`/`uu`. `Value::Struct` используется только там, где struct
   реально есть в сигнатуре (`a(ss)`).
4. **Флаг 0x1 (NO_REPLY_EXPECTED)** ставится всем сообщениям, кроме method call — как libgio;
   без этого байты расходятся с golden (n03).

## 8. Ограничения (не заявляется как поддержка)

- `hints` — всегда пустой `a{sv}` (обосновано `RE-NOTIFICATIONS.md` §3; urgency/image-path —
  отдельное расширение под UI-потребителя).
- `unix:abstract=` не поддержан (`BusAddress::parse` → None), `tcp:` — тоже; список адресов
  через `;` поддержан (берётся первый `unix:path`), percent-кодирование в адресе не
  декодируется.
- Только little-endian; типы `x`/`t`/`d`/`h`/`n`/`q` и unix-fd не поддержаны (нет в `Value`),
  такие сообщения → `DbusError::Protocol`.
- Live-сервис `org.freedesktop.Notifications` на сессионной шине не использовался; уведомления
  не отправлялись. `n16` включается только `DISCORD_LAB_LIVE_NOTIFY=1` и даже тогда только
  читает capabilities.
- Wire-совместимость с Electron-модулем Discord не заявляется: на Linux модуль не
  загружается (`RE-NOTIFICATIONS.md` §1), это самостоятельный D-Bus-клиент.
- DND/группировка/история/бейджи — уровень приложения (P6-01/P3), не этого модуля.
- `Notification::default()` даёт `timeout_ms = 0` (замороженный `derive(Default)`); значение
  «таймаут сервера» `-1` задаётся явно — так и делает тестовый образец.
- Полный `cargo fmt`/`clippy`/`test --workspace`/`npm run gate:worker` не запускались: по
  регламенту батча это делает integration-лейн; воркер гонял только targeted-тесты.

## 9. Замечания по инфраструктуре

- В ходе работы tmpfs `/tmp` упирался в квоту (`EDQUOT`): промежуточные сборки шли в
  `/home/sanya/.cache/target-w30b`, финальные red/green/мутации — на предписанном
  `CARGO_TARGET_DIR=/tmp/opencode/target-w30b` (после освобождения места). На результат это не
  влияет; каталог `/home/sanya/.cache/target-w30b` можно удалить.
- `gen-golden.py` требует GLib ≥ 2.80 (`Gio.DBusMessage.to_blob()`); `mock_service.py` требует
  `python3-dbus` + `gi`. Оба инструмента skip-aware по окружению; фикстуры закоммичены, тесты
  от их регенерации не зависят (кроме пересчёта hex).
- `run-mutations-w30b.py` работает последовательно (cargo не запускается параллельно),
  восстановление файлов — через `finally` + сверка md5.

## 10. Риски / что смотреть на ревью

1. Сигнатуры `notifications`-констант и форм ответов закреплены golden-фикстурой libdbus:
   при смене мока нужно перегенерировать `fixtures/dbus/*.json` тем же `gen-golden.py`.
2. `next_event` использует фиксированный `EVENT_TIMEOUT = 5s` на сигнал (контракт `next_event`
   без параметра таймаута); при долгом отсутствии событий вернётся `DbusError::Timeout`.
3. `Connection` не потокобезопасен и не имеет keepalive-цикла: это клиент «по требованию»
   (вызов → ответ/сигнал), как и требует карточка.
4. Очередь сигналов растёт неограниченно, если потребитель не вызывает `next_signal`; для
   UI-потребителя (P6-01) стоит добавить предел/дренаж — вне scope этой карточки.
