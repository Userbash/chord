# Отчёт воркера G1 Rust Backend: `RW-40` / `INT-04-ATTACH-PUT-BACKEND-PASS2`

- **Batch / карточка**: `RW-40` / `INT-04-ATTACH-PUT-BACKEND-PASS2` (устранение `P1` из `waves/RW-40/PM/b2b-attach-ui/REVIEW-BE-PUT.md`: доказательство извлечения плоского `putPayload` из `composer.js:744-755` с полем `id` и индексным `Uint8Array` на уровне Tauri command extraction с доведением байтов до HTTP PUT)
- **Роль**: Worker G1 Rust Backend (`INT-04-ATTACH-PUT-BACKEND-PASS2`)
- **Дата**: 2026-10-30

## 1. Сделано (`done`)

1. **`rust/gui-spike/src/attachments.rs`**:
   - Добавлена константа `UPLOAD_PUT_COMMAND_PARAM_KEYS: [(&str, &str); 8]` — канонический список параметров `#[tauri::command] async fn attachments_upload_put` в `main.rs` (кроме `backend: tauri::State`) и их `camelCase`-ключей в `InvokeBody::Json` (`payload`, `slotId`, `id`, `channelId`, `uploadUrl`, `contentType`, `data`, `bytes`).
   - Выделена структура `UploadPutCommandArgs` (`#[serde(rename_all = "camelCase")]`), поля которой взаимно-однозначно соответствуют параметрам `#[tauri::command] async fn attachments_upload_put`:
     - метод `UploadPutCommandArgs::from_tauri_ipc(&serde_json::Value)` поключево десериализует каждый параметр `Option<T>` из верхнеуровневого JSON-объекта `invoke("attachments_upload_put", putPayload)` по точному правилу `tauri::ipc::CommandItem::deserialize_option`;
     - метод `UploadPutCommandArgs::into_input(self)` объединяет `self.slot_id.or(self.id)` и вызывает `UploadPutInput::from_ipc`, сохраняя и валидируя идентификатор слота как при передаче `slotId`, так и при передаче плоского `id` из `composer.js`.
   - Добавлена функция `upload_put_command<S: AttachmentTransport + ?Sized>(sender: &S, args: UploadPutCommandArgs) -> Result<(), AttachmentError>`.

2. **`rust/gui-spike/src/lib.rs`**:
   - Добавлена публичная IPC-обёртка `pub async fn attachments_upload_put_command<S: attachments::AttachmentTransport + ?Sized>(sender: &S, args: attachments::UploadPutCommandArgs) -> Result<(), attachments::AttachmentError>`.

3. **`rust/gui-spike/src/main.rs`**:
   - В `#[tauri::command] async fn attachments_upload_put` добавлен параметр `id: Option<String>` (строго без запрещённых сегментов `file`/`path`/`dir`/`root`/`location` из `tests/meta/ipc-path.test.mjs`), чтобы Tauri command extractor извлекал поле `id` из плоского `putPayload` (`composer.js:744-755`) наряду со `slot_id` (`slot_id.or(id)`).
   - Выделена внутренняя функция `async fn execute_attachments_upload_put<S: AttachmentTransport + ?Sized>(api: &S, args: UploadPutCommandArgs) -> Result<(), AttachmentError>`, вызывающая `UploadPutInput::from_ipc(args.payload, args.slot_id.or(args.id), ...)` и `discord_gui_spike::attachments_upload_put(api, &input.upload_url, &input.content_type, input.payload_bytes()).await`.
   - В `mod wiring_tests` добавлен тест `attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes`, который:
     - сверяет реальную сигнатуру `#[tauri::command] async fn attachments_upload_put` в `main.rs` с `UPLOAD_PUT_COMMAND_PARAM_KEYS` (включая `id: Option<String>`);
     - десериализует точный плоский JSON `putPayload` из `composer.js:744-755` (`{channelId, id, uploadUrl, uploadFilename, fileSize, contentType, bytes: {"0": 137, "1": 80}, data: {"0": 137, "1": 80}}`) на уровне Tauri command extraction (`UploadPutCommandArgs::from_tauri_ipc` и `serde_json::from_value`);
     - вызывает `execute_attachments_upload_put(&transport, args)` с `FakePutTransport` и проверяет, что `upload_url`, `content_type = "image/png"` и сырые байты `[137, 80]` реально доходят до HTTP PUT, а пробельный `id: "   "` отклоняется с `Err(AttachmentError::InvalidArg)` до обращения к транспорту.

