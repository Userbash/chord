//! Контракт приёма RTP и статистики потерь/джиттера (P4-07, B07/RW-26).
//!
//! Провенанс: 8 открытых RTP-заголовков клиента из
//! `/home/sanya/re-deep/voice/voice-udp4.pcap`
//! (sha256 `0e6e2f8c…d40e1e2`), SSRC 9208, PT 120, заголовок 24 Б, payload 23 Б.
//! Ожидаемый джиттер посчитан независимо по RFC 3550 §6.4.1 в
//! `waves/RW-26/research/rtcp-extract.py`: 222.141 единиц = 4.628 мс.
//! Копия: `waves/RW-26/research/fixtures/rtp-udp4-client.json`
//! (md5 `c69914605f2a273d1893b7beeca7d483`).

use discord_voice::reception::{
    OPUS_CLOCK_RATE, Observation, ReceptionStats, RtpHeader, parse_rtp_header,
};

/// (arrival_ms, rtp_timestamp) из capture; seq идёт 17466..17473 без пропусков.
const CAPTURE: [(f64, u32); 8] = [
    (1_792_395_568_071.428, 278_470_988),
    (1_792_395_568_091.893, 278_471_948),
    (1_792_395_568_092.791, 278_472_908),
    (1_792_395_568_113.603, 278_473_868),
    (1_792_395_568_156.431, 278_474_828),
    (1_792_395_568_157.201, 278_475_788),
    (1_792_395_568_178.493, 278_476_748),
    (1_792_395_568_220.841, 278_477_708),
];

const CAPTURE_SSRC: u32 = 9208;

fn capture_header(index: usize) -> RtpHeader {
    RtpHeader {
        payload_type: 120,
        marker: false,
        sequence: 17_466 + index as u16,
        timestamp: CAPTURE[index].1,
        ssrc: CAPTURE_SSRC,
    }
}

fn stream_header(sequence: u16, timestamp: u32) -> RtpHeader {
    RtpHeader {
        payload_type: 120,
        marker: false,
        sequence,
        timestamp,
        ssrc: 7,
    }
}

fn approx(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}

#[test]
fn parses_real_rtp_headers() {
    // 12 Б фикс + 4 Б ext-заголовок (0xBEDE, len=2) + 8 Б данных; статистике нужен
    // только фиксированный заголовок, поэтому ext можно не разбирать.
    let mut bytes = vec![0x90, 120, 0x44, 0x3a];
    bytes.extend_from_slice(&278_470_988u32.to_be_bytes());
    bytes.extend_from_slice(&9208u32.to_be_bytes());
    bytes.extend_from_slice(&[0xbe, 0xde, 0x00, 0x02, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        parse_rtp_header(&bytes),
        Some(RtpHeader {
            payload_type: 120,
            marker: false,
            sequence: 17_466,
            timestamp: 278_470_988,
            ssrc: 9208,
        })
    );
}

#[test]
fn rejects_non_rtp_and_short_input() {
    assert_eq!(parse_rtp_header(&[]), None);
    assert_eq!(parse_rtp_header(&[0x80; 11]), None);
    // V=1 — не RTP.
    let mut bytes = vec![0x40; 12];
    bytes[1] = 120;
    assert_eq!(parse_rtp_header(&bytes), None);
}

#[test]
fn real_capture_matches_rfc_jitter_oracle() {
    let mut stats = ReceptionStats::new(OPUS_CLOCK_RATE);
    for (index, (arrival, _)) in CAPTURE.iter().enumerate() {
        assert_eq!(
            stats.observe(&capture_header(index), *arrival),
            Observation::InOrder
        );
    }
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.received, 8);
    assert_eq!(snapshot.expected, 8);
    assert_eq!(snapshot.lost, 0);
    assert_eq!(snapshot.duplicates, 0);
    assert_eq!(snapshot.highest_sequence, Some(17_473));
    assert!(
        approx(snapshot.jitter_units, 222.141, 0.01),
        "jitter_units = {}",
        snapshot.jitter_units
    );
    assert!(
        approx(snapshot.jitter_ms, 4.628, 0.001),
        "jitter_ms = {}",
        snapshot.jitter_ms
    );
}

#[test]
fn sequence_wrap_does_not_create_loss() {
    let mut stats = ReceptionStats::new(OPUS_CLOCK_RATE);
    for (index, sequence) in [65_534u16, 65_535, 0, 1].iter().enumerate() {
        stats.observe(
            &stream_header(*sequence, index as u32 * 960),
            index as f64 * 20.0,
        );
    }
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.received, 4);
    assert_eq!(snapshot.expected, 4);
    assert_eq!(snapshot.lost, 0);
    assert_eq!(snapshot.highest_sequence, Some(1));
}

#[test]
fn gap_across_wrap_counts_one_lost_packet() {
    let mut stats = ReceptionStats::new(OPUS_CLOCK_RATE);
    stats.observe(&stream_header(65_534, 0), 0.0);
    stats.observe(&stream_header(0, 2 * 960), 40.0);
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.received, 2);
    assert_eq!(snapshot.expected, 3);
    assert_eq!(snapshot.lost, 1);
}

#[test]
fn reordering_is_reported_but_not_lost() {
    let mut stats = ReceptionStats::new(OPUS_CLOCK_RATE);
    let observations: Vec<_> = [1u16, 3, 2, 4]
        .iter()
        .enumerate()
        .map(|(index, sequence)| {
            stats.observe(
                &stream_header(*sequence, index as u32 * 960),
                index as f64 * 20.0,
            )
        })
        .collect();
    assert_eq!(
        observations,
        vec![
            Observation::InOrder,
            Observation::InOrder,
            Observation::Reordered,
            Observation::InOrder
        ]
    );
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.received, 4);
    assert_eq!(snapshot.lost, 0);
}

#[test]
fn timestamp_wrap_does_not_spike_jitter() {
    let mut stats = ReceptionStats::new(OPUS_CLOCK_RATE);
    let start = u32::MAX - 960 * 3;
    for index in 0..8u32 {
        stats.observe(
            &stream_header(index as u16 + 1, start.wrapping_add(index * 960)),
            f64::from(index) * 20.0,
        );
    }
    let jitter = stats.snapshot().jitter_units;
    assert!(approx(jitter, 0.0, 1e-6), "jitter={jitter}");
}

#[test]
fn duplicate_does_not_inflate_received_or_jitter() {
    let mut stats = ReceptionStats::new(OPUS_CLOCK_RATE);
    stats.observe(&stream_header(1, 0), 0.0);
    stats.observe(&stream_header(2, 960), 20.0);
    let before = stats.snapshot();
    assert_eq!(
        stats.observe(&stream_header(2, 960), 40.0),
        Observation::Duplicate
    );
    let after = stats.snapshot();
    assert_eq!(after.received, before.received);
    assert_eq!(after.duplicates, 1);
    assert_eq!(after.jitter_units, before.jitter_units);
}

#[test]
fn empty_stats_are_zeroed() {
    let stats = ReceptionStats::new(OPUS_CLOCK_RATE);
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.received, 0);
    assert_eq!(snapshot.expected, 0);
    assert_eq!(snapshot.lost, 0);
    assert_eq!(snapshot.highest_sequence, None);
    assert_eq!(snapshot.jitter_units, 0.0);
}

#[test]
fn zero_clock_rate_does_not_panic() {
    let mut stats = ReceptionStats::new(0);
    stats.observe(&stream_header(1, 0), 0.0);
    stats.observe(&stream_header(2, 960), 20.0);
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.received, 2);
    assert!(snapshot.jitter_ms.is_finite());
}
