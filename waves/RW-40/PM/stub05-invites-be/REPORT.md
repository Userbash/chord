# RW-40 / STUB-05-INVITES-BACKEND — создание приглашения на канал (`POST /channels/{id}/invites`)

- **batch/карточка:** `RW-40 / STUB-05-INVITES-BACKEND` (`waves/RW-40/RE-DEEP-REA.md` §4.3, `waves/RW-40/DESIGN/INVITES-SPEC.md` §1.2);
- **роль:** Воркер G1 GUI-Spike Rust Backend (`CARGO_TARGET_DIR=/tmp/target-g1-invites`);
- **сделано:**
  1. В `rust/gui-spike/src/invites.rs` добавлены константы оракула:
     - `VALID_INVITE_MAX_AGES: [u32; 7] = [0, 1800, 3600, 21600, 43200, 86400, 604800]` (перечисленные 7 неэкспериментальных значений из RE и `INVITES-SPEC.md` §1.2/§1.3);
     - `VALID_INVITE_MAX_USES: [u32; 7] = [0, 1, 5, 10, 25, 50, 100]`;
     - `INVITE_FLAG_IS_GUEST: u32 = 1 << 0` (`IS_GUEST_INVITE`).
  2. Реализованы структуры `CreateInviteParams { max_age, max_uses, temporary, flags }` (с `Default`: `86400`, `0`, `false`, `None`) и `CreatedInvite { code, max_age, max_uses, temporary }` (camelCase в IPC и `alias` для wire snake_case).
  3. В `InviteError` добавлен вариант `InvalidRequest` (`"invalidRequest"`) для предсетевого отказа при невалидном `channel_id`, `max_age` или `max_uses`.
  4. Реализована свободная функция `create_channel_invite(&transport, channel_id, &params)` и метод `DiscordInvites::create_invite(&self, channel_id, &params)` (`POST /channels/{channel_id}/invites`), а также `impl InviteTransport for DiscordInvites`:
     - предсетевая проверка `channel_id` по allowlist (`[A-Za-z0-9_-]`, `1..=64` байта, в паритете с `messages::send_message`) и проверка `max_age`/`max_uses` по таблицам `VALID_INVITE_MAX_AGES`/`VALID_INVITE_MAX_USES` (иначе `InviteError::InvalidRequest` до обращения к транспорту);
     - сериализация тела запроса `{ max_age, max_uses, temporary, flags? }` (`flags` опускается при `None`);
     - разбор ответа `CreatedInvite { code, max_age, max_uses, temporary }` с проверкой обязательности полей и непустого `code`;
     - маппинг `429` с извлечением `retry-after` из заголовка `Retry-After` или JSON-тела `{"retry_after": ...}` в миллисекунды (`InviteError::RateLimited { retry_after_ms }`), а также `403` (`Forbidden`), `401` (`Unauthorized`), `404` (`NotFound`), `4xx`/`5xx`.
  5. В `rust/gui-spike/tests/invites_contract.rs` добавлены 5 новых контрактных тестов (и расширен `errors_serialize_with_camel_case_kind_only` на `InvalidRequest`); все 14 исходных тестов сохранены без ослаблений (итого 20 тестов).

