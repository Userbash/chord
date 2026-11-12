# Beta Tester's Guide for `discord-gpui` (`v1.0.0-beta.1`)

> **Document Status:** `Published Beta Testing Guide (v1.0.0-beta.1)`  
> **Target Release Package:** [`rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz`](rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz)  
> **Executable Binary:** [`rust/dist/discord-gpui`](rust/dist/discord-gpui) (`42 MB`, stripped ELF 64-bit, Thin LTO, `opt-level = 3`)  
> **Checksums:** [`rust/dist/SHA256SUMS`](rust/dist/SHA256SUMS)  

---

## 1. System Requirements & Supported Linux Distributions

The native **`discord-gpui`** client is built on a 100% Rust UI stack (**`gpui-kit` + WGPU**) rendering at 120 FPS without Electron or Chromium. RAM usage averages **11–35 MB VmRSS** (under a strict budget limit of `< 60 MB`).

### 1.1. Minimum and Recommended Requirements

| Component | Minimum Requirements | Recommended (120 FPS + Go Live 60 FPS) |
| :--- | :--- | :--- |
| **CPU Architecture** | `x86_64` (`amd64`), 2 cores (SSE4.2) | `x86_64` (4+ cores, AVX2 support for software codecs) |
| **RAM** | `512 MB` free RAM (client uses `< 60 MB` VmRSS) | `4 GB+` RAM |
| **Linux Kernel & C Library** | Linux Kernel `5.15+`, **`GLIBC_2.35`** (`glibc >= 2.35`) | Linux Kernel `6.6+` / `6.11+`, `glibc 2.38+` |
| **Graphics API (GPU / Renderer)** | OpenGL 3.3 / Mesa **`llvmpipe`** (software fallback) | **Vulkan 1.2+** (Mesa RADV / Intel ANV / NVIDIA 535+) |
| **Audio Subsystem** | ALSA (`libasound.so.2`) / PulseAudio | **PipeWire 1.0+** + WirePlumber (`cpal` 48 kHz stereo) |
| **Secrets & Notifications** | D-Bus (`libdbus-1.so.3`) | `org.freedesktop.secrets` (`gnome-keyring` / `kwalletd6`) + `org.freedesktop.Notifications` |

### 1.2. Tested Linux Distributions & Desktop Environments

- **Ubuntu**: 22.04 LTS (Jammy), 24.04 LTS (Noble)
- **Debian**: 12 (Bookworm), testing (Trixie)
- **Fedora**: 39, 40, 41 (Workstation)
- **Arch Linux**: Rolling (`core`, `extra`, `multilib`)
- **Desktop Environments**: GNOME 42–47 (Wayland/X11), KDE Plasma 5.27 / 6.0–6.2 (Wayland/X11), Hyprland / Sway (wlroots), X11 Window Managers (i3, bspwm, XFCE).

---

## 2. Quick Start: Verify and Launch

### 2.1. Verify Package Checksums
```bash
# Verify the release archive integrity
sha256sum -c rust/dist/SHA256SUMS
```

### 2.2. Launch in One Command
```bash
# Extract and launch the native binary
tar -xzf rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz
cd discord-gpui-v1.0.0-beta.1/
./discord-gpui
```

### 2.3. Run Quick Headless Self-Diagnostics
```bash
# Verify UI initialization, WGPU pipeline, and SQLite store
./discord-gpui --headless-smoke
```

---

## 3. Debugging & Logging Instructions

Enable detailed logs for troubleshooting:

```bash
# Run with informational logs
RUST_LOG=info ./discord-gpui

# Run with full debug tracing for WebRTC and Gateway
RUST_LOG=debug,discord_voice=trace,discord_core=trace ./discord-gpui
```

*Note: All authentication tokens and passwords are automatically redacted (`mfa.[REDACTED]`) in all log files.*

---

## 4. Bug Report Template

When submitting an issue on GitHub, please use the following template:

```markdown
### Bug Description
A clear and concise description of what the bug is.

### Environment
- **OS / Distribution:** (e.g. Ubuntu 24.04 LTS, Arch Linux)
- **Desktop Environment & Display Server:** (e.g. GNOME 46 Wayland, KDE Plasma 6 X11, Hyprland)
- **GPU & Driver:** (e.g. AMD Radeon 780M Mesa 24.2, NVIDIA RTX 4070 550.78, Intel Iris Xe)
- **Audio Subsystem:** (e.g. PipeWire 1.0.5, PulseAudio 16.1)
- **Client Version:** `v1.0.0-beta.1`

### Steps to Reproduce
1. Launch `./discord-gpui`
2. Connect to voice channel 'Lounge'
3. Start screen share
4. See error

### Expected Behavior
Screen share should stream at 60 FPS without frame drops.

### Actual Behavior
Stream freezes after 5 seconds.

### Relevant Logs
```text
(Paste sanitized logs from RUST_LOG=info)
```

### GDB Backtrace (If Crash Occurred)
```bash
gdb -batch -ex "thread apply all bt full" ./discord-gpui core
```
```

### Bug Severity Classification:
- **P0 (Critical)**: Crash on startup, authentication failure, data loss, audio pipeline stall.
- **P1 (High)**: Screen share failure, broken message sending, disconnection without auto-resume.
- **P2 (Medium)**: Visual glitches, minor layout misalignment, missing translation strings.
- **P3 (Low)**: Minor cosmetic inconsistencies, non-critical telemetry delays.
