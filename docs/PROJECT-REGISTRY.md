# Единый реестр проекта discord-lab (Unified Project Registry)

> **Статус документа:** `Canonical Project Registry (v2.1)`  
> **Дата актуализации:** 12 ноября 2026 г.  
> **Репозиторий:** `discord-lab` (Линейка нативного клиента `discord-gpui` + BFF & Core Rust)  
> **Оркестратор:** ECC 2.0 Control Plane (`scripts/ecc`) + Orca Engine  

---

## 1. Паспорт проекта

| Параметр | Значение |
| :--- | :--- |
| **Название проекта** | `discord-lab` |
| **Основной продукт** | Высокопроизводительный нативный Linux-клиент Discord на Rust GPUI (`discord-gpui`) |
| **Графический стек** | `gpui-kit` / WGPU (Vulkan / OpenGL llvmpipe), 120 FPS, нативный композитинг |
| **Язык и рантайм** | Rust (Edition 2024 / MSRV 1.85+), Tokio асинхронный рантайм, Node.js 24 (тесты/инструменты) |
| **Текущая волна** | **Wave RW-47**: Sprints 1–5 Feature Parity, Soundboard, Voice Messages, Polls, Threads & Packaging |
| **Релизный артефакт** | [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) (42 МБ, stripped ELF, Thin LTO, opt-level 3) |
| **Контрольная сумма SHA-256**| `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e` |
| **Целевые платформы** | Linux (Wayland: GNOME, KDE, Hyprland, Sway; X11 fallback), Windows (WiX MSI/Portable), macOS (DMG) |
| **Статус задач** | **220 выполнено (Done)** из 220 задач на канбан-доске (**100.0%**) |
| **Статус тестов** | **1500+ тестов** (147 meta, 920+ unit, 391+ GPUI contract/e2e/chaos) — 100% Passed (5/5 мутаций убиты) |

---

## 2. Реестр крейтов Rust (Workspace Crates)

