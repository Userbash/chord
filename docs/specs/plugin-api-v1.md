# Chord Plugin API v1 Architecture & Capability Specification (`P3-1.1`)

- **Document Version:** `1.0.0`
- **Target Implementation:** `chord-platform/plugins.rs` (`P3-1.2`)
- **Status:** Architectural Specification (Ready for Implementation)
- **Target Target ABI:** `wasm32-wasip2` / WIT package `chord:plugin@1.0.0`
- **Host Engine:** `wasmtime` (Component Model runtime)

---

## 1. Architectural Overview & Threat Model

### 1.1 High-Level Architecture
The Chord Plugin Architecture provides an extensible, memory-safe, and crash-isolated runtime for community and third-party extensions within Chord (the native Rust/GPUI Discord client).

```
                                    +-----------------------------------------------+
                                    |               GPUI Application                |
                                    |  (Views, AppState, 120 FPS Rendering Loop)    |
                                    +-----------------------+-----------------------+
                                                            |
                                               Async Non-blocking Dispatch
                                                            |
                                    +-----------------------v-----------------------+
                                    |            Chord Plugin Host (Rust)           |
                                    |      `chord-platform/src/plugins.rs`          |
                                    |   - Manifest Registry & Signature Verifier    |
                                    |   - Security Sandbox & Capability Firewall    |
                                    |   - Wasmtime Engine, Store & Linker           |
                                    |   - Resource Watchdog & Crash Supervisor      |
                                    +-----------------------+-----------------------+
                                                            |
                                         wasmtime Component Model Runtime
                                           (Fuel Metering, Epoch Ticks)
                                                            |
                 +------------------------------------------+------------------------------------------+
                 |                                                                                     |
    +------------v------------+                                                           +------------v------------+
    |    WASM Plugin A        |                                                           |    WASM Plugin B        |
    | (Linear Memory <= 16M)  |                                                           | (Linear Memory <= 16M)  |
    | `wasm32-wasip2`         |                                                           | `wasm32-wasip2`         |
    | Capability: `chat.*`    |                                                           | Capability: `ui.*`      |
    +-------------------------+                                                           +-------------------------+
```

### 1.2 Sandbox Isolation & Threat Model
Chord executes untrusted third-party code. To prevent credential theft, remote code execution, denial of service, and UI degradation, the plugin environment enforces strict isolation boundaries:

1. **Zero Direct OS Access:**
   - Plugins compiled to `wasm32-wasip2` have **no raw host filesystem access**, **no raw socket access**, **no process execution**, and **no dynamic library loading**.
   - Host calls are exposed strictly through custom WIT interfaces (`chord:plugin/host`).
2. **Credential & Token Isolation (Clean-Room Security Boundary):**
   - Plugins have **zero access to system keyring, user credentials, Discord authentication tokens, or session encryption keys (DAVE MLS / SRTP)**.
   - Any outbound fetch request allowed under `net.fetch_allowlisted` is strictly mediated by the host; the host strips and rejects any request targeting Discord API endpoints (`discord.com/api`, `gateway.discord.gg`, `*.discord.media`) and never appends user auth tokens.
3. **Memory Safety & Process Isolation:**
   - Linear memory per instance is strictly bounded (`16 MiB` default, `32 MiB` maximum). Memory allocation failures trigger immediate instance traps without taking down the Chord host application.
4. **Deterministic Hook Budget (UI Frame Safety):**
   - Synchronous UI hooks (`on_message_render`) must return within a bounded execution slice (`<= 5 ms`) to guarantee that GPUI's 120 FPS frame budget (`8.33 ms` total per frame) is preserved. Execution is governed by Wasmtime fuel consumption and epoch-based dead-man timers.

---

## 2. Plugin Manifest (`plugin.toml` / `manifest.json`)

Every plugin package is delivered as an archive (or directory) containing:
- `plugin.toml` (or `manifest.json`): Declarative manifest and requested permissions.
- `plugin.wasm`: Compiled WASM Component Model binary targeting `wasm32-wasip2`.
- `plugin.sig`: Detached Ed25519 signature of the `plugin.wasm` and manifest metadata.

### 2.1 Manifest Schema (`plugin.toml`)

```toml
[plugin]
id = "org.chord.sample-formatter"
name = "Markdown Syntax Enhancer"
version = "1.0.4"
api_version = "1.0"
entrypoint = "plugin.wasm"
sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
author = "Dev Team <dev@chord.app>"
description = "Custom visual styling and client-side message formatting"
license = "MIT"
homepage = "https://chord.app/plugins/sample-formatter"

[security]
signature = "3a10b42f...ed25519_hex..."
public_key = "a8f3b21c...ed25519_hex..."

[capabilities]
requested = [
    "chat.read_visible",
    "chat.format_message",
    "storage.plugin_kv",
    "net.fetch_allowlisted"
]

[capabilities.config]
allowed_domains = [
    "api.github.com",
    "raw.githubusercontent.com"
]
kv_quota_bytes = 1048576  # 1 MiB max

[ui_slots]
sidebar_panel = false
settings_section = true
status_bar_item = false
```

