// @ts-check
/**
 * T-226 (FE-P1-22 / RW-40 / G2): unit-тесты контроллера модального окна
 * быстрого перехода Quick Switcher (Ctrl+K) (`rust/gui-spike/ui/features/quick-switcher.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  QUICK_SWITCHER_PREFIXES,
  MAX_QUICK_SWITCHER_RESULTS,
  scoreAndFilterQuickSwitcherItems,
  createQuickSwitcherController,
} from "../../rust/gui-spike/ui/features/quick-switcher.js";

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
    this.hidden = false;
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
    if (name === "placeholder") this._placeholder = str;
    if (name === "hidden") this.hidden = true;
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
    if (name === "hidden") this.hidden = false;
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
    activeElement: /** @type {FakeElement | null} */ (null),
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

const SAMPLE_ITEMS = [
  { id: "u-alice", type: /** @type {const} */ ("user"), name: "alice", mentionCount: 2, unreadCount: 5 },
  { id: "u-alex", type: /** @type {const} */ ("user"), name: "alex", mentionCount: 0, unreadCount: 0 },
  { id: "c-general", type: /** @type {const} */ ("text"), name: "general", guildName: "Discord Lab", unreadCount: 3 },
  { id: "c-gendev", type: /** @type {const} */ ("text"), name: "gen-dev", guildName: "Discord Lab", mentionCount: 1, unreadCount: 4 },
  { id: "c-codegen", type: /** @type {const} */ ("text"), name: "codegen", guildName: "Discord Lab", mentionCount: 5, unreadCount: 10 },
  { id: "v-lobby", type: /** @type {const} */ ("voice"), name: "Lobby Voice", guildName: "Discord Lab" },
  { id: "g-rust", type: /** @type {const} */ ("guild"), name: "Rust Community" },
];

/* ------------------------------------------------------------------ *
 * 1. Константы и чистая функция scoreAndFilterQuickSwitcherItems
 * ------------------------------------------------------------------ */

