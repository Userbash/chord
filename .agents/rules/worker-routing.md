# Worker Routing & Effort Selection (`orca-agent-routing`)

This rule applies when the PM / Tech Lead distributes tasks among workers in the Chord project.

## 1. Complexity-First Routing Matrix (Priority `P0–P3` ≠ Model Tier)

Queue priority (`P0–P3`) determines **execution order only**. Select `--model` and `--effort` strictly by the **technical complexity** of the task so heavy models and reasoning budgets are never wasted on simple work:

| Tier / Role | Engine (`--agent`) | Model (`--model`) | Default `--effort` (Allowed Range) | Functional Scope & Qualifying Tasks |
|---|---|---|---|---|
| **S — Routine Code / CI / Config** | `antigravity` | `gemini-4-hydrogen-low` | `low` (`low`–`medium`) | Routine single-step edits (`P0-1.1`, `P2-1.2`, `P2-3.2`), CI/config tweaks (`H-FIX-17`), packaging manifests (`TASK-1015A`, `TASK-1016*`), formatting & cleanup (`G0`) |
| **S — Specs & Docs** | `antigravity` | `gemini-3.8-flash-low` | `low` (`low`) | Design specs & tokens (`G3`), documentation & parity matrices (`G6`) |
| **M — Routine Adapters / Utilities** | `antigravity` | `gemini-4-hydrogen-medium` | `medium` (`low`–`medium`) | Routine boilerplate wrappers, header/locale helpers (`P2-1.1`), module splits (`P2-3.1`), offline installers (`TASK-1011A..1013A`), fixture updates |
| **M — Standard UI / PM** | `antigravity` | `gemini-3.8-flash-medium` / `gemini-3.8-flash-high` | `medium` (`low`–`medium`) | UI views, modals, preferences (`G2`), PM DoD acceptance (`ACCEPT.md`) |
| **M — Light Rust / DevOps** | `antigravity` | `gemini-4-argon-low` | `low` (`low`–`medium`) | DevOps release/gate scripts, lightweight `P2/P3` D-Bus/IPC/autostart adapters, offline unit/contract tests |
| **L — Standard Core / QA** | `antigravity` | `gemini-4-argon-medium` | `medium` (`medium`) | Standard Rust BFF/REST services (`G1`), Voice/Platform session wiring (`G5`), QA contract test suites (`G4`) |
| **L-High — Deep Core / Review / RE** | `antigravity` | `gemini-4-argon-high` | `high` (`medium`–`high`) | Complex Gateway/WS state machines, permissions/KDF engines, independent `5/5` mutation technical review (`REVIEW.md`), deep `G7 RE` tracing |
| **XL — Hardcore Concurrency & Crypto** | `antigravity` | `gemini-4-carbon-high` | `high` (`high`, `xhigh` only on escalation) | **Exclusively** zero-copy/lock-free 60 FPS ring buffers, DAVE MLS v1 / SRTP / AES-GCM cryptography, and the reactive `Tokio <-> GPUI AppState` bridge |
| **Orchestrator (Argon) — Wave Lead** | `antigravity` | `gemini-pro-orchestrator-argon` | `medium` (`low`–`high`) | **Coordinator / Team Lead:** queue prioritization (`P0–P3`), `write-scope` locking, 1-to-1 slot rotation (`<= 6` slots, `< 25 GiB`), worker review & cherry-pick integration |
| **Orchestrator (Carbon) — Chief Architect** | `antigravity` | `gemini-pro-orchestrator` | `high` (`medium`–`xhigh`) | **Chief Architect / Escalation Lead:** multi-module/cross-crate design, resolving merge/spec conflicts, Tier XL arbitration, and re-planning failed steps |

> **Hydrogen High / XHigh Rule:** `gemini-4-hydrogen-high` / `gemini-4-hydrogen-xhigh` are supported in `ecc-orca-bridge.sh` for high-volume mechanical sweeps only; never use `hydrogen` for Tier XL cryptography or lock-free concurrency, and never use `carbon`/`orchestrator` models for Tier S/M routine tasks.

> **Provider Bans:** `--agent codex` and `deepseek` are strictly prohibited (`AGENTS.md` §3). All workers run on `--agent antigravity` via ECC 2.0 (`.claude/ecc2.toml`) and `scripts/ecc-orca-bridge.sh` (`orca orchestration worker-start`).

## 2. Continuous Department Load & Isolation Rules

1. **No department idles:** once a worker completes a card, its artifacts are immediately passed to review (`ACCEPT.md` + `REVIEW.md`), its terminal is reclaimed (`worker-release`), and the slot receives the next card from the queue.
2. **Non-overlapping file scopes:** each active worker owns its own set of files; files belonging to cards under review remain frozen until accepted or returned.
3. **`cargo` serialization (`WORKER_RAM_LIMIT <= 25 GB`):** serialize builds on shared `CARGO_TARGET_DIR` via `flock /tmp/chord-cargo.lock` (see `.agents/rules/cargo-flock.md`) so concurrent `rustc` processes never trigger OOM on `Swap: 0B`.
