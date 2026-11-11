//! RTP reception statistics (RFC 3550 §6.4.1 / Appendix A.3).
//!
//! In Discord voice `*_rtpsize` mode the fixed RTP header is transmitted in the clear,
//! so sequence numbers, timestamps and the SSRC are observable without keys. This
//! module turns a stream of such headers into loss/jitter counters. It does not
//! decrypt or decode media.
//!
//! Provenance and the 8-packet capture oracle: `waves/RW-26/research/RE-VOICE-STATS.md`.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// RTP clock rate for Opus (48 kHz).
pub const OPUS_CLOCK_RATE: u32 = 48_000;

/// Число моно-сэмплов в одном 20 мс кадре при 48 кГц (`Serein/crates/discord-voice/src/lib.rs:L23`).
pub const PCM_FRAME_SAMPLES: usize = 960;

/// Минимальный множитель громкости микшера (`0.0` = 0%).
pub const MIN_MIXER_VOLUME: f32 = 0.0;

/// Стандартный множитель громкости микшера (`1.0` = 100%).
pub const DEFAULT_MIXER_VOLUME: f32 = 1.0;

/// Максимальный множитель громкости микшера (`2.0` = 200%).
pub const MAX_MIXER_VOLUME: f32 = 2.0;

/// Максимальное число одновременно микшируемых участников (`Serein/crates/discord-voice/src/mixer.rs:L93`).
pub const MAX_MIXER_PARTICIPANTS: usize = 64;

/// Один 20 мс моно PCM-кадр при 48 кГц.
pub type PcmFrame = [f32; PCM_FRAME_SAMPLES];

/// Threshold used to detect 16-bit sequence wrap (RFC 3550 A.1).
const SEQ_HALF: i32 = 32_768;

/// Minimum fixed RTP header length.
const FIXED_HEADER_LEN: usize = 12;

/// Fields of the fixed RTP header that matter for reception statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpHeader {
    /// Payload type (7 bits).
    pub payload_type: u8,
    /// Marker bit.
    pub marker: bool,
    /// 16-bit sequence number.
    pub sequence: u16,
    /// 32-bit RTP timestamp.
    pub timestamp: u32,
    /// Synchronization source.
    pub ssrc: u32,
}

/// How a packet relates to the stream seen so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// Packet extends the highest sequence seen so far.
    InOrder,
    /// Packet is older than the highest sequence but was not seen before.
    Reordered,
    /// Exact extended sequence was already observed.
    Duplicate,
}

/// Point-in-time view of reception counters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReceptionSnapshot {
    /// Unique packets received.
    pub received: u64,
    /// Packets expected from the first to the highest extended sequence.
    pub expected: u64,
    /// `expected - received`, saturating.
    pub lost: u64,
    /// Duplicate packets observed.
    pub duplicates: u64,
    /// Low 16 bits of the highest extended sequence.
    pub highest_sequence: Option<u16>,
    /// RFC 3550 interarrival jitter in RTP timestamp units.
    pub jitter_units: f64,
    /// The same jitter converted to milliseconds.
    pub jitter_ms: f64,
}

/// Accumulates reception statistics for one RTP stream.
#[derive(Debug, Clone)]
pub struct ReceptionStats {
    clock_rate: u32,
    base_ext: Option<u32>,
    highest_ext: Option<u32>,
    received: u64,
    duplicates: u64,
    expected_prior: u64,
    received_prior: u64,
    seen: BTreeSet<u32>,
    base_arrival_ms: Option<f64>,
    prev_arrival_units: Option<f64>,
    prev_timestamp: Option<u32>,
    jitter: f64,
}

impl ReceptionStats {
    /// Create counters for a stream with the given RTP clock rate.
    ///
    /// A zero clock rate disables jitter accumulation (counters still work).
    #[must_use]
    pub fn new(clock_rate: u32) -> Self {
        Self {
            clock_rate,
            base_ext: None,
            highest_ext: None,
            received: 0,
            duplicates: 0,
            expected_prior: 0,
            received_prior: 0,
            seen: BTreeSet::new(),
            base_arrival_ms: None,
            prev_arrival_units: None,
            prev_timestamp: None,
            jitter: 0.0,
        }
    }

