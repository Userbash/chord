// @ts-check
/**
 * TASK-616 (RW-40 / G2): модульные тесты UI просмотра чужого стрима и режима PiP
 * (`rust/gui-spike/ui/features/stream-viewer-ui.js`, `rust/gui-spike/ui/styles/stream-viewer.css`).
 *
 * Нормативные источники и RE-оракулы:
 * - `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` §2.2.3, §2.3.1, §3.1, §4, §5, §6;
 * - `/home/sanya/re-deep/voice-deep.md`;
 * - `/home/sanya/re-deep/frontend-d03.md` §1 (Voice & Video, Video stage, Watching);
 * - `docs/re/backend-ipc.json` (`DISCORD_WINDOW_OPEN_DOCUMENT_PIP`);
 * - `rust/gui-spike/ui/tokens.css` (100% token-binding).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

import {
  STREAM_VIEWER_MODES,
  STREAM_VIEWER_ZOOM_LIMITS,
  formatStreamVolumeLabel,
  clampStreamZoomScale,
  createStreamViewerController,
} from "../../rust/gui-spike/ui/features/stream-viewer-ui.js";

const STREAM_VIEWER_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/stream-viewer.css", import.meta.url),
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
   * @param {string} type
   * @param {any} [ev]
   */
  dispatchEvent(type, ev = {}) {
    const event = {
      type,
      target: this,
      currentTarget: this,
      defaultPrevented: false,
      preventDefault() { this.defaultPrevented = true; },
      stopPropagation() {},
      ...ev,
    };
    const list = this._listeners.get(type) ?? [];
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

  getBoundingClientRect() {
    return {
      left: 0,
      top: 0,
      width: 1280,
      height: 720,
      right: 1280,
      bottom: 720,
    };
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

describe("TASK-616: Константы и утилиты управления масштабированием и звуком стрима", () => {
  it("STREAM_VIEWER_MODES фиксирует 4 канонических режима отображения", () => {
    assert.deepEqual(STREAM_VIEWER_MODES, Object.freeze(["stage", "pip", "fullscreen", "hidden"]));
  });

  it("STREAM_VIEWER_ZOOM_LIMITS задаёт минимальный 100% и максимальный 500% зум", () => {
    assert.equal(STREAM_VIEWER_ZOOM_LIMITS.MIN, 1.0);
    assert.equal(STREAM_VIEWER_ZOOM_LIMITS.MAX, 5.0);
    assert.equal(STREAM_VIEWER_ZOOM_LIMITS.DEFAULT, 1.0);
    assert.equal(STREAM_VIEWER_ZOOM_LIMITS.DOUBLE_CLICK, 2.25);
  });

  it("clampStreamZoomScale надёжно ограничивает масштаб диапазоном 1.0..5.0", () => {
    assert.equal(clampStreamZoomScale(0.5), 1.0);
    assert.equal(clampStreamZoomScale(1.0), 1.0);
    assert.equal(clampStreamZoomScale(2.5), 2.5);
    assert.equal(clampStreamZoomScale(5.0), 5.0);
    assert.equal(clampStreamZoomScale(7.2), 5.0);
    assert.equal(clampStreamZoomScale(NaN), 1.0);
  });

  it("formatStreamVolumeLabel форматирует процент громкости и статус ВЫКЛ при mute", () => {
    assert.equal(formatStreamVolumeLabel(100, false), "100%");
    assert.equal(formatStreamVolumeLabel(0, false), "ВЫКЛ");
    assert.equal(formatStreamVolumeLabel(150, false), "150%");
    assert.equal(formatStreamVolumeLabel(100, true), "ВЫКЛ");
    assert.equal(formatStreamVolumeLabel(200, false), "200%");
  });
});

describe("TASK-616: DOM-контракт сцены просмотра чужого стрима (#stream-viewer-overlay)", () => {
  it("рендерит #stream-viewer-overlay с каноническими атрибутами, тулбаром и кнопками", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);

    const overlay = container.querySelector("#stream-viewer-overlay");
    assert.ok(overlay, "Элемент #stream-viewer-overlay не найден");
    assert.equal(overlay.getAttribute("role"), "region");
    assert.equal(overlay.getAttribute("aria-label"), "Просмотр трансляции");
    assert.equal(overlay.getAttribute("data-phase"), "idle");
    assert.equal(overlay.getAttribute("data-paused"), "false");
    assert.equal(overlay.getAttribute("data-transport-ready"), "false");
    assert.equal(overlay.hidden, true);

    const canvas = container.querySelector("#stream-viewer-canvas");
    assert.ok(canvas);
    assert.equal(canvas.getAttribute("role"), "img");
    assert.equal(canvas.getAttribute("data-empty"), "true");

    const banner = container.querySelector("#stream-viewer-status-banner");
    assert.ok(banner);
    assert.equal(banner.getAttribute("role"), "status");

    const controls = container.querySelector("#stream-viewer-controls");
    assert.ok(controls);
    assert.equal(controls.getAttribute("role"), "toolbar");

    const streamerName = container.querySelector("#stream-viewer-streamer-name");
    assert.ok(streamerName);

    const phaseBadge = container.querySelector("#stream-viewer-phase-badge");
    assert.ok(phaseBadge);
    assert.equal(phaseBadge.getAttribute("data-state"), "watching");

    const fsBtn = container.querySelector("#stream-fullscreen-toggle");
    assert.ok(fsBtn);
    assert.equal(fsBtn.getAttribute("data-stream-action"), "fullscreen");

    const leaveBtn = container.querySelector("#stream-leave-watch");
    assert.ok(leaveBtn);
    assert.equal(leaveBtn.getAttribute("data-stream-action"), "leave-watch");

    const pipBtn = container.querySelector("#stream-viewer-pip-toggle");
    assert.ok(pipBtn);
    assert.equal(pipBtn.getAttribute("data-stream-action"), "pip");

    const muteBtn = container.querySelector("#stream-viewer-mute-toggle");
    assert.ok(muteBtn);
    assert.equal(muteBtn.getAttribute("data-stream-action"), "mute");

    const volSlider = container.querySelector("#stream-viewer-volume-slider");
    assert.ok(volSlider);

    controller.dispose();
  });

  it("openViewer активирует оверлей, инициирует watchStream и выставляет параметры стримера", async () => {
    const { container } = setupTestEnvironment();
    /** @type {any[]} */
    const watchCalls = [];
    const controller = createStreamViewerController(container, {
      onWatchStream: (streamKeyOrUserId) => {
        watchCalls.push(streamKeyOrUserId);
      },
    });

    controller.openViewer({
      streamKey: "guild:1001:2002:3003",
      userId: "3003",
      displayName: "AliceStreamer",
      channelId: "2002",
      channelName: "Голосовой канал 1",
    });

    assert.equal(watchCalls.length, 1);
    assert.equal(watchCalls[0], "guild:1001:2002:3003");

    const overlay = container.querySelector("#stream-viewer-overlay");
    assert.ok(overlay);
    assert.equal(overlay.hidden, false);
    assert.equal(overlay.getAttribute("data-user-id"), "3003");
    assert.equal(overlay.getAttribute("data-phase"), "connecting");

    const streamerName = container.querySelector("#stream-viewer-streamer-name");
    assert.equal(streamerName?.textContent, "AliceStreamer");

    // Обновление статуса на активный и transport_ready
    controller.setPhase("active");
    controller.setTransportReady(true);
    assert.equal(overlay.getAttribute("data-phase"), "active");
    assert.equal(overlay.getAttribute("data-transport-ready"), "true");

    // Пауза стрима
    controller.setPaused(true);
    assert.equal(overlay.getAttribute("data-paused"), "true");
    assert.equal(overlay.getAttribute("data-phase"), "paused");
    const banner = container.querySelector("#stream-viewer-status-banner");
    assert.match(banner?.textContent ?? "", /приостановлен/i);

    // Возобновление стрима
    controller.setPaused(false);
    assert.equal(overlay.getAttribute("data-paused"), "false");
    assert.equal(overlay.getAttribute("data-phase"), "active");

    controller.dispose();
  });

  it("клик по кнопке 'Выйти из просмотра' вызывает onLeaveWatch и закрывает просмотр", () => {
    const { container } = setupTestEnvironment();
    let leaveWatchKey = null;
    const controller = createStreamViewerController(container, {
      onLeaveWatch: (key) => {
        leaveWatchKey = key;
      },
    });

    controller.openViewer({
      streamKey: "guild:1:2:3",
      userId: "3",
      displayName: "Bob",
    });

    const leaveBtn = container.querySelector("#stream-leave-watch");
    assert.ok(leaveBtn);
    leaveBtn.click();

    assert.equal(leaveWatchKey, "guild:1:2:3");
    const overlay = container.querySelector("#stream-viewer-overlay");
    assert.equal(overlay?.hidden, true);
    assert.equal(controller.getMode(), "hidden");

    controller.dispose();
  });
});

