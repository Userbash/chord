// @ts-nocheck
/**
 * Wave RW-53 (W06): Cleanroom Discord Emoji/GIF/Sticker Picker,
 * Message Hover Reaction Bar (5 emojis), and Reactive Reaction Counters.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  EMOJI_CATEGORIES,
  EXPRESSION_TABS_RU,
  SKIN_TONE_MODIFIERS,
  isFloatingUiDomPowered,
  isTanstackVirtualPowered,
  createExpressionPickerController,
} from "../../rust/gui-spike/ui/features/expression-picker.js";
import {
  EMOJI_PICKER_CATEGORIES,
  createLiveEmojiPickerController,
} from "../../rust/gui-spike/ui/features/live-emoji-picker.js";
import {
  REACTION_TYPES,
  DEFAULT_HOVER_QUICK_REACTIONS,
  isSignalsCorePowered,
  getHoverQuickReactions,
  createReactionsUiController,
} from "../../rust/gui-spike/ui/features/reactions-ui.js";
import {
  createReactionsController,
  resetReactions,
} from "../../rust/gui-spike/ui/features/reactions.js";

describe("W06-1: Cleanroom Discord Emoji/GIF/Sticker Picker (category navigation, live search, skin tone, ESM libs)", () => {
  it("exports Floating UI and TanStack Virtual integration flags and 9 emoji categories", () => {
    assert.equal(isFloatingUiDomPowered, true);
    assert.equal(isTanstackVirtualPowered, true);
    assert.ok(Array.isArray(EMOJI_CATEGORIES));
    assert.equal(EMOJI_CATEGORIES.length, 9);
    assert.equal(EMOJI_CATEGORIES[0]?.id, "recent");
    assert.equal(EMOJI_PICKER_CATEGORIES.length, 9);
  });

  it("renders category rail, category sections, live search, cleanroom footer, and navigates categories", () => {
    const doc = new ShimDocument();
    const container = doc.createElement("div");
    doc.body.appendChild(container);

    const picker = createExpressionPickerController({
      container,
      activeTab: "emoji",
    });

    const rail = picker.root.querySelector("[data-picker-category-rail]");
    assert.ok(rail, "category rail must be rendered for emoji tab");

    const catBtns = picker.root.querySelectorAll("[data-picker-category-btn]");
    assert.equal(catBtns.length, 9, "must render 9 category navigation buttons");

    const sections = picker.root.querySelectorAll("[data-picker-category-section]");
    assert.ok(sections.length >= 2, "must render categorized emoji sections");

    // Navigate to 'food' category
    const switched = picker.scrollToCategory("food");
    assert.equal(switched, true);
    assert.equal(picker.getState().activeCategory, "food");

    const foodBtn = picker.root.querySelector('[data-picker-category-btn="food"]');
    assert.equal(foodBtn?.getAttribute("aria-pressed"), "true");

    // Live search filters across categories and shows empty state when unmatched
    picker.setSearchQuery("пицца");
    const visibleEmojis = picker.root.querySelectorAll('[data-picker-item-type="emoji"]');
    assert.ok(visibleEmojis.length >= 1, "search for 'пицца' should match pizza emoji");

    picker.setSearchQuery("zzzz_nonexistent_emoji_xyz");
    const emptyEl = picker.root.querySelector("[data-picker-empty]");
    assert.ok(emptyEl, "unmatched search must render empty state");

    // Clear search and switch tabs (GIF & Stickers)
    picker.setSearchQuery("");
    picker.setTab("gif");
    assert.equal(picker.getActiveTab(), "gif");
    assert.ok(
      picker.root.querySelectorAll('[data-picker-item-type="gif"]').length >= 2,
      "GIF tab must render GIF items",
    );

    picker.setTab("stickers");
    assert.equal(picker.getActiveTab(), "stickers");
    assert.ok(
      picker.root.querySelectorAll('[data-picker-item-type="stickers"]').length >= 2,
      "Stickers tab must render sticker items",
    );

    // Cleanroom footer exists
    const footer = picker.root.querySelector("[data-picker-footer]");
    assert.ok(footer, "cleanroom preview footer must exist");
  });

  it("supports skin tone modifier selection and @floating-ui/dom anchor positioning", async () => {
    const doc = new ShimDocument();
    const container = doc.createElement("div");
    const anchor = doc.createElement("button");
    anchor.getBoundingClientRect = () => ({
      x: 300,
      y: 500,
      left: 300,
      top: 500,
      right: 332,
      bottom: 532,
      width: 32,
      height: 32,
    });
    doc.body.appendChild(anchor);
    doc.body.appendChild(container);

    let selected = null;
    const picker = createExpressionPickerController({
      container,
      anchorEl: anchor,
      skinTone: 2,
      onSelectEmoji: (item) => {
        selected = item;
      },
    });

    const pos = await picker.updatePosition();
    assert.ok(pos && Number.isFinite(pos.x) && Number.isFinite(pos.y));

    const waveBtn = picker.root.querySelector('[data-picker-item="wave"]');
    assert.ok(waveBtn, "wave emoji should be present");
    waveBtn.click();
    assert.ok(selected, "clicking emoji should invoke onSelectEmoji");
    assert.equal(selected.glyph, `👋${SKIN_TONE_MODIFIERS[2]}`);
  });
});

describe("W06-2: Message Hover Reaction Bar (5 emojis + add reaction button)", () => {
  it("exports DEFAULT_HOVER_QUICK_REACTIONS with 5 emojis and builds top-5 frequent list", () => {
    assert.equal(DEFAULT_HOVER_QUICK_REACTIONS.length, 5);
    const merged = getHoverQuickReactions(["🚀", "👍", "🎉"]);
    assert.equal(merged.length, 5);
    assert.deepEqual(merged.slice(0, 3), ["🚀", "👍", "🎉"]);
    // No duplicates
    assert.equal(new Set(merged).size, 5);
  });

  it("reactions-ui renders hover reaction bar with 5 quick emojis and add-reaction trigger", () => {
    const doc = new ShimDocument();
    const container = doc.createElement("div");
    doc.body.appendChild(container);

    /** @type {string[]} */
    const clicked = [];
    let addClicked = 0;

    const ui = createReactionsUiController(container, {
      onToggleReaction: (emoji) => clicked.push(emoji),
      onOpenPicker: () => {
        addClicked += 1;
      },
    });

    const bar = ui.renderHoverReactionBar(["🔥", "👍", "❤️", "😂", "😮"]);
    const quickBtns = bar.querySelectorAll("[data-quick-reaction]");
    assert.equal(quickBtns.length, 5, "hover reaction bar must have exactly 5 emoji buttons");

    const addBtn = bar.querySelector("[data-add-reaction-btn]");
    assert.ok(addBtn, "hover reaction bar must include add-reaction button");

    quickBtns[0].click();
    assert.deepEqual(clicked, ["🔥"]);

    addBtn.click();
    assert.equal(addClicked, 1);
  });

  it("reactions.js ownToolbar renders 5 quick reactions and live-emoji-picker starts with 5 frequent emojis", () => {
    resetReactions();
    const doc = new ShimDocument();
    const list = doc.createElement("div");
    list.id = "chat-list";
    const wrapper = doc.createElement("div");
    wrapper.appendChild(list);
    doc.body.appendChild(wrapper);

    const ctrl = createReactionsController({
      list,
      container: wrapper,
    });
    const toolbar = wrapper.querySelector(".msg-hover-toolbar");
    assert.ok(toolbar, "hover toolbar must be mounted");
    const quickBtns = toolbar.querySelectorAll(".quick-reaction-btn");
    assert.equal(quickBtns.length, 5, "hover toolbar must have 5 quick reaction buttons");
    ctrl.destroy();

    const live = createLiveEmojiPickerController({
      byId: () => null,
      chat: { channelId: "c1" },
      composer: { insertText: () => {} },
      invoke: async () => {},
    });
    assert.equal(live.getRecentReactionEmojis().length, 5);
  });
});

