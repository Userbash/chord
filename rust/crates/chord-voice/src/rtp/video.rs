//! Пакетизация видеопотоков H.264 (RFC 6184: Single NALU, STAP-A, FU-A) и AV1 (OBU Aggregation Header)
//! в RTP-пакеты (90 кГц, RFC 3550 + RFC 5285 `0xBEDE`, RTX RFC 4588) и опкоды видеостриминга `18..=22`.
//!
//! Оракулы:
//! - `/home/sanya/re-deep/voice-deep.md` §1, §5, §8 (кодеки `AV1 101/102`, `H265 103/104`,
//!   `H264 105/106`, `VP8 107/108`; частота 90 кГц; опкоды основного шлюза `18..=22`).
//! - `rust/crates/discord-voice/src/media/rtp.rs` (`VideoFrameAssembler` — приёмный депакетизатор).

use std::fmt;

use chord_json::JsonValue;

use crate::media::rtp::{RtpError, RtpHeader, VideoCodec};
use crate::srtp::SrtpError;
use crate::{json_u64, object};

/// Тактовая частота RTP-таймстемпа для видеопотоков (90 кГц по RFC 3551 / `voice-deep.md`).
pub const VIDEO_CLOCK_RATE: u32 = 90_000;

/// Шаг RTP-таймстемпа для 30 кадров/с (`90_000 / 30 = 3_000` тиков).
pub const VIDEO_TICKS_30FPS: u32 = 3_000;

/// Шаг RTP-таймстемпа для 60 кадров/с (`90_000 / 60 = 1_500` тиков).
pub const VIDEO_TICKS_60FPS: u32 = 1_500;

/// Целевая кадровая частота по умолчанию для демонстрации экрана Go Live (60 кадров/с).
pub const DEFAULT_VIDEO_FPS: u32 = 60;

/// Минимальный битрейт видеопотока Go Live / WebRTC (`150 kbps`).
pub const MIN_VIDEO_BITRATE_BPS: u32 = 150_000;

/// Целевой битрейт по умолчанию для профиля 720p60 (`2.5 Mbps`, `TASK-819.4`).
pub const DEFAULT_720P60_BITRATE_BPS: u32 = 2_500_000;

/// Максимальный битрейт видеопотока Go Live без перегрузки UDP-буфера (`3.5 Mbps`, `TASK-819.4`).
pub const MAX_GO_LIVE_VIDEO_BITRATE_BPS: u32 = 3_500_000;

/// Вычисляет шаг RTP-таймстемпа (при тактовой частоте 90 кГц) для заданного `fps`.
///
/// При `fps == 0` возвращает `VIDEO_CLOCK_RATE` (`90_000`, 1 секунда), исключая деление на ноль
/// и нулевой сдвиг таймстемпа, вызывающий `Discord Error 2007`.
#[must_use]
pub const fn video_ticks_for_fps(fps: u32) -> u32 {
    if fps == 0 {
        VIDEO_CLOCK_RATE
    } else {
        let clamped = if fps > 240 { 240 } else { fps };
        VIDEO_CLOCK_RATE / clamped
    }
}

/// Вычисляет непрерывное приращение RTP-таймстемпа (90 кГц) за реальный интервал `elapsed_ms` мс
/// (например, после паузы захвата или статичного экрана), гарантируя минимум один кадровый шаг `video_ticks_for_fps(fps)`.
#[must_use]
pub const fn video_ticks_for_elapsed_ms(elapsed_ms: u64, fps: u32) -> u32 {
    let min_step = video_ticks_for_fps(fps) as u64;
    let raw = elapsed_ms.saturating_mul((VIDEO_CLOCK_RATE / 1_000) as u64);
    let ticks = if raw < min_step { min_step } else { raw };
    if ticks > u32::MAX as u64 {
        u32::MAX
    } else {
        ticks as u32
    }
}

/// Ограничивает целевой битрейт видеопотока диапазоном `[MIN_VIDEO_BITRATE_BPS..=MAX_GO_LIVE_VIDEO_BITRATE_BPS]`.
/// При `bitrate_bps == 0` возвращает профиль по умолчанию `DEFAULT_720P60_BITRATE_BPS` (`2.5 Mbps`).
#[must_use]
pub const fn clamp_video_bitrate_bps(bitrate_bps: u32) -> u32 {
    if bitrate_bps == 0 {
        DEFAULT_720P60_BITRATE_BPS
    } else if bitrate_bps < MIN_VIDEO_BITRATE_BPS {
        MIN_VIDEO_BITRATE_BPS
    } else if bitrate_bps > MAX_GO_LIVE_VIDEO_BITRATE_BPS {
        MAX_GO_LIVE_VIDEO_BITRATE_BPS
    } else {
        bitrate_bps
    }
}

/// Порог полезной нагрузки RTP-видеопакета (1200 байт): кадры/NALU `<= 1200` байт идут как
/// Single NALU / STAP-A, а `> 1200` байт фрагментируются через FU-A (H.264) или Z/Y (AV1).
pub const MAX_VIDEO_RTP_PAYLOAD_LEN: usize = 1200;

/// Минимально допустимый лимит полезной нагрузки RTP для пакетизатора (байт).
pub const MIN_VIDEO_RTP_PAYLOAD_LEN: usize = 16;

/// RTP Payload Type для видеокодека AV1 (`default_client_codecs` в `voice-deep.md`).
pub const RTP_AV1_PAYLOAD_TYPE: u8 = 101;
/// RTP Payload Type для ретрансляции RTX видеокодека AV1.
pub const RTP_AV1_RTX_PAYLOAD_TYPE: u8 = 102;
/// RTP Payload Type для видеокодека H265.
pub const RTP_H265_PAYLOAD_TYPE: u8 = 103;
/// RTP Payload Type для ретрансляции RTX видеокодека H265.
pub const RTP_H265_RTX_PAYLOAD_TYPE: u8 = 104;
/// RTP Payload Type для видеокодека H264.
pub const RTP_H264_PAYLOAD_TYPE: u8 = 105;
/// RTP Payload Type для ретрансляции RTX видеокодека H264.
pub const RTP_H264_RTX_PAYLOAD_TYPE: u8 = 106;
/// RTP Payload Type для видеокодека VP8.
pub const RTP_VP8_PAYLOAD_TYPE: u8 = 107;
/// RTP Payload Type для ретрансляции RTX видеокодека VP8.
pub const RTP_VP8_RTX_PAYLOAD_TYPE: u8 = 108;

/// Опкод основного шлюза Discord: `STREAM_CREATE` (`op = 18`).
pub const GW_OP_STREAM_CREATE: u8 = 18;
/// Опкод основного шлюза Discord: `STREAM_DELETE` (`op = 19`).
pub const GW_OP_STREAM_DELETE: u8 = 19;
/// Опкод основного шлюза Discord: `STREAM_WATCH` (`op = 20`).
pub const GW_OP_STREAM_WATCH: u8 = 20;
/// Опкод основного шлюза Discord: `STREAM_PING` (`op = 21`).
pub const GW_OP_STREAM_PING: u8 = 21;
/// Опкод основного шлюза Discord: `STREAM_SET_PAUSED` (`op = 22`).
pub const GW_OP_STREAM_SET_PAUSED: u8 = 22;

