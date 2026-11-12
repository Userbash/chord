## 🤖 AI Worker PR

**Task:** <!-- P0-X.X | H-FIX-X | link to Issue -->
**Model:** <!-- Gemini-2.5-Pro / Claude-3.7 / GPT-4o / DeepSeek-V3 -->
**Worker:** <!-- G1 Backend / G2 Frontend / G4 QA / G5 Voice / G7 RE -->
**Branch:** `feature/task-<ID>-<slug>` → `unstable`
**AI-Agent:** <!-- true / false -->
**Operator:** <!-- @github-username -->

---

### What was done

- 
- 

### Tests

| Contract file | Tests | Status |
|---------------|-------|--------|
| `tests/.../foo_contract.rs` | N | green |

```
# Paste cargo test output:
test result: ok. N passed; 0 failed
```

### Changed files (my scope only)

- `src/...` — 

### Files NOT touched

- [ ] `Cargo.lock` — not changed
- [ ] `TASKS.md` — not changed
- [ ] `waves/` — not touched
- [ ] Other scope files — not touched

---

### Gate checklist (before requesting review)

- [ ] `cargo fmt --check` → 0 diff
- [ ] `cargo clippy -- -D warnings` → 0 warnings
- [ ] `cargo test -p <pkg> --test <name>_contract` → green
- [ ] No secrets/tokens in code
- [ ] `git diff --name-only` shows only my files
- [ ] CI all checks green

---

### Breaking changes

- [ ] No breaking changes in public API
- [ ] Breaking changes present: <!-- describe + update .contracts/ -->

---

*Protocol: AMCP v1.0 | Auto-checks: AI Collab Gate workflow*
