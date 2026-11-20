// @ts-check
import assert from "node:assert/strict";
import test from "node:test";

import { ShimDocument } from "../support/gui-dom-harness.mjs";
import { createChatHeights } from "../../rust/gui-spike/ui/model.js";
import { VirtualList, isTanstackVirtualPowered } from "../../rust/gui-spike/ui/features/virtual-list.js";
import {
  CHAT_ROWS,
  createChatScreen,
  formatStickyDateLabel,
} from "../../rust/gui-spike/ui/features/chat.js";

test("W02-1: VirtualList delegates range, sizing, and dynamic measureElement to @tanstack/virtual-core", () => {
  assert.equal(isTanstackVirtualPowered, true);
  const doc = new ShimDocument();
  const container = doc.createElement("div");
  container.scrollTop = 0;
  container.boxHeight = 300;

  const heights = createChatHeights(1000);
  const items = Array.from({ length: 1000 }, (_, i) => ({ id: String(i), text: `msg ${i}` }));
  const list = new VirtualList({
    container: /** @type {any} */ (container),
    source: {
      totalRows: 1000,
      getRow: (idx) => items[idx],
      ensureRange: async () => {},
    },
    heights,
    render: (row, item, idx) => {
      row.textContent = item ? `${idx}: ${item.text}` : "";
    },
  });

  const initialTotal = list.totalHeight;
  assert.equal(initialTotal, heights.totalHeight);

  // Dynamic height measurement: row 0 grows from 48px to 120px (+72px)
  assert.equal(typeof list.measureElement, "function", "VirtualList must expose measureElement()");
  list.measureElement(0, 120);
  list.paint();

  assert.equal(list.totalHeight, initialTotal + (120 - heights.height(0)));
  assert.equal(list.sizer.style.height, `${list.totalHeight}px`);

  const row0 = list.rows.get(0);
  const row1 = list.rows.get(1);
  assert.ok(row0 && row1);
  assert.equal(row0.style.height, "120px");
  assert.equal(row1.style.top, "120px");
});

test("W02-2: formatStickyDateLabel formats Russian date dividers like '18 ноября 2026 г.'", () => {
  assert.equal(typeof formatStickyDateLabel, "function", "chat.js must export formatStickyDateLabel");
  assert.equal(formatStickyDateLabel("2026-11-18T13:06:46+05:00"), "18 ноября 2026 г.");
  assert.equal(formatStickyDateLabel("2026-11-18"), "18 ноября 2026 г.");
  assert.equal(formatStickyDateLabel(""), "");
});

/** @param {(doc: any) => any} fn */
async function withShimGlobals(fn) {
  const doc = /** @type {any} */ (new ShimDocument());
  const prevDoc = globalThis.document;
  const prevRaf = globalThis.requestAnimationFrame;
  globalThis.document = doc;
  globalThis.requestAnimationFrame = (cb) => /** @type {any} */ (setTimeout(() => cb(Date.now()), 0));
  try {
    const res = await fn(doc);
    await new Promise((r) => setTimeout(r, 15));
    return res;
  } finally {
    globalThis.document = prevDoc;
    globalThis.requestAnimationFrame = prevRaf;
  }
}

test("W02-3: createChatScreen renders sticky date divider and jump-to-bottom unread pill", async () => {
  await withShimGlobals((doc) => {
    const surface = doc.createElement("div");
    const listEl = doc.createElement("div");
    listEl.id = "chat-list";
    listEl.boxHeight = 400;
    surface.append(listEl);

    let smoothScrollCalls = 0;
    /** @param {any} opts */
    listEl.scrollTo = (opts) => {
      smoothScrollCalls += 1;
      if (opts && typeof opts.top === "number") {
        listEl.scrollTop = opts.top;
      }
    };

    const screen = createChatScreen({ list: listEl });

    // #chat-list invariant: only sizer is a direct child of #chat-list
    assert.equal(listEl.children.length, 1, "#chat-list must contain only sizer");

    // Sticky date divider is a sibling in surface
    const stickyDivider = surface.querySelector(".chat-sticky-date-divider");
    assert.ok(stickyDivider, ".chat-sticky-date-divider must exist in chat surface");
    const stickyLabel = stickyDivider.querySelector(".chat-sticky-date-label");
    assert.ok(stickyLabel && stickyLabel.textContent.length > 0, "sticky date label must be populated");

    // Jump-to-bottom unread pill exists and appears when scrolled up
    const jumpPill = surface.querySelector(".chat-jump-bottom-pill");
    assert.ok(jumpPill, ".chat-jump-bottom-pill must exist in chat surface");
    assert.equal(jumpPill.hasAttribute("hidden"), true, "jump pill is hidden when at bottom");

    // Scroll up far from bottom
    screen.list.scrollToIndex(50_000, { behavior: "smooth" });
    assert.ok(smoothScrollCalls >= 1, "smooth scroll should invoke container.scrollTo");
    assert.equal(jumpPill.hasAttribute("hidden"), false, "jump pill becomes visible when scrolled up");

    // Simulate unread increment & click jump pill
    if (typeof screen.setUnreadPillCount === "function") {
      screen.setUnreadPillCount(5);
      assert.match(jumpPill.textContent, /Новые сообщения/);
    }
    jumpPill.click();
    assert.equal(jumpPill.hasAttribute("hidden"), true, "clicking jump pill scrolls to bottom and hides pill");
    assert.ok(screen.list.firstVisibleIndex() > CHAT_ROWS - 100, "clicking jump pill returns to bottom of 100k list");
  });
});

test("W02-4: 100k message smooth scroll maintains bounded DOM pool without bloat", async () => {
  await withShimGlobals((doc) => {
    const surface = doc.createElement("div");
    const listEl = doc.createElement("div");
    listEl.id = "chat-list";
    listEl.boxHeight = 600;
    surface.append(listEl);

    const screen = createChatScreen({ list: listEl });
    for (const target of [0, 25_000, 75_000, CHAT_ROWS - 1]) {
      screen.list.scrollToIndex(target, { align: "center", behavior: "smooth" });
      assert.equal(screen.list.sizer.children.length, screen.list.rows.size);
      assert.ok(screen.list.rows.size > 0 && screen.list.rows.size <= 60, `DOM pool size=${screen.list.rows.size}`);
    }
  });
});
