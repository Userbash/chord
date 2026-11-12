# Отчёт воркера — `feature/task-112-dm-friends-be` (`N1-07` DM & Friends Backend)

## 1. Batch / Карточка
- **Волна**: `RW-40`
- **Карточка / Ветка**: `feature/task-112-dm-friends-be` (`N1-07`: Личные сообщения и Друзья — Rust Backend: `READY.private_channels` / `READY.relationships` парсер + REST-клиент `/users/@me/channels` и `/users/@me/relationships`)
- **Роль**: Разработчик Backend G1
- **Независимые оракулы**:
  - `waves/RW-40/DM-FRIENDS-RE.md` (§0–§12)
  - `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (§1, §4, §6)
  - `fixtures/gateway/by-type/002-READY.json` (read-only: `146` объектов `d.users`, `17` объектов `d.private_channels`, `60` объектов `d.relationships`)
  - `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json` (read-only: `d.merged_presences.friends`)
  - `fixtures/rest/rest-static.json` (read-only: `USER_CHANNELS`, `USER_GROUP_DM_SHELL`, `USER_RELATIONSHIPS`, `USER_RELATIONSHIP`)

## 2. Что сделано
1. **Контрактные тесты (`rust/gui-spike/tests/dm_friends_contract.rs`) — RED-first**:
   - `ready_private_channels_and_rest_static_oracle_parity`: сверяет маршруты с `fixtures/rest/rest-static.json` (`POST /users/@me/channels`, `0` вызовов `GET /users/@me/channels`), разбирает живую фикстуру `002-READY.json` (`146` пользователей в `d.users`, `17` каналов `type=1` в `d.private_channels`), проверяет разрешение `recipient_ids` через `READY.users`, строгую числовую сортировку по убыванию `u64` `(last_message_snowflake, channel_snowflake)` (самый свежий `1213176899493306439` с `last_message_id = 1561431148641521707` / `.kirumi`, 16-й `914570058590736436` / `zay_alabay`, 17-й `1346114403149549638` с `last_message_id = nil -> None` / `unix2096`), а также разбор `GroupDm` (`type = 3`, `flags = PINNED`, `nicks`) и преимущество 19-значного snowflake `1000134165657825371` над 18-значным `999999999999999999`.
   - `ready_relationships_parsing_and_tabs_filtering_contract`: разбирает `60` записей `d.relationships` из `002-READY.json` (`18` друзей `type = 1`, `42` заблокированных `type = 2`, включая запись `1373418368463278182` с `type = 2` и `user_ignored = "true"`, не попадающую в `is_ignored()` согласно `RelationshipStore` @9319095), связывает с `003-READY_SUPPLEMENTAL.json` (`2` друга в сети: `275258435199827968` `idle` и `634667869254189065` `online`), проверяет фильтрацию по вкладкам `Online / All / Pending / Blocked`, выделение спама/игнора (`filter_pending_spam_and_ignored`) и формулу `pending_incoming_count = max(incoming - spam - ignored, 0)` (@9318383).
   - `rest_create_dm_and_group_dm_validates_snowflakes_and_recipients_bounds`: проверяет `POST /users/@me/channels` с телом `{"recipients": [...]}` (строго без `recipient_id`) для `1..=9` получателей, попадание в локальный кэш `open_private_channel_with_cache` (`0` сетевых вызовов при открытии существующей 1-на-1 DM, @567585), `POST /users/@me/channels/group-dm-shell` с `{"recipient_id": "..."}` (@568215), предсетевую валидацию snowflake ID, пустого токена, дубликатов и границ `1..=9` (`0` сетевых вызовов при ошибке валидации), а также разбор кода ошибки `30004` (`TOO_MANY_RECIPIENTS`).
   - `rest_relationships_put_delete_post_pomelo_and_stranger_80013_contract`: проверяет `PUT /users/@me/relationships/{user_id}` (принятие заявки `{"type": 1, "confirm_stranger_request": false/true}`, блокировка `{"type": 2}`, `from_friend_suggestion`), типизированный перехват кода ошибки `80013` (`DmFriendsError::StrangerConfirmationRequired`), `DELETE /users/@me/relationships/{user_id}`, `POST /users/@me/relationships` для Pomelo (`{"username": "alexvance", "discriminator": null}` при отсутствии `#` или `#0`, @697487) и классического тега (`"alexvance#0421"` -> `"discriminator": 421`), а также предсетевой отказ на невалидных тегах и snowflake ID (`0` сетевых вызовов).
   - `error_display_and_debug_never_leak_token_or_sensitive_headers`: проверяет маскирование токена (`<redacted>`) в `Debug` клиента `DiscordDmFriends` и отсутствие утечки токена в `Display`/`Debug` всех вариантов `DmFriendsError` (включая `401`, `429` с `Retry-After` и `500`, когда сервер отражает токен в теле ошибки).
   - `rest_error_responses_preserve_discord_code_on_403_and_404` (добавлен по Review 40A P2-2): `403 + 40002` (`EMAIL_VERIFICATION_REQUIRED`) и `404 + 10013` (`UNKNOWN_USER`) обязаны сохранять код Discord в `DmFriendsError::DiscordApiCode`; ответы без кода или с `code: 0` остаются `Forbidden`/`NotFound`, `401` — `Unauthorized`.
