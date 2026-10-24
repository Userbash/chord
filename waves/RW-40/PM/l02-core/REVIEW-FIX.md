# REVIEW-FIX-L02-CORE

Дата: 2026-10-24. Режим: read-only review; тесты, cargo, gate и live не запускались.

## Вердикт

**ACCEPT.** P1 замечания прошлого review закрыты по текущему коду и представленным red/green логам. P2 oracle пробел закрыт. UI/live доставка этим review не подтверждается.

## Проверки

- **P1 flush до `?`: закрыто.** В `rust/crates/discord-core/src/ws.rs:653-656` результат `push_binary` сохраняется, `flush_dispatches` вызывается до распространения `parsed?` и до обработки эффектов. Новая ветка теста в `rust/crates/discord-core/tests/gateway_supervisor.rs:952-1005` отправляет READY и ETF-терм с неизвестным тегом `0xff` в одном zstd-фрагменте и требует получить READY в callback после ошибки. При `on_dispatch: None` flush остается no-op, существующий буфер не меняет поведение.
- **RED/green evidence согласуется со сценарием.** `fix-red-core-flush.log` показывает `[]` против `["READY"]` до исправления. `fix-green-core-targeted.log` показывает тест успешным, 14 passed / 0 failed. `fix-red-oracle-token-mutant.log` фиксирует `redacted=2` вместо `4` при мутанте; `fix-green-net-contract.log` показывает новый oracle-тест успешным, 23 passed / 0 failed.
- **MD5 текущих файлов совпадают с FIX-REPORT.md §4:** `ws.rs` `8703c1d695cd0af26458393eb3c1e3f9`; `gateway_supervisor.rs` `7f23d85a4d3b0e54c58d765b8b879c58`; `net_contract.rs` `fe8aa92146e9dcf8b943335dfad652c8`. Старые хэши в `REPORT.md` §4 относятся к состоянию при приёмке L-02; пояснение в §4, строки 110-113, направляет к актуальным хэшам FIX-REPORT. Нет расхождения с текущей версией.
- **P2 token oracle: закрыто.** Независимый `secret_paths` перечисляет точный ключ `token` в `rust/gui-spike/tests/net_contract.rs:355-382`; отдельный тест проверяет вложенный и верхнеуровневый `token`, `access_token`, `refresh_token`, точный счётчик 4 и отсутствие секретных значений в JSON (`:523-565`).
- **Ослаблений не обнаружено.** В двух назначенных тестовых файлах нет `.only`, `#[ignore]` или новых skip. Найденные строки `skip` в `net_contract.rs:130-143` относятся к уже существующей обработке отсутствующих fixture с объяснением причины. Изменения oracle усиливают проверки.
- **Регрессия по просмотренному срезу не выявлена.** Сохраняются flush до `apply_effects`, forwarding при обычном потоке, redaction ключей и проверка счётчика. Полный gate здесь не повторялся согласно read-only ограничениям.

## Findings

P0: нет.

P1: нет.

P2: нет.

## Границы доказательств

Тесты проверяют callback/backend контур и локальный WS-стаб. Подписка UI, отображение события в реальном окне и доставка на live Discord не доказаны; `FIX-REPORT.md` §5 также явно оставляет UI/live вне среза.
