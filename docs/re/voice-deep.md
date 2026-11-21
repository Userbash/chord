# Discord 1.0.162 Voice Subsystem — Deep Analysis (Linux x64, Sessions 1–3)

Empirical observation dataset: three live voice sessions in an isolated client process (separate instance, CDP 9223), engine debug logs, CDP RTC WebSocket frame capture, tcpdump, and renderer process strace (full UDP payload bytes). Sensitive tokens, session IDs, and cryptographic secrets are sanitized; public IPs replaced with `<PUB_IP>`; `secret_key` values replaced with `REDACTED`.

## 0. Primary Artifacts

| Artifact | Data Description |
|---|---|
| `voice/rtc1-frames.jsonl` / `rtc2-frames.jsonl` | RTC WS frames for Sessions 1 and 2 (opcodes, sanitized payloads) |
| `discord-run3.log` | Engine logs: RTCConnection, codecs, modes, DAVE MLS, disconnect events |
| `voice/voice-udp3.pcap` (46 MB) / `voice-udp4.pcap` (2.7 MB, snap 96) | UDP media captures for Sessions 2 and 3 |
| `voice/voice-udp3.rtp.json`, `voice-udp4.rtp.json` | Packet classification and cadence analysis |
| `voice/strace-renderer-s3.log` (37 MB) + `voice/strace-media-s3.jsonl` | Full packet bytes for Session 3 and network syscall traces |
| `voice/ss-*.txt` | Socket ownership mapping (fd / PID) |
| `voice/ui/`, `voice/ui-s2/`, `voice/ui7/` | Voice panel DOM snapshots, search, and popovers |

Session 3 was captured under strace and `ss`, demonstrating socket rotation at +60 s.

## 1. RTC Signaling (WebSocket)

Connection establishment: `RTCControlSocket` → `wss://c-<region>-<hash>.discord.media:<2053|2096>/` (v=9), TLS negotiation ~300 ms.

| Opcode | Direction | Payload Summary |
|---|---|---|
| 8 Hello | Inbound | `{"v":8,"heartbeat_interval":13750}` |
| 0 Identify | Outbound | `server_id`, `channel_id`, `user_id`, `session_id` (REDACTED), `token` (REDACTED), `max_dave_protocol_version:1`, `video:true`, `streams:[{type:"video",rid:"100",quality:100}]` |
| 1 Select Protocol | Outbound | `protocol:"udp"`, `data:{address,port,mode}`, `codecs:[{name:"opus",type:"audio",priority:1000,payload_type:120},{name:"AV1",type:"video",priority:1000,payload_type:101,...}]` |
| 2 Ready | Inbound | `ssrc` (audio), `port`, `ip`, `modes:["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"]`, `experiments:["fixed_keyframe..."]`, `streams:[{type:"video",ssrc,rtx_ssrc,rid:"100",quality:100,active:false}]` |
| 4 Session Description | Inbound | `mode:"aead_aes256_gcm_rtpsize"`, `secret_key` (32 bytes, REDACTED), `media_session_id` (REDACTED), `dave_protocol_version:1`, `secure_frames_version:1`, `audio_codec:"opus"`, `video_codec:"AV1"` |
| 15 Media Sink Wants | Inbound | `{"any":100}` — media sink quality request |
| 16 Backend Version | Inbound | `{"voice":"0.22.1+denbeigh.remove-voice-curator-74a3bd0c5","rtc_worker":"1.6.98"}` |
| 3 / 6 Heartbeat / ACK | Out / In | Heartbeat cadence 13,750 ms, ACK (`seq: s`) |
| Close | Inbound | Code 1000, reason "Force Close" upon normal exit |

Observed Sessions:

| Metric | Session 2 | Session 3 |
|---|---|---|
| RTC Endpoint | `c-fra19-4da7fe5f.discord.media:2053` | `c-arn02-5229f1fc.discord.media:2096` |
| Region Latency Test | frankfurt, rotterdam, paris, milan, brussels | stockholm, helsinki, finland, denmark, warsaw |
| Media Server | `104.29.147.145:19309` | `104.29.146.60:19329` |
| SSRCs (audio / video / rtx) | 17843 / 17844 / 17845 | 9208 / 9209 / 9210 |
| Client Public Endpoint | `<PUB_IP>:52287` | `<PUB_IP>:35251` (post-rotation: updated port) |
| Session Duration | ~88 s | ~104 s |

`RTCLatencyTestManager` probes regional UDP endpoints before joining, prioritizing lowest latency.

