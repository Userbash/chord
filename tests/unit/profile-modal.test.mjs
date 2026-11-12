// @ts-check
/**
 * T-203 (RW-40 / G2): unit-тесты контроллера поповера и модалки профиля
 * (`rust/gui-spike/ui/features/profile-modal.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  STATUS_LABELS_RU,
  createProfileModalController,
} from "../../rust/gui-spike/ui/features/profile-modal.js";

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных unit-тестов
 * ------------------------------------------------------------------ */

class FakeStyle {
  constructor() {
    this.color = "";
    /** @type {Map<string, string>} */
    this._props = new Map();
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setProperty(name, value) {
    const str = String(value);
    this._props.set(name, str);
    if (name === "color") this.color = str;
  }

  /**
   * @param {string} name
   * @returns {string}
   */
  getPropertyValue(name) {
    if (name === "color" && this.color) return this.color;
    return this._props.get(name) ?? "";
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
    this._value = "";
    this._placeholder = "";
    this.style = new FakeStyle();
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
  }

  /**
   * @param {FakeElement} child
   * @returns {FakeElement}
   */
  appendChild(child) {
    child.remove();
    child.parentNode = this;
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
    };
    for (const listener of this._listeners.get(event.type) || []) {
      listener(event);
    }
    return !event.defaultPrevented;
  }

  click() {
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
      if (!m || !m[1]) return false;
      const attrName = m[1];
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

const SAMPLE_PROFILE = Object.freeze({
  userId: "u-42",
  username: "demouser",
  displayName: "DemoUser",
  pronouns: "он/его",
  status: "dnd",
  bio: "Разработчик Rust и исследователь протоколов.",
  note: "Коллега по G2",
  roles: [
    { id: "role-core", name: "Core Dev", color: "#e91e63" },
    { id: "role-mod", name: "Модератор", color: "#5865f2" },
  ],
  mutualGuilds: [
    { id: "g-1", name: "Discord Lab" },
    { id: "g-2", name: "Rust Community" },
  ],
  mutualFriends: [
    { id: "f-1", username: "alice", displayName: "Алиса" },
  ],
});

/* ------------------------------------------------------------------ *
 * Тесты
 * ------------------------------------------------------------------ */

describe("STATUS_LABELS_RU", () => {
  it("заморожен и содержит русские подписи для online, idle, dnd, offline", () => {
    assert.equal(Object.isFrozen(STATUS_LABELS_RU), true);
    assert.deepEqual(STATUS_LABELS_RU, {
      online: "В сети",
      idle: "Не активен",
      dnd: "Не беспокоить",
      offline: "Не в сети",
    });
  });
});

describe("createProfileModalController: openPopout", () => {
  it("рендерит компактную карточку data-profile-mode='popout' с именем, юзернеймом, местоимениями, статусом и био", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    ctrl.openPopout(SAMPLE_PROFILE);

    const card = container.querySelector('[data-profile-mode="popout"]');
    assert.ok(card, "карточка поповера должна быть отрендерена");
    assert.equal(card.getAttribute("data-user-id"), "u-42");

    const displayNameEl = card.querySelector("[data-profile-display-name]");
    assert.ok(displayNameEl);
    assert.equal(displayNameEl.textContent, "DemoUser");

    const usernameEl = card.querySelector("[data-profile-username]");
    assert.ok(usernameEl);
    assert.equal(usernameEl.textContent, "demouser");

    const pronounsEl = card.querySelector("[data-profile-pronouns]");
    assert.ok(pronounsEl);
    assert.equal(pronounsEl.textContent, "он/его");

    const statusEl = card.querySelector("[data-profile-status]");
    assert.ok(statusEl);
    assert.equal(statusEl.getAttribute("data-profile-status"), "dnd");
    assert.equal(statusEl.textContent, "Не беспокоить");

    const bioEl = card.querySelector("[data-profile-bio]");
    assert.ok(bioEl);
    assert.equal(bioEl.textContent, "Разработчик Rust и исследователь протоколов.");
  });

  it("корректно отображает все 4 статуса из STATUS_LABELS_RU и фоллбэчит неизвестный в offline", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    for (const [statusKey, labelRu] of Object.entries(STATUS_LABELS_RU)) {
      ctrl.openPopout({ ...SAMPLE_PROFILE, status: statusKey });
      const statusEl = container.querySelector("[data-profile-status]");
      assert.ok(statusEl);
      assert.equal(statusEl.getAttribute("data-profile-status"), statusKey);
      assert.equal(statusEl.textContent, labelRu);
    }

    ctrl.openPopout({ ...SAMPLE_PROFILE, status: "unknown_status" });
    const fallbackEl = container.querySelector("[data-profile-status]");
    assert.ok(fallbackEl);
    assert.equal(fallbackEl.getAttribute("data-profile-status"), "offline");
    assert.equal(fallbackEl.textContent, "Не в сети");
  });

  it("рендерит пилюли ролей с data-role-id и цветом роли", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    ctrl.openPopout(SAMPLE_PROFILE);

    const pills = container.querySelectorAll("[data-role-id]");
    assert.equal(pills.length, 2);

    assert.equal(pills[0]?.getAttribute("data-role-id"), "role-core");
    assert.equal(pills[0]?.getAttribute("data-role-color"), "#e91e63");
    assert.equal(pills[0]?.style.color, "#e91e63");
    assert.equal(pills[0]?.textContent, "Core Dev");

    assert.equal(pills[1]?.getAttribute("data-role-id"), "role-mod");
    assert.equal(pills[1]?.getAttribute("data-role-color"), "#5865f2");
    assert.equal(pills[1]?.style.color, "#5865f2");
    assert.equal(pills[1]?.textContent, "Модератор");
  });

