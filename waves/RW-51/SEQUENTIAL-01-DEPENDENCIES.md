# Sequential step 1: locked Rust dependency cache restored

Status: completed for the Linux x86_64 GUI test phase. The dependency-intake parent `LIB-GOV-02`, full worker gate and native ARM release remain open.

The fresh offline resolution reproduced the original error: `jiff 0.2.44` was locked but the local index/cache only offered `0.2.38`. An official Cargo `fetch --locked --target x86_64-unknown-linux-gnu` downloaded the existing locked versions. No package version was selected anew and no manifest or lockfile was changed. Cargo lock SHA256 remains `9e1e91728438890cc43f0a73ab66b6f14127016b7b0c201e8de28eae464556d0`.

Offline metadata resolution then succeeded for 360 packages. The actual `cargo test --locked --offline --manifest-path rust/gui-spike/Cargo.toml` command passed **748 tests, 0 failures, 0 ignored** across 51 result summaries. This includes the zero-test documentation summary; it is not a native GUI window or ARM runtime claim.

A full worker gate rerun confirmed the previously failing `spike-rust` phase is now green. Typecheck, UI typecheck, metadata and workspace Rust phases also pass. The gate remains **failed** because 39 existing JS unit tests fail. Smoke still reports its explicit environment skips, and workspace live tests remain separately ignored.

The next sequential step is the 39 unit failures. No further task is marked complete by this environment repair. Native macOS, Windows and ARM dependency/build/runtime qualification require their own evidence.

Exact commands, exits and hashes: [SEQUENTIAL-01-DEPENDENCIES.json](SEQUENTIAL-01-DEPENDENCIES.json). Logs are losslessly archived in `verification/`; raw originals remain in `/root/projects/chord-modernization-verification/rust-dependencies`.
