# Отчёт воркера G1 Rust Backend: `RW-40` / `INT-04-ATTACH-PUT-BACKEND`

- **Batch / карточка**: `RW-40` / `INT-04-ATTACH-PUT-BACKEND` (устранение `P1` из `waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS3.md`: реализация Rust-бэкенда `upload_attachment_bytes` / `attachments_upload_put` и регистрация `#[tauri::command] async fn attachments_upload_put` в `rust/gui-spike/src/main.rs`)
- **Роль**: Worker G1 Rust Backend (`INT-04-ATTACH-PUT-BACKEND`)
- **Дата**: 2026-10-30

## 1. Сделано (`done`)

1. **`rust/gui-spike/tests/attachments_contract.rs`** (RED → GREEN):
   - Добавлены 5 контрактных тестов для PUT-загрузки байтов вложения и Tauri IPC wiring:
     1. `uploads_attachment_bytes_via_put_with_content_type_and_accepts_200_201_204`:
        - проверяет прямую отправку непустых байтов методом `PUT` на pre-signed `upload_url` (`https://...`) с передачей `Content-Type` (а также нормализацию пустого `content_type` в `application/octet-stream`);
        - проверяет делегирование публичной обёртки `discord_gui_spike::attachments_upload_put` и метода `DiscordAttachments::upload_attachment_bytes`;
        - проверяет маппинг статусов `200`, `201`, `204` в `Ok(())`.
     2. `rejects_invalid_or_insecure_upload_url_and_empty_bytes_before_io`:
        - проверяет локальное отклонение (`Err(AttachmentError::InvalidArg)`) до обращения к сети (`transport.put_calls().is_empty()`) для пустого/пробельного URL, небезопасного `http://...`, относительного пути, `ftp://`, URL без хоста, URL с `#`, пробелами или `user@host`;
        - проверяет отклонение пустого буфера байтов `&[]` (`InvalidArg`), управляющих символов `\r`/`\n` в `content_type` (`InvalidArg`) и превышения лимита размера по `GuildBoostTier` (`EntityTooLarge`) до обращения к сети.
     3. `maps_put_status_codes_413_429_5xx_and_network_errors`:
        - проверяет маппинг статусов ответа `PUT`: `413` → `AttachmentError::EntityTooLarge`, `400` с `{"code": 40005}` → `AttachmentError::EntityTooLarge`, `429` с заголовком `Retry-After` → `AttachmentError::RateLimited { retry_after_ms }`, `500`/`502`/`503` → `AttachmentError::Server { status }`, `401` → `Unauthorized`, `403` → `Forbidden`, `404` → `NotFound`, `400` → `BadRequest`, сбой транспорта → `AttachmentError::Network`.
     4. `upload_put_input_deserializes_uint8array_and_vec_and_validates_ipc_payload`:
        - проверяет десериализацию `UploadPutInput` и `UploadBytes` как из стандартного JSON-массива `[u8]`, так и из индексного объекта `{"0": 10, "1": 20, ...}` (результат `JSON.stringify(new Uint8Array(...))` во фронтенде `composer.js`), а также прямой вызов `UploadPutInput::from_ipc` с плоскими аргументами и вложенным `payload`;
        - проверяет отклонение нечислового `channel_id`, пустого `slot_id`, неконсистентных ключей/невалидных байтов `> 255` в `UploadBytes` и пустых байтов до сети.
     5. `tauri_main_registers_and_wires_attachments_upload_put_command`:
        - проверяет наличие `#[tauri::command] async fn attachments_upload_put` в `rust/gui-spike/src/main.rs`, регистрацию `attachments_upload_put` в `tauri::generate_handler![...]`, включение в `TOKEN_COMMANDS`, чтение токена через `token_or::<AttachmentError, _>`, делегирование в `discord_gui_spike::attachments_upload_put` и отсутствие запрещённых path-имён/типов из `tests/meta/ipc-path.test.mjs`.

2. **`rust/gui-spike/src/attachments.rs`**:
   - Добавлена константа `pub const DEFAULT_UPLOAD_CONTENT_TYPE: &str = "application/octet-stream";`.
   - Расширен трейт `AttachmentTransport` методом `put(&self, upload_url: &str, content_type: &str, data: &[u8])` (с дефолтной реализацией, сохраняющей обратную совместимость `FakeAttachmentTransport` в read-only `tests/ipc_wiring_contract.rs`).
   - В `ApiAttachmentTransport::put` реализована отправка `HttpRequest::new(Method::Put, target).header("content-type", content_type).body_bytes(data.to_vec())` через `HttpClient::for_endpoint` на разобранный `https://` endpoint без утечки Discord Bearer-токена на внешний GCS `upload_url`.
   - Добавлены `UploadBytes` и `UploadPutInput` (`from_ipc`, `payload_bytes`) для приёма IPC-вызова `invoke("attachments_upload_put", ...)` из `composer.js` (поддержка плоских аргументов и вложенного `payload`, `Vec<u8>` и `Uint8Array`-объекта `{"0": u8, ...}`).
   - Добавлены функции `upload_attachment_bytes`, `upload_attachment_bytes_with_tier`, `upload_put_input`, `DiscordAttachments::upload_attachment_bytes`, а также валидаторы `parse_https_upload_url`, `normalize_content_type` и декодер статусов `decode_put`.

