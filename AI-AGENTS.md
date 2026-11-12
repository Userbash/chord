# 🤖 AI Agents Guide — Chord Project

> **Machine-readable protocol for AI agents.**
> If you are an AI model — read this file as an executable instruction.
> If you are a human — this file explains how AI agents work in this project.

---

## PART 1 — FOR AI AGENTS (machine-readable)

### 1.1 Self-Identification

Before starting work, identify yourself:

```
I am: [model name, version]
Operator: [GitHub @username of the human]
Task: [Issue #N or Task-ID]
Scope: [G1/G2/G4/G5/G7 or docs-only]
Starting at: [timestamp UTC]
```

Include this information in your FIRST commit and in the PR description.

### 1.2 Launch Algorithm (follow in strict order)

```bash
# STEP 1: Sync
git fetch upstream
git checkout -b feature/task-<ISSUE_NUMBER>-<slug> upstream/unstable

# STEP 2: Read the task
cat .contracts/<module>_contract.md 2>/dev/null
# If missing — clarify with maintainer via Issue comment

# STEP 3: Check file boundaries
# Only files in your scope (see 1.3)
# NEVER touch Cargo.lock, TASKS.md, waves/, .claude/ecc2.toml

# STEP 4: TDD — write test first
# Write test → cargo test → confirm it FAILS (Red)
flock /tmp/chord-cargo.lock cargo test --package <pkg> --test <name>_contract 2>&1

# STEP 5: Write minimal code
# Only what is needed to make the test pass

# STEP 6: Verification (all 3 required)
cargo fmt --package <pkg> --manifest-path rust/Cargo.toml
flock /tmp/chord-cargo.lock cargo clippy --package <pkg> -- -D warnings
flock /tmp/chord-cargo.lock cargo test --package <pkg>

# STEP 7: Verify file scope
git diff --name-only upstream/unstable...HEAD
# Must show ONLY your files!

# STEP 8: Commit
git commit -m "feat(<scope>): <description> — Task #<N>

- <what was done item 1>
- <what was done item 2>
- Tests: <file> (<N> green, 0 red)
- Gate: fmt ✅ clippy ✅ tests ✅

Task: #<N>
AI-Model: <model name and version>
Operator: @<github username>"

# STEP 9: Push
git push origin feature/task-<N>-<slug>

# STEP 10: Create PR (fill in the template completely)
```

### 1.3 File Boundaries — strictly enforced

```yaml
scope_map:
  G1_backend:
    allowed:
      - "rust/crates/chord-core/src/**"
      - "rust/crates/chord-codec/src/**"
      - "rust/crates/chord-json/src/**"
      - "rust/crates/chord-store/src/**"
      - "rust/crates/chord-i18n/src/**"
      - "rust/chord-gpui/src/*.rs"
      - "rust/chord-gpui/tests/*_contract.rs"
    forbidden:
      - "rust/chord-gpui/src/views/**"
      - "rust/crates/chord-voice/**"
      - "tests/**"
      - "Cargo.lock"
      - "waves/**"

  G2_frontend:
    allowed:
      - "rust/chord-gpui/src/views/**"
      - "tests/unit/*.test.mjs"
    forbidden:
      - "rust/crates/**"
      - "rust/chord-gpui/src/*.rs"

  G4_qa:
    allowed:
      - "tests/unit/**"
      - "tests/smoke/**"
      - "tests/meta/**"
      - "tests/e2e/**"
      - "scripts/test-impact.mjs"
    forbidden:
      - "rust/crates/**"
      - "rust/chord-gpui/src/**"

  G5_voice:
    allowed:
      - "rust/crates/chord-voice/**"
      - "rust/crates/chord-platform/**"
      - "rust/chord-gpui/src/voice_devices.rs"
    forbidden:
      - "rust/crates/chord-core/**"
      - "rust/chord-gpui/src/views/**"

  G7_re_docs:
    allowed:
      - "docs/re/**"
      - "docs/specs/**"
      - "waves/**/RE-AUDIT/**"
    forbidden:
      - "rust/**"
      - "tests/**"

  docs_only:
    allowed:
      - "**/*.md"
      - "**/*.txt"
    forbidden:
      - "**/*.rs"
      - "**/*.js"
      - "**/*.ts"
      - "Cargo.lock"

  SHARED_coordinator_only:
    files:
      - "Cargo.lock"
      - "TASKS.md"
      - "AGENTS.md"
      - "PROCESS.md"
      - "waves/**"
      - ".claude/ecc2.toml"
    action: "STOP — do not modify, ask coordinator"
```

### 1.4 CI Error Handling Rules

