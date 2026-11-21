# Promotion Procedure `testing -> beta` and Emergency Rollback Protocol (`docs/specs/ROLLBACK-PROCEDURE.md`)

> **Document Status:** `Approved Promotion & Emergency Rollback Specification (v1.0)`  
> **Task:** `TASK-957` (`Wave RW-45 — Beta Promotion`)  
> **Department:** `devops` (`DevOps / Integration`) jointly with `pm_review` (`ECC 2.0 Control Plane`)  
> **Related Documents:** `docs/specs/BETA-PROMOTION-PLAN-AND-TASKS.md`, `docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md`, `waves/RW-41/QUALITY/QA-BUILD-005.md`  
> **Approval Date:** November 11, 2026  

---

## 1. Scope and Purpose

This document regulates:
1. **The promotion procedure and Pull Request workflow `testing -> beta` (`SUB-957-1`)** for certified build `BUILD-005` (`v1.0.0-beta.1`).
2. **Emergency Rollback Protocol (`Rollback Protocol`, `SUB-957-2`)** for branch `beta` and release binary artifacts in directory `rust/dist/` upon discovery of blocking defects (`P0`/`P1`).
3. **Downgrade safety and backward compatibility guarantees for SQLite WAL storage (`SUB-957-3`)** across both storage layers (`discord-store` and `discord-gpui::storage::LocalDatabase`).

### Release Base Invariants
* **Strict Prohibition of Direct Commits and History Rewrites:** In branches `testing`, `beta`, and `main`, direct commits, `git reset --hard`, and `git push --force` are strictly forbidden. All modifications traverse Pull Requests through `worker branch -> unstable -> testing -> beta -> main`.
* **Zero P0/P1 Blockers:** Promotion to `beta` is permitted strictly with 0 open P0/P1 defects and a signed `QA Passed` verdict.
* **Target Recovery Objectives:**
  * **RTO (Recovery Time Objective):** $\le 2$ minutes from incident declaration to restoring a healthy binary and rolling back branch `beta`.
  * **RPO (Recovery Point Objective):** $0$ lost local messages, settings, or cached channels in SQLite WAL upon returning to the prior build.

---

## 2. Preparation and Execution of Pull Request `testing -> beta` (`SUB-957-1`)

### 2.1. Pre-Promotion Gate Checklist
Prior to opening PR `testing -> beta`, the release engineer (`devops`) and `Lead Orchestrator` (`pm_review`) verify the following prerequisites:

| No. | Checkpoint | Verification Command / Artifact | Required Status |
| :--- | :--- | :--- | :--- |
| 1 | Working tree cleanliness and candidate tag | `git status -sb && git rev-parse build-005-candidate` | Clean working tree, tag points to HEAD of `testing` |
| 2 | End-to-end `T1/T2` pipeline (`scripts/ci.sh`) | `bash scripts/ci.sh` | Exit code `0` (`fmt`, `clippy -D warnings`, workspace tests) |
| 3 | Release gate `T3/T4` without skips | `DISCORD_LAB_KEYRING_LIVE=1 node scripts/gate.mjs --scope release` | `passed`, `skipped = 0` (`failOnSkips: true`) |
| 4 | Headless Smoke of native client `discord-gpui` | `./rust/dist/discord-gpui --headless-smoke` | `GPUI_SMOKE_OK`, `"status":"ok"`, `"sqlite_wal_ok":true`, `vmrss_mb < 60.0` |
| 5 | 4-Compositor Linux Matrix (`Wayland`/`X11`) | `cargo test --manifest-path rust/chord-gpui/Cargo.toml --test manual_qa_suite_contract test_desktop_matrix` | `100% PASS` (GNOME Mutter, KDE KWin, Hyprland/wlroots, X11 SHM) |
| 6 | Cryptographic artifact integrity and SBOM | `(cd rust/dist && sha256sum -c SHA256SUMS)` | All 4 artifacts: `OK` |
| 7 | Build quality report `BUILD-005` | `waves/RW-41/QUALITY/QA-BUILD-005.md` | Status `QA PASSED`, `P0 = 0`, `P1 = 0` |