### 2.2 Validation & Cryptographic Verification Pipeline
Before loading a plugin into the Wasmtime engine:
1. **Manifest Integrity:** Parse and validate required fields: `id` (must match `^[a-z0-9_.-]{3,64}$`), `version` (strict SemVer), `api_version` (supported host API version range `^1.0`).
2. **SHA-256 Digest Verification:** Compute SHA-256 of `plugin.wasm` on disk and assert strict equality with `sha256` in the manifest.
3. **Ed25519 Signature Verification:** Verify `signature` against `public_key` over the payload `sha256 + id + version`. If verification fails, loading aborts immediately with `PluginError::SignatureVerificationFailed`.

---

## 3. Capability Permissions Model (Explicit Opt-In)

Capabilities are fine-grained and follow the principle of least privilege. All capabilities require explicit user opt-in during plugin installation or activation.

| Capability Identifier | Description | Host Guard & Security Boundaries |
| :--- | :--- | :--- |
| `chat.read_visible` | Read decrypted message contents currently rendered in active chat view. | Only messages in the currently focused channel viewport are exposed; no bulk historical scraping. |
| `chat.format_message` | Intercept and augment message render AST (add badges, syntax highlights). | Read-write AST transformation before render; cannot forge sender IDs or delete messages. |
| `commands.register_slash` | Register client-side slash commands (e.g. `/shrug`, `/timestamp`). | Command namespace isolated with `/plugin_id:command` or collision-checked alias. |
| `ui.panel.sidebar` | Mount a custom GPUI sidebar view widget. | Rendered inside a isolated layout container; cannot capture out-of-bounds pointer events. |
| `ui.settings.section` | Provide a plugin-specific preferences configuration panel. | Managed schema inputs validated by host; no arbitrary raw HTML/JS execution. |
| `storage.plugin_kv` | Persistent Key-Value storage for state and preferences. | Strictly namespaced to plugin ID; hard quota **<= 1 MiB** per plugin. |
| `net.fetch_allowlisted` | Outbound HTTP/HTTPS GET/POST to declared domains. | Explicit domain allowlist. **Zero access to Discord auth tokens, cookies, or `discord.com` endpoints.** |

---

## 4. Resource Quotas & Watchdog (`wasmtime` Limits)

The host maintains strict control over plugin resources using Wasmtime's resource limiter and epoch-based interruption.

### 4.1 Memory & Storage Limits
- **Default Linear Memory:** `16 MiB` (`256` Wasm pages of 64 KiB).
- **Max Linear Memory:** `32 MiB` (`512` Wasm pages). Attempts to grow past `32 MiB` via `memory.grow` trap with `OutOfMemory`.
- **KV Storage Quota:** `1,048,576 bytes` (1 MiB) per plugin SQLite/sled namespace. Exceeding quota returns `StorageQuotaExceeded`.
- **Max Timers:** Bounded to at most `8` active timers per instance to prevent task starvation.

### 4.2 Watchdog & Execution Budgets
To guarantee 120 FPS fluid GPUI rendering, callbacks are executed under strict time and fuel limits:
- **UI & Render Hooks (`on_message_render`):**
  - Fuel limit: `50,000,000` instructions.
  - Wall-clock deadline: `<= 5 ms`.
  - Driven by Wasmtime epoch timer ticking at `1 ms` intervals. If 5 epochs elapse during a hook, execution is aborted via `Trap::Interrupt`.
- **Background & Lifecycle Hooks (`on_load`, `on_slash_command`):**
  - Fuel limit: `500,000,000` instructions.
  - Wall-clock deadline: `<= 250 ms`.

### 4.3 Crash Isolation & Auto-Disablement (Dead-Man Switch)
1. When a WASM instance panics, traps (out-of-fuel, memory violation, unreachable opcode), or times out:
   - The trap is captured by the host `WasmPluginInstance`.
   - The active hook returns an empty/fallback result so the UI rendering pipeline never hangs.
   - An increment is recorded in `plugin_crash_counter`.
2. **Auto-Disablement Policy:**
   - If a plugin traps **3 times** (`N=3`) within a 60-second sliding window, the host automatically transitions the plugin into `PluginState::CrashedDisabled`.
   - The user is notified via a non-intrusive GPUI banner with the error log and options to "Restart" or "Uninstall".

---

## 5. WIT Interface & Lifecycle Hooks

The plugin contract is defined using WebAssembly Interface Type (WIT) specification under package `chord:plugin@1.0.0`.

### 5.1 Host & Guest Definitions (`chord.wit`)

