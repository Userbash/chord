// @ts-check
/**
 * T-212 (RW-40 / G2): UI-контроллер панели пилюль реакций и модального окна
 * списка отреагировавших пользователей (`reactions-ui.js`).
 *
 * Контрактные оракулы:
 * - `waves/RW-40/DESIGN/REACTIONS-EXTENDED-SPEC.md`
 * - `waves/RW-40/PM/BACKLOG-RE-FRONTEND.md` (FE-P0-03, FE-P0-04)
 * - `rust/discord-client/src/http/reactions.rs` (`type=0` NORMAL, `type=1` BURST)
 *
 * Отвечает за:
 * - `REACTION_TYPES = Object.freeze({ NORMAL: 0, BURST: 1 })`;
 * - `createReactionsUiController(containerEl, options)`:
 *   - `renderReactionBar(reactions)`: рендер кнопок-пилюль с `data-emoji`,
 *     `data-reaction-burst` (`'true'` при `burstCount > 0`, иначе `'false'`),
 *     `aria-pressed` (`'true'` при `me || meBurst`, иначе `'false'`),
 *     суммарным счётчиком `count + (burstCount || 0)` и вызовом
 *     `onToggleReaction(emoji, meBurst ? REACTION_TYPES.BURST : REACTION_TYPES.NORMAL, !(me || meBurst))`
 *     по клику;
 *   - `openReactorsModal(messageId, reactionsWithUsers)`: рендер русского
 *     диалога (`'Реакции'`) с вкладками фильтра по эмодзи (показывают эмодзи
 *     и счётчик) и списком пользователей (`{ id, displayName, username, reactionType }`)
 *     с бейджем `'Суперреакция'` при `reactionType === 1`, либо пустым
 *     состоянием `'Нет реакций'`.
 *
 * @module ui/features/reactions-ui
 */

/**
 * Коды типов реакций Discord REST v9 / Gateway (`0` — обычная, `1` — суперреакция/burst).
 * @type {Readonly<{ NORMAL: 0, BURST: 1 }>}
 */
export const REACTION_TYPES = Object.freeze({
  NORMAL: 0,
  BURST: 1,
});

/**
 * Русский заголовок диалога списка реакций.
 */
export const REACTORS_DIALOG_TITLE_RU = "Реакции";

/**
 * Русская подпись бейджа суперреакции (`reactionType === 1`).
 */
export const BURST_BADGE_LABEL_RU = "Суперреакция";

/**
 * Русский текст пустого состояния модального окна реакций.
 */
export const REACTIONS_EMPTY_STATE_RU = "Нет реакций";

/**
 * @typedef {object} ReactionBarItem
 * @property {string | { name?: string, id?: string }} emoji
 * @property {number} count
 * @property {number} [burstCount]
 * @property {boolean} [me]
 * @property {boolean} [meBurst]
 * @property {ReadonlyArray<string>} [burstColors]
 */

/**
 * @typedef {object} ReactorUserItem
 * @property {string} id
 * @property {string} [displayName]
 * @property {string} [globalName]
 * @property {string} [username]
 * @property {number} [reactionType]
 * @property {number} [type]
 */

/**
 * @typedef {object} ReactionWithUsersGroup
 * @property {string | { name?: string, id?: string }} emoji
 * @property {number} [count]
 * @property {number} [burstCount]
 * @property {ReadonlyArray<ReactorUserItem>} [users]
 * @property {ReadonlyArray<ReactorUserItem>} [reactors]
 */

/**
 * @typedef {object} NormalizedReactionGroup
 * @property {string} emoji
 * @property {number} count
 * @property {Array<{ id: string, displayName: string, username: string, reactionType: number }>} users
 */

/**
 * @typedef {object} ReactionsUiControllerOptions
 * @property {(emoji: string, reactionType: 0 | 1, nextActive: boolean) => void} [onToggleReaction]
 * @property {(messageIdOrEmoji: string, reactionsWithUsers?: any) => void} [onOpenReactors]
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
 * Заменяет дочерние узлы элемента без использования `innerHTML`.
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
    for (const child of targetEl.children) {
      if (child && typeof child === "object") {
        child.parentNode = null;
      }
    }
    targetEl.children.length = 0;
  }
  for (const node of nodes) {
    targetEl.appendChild(node);
  }
}

/**
 * Удаляет DOM-узел из родительского контейнера.
 *
 * @param {any} parentEl
 * @param {any} childEl
 */
function removeElementFromParent(parentEl, childEl) {
  if (!childEl) return;
  if (typeof childEl.remove === "function") {
    childEl.remove();
    return;
  }
  if (parentEl && typeof parentEl.removeChild === "function") {
    try {
      parentEl.removeChild(childEl);
    } catch {
      // Элемент уже мог быть удалён внешним кодом
    }
  }
}

