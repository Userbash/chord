// @ts-check
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  deduceCommitMessage,
  filterSafeFiles,
  getConfiguredRemotes,
  getCurrentBranch
} from "../../scripts/git-sync.mjs";
import { getWorkerMemoryGiB } from "../../scripts/orchestration-auto-driver.mjs";
import { syncTddPlan } from "../../scripts/sync-tdd-plan.mjs";

describe("Automation Zero-Token Scripts Contract", () => {
  it("git-sync: detects current branch without errors", () => {
    const branch = getCurrentBranch();
    assert.ok(typeof branch === "string");
    assert.ok(branch.length > 0);
  });

  it("git-sync: returns configured remotes list", () => {
    const remotes = getConfiguredRemotes();
    assert.ok(Array.isArray(remotes));
    assert.ok(remotes.includes("origin") || remotes.includes("public"));
  });

  it("git-sync: deduceCommitMessage infers conventional commits", () => {
    const msgMedia = deduceCommitMessage(["rust/crates/chord-media/src/audio.rs"]);
    assert.match(msgMedia, /^feat\(media\):/);

    const msgVoice = deduceCommitMessage(["rust/crates/chord-voice/src/lib.rs"]);
    assert.match(msgVoice, /^feat\(voice\):/);

    const msgKanban = deduceCommitMessage(["waves/kanban-board.json"]);
    assert.match(msgKanban, /^docs\(kanban\):/);

    const msgTest = deduceCommitMessage(["tests/unit/some.test.mjs"]);
    assert.match(msgTest, /^test\(contract\):/);

    const msgScript = deduceCommitMessage(["scripts/some-script.mjs"]);
    assert.match(msgScript, /^chore\(scripts\):/);
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

  it("sync-tdd-plan: check mode verifies all tests are registered", () => {
    const ok = syncTddPlan({ fix: false });
    assert.equal(ok, true, "TDD plan must be in sync with tests on disk");
  });
});
