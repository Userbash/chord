// @ts-check
/**
 * T-217 (FE-P1-19 / RW-40 / G2): unit-тесты контроллера пикера выражений
 * (Эмодзи / GIF / Стикеры) (`rust/gui-spike/ui/features/expression-picker.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  EXPRESSION_TABS_RU,
  SKIN_TONE_MODIFIERS,
  MAX_RECENT_EMOJIS,
  applySkinTone,
  recordRecentEmoji,
  createExpressionPickerController,
} from "../../rust/gui-spike/ui/features/expression-picker.js";

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
 * 1. Константы и чистые функции (applySkinTone, recordRecentEmoji)
 * ------------------------------------------------------------------ */

describe("Константы и чистые хелперы пикера выражений (FE-P1-19)", () => {
  it("EXPRESSION_TABS_RU, SKIN_TONE_MODIFIERS и MAX_RECENT_EMOJIS соответствуют контракту и заморожены", () => {
    assert.equal(Object.isFrozen(EXPRESSION_TABS_RU), true);
    assert.deepEqual(EXPRESSION_TABS_RU, {
      emoji: "Эмодзи",
      gif: "GIF",
      stickers: "Стикеры",
    });

    assert.equal(Object.isFrozen(SKIN_TONE_MODIFIERS), true);
    assert.deepEqual(SKIN_TONE_MODIFIERS, [
      "",
      "\u{1F3FB}",
      "\u{1F3FC}",
      "\u{1F3FD}",
      "\u{1F3FE}",
      "\u{1F3FF}",
    ]);

    assert.equal(MAX_RECENT_EMOJIS, 18);
  });

  it("applySkinTone добавляет Fitzpatrick-модификатор только для индексов 1..5 и не меняет базовый эмодзи при невалидных значениях", () => {
    assert.equal(applySkinTone("👋", 0), "👋");
    assert.equal(applySkinTone("👋", -1), "👋");
    assert.equal(applySkinTone("👋", 6), "👋");
    assert.equal(applySkinTone("👋", 99), "👋");
    assert.equal(applySkinTone("👋", Number.NaN), "👋");
    assert.equal(applySkinTone("", 2), "");
    assert.equal(applySkinTone(/** @type {any} */ (null), 2), null);

    assert.equal(applySkinTone("👋", 1), "👋\u{1F3FB}");
    assert.equal(applySkinTone("👋", 2), "👋\u{1F3FC}");
    assert.equal(applySkinTone("👋", 3), "👋\u{1F3FD}");
    assert.equal(applySkinTone("👋", 4), "👋\u{1F3FE}");
    assert.equal(applySkinTone("👋", 5), "👋\u{1F3FF}");
  });

  it("recordRecentEmoji дедуплицирует по id || name, ставит новый элемент в начало и обрезает до MAX_RECENT_EMOJIS (18)", () => {
    const initial = [
      { id: "e1", name: "fire", glyph: "🔥" },
      { name: "smile", glyph: "😀" },
      { id: "e3", name: "rocket", glyph: "🚀" },
    ];

    // Повтор по id перемещает элемент в начало без дублей
    const afterIdDup = recordRecentEmoji(initial, { id: "e3", name: "rocket", glyph: "🚀" });
    assert.equal(afterIdDup.length, 3);
    assert.deepEqual(
      afterIdDup.map((item) => item.name),
      ["rocket", "fire", "smile"],
    );
    // Исходный массив не мутируется
    assert.equal(initial[0]?.name, "fire");

    // Повтор по name (без id) также дедуплицируется
    const afterNameDup = recordRecentEmoji(afterIdDup, { name: "smile", glyph: "😀" });
    assert.deepEqual(
      afterNameDup.map((item) => item.name),
      ["smile", "rocket", "fire"],
    );

    // Ограничение длины MAX_RECENT_EMOJIS = 18
    /** @type {Array<{ id: string, name: string, glyph: string }>} */
    let list = [];
    for (let i = 1; i <= 22; i += 1) {
      list = recordRecentEmoji(list, {
        id: `emoji-${i}`,
        name: `name-${i}`,
        glyph: "⭐",
      });
    }
    assert.equal(list.length, MAX_RECENT_EMOJIS);
    assert.equal(list[0]?.id, "emoji-22");
    assert.equal(list[17]?.id, "emoji-5");
  });
});

/* ------------------------------------------------------------------ *
 * 2. Контроллер createExpressionPickerController
 * ------------------------------------------------------------------ */

