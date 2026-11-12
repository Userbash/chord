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
