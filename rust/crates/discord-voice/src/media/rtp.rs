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

/// Профиль one-byte-расширения RFC 5285 / RFC 8285 из capture.
pub const EXTENSION_PROFILE: u16 = 0xBEDE;

/// Базовый профиль two-byte-расширения RFC 8285 (`0x1000..=0x100F`, младшие 4 бита — `appbits`).
pub const TWO_BYTE_EXTENSION_PROFILE: u16 = 0x1000;

/// Маска старших 12 бит профиля two-byte-расширения RFC 8285.
pub const TWO_BYTE_EXTENSION_MASK: u16 = 0xFFF0;

/// Проверяет, принадлежит ли `profile` диапазону RFC 8285 Two-Byte Header (`0x1000..=0x100F`).
#[must_use]
pub const fn is_two_byte_extension_profile(profile: u16) -> bool {
    (profile & TWO_BYTE_EXTENSION_MASK) == TWO_BYTE_EXTENSION_PROFILE
}

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
        let standard = profile == EXTENSION_PROFILE || is_two_byte_extension_profile(profile);
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

/// Формат заголовка элементов расширения RTP по RFC 5285 / RFC 8285.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpExtensionFormat {
    /// Однобайтовый заголовок `0xBEDE` (`id: 1..=14`, `len: 1..=16`).
    OneByte,
    /// Двухбайтовый заголовок `0x1000..=0x100F` (`id: 1..=255`, `len: 0..=255`, 4 бита `appbits`).
    TwoByte { appbits: u8 },
}

/// Отдельный элемент расширения заголовка RTP (`RFC 5285` / `RFC 8285`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpExtensionElement<'a> {
    /// Локальный идентификатор расширения (`1..=14` для One-Byte, `1..=255` для Two-Byte).
    pub id: u8,
    /// Срез полезных данных элемента расширения.
    pub data: &'a [u8],
}

/// Ошибка разбора/кодирования элементов расширения заголовка RTP (`RFC 5285` / `RFC 8285`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtpExtensionError {
    /// Профиль расширения не относится к `0xBEDE` или `0x1000..=0x100F`.
    UnsupportedProfile(u16),
    /// Зарезервированный или недопустимый `id` элемента (например `0` с ненулевой длиной или `>14` при кодировании One-Byte).
    ReservedElementId(u8),
    /// Недопустимая длина данных элемента при кодировании (`0` или `>16` в One-Byte, `>255` в Two-Byte).
    InvalidElementLength { id: u8, len: usize },
    /// Данные элемента обрезаны концом блока расширения RTP.
    TruncatedElement {
        id: u8,
        declared_len: usize,
        available: usize,
    },
}

impl fmt::Display for RtpExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile(profile) => {
                write!(
                    f,
                    "неподдерживаемый профиль RTP-расширения: 0x{profile:04x}"
                )
            }
            Self::ReservedElementId(id) => {
                write!(
                    f,
                    "недопустимый идентификатор элемента RTP-расширения: {id}"
                )
            }
            Self::InvalidElementLength { id, len } => {
                write!(
                    f,
                    "недопустимая длина элемента RTP-расширения id={id}: {len} Б"
                )
            }
            Self::TruncatedElement {
                id,
                declared_len,
                available,
            } => write!(
                f,
                "усечён элемент RTP-расширения id={id}: заявлено {declared_len} Б, доступно {available} Б"
            ),
        }
    }
}

impl std::error::Error for RtpExtensionError {}

/// Разбирает элементы расширения заголовка RTP в форматах RFC 5285 / RFC 8285:
/// One-Byte (`0xBEDE`) и Two-Byte (`0x1000..=0x100F`).
pub fn parse_rtp_extension_elements<'a>(
    ext: &RtpExtension<'a>,
) -> Result<(RtpExtensionFormat, Vec<RtpExtensionElement<'a>>), RtpExtensionError> {
    if ext.profile == EXTENSION_PROFILE {
        let mut elements = Vec::new();
        let mut pos = 0usize;
        while pos < ext.data.len() {
            let header_byte = ext.data[pos];
            pos += 1;
            if header_byte == 0 {
                // Байт выравнивания (padding) по RFC 8285 §4.2.
                continue;
            }
            let id = header_byte >> 4;
            let len_minus_one = header_byte & 0x0F;
            if id == 0 {
                // `id == 0` при `L != 0` запрещён стандартом RFC 8285 §4.2.
                return Err(RtpExtensionError::ReservedElementId(0));
            }
            if id == 15 {
                // RFC 5285 §4.2: локальный идентификатор 15 завершает обработку расширения.
                break;
            }
            let elem_len = usize::from(len_minus_one) + 1;
            let available = ext.data.len() - pos;
            if elem_len > available {
                return Err(RtpExtensionError::TruncatedElement {
                    id,
                    declared_len: elem_len,
                    available,
                });
            }
            elements.push(RtpExtensionElement {
                id,
                data: &ext.data[pos..pos + elem_len],
            });
            pos += elem_len;
        }
        Ok((RtpExtensionFormat::OneByte, elements))
    } else if is_two_byte_extension_profile(ext.profile) {
        let appbits = (ext.profile & 0x000F) as u8;
        let mut elements = Vec::new();
        let mut pos = 0usize;
        while pos < ext.data.len() {
            let id = ext.data[pos];
            pos += 1;
            if id == 0 {
                // Байт выравнивания (padding) в Two-Byte Header по RFC 8285 §4.3.
                continue;
            }
            if pos >= ext.data.len() {
                return Err(RtpExtensionError::TruncatedElement {
                    id,
                    declared_len: 1,
                    available: 0,
                });
            }
            let elem_len = usize::from(ext.data[pos]);
            pos += 1;
            let available = ext.data.len() - pos;
            if elem_len > available {
                return Err(RtpExtensionError::TruncatedElement {
                    id,
                    declared_len: elem_len,
                    available,
                });
            }
            elements.push(RtpExtensionElement {
                id,
                data: &ext.data[pos..pos + elem_len],
            });
            pos += elem_len;
        }
        Ok((RtpExtensionFormat::TwoByte { appbits }, elements))
    } else {
        Err(RtpExtensionError::UnsupportedProfile(ext.profile))
    }
}

