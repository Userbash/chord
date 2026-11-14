#![forbid(unsafe_code)]
#![allow(dead_code, deprecated)]
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use cpal::traits::{DeviceTrait, HostTrait};

/// Звуковые устройства и аудио-движок на базе CPAL (заменяет 1 460 строк voice_devices.rs).
pub struct AudioEngine;

impl AudioEngine {
    /// Получить список доступных микрофонов (устройств ввода)
    pub fn list_input_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut names = Vec::new();
        if let Ok(devices) = host.input_devices() {
            for dev in devices {
                if let Ok(name) = dev.name() {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Получить список доступных динамиков / наушников (устройств вывода)
    pub fn list_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut names = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                if let Ok(name) = dev.name() {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Проверка наличия и зондирование видеокамер системы Linux (/dev/video*)
    pub fn probe_video_cameras() -> Vec<crate::state::CameraDevice> {
        let mut cameras = Vec::new();
        for i in 0..8 {
            let path = format!("/dev/video{}", i);
            if std::path::Path::new(&path).exists() {
                cameras.push(crate::state::CameraDevice {
                    id: format!("cam-v4l2-{}", i),
                    name: format!("V4L2 Video Device (/dev/video{})", i),
                    is_default: cameras.is_empty(),
                });
            }
        }
        if cameras.is_empty() {
            // Эмуляция встроенной HD-вебкамеры при отсутствии физического устройства
            cameras.push(crate::state::CameraDevice {
                id: "cam-default".into(),
                name: "Integrated HD Camera (Virtual)".into(),
                is_default: true,
            });
        }
        cameras
    }
}

/// Пакетный генератор и шифрование Discord Voice Protocol v8 по спецификации /home/sanya/re-deep/voice-deep.md
pub struct DiscordVoicePacket;

impl DiscordVoicePacket {
    /// Генерация 74-байтного IP Discovery пакета (re-deep/voice-deep.md §2.1)
    pub fn make_ip_discovery(ssrc: u32) -> [u8; 74] {
        let mut packet = [0u8; 74];
        packet[0..2].copy_from_slice(&1u16.to_be_bytes()); // type = 1
        packet[2..4].copy_from_slice(&70u16.to_be_bytes()); // len = 70 (0x0046)
        packet[4..8].copy_from_slice(&ssrc.to_be_bytes()); // SSRC
        // остальное 64 байта нулей
        packet
    }

    /// Разбор 74-байтного ответа IP Discovery (re-deep/voice-deep.md §2.1)
    pub fn parse_ip_discovery_response(buf: &[u8]) -> Option<(String, u16)> {
        if buf.len() != 74 {
            return None;
        }
        let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
        let msg_len = u16::from_be_bytes([buf[2], buf[3]]);
        if msg_type != 2 || msg_len != 70 {
            return None;
        }
        // Парсинг IP-строки (байты 8..72 с обязательным нулевым хвостом)
        let ip_bytes = &buf[8..72];
        let nul_pos = ip_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(ip_bytes.len());
        if nul_pos == 0 || ip_bytes[nul_pos..].iter().any(|&b| b != 0) {
            return None;
        }
        let ip_str = std::str::from_utf8(&ip_bytes[..nul_pos]).ok()?;
        if !ip_str
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b':')
        {
            return None;
        }
        let port = u16::from_be_bytes([buf[72], buf[73]]);
        if port == 0 {
            return None;
        }
        Some((ip_str.to_string(), port))
    }

    /// Генерация 8-байтного Keepalive пакета (re-deep/voice-deep.md §2.2: 13 37 ca fe 01 00 00 00)
    pub fn make_keepalive(seq: u32) -> [u8; 8] {
        let mut packet = [0u8; 8];
        packet[0..4].copy_from_slice(&0x1337_cafe_u32.to_le_bytes());
        packet[4..8].copy_from_slice(&seq.to_le_bytes());
        packet
    }

    /// Разбор 8-байтного Keepalive пакета (`Some((true, seq))` для запроса `0x1337cafe`,
    /// `Some((false, seq))` для ответа `0x1337f00d`).
    pub fn parse_keepalive(buf: &[u8]) -> Option<(bool, u32)> {
        if buf.len() != 8 {
            return None;
        }
        let magic_le = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let magic_be = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let seq = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        if magic_le == 0x1337_cafe || magic_be == 0x1337_cafe {
            Some((true, seq))
        } else if magic_le == 0x1337_f00d || magic_be == 0x1337_f00d {
            Some((false, seq))
        } else {
            None
        }
    }

    /// Шифрование RTP медиа-кадра AES-256-GCM (re-deep/voice-deep.md §2.3: aead_aes256_gcm_rtpsize)
    pub fn encrypt_rtp_aes_gcm(
        secret_key: &[u8; 32],
        rtp_header: &[u8],
        opus_payload: &[u8],
        seq_counter: u32,
    ) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(secret_key));

        // 12-байтный Nonce (4-байтный LE счетчик + 8 байт нулей)
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[0..4].copy_from_slice(&seq_counter.to_le_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Шифрование полезной нагрузки
        let ciphertext = cipher
            .encrypt(nonce, opus_payload)
            .map_err(|e| format!("AES-GCM encryption error: {:?}", e))?;

        // Сборка пакета: [RTP Header][Ciphertext + 16B GCM tag][4B LE counter]
        let mut final_packet = Vec::with_capacity(rtp_header.len() + ciphertext.len() + 4);
        final_packet.extend_from_slice(rtp_header);
        final_packet.extend_from_slice(&ciphertext);
        final_packet.extend_from_slice(&seq_counter.to_le_bytes());

        Ok(final_packet)
    }

    /// Дешифрование RTP медиа-кадра AES-256-GCM (SRTP без DAVE) с извлечением заголовка RTP (`seq`, `timestamp`, `ssrc`) и полезной нагрузки Opus
    pub fn decrypt_rtp_aes_gcm(
        secret_key: &[u8; 32],
        wire_packet: &[u8],
    ) -> Result<(u16, u32, u32, Vec<u8>), String> {
        use aes_gcm::aead::Payload;

        let header_len =
            Self::parse_rtpsize_header_len(wire_packet).map_err(|e| format!("{:?}", e))?;
        if wire_packet.len() < header_len + 16 + 4 {
            return Err(
                "SRTP пакет слишком короток (меньше заголовка RTP + 16B GCM tag + 4B nonce)"
                    .to_string(),
            );
        }

        let seq = u16::from_be_bytes([wire_packet[2], wire_packet[3]]);
        let timestamp = u32::from_be_bytes([
            wire_packet[4],
            wire_packet[5],
            wire_packet[6],
            wire_packet[7],
        ]);
        let ssrc = u32::from_be_bytes([
            wire_packet[8],
            wire_packet[9],
            wire_packet[10],
            wire_packet[11],
        ]);

        let trailer_offset = wire_packet.len() - 4;
        let ciphertext = &wire_packet[header_len..trailer_offset];
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[0..4].copy_from_slice(&wire_packet[trailer_offset..]);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(secret_key));
        let opus_payload = match cipher.decrypt(nonce, ciphertext) {
            Ok(pt) => pt,
            Err(_) => cipher
                .decrypt(
                    nonce,
                    Payload {
                        msg: ciphertext,
                        aad: &wire_packet[..header_len],
                    },
                )
                .map_err(|e| format!("AES-GCM decryption error: {:?}", e))?,
        };

        Ok((seq, timestamp, ssrc, opus_payload))
    }
}

/// Кольцевой буфер воспроизведения аудио (48 кГц, стерео PCM)
/// Защита от переполнения (overflow) и опустошения (underflow с заполнением тишиной).
#[derive(Debug, Clone)]
pub struct AudioRingBuffer {
    buffer: Vec<f32>,
    capacity: usize,
    read_pos: usize,
    write_pos: usize,
    available: usize,
}

impl AudioRingBuffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            capacity,
            read_pos: 0,
            write_pos: 0,
            available: 0,
        }
    }

    /// Запись декодированных PCM сэмплов в буфер
    pub fn write_samples(&mut self, samples: &[f32]) -> usize {
        let mut written = 0;
        for &sample in samples {
            if self.available >= self.capacity {
                self.read_pos = (self.read_pos + 1) % self.capacity;
                self.available -= 1;
            }
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            self.available += 1;
            written += 1;
        }
        written
    }

    /// Чтение сэмплов для CPAL callback с защитой от underflow (тишина)
    pub fn read_samples(&mut self, output: &mut [f32]) -> usize {
        let mut read = 0;
        for sample in output.iter_mut() {
            if self.available > 0 {
                *sample = self.buffer[self.read_pos];
                self.read_pos = (self.read_pos + 1) % self.capacity;
                self.available -= 1;
                read += 1;
            } else {
                *sample = 0.0;
            }
        }
        read
    }

    /// Количество доступных для чтения сэмплов
    pub fn available_samples(&self) -> usize {
        self.available
    }

    /// Емкость буфера
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Фреймер аудио-потока для голосового чата (20мс 48кГц = 960 сэмплов на канал)
pub struct VoiceAudioFramer {
    pub sample_rate: u32,
    pub channels: u16,
    pub frame_size_samples: usize,
    accumulated: Vec<f32>,
}

impl Default for VoiceAudioFramer {
    fn default() -> Self {
        Self::new(48000, 2)
    }
}

impl VoiceAudioFramer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        // 20ms frame = sample_rate / 50 * channels
        let frame_size_samples = ((sample_rate as f32 * 0.02) as usize) * (channels as usize);
        Self {
            sample_rate,
            channels,
            frame_size_samples,
            accumulated: Vec::with_capacity(frame_size_samples * 2),
        }
    }

    /// Добавление поступающих с микрофона сэмплов и извлечение готовых 20мс фреймов
    pub fn push_samples(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        self.accumulated.extend_from_slice(samples);
        let mut frames = Vec::new();
        while self.accumulated.len() >= self.frame_size_samples {
            let frame = self.accumulated.drain(0..self.frame_size_samples).collect();
            frames.push(frame);
        }
        frames
    }

    /// Сборка RTP заголовка (12 байт) для Discord Voice:
    /// byte 0: 0x80 (версия 2, no padding)
    /// byte 1: 0x78 (payload type 120 = Opus)
    /// byte 2-3: sequence number (BE)
    /// byte 4-7: timestamp (BE)
    /// byte 8-11: ssrc (BE)
    pub fn make_rtp_header(seq: u16, timestamp: u32, ssrc: u32) -> [u8; 12] {
        let mut header = [0u8; 12];
        header[0] = 0x80;
        header[1] = 0x78;
        header[2..4].copy_from_slice(&seq.to_be_bytes());
        header[4..8].copy_from_slice(&timestamp.to_be_bytes());
        header[8..12].copy_from_slice(&ssrc.to_be_bytes());
        header
    }
}

use chord_voice::rtcp::{ReportBlock, RtcpPacket, parse_compound};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

/// Минимальный битрейт Opus аудио-потока (32 кбит/с = 32 000 бит/с)
pub const OPUS_MIN_BITRATE_BPS: u32 = 32_000;
/// Максимальный битрейт Opus аудио-потока (64 кбит/с = 64 000 бит/с)
pub const OPUS_MAX_BITRATE_BPS: u32 = 64_000;
/// Стандартный начальный битрейт Opus аудио-потока (64 кбит/с)
pub const OPUS_DEFAULT_BITRATE_BPS: u32 = 64_000;
/// Минимальный битрейт Opus в кбит/с
pub const OPUS_MIN_BITRATE_KBPS: u32 = 32;
/// Максимальный битрейт Opus в кбит/с
pub const OPUS_MAX_BITRATE_KBPS: u32 = 64;
/// Длительность одного стандартного голосового фрейма Opus при 48 кГц (20 мс = 960 сэмплов/канал)
pub const OPUS_FRAME_DURATION_MS: u32 = 20;
/// Максимальный размер одного фрейма Opus согласно RFC 6716 (1275 байт)
pub const OPUS_MAX_PACKET_BYTES: usize = 1275;

/// Минимальная целевая глубина адаптивного джиттер-буфера (20 мс = 1 фрейм)
pub const JITTER_BUFFER_MIN_MS: u32 = 20;
/// Начальная целевая глубина адаптивного джиттер-буфера по умолчанию (40 мс = 2 фрейма)
pub const JITTER_BUFFER_DEFAULT_MS: u32 = 40;
/// Максимальная целевая глубина адаптивного джиттер-буфера (60 мс = 3 фрейма)
pub const JITTER_BUFFER_MAX_MS: u32 = 60;
/// Максимальная ёмкость очереди джиттер-буфера (в фреймах по 20 мс)
pub const JITTER_BUFFER_MAX_CAPACITY: usize = 32;

/// Порог низких потерь RTCP Receiver Report `fraction_lost` (<= 5/256 ≈ 1.95%)
pub const RTCP_LOSS_LOW_THRESHOLD: u8 = 5;
/// Порог умеренных потерь RTCP Receiver Report `fraction_lost` (>= 13/256 ≈ 5.08%)
pub const RTCP_LOSS_MODERATE_THRESHOLD: u8 = 13;
/// Порог высоких потерь RTCP Receiver Report `fraction_lost` (>= 26/256 ≈ 10.16%)
pub const RTCP_LOSS_HIGH_THRESHOLD: u8 = 26;

/// Контроллер адаптивного масштабирования битрейта Opus (32–64 кбит/с) по RTCP Receiver Reports (`fraction_lost`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpusBitrateController {
    target_bitrate_bps: u32,
    min_bitrate_bps: u32,
    max_bitrate_bps: u32,
    last_fraction_lost: u8,
    reports_processed: u64,
    scale_down_events: u64,
    scale_up_events: u64,
}

impl Default for OpusBitrateController {
    fn default() -> Self {
        Self::new()
    }
}

/// Извлечение максимального показателя потерь `fraction_lost` (1/256) из бинарного пакета RTCP (RR/SR)
pub fn extract_rtcp_fraction_lost(bytes: &[u8]) -> Result<u8, String> {
    if bytes.is_empty() {
        return Err("Пустой пакет RTCP".to_string());
    }
    let compound = parse_compound(bytes).map_err(|e| format!("Ошибка разбора RTCP: {:?}", e))?;
    let mut found_block = false;
    let mut max_fraction_lost = 0u8;

    for pkt in &compound.packets {
        let reports: &[ReportBlock] = match pkt {
            RtcpPacket::ReceiverReport { reports, .. } => reports,
            RtcpPacket::SenderReport { reports, .. } => reports,
            _ => continue,
        };
        for block in reports {
            found_block = true;
            if block.fraction_lost > max_fraction_lost {
                max_fraction_lost = block.fraction_lost;
            }
        }
    }

    if !found_block {
        return Err("Пакет RTCP не содержит блоков отчёта приёмника (ReportBlock)".to_string());
    }

    Ok(max_fraction_lost)
}

impl OpusBitrateController {
    /// Создание контроллера со стандартным диапазоном 32–64 кбит/с (начальный 64 кбит/с)
    pub fn new() -> Self {
        Self {
            target_bitrate_bps: OPUS_DEFAULT_BITRATE_BPS,
            min_bitrate_bps: OPUS_MIN_BITRATE_BPS,
            max_bitrate_bps: OPUS_MAX_BITRATE_BPS,
            last_fraction_lost: 0,
            reports_processed: 0,
            scale_down_events: 0,
            scale_up_events: 0,
        }
    }

    /// Создание контроллера с пользовательскими границами, зажатыми в пределах 32..=64 кбит/с
    pub fn with_bounds(min_bps: u32, max_bps: u32, initial_bps: u32) -> Self {
        let min_clamped = min_bps.clamp(OPUS_MIN_BITRATE_BPS, OPUS_MAX_BITRATE_BPS);
        let max_clamped = max_bps.clamp(min_clamped, OPUS_MAX_BITRATE_BPS);
        let init_clamped = initial_bps.clamp(min_clamped, max_clamped);
        Self {
            target_bitrate_bps: init_clamped,
            min_bitrate_bps: min_clamped,
            max_bitrate_bps: max_clamped,
            last_fraction_lost: 0,
            reports_processed: 0,
            scale_down_events: 0,
            scale_up_events: 0,
        }
    }

    /// Текущий целевой битрейт в бит/с (гарантированно 32_000..=64_000)
    pub fn target_bitrate_bps(&self) -> u32 {
        self.target_bitrate_bps
    }

    /// Текущий целевой битрейт в кбит/с (гарантированно 32..=64)
    pub fn target_bitrate_kbps(&self) -> u32 {
        self.target_bitrate_bps / 1_000
    }

    /// Рекомендуемый бюджет полезной нагрузки одного 20-мс фрейма Opus в байтах (80..=160 байт)
    pub fn target_frame_bytes_20ms(&self) -> usize {
        let bytes_per_sec = self.target_bitrate_bps / 8;
        let frames_per_sec = 1_000 / OPUS_FRAME_DURATION_MS;
        (bytes_per_sec / frames_per_sec) as usize
    }

    /// Последнее зафиксированное значение `fraction_lost` (в долях 1/256)
    pub fn last_fraction_lost(&self) -> u8 {
        self.last_fraction_lost
    }

    /// Общее количество обработанных RTCP отчётов
    pub fn reports_processed(&self) -> u64 {
        self.reports_processed
    }

    /// Количество срабатываний понижения битрейта из-за потерь
    pub fn scale_down_events(&self) -> u64 {
        self.scale_down_events
    }

    /// Количество шагов повышения битрейта при чистом канале
    pub fn scale_up_events(&self) -> u64 {
        self.scale_up_events
    }

    /// Пересчёт целевого битрейта Opus по значению `fraction_lost` (0..=255 в единицах 1/256)
    pub fn on_rtcp_fraction_lost(&mut self, fraction_lost: u8) -> u32 {
        self.last_fraction_lost = fraction_lost;
        self.reports_processed = self.reports_processed.saturating_add(1);

        if fraction_lost >= RTCP_LOSS_HIGH_THRESHOLD {
            // Высокие потери (>= ~10%): быстрое снижение на 16 кбит/с (25% от максимума)
            self.target_bitrate_bps = self
                .target_bitrate_bps
                .saturating_sub(16_000)
                .clamp(self.min_bitrate_bps, self.max_bitrate_bps);
            self.scale_down_events = self.scale_down_events.saturating_add(1);
        } else if fraction_lost >= RTCP_LOSS_MODERATE_THRESHOLD {
            // Умеренные потери (5%..10%): снижение на 8 кбит/с (12.5% от максимума)
            self.target_bitrate_bps = self
                .target_bitrate_bps
                .saturating_sub(8_000)
                .clamp(self.min_bitrate_bps, self.max_bitrate_bps);
            self.scale_down_events = self.scale_down_events.saturating_add(1);
        } else if fraction_lost <= RTCP_LOSS_LOW_THRESHOLD {
            // Чистый канал (<= ~2% потерь): плавное восстановление по +4 кбит/с
            self.target_bitrate_bps = self
                .target_bitrate_bps
                .saturating_add(4_000)
                .clamp(self.min_bitrate_bps, self.max_bitrate_bps);
            self.scale_up_events = self.scale_up_events.saturating_add(1);
        } else {
            // Пограничная зона (6..=12): удерживаем текущий битрейт
            self.target_bitrate_bps = self
                .target_bitrate_bps
                .clamp(self.min_bitrate_bps, self.max_bitrate_bps);
        }

        self.target_bitrate_bps
    }

    /// Обработка одного блока отчёта RTCP Receiver Report (`ReportBlock`)
    pub fn on_report_block(&mut self, block: &ReportBlock) -> u32 {
        self.on_rtcp_fraction_lost(block.fraction_lost)
    }

    /// Разбор бинарного пакета RTCP (RR/SR compound или 52B SRTCP RR) и адаптация битрейта
    pub fn on_rtcp_packet(&mut self, bytes: &[u8]) -> Result<u32, String> {
        let fraction_lost = extract_rtcp_fraction_lost(bytes)?;
        Ok(self.on_rtcp_fraction_lost(fraction_lost))
    }
}

/// Один 20-мс голосовой фрейм Opus в адаптивном джиттер-буфере
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JitterFrame {
    pub sequence: u16,
    pub timestamp: u32,
    pub payload: Vec<u8>,
}

/// Результат извлечения очередного 20-мс фрейма из джиттер-буфера
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitterPopStatus {
    /// Буфер накапливает начальную задержку (20–60 мс) перед стартом воспроизведения
    Buffering,
    /// Готов валидный упорядоченный фрейм Opus
    Ready(JitterFrame),
    /// Обнаружен пропуск sequence — декодер должен сгенерировать PLC-фрейм (Packet Loss Concealment)
    ConcealLoss { missing_sequence: u16 },
    /// Очередь полностью исчерпана — возврат в режим накопления (Buffering)
    Empty,
}

/// Адаптивный джиттер-буфер Opus (20–60 мс / 1–3 фрейма по 20 мс), управляемый RTCP `fraction_lost`
#[derive(Debug, Clone)]
pub struct AdaptiveJitterBuffer {
    frames: BTreeMap<u16, JitterFrame>,
    target_delay_ms: u32,
    next_sequence: Option<u16>,
    buffering: bool,
    packets_received: u64,
    packets_dropped_late: u64,
    packets_dropped_duplicate: u64,
    plc_frames_generated: u64,
    last_fraction_lost: u8,
}

impl Default for AdaptiveJitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveJitterBuffer {
    fn normalize_delay_ms(delay_ms: u32) -> u32 {
        let clamped = delay_ms.clamp(JITTER_BUFFER_MIN_MS, JITTER_BUFFER_MAX_MS);
        if clamped <= 30 {
            JITTER_BUFFER_MIN_MS
        } else if clamped <= 50 {
            JITTER_BUFFER_DEFAULT_MS
        } else {
            JITTER_BUFFER_MAX_MS
        }
    }

