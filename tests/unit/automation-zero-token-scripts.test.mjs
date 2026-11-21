// @ts-check
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { filterSafeFiles, getCurrentBranch } from "../../scripts/git-sync.mjs";
import { getWorkerMemoryGiB } from "../../scripts/orchestration-auto-driver.mjs";

describe("Automation Zero-Token Scripts Contract", () => {
  it("git-sync: detects current branch without errors", () => {
    const branch = getCurrentBranch();
    assert.ok(typeof branch === "string");
    assert.ok(branch.length > 0);
  });

  it("git-sync: filterSafeFiles protects branch tauri from rust/chord-gpui/*", () => {
    const sampleFiles = [
      "src/core.rs",
      "rust/chord-gpui/src/main.rs",
      "rust/crates/chord-platform/src/lib.rs",
      "rust/chord-gpui/tests/test.rs",
      "waves/kanban-board.json",
    ];

    const safe = filterSafeFiles(sampleFiles);
    // When branch is tauri, gpui files should be stripped
    if (getCurrentBranch() === "tauri") {
      assert.ok(!safe.includes("rust/chord-gpui/src/main.rs"));
      assert.ok(!safe.includes("rust/chord-gpui/tests/test.rs"));
      assert.ok(safe.includes("src/core.rs"));
      assert.ok(safe.includes("rust/crates/chord-platform/src/lib.rs"));
      assert.ok(safe.includes("waves/kanban-board.json"));
    } else {
      assert.equal(safe.length, sampleFiles.length);
    }
  });

  it("auto-driver: reports non-negative worker memory", () => {
    const mem = getWorkerMemoryGiB();
    assert.ok(typeof mem === "number");
    assert.ok(mem >= 0);
    assert.ok(mem < 50.0);
  });
});
