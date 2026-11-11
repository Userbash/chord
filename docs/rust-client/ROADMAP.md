# Дорожная карта разработки Rust-клиента (discord-lab)

> **Статус:** `Active Strategic Roadmap (v2.1)`  
> **Дата актуализации:** 12 ноября 2026 г.  

---

## 1. Завершённые вехи (Completed Milestones)

### Веха 1: Реверс-инжиниринг протоколов и Core-крейты (RW-01..RW-35) — 100% DONE
* [x] Разбор дампа `/home/sanya/re-deep/` (REST v9, WebSocket шлюз, ETF, zstd-стриминг).
* [x] Крейт `discord-codec`: ETF энкодер/декодер с защитой от уязвимостей, zstd декомпрессор.
* [x] Крейт `discord-core`: Клиент Discord REST API v9, Gateway WebSocket клиент, редьюсер состояния, права доступа.
* [x] Крейт `discord-media`: Кольцевой аудио-буфер `AudioRingBuffer`, видео-пайплайн.
* [x] Крейт `discord-voice`: Голосовой UDP стример, IP Discovery, RTP/RTCP пакеты, SRTP AES-256-GCM.
* [x] Крейт `discord-store`: SQLite FTS5 полнотекстовый поиск сообщений, режим WAL.
* [x] Крейт `discord-platform`: Системный трей, уведомления D-Bus, хранилище ключей Keyring libsecret FFI.
* [x] Крейт `discord-i18n`: 36 языковых локалей, хэши ключей xxh64.

### Веха 2: Сквозное шифрование DAVE E2EE MLS v1 (RW-40..RW-41) — 100% DONE
* [x] Реализация RFC 9420 KDF и MLS Key Schedule.
* [x] Интеграция `Aes128GcmTruncated8` с усечённым 8-байтным тегом.
* [x] Реализация `DaveHashRatchet` с защитой от атак повторного воспроизведения (Anti-Replay).
* [x] Двухфазная смена эпох сессии в `DaveVoiceSession`.

### Веха 3: Миграция на нативный Rust GPUI клиент (RW-42..RW-43) — 100% DONE
* [x] Полный вывод из эксплуатации медленного WebKit/Tauri прототипа.
* [x] Создание чистого нативного приложения `rust/discord-gpui` на базе `gpui-kit` (WGPU, 120 FPS).
* [x] Реализация полного графического интерфейса:
  * Server Rail (72px) с ховерами и индикаторами.
  * Виртуализированный чат (100k сообщений при RAM < 60 MB).
  * Настройки (8 вкладок `SettingsDialog`).
  * Стриминг экрана `ScreenShareModal` (PipeWire Portal, 720p/1080p, 30/60 FPS).
  * Аутентификация (Email, пароль, токен, 2FA, QR, hCaptcha bridge).
* [x] Сборка релизного бинарника [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) — **42 МБ** (stripped ELF, Thin LTO, opt-level 3).
* [x] Headless Smoke тест: VmRSS **11.26 МБ**, тик кадра < 8.33 мс.

### Веха 4: QA Automation & Hardening (RW-44) — 100% DONE
* [x] **`TASK-904`** — Визуальная регрессия GPUI на базе SSIM (>0.99) и динамических JSON-масок (10/10 passed, 5/5 мутаций).
* [x] **`TASK-905`** — Стенд Chaos Engineering: сетевые обрывы, джиттер 20–60мс, PipeWire restart (53/53 passed, 5/5 мутаций).
* [x] **`TASK-908`** — Регламент и сценарии ручного тестирования Manual QA Suite (56/56 passed).
* [x] **`TASK-902`** — Семантический GUI-драйвер AT-SPI D-Bus и аудит WCAG 2.2 AAA (34/34 passed, 5/5 мутаций).
* [x] **`TASK-901`** — Нативный Headless E2E Harness (Virtual Wayland + Mock Gateway Daemon, 50 journeys).
* [x] **`TASK-903`** — Фаззинг сетевых протоколов и криптографии DAVE MLS v1 (libFuzzer + ASan, 5 targets).
* [x] **`TASK-906`** — Бенчмаркинг 120 FPS и профилирование аллокаций памяти jemalloc.
* [x] **`TASK-907`** — Автоматизированный гейт качества кода (Clippy `-D warnings`, ASan/TSan, 5/5 мутаций).

