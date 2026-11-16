# Architecture Codemap: Chord / Discord-Lab

> **Generated:** 2026-11-16 12:17:58 UTC
> **Engine:** SocratiCode AST Dependency Graph & Code Intelligence
> **Total Indexed Files:** 1,534 | **Dependency Edges:** 8,032 | **Average Coupling:** 5.2 dep/file

---

## 1. High-Level Architectural Flow (Mermaid)

```mermaid
flowchart TD
    subgraph L1_CORE ["Layer 1: Protocol & Core Crates (Rust)"]
        CORE_GATEWAY["discord-core::gateway\n(zstd, ETF, opcodes)"]
        CORE_REST["discord-core::rest\n(HTTP v9, rate limits)"]
        CORE_VOICE["discord-voice\n(RTP, Opus, DAVE MLS, SRTP)"]
        CORE_STATE["discord-core::state\n(Reducer, RingBuffer)"]
    end

    subgraph L2_BFF ["Layer 2: Native BFF Host (Tauri)"]
        BFF_MAIN["rust/gui-spike/src/main.rs\n(81 Tauri Handlers)"]
        BFF_PLUGINS["BFF Domain Plugins\n(Chat, Voice, Guilds, Invites)"]
        BFF_DTO["DTO Serialization Layer\n(GuildSync, VoiceRtc, Snapshot)"]
    end

    subgraph L3_UI ["Layer 3: Frontend WebView & Signals"]
        UI_SHELL["rust/gui-spike/ui/app.js\n(App Shell & Router)"]
        UI_BINDINGS["rust/gui-spike/ui/bindings.js\n(Typed Tauri Invokers)"]
        UI_FEATURES["Features & Modals (100+ files)\n(Chat, Voice Stage, Emoji Grid)"]
        UI_DOM["index.html\n(Virtual List, Focus Trap)"]
    end

    subgraph L4_QA ["Layer 4: Quality & Test Pyramid"]
        TEST_UNIT["Fast Unit Tests (242 suites)"]
        TEST_CONTRACT["Rust BFF Contracts (134 suites)"]
        TEST_SMOKE["Component Smoke (8 suites)"]
        TEST_E2E["Real Native E2E (13 suites)"]
        TEST_PARITY["Parity & FSM Audit Engine"]
    end

    CORE_GATEWAY --> CORE_STATE
    CORE_REST --> CORE_STATE
    CORE_VOICE --> CORE_STATE

    CORE_STATE --> BFF_MAIN
    BFF_MAIN --> BFF_PLUGINS
    BFF_PLUGINS --> BFF_DTO

    BFF_DTO <--> UI_BINDINGS
    UI_BINDINGS --> UI_SHELL
    UI_SHELL --> UI_FEATURES
    UI_FEATURES --> UI_DOM

    TEST_UNIT -.-> UI_FEATURES
    TEST_CONTRACT -.-> BFF_DTO
    TEST_SMOKE -.-> BFF_MAIN
    TEST_E2E -.-> UI_DOM
    TEST_PARITY -.-> UI_BINDINGS
```

---

## 2. Component Inventory & Top Architectural Hubs

### Top Connected Architectural Hubs (SocratiCode Blast Radius):
1. **`rust/crates/chord-core/src/state/mod.rs`** (273 connections) — Центральное ядро стейт-машины протокола.
2. **`rust/crates/chord-core/src/lib.rs`** (256 connections) — Корневой экспорт моделей и DTO.
3. **`rust/gui-spike/src/main.rs`** (104 imports, 81 handlers) — Хост нативного окна и точка входа Tauri BFF.
4. **`rust/gui-spike/ui/app.js`** (50 imports, 6 dependents) — Главный контроллер пользовательского интерфейса.
5. **`rust/crates/chord-voice/src/lib.rs`** (150 connections) — Аудио-движок и RTC сигнализация.

---

## 3. Layer Breakdown

| Architectural Layer | File Count | Primary Role | Key Invariants |
|---|---|---|---|
| **Core Crates (`rust/crates/*`)** | 289 | Протокол, криптография, парсинг шлюза | `#![forbid(unsafe_code)]`, zero-copy ring buffers |
| **BFF Host (`rust/gui-spike/src/*`)** | 58 | Связка Rust $\leftrightarrow$ WebView, IPC команды | 81 зарегистрированная команда, строгая типизация DTO |
| **GPUI Native Client** | 0 | Альтернативный рендерер на GPU | 60 FPS рендеринг, прямое подключение к Metal/Vulkan |
| **Frontend UI (`rust/gui-spike/ui/*`)** | 153 | Компоненты интерфейса, модальные окна, DOM | W3C ARIA диалоги, Focus Trap, отсутствие утечек памяти |
| **Test Suites (`tests/*`)** | 301 | Пирамида верификации качества | 100% assertions, 0 broken imports, 5/5 mutation gate |

---

## 4. Test Pyramid Distribution

```
              ▲
             / \     [ 13 E2E Suites ]         - Real Native Window (AT-SPI/X11) & Gateway
            /───\
           /     \    [ 8 Smoke Suites ]       - Component Bootstrap & PipeWire Sanity
          /───────\
         /         \   [ 134 Contract Suites ]  - 9-Dimension BFF Boundary Invariants
        /───────────\
       /             \  [ 242 Unit Suites ]      - Fast Signals, Pure State, DTO Parsers
      /───────────────\
     [ 22 Meta Suites ] - Architecture Integrity, Plan Sync, Breakpoints & Security
```

---

## 5. Automated Verification Workflows

- `npm run codemap` — Перестроить текущую архитектурную карту через граф SocratiCode.
- `npm run gui:audit` — Экспресс-проверка согласованности вызовов UI и бэкенда (30 мс).
- `npm run test:gui:live` — Тестирование реального живого окна десктоп-клиента (4 сек).
- `npm run qa:system` — Полная 10-этапная сертификация качества.