describe("QUICK_SWITCHER_PREFIXES, MAX_QUICK_SWITCHER_RESULTS и scoreAndFilterQuickSwitcherItems (FE-P1-22)", () => {
  it("экспортирует замороженный словарь QUICK_SWITCHER_PREFIXES и лимит MAX_QUICK_SWITCHER_RESULTS = 10", () => {
    assert.equal(Object.isFrozen(QUICK_SWITCHER_PREFIXES), true);
    assert.deepEqual(QUICK_SWITCHER_PREFIXES, {
      USER: "@",
      TEXT_CHANNEL: "#",
      VOICE_CHANNEL: "!",
      GUILD: "*",
    });
    assert.equal(MAX_QUICK_SWITCHER_RESULTS, 10);
  });

  it("фильтрует элементы по префиксам '@', '#', '!' и '*' и возвращает все элементы соответствующего типа при пустом остатке запроса", () => {
    const users = scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "@");
    assert.deepEqual(
      users.map((item) => item.id),
      ["u-alice", "u-alex"],
    );

    const textChannels = scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "#");
    assert.deepEqual(
      textChannels.map((item) => item.id),
      ["c-codegen", "c-gendev", "c-general"],
    );

    const voiceChannels = scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "!");
    assert.deepEqual(
      voiceChannels.map((item) => item.id),
      ["v-lobby"],
    );

    const guilds = scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "*");
    assert.deepEqual(
      guilds.map((item) => item.id),
      ["g-rust"],
    );

    const filteredTextWithQuery = scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "#gen");
    assert.deepEqual(
      filteredTextWithQuery.map((item) => item.id),
      ["c-gendev", "c-general", "c-codegen"],
    );
  });

  it("выполняет регистронезависимый поиск по подстроке и нечёткой подпоследовательности (fuzzy subsequence) на item.name", () => {
    // Fuzzy subsequence: "gnrl" -> "general"
    const fuzzy = scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "gnrl");
    assert.deepEqual(
      fuzzy.map((item) => item.id),
      ["c-general"],
    );

    // Регистронезависимая подпоследовательность с префиксом: "!lbyvc" -> "Lobby Voice"
    const fuzzyVoice = scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "!LBYVC");
    assert.deepEqual(
      fuzzyVoice.map((item) => item.id),
      ["v-lobby"],
    );

    // Отсутствие совпадений
    assert.deepEqual(scoreAndFilterQuickSwitcherItems(SAMPLE_ITEMS, "zzzz-no-match"), []);
  });

  it("ранжирует результаты: точный префикс имени -> mentionCount > 0 -> unreadCount > 0 -> по алфавиту, и обрезает до MAX_QUICK_SWITCHER_RESULTS (10)", () => {
    const rankingItems = [
      { id: "1", type: /** @type {const} */ ("text"), name: "zeta-gen", mentionCount: 10, unreadCount: 20 },
      { id: "2", type: /** @type {const} */ ("text"), name: "gen-alpha", mentionCount: 0, unreadCount: 0 },
      { id: "3", type: /** @type {const} */ ("text"), name: "gen-beta", mentionCount: 0, unreadCount: 4 },
      { id: "4", type: /** @type {const} */ ("text"), name: "gen-gamma", mentionCount: 2, unreadCount: 0 },
      { id: "5", type: /** @type {const} */ ("text"), name: "gen-aardvark", mentionCount: 0, unreadCount: 0 },
    ];
    const snapshot = rankingItems.map((item) => item.id);

    const ranked = scoreAndFilterQuickSwitcherItems(rankingItems, "gen");
    // Элементы с точным префиксом "gen" идут перед "zeta-gen" (несмотря на упоминания у "zeta-gen"):
    // 1) "gen-gamma" (префикс + mentionCount > 0)
    // 2) "gen-beta" (префикс + unreadCount > 0)
    // 3) "gen-aardvark", "gen-alpha" (префикс, по алфавиту)
    // 4) "zeta-gen" (подстрока без префикса)
    assert.deepEqual(
      ranked.map((item) => item.id),
      ["4", "3", "5", "2", "1"],
    );
    assert.deepEqual(
      rankingItems.map((item) => item.id),
      snapshot,
      "входной массив не должен мутироваться",
    );

    // Проверка обрезки до MAX_QUICK_SWITCHER_RESULTS = 10
    const manyItems = Array.from({ length: 16 }, (_, idx) => ({
      id: `ch-${idx + 1}`,
      type: /** @type {const} */ ("text"),
      name: `channel-${String(idx + 1).padStart(2, "0")}`,
    }));
    const sliced = scoreAndFilterQuickSwitcherItems(manyItems, "");
    assert.equal(sliced.length, MAX_QUICK_SWITCHER_RESULTS);
    assert.equal(sliced[0]?.name, "channel-01");
    assert.equal(sliced[9]?.name, "channel-10");
  });
});

/* ------------------------------------------------------------------ *
 * 2. Контроллер модального окна createQuickSwitcherController
 * ------------------------------------------------------------------ */

