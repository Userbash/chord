// @ts-check
/**
 * T-217 (FE-P1-19 / RW-40 / G2): контроллер вкладочного пикера выражений
 * (Эмодзи / GIF / Стикеры) (`expression-picker.js`).
 *
 * RE-оракул:
 * - `/home/sanya/re-deep/frontend-d03.md` §2.1–§2.2 (`expressionPickerPositionLayer__74017`,
 *   3 вкладки `[role="tab"]`, поиск, селектор тона кожи Fitzpatrick, сетка элементов);
 * - `/home/sanya/re-deep/ui8/21-emoji-picker.json`, `21d-emoji-categories.json`.
 *
 * @module ui/features/expression-picker
 */

/**
 * Русские названия вкладок пикера выражений.
 * @type {Readonly<{ emoji: string, gif: string, stickers: string }>}
 */
export const EXPRESSION_TABS_RU = Object.freeze({
  emoji: "Эмодзи",
  gif: "GIF",
  stickers: "Стикеры",
});

/**
 * Модификаторы тона кожи по шкале Фицпатрика (индекс 0 — нейтральный без модификатора, 1..5 — тона 🏻..🏿).
 * @type {ReadonlyArray<string>}
 */
export const SKIN_TONE_MODIFIERS = Object.freeze([
  "",
  "\u{1F3FB}",
  "\u{1F3FC}",
  "\u{1F3FD}",
  "\u{1F3FE}",
  "\u{1F3FF}",
]);

/**
 * Максимальное количество недавно использованных эмодзи.
 * @type {18}
 */
export const MAX_RECENT_EMOJIS = 18;

/**
 * Порядок вкладок в шапке пикера выражений.
 * @type {ReadonlyArray<ExpressionPickerTabId>}
 */
const TAB_ORDER = Object.freeze(
  /** @type {const} */ (["emoji", "gif", "stickers"]),
);

/**
 * Плейсхолдер поля поиска эмодзи по умолчанию.
 */
export const EMOJI_SEARCH_PLACEHOLDER_RU = "Найти идеальный эмодзи...";

/**
 * Встроенный стартовый набор эмодзи по умолчанию (используется, если не передан `options.emojis`).
 * @type {ReadonlyArray<Readonly<EmojiPickerItem>>}
 */
export const DEFAULT_PICKER_EMOJIS = Object.freeze([
  Object.freeze({ id: "wave", name: "wave", glyph: "👋", emoji: "👋", tags: ["привет", "hello", "рука"] }),
  Object.freeze({ id: "fire", name: "fire", glyph: "🔥", emoji: "🔥", tags: ["огонь", "hot", "пламя"] }),
  Object.freeze({ id: "thumbsup", name: "thumbsup", glyph: "👍", emoji: "👍", tags: ["лайк", "класс", "ok"] }),
  Object.freeze({ id: "heart", name: "heart", glyph: "❤️", emoji: "❤️", tags: ["сердце", "любовь", "love"] }),
  Object.freeze({ id: "rocket", name: "rocket", glyph: "🚀", emoji: "🚀", tags: ["ракета", "старт", "космос"] }),
  Object.freeze({ id: "tada", name: "tada", glyph: "🎉", emoji: "🎉", tags: ["праздник", "ура", "party"] }),
]);

/**
 * Встроенный стартовый набор стикеров по умолчанию.
 * @type {ReadonlyArray<Readonly<StickerPickerItem>>}
 */
export const DEFAULT_PICKER_STICKERS = Object.freeze([
  Object.freeze({ id: "sticker-wumpus-wave", name: "Wumpus Wave", tags: ["привет", "вампус", "wave"] }),
  Object.freeze({ id: "sticker-clyde-love", name: "Clyde Heart", tags: ["любовь", "сердце", "клайд"] }),
]);

/**
 * Встроенный стартовый набор GIF по умолчанию.
 * @type {ReadonlyArray<Readonly<GifPickerItem>>}
 */
export const DEFAULT_PICKER_GIFS = Object.freeze([
  Object.freeze({ id: "gif-cat-vibing", name: "Cat Vibing", tags: ["кот", "музыка", "танец"] }),
  Object.freeze({ id: "gif-thumbs-up", name: "Thumbs Up", tags: ["класс", "одобрение", "лайк"] }),
]);

/**
 * @typedef {"emoji" | "gif" | "stickers"} ExpressionPickerTabId
 */

/**
 * @typedef {object} EmojiPickerItem
 * @property {string} [id]
 * @property {string} name
 * @property {string} [glyph]
 * @property {string} [emoji]
 * @property {ReadonlyArray<string>} [tags]
 * @property {boolean} [supportsSkinTone]
 */

