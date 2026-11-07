// @ts-check
/**
 * T-204 (RW-40 / G2): unit-тесты контроллера панели поиска и парсера токенов
 * (`rust/gui-spike/ui/features/search-panel.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  parseSearchQueryTokens,
  SEARCH_FILTER_SUGGESTIONS_RU,
  SEARCH_SORT_TABS_RU,
  SEARCH_PAGE_STEP,
  MAX_SEARCH_OFFSET,
  clampSearchOffset,
  createSearchPanelController,
} from "../../rust/gui-spike/ui/features/search-panel.js";

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

  remove() {
    if (this.parentNode) {
      this.parentNode.removeChild(this);
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

/* ------------------------------------------------------------------ *
 * 1. parseSearchQueryTokens
 * ------------------------------------------------------------------ */

describe("parseSearchQueryTokens — разбор поисковых токенов и свободного текста", () => {
  it("разбирает from:, in:, has:, mentions:, pinned:true и оставшийся текст", () => {
    const parsed = parseSearchQueryTokens(
      "from:alice in:general has:link has:image mentions:bob pinned:true привет мир",
    );
    assert.deepEqual(parsed, {
      content: "привет мир",
      author: ["alice"],
      channel: ["general"],
      has: ["link", "image"],
      mentions: ["bob"],
      pinned: true,
    });
  });

  it("поддерживает все 5 валидных значений has:<link|embed|file|image|video> и pinned:false (регистронезависимо)", () => {
    const parsed = parseSearchQueryTokens(
      "HAS:LINK has:embed has:file HAS:IMAGE has:video PINNED:FALSE отчёт",
    );
    assert.deepEqual(parsed, {
      content: "отчёт",
      author: [],
      channel: [],
      has: ["link", "embed", "file", "image", "video"],
      mentions: [],
      pinned: false,
    });
  });

  it("собирает несколько токенов одного типа и оставляет неизвестные или незавершённые токены в content", () => {
    const parsed = parseSearchQueryTokens(
      "from:u1 from:u2 in:dev in:qa mentions:m1 mentions:m2 from: has:unknown pinned:maybe релиз",
    );
    assert.deepEqual(parsed, {
      content: "from: has:unknown pinned:maybe релиз",
      author: ["u1", "u2"],
      channel: ["dev", "qa"],
      has: [],
      mentions: ["m1", "m2"],
      pinned: null,
    });
  });

  it("возвращает пустую структуру для пустой строки и невалидных входов", () => {
    const emptyExpected = {
      content: "",
      author: [],
      channel: [],
      has: [],
      mentions: [],
      pinned: null,
    };
    assert.deepEqual(parseSearchQueryTokens(""), emptyExpected);
    assert.deepEqual(parseSearchQueryTokens("   "), emptyExpected);
    assert.deepEqual(parseSearchQueryTokens(/** @type {any} */ (null)), emptyExpected);
    assert.deepEqual(parseSearchQueryTokens(/** @type {any} */ (undefined)), emptyExpected);
  });
});

/* ------------------------------------------------------------------ *
 * 2. SEARCH_FILTER_SUGGESTIONS_RU
 * ------------------------------------------------------------------ */

describe("SEARCH_FILTER_SUGGESTIONS_RU — русские подсказки фильтров", () => {
  it("содержит ровно 6 контрактных подсказок и заморожен", () => {
    assert.equal(Object.isFrozen(SEARCH_FILTER_SUGGESTIONS_RU), true);
    assert.deepEqual(SEARCH_FILTER_SUGGESTIONS_RU, [
      { token: "from:", labelRu: "от: пользователь" },
      { token: "in:", labelRu: "в: канал" },
      { token: "has:link", labelRu: "содержит: ссылку" },
      { token: "has:file", labelRu: "содержит: файл" },
      { token: "has:image", labelRu: "содержит: изображение" },
      { token: "pinned:true", labelRu: "закреплено: да" },
    ]);
    for (const item of SEARCH_FILTER_SUGGESTIONS_RU) {
      assert.equal(Object.isFrozen(item), true);
    }
  });
});

/* ------------------------------------------------------------------ *
 * 3. createSearchPanelController
 * ------------------------------------------------------------------ */