    fn earliest_sequence(&self) -> Option<u16> {
        self.frames
            .keys()
            .copied()
            .min_by(|a, b| (a.wrapping_sub(*b) as i16).cmp(&0))
    }

    /// Создание адаптивного джиттер-буфера со стандартной глубиной 40 мс (2 фрейма)
    pub fn new() -> Self {
        Self {
            frames: BTreeMap::new(),
            target_delay_ms: JITTER_BUFFER_DEFAULT_MS,
            next_sequence: None,
            buffering: true,
            packets_received: 0,
            packets_dropped_late: 0,
            packets_dropped_duplicate: 0,
            plc_frames_generated: 0,
            last_fraction_lost: 0,
        }
    }

    /// Создание адаптивного джиттер-буфера с заданной начальной задержкой (зажимается в 20..=60 мс)
    pub fn with_target_delay_ms(delay_ms: u32) -> Self {
        Self {
            frames: BTreeMap::new(),
            target_delay_ms: Self::normalize_delay_ms(delay_ms),
            next_sequence: None,
            buffering: true,
            packets_received: 0,
            packets_dropped_late: 0,
            packets_dropped_duplicate: 0,
            plc_frames_generated: 0,
            last_fraction_lost: 0,
        }
    }

    /// Целевая задержка буферизации в миллисекундах (гарантированно 20, 40 или 60 мс)
    pub fn target_delay_ms(&self) -> u32 {
        self.target_delay_ms
    }

    /// Целевое количество 20-мс фреймов в буфере перед выдачей (1..=3)
    pub fn target_frames(&self) -> usize {
        (self.target_delay_ms / OPUS_FRAME_DURATION_MS).clamp(1, 3) as usize
    }

    /// Текущее число буферизованных фреймов
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Пуст ли джиттер-буфер
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Находится ли буфер в состоянии первичного накопления (pre-buffering)
    pub fn is_buffering(&self) -> bool {
        self.buffering
    }

    /// Последнее полученное значение `fraction_lost`
    pub fn last_fraction_lost(&self) -> u8 {
        self.last_fraction_lost
    }

    /// Число успешно принятых в буфер пакетов
    pub fn packets_received(&self) -> u64 {
        self.packets_received
    }

    /// Число отброшенных опоздавших пакетов (sequence меньше ожидаемого)
    pub fn packets_dropped_late(&self) -> u64 {
        self.packets_dropped_late
    }

    /// Число отброшенных дубликатов
    pub fn packets_dropped_duplicate(&self) -> u64 {
        self.packets_dropped_duplicate
    }

    /// Число сгенерированных сигналов PLC при пропуске пакетов
    pub fn plc_frames_generated(&self) -> u64 {
        self.plc_frames_generated
    }

    /// Адаптация глубины джиттер-буфера (20–60 мс) по RTCP Receiver Report `fraction_lost`
    pub fn on_rtcp_fraction_lost(&mut self, fraction_lost: u8) -> u32 {
        self.last_fraction_lost = fraction_lost;
        if fraction_lost >= RTCP_LOSS_HIGH_THRESHOLD {
            self.target_delay_ms = JITTER_BUFFER_MAX_MS;
        } else if fraction_lost >= RTCP_LOSS_MODERATE_THRESHOLD {
            self.target_delay_ms = JITTER_BUFFER_DEFAULT_MS;
        } else if fraction_lost <= RTCP_LOSS_LOW_THRESHOLD {
            self.target_delay_ms = self
                .target_delay_ms
                .saturating_sub(OPUS_FRAME_DURATION_MS)
                .max(JITTER_BUFFER_MIN_MS);
        } else if self.target_delay_ms < JITTER_BUFFER_DEFAULT_MS {
            self.target_delay_ms = JITTER_BUFFER_DEFAULT_MS;
        }
        self.target_delay_ms
    }

    /// Адаптация глубины джиттер-буфера по блоку RTCP `ReportBlock`
    pub fn on_report_block(&mut self, block: &ReportBlock) -> u32 {
        self.on_rtcp_fraction_lost(block.fraction_lost)
    }

    /// Разбор бинарного пакета RTCP и обновление целевой глубины джиттер-буфера (20–60 мс)
    pub fn on_rtcp_packet(&mut self, bytes: &[u8]) -> Result<u32, String> {
        let fraction_lost = extract_rtcp_fraction_lost(bytes)?;
        Ok(self.on_rtcp_fraction_lost(fraction_lost))
    }

    /// Добавление входящего RTP/Opus фрейма в джиттер-буфер с учётом переупорядочивания и wrap-around
    pub fn push(
        &mut self,
        sequence: u16,
        timestamp: u32,
        payload: Vec<u8>,
    ) -> Result<bool, String> {
        if payload.is_empty() {
            return Err("Пустой полезный груз пакета Opus".to_string());
        }
        if payload.len() > OPUS_MAX_PACKET_BYTES {
            return Err(format!(
                "Размер пакета Opus ({} байт) превышает лимит RFC 6716 ({} байт)",
                payload.len(),
                OPUS_MAX_PACKET_BYTES
            ));
        }

        if let Some(expected) = self.next_sequence {
            if !self.buffering {
                let diff = sequence.wrapping_sub(expected) as i16;
                if diff < 0 {
                    self.packets_dropped_late = self.packets_dropped_late.saturating_add(1);
                    return Ok(false);
                }
            }
        }

        if self.frames.contains_key(&sequence) {
            self.packets_dropped_duplicate = self.packets_dropped_duplicate.saturating_add(1);
            return Ok(false);
        }

        if self.frames.len() >= JITTER_BUFFER_MAX_CAPACITY {
            if let Some(oldest_seq) = self.earliest_sequence() {
                self.frames.remove(&oldest_seq);
                if self.next_sequence == Some(oldest_seq) {
                    self.next_sequence = Some(oldest_seq.wrapping_add(1));
                }
            }
        }

        self.frames.insert(
            sequence,
            JitterFrame {
                sequence,
                timestamp,
                payload,
            },
        );
        self.packets_received = self.packets_received.saturating_add(1);

        if self.buffering && self.frames.len() >= self.target_frames() {
            self.buffering = false;
            if self.next_sequence.is_none() {
                self.next_sequence = self.earliest_sequence();
            }
        }

        Ok(true)
    }

    /// Извлечение следующего 20-мс фрейма для воспроизведения (или сигнала PLC / Buffering)
    pub fn pop(&mut self) -> JitterPopStatus {
        if self.buffering {
            if self.frames.len() < self.target_frames() {
                return JitterPopStatus::Buffering;
            }
            self.buffering = false;
            if self.next_sequence.is_none() {
                self.next_sequence = self.earliest_sequence();
            }
        }

        if self.frames.is_empty() {
            self.buffering = true;
            self.next_sequence = None;
            return JitterPopStatus::Empty;
        }

        let expected = match self.next_sequence.or_else(|| self.earliest_sequence()) {
            Some(seq) => seq,
            None => {
                self.buffering = true;
                return JitterPopStatus::Empty;
            }
        };
        self.next_sequence = Some(expected.wrapping_add(1));

        if let Some(frame) = self.frames.remove(&expected) {
            JitterPopStatus::Ready(frame)
        } else {
            self.plc_frames_generated = self.plc_frames_generated.saturating_add(1);
            JitterPopStatus::ConcealLoss {
                missing_sequence: expected,
            }
        }
    }

    /// Сброс состояния джиттер-буфера
    pub fn reset(&mut self) {
        self.frames.clear();
        self.next_sequence = None;
        self.buffering = true;
    }
}

/// Конфигурация прямого голосового UDP-подключения
#[derive(Debug, Clone)]
pub struct VoiceUdpConfig {
    pub remote_addr: SocketAddr,
    pub ssrc: u32,
    pub secret_key: [u8; 32],
}

/// Статистика переданных медиа-пакетов и адаптивного управления качеством (Opus + Jitter Buffer + DAVE E2EE)
#[derive(Debug, Clone)]
pub struct VoiceStreamStats {
    pub packets_sent: u64,
    pub bytes_sent: u64,
    pub keepalives_sent: u32,
    pub current_bitrate_bps: u32,
    pub jitter_buffer_ms: u32,
    pub rtcp_reports_received: u64,
    pub last_fraction_lost: u8,
    pub dave_frames_encrypted: u64,
    pub dave_frames_decrypted: u64,
    pub dave_epoch: u64,
}

impl Default for VoiceStreamStats {
    fn default() -> Self {
        Self {
            packets_sent: 0,
            bytes_sent: 0,
            keepalives_sent: 0,
            current_bitrate_bps: OPUS_DEFAULT_BITRATE_BPS,
            jitter_buffer_ms: JITTER_BUFFER_DEFAULT_MS,
            rtcp_reports_received: 0,
            last_fraction_lost: 0,
            dave_frames_encrypted: 0,
            dave_frames_decrypted: 0,
            dave_epoch: 0,
        }
    }
}

/// Прямой стример голосовых UDP пакетов по протоколу Discord Voice v8
/// Использует стандартный tokio::net::UdpSocket и алгоритм Serein transport.rs
pub struct VoiceUdpStreamer {
    pub socket: Arc<UdpSocket>,
    pub config: VoiceUdpConfig,
    pub sequence: Arc<Mutex<u16>>,
    pub timestamp: Arc<Mutex<u32>>,
    pub keepalive_seq: Arc<Mutex<u32>>,
    pub srtp_nonce_counter: Arc<Mutex<u32>>,
    pub dave_session: Arc<Mutex<Option<DaveVoiceSession>>>,
    pub stats: Arc<Mutex<VoiceStreamStats>>,
    pub bitrate_controller: Arc<Mutex<OpusBitrateController>>,
    pub jitter_buffer: Arc<Mutex<AdaptiveJitterBuffer>>,
    pub is_running: Arc<AtomicBool>,
}

impl VoiceUdpStreamer {
    /// Инициализация и привязка сокета к 0.0.0.0:0 с подключением к голосовому шлюзу
    pub async fn bind_and_connect(config: VoiceUdpConfig) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(config.remote_addr).await?;
        Ok(Self {
            socket: Arc::new(socket),
            config,
            sequence: Arc::new(Mutex::new(0)),
            timestamp: Arc::new(Mutex::new(0)),
            keepalive_seq: Arc::new(Mutex::new(0)),
            srtp_nonce_counter: Arc::new(Mutex::new(0)),
            dave_session: Arc::new(Mutex::new(None)),
            stats: Arc::new(Mutex::new(VoiceStreamStats::default())),
            bitrate_controller: Arc::new(Mutex::new(OpusBitrateController::new())),
            jitter_buffer: Arc::new(Mutex::new(AdaptiveJitterBuffer::new())),
            is_running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Подключение сессии сквозного шифрования Discord DAVE E2EE MLS v1 к голосовому стримеру
    pub async fn enable_dave_session(&self, session: DaveVoiceSession) {
        let epoch = session.current_epoch();
        {
            let mut guard = self.dave_session.lock().await;
            *guard = Some(session);
        }
        let mut stats = self.stats.lock().await;
        stats.dave_epoch = epoch;
    }

    /// Отправка двухслойно зашифрованного пакета (DAVE MLS v1 E2EE внутри + SRTP `*_rtpsize` снаружи)
    pub async fn send_dave_audio_packet(
        &self,
        opus_data: &[u8],
        rtp_extension_words: Option<&[[u8; 4]]>,
    ) -> Result<usize, DaveError> {
        let seq = {
            let mut seq_guard = self.sequence.lock().await;
            let current = *seq_guard;
            *seq_guard = seq_guard.wrapping_add(1);
            current
        };
        let ts = {
            let mut ts_guard = self.timestamp.lock().await;
            let current = *ts_guard;
            *ts_guard = ts_guard.wrapping_add(960);
            current
        };
        let srtp_counter = {
            let mut ctr_guard = self.srtp_nonce_counter.lock().await;
            let current = *ctr_guard;
            *ctr_guard = ctr_guard.wrapping_add(1);
            current
        };

        let rtp_header = match rtp_extension_words {
            Some(words) if !words.is_empty() => {
                DiscordVoicePacket::make_rtp_header_with_bede_ext(seq, ts, self.config.ssrc, words)
            }
            _ => VoiceAudioFramer::make_rtp_header(seq, ts, self.config.ssrc).to_vec(),
        };

        let (wire_packet, current_epoch) = {
            let mut session_guard = self.dave_session.lock().await;
            let session = session_guard
                .as_mut()
                .ok_or(DaveError::MalformedFrame("DAVE session is not enabled"))?;
            let mode = session.transport_mode();
            let pkt = DiscordVoicePacket::seal_rtp_with_dave(
                mode,
                &self.config.secret_key,
                &rtp_header,
                opus_data,
                srtp_counter,
                session,
            )?;
            (pkt, session.current_epoch())
        };

        let bytes_sent = self
            .socket
            .send(&wire_packet)
            .await
            .map_err(|e| DaveError::IoError(format!("UDP send DAVE audio error: {}", e)))?;

        let mut stats = self.stats.lock().await;
        stats.packets_sent += 1;
        stats.bytes_sent += bytes_sent as u64;
        stats.dave_frames_encrypted += 1;
        stats.dave_epoch = current_epoch;

        Ok(bytes_sent)
    }

    /// Приём и двухслойная расшифровка входящего UDP медиа-пакета (SRTP `*_rtpsize` + DAVE MLS v1 E2EE)
    pub async fn recv_dave_audio_packet(&self, sender_user_id: u64) -> Result<Vec<u8>, DaveError> {
        let mut buf = [0u8; 2048];
        let recv_fut = self.socket.recv(&mut buf);
        let n = tokio::time::timeout(std::time::Duration::from_millis(1500), recv_fut)
            .await
            .map_err(|_| DaveError::IoError("UDP recv DAVE timeout (1500ms)".to_string()))?
            .map_err(|e| DaveError::IoError(format!("UDP recv DAVE error: {}", e)))?;

        let (opus_plaintext, current_epoch) = {
            let mut session_guard = self.dave_session.lock().await;
            let session = session_guard
                .as_mut()
                .ok_or(DaveError::MalformedFrame("DAVE session is not enabled"))?;
            let mode = session.transport_mode();
            let (_clear_hdr, opus, _counter) = DiscordVoicePacket::open_rtp_with_dave(
                mode,
                &self.config.secret_key,
                &buf[..n],
                sender_user_id,
                session,
            )?;
            (opus, session.current_epoch())
        };

        let mut stats = self.stats.lock().await;
        stats.dave_frames_decrypted += 1;
        stats.dave_epoch = current_epoch;

        Ok(opus_plaintext)
    }

    /// Приём и двухслойная расшифровка входящего UDP медиа-пакета (SRTP `*_rtpsize` + DAVE MLS v1 E2EE)
    /// с извлечением метаданных заголовка RTP (`sequence`, `timestamp`, `ssrc`) для `AdaptiveJitterBuffer`.
    pub async fn recv_dave_rtp_packet(
        &self,
        sender_user_id: u64,
    ) -> Result<DecryptedRtpAudioPacket, DaveError> {
        let mut buf = [0u8; 2048];
        let recv_fut = self.socket.recv(&mut buf);
        let n = tokio::time::timeout(std::time::Duration::from_millis(1500), recv_fut)
            .await
            .map_err(|_| DaveError::IoError("UDP recv DAVE timeout (1500ms)".to_string()))?
            .map_err(|e| DaveError::IoError(format!("UDP recv DAVE error: {}", e)))?;

        let (clear_hdr, opus_plaintext, current_epoch) = {
            let mut session_guard = self.dave_session.lock().await;
            let session = session_guard
                .as_mut()
                .ok_or(DaveError::MalformedFrame("DAVE session is not enabled"))?;
            let mode = session.transport_mode();
            let (clear_hdr, opus, _counter) = DiscordVoicePacket::open_rtp_with_dave(
                mode,
                &self.config.secret_key,
                &buf[..n],
                sender_user_id,
                session,
            )?;
            (clear_hdr, opus, session.current_epoch())
        };

        if clear_hdr.len() < 12 {
            return Err(DaveError::RtpPacketInvalid("RTP header shorter than 12B"));
        }
        let sequence = u16::from_be_bytes([clear_hdr[2], clear_hdr[3]]);
        let timestamp =
            u32::from_be_bytes([clear_hdr[4], clear_hdr[5], clear_hdr[6], clear_hdr[7]]);
        let ssrc = u32::from_be_bytes([clear_hdr[8], clear_hdr[9], clear_hdr[10], clear_hdr[11]]);

        let mut stats = self.stats.lock().await;
        stats.dave_frames_decrypted += 1;
        stats.dave_epoch = current_epoch;

        Ok(DecryptedRtpAudioPacket {
            sequence,
            timestamp,
            ssrc,
            opus_payload: opus_plaintext,
        })
    }

    /// Приём и расшифровка входящего UDP медиа-пакета в режиме SRTP без DAVE (`AES-256-GCM`)
    /// с извлечением `sequence`, `timestamp`, `ssrc` и полезной нагрузки Opus.
    pub async fn recv_audio_packet(&self) -> Result<DecryptedRtpAudioPacket, String> {
        let mut buf = [0u8; 2048];
        let recv_fut = self.socket.recv(&mut buf);
        let n = tokio::time::timeout(std::time::Duration::from_millis(1500), recv_fut)
            .await
            .map_err(|_| "UDP recv audio timeout (1500ms)".to_string())?
            .map_err(|e| format!("UDP recv audio error: {}", e))?;

        let (sequence, timestamp, ssrc, opus_payload) =
            DiscordVoicePacket::decrypt_rtp_aes_gcm(&self.config.secret_key, &buf[..n])?;

        Ok(DecryptedRtpAudioPacket {
            sequence,
            timestamp,
            ssrc,
            opus_payload,
        })
    }

    /// Обработка входящего пакета RTCP Receiver Report (разбор `fraction_lost`, адаптация битрейта 32–64 кбит/с и джиттер-буфера 20–60 мс)
    pub async fn handle_rtcp_receiver_report(
        &self,
        rtcp_bytes: &[u8],
    ) -> Result<(u32, u32), String> {
        let fraction_lost = extract_rtcp_fraction_lost(rtcp_bytes)?;
        Ok(self.apply_fraction_lost(fraction_lost).await)
    }

    /// Применение известного значения `fraction_lost` к контроллеру битрейта и джиттер-буферу стримера
    pub async fn apply_fraction_lost(&self, fraction_lost: u8) -> (u32, u32) {
        let new_bitrate_bps = {
            let mut ctrl = self.bitrate_controller.lock().await;
            ctrl.on_rtcp_fraction_lost(fraction_lost)
        };
        let new_jitter_ms = {
            let mut jb = self.jitter_buffer.lock().await;
            jb.on_rtcp_fraction_lost(fraction_lost)
        };
        {
            let mut stats = self.stats.lock().await;
            stats.current_bitrate_bps = new_bitrate_bps;
            stats.jitter_buffer_ms = new_jitter_ms;
            stats.rtcp_reports_received = stats.rtcp_reports_received.saturating_add(1);
            stats.last_fraction_lost = fraction_lost;
        }
        (new_bitrate_bps, new_jitter_ms)
    }

    /// Выполнение IP Discovery (74 байта) для определения внешнего адреса и порта клиента
    pub async fn perform_ip_discovery(&self) -> Result<(String, u16), String> {
        let req = DiscordVoicePacket::make_ip_discovery(self.config.ssrc);
        self.socket
            .send(&req)
            .await
            .map_err(|e| format!("UDP send discovery error: {}", e))?;

        let mut buf = [0u8; 128];
        let recv_fut = self.socket.recv(&mut buf);
        let res = tokio::time::timeout(std::time::Duration::from_millis(1500), recv_fut)
            .await
            .map_err(|_| "IP Discovery timeout (1500ms)".to_string())?
            .map_err(|e| format!("UDP recv error: {}", e))?;

        if let Some((ip, port)) = DiscordVoicePacket::parse_ip_discovery_response(&buf[..res]) {
            Ok((ip, port))
        } else {
            Err("Не удалось разобрать IP Discovery ответ".to_string())
        }
    }

    /// Отправка Keepalive пакета (8 байт: 0x1337cafe + sequence)
    pub async fn send_keepalive(&self) -> Result<(), String> {
        let mut seq_guard = self.keepalive_seq.lock().await;
        *seq_guard = seq_guard.wrapping_add(1);
        let seq = *seq_guard;
        drop(seq_guard);

        let packet = DiscordVoicePacket::make_keepalive(seq);
        self.socket
            .send(&packet)
            .await
            .map_err(|e| format!("UDP send keepalive error: {}", e))?;

        let mut stats = self.stats.lock().await;
        stats.keepalives_sent += 1;
        Ok(())
    }

    /// Отправка зашифрованного аудио-пакета Opus (RTP + AEAD AES-256-GCM)
    pub async fn send_audio_packet(&self, opus_data: &[u8]) -> Result<usize, String> {
        let mut seq_guard = self.sequence.lock().await;
        let seq = *seq_guard;
        *seq_guard = seq.wrapping_add(1);
        drop(seq_guard);

        let mut ts_guard = self.timestamp.lock().await;
        let ts = *ts_guard;
        // 20мс фрейм при 48 кГц = 960 сэмплов
        *ts_guard = ts.wrapping_add(960);
        drop(ts_guard);

        let rtp_header = VoiceAudioFramer::make_rtp_header(seq, ts, self.config.ssrc);
        let encrypted = DiscordVoicePacket::encrypt_rtp_aes_gcm(
            &self.config.secret_key,
            &rtp_header,
            opus_data,
            seq as u32,
        )?;

        let bytes_sent = self
            .socket
            .send(&encrypted)
            .await
            .map_err(|e| format!("UDP send audio error: {}", e))?;

        let mut stats = self.stats.lock().await;
        stats.packets_sent += 1;
        stats.bytes_sent += bytes_sent as u64;

        Ok(bytes_sent)
    }

    /// Остановка передачи
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

// ============================================================================
// Discord DAVE E2EE MLS v1: Key Schedule, Sender Hash Ratchet & Frame Cryptor
// (спецификация /home/sanya/re-deep/voice-deep.md §1–§3, native-re mlspp, RFC 9420 §8)
// ============================================================================

/// Версия протокола сквозного голосового шифрования Discord DAVE E2EE (`voice-deep.md` §3: `protocol version: 1`).
pub const DAVE_PROTOCOL_VERSION_V1: u16 = 1;
/// Идентификатор MLS Ciphersuite для Discord DAVE v1 (`MLS_128_DHKEMP256_AES128GCM_SHA256_P256`).
pub const DAVE_MLS_CIPHERSUITE_ID: u16 = 0x0001;
/// Метка MLS-экспортёра базового секрета отправителя медиа (`chord_voice.node` / `libdave`).
pub const DAVE_USER_MEDIA_KEY_BASE_LABEL: &str = "Discord Secure Frames v0";
/// Магический маркер конца зашифрованного кадра DAVE (`0xFAFA`).
pub const DAVE_MARKER_BYTES: [u8; 2] = [0xFA, 0xFA];
/// Пакет тишины Opus DTX (`[0xF8, 0xFF, 0xFE]`), пропускаемый DAVE прозрачно без шифрования (`voice-deep.md` §2.3).
pub const DAVE_OPUS_SILENCE_PACKET: [u8; 3] = [0xF8, 0xFF, 0xFE];
/// Длина ключа AES-128-GCM в кадровом шифре DAVE v1 (16 байт).
pub const DAVE_AES128_KEY_BYTES: usize = 16;
/// Длина вектора инициализации AES-128-GCM (12 байт).
pub const DAVE_AES128_NONCE_BYTES: usize = 12;
/// Длина усечённого синхронизационного nonce в DAVE (4 байта LE в `nonce[8..12]`).
pub const DAVE_TRUNCATED_SYNC_NONCE_BYTES: usize = 4;
/// Смещение 4-байтного усечённого nonce внутри 12-байтного вектора инициализации (`12 - 4 = 8`).
pub const DAVE_TRUNCATED_SYNC_NONCE_OFFSET: usize =
    DAVE_AES128_NONCE_BYTES - DAVE_TRUNCATED_SYNC_NONCE_BYTES;
/// Длина усечённого тега аутентификации AES-128-GCM в DAVE v1 (8 байт / 64 бит).
pub const DAVE_TRUNCATED_TAG_BYTES: usize = 8;
/// Сдвиг 8-битного номера поколения внутри 32-битного `truncated_nonce` (`truncated_nonce >> 24`).
pub const DAVE_RATCHET_GENERATION_SHIFT_BITS: u32 = 24;
/// Период оборачивания 8-битного поля поколения в заголовке кадра (`256`).
pub const DAVE_GENERATION_WRAP: u32 = 256;
/// Максимально допустимый скачок поколения вперёд (`libdave`: `MAX_GENERATION_GAP = 250`).
pub const DAVE_MAX_GENERATION_GAP: u32 = 250;
/// Максимальный размер окна пропущенных nonce для защиты от повторов (`MAX_MISSING_NONCES = 1000`).
pub const DAVE_MAX_MISSING_NONCES: u64 = 1000;
/// Минимальный размер supplemental-секции DAVE (`8B tag + 1B size + 2B 0xFAFA = 11`, без учёта LEB128 nonce).
pub const DAVE_MIN_SUPPLEMENTAL_BYTES: usize = DAVE_TRUNCATED_TAG_BYTES + 1 + 2;
/// Старший бит исходящего трейлера-счётчика Discord SRTP (`voice-deep.md` §2.3: `0x80000000 | n`).
pub const SRTP_OUTBOUND_COUNTER_MSB: u32 = 0x8000_0000;

/// Опкоды сигнализации Discord DAVE MLS в голосовом шлюзе (`21..=31`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DaveVoiceOpcode {
    PrepareTransition = 21,
    ExecuteTransition = 22,
    ReadyForTransition = 23,
    PrepareEpoch = 24,
    ExternalSenderPackage = 25,
    KeyPackage = 26,
    Proposals = 27,
    CommitWelcome = 28,
    PrepareCommitTransition = 29,
    Welcome = 30,
    InvalidCommitWelcome = 31,
}

impl DaveVoiceOpcode {
    pub fn from_u8(code: u8) -> Option<Self> {
        match code {
            21 => Some(Self::PrepareTransition),
            22 => Some(Self::ExecuteTransition),
            23 => Some(Self::ReadyForTransition),
            24 => Some(Self::PrepareEpoch),
            25 => Some(Self::ExternalSenderPackage),
            26 => Some(Self::KeyPackage),
            27 => Some(Self::Proposals),
            28 => Some(Self::CommitWelcome),
            29 => Some(Self::PrepareCommitTransition),
            30 => Some(Self::Welcome),
            31 => Some(Self::InvalidCommitWelcome),
            _ => None,
        }
    }
}

/// Транспортные режимы шифрования Discord SRTP `*_rtpsize` (`voice-deep.md` §1, §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaveTransportCipherMode {
    /// `aead_aes256_gcm_rtpsize` — основной выбранный режим движка Discord (`voice-deep.md` §3).
    AeadAes256GcmRtpsize,
    /// `aead_xchacha20_poly1305_rtpsize` — резервный транспортный режим Discord.
    AeadXChaCha20Poly1305Rtpsize,
}

impl DaveTransportCipherMode {
    pub fn as_engine_str(&self) -> &'static str {
        match self {
            Self::AeadAes256GcmRtpsize => "aead_aes256_gcm_rtpsize",
            Self::AeadXChaCha20Poly1305Rtpsize => "aead_xchacha20_poly1305_rtpsize",
        }
    }

