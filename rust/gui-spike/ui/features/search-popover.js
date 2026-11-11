// @ts-check
/**
 * Поисковый попап канала (`search-popover.js`):
 * - Автодополнение фильтр-токенов (`from:`, `mentions:`, `has:`, `in:`, `before:`, `after:`, `pinned:`)
 *   и значений фильтров (`has:image`, `has:video`, `has:link`, `has:file`, `has:embed`, `from:<user>`,
 *   `mentions:<user>`, `in:<channel>`, `pinned:true|false`);
 * - Клавиатурная навигация (`ArrowDown`, `ArrowUp`, `Home`, `End`, `Tab`, `Enter`, `Escape`)
 *   с обновлением `aria-selected` и `aria-activedescendant`;
 * - Подсветка активных токенов в поле ввода и поисковых совпадений в карточках результатов;
 * - Переход к сообщению (`onJumpToMessage`), состояния индексации (`HTTP 202`), ошибки (`HTTP 429`),
 *   NSFW-бейдж `18+` и история недавних поисковых запросов.
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` §1.1–§1.3 (`searchResultsWrap_a98f3b`,
 *   `searchResultGroup_a7e67f`, `id="search-result-<id>"`);
 * - `waves/RW-40/DESIGN/SEARCH-202-NSFW-SPEC.md` §1–§6.
 */
import {
  SEARCH_PAGE_SIZE,
  MAX_SEARCH_OFFSET,
  MAX_SEARCH_PAGES,
  SEARCH_DEFAULT_RETRY_AFTER_MS,
  SEARCH_POPOVER_TOKEN_SUGGESTIONS,
  getSearchTokenSuggestions,
  tokenizeSearchInputForHighlight,
  splitTextBySearchQuery,
  parseSearchQueryTokens,
  splitQueryValues,
  buildSearchQuery,
} from "./search.js";

export {
  SEARCH_POPOVER_TOKEN_SUGGESTIONS,
  getSearchTokenSuggestions,
  tokenizeSearchInputForHighlight,
  splitTextBySearchQuery,
};

/**
 * @typedef {{
 *   token: string,
 *   key: string,
 *   value?: string,
 *   labelRu: string,
 *   hintRu?: string,
 *   example?: string,
 * }} SearchPopoverCompletionItem
 */

/**
 * @typedef {{
 *   id: string,
 *   channelId: string,
 *   channelName?: string,
 *   author?: string,
 *   content?: string,
 *   timestamp?: string,
 *   nsfw?: boolean,
 *   isNsfw?: boolean,
 * }} SearchPopoverResultItem
 */

/**
 * @typedef {{
 *   initialQuery?: string,
 *   users?: readonly string[],
 *   channels?: readonly string[],
 *   onSearch?: (payload: { query: string, freeText: string, parsedQuery: Record<string, unknown> }) => void,
 *   onJumpToMessage?: (messageId: string, channelId: string, item: SearchPopoverResultItem) => void,
 *   onClose?: () => void,
 * }} SearchPopoverOptions
 */

/**
 * Граничные константы поискового попапа (RE-оракул `SEARCH-202-NSFW-SPEC.md` §1.1).
 */
export const SEARCH_POPOVER_LIMITS = Object.freeze({
  PAGE_SIZE: SEARCH_PAGE_SIZE,
  MIN_OFFSET: 0,
  MAX_OFFSET: MAX_SEARCH_OFFSET,
  MAX_PAGES: MAX_SEARCH_PAGES,
  MAX_QUERY_LENGTH: 2000,
  MAX_HISTORY: 10,
  DEFAULT_RETRY_AFTER_MS: SEARCH_DEFAULT_RETRY_AFTER_MS,
});

/**
 * Значения фильтра `has:` согласно RE-оракулу (`frontend-d03.md` §1.2, `SEARCH-202-NSFW-SPEC.md` §2.3).
 * @type {ReadonlyArray<Readonly<{ value: string, token: string, key: "has", labelRu: string }>>}
 */