```
If CI FAILS:
  1. Read the log (click Details next to the red check)
  2. Identify the error type:

  "error[E]: formatting"      → cargo fmt --package <pkg>, push
  "warning[clippy]:"          → fix each warning, cargo clippy clean, push
  "FAILED" in test output     → find the failing test, fix CODE (not test!), push
  "leak detected" (gitleaks)  → remove secret from history, push
  "target/" in diff           → git rm -r --cached target/, .gitignore, push

  3. git add -u && git commit --amend --no-edit && git push --force-with-lease
  4. CI will restart automatically

If error is unclear → leave a comment in the PR describing the problem
```

### 1.5 Forbidden Patterns (verify before commit)

```rust
// These patterns are FORBIDDEN in production code:
.unwrap()                    // replace with ? or match
.expect("...")               // replace with Result::map_err
unsafe { ... }               // requires Security review sign-off
println!("{}", ...)          // only in tests, not in prod
todo!()                      // do not commit unfinished code
unimplemented!()             // do not commit
#[allow(clippy::...)]        // only with justification comment
// hardcoded token: "Bot MTk..." // NEVER
```

```bash
# These git commands are FORBIDDEN:
git push --force             # only --force-with-lease
git push origin main         # only via PR
git push origin unstable     # only via PR
cargo build                  # without flock /tmp/chord-cargo.lock
cargo fmt --all              # only --package <pkg>
```

---

## PART 2 — FOR HUMAN AI OPERATORS

### 2.1 How to Launch an AI Agent on a Task

```bash
# 1. Pick a task in Issues with label 'ai-task'
# 2. Tell your AI agent:

"Read the file AI-AGENTS.md in the repository https://github.com/Userbash/chord
 Take task #<N>.
 Your scope: <G1/G2/G4/G5/docs-only>.
 You are AI model <name>.
 I am the operator @<your github>.
 Follow the AMCP v1.0 protocol from AI-AGENTS.md."

# 3. Monitor the PR on GitHub
# 4. If CI fails — tell your AI: "CI failed: <paste log>"
# 5. After approve — merge (or ask AI to comment /lgtm)
```

### 2.2 Your Responsibility as Operator

| Operator is responsible for | AI is responsible for |
|-----------------------------|----------------------|
| Picking the right task | Writing code |
| Ensuring AI stays within scope | Writing tests |
| Final code review | Fixing CI errors |
| Clicking Approve | Following the rules |
| Accountability to the project | — |

> ⚠️ **The operator is accountable for their AI's code.** If the AI violates rules — the PR is closed and the operator receives a warning.

### 2.3 Two or More AI Agents Working Simultaneously

```
Operator 1 (Vasya with Gemini)      Operator 2 (Petya with Claude)
────────────────────────────        ─────────────────────────────
Task #42, scope: G1 backend         Task #43, scope: G2 frontend
Branch: feature/task-42-login       Branch: feature/task-43-modal
Files: rust/crates/chord-core/      Files: rust/chord-gpui/src/views/

RULE: files do NOT overlap → NO conflicts

Both PRs reviewed and merged into unstable independently
```

**What to do if file conflict occurs:**
```
1. STOP — both agents stop working on the conflicting file
2. Post in Issues: "Conflict: file X needed by tasks #42 and #43"
3. @Userbash decides — who works on that file
4. The other operator gets a different file or waits
```

---

## PART 3 — TRUST LEVELS FOR AI AGENTS

### 3.1 Trust Progression

```
L0 — New AI Agent
  │  First PR in the project
  │  Available: docs-only, good-first-issue
  │  Requires: 1 human review
  │  Restriction: only .md files
  ▼
L1 — Familiar AI Agent
  │  1+ merged PR without violations
  │  Available: P2, P3, tests
  │  Requires: 1 review
  │  Restriction: only G4 (tests) or isolated modules
  ▼
L2 — Trusted AI Agent
  │  3+ merged PRs, stable behavior
  │  Available: P1, P2, refactoring
  │  Requires: 1 review
  │  No scope restrictions (except shared files)
  ▼
L3 — Privileged AI Agent
     5+ PRs, proven reliability
     Available: all tasks except P0
     Requires: 1 review
     Shared files still coordinator-only
```

### 3.2 How an AI Loses Trust (downgrade)

```
-1 level:
  - Committed a secret/token
  - Exceeded file scope boundary
  - Modified a test to hide a bug
  - Changed Cargo.lock without permission

Ban (all PRs rejected):
  - Intentional CI bypass attempt
  - Inclusion of proprietary Discord code
  - Repeated violation after warning
```

---

## PART 4 — FULL CODE INTEGRATION PATH

