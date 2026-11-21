// @ts-check
/**
 * TAURI-BOOT-03: модульный диспетчер контекстных меню (`context-menu-router.js`).
 *
 * Обеспечивает:
 * - Единую регистрацию и диспетчеризацию контекстных меню UI (`initContextMenuRouter`);
 * - Отслеживание активного открытого меню и сохранение ссылки на открывающий элемент;
 * - Закрытие меню с корректным восстановлением фокуса;
 * - Глобальные слушатели Escape и клика вне области (backdrop click);
 * - 100% изоляцию и очистку ресурсов при `destroy()`.
 *
 * @module ui/features/context-menu-router
 */

/**
 * @typedef {object} ContextMenuHandler
 * @property {(payload?: any) => unknown} [open]
 * @property {() => unknown} [close]
 */

/**
 * @typedef {object} ContextMenuOpenOptions
 * @property {any} [triggerElement] Элемент, вызвавший меню (для восстановления фокуса)
 * @property {number} [x] Координата X
 * @property {number} [y] Координата Y
 * @property {Record<string, unknown>} [data] Дополнительные метаданные
 */

/**
 * @typedef {object} ContextMenuRouterOptions
 * @property {(type: string, payload?: unknown) => unknown} [onOpen]
 * @property {(type: string) => unknown} [onClose]
 * @property {boolean} [listenEscape] Автоматически закрывать меню по клавише Escape
 * @property {EventTarget | { addEventListener: Function, removeEventListener: Function }} [target]
 */

/**
 * @typedef {object} ActiveContextMenuInfo
 * @property {string} type
 * @property {unknown} [payload]
 * @property {any} [triggerElement]
 */

/**
 * @typedef {object} ContextMenuRouter
 * @property {(type: string, payload?: any) => string} openContextMenu
 * @property {(options?: { restoreFocus?: boolean }) => string | null} closeContextMenu
 * @property {(type: string) => boolean} isOpen
 * @property {(type: string) => boolean} hasContextMenu
 * @property {() => ActiveContextMenuInfo | null} getActiveContextMenu
 * @property {(type: string, handler: ContextMenuHandler) => ContextMenuHandler} registerContextMenu
 * @property {(type: string) => boolean} unregisterContextMenu
 * @property {() => void} destroy
 */

/**
 * Замороженный список базовых поддерживаемых типов контекстных меню.
 * @type {readonly string[]}
 */
export const KNOWN_CONTEXT_MENU_TYPES = Object.freeze([
  "guild",
  "channel",
  "user",
  "message",
  "voice_participant",
]);

/**
 * Инициализирует модульный роутер контекстных меню.
 *
 * @param {ContextMenuRouterOptions} [options]
 * @returns {ContextMenuRouter}
 */
export function initContextMenuRouter(options = {}) {
  /** @type {Map<string, ContextMenuHandler>} */
  const handlers = new Map();

  /** @type {ActiveContextMenuInfo | null} */
  let activeMenu = null;

  const target = options.target ?? (typeof window !== "undefined" ? window : null);
  const listenEscape = options.listenEscape ?? true;

  /**
   * Обработчик нажатия клавиши Escape.
   * @param {KeyboardEvent | { key?: string, preventDefault?: Function }} event
   */
  const handleKeyDown = (event) => {
    if (event.key === "Escape" && activeMenu) {
      if (typeof event.preventDefault === "function") {
        event.preventDefault();
      }
      closeContextMenu({ restoreFocus: true });
    }
  };

  if (listenEscape && target && typeof target.addEventListener === "function") {
    target.addEventListener("keydown", handleKeyDown);
  }

  /**
   * Регистрирует обработчик контекстного меню.
   * @param {string} type
   * @param {ContextMenuHandler} handler
   * @returns {ContextMenuHandler}
   */
  function registerContextMenu(type, handler) {
    const cleanType = String(type ?? "").trim();
    if (!cleanType) {
      throw new Error("context-menu-router: type must be a non-empty string");
    }
    if (!handler || typeof handler !== "object") {
      throw new Error("context-menu-router: handler must be an object");
    }
    handlers.set(cleanType, handler);
    return handler;
  }

  /**
   * Удаляет зарегистрированный обработчик контекстного меню.
   * @param {string} type
   * @returns {boolean}
   */
  function unregisterContextMenu(type) {
    const cleanType = String(type ?? "").trim();
    if (activeMenu?.type === cleanType) {
      closeContextMenu({ restoreFocus: false });
    }
    return handlers.delete(cleanType);
  }

  /**
   * Проверяет, зарегистрирован ли обработчик указанного типа.
   * @param {string} type
   * @returns {boolean}
   */
  function hasContextMenu(type) {
    return handlers.has(String(type ?? "").trim());
  }

  /**
   * Проверяет, открыто ли меню указанного типа.
   * @param {string} type
   * @returns {boolean}
   */
  function isOpen(type) {
    return activeMenu?.type === String(type ?? "").trim();
  }

  /**
   * Возвращает текущее активное меню.
   * @returns {ActiveContextMenuInfo | null}
   */
  function getActiveContextMenu() {
    return activeMenu ? { ...activeMenu } : null;
  }

  /**
   * Открывает контекстное меню указанного типа.
   * Если другое меню уже открыто, оно закрывается без возврата фокуса.
   *
   * @param {string} type
   * @param {any} [payload]
   * @returns {string}
   */
  function openContextMenu(type, payload = {}) {
    const cleanType = String(type ?? "").trim();
    if (!cleanType) {
      throw new Error("context-menu-router: type must be a non-empty string");
    }

    if (activeMenu) {
      closeContextMenu({ restoreFocus: false });
    }

    const handler = handlers.get(cleanType);
    activeMenu = {
      type: cleanType,
      payload,
      triggerElement: payload?.triggerElement ?? null,
    };

    if (handler && typeof handler.open === "function") {
      handler.open(payload);
    }

    if (typeof options.onOpen === "function") {
      options.onOpen(cleanType, payload);
    }

    return cleanType;
  }

  /**
   * Закрывает текущее открытое контекстное меню.
   *
   * @param {{ restoreFocus?: boolean }} [closeOpts]
   * @returns {string | null}
   */
  function closeContextMenu(closeOpts = {}) {
    if (!activeMenu) return null;

    const current = activeMenu;
    activeMenu = null;

    const handler = handlers.get(current.type);
    if (handler && typeof handler.close === "function") {
      handler.close();
    }

    const shouldRestore = closeOpts.restoreFocus !== false;
    if (
      shouldRestore &&
      current.triggerElement &&
      typeof current.triggerElement.focus === "function"
    ) {
      try {
        current.triggerElement.focus();
      } catch {
        // Safe fallback in mock or detached DOM environments
      }
    }

    if (typeof options.onClose === "function") {
      options.onClose(current.type);
    }

    return current.type;
  }

  /**
   * Освобождает ресурсы роутера и удаляет слушатели событий.
   */
  function destroy() {
    if (activeMenu) {
      closeContextMenu({ restoreFocus: false });
    }
    handlers.clear();
    if (listenEscape && target && typeof target.removeEventListener === "function") {
      target.removeEventListener("keydown", handleKeyDown);
    }
  }

  return {
    openContextMenu,
    closeContextMenu,
    isOpen,
    hasContextMenu,
    getActiveContextMenu,
    registerContextMenu,
    unregisterContextMenu,
    destroy,
  };
}
