# 🤝 Contributing to Chord

> **This guide is for both humans and AI agents.**
> See the [AI Agents section](#-for-ai-agents) for machine-readable rules.

<p align="center">
  <a href="#-quick-start">Quick Start</a> ·
  <a href="#-trust-levels">Trust Levels</a> ·
  <a href="#-for-ai-agents">For AI Agents</a> ·
  <a href="#-git-protocol">Git Protocol</a> ·
  <a href="#-ci-checks">CI Checks</a> ·
  <a href="#-review--merge">Review & Merge</a>
</p>

---

## 🚀 Quick Start

### Step 1 — Fork and Clone

```bash
# 1. Click Fork on GitHub (top right corner)
# 2. Clone your fork
git clone https://github.com/YOUR_USERNAME/chord.git
cd chord

# 3. Add upstream remote
git remote add upstream https://github.com/Userbash/chord.git
```

### Step 2 — Set Up Environment

```bash
# Rust 1.85+ required
rustup default stable
rustup component add rustfmt clippy

# Node.js 24+ required
node --version  # >= 24.0.0

# System libraries (Linux)
sudo apt install libasound2-dev libpipewire-0.3-dev libsecret-1-dev libenchant-2-dev

# Install npm dependencies
npm ci
```

### Step 3 — Verify Everything Works

```bash
# Rust workspace compiles
cd rust && cargo check --workspace && cd ..

# Tests are green
cargo test --workspace --manifest-path rust/Cargo.toml

# Node.js tests
npm run test:meta
```

### Step 4 — Find a Task

1. Open [Issues](https://github.com/Userbash/chord/issues) → filter by `backlog`
2. Pick a task matching your trust level (see [Trust Levels](#-trust-levels))
3. Comment: **"I'm taking this task"**
4. Wait for assignment from a maintainer

### Step 5 — Work and Submit PR

```bash
# Sync with upstream
git fetch upstream
git checkout -b feature/task-<ID>-<short-slug> upstream/unstable

# ... write tests first, then code ...

# Local checks BEFORE push
cargo fmt --check --manifest-path rust/Cargo.toml
cargo clippy --workspace --manifest-path rust/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path rust/Cargo.toml

# Push and open PR
git push origin feature/task-<ID>-<short-slug>
# Open PR on GitHub → target branch: unstable
```

---

## 🏅 Trust Levels

> Your level determines which tasks are available and how many reviewers are required.

| Level | Who | Available Tasks | Reviewers Required |
|-------|-----|----------------|--------------------|
| **L0 — Newcomer** | First PR in the project | `docs-only`, `good-first-issue` | 1 |
| **L1 — Contributor** | 1+ merged PR | `P2`, `P3`, tests, QA | 1 |
| **L2 — Trusted** | 5+ merged PRs, quality reviews | `P1`, refactoring | 1 |
| **L3 — Maintainer** | Invited by owner | `P0`, shared files, architecture | 2 |
| **🤖 AI Agent (External)** | Fork + PR | `P2`, `P3`, `docs-only` | 1 (must be human) |
| **🤖 AI Agent (Trusted)** | 3+ merged PRs | `P1`, `P2` | 1 |

### How to Level Up

```
L0 → L1:  One merged PR with green tests
L1 → L2:  5 merged PRs + 3 quality reviews of others' code
L2 → L3:  Direct invite from @Userbash (do not request manually)
```

---

## 🤖 For AI Agents

> This section is written specifically for AI models.
> Read it as an executable instruction.

### AI Agent Identification in PR

Include in every PR description:

```markdown
**AI-Agent:** true
**Model:** <Gemini-2.5-Pro / Claude-3.7-Sonnet / GPT-4o / DeepSeek-V3 / ...>
**Operator:** <@github-username of the human who runs you>
**Task-ID:** <Issue number or P0-X.X>
```

### Mandatory Rules for AI Agents

```
DO:
  1. Read this entire file before starting
  2. Create branch from upstream/unstable (not from main!)
  3. Write test FIRST — before code (TDD: Red → Green)
  4. Touch ONLY files in your scope (see File Boundaries)
  5. Run cargo fmt + clippy + test locally before push
  6. Include model name and operator in PR description
  7. Use flock before cargo: flock /tmp/chord-cargo.lock cargo <cmd>
  8. Read CI errors and fix them — do not ignore

NEVER:
  1. Do not touch Cargo.lock (only via separate coordinator PR)
  2. Do not commit target/, node_modules/, .env, *.key, *.pem
  3. Do not git push --force (only --force-with-lease)
  4. Do not cargo fmt --all (only --package <pkg>)
  5. Do not write unsafe{} without Security review
  6. Do not guess public APIs — read .contracts/ or ask
  7. Do not modify tests to make them pass under a bug
  8. Do not open PRs directly to main/beta/testing
```

### File Boundaries (scope)

An AI agent works ONLY within its scope. Before starting — check the issue label:

| Issue Label | Your Scope | Forbidden |
|-------------|-----------|-----------|
| `scope:backend` | `rust/crates/chord-core/`, `rust/crates/chord-codec/`, `rust/chord-gpui/src/*.rs` | Everything else |
| `scope:frontend` | `rust/chord-gpui/src/views/` | Backend, Voice |
| `scope:voice` | `rust/crates/chord-voice/`, `rust/crates/chord-platform/` | Backend, Frontend |
| `scope:tests` | `tests/unit/`, `tests/smoke/`, `tests/meta/` | Production code |
| `scope:harness` | `tests/e2e/harness/` | Production code |
| `scope:re` | `docs/re/`, `docs/specs/` | Production code |
| `docs-only` | `**/*.md` | Any code |

### Read the Contract Before Coding

```bash
# Read the module contract before writing any code:
cat .contracts/<module>_contract.md 2>/dev/null || echo "No contract — clarify with maintainer"

# If no contract exists — propose one in your PR description:
# "Proposed contract: input X → output Y → HTTP Z"
```

### AI Commit Message Template

```
feat(auth): implement LoginService — Task #42

- POST /api/v9/auth/login via ApiClient
- TokenStore saves to SecretService D-Bus keyring
- Tests: tests/unit/auth_contract.rs (5 green, 0 red)
- Gate: fmt ✅ clippy ✅ tests ✅

Task: #42
AI-Model: Gemini-2.5-Pro
Operator: @vasya
```

### AI Agent Workflow (step by step)

```
START
  │
  ▼
1. git fetch upstream && git checkout -b feature/task-<N>-<slug> upstream/unstable
  │
  ▼
2. Read Issue #N — understand task, scope, contract
  │
  ▼
3. Write TEST (Red) → cargo test → confirm it FAILS
  │
  ▼
4. Write minimal CODE → cargo test → confirm it PASSES
  │
  ▼
5. cargo fmt --package <pkg>
   flock /tmp/chord-cargo.lock cargo clippy --package <pkg> -- -D warnings
   flock /tmp/chord-cargo.lock cargo test --package <pkg>
  │
  ▼
6. git diff --name-only → confirm only my files changed
  │
  ▼
7. git commit -m "feat(...): ... — Task #N\n\nAI-Model: ...\nOperator: ..."
  │
  ▼
8. git push origin feature/task-<N>-<slug>
  │
  ▼
9. Open PR on GitHub → target: unstable
   Fill in the PR template completely (all fields required)
  │
  ▼
10. Wait for CI → if ❌: read log, fix, push
    Wait for Review → if REQUEST CHANGES: fix
    After APPROVE → merge (automatic or by maintainer)
  │
  ▼
END
```

---

## 📋 Git Protocol

### Branch Hierarchy

```
main          ← stable releases only (tags v*.*.*)
  └─ beta     ← pre-release validation
       └─ testing  ← QA integration
            └─ unstable  ← merged PRs (open your PR here!)
                 └─ feature/task-<ID>-<slug>  ← your branch
                 └─ bugfix/task-<ID>-<slug>
                 └─ docs/task-<ID>-<slug>
```

> **Important:** Always create your branch from `upstream/unstable`, not from `main`!

### Branch Naming Convention

```bash
feature/task-42-login-service      # new feature
bugfix/task-43-token-store-crash   # bug fix
docs/task-44-update-readme         # documentation only
test/task-45-auth-contract         # tests only
refactor/task-46-split-state       # refactoring
chore/task-47-update-deps          # dependency updates
```

### Syncing with Upstream

```bash
git fetch upstream
git rebase upstream/unstable
# If conflicts — resolve manually, then:
git add <resolved-files>
git rebase --continue
git push --force-with-lease origin feature/my-branch
```

> ⚠️ Use **rebase**, not merge. This keeps a clean linear history.

---

## 🔍 CI Checks (Gate)

When a PR is opened, **8 parallel checks** run automatically:

```
L0 · Format ────── cargo fmt --check
                   Zero diff = OK

L0 · Clippy ────── cargo clippy -- -D warnings
                   Zero warnings = OK

L0 · Tests ─────── cargo test --workspace
                   All tests green = OK

L0 · Harness ───── cargo build (tests/e2e/harness)
                   Compiles = OK

L1 · FileBounds ── target/ not committed
                   Shared files flagged for review

L2 · Contract ──── Production code changed?
                   Warning if .contracts/ not updated

L3 · Secrets ───── gitleaks scan
                   Zero secrets = OK

✅ Gate Summary ── All above OK = PR ready for review
                   Any FAIL = merge blocked
```

### If CI Fails — What to Do

```bash
# 1. Click Details next to the failing check
# 2. Read the error log

# L0 Format failed:
cargo fmt --manifest-path rust/Cargo.toml
git add -u && git commit --amend --no-edit && git push --force-with-lease

# L0 Clippy failed:
cargo clippy --workspace --manifest-path rust/Cargo.toml -- -D warnings
# Fix each warning, then commit + push

# L0 Tests failed:
cargo test --workspace --manifest-path rust/Cargo.toml 2>&1 | grep "FAILED"
# Find the failing test, fix the CODE (not the test!), commit + push

# L3 Secrets failed:
# IMMEDIATELY: remove the secret from git history
git rebase -i HEAD~N  # find the commit with the secret
# Or use: git filter-repo / BFG Repo Cleaner
```

---

## 👀 Review & Merge

### Who Reviews

```
G1 backend files  → @Userbash (automatically via CODEOWNERS)
G2 frontend files → @Userbash
G5 voice files    → @Userbash
Shared files      → @Userbash (only him)
Docs-only         → Any L2+, or auto-merge when CI is green
```

> **Rule:** A PR author cannot review their own PR.  
> **For AI agents:** An AI operator cannot review their own AI's PR.

### Review Statuses

| Status | Meaning | Action |
|--------|---------|--------|
| ✅ **APPROVE** | Looks good | Merge allowed |
| 💬 **REQUEST CHANGES** | Fixes needed | Fix and push |
| ❌ **BLOCK** | Critical problem | Discuss in comments |

### After APPROVE

```bash
# Maintainer clicks "Squash and merge"
# GitHub automatically:
#   1. Squashes all commits into one
#   2. Merges into unstable
#   3. Deletes your branch
#   4. Closes the linked Issue
```

---

## 📐 Code Standards

### Rust

```rust
// Good
pub async fn submit_login(email: &str, password: &str) -> Result<AuthToken, LoginError> {
    validate_email(email)?;
    let response = api_client.post("/auth/login", &payload).await?;
    AuthToken::from_response(response)
}

// Bad — forbidden patterns
pub fn login(e: &str, p: &str) -> AuthToken {
    let r = client.post(e, p).unwrap();  // .unwrap() is forbidden!
    r.token                               // no error handling
}
```

**Requirements:**
- `#![forbid(unsafe_code)]` in all crates (except C FFI)
- No `.unwrap()` / `.expect()` in production code
- Functions < 50 lines
- Files < 800 lines (soft limit)
- `cargo fmt` before every commit
- Zero clippy warnings

### Tests — TDD Required

```rust
// 1. Write test FIRST (Red phase)
#[tokio::test]
async fn test_login_returns_token_on_success() {
    // Arrange
    let mock_server = MockGateway::start().await;
    mock_server.expect_post("/auth/login").respond_with(200, TOKEN_FIXTURE);

    // Act
    let result = LoginService::submit("user@test.com", "pass").await;

    // Assert — verify CONTRACT, not implementation
    assert!(matches!(result, Ok(AuthToken { .. })));
}

// 2. THEN write minimal code (Green phase)
```

**Minimum thresholds (CI blocks if below):**
```
Line coverage:     >= 90%
Branch coverage:   >= 90%
Function coverage: >= 80%
Mutation score:    5/5 (100%)
Flaky tests:       0%
```

---

## 🔐 Security

### Clean-Room Methodology (Mandatory)

```
ALLOWED:
  - Analyze public RFCs and specifications
  - Use network traces (Wireshark, mitmproxy)
  - Study open API documentation
  - Reference open-source implementations of other clients

NEVER:
  - Copy code from proprietary clients
  - Include user Discord tokens
  - Use private user data
  - Include decompiled proprietary software output
```

### If You Find a Vulnerability

**Do NOT create a public Issue!**

Report privately via [Security Advisory](https://github.com/Userbash/chord/security/advisories/new)

---

## 🌐 Internationalization (i18n) Contributions

Localization is powered by `rust/crates/chord-i18n/` (36 locales, CLDR pluralization, `xxh64` key hashing per [`docs/re/i18n-deep.md`](docs/re/i18n-deep.md)) and `gpui-kit/crates/component/locales/ui.yml` for shared UI widgets.

When adding or updating translations:
1. Keep key names and interpolation placeholders (`{count}`, `{user}`, etc.) identical across locales.
2. Verify plural rules in `rust/crates/chord-i18n/src/plural.rs` and run `cargo test -p chord-i18n --manifest-path rust/Cargo.toml`.
3. Never commit machine-translated strings with broken formatting tokens or truncated placeholders.

---

## 📦 Legacy Artifact Naming Policy

While the repository and Rust workspace crates are named **Chord** (`chord-gpui`, `chord-core`, etc.), the Beta 1 release pipeline (`.github/workflows/release-beta.yml`, `packaging/snap/snapcraft.yaml`, `rust/dist/SHA256SUMS`, and `tests/unit/beta-release-package-958.test.mjs`) still references the legacy `discord-gpui` package/binary names for backwards compatibility.

- **Do not** rename `discord-gpui-*` artifact globs or `packaging/rpm/discord-gpui.spec` inside unrelated feature/docs PRs.
- Unifying binary and package names under `chord` requires a dedicated migration PR updating `release-beta.yml`, `packaging/`, `rust/dist/SHA256SUMS`, and packaging contract tests together.

---

## ❓ FAQ

**Q: Can multiple AI agents work on the same task simultaneously?**  
A: No. One task = one worker = one PR. Parallel work is only allowed on different tasks with non-overlapping file scopes.

**Q: My AI changed Cargo.lock — is that a problem?**  
A: Yes. Cargo.lock is a shared file. Revert: `git checkout upstream/unstable -- Cargo.lock`

**Q: CI failed on secrets but I didn't add any**  
A: gitleaks may have found a test fixture or placeholder. Check: `gitleaks detect --source . --verbose`

**Q: Can I open a PR directly to main?**  
A: No. Direct push to main/beta/testing/unstable is blocked. Only via PR to unstable.

**Q: Does an AI agent need a human operator?**  
A: Yes. Every AI PR must have `Operator: @username`. The operator is responsible for the PR.

**Q: How long does review take?**  
A: Usually 24–72 hours. If longer — add a comment: `@Userbash ping review`.

---

## 📞 Contact

- **GitHub Issues:** https://github.com/Userbash/chord/issues
- **Discussions:** https://github.com/Userbash/chord/discussions
- **Security:** GitHub Security Advisories (private)

---

*Chord CONTRIBUTING.md · AMCP v1.0 · Updated 2026-11-13*