- **red:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-invites cargo test --offline -p discord-gui-spike --test invites_contract) > waves/RW-40/PM/stub05-invites-be/red.log 2>&1` → `exit=101`, `error[E0432]: unresolved imports discord_gui_spike::invites::create_channel_invite, CreateInviteParams, CreatedInvite, INVITE_FLAG_IS_GUEST, VALID_INVITE_MAX_AGES, VALID_INVITE_MAX_USES` и `error[E0599]: no method named create_invite / no variant InvalidRequest`; лог: `waves/RW-40/PM/stub05-invites-be/red.log`.
- **green:** `(cd rust/gui-spike && CARGO_TARGET_DIR=/tmp/target-g1-invites cargo test --offline -p discord-gui-spike --test invites_contract) > waves/RW-40/PM/stub05-invites-be/green.log 2>&1` → `exit=0`, `test result: ok. 20 passed; 0 failed; 0 ignored`; лог: `waves/RW-40/PM/stub05-invites-be/green.log`.
- **targeted:**
  - `cargo test --offline -p discord-gui-spike --test invites_contract` → `20 passed; 0 failed` (`exit=0`);
  - `cargo clippy --offline -p discord-gui-spike --test invites_contract -- -D warnings` → 0 предупреждений (`exit=0`);
  - `rustfmt --edition 2021 --check rust/gui-spike/src/invites.rs rust/gui-spike/tests/invites_contract.rs` → чисто (`exit=0`).
- **oracle:** `waves/RW-40/RE-DEEP-REA.md` §4.3 (`4d2a64cb5df5fc4e_0@632264`, `@270160`, `@1933819`), модуль `558213`/`735547` дампа RE (`MAX_AGE_OPTIONS`, `MAX_USES_OPTIONS`), `waves/RW-40/DESIGN/INVITES-SPEC.md` §1.2–§1.4.
- **мутации:** 5/5 `KILLED`, эквивалентных — 0, survivors — 0 (`waves/RW-40/PM/stub05-invites-be/mutations.log`):
  - `MUT-1-allow-dot-in-channel-id` (md5 `bf6fa0fd44887aea6d4fb10939fa69ed`) — разрешение `.` в `validate_channel_id` → `KILLED` тестом `create_invite_rejects_unsafe_channel_id_before_network` (`exit=101`);
  - `MUT-2-drop-max-age-table-validation` (md5 `6faa9354d2ecf740d5fdc886ed9f27b1`) — отключение проверки `VALID_INVITE_MAX_AGES` → `KILLED` тестом `create_invite_rejects_unlisted_max_age_and_max_uses_with_invalid_request_before_network` (`exit=101`);
  - `MUT-3-drop-max-uses-table-validation` (md5 `346f396e9485ca9dae33ffb27ec9d661`) — отключение проверки `VALID_INVITE_MAX_USES` → `KILLED` тестом `create_invite_rejects_unlisted_max_age_and_max_uses_with_invalid_request_before_network` (`exit=101`);
  - `MUT-4-drop-flags-from-request-body` (md5 `9d5c3f9c4a4b3b925e3e1f94317ca7db`) — потеря `flags` при сериализации тела `POST` → `KILLED` тестом `create_invite_posts_to_channel_invites_with_params_and_parses_created_invite` (`exit=101`);
  - `MUT-5-ignore-429-retry-after` (md5 `bd1d81ab1f57cbddd2642d6c7427d5b3`) — игнорирование `retry-after` при `429` → `KILLED` тестом `create_invite_maps_429_retry_after_and_403_401_errors` (`exit=101`).
- **baseline/restore:** baseline `rust/gui-spike/src/invites.rs` = `951bb4bc5f79b8e96ac3d96f8f5bafca`, `rust/gui-spike/tests/invites_contract.rs` = `f6a8d252207558d7cae0aa58a67368e4`; после каждого мутанта выполнен restore и проверено совпадение md5 (`match=True`).
- **gate:worker:** в режиме batch воркер запускает только targeted-проверки своего модуля с изолированным `CARGO_TARGET_DIR=/tmp/target-g1-invites`.

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/invites.rs` | `951bb4bc5f79b8e96ac3d96f8f5bafca` |
| `rust/gui-spike/tests/invites_contract.rs` | `f6a8d252207558d7cae0aa58a67368e4` |
| `waves/RW-40/PM/stub05-invites-be/red.log` | `1bca9be8ebb98677f34a83deb7466e72` |
| `waves/RW-40/PM/stub05-invites-be/green.log` | `23b5e5bc011a26ea7ac7789b23aa4fb9` |
| `waves/RW-40/PM/stub05-invites-be/mutations.log` | `6aefeaff46ecd9a066bf1c0ad54350c5` |

## Риски и примечания

1. **Размерность `VALID_INVITE_MAX_AGES`:** в литерале `[0, 1800, 3600, 21600, 43200, 86400, 604800]` (совпадающем со списком 7 пунктов селекта в `INVITES-SPEC.md` §1.2 п.1–7 и §1.3) ровно 7 элементов, поэтому тип константы в Rust объявлен как `[u32; 7]` (в `[u32; 8]` с 7 элементами Rust не компилируется с `E0308`).
2. **Общие файлы (`main.rs`, `lib.rs`):** в соответствии с файловой границей ТЗ затронуты только `rust/gui-spike/src/invites.rs` и `rust/gui-spike/tests/invites_contract.rs`. Регистрация Tauri-команды `create_invite` в `src/main.rs` остаётся за интеграционным шагом Team Lead.
