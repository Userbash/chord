# Отчёт NET-C1 (Core) — auth-примитивы, `/users/@me`, redaction, единая идентичность

- Волна: RW-39 / NET, батч NET-C (карточка C1).
- PM: DeepSeek. HEAD на старте: `0239543` (RW-35 принят).
- Режим: red-first, offline (`cargo --offline`), без сети/окна/GUI/commit.
- Логи: `waves/RW-39/NET/PM/netc1/` (`00-baseline-test.log`, `01-red.log`,
  `02-green-targeted.log`, `03-green-full.log`, `04-clippy.log`).

## 1. Пункт → red → green

| # | Задача (аудит) | Red | Green |
|---|---|---|---|
| 1 | **P0-1**: `ApiAuth::from_token(token, &SuperProperties, locale, timezone)`; 6 непустых заголовков; `Debug` без секрета | `E0432`/`E0599`: нет `ApiAuthError`, `AuthScheme`, `from_token`, `from_token_with_scheme` | `from_token`/`from_token_with_scheme`; `AuthScheme::{User(default),Bot,Bearer}`; `DEBUG_OPTIONS_LIVE="bugReporterEnabled"` (живой захват); пустые token/locale/timezone → `ApiAuthError` |
| 2 | **P0-4**: `rest/user.rs` — `Me` + `parse_me(&[u8])` через `discord-json`, типизированные ошибки | `E0432: unresolved import discord_core::rest::user` | `Me{id,username,discriminator,global_name,avatar,bot}`; `MeError::{NotUtf8,Json,NotObject,MissingField,InvalidField,Unauthorized,Discord}`; `discord-json` — production-зависимость |
| 3 | **P1-2**: `HttpRequest::Debug` — `Authorization`/`Cookie` → `<redacted>` | нет теста/поведения (derive печатал заголовки) | ручной `Debug`: 3 секретных имени без учёта регистра (`authorization`, `proxy-authorization`, `cookie`), тело — только `body_len`; несекретные заголовки видимы |
| 4 | **P1-1**: `IdentifyProperties::from_super_properties(&SuperProperties)` — единый launch_id/UA/версия | `E0599: no function from_super_properties` | 14 пересекающихся полей берутся из `SuperProperties` (build `u32→u64`); константы клиента: `is_fast_connect=false`, `gateway_connect_reasons="COLD_START"`, `installation_id=None` |
| 5 | **P1-4**: `build_identify` с пустым токеном → `IdentifyError::EmptyToken` | `E0599: no variant EmptyToken` | Проверка в `identify_value` (общая для `build_identify` и `build_identify_frame`): пустой/пробельный токен → `EmptyToken`, симметрично `ResumeError::EmptyToken` |

### Решения и TODO

- **Схема `Authorization` параметризована** (`AuthScheme`), дефолт — сырой user-токен.
  Причина: в захвате значения санитизированы (`Authorization: REDACTED`), U-06
  `docs/re/rest-d02.md` — «Bearer vs сырой токен» не подтверждено. TODO(NET-A) стоит в
  rustdoc `AuthScheme`/`from_token`: после подтверждения фиксируем схему (или `Bot`/`Bearer`).
- **`X-Debug-Options`** — `bugReporterEnabled`: значение из живого снимка D-02
  (`d02-snapshot/capture-run2.snap.jsonl`, 221/221 запросов).
- **`X-Discord-Locale`/`X-Discord-Timezone`** — из аргументов; пустые отвергаются.
- **`Accept-Language`** — пока локаль как есть; живой q-список (`navigator.languages`:
  `ru,ru-RU;q=0.9,ru-RU;q=0.8`) требует регионального профиля, которого в захвате нет —
  оставлен TODO(NET-A) в rustdoc `from_token`.
- `ApiAuth::from_token` возвращает `Result<Self, ApiAuthError>`: запрос с пустой «шапкой»
  (прежний дефолт `ApiAuth::default()` — шесть пустых строк) не должен уходить в сеть.
- `ApiConfig::default()` сохранён для стабов, но в rustdoc явно указано: production-auth —
  только из `from_token`.
- Добавлена строка `pub mod user;` в `src/rest/mod.rs` (в списке файлов брифа её не было,
  но без объявления модуля новый `src/rest/user.rs` недоступен). Других правок вне списка нет.
