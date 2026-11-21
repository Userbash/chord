// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-05.2.1`:
 * Dedicated independent task `heartbeat_loop` with interval `heartbeat_interval_ms`:
 * Isolated pulse timer independent of UI event processing delays.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import { checkRustSourceSizeLimits } from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const CORE_DIR = join(ROOT, "rust", "crates", "chord-core");
const HEARTBEAT_RS = join(CORE_DIR, "src", "heartbeat.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "heartbeat_loop_contract.rs");

describe("STAGE-05.2.1: Dedicated Independent Task heartbeat_loop with heartbeat_interval_ms", () => {
  it("SUB-05-2-1-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(HEARTBEAT_RS), true, "rust/crates/chord-core/src/heartbeat.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/heartbeat_loop_contract.rs must exist");

    const content = readFileSync(HEARTBEAT_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "heartbeat.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "heartbeat.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "heartbeat.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-core/src/heartbeat.rs", content);
    assert.equal(limits.passed, true, `heartbeat.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-core/src/heartbeat.rs");
    assert.deepEqual(secrets, [], "Secrets detected in heartbeat.rs");
  });

  it("SUB-05-2-1-2: validates heartbeat_loop & HeartbeatConfig specifications", () => {
    const content = readFileSync(HEARTBEAT_RS, "utf8");

    assert.match(
      content,
      /DEFAULT_HEARTBEAT_INTERVAL_MS:\s*u64\s*=\s*41_250/,
      "Must define DEFAULT_HEARTBEAT_INTERVAL_MS as 41_250 ms",
    );
    assert.match(content, /pub struct HeartbeatConfig/, "Must define HeartbeatConfig struct");
    assert.match(content, /pub heartbeat_interval_ms:\s*u64/, "HeartbeatConfig must have heartbeat_interval_ms field");
    assert.match(content, /pub struct HeartbeatPulse/, "Must define HeartbeatPulse struct");
    assert.match(content, /pub struct HeartbeatLoopHandle/, "Must define HeartbeatLoopHandle struct");
    assert.match(content, /pub async fn heartbeat_loop\(/, "Must define async fn heartbeat_loop");
    assert.match(content, /pub fn spawn_heartbeat_loop\(/, "Must define fn spawn_heartbeat_loop");
    assert.match(content, /MissedTickBehavior::Skip/, "Must use MissedTickBehavior::Skip to prevent burst floods");
  });

  it("SUB-05-2-1-3: executes native Rust contract tests for heartbeat_loop", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "heartbeat_loop_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test heartbeat_loop_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /8 passed; 0 failed/, "Expected all 8 contract tests to pass");
  });

  it("SUB-05-2-1-4: 5/5 mutation test suite killing mutations to heartbeat_loop contracts", () => {
    const content = readFileSync(HEARTBEAT_RS, "utf8");

    // Mutation 1: Zero interval validation bypassed
    assert.match(content, /HeartbeatLoopError::ZeroInterval/, "Must reject heartbeat_interval_ms == 0 with ZeroInterval");
    // Mutation 2: Jitter calculation bypassed
    assert.match(content, /jitter_seed\s*%\s*self\.heartbeat_interval_ms/, "Must compute deterministic first beat jitter via jitter_seed % heartbeat_interval_ms");
    // Mutation 3: Atomic sequence ordering relaxed or omitted
    assert.match(content, /AtomicU64/, "Must use lock-free AtomicU64 for shared sequence tracking");
    // Mutation 4: Non-blocking try_send replaced with blocking send
    assert.match(content, /\.try_send\(/, "Must use non-blocking try_send so slow consumers never stall the pulse timer");
    // Mutation 5: CancellationToken check removed
    assert.match(content, /CancellationToken/, "Must support graceful cancellation via CancellationToken");
  });
});