    pub fn from_engine_str(mode: &str) -> Option<Self> {
        match mode {
            "aead_aes256_gcm_rtpsize" => Some(Self::AeadAes256GcmRtpsize),
            "aead_xchacha20_poly1305_rtpsize" => Some(Self::AeadXChaCha20Poly1305Rtpsize),
            _ => None,
        }
    }
}

/// Ошибки криптографического конвейера Discord DAVE E2EE MLS v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaveError {
    InvalidKeyLength {
        expected: usize,
        actual: usize,
    },
    InvalidOutputLength(usize),
    VlBytesTooLarge(usize),
    GenerationGapExceeded {
        requested: u32,
        current: u32,
        max_gap: u32,
    },
    KeyExpired {
        generation: u32,
        oldest_available: u32,
    },
    ConfirmationTagMismatch,
    DisplayableCodeInvalid(&'static str),
    DowngradeRejected {
        current_version: u16,
        requested_version: u16,
    },
    TransitionNotPrepared {
        transition_id: u16,
    },
    MissingKeyRatchet {
        user_id: u64,
    },
    MalformedFrame(&'static str),
    UnencryptedFrameRejected,
    ReplayOrExpiredNonce {
        generation: u32,
        truncated_nonce: u32,
    },
    AeadAuthenticationFailed,
    RtpPacketInvalid(&'static str),
    IoError(String),
}

impl std::fmt::Display for DaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for DaveError {}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[derive(Clone)]
struct Sha256State {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Sha256State {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    fn compress_block(state: &mut [u32; 8], block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buffer_len > 0 {
            let needed = 64 - self.buffer_len;
            if data.len() < needed {
                self.buffer[self.buffer_len..self.buffer_len + data.len()].copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            }
            self.buffer[self.buffer_len..64].copy_from_slice(&data[..needed]);
            Self::compress_block(&mut self.state, &self.buffer);
            self.buffer_len = 0;
            data = &data[needed..];
        }

        let mut chunks = data.chunks_exact(64);
        for chunk in &mut chunks {
            let mut block = [0u8; 64];
            block.copy_from_slice(chunk);
            Self::compress_block(&mut self.state, &block);
        }

        let rem = chunks.remainder();
        if !rem.is_empty() {
            self.buffer[..rem.len()].copy_from_slice(rem);
            self.buffer_len = rem.len();
        }
    }

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total_len.wrapping_mul(8);
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            for b in &mut self.buffer[self.buffer_len..64] {
                *b = 0;
            }
            Self::compress_block(&mut self.state, &self.buffer);
            self.buffer = [0u8; 64];
            self.buffer_len = 0;
        } else {
            for b in &mut self.buffer[self.buffer_len..56] {
                *b = 0;
            }
        }

        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        Self::compress_block(&mut self.state, &self.buffer);

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// Константно-временное сравнение байтовых срезов (защита от timing side-channel).
fn dave_ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn zeroize_slice(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *std::hint::black_box(b) = 0;
    }
    std::sync::atomic::compiler_fence(Ordering::SeqCst);
}

/// Хеш-функция FIPS 180-4 SHA-256 (используется в MLS 1.0 Ciphersuite `0x0001`).
pub fn dave_sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256State::new();
    hasher.update(data);
    hasher.finalize()
}

/// Код аутентификации сообщений RFC 2104 HMAC-SHA256.
pub fn dave_hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        let hashed = dave_sha256(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }
    zeroize_slice(&mut key_block);

    let mut inner = Sha256State::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_digest = inner.finalize();
    zeroize_slice(&mut ipad);

    let mut outer = Sha256State::new();
    outer.update(&opad);
    outer.update(&inner_digest);
    let result = outer.finalize();
    zeroize_slice(&mut opad);
    result
}

/// Фаза извлечения RFC 5869 §2.2 `HKDF-Extract(salt, IKM)` на базе SHA-256.
pub fn dave_hkdf_extract_sha256(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    let zero_salt = [0u8; 32];
    let effective_salt = if salt.is_empty() { &zero_salt } else { salt };
    dave_hmac_sha256(effective_salt, ikm)
}

/// Фаза расширения RFC 5869 §2.3 `HKDF-Expand(PRK, info, L)` / `mlspp::hkdf_expand` (поддерживает 16B и 32B PRK).
pub fn dave_hkdf_expand_sha256(
    prk: &[u8],
    info: &[u8],
    out_len: usize,
) -> Result<Vec<u8>, DaveError> {
    if prk.is_empty() {
        return Err(DaveError::InvalidKeyLength {
            expected: DAVE_AES128_KEY_BYTES,
            actual: 0,
        });
    }
    if out_len > 255 * 32 {
        return Err(DaveError::InvalidOutputLength(out_len));
    }
    if out_len == 0 {
        return Ok(Vec::new());
    }

    let mut okm = Vec::with_capacity(out_len);
    let mut prev_t = [0u8; 32];
    let mut has_prev = false;
    let mut counter: u8 = 0;

    while okm.len() < out_len {
        counter = counter
            .checked_add(1)
            .ok_or(DaveError::InvalidOutputLength(out_len))?;
        let mut msg = Vec::with_capacity((if has_prev { 32 } else { 0 }) + info.len() + 1);
        if has_prev {
            msg.extend_from_slice(&prev_t);
        }
        msg.extend_from_slice(info);
        msg.push(counter);

        prev_t = dave_hmac_sha256(prk, &msg);
        has_prev = true;
        let remaining = out_len - okm.len();
        let to_copy = remaining.min(32);
        okm.extend_from_slice(&prev_t[..to_copy]);
    }
    zeroize_slice(&mut prev_t);
    Ok(okm)
}

/// Кодирование среза переменной длины MLS `VLBytes` по RFC 9420 §2.1.2.
pub fn mls_encode_vlbytes(data: &[u8], out: &mut Vec<u8>) -> Result<(), DaveError> {
    let len = data.len();
    if len < 64 {
        out.push(len as u8);
    } else if len < 16_384 {
        let header = 0x4000u16 | (len as u16);
        out.extend_from_slice(&header.to_be_bytes());
    } else if len <= 0x3FFF_FFFF {
        let header = 0x8000_0000u32 | (len as u32);
        out.extend_from_slice(&header.to_be_bytes());
    } else {
        return Err(DaveError::VlBytesTooLarge(len));
    }
    out.extend_from_slice(data);
    Ok(())
}

/// Деривация `ExpandWithLabel(Secret, Label, Context, Length)` по RFC 9420 §8 (`KdfLabel` с префиксом `"MLS 1.0 "`).
pub fn mls_expand_with_label(
    secret: &[u8],
    label: &str,
    context: &[u8],
    length: u16,
) -> Result<Vec<u8>, DaveError> {
    let mut full_label = Vec::with_capacity(8 + label.len());
    full_label.extend_from_slice(b"MLS 1.0 ");
    full_label.extend_from_slice(label.as_bytes());

    let mut kdf_label = Vec::with_capacity(2 + 2 + full_label.len() + 2 + context.len());
    kdf_label.extend_from_slice(&length.to_be_bytes());
    mls_encode_vlbytes(&full_label, &mut kdf_label)?;
    mls_encode_vlbytes(context, &mut kdf_label)?;

    dave_hkdf_expand_sha256(secret, &kdf_label, length as usize)
}

/// Деривация 32-байтного секрета `DeriveSecret(Secret, Label) = ExpandWithLabel(Secret, Label, "", 32)` по RFC 9420 §8.
pub fn mls_derive_secret(secret: &[u8], label: &str) -> Result<[u8; 32], DaveError> {
    let expanded = mls_expand_with_label(secret, label, &[], 32)?;
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&expanded);
    Ok(arr)
}

/// Деривация узла дерева / рэтчета `DeriveTreeSecret(Secret, Label, Generation, Length)` по RFC 9420 §9.1 / `mlspp`.
pub fn mls_derive_tree_secret(
    secret: &[u8],
    label: &str,
    generation: u32,
    length: u16,
) -> Result<Vec<u8>, DaveError> {
    mls_expand_with_label(secret, label, &generation.to_be_bytes(), length)
}

/// Генерация отображаемого цифрового кода сверки безопасности DAVE (`https://daveprotocol.com/#displayable-codes`).
pub fn generate_dave_displayable_code(
    data: &[u8],
    desired_length: usize,
    group_size: usize,
) -> Result<String, DaveError> {
    if group_size == 0 || group_size > 8 {
        return Err(DaveError::DisplayableCodeInvalid(
            "group_size must be in 1..=8",
        ));
    }
    if desired_length == 0 || !desired_length.is_multiple_of(group_size) {
        return Err(DaveError::DisplayableCodeInvalid(
            "desired_length must be a non-zero multiple of group_size",
        ));
    }
    if data.len() < desired_length {
        return Err(DaveError::DisplayableCodeInvalid(
            "data length is smaller than desired_length",
        ));
    }

    let modulus = 10u64.pow(group_size as u32);
    let mut out = String::with_capacity(desired_length);
    for group_start in (0..desired_length).step_by(group_size) {
        let mut val: u64 = 0;
        for &byte in &data[group_start..group_start + group_size] {
            val = (val << 8) | (byte as u64);
        }
        out.push_str(&format!("{:0width$}", val % modulus, width = group_size));
    }
    Ok(out)
}

fn vec_to_arr32(v: Vec<u8>) -> Result<[u8; 32], DaveError> {
    if v.len() != 32 {
        return Err(DaveError::InvalidOutputLength(v.len()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&v);
    Ok(arr)
}

/// Сериализуемый контекст группы MLS 1.0 (RFC 9420 §8.1 `GroupContext`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaveMlsGroupContext {
    pub group_id: Vec<u8>,
    pub epoch: u64,
    pub tree_hash: [u8; 32],
    pub confirmed_transcript_hash: [u8; 32],
    pub extensions: Vec<u8>,
}

impl DaveMlsGroupContext {
    /// Сериализация `GroupContext` в проводной формат TLS/MLS 1.0 (RFC 9420 §8.1).
    pub fn to_mls_bytes(&self) -> Result<Vec<u8>, DaveError> {
        let mut buf = Vec::with_capacity(128 + self.group_id.len() + self.extensions.len());
        // ProtocolVersion::mls10 = 1 (u16 в RFC 9420 §5.1)
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&DAVE_MLS_CIPHERSUITE_ID.to_be_bytes());
        mls_encode_vlbytes(&self.group_id, &mut buf)?;
        buf.extend_from_slice(&self.epoch.to_be_bytes());
        mls_encode_vlbytes(&self.tree_hash, &mut buf)?;
        mls_encode_vlbytes(&self.confirmed_transcript_hash, &mut buf)?;
        mls_encode_vlbytes(&self.extensions, &mut buf)?;
        Ok(buf)
    }
}

/// Полный набор производных секретов эпохи MLS 1.0 (`mlspp::KeyScheduleEpoch`, `chord_voice.node`).
#[derive(Clone, PartialEq, Eq)]
pub struct DaveMlsEpochSecrets {
    pub epoch: u64,
    pub joiner_secret: [u8; 32],
    pub welcome_secret: [u8; 32],
    pub epoch_secret: [u8; 32],
    pub sender_data_secret: [u8; 32],
    pub encryption_secret: [u8; 32],
    pub exporter_secret: [u8; 32],
    pub epoch_authenticator: [u8; 32],
    pub external_secret: [u8; 32],
    pub confirmation_key: [u8; 32],
    pub membership_key: [u8; 32],
    pub resumption_psk: [u8; 32],
    pub init_secret: [u8; 32],
}

impl Drop for DaveMlsEpochSecrets {
    fn drop(&mut self) {
        zeroize_slice(&mut self.joiner_secret);
        zeroize_slice(&mut self.welcome_secret);
        zeroize_slice(&mut self.epoch_secret);
        zeroize_slice(&mut self.sender_data_secret);
        zeroize_slice(&mut self.encryption_secret);
        zeroize_slice(&mut self.exporter_secret);
        zeroize_slice(&mut self.epoch_authenticator);
        zeroize_slice(&mut self.external_secret);
        zeroize_slice(&mut self.confirmation_key);
        zeroize_slice(&mut self.membership_key);
        zeroize_slice(&mut self.resumption_psk);
        zeroize_slice(&mut self.init_secret);
    }
}

impl std::fmt::Debug for DaveMlsEpochSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaveMlsEpochSecrets")
            .field("epoch", &self.epoch)
            .field("joiner_secret", &"[REDACTED]")
            .field("epoch_secret", &"[REDACTED]")
            .field("exporter_secret", &"[REDACTED]")
            .field("epoch_authenticator", &"[REDACTED]")
            .field("init_secret", &"[REDACTED]")
            .finish()
    }
}

impl DaveMlsEpochSecrets {
    /// Вычисление всех секретов эпохи MLS 1.0 из `joiner_secret`, сериализованного `GroupContext` и `psk_secret`.
    pub fn from_joiner_secret(
        epoch: u64,
        joiner_secret: [u8; 32],
        group_context_bytes: &[u8],
        psk_secret: Option<&[u8]>,
    ) -> Result<Self, DaveError> {
        let zero_psk = [0u8; 32];
        let psk = psk_secret.unwrap_or(&zero_psk);
        let mut member_secret = dave_hkdf_extract_sha256(&joiner_secret, psk);
        let welcome_secret = mls_derive_secret(&member_secret, "welcome")?;
        let epoch_secret = vec_to_arr32(mls_expand_with_label(
            &member_secret,
            "epoch",
            group_context_bytes,
            32,
        )?)?;
        zeroize_slice(&mut member_secret);

        let sender_data_secret = mls_derive_secret(&epoch_secret, "sender data")?;
        let encryption_secret = mls_derive_secret(&epoch_secret, "encryption")?;
        let exporter_secret = mls_derive_secret(&epoch_secret, "exporter")?;
        let epoch_authenticator = mls_derive_secret(&epoch_secret, "authentication")?;
        let external_secret = mls_derive_secret(&epoch_secret, "external")?;
        let confirmation_key = mls_derive_secret(&epoch_secret, "confirm")?;
        let membership_key = mls_derive_secret(&epoch_secret, "membership")?;
        let resumption_psk = mls_derive_secret(&epoch_secret, "resumption")?;
        let init_secret = mls_derive_secret(&epoch_secret, "init")?;

        Ok(Self {
            epoch,
            joiner_secret,
            welcome_secret,
            epoch_secret,
            sender_data_secret,
            encryption_secret,
            exporter_secret,
            epoch_authenticator,
            external_secret,
            confirmation_key,
            membership_key,
            resumption_psk,
            init_secret,
        })
    }

    /// Вычисление `confirmation_tag = MAC(confirmation_key, confirmed_transcript_hash)` (RFC 9420 §8.2).
    pub fn compute_confirmation_tag(&self, confirmed_transcript_hash: &[u8; 32]) -> [u8; 32] {
        dave_hmac_sha256(&self.confirmation_key, confirmed_transcript_hash)
    }

    /// Константно-временная проверка `confirmation_tag`.
    pub fn verify_confirmation_tag(
        &self,
        confirmed_transcript_hash: &[u8; 32],
        candidate_tag: &[u8],
    ) -> Result<(), DaveError> {
        let expected = self.compute_confirmation_tag(confirmed_transcript_hash);
        if dave_ct_eq(&expected, candidate_tag) {
            Ok(())
        } else {
            Err(DaveError::ConfirmationTagMismatch)
        }
    }

    /// Экспорт секрета `MLS-Exporter(Label, Context, Length)` по RFC 9420 §8.5 (`KeyScheduleEpoch::do_export`).
    pub fn mls_export(
        &self,
        label: &str,
        context: &[u8],
        length: u16,
    ) -> Result<Vec<u8>, DaveError> {
        let mut derived = mls_derive_secret(&self.exporter_secret, label)?;
        let context_hash = dave_sha256(context);
        let exported = mls_expand_with_label(&derived, "exported", &context_hash, length)?;
        zeroize_slice(&mut derived);
        Ok(exported)
    }

