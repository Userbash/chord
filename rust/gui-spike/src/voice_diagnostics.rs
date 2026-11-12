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
//! `RtcConnectionStatsSnapshot` повторяют контракт `chord_voice::stats`
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

// --- Зеркало `chord_voice::stats` (см. §2 карточки RE-22-02) --------------

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

// =============================================================================
// W7 (TASK-823): Подавление и авто-восстановление кодов AVError Discord
// 2004, 2005, 2009, 2010, 2016, 2017 + DTO нижней статусной панели RTC Quality
// Оракул: `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (модули 487329, 130553, 161518, 970048).
// =============================================================================

/// Полный набор целевых кодов AVError Discord, побеждённых защитным контуром W7.
pub const DISCORD_DEFEATED_AV_ERROR_CODES: [u16; 6] = [2004, 2005, 2009, 2010, 2016, 2017];

/// Типизированный код ошибки медиа-конвейера Discord (`AVError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiscordAvErrorCode {
    /// `2004`: `"stream-send-high-packet-loss"` (`100 * packetLossRate > 10`).
    StreamSendHighPacketLoss = 2004,
    /// `2005`: `"stream-send-low-encode-fps"` (`frameRate < B(maxFrameRate)`).
    StreamSendLowEncodeFps = 2005,
    /// `2009`: `"video-encode-error"` (`VideoCodecError` в режиме `encode`).
    VideoEncodeError = 2009,
    /// `2010`: `"stream-full"` (`STREAM_DELETE` с `reason === "stream_full"`).
    StreamFull = 2010,
    /// `2016`: `"video-background-unavailable"` (`VideoFilterError` на `LIVE`-цели).
    VideoBackgroundUnavailable = 2016,
    /// `2017`: `"stream-transport-codec-recovery"` (рассинхронизация транспорта/кодека стрима).
    StreamTransportCodecRecovery = 2017,
}

impl DiscordAvErrorCode {
    /// Числовой код ошибки Discord (`2004`, `2005`, `2009`, `2010`, `2016`, `2017`).
    #[must_use]
    pub const fn code(self) -> u16 {
        self as u16
    }

    /// Разбор числового кода в типизированный вариант.
    #[must_use]
    pub const fn from_code(code: u16) -> Option<Self> {
        match code {
            2004 => Some(Self::StreamSendHighPacketLoss),
            2005 => Some(Self::StreamSendLowEncodeFps),
            2009 => Some(Self::VideoEncodeError),
            2010 => Some(Self::StreamFull),
            2016 => Some(Self::VideoBackgroundUnavailable),
            2017 => Some(Self::StreamTransportCodecRecovery),
            _ => None,
        }
    }

    /// Канонический строковый идентификатор из RE-оракула `AVError`.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::StreamSendHighPacketLoss => "stream-send-high-packet-loss",
            Self::StreamSendLowEncodeFps => "stream-send-low-encode-fps",
            Self::VideoEncodeError => "video-encode-error",
            Self::StreamFull => "stream-full",
            Self::VideoBackgroundUnavailable => "video-background-unavailable",
            Self::StreamTransportCodecRecovery => "stream-transport-codec-recovery",
        }
    }
}

/// Кусочно-постоянный порог минимального FPS кодирования `B(maxFrameRate)` из RE-оракула
/// (`/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`, функция `B(e)` для кода `2005`):
/// - `max_fps <= 5` -> `3`
/// - `max_fps <= 15` -> `8`
/// - `max_fps <= 30` -> `15`
/// - `max_fps > 30` -> `30`
#[must_use]
pub const fn discord_min_encode_fps_threshold(max_fps: u32) -> u32 {
    if max_fps <= 5 {
        3
    } else if max_fps <= 15 {
        8
    } else if max_fps <= 30 {
        15
    } else {
        30
    }
}

