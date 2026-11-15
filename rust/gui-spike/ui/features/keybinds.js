// @ts-check
/**
 * T-224 (FE-P2-23) & TAURI-BOOT-01: парсер горячих клавиш, модульный контроллер
 * регистрации шорткатов (initKeybinds, handleKeybind) и русская подпись для ARIA.
 *
 * Основной контракт:
 * - набор комбинаций по умолчанию (`DEFAULT_SHORTCUTS`),
 * - нормализация события клавиатуры в стабильную строку вида "Ctrl+Shift+M",
 * - сравнение события с сохранённым комбо,
 * - текст подписи `aria-keyshortcuts` на русском,
 * - модульный контроллер клавиатурных комбинаций `initKeybinds(app)` и диспетчер `handleKeybind(combo, app)`.
 */

/**
 * @typedef {object} KeyEventLike
 * @property {string} [key] нажатая клавиша
 * @property {boolean} [ctrlKey] зажат Ctrl
 * @property {boolean} [metaKey] зажат Meta (Cmd)
 * @property {boolean} [shiftKey] зажат Shift
 * @property {boolean} [altKey] зажат Alt
 */

/**
 * Комбинации по умолчанию. Порядок модификаторов — Ctrl, Alt, Shift.
 * @type {Readonly<Record<string, string>>}
 */
export const DEFAULT_SHORTCUTS = Object.freeze({
  QUICK_SWITCHER: "Ctrl+K",
  SEARCH: "Ctrl+F",
  TOGGLE_MUTE: "Ctrl+Shift+M",
  TOGGLE_DEAFEN: "Ctrl+Shift+D",
  MARK_SERVER_READ: "Shift+Escape",
  CLOSE_MODAL: "Escape",
});

/** @type {readonly string[]} */
const MODIFIER_KEYS = Object.freeze(["Control", "Shift", "Alt", "Meta"]);

/**
 * Приводит клавишу к виду, используемому в комбо: одиночный символ — в верхний
 * регистр, пробел — "Space", остальное без изменений.
 * @param {string} key
 * @returns {string}
 */
function normalizeKey(key) {
  if (key === " ") return "Space";
  return key.length === 1 ? key.toUpperCase() : key;
}

/**
 * Собирает строку-комбо из события клавиатуры.
 * Возвращает null, если клавиша пустая или сам вызов — это модификатор.
 * @param {KeyEventLike | null | undefined} event
 * @returns {string | null}
 */
export function normalizeKeyCombo(event) {
  if (!event) return null;
  const key = typeof event.key === "string" ? event.key : "";
  if (!key) return null;
  if (MODIFIER_KEYS.includes(key)) return null;

  /** @type {string[]} */
  const parts = [];
  if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  parts.push(normalizeKey(key));
  return parts.join("+");
}

/**
 * Проверяет, что событие соответствует сохранённому комбо.
 * @param {KeyEventLike | null | undefined} event
 * @param {string | null | undefined} comboString
 * @returns {boolean}
 */
export function matchesShortcut(event, comboString) {
  const normalized = normalizeKeyCombo(event);
  if (normalized === null) return false;
  return normalized === comboString;
}

/**
 * Строит русскую подпись для `aria-keyshortcuts`.
 * @param {string | null | undefined} comboString
 * @returns {string}
 */
export function formatShortcutAriaLabelRu(comboString) {
  return `Горячая клавиша: ${comboString || "не назначена"}`;
}

/**
 * Dispatches a normalized keyboard combo to the application controller (TAURI-BOOT-01).
 *
 * Supported default shortcuts:
 * - `Ctrl+K`: Opens quick switcher (`openQuickSwitcher`, `quickSwitcher.open`, or `shellBridge.openLayer`)
 * - `Ctrl+F`: Opens search panel (`openSearch`, `search.open`, or `shellBridge.openLayer`)
 * - `Escape`: Closes top modal (`closeTopModal`, `closeModal`, or `shellBridge.closeTopLayer`)
 * - `Ctrl+Shift+M`: Toggles mute (`toggleMute`)
 * - `Ctrl+Shift+D`: Toggles deafen (`toggleDeafen`)
 * - `Shift+Escape`: Marks server as read (`markServerRead`)
 *
 * Also checks custom keybinds registered in `app.customKeybinds`, `app.keybinds`, or `app.handlers`.
 *
 * @param {string | KeyEventLike | null | undefined} combo
 *   Normalized combo string (e.g. "Ctrl+K") or a KeyEventLike object.
 * @param {any} [app]
 *   Application context or mock controller.
 * @returns {boolean}
 *   True if the keybind was handled, false otherwise.
 */
