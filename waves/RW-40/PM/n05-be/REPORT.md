# RW-40 / N-05-NOTIFICATIONS-BACKEND — настройки уведомлений гильдий и переопределения каналов (`N-05`)

- **batch/карточка:** `RW-40 / N-05-NOTIFICATIONS-BACKEND` (`waves/RW-40/NOTIFICATIONS-RE.md`, `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` §1.1–§1.6);
- **роль:** Воркер G1 GUI-Spike Rust Backend;
- **сделано:**
  1. В `rust/gui-spike/src/notifications.rs` (экспортирован в `rust/gui-spike/src/lib.rs`) реализованы доменные типы, константы и валидаторы настроек уведомлений:
     - уровни уведомлений `MessageNotificationLevel` (`ALL_MESSAGES = 0`, `ONLY_MENTIONS = 1`, `NO_MESSAGES = 2`, `NULL/Inherit = 3` — `3` допустим только для канала/категории);
     - режимы хайлайтов `NotifyHighlights` (`NULL = 0`, `DISABLED = 1`, `ENABLED = 2`);
     - таймер приглушения `MuteConfig { selected_time_window, end_time }` с проверкой по 6 каноническим пресетам `VALID_MUTE_TIME_WINDOWS = [900, 3600, 10800, 28800, 86400, -1]`, конструктором `MuteConfig::from_window(window, now_unix)` и RFC 3339 / ISO-8601 парсером/форматтером (`parse_iso8601_unix`, `format_iso8601_utc`);
     - битовые маски флагов гильдии (`2048 | 4096 | 8192 | 16384`) и канала (`512 | 1024 | 2048 | 4096 | 8192 | 16384`) и помощники `set_guild_unread_flag`, `clear_guild_unread_flags`, `set_channel_unread_flag`, `clear_channel_unread_flags` (без затирания старших битов `OPT_IN_ENABLED = 4096` и `FAVORITED = 2048`);
     - построители и валидаторы PATCH-пейлоадов `GuildNotificationSettingsPatch` (`PATCH /users/@me/guilds/{guild_id}/settings` и bulk-конверт `to_bulk_patch_value` для `PATCH /users/@me/guilds/settings`) и `ChannelNotificationOverridePatch` (карта `channel_overrides: { "<channel_id>": { ... } }`, включая `reset_override`).
  2. Реализован каскадный резолвер `resolve_effective_notifications(guild, parent_category, channel, now_unix) -> EffectiveNotificationPolicy`:
     - каскад уровня `channel (!= 3) -> parent_category (!= 3) -> guild -> default_message_notifications`;
     - расчёт эффективного `is_muted` (`guild_muted || category_muted || channel_muted`) с учётом истечения `mute_config.end_time < now_unix` (`is_entry_muted`, `is_temporarily_muted`);
     - `allow_no_messages = is_muted || effective_level == 2` (плюс подавление невключённых каналов при `GUILD_FLAG_OPT_IN_CHANNELS_ON`);
     - матрица срабатывания уведомлений и бейджей (`should_alert_mention`, `should_increment_badge`, `should_alert_regular_message`) для `MentionKind::{None, User, Role, Everyone, Here}` (прямой `@user` всегда увеличивает бейдж даже при `is_muted` и `NO_MESSAGES`).
  3. Реализован асинхронный трейт `NotificationTransport`, клиент `DiscordNotifications` и функции `update_guild_notification_settings` + `update_channel_notification_override`:
     - предсетевая валидация отвергает пустой `token` (`MissingToken`), пустой/небезопасный `guild_id` (`InvalidGuildId`), пустой/небезопасный `channel_id` (`InvalidChannelId`), недопустимый `message_notifications` (`InvalidMessageNotifications { value }`), битый `mute_config` (`InvalidMuteConfig`) и пустой PATCH (`EmptyPatch`) до любого вызова транспорта;
     - маппинг HTTP `200`/`204`, `401 Unauthorized`, `403 Forbidden`, `404 NotFound`, `429 RateLimited { retry_after_ms }`, `4xx Client`, `5xx Server`.
  4. Создан контрактный тест `rust/gui-spike/tests/notifications_contract.rs` (4 комплексных теста, сверяющих маршруты с `fixtures/rest/rest-static.json` и 58 записей / 1328 override с реальным `fixtures/gateway/by-type/002-READY.json`). Файл `rust/gui-spike/src/main.rs` не изменялся.

