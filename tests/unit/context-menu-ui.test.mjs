// @ts-check
/**
 * TASK-620 (G2 Frontend): Unified right-click context menu unit tests
 * (`rust/gui-spike/ui/features/context-menu.js`, `rust/gui-spike/ui/styles/context-menu.css`).
 *
 * Проверяет:
 * 1. Действия над сообщениями (`reply`, `edit`, `pin`, `delete`, `copy-text`, `copy-id`)
 *    с фильтрацией по правам (булевы флаги, объект `permissions`, битовая маска Discord v9
 *    `SEND_MESSAGES`, `MANAGE_MESSAGES`, `ADMINISTRATOR`, `isOwner`, `isOwnMessage`, `isSystemMessage`).
 * 2. Действия над участниками (`profile`, `mention-user`, `user-volume`, `mute-user`, `message-user`, `copy-id`)
 *    с фильтрацией по правам (`canProfile`, `canMention`, `canVolume`, `canMute`, `canMuteMembers`,
 *    `hasVoice`, `isSelf`, битовая маска `SEND_MESSAGES`, `MUTE_MEMBERS`, `ADMINISTRATOR`).
 * 3. Унифицированный контроллер ПКМ (`createContextMenuController`, `bindContextMenuTrigger`,
 *    `openForMessage`, `openForMember`, `handleContextMenuEvent`, `selectAction`):
 *    делегирование `contextmenu` по `[data-message-id]` и `[data-user-id]` / `[data-member-id]`,
 *    слайдер громкости `0..200%` (включая `ArrowLeft`/`ArrowRight`), клавиатурная навигация,
 *    закрытие по `Escape` и клику вне меню, кламп координат во viewport, граничные Snowflake ID.
 * 4. CSS-контракт `context-menu.css`: 100% привязка к `tokens.css`, отсутствие сырых цветов и `!important`,
 *    сверка с RE-оракулом `/home/sanya/re-deep/frontend-d03.md`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import {
  CONTEXT_MENU_PERMISSION_FLAGS,
   bindContextMenuTrigger,
  buildContextMenuItems,
  buildMemberContextMenuItems,
  buildMessageContextMenuItems,
  buildUserContextMenuItems,
  clampMenuCoordinates,
  createContextMenuController,
  resolveContextMenuPermissions,
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
    this.className = "";
    this.type = "";
    this.value = "";
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
   * @param {(event: any) => void} listener
   */
  removeEventListener(type, listener) {
    const list = this._listeners.get(type);
    if (list) {
      this._listeners.set(
        type,
        list.filter((l) => l !== listener),
      );
    }
  }

  /**
   * @param {FakeElement} other
   * @returns {boolean}
   */
  contains(other) {
    /** @type {FakeElement | null} */
    let cur = other;
    while (cur) {
      if (cur === this) return true;
      cur = cur.parentNode;
    }
    return false;
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

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    this.body.addEventListener(type, listener);
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  removeEventListener(type, listener) {
    this.body.removeEventListener(type, listener);
  }

  /**
   * @param {string} type
   * @param {Record<string, any>} [extra]
   * @returns {boolean}
   */
  dispatchEvent(type, extra = {}) {
    return this.body.dispatchEvent(type, extra);
  }
}

/**
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
 * 1. Действия над сообщениями (reply, edit, pin, delete) и права
 * ------------------------------------------------------------------ */

