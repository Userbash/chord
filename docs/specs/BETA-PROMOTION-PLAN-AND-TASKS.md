# Codebase Promotion Plan to `beta` Branch & Task Charter (Wave RW-45)

> **Document Status:** `Approved Planning Specification & Task Charter (v1.0)`  
> **Project:** `discord-lab` (`discord-gpui` native client + Core Rust)  
> **Objective:** Full preparation, testing, verification, and regulated promotion of the project to the `beta` branch  
> **Orchestrator:** ECC 2.0 Control Plane (`pm_review`)  
> **Date:** November 11, 2026  

---

## 1. Accompanying Brief and Instructions for Promotion to `beta`

### 1.1. Purpose and Context
Promoting the project to the `beta` branch is a milestone transitioning the project from active core development and fuzzing to pre-release stabilization for real users across various Linux distributions.

The `beta` branch strictly accepts only code that has passed:
1. **100% merge of all worker branches into `unstable`** following dual review (PM + Tech Reviewer, 5/5 mutations killed).
2. **Centralized build manifest `BUILD-005` in the `testing` branch**.
3. **Full end-to-end gate run `T2/T3`** (`scripts/ci.sh` and `scripts/gate.mjs --scope release`).
4. **Security audit, secret leakage checks, and SBOM compliance**.
5. **Testing under 4 Linux Desktop compositors** (GNOME, KDE, Hyprland, X11).
6. **Confirmation of the Rollback Plan** and backward compatibility of SQLite WAL storage.

### 1.2. Base Rules and Operational Constraints
* **Strict Branch Hierarchy:** Direct commits to `beta` and `main` are strictly prohibited. Flow: `unstable -> testing (BUILD-005) -> beta`.
* **Zero P0/P1 Blockers:** No component with known P0 or P1 defects may be promoted to `beta`.
* **Resource Budget:** Test runs must strictly remain within `WORKER_RAM_LIMIT <= 25 GB`.
* **Integrity Guarantee:** Tests validate behavior against independent oracles; tests are never modified to fit code.

---

## 2. Step-by-Step Codebase Promotion Algorithm

