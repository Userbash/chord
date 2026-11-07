# Отчёт воркера: task-245 (`BE-P1-27` — Rust Webhooks & Channel Follow BFF Contract)

- **Batch / Карточка:** `RW-40` / `G1` / `task-245` (`BE-P1-27`)
- **Ветка:** `feature/task-245-be-webhooks`

## 1. Что сделано
1. **Strict TDD (Red):** написан контрактный тест `rust/gui-spike/tests/webhooks_contract.rs` (8 тестовых сценариев: константы типов вебхуков и границ длины имени, serde round-trip `CreateWebhookRequest`, валидация `validate_webhook_name` с `trim()`, границами `1..=80` Unicode-символов и регистронезависимым запретом имени `"clyde"` и подстроки `"discord"`, построение маршрута и JSON-тела `build_create_webhook_route_and_body`, а также построение маршрута и JSON-тела подписки на новостной канал `build_follow_channel_route_and_body` с запретом совпадения `news_channel_id == target_channel_id` и проверкой канонических положительных Snowflake `1..=20` цифр без ведущих нулей). Зафиксировано падение компиляции (`E0432: unresolved import discord_gui_spike::webhooks`) в `red.log`.
2. **Strict TDD (Green):** реализован модуль `rust/gui-spike/src/webhooks.rs` и экспортирован `pub mod webhooks;` в `rust/gui-spike/src/lib.rs`:
   - Константы: `WEBHOOK_TYPE_INCOMING = 1`, `WEBHOOK_TYPE_CHANNEL_FOLLOWER = 2`, `WEBHOOK_TYPE_APPLICATION = 3`, `MIN_WEBHOOK_NAME_CHARS = 1`, `MAX_WEBHOOK_NAME_CHARS = 80`.
   - Структура `CreateWebhookRequest { pub name: String, pub avatar: Option<String> }`.
   - Функции `validate_webhook_name`, `build_create_webhook_route_and_body`, `build_follow_channel_route_and_body` в соответствии с RE-оракулом `/home/sanya/re-deep/rest-static.json` (`CHANNEL_WEBHOOKS`, `CHANNEL_FOLLOWERS`).

## 2. Red-прогон
Команда:
```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test webhooks_contract > /home/sanya/orca/project/waves/RW-40/G1/task-245-be-webhooks/red.log 2>&1
```
Результат: ожидаемое падение компиляции (`error[E0432]: unresolved import discord_gui_spike::webhooks`). Лог сохранён в `/home/sanya/orca/project/waves/RW-40/G1/task-245-be-webhooks/red.log`.

## 3. Green-прогон (Targeted T0)
Команда:
```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test webhooks_contract > /home/sanya/orca/project/waves/RW-40/G1/task-245-be-webhooks/green.log 2>&1
```
Результат: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`. Лог сохранён в `/home/sanya/orca/project/waves/RW-40/G1/task-245-be-webhooks/green.log`.

## 4. Изменённые и созданные файлы (`md5sum`)
| Файл | MD5 |
|---|---|
| `rust/gui-spike/src/webhooks.rs` | `f4aab665ef767512aa9693affebb863d` |
| `rust/gui-spike/src/lib.rs` | `0a9880a8ccdcb0927b404481e54175c0` |
| `rust/gui-spike/tests/webhooks_contract.rs` | `1ec829a980b532208aadd86d01b413b7` |
| `/home/sanya/orca/project/waves/RW-40/G1/task-245-be-webhooks/red.log` | `341ff5360e6476512ac99204fecc1f9f` |
| `/home/sanya/orca/project/waves/RW-40/G1/task-245-be-webhooks/green.log` | `c37e2c1a88a9067fc80c303388b9258a` |

## 5. Риски и замечания
- Затронуты исключительно разрешённые в ТЗ файлы.
- `git commit` не выполнялся в соответствии с правилами воркера.