- Новых внешних крейтов нет: `discord-json` переехал из `[dev-dependencies]` в `[dependencies]`
  (тот же workspace-крейт).

## 2. Тесты

| Прогон | Было (baseline) | Стало |
|---|---:|---:|
| `cargo test --offline -p discord-core` | **531 passed / 0 failed** | **552 passed / 0 failed** |
| targeted: `--test rest_client --test identify --test rest_user` | — | 22 + 5 + 10 = **37 passed** |

Новые тесты (21):
- `tests/rest_client.rs` (+6): 6 непустых заголовков и порядок; trim/пустые входы (`EmptyToken`,
  `EmptyLocale`, `EmptyTimezone`); параметр схемы (`User`/`Bot`/`Bearer`); токен не в `Debug`;
  `HttpRequest::Debug` редактирует `Authorization`+`Cookie`; заголовки из `from_token`
  реально уходят в loopback-стаб (все шесть непустые).
- `tests/rest_user.rs` (+10): маппинг полей; nullable `global_name`/`avatar`; отсутствующий
  `bot=false`; `bot=true`; битый JSON; тело 401 → `Unauthorized`; прочее тело API → `Discord`;
  не-объект/нет поля; неверный тип; не-UTF-8.
- `tests/identify.rs` (+5): совпадение 14 полей с `SuperProperties`; смена launch_id/UA/версии
  (движок не подменяется версией клиента); пустой и пробельный токен → `EmptyToken` для
  `build_identify` и `build_identify_frame`; непустой токен собирается.

## 3. Гейты

| Проверка | Результат |
|---|---|
| `cargo test --offline -p discord-core` | 552 passed, 0 failed (лог `03-green-full.log`) |
| `cargo clippy --offline -p discord-core --all-targets -- -D warnings` | чисто (лог `04-clippy.log`) |
| `cargo fmt -p discord-core -- --check` | чисто |

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/Cargo.toml` | `ad18127ac28403b6466f632177ed067a` |
| `rust/crates/discord-core/src/rest/client.rs` | `9fcb7580b5ad22045315c791b78a548f` |
| `rust/crates/discord-core/src/rest/transport.rs` | `dca95232f2f9d35fa49d873a6b4609be` |
| `rust/crates/discord-core/src/rest/mod.rs` | `2f1ab97f2ff872e371f02ce1020eae8d` |
| `rust/crates/discord-core/src/rest/user.rs` (new) | `c9cde2863a89327ce03ff04ca5f7a15d` |
| `rust/crates/discord-core/src/identify.rs` | `89615bd51f9fa34a5de04de9111d33e5` |
| `rust/crates/discord-core/src/lib.rs` | `5542e0fe1aa81ba19de71258ca4c4a87` |
| `rust/crates/discord-core/tests/rest_client.rs` | `9985fe6e35b326a3cd595709294e1af5` |
| `rust/crates/discord-core/tests/identify.rs` (new) | `26b6595b16290cd4dc7b2ad4cb7f3feb` |
| `rust/crates/discord-core/tests/rest_user.rs` (new) | `4f9ff1944f2ec36ae0d919e48a5a658f` |

`src/lib.rs` дополнительно ре-экспортирует `ApiAuth`, `ApiAuthError`, `AuthScheme`, `Me`,
`MeError`, `parse_me`.

## 5. Что осталось (вне C1)

- **NET-C2**: BFF-команды `auth_status`/`login_token`/`logout`, поток-владелец `Keyring`
  (`!Send`), восстановление сессии, `AuthState` для UI, обработка 401/429 в строках UI.
- **NET-E**: URL шлюза, супервайзер (`plan_reconnect` + backoff + лимит), классификация
  close-кодов, `session_id`/`seq`/`resume_url` в store `kv`.
- **NET-A**: подтвердить схему `Authorization` (U-06) и точный профиль `Accept-Language`;
  после этого убрать оба TODO и зафиксировать значения тестами против живого артефакта.
- Проактивные rate-limit bucket'ы (P1-3), keep-alive (P1-5), единая политика ретраев (P1-7) —
  как и было, отдельные карточки.
