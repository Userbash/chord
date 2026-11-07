// @ts-check
/**
 * T-231 (FE-P1-24 / RW-40 / G2): unit-тесты контроллера модального окна
 * создания приглашения на сервер, настроек ссылки и прямого приглашения друзей
 * (`rust/gui-spike/ui/features/invite-modal.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  INVITE_MAX_AGE_OPTIONS_RU,
  INVITE_MAX_USES_OPTIONS_RU,
  formatInviteUrl,
  createInviteModalController,
} from "../../rust/gui-spike/ui/features/invite-modal.js";

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
    this._checked = false;
    this._disabled = false;
    this.selected = false;
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
  get checked() {
    return this._checked;
  }

  /** @param {boolean} val */
  set checked(val) {
    this._checked = Boolean(val);
    if (this._checked) {
      this._attrs.set("checked", "");
    } else {
      this._attrs.delete("checked");
    }
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
  let inBracket = false;
  for (const ch of selector.trim()) {
    if (ch === "[") inBracket = true;
    if (ch === "]") inBracket = false;
    if (/\s/.test(ch) && !inBracket) {
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

  const last = parts[parts.length - 1];
  if (!last || !matchesCompoundSelector(el, last)) return false;

  /** @type {FakeElement | null} */
  let cursor = el.parentNode;
  for (let i = parts.length - 2; i >= 0; i -= 1) {
    const needed = parts[i];
    if (!needed) return false;
    while (cursor && !matchesCompoundSelector(cursor, needed)) {
      cursor = cursor.parentNode;
    }
    if (!cursor) return false;
    cursor = cursor.parentNode;
  }
  return true;
}

function createFakeContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

/* ------------------------------------------------------------------ *
 * 1. Константы и formatInviteUrl
 * ------------------------------------------------------------------ */

describe("INVITE_MAX_AGE_OPTIONS_RU, INVITE_MAX_USES_OPTIONS_RU и formatInviteUrl", () => {
  it("экспортирует замороженные справочники сроков действия и лимитов использований на русском", () => {
    assert.equal(Object.isFrozen(INVITE_MAX_AGE_OPTIONS_RU), true);
    assert.deepEqual(INVITE_MAX_AGE_OPTIONS_RU, [
      { value: 1800, label: "30 минут" },
      { value: 3600, label: "1 час" },
      { value: 21600, label: "6 часов" },
      { value: 43200, label: "12 часов" },
      { value: 86400, label: "1 день" },
      { value: 604800, label: "7 дней" },
      { value: 0, label: "Бессрочно" },
    ]);

    assert.equal(Object.isFrozen(INVITE_MAX_USES_OPTIONS_RU), true);
    assert.deepEqual(INVITE_MAX_USES_OPTIONS_RU, [
      { value: 0, label: "Без ограничений" },
      { value: 1, label: "1 использование" },
      { value: 5, label: "5 использований" },
      { value: 10, label: "10 использований" },
      { value: 25, label: "25 использований" },
      { value: 100, label: "100 использований" },
    ]);
  });

  it("formatInviteUrl обрезает пробелы и возвращает https://discord.gg/<code> либо пустую строку", () => {
    assert.equal(formatInviteUrl("aBc123XyZ"), "https://discord.gg/aBc123XyZ");
    assert.equal(formatInviteUrl("  rust-lab-2026  "), "https://discord.gg/rust-lab-2026");
    assert.equal(formatInviteUrl(""), "");
    assert.equal(formatInviteUrl("   \t\n  "), "");
    // @ts-expect-error проверка невалидных типов во время выполнения
    assert.equal(formatInviteUrl(null), "");
    // @ts-expect-error проверка undefined
    assert.equal(formatInviteUrl(undefined), "");
  });
});

/* ------------------------------------------------------------------ *
 * 2. Контроллер createInviteModalController
 * ------------------------------------------------------------------ */

describe("createInviteModalController", () => {
  const sampleFriends = [
    { id: "u-101", username: "alice_dev", displayName: "Алиса" },
    { id: "u-102", username: "bob_rust", displayName: "Боб" },
    { id: "u-103", username: "charlie_qa", displayName: "Чарли" },
  ];

  it("open() рендерит диалог [data-invite-modal] с заголовком канала, ссылкой и списком друзей", () => {
    const container = createFakeContainer();
    const controller = createInviteModalController({ container });

    controller.open({
      channelName: "общий-чат",
      inviteCode: "k9LmN2pQ",
      friends: sampleFriends,
    });

    const modal = container.querySelector("[data-invite-modal]");
    assert.ok(modal, "ожидался диалог [data-invite-modal]");
    assert.equal(modal.getAttribute("role"), "dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true");

    const titleEl = modal.querySelector("[data-invite-channel-title]");
    assert.ok(titleEl, "ожидался заголовок канала [data-invite-channel-title]");
    assert.ok(titleEl.textContent.includes("общий-чат"));

    const urlInput = modal.querySelector("[data-invite-url-input]");
    assert.ok(urlInput, "ожидалось поле ссылки [data-invite-url-input]");
    assert.equal(urlInput.value, "https://discord.gg/k9LmN2pQ");

    const copyBtn = modal.querySelector("[data-invite-copy-btn]");
    assert.ok(copyBtn, "ожидалась кнопка копирования [data-invite-copy-btn]");
    assert.equal(copyBtn.textContent, "Копировать");

    const rows = modal.querySelectorAll("[data-invite-friend-row]");
    assert.equal(rows.length, 3);
    assert.ok(rows[0]?.textContent.includes("Алиса"));
    const firstBtn = rows[0]?.querySelector("[data-invite-friend-btn]");
    assert.ok(firstBtn);
    assert.equal(firstBtn.textContent, "Пригласить");
    assert.equal(firstBtn.disabled, false);
  });

  it("клик по [data-invite-copy-btn] меняет текст на 'Скопировано!' и вызывает onCopyLink(url)", () => {
    const container = createFakeContainer();
    /** @type {string[]} */
    const copiedUrls = [];
    const controller = createInviteModalController({
      container,
      onCopyLink: (url) => copiedUrls.push(url),
    });

    controller.open({
      channelName: "новости",
      inviteCode: "  invCode77  ",
      friends: [],
    });

    const copyBtn = container.querySelector("[data-invite-copy-btn]");
    assert.ok(copyBtn);
    assert.equal(copyBtn.textContent, "Копировать");

    copyBtn.click();

    assert.equal(copyBtn.textContent, "Скопировано!");
    assert.deepEqual(copiedUrls, ["https://discord.gg/invCode77"]);
  });

  it("клик по [data-invite-friend-btn] переводит кнопку в disabled 'Отправлено' и вызывает onSendFriendInvite(userId)", () => {
    const container = createFakeContainer();
    /** @type {string[]} */
    const invitedIds = [];
    const controller = createInviteModalController({
      container,
      onSendFriendInvite: (userId) => invitedIds.push(userId),
    });

    controller.open({
      channelName: "игры",
      inviteCode: "game2026",
      friends: sampleFriends,
    });

    const rows = container.querySelectorAll("[data-invite-friend-row]");
    const secondBtn = rows[1]?.querySelector("[data-invite-friend-btn]");
    assert.ok(secondBtn);
    assert.equal(secondBtn.textContent, "Пригласить");

    secondBtn.click();

    assert.equal(secondBtn.textContent, "Отправлено");
    assert.equal(secondBtn.disabled, true);
    assert.deepEqual(invitedIds, ["u-102"]);

    // Повторный клик по disabled кнопке не вызывает колбэк повторно
    secondBtn.click();
    assert.deepEqual(invitedIds, ["u-102"]);
  });

  it("setSearchQuery(q) и ввод в [data-invite-friend-search] фильтруют список друзей без потери статуса 'Отправлено'", () => {
    const container = createFakeContainer();
    /** @type {string[]} */
    const invitedIds = [];
    const controller = createInviteModalController({
      container,
      onSendFriendInvite: (userId) => invitedIds.push(userId),
    });

    controller.open({
      channelName: "разработка",
      inviteCode: "devInvite",
      friends: sampleFriends,
    });

    // Сначала пригласим Алису
    const firstBtn = container.querySelectorAll("[data-invite-friend-row]")[0]?.querySelector("[data-invite-friend-btn]");
    assert.ok(firstBtn);
    firstBtn.click();
    assert.deepEqual(invitedIds, ["u-101"]);

    // Фильтрация по подстроке имени/username через setSearchQuery
    controller.setSearchQuery("боб");
    let rows = container.querySelectorAll("[data-invite-friend-row]");
    assert.equal(rows.length, 1);
    assert.ok(rows[0]?.textContent.includes("Боб"));

    // Сброс фильтра через input-событие на поле поиска
    const searchInput = container.querySelector("[data-invite-friend-search]");
    assert.ok(searchInput);
    searchInput.value = "алиса";
    searchInput.dispatchEvent("input");

    rows = container.querySelectorAll("[data-invite-friend-row]");
    assert.equal(rows.length, 1);
    const aliceBtnAfterFilter = rows[0]?.querySelector("[data-invite-friend-btn]");
    assert.ok(aliceBtnAfterFilter);
    assert.equal(aliceBtnAfterFilter.textContent, "Отправлено");
    assert.equal(aliceBtnAfterFilter.disabled, true);
  });

  it("панель настроек (maxAge, maxUses, temporary) и updateSettings синхронизируются с [data-invite-regenerate-btn]", () => {
    const container = createFakeContainer();
    /** @type {Array<{ maxAge: number, maxUses: number, temporary: boolean }>} */
    const regenerateCalls = [];
    const controller = createInviteModalController({
      container,
      onRegenerateInvite: (settings) => {
        regenerateCalls.push(settings);
      },
    });

    controller.open({
      channelName: "общий",
      inviteCode: "initCode",
      friends: [],
    });

    const maxAgeSelect = container.querySelector("[data-invite-max-age]");
    const maxUsesSelect = container.querySelector("[data-invite-max-uses]");
    const tempCheckbox = container.querySelector("[data-invite-temporary]");
    const regenBtn = container.querySelector("[data-invite-regenerate-btn]");

    assert.ok(maxAgeSelect, "ожидался селект [data-invite-max-age]");
    assert.ok(maxUsesSelect, "ожидался селект [data-invite-max-uses]");
    assert.ok(tempCheckbox, "ожидался чекбокс [data-invite-temporary]");
    assert.ok(regenBtn, "ожидалась кнопка [data-invite-regenerate-btn]");

    assert.equal(maxAgeSelect.querySelectorAll("option").length, INVITE_MAX_AGE_OPTIONS_RU.length);
    assert.equal(maxUsesSelect.querySelectorAll("option").length, INVITE_MAX_USES_OPTIONS_RU.length);
    assert.equal(maxAgeSelect.value, "86400");
    assert.equal(maxUsesSelect.value, "0");
    assert.equal(tempCheckbox.checked, false);

    // Программное обновление через updateSettings
    controller.updateSettings({
      maxAge: 3600,
      maxUses: 10,
      temporary: true,
    });

    assert.equal(maxAgeSelect.value, "3600");
    assert.equal(maxUsesSelect.value, "10");
    assert.equal(tempCheckbox.checked, true);

    regenBtn.click();
    assert.deepEqual(regenerateCalls, [
      { maxAge: 3600, maxUses: 10, temporary: true },
    ]);
  });

  it("close() удаляет модальное окно [data-invite-modal] из контейнера", () => {
    const container = createFakeContainer();
    const controller = createInviteModalController({ container });

    controller.open({
      channelName: "общий",
      inviteCode: "abc123",
      friends: sampleFriends,
    });
    assert.ok(container.querySelector("[data-invite-modal]"));

    controller.close();
    assert.equal(container.querySelector("[data-invite-modal]"), null);
  });
});
