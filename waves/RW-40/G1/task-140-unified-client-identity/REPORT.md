# Отчёт воркера — task-140 (BE-RE-04): единая `ClientIdentity` для IDENTIFY и X-Super-Properties

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-140` / `BE-RE-04`, находка `F-ID-01` (в аудите зафиксирована как `F-GW-09`,
  §3.8 `RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md`): «IDENTIFY и SP — разные значения».
- **Файлы воркера**: `rust/crates/discord-core/src/identify.rs`,
  `rust/crates/discord-core/src/rest/identity.rs`. `super_properties.rs` не понадобился.
- **Ветка/worktree**: `feature-task-140-unified-client-identity`, HEAD `4e24048`,
  `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`.
- **Независимые оракулы**: код веб-клиента, `handleIdentify` в бандле
  `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` —
  `properties: {...L.default.getSuperProperties(), client_app_state, is_fast_connect,
  gateway_connect_reasons, ...installation_id}`; живые значения 22 полей —
  `docs/re/rest-d02.json` (`auth.superProperties`, `95974.2323`).

## 2. Что сделано

Появился единый носитель значений — `ClientIdentity` в `rest/identity.rs`. Он хранит ровно
один `SuperProperties`, поэтому REST-заголовок и OP2 IDENTIFY физически не могут получить
разные `browser`, `os_arch`, `client_launch_id`, `launch_signature` и остальные общие поля.

1. `rest/identity.rs`:
   - `pub struct ClientIdentity { super_properties: SuperProperties }` с конструкторами
     `new(SuperProperties)`, `build(&dyn IdentitySources, &IdentityConfig)` и `rw_client()`;
   - `rw_client()` кэшируется через `OnceLock`: повторные вызовы в процессе (REST-клиент и
     Gateway) получают один и тот же `client_launch_id`/`launch_signature`;
   - дефолт `rw_client()` сохраняет прежние публичные значения IDENTIFY (`browser =
     "Discord Client"`, UA `discord-lab-rs/<version>`, `client_build_number = 641004`,
     `system_locale = "en-US"`), но `os`/`os_arch` берутся из честной среды (`x86_64 → x64`).
2. `identify.rs`:
   - `IdentifyProperties` получил поле `launch_signature` (14 → 15 общих полей с SP) и
     `PartialEq/Eq`; поле сериализуется в `properties` OP2;
   - `IdentifyProperties::from_client_identity(&ClientIdentity)` и
     `ClientIdentity::to_identify_properties(&self)` — единственный путь сборки IDENTIFY;
   - `IdentifyProperties::for_rw_client()` теперь делегирует
     `ClientIdentity::rw_client()`; `from_super_properties` сохранён для обратной совместимости.
3. Публичные сигнатуры сохранены; `build_identify`/`build_identify_frame` не менялись.

## 3. RED

`RED.log` — до реализации, `cargo test --offline -p discord-core --lib`:

```
error[E0432]: unresolved import `crate::rest::identity::ClientIdentity`
error[E0609]: no field `launch_signature` on type `&identify::IdentifyProperties`
error[E0599]: no associated function ... `from_client_identity`
error[E0433]: cannot find type `ClientIdentity` ... (identity.rs)
error: could not compile `discord-core` (lib test) due to 5 previous errors
EXIT=101
```

Тесты написаны по оракулу, а не по реализации: значения всех 22 полей различимы, поэтому
подмена любого общего поля видна; отдельный кейс разбирает собранный ETF и сравнивает
`properties` OP2 со `SuperProperties` по строкам.

## 4. GREEN

`.log`-файлы в каталоге карточки:

| Прогон | Команда | Результат |
|---|---|---|
| targeted lib | `cargo test --offline -p discord-core --lib` | 8 passed, 0 failed, `EXIT=0` (`GREEN-lib.log`) |
| все тесты discord-core | `cargo test --offline -p discord-core` | 53 набора `test result: ok`, 0 failed, `EXIT=0` (`GREEN-core.log`) |
| workspace | `cargo test --offline --workspace` | `EXIT=0` (`GREEN-workspace.log`) |
| gui-spike | `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | `EXIT=0` (`GREEN-gui-spike.log`) |
| clippy | `cargo clippy --offline -p discord-core --all-targets -- -D warnings` | `EXIT=0` (`CLIPPY.log`) |
| fmt | `cargo fmt --all -- --check` | `EXIT=0` (`FMT.log`) |

Ключевые зелёные кейсы:

- `identify::tests::client_identity_drives_identify_properties` — 15 общих полей, включая
  `launch_signature`, равны значениям SP;
- `identify::tests::identify_wire_matches_super_properties_byte_for_byte` — те же значения
  читаются из реального ETF-кадра OP2 (`client_launch_id`, `launch_signature`, `browser`,
  `os_arch`, UA, версии, build number);
- `rest::identity::tests::client_identity_builds_rest_and_identify_from_one_set_of_uuids` —
  три UUID (`client_launch_id`, `launch_signature`, `client_heartbeat_session_id`) берутся из
  трёх вызовов `random_bytes`, а `to_identify_properties()` новых UUID не генерирует;
- `identify::tests::rw_client_identity_matches_identify_defaults` — дефолтный клиент тоже
  имеет один отпечаток.

## 5. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/identify.rs` | `95cae2fdb08951759ae280e35554ce81` |
| `rust/crates/discord-core/src/rest/identity.rs` | `5f99d3e0c75b1745fa89964f00818c42` |
| `waves/RW-40/G1/task-140-unified-client-identity/RED.log` | см. файл |
| `waves/RW-40/G1/task-140-unified-client-identity/GREEN-lib.log` | см. файл |
| `waves/RW-40/G1/task-140-unified-client-identity/GREEN-core.log` | см. файл |
| `waves/RW-40/G1/task-140-unified-client-identity/GREEN-workspace.log` | см. файл |
| `waves/RW-40/G1/task-140-unified-client-identity/GREEN-gui-spike.log` | см. файл |
| `waves/RW-40/G1/task-140-unified-client-identity/CLIPPY.log` | см. файл |
| `waves/RW-40/G1/task-140-unified-client-identity/FMT.log` | см. файл |

`git status`: изменены только эти два `.rs` (плюс чужой `package-lock.json`, который был
изменён до начала работы и не трогался).

## 6. Риски и что осталось

1. **Боевая проводка не переключена.** `rust/gui-spike/src/net.rs:634` создаёт
   `SupervisorConfig::new` (внутри — `IdentifyProperties::for_rw_client()`), а REST-вызовы
   (`auth.rs`, `guilds.rs`, `messages.rs` и др.) отдельно зовут `identity::build(...)`. Пока
   эти два места не начнут брать один `ClientIdentity`, в живом процессе останутся два
   независимых набора UUID. Правка `gui-spike`/`supervisor.rs` — вне разрешённых мне файлов,
   нужна отдельная карточка или integration-патч координатора.
2. **Версия/UA vs build number.** Пара `client_version = 0.1.0` ↔
   `client_build_number = 641004` сохранена как была: продуктовое решение PM по маскировке
   под `1.0.162` в задаче не зафиксировано, менять его самовольно не стал.
3. **`identity::build` не кэширует UUID.** Он остаётся честным конструктором с тремя UUID на
   вызов (этого требуют тесты RW-09); единый отпечаток даёт только `ClientIdentity`.
