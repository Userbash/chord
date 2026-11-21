//! Audio Pipeline & SPSC Ring Buffer contract tests (`audio_pipeline_contract.rs`).
//!
//! Verifies:
//! 1. Bounded lock-free ring buffer behavior and FIFO order preservation.
//! 2. Overflow policy (`DropOldest` vs `DropNewest`) and drop counter observability.
//! 3. Voice Activity Detection (VAD): energy dBFS calculation, speech thresholding, and hangover window.
//! 4. Transparent volume scaling and microphone mute enforcement.
//! 5. Multi-threaded Producer-Consumer throughput across independent OS threads without deadlocks.
//! 6. Native device enumeration via CPAL host.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use audio_spike::pipeline::{AudioPipelineBridge, DeviceDirection};
use audio_spike::ring_buffer::{
    audio_ring_buffer, AudioFrame, OverflowPolicy, SAMPLES_PER_20MS_MONO,
};
use audio_spike::vad::{VadConfig, VadDetector, VadState};

/// Generates a synthetic 20 ms sine wave audio frame at the specified frequency and amplitude.
fn make_sine_frame(freq_hz: f32, amplitude: f32, sequence: u64) -> AudioFrame {
    let sample_rate = 48_000u32;
    let num_samples = SAMPLES_PER_20MS_MONO;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * amplitude;
        samples.push(sample.clamp(-1.0, 1.0));
    }

    AudioFrame {
        samples,
        channels: 1,
        sample_rate,
        sequence,
    }
}

#[test]
fn test_ring_buffer_fifo_order_and_capacity() {
    let (producer, consumer) = audio_ring_buffer(4, OverflowPolicy::DropOldest);

    assert_eq!(producer.len(), 0);
    assert_eq!(consumer.len(), 0);

    // Push 3 frames
    for seq in 1..=3 {
        let frame = make_sine_frame(440.0, 0.5, seq);
        assert!(producer.push(frame));
    }

    assert_eq!(producer.len(), 3);
    assert_eq!(consumer.len(), 3);

    // Pop and verify FIFO order
    for seq in 1..=3 {
        let frame = consumer.pop().expect("frame should be available");
        assert_eq!(frame.sequence, seq);
    }

    assert!(consumer.pop().is_none());
}

#[test]
fn test_ring_buffer_overflow_drop_oldest() {
    let capacity = 3;
    let (producer, consumer) = audio_ring_buffer(capacity, OverflowPolicy::DropOldest);

    // Fill buffer to capacity: seq 1, 2, 3
    for seq in 1..=3 {
        assert!(producer.push(make_sine_frame(440.0, 0.5, seq)));
    }
    assert!(producer.is_full());

    // Push frame 4 (should evict oldest: seq 1)
    let pushed_directly = producer.push(make_sine_frame(440.0, 0.5, 4));
    assert!(!pushed_directly, "overflow should return false");

    // Push frame 5 (should evict oldest: seq 2)
    producer.push(make_sine_frame(440.0, 0.5, 5));

    // Verify remaining frames are 3, 4, 5
    let remaining_seqs: Vec<u64> = (0..capacity)
        .filter_map(|_| consumer.pop().map(|f| f.sequence))
        .collect();

    assert_eq!(remaining_seqs, vec![3, 4, 5]);

    let (pushed, popped, dropped) = consumer.metrics_snapshot();
    assert_eq!(pushed, 5);
    assert_eq!(popped, 3);
    assert_eq!(dropped, 2);
}

#[test]
fn test_vad_energy_dbfs_and_speech_detection() {
    let mut vad = VadDetector::new(VadConfig {
        threshold_dbfs: -30.0,
        hangover: Duration::from_millis(50),
    });

    let now = Instant::now();

    // 1. Pure silence frame (RMS = 0.0, dBFS = -100)
    let silence = AudioFrame::silence(1, 48_000);
    assert!(silence.dbfs() <= -90.0);
    let state = vad.process_frame(&silence, now);
    assert_eq!(state, VadState::Silence);

    // 2. High-energy sine wave frame (amplitude 0.8 => RMS ~0.565 => dBFS ~ -4.9 dBFS)
    let speech = make_sine_frame(1000.0, 0.8, 1);
    assert!(speech.dbfs() > -10.0);
    let state = vad.process_frame(&speech, now);
    assert_eq!(state, VadState::Speech);

    // 3. Low whisper (amplitude 0.01 => RMS ~0.007 => dBFS ~ -43 dBFS, below threshold -30)
    // But within hangover window (now + 20ms <= now + 50ms), should remain Speech!
    let whisper = make_sine_frame(500.0, 0.01, 2);
    let state = vad.process_frame(&whisper, now + Duration::from_millis(20));
    assert_eq!(state, VadState::Speech, "hangover should keep speech active");

    // 4. After hangover window expires (now + 60ms > now + 50ms), should transition to Silence
    let state = vad.process_frame(&whisper, now + Duration::from_millis(60));
    assert_eq!(state, VadState::Silence);
}

