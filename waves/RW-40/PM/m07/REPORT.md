# M-07 — backend read-state ack (red-first)

Дата: 2026-10-24. Воркер: **G1 Backend**. PM: DeepSeek.
Статус: **готово к приёмке**. Live/окно/сеть/commit не использовались.

## Контракт (источник)

- `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.5 (M-07): `POST /channels/{id}/messages/{id}/ack`,
  200×5, тело **ровно** `{token, last_viewed}`; `token` nullable, **не bearer**;
  `last_viewed` — счётчик read-state.
- `docs/re/rest-d02.md` §4, requestId `ready|57992.7577`: `{"token":null,"last_viewed":4310}`.
- `fixtures/gateway/by-type/046-MESSAGE_ACK.json`: `last_viewed: 4310` при
  `message_id: 1561348877573947515` → `last_viewed` — **не** snowflake сообщения.
- Каталог уже содержит маршрут: `routes.rs` `body_keys = ["token","last_viewed"]`;
  `routes.rs` **не менялся**. `README/AGENTS`-запреты соблюдены (только свои файлы).

## Сделано

`rust/gui-spike/src/messages.rs` (без нового модуля — чтобы не трогать общий `src/lib.rs`):

- `MessageError` переиспользован (уже даёт `Unauthorized|Forbidden|NotFound|
  RateLimited|Client|Server|Network|Storage|InvalidResponse`, serde `kind`).
- `AckTransport` — POST с телом (fake в тестах пишет target/тело), боевой
  `ApiAckTransport` поверх `ApiClient::post_once` (ack — изменяющая операция, без повторов).
- `ReadStateAck` — объектно-безопасный контракт:
  `ack_message(channel_id, message_id, last_viewed: u64, token: Option<String>)`.
- `DiscordReadState::new(token)`: bearer только в `ApiAuth`; пустой → `InvalidArg`;
  ручной `Debug` (`finish_non_exhaustive`) — токен не печатается.
  `with_transport(Arc<dyn AckTransport>)` — тестовый вход без сети.
- `ack_body(last_viewed, token)`: тело дословно `{token, last_viewed}`, `None` → `null`;
  bearer в тело не попадает — единственный источник `token` это аргумент вызова.
- Путь: `routes::find(Post, "/channels/{id}/messages/{id}/ack")` + `render_path([channel, message])`;
  id валидируются (цифры) **до** транспорта; ошибки статусов — общий `RestFailure`/`decode_action`
  (200 → `Ok`; 401/403/404/429/прочие 4xx/5xx/сеть — типизированы; 429 из «сырого» статуса
  без выдуманного `retry_after` = 0).
- Свободная `ack_message(&A, …)` — один вход для Tauri и тестов.

`rust/gui-spike/src/main.rs`:

- Tauri-команда `ack_message(backend, channel_id, message_id, last_viewed: u64, token: Option<String>)`:
  bearer — только `token_or::<MessageError,_>(backend.token(), Storage, Unauthorized)`,
  переменная названа `bearer` и уходит в `DiscordReadState::new(&bearer)`; `token` — отдельный
  nullable аргумент тела; `last_viewed` — число; логирования нет.
- Команда добавлена в `invoke_handler!` и в `TOKEN_COMMANDS` (мета-тест «одна точка чтения токена»,
  теперь 9 команд).

`rust/gui-spike/tests/rest_ack.rs` (новый, 9 тестов) — оракульное тело
`{"token":null,"last_viewed":4310}` (JSON-равенство + набор ключей = `body_keys` каталога),
target из каталога, `null`/не-null токен, `last_viewed = 0` и `4310` (число, не id, не строка),
200 → Ok, 401/403/404/429/400/500/503, сетевой/клиентский отказ транспорта, отсечение id до
транспорта (пустая очередь = паника, если бы вызвали), `Debug` без bearer, трейт-объект,
source-проверка Tauri-команды (bearer через `token_or`, `token: Option<String>`, без логов).

`rust/gui-spike/tests/messages_contract.rs` (аддитивно, +2 теста): делегирование
`ack_message` (счётчик и nullable токен дословно), сохранение типизированных ошибок;
`tauri_registers_message_action_commands` расширен на `ack_message`.

## RED (до продуктового кода)

`waves/RW-40/PM/m07/red.log` — тесты написаны раньше модуля:

```
error[E0432]: unresolved imports `discord_gui_spike::messages::ack_message`,
`discord_gui_spike::messages::AckTransport`, `discord_gui_spike::messages::DiscordReadState`,
`discord_gui_spike::messages::ReadStateAck`
 --> tests/rest_ack.rs:12:5
error: could not compile `discord-gui-spike` (test "rest_ack") due to 1 previous error
```

`cargo test` завершился ошибкой компиляции (exit≠0). Продуктовый код после red не «зеленил»
тесты правкой ожиданий: ни один assert не ослаблен.

## GREEN

`waves/RW-40/PM/m07/green-gui.log`, `green-core.log`, `clippy.log`, `fmt.log`.

- `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` — **202 passed / 0 failed**
  (16 бинарников + doc-tests). База 191 → +11 (9 `rest_ack` + 2 `messages_contract`).
  `rest_ack`: 9/9, `messages_contract`: 12/12.
- `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` — **574 passed / 0 failed**
  (без изменений: ядро не трогалось). Примечание: без `--manifest-path` из корня репозитория
  cargo не находит workspace (`Cargo.toml` в `rust/`).
- `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings`
  — чисто (`Finished`), после починки `clippy::type_complexity` в тесте (алиас `AckCall`).
- `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` — чисто (exit 0).

## Файлы + md5

```
8da412cf1a629ac03636d61c5e58be3d  rust/gui-spike/src/messages.rs
5d7f09810daf7540d9eae0bcc6b80b5e  rust/gui-spike/src/main.rs
fe95d3c0bb66047226898ae648c6c16b  rust/gui-spike/tests/rest_ack.rs      (new)
7eff268068a29dfadbf7da2c5db6d353  rust/gui-spike/tests/messages_contract.rs
```

`src/lib.rs`, `routes.rs`, UI и прочие модули не трогались (`cargo fmt` затронул только два моих
файла — mtime остальных раньше времени форматирования).

## Остаток / вне scope

- **UI-триггер** (открытие канала/скролл/фокус), троттлинг и сброс unread-полосы — G2
  (`ui/features/read-state.js`), политика триггера `[A]` → `CONTRACTS/read-state.md`; backend
  готов, `last_viewed` берётся из `READY.read_state`/`MESSAGE_ACK`.
- **Live-паритет** ack (200 на реальном Discord) не проверялся — окно/live запрещены ТЗ.
- `429`: `retry_after_ms` для этого маршрута не подтверждён захватом → 0 (не выдумываем);
  из `ApiError::RateLimited` значение сохраняется.
- В общем дереве параллельно идёт G2/G-01: `npm run test:unit` сейчас 481 тестов, 7 красных
  (рейл серверов, `loading`/`aria-current`) — файлы UI не мои, к M-07 отношения не имеют;
  unit-набор я не запускал как гейт и не правил.
- Коммит не делался (запрещено).