describe("TASK-616: Режим Picture-in-Picture (PiP) и интеграция с DOCUMENT_PIP_TRIGGER", () => {
  it("переключение в режим PiP скрывает overlay и монтирует плавающий мини-плеер #stream-viewer-pip", () => {
    const { container } = setupTestEnvironment();
    /** @type {string[]} */
    const modeEvents = [];
    const controller = createStreamViewerController(container, {
      onModeChange: (m) => modeEvents.push(m),
    });

    controller.openViewer({
      streamKey: "guild:11:22:33",
      userId: "33",
      displayName: "GamerPro",
      channelName: "Gaming Voice",
    });

    assert.equal(controller.getMode(), "stage");

    // Клик по кнопке PiP в контролах оверлея
    const pipToggleBtn = container.querySelector("#stream-viewer-pip-toggle");
    assert.ok(pipToggleBtn);
    pipToggleBtn.click();

    assert.equal(controller.getMode(), "pip");
    assert.ok(modeEvents.includes("pip"));

    const pipWindow = container.ownerDocument.querySelector("#stream-viewer-pip");
    assert.ok(pipWindow, "Мини-плеер #stream-viewer-pip не найден в DOM");
    assert.equal(pipWindow.getAttribute("role"), "region");
    assert.equal(pipWindow.getAttribute("aria-label"), "Мини-плеер трансляции");
    assert.equal(pipWindow.getAttribute("data-pip-active"), "true");

    // Проверяем элементы внутри PiP-окна
    const pipTitle = pipWindow.querySelector(".stream-pip-title");
    assert.equal(pipTitle?.textContent, "GamerPro");

    const pipCanvas = pipWindow.querySelector(".stream-pip-canvas");
    assert.ok(pipCanvas);

    const expandBtn = pipWindow.querySelector("#stream-viewer-pip-expand");
    assert.ok(expandBtn);

    const pipCloseBtn = pipWindow.querySelector("#stream-viewer-pip-close");
    assert.ok(pipCloseBtn);

    // Клик по кнопке expand разворачивает обратно в сцену
    expandBtn.click();
    assert.equal(controller.getMode(), "stage");
    assert.equal(container.ownerDocument.querySelector("#stream-viewer-pip"), null);

    controller.dispose();
  });

  it("DOCUMENT_PIP_TRIGGER_ID (__document_pip_trigger) инициирует переключение PiP через OS IPC", () => {
    const { doc, container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);

    controller.openViewer({
      streamKey: "guild:44:55:66",
      userId: "66",
      displayName: "Alice",
    });

    // Триггер существует в DOM
    const trigger = doc.getElementById("__document_pip_trigger");
    assert.ok(trigger, "Триггер __document_pip_trigger не зарегистрирован в DOM");

    // Симуляция вызова из DiscordNative / Electron preload
    trigger.click();
    assert.equal(controller.getMode(), "pip");

    // Повторный клик разворачивает обратно
    trigger.click();
    assert.equal(controller.getMode(), "stage");

    controller.dispose();
  });

  it("закрытие мини-плеера PiP прекращает просмотр стрима и вызывает onLeaveWatch", () => {
    const { container } = setupTestEnvironment();
    let leftWatch = false;
    const controller = createStreamViewerController(container, {
      onLeaveWatch: () => {
        leftWatch = true;
      },
    });

    controller.openViewer({
      streamKey: "call:100:200",
      userId: "200",
      displayName: "Charlie",
    });

    controller.setMode("pip");
    const pipCloseBtn = container.ownerDocument.querySelector("#stream-viewer-pip-close");
    assert.ok(pipCloseBtn);
    pipCloseBtn.click();

    assert.equal(leftWatch, true);
    assert.equal(controller.getMode(), "hidden");
    assert.equal(container.ownerDocument.querySelector("#stream-viewer-pip"), null);

    controller.dispose();
  });
});

