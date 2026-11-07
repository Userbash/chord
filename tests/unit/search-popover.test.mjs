// @ts-check
/**
 * TASK-618 (G2 Frontend): модульные тесты поискового попапа с подсказками токенов,
 * подсветкой активных токенов в строке ввода, подсветкой совпадений в результатах,
 * прыжком к сообщению (onJumpToMessage) и клавиатурной навигацией
 * (`rust/gui-spike/ui/features/search.js`, `rust/gui-spike/ui/styles/search.css`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` §1.1–§1.3 (`searchResultsWrap_a98f3b`,
 *   `id="search-result-<id>"`, фильтры-токены `from:`, `mentions:`, `has:`, `in:`,
 *   `before:`, `after:`, `pinned:`);
 * - `docs/re/frontend-d03.md` §1.1–§1.3;
 * - `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md` §5–§6 (чипы `.search-chip-prefix`,
 *   `.search-chip-value`, палитра `rust/gui-spike/ui/tokens.css`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import {
  SEARCH_POPOVER_TOKEN_SUGGESTIONS,
  getSearchTokenSuggestions,
  tokenizeSearchInputForHighlight,
  splitTextBySearchQuery,
  createSearchPopover,
} from "../../rust/gui-spike/ui/features/search.js";

const SEARCH_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/search.css", import.meta.url),
);
const TOKENS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url),
);

/* ------------------------------------------------------------------ *
 * Изолированный DOM-шим для unit-тестов без браузера
 * ------------------------------------------------------------------ */

class FakeClassList {
  /** @param {FakeElement} el */
  constructor(el) {
    this._el = el;
    /** @type {Set<string>} */
    this._set = new Set();
  }

  /** @param {string} raw */
  _syncFromAttr(raw) {
    this._set = new Set(
      String(raw ?? "")
        .split(/\s+/)
        .filter(Boolean),
    );
  }

  _syncToAttr() {
    this._el._attrs.set("class", [...this._set].join(" "));
  }

  /** @param {...string} tokens */
  add(...tokens) {
    for (const t of tokens) {
      if (t) this._set.add(t);
    }
    this._syncToAttr();
  }

  /** @param {...string} tokens */
  remove(...tokens) {
    for (const t of tokens) {
      this._set.delete(t);
    }
    this._syncToAttr();
  }

  /**
   * @param {string} token
   * @param {boolean} [force]
   * @returns {boolean}
   */
  toggle(token, force) {
    const shouldAdd = force === undefined ? !this._set.has(token) : Boolean(force);
    if (shouldAdd) {
      this._set.add(token);
    } else {
      this._set.delete(token);
    }
    this._syncToAttr();
    return shouldAdd;
  }

  /**
   * @param {string} token
   * @returns {boolean}
   */
  contains(token) {
    return this._set.has(token);
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
    this.hidden = false;
    this.classList = new FakeClassList(this);
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
  }

  /** @returns {string} */
  get id() {
    return this._attrs.get("id") ?? "";
  }

  /** @param {string} val */
  set id(val) {
    this.setAttribute("id", val);
  }

  /** @returns {string} */
  get className() {
    return this._attrs.get("class") ?? "";
  }

  /** @param {string} val */
  set className(val) {
    this.setAttribute("class", val);
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
    if (name === "class") this.classList._syncFromAttr(str);
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
    if (name === "class") this.classList._syncFromAttr("");
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
    return this.dispatchEvent("click");
  }

