# Packaging prerequisites for the next sequential slice

The latest full gate still has 31 release/package/build prerequisite failures plus three Krisp cases. The genuine x86_64 binary exists outside immutable `rust/dist/`; old release checksums and historical inputs remain unchanged. Official Chord release discovery found no compiled assets to restore from GitHub.

The x86_64 builder `scripts/build-appimage.sh` currently constructs a tar.gz payload (lines312–314), compiles a custom C loader, writes `AI\x02` into its ELF header (line556), and appends a custom `AI2PAYLOAD` trailer (line567). This does not match the [official Type 2 architecture](https://docs.appimage.org/reference/architecture.html), which uses SquashFS with a runtime. A magic-byte check alone cannot certify the package format. The old builder was inspected, not used to manufacture a purported valid package.

Next ordered work: define an independent real-format failing test; bind tests and packages to a source/lock/artifact manifest in a dedicated output directory; delegate the container format to reviewed pinned official tools; verify extraction, permissions, embedded payload hashes and real startup; then run the full gate. Preserve existing negative assertions and immutable historical artifacts.

The official tool/runtime discovery records both x86_64 and ARM64 candidates, asset IDs and digests. The returned `continuous` tags are mutable; nothing was downloaded or adopted. Tool pins, CPU/ABI compatibility and native ARM evidence require explicit qualification before production adoption. The ARM builder already has a separate official-tool path; no ARM success is inferred from an x86 tool.
