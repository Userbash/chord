//! Контракт RTT-оценки и окна статистики (P4-07, B07/RW-26).
//!
//! Формулы — RFC 3550 §6.4.1/§6.4.2: RTT = (arrival − LSR_time) − DLSR,
//! DLSR/LSR — в единицах 1/65536 с. Живых открытых SR/RR-пар в capture нет
//! (`waves/RW-26/research/RE-VOICE-STATS.md` §2), поэтому векторы синтетические.

use discord_voice::rtcp::{
    RTCP_RR, RTCP_SR, RTCP_SR_INTERVAL_MS, ReportBlock, RtcpPacket, RtcpSenderScheduler,
    SenderInfo, build_receiver_report, build_sender_report, build_srtcp_receiver_report,
    build_srtcp_sender_report, is_server_receiver_report, parse_compound,
    parse_server_receiver_report,
};
use discord_voice::stats::{RttEstimator, SessionStats, StatsSample, StatsWindow, ntp_middle_32};

fn block(lsr: u32, dlsr: u32) -> ReportBlock {
    ReportBlock {
        ssrc: 1,
        fraction_lost: 0,
        cumulative_lost: 0,
        extended_highest_seq: 0,
        jitter: 0,
        last_sr: lsr,
        delay_since_last_sr: dlsr,
    }
}

fn sample(at_ms: u64, rtt_ms: Option<f64>, cumulative_lost: i64, jitter_ms: f64) -> StatsSample {
    StatsSample {
        at_ms,
        rtt_ms,
        fraction_lost: 0.0,
        cumulative_lost,
        jitter_ms,
    }
}

#[test]
fn ntp_middle_uses_high_word() {
    assert_eq!(ntp_middle_32(0x8abb_34e7_9b75_ca2c), 0x8abb_34e7);
    assert_eq!(ntp_middle_32(0x0000_0000_ffff_ffff), 0);
}

#[test]
fn rtt_subtracts_dlsr() {
    let mut estimator = RttEstimator::new();
    estimator.note_sender_report(0x1234_5678, 1_000);
    // DLSR 6554 = 0.100006 с → RTT ≈ 100 мс - 100.006 мс < 0 → клампится в 0.
    let rtt = estimator.on_report_block(&block(0x1234_5678, 6_554), 1_100);
    assert_eq!(rtt, Some(0.0));
}

#[test]
fn rtt_without_dlsr_is_plain_round_trip() {
    let mut estimator = RttEstimator::new();
    estimator.note_sender_report(0x1234_5678, 1_000);
    let rtt = estimator.on_report_block(&block(0x1234_5678, 0), 1_100);
    assert!((rtt.unwrap() - 100.0).abs() < 1e-9);
}

#[test]
fn rtt_with_25_percent_dlsr() {
    let mut estimator = RttEstimator::new();
    estimator.note_sender_report(0x0bad_cafe, 500);
    // 16384 / 65536 = 0.25 с = 250 мс; 400 - 250 = 150 мс.
    let rtt = estimator.on_report_block(&block(0x0bad_cafe, 16_384), 900);
    assert!((rtt.unwrap() - 150.0).abs() < 1e-9);
}

#[test]
fn unknown_or_zero_lsr_returns_none() {
    let mut estimator = RttEstimator::new();
    estimator.note_sender_report(7, 100);
    assert_eq!(estimator.on_report_block(&block(0, 0), 200), None);
    assert_eq!(estimator.on_report_block(&block(8, 0), 200), None);
}

#[test]
fn negative_rtt_is_clamped_to_zero() {
    let mut estimator = RttEstimator::new();
    estimator.note_sender_report(9, 1_000);
    // DLSR 1.5 с при общем интервале 0.1 с — часы разъехались.
    let rtt = estimator.on_report_block(&block(9, 98_304), 1_100);
    assert_eq!(rtt, Some(0.0));
}

