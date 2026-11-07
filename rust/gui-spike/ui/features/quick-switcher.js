// @ts-check
/**
 * T-226 (FE-P1-22 / RW-40 / G2): контроллер модального окна быстрого перехода
 * Quick Switcher (`Ctrl+K`) с префиксной фильтрацией (`@`, `#`, `!`, `*`) и
 * нечёткой (fuzzy subsequence) сортировкой каналов, ЛС и серверов.
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` (§1 `mod+k` — модуль `675704` QuickSwitcher);
 * - `/home/sanya/re-deep/ui/03-quickswitcher.html`, `03-quickswitcher-input.html`
 *   (`quickswitcher_ac6cb0`, `input_ac6cb0`, `placeholder="Куда отправимся?"`,
 *   `autocompleteQuerySymbol_ac6cb0`: `@` пользователи, `#` текстовые каналы,
 *   `!` голосовые каналы, `*` серверы).
 *
 * @module ui/features/quick-switcher
 */
import { make } from "./dom.js";
import { TITLES } from "./screens.js";

/** @typedef {ReturnType<typeof import("./screens.js").createScreenRouter>} ScreenRouter */
/** @typedef {{ kind: "channel" | "guild" | "screen", label: string, detail: string, target: string }} Result */

/**
 * @typedef {"user" | "text" | "voice" | "guild"} QuickSwitcherItemType
 */

/**
 * @typedef {object} QuickSwitcherItem
 * @property {string} id
 * @property {QuickSwitcherItemType} type
 * @property {string} name
 * @property {string} [guildName]
 * @property {number} [unreadCount]
 * @property {number} [mentionCount]
 */

/**
 * @typedef {object} QuickSwitcherControllerOptions
 * @property {any} [container]
 * @property {ReadonlyArray<QuickSwitcherItem>} [items]
 * @property {(item: QuickSwitcherItem) => void} [onSelectTarget]
 * @property {() => void} [onClose]
 */

/**
 * Префиксы фильтрации по типу цели в Quick Switcher (`@` — пользователи/ЛС,
 * `#` — текстовые каналы, `!` — голосовые каналы, `*` — серверы/гильдии).
 * @type {Readonly<{ USER: "@", TEXT_CHANNEL: "#", VOICE_CHANNEL: "!", GUILD: "*" }>}
 */
export const QUICK_SWITCHER_PREFIXES = Object.freeze({
  USER: "@",
  TEXT_CHANNEL: "#",
  VOICE_CHANNEL: "!",
  GUILD: "*",
});

/**
 * Максимальное число результатов в выдаче Quick Switcher.
 * @type {10}
 */
export const MAX_QUICK_SWITCHER_RESULTS = 10;

/**
 * Плейсхолдер поля ввода Quick Switcher из эталонного дампа Discord.
 */
export const QUICK_SWITCHER_PLACEHOLDER_RU = "Куда отправимся?";

/**
 * Соответствие префиксного символа типу элемента Quick Switcher.
 * @type {Readonly<Record<string, QuickSwitcherItemType>>}
 */
const PREFIX_TO_ITEM_TYPE = Object.freeze({
  [QUICK_SWITCHER_PREFIXES.USER]: "user",
  [QUICK_SWITCHER_PREFIXES.TEXT_CHANNEL]: "text",
  [QUICK_SWITCHER_PREFIXES.VOICE_CHANNEL]: "voice",
  [QUICK_SWITCHER_PREFIXES.GUILD]: "guild",
});

/**
 * Соответствие типа элемента отображаемому символу-префиксу.
 * @type {Readonly<Record<QuickSwitcherItemType, string>>}
 */
const ITEM_TYPE_TO_PREFIX = Object.freeze({
  user: QUICK_SWITCHER_PREFIXES.USER,
  text: QUICK_SWITCHER_PREFIXES.TEXT_CHANNEL,
  voice: QUICK_SWITCHER_PREFIXES.VOICE_CHANNEL,
  guild: QUICK_SWITCHER_PREFIXES.GUILD,
});

