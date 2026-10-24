# BRIEF NET-C2 — backend auth: login/logout/status + keyring

Владелец: NET-C2 (Backend). PM: DeepSeek. Основание: `ADR-2-auth.md`, `CORE-AUDIT` P0-2/P0-5/P0-6,
`AUTH-RE` (сырой токен).
Запуск: параллельно NET-E1 (другой target-dir). Сеть в тестах — только стабы.

## Файлы (только эти)
- `rust/gui-spike/src/auth.rs` (новый) — AuthState/AuthError/валидация/keyring-поток
- `rust/gui-spike/src/lib.rs`, `src/main.rs` — Tauri-команды
- `rust/gui-spike/Cargo.toml` — path-dep `discord-platform` (keyring), без внешних крейтов
- `rust/gui-spike/tests/auth_contract.rs` (новый)
- логи: `waves/RW-39/NET/PM/netc2/`
READ-ONLY: ui/**, fixtures/**, data.rs (кроме необходимого экспорта), остальные тесты.

## Задачи (red-first)
1. `auth_status() -> AuthState` (idle/validating/ready/error) — без утечек токена.
2. `auth_login_token(token: String)`:
   - `ApiAuth::from_token(AuthScheme::User, …)`, `SuperProperties` из `identity::build`;
   - `ApiClient::get("/users/@me")` → `Me`; 401 → `InvalidToken`, 429 → `RateLimited`,
     сеть → `Network`; **токен не логируется**;
   - keyring.store(token) через поток-владелец (keyring `!Send`), ошибки → `Keyring`.
3. `auth_logout()` — stop/wipe/keyring.clear; повторный logout идемпотентен.
4. Восстановление при старте: `auth_status` читает keyring, но **не** ходит в сеть без
   явного `auth_restore()` (или параметр `probe: bool`).
5. Тесты с фейковым keyring/валидатором (trait), без реальной сети; негатив 401/429/битый JSON.

## Проверки (gui-spike target)
```bash
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
```
Окно/probe/gate/сеть/commit — запрещены.

## Отчёт
`NET/PM/netc2/REPORT.md`: API команд, ошибки, тесты, md5, что осталось (UI/login screen).