#[test]
fn window_keeps_capacity_and_reports_latest() {
    let mut window = StatsWindow::new(3);
    for index in 0..4 {
        window.push(sample(
            index * 100,
            Some(index as f64),
            index as i64,
            index as f64 / 10.0,
        ));
    }
    assert_eq!(window.len(), 3);
    assert!(!window.is_empty());
    assert_eq!(window.latest().map(|item| item.at_ms), Some(300));
}

#[test]
fn window_average_and_max_ignore_missing_rtt() {
    let mut window = StatsWindow::new(4);
    window.push(sample(0, Some(10.0), 0, 1.0));
    window.push(sample(1, None, 1, 3.0));
    window.push(sample(2, Some(30.0), 2, 2.0));
    assert!((window.average_rtt_ms().unwrap() - 20.0).abs() < 1e-9);
    assert!((window.max_jitter_ms().unwrap() - 3.0).abs() < 1e-9);
}

#[test]
fn window_loss_delta_uses_first_and_last_sample() {
    let mut window = StatsWindow::new(8);
    window.push(sample(0, None, 5, 0.0));
    window.push(sample(1, None, 9, 0.0));
    assert_eq!(window.loss_delta(), Some(4));
}

#[test]
fn window_with_single_sample_has_no_loss_delta_or_rtt() {
    let mut window = StatsWindow::new(8);
    assert_eq!(window.latest(), None);
    assert_eq!(window.average_rtt_ms(), None);
    assert_eq!(window.max_jitter_ms(), None);
    assert_eq!(window.loss_delta(), None);
    window.push(sample(0, None, 5, 0.0));
    assert_eq!(window.loss_delta(), None);
    assert_eq!(window.average_rtt_ms(), None);
}

#[test]
fn zero_capacity_window_is_treated_as_one() {
    let mut window = StatsWindow::new(0);
    window.push(sample(0, Some(1.0), 0, 0.0));
    window.push(sample(1, Some(2.0), 0, 0.0));
    assert_eq!(window.len(), 1);
    assert_eq!(window.latest().map(|item| item.at_ms), Some(1));
}

#[test]
fn window_loss_delta_can_be_negative_after_stream_restart() {
    let mut window = StatsWindow::new(4);
    window.push(sample(0, None, 100, 0.0));
    window.push(sample(1, None, 2, 0.0));
    assert_eq!(window.loss_delta(), Some(-98));
}

fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

#[test]
fn srtcp_sender_report_60_byte_wire_layout_matches_re_oracle() {
    // Эталон из strace-media-s3.jsonl (out packet #0):
    // 80c80006000023f88abb34e79b75ca2c3872445b001d6ca27b9c764920730f08afde72c4e657d80674ed13223fc1383a3a28eed1503bb8fb08000080
    // [8B RTCP header + SSRC][20B Sender Info][12B extra][16B AEAD tag][4B LE SRTCP index with E-bit]
    let oracle_hex = "80c80006000023f88abb34e79b75ca2c3872445b001d6ca27b9c764920730f08afde72c4e657d80674ed13223fc1383a3a28eed1503bb8fb08000080";
    let oracle_bytes = decode_hex(oracle_hex);
    assert_eq!(oracle_bytes.len(), 60);

    let ssrc = 0x0000_23f8;
    let info = SenderInfo {
        ntp_timestamp: 0x8abb_34e7_9b75_ca2c,
        rtp_timestamp: 0x3872_445b,
        packet_count: 0x001d_6ca2,
        octet_count: 0x7b9c_7649,
    };
    let mut extra = [0u8; 12];
    extra.copy_from_slice(&oracle_bytes[28..40]);
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&oracle_bytes[40..56]);
    let srtcp_index = 8u32; // trailer 0x80000008 in LE is [0x08, 0x00, 0x00, 0x80]

    let wire = build_srtcp_sender_report(ssrc, &info, Some(&extra), tag, srtcp_index);
    assert_eq!(wire.len(), 60);
    assert_eq!(wire.as_slice(), oracle_bytes.as_slice());

    // Проверяем разбор сгенерированного пакета:
    let compound = parse_compound(&wire).expect("built 60B SR must parse");
    assert_eq!(compound.packets.len(), 1);
    assert_eq!(compound.trailer_len, 32);
    let RtcpPacket::SenderReport {
        sender_ssrc: parsed_ssrc,
        info: parsed_info,
        reports,
    } = &compound.packets[0]
    else {
        panic!("expected SenderReport");
    };
    assert_eq!(*parsed_ssrc, ssrc);
    assert_eq!(*parsed_info, info);
    assert!(reports.is_empty());
}

