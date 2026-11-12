# Rust Client Feature Catalog (Cleanroom Reverse-Engineering Specification)

Each row documents: **ID** -> Feature name -> Purpose -> Operational mechanics (cleanroom reverse-engineered facts) -> Specification source -> Architectural plan phase (`PLAN.md`). Phases: P0 spikes -> P1 network core -> P2 data & state -> P3 UI -> P4 voice -> P5 video/streaming -> P6 platform -> P7 polish.

## GW — Gateway Event Stream

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-GW-01 | Gateway Connection | Real-time bidirectional event transport | `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream`; server may provide `resume_gateway_url` (e.g. `us-east1-c/d`) | `gateway-protocol.md` §1 | P1 |
| F-GW-02 | HELLO & Identify | Initial session negotiation and authentication | Opcode 10 HELLO `{heartbeat_interval: 41250}`; Opcode 2 IDENTIFY (~943 B) with token and client properties | `gateway-protocol.md` §4 | P1 |
| F-GW-03 | Heartbeat & ACK | Keepalive and RTT latency measurement | Opcode 1 heartbeat sent every 41.25s (~90 B); server responds with Opcode 11 ACK | `gateway-protocol.md` §4 | P1 |
| F-GW-04 | Session Resume | Seamless reconnect without losing state events | Opcode 6 RESUME (~173 B) with `session_id` and last received sequence `s` -> dispatches `RESUMED` | `gateway-protocol.md` §4 | P1 |
| F-GW-05 | Gateway Reconnect | Clean connection recovery on server/client drop | Opcode 7 RECONNECT -> close socket -> establish new socket + issue RESUME (~2.9s backoff) | `gateway-protocol.md` §4 | P1 |
| F-GW-06 | Event Dispatch Router | Strongly typed dispatch event multiplexing | Opcode 0 (21 core types): READY, READY_SUPPLEMENTAL, RESUMED, THREAD_LIST_SYNC, SESSIONS_REPLACE, CHANNEL_INFO, GUILD_MEMBER_LIST_UPDATE, GUILD_MEMBERS_CHUNK, PRESENCE_UPDATE, MESSAGE_CREATE, MESSAGE_UPDATE, MESSAGE_ACK, VOICE_STATE_UPDATE, VOICE_CHANNEL_*, SOUNDBOARD_SOUNDS, MESSAGE_POLL_VOTE_ADD, TYPING_START, MESSAGE_REACTION_ADD, USER_SETTINGS_PROTO_UPDATE | `gateway-protocol.md` §4 | P1 |
| F-GW-07 | Lazy Member Subscriptions | Bandwidth conservation for large servers | Opcode 37 SUBSCRIPTIONS: visible ranges `[[0, 99]]` for guild member roster; dynamic viewport updates on scroll | `gateway-protocol.md` §4, `gateway-schema.md` §4 | P2 |
| F-GW-08 | QoS Connection Telemetry | Network connection quality reporting | Opcode 40 (`ver: 31`) periodic reports (~41s interval) | `gateway-protocol.md` §4 | P1 |
| F-GW-09 | Channel Info Request | On-demand retrieval of channel metadata | Opcode 43 REQUEST_CHANNEL_INFO -> dispatches CHANNEL_INFO | `gateway-protocol.md` §4 | P2 |
| F-GW-10 | Presence Updates | Client status management | Opcode 3 PRESENCE update; supports online, idle, dnd, and invisible states | `gateway-protocol.md` §4 | P2 |
| F-GW-11 | Voice Channel State | Joining and leaving voice channels | Opcode 4 VOICE_STATE_UPDATE -> dispatches VOICE_SERVER_UPDATE with RTC signaling endpoint | `voice-deep.md` §1 | P4 |

