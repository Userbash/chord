// @ts-check
/**
 * T-236 (FE-P0-09) & TAURI-EVT-01: реестр фич оболочки UI, единый мост контроллеров
 * и событийно-ориентированный слушатель Tauri без CPU-поллинга (`shell-bridge.js`).
 *
 * Отвечает за:
 * - `SHELL_MODAL_LAYERS`: замороженный канонический реестр 10 слоёв модалок, поповеров и панелей;
 * - управление стеком открытых оверлеев `activeOverlayStack` (LIFO) и реестром контроллеров;
 * - взаимное исключение 5 полноэкранных/центрированных модальных окон
 *   (`quick-switcher`, `settings-modal`, `profile-modal`, `invite-modal`, `forward-modal`);
 * - обработку глобальных сочетаний клавиш (`Ctrl+K`, `Ctrl+F`, `Escape`) через `normalizeKeyCombo`;
 * - проксирование IPC-вызовов (`onDispatchIpc`) и навигации (`onNavigate`);
 * - TAURI-EVT-01: `createTauriEventListener` / `createEventDrivenListener` — подписка на события
 *   Tauri (например, `net://status`, `auth://state`, `gateway:dispatch`) без постоянного опроса CPU
 *   через `setInterval` (`NET_POLL_MS`), с поддержкой безопасного жизненного цикла, опционального
 *   `initialFetch` и изоляции ошибок.
 *
 * @module ui/features/shell-bridge
 */

import { DEFAULT_SHORTCUTS, normalizeKeyCombo } from "./keybinds.js";

/**
 * @typedef {"quick-switcher"
 *   | "settings-modal"
 *   | "profile-modal"
 *   | "invite-modal"
 *   | "forward-modal"
 *   | "expression-picker"
 *   | "context-menu"
 *   | "threads-drawer"
 *   | "search-panel"
 *   | "inbox-popover"} ShellLayerId
 */

/**
 * @typedef {object} ShellLayerController
 * @property {(payload?: any) => unknown} [open]
 * @property {() => unknown} [close]
 */

/**
 * @typedef {object} ShellFeatureBridgeOptions
 * @property {(command: string, payload?: unknown) => unknown} [onDispatchIpc]
 * @property {(target: unknown) => unknown} [onNavigate]
 */

/**
 * Канонический замороженный список всех слоёв оболочки (модалки, поповеры и боковые панели).
 * @type {readonly ShellLayerId[]}
 */
export const SHELL_MODAL_LAYERS = Object.freeze([
  "quick-switcher",
  "settings-modal",
  "profile-modal",
  "invite-modal",
  "forward-modal",
  "expression-picker",
  "context-menu",
  "threads-drawer",
  "search-panel",
  "inbox-popover",
]);

/**
 * Взаимно исключающие модальные окна: открытие любого из них автоматически закрывает
 * любое другое открытое модальное окно из этой группы.
 * @type {readonly ShellLayerId[]}
 */
export const EXCLUSIVE_MODAL_LAYERS = Object.freeze([
  "quick-switcher",
  "settings-modal",
  "profile-modal",
  "invite-modal",
  "forward-modal",
]);

/**
 * Проверяет, что переданный идентификатор входит в `SHELL_MODAL_LAYERS`.
 * @param {string} layerId
 * @returns {asserts layerId is ShellLayerId}
 */
function assertValidLayerId(layerId) {
  if (
    typeof layerId !== "string" ||
    !SHELL_MODAL_LAYERS.includes(/** @type {ShellLayerId} */ (layerId))
  ) {
    throw new Error(`Неизвестный слой оболочки UI: ${String(layerId)}`);
  }
}

/**
 * Проверяет, является ли слой взаимно исключающим модальным окном.
 * @param {string} layerId
 * @returns {boolean}
 */
function isExclusiveModalLayer(layerId) {
  return EXCLUSIVE_MODAL_LAYERS.includes(/** @type {ShellLayerId} */ (layerId));
}

/**
 * Создаёт реестр фич оболочки UI и единый мост контроллеров.
 * @param {ShellFeatureBridgeOptions} [options]
 */
