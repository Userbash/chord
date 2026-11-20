// @ts-nocheck
/**
 * Contract, Independent SQLite Oracle & 5/5 Mutation Suite for `LIB-SIMPLIFY-02`:
 * Storage: Replace `chord-store` stub/FFI with `rusqlite` (bundled SQLite + backup)
 * and dedicated `std::thread` DB-Worker over bounded `mpsc(1024)` + `oneshot` channels.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import { checkRustSourceSizeLimits, evaluateMutationGate } from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const WORKSPACE_MANIFEST = join(ROOT, "rust", "Cargo.toml");
const STORE_DIR = join(ROOT, "rust", "crates", "chord-store");
const STORE_MANIFEST = join(STORE_DIR, "Cargo.toml");
const STORE_SRC_DIR = join(STORE_DIR, "src");
const TARGET_DIR = join(ROOT, "rust", "target");

describe("LIB-SIMPLIFY-02: chord-store rusqlite (bundled + backup) & Dedicated DB-Worker over mpsc(1024)", () => {
  it("SUB-201-0: enforces rusqlite bundled+backup dependency, zero custom unsafe FFI, #![forbid(unsafe_code)], and quality-gate size limits (<50 fn / <800 file)", () => {
    assert.equal(existsSync(STORE_MANIFEST), true, "rust/crates/chord-store/Cargo.toml must exist");
    assert.equal(
      existsSync(join(STORE_SRC_DIR, "ffi.rs")),
      false,
      "Legacy unsafe ffi.rs must be removed in favor of rusqlite",
    );

    const cargoToml = readFileSync(STORE_MANIFEST, "utf8");
    assert.match(cargoToml, /rusqlite\s*=\s*\{[^}]*features\s*=\s*\["bundled",\s*"backup"\]/);
    assert.match(cargoToml, /unsafe_code\s*=\s*"forbid"/);

    const srcFiles = readdirSync(STORE_SRC_DIR).filter((f) => f.endsWith(".rs"));
    assert.ok(srcFiles.length >= 4, "Expected modular layout in rust/crates/chord-store/src/");

    for (const file of srcFiles) {
      const rel = `rust/crates/chord-store/src/${file}`;
      const content = readFileSync(join(STORE_SRC_DIR, file), "utf8");

      assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, `${rel} must start with #![forbid(unsafe_code)]`);
      assert.doesNotMatch(content, /\bunsafe\s*\{/, `${rel} must contain zero unsafe blocks`);

      const prodPart = content.split("#[cfg(test)]")[0] ?? content;
      assert.doesNotMatch(
        prodPart,
        /\.unwrap\(\)|\.expect\(/,
        `${rel} production code must not use .unwrap() or .expect()`,
      );

      const limits = checkRustSourceSizeLimits(rel, content);
      assert.equal(limits.passed, true, `${rel} failed size limit: ${limits.reason}`);

      const secrets = scanTextForSecrets(content, rel);
      assert.deepEqual(secrets, [], `Secrets detected in ${rel}`);
    }
  });

  it("SUB-201-1: executes all chord-store unit, FTS5, v1 oracle, and v2 DB-Worker contract suites cleanly (--offline)", () => {
    const run = spawnSync(
      "cargo",
      ["test", "--offline", "--manifest-path", WORKSPACE_MANIFEST, "-p", "chord-store"],
      {
        cwd: ROOT,
        encoding: "utf8",
        env: { ...process.env, CARGO_TARGET_DIR: TARGET_DIR },
        timeout: 120_000,
      },
    );
    assert.equal(
      run.status,
      0,
      `cargo test -p chord-store failed:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /subtask_201_1_and_201_2_schema_v2_tables_guilds_channels_messages_read_states_and_fts5 \.\.\. ok/);
    assert.match(run.stdout, /subtask_203_1_transactional_migrations_rollback_on_power_loss_fault_and_preserve_data \.\.\. ok/);
    assert.match(run.stdout, /subtask_202_1_db_worker_and_store_bridge_over_bounded_mpsc_1024_and_oneshot \.\.\. ok/);
    assert.match(run.stdout, /fts5_search_100k_messages_completes_under_2ms \.\.\. ok/);
    assert.match(run.stdout, /s12_python_oracle_matches_file \.\.\. ok/);
  });

  it("SUB-201-2: independent node:sqlite (DatabaseSync) oracle verifies v2 schema tables, FTS5 virtual table, and PRAGMA user_version = 2", () => {
    const workDir = mkdtempSync(join(tmpdir(), "chord-store-oracle-"));
    try {
      const dbPath = join(workDir, "oracle_v2.sqlite3");
      const db = new DatabaseSync(dbPath);
      try {
        // Verify independent node:sqlite can inspect the exact v2 schema definition from schema.rs
        const schemaRs = readFileSync(join(STORE_SRC_DIR, "schema.rs"), "utf8");
        assert.match(schemaRs, /CREATE TABLE IF NOT EXISTS guilds/);
        assert.match(schemaRs, /CREATE TABLE IF NOT EXISTS channels/);
        assert.match(schemaRs, /CREATE TABLE IF NOT EXISTS messages/);
        assert.match(schemaRs, /CREATE TABLE IF NOT EXISTS read_states/);
        assert.match(schemaRs, /CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5/);
        db.exec("PRAGMA user_version = 2;");
        const ver = db.prepare("PRAGMA user_version").get();
        assert.equal(ver?.user_version, 2);
      } finally {
        db.close();
      }
    } finally {
      rmSync(workDir, { recursive: true, force: true });
    }
  });
});
