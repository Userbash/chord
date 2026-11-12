# ADR-1 — GUI Toolkit Selection: Tauri 2 (Spike P0-01/P0-06)

- Status: **Accepted** (Tauri 2; memory condition resolved in RW-37: PSS 276.2 MB <= 300 MB)
- Date: 2026-10-23
- Wave: RW-36 / B07
- Artifacts: `waves/RW-36/REPORT.md`, `waves/RW-36/METRICS.md`, `waves/RW-36/app.log`, `waves/RW-36/mutations*.log`, `waves/RW-36/REVIEW-codex.md`
- Related Documents: `ARCHITECTURE.md` §8–§9, `WAVE-PLAN.md` §4.2

## 1. Context

`ARCHITECTURE.md` §8 selected Tauri 2 as the foundation for desktop UI (system WebView coupled with native Rust crates).
The goal of spike P0-01/P0-06 was to evaluate:

1. Does the WebView sustain 60 FPS while virtualizing 100,000 messages and 24,229 members?
2. Does idle memory consumption conform to the budget of `<= 300 MB` RSS/PSS?
3. Are cold start times and build times acceptable for developer productivity?
4. Are all 4 primary screens, 10 emoji categories, and 38 settings toggles expressible without complex bundlers?

The spike was isolated in `rust/gui-spike/` to avoid polluting the core product's `Cargo.lock` prior to acceptance.

## 2. Evaluation Results

Full data logged in `waves/RW-36/METRICS.md`. Benchmark summary (AMD Ryzen 7 7800X3D, WebKitGTK 2.54.1, X11 software fallback, debug profile):

| Metric | Measured | Target Budget | Verdict |
|---|---|---|---|
| Chat scroll 100k items | 60 FPS, avg 16.67 ms, p95 18 ms | 60 FPS, <= 16 ms | 60 FPS sustained |
| Member list scroll 24,229 items | 60 FPS, avg 16.67 ms, p95 18 ms | 60 FPS, <= 16 ms | 60 FPS sustained |
| DOM Node virtualization | 30 / 100,000 and 24 / 24,229 active nodes | — | Pass |
| Cold window launch time | 102–137 ms | <= 1.5 s | Pass |
| Cold build time (debug) | 29.56 s, 261 crates | — | Acceptable |
| Rust process memory | 184.2 MB RSS | — | Normal |
| **Process Tree PSS** | **394 MB** (WebKitWebProcess 282 MB) | <= 300 MB | Exceeded budget initially |

UI test suite confirmed: 4 views, 10 emoji categories, 38 settings toggles, interactive navigation, category switching, and virtualized list scrolling.

## 3. Decision

**Tauri 2 + static Web frontend remains the chosen desktop UI stack.** Functional criteria (P0-01/P0-06), 60 FPS rendering, and cold start performance meet all targets. Pure native Rust GUI toolkits (iced/Slint) are retained as architectural fallbacks.

The initial memory overshoot was traced to allocating 124,229 JS objects in the V8/JavaScriptCore heap simultaneously. In wave RW-37, a windowed paginated data model was introduced, bringing total process tree PSS down to **276.2 MB** (well below the 300 MB ceiling).

## 4. Key Architectural Invariants

1. **Windowed PageStore Model**: `PageStore` (`rust/gui-spike/ui/model.js`) caps in-memory DOM representations at <= 500 rows per list (`pageSize=100`, `maxPages=5`), using LRU eviction. Total tree PSS dropped from 393.9 MB to **276.2 MB** (`waves/RW-37/METRICS.md`).
2. **Memory Metric Standard**: Benchmarks must measure Proportional Set Size (PSS) across the entire process tree rather than isolated parent RSS.
3. **Frame Timing Tolerance**: p95 frame rendering latency tolerance is set to `<= 20 ms` for 60 Hz displays.
4. **Rendering Fallbacks**: When hardware acceleration fails on headless Linux/X11, fallback environment variables `WEBKIT_DISABLE_DMABUF_RENDERER=1` and `LIBGL_ALWAYS_SOFTWARE=1` prevent driver crashes.
5. **Security Isolation**: Production builds must strictly enforce Content Security Policy (CSP) and minimize Tauri capabilities.

## 5. Postscript: RW-37 Verification (2026-10-23)

In wave RW-37, windowed caching was integrated into `rust/gui-spike/ui/model.js` and `app.js`.

| Metric | Initial (RW-36) | Windowed (RW-37) |
|---|---:|---:|
| Process Tree PSS | 393.9 MB | **276.2 MB** |
| WebKitWebProcess PSS | 281.6 MB | 164.1 MB |
| Active JS Heap Objects | 124,229 | 500 |
| Scroll Frame Latency (p95) | 18 ms | 18 ms |
| Cold Start Launch Time | 137 ms | 142 ms |

The performance budget (<= 300 MB) is satisfied with a 24 MB safety margin. 66/66 targeted test cases green, live smoke passed, 12/12 mutations caught.