describe("TASK-616: Управление звуком и масштабированием стрима (Zoom & Audio)", () => {
  it("синхронизирует громкость и mute между stage viewer и PiP плеером", () => {
    const { container } = setupTestEnvironment();
    /** @type {any[]} */
    const audioEvents = [];
    const controller = createStreamViewerController(container, {
      onAudioChange: (state) => audioEvents.push(state),
    });

    controller.openViewer({ streamKey: "guild:1:2:3", userId: "3" });

    // Установка громкости 150%
    controller.setVolume(150);
    assert.equal(controller.getAudioState().volumePercent, 150);
    assert.equal(controller.getAudioState().muted, false);

    const volLabel = container.querySelector("#stream-viewer-volume-label");
    assert.equal(volLabel?.textContent, "150%");

    // Mute toggle
    controller.toggleMute();
    assert.equal(controller.getAudioState().muted, true);
    assert.equal(volLabel?.textContent, "ВЫКЛ");

    // Переход в PiP сохраняет mute состояние
    controller.setMode("pip");
    const pipMuteBtn = container.ownerDocument.querySelector("#stream-viewer-pip-mute");
    assert.ok(pipMuteBtn);
    assert.equal(pipMuteBtn.getAttribute("aria-pressed"), "true");

    // Unmute из PiP
    pipMuteBtn.click();
    assert.equal(controller.getAudioState().muted, false);
    assert.equal(controller.getAudioState().volumePercent, 150);

    controller.dispose();
  });

  it("зум-контролы корректно масштабируют холст в пределах 100%..500%", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);
    controller.openViewer({ streamKey: "guild:1:2:3", userId: "3" });

    assert.equal(controller.getZoomState().scale, 1.0);

    controller.zoomIn();
    assert.ok(controller.getZoomState().scale > 1.0);

    controller.setZoom(2.25);
    assert.equal(controller.getZoomState().scale, 2.25);

    controller.resetZoom();
    assert.equal(controller.getZoomState().scale, 1.0);
    assert.equal(controller.getZoomState().panX, 0);
    assert.equal(controller.getZoomState().panY, 0);

    controller.dispose();
  });

  it("полноэкранный режим открывает #stream-fullscreen и закрывается по кнопке или Escape", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);
    controller.openViewer({
      streamKey: "guild:1:2:3",
      userId: "3",
      displayName: "StreamHost",
    });

    const fsToggle = container.querySelector("#stream-fullscreen-toggle");
    assert.ok(fsToggle);
    fsToggle.click();

    assert.equal(controller.getMode(), "fullscreen");
    const fsDialog = container.ownerDocument.querySelector("#stream-fullscreen");
    assert.ok(fsDialog);
    assert.equal(fsDialog.getAttribute("role"), "dialog");
    assert.equal(fsDialog.getAttribute("aria-modal"), "true");
    assert.equal(fsDialog.hidden, false);

    const fsTitle = fsDialog.querySelector("#stream-fullscreen-title");
    assert.equal(fsTitle?.textContent, "StreamHost");

    const fsClose = fsDialog.querySelector("#stream-fullscreen-close");
    assert.ok(fsClose);
    fsClose.click();

    assert.equal(controller.getMode(), "stage");
    assert.equal(fsDialog.hidden, true);

    controller.dispose();
  });
});