describe("createExpressionPickerController — рендер, вкладки, поиск, тон кожи и выбор", () => {
  const sampleEmojis = [
    { id: "wave", name: "wave", glyph: "👋", tags: ["привет", "hello", "рука"] },
    { id: "fire", name: "fire", glyph: "🔥", tags: ["огонь", "hot", "пламя"] },
    { id: "thumbsup", name: "thumbsup", glyph: "👍", tags: ["класс", "лайк", "ok"] },
  ];

  const sampleStickers = [
    { id: "st-wumpus", name: "Wumpus Wave", tags: ["привет", "вампус"] },
    { id: "st-party", name: "Party Blob", tags: ["праздник", "тусовка"] },
  ];

  const sampleGifs = [
    { id: "gif-cat", name: "Cat Vibing", tags: ["кот", "музыка", "танец"], url: "https://example.test/cat.gif" },
    { id: "gif- applause", name: "Slow Clap", tags: ["аплодисменты", "хлопки"], url: "https://example.test/clap.gif" },
  ];

  it("рендерит диалог [data-expression-picker], 3 вкладки, поиск с русским плейсхолдером, селектор тона кожи и сетку", () => {
    const container = makeContainer();
    const controller = createExpressionPickerController({
      container,
      emojis: sampleEmojis,
      stickers: sampleStickers,
      gifs: sampleGifs,
    });

    const picker = mustQuery(container, "[data-expression-picker]");
    assert.equal(picker.getAttribute("role"), "dialog");
    assert.equal(controller.isOpen(), true);

    const tabs = container.querySelectorAll("[data-picker-tab]");
    assert.equal(tabs.length, 3);
    assert.deepEqual(
      tabs.map((tab) => ({
        id: tab.getAttribute("data-picker-tab"),
        label: tab.textContent,
        selected: tab.getAttribute("aria-selected"),
      })),
      [
        { id: "emoji", label: "Эмодзи", selected: "true" },
        { id: "gif", label: "GIF", selected: "false" },
        { id: "stickers", label: "Стикеры", selected: "false" },
      ],
    );

    const searchInput = mustQuery(container, "[data-picker-search]");
    assert.equal(searchInput.getAttribute("placeholder"), "Найти идеальный эмодзи...");

    const skinSelect = mustQuery(container, "[data-skin-tone-select]");
    assert.ok(skinSelect);

    const grid = mustQuery(container, "[data-picker-grid]");
    const items = grid.querySelectorAll("[data-picker-item]");
    assert.equal(items.length, 3);
  });

  it("переключает вкладки emoji / gif / stickers и обновляет aria-selected и элементы сетки", () => {
    const container = makeContainer();
    createExpressionPickerController({
      container,
      emojis: sampleEmojis,
      stickers: sampleStickers,
      gifs: sampleGifs,
    });

    const emojiTab = mustQuery(container, '[data-picker-tab="emoji"]');
    const gifTab = mustQuery(container, '[data-picker-tab="gif"]');
    const stickersTab = mustQuery(container, '[data-picker-tab="stickers"]');
    const grid = mustQuery(container, "[data-picker-grid]");

    // Переключаемся на GIF
    gifTab.click();
    assert.equal(gifTab.getAttribute("aria-selected"), "true");
    assert.equal(emojiTab.getAttribute("aria-selected"), "false");
    assert.equal(stickersTab.getAttribute("aria-selected"), "false");
    assert.equal(grid.querySelectorAll("[data-picker-item]").length, 2);

    // Переключаемся на Стикеры
    stickersTab.click();
    assert.equal(stickersTab.getAttribute("aria-selected"), "true");
    assert.equal(gifTab.getAttribute("aria-selected"), "false");
    assert.equal(grid.querySelectorAll("[data-picker-item]").length, 2);

    // Возвращаемся на Эмодзи
    emojiTab.click();
    assert.equal(emojiTab.getAttribute("aria-selected"), "true");
    assert.equal(stickersTab.getAttribute("aria-selected"), "false");
    assert.equal(grid.querySelectorAll("[data-picker-item]").length, 3);
  });

  it("фильтрует эмодзи, стикеры и GIF по поисковому запросу без учёта регистра по имени и тегам", () => {
    const container = makeContainer();
    createExpressionPickerController({
      container,
      emojis: sampleEmojis,
      stickers: sampleStickers,
      gifs: sampleGifs,
    });

    const searchInput = mustQuery(container, "[data-picker-search]");
    const grid = mustQuery(container, "[data-picker-grid]");
    const gifTab = mustQuery(container, '[data-picker-tab="gif"]');
    const stickersTab = mustQuery(container, '[data-picker-tab="stickers"]');

    // Поиск эмодзи по русскому тегу в верхнем регистре
    searchInput.value = "ОГОНЬ";
    searchInput.dispatchEvent("input");
    let items = grid.querySelectorAll("[data-picker-item]");
    assert.equal(items.length, 1);
    assert.equal(items[0]?.getAttribute("data-picker-item"), "fire");

    // Переключение на вкладку Стикеры и фильтрация по имени/тегу
    stickersTab.click();
    searchInput.value = "вампус";
    searchInput.dispatchEvent("input");
    items = grid.querySelectorAll("[data-picker-item]");
    assert.equal(items.length, 1);
    assert.equal(items[0]?.getAttribute("data-picker-item"), "st-wumpus");

    // Переключение на вкладку GIF и фильтрация по имени
    gifTab.click();
    searchInput.value = "vIbInG";
    searchInput.dispatchEvent("input");
    items = grid.querySelectorAll("[data-picker-item]");
    assert.equal(items.length, 1);
    assert.equal(items[0]?.getAttribute("data-picker-item"), "gif-cat");
  });

  it("применяет выбранный тон кожи к эмодзи и передаёт модифицированный эмодзи в onSelectEmoji", () => {
    const container = makeContainer();
    /** @type {any[]} */
    const selectedEmojis = [];

    createExpressionPickerController({
      container,
      emojis: sampleEmojis,
      onSelectEmoji: (item) => selectedEmojis.push(item),
    });

    const skinSelect = mustQuery(container, "[data-skin-tone-select]");
    skinSelect.value = "3";
    skinSelect.dispatchEvent("change");

    const waveBtn = mustQuery(container, '[data-picker-item="wave"]');
    assert.equal(waveBtn.textContent, "👋\u{1F3FD}");

    waveBtn.click({ shiftKey: true });
    assert.equal(selectedEmojis.length, 1);
    assert.equal(selectedEmojis[0]?.glyph, "👋\u{1F3FD}");
    assert.equal(selectedEmojis[0]?.emoji, "👋\u{1F3FD}");
  });

  it("вызывает соответствующий callback при выборе элемента, закрывает пикер по обычному клику, оставляет открытым при Shift+клике и закрывает по Escape", () => {
    const container = makeContainer();
    /** @type {any[]} */
    const emojisChosen = [];
    /** @type {any[]} */
    const stickersChosen = [];
    /** @type {any[]} */
    const gifsChosen = [];

    const controller = createExpressionPickerController({
      container,
      emojis: sampleEmojis,
      stickers: sampleStickers,
      gifs: sampleGifs,
      onSelectEmoji: (item) => emojisChosen.push(item),
      onSelectSticker: (item) => stickersChosen.push(item),
      onSelectGif: (item) => gifsChosen.push(item),
    });

    const picker = mustQuery(container, "[data-expression-picker]");

    // 1. Shift+клик по эмодзи: вызывает onSelectEmoji, пикер остаётся открытым, recentEmojis пополняется
    const fireBtn = mustQuery(container, '[data-picker-item="fire"]');
    fireBtn.click({ shiftKey: true });
    assert.equal(emojisChosen.length, 1);
    assert.equal(emojisChosen[0]?.id, "fire");
    assert.equal(controller.isOpen(), true);
    assert.equal(picker.getAttribute("data-open"), "true");
    assert.equal(controller.getState().recentEmojis[0]?.id, "fire");

    // 2. Обычный клик по эмодзи (без Shift): вызывает onSelectEmoji и закрывает пикер
    const waveBtn = mustQuery(container, '[data-picker-item="wave"]');
    waveBtn.click({ shiftKey: false });
    assert.equal(emojisChosen.length, 2);
    assert.equal(emojisChosen[1]?.id, "wave");
    assert.equal(controller.isOpen(), false);
    assert.equal(picker.getAttribute("data-open"), "false");

    // 3. Открываем снова, переключаемся на стикеры, выбираем стикер -> закрывается
    controller.open();
    assert.equal(controller.isOpen(), true);
    mustQuery(container, '[data-picker-tab="stickers"]').click();
    mustQuery(container, '[data-picker-item="st-party"]').click();
    assert.equal(stickersChosen.length, 1);
    assert.equal(stickersChosen[0]?.id, "st-party");
    assert.equal(controller.isOpen(), false);

    // 4. Открываем снова, переключаемся на GIF, выбираем с Shift -> открыт, затем Escape -> закрывается
    controller.open();
    mustQuery(container, '[data-picker-tab="gif"]').click();
    mustQuery(container, '[data-picker-item="gif-cat"]').click({ shiftKey: true });
    assert.equal(gifsChosen.length, 1);
    assert.equal(gifsChosen[0]?.id, "gif-cat");
    assert.equal(controller.isOpen(), true);

    picker.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(controller.isOpen(), false);
    assert.equal(picker.getAttribute("data-open"), "false");
  });
});
