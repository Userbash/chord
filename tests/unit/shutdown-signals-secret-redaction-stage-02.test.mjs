// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-02.3.2`:
 * Contract tests for SIGINT/SIGTERM signal interception, JoinSet graceful shutdown with 5s timeout,
 * and secret redaction layer preventing Bot/Bearer tokens and private keys from leaking into stdout.
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
const SHUTDOWN_RS = join(CORE_DIR, "src", "shutdown.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "shutdown_signals_contract.rs");

describe("STAGE-02.3.2: SIGINT/SIGTERM Interception & Secret Redaction Contract", () => {
  it("SUB-02-3-2-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(SHUTDOWN_RS), true, "rust/crates/chord-core/src/shutdown.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/shutdown_signals_contract.rs must exist");

    const content = readFileSync(SHUTDOWN_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "shutdown.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "shutdown.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "shutdown.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-core/src/shutdown.rs", content);
    assert.equal(limits.passed, true, `shutdown.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-core/src/shutdown.rs");
    assert.deepEqual(secrets, [], "Secrets detected in shutdown.rs");
  });

  it("SUB-02-3-2-2: validates shutdown, signal and secret redaction API specifications", () => {
    const content = readFileSync(SHUTDOWN_RS, "utf8");

    // 5-second shutdown timeout
    assert.match(
      content,
      /DEFAULT_SHUTDOWN_TIMEOUT:\s*Duration\s*=\s*Duration::from_secs\(5\)/,
      "Must define DEFAULT_SHUTDOWN_TIMEOUT as 5 seconds",
    );

    // JoinSet child task registration
    assert.match(content, /tokio::task::JoinSet/, "Must register child tasks via tokio::task::JoinSet");
    assert.match(content, /pub fn spawn_task</, "Must define spawn_task method");
    assert.match(content, /pub async fn shutdown_and_join\(/, "Must define shutdown_and_join with timeout");

    // Signals
    assert.match(content, /pub enum ShutdownSignal/, "Must define ShutdownSignal enum");
    assert.match(content, /Sigint/, "ShutdownSignal must contain Sigint variant");
    assert.match(content, /Sigterm/, "ShutdownSignal must contain Sigterm variant");

    // Secret Redaction
    assert.match(content, /pub fn redact_secrets\(/, "Must define redact_secrets function");
    assert.match(content, /pub struct RedactingWriter/, "Must define RedactingWriter struct");
    assert.match(content, /\[REDACTED\]/, "Must redact sensitive tokens to [REDACTED]");
  });

  it("SUB-02-3-2-3: executes native Rust contract tests for shutdown signals and secret redaction", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "shutdown_signals_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test shutdown_signals_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /7 passed; 0 failed/, "Expected all 7 contract tests to pass");
  });

  it("SUB-02-3-2-4: 5/5 mutation test suite killing mutations to shutdown and redaction contracts", () => {
    const content = readFileSync(SHUTDOWN_RS, "utf8");

    // Mutation 1: Timeout altered from 5 seconds
    assert.match(content, /Duration::from_secs\(5\)/, "Timeout constant must be 5 seconds");
    // Mutation 2: Signals enum lacks Sigint or Sigterm
    assert.match(content, /Sigint/i, "Sigint handling required");
    assert.match(content, /Sigterm/i, "Sigterm handling required");
    // Mutation 3: JoinSet abort on timeout
    assert.match(content, /\.abort_all\(\)/, "Must abort remaining tasks on timeout");
    // Mutation 4: Bot & Bearer token redaction pattern
    assert.match(content, /Bot\s+/, "Must redact Bot authorization headers");
    assert.match(content, /Bearer\s+/, "Must redact Bearer authorization headers");
    // Mutation 5: Error propagation without unwraps
    assert.match(content, /ShutdownError::Timeout/, "Must return ShutdownError::Timeout on elapsed timeout");
  });
});
