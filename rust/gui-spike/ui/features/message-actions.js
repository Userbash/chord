// @ts-check
/**
 * T-210 (RW-40 / G2): контроллер панели действий сообщения и модального окна
 * пересылки сообщения (`message-actions.js`).
 *
 * Оракулы контрактов:
 * - RE-дамп `/home/sanya/re-deep/ui/01-msg-menu-items.json` (`Переслать`, быстрые реакции),
 * - `/home/sanya/re-deep/frontend-d03.md` (§5 Modal / Toolbar / Menu).
 *
 * Отвечает за:
 * - `QUICK_REACTION_EMOJIS`: замороженный набор быстрых реакций `['👍', '❤️', '😂']`;
 * - `MAX_FORWARD_DESTINATIONS`: максимальное число каналов-получателей при пересылке (`5`);
 * - `renderActionBar(message, context)`:
 *   - рендер кнопок быстрых реакций из `QUICK_REACTION_EMOJIS` (`onReact(message.id, emoji)`),
 *   - кнопки «Ответить» (`onReply(message.id)`) и кнопки «Переслать»,
 *   - кнопки «Изменить» ТОЛЬКО при `message.authorId === context.currentUserId` (`onEdit(message.id)`),
 *   - кнопки «Закрепить сообщение» (или «Открепить сообщение» при `message.pinned === true`)
 *     ТОЛЬКО при `context.canManageMessages === true` (`onTogglePin(message.id, !Boolean(message.pinned))`),
 *   - кнопки «Удалить сообщение» при `message.authorId === context.currentUserId || context.canManageMessages === true`
 *     (`onDelete(message.id)`);
 * - `openForwardModal(message, channels)`:
 *   - рендер диалога «Переслать сообщение» (`role="dialog"`, `aria-modal="true"`) с полем фильтра
 *     каналов, чекбоксами назначения (с потолком `MAX_FORWARD_DESTINATIONS = 5`), полем
 *     необязательного комментария и кнопкой «Отправить» (`onForward(message.id, selectedChannelIds, comment)`).
 *
 * @module ui/features/message-actions
 */

/**
 * Быстрые эмодзи-реакции на панели действий сообщения.
 * @type {ReadonlyArray<string>}
 */
export const QUICK_REACTION_EMOJIS = Object.freeze(["👍", "❤️", "😂"]);

/**
 * Максимальное число каналов, которые можно выбрать для пересылки одного сообщения.
 * @type {5}
 */
export const MAX_FORWARD_DESTINATIONS = 5;

/**
 * @typedef {object} ActionableMessage
 * @property {string} id
 * @property {string} [authorId]
 * @property {string} [content]
 * @property {boolean} [pinned]
 */

/**
 * @typedef {object} MessageActionContext
 * @property {string} [currentUserId]
 * @property {boolean} [canManageMessages]
 */

/**
 * @typedef {object} ForwardableChannel
 * @property {string} id
 * @property {string} [name]
 * @property {string} [label]
 */

/**
 * @typedef {object} MessageActionsControllerOptions
 * @property {(messageId: string, emoji: string) => void} [onReact]
 * @property {(messageId: string) => void} [onReply]
 * @property {(messageId: string) => void} [onEdit]
 * @property {(messageId: string, nextPinned: boolean) => void} [onTogglePin]
 * @property {(messageId: string, selectedChannelIds: string[], comment: string) => void} [onForward]
 * @property {(messageId: string) => void} [onDelete]
 */

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
  if (
    containerEl &&
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("containerEl must provide ownerDocument.createElement");
}

/**
 * Удаляет элемент из родительского контейнера безопасно для браузера и DOM-шимов.
 *
 * @param {any} parentEl
 * @param {any} childEl
 */
function detachNode(parentEl, childEl) {
  if (!childEl) return;
  if (typeof childEl.remove === "function") {
    childEl.remove();
    return;
  }
  if (parentEl && typeof parentEl.removeChild === "function") {
    try {
      parentEl.removeChild(childEl);
    } catch {
      // Узел уже мог быть отсоединен внешним кодом
    }
  }
}

/**
 * Заменяет дочерние узлы контейнера списком новых узлов.
 *
 * @param {any} targetEl
 * @param {any[]} nodes
 */
function replaceElementChildren(targetEl, nodes) {
  if (typeof targetEl.replaceChildren === "function") {
    targetEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(targetEl.children)) {
    targetEl.children.length = 0;
  }
  for (const node of nodes) {
    targetEl.appendChild(node);
  }
}