/**
 * Нормализует строковое представление эмодзи.
 *
 * @param {unknown} rawEmoji
 * @returns {string}
 */
function normalizeEmojiKey(rawEmoji) {
  if (typeof rawEmoji === "string") {
    return rawEmoji;
  }
  if (rawEmoji && typeof rawEmoji === "object") {
    const obj = /** @type {{ name?: unknown, id?: unknown }} */ (rawEmoji);
    if (typeof obj.name === "string" && obj.name.length > 0) {
      return obj.name;
    }
    if (typeof obj.id === "string" && obj.id.length > 0) {
      return obj.id;
    }
  }
  return "";
}

/**
 * Нормализует неотрицательное целое число для счётчика реакций.
 *
 * @param {unknown} value
 * @returns {number}
 */
function normalizeNonNegativeInt(value) {
  const num = Number(value);
  if (!Number.isFinite(num) || num <= 0) {
    return 0;
  }
  return Math.trunc(num);
}

/**
 * Нормализует входные данные групп реакций с пользователями для модального окна.
 *
 * @param {unknown} rawReactionsWithUsers
 * @returns {NormalizedReactionGroup[]}
 */
function normalizeReactionsWithUsers(rawReactionsWithUsers) {
  if (!Array.isArray(rawReactionsWithUsers) || rawReactionsWithUsers.length === 0) {
    return [];
  }

  /** @type {NormalizedReactionGroup[]} */
  const groups = [];

  for (let i = 0; i < rawReactionsWithUsers.length; i += 1) {
    const entry = rawReactionsWithUsers[i];
    if (!entry || typeof entry !== "object") continue;

    const rawUsers = Array.isArray(entry.users)
      ? entry.users
      : Array.isArray(entry.reactors)
        ? entry.reactors
        : null;

    if (rawUsers !== null || "emoji" in entry) {
      const emoji = normalizeEmojiKey(entry.emoji) || `emoji-${i}`;
      /** @type {Array<{ id: string, displayName: string, username: string, reactionType: number }>} */
      const users = [];
      if (Array.isArray(rawUsers)) {
        for (let j = 0; j < rawUsers.length; j += 1) {
          const u = rawUsers[j];
          if (!u || typeof u !== "object") continue;
          const id = String(u.id ?? u.userId ?? `user-${j}`);
          const username = String(u.username ?? id);
          const displayName = String(
            u.displayName ?? u.globalName ?? u.global_name ?? username,
          );
          const rawType = u.reactionType ?? u.type ?? REACTION_TYPES.NORMAL;
          const reactionType =
            Number(rawType) === REACTION_TYPES.BURST
              ? REACTION_TYPES.BURST
              : REACTION_TYPES.NORMAL;
          users.push({ id, displayName, username, reactionType });
        }
      }

      const explicitCount =
        typeof entry.count === "number"
          ? normalizeNonNegativeInt(entry.count) +
            normalizeNonNegativeInt(entry.burstCount)
          : users.length;

      groups.push({
        emoji,
        count: explicitCount,
        users,
      });
    }
  }

  return groups;
}

/**
 * Создаёт контроллер панели реакций под сообщением и модального окна
 * списка пользователей, поставивших реакцию.
 *
 * @param {any} containerEl
 * @param {ReactionsUiControllerOptions} [options]
 */