```text
 ┌────────────────────────────────────────────────────────────────────────┐
 │ STAGE 1: Finalization and merge into unstable                         │
 │ • Completion of all Wave RW-44 workers (128/128 tasks done)            │
 │ • Dual review (ACCEPT.md + REVIEW.md), 5/5 mutations killed            │
 │ • Merge of all feature branches into unstable, cleanup of temp branches│
 └───────────────────────────────────┬────────────────────────────────────┘
                                     │
 ┌───────────────────────────────────▼────────────────────────────────────┐
 │ STAGE 2: BUILD-005 manifest generation and promotion to testing        │
 │ • TASK-951: Build manifest BUILD-005 (commit list, hashes, tasks)      │
 │ • Create testing branch from unstable, tag build-005-candidate         │
 └───────────────────────────────────┬────────────────────────────────────┘
                                     │
 ┌───────────────────────────────────▼────────────────────────────────────┐
 │ STAGE 3: Comprehensive QA & Security verification in testing           │
 │ • TASK-952: Full gate scripts/ci.sh + scripts/gate.mjs --scope release │
 │ • TASK-953: Verification of ldd, glibc 2.35+, llvmpipe, base packages  │
 │ • TASK-954: 4-compositor matrix: GNOME, KDE, Hyprland, X11             │
 │ • TASK-955: 4-hour memory stress test (Soak Test, VmRSS < 60 MB)       │
 │ • TASK-956: Secret audit, token masking, reproducible SBOM generation  │
 └───────────────────────────────────┬────────────────────────────────────┘
                                     │
 ┌───────────────────────────────────▼────────────────────────────────────┐
 │ STAGE 4: Official promotion to beta and artifact release               │
 │ • TASK-957: PR testing -> beta, verification of Rollback procedure     │
 │ • TASK-958: Release target archive build, SHA256SUMS, BETA-GUIDE.md    │
 │ • Tag v1.0.0-beta.1 publication                                        │
 └────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Granular Task Decomposition (Tasks -> Subtasks -> Micro-steps)

---

### TASK-951: Centralized Build Manifest `BUILD-005` Generation and `unstable -> testing` Merge Preparation
* **Department:** `devops`
* **Priority:** `P0`
* **Summary:** Consolidate all finished Wave RW-44 tasks in branch `unstable`, verify working tree cleanliness, generate machine-readable manifest `BUILD-005.json`, and promote state to integration branch `testing`.
* **Subtasks and Micro-steps:**
  * **951.1: Verification and merge of worker branches into `unstable`**
    * *Step 1.1:* Execute `git status` and ensure no uncommitted artifacts remain outside `.gitignore`.
    * *Step 1.2:* Check status of all 128 tasks on the board — all tasks must be in `done` state.
    * *Step 1.3:* Merge worker branches into `unstable` with `--no-ff` flag to preserve review history.
  * **951.2: Build manifest `BUILD-005.json` generation**
    * *Step 2.1:* Aggregate list of all 128 completed tasks with IDs, titles, departments, and closure dates.
    * *Step 2.2:* Record commit SHA of `unstable` branch and build timestamp.
    * *Step 2.3:* Write file `waves/BUILD-005.json` and validate its JSON schema.
  * **951.3: Promotion to `testing` branch and baseline tagging**
    * *Step 3.1:* Switch to branch `testing` (`git checkout testing`).
    * *Step 3.2:* Perform merge `unstable -> testing`.
    * *Step 3.3:* Set lightweight Git tag `build-005-candidate`.
* **Definition of Done (DoD):**
  1. Branch `testing` is synchronized with `unstable` with zero merge conflicts.
  2. File `waves/BUILD-005.json` contains full registry of all 128 tasks.
  3. Tag `build-005-candidate` points to HEAD of `testing`.

---

### TASK-952: End-to-End Integration Gate T2/T3 in `testing` Branch (`scripts/ci.sh` & `scripts/gate.mjs`)
* **Department:** `g4_qa`
* **Priority:** `P0`
* **Summary:** Full unconditional execution of all pipeline verification levels in branch `testing` without skips (`failOnSkips: true`).
* **Subtasks and Micro-steps:**
  * **952.1: Local baseline pipeline (`scripts/ci.sh`)**
    * *Step 1.1:* Run `cargo fmt --all -- --check` across workspace (0 discrepancies).
    * *Step 1.2:* Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` (0 warnings).
    * *Step 1.3:* Run `bash scripts/ci.sh` and verify green status across all 391+ tests.
  * **952.2: Release gate with hardware storage validation (`scripts/gate.mjs`)**
    * *Step 2.1:* Run `node scripts/gate.mjs --scope release` with `DISCORD_LAB_KEYRING_LIVE=1`.
    * *Step 2.2:* Verify pass of all phases (`gpui-smoke`, `gpui-e2e`, `gpui-visual`, `keyring-live`).
    * *Step 2.3:* Ensure skipped test count is strictly `0`.
  * **952.3: Quality manifest documentation `QA-BUILD-005.md`**
    * *Step 3.1:* Record precise execution time and peak memory consumption.
    * *Step 3.2:* Draft report `waves/RW-44/QUALITY/QA-BUILD-005.md` with verdict `QA Passed`.
* **Definition of Done (DoD):**
  1. Script `scripts/ci.sh` exits with code `0`.
  2. `scripts/gate.mjs --scope release` yields verdict `passed` (zero skips).
  3. Signed `QA-BUILD-005.md` report with verdict `QA Passed`.

---

