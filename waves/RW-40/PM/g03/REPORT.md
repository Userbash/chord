# G-03 — backend resolve/accept invite (red-first)

Дата: 2026-10-24. Воркер: G1 Backend (subagent PM DeepSeek), batch RW-40.
Статус: **готово к приёмке**. Окно/live/сеть/commit не использовались.

## 1. Контракт (источник)

`waves/RW-40/NETWORK-CONTRACTS.md` §1.1 #1/#12, §1.2, §1.4;
`waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.3. В frozen D-02 invite-ручек нет
(47 путей, 0 invite), поэтому оракул — отдельный `[S]` (D-108, как у `/typing`).

- Resolve: `GET /invites/{code}` + подтверждённые якорем
  (`with_counts:!0,with_expiration:!0`) флаги → в коде только
  `?with_counts=true&with_expiration=true`. Прочие query-параметры (`inputValue`,
  `with_games`, `with_permissions`, …) — analytics/контекст UI, не отправляются.
- Accept: `POST /invites/{code}`, тело **ровно**
  `{session_id, invite_instance_id, received_installation_id}`, все три nullable
  (`getSessionId()` в бандле → `null`; `invite_instance_id` может отсутствовать;
  `received_installation_id` может быть `null`). `context` — analytics-объект
  запроса, в тело **не** попадает. Ответ — invite-объект, из него берутся
  `guild_id`, `target_channel_id?`, `target_message_id?`.
- HTTP-коды и TTL — `[A]` (live не наблюдалось): не выдумываем. Любой 2xx обязан
  принести разбираемый invite-объект; иначе `invalidResponse`.

## 2. Сделано

`rust/gui-spike/src/invites.rs` (new):

- `InvitePreview {code, guildId, guildName, icon?, memberCount?, presenceCount?,
  expiresAt?, channelId?}` и `AcceptedInvite {guildId, targetChannelId?,
  targetMessageId?}` — `Serialize`+`Deserialize`, IPC-camelCase, опциональные поля
  не сериализуются, если сервер их не прислал.
- `AcceptRequest` (все три поля `Option<String>`, `Default` = все `null`);
  тело сериализуется структурой, поэтому в JSON всегда ровно три ключа, `null`
  вместо отсутствующих.
- `InviteError` (`InvalidArg`, `MissingToken`, `Unauthorized`, `Forbidden`,
  `NotFound`, `RateLimited{retry_after_ms}`, `Client{status}`, `Server{status}`,
  `Network`, `Storage`, `InvalidResponse`), `Display`, `Error`, serde
  `{kind: camelCase}`; без паник, без токена/тела в сообщениях.
- `DiscordInvites::new(token)`: пустой/пробельный токен → `MissingToken`; ручной
  `Debug` (`finish_non_exhaustive`) — токен не печатается; ретраи `max_attempts: 1`
  (accept — изменяющая операция).
- Нормализация кода (`G6 §3.3`): trim → снять `?`/`#` → снять хвостовой `/` →
  последний сегмент (`…/invite/<code>`) → `[A-Za-z0-9-]`, длина 2…32. Наша
  валидация (`[A]`), не утверждение о Discord; код не может стать произвольным
  путём. Валидация — **до** транспорта.
- Resolve: `routes::find(Get, INVITE_PATH)` + `render_path` + подтверждённый
  query; Accept: `find(Post, INVITE_PATH)`, тело из `AcceptRequest`.
- Разбор ответа: `guild` обязателен (групповой DM — не цель flow), `code`/
  `guild.id`/`guild.name`/`guild_id` — непустые строки; `channel` отсутствует/`null`
  → `None`, присутствует без `id` → `InvalidResponse`; счётчики `u64`,
  `expires_at` — строка ISO-8601 (`null` → `None`, число → `InvalidResponse`).
- Статусы: 401→`Unauthorized`, 403→`Forbidden`, 404→`NotFound`,
  429→`RateLimited{0}` (тело не разбираем), прочие 4xx→`Client{status}`,
  5xx→`Server{status}`, 1xx/3xx/транспорт→`Network`; **2xx без объекта** (в т.ч.
  204/пустое тело)→`InvalidResponse` — успех не подтверждён.

`rust/crates/discord-core/src/rest/routes.rs` (`[S]`-расширение, как у `/typing`):

- `INVITE_PATH = "/invites/{code}"`, `INVITE_ACCEPT_BODY_KEYS = ["session_id",
  "invite_instance_id", "received_installation_id"]`;
- `find` резолвит `GET`+`POST` по одному шаблону через статический
  `EXTENSIONS` (typing + invites); `ROUTES` остаётся 49 строк, оракул D-02 не
  размывается;
- `PLACEHOLDERS` дополнен `{code}` (иначе `render_path` не подставил бы код);
  `{id}`/`{num}` и все live-маршруты не тронуты.

`rust/gui-spike/src/main.rs`: Tauri-команды `resolve_invite` / `accept_invite`
(обе в `invoke_handler!`), bearer только через `token_or` (`Storage`/`MissingToken`),
токен не приходит из фронтенда и не логируется; аргументы accept —
`session_id/invite_instance_id/received_installation_id: Option<String>`.
`TOKEN_COMMANDS` в wiring-тесте расширен до 12 (обе команды).

`rust/gui-spike/src/lib.rs`: `pub mod invites` + док-абзац.

UI (`rust/gui-spike/ui/**`, `src/**` Node), `guilds`/leave, `fixtures/**`,
`logs/`, `functions/`, `evidence/` — не изменялись.

## 3. RED (до кода)

`waves/RW-40/PM/g03/red.log` — тесты написаны до модуля, `cargo test --test
invites_contract`:

