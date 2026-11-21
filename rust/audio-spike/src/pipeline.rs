//! Audio pipeline transparent bridge (`AudioPipelineBridge`).
//!
//! Connects native audio device enumeration via [`cpal`], non-blocking audio capture,
//! SPSC ring buffering, and voice processing.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait};

use crate::ring_buffer::{
    audio_ring_buffer, AudioConsumer, AudioFrame, AudioProducer, OverflowPolicy,
    DEFAULT_RING_BUFFER_CAPACITY_FRAMES,
};
use crate::vad::{VadConfig, VadDetector, VadState};

/// Discovered native audio device info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub max_channels: u16,
    pub default_sample_rate: u32,
}

/// Direction of audio flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceDirection {
    Input,
    Output,
}

/// Statistics and telemetry emitted by [`AudioPipelineBridge`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioPipelineTelemetry {
    pub captured_frames: u64,
    pub processed_frames: u64,
    pub dropped_frames: u64,
    pub current_dbfs: f32,
    pub is_speaking: bool,
    pub volume_pct: u32,
}

/// Transparent audio engine bridge managing capture, buffering, and VAD processing.
pub struct AudioPipelineBridge {
    producer: AudioProducer,
    consumer: AudioConsumer,
    vad: VadDetector,
    is_muted: Arc<AtomicBool>,
    is_deafened: Arc<AtomicBool>,
    volume_pct: Arc<AtomicU32>,
    captured_count: u64,
    processed_count: u64,
    last_dbfs: f32,
}

impl AudioPipelineBridge {
    /// Creates a new audio pipeline bridge with specified buffer capacity and VAD threshold.
    #[must_use]
    pub fn new(capacity_frames: usize, vad_threshold_dbfs: f32) -> Self {
        let (producer, consumer) =
            audio_ring_buffer(capacity_frames, OverflowPolicy::DropOldest);

        let vad_config = VadConfig {
            threshold_dbfs: vad_threshold_dbfs,
            ..Default::default()
        };

        Self {
            producer,
            consumer,
            vad: VadDetector::new(vad_config),
            is_muted: Arc::new(AtomicBool::new(false)),
            is_deafened: Arc::new(AtomicBool::new(false)),
            volume_pct: Arc::new(AtomicU32::new(100)),
            captured_count: 0,
            processed_count: 0,
            last_dbfs: -100.0,
        }
    }

    /// Enumerate available native host devices for input or output via `cpal`.
    pub fn enumerate_devices(direction: DeviceDirection) -> Vec<AudioDeviceInfo> {
        let host = cpal::default_host();
        let default_device_name = match direction {
            DeviceDirection::Input => host.default_input_device().and_then(|d| d.name().ok()),
            DeviceDirection::Output => host.default_output_device().and_then(|d| d.name().ok()),
        };

        let devices_iter = match direction {
            DeviceDirection::Input => host.input_devices(),
            DeviceDirection::Output => host.output_devices(),
        };

        let Ok(devices) = devices_iter else {
            return Vec::new();
        };

        let mut result = Vec::new();
        for dev in devices {
            if let Ok(name) = dev.name() {
                let is_default = default_device_name.as_deref() == Some(&name);
                let (max_channels, default_sample_rate) = match dev.default_input_config() {
                    Ok(cfg) => (cfg.channels(), cfg.sample_rate().0),
                    Err(_) => (2, 48_000),
                };

                result.push(AudioDeviceInfo {
                    name,
                    is_default,
                    max_channels,
                    default_sample_rate,
                });
            }
        }
        result
    }

    /// Returns a clone of the producer handle to be moved into the CPAL audio callback.
    #[must_use]
    pub fn producer(&self) -> AudioProducer {
        self.producer.clone()
    }

    /// Ingests a raw audio frame into the pipeline (used by CPAL callback or synthetic loopback).
    pub fn ingest_frame(&mut self, mut frame: AudioFrame) -> bool {
        self.captured_count += 1;

        // Apply mute
        if self.is_muted.load(Ordering::Relaxed) {
            frame.samples.fill(0.0);
        } else {
            // Apply volume scale
            let vol = self.volume_pct.load(Ordering::Relaxed);
            if vol != 100 {
                let scale = vol as f32 / 100.0;
                for sample in &mut frame.samples {
                    *sample = (*sample * scale).clamp(-1.0, 1.0);
                }
            }
        }

        self.producer.push(frame)
    }

    /// Processes the next queued audio frame in the audio worker loop.
    /// Returns `Some((processed_frame, is_speech))` or `None` if the queue is empty.
    pub fn process_next_frame(&mut self, now: Instant) -> Option<(AudioFrame, bool)> {
        let frame = self.consumer.pop()?;
        self.processed_count += 1;

        let level = frame.dbfs();
        self.last_dbfs = level;

        let vad_state = self.vad.process_frame(&frame, now);
        let is_speech = vad_state == VadState::Speech;

        Some((frame, is_speech))
    }

    /// Sets microphone mute state.
    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Relaxed);
    }

    /// Sets deafen state.
    pub fn set_deafened(&self, deafened: bool) {
        self.is_deafened.store(deafened, Ordering::Relaxed);
    }

    /// Sets playback/input volume in percentage (`0%` to `200%`).
    pub fn set_volume(&self, volume_pct: u32) {
        self.volume_pct.store(volume_pct.clamp(0, 200), Ordering::Relaxed);
    }

    /// Collects an observable snapshot of pipeline telemetry and metrics.
    #[must_use]
    pub fn telemetry(&self) -> AudioPipelineTelemetry {
        let (pushed, popped, dropped) = self.consumer.metrics_snapshot();
        AudioPipelineTelemetry {
            captured_frames: pushed,
            processed_frames: popped,
            dropped_frames: dropped,
            current_dbfs: self.last_dbfs,
            is_speaking: self.vad.current_state() == VadState::Speech,
            volume_pct: self.volume_pct.load(Ordering::Relaxed),
        }
    }
}

impl Default for AudioPipelineBridge {
    fn default() -> Self {
        Self::new(DEFAULT_RING_BUFFER_CAPACITY_FRAMES, crate::vad::DEFAULT_VAD_THRESHOLD_DBFS)
    }
}
