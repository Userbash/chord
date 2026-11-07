// @ts-check
/**
 * T-204: контроллер панели поиска сообщений и парсер инлайн-токенов фильтрации.
 *
 * RE-оракул:
 * - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@7564745` (`SearchStore` / `queryKey`:
 *   `from:` → `author_id`, `in:` → `channel_id`, `has:` → `has`, `mentions:` → `mentions`,
 *   `pinned:` → `pinned`, свободный текст → `content`);
 * - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@318812` (шаг пагинации `T_y = 25`,
 *   потолок 400 страниц → `MAX_SEARCH_OFFSET = 9975`);
 * - `/home/sanya/re-deep/frontend-d03.md` §1.1–§1.3 (панель результатов, фильтры, сортировка).
 *
 * @module ui/features/search-panel
 */

/** Шаг пагинации поисковой выдачи (25 сообщений на страницу). */
export const SEARCH_PAGE_STEP = 25;

/** Нижняя граница смещения пагинации. */
export const MIN_SEARCH_OFFSET = 0;

/** Верхняя граница смещения пагинации (400 страниц по 25 = 10 000, последний offset = 9975). */
export const MAX_SEARCH_OFFSET = 9975;

/** Допустимые значения фильтра `has:`. */
export const ALLOWED_HAS_TOKENS = Object.freeze(
  /** @type {const} */ (["link", "embed", "file", "image", "video"]),
);

/**
 * Русские подсказки фильтров поиска.
 * @type {ReadonlyArray<Readonly<{ token: string, labelRu: string }>>}
 */
export const SEARCH_FILTER_SUGGESTIONS_RU = Object.freeze([
  Object.freeze({ token: "from:", labelRu: "от: пользователь" }),
  Object.freeze({ token: "in:", labelRu: "в: канал" }),
  Object.freeze({ token: "has:link", labelRu: "содержит: ссылку" }),
  Object.freeze({ token: "has:file", labelRu: "содержит: файл" }),
  Object.freeze({ token: "has:image", labelRu: "содержит: изображение" }),
  Object.freeze({ token: "pinned:true", labelRu: "закреплено: да" }),
]);

/**
 * Русские вкладки сортировки результатов поиска.
 * @type {ReadonlyArray<Readonly<{ id: SearchSortMode, labelRu: string }>>}
 */
export const SEARCH_SORT_TABS_RU = Object.freeze([
  Object.freeze({ id: /** @type {const} */ ("newest"), labelRu: "Новые" }),
  Object.freeze({ id: /** @type {const} */ ("oldest"), labelRu: "Старые" }),
  Object.freeze({ id: /** @type {const} */ ("relevant"), labelRu: "Релевантные" }),
]);

/**
 * @typedef {"newest" | "oldest" | "relevant"} SearchSortMode
 */

/**
 * @typedef {object} ParsedSearchTokens
 * @property {string} content
 * @property {string[]} author
 * @property {string[]} channel
 * @property {string[]} has
 * @property {string[]} mentions
 * @property {boolean | null} pinned
 */

/**
 * @typedef {object} SearchResultHit
 * @property {string} id
 * @property {string} channelId
 * @property {string} [author]
 * @property {string} [content]
 */

/**
 * @typedef {object} SearchRequestPayload
 * @property {string} query
 * @property {string} content
 * @property {string[]} author
 * @property {string[]} channel
 * @property {string[]} has
 * @property {string[]} mentions
 * @property {boolean | null} pinned
 * @property {SearchSortMode} sortBy
 * @property {number} offset
 * @property {number} limit
 */

/**
 * @typedef {object} SearchPanelOptions
 * @property {(payload: SearchRequestPayload) => void} [onSearch]
 * @property {(messageId: string, channelId: string) => void} [onJumpToMessage]
 */

/**
 * Разбирает поисковую строку на фильтр-токены (`from:<user>`, `in:<channel>`,
 * `has:<link|embed|file|image|video>`, `mentions:<user>`, `pinned:<true|false>`)
 * и оставшийся свободный текст `content`.
 *
 * @param {string} rawQuery
 * @returns {ParsedSearchTokens}
 */
