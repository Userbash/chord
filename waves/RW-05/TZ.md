### ТЗ RW-05 — Идентификация: IDENTIFY (op2) + READY (P1-04; T-GW-02, T-GW-04)

**Цель**: реализовать шаг идентификации поверх готового транспорта RW-04:
сборку IDENTIFY-терма (op2) по структуре реального клиента и разбор первого READY — с интеграцией в
цикл `connect_and_run` (отправка IDENTIFY после HELLO) и в пайп (`last_ready`).

**Появление оракулов (лаборатория, только чтение)**:
- Первоисточник IDENTIFY — код веб-клиента Discord:
  `/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`
  (offset ~4907670 `handleIdentify`, ~5019012 `_doIdentify`), факты — `/home/sanya/re-deep/rest-static.md` §4.
  Ключевой фрагмент `_doIdentify` (цитата из бандла):
  ```js
  let{token:l,properties:o={},presence:c}=e;
  let u={token:l,
    capabilities:(useChannelObfuscation ? 1767421 : 1734653),
    properties:o,
    presence:c,
    compress:this.compressionHandler.usesLegacyCompression(),
    client_state:s,      // {guild_versions:{}} — минимум; расширенный набор версий — опционально
    qos_token:a},        // protobuf discord_protos.qos_token.v1.QosToken (client_provided/derived)
  ```
  `handleIdentify` (truthy из бандла): `properties = {...superProperties, client_app_state, is_fast_connect:false, gateway_connect_reasons, installation_id?}`, `presence = getInitialState()`.
  Super-properties (из бандла): `os, browser, release_channel, client_version, os_version, os_arch, app_arch,
  system_locale, has_client_mods, client_launch_id, browser_user_agent, browser_version,
  [window_manager, distro, runtime_environment, display_server | os_sdk_version], client_build_number,
  native_build_number, client_event_source, install_source`; `client_app_state ∈ {"focused","unfocused"}`.
  Protobuf QoS (из бандла): `QosToken{1: client_provided{1: is_active(bool)}, 2: derived{1: claims(bytes), 2: signature(bytes), 3: key_id(uint32)}}`;
  минимальное кодирование `client_provided{is_active:false}` = байты `0A 02 08 00`.
- READY — уже имеющийся артефакт RW-04: `/home/sanya/re-deep/gw-proto/zstd-frames-95974_1389.hexl`
  (137 кадров; второй терм потока = READY, s=1). Контрольные числа (перепроверены координатором
  по `gw-proto/msgs-run2-95974_1389.jsonl`, снимок): `top_keys=40`, `guilds=7`, `private_channels=17`,
  `users=146`, `relationships=60`, `read_state={entries:157, partial:false, version:128949}`,
  `user_guild_settings={entries:58, partial:false, version:734}`, `sessions=1`, `experiments=6`,
  `merged_members=7`, `user` — 32 ключа, `user.id="384299970632876032"`, `session_id` — 32 символа,
  `resume_gateway_url="wss://gateway-us-east1-d.discord.gg"`.

**Файлы (только эти)**:
- `rust/crates/discord-core/src/identify.rs` — новый модуль: параметры, сборка IDENTIFY-терма, QoS-байты.
- `rust/crates/discord-core/src/ready.rs` — новый модуль: сводка READY.
- `rust/crates/discord-core/src/ws.rs` — точечно: `RunOptions`/`connect_and_run_with`, отправка
  IDENTIFY один раз после первого HELLO (нужен вызывающему — см. API), `last_ready` в пайпе.
- `rust/crates/discord-core/src/lib.rs` — `pub mod identify; pub mod ready;`, ре-экспорты, статус.
- `rust/crates/discord-core/tests/identify_ready.rs` — новый тест-файл (+ при необходимости
  `rust/crates/discord-core/tests/common/mod.rs`).
**`Cargo.toml`/`Cargo.lock` не трогать** — новых зависимостей нет.