describe("TASK-616: Клавиатурные сокращения и доступность WAI-ARIA", () => {
  it("клавиши P (PiP), F (Fullscreen), M (Mute), Escape управляют просмотром стрима", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);
    controller.openViewer({ streamKey: "guild:1:2:3", userId: "3" });

    // P -> переход в PiP
    controller.handleKeyDown({ key: "p" });
    assert.equal(controller.getMode(), "pip");

    // P -> возврат в stage
    controller.handleKeyDown({ key: "P" });
    assert.equal(controller.getMode(), "stage");

    // F -> переход в fullscreen
    controller.handleKeyDown({ key: "f" });
    assert.equal(controller.getMode(), "fullscreen");

    // Escape -> возврат из fullscreen
    controller.handleKeyDown({ key: "Escape" });
    assert.equal(controller.getMode(), "stage");

    // M -> Mute toggle
    assert.equal(controller.getAudioState().muted, false);
    controller.handleKeyDown({ key: "m" });
    assert.equal(controller.getAudioState().muted, true);
    controller.handleKeyDown({ key: "m" });
    assert.equal(controller.getAudioState().muted, false);

    controller.dispose();
  });
});

describe("TASK-616: Безопасность и CSS-контракт (T-STREAM-UI-OBS-01)", () => {
  it("скрывает конфиденциальные токены и секреты из DOM и текстовых атрибутов", () => {
    const { container } = setupTestEnvironment();
    const controller = createStreamViewerController(container);

    const sensitiveToken = "secret-rtc-token-xyz-12345";
    const sensitiveEndpoint = "rtc.discord.media:443";
    controller.openViewer({
      streamKey: "guild:111:222:333",
      userId: "333",
      token: sensitiveToken,
      endpoint: sensitiveEndpoint,
    });

    const htmlContent = container.innerHTML || "";
    assert.equal(htmlContent.includes(sensitiveToken), false, "Секретный токен утек в DOM");
    assert.equal(htmlContent.includes(sensitiveEndpoint), false, "Эндпоинт утек в DOM");

    controller.dispose();
  });

  it("CSS-файл styles/stream-viewer.css оформлен строго через tokens.css без сырых цветов и !important", () => {
    assert.ok(existsSync(STREAM_VIEWER_CSS_PATH), "Файл styles/stream-viewer.css должен существовать");
    const css = readFileSync(STREAM_VIEWER_CSS_PATH, "utf8");

    // Запрет !important
    assert.equal(css.includes("!important"), false, "CSS содержит !important");

    // Запрет селектора :has()
    assert.equal(/:has\(/i.test(css), false, "CSS содержит неподдерживаемый :has()");

    // Запрет сырых цветов (#hex, rgb, rgba, hsl, hsla)
    const rawColorMatch = css.match(/#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\(/);
    assert.equal(rawColorMatch, null, `CSS содержит сырой цвет: ${rawColorMatch?.[0]}`);

    // Проверка, что все var(--*) объявлены в tokens.css
    const tokensContent = readFileSync(TOKENS_CSS_PATH, "utf8");
    const declaredTokens = new Set(
      Array.from(tokensContent.matchAll(/--[a-zA-Z0-9_-]+/g)).map((m) => m[0]),
    );

    const usedTokens = Array.from(css.matchAll(/var\((--[a-zA-Z0-9_-]+)/g)).map((m) => m[1]);
    assert.ok(usedTokens.length > 0, "CSS должен использовать токены из tokens.css");

    const undeclared = usedTokens.filter((t) => t && !declaredTokens.has(t));
    assert.deepEqual(undeclared, [], `Найдены необъявленные токены в CSS: ${undeclared.join(", ")}`);
  });
});
