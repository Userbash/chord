// @ts-check
/**
 * T-232 (FE-P1-25 / RW-40 / G2): unit-тесты контроллера домашнего экрана
 * личных сообщений и друзей (`rust/gui-spike/ui/features/friends-home.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  RELATIONSHIP_TYPES,
  FRIENDS_TABS_RU,
  filterRelationshipsByTab,
  createFriendsHomeController,
} from "../../rust/gui-spike/ui/features/friends-home.js";

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных unit-тестов
 * ------------------------------------------------------------------ */

class FakeElement {
  /**
   * @param {string} tagName
   * @param {any} [ownerDocument]
   */
  constructor(tagName, ownerDocument = null) {
    this.tagName = String(tagName).toUpperCase();
    this.ownerDocument = ownerDocument;
    /** @type {Map<string, string>} */
    this._attrs = new Map();
    /** @type {FakeElement[]} */
    this.children = [];
    /** @type {FakeElement | null} */
    this.parentNode = null;
    this._text = "";
    this._hidden = false;
    this.value = "";
    this.disabled = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {boolean} */
  get hidden() {
    return this._hidden || this._attrs.has("hidden");
  }

  /** @param {boolean} val */
  set hidden(val) {
    this._hidden = Boolean(val);
    if (this._hidden) {
      this._attrs.set("hidden", "");
    } else {
      this._attrs.delete("hidden");
    }
  }

  /** @returns {string} */
  get textContent() {
    if (this.children.length > 0) {
      return this.children.map((c) => c.textContent).join("");
    }
    return this._text;
  }

  /** @param {string} val */
  set textContent(val) {
    this.children = [];
    this._text = String(val ?? "");
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setAttribute(name, value) {
    const str = String(value);
    this._attrs.set(name, str);
    if (name === "hidden") {
      this._hidden = true;
    }
    if (name === "value" && !this.value) {
      this.value = str;
    }
  }

  /**
   * @param {string} name
   * @returns {string | null}
   */
  getAttribute(name) {
    return this._attrs.has(name) ? (this._attrs.get(name) ?? null) : null;
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  hasAttribute(name) {
    return this._attrs.has(name);
  }

  /** @param {string} name */
  removeAttribute(name) {
    this._attrs.delete(name);
    if (name === "hidden") {
      this._hidden = false;
    }
  }

  /** @param {FakeElement} child */
  appendChild(child) {
    if (child.parentNode) {
      child.parentNode.removeChild(child);
    }
    child.parentNode = this;
    if (!child.ownerDocument && this.ownerDocument) {
      child.ownerDocument = this.ownerDocument;
    }
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
    for (const child of this.children) {
      child.parentNode = null;
    }
    this.children = [];
    this._text = "";
    for (const node of nodes) {
      this.appendChild(node);
    }
  }

  remove() {
    if (this.parentNode) {
      this.parentNode.removeChild(this);
    }
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    if (!this._listeners.has(type)) {
      this._listeners.set(type, []);
    }
    this._listeners.get(type)?.push(listener);
  }

  /**
   * @param {string} type
   * @param {Record<string, any>} [extra]
   */
  dispatchEvent(type, extra = {}) {
    const event = {
      type,
      target: this,
      currentTarget: this,
      defaultPrevented: false,
      propagationStopped: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {
        this.propagationStopped = true;
      },
      ...extra,
    };
    /** @type {FakeElement | null} */
    let cursor = /** @type {FakeElement} */ (this);
    while (cursor) {
      event.currentTarget = /** @type {any} */ (cursor);
      for (const listener of cursor._listeners.get(type) ?? []) {
        listener(event);
      }
      if (event.propagationStopped) break;
      cursor = cursor.parentNode;
    }
    return !event.defaultPrevented;
  }

  /** @param {Record<string, any>} [extra] */
  click(extra = {}) {
    return this.dispatchEvent("click", extra);
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] ?? null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const out = [];
    /** @param {FakeElement} el */
    const walk = (el) => {
      for (const child of el.children) {
        if (matchesSelector(child, selector)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
}

/**
 * @param {FakeElement} el
 * @param {string} compound
 * @returns {boolean}
 */
function matchesCompoundSelector(el, compound) {
  const tokens = compound.match(/\[[^\]]+\]|[#.][\w-]+|[a-zA-Z][\w-]*/g) ?? [];
  if (tokens.length === 0) return false;
  return tokens.every((token) => {
    if (token.startsWith("#")) return el.getAttribute("id") === token.slice(1);
    if (token.startsWith(".")) {
      const classes = (el.getAttribute("class") ?? "").split(/\s+/).filter(Boolean);
      return classes.includes(token.slice(1));
    }
    if (token.startsWith("[")) {
      const m = token.slice(1, -1).match(/^([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?$/);
      if (!m || !m[1]) return false;
      const attrName = m[1];
      const expected = m[2] ?? m[3] ?? m[4];
      if (expected === undefined) return el.hasAttribute(attrName);
      return el.getAttribute(attrName) === expected;
    }
    return el.tagName === token.toUpperCase();
  });
}

/**
 * @param {FakeElement} el
 * @param {string} selector
 * @returns {boolean}
 */
function matchesSelector(el, selector) {
  /** @type {string[]} */
  const parts = [];
  let current = "";
  let inBrackets = 0;
  for (const ch of selector.trim()) {
    if (ch === "[") inBrackets += 1;
    if (ch === "]") inBrackets = Math.max(0, inBrackets - 1);
    if (/\s/.test(ch) && inBrackets === 0) {
      if (current) {
        parts.push(current);
        current = "";
      }
    } else {
      current += ch;
    }
  }
  if (current) parts.push(current);
  if (parts.length === 0) return false;

  const lastPart = parts[parts.length - 1];
  if (!lastPart || !matchesCompoundSelector(el, lastPart)) return false;

  /** @type {FakeElement | null} */
  let ancestor = el.parentNode;
  for (let i = parts.length - 2; i >= 0; i -= 1) {
    const part = parts[i];
    if (!part) return false;
    while (ancestor && !matchesCompoundSelector(ancestor, part)) {
      ancestor = ancestor.parentNode;
    }
    if (!ancestor) return false;
    ancestor = ancestor.parentNode;
  }
  return true;
}

function makeContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

/**
 * @param {FakeElement} root
 * @param {string} selector
 * @returns {FakeElement}
 */
function mustQuery(root, selector) {
  const el = root.querySelector(selector);
  assert.ok(el, `Не найден элемент по селектору: ${selector}`);
  return el;
}

/* ------------------------------------------------------------------ *
 * Тестовые фикстуры отношений (relationships)
 * ------------------------------------------------------------------ */

const SAMPLE_RELATIONSHIPS = [
  {
    userId: "u-friend-online-zoya",
    username: "zoya",
    displayName: "Зоя",
    type: 1,
    status: /** @type {const} */ ("online"),
  },
  {
    userId: "u-friend-dnd-anna",
    username: "anna",
    displayName: "Анна",
    type: 1,
    status: /** @type {const} */ ("dnd"),
  },
  {
    userId: "u-friend-idle-boris",
    username: "boris",
    displayName: "",
    type: 1,
    status: /** @type {const} */ ("idle"),
  },
  {
    userId: "u-friend-offline-viktor",
    username: "viktor",
    displayName: "Виктор",
    type: 1,
    status: /** @type {const} */ ("offline"),
  },
  {
    userId: "u-blocked-gleb",
    username: "gleb",
    displayName: "Глеб",
    type: 2,
    status: /** @type {const} */ ("online"),
  },
  {
    userId: "u-pending-in-daria",
    username: "daria",
    displayName: "Дарья",
    type: 3,
    status: /** @type {const} */ ("online"),
  },
  {
    userId: "u-pending-in-ivan",
    username: "ivan",
    displayName: "Иван",
    type: 3,
    status: /** @type {const} */ ("offline"),
  },
  {
    userId: "u-pending-out-elena",
    username: "elena",
    displayName: "Елена",
    type: 4,
    status: /** @type {const} */ ("idle"),
  },
];

/* ------------------------------------------------------------------ *
 * Константы и чистая фильтрация filterRelationshipsByTab
 * ------------------------------------------------------------------ */

describe("RELATIONSHIP_TYPES, FRIENDS_TABS_RU и filterRelationshipsByTab (FE-P1-25)", () => {
  it("экспортирует замороженные константы RELATIONSHIP_TYPES и FRIENDS_TABS_RU", () => {
    assert.equal(Object.isFrozen(RELATIONSHIP_TYPES), true);
    assert.deepEqual(RELATIONSHIP_TYPES, {
      FRIEND: 1,
      BLOCKED: 2,
      PENDING_INCOMING: 3,
      PENDING_OUTGOING: 4,
    });

    assert.equal(Object.isFrozen(FRIENDS_TABS_RU), true);
    assert.deepEqual(FRIENDS_TABS_RU, {
      online: "В сети",
      all: "Все",
      pending: "Ожидание",
      blocked: "Заблокированные",
      add: "Добавить в друзья",
    });
  });

  it("фильтрует вкладку 'online' (только type === 1 и status !== 'offline') и сортирует по алфавиту (displayName || username)", () => {
    const result = filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "online", "");
    assert.deepEqual(
      result.map((r) => r.userId),
      [
        "u-friend-dnd-anna", // "Анна"
        "u-friend-online-zoya", // "Зоя"
        "u-friend-idle-boris", // "boris" (fallback на username, латиница после кириллицы в ru)
      ],
    );
  });

  it("фильтрует вкладку 'all' (все type === 1 включая offline), 'pending' (type === 3 и type === 4) и 'blocked' (type === 2)", () => {
    const allFriends = filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "all", "");
    assert.deepEqual(
      allFriends.map((r) => r.userId),
      [
        "u-friend-dnd-anna",
        "u-friend-offline-viktor",
        "u-friend-online-zoya",
        "u-friend-idle-boris",
      ],
    );

    const pending = filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "pending", "");
    assert.deepEqual(
      pending.map((r) => r.userId),
      [
        "u-pending-in-daria",
        "u-pending-out-elena",
        "u-pending-in-ivan",
      ],
    );

    const blocked = filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "blocked", "");
    assert.deepEqual(
      blocked.map((r) => r.userId),
      ["u-blocked-gleb"],
    );

    assert.deepEqual(filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "add", ""), []);
  });

  it("фильтрует по регистронезависимому searchQuery на (displayName || username) и не мутирует входной массив", () => {
    const snapshot = SAMPLE_RELATIONSHIPS.map((r) => ({ ...r }));

    const byDisplay = filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "all", "  виКт  ");
    assert.deepEqual(
      byDisplay.map((r) => r.userId),
      ["u-friend-offline-viktor"],
    );

    const byFallbackUsername = filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "online", "BOR");
    assert.deepEqual(
      byFallbackUsername.map((r) => r.userId),
      ["u-friend-idle-boris"],
    );

    assert.deepEqual(filterRelationshipsByTab(SAMPLE_RELATIONSHIPS, "all", "несуществующий"), []);
    assert.deepEqual(SAMPLE_RELATIONSHIPS, snapshot);
  });
});

/* ------------------------------------------------------------------ *
 * DOM-контроллер createFriendsHomeController
 * ------------------------------------------------------------------ */

describe("createFriendsHomeController — вкладки, бейдж заявок, поиск и действия (FE-P1-25)", () => {
  it("рендерит [data-friends-home], вкладки [data-friends-tab] с aria-selected, бейдж входящих заявок [data-friends-pending-badge] (count of type === 3), поиск [data-friends-search] и список [data-friends-list]", () => {
    const container = makeContainer();
    const controller = createFriendsHomeController({
      container,
      relationships: SAMPLE_RELATIONSHIPS,
    });

    const root = mustQuery(container, "[data-friends-home]");
    const tabs = root.querySelectorAll("[data-friends-tab]");
    assert.equal(tabs.length, 5);
    assert.deepEqual(
      tabs.map((t) => ({
        id: t.getAttribute("data-friends-tab"),
        selected: t.getAttribute("aria-selected"),
      })),
      [
        { id: "online", selected: "true" },
        { id: "all", selected: "false" },
        { id: "pending", selected: "false" },
        { id: "blocked", selected: "false" },
        { id: "add", selected: "false" },
      ],
    );

    // Входящих заявок (type === 3) ровно 2 (Дарья и Иван); исходящая Елена (type === 4) в бейдж не входит
    const badge = mustQuery(root, "[data-friends-pending-badge]");
    assert.equal(badge.textContent, "2");

    const searchInput = mustQuery(root, "[data-friends-search]");
    assert.ok(searchInput);

    const list = mustQuery(root, "[data-friends-list]");
    const rows = list.querySelectorAll("[data-friend-row]");
    assert.equal(rows.length, 3); // В сети: boris, Анна, Зоя

    assert.ok(controller);
  });

  it("переключает вкладки, фильтрует строки по поиску [data-friends-search] и вызывает onOpenDm, onAcceptRequest и onRemoveOrBlock по кликам на кнопки действий", () => {
    const container = makeContainer();
    /** @type {string[]} */
    const openedDms = [];
    /** @type {string[]} */
    const acceptedRequests = [];
    /** @type {string[]} */
    const removedOrBlocked = [];

    const controller = createFriendsHomeController({
      container,
      relationships: SAMPLE_RELATIONSHIPS,
      onOpenDm: (userId) => openedDms.push(userId),
      onAcceptRequest: (userId) => acceptedRequests.push(userId),
      onRemoveOrBlock: (userId) => removedOrBlocked.push(userId),
    });

    const root = mustQuery(container, "[data-friends-home]");

    // Клик по кнопке ЛС на вкладке 'online'
    const firstOnlineRow = mustQuery(root, '[data-friend-row][data-user-id="u-friend-dnd-anna"]');
    assert.ok(firstOnlineRow.textContent.includes("Анна"));
    assert.ok(firstOnlineRow.textContent.includes("dnd"));
    mustQuery(firstOnlineRow, "[data-friend-dm-btn]").click();
    assert.deepEqual(openedDms, ["u-friend-dnd-anna"]);

    // Переключаемся на вкладку 'pending' кликом по кнопке вкладки
    const pendingTabBtn = mustQuery(root, '[data-friends-tab="pending"]');
    pendingTabBtn.click();
    assert.equal(pendingTabBtn.getAttribute("aria-selected"), "true");
    assert.equal(
      mustQuery(root, '[data-friends-tab="online"]').getAttribute("aria-selected"),
      "false",
    );

    const pendingRows = root.querySelectorAll("[data-friend-row]");
    assert.equal(pendingRows.length, 3);

    // Принимаем входящую заявку Дарьи (type === 3)
    const dariaRow = mustQuery(root, '[data-friend-row][data-user-id="u-pending-in-daria"]');
    mustQuery(dariaRow, "[data-friend-accept-btn]").click();
    assert.deepEqual(acceptedRequests, ["u-pending-in-daria"]);

    // Отменяем/удаляем заявку Елены через [data-friend-remove-btn]
    const elenaRow = mustQuery(root, '[data-friend-row][data-user-id="u-pending-out-elena"]');
    mustQuery(elenaRow, "[data-friend-remove-btn]").click();
    assert.deepEqual(removedOrBlocked, ["u-pending-out-elena"]);

    // Фильтрация через поле поиска [data-friends-search]
    const searchInput = mustQuery(root, "[data-friends-search]");
    searchInput.value = "иван";
    searchInput.dispatchEvent("input");

    const filteredRows = root.querySelectorAll("[data-friend-row]");
    assert.equal(filteredRows.length, 1);
    assert.equal(filteredRows[0]?.getAttribute("data-user-id"), "u-pending-in-ivan");

    // Программное обновление через setRelationships пересчитывает бейдж
    controller.setRelationships([
      {
        userId: "u-friend-online-zoya",
        username: "zoya",
        displayName: "Зоя",
        type: 1,
        status: "online",
      },
    ]);
    assert.equal(mustQuery(root, "[data-friends-pending-badge]").textContent, "0");
  });

  it("на вкладке 'add' рендерит форму [data-add-friend-form] с [data-add-friend-input] и [data-add-friend-submit], вызывая onSendFriendRequest(username.trim())", () => {
    const container = makeContainer();
    /** @type {string[]} */
    const sentRequests = [];

    createFriendsHomeController({
      container,
      relationships: SAMPLE_RELATIONSHIPS,
      onSendFriendRequest: (username) => sentRequests.push(username),
    });

    const root = mustQuery(container, "[data-friends-home]");
    mustQuery(root, '[data-friends-tab="add"]').click();

    assert.equal(
      mustQuery(root, '[data-friends-tab="add"]').getAttribute("aria-selected"),
      "true",
    );

    const form = mustQuery(root, "[data-add-friend-form]");
    const input = mustQuery(form, "[data-add-friend-input]");
    const submitBtn = mustQuery(form, "[data-add-friend-submit]");

    // Пустой ввод не отправляет запрос
    input.value = "   ";
    submitBtn.click();
    assert.deepEqual(sentRequests, []);

    // Валидный ввод с пробелами по краям обрезается через trim()
    input.value = "  new_friend#1234  ";
    submitBtn.click();
    assert.deepEqual(sentRequests, ["new_friend#1234"]);
  });
});
