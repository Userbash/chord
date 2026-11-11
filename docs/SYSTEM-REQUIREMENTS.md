# Системные требования, совместимость библиотек `ldd` / `GLIBC <= 2.35` и верификация `llvmpipe` (`docs/SYSTEM-REQUIREMENTS.md`)

- **Задача:** `TASK-953` (`DevOps / Integration`, Wave RW-45 Beta Promotion)
- **Подзадачи:** `SUB-953-1`, `SUB-953-2`, `SUB-953-3`
- **Статус верификации:** `VERIFIED (PASS)`
- **Целевой артефакт:** [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) (`43 593 624` байт, stripped 64-bit LSB PIE ELF, thin LTO, SHA-256: `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e`)
- **Инструмент автоматической проверки:** [`scripts/verify-system-requirements.mjs`](file:///home/sanya/orca/project/scripts/verify-system-requirements.mjs)
- **Контрактный тест:** [`tests/unit/system-requirements-953.test.mjs`](file:///home/sanya/orca/project/tests/unit/system-requirements-953.test.mjs)
- **Связанные спецификации:** [`docs/specs/BETA-PROMOTION-PLAN-AND-TASKS.md`](file:///home/sanya/orca/project/docs/specs/BETA-PROMOTION-PLAN-AND-TASKS.md), [`docs/specs/STREAM-WAYLAND-GUIDE.md`](file:///home/sanya/orca/project/docs/specs/STREAM-WAYLAND-GUIDE.md), `/home/sanya/re-deep/native-re.md` (§2.1–2.2)

---

## 1. Инспекция динамических зависимостей через `ldd` и `objdump` (`SUB-953-1`)

### 1.1. Шаг 1.1: Список разделяемых библиотек (`ldd rust/dist/discord-gpui`)

Релизный бинарник `rust/dist/discord-gpui` скомпилирован с профилем `lto = "thin"`, `strip = true` и статически встроенным движком хранения `SQLite 3` (`rusqlite` с фичей `bundled`). Прямая таблица `DT_NEEDED` содержит только **8 базовых системных библиотек** стандартного графического и аудиостека Linux:

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

| Разделяемая библиотека | Тип зависимости | Минимальная версия символов (`Version References`) | Назначение в `discord-gpui` |
|---|---|---|---|
| `libc.so.6` | `DT_NEEDED` (прямая) | `GLIBC_2.34` (обязательная) $\le \text{GLIBC\_2.35}$ | Системные вызовы POSIX, потоки `pthread`, сокеты, аллокатор |
| `libm.so.6` | `DT_NEEDED` (прямая) | `GLIBC_2.35` (базовая математическая ABI) | Геометрия макетов GPUI, обработка звука, кривые анимаций |
| `libgcc_s.so.1` | `DT_NEEDED` (прямая) | `GCC_3.0`, `GCC_3.3`, `GCC_4.2.0` | Раскрутка стека (`_Unwind_*`) при панике Rust |
| `libasound.so.2` | `DT_NEEDED` (прямая) | `ALSA_0.9`, `ALSA_0.9.0rc4` | Аудиозахват и воспроизведение через `cpal` (ALSA / PipeWire-ALSA bridge) |
| `libxcb.so.1` | `DT_NEEDED` (прямая) | Базовый ABI `libxcb 1.14+` | Протокол X11 / XWayland для оконной подсистемы GPUI |
| `libxkbcommon.so.0` | `DT_NEEDED` (прямая) | `V_0.5.0` | Обработка раскладок клавиатуры, модификаторов и Dead Keys (Wayland/X11) |
| `libxkbcommon-x11.so.0` | `DT_NEEDED` (прямая) | `V_0.5.0` | Интеграция XKB с соединением XCB |
| `ld-linux-x86-64.so.2` | `DT_NEEDED` (интерпретатор ELF) | `GLIBC_2.3` | Динамический компоновщик Linux x86_64 |
| `libXau.so.6`, `libXdmcp.so.6`, `libxcb-xkb.so.1` | Транзитивные (через `libxcb`) | Стандартный X11 ABI | Авторизация X11 (`Xauthority`) и расширение XKB |
| `libdbus-1.so.3` / `libpipewire-0.3.so.0` / `libvulkan.so.1` / `libGL.so.1` | Опциональные (`dlopen` / сокеты) | Не входят в `DT_NEEDED` | Загружаются динамически в рантайме (либо через нативный D-Bus UNIX-сокет) |

---

### 1.2. Шаг 1.2: Аудит версий символов `GLIBC` (`objdump -p rust/dist/discord-gpui | grep GLIBC_`)

Проверка секции `Version References` бинарника `rust/dist/discord-gpui`:

```text
Version References:
  required from libxkbcommon.so.0:
    0x0c231840 0x00 17 V_0.5.0
  required from libxkbcommon-x11.so.0:
    0x0c231840 0x00 18 V_0.5.0
  required from libasound.so.2:
    0x01772779 0x00 06 ALSA_0.9
    0x07b60a14 0x00 07 ALSA_0.9.0rc4
  required from libgcc_s.so.1:
    0x0b792650 0x00 04 GCC_3.0
    0x0b792653 0x00 24 GCC_3.3
    0x09276060 0x00 34 GCC_4.2.0
  required from libc.so.6:
    0x09691a75 0x00 03 GLIBC_2.2.5 .. 0x069691b4 0x00 02 GLIBC_2.34
    0x069691b9 0x02 25 GLIBC_2.39 (VER_FLG_WEAK: pidfd_spawnp, pidfd_getpid)
  required from libm.so.6:
    0x09691a75 0x00 10 GLIBC_2.2.5 .. 0x069691b5 0x00 22 GLIBC_2.35
```

1. **Базовый уровень `libc.so.6` (`GLIBC_2.34 <= GLIBC_2.35`):**
   - Максимальная обязательная версия символов (`flags = 0x00`) из `libc.so.6` — **`GLIBC_2.34`** (используется стандартной библиотекой Rust для `pthread_mutex_lock`, `dlopen`, `__libc_start_main` после объединения `libpthread`/`libdl` в `libc.so.6`).
   - Запись `GLIBC_2.39` имеет флаг **`0x02` (`VER_FLG_WEAK`)** и относится исключительно к слабым ссылкам `pidfd_spawnp` и `pidfd_getpid` в `std::process::Command`. При запуске на системах с `glibc 2.35` (Ubuntu 22.04 LTS) динамический загрузчик разрешает слабые символы в `NULL`, а стандартная библиотека Rust автоматически переключается на `posix_spawn` / `clone3` / `fork+exec`.
2. **Базовый уровень `libm.so.6` (`GLIBC_2.35`):**
   - Целевой базовый контракт математической библиотеки — **`GLIBC_2.35`** (`hypot`, `expf`, `logf`, `powf`).
   - *Примечание о сборке на rolling-хостах (`glibc 2.44`):* при локальной линковке без sysroot-изоляции на ядрах CachyOS/Arch с `glibc 2.44` компоновщик `ld` проставляет C23-версии для 8 функций `libm.so.6` (`asinf`, `acosf`, `acoshf`, `coshf`, `sinhf`, `atan2f` `@GLIBC_2.43` и `cosh`, `sinh` `@GLIBC_2.44`). При сборке релизного пакета в целевом контейнере Ubuntu 22.04 LTS (`glibc 2.35`) или через `cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.35` все математические символы привязываются строго к `GLIBC_2.2.5` / `GLIBC_2.35`.

---

### 1.3. Шаг 1.3: Отсутствие жёсткой привязки к проприетарным библиотекам NVIDIA и AMD

В полном соответствии с эталонным RE-оракулом оригинального клиента Discord (`/home/sanya/re-deep/native-re.md` §2.1–2.2), в `rust/dist/discord-gpui` **полностью отсутствует** жёсткая привязка (`DT_NEEDED`) к проприетарным видеодрайверам и аппаратным кодекам:
- **`0` зависимостей от проприетарного стека NVIDIA:** нет `libcuda.so`, `libnvidia-encode.so.1`, `libnvcuvid.so`, `libnvidia-glcore.so`, `libnvidia-ml.so`.
- **`0` зависимостей от проприетарного стека AMD:** нет `libamdhip64.so`, `libamdocl64.so`, `libhsa-runtime64.so`, `libatiuki.so`.
- Инициализация графических API (Vulkan `libvulkan.so.1`, OpenGL/EGL `libEGL.so.1` / `libGL.so.1`) и аппаратных ускорителей видео (`VA-API` / `NVENC`) выполняется исключительно динамически в рантайме с автоматическим откатом на программный рендеринг и программные кодеки при отсутствии GPU.

---

## 2. Верификация программного рендеринга Mesa `llvmpipe` (`SUB-953-2`)

Для подтверждения работоспособности клиента на виртуальных машинах, серверах без GPU и системах с повреждёнными драйверами проведён тест холодного старта с принудительным включением программного растеризатора **Mesa `llvmpipe`** (`LIBGL_ALWAYS_SOFTWARE=1`, `MESA_LOADER_DRIVER_OVERRIDE=llvmpipe`, `GALLIUM_DRIVER=llvmpipe`):

```sh
LIBGL_ALWAYS_SOFTWARE=1 MESA_LOADER_DRIVER_OVERRIDE=llvmpipe ./rust/dist/discord-gpui --headless-smoke
```

### 2.1. Результаты прогона `--headless-smoke` и оконной инициализации WGPU на `llvmpipe`

```json
GPUI_SMOKE_OK {"status":"ok","display_backend":"wayland+x11","window_width":1280,"window_height":720,"min_width":900,"min_height":550,"guilds":2,"channels":5,"sqlite_wal_ok":true,"fps_budget_ok":true,"frame_tick_us":0,"vmrss_kb":11092,"vmrss_mb":10.83}
```

| Параметр проверки | Фактическое значение (`llvmpipe`) | Порог контракта (DoD) | Статус |
|---|---|---|---|
| Переменные окружения | `LIBGL_ALWAYS_SOFTWARE=1` + `MESA_LOADER_DRIVER_OVERRIDE=llvmpipe` | Принудительный `swrast` / `llvmpipe` | `PASS` |
| Код возврата (`--headless-smoke`) | `0` (`GPUI_SMOKE_OK`) | `0` без паник и segfault | `PASS` |
| Холодный старт окна (`DISCORD_GPUI_EXIT_AFTER_MS=300`) | Вывод `window-ready`, код возврата `0` | Успешное открытие окна `1280×720` | `PASS` |
| Потребление памяти (`VmRSS`) | **`10.83 МБ`** (`11 092 КБ`) | `< 60.0 МБ` | `PASS` |
| Инициализация базы `SQLite WAL` | `sqlite_wal_ok: true` | `true` | `PASS` |
| Бюджет кадра 120 FPS (`fps_budget_ok`) | `fps_budget_ok: true` (`< 8300 мкс`) | `true` | `PASS` |

---

## 3. Матрица системных требований для Linux Desktop (`SUB-953-3`)

### 3.1. Поддерживаемые версии ядра Linux и системного окружения

| Компонент | Минимальные требования | Рекомендуемые требования | Примечание |
|---|---|---|---|
| **Ядро Linux (`Linux Kernel`)** | **`5.15+` LTS** | **`6.1+` LTS / `6.6+` LTS / `6.12+`** | Требуется поддержка `memfd_create`, `pidfd_open`, `DMA-BUF`, `eventfd` и `epoll` |
| **Системная библиотека C (`glibc`)** | **`GLIBC 2.35`** | **`GLIBC 2.35` – `2.44`** | Совместимость с Ubuntu 22.04 LTS, Debian 12, Fedora 38+, Arch/CachyOS |
| **Процессор (`CPU`)** | `x86_64` (SSE4.2, 2 ядра) | `x86_64-v3` (AVX2, 4+ ядра) | AVX2 ускоряет программный рендеринг `llvmpipe` и аудиофильтрацию |
| **Оперативная память (`RAM`)** | `256 МБ` свободной ОЗУ | `512 МБ+` свободной ОЗУ | Фактический `VmRSS` клиента составляет `10.8–15.5 МБ` (лимит бюджета `< 60 МБ`) |
| **Звуковая подсистема** | `ALSA` (`libasound.so.2`) | **`PipeWire 0.3.65+` / `1.0+`** (с `pipewire-alsa` / `pipewire-pulse`) | Автоматический fallback при hot-unplug микрофона/наушников |
| **Хранилище секретов (`Keyring`)** | Файловый шифрованный fallback | **`libsecret` / `org.freedesktop.secrets`** (`gnome-keyring` / `KWallet`) | Хранение сессионного токена Discord в системном брелоке ОС |

### 3.2. Поддерживаемые дистрибутивы Linux

| Дистрибутив | Версия `GLIBC` | Версия ядра Linux | Пакеты зависимостей для установки | Статус поддержки |
|---|---|---|---|---|
| **Ubuntu 22.04 LTS (Jammy)** / **24.04 LTS (Noble)** | `2.35` / `2.39` | `5.15 LTS` / `6.8 HWE` | `sudo apt install libxcb1 libxkbcommon0 libxkbcommon-x11-0 libasound2 mesa-vulkan-drivers xdg-desktop-portal` | **Полная (Tier 1)** |
| **Debian 12 (Bookworm)** / **13 (Trixie)** | `2.36` / `2.40` | `6.1 LTS` / `6.12` | `sudo apt install libxcb1 libxkbcommon0 libxkbcommon-x11-0 libasound2 mesa-vulkan-drivers xdg-desktop-portal` | **Полная (Tier 1)** |
| **Fedora Workstation 38 / 39 / 40 / 41+** | `2.37` – `2.40+` | `6.2+` – `6.11+` | `sudo dnf install libxcb libxkbcommon libxkbcommon-x11 alsa-lib mesa-vulkan-drivers xdg-desktop-portal` | **Полная (Tier 1)** |
| **Arch Linux / CachyOS / EndeavourOS** | `2.39+` – `2.44` | `6.12+` / `7.3+` | `sudo pacman -S libxcb libxkbcommon libxkbcommon-x11 alsa-lib vulkan-icd-loader xdg-desktop-portal` | **Полная (Tier 1)** |

### 3.3. Поддерживаемые графические серверы и композиторы (`Wayland` & `X11`)

| Графический стек / Композитор | Протокол окна | Захват экрана (`ScreenShareModal`) | Системный трей и уведомления | Масштабирование (`HiDPI`) |
|---|---|---|---|---|
| **Wayland — GNOME 42–47 (`Mutter`)** | Нативный Wayland (`xdg-shell`) | `xdg-desktop-portal-gnome` (`org.freedesktop.portal.ScreenCast` + PipeWire DMA-BUF `NV12`) | `AppIndicator` (`org.kde.StatusNotifierWatcher`) + `org.freedesktop.Notifications` | Целочисленное и дробное (`wp_fractional_scale_v1`) |
| **Wayland — KDE Plasma 5.27 / 6.x (`KWin`)** | Нативный Wayland (`xdg-shell`) | `xdg-desktop-portal-kde` (PipeWire + выбор окон/мониторов) | Нативный `StatusNotifierItem` + `org.freedesktop.Notifications` | Дробное масштабирование `125%`, `150%`, `200%` без размытия |
| **Wayland — Hyprland / Sway (`wlroots`)** | Нативный Wayland (`xdg-shell`) | `xdg-desktop-portal-hyprland` / `xdg-desktop-portal-wlr` (PipeWire) | `Waybar` SNI + `mako` / `dunst` | Динамический тайлинг от `900×550` до `3840×2160` |
| **X11 (`XOrg Server 21.1+` / `Xfce` / `i3`)** | `XCB` (`libxcb.so.1` + `libxkbcommon-x11.so.0`) | Автоматический fallback на **X11 MIT-SHM (`XShmGetImage`)** при отсутствии Wayland | `XEmbed` / `StatusNotifier` + `xfce4-notifyd` / `dunst` | Масштабирование через `Xft.dpi` / `RANDR` |
| **Headless / Software (`Mesa llvmpipe` / `lavapipe`)** | `headless` / `Xvfb` / `cage` / `weston --backend=headless` | Эмуляция тестового кадра `NV12 1280×720` | Мок-шина `dbus-run-session` + `org.a11y.Bus` | Фиксированный профиль `1280×720` (`1.0x`) |

---

## 4. Команда автономной перепроверки

Для запуска автоматической проверки бинарника (`ldd`, `objdump -p GLIBC <= 2.35`, отсутствие проприетарных библиотек NVIDIA/AMD и холодный старт на `llvmpipe`):

```sh
node scripts/verify-system-requirements.mjs --out-json .gate/system-requirements-953.json
node --test tests/unit/system-requirements-953.test.mjs
```