export function createShellFeatureBridge({ onDispatchIpc, onNavigate } = {}) {
  /** @type {Map<ShellLayerId, ShellLayerController>} */
  const controllers = new Map();

  /** @type {ShellLayerId[]} */
  const activeOverlayStack = [];

  /**
   * Закрывает конкретный слой по индексу в стеке и вызывает его `close?.()`.
   * @param {number} index
   * @returns {ShellLayerId | null}
   */
  function closeLayerAtIndex(index) {
    const [closedLayerId] = activeOverlayStack.splice(index, 1);
    if (!closedLayerId) {
      return null;
    }
    const controller = controllers.get(closedLayerId);
    controller?.close?.();
    return closedLayerId;
  }

  /**
   * Регистрирует экземпляр контроллера для слоя из `SHELL_MODAL_LAYERS`.
   * @param {string} layerId
   * @param {ShellLayerController} controllerInstance
   * @returns {ShellLayerController}
   */
  function registerController(layerId, controllerInstance) {
    assertValidLayerId(layerId);
    if (!controllerInstance || typeof controllerInstance !== "object") {
      throw new TypeError(`Ожидался объект контроллера для слоя ${layerId}`);
    }
    controllers.set(layerId, controllerInstance);
    return controllerInstance;
  }

  /**
   * Возвращает зарегистрированный контроллер слоя (или `undefined`).
   * @param {string} layerId
   * @returns {ShellLayerController | undefined}
   */
  function getController(layerId) {
    assertValidLayerId(layerId);
    return controllers.get(layerId);
  }

  /**
   * Открывает указанный слой оболочки.
   * Если открывается взаимно исключающее модальное окно, сначала закрывает
   * любое уже открытое модальное окно из `EXCLUSIVE_MODAL_LAYERS`.
   * @param {string} layerId
   * @param {unknown} [payload]
   * @returns {ShellLayerId}
   */
  function openLayer(layerId, payload) {
    assertValidLayerId(layerId);

    if (isExclusiveModalLayer(layerId)) {
      for (let i = activeOverlayStack.length - 1; i >= 0; i -= 1) {
        const currentLayerId = activeOverlayStack[i];
        if (currentLayerId && isExclusiveModalLayer(currentLayerId)) {
          closeLayerAtIndex(i);
        }
      }
    } else {
      const existingIndex = activeOverlayStack.indexOf(layerId);
      if (existingIndex !== -1) {
        activeOverlayStack.splice(existingIndex, 1);
      }
    }

    const controller = controllers.get(layerId);
    controller?.open?.(payload);
    activeOverlayStack.push(layerId);
    return layerId;
  }

  /**
   * Закрывает верхний открытый слой из `activeOverlayStack` (LIFO).
   * @returns {ShellLayerId | null}
   */
  function closeTopLayer() {
    if (activeOverlayStack.length === 0) {
      return null;
    }
    const topLayerId = /** @type {ShellLayerId} */ (activeOverlayStack.pop());
    const controller = controllers.get(topLayerId);
    controller?.close?.();
    return topLayerId;
  }

  /**
   * Закрывает все открытые слои в строгом порядке LIFO.
   * @returns {ShellLayerId[]}
   */
  function closeAllLayers() {
    /** @type {ShellLayerId[]} */
    const closed = [];
    while (activeOverlayStack.length > 0) {
      const layerId = closeTopLayer();
      if (layerId !== null) {
        closed.push(layerId);
      }
    }
    return closed;
  }

  /**
   * Обрабатывает глобальное нажатие клавиши в оболочке приложения.
   * - `Ctrl+K` → открывает `"quick-switcher"`
   * - `Ctrl+F` → открывает `"search-panel"`
   * - `Escape` → вызывает `closeTopLayer()`
   * @param {(import("./keybinds.js").KeyEventLike & { preventDefault?: () => void }) | null | undefined} event
   * @returns {boolean}
   */
  function handleGlobalKeyDown(event) {
    const combo = normalizeKeyCombo(event);
    if (combo === null) {
      return false;
    }
    if (combo === DEFAULT_SHORTCUTS.QUICK_SWITCHER) {
      event?.preventDefault?.();
      openLayer("quick-switcher");
      return true;
    }
    if (combo === DEFAULT_SHORTCUTS.SEARCH) {
      event?.preventDefault?.();
      openLayer("search-panel");
      return true;
    }
    if (combo === DEFAULT_SHORTCUTS.CLOSE_MODAL) {
      event?.preventDefault?.();
      closeTopLayer();
      return true;
    }
    return false;
  }

  /**
   * Проверяет, открыт ли указанный слой в `activeOverlayStack`.
   * @param {string} layerId
   * @returns {boolean}
   */
  function isLayerOpen(layerId) {
    return activeOverlayStack.includes(/** @type {ShellLayerId} */ (layerId));
  }

  /**
   * Проксирует вызов IPC-команды в `onDispatchIpc`.
   * @param {string} command
   * @param {unknown} [payload]
   * @returns {unknown}
   */
  function dispatchIpc(command, payload) {
    return onDispatchIpc?.(command, payload);
  }

  /**
   * Проксирует переход навигации в `onNavigate`.
   * @param {unknown} target
   * @returns {unknown}
   */
  function navigate(target) {
    return onNavigate?.(target);
  }

  return {
    get activeOverlayStack() {
      return [...activeOverlayStack];
    },
    getActiveOverlayStack() {
      return [...activeOverlayStack];
    },
    registerController,
    getController,
    openLayer,
    closeTopLayer,
    closeAllLayers,
    handleGlobalKeyDown,
    isLayerOpen,
    dispatchIpc,
    navigate,
    /**
     * Создаёт событийно-ориентированный слушатель Tauri без CPU-поллинга.
     * @param {TauriEventListenerOptions} listenerOptions
     * @returns {TauriEventListenerController}
     */
    createEventListener(listenerOptions) {
      return createTauriEventListener(listenerOptions);
    },
  };
}