## ENC — Transport Codecs & Encodings

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-ENC-01 | ETF Binary Codec | Binary Erlang External Term Format parser | 11 wire tags; big-int: tags 110/111, length byte, sign flag before bytes, LE bytes; snowflake string formatting; nil -> None | `gateway-protocol.md` §2 | P1 |
| F-ENC-02 | zstd Stream Decompressor | Real-time payload decompression | Shared decompression context per connection; magic bytes `28 B5 2F FD` present only in frame 0; streaming decode | `gateway-protocol.md` §3 | P1 |
| F-ENC-03 | Event Ordering & Deduplication | State machine consistency | Monotonic sequence number `s` on every dispatch; gapless sequencing; sequence continuity across RESUME | `gateway-protocol.md` §4 | P1 |

## AUTH — Authentication & Session Management

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-AUTH-01 | Email/Password & MFA Login | Initial user authentication | REST login flow (`POST /auth/login`), captcha handling, TOTP and SMS MFA challenges | `rest-d02.md` §6 | P1 |
| F-AUTH-02 | Secure Token Vault | Secure credential storage | System Keyring (Secret Service / KWallet / Keychain); zero plaintext tokens on disk; log redaction | Security Policy | P1 |
| F-AUTH-03 | Session Persistence | Persistent client state | In-memory storage of `session_id`, `resume_gateway_url`, and `auth_session_id_hash`; automatic fast resume | `gateway-schema.md` §3 | P1 |
| F-AUTH-04 | Account Switcher & Logout | Multi-account management | Multi-token storage, secure cache invalidation, clean gateway reconnection | Project Architecture | P6 |
| F-AUTH-05 | WebAuthn Security Keys | FIDO2 hardware key authentication | WebAuthn registration and authentication via `/users/@me/mfa/webauthn/credentials` | `main-arch.md` §4.2 | P7 |

## API — REST v9 Client Surface

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-API-01 | REST API Client Engine | Non-gateway API interactions | Base URL `https://discord.com/api/v9`; 6 authentication and fingerprint headers; HTTP 200/204/304/404/429 parsing | `rest-d02.md` §2, §6 | P1 |
| F-API-02 | Client Super Properties | Identifying client properties header | `X-Super-Properties` base64 JSON payload with 22 fields; configurable; honest native values | `rest-d02.md` §6.2 | P1 |
| F-API-03 | Message Dispatching | Sending text and attachments | `POST /channels/{id}/messages` with multipart/form-data for media payloads | `rest-d02.md` §3–4 | P3 |
| F-API-04 | Message Acknowledgements | Marking channels as read | `POST /channels/{id}/messages/{id}/ack` with `{token, last_viewed}` payload | `rest-d02.md` §4 | P2 |
| F-API-05 | Message Reactions | Interacting with emoji reactions | `PUT` and `DELETE` on `/channels/{id}/messages/{id}/reactions/{emoji}/@me` | `rest-d02.md` §2 | P3 |
| F-API-06 | Edit & Delete Messages | Modifying message history | `PATCH` and `DELETE` on `/channels/{id}/messages/{id}` | `rest-d02.md` §8.2 | P3 |
| F-API-07 | History & Pinned Messages | Retrieving channel history | `GET /channels/{id}/messages`, `GET /channels/{id}/messages/pins` | `rest-d02.md` §2 | P2 |
| F-API-08 | Full-Text Message Search | Locating messages across servers | `GET /guilds/{id}/messages/search` with 45 query filters (`author_id`, `mentions`, `has`, `link`, `embed`, etc.) | `rest-d02.md` §3 | P3 |
| F-API-09 | Thread Management | Thread lifecycle operations | `GET /channels/{id}/threads/search`, `THREAD_LIST_SYNC`, `THREAD_CREATE`, `THREAD_UPDATE` | `rest-d02.md` §2, `gateway-protocol.md` §4 | P3 |
| F-API-10 | User Profiles | User card data retrieval | `GET /users/{id}/profile` (biography, badges, server roles, connected accounts) | `rest-d02.md` §2 | P3 |
| F-API-11 | User Settings Synchronization | Synchronizing client settings | `GET` and `PATCH` on `/users/@me/settings-proto/{num}` containing serialized protobuf payloads | `rest-d02.md` §4 | P2 |
| F-API-12 | Rate Limit Handling | Backoff and quota compliance | HTTP 429 parsing: dynamic `Retry-After`, `X-RateLimit-*` headers, exponential backoff | `rest-d02.md` §2 | P1 |
| F-API-13 | Extended Ecosystem Endpoints | Parity surface | Quests, promotions, entitlements, library, and collectibles catalog endpoints | `rest-d02.md` §2 | P6 |
| F-API-14 | Telemetry Opt-Out Policy | User privacy enforcement | Outbound requests to `POST /science` and `POST /beaker` blocked by default | `rest-d02.md` §2 | P1 |

