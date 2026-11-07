// @ts-check
/**
 * T-227 (FE-P1-23 / RW-40 / G2): unit-тесты контроллера контекстного меню
 * и клавиатурной навигации для сообщений, каналов и пользователей
 * (`rust/gui-spike/ui/features/context-menu.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  buildContextMenuItems,
  clampMenuCoordinates,
  createContextMenuController,
} from "../../rust/gui-spike/ui/features/context-menu.js";

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных unit-тестов
 * ------------------------------------------------------------------ */

class FakeElement {
  /**
   * @param {string} tagName
   * @param {FakeDocument | null} [ownerDocument]
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
    this.hidden = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
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
    if (name === "hidden") {
      this.hidden = true;
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
      this.hidden = false;
    }
  }

  /**
   * @param {FakeElement} child
   * @returns {FakeElement}
   */
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

  /**
   * @param {FakeElement} child
   * @returns {FakeElement}
   */
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

  focus() {
    if (this.ownerDocument) {
      this.ownerDocument.activeElement = this;
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
   * @returns {boolean}
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
    const results = [];
    const visit = (/** @type {FakeElement} */ node) => {
      for (const child of node.children) {
        if (matchesSimpleSelector(child, selector)) {
          results.push(child);
        }
        visit(child);
      }
    };
    visit(this);
    return results;
  }
}

class FakeDocument {
  constructor() {
    /** @type {FakeElement | null} */
    this.activeElement = null;
    this.body = new FakeElement("body", this);
  }

