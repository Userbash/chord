# CLAUDE.md — ECC 2.0 Control Plane & Development Guidelines for `discord-lab`

This file is automatically loaded across all sessions, activating **ECC 2.0 (Everything Claude Code)** by default across the `discord-lab` repository.

## 1. Default ECC Configuration
- **ECC 2.0 Control Plane Manifest:** [`.claude/ecc2.toml`](.claude/ecc2.toml)
- **ECC ↔ Orca Orchestration Bridge:** [`scripts/ecc-orca-bridge.sh`](scripts/ecc-orca-bridge.sh)
- **Department Specifications & Integration Rules:** [`.agents/rules/ecc-discord-lab-departments.md`](.agents/rules/ecc-discord-lab-departments.md)
- **Core Repository Rules:** [`AGENTS.md`](AGENTS.md), [`PROCESS.md`](PROCESS.md), [`TEAM.md`](TEAM.md), [`TEST-POLICY.md`](TEST-POLICY.md)
- **ECC Skills & Subagents:** `.agents/skills/*`, `.agents/agents/*`, `.agents/rules/common-*.md`

## 2. Worker Lifecycle via ECC and Orca
1. **Specialist Departments (`write-scope` with zero overlap):**
   - `g1_backend`: `rust/crates/discord-{core,codec,media,store,i18n}`, `rust/discord-gpui/src/*.rs`, `rust/discord-gpui/tests/*_contract.rs`
   - `g2_frontend`: `rust/discord-gpui/src/views/**/*.rs`, `tests/unit/*.test.mjs`
   - `g3_design`: `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`
   - `g4_qa`: `tests/{meta,unit,smoke,e2e}/`, `scripts/test-impact.mjs`, `QA-BUILD-*.md`
   - `g5_voice_platform`: `rust/crates/discord-voice/**`, `rust/crates/discord-platform/**`, voice opcodes `18..=22`, SRTP / DAVE MLS v1 E2EE
   - `g7_re_rea`: Read-only clean-room parity discovery against `docs/re/`, `docs/reference-projects/`, and MCP `rea`
2. **Model Routing & Memory Management (`WORKER_RAM_LIMIT <= 25 GB`):**
   - **Codex Prohibited:** Never invoke `--agent codex`.
   - **Antigravity (`--agent antigravity --model gemini-4-argon-high --effort high`):** Primary implementation of new features and parallel workers.
   - **Review / Validation:** Technical review (`REVIEW.md`), QA verification, and 5/5 control mutations executed via Gemini models.
   - **Worker Pool:** At most `5` concurrent active worker terminals (`max_parallel_sessions = 5` in `.claude/ecc2.toml`), with at most one concurrent `cargo` build process on the shared `CARGO_TARGET_DIR=./rust/target`.
3. **Mandatory Quality and Verification Loop:**
   - Syntax and type validation (`node --check`, `npm run typecheck`, `cargo check`/`cargo build`).
   - Clean binary compilation for distribution with `SHA256SUMS` generation.
   - Full regression and meta-test verification (`npm run test:meta`).