/// Кодирует элементы расширения One-Byte Header (`0xBEDE`, RFC 8285 §4.2) с выравниванием до 4 байт.
pub fn encode_rtp_one_byte_extensions(
    elements: &[(u8, &[u8])],
) -> Result<Vec<u8>, RtpExtensionError> {
    let mut out = Vec::new();
    for &(id, data) in elements {
        if id == 0 || id >= 15 {
            return Err(RtpExtensionError::ReservedElementId(id));
        }
        if data.is_empty() || data.len() > 16 {
            return Err(RtpExtensionError::InvalidElementLength {
                id,
                len: data.len(),
            });
        }
        let header_byte = (id << 4) | ((data.len() - 1) as u8);
        out.push(header_byte);
        out.extend_from_slice(data);
    }
    while !out.len().is_multiple_of(EXTENSION_WORD) {
        out.push(0);
    }
    Ok(out)
}

/// Кодирует элементы расширения Two-Byte Header (`0x1000..=0x100F`, RFC 8285 §4.3) с выравниванием до 4 байт.
pub fn encode_rtp_two_byte_extensions(
    elements: &[(u8, &[u8])],
) -> Result<Vec<u8>, RtpExtensionError> {
    let mut out = Vec::new();
    for &(id, data) in elements {
        if id == 0 {
            return Err(RtpExtensionError::ReservedElementId(0));
        }
        if data.len() > usize::from(u8::MAX) {
            return Err(RtpExtensionError::InvalidElementLength {
                id,
                len: data.len(),
            });
        }
        out.push(id);
        out.push(data.len() as u8);
        out.extend_from_slice(data);
    }
    while !out.len().is_multiple_of(EXTENSION_WORD) {
        out.push(0);
    }
    Ok(out)
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

/// Стартовый префикс Annex-B для H.264 NAL-единиц (`0x00_00_00_01`).
pub const ANNEX_B_START_CODE: [u8; 4] = [0x00, 0x00, 0x00, 0x01];

/// Максимальный размер собираемого видеокадра (2 МБ + 64 КБ, по эталону Serein `MAX_FRAME_BYTES`).
pub const MAX_VIDEO_FRAME_BYTES: usize = 2 * 1024 * 1024 + 64 * 1024;

/// Максимальное количество отложенных при переупорядочивании RTP-пакетов одного кадра.
pub const MAX_PENDING_VIDEO_PACKETS: usize = 128;

/// Максимальный размер одного отложенного RTP-пакета в буфере переупорядочивания.
pub const MAX_PENDING_PACKET_BYTES: usize = 4096;

/// Максимальный суммарный объём буфера переупорядочивания видеокадра (256 КБ).
pub const MAX_PENDING_TOTAL_BYTES: usize = 256 * 1024;

/// Максимальное число отслеживаемых пропущенных `sequence_number` для RTCP NACK.
pub const MAX_NACK_SEQUENCES: usize = 128;

/// Поддерживаемые видеокодеки сборщика [`VideoFrameAssembler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCodec {
    /// H.264 (RFC 6184: Single NAL `1..=23`, STAP-A `24`, FU-A `28` -> Annex-B `0x00_00_00_01`).
    #[default]
    H264,
    /// AV1 (RTP Payload Format for AV1: OBU Aggregation Header `Z/Y/W/N` + LEB128).
    Av1,
}

