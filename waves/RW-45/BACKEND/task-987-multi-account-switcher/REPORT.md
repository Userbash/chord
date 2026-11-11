# Отчёт по задаче TASK-987: Мгновенное переключение аккаунтов (Multi-Account Switcher & Instant Handoff)

## 1. Задача (Batch / Карточка)
- **ID карточки:** `TASK-987` (`waves/kanban-board.json`)
- **Отдел:** `G1 Backend`
- **Приоритет:** `P2`
- **Краткое описание:** Хранение до 5 профилей пользователей (`MAX_MULTI_ACCOUNTS = 5`) с криптографической изоляцией токенов разных аккаунтов (`SafeStorageTokenVault`, `com.discordlab.Token`, `dQw4w9WgXcQ:`) и мгновенным переключением Gateway и REST сессий (`< 300 мс`) без перезапуска GPUI клиента.

## 2. Сделано
1. **Сверка с эталонным RE-дампом `/home/sanya/re-deep/` и независимыми оракулами (`AGENTS.md` §7):**
   - Модуль `274303` (`MultiAccountStore` в `6640343be900b315.js`): жёсткий лимит `MAX_MULTI_ACCOUNTS = 5` с автоматическим вытеснением `splice(5).forEach(e => removeToken(e.id))` и статусы токенов `INVALID = 0`, `VALIDATING = 1`, `VALID = 2`.
   - Модуль `376463` (`MultiAccountSwitchStore`): отслеживание состояния переключения (`is_switching_account`, `target_user_id`, `switch_origin`), защита от коллизии токенов между разными аккаунтами и обработка кодов закрытия шлюза `4004` / `40004` с откатом к предыдущей сессии.
   - Модуль `429707` (`MultiAccountActionCreators`): валидация токена через `GET /api/v9/users/@me` (`docs/re/rest-d02.json`) с инвалидацией при `401`/`403` или несовпадении `user_id` (`PerUserTokenMismatch`) и сохранением валидности при `429` (`retry_after_ms`) / `5xx`.
   - Модуль `247775` (`TokenManager` / `safeStorage`) и `docs/re/backend-ipc.json`: 3 IPC-канала `DISCORD_SAFE_STORAGE_ENCRYPT_STRING`, `DISCORD_SAFE_STORAGE_DECRYPT_STRING`, `DISCORD_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE`, префикс `dQw4w9WgXcQ:`, схема `com.discordlab.Token` (`rust/crates/discord-platform/src/keyring.rs`), а также защита `hide_tokens` (`TokensHiddenInDevtools`).
   - Геометрия и дизайн-токены `Account Switcher Popup` (`waves/kanban-board.json` и `rust/discord-gpui/src/ui/account_switcher.rs`): `280px x auto`, `padding: 8px`, `spacing: 4px`, цвета `#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`, интерактивные состояния `inactive`, `active avatar`, `switched`.
2. **Реализация `rust/discord-gpui/src/multi_account_switcher.rs`:**
   - `#![forbid(unsafe_code)]`, 0 `.unwrap()` / `.expect()` в продакшен-коде, `< 50` строк на каждую функцию, `774` строки в файле (`< 800`), чистое форматирование `rustfmt --edition 2024`.
   - Реализованы `SafeStorageTokenVault`, `IsolatedEncryptedToken` (с автоматическим занулением буфера `zeroize_slice` при `Drop` и удалении/вытеснении), `AccountProfile`, `SwitchHandoffPhaseTimings`, `SessionHandoffReceipt`, `MultiAccountSwitcher`, `validate_workload_boundaries`, `redact_switcher_secrets` и `export_switcher_telemetry_json`.
3. **Покрытие 9 обязательных измерений `TEST-POLICY.md` в `tests/unit/multi-account-switcher-987.test.mjs`:**
   - Позитивные, негативные, граничные (`Snowflake 0`/`u64::MAX`, `5` vs `6` профилей, `299 мс` vs `300 мс`, `2000` символов, `10` файлов / `25 МБ`, `offset=9975`, громкость `0..200%`, бейдж `99+`), ошибки (`401/403/404/413/429 retry_after_ms/5xx`, Gateway `4004`), пустые (`empty`), загрузочные (`HTTP 202`), права доступа (`PERM_VIEW_CHANNEL`), восстановление (`handle_gateway_close_during_switch`) и наблюдаемость с редактированием секретов (`mfa.***`, `dQw4w9WgXcQ:[REDACTED]`, `[REDACTED_TOKEN]`).

## 3. Red-прогон (Strict TDD)
- Команда: `node --test tests/unit/multi-account-switcher-987.test.mjs` (до создания `rust/discord-gpui/src/multi_account_switcher.rs`)
- Результат: `tests 6 | pass 0 | fail 6` (зафиксирован в `waves/RW-45/BACKEND/task-987-multi-account-switcher/red.log`).

## 4. Green-прогон
- Команда: `node --test tests/unit/multi-account-switcher-987.test.mjs`
- Результат: `tests 6 | pass 6 | fail 0` за `860.66ms` (зафиксирован в `waves/RW-45/BACKEND/task-987-multi-account-switcher/green.log`).

## 5. Targeted & 5/5 Мутации
- Мутационный гейт (`SUB-987-5`): убито **5 из 5 мутаций (`100%` kill rate)**:
  1. `MUT-multi_account_switcher-01` (`MaxMultiAccountsCapRelaxed`: `5 -> 8`) — killed
  2. `MUT-multi_account_switcher-02` (`SwitchHandoffSlaRelaxed`: `>= MAX_SWITCH_HANDOFF_MS -> > MAX_SWITCH_HANDOFF_MS`) — killed
  3. `MUT-multi_account_switcher-03` (`CrossAccountTokenIsolationBypass`: отключение проверки `requester_user_id != target_user_id`) — killed
  4. `MUT-multi_account_switcher-04` (`TokenCollisionGuardBypass`: отключение детектора коллизий отпечатков токенов между аккаунтами) — killed
  5. `MUT-multi_account_switcher-05` (`SessionStateIsolationClearOmitted`: `guilds_cleared: true -> false`) — killed
- Лог сохранён в `waves/RW-45/BACKEND/task-987-multi-account-switcher/mutation.log`.

## 6. Файлы и MD5
- `rust/discord-gpui/src/multi_account_switcher.rs` — `dd34b65e84812bdaca81ceca8ecb2e55`
- `tests/unit/multi-account-switcher-987.test.mjs` — `624e9cb98eb3c9c4a7e6788299b4e983`
- `waves/RW-45/BACKEND/task-987-multi-account-switcher/red.log` — `a7f1ff92c3a741f86fa06e5b8bc8215f`
- `waves/RW-45/BACKEND/task-987-multi-account-switcher/green.log` — `5e5d901eceb0f70cccc7b5ac9de80fa2`
- `waves/RW-45/BACKEND/task-987-multi-account-switcher/mutation.log` — `3054ec49645ca1788ac1eb555b054566`

## 7. Риски и заметки для интегратора
- Общие файлы (`rust/discord-gpui/src/main.rs`, `rust/discord-gpui/src/state/mod.rs`, `waves/kanban-board.json`) не модифицировались воркером согласно `AGENTS.md` §1.2; подключение `pub mod multi_account_switcher;` в корень крейта выполняется на интеграционном шаге.
