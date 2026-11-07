// @ts-check
/**
 * T-227 (FE-P1-23 / RW-40 / G2) & TASK-620: унифицированный контроллер контекстного
 * меню и клавиатурной навигации для сообщений, каналов и пользователей (`context-menu.js`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` (§5 п.3–4 Menu / ContextMenu:
 *   `menu_c1e9c4`, `role="menu"`, `role="menuitem"`, `colorDanger`, `separator_c1e9c4`,
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
 * @property {"item" | "slider" | "separator"} [type]
 * @property {number} [min]
 * @property {number} [max]
 * @property {number} [value]
 * @property {number} [step]
 */

/**
 * @typedef {object} BuildContextMenuItemsOptions
 * @property {ContextMenuTargetType} targetType
 * @property {boolean} [isOwnMessage]
 * @property {boolean} [isPinned]
 * @property {boolean} [isMuted]
 * @property {boolean} [canManageMessages]
 * @property {boolean} [canEdit]
 * @property {boolean} [canDelete]
 * @property {boolean} [canMention]
 * @property {number} [volume]
 * @property {boolean} [hasVoice]
 * @property {boolean} [unified]
 * @property {string} [editLabel]
 * @property {string} [deleteLabel]
 * @property {string} [copyIdLabel]
 * @property {string} [messageLabel]
 * @property {string} [mentionLabel]
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
 * @property {boolean} [canEdit]
 * @property {boolean} [canDelete]
 * @property {boolean} [canMention]
 * @property {number} [volume]
 * @property {boolean} [hasVoice]
 * @property {boolean} [unified]
 */

/**
 * @typedef {object} ContextMenuSelectionPayload
 * @property {string} actionId
 * @property {string} id
 * @property {string} label
 * @property {boolean} danger
 * @property {ContextMenuTargetType} targetType
 * @property {string} targetId
 * @property {number} [volume]
 */

/**
 * @typedef {object} ContextMenuControllerOptions
 * @property {any} container
 * @property {(payload: ContextMenuSelectionPayload) => void} [onSelectAction]
 * @property {(volume: number) => void} [onVolumeChange]
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
  const isOwnMessage = opts.isOwnMessage;
  const isPinned = Boolean(opts.isPinned);
  const isMuted = Boolean(opts.isMuted);
  const canManageMessages = Boolean(opts.canManageMessages);

  if (targetType === "message") {
    if (opts.unified) {
      /** @type {ContextMenuItem[]} */
      const items = [
        { id: "reply", label: "Ответить", danger: false },
      ];
      if (isOwnMessage || opts.canEdit || (isOwnMessage === undefined && opts.canEdit === undefined)) {
        items.push({
          id: "edit",
          label: opts.editLabel ?? "Редактировать",
          danger: false,
        });
      }
      items.push({
        id: "copy-text",
        label: "Скопировать текст",
        danger: false,
      });
      items.push({
        id: "copy-id",
        label: opts.copyIdLabel ?? "Скопировать ID",
        danger: false,
      });
      if (isOwnMessage || canManageMessages || opts.canDelete || (isOwnMessage === undefined && opts.canDelete === undefined)) {
        items.push({
          id: "delete",
          label: opts.deleteLabel ?? "Удалить",
          danger: true,
        });
      }
      return items;
    }

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
    if (Boolean(isOwnMessage)) {
      items.push({
        id: "edit",
        label: "Изменить сообщение",
        danger: false,
      });
    }
    if (Boolean(isOwnMessage) || canManageMessages) {
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
    if (opts.unified || opts.canMention !== undefined || opts.volume !== undefined || opts.hasVoice) {
      const vol = typeof opts.volume === "number" && Number.isFinite(opts.volume) ? opts.volume : 100;
      return [
        { id: "profile", label: "Профиль", danger: false },
        { id: "mention-user", label: opts.mentionLabel ?? "Упомянуть", danger: false },
        { id: "message-user", label: opts.messageLabel ?? "Написать в ЛС", danger: false },
        {
          id: "mute-user",
          label: isMuted ? "Включить звук" : "Заглушить",
          danger: false,
        },
        {
          id: "user-volume",
          label: "Громкость",
          danger: false,
          type: "slider",
          min: 0,
          max: 200,
          value: vol,
        },
        { id: "copy-id", label: opts.copyIdLabel ?? "Скопировать ID", danger: false },
      ];
    }

    return [
      { id: "profile", label: "Профиль", danger: false },
      { id: "message-user", label: "Написать сообщение", danger: false },
      { id: "copy-id", label: "Скопировать ID пользователя", danger: false },
    ];
  }

  return [];
}

