// @ts-check
import assert from "node:assert/strict";
import test from "node:test";

import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  PRESENCE_STATUS_LABELS,
  createMemberListController,
  groupMembersByHoistedRole,
  isFloatingUiDomPowered as memberListFloatingPowered,
  isTanstackVirtualPowered as memberListVirtualPowered,
} from "../../rust/gui-spike/ui/features/member-list.js";
import {
  STATUS_LABELS,
  isFloatingUiDomPowered as membersFloatingPowered,
  isTanstackVirtualPowered as membersVirtualPowered,
  renderMemberRow,
  renderMembersList,
} from "../../rust/gui-spike/ui/features/members.js";

/**
 * Generates N synthetic guild members across hoisted roles and online/offline buckets.
 * @param {number} count
 */
function generateSyntheticMembers(count) {
  const statuses = ["online", "idle", "dnd", "mobile", "streaming", "offline"];
  const roles = [
    { id: "r-admin", name: "Администраторы", position: 30, hoist: true, color: 0xe74c3c },
    { id: "r-mod", name: "Модераторы", position: 20, hoist: true, color: 0x3498db },
    { id: "r-vip", name: "VIP", position: 10, hoist: false, color: 0x9b59b6 },
  ];
  /** @type {Array<any>} */
  const members = new Array(count);
  for (let i = 0; i < count; i++) {
    const status = statuses[i % statuses.length];
    const roleIds =
      i % 20 === 0
        ? ["r-admin"]
        : i % 7 === 0
          ? ["r-mod"]
          : i % 3 === 0
            ? ["r-vip"]
            : [];
    members[i] = {
      userId: `u-${i}`,
      username: `user_${i}`,
      displayName: `User ${String(i).padStart(5, "0")}`,
      status,
      roleIds,
      isOwner: i === 0,
      customStatus: i % 5 === 0 ? `Activity #${i}` : undefined,
    };
  }
  return { members, roles };
}

test("W04-1: member-list.js virtualizes 25,000+ grouped members with @tanstack/virtual-core", () => {
  assert.equal(memberListVirtualPowered, true, "member-list.js must be powered by @tanstack/virtual-core");

  const doc = /** @type {any} */ (new ShimDocument());
  const container = doc.createElement("div");
  const { members, roles } = generateSyntheticMembers(25_200);

  const controller = createMemberListController({
    container,
    members,
    roles,
    viewportHeight: 600,
    overscan: 6,
  });

  const listEl = controller.root;
  assert.ok(listEl, "controller.root must exist");
  const sizer = listEl.querySelector("[data-member-list-sizer]");
  assert.ok(sizer, "virtualized member list must render a [data-member-list-sizer]");

  // Total flat items = 25,200 members + 4 role/status headers = 25,204
  assert.equal(controller.getFlatItems().length, 25_204);
  assert.ok(controller.getTotalSize() > 25_000 * 30, "total virtualized height must reflect 25k+ rows");

  // Only visible window + overscan should be in the DOM (<< 25,200)
  const initialHeaders = listEl.querySelectorAll("[data-member-group-header]");
  const initialItems = listEl.querySelectorAll("[data-member-item]");
  assert.ok(initialHeaders.length >= 1, "first group header must be visible at scrollTop=0");
  assert.ok(initialItems.length > 0 && initialItems.length < 60, `expected < 60 DOM items, got ${initialItems.length}`);

  // Scroll deep into the 25k+ list (e.g., halfway down)
  const midOffset = Math.floor(controller.getTotalSize() / 2);
  controller.scrollTo(midOffset, 600);

  const midItems = listEl.querySelectorAll("[data-member-item]");
  assert.ok(midItems.length > 0 && midItems.length < 60, `expected < 60 DOM items at mid scroll, got ${midItems.length}`);
  const firstMidIndex = Number(midItems[0]?.getAttribute("data-index"));
  assert.ok(firstMidIndex > 5_000, `expected scrolled data-index > 5000, got ${firstMidIndex}`);
});

test("W04-2: presence status badges support online, idle, dnd, offline, mobile, streaming", () => {
  const allStatuses = /** @type {const} */ (["online", "idle", "dnd", "offline", "mobile", "streaming"]);
  for (const st of allStatuses) {
    assert.ok(PRESENCE_STATUS_LABELS[st], `member-list.js must define label for status "${st}"`);
    assert.ok(STATUS_LABELS[st], `members.js must define label for status "${st}"`);
  }

  const members = allStatuses.map((status, idx) => ({
    userId: `u-${status}`,
    username: `user_${status}`,
    displayName: `User ${idx}`,
    status,
    roleIds: status === "mobile" || status === "streaming" ? ["r-hoist"] : [],
  }));
  const roles = [{ id: "r-hoist", name: "Стримеры", position: 10, hoist: true, color: 0x9146ff }];

  const groups = groupMembersByHoistedRole(members, roles);
  const hoistGroup = groups.find((g) => g.id === "r-hoist");
  const offlineGroup = groups.find((g) => g.id === "offline");
  assert.ok(hoistGroup, "mobile and streaming members with hoisted role should appear in hoisted group");
  assert.deepEqual(
    hoistGroup.members.map((m) => m.userId).sort(),
    ["u-mobile", "u-streaming"],
  );
  assert.equal(offlineGroup?.members.length, 1);
  assert.equal(offlineGroup?.members[0]?.userId, "u-offline");

  // Verify DOM rendering of mobile & streaming badges in member-list.js
  const doc = /** @type {any} */ (new ShimDocument());
  const container = doc.createElement("div");
  const controller = createMemberListController({ container, members, roles });
  for (const st of allStatuses) {
    const item = controller.root.querySelector(`[data-user-id="u-${st}"]`);
    assert.ok(item, `item for status "${st}" must be rendered`);
    const badge = item.querySelector("[data-status]");
    assert.equal(badge?.getAttribute("data-status"), st);
  }

  // Verify DOM rendering in members.js renderMemberRow
  const prevDoc = globalThis.document;
  globalThis.document = doc;
  try {
    const row = doc.createElement("div");
    renderMemberRow(
      /** @type {any} */ (row),
      {
        name: "Streamer",
        status: "streaming",
        statusLabel: STATUS_LABELS.streaming ?? "стримит",
        role: "MOD",
        badge: "MOD",
        activity: "Стримит игру",
        hue: 120,
      },
      0,
    );
    assert.equal(row.getAttribute("data-status"), "streaming");
    assert.equal(row.querySelector(".status-dot")?.getAttribute("data-status"), "streaming");
  } finally {
    globalThis.document = prevDoc;
  }
});

