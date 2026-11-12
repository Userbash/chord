# REPORT — TASK-611 (G1 Backend): REST `POST /guilds` для создания сервера пользователем

- **Ветка:** `feature/task-611-rest-guild-create`
- **Рабочее дерево:** `/home/user/orca/workspaces/project/task-611-rest-guild-create`
- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Затронутые карточки:** `TASK-611` (`REST POST /api/v9/guilds`)
- **Дата:** 2026-11-07
- **Коммитов нет:** `git commit` / `git push` не выполнялись (правила пайплайна `AGENTS.md` §6).

---

## 1. Сделано

1. **Изучены RE-оракулы и спецификации:**
   - `AGENTS.md`, `PROCESS.md` §4.2, `docs/re/rest-d02.md` §6.1–§6.2 (полный набор из 6 auth-заголовков Discord: `Authorization`, `X-Super-Properties`, `X-Discord-Locale: ru`, `X-Discord-Timezone`, `X-Debug-Options: bugReporterEnabled`, `Accept-Language: ru` + `Content-Type: application/json`).
   - `reference-oracle/rest-static.json` (`Endpoints.GUILDS = "/guilds"`).
   - `reference-oracle/out/bundles/web.4d4b2f75fdea5470.js`:
     - битовая маска `system_channel_flags` (`1 << 0` `SUPPRESS_JOIN_NOTIFICATIONS`, `1 << 1` `SUPPRESS_PREMIUM_SUBSCRIPTIONS`, `1 << 2` `SUPPRESS_GUILD_REMINDER_NOTIFICATIONS`, `1 << 3` `SUPPRESS_JOIN_NOTIFICATION_REPLIES`);
     - коды ошибок `400` (`MAXIMUM_GUILDS = 30005` / `TOO_MANY_USER_GUILDS = 30001`) и `403` (`NO_VERIFIED_PHONE` / `40002`).
2. **Реализован модуль `rust/crates/discord-core/src/rest/guilds.rs` (экспортирован в `rust/crates/discord-core/src/rest/mod.rs`):**
   - Константы маршрута и протокола: `GUILDS_PATH` (`"/guilds"`), `GUILDS_API_V9_PATH` (`"/api/v9/guilds"`), `DISCORD_LOCALE_RU` (`"ru"`), `MIN_GUILD_NAME_CHARS` (`1`), `MAX_GUILD_NAME_CHARS` (`100`), `DISCORD_CODE_MAXIMUM_GUILDS` (`30005`), `DISCORD_CODE_TOO_MANY_USER_GUILDS` (`30001`), `DISCORD_CODE_NO_VERIFIED_PHONE` (`"NO_VERIFIED_PHONE"`), `CHANNEL_TYPE_GUILD_TEXT` (`0`), `CHANNEL_TYPE_GUILD_VOICE` (`2`), `CHANNEL_TYPE_GUILD_CATEGORY` (`4`) и флаги `SYSTEM_CHANNEL_FLAG_*`.
   - Структуры запроса: `GuildCreateChannelSpec` (начальные категории `type: 4` и каналы `type: 0`/`2` с `id`, `name`, `kind`, `parent_id`, `position`) и `GuildCreateRequest` (`name`, `icon`, `channels`, `system_channel_flags`, `guild_template_code`) с валидацией длины имени `1..=100` символов, base64 DataURI иконки (`image/png`, `image/jpeg`, `image/jpg`, `image/gif`, `image/webp`) и сериализацией JSON по RFC 8259.
   - Сборка заголовков и HTTP-запроса: `build_guild_create_headers` и `build_guild_create_http_request` с обязательными заголовками Discord (`X-Super-Properties`, `X-Discord-Locale: ru`, `Accept-Language: ru`, `X-Discord-Timezone`, `X-Debug-Options: bugReporterEnabled`, `Authorization`, `Content-Type: application/json`).
   - Парсер ответа `parse_guild_create_response`:
     - `201 Created` (`200..300`) → строгий разбор `CreatedGuild` (`id`, `name`, `icon`, `owner_id`, `system_channel_id`, `system_channel_flags`), начальных каналов (`Vec<CreatedGuildChannel>`) и ролей (`Vec<CreatedGuildRole>`);
     - `400 Bad Request` с `code: 30005` (`MAXIMUM_GUILDS`) или `30001` → `GuildCreateRestError::MaximumGuilds { code, message }`;
     - `403 Forbidden` с `code: "NO_VERIFIED_PHONE"` или `40002` → `GuildCreateRestError::NoVerifiedPhone { message }`;
     - `401`, `403`, `429` (`retry_after_ms`), `5xx` → соответствующие варианты `GuildCreateRestError`.
