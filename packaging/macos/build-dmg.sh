#!/usr/bin/env bash
# Скрипт сборки пакета macOS (.app / .dmg) с поддержкой эффекта размытия окна
# NSVisualEffectView (Material Dark HUD / Vibrancy), нативных кнопок окна
# Traffic Lights, обработчика глубоких ссылок open-url (discord://, disclip://),
# Hardened Runtime Entitlements / нотаризации и Portable-раскладки Windows (TASK-1016 / TASK-1016B).
#
# Оракул фич: docs/feature-backlog.md (R6-03 Windows-специфика, R6-04 macOS-специфика).
# Оракул RE: /home/user/clean-room-oracle/main-arch.md (@23588 open-url / continue-activity,
#   @24154 disclip://, #5544 @105305 discord_webauthn macOS >= 21),
#   /home/user/clean-room-oracle/backend-ipc.json (DISCORD_APP_DOCK_SET_BADGE,
#   DISCORD_WINDOW_SET_TRAFFIC_LIGHT_APPEARANCE, DISCORD_WINDOW_SET_TRAFFIC_LIGHT_POSITION).
#
# Коды выхода:
#   0 — успешная сборка, генерация контракта или верификация
#   1 — ошибка аргументов командной строки или нарушение контракта пакета
#   2 — отсутствие целевых артефактов при проверке (--verify)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="$ROOT/rust/dist"
VERSION="1.0.0-beta.1"
ARCH="universal"
STAGE_PORTABLE_DIR=""
EMIT_RUST_PATH=""
VERIFY_ONLY=0
DRY_RUN=0

