# Reference Project Guide (`Serein`, `LemonCord`, `rust-discord-activity`) & Mapping to `discord-lab`

Documentation and cross-reference maps for external open-source references analyzed during development:

## 1. Reference Repositories and Documentation

| Project | Upstream Source | Local Mirror | Detailed Function & Call Map |
|---|---|---|---|
| **Serein** (`ViceVerse-cz/Serein`) | `https://github.com/ViceVerse-cz/Serein` | [`docs/reference-projects/sources/Serein/`](sources/Serein) | [`docs/reference-projects/SEREIN-FUNCTION-MAP.md`](SEREIN-FUNCTION-MAP.md) |
| **LemonCord** (`japandotorg/LemonCord`) | `https://github.com/japandotorg/LemonCord` | [`docs/reference-projects/sources/LemonCord/`](sources/LemonCord) | [`docs/reference-projects/LEMONCORD-FUNCTION-MAP.md`](LEMONCORD-FUNCTION-MAP.md) |
| **rust-discord-activity** (`0.3.1`) | `https://docs.rs/rust-discord-activity/latest/rust_discord_activity/struct.DiscordClient.html` | [`docs/reference-projects/sources/rust-discord-activity-0.3.1/`](sources/rust-discord-activity-0.3.1)<br>[`rust_discord_activity_DiscordClient.html`](sources/rust_discord_activity_DiscordClient.html) | [`docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md`](RUST-DISCORD-ACTIVITY-MAP.md) |

---

## 2. Subsystem Mapping Matrix (`Serein` / `LemonCord` / `rust-discord-activity` ↔ `discord-lab`)

This cross-reference table allows comparing implementations in `discord-lab` against patterns in `Serein`, `LemonCord`, or `rust-discord-activity`:

| Subsystem / Feature | Implementation in `discord-lab` | Reference in `Serein` / `LemonCord` / `rust-discord-activity` | Key Reference Functions |
|---|---|---|---|
| **Gateway WebSocket, Zstd, Reconnect, Voice/Stream Signaling (`Op 4`, `Op 18..22`)** | [`rust/crates/discord-core/src/ws.rs`](../../rust/crates/discord-core/src/ws.rs) | [`Serein/crates/discord-gateway/src/lib.rs`](sources/Serein/crates/discord-gateway/src/lib.rs)<br>[`Serein/crates/discord-gateway/src/voice.rs`](sources/Serein/crates/discord-gateway/src/voice.rs) | `run_inner`, `Calls::packet`, `Calls::stream_packet`, `Calls::watch_dispatch`, `Calls::stream_dispatch` |
| **Voice WebSocket v8, UDP IP Discovery, RTP/RTCP, Soundshare, Stream Transport** | [`rust/crates/discord-voice/src/lib.rs`](../../rust/crates/discord-voice/src/lib.rs) | [`Serein/crates/discord-voice/src/transport.rs`](sources/Serein/crates/discord-voice/src/transport.rs)<br>[`Serein/crates/discord-voice/src/crypto.rs`](sources/Serein/crates/discord-voice/src/crypto.rs) | `run_inner`, `run_stream_inner`, `discovery`, `announce_video`, `Encryption::seal`, `Encryption::open` |
| **H.264 Inbound Depacketization (RFC 6184 STAP-A/FU-A), RTX (RFC 4588), PLI** | [`rust/crates/discord-voice/src/lib.rs`](../../rust/crates/discord-voice/src/lib.rs)<br>[`rust/crates/discord-codec/src/lib.rs`](../../rust/crates/discord-codec/src/lib.rs) | [`Serein/crates/discord-voice/src/video_receive.rs`](sources/Serein/crates/discord-voice/src/video_receive.rs)<br>[`Serein/crates/discord-voice/src/video_sps.rs`](sources/Serein/crates/discord-voice/src/video_sps.rs) | `Assembler::push`, `Receivers::restore_rtx`, `spawn_decoder`, `Backend::decode`, `decode_loop`, `video_sps::normalize` |
| **Linux Screen Capture (PipeWire / XDG Portal / X11) & Hardware H.264 Encoding** | [`rust/crates/discord-voice/src/lib.rs`](../../rust/crates/discord-voice/src/lib.rs) | [`Serein/crates/discord-voice/src/screen.rs`](sources/Serein/crates/discord-voice/src/screen.rs)<br>[`Serein/crates/discord-voice/src/screen/linux.rs`](sources/Serein/crates/discord-voice/src/screen/linux.rs) | `Worker::start`, `encode_loop`, `ScreenEncoder::encode`, `gstreamer::Capture::new`, `portal_linux::Portal::open` |
| **Stream Audio Loopback Isolation & Per-User Volume Control (`0..200%`)** | [`rust/crates/discord-voice/src/lib.rs`](../../rust/crates/discord-voice/src/lib.rs) | [`Serein/crates/discord-voice/src/screen/audio_linux.rs`](sources/Serein/crates/discord-voice/src/screen/audio_linux.rs)<br>[`Serein/crates/discord-voice/src/mixer.rs`](sources/Serein/crates/discord-voice/src/mixer.rs) | `OwnApplication::allows`, `Capture::start`, `Mixer::pop_with_volumes`, `Playout::next` |
| **GPU Video Frame Rendering (Zero-Copy YUV420 to RGB)** | [`rust/discord-gpui/src/views/`](../../rust/discord-gpui/src/views/) | [`Serein/apps/desktop/src/gpu_video.rs`](sources/Serein/apps/desktop/src/gpu_video.rs) | `Converter::new`, `VideoTexture::upload_gpu`, `fs_nv12` / `fs_i420`, `watch::store_frame` |
| **Discord Rich Presence IPC (`discord-ipc-0`, `SET_ACTIVITY`)** | [`rust/crates/discord-platform/src/lib.rs`](../../rust/crates/discord-platform/src/lib.rs) | [`rust-discord-activity-0.3.1/src/client/ipc.rs`](sources/rust-discord-activity-0.3.1/src/client/ipc.rs)<br>[`Serein/apps/desktop/src/game_activity.rs`](sources/Serein/apps/desktop/src/game_activity.rs) | Client: `DiscordClient::connect`, `send_payload`, `send`, `recv`.<br>Server: `game_activity::listen`, `serve_ipc`, `read_frame`, `write_frame`. |
