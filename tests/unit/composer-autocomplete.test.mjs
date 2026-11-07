// @ts-check
/**
 * T-222 (FE-P1-21, RW-40 / G2): unit-тесты контроллера автодополнения композера
 * для слэш-команд `/`, упоминаний `@user/@role` и текстовых каналов `#channel`
 * (`rust/gui-spike/ui/features/composer-autocomplete.js`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  AUTOCOMPLETE_MAX_SUGGESTIONS,
  AUTOCOMPLETE_KINDS,
  detectAutocompleteTrigger,
  createComposerAutocompleteController,
} from "../../rust/gui-spike/ui/features/composer-autocomplete.js";

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
    this._disabled = false;
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
      preventDefault() {
        this.defaultPrevented = true;
      },
      ...extra,
    };
    for (const listener of this._listeners.get(type) ?? []) {
      listener(event);
    }
    return !event.defaultPrevented;
  }

  click() {
    if (this._disabled) return false;
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

/* ------------------------------------------------------------------ *
 * Тестовые наборы
 * ------------------------------------------------------------------ */

describe("composer-autocomplete: константы и detectAutocompleteTrigger", () => {
  it("экспортирует замороженный AUTOCOMPLETE_KINDS и лимит AUTOCOMPLETE_MAX_SUGGESTIONS = 10", () => {
    assert.equal(AUTOCOMPLETE_MAX_SUGGESTIONS, 10);
    assert.equal(Object.isFrozen(AUTOCOMPLETE_KINDS), true);
    assert.deepEqual(AUTOCOMPLETE_KINDS, {
      USER: "@",
      CHANNEL: "#",
      SLASH: "/",
    });
  });

  it("распознаёт триггер слэш-команды только в начале строки по /^\\/([a-z0-9_-]*)$/i", () => {
    assert.deepEqual(detectAutocompleteTrigger("/", 1), {
      kind: "slash",
      query: "",
      startIndex: 0,
    });
    assert.deepEqual(detectAutocompleteTrigger("/shrug", 6), {
      kind: "slash",
      query: "shrug",
      startIndex: 0,
    });
    assert.deepEqual(detectAutocompleteTrigger("/Table_Flip-2 extra", 13), {
      kind: "slash",
      query: "Table_Flip-2",
      startIndex: 0,
    });

    // Слэш не в начале строки или после пробела внутри аргументов не активирует slash-автодополнение
    assert.equal(detectAutocompleteTrigger("привет /shrug", 13), null);
    assert.equal(detectAutocompleteTrigger(" /shrug", 7), null);
    assert.equal(detectAutocompleteTrigger("/shrug arg", 10), null);
  });

  it("распознаёт триггеры @user и #channel в начале строки или после пробельного символа", () => {
    assert.deepEqual(detectAutocompleteTrigger("@", 1), {
      kind: "user",
      query: "",
      startIndex: 0,
    });
    assert.deepEqual(detectAutocompleteTrigger("Привет @alice!", 13), {
      kind: "user",
      query: "alice",
      startIndex: 7,
    });
    assert.deepEqual(detectAutocompleteTrigger("первая строка\n@mod_role", 23), {
      kind: "user",
      query: "mod_role",
      startIndex: 14,
    });

    assert.deepEqual(detectAutocompleteTrigger("#", 1), {
      kind: "channel",
      query: "",
      startIndex: 0,
    });
    assert.deepEqual(detectAutocompleteTrigger("Загляни в #general-chat сейчас", 23), {
      kind: "channel",
      query: "general-chat",
      startIndex: 10,
    });
  });

  it("возвращает null для email/anchor внутри слова, после пробела или при невалидном вводе", () => {
    assert.equal(detectAutocompleteTrigger("user@example.com", 16), null);
    assert.equal(detectAutocompleteTrigger("https://discord.com#top", 23), null);
    assert.equal(detectAutocompleteTrigger("Привет @alice ", 14), null);
    assert.equal(detectAutocompleteTrigger("", 0), null);
    assert.equal(detectAutocompleteTrigger("обычный текст", 13), null);
    assert.equal(detectAutocompleteTrigger("/недопустимая!", 14), null);
  });
});