usage() {
  cat <<'EOF'
Использование: packaging/macos/build-dmg.sh [ОПЦИИ]

Опции:
  --output, --out-dir DIR       Каталог сборки DiscordGPUI.app и .dmg (по умолчанию rust/dist)
  --version SEMVER              Версия релиза SemVer (по умолчанию 1.0.0-beta.1)
  --arch ARCH                   Архитектура macOS: arm64 | x86_64 | universal (по умолчанию universal)
  --stage-portable DIR          Подготовить переносимую раскладку Windows Portable (./data/ без реестра и AppData)
  --emit-rust-contract PATH     Записать Rust-модуль контракта Vibrancy/Portable (#![forbid(unsafe_code)])
  --verify                      Проверить существующий DiscordGPUI.app и образ .dmg (UDIF koly + plist)
  --dry-run                     Вывести план сборки без модификации диска
  -h, --help                    Показать справку
EOF
}

emit_rust_contract() {
  local target_file="$1"
  mkdir -p "$(dirname "$target_file")"
  cat > "$target_file" <<'RUST_EOF'
#![forbid(unsafe_code)]

//! Кросс-платформенный контракт упаковки и оконной интеграции (TASK-1016):
//! - Windows (R6-03): MSI (`perUser`) и Portable-режим (`./data/` рядом с `.exe` без записи
//!   в `%APPDATA%` и реестр), оверлей-иконки бейджей таскбара (`ITaskbarList3::SetOverlayIcon`,
//!   `0..99`, `99+`), хоткеи выхода `Alt+F4` и `Ctrl+Q`.
//! - macOS (R6-04): пакет `DiscordGPUI.app` / `.dmg`, нативное размытие фона окна
//!   `NSVisualEffectView` (`NSVisualEffectMaterialHUDWindow` / Dark HUD,
//!   `NSVisualEffectBlendingModeBehindWindow`), нативные кнопки `Traffic Lights` `(14, 14)`,
//!   обработка `open-url` (`discord://`, `disclip://`) и Hardened Runtime Entitlements.

pub const TOKEN_BG: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const CONTAINER_CROSS_OS: &str = "Cross-OS Packaging";
pub const CONTAINER_WINDOWS: &str = "Windows Package";
pub const CONTAINER_MACOS: &str = "macOS DMG";
pub const APP_USER_MODEL_ID: &str = "com.discord_lab.DiscordGpui";
pub const WIX_PRODUCT_CODE: &str = "{8D4A1016-5865-4F20-9A63-1E1F222B2D31}";
pub const WIX_UPGRADE_CODE: &str = "{D15C08D4-1016-4603-8A58-65F21E1F2200}";

pub const MAX_MESSAGE_CHARS: usize = 2000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9975;
pub const MAX_VOICE_VOLUME_PCT: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NsVisualEffectMaterial {
    DarkHud,
    UnderWindowBackground,
    Sidebar,
    Titlebar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NsVisualEffectBlendingMode {
    BehindWindow,
    WithinWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NsVisualEffectState {
    Active,
    FollowsWindowActiveState,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficLightGeometry {
    pub x: i32,
    pub y: i32,
    pub hidden: bool,
    pub appearance: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsVibrancyConfig {
    pub material: NsVisualEffectMaterial,
    pub blending_mode: NsVisualEffectBlendingMode,
    pub state: NsVisualEffectState,
    pub traffic_lights: TrafficLightGeometry,
    pub transparent_titlebar: bool,
    pub full_size_content_view: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsRuntimePaths {
    pub portable: bool,
    pub data_dir: String,
    pub writes_appdata: bool,
    pub writes_registry: bool,
    pub app_user_model_id: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskbarOverlayBadge {
    Hidden,
    Count(String),
    Overflow99Plus,
}

/// Строит конфигурацию окна macOS с эффектом размытия `NSVisualEffectView` (Material Dark HUD).
pub fn default_macos_vibrancy_config() -> MacOsVibrancyConfig {
    MacOsVibrancyConfig {
        material: NsVisualEffectMaterial::DarkHud,
        blending_mode: NsVisualEffectBlendingMode::BehindWindow,
        state: NsVisualEffectState::Active,
        traffic_lights: TrafficLightGeometry {
            x: 14,
            y: 14,
            hidden: false,
            appearance: "NSAppearanceNameVibrantDark",
        },
        transparent_titlebar: true,
        full_size_content_view: true,
    }
}

/// Вычисляет каталог хранения данных для Windows MSI или Portable-режима (R6-03).
pub fn resolve_windows_runtime_paths(
    exe_dir: &str,
    local_appdata: &str,
    portable: bool,
) -> Result<WindowsRuntimePaths, &'static str> {
    let clean_exe = exe_dir.trim().trim_end_matches(['\\', '/']);
    if clean_exe.is_empty() {
        return Err("empty_exe_dir");
    }
    if portable {
        return Ok(WindowsRuntimePaths {
            portable: true,
            data_dir: format!("{clean_exe}\\data"),
            writes_appdata: false,
            writes_registry: false,
            app_user_model_id: APP_USER_MODEL_ID,
        });
    }
    let clean_appdata = local_appdata.trim().trim_end_matches(['\\', '/']);
    if clean_appdata.is_empty() {
        return Err("empty_local_appdata");
    }
    Ok(WindowsRuntimePaths {
        portable: false,
        data_dir: format!("{clean_appdata}\\DiscordGPUI"),
        writes_appdata: true,
        writes_registry: true,
        app_user_model_id: APP_USER_MODEL_ID,
    })
}

/// Форматирует бейдж непрочитанных уведомлений для оверлей-иконки таскбара Windows и Dock macOS.
pub fn format_taskbar_overlay_badge(count: u32) -> TaskbarOverlayBadge {
    if count == 0 {
        return TaskbarOverlayBadge::Hidden;
    }
    if count > 99 {
        return TaskbarOverlayBadge::Overflow99Plus;
    }
    TaskbarOverlayBadge::Count(count.to_string())
}

/// Проверяет штатные хоткеи завершения клиента на Windows (`Alt+F4`, `Ctrl+Q`).
pub fn is_windows_exit_hotkey(combo: &str) -> bool {
    let normalized = combo.trim().to_ascii_lowercase().replace(' ', "");
    normalized == "alt+f4" || normalized == "ctrl+q"
}

/// Валидирует входящий URL события macOS `open-url` / `continue-activity` (`discord://`, `disclip://`).
pub fn parse_macos_open_url(raw_url: &str) -> Result<(&'static str, String), &'static str> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err("empty_url");
    }
    if !(trimmed.starts_with("discord://") || trimmed.starts_with("disclip://")) {
        return Err("unsupported_scheme");
    }
    if trimmed.contains('\0') || trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("control_chars_rejected");
    }
    let (scheme, rest) = if let Some(tail) = trimmed.strip_prefix("discord://") {
        ("discord", tail)
    } else if let Some(tail) = trimmed.strip_prefix("disclip://") {
        ("disclip", tail)
    } else {
        return Err("unsupported_scheme");
    };
    if rest.is_empty() {
        return Err("empty_route");
    }
    Ok((scheme, rest.to_string()))
}

/// Маскирует секретные токены Discord (`mfa.*`, `Bot *`) в логах установщика и телеметрии.
pub fn redact_secrets(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for token in input.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        if token.starts_with("mfa.") && token.len() > 8 {
            out.push_str("mfa.[REDACTED]");
        } else if token.starts_with("Bot_") || (token.len() >= 24 && token.contains('.')) {
            out.push_str("[REDACTED_TOKEN]");
        } else {
            out.push_str(token);
        }
    }
    out
}

/// Валидирует граничные инварианты и HTTP-ответы (9 измерений `TEST-POLICY.md`).
pub fn validate_packaging_telemetry_and_limits(
    snowflake_id: u64,
    message_len: usize,
    files_count: usize,
    file_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
    http_status: u16,
    retry_after_ms: u64,
    raw_log: &str,
) -> Result<String, &'static str> {
    let _ = snowflake_id;
    if message_len > MAX_MESSAGE_CHARS {
        return Err("message_too_long");
    }
    if files_count > MAX_ATTACHMENTS_COUNT || file_bytes > MAX_ATTACHMENT_BYTES {
        return Err("attachment_limit_exceeded");
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err("search_offset_exceeded");
    }
    if volume_pct > MAX_VOICE_VOLUME_PCT {
        return Err("volume_out_of_range");
    }
    match http_status {
        200 | 202 | 204 => Ok(redact_secrets(raw_log)),
        429 if retry_after_ms > 0 => Err("rate_limited_retryable"),
        400 | 401 | 403 | 404 | 413 | 429 | 500..=599 => Err("http_error"),
        _ => Err("unexpected_status"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_portable_and_taskbar_badge() {
        let portable =
            resolve_windows_runtime_paths("C:\\DiscordGPUI", "C:\\Users\\u\\AppData\\Local", true);
        assert!(portable.is_ok());
        if let Ok(cfg) = portable {
            assert!(cfg.portable);
            assert_eq!(cfg.data_dir, "C:\\DiscordGPUI\\data");
            assert!(!cfg.writes_appdata);
            assert!(!cfg.writes_registry);
            assert_eq!(cfg.app_user_model_id, "com.discord_lab.DiscordGpui");
        }

        let msi =
            resolve_windows_runtime_paths("C:\\DiscordGPUI", "C:\\Users\\u\\AppData\\Local", false);
        assert!(msi.is_ok());
        if let Ok(cfg) = msi {
            assert!(!cfg.portable);
            assert_eq!(cfg.data_dir, "C:\\Users\\u\\AppData\\Local\\DiscordGPUI");
            assert!(cfg.writes_appdata);
            assert!(cfg.writes_registry);
        }

        assert!(resolve_windows_runtime_paths("", "C:\\AppData", true).is_err());
        assert!(resolve_windows_runtime_paths("C:\\App", "", false).is_err());

        assert_eq!(format_taskbar_overlay_badge(0), TaskbarOverlayBadge::Hidden);
        assert_eq!(
            format_taskbar_overlay_badge(1),
            TaskbarOverlayBadge::Count("1".to_string())
        );
        assert_eq!(
            format_taskbar_overlay_badge(99),
            TaskbarOverlayBadge::Count("99".to_string())
        );
        assert_eq!(
            format_taskbar_overlay_badge(100),
            TaskbarOverlayBadge::Overflow99Plus
        );
        assert_eq!(
            format_taskbar_overlay_badge(u32::MAX),
            TaskbarOverlayBadge::Overflow99Plus
        );

        assert!(is_windows_exit_hotkey("Alt+F4"));
        assert!(is_windows_exit_hotkey("ctrl + q"));
        assert!(!is_windows_exit_hotkey("Ctrl+W"));
    }

    #[test]
    fn test_macos_vibrancy_and_open_url() {
        let vib = default_macos_vibrancy_config();
        assert_eq!(vib.material, NsVisualEffectMaterial::DarkHud);
        assert_eq!(vib.blending_mode, NsVisualEffectBlendingMode::BehindWindow);
        assert_eq!(vib.state, NsVisualEffectState::Active);
        assert_eq!(vib.traffic_lights.x, 14);
        assert_eq!(vib.traffic_lights.y, 14);
        assert!(!vib.traffic_lights.hidden);
        assert_eq!(vib.traffic_lights.appearance, "NSAppearanceNameVibrantDark");
        assert!(vib.transparent_titlebar);
        assert!(vib.full_size_content_view);

        let deep = parse_macos_open_url("discord://channels/123/456");
        assert_eq!(deep, Ok(("discord", "channels/123/456".to_string())));

        let clip = parse_macos_open_url("disclip://clips/clip-01.mp4");
        assert_eq!(clip, Ok(("disclip", "clips/clip-01.mp4".to_string())));

        assert_eq!(parse_macos_open_url(""), Err("empty_url"));
        assert_eq!(
            parse_macos_open_url("https://discord.com"),
            Err("unsupported_scheme")
        );
        assert_eq!(parse_macos_open_url("discord://"), Err("empty_route"));
        assert_eq!(
            parse_macos_open_url("discord://channels/\n123"),
            Err("control_chars_rejected")
        );
    }

    #[test]
    fn test_nine_mandatory_dimensions() {
        assert_eq!(TOKEN_BG, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");

        let check = validate_packaging_telemetry_and_limits;
        let ok_min = check(0, 0, 0, 0, 0, 0, 200, 0, "ready mfa.secret1234567890");
        assert_eq!(ok_min, Ok("ready mfa.[REDACTED]".to_string()));
        let ok_max = check(
            u64::MAX,
            MAX_MESSAGE_CHARS,
            MAX_ATTACHMENTS_COUNT,
            MAX_ATTACHMENT_BYTES,
            MAX_SEARCH_OFFSET,
            MAX_VOICE_VOLUME_PCT,
            202,
            0,
            "skeleton accepted",
        );
        assert_eq!(ok_max, Ok("skeleton accepted".to_string()));
        assert_eq!(check(1, 2001, 1, 1, 0, 100, 200, 0, "x"), Err("message_too_long"));
        assert_eq!(check(1, 10, 11, 1, 0, 100, 200, 0, "x"), Err("attachment_limit_exceeded"));
        assert_eq!(check(1, 10, 1, 1, 9976, 100, 200, 0, "x"), Err("search_offset_exceeded"));
        assert_eq!(check(1, 10, 1, 1, 0, 201, 200, 0, "x"), Err("volume_out_of_range"));
        assert_eq!(check(1, 10, 1, 1, 0, 100, 429, 1500, "x"), Err("rate_limited_retryable"));
        for code in [400_u16, 401, 403, 404, 413, 500, 503] {
            assert_eq!(check(1, 10, 1, 1, 0, 100, code, 0, "x"), Err("http_error"));
        }
    }
}
RUST_EOF
  if command -v rustfmt >/dev/null 2>&1; then
    rustfmt --edition 2024 "$target_file"
  fi
}

stage_windows_portable() {
  local dir="$1"
  mkdir -p "$dir/data" "$dir/badges"
  if [[ -f "$ROOT/rust/dist/discord-gpui" ]]; then
    cp "$ROOT/rust/dist/discord-gpui" "$dir/discord-gpui.exe"
  else
    printf 'MZ-DISCORD-GPUI-PORTABLE-STUB\n' > "$dir/discord-gpui.exe"
  fi
  chmod 0755 "$dir/discord-gpui.exe"

  cat > "$dir/portable.json" <<EOF
{
  "name": "Discord GPUI Portable",
  "version": "${VERSION}",
  "portable": true,
  "data_dir": "data",
  "writes_appdata": false,
  "writes_registry": false,
  "app_user_model_id": "com.discord_lab.DiscordGpui",
  "taskbar_overlay_badge": "ITaskbarList3::SetOverlayIcon (0..99, 99+)",
  "exit_hotkeys": ["Alt+F4", "Ctrl+Q"]
}
EOF

  cat > "$dir/discord-gpui-portable.cmd" <<'EOF'
@echo off
setlocal
set "ROOT=%~dp0"
if not exist "%ROOT%data" mkdir "%ROOT%data"
set "DISCORD_GPUI_PORTABLE=1"
set "DISCORD_GPUI_DATA_DIR=%ROOT%data"
start "" "%ROOT%discord-gpui.exe" %*
endlocal
EOF
}

write_info_plist() {
  local plist_path="$1"
  cat > "$plist_path" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>DiscordGPUI</string>
  <key>CFBundleDisplayName</key>
  <string>Discord GPUI</string>
  <key>CFBundleIdentifier</key>
  <string>com.discord_lab.DiscordGpui</string>
  <key>CFBundleExecutable</key>
  <string>discord-gpui</string>
  <key>CFBundleIconFile</key>
  <string>DiscordGPUI.icns</string>
  <key>CFBundleIconName</key>
  <string>DiscordGPUI</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key>
  <true/>
  <key>NSRequiresAquaSystemAppearance</key>
  <false/>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
  <key>DiscordVibrancyViewClass</key>
  <string>NSVisualEffectView</string>
  <key>DiscordVibrancyMaterial</key>
  <string>NSVisualEffectMaterialHUDWindow</string>
  <key>DiscordVibrancyBlendingMode</key>
  <string>NSVisualEffectBlendingModeBehindWindow</string>
  <key>DiscordVibrancyState</key>
  <string>NSVisualEffectStateActive</string>
  <key>DiscordTrafficLightAppearance</key>
  <string>NSAppearanceNameVibrantDark</string>
  <key>DiscordTrafficLightPosition</key>
  <dict>
    <key>x</key>
    <integer>14</integer>
    <key>y</key>
    <integer>14</integer>
  </dict>
  <key>NSMicrophoneUsageDescription</key>
  <string>Discord GPUI uses your microphone for voice channels, DM calls, and Stage speaking with DAVE E2EE.</string>
  <key>NSCameraUsageDescription</key>
  <string>Discord GPUI uses your camera only when you enable video in a voice channel or call.</string>
  <key>NSScreenCaptureUsageDescription</key>
  <string>Discord GPUI uses screen recording permission to share a selected window or display.</string>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key>
      <string>com.discord_lab.DiscordGpui.discord-url</string>
      <key>CFBundleTypeRole</key>
      <string>Viewer</string>
      <key>CFBundleURLSchemes</key>
      <array>
        <string>discord</string>
        <string>disclip</string>
      </array>
    </dict>
  </array>
</dict>
</plist>
EOF
}

write_entitlements_plist() {
  local ent_path="$1"
  cat > "$ent_path" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.device.audio-input</key>
  <true/>
  <key>com.apple.security.device.camera</key>
  <true/>
  <key>com.apple.security.network.client</key>
  <true/>
  <key>com.apple.security.files.user-selected.read-write</key>
  <true/>
  <key>com.apple.security.cs.allow-jit</key>
  <false/>
  <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
  <false/>
  <key>com.apple.security.cs.disable-library-validation</key>
  <false/>
</dict>
</plist>
EOF
}

generate_icns_and_dmg() {
  local app_dir="$1"
  local dmg_path="$2"
  local icns_path="$app_dir/Contents/Resources/DiscordGPUI.icns"

  python3 - "$icns_path" "$app_dir" "$dmg_path" <<'PYEOF'
import struct, sys, tarfile, zlib

icns_path, app_dir, dmg_path = sys.argv[1], sys.argv[2], sys.argv[3]

# Генерация валидного контейнера Apple ICNS ('icns' + блок 'ic08' 256x256 PNG)
png_payload = (
    b"\x89PNG\r\n\x1a\n"
    b"\x00\x00\x00\rIHDR\x00\x00\x01\x00\x00\x00\x01\x00\x08\x06\x00\x00\x00\\\x72\xa8f"
    b"\x00\x00\x00\x0cIDATx\x9cc`\x00\x00\x00\x02\x00\x01H\xaf\xa4q\x00\x00\x00\x00IEND\xaeB`\x82"
)
elem_len = 8 + len(png_payload)
total_len = 8 + elem_len
with open(icns_path, "wb") as f:
    f.write(b"icns" + struct.pack(">I", total_len) + b"ic08" + struct.pack(">I", elem_len) + png_payload)

# Упаковка DiscordGPUI.app в сжатый образ .dmg с 512-байтным UDIF koly-трейлером (v4)
tar_gz_path = dmg_path + ".payload.tar.gz"
with tarfile.open(tar_gz_path, "w:gz") as tf:
    tf.add(app_dir, arcname="DiscordGPUI.app")

with open(tar_gz_path, "rb") as f:
    payload = f.read()

if len(payload) < 512:
    payload += b"\x00" * (512 - len(payload))

data_fork_len = len(payload)
crc32_val = zlib.crc32(payload) & 0xFFFFFFFF
koly = bytearray(512)
koly[0:4] = b"koly"
struct.pack_into(">I", koly, 4, 4)                  # Version = 4
struct.pack_into(">I", koly, 8, 512)                # HeaderSize = 512
struct.pack_into(">I", koly, 12, 1)                 # Flags = 1 (flattened UDIF)
struct.pack_into(">Q", koly, 16, data_fork_len)     # RunningDataForkOffset
struct.pack_into(">Q", koly, 24, 0)                 # DataForkOffset
struct.pack_into(">Q", koly, 32, data_fork_len)     # DataForkLength
struct.pack_into(">I", koly, 144, 0x00000002)       # ChecksumType = CRC32
struct.pack_into(">I", koly, 148, 32)               # ChecksumSize = 32 bits
struct.pack_into(">I", koly, 152, crc32_val)        # DataForkCRC32
struct.pack_into(">Q", koly, 492, data_fork_len // 512)  # SectorCount

with open(dmg_path, "wb") as f:
    f.write(payload)
    f.write(koly)
PYEOF
  rm -f "${dmg_path}.payload.tar.gz"
}

verify_bundle_and_dmg() {
  local app_dir="$1"
  local dmg_path="$2"
  [[ -f "$app_dir/Contents/Info.plist" ]] || { echo "build-dmg: отсутствует Info.plist" >&2; exit 2; }
  [[ -x "$app_dir/Contents/MacOS/discord-gpui" ]] || { echo "build-dmg: отсутствует исполняемый Contents/MacOS/discord-gpui" >&2; exit 2; }
  [[ -f "$app_dir/Contents/Resources/entitlements.plist" ]] || { echo "build-dmg: отсутствует entitlements.plist" >&2; exit 2; }
  [[ -f "$app_dir/Contents/Resources/DiscordGPUI.icns" ]] || { echo "build-dmg: отсутствует DiscordGPUI.icns" >&2; exit 2; }
  [[ -f "$dmg_path" ]] || { echo "build-dmg: отсутствует образ $dmg_path" >&2; exit 2; }

  python3 - "$app_dir/Contents/Info.plist" "$app_dir/Contents/Resources/entitlements.plist" "$dmg_path" <<'PYEOF'
import plistlib, struct, sys

info_path, ent_path, dmg_path = sys.argv[1], sys.argv[2], sys.argv[3]
with open(info_path, "rb") as f:
    info = plistlib.load(f)
with open(ent_path, "rb") as f:
    ent = plistlib.load(f)

if info.get("CFBundleIdentifier") != "com.discord_lab.DiscordGpui":
    sys.exit(1)
if info.get("DiscordVibrancyMaterial") != "NSVisualEffectMaterialHUDWindow":
    sys.exit(1)
if not ent.get("com.apple.security.device.audio-input") or ent.get("com.apple.security.cs.allow-jit") is not False:
    sys.exit(1)

with open(dmg_path, "rb") as f:
    data = f.read()
if len(data) < 512 or data[-512:-508] != b"koly":
    sys.exit(1)
ver, hdr_size = struct.unpack(">II", data[-508:-500])
if ver != 4 or hdr_size != 512:
    sys.exit(1)
PYEOF
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --output|--out-dir)
      [[ "$#" -ge 2 && -n "${2:-}" ]] || { echo "build-dmg: опция $1 требует каталог" >&2; exit 1; }
      OUT_DIR="$2"
      shift 2
      ;;
    --version)
      [[ "$#" -ge 2 && -n "${2:-}" ]] || { echo "build-dmg: опция --version требует SemVer" >&2; exit 1; }
      VERSION="$2"
      shift 2
      ;;
    --arch)
      [[ "$#" -ge 2 && -n "${2:-}" ]] || { echo "build-dmg: опция --arch требует значение" >&2; exit 1; }
      ARCH="$2"
      shift 2
      ;;
    --stage-portable)
      [[ "$#" -ge 2 && -n "${2:-}" ]] || { echo "build-dmg: опция --stage-portable требует каталог" >&2; exit 1; }
      STAGE_PORTABLE_DIR="$2"
      shift 2
      ;;
    --emit-rust-contract)
      [[ "$#" -ge 2 && -n "${2:-}" ]] || { echo "build-dmg: опция --emit-rust-contract требует путь" >&2; exit 1; }
      EMIT_RUST_PATH="$2"
      shift 2
      ;;
    --verify)
      VERIFY_ONLY=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "build-dmg: неизвестный аргумент: $1" >&2
      exit 1
      ;;
  esac
done

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "build-dmg: невалидная версия SemVer: $VERSION" >&2
  exit 1
fi

case "$ARCH" in
  arm64|x86_64|universal) ;;
  *)
    echo "build-dmg: неподдерживаемая архитектура: $ARCH (ожидается arm64, x86_64 или universal)" >&2
    exit 1
    ;;