/// Маска типа NAL-единицы H.264 (`nal_unit_type`, младшие 5 бит).
pub const H264_NAL_TYPE_MASK: u8 = 0x1F;
/// Маска важности ссылки H.264 (`nal_ref_idc`, биты 5..6).
pub const H264_NAL_NRI_MASK: u8 = 0x60;
/// Запрещённый старший бит заголовка NAL H.264 (`forbidden_zero_bit`, бит 7).
pub const H264_NAL_F_BIT: u8 = 0x80;

/// H.264 NAL: ключевой IDR-срез (`type = 5`).
pub const H264_NAL_IDR: u8 = 5;
/// H.264 NAL: Supplemental Enhancement Information (`type = 6`).
pub const H264_NAL_SEI: u8 = 6;
/// H.264 NAL: Sequence Parameter Set (`type = 7`).
pub const H264_NAL_SPS: u8 = 7;
/// H.264 NAL: Picture Parameter Set (`type = 8`).
pub const H264_NAL_PPS: u8 = 8;
/// H.264 NAL: Access Unit Delimiter (`type = 9`).
pub const H264_NAL_AUD: u8 = 9;
/// RFC 6184 STAP-A (Single-Time Aggregation Packet type A, `type = 24`).
pub const H264_NAL_STAP_A: u8 = 24;
/// RFC 6184 FU-A (Fragmentation Unit type A, `type = 28`).
pub const H264_NAL_FU_A: u8 = 28;

/// Бит `S` (Start) во втором байте заголовка FU-A (`fu_header`).
pub const H264_FU_HEADER_START: u8 = 0x80;
/// Бит `E` (End) во втором байте заголовка FU-A (`fu_header`).
pub const H264_FU_HEADER_END: u8 = 0x40;

/// Бит `Z` заголовка агрегации AV1 RTP: первый элемент продолжает фрагмент из предыдущего пакета.
pub const AV1_AGGR_Z_CONTINUATION: u8 = 0x80;
/// Бит `Y` заголовка агрегации AV1 RTP: последний элемент будет продолжен в следующем пакете.
pub const AV1_AGGR_Y_FRAGMENTED: u8 = 0x40;
/// Бит `N` заголовка агрегации AV1 RTP: пакет начинает новую кодированную видеопоследовательность (keyframe).
pub const AV1_AGGR_N_NEW_CODED_VIDEO_SEQ: u8 = 0x08;

/// AV1 OBU: `OBU_SEQUENCE_HEADER` (`type = 1`).
pub const AV1_OBU_SEQUENCE_HEADER: u8 = 1;
/// AV1 OBU: `OBU_TEMPORAL_DELIMITER` (`type = 2`).
pub const AV1_OBU_TEMPORAL_DELIMITER: u8 = 2;
/// AV1 OBU: `OBU_FRAME_HEADER` (`type = 3`).
pub const AV1_OBU_FRAME_HEADER: u8 = 3;
/// AV1 OBU: `OBU_TILE_GROUP` (`type = 4`).
pub const AV1_OBU_TILE_GROUP: u8 = 4;
/// AV1 OBU: `OBU_METADATA` (`type = 5`).
pub const AV1_OBU_METADATA: u8 = 5;
/// AV1 OBU: `OBU_FRAME` (`type = 6`).
pub const AV1_OBU_FRAME: u8 = 6;

/// Способ упаковки NAL-единиц H.264 в конкретном RTP-пакете по RFC 6184.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264PacketizationKind {
    /// Одиночная NAL-единица (`1..=23`), уместившаяся целиком в `<= 1200` байт.
    SingleNalu,
    /// Пакет агрегации нескольких малых NAL-единиц `STAP-A` (`type = 24`, например SPS + PPS).
    StapA,
    /// Фрагмент большой NAL-единицы `FU-A` (`type = 28`, для NALU `> 1200` байт).
    FuA {
        /// Первый фрагмент NAL-единицы (`S = 1`).
        start: bool,
        /// Последний фрагмент NAL-единицы (`E = 1`).
        end: bool,
    },
}

/// Ошибки пакетизации видеокадров H.264 / AV1 и шифрования DAVE MLS v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoRtpError {
    /// Входной видеокадр пуст или не содержит ни одной NAL-единицы / OBU.
    EmptyFrame,
    /// Обнаружена пустая NAL-единица.
    EmptyNalu,
    /// В заголовке NAL-единицы H.264 взведён запрещённый бит `F = 1` (`0x80`).
    ForbiddenZeroBitSet(u8),
    /// Недопустимый тип NAL-единицы во входном потоке Annex-B (ожидается `1..=23`).
    InvalidNalType(u8),
    /// Недопустимый лимит полезной нагрузки RTP (`max_payload_len`).
    InvalidMtu(usize),
    /// Пакет `STAP-A` пуст.
    StapAEmpty,
    /// Суммарный размер `STAP-A` превышает допустимый `max_payload_len`.
    StapATooLarge {
        /// Фактическая длина пакета `STAP-A`.
        total_len: usize,
        /// Максимально допустимая длина полезной нагрузки RTP.
        max_payload_len: usize,
    },
    /// Ошибка разбора или пакетизации OBU видеокодека AV1.
    InvalidAv1Obu(&'static str),
    /// Ошибка кодирования заголовка RTP / расширения `0xBEDE`.
    Rtp(RtpError),
    /// Ошибка двухслойного шифрования DAVE MLS v1 / SRTP.
    DaveSrtp(SrtpError),
}

impl fmt::Display for VideoRtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFrame => f.write_str("video frame is empty or contains no NAL/OBU units"),
            Self::EmptyNalu => f.write_str("H.264 NAL unit is empty"),
            Self::ForbiddenZeroBitSet(hdr) => {
                write!(f, "H.264 NAL header 0x{hdr:02x} has forbidden_zero_bit set")
            }
            Self::InvalidNalType(nal_type) => {
                write!(
                    f,
                    "invalid H.264 NAL unit type {nal_type} (expected 1..=23)"
                )
            }
            Self::InvalidMtu(mtu) => write!(f, "invalid RTP video payload MTU {mtu}"),
            Self::StapAEmpty => f.write_str("STAP-A aggregation requires at least one NAL unit"),
            Self::StapATooLarge {
                total_len,
                max_payload_len,
            } => write!(
                f,
                "STAP-A payload length {total_len} exceeds max RTP payload length {max_payload_len}"
            ),
            Self::InvalidAv1Obu(msg) => write!(f, "invalid AV1 OBU: {msg}"),
            Self::Rtp(err) => write!(f, "RTP header encoding error: {err}"),
            Self::DaveSrtp(err) => write!(f, "DAVE MLS / SRTP encryption error: {err}"),
        }
    }
}

impl std::error::Error for VideoRtpError {}

impl From<RtpError> for VideoRtpError {
    fn from(value: RtpError) -> Self {
        Self::Rtp(value)
    }
}

impl From<SrtpError> for VideoRtpError {
    fn from(value: SrtpError) -> Self {
        Self::DaveSrtp(value)
    }
}

/// Сформированный открытый RTP-видеопакет, готовый к отправке или двухслойному шифрованию DAVE MLS v1 + SRTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoRtpPacket {
    /// Заголовок RTP (RFC 3550).
    pub header: RtpHeader,
    /// Необязательное тело однобайтового расширения заголовка RFC 5285 (`0xBEDE`).
    pub extension: Option<Vec<u8>>,
    /// Полезная нагрузка RTP-видеопакета (H.264 Single NALU / STAP-A / FU-A или AV1 Aggregation Header + OBU).
    pub payload: Vec<u8>,
    /// Полностью сериализованный открытый RTP-пакет (`[12B RTP header][0xBEDE ext?][payload]`).
    pub raw: Vec<u8>,
}

