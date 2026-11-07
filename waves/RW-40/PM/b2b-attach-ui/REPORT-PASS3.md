# B2b-ATTACH-UI-FIX-PASS2 — отчёт воркера (Pass 3: закрытие P0 и P1 из REVIEW-PASS2)

- **batch/карточка:** RW-40 / B2b-ATTACH-UI-FIX-PASS2 (Воркер G2 Frontend Lane A; устранение замечаний из `waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS2.md` в пределах назначенных файлов `rust/gui-spike/ui/features/composer.js` и `tests/unit/composer.test.mjs`).
- **сделано:**
  - **P0 (`@ts-ignore` в `composer.js`):** Удалены все 5 директив `// @ts-ignore` из `rust/gui-spike/ui/features/composer.js` (бывшие строки 593, 659, 672, 868, 881). Узлы `.attachment-progress-fill` и `firstFocusable` в модалке подарка типизированы через `/** @type {HTMLElement | null} */`. Проверка `grep -c '@ts-ignore' rust/gui-spike/ui/features/composer.js tests/unit/composer.test.mjs` даёт `0` для обоих файлов; `npx tsc -p tsconfig.json` и `npx tsc -p rust/gui-spike/ui/tsconfig.json` проходят с кодом `0`.
  - **P1 (удержание `File`/`bytes` в очереди и передача в `attachments_upload_put`):** В `addFiles` элемент очереди `AttachmentItem` сохраняет ссылку на исходный объект `file: f` и прочитанные байты `bytes` / `data` (`Uint8Array | null`, извлекаемые через `extractFileBytes` из `File.arrayBuffer()` или полей `bytes`/`data`). В `startUpload` вызов `invoke("attachments_upload_put", putPayload)` и `options.uploadPut(putPayload)` получает `file`, `bytes`, `data` и `contentType` наряду с метаданными слота (`channelId`, `id`, `uploadUrl`, `uploadFilename`, `filename`, `fileSize`), в том числе при повторной отправке (`attachment-retry`) после сетевой ошибки.
  - **Strict TDD:** Сначала в `tests/unit/composer.test.mjs` добавлены проверки сохранения `file`/`bytes`/`data` в очереди `composer.attachments`, передачи `File` и `Uint8Array` в `attachments_upload_put` и `options.uploadPut`, а также сохранности `File` и `bytes` при повторе после `error-network`. Зафиксирован красный прогон в `waves/RW-40/PM/b2b-attach-ui/red-pass3.log` (exit `1`, 24 pass, 4 fail), затем внесён минимальный код в `composer.js` и записан зелёный прогон в `waves/RW-40/PM/b2b-attach-ui/green-pass3.log` (exit `0`, 28 pass, 0 fail).
- **red:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/red-pass3.log 2>&1
  ```
  Код выхода: `1` (24 pass, 4 fail).
  Лог: `waves/RW-40/PM/b2b-attach-ui/red-pass3.log`.
- **green:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/green-pass3.log 2>&1
  npx tsc -p tsconfig.json
  npx tsc -p rust/gui-spike/ui/tsconfig.json
  ```
  Код выхода тестов: `0` (28 pass, 0 fail; ~63 мс).
  Код выхода TypeScript: `0` ошибок в обоих `tsconfig.json`.
  Лог: `waves/RW-40/PM/b2b-attach-ui/green-pass3.log`.
- **targeted:**
  - `tests/unit/composer.test.mjs`: 28/28 pass.
  - Регрессионный прогон (`tests/unit/gui-spike-dom.test.mjs`, `tests/unit/gui-spike-features.test.mjs`, `tests/unit/gui-spike-regressions.test.mjs`): 151/151 pass.
- **oracle:**
  - `waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS2.md` (P0 по `@ts-ignore` в `composer.js`, P1 по удержанию `File`/байтов в `addFiles` и передаче в `attachments_upload_put`).
  - `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §3.1, §3.4, §3.5.
  - `rust/gui-spike/src/attachments.rs` (`MAX_UPLOAD_FILE_BYTES = 10485760`, `MAX_ATTACHMENTS_PER_MESSAGE = 10`).
- **мутации:** 5 мутаций зафиксированы в `waves/RW-40/PM/b2b-attach-ui/mutations-pass3.log`:
  1. `M1-DROP-QUEUE-FILE-REF` (`file: null` вместо `file: f` в `addFiles`): md5 `483edc6d5b8fd25f28316b37998737e9` → exit `1`, `CAUGHT` (4 теста).
  2. `M2-DROP-PUT-BYTES` (`bytes: null, data: null` в `putPayload` внутри `startUpload`): md5 `631e5e06f613a50819a739779b11ba1e` → exit `1`, `CAUGHT` (3 теста).
  3. `M3-SKIP-FILE-ARRAYBUFFER` (отключение чтения `f.arrayBuffer()` в `extractFileBytes`): md5 `afbb1a674feb53e67d2484677a90c4d9` → exit `1`, `CAUGHT` (3 теста).
  4. `M4-SKIP-PUT` (пропуск вызова `attachments_upload_put` / `options.uploadPut` перед `ready`): md5 `f569ef906e166706c7d23c96bc46aeee` → exit `1`, `CAUGHT` (6 тестов).
  5. `M5-NO-BLOCK-SEND-ON-ERROR` (разрешение отправки при `attachState === "error"`): md5 `4f16bc925a425605bde3256887ac6be0` → exit `1`, `CAUGHT` (5 тестов).
  Восстановление `composer.js` после каждого мутанта проверено по md5 (`9d77bde5a644aa7f2ca4f6c99f76aadb`).
- **gate:** В рамках параллельного batch выполнены targeted-тесты (28/28), регрессионные unit-тесты GUI-спайка (151/151) и обе проверки `tsc` (exit `0`). Полный `gate:worker` и `cargo` не запускались согласно правилам параллельного batch.
- **файлы и md5:**
  - `rust/gui-spike/ui/features/composer.js`: `9d77bde5a644aa7f2ca4f6c99f76aadb`
  - `tests/unit/composer.test.mjs`: `af978d589b8e81122b759f5d721acb32`
  - `waves/RW-40/PM/b2b-attach-ui/red-pass3.log`: `4b5eaaa92e19ca2f8b6bcbd271299b52`
  - `waves/RW-40/PM/b2b-attach-ui/green-pass3.log`: `a106e43df593f46618722c19a3c4e6a1`
  - `waves/RW-40/PM/b2b-attach-ui/mutations-pass3.log`: `1ff98120914182b2205c26e7402bfa2f`
- **риски/вопросы:**
  - Правки внесены строго в два назначенных файла (`rust/gui-spike/ui/features/composer.js` и `tests/unit/composer.test.mjs`). Регистрация Tauri-команд в `rust/gui-spike/src/main.rs` и проброс в `rust/gui-spike/ui/app.js` находятся вне файловой матрицы этой карточки и относятся к интеграционному лейну.
