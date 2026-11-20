//! Lock-free real-time audio ring buffer (`RealtimeAudioRingBuffer`).
//!
//! Provides bounded, allocation-free transfer of audio frames between the high-priority
//! CPAL stream callback and the dedicated Audio Processing Worker.
//!
//! Features:
//! - Guaranteed zero allocations after initialization (`#![forbid(unsafe_code)]`).
//! - Lock-free multi-thread safety using [`crossbeam_queue::ArrayQueue`].
//! - Explicit overflow policy: `DropOldest` (for lowest voice latency) or `DropNewest`.
//! - Performance metrics: frames pushed, frames popped, overflow drop count.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use crossbeam_queue::ArrayQueue;

/// Standard Discord voice frame size: 20 ms @ 48 kHz stereo (960 samples per channel = 1920 samples).
pub const SAMPLES_PER_20MS_STEREO: usize = 1920;
/// Standard Discord voice frame size: 20 ms @ 48 kHz mono (960 samples).
pub const SAMPLES_PER_20MS_MONO: usize = 960;

/// Default ring buffer capacity in 20 ms frames (16 frames = 320 ms of audio headroom).
pub const DEFAULT_RING_BUFFER_CAPACITY_FRAMES: usize = 16;

/// A fixed-size PCM audio frame for 20 ms of 48 kHz audio.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioFrame {
    /// PCM samples normalized to `[-1.0, 1.0]`.
    pub samples: Vec<f32>,
    /// Number of channels (1 for mono, 2 for stereo).
    pub channels: u16,
    /// Sample rate in Hz (typically 48,000).
    pub sample_rate: u32,
    /// Monotonic sequence number for jitter tracking.
    pub sequence: u64,
}

impl AudioFrame {
    /// Creates a new silence frame with pre-allocated sample vector.
    #[must_use]
    pub fn silence(channels: u16, sample_rate: u32) -> Self {
        let frame_size = (sample_rate as usize * 20 / 1000) * channels as usize;
        Self {
            samples: vec![0.0; frame_size],
            channels,
            sample_rate,
            sequence: 0,
        }
    }

    /// Calculates Root Mean Square (RMS) energy of the frame.
    #[must_use]
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = self.samples.iter().map(|s| s * s).sum();
        (sum_sq / self.samples.len() as f32).sqrt()
    }

    /// Calculates energy in decibels relative to full scale (dBFS).
    /// Returns a value typically between `-100.0` (silence) and `0.0` (peak).
    #[must_use]
    pub fn dbfs(&self) -> f32 {
        let rms = self.rms();
        if rms <= 1e-5 {
            -100.0
        } else {
            (20.0 * rms.log10()).clamp(-100.0, 0.0)
        }
    }
}

/// Strategy for handling audio frame buffer overflow when the worker lags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    /// Drops the oldest queued frame to ensure real-time voice latency remains minimal.
    DropOldest,
    /// Drops incoming frame, preserving current buffered playback sequence.
    DropNewest,
}

/// Metrics tracking real-time audio buffer throughput and overflows.
#[derive(Debug, Default)]
pub struct BufferMetrics {
    pub pushed_frames: AtomicU64,
    pub popped_frames: AtomicU64,
    pub dropped_frames: AtomicU64,
}

/// Shared internal state of the audio ring buffer.
struct RingBufferInner {
    queue: ArrayQueue<AudioFrame>,
    policy: OverflowPolicy,
    metrics: BufferMetrics,
}

/// Producer handle for sending audio frames into the ring buffer from the CPAL callback.
#[derive(Clone)]
pub struct AudioProducer {
    inner: Arc<RingBufferInner>,
}

impl AudioProducer {
    /// Pushes an audio frame into the buffer without blocking or allocating.
    /// Returns `true` if pushed directly, `false` if an overflow occurred and a frame was dropped.
    pub fn push(&self, frame: AudioFrame) -> bool {
        self.inner.metrics.pushed_frames.fetch_add(1, Ordering::Relaxed);

        match self.inner.queue.push(frame) {
            Ok(()) => true,
            Err(rejected) => {
                // Buffer is full: handle overflow according to policy
                self.inner.metrics.dropped_frames.fetch_add(1, Ordering::Relaxed);
                match self.inner.policy {
                    OverflowPolicy::DropOldest => {
                        let _ = self.inner.queue.pop();
                        let _ = self.inner.queue.push(rejected);
                    }
                    OverflowPolicy::DropNewest => {}
                }
                false
            }
        }
    }

    /// Returns the number of frames currently in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.queue.len()
    }

    /// Checks if the buffer is full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.inner.queue.is_full()
    }
}

/// Consumer handle for receiving audio frames in the Audio Worker thread.
#[derive(Clone)]
pub struct AudioConsumer {
    inner: Arc<RingBufferInner>,
}

impl AudioConsumer {
    /// Pops the next audio frame from the ring buffer.
    /// Returns `None` if the queue is empty.
    pub fn pop(&self) -> Option<AudioFrame> {
        let frame = self.inner.queue.pop();
        if frame.is_some() {
            self.inner.metrics.popped_frames.fetch_add(1, Ordering::Relaxed);
        }
        frame
    }

    /// Returns the number of frames currently waiting in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.queue.len()
    }

    /// Checks if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.queue.is_empty()
    }

    /// Returns a snapshot of buffer performance counters.
    #[must_use]
    pub fn metrics_snapshot(&self) -> (u64, u64, u64) {
        (
            self.inner.metrics.pushed_frames.load(Ordering::Relaxed),
            self.inner.metrics.popped_frames.load(Ordering::Relaxed),
            self.inner.metrics.dropped_frames.load(Ordering::Relaxed),
        )
    }
}

/// Creates a paired `(AudioProducer, AudioConsumer)` with bounded capacity.
#[must_use]
pub fn audio_ring_buffer(
    capacity_frames: usize,
    policy: OverflowPolicy,
) -> (AudioProducer, AudioConsumer) {
    let capacity = capacity_frames.max(2);
    let inner = Arc::new(RingBufferInner {
        queue: ArrayQueue::new(capacity),
        policy,
        metrics: BufferMetrics::default(),
    });

    (
        AudioProducer {
            inner: Arc::clone(&inner),
        },
        AudioConsumer { inner },
    )
}