/**
 * @typedef {object} StickerPickerItem
 * @property {string} [id]
 * @property {string} name
 * @property {ReadonlyArray<string>} [tags]
 * @property {string} [url]
 */

/**
 * @typedef {object} GifPickerItem
 * @property {string} [id]
 * @property {string} [name]
 * @property {string} [title]
 * @property {ReadonlyArray<string>} [tags]
 * @property {string} [url]
 */

/**
 * @typedef {object} ExpressionPickerControllerOptions
 * @property {any} [container]
 * @property {ReadonlyArray<EmojiPickerItem>} [emojis]
 * @property {ReadonlyArray<StickerPickerItem>} [stickers]
 * @property {ReadonlyArray<GifPickerItem>} [gifs]
 * @property {ReadonlyArray<EmojiPickerItem>} [recentEmojis]
 * @property {ExpressionPickerTabId} [activeTab]
 * @property {number} [skinTone]
 * @property {(item: any, event?: any) => void} [onSelectEmoji]
 * @property {(item: any, event?: any) => void} [onSelectSticker]
 * @property {(item: any, event?: any) => void} [onSelectGif]
 * @property {() => void} [onClose]
 */

/**
 * Применяет модификатор тона кожи по индексу `toneIndex` (`1..5`) к базовому эмодзи.
 * Возвращает `baseEmoji` без изменений, если `!baseEmoji`, `toneIndex <= 0`
 * или `toneIndex >= SKIN_TONE_MODIFIERS.length`.
 *
 * @param {string} baseEmoji
 * @param {number} toneIndex
 * @returns {string}
 */
export function applySkinTone(baseEmoji, toneIndex) {
  if (
    !baseEmoji ||
    typeof baseEmoji !== "string" ||
    typeof toneIndex !== "number" ||
    !Number.isInteger(toneIndex) ||
    toneIndex <= 0 ||
    toneIndex >= SKIN_TONE_MODIFIERS.length
  ) {
    return baseEmoji;
  }
  return baseEmoji + (SKIN_TONE_MODIFIERS[toneIndex] ?? "");
}

/**
 * Возвращает ключ дедупликации эмодзи (`emojiItem.id || emojiItem.name`).
 *
 * @param {any} item
 * @returns {string}
 */
function getEmojiDedupKey(item) {
  if (!item || typeof item !== "object") return "";
  const rawKey = item.id || item.name;
  return typeof rawKey === "string" || typeof rawKey === "number" ? String(rawKey) : "";
}

/**
 * Записывает выбранный эмодзи в начало списка недавно использованных:
 * дедуплицирует по `emojiItem.id || emojiItem.name`, ставит `emojiItem` первым
 * и обрезает список до `MAX_RECENT_EMOJIS` (`18`).
 *
 * @template { { id?: string, name?: string } } T
 * @param {ReadonlyArray<T>} recentList
 * @param {T} emojiItem
 * @returns {T[]}
 */
