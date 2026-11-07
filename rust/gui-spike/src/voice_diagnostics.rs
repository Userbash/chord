//! RW-40 / Task-294 (`RE-22-02`): BFF-модуль диагностики голоса — попап
//! `RTCConnectionPopout`, секции `RTCDebugModal`, скользящие окна истории
//! (`RTCDebugStore` 600 точек, `MediaEngineStatsStore` 30 снимков) и
//! `Voice Privacy Code` (30 десятичных цифр, 6 групп по 5).
//!
//! Нормативные оракулы (`/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`):
//! - модуль `978171` (`offset 7458500..7465900`) — поля попапа (`state`, `quality`,
//!   `shortHostname`, `averagePing`, `lastPing`, `outboundLossRate`, `pings`,
//!   `isEndToEndEncrypted`, `secureFramesProtocolVersion`);
//! - модуль `412780` (`offset 9264635`) — секции `zWA` (`TRANSPORT`, `OUTBOUND`,
//!   `INBOUND`, `SCREENSHARE`, `CAMERA`, `CLIPS`, `STREAMS_OUTBOUND`,
//!   `STREAMS_INBOUND`) и окно `600` точек `{ value, time }` / `30` снимков;
//! - модули `917592` (`offset 6025550`) и `998759` (`offset 7464200`) —
//!   `Voice Privacy Code` (DAVE / Secure Frames).
//!
//! Модуль намеренно самодостаточен: `discord-gui-spike` не зависит от крейта
//! `discord-voice` (write-scope задачи — 4 файла, `Cargo.toml` не трогаем).
//! Поэтому `ConnectionQuality`, `PingSample`/`PingHistory`, `short_hostname` и
//! `RtcConnectionStatsSnapshot` повторяют контракт `discord_voice::stats`
//! один-в-один (пороги `250`/`500` мс и `5`/`10`%, буфер `199`/`20`, таймаут
//! `4000` мс) и дополнительно сериализуются, чего в крейте голоса нет.
//!
//! Безопасность: DTO и экспортируемый журнал собираются из типизированных
//! структур с закрытым набором полей, а ключи рядов фильтруются по маркерам
//! `secret_key` / `session_token` / `token` — секреты не попадают в вывод
//! на уровне типов, без строковых замен «по месту».

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

/// Жёсткий лимит точек на одну метрику в `RTCDebugStore` (модуль `412780`).
pub const MAX_DEBUG_SERIES_POINTS: usize = 600;

/// Жёсткий лимит снимков в `MediaEngineStatsStore` (модуль `412780`).
pub const MAX_MEDIA_ENGINE_SNAPSHOTS: usize = 30;

/// Число групп `Voice Privacy Code` (модули `917592`/`998759`).
pub const PRIVACY_CODE_GROUP_COUNT: usize = 6;

/// Длина одной группы `Voice Privacy Code`, цифр.
pub const PRIVACY_CODE_GROUP_LEN: usize = 5;

/// Полная длина `Voice Privacy Code`, цифр.
pub const PRIVACY_CODE_TOTAL_DIGITS: usize = 30;

// --- Зеркало `discord_voice::stats` (см. §2 карточки RE-22-02) --------------

/// Верхняя граница `fine` по пингу, мс.
pub const PING_FINE_MAX_MS: u64 = 250;

/// Верхняя граница `average` по пингу, мс.
pub const PING_AVERAGE_MAX_MS: u64 = 500;

/// Верхняя граница `fine` по потере исходящих пакетов, %.
pub const LOSS_FINE_MAX_PCT: f64 = 5.0;

/// Верхняя граница `average` по потере исходящих пакетов, %.
pub const LOSS_AVERAGE_MAX_PCT: f64 = 10.0;

/// Длина буфера `RTCConnection._pings`.
pub const MAX_PING_HISTORY_LEN: usize = 199;

/// Окно усреднения `getAveragePing()` — первые 20 точек буфера.
pub const PING_AVERAGE_WINDOW: usize = 20;

/// Значение пинга после трёх подряд таймаутов, мс.
pub const DEFAULT_PING_TIMEOUT_MS: u64 = 4_000;