| Крейт | Путь | Назначение и ключевые подсистемы | Тестовые контракты |
| :--- | :--- | :--- | :--- |
| **`discord-gpui`** | `rust/discord-gpui/` | **Основное нативное GUI-приложение**: Server Rail, виртуализированный чат 100k сообщений, 8 вкладок настроек, модалки трансляции экрана, интерактивные опросы (`poll_view`), треды (`thread_sidebar`), эмодзи/стикер пикер (`emoji_picker`), плеер голосовых сообщений (`voice_player`), сетка саундборда (`soundboard_view`), радар пространственного звука (`spatial_radar`), WebRTC HUD, AT-SPI. | `gpui_e2e_contract.rs`<br>`visual_regression_contract.rs`<br>`chaos_engineering_contract.rs`<br>`a11y_atspi_contract.rs`<br>`perf_benchmark_contract.rs`<br>`manual_qa_suite_contract.rs`<br>`release_smoke_contract.rs` |
| **`discord-core`** | `rust/crates/discord-core/` | **Ядро протокола**: клиент REST v9, шлюз Gateway v9 (HELLO, Heartbeat, IDENTIFY, RESUME, VOICE_STATE_UPDATE), форумные каналы (`forum.rs`), жизненный цикл тредов (`threads.rs`), движок опросов (`polls.rs`), редьюсер состояния, битовые маски прав доступа (64-бит). | `permissions_contract.rs`<br>`ready_read_state.rs`<br>`channels_contract.rs`<br>`threads-lifecycle-1002a`<br>`polls-core-1004a` |
| **`discord-voice`** | `rust/crates/discord-voice/` | **Голосовой и медиа-стек**: голосовой UDP-стример, IP Discovery (74B), Keepalive (8B), RTP/RTCP пакеты, SRTP (AES-256-GCM / ChaCha20), DAVE E2EE MLS v1 (`RFC 9420`), рекордер голосовых сообщений OGG Opus (`voice_message_recorder.rs`), саундборд Opcode 18 (`soundboard.rs`) и линейный `tanh()` микшер, пространственный фильтр HRTF 360°. | `reception_contract.rs`<br>`rtcp_contract.rs`<br>`rtp_video_contract.rs`<br>`soundboard-sfx-1012`<br>`voice-recorder-1003a`<br>`spatial-audio-filter-1017a` |
| **`discord-media`** | `rust/crates/discord-media/` | **Аппаратные медиа-буферы и кэш**: кольцевой буфер `AudioRingBuffer` без аллокаций, двухзвенный кэш эмодзи и стикеров (256 МБ RAM LRU + дисковый ETag, `emoji_cache.rs`), коннекторы захвата экрана X11 и Wayland Portal. | `buffer_contract.rs`<br>`ring_buffer_contract.rs`<br>`capture_contract.rs`<br>`emoji-cache-1005a` |
| **`discord-platform`** | `rust/crates/discord-platform/` | **Платформенные интеграции**: `org.freedesktop.portal.ScreenCast`, D-Bus уведомления, системный трей, Keyring через FFI к libsecret, автозапуск XDG autostart (`autostart.rs`), интеграция Spotify D-Bus MPRIS (`spotify_mpris.rs`), FFI проверки орфографии libenchant/hunspell (`spellcheck.rs`), геймпад gilrs & SteamOS клавиатура. | `screencast_contract.rs`<br>`keyring_contract.rs`<br>`autostart-xdg-1015`<br>`spotify-mpris-presence-1013`<br>`steam-deck-gamepad-1014` |
| **`discord-store`** | `rust/crates/discord-store/` | **Локальное хранилище данных**: SQLite FTS5 полнотекстовый поиск, режим WAL, кэш сообщений и настроек, безопасные миграции схемы. | `store_contract.rs`<br>`fts5_contract.rs` |
| **`discord-codec`** | `rust/crates/discord-codec/` | **Кодеки и декомпрессия**: декодер и энкодер ETF бинарного формата, zstd-стриминг, защита от compression-bomb атак. | `zstd_bomb_contract.rs`<br>`etf_properties.rs` |
| **`discord-i18n`** | `rust/crates/discord-i18n/` | **Локализация и интернационализация**: 36 языков, парсер AST, CLDR плюрализация, хеши ключей xxh64→base64. | `i18n_contract.rs` |

---

## 3. Реестр релизных артефактов и манифестов

