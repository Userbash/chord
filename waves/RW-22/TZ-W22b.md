# ТЗ W22b — P4-01: headless RTC signaling

Волна: RW-22 / B03. Владелец: W22b. Работать в `/home/sanya/orca/project`, не коммитить.

## 0. Обязательное чтение

`AGENTS.md`, `PROCESS.md` §4, `waves/RW-22/WAVE-PLAN.md`, `docs/re/voice-deep.md` §1,
`docs/rust-client/PLAN.md` (P4-01), `docs/rust-client/TEST-PLAN.md` (T-VOICE-01…03),
`rust/crates/discord-json/src/lib.rs`, `rust/crates/discord-voice/src/lib.rs`,
`/home/sanya/re-deep/voice/rtc1-frames.jsonl`, `rtc2-frames.jsonl`,
`rtc1-summary.json`, `rtc2-summary.json`.

Твои файлы — **только** `rust/crates/discord-voice/**` (включая его `Cargo.toml`). Не трогай
`Cargo.toml` workspace, `Cargo.lock`, `discord-core`, доки, `waves/*`. Нужен
`discord-json = { workspace = true }`.

## 1. Метод

Strict TDD. Сначала контрактные тесты, red-лог:

```bash
cd /home/sanya/orca/project/rust
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-voice \
  2>&1 | tee ../waves/RW-22/red-w22b.log
```

Red — compile-red по отсутствующему API или behavior-red, exit 101. Затем минимальная
реализация до green, лог `green-w22b.log`. Не правь тесты под код. Только headless: никаких
DNS/TCP/UDP/TLS/WS-подключений, никаких сетевых вызовов в тестах.

## 2. Публичный контракт

Сохрани существующие `pub const` (payload types, encryption mode, DAVE, heartbeat, keepalive,
`OBSERVED_RTC_OPS`). Добавь как минимум:

```rust
pub enum VoiceOp { Identify = 0, SelectProtocol = 1, Ready = 2, Heartbeat = 3,
                   SessionDescription = 4, HeartbeatAck = 6, Hello = 8,
                   Unknown(u8) }
impl VoiceOp { pub fn from_u8(raw: u8) -> VoiceOp; pub fn as_u8(self) -> u8; }

pub struct Hello { pub version: u8, pub heartbeat_interval_ms: u64 }
pub struct StreamSpec { pub kind: String, pub ssrc: Option<u64>, pub rtx_ssrc: Option<u64>,
                        pub rid: Option<String>, pub quality: Option<u16>, pub active: Option<bool> }
pub struct Identify { pub server_id: String, pub channel_id: String, pub user_id: String,
                      pub session_id: String, pub token: String,
                      pub max_dave_protocol_version: u8, pub video: bool,
                      pub streams: Vec<StreamSpec> }
pub struct Ready { pub ssrc: u64, pub port: u16, pub ip: String,
                   pub modes: Vec<String>, pub experiments: Vec<String>,
                   pub streams: Vec<StreamSpec> }
pub struct SessionDescription { pub mode: String, pub secret_key: Vec<u8>,
                                pub media_session_id: String, pub dave_protocol_version: u8,
                                pub secure_frames_version: u8, pub audio_codec: Option<String>,
                                pub video_codec: Option<String> }
pub enum VoicePayload { Hello(Hello), Identify(Identify), Ready(Ready),
                        SessionDescription(SessionDescription), Other { op: VoiceOp, raw: JsonValue } }
pub struct VoiceFrame { pub op: VoiceOp, pub payload: VoicePayload, pub seq: Option<u64> }
pub fn parse_frame(input: &str) -> Result<VoiceFrame, VoiceError>;
```

- `SecretKey`/`SessionDescription::secret_key` — 32 байта после base64 (в capture REDACTED;
  тест с реальной 32-байтовой base64-строкой). Реализуй стандартный base64 decode в крейте
  (без внешней зависимости); неверная длина/алфавит — ошибка. `Debug` **обязан** печатать
  `secret_key: "<redacted>"` (и не печатать token/session_id/media_session_id целиком);
  `Display` не должен раскрывать секрет.