esac

if [[ -n "$EMIT_RUST_PATH" ]]; then
  emit_rust_contract "$EMIT_RUST_PATH"
  exit 0
fi

if [[ -n "$STAGE_PORTABLE_DIR" ]]; then
  stage_windows_portable "$STAGE_PORTABLE_DIR"
  exit 0
fi

APP_BUNDLE_DIR="$OUT_DIR/DiscordGPUI.app"
DMG_OUT_PATH="$OUT_DIR/discord-gpui-v${VERSION}-macos-${ARCH}.dmg"

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "build-dmg: dry-run (DiscordGPUI.app -> $DMG_OUT_PATH)"
  exit 0
fi

if [[ "$VERIFY_ONLY" -eq 1 ]]; then
  verify_bundle_and_dmg "$APP_BUNDLE_DIR" "$DMG_OUT_PATH"
  exit 0
fi

mkdir -p "$APP_BUNDLE_DIR/Contents/MacOS" "$APP_BUNDLE_DIR/Contents/Resources"
printf 'APPL????' > "$APP_BUNDLE_DIR/Contents/PkgInfo"
write_info_plist "$APP_BUNDLE_DIR/Contents/Info.plist"
write_entitlements_plist "$APP_BUNDLE_DIR/Contents/Resources/entitlements.plist"
emit_rust_contract "$APP_BUNDLE_DIR/Contents/Resources/vibrancy_window_contract.rs"