    /// Экспорт 16-байтного базового медиа-ключа отправителя DAVE (`"Discord Secure Frames v0"`, `user_id` LE64).
    pub fn export_user_media_secret(
        &self,
        user_id: u64,
    ) -> Result<[u8; DAVE_AES128_KEY_BYTES], DaveError> {
        let raw = self.mls_export(
            DAVE_USER_MEDIA_KEY_BASE_LABEL,
            &user_id.to_le_bytes(),
            DAVE_AES128_KEY_BYTES as u16,
        )?;
        let mut key = [0u8; DAVE_AES128_KEY_BYTES];
        key.copy_from_slice(&raw);
        Ok(key)
    }

    /// 30-значный код приватности голосового вызова на базе `epoch_authenticator` (`generate_displayable_code(..., 30, 5)`).
    pub fn voice_privacy_code(&self) -> Result<String, DaveError> {
        generate_dave_displayable_code(&self.epoch_authenticator, 30, 5)
    }

    /// Форматированный 30-значный код приватности в виде 6 групп по 5 цифр для UI.
    pub fn formatted_voice_privacy_code(&self) -> Result<String, DaveError> {
        let raw = self.voice_privacy_code()?;
        let groups: Vec<&str> = (0..30).step_by(5).map(|i| &raw[i..i + 5]).collect();
        Ok(groups.join(" "))
    }
}

/// Машина расписания ключей эпох MLS 1.0 для Discord DAVE v1 (`KeyScheduleEpoch`).
#[derive(Clone)]
pub struct DaveMlsKeySchedule {
    current: DaveMlsEpochSecrets,
}

impl std::fmt::Debug for DaveMlsKeySchedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaveMlsKeySchedule")
            .field("epoch", &self.current.epoch)
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}

impl DaveMlsKeySchedule {
    fn derive_epoch(
        init_secret: &[u8; 32],
        commit_secret: &[u8],
        group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
    ) -> Result<DaveMlsEpochSecrets, DaveError> {
        let gc_bytes = group_context.to_mls_bytes()?;
        let mut extracted = dave_hkdf_extract_sha256(init_secret, commit_secret);
        let joiner_secret =
            vec_to_arr32(mls_expand_with_label(&extracted, "joiner", &gc_bytes, 32)?)?;
        zeroize_slice(&mut extracted);
        DaveMlsEpochSecrets::from_joiner_secret(
            group_context.epoch,
            joiner_secret,
            &gc_bytes,
            psk_secret,
        )
    }

    /// Инициализация расписания ключей начальной эпохи из `init_secret`, `commit_secret` и `GroupContext`.
    pub fn from_initial_epoch(
        init_secret: [u8; 32],
        commit_secret: &[u8],
        group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
    ) -> Result<Self, DaveError> {
        let current = Self::derive_epoch(&init_secret, commit_secret, group_context, psk_secret)?;
        Ok(Self { current })
    }

    /// Инициализация расписания ключей при входе в голосовой канал по сообщению MLS `Welcome` (op:30).
    pub fn from_welcome_joiner(
        joiner_secret: [u8; 32],
        group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
    ) -> Result<Self, DaveError> {
        let gc_bytes = group_context.to_mls_bytes()?;
        let current = DaveMlsEpochSecrets::from_joiner_secret(
            group_context.epoch,
            joiner_secret,
            &gc_bytes,
            psk_secret,
        )?;
        Ok(Self { current })
    }

    pub fn current_epoch(&self) -> u64 {
        self.current.epoch
    }

    pub fn current_secrets(&self) -> &DaveMlsEpochSecrets {
        &self.current
    }

    /// Переход к следующей эпохе MLS 1.0 (`init_secret[n] + commit_secret[n+1] -> epoch[n+1]`).
    pub fn advance_epoch(
        &mut self,
        commit_secret: &[u8],
        next_group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
    ) -> Result<&DaveMlsEpochSecrets, DaveError> {
        let next_init = self.current.init_secret;
        self.current =
            Self::derive_epoch(&next_init, commit_secret, next_group_context, psk_secret)?;
        Ok(&self.current)
    }

    /// Экспорт персонального хеш-рэтчета отправителя `user_id` для текущей эпохи.
    pub fn export_sender_ratchet(&self, user_id: u64) -> Result<DaveHashRatchet, DaveError> {
        let media_secret = self.current.export_user_media_secret(user_id)?;
        DaveHashRatchet::new(media_secret.to_vec())
    }
}

/// Однонаправленный хеш-рэтчет отправителя MLS 1.0 (`mlspp::HashRatchet` / `discord::dave::MlsKeyRatchet`).
#[derive(Clone)]
pub struct DaveHashRatchet {
    next_secret: Vec<u8>,
    next_generation: u32,
    oldest_generation: u32,
    exhausted: bool,
    cache: BTreeMap<u32, ([u8; DAVE_AES128_KEY_BYTES], [u8; DAVE_AES128_NONCE_BYTES])>,
}

impl Drop for DaveHashRatchet {
    fn drop(&mut self) {
        zeroize_slice(&mut self.next_secret);
        for (_, (k, n)) in self.cache.iter_mut() {
            zeroize_slice(k);
            zeroize_slice(n);
        }
    }
}

impl std::fmt::Debug for DaveHashRatchet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaveHashRatchet")
            .field("next_secret", &"[REDACTED]")
            .field("next_generation", &self.next_generation)
            .field("oldest_generation", &self.oldest_generation)
            .field("cached_generations", &self.cache.len())
            .finish()
    }
}

impl DaveHashRatchet {
    pub fn new(base_secret: impl Into<Vec<u8>>) -> Result<Self, DaveError> {
        let secret = base_secret.into();
        if secret.is_empty() {
            return Err(DaveError::InvalidKeyLength {
                expected: DAVE_AES128_KEY_BYTES,
                actual: 0,
            });
        }
        Ok(Self {
            next_secret: secret,
            next_generation: 0,
            oldest_generation: 0,
            exhausted: false,
            cache: BTreeMap::new(),
        })
    }

    pub fn next_generation(&self) -> u32 {
        self.next_generation
    }

    pub fn oldest_generation(&self) -> u32 {
        self.oldest_generation
    }

    fn step_once(&mut self) -> Result<(), DaveError> {
        if self.exhausted {
            return Err(DaveError::KeyExpired {
                generation: u32::MAX,
                oldest_available: self.oldest_generation,
            });
        }
        let generation = self.next_generation;
        let key_vec = mls_derive_tree_secret(
            &self.next_secret,
            "key",
            generation,
            DAVE_AES128_KEY_BYTES as u16,
        )?;
        let nonce_vec = mls_derive_tree_secret(
            &self.next_secret,
            "nonce",
            generation,
            DAVE_AES128_NONCE_BYTES as u16,
        )?;
        let next_secret = mls_derive_tree_secret(&self.next_secret, "secret", generation, 32)?;

        let mut key = [0u8; DAVE_AES128_KEY_BYTES];
        key.copy_from_slice(&key_vec);
        let mut nonce = [0u8; DAVE_AES128_NONCE_BYTES];
        nonce.copy_from_slice(&nonce_vec);

        zeroize_slice(&mut self.next_secret);
        self.next_secret = next_secret;
        if generation == u32::MAX {
            self.exhausted = true;
        } else {
            self.next_generation = generation + 1;
        }
        self.cache.insert(generation, (key, nonce));
        Ok(())
    }

    /// Получение (или продвижение вперёд с кэшированием промежуточных поколений) ключа и базового nonce поколения.
    pub fn get(
        &mut self,
        generation: u32,
    ) -> Result<([u8; DAVE_AES128_KEY_BYTES], [u8; DAVE_AES128_NONCE_BYTES]), DaveError> {
        if let Some(&cached) = self.cache.get(&generation) {
            return Ok(cached);
        }
        if self.exhausted || generation < self.next_generation {
            return Err(DaveError::KeyExpired {
                generation,
                oldest_available: self.oldest_generation,
            });
        }
        if generation - self.next_generation > DAVE_MAX_GENERATION_GAP {
            return Err(DaveError::GenerationGapExceeded {
                requested: generation,
                current: self.next_generation,
                max_gap: DAVE_MAX_GENERATION_GAP,
            });
        }
        for _ in self.next_generation..=generation {
            self.step_once()?;
        }
        self.cache
            .get(&generation)
            .copied()
            .ok_or(DaveError::KeyExpired {
                generation,
                oldest_available: self.oldest_generation,
            })
    }

    /// Удаление ключа конкретного поколения с затиранием памяти (Forward Secrecy).
    pub fn delete_key(&mut self, generation: u32) {
        if let Some((mut k, mut n)) = self.cache.remove(&generation) {
            zeroize_slice(&mut k);
            zeroize_slice(&mut n);
        }
        self.refresh_oldest_generation();
    }

    /// Удаление всех ключей поколений строго меньше `up_to_exclusive` (Forward Secrecy).
    pub fn delete_keys_up_to(&mut self, up_to_exclusive: u32) {
        let expired: Vec<u32> = self
            .cache
            .range(..up_to_exclusive)
            .map(|(&g, _)| g)
            .collect();
        for g in expired {
            if let Some((mut k, mut n)) = self.cache.remove(&g) {
                zeroize_slice(&mut k);
                zeroize_slice(&mut n);
            }
        }
        if up_to_exclusive > self.oldest_generation {
            self.oldest_generation = up_to_exclusive;
        }
        self.refresh_oldest_generation();
    }

    fn refresh_oldest_generation(&mut self) {
        if let Some(&first_cached) = self.cache.keys().next() {
            if first_cached < self.oldest_generation {
                self.oldest_generation = first_cached;
            }
        } else if self.next_generation > self.oldest_generation {
            self.oldest_generation = self.next_generation;
        }
    }
}

/// Восстановление полного 32-битного поколения из младших 8 бит относительно `oldest` (`compute_wrapped_generation`).
pub fn dave_compute_wrapped_generation(oldest: u32, generation_u8: u32) -> u32 {
    let delta = ((generation_u8 & 0xFF) as u8).wrapping_sub((oldest & 0xFF) as u8);
    oldest.saturating_add(delta as u32)
}

/// Формирование монотонного 64-битного счётчика кадров из развёрнутого поколения и 24 младших бит `truncated_nonce`.
pub fn dave_compute_wrapped_big_nonce(generation: u32, truncated_nonce: u32) -> u64 {
    let low_mask = (1u64 << DAVE_RATCHET_GENERATION_SHIFT_BITS) - 1;
    let masked_nonce = (truncated_nonce as u64) & low_mask;
    ((generation as u64) << DAVE_RATCHET_GENERATION_SHIFT_BITS) | masked_nonce
}

fn dave_leb128_size(mut value: u64) -> usize {
    let mut size = 1usize;
    while value >= 0x80 {
        size += 1;
        value >>= 7;
    }
    size
}

fn dave_write_leb128(mut value: u64, out: &mut [u8]) -> usize {
    let mut i = 0;
    while value >= 0x80 {
        out[i] = 0x80 | ((value & 0x7F) as u8);
        i += 1;
        value >>= 7;
    }
    out[i] = value as u8;
    i + 1
}

fn dave_read_leb128_u32(slice: &[u8]) -> Option<(u32, usize)> {
    let mut value = 0u64;
    let mut shift = 0u32;
    for (idx, &byte) in slice.iter().enumerate() {
        if idx >= 5 {
            return None;
        }
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            if value > u32::MAX as u64 {
                return None;
            }
            return Some((value as u32, idx + 1));
        }
        shift += 7;
    }
    None
}

/// Умножение в поле Галуа `GF(2^128)` для GHASH по NIST SP 800-38D §6.3 (константное время).
fn ghash_mul_block(x: u128, y: u128) -> u128 {
    let mut z = 0u128;
    let mut v = y;
    for i in 0..128 {
        let bit = (x >> (127 - i)) & 1;
        let mask = 0u128.wrapping_sub(bit);
        z ^= v & mask;
        let lsb = v & 1;
        let r_mask = 0u128.wrapping_sub(lsb);
        v = (v >> 1) ^ (0xE100_0000_0000_0000_0000_0000_0000_0000_u128 & r_mask);
    }
    z
}

fn ghash_update_padded(acc: &mut u128, h: u128, data: &[u8]) {
    let mut chunks = data.chunks_exact(16);
    for chunk in &mut chunks {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        *acc = ghash_mul_block(*acc ^ u128::from_be_bytes(block), h);
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let mut padded = [0u8; 16];
        padded[..rem.len()].copy_from_slice(rem);
        *acc = ghash_mul_block(*acc ^ u128::from_be_bytes(padded), h);
    }
}

/// Шифр AES-128-GCM с 96-битным nonce и 64-битным (8-байтным) усечённым тегом DAVE v1 (NIST SP 800-38D).
#[derive(Clone)]
pub struct Aes128GcmTruncated8 {
    cipher: aes_gcm::aes::Aes128,
    ghash_h: u128,
}

impl Aes128GcmTruncated8 {
    pub fn new(key: &[u8]) -> Result<Self, DaveError> {
        use aes_gcm::aes::cipher::{BlockCipherEncrypt, KeyInit, array::Array};
        if key.len() != DAVE_AES128_KEY_BYTES {
            return Err(DaveError::InvalidKeyLength {
                expected: DAVE_AES128_KEY_BYTES,
                actual: key.len(),
            });
        }
        let mut k_arr = [0u8; DAVE_AES128_KEY_BYTES];
        k_arr.copy_from_slice(key);
        let cipher = aes_gcm::aes::Aes128::new(&Array::from(k_arr));
        zeroize_slice(&mut k_arr);

        let mut h_block = Array::from([0u8; 16]);
        cipher.encrypt_block(&mut h_block);
        let h_bytes: [u8; 16] = h_block.into();
        let ghash_h = u128::from_be_bytes(h_bytes);

        Ok(Self { cipher, ghash_h })
    }

    fn apply_gctr(&self, nonce: &[u8; DAVE_AES128_NONCE_BYTES], buffer: &mut [u8]) {
        use aes_gcm::aes::cipher::{BlockCipherEncrypt, array::Array};
        let mut counter = 2u32;
        for chunk in buffer.chunks_mut(16) {
            let mut cb = [0u8; 16];
            cb[..12].copy_from_slice(nonce);
            cb[12..16].copy_from_slice(&counter.to_be_bytes());
            counter = counter.wrapping_add(1);

            let mut keystream = Array::from(cb);
            self.cipher.encrypt_block(&mut keystream);
            let ks_bytes: [u8; 16] = keystream.into();
            for (dst, &ks) in chunk.iter_mut().zip(ks_bytes.iter()) {
                *dst ^= ks;
            }
        }
    }

    fn compute_full_tag(
        &self,
        nonce: &[u8; DAVE_AES128_NONCE_BYTES],
        aad: &[u8],
        ciphertext: &[u8],
    ) -> [u8; 16] {
        use aes_gcm::aes::cipher::{BlockCipherEncrypt, array::Array};
        let mut j0 = [0u8; 16];
        j0[..12].copy_from_slice(nonce);
        j0[15] = 1;
        let mut mask_block = Array::from(j0);
        self.cipher.encrypt_block(&mut mask_block);
        let mask_bytes: [u8; 16] = mask_block.into();

        let mut acc = 0u128;
        ghash_update_padded(&mut acc, self.ghash_h, aad);
        ghash_update_padded(&mut acc, self.ghash_h, ciphertext);

        let mut lengths_block = [0u8; 16];
        lengths_block[..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
        lengths_block[8..16].copy_from_slice(&((ciphertext.len() as u64) * 8).to_be_bytes());
        acc = ghash_mul_block(acc ^ u128::from_be_bytes(lengths_block), self.ghash_h);

        let tag_u128 = acc ^ u128::from_be_bytes(mask_bytes);
        tag_u128.to_be_bytes()
    }

    pub fn encrypt_in_place_detached(
        &self,
        nonce: &[u8; DAVE_AES128_NONCE_BYTES],
        aad: &[u8],
        buffer: &mut [u8],
    ) -> [u8; DAVE_TRUNCATED_TAG_BYTES] {
        self.apply_gctr(nonce, buffer);
        let full_tag = self.compute_full_tag(nonce, aad, buffer);
        let mut tag8 = [0u8; DAVE_TRUNCATED_TAG_BYTES];
        tag8.copy_from_slice(&full_tag[..DAVE_TRUNCATED_TAG_BYTES]);
        tag8
    }

    pub fn decrypt_in_place_detached(
        &self,
        nonce: &[u8; DAVE_AES128_NONCE_BYTES],
        aad: &[u8],
        buffer: &mut [u8],
        tag: &[u8; DAVE_TRUNCATED_TAG_BYTES],
    ) -> Result<(), DaveError> {
        let expected_full = self.compute_full_tag(nonce, aad, buffer);
        if !dave_ct_eq(&expected_full[..DAVE_TRUNCATED_TAG_BYTES], tag) {
            return Err(DaveError::AeadAuthenticationFailed);
        }
        self.apply_gctr(nonce, buffer);
        Ok(())
    }
}

/// Шифратор исходящих аудио-фреймов Opus по спецификации Discord DAVE v1 (`[ct][8B tag][leb128(nonce)][size][0xFAFA]`).
pub struct DaveFrameEncryptor {
    ratchet: Option<DaveHashRatchet>,
    cipher: Option<Aes128GcmTruncated8>,
    current_generation: u32,
    truncated_nonce: u32,
}

impl DaveFrameEncryptor {
    pub fn new(ratchet: DaveHashRatchet) -> Self {
        Self {
            ratchet: Some(ratchet),
            cipher: None,
            current_generation: 0,
            truncated_nonce: 0,
        }
    }

    pub fn set_key_ratchet(&mut self, ratchet: DaveHashRatchet) {
        self.ratchet = Some(ratchet);
        self.cipher = None;
        self.current_generation = 0;
        self.truncated_nonce = 0;
    }

    pub fn set_truncated_nonce(&mut self, nonce: u32) {
        self.truncated_nonce = nonce;
    }

    pub fn current_truncated_nonce(&self) -> u32 {
        self.truncated_nonce
    }

    pub fn encrypt_opus(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if opus_frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }

        let ratchet = self
            .ratchet
            .as_mut()
            .ok_or(DaveError::MissingKeyRatchet { user_id: 0 })?;

        self.truncated_nonce = self.truncated_nonce.wrapping_add(1);
        let next_gen = dave_compute_wrapped_generation(
            self.current_generation,
            self.truncated_nonce >> DAVE_RATCHET_GENERATION_SHIFT_BITS,
        );

        if next_gen != self.current_generation || self.cipher.is_none() {
            let (mut key, _base_nonce) = ratchet.get(next_gen)?;
            let aead = Aes128GcmTruncated8::new(&key)?;
            zeroize_slice(&mut key);
            if next_gen > 0 {
                ratchet.delete_keys_up_to(next_gen);
            }
            self.current_generation = next_gen;
            self.cipher = Some(aead);
        }

        let cipher = self
            .cipher
            .as_ref()
            .ok_or(DaveError::MalformedFrame("missing outbound cipher"))?;

        let mut nonce_buf = [0u8; DAVE_AES128_NONCE_BYTES];
        nonce_buf[DAVE_TRUNCATED_SYNC_NONCE_OFFSET
            ..DAVE_TRUNCATED_SYNC_NONCE_OFFSET + DAVE_TRUNCATED_SYNC_NONCE_BYTES]
            .copy_from_slice(&self.truncated_nonce.to_le_bytes());

        let mut ciphertext = opus_frame.to_vec();
        let tag8 = cipher.encrypt_in_place_detached(&nonce_buf, &[], &mut ciphertext);

        let nonce_leb_len = dave_leb128_size(self.truncated_nonce as u64);
        let mut nonce_leb = [0u8; 5];
        let written = dave_write_leb128(self.truncated_nonce as u64, &mut nonce_leb);
        debug_assert_eq!(written, nonce_leb_len);

        let supplemental_total = DAVE_MIN_SUPPLEMENTAL_BYTES + nonce_leb_len;
        if supplemental_total > u8::MAX as usize {
            return Err(DaveError::MalformedFrame("supplemental footer overflow"));
        }

        let mut out = Vec::with_capacity(ciphertext.len() + supplemental_total);
        out.extend_from_slice(&ciphertext);
        out.extend_from_slice(&tag8);
        out.extend_from_slice(&nonce_leb[..nonce_leb_len]);
        out.push(supplemental_total as u8);
        out.extend_from_slice(&DAVE_MARKER_BYTES);
        Ok(out)
    }
}

const DAVE_RETAINED_GENERATIONS: u32 = 4;
const DAVE_MAX_RETAINED_EPOCH_MANAGERS: usize = 2;

struct DaveCipherManager {
    ratchet: DaveHashRatchet,
    ciphers: BTreeMap<u32, Aes128GcmTruncated8>,
    oldest_generation: u32,
    newest_generation: u32,
    newest_processed_nonce: Option<u64>,
    missing_nonces: std::collections::VecDeque<u64>,
}

impl DaveCipherManager {
    fn new(ratchet: DaveHashRatchet) -> Self {
        Self {
            ratchet,
            ciphers: BTreeMap::new(),
            oldest_generation: 0,
            newest_generation: 0,
            newest_processed_nonce: None,
            missing_nonces: std::collections::VecDeque::new(),
        }
    }

    fn compute_generation(&self, gen_u8: u32) -> u32 {
        dave_compute_wrapped_generation(self.oldest_generation, gen_u8)
    }

    fn can_process_nonce(&self, generation: u32, truncated_nonce: u32) -> bool {
        let Some(newest) = self.newest_processed_nonce else {
            return true;
        };
        let big_nonce = dave_compute_wrapped_big_nonce(generation, truncated_nonce);
        big_nonce > newest || self.missing_nonces.contains(&big_nonce)
    }