2. **Реализация модуля (`rust/gui-spike/src/dm_friends.rs` + экспорт в `rust/gui-spike/src/lib.rs`)**:
   - Реализованы типы и функции разбора `READY`/`READY_SUPPLEMENTAL`, фильтрации вкладок, валидации snowflake ID и Pomelo/Legacy тегов, трейт `DmFriendsTransport`, боевой адаптер `ApiClientDmFriendsTransport` поверх `discord-core::rest::client::ApiClient` и фасад `DiscordDmFriends`.
   - `map_error_response` извлекает числовой код Discord из тела ответа для любого `4xx`, кроме `401`, до статусного fallback (Review 40A P2-2); `code == 0` кодом не считается.
   - В `rust/gui-spike/src/lib.rs` добавлены исключительно `pub mod dm_friends;` и `pub use dm_friends::*;`.

## 3. RED-прогон (`waves/RW-40/PM/task112-dm-friends-be/red.log`)
Команда:
```sh
(cd rust/gui-spike && cargo test --offline -p discord-gui-spike --test dm_friends_contract -- --nocapture)
```
Результат:
- `test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s`
- Код выхода: `101` (`EXIT=101` зафиксирован в `waves/RW-40/PM/task112-dm-friends-be/red.log`).

`red.log` снят против заглушки и **не переписан** (md5 `9e2c6644d794bcccb14a6718756d4495`).
По Review 40A P2-1 отдельно снят `red-final.log` для финальной ревизии тест-файла — см. §9.

## 4. GREEN-прогон (`waves/RW-40/PM/task112-dm-friends-be/green-final.log`, после фикса P2-2)
Команды:
```sh
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test dm_friends_contract --test ipc_wiring_contract
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test dm_friends_contract -- -D warnings
rustfmt --edition 2021 --check rust/gui-spike/src/dm_friends.rs rust/gui-spike/tests/dm_friends_contract.rs
```
Результат:
- `dm_friends_contract`: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s`
- `ipc_wiring_contract`: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s`
- `TEST_EXIT=0`, `CLIPPY_EXIT=0`, `FMT_EXIT=0`.
- `cargo clippy --all-targets` в этом worktree падает вне карточки: `clippy::type_complexity` в
  `tests/net_contract.rs:782` и `src/main.rs:1667` (файлы других карточек, `dm_friends` там не
  используется). Поэтому clippy, как и в ревью, ограничен `--test dm_friends_contract`.
- Первый зелёный прогон до фикса (5 тестов) сохранён в `green.log` (md5 `06927be94491d833af50f179934214be`).

## 5. Мутационное тестирование (`waves/RW-40/PM/task112-dm-friends-be/mutations.log`)
Baseline, на котором снимались `MUT-1`…`MUT-5`: `d1820fe304816de3e4def848d992d3b9`.
Текущий baseline после фикса Review 40A P2-2: `6e74f81b47b9579bb0f863889c453479`.

