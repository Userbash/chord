// @ts-check
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const workflow = () => readFileSync(new URL("../../.github/workflows/cross-platform.yml", import.meta.url), "utf8");

describe("native cross-platform CI contract", () => {
  it("requires native Linux, Windows and macOS jobs without allowing failures", () => {
    const text = workflow();
    for (const runner of ["ubuntu-24.04", "windows-2025", "macos-15"]) assert.ok(text.includes(`runner: ${runner}`), `${runner} is required`);
    assert.match(text, /runs-on: \$\{\{ matrix\.runner \}\}/);
    assert.match(text, /fail-fast: false/);
    assert.doesNotMatch(text, /continue-on-error|if:.*false|\|\| true/);
    assert.match(text, /pull_request:/);
    assert.match(text, /workflow_dispatch:/);
    assert.match(text, /contents: read/);
  });

  it("runs core and GUI independently on every native runner", () => {
    const text = workflow();
    assert.match(text, /suite: \[core, gui\]/);
    assert.match(text, /runner: \[ubuntu-24\.04, windows-2025, macos-15\]/);
    assert.match(text, /if: matrix\.suite == 'core'/);
    assert.match(text, /if: matrix\.suite == 'gui'/);
  });

  it("verifies the pinned GUI dependency before building both separate manifests", () => {
    const text = workflow();
    const bootstrap = text.indexOf("node scripts/bootstrap-gpui.mjs\n");
    const check = text.indexOf("node scripts/bootstrap-gpui.mjs --check");
    const core = text.indexOf("cargo test --locked --workspace --manifest-path rust/Cargo.toml");
    const gui = text.indexOf("cargo test --locked --manifest-path rust/chord-gpui/Cargo.toml");
    assert.ok(bootstrap >= 0 && check > bootstrap && core > check && gui > core, "bootstrap, verification, core and GUI tests must run in order");
    assert.match(text, /cargo build --locked --manifest-path rust\/chord-gpui\/Cargo.toml/);
    assert.doesNotMatch(text, /rust\/discord-gpui|--exclude.*chord-gpui/);
    assert.doesNotMatch(text, /DISCORD_LAB_LIVE_CDP|secrets\./);
  });

  it("installs real system libraries on each platform", () => {
    const text = workflow();
    for (const name of ["libsqlite3-dev", "libhunspell-dev", "libsecret-1-dev", "libwebkit2gtk-4.1-dev", "libasound2-dev", "libxkbcommon-x11-dev"]) assert.ok(text.includes(name), `Linux requires ${name}`);
    assert.match(text, /brew install .*hunspell.*sqlite.*glib.*libsecret/);
    assert.match(text, /vcpkg\.exe.*install.*sqlite3:x64-windows.*hunspell:x64-windows.*glib:x64-windows/);
    assert.match(text, /GITHUB_ENV/);
    assert.match(text, /GITHUB_PATH/);
    assert.match(text, /xcodebuild -version/);
    assert.match(text, /--headless-smoke/);
  });

  it("checks formatting and lint for the GUI as well as the core workspace", () => {
    const text = workflow();
    for (const manifest of ["rust/Cargo.toml", "rust/chord-gpui/Cargo.toml"]) {
      assert.ok(text.includes(`cargo fmt --manifest-path ${manifest} --check`), `${manifest} needs format verification`);
      assert.ok(text.includes(`cargo clippy --locked --manifest-path ${manifest}`), `${manifest} needs lint verification`);
    }
    assert.match(text, /--all-targets -- -D warnings/);
    assert.match(text, /node-version: "24"/);
  });

  it("keeps native build and test results visible after a lint failure", () => {
    const text = workflow();
    for (const step of ["Core lint", "Core tests", "GUI lint", "GUI build", "GUI tests", "GUI headless smoke"]) {
      const start = text.indexOf(`- name: ${step}\n`);
      assert.ok(start > 0, `${step} must exist`);
      const end = text.indexOf("\n      - name:", start + 1);
      const body = text.slice(start, end < 0 ? text.length : end);
      assert.match(body, /if:.*!cancelled\(\)/, `${step} must still run when an earlier quality check failed`);
    }
    assert.match(text, /id: bootstrap_gpui/);
    assert.match(text, /steps\.bootstrap_gpui\.outcome == 'success'/);
  });

  it("documents the GUI's separate workspace and pinned bootstrap before building", () => {
    const readme = readFileSync(new URL("../../README.md", import.meta.url), "utf8");
    const bootstrap = readme.indexOf("node scripts/bootstrap-gpui.mjs");
    const build = readme.indexOf("cargo build --locked --release --manifest-path rust/chord-gpui/Cargo.toml");
    assert.ok(bootstrap >= 0 && build > bootstrap, "quick start must bootstrap GPUI and build its actual separate manifest");
    assert.ok(readme.includes("./rust/chord-gpui/target/release/chord-gpui"));
    assert.doesNotMatch(readme, /rust\/Cargo\.toml -p chord-gpui/);
  });
});