3. **`rust/gui-spike/src/lib.rs`**:
   - Добавлена публичная обёртка `pub async fn attachments_upload_put<S: attachments::AttachmentTransport + ?Sized>(sender: &S, upload_url: &str, content_type: &str, data: &[u8]) -> Result<(), attachments::AttachmentError>`.

4. **`rust/gui-spike/src/main.rs`**:
   - Добавлена команда `#[tauri::command] async fn attachments_upload_put` с параметрами `slot_id`, `channel_id`, `upload_url`, `content_type`, `data`, `bytes`, `payload` (строго соблюдает `tests/meta/ipc-path.test.mjs` — ни одного запрещённого сегмента пути `path`/`file`/`dir`/`root`/`location` и ни одного файлового типа).
   - Команда зарегистрирована в `tauri::generate_handler![...]` и добавлена в `TOKEN_COMMANDS` (`[&str; 19]`) в `mod wiring_tests`.

## 2. Независимый оракул (`oracle`)

- **`waves/RW-40/reference-oracle-REA.md` §1.1–§1.7 (`CloudUploader.upload` / двухфазная загрузка вложений)**:
  - §1.4: шаг 2 двухфазного протокола — `PUT {upload_url}` с заголовком `Content-Type: file.type || "application/octet-stream"` и сырыми байтами файла в теле запроса;
  - §1.1: лимиты размера по `GuildBoostTier` (`None = 25 MiB = 26_214_400`, `Tier2 = 50 MiB = 52_428_800`, `Tier3 = 100 MiB = 104_857_600`);
  - §1.6: классификация ошибок (`413` и `400 + code=40005` → `EntityTooLarge`, `429` → `RateLimited { retry_after_ms }` из заголовка `retry-after`, `5xx` → `Server { status }`).
- **`waves/RW-40/ATTACH-RE.md` §2**:
  - Спецификация шага 2 (`PUT <upload_url>` на pre-signed `https://` URL из `AttachmentUploadSlot` до вызова `POST /channels/{id}/messages`).
- **`waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS3.md` (`P1`) и `tests/meta/ipc-path.test.mjs`**:
  - Соответствие IPC-вызову `invoke("attachments_upload_put", ...)` из `rust/gui-spike/ui/features/composer.js:750-758` и правилам безопасности сигнатур IPC-команд (`tests/meta/ipc-path.test.mjs`).

## 3. Red-прогон (`red`)

- Лог сохранён в: `waves/RW-40/PM/b2b-attach-ui/red-be-put.log`
- Команда:
  ```bash
  (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo test --offline -p discord-gui-spike --test attachments_contract)
  ```
- Результат RED:
  1. До правок `src/` (compile-time RED): `EXIT_CODE=101` (`error[E0432]: unresolved imports UploadBytes, UploadPutInput, upload_attachment_bytes, upload_attachment_bytes_with_tier, upload_put_input, DEFAULT_UPLOAD_CONTENT_TYPE`, `error[E0407]: method put is not a member of trait AttachmentTransport`, `error[E0599]: cannot find function attachments_upload_put in crate discord_gui_spike`).
  2. С noop-заглушками в `src/` до реализации логики (runtime assertion RED): `EXIT_CODE_RUNTIME_RED=101`, `test result: FAILED. 17 passed; 5 failed` — упали ровно все 5 новых контрактных тестов:
     - `maps_put_status_codes_413_429_5xx_and_network_errors`
     - `rejects_invalid_or_insecure_upload_url_and_empty_bytes_before_io`
     - `tauri_main_registers_and_wires_attachments_upload_put_command`
     - `upload_put_input_deserializes_uint8array_and_vec_and_validates_ipc_payload`
     - `uploads_attachment_bytes_via_put_with_content_type_and_accepts_200_201_204`

## 4. Green-прогон (`green`) и `targeted`-проверки

- Лог сохранён в: `waves/RW-40/PM/b2b-attach-ui/green-be-put.log`
- Команды и результаты:
  1. Контрактные и unit-тесты (`attachments_contract`, `ipc_wiring_contract`, `--lib`, `--bins`):
     ```bash
     (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo test --offline -p discord-gui-spike --test attachments_contract --test ipc_wiring_contract --lib --bins)
     ```
     - `src/lib.rs`: `8 passed; 0 failed`
     - `src/main.rs` (`wiring_tests`): `9 passed; 0 failed`
     - `tests/attachments_contract.rs`: `22 passed; 0 failed`
     - `tests/ipc_wiring_contract.rs`: `5 passed; 0 failed`
     - `EXIT_CODE=0`
  2. Мета-тест безопасности сигнатур IPC (`tests/meta/ipc-path.test.mjs`):
     ```bash
     node --test tests/meta/ipc-path.test.mjs
     ```
     - `pass 12, fail 0` (`EXIT_CODE=0`)
  3. Линтер `clippy` и форматирование `rustfmt`:
     ```bash
     (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo clippy --offline -p discord-gui-spike --lib --bins --test attachments_contract --test ipc_wiring_contract -- -D warnings)
     rustfmt --edition 2021 --check rust/gui-spike/src/attachments.rs rust/gui-spike/src/lib.rs rust/gui-spike/src/main.rs rust/gui-spike/tests/attachments_contract.rs
     ```
     - Оба завершились с `EXIT_CODE=0` (0 предупреждений).