/// Маркеры секретных ключей, которые не должны попадать в журнал диагностики.
const SENSITIVE_METRIC_MARKERS: [&str; 3] = ["secret_key", "session_token", "token"];

/// Качество голосового соединения (`bFR` в эталонном клиенте, модуль `613681`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionQuality {
    /// Нет соединения или нет ни одного замера пинга.
    Unknown,
    /// Пинг `<= 250` мс и потери `<= 5%`.
    Fine,
    /// Пинг `<= 500` мс или потери `<= 10%`.
    Average,
    /// Пинг `> 500` мс или потери `> 10%`.
    Bad,
}

impl ConnectionQuality {
    /// Классификация соединения по последнему пингу и потере исходящих пакетов.
    ///
    /// Некорректная доля потерь (`NaN`, `±∞`, `< 0`, `> 100`) даёт [`Self::Unknown`]:
    /// такие данные не классифицируются, а не «округляются» в удобную сторону.
    #[must_use]
    pub fn evaluate(
        connected: bool,
        last_ping_ms: Option<u64>,
        outbound_loss_rate_pct: Option<f64>,
    ) -> Self {
        let Some(ping_ms) = last_ping_ms.filter(|_| connected) else {
            return Self::Unknown;
        };
        if let Some(loss) = outbound_loss_rate_pct {
            if !loss.is_finite() || !(0.0..=100.0).contains(&loss) {
                return Self::Unknown;
            }
        }
        if ping_ms > PING_AVERAGE_MAX_MS
            || outbound_loss_rate_pct.is_some_and(|loss| loss > LOSS_AVERAGE_MAX_PCT)
        {
            Self::Bad
        } else if ping_ms > PING_FINE_MAX_MS
            || outbound_loss_rate_pct.is_some_and(|loss| loss > LOSS_FINE_MAX_PCT)
        {
            Self::Average
        } else {
            Self::Fine
        }
    }

    /// Строчное имя для попапа: `"unknown"`, `"fine"`, `"average"`, `"bad"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Fine => "fine",
            Self::Average => "average",
            Self::Bad => "bad",
        }
    }
}

/// Одна точка буфера пингов.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingSample {
    /// Время замера, мс.
    pub time_ms: u64,
    /// Измеренная задержка, мс.
    pub value_ms: u64,
}

/// Ограниченный буфер пингов (`RTCConnection._pings`: 199 точек, среднее по 20).
#[derive(Debug, Clone, Default)]
pub struct PingHistory {
    samples: VecDeque<PingSample>,
    ping_bad_count: u64,
    timeout_seqs: Vec<u64>,
}

impl PingHistory {
    /// Пустая история.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Записать замер пинга; значения выше `500` мс увеличивают счётчик «плохих» пингов.
    pub fn push_ping(&mut self, time_ms: u64, value_ms: u64) {
        self.samples.push_back(PingSample { time_ms, value_ms });
        while self.samples.len() > MAX_PING_HISTORY_LEN {
            self.samples.pop_front();
        }
        if value_ms > PING_AVERAGE_MAX_MS {
            self.ping_bad_count += 1;
        }
    }

    /// Записать таймаут пинга.
    ///
    /// Возвращает `true` (и добавляет замер [`DEFAULT_PING_TIMEOUT_MS`]), когда
    /// последние три таймаута идут подряд.
    pub fn note_ping_timeout(&mut self, seq: u64, time_ms: u64) -> bool {
        self.timeout_seqs.push(seq);
        let len = self.timeout_seqs.len();
        if len < 3 {
            return false;
        }
        let (a, b, c) = (
            self.timeout_seqs[len - 3],
            self.timeout_seqs[len - 2],
            self.timeout_seqs[len - 1],
        );
        if b == a + 1 && c == b + 1 {
            self.push_ping(time_ms, DEFAULT_PING_TIMEOUT_MS);
            true
        } else {
            false
        }
    }

    /// Среднее по первым `min(len, 20)` точкам буфера, округлённое; `0` — пусто.
    #[must_use]
    pub fn average_ping_ms(&self) -> u64 {
        let count = self.samples.len().min(PING_AVERAGE_WINDOW);
        if count == 0 {
            return 0;
        }
        let sum: u64 = self.samples.iter().take(count).map(|s| s.value_ms).sum();
        (sum as f64 / count as f64).round() as u64
    }

