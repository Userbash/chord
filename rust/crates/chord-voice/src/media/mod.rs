//! RTP-медиа Discord (P4-04, headless): байтовый слой заголовка/расширения,
//! классификация DTX/comfort-noise и детерминированный джиттер-буфер.
//!
//! Источник фактов — `docs/re/voice-deep.md` §2.3 и golden-оракул
//! `fixtures/voice/rtp-pt120-oracle.json` (10 пакетов PT120 из `voice-udp4.pcap`,
//! только чтение): 12 Б заголовка, расширение RFC 5285 `0xBEDE` (2 слова, 8 Б),
//! хвост `[ciphertext][tag 16][counter 4 LE]`, каденция 20 мс (960 тиков).
//!
//! Границы карточки: сети, Opus-SDK и ключевого материала здесь нет; SRTP-кадрирование
//! — P4-03. Wire-совместимость SRTP этим модулем не заявляется, проверяется только
//! байтовая раскладка RTP и поведение буфера.

pub mod jitter;
pub mod rtp;

pub use jitter::{JitterBuffer, JitterStats, PopOutcome, PushOutcome};
pub use rtp::{
    AudioFrameKind, EXTENSION_PROFILE, FRAME_DURATION_MS, FRAME_TICKS, OPUS_CLOCK_RATE,
    OPUS_PAYLOAD_TYPE, RtpError, RtpExtension, RtpHeader, RtpPacket, RtpSender,
    classify_opus_payload, parse_rtp,
};
