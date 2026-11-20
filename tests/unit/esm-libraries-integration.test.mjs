// @ts-check
import assert from "node:assert/strict";
import test from "node:test";

import {
  signal,
  computed,
  effect,
  batch,
  createSignal,
  createComputed,
  createEffect,
  createAppReactiveStore,
  createDiscordReactiveStore,
  isSignalsCorePowered,
} from "../../rust/gui-spike/ui/features/reactive-store.js";

import {
  signal as sigSignal,
  computed as sigComputed,
  effect as sigEffect,
  batch as sigBatch,
  isSignalsCorePowered as isSignalsModulePowered,
} from "../../rust/gui-spike/ui/state/signals.js";

import {
  createVirtualizer,
  Virtualizer,
  elementScroll,
  isTanstackVirtualPowered,
} from "../../rust/gui-spike/ui/features/virtualizer.js";

import {
  VirtualList,
  isTanstackVirtualPowered as isVirtualListPowered,
} from "../../rust/gui-spike/ui/features/virtual-list.js";

import {
  computeFloatingPosition,
  positionFloatingElement,
  computePosition,
  flip,
  shift,
  offset,
  isFloatingUiDomPowered,
} from "../../rust/gui-spike/ui/features/floating-popover.js";

import {
  tinykeys,
  createKeybindingsHandler,
  createShortcutsController,
  matchShortcut,
  DISCORD_SHORTCUTS,
  isTinykeysPowered,
} from "../../rust/gui-spike/ui/features/shortcuts.js";

import {
  tinykeys as keybindsTinykeys,
  createKeybindingsHandler as keybindsCreateHandler,
  isTinykeysPowered as isKeybindsTinykeysPowered,
} from "../../rust/gui-spike/ui/features/keybinds.js";

import { ShimDocument } from "../support/gui-dom-harness.mjs";

test("ESM Integration 1: @preact/signals-core powers reactive-store.js and state/signals.js", () => {
  assert.equal(isSignalsCorePowered, true, "reactive-store.js must be powered by @preact/signals-core");
  assert.equal(isSignalsModulePowered, true, "state/signals.js must be powered by @preact/signals-core");

  // reactive-store.js API verification
  const count = createSignal(10);
  const doubled = createComputed(() => count.value * 2);
  let effectRan = 0;
  let lastObserved = 0;

  const dispose = createEffect(() => {
    effectRan += 1;
    lastObserved = doubled.value;
  });

  assert.equal(effectRan, 1);
  assert.equal(lastObserved, 20);

  // peek does not track
  assert.equal(count.peek(), 10);

  // batching coalesces
  batch(() => {
    count.value = 15;
    count.value = 25;
  });

  assert.equal(effectRan, 2);
  assert.equal(lastObserved, 50);

  dispose();
  count.value = 100;
  assert.equal(effectRan, 2, "Disposed effect must not fire again");

  // state/signals.js verification
  const s = sigSignal("hello");
  const c = sigComputed(() => s.value.toUpperCase());
  assert.equal(c.value, "HELLO");
  sigBatch(() => {
    s.value = "chord";
  });
  assert.equal(c.value, "CHORD");

  // Store checks
  const appStore = createAppReactiveStore();
  assert.ok(appStore.activeVoiceConnection);
  assert.ok(appStore.dmCallStageSummary);

  const discordStore = createDiscordReactiveStore();
  assert.ok(discordStore.selfUser);
  assert.ok(discordStore.selectedGuildId);
});

test("ESM Integration 2: @tanstack/virtual-core powers virtualizer.js and virtual-list.js", () => {
  assert.equal(isTanstackVirtualPowered, true, "virtualizer.js must be powered by @tanstack/virtual-core");
  assert.equal(isVirtualListPowered, true, "virtual-list.js must integrate with @tanstack/virtual-core");
  assert.equal(typeof Virtualizer, "function", "Virtualizer class must be exported");
  assert.equal(typeof elementScroll, "function", "elementScroll must be exported");

  // createVirtualizer headless API
  const v = createVirtualizer({
    count: 500,
    estimateSize: () => 40,
    overscan: 2,
    scrollOffset: 400,
    viewportSize: 200,
  });

  assert.equal(v.getTotalSize(), 20000);
  const items = v.getVirtualItems();
  assert.ok(items.length > 0);
  assert.equal(items[0]?.index, 8); // 400/40 = 10, overscan 2 -> 8

  v.measureElement(10, 80);
  assert.equal(v.getTotalSize(), 20040);

  const offset = v.scrollToIndex(15, { align: "start" });
  assert.ok(offset >= 600);

  // VirtualList DOM class
  const doc = new ShimDocument();
  const container = doc.createElement("div");
  container.scrollTop = 0;
  container.boxHeight = 400;

  const mockHeights = {
    totalHeight: 4000,
    top: (/** @type {number} */ i) => i * 40,
    height: (/** @type {number} */ _i) => 40,
    indexAt: (/** @type {number} */ offset) => Math.floor(offset / 40),
  };

  const mockSource = {
    totalRows: 100,
    getRow: (/** @type {number} */ i) => ({ id: `row-${i}` }),
    ensureRange: async () => {},
  };

  const vl = new VirtualList({
    container: /** @type {any} */ (container),
    source: mockSource,
    heights: /** @type {any} */ (mockHeights),
    render: (node, item, index) => {
      node.textContent = `${index}: ${item?.id}`;
    },
  });

  assert.ok(vl.virtualizer, "VirtualList must expose underlying virtualizer instance");
  assert.equal(vl.totalHeight, 4000);
  assert.ok(vl.rows.size > 0);
});

