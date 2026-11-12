// @ts-check
/**
 * Реакции сообщений: базовый набор из `rows.js` + локальные переключения,
 * а также UI-контроллер реакций `createReactionsController` (FE-P0-03 / FE-P0-04,
 * task-149: 9-категорийный `#reaction-picker-popover` и модалка `#reaction-users-modal`).
 *
 * Состояние живёт вне DOM (Map по ключу строки), потому что строки
 * виртуализованы и переиспользуются: после скролла туда-обратно своя реакция
 * обязана остаться. Ключ — либо числовой индекс bench-строки, либо
 * `channel:<channelId>:<messageId>` для реального канала: синтетическое окно
 * и `messages_page` не делят состояние (M-06).
 *
 * Состояние меняется оптимистично, а transport откатывает его при ошибке IPC.
 */
import { QUICK_REACTIONS, baseReactions, extrasFor } from "./rows.js";
import { UIError } from "./errors.js";

/** @typedef {import("./rows.js").Reaction} Reaction */
/** @typedef {number | string} ReactionKey */
/** @typedef {(cmd: string, args?: any) => Promise<any> | any} Invoke */

/** Обычная реакция Discord API (`type=0`). */
export const REACTION_TYPE_NORMAL = 0;

/** Супер-реакция / burst Discord API (`type=1`). */
export const REACTION_TYPE_BURST = 1;

/**
 * 9 стандартных категорий эмодзи для поповера реакций (`#reaction-picker-popover`,
 * контракт FE-P0-03 / FE-P0-04).
 * @type {ReadonlyArray<{ id: string, name: string, icon: string, items: readonly string[] }>}
 */