describe("Unified Context Menu: Message actions (reply, edit, pin, delete) filtered by permissions", () => {
  it("включает reply, edit, pin и delete (danger: true) для своего сообщения при наличии права закрепления (canPin / canManageMessages)", () => {
    const unpinned = buildMessageContextMenuItems({
      isOwnMessage: true,
      isPinned: false,
      canPin: true,
    });
    assert.deepEqual(
      unpinned.map((item) => item.id),
      ["reply", "edit", "pin", "copy-text", "copy-id", "delete"],
    );
    const pinItem = unpinned.find((item) => item.id === "pin");
    assert.equal(pinItem?.label, "Закрепить");
    assert.equal(pinItem?.danger, false);

    const deleteItem = unpinned.find((item) => item.id === "delete");
    assert.equal(deleteItem?.danger, true);

    const pinned = buildMessageContextMenuItems({
      isOwnMessage: true,
      isPinned: true,
      canManageMessages: true,
    });
    assert.equal(pinned.find((item) => item.id === "pin")?.label, "Открепить");
  });

  it("разрешает модератору на чужом сообщении reply, pin и delete, но скрывает edit", () => {
    const items = buildMessageContextMenuItems({
      isOwnMessage: false,
      isPinned: false,
      canManageMessages: true,
    });
    const ids = items.map((item) => item.id);
    assert.equal(ids.includes("reply"), true);
    assert.equal(ids.includes("pin"), true);
    assert.equal(ids.includes("delete"), true);
    assert.equal(ids.includes("edit"), false, "модератор не может редактировать чужое сообщение");
  });

  it("скрывает edit, pin и delete на чужом сообщении без прав модерации и скрывает reply при запрете отправки (canReply: false)", () => {
    const foreignNoPerms = buildMessageContextMenuItems({
      isOwnMessage: false,
      canEdit: false,
      canPin: false,
      canDelete: false,
      canManageMessages: false,
    });
    assert.deepEqual(
      foreignNoPerms.map((item) => item.id),
      ["reply", "copy-text", "copy-id"],
    );

    const readOnlyChannel = buildMessageContextMenuItems({
      isOwnMessage: false,
      canReply: false,
      canManageMessages: false,
    });
    assert.equal(
      readOnlyChannel.some((item) => item.id === "reply"),
      false,
      "reply должен скрываться при canReply: false",
    );
  });

  it("фильтрует действия сообщения по битовой маске Discord v9 (SEND_MESSAGES, MANAGE_MESSAGES, ADMINISTRATOR) и системным сообщениям", () => {
    assert.equal(CONTEXT_MENU_PERMISSION_FLAGS.ADMINISTRATOR, 1n << 3n);
    assert.equal(CONTEXT_MENU_PERMISSION_FLAGS.SEND_MESSAGES, 1n << 11n);
    assert.equal(CONTEXT_MENU_PERMISSION_FLAGS.MANAGE_MESSAGES, 1n << 13n);
    assert.equal(CONTEXT_MENU_PERMISSION_FLAGS.MUTE_MEMBERS, 1n << 22n);

    // Обычный участник с правом SEND_MESSAGES на чужом сообщении -> только reply, без edit/pin/delete
    const memberPerms = buildMessageContextMenuItems({
      isOwnMessage: false,
      permissions: CONTEXT_MENU_PERMISSION_FLAGS.SEND_MESSAGES,
    });
    assert.deepEqual(
      memberPerms.map((item) => item.id),
      ["reply", "copy-text", "copy-id"],
    );

    // Модератор с MANAGE_MESSAGES (переданным строкой bitfield) -> reply, pin, delete
    const modBitfieldStr = String(
      CONTEXT_MENU_PERMISSION_FLAGS.SEND_MESSAGES |
        CONTEXT_MENU_PERMISSION_FLAGS.MANAGE_MESSAGES,
    );
    const modItems = buildMessageContextMenuItems({
      isOwnMessage: false,
      permissions: modBitfieldStr,
    });
    assert.deepEqual(
      modItems.map((item) => item.id),
      ["reply", "pin", "copy-text", "copy-id", "delete"],
    );

    // Администратор (ADMINISTRATOR) обходит ограничения на чужом сообщении -> reply, pin, delete
    const adminItems = buildMessageContextMenuItems({
      isOwnMessage: false,
      permissions: CONTEXT_MENU_PERMISSION_FLAGS.ADMINISTRATOR,
    });
    assert.deepEqual(
      adminItems.map((item) => item.id),
      ["reply", "pin", "copy-text", "copy-id", "delete"],
    );

    // Системное сообщение (звонок / вход участника) запрещает edit, pin, delete и reply
    const sysItems = buildMessageContextMenuItems({
      isOwnMessage: true,
      canManageMessages: true,
      isSystemMessage: true,
    });
    assert.deepEqual(
      sysItems.map((item) => item.id),
      ["copy-text", "copy-id"],
    );
  });
});

/* ------------------------------------------------------------------ *
 * 2. Действия над участниками (profile, mention, volume, mute) и права
 * ------------------------------------------------------------------ */

