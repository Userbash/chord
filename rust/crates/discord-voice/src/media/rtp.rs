//! RTP-заголовок и расширение RFC 5285 для медиа-канала Discord (P4-04).
//!
//! Раскладка снята с golden-capture `voice-udp4.pcap` (10 пакетов PT120,
//! `fixtures/voice/rtp-pt120-oracle.json`, `docs/re/voice-deep.md` §2.3):
//! 12 Б фиксированного заголовка (`V=2, P=0, X=1, CC=0`), ext-заголовок `0xBEDE`
//! с 2 словами (8 Б) данных, затем SRTP-хвост `[ciphertext][tag 16][counter 4 LE]`.
//!
//! Модуль только раскладывает байты: шифрование — P4-03, сокет и ротация — P4-02.
//! Никаких `unsafe`, паник и сетевых операций в продакшн-пути нет.

use std::fmt;

/// Payload type Opus в медиа-канале (Select Protocol, priority 1000).
pub const OPUS_PAYLOAD_TYPE: u8 = 120;

/// Частота дискретизации Opus/RTP для голосового канала.
pub const OPUS_CLOCK_RATE: u32 = 48_000;

/// Длительность одного кадра, мс.
pub const FRAME_DURATION_MS: u32 = 20;

/// Тиков RTP на кадр: `OPUS_CLOCK_RATE * FRAME_DURATION_MS / 1000`.
pub const FRAME_TICKS: u32 = 960;

/// Профиль one-byte-расширения RFC 5285 из capture.
pub const EXTENSION_PROFILE: u16 = 0xBEDE;

/// Минимальная длина RTP-заголовка (фиксированная часть, 12 Б).
pub const RTP_HEADER_LEN: usize = 12;

/// Длина ext-заголовка RFC 5285 (profile + words).
pub const EXTENSION_HEADER_LEN: usize = 4;

/// Единица длины расширения — слово по 4 Б.
const EXTENSION_WORD: usize = 4;

/// Максимальная длина данных расширения в этой карточке (15 слов).
const MAX_EXTENSION_BYTES: usize = 60;

/// Максимальная длина шифротекста comfort-noise (эмпирика capture, §2.3).
const COMFORT_NOISE_MAX_ENCRYPTED_LEN: usize = 3;

/// Fixed RTP header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpHeader {
    pub marker: bool,
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

impl RtpHeader {
    /// Кодирует 12 Б заголовка и, если задано, ext-заголовок с данными.
    ///
    /// `ext = Some(data)`: byte0 = `0x90` (X=1), затем `0xBEDE`, `words = len/4` BE
    /// и сами данные. Длина 0 допустима (words = 0), иначе `len % 4 != 0` или
    /// `len > 60` → [`RtpError::ExtensionNotAligned`].
    pub fn encode(&self, ext: Option<&[u8]>) -> Result<Vec<u8>, RtpError> {
        if let Some(data) = ext
            && (!data.len().is_multiple_of(EXTENSION_WORD) || data.len() > MAX_EXTENSION_BYTES)
        {
            return Err(RtpError::ExtensionNotAligned);
        }
        let extension_len = if ext.is_some() {
            EXTENSION_HEADER_LEN
        } else {
            0
        };
        let mut bytes =
            Vec::with_capacity(RTP_HEADER_LEN + extension_len + ext.map_or(0, <[u8]>::len));
        // V=2, P=0, X=ext.is_some(), CC=0.
        bytes.push(0x80 | if ext.is_some() { 0x10 } else { 0 });
        bytes.push((u8::from(self.marker) << 7) | (self.payload_type & 0x7f));
        bytes.extend_from_slice(&self.sequence.to_be_bytes());
        bytes.extend_from_slice(&self.timestamp.to_be_bytes());
        bytes.extend_from_slice(&self.ssrc.to_be_bytes());
        if let Some(data) = ext {
            let words = u16::try_from(data.len() / EXTENSION_WORD)
                .map_err(|_| RtpError::ExtensionNotAligned)?;
            bytes.extend_from_slice(&EXTENSION_PROFILE.to_be_bytes());
            bytes.extend_from_slice(&words.to_be_bytes());
            bytes.extend_from_slice(data);
        }
        Ok(bytes)
    }
}