export function recordRecentEmoji(recentList, emojiItem) {
  const safeList = Array.isArray(recentList) ? recentList : [];
  const targetKey = getEmojiDedupKey(emojiItem);
  if (!targetKey) {
    return safeList.slice(0, MAX_RECENT_EMOJIS);
  }
  const filtered = safeList.filter((entry) => getEmojiDedupKey(entry) !== targetKey);
  return [emojiItem, ...filtered].slice(0, MAX_RECENT_EMOJIS);
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
 * Заменяет дочерние узлы элемента.
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
 * Проверяет, является ли значение валидным идентификатором вкладки пикера.
 *
 * @param {unknown} tab
 * @returns {tab is ExpressionPickerTabId}
 */
function isValidTabId(tab) {
  return tab === "emoji" || tab === "gif" || tab === "stickers";
}

/**
 * Проверяет соответствие элемента поисковому запросу без учёта регистра по имени и тегам.
 *
 * @param {any} item
 * @param {string} rawQuery
 * @returns {boolean}
 */
function matchesSearchQuery(item, rawQuery) {
  const trimmed = typeof rawQuery === "string" ? rawQuery.trim().toLowerCase() : "";
  if (trimmed.length === 0) return true;
  const normalizedNeedle = trimmed.replace(/^:+|:+$/g, "");
  const needle = normalizedNeedle.length > 0 ? normalizedNeedle : trimmed;

  const nameStr = String(item?.name ?? item?.title ?? "").toLowerCase();
  if (nameStr.includes(needle) || nameStr.includes(trimmed)) return true;

  const idStr = String(item?.id ?? "").toLowerCase();
  if (idStr.includes(needle)) return true;

  const glyphStr = String(item?.glyph ?? item?.emoji ?? "").toLowerCase();
  if (glyphStr.length > 0 && glyphStr.includes(trimmed)) return true;

  if (Array.isArray(item?.tags)) {
    for (const tag of item.tags) {
      if (String(tag ?? "").toLowerCase().includes(needle)) {
        return true;
      }
    }
  }
  return false;
}

/**
 * Создаёт контроллер пикера выражений с вкладками «Эмодзи», «GIF» и «Стикеры».
 *
 * Поддерживает вызов как `createExpressionPickerController({ container, ... })`,
 * так и `createExpressionPickerController(container, options)`.
 *
 * @param {ExpressionPickerControllerOptions} configOrContainer
 * @param {ExpressionPickerControllerOptions} [maybeOptions]
 */
export function createExpressionPickerController(configOrContainer, maybeOptions = {}) {
  const rawFirst = /** @type {any} */ (configOrContainer);
  const isFirstArgContainer =
    rawFirst &&
    typeof rawFirst === "object" &&
    !("container" in rawFirst) &&
    (typeof rawFirst.appendChild === "function" ||
      typeof rawFirst.createElement === "function");

  /** @type {ExpressionPickerControllerOptions} */
  const options = isFirstArgContainer
    ? { ...maybeOptions, container: rawFirst }
    : (configOrContainer ?? {});

  const container = options.container;
  if (!container || typeof container.appendChild !== "function") {
    throw new TypeError("options.container must be a valid DOM container element");
  }
  const doc = resolveDocument(container);

  /** @type {ExpressionPickerTabId} */
  let activeTab = isValidTabId(options.activeTab) ? options.activeTab : "emoji";
  /** @type {string} */
  let searchQuery = "";
  /** @type {number} */
  let skinTone =
    typeof options.skinTone === "number" &&
    Number.isInteger(options.skinTone) &&
    options.skinTone >= 0 &&
    options.skinTone < SKIN_TONE_MODIFIERS.length
      ? options.skinTone
      : 0;
  /** @type {boolean} */
  let openState = true;

  /** @type {EmojiPickerItem[]} */
  let emojis = Array.isArray(options.emojis)
    ? [...options.emojis]
    : [...DEFAULT_PICKER_EMOJIS];
  /** @type {StickerPickerItem[]} */
  let stickers = Array.isArray(options.stickers)
    ? [...options.stickers]
    : [...DEFAULT_PICKER_STICKERS];
  /** @type {GifPickerItem[]} */
  let gifs = Array.isArray(options.gifs) ? [...options.gifs] : [...DEFAULT_PICKER_GIFS];
  /** @type {EmojiPickerItem[]} */
  let recentEmojis = Array.isArray(options.recentEmojis)
    ? options.recentEmojis.slice(0, MAX_RECENT_EMOJIS)
    : [];

  // Корневой диалог пикера выражений
  const root = doc.createElement("div");
  root.setAttribute("data-expression-picker", "");
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-label", "Пикер выражений");
  root.setAttribute("data-open", "true");
  root.setAttribute("data-active-tab", activeTab);

  // Шапка с вкладками
  const headerEl = doc.createElement("header");
  headerEl.setAttribute("data-picker-header", "");

  const tabListEl = doc.createElement("div");
  tabListEl.setAttribute("role", "tablist");
  tabListEl.setAttribute("data-picker-tabs", "");

  /** @type {Map<ExpressionPickerTabId, any>} */
  const tabButtons = new Map();
  for (const tabId of TAB_ORDER) {
    const tabBtn = doc.createElement("button");
    tabBtn.setAttribute("type", "button");
    tabBtn.setAttribute("role", "tab");
    tabBtn.setAttribute("data-picker-tab", tabId);
    tabBtn.setAttribute("aria-selected", tabId === activeTab ? "true" : "false");
    tabBtn.textContent = EXPRESSION_TABS_RU[tabId];
    tabBtn.addEventListener("click", () => {
      setTab(tabId);
    });
    tabButtons.set(tabId, tabBtn);
    tabListEl.appendChild(tabBtn);
  }
  headerEl.appendChild(tabListEl);

  // Панель поиска и выбора тона кожи
  const controlsBar = doc.createElement("div");
  controlsBar.setAttribute("data-picker-controls", "");

  const searchInput = doc.createElement("input");
  searchInput.setAttribute("type", "search");
  searchInput.setAttribute("data-picker-search", "");
  searchInput.setAttribute("placeholder", EMOJI_SEARCH_PLACEHOLDER_RU);
  searchInput.setAttribute("aria-label", EMOJI_SEARCH_PLACEHOLDER_RU);
  searchInput.value = "";
  searchInput.addEventListener("input", (/** @type {any} */ event) => {
    const nextVal = event?.target?.value ?? searchInput.value ?? "";
    setSearchQuery(String(nextVal));
  });
  controlsBar.appendChild(searchInput);

  const skinToneSelect = doc.createElement("select");
  skinToneSelect.setAttribute("data-skin-tone-select", "");
  skinToneSelect.setAttribute("aria-label", "Выбрать тон кожи эмодзи");
  for (let idx = 0; idx < SKIN_TONE_MODIFIERS.length; idx += 1) {
    const opt = doc.createElement("option");
    opt.setAttribute("value", String(idx));
    opt.value = String(idx);
    opt.textContent = idx === 0 ? "✋" : applySkinTone("✋", idx);
    skinToneSelect.appendChild(opt);
  }
  skinToneSelect.value = String(skinTone);

  /** @param {any} event */
  const handleSkinToneEvent = (event) => {
    const rawVal = event?.target?.value ?? skinToneSelect.value ?? "0";
    setSkinTone(Number(rawVal));
  };
  skinToneSelect.addEventListener("change", handleSkinToneEvent);
  skinToneSelect.addEventListener("input", handleSkinToneEvent);
  controlsBar.appendChild(skinToneSelect);

  headerEl.appendChild(controlsBar);

  // Сетка элементов
  const gridEl = doc.createElement("div");
  gridEl.setAttribute("data-picker-grid", "");
  gridEl.setAttribute("role", "listbox");

  root.appendChild(headerEl);
  root.appendChild(gridEl);

  /**
   * Обрабатывает нажатие клавиш (закрытие по `Escape`).
   *
   * @param {any} event
   */
  function handleKeyDown(event) {
    const key = event?.key ?? "";
    if (key === "Escape" || key === "Esc") {
      if (typeof event?.preventDefault === "function") {
        event.preventDefault();
      }
      close();
    }
  }

  root.addEventListener("keydown", handleKeyDown);

  function syncTabsState() {
    root.setAttribute("data-active-tab", activeTab);
    for (const tabId of TAB_ORDER) {
      const btn = tabButtons.get(tabId);
      if (btn) {
        const isSelected = tabId === activeTab;
        btn.setAttribute("aria-selected", isSelected ? "true" : "false");
        btn.setAttribute("tabindex", isSelected ? "0" : "-1");
      }
    }
  }

  function syncOpenState() {
    root.setAttribute("data-open", openState ? "true" : "false");
    root.setAttribute("aria-hidden", openState ? "false" : "true");
    if (openState) {
      root.removeAttribute("hidden");
    } else {
      root.setAttribute("hidden", "");
    }
  }

  function renderGrid() {
    /** @type {any[]} */
    const nodes = [];

    if (activeTab === "emoji") {
      const visibleEmojis = emojis.filter((item) => matchesSearchQuery(item, searchQuery));
      for (const item of visibleEmojis) {
        const itemId = String(item.id || item.name || "");
        const baseGlyph = String(item.glyph ?? item.emoji ?? item.name ?? "");
        const canTone = item.supportsSkinTone !== false;
        const displayGlyph = canTone ? applySkinTone(baseGlyph, skinTone) : baseGlyph;

        const btn = doc.createElement("button");
        btn.setAttribute("type", "button");
        btn.setAttribute("role", "option");
        btn.setAttribute("data-picker-item", itemId);
        btn.setAttribute("data-picker-item-type", "emoji");
        btn.setAttribute("data-emoji-name", String(item.name ?? itemId));
        btn.setAttribute("aria-label", String(item.name ?? itemId));
        btn.textContent = displayGlyph;
        btn.addEventListener("click", (/** @type {any} */ event) => {
          const selectedPayload = {
            ...item,
            id: item.id ?? item.name,
            name: item.name,
            glyph: displayGlyph,
            emoji: displayGlyph,
            skinTone,
          };
          recentEmojis = recordRecentEmoji(recentEmojis, selectedPayload);
          if (typeof options.onSelectEmoji === "function") {
            options.onSelectEmoji(selectedPayload, event);
          }
          if (!event?.shiftKey) {
            close();
          }
        });
        nodes.push(btn);
      }
    } else if (activeTab === "stickers") {
      const visibleStickers = stickers.filter((item) =>
        matchesSearchQuery(item, searchQuery),
      );
      for (const item of visibleStickers) {
        const itemId = String(item.id || item.name || "");
        const btn = doc.createElement("button");
        btn.setAttribute("type", "button");
        btn.setAttribute("role", "option");
        btn.setAttribute("data-picker-item", itemId);
        btn.setAttribute("data-picker-item-type", "stickers");
        btn.setAttribute("aria-label", String(item.name ?? itemId));
        btn.textContent = String(item.name ?? itemId);
        btn.addEventListener("click", (/** @type {any} */ event) => {
          if (typeof options.onSelectSticker === "function") {
            options.onSelectSticker(item, event);
          }
          if (!event?.shiftKey) {
            close();
          }
        });
        nodes.push(btn);
      }
    } else if (activeTab === "gif") {
      const visibleGifs = gifs.filter((item) => matchesSearchQuery(item, searchQuery));
      for (const item of visibleGifs) {
        const label = String(item.name ?? item.title ?? item.id ?? "");
        const itemId = String(item.id || label);
        const btn = doc.createElement("button");
        btn.setAttribute("type", "button");
        btn.setAttribute("role", "option");
        btn.setAttribute("data-picker-item", itemId);
        btn.setAttribute("data-picker-item-type", "gif");
        btn.setAttribute("aria-label", label);
        btn.textContent = label;
        btn.addEventListener("click", (/** @type {any} */ event) => {
          if (typeof options.onSelectGif === "function") {
            options.onSelectGif(item, event);
          }
          if (!event?.shiftKey) {
            close();
          }
        });
        nodes.push(btn);
      }
    }

    replaceElementChildren(gridEl, nodes);
  }

  /**
   * Переключает активную вкладку пикера (`emoji` | `gif` | `stickers`).
   *
   * @param {ExpressionPickerTabId} nextTab
   */
  function setTab(nextTab) {
    if (!isValidTabId(nextTab)) return;
    activeTab = nextTab;
    syncTabsState();
    renderGrid();
  }

  /**
   * Устанавливает поисковый запрос и перерисовывает сетку.
   *
   * @param {string} nextQuery
   */
  function setSearchQuery(nextQuery) {
    searchQuery = typeof nextQuery === "string" ? nextQuery : "";
    if (searchInput.value !== searchQuery) {
      searchInput.value = searchQuery;
    }
    renderGrid();
  }

  /**
   * Устанавливает индекс тона кожи (`0..5`) и перерисовывает сетку эмодзи.
   *
   * @param {number} nextTone
   */
  function setSkinTone(nextTone) {
    const normalized =
      typeof nextTone === "number" &&
      Number.isInteger(nextTone) &&
      nextTone >= 0 &&
      nextTone < SKIN_TONE_MODIFIERS.length
        ? nextTone
        : 0;
    skinTone = normalized;
    skinToneSelect.value = String(skinTone);
    renderGrid();
  }

  /**
   * Открывает пикер выражений (при необходимости сразу на указанной вкладке).
   *
   * @param {ExpressionPickerTabId} [tab]
   */
  function open(tab) {
    if (isValidTabId(tab)) {
      activeTab = tab;
      syncTabsState();
    }
    openState = true;
    syncOpenState();
    renderGrid();
  }

  /**
   * Закрывает пикер выражений.
   */
  function close() {
    if (!openState) return;
    openState = false;
    syncOpenState();
    if (typeof options.onClose === "function") {
      options.onClose();
    }
  }

  /**
   * Обновляет наборы данных пикера.
   *
   * @param {{ emojis?: EmojiPickerItem[], stickers?: StickerPickerItem[], gifs?: GifPickerItem[] }} nextItems
   */
  function setItems(nextItems = {}) {
    if (Array.isArray(nextItems.emojis)) emojis = [...nextItems.emojis];
    if (Array.isArray(nextItems.stickers)) stickers = [...nextItems.stickers];
    if (Array.isArray(nextItems.gifs)) gifs = [...nextItems.gifs];
    renderGrid();
  }

  syncTabsState();
  syncOpenState();
  renderGrid();
  replaceElementChildren(container, [root]);

  return {
    root,
    element: root,
    open,
    close,
    isOpen() {
      return openState;
    },
    setTab,
    getActiveTab() {
      return activeTab;
    },
    setSearchQuery,
    setSkinTone,
    setItems,
    handleKeyDown,
    getState() {
      return {
        isOpen: openState,
        activeTab,
        searchQuery,
        skinTone,
        recentEmojis: [...recentEmojis],
      };
    },
  };
}
