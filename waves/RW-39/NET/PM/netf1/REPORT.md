# NET-F1 — backend входа по паролю, MFA и captcha-контекст

**Волна:** RW-39, отдел NET, роль NET-F1 (Backend). PM: DeepSeek. Дата: 2026-10-23.
**Статус:** выполнено, red-first. Сеть/окно/commit не использовались; captcha не решалась
и не обходилась (только контекст для штатного виджета).
**Основание:** `BRIEF-NETF1.md`, `ADR-3-login.md`, `AUTH-RE.md` A-02…A-08/H-01/§6,
`NET-FUNCTIONS.md` NF-01…NF-06.

## 1. Сделано (NF → файл)

| NF | Что | Файл |
|---|---|---|
| NF-01 | `ApiClient::post(target, body)` — те же auth-заголовки/телеметрия/retry/лимиты, что `execute` | `rust/crates/discord-core/src/rest/client.rs` |
| NF-01 | `ApiClient::post_once(target, body, headers)` — один POST, 4xx/5xx отдаются с телом, повторов нет (нужен для `400 + captcha_key` и кодов MFA) | там же |
| NF-02 | `AuthBackend::login_password(login, password, captcha_key?, captcha_rqtoken?)` → `POST /auth/login` (A-02; `authorization:""` по A-03; captcha — заголовками `X-Captcha-Key`/`X-Captcha-Rqtoken`) | `rust/gui-spike/src/auth.rs` |
| NF-03 | `submit_mfa(method, code, ticket, login_instance_id?)` → `POST /auth/mfa/{method}` (A-05); method — whitelist `totp/sms/backup/webauthn/password` | там же |
| NF-04 | `request_sms(ticket)` → `POST /auth/mfa/sms/send {ticket}` (A-07), ответ `{phone}` | там же |
| NF-05/06 | `LoginOutcome::{Token, Mfa{ticket,methods,login_instance_id}, Captcha{sitekey,service,rqtoken}, Error}`; `LoginView` для UI (без токена) | там же |
| NF-07 (backend-часть) | Tauri-команды `auth_login_password`, `auth_mfa`, `auth_sms` | `rust/gui-spike/src/main.rs` |
| — | Тесты на fake transport (без сети/окна/keyring) | `rust/gui-spike/tests/auth_contract.rs`, `rust/crates/discord-core/tests/rest_client.rs` |

### Секреты и ошибки

- Пароль и 2FA-код принимаются по значению, сериализуются в тело и **сразу** затираются
  (`wipe_string`: `mem::take` → `into_bytes` → `fill(0)`); тело запроса затирается после ответа
  (`wipe_bytes`). Копии тела внутри `ApiClient`/сокета освобождаются обычным drop — это
  best-effort, как зафиксировано в ADR-3 (в safe Rust без сторонних крейтов иначе нельзя).
- Пароль/код не попадают в `AuthState`, keyring, `Debug`, `Display`, JSON ошибок. `Debug`
  `LoginOutcome` маскирует токен, ticket и captcha-rqtoken (`<redacted>`).
- `LoginView` (ответ UI) не содержит токена: успешный вход уже проверен `/users/@me` и сохранён
  в keyring → `Ready { user }`.
- 401/`50018` → `InvalidCredentials`; `60006/60008/60009` → `MfaRejected{code}`; прочие
  отказы → `LoginRejected{status, code}`; 429 → `RateLimited{retry_after_ms}` — без текста тела.
- «Долбёжка» исключена: `post_once` не повторяет запросы, `DiscordLogin` работает одной попыткой;
  тесты подтверждают ровно 1 вызов на 401/429/сетевую ошибку/неверный 2FA-код.

## 2. Red → green

| Шаг | Команда | Результат |
|---|---|---|
| baseline core | `cargo test --offline -p discord-core` | pass (`00-baseline-core.log`) |
| baseline gui | `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | pass, 80 тестов (`00-baseline-gui.log`) |
| red core | `cargo test --offline -p discord-core --test rest_client` | **FAIL**: `E0599 no method named post/post_once` (`01-red-core.log`) |
| green core | то же | 26 passed (`02-green-core.log`) |
| red auth | `cargo test --offline --test auth_contract` | **FAIL**: `E0432 unresolved imports LoginOutcome/LoginTransport/...` (`03-red-auth.log`) |
| green auth | то же | 36 passed (`04-green-auth.log`) |

## 3. Финальные проверки

| Проверка | Результат | Лог |
|---|---|---|
| `cargo test --offline -p discord-core` | pass, 572 `ok`, 0 failed (`12-core-clippy.log` — clippy core) | `11-final-test-core.log` |
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | pass, 95 тестов (auth_contract 36) | `09-final-test-gui.log` |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | clean | `10-final-clippy.log` |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | clean (после `cargo fmt`) | `08-fmt-check.log` |
| `cargo clippy --offline -p discord-core --all-targets -- -D warnings` (доп.) | clean | `12-core-clippy.log` |

Тесты: было 80 (gui) / 22 в `rest_client`; стало 95 (gui, +15) / 26 в `rest_client` (+4).

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/rest/client.rs` | `0b11f70442335d9582fafe0b69a7dec7` |
| `rust/crates/discord-core/tests/rest_client.rs` | `ac777396ea49c89d495db38d5ca031a0` |
| `rust/gui-spike/src/auth.rs` | `56c81b7e9964880353333f3a776992ba` |
| `rust/gui-spike/src/main.rs` | `9e1cc3e9bef0b44d51f560bdd4961627` |
| `rust/gui-spike/tests/auth_contract.rs` | `e4910a004f61aeaa8e5839057a0c6912` |

Логи: `00-baseline-core.log`, `00-baseline-gui.log`, `01-red-core.log`, `02-green-core.log`,
`03-red-auth.log`, `04-green-auth.log`, `05-test-core.log`, `06-test-gui.log`,
`07-clippy.log`, `08-fmt-check.log`, `09-final-test-gui.log`, `10-final-clippy.log`,
`11-final-test-core.log`, `12-core-clippy.log`, `md5.txt`.

## 5. Остаток / передача

- **NET-D2 (UI):** экраны captcha (штатный hCaptcha/recaptcha/turnstile виджет) и MFA
  (`totp/sms/backup/password`), вызовы команд `auth_login_password`/`auth_mfa`/`auth_sms`.
  Backend отдаёт контекст: `sitekey/service/rqtoken`, `ticket/methods/login_instance_id`.
- **NF-08:** событие `auth://state` не добавлялось (в брифе NET-F1 не требуется; UI получает
  состояние ответом команд).
- **WebAuthn-MFA:** метод в whitelist и путь поддержаны, но нативная подпись (`discord_webauthn`)
  — вне scope; UI должен показать его как недоступный/через официальный клиент.
- **Live-проверка** `/auth/*` (N-01) и keyring-live — только по согласованному окну с тестовым
  аккаунтом; в этой работе сети не было.
- Риск ToS/блокировки — по ADR-3; используется только тестовый аккаунт владельца.
