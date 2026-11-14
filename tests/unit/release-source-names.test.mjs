// @ts-check
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));
const manifest = readFileSync(join(root, "rust/crates/chord-app/Cargo.toml"), "utf8");
const binary = /\[\[bin\]\]\s*name\s*=\s*"([^"]+)"/.exec(manifest)?.[1];

test("release dry run consumes Cargo's declared binary and preserves the published legacy filename", () => {
  assert.ok(binary, "Cargo must declare the built application binary");
  const directory = mkdtempSync(join(tmpdir(), "chord release "));
  const output = join(directory, "output");
  const target = join(directory, "custom target");
  try {
    const result = spawnSync("bash", ["scripts/release.sh", "--dry-run", "--output", output], {
      cwd: root, encoding: "utf8", env: { ...process.env, CARGO_TARGET_DIR: target },
    });
    assert.equal(result.status, 0, result.stderr || result.error?.message);
    assert.ok(result.stdout.includes(`${target}/release/${binary}`), result.stdout);
    assert.ok(result.stdout.includes(`${output}/discord-rs`), result.stdout);
    assert.ok(!result.stdout.includes(`${target}/release/discord-rs`), result.stdout);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("window probe resolves the current Tauri executable and honors explicit target overrides", () => {
  const source = readFileSync(join(root, "rust/gui-spike/Cargo.toml"), "utf8");
  const name = /\[package\][\s\S]*?\nname\s*=\s*"([^"]+)"/.exec(source)?.[1];
  assert.ok(name, "Tauri manifest must declare its implicit binary name");
  const clean = { ...process.env };
  delete clean.GUI_SPIKE_BIN;
  delete clean.CARGO_TARGET_DIR;
  const cases = [
    { env: clean, expected: join(root, "rust/gui-spike/target/debug", name) },
    { env: { ...clean, CARGO_TARGET_DIR: "/tmp/chord custom target" }, expected: `/tmp/chord custom target/debug/${name}` },
    { env: { ...clean, CARGO_TARGET_DIR: "relative target" }, expected: join(root, "relative target/debug", name) },
    { env: { ...clean, CARGO_TARGET_DIR: "/tmp/unused target", GUI_SPIKE_BIN: "/tmp/chord explicit binary" }, expected: "/tmp/chord explicit binary" },
  ];
  for (const item of cases) {
    const result = spawnSync("python3", ["-c", "import runpy; print(runpy.run_path('scripts/gui-window-probe.py')['BINARY'])"], {
      cwd: root, encoding: "utf8", env: item.env,
    });
    assert.equal(result.status, 0, result.stderr || result.error?.message);
    assert.equal(result.stdout.trim(), item.expected);
  }
});