3. **Реализован клиентский вызов и интеграция в локальное состояние в `rust/gui-spike/src/guilds.rs`:**
   - Добавлены структуры `CreateGuildChannelInput`, `CreateGuildRequest`, `CreateGuildFullOutcome`, методы `DiscordGuilds::from_api_client`, `DiscordGuilds::create_guild_full` и функция `create_guild_full`.
   - В `CreateGuildError` добавлены варианты `MaximumGuilds { code: u32 }` (`"kind": "maximumGuilds"`) и `NoVerifiedPhone` (`"kind": "noVerifiedPhone"`).
   - В `ApiGuildTransport::post` переход на `post_once` с заголовками `X-Discord-Locale: ru` и `Accept-Language: ru` сохраняет JSON-тело ответов `400`/`403` для точной классификации `MAXIMUM_GUILDS 30005` и `NO_VERIFIED_PHONE`.
   - При успешном ответе `201 Created` созданная гильдия, её начальные каналы и роли без падения интегрируются в локальное состояние (`local_created_guild_snapshot` + `crate::data::ingest_accepted_invite_payload` / `ensure_local_guild_present`), сохраняя полную обратную совместимость с `guilds_contract.rs`.

---

## 2. Строгий TDD (Red → Green)

### Red-прогон (до реализации):

- **Команда:**
  `CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test guild_create_contract`
- **Результат:** `exit 101` (ошибка компиляции из-за отсутствия модуля `discord_core::rest::guilds`, типов `CreateGuildRequest`/`CreateGuildFullOutcome` и вариантов `CreateGuildError::MaximumGuilds` / `NoVerifiedPhone`):
  ```text
  error[E0432]: unresolved import `discord_core::rest::guilds`
  error[E0432]: unresolved imports `discord_gui_spike::guilds::clear_local_created_guilds`, `discord_gui_spike::guilds::create_guild_full`, `discord_gui_spike::guilds::local_created_guild_snapshot`, `discord_gui_spike::guilds::CreateGuildChannelInput`, `discord_gui_spike::guilds::CreateGuildFullOutcome`, `discord_gui_spike::guilds::CreateGuildRequest`
  error[E0599]: no variant named `MaximumGuilds` found for enum `CreateGuildError`
  error[E0599]: no variant, associated function, or constant named `NoVerifiedPhone` found for enum `CreateGuildError` in the current scope
  error[E0599]: no associated function or constant named `from_api_client` found for struct `DiscordGuilds` in the current scope
  error: could not compile `discord-gui-spike` (test "guild_create_contract") due to 5 previous errors
  ```

### Green-прогон (после реализации):

1. **Целевой контрактный тест `guild_create_contract`:**
   - **Команда:**
     `CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test guild_create_contract`
   - **Результат:** `8 passed; 0 failed; 0 ignored` (exit 0, прогнано 5 раз подряд — `flaky_rate = 0%`).
2. **Регрессионный контрактный тест `guilds_contract`:**
   - **Команда:**
     `CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test guilds_contract`
   - **Результат:** `31 passed; 0 failed; 0 ignored` (exit 0).
3. **Форматирование и линтер (`rustfmt` + `clippy`):**
   - `rustfmt --edition 2024 --check rust/crates/discord-core/src/rest/mod.rs rust/crates/discord-core/src/rest/guilds.rs rust/gui-spike/src/guilds.rs rust/gui-spike/tests/guild_create_contract.rs` → `exit 0` (чисто).
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test guild_create_contract` → `0 warnings` в затронутых файлах (`guilds.rs`, `guild_create_contract.rs`).

---

## 3. Мутационная проверка (5/5 убито)

Проверены 5 мутаций на `rust/crates/discord-core/src/rest/guilds.rs` и `rust/gui-spike/src/guilds.rs` против `guild_create_contract`:
- `M1_locale_header` (`DISCORD_LOCALE_RU = "ru"` → `"en-US"`): **KILLED** (`exit=101`)
- `M2_name_max_len` (`MAX_GUILD_NAME_CHARS = 100` → `99`): **KILLED** (`exit=101`)
- `M3_maximum_guilds_400` (`if status == 400` → `if status == 499`): **KILLED** (`exit=101`)
- `M4_no_verified_phone_403` (`if status == 403` → `if status == 498`): **KILLED** (`exit=101`)
- `M5_local_state_integration` (отключение `integrate_created_guild_into_local_state` в `execute_create_full`): **KILLED** (`exit=101`)
- После мутационной проверки файлы восстановлены бит-в-бит (`md5sum` совпал).

---

## 4. Затронутые файлы и `md5sum`

```text
7db7e6cc674ae4a30bb2e986e218cbc2  rust/crates/discord-core/src/rest/mod.rs
a6c6f237c0193544d45dc07c5dfc42b4  rust/crates/discord-core/src/rest/guilds.rs
44b8e8bf13c9e2e70b5a0ff1346e6fb3  rust/gui-spike/src/guilds.rs
c353803ff5f79e0c81cc1b40862e0894  rust/gui-spike/tests/guild_create_contract.rs
```

---

## 5. Остаточные риски и замечания

- Все существующие 31 теста в `rust/gui-spike/tests/guilds_contract.rs` полностью сохранены и проходят без изменений публичных сигнатур `CreateGuildPayload` и `CreateGuildOutcome`.
- Токен авторизации никогда не попадает в `Debug`/`Display` ни `HttpRequest`, ни `GuildCreateRestError`/`CreateGuildError`.
