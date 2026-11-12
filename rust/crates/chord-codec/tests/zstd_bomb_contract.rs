//! Контракт защиты `ZstdStream` от zip-bomb / RLE-бомб и сверхбольших окон (`TASK-903.1`).
//!
//! Оракул: RFC 8878 §3.1.1 (Zstandard Frame Header + RLE Blocks `Block_Type = 1`).
//! Сжатый фрейм длиной < 1 КиБ из RLE-блоков по 128 КиБ при отсутствии лимита
//! распаковывается в десятки мегабайт за один `push`, вызывая OOM при фаззинге и на шлюзе.

use chord_codec::zstd::{MAX_DECOMPRESSED_CHUNK_BYTES, ZstdError, ZstdStream};

/// Собирает валидный по RFC 8878 zstd-кадр из `rle_blocks` блоков типа RLE (`Block_Type = 1`),
/// каждый из которых порождает `128 КиБ` байта `0x41` при стоимости всего 4 байта на входе.
fn build_rfc8878_rle_bomb(rle_blocks: usize) -> Vec<u8> {
    let mut wire = Vec::with_capacity(6 + rle_blocks * 4);
    // ZSTD magic (RFC 8878 §3.1.1): 0xFD2FB528 LE
    wire.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD]);
    // Frame_Header_Descriptor = 0x00 (Window_Descriptor присутствует, FCS отсутствует)
    wire.push(0x00);
    // Window_Descriptor = 0x58: Exponent = 11 -> WindowLog = 10 + 11 = 21 (2 МиБ >= 128 КиБ блока)
    wire.push(0x58);

    let block_regen_len: u32 = 128 * 1024;
    for idx in 0..rle_blocks {
        let last_bit = u32::from(idx + 1 == rle_blocks);
        // Block_Header (24-bit LE): Last_Block (bit 0) | Block_Type=1 RLE (bits 1..2) | Size (bits 3..23)
        let hdr = (block_regen_len << 3) | (1u32 << 1) | last_bit;
        let le = hdr.to_le_bytes();
        wire.extend_from_slice(&le[..3]);
        wire.push(b'A');
    }
    wire
}

#[test]
fn zstd_stream_rejects_single_push_rle_decompression_bomb() {
    // 200 RLE-блоков по 128 КиБ = 25 МиБ распакованного вывода из 806 байт входа (> 16 МиБ лимита).
    let bomb = build_rfc8878_rle_bomb(200);
    assert!(bomb.len() < 1024);
    const { assert!(200 * 128 * 1024 > MAX_DECOMPRESSED_CHUNK_BYTES) };

    let mut stream = ZstdStream::new();
    let result = stream.push(&bomb);
    assert!(
        matches!(result, Err(ZstdError::Decode { .. })),
        "RLE zip-bomb обязана отклоняться ошибкой Decode до исчерпания памяти, получено len={:?}",
        result.as_ref().map(Vec::len)
    );
    assert!(
        stream.is_dead(),
        "после RLE-бомбы поток обязан стать dead/poisoned"
    );
    assert_eq!(stream.push(b"28b52ffd"), Err(ZstdError::Poisoned));
}

#[test]
fn zstd_stream_rejects_oversized_window_descriptor() {
    // Window_Descriptor = 0xF8 -> Exponent = 31 -> запрос окна > 16 МиБ (WindowLogMax = 24)
    let mut oversized_window_frame = vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0xF8];
    // Один пустой финальный Raw-блок (Last=1, Type=0, Size=1) + 1 байт
    let hdr = (1u32 << 3) | 1u32;
    let le = hdr.to_le_bytes();
    oversized_window_frame.extend_from_slice(&le[..3]);
    oversized_window_frame.push(0x83);

    let mut stream = ZstdStream::new();
    let result = stream.push(&oversized_window_frame);
    assert!(
        matches!(result, Err(ZstdError::Decode { .. })),
        "кадр со сверхбольшим окном распаковки обязан отклоняться libzstd WindowLogMax"
    );
    assert!(stream.is_dead());
}
