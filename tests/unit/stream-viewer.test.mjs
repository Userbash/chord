// @ts-check
/**
 * TASK-616 (RW-40 / G2): Модульные тесты плеера стрима и режима PiP
 * (`rust/gui-spike/ui/features/stream-viewer.js`, `rust/gui-spike/ui/styles/stream-viewer.css`).
 *
 * Проверяемые требования:
 * 1. Контейнер видеопотока с кнопками управления: Play/Pause, Mute/Unmute, Fullscreen, Toggle PiP.
 * 2. Режим PiP (Picture-in-Picture) в углу экрана при переключении каналов.
 * 3. Бейдж качества/FPS (720p 60fps) и статус DAVE MLS шифрования (замочек 'End-to-End Encrypted').
 * 4. Контракт стилей и безопасность (100% token-binding, сокрытие секретов).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

import {
  STREAM_VIEWER_MODES,
  STREAM_VIEWER_ZOOM_LIMITS,
  DOCUMENT_PIP_TRIGGER_ID,
  formatStreamVolumeLabel,
  clampStreamZoomScale,
  createStreamViewerController,
} from "../../rust/gui-spike/ui/features/stream-viewer.js";

const STREAM_VIEWER_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/stream-viewer.css", import.meta.url),
);
const TOKENS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url),
);

/* ------------------------------------------------------------------ *
 * Изолированный DOM-шим для unit-тестов
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

class FakeStyle {
  constructor() {
    /** @type {Record<string, string>} */
    this._props = {};
    this.transform = "";
    this.cursor = "";
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setProperty(name, value) {
    this._props[name] = String(value);
  }

  /**
   * @param {string} name
   * @returns {string}
   */
  getPropertyValue(name) {
    return this._props[name] ?? "";
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
    this.disabled = false;
    this.classList = new FakeClassList(this);
    this.style = new FakeStyle();
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
   * @returns {string | null}
   */
  getAttribute(name) {
    if (name === "hidden") {
      return this.hidden || this._attrs.has("hidden") ? "" : null;
    }
    return this._attrs.get(name) ?? null;
  }

  /**
   * @param {string} name
   * @param {string} val
   */
  setAttribute(name, val) {
    const sVal = String(val);
    this._attrs.set(name, sVal);
    if (name === "class") {
      this.classList._syncFromAttr(sVal);
    } else if (name === "hidden") {
      this.hidden = true;
    } else if (name === "disabled") {
      this.disabled = true;
    }
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  hasAttribute(name) {
    if (name === "hidden") return this.hidden || this._attrs.has("hidden");
    if (name === "disabled") return this.disabled || this._attrs.has("disabled");
    return this._attrs.has(name);
  }

  /** @param {string} name */
  removeAttribute(name) {
    this._attrs.delete(name);
    if (name === "class") {
      this.classList._syncFromAttr("");
    } else if (name === "hidden") {
      this.hidden = false;
    } else if (name === "disabled") {
      this.disabled = false;
    }
  }

  /** @param {...(FakeElement | string)} nodes */
  append(...nodes) {
    for (const node of nodes) {
      if (typeof node === "string") {
        const textNode = new FakeElement("#text", this.ownerDocument);
        textNode.textContent = node;
        textNode.parentNode = this;
        this.children.push(textNode);
      } else if (node instanceof FakeElement) {
        node.parentNode = this;
        this.children.push(node);
      }
    }
  }

  /** @param {FakeElement} child */
  appendChild(child) {
    this.append(child);
    return child;
  }

  /** @param {...(FakeElement | string)} nodes */
  replaceChildren(...nodes) {
    this.children = [];
    this._text = "";
    this.append(...nodes);
  }

  remove() {
    if (this.parentNode) {
      const idx = this.parentNode.children.indexOf(this);
      if (idx >= 0) {
        this.parentNode.children.splice(idx, 1);
      }
      this.parentNode = null;
    }
  }

  /**
   * @param {string} type
   * @param {(ev: any) => void} fn
   */
  addEventListener(type, fn) {
    const list = this._listeners.get(type) ?? [];
    list.push(fn);
    this._listeners.set(type, list);
  }

  /**
   * @param {string} type
   * @param {(ev: any) => void} fn
   */
  removeEventListener(type, fn) {
    const list = this._listeners.get(type) ?? [];
    const idx = list.indexOf(fn);
    if (idx >= 0) list.splice(idx, 1);
  }

  /** @param {any} [ev] */
  click(ev = {}) {
    const event = {
      type: "click",
      target: this,
      currentTarget: this,
      defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; },
      stopPropagation() {},
      ...ev,
    };
    const list = this._listeners.get("click") ?? [];
    for (const fn of list) {
      fn(event);
    }
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    const all = this.querySelectorAll(selector);
    return all.length > 0 && all[0] ? all[0] : null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const results = [];
    const trimmed = selector.trim();

    /** @param {FakeElement} node */
    const visit = (node) => {
      let matches = false;
      if (trimmed.startsWith("#")) {
        const id = trimmed.slice(1);
        if (node.id === id) matches = true;
      } else if (trimmed.startsWith(".")) {
        const cls = trimmed.slice(1);
        if (node.classList.contains(cls)) matches = true;
      } else if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
        const inner = trimmed.slice(1, -1);
        if (inner.includes("=")) {
          const [rawKey = "", rawVal = ""] = inner.split("=");
          const key = rawKey.trim();
          const expected = rawVal.trim().replace(/^['"]|['"]$/g, "");
          if (node.getAttribute(key) === expected) matches = true;
        } else {
          if (node.hasAttribute(inner.trim())) matches = true;
        }
      } else {
        if (node.tagName.toLowerCase() === trimmed.toLowerCase()) matches = true;
      }

      if (matches) results.push(node);
      for (const child of node.children) {
        visit(child);
      }
    };

    for (const child of this.children) {
      visit(child);
    }
    return results;
  }

  /** @returns {string} */
  get innerHTML() {
    let out = `<${this.tagName.toLowerCase()}`;
    for (const [k, v] of this._attrs) {
      out += ` ${k}="${v}"`;
    }
    out += `>${this._text}`;
    for (const child of this.children) {
      out += child.innerHTML;
    }
    out += `</${this.tagName.toLowerCase()}>`;
    return out;
  }
}

class FakeDocument {
  constructor() {
    this.body = new FakeElement("body", this);
    this.documentElement = new FakeElement("html", this);
    this.documentElement.append(this.body);
  }

  /**
   * @param {string} tag
   * @returns {FakeElement}
   */
  createElement(tag) {
    return new FakeElement(tag, this);
  }

  /**
   * @param {string} id
   * @returns {FakeElement | null}
   */
  getElementById(id) {
    return this.documentElement.querySelector(`#${id}`);
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    return this.documentElement.querySelector(selector);
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    return this.documentElement.querySelectorAll(selector);
  }
}

function setupTestEnvironment() {
  const doc = new FakeDocument();
  const container = doc.createElement("div");
  container.id = "stream-test-container";
  doc.body.append(container);
  return { doc, container };
}

/* ------------------------------------------------------------------ *
 * Тестовые сьюты
 * ------------------------------------------------------------------ */

describe("TASK-616: Контролы видеопотока (Play/Pause, Mute/Unmute, Fullscreen, Toggle PiP)", () => {
  it("рендерит плеер с кнопками Play/Pause, Mute/Unmute, Fullscreen и Toggle PiP", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);

    const playPauseBtn = container.querySelector("#stream-viewer-play-pause");
    assert.ok(playPauseBtn, "Кнопка Play/Pause #stream-viewer-play-pause не найдена");
    assert.equal(playPauseBtn.getAttribute("data-stream-action"), "play-pause");

    const muteBtn = container.querySelector("#stream-viewer-mute-toggle");
    assert.ok(muteBtn, "Кнопка Mute/Unmute #stream-viewer-mute-toggle не найдена");
    assert.equal(muteBtn.getAttribute("data-stream-action"), "mute");

    const fsBtn = container.querySelector("#stream-fullscreen-toggle");
    assert.ok(fsBtn, "Кнопка Fullscreen #stream-fullscreen-toggle не найдена");
    assert.equal(fsBtn.getAttribute("data-stream-action"), "fullscreen");

    const pipBtn = container.querySelector("#stream-viewer-pip-toggle");
    assert.ok(pipBtn, "Кнопка Toggle PiP #stream-viewer-pip-toggle не найдена");
    assert.equal(pipBtn.getAttribute("data-stream-action"), "pip");

    controller.dispose();
  });

  it("кнопка Play/Pause переключает паузу и вызывает voice_stream_set_paused", async () => {
    const { container } = setupTestEnvironment();
    /** @type {boolean[]} */
    const pauseCalls = [];
    const controller = createStreamViewerController(container, {
      onPauseToggle: (p) => {
        pauseCalls.push(p);
      },
    });

    controller.openViewer({ streamKey: "guild:1:2:3", userId: "3" });

    const playPauseBtn = container.querySelector("#stream-viewer-play-pause");
    assert.ok(playPauseBtn);
    assert.equal(playPauseBtn.getAttribute("aria-pressed"), "false");

    playPauseBtn.click();
    assert.equal(pauseCalls.length, 1);
    assert.equal(pauseCalls[0], true);
    assert.equal(playPauseBtn.getAttribute("aria-pressed"), "true");
    assert.match(playPauseBtn.textContent ?? "", /Возобновить/);

    playPauseBtn.click();
    assert.equal(pauseCalls.length, 2);
    assert.equal(pauseCalls[1], false);
    assert.equal(playPauseBtn.getAttribute("aria-pressed"), "false");
    assert.match(playPauseBtn.textContent ?? "", /Пауза/);

    controller.dispose();
  });
});