## DATA — State Machine & Local Storage

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-DATA-01 | READY Event State Ingestion | Bootstrap initial client state | 40 top-level payload keys: `guilds[]`, channels, roles, emojis, private channels, relationships, users, read states | `gateway-protocol.md` §5, `gateway-schema.md` §3 | P2 |
| F-DATA-02 | Relational Local Storage | Fast startup and offline access | SQLite database with WAL journaling: guilds, channels, roles, users, cached messages, and permissions | Architectural Plan | P2 |
| F-DATA-03 | Lazy Member Roster State | Handling massive member lists | `GUILD_MEMBER_LIST_UPDATE` dispatch ops (SYNC, INSERT, UPDATE, DELETE); `GUILD_MEMBERS_CHUNK` paging | `gateway-schema.md` §4 | P2 |
| F-DATA-04 | Unread State & Mentions | Notification badges | 157 read state records from READY; incremental updates via `MESSAGE_ACK`; mention count tracking | `gateway-protocol.md` §5 | P2 |
| F-DATA-05 | User Settings Model | Unified settings storage | `settings-proto` decoding into typed settings model (appearance, voice, notifications, privacy) | `gateway-schema.md` §3 | P2 |
| F-DATA-06 | Local Message Cache | Seamless chat scrolling | Local message store, bidirectional pagination, TTL caching policies, channel/timestamp indices | Architectural Plan | P2 |
| F-DATA-07 | Relationships & Friendships | Friends roster and blocked lists | READY `relationships[]` parsing, friend request dispatch events, user blocking rules | `gateway-protocol.md` §5 | P2 |
| F-DATA-08 | Voice Channel Presence State | Call rosters in channel tree | Real-time tracking of participants via `VOICE_STATE_UPDATE` and `CHANNEL_STATUS_UPDATE` | `gateway-protocol.md` §4 | P4 |
| F-DATA-09 | Multi-Device Session Roster | Active user session awareness | `SESSIONS_REPLACE` dispatch event tracking active desktop, mobile, and web sessions | `gateway-protocol.md` §4 | P6 |

