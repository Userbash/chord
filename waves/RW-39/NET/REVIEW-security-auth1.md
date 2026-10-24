**FAIL**

P0: 0.

P1:

- [rust/gui-spike/src/net.rs:74](/home/sanya/orca/project/rust/gui-spike/src/net.rs:74), [rust/gui-spike/src/net.rs:263](/home/sanya/orca/project/rust/gui-spike/src/net.rs:263)  
  При close `4004` состояние меняется на `Offline`, но токен из keyring не очищается. `TokenStore` поддерживает только `load`, а `AuthTokenStore` read-only. Следующий `connect()` снова использует тот же недействительный токен.  
  Repro: `connect()` → драйвер вызывает `Closed(4004)` → `status() == Offline` → повторный `connect()` снова передаёт прежний токен в `driver.connect`. Это оставляет недействительный credential в постоянном хранилище и вызывает повторные unauthorized-подключения.

P2:

- [rust/gui-spike/src/net.rs:220](/home/sanya/orca/project/rust/gui-spike/src/net.rs:220)  
  `StopReason::InvalidClient { code }` для `4013` и `4014` преобразуется всегда в `GatewayEvent::Closed(4013)`. Код `4014` теряется.  
  Repro: supervisor сообщает `Stopped(InvalidClient { code: 4014 })` → `map_supervisor_event()` возвращает `Closed(4013)`. Обработка статуса остаётся `Error`, но диагностика и downstream policy получают неверный close-код.

- [rust/gui-spike/tests/net_contract.rs:33](/home/sanya/orca/project/rust/gui-spike/tests/net_contract.rs:33)  
  Контрактный тест проверяет `4004` и `4013`, но не проверяет `4014`; текущая ошибка с потерей кода тестами не ловится.

- [rust/gui-spike/ui/features/auth.js:92](/home/sanya/orca/project/rust/gui-spike/ui/features/auth.js:92), [rust/gui-spike/ui/features/auth.js:164](/home/sanya/orca/project/rust/gui-spike/ui/features/auth.js:164)  
  При logout очищаются поля DOM, но не сбрасываются closure-переменные `ticket`, `instance`, `captchaKey`, `captchaRqtoken`, `loginValue`, `passValue`. Это оставляет MFA/captcha-контекст в памяти после выхода. Прямой утечки в DOM, лог или чужой IPC не обнаружено.

Проверено:

- P0-утечек токена, пароля и 2FA-кода через `Debug`, ошибки, DOM, UI snapshot и IPC не найдено.
- Captcha solver/обхода нет; используется только интерактивный adapter.
- DOM-инъекций через `innerHTML`/`href` не найдено; пользовательские значения записываются через `textContent`/атрибуты.
- Shell-инъекций не найдено.
- `unwrap()` на проверяемых входах не найден; mutex poisoning обрабатывается.
- Logout в `AuthBackend` идемпотентен, но сетевой `4004` не вызывает очистку keyring.
- В `tests/unit/gui-spike-auth.test.mjs` и `rust/gui-spike/tests/net_contract.rs` не найдено `.skip`, `.only`, `todo` или отключённых тестов. Cargo/сеть/окно не запускались.