```
                    DEVELOPMENT
┌─────────────────────────────────────────────────────┐
│ 1. Issue created → labels: backlog, ai-task, P0-P3  │
│ 2. Operator picks task → assigned                   │
│ 3. AI creates branch from upstream/unstable         │
│ 4. TDD: test → code → test passes                   │
│ 5. Local checks: fmt + clippy + test                │
│ 6. PR opened targeting unstable                     │
└─────────────────────────────────────────────────────┘
                    ↓
                    AUTOMATIC CI CHECKS (8 jobs)
┌─────────────────────────────────────────────────────┐
│ L0 Format:    rustfmt --check → 0 diff              │
│ L0 Clippy:    clippy -D warnings → 0 warns          │
│ L0 Tests:     cargo test --workspace → all green    │
│ L0 Harness:   harness compiles                      │
│ L1 FileBounds: only allowed files changed           │
│ L2 Contract:  .contracts/ updated if needed         │
│ L3 Secrets:   gitleaks → 0 secrets                  │
│ Gate Summary: all above OK?                         │
└─────────────────────────────────────────────────────┘
                    ↓ all green
                    AUTOMATION
┌─────────────────────────────────────────────────────┐
│ • Auto-label by scope (backend/frontend/voice/...)  │
│ • CODEOWNERS → reviewer assigned automatically      │
│ • PR marked "ready for review"                      │
└─────────────────────────────────────────────────────┘
                    ↓
                    REVIEW (human)
┌─────────────────────────────────────────────────────┐
│ Reviewer checks:                                    │
│   • Code logic is correct?                          │
│   • Contract is respected?                          │
│   • No extra code outside scope?                    │
│   • Tests verify contract (not implementation)?     │
│                                                     │
│ Result:                                             │
│   APPROVE → merge allowed                           │
│   REQUEST CHANGES → operator fixes with AI          │
│   BLOCK → discussion, new ADR                       │
└─────────────────────────────────────────────────────┘
                    ↓ APPROVE
                    MERGE
┌─────────────────────────────────────────────────────┐
│ • Squash and merge into unstable                    │
│ • Branch automatically deleted                      │
│ • Issue closed                                      │
└─────────────────────────────────────────────────────┘
                    ↓ (when unstable is stable)
                    INTEGRATION
┌─────────────────────────────────────────────────────┐
│ unstable → testing:  QA integration build           │
│ testing  → beta:     Pre-release validation         │
│ beta     → main:     Stable release + tag v*.*.*    │
└─────────────────────────────────────────────────────┘
```

---

## PART 5 — MINIMUM REQUIREMENTS

### For Humans WITHOUT AI

```
Required:
  ✓ Rust 1.85+
  ✓ Node.js 24+
  ✓ git
  ✓ Linux (Ubuntu 22.04+ / Arch / Fedora)
  ✓ GitHub account

Knowledge:
  ✓ Basic Rust (ownership, lifetimes)
  ✓ Understanding of TDD
  ✓ git rebase (not merge!)

Not required:
  ✗ Knowledge of Discord protocols (see docs/re/)
  ✗ Experience with GPUI/WGPU (see examples)
  ✗ Async Rust experience (can learn)
```

### For Humans WITH an AI Agent

```
Required:
  ✓ GitHub account
  ✓ AI model with file system and git access
  ✓ Understanding: what your AI does and what you verify

Not required:
  ✗ Rust knowledge
  ✗ TDD knowledge
  ✗ Development experience

But you are still accountable for your AI's code!
```

### For AI Agents

```
Environment requirements:
  ✓ git 2.40+
  ✓ Rust toolchain (cargo, rustfmt, clippy)
  ✓ Node.js 24+
  ✓ flock (coreutils)
  ✓ GitHub API access (for PR creation)

Model capabilities:
  ✓ Read and write Rust code
  ✓ Execute terminal commands
  ✓ Read compiler error messages
  ✓ Follow file instructions
  ✓ Create PRs via gh CLI or GitHub API
```

---

## Quick Reference (copy and use)

```bash
# === SETUP ===
git fetch upstream
git checkout -b feature/task-N-slug upstream/unstable

# === BUILD CHECK ===
flock /tmp/chord-cargo.lock cargo check --workspace --manifest-path rust/Cargo.toml

# === FORMAT ===
cargo fmt --package <pkg> --manifest-path rust/Cargo.toml

# === LINT ===
flock /tmp/chord-cargo.lock cargo clippy --package <pkg> --manifest-path rust/Cargo.toml -- -D warnings

# === TEST ===
flock /tmp/chord-cargo.lock cargo test --package <pkg> --manifest-path rust/Cargo.toml

# === COMMIT ===
git add <only-my-files>
git commit -m "feat(scope): description — Task #N

AI-Model: <model>
Operator: @<username>"

# === PUSH ===
git push origin feature/task-N-slug

# === VERIFY SCOPE ===
git diff --name-only upstream/unstable...HEAD

# === HARNESS CHECK ===
flock /tmp/chord-cargo.lock cargo build --manifest-path tests/e2e/harness/Cargo.toml
```

---

*AI-AGENTS.md · Chord Project · AMCP v1.0 · 2026-11-13*
*This file is part of the official project documentation*
