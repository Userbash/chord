// @ts-check
/**
 * Search overlay (SPEC-P3-02 §6, SRCH-01…SRCH-18; R-10/R-11/R-18).
 *
 * 7 секций фильтров (R-10: 5 input, 9 опций «Содержит»), меню сортировки из
 * 3 пунктов (R-11), результаты срезом по 6 (SRCH-15) и **честные** счётчики.
 *
 * B7b: если ядро отдаёт `search_remote` и известен guildId, запрос уходит на
 * сервер (`content`, `channelId`, `sortBy`, `sortOrder`, `offset`), статус
 * показывает `total_results`, а «Показать ещё» листает `offset` без дублей.
 * Локальный скан загруженного окна (ADR-1, ≤500 строк) остаётся fallback'ом
 * только для демо/недоступной IPC-команды; ошибки `SearchError` (401/403/404,
 * прочие 4xx, 429, 5xx, сеть) — видимая ошибка, а не тихий локальный ответ.
 * STUB-14: все 7 фильтров разбираются и нормализуются в подтверждённые
 * `queryKey` (`author_id`, `mentions`, `has`, `channel_id`, `pinned`,
 * `author_type`, `min_id`/`max_id`, RE-DEEP-REA §2.3–2.4) и уходят в
 * `params.query`; принятый backend B7a читает пока 5 полей, поэтому выдача
 * дополнительно фильтруется локально и секции честно помечены «локально».
 *
 * Панель не модальная (SRCH-02): чат остаётся доступен, Esc/крестик закрывают.
 */
import { byId, formatNumber } from "./dom.js";
import { UIError, asUIError, reportError } from "./errors.js";
import { HAS_OPTIONS, extrasFor } from "./rows.js";
import {
  ADULT_VERIFICATION_STATUSES,
  AGE_GATED_SPACES,
  GUILD_NSFW_AGREE_STORAGE_KEY,
  MAX_SEARCH_OFFSET,
  MAX_SEARCH_PAGES,
  SEARCH_AUTHOR_TYPES,
  SEARCH_DEFAULT_RETRY_AFTER_MS,
  SEARCH_MAX_INDEXING_ATTEMPTS,
  SEARCH_PAGE_SIZE,
  applyIncludeNsfw,
  buildSearchQuery,
  computeIndexingRetryDelayMs,
  didAgreeNsfwGuild,
  formatHistoricalIndexedStatus,
  formatMaxPagesWarning,
  isAgeGatedSpacesRestricted,
  isCalendarDate,
  normalizeHasFilter,
  parseSearchQueryTokens,
  splitQueryValues,
} from "./search.js";

export {
  ADULT_VERIFICATION_STATUSES,
  AGE_GATED_SPACES,
  GUILD_NSFW_AGREE_STORAGE_KEY,
  MAX_SEARCH_OFFSET,
  MAX_SEARCH_PAGES,
  SEARCH_AUTHOR_TYPES,
  SEARCH_DEFAULT_RETRY_AFTER_MS,
  SEARCH_MAX_INDEXING_ATTEMPTS,
  applyIncludeNsfw,
  computeIndexingRetryDelayMs,
  didAgreeNsfwGuild,
  formatHistoricalIndexedStatus,
  formatMaxPagesWarning,
  isAgeGatedSpacesRestricted,
  parseSearchQueryTokens,
};

/** R-10: 7 секций фильтров, порядок как в дампе. */
export const SEARCH_SECTIONS = Object.freeze([
  { key: "from", title: "От", control: "text", placeholder: "Имя автора" },
  { key: "in", title: "В", control: "text", placeholder: "Канал" },
  { key: "has", title: "Содержит", control: "text", placeholder: "Тип содержимого" },
  { key: "mentions", title: "Упоминания", control: "text", placeholder: "Пользователь" },
  { key: "date", title: "Дата", control: "date", placeholder: "" },
  { key: "author", title: "Тип автора", control: "select", options: ["Любой", "Человек", "Бот", "Вебхук"] },
  { key: "pinned", title: "Закреплено", control: "switch" },
]);

/** R-11: 3 пункта сортировки, «По новизне» выбрана в дампе. */
export const SEARCH_SORTS = Object.freeze([
  { key: "new", label: "По новизне" },
  { key: "old", label: "Самое старое" },
  { key: "recent", label: "Самое актуальное" },
]);

/** SRCH-15: 6 результатов на срез. */
export const SEARCH_SLICE = 6;

/** Демо-канал: всё окно — «новости» (фильтр «В» сверяется с ним честно). */
export const DEMO_CHANNEL = "новости";
/** Демо-дата сообщений (makeMessage): 2026-10-23. */
export const DEMO_DAY = "2026-10-23";

/**
 * B7b/ATTACH-RE §1: подтверждённые серверные параметры на каждый пункт R-11.
 * «Самое актуальное» на сервере — `desc`; закреплённые поднимаются уже над
 * полученной страницей (в REST-параметрах закрепления нет — gap).
 * @type {Record<string, { sortBy: string, sortOrder: "asc" | "desc" }>}
 */
export const REMOTE_SORTS = Object.freeze({
  new: { sortBy: "timestamp", sortOrder: "desc" },
  old: { sortBy: "timestamp", sortOrder: "asc" },
  recent: { sortBy: "timestamp", sortOrder: "desc" },
});

/**
 * STUB-14: фильтры, которые уже нормализуются в подтверждённые `queryKey`, но
 * принятый backend B7a их пока не читает — применяются к серверной выдаче
 * локально и честно помечены «локально» (RE-DEEP-REA §2.3).
 */
export const LOCAL_FILTER_KEYS = Object.freeze(["from", "has", "mentions", "date", "author", "pinned"]);

/** `SearchError.kind` из `gui-spike/src/search.rs` (serde tag = kind). */
const SEARCH_ERROR_KINDS = Object.freeze([
  "invalidArg",
  "missingToken",
  "unauthorized",
  "forbidden",
  "notFound",
  "rateLimited",
  "client",
  "server",
  "network",
  "storage",
  "invalidResponse",
]);

/** Подписи чипов активных фильтров (SEARCH-202-NSFW-SPEC §5.3). */
const CHIP_LABELS = Object.freeze({
  from: "от",
  in: "в",
  has: "содержит",
  mentions: "упоминание",
  before: "до",
  after: "после",
  date: "дата",
  author: "тип автора",
  pinned: "закреплено",
});

/**
 * Проверка ответа HTTP 202 / индексации сервера (SEARCH-202-NSFW-RE §1).
 * @param {any} response
 * @returns {boolean}
 */
function isIndexingResponse(response) {
  if (!response || typeof response !== "object") return false;
  return response.status === 202 || response.indexing === true || response.code === 110000;
}

/**
 * @typedef {{
 *   from: string, in: string, has: string, mentions: string, date: string,
 *   author: string, pinned: boolean,
 * }} Filters
 */

/**
 * @param {string} className
 * @param {string} [text]
 * @returns {HTMLElement}
 */
function node(className, text = "") {
  const element = document.createElement("span");
  element.className = className;
  if (text) element.textContent = text;
  return element;
}

/**
 * @param {string} value
 * @returns {boolean}
 */
function isIsoDate(value) {
  // P2: `Date.parse` нормализует 2026-02-31 в март — нужна сверка компонентов.
  return isCalendarDate(value);
}

/**
 * Сообщение, пришедшее с сервера (после нормализации `messages`).
 * @typedef {{
 *   key: string, id: string, author: string, authorId: string, mentionIds: string[],
 *   text: string, timestamp: string, date: string, time: string, channel: string,
 *   pinned: boolean, bot: boolean, raw: any,
 * }} RemoteMessage
 */

/**
 * Выбранный сервер из рейла. Источник — `data-guild-id` (G-01: плитки строит
 * `app.js` из `ready()`); legacy `data-guild` берётся из dataset только если
 * он там реально есть (DOM-шим), а не выдумывается.
 * @returns {string}
 */
export function selectedGuildId() {
  const node = /** @type {HTMLElement | null} */ (
    document.querySelector(".guild-item.is-selected")
  );
  const current = node?.getAttribute("data-guild-id");
  if (typeof current === "string" && current !== "") return current;
  // Legacy-шим держит `data-guild` только в dataset (без атрибута).
  const legacy = node?.dataset?.guild;
  return typeof legacy === "string" ? legacy : "";
}

/**
 * Реальная ошибка бэкенда (`SearchError`) отличима от «команда не
 * зарегистрирована»: у первой есть `kind` из закрытого списка, у второй —
 * строка/иное исключение Tauri. Только второй случай разрешает fallback.
 * @param {unknown} error
 * @returns {boolean}
 */
function isSearchError(error) {
  const kind = /** @type {any} */ (error)?.kind;
  return typeof kind === "string" && SEARCH_ERROR_KINDS.includes(kind);
}

/**
 * HTTP-статус из `SearchError.client/server`; нет числа — честный диапазон.
 * @param {any} error
 * @param {string} fallback
 * @returns {string}
 */
function httpStatusCode(error, fallback) {
  const status = Number(error?.status);
  return Number.isInteger(status) && status > 0 ? String(status) : fallback;
}

/**
 * @param {any} error
 * @returns {string}
 */
function remoteErrorText(error) {
  switch (error?.kind) {
    case "unauthorized":
      return "сессия истекла (401) — войдите заново";
    case "forbidden":
      return "нет доступа (403)";
    case "notFound":
      return "не найдено (404)";
    case "rateLimited": {
      const retry = Number(error?.retryAfterMs ?? 0);
      return `слишком много запросов (429), повтор через ${Number.isFinite(retry) ? retry : 0} мс`;
    }
    case "client":
      // Прочие 4xx: статус из ответа не теряем, без него — «4xx».
      return `ошибка запроса (${httpStatusCode(error, "4xx")})`;
    case "server":
      return `ошибка сервера (${httpStatusCode(error, "5xx")})`;
    case "network":
      return "сеть недоступна";
    case "storage":
      // SEC-02: keyring/blocking недоступны — чинить нужно хранилище, не сеть.
      return "хранилище токена недоступно";
    case "missingToken":
      return "нет сохранённого токена — войдите заново";
    case "invalidArg":
      return "некорректные параметры поиска";
    case "invalidResponse":
      return "некорректный ответ сервера поиска";
    default:
      return "неизвестная ошибка серверного поиска";
  }
}