| ID | Мутация в `rust/gui-spike/src/dm_friends.rs` | Мутированный MD5 | Убивший тест | Статус |
|---|---|---|---|---|
| `MUT-1` | `sort_private_channels_by_last_message_desc`: лексикографическая сортировка строк вместо числового `u64` snowflake | `638606a5978bc8d5c4eb6fe6f52e6c94` | `ready_private_channels_and_rest_static_oracle_parity` | **KILLED** (`exit=101`) |
| `MUT-2` | `RelationshipCounts::from_relationships`: пропуск вычитания `spam_count` и `pending_ignored_count` из `pending_incoming_count` | `dee493d88c0d71c732f6770ab96e6aa3` | `ready_relationships_parsing_and_tabs_filtering_contract` | **KILLED** (`exit=101`) |
| `MUT-3` | `validate_recipients_list`: допуск `10` получателей вместо верхней границы `MAX_DM_RECIPIENTS = 9` | `b830f4d8d136f884b2118cb97b6f237a` | `rest_create_dm_and_group_dm_validates_snowflakes_and_recipients_bounds` | **KILLED** (`exit=101`) |
| `MUT-4` | `map_error_response`: отключение типизированного распознавания кода `80013` (`StrangerConfirmationRequired`) | `a8f531000e426560aa80b196dbb24681` | `rest_relationships_put_delete_post_pomelo_and_stranger_80013_contract` | **KILLED** (`exit=101`) |
| `MUT-5` | `send_friend_request`: сериализация `"0"` вместо `null` для `discriminator: None` (Pomelo) | `14fd47a86cefd8a19a9212079d17b015` | `rest_relationships_put_delete_post_pomelo_and_stranger_80013_contract` | **KILLED** (`exit=101`) |
| `MUT-6` | `map_error_response`: откат фикса P2-2 — код Discord не извлекается для `403`/`404` (редакция `dm_friends.rs` md5 `d1820fe304816de3e4def848d992d3b9`) | `d1820fe304816de3e4def848d992d3b9` | `rest_error_responses_preserve_discord_code_on_403_and_404` | **KILLED** (`exit=101`, лог `red-final.log`) |

Итог мутаций: **6/6 KILLED**, baseline восстановлен и проверен (`md5 = 6e74f81b47b9579bb0f863889c453479`, повторный `cargo test` — `6 passed; 0 failed; 8 passed; 0 failed`).

## 6. Файлы и контрольные суммы (`md5sum`)

| Файл | MD5 | Назначение |
|---|---|---|
| `rust/gui-spike/src/dm_friends.rs` | `6e74f81b47b9579bb0f863889c453479` | Модуль разбора `READY.private_channels`/`relationships` и REST-клиент ЛС/друзей |
| `rust/gui-spike/src/lib.rs` | `dd547303373928de27f6c18ce52c41a9` | Экспорт `pub mod dm_friends;` и `pub use dm_friends::*;` |
| `rust/gui-spike/tests/dm_friends_contract.rs` | `920b11eef5ef5de7bebf715a9846f5ad` | Контрактные тесты (6) против `002-READY.json`, `003-READY_SUPPLEMENTAL.json`, `rest-static.json` и mock-транспорта |
| `waves/.../red.log` | `9e2c6644d794bcccb14a6718756d4495` | Исходный RED-прогон против заглушки (`0 passed; 5 failed`, `EXIT=101`), не изменён |
| `waves/.../red-final.log` | `ce944aa49c6016a07d3a97ad3a86cda4` | RED-final: финальный тест-файл против отката P2-2 (`5 passed; 1 failed`, `EXIT=101`) |
| `waves/.../green.log` | `06927be94491d833af50f179934214be` | Первый GREEN-прогон (5 тестов, до фикса P2-2) |
| `waves/.../green-final.log` | `1ab68c45acdfc7113687f471cbd88354` | GREEN-final: `6 + 8` тестов, clippy и rustfmt после фикса P2-2 |
| `waves/.../mutations.log` | `9d8e7168eb796c2e16f2fb89de057819` | Лог 5 мутаций (`5/5 KILLED`), фикс P2-2 покрыт MUT-6 в `red-final.log` |

