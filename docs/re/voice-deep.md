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

---

## 5. Streaming Protocol Wire Contracts (D-05 — Media Engine Specification)

Full clean-room wire contract specification for WebRTC / Go Live screen share signaling, resolution parameters, video codec negotiation (`H264`, `AV1`, `H265`, `VP8`), RTP packetization rules (`90,000 Hz`, `0xBEDE`, RFC 6184, AV1 OBU Aggregation Header, RFC 4588 RTX), opcodes `18..=22`, and keyframe request opcodes (`RTCP PLI`, `RTCP FIR`, `OP 12 VIDEO`, `OP 15 MEDIA_SINK_WANTS`). Implemented and verified in `chord_voice::{stream_quality, rtp::video, rtcp, session, dave, media::rtp}`.

### 5.1 Resolution Parameters, Quality Profiles & Dynamic Adaptation (`chord_voice::stream_quality`)

- **Dimension & Framerate Bounds:**
  - Width / height range: `MIN_STREAM_DIMENSION = 64` ..= `MAX_STREAM_DIMENSION = 3840` pixels; dimensions must be even (`width % 2 == 0 && height % 2 == 0`) for YUV 4:2:0 chroma subsampling (`StreamQualityError::OddDimensions`).
  - Framerate range: `MIN_STREAM_FPS = 1` ..= `MAX_STREAM_FPS = 60` FPS (`StreamFrameratePreset`: `Fps15 = 15`, `Fps30 = 30`, `Fps60 = 60`).
  - Bitrate hierarchy bounds: `MIN_STREAM_BITRATE_BPS = 150,000` (150 kbps) ..= `MAX_STREAM_BITRATE_BPS = 25,000,000` (25 Mbps), enforcing `min_bitrate_bps <= target_bitrate_bps <= max_bitrate_bps`.
- **Resolution Pixel Limits (`StreamResolutionPreset` & `StreamQualityProfile`):**

| Preset (`StreamResolutionPreset`) | Canonical Dimensions (`W × H`) | Exact Pixel Budget (`max_pixel_count`) | 15 FPS `(min / target / max)` | 30 FPS `(min / target / max)` | 60 FPS `(min / target / max)` |
|---|---|---:|---|---|---|
| `P480` (480p) | `854 × 480` | `409,920` (`PIXEL_LIMIT_480P`) | `200k / 800k / 1.2M` | `250k / 1.2M / 2.0M` | `300k / 2.0M / 3.0M` |
| `P720` (720p) | `1280 × 720` | `921,600` (`PIXEL_LIMIT_720P`) | `300k / 1.5M / 2.5M` | `350k / 2.5M / 3.5M` | `400k / 4.0M / 6.0M` |
| `P1080` (1080p) | `1920 × 1080` | `2,073,600` (`PIXEL_LIMIT_1080P`) | `400k / 3.0M / 4.5M` | `500k / 5.0M / 8.0M` | `600k / 8.0M / 12.0M` |
| `P1440` (1440p) | `2560 × 1440` | `3,686,400` (`PIXEL_LIMIT_1440P`) | `800k / 5.0M / 7.0M` | `1.0M / 8.0M / 12.0M` | `1.2M / 12.0M / 16.0M` |
| `Source4K` (2160p) | `3840 × 2160` | `8,294,400` (`PIXEL_LIMIT_2160P`) | — | `1.5M / 14.0M / 20.0M` | `2.0M / 18.0M / 25.0M` |

- **Tier Policy (`GoLiveQualityPatchPolicy`):**
  - `FreeTierClamped`: Clamps resolution to `1280 × 720` (`921,600` px), framerate to `30` FPS, target bitrate to `2,500,000` bps (`2.5 Mbps`), and ceiling bitrate to `3,500,000` bps (`3.5 Mbps`), matching the unpatched engine log `updateVideoQuality: 1280x720@30, bitrate 2.5 Mbps (+-150k), rid 100`.
  - `PatchedUnrestricted` (`patchStreamQuality`): Unlocks full `1080p`, `1440p`, and `Source4K` profiles up to `60` FPS and `25 Mbps`.