/**
 * Формирует унифицированное контекстное меню для сообщения (TASK-620).
 *
 * @param {Omit<BuildContextMenuItemsOptions, "targetType">} [options]
 * @returns {ContextMenuItem[]}
 */
export function buildMessageContextMenuItems(options = {}) {
  return buildContextMenuItems({
    targetType: "message",
    unified: true,
    ...options,
  });
}

/**
 * Формирует унифицированное контекстное меню для пользователя (TASK-620).
 *
 * @param {Omit<BuildContextMenuItemsOptions, "targetType">} [options]
 * @returns {ContextMenuItem[]}
 */
export function buildUserContextMenuItems(options = {}) {
  return buildContextMenuItems({
    targetType: "user",
    unified: true,
    ...options,
  });
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
 * Проверяет, содержится ли дочерний узел внутри родительского элемента (DOM / FakeElement).
 *
 * @param {any} parent
 * @param {any} child
 * @returns {boolean}
 */
function containsElement(parent, child) {
  if (!parent || !child) return false;
  if (typeof parent.contains === "function") {
    return Boolean(parent.contains(child));
  }
  let cur = child;
  while (cur) {
    if (cur === parent) return true;
    cur = cur.parentNode;
  }
  return false;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any, addEventListener?: any, removeEventListener?: any, body?: any }}
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
 * Создаёт контроллер контекстного меню с поддержкой клавиатурной навигации,
 * слайдера громкости и закрытия по клику вне меню.
 *
 * @param {ContextMenuControllerOptions} options
 */
