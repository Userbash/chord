# BRIEF NET-E2 — connect/READY из приложения

Владелец: G1 Network lead. PM: DeepSeek. Red-first. Сеть — только стабы.
Вход: `NET/AUTH-CONTRACT.md`, `AUTH-RE` G-04…G-20, `NET/PM/netc2/REPORT.md`, core supervisor.

## Файлы
- `rust/gui-spike/src/net.rs` (новый), `src/main.rs` (регистрация), `src/lib.rs`
- `rust/gui-spike/tests/net_contract.rs` (новый)
- логи `NET/PM/nete2/`
READ-ONLY: `core/**`, `ui/**`, `fixtures/**`.

## Задачи
1. `net_connect()`: токен из keyring → `GatewayUrl` → `Supervisor` (core) → IDENTIFY; статус.
2. `net_status()`: `offline/connecting/ready/reconnecting/error`; без токена.
3. `net_disconnect()`: stop supervisor, идемпотентно.
4. Ошибки: 4004 → Unauthorized (logout), 4013/4014 → Stop, 429/сеть → backoff.
5. Событие `gateway://status` (если возможно) или снимок статуса.
6. Тесты: фейковый транспорт/супервайзер, 4004, reconnect, stop; токен не в логах.

## Проверки
```bash
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
```
Без live/окна/commit.

## Отчёт
`NET/PM/nete2/REPORT.md`: команды/статусы, red→green, md5, остаток (live READY).