**API (границы; детали — на усмотрение воркера)**:
```rust
// identify.rs
pub const CAPABILITIES_ACTIVE: u64 = 1_734_653;
pub const CAPABILITIES_CHANNEL_OBFUSCATION: u64 = 1_767_421;
pub struct IdentifyProperties { /* поля из списка супер-свойств (см. выше), String/bool/u64/Option<String>;
    fn for_rw_client() -> Self — дефолты нашего клиента (Linux, "Discord Client", release_channel "stable", …) */ }
pub struct PresenceInfo { pub status: String, pub since: u64, pub activities: Vec<EtfValue>, pub afk: bool }
pub struct IdentifyParams { pub token: String, pub capabilities: u64, pub properties: IdentifyProperties,
    pub presence: Option<PresenceInfo>, pub compress: bool,
    pub client_state_guild_versions: Vec<(String, String)>, pub qos_token: Vec<u8> }
impl std::fmt::Debug for IdentifyParams { /* token -> "REDACTED" */ }
pub enum IdentifyError { QosTokenNotUtf8, Encode(EtfError) /* … */ }  // Display + Error
pub fn build_identify(p: &IdentifyParams) -> Result<Vec<u8>, IdentifyError>; // ETF-терм с ведущим 0x83

// ready.rs
pub struct ReadySummary { pub top_keys: usize, pub guilds: usize, pub private_channels: usize,
    pub users: usize, pub relationships: usize, pub read_state_entries: usize,
    pub read_state_partial: Option<bool>, pub read_state_version: Option<i64>,
    pub user_guild_settings_entries: usize, pub user_keys: Option<usize>, pub user_id: Option<String>,
    pub session_id: Option<String>, pub resume_gateway_url: Option<String>,
    pub sessions: usize, pub experiments: usize, pub merged_members: usize }
impl ReadySummary { pub fn from_dispatch(value: &EtfValue) -> Option<ReadySummary>; }
// from_dispatch: op==0, t=="READY", d — map; иначе None. partial — атом "true"/"false" или int 0/1.

// ws.rs (дополнения)
pub struct RunOptions { pub identify: Option<IdentifyParams> }
impl RunOptions { pub fn none() -> Self }
pub async fn connect_and_run_with(url: &str, pipe: &mut GatewayPipe, opts: RunOptions) -> Result<RunOutcome, TransportError>;
// connect_and_run(url, pipe) остаётся; = connect_and_run_with(url, pipe, RunOptions::none())
impl GatewayPipe { pub fn last_ready(&self) -> Option<&ReadySummary>; }
```
Поведение цикла: IDENTIFY отправляется **один раз** после того, как сессия увидела HELLO
(`session().hello().is_some()`), только если `opts.identify` задан; повторные HELLO — не повторяют
отправку. В `push_binary` после успешного `on_value` каждой op0-распаковки пробовать
`ReadySummary::from_dispatch` и сохранять результат в пайпе. Никаких паник/`unwrap` на входных данных;
`unsafe` не вводить; токен не попадает в Debug/логи/ошибки.

**Кейсы (red-first; имена латиницей; комментарии по-русски; оракулы — выше)**:
Юнит (`identify`):
1. `identify_has_evidence_fields` — сборка с дефолтами (`for_rw_client`, token `"TEST_TOKEN"`):
   декод терма → top-level ключи ровно `{token, capabilities, properties, presence, compress,
   client_state, qos_token}`; `token == "TEST_TOKEN"`; `capabilities == CAPABILITIES_ACTIVE`;
   `compress == false`; `presence == Nil`; `client_state == {guild_versions: {}}`;
   `qos_token` == байты `0A 02 08 00`.
2. `identify_capabilities_can_enable_obfuscation` — с `CAPABILITIES_CHANNEL_OBFUSCATION` → 1767421 в терме.
3. `identify_properties_evidence_defaults` — в `properties` есть ключи: `os`, `browser`("Discord Client"),
   `release_channel`, `client_version`, `os_version`, `os_arch`, `app_arch`, `system_locale`,
   `has_client_mods`(false), `client_launch_id`, `browser_user_agent`, `browser_version`,
   `client_build_number`(int), `client_app_state`("focused"), `is_fast_connect`(false),
   `gateway_connect_reasons`(массив), и `installation_id` — только если задан.
4. `identify_presence_option_encodes_map` — `Some(PresenceInfo{status:"online", since:0, activities:[], afk:false})`
   → в терме map с ключами `{status, since, activities, afk}`; `None` → `Nil` (см. кейс 1).
5. `identify_client_state_guild_versions` — `[("1014537851418980372","17393980185986048")]` →
   `client_state.guild_versions` — map с этой парой.