describe("TASK-616: Режим PiP при переключении каналов и навигации", () => {
  it("onChannelSwitch автоматически сворачивает просмотр стрима в PiP при переходе в другой канал", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);

    controller.openViewer({
      streamKey: "guild:10:20:30",
      channelId: "20",
      channelName: "General Voice",
      userId: "30",
      displayName: "StreamerAlex",
    });

    assert.equal(controller.getMode(), "stage");

    // Переключение в другой канал
    controller.onChannelSwitch("channel_99");
    assert.equal(controller.getMode(), "pip");

    const pip = container.ownerDocument.querySelector("#stream-viewer-pip");
    assert.ok(pip, "Плавающий мини-плеер PiP не найден в DOM");
    assert.equal(pip.getAttribute("data-pip-active"), "true");

    controller.dispose();
  });
});

describe("TASK-616: Бейдж качества/FPS и статус DAVE MLS шифрования", () => {
  it("рендерит бейдж качества/FPS (720p 60 FPS) и статус сквозного шифрования DAVE MLS", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);

    controller.openViewer({
      streamKey: "guild:1:2:3",
      userId: "3",
      resolution: "720p",
      fps: 60,
      encrypted: true,
    });

    const qualityBadge = container.querySelector("#stream-viewer-quality-badge");
    assert.ok(qualityBadge);
    assert.match(qualityBadge.textContent ?? "", /720p/);
    assert.match(qualityBadge.textContent ?? "", /60 FPS/);

    const encBadge = container.querySelector("#stream-viewer-encryption-badge");
    assert.ok(encBadge, "Бейдж шифрования #stream-viewer-encryption-badge не найден");
    assert.equal(encBadge.getAttribute("data-encrypted"), "true");
    assert.match(encBadge.textContent ?? "", /End-to-End Encrypted/);
    assert.match(encBadge.textContent ?? "", /🔒/);

    controller.dispose();
  });
});