export const HAS_FILTER_VALUE_SUGGESTIONS = Object.freeze([
  Object.freeze({ value: "image", token: "has:image", key: /** @type {const} */ ("has"), labelRu: "содержит: изображение" }),
  Object.freeze({ value: "video", token: "has:video", key: /** @type {const} */ ("has"), labelRu: "содержит: видео" }),
  Object.freeze({ value: "link", token: "has:link", key: /** @type {const} */ ("has"), labelRu: "содержит: ссылка" }),
  Object.freeze({ value: "file", token: "has:file", key: /** @type {const} */ ("has"), labelRu: "содержит: файл" }),
  Object.freeze({ value: "embed", token: "has:embed", key: /** @type {const} */ ("has"), labelRu: "содержит: вложение" }),
  Object.freeze({ value: "sound", token: "has:sound", key: /** @type {const} */ ("has"), labelRu: "содержит: звук" }),
  Object.freeze({ value: "poll", token: "has:poll", key: /** @type {const} */ ("has"), labelRu: "содержит: опрос" }),
  Object.freeze({ value: "sticker", token: "has:sticker", key: /** @type {const} */ ("has"), labelRu: "содержит: стикер" }),
  Object.freeze({ value: "forward", token: "has:forward", key: /** @type {const} */ ("has"), labelRu: "содержит: пересланное" }),
]);

/**
 * Зажимает смещение пагинации `offset` в допустимый диапазон `0..9975`.
 * @param {number} offset
 * @returns {number}
 */
export function clampSearchPopoverOffset(offset) {
  const num = Number(offset);
  if (!Number.isFinite(num) || num <= SEARCH_POPOVER_LIMITS.MIN_OFFSET) {
    return SEARCH_POPOVER_LIMITS.MIN_OFFSET;
  }
  return Math.min(Math.trunc(num), SEARCH_POPOVER_LIMITS.MAX_OFFSET);
}

/**
 * Возвращает автодополнения значений для токен-префиксов `from:`, `mentions:`, `has:`, `in:`, `pinned:`
 * либо список базовых токен-подсказок.
 *
 * @param {string | undefined} rawQuery
 * @param {{ users?: readonly string[], channels?: readonly string[] }} [context]
 * @returns {ReadonlyArray<Readonly<SearchPopoverCompletionItem>>}
 */
export function getFilterTokenCompletions(rawQuery, context = {}) {
  if (typeof rawQuery !== "string" || rawQuery.length === 0 || /\s$/.test(rawQuery)) {
    return SEARCH_POPOVER_TOKEN_SUGGESTIONS;
  }
  const parts = rawQuery.split(/\s+/);
  const trailing = parts[parts.length - 1] ?? "";
  const lower = trailing.toLowerCase();
  const colonIdx = lower.indexOf(":");

  if (colonIdx > 0) {
    const op = lower.slice(0, colonIdx);
    const valPrefix = lower.slice(colonIdx + 1);
    return resolveValueCompletions(op, valPrefix, context);
  }
  return getSearchTokenSuggestions(rawQuery);
}

/**
 * Разрешает автодополнение значений после оператора `from:`, `mentions:`, `has:`, `in:`, `pinned:`.
 * @param {string} op
 * @param {string} valPrefix
 * @param {{ users?: readonly string[], channels?: readonly string[] }} context
 * @returns {ReadonlyArray<Readonly<SearchPopoverCompletionItem>>}
 */