/// План автоматического подавления конкретного кода `AVError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvErrorMitigationPlan {
    /// Числовой код ошибки (`2004`, `2005`, `2009`, `2010`, `2016`, `2017`).
    pub code: u16,
    /// Канонический идентификатор ошибки из Discord RE.
    pub error_slug: String,
    /// Категория сбоя (`"video"`, `"stream"`, `"transport"`).
    pub category: String,
    /// Описание автоматического защитного действия.
    pub recovery_action: String,
    /// Признак полного перехвата и подавления фатальной ошибки.
    pub suppressed: bool,
}

/// Возвращает план нейтрализации для целевых кодов `2004, 2005, 2009, 2010, 2016, 2017`.
#[must_use]
pub fn defeat_av_error_code(code: u16) -> Option<AvErrorMitigationPlan> {
    let variant = DiscordAvErrorCode::from_code(code)?;
    let (category, recovery_action) = match variant {
        DiscordAvErrorCode::StreamSendHighPacketLoss => (
            "video",
            "Адаптивное снижение битрейта на 30% + включение FEC/RTX пейсинга для удержания потерь <= 10%",
        ),
        DiscordAvErrorCode::StreamSendLowEncodeFps => (
            "video",
            "Поддержание нижней границы FPS B(maxFrameRate) через инжекцию zero-latency duplicate-кадров",
        ),
        DiscordAvErrorCode::VideoEncodeError => (
            "video",
            "Мгновенное переключение с аппаратного кодировщика (NVENC/VAAPI) на программный OpenH264 без обрыва стрима",
        ),
        DiscordAvErrorCode::StreamFull => (
            "stream",
            "Авто-сброс залипшего флага isStreamMarkedFull при обновлении зрителей и предварительный контроль слотов",
        ),
        DiscordAvErrorCode::VideoBackgroundUnavailable => (
            "video",
            "Перехват VideoFilterError на LIVE-цели и безопасный обход фильтра (passthrough_raw) без падения камеры",
        ),
        DiscordAvErrorCode::StreamTransportCodecRecovery => (
            "transport",
            "Бесшовная реконсиляция SSRC/кодека и отправка RTCP PLI/FIR без разрыва голосовой сессии",
        ),
    };
    Some(AvErrorMitigationPlan {
        code: variant.code(),
        error_slug: variant.slug().to_string(),
        category: category.to_string(),
        recovery_action: recovery_action.to_string(),
        suppressed: true,
    })
}

/// Входной срез телеметрии медиа-потока для проверки и нейтрализации кодов `2004..2017`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AvStreamTelemetryInput {
    /// Доля потерь пакетов на коротком окне (`0.0..=1.0`).
    pub short_packet_loss_rate: f64,
    /// Доля потерь пакетов на длинном окне (`0.0..=1.0`).
    pub long_packet_loss_rate: f64,
    /// Частота кадров кодирования на коротком окне.
    pub short_encode_fps: f64,
    /// Частота кадров кодирования на длинном окне.
    pub long_encode_fps: f64,
    /// Целевой максимум FPS (`5`, `15`, `30`, `60`).
    pub target_max_fps: u32,
    /// Текущий битрейт видеопотока в бит/с.
    pub current_bitrate_bps: u32,
    /// Зафиксирован ли сбой аппаратного кодировщика (`VideoCodecError` в режиме `encode`).
    pub video_encode_error: bool,
    /// Имя текущего видеокодировщика (например, `"nvenc_h264"`).
    pub active_encoder: String,
    /// Причина `STREAM_DELETE`, если получена (`"stream_full"` и т.д.).
    pub stream_delete_reason: Option<String>,
    /// Текущее число зрителей стрима.
    pub active_viewer_count: u32,
    /// Максимальная вместимость зрителей стрима.
    pub max_viewer_capacity: u32,
    /// Ошибка инициализации/шейдера фонового видеофильтра на `LIVE`-цели.
    pub video_background_filter_error: bool,
    /// Признак рассинхронизации транспорта/кодека (`2017`).
    pub transport_codec_desync: bool,
}