/// Проверяет, содержит ли Annex-B кадр H.264 одновременно SPS (`NAL 7`) и PPS (`NAL 8`).
#[must_use]
pub fn h264_has_parameter_sets(frame: &[u8]) -> bool {
    let (mut sps, mut pps) = (false, false);
    let mut at = 0;
    while at + 4 <= frame.len() {
        let size = if frame[at..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if frame[at..].starts_with(&[0, 0, 1]) {
            3
        } else {
            at += 1;
            continue;
        };
        match frame.get(at + size).map(|nal| nal & 0x1f) {
            Some(7) => sps = true,
            Some(8) => pps = true,
            _ => {}
        }
        at += size;
    }
    sps && pps
}

/// Проверяет, содержит ли Annex-B кадр H.264 ключевой IDR-срез (`NAL 5`).
#[must_use]
pub fn h264_is_keyframe(frame: &[u8]) -> bool {
    let mut at = 0;
    while at + 4 <= frame.len() {
        let size = if frame[at..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if frame[at..].starts_with(&[0, 0, 1]) {
            3
        } else {
            at += 1;
            continue;
        };
        if frame.get(at + size).is_some_and(|nal| nal & 0x1f == 5) {
            return true;
        }
        at += size;
    }
    false
}

/// Проверяет, содержит ли собранный поток OBU ключевой заголовок последовательности (`OBU_SEQUENCE_HEADER = 1`).
#[must_use]
pub fn av1_is_keyframe(frame: &[u8]) -> bool {
    let Some(&first) = frame.first() else {
        return false;
    };
    let forbidden = (first >> 7) & 1;
    let obu_type = (first >> 3) & 0x0f;
    forbidden == 0 && obu_type == 1
}

fn read_leb128(data: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    for (index, &byte) in data.iter().take(8).enumerate() {
        let chunk = usize::from(byte & 0x7f);
        let shift = index * 7;
        value |= chunk.checked_shl(shift as u32)?;
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

/// Распаковывает пакет ретрансляции RTX по RFC 4588:
/// проверяет совпадение `rtx_ssrc` и `rtx_pt`, извлекает 16-битный оригинальный
/// номер последовательности (OSN) из первых 2 байт payload и заменяет `ssrc`/`payload_type`
/// на основной `video_ssrc`/`video_pt`.
#[must_use]
pub fn restore_rtx_packet<'a>(
    packet: &RtpPacket<'a>,
    rtx_ssrc: u32,
    rtx_pt: u8,
    video_ssrc: u32,
    video_pt: u8,
) -> Option<RtpPacket<'a>> {
    if packet.header.ssrc != rtx_ssrc || packet.header.payload_type != (rtx_pt & 0x7f) {
        return None;
    }
    if packet.payload.len() < 3 {
        return None;
    }
    let osn = u16::from_be_bytes([packet.payload[0], packet.payload[1]]);
    Some(RtpPacket {
        header: RtpHeader {
            marker: packet.header.marker,
            payload_type: video_pt & 0x7f,
            sequence: osn,
            timestamp: packet.header.timestamp,
            ssrc: video_ssrc,
        },
        extension: packet.extension.clone(),
        payload: &packet.payload[2..],
    })
}

/// Распаковывает сырые байты RTP RTX-пакета по RFC 4588 и возвращает пересобранный
/// основной RTP-пакет с `video_ssrc`, `video_pt` и восстановленным `sequence = OSN`.
pub fn restore_rtx_bytes(
    raw_rtp: &[u8],
    rtx_ssrc: u32,
    rtx_pt: u8,
    video_ssrc: u32,
    video_pt: u8,
) -> Result<Vec<u8>, RtpError> {
    let parsed = parse_rtp(raw_rtp)?;
    let restored = restore_rtx_packet(&parsed, rtx_ssrc, rtx_pt, video_ssrc, video_pt)
        .ok_or(RtpError::TooShort)?;
    let ext_data = match &restored.extension {
        Some(ext) if ext.standard => Some(ext.data),
        Some(_) => return Err(RtpError::ExtensionNotAligned),
        None => None,
    };
    let mut bytes = restored.header.encode(ext_data)?;
    bytes.extend_from_slice(restored.payload);
    Ok(bytes)
}

/// Сборщик фрагментированных видеокадров H.264 (RFC 6184) и AV1 (OBU aggregation header)
/// по аналогии с `Assembler` в `Serein/crates/discord-voice/src/video_receive.rs`.
///
/// Отслеживает пропущенные `sequence_number` для генерации RTCP NACK и взводит запрос
/// RTCP PLI при разрыве последовательности, сбросе неполного кадра или ожидании ключевого кадра.
#[derive(Debug, Clone)]
pub struct VideoFrameAssembler {
    codec: VideoCodec,
    timestamp: u32,
    next_sequence: Option<u16>,
    frame: Vec<u8>,
    fragmenting: bool,
    broken: bool,
    started: bool,
    lost: bool,
    awaiting_keyframe: bool,
    av1_frame_new_sequence: bool,
    pending: Vec<(u16, bool, Vec<u8>)>,
    pending_bytes: usize,
    missing_sequences: Vec<u16>,
}

impl Default for VideoFrameAssembler {
    fn default() -> Self {
        Self::new(VideoCodec::H264)
    }
}

impl VideoFrameAssembler {
    /// Создаёт сборщик для указанного видеокодека (`H264` или `Av1`).
    #[must_use]
    pub fn new(codec: VideoCodec) -> Self {
        Self {
            codec,
            timestamp: 0,
            next_sequence: None,
            frame: Vec::new(),
            fragmenting: false,
            broken: false,
            started: false,
            lost: false,
            awaiting_keyframe: true,
            av1_frame_new_sequence: false,
            pending: Vec::new(),
            pending_bytes: 0,
            missing_sequences: Vec::new(),
        }
    }

    /// Создаёт сборщик H.264 (RFC 6184 -> Annex-B).
    #[must_use]
    pub fn new_h264() -> Self {
        Self::new(VideoCodec::H264)
    }

    /// Создаёт сборщик AV1 (OBU Aggregation Header).
    #[must_use]
    pub fn new_av1() -> Self {
        Self::new(VideoCodec::Av1)
    }

    /// Текущий видеокодек сборщика.
    #[must_use]
    pub const fn codec(&self) -> VideoCodec {
        self.codec
    }

    /// Переключает видеокодек и сбрасывает текущий собираемый кадр.
    pub fn set_codec(&mut self, codec: VideoCodec) {
        if self.codec != codec {
            self.codec = codec;
            self.reset_frame();
            self.started = false;
            self.pending.clear();
            self.pending_bytes = 0;
            self.awaiting_keyframe = true;
        }
    }

    /// Список пропущенных `sequence_number`, ожидающих запроса повторной передачи (RTCP NACK).
    #[must_use]
    pub fn missing_sequences(&self) -> &[u16] {
        &self.missing_sequences
    }

    /// Извлекает и очищает накопленный список пропущенных `sequence_number` для отправки RTCP NACK.
    pub fn take_missing_sequences(&mut self) -> Vec<u16> {
        std::mem::take(&mut self.missing_sequences)
    }

    /// Требуется ли отправка RTCP PLI (разрыв последовательности, битый кадр или ожидание первого keyframe).
    #[must_use]
    pub const fn needs_pli(&self) -> bool {
        self.lost || self.awaiting_keyframe
    }

    /// Находится ли сборщик в состоянии ожидания ключевого кадра (до первого IDR/Sequence Header или после потери).
    #[must_use]
    pub const fn is_awaiting_keyframe(&self) -> bool {
        self.awaiting_keyframe
    }

    /// Была ли зафиксирована потеря или повреждение кадра с момента последнего сброса/ключевого кадра.
    #[must_use]
    pub const fn has_loss(&self) -> bool {
        self.lost
    }

    /// Принудительно переводит сборщик в режим ожидания ключевого кадра и запроса PLI.
    pub fn require_keyframe(&mut self) {
        self.awaiting_keyframe = true;
        self.lost = true;
    }

    /// Снимает флаг `lost` и возвращает, требовался ли PLI (при этом `is_awaiting_keyframe` сохраняется до прихода keyframe).
    pub fn take_pli(&mut self) -> bool {
        let needed = self.needs_pli();
        self.lost = false;
        needed
    }

    /// Подаёт один разобранный [`RtpPacket`] в сборщик.
    pub fn push_packet(&mut self, packet: &RtpPacket<'_>) -> Option<Vec<u8>> {
        self.push(
            packet.header.sequence,
            packet.header.timestamp,
            packet.header.marker,
            packet.payload,
        )
    }

    /// Подаёт один RTP-пакет видеопотока; возвращает собранный кадр при закрытии целостного кадра битом `marker`.
    pub fn push(
        &mut self,
        sequence: u16,
        timestamp: u32,
        marker: bool,
        payload: &[u8],
    ) -> Option<Vec<u8>> {
        if let Some(expected) = self.next_sequence {
            let distance = sequence.wrapping_sub(expected);
            if distance >= 32768 {
                self.missing_sequences.retain(|&seq| seq != sequence);
                return None;
            }
            if distance != 0 {
                let record_limit = usize::from(distance).min(MAX_NACK_SEQUENCES);
                for offset in 0..record_limit {
                    let missing_seq = expected.wrapping_add(offset as u16);
                    if !self.pending.iter().any(|(seq, _, _)| *seq == missing_seq)
                        && !self.missing_sequences.contains(&missing_seq)
                        && self.missing_sequences.len() < MAX_NACK_SEQUENCES
                    {
                        self.missing_sequences.push(missing_seq);
                    }
                }
                if self.started && timestamp == self.timestamp {
                    if self.pending.iter().any(|(seq, _, _)| *seq == sequence) {
                        return None;
                    }
                    if usize::from(distance) < MAX_PENDING_VIDEO_PACKETS
                        && self.pending.len() < MAX_PENDING_VIDEO_PACKETS
                        && payload.len() <= MAX_PENDING_PACKET_BYTES
                        && self.pending_bytes + payload.len() <= MAX_PENDING_TOTAL_BYTES
                    {
                        self.pending_bytes += payload.len();
                        self.pending.push((sequence, marker, payload.to_vec()));
                        return None;
                    }
                    self.pending.clear();
                    self.pending_bytes = 0;
                }
            }
        }
        if timestamp != self.timestamp {
            self.pending.clear();
            self.pending_bytes = 0;
        }
        self.missing_sequences.retain(|&seq| seq != sequence);
        let mut complete = self.push_ordered(sequence, timestamp, marker, payload);
        while complete.is_none() {
            let Some(index) = self
                .pending
                .iter()
                .position(|(seq, _, _)| Some(*seq) == self.next_sequence)
            else {
                break;
            };
            let (seq, pkt_marker, pkt_payload) = self.pending.swap_remove(index);
            self.pending_bytes -= pkt_payload.len();
            self.missing_sequences.retain(|&missing| missing != seq);
            complete = self.push_ordered(seq, timestamp, pkt_marker, &pkt_payload);
        }
        complete
    }

    fn push_ordered(
        &mut self,
        sequence: u16,
        timestamp: u32,
        marker: bool,
        payload: &[u8],
    ) -> Option<Vec<u8>> {
        if self.started && timestamp != self.timestamp {
            // Новый кадр начался до прихода marker предыдущего кадра: сбрасываем неполный и запрашиваем PLI.
            self.lost = true;
            self.awaiting_keyframe = true;
            self.reset_frame();
            self.timestamp = timestamp;
        }
        if !self.started {
            self.started = true;
            self.timestamp = timestamp;
        }
        if self
            .next_sequence
            .is_some_and(|expected| expected != sequence)
        {
            self.broken = true;
            self.awaiting_keyframe = true;
        }
        self.next_sequence = Some(sequence.wrapping_add(1));
        if !self.broken && self.append(payload).is_err() {
            self.broken = true;
            self.awaiting_keyframe = true;
        }
        self.lost |= self.broken;
        if self.lost {
            self.awaiting_keyframe = true;
        }
        if !marker {
            return None;
        }
        let complete = (!self.broken && !self.fragmenting && !self.frame.is_empty())
            .then(|| std::mem::take(&mut self.frame));
        if let Some(ref frame_bytes) = complete {
            let is_kf = match self.codec {
                VideoCodec::H264 => h264_is_keyframe(frame_bytes),
                VideoCodec::Av1 => self.av1_frame_new_sequence || av1_is_keyframe(frame_bytes),
            };
            if is_kf {
                self.awaiting_keyframe = false;
                self.lost = false;
                self.missing_sequences.clear();
            }
        } else {
            self.lost = true;
            self.awaiting_keyframe = true;
        }
        self.reset_frame();
        self.started = false;
        complete
    }

    fn reset_frame(&mut self) {
        self.frame.clear();
        if self.frame.capacity() > MAX_PENDING_TOTAL_BYTES {
            self.frame = Vec::new();
        }
        self.fragmenting = false;
        self.broken = false;
        self.av1_frame_new_sequence = false;
    }

    fn extend(&mut self, parts: &[&[u8]]) -> Result<(), ()> {
        let total: usize = parts.iter().map(|part| part.len()).sum();
        if self.frame.len() + total > MAX_VIDEO_FRAME_BYTES {
            return Err(());
        }
        if self.frame.len() + total > self.frame.capacity()
            && self.frame.capacity().saturating_mul(2) > MAX_VIDEO_FRAME_BYTES
        {
            self.frame
                .reserve_exact(MAX_VIDEO_FRAME_BYTES - self.frame.len());
        }
        for part in parts {
            self.frame.extend_from_slice(part);
        }
        Ok(())
    }

    fn append(&mut self, payload: &[u8]) -> Result<(), ()> {
        match self.codec {
            VideoCodec::H264 => self.append_h264(payload),
            VideoCodec::Av1 => self.append_av1(payload),
        }
    }

    fn append_h264(&mut self, payload: &[u8]) -> Result<(), ()> {
        let Some(&indicator) = payload.first() else {
            return Err(());
        };
        match indicator & 0x1f {
            1..=23 => {
                if self.fragmenting {
                    return Err(());
                }
                self.extend(&[&ANNEX_B_START_CODE, payload])
            }
            24 => {
                if self.fragmenting || payload.len() < 3 {
                    return Err(());
                }
                let mut at = 1;
                while at < payload.len() {
                    if at + 2 > payload.len() {
                        return Err(());
                    }
                    let size = usize::from(u16::from_be_bytes([payload[at], payload[at + 1]]));
                    at += 2;
                    if size == 0 || at + size > payload.len() {
                        return Err(());
                    }
                    self.extend(&[&ANNEX_B_START_CODE, &payload[at..at + size]])?;
                    at += size;
                }
                Ok(())
            }
            28 => {
                if payload.len() < 2 {
                    return Err(());
                }
                let header = payload[1];
                let start = header & 0x80 != 0;
                let end = header & 0x40 != 0;
                let nal_type = header & 0x1f;
                if (start && end) || start == self.fragmenting || !matches!(nal_type, 1..=23) {
                    return Err(());
                }
                if start {
                    let nal_header = (indicator & 0xe0) | nal_type;
                    self.extend(&[&ANNEX_B_START_CODE, &[nal_header], &payload[2..]])?;
                } else {
                    self.extend(&[&payload[2..]])?;
                }
                self.fragmenting = !end;
                Ok(())
            }
            _ => Err(()),
        }
    }

    fn append_av1(&mut self, payload: &[u8]) -> Result<(), ()> {
        if payload.len() < 2 {
            return Err(());
        }
        let aggr = payload[0];
        let continues_prev = (aggr & 0x80) != 0; // Z-bit
        let continues_next = (aggr & 0x40) != 0; // Y-bit
        let w = (aggr >> 4) & 0x03; // W field (0..=3)
        let new_seq = (aggr & 0x08) != 0; // N-bit

        if (continues_prev && new_seq) || continues_prev != self.fragmenting {
            return Err(());
        }
        if new_seq {
            self.av1_frame_new_sequence = true;
        }

        let mut at = 1usize;
        match w {
            0 => {
                while at < payload.len() {
                    let (size, leb_len) = read_leb128(&payload[at..]).ok_or(())?;
                    at += leb_len;
                    if size == 0 || at + size > payload.len() {
                        return Err(());
                    }
                    self.extend(&[&payload[at..at + size]])?;
                    at += size;
                }
            }
            1 => {
                self.extend(&[&payload[at..]])?;
            }
            2 | 3 => {
                for _ in 0..(w - 1) {
                    let (size, leb_len) = read_leb128(&payload[at..]).ok_or(())?;
                    at += leb_len;
                    if size == 0 || at + size >= payload.len() {
                        return Err(());
                    }
                    self.extend(&[&payload[at..at + size]])?;
                    at += size;
                }
                if at >= payload.len() {
                    return Err(());
                }
                self.extend(&[&payload[at..]])?;
            }
            _ => unreachable!(),
        }
        self.fragmenting = continues_next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h264_single_stap_a_and_fu_a_rebuild_annex_b_with_start_codes() {
        let mut assembler = VideoFrameAssembler::new_h264();
        assert!(assembler.needs_pli(), "новый сборщик ожидает ключевой кадр");
        assert!(assembler.is_awaiting_keyframe());

        // STAP-A (24): SPS (0x67, 0xaa) + PPS (0x68), затем FU-A (28) IDR-срез (0x65) из 3 фрагментов.
        let stap = [24, 0, 2, 0x67, 0xaa, 0, 1, 0x68];
        assert!(assembler.push(10, 900, false, &stap).is_none());

        let fu_start = [0x7c, 0x85, 1, 2, 3]; // F=0, NRI=3 (0x60), FU-A=28; S=1, E=0, Type=5 -> NAL 0x65
        let fu_middle = [0x7c, 0x05, 4, 5]; // S=0, E=0, Type=5
        let fu_end = [0x7c, 0x45, 6]; // S=0, E=1, Type=5
        assert!(assembler.push(11, 900, false, &fu_start).is_none());
        assert!(assembler.push(12, 900, false, &fu_middle).is_none());
        let frame = assembler
            .push(13, 900, true, &fu_end)
            .expect("marker завершает целостный IDR-кадр");

        assert_eq!(
            frame,
            vec![
                0, 0, 0, 1, 0x67, 0xaa, // SPS с префиксом Annex-B
                0, 0, 0, 1, 0x68, // PPS с префиксом Annex-B
                0, 0, 0, 1, 0x65, 1, 2, 3, 4, 5, 6 // Собранный IDR NAL 0x65
            ]
        );
        assert!(h264_has_parameter_sets(&frame));
        assert!(h264_is_keyframe(&frame));
        assert!(
            !assembler.is_awaiting_keyframe(),
            "после IDR ключевой кадр получен"
        );
        assert!(!assembler.needs_pli());
        assert!(assembler.missing_sequences().is_empty());

        // Пропуск пакета seq=14: пакет seq=15 отбрасывается, seq=14 попадает в NACK, взводится PLI.
        assert!(assembler.push(15, 1800, true, &[0x41, 9]).is_none());
        assert_eq!(assembler.missing_sequences(), &[14]);
        assert!(assembler.needs_pli());
        assert!(assembler.is_awaiting_keyframe());
        assert!(assembler.has_loss());

        // Следующий целостный P-кадр (seq=16) собирается, но ожидание keyframe (PLI) сохраняется до IDR.
        assert_eq!(
            assembler.push(16, 2700, true, &[0x41, 9]).unwrap(),
            vec![0, 0, 0, 1, 0x41, 9]
        );
        assert!(assembler.is_awaiting_keyframe());
        assert!(assembler.needs_pli());

        // Приход нового IDR (seq=17, NAL 5 = 0x65) сбрасывает ожидание keyframe и PLI.
        assert_eq!(
            assembler.push(17, 3600, true, &[0x65, 0xaa, 0xbb]).unwrap(),
            vec![0, 0, 0, 1, 0x65, 0xaa, 0xbb]
        );
        assert!(!assembler.is_awaiting_keyframe());
        assert!(!assembler.needs_pli());
        assert!(!assembler.has_loss());
    }

    #[test]
    fn h264_reordering_and_rtx_recovery_completes_frame_without_pli() {
        let mut assembler = VideoFrameAssembler::new_h264();
        let video_ssrc = 0x1122_3344;
        let rtx_ssrc = 0x1122_3345;
        let video_pt = 105;
        let rtx_pt = 106;

        // Кадр 1 (IDR, seq=100) инициализирует ожидаемую последовательность.
        assert!(assembler.push(100, 9000, true, &[0x65, 1, 2]).is_some());
        assert!(!assembler.needs_pli());

        // Кадр 2 (timestamp=12000): приходит FU-A start (seq=101), seq=102 теряется, приходит FU-A end (seq=103, marker=true).
        let fu_start = [0x7c, 0x81, 0x10, 0x20];
        let fu_middle = [0x7c, 0x01, 0x30, 0x40];
        let fu_end = [0x7c, 0x41, 0x50, 0x60];

        assert!(assembler.push(101, 12000, false, &fu_start).is_none());
        // Пакет 103 пришёл раньше 102 -> буферизуется, а 102 фиксируется в missing_sequences для NACK!
        assert!(assembler.push(103, 12000, true, &fu_end).is_none());
        assert_eq!(assembler.missing_sequences(), &[102]);

        // Формируем RTX-пакет (RFC 4588): SSRC=rtx_ssrc, PT=rtx_pt, собственный rtx_seq=500,
        // первые 2 байта payload = OSN (102 BE) + оригинальный payload fu_middle.
        let rtx_header = RtpHeader {
            marker: false,
            payload_type: rtx_pt,
            sequence: 500,
            timestamp: 12000,
            ssrc: rtx_ssrc,
        };
        let mut rtx_wire = rtx_header.encode(None).unwrap();
        rtx_wire.extend_from_slice(&102u16.to_be_bytes());
        rtx_wire.extend_from_slice(&fu_middle);

        let parsed_rtx = parse_rtp(&rtx_wire).unwrap();
        let restored =
            restore_rtx_packet(&parsed_rtx, rtx_ssrc, rtx_pt, video_ssrc, video_pt).unwrap();
        assert_eq!(restored.header.ssrc, video_ssrc);
        assert_eq!(restored.header.payload_type, video_pt);
        assert_eq!(restored.header.sequence, 102);
        assert_eq!(restored.payload, &fu_middle);

        // Подача восстановленного пакета seq=102 автоматически дособирает отложенный seq=103 и закрывает кадр!
        let completed = assembler
            .push_packet(&restored)
            .expect("восстановленный через RTX пакет 102 закрывает кадр вместе с отложенным 103");
        assert_eq!(
            completed,
            vec![0, 0, 0, 1, 0x61, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60]
        );
        assert!(assembler.missing_sequences().is_empty());
        assert!(!assembler.needs_pli());
    }

    #[test]
    fn av1_obu_aggregation_single_multi_and_fragmented_reassembly() {
        let mut assembler = VideoFrameAssembler::new_av1();
        assert_eq!(assembler.codec(), VideoCodec::Av1);
        assert!(assembler.needs_pli());

        // 1. Агрегированный AV1 пакет (W=2, N=1: новый ключевой видеопоток):
        // OBU 1 (Sequence Header, 3 байта, с LEB128 длиной = 3) + OBU 2 (Frame Header, 4 байта, без длины).
        // Заголовок агрегации: Z=0, Y=0, W=2 (0b0010_0000 = 0x20), N=1 (0x08) -> 0x28.
        let obu_seq_hdr = [0x0a, 0x01, 0x00]; // obu_type = 1 (OBU_SEQUENCE_HEADER)
        let obu_frame = [0x32, 0x02, 0xaa, 0xbb]; // obu_type = 6 (OBU_FRAME)
        let mut pkt1 = vec![0x28, 0x03];
        pkt1.extend_from_slice(&obu_seq_hdr);
        pkt1.extend_from_slice(&obu_frame);

        let frame1 = assembler
            .push(1, 3000, true, &pkt1)
            .expect("AV1 W=2 пакет с marker=true собирается в кадр");
        let mut expected1 = Vec::new();
        expected1.extend_from_slice(&obu_seq_hdr);
        expected1.extend_from_slice(&obu_frame);
        assert_eq!(frame1, expected1);
        assert!(av1_is_keyframe(&frame1));
        assert!(!assembler.needs_pli());
        assert!(!assembler.is_awaiting_keyframe());

        // 2. Фрагментированный AV1 OBU из 3 пакетов:
        // Пакет A: Z=0, Y=1 (продолжение следует), W=1 -> 0x50
        // Пакет B: Z=1 (продолжает предыдущий), Y=1 (продолжение следует), W=1 -> 0xd0
        // Пакет C: Z=1 (продолжает предыдущий), Y=0 (конец OBU), W=1 -> 0x90, marker=true
        assert!(
            assembler
                .push(2, 6000, false, &[0x50, 0x32, 0x11])
                .is_none()
        );
        assert!(
            assembler
                .push(3, 6000, false, &[0xd0, 0x22, 0x33])
                .is_none()
        );
        let frame2 = assembler
            .push(4, 6000, true, &[0x90, 0x44, 0x55])
            .expect("фрагментированный AV1 OBU собирается по Z/Y флагам");
        assert_eq!(frame2, vec![0x32, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert!(!assembler.needs_pli());
    }

    #[test]
    fn invalid_nals_unclosed_fragments_and_wrong_rtx_are_rejected() {
        let mut assembler = VideoFrameAssembler::new_h264();
        // Неизвестный тип NAL (29 = FU-B) или пустой payload отклоняются и взводят PLI.
        assert!(assembler.push(1, 900, true, &[29, 1, 2]).is_none());
        assert!(assembler.needs_pli());
        assert!(assembler.push(2, 1800, true, &[]).is_none());

        // Незакрытый FU-A приmarker=true отклоняется.
        assert!(assembler.push(3, 2700, true, &[0x7c, 0x85, 1, 2]).is_none());
        assert!(assembler.has_loss());

        // Смена timestamp до прихода marker сбрасывает неполный кадр и запрашивает PLI.
        let _ = assembler.push(4, 3600, true, &[0x65, 1]);
        assert!(!assembler.needs_pli());
        assert!(assembler.push(5, 4500, false, &[0x41, 1]).is_none());
        assert!(assembler.push(6, 5400, true, &[0x41, 2]).is_some());
        assert!(
            assembler.needs_pli(),
            "смена timestamp без marker на предыдущем кадре обязана запросить PLI"
        );

        // Проверка негативных сценариев restore_rtx_packet:
        let header = RtpHeader {
            marker: true,
            payload_type: 106,
            sequence: 77,
            timestamp: 900,
            ssrc: 555,
        };
        // Слишком короткий payload (< 3 байт: нет медиа-байтов после 2-байтного OSN)
        let short_pkt = RtpPacket {
            header,
            extension: None,
            payload: &[0x00, 0x01],
        };
        assert!(restore_rtx_packet(&short_pkt, 555, 106, 554, 105).is_none());
        // Несовпадение SSRC или PT
        let valid_pkt = RtpPacket {
            header,
            extension: None,
            payload: &[0x00, 0x0a, 0x65, 0x01],
        };
        assert!(restore_rtx_packet(&valid_pkt, 999, 106, 554, 105).is_none());
        assert!(restore_rtx_packet(&valid_pkt, 555, 102, 554, 105).is_none());
    }

    #[test]
    fn restore_rtx_bytes_preserves_bede_extension_and_replaces_ssrc_pt_and_sequence() {
        let rtx_header = RtpHeader {
            marker: true,
            payload_type: 102,
            sequence: 999,
            timestamp: 180_000,
            ssrc: 0x2222,
        };
        let ext_data = [0x10, 0xff, 0x90, 0x04, 0, 0, 0, 0];
        let mut rtx_wire = rtx_header.encode(Some(&ext_data)).unwrap();
        // OSN = 0x1234, затем 3 байта медиа-нагрузки
        rtx_wire.extend_from_slice(&0x1234u16.to_be_bytes());
        rtx_wire.extend_from_slice(&[0x65, 0xaa, 0xbb]);

        let restored_wire = restore_rtx_bytes(&rtx_wire, 0x2222, 102, 0x1111, 101).unwrap();
        let parsed = parse_rtp(&restored_wire).unwrap();
        assert!(parsed.header.marker);
        assert_eq!(parsed.header.ssrc, 0x1111);
        assert_eq!(parsed.header.payload_type, 101);
        assert_eq!(parsed.header.sequence, 0x1234);
        assert_eq!(parsed.header.timestamp, 180_000);
        assert_eq!(parsed.extension.unwrap().data, &ext_data);
        assert_eq!(parsed.payload, &[0x65, 0xaa, 0xbb]);
    }

    #[test]
    fn av1_w0_multi_byte_leb128_and_w3_and_invalid_z_n_combinations() {
        let mut assembler = VideoFrameAssembler::new_av1();
        // W=0 (все элементы с LEB128 длиной), N=1 -> 0x08.
        // Элемент 1: длина 130 байт (LEB128: [0x82, 0x01]), первый байт OBU_SEQUENCE_HEADER (0x0a).
        let mut pkt = vec![0x08, 0x82, 0x01, 0x0a];
        pkt.resize(3 + 130, 0x77);
        // Элемент 2: длина 2 байта (LEB128: [0x02]), [0x32, 0x99].
        pkt.extend_from_slice(&[0x02, 0x32, 0x99]);

        let frame = assembler.push(65535, 9000, true, &pkt).unwrap();
        assert_eq!(frame.len(), 132);
        assert_eq!(frame[0], 0x0a);
        assert_eq!(&frame[130..132], &[0x32, 0x99]);
        assert!(!assembler.needs_pli());

        // Переход sequence через 65535 -> 0 с W=3 (2 элемента с длиной, 3-й до конца пакета):
        // Z=0, Y=0, W=3 (0x30), N=0 -> 0x30.
        let pkt_w3 = [0x30, 0x01, 0x11, 0x02, 0x22, 0x23, 0x33, 0x34, 0x35];
        let frame_w3 = assembler.push(0, 12000, true, &pkt_w3).unwrap();
        assert_eq!(frame_w3, vec![0x11, 0x22, 0x23, 0x33, 0x34, 0x35]);

        // Недопустимая комбинация Z=1 и N=1 (0x88) отклоняется и взводит PLI:
        assert!(
            assembler
                .push(1, 15000, true, &[0x88, 0x0a, 0x00])
                .is_none()
        );
        assert!(assembler.needs_pli());
    }
}