  it("вызывает onSaveNote(userId, noteText) при изменении поля личной заметки", () => {
    const container = createTestContainer();
    /** @type {Array<{ userId: string, note: string }>} */
    const savedNotes = [];
    const ctrl = createProfileModalController(container, {
      onSaveNote(userId, note) {
        savedNotes.push({ userId, note });
      },
    });

    ctrl.openPopout(SAMPLE_PROFILE);

    const noteInput = container.querySelector("[data-profile-note-input]");
    assert.ok(noteInput, "поле заметки должно присутствовать в поповере");
    assert.equal(noteInput.value, "Коллега по G2");

    noteInput.value = "Обновлённая заметка";
    noteInput.dispatchEvent("change");

    assert.deepEqual(savedNotes, [{ userId: "u-42", note: "Обновлённая заметка" }]);
  });

  it("рендерит быстрый инпут ЛС с placeholder 'Написать @' + username и вызывает onSendQuickMessage только для непустого текста", () => {
    const container = createTestContainer();
    /** @type {Array<{ userId: string, text: string }>} */
    const sentMessages = [];
    const ctrl = createProfileModalController(container, {
      onSendQuickMessage(userId, text) {
        sentMessages.push({ userId, text });
      },
    });

    ctrl.openPopout(SAMPLE_PROFILE);

    const dmInput = container.querySelector("[data-profile-quick-dm-input]");
    assert.ok(dmInput, "инпут быстрого сообщения должен быть в поповере");
    assert.equal(dmInput.placeholder, "Написать @demouser");
    assert.equal(dmInput.getAttribute("placeholder"), "Написать @demouser");

    // Пустой ввод или пробелы не должны вызывать onSendQuickMessage
    dmInput.value = "   ";
    dmInput.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sentMessages.length, 0);

    // Непустое сообщение отправляется и очищает поле ввода
    dmInput.value = "Привет, проверишь PR?";
    dmInput.dispatchEvent("keydown", { key: "Enter" });
    assert.deepEqual(sentMessages, [
      { userId: "u-42", text: "Привет, проверишь PR?" },
    ]);
    assert.equal(dmInput.value, "");
  });
});