export function parseSearchQueryTokens(rawQuery) {
  /** @type {string[]} */
  const author = [];
  /** @type {string[]} */
  const channel = [];
  /** @type {string[]} */
  const has = [];
  /** @type {string[]} */
  const mentions = [];
  /** @type {boolean | null} */
  let pinned = null;
  /** @type {string[]} */
  const contentParts = [];

  if (typeof rawQuery !== "string") {
    return { content: "", author, channel, has, mentions, pinned };
  }

  const trimmed = rawQuery.trim();
  if (trimmed.length === 0) {
    return { content: "", author, channel, has, mentions, pinned };
  }

  const parts = trimmed.split(/\s+/);
  for (const part of parts) {
    const colonIdx = part.indexOf(":");
    if (colonIdx <= 0) {
      contentParts.push(part);
      continue;
    }

    const prefix = part.slice(0, colonIdx).toLowerCase();
    const rawValue = part.slice(colonIdx + 1);
    if (rawValue.length === 0) {
      contentParts.push(part);
      continue;
    }

    if (prefix === "from") {
      author.push(rawValue);
      continue;
    }

    if (prefix === "in") {
      channel.push(rawValue);
      continue;
    }

    if (prefix === "mentions") {
      mentions.push(rawValue);
      continue;
    }

    if (prefix === "has") {
      const normalizedHas = rawValue.toLowerCase();
      if (ALLOWED_HAS_TOKENS.includes(/** @type {any} */ (normalizedHas))) {
        has.push(normalizedHas);
      } else {
        contentParts.push(part);
      }
      continue;
    }

    if (prefix === "pinned") {
      const normalizedPinned = rawValue.toLowerCase();
      if (normalizedPinned === "true") {
        pinned = true;
      } else if (normalizedPinned === "false") {
        pinned = false;
      } else {
        contentParts.push(part);
      }
      continue;
    }

    contentParts.push(part);
  }

  return {
    content: contentParts.join(" "),
    author,
    channel,
    has,
    mentions,
    pinned,
  };
}

/**
 * Зажимает смещение пагинации в диапазон `0..=9975`.
 *
 * @param {number} offset
 * @returns {number}
 */
export function clampSearchOffset(offset) {
  const num = Number(offset);
  if (!Number.isFinite(num) || num <= MIN_SEARCH_OFFSET) {
    return MIN_SEARCH_OFFSET;
  }
  const truncated = Math.trunc(num);
  if (truncated >= MAX_SEARCH_OFFSET) {
    return MAX_SEARCH_OFFSET;
  }
  return truncated;
}

/**
 * Нормализует идентификатор вкладки сортировки.
 *
 * @param {unknown} value
 * @returns {SearchSortMode}
 */