impl VideoRtpPacket {
    /// Собирает открытый RTP-видеопакет из заголовка, опционального расширения `0xBEDE` и полезной нагрузки.
    pub fn new(
        header: RtpHeader,
        extension: Option<&[u8]>,
        payload: Vec<u8>,
    ) -> Result<Self, VideoRtpError> {
        let mut raw = header.encode(extension)?;
        raw.extend_from_slice(&payload);
        Ok(Self {
            header,
            extension: extension.map(<[u8]>::to_vec),
            payload,
            raw,
        })
    }

    /// Оборачивает данный RTP-видеопакет в пакет ретрансляции RTX по RFC 4588:
    /// подставляет `rtx_ssrc`, `rtx_payload_type` и `rtx_sequence`, а в начало `payload`
    /// записывает 2-байтный оригинальный номер последовательности (`OSN` в big-endian).
    pub fn encode_rtx(
        &self,
        rtx_sequence: u16,
        rtx_ssrc: u32,
        rtx_payload_type: u8,
    ) -> Result<Self, VideoRtpError> {
        let rtx_header = RtpHeader {
            marker: self.header.marker,
            payload_type: rtx_payload_type & 0x7F,
            sequence: rtx_sequence,
            timestamp: self.header.timestamp,
            ssrc: rtx_ssrc,
        };
        let mut rtx_payload = Vec::with_capacity(2 + self.payload.len());
        rtx_payload.extend_from_slice(&self.header.sequence.to_be_bytes());
        rtx_payload.extend_from_slice(&self.payload);
        Self::new(rtx_header, self.extension.as_deref(), rtx_payload)
    }
}

fn validate_h264_nalu(nalu: &[u8]) -> Result<u8, VideoRtpError> {
    let Some(&hdr) = nalu.first() else {
        return Err(VideoRtpError::EmptyNalu);
    };
    if (hdr & H264_NAL_F_BIT) != 0 {
        return Err(VideoRtpError::ForbiddenZeroBitSet(hdr));
    }
    let nal_type = hdr & H264_NAL_TYPE_MASK;
    if !(1..=23).contains(&nal_type) {
        return Err(VideoRtpError::InvalidNalType(nal_type));
    }
    Ok(nal_type)
}

fn find_annex_b_start_code(data: &[u8], from: usize) -> Option<(usize, usize)> {
    let mut i = from;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 {
            if data[i + 2] == 1 {
                return Some((i, 3));
            }
            if i + 4 <= data.len() && data[i + 2] == 0 && data[i + 3] == 1 {
                return Some((i, 4));
            }
        }
        i += 1;
    }
    None
}

/// Разбирает поток H.264 Annex-B (с 3-байтными `00 00 01` или 4-байтными `00 00 00 01` стартовыми кодами)
/// на список валидированных NAL-единиц (`nal_type` в диапазоне `1..=23`).
pub fn parse_h264_annex_b(bitstream: &[u8]) -> Result<Vec<&[u8]>, VideoRtpError> {
    if bitstream.is_empty() {
        return Err(VideoRtpError::EmptyFrame);
    }

    let mut nalus = Vec::new();
    if let Some((first_pos, first_len)) = find_annex_b_start_code(bitstream, 0) {
        if first_pos > 0 {
            let prefix = &bitstream[..first_pos];
            if prefix.iter().any(|&b| b != 0) {
                validate_h264_nalu(prefix)?;
                nalus.push(prefix);
            }
        }
        let mut nal_start = first_pos + first_len;
        while nal_start <= bitstream.len() {
            if let Some((next_pos, next_len)) = find_annex_b_start_code(bitstream, nal_start) {
                let nalu = &bitstream[nal_start..next_pos];
                if !nalu.is_empty() {
                    validate_h264_nalu(nalu)?;
                    nalus.push(nalu);
                }
                nal_start = next_pos + next_len;
            } else {
                let nalu = &bitstream[nal_start..];
                if !nalu.is_empty() {
                    validate_h264_nalu(nalu)?;
                    nalus.push(nalu);
                }
                break;
            }
        }
    } else {
        validate_h264_nalu(bitstream)?;
        nalus.push(bitstream);
    }

    if nalus.is_empty() {
        return Err(VideoRtpError::EmptyFrame);
    }
    Ok(nalus)
}

/// Упаковывает срез малых NAL-единиц H.264 в один пакет агрегации `STAP-A` (`type = 24`) по RFC 6184:
/// `[max_nri | 24][u16 len_be][nal_1][u16 len_be][nal_2]...`.
pub fn encode_h264_stap_a(
    nalus: &[&[u8]],
    max_payload_len: usize,
) -> Result<Vec<u8>, VideoRtpError> {
    if nalus.is_empty() {
        return Err(VideoRtpError::StapAEmpty);
    }
    if max_payload_len < 4 {
        return Err(VideoRtpError::InvalidMtu(max_payload_len));
    }

    let mut max_nri = 0u8;
    let mut total_len = 1usize;
    for &nalu in nalus {
        validate_h264_nalu(nalu)?;
        if nalu.len() > usize::from(u16::MAX) {
            return Err(VideoRtpError::StapATooLarge {
                total_len: total_len.saturating_add(2).saturating_add(nalu.len()),
                max_payload_len,
            });
        }
        max_nri = max_nri.max(nalu[0] & H264_NAL_NRI_MASK);
        total_len = total_len.saturating_add(2).saturating_add(nalu.len());
    }

    if total_len > max_payload_len {
        return Err(VideoRtpError::StapATooLarge {
            total_len,
            max_payload_len,
        });
    }

    let mut out = Vec::with_capacity(total_len);
    out.push(max_nri | H264_NAL_STAP_A);
    for &nalu in nalus {
        let len_u16 = u16::try_from(nalu.len()).expect("checked <= u16::MAX");
        out.extend_from_slice(&len_u16.to_be_bytes());
        out.extend_from_slice(nalu);
    }
    Ok(out)
}

/// Фрагментирует большую NAL-единицу H.264 (`> max_payload_len`) на серию пакетов `FU-A` (`type = 28`)
/// по RFC 6184: `[fu_indicator = (F|NRI)|28][fu_header = (S<<7)|(E<<6)|nal_type][fragment_body]`.
pub fn encode_h264_fu_a(
    nalu: &[u8],
    max_payload_len: usize,
) -> Result<Vec<Vec<u8>>, VideoRtpError> {
    let nal_type = validate_h264_nalu(nalu)?;
    if max_payload_len < 4 {
        return Err(VideoRtpError::InvalidMtu(max_payload_len));
    }
    let body = &nalu[1..];
    if body.is_empty() {
        return Err(VideoRtpError::EmptyNalu);
    }

    let max_chunk = max_payload_len - 2;
    if body.len() <= max_chunk {
        // Для вызова encode_h264_fu_a напрямую на NALU >= 2 байт делим пополам, чтобы S и E не стояли одновременно
        if body.len() < 2 {
            return Err(VideoRtpError::EmptyNalu);
        }
        let mid = body.len() / 2;
        let fu_indicator = (nalu[0] & (H264_NAL_F_BIT | H264_NAL_NRI_MASK)) | H264_NAL_FU_A;
        let mut first = Vec::with_capacity(2 + mid);
        first.push(fu_indicator);
        first.push(H264_FU_HEADER_START | nal_type);
        first.extend_from_slice(&body[..mid]);

        let mut second = Vec::with_capacity(2 + (body.len() - mid));
        second.push(fu_indicator);
        second.push(H264_FU_HEADER_END | nal_type);
        second.extend_from_slice(&body[mid..]);
        return Ok(vec![first, second]);
    }

    let fu_indicator = (nalu[0] & (H264_NAL_F_BIT | H264_NAL_NRI_MASK)) | H264_NAL_FU_A;
    let mut fragments = Vec::with_capacity(body.len().div_ceil(max_chunk));
    let mut offset = 0usize;
    while offset < body.len() {
        let take = (body.len() - offset).min(max_chunk);
        let is_start = offset == 0;
        let is_end = offset + take == body.len();
        let mut fu_header = nal_type;
        if is_start {
            fu_header |= H264_FU_HEADER_START;
        }
        if is_end {
            fu_header |= H264_FU_HEADER_END;
        }
        let mut pkt = Vec::with_capacity(2 + take);
        pkt.push(fu_indicator);
        pkt.push(fu_header);
        pkt.extend_from_slice(&body[offset..offset + take]);
        fragments.push(pkt);
        offset += take;
    }
    Ok(fragments)
}

