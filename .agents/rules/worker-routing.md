# Worker Routing & Effort Selection (`orca-agent-routing`)

This rule applies when the PM / Tech Lead distributes tasks among workers in the Chord project.

## 1. Complexity-First Routing Matrix (Priority `P0–P3` ≠ Model Tier)

Queue priority (`P0–P3`) determines **execution order only**. Select `--model` and `--effort` strictly by the **technical complexity** of the task so heavy models and reasoning budgets are never wasted on simple work:

| Tier | Engine (`--agent`) | Model (`--model`) | `--effort` | Scope & Qualifying Tasks |
|---|---|---|---|---|
| **S — Trivial** | `antigravity` | `gemini-3.8-flash-low` | `low` | Single-line switches (`P0-1.1`), CI/config tweaks (`H-FIX-17`), design specs & tokens (`G3`), documentation & parity matrices (`G6`) |
| **M — Standard UI / Light Infra** | `antigravity` | `gemini-3.8-flash-medium` / `gemini-3.8-flash-high` | `medium` | UI views, modals, preferences (`G2`), PM DoD acceptance (`ACCEPT.md`) |
| **M — Light Rust / DevOps** | `antigravity` | `gemini-4-argon-low` | `low` | DevOps release/gate scripts, lightweight `P2/P3` D-Bus/IPC/autostart adapters, offline test expansion |
| **L — Standard Core / QA** | `antigravity` | `gemini-4-argon-medium` | `medium` | Standard Rust BFF/REST services (`G1`), Voice/Platform session wiring (`G5`), QA contract test suites (`G4`) |
| **L-High — Deep Core / Review / RE** | `antigravity` | `gemini-4-argon-high` | `high` | Complex Gateway/WS state machines, permissions/KDF engines, independent `5/5` mutation technical review (`REVIEW.md`), deep `G7 RE` tracing |
| **XL — Hardcore Concurrency & Crypto** | `antigravity` | `gemini-4-carbon-high` | `high` (`xhigh` on escalation only) | **Exclusively** zero-copy/lock-free 60 FPS ring buffers, DAVE MLS v1 / SRTP / AES-GCM cryptography, and the reactive `Tokio <-> GPUI AppState` bridge |

> **Provider Bans:** `--agent codex` and `deepseek` are strictly prohibited (`AGENTS.md` §3). All workers run on `--agent antigravity` via ECC 2.0 (`.claude/ecc2.toml`) and `scripts/ecc-orca-bridge.sh` (`orca orchestration worker-start`).

## 2. Continuous Department Load & Isolation Rules

1. **No department idles:** once a worker completes a card, its artifacts are immediately passed to review (`ACCEPT.md` + `REVIEW.md`), its terminal is reclaimed (`worker-release`), and the slot receives the next card from the queue.
2. **Non-overlapping file scopes:** each active worker owns its own set of files; files belonging to cards under review remain frozen until accepted or returned.
3. **`cargo` serialization (`WORKER_RAM_LIMIT <= 25 GB`):** serialize builds on shared `CARGO_TARGET_DIR` via `flock /tmp/chord-cargo.lock` (see `.agents/rules/cargo-flock.md`) so concurrent `rustc` processes never trigger OOM on `Swap: 0B`.
