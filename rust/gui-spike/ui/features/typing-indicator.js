// @ts-check
/**
 * P-03-UI (RW-40): исходящий «печатает» и строка «Печатает…» (G6-REPORT §3.6).
 *
 * Контракт:
 * - `POST /channels/{id}/typing` без тела; IPC `send_typing { channelId }`
 *   (`rust/gui-spike/src/typing.rs`), успех `200` → опциональные slowmode-
 *   кулдауны `{messageSendCooldownMs?, threadCreateCooldownMs?}`; токен читает
 *   backend из keyring, во фронтенд он не попадает;
 * - TTL индикатора — core `TYPING_TTL_MS = 10_000` (`state/typing/types.rs`,
 *   активность `now < expires`), это наше значение `[A]`;
 * - интервал повтора `TYPING_RESEND_MS` — именованная политика `[A]` в
 *   диапазоне 8–10 с, а не восстановленное число Discord: проверяется правило
 *   `now > prev + interval`;
 * - локальный guard — не более `TYPING_MAX_CHANNELS` каналов;
 * - индикатор живёт в `aria-live="polite"`-регионе, не показывает себя,
 *   повторные события одного пользователя схлопываются по `(channelId, userId)`;
 * - 401/429/403/slowmode видны текстом и не запускают retry-цикл; тело ответа,
 *   сообщение ошибки и токен в UI/логи не попадают.
 *
 * Сеть здесь не вызывается: исходящий POST идёт только через `invoke(...)`,
 * который передаёт `app.js`. Входящие `TYPING_START` приходят из gateway-адаптера
 * (L-02) через `start()`; до L-02 индикатор работает на fake-событиях.
 */
import { UIError, asUIError } from "./errors.js";

/** TTL индикатора: копия core `TYPING_TTL_MS` (10 000 мс). */
export const TYPING_TTL_MS = 10_000;
/**
 * `[A]`: интервал повторного `send_typing` в одном канале. Не число Discord —
 * восстановленного значения `E` нет (`NETWORK-CONTRACTS §1.6`); диапазон 8–10 с
 * выбран под TTL 10 с, чтобы индикатор собеседника не гас между нажатиями.
 */
export const TYPING_RESEND_MS = 8_000;
/** Guard: одновременно «в наборе» не более 5 каналов (G6 §3.6). */
export const TYPING_MAX_CHANNELS = 5;
/** Guard: одновременно «в наборе» не более 5 каналов (P-03). */
export const MAX_TYPING_CHANNELS = 5;
/** Период проверки истечения TTL; таймер живёт только пока есть записи. */
export const TYPING_SWEEP_MS = 1_000;

/** Тексты индикатора и ошибок: тело ответа/токен сюда не подставляются. */
export const TYPING_TEXTS = Object.freeze({
  one: "Печатает…",
  many: "Несколько человек печатают…",
  unauthorized: "Сессия истекла. Войдите снова.",
  forbidden: "Нет прав писать в этот канал.",
  notFound: "Канал не найден или недоступен.",
  rateLimited: "Слишком часто. Повторите позже.",
  network: "Не удалось отправить «печатает».",
});

/**
 * @param {number} seconds
 * @returns {string}
 */
function slowmodeText(seconds) {
  return `Медленный режим: следующее сообщение через ${seconds} с.`;
}

/**
 * Событие `TYPING_START` (проекция gateway-адаптера; без member payload).
 * @typedef {{
 *   channelId: string,
 *   userId: string,
 *   name?: string,
 *   self?: boolean,
 *   at?: number,
 * }} TypingEvent
 */

/**
 * @typedef {{
 *   channelId: string,
 *   userId: string,
 *   name: string,
 *   receivedAt: number,
 * }} TypingEntry
 */

/**
 * @typedef {{
 *   node: HTMLElement,
 *   start: (event: TypingEvent) => boolean,
 *   sweep: () => number,
 *   refresh: () => void,
 *   notice: (text: string, kind?: string) => void,
 *   clear: () => void,
 *   setSelfUserId: (id: string) => void,
 *   setChannel: (channelId: string) => void,
 *   dispose: () => void,
 *   readonly count: number,
 * }} TypingIndicator
 */

/**
 * Строка «Печатает…» над композером.
 *
 * @param {{
 *   host: HTMLElement,
 *   getChannelId?: () => string,
 *   currentChannelId?: string,
 *   selfUserId?: string,
 *   now?: () => number,
 *   onError?: (error: UIError) => void,
 * }} options
 * @returns {TypingIndicator}
 */
