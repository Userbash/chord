# TASK-294 (RE-22-02) — BFF-модуль `voice_diagnostics.rs` и IPC-контракт диагностики голоса (`L2 / P1`)

- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Уровень сложности / Модель:** `L2 (Типовая задача 2–4 файла)` → `DeepSeek` (`--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Приоритет:** `P1`
- **Ветка:** `feature/task-294-be-voice-diagnostics` (от `unstable`)
- **Источник:** `waves/RW-40/PM/BACKLOG-RE-22.md` (`RE-22-02`) и `waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`

---

## 1. Строгий Write-Scope (ровно 4 файла)

1. `rust/gui-spike/src/voice_diagnostics.rs` (новый BFF-модуль)
2. `rust/gui-spike/src/lib.rs` (добавить `pub mod voice_diagnostics;`)
3. `rust/gui-spike/tests/voice_diagnostics_contract.rs` (новый контрактный тест)
4. `scripts/test-impact.mjs` + `tests/unit/test-impact.test.mjs` (добавить правило `voice_diagnostics` → `voice_diagnostics_contract` в `RUST_SPIKE_MODULES` и в `tests/unit/test-impact.test.mjs`)

Никакие другие файлы менять нельзя.

---

## 2. Точный контракт (`rust/gui-spike/src/voice_diagnostics.rs`)

Оракул `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`:
- Модуль `978171` (`offset 7458500..7465900`) — поля попапа `RTCConnectionPopout`.
- Модуль `412780` (`offset 9264635`) — секции `RTCDebugModal` (`zWA`: `TRANSPORT`, `OUTBOUND`, `INBOUND`, `SCREENSHARE`, `CAMERA`, `CLIPS`, `STREAMS_OUTBOUND`, `STREAMS_INBOUND`), скользящее окно `600` точек `{ value, time }` в `RTCDebugStore` и `30` снимков в `MediaEngineStatsStore`.
- Модули `917592` (`offset 6025550`) и `998759` (`offset 7464200`) — `Voice Privacy Code` (DAVE / Secure Frames): 30 десятичных цифр, разбитых на 6 групп по 5 цифр.

### Обязательные константы и типы:
1. Константы:
   - `pub const MAX_DEBUG_SERIES_POINTS: usize = 600;`
   - `pub const MAX_MEDIA_ENGINE_SNAPSHOTS: usize = 30;`
   - `pub const PRIVACY_CODE_GROUP_COUNT: usize = 6;`
   - `pub const PRIVACY_CODE_GROUP_LEN: usize = 5;`
   - `pub const PRIVACY_CODE_TOTAL_DIGITS: usize = 30;`
2. Перечисление `VoiceDebugModalSection`:
   - Варианты (`#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]`, `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`):
     `Transport`, `Outbound`, `Inbound`, `Screenshare`, `Camera`, `Clips`, `StreamsOutbound`, `StreamsInbound`.
   - Метод `pub const fn as_str(self) -> &'static str` возвращающий `"TRANSPORT"`, `"OUTBOUND"`, `"INBOUND"`, `"SCREENSHARE"`, `"CAMERA"`, `"CLIPS"`, `"STREAMS_OUTBOUND"`, `"STREAMS_INBOUND"`.
3. Структура `TimeSeriesPoint`:
   - `#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]`
   - Поля: `pub time_ms: u64`, `pub value: f64`.
4. Структура `PingSparklinePointDto`:
   - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]`
   - Поля: `pub time_ms: u64`, `pub value_ms: u64`.
5. Структура `VoiceDiagnosticsPopoutDto` (`#[serde(rename_all = "camelCase")]`):
   - `pub state: String` (например `"RTC_CONNECTED"` или `"DISCONNECTED"`),
   - `pub quality: String` (`"fine"`, `"average"`, `"bad"`, `"unknown"` — из `discord_voice::stats::ConnectionQuality`),
   - `pub short_hostname: String` (результат `discord_voice::stats::short_hostname(&endpoint)`),
   - `pub average_ping_ms: u64`,
   - `pub average_ping_formatted: String` (`format!("{}", average_ping_ms)` — эквивалент `toFixed(0)`),
   - `pub last_ping_ms: Option<u64>`,
   - `pub last_ping_formatted: Option<String>` (`last_ping_ms.map(|v| format!("{v}"))`),
   - `pub outbound_loss_rate_pct: Option<f64>`,
   - `pub outbound_loss_formatted: Option<String>` (`outbound_loss_rate_pct.map(|v| format!("{v:.1}"))` — эквивалент `toFixed(1)`),
   - `pub pings: Vec<PingSparklinePointDto>`,
   - `pub is_end_to_end_encrypted: bool`,
   - `pub secure_frames_protocol_version: u32`,
   - `pub privacy_code_chunks: Vec<String>` (пустой `Vec`, если `!is_end_to_end_encrypted` или код отсутствует; иначе 6 групп по 5 цифр).
