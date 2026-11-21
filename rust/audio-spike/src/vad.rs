//! Voice Activity Detection (VAD) engine.
//!
//! Evaluates incoming audio frames against an energy threshold (in dBFS)
//! with a hangover window to avoid stuttering during inter-word pauses.

#![forbid(unsafe_code)]

use std::time::{Duration, Instant};
use crate::ring_buffer::AudioFrame;

/// Default VAD threshold (-45 dBFS).
pub const DEFAULT_VAD_THRESHOLD_DBFS: f32 = -45.0;
/// Default hangover duration (200 ms = 10 frames of 20ms).
pub const DEFAULT_HANGOVER_DURATION_MS: u64 = 200;

/// State of voice activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadState {
    /// Speech detected (transmitting audio).
    Speech,
    /// Silence / below threshold (comfort noise or silence packet).
    Silence,
}

/// Voice Activity Detector configuration.
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    /// Activation threshold in dBFS (e.g. -45.0).
    pub threshold_dbfs: f32,
    /// Hangover duration to keep transmitting after speech drops below threshold.
    pub hangover: Duration,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold_dbfs: DEFAULT_VAD_THRESHOLD_DBFS,
            hangover: Duration::from_millis(DEFAULT_HANGOVER_DURATION_MS),
        }
    }
}

/// Real-time Voice Activity Detector.
pub struct VadDetector {
    config: VadConfig,
    state: VadState,
    last_speech_time: Option<Instant>,
}

impl VadDetector {
    /// Creates a new VAD detector with the specified configuration.
    #[must_use]
    pub fn new(config: VadConfig) -> Self {
        Self {
            config,
            state: VadState::Silence,
            last_speech_time: None,
        }
    }

    /// Evaluates an audio frame, updates internal hangover state, and returns current [`VadState`].
    pub fn process_frame(&mut self, frame: &AudioFrame, now: Instant) -> VadState {
        let level = frame.dbfs();

        if level >= self.config.threshold_dbfs {
            // Immediate speech detection
            self.state = VadState::Speech;
            self.last_speech_time = Some(now);
        } else if let Some(last_speech) = self.last_speech_time {
            // Check hangover window
            if now.saturating_duration_since(last_speech) <= self.config.hangover {
                self.state = VadState::Speech;
            } else {
                self.state = VadState::Silence;
                self.last_speech_time = None;
            }
        } else {
            self.state = VadState::Silence;
        }

        self.state
    }

    /// Returns the current speech state.
    #[must_use]
    pub const fn current_state(&self) -> VadState {
        self.state
    }

    /// Updates the energy threshold.
    pub fn set_threshold(&mut self, threshold_dbfs: f32) {
        self.config.threshold_dbfs = threshold_dbfs.clamp(-90.0, 0.0);
    }
}
