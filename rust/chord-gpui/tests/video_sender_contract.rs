//! Contract tests for P1-2.4: RTP Video Packetizer (90 kHz, MTU 1200, Marker Bit, NACK Retransmit Ring)
//! in `rust/chord-gpui/src/video/sender.rs`.

#![forbid(unsafe_code)]

#[path = "../src/video/sender.rs"]
mod sender;

use sender::{
    DEFAULT_MAX_FRAME_BYTES, DEFAULT_MTU_PAYLOAD_BYTES, DEFAULT_NACK_RING_CAPACITY,
    DEFAULT_VIDEO_PAYLOAD_TYPE, MAX_RTP_PAYLOAD_TYPE, MIN_MTU_PAYLOAD_BYTES, RTP_FIXED_HEADER_LEN,
    RTP_VERSION, RTP_VIDEO_CLOCK_HZ, RtpHeader, VideoRtpSender, VideoRtpSenderConfig,
    VideoSenderError,
};

#[test]
fn single_packet_small_frame_sets_marker_and_roundtrips_rfc3550_header() {
    let mut sender = VideoRtpSender::new(0x1234_5678, DEFAULT_VIDEO_PAYLOAD_TYPE)
        .expect("valid ssrc and payload type");

    assert_eq!(sender.ssrc(), 0x1234_5678);
    assert_eq!(sender.payload_type(), DEFAULT_VIDEO_PAYLOAD_TYPE);
    assert_eq!(sender.mtu_payload_bytes(), DEFAULT_MTU_PAYLOAD_BYTES);

    let payload = vec![0xAB; 500];
    let pts = 90_000;
    let packets = sender
        .packetize_frame(&payload, pts)
        .expect("500-byte frame should packetize into 1 RTP packet");

    assert_eq!(packets.len(), 1);
    let pkt = &packets[0];
    assert_eq!(pkt.len(), RTP_FIXED_HEADER_LEN + 500);

    // Verify exact RFC 3550 big-endian header layout
    // Byte 0: V=2 (0x80), P=0, X=0, CC=0
    assert_eq!(pkt[0], 0x80);
    // Byte 1: M=1 (0x80) | PT=96 (0x60) = 0xE0
    assert_eq!(pkt[1], 0x80 | DEFAULT_VIDEO_PAYLOAD_TYPE);
    // Bytes 2..4: sequence_number = 0
    assert_eq!(&pkt[2..4], &0u16.to_be_bytes());
    // Bytes 4..8: timestamp = 90_000
    assert_eq!(&pkt[4..8], &90_000u32.to_be_bytes());
    // Bytes 8..12: ssrc = 0x1234_5678
    assert_eq!(&pkt[8..12], &0x1234_5678u32.to_be_bytes());
    // Payload matches
    assert_eq!(&pkt[RTP_FIXED_HEADER_LEN..], payload.as_slice());

    // Roundtrip decode
    let decoded = RtpHeader::decode(pkt).expect("valid 12-byte RTP header");
    assert_eq!(decoded.version, RTP_VERSION);
    assert!(!decoded.padding);
    assert!(!decoded.extension);
    assert_eq!(decoded.csrc_count, 0);
    assert!(decoded.marker);
    assert_eq!(decoded.payload_type, DEFAULT_VIDEO_PAYLOAD_TYPE);
    assert_eq!(decoded.sequence_number, 0);
    assert_eq!(decoded.timestamp, pts);
    assert_eq!(decoded.ssrc, 0x1234_5678);
    assert_eq!(decoded.encode(), pkt[..RTP_FIXED_HEADER_LEN]);
}