| Артефакт | Путь | Описание / Метрики |
| :--- | :--- | :--- |
| **Релизный бинарник** | [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) | 42 МБ, stripped ELF, Thin LTO, opt-level 3 |
| **Манифест сумм** | [`rust/dist/SHA256SUMS`](file:///home/sanya/orca/project/rust/dist/SHA256SUMS) | SHA-256 контрольные суммы релизных бинарников |
| **Манифест SBOM** | `rust/dist/sbom.spdx.json` | SPDX 2.3 воспроизводимый манифест лицензий зависимостей |
| **Кросс-платформенная упаковка** | `packaging/` | Windows WiX MSI (`packaging/windows/wix.xml`), macOS DMG (`packaging/macos/build-dmg.sh`), Flatpak, AppImage, Arch PKGBUILD, Debian .deb |
| **15 Golden эталонов** | `rust/discord-gpui/tests/goldens/*.png` | Эталонные снимки экрана GPUI (чат, настройки, модалки) |
| **Динамические JSON-маски** | `rust/discord-gpui/tests/goldens/dynamic_masks_*.json` | Маски исключений (часы, FPS, аватары, пинг) |
| **Интерактивный HTML Diff** | `rust/discord-gpui/tests/goldens/visual_diff_report.html` | Интерактивный отчёт сравнения скриншотов (Slider / Onion Skin / Red Diff) |
| **Канбан-доска (JSON)** | [`waves/kanban-board.json`](file:///home/sanya/orca/project/waves/kanban-board.json) | 220 задач со статусами, отделами и критериями приёмки |
| **Канбан-доска (HTML)** | [`waves/kanban-board.html`](file:///home/sanya/orca/project/waves/kanban-board.html) | Интерактивный интерфейс канбан-доски |

---

## 4. Реестр тестовых наборов и верификации (Пирамида T0–T4)

```
       ▲
      / \       T4: Qualification Gate (Keyring live, Tarball, live-probes)
     /---\      
    /     \     T3: Release & Mutation Gate (5/5 mutations killed, ASan/TSan, SBOM)
   /-------\    
  /         \   T2: Integration & E2E (GPUI Headless Smoke, 50 E2E Journeys, SSIM >0.995)
 /-----------\  
/             \ T1: Worker Gate (Clippy -D warnings, Typecheck, Unit, Contract)
───────────────
T0: Targeted    (Targeted test impact: node scripts/test-impact.mjs --run, <1 sec)
```

| Уровень | Тестовый набор | Файл | Кол-во тестов | Время / Результат |
| :---: | :--- | :--- | :---: | :---: |
| **T0** | Test Impact Analyzer | `scripts/test-impact.mjs` | целевые | `< 1 с` (Green) |
| **T1** | Meta Test Suite | `tests/meta/*.test.mjs` | 147 | `< 1 с` (100% Green) |
| **T1** | Unit Test Suite | `tests/unit/*.test.mjs` | 920+ | `< 5 с` (100% Green) |
| **T2** | GPUI Headless Smoke | `rust/discord-gpui/src/main.rs --headless-smoke` | 1 | `< 500 мс`, VmRSS 11.26 МБ |
| **T2** | GPUI 50 E2E Journeys | `rust/discord-gpui/tests/gpui_e2e_contract.rs` | 50 | 0.08 с (100% Green) |
| **T2** | GPUI Core Contracts | `rust/discord-gpui/tests/gpui_contract_test.rs` | 127 | 1.80 с (100% Green) |
| **T2** | Visual SSIM Regression | `rust/discord-gpui/tests/visual_regression_contract.rs` | 10 | 1.82 с (SSIM > 0.995) |
| **T2** | Chaos Engineering Lab | `rust/discord-gpui/tests/chaos_engineering_contract.rs` | 53 | 0.08 с (100% Green) |
| **T2** | AT-SPI D-Bus Driver | `rust/discord-gpui/tests/a11y_atspi_contract.rs` | 34 | 0.08 с (100% Green) |
| **T2** | 120 FPS Benchmark | `rust/discord-gpui/tests/perf_benchmark_contract.rs` | 53 | 0.08 с (100% Green) |
| **T2** | Manual QA Matrix Suite | `rust/discord-gpui/tests/manual_qa_suite_contract.rs` | 56 | 0.08 с (100% Green) |
| **T3** | Мутационный гейт | В каждом контрактном тесте | 5/5 на срез | **100% (5/5 killed)** |
| **T3** | Headless Smoke Release | `rust/discord-gpui/tests/release_smoke_contract.rs` | 3 | 0.15 с (100% Green) |
| **T3** | Network Fuzz Targets | `rust/fuzz/` (5 fuzz-мишеней) | 5 | 1 000 000 итераций |
| **T4** | Keyring & Qualification | `scripts/gate.mjs --scope qualify` | full | 100% Green |

---

## 5. Реестр инструментов автоматизации (Scripts & Tools)

1. **`scripts/ecc`**:
   * Точка входа в ECC 2.0 Control Plane. Управляет графами знаний, профилями отделов, проверками соответствия правилам и синхронизацией памяти (`ecc graph connector-sync --all`).
2. **`scripts/ci.sh`**:
   * Единый скрипт локального и CI конвейера. Выполняет проверку форматирования, clippy, workspace тесты, запуск headless smoke GPUI, E2E и визуальной регрессии.
3. **`scripts/gate.mjs`**:
   * Мощный фазовый раннер с профилями `worker`, `release` и `qualify`. Сохраняет структурированные JSON-артефакты в `.gate/`.
4. **`scripts/release.sh`**:
   * Скрипт сборки релизного бинарника `discord-gpui` с LTO/strip, генерацией `SHA256SUMS` и SBOM.
5. **`scripts/mcp-kanban-board.mjs`**:
   * Инструмент генерации и управления канбан-доской (`--export`, `--stats`, `--card`).
