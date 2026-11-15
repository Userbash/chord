// @ts-nocheck
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  CHANNEL_KINDS,
  CHANNEL_GROUP_ORDER,
  UNKNOWN_CHANNEL_KIND,
  channelTreeFailureText,
  createChannelTree,
} from "../../rust/gui-spike/ui/features/channel-tree-controller.js";
import { createChannelSwitchPermissionsController } from "../../rust/gui-spike/ui/features/channel-switch-permissions.js";

/**
 * Minimal DOM element mock supporting attributes, classes, styles, events, and simple queries.
 * @param {string} tagName
 * @param {Map<string, any>} idRegistry
 */
function makeMockElement(tagName, idRegistry) {
  /** @type {Map<string, string>} */
  const attrs = new Map();
  /** @type {Map<string, Array<(ev: any) => void>>} */
  const listeners = new Map();
  /** @type {any[]} */
  const children = [];
  let _id = "";
  let _className = "";
  let _textContent = "";
  let _innerHTML = "";

  /** @type {any} */
  const el = {
    tagName: tagName.toUpperCase(),
    children,
    style: {},
    dataset: {},
    disabled: false,
    placeholder: "",
    title: "",
    type: "",
    scrollTop: 0,
    parentElement: null,
    parentNode: null,
    get id() {
      return _id;
    },
    set id(val) {
      if (_id && idRegistry.get(_id) === el) idRegistry.delete(_id);
      _id = String(val ?? "");
      if (_id) idRegistry.set(_id, el);
    },
    get className() {
      return _className;
    },
    set className(val) {
      _className = String(val ?? "");
    },
    get textContent() {
      return _textContent;
    },
    set textContent(val) {
      _textContent = String(val ?? "");
      if (_textContent === "") children.length = 0;
    },
    get innerHTML() {
      return _innerHTML;
    },
    set innerHTML(val) {
      _innerHTML = String(val ?? "");
    },
    setAttribute(name, value) {
      const str = String(value);
      attrs.set(name, str);
      if (name === "id") el.id = str;
      if (name === "class") el.className = str;
    },
    getAttribute(name) {
      if (name === "id") return _id || null;
      return attrs.has(name) ? attrs.get(name) : null;
    },
    removeAttribute(name) {
      attrs.delete(name);
    },
    hasAttribute(name) {
      return attrs.has(name);
    },
    append(...nodes) {
      for (const n of nodes) {
        if (n && typeof n === "object") {
          n.parentElement = el;
          n.parentNode = el;
          children.push(n);
        }
      }
    },
    appendChild(node) {
      el.append(node);
      return node;
    },
    insertBefore(newNode, refNode) {
      if (newNode && typeof newNode === "object") {
        newNode.parentElement = el;
        newNode.parentNode = el;
        const idx = children.indexOf(refNode);
        if (idx >= 0) children.splice(idx, 0, newNode);
        else children.push(newNode);
      }
      return newNode;
    },
    replaceChildren(...nodes) {
      for (const c of children) {
        if (c && typeof c === "object") {
          c.parentElement = null;
          c.parentNode = null;
        }
      }
      children.length = 0;
      el.append(...nodes);
    },
    remove() {
      if (_id && idRegistry.get(_id) === el) idRegistry.delete(_id);
      if (el.parentElement && Array.isArray(el.parentElement.children)) {
        const idx = el.parentElement.children.indexOf(el);
        if (idx >= 0) el.parentElement.children.splice(idx, 1);
      }
      el.parentElement = null;
      el.parentNode = null;
    },
    addEventListener(type, fn) {
      const list = listeners.get(type) ?? [];
      list.push(fn);
      listeners.set(type, list);
    },
    dispatchEvent(ev) {
      const event = ev && typeof ev === "object" ? ev : { type: String(ev) };
      if (!event.preventDefault) event.preventDefault = () => {};
      if (!event.stopPropagation) event.stopPropagation = () => {};
      for (const fn of listeners.get(event.type) ?? []) {
        fn(event);
      }
    },
    click() {
      el.dispatchEvent({ type: "click" });
    },
    focus() {
      el._focused = true;
    },
    querySelectorAll(selector) {
      /** @type {any[]} */
      const out = [];
      const walk = (/** @type {any} */ node) => {
        for (const child of node.children ?? []) {
          if (matchesSelector(child, selector)) out.push(child);
          walk(child);
        }
      };
      walk(el);
      return out;
    },
    querySelector(selector) {
      return el.querySelectorAll(selector)[0] ?? null;
    },
  };
  return el;
}

/**
 * Simple selector matcher for unit tests (#id, .class, [attr="val"]).
 * @param {any} node
 * @param {string} selector
 */