```
error[E0432]: unresolved import `discord_gui_spike::invites`
  --> tests/invites_contract.rs:25:24
error[E0425]: cannot find value `INVITE_PATH` in module `routes`
error[E0425]: cannot find value `INVITE_ACCEPT_BODY_KEYS` in module `routes`
error: could not compile `discord-gui-spike` (test "invites_contract") due to 3 previous errors
```

exit=101. Продуктового кода до red не было.

## 4. GREEN

Targeted: `--test invites_contract` → **14 passed, 0 failed** (`green-gui.log`).

| Команда | Итог |
|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | **231 passed**, 0 failed (было 217 + 14) |
| `cargo test --offline --manifest-path rust/Cargo.toml -p discord-core` | **574 passed**, 0 failed (без изменений) |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | чисто, exit=0 |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | чисто, exit=0 |
| `cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | чисто, exit=0 |

Логи: `red.log`, `targeted.log`, `green-gui.log`, `green-core.log`, `clippy.log`.

Тесты `tests/invites_contract.rs` (fake-транспорт, без сети):

1. `resolve_gets_invite_target_with_counts_and_expiration` — точный target
   `/invites/abc123?with_counts=true&with_expiration=true`, только GET.
2. `resolve_maps_wire_object_to_camel_case_preview` — вложенные `guild`/`channel`,
   счётчики, `expires_at`, IPC-camelCase; минимальный объект → пустые опциональные
   поля не сериализуются.
3. `codes_are_normalized_and_invalid_ones_never_reach_transport` — 12 валидных
   входов (trim, `/`, ссылки, query/fragment, регистр), 13 невалидных → `InvalidArg`
   без GET/POST.
4. `accept_posts_exactly_three_nullable_fields_without_context` — target без query,
   ровно три ключа, `null` при `None`, значения при `Some`, нет `context`/`code`.
5. `accept_maps_response_to_typed_receipt` — `guildId`/`targetChannelId`/
   `targetMessageId` + минимальный ответ.
6. `http_statuses_map_to_typed_errors` — resolve и accept: 401/403/404/429/
   400/500/503, по одному запросу на кейс.
7. `success_without_parsable_invite_object_is_invalid_response` — пустое тело,
   массив/`null`/строка, нет `guild`, пустые id/name/code, `channel:{}`,
   отрицательный счётчик, `expires_at:42`, битый JSON, 204; для accept — `{}`,
   `[]`, пустой/числовой `guild_id`, `target_channel_id:42`.
8. `transport_failure_is_typed_network_error` — GET и POST.
9. `empty_or_blank_token_is_missing_token`.
10. `client_debug_does_not_reveal_token`.
11. `errors_serialize_with_camel_case_kind_only` — 11 kind'ов, наружу только kind
    и числовая деталь.
12. `free_functions_normalize_and_delegate` — нормализация до API, nullable-поля
    без подстановок, типизированные ошибки сохраняются.
13. `invite_routes_are_s_catalog_extension_not_d02` — `[S]`-оракул: GET/POST
    резолвятся, `body_keys` дословно, `render_path` с `{code}`, `ROUTES` = 49,
    D-02 TSV без `/invites/`, точная пара метода/шаблона, typing-расширение цело.
14. `tauri_invite_commands_read_bearer_through_token_or_without_logging` —
    регистрация ровно по одному разу, `token_or::<InviteError`, одно
    `backend.token()`, `Storage`/`MissingToken`, нет токена из фронтенда и логов.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/invites.rs` (new) | `4ce58a583a2384d5f6b0b952e24f054b` |
| `rust/gui-spike/tests/invites_contract.rs` (new) | `cbfef8f6be091893081f0b1ffc20c125` |
| `rust/gui-spike/src/lib.rs` | `a9708bc5f05a0d6143bbb7b3799fc96e` |
| `rust/gui-spike/src/main.rs` | `ab26127ed4fd46cf33d3ca624b191d09` |
| `rust/crates/discord-core/src/rest/routes.rs` | `923ccb1f65dd5daeb4d7bfad96908b80` |

## 6. Остаток / риски

1. **Live-окно не запускалось** (запрет): коды 200/204/400/429, тело ошибки и TTL
   `received_installation_id` — `[A]`. Если окно покажет иной успех (204/пустое
   тело), меняется одна ветка `ensure_success`/`decode`.
2. **`USER_BANNED` отдельным текстом не реализован.** G6 §3.3 просит отдельный
   текст, но форма тела ошибки — `[A]`; разбирать `code`/`message` догадкой нельзя
   (правило «не выдумывать»). Отдельная карточка после live-окна: нужен захват
   тела 403/400 для invite-accept.
3. **`RateLimited{retry_after_ms}` из «сырого» статуса = 0**: тело 429 не
   разбирается (как в guilds/typing); если ядро уже ретраило, значение приходит
   из `ApiError` через `RestFailure`.
4. **Строгий 2xx**: любой успех без invite-объекта → `invalidResponse`. Это
   осознанно (успех accept не подтверждён live), но если Discord вернёт 204 —
   потребуется аддитивное поле/вариант, решение за PM.
5. **Нормализация кода** (снятие query/fragment, `[A-Za-z0-9-]`, 2…32) — наша
   политика (`[A]`); если live покажет иные наборы символов, правится одна
   функция `normalize_code`.
6. **TTL и очистка `received_installation_id`** — клиентская логика UI (G-03b),
   backend значение не хранит.
7. **UI-часть G-03 не делалась** (`#join-server-dialog`, pending-плитка) — это
   G2-лейн; backend-команды готовы.
8. Коммит не делался (уровень L1).