  focus() {
    if (this.ownerDocument) {
      this.ownerDocument.activeElement = this;
    }
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
 * Простой селекторный матчер для тестового DOM-шима (`tag`, `.class`, `#id`, `[attr]`, `[attr="val"]`).
 * @param {FakeElement} el
 * @param {string} selector
 * @returns {boolean}
 */
function matchesSelector(el, selector) {
  const s = selector.trim();
  if (s.startsWith("#")) {
    return el.getAttribute("id") === s.slice(1);
  }
  if (s.startsWith(".")) {
    return el.classList.contains(s.slice(1));
  }
  const attrMatch = /^([a-zA-Z0-9_-]+)?\[([a-zA-Z0-9_-]+)(?:="([^"]*)")?\]$/.exec(s);
  if (attrMatch) {
    const [, tagPart, attrName, attrVal] = attrMatch;
    if (tagPart && el.tagName !== tagPart.toUpperCase()) return false;
    if (!attrName || !el.hasAttribute(attrName)) return false;
    if (attrVal !== undefined) return el.getAttribute(attrName) === attrVal;
    return true;
  }
  return el.tagName === s.toUpperCase();
}

function createFakeContainer() {
  const doc = {
    activeElement: /** @type {FakeElement | null} */ (null),
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement("div", doc);
}

/* ------------------------------------------------------------------ *
 * 1. Контракт выпадающих подсказок токенов
 * ------------------------------------------------------------------ */

describe("TASK-618: выпадающие подсказки поисковых токенов", () => {
  it("SEARCH_POPOVER_TOKEN_SUGGESTIONS содержит все 7 контрактных токенов и заморожен", () => {
    assert.ok(Array.isArray(SEARCH_POPOVER_TOKEN_SUGGESTIONS));
    assert.ok(Object.isFrozen(SEARCH_POPOVER_TOKEN_SUGGESTIONS));
    const tokens = SEARCH_POPOVER_TOKEN_SUGGESTIONS.map((item) => item.token);
    assert.deepEqual(tokens, [
      "from:",
      "mentions:",
      "has:",
      "in:",
      "before:",
      "after:",
      "pinned:",
    ]);
    for (const item of SEARCH_POPOVER_TOKEN_SUGGESTIONS) {
      assert.ok(Object.isFrozen(item));
      assert.equal(typeof item.labelRu, "string");
      assert.ok(item.labelRu.length > 0);
    }
  });

  it("getSearchTokenSuggestions фильтрует подсказки по вводимому префиксу и возвращает все 7 для пустой строки или после пробела", () => {
    assert.equal(getSearchTokenSuggestions("").length, 7);
    assert.equal(getSearchTokenSuggestions("from:alex ").length, 7);
    assert.deepEqual(
      getSearchTokenSuggestions("fr").map((s) => s.token),
      ["from:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("from:alex men").map((s) => s.token),
      ["mentions:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("be").map((s) => s.token),
      ["before:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("af").map((s) => s.token),
      ["after:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("pin").map((s) => s.token),
      ["pinned:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("ha").map((s) => s.token),
      ["has:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("in").map((s) => s.token),
      ["in:"],
    );
  });
});

/* ------------------------------------------------------------------ *
 * 2. Подсветка активных токенов в строке ввода и текста результатов
 * ------------------------------------------------------------------ */

describe("TASK-618: подсветка активных токенов и поискового запроса", () => {
  it("tokenizeSearchInputForHighlight выделяет все 7 активных токенов и сохраняет свободный текст", () => {
    const query =
      "from:alex mentions:bob has:image in:general before:2026-10-20 after:2026-01-01 pinned:true привет мир";
    const segments = tokenizeSearchInputForHighlight(query);
    const tokenSegments = segments.filter((s) => s.type === "token");
    assert.equal(tokenSegments.length, 7);
    assert.deepEqual(
      tokenSegments.map((s) => ({ prefix: s.prefix, value: s.value })),
      [
        { prefix: "from:", value: "alex" },
        { prefix: "mentions:", value: "bob" },
        { prefix: "has:", value: "image" },
        { prefix: "in:", value: "general" },
        { prefix: "before:", value: "2026-10-20" },
        { prefix: "after:", value: "2026-01-01" },
        { prefix: "pinned:", value: "true" },
      ],
    );
    const reconstructed = segments.map((s) => s.text).join("");
    assert.equal(reconstructed, query);
  });

  it("splitTextBySearchQuery регистронезависимо выделяет совпадения поискового запроса и игнорирует токены фильтрации", () => {
    const parts = splitTextBySearchQuery(
      "ProBot блокирует сообщение в канале новости",
      "from:automod сообщение новости",
    );
    const matched = parts.filter((p) => p.match).map((p) => p.text);
    assert.deepEqual(matched, ["сообщение", "новости"]);
    assert.equal(
      parts.map((p) => p.text).join(""),
      "ProBot блокирует сообщение в канале новости",
    );
  });
});

/* ------------------------------------------------------------------ *
 * 3. DOM-контроллер createSearchPopover: подсказки, подсветка, прыжок, клавиатура
 * ------------------------------------------------------------------ */

describe("TASK-618: контроллер поискового попапа createSearchPopover", () => {
  it("рендерит выпадающие подсказки 7 токенов и подставляет токен по клику", () => {
    const container = createFakeContainer();
    /** @type {any[]} */
    const searches = [];
    const popover = createSearchPopover(container, {
      onSearch: (payload) => searches.push(payload),
    });

    const suggestionBtns = popover.root.querySelectorAll("[data-token-suggestion]");
    assert.equal(suggestionBtns.length, 7);
    assert.deepEqual(
      suggestionBtns.map((/** @type {FakeElement} */ b) => b.getAttribute("data-token-suggestion")),
      ["from:", "mentions:", "has:", "in:", "before:", "after:", "pinned:"],
    );

    popover.setQuery("fr");
    const filteredBtns = popover.root.querySelectorAll("[data-token-suggestion]");
    assert.equal(filteredBtns.length, 1);
    assert.equal(filteredBtns[0]?.getAttribute("data-token-suggestion"), "from:");

    filteredBtns[0]?.click();
    assert.equal(popover.getState().query, "from:");
    assert.equal(popover.input.value, "from:");
  });

  it("подсвечивает активные токены в строке ввода и поисковый запрос в списке результатов", () => {
    const container = createFakeContainer();
    const popover = createSearchPopover(container);

    popover.setQuery("from:alex in:новости pinned:true релиз");
    const activeTokenMarks = popover.root.querySelectorAll("[data-active-token]");
    assert.equal(activeTokenMarks.length, 3);
    assert.deepEqual(
      activeTokenMarks.map((/** @type {FakeElement} */ el) => el.getAttribute("data-token-prefix")),
      ["from:", "in:", "pinned:"],
    );
    assert.deepEqual(
      activeTokenMarks.map((/** @type {FakeElement} */ el) => el.getAttribute("data-token-value")),
      ["alex", "новости", "true"],
    );

    popover.setResults({
      totalResults: 2,
      results: [
        {
          id: "1559590835220390014",
          channelId: "1014537851867775060",
          channelName: "новости",
          author: "alex",
          content: "Готовим новый релиз клиента сегодня",
          timestamp: "2026-10-20 14:30",
        },
        {
          id: "1559590835220390015",
          channelId: "1014537851867775060",
          channelName: "новости",
          author: "alex",
          content: "Проверка сборки перед выкладкой",
          timestamp: "2026-10-20 14:35",
        },
      ],
    });

    const resultItems = popover.root.querySelectorAll("[data-search-result-id]");
    assert.equal(resultItems.length, 2);
    assert.equal(resultItems[0]?.getAttribute("id"), "search-result-1559590835220390014");

    const highlights = resultItems[0]?.querySelectorAll("mark[data-search-highlight]") ?? [];
    assert.equal(highlights.length, 1);
    assert.equal(highlights[0]?.textContent, "релиз");
  });

  it("вызывает onJumpToMessage по клику на кнопку прыжка и при навигации стрелками + Enter, закрывает по Escape", () => {
    const container = createFakeContainer();
    /** @type {Array<{ messageId: string, channelId: string }>} */
    const jumps = [];
    let closedCount = 0;

    const popover = createSearchPopover(container, {
      onJumpToMessage: (messageId, channelId) => {
        jumps.push({ messageId, channelId });
      },
      onClose: () => {
        closedCount += 1;
      },
    });

    popover.setQuery("привет");
    popover.setResults([
      {
        id: "msg-101",
        channelId: "ch-1",
        author: "UserA",
        content: "Привет всем в чате",
      },
      {
        id: "msg-102",
        channelId: "ch-2",
        author: "UserB",
        content: "Привет из второго канала",
      },
    ]);

    // 1. Клик по кнопке "Перейти" первой карточки
    const jumpBtns = popover.root.querySelectorAll('[data-action="jump-to-message"]');
    assert.equal(jumpBtns.length, 2);
    jumpBtns[0]?.click();
    assert.deepEqual(jumps, [{ messageId: "msg-101", channelId: "ch-1" }]);

    // 2. Клавиатурная навигация: ArrowDown -> 0, ArrowDown -> 1, ArrowUp -> 0, ArrowUp -> 1 (wrap), Enter -> прыжок
    popover.input.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(popover.getState().activeResultIndex, 0);
    const itemsAfterFirstDown = popover.root.querySelectorAll("[data-search-result-id]");
    assert.equal(itemsAfterFirstDown[0]?.getAttribute("aria-selected"), "true");
    assert.equal(itemsAfterFirstDown[1]?.getAttribute("aria-selected"), "false");

    popover.input.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(popover.getState().activeResultIndex, 1);
    const itemsAfterSecondDown = popover.root.querySelectorAll("[data-search-result-id]");
    assert.equal(itemsAfterSecondDown[0]?.getAttribute("aria-selected"), "false");
    assert.equal(itemsAfterSecondDown[1]?.getAttribute("aria-selected"), "true");

    popover.input.dispatchEvent("keydown", { key: "ArrowUp" });
    assert.equal(popover.getState().activeResultIndex, 0);

    popover.input.dispatchEvent("keydown", { key: "ArrowUp" });
    assert.equal(popover.getState().activeResultIndex, 1);

    popover.input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(jumps.length, 2);
    assert.deepEqual(jumps[1], { messageId: "msg-102", channelId: "ch-2" });

    // 3. Escape закрывает попап и снимает aria-activedescendant
    popover.input.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(popover.getState().isOpen, false);
    assert.equal(popover.root.getAttribute("data-open"), "false");
    assert.equal(popover.root.getAttribute("aria-hidden"), "true");
    assert.equal(popover.input.hasAttribute("aria-activedescendant"), false);
    assert.equal(closedCount, 1);
  });

  it("поддерживает навигацию стрелками и Enter по подсказкам токенов при пустых результатах и экранирует спецсимволы", () => {
    const container = createFakeContainer();
    const popover = createSearchPopover(container);

    popover.setQuery("b");
    assert.deepEqual(
      popover.getState().suggestions.map((s) => s.token),
      ["before:"],
    );
    popover.input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(popover.getState().query, "before:");

    const regexParts = splitTextBySearchQuery("Сборка C++ (v1.0) [beta] готова", "C++ (v1.0)");
    assert.deepEqual(
      regexParts.filter((p) => p.match).map((p) => p.text),
      ["C++", "(v1.0)"],
    );

    assert.throws(() => createSearchPopover(null), TypeError);
  });
});

/* ------------------------------------------------------------------ *
 * 4. Проверка стилей rust/gui-spike/ui/styles/search.css
 * ------------------------------------------------------------------ */

describe("TASK-618: таблица стилей rust/gui-spike/ui/styles/search.css", () => {
  it("содержит все селекторы поискового попапа и использует исключительно токены из tokens.css", () => {
    const css = readFileSync(SEARCH_CSS_PATH, "utf8");
    const tokensCss = readFileSync(TOKENS_CSS_PATH, "utf8");

    const requiredSelectors = [
      ".search-popover",
      ".search-popover-bar",
      ".search-popover-input",
      ".search-popover-input-highlight",
      ".search-input-token",
      ".search-popover-suggestions",
      ".search-popover-suggestion-item",
      ".search-popover-results",
      ".search-popover-result-item",
      ".search-highlight",
      ".search-popover-jump-btn",
    ];
    for (const selector of requiredSelectors) {
      assert.ok(css.includes(selector), `в styles/search.css отсутствует селектор ${selector}`);
    }

    const withoutComments = css.replace(/\/\*[\s\S]*?\*\//g, "");
    assert.doesNotMatch(withoutComments, /#[0-9a-fA-F]{3,8}\b/, "запрещены сырые hex-цвета");
    assert.doesNotMatch(withoutComments, /\brgba?\s*\(/i, "запрещены сырые rgb/rgba()");
    assert.doesNotMatch(withoutComments, /\bhsla?\s*\(/i, "запрещены сырые hsl/hsla()");
    assert.doesNotMatch(withoutComments, /!important/, "запрещён !important");

    const declaredTokens = new Set(
      [...tokensCss.matchAll(/--([\w-]+)\s*:/g)].map((m) => m[1]),
    );
    const usedTokens = new Set(
      [...withoutComments.matchAll(/var\(--([\w-]+)/g)].map((m) => m[1]),
    );
    assert.ok(usedTokens.size > 0, "styles/search.css должен использовать дизайн-токены");
    for (const token of usedTokens) {
      assert.ok(declaredTokens.has(token), `токен --${token} не объявлен в tokens.css`);
    }
  });
});