describe("createSearchPanelController — рендер, сортировка, результаты и пагинация", () => {
  it("рендерит поле поиска, вкладки сортировки, подсказки фильтров и заголовок пустого состояния", () => {
    const container = makeContainer();
    createSearchPanelController(container);

    const input = mustQuery(container, "[data-search-input]");
    assert.equal(input.getAttribute("type"), "search");
    assert.equal(input.getAttribute("placeholder"), "Поиск сообщений...");

    const sortTabs = container.querySelectorAll("[data-sort-tab]");
    assert.equal(sortTabs.length, 3);
    assert.deepEqual(
      sortTabs.map((tab) => ({
        id: tab.getAttribute("data-sort-tab"),
        label: tab.textContent,
        selected: tab.getAttribute("aria-selected"),
      })),
      [
        { id: "newest", label: "Новые", selected: "true" },
        { id: "oldest", label: "Старые", selected: "false" },
        { id: "relevant", label: "Релевантные", selected: "false" },
      ],
    );
    assert.equal(SEARCH_SORT_TABS_RU.length, 3);

    const suggestions = container.querySelectorAll("[data-suggestion-token]");
    assert.equal(suggestions.length, 6);
    assert.equal(suggestions[0]?.getAttribute("data-suggestion-token"), "from:");
    assert.equal(suggestions[0]?.textContent, "от: пользователь");

    const totalHeader = mustQuery(container, "[data-search-total]");
    assert.equal(totalHeader.textContent, "Ничего не найдено");
  });

  it("вызывает onSearch при вводе запроса и переключении вкладок сортировки ('Новые' / 'Старые' / 'Релевантные')", () => {
    const container = makeContainer();
    /** @type {any[]} */
    const calls = [];
    const controller = createSearchPanelController(container, {
      onSearch: (payload) => calls.push(payload),
    });

    const input = mustQuery(container, "[data-search-input]");
    input.value = "from:ivan has:file отчёт";
    input.dispatchEvent("input");

    assert.equal(calls.length, 1);
    assert.deepEqual(calls[0], {
      query: "from:ivan has:file отчёт",
      content: "отчёт",
      author: ["ivan"],
      channel: [],
      has: ["file"],
      mentions: [],
      pinned: null,
      sortBy: "newest",
      offset: 0,
      limit: 25,
    });

    const oldestTab = mustQuery(container, '[data-sort-tab="oldest"]');
    const relevantTab = mustQuery(container, '[data-sort-tab="relevant"]');
    const newestTab = mustQuery(container, '[data-sort-tab="newest"]');

    oldestTab.click();
    assert.equal(oldestTab.getAttribute("aria-selected"), "true");
    assert.equal(newestTab.getAttribute("aria-selected"), "false");
    assert.equal(calls.at(-1)?.sortBy, "oldest");

    relevantTab.click();
    assert.equal(relevantTab.getAttribute("aria-selected"), "true");
    assert.equal(oldestTab.getAttribute("aria-selected"), "false");
    assert.equal(calls.at(-1)?.sortBy, "relevant");

    controller.setSort("newest");
    assert.equal(newestTab.getAttribute("aria-selected"), "true");
    assert.equal(relevantTab.getAttribute("aria-selected"), "false");
    assert.equal(calls.at(-1)?.sortBy, "newest");
  });

  it("рендерит заголовок 'Найдено результатов: N' или 'Ничего не найдено', карточки и кнопку 'Перейти'", () => {
    const container = makeContainer();
    /** @type {Array<{ messageId: string, channelId: string }>} */
    const jumps = [];
    const controller = createSearchPanelController(container, {
      onJumpToMessage: (messageId, channelId) => {
        jumps.push({ messageId, channelId });
      },
    });

    controller.setResults({
      totalCount: 42,
      hits: [
        {
          id: "msg-101",
          channelId: "ch-10",
          author: "alice",
          content: "Первое найденное сообщение",
        },
        {
          id: "msg-202",
          channelId: "ch-20",
          author: "bob",
          content: "Второе найденное сообщение",
        },
      ],
    });

    const totalHeader = mustQuery(container, "[data-search-total]");
    assert.equal(totalHeader.textContent, "Найдено результатов: 42");
    assert.equal(totalHeader.getAttribute("data-total-count"), "42");

    const cards = container.querySelectorAll("[data-search-hit]");
    assert.equal(cards.length, 2);
    assert.equal(mustQuery(cards[0] ?? container, "[data-hit-author]").textContent, "alice");
    assert.equal(
      mustQuery(cards[0] ?? container, "[data-hit-content]").textContent,
      "Первое найденное сообщение",
    );

    const jumpButtons = container.querySelectorAll('[data-action="jump-to-message"]');
    assert.equal(jumpButtons.length, 2);
    assert.equal(jumpButtons[0]?.textContent, "Перейти");
    assert.equal(jumpButtons[1]?.textContent, "Перейти");

    jumpButtons[1]?.click();
    assert.deepEqual(jumps, [{ messageId: "msg-202", channelId: "ch-20" }]);

    controller.setResults({ totalCount: 0, hits: [] });
    assert.equal(totalHeader.textContent, "Ничего не найдено");
    assert.equal(container.querySelectorAll("[data-search-hit]").length, 0);
  });

  it("управляет пагинацией с шагом 25 и зажимает offset в диапазоне 0..=9975", () => {
    assert.equal(SEARCH_PAGE_STEP, 25);
    assert.equal(MAX_SEARCH_OFFSET, 9975);
    assert.equal(clampSearchOffset(Number.NaN), 0);
    assert.equal(clampSearchOffset(-50), 0);
    assert.equal(clampSearchOffset(0), 0);
    assert.equal(clampSearchOffset(25), 25);
    assert.equal(clampSearchOffset(9975), 9975);
    assert.equal(clampSearchOffset(10000), 9975);

    const container = makeContainer();
    /** @type {any[]} */
    const calls = [];
    const controller = createSearchPanelController(container, {
      onSearch: (payload) => calls.push(payload),
    });

    const prevBtn = mustQuery(container, "[data-page-prev]");
    const nextBtn = mustQuery(container, "[data-page-next]");
    assert.equal(prevBtn.getAttribute("aria-disabled"), "true");
    assert.equal(nextBtn.getAttribute("aria-disabled"), "false");

    // Шаг вперёд: 0 -> 25 -> 50
    nextBtn.click();
    assert.equal(controller.getState().offset, 25);
    assert.equal(calls.at(-1)?.offset, 25);
    assert.equal(prevBtn.getAttribute("aria-disabled"), "false");

    nextBtn.click();
    assert.equal(controller.getState().offset, 50);
    assert.equal(calls.at(-1)?.offset, 50);

    // Шаг назад: 50 -> 25 -> 0, и повторный клик не уходит ниже 0
    prevBtn.click();
    assert.equal(controller.getState().offset, 25);
    prevBtn.click();
    assert.equal(controller.getState().offset, 0);
    assert.equal(prevBtn.getAttribute("aria-disabled"), "true");

    prevBtn.click();
    assert.equal(controller.getState().offset, 0);
    assert.equal(calls.at(-1)?.offset, 0);

    // Верхняя граница 9975: 9960 + 25 -> 9975, повторный клик остаётся 9975
    controller.setOffset(9960);
    assert.equal(controller.getState().offset, 9960);
    nextBtn.click();
    assert.equal(controller.getState().offset, 9975);
    assert.equal(nextBtn.getAttribute("aria-disabled"), "true");
    assert.equal(calls.at(-1)?.offset, 9975);

    nextBtn.click();
    assert.equal(controller.getState().offset, 9975);
    assert.equal(calls.at(-1)?.offset, 9975);
  });

  it("бросает TypeError при невалидном контейнере и добавляет токен по клику на подсказку", () => {
    assert.throws(
      () => createSearchPanelController(null),
      /TypeError|Container element must provide ownerDocument\.createElement/,
    );

    const container = makeContainer();
    /** @type {any[]} */
    const calls = [];
    createSearchPanelController(container, {
      onSearch: (payload) => calls.push(payload),
    });

    const hasLinkSuggestion = mustQuery(container, '[data-suggestion-token="has:link"]');
    hasLinkSuggestion.click();

    assert.equal(calls.length, 1);
    assert.deepEqual(calls[0]?.has, ["link"]);
  });
});