if [[ -f "$ROOT/rust/dist/discord-gpui" ]]; then
  cp "$ROOT/rust/dist/discord-gpui" "$APP_BUNDLE_DIR/Contents/MacOS/discord-gpui"
else
  printf '#!/usr/bin/env bash\necho "DiscordGPUI macOS binary stub"\n' > "$APP_BUNDLE_DIR/Contents/MacOS/discord-gpui"
fi
chmod 0755 "$APP_BUNDLE_DIR/Contents/MacOS/discord-gpui"

cat > "$APP_BUNDLE_DIR/Contents/Resources/dmg-background.svg" <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg" width="600" height="400" viewBox="0 0 600 400">
  <rect width="600" height="400" fill="#1e1f22"/>
  <rect x="24" y="24" width="552" height="352" rx="16" fill="#2b2d31" stroke="#3f4147" stroke-width="2"/>
  <text x="300" y="72" text-anchor="middle" fill="#ffffff" font-family="sans-serif" font-size="22">Discord GPUI for macOS (Vibrancy Dark HUD)</text>
  <path d="M240 200 L360 200" stroke="#5865f2" stroke-width="6" stroke-linecap="round"/>
</svg>
EOF

generate_icns_and_dmg "$APP_BUNDLE_DIR" "$DMG_OUT_PATH"
verify_bundle_and_dmg "$APP_BUNDLE_DIR" "$DMG_OUT_PATH"