## MSG — Messaging & Interaction

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-MSG-01 | Rich Markdown Rendering | User text communication | 23 markdown rules: bold, italic, underline, strikethrough, code blocks, syntax highlighting, spoiler tags, blockquotes, user/channel mentions | `i18n-deep.md` §3 | P3 |
| F-MSG-02 | File Attachments | Sharing files and media | Chunked file uploads with progress monitoring; image/video/audio in-line players | `rest-d02.md` §4 | P3 |
| F-MSG-03 | Message Quote Replies | Replying in context | Reference message quoting, author mention toggle, jump to referenced message | `frontend-d03.md` | P3 |
| F-MSG-04 | Emoji Reactions | Expressive feedback | Emoji reaction picker, reaction count badges, list of reacting users | `frontend-d03.md` §2.3 | P3 |
| F-MSG-05 | Message Actions Context Menu | Managing sent messages | Message context menu: edit, delete, copy text, pin, create thread, mark unread | `frontend-d03.md` §5.4 | P3 |
| F-MSG-06 | Real-Time Typing Indicators | Live chat awareness | `TYPING_START` dispatch events with automatic timeout cancellation | `gateway-protocol.md` §4 | P3 |
| F-MSG-07 | Native Polls Engine | Interactive voting in chat | `MESSAGE_POLL_VOTE_ADD` events; real-time vote percentage calculation | `gateway-protocol.md` §4 | P6 |
| F-MSG-08 | Stickers, GIFs & Soundboard | Rich expressive media | `SOUNDBOARD_SOUNDS` integration, custom guild stickers, Tenor GIF search integration | `gateway-protocol.md` §4 | P6 |
| F-MSG-09 | Thread Channels | Focused side discussions | `THREAD_LIST_SYNC`, thread sidebar, active and archived thread lifecycle | `gateway-protocol.md` §4 | P3 |
| F-MSG-10 | System Messages | Server event announcements | Member join, server boost, pin, and invite system message rendering | `frontend-d03.md` | P3 |
| F-MSG-11 | Message Drafts | Preserving unfinished messages | Local per-channel and per-thread draft preservation across navigation | Architectural Plan | P3 |
| F-MSG-12 | Saved Messages / Bookmarks | Bookmarking important messages | `GET /users/@me/saved-messages` integration | `rest-d02.md` §2 | P7 |

## SRCH — Search & Discovery

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-SRCH-01 | Advanced Search Filter Tokens | Granular message search | 7 search filter tokens: `from:`, `mentions:`, `has:`, `link:`, `embed:`, `file:`, `before:`, `after:`; sort by recency or relevance | `frontend-d03.md` §1.2–1.3 | P3 |
| F-SRCH-02 | Search Result Navigation | Contextual result viewing | `total_results` pagination, keyword highlighting, jump to message in channel | `frontend-d03.md` §1.1 | P3 |
| F-SRCH-03 | Cleanroom Search Correction | Defect-free search execution | Fixes official client bug where selecting channel search chip yields `total_results: 0` | `frontend-d03.md` §1.4 | P3 |
| F-SRCH-04 | Global QuickSwitcher | Keyboard-first navigation | Ctrl+K / Cmd+K fuzzy search across channels, DMs, voice rooms, and servers | `frontend-d03.md` §5.4 | P3 |
| F-SRCH-05 | Emoji Autocomplete | Rapid emoji entry | Typing `:shortcode` triggers popover with top 10 matching emojis | `frontend-d03.md` §2 | P3 |

## PRES — Presence & Activities

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-PRES-01 | User Status States | Visibility and availability | `online`, `idle`, `dnd`, and `invisible` states (Opcode 3); idle timeout transition | `gateway-protocol.md` §4, `backend-ipc.md` | P2 |
| F-PRES-02 | Rich Presence Activities | Activity broadcasting | `PRESENCE_UPDATE` events parsing game, Spotify music, and streaming activities | `gateway-protocol.md` §4 | P2 |
| F-PRES-03 | Custom Status Message | Custom user status | Custom text and emoji with expiration timestamp | `frontend-d03.md` §5 | P6 |
| F-PRES-04 | Presence Grouping | Roster organization | Sorting guild member lists and friends roster by status and role hierarchy | `frontend-d03.md` §3 | P3 |

