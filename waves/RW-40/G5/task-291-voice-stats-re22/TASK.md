# TASK — task-291 (`G5 Voice L2 / P1`, `RE-22-01`: `ConnectionQuality`, `PingHistory` (199/20) и `RtcConnectionStatsSnapshot` в `discord-voice/src/stats.rs`)

- **Отдел:** G5 Voice / Platform (`DeepSeek` — `--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Уровень сложности / Приоритет:** `L2 / P1` (по `waves/RW-40/PM/MODEL-ROUTING-CRITERIA.md`)
- **Рабочее дерево:** `/home/user/orca/workspaces/project/feature-task-291-voice-stats-re22` (`branch:feature/task-291-voice-stats-re22`)
- **Разрешённый к изменению файл (строго 1 файл):**
  - `rust/crates/discord-voice/src/stats.rs` (строки после строки 469 + расширение `#[cfg(test)] mod tests`)
- **Артефакты отчёта:**
  - `/home/user/orca/project/waves/RW-40/G5/task-291-voice-stats-re22/REPORT.md`
  - `/home/user/orca/project/waves/RW-40/G5/task-291-voice-stats-re22/red.log`
  - `/home/user/orca/project/waves/RW-40/G5/task-291-voice-stats-re22/green.log`

## Источники контракта (прочитай сразу, не сканируя весь репозиторий)
- `/home/user/orca/project/waves/RW-40/PM/BACKLOG-RE-22.md` (§ `RE-22-01`, строки 20–44)
- `/home/user/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`

## Требования к реализации (Strict TDD: сначала напиши тесты в `stats.rs` с заглушками -> `red.log`, затем реализацию -> `green.log`)

1. **Константы и типы в `rust/crates/discord-voice/src/stats.rs` (по оракулу `decoded/3eefa7dae1e2c234_0`, модули `613681` и `206959`):**
   - `pub const PING_FINE_MAX_MS: u64 = 250;`
   - `pub const PING_AVERAGE_MAX_MS: u64 = 500;`
   - `pub const LOSS_FINE_MAX_PCT: f64 = 5.0;`
   - `pub const LOSS_AVERAGE_MAX_PCT: f64 = 10.0;`
   - `pub const MAX_PING_HISTORY_LEN: usize = 199;`
   - `pub const PING_AVERAGE_WINDOW: usize = 20;`
   - `pub const DEFAULT_PING_TIMEOUT_MS: u64 = 4_000;`
   - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub enum ConnectionQuality { Unknown, Fine, Average, Bad }` с методом `pub fn evaluate(connected: bool, last_ping_ms: Option<u64>, outbound_loss_rate_pct: Option<f64>) -> Self` и `pub const fn as_str(self) -> &'static str` (`"unknown"`, `"fine"`, `"average"`, `"bad"`):
     - если `!connected` или `last_ping_ms.is_none()` -> `Self::Unknown`;
     - если `outbound_loss_rate_pct` задан и (`!loss.is_finite() || loss < 0.0 || loss > 100.0`) -> `Self::Unknown`;
     - если `ping > 500` ИЛИ `loss > 10.0` -> `Self::Bad`;
     - если `ping > 250` ИЛИ `loss > 5.0` -> `Self::Average`;
     - иначе -> `Self::Fine`.
   - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct PingSample { pub time_ms: u64, pub value_ms: u64 }`
   - `#[derive(Debug, Clone, Default)] pub struct PingHistory` (ёмкость `199` элементов `VecDeque<PingSample>`, `ping_bad_count: u64`, `timeout_seqs: Vec<u64>`):
     - `pub fn new() -> Self`
     - `pub fn push_ping(&mut self, time_ms: u64, value_ms: u64)`: если `value_ms > 500`, инкрементирует `ping_bad_count`; поддерживает `while samples.len() >= 200 { pop_front }` до/при вставке так, чтобы в буфере хранилось не более `199` точек (`MAX_PING_HISTORY_LEN`).
     - `pub fn note_ping_timeout(&mut self, seq: u64, time_ms: u64) -> bool`: записывает `seq` в историю таймаутов; если последние 3 таймаута последовательны (`s[n-1] == s[n-2] + 1 && s[n-2] == s[n-3] + 1`), вызывает `self.push_ping(time_ms, DEFAULT_PING_TIMEOUT_MS)` и возвращает `true`, иначе `false`.
     - `pub fn average_ping_ms(&self) -> u64`: среднее по срезу первых `min(len, 20)` элементов (целочисленное округление или деление по модулю `613681`: `(sum as f64 / count as f64).round() as u64`; `0`, если буфер пуст).
     - `pub fn last_ping_ms(&self) -> Option<u64>`, `pub fn ping_bad_count(&self) -> u64`, `pub fn samples(&self) -> &VecDeque<PingSample>`, `pub fn clear(&mut self)`.
   - `#[must_use] pub fn compute_outbound_loss_rate_pct(prev_sent: u64, prev_lost: i64, curr_sent: u64, curr_lost: i64) -> Option<f64>`:
     - если `curr_sent < prev_sent` или `curr_lost < prev_lost` -> `None`;
     - `delta_sent = curr_sent - prev_sent`; `delta_lost = (curr_lost - prev_lost) as u64`;
     - если `delta_sent == 0` -> `Some(0.0)`;
     - иначе `Some(((delta_lost as f64) / ((delta_sent + delta_lost) as f64)).clamp(0.0, 1.0) * 100.0)`.
   - `#[must_use] pub fn short_hostname(endpoint: &str) -> &str`:
     - обрезает пробелы, возвращает подстроку до первой `.` или `:` (например, `"finland10192.discord.media:443"` -> `"finland10192"`).
   - `#[derive(Debug, Clone, PartialEq)] pub struct RtcConnectionStatsSnapshot`:
     - `pub short_hostname: String`, `pub last_ping_ms: Option<u64>`, `pub average_ping_ms: u64`, `pub outbound_loss_rate_pct: Option<f64>`, `pub quality: ConnectionQuality`, `pub packets_sent: u64`, `pub packets_lost: i64`, `pub bytes_sent: u64`, `pub bytes_received: u64`, `pub decryption_failures: u64`, `pub secure_frames_protocol_version: u16`.
2. **Проверки (с `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`):**
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-voice`
   - `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-voice --tests -- -D warnings`
3. Запиши `red.log`, `green.log` и `REPORT.md` (с `md5sum rust/crates/discord-voice/src/stats.rs`) и заверши задачу через `orca orchestration complete`. Не делай `git commit/push`.
