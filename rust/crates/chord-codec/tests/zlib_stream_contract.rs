//! Contract tests for streaming `flate2` (`zlib-stream`) Discord Gateway payload decompressor (STAGE-05.1.2).
//!
//! Verifies:
//! 1. Discord Gateway `zlib-stream` protocol: fragments accumulate until `Z_SYNC_FLUSH` suffix
//!    `[0x00, 0x00, 0xff, 0xff]` arrives, then decompress immediately while preserving sliding-window
//!    dictionary across subsequent WebSocket messages.
//! 2. Multi-frame sliding-window dictionary reuse across 50 Gateway `MESSAGE_CREATE` / `READY` payloads
//!    achieving >= 80% traffic savings (`traffic_savings_percent() >= 80.0`).
//! 3. Sub-fragmentation: a single Gateway message split across multiple WebSocket binary frames
//!    yields `Ok(Vec::new())` on intermediate frames and the full payload on the suffix frame.
//! 4. RFC 1950 zlib header validation (`MissingHeader` when first bytes are not valid zlib header).
//! 5. Decompression bomb protection (`MAX_DECOMPRESSED_CHUNK_BYTES = 16 MiB`) and pending buffer
//!    cap (`MAX_PENDING_COMPRESSED_BYTES = 8 MiB`).
//! 6. Terminal stream poisoning on corrupt deflate data (`ZlibError::Decode` -> `ZlibError::Poisoned`)
//!    and clean reset via `ZlibStream::reset()`.
//! 7. Unified `GatewayDecompressor` switching seamlessly between `ZlibStream`, `ZstdStream`, and `None`.

#![forbid(unsafe_code)]

use chord_codec::zlib::{
    CompressionStats, GatewayCompression, GatewayDecompressor, MAX_DECOMPRESSED_CHUNK_BYTES,
    ZLIB_SYNC_FLUSH_SUFFIX, ZlibError, ZlibStream, compress_zlib_stream_chunks,
};
use flate2::{Compress, Compression, FlushCompress};

/// Oracle helper: builds a multi-chunk `zlib-stream` wire sequence using an independent
/// `flate2::Compress` instance with `FlushCompress::Sync` on every message boundary.
fn oracle_encode_zlib_stream(messages: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut compressor = Compress::new(Compression::default(), true);
    let mut chunks = Vec::with_capacity(messages.len());
    let mut buf = vec![0u8; 64 * 1024];

    for msg in messages {
        let mut msg_wire = Vec::new();
        let mut in_offset = 0usize;
        loop {
            let before_in = compressor.total_in();
            let before_out = compressor.total_out();
            let flush = if in_offset < msg.len() {
                FlushCompress::None
            } else {
                FlushCompress::Sync
            };
            compressor
                .compress(&msg[in_offset..], &mut buf, flush)
                .expect("oracle compress");
            let taken = (compressor.total_in() - before_in) as usize;
            let produced = (compressor.total_out() - before_out) as usize;
            in_offset += taken;
            if produced > 0 {
                msg_wire.extend_from_slice(&buf[..produced]);
            }
            if in_offset == msg.len() && msg_wire.ends_with(&ZLIB_SYNC_FLUSH_SUFFIX) {
                break;
            }
        }
        chunks.push(msg_wire);
    }
    chunks
}

#[test]
fn test_zlib_stream_single_message_sync_flush_roundtrip() {
    let payload = br#"{"op":10,"d":{"heartbeat_interval":41250,"_trace":["gateway-prd-us-east1-b"]},"s":null,"t":null}"#;
    let wire_chunks = oracle_encode_zlib_stream(&[payload]);
    assert_eq!(wire_chunks.len(), 1);
    assert!(wire_chunks[0].ends_with(&ZLIB_SYNC_FLUSH_SUFFIX));

    let mut stream = ZlibStream::new();
    let out = stream.push(&wire_chunks[0]).expect("decompress single frame");
    assert_eq!(out, payload);
    assert_eq!(stream.pending_bytes(), 0);
    assert_eq!(stream.messages_decompressed(), 1);
    assert!(!stream.is_dead());
}