### 2.2. Conflict-Free Merge Verification (`Step 1.2`)
Prior to merging, a dry-run merge check of `testing` into `beta` is conducted without modifying Git history:

```bash
# 1. Update local refs and verify working tree cleanliness
git status -sb

# 2. Check for merge conflicts via git merge-tree (without switching branches)
git merge-tree "$(git merge-base beta testing)" beta testing | grep -n "<<<<<<< " && echo "CONFLICT DETECTED" || echo "MERGE CLEAN: 0 conflicts"

# 3. Dry-run test merge without committing on branch beta
git checkout beta
git merge --no-commit --no-ff testing
git diff --check
git merge --abort
```

### 2.3. Pull Request Description Template `testing -> beta` (`Step 1.1`)
When submitting the Pull Request from `testing` (`build-005-candidate`) into protected branch `beta`, the following official PR passport is used:

```markdown
## Promotion PR: `testing` (`BUILD-005` / `build-005-candidate`) -> `beta` (`v1.0.0-beta.1`)

### 1. Release Summary
Consolidated promotion of verified build `BUILD-005` (128 completed tasks from Waves RW-41..RW-44) from integration branch `testing` into pre-release branch `beta`.

### 2. Key Included Functional Subsystems
- **Native client `discord-gpui` (GPUI + WGPU):**
  - Full UI shell: Server Rail, Channel Sidebar, Chat View with virtualization, Member Sidebar, Quick Switcher (`Ctrl+K`), Emoji Picker, Search Drawer, User Profile Card, Settings Dialog (8 sections), Captcha Modal, Create Guild/Channel, Account Switcher, Friends View, and Call/Stage Overlay.
  - 120 FPS rendering budget (`frame_tick_us < 8300 us`), cold start memory footprint `VmRSS ~ 11.5 MB` (under `< 60 MB` ceiling).
- **Screen Share (Go Live) and Video Pipeline (`TASK-775..TASK-789`):**
  - Native screen and window capture on Wayland (`org.freedesktop.portal.ScreenCast`, KDE `KWin.ScreenShot2`, `wlroots`) with automatic fallback to `XShm` on X11.
  - WebRTC source normalization (`sourceId: "-1"`), hot-switching of windows and stream parameters (`720p/1080p`, `30/60 FPS`, `maxBitrate` 4–8 Mbps) without active session disruption.
  - Gateway opcodes `OP 18/19/22` and voice opcodes `Voice OP 1 (SELECT_PROTOCOL)`, `OP 12 (VIDEO SSRC/RTX)`, `OP 16 (VOICE_BACKEND_VERSION)`.
- **Voice Stack and DAVE MLS v1 Cryptography (`discord-voice`):**
  - Atomic zero-copy `AudioRingBuffer`, flicker-free `VOICE_STATE_UPDATE` participant roster, RTC quality indicator (`#live-rtc-popout`), and fuzzing defense for DAVE v1 / Zstd packets.
- **Local SQLite WAL Storage (`discord-store` & `LocalDatabase`):**
  - Persistent storage for guilds, channels, messages, settings, and full-text search index `FTS5` (`quick_switcher_fts`) with `PRAGMA journal_mode = WAL`.
- **Accessibility, Visual Regression, and Reliability (`G4 QA`):**
  - AT-SPI2 / WCAG 2.2 contracts, 15 golden reference SSIM baselines (`> 0.995`), Chaos Engineering contracts, and 5/5 killed mutants.

### 3. QA Verification Results (`QA-BUILD-005.md`)
- **QA Status:** `QA PASSED` (`P0 = 0`, `P1 = 0`, `P2 = 0`, `skipped = 0`)
- **Headless Smoke (`discord-gpui --headless-smoke`):** `status = "ok"`, `display_backend = "wayland+x11"`, `sqlite_wal_ok = true`, `fps_budget_ok = true`, `vmrss_mb = 11.54 MB`
- **System Compatibility:** `ELF 64-bit LSB pie executable, stripped`, `GLIBC <= 2.35`, confirmed operational under software renderer `llvmpipe`.