function matchesSelector(node, selector) {
  if (!node) return false;
  if (selector.startsWith("#")) {
    return node.id === selector.slice(1);
  }
  const attrMatch = selector.match(/^(\.[a-zA-Z0-9_-]+)?\[([^=\]]+)(?:="([^"]*)")?\]$/);
  if (attrMatch) {
    const [, cls, attrName, attrVal] = attrMatch;
    if (cls) {
      const classes = String(node.className || "").split(/\s+/);
      if (!classes.includes(cls.slice(1))) return false;
    }
    const actual = node.getAttribute(attrName);
    if (attrVal !== undefined) return actual === attrVal;
    return actual !== null;
  }
  if (selector.startsWith(".")) {
    const target = selector.slice(1);
    return String(node.className || "").split(/\s+/).includes(target);
  }
  return false;
}

/**
 * Install minimal global document for tests.
 */
function createTestDom() {
  /** @type {Map<string, any>} */
  const idRegistry = new Map();
  const root = makeMockElement("div", idRegistry);
  const tree = makeMockElement("aside", idRegistry);
  tree.id = "channel-tree";
  const scroll = makeMockElement("div", idRegistry);
  scroll.className = "tree-scroll";
  tree.append(scroll);
  root.append(tree);

  const doc = {
    getElementById: (/** @type {string} */ id) => idRegistry.get(id) ?? null,
    createElement: (/** @type {string} */ tag) => makeMockElement(tag, idRegistry),
    querySelector: (/** @type {string} */ sel) => root.querySelector(sel),
    querySelectorAll: (/** @type {string} */ sel) => root.querySelectorAll(sel),
  };

  return { doc, root, tree, scroll, idRegistry };
}

describe("RW51-EPIC2-T3: channel-tree-controller", () => {
  it("exports CHANNEL_KINDS, CHANNEL_GROUP_ORDER, UNKNOWN_CHANNEL_KIND, and formats failure text", () => {
    assert.equal(CHANNEL_KINDS[0]?.openable, true);
    assert.equal(CHANNEL_KINDS[2]?.key, "voice");
    assert.deepEqual(CHANNEL_GROUP_ORDER, [0, 5, 15, 2, 4]);
    assert.equal(UNKNOWN_CHANNEL_KIND.openable, false);

    assert.equal(channelTreeFailureText({ status: 401 }), "Сессия истекла. Войдите снова.");
    assert.equal(channelTreeFailureText({ kind: "NotFound" }), "Сервер не найден в локальном снимке");
    assert.equal(channelTreeFailureText({ kind: "ParseError" }), "Не удалось прочитать каналы сервера");
    assert.equal(channelTreeFailureText(new Error("boom")), "Каналы сервера недоступны");
  });

  it("loads channels, renders groups, supports keyboard navigation, and handles open/denied states", async () => {
    const { doc, tree } = createTestDom();
    const prevDoc = globalThis.document;
    globalThis.document = /** @type {any} */ (doc);
    try {
      /** @type {Array<{id: string, name: string}>} */
      const opened = [];
      const controller = createChannelTree({
        invoke: async (cmd) => {
          assert.equal(cmd, "channels");
          return [
            { id: "c-voice", type: 2, name: "General Voice", position: 1 },
            { id: "c-text", type: 0, name: "general", position: 0 },
          ];
        },
        onOpen: (ch) => opened.push(ch),
      });

      await controller.load("g-1");
      assert.equal(controller.state, "ready");
      assert.equal(tree.getAttribute("data-state"), "ready");
      assert.equal(controller.channels.length, 2);

      const buttons = controller.node.querySelectorAll(".channel-item");
      assert.equal(buttons.length, 2);
      assert.equal(buttons[0].getAttribute("data-channel-id"), "c-text");
      assert.equal(buttons[0].getAttribute("data-openable"), "true");
      assert.equal(buttons[1].getAttribute("data-channel-id"), "c-voice");
      assert.equal(buttons[1].getAttribute("data-openable"), "false");

      buttons[0].click();
      assert.deepEqual(opened, [{ id: "c-text", name: "general" }]);
      assert.equal(buttons[0].getAttribute("aria-current"), "page");

      buttons[1].click();
      assert.equal(buttons[1].getAttribute("data-denied"), "true");
    } finally {
      globalThis.document = prevDoc;
    }
  });

  it("handles empty and error states with retry, plus reset and dispose", async () => {
    const { doc } = createTestDom();
    const prevDoc = globalThis.document;
    globalThis.document = /** @type {any} */ (doc);
    try {
      let attempt = 0;
      const controller = createChannelTree({
        invoke: async () => {
          attempt += 1;
          if (attempt === 1) throw { status: 404 };
          return [];
        },
      });

      await controller.load("g-err");
      assert.equal(controller.state, "error");
      const retryBtn = controller.node.querySelector(".channel-tree-retry");
      assert.ok(retryBtn);
      retryBtn.click();
      await new Promise((r) => setTimeout(r, 10));
      assert.equal(controller.state, "empty");

      controller.reset();
      assert.equal(controller.state, "idle");
      assert.equal(controller.channels.length, 0);

      controller.dispose();
      await controller.load("g-after-dispose");
      assert.equal(controller.state, "idle");
    } finally {
      globalThis.document = prevDoc;
    }
  });
});