/// Разобранное расширение RTP-заголовка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpExtension<'a> {
    /// Профиль из ext-заголовка (в capture `0xBEDE`; аномалии — `0x19f5`/`0x2d6d`).
    pub profile: u16,
    /// Данные расширения как они лежат в пакете.
    pub data: &'a [u8],
    /// `true`, если профиль — `0xBEDE` (стандартный one-byte RFC 5285).
    pub standard: bool,
}

/// Разобранный RTP-пакет.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpPacket<'a> {
    pub header: RtpHeader,
    pub extension: Option<RtpExtension<'a>>,
    pub payload: &'a [u8],
}

/// Ошибка разбора/кодирования RTP-заголовка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpError {
    /// Пакет короче 12 Б (или обрезан на CSRC).
    TooShort,
    /// Поле версии не равно 2.
    BadVersion,
    /// Ext-заголовок отсутствует или заявленная длина выходит за границу пакета.
    TruncatedExtension,
    /// Паддинг равен 0 или больше payload.
    PaddingOutOfRange,
    /// Длина расширения не кратна 4 или больше 60 Б.
    ExtensionNotAligned,
}

impl RtpError {
    /// Короткое описание для логов и сообщений CLI.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TooShort => "rtp packet too short",
            Self::BadVersion => "rtp version is not 2",
            Self::TruncatedExtension => "rtp extension is truncated",
            Self::PaddingOutOfRange => "rtp padding is out of range",
            Self::ExtensionNotAligned => "rtp extension is not 4-byte aligned or too long",
        }
    }
}

impl fmt::Display for RtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for RtpError {}

/// Разбирает RTP-пакет: фиксированный заголовок, CSRC, расширение и паддинг.
///
/// Ошибки: [`RtpError::TooShort`] (< 12 Б или обрезанные CSRC),
/// [`RtpError::BadVersion`] (V != 2), [`RtpError::TruncatedExtension`] (нет
/// ext-заголовка или `words*4` выходит за пакет у профиля `0xBEDE`),
/// [`RtpError::PaddingOutOfRange`] (P=1 и последний байт 0 или больше payload).
///
/// Нестандартный профиль (`standard = false`) при `words*4` больше остатка пакета
/// не ошибка: в capture Discord (`0x19f5`/`0x2d6d`) поле words не является длиной
/// (59783/28435 при 43-байтовом пакете), поэтому весь хвост отдаётся как opaque
/// данные расширения, а payload пуст. Для `0xBEDE` такое же расхождение —
/// `TruncatedExtension`.
pub fn parse_rtp(bytes: &[u8]) -> Result<RtpPacket<'_>, RtpError> {
    if bytes.len() < RTP_HEADER_LEN {
        return Err(RtpError::TooShort);
    }
    let first = bytes[0];
    if first >> 6 != 2 {
        return Err(RtpError::BadVersion);
    }
    let has_padding = first & 0x20 != 0;
    let has_extension = first & 0x10 != 0;
    let csrc_count = usize::from(first & 0x0f);
    let header = RtpHeader {
        marker: bytes[1] & 0x80 != 0,
        payload_type: bytes[1] & 0x7f,
        sequence: u16::from_be_bytes([bytes[2], bytes[3]]),
        timestamp: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        ssrc: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
    };

    let csrc_end = RTP_HEADER_LEN + csrc_count * EXTENSION_WORD;
    if csrc_end > bytes.len() {
        return Err(RtpError::TooShort);
    }
    let mut offset = csrc_end;
    let mut payload_end = bytes.len();
    let mut extension = None;

    if has_extension {
        if bytes.len() < offset + EXTENSION_HEADER_LEN {
            return Err(RtpError::TruncatedExtension);
        }
        let profile = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        let words = usize::from(u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]));
        let data_start = offset + EXTENSION_HEADER_LEN;
        let declared = words * EXTENSION_WORD;
        let available = bytes.len() - data_start;
        let standard = profile == EXTENSION_PROFILE;
        let data = if declared <= available {
            &bytes[data_start..data_start + declared]
        } else if standard {
            return Err(RtpError::TruncatedExtension);
        } else {
            // Opaque-хвост нестандартного профиля: words не является длиной,
            // payload в таком пакете не выделяется (см. doc парсера).
            &bytes[data_start..]
        };
        offset = data_start + data.len();
        extension = Some(RtpExtension {
            profile,
            data,
            standard,
        });
    }

    if has_padding {
        if payload_end <= offset {
            return Err(RtpError::PaddingOutOfRange);
        }
        let padding = usize::from(bytes[payload_end - 1]);
        if padding == 0 || padding > payload_end - offset {
            return Err(RtpError::PaddingOutOfRange);
        }
        payload_end -= padding;
    }

    Ok(RtpPacket {
        header,
        extension,
        payload: &bytes[offset..payload_end],
    })
}