6. Функция `pub fn format_voice_privacy_code(raw: &str) -> Result<[String; 6], String>`:
   - Отбирает из `raw` все ASCII-цифры (`c.is_ascii_digit()`) ИЛИ если в `raw` содержатся любые непробельные и недефисные нецифровые символы — возвращает `Err(...)`.
   - Требует ровно `30` цифр (`PRIVACY_CODE_TOTAL_DIGITS = 30`). Если цифр `< 30` или `> 30` — возвращает `Err(...)`.
   - Возвращает массив из 6 строк по 5 цифр (`[String; 6]`).
7. Функция построения `pub fn build_popout_dto(...) -> VoiceDiagnosticsPopoutDto`:
   - Принимает `connected: bool`, `state_label: &str`, `endpoint: &str`, `ping_history: &discord_voice::stats::PingHistory`, `outbound_loss_rate_pct: Option<f64>`, `secure_frames_protocol_version: u32`, `raw_privacy_code: Option<&str>`.
   - Вычисляет `last_ping_ms = ping_history.last_ping_ms()`, `average_ping_ms = ping_history.average_ping_ms()`, `quality = discord_voice::stats::ConnectionQuality::evaluate(connected, last_ping_ms, outbound_loss_rate_pct).as_str().to_string()`.
   - Если `secure_frames_protocol_version > 0`, то `is_end_to_end_encrypted = true`, а `privacy_code_chunks` заполняется из `raw_privacy_code` через `format_voice_privacy_code` (при `None` или `Err` — пустой `Vec`).
8. Структура `VoiceDebugHistoryStore`:
   - Хранит временные ряды `BTreeMap<String, VecDeque<TimeSeriesPoint>>` с жёстким лимитом `MAX_DEBUG_SERIES_POINTS = 600` точек на ключ (`push_metric_point(&mut self, key: &str, time_ms: u64, value: f64)` — при `len >= 600` вытесняет старейшую точку `pop_front`).
   - Хранит буфер снимков `VecDeque<discord_voice::stats::RtcConnectionStatsSnapshot>` с жёстким лимитом `MAX_MEDIA_ENGINE_SNAPSHOTS = 30` (`push_snapshot(&mut self, snapshot: RtcConnectionStatsSnapshot)` — при `len >= 30` вытесняет старейший снимок `pop_front`).
   - Методы чтения: `series(&self, key: &str) -> Option<&VecDeque<TimeSeriesPoint>>`, `snapshots(&self) -> &VecDeque<RtcConnectionStatsSnapshot>`, `clear(&mut self)`.
   - Метод безопасного экспорта JSON-журнала `pub fn export_sanitized_json(&self) -> Result<String, String>`: сериализует историю рядов и снимков в JSON-строку; гарантирует отсутствие полей `"secret_key"`, `"session_token"`, `"token"` в выводе.

---

## 3. Команды верификации (Targeted T0)

Используй общий `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`:
```bash
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test voice_diagnostics_contract
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test voice_diagnostics_contract -- -D warnings
node --test tests/unit/test-impact.test.mjs
```
Отчёт оформи в `waves/RW-40/G1/task-294-be-voice-diagnostics/REPORT.md`. Не делай git merge в `unstable`.