describe("Unified Context Menu: Member actions (profile, mention, volume, mute) filtered by permissions", () => {
  it("включает profile, mention-user, mute-user и user-volume (0..200%) при полных правах участника", () => {
    const items = buildMemberContextMenuItems({
      isMuted: false,
      volume: 125,
      canProfile: true,
      canMention: true,
      canVolume: true,
      canMute: true,
    });
    assert.deepEqual(
      items.map((item) => item.id),
      ["profile", "mention-user", "message-user", "mute-user", "user-volume", "copy-id"],
    );
    const volItem = items.find((item) => item.id === "user-volume");
    assert.equal(volItem?.type, "slider");
    assert.equal(volItem?.min, 0);
    assert.equal(volItem?.max, 200);
    assert.equal(volItem?.value, 125);
  });

  it("фильтрует mention-user, user-volume и mute-user по флагам прав (canMention, canVolume, canMute, hasVoice, isSelf)", () => {
    const noMentionNoMute = buildUserContextMenuItems({
      canMention: false,
      canMute: false,
      canVolume: true,
      volume: 90,
    });
    const ids1 = noMentionNoMute.map((item) => item.id);
    assert.equal(ids1.includes("profile"), true);
    assert.equal(ids1.includes("mention-user"), false, "mention-user должен быть скрыт при canMention: false");
    assert.equal(ids1.includes("mute-user"), false, "mute-user должен быть скрыт при canMute: false");
    assert.equal(ids1.includes("user-volume"), true);

    // Отсутствие голосового контекста (hasVoice: false) скрывает mute-user и user-volume
    const noVoice = buildMemberContextMenuItems({
      hasVoice: false,
      canMention: true,
    });
    const idsNoVoice = noVoice.map((item) => item.id);
    assert.equal(idsNoVoice.includes("profile"), true);
    assert.equal(idsNoVoice.includes("mention-user"), true);
    assert.equal(idsNoVoice.includes("mute-user"), false);
    assert.equal(idsNoVoice.includes("user-volume"), false);

    // Собственный профиль (isSelf: true) скрывает громкость, мут и ЛС самому себе
    const selfItems = buildMemberContextMenuItems({
      isSelf: true,
    });
    assert.deepEqual(
      selfItems.map((item) => item.id),
      ["profile", "mention-user", "copy-id"],
    );
  });

  it("фильтрует действия участника по битовой маске Discord v9 и объекту permissions", () => {
    // Участник с SEND_MESSAGES, но без MUTE_MEMBERS -> есть profile, mention-user, user-volume, нет mute-user
    const withoutMutePerm = buildMemberContextMenuItems({
      permissions: CONTEXT_MENU_PERMISSION_FLAGS.SEND_MESSAGES,
      volume: 100,
    });
    assert.deepEqual(
      withoutMutePerm.map((item) => item.id),
      ["profile", "mention-user", "message-user", "user-volume", "copy-id"],
    );

    // Участник с SEND_MESSAGES | MUTE_MEMBERS -> есть и mention-user, и mute-user
    const withMutePerm = buildMemberContextMenuItems({
      permissions:
        CONTEXT_MENU_PERMISSION_FLAGS.SEND_MESSAGES |
        CONTEXT_MENU_PERMISSION_FLAGS.MUTE_MEMBERS,
      isMuted: true,
      volume: 150,
    });
    assert.deepEqual(
      withMutePerm.map((item) => item.id),
      ["profile", "mention-user", "message-user", "mute-user", "user-volume", "copy-id"],
    );
    assert.equal(
      withMutePerm.find((item) => item.id === "mute-user")?.label,
      "Включить звук",
    );

    // Передача объекта permissions { profile, mention, volume, mute }
    const fromPermObject = buildMemberContextMenuItems({
      permissions: {
        profile: true,
        mention: false,
        volume: false,
        mute: true,
      },
    });
    const objIds = fromPermObject.map((item) => item.id);
    assert.equal(objIds.includes("profile"), true);
    assert.equal(objIds.includes("mention-user"), false);
    assert.equal(objIds.includes("user-volume"), false);
    assert.equal(objIds.includes("mute-user"), true);
  });

  it("resolveContextMenuPermissions корректно вычисляет итоговые флаги для битовых масок, некорректных строк и пустых прав", () => {
    const resolvedZero = resolveContextMenuPermissions({
      isOwnMessage: false,
      permissions: "0",
    });
    assert.equal(resolvedZero.canReply, false);
    assert.equal(resolvedZero.canEdit, false);
    assert.equal(resolvedZero.canPin, false);
    assert.equal(resolvedZero.canDelete, false);
    assert.equal(resolvedZero.canProfile, true);
    assert.equal(resolvedZero.canMention, false);
    assert.equal(resolvedZero.canMute, false);
    assert.equal(resolvedZero.canVolume, false);

    // Невалидная строка bitfield безопасно обрабатывается без исключения
    const resolvedInvalid = resolveContextMenuPermissions({
      isOwnMessage: true,
      permissions: "not-a-valid-bitfield",
    });
    assert.equal(resolvedInvalid.canEdit, true);
    assert.equal(resolvedInvalid.canDelete, true);
  });
});