    /// Record one received packet.
    pub fn observe(&mut self, header: &RtpHeader, arrival_ms: f64) -> Observation {
        let arrival_units = match self.base_arrival_ms {
            Some(base) => {
                let scale = if self.clock_rate == 0 {
                    1.0
                } else {
                    f64::from(self.clock_rate) / 1000.0
                };
                (arrival_ms - base) * scale
            }
            None => {
                self.base_arrival_ms = Some(arrival_ms);
                0.0
            }
        };

        let extended = match self.highest_ext {
            None => {
                let extended = u32::from(header.sequence);
                self.base_ext = Some(extended);
                extended
            }
            Some(highest) => extend_sequence(header.sequence, highest),
        };

        if !self.seen.insert(extended) {
            self.duplicates += 1;
            return Observation::Duplicate;
        }
        self.received += 1;
        let reordered = self.highest_ext.is_some_and(|highest| extended < highest);
        if self.highest_ext.is_none_or(|highest| extended > highest) {
            self.highest_ext = Some(extended);
        }

        if self.clock_rate != 0 {
            if let (Some(previous_units), Some(previous_timestamp)) =
                (self.prev_arrival_units, self.prev_timestamp)
            {
                let transit = arrival_units - previous_units;
                let timestamp_delta =
                    f64::from(header.timestamp.wrapping_sub(previous_timestamp) as i32);
                let delta = transit - timestamp_delta;
                self.jitter += (delta.abs() - self.jitter) / 16.0;
            }
            self.prev_arrival_units = Some(arrival_units);
            self.prev_timestamp = Some(header.timestamp);
        }

        if reordered {
            Observation::Reordered
        } else {
            Observation::InOrder
        }
    }

    /// Current counters.
    #[must_use]
    pub fn snapshot(&self) -> ReceptionSnapshot {
        let (expected, highest_sequence) = match (self.base_ext, self.highest_ext) {
            (Some(base), Some(highest)) => (
                u64::from(highest.saturating_sub(base)) + 1,
                Some(highest as u16),
            ),
            _ => (0, None),
        };
        let jitter_ms = if self.clock_rate == 0 {
            0.0
        } else {
            self.jitter / f64::from(self.clock_rate) * 1000.0
        };
        ReceptionSnapshot {
            received: self.received,
            expected,
            lost: expected.saturating_sub(self.received),
            duplicates: self.duplicates,
            highest_sequence,
            jitter_units: self.jitter,
            jitter_ms,
        }
    }

    /// Кумулятивная доля потерь пакетов (`0.0..=1.0`).
    #[must_use]
    pub fn loss_rate(&self) -> f64 {
        let snap = self.snapshot();
        if snap.expected == 0 {
            0.0
        } else {
            (snap.lost as f64) / (snap.expected as f64)
        }
    }

    /// Интервальная доля потерь пакетов (`0.0..=1.0`) по RFC 3550 Appendix A.3 с момента последнего Receiver Report.
    #[must_use]
    pub fn interval_loss_rate(&self) -> f64 {
        let snap = self.snapshot();
        let expected_interval = snap.expected.saturating_sub(self.expected_prior);
        let received_interval = self.received.saturating_sub(self.received_prior);
        if expected_interval == 0 || received_interval >= expected_interval {
            0.0
        } else {
            ((expected_interval - received_interval) as f64) / (expected_interval as f64)
        }
    }

    /// Проверяет, превышает ли текущая интервальная доля потерь порог Discord WebRTC Error 2003 (`> 10%`, т.е. `packetLossRate * 100 > 10`).
    #[must_use]
    pub fn is_high_packet_loss(&self) -> bool {
        self.interval_loss_rate() * 100.0 > crate::rtcp::DISCORD_HIGH_PACKET_LOSS_THRESHOLD_PCT
    }

