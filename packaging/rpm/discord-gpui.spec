# Спецификация RPM-пакета нативного клиента Discord GPUI (TASK-966)
# Поддерживаемые семейства дистрибутивов:
#   - Fedora Workstation 38 / 39 / 40 / 41+
#   - Red Hat Enterprise Linux (RHEL) 9 / 10, AlmaLinux, Rocky Linux
#   - openSUSE Tumbleweed / openSUSE Leap 15.6+

%global debug_package %{nil}
%global _build_id_links none
%global tarball_version 1.0.0-beta.1
%global bin_sha256 d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e
%global tarball_sha256 18a6aa8066ce0b6722c8b83a61bf0c43939d5cee7696c7cd6d282ac1167a349e

Name:           discord-gpui
Version:        1.0.0
Release:        0.1.beta.1%{?dist}
Summary:        Native Linux Discord client built with Rust and GPUI (120 FPS WGPU)
License:        MIT OR Apache-2.0
URL:            https://github.com/discord-lab/discord-gpui
ExclusiveArch:  x86_64
Source0:        discord-gpui-v%{tarball_version}-linux-x86_64.tar.gz

BuildRequires:  desktop-file-utils
BuildRequires:  hicolor-icon-theme

# Базовые системные зависимости рантайма (Fedora / RHEL / openSUSE)
Requires:       pipewire-libs
Requires:       libsecret
Requires:       wayland
Requires:       vulkan-loader
Requires:       alsa-lib
Requires:       libxcb
Requires:       libxkbcommon
Requires:       libxkbcommon-x11
Requires:       xdg-desktop-portal
Requires:       hicolor-icon-theme
Requires:       desktop-file-utils

# Специфичные пакеты для Fedora и RHEL / CentOS Stream / AlmaLinux / Rocky Linux
%if 0%{?fedora} || 0%{?rhel}
Requires:       pipewire-libs >= 0.3.65
Requires:       libsecret >= 0.20
Requires:       wayland >= 1.20
Requires:       vulkan-loader >= 1.3
Requires:       mesa-vulkan-drivers
Recommends:     xdg-desktop-portal-gnome
Recommends:     xdg-desktop-portal-kde
%endif

# Специфичные имена разделяемых библиотек для openSUSE Tumbleweed / Leap 15.6+
%if 0%{?suse_version}
Requires:       libpipewire-0_3-0
Requires:       libsecret-1-0
Requires:       libwayland-client0
Requires:       libvulkan1
Recommends:     xdg-desktop-portal-gtk
%endif

Provides:       discord-gpui = %{version}-%{release}
Provides:       discord-gpui(x86-64) = %{version}-%{release}
Provides:       mimehandler(x-scheme-handler/discord)
Provides:       mimehandler(x-scheme-handler/disclip)

%description
discord-gpui is a 100% native Rust Discord desktop client for Linux built on
the gpui-kit and WGPU rendering stack (Vulkan / OpenGL Mesa llvmpipe fallback)
delivering 120 FPS UI performance with an ultra-low memory footprint (< 60 MB
VmRSS budget, ~11-18 MB typical).

Key capabilities:
- End-to-End Encrypted Voice & Video via DAVE MLS v1 (RFC 9420, Aes128GcmTruncated8).
- Native Wayland screen sharing via org.freedesktop.portal.ScreenCast (PipeWire
  DMA-BUF NV12) with seamless X11 MIT-SHM fallback.
- System keyring integration via libsecret (org.freedesktop.secrets) and local
  SQLite WAL / FTS5 message cache.
- Full WCAG 2.2 AA accessibility via AT-SPI D-Bus (org.a11y.Bus).

