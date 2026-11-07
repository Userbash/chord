// @ts-check
/**
 * T-227 (FE-P1-23 / RW-40 / G2): контроллер контекстного меню и клавиатурной
 * навигации для сообщений, каналов и пользователей (`context-menu.js`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` (§5 п.3–4 Menu / ContextMenu:
 *   `menu_c1e9c4`, `role="menu"`, `role="menuitem"`, `colorDanger`,
 *   `CONTEXT_MENU_OPEN` / `CONTEXT_MENU_CLOSE`);
 * - `/home/sanya/re-deep/ui/01-msg-menu-items.json`, `01-msg-menu.html`.
 *
 * @module ui/features/context-menu
 */

/**
 * Отступ контекстного меню от краёв viewport (px).
 * @type {8}
 */
const VIEWPORT_EDGE_PADDING_PX = 8;

/**
 * Ширина контекстного меню по умолчанию для расчёта границ viewport (px).
 * @type {220}
 */
const DEFAULT_MENU_WIDTH_PX = 220;

/**
 * Высота контекстного меню по умолчанию для расчёта границ viewport (px).
 * @type {240}
 */
const DEFAULT_MENU_HEIGHT_PX = 240;

/**
 * Ширина viewport по умолчанию (px).
 * @type {1280}
 */
const DEFAULT_VIEWPORT_WIDTH_PX = 1280;

/**
 * Высота viewport по умолчанию (px).
 * @type {720}
 */
const DEFAULT_VIEWPORT_HEIGHT_PX = 720;

/**
 * @typedef {"message" | "channel" | "user"} ContextMenuTargetType
 */

/**
 * @typedef {object} ContextMenuItem
 * @property {string} id
 * @property {string} label
 * @property {boolean} danger
 */

/**
 * @typedef {object} BuildContextMenuItemsOptions
 * @property {ContextMenuTargetType} targetType
 * @property {boolean} [isOwnMessage]
 * @property {boolean} [isPinned]
 * @property {boolean} [isMuted]
 * @property {boolean} [canManageMessages]
 */

/**
 * @typedef {object} ClampMenuCoordinatesInput
 * @property {number} x
 * @property {number} y
 * @property {number} menuWidth
 * @property {number} menuHeight
 * @property {number} viewportWidth
 * @property {number} viewportHeight
 */

/**
 * @typedef {object} OpenContextMenuOptions
 * @property {number} [x]
 * @property {number} [y]
 * @property {number} [menuWidth]
 * @property {number} [menuHeight]
 * @property {number} [viewportWidth]
 * @property {number} [viewportHeight]
 * @property {ContextMenuTargetType} targetType
 * @property {string} [targetId]
 * @property {boolean} [isOwnMessage]
 * @property {boolean} [isPinned]
 * @property {boolean} [isMuted]
 * @property {boolean} [canManageMessages]
 */

/**
 * @typedef {object} ContextMenuSelectionPayload
 * @property {string} actionId
 * @property {string} id
 * @property {string} label
 * @property {boolean} danger
 * @property {ContextMenuTargetType} targetType
 * @property {string} targetId
 */

/**
 * @typedef {object} ContextMenuControllerOptions
 * @property {any} container
 * @property {(payload: ContextMenuSelectionPayload) => void} [onSelectAction]
 */

/**
 * Формирует список пунктов контекстного меню для указанного типа цели и флагов прав/состояния.
 *
 * @param {BuildContextMenuItemsOptions} options
 * @returns {ContextMenuItem[]}
 */
export function buildContextMenuItems(options) {
  const opts = options && typeof options === "object" ? options : /** @type {BuildContextMenuItemsOptions} */ ({});
  const targetType = opts.targetType;
  const isOwnMessage = Boolean(opts.isOwnMessage);
  const isPinned = Boolean(opts.isPinned);
  const isMuted = Boolean(opts.isMuted);
  const canManageMessages = Boolean(opts.canManageMessages);

  if (targetType === "message") {
    /** @type {ContextMenuItem[]} */
    const items = [
      { id: "reply", label: "Ответить", danger: false },
      { id: "copy-text", label: "Скопировать текст", danger: false },
    ];
    if (canManageMessages) {
      items.push({
        id: "pin",
        label: isPinned ? "Открепить сообщение" : "Закрепить сообщение",
        danger: false,
      });
    }
    if (isOwnMessage) {
      items.push({
        id: "edit",
        label: "Изменить сообщение",
        danger: false,
      });
    }
    if (isOwnMessage || canManageMessages) {
      items.push({
        id: "delete",
        label: "Удалить сообщение",
        danger: true,
      });
    }
    items.push({
      id: "copy-id",
      label: "Скопировать ID сообщения",
      danger: false,
    });
    return items;
  }

  if (targetType === "channel") {
    return [
      { id: "mark-read", label: "Отметить как прочитанное", danger: false },
      {
        id: "mute-channel",
        label: isMuted ? "Включить уведомления канала" : "Заглушить канал",
        danger: false,
      },
      { id: "copy-id", label: "Скопировать ID канала", danger: false },
    ];
  }

  if (targetType === "user") {
    return [
      { id: "profile", label: "Профиль", danger: false },
      { id: "message-user", label: "Написать сообщение", danger: false },
      { id: "copy-id", label: "Скопировать ID пользователя", danger: false },
    ];
  }

  return [];
}

