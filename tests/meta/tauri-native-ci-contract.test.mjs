// @ts-check
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
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
  it("keeps Rust source and lock bytes canonical when Git enables Windows CRLF conversion", () => {
    const directory = mkdtempSync(join(tmpdir(), "chord-rust-eol-"));
    const emptyAttributes = join(directory, "empty-global-attributes");
    writeFileSync(emptyAttributes, "");
    const environment = Object.fromEntries(Object.entries(process.env)
      .filter(([name]) => !name.toUpperCase().startsWith("GIT_")));
    /** @param {...string} args */
    const git = (...args) => execFileSync("git", ["-c", `core.attributesFile=${emptyAttributes}`,
      "-c", "core.autocrlf=true", "-c", "core.safecrlf=false", ...args], {
      cwd: directory, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
      env: { ...environment, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_SYSTEM: emptyAttributes,
        GIT_CONFIG_GLOBAL: emptyAttributes, GIT_ATTR_NOSYSTEM: "1" },
    });
    try {
      git("init", "--quiet");
      const source = readFileSync(new URL("../../rust/gui-spike/src/main.rs", import.meta.url), "utf8").replaceAll("\r\n", "\n");
      writeFileSync(join(directory, ".gitattributes"), readFileSync(new URL("../../.gitattributes", import.meta.url)));
      writeFileSync(join(directory, "main.rs"), source);
      const lock = readFileSync(new URL("../../rust/gui-spike/Cargo.lock", import.meta.url), "utf8").replaceAll("\r\n", "\n");
      writeFileSync(join(directory, "Cargo.lock"), lock);
      writeFileSync(join(directory, "notes.txt"), "unrelated file\n");
      git("add", "--", ".gitattributes", "main.rs", "Cargo.lock", "notes.txt");
      rmSync(join(directory, "main.rs"));
      rmSync(join(directory, "Cargo.lock"));
      rmSync(join(directory, "notes.txt"));
      git("checkout", "--", "main.rs", "Cargo.lock", "notes.txt");
      const checkedOut = readFileSync(join(directory, "main.rs"), "utf8");
      assert.ok(!checkedOut.includes("\r"), "Rust source contracts require canonical LF bytes on Windows checkouts");
      assert.ok(checkedOut === source, "Git must preserve the complete canonical Rust source");
      const checkedOutLock = readFileSync(join(directory, "Cargo.lock"), "utf8");
      assert.ok(!checkedOutLock.includes("\r"), "Dependency provenance requires canonical LF lock bytes on Windows");
      assert.ok(checkedOutLock === lock, "Git must preserve every canonical lock byte");
      assert.equal(readFileSync(join(directory, "notes.txt"), "utf8"), "unrelated file\r\n");
    } finally {
      assert.equal(dirname(resolve(directory)), resolve(tmpdir()));
      assert.ok(basename(directory).startsWith("chord-rust-eol-"));
      rmSync(directory, { recursive: true, force: true });
    }
  });

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

  it("persists the real vcpkg root, target triplet and dynamic-link selection for Cargo", () => {
    const text = workflow();
    for (const assignment of ["VCPKG_ROOT=$env:VCPKG_INSTALLATION_ROOT", "VCPKGRS_TRIPLET=$triplet", "VCPKGRS_DYNAMIC=1"]) {
      assert.ok(text.includes(`"${assignment}" | Out-File -FilePath $env:GITHUB_ENV`), `${assignment} must reach later Cargo steps`);
    }
  });

  it("executes native platform contracts including the real Windows credential backend", () => {
    const text = workflow();
    assert.match(text, /cargo test --locked --manifest-path rust\/Cargo\.toml -p chord-platform/);
    assert.match(text, /--target "\$\{\{ matrix\.target \}\}" --lib --tests/);
  });

  it("retains bundled DLL copyright files and hashes portable nested artifact paths", () => {
    const text = workflow();
    assert.match(text, /copyright/);
    assert.match(text, /licenses/);
    assert.match(text, /readdirSync\(directory, \{ recursive: true \}\)/);
    assert.match(text, /name\.split\(sep\)\.join\('\/'\)/);
  });
});
