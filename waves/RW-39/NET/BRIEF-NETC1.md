# BRIEF NET-C1 — core-примитивы авторизации (метод-агностик)

Владелец: NET-C1 (Core). PM: DeepSeek. Основание: `NET/CORE-AUDIT.md` P0-1, P0-4, P1-1, P1-2.
Запуск: параллельно NET-A (read-only). Сеть/окно не нужны.

## Файлы (только эти)
- `rust/crates/discord-core/Cargo.toml` — `discord-json` из dev-deps в dependencies
- `rust/crates/discord-core/src/rest/client.rs` — `ApiAuth` конструктор, redaction
- `rust/crates/discord-core/src/rest/transport.rs` — `HttpRequest` Debug
- `rust/crates/discord-core/src/identify.rs` — единый источник свойств, EmptyToken
- `rust/crates/discord-core/src/rest/user.rs` (новый) — `Me` + парсер `/users/@me`
- `rust/crates/discord-core/src/lib.rs` — re-export
- `rust/crates/discord-core/tests/{rest_client.rs,identify.rs,rest_user.rs}`
- логи: `waves/RW-39/NET/PM/netc1/`
READ-ONLY: остальные крейты, fixtures, gui-spike, waves.

## Задачи (red-first)
1. **P0-1**: `ApiAuth::from_token(token, &SuperProperties, locale, timezone)` — 6 заголовков
   непустые, `Authorization` в правильном виде (для user-token — как в захвате; если схема
   неясна — параметризуй `scheme` и оставь TODO до NET-A), `Debug` не печатает секрет.
   Тест: заголовки в порядке, токен не в Debug.
2. **P0-4**: `rest/user.rs`: `Me { id, username, discriminator, global_name, avatar, bot }` +
   `parse_me(&[u8])` через `discord-json`; ошибки типизированы; тест на санитизированном
   сэмпле и на битом JSON/401-теле.
3. **P1-2**: `HttpRequest::Debug` — заголовки с `Authorization`/`Cookie` → `<redacted>`,
   тест.
4. **P1-1**: `IdentifyProperties::from_super_properties(&SuperProperties)` — один источник
   launch_id/UA/версии; тест на совпадение полей.
5. **P1-4**: `build_identify` с пустым токеном → `IdentifyError::EmptyToken`, тест.

## Проверки
```bash
cd rust && cargo test --offline -p discord-core
cargo clippy --offline -p discord-core --all-targets -- -D warnings
cargo fmt -p discord-core -- --check
```
Без сети, окна, GUI, commit. Один cargo за раз.

## Отчёт
`waves/RW-39/NET/PM/netc1/REPORT.md`: пункт → red → green, тесты, md5, что осталось
(вход/logout/шлюз — NET-C2/E).
