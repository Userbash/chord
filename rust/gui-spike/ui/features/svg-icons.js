// @ts-check
/**
 * SVG-иконки Discord UI (task-186): каноническая карта 20 имён, безопасное
 * создание `<svg><path/></svg>` и идемпотентная замена текстовых глифов.
 *
 * Иконки наследуют цвет через `fill="currentColor"` и скрыты от
 * скринридеров (`aria-hidden`), потому что подпись несёт соседний контрол.
 */

/** Канонический размер иконки, px. */
export const ICON_SIZE = 20;

/**
 * Карта имён иконок в данные пути SVG (viewBox 0 0 20 20).
 * @type {Readonly<Record<string, string>>}
 */
export const SVG_ICONS = Object.freeze({
  home: "M10 2.5 2.5 9h2v8h4v-5h3v5h4V9h2L10 2.5Z",
  hash: "M8 3h2l-.7 4h3.2l.7-4h2l-.7 4h2.5v2h-2.9l-.6 3h2.5v2h-2.9l-.7 4h-2l.7-4H6.9l-.7 4h-2l.7-4H2.5v-2h2.8l.6-3H3.4V7h2.9l.7-4Zm.1 6-.6 3h3.2l.6-3H8.1Z",
  voice: "M7.5 4.5a2.5 2.5 0 0 1 5 0v5a2.5 2.5 0 0 1-5 0v-5Zm-3 5.5h1.6v.4a3.9 3.9 0 0 0 7.8 0V10h1.6v.4a5.5 5.5 0 0 1-4.7 5.4V18h-1.6v-2.2a5.5 5.5 0 0 1-4.7-5.4V10Z",
  Announcement: "M3 8.5V12h2.5l5 3.5v-11L5.5 8.5H3Zm9.5-2.2v7.4a3 3 0 0 0 0-7.4Zm1.8-2.6a5.6 5.6 0 0 1 0 12.6l-1-1.4a4 4 0 0 0 0-9.8l1-1.4ZM6 13.4l2 1.1V17H6v-3.6Z",
  lock: "M10 2a3.5 3.5 0 0 0-3.5 3.5V8H6v9h8V8h-.5V5.5A3.5 3.5 0 0 0 10 2Zm2 6H8V5.5a2 2 0 0 1 4 0V8Zm-2 3.2a1.4 1.4 0 0 1 .7 2.6V15H9.3v-1.2A1.4 1.4 0 0 1 10 11.2Z",
  pin: "m12.8 2.5 4.7 4.7-2 2-1.1-.4-2.6 2.6.4 2.5-1.4 1.4-3.7-3.7-3.3 4.9-1.1-1.1 4.9-3.3-3.7-3.7 1.4-1.4 2.5.4L10.8 6l-.4-1.1 2.4-2.4Z",
  thread: "M4 3h12a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-3.4L10 16v-3H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm2.5 3v1.6h7V6h-7Zm0 3v1.6h4.5V9H6.5Z",
  bell: "M10 2a1 1 0 0 1 1 1v.6a5 5 0 0 1 4 4.9V12l1.5 2.5v1H3.5v-1L5 12V8.5a5 5 0 0 1 4-4.9V3a1 1 0 0 1 1-1Zm0 16a2 2 0 0 1-2-2h4a2 2 0 0 1-2 2Z",
  inbox: "M4 3h12a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Zm1 2v7h3l.8 1.5h2.4L12 12h3V5H5Z",
  search: "M9 2a7 7 0 1 0 4.2 12.6l3.1 3.1 1.4-1.4-3.1-3.1A7 7 0 0 0 9 2Zm0 2a5 5 0 1 1 0 10A5 5 0 0 1 9 4Z",
  members: "M7 3a3.5 3.5 0 1 1 0 7 3.5 3.5 0 0 1 0-7Zm7 1a3 3 0 1 1 0 6 3 3 0 0 1 0-6ZM1 17c0-3 2.7-5 6-5s6 2 6 5v1H1v-1Zm13-3.2c2.4.3 4 1.8 4 4.2v1h-4.5v-1c0-1.7-.6-3.2-1.6-4.2.7-.1 1.4 0 2.1 0Z",
  settings: "M10 4.5A5.5 5.5 0 1 1 10 15.5 5.5 5.5 0 0 1 10 4.5Zm0 3.2A2.3 2.3 0 1 0 10 12.3 2.3 2.3 0 0 0 10 7.7Z",
  mic: "M10 3a2.5 2.5 0 0 1 2.5 2.5v5a2.5 2.5 0 0 1-5 0v-5A2.5 2.5 0 0 1 10 3Zm-1 12.8V17h2v-1.2a5.5 5.5 0 0 0 4.5-5.4h-1.6a3.9 3.9 0 0 1-7.8 0H4.5A5.5 5.5 0 0 0 9 15.8Z",
  "mic-off": "M3.3 2 2 3.3l4 4V10a4 4 0 0 0 5.2 3.8l1 1A6 6 0 0 1 4.5 10H3a6 6 0 0 0 5 6.4V18h2v-1.6a6 6 0 0 0 2.6-1L16 18l1.4-1.4L3.3 2ZM10 3a2.5 2.5 0 0 1 2.5 2.5v4.3l-5-5A2.5 2.5 0 0 1 10 3Z",
  headset: "M10 2a8 8 0 0 0-8 8v5a2 2 0 0 0 2 2h1.5v-6H4v-1a6 6 0 0 1 12 0v1h-1.5v6H16a2 2 0 0 0 2-2v-5a8 8 0 0 0-8-8Zm-4 11H4.5v2H6v-2Zm8 0h1.5v2H14v-2Z",
  "headset-off": "M3.3 2 2 3.3 4 5.3A8 8 0 0 0 2 10v5a2 2 0 0 0 2 2h1.5v-6H4v-1c0-1 .2-1.9.6-2.7l1.5 1.5A6 6 0 0 1 10 4a6 6 0 0 1 6 6v1h-1.5v6H16a2 2 0 0 0 2-2v-5c0-1.5-.4-2.9-1.2-4.1L18 4.9 16.7 3.5 3.3 2ZM6 13H4.5v2H6v-2Zm8 0h1.5v2H14v-2Z",
  plus: "M9 3h2v6h6v2h-6v6H9v-6H3V9h6V3Z",
  smile: "M10 2a8 8 0 1 1 0 16 8 8 0 0 1 0-16Zm-3 5.5a1.2 1.2 0 1 0 0 2.4 1.2 1.2 0 0 0 0-2.4Zm6 0a1.2 1.2 0 1 0 0 2.4 1.2 1.2 0 0 0 0-2.4ZM5.6 11.5a4.7 4.7 0 0 0 8.8 0l-1.5-.6a3.2 3.2 0 0 1-5.8 0l-1.5.6Z",
  reply: "M9 4v3.2c4.4.3 7 2.7 7 7.3-1.6-2.1-3.8-3.1-7-3.3V14L4 9l5-5Z",
  more: "M5 8a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm5 0a2 2 0 1 1 0 4 2 2 0 0 1 0-4Zm5 0a2 2 0 1 1 0 4 2 2 0 0 1 0-4Z",
});

