// @ts-nocheck
import { describe, it, beforeEach, afterEach } from "node:test";
import assert from "node:assert/strict";
import {
  renderMessage,
  refreshLiveChatAuthorsInDom,
} from "../../rust/gui-spike/ui/features/message-rendering.js";
import { createPinsThreadsWiring } from "../../rust/gui-spike/ui/features/pins-threads-wiring.js";
import {
  EMOJI_PICKER_CATEGORIES,
  createLiveEmojiPickerController,
} from "../../rust/gui-spike/ui/features/live-emoji-picker.js";
import { createMessageInteractionController } from "../../rust/gui-spike/ui/features/message-interaction-controller.js";

function createFakeElement(tag, doc) {
  const attrs = new Map();
  const listeners = new Map();
  const el = {
    tagName: tag.toUpperCase(),
    ownerDocument: doc,
    id: "",
    className: "",
    textContent: "",
    _innerHTML: "",
    type: "",
    value: "",
    title: "",
    placeholder: "",
    style: {
      setProperty(k, v) {
        this[k] = v;
      },
    },
    classList: {
      add() {},
      remove() {},
      toggle() {},
      contains: () => false,
    },
    dataset: {},
    children: [],
    parentNode: null,
    get innerHTML() {
      return this._innerHTML;
    },
    set innerHTML(val) {
      this._innerHTML = String(val);
    },
    setAttribute(k, v) {
      attrs.set(k, String(v));
      if (k === "id") this.id = String(v);
    },
    getAttribute(k) {
      return attrs.has(k) ? attrs.get(k) : null;
    },
    removeAttribute(k) {
      attrs.delete(k);
    },
    hasAttribute(k) {
      return attrs.has(k);
    },
    append(...nodes) {
      for (const n of nodes) {
        if (n && typeof n === "object") {
          n.parentNode = this;
          this.children.push(n);
          if (n.id) doc._byId.set(n.id, n);
        }
      }
    },
    appendChild(node) {
      this.append(node);
      return node;
    },
    after(node) {
      if (this.parentNode) {
        const idx = this.parentNode.children.indexOf(this);
        if (idx >= 0) {
          node.parentNode = this.parentNode;
          this.parentNode.children.splice(idx + 1, 0, node);
        }
      }
    },
    replaceChildren(...nodes) {
      this.children = [];
      this.append(...nodes);
    },
    remove() {
      if (this.id && doc._byId.get(this.id) === this) {
        doc._byId.delete(this.id);
      }
      if (this.parentNode) {
        const idx = this.parentNode.children.indexOf(this);
        if (idx >= 0) this.parentNode.children.splice(idx, 1);
        this.parentNode = null;
      }
    },
    addEventListener(type, fn) {
      if (!listeners.has(type)) listeners.set(type, []);
      listeners.get(type).push(fn);
    },
    dispatchEvent(ev) {
      const fns = listeners.get(ev.type) ?? [];
      for (const fn of fns) fn(ev);
    },
    focus() {},
    scrollIntoView() {},
    getBoundingClientRect() {
      return { left: 200, top: 200, right: 600, bottom: 240, width: 400, height: 40 };
    },
    closest(selector) {
      if (selector.includes("channel-message") && this.className.includes("channel-message")) {
        return this;
      }
      return this.parentNode ? this.parentNode.closest?.(selector) ?? null : null;
    },
    querySelector(selector) {
      return this.querySelectorAll(selector)[0] ?? null;
    },
    querySelectorAll(selector) {
      const out = [];
      const visit = (node) => {
        for (const child of node.children ?? []) {
          if (matchesSelector(child, selector)) out.push(child);
          visit(child);
        }
      };
      visit(this);
      return out;
    },
  };
  return el;
}