## VOICE — Voice Engine & WebRTC

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-VOICE-01 | Join & Leave Voice Signaling | Establishing voice session | Gateway Opcode 4 -> `VOICE_SERVER_UPDATE` -> connects RTC WebSocket `wss://c-<region>-<hash>.discord.media:2053/2096` | `voice-deep.md` §1 | P4 |
| F-VOICE-02 | Voice Identify Handshake | Authenticating voice transport | Voice Opcode 0: `server_id`, `channel_id`, `user_id`, `session_id`, `token`, `max_dave_protocol_version: 1`, stream descriptors | `voice-deep.md` §1 | P4 |
| F-VOICE-03 | Select Protocol Handshake | Negotiating transport and codecs | Voice Opcode 1: protocol `udp`, audio codec `opus` (PT 120, priority 1000), video codec `AV1` (PT 101) | `voice-deep.md` §1 | P4 |
| F-VOICE-04 | Voice Ready Handshake | Server media endpoint response | Voice Opcode 2: `ssrc`, `ip`, `port`, encryption modes `aead_aes256_gcm_rtpsize` and `aead_xchacha20_poly1305_rtpsize` | `voice-deep.md` §1 | P4 |
| F-VOICE-05 | Session Description Handshake | Finalizing encryption keys | Voice Opcode 4: negotiated encryption `mode`, 32-byte `secret_key`, `media_session_id`, `dave_protocol_version: 1` | `voice-deep.md` §1 | P4 |
| F-VOICE-06 | UDP IP Discovery | NAT public IP/port discovery | 74-byte UDP probe: type 1/2, SSRC, ASCII IP string, port BE@72; repeated until Voice Ready | `voice-deep.md` §2.1 | P4 |
| F-VOICE-07 | NAT Keepalive | Maintaining UDP state | 8-byte UDP packet: `1337cafe` or `1337f00d` transmitted every 5 seconds | `voice-deep.md` §2.2 | P4 |
| F-VOICE-08 | RTP Opus Audio Transport | Audio packet framing | RTP payload type 120, 20ms frame duration, 48 kHz stereo; header extension `0xBEDE`; sequence numbers and timestamps | `voice-deep.md` §2.3 | P4 |
| F-VOICE-09 | SRTP Media Encryption | Encrypting audio payloads | AES-256-GCM (16-byte authentication tag + 4-byte LE counter tail; client counter `0x80000000 | n`); AAD = RTP header | `voice-deep.md` §2.3–2.4 | P4 |
| F-VOICE-10 | RTCP Telemetry & Quality | Monitoring call metrics | Sender Report (SR) ~5.2s interval; Receiver Report (RR) 1s interval; loss and jitter metrics | `voice-deep.md` §2.4 | P4 |
| F-VOICE-11 | Discontinuous Transmission (DTX) | Bandwidth conservation | Silence detection transmits ~10 comfort noise packets (200ms) before muting stream | `voice-deep.md` §2.3 | P4 |
| F-VOICE-12 | Socket Port Rotation | Network path agility | Periodic socket rotation at +60s: allocate new UDP socket -> issue IP discovery with existing SSRC -> swap active port | `voice-deep.md` §2.5 | P4 |
| F-VOICE-13 | Speaking State & Indicators | Audio activity detection | Voice Opcode 5 speaking state; green speaking rings around avatars | `voice-deep.md` §0/§4 | P4 |
| F-VOICE-14 | Push-to-Talk (PTT) | Input activation modes | Voice Activity Detection (VAD) and Push-to-Talk via global hotkeys | `backend-ipc.md`, R1 | P4 |
| F-VOICE-15 | Audio Device Management | Hardware configuration | Audio input/output device enumeration, hotplug detection, volume controls, echo cancellation (AEC), automatic gain control (AGC) | `frontend-d03.md` §4.2 | P4 |
| F-VOICE-16 | Neural Noise Suppression | Audio clarity enhancement | RNNoise neural audio filter integration with adjustable suppression levels | `native-re.md` §4 | P4 |
| F-VOICE-17 | DAVE / MLS End-to-End Encryption | Secure group voice calls | RFC 9420 MLS protocol integration; epoch ratchets; truncated 8-byte authentication tags | `voice-deep.md` §1, `native-re.md` §3.5 | P4.5 |
| F-VOICE-18 | Region Latency Probing | Optimal voice server routing | UDP ping probes across server regions (e.g. Frankfurt, Stockholm) to select lowest latency node | `voice-deep.md` §1 | P4 |
| F-VOICE-19 | Real-Time Call Diagnostics | Diagnostic telemetry HUD | RTT latency, packet loss percentage, audio bitrate, and jitter buffer health overlay | `voice-deep.md` §2.4 | P4 |
| F-VOICE-20 | Soundboard Player | In-channel audio soundboard | `SOUNDBOARD_SOUNDS` dispatch; audio decoding and soft mixing into voice channel | `gateway-protocol.md` §4 | P6 |
| F-VOICE-21 | In-Call Video Streams | Video camera feeds | Video stream descriptors `streams[{type: video, rid: "100", quality: 100}]`; video SSRC and RTX SSRC binding | `voice-deep.md` §1 | P5 |

