# Token Efficiency — Token Conservation Rules for Workers

> OPT-6: Ban `cat` on large files + consolidate tool calls.
> Violation = P1 block at code review.

## Forbidden Patterns (FORBIDDEN)

```bash
# FORBIDDEN — loads the entire file into context
cat rust/chord-gpui/src/state/mod.rs           # 2500+ lines = ~40K tokens
cat tests/e2e/harness/trace_loader.rs
grep -rn "fn submit_login" rust/              # without | head -20 = noise

# FORBIDDEN — repeated calls to the same tool
run_command("grep -n 'fn foo' file.rs")        # first call
run_command("sed -n '100,150p' file.rs")       # second call (can be merged)
```

## Required Patterns (REQUIRED)

### 1. Reading a File — Targeted Line Range Only
```bash
# STEP 1: find the line number
grep -n "fn submit_login\|fn connect_voice\|fn start_screen" rust/chord-gpui/src/state/mod.rs

# STEP 2: read only the needed range
sed -n '1380,1450p' rust/chord-gpui/src/state/mod.rs
# OR: read_file(StartLine=1380, EndLine=1450)
```

### 2. Grep Always with Output Limit
```bash
grep -rn "pattern" rust/ | head -20
grep -n "fn " file.rs | head -30
```

### 3. Merge Independent Commands into One Call
```bash
# CORRECT — single run_command call
grep -n "fn submit_login" state/mod.rs && grep -n "fn connect_voice" state/mod.rs

# OR use parallel tool_calls when independent
```

### 4. Use `wc -l` Before `cat`
```bash
wc -l file.rs
# If > 100 lines — grep first, then sed -n 'X,Yp'
# If < 100 lines — read_file in full is fine
```

### 5. Harness Smoke — Always Use Summary Mode
```bash
# CORRECT — one line, no JSON parsing needed
./chord-gpui --smoke=summary
# → SMOKE ok backend=headless guilds=5 ch=23 sqlite=true fps=true tick=420us vmrss=38.2MB

# AVOID — full JSON for AI workers (CI only)
./chord-gpui --headless-smoke
```

### 6. Codebase Search — Use `codebase_search` / `understand_search`
```
# Instead of grep -rn across the whole project
codebase_search("submit_login")          # semantic search
understand_search("AppState login")     # architectural context
```

## File Size Thresholds

| Lines | Action |
|-------|--------|
| < 100 | `read_file` in full ✅ |
| 100–500 | `grep -n` → `sed -n 'X,Yp'` ✅ |
| > 500 | Targeted ranges only, `grep` first ✅ |
| > 2000 | Request Team Lead approval before reading ⚠️ |

## Call Batching

Tools can be called in parallel when there are no dependencies:
```
# In one turn — parallel:
read_file("state/mod.rs", StartLine=1380, EndLine=1430)
read_file("dispatch/mod.rs", StartLine=1, EndLine=50)
run_command("cargo check -p chord-gpui 2>&1 | head -20")
```

Do not chain N calls where 1–2 would suffice.
