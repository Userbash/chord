# BRIEF B1a — backend отправки сообщений

Владелец: G1 Backend/Network. PM: DeepSeek. Red-first, без live-сети.
Файлы: `core/src/rest/routes.rs`, `core/tests/rest_routes.rs`, `gui-spike/src/messages.rs` (new),
`gui-spike/src/lib.rs`, `src/main.rs`, `tests/messages_contract.rs` (new).
READ-ONLY: `ui/**`, `fixtures/**`, остальные тесты.

## Задачи
1. Route `POST /channels/{id}/messages` в каталог (capture D-02); обновить счётчик/тест.
2. `MessageSender` trait + `DiscordMessageSender` на `ApiClient`:
   `send_message(channel_id, content, nonce)`, JSON `{content, nonce, tts:false}`.
3. Валидация: пустой/пробельный content → `InvalidArg`; лимит 2000; не логировать.
4. Ошибки: 401 → Unauthorized, 429 → RateLimited, сеть/JSON → typed; nonce сохраняет
   идемпотентность при повторе.
5. Tauri `send_message`; токен только из `AuthBackend::token()`/keyring.
6. Тесты: fake sender (ok/401/429/empty/too long), route POST, отсутствие токена в ошибках.
7. Не ломать 573 core / 105 gui.

## Проверки
```bash
cd rust && cargo test --offline -p discord-core
cargo test --offline --manifest-path gui-spike/Cargo.toml
cargo clippy --offline -p discord-core --all-targets -- -D warnings
cargo clippy --offline --manifest-path gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt -p discord-core -- --check
cargo fmt --manifest-path gui-spike/Cargo.toml -- --check
```
## Отчёт
`NET/PM/b1a/REPORT.md`: команды, тесты, md5, остаток (live send).
