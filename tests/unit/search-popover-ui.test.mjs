// @ts-check
/**
 * Модульные тесты UI поискового попапа канала (`search-popover.js`, `search-popover.css`).
 *
 * Контракт:
 * - Автодополнение фильтр-токенов (`from:`, `has:`, `mentions:`, `in:`, `before:`, `after:`, `pinned:`)
 *   и значений фильтров (`has:image`, `has:video`, `has:link`, `has:file`, `has:embed`, `from:<user>`,
 *   `mentions:<user>`, `in:<channel>`);
 * - Клавиатурная навигация (`ArrowDown`, `ArrowUp`, `Home`, `End`, `Tab`, `Enter`, `Escape`)
 *   по подсказкам фильтров и карточкам результатов с обновлением `aria-selected` и `aria-activedescendant`;
 * - Подсветка активных токенов в поле ввода и совпадений текста в результатах (`<mark data-search-highlight>`);
 * - Переход к сообщению (`onJumpToMessage`), граничные условия (`offset 0..9975`, Snowflake `0`/`u64::MAX`,
 *   лимит 2000 символов), состояния загрузки/индексации (`HTTP 202`), ошибки (`HTTP 429`), `18+` NSFW-бейдж;
 * - Таблица стилей `search-popover.css` строго на дизайн-токенах `rust/gui-spike/ui/tokens.css`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import {
  SEARCH_POPOVER_TOKEN_SUGGESTIONS,
  HAS_FILTER_VALUE_SUGGESTIONS,
  SEARCH_POPOVER_LIMITS,
  getSearchTokenSuggestions,
  getFilterTokenCompletions,
  completeFilterToken,
  parseSearchPopoverQuery,
  tokenizeSearchInputForHighlight,
  splitTextBySearchQuery,
  clampSearchPopoverOffset,
  createSearchPopover,
  createSearchPopoverController,
} from "../../rust/gui-spike/ui/features/search-popover.js";

const SEARCH_POPOVER_STYLES_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/search-popover.css", import.meta.url),
);
const SEARCH_POPOVER_COMPONENTS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/components/search-popover.css", import.meta.url),
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
 * Простой селекторный матчер (`tag`, `.class`, `#id`, `[attr]`, `[attr="val"]`).
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
 * 1. Автодополнение фильтр-токенов (from:, has:, mentions: и др.)
 * ------------------------------------------------------------------ */

