// @ts-check
/**
 * Unit-тесты контроллера управления ролями сервера и матрицы прав (task-248 / FE-P0-12).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  ROLE_PERMISSIONS_CATALOG_RU,
  sortRolesByPositionDesc,
  createRoleEditorController,
} from "../../rust/gui-spike/ui/features/role-editor.js";

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
    this._value = "";
    this._disabled = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {string} */
  get value() {
    return this._value;
  }

  /** @param {string} val */
  set value(val) {
    this._value = String(val ?? "");
    this._attrs.set("value", this._value);
  }

  /** @returns {boolean} */
  get disabled() {
    return this._disabled || this._attrs.has("disabled");
  }

  /** @param {boolean} val */
  set disabled(val) {
    this._disabled = Boolean(val);
    if (this._disabled) {
      this._attrs.set("disabled", "");
    } else {
      this._attrs.delete("disabled");
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
    for (const child of this.children) {
      child.parentNode = null;
    }
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
    if (name === "value") this._value = str;
    if (name === "disabled") this._disabled = true;
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
    if (name === "disabled") this._disabled = false;
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
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {},
      ...extra,
    };
    for (const listener of this._listeners.get(type) ?? []) {
      listener(event);
    }
    return !event.defaultPrevented;
  }

  click() {
    if (this.disabled) return false;
    return this.dispatchEvent("click");
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
 * @param {string} selector
 * @returns {boolean}
 */
function matchesSelector(el, selector) {
  const tokens = selector.match(/\[[^\]]+\]|[#.][\w-]+|[a-zA-Z][\w-]*/g) ?? [];
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

function makeContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

describe("ROLE_PERMISSIONS_CATALOG_RU", () => {
  it("содержит замороженный каталог из 6 прав с русскими подписями и битовыми масками", () => {
    assert.equal(Object.isFrozen(ROLE_PERMISSIONS_CATALOG_RU), true);
    assert.deepEqual(ROLE_PERMISSIONS_CATALOG_RU, [
      { key: "ADMINISTRATOR", bit: 8, label: "Администратор" },
      { key: "MANAGE_GUILD", bit: 32, label: "Управление сервером" },
      { key: "MANAGE_ROLES", bit: 268435456, label: "Управление ролями" },
      { key: "MANAGE_CHANNELS", bit: 16, label: "Управление каналами" },
      { key: "KICK_MEMBERS", bit: 2, label: "Выгонять участников" },
      { key: "BAN_MEMBERS", bit: 4, label: "Банить участников" },
    ]);
  });
});

describe("sortRolesByPositionDesc", () => {
  it("сортирует роли по убыванию position и при равенстве по String(id) по возрастанию без мутации входа", () => {
    const input = [
      { id: "role-b", name: "Модератор", position: 5, color: "#3498db", hoist: true, permissions: 6 },
      { id: "role-c", name: "Участник", position: 1, color: "#99aab5", hoist: false, permissions: 0 },
      { id: "role-a", name: "Куратор", position: 5, color: "#2ecc71", hoist: true, permissions: 16 },
      { id: "role-d", name: "Админ", position: 10, color: "#e74c3c", hoist: true, permissions: 8 },
    ];
    const snapshot = input.map((r) => ({ ...r }));

    const sorted = sortRolesByPositionDesc(input);

    assert.deepEqual(
      sorted.map((r) => r.id),
      ["role-d", "role-a", "role-b", "role-c"],
    );
    assert.deepEqual(input, snapshot, "входной массив не должен мутироваться");
    assert.notEqual(sorted[0], input[3], "элементы должны клонироваться");
  });

  it("безопасно обрабатывает роли без position и невалидный вход", () => {
    const roles = [
      { id: "20", name: "Вторая" },
      { id: "10", name: "Первая" },
      { id: "30", name: "Высокая", position: 3 },
    ];
    const sorted = sortRolesByPositionDesc(roles);
    assert.deepEqual(
      sorted.map((r) => r.id),
      ["30", "10", "20"],
    );
    assert.deepEqual(sortRolesByPositionDesc(/** @type {any} */ (null)), []);
  });
});

describe("createRoleEditorController", () => {
  const SAMPLE_ROLES = [
    {
      id: "r-mod",
      name: "Модератор",
      position: 5,
      color: "#3498db",
      hoist: true,
      permissions: 2 | 4, // KICK_MEMBERS | BAN_MEMBERS = 6
    },
    {
      id: "r-admin",
      name: "Основатель",
      position: 10,
      color: "#e74c3c",
      hoist: true,
      permissions: 8, // ADMINISTRATOR
    },
    {
      id: "r-member",
      name: "Участник",
      position: 1,
      color: "#99aab5",
      hoist: false,
      permissions: 0,
    },
  ];

  it("рендерит [data-role-editor], отсортированный список ролей и контролы выбранной роли", () => {
    const container = makeContainer();
    createRoleEditorController({
      container,
      roles: SAMPLE_ROLES,
      selectedRoleId: "r-mod",
    });

    const root = container.querySelector("[data-role-editor]");
    assert.ok(root, "регион [data-role-editor] должен существовать");
    assert.equal(root.getAttribute("role"), "region");
    assert.equal(root.getAttribute("aria-label"), "Редактор ролей");

    const roleItems = container.querySelectorAll("[data-role-item-id]");
    assert.deepEqual(
      roleItems.map((el) => el.getAttribute("data-role-item-id")),
      ["r-admin", "r-mod", "r-member"],
    );

    const nameInput = container.querySelector("[data-role-name-input]");
    const colorInput = container.querySelector("[data-role-color-input]");
    const hoistToggle = container.querySelector("[data-role-hoist-toggle]");
    assert.ok(nameInput);
    assert.ok(colorInput);
    assert.ok(hoistToggle);
    assert.equal(nameInput.value, "Модератор");
    assert.equal(colorInput.value, "#3498db");
    assert.equal(hoistToggle.getAttribute("role"), "switch");
    assert.equal(hoistToggle.getAttribute("aria-checked"), "true");

    const kickPerm = container.querySelector('[data-role-perm-toggle="KICK_MEMBERS"]');
    const banPerm = container.querySelector('[data-role-perm-toggle="BAN_MEMBERS"]');
    const adminPerm = container.querySelector('[data-role-perm-toggle="ADMINISTRATOR"]');
    const manageRolesPerm = container.querySelector('[data-role-perm-toggle="MANAGE_ROLES"]');
    assert.ok(kickPerm);
    assert.ok(banPerm);
    assert.ok(adminPerm);
    assert.ok(manageRolesPerm);
    assert.equal(kickPerm.getAttribute("role"), "switch");
    assert.equal(kickPerm.getAttribute("aria-checked"), "true");
    assert.equal(banPerm.getAttribute("aria-checked"), "true");
    assert.equal(adminPerm.getAttribute("aria-checked"), "false");
    assert.equal(manageRolesPerm.getAttribute("aria-checked"), "false");

    const saveBtn = container.querySelector("[data-role-save-btn]");
    assert.ok(saveBtn);
    assert.equal(saveBtn.textContent, "Сохранить изменения");
  });

  it("переключает выбранную роль по клику в сайдбаре и через selectRole(roleId)", () => {
    const container = makeContainer();
    const ctrl = createRoleEditorController({
      container,
      roles: SAMPLE_ROLES,
    });

    // По умолчанию выбирается первая роль в отсортированном порядке (r-admin)
    assert.equal(container.querySelector("[data-role-name-input]")?.value, "Основатель");
    assert.equal(
      container.querySelector('[data-role-perm-toggle="ADMINISTRATOR"]')?.getAttribute("aria-checked"),
      "true",
    );

    // Клик по элементу списка ролей
    const memberItem = container.querySelector('[data-role-item-id="r-member"]');
    assert.ok(memberItem);
    memberItem.click();

    assert.equal(container.querySelector("[data-role-name-input]")?.value, "Участник");
    assert.equal(container.querySelector("[data-role-hoist-toggle]")?.getAttribute("aria-checked"), "false");
    assert.equal(
      container.querySelector('[data-role-perm-toggle="ADMINISTRATOR"]')?.getAttribute("aria-checked"),
      "false",
    );

    // Программный выбор через selectRole
    ctrl.selectRole("r-mod");
    assert.equal(container.querySelector("[data-role-name-input]")?.value, "Модератор");
    assert.equal(container.querySelector("[data-role-color-input]")?.value, "#3498db");
  });

  it("переключает биты прав и флаг hoist в черновике и вызывает onSaveRole с обрезанным именем", () => {
    const container = makeContainer();
    /** @type {Array<{ id: string, name: string, color: string, hoist: boolean, permissions: number }>} */
    const savedRoles = [];

    createRoleEditorController({
      container,
      roles: SAMPLE_ROLES,
      selectedRoleId: "r-mod",
      onSaveRole: (role) => savedRoles.push(role),
    });

    const nameInput = container.querySelector("[data-role-name-input]");
    const colorInput = container.querySelector("[data-role-color-input]");
    const hoistToggle = container.querySelector("[data-role-hoist-toggle]");
    assert.ok(nameInput);
    assert.ok(colorInput);
    assert.ok(hoistToggle);

    nameInput.value = "  Старший модератор  ";
    nameInput.dispatchEvent("input");
    colorInput.value = "#1abc9c";
    colorInput.dispatchEvent("input");

    // Выключаем hoist (true -> false)
    hoistToggle.click();
    assert.equal(
      container.querySelector("[data-role-hoist-toggle]")?.getAttribute("aria-checked"),
      "false",
    );

    // Снимаем KICK_MEMBERS (bit 2) и включаем MANAGE_CHANNELS (bit 16) и MANAGE_ROLES (bit 268435456)
    container.querySelector('[data-role-perm-toggle="KICK_MEMBERS"]')?.click();
    container.querySelector('[data-role-perm-toggle="MANAGE_CHANNELS"]')?.click();
    container.querySelector('[data-role-perm-toggle="MANAGE_ROLES"]')?.click();

    assert.equal(
      container.querySelector('[data-role-perm-toggle="KICK_MEMBERS"]')?.getAttribute("aria-checked"),
      "false",
    );
    assert.equal(
      container.querySelector('[data-role-perm-toggle="BAN_MEMBERS"]')?.getAttribute("aria-checked"),
      "true",
    );
    assert.equal(
      container.querySelector('[data-role-perm-toggle="MANAGE_CHANNELS"]')?.getAttribute("aria-checked"),
      "true",
    );
    assert.equal(
      container.querySelector('[data-role-perm-toggle="MANAGE_ROLES"]')?.getAttribute("aria-checked"),
      "true",
    );

    const saveBtn = container.querySelector("[data-role-save-btn]");
    assert.ok(saveBtn);
    saveBtn.click();

    assert.deepEqual(savedRoles, [
      {
        id: "r-mod",
        name: "Старший модератор",
        color: "#1abc9c",
        hoist: false,
        permissions: 4 | 16 | 268435456,
      },
    ]);
  });

  it("валидирует длину имени роли (1..100 символов после trim) перед вызовом onSaveRole", () => {
    const container = makeContainer();
    /** @type {Array<any>} */
    const savedRoles = [];

    createRoleEditorController({
      container,
      roles: SAMPLE_ROLES,
      selectedRoleId: "r-member",
      onSaveRole: (role) => savedRoles.push(role),
    });

    const nameInput = container.querySelector("[data-role-name-input]");
    const saveBtn = container.querySelector("[data-role-save-btn]");
    assert.ok(nameInput);
    assert.ok(saveBtn);

    // Пустое имя после trim
    nameInput.value = "    ";
    nameInput.dispatchEvent("input");
    saveBtn.click();
    assert.equal(savedRoles.length, 0);

    // Слишком длинное имя (> 100 символов)
    nameInput.value = "А".repeat(101);
    nameInput.dispatchEvent("input");
    saveBtn.click();
    assert.equal(savedRoles.length, 0);

    // Граничное валидное имя (100 символов)
    const valid100 = "Б".repeat(100);
    nameInput.value = `  ${valid100}  `;
    nameInput.dispatchEvent("input");
    saveBtn.click();
    assert.equal(savedRoles.length, 1);
    assert.equal(savedRoles[0].name, valid100);
  });

  it("поддерживает setState, getState и destroy", () => {
    const container = makeContainer();
    const ctrl = createRoleEditorController({
      container,
      roles: SAMPLE_ROLES,
      selectedRoleId: "r-member",
    });

    assert.equal(ctrl.getState().selectedRoleId, "r-member");

    ctrl.setState({
      roles: [
        { id: "r-new", name: "Новая роль", position: 20, color: "#f1c40f", hoist: true, permissions: 32 },
      ],
      selectedRoleId: "r-new",
    });

    assert.equal(ctrl.getState().selectedRoleId, "r-new");
    assert.equal(container.querySelector("[data-role-name-input]")?.value, "Новая роль");
    assert.equal(
      container.querySelector('[data-role-perm-toggle="MANAGE_GUILD"]')?.getAttribute("aria-checked"),
      "true",
    );

    ctrl.destroy();
    assert.equal(container.querySelector("[data-role-editor]"), null);
  });
});