%prep
%setup -q -c -n %{name}-%{version}
# Flatten if tarball extracted into a versioned subdirectory
if [ -d "discord-gpui-v%{tarball_version}-linux-x86_64" ]; then
    mv discord-gpui-v%{tarball_version}-linux-x86_64/* .
fi
echo "%{bin_sha256}  discord-gpui" | sha256sum -c -

%build
# Формирование XDG Desktop Entry (Freedesktop Desktop Entry Specification 1.5)
cat << 'EOF' > discord-gpui.desktop
[Desktop Entry]
Version=1.5
Type=Application
Name=Discord GPUI
GenericName=Internet Messenger
Comment=Native Linux Discord Client (Rust GPUI 120 FPS)
Comment[ru]=Нативный клиент Discord для Linux (Rust GPUI 120 FPS)
Exec=discord-gpui %u
TryExec=discord-gpui
Icon=discord-gpui
Terminal=false
Categories=Network;InstantMessaging;Chat;
MimeType=x-scheme-handler/discord;x-scheme-handler/disclip;
StartupWMClass=discord-gpui
StartupNotify=true
Keywords=Discord;Chat;Voice;Wayland;PipeWire;GPUI;
X-GNOME-UsesNotifications=true
EOF

# Формирование масштабируемой векторной иконки (Discord Blurple #5865F2)
cat << 'EOF' > discord-gpui.svg
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256">
  <rect width="256" height="256" rx="56" fill="#5865F2"/>
  <path fill="#FFFFFF" d="M197.3 69.8a165.1 165.1 0 0 0-40.7-12.6 1.1 1.1 0 0 0-1.2.5c-1.8 3.1-3.7 7.2-5.1 10.4a152.6 152.6 0 0 0-44.6 0c-1.4-3.2-3.4-7.3-5.2-10.4a1.1 1.1 0 0 0-1.2-.5 164.7 164.7 0 0 0-40.7 12.6 1.1 1.1 0 0 0-.5.4C32.2 108.5 25.1 145.9 28.5 182.8c.1.4.3.8.6 1a166 166 0 0 0 49.9 25.2c.6.2 1.2 0 1.5-.5 3.8-5.2 7.3-10.7 10.2-16.5.3-.6 0-1.4-.6-1.6a109.4 109.4 0 0 1-15.6-7.5c-.7-.4-.7-1.4 0-1.8 1.1-.8 2.1-1.6 3.1-2.5.4-.3.9-.4 1.4-.2 32.7 15 68.1 15 100.4 0 .5-.2 1-.1 1.4.2 1 .9 2 1.7 3.1 2.5.7.4.6 1.4 0 1.8-5 2.9-10.2 5.4-15.6 7.5-.7.2-.9 1-.6 1.6 3 5.8 6.4 11.3 10.2 16.5.3.5.9.7 1.5.5a165.4 165.4 0 0 0 50-25.2c.3-.2.5-.6.6-1 4.1-42.6-6.9-79.6-32.7-112.6a.9.9 0 0 0-.5-.4ZM95.1 160.3c-9.9 0-18-9.1-18-20.2s8-20.2 18-20.2c10.1 0 18.2 9.1 18 20.2 0 11.1-8 20.2-18 20.2Zm65.8 0c-9.9 0-18-9.1-18-20.2s8-20.2 18-20.2c10.1 0 18.1 9.1 18 20.2 0 11.1-7.9 20.2-18 20.2Z"/>
</svg>
EOF

# Формирование растровой иконки 256x256 PNG (8-bit RGB #5865F2)
if [ ! -f "discord-gpui.png" ]; then
base64 -d << 'EOF' > discord-gpui.png
iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAIAAADTED8xAAACAElEQVR42u3TQQkAAAgEwYtpAfuDCXybwYFJsLCpHngrEmAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAOogAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAAMAAYAAwABgADgAHAAGAADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAANgABUwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADAAGAAOAAcAAYAAwABgADIABwABgADAAGAAMAAYAA4ABwABgADAAGAAMAAYAA4ABwABgADAAGAAMAAYAA4ABwABgADAAGAAMAAYAA4ABwABgADAAGAAMAAYAA4ABwABgADAAGAAMAAYAA8C1UbcZQm1rhfUAAAAASUVORK5CYII=
EOF
fi

%install
# 1. Установка нативного бинарника discord-gpui
install -D -m 0755 discord-gpui %{buildroot}%{_bindir}/discord-gpui

# 2. Установка файла интеграции рабочего стола XDG (.desktop)
install -D -m 0644 discord-gpui.desktop %{buildroot}%{_datadir}/applications/discord-gpui.desktop

# 3. Установка векторной и растровой иконок в тему hicolor
install -D -m 0644 discord-gpui.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/discord-gpui.svg
install -D -m 0644 discord-gpui.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/discord-gpui.png

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/discord-gpui.desktop
%{buildroot}%{_bindir}/discord-gpui --headless-smoke

%post
update-desktop-database -q %{_datadir}/applications &> /dev/null || :
gtk-update-icon-cache -q -t -f %{_datadir}/icons/hicolor &> /dev/null || :

%postun
update-desktop-database -q %{_datadir}/applications &> /dev/null || :
gtk-update-icon-cache -q -t -f %{_datadir}/icons/hicolor &> /dev/null || :

%files
%license LICENSE-MIT LICENSE-APACHE
%doc BETA-TESTING-GUIDE.md RELEASE-NOTES-v%{tarball_version}.md sbom.spdx.json
%{_bindir}/discord-gpui
%{_datadir}/applications/discord-gpui.desktop
%{_datadir}/icons/hicolor/scalable/apps/discord-gpui.svg
%{_datadir}/icons/hicolor/256x256/apps/discord-gpui.png

%changelog
* Wed Nov 11 2026 Discord Lab Packaging Team <release@discord-lab.local> - 1.0.0-0.1.beta.1
- Initial RPM package specification for Fedora, RHEL, and openSUSE (TASK-966)
- Add runtime dependencies for pipewire-libs, libsecret, wayland, and vulkan-loader
- Install XDG desktop entry with discord:// and disclip:// URI schemes and hicolor icons
