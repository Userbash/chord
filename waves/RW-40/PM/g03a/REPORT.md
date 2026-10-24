# G-03a — backend входа в сервер из preview/lurker (red-first)

Дата: 2026-10-24. Воркер: G1 Backend/Network. PM: DeepSeek.
Статус: **готово к приёмке**. Live/окно/сеть/commit не использовались.

## Контракт (источник)

`waves/RW-40/NETWORK-CONTRACTS.md` §1.1 #2 (`D02:101,138,225`, requestId
`ready|57992.6989`, `run2|95974.7409`), `waves/RW-40/PARITY/PARITY-MATRIX.md` G-03a:

- `PUT /guilds/{id}/members/@me?lurker=true`
- тело — ровно `{}` (`body_keys = ["(empty-object)"]` в оракуле `D02`, `routes.rs` уже содержит маршрут)
- ответ — **204** без тела
- `POST /invites/{code}` (join по инвайту) **не** реализован — другой flow, другие поля.

## Сделано

Новый модуль `rust/gui-spike/src/guilds.rs`:

- `EnterGuildError` (`InvalidArg`, `MissingToken`, `Unauthorized`, `Forbidden`, `NotFound`,
  `RateLimited{retry_after_ms}`, `Network`, `Parse`), `Display`, `Error`, serde
  `{kind: camelCase}`; без паник, без токена/тела в сообщениях.
- `DiscordGuilds::new(token)`: пустой/пробельный токен → `MissingToken`; ручной `Debug`
  (`finish_non_exhaustive`) — токен не печатается.
- `DiscordGuilds::enter_guild` / трейты `GuildMembership`, `GuildTransport` (фейк в тестах),
  свободные `enter_guild(..)` / `join_guild(..)` — одна валидация на всех входах:
  `guild_id` — только цифры и влезает в `u64` (снежинка, ≤20 цифр), иначе `InvalidArg`
  **до** транспорта.
- Запрос: `routes::find(Put, "/guilds/{id}/members/@me")` + `render_path` + `?lurker=true`,
  тело `b"{}"`. Ретраев нет (`max_attempts: 1`): операция изменяющая.
- Маппинг статусов: `204 → Ok`; `401 → Unauthorized`; `403 → Forbidden`; `404 → NotFound`;
  `429 → RateLimited{0}` (тело 429 для этого маршрута не подтверждено — задержку не выдумываем);
  прочие не-2xx → `Network`; **иной 2xx (200/201/…) → `Parse`** — единственный live-подтверждённый
  успех это 204, соглашаться на неизвестный успех нельзя.
- `ApiError` → `EnterGuildError`: `Unauthorized`, `RateLimited` (retry_after из ответа),
  `Client{403}→Forbidden`, `Client{404}→NotFound`, остальное → `Network`.

`rust/gui-spike/src/main.rs`: Tauri-команды `join_guild` и `enter_guild` (оба в
`invoke_handler!`). Токен берётся только через общий `guild_token(backend) → backend.token()`
в `spawn_blocking`; аргумента токена нет, логирования нет. `enter_guild` — синоним для UI.

`rust/gui-spike/src/lib.rs`: `pub mod guilds` + док-абзац.

`rust/crates/discord-core/src/rest/routes.rs` **не менялся**: маршрут `PUT /guilds/{id}/members/@me`
с `body_keys = ["(empty-object)"]` уже в каталоге и в оракуле `tests/rest_routes.rs`.

## RED

`waves/RW-40/PM/g03a/red.log` — тесты написаны до модуля:

```
error[E0432]: unresolved import `discord_gui_spike::guilds`
 --> tests/guilds_contract.rs:8:24
8 | use discord_gui_spike::guilds::{
  |                        ^^^^^^ could not find `guilds` in `discord_gui_spike`
error: could not compile `discord-gui-spike` (test "guilds_contract") due to 1 previous error
```

`exit=101`. Других правок продуктового кода до red не было.

## GREEN

Targeted (после реализации): `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test guilds_contract`
— **12 passed, 0 failed**.

Полные прогоны (`waves/RW-40/PM/g03a/green-*.log`):

| Команда | Итог |
|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | **161 passed**, 0 failed (было 149 + 12 новых) |
| `cargo test --offline -p discord-core` (из `rust/`) | **574 passed**, 0 failed (без изменений) |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | чисто, `exit=0` |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | чисто, `exit=0` |

Тесты `tests/guilds_contract.rs` (фейковый транспорт, без сети):

1. `enter_guild_puts_lurker_query_with_empty_json_object` — точный target
   `/guilds/123456789012345678/members/@me?lurker=true`, тело `{}`, нет `invites`/`session_id`.
2. `enter_guild_accepts_204_without_body` — 204 и пустое тело → `Ok`.
3. `http_statuses_map_to_typed_errors` — 401/403/404/429/500/400/200/201.
4. `transport_failure_is_typed_network_error`.
5. `invalid_guild_id_is_rejected_before_transport` — 10 вариантов, транспорт не вызывается.
6. `empty_or_blank_token_is_missing_token`.
7. `client_debug_does_not_reveal_token`.
8. `errors_serialize_with_camel_case_kind_only`.
9. `free_functions_validate_before_delegating` (fake `GuildMembership`).
10. `free_functions_preserve_typed_error`.
11. `tauri_registers_join_and_enter_guild`.
12. `tauri_commands_take_no_token_and_read_backend` — нет `token: String`, нет `println!/eprintln!`,
    токен только из `backend.token()`.

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/guilds.rs` (new) | `db461cfbe3f3e1055995a88239b941bc` |
| `rust/gui-spike/tests/guilds_contract.rs` (new) | `0a26170f4c2d5931d33400c2d2747f79` |
| `rust/gui-spike/src/lib.rs` | `c37a2bd01d9439ae12ecb0b8d6acf913` |
| `rust/gui-spike/src/main.rs` | `b06451e4c66f761956c3ea425d5ded8f` |

`ui/**`, `fixtures/**`, `logs/`, `functions/`, `evidence/`, `core/src/rest/routes.rs` — не изменялись.

## Остаток / риски

- Live-проверка 204 (окно 1) — **не выполнялась** (запрет окна/live), fake-контракт покрыт.
- Тело 429 не разбирается: `retry_after_ms = 0`. Если окно покажет JSON `retry_after` — отдельная
  карточка, сейчас не выдумываем поля.
- Иной 2xx → `Parse` (осознанно строго, см. выше). Если live покажет 200 — правится одной ветвью `decode`.
- Две IPC-команды (`join_guild`/`enter_guild`) — тонкие синонимы поверх одной логики; если UI
  выберет одно имя, лишнее снимается без изменения модуля.
- `MissingToken` в Tauri: отсутствие токена в keyring и ошибка keyring (`AuthError`) сейчас
  различаются (`MissingToken` vs `Network`) — как в `send_message`/`toggle_reaction`.
- Коммит не делался (уровень L1).