    fn get_or_derive_cipher(&mut self, generation: u32) -> Result<&Aes128GcmTruncated8, DaveError> {
        if generation < self.oldest_generation {
            return Err(DaveError::KeyExpired {
                generation,
                oldest_available: self.oldest_generation,
            });
        }
        if generation
            > self
                .newest_generation
                .saturating_add(DAVE_MAX_GENERATION_GAP)
        {
            return Err(DaveError::GenerationGapExceeded {
                requested: generation,
                current: self.newest_generation,
                max_gap: DAVE_MAX_GENERATION_GAP,
            });
        }
        if !self.ciphers.contains_key(&generation) {
            let (mut key, _nonce) = self.ratchet.get(generation)?;
            let cipher = Aes128GcmTruncated8::new(&key)?;
            zeroize_slice(&mut key);
            self.ciphers.insert(generation, cipher);
        }
        self.ciphers.get(&generation).ok_or(DaveError::KeyExpired {
            generation,
            oldest_available: self.oldest_generation,
        })
    }

    fn record_success(&mut self, generation: u32, truncated_nonce: u32) {
        let big_nonce = dave_compute_wrapped_big_nonce(generation, truncated_nonce);
        match self.newest_processed_nonce {
            None => {
                let oldest_allowed = big_nonce.saturating_sub(DAVE_MAX_MISSING_NONCES);
                for missing in oldest_allowed..big_nonce {
                    self.missing_nonces.push_back(missing);
                }
                self.newest_processed_nonce = Some(big_nonce);
            }
            Some(newest) if big_nonce > newest => {
                let oldest_allowed = big_nonce.saturating_sub(DAVE_MAX_MISSING_NONCES);
                while let Some(&front) = self.missing_nonces.front() {
                    if front < oldest_allowed {
                        self.missing_nonces.pop_front();
                    } else {
                        break;
                    }
                }
                let start = oldest_allowed.max(newest.saturating_add(1));
                for missing in start..big_nonce {
                    self.missing_nonces.push_back(missing);
                }
                self.newest_processed_nonce = Some(big_nonce);
            }
            Some(_) => {
                if let Some(pos) = self.missing_nonces.iter().position(|&n| n == big_nonce) {
                    self.missing_nonces.remove(pos);
                }
            }
        }

        if generation > self.newest_generation {
            self.newest_generation = generation;
            if generation > DAVE_RETAINED_GENERATIONS {
                let prune_before = generation - DAVE_RETAINED_GENERATIONS;
                self.oldest_generation = self.oldest_generation.max(prune_before);
                self.ratchet.delete_keys_up_to(self.oldest_generation);
                let stale: Vec<u32> = self
                    .ciphers
                    .range(..self.oldest_generation)
                    .map(|(&g, _)| g)
                    .collect();
                for g in stale {
                    self.ciphers.remove(&g);
                }
            }
        }
    }
}

struct ParsedDaveFrame<'a> {
    ciphertext: &'a [u8],
    tag: [u8; DAVE_TRUNCATED_TAG_BYTES],
    truncated_nonce: u32,
}

fn parse_dave_encrypted_frame(frame: &[u8]) -> Option<ParsedDaveFrame<'_>> {
    // Минимальный размер зашифрованного кадра: 8B tag + 1B leb128 nonce + 1B size + 2B 0xFAFA = 12 байт
    if frame.len() < DAVE_MIN_SUPPLEMENTAL_BYTES + 1 {
        return None;
    }
    if frame[frame.len() - 2..] != DAVE_MARKER_BYTES {
        return None;
    }
    let supplemental_size = frame[frame.len() - 3] as usize;
    if supplemental_size < DAVE_MIN_SUPPLEMENTAL_BYTES + 1 || supplemental_size > frame.len() {
        return None;
    }

    let ct_len = frame.len() - supplemental_size;
    let supplemental = &frame[ct_len..];
    let mut tag = [0u8; DAVE_TRUNCATED_TAG_BYTES];
    tag.copy_from_slice(&supplemental[..DAVE_TRUNCATED_TAG_BYTES]);

    let meta_slice = &supplemental[DAVE_TRUNCATED_TAG_BYTES..supplemental.len() - 3];
    let (truncated_nonce, nonce_len) = dave_read_leb128_u32(meta_slice)?;
    // Для Opus unencrypted_ranges пусты, поэтому meta_slice содержит ровно LEB128 nonce
    if nonce_len != meta_slice.len() {
        return None;
    }

    Some(ParsedDaveFrame {
        ciphertext: &frame[..ct_len],
        tag,
        truncated_nonce,
    })
}

/// Дешифратор входящих аудио-фреймов Opus участника с защитой от повторов и плавной ротацией рэтчетов.
pub struct DaveFrameDecryptor {
    managers: std::collections::VecDeque<DaveCipherManager>,
    passthrough_enabled: bool,
}

impl DaveFrameDecryptor {
    pub fn new(ratchet: DaveHashRatchet) -> Self {
        let mut managers =
            std::collections::VecDeque::with_capacity(DAVE_MAX_RETAINED_EPOCH_MANAGERS);
        managers.push_back(DaveCipherManager::new(ratchet));
        Self {
            managers,
            passthrough_enabled: false,
        }
    }

    pub fn set_passthrough(&mut self, enabled: bool) {
        self.passthrough_enabled = enabled;
    }

    /// Переход на рэтчет новой эпохи с удержанием предыдущего менеджера для дошифровки летящих в сети пакетов.
    pub fn transition_to_key_ratchet(&mut self, ratchet: DaveHashRatchet) {
        if self.managers.len() >= DAVE_MAX_RETAINED_EPOCH_MANAGERS {
            self.managers.pop_front();
        }
        self.managers.push_back(DaveCipherManager::new(ratchet));
    }

    pub fn decrypt_opus(&mut self, encrypted_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        if encrypted_frame == DAVE_OPUS_SILENCE_PACKET {
            return Ok(DAVE_OPUS_SILENCE_PACKET.to_vec());
        }

        let Some(parsed) = parse_dave_encrypted_frame(encrypted_frame) else {
            if self.passthrough_enabled {
                return Ok(encrypted_frame.to_vec());
            }
            return Err(DaveError::UnencryptedFrameRejected);
        };

        let mut nonce_buf = [0u8; DAVE_AES128_NONCE_BYTES];
        nonce_buf[DAVE_TRUNCATED_SYNC_NONCE_OFFSET
            ..DAVE_TRUNCATED_SYNC_NONCE_OFFSET + DAVE_TRUNCATED_SYNC_NONCE_BYTES]
            .copy_from_slice(&parsed.truncated_nonce.to_le_bytes());

        let gen_lsb = parsed.truncated_nonce >> DAVE_RATCHET_GENERATION_SHIFT_BITS;
        let mut saw_replay_rejection = None;
        let mut last_ratchet_err = None;

        // Проверяем от новейшего рэтчета эпохи к предыдущему (для бесшовной смены эпох op:21/22)
        for manager in self.managers.iter_mut().rev() {
            let generation = manager.compute_generation(gen_lsb);
            if !manager.can_process_nonce(generation, parsed.truncated_nonce) {
                saw_replay_rejection = Some((generation, parsed.truncated_nonce));
                continue;
            }
            let cipher = match manager.get_or_derive_cipher(generation) {
                Ok(c) => c,
                Err(e) => {
                    last_ratchet_err = Some(e);
                    continue;
                }
            };
            let mut candidate = parsed.ciphertext.to_vec();
            if cipher
                .decrypt_in_place_detached(&nonce_buf, &[], &mut candidate, &parsed.tag)
                .is_ok()
            {
                manager.record_success(generation, parsed.truncated_nonce);
                return Ok(candidate);
            }
        }

        if let Some((generation, truncated_nonce)) = saw_replay_rejection {
            return Err(DaveError::ReplayOrExpiredNonce {
                generation,
                truncated_nonce,
            });
        }
        if let Some(err) = last_ratchet_err {
            return Err(err);
        }
        Err(DaveError::AeadAuthenticationFailed)
    }
}

struct PreparedDaveTransition {
    next_schedule: DaveMlsKeySchedule,
    member_user_ids: Vec<u64>,
}

/// Многопользовательская сессия Discord DAVE E2EE MLS v1 для голосового канала.
pub struct DaveVoiceSession {
    self_user_id: u64,
    protocol_version: u16,
    transport_mode: DaveTransportCipherMode,
    key_schedule: DaveMlsKeySchedule,
    encryptor: DaveFrameEncryptor,
    decryptors: BTreeMap<u64, DaveFrameDecryptor>,
    pending_transitions: BTreeMap<u16, PreparedDaveTransition>,
}

impl DaveVoiceSession {
    pub fn new(
        self_user_id: u64,
        key_schedule: DaveMlsKeySchedule,
        member_user_ids: &[u64],
        transport_mode: DaveTransportCipherMode,
    ) -> Result<Self, DaveError> {
        let self_ratchet = key_schedule.export_sender_ratchet(self_user_id)?;
        let encryptor = DaveFrameEncryptor::new(self_ratchet);
        let mut decryptors = BTreeMap::new();
        for &uid in member_user_ids {
            if uid != self_user_id {
                let peer_ratchet = key_schedule.export_sender_ratchet(uid)?;
                decryptors.insert(uid, DaveFrameDecryptor::new(peer_ratchet));
            }
        }

        Ok(Self {
            self_user_id,
            protocol_version: DAVE_PROTOCOL_VERSION_V1,
            transport_mode,
            key_schedule,
            encryptor,
            decryptors,
            pending_transitions: BTreeMap::new(),
        })
    }

    pub fn current_epoch(&self) -> u64 {
        self.key_schedule.current_epoch()
    }

    pub fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    pub fn transport_mode(&self) -> DaveTransportCipherMode {
        self.transport_mode
    }

    /// Защита от downgrade-атак (`voice-deep.md` §3): запрещает сброс в `protocol_version = 0` после старта E2EE.
    pub fn negotiate_protocol_version(&mut self, requested_version: u16) -> Result<(), DaveError> {
        if requested_version < self.protocol_version {
            return Err(DaveError::DowngradeRejected {
                current_version: self.protocol_version,
                requested_version,
            });
        }
        self.protocol_version = requested_version;
        Ok(())
    }

    pub fn voice_privacy_code(&self) -> Result<String, DaveError> {
        self.key_schedule.current_secrets().voice_privacy_code()
    }

    pub fn add_or_update_member(&mut self, user_id: u64) -> Result<(), DaveError> {
        if user_id == self.self_user_id {
            let self_ratchet = self.key_schedule.export_sender_ratchet(self.self_user_id)?;
            self.encryptor.set_key_ratchet(self_ratchet);
            return Ok(());
        }
        let ratchet = self.key_schedule.export_sender_ratchet(user_id)?;
        match self.decryptors.get_mut(&user_id) {
            Some(dec) => dec.transition_to_key_ratchet(ratchet),
            None => {
                self.decryptors
                    .insert(user_id, DaveFrameDecryptor::new(ratchet));
            }
        }
        Ok(())
    }

    pub fn remove_member(&mut self, user_id: u64) {
        self.decryptors.remove(&user_id);
    }

    /// Фаза 1 смены эпохи MLS (`op:21 PrepareTransition` / `op:29 PrepareCommitTransition`).
    pub fn prepare_epoch_transition(
        &mut self,
        transition_id: u16,
        commit_secret: &[u8],
        next_group_context: &DaveMlsGroupContext,
        psk_secret: Option<&[u8]>,
        member_user_ids: &[u64],
    ) -> Result<(), DaveError> {
        let mut next_schedule = self.key_schedule.clone();
        next_schedule.advance_epoch(commit_secret, next_group_context, psk_secret)?;
        self.pending_transitions.insert(
            transition_id,
            PreparedDaveTransition {
                next_schedule,
                member_user_ids: member_user_ids.to_vec(),
            },
        );
        Ok(())
    }

    /// Фаза 2 смены эпохи MLS (`op:22 ExecuteTransition`): активирует новые рэтчеты с сохранением окна приёма старой эпохи.
    pub fn execute_transition(&mut self, transition_id: u16) -> Result<u64, DaveError> {
        let prepared = self
            .pending_transitions
            .remove(&transition_id)
            .ok_or(DaveError::TransitionNotPrepared { transition_id })?;

        self.key_schedule = prepared.next_schedule;
        let new_self_ratchet = self.key_schedule.export_sender_ratchet(self.self_user_id)?;
        self.encryptor.set_key_ratchet(new_self_ratchet);

        self.decryptors
            .retain(|uid, _| prepared.member_user_ids.contains(uid));
        for &uid in &prepared.member_user_ids {
            if uid != self.self_user_id {
                let peer_ratchet = self.key_schedule.export_sender_ratchet(uid)?;
                match self.decryptors.get_mut(&uid) {
                    Some(dec) => dec.transition_to_key_ratchet(peer_ratchet),
                    None => {
                        self.decryptors
                            .insert(uid, DaveFrameDecryptor::new(peer_ratchet));
                    }
                }
            }
        }

        Ok(self.key_schedule.current_epoch())
    }

    pub fn encrypt_opus(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, DaveError> {
        self.encryptor.encrypt_opus(opus_frame)
    }

    pub fn decrypt_opus(
        &mut self,
        sender_user_id: u64,
        dave_frame: &[u8],
    ) -> Result<Vec<u8>, DaveError> {
        let decryptor =
            self.decryptors
                .get_mut(&sender_user_id)
                .ok_or(DaveError::MissingKeyRatchet {
                    user_id: sender_user_id,
                })?;
        decryptor.decrypt_opus(dave_frame)
    }
}

impl DiscordVoicePacket {
    /// Сборка заголовка RTP v2 (`PT=120`) с расширением RFC 5285 `0xBEDE` (`voice-deep.md` §2.3).
    pub fn make_rtp_header_with_bede_ext(
        seq: u16,
        timestamp: u32,
        ssrc: u32,
        extension_words: &[[u8; 4]],
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(12 + 4 + extension_words.len() * 4);
        let first_byte = if extension_words.is_empty() {
            0x80
        } else {
            0x90 // V=2, P=0, X=1, CC=0
        };
        header.push(first_byte);
        header.push(0x78); // PT=120 (Opus)
        header.extend_from_slice(&seq.to_be_bytes());
        header.extend_from_slice(&timestamp.to_be_bytes());
        header.extend_from_slice(&ssrc.to_be_bytes());
        if !extension_words.is_empty() {
            header.extend_from_slice(&0xBEDEu16.to_be_bytes());
            header.extend_from_slice(&(extension_words.len() as u16).to_be_bytes());
            for word in extension_words {
                header.extend_from_slice(word);
            }
        }
        header
    }

    /// Вычисление длины открытой области RTP-заголовка (+ CSRC + RFC 5285 `0xBEDE`) для AAD в режиме `*_rtpsize`.
    pub fn parse_rtpsize_header_len(packet: &[u8]) -> Result<usize, DaveError> {
        if packet.len() < 12 {
            return Err(DaveError::RtpPacketInvalid(
                "RTP packet shorter than 12 bytes",
            ));
        }
        if (packet[0] >> 6) != 2 {
            return Err(DaveError::RtpPacketInvalid("RTP version != 2"));
        }
        let cc = usize::from(packet[0] & 0x0F);
        let mut header_len = 12 + 4 * cc;
        if packet.len() < header_len {
            return Err(DaveError::RtpPacketInvalid("RTP CSRC list truncated"));
        }
        if (packet[0] & 0x10) != 0 {
            if packet.len() < header_len + 4 {
                return Err(DaveError::RtpPacketInvalid(
                    "RTP extension preamble truncated",
                ));
            }
            let ext_words = usize::from(u16::from_be_bytes([
                packet[header_len + 2],
                packet[header_len + 3],
            ]));
            header_len += 4 + 4 * ext_words;
            if packet.len() < header_len {
                return Err(DaveError::RtpPacketInvalid("RTP extension body truncated"));
            }
        }
        Ok(header_len)
    }

    /// Двухслойное шифрование RTP-кадра: внутренний слой DAVE E2EE MLS v1 + внешний слой SRTP `*_rtpsize` (`0x80000000 | counter`).
    pub fn seal_rtp_with_dave(
        mode: DaveTransportCipherMode,
        srtp_secret_key: &[u8; 32],
        rtp_header_and_ext: &[u8],
        opus_payload: &[u8],
        srtp_counter: u32,
        dave_session: &mut DaveVoiceSession,
    ) -> Result<Vec<u8>, DaveError> {
        use aes_gcm::aead::Payload;
        use chacha20poly1305::{XChaCha20Poly1305, XNonce};

        let clear_len = Self::parse_rtpsize_header_len(rtp_header_and_ext)?;
        if clear_len != rtp_header_and_ext.len() {
            return Err(DaveError::RtpPacketInvalid(
                "rtp_header_and_ext contains trailing payload bytes",
            ));
        }

        let dave_frame = dave_session.encrypt_opus(opus_payload)?;
        let wire_counter = SRTP_OUTBOUND_COUNTER_MSB | (srtp_counter & 0x7FFF_FFFF);
        let trailer = wire_counter.to_le_bytes();

        let srtp_ciphertext_and_tag = match mode {
            DaveTransportCipherMode::AeadAes256GcmRtpsize => {
                let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(srtp_secret_key));
                let mut nonce = [0u8; 12];
                nonce[0..4].copy_from_slice(&trailer);
                cipher
                    .encrypt(
                        Nonce::from_slice(&nonce),
                        Payload {
                            msg: &dave_frame,
                            aad: rtp_header_and_ext,
                        },
                    )
                    .map_err(|_| DaveError::AeadAuthenticationFailed)?
            }
            DaveTransportCipherMode::AeadXChaCha20Poly1305Rtpsize => {
                let cipher = XChaCha20Poly1305::new(srtp_secret_key.into());
                let mut nonce = [0u8; 24];
                nonce[0..4].copy_from_slice(&trailer);
                cipher
                    .encrypt(
                        XNonce::from_slice(&nonce),
                        Payload {
                            msg: &dave_frame,
                            aad: rtp_header_and_ext,
                        },
                    )
                    .map_err(|_| DaveError::AeadAuthenticationFailed)?
            }
        };

        let mut wire =
            Vec::with_capacity(rtp_header_and_ext.len() + srtp_ciphertext_and_tag.len() + 4);
        wire.extend_from_slice(rtp_header_and_ext);
        wire.extend_from_slice(&srtp_ciphertext_and_tag);
        wire.extend_from_slice(&trailer);
        Ok(wire)
    }

    /// Двухслойная расшифровка RTP-кадра: проверка AAD заголовка и снятие внешнего слоя SRTP `*_rtpsize`, затем DAVE E2EE MLS v1.
    pub fn open_rtp_with_dave(
        mode: DaveTransportCipherMode,
        srtp_secret_key: &[u8; 32],
        wire_packet: &[u8],
        sender_user_id: u64,
        dave_session: &mut DaveVoiceSession,
    ) -> Result<(Vec<u8>, Vec<u8>, u32), DaveError> {
        use aes_gcm::aead::Payload;
        use chacha20poly1305::{XChaCha20Poly1305, XNonce};

        let clear_len = Self::parse_rtpsize_header_len(wire_packet)?;
        // Минимум после clear-заголовка: 16 байт AEAD tag + 4 байта LE трейлера
        if wire_packet.len() < clear_len + 16 + 4 {
            return Err(DaveError::RtpPacketInvalid(
                "SRTP packet shorter than header + 16B tag + 4B counter",
            ));
        }

        let aad = &wire_packet[..clear_len];
        let trailer_offset = wire_packet.len() - 4;
        let encrypted_body = &wire_packet[clear_len..trailer_offset];
        let mut trailer = [0u8; 4];
        trailer.copy_from_slice(&wire_packet[trailer_offset..]);
        let counter_val = u32::from_le_bytes(trailer);

        let dave_frame = match mode {
            DaveTransportCipherMode::AeadAes256GcmRtpsize => {
                let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(srtp_secret_key));
                let mut nonce = [0u8; 12];
                nonce[0..4].copy_from_slice(&trailer);
                cipher
                    .decrypt(
                        Nonce::from_slice(&nonce),
                        Payload {
                            msg: encrypted_body,
                            aad,
                        },
                    )
                    .map_err(|_| DaveError::AeadAuthenticationFailed)?
            }
            DaveTransportCipherMode::AeadXChaCha20Poly1305Rtpsize => {
                let cipher = XChaCha20Poly1305::new(srtp_secret_key.into());
                let mut nonce = [0u8; 24];
                nonce[0..4].copy_from_slice(&trailer);
                cipher
                    .decrypt(
                        XNonce::from_slice(&nonce),
                        Payload {
                            msg: encrypted_body,
                            aad,
                        },
                    )
                    .map_err(|_| DaveError::AeadAuthenticationFailed)?
            }
        };

        let opus_plaintext = dave_session.decrypt_opus(sender_user_id, &dave_frame)?;
        Ok((aad.to_vec(), opus_plaintext, counter_val))
    }
}

// ============================================================================
// W3-2 (P1-1.3, P1-1.4): Живые конвейеры захвата микрофона и воспроизведения
// (AudioCodecBackend, SoftwareOpusCodec, AudioCapturePipeline, AudioPlaybackPipeline)
// ============================================================================

/// Стандартная частота дискретизации голосового тракта Discord Voice (48 000 Гц).
pub const VOICE_SAMPLE_RATE_HZ: u32 = 48_000;
/// Число сэмплов на один канал в 20-мс фрейме при 48 кГц (`48_000 / 50 = 960`).
pub const VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS: usize = 960;
/// Максимальное усиление микрофона и громкости воспроизведения (`2.0 = 200%`).
pub const VOICE_MAX_GAIN_SCALE: f32 = 2.0;
/// Внутренний маркер заголовка детерминированного программного кодека `SoftwareOpusCodec`.
const SOFTWARE_OPUS_MAGIC_TOC: u8 = 0x78;

/// Расшифрованный входящий RTP/Opus аудио-пакет с извлечёнными полями заголовка RTP v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedRtpAudioPacket {
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub opus_payload: Vec<u8>,
}

