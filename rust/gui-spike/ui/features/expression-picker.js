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

import {
  computePosition,
  flip,
  shift,
  offset,
} from "@floating-ui/dom";
import { Virtualizer } from "@tanstack/virtual-core";
import { computeFloatingPosition } from "./floating-popover.js";

export const isFloatingUiDomPowered = typeof computePosition === "function";
export const isTanstackVirtualPowered = typeof Virtualizer === "function";

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
  Object.freeze({ id: "wave", name: "wave", glyph: "👋", emoji: "👋", category: "people", tags: ["привет", "hello", "рука"] }),
  Object.freeze({ id: "fire", name: "fire", glyph: "🔥", emoji: "🔥", category: "travel", tags: ["огонь", "hot", "пламя"] }),
  Object.freeze({ id: "thumbsup", name: "thumbsup", glyph: "👍", emoji: "👍", category: "people", tags: ["лайк", "класс", "ok"] }),
  Object.freeze({ id: "heart", name: "heart", glyph: "❤️", emoji: "❤️", category: "symbols", tags: ["сердце", "любовь", "love"] }),
  Object.freeze({ id: "rocket", name: "rocket", glyph: "🚀", emoji: "🚀", category: "travel", tags: ["ракета", "старт", "космос"] }),
  Object.freeze({ id: "tada", name: "tada", glyph: "🎉", emoji: "🎉", category: "activity", tags: ["праздник", "ура", "party"] }),
]);

/**
 * 9 стандартных категорий эмодзи Discord Cleanroom.
 * @type {ReadonlyArray<{ id: string, icon: string, title: string, items: ReadonlyArray<EmojiPickerItem> }>}
 */
