# Platform Privacy Guardrails (D-08) — WebRTC IP Leakage Prevention, MediaSession Key Bindings Isolation, Auto-Play Media Audio Suppression, and Native Privacy Checklist

Normative specification for **Task D-08** (`Platform Privacy Guardrails`) in the Platform Shell (`chord-platform::privacy_guardrails`). Synthesized from clean-room empirical captures (`docs/re/main-arch.md`, `docs/re/main-arch.json`, `docs/re/voice-deep.md`, `docs/re/backend-ipc.md`, `docs/re/rest-d02.md`) and reference privacy configuration patterns (`docs/vesktop-analysis.md`, `docs/feature-backlog.md` row `D-08`).

---

## 0. Quantitative Oracle Summary

| Metric / Constant | Value | Source Oracle |
|---|---|---|
| Legacy Electron Bootstrap Module | `app/bundle.js:10` (`#23151`, `@15212..25870`) | `docs/re/main-arch.md` §1.2 |
| Base Disabled Chromium Features (`DISABLED_CHROMIUM_FEATURES_BASE`) | **6** (`WinRetrieveSuggestionsOnlyOnDemand`, `HardwareMediaKeyHandling`, `MediaSessionService`, `UseEcoQoSForBackgroundProcess`, `IntensiveWakeUpThrottling`, `AllowAggressiveThrottlingWithWebSocket`) | `app/bundle.js:10` (`#23151`, `@15800..16000`) |
| macOS Pre-Darwin 24 Disabled Features (`DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24`) | **7** (`ScreenCaptureKitMac*`, `WarmScreenCaptureSonoma`, `UseSCContentSharingPicker`) | `app/bundle.js:10` (`#23151`) |
| Linux Disabled Chromium Feature | **1** (`WaylandWpColorManagerV1`) | `app/bundle.js:10` (`#23151`, `@15800..16000`) |
| MediaSession Hijack Features Disabled by Default | **2** (`HardwareMediaKeyHandling`, `MediaSessionService`) | `app/bundle.js:10` (`#23151`), `docs/vesktop-analysis.md` |
| WebRTC IP Handling Policy Modes (`WebRtcIpHandlingPolicy`) | **4** (`default`, `default_public_and_private_interfaces`, `default_public_interface_only`, `disable_non_proxied_udp`) | Reference Code: Privacy Config (`webPreferences.webRTCIPHandlingPolicy`) & Native UDP Voice (`docs/re/voice-deep.md` §2.1) |
| Voice UDP IP Discovery Packet Length | **74 B** (`type=1` req / `type=2` resp, 64 B ASCII address + `u16` BE port `@72`) | `docs/re/voice-deep.md` §2.1 |
| Voice UDP Media Socket Rotation Cadence | **60,080 ms** (`+60.08 s`) | `docs/re/voice-deep.md` §2.5 |
| Blocked Telemetry & Crash Collector Domains | `dc-telemetry.net` (`DnsReporter` `#65445`), `o64374.ingest.sentry.io` (`#62457`), `/api/v9/science` | `docs/re/main-arch.json` §7–§8, `docs/re/rest-d02.md` |
| Mandatory Native Privacy Checklist Controls | **12** (`D08-CHK-01` .. `D08-CHK-12`) | `docs/feature-backlog.md` (`D-08`) |

---

## 1. WebRTC IP Leakage Prevention Policy

### 1.1 Four Canonical WebRTC IP Handling Policies (`WebRtcIpHandlingPolicy`)

To prevent accidental disclosure of RFC 1918 LAN IPv4 addresses (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), RFC 4193 IPv6 Unique Local Addresses (`fc00::/7`), link-local interfaces (`169.254.0.0/16`, `fe80::/10`), VPN virtual adapters (`tun0`, `wg0`), or non-proxied host IP addresses during ICE candidate gathering and Voice UDP IP Discovery (`docs/re/voice-deep.md` §2.1), the Platform Shell enforces four canonical policy tiers:

| Policy Wire Identifier | Enum Variant | Local RFC 1918 / ULA Candidates | Public STUN / 74-Byte UDP Discovery | Non-Proxied Direct UDP | Target Security Profile |
|---|---|---|---|---|---|
| `"default"` | `Default` | Permitted (unless masked by `.local` mDNS) | Permitted | Permitted | Legacy unhardened baseline (rejected by strict D-08 checklist) |
| `"default_public_and_private_interfaces"` | `DefaultPublicAndPrivateInterfaces` | Default route private + public interface only (no enumerate-all-NICs) | Permitted | Permitted | Multi-NIC workstation isolation |
| `"default_public_interface_only"` | `DefaultPublicInterfaceOnly` | **Blocked** (zero RFC 1918 / link-local / ULA / loopback candidates) | Permitted via default public route only | Permitted | **Recommended Native Client Default** |
| `"disable_non_proxied_udp"` | `DisableNonProxiedUdp` | **Blocked** | **Blocked** unless routed via configured proxy/TURN | **Blocked** (forces proxy/TURN or aborts direct UDP) | High-privacy / VPN-killswitch / Tor / SOCKS5 proxy mode |

