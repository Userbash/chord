// @ts-check
/**
 * TASK-187 (RW-40 / G2): unit-тесты центра «Почта» (`inbox.js`).
 *
 * Независимый оракул — контракт задачи: 4 вкладки, бейдж `""`/`"99+"`/`String(n)`,
 * счётчик `mentions + friendRequests`, пустое состояние «Всё прочитано».
 * DOM — локальный минимальный шим (без зависимостей), поэтому тесты проверяют
 * только стандартные DOM-операции, которые поддерживает реальный Chromium.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  INBOX_TABS,
  formatInboxBadge,
  createInboxController,
} from "../../rust/gui-spike/ui/features/inbox.js";

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим
 * ------------------------------------------------------------------ */

class ClassList {
  /** @param {FakeElement} el */
  constructor(el) { this.el = el; }
  /** @param {string} name */
  add(name) {
    const set = new Set((this.el.getAttribute("class") || "").split(/\s+/).filter(Boolean));
    set.add(name);
    this.el.setAttribute("class", [...set].join(" "));
  }
  /** @param {string} name */
  contains(name) {
    return (this.el.getAttribute("class") || "").split(/\s+/).includes(name);
  }
}

class FakeElement {
  /** @param {string} tagName */
  constructor(tagName) {
    this.tagName = tagName.toUpperCase();
    /** @type {Map<string, string>} */
    this._attrs = new Map();
    /** @type {FakeElement[]} */
    this.children = [];
    /** @type {FakeElement | null} */
    this.parentNode = null;
    this._text = "";
    /** @type {Map<string, Function[]>} */
    this._listeners = new Map();
    this.classList = new ClassList(this);
  }
  /** @returns {string} */
  get id() { return this.getAttribute("id") || ""; }
  /** @param {string} value */
  set id(value) { this.setAttribute("id", value); }
  /** @returns {boolean} */
  get hidden() { return this.hasAttribute("hidden"); }
  /** @param {boolean} value */
  set hidden(value) { if (value) this.setAttribute("hidden", ""); else this.removeAttribute("hidden"); }
  /** @returns {string} */
  get textContent() {
    if (this.children.length > 0) return this.children.map((c) => c.textContent).join("");
    return this._text;
  }
  set textContent(value) {
    this.children = [];
    this._text = String(value);
  }
  /** @param {string} name @param {string} value */
  setAttribute(name, value) { this._attrs.set(name, String(value)); }
  /** @param {string} name */
  getAttribute(name) { return this._attrs.has(name) ? this._attrs.get(name) : null; }
  /** @param {string} name */
  hasAttribute(name) { return this._attrs.has(name); }
  /** @param {string} name */
  removeAttribute(name) { this._attrs.delete(name); }
  /** @param {FakeElement} child */
  appendChild(child) {
    child.remove();
    child.parentNode = this;
    this.children.push(child);
    return child;
  }
  /** @param {FakeElement} child */
  removeChild(child) {
    this.children = this.children.filter((c) => c !== child);
    child.parentNode = null;
    return child;
  }
  /** @param {...FakeElement} nodes */
  replaceChildren(...nodes) {
    for (const child of this.children) child.parentNode = null;
    this.children = [];
    this._text = "";
    for (const node of nodes) this.appendChild(node);
  }
  remove() {
    if (this.parentNode) this.parentNode.removeChild(this);
  }
  /** @param {string} type @param {Function} listener */
  addEventListener(type, listener) {
    if (!this._listeners.has(type)) this._listeners.set(type, []);
    this._listeners.get(type)?.push(listener);
  }
  /** @param {string} type */
  dispatchEvent(type) {
    const event = { type, target: this, currentTarget: this };
    for (const listener of this._listeners.get(type) || []) listener(event);
    return true;
  }
  click() { return this.dispatchEvent("click"); }
  /** @param {string} selector @returns {FakeElement | null} */
  querySelector(selector) { return this.querySelectorAll(selector)[0] ?? null; }
  /** @param {string} selector @returns {FakeElement[]} */
  querySelectorAll(selector) { return collect(this, selector); }
}

/** @param {FakeElement} root @param {string} selector @returns {FakeElement[]} */
function collect(root, selector) {
  /** @type {FakeElement[]} */
  const out = [];
  const walk = (/** @type {FakeElement} */ el) => {
    for (const child of el.children) {
      if (matches(child, selector)) out.push(child);
      walk(child);
    }
  };
  walk(root);
  return out;
}