- **Connection & Frame Adaptation (`StreamQualityController`):**
  - **Aspect-Ratio YUV420 Scaling (`scale_dimensions_to_pixel_limit`):** Preserves source aspect ratio within `effective_pixel_limit`, rounding dimensions down to even integers (`>= 64`). Any mid-stream resolution change increments `resolution_scalings` and triggers an immediate keyframe request.
  - **RTCP Loss Congestion & Recovery:** Receiver Report `fraction_lost >= 13` (`13/256 ≈ 5.08%`, `LOSS_CONGESTION_THRESHOLD_256`) reduces target bitrate to `4/5` (`-20%`). Sustained low loss `fraction_lost <= 5` (`5/256 ≈ 1.95%`, `LOSS_RECOVERY_THRESHOLD_256`) after `CONGESTION_RECOVERY_HOLDOFF_MS = 2,000 ms` steps bitrate up by `max(target / 20, 25_000)` bps per `ADAPTATION_TICK_INTERVAL_MS = 1,000 ms`.
  - **RTCP REMB Headroom:** Inbound `RtcpRemb` (`PT=206, FMT=15, "REMB"`) wire estimate is scaled by `4/5` (`REMB_HEADROOM_NUMERATOR / REMB_HEADROOM_DENOMINATOR`) with a `5,000 ms` TTL (`REMB_LIMIT_TTL_MS`).
  - **Static Screen Idle Framerate Reducer (`ConnectionEventFramerateReducer`):** When static screen idle duration reaches `CONNECTION_IDLE_FRAMERATE_REDUCE_MS = 15,000 ms`, effective framerate is capped to `IDLE_REDUCED_MAX_FRAMERATE = 20` FPS (matching `remoteSinkWantsMaxFramerate 20` and `Reduced framerate after 15000 ms`).

### 5.2 Video Codec Negotiation (`default_client_codecs`, `OP 0..=4`)

Client codec table advertised in `Voice Opcode 1 (SELECT_PROTOCOL)` (`chord_voice::default_client_codecs`):

| Codec (`name`) | Media Type (`type`) | Priority (`priority`) | Primary RTP Payload Type (`payload_type`) | RTX Payload Type (`rtx_payload_type`) | `encode` | `decode` | Clock Rate |
|---|---|---:|---:|---:|---|---|---:|
| `opus` | `audio` | `1000` | `120` (`RTP_OPUS_PAYLOAD_TYPE`) | — | — | — | `48,000 Hz` |
| `AV1` | `video` | `1000` | `101` (`RTP_AV1_PAYLOAD_TYPE`) | `102` (`RTP_AV1_RTX_PAYLOAD_TYPE`) | `true` | `true` | `90,000 Hz` |
| `H265` | `video` | `2000` | `103` (`RTP_H265_PAYLOAD_TYPE`) | `104` (`RTP_H265_RTX_PAYLOAD_TYPE`) | `true` | `true` | `90,000 Hz` |
| `H264` | `video` | `3000` | `105` (`RTP_H264_PAYLOAD_TYPE`) | `106` (`RTP_H264_RTX_PAYLOAD_TYPE`) | `true` | `true` | `90,000 Hz` |
| `VP8` | `video` | `4000` | `107` (`RTP_VP8_PAYLOAD_TYPE`) | `108` (`RTP_VP8_RTX_PAYLOAD_TYPE`) | `true` | `true` | `90,000 Hz` |