#[test]
fn multi_packet_keyframe_splits_by_mtu_and_marks_only_final_fragment() {
    let mut sender = VideoRtpSender::new(9209, 101).expect("valid sender");
    // 3,000 bytes with 1200 MTU -> 3 packets of payload 1200, 1200, 600
    let frame: Vec<u8> = (0..3000).map(|i| (i % 251) as u8).collect();
    let pts = 270_000;

    let packets = sender
        .packetize_frame(&frame, pts)
        .expect("3000-byte keyframe should split into 3 packets");

    assert_eq!(packets.len(), 3);
    assert_eq!(packets[0].len(), RTP_FIXED_HEADER_LEN + 1200);
    assert_eq!(packets[1].len(), RTP_FIXED_HEADER_LEN + 1200);
    assert_eq!(packets[2].len(), RTP_FIXED_HEADER_LEN + 600);

    let h0 = RtpHeader::decode(&packets[0]).unwrap();
    let h1 = RtpHeader::decode(&packets[1]).unwrap();
    let h2 = RtpHeader::decode(&packets[2]).unwrap();

    // Marker bit MUST be false on fragments 0 and 1, and true ONLY on fragment 2
    assert!(!h0.marker, "first fragment must have M=0");
    assert!(!h1.marker, "middle fragment must have M=0");
    assert!(h2.marker, "final fragment must have M=1");

    // Sequential sequence numbers
    assert_eq!(h0.sequence_number, 0);
    assert_eq!(h1.sequence_number, 1);
    assert_eq!(h2.sequence_number, 2);

    // Identical 90 kHz timestamp and SSRC across all fragments of the frame
    for h in [h0, h1, h2] {
        assert_eq!(h.timestamp, pts);
        assert_eq!(h.ssrc, 9209);
        assert_eq!(h.payload_type, 101);
    }

    // Reassembled payload matches original frame
    let mut reassembled = Vec::new();
    for pkt in &packets {
        reassembled.extend_from_slice(&pkt[RTP_FIXED_HEADER_LEN..]);
    }
    assert_eq!(reassembled, frame);

    let stats = sender.stats();
    assert_eq!(stats.frames_sent, 1);
    assert_eq!(stats.packets_sent, 3);
    assert_eq!(stats.bytes_sent, 3 * RTP_FIXED_HEADER_LEN as u64 + 3000);
}

#[test]
fn exact_mtu_multiple_and_boundary_sizes_fragment_accurately() {
    let mut sender = VideoRtpSender::new(17844, MAX_RTP_PAYLOAD_TYPE).expect("valid sender");

    // Exact 1x MTU (1200 bytes) -> 1 packet with M=1
    let exact_one = vec![0x42; DEFAULT_MTU_PAYLOAD_BYTES];
    let pkts1 = sender.packetize_frame(&exact_one, 3000).unwrap();
    assert_eq!(pkts1.len(), 1);
    assert_eq!(
        pkts1[0].len(),
        RTP_FIXED_HEADER_LEN + DEFAULT_MTU_PAYLOAD_BYTES
    );
    assert!(RtpHeader::decode(&pkts1[0]).unwrap().marker);

    // Exact 2x MTU (2400 bytes) -> 2 packets (1200, 1200), M=0 then M=1
    let exact_two = vec![0x24; DEFAULT_MTU_PAYLOAD_BYTES * 2];
    let pkts2 = sender.packetize_frame(&exact_two, 6000).unwrap();
    assert_eq!(pkts2.len(), 2);
    assert!(!RtpHeader::decode(&pkts2[0]).unwrap().marker);
    assert!(RtpHeader::decode(&pkts2[1]).unwrap().marker);
}

#[test]
fn sequence_number_and_timestamp_wrap_around_cleanly() {
    let mut sender = VideoRtpSender::new(17844, DEFAULT_VIDEO_PAYLOAD_TYPE)
        .expect("valid sender")
        .with_initial_sequence_number(65534)
        .with_initial_timestamp(u32::MAX - 1000);

    // 4 packets in one frame: seq should go 65534 -> 65535 -> 0 -> 1
    let frame = vec![0x55; DEFAULT_MTU_PAYLOAD_BYTES * 3 + 100];
    let pts = sender.current_timestamp();
    let packets = sender.packetize_frame(&frame, pts).unwrap();
    assert_eq!(packets.len(), 4);

    let seqs: Vec<u16> = packets
        .iter()
        .map(|p| RtpHeader::decode(p).unwrap().sequence_number)
        .collect();
    assert_eq!(seqs, vec![65534, 65535, 0, 1]);
    assert_eq!(sender.next_sequence_number(), 2);

    // NACK retransmit across u16 wraparound (65535 and 0)
    let retrans = sender.retransmit_nack(&[65535, 0]);
    assert_eq!(retrans.len(), 2);
    assert_eq!(
        RtpHeader::decode(&retrans[0]).unwrap().sequence_number,
        65535
    );
    assert_eq!(RtpHeader::decode(&retrans[1]).unwrap().sequence_number, 0);

    // Advance timestamp at 30 fps (+3000 ticks) across u32::MAX
    let next_ts = sender.advance_timestamp_for_fps(30);
    assert_eq!(next_ts, (u32::MAX - 1000).wrapping_add(3000));
    assert_eq!(RTP_VIDEO_CLOCK_HZ, 90_000);

    // Advance timestamp at 60 fps (+1500 ticks)
    let ts_after_60 = sender.advance_timestamp_for_fps(60);
    assert_eq!(ts_after_60, next_ts.wrapping_add(1500));

    // Advance timestamp with fps == 0 fallback (+90_000 ticks)
    let ts_after_zero = sender.advance_timestamp_for_fps(0);
    assert_eq!(ts_after_zero, ts_after_60.wrapping_add(RTP_VIDEO_CLOCK_HZ));
}