## 2. UDP Transport Layer

### 2.1 IP Discovery
Request (74 B): `[u16 type=1][u16 len=0x0046][u32 ssrc][64 B zero padding]`  
Response (74 B): `[u16 type=2][u16 len=0x0046][u32 ssrc][64 B NUL-padded ASCII address][u16 BE port @72]`.  
Example Response: `0002 0046 000023f8 "<PUB_IP>\0…" 89b3` → port 35251.  
Two candidate sockets transmit discovery simultaneously; the fastest response establishes the route.

### 2.2 Keepalive
8 Bytes: client sends `13 37 ca fe 01 00 00 00`, server replies `13 37 f0 0d 01 00 00 00`. Cadence: 5.000 s (4.96–5.02 s range), bidirectional.

### 2.3 RTP Media
Header format: `V=2, P=0, X=1, CC=0`, `PT=120` (opus), RFC 5285 extension header `0xBEDE`, 2 words (8 B data).  
Trailer: `[ciphertext][16 B GCM tag][4 B LE counter]`. Counter: `0x80000000 | n`, sequential across RTP and RTCP on the socket.  
Packet interval: 20 ms (timestamp deltas 960/1050/870 sum to 3840 per 4 frames = 960 @ 48 kHz).  
During idle periods, an initial 200 ms talkspurt (10 packets) transitions to complete DTX silence.

### 2.4 RTCP / SRTCP
- `SR` (PT 200) Client → Server: `[8 B header][ssrc][20 B encrypted sender info][16 B tag][4 B counter]` = 60 B. Intervals: 3.0–6.3 s (median 5.2 s).
- `RR` (PT 201) Server → Client: `[8 B header][ssrc][24 B encrypted report block][16 B tag][4 B counter]` = 52 B, exactly 1/s (median 1001 ms). Server trailer lacks the `0x80` flag.
- Empty `RR` (28 B) Client → Server: Transmitted immediately post-socket rotation.

### 2.5 Media Socket Rotation (+60 s)
Replicated across sessions and verified via strace:  
`12:39:27.880 socket() → sendmsg(discovery)` → **`12:40:27.961 socket()`** (+60.08 s) → discovery with same SSRC → new NAT port → empty RR → uninterrupted SR/keepalive flow until disconnect.

## 3. Engine Log Analysis

```text
[Connection] Available engine codecs: AV1(dec+enc), H264, H265, VP8
[Connection] Experimental codecs: AV1, H265
[Connection] Audio codecs: opus
[Connection] Encryption modes: aead_aes256_gcm_rtpsize, aead_aes256_gcm,
  aead_xchacha20_poly1305_rtpsize, xsalsa20_poly1305_lite_rtpsize,
  xsalsa20_poly1305_lite, xsalsa20_poly1305_suffix, xsalsa20_poly1305
[Connection] Selected encryption mode: aead_aes256_gcm_rtpsize
[Connection] updateVideoQuality: 1280x720@30, bitrate 2.5 Mbps (+-150k), rid 100,
  ssrc 9209 (rtx 9210), remoteSinkWantsMaxFramerate 20
[ConnectionEventFramerateReducer] Reduced framerate after 15000 ms
[RTCConnection] Remote media sink wants: {"any":100}
[RTCConnection] DAVE protocol init with protocol version: 1
[RTCConnection] Received MLS external sender package / Got MLS key package, sending to RTC server
[RTCConnection] Setting media-session-id: <REDACTED>
[RTCConnection] Disconnected clean: true, code: 1000, reason: Force Close
[VOICE_DISCONNECT] krisp_nc_model: krisp-nc-o-med-v7.kef
```

- Krisp Noise Cancellation uses model `krisp-nc-o-med-v7.kef`.
- DAVE MLS v1 provides end-to-end encryption atop SRTP transport.

## 4. Voice UI Components
Voice status rendered in `[class*=panels]` with Mute/Deafen/Settings/Disconnect controls, ping indicator, and audio device selectors (`voice/ui/01-voice-panel.html`). Channel joining executed via QuickSwitcher (`Ctrl+K`); disconnection via clean close code 1000.

## 5. Media Capture Constraints Protocol (D-04)

Empirical specification of Discord's audio capture parameters, sample rate requirements, media device selection, echo cancellation (AEC) options, Automatic Gain Control (AGC) algorithms, noise suppression flags, and stream renegotiation (`discord_voice.node` `src/transport_settings.cpp`, `src/media/audio_device/device_selection.cpp`, `src/media/captured_audio_processor.cpp`, `src/media/echo_canceller_factory.cpp`, `src/krisp/noise_canceller.cpp`, and `chord_voice::{media_constraints,devices,noise,stream_quality}`).