#[test]
fn clear_sender_report_28_byte_builder_and_compound_parse() {
    let ssrc: u32 = 0x1234_5678;
    let info = SenderInfo {
        ntp_timestamp: 0x1122_3344_5566_7788,
        rtp_timestamp: 96000,
        packet_count: 100,
        octet_count: 16000,
    };

    let clear_bytes = build_sender_report(ssrc, &info);
    assert_eq!(clear_bytes.len(), 28);
    assert_eq!(clear_bytes[0], 0x80); // V=2, P=0, RC=0
    assert_eq!(clear_bytes[1], RTCP_SR); // 200
    assert_eq!(&clear_bytes[2..4], &6u16.to_be_bytes()); // len words = 6
    assert_eq!(&clear_bytes[4..8], &ssrc.to_be_bytes());

    let compound = parse_compound(&clear_bytes).expect("clear SR must parse");
    assert_eq!(compound.trailer_len, 0);
    assert_eq!(compound.packets.len(), 1);
    let RtcpPacket::SenderReport {
        sender_ssrc,
        info: parsed_info,
        reports,
    } = &compound.packets[0]
    else {
        panic!("expected SenderReport");
    };
    assert_eq!(*sender_ssrc, ssrc);
    assert_eq!(*parsed_info, info);
    assert!(reports.is_empty());
}

#[test]
fn rtcp_sender_scheduler_5000ms_interval() {
    assert_eq!(RTCP_SR_INTERVAL_MS, 5_000);
    let mut scheduler = RtcpSenderScheduler::default();
    assert_eq!(scheduler.interval_ms(), 5_000);
    assert_eq!(scheduler.last_sent_at_ms(), None);
    assert!(scheduler.is_due(0));
    assert_eq!(scheduler.time_until_due_ms(0), 0);

    // Отправили SR в t = 1_000 мс
    scheduler.note_sent(1_000);
    assert_eq!(scheduler.last_sent_at_ms(), Some(1_000));
    assert!(!scheduler.is_due(1_000));
    assert_eq!(scheduler.time_until_due_ms(1_000), 5_000);

    // t = 3_500 мс (прошло 2_500 мс, осталось 2_500 мс)
    assert!(!scheduler.is_due(3_500));
    assert_eq!(scheduler.time_until_due_ms(3_500), 2_500);

    // t = 5_999 мс (прошло 4_999 мс, остался 1 мс)
    assert!(!scheduler.is_due(5_999));
    assert_eq!(scheduler.time_until_due_ms(5_999), 1);

    // t = 6_000 мс (интервал 5_000 мс истёк)
    assert!(scheduler.is_due(6_000));
    assert_eq!(scheduler.time_until_due_ms(6_000), 0);

    // t = 8_000 мс (овердью)
    assert!(scheduler.is_due(8_000));
    assert_eq!(scheduler.time_until_due_ms(8_000), 0);

    // Сброс
    scheduler.reset();
    assert_eq!(scheduler.last_sent_at_ms(), None);
    assert!(scheduler.is_due(8_000));
}