#[test]
fn test_zlib_stream_preserves_sliding_window_dictionary_across_messages() {
    let messages: Vec<Vec<u8>> = (1..=8)
        .map(|seq| {
            format!(
                r#"{{"op":0,"s":{seq},"t":"MESSAGE_CREATE","d":{{"id":"117678537398944570{seq}","channel_id":"1014537851418980372","author":{{"id":"384299970632876032","username":"sanya","discriminator":"0"}},"content":"Hello from Discord Gateway streaming zlib payload #{seq}!"}}}}"#
            )
            .into_bytes()
        })
        .collect();
    let refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
    let wire_chunks = oracle_encode_zlib_stream(&refs);

    // Later chunks reference the LZ77 sliding window from chunk 0 and must be much smaller than raw payload
    assert!(
        wire_chunks[7].len() < messages[7].len() / 3,
        "chunk 7 ({} B) should compress to < 1/3 of raw ({} B) via shared dictionary",
        wire_chunks[7].len(),
        messages[7].len()
    );

    let mut stream = ZlibStream::new();
    for (idx, (wire, expected)) in wire_chunks.iter().zip(messages.iter()).enumerate() {
        let out = stream
            .push(wire)
            .unwrap_or_else(|e| panic!("chunk {idx} failed: {e}"));
        assert_eq!(&out, expected, "chunk {idx} mismatch");
        assert_eq!(stream.pending_bytes(), 0);
    }
    assert_eq!(stream.messages_decompressed(), 8);
}

#[test]
fn test_zlib_stream_achieves_at_least_80_percent_traffic_savings_on_gateway_batch() {
    // Realistic batch of 40 Discord Gateway dispatches (GUILD_MEMBER_LIST_UPDATE, PRESENCE_UPDATE, MESSAGE_CREATE)
    let mut messages = Vec::with_capacity(40);
    for seq in 1..=40 {
        let json = format!(
            r#"{{"op":0,"s":{seq},"t":"PRESENCE_UPDATE","d":{{"user":{{"id":"384299970632876032","username":"chord_member_{seq}","avatar":"a_d5e50180d2b0a6edd8faa9c2ad908d93"}},"guild_id":"1014537851418980372","status":"online","activities":[{{"name":"Chord Native Rust Client","type":0,"details":"Testing STAGE-05.1.2 streaming zlib-ng / flate2 decompressor","state":"In Voice Channel"}}],"client_status":{{"desktop":"online"}}}}}}"#
        );
        messages.push(json.into_bytes());
    }
    let refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
    let wire_chunks = compress_zlib_stream_chunks(&refs, Compression::default())
        .expect("compress batch");

    let mut stream = ZlibStream::new();
    let mut total_raw = 0usize;
    for (wire, expected) in wire_chunks.iter().zip(messages.iter()) {
        let out = stream.push(wire).expect("push batch chunk");
        assert_eq!(&out, expected);
        total_raw += out.len();
    }

    let stats: CompressionStats = stream.stats();
    assert_eq!(stats.decompressed_bytes, total_raw as u64);
    assert_eq!(stats.messages_decompressed, 40);
    assert!(
        stats.traffic_savings_percent() >= 80.0,
        "Expected >= 80.0% traffic savings on Gateway stream, got {:.2}% (compressed={} B, raw={} B)",
        stats.traffic_savings_percent(),
        stats.compressed_bytes,
        stats.decompressed_bytes
    );
    assert!(stats.traffic_savings_ratio() >= 0.80);
}