impl std::ops::Deref for DecryptedRtpAudioPacket {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.opus_payload
    }
}

impl AsRef<[u8]> for DecryptedRtpAudioPacket {
    fn as_ref(&self) -> &[u8] {
        &self.opus_payload
    }
}

impl PartialEq<Vec<u8>> for DecryptedRtpAudioPacket {
    fn eq(&self, other: &Vec<u8>) -> bool {
        &self.opus_payload == other
    }
}

impl PartialEq<[u8]> for DecryptedRtpAudioPacket {
    fn eq(&self, other: &[u8]) -> bool {
        self.opus_payload.as_slice() == other
    }
}

impl PartialEq<&[u8]> for DecryptedRtpAudioPacket {
    fn eq(&self, other: &&[u8]) -> bool {
        self.opus_payload.as_slice() == *other
    }
}

impl From<DecryptedRtpAudioPacket> for Vec<u8> {
    fn from(pkt: DecryptedRtpAudioPacket) -> Self {
        pkt.opus_payload
    }
}

impl From<DecryptedRtpAudioPacket> for (u16, u32, Vec<u8>) {
    fn from(pkt: DecryptedRtpAudioPacket) -> Self {
        (pkt.sequence, pkt.timestamp, pkt.opus_payload)
    }
}

/// Абстракция кодека Opus (48 кГц, 20 мс) для тестируемого конвейера без привязки к C FFI.
pub trait AudioCodecBackend: Send + Sync {
    /// Кодирование одного 20-мс фрейма PCM (`960 * channels` сэмплов `f32`) в пакет Opus
    /// с целевым бюджетом `target_bytes` (`80..=160` байт, не более `OPUS_MAX_PACKET_BYTES = 1275`).
    fn encode_20ms(
        &mut self,
        pcm: &[f32],
        channels: u16,
        target_bytes: usize,
    ) -> Result<Vec<u8>, String>;

    /// Декодирование входящего пакета Opus (или 3-байтного DTX пакета тишины `[0xF8, 0xFF, 0xFE]`)
    /// в 20-мс фрейм PCM (`960 * channels` сэмплов `f32` в диапазоне `[-1.0, 1.0]`).
    fn decode_20ms(&mut self, opus_packet: &[u8], channels: u16) -> Result<Vec<f32>, String>;

    /// Генерация 20-мс фрейма маскирования потери пакета (Packet Loss Concealment, PLC).
    fn conceal_loss_20ms(&mut self, channels: u16) -> Vec<f32>;

    /// Синоним `encode_20ms` для универсального вызова.
    fn encode(
        &mut self,
        pcm: &[f32],
        channels: u16,
        target_bytes: usize,
    ) -> Result<Vec<u8>, String> {
        self.encode_20ms(pcm, channels, target_bytes)
    }

    /// Синоним `decode_20ms` для универсального вызова.
    fn decode(&mut self, opus_packet: &[u8], channels: u16) -> Result<Vec<f32>, String> {
        self.decode_20ms(opus_packet, channels)
    }
}

/// Чисто-Rust (`#![forbid(unsafe_code)]`) детерминированный кодек Opus для автономного тестирования
/// и fallback-работы без внешних системных библиотек `libopus`.
#[derive(Debug, Clone)]
pub struct SoftwareOpusCodec {
    last_decoded_frame: Vec<f32>,
    consecutive_plc_count: u32,
}

impl Default for SoftwareOpusCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareOpusCodec {
    pub fn new() -> Self {
        Self {
            last_decoded_frame: Vec::new(),
            consecutive_plc_count: 0,
        }
    }
}

impl AudioCodecBackend for SoftwareOpusCodec {
    fn encode_20ms(
        &mut self,
        pcm: &[f32],
        channels: u16,
        target_bytes: usize,
    ) -> Result<Vec<u8>, String> {
        if channels != 1 && channels != 2 {
            return Err(format!(
                "Неподдерживаемое число каналов Opus: {} (допустимо 1 или 2)",
                channels
            ));
        }
        let expected_samples = VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS * (channels as usize);
        if pcm.len() != expected_samples {
            return Err(format!(
                "Некорректный размер 20-мс PCM фрейма: {} сэмплов (ожидалось {})",
                pcm.len(),
                expected_samples
            ));
        }
        if target_bytes == 0 || target_bytes > OPUS_MAX_PACKET_BYTES {
            return Err(format!(
                "Целевой размер пакета Opus ({} байт) выходит за пределы 1..={}",
                target_bytes, OPUS_MAX_PACKET_BYTES
            ));
        }

        let packet_len = target_bytes.clamp(12, OPUS_MAX_PACKET_BYTES);
        let mut peak = 0.0f32;
        for &s in pcm {
            if s.is_finite() {
                let abs = s.clamp(-1.0, 1.0).abs();
                if abs > peak {
                    peak = abs;
                }
            }
        }

        // Формат пакета SoftwareOpusCodec:
        // [0]: SOFTWARE_OPUS_MAGIC_TOC (0x78)
        // [1]: channels (1 или 2)
        // [2..6]: peak_f32 (LE)
        // [6..packet_len]: квантованные поддиапазоны i8 (-127..=127) по каналам
        let mut out = vec![0u8; packet_len];
        out[0] = SOFTWARE_OPUS_MAGIC_TOC;
        out[1] = channels as u8;
        out[2..6].copy_from_slice(&peak.to_le_bytes());

        if peak <= 1e-7 {
            return Ok(out);
        }

        let ch_usize = channels as usize;
        let payload_bytes = packet_len - 6;
        let buckets_per_channel = (payload_bytes / ch_usize).max(1);

        for b_idx in 0..buckets_per_channel {
            let start_frame = (b_idx * VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS) / buckets_per_channel;
            let end_frame = (((b_idx + 1) * VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS)
                / buckets_per_channel)
                .max(start_frame + 1)
                .min(VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS);
            let count = (end_frame - start_frame) as f32;

            for ch in 0..ch_usize {
                let byte_offset = 6 + b_idx * ch_usize + ch;
                if byte_offset >= packet_len {
                    break;
                }
                let mut sum = 0.0f32;
                for f in start_frame..end_frame {
                    let s = pcm[f * ch_usize + ch];
                    if s.is_finite() {
                        sum += s.clamp(-1.0, 1.0);
                    }
                }
                let mean = sum / count;
                let normalized = (mean / peak).clamp(-1.0, 1.0);
                let q = (normalized * 127.0).round() as i8;
                out[byte_offset] = q as u8;
            }
        }

        Ok(out)
    }

    fn decode_20ms(&mut self, opus_packet: &[u8], channels: u16) -> Result<Vec<f32>, String> {
        if channels != 1 && channels != 2 {
            return Err(format!(
                "Неподдерживаемое число каналов декодера Opus: {}",
                channels
            ));
        }
        if opus_packet.is_empty() {
            return Err("Пустой пакет Opus (0 байт) не может быть декодирован".to_string());
        }
        if opus_packet.len() > OPUS_MAX_PACKET_BYTES {
            return Err(format!(
                "Пакет Opus ({} байт) превышает лимит RFC 6716 ({} байт)",
                opus_packet.len(),
                OPUS_MAX_PACKET_BYTES
            ));
        }

        let total_samples = VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS * (channels as usize);
        self.consecutive_plc_count = 0;

        // Пакет тишины Opus DTX ([0xF8, 0xFF, 0xFE]) всегда декодируется в чистые нули
        if opus_packet == DAVE_OPUS_SILENCE_PACKET {
            let silence = vec![0.0f32; total_samples];
            self.last_decoded_frame = silence.clone();
            return Ok(silence);
        }

        let mut pcm = vec![0.0f32; total_samples];
        let out_ch = channels as usize;

        if opus_packet.len() >= 8
            && opus_packet[0] == SOFTWARE_OPUS_MAGIC_TOC
            && (opus_packet[1] == 1 || opus_packet[1] == 2)
        {
            let pkt_ch = opus_packet[1] as usize;
            let raw_peak = f32::from_le_bytes([
                opus_packet[2],
                opus_packet[3],
                opus_packet[4],
                opus_packet[5],
            ]);
            let peak = if raw_peak.is_finite() {
                raw_peak.clamp(0.0, 1.0)
            } else {
                0.0
            };

            if peak > 0.0 {
                let payload = &opus_packet[6..];
                let buckets_per_channel = (payload.len() / pkt_ch).max(1);
                for f in 0..VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS {
                    let b_idx = ((f * buckets_per_channel) / VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS)
                        .min(buckets_per_channel - 1);
                    for ch in 0..out_ch {
                        let src_ch = ch.min(pkt_ch - 1);
                        let idx = (b_idx * pkt_ch + src_ch).min(payload.len() - 1);
                        let q = payload[idx] as i8;
                        let sample = ((q as f32) / 127.0) * peak;
                        pcm[f * out_ch + ch] = sample.clamp(-1.0, 1.0);
                    }
                }
            }
        } else {
            // Детерминированная реконструкция для произвольных внешних тестовых пакетов Opus
            for (i, sample) in pcm.iter_mut().enumerate() {
                let b = opus_packet[i % opus_packet.len()] as i8;
                *sample = ((b as f32) / 128.0).clamp(-1.0, 1.0);
            }
        }

        self.last_decoded_frame = pcm.clone();
        Ok(pcm)
    }

    fn conceal_loss_20ms(&mut self, channels: u16) -> Vec<f32> {
        let total_samples = VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS * (channels.max(1) as usize);
        self.consecutive_plc_count = self.consecutive_plc_count.saturating_add(1);

        if self.last_decoded_frame.len() != total_samples {
            self.last_decoded_frame = vec![0.0f32; total_samples];
            return self.last_decoded_frame.clone();
        }

        // Плавное экспоненциальное затухание (0.7^n) при маскировании потери пакета
        let attenuation = 0.7f32.powi(self.consecutive_plc_count.min(16) as i32);
        for s in &mut self.last_decoded_frame {
            if s.is_finite() {
                *s = (*s * attenuation).clamp(-1.0, 1.0);
            } else {
                *s = 0.0;
            }
        }
        self.last_decoded_frame.clone()
    }
}

/// Конфигурация живого конвейера захвата микрофона (`P1-1.3`).
#[derive(Debug, Clone)]
pub struct AudioCaptureConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub mic_gain: f32,
    pub vad_threshold: f32,
    pub mute: bool,
    pub emit_dtx_silence_frames: bool,
    pub ring_capacity_samples: usize,
}

impl Default for AudioCaptureConfig {
    fn default() -> Self {
        Self::new(VOICE_SAMPLE_RATE_HZ, 2)
    }
}

impl AudioCaptureConfig {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let norm_rate = if sample_rate == 0 {
            VOICE_SAMPLE_RATE_HZ
        } else {
            sample_rate
        };
        let norm_channels = channels.clamp(1, 2);
        Self {
            sample_rate: norm_rate,
            channels: norm_channels,
            mic_gain: 1.0,
            vad_threshold: 0.001,
            mute: false,
            emit_dtx_silence_frames: true,
            ring_capacity_samples: (norm_rate as usize) * (norm_channels as usize),
        }
    }

    pub fn with_mic_gain(mut self, gain: f32) -> Self {
        self.mic_gain = if gain.is_finite() {
            gain.clamp(0.0, VOICE_MAX_GAIN_SCALE)
        } else {
            0.0
        };
        self
    }

    pub fn with_vad_threshold(mut self, threshold: f32) -> Self {
        self.vad_threshold = if threshold.is_finite() {
            threshold.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    pub fn with_mute(mut self, mute: bool) -> Self {
        self.mute = mute;
        self
    }

    pub fn with_dtx_silence(mut self, emit_dtx: bool) -> Self {
        self.emit_dtx_silence_frames = emit_dtx;
        self
    }
}

/// Накопительная телеметрия конвейера захвата микрофона (`P1-1.3`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioCaptureStats {
    pub captured_samples: u64,
    pub frames_encoded: u64,
    pub silence_frames: u64,
    pub packets_sent: u64,
    pub bytes_sent: u64,
    pub overrun_drops: u64,
}

/// Результат одного такта опроса и отправки 20-мс фреймов в `VoiceUdpStreamer`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureTickStats {
    pub frames_polled: usize,
    pub frames_encoded: usize,
    pub silence_frames: usize,
    pub packets_sent: usize,
    pub bytes_sent: usize,
    pub payloads: Vec<Vec<u8>>,
}

/// Живой конвейер захвата микрофона: `AudioRingBuffer -> VoiceAudioFramer (20ms) -> VAD/Mute -> Opus -> RTP/DAVE UDP` (`P1-1.3`).
pub struct AudioCapturePipeline {
    config: AudioCaptureConfig,
    mic_ring: AudioRingBuffer,
    framer: VoiceAudioFramer,
    codec: Box<dyn AudioCodecBackend>,
    bitrate_controller: OpusBitrateController,
    stats: AudioCaptureStats,
}

impl AudioCapturePipeline {
    pub fn new(config: AudioCaptureConfig) -> Self {
        Self::with_codec(config, Box::new(SoftwareOpusCodec::new()))
    }

    pub fn with_codec(mut config: AudioCaptureConfig, codec: Box<dyn AudioCodecBackend>) -> Self {
        config.channels = config.channels.clamp(1, 2);
        config.mic_gain = if config.mic_gain.is_finite() {
            config.mic_gain.clamp(0.0, VOICE_MAX_GAIN_SCALE)
        } else {
            0.0
        };
        config.vad_threshold = if config.vad_threshold.is_finite() {
            config.vad_threshold.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let frame_samples = VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS * (config.channels as usize);
        let ring_cap = config.ring_capacity_samples.max(frame_samples * 2);
        config.ring_capacity_samples = ring_cap;

        Self {
            mic_ring: AudioRingBuffer::with_capacity(ring_cap),
            framer: VoiceAudioFramer::new(config.sample_rate, config.channels),
            config,
            codec,
            bitrate_controller: OpusBitrateController::new(),
            stats: AudioCaptureStats::default(),
        }
    }

    pub fn config(&self) -> &AudioCaptureConfig {
        &self.config
    }

    pub fn stats(&self) -> &AudioCaptureStats {
        &self.stats
    }

    pub fn is_muted(&self) -> bool {
        self.config.mute
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.config.mute = mute;
    }

    pub fn mic_gain(&self) -> f32 {
        self.config.mic_gain
    }

    pub fn set_mic_gain(&mut self, gain: f32) {
        self.config.mic_gain = if gain.is_finite() {
            gain.clamp(0.0, VOICE_MAX_GAIN_SCALE)
        } else {
            0.0
        };
    }

    pub fn set_vad_threshold(&mut self, threshold: f32) {
        self.config.vad_threshold = if threshold.is_finite() {
            threshold.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    pub fn bitrate_controller(&self) -> &OpusBitrateController {
        &self.bitrate_controller
    }

    pub fn bitrate_controller_mut(&mut self) -> &mut OpusBitrateController {
        &mut self.bitrate_controller
    }

    /// Запись сырых сэмплов с микрофона (CPAL input callback) в кольцевой буфер с очисткой `NaN`/`±Inf`.
    pub fn push_mic_samples(&mut self, samples: &[f32]) -> usize {
        if samples.is_empty() {
            return 0;
        }
        let free_space = self
            .mic_ring
            .capacity()
            .saturating_sub(self.mic_ring.available_samples());
        if samples.len() > free_space {
            let dropped = samples.len() - free_space;
            self.stats.overrun_drops = self.stats.overrun_drops.saturating_add(dropped as u64);
        }

        let sanitized: Vec<f32> = samples
            .iter()
            .map(|&s| if s.is_finite() { s } else { 0.0 })
            .collect();
        let written = self.mic_ring.write_samples(&sanitized);
        self.stats.captured_samples = self.stats.captured_samples.saturating_add(written as u64);
        written
    }

    /// Извлечение всех накопившихся полных 20-мс чанков (`960 * channels` сэмплов) и их кодирование в Opus / DTX.
    pub fn drain_and_encode_frames(&mut self) -> Result<Vec<Vec<u8>>, String> {
        let mut packets = Vec::new();
        self.drain_and_encode_frames_with_counts(&mut packets)?;
        Ok(packets)
    }

    fn drain_and_encode_frames_with_counts(
        &mut self,
        out_packets: &mut Vec<Vec<u8>>,
    ) -> Result<(usize, usize, usize), String> {
        let avail = self.mic_ring.available_samples();
        if avail > 0 {
            let mut raw = vec![0.0f32; avail];
            let read_n = self.mic_ring.read_samples(&mut raw);
            raw.truncate(read_n);
            let frames = self.framer.push_samples(&raw);
            let target_bytes = self
                .bitrate_controller
                .target_frame_bytes_20ms()
                .clamp(80, OPUS_MAX_PACKET_BYTES);

            let mut polled = 0usize;
            let mut encoded_count = 0usize;
            let mut silence_count = 0usize;

            for mut frame in frames {
                polled += 1;
                let gain = self.config.mic_gain;
                let mut sum_sq = 0.0f32;
                for s in &mut frame {
                    let clean = if s.is_finite() { *s * gain } else { 0.0 };
                    let clamped = clean.clamp(-1.0, 1.0);
                    *s = clamped;
                    sum_sq += clamped * clamped;
                }
                let rms = if frame.is_empty() {
                    0.0
                } else {
                    (sum_sq / (frame.len() as f32)).sqrt()
                };

                if self.config.mute || gain <= 0.0 || rms <= self.config.vad_threshold {
                    silence_count += 1;
                    self.stats.silence_frames = self.stats.silence_frames.saturating_add(1);
                    if self.config.emit_dtx_silence_frames {
                        out_packets.push(DAVE_OPUS_SILENCE_PACKET.to_vec());
                    }
                } else {
                    let opus =
                        self.codec
                            .encode_20ms(&frame, self.config.channels, target_bytes)?;
                    if opus.is_empty() || opus.len() > OPUS_MAX_PACKET_BYTES {
                        return Err(format!(
                            "Кодек вернул пакет недопустимого размера: {} байт",
                            opus.len()
                        ));
                    }
                    encoded_count += 1;
                    self.stats.frames_encoded = self.stats.frames_encoded.saturating_add(1);
                    out_packets.push(opus);
                }
            }
            return Ok((polled, encoded_count, silence_count));
        }
        Ok((0, 0, 0))
    }

    /// Опрос готовых 20-мс фреймов, синхронизация целевого битрейта со стримером и отправка по UDP (DAVE или SRTP).
    pub async fn poll_and_send_frames(
        &mut self,
        streamer: &VoiceUdpStreamer,
    ) -> Result<CaptureTickStats, String> {
        {
            let streamer_ctrl = streamer.bitrate_controller.lock().await;
            self.bitrate_controller = streamer_ctrl.clone();
        }

        let mut payloads = Vec::new();
        let (frames_polled, frames_encoded, silence_frames) =
            self.drain_and_encode_frames_with_counts(&mut payloads)?;

        let use_dave = streamer.dave_session.lock().await.is_some();
        let mut packets_sent = 0usize;
        let mut bytes_sent = 0usize;

        for pkt in &payloads {
            let sent = if use_dave {
                streamer
                    .send_dave_audio_packet(pkt, None)
                    .await
                    .map_err(|e| format!("Ошибка отправки DAVE аудио-пакета: {:?}", e))?
            } else {
                streamer.send_audio_packet(pkt).await?
            };
            packets_sent += 1;
            bytes_sent += sent;
            self.stats.packets_sent = self.stats.packets_sent.saturating_add(1);
            self.stats.bytes_sent = self.stats.bytes_sent.saturating_add(sent as u64);
        }

        Ok(CaptureTickStats {
            frames_polled,
            frames_encoded,
            silence_frames,
            packets_sent,
            bytes_sent,
            payloads,
        })
    }
}

/// Конфигурация живого конвейера воспроизведения голоса (`P1-1.4`).
#[derive(Debug, Clone)]
pub struct AudioPlaybackConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub output_volume: f32,
    pub deafen: bool,
    pub initial_jitter_delay_ms: u32,
    pub speaker_ring_capacity_samples: usize,
}

impl Default for AudioPlaybackConfig {
    fn default() -> Self {
        Self::new(VOICE_SAMPLE_RATE_HZ, 2)
    }
}

impl AudioPlaybackConfig {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let norm_rate = if sample_rate == 0 {
            VOICE_SAMPLE_RATE_HZ
        } else {
            sample_rate
        };
        let norm_channels = channels.clamp(1, 2);
        Self {
            sample_rate: norm_rate,
            channels: norm_channels,
            output_volume: 1.0,
            deafen: false,
            initial_jitter_delay_ms: JITTER_BUFFER_DEFAULT_MS,
            speaker_ring_capacity_samples: (norm_rate as usize) * (norm_channels as usize),
        }
    }

    pub fn with_output_volume(mut self, volume: f32) -> Self {
        self.output_volume = if volume.is_finite() {
            volume.clamp(0.0, VOICE_MAX_GAIN_SCALE)
        } else {
            0.0
        };
        self
    }

    pub fn with_deafen(mut self, deafen: bool) -> Self {
        self.deafen = deafen;
        self
    }

    pub fn with_jitter_delay_ms(mut self, delay_ms: u32) -> Self {
        self.initial_jitter_delay_ms = delay_ms.clamp(JITTER_BUFFER_MIN_MS, JITTER_BUFFER_MAX_MS);
        self
    }
}

/// Накопительная телеметрия конвейера воспроизведения голоса (`P1-1.4`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioPlaybackStats {
    pub packets_received: u64,
    pub packets_decoded: u64,
    pub plc_frames: u64,
    pub dropped_late: u64,
    pub dropped_duplicate: u64,
    pub samples_written_to_ring: u64,
}

