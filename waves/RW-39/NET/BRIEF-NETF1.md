# BRIEF NET-F1 — backend: password login + MFA + captcha-контекст

Владелец: NET-F1 (Backend). PM: DeepSeek. Red-first. Сеть — только фейки/стабы.
Основание: `ADR-3-login.md`, `AUTH-RE` A-02…A-08, `NET-FUNCTIONS` NF-01…NF-09.

## Файлы
- `rust/crates/discord-core/src/rest/client.rs` — `ApiClient::post` (JSON body) + тест
- `rust/gui-spike/src/auth.rs`, `src/main.rs`, `tests/auth_contract.rs`
- при необходимости новый `rust/gui-spike/src/auth_flow.rs`
- логи `NET/PM/netf1/`

## Задачи
1. `ApiClient::post(path, body: &[u8])` — те же заголовки/retry/лимиты; тест на стабе.
2. `LoginOutcome`: `Token(String)`, `Mfa{ticket, methods}`, `Captcha{sitekey, service, rqtoken}`,
   `Error(AuthError)`.
3. `login_password(login, password, captcha_key?, captcha_rqtoken?)` →
   `POST /auth/login` (тело по AUTH-RE A-02 + captcha-поля, если есть).
4. `submit_mfa(method, code, ticket, login_instance_id?)` → `/auth/mfa/{method}`;
   `request_sms(ticket)` → `/auth/mfa/sms/send`.
5. Tauri-команды `auth_login_password`, `auth_mfa`, `auth_sms` (ошибки без секретов).
6. Пароль/код: только в памяти, best-effort overwrite; **не** в keyring/логах/Debug/ошибках.
7. Тесты: fake transport, 401/429/captcha/mfa/ok, пароль не в Debug/ошибке, retry не долбит.

## Проверки
```bash
cargo test --offline -p discord-core
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
```
Окно/сеть/commit — запрещены. Один cargo за раз (target-dir разные — последовательно).

## Отчёт
`NET/PM/netf1/REPORT.md`: NF → red → green, тесты, md5, остаток (UI captcha, live).