export const EMOJI_CATEGORIES = Object.freeze([
  Object.freeze({
    id: "recent",
    icon: "🕒",
    title: "НЕДАВНО ИСПОЛЬЗОВАННЫЕ",
    items: Object.freeze([
      Object.freeze({ id: "wave", name: "wave", glyph: "👋", emoji: "👋", category: "recent", tags: ["привет", "hello", "рука"] }),
      Object.freeze({ id: "thumbsup", name: "thumbsup", glyph: "👍", emoji: "👍", category: "recent", tags: ["лайк", "класс", "ok"] }),
      Object.freeze({ id: "fire", name: "fire", glyph: "🔥", emoji: "🔥", category: "recent", tags: ["огонь", "hot", "пламя"] }),
      Object.freeze({ id: "heart", name: "heart", glyph: "❤️", emoji: "❤️", category: "recent", tags: ["сердце", "любовь", "love"] }),
      Object.freeze({ id: "joy", name: "joy", glyph: "😂", emoji: "😂", category: "recent", tags: ["смех", "слёзы", "joy"] }),
    ]),
  }),
  Object.freeze({
    id: "people",
    icon: "😀",
    title: "ЛЮДИ И ЭМОЦИИ",
    items: Object.freeze([
      Object.freeze({ id: "grinning", name: "grinning", glyph: "😀", emoji: "😀", category: "people", tags: ["улыбка", "радость"] }),
      Object.freeze({ id: "smile", name: "smile", glyph: "😄", emoji: "😄", category: "people", tags: ["улыбка", "счастье"] }),
      Object.freeze({ id: "joy", name: "joy", glyph: "😂", emoji: "😂", category: "people", tags: ["смех", "joy"] }),
      Object.freeze({ id: "heart_eyes", name: "heart_eyes", glyph: "😍", emoji: "😍", category: "people", tags: ["любовь", "сердечки"] }),
      Object.freeze({ id: "thinking", name: "thinking", glyph: "🤔", emoji: "🤔", category: "people", tags: ["думает", "вопрос"] }),
      Object.freeze({ id: "wave", name: "wave", glyph: "👋", emoji: "👋", category: "people", tags: ["привет", "hello", "рука"] }),
      Object.freeze({ id: "thumbsup", name: "thumbsup", glyph: "👍", emoji: "👍", category: "people", tags: ["лайк", "класс", "ok"] }),
      Object.freeze({ id: "clap", name: "clap", glyph: "👏", emoji: "👏", category: "people", tags: ["аплодисменты", "браво"] }),
    ]),
  }),
  Object.freeze({
    id: "nature",
    icon: "🐻",
    title: "ЖИВОТНЫЕ И ПРИРОДА",
    items: Object.freeze([
      Object.freeze({ id: "dog", name: "dog", glyph: "🐶", emoji: "🐶", category: "nature", tags: ["собака", "пес"] }),
      Object.freeze({ id: "cat", name: "cat", glyph: "🐱", emoji: "🐱", category: "nature", tags: ["кот", "кошка"] }),
      Object.freeze({ id: "fox", name: "fox", glyph: "🦊", emoji: "🦊", category: "nature", tags: ["лиса"] }),
      Object.freeze({ id: "frog", name: "frog", glyph: "🐸", emoji: "🐸", category: "nature", tags: ["лягушка"] }),
      Object.freeze({ id: "cherry_blossom", name: "cherry_blossom", glyph: "🌸", emoji: "🌸", category: "nature", tags: ["сакура", "цветок"] }),
    ]),
  }),
  Object.freeze({
    id: "food",
    icon: "🍔",
    title: "ЕДА И НАПИТКИ",
    items: Object.freeze([
      Object.freeze({ id: "pizza", name: "pizza", glyph: "🍕", emoji: "🍕", category: "food", tags: ["пицца", "еда"] }),
      Object.freeze({ id: "hamburger", name: "hamburger", glyph: "🍔", emoji: "🍔", category: "food", tags: ["бургер", "гамбургер"] }),
      Object.freeze({ id: "coffee", name: "coffee", glyph: "☕", emoji: "☕", category: "food", tags: ["кофе", "чай"] }),
      Object.freeze({ id: "cake", name: "cake", glyph: "🍰", emoji: "🍰", category: "food", tags: ["торт", "пирожное"] }),
    ]),
  }),
  Object.freeze({
    id: "activity",
    icon: "⚽",
    title: "АКТИВНОСТИ",
    items: Object.freeze([
      Object.freeze({ id: "tada", name: "tada", glyph: "🎉", emoji: "🎉", category: "activity", tags: ["праздник", "ура", "party"] }),
      Object.freeze({ id: "video_game", name: "video_game", glyph: "🎮", emoji: "🎮", category: "activity", tags: ["игры", "геймпад"] }),
      Object.freeze({ id: "trophy", name: "trophy", glyph: "🏆", emoji: "🏆", category: "activity", tags: ["кубок", "победа"] }),
      Object.freeze({ id: "soccer", name: "soccer", glyph: "⚽", emoji: "⚽", category: "activity", tags: ["футбол", "мяч"] }),
    ]),
  }),
  Object.freeze({
    id: "travel",
    icon: "🚗",
    title: "ПУТЕШЕСТВИЯ И МЕСТА",
    items: Object.freeze([
      Object.freeze({ id: "rocket", name: "rocket", glyph: "🚀", emoji: "🚀", category: "travel", tags: ["ракета", "старт", "космос"] }),
      Object.freeze({ id: "fire", name: "fire", glyph: "🔥", emoji: "🔥", category: "travel", tags: ["огонь", "hot", "пламя"] }),
      Object.freeze({ id: "airplane", name: "airplane", glyph: "✈️", emoji: "✈️", category: "travel", tags: ["самолет", "полет"] }),
      Object.freeze({ id: "star", name: "star", glyph: "⭐", emoji: "⭐", category: "travel", tags: ["звезда"] }),
    ]),
  }),
  Object.freeze({
    id: "objects",
    icon: "💡",
    title: "ПРЕДМЕТЫ",
    items: Object.freeze([
      Object.freeze({ id: "bulb", name: "bulb", glyph: "💡", emoji: "💡", category: "objects", tags: ["идея", "лампочка"] }),
      Object.freeze({ id: "laptop", name: "laptop", glyph: "💻", emoji: "💻", category: "objects", tags: ["ноутбук", "код"] }),
      Object.freeze({ id: "key", name: "key", glyph: "🔑", emoji: "🔑", category: "objects", tags: ["ключ"] }),
    ]),
  }),
  Object.freeze({
    id: "symbols",
    icon: "❤️",
    title: "СИМВОЛЫ",
    items: Object.freeze([
      Object.freeze({ id: "heart", name: "heart", glyph: "❤️", emoji: "❤️", category: "symbols", tags: ["сердце", "любовь", "love"] }),
      Object.freeze({ id: "100", name: "100", glyph: "💯", emoji: "💯", category: "symbols", tags: ["сто", "идеально"] }),
      Object.freeze({ id: "white_check_mark", name: "white_check_mark", glyph: "✅", emoji: "✅", category: "symbols", tags: ["готово", "ок"] }),
    ]),
  }),
  Object.freeze({
    id: "flags",
    icon: "🏳️",
    title: "ФЛАГИ",
    items: Object.freeze([
      Object.freeze({ id: "checkered_flag", name: "checkered_flag", glyph: "🏁", emoji: "🏁", category: "flags", tags: ["финиш", "флаг"] }),
      Object.freeze({ id: "flag_ru", name: "flag_ru", glyph: "🇷🇺", emoji: "🇷🇺", category: "flags", tags: ["россия", "ru"] }),
      Object.freeze({ id: "flag_us", name: "flag_us", glyph: "🇺🇸", emoji: "🇺🇸", category: "flags", tags: ["сша", "us"] }),
    ]),
  }),
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
 * @property {string} [category]
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
 * @property {string} [activeCategory]
 * @property {any} [anchorEl]
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
/**
 * @typedef {object} ReactionCountEntry
 * @property {string} emoji
 * @property {number} count
 * @property {boolean} [me]
 */

/**
 * Неизменяемо увеличивает счётчик реакции (или добавляет новую с count = 1, me = true).
 *
 * @param {ReadonlyArray<ReactionCountEntry>} reactions
 * @param {string} emojiKey
 * @returns {ReactionCountEntry[]}
 */
export function incrementReactionCount(reactions, emojiKey) {
  const list = Array.isArray(reactions) ? reactions : [];
  const key = typeof emojiKey === "string" ? emojiKey.trim() : "";
  if (!key) return list.map((item) => ({ ...item }));

  let found = false;
  const next = list.map((item) => {
    if (item && item.emoji === key) {
      found = true;
      const current = Number.isFinite(item.count) ? Math.max(0, item.count) : 0;
      return { ...item, count: current + 1, me: true };
    }
    return { ...item };
  });

  if (!found) {
    next.push({ emoji: key, count: 1, me: true });
  }
  return next;
}

/**
 * Неизменяемо уменьшает счётчик реакции (удаляет запись при достижении 0).
 *
 * @param {ReadonlyArray<ReactionCountEntry>} reactions
 * @param {string} emojiKey
 * @returns {ReactionCountEntry[]}
 */
export function decrementReactionCount(reactions, emojiKey) {
  const list = Array.isArray(reactions) ? reactions : [];
  const key = typeof emojiKey === "string" ? emojiKey.trim() : "";
  if (!key) return list.map((item) => ({ ...item }));

  /** @type {ReactionCountEntry[]} */
  const next = [];
  for (const item of list) {
    if (item && item.emoji === key) {
      const current = Number.isFinite(item.count) ? Math.max(0, item.count) : 0;
      const updatedCount = Math.max(0, current - 1);
      if (updatedCount > 0) {
        next.push({ ...item, count: updatedCount, me: false });
      }
    } else if (item) {
      next.push({ ...item });
    }
  }
  return next;
}

/**
 * Переключает реакцию текущего пользователя: уменьшает, если me === true, иначе увеличивает.
 *
 * @param {ReadonlyArray<ReactionCountEntry>} reactions
 * @param {string} emojiKey
 * @returns {ReactionCountEntry[]}
 */
export function toggleReactionCount(reactions, emojiKey) {
  const list = Array.isArray(reactions) ? reactions : [];
  const key = typeof emojiKey === "string" ? emojiKey.trim() : "";
  if (!key) return list.map((item) => ({ ...item }));

  const existing = list.find((item) => item && item.emoji === key);
  if (existing && existing.me === true) {
    return decrementReactionCount(list, key);
  }
  return incrementReactionCount(list, key);
}

/**
 * @param {any} [containerEl]
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
  let activeCategory =
    typeof options.activeCategory === "string" && options.activeCategory
      ? options.activeCategory
      : EMOJI_CATEGORIES[0]?.id ?? "recent";
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

  let hasCustomEmojis = Array.isArray(options.emojis);
  /** @type {EmojiPickerItem[]} */
  let emojis = hasCustomEmojis
    ? [.../** @type {EmojiPickerItem[]} */ (options.emojis)]
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

  // Тело пикера: левая панель категорий + сетка элементов
  const bodyEl = doc.createElement("div");
  bodyEl.setAttribute("data-picker-body", "");

  const categoryRailEl = doc.createElement("nav");
  categoryRailEl.setAttribute("data-picker-category-rail", "");
  categoryRailEl.setAttribute("aria-label", "Категории эмодзи");

  /** @type {Map<string, any>} */
  const categoryButtons = new Map();
  /** @type {Map<string, any>} */
  const categorySections = new Map();

  for (const cat of EMOJI_CATEGORIES) {
    const catBtn = doc.createElement("button");
    catBtn.setAttribute("type", "button");
    catBtn.setAttribute("data-picker-category-btn", cat.id);
    catBtn.setAttribute("aria-label", cat.title);
    catBtn.setAttribute("aria-pressed", cat.id === activeCategory ? "true" : "false");
    catBtn.textContent = cat.icon;
    catBtn.addEventListener("click", () => {
      scrollToCategory(cat.id);
    });
    categoryButtons.set(cat.id, catBtn);
    categoryRailEl.appendChild(catBtn);
  }

  // Сетка элементов
  const gridEl = doc.createElement("div");
  gridEl.setAttribute("data-picker-grid", "");
  gridEl.setAttribute("role", "listbox");

  bodyEl.appendChild(categoryRailEl);
  bodyEl.appendChild(gridEl);

  // Нижняя панель предпросмотра (Cleanroom footer)
  const footerEl = doc.createElement("footer");
  footerEl.setAttribute("data-picker-footer", "");
  const footerGlyphEl = doc.createElement("span");
  footerGlyphEl.setAttribute("data-picker-footer-glyph", "");
  footerGlyphEl.textContent = "😀";
  const footerLabelEl = doc.createElement("span");
  footerLabelEl.setAttribute("data-picker-footer-label", "");
  footerLabelEl.textContent = ":grinning:";
  const footerSvg = doc.createElement("svg");
  footerSvg.setAttribute("data-cleanroom-svg", "picker-preview");
  footerSvg.setAttribute("width", "16");
  footerSvg.setAttribute("height", "16");
  footerSvg.setAttribute("viewBox", "0 0 24 24");
  footerSvg.setAttribute("aria-hidden", "true");
  footerEl.appendChild(footerSvg);
  footerEl.appendChild(footerGlyphEl);
  footerEl.appendChild(footerLabelEl);

  root.appendChild(headerEl);
  root.appendChild(bodyEl);
  root.appendChild(footerEl);

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

  function syncCategoryButtons() {
    root.setAttribute("data-active-category", activeCategory);
    for (const [catId, btn] of categoryButtons.entries()) {
      btn.setAttribute("aria-pressed", catId === activeCategory ? "true" : "false");
    }
  }

  function syncTabsState() {
    root.setAttribute("data-active-tab", activeTab);
    const isEmojiTab = activeTab === "emoji";
    if (isEmojiTab) {
      categoryRailEl.removeAttribute("hidden");
    } else {
      categoryRailEl.setAttribute("hidden", "");
    }
    for (const tabId of TAB_ORDER) {
      const btn = tabButtons.get(tabId);
      if (btn) {
        const isSelected = tabId === activeTab;
        btn.setAttribute("aria-selected", isSelected ? "true" : "false");
        btn.setAttribute("tabindex", isSelected ? "0" : "-1");
      }
    }
  }

  /**
   * Создаёт кнопку эмодзи для сетки и привязывает hover/click обработчики.
   *
   * @param {EmojiPickerItem} item
   */
  function buildEmojiButton(item) {
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

    const updateFooter = () => {
      footerGlyphEl.textContent = displayGlyph;
      footerLabelEl.textContent = `:${String(item.name ?? itemId)}:`;
    };
    btn.addEventListener("mouseenter", updateFooter);
    btn.addEventListener("focus", updateFooter);

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
    return btn;
  }

  function renderGrid() {
    /** @type {any[]} */
    const nodes = [];
    categorySections.clear();

    if (activeTab === "emoji") {
      if (hasCustomEmojis) {
        const visibleEmojis = emojis.filter((item) => matchesSearchQuery(item, searchQuery));
        for (const item of visibleEmojis) {
          nodes.push(buildEmojiButton(item));
        }
      } else {
        const dynamicCategories = EMOJI_CATEGORIES.map((cat) => {
          if (cat.id !== "recent") return cat;
          const mergedRecent = [...recentEmojis];
          for (const baseItem of cat.items) {
            const key = getEmojiDedupKey(baseItem);
            if (!mergedRecent.some((r) => getEmojiDedupKey(r) === key)) {
              mergedRecent.push(baseItem);
            }
          }
          return {
            ...cat,
            items: mergedRecent.slice(0, MAX_RECENT_EMOJIS),
          };
        });

        for (const cat of dynamicCategories) {
          const filtered = cat.items.filter((item) => matchesSearchQuery(item, searchQuery));
          if (filtered.length === 0) continue;

          const sectionEl = doc.createElement("section");
          sectionEl.setAttribute("data-picker-category-section", cat.id);

          const sectionHeader = doc.createElement("div");
          sectionHeader.setAttribute("data-picker-category-header", cat.id);
          sectionHeader.textContent = `${cat.icon} ${cat.title}`;
          sectionEl.appendChild(sectionHeader);

          const sectionGrid = doc.createElement("div");
          sectionGrid.setAttribute("data-picker-category-grid", cat.id);
          for (const item of filtered) {
            sectionGrid.appendChild(buildEmojiButton(item));
          }
          sectionEl.appendChild(sectionGrid);
          categorySections.set(cat.id, sectionEl);
          nodes.push(sectionEl);
        }
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
        btn.addEventListener("mouseenter", () => {
          footerGlyphEl.textContent = "✨";
          footerLabelEl.textContent = String(item.name ?? itemId);
        });
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
        btn.addEventListener("mouseenter", () => {
          footerGlyphEl.textContent = "GIF";
          footerLabelEl.textContent = label;
        });
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

    if (nodes.length === 0) {
      const emptyEl = doc.createElement("div");
      emptyEl.setAttribute("data-picker-empty", "");
      emptyEl.textContent = "Ничего не найдено";
      nodes.push(emptyEl);
    }

    replaceElementChildren(gridEl, nodes);
  }

  /**
   * Переходит к указанной категории эмодзи и прокручивает к ней сетку.
   *
   * @param {string} categoryId
   * @returns {boolean}
   */
  function scrollToCategory(categoryId) {
    if (!categoryButtons.has(categoryId)) return false;
    activeCategory = categoryId;
    syncCategoryButtons();
    if (activeTab !== "emoji") {
      setTab("emoji");
    }
    if (searchQuery !== "") {
      setSearchQuery("");
    }
    const sectionEl = categorySections.get(categoryId);
    if (sectionEl && typeof sectionEl.scrollIntoView === "function") {
      sectionEl.scrollIntoView({ block: "start", behavior: "smooth" });
    }
    return true;
  }

  /**
   * Позиционирует пикер относительно `options.anchorEl` через `@floating-ui/dom` (`computeFloatingPosition`).
   *
   * @param {any} [overrideAnchor]
   * @returns {Promise<{ x: number, y: number, placement: string }>}
   */
  async function updatePosition(overrideAnchor) {
    const anchor = overrideAnchor ?? options.anchorEl;
    const anchorRect =
      anchor && typeof anchor.getBoundingClientRect === "function"
        ? anchor.getBoundingClientRect()
        : { x: 16, y: 16, width: 32, height: 32 };
    const floatingRect =
      typeof root.getBoundingClientRect === "function"
        ? root.getBoundingClientRect()
        : { width: 400, height: 430 };
    const pos = computeFloatingPosition(anchorRect, {
      width: floatingRect.width || 400,
      height: floatingRect.height || 430,
    }, { placement: "top-end", offset: 8, padding: 12 });
    if (root.style && typeof root.style === "object") {
      root.style.left = `${pos.x}px`;
      root.style.top = `${pos.y}px`;
    }
    root.setAttribute("data-placement", pos.placement);
    return pos;
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

  function syncOpenState() {
    root.setAttribute("data-open", openState ? "true" : "false");
    root.setAttribute("data-state", openState ? "open" : "closed");
    root.hidden = !openState;
    if (root.style) {
      root.style.display = openState ? "" : "none";
    }
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
    if (Array.isArray(nextItems.emojis)) {
      hasCustomEmojis = true;
      emojis = [...nextItems.emojis];
    }
    if (Array.isArray(nextItems.stickers)) stickers = [...nextItems.stickers];
    if (Array.isArray(nextItems.gifs)) gifs = [...nextItems.gifs];
    renderGrid();
  }

  syncCategoryButtons();
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
    scrollToCategory,
    updatePosition,
    setSearchQuery,
    setSkinTone,
    setItems,
    handleKeyDown,
    getState() {
      return {
        isOpen: openState,
        activeTab,
        activeCategory,
        searchQuery,
        skinTone,
        recentEmojis: [...recentEmojis],
      };
    },
  };
}
