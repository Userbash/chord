// @ts-check
/**
 * L-02-UI (RW-40): транспорт dispatch-кадра + счётчики + изоляция ошибок.
 *
 * `start()` — после `auth ready` (как `net.start()`), `stop()` — на logout
 * (как `net.stop()`/`typingSender.reset()`). Подписка одна: повторный `start()`
 * не создаёт вторую, `stop()` дожидается и вызывает `unlisten`.
 *
 * Наблюдаемость (`bridge.stats`, снапшот `dispatch`) — только имена/числа:
 * ни payload, ни текста сообщений, ни секретов (`CONTRACTS/dispatch.md` §8).
 */
import {
  DISPATCH_EVENT,
  applyDispatch,
  createDispatchStore,
  setDispatchSelfUserId,
} from "./dispatch-model.js";

/** Счётчики модели, которые складываются с view-счётчиками. */
const MODEL_COUNTERS = [
  "malformed",
  "ignored",
  "duplicate",
  "stale",
  "outOfRange",
  "skipped",
  "degraded",
  "unknownStatus",
  "notLoaded",
  "notOpen",
  "redactedKeys",
];

/**
 * Tauri v2 `window.__TAURI__.event.listen` или `null`, если транспорта нет.
 * Возвращается сама функция (тест сверяет идентичность), без обёртки.
 * @param {any} [win]
 * @returns {((event: string, handler: (payload: any) => void) => Promise<() => void>) | null}
 */
export function resolveDispatchListen(win = /** @type {any} */ (globalThis).window) {
  const listen = win?.__TAURI__?.event?.listen;
  return typeof listen === "function" ? listen : null;
}

/**
 * Tauri-событие приходит как `{event, id, payload}`; тестовый путь и
 * `currentApp().dispatch.handle` — сразу конвертом `{kind, data}`.
 * @param {unknown} value
 * @returns {unknown}
 */
function unwrapEnvelope(value) {
  if (value !== null && typeof value === "object" && "payload" in value) {
    const payload = /** @type {{ payload: unknown }} */ (value).payload;
    if (payload !== null && typeof payload === "object" && "kind" in payload) return payload;
  }
  return value;
}

/**
 * @param {{
 *   listen?: ((event: string, handler: (payload: any) => void) => Promise<() => void>) | null,
 *   view?: {
 *     apply?: (changes: any[]) => { applied?: number, ignored?: number, failed?: number, notOpen?: number, notLoaded?: number },
 *     setSource?: (source: string) => void,
 *   } | null,
 *   store?: import("./dispatch-model.js").DispatchStore,
 *   now?: () => number,
 *   dedupeSize?: number,
 *   onDispatch?: (envelope: any) => void,
 * }} options
 */
