// @ts-check
/**
 * T-210 (RW-40 / G2): unit-тесты контроллера панели действий сообщения
 * и модального окна пересылки (`rust/gui-spike/ui/features/message-actions.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  QUICK_REACTION_EMOJIS,
  MAX_FORWARD_DESTINATIONS,
  createMessageActionsController,
} from "../../rust/gui-spike/ui/features/message-actions.js";

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
    this._placeholder = "";
    this._checked = false;
    this._disabled = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {string} */
  get id() {
    return this.getAttribute("id") || "";
  }

  /** @param {string} val */
  set id(val) {
    this.setAttribute("id", val);
  }

  /** @returns {boolean} */
  get hidden() {
    return this.hasAttribute("hidden");
  }

  /** @param {boolean} val */
  set hidden(val) {
    if (val) this.setAttribute("hidden", "");
    else this.removeAttribute("hidden");
  }

  /** @returns {boolean} */
  get disabled() {
    return this._disabled || this._attrs.has("disabled");
  }

  /** @param {boolean} val */
  set disabled(val) {
    this._disabled = Boolean(val);
    if (this._disabled) this._attrs.set("disabled", "");
    else this._attrs.delete("disabled");
  }

  /** @returns {boolean} */
  get checked() {
    return this._checked;
  }

  /** @param {boolean} val */
  set checked(val) {
    this._checked = Boolean(val);
    if (this._checked) this._attrs.set("checked", "");
    else this._attrs.delete("checked");
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

  /** @returns {string} */
  get placeholder() {
    return this._placeholder || this._attrs.get("placeholder") || "";
  }

  /** @param {string} val */
  set placeholder(val) {
    this._placeholder = String(val ?? "");
    this._attrs.set("placeholder", this._placeholder);
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
    for (const child of this.children) child.parentNode = null;
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
    if (name === "placeholder") this._placeholder = str;
    if (name === "value") this._value = str;
    if (name === "disabled") this._disabled = true;
    if (name === "checked") this._checked = true;
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
    if (name === "checked") this._checked = false;
  }

  /**
   * @param {FakeElement} child
   * @returns {FakeElement}
   */
  appendChild(child) {
    child.remove();
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
    for (const child of this.children) child.parentNode = null;
    this.children = [];
    this._text = "";
    for (const node of nodes) this.appendChild(node);
  }

  remove() {
    if (this.parentNode) this.parentNode.removeChild(this);
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    if (!this._listeners.has(type)) this._listeners.set(type, []);
    this._listeners.get(type)?.push(listener);
  }

  /**
   * @param {string | { type: string, [key: string]: any }} typeOrEvent
   * @param {Record<string, any>} [init]
   */
  dispatchEvent(typeOrEvent, init = {}) {
    /** @type {{ type: string, [key: string]: any }} */
    const base =
      typeof typeOrEvent === "string"
        ? { type: typeOrEvent, ...init }
        : { ...typeOrEvent };
    const event = {
      ...base,
      type: String(base.type),
      target: base.target ?? this,
      currentTarget: this,
      defaultPrevented: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {},
    };
    for (const listener of this._listeners.get(event.type) || []) {
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
    const walk = (/** @type {FakeElement} */ el) => {
      for (const child of el.children) {
        if (matches(child, selector)) out.push(child);
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
function matches(el, selector) {
  const tokens = selector.match(/\[[^\]]+\]|[#.][\w-]+|[a-zA-Z][\w-]*/g) || [];
  if (tokens.length === 0) return false;
  return tokens.every((token) => {
    if (token.startsWith("#")) return el.id === token.slice(1);
    if (token.startsWith(".")) {
      const classes = (el.getAttribute("class") || "").split(/\s+/).filter(Boolean);
      return classes.includes(token.slice(1));
    }
    if (token.startsWith("[")) {
      const m = token
        .slice(1, -1)
        .match(/^([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?$/);
      const attrName = m?.[1];
      if (!m || !attrName) return false;
      const value = m[2] ?? m[3] ?? m[4];
      if (value === undefined) return el.hasAttribute(attrName);
      return el.getAttribute(attrName) === value;
    }
    return el.tagName === token.toUpperCase();
  });
}

function createTestContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

const SAMPLE_MESSAGE = Object.freeze({
  id: "msg-101",
  authorId: "user-author",
  content: "Привет из тестового сообщения!",
  pinned: false,
});

const SAMPLE_CHANNELS = Object.freeze([
  { id: "ch-1", name: "общий" },
  { id: "ch-2", name: "новости" },
  { id: "ch-3", name: "разработка-rust" },
  { id: "ch-4", name: "дизайн-ui" },
  { id: "ch-5", name: "тестирование-qa" },
  { id: "ch-6", name: "оффтоп" },
  { id: "ch-7", name: "голосовой-чат" },
]);

/* ------------------------------------------------------------------ *
 * Тесты
 * ------------------------------------------------------------------ */

describe("Константы QUICK_REACTION_EMOJIS и MAX_FORWARD_DESTINATIONS", () => {
  it("QUICK_REACTION_EMOJIS заморожен и содержит ['👍', '❤️', '😂'], а MAX_FORWARD_DESTINATIONS равен 5", () => {
    assert.equal(Object.isFrozen(QUICK_REACTION_EMOJIS), true);
    assert.deepEqual(QUICK_REACTION_EMOJIS, ["👍", "❤️", "😂"]);
    assert.equal(MAX_FORWARD_DESTINATIONS, 5);
  });
});

describe("createMessageActionsController: renderActionBar", () => {
  it("всегда рендерит 3 быстрые реакции, 'Ответить' и 'Переслать' и вызывает onReact / onReply", () => {
    const container = createTestContainer();
    /** @type {Array<{ messageId: string, emoji: string }>} */
    const reactions = [];
    /** @type {string[]} */
    const replies = [];

    const ctrl = createMessageActionsController(container, {
      onReact(messageId, emoji) {
        reactions.push({ messageId, emoji });
      },
      onReply(messageId) {
        replies.push(messageId);
      },
    });

    ctrl.renderActionBar(SAMPLE_MESSAGE, {
      currentUserId: "user-other",
      canManageMessages: false,
    });

    const bar = container.querySelector('[role="toolbar"]');
    assert.ok(bar, "панель действий сообщения должна иметь role='toolbar'");
    assert.equal(bar.getAttribute("data-message-id"), "msg-101");

    const quickReactBtns = bar.querySelectorAll('[data-action="quick-react"]');
    assert.equal(quickReactBtns.length, 3);
    assert.deepEqual(
      quickReactBtns.map((btn) => btn.textContent),
      ["👍", "❤️", "😂"],
    );

    quickReactBtns[0]?.click();
    quickReactBtns[2]?.click();
    assert.deepEqual(reactions, [
      { messageId: "msg-101", emoji: "👍" },
      { messageId: "msg-101", emoji: "😂" },
    ]);

    const replyBtn = bar.querySelector('[data-action="reply"]');
    assert.ok(replyBtn, "кнопка 'Ответить' должна присутствовать");
    assert.equal(replyBtn.textContent, "Ответить");
    replyBtn.click();
    assert.deepEqual(replies, ["msg-101"]);

    const forwardBtn = bar.querySelector('[data-action="forward"]');
    assert.ok(forwardBtn, "кнопка 'Переслать' должна присутствовать");
    assert.equal(forwardBtn.textContent, "Переслать");

    // Для чужого сообщения без прав управления нет кнопок 'Изменить', 'Закрепить сообщение', 'Удалить сообщение'
    assert.equal(bar.querySelector('[data-action="edit"]'), null);
    assert.equal(bar.querySelector('[data-action="toggle-pin"]'), null);
    assert.equal(bar.querySelector('[data-action="delete"]'), null);
  });

  it("рендерит 'Изменить' ТОЛЬКО когда message.authorId === context.currentUserId и вызывает onEdit(message.id)", () => {
    const container = createTestContainer();
    /** @type {string[]} */
    const edited = [];

    const ctrl = createMessageActionsController(container, {
      onEdit(messageId) {
        edited.push(messageId);
      },
    });

    // Автор сообщения -> видит 'Изменить'
    ctrl.renderActionBar(SAMPLE_MESSAGE, {
      currentUserId: "user-author",
      canManageMessages: false,
    });
    const editBtn = container.querySelector('[data-action="edit"]');
    assert.ok(editBtn, "автор должен видеть кнопку 'Изменить'");
    assert.equal(editBtn.textContent, "Изменить");
    editBtn.click();
    assert.deepEqual(edited, ["msg-101"]);

    // Модератор, но НЕ автор -> НЕ видит 'Изменить'
    ctrl.renderActionBar(SAMPLE_MESSAGE, {
      currentUserId: "user-moderator",
      canManageMessages: true,
    });
    assert.equal(
      container.querySelector('[data-action="edit"]'),
      null,
      "модератор не должен видеть кнопку 'Изменить' на чужом сообщении",
    );

    // Пустой контекст (undefined === undefined не должно срабатывать)
    ctrl.renderActionBar({ id: "msg-anon" }, {});
    assert.equal(container.querySelector('[data-action="edit"]'), null);
  });

  it("рендерит 'Закрепить сообщение' / 'Открепить сообщение' ТОЛЬКО при context.canManageMessages === true и вызывает onTogglePin", () => {
    const container = createTestContainer();
    /** @type {Array<{ messageId: string, nextPinned: boolean }>} */
    const pinCalls = [];

    const ctrl = createMessageActionsController(container, {
      onTogglePin(messageId, nextPinned) {
        pinCalls.push({ messageId, nextPinned });
      },
    });

    // Автор без прав canManageMessages -> НЕ видит кнопку закрепления
    ctrl.renderActionBar(SAMPLE_MESSAGE, {
      currentUserId: "user-author",
      canManageMessages: false,
    });
    assert.equal(container.querySelector('[data-action="toggle-pin"]'), null);

    // С правом canManageMessages === true и pinned === false -> 'Закрепить сообщение'
    ctrl.renderActionBar(
      { ...SAMPLE_MESSAGE, pinned: false },
      { currentUserId: "user-mod", canManageMessages: true },
    );
    const pinBtn = container.querySelector('[data-action="toggle-pin"]');
    assert.ok(pinBtn, "при canManageMessages=true должна быть кнопка закрепления");
    assert.equal(pinBtn.textContent, "Закрепить сообщение");
    pinBtn.click();

    // С правом canManageMessages === true и pinned === true -> 'Открепить сообщение'
    ctrl.renderActionBar(
      { ...SAMPLE_MESSAGE, pinned: true },
      { currentUserId: "user-mod", canManageMessages: true },
    );
    const unpinBtn = container.querySelector('[data-action="toggle-pin"]');
    assert.ok(unpinBtn);
    assert.equal(unpinBtn.textContent, "Открепить сообщение");
    unpinBtn.click();

    assert.deepEqual(pinCalls, [
      { messageId: "msg-101", nextPinned: true },
      { messageId: "msg-101", nextPinned: false },
    ]);
  });

  it("рендерит 'Удалить сообщение' для автора ИЛИ при canManageMessages === true и вызывает onDelete(message.id)", () => {
    const container = createTestContainer();
    /** @type {string[]} */
    const deleted = [];

    const ctrl = createMessageActionsController(container, {
      onDelete(messageId) {
        deleted.push(messageId);
      },
    });

    // 1. Автор без canManageMessages -> видит 'Удалить сообщение'
    ctrl.renderActionBar(SAMPLE_MESSAGE, {
      currentUserId: "user-author",
      canManageMessages: false,
    });
    const deleteByAuthor = container.querySelector('[data-action="delete"]');
    assert.ok(deleteByAuthor);
    assert.equal(deleteByAuthor.textContent, "Удалить сообщение");
    deleteByAuthor.click();

    // 2. Не автор, но canManageMessages === true -> видит 'Удалить сообщение'
    ctrl.renderActionBar(SAMPLE_MESSAGE, {
      currentUserId: "user-mod",
      canManageMessages: true,
    });
    const deleteByMod = container.querySelector('[data-action="delete"]');
    assert.ok(deleteByMod);
    assert.equal(deleteByMod.textContent, "Удалить сообщение");
    deleteByMod.click();

    // 3. Не автор и canManageMessages === false -> НЕ видит 'Удалить сообщение'
    ctrl.renderActionBar(SAMPLE_MESSAGE, {
      currentUserId: "user-stranger",
      canManageMessages: false,
    });
    assert.equal(container.querySelector('[data-action="delete"]'), null);

    assert.deepEqual(deleted, ["msg-101", "msg-101"]);
  });
});

describe("createMessageActionsController: openForwardModal", () => {
  it("рендерит диалог 'Переслать сообщение' с фильтром каналов, чекбоксами, полем комментария и кнопкой 'Отправить'", () => {
    const container = createTestContainer();
    const ctrl = createMessageActionsController(container);

    ctrl.openForwardModal(SAMPLE_MESSAGE, SAMPLE_CHANNELS);

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog, "должен быть отрендерен диалог пересылки");
    assert.equal(dialog.getAttribute("aria-modal"), "true");
    assert.ok(dialog.textContent.includes("Переслать сообщение"));

    const filterInput = dialog.querySelector("[data-forward-filter]");
    assert.ok(filterInput, "должно присутствовать поле фильтра каналов");

    const checkboxes = dialog.querySelectorAll('input[type="checkbox"]');
    assert.equal(checkboxes.length, SAMPLE_CHANNELS.length);

    const commentInput = dialog.querySelector("[data-forward-comment]");
    assert.ok(commentInput, "должно присутствовать поле необязательного комментария");

    const submitBtn = dialog.querySelector('[data-action="forward-submit"]');
    assert.ok(submitBtn, "должна присутствовать кнопка 'Отправить'");
    assert.equal(submitBtn.textContent, "Отправить");
  });

  it("ограничивает выбор получателей лимитом MAX_FORWARD_DESTINATIONS (5) и блокирует остальные чекбоксы", () => {
    const container = createTestContainer();
    /** @type {Array<{ messageId: string, selectedChannelIds: string[], comment: string }>} */
    const forwards = [];

    const ctrl = createMessageActionsController(container, {
      onForward(messageId, selectedChannelIds, comment) {
        forwards.push({ messageId, selectedChannelIds, comment });
      },
    });

    ctrl.openForwardModal(SAMPLE_MESSAGE, SAMPLE_CHANNELS);

    const checkboxes = container.querySelectorAll('input[type="checkbox"]');
    assert.equal(checkboxes.length, 7);

    // Выбираем первые 5 каналов
    for (let i = 0; i < MAX_FORWARD_DESTINATIONS; i += 1) {
      const box = checkboxes[i];
      assert.ok(box);
      box.click();
      assert.equal(box.checked, true);
    }

    const box2 = checkboxes[1];
    const box6 = checkboxes[5];
    const box7 = checkboxes[6];
    assert.ok(box2 && box6 && box7);

    // 6-й и 7-й чекбоксы должны стать заблокированными (disabled)
    assert.equal(box6.disabled, true);
    assert.equal(box7.disabled, true);

    // Попытка кликнуть или вызвать change на 6-м чекбоксе не должна добавить его в выбранные
    box6.click();
    box6.dispatchEvent("change", { target: { checked: true } });
    assert.equal(box6.checked, false);

    // Снимаем выбор со 2-го канала -> 6-й чекбокс разблокируется, можно выбрать его
    box2.click();
    assert.equal(box2.checked, false);
    assert.equal(box6.disabled, false);

    box6.click();
    assert.equal(box6.checked, true);
    assert.equal(box7.disabled, true);

    const submitBtn = container.querySelector('[data-action="forward-submit"]');
    assert.ok(submitBtn);
    submitBtn.click();

    assert.equal(forwards.length, 1);
    const firstForward = forwards[0];
    assert.ok(firstForward);
    assert.equal(firstForward.messageId, "msg-101");
    assert.deepEqual(firstForward.selectedChannelIds, [
      "ch-1",
      "ch-3",
      "ch-4",
      "ch-5",
      "ch-6",
    ]);
    assert.equal(firstForward.comment, "");
  });

  it("фильтрует список каналов по вводу и передаёт комментарий в onForward(message.id, selectedChannelIds, comment)", () => {
    const container = createTestContainer();
    /** @type {Array<{ messageId: string, selectedChannelIds: string[], comment: string }>} */
    const forwards = [];

    const ctrl = createMessageActionsController(container, {
      onForward(messageId, selectedChannelIds, comment) {
        forwards.push({ messageId, selectedChannelIds, comment });
      },
    });

    ctrl.openForwardModal(SAMPLE_MESSAGE, SAMPLE_CHANNELS);

    // Выбираем первый канал до фильтрации
    const initialBoxes = container.querySelectorAll('input[type="checkbox"]');
    initialBoxes[0]?.click(); // ch-1 ("общий")

    // Фильтруем по подстроке "разработка"
    const filterInput = container.querySelector("[data-forward-filter]");
    assert.ok(filterInput);
    filterInput.value = "РАЗРАБОТКА";
    filterInput.dispatchEvent("input");

    const filteredBoxes = container.querySelectorAll('input[type="checkbox"]');
    assert.equal(filteredBoxes.length, 1);
    const matchedBox = filteredBoxes[0];
    assert.ok(matchedBox);
    assert.equal(matchedBox.getAttribute("data-channel-id"), "ch-3");

    // Выбираем найденный канал ch-3
    matchedBox.click();
    assert.equal(matchedBox.checked, true);

    // Вводим комментарий
    const commentInput = container.querySelector("[data-forward-comment]");
    assert.ok(commentInput);
    commentInput.value = "Срочно посмотрите этот баг-репорт";
    commentInput.dispatchEvent("input");

    const submitBtn = container.querySelector('[data-action="forward-submit"]');
    assert.ok(submitBtn);
    submitBtn.click();

    assert.deepEqual(forwards, [
      {
        messageId: "msg-101",
        selectedChannelIds: ["ch-1", "ch-3"],
        comment: "Срочно посмотрите этот баг-репорт",
      },
    ]);
  });

  it("не вызывает onForward при пустом выборе каналов и валидирует containerEl", () => {
    assert.throws(() => createMessageActionsController(null), TypeError);

    const container = createTestContainer();
    let called = false;
    const ctrl = createMessageActionsController(container, {
      onForward() {
        called = true;
      },
    });

    ctrl.openForwardModal(SAMPLE_MESSAGE, SAMPLE_CHANNELS);
    const submitBtn = container.querySelector('[data-action="forward-submit"]');
    assert.ok(submitBtn);
    submitBtn.click();
    assert.equal(called, false);
  });
});
