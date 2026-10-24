# NET-C2 — backend авторизации (`auth_status`/`auth_login_token`/`auth_logout`/`auth_restore`)

**Волна:** RW-39, отдел NET, роль NET-C2 (Backend). PM: DeepSeek. Дата: 2026-10-23.
**Статус:** выполнено, red-first; сеть/окно/probe/keyring-live не использовались.
**Основание:** `BRIEF-NETC2.md`, `ADR-2-auth.md` §A/§4, `AUTH-RE.md` H-01/A-15/E-07.

## 1. Что сделано

| Файл | Что |
|---|---|
| `rust/gui-spike/src/auth.rs` (новый) | `AuthStatus`/`AuthUser`/`AuthError`/`AuthState`, `IdentityContext`, `TokenValidator` + `DiscordValidator`, `TokenStore` + `ThreadedStore` (поток-владелец keyring), `AuthBackend` |
| `rust/gui-spike/src/lib.rs` | `pub mod auth;` + шапка |
| `rust/gui-spike/src/main.rs` | 4 Tauri-команды, `spawn_blocking`, `app.manage(Arc<AuthBackend>)`, регистрация в `invoke_handler!` |
| `rust/gui-spike/Cargo.toml` | path-dep `discord-platform` (keyring), `tokio` (`rt`,`time`,`net`) — уже в графе (tauri/core), версия из офлайн-кэша |
| `rust/gui-spike/tests/auth_contract.rs` (новый) | 21 контрактный тест, без сети/окна/Secret Service |
| `waves/RW-39/NET/PM/netc2/*.log` | baseline, red, green, fmt, clippy, full |

### API команд

| Команда | Вход | Выход |
|---|---|---|
| `auth_status` | — | `Result<AuthState, AuthError>` — **только keyring, без сети** (keyring читается один раз за жизнь backend) |
| `auth_login_token(token: String)` | сырой user-токен (trim) | `Result<AuthState, AuthError>`; `GET /users/@me` → keyring.store → `Ready`, иначе `Error` |
| `auth_restore` | — | `Result<AuthState, AuthError>`; keyring.load → `/users/@me`; **единственный** сетевой путь при старте |
| `auth_logout` | — | `Result<AuthState, AuthError>`; keyring.clear + сброс сессии; идемпотентно |

`AuthState` (snake_case, как остальные payload спайка):
`{status: "idle"|"validating"|"ready"|"error", user?: {id, username, discriminator, global_name, avatar, bot}, error?: {kind, message, retry_after_ms?}, stored: bool}`.
`stored` — «в keyring есть токен», значения токена в состоянии нет.

### Ошибки (`AuthError`, `kind` — как у `DataError`)

| kind | Когда |
|---|---|
| `EmptyToken` | пустой/пробельный ввод — в сеть и keyring не уходит |
| `InvalidToken` | `ApiError::Unauthorized` (401) либо тело `/users/@me` без `id` с `401` |
| `RateLimited { retry_after_ms }` | 429 после повторов ядра (`retry_after`) |
| `Network { message }` | транспорт/таймаут/5xx/невалидная база |
| `Keyring { message }` | connect/lookup/store/clear, поток-владелец мёртв/не ответил |
| `Parse { message }` | тело `/users/@me` не JSON/нет полей/тело ошибки API |
| `Internal { message }` | конфигурация (локаль/таймзона, runtime) и сорванный `spawn_blocking` |

Токен не попадает ни в `Display`/`Debug` ошибок, ни в JSON состояния, ни в логи
(`ValidationRequest` имеет ручной `Debug` с `<redacted>`; тексты проходят через `sanitize`).

### Keyring `!Send`

`discord-platform::Keyring` — `!Send + !Sync`, поэтому:

- `TokenStore` — трейт **без** `Send`: `KeyringStore` создаётся и живёт **внутри** потока
  `auth-keyring`; наружу торчит `Box<dyn TokenStore + Send>` (`ThreadedStore`);
- `ThreadedStore` — канал `Request::{Load,Store,Clear}` + канал ответа на каждый запрос;
  `Keyring::connect()` — ленивый, при первом запросе (окно стартует без Secret Service);
- ошибка открытия хранилища отвечает на каждый запрос причиной (UI не висит);
- `Drop` закрывает канал и `join`-ит поток; Tauri-команды выполняют блокирующие вызовы в
  `tokio::task::spawn_blocking`, UI-поток не блокируется.

