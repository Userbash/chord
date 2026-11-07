# B2b-ATTACH-UI-20MIB-SYNC — отчёт воркера (Pass 4: синхронизация базового лимита 20 MiB с бэкендом B2a)

- **batch/карточка:** RW-40 / B2b-ATTACH-UI-20MIB-SYNC (Воркер G2 Frontend Lane A; устранение замечания P2 из `waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS3.md` и синхронизация базового лимита с принятым контрактом `waves/RW-40/PM/b2a-attach/ACCEPT.md` в назначенных файлах `rust/gui-spike/ui/features/composer.js` и `tests/unit/composer.test.mjs`).
- **сделано:**
  - **Синхронизация базового лимита на 20 MiB (`20 * 1024 * 1024 = 20_971_520` байт):**
    В соответствии с оракулом RE (`Math.max(0x1400000, tierLimit)`) и принятым контрактом `rust/gui-spike/src/attachments.rs` константа `MAX_UPLOAD_FILE_BYTES` в `rust/gui-spike/ui/features/composer.js` обновлена с 10 МиБ на 20 МиБ (`20 * 1024 * 1024 = 20_971_520` байт). В `ATTACH_LIMITS` поле `maxFileSize` автоматически получило значение `20_971_520`. Текст ошибки лимита (`ATTACH_TEXT.tooLarge(formatBytes(MAX_UPLOAD_FILE_BYTES))`) теперь выводит «Файл больше 20 МиБ. Уменьшите размер и попробуйте снова.».
  - **Тесты контракта и граничных условий:**
    В `tests/unit/composer.test.mjs` обновлены проверки констант `MAX_UPLOAD_FILE_BYTES` и `ATTACH_LIMITS.maxFileSize` на `20_971_520`. Регулярное выражение проверки текста ошибки обновлено на `/больше 20\s*(?:МиБ|МБ)/i`. Добавлен тест, проверяющий, что файлы в диапазоне 10…20 МиБ (15 МиБ и ровно 20 МиБ) не отсекаются с `error-size`, а корректно принимаются в очередь (`queued`).
  - **TDD-цикл:**
    1. Сначала обновлены тесты в `tests/unit/composer.test.mjs`, зафиксирован красный прогон `red-pass4.log` (exit `1`, 26 pass, 3 fail: расхождение `10485760 !== 20971520`, несовпадение текста ошибки и ложное отсечение файлов до 20 МиБ).
    2. Внесён минимальный продуктовый код в `rust/gui-spike/ui/features/composer.js` (`MAX_UPLOAD_FILE_BYTES = 20 * 1024 * 1024`).
    3. Зафиксирован зелёный прогон `green-pass4.log` (exit `0`, 29 pass, 0 fail; ~65 мс).
  - **Типизация и чистота:**
    `npx tsc -p tsconfig.json` — exit `0`, 0 ошибок.
    `npx tsc -p rust/gui-spike/ui/tsconfig.json` — exit `0`, 0 ошибок.
    Директивы `// @ts-ignore` — `0` в `composer.js` и `composer.test.mjs`.
- **red:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/red-pass4.log 2>&1
  ```
  Код выхода: `1` (26 pass, 3 fail).
  Лог: `waves/RW-40/PM/b2b-attach-ui/red-pass4.log`.
- **green:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/green-pass4.log 2>&1
  ```
  Код выхода: `0` (29 pass, 0 fail).
  Лог: `waves/RW-40/PM/b2b-attach-ui/green-pass4.log`.
- **targeted:**
  - `tests/unit/composer.test.mjs`: 29/29 pass.
  - Регрессионная связка GUI-спайка (`tests/unit/gui-spike-dom.test.mjs`, `tests/unit/gui-spike-features.test.mjs`, `tests/unit/gui-spike-regressions.test.mjs`, `tests/unit/composer.test.mjs`): 180/180 pass.
- **oracle:**
  - `waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS3.md` (P2 по расхождению базового лимита UI 10 MiB с бэкендом).
  - `waves/RW-40/PM/b2a-attach/ACCEPT.md` (`MAX_UPLOAD_FILE_BYTES = 20 * 1024 * 1024` = `20_971_520`, `Math.max(0x1400000, tierLimit)`).
  - `rust/gui-spike/src/attachments.rs` (базовый лимит `20 MiB`, бусты 50 / 100 MiB).
- **мутации:** 5/5 мутаций пойманы (зафиксированы в `waves/RW-40/PM/b2b-attach-ui/mutations-pass4.log`):
  1. `M1-20MIB-TO-10MIB` (возврат к `10 * 1024 * 1024`): exit `1`, `CAUGHT` (3 теста: расхождение констант, текст ошибки, блокировка файлов 10…20 МиБ).
  2. `M2-ATTACH-LIMIT-HARDCODED` (`ATTACH_LIMITS.maxFileSize = 10485760`): exit `1`, `CAUGHT` (1 тест).
  3. `M3-SIZE-GREATER-EQUAL` (замена `>` на `>=` на границе размера): exit `1`, `CAUGHT` (1 тест: файл ровно 20 МиБ).
  4. `M4-NO-SIZE-CHECK` (отключение предпроверки размера в `addFiles`): exit `1`, `CAUGHT` (1 тест: вызов сети при превышении размера).
  5. `M5-WRONG-ERROR-MSG` (подмена текста ошибки лимита): exit `1`, `CAUGHT` (1 тест: несовпадение regex сообщения).
  После каждой мутации восстановление `rust/gui-spike/ui/features/composer.js` проверено по исходному md5 (`223e7641d973ea68c93fde90969f3053`).
- **gate:**
  - `tests/unit/composer.test.mjs`: 29/29 pass.
  - GUI-спайк регрессии: 180/180 pass.
  - `npx tsc -p tsconfig.json`: exit `0`.
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json`: exit `0`.
  - Проверка отсутствия `@ts-ignore`: 0 директив.
- **файлы и md5:**
  - `rust/gui-spike/ui/features/composer.js`: `223e7641d973ea68c93fde90969f3053`
  - `tests/unit/composer.test.mjs`: `fd4211394ef4bb7659477fd5904ab584`
  - `waves/RW-40/PM/b2b-attach-ui/red-pass4.log`: `f82d13003fb61ac3623fe9d2ac1550b4`
  - `waves/RW-40/PM/b2b-attach-ui/green-pass4.log`: `98d88999a00a26f191737af16442da09`
  - `waves/RW-40/PM/b2b-attach-ui/mutations-pass4.log`: `15c4f952067dd2d2b0e129f0ac45de5e`
- **риски/вопросы:**
  - Изменения ограничены строго двумя назначенными файлами (`rust/gui-spike/ui/features/composer.js` и `tests/unit/composer.test.mjs`). Замечание P2 из REVIEW-PASS3 полностью закрыто синхронизацией базового предела 20 MiB (`20_971_520` байт).