### 5.1 Audio Capture Parameters & Sample Rate Requirements

| Parameter / Symbol | Wire / Native Value | Voice Engine Mapping (`chord_voice`) | Contract Notes |
|---|---|---|---|
| `DEFAULT_AUDIO_SAMPLE_RATE_HZ` | `48000` Hz | `media_constraints::DEFAULT_AUDIO_SAMPLE_RATE_HZ` | Canonical WebRTC Opus capture & screen-share loopback clock |
| `DEFAULT_AUDIO_CHANNELS` | `2` (stereo) | `media_constraints::DEFAULT_AUDIO_CHANNELS` | Canonical WebRTC Opus voice & virtual desktop audio channel count |
| `SUPPORTED_SAMPLE_RATES_HZ` | `[8000, 16000, 32000, 48000]` Hz | `noise::SUPPORTED_SAMPLE_RATES_HZ` | Supported APM & Krisp capture sample rates (`NB`, `WB`, `SWB`, `FB`) |
| `SUPPORTED_FRAME_DURATIONS_MS` | `[10, 20, 30]` ms | `noise::SUPPORTED_FRAME_DURATIONS_MS` | Supported PCM S16LE frame intervals (`960` samples/ch @ `48 kHz` / `20 ms`) |
| `setVoiceSampleRateCap` | NAPI property (`strings-a-n6.txt:73302`) | `NoiseSuppressionPipeline::setup` | Caps hardware capture sample rate to `[8000..=48000]` Hz |
| `setVoiceChannelCountCap` | NAPI property (`strings-a-n6.txt:73303`) | `NoiseSuppressionPipeline::setup` (`1..=2`) | Caps capture channel count (`1` mono / `2` stereo) |
| `sampleRateMismatchPercent` | `<= 15%` (`strings-a-n6.txt:73961`) | `noise::MAX_SAMPLE_RATE_MISMATCH_PERCENT = 15` | Clock drift `> 15%` automatically degrades `Krisp` -> `RnNoise` |
| `inputVolume` / `outputVolume` | `0..=100%` / `0..=200%` (`strings-a-n6.txt:73231-73233`) | `devices::{MAX_INPUT_VOLUME, MAX_OUTPUT_VOLUME}` | Default `100%` (`DEFAULT_INPUT_VOLUME`, `DEFAULT_OUTPUT_VOLUME`) |

### 5.2 Media Device Selection & Virtual Audio Loopback Injection

- **Native NAPI Device Selectors (`src/media/audio_device/device_selection.cpp`):**
  - `getInputDevices`, `getOutputDevices`, `getVideoInputDevices` enumerate active OS endpoints.
  - `setInputDevice`, `setOutputDevice`, `setVideoInputDevice` accept either a device index or exact device ID string (`"setInputDevice expects either a device index or device id string"`, `strings-a-n6.txt:73237-73239`).
  - `setDeviceChangeCallback` notifies the Voice Engine on OS hot-plug/unplug topology changes.
- **Exact `deviceId` Matching & Hot-Plug Stability (`chord_voice::media_constraints::DeviceIdMatchConstraint`):**
  - Voice input and virtual stream tracks bind via `DeviceIdMatchConstraint::Exact("<id>")` (`"deviceId":{"exact":"<id>"}`), preventing OS default-device hijack when Bluetooth or USB headsets are hot-plugged.
  - When a pinned physical microphone disconnects, `MediaConstraintsEngine::handle_device_Topology_change` transitions to `DeviceIdMatchConstraint::ExactWithFallback { preferred_exact_id, effective_active_id }` (falling back to the `is_default == true` input device) and automatically restores `Exact(preferred_exact_id)` when the device reconnects.
- **Virtual Audio Track Injection & Anti-Echo Isolation:**
  - Linux PipeWire virtual loopback (`VirtualAudioSourceKind::PipeWireLoopback`): `DEFAULT_VIRTUAL_AUDIO_DEVICE_ID = "chord-screen-share-audio"` (`48000` Hz, `2` ch).
  - Windows WASAPI loopback (`VirtualAudioSourceKind::WasapiLoopback`): `DEFAULT_WASAPI_VIRTUAL_AUDIO_DEVICE_ID = "chord-wasapi-loopback-audio"` (`48000` Hz, `2` ch).
  - macOS CoreAudio process tap (`VirtualAudioSourceKind::CoreAudioProcessTap`).
  - `OwnApplicationFilter` (`devices::OwnApplicationFilter`) excludes `target_pid == 0`, `target_pid == own_pid`, and client binary names from desktop audio capture so remote call participants and local microphone loopback never echo into screen-share streams.
  - Human-readable OS device `label` fields in `AudioDevice` and `VirtualAudioTrackDescriptor` are redacted as `"<redacted>"` in `Debug` output.