export function createTypingIndicator(options) {
  const host = options.host;
  if (!host) throw new UIError("ELEMENT_MISSING", "индикатор набора: нет контейнера");
  const now = options.now ?? (() => performance.now());
  const onError = options.onError ?? (() => {});
  /** Канал, который сейчас открыт; пустая строка — бенч/не выбран. */
  let fallbackChannel = String(options.currentChannelId ?? "");
  const getChannelId = options.getChannelId ?? (() => fallbackChannel);
  let selfUserId = String(options.selfUserId ?? "");

  const node = document.createElement("div");
  node.id = "typing-indicator";
  node.className = "typing-indicator";
  // Один live-регион: текст набора и сообщение slowmode/ошибки озвучиваются
  // сменой содержимого, а не фокусом.
  node.setAttribute("role", "status");
  node.setAttribute("aria-live", "polite");
  node.setAttribute("aria-atomic", "true");
  node.setAttribute("data-visible", "false");
  node.setAttribute("data-count", "0");
  node.setAttribute("hidden", "");

  const text = document.createElement("span");
  text.className = "typing-indicator-text";
  const noticeNode = document.createElement("span");
  noticeNode.className = "typing-indicator-notice";
  noticeNode.setAttribute("hidden", "");
  node.append(text, noticeNode);
  host.append(node);

  /** @type {Map<string, TypingEntry>} */
  const entries = new Map();
  /** @type {ReturnType<typeof setInterval> | null} */
  let timer = null;
  let noticeText = "";
  let disposed = false;

  /** @param {TypingEntry} entry @param {number} at */
  function isActive(entry, at) {
    // Граница — как в core: `now < receivedAt + TTL`; на самом TTL уже неактивен.
    return at < entry.receivedAt + TYPING_TTL_MS;
  }

  /** @returns {TypingEntry[]} */
  function activeEntries() {
    const channelId = String(getChannelId() ?? "");
    if (channelId === "") return [];
    const at = now();
    /** @type {TypingEntry[]} */
    const rows = [];
    for (const entry of entries.values()) {
      if (entry.channelId !== channelId) continue;
      if (selfUserId !== "" && entry.userId === selfUserId) continue;
      if (!isActive(entry, at)) continue;
      rows.push(entry);
    }
    return rows;
  }

  function render() {
    const rows = activeEntries();
    const count = rows.length;
    const visible = count > 0 || noticeText !== "";
    text.textContent = count === 0 ? "" : count === 1 ? TYPING_TEXTS.one : TYPING_TEXTS.many;
    node.setAttribute("data-count", String(count));
    node.setAttribute("data-visible", visible ? "true" : "false");
    if (visible) node.removeAttribute("hidden");
    else node.setAttribute("hidden", "");
  }

  function stopTimer() {
    if (timer === null) return;
    clearInterval(timer);
    timer = null;
  }

  /** Таймер нужен только пока есть записи: пустой индикатор не будит процесс. */
  function ensureTimer() {
    if (timer !== null || disposed) return;
    timer = setInterval(() => {
      sweep();
      if (entries.size === 0) stopTimer();
    }, TYPING_SWEEP_MS);
  }

  /** Удалить истёкшие записи всех каналов; возвращает их число. */
  function sweep() {
    const at = now();
    let removed = 0;
    for (const [key, entry] of [...entries.entries()]) {
      if (isActive(entry, at)) continue;
      entries.delete(key);
      removed += 1;
    }
    if (removed > 0 || entries.size === 0) {
      if (entries.size === 0) stopTimer();
      render();
    }
    return removed;
  }

  /** @param {TypingEvent} event @returns {boolean} принято ли событие */
  function start(event) {
    try {
      const channelId = String(event?.channelId ?? "").trim();
      const userId = String(event?.userId ?? "").trim();
      if (channelId === "" || userId === "") return false;
      if (event?.self === true) return false;
      if (selfUserId !== "" && userId === selfUserId) return false;
      entries.set(`${channelId}:${userId}`, {
        channelId,
        userId,
        name: String(event?.name ?? ""),
        receivedAt: typeof event?.at === "number" && Number.isFinite(event.at) ? event.at : now(),
      });
      ensureTimer();
      render();
      return true;
    } catch (error) {
      onError(asUIError("ACTION_FAILED", error));
      return false;
    }
  }

  /** @param {string} message @param {string} [kind] */
  function notice(message, kind = "info") {
    noticeText = String(message ?? "");
    noticeNode.textContent = noticeText;
    if (noticeText === "") {
      noticeNode.setAttribute("hidden", "");
      noticeNode.removeAttribute("data-kind");
    } else {
      noticeNode.setAttribute("data-kind", kind);
      noticeNode.removeAttribute("hidden");
    }
    render();
  }

  function clear() {
    entries.clear();
    stopTimer();
    noticeText = "";
    noticeNode.textContent = "";
    noticeNode.setAttribute("hidden", "");
    noticeNode.removeAttribute("data-kind");
    render();
  }

  /** @param {string} id */
  function setSelfUserId(id) {
    selfUserId = String(id ?? "");
    render();
  }

  /** @param {string} channelId */
  function setChannel(channelId) {
    fallbackChannel = String(channelId ?? "");
    render();
  }

  function dispose() {
    disposed = true;
    stopTimer();
    entries.clear();
    node.remove();
  }

  render();

  return {
    node,
    start,
    sweep,
    refresh: render,
    notice,
    clear,
    setSelfUserId,
    setChannel,
    dispose,
    get count() {
      return activeEntries().length;
    },
  };
}