6. `identify_qos_token_minimal_and_custom` — кастомные байты `0A 02 08 01` → в терме как строка этих байт;
   не-UTF8 байты (`FF FE`) → `Err(IdentifyError::QosTokenNotUtf8)` (без паники).
7. `identify_debug_redacts_token` — `format!("{:?}", params)` не содержит `TEST_TOKEN` (есть `REDACTED`).
8. `identify_size_is_plausible_against_943` — с токеном 59 символов и явными свойствами размер терма
   в `500..=1200` Б (реальный IDENTIFY в захвате — 943 Б; точное сравнение невозможно: токен/свойства
   реального клиента отличаются — задокументировать в комментарии).

Юнит (`ready`):
9. `ready_summary_from_synthetic_dispatch` — синтетический терм (op0, t=READY, d с 2 guilds, 3 users,
   1 private channel, read_state{entries 2, partial "false", version 7}, user_guild_settings{entries 1},
   user{id}, session_id, resume_gateway_url) → сводка со всеми полями; `read_state_partial == Some(false)`.
10. `ready_summary_none_for_non_ready` — терм без READY-формы (op10 HELLO; op0 с t=OTHER; op0 с d без
    `guilds`) → `None`.

Lab (skip-aware; нет `RE_LAB_ROOT`/файла → skip с причиной):
11. `lab_ready_from_run2_stream` — `{RE_LAB_ROOT:-/home/sanya/re-deep}/gw-proto/zstd-frames-95974_1389.hexl`:
    подать все кадры через `ZstdStream` (как в RW-04), разобрать термы, найти READY; сверить все числа
    из блока «READY» выше (40/7/17/146/60/157/false/128949/58/1/6/7/32 ключей user/…) и
    `resume_gateway_url`; `user_id` и `session_id` (длина 32) — как указано.

Стаб (реальные сокеты `127.0.0.1`; `tokio::time::timeout(5 с)`):
12. `ws_identify_sent_once_after_hello` — стаб: HELLO(60 мс) → ждёт IDENTIFY, декодирует терм,
    проверяет поля (op=2, token placeholder, capabilities 1734653, `properties.is_fast_connect == false`,
    `client_state.guild_versions == {}`), отвечает READY (s=1) и ACK на heartbeat; клиент —
    `connect_and_run_with(... RunOptions{identify: Some(...)})`; после N ударов стаб закрывает →
    `Closed`; тест: IDENTIFY ровно 1 (счётчик стаба), `pipe.session().last_seq() == Some(1)`,
    `pipe.last_ready()` — числа синтетического READY (2 гильдии и т.д.), `rtt_ms()` измерен.
13. `ws_no_identify_without_option` — стаб: HELLO + ACK-цикл; клиент `connect_and_run` (без опции) →
    стаб не получил ни одного op2 за сессию; протокол не сломан (удары идут, зомби нет).

**Порядок работы (hard TDD)**:
1. Сначала тесты (13) + заглушки `identify.rs`/`ready.rs` (и минимальные правки `ws.rs` под API),
   red-прогон: `cd rust && cargo test --offline -p discord-core 2>&1 | tee /tmp/opencode/rw-05/red.txt`
   (в лог — `# EXIT=…` и md5 тест-файла; политика нита RW-04).
2. Реализация до green: `cargo test --offline -p discord-core`, затем весь воркспейс `cargo test --offline`.
3. `cargo clippy --offline -p discord-core --all-targets -- -D warnings`; `cargo fmt -p discord-core`
   (в конце `-- --check`).
4. `npm run gate:worker` из корня — passed.
5. Отчёт §4.2 → `/tmp/opencode/rw-05/report.md` (red/green с EXIT и md5, gate-артефакт, файлы + md5,
   риски; отдельно: что сверено с бандлом/лабораторией и что осталось неподтверждаемым офлайн —
   e.g. точные значения `gateway_connect_reasons` и форма `presence` на живом логине). Верни текст отчёта.

**Запрещено**: сетевые подключения к не-localhost; правки `Cargo.toml`/`Cargo.lock`; golden
(`logs/`, `functions/`, `evidence/`); запись в `/home/sanya/re-deep` (только чтение); `docs/`,
`TASKS.md`; чужие крейты; коммиты; паники/`unwrap` на входных данных; утечка токена в Debug/ошибки.