### 1.2 Candidate Filtering & 74-Byte UDP IP Discovery Scrubbing (`voice-deep.md` §2.1)

1. **ICE & Socket Candidate Classification (`IpAddrPrivacyClass`)**:
   - `Loopback`: `127.0.0.0/8`, `::1` — always stripped from outbound signaling (`OP 1 SELECT_PROTOCOL` and ICE SDP candidates).
   - `LinkLocal`: `169.254.0.0/16`, `fe80::/10` — always stripped from outbound signaling.
   - `PrivateLan`: RFC 1918 (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), RFC 6598 CGNAT (`100.64.0.0/10`), and RFC 4193 IPv6 ULA (`fc00::/7`, `fd00::/8`) — blocked under `DefaultPublicInterfaceOnly` and `DisableNonProxiedUdp`.
   - `MdnsObfuscated`: `<uuid>.local` (RFC 8828 WebRTC mDNS) — allowed as an opaque local placeholder when `enable_mdns_obfuscation == true`, never exposing raw MAC/EUI-64 or LAN subnets.
   - `PublicUnicast`: Routable public IPv4/IPv6 address — permitted under `DefaultPublicInterfaceOnly` (via the OS default route) and blocked under `DisableNonProxiedUdp` when `is_proxied == false`.
2. **Discord 74-Byte UDP IP Discovery Guard (`chord_voice::udp::DiscoveryPacket`)**:
   - In `docs/re/voice-deep.md` §2.1, the client sends a 74-byte UDP request (`[u16 type=1][u16 len=0x0046][u32 ssrc][64B zero padding]`) to the Discord Voice Media Server (`104.29.147.145:19309`), receives the reflected public endpoint (`type=2`), and transmits it in Voice Gateway **Opcode 1 (`SELECT_PROTOCOL`)**: `{"protocol":"udp","data":{"address":"<ip>","port":<port>,"mode":"aead_aes256_gcm_rtpsize"}}`.
   - Under `DisableNonProxiedUdp` without an active proxy/tunnel, direct UDP socket binding and 74-byte IP Discovery transmission are **vetoed** before any packet leaves the host interface (`D08PrivacyError::NonProxiedUdpBlocked`).
   - Under `DefaultPublicInterfaceOnly`, `SELECT_PROTOCOL` validation rejects any `address` that resolves to a private LAN, loopback, link-local, or unspecified (`0.0.0.0`, `::`) address, preventing local subnet leakage to the remote Voice Gateway.
3. **Diagnostic & Log IP Redaction (`<PUB_IP>`)**:
   - All client logs, diagnostic windows (`R6-05`), RTC Internals exports, and crash reports automatically mask public IPv4/IPv6 literals as `"<PUB_IP>"` and private LAN addresses as `"<LAN_IP>"`, matching the clean-room sanitization standard in `docs/re/voice-deep.md` §0.

---

## 2. MediaSession Key Bindings Isolation

### 2.1 Prevention of OS Hardware Media Key Hijacking

In Electron/Chromium builds (`docs/re/main-arch.md` §1.2 Step 6, `app/bundle.js:10` `#23151` `@15800..16000`) and reference desktop shells (`Vesktop` privacy/media configuration), playing any inline chat video, animated GIF-video (`mp4`/`webm`), soundboard clip, or voice notification cue causes the rendering engine to register an active OS media session:
- **Linux**: claims `org.mpris.MediaPlayer2.chromium.instance*` or `org.mpris.MediaPlayer2.discord` on the session D-Bus;
- **Windows**: claims `SystemMediaTransportControls` (`SMTC`);
- **macOS**: claims `MPNowPlayingInfoCenter` / `MPRemoteCommandCenter`.

When this occurs, pressing physical keyboard media keys (`XF86AudioPlay`, `XF86AudioPause`, `XF86AudioNext`, `XF86AudioPrev`, `XF86AudioStop`) hijacks playback away from the user's actual music player (e.g., Spotify, MPD, Rhythmbox, Apple Music) and leaks the filename/title of the viewed Discord attachment into the OS lock-screen / desktop notification tray widget.

### 2.2 Platform Shell Isolation Contract (`MediaSessionIsolationPolicy`)