test("ESM Integration 3: @floating-ui/dom powers floating-popover.js", async () => {
  assert.equal(isFloatingUiDomPowered, true, "floating-popover.js must be powered by @floating-ui/dom");
  assert.equal(typeof computePosition, "function", "computePosition must be exported");
  assert.equal(typeof flip, "function", "flip middleware must be exported");
  assert.equal(typeof shift, "function", "shift middleware must be exported");
  assert.equal(typeof offset, "function", "offset middleware must be exported");

  // computeFloatingPosition synchronous helper for popouts
  const pos = computeFloatingPosition(
    { x: 1200, y: 650, width: 50, height: 30 },
    { width: 200, height: 150 },
    {
      placement: "bottom-start",
      offset: 8,
      padding: 10,
      viewport: { width: 1280, height: 720 },
    },
  );

  assert.ok(pos.placement.startsWith("top"), "Must flip to top when overflowing bottom");
  assert.ok(pos.x + 200 <= 1280 - 10, "Must shift within right viewport edge");
  assert.ok(pos.y >= 10, "Must stay above top viewport padding");

  // positionFloatingElement DOM helper
  const doc = new ShimDocument();
  const refEl = doc.createElement("button");
  refEl.getBoundingClientRect = () => ({
    x: 100, y: 100, width: 80, height: 32,
    top: 100, left: 100, right: 180, bottom: 132,
  });
  const popEl = doc.createElement("div");
  popEl.getBoundingClientRect = () => ({
    x: 0, y: 0, width: 160, height: 80,
    top: 0, left: 0, right: 160, bottom: 80,
  });

  const res = await positionFloatingElement(refEl, popEl, {
    placement: "bottom-start",
    viewport: { width: 1280, height: 720 },
  });

  assert.ok(res.x >= 0);
  assert.ok(res.y >= 0);
  assert.equal(popEl.style.left, `${res.x}px`);
  assert.equal(popEl.style.top, `${res.y}px`);
});

test("ESM Integration 4: tinykeys powers shortcuts.js and keybinds.js", () => {
  assert.equal(isTinykeysPowered, true, "shortcuts.js must be powered by tinykeys");
  assert.equal(isKeybindsTinykeysPowered, true, "keybinds.js must be powered by tinykeys");
  assert.equal(typeof tinykeys, "function", "tinykeys function must be exported from shortcuts.js");
  assert.equal(typeof createKeybindingsHandler, "function", "createKeybindingsHandler must be exported");
  assert.equal(typeof keybindsTinykeys, "function", "tinykeys must be exported from keybinds.js");

  // Test tinykeys handler creation and firing
  let triggered = false;
  const handler = createKeybindingsHandler({
    "$mod+k": () => { triggered = true; },
  }, {
    ignore: () => false,
  });

  const event = {
    key: "k",
    code: "KeyK",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    getModifierState: (/** @type {string} */ mod) => mod === "Control",
    target: {},
    preventDefault: () => {},
  };

  handler(/** @type {any} */ (event));
  assert.equal(triggered, true, "tinykeys handler must dispatch registered combination");

  // Test shortcuts controller integration
  const doc = new ShimDocument();
  let switcherCalled = false;
  const controller = createShortcutsController(doc, {
    quick_switcher: () => { switcherCalled = true; },
  });

  assert.ok(controller.keybindingsHandler, "controller must have tinykeys handler");
  doc.body.dispatchEvent("keydown", /** @type {any} */ ({
    key: "k",
    code: "KeyK",
    ctrlKey: true,
    preventDefault: () => {},
  }));
  assert.equal(switcherCalled, true, "createShortcutsController must trigger quick_switcher via tinykeys");
  controller.destroy();
});