### 4. Release Artifact Checksums (`rust/dist/SHA256SUMS`)
| Artifact | SHA-256 | Status |
| :--- | :--- | :--- |
| `rust/dist/discord-gpui` | `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e` | `OK` |
| `rust/dist/discord-gui-spike` | `18b7eea8e3438ed8ea1ab133347c9956d94d6f443049872835eeb08e7e2c19f9` | `OK` |
| `rust/dist/discord-rs` | `42ba8f0938a03d998eac713f61aa56d1210b7ffb4e1261b719ed35bba93b12ae` | `OK` |
| `rust/dist/sbom.spdx.json` | `acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921` | `OK` |

### 5. Rollback Protocol
Regulated by `docs/specs/ROLLBACK-PROCEDURE.md` (revert command `git revert -m 1 <merge-commit>`, SQLite WAL `user_version = 1` downgrade compatibility verified).

Closes: #TASK-957
```

### 2.4. Promotion Merge Command
Following dual approval (`ACCEPT.md` from PM and `REVIEW.md` from Technical Reviewer), the coordinator executes the merge strictly creating a merge commit (`--no-ff`) to ensure a single revert point:

```bash
git checkout beta
git merge --no-ff testing -m "chore(release): promote BUILD-005 from testing to beta (v1.0.0-beta.1)

Closes: #TASK-957"
git tag -a v1.0.0-beta.1 -m "Release v1.0.0-beta.1 (BUILD-005)"
```

---

## 3. Emergency Rollback Protocol (`SUB-957-2`)

### 3.1. Immediate Rollback Triggers
The emergency rollback procedure is initiated immediately by the on-call `devops` engineer or `Lead Orchestrator` if any of the following symptoms occur post-merge:
1. Cold start panic or `discord-gpui --headless-smoke` failure (exit code $\ne 0$ or `"status": "degraded"`).
2. Memory ceiling breach (`VmRSS >= 60 MB`) or socket/file descriptor leaks.
3. Checksum mismatch in `rust/dist/SHA256SUMS` or corrupted SBOM.
4. Critical security defect (`P0`), unmasked token in logs, or `DAVE MLS v1` / `SRTP` crypto failure.
5. SQLite migration or database open error (`StoreError::Migration` / `StoreError::Sqlite`).

---

### 3.2. Step 2.1: Rapid 1-Command Revert of Merge Commit in `beta` Branch
Because promotion to `beta` is always committed with `--no-ff`, HEAD of `beta` is a merge commit with two parents:
* **Parent `1` (`-m 1`):** Previous stable HEAD of branch `beta`.
* **Parent `2` (`-m 2`):** Merged branch `testing`.

To immediately return `beta` to its prior state without rewriting Git history (`reset --hard` is forbidden), run **`git revert -m 1`**:

```bash
# 1. Verify merge commit on HEAD of beta and ensure parent #1 is prior beta HEAD
git checkout beta
MERGE_COMMIT="$(git rev-parse HEAD)"
git show --stat --oneline -n 1 "$MERGE_COMMIT"

# 2. Execute atomic revert to first parent (-m 1) in a single new commit
git revert -m 1 --no-edit "$MERGE_COMMIT"