- **SSRC Allocation & Stabilization (`compute_stream_ssrcs`):**
  - `OP 2 READY` assigns base `audio_ssrc` (e.g., `17843` in Session 2, `9208` in Session 3) and `streams: [{type: "video", ssrc: audio_ssrc + 1, rtx_ssrc: audio_ssrc + 2, rid: "100", quality: 100, active: false}]` (`17844/17845` in Session 2, `9209/9210` in Session 3).
  - Once locked on initial `OP 2 READY`, `(video_ssrc, rtx_ssrc)`, RTP `sequence`, `rtx_sequence`, 90 kHz `timestamp`, and SRTP nonce counter remain stabilized across mid-stream codec switches (`RtpVideoPacketizer::reconfigure_codec`, e.g., `H264 <-> AV1`), source switches (`switch_video_source`), and `OP 4 SESSION_DESCRIPTION` renegotiations.
  - Go Live dedicated child sessions compute `rtc_channel_id = (u64::from_str(rtc_server_id) - 1).to_string()` (`compute_go_live_rtc_channel_id`) to prevent Discord close code `2001` (`stream-failed-to-start`).

### 5.3 RTP Framing & Video Packetization Rules (`chord_voice::rtp::video`, `chord_voice::media::rtp`)

- **Fixed RTP Header & RFC 5285 `0xBEDE` Extension (`RtpHeader`, `VideoRtpPacket`):**
  - Fixed header (12 B): `byte[0] = 0x90` (`V=2, P=0, X=1, CC=0` when extension present, `0x80` when absent), `byte[1] = (marker << 7) | (payload_type & 0x7F)`, `bytes[2..4] = sequence (u16 BE)`, `bytes[4..8] = timestamp (u32 BE, 90,000 Hz)`, `bytes[8..12] = ssrc (u32 BE)`.
  - Extension header (4 B + `words * 4` B): profile `0xBEDE` (One-Byte Header, RFC 5285) or `0x1000..=0x100F` (Two-Byte Header, RFC 8285).
  - Timestamp cadence at `VIDEO_CLOCK_RATE = 90,000 Hz`: `1,500` ticks per frame at `60 FPS` (`VIDEO_TICKS_60FPS`), `3,000` ticks at `30 FPS` (`VIDEO_TICKS_30FPS`), `6,000` ticks at `15 FPS`. All fragments of the same video frame share the identical RTP `timestamp`; `marker = true` (`M = 1`) is set strictly on the final packet of the frame, after which `timestamp` advances by `timestamp_delta`. During capture pauses, `video_ticks_for_elapsed_ms` advances the 90 kHz clock continuously to prevent WebRTC media timeout (`Close Code 2007`).
  - Maximum RTP video payload MTU: `MAX_VIDEO_RTP_PAYLOAD_LEN = 1200` bytes (`MIN_VIDEO_RTP_PAYLOAD_LEN = 16` bytes).
- **H.264 Packetization Rules (RFC 6184, `PT = 105`, `RTX PT = 106`):**
  - Input Annex-B bitstream (`00 00 01` or `00 00 00 01` start codes) is parsed into NAL units (`parse_h264_annex_b`). `forbidden_zero_bit` (`0x80`) must be `0`; input `nal_unit_type` (`hdr & 0x1F`) must be in `1..=23` (e.g., `5 = IDR`, `6 = SEI`, `7 = SPS`, `8 = PPS`, `9 = AUD`).
  - **Single NALU (`H264PacketizationKind::SingleNalu`):** A single NAL unit `<= 1200` bytes that is not aggregated with adjacent small NALUs is emitted directly as the RTP payload.
  - **STAP-A (`H264_NAL_STAP_A = 24`, `H264PacketizationKind::StapA`):** Consecutive small NAL units (such as `SPS` + `PPS`) whose combined wire size `1 + sum(2 + nalu.len()) <= 1200` bytes are aggregated into `[max_nri | 24][u16 len_be][nal_1][u16 len_be][nal_2]...`.
  - **FU-A (`H264_NAL_FU_A = 28`, `H264PacketizationKind::FuA`):** Any NAL unit `> 1200` bytes (such as a `1201+` byte IDR or P-slice) has its 1-byte NAL header stripped and its body split into chunks of `<= 1198` bytes (`max_payload_len - 2`), each prefixed with a 2-byte FU-A header:
    - `fu_indicator = (nal_hdr & 0xE0) | 28`
    - `fu_header = (S << 7) | (E << 6) | nal_type` (`S = 0x80` on first fragment, `E = 0x40` on last fragment; `S` and `E` are never both `1`).