## MEDIA — Video & Screen Streaming

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-MEDIA-01 | Camera Video Capture | Video calling | V4L2 device capture -> hardware encoder -> RTP video packetization | `voice-deep.md` §1 | P5 |
| F-MEDIA-02 | Screen Share (Go Live) | Desktop streaming | Window or monitor screen capture; dynamic resolution/framerate adaptation (720p/1080p, 30/60 FPS) | `voice-deep.md` §1 | P5 |
| F-MEDIA-03 | Linux Screen Capture Engine | Wayland and X11 capture | PipeWire XDG Desktop Portal integration for Wayland; X11 fallback capture | `native-re.md` §2.2 | P5 |
| F-MEDIA-04 | Desktop Audio Loopback | Sharing system audio | Desktop audio capture via PipeWire monitor source | `feature-backlog.md` R5-* | P5 |
| F-MEDIA-05 | Hardware Video Acceleration | Low-overhead encoding | Hardware-accelerated encoding via VA-API, Vulkan Video, or NVENC (H.264 / AV1) | GPU Codec Specifications | P5 |
| F-MEDIA-06 | Inline Media Players | Embedded media previews | Custom in-line players for MP4, WebM, GIF, and audio files in message stream | `frontend-d03.md` §5 | P3 |
| F-MEDIA-07 | Gaming Clips Engine | Recording gameplay clips | In-game clip recording and clipping tool integration | `main-arch.md` §3.5 | P7 |

