FAIL

- **P1** — [rust/gui-spike/src/net.rs:132](/home/user/orca/project/rust/gui-spike/src/net.rs:132): `AuthTokenStore::clear()` игнорирует результат `AuthBackend::logout()` и всегда возвращает `Ok(())`. При ошибке очистки keyring 4004 всё равно переводит соединение в `Offline`; следующий `connect()` может снова получить токен вместо обязательного `MissingToken`. Тест проверяет только `StaticTokenStore`, не production keyring adapter.
- **P2** — [rust/gui-spike/ui/features/auth.js:164](/home/user/orca/project/rust/gui-spike/ui/features/auth.js:164): сброс `ticket`, `instance`, captcha-полей, логина, пароля и кода выполняется только после успешного `auth_logout`. При reject IPC эти секреты остаются в замыкании и `code` может остаться в DOM.

Проверки:

- **P0: 0**
- 4014 сохраняется как `GatewayEvent::Closed(4014)` и классифицируется в `Error`; обход captcha не обнаружен.
- Явных утечек токена/пароля/кодов в UI, логах и snapshots не обнаружено.
- Ослабления тестов (`skip`/`only`/`todo`, удаление assertions) в проверенных файлах не обнаружено.
- Сеть, окно и cargo не запускались.