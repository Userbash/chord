# Cargo Flock — Protection Against Parallel Builds

> FIX-22: Mandatory rule for all workers running `cargo` commands.
> Violation = OOM kill (25 GB RAM limit across 5 parallel workers).

## Rule

**ALWAYS** wrap `cargo` commands with `flock`:

```bash
# Template (MANDATORY):
flock /tmp/discord-lab-cargo.lock cargo <command> ...

# Examples:
flock /tmp/discord-lab-cargo.lock cargo build --manifest-path rust/Cargo.toml -p chord-core
flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p chord-voice --test voice_contract
flock /tmp/discord-lab-cargo.lock cargo check --manifest-path rust/chord-gpui/Cargo.toml
```

## Why

- `CARGO_TARGET_DIR = /home/user/orca/project/rust/target` — shared across all workers
- Parallel `cargo build` on the same `target-dir` → corrupted incremental cache → OOM
- AMD Ryzen 7 7800X3D: 1 cargo build × 8 threads already uses ~4–6 GB RAM
- With 5 workers and no flock → 20–30 GB → OOM kill

## Exceptions

- `cargo check` without codegen — may skip flock (quick type check only)
- `cargo fmt` — always skip flock (formatting only)
- `cargo clippy --no-deps` — skip flock (static analysis without linking)

## RAM Monitor Before Starting `cargo`

```bash
CURRENT_RSS=$(ps -eo rss | awk '{s+=$1} END {printf "%.1f", s/1024/1024}')
echo "Current RSS: ${CURRENT_RSS} GiB"
# If > 20 GiB — wait for a free slot before launching cargo
```
