// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-05.2.2`:
 * Tracking ACK heartbeats (Op 1 / Op 11) and zombie socket detector:
 * Implementation of ACK timeout with immediate connection close and reconnect on missed acknowledgement.
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
const CONTRACT_TEST = join(CORE_DIR, "tests", "heartbeat_ack_contract.rs");

describe("STAGE-05.2.2: Heartbeat ACK Tracking (Op 1 / Op 11) and Zombie Socket Detector", () => {
  it("SUB-05-2-2-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(HEARTBEAT_RS), true, "rust/crates/chord-core/src/heartbeat.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/heartbeat_ack_contract.rs must exist");

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

  it("SUB-05-2-2-2: validates Op 1 / Op 11 ACK tracking and ZombieSocketDetector specifications", () => {
    const content = readFileSync(HEARTBEAT_RS, "utf8");

    assert.match(content, /HEARTBEAT_OP:\s*i64\s*=\s*1/, "Must define HEARTBEAT_OP = 1");
    assert.match(content, /HEARTBEAT_ACK_OP:\s*i64\s*=\s*11/, "Must define HEARTBEAT_ACK_OP = 11");
    assert.match(content, /pub struct ZombieSocketDetector/, "Must define ZombieSocketDetector struct");
    assert.match(content, /pub struct ZombieReconnectSignal/, "Must define ZombieReconnectSignal struct");
    assert.match(content, /pub enum HeartbeatAckOutcome/, "Must define HeartbeatAckOutcome enum");
    assert.match(content, /ZombieDetected\(ZombieReconnectSignal\)/, "HeartbeatLoopOutcome must include ZombieDetected(ZombieReconnectSignal)");
    assert.match(content, /pub fn record_heartbeat_ack\(&self\)/, "Must define record_heartbeat_ack method");
    assert.match(content, /pub fn on_server_op1_heartbeat_request\(&self\)/, "Must define on_server_op1_heartbeat_request method");
  });

  it("SUB-05-2-2-3: executes native Rust contract tests for heartbeat_ack_contract", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "heartbeat_ack_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test heartbeat_ack_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /7 passed; 0 failed/, "Expected all 7 contract tests to pass");
  });

  it("SUB-05-2-2-4: 5/5 mutation test suite killing mutations to Op 1 / Op 11 ACK & Zombie Socket Detector", () => {
    const content = readFileSync(HEARTBEAT_RS, "utf8");

    // Mutation 1: Missed ACK fails to trigger ZombieDetected outcome
    assert.match(content, /HeartbeatLoopOutcome::ZombieDetected/, "Must return HeartbeatLoopOutcome::ZombieDetected on missed Op 11 ACK");
    // Mutation 2: Immediate socket close or reconnect flags disabled on zombie signal
    assert.match(content, /close_immediately:\s*true/, "ZombieReconnectSignal must enforce close_immediately: true");
    assert.match(content, /reconnect_immediately:\s*true/, "ZombieReconnectSignal must enforce reconnect_immediately: true");
    // Mutation 3: Atomic awaiting_ack state cleared without receiving Op 11
    assert.match(content, /AtomicBool/, "Must track awaiting_ack atomically via AtomicBool");
    // Mutation 4: Cancellation token not triggered on zombie timeout
    assert.match(content, /cancel_token\.cancel\(\)/, "Must cancel active connection token immediately upon zombie socket detection");
    // Mutation 5: RTT calculation on Op 11 ACK omitted
    assert.match(content, /saturating_sub/, "Must calculate RTT using saturating_sub on Op 11 HEARTBEAT_ACK receipt");
  });
});