/**
 * Discord отдаёт `messages` как массив сообщений или как массив групп
 * (массив массивов); разворачиваем ровно один уровень.
 * @param {any} value
 * @returns {any[]}
 */
function flattenServerMessages(value) {
  if (!Array.isArray(value)) return [];
  /** @type {any[]} */
  const flat = [];
  for (const entry of value) {
    if (Array.isArray(entry)) {
      for (const item of entry) flat.push(item);
    } else {
      flat.push(entry);
    }
  }
  return flat;
}

/**
 * @param {any} value
 * @returns {RemoteMessage}
 */
function normalizeRemoteMessage(value) {
  const message = /** @type {any} */ (value ?? {});
  const author = /** @type {any} */ (message.author ?? {});
  const id = String(message.id ?? "");
  const timestamp = String(message.timestamp ?? "");
  const authorName = String(author.username ?? author.global_name ?? author.globalName ?? "автор");
  const authorId = String(author.id ?? message.author_id ?? message.authorId ?? "");
  const mentions = /** @type {any[]} */ (Array.isArray(message.mentions) ? message.mentions : []);
  const text = String(message.content ?? "");
  return {
    key: id !== "" ? id : `${timestamp}|${authorName}|${text}`,
    id,
    author: authorName,
    authorId,
    mentionIds: mentions
      .map((mention) => String(mention?.id ?? mention ?? ""))
      .filter((mentionId) => mentionId !== ""),
    text,
    timestamp,
    date: timestamp.slice(0, 10),
    time: timestamp.length >= 16 ? timestamp.slice(11, 16) : "",
    channel: String(message.channel_id ?? message.channelId ?? ""),
    pinned: message.pinned === true,
    bot: author.bot === true,
    raw: message,
  };
}

/**
 * P1: значение фильтра «От»/«Упоминания» — это имя или числовой snowflake.
 * Сравнение только с отображаемым именем ложно отбрасывало серверное
 * сообщение, уже отфильтрованное по `author_id`/`mentions` на сервере.
 * @param {string} filter
 * @param {string[]} ids
 * @param {string[]} names
 * @returns {boolean}
 */
function matchesFilterValue(filter, ids, names) {
  const values = splitQueryValues(filter);
  if (values.length === 0) return true;
  return values.some((value) => {
    if (ids.includes(value)) return true;
    const needle = value.toLowerCase();
    return names.some((name) => name.toLowerCase().includes(needle));
  });
}

/**
 * @param {{
 *   chat: ReturnType<typeof import("./chat.js").createChatScreen>,
 *   header?: HTMLInputElement | null,
 *   container?: HTMLElement | null,
 *   onError?: (error: UIError) => void,
 *   invoke?: ((cmd: string, args: any) => Promise<any>) | null,
 *   guildId?: string | null,
 *   getGuildId?: (() => string) | null,
 * }} options
 */