  /**
   * @param {string} tagName
   * @returns {FakeElement}
   */
  createElement(tagName) {
    return new FakeElement(tagName, this);
  }
}

/**
 * Поддержка простых селекторов вида `tag`, `[attr]`, `[attr="val"]` и их комбинаций.
 *
 * @param {FakeElement} el
 * @param {string} selector
 * @returns {boolean}
 */
function matchesSimpleSelector(el, selector) {
  const trimmed = selector.trim();
  if (!trimmed) return false;
  const attrPattern = /\[([a-zA-Z0-9_-]+)(?:="([^"]*)")?\]/g;
  const tagPart = trimmed.replace(attrPattern, "").trim();
  if (tagPart && el.tagName !== tagPart.toUpperCase()) {
    return false;
  }
  let match;
  while ((match = attrPattern.exec(trimmed)) !== null) {
    const attrName = match[1];
    const expectedVal = match[2];
    if (!attrName || !el.hasAttribute(attrName)) {
      return false;
    }
    if (expectedVal !== undefined && el.getAttribute(attrName) !== expectedVal) {
      return false;
    }
  }
  return true;
}

/* ------------------------------------------------------------------ *
 * Тесты buildContextMenuItems и clampMenuCoordinates
 * ------------------------------------------------------------------ */

describe("buildContextMenuItems", () => {
  it("формирует базовые пункты для чужого сообщения без прав модерации", () => {
    const items = buildContextMenuItems({
      targetType: "message",
      isOwnMessage: false,
      isPinned: false,
      canManageMessages: false,
    });
    assert.deepEqual(items, [
      { id: "reply", label: "Ответить", danger: false },
      { id: "copy-text", label: "Скопировать текст", danger: false },
      { id: "copy-id", label: "Скопировать ID сообщения", danger: false },
    ]);
  });

  it("добавляет edit и delete (danger: true) для собственного сообщения без прав модерации", () => {
    const items = buildContextMenuItems({
      targetType: "message",
      isOwnMessage: true,
      isPinned: false,
      canManageMessages: false,
    });
    assert.deepEqual(items, [
      { id: "reply", label: "Ответить", danger: false },
      { id: "copy-text", label: "Скопировать текст", danger: false },
      { id: "edit", label: "Изменить сообщение", danger: false },
      { id: "delete", label: "Удалить сообщение", danger: true },
      { id: "copy-id", label: "Скопировать ID сообщения", danger: false },
    ]);
  });

  it("добавляет pin и delete для модератора на чужом сообщении и учитывает флаг isPinned", () => {
    const unpinnedItems = buildContextMenuItems({
      targetType: "message",
      isOwnMessage: false,
      isPinned: false,
      canManageMessages: true,
    });
    assert.deepEqual(unpinnedItems, [
      { id: "reply", label: "Ответить", danger: false },
      { id: "copy-text", label: "Скопировать текст", danger: false },
      { id: "pin", label: "Закрепить сообщение", danger: false },
      { id: "delete", label: "Удалить сообщение", danger: true },
      { id: "copy-id", label: "Скопировать ID сообщения", danger: false },
    ]);

    const pinnedOwnItems = buildContextMenuItems({
      targetType: "message",
      isOwnMessage: true,
      isPinned: true,
      canManageMessages: true,
    });
    assert.deepEqual(pinnedOwnItems, [
      { id: "reply", label: "Ответить", danger: false },
      { id: "copy-text", label: "Скопировать текст", danger: false },
      { id: "pin", label: "Открепить сообщение", danger: false },
      { id: "edit", label: "Изменить сообщение", danger: false },
      { id: "delete", label: "Удалить сообщение", danger: true },
      { id: "copy-id", label: "Скопировать ID сообщения", danger: false },
    ]);
  });

  it("формирует пункты меню канала с учётом флага isMuted", () => {
    const unmuted = buildContextMenuItems({
      targetType: "channel",
      isMuted: false,
    });
    assert.deepEqual(unmuted, [
      { id: "mark-read", label: "Отметить как прочитанное", danger: false },
      { id: "mute-channel", label: "Заглушить канал", danger: false },
      { id: "copy-id", label: "Скопировать ID канала", danger: false },
    ]);

    const muted = buildContextMenuItems({
      targetType: "channel",
      isMuted: true,
    });
    assert.deepEqual(muted, [
      { id: "mark-read", label: "Отметить как прочитанное", danger: false },
      { id: "mute-channel", label: "Включить уведомления канала", danger: false },
      { id: "copy-id", label: "Скопировать ID канала", danger: false },
    ]);
  });

  it("формирует пункты меню пользователя и возвращает пустой массив для неизвестного типа", () => {
    const userItems = buildContextMenuItems({
      targetType: "user",
    });
    assert.deepEqual(userItems, [
      { id: "profile", label: "Профиль", danger: false },
      { id: "message-user", label: "Написать сообщение", danger: false },
      { id: "copy-id", label: "Скопировать ID пользователя", danger: false },
    ]);

    assert.deepEqual(
      buildContextMenuItems({ targetType: /** @type {any} */ ("unknown") }),
      [],
    );
  });
});

describe("clampMenuCoordinates", () => {
  it("сохраняет координаты внутри доступной области экрана", () => {
    const coords = clampMenuCoordinates({
      x: 240,
      y: 180,
      menuWidth: 200,
      menuHeight: 160,
      viewportWidth: 1280,
      viewportHeight: 720,
    });
    assert.deepEqual(coords, { x: 240, y: 180 });
  });

  it("ограничивает координаты по нижней/правой границе с отступом 8px и не допускает отрицательных значений", () => {
    const overflow = clampMenuCoordinates({
      x: 1200,
      y: 680,
      menuWidth: 200,
      menuHeight: 160,
      viewportWidth: 1280,
      viewportHeight: 720,
    });
    assert.deepEqual(overflow, {
      x: 1280 - 200 - 8,
      y: 720 - 160 - 8,
    });

    const negative = clampMenuCoordinates({
      x: -50,
      y: -12,
      menuWidth: 200,
      menuHeight: 160,
      viewportWidth: 1280,
      viewportHeight: 720,
    });
    assert.deepEqual(negative, { x: 0, y: 0 });

    const tinyViewport = clampMenuCoordinates({
      x: 100,
      y: 100,
      menuWidth: 300,
      menuHeight: 300,
      viewportWidth: 250,
      viewportHeight: 200,
    });
    assert.deepEqual(tinyViewport, { x: 0, y: 0 });
  });
});

/* ------------------------------------------------------------------ *
 * Тесты createContextMenuController
 * ------------------------------------------------------------------ */

describe("createContextMenuController", () => {
  it("рендерит [data-context-menu] с role='menu', открывает меню с нужными пунктами и помечает опасные пункты data-danger='true'", () => {
    const doc = new FakeDocument();
    const container = doc.createElement("div");
    doc.body.appendChild(container);

    assert.throws(
      () => createContextMenuController({ container: null }),
      TypeError,
    );

    const controller = createContextMenuController({ container });
    const menuEl = container.querySelector('[data-context-menu][role="menu"]');
    assert.ok(menuEl, "элемент [data-context-menu][role='menu'] должен быть создан");
    if (!menuEl) throw new Error("missing menuEl");
    assert.equal(controller.isOpen(), false);
    assert.equal(menuEl.hidden, true);
    assert.equal(menuEl.getAttribute("data-open"), "false");

    const opened = controller.open({
      x: 990,
      y: 590,
      menuWidth: 200,
      menuHeight: 150,
      viewportWidth: 1000,
      viewportHeight: 600,
      targetType: "message",
      targetId: "msg-42",
      isOwnMessage: true,
      isPinned: false,
      canManageMessages: true,
    });
    assert.equal(opened, true);
    assert.equal(controller.isOpen(), true);
    assert.equal(menuEl.hidden, false);
    assert.equal(menuEl.getAttribute("data-open"), "true");
    assert.equal(menuEl.getAttribute("data-target-type"), "message");
    assert.equal(menuEl.getAttribute("data-target-id"), "msg-42");
    assert.equal(menuEl.getAttribute("data-x"), String(1000 - 200 - 8));
    assert.equal(menuEl.getAttribute("data-y"), String(600 - 150 - 8));

    const items = menuEl.querySelectorAll('[data-context-item][role="menuitem"]');
    assert.equal(items.length, 6);
    assert.deepEqual(
      items.map((el) => el.getAttribute("data-context-item")),
      ["reply", "copy-text", "pin", "edit", "delete", "copy-id"],
    );
    assert.deepEqual(
      items.map((el) => el.textContent),
      [
        "Ответить",
        "Скопировать текст",
        "Закрепить сообщение",
        "Изменить сообщение",
        "Удалить сообщение",
        "Скопировать ID сообщения",
      ],
    );

    const dangerItems = menuEl.querySelectorAll('[data-context-item][data-danger="true"]');
    assert.equal(dangerItems.length, 1);
    assert.equal(dangerItems[0]?.getAttribute("data-context-item"), "delete");
    assert.equal(controller.getFocusedIndex(), 0);
    assert.equal(items[0]?.getAttribute("data-focused"), "true");
    assert.equal(items[0]?.getAttribute("tabindex"), "0");
    assert.equal(items[1]?.getAttribute("tabindex"), "-1");
  });

  it("поддерживает циклическую навигацию moveFocus(delta), клавиши ArrowDown/ArrowUp/Home/End и активацию через activateFocusedItem()", () => {
    const doc = new FakeDocument();
    const container = doc.createElement("div");
    doc.body.appendChild(container);
    /** @type {Array<Record<string, any>>} */
    const selectedActions = [];

    const controller = createContextMenuController({
      container,
      onSelectAction: (payload) => selectedActions.push(payload),
    });

    assert.equal(controller.moveFocus(1), -1);
    assert.equal(controller.activateFocusedItem(), null);

    controller.open({
      x: 100,
      y: 120,
      targetType: "channel",
      targetId: "chan-77",
      isMuted: true,
    });

    const menuEl = /** @type {FakeElement} */ (
      container.querySelector("[data-context-menu]")
    );
    const items = menuEl.querySelectorAll("[data-context-item]");
    assert.equal(items.length, 3);
    assert.equal(controller.getFocusedIndex(), 0);

    // Циклическое перемещение назад с 0 на последний элемент (2)
    assert.equal(controller.moveFocus(-1), 2);
    assert.equal(items[2]?.getAttribute("data-focused"), "true");
    assert.equal(doc.activeElement, items[2]);

    // Перемещение вперёд с 2 на 0 и далее на 1
    assert.equal(controller.moveFocus(1), 0);
    menuEl.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(controller.getFocusedIndex(), 1);
    assert.equal(items[1]?.getAttribute("data-context-item"), "mute-channel");
    assert.equal(items[1]?.textContent, "Включить уведомления канала");

    // Клавиши End и Home, затем ArrowDown к пункту mute-channel
    menuEl.dispatchEvent("keydown", { key: "End" });
    assert.equal(controller.getFocusedIndex(), 2);
    menuEl.dispatchEvent("keydown", { key: "Home" });
    assert.equal(controller.getFocusedIndex(), 0);
    menuEl.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(controller.getFocusedIndex(), 1);

    const activated = controller.activateFocusedItem();
    assert.ok(activated);
    if (!activated) throw new Error("expected activated payload");
    assert.equal(activated.actionId, "mute-channel");
    assert.equal(activated.id, "mute-channel");
    assert.equal(activated.label, "Включить уведомления канала");
    assert.equal(activated.targetType, "channel");
    assert.equal(activated.targetId, "chan-77");
    assert.equal(selectedActions.length, 1);
    assert.deepEqual(selectedActions[0], activated);
    assert.equal(controller.isOpen(), false);
    assert.equal(menuEl.hidden, true);
  });

  it("активирует пункт по клику и клавише Enter, закрывает меню по Escape и close()", () => {
    const doc = new FakeDocument();
    const container = doc.createElement("div");
    doc.body.appendChild(container);
    /** @type {Array<Record<string, any>>} */
    const selectedActions = [];

    const controller = createContextMenuController({
      container,
      onSelectAction: (payload) => selectedActions.push(payload),
    });

    controller.open({
      x: 40,
      y: 60,
      targetType: "user",
      targetId: "user-99",
    });

    const menuEl = /** @type {FakeElement} */ (
      container.querySelector("[data-context-menu]")
    );
    const items = menuEl.querySelectorAll("[data-context-item]");
    assert.equal(items.length, 3);

    // Клик по второму пункту ("message-user")
    items[1]?.click();
    assert.equal(selectedActions.length, 1);
    assert.equal(selectedActions[0]?.actionId, "message-user");
    assert.equal(selectedActions[0]?.targetType, "user");
    assert.equal(selectedActions[0]?.targetId, "user-99");
    assert.equal(controller.isOpen(), false);

    // Повторное открытие и выбор через клавишу Enter
    controller.open({
      x: 40,
      y: 60,
      targetType: "user",
      targetId: "user-99",
    });
    menuEl.dispatchEvent("keydown", { key: "ArrowUp" });
    assert.equal(controller.getFocusedIndex(), 2);
    menuEl.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(selectedActions.length, 2);
    assert.equal(selectedActions[1]?.actionId, "copy-id");
    assert.equal(controller.isOpen(), false);

    // Закрытие по Escape и проверка неизвестного targetType
    controller.open({
      x: 10,
      y: 10,
      targetType: "user",
      targetId: "user-99",
    });
    assert.equal(controller.isOpen(), true);
    menuEl.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(controller.isOpen(), false);

    const invalidOpened = controller.open({
      x: 10,
      y: 10,
      targetType: /** @type {any} */ ("invalid"),
      targetId: "x",
    });
    assert.equal(invalidOpened, false);
    assert.equal(controller.isOpen(), false);
  });
});