### Веха 5: Дистрибуция и упаковка под Linux, Windows и macOS (RW-45..RW-46) — 100% DONE
* [x] Flatpak манифест (`com.discord_lab.DiscordGpui`).
* [x] Автономный AppImage с бандлингом системных библиотек.
* [x] Пакеты для Arch Linux (PKGBUILD) и Debian/Ubuntu (`.deb`), RedHat (`.rpm`), Snapcraft.
* [x] Windows WiX Toolset MSI инсталлятор + Portable `.zip`.
* [x] macOS DMG образ с поддержкой эффекта Vibrancy.

---

## 2. Текущая активная веха: Wave RW-47 (Sprints 1–5 Parity & Polishing)

Цель: Достижение максимального функционального паритета с официальным клиентом Discord (покрытие > 92%).

* [x] **Sprint 1 (Channels & Threads)**:
  * Форумные каналы (тип 15) и 2-колоночная сетка карточек `ForumGridView` (`TASK-1001A`, `TASK-1001B`).
  * Жизненный цикл тредов и боковая панель 360px `ThreadSidebar` (`TASK-1002A`, `TASK-1002B`).
* [x] **Sprint 2 (Voice Messages & Polls)**:
  * Рекордер OGG Opus 32 kbps и 256-байтный вейвформ (`TASK-1003A`).
  * Интерактивный плеер `VoiceMessagePlayer` с переключением скорости (`TASK-1003B`).
  * Движок опросов, голосование и подсчет процентов (`TASK-1004A`).
  * Карточка опроса `PollCardView` с бейджем победителя (`TASK-1004B`).
* [x] **Sprint 3 (Media, SFX & Spellchecking)**:
  * Двухзвенный кэш эмодзи (256 МБ LRU + ETag) и пикер 380x420px (`TASK-1005A`, `TASK-1005B`).
  * Саундборд: Voice Opcode 18, Opus-кэш и линейный микшер с `tanh()` (`TASK-1012A`, `TASK-1012B`, `TASK-1012C`).
  * Нативная проверка орфографии: FFI к libenchant/hunspell, волнистая линия `#f23f43` (`TASK-1006A`, `TASK-1006B`).
* [x] **Sprint 4 (Guild Administration & Platform)**:
  * 64-битный редактор прав и ролей гильдии (`TASK-1007`).
  * Просмотр журнала аудита сервера с фильтрами (`TASK-1008`).
  * TOTP 2FA диалог и управление активными сессиями (`TASK-1009`).
  * Монитор Voice AFK и авто-перевод в канал бездействия (`TASK-1010`).
  * Интеграция Spotify D-Bus MPRIS и функция Listen Along (`TASK-1013`).
  * Поддержка геймпадов Steam Deck и виртуальной клавиатуры SteamOS (`TASK-1014`).
  * XDG автозапуск и старт в трей `--start-minimized` (`TASK-1015`).
* [x] **Sprint 5 (Spatial Audio & Real-Time Diagnostics)**:
  * 360-градусный бинауральный HRTF фильтр KEMAR (`TASK-1017A`).
  * WGPU 2D радар участников Spatial Audio (`TASK-1017B`).
  * Сборщик RTCP метрик с RingBuffer 120 (`TASK-1011A`).
  * Векторный 60 FPS экран диагностики WebRTC (`TASK-1011B`).

---

## 3. Следующие вехи (Future Roadmap)

### Веха 7: Релиз v1.0.0 Production Stable
* Финальная предрелизная сборка `BUILD-005` в ветку `testing` $\to$ `beta` $\to$ `main`.
* Квалификационный гейт (`scripts/gate.mjs --scope qualify`).
* Подписание релизного бинарника GPG ключом и публикация на GitHub Releases.

### Веха 8: Пост-релизные возможности (Post-v1.0.0)
* Поддержка пользовательских тем оформления через CSS-токены GPUI.
* Видео-звонки высокого разрешения (4K 60 FPS с аппаратным энкодером NVENC/VA-API).
* Плагин-система расширений (WASM-based sandboxed extensions).
