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

#[test]
fn rfc3550_a3_interval_fraction_lost_and_receiver_report_generation() {
    use discord_voice::rtcp::{RTCP_RR, RTCP_RR_CLEAR_LEN, RtcpPacket, parse_compound};

    // Поток видео 90 кГц (SSRC 40001): проверяем точный алгоритм RFC 3550 Appendix A.3
    // для интервальной доли потерь `fraction_lost` (в 1/256 долях) и генерации PT=201 Receiver Report.
    let mut stats = ReceptionStats::new(90_000);

    // Интервал 1: ожидается 20 пакетов (seq 100..=119), пришло 15 (потеряно 5 -> 5/20 = 25% > 10% порога кода 2003).
    // fraction_lost = floor(5 * 256 / 20) = 64 (0x40).
    for i in 0..20u16 {
        if matches!(i, 3 | 7 | 11 | 14 | 18) {
            continue;
        }
        let hdr = RtpHeader {
            payload_type: 105,
            marker: true,
            sequence: 100 + i,
            timestamp: 90_000 + u32::from(i) * 3_000,
            ssrc: 40_001,
        };
        stats.observe(&hdr, f64::from(i) * 33.333);
    }

    assert!(stats.is_high_packet_loss());
    assert!(approx(stats.loss_rate(), 0.25, 1e-6));
    assert!(approx(stats.interval_loss_rate(), 0.25, 1e-6));

    let peek_rb = stats.peek_report_block(40_001, 0x1122_3344, 0x0001_0000);
    assert_eq!(peek_rb.ssrc, 40_001);
    assert_eq!(peek_rb.fraction_lost, 64);
    assert_eq!(peek_rb.cumulative_lost, 5);
    assert_eq!(peek_rb.extended_highest_seq, 119);
    assert_eq!(peek_rb.last_sr, 0x1122_3344);
    assert_eq!(peek_rb.delay_since_last_sr, 0x0001_0000);

    // Собираем 32-байтный открытый пакет RTCP Receiver Report (PT=201) с фиксацией интервала A.3
    let rr_wire = stats.build_receiver_report(9208, 40_001, 0x1122_3344, 0x0001_0000);
    assert_eq!(rr_wire.len(), RTCP_RR_CLEAR_LEN);
    assert_eq!(rr_wire[0], 0x81); // V=2, P=0, RC=1
    assert_eq!(rr_wire[1], RTCP_RR); // PT=201
    let compound = parse_compound(&rr_wire).expect("valid RTCP RR packet");
    assert_eq!(
        compound.packets,
        vec![RtcpPacket::ReceiverReport {
            sender_ssrc: 9208,
            reports: vec![peek_rb],
        }]
    );

    // Интервал 2: приходят следующие 20 пакетов (seq 120..=139) без единой потери.
    // Кумулятивные потери остаются 5 (из 40 = 12.5%), но интервальная доля потерь fraction_lost = 0!
    for i in 20..40u16 {
        let hdr = RtpHeader {
            payload_type: 105,
            marker: true,
            sequence: 100 + i,
            timestamp: 90_000 + u32::from(i) * 3_000,
            ssrc: 40_001,
        };
        stats.observe(&hdr, f64::from(i) * 33.333);
    }

    assert!(approx(stats.interval_loss_rate(), 0.0, 1e-6));
    assert!(!stats.is_high_packet_loss());

    let rb_interval_2 = stats.next_report_block(40_001, 0x5566_7788, 32_768);
    assert_eq!(rb_interval_2.fraction_lost, 0);
    assert_eq!(rb_interval_2.cumulative_lost, 5);
    assert_eq!(rb_interval_2.extended_highest_seq, 139);
    assert_eq!(rb_interval_2.last_sr, 0x5566_7788);
    assert_eq!(rb_interval_2.delay_since_last_sr, 32_768);
}