## UI — User Interface & Components

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-UI-01 | Application Window & Shell | Main client window | Dark and light themes, compact and cozy density modes, frameless or system titlebar | `frontend-d03.md` §6 | P3 |
| F-UI-02 | Server Rail Navigation | Guild list navigation | Server icons, folder hierarchies, unread mention badges, drag-and-drop server reordering | `frontend-d03.md` §3 | P3 |
| F-UI-03 | Channel Tree Navigation | Channel hierarchy | Category folders, collapse toggles, unread highlights, active voice participants, threads | `frontend-d03.md` §3 | P3 |
| F-UI-04 | Virtualized Chat View | High-performance message display | Virtualized list handling 100k+ messages, author grouping, day dividers, rich embeds, inline editing | `frontend-d03.md` §5.4–5.12 | P3 |
| F-UI-05 | Message Composer Bar | Text input and drafting | Multi-line text editor, emoji autocomplete, mention picker, file attachment tray | `frontend-d03.md` §2 | P3 |
| F-UI-06 | Guild Member List | Server member roster | Virtualized list handling up to 24k members, role-based grouping, member search | `gateway-protocol.md` §5 | P3 |
| F-UI-07 | Direct Messages Home | Private chats and friends | Friends list tabs (Online, All, Pending, Blocked), direct message channels | `frontend-d03.md` §3 | P3 |
| F-UI-08 | Search Overlay Dialog | Message search UI | Filter chips, search results list, sorting toggles, jump-to-message action | `frontend-d03.md` §1 | P3 |
| F-UI-09 | Emoji & Sticker Picker | Expression selector | 3 tabs (emojis, stickers, GIFs), 10 category headers, frequently used section, search filter | `frontend-d03.md` §2 | P3 |
| F-UI-10 | Settings Dialog | Client configuration | Settings categories, Voice & Video configuration (38 controls), Keybinds, Language, Notifications | `frontend-d03.md` §4 | P3 |
| F-UI-11 | User Profile Popover | User information card | Avatar, banner, username, custom status, biography, badges, mutual guilds and friends | `frontend-d03.md` | P3 |
| F-UI-12 | Context Menus | Action menus | Accessible context menus with default and danger item styling, submenus, keyboard navigation | `frontend-d03.md` §5.3 | P3 |
| F-UI-13 | Modal Dialogs | Overlays and wizards | Focus traps, ARIA dialog attributes, smooth open/close transitions, Escape key dismissal | `frontend-d03.md` §5.5 | P3 |
| F-UI-14 | Tooltip Overlays | Contextual hints | Standard tooltips and rich badges with positioning algorithms | `frontend-d03.md` §5.7 | P3 |
| F-UI-15 | Form Controls & Sliders | Settings UI controls | Accessible toggle switches, sliders with tick marks, radio groups, and text fields | `frontend-d03.md` §5.8–5.9 | P3 |
| F-UI-16 | Avatars & Status Rings | Visual user identity | Circular avatars with status indicator cutouts (online, idle, dnd, offline) | `frontend-d03.md` §5.10 | P3 |
| F-UI-17 | Notification Badges | Visual notification indicators | Numbered unread badges, mention badges, guild notification pips | `frontend-d03.md` §5.11 | P3 |
| F-UI-18 | Custom Scrollbars | Content scrolling | Thin, auto-hiding scrollbars with smooth scrolling and gutter reservations | `frontend-d03.md` §5.12 | P3 |
| F-UI-19 | Design Token System | Visual styling consistency | 5,132 cleanroom-extracted variables mapped into cohesive CSS / GPUI styling tokens | `frontend-d03.md` §6 | P3 |
| F-UI-20 | Interface Sound Effects | Audio feedback | 15 distinct sound effects (message, join, leave, mute, unmute, ring, incoming call) | Audio Assets Specification | P3 |
| F-UI-21 | Keyboard Shortcuts | Accelerator navigation | Global shortcuts: Ctrl+K (QuickSwitcher), Ctrl+F (Search), Ctrl+E (Emoji), Esc (Close) | `frontend-d03.md` §5.4 | P3 |
| F-UI-22 | Layer & Popover Stacking | Z-index and popup management | Portal layers, click-outside dismissal, scroll locking, and focus restoration | `frontend-d03.md` §5.6 | P3 |
| F-UI-23 | Accessibility (A11y) | Inclusive user experience | WCAG 2.2 AAA compliance, AT-SPI screen reader accessibility, roving tabindex | `frontend-d03.md` §5.5 | P3 |
| F-UI-24 | Interface Animations | Visual smoothness | Hardware-accelerated transitions, modal fade/scale animations, loading spinners | `frontend-d03.md` §5 | P3 |

## I18N — Internationalization & Catalogs

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-I18N-01 | Custom Message Format Engine | String localization | Custom AST runtime for string interpolation with argument formatting | `i18n-deep.md` §1 | P3 |
| F-I18N-02 | Key Hashing Function | Fast translation lookup | `xxh64(LE string bytes) -> base64 -> first 6 characters` (36-bit compact hash key) | `i18n-deep.md` §0 | P3 |
| F-I18N-03 | CLDR Pluralization Rules | Grammatical correctness | Full CLDR plural categories (`zero`, `one`, `two`, `few`, `many`, `other`) + exact `=N` matches | `i18n-deep.md` §2 | P3 |
| F-I18N-04 | Rich Text Tag Formatting | Formatted localized strings | 23 rich-text tag formatting rules (bold, italic, links, embedded UI components) | `i18n-deep.md` §3 | P3 |
| F-I18N-05 | Locale-Aware Formats | Numbers and dates | Cultural number grouping separators, relative timestamps, localized dates | `i18n-deep.md` §5–6 | P3 |
| F-I18N-06 | Language Switcher & Fallback | Multi-language support | 36 supported languages; deterministic fallback to `en-US`; runtime locale switching | `i18n-deep.md` §7 | P7 |
| F-I18N-07 | Open-Source Translation Strings | Cleanroom localization text | Independent, original open-source string catalogs | Architectural Plan | P3 |