/**
 * Проверяет регистронезависимое совпадение по подстроке или нечёткой
 * подпоследовательности символов (`fuzzy subsequence`) в `name`.
 *
 * @param {string} name
 * @param {string} queryAfterPrefix
 * @returns {boolean}
 */
function matchesNameSubstringOrFuzzy(name, queryAfterPrefix) {
  if (!queryAfterPrefix) return true;
  const haystack = String(name ?? "").toLowerCase();
  if (!haystack) return false;
  const needle = queryAfterPrefix.toLowerCase();
  if (haystack.includes(needle)) return true;

  const compactNeedle = needle.replace(/\s+/g, "");
  if (!compactNeedle) return true;

  let cursor = 0;
  for (const ch of compactNeedle) {
    const foundIndex = haystack.indexOf(ch, cursor);
    if (foundIndex < 0) return false;
    cursor = foundIndex + 1;
  }
  return true;
}

/**
 * Фильтрует и ранжирует элементы Quick Switcher по строке запроса `rawQuery`:
 * - если запрос начинается с `@`, `#`, `!` или `*`, оставляет только элементы
 *   соответствующего типа (`"user"`, `"text"`, `"voice"`, `"guild"`) и отбрасывает префикс;
 * - фильтрует по регистронезависимой подстроке / нечёткой подпоследовательности
 *   (`fuzzy subsequence`) в `item.name` (или возвращает все элементы подходящего типа,
 *   если строка после префикса пуста);
 * - ранжирует: точное совпадение префикса `name` -> `mentionCount > 0` ->
 *   `unreadCount > 0` -> по алфавиту `name`;
 * - обрезает выдачу до `MAX_QUICK_SWITCHER_RESULTS` (`10`) без мутации входного массива.
 *
 * @template {QuickSwitcherItem} T
 * @param {ReadonlyArray<T>} items
 * @param {string} [rawQuery]
 * @returns {T[]}
 */