describe("composer-autocomplete: createComposerAutocompleteController DOM и навигация", () => {
  const SAMPLE_USERS = [
    { id: "101", username: "alice", displayName: "Алиса" },
    { id: "102", username: "alex", displayName: "Алексей" },
    { id: "103", username: "bob", displayName: "Боб" },
  ];

  const SAMPLE_CHANNELS = [
    { id: "201", name: "general" },
    { id: "202", name: "announcements" },
    { id: "203", name: "gaming-lounge" },
  ];

  const SAMPLE_COMMANDS = [
    { name: "shrug", description: "Добавить ¯\\_(ツ)_/¯ в сообщение" },
    { name: "spoiler", description: "Скрыть текст под спойлером" },
    { name: "nick", description: "Изменить никнейм на сервере" },
  ];

  it("рендерит listbox [data-composer-autocomplete] с заголовками КОМАНДЫ / УЧАСТНИКИ И РОЛИ / ТЕКСТОВЫЕ КАНАЛЫ", () => {
    const container = makeContainer();
    const controller = createComposerAutocompleteController({
      container,
      onInsertReplacement: () => {},
    });

    // 1. Слэш-команды -> КОМАНДЫ
    controller.updateFromInput({
      text: "/s",
      cursorIndex: 2,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });

    let listbox = container.querySelector("[data-composer-autocomplete]");
    assert.ok(listbox, "listbox должен быть отрендерен для /s");
    assert.equal(listbox.getAttribute("role"), "listbox");
    let header = container.querySelector("[data-autocomplete-header]");
    assert.ok(header);
    assert.equal(header.textContent, "КОМАНДЫ");
    let options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options.length, 2); // shrug, spoiler
    assert.equal(options[0]?.getAttribute("aria-selected"), "true");
    assert.equal(options[1]?.getAttribute("aria-selected"), "false");

    // 2. Участники и роли -> УЧАСТНИКИ И РОЛИ
    controller.updateFromInput({
      text: "Привет @al",
      cursorIndex: 10,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });

    listbox = container.querySelector("[data-composer-autocomplete]");
    assert.ok(listbox);
    header = container.querySelector("[data-autocomplete-header]");
    assert.equal(header?.textContent, "УЧАСТНИКИ И РОЛИ");
    options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options.length, 2); // alice, alex
    assert.equal(options[0]?.getAttribute("aria-selected"), "true");
    assert.equal(options[1]?.getAttribute("aria-selected"), "false");

    // 3. Текстовые каналы -> ТЕКСТОВЫЕ КАНАЛЫ
    controller.updateFromInput({
      text: "Смотри #g",
      cursorIndex: 9,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });

    header = container.querySelector("[data-autocomplete-header]");
    assert.equal(header?.textContent, "ТЕКСТОВЫЕ КАНАЛЫ");
    options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options.length, 2); // general, gaming-lounge
  });

  it("ограничивает число опций до AUTOCOMPLETE_MAX_SUGGESTIONS (10) и поддерживает циклическую навигацию moveSelection(delta)", () => {
    const container = makeContainer();
    const manyUsers = Array.from({ length: 15 }, (_, idx) => ({
      id: String(1000 + idx),
      username: `user_${idx + 1}`,
      displayName: `Пользователь ${idx + 1}`,
    }));

    const controller = createComposerAutocompleteController({
      container,
      onInsertReplacement: () => {},
    });

    controller.updateFromInput({
      text: "@user",
      cursorIndex: 5,
      users: manyUsers,
      channels: [],
      commands: [],
    });

    let options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options.length, AUTOCOMPLETE_MAX_SUGGESTIONS);
    assert.equal(options[0]?.getAttribute("aria-selected"), "true");

    // Навигация вверх с 0 оборачивается на последний элемент (9)
    controller.moveSelection(-1);
    options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options[0]?.getAttribute("aria-selected"), "false");
    assert.equal(options[9]?.getAttribute("aria-selected"), "true");

    // Навигация вниз с 9 оборачивается на 0
    controller.moveSelection(1);
    options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options[0]?.getAttribute("aria-selected"), "true");
    assert.equal(options[9]?.getAttribute("aria-selected"), "false");

    // Сдвиг на +2 выбирает индекс 2
    controller.moveSelection(2);
    options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options[2]?.getAttribute("aria-selected"), "true");
  });

  it("confirmSelection() заменяет токен на <@id>, <#id> или '/command ', вызывает onInsertReplacement и закрывает список", () => {
    const container = makeContainer();
    /** @type {Array<{ newText: string, newCursorIndex: number, item: any }>} */
    const inserted = [];

    const controller = createComposerAutocompleteController({
      container,
      onInsertReplacement: (payload) => {
        inserted.push(payload);
      },
    });

    // 1. Вставка упоминания пользователя <@id> в середину текста
    controller.updateFromInput({
      text: "Привет @al, как дела?",
      cursorIndex: 10,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });
    controller.moveSelection(1); // выбираем второго: alex (id: "102")
    controller.confirmSelection();

    assert.equal(inserted.length, 1);
    assert.equal(inserted[0]?.newText, "Привет <@102>, как дела?");
    assert.equal(inserted[0]?.newCursorIndex, "Привет <@102>".length);
    assert.equal(inserted[0]?.item.id, "102");
    assert.equal(container.querySelector("[data-composer-autocomplete]"), null);

    // 2. Вставка канала <#id>
    controller.updateFromInput({
      text: "Переходи в #gen!",
      cursorIndex: 15,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });
    controller.confirmSelection();

    assert.equal(inserted.length, 2);
    assert.equal(inserted[1]?.newText, "Переходи в <#201>!");
    assert.equal(inserted[1]?.newCursorIndex, "Переходи в <#201>".length);
    assert.equal(inserted[1]?.item.id, "201");
    assert.equal(container.querySelector("[data-composer-autocomplete]"), null);

    // 3. Вставка слэш-команды "/command " с пробелом на конце
    controller.updateFromInput({
      text: "/sh",
      cursorIndex: 3,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });
    controller.confirmSelection();

    assert.equal(inserted.length, 3);
    assert.equal(inserted[2]?.newText, "/shrug ");
    assert.equal(inserted[2]?.newCursorIndex, "/shrug ".length);
    assert.equal(inserted[2]?.item.name, "shrug");
    assert.equal(container.querySelector("[data-composer-autocomplete]"), null);
  });

  it("поддерживает выбор опции кликом мыши и закрытие через close() или при отсутствии совпадений", () => {
    const container = makeContainer();
    /** @type {Array<{ newText: string, newCursorIndex: number, item: any }>} */
    const inserted = [];

    const controller = createComposerAutocompleteController({
      container,
      onInsertReplacement: (payload) => {
        inserted.push(payload);
      },
    });

    controller.updateFromInput({
      text: "@",
      cursorIndex: 1,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });

    const options = container.querySelectorAll("[data-autocomplete-option]");
    assert.equal(options.length, 3);
    // Клик по третьей опции (bob, id: "103")
    options[2]?.click();

    assert.equal(inserted.length, 1);
    assert.equal(inserted[0]?.newText, "<@103>");
    assert.equal(inserted[0]?.newCursorIndex, "<@103>".length);
    assert.equal(container.querySelector("[data-composer-autocomplete]"), null);

    // Повторное открытие и закрытие через close()
    controller.updateFromInput({
      text: "#g",
      cursorIndex: 2,
      users: SAMPLE_USERS,
      channels: SAMPLE_CHANNELS,
      commands: SAMPLE_COMMANDS,
    });
    assert.ok(container.querySelector("[data-composer-autocomplete]"));

    controller.close();
    assert.equal(container.querySelector("[data-composer-autocomplete]"), null);
    assert.equal(controller.confirmSelection(), null);
  });
});
