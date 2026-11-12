# Отчёт воркера: `TASK-626` (`G1 Backend` — Nitro Gifts & Entitlements REST + Tauri IPC)

## 1. Batch / Карточка
- **Карточка:** `TASK-626` (`STUB-12` — Nitro Gifts & Entitlements REST handlers + Tauri IPC commands)
- **Ветка:** `feature/task-626-gifts-be` (база `unstable` `a7be4c7`)
- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`

## 2. Сделано (сверка с RE-оракулами `reference-oracle/` и `waves/RW-40/`)
1. **RE-оракулы:**
   - `waves/RW-40/reference-oracle-REA.md` §5 (`STUB-12 — кнопка Gift 🎁`, маршруты `@290386..@290799`, `createGiftCode` `@613587`, `gift_style` `0..=2`).
   - `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §4.1–§4.5 (`#gift-modal`, лимит `200` символов, расчёт `gift_style`: `2` при непустом сообщении или эмодзи/звуке, иначе `1`).
   - `reference-oracle/decoded/4d2a64cb5df5fc4e_0`:
     - модуль `869038` (`@613060`): `resolveGiftCode`, `fetchUserGiftCodesForSKU`, `createGiftCode`, `revokeGiftCode`;
     - модуль `75255` (`@735306`): `redeemGiftCode` (`POST /entitlements/gift-codes/{code}/redeem`, тело `{channel_id, payment_source_id, gateway_checkout_context}`);
     - модуль `339048` (`@596307`): `fetchEntitlementsForApplication`, `fetchEntitlementsForUser`, `fetchGiftableEntitlements`;
     - модуль `45938` (`@9711598`): `GiftStyle` (`0..=2`), `resolveGiftCode`, извлечение подарочных ссылок (`discord.gift`, `discord.com/gifts`, `discordapp.com/gifts`, `promos.discord.gg`, `discord.com/billing/promotions`, не более 3 кодов на сообщение);
     - модуль `7133` (`@8906800`): `GiftCodeRecord` (`createFromServer`, битовые флаги `PAYMENT_SOURCE_REQUIRED = 1`, `EXISTING_PREMIUM_SUBSCRIPTION_DISALLOWED = 2`, `NOT_SELF_REDEEMABLE = 4`);
     - модуль `557009` (`@8904412`): `EntitlementRecord` (`createFromServer`, `isGiftable`: `type === USER_GIFT (6) && gifterId == null`);
     - модуль `202541` (`@6789126..@6797000`): `NITRO_APPLICATION_ID = "521842831262875670"`, `SKU_*` (`TIER_0`, `TIER_1`, `TIER_2`, `GUILD`, `NONE`, `LEGACY`) и 10 планов подписок `PLAN_*`;
     - модуль `652215` (`@320207`, `@325314`): коды ошибок Discord (`10038`, `50035`, `50050`, `50051`, `50054`, `50092`, `50095`, `50097`).
   - Live D-02 трассы (`reference-oracle/d02-snapshot/capture-ready.snap.jsonl:4004`, `capture-run2.snap.jsonl:8350`):
     - `GET /users/@me/entitlements?with_sku=false&with_application=false&entitlement_type=11&exclude_ended=true`
     - `GET /users/@me/applications/521842831262875670/entitlements?exclude_consumed=true`
     - `GET /users/@me/entitlements/gifts?country_code=LV`
2. **Реализация (`rust/gui-spike/src/gifts.rs`):**
   - Константы `NITRO_APPLICATION_ID`, `SKU_*`, `PLAN_*`, `GIFT_STYLE_*`, `GIFT_FLAG_*`, `ENTITLEMENT_TYPE_*`, `DISCORD_ERR_*`, `GIFT_IPC_COMMANDS`.
   - Каталог `list_nitro_gift_plans() -> Vec<NitroGiftPlan>` (10 планов из модуля `202541`).
   - Чистые функции `resolve_gift_style`, `extract_gift_codes_from_text`, `normalize_gift_code`, предсетевая валидация Snowflake (`1..=u64::MAX`, запрет ведущего нуля), кода подарка (`2..=64` символов) и `country_code` (ISO-3166-1 alpha-2).
   - Модели `GiftCodeRecord`, `EntitlementRecord`, `GiftResolveOptions`, `CreateGiftCodeRequest`, `GiftRedeemRequest`, `RedeemedGiftOutcome`, `EntitlementQuery`, `GiftError`.
   - Транспорт `GiftTransport` и `DiscordGifts` (с редактированным `Debug` без утечки токена).
   - 8 асинхронных REST-хендлеров: `resolve_gift_code`, `create_gift_code`, `fetch_user_gift_codes`, `revoke_gift_code`, `redeem_gift_code`, `fetch_user_entitlements`, `fetch_application_entitlements`, `fetch_giftable_entitlements`.
