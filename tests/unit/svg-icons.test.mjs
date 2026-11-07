// @ts-check
// RED (task-186): контракт SVG-иконок Discord UI. Реализация — rust/gui-spike/ui/features/svg-icons.js.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  ICON_SIZE,
  SVG_ICONS,
  createIconSvg,
  upgradeGlyphIcons,
} from "../../rust/gui-spike/ui/features/svg-icons.js";

const REQUIRED_ICONS = [
  "home",
  "hash",
  "voice",
  "Announcement",
  "lock",
  "pin",
  "thread",
  "bell",
  "inbox",
  "search",
  "members",
  "settings",
  "mic",
  "mic-off",
  "headset",
  "headset-off",
  "plus",
  "smile",
  "reply",
  "more",
];

class FakeNode {
  /** @param {any} doc @param {string} tagName */
  constructor(doc, tagName) {
    this.ownerDocument = doc;
    this.tagName = tagName.toUpperCase();
    /** @type {Map<string, string>} */
    this.attributes = new Map();
    /** @type {any[]} */
    this.childNodes = [];
    this.parentNode = null;
  }
  /** @param {string} name @param {unknown} value */
  setAttribute(name, value) {
    this.attributes.set(name, String(value));
  }
  /** @param {string} name */
  getAttribute(name) {
    return this.attributes.has(name) ? this.attributes.get(name) : null;
  }
  /** @param {string} name */
  hasAttribute(name) {
    return this.attributes.has(name);
  }
  /** @param {any} child */
  appendChild(child) {
    child.parentNode = this;
    this.childNodes.push(child);
    return child;
  }
  /** @param {...any} nodes */
  replaceChildren(...nodes) {
    for (const child of this.childNodes) child.parentNode = null;
    this.childNodes = [...nodes];
    for (const child of nodes) child.parentNode = this;
  }
  get children() {
    return this.childNodes;
  }
  get textContent() {
    return this.childNodes.map((child) => child.textContent ?? "").join("");
  }
  set textContent(value) {
    this.childNodes = [];
    if (value) this.childNodes.push({ textContent: String(value), parentNode: this });
  }
  /** @param {string} selector */
  querySelectorAll(selector) {
    /** @type {any[]} */
    const found = [];
    const walk = (/** @type {any} */ node) => {
      for (const child of node.childNodes) {
        if (!child.tagName) continue;
        if (selector === "[data-icon]" && child.hasAttribute("data-icon")) found.push(child);
        walk(child);
      }
    };
    walk(this);
    return found;
  }
}

function makeDoc() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeNode(doc, tag);
    },
  };
  return doc;
}

/** @param {any} doc @param {string} name @param {string} glyph */
function makeGlyphHost(doc, name, glyph) {
  const host = doc.createElement("span");
  host.setAttribute("data-icon", name);
  host.textContent = glyph;
  return host;
}

describe("ICON_SIZE и SVG_ICONS", () => {
  it("канонический размер иконки — 20", () => {
    assert.equal(ICON_SIZE, 20);
  });

  it("карта иконок заморожена и содержит ровно 20 канонических имён", () => {
    assert.ok(Object.isFrozen(SVG_ICONS));
    assert.deepEqual(Object.keys(SVG_ICONS).sort(), [...REQUIRED_ICONS].sort());
  });

  it("каждый path — непустая строка d, начинающаяся с команды перемещения", () => {
    for (const name of REQUIRED_ICONS) {
      const d = SVG_ICONS[name] ?? "";
      assert.equal(typeof SVG_ICONS[name], "string", `${name}: d должен быть строкой`);
      assert.ok(d.length > 10, `${name}: путь слишком короткий`);
      assert.match(d, /^[Mm]/, `${name}: путь должен начинаться с M/m`);
    }
  });
});