function normalizeSortMode(value) {
  if (value === "oldest" || value === "relevant" || value === "newest") {
    return value;
  }
  return "newest";
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
 * Заменяет дочерние узлы контейнера на переданный массив элементов.
 *
 * @param {any} parentEl
 * @param {any[]} nodes
 */
function replaceContainerChildren(parentEl, nodes) {
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
 * Создаёт контроллер панели поиска сообщений внутри `containerEl`.
 *
 * @param {any} containerEl
 * @param {SearchPanelOptions} [options]
 */
export function createSearchPanelController(containerEl, options = {}) {
  const doc = resolveDocument(containerEl);

  /** @type {string} */
  let query = "";
  /** @type {SearchSortMode} */
  let sortBy = "newest";
  /** @type {number} */
  let offset = 0;
  /** @type {number} */
  let totalCount = 0;
  /** @type {SearchResultHit[]} */
  let hits = [];

  const root = doc.createElement("section");
  root.setAttribute("data-search-panel", "");

  const headerEl = doc.createElement("header");
  headerEl.setAttribute("data-search-header", "");

  const searchInput = doc.createElement("input");
  searchInput.setAttribute("type", "search");
  searchInput.setAttribute("data-search-input", "");
  searchInput.setAttribute("placeholder", "Поиск сообщений...");
  searchInput.setAttribute("aria-label", "Поиск сообщений");
  searchInput.addEventListener("input", (/** @type {any} */ event) => {
    const nextVal = event?.target?.value ?? searchInput.value ?? "";
    query = String(nextVal);
    offset = 0;
    renderPaginationState();
    emitSearch();
  });
  headerEl.appendChild(searchInput);

  const suggestionsBar = doc.createElement("div");
  suggestionsBar.setAttribute("data-search-suggestions", "");
  for (const item of SEARCH_FILTER_SUGGESTIONS_RU) {
    const chip = doc.createElement("button");
    chip.setAttribute("type", "button");
    chip.setAttribute("data-suggestion-token", item.token);
    chip.textContent = item.labelRu;
    chip.addEventListener("click", () => {
      const nextQuery = query.trim().length > 0 ? `${query.trim()} ${item.token}` : item.token;
      setQuery(nextQuery);
    });
    suggestionsBar.appendChild(chip);
  }
  headerEl.appendChild(suggestionsBar);

  const sortTabList = doc.createElement("div");
  sortTabList.setAttribute("role", "tablist");
  sortTabList.setAttribute("data-search-sort-tabs", "");
  /** @type {Map<SearchSortMode, any>} */
  const sortTabButtons = new Map();

  for (const tab of SEARCH_SORT_TABS_RU) {
    const btn = doc.createElement("button");
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "tab");
    btn.setAttribute("data-sort-tab", tab.id);
    btn.setAttribute("aria-selected", tab.id === sortBy ? "true" : "false");
    btn.textContent = tab.labelRu;
    btn.addEventListener("click", () => {
      setSort(tab.id);
    });
    sortTabButtons.set(tab.id, btn);
    sortTabList.appendChild(btn);
  }
  headerEl.appendChild(sortTabList);

  const totalHeader = doc.createElement("div");
  totalHeader.setAttribute("data-search-total", "");
  totalHeader.setAttribute("aria-live", "polite");
  headerEl.appendChild(totalHeader);

  const resultsList = doc.createElement("div");
  resultsList.setAttribute("data-search-results", "");

  const paginationEl = doc.createElement("nav");
  paginationEl.setAttribute("data-search-pagination", "");

  const prevBtn = doc.createElement("button");
  prevBtn.setAttribute("type", "button");
  prevBtn.setAttribute("data-page-prev", "");
  prevBtn.textContent = "Назад";
  prevBtn.addEventListener("click", () => {
    prevPage();
  });

  const offsetLabel = doc.createElement("span");
  offsetLabel.setAttribute("data-page-offset", "");

  const nextBtn = doc.createElement("button");
  nextBtn.setAttribute("type", "button");
  nextBtn.setAttribute("data-page-next", "");
  nextBtn.textContent = "Вперёд";
  nextBtn.addEventListener("click", () => {
    nextPage();
  });

  paginationEl.appendChild(prevBtn);
  paginationEl.appendChild(offsetLabel);
  paginationEl.appendChild(nextBtn);

  root.appendChild(headerEl);
  root.appendChild(resultsList);
  root.appendChild(paginationEl);
  replaceContainerChildren(containerEl, [root]);

  function renderSortTabsState() {
    for (const tab of SEARCH_SORT_TABS_RU) {
      const btn = sortTabButtons.get(tab.id);
      if (btn) {
        btn.setAttribute("aria-selected", tab.id === sortBy ? "true" : "false");
      }
    }
  }

  function renderPaginationState() {
    offsetLabel.setAttribute("data-offset", String(offset));
    offsetLabel.textContent = String(offset);
    prevBtn.setAttribute("aria-disabled", offset <= MIN_SEARCH_OFFSET ? "true" : "false");
    nextBtn.setAttribute("aria-disabled", offset >= MAX_SEARCH_OFFSET ? "true" : "false");
  }

  function renderResultsList() {
    if (totalCount > 0) {
      totalHeader.textContent = `Найдено результатов: ${totalCount}`;
    } else {
      totalHeader.textContent = "Ничего не найдено";
    }
    totalHeader.setAttribute("data-total-count", String(totalCount));

    /** @type {any[]} */
    const cardNodes = [];
    for (const hit of hits) {
      const messageId = String(hit.id ?? "");
      const channelId = String(hit.channelId ?? "");

      const card = doc.createElement("article");
      card.setAttribute("data-search-hit", messageId);
      card.setAttribute("data-channel-id", channelId);

      const authorEl = doc.createElement("span");
      authorEl.setAttribute("data-hit-author", "");
      authorEl.textContent = typeof hit.author === "string" ? hit.author : "";
      card.appendChild(authorEl);

      const contentEl = doc.createElement("p");
      contentEl.setAttribute("data-hit-content", "");
      contentEl.textContent = typeof hit.content === "string" ? hit.content : "";
      card.appendChild(contentEl);

      const jumpBtn = doc.createElement("button");
      jumpBtn.setAttribute("type", "button");
      jumpBtn.setAttribute("data-action", "jump-to-message");
      jumpBtn.setAttribute("data-jump-message-id", messageId);
      jumpBtn.setAttribute("data-jump-channel-id", channelId);
      jumpBtn.textContent = "Перейти";
      jumpBtn.addEventListener("click", () => {
        if (typeof options.onJumpToMessage === "function") {
          options.onJumpToMessage(messageId, channelId);
        }
      });
      card.appendChild(jumpBtn);

      cardNodes.push(card);
    }

    replaceContainerChildren(resultsList, cardNodes);
  }

  /**
   * Формирует поисковый запрос и вызывает `options.onSearch`.
   *
   * @returns {SearchRequestPayload}
   */
  function emitSearch() {
    const parsed = parseSearchQueryTokens(query);
    /** @type {SearchRequestPayload} */
    const payload = {
      query,
      content: parsed.content,
      author: parsed.author,
      channel: parsed.channel,
      has: parsed.has,
      mentions: parsed.mentions,
      pinned: parsed.pinned,
      sortBy,
      offset,
      limit: SEARCH_PAGE_STEP,
    };
    if (typeof options.onSearch === "function") {
      options.onSearch(payload);
    }
    return payload;
  }

  /**
   * Устанавливает строку поиска, сбрасывает `offset` в `0` и запускает поиск.
   *
   * @param {string} nextQuery
   * @returns {SearchRequestPayload}
   */
  function setQuery(nextQuery) {
    query = typeof nextQuery === "string" ? nextQuery : "";
    searchInput.value = query;
    offset = 0;
    renderPaginationState();
    return emitSearch();
  }

  /**
   * Меняет режим сортировки (`newest` | `oldest` | `relevant`) и запускает поиск.
   *
   * @param {SearchSortMode} nextSort
   * @returns {SearchRequestPayload}
   */
  function setSort(nextSort) {
    sortBy = normalizeSortMode(nextSort);
    offset = 0;
    renderSortTabsState();
    renderPaginationState();
    return emitSearch();
  }

  /**
   * Устанавливает смещение пагинации с зажимом в `0..=9975` и запускает поиск.
   *
   * @param {number} nextOffset
   * @returns {number}
   */
  function setOffset(nextOffset) {
    offset = clampSearchOffset(nextOffset);
    renderPaginationState();
    emitSearch();
    return offset;
  }

  /**
   * Переходит на следующую страницу (`+25`, максимум `9975`).
   *
   * @returns {number}
   */
  function nextPage() {
    return setOffset(offset + SEARCH_PAGE_STEP);
  }

  /**
   * Переходит на предыдущую страницу (`-25`, минимум `0`).
   *
   * @returns {number}
   */
  function prevPage() {
    return setOffset(offset - SEARCH_PAGE_STEP);
  }

  /**
   * Обновляет список найденных сообщений и общее число результатов.
   *
   * @param {{ hits?: SearchResultHit[], totalCount?: number }} payload
   */
  function setResults(payload = {}) {
    hits = Array.isArray(payload.hits) ? [...payload.hits] : [];
    const rawTotal = payload.totalCount;
    totalCount =
      typeof rawTotal === "number" && Number.isFinite(rawTotal) && rawTotal >= 0
        ? Math.trunc(rawTotal)
        : hits.length;
    renderResultsList();
  }

  renderSortTabsState();
  renderPaginationState();
  renderResultsList();

  return {
    root,
    setQuery,
    setSort,
    setOffset,
    nextPage,
    prevPage,
    setResults,
    getState() {
      return {
        query,
        sortBy,
        offset,
        totalCount,
        hits: [...hits],
      };
    },
  };
}