export function createDispatchBridge(options) {
  const listen = options.listen ?? null;
  const view = options.view ?? null;
  const store = options.store ?? createDispatchStore();
  /** @type {{ source: string, received: number, applied: number, ignored: number, malformed: number, duplicate: number, stale: number, outOfRange: number, skipped: number, degraded: number, unknownStatus: number, notLoaded: number, notOpen: number, failed: number, redactedKeys: number, lastErrorKind: string }} */
  const stats = {
    source: listen ? "live" : "absent",
    received: 0,
    applied: 0,
    ignored: 0,
    malformed: 0,
    duplicate: 0,
    stale: 0,
    outOfRange: 0,
    skipped: 0,
    degraded: 0,
    unknownStatus: 0,
    notLoaded: 0,
    notOpen: 0,
    failed: 0,
    redactedKeys: 0,
    lastErrorKind: "",
  };

  /** @type {(() => void) | null} */
  let unlisten = null;
  /** @type {Promise<void> | null} */
  let pending = null;
  /** Подписка нужна (true) или stop() уже попросил снять её (false). */
  let wanted = false;

  function publishSource() {
    view?.setSource?.(stats.source);
  }

  // Источник честен ещё до start(): «нет транспорта» видно сразу.
  publishSource();

  /**
   * @param {import("./dispatch-model.js").ModelCounters} counters
   */
  function mergeCounters(counters) {
    /** @type {Record<string, number>} */
    const target = /** @type {Record<string, number>} */ (/** @type {any} */ (stats));
    for (const name of MODEL_COUNTERS) {
      const value = counters[/** @type {keyof import("./dispatch-model.js").ModelCounters} */ (name)];
      if (typeof value !== "number") continue;
      target[name] = (target[name] ?? 0) + value;
    }
  }

  /**
   * Один конверт: модель → view. Исключение view не глушится (failed +
   * lastErrorKind без payload), следующее событие обрабатывается (N13).
   * @param {unknown} value
   */
  function handle(value) {
    stats.received += 1;
    const envelope = unwrapEnvelope(value);
    if (typeof options.onDispatch === "function") {
      try {
        options.onDispatch(envelope);
      } catch {
        // Исключение внешнего подписчика не роняет цикл диспатчей
      }
    }
    let result;
    try {
      result = applyDispatch(store, envelope);
    } catch {
      stats.failed += 1;
      stats.lastErrorKind = "model";
      return { ok: false, kind: "", changes: [], counters: null };
    }
    mergeCounters(result.counters);
    if (result.changes.length === 0) return result;
    try {
      const outcome = view?.apply?.(result.changes)
        ?? { applied: 0, ignored: 0, failed: 0, notOpen: 0, notLoaded: 0 };
      stats.applied += Number(outcome.applied) || 0;
      stats.ignored += Number(outcome.ignored) || 0;
      stats.notOpen += Number(outcome.notOpen) || 0;
      stats.notLoaded += Number(outcome.notLoaded) || 0;
      stats.failed += Number(outcome.failed) || 0;
      if ((Number(outcome.failed) || 0) > 0) stats.lastErrorKind = "apply";
    } catch {
      stats.failed += 1;
      stats.lastErrorKind = "apply";
    }
    return result;
  }

  /** @returns {Promise<void>} */
  function start() {
    wanted = true;
    if (unlisten) return Promise.resolve();
    if (pending) return pending;
    if (!listen) {
      stats.source = "absent";
      publishSource();
      return Promise.resolve();
    }
    pending = Promise.resolve()
      .then(() => listen(DISPATCH_EVENT, (payload) => { handle(payload); }))
      .then((fn) => {
        // stop() мог обогнать резолв listen: подписку снимаем сразу.
        if (!wanted) {
          if (typeof fn === "function") fn();
          return;
        }
        unlisten = typeof fn === "function" ? fn : null;
        stats.source = "live";
        publishSource();
      })
      .catch(() => {
        // Ошибка IPC не должна всплывать unhandled rejection: причина видна
        // в source=unavailable и счётчике.
        stats.failed += 1;
        stats.lastErrorKind = "listen";
        stats.source = "unavailable";
        publishSource();
      })
      .finally(() => {
        pending = null;
      });
    return pending;
  }

  /** @returns {Promise<void>} */
  function stop() {
    wanted = false;
    const fn = unlisten;
    unlisten = null;
    if (typeof fn !== "function") return Promise.resolve();
    return Promise.resolve(fn()).catch(() => {
      stats.failed += 1;
      stats.lastErrorKind = "unlisten";
    });
  }

  return {
    start,
    stop,
    handle,
    /** Снимок счётчиков без payload/секретов. */
    get stats() {
      return { ...stats };
    },
    /** @returns {string} */
    get source() {
      return stats.source;
    },
    /** @returns {import("./dispatch-model.js").DispatchStore} */
    get store() {
      return store;
    },
    /**
     * Авторитетный `selfUserId` из `ready()`: READY dispatch его не перебивает.
     * @param {unknown} value
     */
    setSelfUserId(value) {
      setDispatchSelfUserId(store, value);
    },
  };
}

/** @typedef {ReturnType<typeof createDispatchBridge>} DispatchBridge */