- **red:** `(cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test notifications_contract --lib) > waves/RW-40/PM/n05-be/red.log 2>&1` → `exit=101`, `test result: FAILED. 0 passed; 4 failed; 0 ignored`; лог: `waves/RW-40/PM/n05-be/red.log`.
- **green:** `(cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test notifications_contract --lib && cargo clippy --offline -p discord-gui-spike --lib --test notifications_contract -- -D warnings && rustfmt --edition 2021 --check src/lib.rs src/notifications.rs tests/notifications_contract.rs) > waves/RW-40/PM/n05-be/green.log 2>&1` → `exit=0`, `8 passed` (lib) + `4 passed; 0 failed` (`notifications_contract`), clippy 0 предупреждений, rustfmt чисто; лог: `waves/RW-40/PM/n05-be/green.log`.
- **targeted:**
  - `cargo test --offline -p discord-gui-spike --test notifications_contract --lib` → `8 passed` (lib) + `4 passed` (`notifications_contract`), `0 failed` (`exit=0`);
  - `cargo clippy --offline -p discord-gui-spike --lib --test notifications_contract -- -D warnings` → 0 предупреждений (`exit=0`);
  - `rustfmt --edition 2021 --check rust/gui-spike/src/lib.rs rust/gui-spike/src/notifications.rs rust/gui-spike/tests/notifications_contract.rs` → чисто (`exit=0`).
- **oracle:** `waves/RW-40/NOTIFICATIONS-RE.md` (§1–§12), `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` (§1.1–§1.6), `fixtures/rest/rest-static.json` (`USER_GUILD_SETTINGS`, `USER_GUILD_SETTINGS_BULK`), `fixtures/gateway/by-type/002-READY.json` (58 записей `user_guild_settings`, 1328 `channel_overrides`, 3 истёкших `end_time` 2021 г.).
- **мутации:** 5/5 `KILLED`, эквивалентных — 0, survivors — 0 (`waves/RW-40/PM/n05-be/mutations.log`):
  - `MUT-1-allow-guild-message-notifications-3` (md5 `fec5b64d0eab650072758126f24b0b92`) — разрешение кода `3` (`NULL`) для уровня гильдии в `from_guild_code` → `KILLED` (`exit=101`) тестами `guild_and_channel_patch_builders_and_validators_enforce_re_contract` и `transport_functions_reject_invalid_inputs_before_network_and_send_exact_patch_payloads`;
  - `MUT-2-ignore-mute-end-time-expiry` (md5 `c883d6518e3026007796a5d277ca38ad`) — игнорирование истечения `end_time < now_unix` в `is_entry_muted` → `KILLED` (`exit=101`) тестами `oracle_ready_and_rest_static_parity_for_user_guild_settings` и `cascade_resolver_resolves_hierarchy_mute_expiry_and_mention_policy`;
  - `MUT-3-swap-cascade-category-over-channel` (md5 `fb1f7c3c63276f90be39e3832f7d0299`) — инверсия приоритета каскада (`parent_category` важнее `channel`) в `resolve_effective_notifications` → `KILLED` (`exit=101`) тестом `cascade_resolver_resolves_hierarchy_mute_expiry_and_mention_policy`;
  - `MUT-4-suppress-user-mention-badge-when-muted` (md5 `18564922aaf1f83ded596ad0a5e8f89c`) — подавление бейджа прямого `@user` упоминания при `is_muted` в `should_increment_badge` → `KILLED` (`exit=101`) тестом `cascade_resolver_resolves_hierarchy_mute_expiry_and_mention_policy`;
  - `MUT-5-skip-pre-network-channel-id-validation` (md5 `7b6fd5c967fd618376e8de9547299d98`) — отключение предсетевой проверки `validate_channel_id` → `KILLED` (`exit=101`) тестом `transport_functions_reject_invalid_inputs_before_network_and_send_exact_patch_payloads`.
- **baseline/restore:** baseline `rust/gui-spike/src/notifications.rs` = `2768dddc07f378bbed7884f5f21579bb`; после прогона всех 5 мутантов выполнен restore и проверено побайтовое совпадение md5 (`match=True`).

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/notifications.rs` | `2768dddc07f378bbed7884f5f21579bb` |
| `rust/gui-spike/src/lib.rs` | `155ad039f672518b8298fb1b52789560` |
| `rust/gui-spike/tests/notifications_contract.rs` | `9f047fd938b856037d1c1f5cbc3324a3` |
| `waves/RW-40/PM/n05-be/red.log` | `32d6ab770698a0b7b74cd16749e8ed9b` |
| `waves/RW-40/PM/n05-be/green.log` | `f135a2669e7479efc2a9e6ea4fead170` |
| `waves/RW-40/PM/n05-be/mutations.log` | `f329cb0585f023bf72a6449e107b54ae` |

## Риски и примечания

1. **Файловая граница:** созданы/изменены только назначенные файлы (`rust/gui-spike/src/notifications.rs`, `rust/gui-spike/src/lib.rs`, `rust/gui-spike/tests/notifications_contract.rs`, `waves/RW-40/PM/n05-be/*`); `rust/gui-spike/src/main.rs` не затрагивался.
2. **Поддержка одиночного и bulk-маршрутов:** `update_guild_notification_settings` и `update_channel_notification_override` отправляют `PATCH /users/@me/guilds/{guild_id}/settings` (включая `@me`), а метод `GuildNotificationSettingsPatch::to_bulk_patch_value(guild_id)` формирует обёртку `{"guilds": {"<guild_id>": ...}}` для `PATCH /users/@me/guilds/settings`.