1. **Mandatory Disabled Engine Features (`DISABLED_CHROMIUM_FEATURES_BASE`, `app/bundle.js` `#23151`)**:
   - `"HardwareMediaKeyHandling"`: MUST be disabled unconditionally across `linux`, `win32`, and `darwin`.
   - `"MediaSessionService"`: MUST be disabled unconditionally so inline media elements and WebAudio/CPAL notification sinks never publish metadata to OS media transport controls or lock-screen widgets.
   - Companion baseline flags from `#23151`: `"WinRetrieveSuggestionsOnlyOnDemand"`, `"UseEcoQoSForBackgroundProcess"`, `"IntensiveWakeUpThrottling"`, `"AllowAggressiveThrottlingWithWebSocket"`, plus platform-specific `"WaylandWpColorManagerV1"` on Linux and `"ScreenCaptureKitMac*"` on macOS `< 24`.
2. **D-Bus MPRIS Consumer-Only Isolation (`chord-platform::spotify_mpris`)**:
   - The native client operates strictly as a **read-only D-Bus MPRIS consumer** of `org.mpris.MediaPlayer2.spotify` (`DBUS_BUS_NAME = "org.mpris.MediaPlayer2.spotify"`, `PropertiesChanged` on `org.mpris.MediaPlayer2.Player`) for Gateway Opcode 3 `LISTENING` presence (`spotify_mpris.rs`).
   - By default (`allow_os_media_transport_registration = false`), the client MUST NOT call `org.freedesktop.DBus.RequestName` for any `org.mpris.MediaPlayer2.*` service name, MUST NOT export `/org/mpris/MediaPlayer2` player objects, and MUST NOT intercept hardware media keys (`Play`, `Pause`, `Next`, `Previous`, `Stop`).
3. **Scoped Voice Keybind Routing vs Media Keys**:
   - Only explicitly user-bound voice shortcuts (`ToggleMute`, `ToggleDeafen`, `PushToTalk`, `ToggleStreamerMode`) are routed to the Voice Engine.
   - Hardware media transport keys (`MediaPlayPause`, `MediaNextTrack`, `MediaPrevTrack`, `MediaStop`) are rejected by the keybind router unless the user explicitly enables an isolated in-app media focus override while a standalone fullscreen stream/video modal has active keyboard focus (`MediaKeyRoutingDecision::PassThroughToOs`).

---

## 3. Auto-Play Media Audio Suppression

### 3.1 Evolution from Legacy Electron `--autoplay-policy` to Native Audio Suppression

In legacy Discord Desktop 1.0.162 (`docs/re/main-arch.md` §1.2 Step 5, `app/bundle.js:10` `#23151` `@16000..16300`), the bootstrapper appends `--autoplay-policy=no-user-gesture-required` so WebRTC voice `<audio>` elements and UI sound cues can play without a DOM click in Chromium. However, a global `no-user-gesture-required` policy creates a severe privacy and UX hazard in hybrid/WebView and native chat views:
- Unsolicited inline video embeds (`<video>`, YouTube/Twitch iframe embeds with `?autoplay=1`, custom activity iframes) can suddenly blast audio upon channel switch or scroll hydration;
- Malicious or tracking media embeds can probe audio output timing or start hidden audio contexts without user consent.

### 3.2 Native Source-Classified Auto-Play Suppression Engine (`AutoplaySuppressionPolicy`)

The Platform Shell replaces the coarse global flag with a **source-classified auto-play audio policy** (`Default = DocumentUserActivationRequired` for chat/embed media, with explicit separation of verified voice call RTP streams and user-configured UI notification cues):

| Media Source Category (`MediaPlaybackSource`) | Auto-Play Video Frames | Auto-Play Audio Track | Unmute / Audio Activation Requirement | Privacy & Acoustic Guardrail |
|---|---|---|---|---|
| `InlineAttachmentVideo` (`.mp4`, `.webm`, `.mov` in chat) | Permitted only when `muted == true` and viewport-visible | **Suppressed (`muted = true`)** | Explicit user gesture (`Click` / `Space` / `Enter` on play/unmute control) | Prevents sudden audio spikes when scrolling channels |
| `AnimatedGifVideo` (`GIFV` / Tenor / Giphy `.mp4`/`.webm`) | Permitted when `animate_on_focus == true` and `muted == true` | **Permanently Suppressed** (audio track stripped/zeroed) | Never outputs audio | Eliminates hidden audio tracks in disguised GIFV containers |
| `ExternalEmbedIframe` (YouTube, Twitch, External URL) | Blocked from auto-loading until click-to-load consent | **Suppressed** (`?autoplay=1` stripped or forced `autoplay=0&mute=1`) | Explicit user click on embed preview card | Prevents third-party tracking beacons & unsolicited audio before user click |
| `VoiceMessageAttachment` (OGG Opus flags `8192`) | N/A | **Suppressed** on load | Explicit user click on waveform Play button | Prevents voice messages from auto-playing on channel open |
| `SoundboardClipPreview` (Voice Opcode 18 / chat preview) | N/A | Allowed in active voice call (if not deafened/muted); **Suppressed** in text preview without click | Explicit user gesture or active voice channel membership + `!suppress_soundboard` | Respects per-user/global soundboard volume (`0..=200%`) and deafen state |
| `ActiveVoiceCallRtc` (Joined Voice Channel / Go Live Stream) | Allowed when user clicked "Watch Stream" | Allowed after explicit "Join Voice" user gesture (unless self-deafened) | Joining the voice channel counts as the explicit session activation gesture | Never auto-joins or plays remote RTC audio without an active voice session |
| `NotificationSoundCue` (`SoundCue::Mention`, `UserJoin`, etc.) | N/A | Governed by D-06 (`!disable_all_sounds && !user_dnd && !streamer_mode`) | Authorized by D-06 `D06ClientNotificationDefaults` | Zero child processes (`cpal` in-memory PCM synthesis, `#![forbid(unsafe_code)]`) |