/**
 * Разбор ошибки `send_typing`. `permanent` — повторная отправка запрещена до
 * `reset()` (401/403); `pauseMs` — пауза после 429.
 * @typedef {{ kind: string, text: string, permanent: boolean, pauseMs: number }} TypingFailure
 */

/**
 * Тип ошибки `send_typing` без тела ответа: 401/403/404/429 объясняются,
 * остальное — общий отказ. Текст ошибки/токен не переиспользуются.
 * @param {unknown} error
 * @returns {TypingFailure}
 */
export function typingFailure(error) {
  const bag = error !== null && typeof error === "object"
    ? /** @type {Record<string, any>} */ (error)
    : {};
  const kind = String(bag.kind ?? "");
  const status = Number(bag.status ?? bag.code ?? 0);
  const retry = Number(bag.retryAfterMs ?? bag.retry_after_ms ?? 0);
  if (status === 401 || /unauthor/i.test(kind)) {
    return {
      kind: "Unauthorized",
      text: TYPING_TEXTS.unauthorized,
      permanent: true,
      pauseMs: 0,
    };
  }
  if (status === 403 || /forbidden/i.test(kind)) {
    return {
      kind: "Forbidden",
      text: TYPING_TEXTS.forbidden,
      permanent: true,
      pauseMs: 0,
    };
  }
  if (status === 429 || /ratelimit/i.test(kind)) {
    return {
      kind: "RateLimited",
      text: TYPING_TEXTS.rateLimited,
      permanent: false,
      // Нет retry_after — пауза не меньше интервала повтора: цикл не разгоняется.
      pauseMs: retry > 0 ? retry : TYPING_RESEND_MS,
    };
  }
  if (status === 404 || /notfound/i.test(kind)) {
    return {
      kind: "NotFound",
      text: TYPING_TEXTS.notFound,
      permanent: false,
      pauseMs: 0,
    };
  }
  return {
    kind: "Network",
    text: TYPING_TEXTS.network,
    permanent: false,
    pauseMs: 0,
  };
}

/**
 * @typedef {{
 *   sent: boolean,
 *   reason?: "invalid" | "unavailable" | "blocked" | "cooldown" | "throttle" | "guard" | "pending" | "failed",
 *   kind?: string,
 *   cooldownMs?: number,
 * }} TypingSendResult
 */

/**
 * @typedef {{
 *   note: (channelId: string) => Promise<TypingSendResult>,
 *   reset: () => void,
 *   readonly blocked: boolean,
 *   readonly channels: number,
 * }} TypingSender
 */

/**
 * Исходящий typing: `send_typing { channelId }` не чаще `resendMs` на канал,
 * не более `maxChannels` каналов, без retry-цикла после 401/403/429/slowmode.
 *
 * @param {{
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   now?: () => number,
 *   resendMs?: number,
 *   maxChannels?: number,
 *   onNotice?: (text: string, kind: string) => void,
 * }} [options]
 * @returns {TypingSender}
 */