fn flush_h264_pending(
    pending: &mut Vec<&[u8]>,
    max_payload_len: usize,
    out: &mut Vec<(Vec<u8>, bool, H264PacketizationKind)>,
) -> Result<(), VideoRtpError> {
    if pending.is_empty() {
        return Ok(());
    }
    if pending.len() == 1 {
        let single = pending.pop().expect("len == 1");
        out.push((single.to_vec(), false, H264PacketizationKind::SingleNalu));
    } else {
        let stap = encode_h264_stap_a(pending, max_payload_len)?;
        pending.clear();
        out.push((stap, false, H264PacketizationKind::StapA));
    }
    Ok(())
}

/// Пакетизирует кадр H.264 Annex-B с заданным порогом `max_payload_len` (по умолчанию 1200 байт):
/// - несколько подряд идущих малых NAL-единиц (например SPS + PPS), умещающихся в `max_payload_len`,
///   объединяются в один пакет `STAP-A` (`type = 24`);
/// - одиночная NAL-единица `<= max_payload_len` упаковывается как `SingleNalu` (`1..=23`);
/// - NAL-единица `> max_payload_len` фрагментируется на пакеты `FU-A` (`type = 28`).
///
/// Бит `marker = true` выставляется строго на последнем RTP-пакете кадра.
pub fn packetize_h264_annex_b_with_mtu(
    bitstream: &[u8],
    max_payload_len: usize,
) -> Result<Vec<(Vec<u8>, bool, H264PacketizationKind)>, VideoRtpError> {
    if max_payload_len < MIN_VIDEO_RTP_PAYLOAD_LEN {
        return Err(VideoRtpError::InvalidMtu(max_payload_len));
    }
    let nalus = parse_h264_annex_b(bitstream)?;
    let mut out: Vec<(Vec<u8>, bool, H264PacketizationKind)> = Vec::new();
    let mut pending_small: Vec<&[u8]> = Vec::new();
    let mut pending_stap_len: usize = 1; // 1 байт заголовка STAP-A

    for nalu in nalus {
        if nalu.len() > max_payload_len {
            flush_h264_pending(&mut pending_small, max_payload_len, &mut out)?;
            pending_stap_len = 1;

            let fu_fragments = encode_h264_fu_a(nalu, max_payload_len)?;
            let total_frags = fu_fragments.len();
            for (idx, frag) in fu_fragments.into_iter().enumerate() {
                out.push((
                    frag,
                    false,
                    H264PacketizationKind::FuA {
                        start: idx == 0,
                        end: idx + 1 == total_frags,
                    },
                ));
            }
            continue;
        }

        let entry_len = 2 + nalu.len();
        if pending_small.is_empty() {
            if entry_len < max_payload_len {
                pending_small.push(nalu);
                pending_stap_len = 1 + entry_len;
            } else {
                // NALU помещается как Single NALU (<= max_payload_len), но не помещается в STAP-A (+3 байта)
                out.push((nalu.to_vec(), false, H264PacketizationKind::SingleNalu));
            }
        } else if pending_stap_len + entry_len <= max_payload_len {
            pending_small.push(nalu);
            pending_stap_len += entry_len;
        } else {
            flush_h264_pending(&mut pending_small, max_payload_len, &mut out)?;
            if entry_len < max_payload_len {
                pending_small.push(nalu);
                pending_stap_len = 1 + entry_len;
            } else {
                pending_stap_len = 1;
                out.push((nalu.to_vec(), false, H264PacketizationKind::SingleNalu));
            }
        }
    }

    flush_h264_pending(&mut pending_small, max_payload_len, &mut out)?;
    if let Some(last) = out.last_mut() {
        last.1 = true;
    }
    Ok(out)
}

fn write_leb128(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn leb128_len(mut value: usize) -> usize {
    let mut len = 1;
    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }
    len
}

fn read_leb128(data: &[u8]) -> Option<(usize, usize)> {
    let mut value = 0usize;
    for (index, &byte) in data.iter().take(8).enumerate() {
        let chunk = usize::from(byte & 0x7F);
        let shift = index * 7;
        value |= chunk.checked_shl(shift as u32)?;
        if (byte & 0x80) == 0 {
            return Some((value, index + 1));
        }
    }
    None
}

/// Разбирает саморазграничивающийся поток AV1 OBU (где `obu_has_size_field = 1`, либо одиночный OBU)
/// на список срезов отдельных OBU-элементов.
pub fn parse_av1_obu_stream(bitstream: &[u8]) -> Result<Vec<&[u8]>, VideoRtpError> {
    if bitstream.is_empty() {
        return Err(VideoRtpError::EmptyFrame);
    }
    let mut obus = Vec::new();
    let mut at = 0usize;
    while at < bitstream.len() {
        let hdr = bitstream[at];
        if (hdr & 0x80) != 0 {
            return Err(VideoRtpError::InvalidAv1Obu("obu_forbidden_bit must be 0"));
        }
        let has_extension = (hdr & 0x04) != 0;
        let has_size_field = (hdr & 0x02) != 0;
        let header_len = if has_extension { 2 } else { 1 };
        if at + header_len > bitstream.len() {
            return Err(VideoRtpError::InvalidAv1Obu(
                "truncated OBU extension header",
            ));
        }
        if has_size_field {
            let (payload_size, leb_bytes) = read_leb128(&bitstream[at + header_len..])
                .ok_or(VideoRtpError::InvalidAv1Obu("invalid OBU LEB128 size"))?;
            let obu_end = at
                .checked_add(header_len)
                .and_then(|v| v.checked_add(leb_bytes))
                .and_then(|v| v.checked_add(payload_size))
                .ok_or(VideoRtpError::InvalidAv1Obu("OBU size overflow"))?;
            if obu_end > bitstream.len() {
                return Err(VideoRtpError::InvalidAv1Obu(
                    "truncated OBU payload beyond bitstream",
                ));
            }
            obus.push(&bitstream[at..obu_end]);
            at = obu_end;
        } else {
            obus.push(&bitstream[at..]);
            break;
        }
    }
    if obus.is_empty() {
        return Err(VideoRtpError::EmptyFrame);
    }
    Ok(obus)
}

