# REPORT — TASK-969: Генератор бинарных дельта-патчей zstd/bsdiff и проверка отката

- **Волна / Карточка:** Wave RW-45 / `TASK-969` (`task_9c3aeeba32ca`, dispatch `ctx_37678036b4d6`)
- **Отдел:** `DevOps / Integration`
- **Приоритет:** `P1`
- **Статус:** Выполнено (все критерии приёмки DoD 1–3 закрыты на 100%)

---

## 1. Сделано

1. **Подзадача 969.1 — Генератор и валидатор бинарных дельт [`scripts/generate-delta.mjs`](file:///home/sanya/orca/project/scripts/generate-delta.mjs):**
   - Реализована генерация и применение бинарных дельт в двух режимах:
     - `bsdiff43-zstd` (`ENDSLEY/BSDIFF43+ZSTD`): заголовок `ENDSLEY/BSDIFF43` (16 байт, по эталонному RE-дампу `/home/sanya/re-deep/native-re/raw/updater/src-files.txt:5` `discord_common/rust/endsley_bsdiff/src/lib.rs`) + 8-байтный `offtout(new_size)` + поток триплетов `(add_len, copy_len, seek_offset)`, байтов разностей и литеральных вставок, упакованный в кадр Zstandard (`0x28 0xb5 0x2f 0xfd` по `/home/sanya/re-deep/gateway-protocol.md` §3).
     - `zstd-patch-from` (`ZSTD-PATCH-FROM`): словарный дифф-движок `/usr/bin/zstd --patch-from=<old>` и распаковка `zstd -d --patch-from=<old>`.
   - Встроена обязательная самопроверка (`selfVerified: true`) при генерации каждого патча: сухой прогон распаковки и сверка `SHA-256` и размера с целевым образом.
   - Реализованы валидаторы `validateDeltaManifest`, `verifyFileHash` и `verifyDownloadedPatchPackage` со строгим соблюдением диагностических контрактов оригинального апдейтера Discord (`/home/sanya/re-deep/native-re/raw/updater/strings-a-n6.txt`, строки 8873, 9822–9826, 10197):
     - `expected hash '<expected>' for '<path>', actually '<actual>'` (`delta_update/src/verify.rs`, `VerifyResult::InvalidHash`);
     - `SHA256 mismatch for downloaded package: expected <expected>, got <actual>` (`updater_client/src/install.rs`);
     - `Content length was <expected> but only transferred <actual> bytes` (`updater_client/src/install.rs`).
   - Реализован транзакционный механизм наложения дельт с автоматическим откатом `applyDeltaWithRollback`:
     - Предварительная сверка `SHA-256` исходного файла и создание резервной копии с сохранением прав доступа (`0o755`).
     - При любом сбое (искажение битов патча, усечение файла, порча магических байт Zstd или триплетов `ENDSLEY/BSDIFF43`, несовпадение целевого `SHA-256` после применения) выполняется немедленный атомарный откат к исходному бинарнику со 100% сохранением его `SHA-256` и прав `0o755`, очисткой `.staging`/`.rollback.bak` файлов и переключением `retryOptions.skip_host_delta = true` / `retryOptions.skip_module_delta[moduleName] = true` (по `/home/sanya/re-deep/main-arch.md` §5.2, `app/bundle.js:12` `#53006 @98275` `updateUntilCurrent`), а также опциональным накатом полного пакета `fallbackFullFile`.
   - Реализован CLI (`--generate`, `--verify`, `--apply`, `--self-test`, коды выхода `0`/`1`/`2`) и маскирование секретов `maskSensitiveTokens` (`mfa.***`, `Bot ***`, `Bearer ***`, `ghp_***`).

2. **Подзадача 969.2 — Контрактный и модульный тест [`tests/unit/binary-delta-969.test.mjs`](file:///home/sanya/orca/project/tests/unit/binary-delta-969.test.mjs) (Strict TDD):**
   - `SUB-969-1`: кодирование `encodeOfft`/`decodeOfft` на граничных значениях `0..±0x7fffffffffffffffn`, проверка заголовков `28 b5 2f fd` и `ENDSLEY/BSDIFF43` через независимый системный оракул `/usr/bin/zstd -d`, граничные переходы `0 -> N`, `N -> 0`, `N -> N`.
   - `SUB-969-2`: генерация и наложение дельт в обоих режимах (`bsdiff43-zstd` и `zstd-patch-from`) на базе реального релизного бинарника `rust/dist/discord-gpui` (сверка с `rust/dist/SHA256SUMS`), 100% восстановление `SHA-256` и валидация `ManifestPatch`.
   - `SUB-969-3`: проверка точных RE-строк `delta_update::verify` и `updater_client::install`.
   - `SUB-969-4`: проверка атомарного отката при bit-flip, усечении патча, повреждении триплетов `ENDSLEY/BSDIFF43` и подмене целевого хеша, проверка переключения `skip_host_delta`/`skip_module_delta` и фолбэка на полный образ.
   - `SUB-969-5`: сквозная проверка CLI (`--generate`, `--verify`, `--apply`, `--self-test`), кодов выхода `0`/`1`/`2` и маскирования токенов.

---

## 2. Red-прогон (TDD)

Лог: [`waves/RW-45/DEVOPS/task-969-binary-delta/red.log`](file:///home/sanya/orca/project/waves/RW-45/DEVOPS/task-969-binary-delta/red.log) (запуск до создания `scripts/generate-delta.mjs`):

```text
▶ TASK-969: Генератор бинарных дельта-патчей zstd/bsdiff и проверка отката
  ✖ SUB-969-1: кодирует ENDSLEY/BSDIFF43 + Zstandard (0x28 0xb5 0x2f 0xfd) и проходит сверку с независимым /usr/bin/zstd (0.724577ms)
  ✖ SUB-969-2: генерирует дельты в режимах bsdiff43-zstd и zstd-patch-from со 100% восстановлением SHA-256 и валидирует ManifestPatch (0.273533ms)
  ✖ SUB-969-3: формирует точные диагностические контракты RE-дампа delta_update::verify и updater_client::install при искажении хешей и размера (0.617147ms)
  ✖ SUB-969-4: выполняет атомарный откат при повреждении патча/целевого хеша и переключает skip_host_delta / skip_module_delta (0.278283ms)
  ✖ SUB-969-5: проверяет CLI scripts/generate-delta.mjs (--generate, --verify, --apply, --self-test), коды 0/1/2 и маскирование токенов (0.236712ms)
ℹ tests 5 | suites 1 | pass 0 | fail 5
```

---

## 3. Green-прогон (TDD)

Лог: [`waves/RW-45/DEVOPS/task-969-binary-delta/green.log`](file:///home/sanya/orca/project/waves/RW-45/DEVOPS/task-969-binary-delta/green.log):

```text
▶ TASK-969: Генератор бинарных дельта-патчей zstd/bsdiff и проверка отката
  ✔ SUB-969-1: кодирует ENDSLEY/BSDIFF43 + Zstandard (0x28 0xb5 0x2f 0xfd) и проходит сверку с независимым /usr/bin/zstd (50.569519ms)
  ✔ SUB-969-2: генерирует дельты в режимах bsdiff43-zstd и zstd-patch-from со 100% восстановлением SHA-256 и валидирует ManifestPatch (48.432858ms)
  ✔ SUB-969-3: формирует точные диагностические контракты RE-дампа delta_update::verify и updater_client::install при искажении хешей и размера (0.717227ms)
  ✔ SUB-969-4: выполняет атомарный откат при повреждении патча/целевого хеша и переключает skip_host_delta / skip_module_delta (37.706532ms)
  ✔ SUB-969-5: проверяет CLI scripts/generate-delta.mjs (--generate, --verify, --apply, --self-test), коды 0/1/2 и маскирование токенов (210.062093ms)
ℹ tests 5 | suites 1 | pass 5 | fail 0 | skipped 0
```

---

## 4. Targeted, Typecheck & Мутационный гейт (5/5)

- **Targeted (`node --test tests/unit/binary-delta-969.test.mjs`):** `5/5 pass (0 fail, 0 skip)`.
- **Typecheck (`npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict ... scripts/generate-delta.mjs tests/unit/binary-delta-969.test.mjs`):** `0 errors`.
- **Мутационный гейт ([`waves/RW-45/DEVOPS/task-969-binary-delta/mutation.log`](file:///home/sanya/orca/project/waves/RW-45/DEVOPS/task-969-binary-delta/mutation.log)):**
  1. `M1` (Подмена сигнатуры `ENDSLEY_BSDIFF43_MAGIC` на `ENDSLEY/BSDIFF00`) — **KILLED**
  2. `M2` (Отключение знакового бита `SIGN_BIT_64` в `encodeOfft`) — **KILLED**
  3. `M3` (Пропуск сверки `SHA-256` пакета патча в `verifyDownloadedPatchPackage`) — **KILLED**
  4. `M4` (Отключение восстановления `backupPath -> targetFile` при откате в `restoreFromBackupAndFallback`) — **KILLED**
  5. `M5` (Отключение выставления флага `skip_host_delta: true` при отказе `HostInstall`) — **KILLED**
  - Итог: **5/5 мутаций убито (100% kill rate)**, оригинал побайтно восстановлен и проверен по MD5.

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `scripts/generate-delta.mjs` | `ba771094316cdd3917394452d78cda71` |
| `tests/unit/binary-delta-969.test.mjs` | `942417d11bc8baa41180858287925da6` |

---

## 6. Риски и замечания

- Временные файлы транзакции обновления (`.staging` и `.rollback.bak`) гарантированно очищаются как при успешном применении дельты, так и при аварийном откате.
- Блокеров нет (`P0 = 0`, `P1 = 0`).