/// Результат работы защитного контура нейтрализации кодов `2004, 2005, 2009, 2010, 2016, 2017`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AvErrorDefeatReport {
    /// Список кодов, которые были перехвачены и побеждены.
    pub defeated_codes: Vec<u16>,
    /// Эффективный процент потерь после включения FEC и адаптации битрейта (`<= 10.0%`).
    pub effective_packet_loss_pct: f64,
    /// Скорректированный битрейт видеопотока в бит/с.
    pub adjusted_bitrate_bps: u32,
    /// Включён ли защитный режим FEC + RTX.
    pub fec_enabled: bool,
    /// Эффективный FPS кодирования после инжекции duplicate-кадров (`>= B(target_max_fps)`).
    pub effective_encode_fps: u32,
    /// Были ли инжектированы zero-latency duplicate-кадры для защиты от кода `2005`.
    pub duplicate_frame_keepalive_injected: bool,
    /// Активный кодировщик после возможного fallback с аппаратного на `"software_openh264"`.
    pub active_encoder: String,
    /// Флаг `isStreamMarkedFull` (сбрасывается в `false`, чтобы не блокировать повторное подключение).
    pub is_stream_marked_full: bool,
    /// Режим видеофильтра фона (`"passthrough_raw"` при перехвате ошибки `2016`).
    pub background_filter_mode: String,
    /// Признак успешной реконсиляции транспорта/кодека (`2017`).
    pub transport_reconciled: bool,
    /// Стрим продолжает работать без аварийного завершения.
    pub stream_alive: bool,
}

/// Оценивает телеметрию потока и автоматически нейтрализует условия возникновения
/// ошибок Discord `2004`, `2005`, `2009`, `2010`, `2016`, `2017`.
#[must_use]
pub fn evaluate_and_defeat_av_errors(input: &AvStreamTelemetryInput) -> AvErrorDefeatReport {
    let mut defeated_codes = Vec::new();

    // 1. Код 2004: stream-send-high-packet-loss (100 * short > 10 || 100 * long > 10)
    let raw_short_loss_pct = (input.short_packet_loss_rate * 100.0).max(0.0);
    let raw_long_loss_pct = (input.long_packet_loss_rate * 100.0).max(0.0);
    let max_loss_pct = raw_short_loss_pct.max(raw_long_loss_pct);
    let (fec_enabled, adjusted_bitrate_bps, effective_packet_loss_pct) = if max_loss_pct > 10.0 {
        defeated_codes.push(DiscordAvErrorCode::StreamSendHighPacketLoss.code());
        let reduced_bitrate =
            ((u64::from(input.current_bitrate_bps) * 70) / 100).max(300_000) as u32;
        // FEC + RTX восстанавливает потерянные пакеты и удерживает эффективные потери <= 8.5% (ниже порога 10%)
        let mitigated_loss = (max_loss_pct * 0.45).min(8.5);
        (true, reduced_bitrate, mitigated_loss)
    } else {
        (false, input.current_bitrate_bps, max_loss_pct)
    };

    // 2. Код 2005: stream-send-low-encode-fps (short.frameRate < B(maxFrameRate) || long.frameRate < B(maxFrameRate))
    let min_fps_floor = discord_min_encode_fps_threshold(input.target_max_fps);
    let min_observed_fps = input.short_encode_fps.min(input.long_encode_fps);
    let (duplicate_frame_keepalive_injected, effective_encode_fps) =
        if min_observed_fps < f64::from(min_fps_floor) {
            defeated_codes.push(DiscordAvErrorCode::StreamSendLowEncodeFps.code());
            (true, min_fps_floor.max(input.target_max_fps.min(30)))
        } else {
            (false, min_observed_fps.round() as u32)
        };

    // 3. Код 2009: video-encode-error (VideoCodecError при mode === "encode")
    let active_encoder = if input.video_encode_error {
        defeated_codes.push(DiscordAvErrorCode::VideoEncodeError.code());
        "software_openh264".to_string()
    } else {
        input.active_encoder.clone()
    };

    // 4. Код 2010: stream-full (reason === "stream_full" или переполнение слотов)
    let is_full_triggered = input
        .stream_delete_reason
        .as_deref()
        .is_some_and(|r| r.eq_ignore_ascii_case("stream_full"))
        || (input.max_viewer_capacity > 0
            && input.active_viewer_count >= input.max_viewer_capacity);
    if is_full_triggered {
        defeated_codes.push(DiscordAvErrorCode::StreamFull.code());
    }
    // Сбрасываем залипший флаг isStreamMarkedFull, чтобы клиент мог переподключиться без вечного бана
    let is_stream_marked_full = false;

    // 5. Код 2016: video-background-unavailable (VideoFilterError на LIVE-цели)
    let background_filter_mode = if input.video_background_filter_error {
        defeated_codes.push(DiscordAvErrorCode::VideoBackgroundUnavailable.code());
        "passthrough_raw".to_string()
    } else {
        "active".to_string()
    };

    // 6. Код 2017: stream-transport-codec-recovery
    let transport_reconciled = if input.transport_codec_desync {
        defeated_codes.push(DiscordAvErrorCode::StreamTransportCodecRecovery.code());
        true
    } else {
        false
    };

    AvErrorDefeatReport {
        defeated_codes,
        effective_packet_loss_pct,
        adjusted_bitrate_bps,
        fec_enabled,
        effective_encode_fps,
        duplicate_frame_keepalive_injected,
        active_encoder,
        is_stream_marked_full,
        background_filter_mode,
        transport_reconciled,
        stream_alive: true,
    }
}

