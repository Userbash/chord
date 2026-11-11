# Руководство бета-тестировщика `discord-gpui` (`v1.0.0-beta.1`)

> **Статус документа:** `Published Beta Testing Guide (v1.0.0-beta.1)`  
> **Задача:** `TASK-958` (`SUB-958-1`, `SUB-958-2`, `SUB-958-3`, отдел `G6 Tech Writer & Parity`)  
> **Целевой артефакт:** [`rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz`](file:///home/sanya/orca/project/rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz)  
> **Исполняемый бинарник:** [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) (`42 МБ`, stripped ELF 64-bit, Thin LTO, `opt-level = 3`)  
> **Контрольные суммы:** [`rust/dist/SHA256SUMS`](file:///home/sanya/orca/project/rust/dist/SHA256SUMS)  
> **Релизные заметки:** [`RELEASE-NOTES-v1.0.0-beta.1.md`](file:///home/sanya/orca/project/RELEASE-NOTES-v1.0.0-beta.1.md)  

---

## 1. Системные требования и поддерживаемые дистрибутивы Linux

Нативный клиент **`discord-gpui`** построен на 100% Rust UI-стеке **`gpui-kit` + WGPU** (120 FPS) без использования Electron или Chromium/WebKit для основного интерфейса. Потребление оперативной памяти составляет **11–35 МБ VmRSS** (при жестком бюджете `< 60 МБ`).

### 1.1. Минимальные и рекомендуемые аппаратные/системные требования

| Компонент | Минимальные требования | Рекомендуемая конфигурация (120 FPS + Go Live 60 FPS) |
| :--- | :--- | :--- |
| **Архитектура ЦП** | `x86_64` (`amd64`), 2 физических ядра (SSE4.2) | `x86_64` (4+ ядра, поддержка AVX2 для программных кодеков) |
| **Оперативная память (RAM)** | `512 МБ` свободной ОЗУ (клиент потребляет `< 60 МБ` VmRSS) | `4 ГБ+` ОЗУ |
| **Ядро Linux и Системная библиотека C** | Linux Kernel `5.15+`, **`GLIBC_2.35`** (`glibc >= 2.35`) | Linux Kernel `6.6+` / `6.11+`, `glibc 2.38+` |
| **Графический API (GPU / Рендерер)** | OpenGL 3.3 / Mesa **`llvmpipe`** (софтверный рендеринг без GPU) | **Vulkan 1.2+** (Mesa RADV / Intel ANV / NVIDIA 535+) |
| **Звуковая подсистема** | ALSA (`libasound.so.2`) / PulseAudio | **PipeWire 1.0+** + WirePlumber (`cpal` 48 kHz stereo) |
| **Секреты и уведомления** | D-Bus (`libdbus-1.so.3`) | `org.freedesktop.secrets` (`gnome-keyring` / `kwalletd6`) + `org.freedesktop.Notifications` |

### 1.2. Поддерживаемые дистрибутивы Linux (Проверено с `GLIBC_2.35+`)

Бинарник скомпилирован без жёсткой привязки к проприетарным драйверам NVIDIA/AMD и готов к запуску «из коробки» на следующих дистрибутивах:
- **Ubuntu 22.04 LTS (Jammy)**, **Ubuntu 24.04 LTS (Noble)** и производные (Linux Mint 21/22, Pop!_OS 22.04/24.04).
- **Debian 12 (Bookworm)** и **Debian 13 (Trixie / Sid)**.
- **Fedora Workstation 38 / 39 / 40 / 41** (GNOME и KDE Spin).
- **Arch Linux**, **EndeavourOS**, **CachyOS**, **Manjaro** (rolling-релизы).
- **openSUSE Tumbleweed** и **openSUSE Leap 15.6+**.

Минимальный набор системных разделяемых библиотек (проверяется командой `ldd ./discord-gpui`):
- `libc.so.6`, `libm.so.6`, `libgcc_s.so.1` (стандартный рантайм `glibc >= 2.35`)
- `libasound.so.2` (захват и вывод звука через ALSA/PipeWire-ALSA)
- `libdbus-1.so.3` (системная шина D-Bus: порталы захвата экрана, трей, уведомления, AT-SPI, Keyring)
- `libxcb.so.1`, `libX11.so.6` (оконный стек X11 / XWayland fallback)

### 1.3. Матрица сертифицированных графических окружений (Wayland & X11)

Клиент сертифицирован на 4 эталонных стеках Linux Desktop:

| Графическое окружение | Протокол сессии | Захват экрана (Go Live 60 FPS) | Трей и системные интеграции |
| :--- | :--- | :--- | :--- |
| **1. GNOME 46 / 47** | Wayland (`WAYLAND_DISPLAY=wayland-0`) | `xdg-desktop-portal-gnome` (`org.freedesktop.portal.ScreenCast` + PipeWire DMA-BUF NV12) | AppIndicator (`org.kde.StatusNotifierWatcher`) + `org.freedesktop.Notifications` |
| **2. KDE Plasma 6 (KWin)** | Wayland (`XDG_CURRENT_DESKTOP=KDE`) | `xdg-desktop-portal-kde` (PipeWire) + поддержка дробного масштабирования (`125%`, `150%`) | Нативный `StatusNotifierItem` + KWallet / Secret Service |
| **3. Hyprland / Sway (wlroots)** | Wayland (`wayland-1`) | `xdg-desktop-portal-hyprland` / `xdg-desktop-portal-wlr` (PipeWire) | Waybar SNI + `mako` / `dunst`, адаптация к динамическому тайлингу от `900×550` до `3840×2160` |
| **4. X11 (Xfce / i3 / Cinnamon)** | X11 (`DISPLAY=:0`, без `WAYLAND_DISPLAY`) | Автоматический бесшовный откат на **X11 MIT-SHM (`XShmGetImage`)** | XEmbed / StatusNotifier + `xfce4-notifyd` / `dunst` |

---

## 2. Быстрый старт: проверка целостности и запуск в 1 команду

### 2.1. Шаг 1: Проверка контрольных сумм `SHA256SUMS`

Перед распаковкой и запуском обязательно убедитесь в подлинности и целостности загруженного архива и бинарника с помощью системной утилиты `sha256sum`:

```bash
cd rust/dist
sha256sum -c SHA256SUMS
```

Ожидаемый вывод для всех артефактов релиза:
```text
discord-gpui: ЦЕЛ
discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz: ЦЕЛ
discord-gui-spike: ЦЕЛ
discord-rs: ЦЕЛ
sbom.spdx.json: ЦЕЛ
```

### 2.2. Шаг 2: Распаковка архива и запуск в 1 команду (`./discord-gpui`)

Распакуйте релизный архив `discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz` и запустите клиент одной командой:

```bash
tar -xzf rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz
cd discord-gpui-v1.0.0-beta.1-linux-x86_64
./discord-gpui
```

Или напрямую из каталога `rust/dist/`:
```bash
./discord-gpui
```

### 2.3. Быстрая самодиагностика (`--headless-smoke`) и софтверный рендеринг `llvmpipe`

Для мгновенной проверки работоспособности графического бэкенда, локальной базы данных SQLite WAL, кольцевого аудио-буфера и бюджета памяти (`VmRSS < 60 МБ`) без открытия графического окна выполните:

```bash
./discord-gpui --headless-smoke
```

Ожидаемый ответ (`exit code 0`):
```text
GPUI_SMOKE_OK {"channels":12,"display_backend":"wayland+x11","fps_budget_ok":true,"frame_tick_us":185,"guilds":4,"min_height":550,"min_width":900,"sqlite_wal_ok":true,"status":"ok","vmrss_kb":11532,"vmrss_mb":11.26,"window_height":720,"window_width":1280}
```

Если на тестовой машине или в виртуальной среде отсутствует аппаратное ускорение Vulkan/GPU, клиент поддерживает принудительный запуск на программном растеризаторе Mesa **`llvmpipe`**:

```bash
LIBGL_ALWAYS_SOFTWARE=1 MESA_LOADER_DRIVER_OVERRIDE=llvmpipe ./discord-gpui
```

---

## 3. Включение отладочных логов и политика безопасности токенов

### 3.1. Запуск с подробным логированием (`RUST_LOG=info ./discord-gpui`)

При прохождении тестовых сценариев (аутентификация, переключение серверов, голосовые вызовы с шифрованием DAVE MLS v1, трансляция экрана PipeWire) запускайте клиент с включённым журналом событий:

```bash
# Базовый информационный уровень для бета-тестирования:
RUST_LOG=info ./discord-gpui

# Расширенная трассировка сетевого шлюза, голосового транспорта и порталов Wayland:
RUST_LOG=debug ./discord-gpui 2>&1 | tee discord-gpui-beta.log
```

### 3.2. Автоматическое маскирование секретов и защита данных

В `discord-gpui` встроена принудительная санитизация всех логов, дампов состояния и отчётов об ошибках:
- Любые пользовательские и MFA-токены в выводе автоматически маскируются как `mfa.[REDACTED]`, `mfa.***` или `Bot ***`.
- Сессионные ключи и ратчеты сквозного шифрования голоса (`DAVE MLS v1`, `RFC 9420`) никогда не выводятся в `stdout`/`stderr` и затираются в памяти после смены эпохи.
- **Важно:** Несмотря на автоматическую фильтрацию, перед отправкой логов убедитесь, что вы не вставили приватные токены вручную в переменные окружения командной строки.

---

## 4. Что проверить в рамках бета-тестирования (Ключевые сценарии)

Подробные чек-листы приведены в документе [`docs/specs/MANUAL-QA-SUITE.md`](file:///home/sanya/orca/project/docs/specs/MANUAL-QA-SUITE.md). В первую очередь рекомендуем проверить:
1. **Авторизация и Keyring (`AUTH-01..05`):** вход по логину/паролю, 2FA TOTP, QR-коду, прохождение капчи (`40003`) в изолированном окне и сохранение сессии в системном `libsecret`.
2. **Виртуализированный чат 120 FPS (`CHAT-01..04`):** плавная прокрутка длинных каналов (`100 000+` сообщений), отправка текста до `2000` символов, реакции, прикрепление до `10` файлов (`<= 25 МБ`), полнотекстовый поиск (`Ctrl+F` / SQLite FTS5) и быстрый переход `Ctrl+K`.
3. **Голосовая связь и DAVE E2EE (`VOICE-01..04`):** подключение к голосовому каналу, проверка индикатора сквозного шифрования **DAVE MLS v1** и 30-значного кода приватности, переключение `Mute`/`Deafen`, горячее переключение USB/Bluetooth-гарнитур и ползунки громкости `0..200%`.
4. **Демонстрация экрана Go Live (`STREAM-01..03`):** выбор монитора или отдельного окна через портал `org.freedesktop.portal.ScreenCast` (или X11 SHM fallback), переключение пресетов `720p`/`1080p` @ `30`/`60 FPS` на лету без обрыва звонка.
5. **Устойчивость к сбоям сети (`Exploratory`):** отключение интернета на `30 секунд` с проверкой автоматического восстановления сессии через `Opcode 6 RESUME` без потери набранного черновика.

---

## 5. Шаблон репорта о найденной ошибке (Bug Report Template)

При обнаружении любого дефекта, артефакта вёрстки, просадки FPS или падения процесса оформите карточку по шаблону ниже.

### 5.1. Снятие стектрейса GDB (при падении или зависании)

Если процесс завершился аварийно или завис, снимите полный стек вызовов всех потоков через `gdb`:

```bash
gdb -batch -ex "thread apply all bt full" ./discord-gpui core
# Или для запущенного процесса по PID:
gdb -batch -ex "thread apply all bt full" -p $(pidof discord-gpui) > gdb-backtrace.txt
```

### 5.2. Классификация критичности дефектов (`P0–P3`)

- **`P0` (Blocker):** Паника процесса (`panic!`/`SIGSEGV`), утечка секретов, невозможность входа в аккаунт, повреждение локальной БД SQLite или отказ шифрования DAVE E2EE.
- **`P1` (Critical):** Не работает ключевая функция (отправка сообщений, подключение к голосу, захват экрана PipeWire/X11, поломка сетки окна при масштабировании `125%`/`150%` или превышение бюджета `VmRSS >= 60 МБ`).
- **`P2` (Minor):** Неблокирующие визуальные или UX-отклонения при наличии простого обходного пути.
- **`P3` (Trivial):** Косметические замечания, опечатки локализации и пожелания по удобству.

### 5.3. Форма репорта (скопируйте и заполните)

```markdown
### [BUG-BETA-XX] Краткое описание проблемы

- **Критичность:** `P0` | `P1` | `P2` | `P3`
- **Версия сборки:** `discord-gpui v1.0.0-beta.1`
- **SHA-256 бинарника:** `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e`

#### 1. Окружение тестировщика (Версия Linux, GPU, Графический стек)
- **Дистрибутив Linux и версия ядра (`uname -sr`):** (например: Fedora 41 / Linux 6.11.6)
- **Версия GLIBC (`ldd --version | head -n1`):** (например: glibc 2.39)
- **Графическое окружение и протокол (`$XDG_CURRENT_DESKTOP` / `$XDG_SESSION_TYPE`):** (GNOME 47 Wayland / KDE Plasma 6 / Hyprland / X11)
- **Модель GPU и версия драйвера (`vulkaninfo --summary` или `glxinfo -B`):** (например: AMD Radeon RX 7800 XT, Mesa 24.2 RADV / llvmpipe)
- **Масштабирование экрана и разрешение:** (например: 2560x1440 @ 125%)
- **Аудио-сервер (`pactl info | grep "Server Name"`):** (например: PulseAudio (on PipeWire 1.2.5))

#### 2. Шаги воспроизведения (Reproduction Steps)
1. Запустить клиент командой `RUST_LOG=info ./discord-gpui`.
2. Перейти в ...
3. Нажать на ...

#### 3. Ожидаемый результат (Expected Behavior)
(Опишите ожидаемое поведение согласно спецификации)

#### 4. Фактический результат (Actual Behavior)
(Опишите фактическое поведение, приложите скриншот или видеозапись)

#### 5. Логи (`RUST_LOG=info ./discord-gpui`) и стектрейс GDB
```text
(Вставьте последние 50 строк вывода RUST_LOG=info ./discord-gpui и вывод gdb -batch -ex "thread apply all bt full" — убедитесь, что токены замаскированы как mfa.[REDACTED])
```
```