export function handleKeybind(combo, app = {}) {
  const comboString = typeof combo === "string"
    ? combo
    : normalizeKeyCombo(combo);

  if (!comboString) return false;

  // 1. Custom handlers on app
  if (app && typeof app === "object") {
    if (app.customKeybinds instanceof Map && app.customKeybinds.has(comboString)) {
      const handler = app.customKeybinds.get(comboString);
      const res = handler(app, comboString);
      return res !== false;
    }
    if (app.customKeybinds && typeof app.customKeybinds[comboString] === "function") {
      const res = app.customKeybinds[comboString](app, comboString);
      return res !== false;
    }
    if (app.keybinds instanceof Map && app.keybinds.has(comboString)) {
      const handler = app.keybinds.get(comboString);
      const res = handler(app, comboString);
      return res !== false;
    }
    if (app.keybinds && typeof app.keybinds[comboString] === "function") {
      const res = app.keybinds[comboString](app, comboString);
      return res !== false;
    }
    if (app.handlers && typeof app.handlers[comboString] === "function") {
      const res = app.handlers[comboString](app, comboString);
      return res !== false;
    }
  }

  // 2. Default application shortcuts
  if (comboString === DEFAULT_SHORTCUTS.QUICK_SWITCHER) {
    if (typeof app?.openQuickSwitcher === "function") {
      app.openQuickSwitcher();
      return true;
    }
    if (typeof app?.quickSwitcher?.open === "function") {
      app.quickSwitcher.open();
      return true;
    }
    if (typeof app?.shellBridge?.openLayer === "function") {
      app.shellBridge.openLayer("quick-switcher");
      return true;
    }
    if (typeof app?.openLayer === "function") {
      app.openLayer("quick-switcher");
      return true;
    }
    if (typeof app?.onQuickSwitcher === "function") {
      app.onQuickSwitcher();
      return true;
    }
    return false;
  }

  if (comboString === DEFAULT_SHORTCUTS.SEARCH) {
    if (typeof app?.openSearch === "function") {
      app.openSearch();
      return true;
    }
    if (typeof app?.search?.open === "function") {
      app.search.open();
      return true;
    }
    if (typeof app?.shellBridge?.openLayer === "function") {
      app.shellBridge.openLayer("search-panel");
      return true;
    }
    if (typeof app?.openLayer === "function") {
      app.openLayer("search-panel");
      return true;
    }
    if (typeof app?.onSearch === "function") {
      app.onSearch();
      return true;
    }
    return false;
  }

  if (comboString === DEFAULT_SHORTCUTS.CLOSE_MODAL) {
    if (typeof app?.closeTopModal === "function") {
      app.closeTopModal();
      return true;
    }
    if (typeof app?.closeModal === "function") {
      app.closeModal();
      return true;
    }
    if (typeof app?.shellBridge?.closeTopLayer === "function") {
      app.shellBridge.closeTopLayer();
      return true;
    }
    if (typeof app?.closeTopLayer === "function") {
      app.closeTopLayer();
      return true;
    }
    if (typeof app?.onEscape === "function") {
      app.onEscape();
      return true;
    }
    return false;
  }

  if (comboString === DEFAULT_SHORTCUTS.TOGGLE_MUTE) {
    if (typeof app?.toggleMute === "function") {
      app.toggleMute();
      return true;
    }
    if (typeof app?.voice?.toggleMute === "function") {
      app.voice.toggleMute();
      return true;
    }
    return false;
  }

  if (comboString === DEFAULT_SHORTCUTS.TOGGLE_DEAFEN) {
    if (typeof app?.toggleDeafen === "function") {
      app.toggleDeafen();
      return true;
    }
    if (typeof app?.voice?.toggleDeafen === "function") {
      app.voice.toggleDeafen();
      return true;
    }
    return false;
  }

  if (comboString === DEFAULT_SHORTCUTS.MARK_SERVER_READ) {
    if (typeof app?.markServerRead === "function") {
      app.markServerRead();
      return true;
    }
    return false;
  }

  return false;
}