describe("createQuickSwitcherController — рендер, фильтрация, навигация и выбор (FE-P1-22)", () => {
  it("рендерит диалог [data-quick-switcher] (role='dialog', aria-modal='true'), поле [data-quick-switcher-input] с placeholder 'Куда отправимся?', список [data-quick-switcher-results], опции [data-quick-switcher-option] и бейджи [data-quick-switcher-badge]", () => {
    const container = makeContainer();
    const controller = createQuickSwitcherController({ container });

    controller.open(SAMPLE_ITEMS);

    const dialog = mustQuery(container, "[data-quick-switcher]");
    assert.equal(dialog.getAttribute("role"), "dialog");
    assert.equal(dialog.getAttribute("aria-modal"), "true");
    assert.equal(dialog.hasAttribute("hidden"), false);

    const input = mustQuery(dialog, "[data-quick-switcher-input]");
    assert.equal(input.getAttribute("placeholder"), "Куда отправимся?");
    assert.equal(input.value, "");

    const listbox = mustQuery(dialog, "[data-quick-switcher-results]");
    assert.equal(listbox.getAttribute("role"), "listbox");

    const options = listbox.querySelectorAll("[data-quick-switcher-option]");
    assert.equal(options.length, SAMPLE_ITEMS.length);
    assert.equal(options[0]?.getAttribute("aria-selected"), "true");
    assert.equal(options[1]?.getAttribute("aria-selected"), "false");

    // Первые элементы имеют mentionCount/unreadCount -> рендерят [data-quick-switcher-badge]
    const firstBadge = options[0]?.querySelector("[data-quick-switcher-badge]");
    assert.ok(firstBadge, "элемент с упоминаниями/непрочитанными должен иметь [data-quick-switcher-badge]");
    assert.equal(firstBadge.textContent, "2");

    // Элемент без упоминаний и непрочитанных не должен рендерить бейдж
    const lastOption = options[options.length - 1];
    assert.equal(lastOption?.querySelector("[data-quick-switcher-badge]"), null);
  });

  it("setQuery(q) обновляет фильтр, перерисовывает [data-quick-switcher-option] и сбрасывает выделение на первый элемент", () => {
    const container = makeContainer();
    const controller = createQuickSwitcherController({ container });
    controller.open(SAMPLE_ITEMS);

    controller.moveSelection(2);
    controller.setQuery("#gen");

    const input = mustQuery(container, "[data-quick-switcher-input]");
    assert.equal(input.value, "#gen");

    const options = container.querySelectorAll("[data-quick-switcher-option]");
    assert.deepEqual(
      options.map((opt) => opt.getAttribute("data-item-id")),
      ["c-gendev", "c-general", "c-codegen"],
    );
    assert.equal(options[0]?.getAttribute("aria-selected"), "true");
    assert.equal(options[1]?.getAttribute("aria-selected"), "false");
  });

  it("moveSelection(delta) циклически перемещает выделение (wrapping) и обновляет aria-selected", () => {
    const container = makeContainer();
    const controller = createQuickSwitcherController({ container });
    controller.open(SAMPLE_ITEMS.slice(0, 3));

    // Начальный индекс 0 -> шаг -1 переводит на последний элемент (индекс 2)
    controller.moveSelection(-1);
    let options = container.querySelectorAll("[data-quick-switcher-option]");
    assert.equal(options[0]?.getAttribute("aria-selected"), "false");
    assert.equal(options[2]?.getAttribute("aria-selected"), "true");

    // Шаг +1 с последнего (индекс 2) возвращает на первый (индекс 0)
    controller.moveSelection(1);
    options = container.querySelectorAll("[data-quick-switcher-option]");
    assert.equal(options[0]?.getAttribute("aria-selected"), "true");
    assert.equal(options[2]?.getAttribute("aria-selected"), "false");

    // Шаг +2 переводит на индекс 2
    controller.moveSelection(2);
    options = container.querySelectorAll("[data-quick-switcher-option]");
    assert.equal(options[2]?.getAttribute("aria-selected"), "true");
  });

  it("confirmSelection() вызывает onSelectTarget(item) с текущим выбранным элементом и закрывает модальное окно, а close() закрывает без вызова callback", () => {
    const container = makeContainer();
    /** @type {any[]} */
    const selected = [];
    const controller = createQuickSwitcherController({
      container,
      onSelectTarget: (item) => selected.push(item),
    });

    controller.open(SAMPLE_ITEMS);
    controller.setQuery("@");
    controller.moveSelection(1);
    controller.confirmSelection();

    assert.equal(selected.length, 1);
    assert.equal(selected[0]?.id, "u-alex");

    const dialog = mustQuery(container, "[data-quick-switcher]");
    assert.equal(dialog.hasAttribute("hidden"), true);

    // Повторное открытие и закрытие через close() не вызывает onSelectTarget
    controller.open(SAMPLE_ITEMS);
    assert.equal(dialog.hasAttribute("hidden"), false);
    controller.close();
    assert.equal(dialog.hasAttribute("hidden"), true);
    assert.equal(selected.length, 1);
  });
});