## 5. Мутационное тестирование (`mutations`)

- Полный лог 5 мутаций и восстановления md5: `waves/RW-40/PM/b2b-attach-ui/mutations-be-put.log`
- Сводка (5/5 мутантов убиты):

| # | Файл и место мутации | Суть мутации | Убивший тест | Код выхода |
|---|---|---|---|---|
| M1 | `rust/gui-spike/src/attachments.rs` (`parse_https_upload_url`) | Разрешена схема `http://` наряду с `https://` (`md5=da1a6f62a463e0514f24ae7d0116d264`) | `rejects_invalid_or_insecure_upload_url_and_empty_bytes_before_io` | `101` (KILLED) |
| M2 | `rust/gui-spike/src/attachments.rs` (`upload_attachment_bytes_with_tier`) | Удалена проверка `if data.is_empty() { return Err(AttachmentError::InvalidArg); }` (`md5=788d55cc34a865c25d367758d84846a9`) | `rejects_invalid_or_insecure_upload_url_and_empty_bytes_before_io` | `101` (KILLED) |
| M3 | `rust/gui-spike/src/attachments.rs` (`upload_attachment_bytes_with_tier`) | Подменены аргументы `sender.put(trimmed_url, DEFAULT_UPLOAD_CONTENT_TYPE, &[])` вместо `(trimmed_url, mime, data)` (`md5=dcc00db6c9a329ad0dea0f16c7296161`) | `uploads_attachment_bytes_via_put_with_content_type_and_accepts_200_201_204`, `upload_put_input_deserializes_uint8array_and_vec_and_validates_ipc_payload` | `101` (KILLED) |
| M4 | `rust/gui-spike/src/attachments.rs` (`decode_put`) | Статус `413` смикширован в `Ok(())` вместо `Err(AttachmentError::EntityTooLarge)` (`md5=2242590308f98489109f0e7a3adf5100`) | `maps_put_status_codes_413_429_5xx_and_network_errors` | `101` (KILLED) |
| M5 | `rust/gui-spike/src/main.rs` (`tauri::generate_handler!`) | Исключена регистрация `attachments_upload_put` из `generate_handler!` (`md5=1cce0298f8329de96639846e3f67934e`) | `tauri_main_registers_and_wires_attachments_upload_put_command` | `101` (KILLED) |

После всех 5 мутаций файлы восстановлены до базовых md5, повторный green-прогон (`cargo test`, `cargo clippy -- -D warnings`, `rustfmt --check`, `node --test tests/meta/ipc-path.test.mjs`) прошёл с кодом `0`.

## 6. Затронутые файлы и контрольные суммы (`files+md5`)

Назначенные кодовые и тестовые файлы:
- `84d8905b20a67c46a7908c9ac103560a  rust/gui-spike/src/attachments.rs`
- `48d01ef823c7babff52614ad95b0e60d  rust/gui-spike/src/lib.rs`
- `dc1d395fd083020f5a7d5e038925cd1d  rust/gui-spike/src/main.rs`
- `d1113587c21f7a91e4e0f00dc4e0c435  rust/gui-spike/tests/attachments_contract.rs`

Артефакты волны в `waves/RW-40/PM/b2b-attach-ui/`:
- `waves/RW-40/PM/b2b-attach-ui/red-be-put.log`
- `waves/RW-40/PM/b2b-attach-ui/green-be-put.log`
- `waves/RW-40/PM/b2b-attach-ui/mutations-be-put.log`
- `waves/RW-40/PM/b2b-attach-ui/REPORT-BE-PUT.md`

## 7. Риски и замечания (`risks`)

- **Формат передачи `Uint8Array` через Tauri IPC**: во фронтенде (`composer.js`) `Uint8Array` при JSON-сериализации превращается в объект с индексными ключами `{"0": b0, "1": b1, ...}` либо передаётся как массив `[u8]`. Десериализатор `UploadBytes` в `attachments.rs` прозрачно принимает оба представления и отклоняет непоследовательные индексы или числа вне `0..=255`.
- **Изоляция Bearer-токена**: в `ApiAttachmentTransport::put` создаётся отдельный `HttpClient::for_endpoint` для хоста `upload_url` без заголовка `Authorization`, поэтому Discord Bearer-токен никогда не отправляется на внешний pre-signed GCS URL.
- **Совместимость с `tests/ipc_wiring_contract.rs`**: файл `ipc_wiring_contract.rs` вне списка назначенных файлов не изменялся; все 5 его тестов зелены.
