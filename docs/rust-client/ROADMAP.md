# Дорожная карта разработки Rust-клиента (discord-lab)

> **Статус:** `Active Strategic Roadmap (v2.0)`  
> **Дата актуализации:** Ноябрь 2026 г.  

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

---

## 2. Текущая активная веха: Wave RW-44 (QA Automation & Hardening)

Цель: Создание всеобъемлющего, детерминированного тестового контура вокруг нативного GPUI клиента.

* [x] **`TASK-904`** — Визуальная регрессия GPUI на базе SSIM (>0.99) и динамических JSON-масок (10/10 passed, 5/5 мутаций).
* [x] **`TASK-905`** — Стенд Chaos Engineering: сетевые обрывы, джиттер 20–60мс, PipeWire restart (53/53 passed, 5/5 мутаций).
* [x] **`TASK-908`** — Регламент и сценарии ручного тестирования Manual QA Suite (56/56 passed).
* [x] **`TASK-902`** — Семантический GUI-драйвер AT-SPI D-Bus и аудит WCAG 2.2 AAA (34/34 passed, 5/5 мутаций).
* [ ] **`TASK-901`** — Нативный Headless E2E Harness (Virtual Wayland + Mock Gateway Daemon).
* [ ] **`TASK-903`** — Фаззинг сетевых протоколов и криптографии DAVE MLS v1 (libFuzzer + ASan).
* [ ] **`TASK-906`** — Бенчмаркинг 120 FPS и профилирование аллокаций памяти jemalloc.
* [ ] **`TASK-907`** — Автоматизированный гейт качества кода (Clippy `-D warnings`, ASan/TSan, 5/5 мутаций).

---

## 3. Следующие вехи (Future Roadmap)

### Веха 5: Wave RW-45 — Дистрибуция и упаковка под Linux
* Сборка пакетов дистрибуции:
  * Flatpak манифест (`com.discord_lab.DiscordGpui`).
  * Автономный AppImage с бандлингом системных библиотек.
  * Пакеты для Arch Linux (PKGBUILD) и Debian/Ubuntu (`.deb`).
* Автоматизация обновления через GitHub Releases и SBOM манифесты.

### Веха 6: Релиз v1.0.0 Production Stable
* Финальный прогон квалификационного гейта (`scripts/gate.mjs --scope qualify`).
* Подписание релизного бинарника GPG ключом и публикация `SHA256SUMS`.
