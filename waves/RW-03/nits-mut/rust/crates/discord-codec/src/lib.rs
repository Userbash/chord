//! # discord-codec
//!
//! Кодеки кадров шлюза Discord: бинарный **ETF** (erlpack-совместимый) и **zstd-стрим**.
//!
//! Спека: `docs/re/gateway-protocol.md` §2–3 (11 фактических тегов, big-int 110/111,
//! инкрементальный `createZstdDecompress` без `end()`).
//! План: P1-01/P1-02; тесты: T-ENC-01…05 (`docs/rust-client/TEST-PLAN.md` §3.1).
//!
//! Статус: ETF-кодек реализован (RW-01/P0-02; T-ENC-01…03 + fuzz-lite; приёмка 2026-10-19);
//! zstd-стрим — прототип (RW-02/P0-03; T-ENC-04) поверх `ruzstd`; доработка удержания окна — P1-02.

pub mod etf;
pub mod zstd;

pub use etf::{EtfError, EtfValue, MAX_DEPTH, decode, decode_term, encode, encode_term};
pub use zstd::{ZstdError, ZstdStream};

/// Тег ETF `NEW_FLOAT_EXT` (double BE) — 0x46 (`F`), 240 вхождений в захвате.
pub const TAG_NEW_FLOAT_EXT: u8 = 0x46;

/// Тег ETF `NIL_EXT` (→ `null`) — 0x6a (`j`), 12 621 вхождение.
pub const TAG_NIL_EXT: u8 = 0x6a;

/// Полный набор 11 фактических ETF-тегов волны R7 (`gw-proto/facts-frozen.json`).
pub const ETF_TAG_CODES: [u8; 11] = [
    0x46, 0x61, 0x62, 0x64, 0x68, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x74,
];

/// Магия zstd-кадра; поток шлюза — склеенные кадры без `end()` (`gw-proto`).
pub const ZSTD_FRAME_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

/// Смещение для big-int тегов ETF (110/111); знак идёт **до** цифр (см. §2.2 спеки).
pub const BIG_INT_TAGS: [u8; 2] = [110, 111];

#[cfg(test)]
mod tests {
    #[test]
    fn etf_tags_are_the_frozen_eleven() {
        let mut codes = super::ETF_TAG_CODES;
        codes.sort_unstable();
        assert_eq!(codes.len(), 11);
        assert_eq!(codes[0], super::TAG_NEW_FLOAT_EXT);
        assert!(codes.contains(&super::TAG_NIL_EXT));
    }

    #[test]
    fn zstd_magic_matches_capture() {
        assert_eq!(super::ZSTD_FRAME_MAGIC, [0x28, 0xb5, 0x2f, 0xfd]);
    }
}
