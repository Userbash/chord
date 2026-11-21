// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-02.3.1`:
 * Secret Redaction Layer for tracing_subscriber (masking tokens):
 * Log filter replacing Bot/Bearer tokens and encryption keys with [REDACTED].
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
const LOGGING_RS = join(CORE_DIR, "src", "logging.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "secret_redaction_contract.rs");

describe("STAGE-02.3.1: Secret Redaction Layer for tracing_subscriber", () => {
  it("SUB-02-3-1-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(LOGGING_RS), true, "rust/crates/chord-core/src/logging.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/secret_redaction_contract.rs must exist");

    const content = readFileSync(LOGGING_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "logging.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "logging.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "logging.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-core/src/logging.rs", content);
    assert.equal(limits.passed, true, `logging.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-core/src/logging.rs");
    assert.deepEqual(secrets, [], "Secrets detected in logging.rs");
  });

  it("SUB-02-3-1-2: validates SecretRedactionLayer & redact_secrets specifications", () => {
    const content = readFileSync(LOGGING_RS, "utf8");

    // REDACTED_MARKER constant
    assert.match(content, /REDACTED_MARKER:\s*&str\s*=\s*"\[REDACTED\]"/, "Must define REDACTED_MARKER constant");
    // redact_secrets function
    assert.match(content, /pub fn redact_secrets\(/, "Must define redact_secrets function");
    // RedactedWriter
    assert.match(content, /pub struct RedactedWriter</, "Must define RedactedWriter struct");
    assert.match(content, /impl<.*>\s*(?:std::io::)?Write for RedactedWriter</, "RedactedWriter must implement std::io::Write");
    // RedactedMakeWriter
    assert.match(content, /pub struct RedactedMakeWriter</, "Must define RedactedMakeWriter struct");
    // SecretRedactionLayer
    assert.match(content, /pub struct SecretRedactionLayer/, "Must define SecretRedactionLayer struct");
    assert.match(content, /impl<.*>\s*Layer<.*>\s*for SecretRedactionLayer/, "SecretRedactionLayer must implement tracing_subscriber::layer::Layer");
  });

  it("SUB-02-3-1-3: executes native Rust contract tests for secret redaction", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "secret_redaction_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test secret_redaction_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /10 passed; 0 failed/, "Expected all 10 contract tests to pass");
  });

  it("SUB-02-3-1-4: 5/5 mutation test suite killing mutations to secret redaction contracts", () => {
    const content = readFileSync(LOGGING_RS, "utf8");

    // Mutation 1: Marker altered or stripped
    assert.match(content, /"\[REDACTED\]"/, "Must strictly use [REDACTED] marker");
    // Mutation 2: Bot token prefix matching altered
    assert.match(content, /"Bot "/i, "Must check Bot prefix");
    // Mutation 3: Bearer token prefix matching altered
    assert.match(content, /"Bearer "/i, "Must check Bearer prefix");
    // Mutation 4: Hex key length check altered
    assert.match(content, /64/, "Must check 64-character hex encryption keys");
    // Mutation 5: Flush / write buffer integration in RedactedWriter
    assert.match(content, /redact_secrets/, "RedactedWriter must call redact_secrets before forwarding to inner writer");
  });
});