export const REACTION_CATEGORIES = Object.freeze([
  Object.freeze({
    id: "recent",
    name: "Часто используемые",
    icon: "🕒",
    items: Object.freeze(["👍", "❤️", "😂", "😮", "😢", "🙏", "🔥", "🎉", "✅", "👀", "💯", "🚀"]),
  }),
  Object.freeze({
    id: "people",
    name: "Люди",
    icon: "😀",
    items: Object.freeze(["😀", "😃", "😄", "😁", "😅", "🤣", "🙂", "😉", "😊", "😍", "🥳", "🤔", "😎", "👋", "👏", "🙌"]),
  }),
  Object.freeze({
    id: "nature",
    name: "Природа",
    icon: "🌿",
    items: Object.freeze(["🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🦁", "🐸", "🌸", "🌺", "🌻", "🌲", "🌿", "🍀"]),
  }),
  Object.freeze({
    id: "food",
    name: "Еда",
    icon: "🍔",
    items: Object.freeze(["🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🍒", "🍑", "🍕", "🍔", "🍟", "🌭", "🍿", "☕"]),
  }),
  Object.freeze({
    id: "activities",
    name: "Активности",
    icon: "⚽",
    items: Object.freeze(["⚽", "🏀", "🏈", "⚾", "🎾", "🏐", "🎱", "🏓", "🏸", "🥊", "🎮", "🎲", "🎯", "🎳", "🏆", "🥇"]),
  }),
  Object.freeze({
    id: "travel",
    name: "Путешествия",
    icon: "🚀",
    items: Object.freeze(["🚗", "🚕", "🚙", "🚌", "🏎️", "🚓", "🚑", "🚒", "🚲", "✈️", "🚀", "🛸", "🚁", "⛵", "🚂", "🗺️"]),
  }),
  Object.freeze({
    id: "objects",
    name: "Предметы",
    icon: "💡",
    items: Object.freeze(["⌚", "📱", "💻", "⌨️", "🖥️", "🖨️", "🖱️", "📷", "🎥", "📞", "💡", "🔦", "📚", "✏️", "🔑", "🔒"]),
  }),
  Object.freeze({
    id: "symbols",
    name: "Символы",
    icon: "❤️",
    items: Object.freeze(["❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "💔", "❣️", "💕", "✨", "⭐", "🌟", "💫", "⚡"]),
  }),
  Object.freeze({
    id: "flags",
    name: "Флаги",
    icon: "🏳️",
    items: Object.freeze(["🏁", "🚩", "🎌", "🏴", "🏳️", "🏳️‍🌈", "🏴‍☠️", "🇺🇳", "🇷🇺", "🇺🇸", "🇬🇧", "🇩🇪", "🇫🇷", "🇯🇵", "🇨🇳", "🇧🇷"]),
  }),
]);

/**
 * Переопределения по ключу строки: emoji → { count, me }.
 * @type {Map<ReactionKey, Map<string, { count: number, me: boolean }>>}
 */
const overrides = new Map();
const pending = new Set();
const KNOWN_REACTIONS = new Set(["🔥", "✅", "👍", "❤️", "😂", "😮"]);
/** @type {((cmd: string, args: any) => Promise<any>) | null} */
let invokeReaction = null;
/** @type {(error: UIError) => void} */
let reportReactionError = () => {};

/** @param {(cmd: string, args: any) => Promise<any> | any} [invoke] @param {(error: UIError) => void} [onError] */
export function configureReactionTransport(invoke, onError) {
  invokeReaction = invoke ? (cmd, args) => Promise.resolve(invoke(cmd, args)) : null;
  reportReactionError = onError ?? (() => {});
}

/**
 * Ключ состояния реального канала: не пересекается с числовыми bench-индексами.
 * @param {string} channelId
 * @param {string} messageId
 * @returns {string}
 */
export function channelReactionKey(channelId, messageId) {
  return `channel:${channelId}:${messageId}`;
}

/**
 * @param {unknown} key
 * @returns {key is ReactionKey}
 */
function isReactionKey(key) {
  if (typeof key === "number") return Number.isInteger(key) && key >= 0;
  return typeof key === "string" && key.trim() !== "";
}

/**
 * `messages_page`/фикстуры могут прислать мусор: невалидная запись не должна
 * ронять строку, но и не притворяется реакцией с выдуманным emoji.
 * @param {unknown} value
 * @returns {Reaction | null}
 */
function normalizeReaction(value) {
  if (value === null || typeof value !== "object") return null;
  const bag = /** @type {Record<string, unknown>} */ (value);
  const emoji = typeof bag.emoji === "string" ? bag.emoji.trim() : "";
  if (emoji === "") return null;
  const raw = typeof bag.count === "number" && Number.isFinite(bag.count) ? bag.count : 0;
  return {
    emoji,
    count: Math.max(0, Math.trunc(raw)),
    me: bag.me === true,
  };
}

/**
 * @param {unknown} value
 * @returns {Reaction[]}
 */
function normalizeBase(value) {
  if (!Array.isArray(value)) return [];
  /** @type {Reaction[]} */
  const result = [];
  for (const entry of value) {
    const reaction = normalizeReaction(entry);
    if (reaction) result.push(reaction);
  }
  return result;
}

/**
 * @param {ReactionKey} key
 * @returns {Reaction[]}
 */
function defaultBase(key) {
  return typeof key === "number" ? baseReactions(key) : [];
}

/**
 * @param {ReactionKey} key
 * @param {Reaction[]} [base] видимый базовый набор строки (пустой = реакций нет)
 * @returns {Reaction[]}
 */
export function reactionsFor(key, base = /** @type {Reaction[]} */ (defaultBase(key))) {
  const visible = normalizeBase(base);
  const local = overrides.get(key);
  if (!local) return visible;
  /** @type {Reaction[]} */
  const result = visible.map((reaction) => {
    const patch = local.get(reaction.emoji);
    return patch ? { ...reaction, ...patch } : reaction;
  });
  for (const [emoji, patch] of local) {
    if (!result.some((reaction) => reaction.emoji === emoji)) result.push({ emoji, ...patch });
  }
  return result;
}

/**
 * Переключает свою реакцию. Возвращает новое состояние (`true` — поставлена).
 * @param {ReactionKey} key индекс bench-строки или `channel:<id>:<id>`
 * @param {string} emoji
 * @param {Reaction[]} [base] видимый базовый набор строки
 * @param {{ allowUnknown?: boolean, burst?: boolean }} [options] `allowUnknown` — эмодзи из
 *   реального сообщения (кастомный emoji), а не из быстрого набора панели
 * @returns {boolean}
 */
export function toggleReaction(key, emoji, base = /** @type {Reaction[]} */ (defaultBase(key)), options = {}) {
  if (!isReactionKey(key)) {
    throw new UIError("ACTION_FAILED", `toggleReaction: ключ строки ${String(key)} вне окна`);
  }
  if (typeof emoji !== "string" || emoji.trim() === "") {
    throw new UIError("ACTION_FAILED", "toggleReaction: пустая реакция");
  }
  if (!options.allowUnknown && !KNOWN_REACTIONS.has(emoji)) {
    throw new UIError("ACTION_FAILED", `toggleReaction: неизвестная реакция «${emoji}»`);
  }
  let local = overrides.get(key);
  if (!local) {
    local = new Map();
    overrides.set(key, local);
  }
  const current = reactionsFor(key, base).find((reaction) => reaction.emoji === emoji);
  const count = current ? current.count : 0;
  const me = current ? current.me : false;
  const next = { count: Math.max(0, count + (me ? -1 : 1)), me: !me };
  local.set(emoji, next);
  return next.me;
}

/**
 * Optimistic переключатель с одним IPC-запросом на пару строка/emoji.
 * Для bench (`key` — число) адрес остаётся `"news"`/индекс; для реального
 * канала вызывающий обязан передать `channelId`/`messageId` из `messages_page`.
 * @param {ReactionKey} key
 * @param {string} emoji
 * @param {Reaction[]} base
 * @param {{
 *   element?: HTMLElement | null,
 *   channelId?: string,
 *   messageId?: string,
 *   burst?: boolean,
 *   invoke?: Invoke | null,
 *   onError?: ((error: UIError) => void) | null,
 *   onToggleReaction?: ((payload: {
 *     action: "add_reaction" | "remove_reaction",
 *     channelId: string,
 *     messageId: string,
 *     key: ReactionKey,
 *     emoji: string,
 *     active: boolean,
 *     added: boolean,
 *     burst: boolean,
 *     type: 0 | 1,
 *   }) => void | Promise<any>) | null,
 *   onSettled?: () => void,
 * }} [options]
 * @returns {Promise<void>}
 */
export async function requestReaction(key, emoji, base, options = {}) {
  const requestKey = `${String(key)}:${emoji}`;
  if (pending.has(requestKey)) return;
  const before = reactionsFor(key, base).find((reaction) => reaction.emoji === emoji);
  const burst = options.burst === true;
  let active;
  try {
    active = toggleReaction(key, emoji, base, { allowUnknown: true, burst });
  } catch (error) {
    const errFn = options.onError ?? reportReactionError;
    errFn(error instanceof UIError ? error : new UIError("ACTION_FAILED", String(error), error));
    return;
  }

  const element = options.element ?? null;
  if (element) {
    element.classList.toggle("active", active);
    if (burst && active) {
      element.setAttribute("data-burst", "true");
      element.setAttribute("data-reaction-type", "1");
      element.classList.add("reaction--burst");
    } else if (!active) {
      element.removeAttribute("data-burst");
      element.setAttribute("data-reaction-type", "0");
      element.classList.remove("reaction--burst");
    }
  }

  const channelId = options.channelId ?? "news";
  const messageId = options.messageId ?? String(key);

  if (options.onToggleReaction) {
    await options.onToggleReaction({
      action: active ? "add_reaction" : "remove_reaction",
      channelId,
      messageId,
      key,
      emoji,
      active,
      added: active,
      burst,
      type: burst ? REACTION_TYPE_BURST : REACTION_TYPE_NORMAL,
    });
  }

  const invoker = options.invoke !== undefined ? options.invoke : invokeReaction;
  pending.add(requestKey);
  element?.setAttribute("data-pending", "true");
  if (!invoker) {
    pending.delete(requestKey);
    element?.removeAttribute("data-pending");
    options.onSettled?.();
    return;
  }
  try {
    await invoker("toggle_reaction", {
      // Явный реальный адрес приоритетнее bench-значений по умолчанию.
      channelId,
      messageId,
      emoji,
      active,
    });
  } catch (error) {
    const current = overrides.get(key);
    if (before) current?.set(emoji, { count: before.count, me: before.me });
    else current?.delete(emoji);
    if (current && current.size === 0) overrides.delete(key);
    if (element) {
      element.classList.toggle("active", before?.me ?? false);
    }
    const details = /** @type {any} */ (error);
    const status = details?.status ?? details?.code;
    const kindStr = String(details?.kind ?? "");
    const message = status === 401 || kindStr === "Unauthorized" || kindStr === "unauthorized"
      ? "сессия истекла"
      : status === 403 || kindStr === "Forbidden" || kindStr === "forbidden"
        ? "нет прав на добавление реакций в этом канале"
        : status === 429 || kindStr === "RateLimited" || kindStr === "rate_limited"
          ? `ограничение частоты; повтор через ${details?.retry_after_ms ?? details?.retryAfterMs ?? 0} мс`
          : "сеть недоступна";
    const errFn = options.onError ?? reportReactionError;
    errFn(new UIError("ACTION_FAILED", message, error));
  } finally {
    pending.delete(requestKey);
    element?.removeAttribute("data-pending");
    options.onSettled?.();
  }
}

/**
 * L-02-UI: аддитивная правка live-счётчика. Вызывается из `chat.applyReaction`
 * после dispatch-события, чтобы следующий `refreshRow` (виртуализация/скролл/ack)
 * не перетёр пилюлю старым optimistic-состоянием (PLAN §6, риск R7).
 *
 * `delta` — разница между новым видимым счётчиком и текущим; `base` — базовый
 * набор строки из `messages_page`, чтобы patch нёс абсолютный count.
 * @param {ReactionKey} key
 * @param {string} emoji
 * @param {number} delta
 * @param {boolean} me
 * @param {Reaction[]} [base]
 * @returns {boolean}
 */
export function adjustReactionOverride(
  key,
  emoji,
  delta,
  me,
  base = /** @type {Reaction[]} */ (defaultBase(key)),
) {
  if (!isReactionKey(key)) return false;
  if (typeof emoji !== "string" || emoji.trim() === "") return false;
  const visible = reactionsFor(key, base).find((reaction) => reaction.emoji === emoji);
  const step = Number.isFinite(delta) ? Math.trunc(delta) : 0;
  const count = Math.max(0, (visible?.count ?? 0) + step);
  let local = overrides.get(key);
  if (count <= 0 && !me) {
    // Ноль без собственной реакции — пилюли нет: override не должен её воскрешать.
    if (!local) return true;
    local.delete(emoji);
    if (local.size === 0) overrides.delete(key);
    return true;
  }
  if (!local) {
    local = new Map();
    overrides.set(key, local);
  }
  local.set(emoji, { count, me: Boolean(me) });
  return true;
}

/** Сброс локального состояния (используется тестами/диагностикой). */
export function resetReactions() {
  overrides.clear();
  pending.clear();
}

/**
 * Разобрать список пользователей реакции из IPC или тестового стаба.
 * @param {unknown} raw
 * @param {string} emoji
 * @param {0 | 1} type
 * @returns {Array<{ id: string, username: string, displayName: string }>}
 */
function normalizeReactionUsers(raw, emoji, type) {
  if (Array.isArray(raw)) {
    /** @type {Array<{ id: string, username: string, displayName: string }>} */
    const result = [];
    for (const item of raw) {
      if (!item || typeof item !== "object") continue;
      const bag = /** @type {Record<string, unknown>} */ (item);
      const id = String(bag.id ?? bag.userId ?? "");
      const username = String(bag.username ?? bag.name ?? (id || "user"));
      const displayName = String(bag.global_name ?? bag.globalName ?? bag.displayName ?? username);
      result.push({ id: id || username, username, displayName });
    }
    return result;
  }
  if (type === REACTION_TYPE_BURST) {
    return [{ id: "u-burst-default", username: "user", displayName: "User" }];
  }
  return [
    { id: "u-1", username: "sasha", displayName: `sasha (${emoji})` },
    { id: "u-2", username: "mira", displayName: "mira" },
  ];
}

/**
 * Контроллер интерактивных реакций в чате (FE-P0-03 / FE-P0-04, task-149):
 * - кликабельные пилюли реакций под сообщением (`.reactions-row .reaction`) с поддержкой
 *   `burst=false/true`, оптимистичным обновлением счётчика и класса `.active`;
 * - всплывающая панель действий над сообщением (`.message-action-toolbar` / `.msg-hover-toolbar`)
 *   с быстрыми реакциями, открытием 9-категорийного пикера (`#reaction-picker-popover`, `role="dialog"`)
 *   и кнопкой ответа;
 * - модалка «Кто поставил реакцию» (`#reaction-users-modal`, вкладки «Обычные» `type=0`
 *   и «Супер-реакции» `type=1`) из контекстного меню или действия пилюли реакции;
 * - полная очистка обработчиков и временных DOM-узлов при перерисовке и `destroy()`.
 *
 * @param {{
 *   list?: HTMLElement | null,
 *   container?: HTMLElement | null,
 *   channelId?: string | (() => string),
 *   invoke?: Invoke | null,
 *   onError?: ((error: UIError) => void) | null,
 *   onToggleReaction?: ((payload: {
 *     action: "add_reaction" | "remove_reaction",
 *     channelId: string,
 *     messageId: string,
 *     key: ReactionKey,
 *     emoji: string,
 *     active: boolean,
 *     added: boolean,
 *     burst: boolean,
 *     type: 0 | 1,
 *   }) => void | Promise<any>) | null,
 *   onFetchReactionUsers?: ((query: {
 *     channelId: string,
 *     messageId: string,
 *     emoji: string,
 *     type: 0 | 1,
 *   }) => any) | null,
 *   onReply?: ((payload: { index: number, messageId: string, channelId: string }) => void) | null,
 *   renderRow?: ((index: number) => void) | null,
 *   reactionTargetOf?: ((index: number) => {
 *     base: Reaction[],
 *     key: ReactionKey,
 *     channelId?: string,
 *     messageId?: string,
 *   }) | null,
 *   categories?: ReadonlyArray<{ id?: string, name: string, icon: string, items: readonly string[] }>,
 * }} [options]
 */
export function createReactionsController(options = {}) {
  const doc = typeof document !== "undefined" ? document : null;
  const listEl = options.list ?? (doc ? doc.getElementById("chat-list") : null);
  const containerEl =
    options.container ??
    (listEl?.parentNode ? /** @type {HTMLElement} */ (listEl.parentNode) : null) ??
    (doc ? doc.body : null);
  const categories = options.categories ?? REACTION_CATEGORIES;

  let destroyed = false;
  /** @type {Array<() => void>} */
  const cleanups = [];
  /** @type {Map<HTMLElement, Array<() => void>>} */
  const rowCleanups = new Map();

  /** @type {HTMLElement | null} */
  let activePopover = null;
  /** @type {HTMLElement | null} */
  let activePopoverTrigger = null;
  /** @type {HTMLElement | null} */
  let activeModal = null;
  /** @type {HTMLElement | null} */
  let activeModalTrigger = null;
  /** @type {HTMLElement | null} */
  let ownToolbar = null;

  /** @returns {string} */
  function resolveChannelId() {
    if (typeof options.channelId === "function") {
      return options.channelId() || "news";
    }
    if (typeof options.channelId === "string" && options.channelId.trim() !== "") {
      return options.channelId;
    }
    return "news";
  }

  /** @returns {Invoke | null} */
  function resolveInvoke() {
    if (options.invoke !== undefined && options.invoke !== null) return options.invoke;
    if (options.onToggleReaction) return null;
    if (invokeReaction) return invokeReaction;
    const win = /** @type {any} */ (typeof window !== "undefined" ? window : null);
    const tauriInvoke = win?.__TAURI__?.core?.invoke;
    return typeof tauriInvoke === "function" ? tauriInvoke : null;
  }

  /**
   * Синхронизировать DOM пилюли реакции.
   * @param {HTMLElement} pill
   * @param {Reaction | undefined} state
   * @param {boolean} [burst]
   */
  function syncPillDom(pill, state, burst = false) {
    if (!state) return;
    pill.setAttribute("data-me", state.me ? "true" : "false");
    pill.setAttribute("aria-pressed", state.me ? "true" : "false");
    pill.classList.toggle("active", state.me);
    pill.setAttribute("aria-label", `Реакция ${state.emoji}, ${state.count}`);
    const countNode = pill.querySelector(".reaction-count");
    if (countNode) countNode.textContent = String(state.count);
    if (burst && state.me) {
      pill.setAttribute("data-burst", "true");
      pill.setAttribute("data-reaction-type", "1");
      pill.classList.add("reaction--burst");
    } else if (!state.me) {
      pill.removeAttribute("data-burst");
      pill.setAttribute("data-reaction-type", "0");
      pill.classList.remove("reaction--burst");
    }
  }

  /**
   * Очистить ранее привязанные обработчики конкретной строки сообщения.
   * @param {HTMLElement} row
   */
  function cleanupRow(row) {
    const fns = rowCleanups.get(row);
    if (!fns) return;
    for (const fn of fns) fn();
    rowCleanups.delete(row);
  }

  /**
   * Привязать обработчики клика и контекстного меню к пилюлям реакций строки.
   * Повторный вызов на той же строке при перерисовке сначала снимает старые слушатели.
   * @param {HTMLElement} row
   * @param {{
   *   channelId?: string,
   *   messageId?: string,
   *   index?: number,
   *   key?: ReactionKey,
   *   base?: Reaction[],
   * }} [meta]
   */
  function bindMessageRow(row, meta = {}) {
    if (destroyed || !row) return;
    cleanupRow(row);

    /** @type {Array<() => void>} */
    const disposers = [];
    const indexAttr = row.getAttribute("data-index");
    const index = meta.index !== undefined
      ? meta.index
      : indexAttr !== null && indexAttr !== ""
        ? Number(indexAttr)
        : 0;
    const isChannelRow = row.getAttribute("data-channel-message") === "true";
    const channelId = meta.channelId ?? resolveChannelId();
    const messageId = meta.messageId ?? row.getAttribute("data-message-id") ?? String(index);
    const key = meta.key !== undefined
      ? meta.key
      : isChannelRow
        ? channelReactionKey(channelId, messageId)
        : index;
    const base = meta.base ?? (Number.isInteger(index) && index >= 0 ? extrasFor(index).reactions : []);

    const pills = row.querySelectorAll(".reaction");
    for (const rawPill of pills) {
      const pill = /** @type {HTMLElement} */ (rawPill);
      const emoji = pill.getAttribute("data-emoji") ?? "";
      if (!emoji) continue;
      if (pill.getAttribute("data-me") === "true") {
        pill.classList.add("active");
      }

      // Заменяем первичные слушатели пилюли на управляемые контроллером, чтобы
      // перерисовка и destroy() гарантированно снимали обработчики без дублей.
      const shimListeners = /** @type {Map<string, Array<(e: any) => void>> | undefined} */ (
        /** @type {any} */ (pill).listeners
      );
      if (shimListeners) {
        shimListeners.delete("click");
        shimListeners.delete("contextmenu");
      }

      /** @param {any} event */
      const onPillClick = (event) => {
        if (destroyed) return;
        const burst = Boolean(event?.shiftKey) || (pill.getAttribute("data-burst") === "true" && pill.getAttribute("data-me") !== "true");
        void requestReaction(key, emoji, base, {
          channelId,
          messageId,
          element: pill,
          burst,
          invoke: resolveInvoke(),
          onError: options.onError,
          onToggleReaction: options.onToggleReaction ?? null,
          onSettled: () => {
            if (!destroyed) options.renderRow?.(index);
          },
        });
        const next = reactionsFor(key, base).find((item) => item.emoji === emoji);
        syncPillDom(pill, next, burst);
      };

      /** @param {any} event */
      const onPillContextMenu = (event) => {
        if (destroyed) return;
        event?.preventDefault?.();
        openReactionUsersModal({
          channelId,
          messageId,
          emoji,
          type: pill.getAttribute("data-burst") === "true" ? REACTION_TYPE_BURST : REACTION_TYPE_NORMAL,
          trigger: pill,
          reactions: reactionsFor(key, base),
        });
      };

      pill.addEventListener("click", onPillClick);
      pill.addEventListener("contextmenu", onPillContextMenu);
      disposers.push(() => {
        pill.removeEventListener("click", onPillClick);
        pill.removeEventListener("contextmenu", onPillContextMenu);
      });
    }

    rowCleanups.set(row, disposers);
  }

  /**
   * Закрыть и размонтировать `#reaction-picker-popover` (lazy destroy).
   * @param {{ restoreFocus?: boolean }} [closeOptions]
   */
  function closePicker(closeOptions = {}) {
    if (activePopover) {
      activePopover.remove();
      activePopover = null;
    }
    const existing = doc?.getElementById("reaction-picker-popover");
    if (existing) existing.remove();
    if (activePopoverTrigger) {
      activePopoverTrigger.setAttribute("aria-expanded", "false");
      activePopoverTrigger.removeAttribute("aria-controls");
      if (closeOptions.restoreFocus !== false) {
        activePopoverTrigger.focus?.();
      }
      activePopoverTrigger = null;
    }
  }

  /**
   * Открыть 9-категорийный поповер выбора реакции (`#reaction-picker-popover`, `role="dialog"`).
   * @param {{
   *   channelId?: string,
   *   messageId?: string,
   *   index?: number,
   *   key?: ReactionKey,
   *   row?: HTMLElement | null,
   *   base?: Reaction[],
   *   trigger?: HTMLElement | null,
   * }} [target]
   * @returns {HTMLElement | null}
   */
  function openPicker(target = {}) {
    if (destroyed || !doc) return null;
    closePicker({ restoreFocus: false });

    const index = target.index ?? 0;
    const resolvedTarget = options.reactionTargetOf?.(index);
    const row =
      target.row ??
      (listEl ? /** @type {HTMLElement | null} */ (listEl.querySelector(`.row[data-index="${index}"]`)) : null);
    const channelId = target.channelId ?? resolvedTarget?.channelId ?? resolveChannelId();
    const messageId =
      target.messageId ??
      resolvedTarget?.messageId ??
      row?.getAttribute("data-message-id") ??
      String(index);
    const key =
      target.key ??
      resolvedTarget?.key ??
      (row?.getAttribute("data-channel-message") === "true"
        ? channelReactionKey(channelId, messageId)
        : index);
    const base =
      target.base ??
      resolvedTarget?.base ??
      (Number.isInteger(index) && index >= 0 ? extrasFor(index).reactions : []);

    const trigger = target.trigger ?? null;
    if (trigger) {
      activePopoverTrigger = trigger;
      trigger.setAttribute("aria-expanded", "true");
      trigger.setAttribute("aria-controls", "reaction-picker-popover");
    }

    let activeCatIndex = 0;
    let searchQuery = "";
    let burstMode = false;

    const popover = doc.createElement("div");
    popover.id = "reaction-picker-popover";
    popover.className = "reaction-picker-popover";
    popover.setAttribute("role", "dialog");
    popover.setAttribute("aria-modal", "false");
    popover.setAttribute("aria-label", "Выбор реакции");
    popover.setAttribute("data-state", "ready");
    popover.setAttribute("data-for-message", messageId);

    const head = doc.createElement("div");
    head.className = "reaction-picker-head";
    const searchInput = doc.createElement("input");
    searchInput.id = "reaction-picker-search";
    searchInput.className = "search-input reaction-picker-search";
    searchInput.type = "search";
    searchInput.placeholder = "Найти эмодзи…";
    searchInput.setAttribute("aria-label", "Поиск эмодзи для реакции");

    const burstBtn = doc.createElement("button");
    burstBtn.className = "icon-btn reaction-picker-burst";
    burstBtn.setAttribute("type", "button");
    burstBtn.setAttribute("data-action", "toggle-burst");
    burstBtn.setAttribute("aria-pressed", "false");
    burstBtn.setAttribute("aria-label", "Супер-реакция");
    burstBtn.title = "Супер-реакция (Burst)";
    burstBtn.textContent = "✨";

    const closeBtn = doc.createElement("button");
    closeBtn.className = "icon-btn reaction-picker-close";
    closeBtn.setAttribute("type", "button");
    closeBtn.setAttribute("data-action", "reaction-picker-close");
    closeBtn.setAttribute("aria-label", "Закрыть пикер реакций");
    closeBtn.textContent = "✕";

    head.append(searchInput, burstBtn, closeBtn);

    const body = doc.createElement("div");
    body.className = "reaction-picker-body";

    const tabs = doc.createElement("div");
    tabs.id = "reaction-picker-tabs";
    tabs.className = "reaction-picker-tabs";
    tabs.setAttribute("role", "tablist");
    tabs.setAttribute("aria-label", "Категории эмодзи");
    tabs.setAttribute("aria-orientation", "vertical");

    const grid = doc.createElement("div");
    grid.id = "reaction-picker-grid";
    grid.className = "reaction-picker-grid";
    grid.setAttribute("role", "listbox");
    grid.setAttribute("aria-label", "Эмодзи для реакции");

    /** @type {HTMLElement[]} */
    const tabButtons = [];

    const renderGrid = () => {
      grid.replaceChildren();
      const query = searchQuery.trim().toLowerCase();
      /** @type {string[]} */
      let emojis = [];
      if (query !== "") {
        const seen = new Set();
        for (const cat of categories) {
          if (cat.name.toLowerCase().includes(query)) {
            for (const item of cat.items) {
              if (!seen.has(item)) {
                seen.add(item);
                emojis.push(item);
              }
            }
          } else {
            for (const item of cat.items) {
              if (item.toLowerCase().includes(query) && !seen.has(item)) {
                seen.add(item);
                emojis.push(item);
              }
            }
          }
        }
      } else {
        const activeCat = categories[activeCatIndex] ?? categories[0];
        emojis = activeCat ? [...activeCat.items] : [];
      }

      popover.setAttribute("data-state", emojis.length === 0 ? "empty" : "ready");

      for (const emoji of emojis) {
        const cell = doc.createElement("button");
        cell.className = "emoji-cell reaction-picker-cell";
        cell.setAttribute("type", "button");
        cell.setAttribute("role", "option");
        cell.setAttribute("data-emoji", emoji);
        cell.setAttribute("aria-label", `Реакция ${emoji}`);
        cell.setAttribute("aria-selected", "false");
        cell.title = emoji;
        cell.textContent = emoji;

        /** @param {any} event */
        const onSelectEmoji = (event) => {
          if (destroyed) return;
          const isBurst = burstMode || Boolean(event?.shiftKey);
          const keepOpen = Boolean(event?.shiftKey);
          void requestReaction(key, emoji, base, {
            channelId,
            messageId,
            burst: isBurst,
            invoke: resolveInvoke(),
            onError: options.onError,
            onToggleReaction: options.onToggleReaction ?? null,
            onSettled: () => {
              if (!destroyed) options.renderRow?.(index);
            },
          });

          const currentRow =
            row ??
            (listEl ? /** @type {HTMLElement | null} */ (listEl.querySelector(`.row[data-index="${index}"]`)) : null);
          if (currentRow) {
            options.renderRow?.(index);
            const refreshedRow =
              (listEl ? /** @type {HTMLElement | null} */ (listEl.querySelector(`.row[data-index="${index}"]`)) : null) ??
              currentRow;
            let pill = /** @type {HTMLElement | null} */ (
              refreshedRow.querySelector(`.reaction[data-emoji="${emoji}"]`)
            );
            const updatedState = reactionsFor(key, base).find((entry) => entry.emoji === emoji);
            if (!pill && updatedState && updatedState.count > 0) {
              const textContainer = refreshedRow.querySelector(".text");
              if (textContainer) {
                let group = textContainer.querySelector(".reactions-row");
                if (!group) {
                  group = doc.createElement("div");
                  group.className = "reactions-row";
                  group.setAttribute("role", "group");
                  group.setAttribute("aria-label", "Реакции");
                  textContainer.append(group);
                  textContainer.setAttribute("data-stack", "true");
                }
                pill = doc.createElement("button");
                pill.className = "reaction";
                pill.setAttribute("type", "button");
                pill.setAttribute("data-emoji", emoji);
                const glyph = doc.createElement("span");
                glyph.className = "reaction-emoji";
                glyph.setAttribute("aria-hidden", "true");
                glyph.textContent = emoji;
                const countSpan = doc.createElement("span");
                countSpan.className = "reaction-count";
                countSpan.textContent = String(updatedState.count);
                pill.append(glyph, countSpan);
                group.append(pill);
              }
            }
            if (pill && updatedState) {
              syncPillDom(pill, updatedState, isBurst);
            }
            bindMessageRow(refreshedRow, { channelId, messageId, index, key, base });
          }

          if (!keepOpen) {
            closePicker();
          } else {
            cell.setAttribute("data-reacted", "true");
          }
        };

        cell.addEventListener("click", onSelectEmoji);
        grid.append(cell);
      }
    };

    categories.forEach((cat, catIdx) => {
      const tabBtn = doc.createElement("button");
      tabBtn.id = `reaction-picker-tab-${catIdx}`;
      tabBtn.className = `reaction-picker-tab tab${catIdx === 0 ? " active" : ""}`;
      tabBtn.setAttribute("type", "button");
      tabBtn.setAttribute("role", "tab");
      tabBtn.setAttribute("data-category-index", String(catIdx));
      tabBtn.setAttribute("aria-label", cat.name);
      tabBtn.setAttribute("aria-selected", catIdx === 0 ? "true" : "false");
      tabBtn.setAttribute("tabindex", catIdx === 0 ? "0" : "-1");
      tabBtn.setAttribute("aria-controls", "reaction-picker-grid");
      tabBtn.title = cat.name;
      tabBtn.textContent = cat.icon;

      tabBtn.addEventListener("click", () => {
        if (destroyed) return;
        activeCatIndex = catIdx;
        tabButtons.forEach((btn, i) => {
          const selected = i === activeCatIndex;
          btn.classList.toggle("active", selected);
          btn.setAttribute("aria-selected", selected ? "true" : "false");
          btn.setAttribute("tabindex", selected ? "0" : "-1");
        });
        renderGrid();
      });

      tabButtons.push(tabBtn);
      tabs.append(tabBtn);
    });

    searchInput.addEventListener("input", () => {
      if (destroyed) return;
      searchQuery = searchInput.value;
      renderGrid();
    });

    burstBtn.addEventListener("click", () => {
      if (destroyed) return;
      burstMode = !burstMode;
      burstBtn.setAttribute("aria-pressed", burstMode ? "true" : "false");
      burstBtn.classList.toggle("active", burstMode);
      popover.setAttribute("data-burst-mode", burstMode ? "true" : "false");
    });

    closeBtn.addEventListener("click", () => {
      closePicker();
    });

    /** @param {any} event */
    const onKeyDown = (event) => {
      if (event?.key === "Escape") {
        event.preventDefault?.();
        closePicker();
      }
    };
    popover.addEventListener("keydown", onKeyDown);
    searchInput.addEventListener("keydown", onKeyDown);

    body.append(tabs, grid);
    popover.append(head, body);

    const mountTarget = containerEl ?? doc.body;
    mountTarget.append(popover);
    activePopover = popover;
    renderGrid();
    searchInput.focus?.();
    return popover;
  }

  /**
   * Закрыть и размонтировать модалку `#reaction-users-modal`.
   */
  function closeReactionUsersModal() {
    if (activeModal) {
      activeModal.remove();
      activeModal = null;
    }
    const existing = doc?.getElementById("reaction-users-modal");
    if (existing) existing.remove();
    if (activeModalTrigger) {
      activeModalTrigger.focus?.();
      activeModalTrigger = null;
    }
  }

  /**
   * Открыть модалку «Кто поставил реакцию» (`#reaction-users-modal`, вкладки `type=0` и `type=1`).
   * @param {{
   *   channelId?: string,
   *   messageId?: string,
   *   emoji?: string,
   *   type?: 0 | 1,
   *   trigger?: HTMLElement | null,
   *   reactions?: readonly Reaction[],
   * }} [params]
   * @returns {HTMLElement | null}
   */
  function openReactionUsersModal(params = {}) {
    if (destroyed || !doc) return null;
    closeReactionUsersModal();

    const channelId = params.channelId ?? resolveChannelId();
    const messageId = params.messageId ?? "0";
    const currentEmoji = params.emoji ?? "👍";
    /** @type {0 | 1} */
    let currentType = params.type === REACTION_TYPE_BURST ? REACTION_TYPE_BURST : REACTION_TYPE_NORMAL;
    activeModalTrigger = params.trigger ?? null;

    const modal = doc.createElement("div");
    modal.id = "reaction-users-modal";
    modal.className = "reaction-users-modal";
    modal.setAttribute("role", "dialog");
    modal.setAttribute("aria-modal", "true");
    modal.setAttribute("aria-label", "Кто поставил реакцию");
    modal.setAttribute("data-channel-id", channelId);
    modal.setAttribute("data-message-id", messageId);
    modal.setAttribute("data-emoji", currentEmoji);
    modal.setAttribute("data-reaction-type", String(currentType));

    const header = doc.createElement("div");
    header.className = "reaction-users-head";
    const title = doc.createElement("h2");
    title.className = "reaction-users-title";
    title.textContent = "Кто поставил реакцию";
    const closeBtn = doc.createElement("button");
    closeBtn.className = "icon-btn reaction-users-close";
    closeBtn.setAttribute("type", "button");
    closeBtn.setAttribute("data-action", "reaction-users-close");
    closeBtn.setAttribute("aria-label", "Закрыть");
    closeBtn.textContent = "✕";
    header.append(title, closeBtn);

    const typeTabs = doc.createElement("div");
    typeTabs.className = "reaction-users-type-tabs";
    typeTabs.setAttribute("role", "tablist");
    typeTabs.setAttribute("aria-label", "Тип реакций");

    const normalTab = doc.createElement("button");
    normalTab.className = `reaction-users-tab${currentType === 0 ? " active" : ""}`;
    normalTab.setAttribute("type", "button");
    normalTab.setAttribute("role", "tab");
    normalTab.setAttribute("data-reaction-type", "0");
    normalTab.setAttribute("data-type", "0");
    normalTab.setAttribute("aria-selected", currentType === 0 ? "true" : "false");
    normalTab.textContent = "Обычные";

    const burstTab = doc.createElement("button");
    burstTab.className = `reaction-users-tab${currentType === 1 ? " active" : ""}`;
    burstTab.setAttribute("type", "button");
    burstTab.setAttribute("role", "tab");
    burstTab.setAttribute("data-reaction-type", "1");
    burstTab.setAttribute("data-type", "1");
    burstTab.setAttribute("aria-selected", currentType === 1 ? "true" : "false");
    burstTab.textContent = "Супер-реакции";

    typeTabs.append(normalTab, burstTab);

    const usersList = doc.createElement("div");
    usersList.id = "reaction-users-list";
    usersList.className = "reaction-users-list";
    usersList.setAttribute("role", "list");
    usersList.setAttribute("aria-label", "Пользователи, поставившие реакцию");

    let fetchSeq = 0;
    const loadUsers = async () => {
      const seq = ++fetchSeq;
      modal.setAttribute("data-reaction-type", String(currentType));
      modal.setAttribute("data-emoji", currentEmoji);
      normalTab.classList.toggle("active", currentType === 0);
      normalTab.setAttribute("aria-selected", currentType === 0 ? "true" : "false");
      burstTab.classList.toggle("active", currentType === 1);
      burstTab.setAttribute("aria-selected", currentType === 1 ? "true" : "false");

      /** @type {unknown} */
      let rawUsers = null;
      const invoker = resolveInvoke();
      try {
        if (options.onFetchReactionUsers) {
          rawUsers = await options.onFetchReactionUsers({
            channelId,
            messageId,
            emoji: currentEmoji,
            type: currentType,
          });
        } else if (invoker) {
          rawUsers = await invoker("get_reaction_users", {
            channelId,
            messageId,
            emoji: currentEmoji,
            type: currentType,
          });
        }
      } catch {
        rawUsers = null;
      }
      if (destroyed || seq !== fetchSeq || activeModal !== modal) return;

      const users = normalizeReactionUsers(rawUsers, currentEmoji, currentType);
      usersList.replaceChildren();
      for (const user of users) {
        const item = doc.createElement("div");
        item.className = "reaction-user-row";
        item.setAttribute("role", "listitem");
        item.setAttribute("data-user-id", user.id);

        const avatar = doc.createElement("span");
        avatar.className = "avatar avatar--sm";
        avatar.setAttribute("aria-hidden", "true");
        avatar.textContent = user.displayName.slice(0, 1).toUpperCase();

        const name = doc.createElement("span");
        name.className = "reaction-user-name";
        name.textContent = user.displayName;

        const handle = doc.createElement("span");
        handle.className = "reaction-user-tag";
        handle.textContent = `@${user.username}`;

        item.append(avatar, name, handle);
        usersList.append(item);
      }
    };

    normalTab.addEventListener("click", () => {
      if (destroyed) return;
      currentType = REACTION_TYPE_NORMAL;
      void loadUsers();
    });

    burstTab.addEventListener("click", () => {
      if (destroyed) return;
      currentType = REACTION_TYPE_BURST;
      void loadUsers();
    });

    closeBtn.addEventListener("click", () => {
      closeReactionUsersModal();
    });

    modal.addEventListener("keydown", (event) => {
      if (/** @type {any} */ (event).key === "Escape") {
        event.preventDefault?.();
        closeReactionUsersModal();
      }
    });

    modal.append(header, typeTabs, usersList);
    const mountTarget = containerEl ?? doc.body;
    mountTarget.append(modal);
    activeModal = modal;
    void loadUsers();
    return modal;
  }

  if (containerEl && doc) {
    const existingToolbar = /** @type {HTMLElement | null} */ (
      containerEl.querySelector(".msg-toolbar")
    );
    if (existingToolbar) {
      existingToolbar.classList.add("message-action-toolbar", "msg-hover-toolbar");
      const addReactionBtn = /** @type {HTMLElement | null} */ (
        existingToolbar.querySelector('[data-command="add-reaction"]')
      );
      if (addReactionBtn) {
        addReactionBtn.classList.add("open-picker-btn");
        addReactionBtn.setAttribute("aria-haspopup", "dialog");
        addReactionBtn.setAttribute("aria-expanded", "false");

        /** @param {any} event */
        const onOpenPickerClick = (event) => {
          if (destroyed) return;
          event?.stopImmediatePropagation?.();
          if (activePopover) {
            closePicker();
            return;
          }
          const idxAttr = existingToolbar.getAttribute("data-for-index");
          const index = idxAttr !== null ? Number(idxAttr) : 0;
          openPicker({
            index: Number.isInteger(index) ? index : 0,
            trigger: addReactionBtn,
          });
        };

        const shimListeners = /** @type {Map<string, Array<(e: any) => void>> | undefined} */ (
          /** @type {any} */ (addReactionBtn).listeners
        );
        if (shimListeners) {
          const bucket = shimListeners.get("click") ?? [];
          bucket.unshift(onOpenPickerClick);
          shimListeners.set("click", bucket);
        } else {
          addReactionBtn.addEventListener("click", onOpenPickerClick);
        }
        cleanups.push(() => {
          addReactionBtn.removeEventListener("click", onOpenPickerClick);
        });
      }
    } else {
      const bar = doc.createElement("div");
      bar.className = "message-action-toolbar msg-hover-toolbar";
      bar.setAttribute("role", "toolbar");
      bar.setAttribute("aria-label", "Действия с сообщением");
      bar.setAttribute("data-open", "false");

      for (const emoji of QUICK_REACTIONS) {
        const btn = doc.createElement("button");
        btn.className = "icon-btn msg-toolbar-btn quick-reaction-btn";
        btn.setAttribute("type", "button");
        btn.setAttribute("data-command", "react");
        btn.setAttribute("data-emoji", emoji);
        btn.setAttribute("aria-label", `Реакция ${emoji}`);
        btn.setAttribute("aria-pressed", "false");
        btn.textContent = emoji;
        btn.addEventListener("click", () => {
          if (destroyed) return;
          const idx = Number(bar.getAttribute("data-for-index") ?? "0");
          void requestReaction(idx, emoji, extrasFor(idx).reactions, {
            channelId: resolveChannelId(),
            messageId: String(idx),
            invoke: resolveInvoke(),
            onError: options.onError,
            onToggleReaction: options.onToggleReaction ?? null,
          });
        });
        bar.append(btn);
      }

      const openBtn = doc.createElement("button");
      openBtn.className = "icon-btn msg-toolbar-btn open-picker-btn";
      openBtn.setAttribute("type", "button");
      openBtn.setAttribute("data-command", "add-reaction");
      openBtn.setAttribute("aria-label", "Добавить реакцию");
      openBtn.setAttribute("aria-haspopup", "dialog");
      openBtn.setAttribute("aria-expanded", "false");
      openBtn.textContent = "Добавить реакцию";
      openBtn.addEventListener("click", () => {
        if (destroyed) return;
        const idx = Number(bar.getAttribute("data-for-index") ?? "0");
        openPicker({ index: idx, trigger: openBtn });
      });

      const replyBtn = doc.createElement("button");
      replyBtn.className = "icon-btn msg-toolbar-btn reply-btn";
      replyBtn.setAttribute("type", "button");
      replyBtn.setAttribute("data-command", "reply");
      replyBtn.setAttribute("aria-label", "Ответить");
      replyBtn.textContent = "Ответить";
      replyBtn.addEventListener("click", () => {
        if (destroyed) return;
        const idx = Number(bar.getAttribute("data-for-index") ?? "0");
        options.onReply?.({
          index: idx,
          messageId: String(idx),
          channelId: resolveChannelId(),
        });
      });

      bar.append(openBtn, replyBtn);
      containerEl.append(bar);
      ownToolbar = bar;
    }
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    closePicker({ restoreFocus: false });
    closeReactionUsersModal();
    for (const row of [...rowCleanups.keys()]) {
      cleanupRow(row);
    }
    for (const fn of cleanups.splice(0)) {
      fn();
    }
    if (ownToolbar) {
      ownToolbar.remove();
      ownToolbar = null;
    }
  }

  return {
    bindMessageRow,
    openPicker,
    closePicker,
    openReactionUsersModal,
    closeReactionUsersModal,
    destroy,
    dispose: destroy,
    get isDestroyed() {
      return destroyed;
    },
  };
}