/// Исходящий RTP-поток Opus: sequence/timestamp и одноразовый marker talkspurt.
#[derive(Debug, Clone)]
pub struct RtpSender {
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    marker_pending: bool,
}

impl RtpSender {
    /// Новый поток: `sequence = 0`, `timestamp = 0`, marker взведён.
    #[must_use]
    pub const fn new(ssrc: u32) -> Self {
        Self {
            ssrc,
            sequence: 0,
            timestamp: 0,
            marker_pending: true,
        }
    }

    /// Текущий (ещё не отправленный) sequence.
    #[must_use]
    pub const fn sequence(&self) -> u16 {
        self.sequence
    }

    /// Текущая (ещё не отправленная) метка времени.
    #[must_use]
    pub const fn timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Собирает пакет: заголовок + расширение + payload.
    ///
    /// На успешном пакете `marker = marker_pending`, затем marker снимается,
    /// `sequence += 1`, `timestamp += 960`. При ошибке кодирования расширения
    /// состояние не двигается.
    pub fn next_packet(&mut self, payload: &[u8], ext: Option<&[u8]>) -> Result<Vec<u8>, RtpError> {
        let header = RtpHeader {
            marker: self.marker_pending,
            payload_type: OPUS_PAYLOAD_TYPE,
            sequence: self.sequence,
            timestamp: self.timestamp,
            ssrc: self.ssrc,
        };
        let mut bytes = header.encode(ext)?;
        bytes.extend_from_slice(payload);
        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(FRAME_TICKS);
        self.marker_pending = false;
        Ok(bytes)
    }

    /// DTX: двигает только timestamp на `960 * elapsed_frames` и взводит marker.
    ///
    /// `sequence` не растёт — пакеты за время тишины не отправлялись.
    pub fn silence(&mut self, elapsed_frames: u32) {
        self.timestamp = self
            .timestamp
            .wrapping_add(FRAME_TICKS.wrapping_mul(elapsed_frames));
        self.marker_pending = true;
    }
}

/// Вид аудиокадра по длине шифротекста (без SRTP-трейлера).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFrameKind {
    /// Пакет без полезной нагрузки.
    Empty,
    /// Comfort-noise / DTX-кадр.
    ComfortNoise,
    /// Речевой кадр.
    Speech,
}

/// Классифицирует длину шифротекста Opus: `0` → Empty, `1..=3` → CN, иначе речь.
///
/// Порог 3 Б — эмпирика capture: единственные медиа-пакеты talkspurt содержали
/// 3-байтовый шифротекст comfort-noise (`docs/re/voice-deep.md` §2.3).
#[must_use]
pub fn classify_opus_payload(encrypted_len: usize) -> AudioFrameKind {
    match encrypted_len {
        0 => AudioFrameKind::Empty,
        1..=COMFORT_NOISE_MAX_ENCRYPTED_LEN => AudioFrameKind::ComfortNoise,
        _ => AudioFrameKind::Speech,
    }
}