- `parse_frame` принимает внутренний payload (`{"op":..,"d":..}`), не wrapper JSONL.
  Для `op 15/16/3/6` — `Other` с `raw`, но op/seq сохраняются. Неизвестный op — `Other`
  (или `Unknown`), не паника. Отсутствующий `d` у известных op — ошибка.
- `Identify`: `max_dave_protocol_version` = 1; streams rid `"100"`, quality 100; валидировать
  непустые id/token? Для тестов capture token REDACTED; не требовать формат snowflake.
- `SelectProtocol`: тип `SelectProtocol { protocol, address, port, mode, codecs }`,
  `Codec { name, kind, priority, payload_type }`; opus PT120 priority 1000, AV1 PT101.
  `parse_frame` для op1 (out) тоже поддержать.
- `Ready`: ssrc/port/ip/modes/experiments/streams; mode `aead_aes256_gcm_rtpsize` должен
  принимать; неизвестные modes сохранять (не отбрасывать).
- `SessionDescription`: mode, 32-byte key, media_session_id, DAVE/secure versions, codecs.

## 3. Endpoint и регион

```rust
pub struct VoiceServerUpdate { pub token: String, pub guild_id: String, pub endpoint: String }
impl VoiceServerUpdate {
    pub fn parse(input: &str) -> Result<Self, VoiceError>;
    pub fn websocket_url(&self) -> Result<String, VoiceError>;
}
pub fn select_region(samples: &[(String, u32)], geo: &[String],
                     manual: Option<&str>) -> Result<String, VoiceError>;
```

- `VOICE_SERVER_UPDATE` из gateway: поля `token`, `guild_id`, `endpoint`. Endpoint может быть
  `c-fra19-....discord.media:2053`; URL = `wss://<endpoint>/?v=9`. Если endpoint уже `wss://`,
  не дублировать схему; `http://`/`ws://`/пустой/с пробелами/управляющими — ошибка. Порт
  опционален; host — непустой, без `/`, `@`, `?`, `#`. Никакого DNS/сети.
- `select_region`: manual (если задан) должен быть в geo/available, иначе ошибка; иначе
  минимальная latency, tie-break по порядку `geo`, затем лексикографически. Пустые samples —
  первый geo или ошибка, если geo пуст. Дубликаты регионов — last-wins или min, зафиксируй
  тестом и комментарием; не паникуй.
- `Debug` `VoiceServerUpdate` не печатает token (только `<redacted>`).

## 4. Обязательные тесты (минимум)

1. Все 9 op из `OBSERVED_RTC_OPS`, mapping round-trip; неизвестный op.
2. Реальные `rtc1`/`rtc2` wrappers: распаковать `payload`, `parse_frame`, проверить Hello v8
   13750, Ready ssrc/port/ip/modes, SessionDescription mode/key length/versions. Если
   `/home/sanya/re-deep` нет — skip с причиной.
3. Identify из capture: server/channel/user ids, max_dave=1, video=true, stream rid/quality.
4. Select Protocol: protocol udp, codecs opus/AV1, PT120/101, priority 1000.
5. Secret key: 32 bytes из base64; неверная длина/алфавит; Debug/Display redaction
   (assert!(!debug.contains(secret))).
6. Endpoint: bare host:port → wss URL; already wss; reject http/ws/empty/space/`@`.
7. Region: latency order, tie geo/lex, manual valid/invalid, empty samples/geo.
8. Truncation/fuzz: 10k prefixes of frames не паникуют (parse возвращает Err).
9. Нет сети: в тестах не должно быть `TcpStream`, `UdpSocket`, `reqwest`, `tokio::net`.

## 5. Отчёт

Верни red/green команды+exit, файлы, число тестов, redaction-доказательство, риски/unknowns,
что отложено в P4-02 (UDP discovery/keepalive/ротация). Не запускай `cargo test --workspace`.