### 5.3 Echo Cancellation (AEC) Options & Acoustic Pipeline

Extracted from `src/transport_settings.cpp`, `src/media/echo_canceller_factory.cpp`, `src/media/echo_canceller_probe_bank.cpp`, and `modules/audio_processing/aec3/echo_canceller3.cc` (`strings-a-n6.txt:73211-73232`, `83539-83542`):

| Option / Field | Default (Voice Mic) | Screen-Share Loopback | Purpose & Engine Behavior |
|---|---|---|---|
| `echoCancellation` | `true` | `false` | Enables WebRTC `EchoCanceller3` (`AEC3`) acoustic echo cancellation |
| `echoCancellationPreEcho` | `true` | `false` | Pre-echo suppression filter ahead of nonlinear AEC3 stage |
| `echoReferenceMode` | `"system"` / `"loopback"` | `"disabled"` | Selects playout reference sink for acoustic delay estimation |
| `builtInEchoCancellation` | `false` (software AEC3 preferred) | `false` | Hardware/OS ADM built-in AEC toggle |
| `bypassSystemProcessing` | `false` | `true` | Bypasses OS/driver DSP when native WebRTC APM/Krisp is active |
| `enforce_high_pass_filtering` | `true` | `false` | High-pass DC/rumble removal in `AudioProcessing::Config::echo_canceller` |
| `echo_delay_ms` | Telemetry counter (`u32`) | `0` | Estimated acoustic path delay in `NoiseCancellationStats` (`erl`, `erle`) |
| `NoopEchoControl` | Fallback when `echoCancellation: false` | Active | Bypasses linear AEC output (`echo_canceller_factory.cpp`) |

### 5.4 Automatic Gain Control (AGC) Algorithms & Profiles

Discord's native `setTransportOptions` (`automaticGainControl`, `automaticGainControlConfig`, `strings-a-n6.txt:73214-73215`) and `AudioProcessing::Config` (`strings-a-n6.txt:83544-83576`) combine `gain_controller1` (`target_level_dbfs`, `compression_gain_db`, `enable_limiter`, `analog_gain_controller`, `clipping_predictor`) and `gain_controller2` (`fixed_digital`, `adaptive_digital`). The Voice Engine enforces three deterministic `AgcMode` profiles (`chord_voice::media_constraints::AgcPolicyConfig`):

| Profile (`AgcMode`) | `auto_gain_control` | `echo_cancellation` | `noise_suppression` | `typing_noise_detection` | `target_level_dbfs` | `compression_gain_db` | `limiter_enabled` | Target Use Case |
|---|---|---|---|---|---:|---:|---|---|
| `AdaptiveDigital` (`voice_adaptive_default`) | `true` | `true` | `true` | `true` | `-3` dBFS | `9` dB | `true` | Standard voice microphones (elevates `vadKrispActivationThreshold` to `0.8`) |
| `FixedDigital` (`voice_fixed_manual`) | `false` | `true` | `true` | `true` | `-3` dBFS | `0` dB | `true` | Studio USB audio interfaces with hardware gain knobs |
| `BypassedForStream` (`screen_share_passthrough`) | `false` | `false` | `false` | `false` | `0` dBFS | `0` dB | `false` | Virtual desktop/application audio tracks (`SOUNDSHARE`) |

### 5.5 Noise Suppression Flags, Krisp Models & VAD Gating

Extracted from `src/transport_settings.cpp`, `src/krisp/noise_canceller.cpp`, `src/krisp/voice_activity_detector.cpp`, and `src/media/captured_audio_processor.cpp`:

