# Независимое ревью — STUB-27-CORE-STREAM-OPCODES

Дата: 2026-10-30. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Продуктовый код read-only; `cargo` и тесты не запускались.

## Вердикт

**RETURN. P0=0, P1=2.** Red/green и mutation evidence из отчёта сверены с логами; md5 исходника и теста совпадают с отчётом.

### P1 — изменение общего `lib.rs` вне назначения

`REPORT.md` §1 сообщает о добавлении `pub mod stream;` в `rust/crates/discord-core/src/lib.rs`, и файл изменён. Он не входит в указанную файловую матрицу карточки, а `AGENTS.md` §2 относит общие `src/lib.rs` к файлам Team Lead/Tech Lead. Это изменение должно быть отдельно назначено/принято ответственным владельцем общего файла.

### P1 — отсутствие RE-бандла считается успешной проверкой

В `stream_gateway_contract.rs:126-133` ошибка чтения внешнего oracle превращается в `None`, и проверки размера/байтов пропускаются, хотя тест продолжает считаться прошедшим. Это не skip с причиной и не `EnvFail`, как требует TEST-POLICY и правило skip-aware тестов в `AGENTS.md`. Для T-STREAM-00 недоступность обязательного oracle не должна давать зелёный результат.

## Evidence

- RED: `red.log`, unresolved import `discord_core::stream`, exit 101.
- GREEN: `green.log`, 11 passed / 0 failed, exit 0.
- Мутации: `mutations.log`, 5/5 caught; восстановление завершилось зелёным прогоном.
- Проверенные md5: `stream.rs` `e4e0450559bcf947261ca7bc91aea91f`; тест `bfc82f9d2dacc7bccb1e1475ddb5c7c0`.
- По отчёту полный batch gate оставлен integration-лейну; gate evidence в артефактах карточки не представлен.