#[test]
fn test_audio_pipeline_bridge_mute_and_volume() {
    let mut bridge = AudioPipelineBridge::new(8, -45.0);

    // Ingest frame with volume 100% (amplitude 0.5)
    let frame = make_sine_frame(440.0, 0.5, 1);
    assert!(bridge.ingest_frame(frame));

    let now = Instant::now();
    let (processed, is_speech) = bridge
        .process_next_frame(now)
        .expect("frame should be processed");
    assert!(is_speech);
    let peak_orig = processed
        .samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!((peak_orig - 0.5).abs() < 0.05);

    // Test mute: all samples become 0.0
    bridge.set_muted(true);
    let frame_muted = make_sine_frame(440.0, 0.5, 2);
    bridge.ingest_frame(frame_muted);

    let (processed_muted, _) = bridge
        .process_next_frame(now)
        .expect("muted frame should be processed");
    assert!(processed_muted.samples.iter().all(|&s| s == 0.0));

    // Test volume scaling 50% (amplitude 0.5 * 0.5 = 0.25)
    bridge.set_muted(false);
    bridge.set_volume(50);
    let frame_scaled = make_sine_frame(440.0, 0.5, 3);
    bridge.ingest_frame(frame_scaled);

    let (processed_scaled, _) = bridge
        .process_next_frame(now)
        .expect("scaled frame should be processed");
    let peak_scaled = processed_scaled
        .samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max);
    assert!((peak_scaled - 0.25).abs() < 0.05);
}

#[test]
fn test_multi_threaded_audio_producer_consumer_throughput() {
    let (producer, consumer) = audio_ring_buffer(32, OverflowPolicy::DropOldest);
    let stop_signal = Arc::new(AtomicBool::new(false));

    let total_frames = 200u64;

    // Simulate high-priority audio callback thread (produces 1 frame every 1ms for speed)
    let prod_stop = Arc::clone(&stop_signal);
    let producer_handle = thread::spawn(move || {
        for seq in 1..=total_frames {
            if prod_stop.load(Ordering::Relaxed) {
                break;
            }
            let frame = make_sine_frame(440.0, 0.3, seq);
            producer.push(frame);
            thread::sleep(Duration::from_micros(200));
        }
    });

    // Simulate audio worker consumer thread
    let cons_stop = Arc::clone(&stop_signal);
    let consumer_handle = thread::spawn(move || {
        let mut received = 0u64;
        let mut last_seq = 0u64;

        let start = Instant::now();
        while received < total_frames && start.elapsed() < Duration::from_secs(3) {
            if let Some(frame) = consumer.pop() {
                assert!(frame.sequence > last_seq, "sequence must strictly advance");
                last_seq = frame.sequence;
                received += 1;
            } else {
                thread::yield_now();
            }
        }
        cons_stop.store(true, Ordering::Relaxed);
        received
    });

    producer_handle.join().expect("producer thread joined cleanly");
    let received_count = consumer_handle.join().expect("consumer thread joined cleanly");

    assert!(
        received_count >= 190,
        "expected at least 190 of 200 frames to arrive (received: {received_count})"
    );
}

#[test]
fn test_device_enumeration_contract() {
    let input_devices = AudioPipelineBridge::enumerate_devices(DeviceDirection::Input);
    let output_devices = AudioPipelineBridge::enumerate_devices(DeviceDirection::Output);

    // On Linux CI/containers without ALSA audio cards, lists may be empty or contain dummy/pulse devices
    // The contract requires non-crashing safe execution and well-formed metadata.
    for dev in input_devices.iter().chain(output_devices.iter()) {
        assert!(!dev.name.is_empty());
        assert!(dev.max_channels >= 1);
        assert!(dev.default_sample_rate >= 8000);
    }
}