/** @param {FakeElement} el @param {string} selector */
function matches(el, selector) {
  const tokens = selector.match(/\[[^\]]+\]|[#.][\w-]+|[a-zA-Z][\w-]*/g) || [];
  if (tokens.length === 0) return false;
  return tokens.every((token) => {
    if (token.startsWith("#")) return el.id === token.slice(1);
    if (token.startsWith(".")) return el.classList.contains(token.slice(1));
    if (token.startsWith("[")) {
      const m = token.slice(1, -1).match(/^([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?$/);
      if (!m || !m[1]) return false;
      const attrName = m[1];
      const value = m[2] ?? m[3] ?? m[4];
      if (value === undefined) return el.hasAttribute(attrName);
      return el.getAttribute(attrName) === value;
    }
    return el.tagName === token.toUpperCase();
  });
}

function makeDoc() {
  const body = new FakeElement("body");
  const documentElement = new FakeElement("html");
  documentElement.appendChild(body);
  return {
    body,
    documentElement,
    /** @param {string} tag */
    createElement: (tag) => new FakeElement(tag),
    /** @param {string} selector */
    querySelector: (selector) => (matches(body, selector) ? body : body.querySelector(selector)),
    /** @param {string} selector */
    querySelectorAll: (selector) => body.querySelectorAll(selector),
  };
}

/* ------------------------------------------------------------------ *
 * Тесты
 * ------------------------------------------------------------------ */

describe("INBOX_TABS", () => {
  it("содержит ровно 4 вкладки в контрактном порядке и заморожен", () => {
    assert.deepEqual([...INBOX_TABS], ["for_you", "unreads", "mentions", "friend_requests"]);
    assert.equal(Object.isFrozen(INBOX_TABS), true);
  });
});

describe("formatInboxBadge", () => {
  it("возвращает пустую строку для 0 и отрицательных", () => {
    assert.equal(formatInboxBadge(0), "");
    assert.equal(formatInboxBadge(-3), "");
  });
  it("возвращает строку числа для 1..99", () => {
    assert.equal(formatInboxBadge(1), "1");
    assert.equal(formatInboxBadge(42), "42");
    assert.equal(formatInboxBadge(99), "99");
  });
  it("сжимает всё больше 99 в 99+", () => {
    assert.equal(formatInboxBadge(100), "99+");
    assert.equal(formatInboxBadge(1234), "99+");
  });
});

describe("createInboxController: DOM-каркас", () => {
  it("рендерит попап с ролью dialog, aria-label и скрытым состоянием", () => {
    const doc = makeDoc();
    createInboxController(doc);
    const popover = doc.querySelector("#inbox-popover");
    assert.ok(popover);
    assert.equal(popover.getAttribute("role"), "dialog");
    assert.equal(popover.getAttribute("aria-label"), "Почта");
    assert.equal(popover.hidden, true);
    assert.equal(popover.getAttribute("aria-hidden"), "true");
  });

  it("рендерит по кнопке на каждую вкладку, бейдж и кнопку «прочитать все»", () => {
    const doc = makeDoc();
    createInboxController(doc);
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    const tabs = popover.querySelectorAll("[data-inbox-tab]");
    assert.deepEqual(tabs.map((t) => t.getAttribute("data-inbox-tab")), [...INBOX_TABS]);
    assert.ok(popover.querySelector("[data-inbox-badge]"));
    assert.ok(popover.querySelector('[data-action="inbox-mark-all-read"]'));
  });

  it("open/close/toggle синхронизируют hidden и aria-hidden", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    inbox.open();
    assert.equal(popover.hidden, false);
    assert.equal(popover.getAttribute("aria-hidden"), "false");
    inbox.close();
    assert.equal(popover.hidden, true);
    assert.equal(popover.getAttribute("aria-hidden"), "true");
    inbox.toggle();
    assert.equal(popover.hidden, false);
    inbox.toggle();
    assert.equal(popover.hidden, true);
  });

  it("setTab переключает aria-selected и содержимое панели", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setItems({ mentions: [{ id: "m1", title: "Упоминание" }] });
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    inbox.setTab("mentions");
    const selected = popover.querySelectorAll('[data-inbox-tab][aria-selected="true"]');
    assert.equal(selected.length, 1);
    assert.equal(selected[0]?.getAttribute("data-inbox-tab"), "mentions");
    assert.equal(popover.querySelectorAll("[data-inbox-item]").length, 1);
  });

  it("клик по кнопке вкладки переключает активную вкладку", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setItems({ unreads: [{ id: "u1", title: "Непрочитанное" }] });
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    /** @type {FakeElement} */ (popover.querySelector('[data-inbox-tab="unreads"]')).click();
    assert.equal(inbox.getActiveTab(), "unreads");
    assert.equal(popover.querySelectorAll("[data-inbox-item]").length, 1);
  });
});