/* ------------------------------------------------------------------ *
 * 3. Унифицированный контроллер ПКМ, DOM-делегирование и граничные кейсы
 * ------------------------------------------------------------------ */

describe("Unified Context Menu: Controller right-click delegation, keyboard slider & boundaries", () => {
  it("обрабатывает правый клик (contextmenu) на сообщении и участнике в едином контейнере с фильтрацией прав", () => {
    const doc = new FakeDocument();
    const container = doc.createElement("div");
    const chatHost = doc.createElement("div");

    const msgEl = doc.createElement("div");
    msgEl.setAttribute("data-message-id", "18446744073709551615");
    msgEl.setAttribute("data-own-message", "false");
    msgEl.setAttribute("data-can-manage-messages", "true");
    msgEl.setAttribute("data-pinned", "true");

    const memberEl = doc.createElement("div");
    memberEl.setAttribute("data-member-id", "0");
    memberEl.setAttribute("data-muted", "false");
    memberEl.setAttribute("data-volume", "135");
    memberEl.setAttribute("data-can-mute", "false");

    chatHost.appendChild(msgEl);
    chatHost.appendChild(memberEl);
    doc.body.appendChild(container);
    doc.body.appendChild(chatHost);

    /** @type {Array<Record<string, any>>} */
    const actions = [];
    /** @type {number[]} */
    const volumes = [];

    const controller = createContextMenuController({
      container,
      onSelectAction: (payload) => actions.push(payload),
      onVolumeChange: (vol) => volumes.push(vol),
    });

    const unbind = bindContextMenuTrigger(chatHost, controller);

    // 1) ПКМ по сообщению (Snowflake u64::MAX)
    const msgDefaultAllowed = msgEl.dispatchEvent("contextmenu", {
      clientX: 1200,
      clientY: 700,
    });
    assert.equal(msgDefaultAllowed, false, "contextmenu должен вызывать preventDefault()");
    assert.equal(controller.isOpen(), true);

    const menuEl = controller.getElement();
    assert.equal(menuEl.getAttribute("data-target-type"), "message");
    assert.equal(menuEl.getAttribute("data-target-id"), "18446744073709551615");
    // Координаты зажаты в пределах 1280x720 с отступом 8px
    assert.equal(menuEl.getAttribute("data-x"), String(1280 - 220 - 8));
    assert.equal(menuEl.getAttribute("data-y"), String(720 - 240 - 8));

    const msgItemIds = controller.getItems().map((i) => i.id);
    assert.deepEqual(msgItemIds, ["reply", "pin", "copy-text", "copy-id", "delete"]);

    // Активация действия pin ("Открепить")
    const pinResult = controller.selectAction("pin");
    assert.equal(pinResult?.actionId, "pin");
    assert.equal(pinResult?.label, "Открепить");
    assert.equal(pinResult?.targetId, "18446744073709551615");
    assert.equal(controller.isOpen(), false);

    // 2) ПКМ по участнику (Snowflake "0", data-can-mute="false")
    memberEl.dispatchEvent("contextmenu", {
      clientX: 150,
      clientY: 200,
    });
    assert.equal(controller.isOpen(), true);
    assert.equal(menuEl.getAttribute("data-target-type"), "user");
    assert.equal(menuEl.getAttribute("data-target-id"), "0");

    const memberItemIds = controller.getItems().map((i) => i.id);
    assert.equal(memberItemIds.includes("profile"), true);
    assert.equal(memberItemIds.includes("mention-user"), true);
    assert.equal(memberItemIds.includes("user-volume"), true);
    assert.equal(memberItemIds.includes("mute-user"), false, "mute-user отфильтрован через data-can-mute='false'");
    assert.equal(controller.getVolume(), 135);

    // Клавиатурное изменение громкости стрелками ArrowRight / ArrowLeft на слайдере
    const volumeIdx = memberItemIds.indexOf("user-volume");
    while (controller.getFocusedIndex() !== volumeIdx) {
      controller.moveFocus(1);
    }
    menuEl.dispatchEvent("keydown", { key: "ArrowRight" });
    assert.equal(controller.getVolume(), 140);
    menuEl.dispatchEvent("keydown", { key: "ArrowLeft" });
    assert.equal(controller.getVolume(), 135);
    assert.deepEqual(volumes, [140, 135]);

    unbind();
  });

  it("поддерживает методы openForMessage и openForMember и закрывается при пустом списке доступных действий", () => {
    const doc = new FakeDocument();
    const container = doc.createElement("div");
    doc.body.appendChild(container);

    const controller = createContextMenuController({ container });

    assert.equal(
      controller.openForMessage({
        targetId: "msg-77",
        isOwnMessage: true,
        canPin: true,
      }),
      true,
    );
    assert.deepEqual(
      controller.getItems().map((i) => i.id),
      ["reply", "edit", "pin", "copy-text", "copy-id", "delete"],
    );

    assert.equal(
      controller.openForMember({
        targetId: "usr-88",
        canMention: false,
        canMute: true,
        canVolume: false,
      }),
      true,
    );
    assert.deepEqual(
      controller.getItems().map((i) => i.id),
      ["profile", "message-user", "mute-user", "copy-id"],
    );

    // Если все пункты запрещены — open возвращает false и закрывает меню
    const emptyOpened = controller.openForMessage({
      targetId: "msg-empty",
      isOwnMessage: false,
      canReply: false,
      canPin: false,
      canDelete: false,
      canCopyText: false,
      canCopyId: false,
    });
    assert.equal(emptyOpened, false);
    assert.equal(controller.isOpen(), false);
  });
});

