# Contributing to Discord Lab & Discord GPUI

Thank you for your interest in contributing to **Discord Lab & Discord GPUI**! We welcome open-source contributions, bug reports, and architectural enhancements.

This document outlines our engineering standards, Clean-Room reverse engineering guidelines, Test-Driven Development (TDD) requirements, and pull request workflow.

---

## 1. Clean-Room Methodology (Critical)

This project strictly adheres to **Clean-Room Software Engineering** principles:
- **No Proprietary Code**: Never copy, decompile, or incorporate proprietary code, tokens, or intellectual property from Discord Inc. into this codebase.
- **Protocol Verification Only**: Research is limited to local, non-invasive protocol tracing (Frida/CDP) and public protocol specifications (RFC 9420 DAVE MLS, WebRTC, Opus).
- **Clean Commits**: PRs containing scraped user tokens, private data, or unvetted blobs will be closed immediately.

---

## 2. Test-Driven Development (Strict TDD)

Every feature or bugfix must follow our **Strict TDD** methodology:
1. **Red Phase**: Write the failing test first. The test must verify a behavioral contract against an independent oracle or specification, never against existing implementation stubs.
2. **Green Phase**: Write the minimal implementation required to make the test pass.
3. **Mutation Testing**: Ensure your tests kill **5/5 (100%)** injected mutations. Code changes without regression tests or with failing mutations will not be merged.

### Running Local Checks:
```bash
# Run architectural and documentation meta-tests
npm run test:meta

# Run targeted tests for modified files
node scripts/test-impact.mjs --run

# Run TypeScript typechecks
npm run typecheck

# Verify Rust crates compile cleanly
cargo check --manifest-path rust/Cargo.toml --workspace
```

---

## 3. Code Style & Architecture Guidelines

- **Rust (Edition 2024)**:
  - All crates enforce `#![forbid(unsafe_code)]` unless interacting with C FFI (e.g. `libenchant-2`).
  - No unhandled `.unwrap()` or `.expect()` in production code. Use `Result` and `Option` matching with comprehensive error types.
  - Functions should remain focused (< 50 lines). Source files follow an 800-line soft maintainability ceiling.
  - Formatting must be strictly clean with `rustfmt` (`cargo fmt --check`).
- **JavaScript / Node.js**:
  - Node.js 24+, native ECMAScript Modules (`import`/`export`).
  - Every module starts with `// @ts-check` and comprehensive JSDoc type annotations.
  - Zero console logging in production modules.
- **Immutability & Safety**:
  - Prefer immutable patterns. Never mutate shared state in-place without proper atomics or message passing channels.

---

## 4. Branching Model & Pull Requests

Our Git development flow follows this hierarchy:

```
feature/<task-id>-<slug> ──► unstable ──► testing ──► beta ──► main
```

1. **Create a branch** off `unstable`:
   ```bash
   git checkout unstable
   git checkout -b feature/task-<id>-<short-description>
   ```
2. **Commit Messages**: Follow [Conventional Commits](https://www.conventionalcommits.org/):
   ```
   <type>(<scope>): <short description>
   
   <detailed body explaining the change and verification>
   ```
   *Types:* `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`.
3. **Submit a Pull Request**:
   - Ensure all CI gates, meta-tests, and unit tests are passing.
   - Describe what changed, link corresponding issue/task IDs, and include test verification evidence.

---

## 5. Security & Responsible Disclosure

If you discover a security vulnerability or security-sensitive bug (e.g., token exposure, memory corruption, or cryptographic flaw in DAVE MLS), please report it responsibly by contacting the maintainers via private channels or opening a draft security advisory on GitHub rather than filing a public issue.

---

## 6. Community & Communication

- Be respectful and constructive.
- For design discussions or architectural RFCs, open an Issue with the `[RFC]` tag.