    /// Последний замер пинга.
    #[must_use]
    pub fn last_ping_ms(&self) -> Option<u64> {
        self.samples.back().map(|sample| sample.value_ms)
    }

    /// Число замеров выше `500` мс.
    #[must_use]
    pub fn ping_bad_count(&self) -> u64 {
        self.ping_bad_count
    }

    /// Точки буфера, от старых к новым.
    #[must_use]
    pub fn samples(&self) -> &VecDeque<PingSample> {
        &self.samples
    }

    /// Сбросить точки, счётчик и историю таймаутов.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.ping_bad_count = 0;
        self.timeout_seqs.clear();
    }
}

/// Короткое имя RTC-сервера: текст до первой `.` или `:`.
#[must_use]
pub fn short_hostname(endpoint: &str) -> &str {
    let trimmed = endpoint.trim();
    let end = trimmed.find(['.', ':']).unwrap_or(trimmed.len());
    &trimmed[..end]
}

/// Агрегированные метрики RTC для попапа и журнала диагностики.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RtcConnectionStatsSnapshot {
    /// Короткое имя RTC-сервера.
    pub short_hostname: String,
    /// Последний пинг, мс.
    pub last_ping_ms: Option<u64>,
    /// Средний пинг по окну истории, мс.
    pub average_ping_ms: u64,
    /// Потеря исходящих пакетов, %.
    pub outbound_loss_rate_pct: Option<f64>,
    /// Классифицированное качество соединения.
    pub quality: ConnectionQuality,
    /// Отправлено RTP-пакетов (всего).
    pub packets_sent: u64,
    /// Потеряно RTP-пакетов (всего).
    pub packets_lost: i64,
    /// Отправлено байт полезной нагрузки (всего).
    pub bytes_sent: u64,
    /// Получено байт полезной нагрузки (всего).
    pub bytes_received: u64,
    /// Неудачных попыток расшифровки входящего потока.
    pub decryption_failures: u64,
    /// Версия протокола DAVE (Secure Frames).
    pub secure_frames_protocol_version: u16,
}

/// Секция модального окна `RTCDebugModal` (перечисление `zWA`, модуль `412780`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VoiceDebugModalSection {
    /// Транспорт: адрес, протокол, шифрование.
    Transport,
    /// Исходящий поток.
    Outbound,
    /// Входящий поток.
    Inbound,
    /// Демонстрация экрана.
    Screenshare,
    /// Камера.
    Camera,
    /// Клипы (Clip).
    Clips,
    /// Исходящие стримы.
    StreamsOutbound,
    /// Входящие стримы.
    StreamsInbound,
}

impl VoiceDebugModalSection {
    /// Имя секции в эталонном клиенте (`"TRANSPORT"`..`"STREAMS_INBOUND"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transport => "TRANSPORT",
            Self::Outbound => "OUTBOUND",
            Self::Inbound => "INBOUND",
            Self::Screenshare => "SCREENSHARE",
            Self::Camera => "CAMERA",
            Self::Clips => "CLIPS",
            Self::StreamsOutbound => "STREAMS_OUTBOUND",
            Self::StreamsInbound => "STREAMS_INBOUND",
        }
    }
}

/// Точка временного ряда `RTCDebugStore` (`{ value, time }`, окно 600 точек).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    /// Время замера, мс.
    pub time_ms: u64,
    /// Значение метрики.
    pub value: f64,
}

/// Точка спарклайна пинга (`Canvas` `258×80`) в контракте IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingSparklinePointDto {
    /// Время замера, мс.
    pub time_ms: u64,
    /// Задержка, мс.
    pub value_ms: u64,
}

