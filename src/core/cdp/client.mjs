// @ts-check
// CDP-клиент discord-lab. Контракт: tests/unit/cdp.test.mjs + tests/e2e/cdp-browser.e2e.test.mjs.
// WebSocket и fetch инжектируются — unit-тесты не открывают реальных соединений.

/** @typedef {import("../types.js").CdpTarget} CdpTarget */
/** @typedef {import("../types.js").PageTarget} PageTarget */
/** @typedef {import("../types.js").CdpEventClient} CdpEventClient */
/** @typedef {import("../types.js").CdpClient} CdpClient */
/** @typedef {import("../types.js").WebSocketLikeConstructor} WebSocketLikeConstructor */
/** @typedef {import("../types.js").FetchLike} FetchLike */

/**
 * Endpoint CDP из окружения: `CDP_ENDPOINT` или `http://127.0.0.1:9222`.
 * @param {Record<string, string | undefined>} [env]
 * @returns {string}
 */
export function resolveEndpoint(env) {
  return (env ?? process.env).CDP_ENDPOINT || "http://127.0.0.1:9222";
}

/**
 * Подстрока целевой страницы: `CDP_TARGET` или `discord.com`.
 * @param {Record<string, string | undefined>} [env]
 * @returns {string}
 */
export function resolveTargetHint(env) {
  return (env ?? process.env).CDP_TARGET || "discord.com";
}

/**
 * GET `<endpoint>/json/list`.
 * Ошибка HTTP → `CDP list failed: <status>`.
 * @param {string} endpoint
 * @param {FetchLike} [fetchImpl]
 * @param {{ timeoutMs?: number }} [options]
 * @returns {Promise<CdpTarget[]>}
 */
export async function listTargets(endpoint, fetchImpl, options = {}) {
  const fetcher = fetchImpl ?? globalThis.fetch;
  const timeoutMs = options.timeoutMs ?? 5000;
  const signal = AbortSignal.timeout(timeoutMs);
  let response;
  try {
    response = await fetcher(`${endpoint.replace(/\/$/, "")}/json/list`, { signal });
  } catch (error) {
    if (signal.aborted) throw new Error(`CDP list failed: таймаут ${timeoutMs} мс`);
    throw error;
  }
  if (!response.ok) throw new Error(`CDP list failed: ${response.status}`);
  const data = await response.json();
  if (!Array.isArray(data)) throw new Error("CDP list failed: ответ не является массивом");
  return /** @type {CdpTarget[]} */ (data);
}

/**
 * Выбирает страницу для работы среди записей `type === "page"` с непустым
 * `webSocketDebuggerUrl`:
 * 1) url содержит hint; 2) url содержит "discord"; 3) первая страница;
 * страниц нет → undefined.
 * @param {readonly CdpTarget[]} targets
 * @param {string} [hint]
 * @returns {PageTarget | undefined}
 */
export function pickTarget(targets, hint) {
  const pages = targets.filter((target) => target.type === "page" && Boolean(target.webSocketDebuggerUrl));
  const term = (hint ?? "").toLowerCase();
  return /** @type {PageTarget | undefined} */ (
    pages.find((target) => term && (target.url ?? "").toLowerCase().includes(term))
      ?? pages.find((target) => (target.url ?? "").toLowerCase().includes("discord"))
      ?? pages[0]
  );
}

/**
 * Подключается к CDP по `wsUrl` и возвращает клиент:
 * - `send(method, params)`: resolve `result`; при `error` —
 *   reject `Error("<method>: <message>")`;
 * - нет ответа за `timeoutMs` (по умолчанию 15000) —
 *   reject `Error("<method>: таймаут <timeoutMs> мс")`;
 * - сообщения без id с `method` копятся в `events` как
 *   `{ at: Date.now(), method, params }`;
 * - `close()` закрывает сокет; незавершённые запросы reject
 *   `Error("<method>: CDP соединение закрыто")`;
 * - мусорные (не-JSON) сообщения игнорируются, поток не падает.
 * @param {{ wsUrl: string, WebSocket?: WebSocketLikeConstructor, timeoutMs?: number }} options
 * @returns {Promise<CdpEventClient>}
 */
export async function createCdpClient(options) {
  const WebSocketImpl = options.WebSocket ?? /** @type {any} */ (globalThis).WebSocket;
  if (!WebSocketImpl) throw new Error("WebSocket is not available");
  const timeoutMs = options.timeoutMs ?? 15000;
  const ws = new WebSocketImpl(options.wsUrl);
  /** @type {Map<number, { method: string, resolve: (value: any) => void, reject: (error: Error) => void, timer: ReturnType<typeof setTimeout> }>} */
  const pending = new Map();
  /** @type {import("../types.js").CdpEvent[]} */
  const events = [];
  let nextId = 0;
  let closed = false;

  const rejectPending = () => {
    for (const [id, request] of pending) {
      clearTimeout(request.timer);
      request.reject(new Error(`${request.method}: CDP соединение закрыто`));
      pending.delete(id);
    }
  };

  const opened = new Promise((resolve, reject) => {
    ws.addEventListener("open", () => resolve(undefined), { once: true });
    ws.addEventListener("error", () => reject(new Error("WebSocket error")), { once: true });
  });
  ws.addEventListener("close", () => {
    closed = true;
    rejectPending();
  });
  ws.addEventListener("message", (/** @type {{ data: unknown }} */ event) => {
    let message;
    try {
      const data = event.data;
      message = JSON.parse(typeof data === "string" ? data : String(data));
    } catch {
      return;
    }
    if (typeof message.id === "number") {
      const request = pending.get(message.id);
      if (!request) return;
      clearTimeout(request.timer);
      pending.delete(message.id);
      if (message.error) request.reject(new Error(`${request.method}: ${message.error.message}`));
      else request.resolve(message.result);
    } else if (typeof message.method === "string") {
      events.push({ at: Date.now(), method: message.method, params: message.params });
    }
  });

  try {
    await opened;
  } catch (error) {
    ws.close();
    throw error;
  }
  return /** @type {any} */ ({
    events,
    mark() {
      return events.length;
    },
    eventsSince(/** @type {number} */ mark) {
      return events.slice(Math.max(0, mark));
    },
    drainEvents() {
      const drained = events.slice();
      events.length = 0;
      return drained;
    },
    send(/** @type {string} */ method, /** @type {Record<string, unknown>} */ params = {}) {
      if (closed) return Promise.reject(new Error(`${method}: CDP соединение закрыто`));
      const id = ++nextId;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`${method}: таймаут ${timeoutMs} мс`));
        }, timeoutMs);
        pending.set(id, { method, resolve, reject, timer });
        try {
          ws.send(JSON.stringify({ id, method, params }));
        } catch (error) {
          clearTimeout(timer);
          pending.delete(id);
          reject(error instanceof Error ? error : new Error(String(error)));
        }
      });
    },
    close() {
      if (closed) return;
      closed = true;
      rejectPending();
      ws.close();
    },
  });
}

/**
 * `Runtime.evaluate` c `returnByValue: true` и `awaitPromise: true`;
 * при `exceptionDetails` кидает `Error(exception?.description ?? text)`.
 * @param {CdpClient} cdp
 * @param {string} expression
 * @returns {Promise<any>}
 */
export async function evaluate(cdp, expression) {
  const response = await cdp.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (response?.exceptionDetails) {
    const details = response.exceptionDetails;
    throw new Error(details.exception?.description ?? details.text);
  }
  return response?.result?.value;
}
