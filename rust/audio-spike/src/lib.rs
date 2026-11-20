//! Audio Engine spike prototype (`audio_spike`) (Roadmap Stage 8, `LIB-SIMPLIFY-04`).
//!
//! Implements a low-latency, real-time audio pipeline:
//! `cpal mic callback -> Lock-Free SPSC Ring Buffer -> Audio Worker -> Opus/PCM -> Network UDP`
//!
//! Enforces:
//! - `#![forbid(unsafe_code)]` across the entire crate
//! - Zero memory allocations in the audio callback path
//! - Bounded latency (< 5ms) and deterministic drop on buffer overflow
//! - Standard Discord 48 kHz / 20ms audio frames (960 samples per channel)

#![forbid(unsafe_code)]

pub mod ring_buffer;
pub mod pipeline;
pub mod vad;

#[cfg(test)]
mod tests {
    use super::ring_buffer::AudioFrame;

    #[test]
    fn test_silence_frame_properties() {
        let frame = AudioFrame::silence(2, 48_000);
        assert_eq!(frame.channels, 2);
        assert_eq!(frame.sample_rate, 48_000);
        assert_eq!(frame.samples.len(), 1920); // 20ms of stereo @ 48kHz
        assert_eq!(frame.rms(), 0.0);
        assert_eq!(frame.dbfs(), -100.0);
    }
}
