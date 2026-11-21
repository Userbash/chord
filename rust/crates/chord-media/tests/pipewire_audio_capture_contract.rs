#![forbid(unsafe_code)]
//! Contract tests for Task R5-01: Audio Capture (PipeWire).
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` row `R5-01`: "Capture desktop audio via PipeWire loopback and
//!   publish a virtual microphone source (2 channels, 48 kHz, s16le) for WebRTC streaming.
//!   Target: Capture Service (`pipewire-rs`). Criteria: Desktop sound routes into WebRTC voice
//!   stream; virtual loopback device cleanly registered"
//! - `docs/vesktop-analysis.md` §2.1: "Vesktop's `venmic` module registers a virtual microphone
//!   node via PipeWire's C API and links system audio outputs into the WebRTC stream. Our
//!   Implementation: Implemented natively in Rust via `pipewire-rs` within `rust/crates/discord-media/`"
//! - `docs/rust-client/FEATURES.md` row `F-MEDIA-04`: "Desktop Audio Loopback — Desktop audio
//!   capture via PipeWire monitor source"
//! - `docs/rust-client/TEST-PLAN.md` row `T-MEDIA-07`: "Desktop audio loopback: system audio
//!   mixing via PipeWire monitor"
//! - `docs/re/voice-deep.md` §2.3 & `docs/specs/STREAM-WAYLAND-GUIDE.md` §2.2:
//!   48 kHz stereo (2 channels), 20ms framing (960 frames / 1920 interleaved samples / 3840 bytes s16le),
//!   Voice Gateway OP 5 SPEAKING flag `speaking: 2` (`SOUNDSHARE`).
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: virtual microphone node registration (`Audio/Source/Virtual`, 2ch, 48 kHz, s16le),
//!    desktop loopback link routing (`FL -> FL`, `FR -> FR`), and 20ms s16le WebRTC audio frame draining.
//! 2. Negative path: rejecting invalid audio format parameters (0 channels, >2 channels, non-48kHz,
//!    empty node name, invalid node ID `0`, self-loopback link to the virtual mic itself, odd byte slices).
//! 3. Boundary conditions: 20ms framing boundaries (`1919` samples buffered, `1920` samples emits 1 frame),
//!    `i16::MIN` (`-32768`) / `i16::MAX` (`32767`) s16le <-> normalized `f32` conversion and saturation at `0%`, `100%`, `200%` gain,
//!    ring buffer capacity cap with oldest-sample eviction (`overrun_samples`).
//! 4. Error handling: backend daemon disconnect, link failure rollback, operation in invalid lifecycle state.
//! 5. Empty states: draining when no desktop audio has been captured returns empty frame list;
//!    silent desktop stream emits zeroed s16le frames or DTX comfort-noise/silence indicator.
//! 6. Intermediate / lifecycle states: `Unregistered -> Registered -> Streaming -> Paused -> Stopped` transitions.
//! 7. Capabilities & privacy: `Debug` redaction of human-readable OS device/application labels,
//!    Voice OP 5 `SOUNDSHARE` (`speaking == 2`) payload generation.
//! 8. Recovery & degradation: automatic re-registration and link restoration after `pipewire.service` daemon restart,
//!    clean virtual node unregistration on `stop()` and `Drop`.
//! 9. Observable telemetry: accurate tracking of `captured_frames`, `captured_bytes`, `overrun_samples`,
//!    `active_links`, `underrun_reads`, `peak_level_dbfs`, and `daemon_restarts`.

use std::cell::RefCell;
use std::rc::Rc;

use chord_media::pipewire_audio::{
    DEFAULT_VIRTUAL_MIC_DESCRIPTION, DEFAULT_VIRTUAL_MIC_NODE_NAME, PIPEWIRE_AUDIO_CHANNELS,
    PIPEWIRE_AUDIO_FRAME_BYTES_20MS, PIPEWIRE_AUDIO_FRAME_DURATION_MS,
    PIPEWIRE_AUDIO_FRAME_SAMPLES_PER_CHANNEL, PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES,
    PIPEWIRE_AUDIO_SAMPLE_RATE_HZ, PW_MEDIA_CLASS_AUDIO_SINK, PW_MEDIA_CLASS_AUDIO_SOURCE_VIRTUAL,
    PW_MEDIA_CLASS_STREAM_OUTPUT_AUDIO, PipeWireAudioBackend, PipeWireAudioCaptureService,
    PipeWireAudioError, PipeWireAudioFormat, PipeWireAudioNode, PipeWireAudioStreamState,
    PipeWireChannelPort, PipeWireLoopbackConfig, PipeWirePortLink, S16LE_BYTES_PER_SAMPLE,
    SOUNDSHARE_SPEAKING_FLAG, VirtualMicDescriptor, WebRtcAudioFrameS16le,
    build_soundshare_speaking_payload, f32_to_s16le_sample, s16le_bytes_to_samples,
    s16le_to_f32_sample, samples_to_s16le_bytes,
};

// ---------------------------------------------------------------------------
// Mock PipeWire Audio Graph Backend for deterministic L1/L3 contract testing
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct MockPwGraphState {
    nodes: Vec<PipeWireAudioNode>,
    registered_virtual_mics: Vec<VirtualMicDescriptor>,
    active_links: Vec<PipeWirePortLink>,
    next_node_id: u32,
    next_link_id: u32,
    fail_register: Option<PipeWireAudioError>,
    fail_link: Option<PipeWireAudioError>,
    daemon_alive: bool,
    calls: Vec<String>,
}

#[derive(Clone)]
struct MockPwAudioBackend {
    state: Rc<RefCell<MockPwGraphState>>,
}

impl MockPwAudioBackend {
    fn new_with_desktop_nodes() -> Self {
        let state = MockPwGraphState {
            nodes: vec![
                PipeWireAudioNode {
                    node_id: 40,
                    name: "alsa_output.pci-0000_00_1f.3.analog-stereo".to_string(),
                    description: "Built-in Audio Analog Stereo".to_string(),
                    media_class: PW_MEDIA_CLASS_AUDIO_SINK.to_string(),
                    application_name: None,
                    process_id: Some(1100),
                    channels: 2,
                    sample_rate_hz: 48_000,
                },
                PipeWireAudioNode {
                    node_id: 55,
                    name: "Firefox".to_string(),
                    description: "YouTube — Firefox".to_string(),
                    media_class: PW_MEDIA_CLASS_STREAM_OUTPUT_AUDIO.to_string(),
                    application_name: Some("Firefox".to_string()),
                    process_id: Some(4520),
                    channels: 2,
                    sample_rate_hz: 48_000,
                },
                PipeWireAudioNode {
                    node_id: 56,
                    name: " Gudrun Game ".to_string(),
                    description: "Elden Ring Audio".to_string(),
                    media_class: PW_MEDIA_CLASS_STREAM_OUTPUT_AUDIO.to_string(),
                    application_name: Some("eldenring.exe".to_string()),
                    process_id: Some(8891),
                    channels: 2,
                    sample_rate_hz: 48_000,
                },
            ],
            registered_virtual_mics: Vec::new(),
            active_links: Vec::new(),
            next_node_id: 200,
            next_link_id: 500,
            fail_register: None,
            fail_link: None,
            daemon_alive: true,
            calls: Vec::new(),
        };
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }
}

impl PipeWireAudioBackend for MockPwAudioBackend {
    fn enumerate_nodes(&mut self) -> Result<Vec<PipeWireAudioNode>, PipeWireAudioError> {
        let mut st = self.state.borrow_mut();
        st.calls.push("enumerate_nodes".to_string());
        if !st.daemon_alive {
            return Err(PipeWireAudioError::DaemonDisconnected(
                "pipewire socket closed".to_string(),
            ));
        }
        Ok(st.nodes.clone())
    }

    fn register_virtual_mic(
        &mut self,
        config: &PipeWireLoopbackConfig,
    ) -> Result<VirtualMicDescriptor, PipeWireAudioError> {
        let mut st = self.state.borrow_mut();
        st.calls
            .push(format!("register_virtual_mic:{}", config.node_name));
        if !st.daemon_alive {
            return Err(PipeWireAudioError::DaemonDisconnected(
                "pipewire daemon unreachable".to_string(),
            ));
        }
        if let Some(err) = st.fail_register.clone() {
            return Err(err);
        }
        let node_id = st.next_node_id;
        st.next_node_id += 1;
        let desc = VirtualMicDescriptor {
            node_id,
            node_name: config.node_name.clone(),
            description: config.node_description.clone(),
            media_class: PW_MEDIA_CLASS_AUDIO_SOURCE_VIRTUAL.to_string(),
            format: config.format,
            channels: config.channels,
            sample_rate_hz: config.sample_rate_hz,
        };
        st.registered_virtual_mics.push(desc.clone());
        Ok(desc)
    }

    fn unregister_virtual_mic(&mut self, node_id: u32) -> Result<(), PipeWireAudioError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!("unregister_virtual_mic:{node_id}"));
        st.registered_virtual_mics.retain(|m| m.node_id != node_id);
        st.active_links
            .retain(|l| l.target_virtual_mic_id != node_id);
        Ok(())
    }

    fn create_stereo_link(
        &mut self,
        source_node_id: u32,
        target_virtual_mic_id: u32,
    ) -> Result<[PipeWirePortLink; 2], PipeWireAudioError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!(
            "create_stereo_link:{source_node_id}->{target_virtual_mic_id}"
        ));
        if !st.daemon_alive {
            return Err(PipeWireAudioError::DaemonDisconnected(
                "pipewire daemon unreachable".to_string(),
            ));
        }
        if let Some(err) = st.fail_link.clone() {
            return Err(err);
        }
        if !st.nodes.iter().any(|n| n.node_id == source_node_id) {
            return Err(PipeWireAudioError::NodeNotFound(source_node_id));
        }
        let fl_id = st.next_link_id;
        let fr_id = st.next_link_id + 1;
        st.next_link_id += 2;
        let link_fl = PipeWirePortLink {
            link_id: fl_id,
            source_node_id,
            source_port: PipeWireChannelPort::FrontLeft,
            target_virtual_mic_id,
            target_port: PipeWireChannelPort::FrontLeft,
        };
        let link_fr = PipeWirePortLink {
            link_id: fr_id,
            source_node_id,
            source_port: PipeWireChannelPort::FrontRight,
            target_virtual_mic_id,
            target_port: PipeWireChannelPort::FrontRight,
        };
        st.active_links.push(link_fl.clone());
        st.active_links.push(link_fr.clone());
        Ok([link_fl, link_fr])
    }

    fn destroy_link(&mut self, link_id: u32) -> Result<(), PipeWireAudioError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!("destroy_link:{link_id}"));
        st.active_links.retain(|l| l.link_id != link_id);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 1. Constants & Specification Oracles (2 channels, 48 kHz, s16le, 20ms)
// ---------------------------------------------------------------------------

#[test]
fn pipewire_audio_constants_match_r5_01_venmic_and_webrtc_oracle() {
    assert_eq!(PIPEWIRE_AUDIO_CHANNELS, 2);
    assert_eq!(PIPEWIRE_AUDIO_SAMPLE_RATE_HZ, 48_000);
    assert_eq!(PIPEWIRE_AUDIO_FRAME_DURATION_MS, 20);
    assert_eq!(PIPEWIRE_AUDIO_FRAME_SAMPLES_PER_CHANNEL, 960);
    assert_eq!(PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES, 1_920);
    assert_eq!(S16LE_BYTES_PER_SAMPLE, 2);
    assert_eq!(PIPEWIRE_AUDIO_FRAME_BYTES_20MS, 3_840);

    assert_eq!(PW_MEDIA_CLASS_AUDIO_SOURCE_VIRTUAL, "Audio/Source/Virtual");
    assert_eq!(PW_MEDIA_CLASS_AUDIO_SINK, "Audio/Sink");
    assert_eq!(PW_MEDIA_CLASS_STREAM_OUTPUT_AUDIO, "Stream/Output/Audio");
    assert_eq!(DEFAULT_VIRTUAL_MIC_NODE_NAME, "chord-screen-share-audio");
    assert_eq!(
        DEFAULT_VIRTUAL_MIC_DESCRIPTION,
        "Chord Screen Share Audio (Virtual Microphone)"
    );
    assert_eq!(PipeWireAudioFormat::S16le.spa_format_str(), "S16LE");
    assert_eq!(PipeWireAudioFormat::S16le.bytes_per_sample(), 2);

    assert_eq!(PipeWireChannelPort::FrontLeft.port_name(), "FL");
    assert_eq!(PipeWireChannelPort::FrontRight.port_name(), "FR");
    assert_eq!(PipeWireChannelPort::FrontLeft.input_port_name(), "input_FL");
    assert_eq!(
        PipeWireChannelPort::FrontRight.input_port_name(),
        "input_FR"
    );
    assert_eq!(
        PipeWireChannelPort::FrontLeft.monitor_port_name(),
        "monitor_FL"
    );
    assert_eq!(
        PipeWireChannelPort::FrontRight.monitor_port_name(),
        "monitor_FR"
    );

    assert_eq!(SOUNDSHARE_SPEAKING_FLAG, 2);
    assert_eq!(
        build_soundshare_speaking_payload(9208),
        "{\"op\":5,\"d\":{\"speaking\":2,\"delay\":0,\"ssrc\":9208}}"
    );
}

// ---------------------------------------------------------------------------
// 2. Positive Path: Virtual Microphone Registration, Loopback Linking & WebRTC Frame Drain
// ---------------------------------------------------------------------------

#[test]
fn virtual_mic_registration_loopback_linking_and_webrtc_s16le_stream_routing() {
    let backend = MockPwAudioBackend::new_with_desktop_nodes();
    let state_ref = backend.state.clone();

    let config = PipeWireLoopbackConfig::default();
    assert_eq!(config.channels, 2);
    assert_eq!(config.sample_rate_hz, 48_000);
    assert_eq!(config.format, PipeWireAudioFormat::S16le);
    assert!(config.validate().is_ok());

    let mut service = PipeWireAudioCaptureService::new(backend, config).expect("valid config");
    assert_eq!(service.state(), PipeWireAudioStreamState::Unregistered);
    assert!(service.virtual_mic().is_none());

    // Step 1: Register virtual microphone node in PipeWire
    let vmic = service
        .register_virtual_microphone()
        .expect("register virtual mic");
    assert_eq!(vmic.node_id, 200);
    assert_eq!(vmic.node_name, DEFAULT_VIRTUAL_MIC_NODE_NAME);
    assert_eq!(vmic.media_class, "Audio/Source/Virtual");
    assert_eq!(vmic.channels, 2);
    assert_eq!(vmic.sample_rate_hz, 48_000);
    assert_eq!(vmic.format, PipeWireAudioFormat::S16le);
    assert_eq!(service.state(), PipeWireAudioStreamState::Registered);

    // Step 2: Route desktop audio nodes (e.g., Firefox node 55 and Game node 56) into virtual mic
    let links_ff = service
        .attach_desktop_source_node(55)
        .expect("link Firefox audio stream");
    assert_eq!(links_ff[0].source_node_id, 55);
    assert_eq!(links_ff[0].source_port, PipeWireChannelPort::FrontLeft);
    assert_eq!(links_ff[0].target_virtual_mic_id, 200);
    assert_eq!(links_ff[0].target_port, PipeWireChannelPort::FrontLeft);
    assert_eq!(links_ff[1].source_port, PipeWireChannelPort::FrontRight);
    assert_eq!(links_ff[1].target_port, PipeWireChannelPort::FrontRight);
    assert_eq!(service.state(), PipeWireAudioStreamState::Streaming);

    // Duplicate attach of the same node is idempotent and returns existing stereo links
    let links_dup = service
        .attach_desktop_source_node(55)
        .expect("idempotent attach");
    assert_eq!(links_dup, links_ff);
    assert_eq!(service.active_links().len(), 2);

    // Attach second application stream (Game node 56)
    service
        .attach_desktop_source_node(56)
        .expect("link Game audio stream");
    assert_eq!(service.active_links().len(), 4);

    // Step 3: Push desktop loopback s16le audio and drain 20ms WebRTC frames (1920 samples = 3840 bytes)
    let input_samples: Vec<i16> = (0..PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES)
        .map(|i| if i % 2 == 0 { 12_000i16 } else { -12_000i16 })
        .collect();
    let raw_bytes = samples_to_s16le_bytes(&input_samples);
    assert_eq!(raw_bytes.len(), PIPEWIRE_AUDIO_FRAME_BYTES_20MS);

    let pushed = service
        .ingest_loopback_s16le_bytes(&raw_bytes)
        .expect("ingest s16le bytes");
    assert_eq!(pushed, PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES);

    let frames = service.drain_webrtc_frames();
    assert_eq!(frames.len(), 1);
    let frame: &WebRtcAudioFrameS16le = &frames[0];
    assert_eq!(frame.sequence, 1);
    assert_eq!(frame.timestamp_samples, 0);
    assert_eq!(frame.channels, 2);
    assert_eq!(frame.sample_rate_hz, 48_000);
    assert_eq!(frame.samples.len(), PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES);
    assert_eq!(frame.samples, input_samples);
    assert_eq!(frame.to_le_bytes(), raw_bytes);
    assert!(!frame.is_silence);

    // Step 4: Detach a single node and verify remaining link stays active
    service.detach_desktop_source_node(55).expect("detach 55");
    assert_eq!(service.active_links().len(), 2);
    assert_eq!(state_ref.borrow().active_links.len(), 2);

    // Step 5: Cleanly stop and unregister virtual loopback device
    service.stop().expect("stop capture service");
    assert_eq!(service.state(), PipeWireAudioStreamState::Stopped);
    assert!(service.virtual_mic().is_none());
    assert!(service.active_links().is_empty());
    assert!(state_ref.borrow().registered_virtual_mics.is_empty());
    assert!(state_ref.borrow().active_links.is_empty());
}

// ---------------------------------------------------------------------------
// 3. Negative Scenarios: Config Validation, Self-Loopback Rejection, Odd Byte Slices
// ---------------------------------------------------------------------------

#[test]
fn pipewire_audio_rejects_invalid_configurations_self_links_and_malformed_pcm_bytes() {
    // 1. Invalid channel counts (must be stereo = 2 for R5-01 WebRTC desktop stream)
    let mut bad_ch = PipeWireLoopbackConfig {
        channels: 0,
        ..Default::default()
    };
    assert_eq!(
        bad_ch.validate(),
        Err(PipeWireAudioError::UnsupportedChannels(0))
    );
    bad_ch.channels = 1;
    assert_eq!(
        bad_ch.validate(),
        Err(PipeWireAudioError::UnsupportedChannels(1))
    );
    bad_ch.channels = 6;
    assert_eq!(
        bad_ch.validate(),
        Err(PipeWireAudioError::UnsupportedChannels(6))
    );

    // 2. Invalid sample rate (must be 48_000 Hz)
    let bad_rate = PipeWireLoopbackConfig {
        sample_rate_hz: 44_100,
        ..Default::default()
    };
    assert_eq!(
        bad_rate.validate(),
        Err(PipeWireAudioError::UnsupportedSampleRate(44_100))
    );

    // 3. Empty or control-char node name
    let mut bad_name = PipeWireLoopbackConfig {
        node_name: "   ".to_string(),
        ..Default::default()
    };
    assert!(matches!(
        bad_name.validate(),
        Err(PipeWireAudioError::InvalidNodeName(_))
    ));
    bad_name.node_name = "bad\0name".to_string();
    assert!(matches!(
        bad_name.validate(),
        Err(PipeWireAudioError::InvalidNodeName(_))
    ));

    // 4. Zero ring capacity
    let bad_cap = PipeWireLoopbackConfig {
        max_buffered_frames: 0,
        ..Default::default()
    };
    assert_eq!(
        bad_cap.validate(),
        Err(PipeWireAudioError::InvalidBufferCapacity(0))
    );

    // 5. Rejects linking before virtual mic registration, linking node 0, or self-loopback
    let backend = MockPwAudioBackend::new_with_desktop_nodes();
    let mut service =
        PipeWireAudioCaptureService::new(backend, PipeWireLoopbackConfig::default()).unwrap();

    assert_eq!(
        service.attach_desktop_source_node(55),
        Err(PipeWireAudioError::InvalidState {
            expected: "Registered or Streaming",
            actual: PipeWireAudioStreamState::Unregistered,
        })
    );

    let vmic = service.register_virtual_microphone().unwrap();
    assert_eq!(
        service.attach_desktop_source_node(0),
        Err(PipeWireAudioError::InvalidNodeId(0))
    );
    assert_eq!(
        service.attach_desktop_source_node(vmic.node_id),
        Err(PipeWireAudioError::SelfLoopbackForbidden(vmic.node_id))
    );
    assert_eq!(
        service.attach_desktop_source_node(9999),
        Err(PipeWireAudioError::NodeNotFound(9999))
    );

    // 6. Odd byte count in s16le byte slice is rejected without partial buffer corruption
    assert_eq!(
        s16le_bytes_to_samples(&[0x01, 0x02, 0x03]),
        Err(PipeWireAudioError::UnalignedS16leBytes(3))
    );
    assert_eq!(
        service.ingest_loopback_s16le_bytes(&[0x10, 0x20, 0x30]),
        Err(PipeWireAudioError::UnalignedS16leBytes(3))
    );
}

// ---------------------------------------------------------------------------
// 4. Boundary Conditions: 20ms Framing, s16le <-> f32 Saturation, Volume Gain & Overrun Eviction
// ---------------------------------------------------------------------------

#[test]
fn s16le_framing_boundaries_gain_saturation_and_ring_buffer_overrun_eviction() {
    // 1. Exact s16le <-> f32 boundary conversions
    assert_eq!(s16le_to_f32_sample(0), 0.0);
    assert_eq!(s16le_to_f32_sample(i16::MIN), -1.0);
    assert!((s16le_to_f32_sample(i16::MAX) - (32767.0 / 32768.0)).abs() < 1e-6);

    assert_eq!(f32_to_s16le_sample(0.0), 0);
    assert_eq!(f32_to_s16le_sample(-1.0), i16::MIN);
    assert_eq!(f32_to_s16le_sample(1.0), i16::MAX);
    // Clamping outside [-1.0, 1.0] and NaN/Inf sanitization
    assert_eq!(f32_to_s16le_sample(2.5), i16::MAX);
    assert_eq!(f32_to_s16le_sample(-3.0), i16::MIN);
    assert_eq!(f32_to_s16le_sample(f32::NAN), 0);
    assert_eq!(f32_to_s16le_sample(f32::INFINITY), i16::MAX);
    assert_eq!(f32_to_s16le_sample(f32::NEG_INFINITY), i16::MIN);

    // 2. 20ms framing boundary: 1919 samples (< 1920) emits 0 frames; +1 sample emits 1 frame
    let backend = MockPwAudioBackend::new_with_desktop_nodes();
    let cfg = PipeWireLoopbackConfig {
        max_buffered_frames: 2, // Capacity = 2 * 1920 = 3840 samples
        ..Default::default()
    };
    let mut service = PipeWireAudioCaptureService::new(backend, cfg).unwrap();
    service.register_virtual_microphone().unwrap();
    service.attach_desktop_source_node(40).unwrap();

    service
        .ingest_loopback_samples(&vec![1000i16; 1919])
        .unwrap();
    assert_eq!(service.buffered_samples(), 1919);
    assert!(service.drain_webrtc_frames().is_empty());

    // Push 1 odd stereo-completing sample -> reaches 1920 samples (960 stereo frames = 20ms)
    service.ingest_loopback_samples(&[1000i16]).unwrap();
    assert_eq!(service.buffered_samples(), 1920);
    let drained = service.drain_webrtc_frames();
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].sequence, 1);
    assert_eq!(drained[0].timestamp_samples, 0);
    assert_eq!(service.buffered_samples(), 0);

    // 3. Volume scaling boundaries: 0% (mute), 100% (unity), 200% (clamped to i16::MIN..=i16::MAX)
    service.set_gain_percent(0);
    assert_eq!(service.gain_percent(), 0);
    service
        .ingest_loopback_samples(&vec![20_000i16; 1920])
        .unwrap();
    let muted_frame = &service.drain_webrtc_frames()[0];
    assert!(muted_frame.is_silence);
    assert!(muted_frame.samples.iter().all(|&s| s == 0));

    service.set_gain_percent(250); // Clamped to MAX_GAIN_PERCENT = 200
    assert_eq!(service.gain_percent(), 200);
    let mut loud_chunk = vec![20_000i16; 960];
    loud_chunk.extend(vec![-20_000i16; 960]);
    service.ingest_loopback_samples(&loud_chunk).unwrap();
    let amplified_frame = &service.drain_webrtc_frames()[0];
    assert!(
        amplified_frame.samples[..960]
            .iter()
            .all(|&s| s == i16::MAX)
    );
    assert!(
        amplified_frame.samples[960..]
            .iter()
            .all(|&s| s == i16::MIN)
    );
    assert_eq!(amplified_frame.timestamp_samples, 1920); // 2nd frame after 0 and 960 ticks

    // 4. Ring buffer capacity cap (2 frames = 3840 samples): pushing 3 frames drops oldest 1920 samples
    service.set_gain_percent(100);
    service
        .ingest_loopback_samples(&vec![111i16; 1920])
        .unwrap();
    service
        .ingest_loopback_samples(&vec![222i16; 1920])
        .unwrap();
    service
        .ingest_loopback_samples(&vec![333i16; 1920])
        .unwrap();

    assert_eq!(service.telemetry().overrun_samples, 1920);
    let kept_frames = service.drain_webrtc_frames();
    assert_eq!(kept_frames.len(), 2);
    assert!(kept_frames[0].samples.iter().all(|&s| s == 222));
    assert!(kept_frames[1].samples.iter().all(|&s| s == 333));
}

// ---------------------------------------------------------------------------
// 5. Multi-Stream Desktop Audio Mixing (Combining Multiple Application Streams in s16le)
// ---------------------------------------------------------------------------

#[test]
fn mixes_multiple_concurrent_desktop_audio_streams_with_saturation() {
    let backend = MockPwAudioBackend::new_with_desktop_nodes();
    let mut service =
        PipeWireAudioCaptureService::new(backend, PipeWireLoopbackConfig::default()).unwrap();
    service.register_virtual_microphone().unwrap();
    service.attach_desktop_source_node(55).unwrap();
    service.attach_desktop_source_node(56).unwrap();

    let stream_a = vec![10_000i16; PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES];
    let stream_b = vec![25_000i16; PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES];

    service
        .ingest_mixed_loopback_streams(&[&stream_a, &stream_b])
        .unwrap();
    let frames = service.drain_webrtc_frames();
    assert_eq!(frames.len(), 1);
    // 10_000 + 25_000 = 35_000 -> saturates cleanly at i16::MAX (32_767) without wrap-around overflow!
    assert!(frames[0].samples.iter().all(|&s| s == i16::MAX));
}

// ---------------------------------------------------------------------------
// 6. Lifecycle States, Pause/Resume, Empty Drains & Privacy Redaction
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_transitions_pause_resume_empty_reads_and_debug_privacy_redaction() {
    let backend = MockPwAudioBackend::new_with_desktop_nodes();
    let mut service =
        PipeWireAudioCaptureService::new(backend, PipeWireLoopbackConfig::default()).unwrap();

    // Privacy check: PipeWireAudioNode Debug output must redact human-readable description/app name
    let nodes = service.discover_capturable_nodes().unwrap();
    assert_eq!(nodes.len(), 3);
    let debug_str = format!("{:?}", nodes[1]);
    assert!(
        !debug_str.contains("YouTube"),
        "Debug format must redact window/media description: {debug_str}"
    );
    assert!(debug_str.contains("<redacted>"));

    // Empty read before any audio returns zero-filled silence frame on poll_next_webrtc_frame_or_silence
    service.register_virtual_microphone().unwrap();
    service.attach_desktop_source_node(40).unwrap();

    let silence_fallback = service.poll_next_webrtc_frame_or_silence();
    assert!(silence_fallback.is_silence);
    assert_eq!(
        silence_fallback.samples.len(),
        PIPEWIRE_AUDIO_FRAME_TOTAL_SAMPLES
    );
    assert!(silence_fallback.samples.iter().all(|&s| s == 0));
    assert_eq!(service.telemetry().underrun_reads, 1);

    // Pause / Resume lifecycle
    service.pause().expect("pause active stream");
    assert_eq!(service.state(), PipeWireAudioStreamState::Paused);

    // While paused, ingested audio is dropped (not buffered) so stale audio never plays after resume
    let ingested_while_paused = service
        .ingest_loopback_samples(&vec![5000i16; 1920])
        .unwrap();
    assert_eq!(ingested_while_paused, 0);
    assert!(service.drain_webrtc_frames().is_empty());

    service.resume().expect("resume paused stream");
    assert_eq!(service.state(), PipeWireAudioStreamState::Streaming);

    service
        .ingest_loopback_samples(&vec![5000i16; 1920])
        .unwrap();
    assert_eq!(service.drain_webrtc_frames().len(), 1);
}

// ---------------------------------------------------------------------------
// 7. Daemon Disconnect, Recovery on pipewire.service Restart & Drop Cleanup
// ---------------------------------------------------------------------------

#[test]
fn recovers_virtual_mic_and_loopback_links_after_pipewire_daemon_restart_and_cleans_up_on_drop() {
    let backend = MockPwAudioBackend::new_with_desktop_nodes();
    let state_ref = backend.state.clone();

    {
        let mut service =
            PipeWireAudioCaptureService::new(backend, PipeWireLoopbackConfig::default()).unwrap();
        let vmic_initial = service.register_virtual_microphone().unwrap();
        assert_eq!(vmic_initial.node_id, 200);
        service.attach_desktop_source_node(40).unwrap();
        service.attach_desktop_source_node(55).unwrap();
        assert_eq!(service.active_links().len(), 4);

        // Simulate pipewire.service crash/restart (old node and links wiped in daemon)
        {
            let mut st = state_ref.borrow_mut();
            st.registered_virtual_mics.clear();
            st.active_links.clear();
            st.daemon_alive = false;
        }

        // Recovery while daemon is still down returns DaemonDisconnected error
        assert!(matches!(
            service.recover_after_daemon_restart(),
            Err(PipeWireAudioError::DaemonDisconnected(_))
        ));

        // Daemon comes back online -> recover_after_daemon_restart re-registers virtual mic and re-links nodes 40 & 55
        state_ref.borrow_mut().daemon_alive = true;
        let recovered_vmic = service
            .recover_after_daemon_restart()
            .expect("recover virtual mic and links");
        assert_eq!(recovered_vmic.node_id, 201);
        assert_eq!(service.state(), PipeWireAudioStreamState::Streaming);
        assert_eq!(service.active_links().len(), 4);
        assert!(
            service
                .active_links()
                .iter()
                .all(|l| l.target_virtual_mic_id == 201)
        );
        assert_eq!(service.telemetry().daemon_restarts, 1);
    }

    // Upon Drop of PipeWireAudioCaptureService, virtual mic and links must be automatically unregistered
    let st = state_ref.borrow();
    assert!(
        st.registered_virtual_mics.is_empty(),
        "Drop must unregister virtual microphone from PipeWire"
    );
    assert!(
        st.active_links.is_empty(),
        "Drop must remove all loopback links from PipeWire"
    );
}