function matchesSelector(el, selector) {
  if (!el || typeof el !== "object") return false;
  const parts = selector.trim().split(/\s+/);
  const leaf = parts[parts.length - 1] ?? "";
  if (leaf.startsWith("#")) {
    const id = leaf.slice(1).split(/[.[]/)[0];
    if (el.id !== id) return false;
  }
  if (leaf.includes(".reactions") && !el.className.split(/\s+/).includes("reactions")) return false;
  if (leaf.includes(".reaction-pill") && !el.className.split(/\s+/).includes("reaction-pill")) return false;
  if (leaf.includes(".reaction-count") && !el.className.split(/\s+/).includes("reaction-count")) return false;
  if (leaf.includes(".text-body") && !el.className.split(/\s+/).includes("text-body")) return false;
  if (leaf.includes(".text") && !leaf.includes(".text-body") && !el.className.split(/\s+/).includes("text")) return false;
  if (leaf.includes(".author-name") && !el.className.split(/\s+/).includes("author-name")) return false;
  if (leaf.includes(".author") && !leaf.includes(".author-name") && !el.className.split(/\s+/).includes("author")) return false;
  if (leaf.includes(".chat-clan-badge") && !el.className.split(/\s+/).includes("chat-clan-badge")) return false;
  if (leaf.includes(".avatar") && !el.className.split(/\s+/).includes("avatar")) return false;
  if (leaf.includes(".live-msg-hover-toolbar") && !el.className.split(/\s+/).includes("live-msg-hover-toolbar")) return false;
  const emojiMatch = leaf.match(/\[data-emoji="([^"]+)"\]/);
  if (emojiMatch && el.getAttribute("data-emoji") !== emojiMatch[1]) return false;
  if (leaf.includes("[data-author-id]") && el.getAttribute("data-author-id") === null) return false;
  return true;
}

function createFakeDocument() {
  const doc = {
    _byId: new Map(),
    body: null,
    createElement(tag) {
      return createFakeElement(tag, doc);
    },
    createTextNode(text) {
      return { nodeType: 3, textContent: String(text) };
    },
    getElementById(id) {
      return doc._byId.get(id) ?? null;
    },
    querySelector(selector) {
      return doc.body.querySelector(selector);
    },
    querySelectorAll(selector) {
      return doc.body.querySelectorAll(selector);
    },
  };
  doc.body = createFakeElement("body", doc);
  return doc;
}

describe("RW51-EPIC2-T4: Modular Message Rendering, Pins/Threads, Emoji Picker & Interactions", () => {
  let origDoc;
  let origWin;
  let origRaf;
  let doc;

  beforeEach(() => {
    origDoc = globalThis.document;
    origWin = globalThis.window;
    origRaf = globalThis.requestAnimationFrame;
    doc = createFakeDocument();
    globalThis.document = doc;
    globalThis.window = { innerWidth: 1280, innerHeight: 800 };
    globalThis.requestAnimationFrame = (fn) => {
      fn();
      return 1;
    };
  });

  afterEach(() => {
    globalThis.document = origDoc;
    globalThis.window = origWin;
    globalThis.requestAnimationFrame = origRaf;
  });

  it("1. renderMessage renders into an existing row or creates a new row via opts.document", () => {
    const row = doc.createElement("div");
    const res1 = renderMessage(row, { id: "m1", author: "Alice", text: "Hello" }, 0, 1, {});
    assert.equal(res1, row);
    assert.ok(row.className.includes("channel-message"));

    const res2 = renderMessage({ id: "m2", author: "Bob", text: "World" }, { document: doc });
    assert.ok(res2);
    assert.ok(res2.className.includes("channel-message"));
  });

  it("2. refreshLiveChatAuthorsInDom hydrates role colors, display names, and clan badges", () => {
    const chatList = doc.createElement("div");
    chatList.id = "chat-list";
    const row = doc.createElement("div");
    row.className = "row channel-message";
    row.setAttribute("data-author-id", "u1");
    const author = doc.createElement("div");
    author.className = "author";
    const authorName = doc.createElement("span");
    authorName.className = "author-name";
    authorName.textContent = "u1";
    author.append(authorName);
    row.append(author);
    chatList.append(row);
    doc.body.append(chatList);

    refreshLiveChatAuthorsInDom({
      isLiveSession: () => true,
      currentGuildInfo: () => ({ id: "g1" }),
      rail: { selectedId: "g1" },
      selectedGuildId: () => "g1",
      HOME_GUILD_ID: "@me",
      cleanDiscordStr: (v) => String(v ?? "").trim(),
      resolveUserRoleInfo: () => ({ colorHex: "#ff5500", topRoleName: "Mod", roles: [] }),
      liveUsersById: new Map([["u1", { globalName: "Alice" }]]),
      liveMemberClanTagByUser: new Map([["u1", "CHRD"]]),
      formatClanTagBadge: (tag) => `[${tag}]`,
    });

    assert.equal(authorName.style.color, "#ff5500");
    assert.equal(authorName.getAttribute("data-role-color"), "#ff5500");
    assert.equal(authorName.textContent, "Alice");
    const badge = author.querySelector(".chat-clan-badge");
    assert.ok(badge);
    assert.equal(badge.textContent, "[CHRD]");
  });

  it("3. createPinsThreadsWiring lazily creates panel controller and binds pins/threads buttons", async () => {
    const calls = [];
    let opened = [];
    const pinsBtn = doc.createElement("button");
    const threadsBtn = doc.createElement("button");

    const wiring = createPinsThreadsWiring({
      document: doc,
      container: doc.body,
      pinsButton: pinsBtn,
      threadsButton: threadsBtn,
      chat: { mode: "channel", channelId: "c1" },
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        return [{ id: "1" }];
      },
      router: {},
      createPinsThreadsPanels: (opts) => ({
        _opts: opts,
        openPins() {
          opened.push("pins");
        },
        openThreads() {
          opened.push("threads");
        },
      }),
    });

    const p1 = wiring.getPinsThreads();
    const p2 = wiring.getPinsThreads();
    assert.equal(p1, p2);
    await p1._opts.loadPins();
    await p1._opts.loadThreads();
    assert.deepEqual(calls, [
      { cmd: "pins_list", args: { channelId: "c1" } },
      { cmd: "threads_list", args: { channelId: "c1" } },
    ]);

    wiring.bindPinsThreadsButtons();
    pinsBtn.dispatchEvent({ type: "click" });
    threadsBtn.dispatchEvent({ type: "click" });
    assert.deepEqual(opened, ["pins", "threads"]);
  });

  it("4. createLiveEmojiPickerController toggles reactions and mounts emoji picker popout", () => {
    assert.ok(Array.isArray(EMOJI_PICKER_CATEGORIES));
    assert.ok(EMOJI_PICKER_CATEGORIES.length >= 9);

    const ipcCalls = [];
    const inserted = [];
    const emojiCtrl = createLiveEmojiPickerController({
      byId: (id) => doc.getElementById(id),
      chat: { channelId: "c1" },
      composer: { insertText: (txt) => inserted.push(txt) },
      invoke: async (cmd, args) => {
        ipcCalls.push({ cmd, args });
      },
    });

    const msgRow = doc.createElement("div");
    msgRow.setAttribute("data-message-id", "m1");
    msgRow.setAttribute("data-channel-id", "c1");
    const textEl = doc.createElement("div");
    textEl.className = "text";
    msgRow.append(textEl);

    emojiCtrl.applyQuickReactionToRow(msgRow, "🔥", ":fire:");
    let pill = msgRow.querySelector('.reaction-pill[data-emoji="🔥"]');
    assert.ok(pill);
    assert.equal(pill.getAttribute("data-me"), "true");
    assert.equal(ipcCalls[0].cmd, "toggle_reaction");
    assert.equal(ipcCalls[0].args.active, true);

    emojiCtrl.applyQuickReactionToRow(msgRow, "🔥", ":fire:");
    pill = msgRow.querySelector('.reaction-pill[data-emoji="🔥"]');
    assert.equal(pill, null);
    assert.equal(ipcCalls[1].args.active, false);

    const anchor = doc.createElement("button");
    emojiCtrl.openLiveComposerEmojiPicker(anchor);
    const pop = doc.getElementById("live-emoji-picker-popout");
    assert.ok(pop);
    const emojiBtn = pop.querySelectorAll("button").find((b) => b.getAttribute("data-emoji-glyph") === "🚀");
    assert.ok(emojiBtn);
    emojiBtn.dispatchEvent({ type: "click" });
    assert.deepEqual(inserted, ["🚀"]);
    assert.equal(doc.getElementById("live-emoji-picker-popout"), null);
  });

  it("5. createMessageInteractionController enforces DM/Guild permissions, deletes rows, and opens context menu", async () => {
    const toasts = [];
    const ipcCalls = [];
    const removedFromWin = [];
    let isDm = false;
    let hasManageMessages = false;

    const emojiCtrl = createLiveEmojiPickerController({
      byId: (id) => doc.getElementById(id),
      chat: { channelId: "c1" },
      composer: { insertText: () => {} },
      invoke: async () => {},
    });

    const msgCtrl = createMessageInteractionController({
      byId: (id) => doc.getElementById(id),
      content: doc.body,
      chat: {
        channelId: "c1",
        removeMessageFromWindow: (mid, cid) => removedFromWin.push({ mid, cid }),
      },
      rail: { selectedId: "g1" },
      tree: { channels: [{ id: "c1" }] },
      dmFriendsController: { getPrivateChannels: () => (isDm ? [{ id: "c1" }] : []) },
      liveSelfUser: { id: "self-1", username: "me" },
      selectedGuildId: () => "g1",
      cleanDiscordStr: (v) => String(v ?? "").trim(),
      computeEffectiveChannelPermissions: () => (hasManageMessages ? 1n << 13n : 0n),
      computeServerPermissionsAndHierarchy: () => ({ isOwner: false, isAdmin: false, canManageMessages: hasManageMessages }),
      clearErrorState: () => {},
      showVoiceToastNotice: (msg) => toasts.push(msg),
      invoke: async (cmd, args) => {
        ipcCalls.push({ cmd, args });
      },
      isLiveSession: () => true,
      PIN_CHANNEL_MESSAGE_IPC_CMD: "pin_channel_message",
      emojiPickerController: emojiCtrl,
    });

    const ownRow = doc.createElement("div");
    ownRow.setAttribute("data-author-id", "self-1");
    ownRow.setAttribute("data-message-id", "m-own");
    ownRow.setAttribute("data-channel-id", "c1");
    const ownBody = doc.createElement("div");
    ownBody.className = "text-body";
    ownBody.textContent = "My message";
    ownRow.append(ownBody);

    const otherRow = doc.createElement("div");
    otherRow.setAttribute("data-author-id", "other-2");
    otherRow.setAttribute("data-message-id", "m-other");
    otherRow.setAttribute("data-channel-id", "c1");
    const otherBody = doc.createElement("div");
    otherBody.className = "text-body";
    otherBody.textContent = "Other message";
    otherRow.append(otherBody);

    assert.equal(msgCtrl.isOwnLiveMessage(ownRow), true);
    assert.equal(msgCtrl.canEditLiveMessage(ownRow), true);
    assert.equal(msgCtrl.canEditLiveMessage(otherRow), false);
    assert.equal(msgCtrl.canDeleteLiveMessage(ownRow), true);
    assert.equal(msgCtrl.canDeleteLiveMessage(otherRow), false);

    hasManageMessages = true;
    assert.equal(msgCtrl.canDeleteLiveMessage(otherRow), true);
    assert.equal(msgCtrl.canPinLiveMessage(otherRow), true);

    isDm = true;
    assert.equal(msgCtrl.canDeleteLiveMessage(otherRow), false);
    assert.equal(msgCtrl.canPinLiveMessage(otherRow), true);

    await msgCtrl.deleteLiveMessageRow(otherRow);
    assert.ok(toasts.some((t) => t.includes("нет прав")));

    await msgCtrl.deleteLiveMessageRow(ownRow);
    assert.equal(ipcCalls[0].cmd, "delete_message");
    assert.deepEqual(removedFromWin, [{ mid: "m-own", cid: "c1" }]);

    msgCtrl.openLiveMessageContextMenu(ownRow, 100, 100);
    const menu = doc.getElementById("live-msg-context-menu");
    assert.ok(menu);
    assert.equal(menu.getAttribute("role"), "menu");
  });
});