fn av1_batch_encoded_len(batch: &[&[u8]]) -> usize {
    if batch.is_empty() {
        return 0;
    }
    if batch.len() <= 3 {
        let mut total = 1usize; // 1B aggregation header
        for (idx, obu) in batch.iter().enumerate() {
            if idx + 1 < batch.len() {
                total += leb128_len(obu.len());
            }
            total += obu.len();
        }
        total
    } else {
        let mut total = 1usize;
        for obu in batch {
            total += leb128_len(obu.len()) + obu.len();
        }
        total
    }
}

fn flush_av1_batch(
    batch: &mut Vec<&[u8]>,
    n_bit_pending: &mut bool,
    out: &mut Vec<(Vec<u8>, bool)>,
) {
    if batch.is_empty() {
        return;
    }
    let w_field: u8 = if batch.len() <= 3 {
        batch.len() as u8
    } else {
        0
    };
    let n_flag = if *n_bit_pending {
        *n_bit_pending = false;
        AV1_AGGR_N_NEW_CODED_VIDEO_SEQ
    } else {
        0
    };
    let aggr_header = (w_field << 4) | n_flag;
    let mut payload = Vec::with_capacity(av1_batch_encoded_len(batch));
    payload.push(aggr_header);

    if w_field == 0 {
        for obu in batch.iter() {
            write_leb128(obu.len(), &mut payload);
            payload.extend_from_slice(obu);
        }
    } else {
        let count = batch.len();
        for (idx, obu) in batch.iter().enumerate() {
            if idx + 1 < count {
                write_leb128(obu.len(), &mut payload);
            }
            payload.extend_from_slice(obu);
        }
    }
    batch.clear();
    out.push((payload, false));
}

/// Пакетизирует срез OBU-элементов AV1 в RTP-нагрузки по спецификации RTP Payload Format for AV1:
/// - выставляет `N = 1` (`0x08`) на первом пакете ключевого кадра (`OBU_SEQUENCE_HEADER = 1`);
/// - агрегирует несколько малых OBU в один RTP-пакет (`W = 1..=3` или `W = 0` с LEB128 длинами для 4+ OBU);
/// - фрагментирует крупные OBU (`> max_payload_len - 1`) на несколько RTP-пакетов с флагами `Z` (`0x80`) и `Y` (`0x40`);
/// - взводит `marker = true` строго на последнем RTP-пакете кадра.
pub fn packetize_av1_obus_with_mtu(
    obus: &[&[u8]],
    max_payload_len: usize,
) -> Result<Vec<(Vec<u8>, bool)>, VideoRtpError> {
    if obus.is_empty() {
        return Err(VideoRtpError::EmptyFrame);
    }
    if max_payload_len < MIN_VIDEO_RTP_PAYLOAD_LEN {
        return Err(VideoRtpError::InvalidMtu(max_payload_len));
    }

    for &obu in obus {
        let Some(&hdr) = obu.first() else {
            return Err(VideoRtpError::InvalidAv1Obu("empty OBU element"));
        };
        if (hdr & 0x80) != 0 {
            return Err(VideoRtpError::InvalidAv1Obu("obu_forbidden_bit must be 0"));
        }
    }

    let first_obu_type = (obus[0][0] >> 3) & 0x0F;
    let mut n_bit_pending = first_obu_type == AV1_OBU_SEQUENCE_HEADER;
    let max_single_obu_bytes = max_payload_len - 1;

    let mut out: Vec<(Vec<u8>, bool)> = Vec::new();
    let mut batch: Vec<&[u8]> = Vec::new();

    for &obu in obus {
        if obu.len() > max_single_obu_bytes {
            flush_av1_batch(&mut batch, &mut n_bit_pending, &mut out);
            let mut offset = 0usize;
            while offset < obu.len() {
                let take = (obu.len() - offset).min(max_single_obu_bytes);
                let is_first_frag = offset == 0;
                let is_last_frag = offset + take == obu.len();
                let z_flag = if is_first_frag {
                    0
                } else {
                    AV1_AGGR_Z_CONTINUATION
                };
                let y_flag = if is_last_frag {
                    0
                } else {
                    AV1_AGGR_Y_FRAGMENTED
                };
                let n_flag = if is_first_frag && n_bit_pending {
                    n_bit_pending = false;
                    AV1_AGGR_N_NEW_CODED_VIDEO_SEQ
                } else {
                    0
                };
                let aggr_header = z_flag | y_flag | (1 << 4) | n_flag;
                let mut pkt = Vec::with_capacity(1 + take);
                pkt.push(aggr_header);
                pkt.extend_from_slice(&obu[offset..offset + take]);
                out.push((pkt, false));
                offset += take;
            }
            continue;
        }

        batch.push(obu);
        if av1_batch_encoded_len(&batch) > max_payload_len {
            let popped = batch.pop().expect("just pushed");
            flush_av1_batch(&mut batch, &mut n_bit_pending, &mut out);
            batch.push(popped);
        }
    }

    flush_av1_batch(&mut batch, &mut n_bit_pending, &mut out);
    if let Some(last) = out.last_mut() {
        last.1 = true;
    }
    Ok(out)
}

/// Разбирает саморазграничивающийся поток AV1 OBU и пакетизирует его с заданным `max_payload_len`.
pub fn packetize_av1_bitstream_with_mtu(
    bitstream: &[u8],
    max_payload_len: usize,
) -> Result<Vec<(Vec<u8>, bool)>, VideoRtpError> {
    let obus = parse_av1_obu_stream(bitstream)?;
    packetize_av1_obus_with_mtu(&obus, max_payload_len)
}

/// Состоятельный RTP-пакетизатор видеопотоков H.264 и AV1 (90 кГц, RFC 3550 / 6184 / 4588).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpVideoPacketizer {
    codec: VideoCodec,
    ssrc: u32,
    rtx_ssrc: u32,
    payload_type: u8,
    rtx_payload_type: u8,
    sequence: u16,
    rtx_sequence: u16,
    timestamp: u32,
    max_payload_len: usize,
    target_fps: u32,
    cached_sps: Option<Vec<u8>>,
    cached_pps: Option<Vec<u8>>,
    cached_idr: Option<Vec<u8>>,
    cached_av1_seq_hdr: Option<Vec<u8>>,
    cached_av1_frame: Option<Vec<u8>>,
    keyframe_ctrl: crate::rtcp::RtcpKeyframeController,
}

impl RtpVideoPacketizer {
    /// Создаёт RTP-пакетизатор для H.264 (`PT = 105`, `RTX PT = 106`, `MTU = 1200`).
    #[must_use]
    pub const fn new_h264(ssrc: u32) -> Self {
        Self {
            codec: VideoCodec::H264,
            ssrc,
            rtx_ssrc: ssrc.wrapping_add(1),
            payload_type: RTP_H264_PAYLOAD_TYPE,
            rtx_payload_type: RTP_H264_RTX_PAYLOAD_TYPE,
            sequence: 0,
            rtx_sequence: 0,
            timestamp: 0,
            max_payload_len: MAX_VIDEO_RTP_PAYLOAD_LEN,
            target_fps: DEFAULT_VIDEO_FPS,
            cached_sps: None,
            cached_pps: None,
            cached_idr: None,
            cached_av1_seq_hdr: None,
            cached_av1_frame: None,
            keyframe_ctrl: crate::rtcp::RtcpKeyframeController::new(ssrc, ssrc),
        }
    }