export function createTypingSender(options = {}) {
  const invoke = options.invoke;
  const now = options.now ?? (() => performance.now());
  const resendMs = Number.isFinite(options.resendMs) && Number(options.resendMs) >= 0
    ? Number(options.resendMs)
    : TYPING_RESEND_MS;
  const maxChannels = Number.isFinite(options.maxChannels) && Number(options.maxChannels) > 0
    ? Math.trunc(Number(options.maxChannels))
    : TYPING_MAX_CHANNELS;
  const onNotice = options.onNotice ?? (() => {});

  /** Последняя отправка по каналу (мс) — окно троттлинга. @type {Map<string, number>} */
  const lastSentAt = new Map();
  /** Каналы в полёте: второй `input` не шлёт второй POST. @type {Set<string>} */
  const inFlight = new Set();
  /** Постоянная блокировка (401/403) — до `reset()` (повторный вход). */
  let blocked = false;
  /** Пауза после 429/slowmode (монотонные часы), 0 — паузы нет. */
  let blockedUntil = 0;

  /**
   * @param {unknown} value
   * @returns {number}
   */
  function cooldownOf(value) {
    if (value === null || typeof value !== "object") return 0;
    const bag = /** @type {Record<string, unknown>} */ (value);
    // backend отдаёт camelCase, но принимаем и wire snake_case (устойчивость).
    const raw = Number(bag.messageSendCooldownMs ?? bag.message_send_cooldown_ms ?? 0);
    return Number.isFinite(raw) && raw > 0 ? Math.trunc(raw) : 0;
  }

  /** @param {string} channelId @returns {Promise<TypingSendResult>} */
  async function note(channelId) {
    const channel = String(channelId ?? "").trim();
    // Синтетический бенч («news») и мусор не снежинки: IPC не вызываем.
    if (!/^\d{1,20}$/.test(channel)) return { sent: false, reason: "invalid" };
    if (!invoke) return { sent: false, reason: "unavailable" };
    if (blocked) return { sent: false, reason: "blocked" };
    const at = now();
    if (blockedUntil > 0) {
      if (at < blockedUntil) return { sent: false, reason: "cooldown" };
      blockedUntil = 0;
    }
    if (inFlight.has(channel)) return { sent: false, reason: "pending" };
    const previous = lastSentAt.get(channel);
    // Правило G6: повтор только когда `now > prev + interval` (граница — стоп).
    if (previous !== undefined && at <= previous + resendMs) {
      return { sent: false, reason: "throttle" };
    }
    // Guard считается по каналам, где уже был хотя бы один успешный typing.
    if (previous === undefined && lastSentAt.size >= maxChannels) {
      return { sent: false, reason: "guard" };
    }
    inFlight.add(channel);
    try {
      const result = await invoke("send_typing", { channelId: channel });
      // Окно повтора стартует от успешной отправки: отказ сервера не должен
      // съедать интервал и не должен превращаться в повтор на каждое нажатие.
      lastSentAt.set(channel, at);
      const cooldownMs = cooldownOf(result);
      if (cooldownMs > 0) {
        blockedUntil = at + cooldownMs;
        onNotice(slowmodeText(Math.ceil(cooldownMs / 1_000)), "slowmode");
      } else {
        // Успешный typing снимает прошлое сообщение об ошибке/slowmode.
        onNotice("", "info");
      }
      return { sent: true, cooldownMs };
    } catch (error) {
      const failure = typingFailure(error);
      if (failure.permanent) blocked = true;
      // Любой отказ держит паузу минимум на интервал повтора: набор текста не
      // разгоняет поток запросов, а 429 ждёт `retry_after` из ответа.
      blockedUntil = at + (failure.pauseMs > 0 ? failure.pauseMs : resendMs);
      onNotice(failure.text, failure.kind);
      return { sent: false, reason: "failed", kind: failure.kind };
    } finally {
      inFlight.delete(channel);
    }
  }

  function reset() {
    blocked = false;
    blockedUntil = 0;
    lastSentAt.clear();
    inFlight.clear();
  }

  return {
    note,
    reset,
    get blocked() {
      return blocked;
    },
    get channels() {
      return lastSentAt.size;
    },
  };
}

/**
 * Форматирование русского текста для печатающих участников (P-03):
 * - 1 чел. -> 'X печатает…'
 * - 2 чел. -> 'X и Y печатают…'
 * - 3 чел. -> 'X, Y и Z печатают…'
 * - >3 чел. -> 'Несколько человек печатают…'
 * @param {string[]} names
 * @returns {string}
 */