- **AV1 Packetization Rules (RTP Payload Format for AV1, `PT = 101`, `RTX PT = 102`):**
  - Self-delimiting OBU stream (`parse_av1_obu_stream`, `obu_forbidden_bit == 0`) is packetized with a 1-byte **AV1 Aggregation Header** `[Z:1][Y:1][W:2][N:1][0:3]`:
    - `Z` (`0x80`, `AV1_AGGR_Z_CONTINUATION`): Set to `1` if the first OBU element in the packet is a continuation fragment of an OBU started in the previous packet.
    - `Y` (`0x40`, `AV1_AGGR_Y_FRAGMENTED`): Set to `1` if the last OBU element in the packet will be continued in the next packet.
    - `W` (bits `4..5`, `0..=3`): Number of OBU elements in the packet (`1..=3` omits the trailing LEB128 size field on the final element; `W = 0` prefixes every element with its LEB128 byte length, supporting `4+` aggregated OBUs).
    - `N` (`0x08`, `AV1_AGGR_N_NEW_CODED_VIDEO_SEQ`): Set to `1` on the first packet of a new coded video sequence (keyframe starting with `OBU_SEQUENCE_HEADER = 1`).
- **RFC 4588 RTX Retransmission (`encode_rtx`, `restore_rtx_bytes`):**
  - Retransmitted packets use `rtx_ssrc = video_ssrc + 1`, `rtx_payload_type` (`102` for AV1, `106` for H.264), and an independent monotonic `rtx_sequence`, prepending the 2-byte big-endian Original Sequence Number (`OSN`) to the original RTP payload while preserving the original `timestamp`, `marker`, and `0xBEDE` extension.
- **Two-Layer DAVE MLS v1 + SRTP Encryption (`DaveMlsEngine` / `SrtpContext`):**
  - When DAVE MLS v1 E2EE is active, video RTP payloads are first inner-encrypted with the sender's MLS ratchet key (`AES-128-GCM` 8-byte truncated tag + LEB128 nonce + supplemented ranges + `0xFAFA` magic marker, keeping H.264 NAL headers / AV1 aggregation headers unencrypted for SFU routing) and then outer-encrypted with SRTP (`aead_xchacha20_poly1305_rtpsize` or `aead_aes256_gcm_rtpsize` with 16-byte AEAD tag + 4-byte LE packet counter).

### 5.4 Opcodes `18..=22` Wire Contracts (Main Gateway Go Live & Voice Gateway)

Discord uses opcodes `18..=22` across two distinct WebSocket control planes:

#### A. Main Gateway Go Live Streaming Opcodes `18..=22` (`chord_voice::rtp::video`)

| Opcode | Constant / Name | Direction | Wire JSON Payload (`{"op": N, "d": ...}`) | Purpose |
|---:|---|---|---|---|
| `18` | `GW_OP_STREAM_CREATE` (`STREAM_CREATE`) | Client → Gateway | `{"type":"guild"\|"call","guild_id":"<id>"\|null,"channel_id":"<id>","preferred_region":"<region>"\|null}` | Initiates a Go Live screen share stream; triggers `STREAM_CREATE` and `STREAM_SERVER_UPDATE` dispatches |
| `19` | `GW_OP_STREAM_DELETE` (`STREAM_DELETE`) | Client → Gateway | `{"stream_key":"guild:<gid>:<cid>:<uid>"}` | Terminates an active Go Live stream |
| `20` | `GW_OP_STREAM_WATCH` (`STREAM_WATCH`) | Client → Gateway | `{"stream_key":"guild:<gid>:<cid>:<uid>"}` | Subscribes a viewer to a remote participant's Go Live stream |
| `21` | `GW_OP_STREAM_PING` (`STREAM_PING`) | Client → Gateway | `{"stream_key":"guild:<gid>:<cid>:<uid>"}` | Periodic heartbeat ping keeping an active or watched Go Live stream alive |
| `22` | `GW_OP_STREAM_SET_PAUSED` (`STREAM_SET_PAUSED`) | Client → Gateway | `{"stream_key":"guild:<gid>:<cid>:<uid>","paused":true\|false}` | Toggles pause state of the Go Live stream |