export function createReactionsUiController(containerEl, options = {}) {
  if (!containerEl || typeof containerEl.appendChild !== "function") {
    throw new TypeError("containerEl must be a valid DOM container element");
  }
  const doc = resolveDocument(containerEl);

  /** @type {any | null} */
  let reactionBarEl = null;
  /** @type {any | null} */
  let modalEl = null;
  /** @type {NormalizedReactionGroup[]} */
  let currentGroups = [];
  /** @type {string | null} */
  let activeEmoji = null;
  /** @type {Map<string, any>} */
  let tabButtonMap = new Map();
  /** @type {any | null} */
  let usersListContainerEl = null;

  /**
   * Рендерит панель пилюль реакций в `containerEl`.
   *
   * @param {ReadonlyArray<ReactionBarItem>} reactions
   * @returns {any}
   */
  function renderReactionBar(reactions) {
    const items = Array.isArray(reactions) ? reactions : [];

    if (!reactionBarEl) {
      reactionBarEl = doc.createElement("div");
      reactionBarEl.setAttribute("class", "reactions-row");
      reactionBarEl.setAttribute("role", "group");
      reactionBarEl.setAttribute("aria-label", REACTORS_DIALOG_TITLE_RU);
      reactionBarEl.setAttribute("data-reaction-bar", "");
      containerEl.appendChild(reactionBarEl);
    }

    /** @type {any[]} */
    const pillButtons = [];

    for (let i = 0; i < items.length; i += 1) {
      const item = items[i];
      if (!item || typeof item !== "object") continue;

      const emoji = normalizeEmojiKey(item.emoji);
      if (!emoji) continue;

      const normalCount = normalizeNonNegativeInt(item.count);
      const burstCount = normalizeNonNegativeInt(item.burstCount);
      const totalCount = normalCount + burstCount;
      const isBurst = burstCount > 0;
      const me = Boolean(item.me);
      const meBurst = Boolean(item.meBurst);
      const isPressed = me || meBurst;

      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute(
        "class",
        isPressed ? "reaction active" : "reaction",
      );
      btn.setAttribute("data-emoji", emoji);
      btn.setAttribute("data-reaction-burst", isBurst ? "true" : "false");
      btn.setAttribute("aria-pressed", isPressed ? "true" : "false");
      btn.setAttribute("data-me", me ? "true" : "false");
      btn.setAttribute("data-me-burst", meBurst ? "true" : "false");
      btn.setAttribute("data-count", String(normalCount));
      btn.setAttribute("data-burst-count", String(burstCount));
      btn.setAttribute("data-total-count", String(totalCount));

      if (Array.isArray(item.burstColors) && item.burstColors.length > 0) {
        const cleanColors = item.burstColors
          .map((/** @type {unknown} */ c) => String(c ?? "").trim())
          .filter(Boolean);
        if (cleanColors.length > 0) {
          btn.setAttribute("data-burst-colors", cleanColors.join(","));
          const primaryColor = cleanColors[0];
          if (primaryColor && btn.style && typeof btn.style.setProperty === "function") {
            btn.style.setProperty("--reaction-burst-color", primaryColor);
          }
        }
      }

      const emojiSpan = doc.createElement("span");
      emojiSpan.setAttribute("class", "reaction-emoji");
      emojiSpan.setAttribute("data-reaction-emoji", "");
      emojiSpan.textContent = emoji;

      const countSpan = doc.createElement("span");
      countSpan.setAttribute("class", "reaction-count");
      countSpan.setAttribute("data-reaction-count", "");
      countSpan.textContent = String(totalCount);

      btn.appendChild(emojiSpan);
      btn.appendChild(countSpan);

      btn.addEventListener("click", () => {
        if (typeof options.onToggleReaction === "function") {
          const reactionType = meBurst
            ? REACTION_TYPES.BURST
            : REACTION_TYPES.NORMAL;
          const nextActive = !(me || meBurst);
          options.onToggleReaction(emoji, reactionType, nextActive);
        }
      });

      btn.addEventListener("contextmenu", (/** @type {any} */ event) => {
        if (event && typeof event.preventDefault === "function") {
          event.preventDefault();
        }
        if (typeof options.onOpenReactors === "function") {
          options.onOpenReactors(emoji);
        }
      });

      pillButtons.push(btn);
    }

    replaceElementChildren(reactionBarEl, pillButtons);
    return reactionBarEl;
  }

  /**
   * Перерисовывает список пользователей выбранной вкладки эмодзи в модальном окне.
   */
  function renderReactorsUsersList() {
    if (!usersListContainerEl) return;

    const selectedGroup =
      currentGroups.find((g) => g.emoji === activeEmoji) ??
      currentGroups[0] ??
      null;

    if (!selectedGroup || selectedGroup.users.length === 0) {
      const emptyEl = doc.createElement("div");
      emptyEl.setAttribute("class", "reactors-empty");
      emptyEl.setAttribute("data-reactors-empty", "");
      emptyEl.textContent = REACTIONS_EMPTY_STATE_RU;
      replaceElementChildren(usersListContainerEl, [emptyEl]);
      return;
    }

    /** @type {any[]} */
    const userNodes = [];
    for (const user of selectedGroup.users) {
      const row = doc.createElement("div");
      row.setAttribute("role", "listitem");
      row.setAttribute("class", "reactor-user-row");
      row.setAttribute("data-reactor-user-id", user.id);
      row.setAttribute("data-user-id", user.id);
      row.setAttribute("data-reaction-type", String(user.reactionType));

      const displayNameEl = doc.createElement("span");
      displayNameEl.setAttribute("class", "reactor-display-name");
      displayNameEl.setAttribute("data-reactor-display-name", "");
      displayNameEl.textContent = user.displayName;
      row.appendChild(displayNameEl);

      const usernameEl = doc.createElement("span");
      usernameEl.setAttribute("class", "reactor-username");
      usernameEl.setAttribute("data-reactor-username", "");
      usernameEl.textContent = user.username;
      row.appendChild(usernameEl);

      if (user.reactionType === REACTION_TYPES.BURST) {
        const badgeEl = doc.createElement("span");
        badgeEl.setAttribute("class", "reactor-burst-badge");
        badgeEl.setAttribute("data-reactor-burst-badge", "");
        badgeEl.textContent = BURST_BADGE_LABEL_RU;
        row.appendChild(badgeEl);
      }

      userNodes.push(row);
    }

    replaceElementChildren(usersListContainerEl, userNodes);
  }

  /**
   * Переключает активную вкладку фильтра по эмодзи в модальном окне.
   *
   * @param {string} emoji
   * @returns {boolean}
   */
  function selectEmojiTab(emoji) {
    if (!modalEl || !tabButtonMap.has(emoji)) {
      return false;
    }
    activeEmoji = emoji;
    modalEl.setAttribute("data-active-emoji", emoji);

    for (const [key, btn] of tabButtonMap.entries()) {
      const selected = key === emoji;
      btn.setAttribute("aria-selected", selected ? "true" : "false");
      btn.setAttribute("tabindex", selected ? "0" : "-1");
    }

    renderReactorsUsersList();
    return true;
  }

  /**
   * Закрывает и размонтирует модальное окно списка реакций.
   */
  function closeReactorsModal() {
    if (modalEl) {
      removeElementFromParent(containerEl, modalEl);
    }
    modalEl = null;
    currentGroups = [];
    activeEmoji = null;
    tabButtonMap = new Map();
    usersListContainerEl = null;
  }

  /**
   * Открывает модальное окно «Реакции» с вкладками фильтра эмодзи и списком пользователей.
   *
   * @param {string} messageId
   * @param {ReadonlyArray<ReactionWithUsersGroup> | null | undefined} reactionsWithUsers
   * @returns {any}
   */
  function openReactorsModal(messageId, reactionsWithUsers) {
    closeReactorsModal();

    const normalizedMessageId = String(messageId ?? "");
    currentGroups = normalizeReactionsWithUsers(reactionsWithUsers);
    activeEmoji = currentGroups[0]?.emoji ?? null;

    if (typeof options.onOpenReactors === "function") {
      options.onOpenReactors(normalizedMessageId, reactionsWithUsers);
    }

    const dialog = doc.createElement("div");
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-label", REACTORS_DIALOG_TITLE_RU);
    dialog.setAttribute("data-reactors-modal", "");
    dialog.setAttribute("data-message-id", normalizedMessageId);
    if (activeEmoji) {
      dialog.setAttribute("data-active-emoji", activeEmoji);
    }

    const header = doc.createElement("header");
    header.setAttribute("data-reactors-header", "");

    const titleEl = doc.createElement("h2");
    titleEl.setAttribute("data-reactors-title", "");
    titleEl.textContent = REACTORS_DIALOG_TITLE_RU;
    header.appendChild(titleEl);
    dialog.appendChild(header);

    const tabList = doc.createElement("div");
    tabList.setAttribute("role", "tablist");
    tabList.setAttribute("aria-label", REACTORS_DIALOG_TITLE_RU);
    tabList.setAttribute("data-reactors-tabs", "");

    for (const group of currentGroups) {
      const tabBtn = doc.createElement("button");
      tabBtn.setAttribute("type", "button");
      tabBtn.setAttribute("role", "tab");
      tabBtn.setAttribute("data-emoji", group.emoji);
      tabBtn.setAttribute("data-tab-emoji", group.emoji);
      tabBtn.setAttribute("data-tab-count", String(group.count));
      const isSelected = group.emoji === activeEmoji;
      tabBtn.setAttribute("aria-selected", isSelected ? "true" : "false");
      tabBtn.setAttribute("tabindex", isSelected ? "0" : "-1");

      const emojiPart = doc.createElement("span");
      emojiPart.setAttribute("data-tab-emoji-glyph", "");
      emojiPart.textContent = group.emoji;

      const spacer = doc.createElement("span");
      spacer.textContent = " ";

      const countPart = doc.createElement("span");
      countPart.setAttribute("data-tab-emoji-count", "");
      countPart.textContent = String(group.count);

      tabBtn.appendChild(emojiPart);
      tabBtn.appendChild(spacer);
      tabBtn.appendChild(countPart);

      tabBtn.addEventListener("click", () => {
        selectEmojiTab(group.emoji);
      });

      tabButtonMap.set(group.emoji, tabBtn);
      tabList.appendChild(tabBtn);
    }

    dialog.appendChild(tabList);

    const listEl = doc.createElement("div");
    listEl.setAttribute("role", "list");
    listEl.setAttribute("data-reactors-list", "");
    usersListContainerEl = listEl;
    dialog.appendChild(listEl);

    modalEl = dialog;
    containerEl.appendChild(dialog);
    renderReactorsUsersList();
    return dialog;
  }

  return {
    renderReactionBar,
    openReactorsModal,
    selectEmojiTab,
    closeReactorsModal,
    close: closeReactorsModal,
  };
}