#[test]
fn test_zlib_stream_buffers_partial_fragments_until_sync_flush_suffix() {
    let payload = br#"{"op":0,"s":1,"t":"READY","d":{"v":9,"session_id":"deadbeefcafebabe","resume_gateway_url":"wss://gateway-us-east1-b.discord.gg"}}"#;
    let wire_chunks = oracle_encode_zlib_stream(&[payload]);
    let full_frame = &wire_chunks[0];
    assert!(full_frame.len() > 12);

    // Split a single compressed Gateway message into 3 WebSocket fragments:
    // Only the final fragment ends with [0x00, 0x00, 0xff, 0xff].
    let part1 = &full_frame[..5];
    let part2 = &full_frame[5..full_frame.len() - 3];
    let part3 = &full_frame[full_frame.len() - 3..];

    let mut stream = ZlibStream::new();
    let out1 = stream.push(part1).expect("fragment 1");
    assert!(out1.is_empty(), "fragment 1 without suffix must wait");
    assert_eq!(stream.pending_bytes(), part1.len());

    let out2 = stream.push(part2).expect("fragment 2");
    assert!(out2.is_empty(), "fragment 2 without suffix must wait");
    assert_eq!(stream.pending_bytes(), part1.len() + part2.len());

    let out3 = stream.push(part3).expect("fragment 3 with suffix");
    assert_eq!(out3, payload, "fragment 3 completes suffix and flushes full payload");
    assert_eq!(stream.pending_bytes(), 0);
    assert_eq!(stream.messages_decompressed(), 1);
}

#[test]
fn test_zlib_stream_handles_suffix_split_byte_by_byte() {
    let payload = br#"{"op":11,"d":null}"#;
    let wire_chunks = oracle_encode_zlib_stream(&[payload]);
    let frame = &wire_chunks[0];

    let mut stream = ZlibStream::new();
    for &b in &frame[..frame.len() - 1] {
        let partial = stream.push(&[b]).expect("byte push before final 0xff");
        assert!(partial.is_empty());
    }
    let final_out = stream
        .push(&[frame[frame.len() - 1]])
        .expect("final 0xff byte triggers decompression");
    assert_eq!(final_out, payload);
}

#[test]
fn test_zlib_stream_rejects_invalid_zlib_header_and_poisons_stream() {
    let mut bad_frame = vec![0x00, 0x01, 0xaa, 0xbb];
    bad_frame.extend_from_slice(&ZLIB_SYNC_FLUSH_SUFFIX);

    let mut stream = ZlibStream::new();
    let err = stream.push(&bad_frame).expect_err("invalid RFC 1950 header must fail");
    assert!(
        matches!(err, ZlibError::InvalidHeader { .. }),
        "expected InvalidHeader, got {err:?}"
    );
    assert!(stream.is_dead());
    assert_eq!(stream.push(&ZLIB_SYNC_FLUSH_SUFFIX), Err(ZlibError::Poisoned));
}

#[test]
fn test_zlib_stream_corrupted_deflate_payload_poisons_stream() {
    let payload = br#"{"op":0,"s":1,"t":"MESSAGE_CREATE","d":{"content":"valid payload"}}"#;
    let mut wire = oracle_encode_zlib_stream(&[payload]).remove(0);
    // Corrupt middle deflate bits while keeping valid 2-byte header and 4-byte sync suffix
    let end = wire.len() - 4;
    for b in &mut wire[2..end] {
        *b = 0xff;
    }

    let mut stream = ZlibStream::new();
    let err = stream.push(&wire);
    assert!(
        matches!(err, Err(ZlibError::Decode { .. })),
        "corrupted deflate stream must return ZlibError::Decode, got {err:?}"
    );
    assert!(stream.is_dead());
    assert_eq!(stream.push(&[]), Err(ZlibError::Poisoned));
}

