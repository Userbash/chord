// @ts-check
// RED (task-184): контракт locale-switcher.js — реестр локалей, storage, атрибуты lang/data-locale, renderSelect.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  SUPPORTED_LOCALES,
  LOCALE_STORAGE_KEY,
  createLocaleSwitcher,
} from "../../rust/gui-spike/ui/features/locale-switcher.js";

/** @param {string} tag */
function makeElement(tag) {
  return {
    tag,
    attrs: /** @type {Record<string, string>} */ ({}),
    children: /** @type {any[]} */ ([]),
    listeners: /** @type {Record<string, Function[]>} */ ({}),
    value: "",
    textContent: "",
    selected: false,
    /** @param {string} n @param {unknown} v */
    setAttribute(n, v) { this.attrs[n] = String(v); },
    /** @param {string} n */
    getAttribute(n) { return n in this.attrs ? this.attrs[n] : null; },
    /** @param {any} c */
    appendChild(c) { this.children.push(c); return c; },
    /** @param {any[]} cs */
    append(...cs) { for (const c of cs) this.appendChild(c); },
    /** @param {string} t @param {Function} fn */
    addEventListener(t, fn) { (this.listeners[t] ||= []).push(fn); },
    /** @param {string} t @param {Function} fn */
    removeEventListener(t, fn) { this.listeners[t] = (this.listeners[t] || []).filter((f) => f !== fn); },
    /** @param {{type: string}} evt */
    dispatchEvent(evt) { for (const fn of this.listeners[evt.type] || []) fn(evt); return true; },
  };
}

function makeDoc() {
  const root = makeElement("html");
  return {
    documentElement: root,
    /** @param {string} t */
    createElement: (t) => makeElement(t),
  };
}

/** @param {Record<string, string>} [seed] */
function makeStorage(seed = {}) {
  const map = new Map(Object.entries(seed));
  return {
    getItem: (/** @type {string} */ k) => (map.has(k) ? (map.get(k) ?? null) : null),
    setItem: (/** @type {string} */ k, /** @type {string} */ v) => { map.set(k, v); },
    map,
  };
}

describe("реестр локалей", () => {
  it("SUPPORTED_LOCALES заморожен и содержит ru-RU и en-US", () => {
    assert.ok(Object.isFrozen(SUPPORTED_LOCALES));
    assert.deepEqual(SUPPORTED_LOCALES.map((l) => l.code), ["ru", "en-US"]);
    assert.equal(SUPPORTED_LOCALES[0]?.bcp47, "ru-RU");
    assert.equal(SUPPORTED_LOCALES[1]?.bcp47, "en-US");
    assert.equal(SUPPORTED_LOCALES[1]?.name, "English, US");
  });

  it("LOCALE_STORAGE_KEY === discord.locale", () => {
    assert.equal(LOCALE_STORAGE_KEY, "discord.locale");
  });
});

describe("createLocaleSwitcher: инициализация", () => {
  it("без storage берёт defaultLocale ru и синхронизирует атрибуты", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc);
    assert.equal(sw.getLocale(), "ru");
    assert.equal(doc.documentElement.getAttribute("lang"), "ru");
    assert.equal(doc.documentElement.getAttribute("data-locale"), "ru");
  });

  it("читает сохранённую локаль из storage", () => {
    const doc = makeDoc();
    const storage = makeStorage({ [LOCALE_STORAGE_KEY]: "en-US" });
    const sw = createLocaleSwitcher(doc, { storage });
    assert.equal(sw.getLocale(), "en-US");
    assert.equal(doc.documentElement.getAttribute("lang"), "en-US");
    assert.equal(doc.documentElement.getAttribute("data-locale"), "en-US");
  });

  it("невалидное значение в storage откатывается к defaultLocale", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc, { storage: makeStorage({ [LOCALE_STORAGE_KEY]: "xx-YY" }) });
    assert.equal(sw.getLocale(), "ru");
  });

  it("storage без метода getItem не ломает инициализацию", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc, { storage: /** @type {any} */ ({}) });
    assert.equal(sw.getLocale(), "ru");
  });
});

describe("setLocale", () => {
  it("меняет локаль, атрибуты и сохраняет в storage", () => {
    const doc = makeDoc();
    const storage = makeStorage();
    const sw = createLocaleSwitcher(doc, { storage });
    assert.equal(sw.setLocale("en-US"), true);
    assert.equal(sw.getLocale(), "en-US");
    assert.equal(doc.documentElement.getAttribute("lang"), "en-US");
    assert.equal(doc.documentElement.getAttribute("data-locale"), "en-US");
    assert.equal(storage.map.get(LOCALE_STORAGE_KEY), "en-US");
  });

  it("неизвестный код → RangeError, локаль не меняется", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc);
    assert.throws(() => sw.setLocale("de-DE"), RangeError);
    assert.equal(sw.getLocale(), "ru");
    assert.equal(doc.documentElement.getAttribute("lang"), "ru");
  });

  it("невалидный defaultLocale откатывается к ru", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc, { defaultLocale: "nope" });
    assert.equal(sw.getLocale(), "ru");
  });
});

describe("onChange", () => {
  it("слушатель получает новый код", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc);
    /** @type {string[]} */
    const seen = [];
    sw.onChange((code) => seen.push(code));
    sw.setLocale("en-US");
    assert.deepEqual(seen, ["en-US"]);
  });

  it("отписка прекращает уведомления", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc);
    /** @type {string[]} */
    const seen = [];
    const off = sw.onChange((code) => seen.push(code));
    off();
    sw.setLocale("en-US");
    assert.deepEqual(seen, []);
  });
});

describe("renderSelect", () => {
  it("рендерит select с aria-label, data-role и двумя опциями", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc);
    const container = makeElement("div");
    const select = sw.renderSelect(container);
    assert.equal(select.tag, "select");
    assert.equal(select.getAttribute("data-role"), "locale-select");
    assert.equal(select.getAttribute("aria-label"), "Язык интерфейса");
    assert.equal(select.children.length, 2);
    assert.deepEqual(select.children.map((/** @type {any} */ o) => o.getAttribute("value")), ["ru", "en-US"]);
    assert.equal(container.children[0], select);
  });

  it("помечает текущую локаль выбранной", () => {
    const doc = makeDoc();
    const sw = createLocaleSwitcher(doc);
    const select = sw.renderSelect(makeElement("div"));
    const selected = select.children.filter((/** @type {any} */ o) => o.selected);
    assert.equal(selected.length, 1);
    assert.equal(selected[0].getAttribute("value"), "ru");
  });

  it("change на select вызывает setLocale", () => {
    const doc = makeDoc();
    const storage = makeStorage();
    const sw = createLocaleSwitcher(doc, { storage });
    const select = sw.renderSelect(makeElement("div"));
    select.value = "en-US";
    select.dispatchEvent({ type: "change" });
    assert.equal(sw.getLocale(), "en-US");
    assert.equal(doc.documentElement.getAttribute("lang"), "en-US");
    assert.equal(storage.map.get(LOCALE_STORAGE_KEY), "en-US");
  });
});