/**
 * @typedef {object} KeybindsController
 * @property {() => void} destroy
 *   Removes the keydown listener and cleans up bindings.
 * @property {() => void} unbind
 *   Alias for destroy.
 * @property {(comboOrEvent: string | KeyEventLike) => boolean} handleKeybind
 *   Manually dispatches a keybind through the controller.
 * @property {(combo: string, handler: (app: any, combo: string) => unknown) => void} register
 *   Registers a custom keybind handler.
 * @property {(combo: string) => boolean} unregister
 *   Unregisters a custom keybind handler.
 */

/**
 * Initializes keyboard shortcut registration on the target window or document (TAURI-BOOT-01).
 *
 * @param {any} [app] Application instance or context.
 * @param {object} [options]
 * @param {EventTarget | { addEventListener: Function, removeEventListener: Function }} [options.target]
 *   Target DOM or EventTarget to attach keydown listener to (defaults to window/document).
 * @param {boolean} [options.preventDefault]
 *   Whether to prevent default browser behavior when shortcut is handled (default: true).
 * @returns {KeybindsController}
 */
export function initKeybinds(app = {}, options = {}) {
  const target = options.target ??
    app?.target ??
    app?.window ??
    app?.document ??
    (typeof window !== "undefined" ? window : (typeof document !== "undefined" ? document : null));

  /** @type {Map<string, (app: any, combo: string) => unknown>} */
  const localKeybinds = new Map();

  const preventDefault = options.preventDefault !== false;

  /**
   * @param {string} combo
   * @param {(app: any, combo: string) => unknown} handler
   */
  function register(combo, handler) {
    if (typeof combo !== "string" || typeof handler !== "function") return;
    localKeybinds.set(combo, handler);
    if (app && typeof app === "object") {
      if (!app.customKeybinds) {
        app.customKeybinds = new Map();
      }
      if (app.customKeybinds instanceof Map) {
        app.customKeybinds.set(combo, handler);
      } else if (typeof app.customKeybinds === "object") {
        app.customKeybinds[combo] = handler;
      }
    }
  }

  /**
   * @param {string} combo
   * @returns {boolean}
   */
  function unregister(combo) {
    const deleted = localKeybinds.delete(combo);
    if (app?.customKeybinds instanceof Map) {
      app.customKeybinds.delete(combo);
    } else if (app?.customKeybinds && typeof app.customKeybinds === "object") {
      delete app.customKeybinds[combo];
    }
    return deleted;
  }

  /**
   * @param {string | KeyEventLike} comboOrEvent
   * @returns {boolean}
   */
  function dispatch(comboOrEvent) {
    const comboString = typeof comboOrEvent === "string"
      ? comboOrEvent
      : normalizeKeyCombo(comboOrEvent);

    if (!comboString) return false;

    if (localKeybinds.has(comboString)) {
      const handler = localKeybinds.get(comboString);
      const res = handler?.(app, comboString);
      return res !== false;
    }

    return handleKeybind(comboString, app);
  }

  /**
   * @param {any} event
   */
  function onKeyDown(event) {
    const combo = normalizeKeyCombo(event);
    if (!combo) return;

    const handled = dispatch(combo);
    if (handled && preventDefault) {
      event?.preventDefault?.();
      event?.stopPropagation?.();
    }
  }

  if (target && typeof target.addEventListener === "function") {
    target.addEventListener("keydown", onKeyDown);
  }

  function destroy() {
    if (target && typeof target.removeEventListener === "function") {
      target.removeEventListener("keydown", onKeyDown);
    }
    localKeybinds.clear();
  }

  return {
    destroy,
    unbind: destroy,
    handleKeybind: dispatch,
    register,
    unregister,
  };
}