- **`STREAM_SERVER_UPDATE` Dispatch (`StreamServerUpdate`):** Inbound Main Gateway event `{"d":{"stream_key":"guild:<gid>:<cid>:<uid>","server_id":"<id>","endpoint":"c-<region>.discord.media:2053","token":"REDACTED"}}` used by `VoiceSessionDriver::start_go_live_session` to open the dedicated Go Live RTC WebSocket connection (`wss://<endpoint>/?v=9`).

#### B. Voice Gateway Opcodes `18..=22` (`chord_voice::{VoiceOp, soundboard_protocol, dave, session}`)

| Opcode | Constant / `VoiceOp` | Direction | Wire Payload Format | Purpose |
|---:|---|---|---|---|
| `18` | `VOICE_OP_SOUNDBOARD_SOUND` (`SOUNDBOARD_SOUND`) | Client → Voice WS | `{"op":18,"d":{"sound_id":"<id>","source_guild_id":"<gid>","volume":0.75,"emoji_id":...,"emoji_name":...,"ssrc":9208}}` | Signals soundboard SFX playback on the active audio SSRC |
| `19` | `VOICE_OP_SPEAKING_SOUNDBOARD` (`SPEAKING`) | Voice WS → Client | `{"op":19,"d":{"user_id":"<uid>","ssrc":9208,"speaking":2,"sound_id":"<id>","source_guild_id":"<gid>","volume":0.80}}` | Broadcasts remote soundboard/soundshare speaking activity (`SOUND_SHARE = 1 << 1`) |
| `20` | `VoiceOp::Unknown(20)` | Reserved | Unassigned on Voice WS (rejected by `VoiceOp19SpeakingSoundboardEvent::parse` with `UnsupportedVoiceOpcode(20)`) | Reserved gap between soundboard and DAVE opcodes |
| `21` | `OP_PREPARE_TRANSITION` (`SECURE_FRAMES_PREPARE_PROTOCOL_TRANSITION`) | Voice WS → Client | `{"op":21,"d":{"transition_id":14,"protocol_version":1}}` (`DavePrepareTransition`) | Instructs client to prepare DAVE MLS protocol transition; client replies with `Op 23` (`{"op":23,"d":{"transition_id":14}}`) |
| `22` | `OP_EXECUTE_TRANSITION` (`SECURE_FRAMES_EXECUTE_TRANSITION`) | Voice WS → Client | `{"op":22,"d":{"transition_id":14}}` (`DaveExecuteTransition`) | Instructs client to atomically activate the prepared DAVE MLS transition epoch |

### 5.5 Keyframe Request Opcodes & Media Signaling (`RTCP PLI`, `RTCP FIR`, `OP 12`, `OP 15`)

- **WebRTC Signaling Keyframe & Stream Control Opcodes (`chord_voice::session`):**
  - **`Voice Opcode 12 (VIDEO)` (`VoiceVideo`):** Outbound frame `{"op":12,"d":{"audio_ssrc":9208,"video_ssrc":9209,"rtx_ssrc":9210,"streams":[{"type":"screen","rid":"100","quality":100,"ssrc":9209,"rtx_ssrc":9210,"active":true,"max_bitrate":3500000,"max_framerate":30,"max_resolution":{"type":"fixed","width":1280,"height":720}}]}}`. Sent on Go Live start and on resolution/source updates.
  - **`Voice Opcode 15 (MEDIA_SINK_WANTS)` (`MediaSinkWants`):** Bidirectional quality/pixel-budget request `{"op":15,"d":{"any":100,"9209":50,"pixelCounts":409920}}`. When a sink transitions from `0` to `> 0` or requests a resolution tier change, the encoder emits a fresh keyframe.
