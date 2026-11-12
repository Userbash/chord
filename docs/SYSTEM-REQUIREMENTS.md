# System Requirements, Library Compatibility (`ldd` / `GLIBC <= 2.35`), and `llvmpipe` Verification (`docs/SYSTEM-REQUIREMENTS.md`)

- **Task:** `TASK-953` (`DevOps / Integration`, Wave RW-45 Beta Promotion)
- **Subtasks:** `SUB-953-1`, `SUB-953-2`, `SUB-953-3`
- **Verification Status:** `VERIFIED (PASS)`
- **Target Artifact:** [`rust/dist/discord-gpui`](rust/dist/discord-gpui) (`43,593,624` bytes, stripped 64-bit LSB PIE ELF, thin LTO, SHA-256: `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e`)
- **Automated Verification Tool:** [`scripts/verify-system-requirements.mjs`](scripts/verify-system-requirements.mjs)
- **Contract Test:** [`tests/unit/system-requirements-953.test.mjs`](tests/unit/system-requirements-953.test.mjs)
- **Related Specifications:** [`docs/specs/BETA-PROMOTION-PLAN-AND-TASKS.md`](specs/BETA-PROMOTION-PLAN-AND-TASKS.md), [`docs/specs/STREAM-WAYLAND-GUIDE.md`](specs/STREAM-WAYLAND-GUIDE.md), `docs/re/native-re.md` (§2.1–2.2)

---

## 1. Dynamic Dependency Inspection via `ldd` and `objdump` (`SUB-953-1`)

### 1.1. Step 1.1: Shared Library List (`ldd rust/dist/discord-gpui`)

The release binary `rust/dist/discord-gpui` is compiled with `lto = "thin"`, `strip = true`, and statically embeds the `SQLite 3` storage engine (`rusqlite` with the `bundled` feature). The dynamic table (`DT_NEEDED`) contains only **8 standard system libraries** from the default Linux graphics and audio stack:

```text
$ ldd rust/dist/discord-gpui
	linux-vdso.so.1 (0x00007f783be92000)
	libxcb.so.1 => /usr/lib/libxcb.so.1 (0x00007f783be31000)
	libxkbcommon.so.0 => /usr/lib/libxkbcommon.so.0 (0x00007f783bdc3000)
	libxkbcommon-x11.so.0 => /usr/lib/libxkbcommon-x11.so.0 (0x00007f783bdb9000)
	libasound.so.2 => /usr/lib/libasound.so.2 (0x00007f78392d9000)
	libgcc_s.so.1 => /usr/lib/libgcc_s.so.1 (0x00007f78392a4000)
	libm.so.6 => /usr/lib/libm.so.6 (0x00007f783919b000)
	libc.so.6 => /usr/lib/libc.so.6 (0x00007f7838e00000)
	/lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x00007f783be94000)
	libXau.so.6 => /usr/lib/libXau.so.6 (0x00007f783bdb1000)
	libXdmcp.so.6 => /usr/lib/libXdmcp.so.6 (0x00007f783bda9000)
	libxcb-xkb.so.1 => /usr/lib/libxcb-xkb.so.1 (0x00007f7839177000)
```

| Shared Library | Dependency Type | Minimum Symbol Version (`Version References`) | Purpose in `discord-gpui` |
|---|---|---|---|
| `libc.so.6` | `DT_NEEDED` (Direct) | `GLIBC_2.34` (Mandatory) $\le \text{GLIBC\_2.35}$ | POSIX system calls, `pthread` threads, sockets, memory allocator |
| `libm.so.6` | `DT_NEEDED` (Direct) | `GLIBC_2.35` (Base mathematical ABI) | GPUI layout geometry, audio DSP filtering, animation easing curves |
| `libgcc_s.so.1` | `DT_NEEDED` (Direct) | `GCC_3.0`, `GCC_3.3`, `GCC_4.2.0` | Stack unwinding (`_Unwind_*`) during Rust panics |
| `libasound.so.2` | `DT_NEEDED` (Direct) | `ALSA_0.9` | Low-level ALSA audio input/output, PipeWire ALSA bridge |
| `libxcb.so.1` | `DT_NEEDED` (Direct) | None (Unversioned C ABI) | X11 client window management, MIT-SHM frame buffer transfer |
| `libxkbcommon.so.0` | `DT_NEEDED` (Direct) | `V_0.5.0` | Keyboard keymap compilation and Unicode input event decoding |
| `libxkbcommon-x11.so.0` | `DT_NEEDED` (Direct) | `V_0.5.0` | X11 keyboard context integration |
| `ld-linux-x86-64.so.2` | Dynamic Linker | `GLIBC_2.3` | Standard ELF 64-bit dynamic program interpreter |

> **No Missing Libraries:** `ldd` reports 0 unresolved libraries (`not found`).
> **No Proprietary Vendor Locks:** No links to proprietary GPU runtimes (`libcuda.so.1`, `libnvidia-encode.so.1`, `libamdhip64.so.6`).

---

## 2. GLIBC Symbol Version Verification (`GLIBC <= 2.35`)

To guarantee compatibility across standard LTS Linux distributions (Ubuntu 22.04 LTS, Debian 12, Fedora 38+), the release binary respects `GLIBC <= 2.35`.

```bash
# Verify max GLIBC version reference
objdump -p rust/dist/discord-gpui | grep GLIBC_ | sort -V | tail -n 1
# Expected output: GLIBC_2.35
```

---

## 3. Headless Software Rasterization Verification (`llvmpipe`) (`SUB-953-2`)

The client provides automatic fallback to software rasterization using Mesa's `llvmpipe` driver when hardware GPU acceleration is unavailable or running in automated headless CI:

