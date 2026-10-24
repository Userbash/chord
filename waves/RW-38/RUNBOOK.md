# RW-38 — RUNBOOK: усиленные тесты GUI

## 1. Быстрые (без дисплея)

```bash
# DOM-харнесс: реальный app.js в Node, без npm-зависимостей
node --test tests/unit/gui-spike-dom.test.mjs

# Модель + бенч + DOM
node --test tests/unit/gui-spike-model.test.mjs \
           tests/unit/gui-spike-bench.test.mjs \
           tests/unit/gui-spike-dom.test.mjs

# Rust: lib + IPC-контракт
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
```

## 2. Живые (нужен дисплей)

```bash
# Tauri launch (Rust, opt-in): окно + UI_SNAPSHOT + отсутствие GBM/panic
GUI_SPIKE_E2E=1 cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test launch -- --nocapture

# AT-SPI + X11: элементы внутри окна + PNG
python3 scripts/gui-window-probe.py --json /tmp/opencode/gui-probe.json --timeout 30

# Node-обёртка (входит в smoke-сьют; на дисплее запускается автоматически)
GUI_SPIKE_E2E=1 node --test tests/smoke/gui-window.smoke.test.mjs
GUI_SPIKE_E2E=1 node --test tests/smoke/gui-spike.smoke.test.mjs
```

## 3. Метрики с FPS

```bash
# RSS + FPS/p95 из app.log в одном JSON
node scripts/gui-spike-bench.mjs --pid <PID> --tree \
  --bench-log waves/RW-37/app.log --json waves/RW-38/metrics.json
```

## 4. Гейт

```bash
npm run gate:worker          # на машине с дисплеем GUI-smoke обязателен
GUI_SPIKE_E2E=0 npm run gate:worker   # явно отключить GUI-smoke
```

Политика: `Failed to create GBM buffer`, `Gdk-Message:`, `panic`, `segmentation fault`
в stderr → **failed**, не skip. Устаревший бинарь (старше `ui/*`) → **failed**:
Tauri встраивает `ui/*` при сборке, иначе проба проверяет старую копию.

## 5. Требования среды

- X11 `DISPLAY`, GTK3 + WebKitGTK, PyGObject (`gi`), Pillow, AT-SPI
  (`GTK_MODULES=gail:atk-bridge`, `NO_AT_BRIDGE=0`).
- Без сети и установок; `/home/sanya/re-deep`, `logs/`, `functions/`, `evidence/` — read-only.
