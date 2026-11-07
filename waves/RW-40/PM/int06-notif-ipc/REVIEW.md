# Независимое ревью — INT-06-NOTIFICATIONS-IPC-WIRING (F-5)

Дата: 2026-10-30. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Ветка ревью: `rw40/review-int06-notif-ipc`. Продуктовый код не менялся.

## Вердикт

**PASS по карточке. P0 = 0, P1 = 0, P2 = 3.**
Обе IPC-команды уведомлений действительно объявлены, читают токен через `token_or`, делегируют в
`discord_gui_spike::*` и зарегистрированы в `generate_handler!` ровно по одному разу. Артефакты
red/green/mutations воспроизводятся: MD5 `main.rs`, `ipc_wiring_contract.rs` и всех трёх логов
совпадают с REPORT.md, мутантные MD5 MUT-1/MUT-2 воспроизведены независимо. P2-замечания ниже
не блокируют открытие PR, но формулировку «38/38 подтверждено тестом» использовать нельзя.

## Проверка контракта

| Пункт | Результат | Evidence |
|---|---|---|
| `#[tauri::command] async fn update_guild_notification_settings` | есть | `main.rs:862-880` |
| `#[tauri::command] async fn update_channel_notification_override` | есть | `main.rs:884-910` |
| Чтение токена через `token_or::<NotificationError, _>` | есть, по одному `backend.token()` | `main.rs:871-876`, `894-899` |
| Делегирование в `discord_gui_spike::update_guild_notification_settings(&api,&token,&guild_id,&resolved).await` | есть | `main.rs:879` |
| Делегирование в `discord_gui_spike::update_channel_notification_override(&api,&token,&guild_id,&channel_id,&resolved).await` | есть | `main.rs:902-909` |
| Регистрация в `tauri::generate_handler![...]` | обе, ровно по одному разу | `main.rs:1201-1202` |
| Аргументы совместимы с `notifications.js` | да, с оговоркой P2-3 | `ui/features/notifications.js:1335,1389` |
| Рекурсивный обход `ui/**/*.js` | да, 32 файла, включая `notifications.js` | `collect_ui_js_files`, тест `every_ui_invoke_command...` |
| «Все 38 команд» проверяются тестом | **нет, только 26 из 38** | P2-1 |

Совместимость аргументов: UI шлёт `guildId`, `patch`, `settings` (гильдия) и `guildId`,
`channelId`, `override`, `patch` (канал). Tauri маппит `guild_id`/`channel_id` из camelCase,
конверт `{ channel_overrides: { [channelId]: overridePatch } }` разбирается через
`ChannelOverrideInput::Envelope`, прямые вызовы покрыты `override_patch`/`payload`. `endpoint` и
`body` из UI не объявлены в сигнатурах и игнорируются — это безопасно, потому что путь и тело
PATCH заново строит `notifications.rs` (`PATCH /users/@me/guilds/{guild_id}/settings`, `@me`
допускается `validate_guild_id`). Проверка `../bad` до сети подтверждена поведенческим тестом на
`FakeNotificationTransport`.

## Прогоны

RED: `waves/RW-40/PM/int06-notif-ipc/red.log`, `EXIT_CODE=101`, `6 passed; 2 failed`; тексты падений
и номера строк (`1040:13`, `1122:9`) совпадают с текущей версией теста — лог снят с этого файла.

GREEN (повторено на текущих байтах):

```text
cargo test -p discord-gui-spike --test ipc_wiring_contract   → 8 passed; 0 failed
cargo test -p discord-gui-spike --lib --bins --test notifications_contract
    src/lib.rs 8 passed; src/main.rs 12 passed; tests/notifications_contract.rs 4 passed
cargo clippy --offline -p discord-gui-spike --lib --bins --test ipc_wiring_contract -- -D warnings → 0
rustfmt --edition 2021 --check src/main.rs tests/ipc_wiring_contract.rs → exit 0
node --test tests/meta/ipc-path.test.mjs      → 12 pass / 0 fail
node --test tests/meta/test-policy.test.mjs   → 17 pass / 0 fail
```

