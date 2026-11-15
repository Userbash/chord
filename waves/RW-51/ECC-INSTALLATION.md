# Authentic ECC installation and execution

Installed on 2026-11-15 in Ubuntu-24.04 WSL under the operator's instruction to find the project on GitHub and install it.

## Provenance

- Upstream: [affaan-m/ECC, `ecc2/` at v2.2.3](https://github.com/affaan-m/ECC/tree/v2.2.3/ecc2), MIT.
- Pinned source: `c05b2d6614f62f6db0047669aa4eefb223d478f9` in `/root/tools/ecc-v2.2.3-c05b2d66`.
- Actual native Rust executable: `/root/.local/bin/ecc`, reporting `ecc 0.1.0`. This is the upstream ECC2 alpha control plane, not the separate Node skills installer or an emulated Orca command.
- Build: upstream-pinned Rust 1.96.0, `cargo build --locked --release`, four jobs, global Cargo lock, isolated `/root/tools/ecc-build-c05b2d66` target. The upstream build emitted 24 warnings; the binary built successfully.
- Executable SHA256: `137ec2cdca23ed28639eed50886f2393c63d647658f7dc7108100ed4fe9b0296`.

The installation report contains the exact full digest, Cargo lock digest, toolchain and installation time. Source/build caches remain outside the application repository.

## Project entrypoint

The old `scripts/ecc` symlink targeted another machine's `/home/sanya/.../ecc`. The new small POSIX launcher executes a genuine backend chosen by explicit `ECC_BIN`, then PATH, then the standard user-local executable. It does not implement ECC semantics. Literal argument/exit/PID propagation, recursion prevention and missing-environment failures have eight process tests and five detected behavioral mutations.

## Real control-plane state

Runtime directory: `/root/projects/chord-modernization-ecc-runtime`.

ECC supports direct sessions and a persisted remote request queue. It does not require the old Orca desktop bridge for these launches. The closest runtime `ecc2.toml` selects supported harness runners and a common SQLite store; no fabricated Orca run/dispatch IDs are assigned to native ECC sessions.

Use the installed CLI from this runtime directory:

```sh
/root/.local/bin/ecc sessions
/root/.local/bin/ecc status 8aa61e7a
/root/.local/bin/ecc status 051d71c3
/root/.local/bin/ecc status 2dc1d8ea
```

The current session/task associations and observed heartbeat/outcome are recorded in `DISPATCH-RECEIPTS.json`. CLI state is the live source of truth; static board exports show their stated observation time.

Do not enqueue all dependency-blocked cards as executable pending requests and start the daemon. In this alpha, the daemon processes pending remote requests independently of the unread-handoff auto-dispatch setting. The complete DAG remains on Kanban; only admitted tasks receive executable sessions.

## Harness and operator decision

Google Gemini was unavailable for this configured account/location, and the repository model alias returned 404. The operator explicitly authorized Codex as an exception to the internal model policy; see `OPERATOR-OVERRIDE.md` and sanitized provider records.

The actual first-batch harness uses the official app-bundled **Linux ELF** Codex CLI, version `0.169.0-alpha.11.1`, existing ChatGPT authentication through supported `CODEX_HOME`, actual model `gpt-6.1-sol`, explicit medium effort, read-only sandbox and ephemeral structured output. Credentials were not copied into this repository or reports. Unrelated MCP connectors are disabled only for these source-only sessions.

Initial native Windows transport attempts encountered UNC output-path guards and interop proxy lifetime problems. Those attempts are retained as failed/stopped configuration history. The supported Linux transport preserves the sandbox and normal Linux source paths; real model output and completed source-inspection commands are required before counting a worker launch.

## First admitted phase

Three isolated clean source worktrees at `2b7cfb52`:

| Card | Purpose | ECC session |
| --- | --- | --- |
| LIB-GOV-01 | Source-grounded feature and evidence-gap ledger | `8aa61e7a` |
| LIB-GOV-02 | Actual dependency/target intake audit | `051d71c3` |
| LIB-RSA-01 | Independent RSA remote-auth contract specification | `2dc1d8ea` |

This first phase is read-only analysis. The coordinator persists their structured results. It does not implement cryptography, install application dependencies, run a full parity gate or mark application migrations accepted. Subsequent implementation requires the exact card's readiness, leases, independent RED contracts and review.

ARM follow-up: session `af4a762f` completed a fourth, bounded read-only audit for `LIB-ARM-01` using the same genuine Linux Codex route and unchanged source baseline. Its seven source findings are recorded separately; it does not claim an ARM build or native runtime result.