    /// Вычисляет 24-байтный `ReportBlock` по RFC 3550 Appendix A.3 без сдвига интервальных счётчиков `expected_prior` / `received_prior`.
    #[must_use]
    pub fn peek_report_block(
        &self,
        ssrc: u32,
        last_sr: u32,
        delay_since_last_sr: u32,
    ) -> crate::rtcp::ReportBlock {
        let snap = self.snapshot();
        let expected_interval = snap.expected.saturating_sub(self.expected_prior);
        let received_interval = self.received.saturating_sub(self.received_prior);
        let fraction_lost = if expected_interval == 0 || received_interval >= expected_interval {
            0u8
        } else {
            let lost_interval = expected_interval - received_interval;
            ((lost_interval * 256) / expected_interval).min(255) as u8
        };
        let cumulative_lost_i64 = i64::try_from(snap.lost).unwrap_or(i64::MAX);
        let cumulative_lost = cumulative_lost_i64.clamp(
            i64::from(crate::rtcp::CUMULATIVE_LOST_MIN),
            i64::from(crate::rtcp::CUMULATIVE_LOST_MAX),
        ) as i32;
        crate::rtcp::ReportBlock {
            ssrc,
            fraction_lost,
            cumulative_lost,
            extended_highest_seq: self.highest_ext.unwrap_or(0),
            jitter: self.jitter.round() as u32,
            last_sr,
            delay_since_last_sr,
        }
    }

    /// Вычисляет `ReportBlock` по RFC 3550 Appendix A.3 и фиксирует границу интервала (`expected_prior`, `received_prior`).
    pub fn next_report_block(
        &mut self,
        ssrc: u32,
        last_sr: u32,
        delay_since_last_sr: u32,
    ) -> crate::rtcp::ReportBlock {
        let rb = self.peek_report_block(ssrc, last_sr, delay_since_last_sr);
        let snap = self.snapshot();
        self.expected_prior = snap.expected;
        self.received_prior = self.received;
        rb
    }

    /// Собирает готовый 32-байтный открытый пакет `RTCP Receiver Report` (`PT = 201, RC = 1`) по RFC 3550 §6.4.2 и Appendix A.3.
    pub fn build_receiver_report(
        &mut self,
        sender_ssrc: u32,
        media_ssrc: u32,
        last_sr: u32,
        delay_since_last_sr: u32,
    ) -> [u8; crate::rtcp::RTCP_RR_CLEAR_LEN] {
        let rb = self.next_report_block(media_ssrc, last_sr, delay_since_last_sr);
        crate::rtcp::build_receiver_report(sender_ssrc, &rb)
    }
}

/// Parse the fixed part of an RTP header.
///
/// Returns `None` for non-RTP data (version != 2) and for inputs shorter than the
/// fixed 12-byte header. Header extensions are allowed and deliberately ignored.
#[must_use]
pub fn parse_rtp_header(bytes: &[u8]) -> Option<RtpHeader> {
    if bytes.len() < FIXED_HEADER_LEN || bytes[0] >> 6 != 2 {
        return None;
    }
    Some(RtpHeader {
        payload_type: bytes[1] & 0x7f,
        marker: bytes[1] & 0x80 != 0,
        sequence: u16::from_be_bytes([bytes[2], bytes[3]]),
        timestamp: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        ssrc: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
    })
}

/// Extend a 16-bit sequence number relative to the highest extended value.
fn extend_sequence(sequence: u16, highest: u32) -> u32 {
    let cycle = highest & 0xffff_0000;
    let highest_sequence = (highest & 0xffff) as i32;
    let difference = i32::from(sequence) - highest_sequence;
    let extended = if difference > SEQ_HALF {
        cycle.wrapping_add(u32::from(sequence)).wrapping_sub(65_536)
    } else if difference < -SEQ_HALF {
        cycle.wrapping_add(u32::from(sequence)).wrapping_add(65_536)
    } else {
        cycle | u32::from(sequence)
    };
    // A packet far "ahead" can only be an artefact of wrapping below the stream
    // base; clamp it to the highest value so it is treated as a duplicate rather
    // than creating a huge phantom gap.
    if extended > highest.saturating_add(SEQ_HALF as u32) {
        highest
    } else {
        extended
    }
}

/// Мягкий лимитер `tanh` для предотвращения жёсткого клиппинга при одновременном разговоре
/// нескольких участников и воспроизведении звука демонстрации экрана (`P1.4`).
#[must_use]
pub fn soft_limit_tanh(sample: f32) -> f32 {
    if sample.is_nan() {
        0.0
    } else {
        sample.tanh().clamp(-1.0, 1.0)
    }
}

