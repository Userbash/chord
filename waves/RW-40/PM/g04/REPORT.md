# G-04 — backend выхода с сервера (red-first)

Дата: 2026-10-24. Воркер: G1 Backend/Network. PM: DeepSeek.
Статус: **готово к приёмке**. Live/окно/сеть/commit не использовались.

## Контракт (источник)

`waves/RW-40/NETWORK-CONTRACTS.md` §1.1 #4 (`B` anchor `async leaveGuild(e)`; `RS:786`),
`waves/RW-40/PARITY/PARITY-MATRIX.md` G-04:

- `DELETE /users/@me/guilds/{id}`, тело `{lurking: bool}` — **[S]**;
  в call-site `lurking = isLurking(id) || isCurrentUserGuest(id)`, т.е. это решение
  UI/caller'а, а не модуля.
- Ответ не читается (в клиенте только `.then()`), успех — 2xx (204/200).
- HTTP-коды и тело ошибки — **[A]**: реализация их не выдумывает. Типизированы
  401/403/404/429; 4xx-прочие → `Client{status}`, 5xx → `Server{status}` (политика
  G1-01 из `rest_failure.rs`: статус не теряется), транспорт/1xx/3xx → `Network`.
- `POST /guilds/{id}/delete` (delete guild владельцем) **не** реализован — другой flow;
  тест это фиксирует (`target` не оканчивается на `/delete`, нет `members/@me`).

## Сделано

`rust/gui-spike/src/guilds.rs`:

- `LeaveGuildError` (`InvalidArg`, `MissingToken`, `Unauthorized`, `Forbidden`,
  `NotFound`, `RateLimited{retry_after_ms}`, `Network`, `InvalidResponse` + аддитивные
  `Client{status}`/`Server{status}` из политики G1-01), `Display`, `Error`, serde
  `{kind: camelCase}`; без паник, без токена/тела в сообщениях.
- `From<RestFailure>` и `From<EnterGuildError>` (команда выхода переиспользует
  `DiscordGuilds::new`, второго конструктора с копией проверки токена нет).
- Трейт `GuildLeaving::leave_guild(guild_id, lurking)`; свободная
  `leave_guild(&guilds, id, lurking)` валидирует снежинку **до** сети.
- `GuildTransport` дополнен `delete(target, body)`; боевая реализация — через
  `ApiClient::execute(HttpRequest::new(Delete, ..).body(..))` (в ядре нет
  `delete` с телом). `discord-core` **не менялся**.
- Тело — `serde_json::json!({"lurking": bool})`, ровно одно логическое поле;
  `validate_guild_id`/`validate_leave_guild_id` используют общий `is_snowflake`.
- Ретраев нет (`max_attempts: 1`, изменяющая операция) — как у входа.

`rust/gui-spike/src/main.rs`:

- Tauri-команда `leave_guild(backend, guild_id: String, lurking: bool)`: токен только
  через `token_or` (`keyring-сбой → Network`, `нет токена → MissingToken`), аргумента
  токена нет, логирования нет; зарегистрирована в `invoke_handler!` один раз.
- `TOKEN_COMMANDS` расширен `leave_guild`; в тест `token_or_preserves_domain_error_codes`
  добавлены кейсы `LeaveGuildError`.

`ui/**` не менялся (read-only соблюдён). `tests/guilds_contract.rs` — 11 новых тестов,
старые 13 не ослаблены.

## RED

`waves/RW-40/PM/g04/red.log` — тесты написаны до модуля, `exit=101`:

```
error[E0432]: unresolved imports `discord_gui_spike::guilds::leave_guild`,
  `discord_gui_spike::guilds::GuildLeaving`, `discord_gui_spike::guilds::LeaveGuildError`
error[E0407]: method `delete` is not a member of trait `GuildTransport`
error: could not compile `discord-gui-spike` (test "guilds_contract") due to 2 previous errors
```

## GREEN

Targeted: `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test guilds_contract`
— **24 passed, 0 failed** (13 прежних + 11 новых).

Полные прогоны:

| Команда | Итог | Лог |
|---|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | **185 passed**, 0 failed (было 174) | `green-gui.log` |
| `cargo test --offline -p discord-core` (из `rust/`) | **574 passed**, 0 failed (без изменений) | `green-core.log` |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | чисто, `exit=0` | `clippy.log` |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | чисто, `exit=0` | `fmt.log` |

Новые тесты (фейковый транспорт, без сети):

1. `leave_guild_deletes_users_me_guilds_route_with_lurking_true` — точный target
   `/users/@me/guilds/123456789012345678`, тело `{"lurking":true}`, не `delete`/не `members`.
2. `leave_guild_sends_explicit_false_lurking` — `{"lurking":false}`, JSON-объект с одним полем.
3. `leave_guild_accepts_2xx_without_reading_body` — 200/201/204/299 → `Ok`.
4. `leave_guild_maps_http_statuses_to_typed_errors` — 401/403/404/429, 400/409, 500/502.
5. `leave_guild_transport_failure_is_typed_network_error`.
6. `leave_guild_invalid_guild_id_is_rejected_before_transport` — 10 видов мусора, сеть не тронута.
7. `leave_guild_without_token_is_missing_token` — пустой/пробельный токен.
8. `leave_errors_serialize_with_camel_case_kind_only` — 10 вариантов, `kind` camelCase.
9. `leave_guild_free_function_validates_before_delegating` — фейк `GuildLeaving`.
10. `leave_guild_free_function_preserves_typed_error`.
11. `tauri_registers_leave_guild_once_and_reads_token_via_helper` — одна регистрация,
    `token_or`, нет токена из фронтенда, нет логов, `lurking: bool`, `MissingToken`.

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/guilds.rs` | `2959299eab57ae6c9c6ceb8009e9ec7b` |
| `rust/gui-spike/src/main.rs` | `6eb14d35bd71ba9f1756c79b2b9994b5` |
| `rust/gui-spike/tests/guilds_contract.rs` | `a293fd8b9ae442b1b0a177ea18151786` |
| `rust/crates/discord-core/src/rest/client.rs` | `6ca55bd6e5b4fb06350107874b22526f` (не менялся в G-04) |

## Риски и остаток

- **[A] HTTP-коды leave** live не подтверждены: 403 у владельца сервера, 404 при
  отсутствии участия, тело 429 (`retry_after`) не разбирается — задержка 0, как у G-03a.
  Проверка — только в согласованном окне (не в этой задаче).
- **`lurking` — ответственность caller'а.** Если UI передаст `false` для lurker/guest,
  возможен 403; маппинг `isLurking || isGuest` — часть G-04 UI (вне моего скоупа).
- **Аддитивные `Client`/`Server`** в `LeaveGuildError` — сознательное расширение списка
  из ТЗ, чтобы 400/5xx не схлопывались в `Network` (регрессия G1-01). Если PM требует
  ровно 8 вариантов — скажите, сверну в `Network` (потеря статуса будет задокументирована).
- **Нет UI-пункта «Покинуть сервер»/confirm** и нет обновления списка/канала после
  выхода — это G-04 UI-часть (`ui/**` read-only для меня).
- `discord-core` не менялся: `delete` с телом собран из публичного `HttpRequest` +
  `ApiClient::execute`; отдельного метода в ядре нет (при переиспользовании вне
  guilds — кандидат на аддитивный `ApiClient::delete_with_body`).