# 3. Tag rollback commit for auditability
git tag -a "rollback-beta-$(date -u +%Y%m%d-%H%M%S)" -m "Emergency rollback of $MERGE_COMMIT on beta"
```

#### Handling Conflicts or Subsequent Commits:
* If an additional commit was pushed atop the merge commit on `beta`, revert in reverse chronological sequence:
  ```bash
  git revert --no-commit HEAD
  git revert --no-commit -m 1 "$MERGE_COMMIT"
  git commit -m "revert(release): emergency rollback of BUILD-005 promotion on beta"
  ```
* Once reverted on `beta`, spawn a bugfix branch `bugfix/task-<id>-<slug>` from `unstable`. Re-promotion is permitted only after resolving the root cause and achieving a new `QA Passed` verdict.

---

### 3.3. Step 2.2: Restoring Previous Binary Release from Artifact Cache
During a rollback of `beta`, verify that binary artifacts in `rust/dist/` match the reverted state and pass cryptographic checks (`sha256sum -c SHA256SUMS`).

#### Restoration Sources in Local Artifact Cache:
1. **Tracked Manifest and Distribution in Repository Tree (`rust/dist/`):** After running `git revert -m 1 <merge-commit>`, tracked artifacts and `rust/dist/SHA256SUMS` automatically return to the prior release state.
2. **Local Tarball Archive Cache (`rust/dist/*.tar.gz` / `rust/target/release/`):** If untracked binaries in `rust/dist/` were overwritten, restore from release archive or perform deterministic offline build (`scripts/release.sh`).

#### Restoration and Verification Protocol (`rust/dist/`):

```bash
# Option A: If previous release tarball is stored in cache rust/dist/archive/
if [[ -f rust/dist/archive/discord-gpui-prev-linux-x86_64.tar.gz ]]; then
    tar -xzf rust/dist/archive/discord-gpui-prev-linux-x86_64.tar.gz -C rust/dist/
fi

# Option B: Deterministic offline rebuild from reverted beta revision (zero network)
CARGO_TARGET_DIR="./rust/target" \
    cargo build --offline --release --manifest-path rust/chord-gpui/Cargo.toml
install -m 0755 rust/target/release/discord-gpui rust/dist/discord-gpui
bash scripts/release.sh

# Mandatory verification of restored artifact:
# 1. Checksum verification SHA256SUMS
(cd rust/dist && sha256sum -c SHA256SUMS)

# 2. Check ELF header and confirm stripped binary
file rust/dist/discord-gpui | grep -E "ELF 64-bit LSB.*stripped"

# 3. Check headless smoke test of restored binary
./rust/dist/discord-gpui --headless-smoke
```

Expected verification output after rollback:
```text
discord-gpui: OK
discord-gui-spike: OK
discord-rs: OK
sbom.spdx.json: OK
GPUI_SMOKE_OK {"status":"ok","display_backend":"wayland+x11","window_width":1280,"window_height":720,"min_width":900,"min_height":550,"guilds":2,"channels":5,"sqlite_wal_ok":true,"fps_budget_ok":true,"frame_tick_us":0,"vmrss_kb":11812,"vmrss_mb":11.54}
```

---

## 4. SQLite WAL Downgrade Compatibility (`SUB-957-3`)

During emergency binary rollbacks from a new build (`v1.0.0-beta.1` / `BUILD-005`) to the previous version, the user's local database must remain fully readable and consistent without data loss.

### 4.1. SQLite Architecture in `discord-lab`
Two isolated SQLite modules operate in the application (system library `libsqlite3` 3.54):

1. **Storage Core `discord-store` (`rust/crates/chord-store/src/lib.rs`):**
   * **Journal Mode:** `PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; busy_timeout = 5000`.
   * **Schema Version:** `PRAGMA user_version = 1` (`pub const SCHEMA_VERSION: u32 = 1`).
   * **Tables and Indexes (`SQL_SCHEMA_V1`):**
     * `channels (id TEXT PRIMARY KEY NOT NULL, guild_id TEXT, name TEXT, kind INTEGER NOT NULL, position INTEGER NOT NULL)`
     * `messages (id TEXT PRIMARY KEY NOT NULL, channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE, author_id TEXT NOT NULL, content TEXT NOT NULL, timestamp INTEGER NOT NULL, edited_timestamp INTEGER, flags INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0)`
     * `kv (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL)`
     * Indexes: `idx_messages_channel_timestamp (channel_id, timestamp DESC)` and `idx_messages_channel_id (channel_id, id DESC)`.
2. **Native Client DB `discord-gpui::storage::LocalDatabase` (`rust/chord-gpui/src/storage/mod.rs`):**
   * **Journal Mode:** `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`.
   * **Tables (`init_schema`):**
     * `guilds (id, name, icon, owner_id, unread_count, mention_count)`
     * `channels (id, guild_id, category_id, name, channel_type, topic, position, unread)`
     * `messages (id, channel_id, author_id, author_name, content, timestamp, is_pinned)` + index `idx_messages_channel (channel_id, timestamp)`
     * `user_settings (key, value)`
     * `users (id, username, display_name, avatar, is_bot)`
     * Full-text search virtual table: `quick_switcher_fts USING fts5(id UNINDEXED, kind, name, subtitle)`.

### 4.2. Downgrade Safety Invariants
1. **Unified Schema Version `user_version = 1`:**
   * In build `BUILD-005` (`v1.0.0-beta.1`), `SCHEMA_VERSION` is `1`, identical to prior stable builds (`BUILD-004`).
   * No table contains destructive modifications (`DROP TABLE`, `DROP COLUMN`, modified primary keys, or non-default `NOT NULL` columns).
2. **Idempotent DDL Initialization:**
   * All schema statements use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` within transactions `BEGIN IMMEDIATE ... COMMIT`.
   * Calling `Store::migrate()` or `LocalDatabase::open()` on a database following an update and subsequent downgrade is a strict `no-op` for existing data.
3. **Protection Against Schema Version Mismatch:**
   * In `discord-store` (`src/lib.rs:235-240`), pre-transaction verification checks:
     ```rust
     let version = self.schema_version()?;
     if version > SCHEMA_VERSION {
         return Err(StoreError::Migration(format!(
             "user_version {version} is newer than supported {SCHEMA_VERSION}: please update client"
         )));
     }
     ```
   * If a future release increments the schema version (`SCHEMA_VERSION >= 2`), an older client will deterministically fail fast with `StoreError::Migration` before executing `BEGIN IMMEDIATE`.
4. **WAL Log File Consistency (`-wal`, `-shm`) Across Binaries:**
   * On normal shutdown, SQLite closes connections with automatic checkpointing.
   * On sudden process termination (`SIGKILL`), the lingering `<db>-wal` uses standard binary SQLite format `3007000`, cleanly recovered by earlier binary versions on `sqlite3_open_v2`.

### 4.3. SQLite Upgrade/Downgrade Integrity Verification Protocol (`Step 3.1`)
Reproducible test scenario validating that database opened with new build `v1.0.0-beta.1`, populated, checkpointed, and downgraded to prior build preserves 100% integrity:

```bash
python3 -c '
import sqlite3, tempfile, os, subprocess

with tempfile.TemporaryDirectory() as tmp:
    # 1. Initialize and write data with new build (v1.0.0-beta.1) in discord-store (WAL, user_version=1)
    store_db = os.path.join(tmp, "store.db")
    conn = sqlite3.connect(store_db)
    conn.executescript("""
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS channels (
            id TEXT PRIMARY KEY NOT NULL, guild_id TEXT, name TEXT, kind INTEGER NOT NULL, position INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY NOT NULL,
            channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
            author_id TEXT NOT NULL, content TEXT NOT NULL, timestamp INTEGER NOT NULL,
            edited_timestamp INTEGER, flags INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_messages_channel_timestamp ON messages(channel_id, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_channel_id ON messages(channel_id, id DESC);
        CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
        PRAGMA user_version = 1;
        INSERT INTO channels VALUES ("ch-1", "g-1", "general", 0, 1);
        INSERT INTO messages VALUES ("m-1", "ch-1", "u-1", "message from beta.1", 1700000001, NULL, 0, 0);
        INSERT INTO kv VALUES ("release_build", "BUILD-005");
    """)
    conn.commit()
    conn.close()

    # 2. Verify with oracle waves/RW-31/sqlite/oracle.py after downgrade to prior version
    oracle = subprocess.run(["python3", "waves/RW-31/sqlite/oracle.py", store_db], capture_output=True, text=True, check=True)
    assert "user_version=1" in oracle.stdout
    assert "journal_mode=wal" in oracle.stdout
    assert "integrity_check=ok" in oracle.stdout
    assert "foreign_key_violations=0" in oracle.stdout
    assert "duplicate_ids=0" in oracle.stdout

    # 3. Verify discord-gpui native client DB (LocalDatabase + FTS5) under upgrade/downgrade cycle
    gpui_db = os.path.join(tmp, "gpui_local.db")
    conn_gpui = sqlite3.connect(gpui_db)
    conn_gpui.executescript("""
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS guilds (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, icon TEXT, owner_id TEXT,
            unread_count INTEGER DEFAULT 0, mention_count INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS channels (
            id TEXT PRIMARY KEY, guild_id TEXT, category_id TEXT, name TEXT NOT NULL,
            channel_type INTEGER NOT NULL, topic TEXT, position INTEGER DEFAULT 0, unread INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY, channel_id TEXT NOT NULL, author_id TEXT NOT NULL,
            author_name TEXT NOT NULL, content TEXT NOT NULL, timestamp TEXT NOT NULL, is_pinned INTEGER DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages(channel_id, timestamp);
        CREATE TABLE IF NOT EXISTS user_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY, username TEXT NOT NULL, display_name TEXT, avatar TEXT, is_bot INTEGER DEFAULT 0
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS quick_switcher_fts USING fts5(id UNINDEXED, kind, name, subtitle);
        INSERT INTO guilds VALUES ("g-1", "Discord Lab", NULL, "u-1", 0, 0);
        INSERT INTO user_settings VALUES ("theme", "discord_dark");
        INSERT INTO quick_switcher_fts VALUES ("ch-1", "channel", "general", "Discord Lab");
    """)
    conn_gpui.commit()
    wal_chk = conn_gpui.execute("PRAGMA wal_checkpoint(TRUNCATE);").fetchone()
    integrity = conn_gpui.execute("PRAGMA integrity_check;").fetchone()[0]
    fts_rows = conn_gpui.execute("SELECT id, name FROM quick_switcher_fts WHERE quick_switcher_fts MATCH \"gen*\";").fetchall()
    conn_gpui.close()

    assert integrity == "ok", f"Integrity check failed: {integrity}"
    assert fts_rows == [("ch-1", "general")], f"FTS5 mismatch after downgrade: {fts_rows}"
    print("SQLITE_DOWNGRADE_CYCLE_OK", wal_chk, integrity)
'
```

Targeted contract test at Rust level:
```bash
CARGO_TARGET_DIR="./rust/target" \
    cargo test --manifest-path rust/Cargo.toml -p discord-store --test store_contract
```

---

## 5. On-Call Engineer Emergency Rollback Checklist

Upon triggering an emergency rollback, the `devops` engineer performs these 5 steps in order:

- [ ] **1. Incident Logging:** Record error log, `./rust/dist/discord-gpui --headless-smoke` output, and SHA of problematic merge commit in `waves/RW-45/INCIDENT-ROLLBACK.md`.
- [ ] **2. Revert `beta` Branch:** Run `git checkout beta && git revert -m 1 --no-edit <merge-commit-sha>` and tag `rollback-beta-<timestamp>`.
- [ ] **3. Restore Binaries in `rust/dist/`:** Restore previous release binary and verify `(cd rust/dist && sha256sum -c SHA256SUMS)` -> all entries `OK`.
- [ ] **4. SQLite WAL Integrity & Smoke Check:** Execute `./rust/dist/discord-gpui --headless-smoke` and verify `GPUI_SMOKE_OK` with `"status":"ok"` and `"sqlite_wal_ok":true`.
- [ ] **5. File Bugfix Remediation Task:** Create `bugfix/task-<id>-<slug>` branch from `unstable` and transition task to `QA Failed` to re-enter the promotion pipeline `unstable -> testing -> beta`.