export function createSearchOverlay(options) {
  const { chat } = options;
  const onError = options.onError ?? (() => {});
  const invoke = typeof options.invoke === "function" ? options.invoke : null;
  // QA-M2-03: на boot рейл бывает пуст, поэтому guildId возможен как getter —
  // тогда он читается заново на каждый запрос, а не фиксируется один раз.
  const fixedGuildId = options.guildId === undefined || options.guildId === null
    ? null
    : String(options.guildId);
  const readGuildId = typeof options.getGuildId === "function"
    ? () => String(options.getGuildId?.() ?? "")
    : fixedGuildId === null
      ? selectedGuildId
      : () => fixedGuildId;
  const container = options.container
    ?? /** @type {HTMLElement | null} */ (byId("screen-chat"));
  const header = options.header ?? /** @type {HTMLInputElement | null} */ (byId("chat-search"));
  if (!container) {
    throw new UIError("ELEMENT_MISSING", "search overlay: нет #screen-chat");
  }

  const panel = document.createElement("aside");
  panel.className = "search-overlay";
  panel.setAttribute("role", "search");
  panel.setAttribute("aria-label", "Поиск сообщений");
  panel.setAttribute("aria-busy", "false");
  panel.setAttribute("data-open", "false");
  panel.setAttribute("aria-hidden", "true");

  /* ---------- шапка панели ---------- */
  const head = document.createElement("header");
  head.className = "search-overlay-head";
  const input = document.createElement("input");
  input.className = "search-overlay-input search-input";
  input.setAttribute("type", "search");
  input.setAttribute("aria-label", "Запрос поиска сообщений");
  input.setAttribute("placeholder", "Поиск");
  const closeButton = document.createElement("button");
  closeButton.className = "icon-btn search-close";
  closeButton.setAttribute("type", "button");
  closeButton.setAttribute("aria-label", "Закрыть поиск");
  closeButton.textContent = "✕";
  head.append(input, closeButton);

  /* ---------- 7 секций ---------- */
  const sections = document.createElement("div");
  sections.className = "search-sections";
  /** @type {Map<string, HTMLElement>} */
  const controls = new Map();
  for (const section of SEARCH_SECTIONS) {
    const box = document.createElement("section");
    box.className = "search-section";
    box.setAttribute("data-key", section.key);
    // B7b: «В» уходит на сервер как channel_id; остальные 6 фильтров
    // применяются к загруженным результатам и честно помечаются «локально».
    const local = LOCAL_FILTER_KEYS.includes(section.key);
    box.setAttribute("data-scope", local ? "local" : "server");
    const title = document.createElement("h3");
    title.className = "search-section-title";
    title.textContent = section.title;
    box.append(title);

    if (section.control === "text" || section.control === "date") {
      const field = document.createElement("input");
      field.className = "search-control";
      field.setAttribute("type", section.control === "date" ? "date" : "text");
      field.setAttribute("aria-label", section.title);
      if (section.placeholder) field.setAttribute("placeholder", section.placeholder);
      field.addEventListener("input", () => reset());
      controls.set(section.key, field);
      box.append(field);
    } else if (section.control === "select") {
      const select = document.createElement("select");
      select.className = "search-control";
      select.setAttribute("aria-label", section.title);
      for (const label of section.options ?? []) {
        const option = document.createElement("option");
        option.setAttribute("value", label);
        option.textContent = label;
        select.append(option);
      }
      select.addEventListener("change", () => reset());
      controls.set(section.key, select);
      box.append(select);
    } else {
      const toggle = document.createElement("button");
      toggle.className = "search-control search-switch";
      toggle.setAttribute("type", "button");
      toggle.setAttribute("role", "switch");
      toggle.setAttribute("aria-checked", "false");
      toggle.setAttribute("aria-label", section.title);
      toggle.textContent = "Выкл";
      toggle.addEventListener("click", () => {
        const next = toggle.getAttribute("aria-checked") !== "true";
        toggle.setAttribute("aria-checked", next ? "true" : "false");
        toggle.textContent = next ? "Вкл" : "Выкл";
        reset();
      });
      controls.set(section.key, toggle);
      box.append(toggle);
    }

    if (section.key === "has") {
      // R-10: 9 опций «Содержит»; тексты — из данных (G-05), не хардкод в разметке.
      const list = document.createElement("div");
      list.className = "search-options";
      for (const option of HAS_OPTIONS) {
        const chip = document.createElement("button");
        chip.className = "search-option";
        chip.setAttribute("type", "button");
        chip.setAttribute("aria-label", `Содержит: ${option}`);
        chip.textContent = option;
        chip.addEventListener("click", () => {
          const control = field("has");
          if (control) control.value = option;
          reset();
        });
        list.append(chip);
      }
      box.append(list);
    }
    if (local) {
      // Видимая, а не только в title, честность: сервер этот фильтр не получает.
      const note = document.createElement("span");
      note.className = "search-section-note";
      note.textContent = "локально";
      note.setAttribute("title", "Фильтр применяется к уже загруженным результатам");
      box.append(note);
    }
    sections.append(box);
  }

  /* ---------- сортировка (R-11) ---------- */
  const sortBox = document.createElement("div");
  sortBox.className = "search-sort";
  sortBox.setAttribute("data-scope", "server");
  const trigger = document.createElement("button");
  trigger.className = "search-sort-trigger";
  trigger.setAttribute("type", "button");
  trigger.setAttribute("aria-haspopup", "menu");
  trigger.setAttribute("aria-expanded", "false");
  trigger.textContent = SEARCH_SORTS[0]?.label ?? "";
  const menu = document.createElement("div");
  menu.className = "search-sort-menu";
  menu.setAttribute("role", "menu");
  menu.setAttribute("aria-label", "Сортировка результатов");
  menu.setAttribute("data-open", "false");
  /** @type {HTMLElement[]} */
  const items = [];
  for (const sort of SEARCH_SORTS) {
    const item = document.createElement("button");
    item.className = "search-sort-item";
    item.setAttribute("type", "button");
    item.setAttribute("role", "menuitemradio");
    item.setAttribute("aria-checked", sort.key === "new" ? "true" : "false");
    item.setAttribute("tabindex", sort.key === "new" ? "0" : "-1");
    item.setAttribute("data-sort", sort.key);
    item.textContent = sort.label;
    item.addEventListener("click", () => {
      sortKey = sort.key;
      syncSort();
      refresh();
      setMenu(false);
    });
    items.push(item);
    menu.append(item);
  }
  trigger.addEventListener("click", () => setMenu(menu.getAttribute("data-open") !== "true"));
  sortBox.append(trigger, menu);

  /** @param {boolean} open */
  function setMenu(open) {
    menu.setAttribute("data-open", open ? "true" : "false");
    trigger.setAttribute("aria-expanded", open ? "true" : "false");
    if (!open) return;
    // Popover: фокус уходит в меню, roving по выбранному пункту.
    const active = items.find((item) => item.getAttribute("aria-checked") === "true") ?? items[0];
    for (const item of items) item.setAttribute("tabindex", item === active ? "0" : "-1");
    /** @type {any} */ (active)?.focus?.();
  }

  function syncSort() {
    for (const item of items) {
      const checked = item.getAttribute("data-sort") === sortKey;
      item.setAttribute("aria-checked", checked ? "true" : "false");
      item.setAttribute("tabindex", checked ? "0" : "-1");
    }
    trigger.textContent = SEARCH_SORTS.find((sort) => sort.key === sortKey)?.label ?? "";
  }

  // Клавиатура popover-меню: стрелки/Home/End, Esc и Tab закрывают (не ловушка:
  // Tab выходит из меню, а не запирает — панель поиска не модальная, SRCH-02).
  menu.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (key === "Escape" || key === "Tab") {
      setMenu(false);
      /** @type {any} */ (trigger)?.focus?.();
      return;
    }
    const index = items.findIndex((item) => item.getAttribute("tabindex") === "0");
    /** @type {number | null} */
    let next = null;
    if (key === "ArrowDown") next = (index + 1) % items.length;
    else if (key === "ArrowUp") next = (index - 1 + items.length) % items.length;
    else if (key === "Home") next = 0;
    else if (key === "End") next = items.length - 1;
    else if (key === "Enter" || key === " ") next = index;
    if (next === null) return;
    const target = items[next];
    if (!target) return;
    for (const item of items) item.setAttribute("tabindex", item === target ? "0" : "-1");
    /** @type {any} */ (target)?.focus?.();
    if (key === "Enter" || key === " ") target.click();
  });

  /* ---------- M-13: полоса чипов активных фильтров ---------- */
  const chipsBar = document.createElement("div");
  chipsBar.className = "search-filter-chips";
  chipsBar.setAttribute("role", "toolbar");
  chipsBar.setAttribute("aria-label", "Активные фильтры поиска");
  chipsBar.setAttribute("hidden", "");
  const chipList = document.createElement("div");
  chipList.className = "search-chip-list";
  chipList.setAttribute("role", "list");
  const chipsClearBtn = document.createElement("button");
  chipsClearBtn.className = "search-chips-clear";
  chipsClearBtn.setAttribute("type", "button");
  chipsClearBtn.setAttribute("aria-label", "Сбросить все фильтры поиска");
  chipsClearBtn.textContent = "Сбросить";
  chipsBar.append(chipList, chipsClearBtn);

  /* ---------- M-13: баннер индексации (HTTP 202 / deep historical) ---------- */
  const indexingBanner = document.createElement("div");
  indexingBanner.className = "search-indexing-banner";
  indexingBanner.setAttribute("role", "status");
  indexingBanner.setAttribute("aria-live", "polite");
  indexingBanner.setAttribute("aria-atomic", "true");
  indexingBanner.setAttribute("data-indexing-state", "idle");
  indexingBanner.setAttribute("hidden", "");
  const indexingHeader = document.createElement("div");
  indexingHeader.className = "search-indexing-header";
  const indexingSpinner = document.createElement("span");
  indexingSpinner.className = "search-indexing-spinner";
  indexingSpinner.setAttribute("aria-hidden", "true");
  const indexingTitle = document.createElement("span");
  indexingTitle.className = "search-indexing-title";
  indexingTitle.textContent = "Индексация";
  const indexingBadge = document.createElement("span");
  indexingBadge.className = "search-indexing-badge";
  indexingBadge.textContent = `Попытка 1 из ${SEARCH_MAX_INDEXING_ATTEMPTS}`;
  indexingHeader.append(indexingSpinner, indexingTitle, indexingBadge);
  const indexingDetail = document.createElement("p");
  indexingDetail.className = "search-indexing-detail";
  indexingDetail.textContent = "";
  const indexingProgress = document.createElement("div");
  indexingProgress.className = "search-indexing-progress";
  indexingProgress.setAttribute("role", "progressbar");
  indexingProgress.setAttribute("aria-valuemin", "0");
  indexingProgress.setAttribute("aria-valuemax", String(SEARCH_MAX_INDEXING_ATTEMPTS));
  indexingProgress.setAttribute("aria-valuenow", "1");
  indexingProgress.setAttribute("aria-valuetext", `Попытка 1 из ${SEARCH_MAX_INDEXING_ATTEMPTS}`);
  const indexingProgressBar = document.createElement("span");
  indexingProgressBar.className = "search-indexing-progress-bar";
  indexingProgress.append(indexingProgressBar);
  const indexingActions = document.createElement("div");
  indexingActions.className = "search-indexing-actions";
  indexingActions.setAttribute("hidden", "");
  const indexingRetryBtn = document.createElement("button");
  indexingRetryBtn.className = "search-indexing-retry-btn";
  indexingRetryBtn.setAttribute("type", "button");
  indexingRetryBtn.textContent = "Повторить запрос";
  indexingActions.append(indexingRetryBtn);
  indexingBanner.append(indexingHeader, indexingDetail, indexingProgress, indexingActions);

  /* ---------- M-13: баннер NSFW 18+ гейта ---------- */
  const nsfwBanner = document.createElement("div");
  nsfwBanner.className = "search-nsfw-banner search-nsfw-gate";
  nsfwBanner.setAttribute("role", "region");
  nsfwBanner.setAttribute("aria-label", "Предупреждение о возрастном ограничении 18+");
  nsfwBanner.setAttribute("aria-live", "polite");
  nsfwBanner.setAttribute("aria-atomic", "true");
  nsfwBanner.setAttribute("data-nsfw-mode", "prompt");
  nsfwBanner.setAttribute("hidden", "");
  const nsfwHeader = document.createElement("div");
  nsfwHeader.className = "search-nsfw-header";
  const nsfwBadge = document.createElement("span");
  nsfwBadge.className = "search-nsfw-badge";
  nsfwBadge.setAttribute("aria-hidden", "true");
  nsfwBadge.textContent = "18+";
  const nsfwTitle = document.createElement("h3");
  nsfwTitle.className = "search-nsfw-title";
  nsfwTitle.textContent = "Канал с возрастным ограничением";
  nsfwHeader.append(nsfwBadge, nsfwTitle);
  const nsfwDesc = document.createElement("p");
  nsfwDesc.className = "search-nsfw-desc";
  nsfwDesc.textContent =
    "Пожалуйста, подтвердите, что вам исполнилось 18 лет и вы хотите просматривать контент для взрослых.";
  const nsfwActions = document.createElement("div");
  nsfwActions.className = "search-nsfw-actions";
  const nsfwContinueBtn = document.createElement("button");
  nsfwContinueBtn.className = "search-nsfw-btn search-nsfw-btn-continue";
  nsfwContinueBtn.setAttribute("type", "button");
  nsfwContinueBtn.textContent = "Продолжить";
  const nsfwBackBtn = document.createElement("button");
  nsfwBackBtn.className = "search-nsfw-btn search-nsfw-btn-back";
  nsfwBackBtn.setAttribute("type", "button");
  nsfwBackBtn.textContent = "Вернуться";
  nsfwActions.append(nsfwContinueBtn, nsfwBackBtn);
  nsfwBanner.append(nsfwHeader, nsfwDesc, nsfwActions);

  /* ---------- статус и результаты ---------- */
  const status = document.createElement("p");
  status.className = "search-status";
  status.setAttribute("aria-live", "polite");
  status.setAttribute("data-state", "ok");
  status.textContent = "";

  const results = document.createElement("div");
  results.className = "search-results";

  panel.append(
    head,
    chipsBar,
    sections,
    sortBox,
    indexingBanner,
    nsfwBanner,
    status,
    results,
    createFootnote(),
  );
  container.append(panel);

  /** @returns {HTMLElement} */
  function createFootnote() {
    const note = document.createElement("p");
    note.className = "search-footnote visually-hidden";
    note.textContent = "Поиск выполняется по загруженному окну данных, полный поиск — за сервером";
    return note;
  }

  /* ---------- состояние ---------- */
  /** @type {Filters} */
  const filters = { from: "", in: "", has: "", mentions: "", date: "", author: "Любой", pinned: false };
  let query = "";
  /** @type {string} */
  let sortKey = "new";
  let slice = SEARCH_SLICE;
  /** @type {number[]} */
  let lastMatches = [];

  /* ---------- B7b + M-13: сетевое состояние ---------- */
  /** `unknown` — команда ещё не проверена; `available`/`unavailable` — после ответа. */
  /** @type {"unknown" | "available" | "unavailable"} */
  let remoteState = invoke !== null ? "unknown" : "unavailable";
  /** Откуда сейчас выдача: локальное окно или сервер. @type {"local" | "remote"} */
  let mode = "local";
  /** Причина локального режима для честного статуса. @type {"" | "ipc" | "preview"} */
  let localReason = "";
  /** Номер последнего запроса: ответы устаревших запросов не применяются. */
  let requestSeq = 0;
  let remoteLoading = false;
  /** @type {RemoteMessage[]} */
  let remoteMessages = [];
  /** @type {Set<string>} */
  let remoteIds = new Set();
  let remoteTotal = 0;
  /** Сколько серверных записей уже пропущено (offset следующей страницы). */
  let remoteOffset = 0;
  let lastRequestOffset = 0;
  let remoteExhausted = false;
  let deepIndex = false;
  let documentsIndexed = 0;
  let indexingAttempts = 0;
  /** @type {ReturnType<typeof setTimeout> | null} */
  let indexingPollId = null;
  /** Незавершённые запросы (для `whenIdle()` в тестах/диагностике). */
  let inflight = 0;
  /** @type {Array<() => void>} */
  let idleWaiters = [];

  /**
   * M-13: Контекст NSFW и `GuildNSFWAgreeStore` (с fallback в память).
   * @type {{
   *   guild?: any,
   *   user?: any,
   *   regionalFeatureConfig?: any,
   *   channels?: Array<{ id: string, name?: string, nsfw?: boolean }>,
   *   currentChannel?: { id?: string, channelId?: string, nsfw?: boolean } | string | null,
   *   channel?: { id?: string, channelId?: string, nsfw?: boolean } | string | null,
   *   agreedStore?: Record<string, boolean>,
   * }}
   */
  let nsfwContext = {};
  /** @type {Record<string, boolean>} */
  const memoryNsfwAgreed = {};

  function cancelIndexingPoll() {
    if (indexingPollId !== null) {
      clearTimeout(indexingPollId);
      indexingPollId = null;
    }
  }

  function hideIndexingBanner() {
    indexingBanner.setAttribute("hidden", "");
    indexingBanner.setAttribute("data-indexing-state", "idle");
    indexingActions.setAttribute("hidden", "");
  }

  /** @returns {Storage | null} */
  function getBrowserLocalStorage() {
    // Браузер/WebView: штатный путь — `window.localStorage` (getter сам решает,
    // доступно ли хранилище в privacy mode).
    try {
      if (typeof globalThis.window !== "undefined" && globalThis.window) {
        const viaWindow = globalThis.window.localStorage;
        if (viaWindow) return viaWindow;
      }
    } catch {
      // Доступ к window.localStorage запрещён — пробуем descriptor ниже.
    }
    try {
      const desc = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
      if (!desc) return null;
      // Обычное data-свойство (полифилл/тест/старый WebView).
      if ("value" in desc) return /** @type {Storage | null} */ (desc.value ?? null);
      if (typeof desc.get !== "function") return null;
      // Встроенный getter Node без `--localstorage-file` только эмитит
      // ExperimentalWarning и отдаёт undefined — не трогаем его. Эмуляция
      // accessor'а в тестах и браузерный getter проходят дальше.
      if (isNodeBuiltinStorageGetter(desc.get)) return null;
      // Accessor `get`/`set` (браузер, WebView, эмуляция): читаем через `get`.
      return /** @type {Storage | null} */ (desc.get.call(globalThis) ?? null);
    } catch {
      return null;
    }
  }

  /**
   * Встроенный getter `globalThis.localStorage` в Node (native code) без
   * `window`: вызывать его бессмысленно и шумно.
   * @param {() => any} getter
   * @returns {boolean}
   */
  function isNodeBuiltinStorageGetter(getter) {
    const nodeProcess = /** @type {any} */ (globalThis).process;
    if (!nodeProcess || !nodeProcess.versions?.node) return false;
    if (typeof globalThis.window !== "undefined") return false;
    return /\[native code\]/.test(Function.prototype.toString.call(getter));
  }

  /** @returns {Record<string, boolean>} */
  function readPersistedNsfwStore() {
    if (nsfwContext.agreedStore && typeof nsfwContext.agreedStore === "object") {
      return nsfwContext.agreedStore;
    }
    try {
      const raw = getBrowserLocalStorage()?.getItem(GUILD_NSFW_AGREE_STORAGE_KEY);
      if (typeof raw === "string" && raw !== "") {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed === "object") return parsed;
      }
    } catch {
      // Best-effort: при недоступном localStorage используется memoryNsfwAgreed.
    }
    return memoryNsfwAgreed;
  }

  /** @param {string} guildId */
  function recordNsfwAgreement(guildId) {
    if (!guildId) return;
    memoryNsfwAgreed[guildId] = true;
    if (nsfwContext.agreedStore && typeof nsfwContext.agreedStore === "object") {
      nsfwContext.agreedStore[guildId] = true;
    }
    try {
      const next = { ...readPersistedNsfwStore(), [guildId]: true };
      getBrowserLocalStorage()?.setItem(GUILD_NSFW_AGREE_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Best-effort: уже записано в memoryNsfwAgreed / nsfwContext.agreedStore.
    }
  }

  /** @returns {boolean} */
  function currentDidAgreeNsfw() {
    const guildId = currentGuildId();
    if (!guildId) return false;
    return didAgreeNsfwGuild(
      guildObjectFor(guildId),
      nsfwContext.user,
      readPersistedNsfwStore(),
      nsfwContext.regionalFeatureConfig,
    );
  }

  /** id текущей гильдии: выбранная в рейле, иначе контекст `setNsfwContext`. */
  function currentGuildId() {
    return readGuildId() || String(nsfwContext.guild?.id ?? "");
  }

  /**
   * Гильдия для `didAgreeNsfwGuild` с id, совпадающим с ключом стора:
   * запись и чтение согласия обязаны использовать один и тот же id.
   * @param {string} guildId
   * @returns {any}
   */
  function guildObjectFor(guildId) {
    return nsfwContext.guild && typeof nsfwContext.guild === "object"
      ? { ...nsfwContext.guild, id: String(guildId) }
      : { id: String(guildId) };
  }

  /**
   * Является ли целевой канал фильтра `in` возрастным (18+).
   * @param {string} channelValue
   * @returns {boolean}
   */
  function isChannelNsfw(channelValue) {
    const target = String(channelValue ?? "").trim();
    if (target === "") return false;
    if (Array.isArray(nsfwContext.channels)) {
      const match = nsfwContext.channels.find(
        (ch) => String(ch?.id ?? "") === target || String(ch?.name ?? "") === target,
      );
      if (match && match.nsfw === true) return true;
    }
    const domChannel = document.querySelector(
      `[data-channel-id="${target}"][data-nsfw="true"], [data-channel="${target}"][data-nsfw="true"]`,
    );
    return domChannel !== null;
  }

  /**
   * Текущий открытый канал чата помечен 18+ (`nsfw === true`).
   * @returns {boolean}
   */
  function currentChannelIsNsfw() {
    const current = nsfwContext.currentChannel ?? nsfwContext.channel;
    if (current && typeof current === "object" && current.nsfw === true) return true;
    const id = typeof current === "string"
      ? current
      : String(current?.id ?? current?.channelId ?? "");
    return id !== "" && isChannelNsfw(id);
  }

  /**
   * Режим NSFW-баннера: `underage` при возрастном ограничении не-взрослого, иначе `prompt`.
   * @returns {"prompt" | "underage"}
   */
  function resolveNsfwGateMode() {
    const guildId = currentGuildId();
    if (nsfwContext.user?.nsfwAllowed === false || nsfwContext.user?.nsfw_allowed === false) {
      return "underage";
    }
    if (isAgeGatedSpacesRestricted(nsfwContext.user, nsfwContext.regionalFeatureConfig)) {
      return "underage";
    }
    return "prompt";
  }

  /**
   * §4.3: заголовок баннера зависит от триггера — `xi46lg` для гильдии,
   * `ZmwvDc` для канала.
   * @param {"prompt" | "underage"} gateMode
   * @param {"channel" | "guild"} [source]
   */
  function showNsfwGate(gateMode, source = "channel") {
    const firstOpen = nsfwBanner.hasAttribute("hidden");
    nsfwBanner.removeAttribute("hidden");
    nsfwBanner.setAttribute("data-nsfw-mode", gateMode);
    if (gateMode === "underage") {
      nsfwTitle.textContent =
        "К сожалению, вы ещё слишком молоды для просмотра этого канала с возрастным ограничением";
      nsfwDesc.textContent =
        "Для просмотра каналов с возрастными ограничениями в Discord вам должно быть не менее 18 лет. Ознакомьтесь с нашей полезной статьёй о подтверждении возраста.";
      nsfwContinueBtn.setAttribute("hidden", "");
      nsfwBackBtn.removeAttribute("hidden");
      // §7.1: фокус — только при открытии баннера и не из поля ввода.
      if (firstOpen && document.activeElement !== input) nsfwBackBtn.focus();
    } else {
      nsfwTitle.textContent = source === "guild"
        ? "Сервер с возрастным ограничением"
        : "Канал с возрастным ограничением";
      nsfwDesc.textContent =
        "В этом канале содержится контент для взрослых с возрастными ограничениями. Хотите продолжить?";
      nsfwContinueBtn.removeAttribute("hidden");
      nsfwBackBtn.removeAttribute("hidden");
      if (firstOpen && document.activeElement !== input) nsfwContinueBtn.focus();
    }
  }

  function hideNsfwGate() {
    nsfwBanner.setAttribute("hidden", "");
  }

  /**
   * §4.2 (B): общегильдийский поиск без согласия. Сообщения из 18+ каналов
   * исключаются из выдачи, а над результатами показывается баннер `prompt`.
   * Сетевой запрос при этом уходит без `include_nsfw`.
   * @param {boolean} hasNsfwMatches
   */
  function syncGuildNsfwHint(hasNsfwMatches) {
    if (!hasNsfwMatches || currentDidAgreeNsfw()) return;
    const parsed = parseSearchQueryTokens(query);
    const inlineIn = parsed.chips.find((c) => c.key === "in" && !c.negated)?.value ?? "";
    if (filters.in !== "" || inlineIn !== "") return;
    showNsfwGate("prompt", "guild");
  }

  nsfwContinueBtn.addEventListener("click", () => {
    recordNsfwAgreement(currentGuildId());
    hideNsfwGate();
    refresh();
  });

  nsfwBackBtn.addEventListener("click", () => {
    hideNsfwGate();
    const inControl = field("in");
    if (inControl && typeof inControl.value === "string") {
      inControl.value = "";
    }
    filters.in = "";
    cancelIndexingPoll();
    indexingAttempts = 0;
    results.replaceChildren();
    status.textContent = "";
    status.setAttribute("data-state", "ok");
    // §4.3: если сам открытый канал 18+, возврат закрывает панель (безопасный
    // поиск невозможен в NSFW-канале); иначе фокус — в поле запроса.
    if (currentChannelIsNsfw()) {
      close(true);
      return;
    }
    /** @type {any} */ (input)?.focus?.();
  });

  indexingRetryBtn.addEventListener("click", () => {
    cancelIndexingPoll();
    indexingAttempts = 0;
    void fetchRemote(false, false);
  });

  /** §2.1/§5.3: канонический префикс токена для `data-chip-remove`. */
  const CHIP_PREFIXES = Object.freeze({
    from: "from:",
    in: "in:",
    has: "has:",
    mentions: "mentions:",
    before: "before:",
    after: "after:",
    on: "on:",
    date: "on:",
    authorType: "authorType:",
    author: "authorType:",
    pinned: "pinned:",
  });

  /**
   * Контрол `.search-sections`, соответствующий чипу (§5.1): у `before`/`after`
   * отдельного контрола нет, `on:`/`день:` живут в секции «Дата».
   * @param {string} chipKey
   * @returns {string | null}
   */
  function controlKeyForChip(chipKey) {
    if (chipKey === "on" || chipKey === "date") return "date";
    if (chipKey === "before" || chipKey === "after") return null;
    if (chipKey === "authorType" || chipKey === "author") return "author";
    return chipKey;
  }

  /**
   * §5.3.1: инлайн-токены заполняют ещё пустые контролы `.search-sections`
   * (двусторонняя синхронизация). Введённые пользователем значения не
   * перетираются.
   * @param {{ chips: Array<{ key: string, value: string, negated: boolean }> }} parsed
   */
  function syncInlineTokensToControls(parsed) {
    /** @type {Map<string, string[]>} */
    const values = new Map();
    for (const chip of parsed.chips) {
      const controlKey = controlKeyForChip(chip.key);
      if (controlKey === null) continue;
      const list = values.get(controlKey) ?? [];
      const token = `${chip.negated ? "-" : ""}${chip.value}`;
      if (!list.includes(token)) list.push(token);
      values.set(controlKey, list);
    }
    for (const [controlKey, list] of values) {
      const control = field(controlKey);
      if (!control || list.length === 0) continue;
      if (controlKey === "pinned") {
        if (list.includes("true")) {
          control.setAttribute("aria-checked", "true");
          control.textContent = "Вкл";
        }
        continue;
      }
      if (controlKey === "author") {
        const canonical = list[list.length - 1]?.replace(/^-/, "") ?? "";
        const label = Object.entries(SEARCH_AUTHOR_TYPES).find(([, value]) => value === canonical)?.[0];
        const current = typeof control.value === "string" ? control.value : "";
        if (label && (current === "" || current === "Любой")) control.value = label;
        continue;
      }
      const current = typeof control.value === "string" ? control.value.trim() : "";
      if (current === "") control.value = list.join(" ");
    }
  }

  /**
   * §5.3.2: удаление чипа чистит и контрол, и инлайн-токен, чтобы фильтр не
   * «воскрес» из синхронизированного значения.
   * @param {string} controlKey
   * @param {string} value
   * @param {boolean} negated
   */
  function clearControlValue(controlKey, value, negated) {
    const control = field(controlKey);
    if (!control) return;
    if (controlKey === "pinned") {
      control.setAttribute("aria-checked", "false");
      control.textContent = "Выкл";
      return;
    }
    if (controlKey === "author") {
      if (typeof control.value === "string") control.value = "Любой";
      return;
    }
    if (typeof control.value !== "string") return;
    const wanted = `${negated ? "-" : ""}${value}`;
    // Значение может содержать пробел (`from:"Alex Mercer"`), поэтому режем по
    // границам токена, а не split(/\s+/).
    control.value = stripControlValue(control.value, wanted);
    if (negated) control.value = stripControlValue(control.value, value);
  }

  /**
   * @param {string} text
   * @param {string} value
   * @returns {string}
   */
  function stripControlValue(text, value) {
    if (value === "") return text.trim();
    const escaped = value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return text
      .replace(new RegExp(`(^|[\\s,])${escaped}(?=[\\s,]|$)`, "g"), " ")
      .replace(/[\s,]+/g, " ")
      .trim();
  }

  /** @param {string} raw @param {string} token */
  function removeQueryToken(raw, token) {
    return String(raw ?? "").split(token).join("").replace(/\s+/g, " ").trim();
  }

  /** §7.2: кольцо фокуса по кнопкам чипов и «Сбросить». @returns {HTMLElement[]} */
  function chipFocusTargets() {
    return [
      .../** @type {HTMLElement[]} */ ([...chipList.querySelectorAll(".search-chip-remove")]),
      chipsClearBtn,
    ];
  }

  /**
   * @param {HTMLElement | null} from
   * @param {number} delta
   */
  function moveChipFocus(from, delta) {
    const targets = chipFocusTargets();
    const index = targets.indexOf(/** @type {HTMLElement} */ (from));
    if (index < 0 || targets.length === 0) return;
    const next = targets[(index + delta + targets.length) % targets.length];
    /** @type {any} */ (next)?.focus?.();
  }

  /**
   * §7.2: Backspace/Delete удаляет чип и уводит фокус на соседний, а если чипов
   * не осталось — в поле запроса.
   * @param {number} index
   * @param {() => void} remove
   */
  function removeChipWithFocus(index, remove) {
    remove();
    const remaining = /** @type {HTMLElement[]} */ ([...chipList.querySelectorAll(".search-chip-remove")]);
    if (remaining.length === 0) {
      /** @type {any} */ (input)?.focus?.();
      return;
    }
    const target = remaining[Math.min(index, remaining.length - 1)];
    /** @type {any} */ (target)?.focus?.();
  }

  /**
   * M-13: Сбор и рендеринг чипов активных фильтров (инлайн-токены + контролы).
   */
  function syncFilterChips() {
    const parsed = parseSearchQueryTokens(query);
    /**
     * @type {Array<{
     *   key: string,
     *   value: string,
     *   negated: boolean,
     *   raw?: string,
     *   source: "inline" | "control",
     * }>}
     */
    const active = parsed.chips.map((chip) => ({
      key: chip.key,
      value: chip.value,
      negated: chip.negated,
      raw: chip.rawToken,
      source: "inline",
    }));

    const addControlChip = (/** @type {string} */ key, /** @type {string} */ val, negated = false) => {
      if (val === "") return;
      if (active.some((c) => c.key === key && c.value === val && c.negated === negated)) return;
      active.push({ key, value: val, negated, source: "control" });
    };

    for (const val of splitQueryValues(filters.from)) addControlChip("from", val);
    for (const val of splitQueryValues(filters.in)) addControlChip("in", val);
    for (const val of splitQueryValues(filters.has)) {
      const neg = val.startsWith("-");
      addControlChip("has", neg ? val.slice(1) : val, neg);
    }
    for (const val of splitQueryValues(filters.mentions)) addControlChip("mentions", val);
    if (filters.date !== "") addControlChip("date", filters.date);
    if (filters.author !== "" && filters.author !== "Любой") addControlChip("author", filters.author);
    if (filters.pinned) addControlChip("pinned", "true");

    if (active.length === 0) {
      chipList.replaceChildren();
      chipsBar.setAttribute("hidden", "");
      return;
    }

    chipsBar.removeAttribute("hidden");
    /** @type {HTMLElement[]} */
    const chipNodes = [];
    active.forEach((chip, index) => {
      const item = document.createElement("span");
      item.className = "search-chip";
      item.setAttribute("role", "listitem");
      item.setAttribute("data-chip-key", chip.key);
      item.setAttribute("data-chip-value", chip.value);
      item.setAttribute("data-chip-negated", chip.negated ? "true" : "false");

      const labelText = /** @type {Record<string, string>} */ (CHIP_LABELS)[chip.key] ?? chip.key;
      const prefix = /** @type {Record<string, string>} */ (CHIP_PREFIXES)[chip.key] ?? `${chip.key}:`;
      const tokenLabel = `${chip.negated ? "-" : ""}${prefix}${chip.value}`;
      // §2.1: span.search-chip-prefix / span.search-chip-value.
      const keySpan = node("search-chip-prefix", `${chip.negated ? "−" : ""}${labelText}:`);
      const valSpan = node("search-chip-value", chip.value);

      const removeBtn = document.createElement("button");
      removeBtn.className = "search-chip-remove";
      removeBtn.setAttribute("type", "button");
      removeBtn.setAttribute("data-chip-remove", tokenLabel);
      removeBtn.setAttribute("aria-label", `Удалить фильтр ${labelText}: ${chip.value}`);
      removeBtn.textContent = "✕";

      const remove = () => {
        if (chip.source === "inline" && chip.raw) {
          query = removeQueryToken(query, chip.raw);
          input.value = query;
          if (header) header.value = query;
        }
        const controlKey = controlKeyForChip(chip.key);
        if (controlKey !== null) clearControlValue(controlKey, chip.value, chip.negated);
        reset();
      };

      removeBtn.addEventListener("click", () => removeChipWithFocus(index, remove));
      removeBtn.addEventListener("keydown", (event) => {
        const key = "key" in event ? String(event.key) : "";
        if (key === "Backspace" || key === "Delete") {
          removeChipWithFocus(index, remove);
          return;
        }
        if (key === "ArrowLeft") {
          event.preventDefault();
          moveChipFocus(removeBtn, -1);
          return;
        }
        if (key === "ArrowRight") {
          event.preventDefault();
          moveChipFocus(removeBtn, 1);
        }
      });

      item.append(keySpan, valSpan, removeBtn);
      chipNodes.push(item);
    });
    chipList.replaceChildren(...chipNodes);
  }

  chipsClearBtn.addEventListener("click", () => {
    const parsed = parseSearchQueryTokens(query);
    query = parsed.freeText;
    input.value = query;
    if (header) header.value = query;
    for (const [key, ctrl] of controls.entries()) {
      if (key === "pinned") {
        ctrl.setAttribute("aria-checked", "false");
        ctrl.textContent = "Выкл";
      } else if (key === "author") {
        /** @type {any} */ (ctrl).value = "Любой";
      } else {
        /** @type {any} */ (ctrl).value = "";
      }
    }
    reset();
    // §5.3.3: фокус возвращается в поле запроса.
    /** @type {any} */ (input)?.focus?.();
  });

  chipsClearBtn.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (key !== "ArrowLeft" && key !== "ArrowRight") return;
    event.preventDefault();
    moveChipFocus(chipsClearBtn, key === "ArrowLeft" ? -1 : 1);
  });

  // §7.2: Backspace в позиции 0 уводит фокус на последний чип.
  input.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (key !== "Backspace") return;
    if (Number(input.selectionStart ?? 0) !== 0 || Number(input.selectionEnd ?? 0) !== 0) return;
    const removes = /** @type {HTMLElement[]} */ ([...chipList.querySelectorAll(".search-chip-remove")]);
    if (removes.length === 0) return;
    event.preventDefault();
    /** @type {any} */ (removes[removes.length - 1])?.focus?.();
  });

  function beginRequest() {
    inflight += 1;
    panel.setAttribute("aria-busy", "true");
  }

  function endRequest() {
    inflight = Math.max(0, inflight - 1);
    panel.setAttribute("aria-busy", inflight > 0 || indexingPollId !== null ? "true" : "false");
    if (inflight > 0) return;
    const waiters = idleWaiters;
    idleWaiters = [];
    for (const resolve of waiters) resolve();
  }

  /** @returns {Promise<void>} */
  function whenIdle() {
    if (inflight === 0) return Promise.resolve();
    return new Promise((resolve) => idleWaiters.push(resolve));
  }

  /** Есть ли у сервера вход, по которому можно искать (content обязателен). */
  const hasRemoteCriteria = () => query.trim() !== "";

  const canRemote = () => remoteState !== "unavailable" && invoke !== null && readGuildId() !== "";

  /**
   * B7b + STUB-14 + M-13: подтверждённые параметры поиска. Принятый backend B7a
   * (`search.rs`) читает `content`/`channelId`/`sortBy`/`sortOrder`/`offset`;
   * полный набор queryKey из RE-DEEP-REA §2.3–2.4 уходит в `params.query`.
   * @param {number} offset
   * @returns {{
   *   content: string, channelId?: string, sortBy: string, sortOrder: string,
   *   offset: number, query: Record<string, unknown>,
   * }}
   */
  function buildRemoteParams(offset) {
    const sort = REMOTE_SORTS[sortKey] ?? { sortBy: "timestamp", sortOrder: "desc" };
    const parsed = parseSearchQueryTokens(query);
    const didAgree = currentDidAgreeNsfw();
    const builtQuery = buildSearchQuery(filters, /** @type {any} */ ({
      content: query.trim(),
      sortBy: sort.sortBy,
      sortOrder: sort.sortOrder,
      offset,
      includeNsfw: didAgree,
      attempts: indexingAttempts > 0 ? indexingAttempts : undefined,
    }));
    /** @type {{
     *   content: string, channelId?: string, sortBy: string, sortOrder: string,
     *   offset: number, query: Record<string, unknown>,
     * }} */
    const params = {
      content: parsed.freeText !== "" ? parsed.freeText : query.trim(),
      sortBy: sort.sortBy,
      sortOrder: sort.sortOrder,
      offset,
      query: builtQuery,
    };
    if (filters.in !== "") params.channelId = filters.in;
    return params;
  }

  /**
   * Контрол секции: input/select (у switch `value` нет — читаем aria-checked).
   * @param {string} key
   * @returns {any}
   */
  const field = (key) => controls.get(key) ?? null;

  function readControls() {
    const value = (/** @type {string} */ key) => {
      const control = field(key);
      return control && typeof control.value === "string" ? control.value.trim() : "";
    };
    filters.from = value("from");
    filters.in = value("in");
    filters.has = value("has");
    filters.mentions = value("mentions");
    filters.date = value("date");
    const author = field("author");
    filters.author = author && typeof author.value === "string" ? author.value : "Любой";
    const pinned = field("pinned");
    filters.pinned = pinned?.getAttribute("aria-checked") === "true";
  }

  /**
   * @param {string} has
   * @param {number} index
   * @returns {boolean}
   */
  function hasContent(has, index) {
    const target = has.toLowerCase();
    if (target === "") return true;
    const extras = extrasFor(index);
    if (target === "ссылка") return extras.embeds.some((embed) => embed.kind === "link");
    if (target === "картинка") return extras.embeds.some((embed) => embed.kind === "image");
    if (target === "эмодзи") return extras.reactions.length > 0;
    // Файлов/звука/видео/опросов в демо-данных нет — честный false, а не «всё».
    return false;
  }

  /**
   * @param {any} item
   * @param {number} index
   * @returns {boolean}
   */
  function matches(item, index) {
    const text = String(item?.text ?? "").toLowerCase();
    const author = String(item?.author ?? "").toLowerCase();
    const needle = query.trim().toLowerCase();
    if (needle !== "" && !(text.includes(needle) || author.includes(needle) || String(index).includes(needle))) {
      return false;
    }
    if (filters.from && !author.includes(filters.from.toLowerCase())) return false;
    if (filters.mentions && !author.includes(filters.mentions.toLowerCase())) return false;
    if (filters.in && !DEMO_CHANNEL.includes(filters.in.toLowerCase())) return false;
    if (filters.date && filters.date !== DEMO_DAY) return false;
    if (!hasContent(filters.has, index)) return false;
    if (filters.author === "Человек" && item?.bot) return false;
    if (filters.author === "Бот" && !item?.bot) return false;
    if (filters.author === "Вебхук") return false;
    if (filters.pinned && !item?.pinned) return false;
    return true;
  }

  /**
   * STUB-14: «Содержит» для серверного сообщения по подтверждённым токенам
   * §2.3 (`link/embed/poll/snapshot/file/video/image/sound/sticker`).
   * Работаем только с приложенными полями; нет данных — честный false.
   * @param {string} has
   * @param {RemoteMessage} message
   * @returns {boolean}
   */
  function remoteHasContent(has, message) {
    if (has.trim() === "") return true;
    const tokens = normalizeHasFilter(has);
    if (tokens.length === 0) return false;
    const raw = /** @type {any} */ (message.raw ?? {});
    return tokens.every((token) => remoteHasToken(token, raw));
  }

  /**
   * @param {string} token подтверждённое значение `has` (§2.3)
   * @param {any} raw серверное сообщение
   * @returns {boolean}
   */
  function remoteHasToken(token, raw) {
    const embeds = /** @type {any[]} */ (Array.isArray(raw?.embeds) ? raw.embeds : []);
    const attachments = /** @type {any[]} */ (
      Array.isArray(raw?.attachments) ? raw.attachments : []
    );
    const stickers = /** @type {any[]} */ (
      Array.isArray(raw?.sticker_items)
        ? raw.sticker_items
        : Array.isArray(raw?.stickerItems)
          ? raw.stickerItems
          : Array.isArray(raw?.stickers)
            ? raw.stickers
            : []
    );
    const snapshots = /** @type {any[]} */ (
      Array.isArray(raw?.message_snapshots)
        ? raw.message_snapshots
        : Array.isArray(raw?.messageSnapshots)
          ? raw.messageSnapshots
          : []
    );
    const contentTypes = attachments.map((/** @type {any} */ item) =>
      String(item?.content_type ?? "")
    );
    const embedTypes = embeds.map((/** @type {any} */ item) => String(item?.type ?? ""));
    switch (token) {
      case "link":
        return embeds.length > 0
          || attachments.some((/** @type {any} */ item) => typeof item?.url === "string");
      case "embed":
        return embeds.length > 0;
      case "poll":
        return raw.poll !== undefined && raw.poll !== null;
      case "snapshot":
        return snapshots.length > 0;
      case "file":
        return attachments.length > 0;
      case "video":
        return contentTypes.some((type) => type.startsWith("video/")) || embedTypes.includes("video");
      case "image":
        return contentTypes.some((type) => type.startsWith("image/")) || embedTypes.includes("image");
      case "sound":
        return contentTypes.some((type) => type.startsWith("audio/")) || embedTypes.includes("audio");
      case "sticker":
        return stickers.length > 0;
      default:
        return false;
    }
  }

  /**
   * Пять фильтров без подтверждённого REST-параметра применяются локально
   * к уже полученным серверным результатам (честная подпись «локально»).
   * @param {RemoteMessage[]} list
   * @returns {RemoteMessage[]}
   */
  function applyRemoteFilters(list) {
    const agreed = currentDidAgreeNsfw();
    return list.filter((message) => {
      // §4.2 (B): без согласия сообщения из 18+ каналов не показываются.
      if (!agreed && isChannelNsfw(message.channel)) return false;
      // P1: числовой id сверяется с author.id/mentions[].id, имя — с именем.
      if (!matchesFilterValue(filters.from, message.authorId !== "" ? [message.authorId] : [], [message.author])) {
        return false;
      }
      if (!matchesFilterValue(filters.mentions, message.mentionIds, [message.author])) return false;
      if (filters.date && message.date !== filters.date) return false;
      if (!remoteHasContent(filters.has, message)) return false;
      if (filters.author === "Человек" && message.bot) return false;
      if (filters.author === "Бот" && !message.bot) return false;
      if (filters.author === "Вебхук") return false;
      if (filters.pinned && !message.pinned) return false;
      return true;
    });
  }

  /**
   * @param {RemoteMessage[]} list
   * @returns {RemoteMessage[]}
   */
  function sortRemote(list) {
    if (sortKey !== "recent") return [...list];
    // «Самое актуальное»: закреплённые выше; внутри групп — порядок сервера.
    return [...list].sort((left, right) => {
      if (left.pinned === right.pinned) return 0;
      return left.pinned ? -1 : 1;
    });
  }

  /**
   * Скан загруженного окна (ADR-1): только страницы в `store.pages`.
   * @returns {{ indices: number[], loadedRows: number }}
   */
  function scan() {
    /** @type {number[]} */
    const indices = [];
    let loadedRows = 0;
    const store = chat.store;
    for (const [pageIndex, page] of store.pages) {
      for (let offset = 0; offset < page.rows.length; offset++) {
        const index = pageIndex * store.pageSize + offset;
        loadedRows += 1;
        if (matches(page.rows[offset], index)) indices.push(index);
      }
    }
    return { indices, loadedRows };
  }

  /**
   * @param {number[]} indices
   * @returns {number[]}
   */
  function sorted(indices) {
    const list = [...indices];
    if (sortKey === "old") list.sort((left, right) => left - right);
    else if (sortKey === "recent") {
      // «Самое актуальное» (R-11): закреплённые выше, затем свежие сверху.
      // От «По новизне» отличается именно закреплением, а не порядком времени.
      list.sort((left, right) => {
        const leftPinned = chat.store.getRow(left)?.pinned === true;
        const rightPinned = chat.store.getRow(right)?.pinned === true;
        if (leftPinned !== rightPinned) return leftPinned ? -1 : 1;
        return right - left;
      });
    } else list.sort((left, right) => right - left);
    return list;
  }

  /**
   * @param {number} index
   * @param {string} label
   * @returns {HTMLElement}
   */
  function resultItem(index, label) {
    const item = chat.store.getRow(index);
    const button = document.createElement("button");
    button.className = "search-result";
    button.setAttribute("type", "button");
    button.setAttribute("data-index", String(index));
    button.setAttribute("aria-label", `Перейти к сообщению #${index} от ${item?.author ?? "автора"}`);
    const head = node("search-result-head", `${item?.author ?? "автор"} · ${DEMO_CHANNEL} · ${item?.time ?? ""}`);
    const text = String(item?.text ?? label);
    const snippet = text.length > 120 ? `${text.slice(0, 117)}…` : text;
    const body = node("search-result-text", snippet);
    button.append(head, body);
    button.addEventListener("click", () => {
      chat.jumpTo(index);
      close(true);
    });
    return button;
  }

  /** @param {string} text @returns {HTMLElement} */
  function emptyNode(text) {
    const empty = document.createElement("p");
    empty.className = "search-empty";
    empty.setAttribute("role", "status");
    empty.textContent = text;
    return empty;
  }

  /**
   * Серверный результат: id/канал — из ответа, переход по серверу не
   * подменяется локальным индексом (сообщения может не быть в окне).
   * @param {RemoteMessage} message
   * @returns {HTMLElement}
   */
  function remoteResultItem(message) {
    const button = document.createElement("button");
    button.className = "search-result";
    button.setAttribute("type", "button");
    button.setAttribute("data-id", message.id);
    button.setAttribute("data-source", "remote");
    const where = message.channel !== "" ? `канал ${message.channel}` : "сервер";
    button.setAttribute("aria-label", `Серверный результат: ${message.author}, ${message.date}`);
    const head = node("search-result-head", `${message.author} · ${where} · ${message.date} ${message.time}`.trim());
    const isNsfwMessage = message.raw?.nsfw === true || isChannelNsfw(message.channel);
    if (isNsfwMessage) {
      const badge = node("search-result-nsfw-badge", "18+");
      head.append(badge);
    }
    const snippet = message.text.length > 120 ? `${message.text.slice(0, 117)}…` : message.text;
    const body = node("search-result-text", snippet);
    button.append(head, body);
    button.addEventListener("click", () => {
      setStatus(
        `Сообщение ${message.id !== "" ? message.id : "—"} с сервера вне загруженного окна — `
          + "переход по серверным результатам пока недоступен",
      );
    });
    return button;
  }

  /**
   * Оконный статус (прежний контракт RW-39) + честная причина локального режима.
   * @param {number} found
   * @param {number} loadedRows
   * @returns {string}
   */
  function localStatus(found, loadedRows) {
    const base = `найдено ${found} из ${loadedRows} загруженных `
      + `(всего ${formatNumber(chat.store.totalRows)})`;
    if (localReason === "ipc") return `${base} · локально: сетевая команда поиска недоступна`;
    if (localReason === "preview") return `${base} · локальный обзор загруженного окна`;
    return base;
  }

  /** Новый запрос/фильтр начинает выдачу с первого среза (SRCH-15). */
  function reset() {
    slice = SEARCH_SLICE;
    cancelIndexingPoll();
    indexingAttempts = 0;
    return refresh();
  }

  /** @param {string} text @param {"ok" | "error" | "loading"} [state] */
  function setStatus(text, state = "ok") {
    status.textContent = text;
    status.setAttribute("data-state", state);
  }

  /** Локальный скан загруженного окна (ADR-1) — fallback и пустой запрос. */
  function renderLocal() {
    hideIndexingBanner();
    const { indices, loadedRows } = scan();
    lastMatches = sorted(indices);
    const shown = lastMatches.slice(0, slice);
    const nodes = shown.map((index) => resultItem(index, ""));
    if (lastMatches.length > shown.length) {
      const more = document.createElement("button");
      more.className = "search-more";
      more.setAttribute("type", "button");
      more.setAttribute("aria-label", "Показать ещё результаты");
      more.textContent = `Показать ещё (${lastMatches.length - shown.length})`;
      more.addEventListener("click", () => {
        slice += SEARCH_SLICE;
        refresh();
      });
      nodes.push(more);
    }
    if (nodes.length === 0) nodes.push(emptyNode("Ничего не найдено"));
    results.replaceChildren(...nodes);
    setStatus(localStatus(lastMatches.length, loadedRows));
    return lastMatches;
  }

  /**
   * @param {boolean} append
   */
  function renderLoading(append) {
    setStatus("Ищем на сервере…", "loading");
    // При догрузке страницы уже показанные результаты не мигают.
    if (!append) results.replaceChildren(emptyNode("Загрузка…"));
  }

  /** Отрисовка накопленных серверных страниц (offset-пагинация, без дублей). */
  function renderRemote() {
    const visible = sortRemote(applyRemoteFilters(remoteMessages));
    /** @type {HTMLElement[]} */
    const nodes = visible.map(remoteResultItem);
    const remaining = Math.max(0, remoteTotal - remoteMessages.length);
    const ceilingReached = lastRequestOffset >= MAX_SEARCH_OFFSET || remoteOffset > MAX_SEARCH_OFFSET;
    if (!remoteExhausted && !ceilingReached && remaining > 0) {
      const more = document.createElement("button");
      more.className = "search-more";
      more.setAttribute("type", "button");
      more.setAttribute("aria-label", "Показать ещё результаты с сервера");
      more.textContent = `Показать ещё (${formatNumber(remaining)})`;
      more.addEventListener("click", () => {
        if (remoteLoading) return;
        void fetchRemote(true);
      });
      nodes.push(more);
    }
    if (nodes.length === 0) {
      nodes.push(emptyNode(
        remoteMessages.length > 0 ? "Под локальные фильтры ничего не подошло" : "Ничего не найдено",
      ));
    }
    results.replaceChildren(...nodes);

    if (deepIndex) {
      indexingBanner.removeAttribute("hidden");
      indexingBanner.setAttribute("data-indexing-state", "historical");
      indexingTitle.textContent = "Индексация";
      indexingBadge.textContent = "Архив";
      indexingDetail.textContent = formatHistoricalIndexedStatus(documentsIndexed);
      indexingProgress.setAttribute("aria-valuenow", String(SEARCH_MAX_INDEXING_ATTEMPTS));
      indexingProgress.setAttribute("aria-valuetext", `Архивная индексация: ${documentsIndexed}`);
      indexingActions.setAttribute("hidden", "");
    } else {
      hideIndexingBanner();
    }

    // §4.2 (B): баннер над общегильдийской выдачей при найденных NSFW-каналах.
    syncGuildNsfwHint(remoteMessages.some((message) => isChannelNsfw(message.channel)));

    if (remoteMessages.length === 0 && remoteTotal === 0) {
      setStatus("на сервере ничего не найдено");
      return;
    }
    const parts = [
      `на сервере найдено ${formatNumber(remoteTotal)}`,
      `показано ${formatNumber(remoteMessages.length)}`,
    ];
    if (visible.length !== remoteMessages.length) {
      parts.push(`после локальных фильтров ${formatNumber(visible.length)}`);
    }
    if (deepIndex) parts.push("индекс ещё строится");
    if (ceilingReached) {
      parts.push(formatMaxPagesWarning(MAX_SEARCH_PAGES));
    }
    setStatus(parts.join(" · "));
  }

  /**
   * `SearchError` (401/403/404/4xx/429/5xx/сеть) — видимая ошибка. Локальный
   * fallback здесь запрещён: иначе пользователь примет неполную выдачу за
   * серверную, а «нет доступа» превратится в «команда недоступна».
   * @param {any} error
   */
  function renderRemoteError(error) {
    hideIndexingBanner();
    const text = remoteErrorText(error);
    const reported = new UIError("ACTION_FAILED", `поиск: ${text}`);
    reportError(reported.code, reported);
    setStatus(`Ошибка: ${text}`, "error");
    results.replaceChildren(emptyNode("Не удалось выполнить поиск"));
    onError(reported);
  }

  /**
   * Отрисовка состояния HTTP 202 (индексация / исчерпание 5 попыток).
   * @param {any} response
   * @param {boolean} append
   */
  function handleIndexingResponse(response, append) {
    indexingAttempts += 1;
    if (indexingAttempts > SEARCH_MAX_INDEXING_ATTEMPTS) {
      cancelIndexingPoll();
      indexingBanner.removeAttribute("hidden");
      indexingBanner.setAttribute("data-indexing-state", "exhausted");
      indexingTitle.textContent = "Индексация занимает больше времени";
      const label = `${SEARCH_MAX_INDEXING_ATTEMPTS} из ${SEARCH_MAX_INDEXING_ATTEMPTS} попыток`;
      indexingBadge.textContent = label;
      indexingDetail.textContent =
        "Нам нужно проиндексировать этот сервер перед поиском. Дайте нам минуточку.";
      indexingProgress.setAttribute("aria-valuenow", String(SEARCH_MAX_INDEXING_ATTEMPTS));
      indexingProgress.setAttribute("aria-valuetext", label);
      indexingActions.removeAttribute("hidden");
      setStatus("Индексация занимает больше времени", "ok");
      return;
    }

    const delayMs = computeIndexingRetryDelayMs(response);
    const delaySec = Math.max(1, Math.round(delayMs / 1000));
    const attemptLabel = `Попытка ${indexingAttempts} из ${SEARCH_MAX_INDEXING_ATTEMPTS}`;
    indexingBanner.removeAttribute("hidden");
    indexingBanner.setAttribute("data-indexing-state", "indexing");
    indexingTitle.textContent = "Индексация";
    indexingBadge.textContent = attemptLabel;
    // SEARCH-202-NSFW-RE §1.3: тело 202 игнорируется; выводится строка AXPbZr + задержка из Retry-After.
    indexingDetail.textContent =
      `Нам нужно проиндексировать этот сервер перед поиском. Дайте нам минуточку. Повтор через ${delaySec} с…`;
    indexingProgress.setAttribute("aria-valuenow", String(indexingAttempts));
    indexingProgress.setAttribute("aria-valuetext", attemptLabel);
    indexingActions.setAttribute("hidden", "");
    setStatus("Индексация сервера…", "loading");

    cancelIndexingPoll();
    indexingPollId = setTimeout(() => {
      indexingPollId = null;
      void fetchRemote(append, true);
    }, delayMs);
  }

  /**
   * @param {boolean} append true — следующая страница по offset
   * @param {boolean} [isRetry] true — автоматический повтор по HTTP 202
   * @param {number} [explicitOffset] явный offset страницы (≤ MAX_SEARCH_OFFSET = 9975)
   * @returns {Promise<void>}
   */
  async function fetchRemote(append, isRetry = false, explicitOffset = undefined) {
    // Снимок на запрос: выбранный сервер не меняется посреди страницы.
    const guildId = readGuildId();
    if (invoke === null || guildId === "") return;
    if (append && (remoteLoading || remoteExhausted)) return;

    const requestOffset = explicitOffset !== undefined
      ? explicitOffset
      : append
        ? remoteOffset
        : 0;
    if (requestOffset < 0 || requestOffset > MAX_SEARCH_OFFSET) {
      lastRequestOffset = MAX_SEARCH_OFFSET;
      renderRemote();
      return;
    }

    if (!isRetry) {
      cancelIndexingPoll();
      indexingAttempts = 0;
    }
    const previous = remoteMessages.length;
    if (!append && !isRetry && explicitOffset === undefined) {
      remoteMessages = [];
      remoteIds = new Set();
      remoteTotal = 0;
      remoteOffset = 0;
      lastRequestOffset = 0;
      remoteExhausted = false;
      deepIndex = false;
      documentsIndexed = 0;
    }
    lastRequestOffset = requestOffset;
    const seq = ++requestSeq;
    remoteLoading = true;
    beginRequest();
    renderLoading(append || explicitOffset !== undefined);
    try {
      const response = await invoke("search_remote", {
        guildId,
        // Контракт B7a: `search_remote(backend, guild_id, params: Option<SearchParams>)`.
        params: buildRemoteParams(requestOffset),
      });
      if (seq !== requestSeq) return;
      remoteState = "available";
      mode = "remote";
      if (isIndexingResponse(response)) {
        handleIndexingResponse(response, append);
        return;
      }
      cancelIndexingPoll();
      indexingAttempts = 0;
      const raw = flattenServerMessages(response?.messages);
      for (const value of raw) {
        const message = normalizeRemoteMessage(value);
        if (remoteIds.has(message.key)) continue;
        remoteIds.add(message.key);
        remoteMessages.push(message);
      }
      const total = Number(response?.total_results);
      remoteTotal = Number.isFinite(total)
        ? Math.max(remoteMessages.length, Math.max(0, total))
        : remoteMessages.length;
      deepIndex = response?.doing_deep_historical_index === true;
      const indexedCount = Number(response?.documents_indexed);
      documentsIndexed = Number.isFinite(indexedCount) && indexedCount >= 0 ? indexedCount : 0;
      remoteOffset = requestOffset + raw.length;
      if (raw.length === 0 || remoteTotal <= remoteMessages.length || requestOffset >= MAX_SEARCH_OFFSET) {
        remoteExhausted = true;
      }
      // Страница целиком из дублей: следующий offset не даст нового.
      if (append && remoteMessages.length === previous) remoteExhausted = true;
      renderRemote();
    } catch (error) {
      if (seq !== requestSeq) return;
      cancelIndexingPoll();
      if (isSearchError(error)) {
        renderRemoteError(error);
      } else {
        // IPC-команды нет/она недоступна (демо, старый бэкенд) — честный fallback.
        remoteState = "unavailable";
        mode = "local";
        localReason = "ipc";
        renderLocal();
      }
    } finally {
      if (seq === requestSeq) remoteLoading = false;
      endRequest();
    }
  }

  /**
   * Пересчёт выдачи. Ошибки фильтра — `UIError` + видимый статус (не throw).
   * @returns {number[]}
   */
  function refresh() {
    try {
      readControls();
      // §5.3.1: двусторонняя синхронизация — инлайн-токены заполняют пустые
      // контролы `.search-sections`, затем значения читаются обратно.
      syncInlineTokensToControls(parseSearchQueryTokens(query));
      readControls();
      syncFilterChips();
      if (filters.date && !isIsoDate(filters.date)) {
        throw new UIError("ACTION_FAILED", `дата «${filters.date}» не в формате ГГГГ-ММ-ДД`);
      }
      // M-13 NSFW Gate (§4.2): фильтр «В» на 18+ канал, NSFW-гильдия
      // (`nsfw_level ∈ {1, 3}`) или сам открытый канал 18+ — до согласия
      // показываем баннер и очищаем выдачу.
      const parsed = parseSearchQueryTokens(query);
      const inlineIn = parsed.chips.find((c) => c.key === "in" && !c.negated)?.value ?? "";
      const targetIn = filters.in !== "" ? filters.in : inlineIn;
      const guildNsfw = [1, 3].includes(Number(nsfwContext.guild?.nsfw_level ?? nsfwContext.guild?.nsfwLevel));
      const channelNsfw = currentChannelIsNsfw();
      const channelTarget = targetIn !== "" && isChannelNsfw(targetIn);
      if ((channelTarget || guildNsfw || channelNsfw) && !currentDidAgreeNsfw()) {
        cancelIndexingPoll();
        results.replaceChildren();
        showNsfwGate(resolveNsfwGateMode(), channelTarget || channelNsfw ? "channel" : "guild");
        return lastMatches;
      }
      hideNsfwGate();

      if (hasRemoteCriteria() && canRemote()) {
        mode = "remote";
        void fetchRemote(false);
        return lastMatches;
      }
      // Пустой запрос — локальный обзор окна; непустой здесь означает, что
      // сетевой команды нет/она недоступна (fallback с видимой пометкой).
      cancelIndexingPoll();
      requestSeq += 1;
      remoteLoading = false;
      mode = "local";
      // Пустой guildId при живой IPC — не «команда недоступна», а локальный
      // обзор: сервер ещё не выбран (рейл пуст после boot).
      localReason = hasRemoteCriteria() && (invoke === null || remoteState === "unavailable")
        ? "ipc"
        : "preview";
      return renderLocal();
    } catch (error) {
      const reported = asUIError("ACTION_FAILED", error);
      reportError(reported.code, reported);
      setStatus(`Ошибка: ${reported.message}`, "error");
      onError(reported);
      return lastMatches;
    }
  }

  /** @param {boolean} [returnFocus] */
  function show(returnFocus = false) {
    panel.setAttribute("data-open", "true");
    panel.setAttribute("aria-hidden", "false");
    if (returnFocus) {
      /** @type {any} */ (input)?.focus?.();
    }
    refresh();
  }

  /** @param {boolean} [returnFocus] */
  function close(returnFocus = false) {
    cancelIndexingPoll();
    panel.setAttribute("data-open", "false");
    panel.setAttribute("aria-hidden", "true");
    setMenu(false);
    if (returnFocus) {
      /** @type {any} */ (header)?.focus?.();
    }
  }

  /** @returns {boolean} */
  const isOpen = () => panel.getAttribute("data-open") === "true";

  input.addEventListener("input", () => {
    query = input.value;
    slice = SEARCH_SLICE;
    cancelIndexingPoll();
    indexingAttempts = 0;
    show();
  });
  closeButton.addEventListener("click", () => close(true));

  if (header) {
    header.addEventListener("input", () => {
      query = header.value;
      input.value = header.value;
      slice = SEARCH_SLICE;
      cancelIndexingPoll();
      indexingAttempts = 0;
      show();
    });
  }

  // Начальное состояние: выдача по пустому запросу не нужна до первого открытия.
  panel.setAttribute("data-open", "false");
  setStatus(`найдено 0 из 0 загруженных (всего ${formatNumber(chat.store.totalRows)})`);
  status.setAttribute("data-state", "ok");

  return {
    panel,
    input,
    status,
    /** Пересчитать выдачу по текущим контролам. */
    refresh,
    /** @param {string} value */
    setQuery(value) {
      query = value;
      input.value = value;
      if (header) header.value = value;
      cancelIndexingPoll();
      indexingAttempts = 0;
      return refresh();
    },
    /**
     * M-13: Установка контекста NSFW для гейта 18+ (`didAgreeNsfwGuild`).
     * @param {{
     *   guild?: any,
     *   user?: any,
     *   regionalFeatureConfig?: any,
     *   channels?: Array<{ id: string, name?: string, nsfw?: boolean }>,
     *   agreedStore?: Record<string, boolean>,
     * }} context
     */
    setNsfwContext(context) {
      nsfwContext = context && typeof context === "object" ? context : {};
    },
    /**
     * M-13: Загрузка результатов по явному `offset` с проверкой потолка `MAX_SEARCH_OFFSET = 9975`.
     * @param {number} offset
     * @returns {Promise<void>}
     */
    loadAtOffset(offset) {
      return fetchRemote(false, false, offset);
    },
    /**
     * Программный фильтр (диагностика/тесты): значения пишутся в контролы,
     * затем та же валидация и тот же разбор, что у пользовательского ввода.
     * @param {Partial<Filters>} patch
     */
    applyFilter(patch) {
      if (patch.date !== undefined && patch.date !== "" && !isIsoDate(patch.date)) {
        throw new UIError("ACTION_FAILED", `дата «${patch.date}» не в формате ГГГГ-ММ-ДД`);
      }
      for (const [key, value] of Object.entries(patch)) {
        if (value === undefined) continue;
        const control = field(key);
        if (!control) continue;
        if (key === "pinned") {
          const on = value === true;
          control.setAttribute("aria-checked", on ? "true" : "false");
          control.textContent = on ? "Вкл" : "Выкл";
          continue;
        }
        if (typeof control.value === "string") control.value = String(value);
      }
      return refresh();
    },
    /** Открыть панель (фокус — в поле запроса). @param {boolean} [returnFocus] */
    show,
    close,
    /** @returns {boolean} */
    get open() {
      return isOpen();
    },
    /** @returns {number[]} последняя выдача (индексы, уже отсортированные) */
    get matches() {
      return [...lastMatches];
    },
    /** @returns {number} */
    get matchesCount() {
      return lastMatches.length;
    },
    /** @returns {string} */
    get sort() {
      return sortKey;
    },
    /** `local`/`remote` — куда ушла текущая выдача (для UI_SNAPSHOT/диагностики). */
    /** @returns {"local" | "remote"} */
    get mode() {
      return mode;
    },
    /** @returns {number} `total_results` последнего серверного ответа */
    get remoteTotal() {
      return remoteTotal;
    },
    /** @returns {number} сколько уникальных серверных сообщений накоплено */
    get remoteCount() {
      return remoteMessages.length;
    },
    /** Дождаться завершения текущих серверных запросов (тесты/диагностика). */
    whenIdle,
  };
}

/** @typedef {ReturnType<typeof createSearchOverlay>} SearchOverlay */
