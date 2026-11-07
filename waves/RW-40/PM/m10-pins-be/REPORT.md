# RW-40 / M-10-M-11-PINS-THREADS-BACKEND — пины (`M-10`) и ветки канала (`M-11`)

- **batch/карточка:** `RW-40 / M-10-M-11-PINS-THREADS-BACKEND` (`waves/RW-40/PARITY/PARITY-MATRIX.md` строки M-10 и M-11, `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` §2, §3);
- **роль:** Воркер G1 GUI-Spike Rust Backend (`CARGO_TARGET_DIR=/tmp/target-g1-pins`);
- **сделано:**
  1. В `rust/gui-spike/src/pins_threads.rs` реализованы структуры данных:
     - `PinnedMessage { id, channel_id, author, content, timestamp, pinned: bool }` (camelCase в IPC + `alias` для wire snake_case, разбор автора как из строкового поля, так и из user-объекта `{ global_name, username }`);
     - `ThreadSummary { id, parent_id, name, message_count, member_count, archived: bool, locked: bool, last_message_id: Option<String> }` (camelCase в IPC + `alias` для wire snake_case, извлечение `archived`/`locked` как с верхнего уровня, так и из вложенного `thread_metadata`).
  2. Реализованы трейт `PinsThreadsTransport`, клиент `DiscordPinsThreads` и функции/методы:
     - `list_channel_pins(&transport, channel_id)` -> `GET /channels/{channel_id}/pins` (`Vec<PinnedMessage>`);
     - `pin_channel_message(&transport, channel_id, message_id)` -> `PUT /channels/{channel_id}/pins/{message_id}` (`204 No Content` / `200 OK`);
     - `unpin_channel_message(&transport, channel_id, message_id)` -> `DELETE /channels/{channel_id}/pins/{message_id}` (`204 No Content` / `200 OK`);
     - `list_channel_threads(&transport, channel_id, archived: bool)` -> `GET /channels/{channel_id}/threads/active` (при `archived == false`) или `GET /channels/{channel_id}/threads/archived/public` (при `archived == true`) (`Vec<ThreadSummary>`).
  3. Добавлена предсетевая валидация `channel_id` и `message_id` по allowlist (`[A-Za-z0-9_-]`, `1..=64` байта, в паритете с `messages::send_message`) — при нарушении возвращается `PinsThreadsError::InvalidChannelId` / `PinsThreadsError::InvalidMessageId` до любого вызова транспорта.
  4. Реализован маппинг HTTP `429` с извлечением `retry-after` (из заголовка `Retry-After` или JSON-тела `{"retry_after": ...}`) в миллисекунды (`PinsThreadsError::RateLimited { retry_after_ms }`), а также `401 Unauthorized`, `403 Forbidden`, `404 NotFound`, `4xx Client`, `5xx Server`.
  5. Создан контрактный тест `rust/gui-spike/tests/pins_threads_contract.rs` (6 комплексных контрактных тестов). Файлы `src/lib.rs`, `src/main.rs`, `src/messages.rs` и прочие модули не изменялись.

- **red:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-pins cargo test --offline -p discord-gui-spike --test pins_threads_contract) > waves/RW-40/PM/m10-pins-be/red.log 2>&1` → `exit=101`, `test result: FAILED. 0 passed; 6 failed; 0 ignored` (первая ошибка: `assertion left == right failed: list_channel_pins bad_channel="" left: Err(Network) right: Err(InvalidChannelId)`); лог: `waves/RW-40/PM/m10-pins-be/red.log`.
- **green:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-pins cargo test --offline -p discord-gui-spike --test pins_threads_contract) > waves/RW-40/PM/m10-pins-be/green.log 2>&1` → `exit=0`, `test result: ok. 6 passed; 0 failed; 0 ignored`; лог: `waves/RW-40/PM/m10-pins-be/green.log`.
- **targeted:**
  - `CARGO_TARGET_DIR=/tmp/target-g1-pins cargo test --offline -p discord-gui-spike --test pins_threads_contract` → `6 passed; 0 failed` (`exit=0`);
  - `CARGO_TARGET_DIR=/tmp/target-g1-pins cargo clippy --offline -p discord-gui-spike --test pins_threads_contract -- -D warnings` → 0 предупреждений (`exit=0`);
  - `rustfmt --edition 2021 --check rust/gui-spike/src/pins_threads.rs rust/gui-spike/tests/pins_threads_contract.rs` → чисто (`exit=0`).