3. **Проводка Tauri IPC (`rust/gui-spike/src/main.rs`):**
   - Подключён `pub mod gifts;`, объявлены 9 `#[tauri::command]` (`resolve_gift_code`, `create_gift_code`, `fetch_user_gift_codes`, `revoke_gift_code`, `redeem_gift_code`, `fetch_user_entitlements`, `fetch_application_entitlements`, `fetch_giftable_entitlements`, `list_nitro_gift_plans`) и зарегистрированы в `tauri::generate_handler![]`.
   - Все 8 сетевых команд читают токен исключительно через `token_or::<GiftError, _>(..., || GiftError::Storage, || GiftError::MissingToken)` и добавлены в `TOKEN_COMMANDS` (`wiring_tests::every_network_command_reads_token_through_one_helper`).

## 3. Red-прогон (Strict TDD)
- Команда:
  ```sh
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test gifts_contract
  ```
- Результат: `EXIT:101` (`error: couldn't find file tests/../src/gifts.rs`), лог сохранён в `waves/RW-40/G1/task-626-gifts-be/red.log`.

## 4. Green-прогон и Targeted / Meta проверки
- Команда:
  ```sh
  CARGO_TARGET_DIR=/home/user/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/gui-spike/Cargo.toml --test gifts_contract --test ipc_wiring_contract --bin discord-gui-spike
  node --test tests/meta/ipc-path.test.mjs
  ```
- Результат:
  - `gifts_contract`: `13 passed; 0 failed`
  - `ipc_wiring_contract`: `9 passed; 0 failed`
  - `discord-gui-spike` (bin `wiring_tests`): `13 passed; 0 failed`
  - `tests/meta/ipc-path.test.mjs`: `12 pass; 0 fail`
  - Лог сохранён в `waves/RW-40/G1/task-626-gifts-be/green.log`.

## 5. Проверка мутациями (5/5 = 100% Killed)
- Лог: `waves/RW-40/G1/task-626-gifts-be/mutations.log`
  - `M1`: сдвиг границы длины сообщения `> 200` -> `> 205` в `resolve_gift_style` — **KILLED** (`exit=101`)
  - `M2`: ослабление лимита `MAX_EXTRACTED_GIFT_CODES` (`3` -> `10`) в `extract_gift_codes_from_text` — **KILLED** (`exit=101`)
  - `M3`: подмена маршрута `POST /users/@me/entitlements/gift-codes` -> `/users/@me/entitlements/gifts` — **KILLED** (`exit=101`)
  - `M4`: инверсия `self.gifter_user_id.is_none()` -> `is_some()` в `EntitlementRecord::is_giftable` — **KILLED** (`exit=101`)
  - `M5`: подмена классификации кода Discord `50050` (`GiftRedemptionExhausted` -> `NotFound`) — **KILLED** (`exit=101`)
  - Исходный файл `rust/gui-spike/src/gifts.rs` восстановлен байт-в-байт и отформатирован `rustfmt`.

## 6. Затронутые файлы и `md5sum`
- `rust/gui-spike/src/gifts.rs` — `ec3cf159ec2b9e90c65068fa4cfc67c8`
- `rust/gui-spike/src/main.rs` — `d5162e53c9639fe824ccab54f6657230`
- `rust/gui-spike/tests/gifts_contract.rs` — `cd501b62b32ab84678e356080a1faa70`
- `waves/RW-40/G1/task-626-gifts-be/red.log`
- `waves/RW-40/G1/task-626-gifts-be/green.log`
- `waves/RW-40/G1/task-626-gifts-be/mutations.log`
- `waves/RW-40/G1/task-626-gifts-be/REPORT.md`

## 7. Риски и примечания
- Общий файл `rust/gui-spike/src/lib.rs` не модифицировался в соответствии с `AGENTS.md` §1.2; модуль `gifts` подключён в `src/main.rs` и `tests/gifts_contract.rs`.
- Никаких секретов или токенов в логах/`Debug`/`Display` не раскрывается; все сетевые IPC-команды используют `token_or`.
