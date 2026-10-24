// @ts-check
/**
 * Реакции сообщений: базовый набор из `rows.js` + локальные переключения.
 *
 * Состояние живёт вне DOM (Map по ключу строки), потому что строки
 * виртуализованы и переиспользуются: после скролла туда-обратно своя реакция
 * обязана остаться. Ключ — либо числовой индекс bench-строки, либо
 * `channel:<channelId>:<messageId>` для реального канала: синтетическое окно
 * и `messages_page` не делят состояние (M-06).
 *
 * Состояние меняется оптимистично, а transport откатывает его при ошибке IPC.
 */
import { baseReactions } from "./rows.js";
import { UIError } from "./errors.js";

/** @typedef {import("./rows.js").Reaction} Reaction */
/** @typedef {number | string} ReactionKey */

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
 * @param {{ allowUnknown?: boolean }} [options] `allowUnknown` — эмодзи из
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
 *   onSettled?: () => void,
 * }} [options]
 * @returns {Promise<void>}
 */
export async function requestReaction(key, emoji, base, options = {}) {
  const requestKey = `${String(key)}:${emoji}`;
  if (pending.has(requestKey)) return;
  const before = reactionsFor(key, base).find((reaction) => reaction.emoji === emoji);
  let active;
  try {
    active = toggleReaction(key, emoji, base, { allowUnknown: true });
  } catch (error) {
    reportReactionError(error instanceof UIError ? error : new UIError("ACTION_FAILED", String(error), error));
    return;
  }
  pending.add(requestKey);
  options.element?.setAttribute("data-pending", "true");
  if (!invokeReaction) {
    pending.delete(requestKey);
    options.onSettled?.();
    return;
  }
  try {
    await invokeReaction("toggle_reaction", {
      // Явный реальный адрес приоритетнее bench-значений по умолчанию.
      channelId: options.channelId ?? "news",
      messageId: options.messageId ?? String(key),
      emoji,
      active,
    });
  } catch (error) {
    const current = overrides.get(key);
    if (before) current?.set(emoji, { count: before.count, me: before.me });
    else current?.delete(emoji);
    if (current && current.size === 0) overrides.delete(key);
    const details = /** @type {any} */ (error);
    const status = details?.status ?? details?.code;
    const message = status === 401 || details?.kind === "Unauthorized"
      ? "сессия истекла"
      : status === 429 || details?.kind === "RateLimited"
        ? `ограничение частоты; повтор через ${details?.retry_after_ms ?? details?.retryAfterMs ?? 0} мс`
        : "сеть недоступна";
    reportReactionError(new UIError("ACTION_FAILED", message, error));
  } finally {
    pending.delete(requestKey);
    options.element?.removeAttribute("data-pending");
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
