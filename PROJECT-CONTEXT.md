# discord-lab: session checkpoint (сжатый контекст)

Точка входа для нового диалога. Читать первым; детали — точечно по ссылкам.
Обновлён: **2026-11-12** (Волна **RW-47** в активной разработке; канбан-доска закрыта на **85.00%**).

---

## 1. Текущий статус проекта (2026-11-12)

- **Нативный Rust GPUI клиент (`discord-gpui`)**:
  - Полный отказ от WebKit/Tauri прототипа в пользу прямого GPU-рендеринга WGPU 120 FPS (`rust/discord-gpui/`).
  - Релизный бинарник: [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) — **42 МБ** (stripped ELF, thin LTO, opt-level 3).
  - Контрольная сумма в [`rust/dist/SHA256SUMS`](file:///home/sanya/orca/project/rust/dist/SHA256SUMS): `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e`.
  - Потребление памяти в простое: **11.26–18.00 МБ VmRSS** (лимит бюджета `<60 МБ`), 120 FPS frame budget соблюдён.
  - Детерминированный E2E-контур: [`rust/discord-gpui/tests/gpui_e2e_contract.rs`](file:///home/sanya/orca/project/rust/discord-gpui/tests/gpui_e2e_contract.rs) — 50 сквозных пользовательских путей (100% pass за 0.08с).

- **Текущая волна: Wave RW-47 (Sprints 1–5 & Deep Parity Integration)**:
  - Активный run оркестрации: `run_a9411d4ae3c4` под управлением Orca Orchestration.
  - **Канбан-доска:** [`waves/kanban-board.html`](file:///home/sanya/orca/project/waves/kanban-board.html) и [`waves/kanban-board.json`](file:///home/sanya/orca/project/waves/kanban-board.json):
    - Всего карточек: **220**
    - **Закрыто и запушено в GitHub (`done`):** **187 задач (85.00%)**
    - **В параллельной разработке (`in_dev`):** **10 задач** (полная загрузка пула под Zero-Idle)
    - **В бэклоге (`backlog`):** **23 задачи**
  - **Свежепринятые и проверенные подсистемы (100% Green + 5/5 мутаций):**
    - `TASK-1004` / `TASK-1004B`: Интерактивные опросы в чате (Polls & Voting, PollCardView с анимированными шкалами и бейджем победителя).
    - `TASK-1006`: Нативная проверка орфографии в поле ввода (hunspell / libenchant-2, волнистое подчёркивание `#f23f43`, контекстное меню).
    - `TASK-1007`: Иерархия ролей сервера, drag-and-drop PATCH `/guilds/{id}/roles` и редактор 64-битной маски прав (41 флаг).
    - `TASK-1008`: Журнал аудита сервера (Server Audit Log GET `/guilds/{id}/audit-logs`, декодирование диффов, маскирование секретов).
    - `TASK-1009`: Двухфакторная аутентификация TOTP 2FA и управление активными сессиями.
    - `TASK-1010`: Голосовой AFK-монитор и автоматический переход в AFK-канал (Wayland / D-Bus / X11).
    - `TASK-1011`: Продвинутая WebRTC диагностика и RTC Internals HUD (RTCP Sender/Receiver Reports, RTT/jitter/loss 60fps кривые, DAVE MLS v1 Privacy Code 6x5).
    - `TASK-1012` / `TASK-1012A`: Звуковая панель Soundboard SFX (Voice Opcode 18, Opcode 19 SPEAKING, Opus 48kHz stereo LRU-кэш, 20ms RTP микшер с `tanh()` софт-клиппингом).
    - `TASK-1013`: Интеграция со Spotify по D-Bus MPRIS (Listen Along, перехват треков, Opcode 3 `LISTENING` активность).
    - `TASK-1014`: Режим Steam Deck & Gamepad UI (gilrs, 4-зонная фокусная сетка, экранная клавиатура SteamOS по D-Bus).
    - `TASK-1015`: Автозапуск с системой (XDG Autostart `~/.config/autostart/` и старт свёрнутым `--start-minimized` с VmRSS < 8 МБ).
    - `TASK-1016`: Кроссплатформенные сборки и пакеты (Windows WiX MSI / Portable в `data/` и macOS DMG с Vibrancy NSVisualEffectView Dark HUD).
    - `TASK-1001` / `TASK-1001A` / `TASK-1001B`: Форум-каналы (`GUILD_FORUM` type 15), REST пагинация активных постов и 2-колоночная сетка `ForumGridView` с фильтрацией тегов.
    - `TASK-1002` / `TASK-1002A`: Вложенные треды обсуждений (`PUBLIC_THREAD` / `PRIVATE_THREAD`), таймеры автоархивации (60..10080 мин) и синхронизация участников.
    - `TASK-1003` / `TASK-1003A`: Голосовые сообщения с Waveform (OGG Opus 48kHz mono 32kbps, 256-байтный RMS waveform, flags 8192).
    - `TASK-1005` / `TASK-1005A` / `TASK-1005B`: Кастомные эмодзи и Lottie-стикеры (двухуровневый кэш 256 МБ RAM + диск, интерактивный пикер 380x420px).

---

## 2. Архитектура и структура директорий (Чистый стандарт)

- **Структура репозитория:**
  - `rust/crates/`: модули ядра (`discord-core`, `discord-voice`, `discord-codec`, `discord-media`, `discord-platform`, `discord-store`, `discord-i18n`).
  - `rust/discord-gpui/`: нативный UI-клиент на GPU WGPU 120 FPS.
  - `packaging/`: манифесты сборки дистрибутивов (`windows/wix.xml`, `macos/build-dmg.sh`, Linux deb/rpm/AppImage).
  - `docs/articles/`: публичные и технические статьи по проекту.
  - `docs/archive/wave-artifacts/`: архив промежуточных артефактов волн (ранние `CONTRACTS`, `PM`, `QA`, `QUALITY`).
  - `waves/`: история волн, журнал событий `db.jsonl`, состояние `STATE.md`, канбан-доска `kanban-board.json`/`html`.
  - `tests/`: 12-уровневый контур тестов (`tests/unit/`, `tests/smoke/`, `tests/e2e/`, `tests/meta/`).
  - `scripts/`: утилиты гейта, сборки, SBOM, хеширования и проверки паритета (`gate.mjs`, `ci.sh`, `release.sh`, `quality-gate.mjs`, `mcp-kanban-board.mjs`).

- **Контроль качества и мета-тесты:**
  - **147 / 147 мета-тестов пройдены успешно (100% PASS, 0 FAIL)** (`npm run test:meta`).
  - Строгий запрет ослабления тестов (`TEST-POLICY.md`).
  - Мутационный гейт: **100% Mutation Kill Rate (5/5 контрольных мутаций убиты на каждый модуль)**.
  - `#![forbid(unsafe_code)]` во всех модулях Rust.
  - Ограничение сложности: `< 50 строк на функцию`, `< 800 строк на файл`.
  - Модель памяти: `WORKER_RAM_LIMIT <= 25 GB` (свободно 26–34 GB).
