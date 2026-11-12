# GPUI Kit Migration Architectural Manifest (`docs/specs/GPUI-ARCHITECTURE-MIGRATION.md`)

## 1. Modernization Goals and Principles

### 1.1. Primary Objective
Complete replacement of the hybrid architecture **Tauri 2 (WebKit2GTK + 107 JS files + manual DOM/CSS + IPC bridge)** with a **100% native Rust UI stack powered by `gpui-kit`** (`./gpui-kit`).
This transition eliminates all fundamental drawbacks of the current approach:
- **IPC Bridge Elimination**: All data is transferred directly via Rust `Model<T>` and reactive `Context<T>`, without serializing every action into JSON strings.
- **WebKit Overhead Removal**: RAM usage drops from 260–390 MB down to 35–65 MB; UI frame rate achieves a stable 120 FPS powered by the GPU (Vulkan / Wayland / X11).
- **Fragile Custom Code Liquidation**: Replacing 15,000+ lines of custom JS scripts, manual CSS, bespoke virtual scrollers, and regular expression parsers with production-grade industrial libraries (crates).

---

## 2. Replacement Matrix: Custom Scripts → Industry Crates

| Functional Domain | Current Implementation (Custom Code) | Standard Library / Crate Replacement | Solution Benefit |
|---|---|---|---|
| **Windowing System & UI Components** | Tauri 2 + 107 JS files (`rust/gui-spike/ui/**`), manual DOM, manual CSS | **`gpui-kit` + `gpui-component`** (`./gpui-kit`) | 75+ ready-made components: `Button`, `Sidebar`, `Dialog`, `DropdownMenu`, `Input`, `Avatar`, `Badge`, `Checkbox`, `Switch`, `Tooltip`. GPU-accelerated rendering, zero JavaScript. |
| **Chat & Member Virtual Scrolling** | Custom JS row-height calculation, manual `translateY` transforms, and `onscroll` listeners | **`gpui-component::virtual_list`** / **`gpui-component::table`** | Native GPU scrolling, O(1) memory footprint with 100,000+ messages, seamless scrolling without blank screens or jitter. |
| **Splits & Panel Layout** | Custom CSS Flexbox columns with manual mouse-drag handlers for resizing (`actions.js`) | **`gpui-kit::component::resizable::{h_resizable, resizable_panel}`** | Built-in multi-column resize support, persistent column widths, native OS resize cursors. |
| **Text Rendering, Markdown & Highlighting** | Custom JS regular expressions, manual `<span>` tags, vulnerable to XSS | **`gpui-component::text::TextView` + `gpui-component::highlighter`** (Syntect + Markdown AST) | Secure AST parser, Syntect-based syntax highlighting, native interactive hyperlinks, @mentions, and custom emoji. |
| **Message Input & Hotkeys** | HTML `<div contenteditable>` / `<textarea>` with manual `keydown` event trapping | **`gpui-component::input::{Input, Editor, InputState}`** | Fully featured native text editor supporting Undo/Redo, clipboard, history, IME, and keyboard shortcuts. |
| **Reactive State & Event Bus** | String IPC `window.__TAURI__.invoke` + `CustomEvent` dispatched on global `window` | **GPUI Reactive Model (`Model<AppState>`, `cx.notify()`, `cx.subscribe()`)** | Compile-time Rust type safety, zero data races, 0 serialization/deserialization overhead. |
| **Interactive Captcha / Web Auth** | Injecting `<iframe>` into WebView DOM with styling collisions and focus loss | **`gpui-webview::WebView`** | Isolated native modal based on `wry::WebViewBuilder` used exclusively during hCaptcha or OAuth verification; remainder of app remains pure GPUI. |
| **REST API Network Client** | Custom HTTP/1.1 parsing over raw sockets and text strings | **`reqwest`** (with connection pooling, HTTP/2, Rustls, and `serde` JSON mapping) + **`backoff`** | Automatic keep-alive handling, connection rotation, exponential backoff on HTTP 429 rate limits, memory safety. |
| **Gateway & Compression** | Manual wrappers | **`tokio-tungstenite`** + **`ruzstd`** (streaming Zstandard decompressor) + **`discord-codec`** (ETF) | Industry-standard WebSocket stack with zero memory leaks and proper Heartbeat management. |
| **Audio Input/Output Devices** | Manual FFI invocations via shell commands | **`cpal`** (Cross-Platform Audio Library) + **`rubato`** (resampling) | Battle-tested Rust audio standard for ultra-low latency microphone capture and playback. |
| **Secure Token Storage** | Manual FFI to libsecret | **`keyring`** crate (supports Secret Service, KWallet, Windows Credential Manager, macOS Keychain) | Single verified cross-platform abstraction interface without code duplication. |

---

## 3. Department Roles and Responsibilities

```text
┌──────────────────────────────────────────────────────────────────────────────────┐
│                   CHIEF SOFTWARE ARCHITECT / TECH LEAD                           │
│        (GPUI Architecture Design, App/Window Lifecycle, Data Model)              │
└────────┬─────────────────┬─────────────────┬──────────────────┬──────────────────┘
         │                 │                 │                  │
         ▼                 ▼                 ▼                  ▼
┌─────────────────┐┌────────────────┐┌────────────────┐┌─────────────────┐
│   G1 BACKEND    ││ G2 GPUI FRONT  ││   G3 DESIGN    ││     G4 QA       │
│  (Data & Libs)  ││ (Native Views) ││ (Theme/Tokens) ││  (GPUI Tests)   │
└─────────────────┘└────────────────┘└────────────────┘└─────────────────┘
         │                                                      ▲
         ▼                                                      │
┌─────────────────┐                                             │
│    G5 MEDIA     │─────────────────────────────────────────────┘
│ (Voice/Capture) │
└─────────────────┘
```