export function createContextMenuController(options) {
  const opts = options && typeof options === "object" ? options : /** @type {ContextMenuControllerOptions} */ ({});
  const container = opts.container;
  const doc = resolveDocument(container);
  const onSelectAction =
    typeof opts.onSelectAction === "function" ? opts.onSelectAction : null;
  const onVolumeChange =
    typeof opts.onVolumeChange === "function" ? opts.onVolumeChange : null;

  /** @type {ContextMenuItem[]} */
  let currentItems = [];
  /** @type {any[]} */
  let itemElements = [];
  /** @type {ContextMenuTargetType | null} */
  let currentTargetType = null;
  let currentTargetId = "";
  let focusedIndex = -1;
  let openState = false;
  let activeVolume = 100;
  let outsideListenerAttached = false;

  const menuEl = doc.createElement("div");
  menuEl.setAttribute("data-context-menu", "");
  menuEl.setAttribute("role", "menu");
  menuEl.setAttribute("aria-label", "Контекстное меню");
  menuEl.setAttribute("tabindex", "-1");
  menuEl.setAttribute("data-open", "false");
  menuEl.setAttribute("aria-hidden", "true");
  menuEl.setAttribute("hidden", "");

  /**
   * Закрывает меню при клике вне его пределов.
   *
   * @param {any} ev
   */
  function handleOutsideClick(ev) {
    if (!openState) return;
    const target = ev?.target;
    if (target && containsElement(menuEl, target)) {
      return;
    }
    close();
  }

  function attachOutsideListener() {
    if (outsideListenerAttached) return;
    const eventRoot = doc && typeof doc.addEventListener === "function"
      ? doc
      : doc?.body && typeof doc.body.addEventListener === "function"
        ? doc.body
        : container;
    if (eventRoot && typeof eventRoot.addEventListener === "function") {
      eventRoot.addEventListener("click", handleOutsideClick);
      outsideListenerAttached = true;
    }
  }

  function detachOutsideListener() {
    if (!outsideListenerAttached) return;
    const eventRoot = doc && typeof doc.removeEventListener === "function"
      ? doc
      : doc?.body && typeof doc.body.removeEventListener === "function"
        ? doc.body
        : container;
    if (eventRoot && typeof eventRoot.removeEventListener === "function") {
      eventRoot.removeEventListener("click", handleOutsideClick);
    }
    outsideListenerAttached = false;
  }

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
    detachOutsideListener();
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
      volume: item.id === "user-volume" ? activeVolume : undefined,
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
   * Программно устанавливает уровень громкости пользователя (0..200%).
   *
   * @param {number} volume
   * @returns {number}
   */
  function setVolume(volume) {
    const num = typeof volume === "number" && Number.isFinite(volume) ? volume : 100;
    activeVolume = Math.max(0, Math.min(200, Math.round(num)));
    const sliderInput = menuEl.querySelector("[data-volume-slider]");
    if (sliderInput) {
      /** @type {any} */ (sliderInput).value = String(activeVolume);
      sliderInput.setAttribute("aria-valuenow", String(activeVolume));
    }
    const valDisplay = menuEl.querySelector("[data-volume-value]");
    if (valDisplay) {
      valDisplay.textContent = `${activeVolume}%`;
    }
    if (onVolumeChange) {
      onVolumeChange(activeVolume);
    }
    return activeVolume;
  }

  /**
   * Возвращает текущий уровень громкости пользователя.
   *
   * @returns {number}
   */
  function getVolume() {
    return activeVolume;
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
      canEdit: cfg.canEdit,
      canDelete: cfg.canDelete,
      canMention: cfg.canMention,
      volume: cfg.volume,
      hasVoice: cfg.hasVoice,
      unified: cfg.unified,
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
    activeVolume = typeof cfg.volume === "number" && Number.isFinite(cfg.volume)
      ? Math.max(0, Math.min(200, cfg.volume))
      : 100;
    openState = true;

    itemElements = currentItems.map((item, idx) => {
      if (item.type === "slider" || item.id === "user-volume") {
        const wrap = doc.createElement("div");
        wrap.setAttribute("data-context-item", item.id);
        wrap.setAttribute("data-action-id", item.id);
        wrap.setAttribute("role", "presentation");
        wrap.className = "context-menu-volume-wrap";

        const header = doc.createElement("div");
        header.className = "context-menu-volume-header";

        const labelSpan = doc.createElement("span");
        labelSpan.className = "context-menu-volume-label";
        labelSpan.textContent = item.label;

        const valSpan = doc.createElement("span");
        valSpan.className = "context-menu-volume-value";
        valSpan.setAttribute("data-volume-value", "");
        valSpan.textContent = `${activeVolume}%`;

        header.appendChild(labelSpan);
        header.appendChild(valSpan);

        const slider = doc.createElement("input");
        slider.type = "range";
        slider.setAttribute("type", "range");
        slider.setAttribute("min", String(item.min ?? 0));
        slider.setAttribute("max", String(item.max ?? 200));
        slider.setAttribute("value", String(activeVolume));
        slider.value = String(activeVolume);
        slider.setAttribute("role", "slider");
        slider.setAttribute("aria-label", item.label);
        slider.setAttribute("aria-valuemin", String(item.min ?? 0));
        slider.setAttribute("aria-valuemax", String(item.max ?? 200));
        slider.setAttribute("aria-valuenow", String(activeVolume));
        slider.setAttribute("data-volume-slider", "");
        slider.className = "context-menu-slider";

        const handleVolumeUpdate = (/** @type {any} */ event) => {
          if (event && typeof event.stopPropagation === "function") {
            event.stopPropagation();
          }
          const num = Number(slider.value);
          const clamped = Math.max(0, Math.min(200, Number.isFinite(num) ? num : 100));
          activeVolume = clamped;
          valSpan.textContent = `${clamped}%`;
          slider.setAttribute("aria-valuenow", String(clamped));
          if (onVolumeChange) {
            onVolumeChange(clamped);
          }
        };

        slider.addEventListener("input", handleVolumeUpdate);
        slider.addEventListener("change", handleVolumeUpdate);

        wrap.appendChild(header);
        wrap.appendChild(slider);
        return wrap;
      }

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
    attachOutsideListener();
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
    setVolume,
    getVolume,
    isOpen: () => openState,
    getFocusedIndex: () => focusedIndex,
  };
}

/**
 * @callback ContextMenuTriggerResolver
 * @param {any} event
 * @returns {OpenContextMenuOptions}
 */

/**
 * Привязывает обработчик контекстного меню (ПКМ) к элементу.
 *
 * @param {any} element
 * @param {ReturnType<typeof createContextMenuController>} controller
 * @param {ContextMenuTriggerResolver | OpenContextMenuOptions} resolverOrOptions
 * @returns {() => void} Функция отписки (unbind)
 */
export function bindContextMenuTrigger(element, controller, resolverOrOptions) {
  if (!element || typeof element.addEventListener !== "function") {
    throw new TypeError("Element must be a valid DOM object with addEventListener");
  }
  if (!controller || typeof controller.open !== "function") {
    throw new TypeError("Controller must provide an open method");
  }

  const handler = (/** @type {any} */ event) => {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    if (event && typeof event.stopPropagation === "function") {
      event.stopPropagation();
    }
    const resolved =
      typeof resolverOrOptions === "function"
        ? resolverOrOptions(event)
        : resolverOrOptions;
    const clientX =
      event && typeof event.clientX === "number" && Number.isFinite(event.clientX)
        ? event.clientX
        : 0;
    const clientY =
      event && typeof event.clientY === "number" && Number.isFinite(event.clientY)
        ? event.clientY
        : 0;

    controller.open({
      x: clientX,
      y: clientY,
      ...resolved,
    });
  };

  element.addEventListener("contextmenu", handler);

  return function unbind() {
    if (typeof element.removeEventListener === "function") {
      element.removeEventListener("contextmenu", handler);
    }
  };
}
