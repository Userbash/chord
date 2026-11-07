# Независимое ревью — V-01-RTC-SESSION-OFFLINE

Дата: 2026-10-30. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Продуктовый код read-only; `cargo` и тесты не запускались.

## Вердикт

**RETURN. P0=0, P1=1, P2=1.** TDD и мутационные логи содержат широкий набор сценариев и восстановление md5, но обработка ошибки транспорта не исполняет необходимые эффекты.

### P1 — `TransportFailed` и oversize теряют эффекты реконнекта

`session.rs:640-643` и `801-804` вызывают `begin_reconnect()`, отбрасывают возвращённые эффекты (`let _ = ...`) и возвращают только ошибку. Поэтому вызывающая сторона не получает `CloseSocket`, `StartTimer(Reconnect)` и notices, хотя состояние уже переведено в `Reconnecting`. Это расходится с V01-SESSION-CONTRACT §2.3 пп.13/15: переход должен инициировать закрытие/переподключение. В тестах проверяется состояние/ошибка, но не наблюдаемый reconnect effect при `TransportFailed` и oversize.

### P2 — недоступный loopback засчитывается как passed

`session_contract.rs:1216-1234` печатает `skip` и возвращает управление из теста при `PermissionDenied`. Runner отмечает такой тест passed, а не skipped, поэтому CI не отличит выполненную интеграционную проверку от непроверенной. Это нарушает skip-aware требование `AGENTS.md` и должно быть отражено явным skipped/evidence-механизмом.

## Evidence

- RED: `red.log`, 16/16 новых session tests failed до реализации, exit 101.
- GREEN: `green.log`, отчёт суммирует 147 passed / 0 failed; `session_contract` 16 passed.
- Мутации: `mutations.log`, 10 заявленных мутаций caught, restore md5 и финальный green.
- md5 `session.rs`, `lib.rs`, `session_contract.rs` совпадают с отчётом.