/**
 * Синхронизирует атрибут и свойство `disabled` + `aria-disabled` у интерактивного элемента.
 *
 * @param {any} el
 * @param {boolean} isDisabled
 */
function setElementDisabled(el, isDisabled) {
  el.disabled = isDisabled;
  if (isDisabled) {
    el.setAttribute("disabled", "");
    el.setAttribute("aria-disabled", "true");
  } else {
    el.removeAttribute("disabled");
    el.setAttribute("aria-disabled", "false");
  }
}

/**
 * Синхронизирует атрибут и свойство `checked` у чекбокса.
 *
 * @param {any} checkboxEl
 * @param {boolean} isChecked
 */
function setCheckboxChecked(checkboxEl, isChecked) {
  checkboxEl.checked = isChecked;
  if (isChecked) {
    checkboxEl.setAttribute("checked", "");
  } else {
    checkboxEl.removeAttribute("checked");
  }
}

/**
 * Нормализует входной список каналов для пересылки.
 *
 * @param {ReadonlyArray<ForwardableChannel | string> | null | undefined} rawChannels
 * @returns {Array<{ id: string, name: string }>}
 */
function normalizeChannels(rawChannels) {
  /** @type {Array<{ id: string, name: string }>} */
  const normalized = [];
  if (!Array.isArray(rawChannels)) return normalized;

  /** @type {Set<string>} */
  const seen = new Set();

  for (let i = 0; i < rawChannels.length; i += 1) {
    const item = rawChannels[i];
    if (item === null || item === undefined) continue;
    if (typeof item === "string") {
      if (item.length === 0 || seen.has(item)) continue;
      seen.add(item);
      normalized.push({ id: item, name: item });
    } else if (typeof item === "object") {
      const id = String(item.id ?? "").trim();
      if (id.length === 0 || seen.has(id)) continue;
      seen.add(id);
      const name = String(item.name ?? item.label ?? id);
      normalized.push({ id, name });
    }
  }

  return normalized;
}

/**
 * Создаёт контроллер панели действий сообщения и модального окна пересылки.
 *
 * @param {any} containerEl
 * @param {MessageActionsControllerOptions} [options]
 */