- **oracle:** `waves/RW-40/PARITY/PARITY-MATRIX.md` (M-10, M-11), `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` (§2, §3), `rust/gui-spike/src/messages.rs` (allowlist `channel_id` `1..=64` байт `[A-Za-z0-9_-]`).
- **мутации:** 5/5 `KILLED`, эквивалентных — 0, survivors — 0 (`waves/RW-40/PM/m10-pins-be/mutations.log`):
  - `MUT-1-allow-dot-in-id-allowlist` (md5 `5a486c2ca0ca7cf3e6d884a2f80255e6`) — разрешение `.` в allowlist `is_safe_id` → `KILLED` тестом `rejects_invalid_channel_id_and_message_id_before_any_transport_call_in_parity_with_messages` (`exit=101`);
  - `MUT-2-wrong-archived-threads-endpoint` (md5 `03ff2628572ccb22763d8d1b0ba1d335`) — подмена маршрута архивных веток на `/threads/active` → `KILLED` тестом `list_channel_threads_routes_active_and_archived_public_and_parses_thread_summaries` (`exit=101`);
  - `MUT-3-reject-204-on-pin-unpin-action` (md5 `a2b3410ec9fe2536b784e4521956c070`) — отказ на статус `204 No Content` в `decode_pin_action` → `KILLED` двумя тестами одновременно (`exit=101`):
    1. `pin_and_unpin_channel_message_use_put_and_delete_on_pins_endpoint_and_accept_204_or_200` (строка 302: `pin free fn HTTP 204` возвращает `Err(InvalidResponse)` вместо `Ok(())`);
    2. `rejects_invalid_channel_id_and_message_id_before_any_transport_call_in_parity_with_messages` (строки 525–556: для валидного ID `"1014537851418980375"` мок-транспорт возвращает HTTP `204` в `pin_channel_message`, поэтому при мутации `MUT-3` `pin_res.is_ok()` становится `false`, тогда как эталон `messages::send_message` возвращает `Ok` (`true`), что детерминированно роняет проверку паритета на строке 556).
  - `MUT-4-ignore-429-retry-after-ms` (md5 `0e3f1d6c57eb4a73f3f500e813299431`) — обнуление `retry_after_ms` при HTTP `429` → `KILLED` тестом `maps_429_retry_after_ms_and_401_403_404_errors_across_all_operations` (`exit=101`);
  - `MUT-5-return-invalid-channel-id-for-message-id` (md5 `bbaaecd9fad9b1aa4205d0b645438c11`) — возврат `InvalidChannelId` вместо `InvalidMessageId` при невалидном `message_id` → `KILLED` тестом `rejects_invalid_channel_id_and_message_id_before_any_transport_call_in_parity_with_messages` (`exit=101`).
- **baseline/restore:** baseline `rust/gui-spike/src/pins_threads.rs` = `839e78e1a62e585483076a329c9c0dbc`, `rust/gui-spike/tests/pins_threads_contract.rs` = `55c4f3344ba5f66c2600a59e9fd34ff0`; после каждого мутанта выполнен restore и проверено совпадение md5 (`match=True`).
- **gate:worker:** в режиме batch воркер выполняет targeted-проверки своего модуля с изолированным `CARGO_TARGET_DIR=/tmp/target-g1-pins`.

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/pins_threads.rs` | `839e78e1a62e585483076a329c9c0dbc` |
| `rust/gui-spike/tests/pins_threads_contract.rs` | `55c4f3344ba5f66c2600a59e9fd34ff0` |
| `waves/RW-40/PM/m10-pins-be/red.log` | `a93fed49ed1d775122dbf90c4e315541` |
| `waves/RW-40/PM/m10-pins-be/green.log` | `7dc8ca4828219523f027346fc0652eaf` |
| `waves/RW-40/PM/m10-pins-be/mutations.log` | `624f20716537fc9f71bd7ae0d00beaba` |

## Риски и примечания

1. **Файловая граница:** изменены исключительно `rust/gui-spike/src/pins_threads.rs` и `rust/gui-spike/tests/pins_threads_contract.rs`; общие файлы (`src/lib.rs`, `src/main.rs`, `src/messages.rs`) не затрагивались.
2. **Совместимость wire-ответов веток и пинов:** декодер пинов принимает поле `author` как в виде строки, так и в виде объекта пользователя Discord (`{ global_name, username }`), а декодер веток принимает как обёртку `{"threads": [...]}`, так и плоский массив `[...]`, с чтением `archived`/`locked` как из `thread_metadata`, так и с верхнего уровня объекта.
