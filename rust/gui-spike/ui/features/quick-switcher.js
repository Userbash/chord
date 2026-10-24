// @ts-check
import { make } from "./dom.js";
import { TITLES } from "./screens.js";

/** @typedef {ReturnType<typeof import("./screens.js").createScreenRouter>} ScreenRouter */
/** @typedef {{ kind: "channel" | "guild" | "screen", label: string, detail: string, target: string }} Result */

/** @param {string} value */
function normalize(value) {
  return value.normalize("NFKD").replace(/[\u0300-\u036f]/g, "").toLocaleLowerCase("ru");
}

/** Fuzzy subsequence score: consecutive and earlier matches rank first. @param {string} query @param {string} value @returns {number | null} */
function score(query, value) {
  const needle = normalize(query).replace(/^[#!*@]+/, "").replace(/\s+/g, "");
  if (!needle) return 0;
  const haystack = normalize(value);
  let cursor = 0;
  let result = 0;
  let previous = -2;
  for (const char of needle) {
    const found = haystack.indexOf(char, cursor);
    if (found < 0) return null;
    result += found === previous + 1 ? 0 : found;
    previous = found;
    cursor = found + 1;
  }
  return result;
}

/**
 * @param {{ router: ScreenRouter }} options
 */
export function createQuickSwitcher({ router }) {
  const dialog = /** @type {HTMLElement} */ (document.querySelector("#quick-switcher"));
  const input = /** @type {HTMLInputElement} */ (document.querySelector("#quick-switcher-input"));
  const results = /** @type {HTMLElement} */ (document.querySelector("#quick-switcher-results"));
  if (!dialog || !input || !results) throw new Error("QuickSwitcher: отсутствует host/input/listbox");
  let empty = /** @type {HTMLElement | null} */ (document.querySelector(".quick-switcher-empty"));
  if (!empty) {
    empty = document.createElement("p");
    empty.className = "quick-switcher-empty";
    empty.textContent = "Ничего не найдено";
    empty.setAttribute("hidden", "");
    results.parentNode?.appendChild(empty);
  }
  const emptyNode = /** @type {HTMLElement} */ (empty);
  const closeButton = /** @type {HTMLButtonElement} */ (document.createElement("button"));
  closeButton.className = "quick-switcher-close";
  closeButton.type = "button";
  closeButton.textContent = "×";
  closeButton.setAttribute("aria-label", "Закрыть быстрый переход");
  dialog.querySelector(".quick-switcher-panel")?.appendChild(closeButton);

  /** @type {Result[]} */
  let visible = [];
  let activeIndex = 0;
  let open = false;
  const allResults = () => {
    /** @type {Result[]} */
    const items = [];
    for (const node of /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".channel-item")])) {
      const name = /** @type {HTMLElement | null} */ (node.querySelector(".channel-name"))?.textContent?.trim() ?? "";
      if (name) items.push({ kind: "channel", label: `# ${name}`, detail: document.querySelector(".tree-guild")?.textContent?.trim() ?? "", target: name });
    }
    for (const node of /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".guild-item")])) {
      items.push({ kind: "guild", label: node.getAttribute("aria-label") ?? node.textContent?.trim() ?? "", detail: "Сервер", target: node.dataset.guild ?? "" });
    }
    for (const node of /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".rail-btn")])) {
      const target = node.dataset.screen ?? "";
      const label = TITLES[/** @type {keyof typeof TITLES} */ (target)] ?? target;
      items.push({ kind: "screen", label, detail: "Экран", target });
    }
    return items;
  };

  function render() {
    const query = input.value.trim();
    const ranked = allResults()
      .map((item, index) => ({ item, index, score: score(query, `${item.label} ${item.detail}`) }))
      .filter((entry) => entry.score !== null)
      .sort((a, b) => /** @type {number} */ (a.score) - /** @type {number} */ (b.score) || a.index - b.index)
      .slice(0, 8);
    visible = ranked.map((entry) => entry.item);
    activeIndex = Math.min(activeIndex, Math.max(0, visible.length - 1));
    results.replaceChildren();
    visible.forEach((item, index) => {
      const option = /** @type {HTMLButtonElement} */ (make("button", "quick-switcher-option"));
      option.id = `quick-switcher-option-${index}`;
      option.type = "button";
      option.setAttribute("role", "option");
      option.setAttribute("aria-selected", index === activeIndex ? "true" : "false");
      option.setAttribute("data-kind", item.kind);
      option.appendChild(make("span", "quick-switcher-label", item.label));
      const detail = make("span", "quick-switcher-detail", item.detail);
      option.appendChild(detail);
      option.addEventListener("click", () => choose(index));
      results.appendChild(option);
    });
    // Пустое состояние совпадает с выдачей: hidden только при нуле опций.
    if (visible.length === 0) emptyNode.removeAttribute("hidden");
    else emptyNode.setAttribute("hidden", "");
    if (visible.length) input.setAttribute("aria-activedescendant", `quick-switcher-option-${activeIndex}`);
    else input.removeAttribute("aria-activedescendant");
  }

  function show() {
    open = true;
    dialog.removeAttribute("hidden");
    input.setAttribute("aria-expanded", "true");
    input.value = "";
    activeIndex = 0;
    render();
    input.focus();
  }

  /** @param {boolean} [restore] */
  function close(restore = true) {
    if (!open) return;
    open = false;
    dialog.setAttribute("hidden", "");
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
    if (restore) /** @type {HTMLElement | null} */ (document.querySelector('.rail-btn[aria-current="page"]'))?.focus();
  }

  /** @param {number} index */
  function choose(index) {
    const item = visible[index];
    if (!item) return;
    if (item.kind === "screen") {
      if (["chat", "emoji", "members", "settings"].includes(item.target)) router.switchScreen(/** @type {"chat" | "emoji" | "members" | "settings"} */ (item.target));
    } else if (item.kind === "channel") {
      for (const node of /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".channel-item")])) {
        const selected = /** @type {HTMLElement | null} */ (node.querySelector(".channel-name"))?.textContent?.trim() === item.target;
        if (selected) node.setAttribute("aria-current", "page");
        else node.removeAttribute("aria-current");
      }
      const title = /** @type {HTMLElement | null} */ (document.querySelector(".chat-title-text"));
      if (title) title.textContent = item.target;
      router.switchScreen("chat");
    } else {
      for (const node of /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".guild-item")])) {
        const selected = node.dataset.guild === item.target;
        node.classList.toggle("is-selected", selected);
        if (selected) node.setAttribute("aria-current", "true");
        else node.removeAttribute("aria-current");
      }
      const guild = /** @type {HTMLElement | undefined} */ (
        /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".guild-item")]).find((node) => node.dataset.guild === item.target)
      );
      const name = guild?.getAttribute("aria-label")?.replace(/^Сервер\s*/, "") ?? item.label;
      const heading = /** @type {HTMLElement | null} */ (document.querySelector(".tree-guild"));
      if (heading) heading.textContent = name;
    }
    close();
  }

  input.addEventListener("input", () => { activeIndex = 0; render(); });
  closeButton.addEventListener("click", () => close());
  document.body.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    const modifier = Boolean(("ctrlKey" in event && event.ctrlKey) || ("metaKey" in event && event.metaKey));
    if (modifier && key.toLowerCase() === "k") {
      event.preventDefault();
      if (open) close(false);
      else show();
      return;
    }
    if (!open) return;
    if (key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); close(); return; }
    if (key === "Tab") {
      event.preventDefault();
      const backwards = Boolean("shiftKey" in event && event.shiftKey);
      const active = /** @type {any} */ (document).activeElement;
      if (active === input) closeButton.focus();
      else input.focus();
      if (backwards && active === closeButton) input.focus();
      return;
    }
    if (key === "ArrowDown" || key === "ArrowUp") {
      event.preventDefault();
      if (visible.length) activeIndex = (activeIndex + (key === "ArrowDown" ? 1 : -1) + visible.length) % visible.length;
      render();
    } else if (key === "Enter") {
      event.preventDefault();
      choose(activeIndex);
    }
  });

  return { show, close, get open() { return open; } };
}