export function createMessageActionsController(containerEl, options = {}) {
  if (!containerEl || typeof containerEl.appendChild !== "function") {
    throw new TypeError("containerEl must be a valid DOM container element");
  }
  const doc = resolveDocument(containerEl);

  /** @type {any | null} */
  let activeActionBar = null;
  /** @type {any | null} */
  let activeForwardModal = null;

  /**
   * Создаёт кнопку панели действий с указанными подписью и `data-action`.
   *
   * @param {string} label
   * @param {string} actionName
   * @param {() => void} onClick
   * @returns {any}
   */
  function createActionButton(label, actionName, onClick) {
    const btn = doc.createElement("button");
    btn.setAttribute("type", "button");
    btn.setAttribute("data-action", actionName);
    btn.setAttribute("aria-label", label);
    btn.textContent = label;
    btn.addEventListener("click", () => {
      onClick();
    });
    return btn;
  }

  /**
   * Рендерит панель действий для сообщения с учётом прав текущего пользователя.
   *
   * @param {ActionableMessage} message
   * @param {MessageActionContext} [context]
   * @returns {any}
   */
  function renderActionBar(message, context = {}) {
    detachNode(containerEl, activeActionBar);

    const msgObj = message && typeof message === "object" ? message : { id: "" };
    const ctxObj = context && typeof context === "object" ? context : {};

    const messageId = String(msgObj.id ?? "");
    const authorId =
      typeof msgObj.authorId === "string" ? msgObj.authorId : "";
    const currentUserId =
      typeof ctxObj.currentUserId === "string" ? ctxObj.currentUserId : "";
    const isOwnMessage =
      authorId.length > 0 &&
      currentUserId.length > 0 &&
      authorId === currentUserId;
    const canManageMessages = ctxObj.canManageMessages === true;
    const isPinned = msgObj.pinned === true;

    const bar = doc.createElement("div");
    bar.setAttribute("role", "toolbar");
    bar.setAttribute("aria-label", "Действия с сообщением");
    bar.setAttribute("data-message-action-bar", "");
    bar.setAttribute("data-message-id", messageId);

    // 1. Быстрые реакции из QUICK_REACTION_EMOJIS
    for (const emoji of QUICK_REACTION_EMOJIS) {
      const reactBtn = doc.createElement("button");
      reactBtn.setAttribute("type", "button");
      reactBtn.setAttribute("data-action", "quick-react");
      reactBtn.setAttribute("data-emoji", emoji);
      reactBtn.setAttribute("data-reaction-emoji", emoji);
      reactBtn.setAttribute("aria-label", `Реакция ${emoji}`);
      reactBtn.textContent = emoji;
      reactBtn.addEventListener("click", () => {
        if (typeof options.onReact === "function") {
          options.onReact(messageId, emoji);
        }
      });
      bar.appendChild(reactBtn);
    }

    // 2. Кнопка «Ответить»
    bar.appendChild(
      createActionButton("Ответить", "reply", () => {
        if (typeof options.onReply === "function") {
          options.onReply(messageId);
        }
      }),
    );

    // 3. Кнопка «Переслать»
    bar.appendChild(
      createActionButton("Переслать", "forward", () => {
        // Кнопка всегда рендерится; при необходимости открывает модалку пересылки
      }),
    );

    // 4. Кнопка «Изменить» — ТОЛЬКО для автора сообщения
    if (isOwnMessage) {
      bar.appendChild(
        createActionButton("Изменить", "edit", () => {
          if (typeof options.onEdit === "function") {
            options.onEdit(messageId);
          }
        }),
      );
    }

    // 5. Кнопка «Закрепить сообщение» / «Открепить сообщение» — ТОЛЬКО при canManageMessages === true
    if (canManageMessages) {
      const pinLabel = isPinned
        ? "Открепить сообщение"
        : "Закрепить сообщение";
      const pinBtn = createActionButton(pinLabel, "toggle-pin", () => {
        if (typeof options.onTogglePin === "function") {
          options.onTogglePin(messageId, !isPinned);
        }
      });
      pinBtn.setAttribute("data-pinned", isPinned ? "true" : "false");
      bar.appendChild(pinBtn);
    }

    // 6. Кнопка «Удалить сообщение» — для автора ИЛИ при canManageMessages === true
    if (isOwnMessage || canManageMessages) {
      bar.appendChild(
        createActionButton("Удалить сообщение", "delete", () => {
          if (typeof options.onDelete === "function") {
            options.onDelete(messageId);
          }
        }),
      );
    }

    containerEl.appendChild(bar);
    activeActionBar = bar;
    return bar;
  }

  /**
   * Закрывает открытое модальное окно пересылки сообщения.
   */
  function closeForwardModal() {
    detachNode(containerEl, activeForwardModal);
    activeForwardModal = null;
  }

  /**
   * Открывает модальное окно «Переслать сообщение» с фильтром каналов,
   * чекбоксами назначения (макс. `MAX_FORWARD_DESTINATIONS`), полем комментария
   * и кнопкой «Отправить».
   *
   * @param {ActionableMessage} message
   * @param {ReadonlyArray<ForwardableChannel | string>} channels
   * @returns {any}
   */
  function openForwardModal(message, channels) {
    closeForwardModal();

    const msgObj = message && typeof message === "object" ? message : { id: "" };
    const messageId = String(msgObj.id ?? "");
    const allChannels = normalizeChannels(channels);

    /** @type {Set<string>} */
    const selectedChannelIds = new Set();
    /** @type {Map<string, any>} */
    const renderedCheckboxes = new Map();
    let filterQuery = "";

    const modal = doc.createElement("div");
    modal.setAttribute("role", "dialog");
    modal.setAttribute("aria-modal", "true");
    modal.setAttribute("aria-label", "Переслать сообщение");
    modal.setAttribute("data-forward-modal", "");
    modal.setAttribute("data-message-id", messageId);

    const titleEl = doc.createElement("h2");
    titleEl.setAttribute("data-forward-title", "");
    titleEl.textContent = "Переслать сообщение";
    modal.appendChild(titleEl);

    const filterInput = doc.createElement("input");
    filterInput.setAttribute("type", "search");
    filterInput.setAttribute("data-forward-filter", "");
    filterInput.setAttribute("placeholder", "Поиск каналов...");
    filterInput.setAttribute("aria-label", "Поиск каналов");
    filterInput.placeholder = "Поиск каналов...";
    filterInput.value = "";
    modal.appendChild(filterInput);

    const channelListEl = doc.createElement("div");
    channelListEl.setAttribute("data-forward-channel-list", "");
    channelListEl.setAttribute("role", "group");
    channelListEl.setAttribute("aria-label", "Каналы для пересылки");
    modal.appendChild(channelListEl);

    const commentInput = doc.createElement("textarea");
    commentInput.setAttribute("data-forward-comment", "");
    commentInput.setAttribute("placeholder", "Добавить комментарий (необязательно)");
    commentInput.setAttribute("aria-label", "Комментарий к пересылке");
    commentInput.placeholder = "Добавить комментарий (необязательно)";
    commentInput.value = "";
    modal.appendChild(commentInput);

    const submitBtn = doc.createElement("button");
    submitBtn.setAttribute("type", "button");
    submitBtn.setAttribute("data-action", "forward-submit");
    submitBtn.setAttribute("data-forward-submit", "");
    submitBtn.textContent = "Отправить";
    modal.appendChild(submitBtn);

    /**
     * Обновляет состояния `checked` и `disabled` всех отрендеренных чекбоксов
     * и кнопки «Отправить» без пересоздания DOM-узлов.
     */
    function syncSelectionState() {
      const capReached = selectedChannelIds.size >= MAX_FORWARD_DESTINATIONS;
      for (const [chId, checkbox] of renderedCheckboxes.entries()) {
        const isSelected = selectedChannelIds.has(chId);
        setCheckboxChecked(checkbox, isSelected);
        setElementDisabled(checkbox, !isSelected && capReached);
      }
      setElementDisabled(submitBtn, selectedChannelIds.size === 0);
    }

    /**
     * Применяет попытку переключения выбора канала с учётом лимита `MAX_FORWARD_DESTINATIONS`.
     *
     * @param {string} channelId
     * @param {boolean} wantSelected
     */
    function applyChannelSelection(channelId, wantSelected) {
      if (wantSelected) {
        if (
          !selectedChannelIds.has(channelId) &&
          selectedChannelIds.size >= MAX_FORWARD_DESTINATIONS
        ) {
          syncSelectionState();
          return;
        }
        selectedChannelIds.add(channelId);
      } else {
        selectedChannelIds.delete(channelId);
      }
      syncSelectionState();
    }

    /**
     * Перерисовывает список чекбоксов каналов с учётом текущего фильтра.
     */
    function renderChannelCheckboxes() {
      renderedCheckboxes.clear();
      const needle = filterQuery.trim().toLowerCase();
      const visibleChannels =
        needle.length === 0
          ? allChannels
          : allChannels.filter(
              (ch) =>
                ch.name.toLowerCase().includes(needle) ||
                ch.id.toLowerCase().includes(needle),
            );

      /** @type {any[]} */
      const itemNodes = [];

      for (const ch of visibleChannels) {
        const labelEl = doc.createElement("label");
        labelEl.setAttribute("data-forward-channel-item", ch.id);

        const checkbox = doc.createElement("input");
        checkbox.setAttribute("type", "checkbox");
        checkbox.setAttribute("data-forward-channel-checkbox", "");
        checkbox.setAttribute("data-channel-id", ch.id);
        checkbox.value = ch.id;

        checkbox.addEventListener("click", () => {
          const wantSelected = !selectedChannelIds.has(ch.id);
          applyChannelSelection(ch.id, wantSelected);
        });

        checkbox.addEventListener("change", (/** @type {any} */ event) => {
          const explicitTargetChecked = event?.target?.checked;
          const wantSelected =
            typeof explicitTargetChecked === "boolean"
              ? explicitTargetChecked
              : !selectedChannelIds.has(ch.id);
          applyChannelSelection(ch.id, wantSelected);
        });

        const nameSpan = doc.createElement("span");
        nameSpan.setAttribute("data-forward-channel-name", "");
        nameSpan.textContent = ch.name;

        labelEl.appendChild(checkbox);
        labelEl.appendChild(nameSpan);

        renderedCheckboxes.set(ch.id, checkbox);
        itemNodes.push(labelEl);
      }

      replaceElementChildren(channelListEl, itemNodes);
      syncSelectionState();
    }

    filterInput.addEventListener("input", (/** @type {any} */ event) => {
      filterQuery = String(event?.target?.value ?? filterInput.value ?? "");
      renderChannelCheckboxes();
    });

    filterInput.addEventListener("change", (/** @type {any} */ event) => {
      filterQuery = String(event?.target?.value ?? filterInput.value ?? "");
      renderChannelCheckboxes();
    });

    submitBtn.addEventListener("click", () => {
      if (selectedChannelIds.size === 0) return;
      const destinations = Array.from(selectedChannelIds).slice(
        0,
        MAX_FORWARD_DESTINATIONS,
      );
      const comment = String(commentInput.value ?? "");
      if (typeof options.onForward === "function") {
        options.onForward(messageId, destinations, comment);
      }
    });

    renderChannelCheckboxes();
    containerEl.appendChild(modal);
    activeForwardModal = modal;
    return modal;
  }

  return {
    renderActionBar,
    openForwardModal,
    closeForwardModal,
  };
}