---

## 4. Comprehensive Platform Privacy Checklist (`D08-CHK-01` .. `D08-CHK-12`)

Every native client release (`chord-gpui` / `chord-platform`) is validated against the 12-point **Platform Privacy Checklist** (`D08PrivacyChecklistReport`) to guarantee zero user IP leakage, zero OS media-key hijacking, zero unsolicited auto-play audio, and zero unauthorized telemetry transmission:

| ID | Category | Control Requirement | Verification Rule | Severity |
|---|---|---|---|---|
| `D08-CHK-01` | WebRTC IP Privacy | Enforce `DefaultPublicInterfaceOnly` or `DisableNonProxiedUdp` (never `Default` in hardened builds) | `policy ∈ {DefaultPublicInterfaceOnly, DisableNonProxiedUdp}` | `P0` |
| `D08-CHK-02` | WebRTC IP Privacy | Strip RFC 1918, RFC 6598 CGNAT, RFC 4193 ULA, loopback, and link-local addresses from outbound `SELECT_PROTOCOL` and ICE candidates | `leaked_private_candidates == 0` | `P0` |
| `D08-CHK-03` | WebRTC IP Privacy | Block non-proxied 74-byte UDP IP Discovery packets when `DisableNonProxiedUdp` or VPN killswitch is active | `unproxied_udp_blocked_when_required == true` | `P0` |
| `D08-CHK-04` | WebRTC IP Privacy | Mask public IP addresses as `<PUB_IP>` and LAN IPs as `<LAN_IP>` across all logs, diagnostics (`R6-05`), and crash dumps | `diagnostic_ip_redaction_enabled == true` | `P0` |
| `D08-CHK-05` | MediaSession Isolation | Disable `HardwareMediaKeyHandling` and `MediaSessionService` in all runtime feature sets | `hardware_media_keys_disabled && media_session_service_disabled` | `P0` |
| `D08-CHK-06` | MediaSession Isolation | Prohibit registering `org.mpris.MediaPlayer2.*`, Windows `SMTC`, or macOS `MPNowPlayingInfoCenter` for chat attachments or notification cues | `!client_registers_os_media_player && mpris_consumer_only` | `P0` |
| `D08-CHK-07` | Auto-Play Suppression | Enforce `DocumentUserActivationRequired` for all inline videos, external embeds, and voice messages (`muted = true` until explicit user gesture) | `autoplay_policy == DocumentUserActivationRequired` | `P0` |
| `D08-CHK-08` | Auto-Play Suppression | Sanitize external embed URLs by stripping `autoplay=1` and blocking third-party iframe loads until explicit click-to-load consent | `external_embed_click_to_load && autoplay_query_stripped` | `P0` |
| `D08-CHK-09` | Telemetry Protection | Block DNS telemetry beacons to `dc-telemetry.net` (`DnsReporter` `#65445`) and REST analytics to `/api/v9/science`, `/api/v9/metrics`, `/api/v9/track` | `dns_telemetry_blocked && science_endpoints_blocked` | `P0` |
| `D08-CHK-10` | Telemetry Protection | Disable automatic Crashpad / Sentry minidump uploads to `o64374.ingest.sentry.io` (`uploadToServer = false`) | `sentry_crashpad_upload_disabled == true` | `P0` |
| `D08-CHK-11` | Artifact & Secret Privacy | Redact `token`, `secret_key`, `session_id`, `media_session_id`, `analytics_token`, `user_settings_proto`, and OS audio device `label` fields in `Debug`/logs | `secret_and_device_label_redaction_enabled == true` | `P0` |
| `D08-CHK-12` | Streamer Mode Guardrails | Automatically mask personal identifiers, invite codes, tokens, and desktop notifications when Streamer Mode (`R5-10`) is active | `streamer_mode_pii_protection_enabled == true` | `P0` |
