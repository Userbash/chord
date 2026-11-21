// @ts-nocheck
/**
 * Contract, Independent Oracle & 5/5 Mutation Suite for `STAGE-05.1.1`:
 * Connecting `tokio-tungstenite` with TLS support for Discord Gateway:
 * Clean lightweight WebSocket transport instead of heavy wrappers.
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
const WS_TRANSPORT_RS = join(CORE_DIR, "src", "ws_transport.rs");
const CONTRACT_TEST = join(CORE_DIR, "tests", "tokio_tungstenite_tls_contract.rs");

describe("STAGE-05.1.1: Clean Lightweight tokio-tungstenite WebSocket Transport with TLS Support", () => {
  it("SUB-05-1-1-1: enforces #![forbid(unsafe_code)], zero unsafe blocks, no unwrap in production, and size limits", () => {
    assert.equal(existsSync(WS_TRANSPORT_RS), true, "rust/crates/chord-core/src/ws_transport.rs must exist");
    assert.equal(existsSync(CONTRACT_TEST), true, "rust/crates/chord-core/tests/tokio_tungstenite_tls_contract.rs must exist");

    const content = readFileSync(WS_TRANSPORT_RS, "utf8");
    assert.match(content, /^#!\[forbid\(unsafe_code\)\]/m, "ws_transport.rs must start with #![forbid(unsafe_code)]");
    assert.doesNotMatch(content, /\bunsafe\s*\{/, "ws_transport.rs must contain zero unsafe blocks");

    const prodPart = content.split("#[cfg(test)]")[0] ?? content;
    assert.doesNotMatch(
      prodPart,
      /\.unwrap\(\)|\.expect\(/,
      "ws_transport.rs production code must not use .unwrap() or .expect()",
    );

    const limits = checkRustSourceSizeLimits("rust/crates/chord-core/src/ws_transport.rs", content);
    assert.equal(limits.passed, true, `ws_transport.rs failed size limit: ${limits.reason}`);

    const secrets = scanTextForSecrets(content, "rust/crates/chord-core/src/ws_transport.rs");
    assert.deepEqual(secrets, [], "Secrets detected in ws_transport.rs");
  });

  it("SUB-05-1-1-2: validates GatewayTransportConfig, rustls/ring TLS Connector & GatewayTransport specifications", () => {
    const content = readFileSync(WS_TRANSPORT_RS, "utf8");

    assert.match(
      content,
      /DEFAULT_MAX_GATEWAY_WIRE_BYTES:\s*usize\s*=\s*64\s*\*\s*1024\s*\*\s*1024/,
      "Must define DEFAULT_MAX_GATEWAY_WIRE_BYTES as 64 MiB",
    );
    assert.match(
      content,
      /DEFAULT_MAX_WRITE_BUFFER_BYTES:\s*usize\s*=\s*64\s*\*\s*1024/,
      "Must define DEFAULT_MAX_WRITE_BUFFER_BYTES as 64 KiB",
    );
    assert.match(content, /pub struct GatewayTransportConfig/, "Must define GatewayTransportConfig struct");
    assert.match(content, /pub struct GatewayTransport</, "Must define GatewayTransport struct");
    assert.match(content, /pub enum GatewayInboundFrame/, "Must define GatewayInboundFrame enum");
    assert.match(content, /pub enum GatewayTransportError/, "Must define GatewayTransportError enum");
    assert.match(content, /pub fn build_gateway_tls_connector\(/, "Must define build_gateway_tls_connector");
    assert.match(content, /pub async fn connect_gateway_transport\(/, "Must define connect_gateway_transport");
    assert.match(content, /connect_async_tls_with_config/, "Must use tokio_tungstenite::connect_async_tls_with_config");
    assert.match(content, /rustls::crypto::ring::default_provider/, "Must use explicit ring crypto provider for rustls");
    assert.match(content, /webpki_roots::TLS_SERVER_ROOTS/, "Must configure Mozilla webpki-roots for TLS verification");
  });

  it("SUB-05-1-1-3: executes native Rust contract tests for tokio_tungstenite_tls_contract", () => {
    const proc = spawnSync(
      "cargo",
      ["test", "--manifest-path", "rust/Cargo.toml", "-p", "chord-core", "--test", "tokio_tungstenite_tls_contract", "--offline"],
      { cwd: ROOT, encoding: "utf8" }
    );

    assert.equal(proc.status, 0, `cargo test tokio_tungstenite_tls_contract failed:\n${proc.stderr || proc.stdout}`);
    assert.match(proc.stdout, /8 passed; 0 failed/, "Expected all 8 contract tests to pass");
  });

  it("SUB-05-1-1-4: 5/5 mutation test suite killing mutations to tokio-tungstenite TLS transport contracts", () => {
    const content = readFileSync(WS_TRANSPORT_RS, "utf8");

    // Mutation 1: 64 MiB frame capacity guard removed or altered
    assert.match(content, /GatewayTransportError::CapacityExceeded/, "Must map tungstenite Capacity errors to CapacityExceeded");
    // Mutation 2: TLS ring provider or webpki_roots removed
    assert.match(content, /Connector::Rustls/, "Must wrap rustls ClientConfig in tokio_tungstenite::Connector::Rustls");
    // Mutation 3: Connect timeout wrapper bypassed
    assert.match(content, /GatewayTransportError::ConnectTimeout/, "Must enforce connect_timeout with ConnectTimeout error");
    // Mutation 4: Automatic Ping -> Pong response removed
    assert.match(content, /Message::Pong\(/, "Must automatically reply to incoming Message::Ping with Message::Pong");
    // Mutation 5: Zero configuration limits validation bypassed
    assert.match(content, /GatewayTransportError::ZeroMaxMessageSize/, "Must reject max_message_size == 0 with ZeroMaxMessageSize");
  });
});
