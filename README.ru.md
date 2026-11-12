# Discord Lab & Native Discord GPUI Client

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/gui-GPUI%20%2F%20WGPU-blue.svg)](https://github.com/zed-industries/zed)
[![Tests](https://img.shields.io/badge/tests-1500%2B%20100%25%20green-brightgreen.svg)](tests/)
[![Mutations](https://img.shields.io/badge/mutations-5%2F5%20killed-success.svg)](waves/)
[![RAM](https://img.shields.io/badge/memory-%3C%2060%20MB%20RAM-blueviolet.svg)](docs/rust-client/STATUS.md)
[![DAVE E2EE](https://img.shields.io/badge/security-DAVE%20MLS%20v1%20E2EE-darkgreen.svg)](rust/crates/discord-voice/)

Высокопроизводительный, GPU-ускоренный нативный клиент Discord на чистом Rust (**`discord-gpui`** на базе WGPU / 120 FPS) в сочетании с исследовательской лабораторией анализа протоколов и обратного инжиниринга (**`discord-lab`** CLI, Frida, CDP).

---

## 1. Ключевые преимущества и бенчмарки

| Параметр / Характеристика | Официальный клиент Discord (Electron) | Нативный клиент `discord-gpui` (Rust) |
| :--- | :---: | :---: |
| **Потребление RAM в простое** | 800 МБ – 1.4 ГБ | **11.26 МБ** VmRSS (бюджет < 60 МБ) |
| **Частота кадров рендеринга (FPS)** | 30–60 FPS (CPU Chromium лаги) | **120 FPS** (аппаратный WGPU на GPU) |
| **Холодный запуск приложения** | 4.5 – 9.0 сек | **< 480 мс** (SQLite WAL гидратация) |
| **Сквозное шифрование голоса (E2EE)** | Частично в WebRTC | **DAVE MLS v1 (`RFC 9420`)** 100% паритет |
| **Размер дистрибутива** | 180 – 250 МБ | **42 МБ** (stripped ELF с Thin LTO) |
| **Стриминг экрана в Linux** | Проблемы в Wayland / без звука | **PipeWire Portal + DMA-BUF zero-copy** со звуком |
| **Проверка орфографии** | Chromium hunspell (утечки памяти) | **Нативный FFI libenchant/hunspell** (< 50 строк/fn) |
| **Интеграция с портативными консолями** | Отсутствует | **Steam Deck Gamepad UI + SteamOS Keyboard** |

---

## 2. Архитектура крейтов Rust (Workspace)

Проект организован как монорепозиторий модульных Rust-крейтов:

```
rust/
├── discord-gpui/         # Нативное GPU-приложение (GPUI/WGPU 120 FPS)
└── crates/
    ├── discord-core/     # Клиент REST API v9, Gateway v9, форум, треды, опросы
    ├── discord-voice/    # WebRTC, RTP/RTCP, DAVE MLS v1 E2EE, саундборд, HRTF 360°
    ├── discord-media/    # Аудио-буферы, двухзвенный LRU/ETag кэш медиа и эмодзи
    ├── discord-platform/ # XDG автозапуск, трей, D-Bus, FFI Keyring, Spotify MPRIS
    ├── discord-store/    # SQLite FTS5 полнотекстовый поиск, режим WAL, кэш
    ├── discord-codec/    # ETF бинарный кодек (защита от бомб) и zstd-stream
    └── discord-i18n/     # Интернационализация: 36 языков, хэши xxh64
```

### Подробное описание крейтов:
1. **`discord-gpui`**:
   - Полнофункциональный интерфейс: Server Rail (72px), список личных сообщений и друзей.
   - Виртуализированный скролл чата (100k+ сообщений при памяти < 60 МБ).
   - Форумные каналы (тип 15) с 2-колоночной сеткой `ForumGridView`.
   - Вложенные треды (типы 11/12) с боковой панелью 360px `ThreadSidebar`.
   - Интерактивные опросы с голосованием и процентными шкалами `PollCardView`.
   - Плеер голосовых сообщений `VoiceMessagePlayer` с 256-байтным RMS-вейвформом и скоростями 1x/1.5x/2x.
   - Модалка выбора эмодзи и стикеров `EmojiPickerModal` (380x420px, сетка 8x32px).
   - Встроенный саундборд `SoundboardGridView` с хоткеями.
   - Интерактивный 2D радар участников Spatial Audio `SpatialRadar` на WGPU.
   - 60 FPS векторный оверлей диагностики WebRTC `RtcDiagnosticsHud`.
   - Полноценные настройки: все 8 разделов официального клиента Discord.
2. **`discord-core`**:
   - Реализация шлюза WebSocket Gateway v9 (HELLO, Heartbeat, IDENTIFY, RESUME, VOICE_STATE_UPDATE).
   - Пул HTTP/2 соединений REST API v9 с умным rate-limiting и отслеживанием `retry_after_ms`.
   - 64-битный калькулятор прав доступа и ролей гильдии.
3. **`discord-voice`**:
   - DAVE E2EE MLS v1 (`RFC 9420`): криптографические храповики `DaveHashRatchet`, усечённый 8-байтный тег `Aes128GcmTruncated8`, подтверждающие теги и смена эпох.
   - Голосовые сообщения: запись 48 кГц моно, кодирование в OGG Opus 32 kbps VBR.
   - Саундборд: Voice Opcode 18 (`SOUNDBOARD_SOUND`), LRU-кэш Opus и линейный микшер с `tanh()` в микрофон.
   - Пространственный звук: 360-градусный бинауральный HRTF фильтр KEMAR.
   - Адаптивный Jitter Buffer (20–60 мс) и сокрытие потерь пакетов (PLC).
4. **`discord-platform`**:
   - Автозапуск XDG autostart и запуск в трей с отложенным WGPU окном (`--start-minimized`, VmRSS < 8 МБ).
   - Spotify D-Bus MPRIS интеграция статуса прослушивания (Opcode 3) и совместное прослушивание (Listen Along).
   - Поддержка геймпадов (gilrs) и виртуальной клавиатуры SteamOS на Steam Deck.
   - Безопасное хранение токенов в системном связке ключей Keyring через FFI к libsecret.
   - Нативная проверка орфографии в реальном времени через FFI к `libenchant-2` / `hunspell`.
5. **`discord-media`**:
   - Аппаратный кольцевой буфер `AudioRingBuffer` с нулевым копированием.
   - Двухзвенный кэш эмодзи и стикеров: 256 МБ RAM LRU + дисковый кэш с валидацией ETag.

---

## 3. Исследовательская лаборатория (`discord-lab` CLI & RE)

Репозиторий включает набор инструментов глубокого анализа запущенных процессов Discord через Frida и Chrome DevTools Protocol (CDP):

```bash
# Основной CLI-интерфейс лаборатории (bin/discord-lab.mjs)
node bin/discord-lab.mjs --help                # список команд
node bin/discord-lab.mjs pids                  # процессы Discord, видимые Frida
node bin/discord-lab.mjs inventory --pid <PID> # инвентарь модулей/символов (JSON/CSV)
node bin/discord-lab.mjs hook --pid <PID> --pattern 'SendRtp|OnIncomingRtp' # перехват вызовов
node bin/discord-lab.mjs summarize logs/calls-<PID>.jsonl # аналитическая сводка вызовов
node bin/discord-lab.mjs cdp targets           # доступные цели рендерера Electron
node bin/discord-lab.mjs cdp shot /tmp/page.png # снятие скриншота через CDP
```

Поддерживаемые команды CLI:
- `pids` — список процессов Discord, видимых Frida
- `inventory` — сбор инвентаря модулей, символов и экспортов
- `hook` — динамический перехват и трассировка вызовов функций
- `summarize` — аналитическая сводка вызовов из логов JSONL
- `cdp` — инспекция, eval, клики и скриншоты страниц рендерера
- `walk` — автоматический обход интерфейса и UI-прогоны
- `domains` — классификация и отчёт по доменам функций
- `static` — таблицы статических символов из бинарников

Собранная база знаний обратного инжиниринга находится в `/home/sanya/re-deep/` (read-only оракул) и `docs/re/`:
- `gateway-protocol.md`: трассы шлюза, ETF big-int, zstd-стримы и 4 логина READY.
- `rest-d02.md`: маршруты REST v9, заголовки, `X-Super-Properties` и коды ошибок.
- `voice-deep.md`: сигнализация RTC, RTP/RTCP пакеты, крипто-режимы SRTP и DAVE.
- `i18n-deep.md`: хэши ключей локализации (`xxh64 -> base64[0..6]`) для 36 языков.

---

## 4. Качество, тестирование и мутационный гейт

Проект следует бескомпромиссным стандартам качества **Strict TDD** и **5/5 Mutation Gate**:

```
       ▲
      / \       T4: Qualification Gate (Keyring live, Tarball, live-probes)
     /---\      
    /     \     T3: Release & Mutation Gate (5/5 mutations killed, ASan/TSan, SBOM)
   /-------\    
  /         \   T2: Integration & E2E (GPUI Headless Smoke, 50 E2E Journeys, SSIM >0.995)
 /-----------\  
/             \ T1: Worker Gate (Clippy -D warnings, Typecheck, 147 Meta, 920+ Unit)
───────────────
T0: Targeted    (Targeted test impact: node scripts/test-impact.mjs --run, <1 sec)
```

- **100% Pass Rate**: 147 мета-тестов, 920+ модульных тестов, 391+ GPUI контрактных тестов.
- **5/5 Mutation Kill Rate**: каждый pull request проходит через мутационный фаззер, убивающий 100% искусственных дефектов.
- **Визуальная регрессия**: SSIM > 0.995 на 15 эталонных экранах с динамическими JSON-масками.
- **Chaos Engineering**: 53 автоматизированных стресс-сценария (падения PipeWire, сетевой джиттер 20–60 мс, обрывы шлюза).

---

## 5. Быстрый старт и сборка

### Требования к окружению:
- **Rust**: 1.85+ (Edition 2024, `rustup default stable`)
- **Системные библиотеки**: `libasound2-dev`, `libpipewire-0.3-dev`, `libsecret-1-dev`, `libenchant-2-dev` (или `hunspell`)
- **Node.js**: 24+ (для запуска тестов и канбан-доски)

### Сборка нативного клиента:
```bash
# Клонирование репозитория
git clone https://github.com/wairuste-lab/discord-lab.git
cd discord-lab

# Сборка оптимизированного релизного бинарника
cargo build --release -p discord-gpui

# Запуск нативного клиента
./target/release/discord-gpui
```

### Запуск тестов:
```bash
# Мета-тесты архитектуры и документации (147 тестов)
npm run test:meta

# Все модульные и контрактные тесты
npm test

# Быстрый выборочный запуск тестов по затронутым файлам
node scripts/test-impact.mjs --run

# Полный квалификационный гейт
node scripts/gate.mjs --scope worker
```

### Канбан-доска проекта:
```bash
# Экспорт интерактивной HTML-версии канбан-доски
node scripts/mcp-kanban-board.mjs --export

# Открытие канбан-доски в браузере
xdg-open waves/kanban-board.html
```

---

## 6. Кросс-платформенная дистрибуция

Пакеты для всех основных операционных систем:
- **Linux Flatpak**: `packaging/linux/flatpak/` (`com.discord_lab.DiscordGpui`)
- **Linux AppImage**: `packaging/linux/appimage/` (автономный бандл)
- **Arch Linux**: `packaging/linux/arch/PKGBUILD`
- **Debian / Ubuntu**: `packaging/linux/debian/` (`.deb`)
- **Windows**: `packaging/windows/wix.xml` (инсталлятор WiX Toolset MSI + Portable `.zip`)
- **macOS**: `packaging/macos/build-dmg.sh` (DMG-образ с поддержкой Vibrancy)

---

## 7. Документация проекта

| Документ | Путь | Описание |
| :--- | :--- | :--- |
| **Паспорт проекта** | [`docs/PROJECT-REGISTRY.md`](docs/PROJECT-REGISTRY.md) | Канонический реестр крейтов, артефактов и тестов |
| **Бэклог переноса** | [`docs/feature-backlog.md`](docs/feature-backlog.md) | Матрица паритета и бэклог миграции функционала |
| **Анализ Vesktop** | [`docs/vesktop-analysis.md`](docs/vesktop-analysis.md) | Архитектурный разбор клиента Vesktop / Vencord |
| **Анализ Discord-Qt** | [`docs/discord-qt-analysis.md`](docs/discord-qt-analysis.md) | Сравнительный анализ альтернативного Qt-клиента |
| **Текущий статус** | [`docs/rust-client/STATUS.md`](docs/rust-client/STATUS.md) | Актуальные метрики памяти, FPS и прогресс волны |
| **Дорожная карта** | [`docs/rust-client/ROADMAP.md`](docs/rust-client/ROADMAP.md) | Вехи разработки, завершенные и будущие спринты |
| **Архитектура GPUI** | [`docs/specs/GPUI-ARCHITECTURE-MIGRATION.md`](docs/specs/GPUI-ARCHITECTURE-MIGRATION.md) | Переход с WebKit на WGPU / GPUI-kit |
| **Шифрование DAVE** | [`docs/specs/POST-BETA-ROADMAP-AND-CODING-INSTRUCTIONS.md`](docs/specs/POST-BETA-ROADMAP-AND-CODING-INSTRUCTIONS.md) | Спецификация E2EE MLS v1 |
| **Правила агентов** | [`AGENTS.md`](AGENTS.md) | Регламент работы воркеров, оркестратора и отделов |
| **Карта проекта** | [`docs/map.md`](docs/map.md) | Граф взаимосвязей модулей и ресёрч-слоёв |

---

## 8. Правовой статус и Clean-Room разработка

Данный проект разработан с применением строгой методологии **Clean-Room Reverse Engineering**:
- Реализация выполнена на чистом Rust без использования проприетарного исходного кода Discord Inc.
- Официальный клиент Discord использовался исключительно как локальный объект анализа сетевых протоколов и аудио-пайплайна.
- Проект не аффилирован с Discord Inc. Название Discord используется исключительно в информационных целях совместимости.
- Рекомендуемая лицензия для исходного кода: **GNU General Public License v3.0 (GPL-3.0)**.