export function formatTypingText(names) {
  if (!Array.isArray(names) || names.length === 0) return "";
  if (names.length === 1) return `${names[0]} печатает…`;
  if (names.length === 2) return `${names[0]} и ${names[1]} печатают…`;
  if (names.length === 3) return `${names[0]}, ${names[1]} и ${names[2]} печатают…`;
  return "Несколько человек печатают…";
}

/**
 * @typedef {{
 *   channelId: string,
 *   userId: string,
 *   username?: string,
 *   now?: number,
 * }} TypingStartEvent
 */

/**
 * @typedef {{
 *   channelId: string,
 *   authorId: string,
 * }} MessageCreatedEvent
 */

/**
 * @typedef {{
 *   container?: HTMLElement,
 *   host?: HTMLElement,
 *   document?: Document,
 *   currentUserId?: string,
 *   currentChannelId?: string,
 *   channelId?: string,
 *   getCurrentChannelId?: () => string,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   now?: () => number,
 *   ttlMs?: number,
 *   resendMs?: number,
 *   maxChannels?: number,
 * }} TypingControllerOptions
 */

/**
 * @typedef {{
 *   username: string,
 *   expiresAt: number,
 *   startedAt: number,
 * }} UserTypingEntry
 */

/**
 * @typedef {{
 *   element: HTMLElement,
 *   node: HTMLElement,
 *   onTypingStart: (event: TypingStartEvent) => boolean,
 *   onMessageCreated: (event: MessageCreatedEvent) => void,
 *   notifyInput: (channelId: string, text: string, now?: number) => Promise<{ sent: boolean, reason?: string, retryAfterMs?: number, result?: any, error?: any }>,
 *   sweep: (at?: number) => number,
 *   render: (at?: number) => void,
 *   setChannel: (channelId: string) => void,
 *   setChannelId: (channelId: string) => void,
 *   setCurrentUserId: (userId: string) => void,
 *   getTypingText: (channelId: string) => string,
 *   getTypingUsers: (channelId: string) => string[],
 *   dispose: () => void,
 *   readonly currentChannelId: string | null,
 *   readonly currentUserId: string,
 *   readonly channels: number,
 * }} TypingController
 */

/**
 * Контроллер индикатора набора и исходящего typing (P-03).
 *
 * @param {TypingControllerOptions} [options]
 * @returns {TypingController}
 */