```wit
package chord:plugin@1.0.0;

interface types {
    enum log-level {
        trace,
        debug,
        info,
        warn,
        error
    }

    record plugin-metadata {
        id: string,
        name: string,
        version: string,
        author: string
    }

    record message-payload {
        id: u64,
        channel-id: u64,
        author-id: u64,
        author-name: string,
        content: string,
        timestamp-ms: u64
    }

    record rendered-token {
        token-type: string,
        text: string,
        color: option<string>,
        bold: bool,
        italic: bool
    }

    record http-request {
        url: string,
        method: string,
        headers: list<tuple<string, string>>,
        body: option<list<u8>>
    }

    record http-response {
        status: u16,
        headers: list<tuple<string, string>>,
        body: list<u8>
    }

    variant plugin-error {
        unauthorized(string),
        invalid-argument(string),
        storage-quota-exceeded,
        network-error(string),
        internal-error(string)
    }
}

interface host {
    use types.{log-level, http-request, http-response, plugin-error};

    /// Write a structured log entry into Chord's tracing telemetry
    log: func(level: log-level, message: string);

    /// Key-Value Storage operations (namespaced per plugin ID, quota <= 1 MiB)
    kv-get: func(key: string) -> result<option<list<u8>>, plugin-error>;
    kv-set: func(key: string, value: list<u8>) -> result<(), plugin-error>;
    kv-delete: func(key: string) -> result<bool, plugin-error>;

    /// Allowlisted outbound network fetch (Discord credentials strictly prohibited)
    http-fetch: func(req: http-request) -> result<http-response, plugin-error>;
}

world plugin {
    import host;

    use types.{plugin-metadata, message-payload, rendered-token, plugin-error};

    /// Lifecycle Hooks
    export on-load: func(meta: plugin-metadata) -> result<(), plugin-error>;
    export on-unload: func() -> result<(), plugin-error>;
    export on-enable: func() -> result<(), plugin-error>;
    export on-disable: func() -> result<(), plugin-error>;

    /// Event Hooks
    export on-message-render: func(msg: message-payload) -> result<list<rendered-token>, plugin-error>;
    export on-slash-command: func(command: string, args: list<string>) -> result<string, plugin-error>;
}
```

### 5.2 Deterministic Serialization & Telemetry
- Data crossing the boundary uses Component Model canonical ABI.
- All host interactions emit structured `tracing` spans:
  - `plugin.trap_count{plugin_id}`
  - `plugin.fuel_consumed{plugin_id, hook}`
  - `plugin.hook_latency_us{plugin_id, hook}`

---

## 6. Implementation Roadmap for `P3-1.2` & 9-Dimension Test Checklist

### 6.1 Rust Module Layout (`chord-platform/src/plugins.rs`)

```rust
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: semver::Version,
    pub api_version: String,
    pub entrypoint: PathBuf,
    pub sha256: [u8; 32],
    pub author: String,
    pub capabilities: HashSet<PluginCapability>,
    pub allowed_domains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginCapability {
    ChatReadVisible,
    ChatFormatMessage,
    CommandsRegisterSlash,
    UiPanelSidebar,
    UiSettingsSection,
    StoragePluginKv,
    NetFetchAllowlisted,
}

pub struct PluginSandboxConfig {
    pub max_memory_bytes: usize, // default: 16 MiB, max: 32 MiB
    pub fuel_limit_per_hook: u64, // 50_000_000 for UI, 500_000_000 for background
    pub epoch_deadline_ms: u64,  // 5 ms for UI hooks
    pub crash_threshold: u32,    // 3 traps -> auto-disable
}

pub struct PluginHost {
    pub engine: wasmtime::Engine,
    pub linker: wasmtime::component::Linker<PluginHostState>,
    pub instances: HashMap<String, WasmPluginInstance>,
}

pub struct WasmPluginInstance {
    pub manifest: PluginManifest,
    pub store: wasmtime::Store<PluginHostState>,
    pub crash_count: u32,
    pub state: PluginState,
}
```

### 6.2 9-Dimension Verification Checklist for `P3-1.2`

| # | Dimension | Test Specification & Oracle Verification |
| :---: | :--- | :--- |
| 1 | **Positive (Happy Path)** | Load valid signed WASM plugin, initialize lifecycle `on_load`, invoke `on_message_render`, verify expected tokens. |
| 2 | **Negative (Invalid Signatures & Tampering)** | Tamper 1 byte of `plugin.wasm` or modify manifest `sha256`; assert loading fails with `SignatureVerificationFailed`. |
| 3 | **Edge Boundaries** | Allocate memory up to exactly `16 MiB`; test max KV payload (`1,048,576` bytes); test zero-length payloads. |
| 4 | **Error Handling & Traps** | Plugin invokes `unreachable` or division by zero; host catches trap, returns fallback, application remains stable. |
| 5 | **Empty States & Defaults** | Plugin with empty capability list loads cleanly and cannot call host functions; empty messages return empty tokens. |
| 6 | **Loading Skeletons & Lifecycle** | State transitions strictly: `Unloaded -> Loaded -> Enabled -> Disabled -> Unloaded`. |
| 7 | **Capability Security Enforcement** | Plugin without `net.fetch_allowlisted` calls `http_fetch`; host rejects with `Unauthorized`. Plugin attempts fetch to `discord.com/api`; host firewall immediately blocks. |
| 8 | **Recovery & Dead-Man Switch** | Plugin traps 3 times consecutively; host marks state `CrashedDisabled`, disables hooks, emits alert telemetry. |
| 9 | **Observable Telemetry** | Trapped execution and execution latency recorded via `tracing` metrics and assertions against hook latency counters. |