describe("createInboxController: данные и счётчики", () => {
  it("бейдж считает mentions + friendRequests", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    inbox.setItems({
      mentions: [{ id: "m1" }, { id: "m2" }],
      friendRequests: [{ id: "f1" }],
      unreads: [{ id: "u1" }, { id: "u2" }, { id: "u3" }],
    });
    assert.equal(inbox.getUnreadCount(), 3);
    assert.equal(/** @type {FakeElement} */ (popover.querySelector("[data-inbox-badge]")).textContent, "3");
  });

  it("бейдж сжимается в 99+ при 100 упоминаниях", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setItems({ mentions: Array.from({ length: 100 }, (_, i) => ({ id: `m${i}` })) });
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    assert.equal(/** @type {FakeElement} */ (popover.querySelector("[data-inbox-badge]")).textContent, "99+");
    assert.equal(inbox.getUnreadCount(), 100);
  });

  it("показывает «Всё прочитано» на пустой вкладке", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setTab("mentions");
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    assert.equal(popover.querySelectorAll("[data-inbox-item]").length, 0);
    assert.equal(/** @type {FakeElement} */ (popover.querySelector("[data-inbox-empty]")).textContent, "Всё прочитано");
  });

  it("вкладка for_you объединяет упоминания и заявки в друзья", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setItems({
      mentions: [{ id: "m1", title: "Упомянули" }],
      friendRequests: [{ id: "f1", title: "Заявка" }],
    });
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    const items = popover.querySelectorAll("[data-inbox-item]");
    assert.deepEqual(items.map((i) => i.getAttribute("data-inbox-item")), ["m1", "f1"]);
  });

  it("клик по элементу зовёт onSelectMessage", () => {
    const doc = makeDoc();
    /** @type {any[]} */
    const selected = [];
    const inbox = createInboxController(doc, { onSelectMessage: (item) => selected.push(item) });
    inbox.setItems({ mentions: [{ id: "m1", title: "Упомянули" }] });
    inbox.setTab("mentions");
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    /** @type {FakeElement} */ (popover.querySelector('[data-inbox-item="m1"]')?.querySelector('button'))?.click();
    assert.equal(selected.length, 1);
    assert.equal(selected[0].id, "m1");
  });

  it("кнопка «Принять» на заявке зовёт onAcceptFriend", () => {
    const doc = makeDoc();
    /** @type {any[]} */
    const accepted = [];
    const inbox = createInboxController(doc, { onAcceptFriend: (item) => accepted.push(item) });
    inbox.setItems({ friendRequests: [{ id: "f1", title: "Заявка" }] });
    inbox.setTab("friend_requests");
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    /** @type {FakeElement} */ (popover.querySelector('[data-action="inbox-accept-friend"]')).click();
    assert.equal(accepted.length, 1);
    assert.equal(accepted[0].id, "f1");
  });

  it("markItemRead удаляет элемент и уменьшает бейдж", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setItems({ mentions: [{ id: "m1" }, { id: "m2" }], friendRequests: [{ id: "f1" }] });
    assert.equal(inbox.markItemRead("m2"), true);
    assert.equal(inbox.getUnreadCount(), 2);
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    assert.equal(popover.querySelectorAll('[data-inbox-item="m2"]').length, 0);
    assert.equal(inbox.markItemRead("nope"), false);
  });

  it("markAllRead очищает списки, бейдж и показывает пустое состояние", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setItems({
      mentions: [{ id: "m1" }],
      unreads: [{ id: "u1" }],
      friendRequests: [{ id: "f1" }],
    });
    inbox.markAllRead();
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    assert.equal(inbox.getUnreadCount(), 0);
    assert.equal(/** @type {FakeElement} */ (popover.querySelector("[data-inbox-badge]")).textContent, "");
    assert.equal(popover.querySelectorAll("[data-inbox-item]").length, 0);
    assert.ok(popover.querySelector("[data-inbox-empty]"));
  });

  it("клик по кнопке mark-all-read очищает бейдж", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    inbox.setItems({ mentions: [{ id: "m1" }] });
    const popover = /** @type {FakeElement} */ (doc.querySelector("#inbox-popover"));
    /** @type {FakeElement} */ (popover.querySelector('[data-action="inbox-mark-all-read"]')).click();
    assert.equal(inbox.getUnreadCount(), 0);
  });

  it("destroy удаляет попап из документа", () => {
    const doc = makeDoc();
    const inbox = createInboxController(doc);
    assert.ok(doc.querySelector("#inbox-popover"));
    inbox.destroy();
    assert.equal(doc.querySelector("#inbox-popover"), null);
  });
});