/// Живой конвейер воспроизведения голоса: `UDP/RTP (SRTP | DAVE) -> AdaptiveJitterBuffer -> Opus/PLC -> Volume/Deafen -> Speaker AudioRingBuffer` (`P1-1.4`).
pub struct AudioPlaybackPipeline {
    config: AudioPlaybackConfig,
    jitter_buffer: AdaptiveJitterBuffer,
    speaker_ring: AudioRingBuffer,
    codec: Box<dyn AudioCodecBackend>,
    stats: AudioPlaybackStats,
}

impl AudioPlaybackPipeline {
    pub fn new(config: AudioPlaybackConfig) -> Self {
        Self::with_codec(config, Box::new(SoftwareOpusCodec::new()))
    }

    pub fn with_codec(mut config: AudioPlaybackConfig, codec: Box<dyn AudioCodecBackend>) -> Self {
        config.channels = config.channels.clamp(1, 2);
        config.output_volume = if config.output_volume.is_finite() {
            config.output_volume.clamp(0.0, VOICE_MAX_GAIN_SCALE)
        } else {
            0.0
        };
        let frame_samples = VOICE_FRAME_SAMPLES_PER_CHANNEL_20MS * (config.channels as usize);
        let ring_cap = config.speaker_ring_capacity_samples.max(frame_samples * 8);
        config.speaker_ring_capacity_samples = ring_cap;

        Self {
            jitter_buffer: AdaptiveJitterBuffer::with_target_delay_ms(
                config.initial_jitter_delay_ms,
            ),
            speaker_ring: AudioRingBuffer::with_capacity(ring_cap),
            config,
            codec,
            stats: AudioPlaybackStats::default(),
        }
    }

    pub fn config(&self) -> &AudioPlaybackConfig {
        &self.config
    }

    pub fn stats(&self) -> &AudioPlaybackStats {
        &self.stats
    }

    pub fn output_volume(&self) -> f32 {
        self.config.output_volume
    }

    pub fn set_output_volume(&mut self, volume: f32) {
        self.config.output_volume = if volume.is_finite() {
            volume.clamp(0.0, VOICE_MAX_GAIN_SCALE)
        } else {
            0.0
        };
    }

    pub fn is_deafened(&self) -> bool {
        self.config.deafen
    }

    pub fn set_deafen(&mut self, deafen: bool) {
        self.config.deafen = deafen;
    }

    pub fn jitter_buffer(&self) -> &AdaptiveJitterBuffer {
        &self.jitter_buffer
    }

    pub fn jitter_buffer_mut(&mut self) -> &mut AdaptiveJitterBuffer {
        &mut self.jitter_buffer
    }

    pub fn speaker_ring(&self) -> &AudioRingBuffer {
        &self.speaker_ring
    }

    fn sync_jitter_counters(&mut self) {
        self.stats.packets_received = self.jitter_buffer.packets_received();
        self.stats.dropped_late = self.jitter_buffer.packets_dropped_late();
        self.stats.dropped_duplicate = self.jitter_buffer.packets_dropped_duplicate();
        self.stats.plc_frames = self.jitter_buffer.plc_frames_generated();
    }

    fn scale_and_push_pcm(&mut self, pcm: &mut [f32]) {
        let vol = if self.config.deafen {
            0.0
        } else {
            self.config.output_volume
        };
        for sample in pcm.iter_mut() {
            if !sample.is_finite() || vol <= 0.0 {
                *sample = 0.0;
            } else {
                *sample = (*sample * vol).clamp(-1.0, 1.0);
            }
        }
        let written = self.speaker_ring.write_samples(pcm);
        self.stats.samples_written_to_ring = self
            .stats
            .samples_written_to_ring
            .saturating_add(written as u64);
    }

    /// Прямая подача расшифрованного RTP/Opus фрейма в адаптивный джиттер-буфер.
    pub fn ingest_rtp_opus_frame(
        &mut self,
        sequence: u16,
        timestamp: u32,
        opus_payload: Vec<u8>,
    ) -> Result<bool, String> {
        let res = self.jitter_buffer.push(sequence, timestamp, opus_payload);
        self.sync_jitter_counters();
        res
    }

    /// Приём одного зашифрованного пакета из `VoiceUdpStreamer` (автоматически выбирает DAVE E2EE или SRTP)
    /// и постановка его в `AdaptiveJitterBuffer`.
    pub async fn recv_and_queue_packet(
        &mut self,
        streamer: &VoiceUdpStreamer,
        sender_user_id: u64,
    ) -> Result<DecryptedRtpAudioPacket, String> {
        let use_dave = streamer.dave_session.lock().await.is_some();
        let pkt = if use_dave {
            streamer
                .recv_dave_rtp_packet(sender_user_id)
                .await
                .map_err(|e| format!("Ошибка приёма DAVE RTP пакета: {:?}", e))?
        } else {
            streamer.recv_audio_packet().await?
        };
        self.ingest_rtp_opus_frame(pkt.sequence, pkt.timestamp, pkt.opus_payload.clone())?;
        Ok(pkt)
    }

    /// Извлечение следующего упорядоченного 20-мс фрейма (или сигнала PLC) из джиттер-буфера,
    /// декодирование в PCM, масштабирование громкости (`0%..=200%` / `deafen`) и запись в `speaker_ring`.
    pub fn pop_and_decode_next_frame(&mut self) -> Result<JitterPopStatus, String> {
        let status = self.jitter_buffer.pop();
        self.sync_jitter_counters();

        match &status {
            JitterPopStatus::Ready(frame) => {
                let mut pcm = self
                    .codec
                    .decode_20ms(&frame.payload, self.config.channels)?;
                self.scale_and_push_pcm(&mut pcm);
                self.stats.packets_decoded = self.stats.packets_decoded.saturating_add(1);
            }
            JitterPopStatus::ConcealLoss { .. } => {
                let mut pcm = self.codec.conceal_loss_20ms(self.config.channels);
                self.scale_and_push_pcm(&mut pcm);
            }
            JitterPopStatus::Buffering | JitterPopStatus::Empty => {}
        }

        Ok(status)
    }

    /// Декодирование всех готовых к выдаче фреймов из `AdaptiveJitterBuffer` в `speaker_ring`.
    pub fn drain_jitter_to_speaker_ring(&mut self) -> Result<usize, String> {
        let mut count = 0usize;
        loop {
            match self.pop_and_decode_next_frame()? {
                JitterPopStatus::Ready(_) | JitterPopStatus::ConcealLoss { .. } => {
                    count += 1;
                }
                JitterPopStatus::Buffering | JitterPopStatus::Empty => break,
            }
        }
        Ok(count)
    }

