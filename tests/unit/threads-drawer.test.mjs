// @ts-check
/**
 * T-208 (RW-40 / G2): unit-тесты контроллера панели веток канала
 * (`rust/gui-spike/ui/features/threads-drawer.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  AUTO_ARCHIVE_DURATIONS_RU,
  createThreadsDrawerController,
} from "../../rust/gui-spike/ui/features/threads-drawer.js";

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

  /** @returns {string} */
  get placeholder() {
    return this._placeholder || this._attrs.get("placeholder") || "";
  }

  /** @param {string} val */
  set placeholder(val) {
    this._placeholder = String(val ?? "");
    this._attrs.set("placeholder", this._placeholder);
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
    if (name === "placeholder") this._placeholder = str;
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

const SAMPLE_THREADS = [
  {
    id: "th-1",
    name: "Рефакторинг редьюсера",
    archived: false,
    messageCount: 14,
    autoArchiveDuration: 60,
    joined: true,
  },
  {
    id: "th-2",
    name: "Обсуждение релиза RW-40",
    archived: false,
    messageCount: 42,
    autoArchiveDuration: 1440,
    joined: false,
  },
  {
    id: "th-3",
    name: "Старый протокол голоса",
    archived: true,
    messageCount: 8,
    autoArchiveDuration: 4320,
    joined: false,
  },
  {
    id: "th-4",
    name: "Архив миграции БД",
    archived: true,
    messageCount: 105,
    autoArchiveDuration: 10080,
    joined: true,
  },
];

describe("AUTO_ARCHIVE_DURATIONS_RU — константный справочник автоархивации", () => {
  it("содержит 4 замороженных срока автоархивации (60, 1440, 4320, 10080 минут) с русскими подписями", () => {
    assert.equal(Object.isFrozen(AUTO_ARCHIVE_DURATIONS_RU), true);
    assert.deepEqual(AUTO_ARCHIVE_DURATIONS_RU, [
      { minutes: 60, labelRu: "1 ч." },
      { minutes: 1440, labelRu: "24 ч." },
      { minutes: 4320, labelRu: "3 дн." },
      { minutes: 10080, labelRu: "1 нед." },
    ]);
  });
});

describe("createThreadsDrawerController — вкладки, поиск и пустое состояние", () => {
  it("рендерит вкладки «Активные ветки» ('active') и «Архивные ветки» ('archived'), поле поиска с placeholder «Поиск веток...» и список активных веток по умолчанию", () => {
    const container = makeContainer();
    createThreadsDrawerController(container, { threads: SAMPLE_THREADS });

    const tabActive = container.querySelector('[data-tab="active"]');
    const tabArchived = container.querySelector('[data-tab="archived"]');
    assert.ok(tabActive, "должна быть вкладка data-tab='active'");
    assert.ok(tabArchived, "должна быть вкладка data-tab='archived'");
    assert.equal(tabActive?.textContent, "Активные ветки");
    assert.equal(tabArchived?.textContent, "Архивные ветки");
    assert.equal(tabActive?.getAttribute("aria-selected"), "true");
    assert.equal(tabArchived?.getAttribute("aria-selected"), "false");

    const searchInput = container.querySelector('[placeholder="Поиск веток..."]');
    assert.ok(searchInput, "должно быть поле поиска с placeholder «Поиск веток...»");

    const items = container.querySelectorAll("[data-thread-id]");
    assert.deepEqual(
      items.map((el) => el.getAttribute("data-thread-id")),
      ["th-1", "th-2"],
    );
  });

  it("переключение между вкладками «Активные ветки» и «Архивные ветки» фильтрует список веток", () => {
    const container = makeContainer();
    createThreadsDrawerController(container, { threads: SAMPLE_THREADS });

    const tabArchived = container.querySelector('[data-tab="archived"]');
    tabArchived?.click();

    assert.equal(
      container.querySelector('[data-tab="archived"]')?.getAttribute("aria-selected"),
      "true",
    );
    assert.equal(
      container.querySelector('[data-tab="active"]')?.getAttribute("aria-selected"),
      "false",
    );

    const archivedItems = container.querySelectorAll("[data-thread-id]");
    assert.deepEqual(
      archivedItems.map((el) => el.getAttribute("data-thread-id")),
      ["th-3", "th-4"],
    );

    const tabActive = container.querySelector('[data-tab="active"]');
    tabActive?.click();

    const activeItems = container.querySelectorAll("[data-thread-id]");
    assert.deepEqual(
      activeItems.map((el) => el.getAttribute("data-thread-id")),
      ["th-1", "th-2"],
    );
  });

  it("поиск по названию фильтрует ветки текущей вкладки и рендерит «Ветки не найдены» при отсутствии совпадений", () => {
    const container = makeContainer();
    createThreadsDrawerController(container, { threads: SAMPLE_THREADS });

    const searchInput = container.querySelector('[placeholder="Поиск веток..."]');
    assert.ok(searchInput);

    searchInput.value = "релиз";
    searchInput.dispatchEvent("input");

    const matched = container.querySelectorAll("[data-thread-id]");
    assert.deepEqual(
      matched.map((el) => el.getAttribute("data-thread-id")),
      ["th-2"],
    );
    assert.equal(container.querySelector("[data-threads-empty]"), null);

    searchInput.value = "несуществующая ветка";
    searchInput.dispatchEvent("input");

    assert.equal(container.querySelectorAll("[data-thread-id]").length, 0);
    const emptyEl = container.querySelector("[data-threads-empty]");
    assert.ok(emptyEl, "должно отображаться пустое состояние");
    assert.equal(emptyEl?.textContent, "Ветки не найдены");
  });

  it("рендерит пустое состояние «Ветки не найдены», если исходный список веток пуст", () => {
    const container = makeContainer();
    createThreadsDrawerController(container, { threads: [] });

    assert.equal(container.querySelectorAll("[data-thread-id]").length, 0);
    const emptyEl = container.querySelector("[data-threads-empty]");
    assert.ok(emptyEl);
    assert.equal(emptyEl?.textContent, "Ветки не найдены");
  });
});

describe("createThreadsDrawerController — карточка ветки, вступление/выход и открытие split-pane", () => {
  it("каждая ветка рендерит name, messageCount, метку автоархивации из AUTO_ARCHIVE_DURATIONS_RU и кнопку вступления/выхода", () => {
    const container = makeContainer();
    createThreadsDrawerController(container, { threads: SAMPLE_THREADS });

    const item1 = container.querySelector('[data-thread-id="th-1"]');
    assert.ok(item1);
    assert.equal(item1?.querySelector("[data-thread-name]")?.textContent, "Рефакторинг редьюсера");
    assert.equal(item1?.querySelector("[data-message-count]")?.textContent, "14");
    assert.equal(item1?.querySelector("[data-auto-archive-label]")?.textContent, "1 ч.");
    assert.equal(item1?.querySelector("[data-thread-join-btn]")?.textContent, "Покинуть ветку");

    const item2 = container.querySelector('[data-thread-id="th-2"]');
    assert.ok(item2);
    assert.equal(item2?.querySelector("[data-thread-name]")?.textContent, "Обсуждение релиза RW-40");
    assert.equal(item2?.querySelector("[data-message-count]")?.textContent, "42");
    assert.equal(item2?.querySelector("[data-auto-archive-label]")?.textContent, "24 ч.");
    assert.equal(item2?.querySelector("[data-thread-join-btn]")?.textContent, "Вступить в ветку");

    // Проверим также метки 3 дн. и 1 нед. во вкладке архивных веток
    container.querySelector('[data-tab="archived"]')?.click();
    const item3 = container.querySelector('[data-thread-id="th-3"]');
    const item4 = container.querySelector('[data-thread-id="th-4"]');
    assert.equal(item3?.querySelector("[data-auto-archive-label]")?.textContent, "3 дн.");
    assert.equal(item4?.querySelector("[data-auto-archive-label]")?.textContent, "1 нед.");
  });

  it("клик по кнопке вступления/выхода вызывает onToggleJoin(thread.id, !thread.joined), меняет подпись кнопки и не вызывает onOpenThread", () => {
    const container = makeContainer();
    /** @type {Array<{ id: string, nextJoined: boolean }>} */
    const joinCalls = [];
    /** @type {string[]} */
    const openedIds = [];

    createThreadsDrawerController(
      container,
      { threads: SAMPLE_THREADS },
      {
        onToggleJoin: (id, nextJoined) => joinCalls.push({ id, nextJoined }),
        onOpenThread: (id) => openedIds.push(id),
      },
    );

    const joinBtn1 = container.querySelector('[data-thread-id="th-1"] [data-thread-join-btn]');
    assert.equal(joinBtn1?.textContent, "Покинуть ветку");
    joinBtn1?.click();

    assert.deepEqual(joinCalls, [{ id: "th-1", nextJoined: false }]);
    assert.equal(
      container.querySelector('[data-thread-id="th-1"] [data-thread-join-btn]')?.textContent,
      "Вступить в ветку",
    );

    const joinBtn2 = container.querySelector('[data-thread-id="th-2"] [data-thread-join-btn]');
    assert.equal(joinBtn2?.textContent, "Вступить в ветку");
    joinBtn2?.click();

    assert.deepEqual(joinCalls, [
      { id: "th-1", nextJoined: false },
      { id: "th-2", nextJoined: true },
    ]);
    assert.equal(
      container.querySelector('[data-thread-id="th-2"] [data-thread-join-btn]')?.textContent,
      "Покинуть ветку",
    );
    assert.equal(openedIds.length, 0);
  });

  it("клик по карточке ветки открывает split-pane ветки и вызывает onOpenThread(thread.id)", () => {
    const container = makeContainer();
    /** @type {string[]} */
    const openedIds = [];

    createThreadsDrawerController(
      container,
      { threads: SAMPLE_THREADS },
      { onOpenThread: (id) => openedIds.push(id) },
    );

    const item1 = container.querySelector('[data-thread-id="th-1"]');
    const item2 = container.querySelector('[data-thread-id="th-2"]');
    item2?.click();
    item1?.click();

    assert.deepEqual(openedIds, ["th-2", "th-1"]);
  });
});

describe("createThreadsDrawerController — createThread валидация и создание", () => {
  it("возвращает { ok: false, errorRu: 'Введите название ветки' } при пустом или пробельном имени и не вызывает onCreateThread", () => {
    const container = makeContainer();
    /** @type {any[]} */
    const created = [];
    const controller = createThreadsDrawerController(
      container,
      { threads: SAMPLE_THREADS },
      { onCreateThread: (payload) => created.push(payload) },
    );

    assert.deepEqual(controller.createThread({ name: "" }), {
      ok: false,
      errorRu: "Введите название ветки",
    });
    assert.deepEqual(controller.createThread({ name: "   \t\n  ", autoArchiveDuration: 60 }), {
      ok: false,
      errorRu: "Введите название ветки",
    });
    assert.deepEqual(controller.createThread({}), {
      ok: false,
      errorRu: "Введите название ветки",
    });
    assert.equal(created.length, 0);
  });

  it("возвращает { ok: false, errorRu: 'Некорректный срок автоархивации' } при недопустимом autoArchiveDuration и не вызывает onCreateThread", () => {
    const container = makeContainer();
    /** @type {any[]} */
    const created = [];
    const controller = createThreadsDrawerController(
      container,
      { threads: SAMPLE_THREADS },
      { onCreateThread: (payload) => created.push(payload) },
    );

    for (const invalidDuration of [0, 30, 120, 720, 9999, -60, Number.NaN]) {
      const res = controller.createThread({
        name: "Новая ветка",
        autoArchiveDuration: invalidDuration,
      });
      assert.deepEqual(res, {
        ok: false,
        errorRu: "Некорректный срок автоархивации",
      });
    }
    assert.equal(created.length, 0);
  });

  it("по умолчанию использует autoArchiveDuration = 1440, принимает все допустимые значения (60|1440|4320|10080), вызывает onCreateThread и возвращает { ok: true }", () => {
    const container = makeContainer();
    /** @type {any[]} */
    const created = [];
    const controller = createThreadsDrawerController(
      container,
      { threads: SAMPLE_THREADS },
      { onCreateThread: (payload) => created.push(payload) },
    );

    const resDefault = controller.createThread({
      name: "  Ветка по умолчанию  ",
      starterMessage: "  Первое сообщение  ",
    });
    assert.deepEqual(resDefault, { ok: true });
    assert.equal(created.length, 1);
    assert.deepEqual(created[0], {
      name: "Ветка по умолчанию",
      autoArchiveDuration: 1440,
      starterMessage: "Первое сообщение",
    });

    for (const validDuration of [60, 1440, 4320, 10080]) {
      const res = controller.createThread({
        name: `Ветка ${validDuration}`,
        autoArchiveDuration: validDuration,
        starterMessage: "Старт",
      });
      assert.deepEqual(res, { ok: true });
    }
    assert.equal(created.length, 5);
  });
});