    /// Создаёт RTP-пакетизатор для AV1 (`PT = 101`, `RTX PT = 102`, `MTU = 1200`).
    #[must_use]
    pub const fn new_av1(ssrc: u32) -> Self {
        Self {
            codec: VideoCodec::Av1,
            ssrc,
            rtx_ssrc: ssrc.wrapping_add(1),
            payload_type: RTP_AV1_PAYLOAD_TYPE,
            rtx_payload_type: RTP_AV1_RTX_PAYLOAD_TYPE,
            sequence: 0,
            rtx_sequence: 0,
            timestamp: 0,
            max_payload_len: MAX_VIDEO_RTP_PAYLOAD_LEN,
            target_fps: DEFAULT_VIDEO_FPS,
            cached_sps: None,
            cached_pps: None,
            cached_idr: None,
            cached_av1_seq_hdr: None,
            cached_av1_frame: None,
            keyframe_ctrl: crate::rtcp::RtcpKeyframeController::new(ssrc, ssrc),
        }
    }

    /// Настраивает `rtx_ssrc` и `rtx_payload_type` для пакетов ретрансляции RFC 4588.
    #[must_use]
    pub const fn with_rtx(mut self, rtx_ssrc: u32, rtx_payload_type: u8) -> Self {
        self.rtx_ssrc = rtx_ssrc;
        self.rtx_payload_type = rtx_payload_type;
        self
    }

    /// Задаёт начальные значения `sequence` и `timestamp`.
    #[must_use]
    pub const fn with_initial_state(mut self, sequence: u16, timestamp: u32) -> Self {
        self.sequence = sequence;
        self.timestamp = timestamp;
        self
    }

    /// Задаёт порог полезной нагрузки RTP-пакета (`max_payload_len >= 16`).
    pub fn with_max_payload_len(mut self, max_payload_len: usize) -> Result<Self, VideoRtpError> {
        if max_payload_len < MIN_VIDEO_RTP_PAYLOAD_LEN {
            return Err(VideoRtpError::InvalidMtu(max_payload_len));
        }
        self.max_payload_len = max_payload_len;
        Ok(self)
    }

    /// Целевая кадровая частота пакетизатора (`fps`, по умолчанию 60).
    #[must_use]
    pub const fn target_fps(&self) -> u32 {
        self.target_fps
    }

