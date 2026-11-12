# System Requirements & Binary Compatibility (`docs/SYSTEM-REQUIREMENTS.en.md`)

- **Target Binary:** [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) (`42 MB`, stripped 64-bit LSB PIE ELF, Thin LTO)
- **Automated Verification:** [`scripts/verify-system-requirements.mjs`](file:///home/sanya/orca/project/scripts/verify-system-requirements.mjs)
- **Russian Version:** [`docs/SYSTEM-REQUIREMENTS.md`](SYSTEM-REQUIREMENTS.md)

---

## 1. Dynamic Library Dependencies (`ldd`)

The release binary `rust/dist/discord-gpui` is compiled with `lto = "thin"`, `strip = true`, and statically embeds `SQLite 3` (`rusqlite` bundled). The dynamic table (`DT_NEEDED`) requires only **8 standard system libraries**:

```text
$ ldd rust/dist/discord-gpui
	linux-vdso.so.1
	libxcb.so.1 => /usr/lib/libxcb.so.1
	libxkbcommon.so.0 => /usr/lib/libxkbcommon.so.0
	libxkbcommon-x11.so.0 => /usr/lib/libxkbcommon-x11.so.0
	libasound.so.2 => /usr/lib/libasound.so.2
	libgcc_s.so.1 => /usr/lib/libgcc_s.so.1
	libpthread.so.0 => /usr/lib/libpthread.so.0
	libm.so.6 => /usr/lib/libm.so.6
	libdl.so.2 => /usr/lib/libdl.so.2
	libc.so.6 => /usr/lib/libc.so.6
	ld-linux-x86-64.so.2
```

---

## 2. GLIBC Compatibility (`GLIBC <= 2.35`)

To ensure broad compatibility across long-term support distributions (such as Ubuntu 22.04 LTS and Debian 12), all symbol versions are capped at **`GLIBC_2.35`**:

```bash
# Verify max GLIBC symbol requirement
objdump -p rust/dist/discord-gpui | grep GLIBC_ | sort -V | tail -n 1
# Output: GLIBC_2.35
```

---

## 3. Graphics & Headless Verification (`llvmpipe`)

The client supports software rendering fallback via Mesa's `llvmpipe` for automated headless CI/CD and systems without dedicated GPU drivers:

```bash
# Run headless verification using software rasterizer
LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe ./discord-gpui --headless-smoke
```