export function createTypingController(options = {}) {
  const doc = options.document
    ?? (typeof document !== "undefined" ? document : null)
    ?? options.container?.ownerDocument
    ?? options.host?.ownerDocument
    ?? null;

  if (!doc) {
    throw new UIError("ELEMENT_MISSING", "индикатор набора: недоступен document");
  }

  let lastKnownTime = 0;

  /**
   * @param {number} [at]
   * @returns {number}
   */
  function getCurrentTime(at) {
    if (typeof at === "number" && Number.isFinite(at)) {
      lastKnownTime = Math.max(lastKnownTime, at);
      return at;
    }
    if (options.now) {
      return options.now();
    }
    if (lastKnownTime > 0) {
      return lastKnownTime;
    }
    return Date.now();
  }
  const ttlMs = Number.isFinite(options.ttlMs) && Number(options.ttlMs) > 0
    ? Number(options.ttlMs)
    : TYPING_TTL_MS;
  const resendMs = Number.isFinite(options.resendMs) && Number(options.resendMs) >= 0
    ? Number(options.resendMs)
    : TYPING_RESEND_MS;
  const maxChannels = Number.isFinite(options.maxChannels) && Number(options.maxChannels) > 0
    ? Math.trunc(Number(options.maxChannels))
    : MAX_TYPING_CHANNELS;

  let currentUserId = String(options.currentUserId ?? "").trim();
  /** @type {string | null} */
  let currentChannelId = options.currentChannelId ?? options.channelId ?? null;
  const getCurrentChannelId = options.getCurrentChannelId
    ?? (() => currentChannelId ?? "");

  const invoke = options.invoke ?? null;

  // Прогрессивный поиск или создание узла #chat-typing-indicator
  /** @type {HTMLElement | null} */
  let element = doc.getElementById("chat-typing-indicator");
  if (!element && options.container?.querySelector) {
    element = options.container.querySelector("#chat-typing-indicator");
  }
  if (!element && options.host?.querySelector) {
    element = options.host.querySelector("#chat-typing-indicator");
  }

  if (!element) {
    element = doc.createElement("div");
    element.id = "chat-typing-indicator";
    element.setAttribute("role", "status");
    element.setAttribute("aria-live", "polite");
    element.setAttribute("data-state", "idle");
    element.textContent = "";

    const parent = options.container
      ?? options.host
      ?? doc.getElementById("composer")
      ?? (doc.querySelector ? doc.querySelector(".chat-main") : null)
      ?? doc.body;

    if (parent?.append) {
      parent.append(element);
    }
  } else {
    if (!element.getAttribute("role")) {
      element.setAttribute("role", "status");
    }
    if (!element.getAttribute("aria-live")) {
      element.setAttribute("aria-live", "polite");
    }
    if (!element.getAttribute("data-state")) {
      element.setAttribute("data-state", "idle");
    }
  }

  /**
   * Канал -> Map<userId, UserTypingEntry>
   * @type {Map<string, Map<string, UserTypingEntry>>}
   */
  const channels = new Map();

  /**
   * Последняя отправка по каналу (мс) — для троттлинга TYPING_RESEND_MS.
   * @type {Map<string, number>}
   */
  const lastSentAt = new Map();

  /** Блокировка повторной отправки после 429/slowmode (монотонные часы/время). */
  let blockedUntil = 0;

  /** @type {ReturnType<typeof setInterval> | null} */
  let sweepTimer = null;
  let disposed = false;

  /**
   * @param {number} [at]
   */
  function render(at) {
    if (disposed || !element) return;
    const currentTime = getCurrentTime(at);
    const activeChannel = getCurrentChannelId();

    if (!activeChannel) {
      const currentState = element.getAttribute("data-state");
      if (currentState !== "slowmode" && currentState !== "error") {
        element.setAttribute("data-state", "idle");
        element.textContent = "";
      }
      return;
    }

    const channelUsers = channels.get(activeChannel);
    if (!channelUsers || channelUsers.size === 0) {
      const currentState = element.getAttribute("data-state");
      if (currentState !== "slowmode" && currentState !== "error") {
        element.setAttribute("data-state", "idle");
        element.textContent = "";
      }
      return;
    }

    // Собираем неистёкших
    /** @type {string[]} */
    const activeNames = [];
    for (const [userId, entry] of [...channelUsers.entries()]) {
      if (currentTime >= entry.expiresAt) {
        channelUsers.delete(userId);
      } else {
        activeNames.push(entry.username);
      }
    }

    if (channelUsers.size === 0) {
      channels.delete(activeChannel);
    }

    if (activeNames.length > 0) {
      element.setAttribute("data-state", "active");
      element.textContent = formatTypingText(activeNames);
    } else {
      const currentState = element.getAttribute("data-state");
      if (currentState !== "slowmode" && currentState !== "error") {
        element.setAttribute("data-state", "idle");
        element.textContent = "";
      }
    }
  }

  /**
   * Очистить истёкшие записи во всех каналах.
   * @param {number} [at]
   * @returns {number} количество удалённых записей
   */
  function sweep(at) {
    if (disposed) return 0;
    const currentTime = getCurrentTime(at);
    let removed = 0;
    for (const [chId, userMap] of [...channels.entries()]) {
      for (const [uId, entry] of [...userMap.entries()]) {
        if (currentTime >= entry.expiresAt) {
          userMap.delete(uId);
          removed++;
        }
      }
      if (userMap.size === 0) {
        channels.delete(chId);
      }
    }
    render(currentTime);
    return removed;
  }

  /**
   * @param {TypingStartEvent} event
   * @returns {boolean}
   */
  function onTypingStart(event) {
    if (disposed) return false;
    const channelId = String(event?.channelId ?? "").trim();
    const userId = String(event?.userId ?? "").trim();
    if (!channelId || !userId) return false;

    // Игнорирование собственного currentUserId
    if (currentUserId && userId === currentUserId) return false;

    const timestamp = getCurrentTime(event?.now);

    // Если текущий канал не был явно задан — переключаемся на этот канал
    if (currentChannelId === null) {
      currentChannelId = channelId;
    }

    // Хранение активных печатающих по каналам (не более 5 каналов с вытеснением самого старого)
    let userMap = channels.get(channelId);
    if (userMap) {
      // Обновляем порядок использования канала (LRU)
      channels.delete(channelId);
      channels.set(channelId, userMap);
    } else {
      // Сначала чистим пустые/истёкшие каналы
      for (const [existingChId, existingUsers] of [...channels.entries()]) {
        for (const [uId, entry] of [...existingUsers.entries()]) {
          if (timestamp >= entry.expiresAt) {
            existingUsers.delete(uId);
          }
        }
        if (existingUsers.size === 0) {
          channels.delete(existingChId);
        }
      }

      // Если всё ещё каналов >= maxChannels, вытесняем самый старый
      if (channels.size >= maxChannels) {
        const oldestChannelId = channels.keys().next().value;
        if (oldestChannelId !== undefined) {
          channels.delete(oldestChannelId);
        }
      }

      userMap = new Map();
      channels.set(channelId, userMap);
    }

    const username = String(event?.username ?? "").trim() || userId;
    userMap.set(userId, {
      username,
      expiresAt: timestamp + ttlMs,
      startedAt: timestamp,
    });

    render(timestamp);
    return true;
  }

  /**
   * @param {MessageCreatedEvent} event
   */
  function onMessageCreated(event) {
    if (disposed) return;
    const channelId = String(event?.channelId ?? "").trim();
    const authorId = String(event?.authorId ?? "").trim();
    if (!channelId || !authorId) return;

    const userMap = channels.get(channelId);
    if (userMap && userMap.has(authorId)) {
      userMap.delete(authorId);
      if (userMap.size === 0) {
        channels.delete(channelId);
      }
      render();
    }
  }

  /**
   * @param {string} channelId
   * @param {string} text
   * @param {number} [now]
   * @returns {Promise<{ sent: boolean, reason?: string, retryAfterMs?: number, result?: any, error?: any }>}
   */
  async function notifyInput(channelId, text, now) {
    if (disposed) return { sent: false, reason: "disposed" };
    const ch = String(channelId ?? "").trim();
    if (!ch) return { sent: false, reason: "invalid" };

    // Не вызывает invoke('send_typing', { channelId }) на пустом/пробельном тексте
    if (typeof text !== "string" || text.trim().length === 0) {
      return { sent: false, reason: "empty" };
    }

    const at = getCurrentTime(now);

    // Блокировка после 429 RateLimited
    if (blockedUntil > 0) {
      if (at < blockedUntil) {
        return { sent: false, reason: "slowmode", retryAfterMs: blockedUntil - at };
      }
      blockedUntil = 0;
      if (element?.getAttribute("data-state") === "slowmode") {
        element.setAttribute("data-state", "idle");
      }
    }

    // Троттлинг TYPING_RESEND_MS (8000 мс) на канал
    const lastSent = lastSentAt.get(ch);
    if (lastSent !== undefined && at < lastSent + resendMs) {
      return { sent: false, reason: "throttle" };
    }

    if (!invoke) {
      return { sent: false, reason: "unavailable" };
    }

    try {
      const result = await invoke("send_typing", { channelId: ch });
      lastSentAt.set(ch, at);

      // Если был в состоянии ошибки/slowmode — восстанавливаем
      if (element?.getAttribute("data-state") === "error" || element?.getAttribute("data-state") === "slowmode") {
        render(at);
      }

      // Проверяем кулдаун в ответе
      const cooldownMs = Number(result?.messageSendCooldownMs ?? result?.message_send_cooldown_ms ?? 0);
      if (cooldownMs > 0) {
        blockedUntil = at + cooldownMs;
        element?.setAttribute("data-state", "slowmode");
      }

      return { sent: true, result };
    } catch (error) {
      const bag = /** @type {Record<string, any>} */ (error ?? {});
      const status = Number(bag.status ?? bag.code ?? 0);
      const kind = String(bag.kind ?? bag.message ?? "");
      const isRateLimit = status === 429 || /ratelimit/i.test(kind);

      if (isRateLimit) {
        const retryAfterMs = Number(bag.retry_after_ms ?? bag.retryAfterMs ?? bag.retryAfter ?? 0) || resendMs;
        blockedUntil = at + retryAfterMs;
        element?.setAttribute("data-state", "slowmode");
        return { sent: false, reason: "slowmode", retryAfterMs, error };
      } else {
        element?.setAttribute("data-state", "error");
        return { sent: false, reason: "error", error };
      }
    }
  }

  /**
   * @param {string} channelId
   * @returns {string}
   */
  function getTypingText(channelId) {
    const ch = String(channelId ?? "").trim();
    const userMap = channels.get(ch);
    if (!userMap) return "";
    const names = [...userMap.values()].map((e) => e.username);
    return formatTypingText(names);
  }

  /**
   * @param {string} channelId
   * @returns {string[]}
   */
  function getTypingUsers(channelId) {
    const ch = String(channelId ?? "").trim();
    const userMap = channels.get(ch);
    if (!userMap) return [];
    return [...userMap.values()].map((e) => e.username);
  }

  /**
   * @param {string} channelId
   */
  function setChannel(channelId) {
    currentChannelId = String(channelId ?? "");
    render();
  }

  /**
   * @param {string} userId
   */
  function setCurrentUserId(userId) {
    currentUserId = String(userId ?? "").trim();
    render();
  }

  function dispose() {
    disposed = true;
    if (sweepTimer !== null) {
      clearInterval(sweepTimer);
      sweepTimer = null;
    }
    channels.clear();
    lastSentAt.clear();
  }

  render();

  return {
    element,
    node: element,
    onTypingStart,
    onMessageCreated,
    notifyInput,
    sweep,
    render,
    setChannel,
    setChannelId: setChannel,
    setCurrentUserId,
    getTypingText,
    getTypingUsers,
    dispose,
    get currentChannelId() {
      return currentChannelId;
    },
    get currentUserId() {
      return currentUserId;
    },
    get channels() {
      return channels.size;
    },
  };
}

