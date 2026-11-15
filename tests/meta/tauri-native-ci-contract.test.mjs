// @ts-check
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const workflow = () => readFileSync(new URL("../../.github/workflows/tauri-native.yml", import.meta.url), "utf8");
const targets = [
  ["ubuntu-24.04", "x86_64-unknown-linux-gnu"],
  ["ubuntu-24.04-arm", "aarch64-unknown-linux-gnu"],
  ["windows-2025", "x86_64-pc-windows-msvc"],
  ["windows-11-arm", "aarch64-pc-windows-msvc"],
  ["macos-15-intel", "x86_64-apple-darwin"],
  ["macos-15", "aarch64-apple-darwin"],
];

describe("Tauri native compilation contract", () => {
  it("requires the six OS/architecture combinations on matching native runners", () => {
    const text = workflow();
    for (const [runner, target] of targets) {
      assert.ok(text.includes(`- runner: ${runner}\n            target: ${target}\n`), `${target} requires ${runner}`);
    }
    assert.match(text, /runs-on: \$\{\{ matrix\.runner \}\}/);
    assert.match(text, /fail-fast: false/);
    assert.doesNotMatch(text, /continue-on-error|\|\| true/);
  });

  it("builds the actual Tauri release binary with embedded frontend and locked dependencies", () => {
    const text = workflow();
    assert.match(text, /cargo build --locked --release --manifest-path rust\/gui-spike\/Cargo\.toml/);
    assert.match(text, /--target "\$\{\{ matrix\.target \}\}" --features tauri\/custom-protocol/);
    assert.doesNotMatch(text, /cargo (?:check|metadata).*#.*build/);
    assert.match(text, /cargo test --locked --manifest-path rust\/gui-spike\/Cargo\.toml/);
  });

  it("installs platform libraries for Tauri and its shared native dependencies", () => {
    const text = workflow();
    for (const library of ["libwebkit2gtk-4.1-dev", "libayatana-appindicator3-dev", "libhunspell-dev", "libsecret-1-dev", "libasound2-dev"]) {
      assert.ok(text.includes(library), `Linux requires ${library}`);
    }
    assert.match(text, /brew install .*hunspell.*sqlite.*glib.*libsecret/);
    assert.match(text, /vcpkg\.exe.*install.*sqlite3:\$triplet.*hunspell:\$triplet/);
    assert.match(text, /triplet: x64-windows/);
    assert.match(text, /triplet: arm64-windows/);
  });

  it("preserves diagnostics and build artifacts after an independent test failure", () => {
    const text = workflow();
    assert.match(text, /id: tauri_build/);
    assert.match(text, /if:.*!cancelled\(\).*steps\.tauri_build\.outcome == 'success'/);
    assert.match(text, /actions\/upload-artifact@/);
    assert.match(text, /if-no-files-found: error/);
    assert.match(text, /SHA256/);
    assert.match(text, /Cargo\.lock/);
    assert.match(text, /git.*rev-parse.*HEAD/);
    assert.match(text, /process\.arch/);
  });

  it("keeps release compilation separate from native GUI and instruction-set qualification", () => {
    const text = workflow();
    assert.match(text, /pull_request:/);
    assert.match(text, /workflow_dispatch:/);
    assert.match(text, /contents: read/);
    assert.match(text, /persist-credentials: false/);
    assert.doesNotMatch(text, /pull_request_target:|secrets\.|target-cpu=native|-march=native|-mcpu=native/);
    assert.doesNotMatch(text, /target-feature=\+(?:avx|aes|sha|lse|sve)/);
    assert.match(text, /native_runtime_qualified: false/);
  });

  it("uploads only a completely staged artifact even if independent contracts failed", () => {
    const text = workflow();
    const stage = text.indexOf("- name: Stage compiled binary and provenance\n");
    const upload = text.indexOf("- name: Upload Tauri build artifact\n");
    assert.ok(stage > 0 && upload > stage);
    assert.match(text.slice(stage, upload), /id: tauri_artifact/);
    assert.match(text.slice(upload), /if:.*!cancelled\(\).*steps\.tauri_artifact\.outcome == 'success'/);
  });
});
