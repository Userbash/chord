# discord-lab: session checkpoint (сжатый контекст)

Точка входа для нового диалога. Читать первым; детали — точечно по ссылкам.
Обновлён: **2026-11-11** (Волна **RW-44** в активной разработке; миграция на нативный GPUI завершена).

---

## 1. Текущий статус проекта (2026-11-11)

- **Нативный Rust GPUI клиент (`discord-gpui`)**:
  - Полный отказ от WebKit/Tauri прототипа в пользу прямого GPU-рендеринга WGPU 120 FPS (`rust/discord-gpui/`).
  - Релизный бинарник: [`rust/dist/discord-gpui`](file:///home/sanya/orca/project/rust/dist/discord-gpui) — **42 МБ** (stripped ELF, thin LTO, opt-level 3).
  - Контрольная сумма в [`rust/dist/SHA256SUMS`](file:///home/sanya/orca/project/rust/dist/SHA256SUMS): `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e`.
  - Headless Smoke: старт `<500ms`, VmRSS **11.26 МБ** (лимит бюджета `<60 МБ`), 120 FPS frame budget соблюдён.
  - Детерминированный E2E-контур: [`rust/discord-gpui/tests/gpui_e2e_contract.rs`](file:///home/sanya/orca/project/rust/discord-gpui/tests/gpui_e2e_contract.rs) — 50 сквозных пользовательских путей (100% pass за 0.08с).

- **Текущая волна: Wave RW-44 (Full-Scale QA Automation, AT-SPI, Fuzzing & E2E)**:
  - Активный run оркестрации: `run_a1698b82d00f` под управлением ECC 2.0 Control Plane.
  - **121 / 128 задач выполнено (Done)**:
    - Завершена и принята задача **`TASK-904`** (Визуальная регрессия GPUI SSIM >0.99, JSON-маски, 15 Golden Snapshots, HTML diff report).
  - **7 параллельных воркеров в активной разработке**:
    - `TASK-901`: Headless Harness (Virtual Wayland + Mock Gateway Daemon).
    - `TASK-902`: Семантический GUI-драйвер AT-SPI D-Bus (`org.a11y.Bus`).
    - `TASK-903`: Фаззинг сетевых протоколов и криптографии DAVE MLS v1.
    - `TASK-905`: Стенд Chaos Engineering (джиттер, packet drop, TCP drop, PipeWire).
    - `TASK-906`: Бенчмаркинг 120 FPS и профилирование аллокаций памяти jemalloc.
    - `TASK-907`: Автоматизированный гейт качества кода (Clippy `-D warnings`, 5/5 мутаций).
    - `TASK-908`: Регламент и сценарии ручного тестирования Manual QA Suite.
  - Канбан-доска: [`waves/kanban-board.html`](file:///home/sanya/orca/project/waves/kanban-board.html) и [`waves/kanban-board.json`](file:///home/sanya/orca/project/waves/kanban-board.json).

---

## 2. Архитектура управления и конвейера CI/CD

- **ECC 2.0 Control Plane & Orchestration**:
  - Управление: `scripts/ecc` (`default_agent_profile = "pm_review"`), конфигурация `.claude/ecc2.toml`.
  - Архитектурная спецификация: [`docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md`](file:///home/sanya/orca/project/docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md).
  - Граф знаний: 17 коннекторов синхронизированы через `scripts/ecc graph connector-sync --all`.
  - Контроль памяти: `WORKER_RAM_LIMIT <= 25 GB` (свободно 28–34 GiB, риск OOM 0%).
  - Маршрутизация моделей: strictly Gemini (`gemini-4-carbon-high` для P0 крипто/ядра, `gemini-4-argon-high` для BFF/QA, `gemini-3.8-flash-high` для UI/доков; запрет `codex` и `deepseek`).

- **Конвейер проверки кода (Пирамида T0–T4)**:
  - **T0 (Targeted):** `node scripts/test-impact.mjs --run` (<1 сек).
  - **T1 (Worker Gate):** `cargo fmt --check`, `clippy -D warnings`, `node scripts/gate.mjs --scope worker`.
  - **T2 (Integration & E2E):** `gpui-smoke`, `gpui-e2e` (50 journeys), `gpui-visual` (SSIM >0.995).
  - **T3 (Release & Mutation):** 5/5 намеренных мутаций убиты, ASan/TSan, SBOM, SHA256SUMS.
  - **T4 (Qualification):** `node scripts/gate.mjs --scope qualify` с live-keyring хранилищем.

- **Ветвление и GitHub**:
  - `<type>/task-<id>-<slug>` $\to$ PR в `unstable` $\to$ Двойное ревью (PM `ACCEPT.md` + Tech Reviewer `REVIEW.md` 5/5 мутаций) $\to$ `unstable` $\to$ `testing` (BUILD-00X) $\to$ `beta` $\to$ `main`.
  - 13 обязательных статусов жизненного цикла.
