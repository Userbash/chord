// @ts-check
/**
 * T-197 (UI guild folders): контракт чистой логики папок серверов.
 *
 * Проверяет: подсчёт серверов в папке, агрегацию непрочитанного
 * (`unread === true` или `mentionCount > 0`), сумму упоминаний только по
 * положительным целым, текст бейджа (`''`, `String(n)`, `99+`), флаг
 * активного голоса и отсутствие мутации входа при переключении папки.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  computeGuildFolderState,
  toggleFolderCollapsed,
} from "../../rust/gui-spike/ui/features/guild-folders.js";

describe("computeGuildFolderState", () => {
  it("returns zeroed state for an empty folder", () => {
    assert.deepEqual(computeGuildFolderState([], {}), {
      guildCount: 0,
      hasUnread: false,
      mentionCount: 0,
      badgeText: "",
      hasActiveVoice: false,
    });
  });

  it("counts guilds in the folder regardless of their state", () => {
    /** @type {Record<string, { unread?: boolean, mentionCount?: number, hasActiveVoice?: boolean }>} */
    const states = { "1": {}, "2": { unread: false }, "3": {} };
    assert.equal(computeGuildFolderState(["1", "2", "3"], states).guildCount, 3);
  });

  it("treats unread === true as unread even without mentions", () => {
    const state = computeGuildFolderState(["1", "2"], { "2": { unread: true } });
    assert.equal(state.hasUnread, true);
    assert.equal(state.mentionCount, 0);
    assert.equal(state.badgeText, "");
  });

  it("treats mentionCount > 0 as unread", () => {
    const state = computeGuildFolderState(["1"], { "1": { mentionCount: 3 } });
    assert.equal(state.hasUnread, true);
  });

  it("ignores guilds that are not in the folder", () => {
    const state = computeGuildFolderState(["1"], {
      "1": {},
      "2": { unread: true, mentionCount: 50, hasActiveVoice: true },
    });
    assert.equal(state.hasUnread, false);
    assert.equal(state.mentionCount, 0);
    assert.equal(state.hasActiveVoice, false);
  });

  it("sums positive integer mentions and skips invalid values", () => {
    const state = computeGuildFolderState(["1", "2", "3", "4"], {
      "1": { mentionCount: 2 },
      "2": { mentionCount: 5 },
      "3": { mentionCount: -4 },
      "4": { mentionCount: 1.5 },
    });
    assert.equal(state.mentionCount, 7);
    assert.equal(state.badgeText, "7");
  });

  it("ignores missing guild ids in the state map", () => {
    const state = computeGuildFolderState(["missing"], {});
    assert.equal(state.guildCount, 1);
    assert.equal(state.hasUnread, false);
  });

  it("formats the badge: empty, plain number, then 99+", () => {
    assert.equal(computeGuildFolderState(["1"], { "1": { mentionCount: 0 } }).badgeText, "");
    assert.equal(computeGuildFolderState(["1"], { "1": { mentionCount: 99 } }).badgeText, "99");
    assert.equal(computeGuildFolderState(["1"], { "1": { mentionCount: 100 } }).badgeText, "99+");
  });

  it("caps only the text, not the numeric mention total", () => {
    const state = computeGuildFolderState(["1", "2"], {
      "1": { mentionCount: 60 },
      "2": { mentionCount: 60 },
    });
    assert.equal(state.mentionCount, 120);
    assert.equal(state.badgeText, "99+");
  });

  it("reports active voice when any guild in the folder has it", () => {
    const state = computeGuildFolderState(["1", "2"], { "2": { hasActiveVoice: true } });
    assert.equal(state.hasActiveVoice, true);
  });
});

describe("toggleFolderCollapsed", () => {
  it("appends a folder that is not collapsed yet", () => {
    assert.deepEqual(toggleFolderCollapsed(["a"], "b"), ["a", "b"]);
  });

  it("removes a folder that is already collapsed", () => {
    assert.deepEqual(toggleFolderCollapsed(["a", "b", "c"], "b"), ["a", "c"]);
  });

  it("does not mutate the input array", () => {
    const input = ["a"];
    const next = toggleFolderCollapsed(input, "a");
    assert.deepEqual(input, ["a"]);
    assert.notEqual(next, input);
  });

  it("does not mutate the input array on the append path", () => {
    const input = ["a"];
    const next = toggleFolderCollapsed(input, "b");
    assert.deepEqual(input, ["a"]);
    assert.notEqual(next, input);
    assert.deepEqual(next, ["a", "b"]);
  });

  it("returns an empty array for an empty input", () => {
    assert.deepEqual(toggleFolderCollapsed([], "x"), ["x"]);
  });
});