/**
 * @typedef {Object} IconOptions
 * @property {number} [size] Размер стороны, px.
 * @property {string} [className] CSS-класс корневого `<svg>`.
 */

/**
 * Создаёт `<svg><path d="…"/></svg>` для известной иконки.
 * @param {any} doc Документ, в котором создаются узлы.
 * @param {string} name Имя иконки из {@link SVG_ICONS}.
 * @param {IconOptions} [options]
 * @returns {any} `null`, если имя неизвестно или документ недоступен.
 */
export function createIconSvg(doc, name, options = {}) {
  if (!doc || typeof doc.createElement !== "function") return null;
  if (typeof name !== "string" || !Object.prototype.hasOwnProperty.call(SVG_ICONS, name)) {
    return null;
  }
  const spec = options ?? {};
  const size = typeof spec.size === "number" && Number.isFinite(spec.size) ? spec.size : ICON_SIZE;
  const className =
    typeof spec.className === "string" && spec.className.length > 0 ? spec.className : "ui-icon";

  const svg = doc.createElement("svg");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("viewBox", `0 0 ${ICON_SIZE} ${ICON_SIZE}`);
  svg.setAttribute("fill", "currentColor");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  svg.setAttribute("data-icon", name);
  svg.setAttribute("class", className);

  const path = doc.createElement("path");
  path.setAttribute("d", SVG_ICONS[name] ?? "");
  svg.appendChild(path);
  return /** @type {any} */ (svg);
}

/**
 * Заменяет текстовые глифы внутри `[data-icon]` на соответствующие `<svg>`.
 * Повторный вызов на том же дереве ничего не меняет (маркер
 * `data-icon-rendered="svg"`).
 * @param {any} root
 * @returns {number} Сколько узлов реально обновлено.
 */
export function upgradeGlyphIcons(root) {
  if (!root || typeof root.querySelectorAll !== "function") return 0;

  /** @type {Element[]} */
  const targets = [];
  if ("hasAttribute" in root && typeof root.hasAttribute === "function" && root.hasAttribute("data-icon")) {
    targets.push(/** @type {Element} */ (root));
  }
  for (const node of root.querySelectorAll("[data-icon]")) targets.push(node);

  let upgraded = 0;
  for (const host of targets) {
    // Готовый svg тоже несёт `data-icon`; его нельзя заворачивать повторно.
    const tag = typeof host.tagName === "string" ? host.tagName.toLowerCase() : "";
    if (tag === "svg") continue;
    if (host.getAttribute("data-icon-rendered") === "svg") continue;
    const name = host.getAttribute("data-icon");
    if (!name) continue;
    const doc = host.ownerDocument ?? ("createElement" in root && typeof root.createElement === "function" ? /** @type {Document} */ (root) : null);
    const svg = createIconSvg(doc, name);
    if (!svg) continue;

    if (typeof host.replaceChildren === "function") {
      host.replaceChildren(svg);
    } else {
      while (host.firstChild) host.removeChild(host.firstChild);
      host.appendChild(svg);
    }
    host.setAttribute("data-icon-rendered", "svg");
    upgraded += 1;
  }
  return upgraded;
}