/**
 * Входящая запись «печатает» для строки индикатора (task-206).
 * @typedef {{
 *   userId: string,
 *   displayName: string,
 *   startedAtMs: number,
 * }} TypingUserEntry
 */

/**
 * Оставить только живые записи «печатает» и схлопнуть дубли по `userId`.
 *
 * Окно активности — `0 <= nowMs - startedAtMs < TYPING_TTL_MS`: на самой
 * границе TTL запись уже мертва, «из будущего» (`age < 0`) — тоже, чтобы
 * расхождение часов не держало индикатор вечно. При дублях остаётся запись с
 * наибольшим `startedAtMs` (устаревший `TYPING_START` не откатывает строку
 * назад). Возвращаемые записи упорядочены по `startedAtMs` по возрастанию —
 * так строка не «прыгает» между рендерами.
 *
 * @param {readonly TypingUserEntry[]} entries
 * @param {number} [nowMs] монотонное «сейчас»; по умолчанию `Date.now()`
 * @returns {TypingUserEntry[]}
 */
export function pruneExpiredTypingUsers(entries, nowMs = Date.now()) {
  if (!Array.isArray(entries)) return [];
  const at = Number.isFinite(nowMs) ? Number(nowMs) : Date.now();
  /** @type {Map<string, TypingUserEntry>} */
  const latest = new Map();
  for (const raw of entries) {
    if (raw === null || typeof raw !== "object") continue;
    const userId = String(raw.userId ?? "").trim();
    if (userId === "") continue;
    const startedAtMs = Number(raw.startedAtMs);
    if (!Number.isFinite(startedAtMs)) continue;
    const age = at - startedAtMs;
    if (!(age >= 0 && age < TYPING_TTL_MS)) continue;
    const previous = latest.get(userId);
    if (previous !== undefined && previous.startedAtMs >= startedAtMs) continue;
    latest.set(userId, {
      userId,
      displayName: String(raw.displayName ?? ""),
      startedAtMs,
    });
  }
  return [...latest.values()].sort((a, b) => a.startedAtMs - b.startedAtMs);
}

/**
 * Русская строка «печатает» для набора имён (task-206).
 *
 * Пустые и пробельные имена отбрасываются до подсчёта, поэтому «Аня, '   '»
 * даёт форму единственного числа, а не «Несколько человек». Формы:
 * `0 -> ''`, `1 -> 'X печатает...'`, `2 -> 'X и Y печатают...'`,
 * `3 -> 'X, Y и Z печатают...'`, `>= 4 -> 'Несколько человек печатают...'`.
 *
 * @param {readonly string[]} displayNames
 * @returns {string}
 */
export function formatTypingIndicatorRu(displayNames) {
  if (!Array.isArray(displayNames)) return "";
  /** @type {string[]} */
  const names = [];
  for (const raw of displayNames) {
    const name = String(raw ?? "").trim();
    if (name !== "") names.push(name);
  }
  if (names.length === 0) return "";
  if (names.length === 1) return `${names[0]} печатает...`;
  if (names.length === 2) return `${names[0]} и ${names[1]} печатают...`;
  if (names.length === 3) return `${names[0]}, ${names[1]} и ${names[2]} печатают...`;
  return "Несколько человек печатают...";
}