describe("RW51-EPIC2-T3: channel-switch-permissions", () => {
  it("renders lurker, pending, and read-only banners in syncActiveChannelPermissionsInDom and handles onChannelSwitch", () => {
    const { doc, root, idRegistry } = createTestDom();
    const prevDoc = globalThis.document;
    const prevImage = globalThis.Image;
    globalThis.document = /** @type {any} */ (doc);
    globalThis.Image = /** @type {any} */ (function FakeImage() {});
    try {
      const chatPane = makeMockElement("div", idRegistry);
      const compForm = makeMockElement("form", idRegistry);
      compForm.id = "composer";
      const compInput = makeMockElement("textarea", idRegistry);
      compInput.id = "composer-input";
      compForm.append(compInput);
      chatPane.append(compForm);
      root.append(chatPane);

      /** @type {Record<string, any>} */
      const guild = { id: "g1", name: "Guild", lurker: true };
      const guildsMap = new Map([["g1", guild]]);
      let minimizedCount = 0;
      const stageViewer = makeMockElement("div", idRegistry);
      stageViewer.id = "live-stream-stage-viewer";
      const minBtn = makeMockElement("button", idRegistry);
      minBtn.id = "stage-minimize-viewer-btn";
      minBtn.addEventListener("click", () => {
        minimizedCount += 1;
      });
      root.append(stageViewer, minBtn);

      const ctrl = createChannelSwitchPermissionsController({
        isLiveSession: () => true,
        cleanDiscordStr: (/** @type {any} */ s) => String(s ?? "").trim(),
        getSelectedGuildId: () => "g1",
        refreshTreeView: () => {},
        getTreeChannels: () => [{ id: "c1", type: 0, name: "general", parentId: "", position: 0 }],
        getActiveOpenChannelTarget: () => ({ id: "c1", type: 0, name: "general" }),
        setActiveOpenChannelTarget: () => {},
        getChatChannel: () => ({ id: "c1", type: 0, name: "general" }),
        getChatChannelId: () => "c1",
        openChatChannel: async () => {},
        resolveChannelObject: (/** @type {any} */ target) => target,
        getGuildById: (/** @type {string} */ gid) => guildsMap.get(gid) ?? null,
        computeEffectiveChannelPermissions: () => 0n,
        getSelfUserId: () => "u1",
        evaluateClientChannelPolicy: ({ channel }) => ({
          canSend: channel.canSendMessages !== false,
        }),
        applyComposerChannelPolicy: () => {},
        invoke: async () => ({}),
        showVoiceToastNotice: () => {},
        isRulesScreeningModalOpen: () => false,
        closeRulesScreeningModal: () => {},
        openRulesScreeningModal: () => {},
        getCurrentGuildInfo: () => guild,
        extractRulesFromGuild: () => [],
        rulesScreeningText: { bannerNotice: "Rules", bannerButton: "Complete" },
        refreshLiveChatAuthorsInDom: () => {},
        getActiveVoiceConnection: () => null,
        renderLiveVoicePanel: () => {},
        clearFloatingPipPosition: () => {},
      });

      ctrl.syncActiveChannelPermissionsInDom("g1");
      assert.ok(doc.getElementById("guild-preview-join-banner"));
      assert.equal(compInput.disabled, true);

      guild.lurker = false;
      guild.pending = true;
      ctrl.syncActiveChannelPermissionsInDom("g1");
      assert.equal(doc.getElementById("guild-preview-join-banner"), null);
      assert.ok(doc.getElementById("guild-pending-screening-banner"));
      assert.equal(compInput.disabled, true);

      guild.pending = false;
      ctrl.syncActiveChannelPermissionsInDom("g1");
      assert.equal(doc.getElementById("guild-pending-screening-banner"), null);
      assert.ok(doc.getElementById("channel-readonly-access-banner"));
      assert.equal(compInput.disabled, true);

      ctrl.onChannelSwitch({ id: "c1", type: 0 });
      assert.equal(minimizedCount, 1);
      ctrl.onChannelSwitch({ id: "v1", type: 2 });
      assert.equal(minimizedCount, 1);
    } finally {
      globalThis.document = prevDoc;
      globalThis.Image = prevImage;
    }
  });
});
