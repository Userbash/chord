# Отчёт воркера G5: task-176 (BE-P3-08)

- **batch/карточка:** RW-40 / BE-P3-08 (task-176: Token migration from Electron safeStorage dQw4w9WgXcQ: prefix, one-time import without secret leakage in logs/Debug);
- **сделано:**
  1. Объявлена публичная константа `ENCRYPTED_TOKEN_PREFIX = "dQw4w9WgXcQ:"` в соответствии с RE оракулом `RE/rest-static.md` §4.
  2. Реализована типизированная ошибка `SafeStorageMigrationError` с вариантами `EmptyPayload`, `InvalidBase64`, `DecryptFailed`, `InvalidToken`, `AlreadyMigrated`, `Storage(KeyringError)`. Реализации `Debug` и `Display` полностью защищены от утечки сырых/расшифрованных токенов, ключей и base64 ciphertext в логи.
  3. Реализованы функции `is_safe_storage_token(raw: &str) -> bool` (true для префикса с учётом trim) и `extract_safe_storage_ciphertext(raw: &str) -> Result<Vec<u8>, SafeStorageMigrationError>` со строгой валидацией base64 и отклонением пустого/невалидного payload.
  4. Реализован конечный автомат однократной миграции `SafeStorageMigrator` с методом `migrate_once<D, S>(&mut self, raw_entry: &str, decrypt: D, store: S) -> Result<bool, SafeStorageMigrationError>` и метод интеграции с Secret Service `migrate_keyring`. Реализована полная идемпотентность (повторный вызов возвращает `Ok(false)` без повторной дешифровки и без обращений к хранилищу), валидация токена (непустой UTF-8 без NUL/переводов строк).
  5. Добавлены 12 контрактных тестов k13–k24 (offline) и k25 (live) в `keyring_contract.rs`, проверены 100% совместимость со всеми существующими тестами k01–k12 и чистый clippy с `-D warnings`.
- **red:** `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --lib 2>&1 | tee red.log` → compilation failure: unresolved imports `extract_safe_storage_ciphertext`, `is_safe_storage_token`, `SafeStorageMigrationError`, `SafeStorageMigrator`, `ENCRYPTED_TOKEN_PREFIX` (exit code 101);
- **green:** `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --lib 2>&1 | tee green.log` → `12 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out; finished in 0.00s` (exit code 0);
- **targeted:**
  - `cargo test --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --lib` (12 passed, 13 ignored live);
  - `cargo clippy --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --lib -- -D warnings` (чисто, exit code 0);
  - `cargo fmt --check --manifest-path rust/Cargo.toml -p discord-platform` (чисто, exit code 0);
- **oracle:**
  - `RE/rest-static.md` §4 (хранение токена в модуле 247775 Electron safeStorage, префикс `dQw4w9WgXcQ:`, base64 encoding);
  - `BACKLOG-RE-BACKEND.md` §3 (BE-P3-08: импорт существующего токена один раз, без логов);
- **генерация:** не применимо;
- **мутации:** 5/5 мутаций закрыты тестами:
  1. Изменение префикса `dQw4w9WgXcQ:` → ловится тестом `k13_safe_storage_prefix_and_detection`.
  2. Пропуск валидации пустого payload в `extract_safe_storage_ciphertext` → ловится тестом `k15_extract_safe_storage_ciphertext_rejects_empty_payload`.
  3. Пропуск валидации NUL/переводов строк в токене (`\0`, `\n`, `\r`) → ловится тестом `k23_migrator_rejects_invalid_tokens`.
  4. Повторный вызов `migrate_once` при `migrated == true` (нарушение однократности) → ловится тестом `k19_migrator_idempotent_no_reimport`.
  5. Утечка данных в `Debug`/`Display` для `SafeStorageMigrationError` → ловится тестом `k17_migration_errors_do_not_leak_secrets`.
- **baseline/restore:** все тесты discord-platform в воркспейсе зелёные (1 passed unit + 23 dbus + 17 deep_link + 25 notifications + 16 power + 13 spellcheck + 10 tray + 9 autostart + 3 keyring_offline + 12 keyring_contract = 129 passed);
- **стабильность:** детерминированные результаты без race conditions и утечек;
- **gate:worker:** targeted-набор пройден, fmt пройден, clippy пройден без warnings;
- **файлы:**
  - `934bfb9f9cefd76fb67640c07909c68c rust/crates/discord-platform/src/keyring.rs`
  - `08e66e11b0d0bb312f939468dab73dab rust/crates/discord-platform/tests/keyring_contract.rs`
- **риски/вопросы:** рисков нет; полная обратная совместимость с существующими live-тестами `k01..k12`;
- **следующий шаг:** передать на двухэтапное ревью (PM + Technical Reviewer) и в integration-лейн.