4. **`rust/gui-spike/tests/attachments_contract.rs`**:
   - Добавлен контрактный тест `tauri_command_extracts_flat_composer_put_payload_with_id_and_uint8array_into_http_put`, проверяющий фильтрацию плоского `putPayload` строго по объявленным параметрам `#[tauri::command] async fn attachments_upload_put` из `src/main.rs`, извлечение `id: Some("slot-1")` и `UploadBytes([137, 80])` из индексного объекта `Uint8Array` (`{"0": 137, "1": 80}`), сохранение `slot_id = Some("slot-1")` через `slot_id.or(id)`, доставку `[137, 80]` и `"image/png"` в HTTP PUT через `attachments_upload_put_command` / `upload_put_command` и отклонение пробельного `id` до сети.

## 2. Независимый оракул (`oracle`)

- **`rust/gui-spike/ui/features/composer.js:744-761`**:
  - Форма плоского объекта `putPayload` при вызове `invokeFn("attachments_upload_put", putPayload)`:
    `{ channelId, id: slot.id, uploadUrl: slot.upload_url, uploadFilename: slot.upload_filename, filename: item.name, fileSize: item.size, contentType: item.type, file: item.file ?? null, bytes: item.bytes ?? null, data: item.data ?? item.bytes ?? null }`.
- **Механизм Tauri v2 command extraction (`tauri-macros` `parse_arg` + `tauri::ipc::CommandItem::deserialize_option`)**:
  - Имя каждого параметра `#[tauri::command]` переводится в `lowerCamelCase` и ищется в верхнеуровневом JSON-объекте `InvokeBody::Json` (`v.get(self.key)`).
- **`waves/RW-40/RE-DEEP-REA.md` §1.4 и `tests/meta/ipc-path.test.mjs`**:
  - Передача `Content-Type` и сырых байтов в `PUT {upload_url}`; соблюдение PLAT-11 (отсутствие path/file-имён и типов в параметрах `#[tauri::command]`).

## 3. Red-прогон (`red`)

- Лог сохранён в: `waves/RW-40/PM/b2b-attach-ui/red-be-put-pass2.log`
- Команды:
  ```bash
  (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo test --offline -p discord-gui-spike --test attachments_contract)
  (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo test --offline -p discord-gui-spike --test attachments_contract --bins)
  ```
- Результат RED:
  1. **Stage 1 (неизменённые `src/attachments.rs`, `src/lib.rs`, `src/main.rs` из PASS1 без параметра `id`)**:
     - `EXIT_CODE_STAGE1=101`, `test result: FAILED. 22 passed; 1 failed` — упал `tauri_command_extracts_flat_composer_put_payload_with_id_and_uint8array_into_http_put` (`#[tauri::command] async fn attachments_upload_put обязан принимать параметр id: Option<String> для плоского putPayload из composer.js; текущие параметры: [("payload", "payload"), ("slot_id", "slotId"), ("channel_id", "channelId"), ("upload_url", "uploadUrl"), ("content_type", "contentType"), ("data", "data"), ("bytes", "bytes")]`).
  2. **Stage 2 (compile-time RED нового контракта `UploadPutCommandArgs` / `attachments_upload_put_command` до правок `src/`)**:
     - `EXIT_CODE_STAGE2=101` (`error[E0432]: unresolved imports upload_put_command, UploadPutCommandArgs, UPLOAD_PUT_COMMAND_PARAM_KEYS, attachments_upload_put_command`).
  3. **Stage 3 (runtime RED в `main.rs` `wiring_tests` и `attachments_contract` на старом `main.rs` без `id` и без `slot_id.or(id)`)**:
     - `EXIT_CODE=101`, `test result: FAILED. 9 passed; 1 failed` — упал `wiring_tests::attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes`.

## 4. Green-прогон (`green`) и `targeted`-проверки

- Лог сохранён в: `waves/RW-40/PM/b2b-attach-ui/green-be-put-pass2.log`
- Команды и результаты:
  1. Контрактные и unit-тесты (`attachments_contract`, `ipc_wiring_contract`, `--lib`, `--bins`):
     ```bash
     (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo test --offline -p discord-gui-spike --test attachments_contract --test ipc_wiring_contract --lib --bins)
     ```
     - `src/lib.rs`: `8 passed; 0 failed`
     - `src/main.rs` (`wiring_tests`): `10 passed; 0 failed`
     - `tests/attachments_contract.rs`: `23 passed; 0 failed`
     - `tests/ipc_wiring_contract.rs`: `5 passed; 0 failed`
     - `EXIT_CODE_CARGO_TEST=0`
  2. Мета-тест безопасности сигнатур IPC (`tests/meta/ipc-path.test.mjs`):
     ```bash
     node --test tests/meta/ipc-path.test.mjs
     ```
     - `pass 12, fail 0` (`EXIT_CODE_IPC_PATH=0`)
  3. Линтер `clippy` и форматирование `rustfmt`:
     ```bash
     (cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-int01 cargo clippy --offline -p discord-gui-spike --lib --bins --test attachments_contract --test ipc_wiring_contract -- -D warnings)
     rustfmt --edition 2021 --check rust/gui-spike/src/attachments.rs rust/gui-spike/src/lib.rs rust/gui-spike/src/main.rs rust/gui-spike/tests/attachments_contract.rs
     ```
     - Оба завершились с `EXIT_CODE=0` (0 предупреждений).