    /// Устанавливает целевую кадровую частоту (`fps`) для вычисления шага таймстемпа по умолчанию.
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = if fps == 0 {
            DEFAULT_VIDEO_FPS
        } else {
            fps.clamp(1, 240)
        };
    }

    /// Переключает кодек и `payload_type` / `rtx_payload_type` на лету без сброса
    /// стабилизированных `ssrc`, `rtx_ssrc`, `sequence`, `rtx_sequence` и `timestamp` (`TASK-819.1`).
    pub fn reconfigure_codec(&mut self, payload_type: u8, rtx_payload_type: u8) {
        self.payload_type = payload_type & 0x7F;
        self.rtx_payload_type = rtx_payload_type & 0x7F;
        if self.payload_type == RTP_AV1_PAYLOAD_TYPE {
            self.codec = VideoCodec::Av1;
        } else if self.payload_type == RTP_H264_PAYLOAD_TYPE {
            self.codec = VideoCodec::H264;
        }
    }

    /// Фиксирует пару `(ssrc, rtx_ssrc)`, не сбрасывая `sequence` и `timestamp`.
    pub fn stabilize_ssrcs(&mut self, ssrc: u32, rtx_ssrc: u32) {
        self.ssrc = ssrc;
        self.rtx_ssrc = rtx_ssrc;
        self.keyframe_ctrl.set_local_ssrcs(ssrc, ssrc);
    }

    /// Непрерывно продвигает `timestamp` (90 кГц) на время паузы захвата `elapsed_ms` (`TASK-819.2`).
    pub fn advance_for_pause_ms(&mut self, elapsed_ms: u64) -> u32 {
        let delta = video_ticks_for_elapsed_ms(elapsed_ms, self.target_fps);
        self.timestamp = self.timestamp.wrapping_add(delta);
        self.timestamp
    }

    /// Видеокодек пакетизатора (`H264` или `Av1`).
    #[must_use]
    pub const fn codec(&self) -> VideoCodec {
        self.codec
    }

    /// Основной видео `SSRC`.
    #[must_use]
    pub const fn ssrc(&self) -> u32 {
        self.ssrc
    }

    /// `SSRC` потока ретрансляции RTX.
    #[must_use]
    pub const fn rtx_ssrc(&self) -> u32 {
        self.rtx_ssrc
    }

    /// RTP Payload Type основного видеопотока.
    #[must_use]
    pub const fn payload_type(&self) -> u8 {
        self.payload_type
    }

    /// RTP Payload Type потока ретрансляции RTX.
    #[must_use]
    pub const fn rtx_payload_type(&self) -> u8 {
        self.rtx_payload_type
    }

    /// Следующий номер последовательности `sequence` основного видеопотока.
    #[must_use]
    pub const fn sequence(&self) -> u16 {
        self.sequence
    }

    /// Алиас `sequence()` — следующий номер последовательности основного видеопотока.
    #[must_use]
    pub const fn next_sequence(&self) -> u16 {
        self.sequence
    }

    /// Следующий номер последовательности `rtx_sequence` потока ретрансляции RTX.
    #[must_use]
    pub const fn rtx_sequence(&self) -> u16 {
        self.rtx_sequence
    }

    /// Текущий RTP `timestamp` видеопотока (90 кГц).
    #[must_use]
    pub const fn timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Алиас `timestamp()` — следующий RTP `timestamp` видеопотока (90 кГц).
    #[must_use]
    pub const fn next_timestamp(&self) -> u32 {
        self.timestamp
    }

    /// Порог полезной нагрузки RTP-пакета (по умолчанию 1200 байт).
    #[must_use]
    pub const fn max_payload_len(&self) -> usize {
        self.max_payload_len
    }

    /// Пакетизирует кадр H.264 Annex-B в серию RTP-пакетов (Single NALU, STAP-A, FU-A)
    /// и сдвигает `timestamp` на `timestamp_delta` тиков (например `3000` для 30 fps).
    pub fn packetize_h264_frame(
        &mut self,
        annex_b_frame: &[u8],
        timestamp_delta: u32,
        ext: Option<&[u8]>,
    ) -> Result<Vec<VideoRtpPacket>, VideoRtpError> {
        let nalus = parse_h264_annex_b(annex_b_frame)?;
        let mut has_sps = false;
        let mut has_pps = false;
        let mut has_idr = false;
        for &nalu in &nalus {
            if let Some(&hdr) = nalu.first() {
                match hdr & 0x1F {
                    H264_NAL_SPS => {
                        has_sps = true;
                        self.cached_sps = Some(nalu.to_vec());
                    }
                    H264_NAL_PPS => {
                        has_pps = true;
                        self.cached_pps = Some(nalu.to_vec());
                    }
                    H264_NAL_IDR => {
                        has_idr = true;
                        self.cached_idr = Some(nalu.to_vec());
                    }
                    _ => {}
                }
            }
        }

        let upgraded_frame: Option<Vec<u8>> =
            if has_idr && (!has_sps || !has_pps) && self.keyframe_ctrl.is_keyframe_requested() {
                if let (Some(sps), Some(pps)) = (&self.cached_sps, &self.cached_pps) {
                    let mut combined =
                        Vec::with_capacity(12 + sps.len() + pps.len() + annex_b_frame.len());
                    if !has_sps {
                        combined.extend_from_slice(&[0, 0, 0, 1]);
                        combined.extend_from_slice(sps);
                    }
                    if !has_pps {
                        combined.extend_from_slice(&[0, 0, 0, 1]);
                        combined.extend_from_slice(pps);
                    }
                    combined.extend_from_slice(annex_b_frame);
                    Some(combined)
                } else {
                    None
                }
            } else {
                None
            };

        let effective_frame = upgraded_frame.as_deref().unwrap_or(annex_b_frame);
        let chunks = packetize_h264_annex_b_with_mtu(effective_frame, self.max_payload_len)?;
        if has_idr && (has_sps && has_pps || upgraded_frame.is_some()) {
            let _ = self.keyframe_ctrl.take_keyframe_request();
        }
        let frame_ts = self.timestamp;
        let step = if timestamp_delta == 0 {
            video_ticks_for_fps(self.target_fps)
        } else {
            timestamp_delta
        };
        let mut packets = Vec::with_capacity(chunks.len());
        for (payload, marker, _kind) in chunks {
            let header = RtpHeader {
                marker,
                payload_type: self.payload_type,
                sequence: self.sequence,
                timestamp: frame_ts,
                ssrc: self.ssrc,
            };
            self.sequence = self.sequence.wrapping_add(1);
            packets.push(VideoRtpPacket::new(header, ext, payload)?);
        }
        self.timestamp = self.timestamp.wrapping_add(step);
        Ok(packets)
    }

    /// Алиас `packetize_h264_frame` для пакетизации потока H.264 Annex-B.
    pub fn packetize_h264_annex_b(
        &mut self,
        annex_b_frame: &[u8],
        timestamp_delta: u32,
        ext: Option<&[u8]>,
    ) -> Result<Vec<VideoRtpPacket>, VideoRtpError> {
        self.packetize_h264_frame(annex_b_frame, timestamp_delta, ext)
    }

    /// Пакетизирует срез AV1 OBU в серию RTP-пакетов (`W=1..3`, `W=0`, `Z/Y` фрагментация)
    /// и сдвигает `timestamp` на `timestamp_delta` тиков.
    pub fn packetize_av1_obus(
        &mut self,
        obus: &[&[u8]],
        timestamp_delta: u32,
        ext: Option<&[u8]>,
    ) -> Result<Vec<VideoRtpPacket>, VideoRtpError> {
        let mut has_seq_hdr = false;
        for &obu in obus {
            if let Some(&hdr) = obu.first() {
                let obu_type = (hdr >> 3) & 0x0F;
                if obu_type == AV1_OBU_SEQUENCE_HEADER {
                    has_seq_hdr = true;
                    self.cached_av1_seq_hdr = Some(obu.to_vec());
                } else if obu_type == AV1_OBU_FRAME {
                    self.cached_av1_frame = Some(obu.to_vec());
                }
            }
        }

        let seq_hdr_clone = if !has_seq_hdr && self.keyframe_ctrl.is_keyframe_requested() {
            self.cached_av1_seq_hdr.clone()
        } else {
            None
        };
        let chunks = if let Some(ref seq_hdr) = seq_hdr_clone {
            let mut upgraded: Vec<&[u8]> = Vec::with_capacity(1 + obus.len());
            upgraded.push(seq_hdr.as_slice());
            upgraded.extend_from_slice(obus);
            packetize_av1_obus_with_mtu(&upgraded, self.max_payload_len)?
        } else {
            packetize_av1_obus_with_mtu(obus, self.max_payload_len)?
        };
        if has_seq_hdr || seq_hdr_clone.is_some() {
            let _ = self.keyframe_ctrl.take_keyframe_request();
        }
        let frame_ts = self.timestamp;
        let step = if timestamp_delta == 0 {
            video_ticks_for_fps(self.target_fps)
        } else {
            timestamp_delta
        };
        let mut packets = Vec::with_capacity(chunks.len());
        for (payload, marker) in chunks {
            let header = RtpHeader {
                marker,
                payload_type: self.payload_type,
                sequence: self.sequence,
                timestamp: frame_ts,
                ssrc: self.ssrc,
            };
            self.sequence = self.sequence.wrapping_add(1);
            packets.push(VideoRtpPacket::new(header, ext, payload)?);
        }
        self.timestamp = self.timestamp.wrapping_add(step);
        Ok(packets)
    }

    /// Возвращает закэшированные наборы параметров H.264 `(SPS, PPS)`, если они были зафиксированы.
    #[must_use]
    pub fn cached_h264_parameter_sets(&self) -> Option<(&[u8], &[u8])> {
        Some((self.cached_sps.as_deref()?, self.cached_pps.as_deref()?))
    }

    /// Явно задаёт закэшированные `SPS` (`NAL 7`) и `PPS` (`NAL 8`) для генерации ключевых кадров H.264 по PLI/FIR.
    pub fn set_h264_parameter_sets(&mut self, sps: Vec<u8>, pps: Vec<u8>) {
        self.cached_sps = Some(sps);
        self.cached_pps = Some(pps);
    }

    /// Возвращает закэшированный `OBU_SEQUENCE_HEADER` (`type = 1`) для AV1, если он был зафиксирован.
    #[must_use]
    pub fn cached_av1_sequence_header(&self) -> Option<&[u8]> {
        self.cached_av1_seq_hdr.as_deref()
    }

    /// Явно задаёт закэшированный `OBU_SEQUENCE_HEADER` для генерации ключевых кадров AV1 по PLI/FIR.
    pub fn set_av1_sequence_header(&mut self, seq_hdr: Vec<u8>) {
        self.cached_av1_seq_hdr = Some(seq_hdr);
    }

    /// Обрабатывает один разобранный `RtcpPacket` (`PLI`, `FIR`, `RR`) и возвращает `true`, если запрошен ключевой кадр.
    pub fn on_rtcp_packet(&mut self, packet: &crate::rtcp::RtcpPacket) -> bool {
        self.keyframe_ctrl.on_rtcp_packet(packet)
    }

    /// Разбирает открытый составной RTCP-пакет и обрабатывает содержащиеся в нём запросы `PLI (PT=206, FMT=1)` / `FIR (PT=206, FMT=4)`.
    pub fn on_rtcp_compound(&mut self, bytes: &[u8]) -> Result<bool, crate::rtcp::RtcpError> {
        let compound = crate::rtcp::parse_compound(bytes)?;
        Ok(self.keyframe_ctrl.on_compound(&compound))
    }

    /// Принудительно выставляет запрос генерации ключевого кадра.
    pub fn request_keyframe(&mut self) {
        self.keyframe_ctrl.request_keyframe();
    }

    /// Проверяет, ожидает ли пакетизатор генерации ключевого кадра (`IDR` / `OBU_SEQUENCE_HEADER`).
    #[must_use]
    pub const fn is_keyframe_requested(&self) -> bool {
        self.keyframe_ctrl.is_keyframe_requested()
    }

    /// Забирает и сбрасывает флаг запроса ключевого кадра.
    pub fn take_keyframe_request(&mut self) -> bool {
        self.keyframe_ctrl.take_keyframe_request()
    }

    /// Количество принятых запросов `RTCP PLI (PT=206, FMT=1)`.
    #[must_use]
    pub const fn pli_received_count(&self) -> u64 {
        self.keyframe_ctrl.inbound_pli_count()
    }

    /// Количество принятых новых запросов `RTCP FIR (PT=206, FMT=4)`.
    #[must_use]
    pub const fn fir_received_count(&self) -> u64 {
        self.keyframe_ctrl.inbound_fir_count()
    }

    /// Немедленно генерирует RTP-пакеты полного ключевого кадра для текущего кодека (`H264`: `STAP-A(SPS,PPS) + IDR`,
    /// `AV1`: `OBU_SEQUENCE_HEADER + OBU_FRAME` с `N = 1`) в ответ на `RTCP PLI` / `FIR` либо для удержания FPS (`2012`, `2002`, `2013`).
    pub fn generate_keyframe_packets(
        &mut self,
        timestamp_delta: u32,
        ext: Option<&[u8]>,
    ) -> Result<Vec<VideoRtpPacket>, VideoRtpError> {
        match self.codec {
            VideoCodec::H264 => {
                let sps = self.cached_sps.clone().unwrap_or_else(|| {
                    vec![
                        0x67, 0x42, 0xC0, 0x1F, 0xDA, 0x01, 0x40, 0x16, 0xEC, 0x04, 0x40,
                    ]
                });
                let pps = self
                    .cached_pps
                    .clone()
                    .unwrap_or_else(|| vec![0x68, 0xCE, 0x3C, 0x80]);
                let idr = self
                    .cached_idr
                    .clone()
                    .unwrap_or_else(|| vec![0x65, 0x88, 0x84, 0x21, 0xA0]);
                let mut annex_b = Vec::with_capacity(12 + sps.len() + pps.len() + idr.len());
                annex_b.extend_from_slice(&[0, 0, 0, 1]);
                annex_b.extend_from_slice(&sps);
                annex_b.extend_from_slice(&[0, 0, 0, 1]);
                annex_b.extend_from_slice(&pps);
                annex_b.extend_from_slice(&[0, 0, 0, 1]);
                annex_b.extend_from_slice(&idr);
                let _ = self.keyframe_ctrl.take_keyframe_request();
                self.packetize_h264_frame(&annex_b, timestamp_delta, ext)
            }
            VideoCodec::Av1 => {
                let seq_hdr = self.cached_av1_seq_hdr.clone().unwrap_or_else(|| {
                    vec![
                        (AV1_OBU_SEQUENCE_HEADER << 3) | 0x02,
                        0x04,
                        0x01,
                        0x02,
                        0x03,
                        0x04,
                    ]
                });
                let frame_obu = self
                    .cached_av1_frame
                    .clone()
                    .unwrap_or_else(|| vec![(AV1_OBU_FRAME << 3) | 0x02, 0x03, 0xAA, 0xBB, 0xCC]);
                let _ = self.keyframe_ctrl.take_keyframe_request();
                self.packetize_av1_obus(
                    &[seq_hdr.as_slice(), frame_obu.as_slice()],
                    timestamp_delta,
                    ext,
                )
            }
        }
    }

    /// Разбирает саморазграничивающийся поток AV1 OBU и пакетизирует его в серию RTP-пакетов.
    pub fn packetize_av1_bitstream(
        &mut self,
        bitstream: &[u8],
        timestamp_delta: u32,
        ext: Option<&[u8]>,
    ) -> Result<Vec<VideoRtpPacket>, VideoRtpError> {
        let obus = parse_av1_obu_stream(bitstream)?;
        self.packetize_av1_obus(&obus, timestamp_delta, ext)
    }

    /// Строит пакет ретрансляции RTX (RFC 4588) для переданного основного пакета `original`,
    /// увеличивая `rtx_sequence` на 1.
    pub fn build_rtx_packet(
        &mut self,
        original: &VideoRtpPacket,
    ) -> Result<VideoRtpPacket, VideoRtpError> {
        let seq = self.rtx_sequence;
        self.rtx_sequence = self.rtx_sequence.wrapping_add(1);
        original.encode_rtx(seq, self.rtx_ssrc, self.rtx_payload_type)
    }
}

