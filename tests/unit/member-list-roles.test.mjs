// @ts-nocheck
/**
 * W53-04: Guild Member List & Role Groups (`member-list.js`).
 *
 * Проверяет:
 * - группировку участников по hoisted-ролям (по убыванию position), «В сети» и «Не в сети»;
 * - плоское представление (flattenMemberGroups) для виртуализованного списка;
 * - рендеринг статус-точек (online, idle, dnd, offline), бейджа владельца и customStatus;
 * - виртуализацию (scrollTo / scrollToIndex) и переключение видимости (toggleVisibility).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  DEFAULT_GROUP_LABELS_RU,
  createMemberListController,
  flattenMemberGroups,
  groupMembersByHoistedRole,
} from "../../rust/gui-spike/ui/features/member-list.js";

const ROLES = [
  { id: "r-admin", name: "Admins", position: 10, hoist: true, color: 0xe74c3c },
  { id: "r-mod", name: "Mods", position: 5, hoist: true, color: 0x3498db },
  { id: "r-helper", name: "Helpers", position: 2, hoist: false, color: 0x2ecc71 },
];

const MEMBERS = [
  {
    userId: "1",
    username: "alice",
    displayName: "Alice",
    status: "online",
    roleIds: ["r-admin", "r-mod"],
    customStatus: "Building Chord",
    isOwner: true,
  },
  {
    userId: "2",
    username: "bob",
    displayName: "Bob",
    status: "idle",
    roleIds: ["r-mod"],
  },
  {
    userId: "3",
    username: "charlie",
    displayName: "Charlie",
    status: "dnd",
    roleIds: ["r-helper"],
    customStatus: "In a meeting",
  },
  {
    userId: "4",
    username: "dave",
    displayName: "Dave",
    status: "offline",
    roleIds: ["r-admin"],
  },
];

describe("W53-04: Guild Member List & Role Groups (member-list-roles)", () => {
  it("группирует участников по hoisted-ролям, В сети и Не в сети", () => {
    const groups = groupMembersByHoistedRole(MEMBERS, ROLES);

    assert.equal(groups.length, 4);
    assert.equal(groups[0].label, "Admins");
    assert.equal(groups[0].count, 1);
    assert.equal(groups[0].members[0].userId, "1");

    assert.equal(groups[1].label, "Mods");
    assert.equal(groups[1].count, 1);
    assert.equal(groups[1].members[0].userId, "2");

    assert.equal(groups[2].label, DEFAULT_GROUP_LABELS_RU.online);
    assert.equal(groups[2].count, 1);
    assert.equal(groups[2].members[0].userId, "3");

    assert.equal(groups[3].label, DEFAULT_GROUP_LABELS_RU.offline);
    assert.equal(groups[3].count, 1);
    assert.equal(groups[3].members[0].userId, "4");
  });

  it("корректно обрабатывает пустой список участников", () => {
    const groups = groupMembersByHoistedRole([], ROLES);
    assert.deepEqual(groups, []);
  });

  it("разворачивает группы в плоский список заголовков и строк участников", () => {
    const groups = groupMembersByHoistedRole(MEMBERS, ROLES);
    const flat = flattenMemberGroups(groups);

    // 4 группы + 4 участника = 8 элементов
    assert.equal(flat.length, 8);
    assert.equal(flat[0].type, "header");
    assert.equal(flat[1].type, "member");
    if (flat[1].type === "member") {
      assert.equal(flat[1].member.userId, "1");
    }
  });

  it("рендерит статус-точки (online, idle, dnd, offline), бейдж владельца и customStatus", () => {
    const doc = new ShimDocument();
    const container = /** @type {any} */ (doc.createElement("aside"));
    const controller = createMemberListController({
      container,
      viewportHeight: 600,
    });

    controller.render({ members: MEMBERS, roles: ROLES });

    /** @param {any} node @param {any[]} out */
    function walk(node, out = []) {
      if (!node) return out;
      out.push(node);
      if (Array.isArray(node.children)) {
        for (const child of node.children) walk(child, out);
      }
      return out;
    }

    const nodes = walk(container);
    const badges = nodes
      .map((n) => n.getAttribute?.("data-presence-badge"))
      .filter(Boolean);
    assert.ok(badges.includes("online"));
    assert.ok(badges.includes("idle"));
    assert.ok(badges.includes("dnd"));
    assert.ok(badges.includes("offline"));
    assert.ok(nodes.some((n) => n.hasAttribute?.("data-owner-badge")));
    const texts = nodes.map((n) => n.textContent ?? "").join(" ");
    assert.ok(texts.includes("Building Chord"));
    assert.ok(texts.includes("In a meeting"));
  });

  it("поддерживает виртуализацию (scrollTo, scrollToIndex) и toggleVisibility", () => {
    const doc = new ShimDocument();
    const container = /** @type {any} */ (doc.createElement("aside"));
    const controller = createMemberListController({
      container,
      viewportHeight: 200,
    });

    controller.render({ members: MEMBERS, roles: ROLES });
    controller.scrollTo(40, 200);
    controller.scrollToIndex(2);

    const visibleAfterHide = controller.toggleVisibility();
    assert.equal(visibleAfterHide, false);
    const visibleAfterShow = controller.toggleVisibility();
    assert.equal(visibleAfterShow, true);
  });
});