## 5. Мутационное тестирование (`mutations`)

- Полный лог 5 мутаций и восстановления md5: `waves/RW-40/PM/b2b-attach-ui/mutations-be-put-pass2.log`
- Сводка (5/5 мутантов убиты):

| # | Файл и место мутации | Суть мутации | Убивший тест | Код выхода |
|---|---|---|---|---|
| M1 | `rust/gui-spike/src/main.rs` (`#[tauri::command] async fn attachments_upload_put`) | Удалён параметр `id: Option<String>` из сигнатуры `attachments_upload_put` (`md5=d68129534da1823b5e109fd8cb701b6c`) | `wiring_tests::attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes` | `101` (KILLED) |
| M2 | `rust/gui-spike/src/main.rs` (`execute_attachments_upload_put`) | Заменён `args.slot_id.or(args.id)` на `args.slot_id` (потеря плоского поля `id` при вызове команды, `md5=bf08a05d198c4d9726881da875ed41cb`) | `wiring_tests::attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes` | `101` (KILLED) |
| M3 | `rust/gui-spike/src/attachments.rs` (`UploadPutCommandArgs::into_input`) | Заменён `self.slot_id.or(self.id)` на `self.slot_id` (`md5=773edba93ab3d0b5a4d2c7a495c2d945`) | `tauri_command_extracts_flat_composer_put_payload_with_id_and_uint8array_into_http_put` | `101` (KILLED) |
| M4 | `rust/gui-spike/src/attachments.rs` (`UploadPutCommandArgs::from_tauri_ipc`) | Отключено извлечение `data`/`bytes` (`data: None, bytes: None`, `md5=970096a1496a35e9c9fb6a8685c4f976`) | `wiring_tests::attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes` | `101` (KILLED) |
| M5 | `rust/gui-spike/src/main.rs` (`execute_attachments_upload_put`) | Подменены аргументы `(&input.content_type, input.payload_bytes())` на `("", &[0])` при вызове `discord_gui_spike::attachments_upload_put` (`md5=43aa45f9341e91dd798caaa3ebdc6388`) | `wiring_tests::attachments_upload_put_command_extracts_flat_composer_payload_and_sends_put_bytes` | `101` (KILLED) |

После всех 5 мутаций файлы восстановлены до базовых md5, повторный green-прогон (`cargo test`, `cargo clippy -- -D warnings`, `rustfmt --check`, `node --test tests/meta/ipc-path.test.mjs`) прошёл с кодом `0`.

## 6. Затронутые файлы и контрольные суммы (`files+md5`)

Назначенные кодовые и тестовые файлы:
- `75f50c624ab5a6b21a67281235f4b289  rust/gui-spike/src/attachments.rs`
- `bc38ac5943b929e7a620ef911cb208a5  rust/gui-spike/src/lib.rs`
- `8a71f311bf0ab47ff24450eaf389b2d4  rust/gui-spike/src/main.rs`
- `77157aadf4267ded8c225d8891c71a9f  rust/gui-spike/tests/attachments_contract.rs`

Артефакты волны в `waves/RW-40/PM/b2b-attach-ui/`:
- `waves/RW-40/PM/b2b-attach-ui/red-be-put-pass2.log`
- `waves/RW-40/PM/b2b-attach-ui/green-be-put-pass2.log`
- `waves/RW-40/PM/b2b-attach-ui/mutations-be-put-pass2.log`
- `waves/RW-40/PM/b2b-attach-ui/REPORT-BE-PUT-PASS2.md`

## 7. Риски и замечания (`risks`)

- **Совместимость обоих форматов IPC**: команда `attachments_upload_put` теперь принимает как плоский `putPayload` из `composer.js` с полем `id` (`slot_id.or(id)`), так и `slotId` или вложенный `payload: UploadPutInput`.
- **Соблюдение PLAT-11 (`tests/meta/ipc-path.test.mjs`)**: ни одно имя параметра `#[tauri::command] async fn attachments_upload_put` (`backend`, `payload`, `slot_id`, `id`, `channel_id`, `upload_url`, `content_type`, `data`, `bytes`) не содержит запрещённых сегментов (`file`, `path`, `filepath`, `filename`, `dir`, `dirname`, `folder`, `root`, `location`).