#[test]
fn nack_retransmit_returns_cached_packets_and_records_misses_on_overwrite() {
    let config = VideoRtpSenderConfig {
        ssrc: 0xCAFE_BABE,
        payload_type: 96,
        mtu_payload_bytes: MIN_MTU_PAYLOAD_BYTES,
        max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        nack_ring_capacity: 4,
    };
    let mut sender = VideoRtpSender::with_config(config).expect("valid config");

    // Emit 4 single-packet frames -> sequences 0, 1, 2, 3
    for i in 0..4u8 {
        let pkt = sender
            .packetize_frame(&vec![i + 1; 50], (i as u32) * 3000)
            .unwrap();
        assert_eq!(pkt.len(), 1);
    }

    // Retransmit seq 1 and seq 3 -> both should hit
    let retransmitted = sender.retransmit_nack(&[1, 3]);
    assert_eq!(retransmitted.len(), 2);
    assert_eq!(
        RtpHeader::decode(&retransmitted[0])
            .unwrap()
            .sequence_number,
        1
    );
    assert_eq!(&retransmitted[0][RTP_FIXED_HEADER_LEN..], &[2u8; 50]);
    assert_eq!(
        RtpHeader::decode(&retransmitted[1])
            .unwrap()
            .sequence_number,
        3
    );
    assert_eq!(&retransmitted[1][RTP_FIXED_HEADER_LEN..], &[4u8; 50]);
    assert_eq!(sender.stats().nack_retransmits, 2);
    assert_eq!(sender.stats().nack_misses, 0);

    // Emit 2 more packets (sequences 4 and 5) -> overwrites slots for seq 0 and seq 1 (since ring capacity = 4)
    sender.packetize_frame(&[0xEE; 50], 12_000).unwrap();
    sender.packetize_frame(&[0xFF; 50], 15_000).unwrap();

    // Request NACK for [0 (overwritten), 1 (overwritten), 2 (still in ring), 5 (in ring), 99 (never sent)]
    let after_wrap = sender.retransmit_nack(&[0, 1, 2, 5, 99]);
    assert_eq!(after_wrap.len(), 2);
    assert_eq!(
        RtpHeader::decode(&after_wrap[0]).unwrap().sequence_number,
        2
    );
    assert_eq!(
        RtpHeader::decode(&after_wrap[1]).unwrap().sequence_number,
        5
    );

    assert_eq!(sender.stats().nack_retransmits, 4);
    assert_eq!(sender.stats().nack_misses, 3);
}