/**
 * Ограничивает координаты контекстного меню пределами viewport с отступом 8px:
 * - `x` зажимается в `[0, Math.max(0, viewportWidth - menuWidth - 8)]`,
 * - `y` зажимается в `[0, Math.max(0, viewportHeight - menuHeight - 8)]`.
 *
 * @param {ClampMenuCoordinatesInput} input
 * @returns {{ x: number, y: number }}
 */
export function clampMenuCoordinates(input) {
  const src = input && typeof input === "object" ? input : /** @type {ClampMenuCoordinatesInput} */ ({});
  const rawX = typeof src.x === "number" && Number.isFinite(src.x) ? src.x : 0;
  const rawY = typeof src.y === "number" && Number.isFinite(src.y) ? src.y : 0;
  const menuWidth =
    typeof src.menuWidth === "number" && Number.isFinite(src.menuWidth) && src.menuWidth >= 0
      ? src.menuWidth
      : 0;
  const menuHeight =
    typeof src.menuHeight === "number" && Number.isFinite(src.menuHeight) && src.menuHeight >= 0
      ? src.menuHeight
      : 0;
  const viewportWidth =
    typeof src.viewportWidth === "number" && Number.isFinite(src.viewportWidth) && src.viewportWidth >= 0
      ? src.viewportWidth
      : 0;
  const viewportHeight =
    typeof src.viewportHeight === "number" && Number.isFinite(src.viewportHeight) && src.viewportHeight >= 0
      ? src.viewportHeight
      : 0;

  const maxX = Math.max(0, viewportWidth - menuWidth - VIEWPORT_EDGE_PADDING_PX);
  const maxY = Math.max(0, viewportHeight - menuHeight - VIEWPORT_EDGE_PADDING_PX);

  return {
    x: Math.min(Math.max(0, rawX), maxX),
    y: Math.min(Math.max(0, rawY), maxY),
  };
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
  if (!containerEl || typeof containerEl !== "object") {
    throw new TypeError("Container element must be a valid DOM object");
  }
  if (
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("Container element must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы элемента.
 *
 * @param {any} parentEl
 * @param {any[]} nodes
 */
function replaceElementChildren(parentEl, nodes) {
  if (typeof parentEl.replaceChildren === "function") {
    parentEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(parentEl.children)) {
    parentEl.children.length = 0;
  }
  for (const node of nodes) {
    parentEl.appendChild(node);
  }
}

/**
 * Создаёт контроллер контекстного меню с поддержкой клавиатурной навигации.
 *
 * @param {ContextMenuControllerOptions} options
 */
export function createContextMenuController(options) {
  const opts = options && typeof options === "object" ? options : /** @type {ContextMenuControllerOptions} */ ({});
  const container = opts.container;
  const doc = resolveDocument(container);
  const onSelectAction =
    typeof opts.onSelectAction === "function" ? opts.onSelectAction : null;

  /** @type {ContextMenuItem[]} */
  let currentItems = [];
  /** @type {any[]} */
  let itemElements = [];
  /** @type {ContextMenuTargetType | null} */
  let currentTargetType = null;
  let currentTargetId = "";
  let focusedIndex = -1;
  let openState = false;

  const menuEl = doc.createElement("div");
  menuEl.setAttribute("data-context-menu", "");
  menuEl.setAttribute("role", "menu");
  menuEl.setAttribute("aria-label", "Контекстное меню");
  menuEl.setAttribute("tabindex", "-1");
  menuEl.setAttribute("data-open", "false");
  menuEl.setAttribute("aria-hidden", "true");
  menuEl.setAttribute("hidden", "");

  /**
   * Синхронизирует DOM-атрибуты фокуса на пунктах меню и переводит фокус на активный элемент.
   *
   * @param {number} nextIndex
   */
  function applyFocusAt(nextIndex) {
    if (!openState || currentItems.length === 0) {
      focusedIndex = -1;
      return;
    }
    const len = currentItems.length;
    focusedIndex = (((nextIndex % len) + len) % len);
    for (let i = 0; i < itemElements.length; i += 1) {
      const el = itemElements[i];
      const isFocused = i === focusedIndex;
      el.setAttribute("tabindex", isFocused ? "0" : "-1");
      el.setAttribute("data-focused", isFocused ? "true" : "false");
      if (isFocused && typeof el.focus === "function") {
        el.focus();
      }
    }
  }

  /**
   * Циклически сдвигает фокус по пунктам открытого меню на `delta`.
   *
   * @param {number} delta
   * @returns {number}
   */
  function moveFocus(delta) {
    if (!openState || currentItems.length === 0) {
      return -1;
    }
    const step = typeof delta === "number" && Number.isFinite(delta) ? Math.trunc(delta) : 0;
    const base = focusedIndex >= 0 ? focusedIndex : 0;
    applyFocusAt(base + step);
    return focusedIndex;
  }

  /**
   * Закрывает контекстное меню и очищает список пунктов.
   */
  function close() {
    openState = false;
    currentItems = [];
    itemElements = [];
    currentTargetType = null;
    currentTargetId = "";
    focusedIndex = -1;
    replaceElementChildren(menuEl, []);
    menuEl.setAttribute("data-open", "false");
    menuEl.setAttribute("aria-hidden", "true");
    menuEl.setAttribute("hidden", "");
    menuEl.removeAttribute("data-target-type");
    menuEl.removeAttribute("data-target-id");
  }

  /**
   * Активирует пункт меню по индексу, вызывает `onSelectAction` и закрывает меню.
   *
   * @param {number} index
   * @returns {ContextMenuSelectionPayload | null}
   */
  function selectItemAt(index) {
    if (!openState || index < 0 || index >= currentItems.length || !currentTargetType) {
      return null;
    }
    const item = currentItems[index];
    if (!item) {
      return null;
    }
    /** @type {ContextMenuSelectionPayload} */
    const payload = {
      actionId: item.id,
      id: item.id,
      label: item.label,
      danger: item.danger,
      targetType: currentTargetType,
      targetId: currentTargetId,
    };
    close();
    if (onSelectAction) {
      onSelectAction(payload);
    }
    return payload;
  }

  /**
   * Активирует текущий сфокусированный пункт меню.
   *
   * @returns {ContextMenuSelectionPayload | null}
   */
  function activateFocusedItem() {
    return selectItemAt(focusedIndex);
  }

  /**
   * Открывает контекстное меню в заданных координатах для указанной цели.
   *
   * @param {OpenContextMenuOptions} openOptions
   * @returns {boolean}
   */
  function open(openOptions) {
    const cfg =
      openOptions && typeof openOptions === "object"
        ? openOptions
        : /** @type {OpenContextMenuOptions} */ ({});
    const builtItems = buildContextMenuItems({
      targetType: cfg.targetType,
      isOwnMessage: cfg.isOwnMessage,
      isPinned: cfg.isPinned,
      isMuted: cfg.isMuted,
      canManageMessages: cfg.canManageMessages,
    });
    if (builtItems.length === 0) {
      close();
      return false;
    }

    const coords = clampMenuCoordinates({
      x: cfg.x ?? 0,
      y: cfg.y ?? 0,
      menuWidth: cfg.menuWidth ?? DEFAULT_MENU_WIDTH_PX,
      menuHeight: cfg.menuHeight ?? DEFAULT_MENU_HEIGHT_PX,
      viewportWidth: cfg.viewportWidth ?? DEFAULT_VIEWPORT_WIDTH_PX,
      viewportHeight: cfg.viewportHeight ?? DEFAULT_VIEWPORT_HEIGHT_PX,
    });

    currentItems = builtItems;
    currentTargetType = cfg.targetType;
    currentTargetId = cfg.targetId !== undefined && cfg.targetId !== null ? String(cfg.targetId) : "";
    openState = true;

    itemElements = currentItems.map((item, idx) => {
      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("role", "menuitem");
      btn.setAttribute("data-context-item", item.id);
      btn.setAttribute("data-action-id", item.id);
      if (item.danger) {
        btn.setAttribute("data-danger", "true");
      }
      btn.textContent = item.label;
      btn.addEventListener("click", (/** @type {any} */ event) => {
        if (event && typeof event.stopPropagation === "function") {
          event.stopPropagation();
        }
        focusedIndex = idx;
        selectItemAt(idx);
      });
      return btn;
    });

    replaceElementChildren(menuEl, itemElements);
    menuEl.removeAttribute("hidden");
    menuEl.setAttribute("data-open", "true");
    menuEl.setAttribute("aria-hidden", "false");
    menuEl.setAttribute("data-target-type", String(currentTargetType));
    menuEl.setAttribute("data-target-id", currentTargetId);
    menuEl.setAttribute("data-x", String(coords.x));
    menuEl.setAttribute("data-y", String(coords.y));
    menuEl.setAttribute("style", `left: ${coords.x}px; top: ${coords.y}px;`);

    applyFocusAt(0);
    return true;
  }

  menuEl.addEventListener("keydown", (/** @type {any} */ event) => {
    if (!openState) return;
    const key = event && typeof event.key === "string" ? event.key : "";
    if (key === "ArrowDown") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      moveFocus(1);
    } else if (key === "ArrowUp") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      moveFocus(-1);
    } else if (key === "Home") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      applyFocusAt(0);
    } else if (key === "End") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      applyFocusAt(currentItems.length - 1);
    } else if (key === "Enter" || key === " ") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      activateFocusedItem();
    } else if (key === "Escape") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      close();
    }
  });

  container.appendChild(menuEl);

  return {
    open,
    moveFocus,
    activateFocusedItem,
    close,
    isOpen: () => openState,
    getFocusedIndex: () => focusedIndex,
  };
}