- **Transport & Filter Flags (`NoiseSuppressionConfig`):**
  - `noiseSuppression: true` (standard spectral/RNN fallback), `noiseCancellation: true` (Krisp deep-learning NC), `noiseCancellationDuringProcessing: false`, `skipNoiseCancellationIfMuted: true` (zeroes muted frames and skips neural inference to save CPU).
  - `suppressionLevel: 0..=100` (default `100%`), `max_attenuation_db: 0..=60` dB (`45` dB for `Krisp`, `30` dB for `RnNoise`).
  - `vadThreshold: -100..=0` dBFS (default `-40` dBFS), `vadAutoThreshold: true` (adaptive EMA noise floor `[-100.0, -20.0]` dBFS with `+6.0` dB SNR margin and `4.0` dB whisper hysteresis), `vadLeading: 5` frames (`100` ms), `vadTrailing: 25` frames (`500` ms, max `100` frames).
  - `vadUseKrisp: true`, `vadKrispActivationThreshold`: `0.5` (`DEFAULT_KRISP_VAD_THRESHOLD`) when AGC is off, `0.8` (`AGC_KRISP_VAD_THRESHOLD`) when AGC is on.
- **Krisp Model Tiers (`KrispModelTier`) & Bundled `.kef` Weights (`discord_krisp.node`):**
  - `SmallNc8k` (`"small_NC_8k"` -> `krisp-nc-o-nb-v2.kef`, selected at `8000` Hz);
  - `SmallNc` (`"small_NC"` -> `krisp-nc-o-lite-v1.kef`, selected at `>= 16000` Hz when `fullband_performance == false`);
  - `FullNc` (`"full_NC"` -> `krisp-nc-o-med-v7.kef`, selected at `48000` Hz with `fullband_performance == true`);
  - Auxiliary models: `krisp-bvc-o-pro-v3.kef` (Background Voice Cancellation Pro) and `krisp-vad-o-v2.kef` (Krisp VAD).
- **CPU Overuse Guard (`MaybeToggleNoiseCanceller`):**
  - If frame processing time exceeds `MAX_CPU_PROCESS_TIME_US_PER_20MS = 12_000` µs for `CPU_OVERUSE_CONSECUTIVE_FRAMES_LIMIT = 5` consecutive 20 ms frames, `NoiseCancellerState` transitions to `DisabledByCpuOveruse` and automatically falls back from `Krisp` to `RnNoise` (or `StandardWebRtc`).

### 5.6 Stream Renegotiation & Video Encoder Resolution Bounds

- **Lifecycle & Mid-Stream Renegotiation (`MediaConstraintsEngine`):**
  - State machine: `Idle -> Active -> Stopped`.
  - Mid-stream virtual audio injection via `inject_virtual_audio_track` attaches a 48 kHz stereo loopback track with `AgcMode::BypassedForStream` without interrupting video encoding, setting Voice Gateway Opcode 5 `SPEAKING` bitflag `SOUNDSHARE_SPEAKING_BIT = 2` (`1 << 1`).
  - Mid-stream resolution/FPS update via `renegotiate_video_bounds` updates `ResolutionBounds` (`160x90`..`3840x2160`, even YUV420 dimensions, `1..=120` FPS; canonical presets `480p = 854x480`, `720p = 1280x720`, `1080p = 1920x1080`, `1440p = 2560x1440`, `2160p = 3840x2160`) while preserving the active virtual audio track.
- **Signaling & Bandwidth Adaptation (`StreamQualityController`):**
  - `handle_go_live`, `patch_stream_quality`, and `set_quality_profile` emit Voice Gateway **Opcode 12 (`Video`)** with `audio_ssrc`, `video_ssrc = audio_ssrc + 1`, `rtx_ssrc = audio_ssrc + 2`, `rid: "100"`, `max_resolution`, `max_framerate`, and `max_bitrate` (`150 kbps ..= 25 Mbps`; `720p@30 = 2.5 Mbps`, `720p@60 = 4.0 Mbps`).
  - Inbound Voice Gateway **Opcode 15 (`MediaSinkWants`)** dynamically scales or pauses encoding (`{"any": 0}` or per-SSRC `0` transitions to `StreamAdaptationState::PausedBySink`; `{"any": 100}` resumes `Active`).
  - `ConnectionEventFramerateReducer` reduces maximum encoding framerate to `IDLE_REDUCED_MAX_FRAMERATE = 20` FPS after `CONNECTION_IDLE_FRAMERATE_REDUCE_MS = 15_000` ms of static screen content.
  - RTCP Receiver Report loss (`fraction_lost >= 13/256` congestion drop `-20%`; recovery after `2000` ms holdoff at `<= 5/256`) and RTCP REMB (`4/5` headroom, `5000` ms TTL) adapt target bitrate within `[min_bitrate_bps, max_bitrate_bps]`.