#[test]
fn server_receiver_report_52_byte_wire_layout_parsing_and_roundtrip() {
    // Эталон из strace-media-s3.jsonl (in packet #0):
    // 81c90007000023f8b58acbc1675a4d35a0ac12ccf8f151bc0f002888166d180d33c307bba4d3e9d3fc94880e9397111d00000000
    // [8B RTCP header + SSRC][24B Report Block][16B AEAD tag][4B counter without E-bit]
    let oracle_hex = "81c90007000023f8b58acbc1675a4d35a0ac12ccf8f151bc0f002888166d180d33c307bba4d3e9d3fc94880e9397111d00000000";
    let oracle_bytes = decode_hex(oracle_hex);
    assert_eq!(oracle_bytes.len(), 52);

    assert!(is_server_receiver_report(&oracle_bytes));

    let (sender_ssrc, report, tag, counter) =
        parse_server_receiver_report(&oracle_bytes).expect("52B server RR must parse");
    assert_eq!(sender_ssrc, 0x0000_23f8);
    assert_eq!(report.ssrc, 0xb58a_cbc1);
    assert_eq!(report.fraction_lost, 0x67);
    assert_eq!(report.cumulative_lost, 0x5a4d35);
    assert_eq!(report.extended_highest_seq, 0xa0ac_12cc);
    assert_eq!(report.jitter, 0xf8f1_51bc);
    assert_eq!(report.last_sr, 0x0f00_2888);
    assert_eq!(report.delay_since_last_sr, 0x166d_180d);
    assert_eq!(&tag, &decode_hex("33c307bba4d3e9d3fc94880e9397111d")[..]);
    assert_eq!(counter, 0);

    // Проверяем обратную сборку build_srtcp_receiver_report:
    let mut report_bytes = [0u8; 24];
    report_bytes.copy_from_slice(&oracle_bytes[8..32]);
    let rebuilt = build_srtcp_receiver_report(sender_ssrc, &report_bytes, tag, counter);
    assert_eq!(rebuilt.as_slice(), oracle_bytes.as_slice());

    // Проверяем сборку открытого 32-байтового RR:
    let clear_rr = build_receiver_report(sender_ssrc, &report);
    assert_eq!(clear_rr.len(), 32);
    assert_eq!(clear_rr[0], 0x81); // V=2, RC=1
    assert_eq!(clear_rr[1], RTCP_RR); // 201
    assert_eq!(&clear_rr[2..4], &7u16.to_be_bytes()); // len words = 7
    assert_eq!(&clear_rr[4..8], &sender_ssrc.to_be_bytes());
    assert_eq!(&clear_rr[8..32], &oracle_bytes[8..32]);
}

#[test]
fn stats_sample_from_report_block_calculates_fraction_cumulative_and_jitter_ms() {
    let block = ReportBlock {
        ssrc: 9208,
        fraction_lost: 64, // 64 / 256 = 0.25
        cumulative_lost: -5,
        extended_highest_seq: 1000,
        jitter: 240, // 240 units / 48 kHz = 5.0 ms
        last_sr: 0x1234_5678,
        delay_since_last_sr: 0,
    };

    let sample = StatsSample::from_report_block(&block, 5_000, Some(42.5));
    assert_eq!(sample.at_ms, 5_000);
    assert_eq!(sample.rtt_ms, Some(42.5));
    assert!((sample.fraction_lost - 0.25).abs() < 1e-9);
    assert_eq!(sample.cumulative_lost, -5);
    assert!((sample.jitter_ms - 5.0).abs() < 1e-9);
}