## SYS — Platform Integration & Operating System

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-SYS-01 | System Tray Integration | Background operation | System tray icon, context menu, minimize-to-tray, unread notification flashing | `backend-ipc.md` | P6 |
| F-SYS-02 | Application Launcher Badge | Unread badge counters | Desktop dock/taskbar unread counter badges (Unity API on Linux, Dock on macOS) | `backend-ipc.md` | P6 |
| F-SYS-03 | Desktop Notifications | System alert integration | FreeDesktop D-Bus Notifications (`org.freedesktop.Notifications`) with action buttons | `main-arch.md` §3.2 | P6 |
| F-SYS-04 | Deep Link URI Handler | Protocol scheme integration | `discord://` URI scheme registration with single-instance IPC forwarding | `main-arch.md` | P6 |
| F-SYS-05 | Desktop Autostart | Boot-time startup | XDG autostart `.desktop` entry generation with `--start-minimized` flag | `backend-ipc.md` | P6 |
| F-SYS-06 | In-App Auto-Updater | Binary delivery | Standalone updater engine: release manifest parsing, Ed25519 signature checks, atomic rollback | `main-arch.md` §5 | P8 |
| F-SYS-07 | System Clipboard Integration | Media copy-pasting | Text and image clipboard access; direct paste-to-upload in composer | `backend-ipc.md` | P6 |
| F-SYS-08 | Power Management Inhibitor | Uninterrupted calls | System sleep inhibition (`org.freedesktop.ScreenSaver` / `systemd-logind`) during active voice calls | `backend-ipc.md` | P6 |
| F-SYS-09 | Idle State Detection | Automatic user status | Wayland idle detection (`ext-idle-notify-v1`) and X11 Screensaver extension | `backend-ipc.md` | P6 |
| F-SYS-10 | Native Spellchecker | Language correctness | Hunspell dictionary integration via native FFI; wavy red underline and correction popup | `native-re.md` §6 | P6 |
| F-SYS-11 | Game Clips Management | Video clip viewing | Local file management for recorded video clips | `main-arch.md` §3.5 | P7 |
| F-SYS-12 | Detachable Popout Windows | Multi-window support | Detachable video call and channel popout windows | `main-arch.md` §2.3 | P7 |
| F-SYS-13 | Cross-Platform Support | Windows and macOS parity | Native platform backends: WASAPI / CoreAudio, native trays, global shortcuts | Architectural Plan | P7 |

## SEC — Security & Privacy Architecture

| ID | Feature | Purpose | Cleanroom Reverse-Engineered Mechanics | Source Reference | Phase |
|---|---|---|---|---|---|
| F-SEC-01 | Keyring-Only Token Storage | Protecting user credentials | Authentication tokens stored exclusively in system Keyring; zero plaintext on disk | Security Policy | P1 |
| F-SEC-02 | Sensitive Log Redaction | Safe diagnostic logging | Automatic filtering and masking of tokens, session IDs, passwords, and private IPs | Log Sanitizer Specification | P1 |
| F-SEC-03 | Telemetry Opt-Out | User privacy protection | Analytics and tracking endpoints disabled by default | `rest-d02.md` §2 | P1 |
| F-SEC-04 | Honest Client Fingerprint | Transparent network identity | Transparent and configurable `X-Super-Properties` headers without anti-abuse spoofing | `rest-d02.md` §6.2 | P1 |
| F-SEC-05 | Cryptographic Update Signing | Update package integrity | Ed25519 cryptographic signatures on update manifests and release binaries | Architectural Plan | P8 |
| F-SEC-06 | Cleanroom Asset Licensing | Legal and copyright integrity | Original and permissively licensed open-source assets with zero proprietary distribution | Legal Framework | — |
