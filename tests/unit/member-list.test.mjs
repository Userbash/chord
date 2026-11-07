// @ts-check
/**
 * T-220 (FE-P1-20 / RW-40 / G2): unit-тесты контроллера списка участников гильдии,
 * ролевых групп и сайдбара присутствия (`rust/gui-spike/ui/features/member-list.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  DEFAULT_GROUP_LABELS_RU,
  groupMembersByHoistedRole,
  createMemberListController,
} from "../../rust/gui-spike/ui/features/member-list.js";

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных unit-тестов
 * ------------------------------------------------------------------ */

class FakeStyle {
  /**
   * @param {FakeElement} owner
   */
  constructor(owner) {
    this._owner = owner;
    this._color = "";
    /** @type {Map<string, string>} */
    this._props = new Map();
  }

  /** @returns {string} */
  get color() {
    return this._color;
  }

  /** @param {string} val */
  set color(val) {
    this._color = String(val ?? "");
    if (this._color) {
      this._props.set("color", this._color);
    } else {
      this._props.delete("color");
    }
    this._syncAttr();
  }

  /**
   * @param {string} prop
   * @param {string} val
   */
  setProperty(prop, val) {
    const str = String(val ?? "");
    if (prop === "color") {
      this._color = str;
    }
    if (str) {
      this._props.set(prop, str);
    } else {
      this._props.delete(prop);
    }
    this._syncAttr();
  }

  /**
   * @param {string} prop
   * @returns {string}
   */
  getPropertyValue(prop) {
    return this._props.get(prop) ?? "";
  }

