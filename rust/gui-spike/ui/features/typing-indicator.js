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