describe("createIconSvg", () => {
  it("создаёт svg с каноническими атрибутами по умолчанию", () => {
    const doc = makeDoc();
    const svg = createIconSvg(doc, "home");
    assert.ok(svg);
    assert.equal(svg.tagName, "SVG");
    assert.equal(svg.getAttribute("width"), "20");
    assert.equal(svg.getAttribute("height"), "20");
    assert.equal(svg.getAttribute("viewBox"), "0 0 20 20");
    assert.equal(svg.getAttribute("fill"), "currentColor");
    assert.equal(svg.getAttribute("aria-hidden"), "true");
    assert.equal(svg.getAttribute("focusable"), "false");
    assert.equal(svg.getAttribute("data-icon"), "home");
    assert.equal(svg.getAttribute("class"), "ui-icon");
  });

  it("вкладывает ровно один path с d из карты", () => {
    const doc = makeDoc();
    const svg = createIconSvg(doc, "mic-off");
    assert.ok(svg);
    assert.equal(svg.childNodes.length, 1);
    assert.equal(/** @type {any} */ (svg.childNodes[0])?.tagName, "PATH");
    assert.equal(/** @type {any} */ (svg.childNodes[0])?.getAttribute("d"), SVG_ICONS["mic-off"]);
  });

  it("принимает размер и className из options", () => {
    const doc = makeDoc();
    const svg = createIconSvg(doc, "search", { size: 32, className: "icon big" });
    assert.ok(svg);
    assert.equal(svg.getAttribute("width"), "32");
    assert.equal(svg.getAttribute("height"), "32");
    assert.equal(svg.getAttribute("class"), "icon big");
    assert.equal(svg.getAttribute("viewBox"), "0 0 20 20");
  });

  it("дополняет частичные options значениями по умолчанию", () => {
    const doc = makeDoc();
    const svg = createIconSvg(doc, "bell", { size: 24 });
    assert.ok(svg);
    assert.equal(svg.getAttribute("width"), "24");
    assert.equal(svg.getAttribute("class"), "ui-icon");
  });

  it("возвращает null для неизвестного имени и для пустого документа", () => {
    const doc = makeDoc();
    assert.equal(createIconSvg(doc, "unknown-icon"), null);
    assert.equal(createIconSvg(doc, ""), null);
    assert.equal(createIconSvg(doc, "toString"), null);
    assert.equal(createIconSvg(null, "home"), null);
  });
});

describe("upgradeGlyphIcons", () => {
  it("заменяет текстовый глиф на svg внутри [data-icon]", () => {
    const doc = makeDoc();
    const root = doc.createElement("div");
    const host = makeGlyphHost(doc, "hash", "#");
    root.appendChild(host);

    const replaced = upgradeGlyphIcons(root);

    assert.equal(replaced, 1);
    assert.equal(host.childNodes.length, 1);
    assert.equal(host.childNodes[0].tagName, "SVG");
    assert.equal(host.childNodes[0].getAttribute("data-icon"), "hash");
    assert.equal(host.textContent.includes("#"), false);
  });

  it("идемпотентен: повторный вызов не дублирует svg", () => {
    const doc = makeDoc();
    const root = doc.createElement("div");
    const host = makeGlyphHost(doc, "pin", "📌");
    root.appendChild(host);

    assert.equal(upgradeGlyphIcons(root), 1);
    const first = host.childNodes[0];
    assert.equal(upgradeGlyphIcons(root), 0);
    assert.equal(host.childNodes.length, 1);
    assert.equal(host.childNodes[0], first);
  });

  it("обновляет несколько узлов и считает только реально заменённые", () => {
    const doc = makeDoc();
    const root = doc.createElement("nav");
    for (const name of ["home", "thread", "unknown"]) {
      root.appendChild(makeGlyphHost(doc, name, "?"));
    }

    assert.equal(upgradeGlyphIcons(root), 2);
    const hosts = root.children;
    assert.equal(hosts[0].childNodes[0].tagName, "SVG");
    assert.equal(hosts[1].childNodes[0].tagName, "SVG");
    assert.equal(hosts[2].childNodes.length, 1);
    assert.equal(hosts[2].childNodes[0].tagName, undefined);
  });

  it("неизвестная иконка остаётся нетронутой", () => {
    const doc = makeDoc();
    const root = doc.createElement("div");
    const host = makeGlyphHost(doc, "mystery", "✳");
    root.appendChild(host);

    assert.equal(upgradeGlyphIcons(root), 0);
    assert.equal(host.textContent, "✳");
    assert.equal(host.getAttribute("data-icon"), "mystery");
  });

  it("обновляет и сам корень, если он помечен data-icon", () => {
    const doc = makeDoc();
    const host = makeGlyphHost(doc, "plus", "+");

    assert.equal(upgradeGlyphIcons(host), 1);
    assert.equal(host.childNodes[0].tagName, "SVG");
  });

  it("возвращает 0 без исключения для пустого корня", () => {
    assert.equal(upgradeGlyphIcons(null), 0);
    assert.equal(upgradeGlyphIcons(undefined), 0);
  });
});
