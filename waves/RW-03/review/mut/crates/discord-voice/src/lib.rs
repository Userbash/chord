//! # discord-voice
//!
//! Голосовой стек: WebSocket RTC-сигнализация, UDP-медиа, SRTP (`aead_aes256_gcm_rtpsize`),
//! keepalive-обмен и рукопожатие DAVE/MLS.
//!
//! Спека: `docs/re/voice-deep.md` §2–3 (сессии s2/s3, константы из strace).
//! План: P3 (сигнализация) и P4.5 (DAVE/MLS); тесты: T-VOICE-*.
//!
//! Статус: каркас P0 — константы, восстановленные наблюдательно.

/// Payload type OPUS для медиа-канала (Select Protocol, priority 1000).
pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;

/// Payload type AV1 (video) из списка кодеков Select Protocol.
pub const RTP_AV1_PAYLOAD_TYPE: u8 = 101;

/// Режим шифрования из Session Description (s2/s3).
pub const MEDIA_ENCRYPTION_MODE: &str = "aead_aes256_gcm_rtpsize";

/// Версия протокола DAVE (MLS) в Session Description.
pub const DAVE_PROTOCOL_VERSION: u8 = 1;

/// Версия secure frames в Session Description.
pub const SECURE_FRAMES_VERSION: u8 = 1;

/// Интервал heartbeat голосового шлюза (op8 Hello) — 13 750 мс.
pub const VOICE_HEARTBEAT_INTERVAL_MS: u64 = 13_750;

/// Магия keepalive-пакета (Little Endian, из strace-дампов s3).
pub const KEEPALIVE_MAGIC: u32 = 0x1337_cafe;

/// Магия ответа на keepalive.
pub const KEEPALIVE_REPLY: u32 = 0x1337_f00d;

/// Наблюдавшиеся op-коды RTC-сигнализации (union сессий 1–2).
pub const OBSERVED_RTC_OPS: [u8; 9] = [0, 1, 2, 3, 4, 6, 8, 15, 16];

#[cfg(test)]
mod tests {
    #[test]
    fn voice_constants_match_re() {
        assert_eq!(super::RTP_OPUS_PAYLOAD_TYPE, 120);
        assert_eq!(super::VOICE_HEARTBEAT_INTERVAL_MS, 13_750);
        assert_eq!(super::KEEPALIVE_MAGIC, 0x1337_cafe);
        assert_ne!(super::KEEPALIVE_MAGIC, super::KEEPALIVE_REPLY);
        assert_eq!(super::OBSERVED_RTC_OPS.len(), 9);
    }
}