Skip/`only`/`ignore` в `ipc_wiring_contract.rs` отсутствуют. Полный `gate:worker` относится к
integration-лейну и этим ревью не подтверждается.

## MD5

| Файл | MD5 | Сверка |
|---|---|---|
| `rust/gui-spike/src/main.rs` | `ae573539a9b8a8cc6e355c3975edaff0` | = REPORT.md, = mutations.log baseline |
| `rust/gui-spike/tests/ipc_wiring_contract.rs` | `159053d11b700daba7a0ecdc863fb7fc` | = REPORT.md |
| `waves/.../red.log` | `1876c2d4ec7cbb2d4c2a82433dea96ad` | = REPORT.md |
| `waves/.../green.log` | `375f8e323295df42ac1440f0d8477b85` | = REPORT.md |
| `waves/.../mutations.log` | `5e06bb03d823bc184cdc5a046f8091d2` | = REPORT.md |

Мутации: удаление регистрации каждой команды воспроизведено вручную на копиях `main.rs` —
MD5 мутантов `9f0ceede9031e92ed53ff1529581f5f2` (MUT-1) и `97d70510e94493b12a6abbe193ba9cee`
(MUT-2) совпали с логом. MUT-3…MUT-5 подтверждены по логу: каждый убит
`notifications_ipc_commands_are_registered_and_delegate_to_lib`, финальный restore = baseline,
`FINAL_GREEN_EXIT_CODE=0`.

## Замечания

**P2-1. Оракул «every_ui_invoke…» покрывает 26 имён из 38, а не все.**
Тест сканирует все 32 файла `ui/**/*.js` (это правда) и теперь видит `invokeReaction`, но
извлекает только литеральные формы `invoke("…")` / `invokeFn("…")` / `invokeReaction("…")`.
Вызовы через константы не разбираются, поэтому 12 команд из списка MERGE-READINESS-13 §3.4
остаются вне проверки: `auth_status`, `auth_login_token`, `auth_restore`, `auth_login_password`,
`auth_mfa`, `auth_sms`, `resolve_invite`, `accept_invite`, `join_guild`, `net_status`,
`net_connect`, `ready` (`AUTH_COMMANDS`, `JOIN_COMMANDS`, `NET_COMMANDS`, `READY_COMMAND`).
Независимый подсчёт: 63 call-site, 26 уникальных литеральных имён. Рекомендация §3.5
(«разбирать все идентификаторы, содержащие `invoke`») выполнена частично. Для F-5 это не
блокер: обе новые команды вызываются литералами в `notifications.js` и покрыты отдельно, но
фраза «тест подтверждает 38/38» неверна.

**P2-2. Тихий fallback на чужой канал в `ChannelOverrideInput::into_patch`.**
`channel_overrides.remove(channel_id).or_else(|| channel_overrides.into_values().next())`
(`main.rs:840-843`): если конверт не содержит запрошенный `channelId`, применится переопределение
первого попавшегося канала вместо ошибки. Текущий UI всегда кладёт ключ `channelId`, поэтому путь
недостижим из `notifications.js`, но прямой IPC-вызов с рассинхроном молча запишет чужие настройки.
Лучше вернуть `NotificationError::InvalidChannelId`/пустой патч, а не подставлять другой канал.

**P2-3. UI-параметр `override` не объявлен и молча игнорируется.**
`notifications.js:1389` передаёт `override: overridePatch`, а команда принимает
`override_patch`/`patch`/`payload`. Сейчас данные не теряются, потому что тот же вызов передаёт
`patch` с конвертом. Но контракт «совместим с форматом UI» держится на дублировании поля; если
UI когда-нибудь оставит только `override`, команда применит `unwrap_or_default()` — пустой патч
без ошибки. Стоит либо принимать `override`, либо валидировать непустой результат.

P0: 0. P1: 0. Функциональных дефектов в подключении F-5 не найдено.
