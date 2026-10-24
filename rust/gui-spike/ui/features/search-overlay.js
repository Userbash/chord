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
 * Пять фильтров вне подтверждённых
 * серверных параметров применяются к уже загруженным результатам и честно
 * помечены «локально» (ATTACH-RE §1: точные REST-параметры — gap).
 *
 * Панель не модальная (SRCH-02): чат остаётся доступен, Esc/крестик закрывают.
 */
import { byId, formatNumber } from "./dom.js";
import { UIError, asUIError, reportError } from "./errors.js";
import { HAS_OPTIONS, extrasFor } from "./rows.js";

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

/** B7b: секции, у которых нет подтверждённого REST-параметра. */
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
  return /^\d{4}-\d{2}-\d{2}$/.test(value) && !Number.isNaN(Date.parse(value));
}

/**
 * Сообщение, пришедшее с сервера (после нормализации `messages`).
 * @typedef {{
 *   key: string, id: string, author: string, text: string, timestamp: string,
 *   date: string, time: string, channel: string, pinned: boolean, bot: boolean,
 *   raw: any,
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
  const text = String(message.content ?? "");
  return {
    key: id !== "" ? id : `${timestamp}|${authorName}|${text}`,
    id,
    author: authorName,
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

  /* ---------- статус и результаты ---------- */
  const status = document.createElement("p");
  status.className = "search-status";
  status.setAttribute("aria-live", "polite");
  status.setAttribute("data-state", "ok");
  status.textContent = "";

  const results = document.createElement("div");
  results.className = "search-results";

  panel.append(head, sections, sortBox, status, results, createFootnote());
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

  /* ---------- B7b: сетевое состояние ---------- */
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
  let remoteExhausted = false;
  let deepIndex = false;
  /** Незавершённые запросы (для `whenIdle()` в тестах/диагностике). */
  let inflight = 0;
  /** @type {Array<() => void>} */
  let idleWaiters = [];

  function beginRequest() {
    inflight += 1;
  }

  function endRequest() {
    inflight = Math.max(0, inflight - 1);
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
   * B7b: только подтверждённые ATTACH-RE §1 параметры. Имя канала UI пока не
   * умеет превращать в snowflake (в дампе нет каталога id) — уходит как есть;
   * числовой id работает штатно, произвольный текст вернёт видимую ошибку.
   * @param {number} offset
   * @returns {{ content: string, channelId?: string, sortBy: string, sortOrder: string, offset: number }}
   */
  function buildRemoteParams(offset) {
    const sort = REMOTE_SORTS[sortKey] ?? { sortBy: "timestamp", sortOrder: "desc" };
    /** @type {{ content: string, channelId?: string, sortBy: string, sortOrder: string, offset: number }} */
    const params = {
      content: query.trim(),
      sortBy: sort.sortBy,
      sortOrder: sort.sortOrder,
      offset,
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
   * «Содержит» для серверного сообщения: работаем только с приложенными
   * полями (embeds/attachments/reactions); нет данных — честный false.
   * @param {string} has
   * @param {RemoteMessage} message
   * @returns {boolean}
   */
  function remoteHasContent(has, message) {
    const target = has.toLowerCase();
    if (target === "") return true;
    const raw = /** @type {any} */ (message.raw ?? {});
    const embeds = /** @type {any[]} */ (Array.isArray(raw.embeds) ? raw.embeds : []);
    const attachments = /** @type {any[]} */ (Array.isArray(raw.attachments) ? raw.attachments : []);
    const reactions = /** @type {any[]} */ (Array.isArray(raw.reactions) ? raw.reactions : []);
    if (target === "ссылка") {
      return embeds.length > 0 || attachments.some((item) => typeof item?.url === "string");
    }
    if (target === "картинка") {
      return embeds.some((item) => String(item?.type ?? "") === "image")
        || attachments.some((item) => String(item?.content_type ?? "").startsWith("image/"));
    }
    if (target === "эмодзи") return reactions.length > 0;
    return false;
  }

  /**
   * Пять фильтров без подтверждённого REST-параметра применяются локально
   * к уже полученным серверным результатам (честная подпись «локально»).
   * @param {RemoteMessage[]} list
   * @returns {RemoteMessage[]}
   */
  function applyRemoteFilters(list) {
    return list.filter((message) => {
      const author = message.author.toLowerCase();
      if (filters.from && !author.includes(filters.from.toLowerCase())) return false;
      if (filters.mentions && !author.includes(filters.mentions.toLowerCase())) return false;
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
    return refresh();
  }

  /** @param {string} text @param {"ok" | "error" | "loading"} [state] */
  function setStatus(text, state = "ok") {
    status.textContent = text;
    status.setAttribute("data-state", state);
  }

  /** Локальный скан загруженного окна (ADR-1) — fallback и пустой запрос. */
  function renderLocal() {
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
    if (!remoteExhausted && remaining > 0) {
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
    setStatus(parts.join(" · "));
  }

  /**
   * `SearchError` (401/403/404/4xx/429/5xx/сеть) — видимая ошибка. Локальный
   * fallback здесь запрещён: иначе пользователь примет неполную выдачу за
   * серверную, а «нет доступа» превратится в «команда недоступна».
   * @param {any} error
   */
  function renderRemoteError(error) {
    const text = remoteErrorText(error);
    const reported = new UIError("ACTION_FAILED", `поиск: ${text}`);
    reportError(reported.code, reported);
    setStatus(`Ошибка: ${text}`, "error");
    results.replaceChildren(emptyNode("Не удалось выполнить поиск"));
    onError(reported);
  }

  /**
   * @param {boolean} append true — следующая страница по offset
   * @returns {Promise<void>}
   */
  async function fetchRemote(append) {
    // Снимок на запрос: выбранный сервер не меняется посреди страницы.
    const guildId = readGuildId();
    if (invoke === null || guildId === "") return;
    if (append && (remoteLoading || remoteExhausted)) return;
    const previous = remoteMessages.length;
    const requestOffset = append ? remoteOffset : 0;
    if (!append) {
      remoteMessages = [];
      remoteIds = new Set();
      remoteTotal = 0;
      remoteOffset = 0;
      remoteExhausted = false;
      deepIndex = false;
    }
    const seq = ++requestSeq;
    remoteLoading = true;
    beginRequest();
    renderLoading(append);
    try {
      const response = await invoke("search_remote", {
        guildId,
        // Контракт B7a: `search_remote(backend, guild_id, params: Option<SearchParams>)`.
        params: buildRemoteParams(requestOffset),
      });
      if (seq !== requestSeq) return;
      remoteState = "available";
      mode = "remote";
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
      remoteOffset = requestOffset + raw.length;
      if (raw.length === 0 || remoteTotal <= remoteMessages.length) remoteExhausted = true;
      // Страница целиком из дублей: следующий offset не даст нового.
      if (append && remoteMessages.length === previous) remoteExhausted = true;
      renderRemote();
    } catch (error) {
      if (seq !== requestSeq) return;
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
      if (filters.date && !isIsoDate(filters.date)) {
        throw new UIError("ACTION_FAILED", `дата «${filters.date}» не в формате ГГГГ-ММ-ДД`);
      }
      if (hasRemoteCriteria() && canRemote()) {
        mode = "remote";
        void fetchRemote(false);
        return lastMatches;
      }
      // Пустой запрос — локальный обзор окна; непустой здесь означает, что
      // сетевой команды нет/она недоступна (fallback с видимой пометкой).
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
    show();
  });
  closeButton.addEventListener("click", () => close(true));

  if (header) {
    header.addEventListener("input", () => {
      query = header.value;
      input.value = header.value;
      slice = SEARCH_SLICE;
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
      return refresh();
    },
    /**
     * Программный фильтр (диагностика/тесты): та же валидация, что у полей.
     * @param {Partial<Filters>} patch
     */
    applyFilter(patch) {
      if (patch.date !== undefined && patch.date !== "" && !isIsoDate(patch.date)) {
        throw new UIError("ACTION_FAILED", `дата «${patch.date}» не в формате ГГГГ-ММ-ДД`);
      }
      const dateField = field("date");
      if (patch.date !== undefined && dateField) dateField.value = patch.date;
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