describe("search-popover-ui: 1. Автодополнение фильтр-токенов (from:, has:, mentions:)", () => {
  it("SEARCH_POPOVER_TOKEN_SUGGESTIONS и HAS_FILTER_VALUE_SUGGESTIONS заморожены и соответствуют RE-оракулу", () => {
    assert.ok(Array.isArray(SEARCH_POPOVER_TOKEN_SUGGESTIONS));
    assert.ok(Object.isFrozen(SEARCH_POPOVER_TOKEN_SUGGESTIONS));
    assert.deepEqual(
      SEARCH_POPOVER_TOKEN_SUGGESTIONS.map((item) => item.token),
      ["from:", "mentions:", "has:", "in:", "before:", "after:", "pinned:"],
    );
    for (const item of SEARCH_POPOVER_TOKEN_SUGGESTIONS) {
      assert.ok(Object.isFrozen(item));
      assert.equal(typeof item.labelRu, "string");
      assert.ok(item.labelRu.length > 0);
    }

    assert.ok(Array.isArray(HAS_FILTER_VALUE_SUGGESTIONS));
    assert.ok(Object.isFrozen(HAS_FILTER_VALUE_SUGGESTIONS));
    assert.deepEqual(
      HAS_FILTER_VALUE_SUGGESTIONS.map((item) => item.value),
      ["image", "video", "link", "file", "embed", "sound", "poll", "sticker", "forward"],
    );
  });

  it("getSearchTokenSuggestions и getFilterTokenCompletions дополняют from:, has:, mentions: и значения фильтров", () => {
    assert.equal(getSearchTokenSuggestions("").length, 7);
    assert.equal(getSearchTokenSuggestions("from:alex ").length, 7);
    assert.deepEqual(
      getSearchTokenSuggestions("fr").map((s) => s.token),
      ["from:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("ha").map((s) => s.token),
      ["has:"],
    );
    assert.deepEqual(
      getSearchTokenSuggestions("men").map((s) => s.token),
      ["mentions:"],
    );

    // Автодополнение значений после ввода префикса has:
    const hasCompletions = getFilterTokenCompletions("has:im");
    assert.deepEqual(
      hasCompletions.map((c) => c.token),
      ["has:image"],
    );

    const allHasCompletions = getFilterTokenCompletions("has:");
    assert.equal(allHasCompletions.length, 9);
    assert.equal(allHasCompletions[0]?.token, "has:image");

    // Автодополнение пользователей для from: и mentions: из контекста канала
    const fromCompletions = getFilterTokenCompletions("from:al", {
      users: ["alex", "alice", "bob"],
    });
    assert.deepEqual(
      fromCompletions.map((c) => c.token),
      ["from:alex", "from:alice"],
    );

    const mentionsCompletions = getFilterTokenCompletions("from:alex mentions:bo", {
      users: ["alex", "bob", "boris"],
    });
    assert.deepEqual(
      mentionsCompletions.map((c) => c.token),
      ["mentions:bob", "mentions:boris"],
    );

    // Автодополнение каналов для in:
    const inCompletions = getFilterTokenCompletions("in:ге", {
      channels: ["генеральный", "новости", "флуд"],
    });
    assert.deepEqual(
      inCompletions.map((c) => c.token),
      ["in:генеральный"],
    );
  });

  it("completeFilterToken заменяет только последний незавершённый токен и сохраняет предыдущие токены", () => {
    assert.equal(completeFilterToken("fr", "from:"), "from:");
    assert.equal(completeFilterToken("from:alex ha", "has:"), "from:alex has:");
    assert.equal(completeFilterToken("from:alex has:im", "has:image"), "from:alex has:image ");
    assert.equal(
      completeFilterToken("from:alex men", "mentions:"),
      "from:alex mentions:",
    );
    assert.equal(
      completeFilterToken("from:alex mentions:bo", "mentions:bob"),
      "from:alex mentions:bob ",
    );
    assert.equal(completeFilterToken("", "from:"), "from:");
    assert.equal(completeFilterToken("привет ", "has:"), "привет has:");
  });
});

/* ------------------------------------------------------------------ *
 * 2. Разбор запроса, подсветка активных токенов и текста результатов
 * ------------------------------------------------------------------ */

describe("search-popover-ui: 2. Парсинг и подсветка токенов и поисковых совпадений", () => {
  it("parseSearchPopoverQuery разбирает from:, has:, mentions:, in:, before:, after:, pinned: и свободный текст", () => {
    const parsed = parseSearchPopoverQuery(
      "from:alex mentions:bob has:image has:link in:general before:2026-10-20 after:2026-01-01 pinned:true отчёт по релизу",
    );
    assert.deepEqual(parsed.from, ["alex"]);
    assert.deepEqual(parsed.mentions, ["bob"]);
    assert.deepEqual(parsed.has, ["image", "link"]);
    assert.deepEqual(parsed.in, ["general"]);
    assert.equal(parsed.before, "2026-10-20");
    assert.equal(parsed.after, "2026-01-01");
    assert.equal(parsed.pinned, true);
    assert.equal(parsed.freeText, "отчёт по релизу");
  });

  it("tokenizeSearchInputForHighlight и splitTextBySearchQuery корректно размечают токены и экранируют спецсимволы", () => {
    const query = "from:alex has:image mentions:bob C++ (v1.0)";
    const segments = tokenizeSearchInputForHighlight(query);
    const tokenSegments = segments.filter((s) => s.type === "token");
    assert.deepEqual(
      tokenSegments.map((s) => ({ prefix: s.prefix, value: s.value })),
      [
        { prefix: "from:", value: "alex" },
        { prefix: "has:", value: "image" },
        { prefix: "mentions:", value: "bob" },
      ],
    );
    assert.equal(segments.map((s) => s.text).join(""), query);

    const parts = splitTextBySearchQuery("Сборка C++ (v1.0) [beta] готова", query);
    assert.deepEqual(
      parts.filter((p) => p.match).map((p) => p.text),
      ["C++", "(v1.0)"],
    );
  });
});

/* ------------------------------------------------------------------ *
 * 3. Клавиатурная навигация (ArrowDown, ArrowUp, Home, End, Tab, Enter, Escape)
 * ------------------------------------------------------------------ */

describe("search-popover-ui: 3. Клавиатурная навигация по подсказкам и результатам", () => {
  it("навигация ArrowDown/ArrowUp и автодополнение по Tab/Enter работают для подсказок фильтров (from:, has:, mentions:)", () => {
    const container = createFakeContainer();
    const popover = createSearchPopover(container, {
      users: ["alex", "maria"],
      channels: ["general", "dev"],
    });

    // В начале доступны все 7 базовых фильтр-токенов: from:, mentions:, has:, in:, before:, after:, pinned:
    assert.equal(popover.getState().suggestions.length, 7);
    assert.equal(popover.getState().activeSuggestionIndex, 0);

    // ArrowDown -> 1 (mentions:), ArrowDown -> 2 (has:)
    popover.input.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(popover.getState().activeSuggestionIndex, 1);
    popover.input.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(popover.getState().activeSuggestionIndex, 2);

    const suggestionBtns = popover.root.querySelectorAll("[data-token-suggestion]");
    assert.equal(suggestionBtns[2]?.getAttribute("aria-selected"), "true");
    assert.equal(suggestionBtns[2]?.getAttribute("data-token-suggestion"), "has:");

    // Tab подставляет выбранный токен has:
    popover.input.dispatchEvent("keydown", { key: "Tab" });
    assert.equal(popover.getState().query, "has:");
    assert.equal(popover.input.value, "has:");

    // Теперь при вводе "has:v" предлагается "has:video", Enter завершает его с пробелом
    popover.setQuery("has:v");
    assert.deepEqual(
      popover.getState().suggestions.map((s) => s.token),
      ["has:video"],
    );
    popover.input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(popover.getState().query, "has:video ");

    // Вводим "men" -> предлагается "mentions:", завершаем через Tab
    popover.setQuery("has:video men");
    assert.deepEqual(
      popover.getState().suggestions.map((s) => s.token),
      ["mentions:"],
    );
    popover.input.dispatchEvent("keydown", { key: "Tab" });
    assert.equal(popover.getState().query, "has:video mentions:");

    // Вводим "has:video mentions:ma" -> предлагается "mentions:maria" из контекста users
    popover.setQuery("has:video mentions:ma");
    assert.deepEqual(
      popover.getState().suggestions.map((s) => s.token),
      ["mentions:maria"],
    );
    popover.input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(popover.getState().query, "has:video mentions:maria ");
  });

  it("навигация ArrowDown/ArrowUp/Home/End/Enter/Escape по списку результатов обновляет aria-activedescendant и вызывает onJumpToMessage", () => {
    const container = createFakeContainer();
    /** @type {Array<{ messageId: string, channelId: string }>} */
    const jumps = [];
    let closed = 0;

    const popover = createSearchPopoverController(container, {
      onJumpToMessage: (messageId, channelId) => {
        jumps.push({ messageId, channelId });
      },
      onClose: () => {
        closed += 1;
      },
    });

    popover.setQuery("from:alex релиз");
    popover.setResults({
      totalResults: 3,
      results: [
        { id: "1001", channelId: "ch-1", author: "alex", content: "Первый релиз готов" },
        { id: "1002", channelId: "ch-1", author: "alex", content: "Второй релиз в тесте" },
        { id: "1003", channelId: "ch-2", author: "alex", content: "Третий релиз в проде" },
      ],
    });

    // ArrowDown -> 0
    popover.input.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(popover.getState().activeResultIndex, 0);
    assert.equal(popover.input.getAttribute("aria-activedescendant"), "search-result-1001");

    // End -> 2 (последний элемент)
    popover.input.dispatchEvent("keydown", { key: "End" });
    assert.equal(popover.getState().activeResultIndex, 2);
    assert.equal(popover.input.getAttribute("aria-activedescendant"), "search-result-1003");

    // ArrowDown с последнего элемента циклически переходит на 0
    popover.input.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(popover.getState().activeResultIndex, 0);

    // ArrowUp с 0 циклически переходит на 2
    popover.input.dispatchEvent("keydown", { key: "ArrowUp" });
    assert.equal(popover.getState().activeResultIndex, 2);

    // Home -> 0
    popover.input.dispatchEvent("keydown", { key: "Home" });
    assert.equal(popover.getState().activeResultIndex, 0);

    // ArrowDown -> 1 и Enter -> прыжок к сообщению 1002
    popover.input.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(popover.getState().activeResultIndex, 1);
    popover.input.dispatchEvent("keydown", { key: "Enter" });
    assert.deepEqual(jumps, [{ messageId: "1002", channelId: "ch-1" }]);

    // Escape закрывает попап и очищает aria-activedescendant
    popover.input.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(popover.getState().isOpen, false);
    assert.equal(popover.root.getAttribute("data-open"), "false");
    assert.equal(popover.root.getAttribute("aria-hidden"), "true");
    assert.equal(popover.input.hasAttribute("aria-activedescendant"), false);
    assert.equal(closed, 1);
  });
});

/* ------------------------------------------------------------------ *
 * 4. Граничные условия, HTTP 202 Indexing, HTTP 429 Error, NSFW, История (9 измерений)
 * ------------------------------------------------------------------ */

describe("search-popover-ui: 4. Граничные значения, ошибки 429, индексация 202, NSFW и история", () => {
  it("проверяет граничные значения offset (0..9975), длину запроса (<= 2000) и Snowflake ID (0 / u64::MAX)", () => {
    assert.equal(SEARCH_POPOVER_LIMITS.MAX_OFFSET, 9975);
    assert.equal(SEARCH_POPOVER_LIMITS.PAGE_SIZE, 25);
    assert.equal(SEARCH_POPOVER_LIMITS.MAX_QUERY_LENGTH, 2000);

    assert.equal(clampSearchPopoverOffset(-50), 0);
    assert.equal(clampSearchPopoverOffset(0), 0);
    assert.equal(clampSearchPopoverOffset(25), 25);
    assert.equal(clampSearchPopoverOffset(9975), 9975);
    assert.equal(clampSearchPopoverOffset(10000), 9975);
    assert.equal(clampSearchPopoverOffset(Number.NaN), 0);

    const container = createFakeContainer();
    const popover = createSearchPopover(container);

    const overlong = "а".repeat(2100);
    popover.setQuery(overlong);
    assert.equal(popover.getState().query.length, 2000);

    popover.setResults([
      { id: "0", channelId: "0", author: "system", content: "Genesis message" },
      {
        id: "18446744073709551615",
        channelId: "18446744073709551615",
        author: "max",
        content: "Max snowflake message",
        nsfw: true,
      },
    ]);

    const items = popover.root.querySelectorAll("[data-search-result-id]");
    assert.equal(items.length, 2);
    assert.equal(items[0]?.getAttribute("id"), "search-result-0");
    assert.equal(items[1]?.getAttribute("id"), "search-result-18446744073709551615");

    const nsfwBadge = items[1]?.querySelector("[data-result-nsfw-badge]");
    assert.ok(nsfwBadge, "для NSFW-результата должен рендериться бейдж 18+");
    assert.equal(nsfwBadge?.textContent, "18+");
  });

  it("отображает состояние индексации HTTP 202, ошибку HTTP 429 с retry_after_ms и восстанавливается", () => {
    const container = createFakeContainer();
    const popover = createSearchPopover(container);

    // HTTP 202 Indexing
    popover.setIndexing({ indexing: true, retryAfterMs: 5000 });
    assert.equal(popover.getState().isIndexing, true);
    assert.equal(popover.root.getAttribute("aria-busy"), "true");
    const indexingBanner = popover.root.querySelector("[data-search-indexing-banner]");
    assert.ok(indexingBanner);
    assert.equal(indexingBanner?.hidden, false);

    popover.setIndexing({ indexing: false });
    assert.equal(popover.getState().isIndexing, false);
    assert.equal(popover.root.getAttribute("aria-busy"), "false");

    // HTTP 429 Rate Limit Error
    popover.setError({
      status: 429,
      retryAfterMs: 3000,
      message: "Слишком много запросов. Повторите через 3 сек.",
    });
    assert.equal(popover.getState().error?.status, 429);
    const errorEl = popover.root.querySelector("[data-search-error]");
    assert.ok(errorEl);
    assert.equal(errorEl?.hidden, false);
    assert.match(errorEl?.textContent ?? "", /Слишком много запросов/);

    // Восстановление после ошибки
    popover.clearError();
    assert.equal(popover.getState().error, null);
    assert.equal(errorEl?.hidden, true);

    // Некорректный контейнер бросает TypeError
    assert.throws(() => createSearchPopover(null), TypeError);
    assert.throws(() => createSearchPopover({}), TypeError);
  });

  it("сохраняет историю отправленных поисковых запросов без дубликатов и очищает её", () => {
    const container = createFakeContainer();
    const popover = createSearchPopover(container);

    popover.setQuery("from:alex отчёт");
    popover.input.dispatchEvent("keydown", { key: "Enter" });

    popover.setQuery("has:image макет");
    popover.input.dispatchEvent("keydown", { key: "Enter" });

    popover.setQuery("from:alex отчёт");
    popover.input.dispatchEvent("keydown", { key: "Enter" });

    assert.deepEqual(popover.getState().recentQueries, [
      "from:alex отчёт",
      "has:image макет",
    ]);

    popover.clearHistory();
    assert.deepEqual(popover.getState().recentQueries, []);
  });
});

/* ------------------------------------------------------------------ *
 * 5. Проверка таблицы стилей search-popover.css
 * ------------------------------------------------------------------ */

describe("search-popover-ui: 5. Таблица стилей search-popover.css", () => {
  it("содержит все селекторы поискового попапа и использует исключительно токены из tokens.css", () => {
    assert.ok(existsSync(SEARCH_POPOVER_STYLES_PATH), "styles/search-popover.css должен существовать");
    assert.ok(
      existsSync(SEARCH_POPOVER_COMPONENTS_CSS_PATH),
      "components/search-popover.css должен существовать",
    );

    const css = readFileSync(SEARCH_POPOVER_STYLES_PATH, "utf8");
    const tokensCss = readFileSync(TOKENS_CSS_PATH, "utf8");

    const requiredSelectors = [
      ".search-popover",
      ".search-popover-bar",
      ".search-popover-input",
      ".search-popover-input-highlight",
      ".search-input-token",
      ".search-chip-prefix",
      ".search-chip-value",
      ".search-popover-suggestions",
      ".search-popover-suggestion-item",
      ".search-popover-header",
      ".search-popover-results",
      ".search-popover-result-item",
      ".search-highlight",
      ".search-popover-jump-btn",
      ".search-popover-indexing",
      ".search-popover-error",
      ".search-result-nsfw-badge",
    ];
    for (const selector of requiredSelectors) {
      assert.ok(css.includes(selector), `в search-popover.css отсутствует селектор ${selector}`);
    }

    const withoutComments = css.replace(/\/\*[\s\S]*?\*\//g, "");
    assert.doesNotMatch(withoutComments, /#[0-9a-fA-F]{3,8}\b/, "запрещены сырые hex-цвета");
    assert.doesNotMatch(withoutComments, /\brgba?\s*\(/i, "запрещены сырые rgb/rgba()");
    assert.doesNotMatch(withoutComments, /\bhsla?\s*\(/i, "запрещены сырые hsl/hsla()");
    assert.doesNotMatch(withoutComments, /!important/, "запрещён !important");
    assert.doesNotMatch(withoutComments, /:has\s*\(/i, "запрещён :has()");

    const declaredTokens = new Set(
      [...tokensCss.matchAll(/--([\w-]+)\s*:/g)].map((m) => m[1]),
    );
    const usedTokens = new Set(
      [...withoutComments.matchAll(/var\(--([\w-]+)/g)].map((m) => m[1]),
    );
    assert.ok(usedTokens.size >= 12, "search-popover.css должен использовать дизайн-токены");
    for (const token of usedTokens) {
      assert.ok(declaredTokens.has(token), `токен --${token} не объявлен в tokens.css`);
    }
  });
});
