# TZ W33a — P1-10 Keyring через FFI к libsecret (headless)

Волна: RW-33. Дата: 2026-10-22. Крейт: `discord-platform`. Сеть/live Discord не используются.
Система: `libsecret 0.21.8`, `org.freedesktop.secrets` (ksecretd), CLI-оракул `secret-tool`.

## 1. Замороженный контракт

```rust
pub struct Keyring { /* *mut SecretService */ }
pub struct KeyringError { /* ... */ }

impl Keyring {
    /// Подключение к default collection через `secret_service_get_sync`.
    pub fn connect() -> Result<Self, KeyringError>;
    pub fn store(&mut self, key: &str, secret: &str) -> Result<(), KeyringError>;
    pub fn lookup(&mut self, key: &str) -> Result<Option<String>, KeyringError>;
    pub fn clear(&mut self, key: &str) -> Result<bool, KeyringError>;
}
```

Инварианты:

- **FFI**: только `src/ffi_secret.rs` (`#[allow(unsafe_code)]`, `// SAFETY:`); линковка
  `#[link(name = "secret-1")]`, `#[link(name = "glib-2.0")]`, `#[link(name = "gobject-2.0")]`,
  `#[link(name = "gio-2.0")]`. Схема — `secret_schema_newv` (не variadic), атрибут `key`
  (string), имя схемы `com.discordlab.Token`; коллекция — default (`NULL`), метка — `"discord-lab token"`.
- `store`: пустой `key` → `KeyringError::InvalidKey`; NUL в `key`/`secret` → `InvalidKey`/`InvalidSecret`;
  успех — `secret_service_store_sync` вернул TRUE; `GError` → `KeyringError::Secret { code, message }`.
- `lookup`: отсутствие записи → `Ok(None)`; найденное значение копируется в `String` и
  освобождается `g_free`; невалидный UTF-8 → `KeyringError::Encoding`.
- `clear`: TRUE → `Ok(true)`, отсутствие/ошибка → `Ok(false)`/`Err`; повторный clear — `Ok(false)`.
- `Drop`: `g_object_unref(service)` и `secret_schema_unref(schema)` ровно один раз; `!Send + !Sync`.
- Секреты не логируются, не попадают в `Debug` (ручной `Debug` без значения секрета).
- Никаких паник/`unwrap`/`expect` в продакшн-пути; ошибки glib не теряются.

## 2. Оракул и проверка

- Реальный ksecretd на сессионной шине: `secret-tool store/lookup/clear` (independent CLI) ↔
  наш `Keyring`. Live-кейсы skip-aware (`DISCORD_LAB_KEYRING_LIVE=1`), иначе `SKIP: причина`.
- Unit-кейсы (валидация, Debug-redaction, double clear) — без шины.
- Секреты синтетические, после каждого live-кейса `clear`; уникальный ключ включает `pid`.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| k01 | `connect` к ksecretd | Ok (live), иначе skip |
| k02 | store → lookup | значение совпадает |
| k03 | lookup отсутствующего | `Ok(None)` |
| k04 | clear существующего | `Ok(true)`, затем `Ok(None)` |
| k05 | перезапись | новое значение |
| k06 | Unicode-секрет | дословно |
| k07 | secret-tool lookup после нашего store | значение совпадает |
| k08 | secret-tool store → наш lookup | значение совпадает |
| k09 | пустой ключ | `InvalidKey` |
| k10 | NUL в ключе/секрете | ошибка, без паники |
| k11 | повторный clear | `Ok(false)` |
| k12 | `Debug` не печатает секрет | редекция |

Бюджет: 10–12 тестов; мутации 5–7 (schema attr, store/clear, NULL→None, g_free, Debug).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w33a cargo test --offline -p discord-platform --test keyring_contract
```

## 5. DoD

red → green → мутации (`--nocapture`, harness-SKIP=0 для не-live, restore по md5) → отчёт
`waves/RW-33/REPORT-W33a.md`.