#[test]
fn rtp_header_flags_and_validation_reject_invalid_inputs() {
    // RtpHeader::new validation
    assert_eq!(
        RtpHeader::new(true, 96, 1, 1000, 0).unwrap_err(),
        VideoSenderError::InvalidSsrc
    );
    assert_eq!(
        RtpHeader::new(true, 128, 1, 1000, 1).unwrap_err(),
        VideoSenderError::InvalidPayloadType(128)
    );

    // Header flags roundtrip (padding, extension, csrc_count)
    let mut custom_hdr = RtpHeader::new(false, 107, 42, 90_000, 0xDEAD_BEEF).unwrap();
    custom_hdr.padding = true;
    custom_hdr.extension = true;
    custom_hdr.csrc_count = 3;
    let bytes = custom_hdr.encode();
    let decoded = RtpHeader::decode(&bytes).unwrap();
    assert_eq!(decoded, custom_hdr);

    // ssrc == 0 rejected
    assert_eq!(
        VideoRtpSender::new(0, 96).unwrap_err(),
        VideoSenderError::InvalidSsrc
    );

    // payload_type > 127 rejected
    assert_eq!(
        VideoRtpSender::new(1234, 128).unwrap_err(),
        VideoSenderError::InvalidPayloadType(128)
    );

    // mtu < MIN_MTU_PAYLOAD_BYTES (64) rejected
    let bad_mtu_cfg = VideoRtpSenderConfig {
        ssrc: 1234,
        payload_type: 96,
        mtu_payload_bytes: MIN_MTU_PAYLOAD_BYTES - 1,
        max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        nack_ring_capacity: DEFAULT_NACK_RING_CAPACITY,
    };
    assert_eq!(
        VideoRtpSender::with_config(bad_mtu_cfg).unwrap_err(),
        VideoSenderError::InvalidMtu(MIN_MTU_PAYLOAD_BYTES - 1)
    );

    // nack_ring_capacity == 0 rejected
    let bad_nack_cfg = VideoRtpSenderConfig {
        ssrc: 1234,
        payload_type: 96,
        mtu_payload_bytes: DEFAULT_MTU_PAYLOAD_BYTES,
        max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        nack_ring_capacity: 0,
    };
    assert_eq!(
        VideoRtpSender::with_config(bad_nack_cfg).unwrap_err(),
        VideoSenderError::InvalidNackCapacity(0)
    );

    let mut sender = VideoRtpSender::with_config(VideoRtpSenderConfig {
        ssrc: 5555,
        payload_type: 96,
        mtu_payload_bytes: DEFAULT_MTU_PAYLOAD_BYTES,
        max_frame_bytes: 4096,
        nack_ring_capacity: DEFAULT_NACK_RING_CAPACITY,
    })
    .unwrap();

    let seq_before = sender.next_sequence_number();

    // Empty frame rejected without advancing sequence number
    assert_eq!(
        sender.packetize_frame(&[], 3000).unwrap_err(),
        VideoSenderError::EmptyFrame
    );
    assert_eq!(sender.next_sequence_number(), seq_before);

    // Frame == max_frame_bytes succeeds
    assert!(sender.packetize_frame(&vec![0xAA; 4096], 3000).is_ok());

    // Frame > max_frame_bytes rejected without advancing sequence number
    let seq_after_ok = sender.next_sequence_number();
    let oversized = vec![0x11; 4097];
    assert_eq!(
        sender.packetize_frame(&oversized, 6000).unwrap_err(),
        VideoSenderError::FrameTooLarge {
            size: 4097,
            max: 4096,
        }
    );
    assert_eq!(sender.next_sequence_number(), seq_after_ok);

    // Truncated header decode (< 12 bytes) rejected
    assert_eq!(
        RtpHeader::decode(&[0x80, 0x60, 0x00]).unwrap_err(),
        VideoSenderError::PacketTooShort {
            actual: 3,
            minimum: RTP_FIXED_HEADER_LEN,
        }
    );

    // Invalid RTP version in decode rejected
    let mut bad_ver = [0u8; 12];
    bad_ver[0] = 0x40; // V=1
    bad_ver[8..12].copy_from_slice(&1u32.to_be_bytes());
    assert_eq!(
        RtpHeader::decode(&bad_ver).unwrap_err(),
        VideoSenderError::InvalidVersion(1)
    );

    // Zero SSRC in decode rejected
    let mut zero_ssrc = [0u8; 12];
    zero_ssrc[0] = 0x80; // V=2
    assert_eq!(
        RtpHeader::decode(&zero_ssrc).unwrap_err(),
        VideoSenderError::InvalidSsrc
    );

    // Error Display messages are non-empty
    assert!(!VideoSenderError::InvalidSsrc.to_string().is_empty());
    assert!(!VideoSenderError::EmptyFrame.to_string().is_empty());
}
