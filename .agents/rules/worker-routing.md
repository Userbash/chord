# Worker Routing & Effort Selection (`orca-agent-routing` — RW-40)

This rule applies when the PM / Tech Lead distributes tasks among workers in the Chord project.

## 1. Available Engines, Models, and Effort Levels

Workers are launched via `orca agent launch --worktree current --agent <engine> --model <model> [--effort <low|medium|high>] --prompt "..." --json`.

| Engine (`--agent`) | Model (`--model`) | `--effort` | Purpose & Priority |
|---|---|---|---|
| `antigravity` | `gemini-4-argon-high` | `high` | **G1 Core Rust Architecture / RE Deep**: complex core architecture (`discord-core`), reducers, cryptography/transport, multi-file integration, deep RE via REA MCP |
| `antigravity` | `gemini-4-argon-medium` | `medium` | **G1 GUI-Spike Backend / Security P1**: Rust backend `gui-spike` (`net.rs`, `attachments.rs`, `search.rs`, `messages.rs`), P1/P2 closure |
| `antigravity` | `gemini-4-argon-low` | `low` | **G5 Platform / Voice Offline**: revision and expansion of offline tests for `discord-voice`, RTC/media invariant verification |
| `antigravity` | `gemini-3.8-flash-medium` | `medium` | **G2 Frontend Complex UI**: composite UI features with state machines and modals (`composer.js`, attachment queue, gift modal) |
| `antigravity` | `gemini-3.8-flash-low` | `low` | **G3 Design / Specs / QA**: design specs (`DESIGN/*.md`), tokens, CSS verification, quick doc fixes |
| `opencode` | `deepseek/deepseek-v4.1-flash` | *(not specified)* | **Independent Review, QA & Mutation Audit (Beijing window Mon–Fri ≥ 18:00, Sat–Sun 24h)**: technical review (`REVIEW.md`), mutation gate `5/5`, QA verification and Pass 2/3 refactoring |

> **`codex` banned:** `--agent codex` is fully prohibited (`AGENTS.md` §3.2). All workers are managed via ECC 2.0 Control Plane (`.claude/ecc2.toml`) and `scripts/ecc-orca-bridge.sh` (`orca orchestration worker-start`).

## 2. Continuous Department Load & Isolation Rules

1. **No department idles:** once a worker completes a card, its artifacts are immediately passed to the review batch (`REVIEW-BATCH-*`), and the department receives the next card from the queue (`orca agent launch`).
2. **Non-overlapping file scopes:** each active worker owns its own set of files; files belonging to cards under review are frozen until an `ACCEPT`/`RETURN` verdict is issued.
3. **`cargo` isolation with parallel Rust workers:** when multiple Rust workers (`G1 Core`, `G1 GUI-Spike`, `G5 Voice`) run simultaneously, each must launch `cargo` with its own `CARGO_TARGET_DIR` (`/tmp/target-g1-core`, `/tmp/target-g1-spike`, `/tmp/target-g5-voice`) to avoid blocking the shared `target/`.