describe("createProfileModalController: openFullModal и setTab", () => {
  it("рендерит диалог полного профиля (role='dialog', aria-modal='true', data-profile-mode='modal') с 3 вкладками и счётчиками", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    ctrl.openFullModal(SAMPLE_PROFILE);

    const modal = container.querySelector('[data-profile-mode="modal"]');
    assert.ok(modal, "модалка полного профиля должна быть отрендерена");
    assert.equal(modal.getAttribute("role"), "dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true");

    const tabs = modal.querySelectorAll('[role="tab"]');
    assert.equal(tabs.length, 3);

    const [aboutTab, guildsTab, friendsTab] = tabs;
    assert.ok(aboutTab && guildsTab && friendsTab);
    assert.equal(aboutTab.getAttribute("data-profile-tab"), "about");
    assert.equal(aboutTab.getAttribute("aria-selected"), "true");
    assert.ok(aboutTab.textContent?.includes("Обо мне"));

    assert.equal(guildsTab.getAttribute("data-profile-tab"), "mutual_guilds");
    assert.equal(guildsTab.getAttribute("aria-selected"), "false");
    assert.equal(guildsTab.getAttribute("data-tab-count"), "2");
    assert.ok(guildsTab.textContent?.includes("Общие серверы"));
    assert.ok(guildsTab.textContent?.includes("2"));

    assert.equal(friendsTab.getAttribute("data-profile-tab"), "mutual_friends");
    assert.equal(friendsTab.getAttribute("aria-selected"), "false");
    assert.equal(friendsTab.getAttribute("data-tab-count"), "1");
    assert.ok(friendsTab.textContent?.includes("Общие друзья"));
    assert.ok(friendsTab.textContent?.includes("1"));

    // По умолчанию открыта вкладка 'about'
    const bioEl = modal.querySelector("[data-profile-bio]");
    assert.ok(bioEl);
    assert.equal(bioEl.textContent, "Разработчик Rust и исследователь протоколов.");
  });

  it("setTab('mutual_guilds') и setTab('mutual_friends') переключают активную вкладку и рендерят списки", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    ctrl.openFullModal(SAMPLE_PROFILE);

    assert.equal(ctrl.setTab("mutual_guilds"), true);
    const guildsTab = container.querySelector('[data-profile-tab="mutual_guilds"]');
    const aboutTab = container.querySelector('[data-profile-tab="about"]');
    assert.equal(guildsTab?.getAttribute("aria-selected"), "true");
    assert.equal(aboutTab?.getAttribute("aria-selected"), "false");

    const guildItems = container.querySelectorAll("[data-mutual-guild-id]");
    assert.equal(guildItems.length, 2);
    assert.equal(guildItems[0]?.getAttribute("data-mutual-guild-id"), "g-1");
    assert.equal(guildItems[0]?.textContent, "Discord Lab");
    assert.equal(guildItems[1]?.getAttribute("data-mutual-guild-id"), "g-2");
    assert.equal(guildItems[1]?.textContent, "Rust Community");

    assert.equal(ctrl.setTab("mutual_friends"), true);
    const friendsTab = container.querySelector('[data-profile-tab="mutual_friends"]');
    assert.equal(friendsTab?.getAttribute("aria-selected"), "true");
    assert.equal(guildsTab?.getAttribute("aria-selected"), "false");

    const friendItems = container.querySelectorAll("[data-mutual-friend-id]");
    assert.equal(friendItems.length, 1);
    assert.equal(friendItems[0]?.getAttribute("data-mutual-friend-id"), "f-1");
    assert.ok(friendItems[0]?.textContent?.includes("Алиса"));
  });

  it("рендерит русские пустые состояния 'Нет общих серверов' и 'Нет общих друзей' при пустых списках", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    ctrl.openFullModal({
      ...SAMPLE_PROFILE,
      mutualGuilds: [],
      mutualFriends: [],
    });

    ctrl.setTab("mutual_guilds");
    const emptyGuilds = container.querySelector('[data-profile-empty="mutual_guilds"]');
    assert.ok(emptyGuilds, "должен отображаться блок пустого состояния общих серверов");
    assert.equal(emptyGuilds.textContent, "Нет общих серверов");

    ctrl.setTab("mutual_friends");
    const emptyFriends = container.querySelector('[data-profile-empty="mutual_friends"]');
    assert.ok(emptyFriends, "должен отображаться блок пустого состояния общих друзей");
    assert.equal(emptyFriends.textContent, "Нет общих друзей");
  });

  it("клик по кнопке вкладки переключает вкладку, а невалидный setTab возвращает false", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    ctrl.openFullModal(SAMPLE_PROFILE);

    const friendsTabBtn = container.querySelector('[data-profile-tab="mutual_friends"]');
    assert.ok(friendsTabBtn);
    friendsTabBtn.click();
    assert.equal(friendsTabBtn.getAttribute("aria-selected"), "true");
    assert.equal(container.querySelectorAll("[data-mutual-friend-id]").length, 1);

    assert.equal(ctrl.setTab("non_existent_tab"), false);
    assert.equal(friendsTabBtn.getAttribute("aria-selected"), "true");
  });
});

describe("createProfileModalController: переключение режимов и close()", () => {
  it("openFullModal заменяет открытый popout, а close() полностью удаляет карточку/модалку из контейнера", () => {
    const container = createTestContainer();
    const ctrl = createProfileModalController(container);

    ctrl.openPopout(SAMPLE_PROFILE);
    assert.ok(container.querySelector('[data-profile-mode="popout"]'));
    assert.equal(ctrl.getMode(), "popout");

    ctrl.openFullModal(SAMPLE_PROFILE);
    assert.equal(container.querySelector('[data-profile-mode="popout"]'), null);
    assert.ok(container.querySelector('[data-profile-mode="modal"]'));
    assert.equal(ctrl.getMode(), "modal");

    ctrl.close();
    assert.equal(container.querySelector('[data-profile-mode="modal"]'), null);
    assert.equal(container.children.length, 0);
    assert.equal(ctrl.getMode(), null);

    // Повторный close() безопасен и идемпотентен
    assert.doesNotThrow(() => ctrl.close());
  });

  it("поддерживает поле about как синоним bio, скрывает пустые местоимения и отвергает невалидный containerEl", () => {
    assert.throws(
      () => createProfileModalController(null),
      TypeError,
    );

    const container = createTestContainer();
    const ctrl = createProfileModalController(container);
    ctrl.openPopout({
      id: "u-99",
      username: "guest",
      about: "Текст из поля about",
    });

    const bioEl = container.querySelector("[data-profile-bio]");
    assert.ok(bioEl);
    assert.equal(bioEl.textContent, "Текст из поля about");

    const pronounsEl = container.querySelector("[data-profile-pronouns]");
    assert.ok(pronounsEl);
    assert.equal(pronounsEl.hidden, true);
  });
});