/// Формирует JSON-кадр шлюза `Op 18 STREAM_CREATE` для гильдейского стрима (`type: "guild"`).
#[must_use]
pub fn build_stream_create_guild(
    guild_id: &str,
    channel_id: &str,
    preferred_region: Option<&str>,
) -> String {
    let region = preferred_region.map_or(JsonValue::Null, |r| JsonValue::String(r.to_owned()));
    let d = object(vec![
        ("type", JsonValue::String("guild".to_owned())),
        ("guild_id", JsonValue::String(guild_id.to_owned())),
        ("channel_id", JsonValue::String(channel_id.to_owned())),
        ("preferred_region", region),
    ]);
    chord_json::to_json_string(&object(vec![
        ("op", json_u64(u64::from(GW_OP_STREAM_CREATE))),
        ("d", d),
    ]))
}

/// Формирует JSON-кадр шлюза `Op 18 STREAM_CREATE` для звонка DM / Group DM (`type: "call"`).
#[must_use]
pub fn build_stream_create_call(channel_id: &str, preferred_region: Option<&str>) -> String {
    let region = preferred_region.map_or(JsonValue::Null, |r| JsonValue::String(r.to_owned()));
    let d = object(vec![
        ("type", JsonValue::String("call".to_owned())),
        ("guild_id", JsonValue::Null),
        ("channel_id", JsonValue::String(channel_id.to_owned())),
        ("preferred_region", region),
    ]);
    chord_json::to_json_string(&object(vec![
        ("op", json_u64(u64::from(GW_OP_STREAM_CREATE))),
        ("d", d),
    ]))
}

/// Формирует JSON-кадр шлюза `Op 19 STREAM_DELETE`.
#[must_use]
pub fn build_stream_delete(stream_key: &str) -> String {
    let d = object(vec![(
        "stream_key",
        JsonValue::String(stream_key.to_owned()),
    )]);
    chord_json::to_json_string(&object(vec![
        ("op", json_u64(u64::from(GW_OP_STREAM_DELETE))),
        ("d", d),
    ]))
}

/// Формирует JSON-кадр шлюза `Op 20 STREAM_WATCH`.
#[must_use]
pub fn build_stream_watch(stream_key: &str) -> String {
    let d = object(vec![(
        "stream_key",
        JsonValue::String(stream_key.to_owned()),
    )]);
    chord_json::to_json_string(&object(vec![
        ("op", json_u64(u64::from(GW_OP_STREAM_WATCH))),
        ("d", d),
    ]))
}

/// Формирует JSON-кадр шлюза `Op 21 STREAM_PING`.
#[must_use]
pub fn build_stream_ping(stream_key: &str) -> String {
    let d = object(vec![(
        "stream_key",
        JsonValue::String(stream_key.to_owned()),
    )]);
    chord_json::to_json_string(&object(vec![
        ("op", json_u64(u64::from(GW_OP_STREAM_PING))),
        ("d", d),
    ]))
}

/// Формирует JSON-кадр шлюза `Op 22 STREAM_SET_PAUSED`.
#[must_use]
pub fn build_stream_set_paused(stream_key: &str, paused: bool) -> String {
    let d = object(vec![
        ("stream_key", JsonValue::String(stream_key.to_owned())),
        ("paused", JsonValue::Bool(paused)),
    ]);
    chord_json::to_json_string(&object(vec![
        ("op", json_u64(u64::from(GW_OP_STREAM_SET_PAUSED))),
        ("d", d),
    ]))
}
