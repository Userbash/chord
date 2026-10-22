//! Контракт RTT-оценки и окна статистики (P4-07, B07/RW-26).
//!
//! Формулы — RFC 3550 §6.4.1/§6.4.2: RTT = (arrival − LSR_time) − DLSR,
//! DLSR/LSR — в единицах 1/65536 с. Живых открытых SR/RR-пар в capture нет
//! (`waves/RW-26/research/RE-VOICE-STATS.md` §2), поэтому векторы синтетические.

use discord_voice::rtcp::ReportBlock;
use discord_voice::stats::{RttEstimator, StatsSample, StatsWindow, ntp_middle_32};

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