export function scoreAndFilterQuickSwitcherItems(items, rawQuery = "") {
  if (!Array.isArray(items)) return [];

  const trimmedQuery = typeof rawQuery === "string" ? rawQuery.trim() : "";
  const firstChar = trimmedQuery.charAt(0);
  const requiredType =
    firstChar && Object.prototype.hasOwnProperty.call(PREFIX_TO_ITEM_TYPE, firstChar)
      ? PREFIX_TO_ITEM_TYPE[firstChar]
      : null;
  const queryAfterPrefix = requiredType ? trimmedQuery.slice(1).trim() : trimmedQuery;
  const needleLower = queryAfterPrefix.toLowerCase();

  const matched = items.filter((item) => {
    if (!item || typeof item !== "object") return false;
    if (requiredType && item.type !== requiredType) return false;
    return matchesNameSubstringOrFuzzy(String(item.name ?? ""), queryAfterPrefix);
  });

  return matched
    .sort((a, b) => {
      const aName = String(a.name ?? "");
      const bName = String(b.name ?? "");
      const aNameLower = aName.toLowerCase();
      const bNameLower = bName.toLowerCase();

      if (needleLower.length > 0) {
        const aExactPrefix = aNameLower.startsWith(needleLower) ? 1 : 0;
        const bExactPrefix = bNameLower.startsWith(needleLower) ? 1 : 0;
        if (aExactPrefix !== bExactPrefix) return bExactPrefix - aExactPrefix;
      }

      const aHasMentions =
        typeof a.mentionCount === "number" && Number.isFinite(a.mentionCount) && a.mentionCount > 0
          ? 1
          : 0;
      const bHasMentions =
        typeof b.mentionCount === "number" && Number.isFinite(b.mentionCount) && b.mentionCount > 0
          ? 1
          : 0;
      if (aHasMentions !== bHasMentions) return bHasMentions - aHasMentions;

      const aHasUnread =
        typeof a.unreadCount === "number" && Number.isFinite(a.unreadCount) && a.unreadCount > 0
          ? 1
          : 0;
      const bHasUnread =
        typeof b.unreadCount === "number" && Number.isFinite(b.unreadCount) && b.unreadCount > 0
          ? 1
          : 0;
      if (aHasUnread !== bHasUnread) return bHasUnread - aHasUnread;

      const byName = aName.localeCompare(bName, "ru", { sensitivity: "base" });
      if (byName !== 0) return byName;
      return String(a.id ?? "").localeCompare(String(b.id ?? ""));
    })
    .slice(0, MAX_QUICK_SWITCHER_RESULTS);
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
  if (
    containerEl &&
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("Container element must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы родительского элемента.
 *
 * @param {any} parentEl
 * @param {any[]} nodes
 */
function replaceElementChildren(parentEl, nodes) {
  if (typeof parentEl.replaceChildren === "function") {
    parentEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(parentEl.children)) {
    parentEl.children.length = 0;
  }
  for (const node of nodes) {
    parentEl.appendChild(node);
  }
}

/**
 * Форматирует число непрочитанных/упоминаний для бейджа (`99+` при `> 99`).
 *
 * @param {number} count
 * @returns {string}
 */
function formatBadgeCount(count) {
  return count > 99 ? "99+" : String(Math.floor(count));
}

/**
 * Создаёт изолированный контроллер модального окна Quick Switcher (`Ctrl+K`).
 *
 * Поддерживает вызов как `createQuickSwitcherController({ container, onSelectTarget })`,
 * так и `createQuickSwitcherController(container, { onSelectTarget })`.
 *
 * @param {QuickSwitcherControllerOptions} configOrContainer
 * @param {QuickSwitcherControllerOptions} [maybeOptions]
 */
export function createQuickSwitcherController(configOrContainer, maybeOptions = {}) {
  const rawFirst = /** @type {any} */ (configOrContainer);
  const isFirstArgContainer =
    rawFirst &&
    typeof rawFirst === "object" &&
    !("container" in rawFirst) &&
    (typeof rawFirst.appendChild === "function" ||
      typeof rawFirst.createElement === "function");

  /** @type {QuickSwitcherControllerOptions} */
  const options = isFirstArgContainer
    ? { ...maybeOptions, container: rawFirst }
    : (configOrContainer ?? {});

  const container = options.container;
  if (!container || typeof container.appendChild !== "function") {
    throw new TypeError("options.container must be a valid DOM container element");
  }
  const doc = resolveDocument(container);

  /** @type {QuickSwitcherItem[]} */
  let allItems = Array.isArray(options.items) ? [...options.items] : [];
  /** @type {QuickSwitcherItem[]} */
  let visibleItems = [];
  /** @type {string} */
  let currentQuery = "";
  /** @type {number} */
  let activeIndex = 0;
  /** @type {boolean} */
  let isOpen = false;

  const dialog = doc.createElement("div");
  dialog.setAttribute("data-quick-switcher", "");
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-label", "Быстрый переход");
  dialog.setAttribute("data-open", "false");
  dialog.setAttribute("hidden", "");

  const input = doc.createElement("input");
  input.setAttribute("type", "text");
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.setAttribute("aria-label", "Быстрый переход");
  input.setAttribute("data-quick-switcher-input", "");
  input.setAttribute("placeholder", QUICK_SWITCHER_PLACEHOLDER_RU);
  input.placeholder = QUICK_SWITCHER_PLACEHOLDER_RU;
  input.value = "";

  const resultsListbox = doc.createElement("div");
  resultsListbox.setAttribute("data-quick-switcher-results", "");
  resultsListbox.setAttribute("role", "listbox");
  resultsListbox.setAttribute("aria-label", "Результаты");

  dialog.appendChild(input);
  dialog.appendChild(resultsListbox);
  replaceElementChildren(container, [dialog]);

  function syncSelectionAttributes() {
    const optionNodes = resultsListbox.querySelectorAll
      ? resultsListbox.querySelectorAll("[data-quick-switcher-option]")
      : (resultsListbox.children ?? []);
    optionNodes.forEach((/** @type {any} */ optNode, /** @type {number} */ idx) => {
      const selected = idx === activeIndex;
      optNode.setAttribute("aria-selected", selected ? "true" : "false");
    });
    if (visibleItems.length > 0 && activeIndex >= 0 && activeIndex < visibleItems.length) {
      input.setAttribute("aria-activedescendant", `quick-switcher-opt-${activeIndex}`);
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  function renderResults() {
    visibleItems = scoreAndFilterQuickSwitcherItems(allItems, currentQuery);
    if (visibleItems.length === 0) {
      activeIndex = 0;
    } else if (activeIndex >= visibleItems.length || activeIndex < 0) {
      activeIndex = 0;
    }

    /** @type {any[]} */
    const optionNodes = [];
    visibleItems.forEach((item, index) => {
      const opt = doc.createElement("button");
      opt.setAttribute("type", "button");
      opt.setAttribute("role", "option");
      opt.setAttribute("id", `quick-switcher-opt-${index}`);
      opt.setAttribute("data-quick-switcher-option", "");
      opt.setAttribute("data-item-id", String(item.id ?? ""));
      opt.setAttribute("data-item-type", String(item.type ?? ""));
      opt.setAttribute("aria-selected", index === activeIndex ? "true" : "false");

      const prefixSpan = doc.createElement("span");
      prefixSpan.setAttribute("data-quick-switcher-prefix", "");
      prefixSpan.textContent = ITEM_TYPE_TO_PREFIX[item.type] ?? "";
      opt.appendChild(prefixSpan);

      const nameSpan = doc.createElement("span");
      nameSpan.setAttribute("data-quick-switcher-name", "");
      nameSpan.textContent = String(item.name ?? "");
      opt.appendChild(nameSpan);

      if (typeof item.guildName === "string" && item.guildName.length > 0) {
        const guildSpan = doc.createElement("span");
        guildSpan.setAttribute("data-quick-switcher-guild", "");
        guildSpan.textContent = item.guildName;
        opt.appendChild(guildSpan);
      }

      const mentionCount =
        typeof item.mentionCount === "number" && Number.isFinite(item.mentionCount) && item.mentionCount > 0
          ? item.mentionCount
          : 0;
      const unreadCount =
        typeof item.unreadCount === "number" && Number.isFinite(item.unreadCount) && item.unreadCount > 0
          ? item.unreadCount
          : 0;

      if (mentionCount > 0 || unreadCount > 0) {
        const badge = doc.createElement("span");
        badge.setAttribute("data-quick-switcher-badge", "");
        if (mentionCount > 0) {
          badge.setAttribute("data-badge-kind", "mention");
          badge.setAttribute("data-mention-count", String(mentionCount));
          badge.textContent = formatBadgeCount(mentionCount);
        } else {
          badge.setAttribute("data-badge-kind", "unread");
          badge.setAttribute("data-unread-count", String(unreadCount));
          badge.textContent = formatBadgeCount(unreadCount);
        }
        opt.appendChild(badge);
      }

      opt.addEventListener("click", () => {
        activeIndex = index;
        confirmSelection();
      });

      optionNodes.push(opt);
    });

    replaceElementChildren(resultsListbox, optionNodes);
    syncSelectionAttributes();
  }

  /**
   * Открывает модальное окно Quick Switcher с переданным списком элементов.
   *
   * @param {ReadonlyArray<QuickSwitcherItem>} [items]
   */
  function open(items) {
    if (Array.isArray(items)) {
      allItems = [...items];
    }
    isOpen = true;
    currentQuery = "";
    activeIndex = 0;
    input.value = "";
    dialog.removeAttribute("hidden");
    dialog.setAttribute("data-open", "true");
    input.setAttribute("aria-expanded", "true");
    renderResults();
    if (typeof input.focus === "function") {
      input.focus();
    }
  }

  /**
   * Устанавливает строку поискового запроса, сбрасывает выделение и перерисовывает список.
   *
   * @param {string} q
   * @returns {QuickSwitcherItem[]}
   */
  function setQuery(q) {
    currentQuery = typeof q === "string" ? q : String(q ?? "");
    if (input.value !== currentQuery) {
      input.value = currentQuery;
    }
    activeIndex = 0;
    renderResults();
    return [...visibleItems];
  }

  /**
   * Циклически сдвигает выделенный элемент на `delta` (с переходом через края списка).
   *
   * @param {number} delta
   * @returns {number}
   */
  function moveSelection(delta) {
    const count = visibleItems.length;
    if (count === 0) {
      activeIndex = 0;
      return 0;
    }
    const step = typeof delta === "number" && Number.isFinite(delta) ? Math.trunc(delta) : 0;
    activeIndex = (((activeIndex + step) % count) + count) % count;
    syncSelectionAttributes();
    return activeIndex;
  }

  /**
   * Закрывает модальное окно Quick Switcher.
   */
  function close() {
    isOpen = false;
    dialog.setAttribute("hidden", "");
    dialog.setAttribute("data-open", "false");
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
    if (typeof options.onClose === "function") {
      options.onClose();
    }
  }

  /**
   * Подтверждает выбор текущего активного элемента, вызывает `onSelectTarget(item)`
   * и закрывает модальное окно.
   *
   * @returns {QuickSwitcherItem | null}
   */
  function confirmSelection() {
    const selectedItem = visibleItems[activeIndex] ?? null;
    if (!selectedItem) return null;
    if (typeof options.onSelectTarget === "function") {
      options.onSelectTarget(selectedItem);
    }
    close();
    return selectedItem;
  }

  input.addEventListener("input", (/** @type {any} */ event) => {
    const nextValue = event?.target?.value ?? input.value ?? "";
    setQuery(String(nextValue));
  });

  dialog.addEventListener("keydown", (/** @type {any} */ event) => {
    const key = typeof event?.key === "string" ? event.key : "";
    if (key === "ArrowDown") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      moveSelection(1);
    } else if (key === "ArrowUp") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      moveSelection(-1);
    } else if (key === "Enter") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      confirmSelection();
    } else if (key === "Escape") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      close();
    }
  });

  return {
    element: dialog,
    open,
    setQuery,
    moveSelection,
    confirmSelection,
    close,
    /** @returns {boolean} */
    isOpen() {
      return isOpen;
    },
    /** @returns {number} */
    getActiveIndex() {
      return activeIndex;
    },
    /** @returns {string} */
    getQuery() {
      return currentQuery;
    },
    /** @returns {QuickSwitcherItem[]} */
    getResults() {
      return [...visibleItems];
    },
  };
}

/* ---------- Shell-интеграция QuickSwitcher для `app.js` ---------- */

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

/* ---------- C-07 (RW-40): плитка гильдии адресуется `data-guild-id` ---------- */

/**
 * id гильдии из плитки рейла. Контракт G-01b — `data-guild-id` (снежинка из
 * READY); legacy `data-guild` удалён вместе со статичной заглушкой и не читается.
 * `dataset.guildId` — то же значение для DOM-шима (`setAttribute` там не
 * выводит `data-*` в `dataset`).
 * @param {HTMLElement} node
 * @returns {string}
 */
function guildIdOf(node) {
  return node.getAttribute("data-guild-id") ?? node.dataset.guildId ?? "";
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
      items.push({ kind: "guild", label: node.getAttribute("aria-label") ?? node.textContent?.trim() ?? "", detail: "Сервер", target: guildIdOf(node) });
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
        const selected = guildIdOf(node) === item.target;
        node.classList.toggle("is-selected", selected);
        if (selected) node.setAttribute("aria-current", "true");
        else node.removeAttribute("aria-current");
      }
      const guild = /** @type {HTMLElement | undefined} */ (
        /** @type {HTMLElement[]} */ ([...document.querySelectorAll(".guild-item")]).find((node) => guildIdOf(node) === item.target)
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
