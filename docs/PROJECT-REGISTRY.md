# Единый реестр проекта discord-lab (Unified Project Registry)

> **Статус документа:** `Canonical Project Registry (v2.0)`  
> **Дата актуализации:** Ноябрь 2026 г.  
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
| **Текущая волна** | **Wave RW-44**: Full-Scale QA Automation, AT-SPI Driver, Fuzzing & E2E |
| **Релизный артефакт** | [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) (42 МБ, stripped ELF, Thin LTO, opt-level 3) |
| **Контрольная сумма SHA-256**| `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e` |
| **Целевые платформы** | Linux (Wayland: GNOME, KDE, Hyprland, Sway; X11 fallback) |
| **Статус задач** | **124 выполнено (Done)** из 128 задач на канбан-доске (**96.9%**) |
| **Статус тестов** | **391+ тестов** — 100% Passed (0 failed, 0 flaky, 5/5 мутаций убиты) |

---

## 2. Реестр крейтов Rust (Workspace Crates)

| Крейт | Путь | Назначение и ключевые подсистемы | Тестовые контракты |
| :--- | :--- | :--- | :--- |
| **`discord-gpui`** | `rust/discord-gpui/` | **Основное нативное GUI-приложение**: Server Rail, виртуализированный чат 100k сообщений, 8 вкладок настроек, модалки трансляции экрана, капчи, контекстные меню, трей, AT-SPI. | `gpui_e2e_contract.rs`<br>`visual_regression_contract.rs`<br>`chaos_engineering_contract.rs`<br>`a11y_atspi_contract.rs`<br>`perf_benchmark_contract.rs`<br>`manual_qa_suite_contract.rs`<br>`release_smoke_contract.rs` |
| **`discord-core`** | `rust/crates/discord-core/` | **Ядро протокола**: клиент REST v9, шлюз Gateway v9 (HELLO, Heartbeat, IDENTIFY, RESUME, VOICE_STATE_UPDATE), редьюсер состояния, битовые маски прав доступа. | `permissions_contract.rs`<br>`ready_read_state.rs`<br>`channels_contract.rs` |
| **`discord-voice`** | `rust/crates/discord-voice/` | **Голосовой и медиа-стек**: голосовой UDP-стример, IP Discovery (74B), Keepalive (8B), RTP/RTCP пакеты, SRTP (AES-256-GCM / ChaCha20), DAVE E2EE MLS v1 (`RFC 9420`). | `reception_contract.rs`<br>`rtcp_contract.rs`<br>`rtp_video_contract.rs`<br>`fuzz_protocol_dave_contract.rs` |
| **`discord-media`** | `rust/crates/discord-media/` | **Аппаратные медиа-буферы**: кольцевой кольцевой буфер `AudioRingBuffer` без аллокаций, коннекторы захвата экрана X11 и Wayland Portal. | `buffer_contract.rs`<br>`ring_buffer_contract.rs`<br>`capture_contract.rs` |
| **`discord-platform`** | `rust/crates/discord-platform/` | **Платформенные интеграции Linux**: `org.freedesktop.portal.ScreenCast`, D-Bus уведомления, системный трей, Keyring через FFI к libsecret. | `screencast_contract.rs`<br>`screencast_portal_contract.rs`<br>`keyring_contract.rs` |
| **`discord-store`** | `rust/crates/discord-store/` | **Локальное хранилище данных**: SQLite FTS5 полнотекстовый поиск, режим WAL, кэш сообщений и настроек. | `store_contract.rs`<br>`fts5_contract.rs` |
| **`discord-codec`** | `rust/crates/discord-codec/` | **Кодеки и декомпрессия**: декодер и энкодер ETF бинарного формата, zstd-стриминг, защита от compression-bomb атак. | `zstd_bomb_contract.rs`<br>`etf_properties.rs` |
| **`discord-i18n`** | `rust/crates/discord-i18n/` | **Локализация и интернационализация**: 36 языков, парсер AST, CLDR плюрализация, хеши ключей xxh64→base64. | `i18n_contract.rs` |

---

## 3. Реестр релизных артефактов и манифестов

| Артефакт | Путь | Описание / Метрики |
| :--- | :--- | :--- |
| **Релизный бинарник** | [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) | 42 МБ, stripped ELF, Thin LTO, opt-level 3 |
| **Манифест сумм** | [`rust/dist/SHA256SUMS`](file:///home/sanya/orca/project/rust/dist/SHA256SUMS) | SHA-256 контрольные суммы релизных бинарников |
| **Манифест SBOM** | `rust/dist/sbom.spdx.json` | SPDX 2.3 воспроизводимый манифест лицензий зависимостей |
| **15 Golden эталонов** | `rust/discord-gpui/tests/goldens/*.png` | Эталонные снимки экрана GPUI (чат, настройки, модалки) |
| **Динамические JSON-маски** | `rust/discord-gpui/tests/goldens/dynamic_masks_*.json` | Маски исключений (часы, FPS, аватары, пинг) |
| **Интерактивный HTML Diff** | `rust/discord-gpui/tests/goldens/visual_diff_report.html` | Интерактивный отчёт сравнения скриншотов (Slider / Onion Skin / Red Diff) |
| **Канбан-доска (JSON)** | [`waves/kanban-board.json`](file:///home/sanya/orca/project/waves/kanban-board.json) | 128 задач со статусами, отделами и критериями приёмки |
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
| **T1** | Worker Gate | `scripts/gate.mjs --scope worker` | ~150 | `< 30 с` (Green) |
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
5. **`scripts/test-impact.mjs`**:
   * Ультрабыстрый анализатор зависимостей файлов и тестов для T0 верификации.
6. **`scripts/sbom.mjs`**:
   * Автономный офлайн-генератор и валидатор манифеста лицензий SPDX 2.3.

---

## 6. Реестр нормативной и технической документации

* 📘 [`PROJECT-CONTEXT.md`](file:///home/sanya/orca/project/PROJECT-CONTEXT.md) — Сжатый актуальный контекст проекта и точка входа.
* 🏛 [`docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md`](file:///home/sanya/orca/project/docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md) — Архитектура управления ECC 2.0, конвейер T0–T4 и процессы Git/GitHub.
* 📊 [`docs/rust-client/STATUS.md`](file:///home/sanya/orca/project/docs/rust-client/STATUS.md) — Фактический статус реализации подсистем и результаты контрактных тестов.
* 🗺 [`docs/rust-client/ROADMAP.md`](file:///home/sanya/orca/project/docs/rust-client/ROADMAP.md) — Стратегическая дорожная карта и завершённые вехи.
* 🗺 [`docs/map.md`](file:///home/sanya/orca/project/docs/map.md) — Полная архитектурная карта проекта в формате Mermaid.
* 📋 [`waves/STATE.md`](file:///home/sanya/orca/project/waves/STATE.md) — Журнал фиксации волн разработки и текущего среза.
* 🧪 [`docs/specs/QA-AUTOMATION-STRATEGY-AND-TASKS.md`](file:///home/sanya/orca/project/docs/specs/QA-AUTOMATION-STRATEGY-AND-TASKS.md) — Стратегия автоматизации тестирования и декомпозиция Wave RW-44.
* 📝 [`docs/specs/MANUAL-QA-SUITE.md`](file:///home/sanya/orca/project/docs/specs/MANUAL-QA-SUITE.md) — Регламент и чек-листы ручного и исследовательского тестирования.