function resolveValueCompletions(op, valPrefix, context) {
  if (op === "has" || op === "имеет") {
    return HAS_FILTER_VALUE_SUGGESTIONS.filter(
      (item) => item.value.startsWith(valPrefix) || item.labelRu.toLowerCase().includes(valPrefix),
    );
  }
  if (op === "from" || op === "от" || op === "mentions") {
    const key = op === "mentions" ? "mentions" : "from";
    const labelPrefix = key === "mentions" ? "упоминания" : "от";
    const users = Array.isArray(context?.users) ? context.users : [];
    return users
      .map((u) => String(u ?? "").trim())
      .filter((u) => u.length > 0 && u.toLowerCase().startsWith(valPrefix))
      .map((u) =>
        Object.freeze({
          token: `${key}:${u}`,
          key,
          value: u,
          labelRu: `${labelPrefix}: ${u}`,
        }),
      );
  }
  if (op === "in" || op === "в") {
    const cleanPrefix = valPrefix.replace(/^#+/, "");
    const channels = Array.isArray(context?.channels) ? context.channels : [];
    return channels
      .map((c) => String(c ?? "").replace(/^#+/, "").trim())
      .filter((c) => c.length > 0 && c.toLowerCase().startsWith(cleanPrefix))
      .map((c) =>
        Object.freeze({
          token: `in:${c}`,
          key: "in",
          value: c,
          labelRu: `в: #${c}`,
        }),
      );
  }
  if (op === "pinned" || op === "закреплённые") {
    return ["true", "false"]
      .filter((v) => v.startsWith(valPrefix))
      .map((v) =>
        Object.freeze({
          token: `pinned:${v}`,
          key: "pinned",
          value: v,
          labelRu: `закреплено: ${v}`,
        }),
      );
  }
  return [];
}

/**
 * Подставляет выбранный токен или пару `токен:значение` вместо последнего незавершённого фрагмента.
 * Если подставляется завершённое значение фильтра (`has:image`, `from:alex`), в конец добавляется пробел.
 *
 * @param {string | undefined} rawQuery
 * @param {string} tokenToApply
 * @returns {string}
 */
export function completeFilterToken(rawQuery, tokenToApply) {
  const input = typeof rawQuery === "string" ? rawQuery : "";
  const cleanToken = String(tokenToApply ?? "").trim();
  if (cleanToken === "") return input;

  const isCompletedPair = cleanToken.includes(":") && !cleanToken.endsWith(":");
  const replacement = isCompletedPair ? `${cleanToken} ` : cleanToken;

  if (input.length === 0 || /\s$/.test(input)) {
    return `${input}${replacement}`;
  }
  const lastTokenIdx = input.search(/\S+$/);
  const prefix = lastTokenIdx > 0 ? input.slice(0, lastTokenIdx) : "";
  return `${prefix}${replacement}`;
}

/**
 * Разбирает строку запроса поискового попапа в структурированный объект фильтров.
 *
 * @param {string | undefined} rawQuery
 * @returns {{
 *   from: string[],
 *   mentions: string[],
 *   has: string[],
 *   in: string[],
 *   before: string | null,
 *   after: string | null,
 *   pinned: boolean | null,
 *   freeText: string,
 *   chips: ReturnType<typeof parseSearchQueryTokens>["chips"],
 * }}
 */
export function parseSearchPopoverQuery(rawQuery) {
  const parsed = parseSearchQueryTokens(rawQuery);
  return {
    from: splitQueryValues(parsed.tokenFilters.from),
    mentions: splitQueryValues(parsed.tokenFilters.mentions),
    has: splitQueryValues(parsed.tokenFilters.has),
    in: splitQueryValues(parsed.tokenFilters.in),
    before: parsed.tokenFilters.before || null,
    after: parsed.tokenFilters.after || null,
    pinned: parsed.tokenFilters.pinned,
    freeText: parsed.freeText,
    chips: parsed.chips,
  };
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolvePopoverDocument(containerEl) {
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
  throw new TypeError("containerEl must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы элемента без использования `innerHTML`.
 * @param {any} targetEl
 * @param {any[]} nodes
 */
function replacePopoverChildren(targetEl, nodes) {
  if (typeof targetEl.replaceChildren === "function") {
    targetEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(targetEl.children)) {
    for (const child of targetEl.children) {
      if (child && typeof child === "object") {
        child.parentNode = null;
      }
    }
    targetEl.children.length = 0;
  }
  for (const node of nodes) {
    targetEl.appendChild(node);
  }
}

/**
 * Создаёт карточку одного результата поиска с подсветкой совпадений и кнопкой прыжка.
 * @param {{ createElement: (tag: string) => any }} doc
 * @param {SearchPopoverResultItem} item
 * @param {boolean} isSelected
 * @param {string} query
 * @param {(item: SearchPopoverResultItem) => void} onJump
 */
function buildResultCardNode(doc, item, isSelected, query, onJump) {
  const msgId = String(item.id ?? "");
  const channelId = String(item.channelId ?? "");
  const card = doc.createElement("article");
  card.setAttribute("id", `search-result-${msgId}`);
  card.setAttribute("role", "option");
  card.setAttribute(
    "class",
    isSelected ? "search-popover-result-item is-active" : "search-popover-result-item",
  );
  card.setAttribute("data-search-result-id", msgId);
  card.setAttribute("data-channel-id", channelId);
  card.setAttribute("aria-selected", isSelected ? "true" : "false");

  const head = buildResultCardHeader(doc, item, msgId, channelId, onJump);
  const contentEl = buildResultCardContent(doc, item.content ?? "", query);
  card.appendChild(head);
  card.appendChild(contentEl);
  return card;
}

/**
 * Создаёт шапку карточки результата поиска (автор, канал, NSFW-бейдж, время, кнопка «Перейти»).
 * @param {{ createElement: (tag: string) => any }} doc
 * @param {SearchPopoverResultItem} item
 * @param {string} msgId
 * @param {string} channelId
 * @param {(item: SearchPopoverResultItem) => void} onJump
 */
function buildResultCardHeader(doc, item, msgId, channelId, onJump) {
  const head = doc.createElement("div");
  head.setAttribute("class", "search-popover-result-head");

  const authorEl = doc.createElement("span");
  authorEl.setAttribute("class", "search-popover-result-author");
  authorEl.setAttribute("data-result-author", "");
  authorEl.textContent = typeof item.author === "string" ? item.author : "";
  head.appendChild(authorEl);

  if (typeof item.channelName === "string" && item.channelName.length > 0) {
    const channelEl = doc.createElement("span");
    channelEl.setAttribute("class", "search-popover-result-channel");
    channelEl.setAttribute("data-result-channel", "");
    channelEl.textContent = `#${item.channelName.replace(/^#+/, "")}`;
    head.appendChild(channelEl);
  }

  if (item.nsfw === true || item.isNsfw === true) {
    const nsfwBadge = doc.createElement("span");
    nsfwBadge.setAttribute("class", "search-result-nsfw-badge");
    nsfwBadge.setAttribute("data-result-nsfw-badge", "");
    nsfwBadge.textContent = "18+";
    head.appendChild(nsfwBadge);
  }

  if (typeof item.timestamp === "string" && item.timestamp.length > 0) {
    const timeEl = doc.createElement("span");
    timeEl.setAttribute("class", "search-popover-result-timestamp");
    timeEl.setAttribute("data-result-timestamp", "");
    timeEl.textContent = item.timestamp;
    head.appendChild(timeEl);
  }

  const jumpBtn = doc.createElement("button");
  jumpBtn.setAttribute("type", "button");
  jumpBtn.setAttribute("class", "search-popover-jump-btn");
  jumpBtn.setAttribute("data-action", "jump-to-message");
  jumpBtn.setAttribute("data-jump-message-id", msgId);
  jumpBtn.setAttribute("data-jump-channel-id", channelId);
  jumpBtn.textContent = "Перейти";
  jumpBtn.addEventListener("click", () => onJump(item));
  head.appendChild(jumpBtn);
  return head;
}

/**
 * Создаёт абзац текста результата поиска с подсветкой совпадений (`<mark data-search-highlight>`).
 * @param {{ createElement: (tag: string) => any }} doc
 * @param {string} rawContent
 * @param {string} query
 */
function buildResultCardContent(doc, rawContent, query) {
  const contentEl = doc.createElement("p");
  contentEl.setAttribute("class", "search-popover-result-content");
  contentEl.setAttribute("data-result-content", "");

  const textParts = splitTextBySearchQuery(rawContent, query);
  /** @type {any[]} */
  const contentNodes = [];
  for (const part of textParts) {
    if (part.match) {
      const mark = doc.createElement("mark");
      mark.setAttribute("class", "search-highlight");
      mark.setAttribute("data-search-highlight", "");
      mark.textContent = part.text;
      contentNodes.push(mark);
    } else {
      const span = doc.createElement("span");
      span.textContent = part.text;
      contentNodes.push(span);
    }
  }
  replacePopoverChildren(contentEl, contentNodes);
  return contentEl;
}

/**
 * Создаёт DOM-каркас поискового попапа.
 * @param {{ createElement: (tag: string) => any }} doc
 * @param {string} initialQuery
 */
function buildPopoverDomSkeleton(doc, initialQuery) {
  const root = doc.createElement("section");
  root.setAttribute("class", "search-popover");
  root.setAttribute("data-search-popover", "");
  root.setAttribute("role", "search");
  root.setAttribute("aria-label", "Поиск сообщений");
  root.setAttribute("data-open", "true");
  root.setAttribute("aria-hidden", "false");
  root.setAttribute("aria-busy", "false");

  const barEl = doc.createElement("div");
  barEl.setAttribute("class", "search-popover-bar");
  barEl.setAttribute("data-search-popover-bar", "");

  const input = doc.createElement("input");
  input.setAttribute("type", "search");
  input.setAttribute("class", "search-popover-input search-input");
  input.setAttribute("data-search-popover-input", "");
  input.setAttribute("placeholder", "Поиск");
  input.setAttribute("aria-label", "Поиск сообщений");
  input.setAttribute("autocomplete", "off");
  input.value = initialQuery;

  const highlightBar = doc.createElement("div");
  highlightBar.setAttribute("class", "search-popover-input-highlight");
  highlightBar.setAttribute("data-search-input-highlight", "");
  highlightBar.setAttribute("aria-hidden", "true");

  barEl.appendChild(input);
  barEl.appendChild(highlightBar);

  const suggestionsList = doc.createElement("div");
  suggestionsList.setAttribute("class", "search-popover-suggestions");
  suggestionsList.setAttribute("role", "listbox");
  suggestionsList.setAttribute("aria-label", "Подсказки фильтров поиска");
  suggestionsList.setAttribute("data-search-token-suggestions", "");

  const indexingBanner = doc.createElement("div");
  indexingBanner.setAttribute("class", "search-popover-indexing");
  indexingBanner.setAttribute("data-search-indexing-banner", "");
  indexingBanner.setAttribute("role", "status");
  indexingBanner.setAttribute("aria-live", "polite");
  indexingBanner.setAttribute("hidden", "");

  const errorBanner = doc.createElement("div");
  errorBanner.setAttribute("class", "search-popover-error");
  errorBanner.setAttribute("data-search-error", "");
  errorBanner.setAttribute("role", "alert");
  errorBanner.setAttribute("hidden", "");

  const headerEl = doc.createElement("div");
  headerEl.setAttribute("class", "search-popover-header");
  headerEl.setAttribute("data-search-results-header", "");
  headerEl.setAttribute("aria-live", "polite");

  const resultsList = doc.createElement("div");
  resultsList.setAttribute("class", "search-popover-results");
  resultsList.setAttribute("role", "listbox");
  resultsList.setAttribute("aria-label", "Результаты поиска");
  resultsList.setAttribute("data-search-popover-results", "");

  root.appendChild(barEl);
  root.appendChild(suggestionsList);
  root.appendChild(indexingBanner);
  root.appendChild(errorBanner);
  root.appendChild(headerEl);
  root.appendChild(resultsList);

  return {
    root,
    input,
    highlightBar,
    suggestionsList,
    indexingBanner,
    errorBanner,
    headerEl,
    resultsList,
  };
}

/**
 * Создаёт контроллер и DOM поискового попапа канала.
 * @param {any} containerEl
 * @param {SearchPopoverOptions} [options]
 */
export function createSearchPopover(containerEl, options = {}) {
  if (!containerEl || typeof containerEl !== "object") {
    throw new TypeError("containerEl must be a valid DOM container");
  }
  const doc = resolvePopoverDocument(containerEl);
  if (typeof containerEl.appendChild !== "function" && typeof containerEl.replaceChildren !== "function") {
    throw new TypeError("containerEl must support appendChild or replaceChildren");
  }

  let isOpen = true;
  let isIndexing = false;
  /** @type {{ status: number, retryAfterMs: number, message: string } | null} */
  let errorState = null;
  let query = String(options.initialQuery ?? "").slice(0, SEARCH_POPOVER_LIMITS.MAX_QUERY_LENGTH);
  /** @type {SearchPopoverResultItem[]} */
  let results = [];
  let totalResults = 0;
  let activeResultIndex = -1;
  let activeSuggestionIndex = 0;
  /** @type {string[]} */
  let recentQueries = [];

  const dom = buildPopoverDomSkeleton(doc, query);
  replacePopoverChildren(containerEl, [dom.root]);

  function getCurrentSuggestions() {
    return getFilterTokenCompletions(query, {
      users: options.users,
      channels: options.channels,
    });
  }

  function renderInputHighlight() {
    const segments = tokenizeSearchInputForHighlight(query);
    /** @type {any[]} */
    const nodes = [];
    for (const seg of segments) {
      if (seg.type === "token") {
        const mark = doc.createElement("mark");
        mark.setAttribute("class", "search-input-token");
        mark.setAttribute("data-active-token", String(seg.key ?? ""));
        mark.setAttribute("data-token-prefix", String(seg.prefix ?? ""));
        mark.setAttribute("data-token-value", String(seg.value ?? ""));

        const prefixSpan = doc.createElement("span");
        prefixSpan.setAttribute("class", "search-chip-prefix");
        prefixSpan.textContent = String(seg.prefix ?? "");

        const valueSpan = doc.createElement("span");
        valueSpan.setAttribute("class", "search-chip-value");
        valueSpan.textContent = String(seg.value ?? "");

        mark.appendChild(prefixSpan);
        mark.appendChild(valueSpan);
        nodes.push(mark);
      } else if (seg.text.trim().length > 0) {
        const textSpan = doc.createElement("span");
        textSpan.setAttribute("class", "search-input-free-text");
        textSpan.textContent = seg.text;
        nodes.push(textSpan);
      }
    }
    replacePopoverChildren(dom.highlightBar, nodes);
  }

  function renderSuggestions() {
    const items = getCurrentSuggestions();
    if (activeSuggestionIndex >= items.length) {
      activeSuggestionIndex = 0;
    }
    /** @type {any[]} */
    const nodes = [];
    items.forEach((item, idx) => {
      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("role", "option");
      btn.setAttribute("class", "search-popover-suggestion-item");
      btn.setAttribute("data-token-suggestion", item.token);
      btn.setAttribute("aria-selected", idx === activeSuggestionIndex ? "true" : "false");

      const tokenSpan = doc.createElement("span");
      tokenSpan.setAttribute("class", "search-suggestion-token");
      tokenSpan.textContent = item.token;

      const labelSpan = doc.createElement("span");
      labelSpan.setAttribute("class", "search-suggestion-label");
      labelSpan.textContent = item.labelRu;

      btn.appendChild(tokenSpan);
      btn.appendChild(labelSpan);
      btn.addEventListener("click", () => applyTokenSuggestion(item.token));
      nodes.push(btn);
    });
    replacePopoverChildren(dom.suggestionsList, nodes);
  }

  function renderResults() {
    dom.headerEl.textContent =
      totalResults > 0 ? `Найдено результатов: ${totalResults}` : "Нет результатов";
    dom.headerEl.setAttribute("data-total-results", String(totalResults));

    const nodes = results.map((item, idx) =>
      buildResultCardNode(doc, item, idx === activeResultIndex, query, triggerJump),
    );
    replacePopoverChildren(dom.resultsList, nodes);

    const activeItem =
      activeResultIndex >= 0 && activeResultIndex < results.length
        ? results[activeResultIndex]
        : null;
    if (activeItem) {
      dom.input.setAttribute("aria-activedescendant", `search-result-${String(activeItem.id ?? "")}`);
    } else {
      dom.input.removeAttribute("aria-activedescendant");
    }
  }

  /** @param {SearchPopoverResultItem} item */
  function triggerJump(item) {
    if (typeof options.onJumpToMessage === "function") {
      options.onJumpToMessage(String(item.id ?? ""), String(item.channelId ?? ""), item);
    }
  }

  function emitSearch() {
    const parsedTokens = parseSearchQueryTokens(query);
    const parsedQuery = buildSearchQuery({}, { content: query });
    if (typeof options.onSearch === "function") {
      options.onSearch({ query, freeText: parsedTokens.freeText, parsedQuery });
    }
  }

  /** @param {string} rawText */
  function recordRecentQuery(rawText) {
    const trimmed = String(rawText ?? "").trim();
    if (trimmed === "") return;
    recentQueries = [
      trimmed,
      ...recentQueries.filter((entry) => entry !== trimmed),
    ].slice(0, SEARCH_POPOVER_LIMITS.MAX_HISTORY);
  }

  /** @param {string} token */
  function applyTokenSuggestion(token) {
    const next = completeFilterToken(query, token);
    if (next === query) return;
    setQuery(next);
  }

  /** @param {string} nextQuery */
  function setQuery(nextQuery) {
    const raw = typeof nextQuery === "string" ? nextQuery : "";
    query = raw.slice(0, SEARCH_POPOVER_LIMITS.MAX_QUERY_LENGTH);
    dom.input.value = query;
    activeResultIndex = -1;
    if (!isOpen) open();
    renderInputHighlight();
    renderSuggestions();
    renderResults();
    emitSearch();
  }

  /** @param {any} payload */
  function setResults(payload) {
    if (Array.isArray(payload)) {
      results = payload.map((/** @type {SearchPopoverResultItem} */ item) => ({ ...item }));
      totalResults = results.length;
    } else if (payload && typeof payload === "object") {
      const list = Array.isArray(payload.results)
        ? payload.results
        : Array.isArray(payload.hits)
          ? payload.hits
          : [];
      results = list.map((/** @type {SearchPopoverResultItem} */ item) => ({ ...item }));
      const rawTotal = payload.totalResults ?? payload.totalCount;
      totalResults =
        typeof rawTotal === "number" && Number.isFinite(rawTotal) && rawTotal >= 0
          ? Math.trunc(rawTotal)
          : results.length;
    } else {
      results = [];
      totalResults = 0;
    }
    activeResultIndex = -1;
    renderResults();
  }

  /** @param {{ indexing?: boolean, retryAfterMs?: number }} [payload] */
  function setIndexing(payload = {}) {
    isIndexing = Boolean(payload?.indexing);
    dom.root.setAttribute("aria-busy", isIndexing ? "true" : "false");
    if (isIndexing) {
      const delaySec = Math.max(
        1,
        Math.round(Number(payload?.retryAfterMs ?? SEARCH_DEFAULT_RETRY_AFTER_MS) / 1000),
      );
      dom.indexingBanner.textContent = `Индексация сообщений канала… Повтор через ${delaySec} сек.`;
      dom.indexingBanner.removeAttribute("hidden");
    } else {
      dom.indexingBanner.textContent = "";
      dom.indexingBanner.setAttribute("hidden", "");
    }
  }

  /** @param {{ status?: number, retryAfterMs?: number, message?: string } | null} err */
  function setError(err) {
    if (!err || typeof err !== "object") {
      clearError();
      return;
    }
    errorState = {
      status: Number(err.status ?? 0),
      retryAfterMs: Number(err.retryAfterMs ?? 0),
      message: String(err.message ?? "Ошибка выполнения поиска"),
    };
    dom.errorBanner.textContent = errorState.message;
    dom.errorBanner.removeAttribute("hidden");
  }

  function clearError() {
    errorState = null;
    dom.errorBanner.textContent = "";
    dom.errorBanner.setAttribute("hidden", "");
  }

  function clearHistory() {
    recentQueries = [];
  }

  function open() {
    isOpen = true;
    dom.root.setAttribute("data-open", "true");
    dom.root.setAttribute("aria-hidden", "false");
  }

  function close() {
    isOpen = false;
    activeResultIndex = -1;
    dom.root.setAttribute("data-open", "false");
    dom.root.setAttribute("aria-hidden", "true");
    renderResults();
    if (typeof options.onClose === "function") {
      options.onClose();
    }
  }

  /** @param {1 | -1 | "home" | "end"} direction */
  function moveSelection(direction) {
    if (results.length > 0) {
      if (direction === "home") activeResultIndex = 0;
      else if (direction === "end") activeResultIndex = results.length - 1;
      else if (direction === 1) {
        activeResultIndex = activeResultIndex < 0 ? 0 : (activeResultIndex + 1) % results.length;
      } else {
        activeResultIndex = activeResultIndex <= 0 ? results.length - 1 : activeResultIndex - 1;
      }
      renderResults();
      return;
    }
    const suggestions = getCurrentSuggestions();
    if (suggestions.length === 0) return;
    if (direction === "home") activeSuggestionIndex = 0;
    else if (direction === "end") activeSuggestionIndex = suggestions.length - 1;
    else if (direction === 1) {
      activeSuggestionIndex = (activeSuggestionIndex + 1) % suggestions.length;
    } else {
      activeSuggestionIndex =
        activeSuggestionIndex <= 0 ? suggestions.length - 1 : activeSuggestionIndex - 1;
    }
    renderSuggestions();
  }

  /** @param {any} event */
  function handleKeyDown(event) {
    const key = event?.key;
    if (key === "Escape") {
      event?.preventDefault?.();
      close();
      return;
    }
    if (key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End") {
      event?.preventDefault?.();
      const dir =
        key === "Home" ? "home" : key === "End" ? "end" : key === "ArrowDown" ? 1 : -1;
      moveSelection(dir);
      return;
    }
    if (key === "Tab") {
      if (results.length === 0) {
        const chosen = getCurrentSuggestions()[activeSuggestionIndex];
        if (chosen) {
          event?.preventDefault?.();
          applyTokenSuggestion(chosen.token);
        }
      }
      return;
    }
    if (key === "Enter") {
      if (activeResultIndex >= 0 && activeResultIndex < results.length) {
        event?.preventDefault?.();
        const activeHit = results[activeResultIndex];
        if (activeHit) triggerJump(activeHit);
        return;
      }
      const suggestions = getCurrentSuggestions();
      const chosen = suggestions[activeSuggestionIndex];
      const hasIncompleteTrailing = query.trim().length > 0 && !/\s$/.test(query);
      if (results.length === 0 && hasIncompleteTrailing && chosen) {
        event?.preventDefault?.();
        applyTokenSuggestion(chosen.token);
        return;
      }
      recordRecentQuery(query);
      emitSearch();
    }
  }

  dom.input.addEventListener("input", (/** @type {any} */ event) => {
    const nextVal = event?.target?.value ?? dom.input.value ?? "";
    setQuery(String(nextVal));
  });
  dom.input.addEventListener("keydown", (/** @type {any} */ event) => {
    handleKeyDown(event);
  });

  renderInputHighlight();
  renderSuggestions();
  renderResults();

  return {
    root: dom.root,
    input: dom.input,
    setQuery,
    setResults,
    setIndexing,
    setError,
    clearError,
    clearHistory,
    applyTokenSuggestion,
    handleKeyDown,
    open,
    close,
    getState() {
      const parsed = parseSearchQueryTokens(query);
      return {
        isOpen,
        isIndexing,
        error: errorState ? { ...errorState } : null,
        query,
        freeText: parsed.freeText,
        activeTokens: tokenizeSearchInputForHighlight(query).filter((s) => s.type === "token"),
        suggestions: [...getCurrentSuggestions()],
        results: results.map((r) => ({ ...r })),
        totalResults,
        activeResultIndex,
        activeSuggestionIndex,
        recentQueries: [...recentQueries],
      };
    },
  };
}

export const createSearchPopoverController = createSearchPopover;
