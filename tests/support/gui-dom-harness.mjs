// @ts-check
/**
 * Минимальный DOM-шим для реального импорта `rust/gui-spike/ui/app.js` (RW-38 / W38a).
 *
 * Зачем: до этой волны DOM-контракт спайка проверялся чтением исходников (regex по
 * `app.js`) и живым smoke через `UI_SNAPSHOT`. Шим позволяет прогнать настоящие
 * обработчики, виртуализацию и автозамер в обычном Node без npm-зависимостей,
 * браузера и дисплея.
 *
 * Осознанные ограничения (это не браузер и не jsdom):
 * - layout не считается: `clientHeight` равен `boxHeight` контейнера, заданному
 *   харнессом, и 0 для скрытого экрана (аналог `display:none` у `.screen` без
 *   `.active`); `scrollHeight` выводится из `top+height` детей. `getBoundingClientRect`,
 *   CSS-каскад, шрифты и реальные размеры окна не моделируются.
 * - поддерживаются только селекторы, которые использует `app.js`: `#id`, `.class`,
 *   `tag`, `tag.class`, `.a, .b` (группы), `.screen.active` (compound),
 *   `input[type="checkbox"]` (атрибут), `#member-list .member` (потомок через пробел).
 * - события не всплывают: `click()` вызывает только слушатели самого элемента;
 *   `scroll`/`input` эмитируются тестом через `element.emit(type)`.
 * - `setTimeout`/`setInterval` не тикают сами (только записываются) — иначе
 *   guard-таймеры `nextFrame()`/`benchScroll()` давали бы nondeterminism; тест
 *   управляет ими через `flushTimers()`.
 * - `requestAnimationFrame` — очередь, которую тест двигает `flushRaf()`
 *   (кадр = +16.7 мс монотонных часов).
 * - `window.__TAURI__.core.invoke` — заглушка, записывающая вызовы; неизвестная
 *   команда реджектится, а не молча возвращает undefined (контракт IPC виден).
 */

import { setImmediate as tickImmediate } from "node:timers/promises";
import { fileURLToPath, pathToFileURL } from "node:url";

/**
 * Запись вызова IPC из фронтенда.
 * @typedef {{ cmd: string, args: any }} InvokeCall
 */

/**
 * Событие шима: минимум, который читает `app.js`.
 * `key`/модификаторы опциональны: `app.js` проверяет их через `in`, а тесты
 * передают их в `dispatchEvent(type, init)`; `dispatchEvent` заполняет остальное.
 * @typedef {{
 *   type: string,
 *   key?: string,
 *   altKey?: boolean,
 *   ctrlKey?: boolean,
 *   metaKey?: boolean,
 *   shiftKey?: boolean,
 *   target: ShimElement | null,
 *   currentTarget: ShimElement | null,
 *   prevented: boolean,
 *   preventDefault: () => void,
 *   stopPropagation: () => void,
 *   stopImmediatePropagation: () => void,
 * }} ShimEvent
 */

/**
 * Разобранный compound-селектор (`tag#id.class[attr="value"]`).
 * @typedef {{ tag: string, id: string, classes: string[], attrs: Array<[string, string]> }} Compound
 */

/** Ширина кадра в монотонных часах шима (60 fps). */
const FRAME_MS = 16.7;

/**
 * @param {string} value
 * @returns {number}
 */
