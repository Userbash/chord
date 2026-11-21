import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { spawnSync } from "node:child_process";

const PROJECT_ROOT = path.resolve(import.meta.dirname, "../..");
const SCRIPT_PATH = path.join(PROJECT_ROOT, "scripts/smart-typecheck.mjs");

describe("LIB-SIMPLIFY-06: smart-typecheck.mjs & Concurrency/Automation Artifacts", () => {
  it("verifies all LIB-SIMPLIFY-06 core Rust & automation artifacts exist", () => {
    const requiredFiles = [
      "rust/crates/chord-core/src/runtime.rs",
      "rust/crates/chord-core/src/shutdown.rs",
      "rust/crates/chord-core/src/logging.rs",
      "scripts/kanban-automate.mjs",
      "scripts/worker-ram-guard.sh",
      "scripts/smart-test-runner.mjs",
      "scripts/smart-typecheck.mjs",
    ];
    for (const rel of requiredFiles) {
      assert.equal(
        fs.existsSync(path.join(PROJECT_ROOT, rel)),
        true,
        `Expected LIB-SIMPLIFY-06 artifact to exist: ${rel}`,
      );
    }
  });

  it("prints usage and exits 0 on --help", () => {
    const res = spawnSync(process.execPath, [SCRIPT_PATH, "--help"], {
      cwd: PROJECT_ROOT,
      encoding: "utf8",
    });
    assert.equal(res.status, 0, `Expected exit 0 for --help, got ${res.status}: ${res.stderr}`);
    assert.match(res.stdout, /smart-typecheck\.mjs/);
  });

  it("condenses passing typecheck into a compact token-saving summary", () => {
    const res = spawnSync(process.execPath, [SCRIPT_PATH, "-p", "tsconfig.json"], {
      cwd: PROJECT_ROOT,
      encoding: "utf8",
    });
    assert.equal(res.status, 0, `Expected exit 0, got ${res.status}: ${res.stderr}`);
    assert.match(res.stdout, /SUCCESS/i);
    const lineCount = res.stdout.trim().split("\n").length;
    assert.ok(lineCount <= 10, `Expected compact output <= 10 lines, got ${lineCount}`);
  });

  it("condenses failing typecheck errors and caps excessive output", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "smart-tc-"));
    try {
      const badTs = path.join(tmpDir, "bad.ts");
      const badCfg = path.join(tmpDir, "tsconfig.json");
      const lines = [];
      for (let i = 0; i < 40; i++) {
        lines.push(`const x${i}: number = "wrong_${i}";`);
      }
      fs.writeFileSync(badTs, lines.join("\n"));
      fs.writeFileSync(
        badCfg,
        JSON.stringify({
          compilerOptions: { noEmit: true, strict: true },
          files: ["bad.ts"],
        }),
      );

      const res = spawnSync(process.execPath, [SCRIPT_PATH, "-p", badCfg], {
        cwd: PROJECT_ROOT,
        encoding: "utf8",
      });
      assert.notEqual(res.status, 0, "Expected non-zero exit code on type errors");
      const combined = res.stdout + "\n" + res.stderr;
      assert.match(combined, /FAILED/i);
      assert.match(combined, /error TS2322/);
      assert.match(combined, /обрезан|truncated/i);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