### 3.1. Chief Software Architect
- **Owner**: Team Lead / Software Architect
- **Responsibilities**:
  1. Designing root structure of the GPUI application (`crates/discord-app-gpui`).
  2. Defining unified reactive state model `AppState` (`Model<CurrentUser>`, `Model<GuildTree>`, `Model<MessageStore>`, `Model<VoiceSessionState>`).
  3. Enforcing ban on ad-hoc scripts: every architectural choice must rely on a proven crate.

### 3.2. G1 Backend (Data, Domain & Standard Libraries Lead)
- **Owner**: G1 Backend Lead
- **Responsibilities**:
  1. Extracting domain structs from the prototype into clean data crates.
  2. Integrating `reqwest` with automatic retry/rate-limit complying with Discord REST v9.
  3. Ensuring thread-safe interaction between background Tokio tasks (Gateway, Voice) and the main GPUI thread via `cx.update_model()`.

### 3.3. G2 GPUI Frontend (Native UI & Components Lead)
- **Owner**: G2 Frontend Lead
- **Responsibilities**:
  1. Implementing main window layout: `ServerRail` (guild column), `ChannelSidebar` (channel list), `ChatView` (messages), `MemberSidebar` (member roster).
  2. Integrating `virtual_list` for chat and member lists: smooth scrolling across 100k messages and 25k members.
  3. Implementing modals on top of `gpui-kit::component::dialog`: server creation wizard, message search, settings, and captcha resolution modal via `gpui-webview`.

### 3.4. G3 Design (UX/UI Tokens & Theming Lead)
- **Owner**: G3 Design Lead
- **Responsibilities**:
  1. Porting Discord Dark Theme into `gpui-kit` theme system (`Theme` / `ActiveTheme`).
  2. Mapping Lucide / Discord icons into `gpui-kit-assets`.
  3. Configuring contrast ratios, typography, and keyboard focus outlines according to AccessKit (WCAG 2.2 AA).

### 3.5. G4 QA (Automation & Test Harness Lead)
- **Owner**: G4 QA Lead
- **Responsibilities**:
  1. Constructing native test harness on `gpui::TestAppContext` (headless UI testing without requiring Chromium or Xvfb).
  2. Validating business logic invariants (TDD Red -> Green).
  3. Guaranteeing 100% mutation test kill-rate (5/5 on each layer).

### 3.6. G5 Voice / Platform Lead
- **Owner**: G5 Platform Lead
- **Responsibilities**:
  1. Connecting `cpal` audio subsystem directly to `discord-voice` session.
  2. Rendering voice status, ping indicators, and speaking activity indicators in GPUI.
  3. Integrating video stream overlay and Push-To-Talk shortcuts.

### 3.7. Product Manager & Technical Reviewer (PM / Tech Review)
- **Owner**: PM Reviewer & Technical Reviewer
- **Responsibilities**:
  1. Zero-Idle task dispatching to workers.
  2. Dual review of every PR (`ACCEPT.md` + `REVIEW.md`).
  3. Code hygiene enforcement and regression prevention.

---

## 4. Architectural Structure of New Crate (`rust/crates/discord-gpui`)

```text
rust/crates/discord-gpui/
├── Cargo.toml                  # Dependencies: gpui-kit, reqwest, tokio, serde, cpal
├── src/
│   ├── main.rs                 # Entry point: GPUI App initialization, window, assets
│   ├── state/                  # Reactive state
│   │   ├── mod.rs              # Root AppState
│   │   ├── auth.rs             # Authentication and session state
│   │   ├── guilds.rs           # Guild tree, folders, and channels
│   │   ├── messages.rs         # Message cache and virtual pages
│   │   ├── members.rs          # Guild member roster
│   │   └── voice.rs            # Voice connection state
│   ├── views/                  # Native GPUI screens and views
│   │   ├── mod.rs
│   │   ├── app_layout.rs       # Three-column layout split (Rail + Sidebar + Main)
│   │   ├── server_rail.rs      # Left column server icon rail
│   │   ├── channel_list.rs     # Text and voice channel tree
│   │   ├── chat_view.rs        # Chat: VirtualList + TextView (Markdown) + MessageBar
│   │   ├── message_item.rs     # Message item: avatar, author, timestamp, attachments
│   │   ├── member_list.rs      # Right column: VirtualList of members with roles
│   │   └── voice_panel.rs      # Voice connection control bar
│   ├── dialogs/                # Modal dialogs
│   │   ├── mod.rs
│   │   ├── captcha_modal.rs    # hCaptcha modal window embedding gpui-webview
│   │   ├── create_server.rs    # Server creation wizard
│   │   ├── join_server.rs      # Invite input and confirmation
│   │   └── settings_dialog.rs  # User settings: account, voice, appearance
│   ├── theme/                  # Theme configuration
│   │   ├── mod.rs
│   │   └── discord_dark.rs     # Discord Dark color palette
│   └── services/               # Background services
│       ├── gateway_runner.rs   # WebSocket Gateway supervision
│       └── rest_client.rs      # REST v9 client using reqwest
└── tests/                      # GPUI integration tests
    ├── chat_view_test.rs       # Headless UI test for virtual scrolling
    ├── navigation_test.rs      # Channel and guild switching
    └── captcha_dialog_test.rs  # Captcha modal display on error 40003
```
