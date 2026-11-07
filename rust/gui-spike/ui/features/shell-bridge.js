// @ts-check
/**
 * T-236 (FE-P0-09): реестр фич оболочки UI и единый мост контроллеров (`shell-bridge.js`).
 *
 * Отвечает за:
 * - `SHELL_MODAL_LAYERS`: замороженный канонический реестр 10 слоёв модалок, поповеров и панелей;
 * - управление стеком открытых оверлеев `activeOverlayStack` (LIFO) и реестром контроллеров;
 * - взаимное исключение 5 полноэкранных/центрированных модальных окон
 *   (`quick-switcher`, `settings-modal`, `profile-modal`, `invite-modal`, `forward-modal`);
 * - обработку глобальных сочетаний клавиш (`Ctrl+K`, `Ctrl+F`, `Escape`) через `normalizeKeyCombo`;
 * - проксирование IPC-вызовов (`onDispatchIpc`) и навигации (`onNavigate`).
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
  };
}
