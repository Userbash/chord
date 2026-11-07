# Независимое ревью — INT-03-SAVE-QUEUE-FIX (G-01b-UI Pass 4)

Дата: 2026-10-30. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Режим: read-only; продуктовый код и тесты не менялись; `cargo` не запускался.

## Вердикт

**PASS по предоставленной поверхности ревью. P0=0, P1=0, P2=0.** P1 из Pass 3 исправлен: `rememberGuild` сериализует чтение/запись через `saveQueue` и пропускает устаревшие задачи до начала следующего IPC. Batch gate в рассмотренных материалах не представлен; этот PASS не является подтверждением gate или финальной приёмкой batch.

## Проверки

- **RED:** `red-pass4.log` показывает ожидаемое падение нового сценария при старом поведении: после A → B в store оставался A; exit 1.
- **GREEN:** `green-pass4.log` — 16 passed, 0 failed; exit 0.
- **TARGETED:** `targeted-pass4.log` содержит 98 passed, 0 failed (exit 0), `npx tsc -p tsconfig.json` (exit 0) и `npx tsc -p rust/gui-spike/ui/tsconfig.json` (exit 0).
- **MUTATION:** все пять мутантов в `mutations-pass4.log` убиты ожидаемыми тестами; после каждого MD5 `app.js` восстановлен к baseline `8f5d84c14dc6e9aea749a202ecb3a96b`.
- **Корректность сценария:** тест откладывает завершение `settings_save` A, выбирает B, затем разрешает A и проверяет итоговый `lastGuildId` и порядок `start:A, finish:A, start:B, finish:B`. Реализация ждёт завершения предыдущей записи перед чтением/записью B и проверяет `latestGuildId` до лишних IPC-вызовов.
- **TEST-POLICY:** в проверенном тесте нет ослабления assertion или сокрытия результата; MD5 теста после мутаций совпадает с baseline из лога.

## Ограничение evidence

`REPORT-PASS4.md` и перечисленные pass4-логи не содержат batch-level `gate:worker` evidence. По TEST-POLICY и PROCESS это остаётся отдельной предпосылкой приёмки batch; `cargo` в рамках этого read-only ревью не запускался.