/**
 * @typedef {object} TauriEventListenerOptions
 * @property {string} event
 *   Tauri event name to subscribe to (e.g. "net://status", "net_status", or "auth://state").
 * @property {(payload: any, rawEvent?: any) => void} onEvent
 *   Callback invoked when the Tauri event is received. Receives unwrapped payload and raw event.
 * @property {((event: string, handler: (payload: any) => void) => Promise<(() => void) | void> | (() => void) | void) | null} [listen]
 *   Custom listen function (defaults to resolveTauriListen()).
 * @property {(() => Promise<unknown> | unknown)} [initialFetch]
 *   Optional one-shot initial fetch function executed on start() to obtain initial state
 *   immediately without CPU polling or waiting for the first push event.
 * @property {(error: unknown) => void} [onError]
 *   Optional error handler for listen setup or event processing failures.
 * @property {boolean} [autoStart]
 *   Whether to start listening immediately upon creation (default: false).
 */

/**
 * @typedef {object} TauriEventListenerController
 * @property {() => Promise<boolean>} start
 *   Starts listening for events and runs initialFetch if provided. Idempotent.
 * @property {() => Promise<void>} stop
 *   Stops listening and cleans up active unlisten subscription.
 * @property {() => boolean} isListening
 *   Returns true if the listener is currently active.
 * @property {() => Promise<void>} dispose
 *   Permanently disposes the listener and cancels any active subscription.
 * @property {(payload: any) => void} emitMock
 *   Dispatches a mock event payload to the onEvent callback (useful in tests/QA).
 */

/**
 * Resolves the Tauri event listen function from the global environment.
 * Checks `window.__TAURI__.event.listen` or custom injected window.
 *
 * @param {any} [win] Window object or global context.
 * @returns {((event: string, handler: (payload: any) => void) => Promise<(() => void) | void> | (() => void) | void) | null}
 */
export function resolveTauriListen(win = (typeof window !== "undefined" ? window : /** @type {any} */ (globalThis).window)) {
  const listen = win?.__TAURI__?.event?.listen;
  return typeof listen === "function" ? listen : null;
}

/**
 * Unwraps Tauri event envelope if present, extracting `payload`.
 * Tauri events are dispatched as `{ event: string, id: number, payload: any }`.
 *
 * @param {unknown} value
 * @returns {unknown}
 */
export function unwrapTauriPayload(value) {
  if (value !== null && typeof value === "object" && "payload" in value) {
    return /** @type {{ payload: unknown }} */ (value).payload;
  }
  return value;
}

/**
 * Creates an event-driven listener for Tauri events without CPU polling (TAURI-EVT-01).
 *
 * Replaces periodic `setInterval` CPU polling (such as `NET_POLL_MS` net_status polling)
 * with a reactive push subscription using Tauri's event bus.
 *
 * Key advantages over polling:
 * 1. Zero timer wakeups: eliminates `setInterval` overhead, avoiding battery drain and CPU spikes.
 * 2. Instant notification: updates propagate in sub-millisecond time rather than waiting for next poll cycle.
 * 3. Graceful fallback: handles environments where Tauri event transport is unavailable.
 * 4. Safe lifecycle: idempotent start/stop, cleans up subscriptions and prevents memory/event leaks.
 *
 * @param {TauriEventListenerOptions} options
 * @returns {TauriEventListenerController}
 */