### TASK-953: Standalone `ldd` System Library and Linux Distribution Compatibility Verification
* **Department:** `devops`
* **Priority:** `P1`
* **Summary:** Dynamic link analysis of release binary `discord-gpui`, verification of baseline dependencies (glibc, libasound, libdbus-1), and cold start testing under software rendering llvmpipe.
* **Subtasks and Micro-steps:**
  * **953.1: Dependency inspection via `ldd` and `objdump`**
    * *Step 1.1:* Run `ldd rust/dist/discord-gpui` and inspect shared library list.
    * *Step 1.2:* Verify minimum required GLIBC version (`objdump -p rust/dist/discord-gpui | grep GLIBC_`). Requirement: at most `GLIBC_2.35`.
    * *Step 1.3:* Ensure no hard links to proprietary NVIDIA/AMD drivers exist.
  * **953.2: `llvmpipe` software rendering verification**
    * *Step 2.1:* Execute binary with `LIBGL_ALWAYS_SOFTWARE=1` and `MESA_LOADER_DRIVER_OVERRIDE=llvmpipe`.
    * *Step 2.2:* Run `--headless-smoke` and verify successful WGPU OpenGL/Vulkan initialization.
  * **953.3: System requirements report**
    * *Step 3.1:* Create document `docs/SYSTEM-REQUIREMENTS.md` listing supported Linux kernels (5.15+) and display servers.
* **Definition of Done (DoD):**
  1. Requirement GLIBC $\le 2.35$ satisfied (compatibility with Ubuntu 22.04 LTS, Debian 12, Fedora 38+).
  2. Binary successfully starts under software driver `llvmpipe` without crashing.
  3. Document `docs/SYSTEM-REQUIREMENTS.md` published and verified.

---

### TASK-954: Screen and Audio Capture Matrix Testing Across 4 Wayland/X11 Compositors
* **Department:** `g4_qa`
* **Priority:** `P1`
* **Summary:** Verification of screen sharing (`ScreenShareModal`) and audio capture across 4 primary Linux Desktop environments per `manual_qa_suite_contract.rs`.
* **Subtasks and Micro-steps:**
  * **954.1: GNOME (Mutter) portal test**
    * *Step 1.1:* Verify `org.freedesktop.portal.ScreenCast` D-Bus invocation under GNOME session.
    * *Step 1.2:* Confirm PipeWire FD transfer and NV12 60 FPS format negotiation.
  * **954.2: KDE Plasma 6 (KWin) portal test**
    * *Step 2.1:* Test window and monitor selection via KWin portal dialog.
    * *Step 2.2:* Check descriptor cleanup upon stream termination.
  * **954.3: Hyprland / Sway (wlroots) test**
    * *Step 3.1:* Verify screen capture via `xdg-desktop-portal-wlr`.
  * **954.4: X11 backward compatibility test**
    * *Step 4.1:* Verify automatic fallback to `XShm` in the absence of Wayland compositor.
* **Definition of Done (DoD):**
  1. Test `cargo test --test manual_qa_suite_contract test_desktop_matrix` passes 100% Green.
  2. DMA-BUF capture and X11 fallback verified without panics.

---

### TASK-955: Long-Term Memory Stability and Leak-Free Stress Test (Soak Test 4h)
* **Department:** `g4_qa`
* **Priority:** `P1`
* **Summary:** Automated long-running stress test of native client ensuring zero memory leaks, heap fragmentation, or file descriptor leaks under continuous load.
* **Subtasks and Micro-steps:**
  * **955.1: Long-term load scenario setup (`scripts/soak-test.mjs`)**
    * *Step 1.1:* Write load test simulating 10,000 messages per minute, 500 channel transitions, and 50 voice calls.
    * *Step 1.2:* Add cyclical media cache pruning upon hitting threshold.
  * **955.2: VmRSS and FD monitoring**
    * *Step 2.1:* Read `/proc/$PID/status` every 30 seconds.
    * *Step 2.2:* Verify `/proc/$PID/fd` for absence of socket descriptor leaks.
  * **955.3: Memory degradation evaluation**
    * *Step 3.1:* Ensure memory drift is strictly under $1$ MB per hour.
    * *Step 3.2:* Draft report `waves/RW-44/QUALITY/SOAK-TEST-REPORT.md`.
* **Definition of Done (DoD):**
  1. Peak memory consumption does not exceed 60 MB.
  2. Socket and file descriptor leaks are strictly 0.
  3. Report `SOAK-TEST-REPORT.md` generated and committed.