function px(value) {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

/** Инлайн-стиль: только свойства, которые пишет `app.js` (+ запас на ARIA-правки). */
export class StyleDecl {
  constructor() {
    this.position = "";
    this.left = "";
    this.right = "";
    this.top = "";
    this.bottom = "";
    this.width = "";
    this.height = "";
    this.display = "";
    this.color = "";
    this.marginLeft = "";
    this.overflowY = "";
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setProperty(name, value) {
    const bag = /** @type {Record<string, string>} */ (/** @type {unknown} */ (this));
    bag[name] = String(value);
  }

  /**
   * @param {string} name
   * @returns {string}
   */
  getPropertyValue(name) {
    const bag = /** @type {Record<string, string>} */ (/** @type {unknown} */ (this));
    return bag[name] ?? "";
  }

  /** @returns {string} */
  get cssText() {
    const bag = /** @type {Record<string, string>} */ (/** @type {unknown} */ (this));
    return Object.entries(bag)
      .filter(([, value]) => value !== "")
      .map(([name, value]) => `${name}: ${value}`)
      .join("; ");
  }
}

/** `classList` поверх `Set` владельца: add/remove/toggle/contains, как в DOM. */
export class ClassList {
  /** @param {ShimElement} owner */
  constructor(owner) {
    /** @type {ShimElement} */
    this.owner = owner;
  }

  /** @param {...string} names */
  add(...names) {
    for (const name of names) if (name) this.owner.classes.add(name);
  }

  /** @param {...string} names */
  remove(...names) {
    for (const name of names) this.owner.classes.delete(name);
  }

  /**
   * @param {string} name
   * @param {boolean} [force]
   * @returns {boolean}
   */
  toggle(name, force) {
    const next = force === undefined ? !this.owner.classes.has(name) : Boolean(force);
    if (next) this.owner.classes.add(name);
    else this.owner.classes.delete(name);
    return next;
  }

  /** @param {string} name @returns {boolean} */
  contains(name) {
    return this.owner.classes.has(name);
  }

  /** @param {number} index @returns {string | null} */
  item(index) {
    return [...this.owner.classes][index] ?? null;
  }

  /** @returns {number} */
  get length() {
    return this.owner.classes.size;
  }

  /** @returns {string} */
  get value() {
    return [...this.owner.classes].join(" ");
  }

  /** @returns {string} */
  toString() {
    return this.value;
  }
}

/** `data-*`: `app.js` пишет `state`, разметка — `screen`, рейл серверов — `guild`. */
export class Dataset {
  constructor() {
    this.state = "";
    this.screen = "";
    this.guild = "";
  }
}

/** Узел DOM с тем подмножеством API, которое нужно `app.js` и тестам. */
export class ShimElement {
  /**
   * @param {string} tagName
   * @param {ShimDocument} ownerDocument
   */
  constructor(tagName, ownerDocument) {
    /** @type {string} */
    this.tagName = tagName.toUpperCase();
    /** @type {ShimDocument} */
    this.ownerDocument = ownerDocument;
    /** @type {ShimElement[]} */
    this.children = [];
    /** @type {ShimElement | null} */
    this.parentNode = null;
    /** @type {Set<string>} */
    this.classes = new Set();
    /** @type {ClassList} */
    this.classList = new ClassList(this);
    /** @type {StyleDecl} */
    this.style = new StyleDecl();
    /** @type {Dataset} */
    this.dataset = new Dataset();
    /** @type {string} */
    this.id = "";
    /** @type {string} */
    this.title = "";
    /** @type {string} */
    this.type = "";
    /** @type {boolean} */
    this.checked = false;
    /** @type {string} */
    this.value = "";
    /** @type {string} */
    this.placeholder = "";
    /** Каретка поля ввода: в шиме нет layout, значения выставляет тест. @type {number | null} */
    this.selectionStart = null;
    /** @type {number | null} */
    this.selectionEnd = null;
    /** @type {number} */
    this.scrollTop = 0;
    /** «CSS-высота» контейнера; реального layout в шиме нет. @type {number} */
    this.boxHeight = 0;
    /** Текст элемента (текстовых узлов-объектов шим не создаёт). @type {string} */
    this.nodeText = "";
    /** @type {Map<string, Array<(event: ShimEvent) => void>>} */
    this.listeners = new Map();
    /** Прочие атрибуты (aria-*, style-хуки ARIA-правок W38c). @type {Map<string, string>} */
    this.attributes = new Map();
  }

  focus() {
    this.ownerDocument.activeElement = this;
  }

  /** @returns {string} */
  get className() {
    return [...this.classes].join(" ");
  }

  /** @param {string} value */
  set className(value) {
    this.classes = new Set(String(value).split(/\s+/).filter(Boolean));
  }

  /** @returns {string} */
  get textContent() {
    if (this.children.length === 0) return this.nodeText;
    let text = "";
    for (const child of this.children) text += child.textContent;
    return text;
  }

  /** @param {string} value */
  set textContent(value) {
    this.replaceChildren();
    this.nodeText = String(value);
  }

  /**
   * Каретка/выделение поля: в браузере метод есть у input/textarea, в шиме —
   * аддитивная заглушка, чтобы код вставки не ветвился по среде.
   * @param {number} start
   * @param {number} end
   */
  setSelectionRange(start, end) {
    this.selectionStart = start;
    this.selectionEnd = end;
  }

  /**
   * Видимость: скрытый экран (`.screen` без `.active`) имеет clientHeight=0,
   * как `display:none` в браузере — на этом держится P1-1 (см. VirtualList).
   * @returns {boolean}
   */
  get visible() {
    /** @type {ShimElement | null} */
    let node = this;
    while (node) {
      if (node.classes.has("screen") && !node.classes.has("active")) return false;
      if (node.style.display === "none") return false;
      node = node.parentNode;
    }
    return true;
  }

  /** @returns {number} */
  get clientHeight() {
    return this.visible ? this.boxHeight : 0;
  }

  /** Высота содержимого по `top+height` детей (sizer виртуального списка). */
  get scrollHeight() {
    let height = this.boxHeight;
    for (const child of this.children) {
      const bottom = px(child.style.top) + px(child.style.height);
      if (bottom > height) height = bottom;
    }
    return height;
  }

  /**
   * @param {ShimElement} child
   * @returns {ShimElement}
   */
  appendChild(child) {
    child.parentNode = this;
    this.children.push(child);
    return child;
  }

  /** @param {...ShimElement} nodes */
  append(...nodes) {
    for (const node of nodes) this.appendChild(node);
  }

  /** @param {...ShimElement} nodes */
  replaceChildren(...nodes) {
    for (const child of this.children) child.parentNode = null;
    this.children = [];
    this.nodeText = "";
    this.append(...nodes);
  }

  remove() {
    const parent = this.parentNode;
    if (!parent) return;
    const index = parent.children.indexOf(this);
    if (index >= 0) parent.children.splice(index, 1);
    this.parentNode = null;
  }

  /**
   * @param {string} type
   * @param {(event: ShimEvent) => void} listener
   * @param {any} [_options]
   */
  addEventListener(type, listener, _options) {
    let bucket = this.listeners.get(type);
    if (!bucket) {
      bucket = [];
      this.listeners.set(type, bucket);
    }
    bucket.push(listener);
  }

  /**
   * @param {string} type
   * @param {(event: ShimEvent) => void} listener
   */
  removeEventListener(type, listener) {
    const bucket = this.listeners.get(type);
    if (!bucket) return;
    const index = bucket.indexOf(listener);
    if (index >= 0) bucket.splice(index, 1);
  }

  /**
   * @param {string} type
   * @param {Partial<ShimEvent>} [init]
   * @returns {ShimEvent}
   */
  dispatchEvent(type, init = {}) {
    /** @type {ShimEvent} */
    const event = {
      type,
      target: this,
      currentTarget: this,
      prevented: false,
      preventDefault() {
        event.prevented = true;
      },
      stopPropagation() {},
      stopImmediatePropagation() { /** @type {any} */ (event).immediateStopped = true; },
      ...init,
    };
    // Слушатели читаются из копии: добавление/снятие в обработчике не ломает обход.
    for (const listener of [...(this.listeners.get(type) ?? [])]) {
      listener(event);
      if (/** @type {any} */ (event).immediateStopped) break;
    }
    return event;
  }

  /**
   * Эмиссия события тестом (scroll/input/change) без «мышиного» поведения.
   * @param {string} type
   * @returns {ShimEvent}
   */
  emit(type) {
    return this.dispatchEvent(type);
  }

  /**
   * `click()` как активация: у checkbox сначала переключается `checked`,
   * затем вызываются слушатели (порядок spec pre-click activation).
   */
  click() {
    if (this.tagName === "INPUT" && this.type === "checkbox") this.checked = !this.checked;
    this.dispatchEvent("click");
  }

  /**
   * @param {string} selector
   * @returns {ShimElement | null}
   */
  querySelector(selector) {
    return querySelectorAllFrom(this, selector, false)[0] ?? null;
  }

  /**
   * @param {string} selector
   * @returns {ShimElement[]}
   */
  querySelectorAll(selector) {
    return querySelectorAllFrom(this, selector, false);
  }

  /** @param {string} selector @returns {boolean} */
  matches(selector) {
    return parseSelector(selector).some((group) => matchesGroup(this, group));
  }

  /**
   * @param {string} name
   * @returns {string | null}
   */
  getAttribute(name) {
    switch (name) {
      case "id":
        return this.id || null;
      case "class":
        return this.className || null;
      case "type":
        return this.type || null;
      case "title":
        return this.title || null;
      case "placeholder":
        return this.placeholder || null;
      case "data-screen":
        return this.dataset.screen || null;
      case "data-state":
        return this.dataset.state || null;
      default:
        return this.attributes.get(name) ?? null;
    }
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setAttribute(name, value) {
    if (name === "id") this.id = value;
    else if (name === "class") this.className = value;
    else if (name === "type") this.type = value;
    else if (name === "title") this.title = value;
    else if (name === "placeholder") this.placeholder = value;
    else if (name === "data-screen") this.dataset.screen = value;
    else if (name === "data-state") this.dataset.state = value;
    else this.attributes.set(name, value);
  }

  /** @param {string} name @returns {boolean} */
  hasAttribute(name) {
    return this.getAttribute(name) !== null;
  }

  /** @param {string} name */
  removeAttribute(name) {
    if (name === "id") this.id = "";
    else if (name === "class") this.className = "";
    else if (name === "type") this.type = "";
    else if (name === "title") this.title = "";
    else if (name === "placeholder") this.placeholder = "";
    else if (name === "data-screen") this.dataset.screen = "";
    else if (name === "data-state") this.dataset.state = "";
    else this.attributes.delete(name);
  }

  /** @returns {ShimElement | null} */
  get firstChild() {
    return this.children[0] ?? null;
  }

  /** Следующий элемент-сосед: нужен тестам структуры (в браузере есть). @returns {ShimElement | null} */
  get nextElementSibling() {
    const siblings = this.parentNode?.children ?? [];
    const index = siblings.indexOf(this);
    return index < 0 ? null : siblings[index + 1] ?? null;
  }

  /** @returns {ShimElement | null} */
  get previousElementSibling() {
    const siblings = this.parentNode?.children ?? [];
    const index = siblings.indexOf(this);
    return index <= 0 ? null : siblings[index - 1] ?? null;
  }

  /** @returns {number} */
  get childElementCount() {
    return this.children.length;
  }
}

/** `<input>`: нужен для `instanceof HTMLInputElement` в `app.js`. */
export class HTMLInputElement extends ShimElement {}

/** `<button>`: нужен для `instanceof HTMLButtonElement` в `app.js`. */
export class HTMLButtonElement extends ShimElement {}

/** Документ: дерево `html > head + body`, поиск обходом (id-registry не нужен). */
export class ShimDocument {
  constructor() {
    /** @type {ShimElement} */
    this.documentElement = new ShimElement("html", this);
    /** @type {ShimElement} */
    this.head = new ShimElement("head", this);
    /** @type {ShimElement} */
    this.body = new ShimElement("body", this);
    this.documentElement.append(this.head, this.body);
    /** @type {string} */
    this.title = "";
    /** @type {ShimElement | null} */
    this.activeElement = null;
  }

  /**
   * @param {string} tagName
   * @returns {ShimElement}
   */
  createElement(tagName) {
    const name = String(tagName).toLowerCase();
    if (name === "input") return new HTMLInputElement(name, this);
    if (name === "button") return new HTMLButtonElement(name, this);
    return new ShimElement(name, this);
  }

  /**
   * Пространства имён (inline-SVG QR-панели): тип узла тот же ShimElement —
   * раскладка и события шиму для SVG не нужны, важны атрибуты/дерево.
   * @param {string} _namespace
   * @param {string} tagName
   * @returns {ShimElement}
   */
  createElementNS(_namespace, tagName) {
    const name = String(tagName).toLowerCase();
    return new ShimElement(name, this);
  }

  /**
   * @param {string} text
   * @returns {ShimElement}
   */
  createTextNode(text) {
    const node = new ShimElement("#text", this);
    node.nodeText = String(text);
    return node;
  }

  /**
   * @param {string} id
   * @returns {ShimElement | null}
   */
  getElementById(id) {
    /** @type {ShimElement[]} */
    const queue = [this.documentElement];
    while (queue.length > 0) {
      const node = /** @type {ShimElement} */ (queue.shift());
      if (node.id === id) return node;
      queue.push(...node.children);
    }
    return null;
  }

  /**
   * @param {string} selector
   * @returns {ShimElement | null}
   */
  querySelector(selector) {
    return querySelectorAllFrom(this.documentElement, selector, true)[0] ?? null;
  }

  /**
   * @param {string} selector
   * @returns {ShimElement[]}
   */
  querySelectorAll(selector) {
    return querySelectorAllFrom(this.documentElement, selector, true);
  }
}

/* ---------- мини-движок селекторов ---------- */

/**
 * @param {string} selector
 * @returns {Compound[][]}
 */
function parseSelector(selector) {
  return String(selector)
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((group) =>
      group
        .split(/\s+/)
        .filter(Boolean)
        .map(parseCompound),
    );
}

/**
 * @param {string} text
 * @returns {Compound}
 */
function parseCompound(text) {
  /** @type {Compound} */
  const compound = { tag: "", id: "", classes: [], attrs: [] };
  const tokens = text.match(/(#[^.#[\]]+|\.[^.#[\]]+|\[[^\]]*\]|[A-Za-z][\w-]*)/g) ?? [];
  for (const token of tokens) {
    if (token.startsWith("#")) {
      compound.id = token.slice(1);
    } else if (token.startsWith(".")) {
      compound.classes.push(token.slice(1));
    } else if (token.startsWith("[")) {
      const match = token.match(/^\[\s*([\w-]+)\s*(?:=\s*"?([^"\]]*)"?\s*)?\]$/);
      if (match) compound.attrs.push([match[1] ?? "", match[2] ?? ""]);
    } else {
      compound.tag = token.toLowerCase();
    }
  }
  return compound;
}

/**
 * @param {ShimElement} node
 * @param {Compound} compound
 * @returns {boolean}
 */
function matchesCompound(node, compound) {
  if (compound.tag && node.tagName.toLowerCase() !== compound.tag) return false;
  if (compound.id && node.id !== compound.id) return false;
  for (const name of compound.classes) if (!node.classes.has(name)) return false;
  for (const [name, value] of compound.attrs) {
    const actual = node.getAttribute(name);
    if (actual === null) return false;
    if (value !== "" && actual !== value) return false;
  }
  return true;
}

/**
 * Потомок через пробел: правый compound проверяется на узле, остальные — на предках
 * (ближайший подходящий предок), как в CSS descendant combinator.
 * @param {ShimElement} node
 * @param {Compound[]} group
 * @returns {boolean}
 */
function matchesGroup(node, group) {
  const own = group[group.length - 1];
  if (!own || !matchesCompound(node, own)) return false;
  /** @type {ShimElement | null} */
  let ancestor = node.parentNode;
  for (let index = group.length - 2; index >= 0; index--) {
    const compound = /** @type {Compound} */ (group[index]);
    /** @type {ShimElement | null} */
    let found = null;
    while (ancestor) {
      if (matchesCompound(ancestor, compound)) {
        found = ancestor;
        break;
      }
      ancestor = ancestor.parentNode;
    }
    if (!found) return false;
    ancestor = found.parentNode;
  }
  return true;
}

/**
 * @param {ShimElement} root
 * @param {string} selector
 * @param {boolean} includeRoot
 * @returns {ShimElement[]}
 */
function querySelectorAllFrom(root, selector, includeRoot) {
  const groups = parseSelector(selector);
  if (groups.length === 0) return [];
  /** @type {ShimElement[]} */
  const candidates = [];
  if (includeRoot) candidates.push(root);
  /** @param {ShimElement} node */
  const walk = (node) => {
    for (const child of node.children) {
      candidates.push(child);
      walk(child);
    }
  };
  walk(root);
  // Порядок — документный, независимо от порядка групп в селекторе.
  return candidates.filter((node) => groups.some((group) => matchesGroup(node, group)));
}

/* ---------- глобалы, таймеры, rAF, ResizeObserver, IPC ---------- */

/** @type {GuiDom | null} */
let activeEnv = null;

/** `ResizeObserver`: хранит цели и будит колбэк по `triggerResize()`. */
class ResizeObserverShim {
  /** @param {(entries: any[], observer: ResizeObserverShim) => void} callback */
  constructor(callback) {
    /** @type {(entries: any[], observer: ResizeObserverShim) => void} */
    this.callback = callback;
    /** @type {Set<ShimElement>} */
    this.targets = new Set();
    if (activeEnv) activeEnv.observers.add(this);
  }

  /** @param {ShimElement} target */
  observe(target) {
    this.targets.add(target);
  }

  /** @param {ShimElement} target */
  unobserve(target) {
    this.targets.delete(target);
  }

  disconnect() {
    this.targets.clear();
  }
}

/**
 * Построение DOM, соответствующего `ui/index.html`. Отдельный тест сверяет его с
 * самим html-файлом, чтобы разметка шима не расходилась со сдаваемой.
 * @param {ShimDocument} doc
 * @returns {ShimElement}
 */
function buildDom(doc) {
  /** @param {string} tag @param {string} className @returns {ShimElement} */
  const make = (tag, className) => {
    const node = doc.createElement(tag);
    node.className = className;
    return node;
  };
  /** @param {string} tag @param {string} className @param {string} value @returns {ShimElement} */
  const text = (tag, className, value) => {
    const node = make(tag, className);
    node.textContent = value;
    return node;
  };
  /**
   * Иконочная кнопка; `action` — зеркало `data-action` из `ui/index.html` (§8.1),
   * чтобы действия были доступны и в шиме (аддитивно, разметка не менялась).
   * @param {string} tag @param {string} className @param {string} label @param {string} [action]
   * @returns {ShimElement}
   */
  const iconBtn = (tag, className, label, action = "") => {
    const node = make(tag, className);
    node.setAttribute("aria-label", label);
    if (action) node.setAttribute("data-action", action);
    return node;
  };

  /* 1. Строка заголовка: .topbar (вне shell). */
  const topbar = make("header", "topbar");
  topbar.setAttribute("role", "banner");
  const title = text("div", "screen-title", "Чат · 100 000 сообщений");
  const metrics = make("div", "metrics");
  metrics.setAttribute("aria-live", "polite");
  /** @param {string} label @param {string} id */
  const metric = (label, id) => {
    const span = doc.createElement("span");
    span.textContent = label;
    const value = doc.createElement("b");
    value.id = id;
    value.textContent = "—";
    span.append(value);
    return span;
  };
  metrics.append(
    metric("FPS ", "m-fps"),
    metric("p95 кадра ", "m-p95"),
    metric("RAM ", "m-rss"),
    metric("потоки ", "m-threads"),
  );
  const bench = doc.createElement("button");
  bench.id = "bench";
  bench.className = "bench";
  bench.textContent = "Замер 5 с";
  metrics.append(bench);
  topbar.append(title, metrics);

  /* 2. Рейл: бренд DL, гильдии, разделитель, 4 кнопки экранов. */
  const rail = doc.createElement("aside");
  rail.id = "guild-rail";
  rail.className = "rail";
  rail.setAttribute("role", "navigation");
  rail.setAttribute("aria-label", "Серверы и экраны");
  const brand = text("div", "brand", "DL");
  brand.title = "discord-lab";
  brand.setAttribute("aria-hidden", "true");
  rail.append(brand);
  /** @type {Array<[string, string, string]>} */
  const guilds = [
    ["1", "Сервер ITSKIDON PC BOOST", "IS"],
    ["2", "Сервер 2", "S2"],
    ["3", "Сервер 3", "S3"],
  ];
  for (const [guild, label, mark] of guilds) {
    const button = doc.createElement("button");
    button.className = `guild-item${guild === "1" ? " is-selected" : ""}`;
    button.dataset.guild = guild;
    button.setAttribute("aria-label", label);
    button.setAttribute("aria-current", guild === "1" ? "true" : "false");
    button.textContent = mark;
    rail.append(button);
  }
  rail.append(make("div", "rail-sep"));

  /** @type {Array<[string, string, string]>} */
  const railButtons = [
    ["chat", "💬", "Чат 100k"],
    ["emoji", "😀", "Эмодзи-пикер"],
    ["members", "👥", "Участники 24k"],
    ["settings", "⚙️", "Настройки"],
  ];
  for (const [screen, glyph, title] of railButtons) {
    const button = doc.createElement("button");
    button.className = `rail-btn${screen === "chat" ? " active" : ""}`;
    button.dataset.screen = screen;
    button.title = title;
    button.textContent = glyph;
    if (screen === "chat") button.setAttribute("aria-current", "page");
    rail.append(button);
  }

  /* 3. Дерево каналов + строка пользователя. */
  const tree = doc.createElement("aside");
  tree.id = "channel-tree";
  tree.className = "tree";
  tree.setAttribute("role", "navigation");
  tree.setAttribute("aria-label", "Каналы сервера");
  const treeHeader = make("header", "tree-header");
  treeHeader.append(text("h2", "tree-guild", "ITSKIDON PC BOOST"), iconBtn("button", "icon-btn", "Меню сервера", "guild-menu"));
  const treeScroll = doc.createElement("nav");
  treeScroll.className = "tree-scroll";
  /**
   * @param {string} groupTitle
   * @param {Array<[string, string, boolean]>} channels
   * @returns {ShimElement}
   */
  const channelGroup = (groupTitle, channels) => {
    const section = make("section", "channel-group");
    const heading = make("h3", "channel-group-title");
    heading.append(text("button", "group-toggle", groupTitle));
    const list = doc.createElement("ul");
    list.className = "channel-list";
    list.setAttribute("role", "list");
    for (const [icon, name, unread] of channels) {
      const item = doc.createElement("li");
      const button = doc.createElement("button");
      button.className = `channel-item${unread ? " is-unread" : ""}`;
      const iconNode = text("span", "channel-icon", icon);
      iconNode.setAttribute("aria-hidden", "true");
      button.append(iconNode, text("span", "channel-name", name));
      item.append(button);
      list.append(item);
    }
    section.append(heading, list);
    return section;
  };
  treeScroll.append(
    channelGroup("НОВОСТИ", [
      ["#", "новости", false],
      ["#", "объявления", true],
    ]),
    channelGroup("ГОЛОСОВЫЕ КАНАЛЫ", [["🔊", "Пекарня", false]]),
  );
  // V-08: host голосовой панели — содержимое собирает features/voice.js,
  // как #auth-gate у features/auth.js (шим не разбирает рантайм-разметку).
  const voicePanel = doc.createElement("section");
  voicePanel.id = "voice-panel";
  voicePanel.className = "voice-panel";
  voicePanel.setAttribute("aria-label", "Голосовая связь");
  voicePanel.setAttribute("data-state", "disabled");
  voicePanel.setAttribute("aria-disabled", "true");
  voicePanel.setAttribute("data-self-mute", "false");
  voicePanel.setAttribute("data-self-deaf", "false");
  voicePanel.setAttribute("data-ptt", "idle");
  const userBar = doc.createElement("footer");
  userBar.id = "user-bar";
  userBar.className = "user-bar";
  const profile = make("button", "user-profile");
  profile.setAttribute("aria-label", "Профиль: ShadowByte, невидимый");
  profile.setAttribute("data-action", "profile");
  const avatarSm = text("span", "avatar avatar--sm", "SB");
  avatarSm.setAttribute("aria-hidden", "true");
  const userMeta = make("span", "user-meta");
  userMeta.append(text("span", "user-name", "ShadowByte"), text("span", "user-status", "Невидимый"));
  profile.append(avatarSm, userMeta);
  const userActions = make("div", "user-actions");
  userActions.append(
    iconBtn("button", "icon-btn", "Выключить микрофон", "mute"),
    iconBtn("button", "icon-btn", "Выключить звук", "deafen"),
    iconBtn("button", "icon-btn", "Настройки пользователя", "settings"),
  );
  userBar.append(profile, userActions);
  tree.append(treeHeader, treeScroll, voicePanel, userBar);

  /* 4. Контент: шапка чата — sibling экранов, не внутри #screen-chat. */
  const content = doc.createElement("main");
  content.className = "content";
  const chatHeader = doc.createElement("header");
  chatHeader.id = "chat-header";
  chatHeader.className = "chat-header";
  const chatTitle = make("h1", "chat-title");
  const hash = text("span", "hash", "#");
  hash.setAttribute("aria-hidden", "true");
  chatTitle.append(hash, text("span", "chat-title-text", "новости"));
  const chatActions = make("div", "chat-actions");
  const bellButton = iconBtn("button", "icon-btn", "Уведомления", "inbox");
  bellButton.title = "Уведомления (Phase 3)";
  bellButton.setAttribute("aria-disabled", "true");
  chatActions.append(
    iconBtn("button", "icon-btn", "Ветки", "threads"),
    bellButton,
    iconBtn("button", "icon-btn", "Закреплённые сообщения", "pins"),
    iconBtn("button", "icon-btn", "Участники", "members"),
  );
  const chatSearchField = doc.createElement("search");
  chatSearchField.className = "search-field";
  chatSearchField.setAttribute("role", "search");
  const chatSearch = doc.createElement("input");
  chatSearch.id = "chat-search";
  chatSearch.className = "search-input";
  chatSearch.type = "search";
  chatSearchField.append(chatSearch);
  chatActions.append(chatSearchField);
  chatHeader.append(iconBtn("button", "icon-btn", "Свернуть каналы", "toggle-tree"), chatTitle, chatActions);

  /* 5. Экран chat. */
  const chatScreen = make("section", "screen active");
  chatScreen.id = "screen-chat";
  chatScreen.setAttribute("role", "region");
  chatScreen.setAttribute("aria-label", "Чат");
  chatScreen.setAttribute("aria-hidden", "false");
  const chatMain = make("div", "chat-main");
  const chatList = make("div", "list chat-list");
  chatList.id = "chat-list";
  chatList.setAttribute("tabindex", "0");
  chatList.setAttribute("role", "list");
  chatList.setAttribute("aria-label", "Сообщения");
  chatList.boxHeight = 800;
  const chatNotice = text("p", "chat-notice", "Отправка недоступна в спайке (без сети)");
  chatNotice.setAttribute("role", "note");
  const composer = make("div", "composer");
  composer.id = "composer";
  composer.setAttribute("role", "group");
  composer.setAttribute("aria-label", "Поле ввода сообщения");
  const composerInput = doc.createElement("textarea");
  composerInput.id = "composer-input";
  composerInput.className = "composer-input";
  const attachButton = text("button", "icon-btn composer-attach", "＋");
  attachButton.setAttribute("data-action", "attach");
  composer.append(
    attachButton,
    composerInput,
    iconBtn("button", "icon-btn", "Эмодзи", "emoji"),
    iconBtn("button", "icon-btn", "Подарок", "gift"),
  );
  const hint = text("p", "visually-hidden", "Отправка сообщений в спайке отключена");
  hint.id = "composer-hint";
  chatMain.append(chatList, chatNotice, composer, hint);

  const memberPanel = doc.createElement("aside");
  memberPanel.id = "member-panel";
  memberPanel.className = "member-panel";
  memberPanel.setAttribute("role", "complementary");
  memberPanel.setAttribute("aria-label", "Участники сервера");
  const panelTitle = make("h2", "member-panel-title");
  panelTitle.textContent = "Участники — ";
  const online = doc.createElement("span");
  online.id = "member-online";
  online.textContent = "735";
  panelTitle.append(online);
  const panelList = make("div", "member-panel-list");
  panelList.id = "member-panel-list";
  panelList.setAttribute("role", "list");
  const allMembers = text("button", "member-panel-all", "Показать всех");
  allMembers.setAttribute("data-action", "all-members");
  memberPanel.append(panelTitle, panelList, allMembers);
  chatScreen.append(chatMain, memberPanel);

  /* 6. Экран emoji: карточка 498×440. */
  const emojiScreen = make("section", "screen");
  emojiScreen.id = "screen-emoji";
  const emojiWrap = make("div", "emoji-wrap");
  const emojiCard = make("div", "emoji-card");
  emojiCard.setAttribute("role", "dialog");
  const emojiHead = make("div", "emoji-head");
  const emojiSearchField = doc.createElement("search");
  emojiSearchField.className = "search-field";
  emojiSearchField.setAttribute("role", "search");
  const emojiSearch = doc.createElement("input");
  emojiSearch.id = "emoji-search";
  emojiSearch.className = "search-input";
  emojiSearch.type = "search";
  emojiSearchField.append(emojiSearch);
  emojiHead.append(emojiSearchField, iconBtn("button", "icon-btn", "Закрыть", "emoji-close"));
  const emojiBody = make("div", "emoji-body");
  const emojiTabs = make("div", "emoji-tabs");
  emojiTabs.id = "emoji-tabs";
  emojiTabs.setAttribute("role", "tablist");
  const emojiGrid = make("div", "emoji-grid");
  emojiGrid.id = "emoji-grid";
  emojiGrid.setAttribute("role", "listbox");
  emojiGrid.setAttribute("aria-label", "Эмодзи");
  emojiGrid.boxHeight = 600;
  emojiBody.append(emojiTabs, emojiGrid);
  const emojiPreview = make("div", "emoji-preview");
  const previewGlyph = text("span", "emoji-preview-glyph", "🙂");
  previewGlyph.setAttribute("aria-hidden", "true");
  emojiPreview.append(previewGlyph, text("span", "emoji-preview-name", "Выберите эмодзи"));
  emojiCard.append(emojiHead, emojiBody, emojiPreview);
  emojiWrap.append(emojiCard);
  emojiScreen.append(emojiWrap);

  /* 7. Экран members: тулбар + виртуализованный список. */
  const membersScreen = make("section", "screen");
  membersScreen.id = "screen-members";
  const toolbar = make("div", "members-toolbar");
  const memberSearchField = doc.createElement("search");
  memberSearchField.className = "search-field";
  memberSearchField.setAttribute("role", "search");
  const memberSearch = doc.createElement("input");
  memberSearch.id = "member-search";
  memberSearch.className = "search-input";
  memberSearch.type = "search";
  memberSearchField.append(memberSearch);
  const memberCount = text("span", "members-count", "24 229");
  memberCount.id = "member-count";
  toolbar.append(memberSearchField, memberCount);
  const memberList = make("div", "list member-list");
  memberList.id = "member-list";
  memberList.setAttribute("tabindex", "0");
  memberList.setAttribute("role", "list");
  memberList.setAttribute("aria-label", "Участники");
  memberList.boxHeight = 800;
  membersScreen.append(toolbar, memberList);

  /* 8. Экран settings: оверлей с навигацией и контентом. */
  const settingsScreen = make("section", "screen");
  settingsScreen.id = "screen-settings";
  const settingsShell = make("div", "settings-shell");
  settingsShell.setAttribute("role", "dialog");
  settingsShell.setAttribute("aria-modal", "true");
  const settingsHead = make("header", "settings-head");
  const avatarMd = text("span", "avatar avatar--md", "SB");
  avatarMd.setAttribute("aria-hidden", "true");
  const settingsUser = make("span", "settings-user");
  settingsUser.append(
    text("span", "settings-user-name", "ShadowByte"),
    text("span", "settings-user-hint", "Редактировать профиль"),
  );
  settingsHead.append(avatarMd, settingsUser, iconBtn("button", "icon-btn", "Закрыть настройки", "settings-close"));
  const settingsBody = make("div", "settings-body");
  const settingsNav = doc.createElement("nav");
  settingsNav.id = "settings-nav";
  settingsNav.className = "settings-nav";
  const settings = make("div", "settings");
  settings.id = "settings";
  settings.setAttribute("role", "group");
  settings.setAttribute("aria-label", "Настройки");
  settings.boxHeight = 800;
  settingsBody.append(settingsNav, settings);
  settingsShell.append(settingsHead, settingsBody);
  settingsScreen.append(settingsShell);

  content.append(chatHeader, chatScreen, emojiScreen, membersScreen, settingsScreen);

  const quickSwitcher = make("div", "quick-switcher");
  quickSwitcher.id = "quick-switcher";
  quickSwitcher.setAttribute("role", "dialog");
  quickSwitcher.setAttribute("aria-modal", "true");
  quickSwitcher.setAttribute("aria-label", "Быстрый переход");
  quickSwitcher.setAttribute("hidden", "");
  const quickPanel = make("div", "quick-switcher-panel");
  const quickInput = doc.createElement("input");
  quickInput.id = "quick-switcher-input";
  quickInput.className = "quick-switcher-input";
  quickInput.setAttribute("role", "combobox");
  quickInput.setAttribute("aria-expanded", "false");
  quickInput.setAttribute("aria-autocomplete", "list");
  quickInput.setAttribute("aria-controls", "quick-switcher-results");
  quickInput.setAttribute("aria-label", "Быстрый переход");
  quickInput.placeholder = "Куда отправимся?";
  const quickResults = make("div", "quick-switcher-results");
  quickResults.id = "quick-switcher-results";
  quickResults.setAttribute("role", "listbox");
  quickResults.setAttribute("aria-label", "Результаты");
  const quickEmpty = text("p", "quick-switcher-empty", "Ничего не найдено");
  quickEmpty.setAttribute("hidden", "");
  const quickHint = text("p", "quick-switcher-hint", "↑ ↓ выбрать · Enter перейти · Esc закрыть");
  quickPanel.append(quickInput, quickResults, quickEmpty, quickHint);
  quickSwitcher.append(quickPanel);

  const workspace = make("div", "workspace");
  workspace.append(tree, content);
  const shell = make("div", "shell");
  shell.append(rail, workspace);
  doc.body.append(topbar, shell, quickSwitcher);
  return doc.body;
}

/**
 * Окружение одного импорта `app.js`: свой документ, свои таймеры и rAF.
 * Тесты работают с ним через `loadApp()`.
 */
export class GuiDom {
  /** @param {{ appPath: string, invokeHandler?: (cmd: string, args: any) => any }} options */
  constructor(options) {
    /** @type {string} */
    this.appPath = options.appPath;
    /** @type {((cmd: string, args: any) => any) | null} */
    this.invokeHandler = options.invokeHandler ?? null;
    /** @type {ShimDocument} */
    this.document = new ShimDocument();
    /** @type {InvokeCall[]} */
    this.invokeCalls = [];
    /** @type {string[]} */
    this.consoleErrors = [];
    /** @type {string[]} */
    this.consoleWarns = [];
    /** @type {Set<ResizeObserverShim>} */
    this.observers = new Set();
    /** @type {Map<number, (time: number) => void>} */
    this.rafQueue = new Map();
    this.nextRafId = 1;
    /** Монотонные «часы»: двигаются только кадрами rAF. */
    this.clock = 0;
    /** @type {Map<number, { kind: "timeout" | "interval", run: () => void, delay: number }>} */
    this.timers = new Map();
    this.nextTimerId = 1;
    /** @type {any} */
    this.appModule = null;
    this.disposed = false;
    /** @type {Console["error"]} */
    this.realError = console.error;
    /** @type {Console["warn"]} */
    this.realWarn = console.warn;
    /** @type {{ document: ShimDocument, __TAURI__: any, addEventListener: (...args: any[]) => void, removeEventListener: (...args: any[]) => void, requestAnimationFrame: (cb: (time: number) => void) => number }} */
    this.window = {
      document: this.document,
      __TAURI__: {
        core: {
          /** @param {string} cmd @param {any} [args] @returns {Promise<any>} */
          invoke: (cmd, args) => this.invoke(cmd, args),
        },
      },
      addEventListener() {},
      removeEventListener() {},
      requestAnimationFrame: (callback) => this.requestAnimationFrame(callback),
    };
    buildDom(this.document);
    this.patchConsole();
  }

  /**
   * Совместимость с тестами, которые создают узлы через env (не env.document).
   * @param {string} tagName
   * @returns {ShimElement}
   */
  createElement(tagName) {
    return this.document.createElement(tagName);
  }

  /* ---------- IPC ---------- */

  /**
   * Заглушка `invoke`: пишет вызов и отдаёт валидные данные для команд спайка.
   * @param {string} cmd
   * @param {any} [args]
   * @returns {Promise<any>}
   */
  async invoke(cmd, args) {
    this.invokeCalls.push({ cmd, args: args ?? null });
    if (this.invokeHandler) {
      const handled = await this.invokeHandler(cmd, args);
      if (handled !== undefined) return handled;
    }
    switch (cmd) {
      case "spike_config":
        // autobench выключен: тесты не должны ждать 5-секундный замер.
        return { autobench: false };
      case "process_metrics":
        return { rss_kib: 184_320, pss_kib: 131_072, threads: 7, cpu_pct: 3.5 };
      case "report_ui_snapshot":
      case "report_bench":
      case "window_ready":
      case "settings_save":
        return null;
      case "settings_load":
        // RW-40/B8b: дефолты `UiSettings::default` (settings_store.rs).
        return { version: 1, toggles: {}, density: "cozy", theme: "dark", locale: "ru" };
      case "net_status":
        // RW-40/L-01: после auth ready UI опрашивает статус; штатно — ready,
        // без баннера. Сценарии offline/reconnect задают свой invokeHandler.
        return "ready";
      default:
        throw new Error(`gui-dom-harness: неизвестная IPC-команда «${cmd}»`);
    }
  }

  /** @param {(cmd: string, args: any) => any} handler */
  setInvokeHandler(handler) { this.invokeHandler = handler; }

  /** @param {string} cmd @returns {InvokeCall[]} */
  callsOf(cmd) {
    return this.invokeCalls.filter((call) => call.cmd === cmd);
  }

  /**
   * Последний `report_ui_snapshot` с разобранным payload.
   * @returns {{ call: InvokeCall | null, payload: any | null }}
   */
  get snapshot() {
    const calls = this.callsOf("report_ui_snapshot");
    const call = calls[calls.length - 1] ?? null;
    if (!call) return { call: null, payload: null };
    const payload = call.args?.payload;
    if (typeof payload !== "string") return { call, payload: null };
    return { call, payload: JSON.parse(payload) };
  }

  /* ---------- глобалы ---------- */

  installGlobals() {
    const globals = /** @type {Record<string, any>} */ (/** @type {unknown} */ (globalThis));
    globals.document = this.document;
    globals.window = this.window;
    globals.Element = ShimElement;
    globals.HTMLElement = ShimElement;
    globals.HTMLInputElement = HTMLInputElement;
    globals.HTMLButtonElement = HTMLButtonElement;
    globals.ResizeObserver = ResizeObserverShim;
    globals.requestAnimationFrame = this.requestAnimationFrame.bind(this);
    globals.cancelAnimationFrame = this.cancelRaf.bind(this);
    globals.performance = {
      now: () => this.clock,
      timeOrigin: 0,
      mark() {},
      measure() {},
    };
    globals.setTimeout = this.setTimeout.bind(this);
    globals.clearTimeout = this.clearTimer.bind(this);
    globals.setInterval = this.setInterval.bind(this);
    globals.clearInterval = this.clearTimer.bind(this);
  }

  patchConsole() {
    console.error = (...args) => {
      this.consoleErrors.push(args.map((arg) => String(arg)).join(" "));
      this.realError(...args);
    };
    console.warn = (...args) => {
      this.consoleWarns.push(args.map((arg) => String(arg)).join(" "));
      this.realWarn(...args);
    };
  }

  /* ---------- таймеры и кадры ---------- */

  /**
   * @param {(...args: any[]) => void} callback
   * @param {number} [delay]
   * @param {...any} args
   * @returns {number}
   */
  setTimeout(callback, delay = 0, ...args) {
    const id = this.nextTimerId++;
    this.timers.set(id, { kind: "timeout", run: () => callback(...args), delay });
    return id;
  }

  /**
   * @param {(...args: any[]) => void} callback
   * @param {number} [delay]
   * @param {...any} args
   * @returns {number}
   */
  setInterval(callback, delay = 0, ...args) {
    const id = this.nextTimerId++;
    this.timers.set(id, { kind: "interval", run: () => callback(...args), delay });
    return id;
  }

  /** @param {number} id */
  clearTimer(id) {
    this.timers.delete(id);
  }

  /** Однократно выполняет все записанные таймеры (интервалы остаются в очереди). */
  flushTimers() {
    const entries = [...this.timers.entries()].sort((left, right) => left[0] - right[0]);
    for (const [id, timer] of entries) {
      if (timer.kind === "timeout") this.timers.delete(id);
      timer.run();
    }
  }

  /**
   * @param {(time: number) => void} callback
   * @returns {number}
   */
  requestAnimationFrame(callback) {
    const id = this.nextRafId++;
    this.rafQueue.set(id, callback);
    return id;
  }

  /** @param {number} id */
  cancelRaf(id) {
    this.rafQueue.delete(Number(id));
  }

  /**
   * Один кадр: выполняются только колбэки, стоявшие в очереди на входе; новые
   * (в т.ч. перезапрос `frameLoop`) уходят в следующий кадр.
   * @returns {number} число выполненных колбэков
   */
  flushRaf() {
    const ids = [...this.rafQueue.keys()].sort((left, right) => left - right);
    for (const id of ids) {
      const callback = this.rafQueue.get(id);
      this.rafQueue.delete(id);
      if (!callback) continue;
      this.clock += FRAME_MS;
      callback(this.clock);
    }
    return ids.length;
  }

  /**
   * Прогон кадров с дренажом микрозадач: промисы `PageStore.ensureRange()`
   * успевают разрешиться и запросить следующий кадр.
   * @param {number} [rounds]
   * @returns {Promise<GuiDom>}
   */
  async settle(rounds = 12) {
    for (let index = 0; index < rounds; index++) {
      this.flushRaf();
      await tickImmediate();
    }
    return this;
  }

  /**
   * Будит `ResizeObserver`: колбэк вызывается только для наблюдаемых целей
   * (как в браузере, где `observe()` обязателен).
   */
  triggerResize() {
    for (const observer of this.observers) {
      for (const target of observer.targets) {
        observer.callback([{ target, contentRect: { width: 0, height: target.clientHeight } }], observer);
      }
    }
  }

  /** Снимает патч консоли, чистит очереди и отвязывает окружение от глобалов. */
  dispose() {
    this.disposed = true;
    this.rafQueue.clear();
    this.timers.clear();
    this.observers.clear();
    console.error = this.realError;
    console.warn = this.realWarn;
    if (activeEnv === this) activeEnv = null;
  }
}

let importCounter = 0;

/**
 * Импортирует настоящий `ui/app.js` в шим-документ и доводит его до устойчивого
 * состояния (кадры rAF + микрозадачи), включая автовызов `report_ui_snapshot`.
 *
 * `GUI_DOM_APP` позволяет прогнать тесты против копии app.js (red-first мутации без
 * правки сдаваемого файла); копия должна лежать рядом со своим `model.js`.
 * @param {{ appPath?: string, settleRounds?: number, invokeHandler?: (cmd: string, args: any) => any }} [options]
 * @returns {Promise<GuiDom>}
 */
export async function loadApp(options = {}) {
  const appPath =
    options.appPath ??
    process.env.GUI_DOM_APP ??
    fileURLToPath(new URL("../../rust/gui-spike/ui/app.js", import.meta.url));
  if (activeEnv) activeEnv.dispose();
  const env = new GuiDom({ appPath, invokeHandler: options.invokeHandler });
  activeEnv = env;
  env.installGlobals();
  // Query-строка — новый экземпляр модуля: состояние app.js не утекает между тестами.
  const moduleUrl = `${pathToFileURL(appPath).href}?test=${++importCounter}`;
  env.appModule = await import(moduleUrl);
  await env.settle(options.settleRounds ?? 30);
  return env;
}