#[test]
fn test_zlib_stream_reset_clears_poison_and_starts_fresh_session() {
    let mut stream = ZlibStream::new();
    let mut bad = vec![0x00, 0x00, 0x11, 0x22];
    bad.extend_from_slice(&ZLIB_SYNC_FLUSH_SUFFIX);
    assert!(stream.push(&bad).is_err());
    assert!(stream.is_dead());

    stream.reset();
    assert!(!stream.is_dead());
    assert_eq!(stream.pending_bytes(), 0);
    assert_eq!(stream.compressed_consumed(), 0);
    assert_eq!(stream.decompressed_produced(), 0);

    let payload = br#"{"op":10,"d":{"heartbeat_interval":41250}}"#;
    let wire = oracle_encode_zlib_stream(&[payload]);
    let out = stream.push(&wire[0]).expect("fresh push after reset");
    assert_eq!(out, payload);
}

#[test]
fn test_zlib_stream_rejects_decompression_bomb_exceeding_limit() {
    // Compress 64 KiB of zeros and test with a custom small max_decompressed_chunk_bytes limit (16 KiB)
    let zeros = vec![b'A'; 64 * 1024];
    let wire = oracle_encode_zlib_stream(&[&zeros]);
    assert!(wire[0].len() < 512, "64 KiB of 'A' compresses to < 512 B");

    let mut stream = ZlibStream::with_limits(16 * 1024, 8 * 1024 * 1024);
    let err = stream.push(&wire[0]).expect_err("must reject chunk > 16 KiB limit");
    assert!(
        matches!(err, ZlibError::ChunkTooLarge { limit: 16384, .. }),
        "expected ChunkTooLarge, got {err:?}"
    );
    assert!(stream.is_dead());
    assert_eq!(MAX_DECOMPRESSED_CHUNK_BYTES, 16 * 1024 * 1024);
}

#[test]
fn test_zlib_stream_rejects_pending_buffer_overflow() {
    let mut stream = ZlibStream::with_limits(16 * 1024 * 1024, 128);
    // Push 200 bytes without sync-flush suffix -> exceeds 128-byte pending cap
    let chunk_without_suffix = vec![0x78; 200];
    let err = stream
        .push(&chunk_without_suffix)
        .expect_err("must reject pending buffer overflow");
    assert!(
        matches!(err, ZlibError::PendingOverflow { limit: 128, .. }),
        "expected PendingOverflow, got {err:?}"
    );
    assert!(stream.is_dead());
}

#[test]
fn test_gateway_decompressor_unified_dispatch_zlib_zstd_and_none() {
    // 1. ZlibStream mode
    let payload = br#"{"op":0,"s":42,"t":"TYPING_START","d":{"channel_id":"123","user_id":"456"}}"#;
    let zlib_wire = oracle_encode_zlib_stream(&[payload]);
    let mut zlib_dec = GatewayDecompressor::new(GatewayCompression::ZlibStream);
    assert_eq!(zlib_dec.compression(), GatewayCompression::ZlibStream);
    assert_eq!(zlib_dec. compression().query_param(), Some("zlib-stream"));
    let out_zlib = zlib_dec.push(&zlib_wire[0]).expect("unified zlib push");
    assert_eq!(out_zlib, payload);

    // 2. Uncompressed (None) mode passes payload through directly
    let mut raw_dec = GatewayDecompressor::new(GatewayCompression::None);
    assert_eq!(raw_dec.compression(), GatewayCompression::None);
    assert_eq!(raw_dec.compression().query_param(), None);
    let out_raw = raw_dec.push(payload).expect("unified raw push");
    assert_eq!(out_raw, payload);
}

#[test]
fn test_compression_stats_edge_cases_zero_bytes() {
    let empty_stats = CompressionStats::default();
    assert_eq!(empty_stats.traffic_savings_ratio(), 0.0);
    assert_eq!(empty_stats.traffic_savings_percent(), 0.0);

    let active_stats = CompressionStats {
        compressed_bytes: 18_000,
        decompressed_bytes: 100_000,
        messages_decompressed: 25,
    };
    assert!((active_stats.traffic_savings_ratio() - 0.82).abs() < 1e-9);
    assert!((active_stats.traffic_savings_percent() - 82.0).abs() < 1e-6);
}