export function createTauriEventListener(options) {
  const {
    event: eventName,
    onEvent,
    listen: customListen,
    initialFetch,
    onError,
    autoStart = false,
  } = options;

  if (typeof eventName !== "string" || eventName.trim() === "") {
    throw new TypeError("TauriEventListener: 'event' must be a non-empty string");
  }
  if (typeof onEvent !== "function") {
    throw new TypeError("TauriEventListener: 'onEvent' must be a function");
  }

  const resolveListenFn = customListen !== undefined
    ? () => customListen
    : () => resolveTauriListen();

  /** @type {(() => void) | null} */
  let unlistenFn = null;
  /** @type {Promise<void> | null} */
  let pendingSubscription = null;
  let active = false;
  let disposed = false;

  /**
   * Starts listening to the Tauri event.
   * Runs `initialFetch` if provided and hooks into the event bus.
   * @returns {Promise<boolean>}
   */
  async function start() {
    if (disposed || active) {
      return active;
    }
    active = true;

    // Run initial fetch if provided (to initialize state without CPU polling)
    if (typeof initialFetch === "function") {
      try {
        const initialValue = await initialFetch();
        if (active && !disposed) {
          onEvent(unwrapTauriPayload(initialValue), initialValue);
        }
      } catch (err) {
        if (typeof onError === "function") {
          onError(err);
        }
      }
    }

    if (!active || disposed) {
      return false;
    }

    const listen = resolveListenFn();
    if (typeof listen !== "function") {
      // Event transport not available in this environment
      return false;
    }

    if (unlistenFn || pendingSubscription) {
      return true;
    }

    pendingSubscription = (async () => {
      try {
        const dispose = await listen(eventName, (rawEvent) => {
          if (!active || disposed) return;
          try {
            const payload = unwrapTauriPayload(rawEvent);
            onEvent(payload, rawEvent);
          } catch (dispatchErr) {
            onError?.(dispatchErr);
          }
        });

        if (!active || disposed) {
          if (typeof dispose === "function") {
            try {
              dispose();
            } catch {
              /* ignore */
            }
          }
          unlistenFn = null;
        } else {
          unlistenFn = typeof dispose === "function" ? dispose : null;
        }
      } catch (subErr) {
        onError?.(subErr);
      } finally {
        pendingSubscription = null;
      }
    })();

    await pendingSubscription;
    return active;
  }

  /**
   * Stops listening and cleans up subscriptions.
   * @returns {Promise<void>}
   */
  async function stop() {
    active = false;
    if (pendingSubscription) {
      try {
        await pendingSubscription;
      } catch {
        /* ignore */
      }
    }
    if (unlistenFn) {
      const fn = unlistenFn;
      unlistenFn = null;
      try {
        fn();
      } catch {
        /* ignore */
      }
    }
  }

  /**
   * Returns whether the listener is actively listening.
   * @returns {boolean}
   */
  function isListening() {
    return active && !disposed;
  }

  /**
   * Permanently disposes the listener.
   * @returns {Promise<void>}
   */
  async function dispose() {
    disposed = true;
    await stop();
  }

  /**
   * Emits a mock payload directly (useful for tests and QA verification).
   * @param {unknown} mockPayload
   */
  function emitMock(mockPayload) {
    if (disposed) return;
    try {
      const payload = unwrapTauriPayload(mockPayload);
      onEvent(payload, mockPayload);
    } catch (err) {
      onError?.(err);
    }
  }

  if (autoStart) {
    void start();
  }

  return {
    start,
    stop,
    isListening,
    dispose,
    emitMock,
  };
}

/**
 * Event-driven listener helper for Tauri events without CPU polling (TAURI-EVT-01).
 * Alias for `createTauriEventListener`.
 */
export const createEventDrivenListener = createTauriEventListener;

/**
 * One-shot helper to listen for a Tauri event and return an unlisten callback.
 *
 * @param {string} event
 * @param {(payload: any, rawEvent?: any) => void} onEvent
 * @param {Omit<TauriEventListenerOptions, "event" | "onEvent" | "autoStart">} [options]
 * @returns {() => Promise<void>}
 */
export function listenTauriEvent(event, onEvent, options = {}) {
  const listener = createTauriEventListener({
    event,
    onEvent,
    ...options,
    autoStart: true,
  });
  return () => listener.stop();
}