#[test]
fn session_stats_records_loss_jitter_and_rtt_on_report_block() {
    let mut stats = SessionStats::new(9208, 10);
    assert_eq!(stats.local_ssrc(), 9208);
    assert_eq!(stats.sr_sent_count(), 0);
    assert_eq!(stats.rr_received_count(), 0);
    assert_eq!(stats.undecrypted_rr_count(), 0);

    // Локально отправляем SR в t = 1_000 мс с NTP timestamp
    let ntp = 0x8abb_34e7_9b75_ca2c;
    stats.note_sr_sent(ntp, 1_000);
    assert_eq!(stats.sr_sent_count(), 1);
    assert_eq!(stats.sr_scheduler().last_sent_at_ms(), Some(1_000));

    // Сервер присылает ReportBlock в t = 1_100 мс, эхо lsr = 0x8abb_34e7, dlsr = 0 (RTT = 100 мс)
    let block = ReportBlock {
        ssrc: 9208,
        fraction_lost: 32, // 32 / 256 = 0.125 (12.5% потерь)
        cumulative_lost: 15,
        extended_highest_seq: 2000,
        jitter: 96, // 96 / 48 = 2.0 мс джиттера
        last_sr: ntp_middle_32(ntp),
        delay_since_last_sr: 0,
    };

    stats.on_report_block(&block, 1_100);
    assert_eq!(stats.rr_received_count(), 1);
    assert_eq!(stats.undecrypted_rr_count(), 0);
    assert!((stats.latest_fraction_lost().unwrap() - 0.125).abs() < 1e-9);
    assert_eq!(stats.latest_cumulative_lost(), Some(15));
    assert!((stats.latest_jitter_ms().unwrap() - 2.0).abs() < 1e-9);
    assert!((stats.latest_rtt_ms().unwrap() - 100.0).abs() < 1e-9);
    assert_eq!(stats.window().len(), 1);
    assert!((stats.average_rtt_ms().unwrap() - 100.0).abs() < 1e-9);
    assert!((stats.max_jitter_ms().unwrap() - 2.0).abs() < 1e-9);
}

#[test]
fn session_stats_safe_counting_without_key() {
    let mut stats = SessionStats::new(9208, 10);

    let oracle_hex = "81c90007000023f8b58acbc1675a4d35a0ac12ccf8f151bc0f002888166d180d33c307bba4d3e9d3fc94880e9397111d00000000";
    let oracle_bytes = decode_hex(oracle_hex);

    // При отсутствии ключа (has_key = false): счётчик пакетов инкрементируется,
    // зашифрованное тело не трактуется как статистика, падений нет.
    stats.on_incoming_rr_packet(&oracle_bytes, 1_000, false);
    assert_eq!(stats.rr_received_count(), 1);
    assert_eq!(stats.undecrypted_rr_count(), 1);
    assert_eq!(stats.server_ssrc(), Some(0x0000_23f8));
    assert_eq!(stats.latest_fraction_lost(), None);
    assert_eq!(stats.latest_cumulative_lost(), None);
    assert_eq!(stats.latest_jitter_ms(), None);
    assert_eq!(stats.latest_rtt_ms(), None);
    assert!(stats.window().is_empty());

    // Пришёл второй зашифрованный RR
    stats.on_incoming_rr_packet(&oracle_bytes, 2_000, false);
    assert_eq!(stats.rr_received_count(), 2);
    assert_eq!(stats.undecrypted_rr_count(), 2);
    assert!(stats.window().is_empty());
}

#[test]
fn session_stats_with_key_updates_all_metrics() {
    let mut stats = SessionStats::new(9208, 10);
    let ntp = 0x0f00_2888_0000_0000u64; // middle 32 = 0x0f00_2888
    stats.note_sr_sent(ntp, 1_000);

    let oracle_hex = "81c90007000023f8b58acbc1675a4d35a0ac12ccf8f151bc0f002888166d180d33c307bba4d3e9d3fc94880e9397111d00000000";
    let oracle_bytes = decode_hex(oracle_hex);

    // При наличии ключа (has_key = true): расшифрованное/распарсенное тело обновляет метрики
    stats.on_incoming_rr_packet(&oracle_bytes, 1_500, true);
    assert_eq!(stats.rr_received_count(), 1);
    assert_eq!(stats.undecrypted_rr_count(), 0);
    assert_eq!(stats.server_ssrc(), Some(0x0000_23f8));
    assert!(stats.latest_fraction_lost().is_some());
    assert_eq!(stats.latest_cumulative_lost(), Some(0x5a4d35));
    assert!(stats.latest_jitter_ms().is_some());
    assert_eq!(stats.window().len(), 1);
}