    /// Чтение декодированных PCM-сэмплов для выходного колбэка CPAL с защитой от underflow (заполнение `0.0`).
    pub fn read_speaker_samples(&mut self, out: &mut [f32]) -> usize {
        while self.speaker_ring.available_samples() < out.len()
            && !self.jitter_buffer.is_buffering()
            && !self.jitter_buffer.is_empty()
        {
            if self.pop_and_decode_next_frame().is_err() {
                break;
            }
        }
        let read_n = self.speaker_ring.read_samples(out);
        if self.config.deafen || self.config.output_volume <= 0.0 {
            for s in out.iter_mut() {
                *s = 0.0;
            }
        }
        read_n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_voice::rtcp::{ReportBlock, build_receiver_report, build_srtcp_receiver_report};

    fn sample_report_block(ssrc: u32, fraction_lost: u8, jitter: u32) -> ReportBlock {
        ReportBlock {
            ssrc,
            fraction_lost,
            cumulative_lost: 10,
            extended_highest_seq: 1024,
            jitter,
            last_sr: 0,
            delay_since_last_sr: 0,
        }
    }

    #[test]
    fn test_opus_bitrate_scaling_bounds_and_loss_reaction() {
        let mut ctrl = OpusBitrateController::new();
        assert_eq!(ctrl.target_bitrate_bps(), 64_000);
        assert_eq!(ctrl.target_bitrate_kbps(), 64);
        assert_eq!(ctrl.target_frame_bytes_20ms(), 160);

        // Умеренные потери (fraction_lost = 16 -> ~6.25%) снижают битрейт на 8 кбит/с (до 56 кбит/с)
        let b1 = ctrl.on_rtcp_fraction_lost(16);
        assert_eq!(b1, 56_000);
        assert_eq!(ctrl.target_bitrate_kbps(), 56);
        assert_eq!(ctrl.scale_down_events(), 1);

        // Высокие потери (fraction_lost = 64 -> 25%) снижают битрейт быстрее (шаг 16 кбит/с -> 40 кбит/с)
        let b2 = ctrl.on_rtcp_fraction_lost(64);
        assert_eq!(b2, 40_000);
        assert_eq!(ctrl.target_bitrate_kbps(), 40);

        // Повторные высокие потери упираются в нижнюю границу 32 кбит/с (32_000 бит/с)
        let b3 = ctrl.on_rtcp_fraction_lost(128);
        assert_eq!(b3, 32_000);
        assert_eq!(ctrl.target_bitrate_kbps(), 32);
        assert_eq!(ctrl.target_frame_bytes_20ms(), 80);

        // Дальнейшие экстремальные потери (255) не опускают битрейт ниже 32 кбит/с
        assert_eq!(ctrl.on_rtcp_fraction_lost(255), 32_000);

        // Промежуточная зона (fraction_lost = 8, ~3.1%) удерживает текущий битрейт
        assert_eq!(ctrl.on_rtcp_fraction_lost(8), 32_000);

        // Низкие потери (fraction_lost <= 5) плавно восстанавливают битрейт шагами по +4 кбит/с до потолка 64 кбит/с
        for expected_kbps in (36..=64).step_by(4) {
            let bps = ctrl.on_rtcp_fraction_lost(0);
            assert_eq!(bps, expected_kbps * 1_000);
        }
        assert_eq!(ctrl.target_bitrate_bps(), 64_000);
        // Выше 64 кбит/с не поднимается
        assert_eq!(ctrl.on_rtcp_fraction_lost(0), 64_000);
    }

    #[test]
    fn test_adaptive_jitter_buffer_20_to_60ms_scaling_and_reordering() {
        let mut jb = AdaptiveJitterBuffer::new();
        // По умолчанию 40 мс (2 фрейма по 20 мс)
        assert_eq!(jb.target_delay_ms(), 40);
        assert_eq!(jb.target_frames(), 2);
        assert!(jb.is_buffering());

        // При чистом канале (fraction_lost = 0) буфер сжимается до минимальных 20 мс (1 фрейм)
        assert_eq!(jb.on_rtcp_fraction_lost(0), 20);
        assert_eq!(jb.target_frames(), 1);
        assert_eq!(jb.on_rtcp_fraction_lost(0), 20);

        // При умеренных потерях (fraction_lost = 15) буфер расширяется до 40 мс (2 фрейма)
        assert_eq!(jb.on_rtcp_fraction_lost(15), 40);
        assert_eq!(jb.target_frames(), 2);

        // При высоких потерях (fraction_lost = 32) буфер расширяется до максимальных 60 мс (3 фрейма)
        assert_eq!(jb.on_rtcp_fraction_lost(32), 60);
        assert_eq!(jb.target_frames(), 3);
        assert_eq!(jb.on_rtcp_fraction_lost(255), 60);

        // При глубине 60 мс (3 фрейма) проверяем переупорядочивание пакетов (пришли seq 102, 100, 101)
        assert_eq!(jb.push(102, 2880, vec![0xAA; 80]).unwrap(), true);
        assert_eq!(jb.pop(), JitterPopStatus::Buffering);
        assert_eq!(jb.push(100, 960, vec![0xBB; 80]).unwrap(), true);
        assert_eq!(jb.pop(), JitterPopStatus::Buffering);
        // Дубликат seq 100 должен быть отброшен
        assert_eq!(jb.push(100, 960, vec![0xBB; 80]).unwrap(), false);
        assert_eq!(jb.packets_dropped_duplicate(), 1);

        // Третий уникальный пакет завершает pre-buffering (3 фрейма = 60 мс)
        assert_eq!(jb.push(101, 1920, vec![0xCC; 80]).unwrap(), true);
        assert!(!jb.is_buffering());

        // Извлечение идёт строго по порядку: 100, 101, 102
        match jb.pop() {
            JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 100),
            other => panic!("Expected Ready(100), got {:?}", other),
        }
        // Опоздавший пакет seq 99 (меньше ожидаемого 101) отбрасывается
        assert_eq!(jb.push(99, 0, vec![0xDD; 80]).unwrap(), false);
        assert_eq!(jb.packets_dropped_late(), 1);

        match jb.pop() {
            JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 101),
            other => panic!("Expected Ready(101), got {:?}", other),
        }
        match jb.pop() {
            JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 102),
            other => panic!("Expected Ready(102), got {:?}", other),
        }
        assert_eq!(jb.pop(), JitterPopStatus::Empty);
        assert!(jb.is_buffering());
    }

    #[test]
    fn test_jitter_buffer_wrap_around_plc_and_validation() {
        let mut jb = AdaptiveJitterBuffer::with_target_delay_ms(20);
        assert_eq!(jb.target_delay_ms(), 20);
        assert_eq!(jb.target_frames(), 1);

        // Проверка валидации пустого и слишком большого (>1275 байт) пакета Opus
        assert!(jb.push(1, 960, vec![]).is_err());
        assert!(
            jb.push(1, 960, vec![0u8; OPUS_MAX_PACKET_BYTES + 1])
                .is_err()
        );

        // Проверка перехода sequence через u16::MAX (65535 -> 0 -> 2, с потерей seq 1)
        assert!(jb.push(65535, 960, vec![1, 2, 3]).unwrap());
        assert!(jb.push(0, 1920, vec![4, 5, 6]).unwrap());
        assert!(jb.push(2, 3840, vec![7, 8, 9]).unwrap());

        match jb.pop() {
            JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 65535),
            other => panic!("Expected 65535, got {:?}", other),
        }
        match jb.pop() {
            JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 0),
            other => panic!("Expected 0, got {:?}", other),
        }
        // seq 1 потерян, а seq 2 уже в буфере -> должен вернуться сигнал ConcealLoss { missing_sequence: 1 }
        assert_eq!(
            jb.pop(),
            JitterPopStatus::ConcealLoss {
                missing_sequence: 1
            }
        );
        assert_eq!(jb.plc_frames_generated(), 1);

        // Следующий вызов выдаёт seq 2
        match jb.pop() {
            JitterPopStatus::Ready(f) => assert_eq!(f.sequence, 2),
            other => panic!("Expected 2, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_rtcp_wire_packet_integration_with_voice_udp_streamer() {
        let server_socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();

        let streamer = VoiceUdpStreamer::bind_and_connect(VoiceUdpConfig {
            remote_addr: server_addr,
            ssrc: 0x1234_5678,
            secret_key: [7u8; 32],
        })
        .await
        .unwrap();

        // 1. Формируем 32-байтный RTCP Receiver Report с высокими потерями (fraction_lost = 40)
        let rr_high =
            build_receiver_report(0x9999_0001, &sample_report_block(0x1234_5678, 40, 120));
        let (bitrate_bps, jitter_ms) = streamer
            .handle_rtcp_receiver_report(&rr_high)
            .await
            .expect("Разбор 32B RTCP RR должен быть успешным");
        assert_eq!(bitrate_bps, 48_000);
        assert_eq!(jitter_ms, 60);

        // 2. Формируем 52-байтный серверный SRTCP Receiver Report с нулевыми потерями (fraction_lost = 0)
        let clear_rr = build_receiver_report(0x9999_0001, &sample_report_block(0x1234_5678, 0, 10));
        let mut block_bytes = [0u8; 24];
        block_bytes.copy_from_slice(&clear_rr[8..32]);
        let srtcp_rr = build_srtcp_receiver_report(0x9999_0001, &block_bytes, [0xAA; 16], 1);

        let (bitrate_bps2, jitter_ms2) = streamer
            .handle_rtcp_receiver_report(&srtcp_rr)
            .await
            .expect("Разбор 52B SRTCP RR должен быть успешным");
        assert_eq!(bitrate_bps2, 52_000);
        assert_eq!(jitter_ms2, 40);

        let stats = streamer.stats.lock().await.clone();
        assert_eq!(stats.rtcp_reports_received, 2);
        assert_eq!(stats.current_bitrate_bps, 52_000);
        assert_eq!(stats.jitter_buffer_ms, 40);
        assert_eq!(stats.last_fraction_lost, 0);

        // 3. Проверка отклонения битого/усечённого RTCP пакета
        assert!(
            streamer
                .handle_rtcp_receiver_report(&[0x81, 201])
                .await
                .is_err()
        );
    }

    fn hex_to_vec(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    fn sample_group_context(epoch: u64, transcript_seed: u8) -> DaveMlsGroupContext {
        DaveMlsGroupContext {
            group_id: b"discord-voice-channel-777".to_vec(),
            epoch,
            tree_hash: [0x11u8.wrapping_add(epoch as u8); 32],
            confirmed_transcript_hash: [transcript_seed; 32],
            extensions: vec![0x00, 0x02, 0xAA, 0xBB],
        }
    }

    #[test]
    fn test_dave_sha256_hkdf_and_mls_kdf_vectors() {
        // 1. FIPS 180-4 SHA-256 официальные эталонные векторы
        assert_eq!(
            dave_sha256(b"").to_vec(),
            hex_to_vec("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        assert_eq!(
            dave_sha256(b"abc").to_vec(),
            hex_to_vec("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(
            dave_sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq").to_vec(),
            hex_to_vec("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1")
        );

        // 2. RFC 5869 §A.1 Test Case 1 — HKDF-SHA256
        let ikm = [0x0bu8; 22];
        let salt: Vec<u8> = (0x00u8..=0x0cu8).collect();
        let info: Vec<u8> = (0xf0u8..=0xf9u8).collect();
        let prk = dave_hkdf_extract_sha256(&salt, &ikm);
        assert_eq!(
            prk.to_vec(),
            hex_to_vec("077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5")
        );
        let okm = dave_hkdf_expand_sha256(&prk, &info, 42).unwrap();
        assert_eq!(
            okm,
            hex_to_vec(
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
            )
        );
        // Граница RFC 5869: длина > 255 * 32 = 8160 байт отклоняется
        assert!(dave_hkdf_expand_sha256(&prk, &info, 255 * 32 + 1).is_err());

        // 3. Кодирование RFC 9420 §2.1.2 VLBytes на границах 0, 63, 64, 16383, 16384
        let mut out = Vec::new();
        mls_encode_vlbytes(&[], &mut out).unwrap();
        assert_eq!(out, vec![0x00]);

        out.clear();
        mls_encode_vlbytes(&vec![0xAB; 63], &mut out).unwrap();
        assert_eq!(out[0], 63);
        assert_eq!(out.len(), 64);

        out.clear();
        mls_encode_vlbytes(&vec![0xCD; 64], &mut out).unwrap();
        assert_eq!(&out[0..2], &[0x40, 0x40]);
        assert_eq!(out.len(), 66);

        out.clear();
        mls_encode_vlbytes(&vec![0xEF; 16384], &mut out).unwrap();
        assert_eq!(&out[0..4], &[0x80, 0x00, 0x40, 0x00]);
        assert_eq!(out.len(), 16388);

        // 4. Декодирование опкодов DAVE 21..=31
        assert_eq!(
            DaveVoiceOpcode::from_u8(21),
            Some(DaveVoiceOpcode::PrepareTransition)
        );
        assert_eq!(
            DaveVoiceOpcode::from_u8(31),
            Some(DaveVoiceOpcode::InvalidCommitWelcome)
        );
        assert_eq!(DaveVoiceOpcode::from_u8(20), None);
        assert_eq!(DaveVoiceOpcode::from_u8(32), None);
    }

    #[test]
    fn test_dave_hash_ratchet_official_kat_and_forward_secrecy() {
        // Официальный KAT вектор mlspp::HashRatchet (davey / libdave)
        let secret: Vec<u8> = vec![
            206, 221, 97, 177, 184, 161, 202, 105, 4, 101, 84, 40, 44, 247, 11, 123,
        ];
        let mut ratchet = DaveHashRatchet::new(secret.clone()).unwrap();

        let (key0, nonce0) = ratchet.get(0).unwrap();
        assert_eq!(
            key0,
            [
                117, 48, 249, 169, 148, 94, 45, 46, 6, 208, 101, 31, 123, 42, 134, 75
            ]
        );
        assert_eq!(nonce0, [48, 30, 95, 75, 116, 9, 15, 152, 94, 114, 107, 178]);
        assert_eq!(ratchet.next_generation(), 1);

        // Повторное чтение поколения 0 из кэша возвращает идентичную пару
        assert_eq!(ratchet.get(0).unwrap(), (key0, nonce0));

        // Внеочередной прыжок на поколение 5 кэширует промежуточные поколения 1..=5
        let (key5, nonce5) = ratchet.get(5).unwrap();
        assert_ne!(key5, key0);
        assert_ne!(nonce5, nonce0);
        let (key2, _) = ratchet.get(2).unwrap();

        // Второй независимый рэтчет при последовательном шаге даёт идентичные ключи 2 и 5
        let mut seq_ratchet = DaveHashRatchet::new(secret).unwrap();
        for g in 0..=5 {
            let (k, n) = seq_ratchet.get(g).unwrap();
            if g == 2 {
                assert_eq!(k, key2);
            }
            if g == 5 {
                assert_eq!((k, n), (key5, nonce5));
            }
        }

        // Forward Secrecy: удаление ключей до поколения 3 безвозвратно стирает поколения 0, 1, 2
        ratchet.delete_keys_up_to(3);
        assert!(matches!(
            ratchet.get(0),
            Err(DaveError::KeyExpired { generation: 0, .. })
        ));
        assert!(matches!(
            ratchet.get(2),
            Err(DaveError::KeyExpired { generation: 2, .. })
        ));
        // Поколение 3 остаётся доступным
        assert!(ratchet.get(3).is_ok());

        // Граница DAVE_MAX_GENERATION_GAP = 250 (next_generation == 6 -> 6 + 250 = 256 допустимо, 257 отклоняется)
        assert_eq!(ratchet.next_generation(), 6);
        assert!(matches!(
            ratchet.get(6 + DAVE_MAX_GENERATION_GAP + 1),
            Err(DaveError::GenerationGapExceeded { .. })
        ));
        assert!(ratchet.get(6 + DAVE_MAX_GENERATION_GAP).is_ok());

        // Пустой ключ запрещён
        assert!(matches!(
            DaveHashRatchet::new(Vec::<u8>::new()),
            Err(DaveError::InvalidKeyLength { .. })
        ));

        // Наблюдаемость / безопасность: Debug маскирует секреты
        let dbg = format!("{:?}", ratchet);
        assert!(dbg.contains("[REDACTED]"));
        assert!(!dbg.contains("117, 48, 249"));
    }

    #[test]
    fn test_dave_mls_key_schedule_epochs_confirmation_and_exporter() {
        let init0 = [0x42u8; 32];
        let commit0 = [0x99u8; 32];
        let ctx0 = sample_group_context(0, 0xA1);

        let mut ks = DaveMlsKeySchedule::from_initial_epoch(init0, &commit0, &ctx0, None).unwrap();
        assert_eq!(ks.current_epoch(), 0);
        let s0 = ks.current_secrets().clone();

        // Все производные секреты эпохи различны и ненулевые
        assert_ne!(s0.joiner_secret, [0u8; 32]);
        assert_ne!(s0.welcome_secret, s0.epoch_secret);
        assert_ne!(s0.exporter_secret, s0.epoch_authenticator);
        assert_ne!(s0.confirmation_key, s0.membership_key);
        assert_ne!(s0.init_secret, init0);

        // Проверка эквивалентности входа через MLS Welcome (joiner_secret + GroupContext)
        let ks_welcome =
            DaveMlsKeySchedule::from_welcome_joiner(s0.joiner_secret, &ctx0, None).unwrap();
        assert_eq!(ks_welcome.current_secrets(), &s0);

        // Проверка confirmation_tag (позитивный и негативный с подделкой 1 бита)
        let tag = s0.compute_confirmation_tag(&ctx0.confirmed_transcript_hash);
        assert!(
            s0.verify_confirmation_tag(&ctx0.confirmed_transcript_hash, &tag)
                .is_ok()
        );
        let mut bad_tag = tag;
        bad_tag[15] ^= 0x01;
        assert_eq!(
            s0.verify_confirmation_tag(&ctx0.confirmed_transcript_hash, &bad_tag),
            Err(DaveError::ConfirmationTagMismatch)
        );
        assert_eq!(
            s0.verify_confirmation_tag(&ctx0.confirmed_transcript_hash, &tag[..31]),
            Err(DaveError::ConfirmationTagMismatch)
        );

        // Изоляция экспортёра медиа-ключей участников по граничным Snowflake (0, 1001, u64::MAX)
        let k_zero = s0.export_user_media_secret(0).unwrap();
        let k_alice = s0.export_user_media_secret(1001).unwrap();
        let k_max = s0.export_user_media_secret(u64::MAX).unwrap();
        assert_ne!(k_zero, k_alice);
        assert_ne!(k_alice, k_max);
        assert_ne!(k_zero, k_max);

        // Голосовой код приватности (30 цифр = 6 групп по 5 цифр) + независимый оракул
        let mut known_auth = [0u8; 32];
        known_auth[4] = 7; // группа 0 = 00007
        known_auth[8] = 1;
        known_auth[9] = 0; // группа 1 = 256 -> 00256
        known_auth[29] = 99; // группа 5 = 00099
        let code_known = generate_dave_displayable_code(&known_auth, 30, 5).unwrap();
        assert_eq!(code_known.len(), 30);
        assert_eq!(&code_known[0..5], "00007");
        assert_eq!(&code_known[5..10], "00256");
        assert_eq!(&code_known[25..30], "00099");
        assert!(generate_dave_displayable_code(&known_auth[..10], 30, 5).is_err());
        assert!(generate_dave_displayable_code(&known_auth, 30, 9).is_err());

        let code0 = s0.voice_privacy_code().unwrap();
        assert_eq!(code0.len(), 30);
        assert!(code0.chars().all(|c| c.is_ascii_digit()));
        let formatted0 = s0.formatted_voice_privacy_code().unwrap();
        assert_eq!(formatted0.split(' ').count(), 6);

        // Продвижение расписания ключей эпох 0 -> 1 -> 2 меняет все ключи и код приватности
        let ctx1 = sample_group_context(1, 0xB2);
        let commit1 = [0x77u8; 32];
        ks.advance_epoch(&commit1, &ctx1, None).unwrap();
        assert_eq!(ks.current_epoch(), 1);
        let s1 = ks.current_secrets().clone();
        assert_ne!(s1.epoch_secret, s0.epoch_secret);
        assert_ne!(s1.export_user_media_secret(1001).unwrap(), k_alice);
        assert_ne!(s1.voice_privacy_code().unwrap(), code0);

        // Секреты замаскированы в Debug
        let dbg_secrets = format!("{:?}", s1);
        assert!(dbg_secrets.contains("[REDACTED]"));
    }

    #[test]
    fn test_dave_aes128_gcm_truncated8_matches_rustcrypto_oracle() {
        use aes_gcm::{Aes128Gcm, aead::Payload};

        let key_bytes = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let nonce_bytes = [0, 0, 0, 0, 0, 0, 0, 0, 0x78, 0x56, 0x34, 0x12];
        let aad = b"discord-dave-aad-header";

        let oracle = Aes128Gcm::new_from_slice(&key_bytes).unwrap();
        let dave_aead = Aes128GcmTruncated8::new(&key_bytes).unwrap();

        for len in [0usize, 1, 15, 16, 17, 80, 1275] {
            let plaintext: Vec<u8> = (0..len).map(|i| ((i * 37 + 13) & 0xFF) as u8).collect();
            let oracle_out = oracle
                .encrypt(
                    Nonce::from_slice(&nonce_bytes),
                    Payload {
                        msg: &plaintext,
                        aad,
                    },
                )
                .unwrap();
            let (oracle_ct, oracle_full_tag) = oracle_out.split_at(len);

            let mut buf = plaintext.clone();
            let tag8 = dave_aead.encrypt_in_place_detached(&nonce_bytes, aad, &mut buf);
            assert_eq!(
                buf.as_slice(),
                oracle_ct,
                "Несовпадение шифротекста при len={len}"
            );
            assert_eq!(
                &tag8[..],
                &oracle_full_tag[..8],
                "8-байтный тег DAVE обязан совпадать с первыми 8 байтами AES-128-GCM при len={len}"
            );

            // Обратная расшифровка и проверка подделки
            dave_aead
                .decrypt_in_place_detached(&nonce_bytes, aad, &mut buf, &tag8)
                .unwrap();
            assert_eq!(buf, plaintext);

            let mut tampered_tag = tag8;
            tampered_tag[7] ^= 0x01;
            let mut ct_copy = oracle_ct.to_vec();
            assert_eq!(
                dave_aead.decrypt_in_place_detached(&nonce_bytes, aad, &mut ct_copy, &tampered_tag),
                Err(DaveError::AeadAuthenticationFailed)
            );
        }
    }

    #[test]
    fn test_dave_frame_encryptor_decryptor_silence_replay_and_wrap() {
        let base_secret = [0x5Au8; 16];
        let enc_ratchet = DaveHashRatchet::new(base_secret.to_vec()).unwrap();
        let dec_ratchet = DaveHashRatchet::new(base_secret.to_vec()).unwrap();

        let mut encryptor = DaveFrameEncryptor::new(enc_ratchet);
        let mut decryptor = DaveFrameDecryptor::new(dec_ratchet);

        // 1. Пакет тишины Opus ([0xF8, 0xFF, 0xFE]) проходит прозрачно без маркера 0xFAFA
        let enc_silence = encryptor.encrypt_opus(&DAVE_OPUS_SILENCE_PACKET).unwrap();
        assert_eq!(enc_silence, DAVE_OPUS_SILENCE_PACKET);
        let dec_silence = decryptor.decrypt_opus(&enc_silence).unwrap();
        assert_eq!(dec_silence, DAVE_OPUS_SILENCE_PACKET);

        // 2. Шифрование серии пакетов речи и внеочередная доставка с защитой от повторов (replay)
        let f1 = encryptor.encrypt_opus(&[0x10, 0x20, 0x30, 0x40]).unwrap();
        let f2 = encryptor.encrypt_opus(&[0x50, 0x60, 0x70, 0x80]).unwrap();
        let f3 = encryptor.encrypt_opus(&[0x90, 0xA0, 0xB0, 0xC0]).unwrap();
        assert_eq!(&f1[f1.len() - 2..], &DAVE_MARKER_BYTES);

        // Доставка вне очереди: сначала f3, затем f1, затем f2
        assert_eq!(
            decryptor.decrypt_opus(&f3).unwrap(),
            vec![0x90, 0xA0, 0xB0, 0xC0]
        );
        assert_eq!(
            decryptor.decrypt_opus(&f1).unwrap(),
            vec![0x10, 0x20, 0x30, 0x40]
        );
        assert_eq!(
            decryptor.decrypt_opus(&f2).unwrap(),
            vec![0x50, 0x60, 0x70, 0x80]
        );

        // Повтор (replay) любого уже принятого фрейма отклоняется
        assert!(matches!(
            decryptor.decrypt_opus(&f3),
            Err(DaveError::ReplayOrExpiredNonce { .. })
        ));
        assert!(matches!(
            decryptor.decrypt_opus(&f1),
            Err(DaveError::ReplayOrExpiredNonce { .. })
        ));

        // 3. Нешифрованный не-тихий пакет при выключенном passthrough строго отклоняется
        assert_eq!(
            decryptor.decrypt_opus(&[0x01, 0x02, 0x03, 0x04, 0x05]),
            Err(DaveError::UnencryptedFrameRejected)
        );

        // 4. Оборачивание 8-битного поколения 255 -> 256 (compute_wrapped_generation)
        assert_eq!(dave_compute_wrapped_generation(0, 0), 0);
        assert_eq!(dave_compute_wrapped_generation(0, 250), 250);
        assert_eq!(dave_compute_wrapped_generation(250, 0), 256);
        assert_eq!(dave_compute_wrapped_generation(255, 2), 258);

        let wrap_secret = [0x33u8; 16];
        let mut wrap_enc =
            DaveFrameEncryptor::new(DaveHashRatchet::new(wrap_secret.to_vec()).unwrap());
        let mut wrap_dec =
            DaveFrameDecryptor::new(DaveHashRatchet::new(wrap_secret.to_vec()).unwrap());

        // Проходим поколения шагами по 200 до 255 -> 256 без превышения MAX_GENERATION_GAP=250
        wrap_enc.set_truncated_nonce((200u32 << 24) - 1);
        let p_gen200 = wrap_enc.encrypt_opus(b"gen-200").unwrap();
        assert_eq!(wrap_dec.decrypt_opus(&p_gen200).unwrap(), b"gen-200");

        wrap_enc.set_truncated_nonce(0xFFFF_FFFF); // инкремент -> 0x0000_0000 (wrapped generation 256!)
        let p_gen256 = wrap_enc.encrypt_opus(b"gen-256-after-wrap").unwrap();
        assert_eq!(
            wrap_dec.decrypt_opus(&p_gen256).unwrap(),
            b"gen-256-after-wrap"
        );
    }

    #[test]
    fn test_dave_session_epoch_transition_and_downgrade_rejection() {
        let init_secret = [0x17u8; 32];
        let commit_e1 = [0x28u8; 32];
        let ctx_e1 = sample_group_context(1, 0x31);
        let members = [1001u64, 2002u64];

        let ks_alice =
            DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit_e1, &ctx_e1, None).unwrap();
        let ks_bob =
            DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit_e1, &ctx_e1, None).unwrap();

        let mut alice = DaveVoiceSession::new(
            1001,
            ks_alice,
            &members,
            DaveTransportCipherMode::AeadAes256GcmRtpsize,
        )
        .unwrap();
        let mut bob = DaveVoiceSession::new(
            2002,
            ks_bob,
            &members,
            DaveTransportCipherMode::AeadAes256GcmRtpsize,
        )
        .unwrap();

        assert_eq!(
            alice.voice_privacy_code().unwrap(),
            bob.voice_privacy_code().unwrap()
        );

        // Двусторонний обмен в эпохе 1
        let msg_a_e1 = alice.encrypt_opus(b"hello-bob-epoch-1").unwrap();
        let in_flight_a_e1 = alice.encrypt_opus(b"inflight-before-transition").unwrap();
        assert_eq!(
            bob.decrypt_opus(1001, &msg_a_e1).unwrap(),
            b"hello-bob-epoch-1"
        );

        // Неизвестный отправитель отклоняется
        assert_eq!(
            bob.decrypt_opus(9999, &msg_a_e1),
            Err(DaveError::MissingKeyRatchet { user_id: 9999 })
        );

        // Неподготовленный transition_id отклоняется
        assert_eq!(
            bob.execute_transition(77),
            Err(DaveError::TransitionNotPrepared { transition_id: 77 })
        );

        // Двухфазная ротация эпохи 1 -> 2 (op:21/29 Prepare -> op:22 Execute)
        let commit_e2 = [0x66u8; 32];
        let ctx_e2 = sample_group_context(2, 0x52);
        alice
            .prepare_epoch_transition(12, &commit_e2, &ctx_e2, None, &members)
            .unwrap();
        bob.prepare_epoch_transition(12, &commit_e2, &ctx_e2, None, &members)
            .unwrap();

        assert_eq!(bob.execute_transition(12).unwrap(), 2);
        assert_eq!(alice.execute_transition(12).unwrap(), 2);

        // Задержавшийся в сети пакет из эпохи 1 всё ещё штатно расшифровывается Бобом после перехода на эпоху 2!
        assert_eq!(
            bob.decrypt_opus(1001, &in_flight_a_e1).unwrap(),
            b"inflight-before-transition"
        );

        // Новые пакеты эпохи 2 шифруются новыми рэтчетами и успешно расшифровываются
        let msg_a_e2 = alice.encrypt_opus(b"hello-bob-epoch-2").unwrap();
        assert_eq!(
            bob.decrypt_opus(1001, &msg_a_e2).unwrap(),
            b"hello-bob-epoch-2"
        );

        // Защита от downgrade-атаки (protocol_version = 0 запрещён при активной сессии v1)
        assert_eq!(
            alice.negotiate_protocol_version(0),
            Err(DaveError::DowngradeRejected {
                current_version: 1,
                requested_version: 0,
            })
        );
    }

    #[tokio::test]
    async fn test_two_layer_rtp_srtp_rtpsize_and_voice_udp_streamer_dave_e2e() {
        let init_secret = [0x88u8; 32];
        let commit = [0x44u8; 32];
        let ctx = sample_group_context(1, 0x77);
        let members = [1001u64, 2002u64];
        let srtp_key = [0xABu8; 32];

        // 1. Точная побайтовая проверка раскладки re-deep/voice-deep.md §2.3:
        // 12B RTP + 12B RFC 5285 0xBEDE (2 слова) = 24B clear AAD + 3B DTX silence + 16B GCM tag + 4B LE counter = 47 Б!
        let rtp_ext_hdr = DiscordVoicePacket::make_rtp_header_with_bede_ext(
            42,
            960,
            9208,
            &[[0x10, 0xFF, 0x00, 0x00], [0x21, 0x01, 0x02, 0x00]],
        );
        assert_eq!(rtp_ext_hdr.len(), 24);
        assert_eq!(&rtp_ext_hdr[12..16], &[0xBE, 0xDE, 0x00, 0x02]);
        assert_eq!(
            DiscordVoicePacket::parse_rtpsize_header_len(&rtp_ext_hdr).unwrap(),
            24
        );

        for mode in [
            DaveTransportCipherMode::AeadAes256GcmRtpsize,
            DaveTransportCipherMode::AeadXChaCha20Poly1305Rtpsize,
        ] {
            let mut sender_sess = DaveVoiceSession::new(
                1001,
                DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit, &ctx, None).unwrap(),
                &members,
                mode,
            )
            .unwrap();
            let mut receiver_sess = DaveVoiceSession::new(
                2002,
                DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit, &ctx, None).unwrap(),
                &members,
                mode,
            )
            .unwrap();

            // Пакет тишины с 2-словным 0xBEDE расширением обязан давать ровно 47 байт и счётчик 0x80000000 | 0
            let wire_silence = DiscordVoicePacket::seal_rtp_with_dave(
                mode,
                &srtp_key,
                &rtp_ext_hdr,
                &DAVE_OPUS_SILENCE_PACKET,
                0,
                &mut sender_sess,
            )
            .unwrap();
            assert_eq!(wire_silence.len(), 47);
            assert_eq!(
                &wire_silence[43..47],
                &SRTP_OUTBOUND_COUNTER_MSB.to_le_bytes()
            );

            let (clear_hdr, dec_silence, counter_val) = DiscordVoicePacket::open_rtp_with_dave(
                mode,
                &srtp_key,
                &wire_silence,
                1001,
                &mut receiver_sess,
            )
            .unwrap();
            assert_eq!(clear_hdr, rtp_ext_hdr);
            assert_eq!(dec_silence, DAVE_OPUS_SILENCE_PACKET);
            assert_eq!(counter_val, SRTP_OUTBOUND_COUNTER_MSB);

            // Полноценный голосовой фрейм Opus (80 байт): маркер 0xFAFA скрыт под внешним слоем SRTP
            let speech_opus = vec![0x6B; 80];
            let mut wire_speech = DiscordVoicePacket::seal_rtp_with_dave(
                mode,
                &srtp_key,
                &rtp_ext_hdr,
                &speech_opus,
                1,
                &mut sender_sess,
            )
            .unwrap();
            let (_, dec_speech, c2) = DiscordVoicePacket::open_rtp_with_dave(
                mode,
                &srtp_key,
                &wire_speech,
                1001,
                &mut receiver_sess,
            )
            .unwrap();
            assert_eq!(dec_speech, speech_opus);
            assert_eq!(c2, SRTP_OUTBOUND_COUNTER_MSB | 1);

            // Модификация байта в открытом расширении 0xBEDE ломает AAD транспортного AEAD
            wire_speech[18] ^= 0x80;
            assert_eq!(
                DiscordVoicePacket::open_rtp_with_dave(
                    mode,
                    &srtp_key,
                    &wire_speech,
                    1001,
                    &mut receiver_sess,
                ),
                Err(DaveError::AeadAuthenticationFailed)
            );
        }

        // 2. Сквозной UDP E2E обмен между двумя VoiceUdpStreamer с активными сессиями DAVE v1
        let recv_probe = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let recv_addr = recv_probe.local_addr().unwrap();
        drop(recv_probe);

        let receiver_sock = tokio::net::UdpSocket::bind(recv_addr).await.unwrap();
        let sender_streamer = VoiceUdpStreamer::bind_and_connect(VoiceUdpConfig {
            remote_addr: recv_addr,
            ssrc: 9208,
            secret_key: srtp_key,
        })
        .await
        .unwrap();

        let sender_local_addr = sender_streamer.socket.local_addr().unwrap();
        receiver_sock.connect(sender_local_addr).await.unwrap();
        let receiver_streamer = VoiceUdpStreamer {
            socket: Arc::new(receiver_sock),
            config: VoiceUdpConfig {
                remote_addr: sender_local_addr,
                ssrc: 17843,
                secret_key: srtp_key,
            },
            sequence: Arc::new(Mutex::new(0)),
            timestamp: Arc::new(Mutex::new(0)),
            keepalive_seq: Arc::new(Mutex::new(0)),
            srtp_nonce_counter: Arc::new(Mutex::new(0)),
            dave_session: Arc::new(Mutex::new(None)),
            stats: Arc::new(Mutex::new(VoiceStreamStats::default())),
            bitrate_controller: Arc::new(Mutex::new(OpusBitrateController::new())),
            jitter_buffer: Arc::new(Mutex::new(AdaptiveJitterBuffer::new())),
            is_running: Arc::new(AtomicBool::new(true)),
        };

        sender_streamer
            .enable_dave_session(
                DaveVoiceSession::new(
                    1001,
                    DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit, &ctx, None)
                        .unwrap(),
                    &members,
                    DaveTransportCipherMode::AeadAes256GcmRtpsize,
                )
                .unwrap(),
            )
            .await;
        receiver_streamer
            .enable_dave_session(
                DaveVoiceSession::new(
                    2002,
                    DaveMlsKeySchedule::from_initial_epoch(init_secret, &commit, &ctx, None)
                        .unwrap(),
                    &members,
                    DaveTransportCipherMode::AeadAes256GcmRtpsize,
                )
                .unwrap(),
            )
            .await;

        let opus_frame = vec![0x48, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        let ext_words = [[0x10, 0xAA, 0x00, 0x00], [0x21, 0xBB, 0xCC, 0x00]];
        let sent_len = sender_streamer
            .send_dave_audio_packet(&opus_frame, Some(&ext_words))
            .await
            .unwrap();
        assert!(sent_len > 47);

        let received_opus = receiver_streamer
            .recv_dave_audio_packet(1001)
            .await
            .unwrap();
        assert_eq!(received_opus, opus_frame);

        let sender_stats = sender_streamer.stats.lock().await.clone();
        let receiver_stats = receiver_streamer.stats.lock().await.clone();
        assert_eq!(sender_stats.dave_frames_encrypted, 1);
        assert_eq!(sender_stats.dave_epoch, 1);
        assert_eq!(receiver_stats.dave_frames_decrypted, 1);
        assert_eq!(receiver_stats.dave_epoch, 1);
    }
}