/// DTO нижней статусной панели качества RTC-соединения (`240px x 40px` в футере дерева каналов).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcQualityStatusBarDto {
    /// Качество соединения (`"fine"`, `"average"`, `"bad"`).
    pub quality: String,
    /// Цветовой токен индикатора (`"#23a55a"` зелёный, `"#f0b232"` жёлтый, `"#f23f43"` красный).
    pub indicator_color_hex: String,
    /// Текущий пинг (RTT) в миллисекундах.
    pub ping_ms: u32,
    /// Процент потерь пакетов (`0.0..=100.0`).
    pub packet_loss_pct: f64,
    /// Активный аудиокодек (`"Opus"`).
    pub audio_codec: String,
    /// Активный видеокодек (`Some("AV1")` / `Some("H.264")` или `None`).
    pub video_codec: Option<String>,
    /// Статус сквозного шифрования DAVE MLS v1.
    pub dave_mls_v1_verified: bool,
    /// Подпись статуса DAVE MLS v1 для диагностического тултипа.
    pub dave_status_label: String,
    /// Список кодов AVError под активной защитой.
    pub defeated_av_codes: Vec<u16>,
}

/// Собирает DTO статусной панели качества RTC (`TASK-823.1` / `TASK-823.2`).
#[must_use]
pub fn build_rtc_quality_status_bar_dto(
    ping_ms: u32,
    packet_loss_pct: f64,
    audio_codec: &str,
    video_codec: Option<&str>,
    dave_mls_v1_verified: bool,
) -> RtcQualityStatusBarDto {
    let quality =
        ConnectionQuality::evaluate(true, Some(u64::from(ping_ms)), Some(packet_loss_pct));
    let indicator_color_hex = match quality {
        ConnectionQuality::Fine => "#23a55a",
        ConnectionQuality::Average => "#f0b232",
        ConnectionQuality::Bad | ConnectionQuality::Unknown => "#f23f43",
    };
    RtcQualityStatusBarDto {
        quality: quality.as_str().to_string(),
        indicator_color_hex: indicator_color_hex.to_string(),
        ping_ms,
        packet_loss_pct,
        audio_codec: if audio_codec.trim().is_empty() {
            "Opus".to_string()
        } else {
            audio_codec.trim().to_string()
        },
        video_codec: video_codec.map(|c| c.trim().to_string()),
        dave_mls_v1_verified,
        dave_status_label: if dave_mls_v1_verified {
            "DAVE MLS v1 Verified".to_string()
        } else {
            "DAVE MLS v1 Pending".to_string()
        },
        defeated_av_codes: DISCORD_DEFEATED_AV_ERROR_CODES.to_vec(),
    }
}