fn clamp_volume(volume: f32) -> f32 {
    if volume.is_nan() {
        DEFAULT_MIXER_VOLUME
    } else {
        volume.clamp(MIN_MIXER_VOLUME, MAX_MIXER_VOLUME)
    }
}

/// Раздельный микшер громкости участников (`HashMap<u64, f32>`) и независимого регулятора
/// громкости демонстрации экрана `stream_volume` (`0.0..=2.0`) с мягким лимитером `tanh`
/// (`P1.4`, эталон `Serein/crates/discord-voice/src/mixer.rs:95-165`).
#[derive(Debug, Clone)]
pub struct StreamVoiceMixer {
    participant_volumes: HashMap<u64, f32>,
    stream_volume: f32,
    ssrc_to_user: HashMap<u32, u64>,
    participant_buffers: BTreeMap<u64, VecDeque<f32>>,
    stream_buffer: VecDeque<f32>,
    active_speakers: BTreeSet<u64>,
}

/// Псевдоним типа микшера по эталону `Serein`.
pub type Mixer = StreamVoiceMixer;

impl Default for StreamVoiceMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamVoiceMixer {
    /// Создаёт новый микшер с пустой картой индивидуальных громкостей и `stream_volume = 1.0`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            participant_volumes: HashMap::new(),
            stream_volume: DEFAULT_MIXER_VOLUME,
            ssrc_to_user: HashMap::new(),
            participant_buffers: BTreeMap::new(),
            stream_buffer: VecDeque::new(),
            active_speakers: BTreeSet::new(),
        }
    }

    /// Карта индивидуальных громкостей участников (`user_id -> 0.0..=2.0`).
    #[must_use]
    pub fn participant_volumes(&self) -> &HashMap<u64, f32> {
        &self.participant_volumes
    }

    /// Возвращает текущий множитель громкости участника (`0.0..=2.0`, по умолчанию `1.0`).
    #[must_use]
    pub fn participant_volume(&self, user_id: u64) -> f32 {
        self.participant_volumes
            .get(&user_id)
            .copied()
            .unwrap_or(DEFAULT_MIXER_VOLUME)
    }

    /// Устанавливает индивидуальную громкость участника с клампом в `0.0..=2.0`.
    /// Не затрагивает `stream_volume`.
    pub fn set_participant_volume(&mut self, user_id: u64, volume: f32) -> f32 {
        let clamped = clamp_volume(volume);
        self.participant_volumes.insert(user_id, clamped);
        clamped
    }

    /// Устанавливает индивидуальную громкость участника в процентах (`0..=200%`).
    pub fn set_participant_volume_percent(&mut self, user_id: u64, percent: u16) -> f32 {
        let multiplier = f32::from(percent.min(200)) / 100.0;
        self.set_participant_volume(user_id, multiplier)
    }

    /// Независимая громкость звука демонстрации экрана (`0.0..=2.0`).
    #[must_use]
    pub fn stream_volume(&self) -> f32 {
        self.stream_volume
    }

    /// Устанавливает независимую громкость демонстрации экрана с клампом в `0.0..=2.0`.
    /// Не затрагивает громкости участников.
    pub fn set_stream_volume(&mut self, volume: f32) -> f32 {
        let clamped = clamp_volume(volume);
        self.stream_volume = clamped;
        clamped
    }

    /// Устанавливает громкость демонстрации экрана в процентах (`0..=200%`).
    pub fn set_stream_volume_percent(&mut self, percent: u16) -> f32 {
        let multiplier = f32::from(percent.min(200)) / 100.0;
        self.set_stream_volume(multiplier)
    }

    /// Синхронизирует карту громкостей участников при перерисовке UI без сброса `stream_volume`.
    pub fn sync_ui_participant_volumes(&mut self, volumes: &HashMap<u64, f32>) {
        self.participant_volumes.clear();
        for (&user_id, &vol) in volumes.iter().take(MAX_MIXER_PARTICIPANTS) {
            self.participant_volumes.insert(user_id, clamp_volume(vol));
        }
    }

    /// Регистрирует привязку SSRC к `user_id` (с защитой от коллизии SSRC между разными участниками,
    /// по эталону `Serein/crates/discord-voice/src/mixer.rs:20-50`).
    pub fn announce(&mut self, user_id: u64, ssrc: u32) -> Result<(), &'static str> {
        if let Some(&existing_user) = self.ssrc_to_user.get(&ssrc)
            && existing_user != user_id
        {
            return Err("Voice SSRC belongs to another participant");
        }
        if self.ssrc_to_user.get(&ssrc) == Some(&user_id)
            && self.participant_buffers.contains_key(&user_id)
        {
            return Ok(());
        }
        self.ssrc_to_user.retain(|_, &mut u| u != user_id);
        if !self.participant_buffers.contains_key(&user_id)
            && self.participant_buffers.len() >= MAX_MIXER_PARTICIPANTS
        {
            return Err("Voice decoder budget exceeded");
        }
        self.ssrc_to_user.insert(ssrc, user_id);
        self.participant_buffers.entry(user_id).or_default();
        Ok(())
    }

    /// Удаляет участника и освобождает его буферы (сохраняя индивидуальные настройки громкости и `stream_volume`).
    pub fn remove_participant(&mut self, user_id: u64) {
        self.ssrc_to_user.retain(|_, &mut u| u != user_id);
        self.participant_buffers.remove(&user_id);
        self.active_speakers.remove(&user_id);
    }

    /// Находит `user_id` по `ssrc`.
    #[must_use]
    pub fn user_for_ssrc(&self, ssrc: u32) -> Option<u64> {
        self.ssrc_to_user.get(&ssrc).copied()
    }

    /// Добавляет декодированные PCM-сэмплы участника в очередь микширования.
    pub fn push_participant_pcm(&mut self, user_id: u64, samples: &[f32]) {
        if !self.participant_buffers.contains_key(&user_id)
            && self.participant_buffers.len() >= MAX_MIXER_PARTICIPANTS
        {
            return;
        }
        let buf = self.participant_buffers.entry(user_id).or_default();
        let max_buffered = PCM_FRAME_SAMPLES * 6;
        if buf.len().saturating_add(samples.len()) > max_buffered {
            let drop_count = (buf.len() + samples.len()).saturating_sub(max_buffered);
            buf.drain(..drop_count.min(buf.len()));
        }
        buf.extend(samples.iter().copied());
    }

    /// Добавляет декодированные PCM-сэмплы по `ssrc` (если `ssrc` зарегистрирован за участником).
    pub fn push_ssrc_pcm(&mut self, ssrc: u32, samples: &[f32]) -> bool {
        let Some(user_id) = self.user_for_ssrc(ssrc) else {
            return false;
        };
        self.push_participant_pcm(user_id, samples);
        true
    }

    /// Добавляет декодированные PCM-сэмплы потока демонстрации экрана (Go Live).
    /// По эталону `Serein` (`stream_playout.rs`) удерживает не более 2 кадров (40 мс), чтобы не накапливать задержку.
    pub fn push_stream_pcm(&mut self, samples: &[f32]) {
        let max_stream_samples = PCM_FRAME_SAMPLES * 2;
        if self.stream_buffer.len().saturating_add(samples.len()) > max_stream_samples {
            let drop_count =
                (self.stream_buffer.len() + samples.len()).saturating_sub(max_stream_samples);
            self.stream_buffer
                .drain(..drop_count.min(self.stream_buffer.len()));
        }
        self.stream_buffer.extend(samples.iter().copied());
    }

    /// Очищает очереди PCM-сэмплов без сброса настроек громкости `participant_volumes` и `stream_volume`.
    pub fn clear(&mut self) {
        for buf in self.participant_buffers.values_mut() {
            buf.clear();
        }
        self.stream_buffer.clear();
        self.active_speakers.clear();
    }

    /// Итератор по `user_id` участников, имевших ненулевую энергию сигнала в последнем кадре.
    pub fn speaking(&self) -> impl Iterator<Item = u64> + '_ {
        self.active_speakers.iter().copied()
    }

    /// Микширует один 20 мс кадр (`960` сэмплов) с текущими `participant_volumes` и `stream_volume`.
    pub fn pop(&mut self) -> (Option<PcmFrame>, bool) {
        let empty = HashMap::new();
        self.pop_with_volumes(&empty, None)
    }

    /// Микширует один 20 мс кадр (`960` сэмплов) с переданными громкостями участников
    /// (`HashMap<u64, f32>`) и независимой громкостью стрима (если `stream_volume_override == None`,
    /// используется сохранённый `self.stream_volume`), применяя мягкий лимитер `tanh`.
    pub fn pop_with_volumes(
        &mut self,
        volumes: &HashMap<u64, f32>,
        stream_volume_override: Option<f32>,
    ) -> (Option<PcmFrame>, bool) {
        for (&user_id, &vol) in volumes.iter().take(MAX_MIXER_PARTICIPANTS) {
            self.participant_volumes.insert(user_id, clamp_volume(vol));
        }
        if let Some(stream_vol) = stream_volume_override {
            self.stream_volume = clamp_volume(stream_vol);
        }

        let mut output = [0.0_f32; PCM_FRAME_SAMPLES];
        let mut active = false;
        let mut heard = false;
        self.active_speakers.clear();

        for (&user_id, buf) in self
            .participant_buffers
            .iter_mut()
            .take(MAX_MIXER_PARTICIPANTS)
        {
            if buf.is_empty() {
                continue;
            }
            let gain = self
                .participant_volumes
                .get(&user_id)
                .copied()
                .map_or(DEFAULT_MIXER_VOLUME, clamp_volume);
            let count = PCM_FRAME_SAMPLES.min(buf.len());
            let mut energy = 0.0_f32;
            for (idx, sample) in buf.drain(..count).enumerate() {
                if sample.is_finite() {
                    output[idx] += sample * gain;
                    energy += sample * sample;
                    if sample.abs() > 0.0 {
                        heard = true;
                    }
                }
            }
            if count > 0 {
                active = true;
            }
            if energy > 1e-6 {
                self.active_speakers.insert(user_id);
            }
        }

        if !self.stream_buffer.is_empty() {
            let count = PCM_FRAME_SAMPLES.min(self.stream_buffer.len());
            let stream_gain = clamp_volume(self.stream_volume);
            for (idx, sample) in self.stream_buffer.drain(..count).enumerate() {
                if stream_gain > 0.0 && sample.is_finite() {
                    output[idx] += sample * stream_gain;
                    if sample.abs() > 0.0 {
                        heard = true;
                    }
                }
            }
            if count > 0 {
                active = true;
            }
        }

        if active {
            for sample in &mut output {
                *sample = soft_limit_tanh(*sample);
            }
            (Some(output), heard)
        } else {
            (None, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_sequence_wraps_forward() {
        assert_eq!(extend_sequence(0, 65_535), 65_536);
        assert_eq!(extend_sequence(1, 65_536), 65_537);
        assert_eq!(extend_sequence(65_534, 65_535), 65_534);
    }

    #[test]
    fn mixer_separates_participant_volumes_from_stream_volume_and_preserves_on_ui_rerender() {
        let mut mixer = StreamVoiceMixer::new();
        assert!((mixer.stream_volume() - 1.0).abs() < 1e-6);
        assert!((mixer.participant_volume(101) - 1.0).abs() < 1e-6);

        // Установка громкости демонстрации экрана (0.0..=2.0) не влияет на участников.
        assert!((mixer.set_stream_volume(1.5) - 1.5).abs() < 1e-6);
        assert!((mixer.stream_volume() - 1.5).abs() < 1e-6);
        assert!((mixer.participant_volume(101) - 1.0).abs() < 1e-6);

        // Клампинг граничных значений (0.0..=2.0 и 0..=200%).
        assert!((mixer.set_stream_volume(3.5) - 2.0).abs() < 1e-6);
        assert!((mixer.set_stream_volume(-0.5) - 0.0).abs() < 1e-6);
        assert!((mixer.set_stream_volume_percent(125) - 1.25).abs() < 1e-6);

        assert!((mixer.set_participant_volume(101, 0.5) - 0.5).abs() < 1e-6);
        assert!((mixer.set_participant_volume(102, 2.5) - 2.0).abs() < 1e-6);
        assert!((mixer.set_participant_volume_percent(103, 75) - 0.75).abs() < 1e-6);

        // Перерисовка UI обновляет громкости участников, но НЕ сбрасывает stream_volume.
        let mut ui_volumes = HashMap::new();
        ui_volumes.insert(101, 0.8);
        ui_volumes.insert(102, 1.2);
        mixer.sync_ui_participant_volumes(&ui_volumes);
        assert!((mixer.stream_volume() - 1.25).abs() < 1e-6);
        assert!((mixer.participant_volume(101) - 0.8).abs() < 1e-6);
        assert!((mixer.participant_volume(102) - 1.2).abs() < 1e-6);
    }

    #[test]
    fn pop_with_volumes_mixes_participants_and_stream_independently_with_tanh_soft_limiter() {
        let mut mixer = StreamVoiceMixer::new();
        mixer.announce(1, 11).unwrap();
        mixer.announce(2, 22).unwrap();
        // Коллизия SSRC с другим пользователем отклоняется.
        assert!(mixer.announce(3, 11).is_err());
        assert_eq!(mixer.user_for_ssrc(11), Some(1));

        mixer.set_stream_volume(0.5);
        assert!(mixer.push_ssrc_pcm(11, &[0.4; PCM_FRAME_SAMPLES]));
        assert!(mixer.push_ssrc_pcm(22, &[0.2; PCM_FRAME_SAMPLES]));
        mixer.push_stream_pcm(&[0.6; PCM_FRAME_SAMPLES]);

        let mut overrides = HashMap::new();
        overrides.insert(1, 1.0); // 0.4 * 1.0 = 0.4
        overrides.insert(2, 0.5); // 0.2 * 0.5 = 0.1
        // Stream: 0.6 * 0.5 = 0.3 -> сумма до лимитера = 0.8 -> tanh(0.8)
        let (frame, heard) = mixer.pop_with_volumes(&overrides, None);
        let frame = frame.expect("mixed frame should be present");
        assert!(heard);
        let expected = 0.8_f32.tanh();
        for sample in frame {
            assert!(
                (sample - expected).abs() < 1e-5,
                "sample={sample}, expected={expected}"
            );
        }
        assert_eq!(mixer.speaking().collect::<Vec<_>>(), vec![1, 2]);

        // Изменение stream_volume в 0.0 заглушает только поток демки, голоса участников остаются.
        mixer.set_stream_volume(0.0);
        mixer.push_participant_pcm(1, &[0.4; PCM_FRAME_SAMPLES]);
        mixer.push_stream_pcm(&[0.9; PCM_FRAME_SAMPLES]);
        let (frame_muted_stream, _) = mixer.pop_with_volumes(&overrides, None);
        let frame_muted_stream = frame_muted_stream.unwrap();
        let expected_voice_only = 0.4_f32.tanh();
        for sample in frame_muted_stream {
            assert!((sample - expected_voice_only).abs() < 1e-5);
        }

        // Проверка мягкого лимитера tanh при перегрузке (сумма > 1.0 не даёт жёсткой ступеньки 1.0, но < 1.0).
        mixer.set_stream_volume(2.0);
        mixer.push_participant_pcm(1, &[0.9; PCM_FRAME_SAMPLES]);
        mixer.push_participant_pcm(2, &[0.9; PCM_FRAME_SAMPLES]);
        mixer.push_stream_pcm(&[0.9; PCM_FRAME_SAMPLES]);
        let mut loud = HashMap::new();
        loud.insert(1, 2.0);
        loud.insert(2, 2.0);
        let (overloaded, _) = mixer.pop_with_volumes(&loud, None);
        let overloaded = overloaded.unwrap();
        let raw_sum: f32 = 0.9 * 2.0 + 0.9 * 2.0 + 0.9 * 2.0;
        let expected_tanh = raw_sum.tanh();
        for sample in overloaded {
            assert!((sample - expected_tanh).abs() < 1e-5);
            assert!(sample <= 1.0 && sample > 0.99);
        }
        assert_eq!(soft_limit_tanh(f32::NAN), 0.0);
        assert_eq!(soft_limit_tanh(f32::INFINITY), 1.0);
    }
}