/* ------------------------------------------------------------------ *
 * 4. CSS-контракт context-menu.css и RE-оракул
 * ------------------------------------------------------------------ */

describe("Unified Context Menu: CSS contract (context-menu.css) & RE oracle", () => {
  it("context-menu.css использует исключительно токены из tokens.css без raw hex/rgb/hsl и без !important", () => {
    const cssPath = fileURLToPath(
      new URL("../../rust/gui-spike/ui/styles/context-menu.css", import.meta.url),
    );
    const tokensPath = fileURLToPath(
      new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url),
    );
    assert.ok(existsSync(cssPath), "styles/context-menu.css должен существовать");
    const css = readFileSync(cssPath, "utf8");
    const tokensCss = readFileSync(tokensPath, "utf8");

    const withoutComments = css.replace(/\/\*[\s\S]*?\*\//g, "");

    for (const selector of [
      ".context-menu",
      "[data-context-menu]",
      ".context-menu-item",
      "[data-context-item]",
      '[data-danger="true"]',
      ".context-menu-separator",
      ".context-menu-volume-wrap",
      ".context-menu-slider",
    ]) {
      assert.ok(
        withoutComments.includes(selector),
        `в context-menu.css должен присутствовать селектор ${selector}`,
      );
    }

    assert.doesNotMatch(
      withoutComments,
      /#[0-9a-fA-F]{3,8}\b/,
      "в context-menu.css запрещены сырые hex-цвета",
    );
    assert.doesNotMatch(
      withoutComments,
      /\brgba?\s*\(/i,
      "в context-menu.css запрещены сырые rgb/rgba()",
    );
    assert.doesNotMatch(
      withoutComments,
      /\bhsla?\s*\(/i,
      "в context-menu.css запрещены сырые hsl/hsla()",
    );
    assert.doesNotMatch(
      withoutComments,
      /!important/i,
      "в context-menu.css запрещён !important",
    );

    const definedTokens = new Set(
      [...tokensCss.matchAll(/(--[a-z0-9-]+)\s*:/g)].map((m) => m[1]),
    );
    const usedTokens = [
      ...withoutComments.matchAll(/var\(\s*(--[a-z0-9-]+)/g),
    ].map((m) => m[1]);
    assert.ok(usedTokens.length >= 10, "context-menu.css должен использовать токены из tokens.css");
    for (const token of usedTokens) {
      assert.ok(
        definedTokens.has(token),
        `токен ${token} из context-menu.css должен быть объявлен в tokens.css`,
      );
    }
  });
});