  _syncAttr() {
    if (this._props.size === 0) {
      this._owner._attrs.delete("style");
      return;
    }
    const serialized = Array.from(this._props.entries())
      .map(([k, v]) => `${k}: ${v}`)
      .join("; ");
    this._owner._attrs.set("style", serialized);
  }
}

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
    this.style = new FakeStyle(this);
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
    if (name === "style") {
      const m = str.match(/color\s*:\s*([^;]+)/i);
      if (m && m[1]) {
        this.style._color = m[1].trim();
        this.style._props.set("color", this.style._color);
      }
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

const SAMPLE_ROLES = [
  { id: "r-everyone", name: "@everyone", position: 0, hoist: false, color: 0 },
  { id: "r-vip", name: "VIP", position: 2, hoist: false, color: 0x00aaff },
  { id: "r-mod", name: "Модераторы", position: 5, hoist: true, color: 0x3498db },
  { id: "r-admin", name: "Админы", position: 10, hoist: true, color: 0xe91e63 },
  { id: "r-empty-hoist", name: "Пустая роль", position: 8, hoist: true, color: 0x2ecc71 },
];

const SAMPLE_MEMBERS = [
  {
    userId: "u-admin-2",
    username: "zoya",
    displayName: "Яна",
    status: /** @type {const} */ ("online"),
    roleIds: ["r-mod", "r-admin"],
    customStatus: "Деплою прод",
    isOwner: true,
  },
  {
    userId: "u-admin-1",
    username: "anton",
    displayName: "Антон",
    status: /** @type {const} */ ("dnd"),
    roleIds: ["r-admin"],
  },
  {
    userId: "u-mod-1",
    username: "boris",
    displayName: "Борис",
    status: /** @type {const} */ ("idle"),
    roleIds: ["r-mod", "r-vip"],
    customStatus: "Отошёл на 5 минут",
  },
  {
    userId: "u-online-2",
    username: "viktor",
    displayName: "",
    status: /** @type {const} */ ("online"),
    roleIds: ["r-vip"],
  },
  {
    userId: "u-online-1",
    username: "alex",
    displayName: "Алексей",
    status: /** @type {const} */ ("online"),
    roleIds: [],
  },
  {
    userId: "u-offline-2",
    username: "dmitry",
    displayName: "Дмитрий",
    status: /** @type {const} */ ("offline"),
    roleIds: ["r-admin"],
  },
  {
    userId: "u-offline-1",
    username: "gleb",
    displayName: "Глеб",
    status: /** @type {any} */ (undefined),
    roleIds: [],
  },
];

describe("DEFAULT_GROUP_LABELS_RU и groupMembersByHoistedRole (FE-P1-20)", () => {
  it("DEFAULT_GROUP_LABELS_RU заморожен и содержит русские подписи 'В сети' и 'Не в сети'", () => {
    assert.equal(Object.isFrozen(DEFAULT_GROUP_LABELS_RU), true);
    assert.deepEqual(DEFAULT_GROUP_LABELS_RU, {
      online: "В сети",
      offline: "Не в сети",
    });
  });

  it("группирует участников по старшей hoisted-роли (position по убыванию), 'online' и 'offline', сортируя внутри групп по localeCompare('ru')", () => {
    const groups = groupMembersByHoistedRole(SAMPLE_MEMBERS, SAMPLE_ROLES);

    assert.deepEqual(
      groups.map((g) => ({
        id: g.id,
        label: g.label,
        count: g.count,
        userIds: g.members.map((m) => m.userId),
      })),
      [
        {
          id: "r-admin",
          label: "Админы",
          count: 2,
          userIds: ["u-admin-1", "u-admin-2"], // Антон, Яна
        },
        {
          id: "r-mod",
          label: "Модераторы",
          count: 1,
          userIds: ["u-mod-1"], // Борис
        },
        {
          id: "online",
          label: "В сети",
          count: 2,
          userIds: ["u-online-1", "u-online-2"], // Алексей, viktor (fallback на username)
        },
        {
          id: "offline",
          label: "Не в сети",
          count: 2,
          userIds: ["u-offline-1", "u-offline-2"], // Глеб, Дмитрий (даже с r-admin уходит в offline)
        },
      ],
    );
  });

  it("исключает пустые группы и не мутирует входные массивы members и roles", () => {
    const membersCopy = SAMPLE_MEMBERS.map((m) => ({ ...m, roleIds: [...m.roleIds] }));
    const rolesCopy = SAMPLE_ROLES.map((r) => ({ ...r }));

    const onlyOnline = [
      {
        userId: "u-1",
        username: "ivan",
        displayName: "Иван",
        status: /** @type {const} */ ("online"),
        roleIds: [],
      },
    ];
    const groups = groupMembersByHoistedRole(onlyOnline, SAMPLE_ROLES);
    assert.equal(groups.length, 1);
    assert.equal(groups[0]?.id, "online");
    assert.equal(groups[0]?.label, "В сети");
    assert.equal(groups[0]?.count, 1);

    assert.deepEqual(groupMembersByHoistedRole([], SAMPLE_ROLES), []);
    assert.deepEqual(groupMembersByHoistedRole(/** @type {any} */ (null), []), []);
    assert.deepEqual(SAMPLE_MEMBERS, membersCopy);
    assert.deepEqual(SAMPLE_ROLES, rolesCopy);
  });
});

describe("createMemberListController — рендер, клик по участнику и toggleVisibility", () => {
  it("рендерит [data-member-list], заголовки групп [data-member-group-header] ('NAME — COUNT') и элементы [data-member-item] с цветом роли, статусом, короной владельца и customStatus", () => {
    const container = makeContainer();
    const controller = createMemberListController({
      container,
      members: SAMPLE_MEMBERS,
      roles: SAMPLE_ROLES,
    });

    const listEl = mustQuery(container, "[data-member-list]");
    assert.ok(
      listEl.tagName === "ASIDE" || listEl.tagName === "SECTION",
      "Корневой элемент должен быть <aside> или <section>",
    );
    assert.equal(listEl.hidden, false);

    const headers = listEl.querySelectorAll("[data-member-group-header]");
    assert.deepEqual(
      headers.map((h) => h.textContent),
      ["Админы — 2", "Модераторы — 1", "В сети — 2", "Не в сети — 2"],
    );

    const items = listEl.querySelectorAll("[data-member-item]");
    assert.equal(items.length, 7);

    // Проверим владельца (Яна, u-admin-2): цвет #e91e63, статус online, корона [data-owner-badge], customStatus
    const ownerItem = mustQuery(listEl, '[data-user-id="u-admin-2"]');
    assert.ok(ownerItem.textContent.includes("Яна"));
    assert.ok(ownerItem.textContent.includes("Деплою прод"));
    assert.equal(
      ownerItem.style.color || ownerItem.querySelector("[data-member-name]")?.style.color,
      "#e91e63",
    );
    assert.equal(
      mustQuery(ownerItem, "[data-status]").getAttribute("data-status"),
      "online",
    );
    assert.ok(
      ownerItem.querySelector("[data-owner-badge]"),
      "Для isOwner === true должен рендериться [data-owner-badge]",
    );

    // Проверим участника без isOwner (Антон, u-admin-1): короны нет, статус dnd
    const admin1Item = mustQuery(listEl, '[data-user-id="u-admin-1"]');
    assert.equal(admin1Item.querySelector("[data-owner-badge]"), null);
    assert.equal(
      mustQuery(admin1Item, "[data-status]").getAttribute("data-status"),
      "dnd",
    );

    // Проверим участника с не-hoisted цветной ролью VIP (viktor, u-online-2): в группе 'В сети', но цвет #00aaff (с ведущими нулями)
    const vipOnlineItem = mustQuery(listEl, '[data-user-id="u-online-2"]');
    assert.ok(vipOnlineItem.textContent.includes("viktor"));
    assert.equal(
      vipOnlineItem.style.color || vipOnlineItem.querySelector("[data-member-name]")?.style.color,
      "#00aaff",
    );

    // Проверим участника без цветных ролей (Алексей, u-online-1): цвет не задан
    const plainOnlineItem = mustQuery(listEl, '[data-user-id="u-online-1"]');
    assert.equal(plainOnlineItem.style.color, "");

    assert.ok(controller);
  });

  it("поддерживает динамическое обновление списка через controller.render({ members, roles })", () => {
    const container = makeContainer();
    const controller = createMemberListController({ container });

    const listEl = mustQuery(container, "[data-member-list]");
    assert.equal(listEl.querySelectorAll("[data-member-item]").length, 0);

    controller.render({ members: SAMPLE_MEMBERS, roles: SAMPLE_ROLES });
    assert.equal(listEl.querySelectorAll("[data-member-item]").length, 7);
    assert.deepEqual(
      listEl.querySelectorAll("[data-member-group-header]").map((h) => h.textContent),
      ["Админы — 2", "Модераторы — 1", "В сети — 2", "Не в сети — 2"],
    );
  });

  it("клик по элементу участника [data-member-item] вызывает onSelectMember({ userId, anchorElement })", () => {
    const container = makeContainer();
    /** @type {Array<{ userId: string, anchorElement: any }>} */
    const selected = [];

    createMemberListController({
      container,
      members: SAMPLE_MEMBERS,
      roles: SAMPLE_ROLES,
      onSelectMember: (payload) => selected.push(payload),
    });

    const modItem = mustQuery(container, '[data-user-id="u-mod-1"]');
    modItem.click();

    assert.equal(selected.length, 1);
    assert.equal(selected[0]?.userId, "u-mod-1");
    assert.equal(selected[0]?.anchorElement, modItem);
  });

  it("toggleVisibility() переключает видимость сайдбара, синхронизирует атрибут hidden и возвращает boolean состояние", () => {
    const container = makeContainer();
    const controller = createMemberListController({
      container,
      members: SAMPLE_MEMBERS,
      roles: SAMPLE_ROLES,
    });

    const listEl = mustQuery(container, "[data-member-list]");
    assert.equal(listEl.hidden, false);
    assert.equal(listEl.hasAttribute("hidden"), false);

    const afterHide = controller.toggleVisibility();
    assert.equal(afterHide, false);
    assert.equal(listEl.hidden, true);
    assert.equal(listEl.hasAttribute("hidden"), true);

    const afterShow = controller.toggleVisibility();
    assert.equal(afterShow, true);
    assert.equal(listEl.hidden, false);
    assert.equal(listEl.hasAttribute("hidden"), false);
  });
});