## 7. Описание для открытия PR в `unstable`
- **Заголовок PR**: `feat(gui-spike): implement DM & Friends READY parser and REST client (task-112 / N1-07)`
- **Краткое резюме**:
  - Добавлен модуль `discord_gui_spike::dm_friends` (`rust/gui-spike/src/dm_friends.rs`), реализующий бессетевой разбор `READY.private_channels` (`17` каналов в `002-READY.json`, разрешение получателей через `146` записей `READY.users`, числовая `u64`-сортировка по `last_message_id` без вызовов `GET /users/@me/channels`) и `READY.relationships` (`60` связей в `002-READY.json`, фильтрация по вкладкам `Online / All / Pending / Blocked` с учётом `003-READY_SUPPLEMENTAL.json`, спама и `user_ignored`).
  - Реализованы REST-операции `POST /users/@me/channels` (`{"recipients": [...]}` для `1..=9` получателей + локальный кэш для 1-на-1 DM), `POST /users/@me/channels/group-dm-shell` (`{"recipient_id": "..."}`), `PUT /users/@me/relationships/{user_id}` (с типизированным разбором кода `80013` `StrangerConfirmationRequired`), `DELETE /users/@me/relationships/{user_id}` и `POST /users/@me/relationships` (Pomelo `"discriminator": null` и legacy `"discriminator": u16`).
  - Покрыто 6 контрактными тестами в `rust/gui-spike/tests/dm_friends_contract.rs` и проверено 6 мутациями (`6/6 KILLED`).

## 8. Риски и открытые вопросы
- Рисков нет: ни один файл за пределами разрешённого списка не затронут; `fixtures/` читаются строго в режиме read-only; сетевые вызовы в тестах изолированы fake-транспортом.

## 9. Исправления по Review 40A (`waves/RW-40/PM/task112-dm-friends-be/REVIEW.md`)

### P2-1 — новый `red-final.log` для финальной ревизии тестов

- Старый `red.log` не переписан (md5 `9e2c6644d794bcccb14a6718756d4495`, 20:28:34).
- `red-final.log` снят против **финальной отформатированной** ревизии `dm_friends_contract.rs`
  (md5 `920b11eef5ef5de7bebf715a9846f5ad`) и отката реализации P2-2
  (`dm_friends.rs` md5 `d1820fe304816de3e4def848d992d3b9`, редакция до фикса).
- Результат: `5 passed; 1 failed`, `EXIT=101`; паника на `tests/dm_friends_contract.rs:1127`
  (`assert_eq!` в 6.1) — номер строки соответствует финальному файлу, смещения из старого лога в
  новых утверждениях не участвуют.
- Финальная редакция тестов отличается от исходной только блоком §6 (новый тест); остальные
  пять тестов и их утверждения не менялись, поэтому их red-first закрывает исходный `red.log`.
- Этот же прогон служит мутацией `MUT-6` (см. §5).

### P2-2 — `map_error_response` больше не теряет код Discord на `403`/`404`

`rust/gui-spike/src/dm_friends.rs`, `map_error_response`:

- числовой код извлекается из тела **до** статусного fallback для любого `4xx`, кроме `401`;
- `403 {"code":40002}` → `DmFriendsError::DiscordApiCode { status: 403, code: 40002 }`
  (`EMAIL_VERIFICATION_REQUIRED`, SPEC §1.4), `404 {"code":10013}` → `DiscordApiCode { status: 404, code: 10013 }`
  (`UNKNOWN_USER`);
- `code == 0` кодом не считается: `403`/`404` без кода остаются `Forbidden`/`NotFound`;
- `401` остаётся `Unauthorized` (Discord отдаёт в теле только `code: 0`, UI различает по статусу);
- `429` и `80013` обрабатываются как раньше; `Display`/`Debug` токен не печатают.

Тест `rest_error_responses_preserve_discord_code_on_403_and_404` покрывает `403 + 40002`,
`404 + 10013`, `403`/`404` без кода, `404` с `code: 0` и `401` с `code: 0`.
RED: `red-final.log` (`Forbidden` вместо `DiscordApiCode { 403, 40002 }`), GREEN: `green-final.log`.