test("W04-3: @floating-ui/dom context menu & hover card triggers on member items", async () => {
  assert.equal(memberListFloatingPowered, true, "member-list.js must be powered by @floating-ui/dom");
  assert.equal(membersFloatingPowered, true, "members.js must be powered by @floating-ui/dom");

  const doc = /** @type {any} */ (new ShimDocument());
  const container = doc.createElement("div");
  /** @type {Array<any>} */
  const contextEvents = [];
  /** @type {Array<any>} */
  const hoverEvents = [];
  /** @type {Array<any>} */
  const hoverEndEvents = [];

  const controller = createMemberListController({
    container,
    members: [
      { userId: "u-1", username: "alice", displayName: "Alice", status: "online", roleIds: [] },
    ],
    roles: [],
    onMemberContextMenu: (payload) => contextEvents.push(payload),
    onMemberHover: (payload) => hoverEvents.push(payload),
    onMemberHoverEnd: (payload) => hoverEndEvents.push(payload),
  });

  const item = controller.root.querySelector('[data-user-id="u-1"]');
  assert.ok(item, "member item must exist");
  assert.equal(item.getAttribute("data-context-menu-trigger"), "member");
  assert.equal(item.getAttribute("data-hover-card-trigger"), "member");

  item.getBoundingClientRect = () => ({ left: 1020, top: 140, right: 1240, bottom: 182, width: 220, height: 42 });

  // Trigger contextmenu
  item.dispatchEvent("contextmenu", { clientX: 1100, clientY: 160 });
  // Wait a microtask in case async floating positioning is used
  await Promise.resolve();
  assert.equal(contextEvents.length, 1, "contextmenu must invoke onMemberContextMenu");
  assert.equal(contextEvents[0].userId, "u-1");
  assert.equal(typeof contextEvents[0].position?.x, "number");
  assert.equal(typeof contextEvents[0].position?.y, "number");
  assert.ok(contextEvents[0].position.x >= 0 && contextEvents[0].position.y >= 0);

  // Trigger hover card
  item.dispatchEvent("mouseenter", {});
  await Promise.resolve();
  assert.equal(hoverEvents.length, 1, "mouseenter must invoke onMemberHover");
  assert.equal(hoverEvents[0].userId, "u-1");
  assert.equal(typeof hoverEvents[0].position?.x, "number");
  assert.equal(typeof hoverEvents[0].position?.y, "number");

  item.dispatchEvent("mouseleave", {});
  assert.equal(hoverEndEvents.length, 1, "mouseleave must invoke onMemberHoverEnd");
});

test("W04-4: members.js renderMembersList virtualizes 25,000+ users grouped by roles with @tanstack/virtual-core", () => {
  assert.equal(membersVirtualPowered, true, "members.js must be powered by @tanstack/virtual-core");

  const doc = /** @type {any} */ (new ShimDocument());
  const container = doc.createElement("div");
  container.boxHeight = 600;

  const total = 25_000;
  const members = Array.from({ length: total }, (_, i) => ({
    userId: `m-${i}`,
    name: `Member ${i}`,
    status: i % 5 === 0 ? "offline" : i % 4 === 0 ? "mobile" : "online",
    roles: i % 10 === 0 ? ["role-admin"] : [],
  }));
  const groups = [
    { id: "role-admin", name: "Админы", count: 2500, roleColor: 0xe91e63 },
    { id: "online", name: "В сети", count: 18000 },
    { id: "offline", name: "Не в сети", count: 4500 },
  ];

  const handle = renderMembersList(/** @type {any} */ (container), members, {
    groups,
    virtualize: true,
    viewportHeight: 600,
    overscan: 6,
  });

  assert.ok(handle, "virtualized renderMembersList should return a virtual controller handle");
  const sizer = container.querySelector("[data-member-list-sizer]");
  assert.ok(sizer, "virtualized renderMembersList must create [data-member-list-sizer]");

  const renderedRows = container.querySelectorAll(".member");
  assert.ok(renderedRows.length > 0 && renderedRows.length < 60, `expected < 60 rendered .member rows, got ${renderedRows.length}`);

  // Scroll to middle
  const midOffset = Math.floor((handle?.getTotalSize?.() ?? 0) / 2);
  handle?.scrollTo(midOffset, 600);
  const midRows = container.querySelectorAll(".member");
  assert.ok(midRows.length > 0 && midRows.length < 60, `expected < 60 rendered .member rows after scroll, got ${midRows.length}`);
});