- **RTCP Payload-Specific Feedback Keyframe Request Opcodes (`chord_voice::rtcp`, `RtcpKeyframeController`, `RtpVideoPacketizer`):**

| Mechanism | Payload Type (`PT`) | Feedback Type (`FMT`) | Clear Wire Length | Wire Layout (Big-Endian) | Behavior in `RtcpKeyframeController` & `RtpVideoPacketizer` |
|---|---:|---:|---:|---|---|
| **RTCP PLI** (Picture Loss Indication, RFC 4585 §6.3.1) | `PT=206` (`RTCP_PSFB`) | `FMT=1` (`RTCP_FMT_PLI`) | `12 B` (`RTCP_PLI_LEN`) | `[0x81][206][u16 len=2][u32 sender_ssrc][u32 media_ssrc]` | Sets `keyframe_requested = true`, increments `inbound_pli_count`, records `KeyframeRequestKind::Pli` |
| **RTCP FIR** (Full Intra Request, RFC 5104 §4.3.1) | `PT=206` (`RTCP_PSFB`) | `FMT=4` (`RTCP_FMT_FIR`) | `20 B` (`RTCP_FIR_LEN`, 1 entry) | `[0x84][206][u16 len=4][u32 sender_ssrc][u32 media_ssrc=0][u32 target_ssrc][u8 seq][24-bit 0]` | Deduplicates per `(sender_ssrc, target_ssrc)` 8-bit `sequence_number` (`0..=255` wrap-around); new seq sets `keyframe_requested = true`, increments `inbound_fir_count` |
| **RTCP Generic NACK** (RFC 4585 §6.2.1) | `PT=205` (`RTCP_RTPFB`) | `FMT=1` (`RTCP_FMT_NACK`) | `12 + 4*N B` | `[0x81][205][u16 len][u32 sender_ssrc][u32 media_ssrc][u16 PID, u16 BLP]...` | Expands `(PID, BLP)` bitmask into lost RTP sequence numbers (up to `MAX_NACK_SEQUENCES = 128`) for RFC 4588 RTX retransmission |
| **RTCP REMB** (Receiver Estimated Max Bitrate) | `PT=206` (`RTCP_PSFB`) | `FMT=15` (`RTCP_FMT_REMB`) | `20 + 4*N B` | `[0x8F][206][u16 len][u32 sender_ssrc][0][b"REMB"][u8 num_ssrc][6b exp \| 18b mantissa][u32 ssrc]...` | Decodes `bitrate_bps = mantissa << exponent`; scales by `4/5` headroom in `StreamQualityController` |

- **Automatic Parameter Set Injection & Error Recovery:**
  - When `RtpVideoPacketizer::is_keyframe_requested()` is true, `packetize_h264_frame` automatically prepends cached `SPS (NAL 7)` + `PPS (NAL 8)` (`STAP-A`) to the next `IDR (NAL 5)` frame if absent, and `packetize_av1_obus` prepends cached `OBU_SEQUENCE_HEADER (type = 1)` with aggregation bit `N = 1` (`0x08`), then clears the pending keyframe flag.
  - Recoverable stream close codes `2000` (`VOICE_CLOSE_CODE_STREAM_INTERNAL_ERROR`) and `2007` (`VOICE_CLOSE_CODE_WEBRTC_MEDIA_TIMEOUT`) classify as `VoiceCloseDisposition::ResumePreservingMediaState`: `VoiceSessionDriver::recover_from_close_code` emits `Voice Opcode 7 (RESUME)` while preserving stabilized SSRCs, RTP `sequence`/`timestamp`, and flags an immediate keyframe upon `OP 9 RESUMED`.

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
