# B2b-ATTACH-UI-FIX — отчёт воркера (закрытие P0 и P1)

- **batch/карточка:** RW-40 / B2b-ATTACH-UI-FIX (Frontend Lane A; устранение замечаний из `waves/RW-40/PM/b2b-attach-ui/REVIEW.md`).
- **сделано:**
  - **P0 (@ts-ignore):** В `tests/unit/composer.test.mjs` полностью удалены все 5 директив `@ts-ignore` (строки 32, 34, 316, 333, 335). Присваивание глобалов переведено на `Object.assign(globalThis, { document, window })`, кастомные свойства ошибок (`status`, `retry_after_ms`) типизированы через `Object.assign(new Error(...), { status, retry_after_ms })`. Команда `grep -c '@ts-ignore' tests/unit/composer.test.mjs` возвращает `0`, а `npx tsc -p tsconfig.json` проходит с кодом `0`.
  - **P1 (PUT на upload_url):** В `rust/gui-spike/ui/features/composer.js` (`startUpload`) убран преждевременный переход чипа в `ready` сразу после получения слотов. Удалён синтетический fallback `uploaded_filename`. Реализован обязательный вызов загрузки `invoke('attachments_upload_put', { channelId, id: slot.id, uploadUrl: slot.upload_url, uploadFilename: slot.upload_filename, filename: item.name, fileSize: item.size })` (или `options.uploadPut` при наличии). Перевод чипа в `ready` и фиксация `uploaded_filename` происходят **только** после успешного завершения PUT.
  - **P1 (обработка ошибок PUT и блокировка):** Если PUT падает с HTTP 413 / `EntityTooLarge` — чип переходит в `error-413`. Если PUT падает с HTTP 429 / `RateLimited` — чип переходит в `error-429`. При сетевом сбое/таймауте — чип переходит в `error-network`. Любая ошибка вложений выставляет композеру `data-attach-state="error"` и блокирует отправку сообщения. В `onKeydown` и `getUploadedAttachments` исключены файлы без подтверждённого `uploaded_filename`.
  - **P1 (мутационный прогон):** Проведены 5 реальных мутаций кода `composer.js`. Все 5 мутантов пойманы (`status=CAUGHT`, код выхода `1`), зафиксированы md5 мутантов, ключевые упавшие тесты и md5 восстановления в `waves/RW-40/PM/b2b-attach-ui/mutations.log`.
  - **Strict TDD:** Сначала добавлены red-тесты на вызов PUT, проверку `options.uploadPut`, падение PUT (413/429/сеть) и запрет fallback'ов при отсутствии слота. Зафиксирован `red-fix.log` (exit `1`, 6 упавших тестов), затем реализован минимальный код в `composer.js` и зафиксирован `green-fix.log` (exit `0`, 26 passed).
- **red:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/red-fix.log 2>&1
  ```
  Exit code: `1` (20 pass, 6 fail).
  Лог: `waves/RW-40/PM/b2b-attach-ui/red-fix.log`.
- **green:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/green-fix.log 2>&1
  npx tsc -p tsconfig.json
  ```
  Exit code: `0` (26 pass, 0 fail; duration ~54ms).
  TypeScript: `0` ошибок (`npx tsc -p tsconfig.json` exit code 0).
  Лог: `waves/RW-40/PM/b2b-attach-ui/green-fix.log`.
- **targeted:**
  - `tests/unit/composer.test.mjs`: 26 тестов (все pass).
  - Регрессионный контроль: `tests/unit/gui-spike-dom.test.mjs` + `tests/unit/gui-spike-features.test.mjs` + `tests/unit/gui-spike-regressions.test.mjs` — 140 тестов (все pass).
- **oracle:**
  - `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §3.1 (состояния чипа), §3.4 (блокировка отправки).
  - `waves/RW-40/reference-oracle-REA.md` §1 (STUB-13 вложения), §5 (STUB-12 подарок).
  - `rust/gui-spike/src/attachments.rs` (`MAX_UPLOAD_FILE_BYTES = 10485760`, `MAX_ATTACHMENTS_PER_MESSAGE = 10`, `AttachmentUploadSlot`).
  - `waves/RW-40/PM/b2b-attach-ui/REVIEW.md` (замечания P0, P1, P1).
- **генерация:** не применимо.
- **мутации:** 5 реальных мутаций зафиксированы в `waves/RW-40/PM/b2b-attach-ui/mutations.log`:
  1. `M1-SKIP-PUT` (пропуск PUT перед ready): md5 `adc9d4dba59223e8ed6d6d21da26602d` → exit 1, CAUGHT (5 тестов).
  2. `M2-SYNTH-FALLBACK` (синтез fallback upload_filename при отсутствии слота): md5 `e14b5f506e316756570dc457efd8ffe3` → exit 1, CAUGHT (1 тест).
  3. `M3-IGNORE-PUT-413` (игнорирование статуса ошибки 413): md5 `219c38cc59c99557d14ca4f1bd860513` → exit 1, CAUGHT (2 теста).
  4. `M4-NO-BLOCK-SEND-ON-ERROR` (разрешение отправки при ошибке): md5 `2d8b3000b4d46d9c3c0a2ae874f4aef8` → exit 1, CAUGHT (5 тестов).
  5. `M5-IGNORE-PUT-429` (игнорирование статуса ошибки 429): md5 `607eafb0968315dad43cf3d4df9a0e43` → exit 1, CAUGHT (2 теста).
  Хеш восстановления для всех 5 мутантов проверен и строго совпадает с базовым (`2b1d80a50dcd8495a204b32390a63dc0`).
- **baseline/restore:** продуктовый код и тесты проверены `npx tsc -p tsconfig.json` (exit code 0).
- **стабильность:** таймеры и `AbortController` очищаются при удалении/очистке чипов, утечек памяти нет, гонки состояний отсутствуют.
- **gate:worker:** targeted-тесты зелёные (26/26), регрессионный прогон зелёный (140/140), `tsc` чистый, `@ts-ignore` в тестах отсутствует.
- **файлы и md5:**
  - `rust/gui-spike/ui/features/composer.js`: `2b1d80a50dcd8495a204b32390a63dc0`
  - `tests/unit/composer.test.mjs`: `eafbdf850a02c0c26822b37a20c5cc7f`
  - `waves/RW-40/PM/b2b-attach-ui/red-fix.log`: `662e357d1061548a5e77e52a678789d9`
  - `waves/RW-40/PM/b2b-attach-ui/green-fix.log`: `703f24e543056bdcc6224fc38be16359`
  - `waves/RW-40/PM/b2b-attach-ui/mutations.log`: `b59793b7cacc8f143aa11383fae11c4e`
- **риски/вопросы:**
  - Изменялись исключительно назначенные файлы: `rust/gui-spike/ui/features/composer.js` и `tests/unit/composer.test.mjs`. Чужие файлы не затронуты.
- **следующий шаг:** передать на повторное ревью Team Lead / Coordinator (REVIEW-PASS2 / ACCEPT).