---

### TASK-956: Comprehensive Security Audit, Secret Scrubbing, and SBOM Manifest Generation
* **Department:** `devops`
* **Priority:** `P0`
* **Summary:** Audit codebase and release binaries for hardcoded tokens, passwords, private keys, verify log masking, and generate reproducible SBOM.
* **Subtasks and Micro-steps:**
  * **956.1: Static regex scan for secrets**
    * *Step 1.1:* Scan codebase against Discord token patterns (MFA, Bot, User tokens).
    * *Step 1.2:* Verify absence of private DAVE MLS keys in source files.
  * **956.2: Log masking and panic output audit**
    * *Step 2.1:* Verify that in panic messages and log outputs, tokens are masked as `mfa.***` or `Bot ***`.
  * **956.3: Final SBOM manifest generation**
    * *Step 3.1:* Run `node scripts/sbom.mjs --check --output rust/dist/sbom.spdx.json`.
    * *Step 3.2:* Validate licenses of all 85+ dependencies against open-source compliance (MIT / Apache-2.0 / BSD-3).
* **Definition of Done (DoD):**
  1. Secret scan yields 0 findings.
  2. Manifest `rust/dist/sbom.spdx.json` generated and conforms to SPDX 2.3.

---

### TASK-957: Promotion Procedure `testing -> beta` and Emergency Rollback Protocol
* **Department:** `devops`
* **Priority:** `P0`
* **Summary:** Prepare PR for promoting validated build from `testing` to `beta`, establish immediate rollback protocol for unexpected defects, and verify SQLite WAL database integrity on version downgrades.
* **Subtasks and Micro-steps:**
  * **957.1: PR preparation `testing -> beta`**
    * *Step 1.1:* Compile detailed PR description with included features, QA results, and checksums.
    * *Step 1.2:* Verify zero merge conflicts and receive Lead Orchestrator sign-off.
  * **957.2: Rollback protocol formulation**
    * *Step 2.1:* Document rapid commit revert in `beta` branch (`git revert -m 1 <merge-commit>`).
    * *Step 2.2:* Verify restoration procedure for previous release binary from cache.
  * **957.3: SQLite downgrade schema compatibility test**
    * *Step 3.1:* Verify that database opened with new build does not corrupt when reverted to previous client version.
* **Definition of Done (DoD):**
  1. Document `docs/specs/ROLLBACK-PROCEDURE.md` published and approved.
  2. SQLite database preserves integrity across cyclical update/rollback cycles.

---

### TASK-958: Beta Release Package, `SHA256SUMS` Checksums, and Testing Guide
* **Department:** `g6_tech_writer`
* **Priority:** `P1`
* **Summary:** Package stripped binary into release archive, update `SHA256SUMS`, author clear getting started and feedback guide in `docs/BETA-TESTING-GUIDE.md`.
* **Subtasks and Micro-steps:**
  * **958.1: Release archive packaging `tar.gz`**
    * *Step 1.1:* Package stripped binary `discord-gpui` and license files into `rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz`.
    * *Step 1.2:* Recalculate checksums and update `rust/dist/SHA256SUMS`.
  * **958.2: Beta tester guide creation**
    * *Step 2.1:* Author `docs/BETA-TESTING-GUIDE.md`:
      * System requirements and supported distributions.
      * Quick start: 1-command launch (`./discord-gpui`).
      * Debug logging instructions (`RUST_LOG=info ./discord-gpui`).
      * Defect reporting template (Linux distribution, GPU, logs).
  * **958.3: Release Notes publication for Beta 1**
    * *Step 3.1:* Create file `RELEASE-NOTES-v1.0.0-beta.1.md` summarizing delivered capabilities.
* **Definition of Done (DoD):**
  1. Archive `discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz` ready for distribution.
  2. File `rust/dist/SHA256SUMS` contains valid checksum for release archive.
  3. Documents `BETA-TESTING-GUIDE.md` and `RELEASE-NOTES-v1.0.0-beta.1.md` published.