/// DTO попапа `RTCConnectionPopout` (`camelCase` в JSON, модуль `978171`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDiagnosticsPopoutDto {
    /// Фаза соединения, например `"RTC_CONNECTED"` или `"DISCONNECTED"`.
    pub state: String,
    /// Качество соединения: `"fine"`, `"average"`, `"bad"` или `"unknown"`.
    pub quality: String,
    /// Короткое имя RTC-сервера.
    pub short_hostname: String,
    /// Средний пинг, мс.
    pub average_ping_ms: u64,
    /// Средний пинг как строка (эквивалент `toFixed(0)`).
    pub average_ping_formatted: String,
    /// Последний пинг, мс.
    pub last_ping_ms: Option<u64>,
    /// Последний пинг как строка (эквивалент `toFixed(0)`); `None`, если замеров нет.
    pub last_ping_formatted: Option<String>,
    /// Потеря исходящих пакетов, %.
    pub outbound_loss_rate_pct: Option<f64>,
    /// Потеря исходящих пакетов как строка (эквивалент `toFixed(1)`).
    pub outbound_loss_formatted: Option<String>,
    /// Точки спарклайна пинга.
    pub pings: Vec<PingSparklinePointDto>,
    /// Включено ли сквозное шифрование (DAVE / Secure Frames).
    pub is_end_to_end_encrypted: bool,
    /// Версия протокола Secure Frames (`0` — шифрование не согласовано).
    pub secure_frames_protocol_version: u32,
    /// Группы `Voice Privacy Code`: 6 строк по 5 цифр, пусто без шифрования.
    pub privacy_code_chunks: Vec<String>,
}

/// Форматирует `Voice Privacy Code` в 6 групп по 5 цифр.
///
/// Из `raw` берутся только ASCII-цифры; пробелы и дефисы — допустимые
/// разделители, любой другой символ (включая юникод-цифры) — ошибка. Длина
/// должна быть ровно [`PRIVACY_CODE_TOTAL_DIGITS`] цифр: обрезать или
/// дополнять код нельзя, иначе пользователь сверит не тот код.
pub fn format_voice_privacy_code(raw: &str) -> Result<[String; PRIVACY_CODE_GROUP_COUNT], String> {
    let mut digits = String::with_capacity(PRIVACY_CODE_TOTAL_DIGITS);
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !(ch.is_whitespace() || ch == '-') {
            return Err(format!(
                "Voice Privacy Code: недопустимый символ {ch:?} — ожидаются только цифры"
            ));
        }
    }
    if digits.len() != PRIVACY_CODE_TOTAL_DIGITS {
        return Err(format!(
            "Voice Privacy Code: ожидалось {PRIVACY_CODE_TOTAL_DIGITS} цифр, получено {}",
            digits.len()
        ));
    }
    Ok(std::array::from_fn(|group| {
        let start = group * PRIVACY_CODE_GROUP_LEN;
        digits[start..start + PRIVACY_CODE_GROUP_LEN].to_string()
    }))
}

/// Собирает DTO попапа `RTCConnectionPopout` из текущего состояния связи.
///
/// `pings` повторяют буфер `PingHistory` (до 199 точек, старые первыми).
/// Группы `Voice Privacy Code` заполняются только при
/// `secure_frames_protocol_version > 0`; отсутствующий или битый код даёт
/// пустой список, а не ошибку — попап обязан открыться и без кода.
#[must_use]
pub fn build_popout_dto(
    connected: bool,
    state_label: &str,
    endpoint: &str,
    ping_history: &PingHistory,
    outbound_loss_rate_pct: Option<f64>,
    secure_frames_protocol_version: u32,
    raw_privacy_code: Option<&str>,
) -> VoiceDiagnosticsPopoutDto {
    let last_ping_ms = ping_history.last_ping_ms();
    let average_ping_ms = ping_history.average_ping_ms();
    let quality = ConnectionQuality::evaluate(connected, last_ping_ms, outbound_loss_rate_pct);
    let is_end_to_end_encrypted = secure_frames_protocol_version > 0;
    let privacy_code_chunks = if is_end_to_end_encrypted {
        raw_privacy_code
            .and_then(|raw| format_voice_privacy_code(raw).ok())
            .map(|chunks| chunks.to_vec())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    VoiceDiagnosticsPopoutDto {
        state: state_label.to_string(),
        quality: quality.as_str().to_string(),
        short_hostname: short_hostname(endpoint).to_string(),
        average_ping_ms,
        average_ping_formatted: average_ping_ms.to_string(),
        last_ping_ms,
        last_ping_formatted: last_ping_ms.map(|value| value.to_string()),
        outbound_loss_rate_pct,
        outbound_loss_formatted: outbound_loss_rate_pct.map(|value| format!("{value:.1}")),
        pings: ping_history
            .samples()
            .iter()
            .map(|sample| PingSparklinePointDto {
                time_ms: sample.time_ms,
                value_ms: sample.value_ms,
            })
            .collect(),
        is_end_to_end_encrypted,
        secure_frames_protocol_version,
        privacy_code_chunks,
    }
}

/// Типизированный экспорт журнала: только эти поля и попадают в JSON.
#[derive(Debug, Serialize)]
struct SanitizedExport<'a> {
    series: BTreeMap<&'a str, &'a VecDeque<TimeSeriesPoint>>,
    snapshots: &'a VecDeque<RtcConnectionStatsSnapshot>,
}