## 2. Проверки (gui-spike target, офлайн)

| # | Команда | Итог |
|---|---|---|
| 1 | `cargo test --offline` (baseline до правок) | 59 passed (3 main + 6 core_bridge + 29 data + 20 ipc + 1 launch-skip) → `00-baseline-test.log` |
| 2 | `cargo test --offline --test auth_contract` (red) | `error[E0432]: unresolved import discord_gui_spike::auth` → `01-red.log` |
| 3 | то же (green) | **21 passed** → `02-green-auth.log` |
| 4 | `cargo fmt -- --check` до форматирования | диффы только в моих 3 файлах → `03-fmt-check.log`; после `cargo fmt` — чисто → `03b-fmt-ok.log` |
| 5 | `cargo clippy --offline --all-targets -- -D warnings` | чисто (exit 0) → `04-clippy.log` |
| 6 | `cargo test --offline` (full, после fmt) | **80 passed**, 0 failed (3+21+6+29+20+1) → `05-full-test.log` |

Тесты не ходят в сеть, не запускают окно (`launch.rs` — skip без `GUI_SPIKE_E2E=1`), не
трогают Secret Service (фейковые `TokenStore`/`TokenValidator`; поток-владелец проверяется
тем же механизмом с фейковым хранилищем).

Покрыто: idle без токена и без сети; `stored=true` без probe; login 200 → `Ready` + запись в
keyring; trim вставленного токена; пустой токен; 401 → `InvalidToken` (keyring не тронут);
429 → `RateLimited{retry_after_ms}`; сеть → `Network`; битый JSON → `Parse`; тело 401 без `id` →
`InvalidToken`; отказ keyring при login → `Keyring`, не `Ready`; restore (успех/нет токена/
отозванный токен) — сеть только по команде; logout + идемпотентность; logout при отказе keyring
гасит сессию; поток-владелец (3 операции + отказ открытия); маппинг ошибок ядра и отсутствие
токена в тексте; JSON состояния без токена; identity-заголовки; регистрация 4 команд в
`invoke_handler!`.

## 3. md5

```
e0f9ae5a0a1c5284ae69b9c2e47e4273  rust/gui-spike/src/auth.rs
d23d0c68db01935989cf5c9341def585  rust/gui-spike/src/main.rs
4a877e7cdaf70cc2eceb88130672cca9  rust/gui-spike/src/lib.rs
d63184cfc0057c8ff87169756c98b954  rust/gui-spike/Cargo.toml
b87439c6020d44031f2a8268052bc935  rust/gui-spike/tests/auth_contract.rs
7d66c56ef6f7e6e06cb7885a7d9a4bb7  rust/gui-spike/Cargo.lock   (добавлены discord-platform, tokio)
```

Границы соблюдены: `ui/**`, `fixtures/**`, `data.rs`, чужие тесты не изменялись;
`logs/`, `functions/`, `evidence/` не трогались; коммитов нет.

## 4. Что осталось (вне NET-C2)

1. **UI/login screen**: поле ввода токена, вызов `auth_login_token`, состояния `idle/
   validating/ready/error`, `stored → «восстановить сессию»` (`auth_restore`).
2. **Событие `auth://state`** (ADR-2 §5) не эмитится: команды возвращают снимок; эмиссию
   добавить в UI-волне (`AppHandle::emit`). Поэтому `validating` сейчас — промежуточное
   внутреннее состояние (команды блокирующие), для спиннера нужен стрим.
3. **Gateway**: logout пока не останавливает супервизор (его нет в gui-spike); при появлении
   `stop supervisor → keyring.clear → wipe session` порядок уже зафиксирован в ADR-2 §6.
4. **Live-проверка** (ADR-2 Acceptance): вставка токена → READY, 401, рестарт из keyring —
   только в согласованном окне; здесь запрещено.
5. **Риск libsecret**: сборка офлайн прошла (системные `libsecret-1`/`glib-2.0`/`gio-2.0` в
   `/usr/lib`, `pkg-config` → 0.21.8.2), но рантайм требует `org.freedesktop.secrets` в
   сессионной шине; при заблокированной коллекции первый вызов может ждать prompt (поток-
   владелец, не UI). Вендорить ничего не пришлось.
6. **ToS**: импорт user-токена — риск блокировки аккаунта (ADR-2, `AUTH-RE.md` §8); только
   тестовый аккаунт, письменная фиксация у владельца.