describe("W06-3: Reactive Reaction Counters with Animated Pulse on Click", () => {
  it("uses @preact/signals-core and triggers animated pulse when reaction pill is clicked or updated", () => {
    assert.equal(isSignalsCorePowered, true);

    const doc = new ShimDocument();
    const container = doc.createElement("div");
    doc.body.appendChild(container);

    /** @type {Array<{ emoji: string, type: number, nextActive: boolean }>} */
    const toggles = [];
    const ui = createReactionsUiController(container, {
      onToggleReaction: (emoji, type, nextActive) => {
        toggles.push({ emoji, type, nextActive });
      },
    });

    const bar = ui.renderReactionBar([
      { emoji: "👍", count: 2, me: false },
      { emoji: "🔥", count: 1, burstCount: 1, me: false, meBurst: false },
    ]);

    const thumbPill = bar.querySelector('[data-emoji="👍"]');
    assert.ok(thumbPill);
    assert.equal(thumbPill.querySelector(".reaction-count")?.textContent, "2");

    // Click triggers pulse & reactive update when updateReactions is called
    thumbPill.click();
    assert.equal(toggles.length, 1);
    assert.equal(thumbPill.getAttribute("data-reaction-pulse"), "true");
    assert.equal(thumbPill.classList.contains("reaction--pulse"), true);

    // Reactive signal update updates DOM counter & aria-pressed without manual re-render call
    ui.updateReactions([
      { emoji: "👍", count: 3, me: true },
      { emoji: "🔥", count: 1, burstCount: 1, me: false, meBurst: false },
    ]);

    const updatedThumb = bar.querySelector('[data-emoji="👍"]');
    assert.equal(updatedThumb?.querySelector(".reaction-count")?.textContent, "3");
    assert.equal(updatedThumb?.getAttribute("aria-pressed"), "true");
    assert.equal(ui.getReactionsSignal().value.length, 2);

    ui.destroy();
  });
});