```bash
LIBGL_ALWAYS_SOFTWARE=1 MESA_LOADER_DRIVER_OVERRIDE=llvmpipe GALLIUM_DRIVER=llvmpipe ./rust/dist/discord-gpui --headless-smoke
```

### 3.1. Minimum and Recommended System Requirements

| Parameter | Minimum Requirement | Recommended Specification | Notes |
|---|---|---|---|
| **Linux Kernel** | **`5.15+ LTS`** | **`6.8+` / `6.12+ LTS`** | Required for modern Wayland DMA-BUF, PipeWire, `pidfd_open`, and `epoll` |
| **C Standard Library (`glibc`)** | **`GLIBC 2.35`** | **`GLIBC 2.35` – `2.44`** | Verified on Ubuntu 22.04 LTS, Debian 12, Fedora 38+, Arch/CachyOS |
| **Processor (`CPU`)** | `x86_64` (SSE4.2, 2 cores) | `x86_64-v3` (AVX2, 4+ cores) | AVX2 accelerates software `llvmpipe` rendering and audio DSP |
| **System Memory (`RAM`)** | `256 MB` available RAM | `512 MB+` available RAM | Client `VmRSS` runs at `10.8–15.5 MB` (budget limit `< 60 MB`) |
| **Audio Subsystem** | `ALSA` (`libasound.so.2`) | **`PipeWire 0.3.65+` / `1.0+`** (with `pipewire-alsa` / `pipewire-pulse`) | Seamless hotplug and device switching |
| **Secret Storage (`Keyring`)** | Encrypted file fallback | **`libsecret` / `org.freedesktop.secrets`** (`gnome-keyring` / `KWallet`) | Secure token persistence in OS keyring |

### 3.2. Supported Linux Distributions

| Distribution | `GLIBC` Version | Linux Kernel | Required Dependency Packages | Support Tier |
|---|---|---|---|---|
| **Ubuntu 22.04 LTS (Jammy)** / **24.04 LTS (Noble)** | `2.35` / `2.39` | `5.15 LTS` / `6.8 HWE` | `sudo apt install libxcb1 libxkbcommon0 libxkbcommon-x11-0 libasound2 mesa-vulkan-drivers xdg-desktop-portal` | **Full (Tier 1)** |
| **Debian 12 (Bookworm)** / **13 (Trixie)** | `2.36` / `2.40` | `6.1 LTS` / `6.12` | `sudo apt install libxcb1 libxkbcommon0 libxkbcommon-x11-0 libasound2 mesa-vulkan-drivers xdg-desktop-portal` | **Full (Tier 1)** |
| **Fedora 38+ (Workstation 38 / 39 / 40 / 41)** | `2.37` – `2.40+` | `6.2+` – `6.11+` | `sudo dnf install libxcb libxkbcommon libxkbcommon-x11 alsa-lib mesa-vulkan-drivers xdg-desktop-portal` | **Full (Tier 1)** |
| **Arch Linux / CachyOS / EndeavourOS** | `2.39+` – `2.44` | `6.12+` / `7.3+` | `sudo pacman -S libxcb libxkbcommon libxkbcommon-x11 alsa-lib vulkan-icd-loader xdg-desktop-portal` | **Full (Tier 1)** |

### 3.3. Supported Display Servers and Compositors (`Wayland` & `X11`)

| Display Stack / Compositor | Window Protocol | Screen Capture (`ScreenShareModal`) | Tray & Notifications | HiDPI Scaling |
|---|---|---|---|---|
| **Wayland — GNOME 42–47 (`Mutter`)** | Native Wayland (`xdg-shell`) | `xdg-desktop-portal-gnome` (`org.freedesktop.portal.ScreenCast` + PipeWire DMA-BUF `NV12`) | `AppIndicator` (`org.kde.StatusNotifierWatcher`) + `org.freedesktop.Notifications` | Integer & Fractional (`wp_fractional_scale_v1`) |
| **Wayland — KDE Plasma 5.27 / 6.x (`KWin`)** | Native Wayland (`xdg-shell`) | `xdg-desktop-portal-kde` (PipeWire window/screen selector) | Native `StatusNotifierItem` + `org.freedesktop.Notifications` | Fractional scaling `125%`, `150%`, `200%` crisp |
| **Wayland — Hyprland / Sway (`wlroots`)** | Native Wayland (`xdg-shell`) | `xdg-desktop-portal-hyprland` / `xdg-desktop-portal-wlr` (PipeWire) | `Waybar` SNI + `mako` / `dunst` | Dynamic tiling `900×550` to `3840×2160` |
| **X11 (`XOrg Server 21.1+` / `Xfce` / `i3`)** | `XCB` (`libxcb.so.1` + `libxkbcommon-x11.so.0`) | Automatic fallback to **X11 MIT-SHM (`XShmGetImage`)** when Wayland portal unavailable | `XEmbed` / `StatusNotifier` + `xfce4-notifyd` / `dunst` | Scaling via `Xft.dpi` / `RANDR` |
| **Headless / Software (`Mesa llvmpipe` / `lavapipe`)** | `headless` / `Xvfb` / `cage` / `weston --backend=headless` | Emulated test frame `NV12 1280×720` | Mock bus `dbus-run-session` + `org.a11y.Bus` | Fixed profile `1280×720` (`1.0x`) |

---

## 4. Verification Command

Run automated verification for dynamic libraries (`ldd`), symbol versions (`objdump -p GLIBC <= 2.35`), vendor library isolation, and cold start on `llvmpipe`:

```sh
node scripts/verify-system-requirements.mjs --out-json .gate/system-requirements-953.json
node --test tests/unit/system-requirements-953.test.mjs
```