/// Ключ ряда, который нельзя выгружать: в нём маркер секрета.
fn is_sensitive_metric_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    SENSITIVE_METRIC_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
}

/// История `RTCDebugStore` + `MediaEngineStatsStore` для модального окна отладки.
///
/// Оба буфера ограничены жёстко: 600 точек на ключ и 30 снимков. Вытеснение —
/// FIFO (`pop_front`), то есть в окне всегда последние замеры, а не первые.
#[derive(Debug, Clone, Default)]
pub struct VoiceDebugHistoryStore {
    series: BTreeMap<String, VecDeque<TimeSeriesPoint>>,
    snapshots: VecDeque<RtcConnectionStatsSnapshot>,
}

impl VoiceDebugHistoryStore {
    /// Пустая история.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавить точку метрики; при переполнении вытесняется старейшая точка ключа.
    pub fn push_metric_point(&mut self, key: &str, time_ms: u64, value: f64) {
        let series = self.series.entry(key.to_string()).or_default();
        series.push_back(TimeSeriesPoint { time_ms, value });
        while series.len() > MAX_DEBUG_SERIES_POINTS {
            series.pop_front();
        }
    }

    /// Добавить снимок RTC-метрик; при переполнении вытесняется старейший снимок.
    pub fn push_snapshot(&mut self, snapshot: RtcConnectionStatsSnapshot) {
        self.snapshots.push_back(snapshot);
        while self.snapshots.len() > MAX_MEDIA_ENGINE_SNAPSHOTS {
            self.snapshots.pop_front();
        }
    }

    /// Ряд метрики, если он есть.
    #[must_use]
    pub fn series(&self, key: &str) -> Option<&VecDeque<TimeSeriesPoint>> {
        self.series.get(key)
    }

    /// Снимки, от старых к новым.
    #[must_use]
    pub fn snapshots(&self) -> &VecDeque<RtcConnectionStatsSnapshot> {
        &self.snapshots
    }

    /// Сбросить историю (реконнект, закрытие попапа).
    pub fn clear(&mut self) {
        self.series.clear();
        self.snapshots.clear();
    }

    /// Экспорт журнала диагностики в JSON.
    ///
    /// В вывод попадают только типизированные поля `series`/`snapshots`; ряды,
    /// в имени которых есть `secret_key`, `session_token` или `token`,
    /// отбрасываются целиком — поэтому ключи сессии и секреты не утекают даже
    /// если метрику случайно назвали «по-секретному».
    ///
    /// # Errors
    ///
    /// Возвращает текст ошибки, если сериализация JSON не удалась.
    pub fn export_sanitized_json(&self) -> Result<String, String> {
        let series: BTreeMap<&str, &VecDeque<TimeSeriesPoint>> = self
            .series
            .iter()
            .filter(|(key, _)| !is_sensitive_metric_key(key))
            .map(|(key, points)| (key.as_str(), points))
            .collect();
        let payload = SanitizedExport {
            series,
            snapshots: &self.snapshots,
        };
        serde_json::to_string(&payload)
            .map_err(|err| format!("журнал диагностики не сериализуется: {err}"))
    }
}
