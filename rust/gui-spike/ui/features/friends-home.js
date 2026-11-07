// @ts-check
/**
 * T-232 (FE-P1-25 / RW-40 / G2): контроллер домашнего экрана личных сообщений
 * и друзей — вкладки «В сети», «Все», «Ожидание», «Заблокированные» и
 * «Добавить в друзья» (`friends-home.js`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@258005` (`RelationshipType` enum `eA$`:
 *   `FRIEND = 1`, `BLOCKED = 2`, `PENDING_INCOMING = 3`, `PENDING_OUTGOING = 4`);
 * - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@243700` (`FRIENDS_SECTIONS`:
 *   `ONLINE`, `ALL`, `PENDING`, `BLOCKED`, `ADD_FRIEND`);
 * - `waves/RW-40/DM-FRIENDS-RE.md` (§4–§6) и `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (§2.3, §4).
 *
 * @module ui/features/friends-home
 */

/**
 * Коды типов связей пользователя (`RelationshipType`).
 * @type {Readonly<{ FRIEND: 1, BLOCKED: 2, PENDING_INCOMING: 3, PENDING_OUTGOING: 4 }>}
 */
export const RELATIONSHIP_TYPES = Object.freeze({
  FRIEND: 1,
  BLOCKED: 2,
  PENDING_INCOMING: 3,
  PENDING_OUTGOING: 4,
});

/**
 * Русские подписи вкладок домашнего экрана друзей.
 * @type {Readonly<{ online: string, all: string, pending: string, blocked: string, add: string }>}
 */
export const FRIENDS_TABS_RU = Object.freeze({
  online: "В сети",
  all: "Все",
  pending: "Ожидание",
  blocked: "Заблокированные",
  add: "Добавить в друзья",
});

/**
 * Упорядоченный список идентификаторов вкладок экрана друзей.
 * @type {ReadonlyArray<FriendsTabId>}
 */
const TAB_ORDER = Object.freeze(
  /** @type {const} */ (["online", "all", "pending", "blocked", "add"]),
);

/**
 * Статусы присутствия, считающиеся активными («В сети»).
 * @type {ReadonlySet<string>}
 */
const ONLINE_STATUSES = new Set(["online", "idle", "dnd"]);

/**
 * @typedef {"online" | "all" | "pending" | "blocked" | "add"} FriendsTabId
 */

/**
 * @typedef {"online" | "idle" | "dnd" | "offline"} FriendPresenceStatus
 */

/**
 * @typedef {1 | 2 | 3 | 4} RelationshipTypeCode
 */

/**
 * @typedef {object} RelationshipEntry
 * @property {string} userId
 * @property {string} username
 * @property {string} [displayName]
 * @property {RelationshipTypeCode | number} type
 * @property {FriendPresenceStatus | string} [status]
 */

/**
 * @typedef {object} FriendsHomeControllerOptions
 * @property {any} [container]
 * @property {ReadonlyArray<RelationshipEntry>} [relationships]
 * @property {FriendsTabId} [initialTab]
 * @property {string} [searchQuery]
 * @property {(userId: string, entry?: RelationshipEntry) => void} [onOpenDm]
 * @property {(userId: string, entry?: RelationshipEntry) => void} [onAcceptRequest]
 * @property {(userId: string, entry?: RelationshipEntry) => void} [onRemoveOrBlock]
 * @property {(username: string) => void} [onSendFriendRequest]
 */

/**
 * Возвращает отображаемое имя пользователя (`displayName || username`).
 *
 * @param {RelationshipEntry} entry
 * @returns {string}
 */
function getRelationshipDisplayLabel(entry) {
  const display = typeof entry?.displayName === "string" ? entry.displayName.trim() : "";
  if (display.length > 0) {
    return display;
  }
  return typeof entry?.username === "string" ? entry.username.trim() : "";
}

/**
 * Нормализует статус присутствия пользователя.
 *
 * @param {unknown} rawStatus
 * @returns {FriendPresenceStatus}
 */
function normalizePresenceStatus(rawStatus) {
  if (typeof rawStatus === "string" && ONLINE_STATUSES.has(rawStatus)) {
    return /** @type {FriendPresenceStatus} */ (rawStatus);
  }
  return "offline";
}

/**
 * Проверяет, принадлежит ли запись связи указанной вкладке.
 *
 * @param {RelationshipEntry} entry
 * @param {FriendsTabId | string} tab
 * @returns {boolean}
 */
function matchesFriendsTab(entry, tab) {
  const relType = entry?.type;
  if (tab === "online") {
    const status = normalizePresenceStatus(entry?.status);
    return relType === RELATIONSHIP_TYPES.FRIEND && status !== "offline";
  }
  if (tab === "all") {
    return relType === RELATIONSHIP_TYPES.FRIEND;
  }
  if (tab === "pending") {
    return (
      relType === RELATIONSHIP_TYPES.PENDING_INCOMING ||
      relType === RELATIONSHIP_TYPES.PENDING_OUTGOING
    );
  }
  if (tab === "blocked") {
    return relType === RELATIONSHIP_TYPES.BLOCKED;
  }
  return false;
}

/**
 * Фильтрует список связей `relationships` по активной вкладке `tab` и
 * регистронезависимому поисковому запросу `searchQuery` (по `displayName || username`),
 * возвращая новый массив, отсортированный по алфавиту.
 *
 * Правила вкладок:
 * - `"online"`: только `type === 1` (`FRIEND`) и `status !== "offline"` (`"online" | "idle" | "dnd"`).
 * - `"all"`: все `type === 1` (`FRIEND`).
 * - `"pending"`: `type === 3` (`PENDING_INCOMING`) или `type === 4` (`PENDING_OUTGOING`).
 * - `"blocked"`: `type === 2` (`BLOCKED`).
 * - `"add"` (или любая иная вкладка): пустой массив `[]`.
 *
 * @param {ReadonlyArray<RelationshipEntry>} relationships
 * @param {FriendsTabId | string} tab
 * @param {string} [searchQuery]
 * @returns {RelationshipEntry[]}
 */
export function filterRelationshipsByTab(relationships, tab, searchQuery = "") {
  if (!Array.isArray(relationships) || relationships.length === 0) {
    return [];
  }

  const normalizedQuery =
    typeof searchQuery === "string" ? searchQuery.trim().toLocaleLowerCase("ru") : "";

  /** @type {RelationshipEntry[]} */
  const filtered = [];

  for (const entry of relationships) {
    if (!entry || typeof entry !== "object") {
      continue;
    }
    if (!matchesFriendsTab(entry, tab)) {
      continue;
    }
    if (normalizedQuery.length > 0) {
      const label = getRelationshipDisplayLabel(entry).toLocaleLowerCase("ru");
      if (!label.includes(normalizedQuery)) {
        continue;
      }
    }
    filtered.push(entry);
  }

  return filtered.sort((a, b) =>
    getRelationshipDisplayLabel(a).localeCompare(getRelationshipDisplayLabel(b), "ru", {
      sensitivity: "base",
    }),
  );
}

/**
 * Подсчитывает число входящих заявок в друзья (`type === 3`).
 *
 * @param {ReadonlyArray<RelationshipEntry>} relationships
 * @returns {number}
 */
function countIncomingPendingRequests(relationships) {
  if (!Array.isArray(relationships) || relationships.length === 0) {
    return 0;
  }
  let count = 0;
  for (const entry of relationships) {
    if (entry && entry.type === RELATIONSHIP_TYPES.PENDING_INCOMING) {
      count += 1;
    }
  }
  return count;
}

/**
 * Проверяет, является ли значение допустимым идентификатором вкладки друзей.
 *
 * @param {unknown} value
 * @returns {value is FriendsTabId}
 */
function isValidFriendsTab(value) {
  return (
    value === "online" ||
    value === "all" ||
    value === "pending" ||
    value === "blocked" ||
    value === "add"
  );
}

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
 * Создаёт контроллер домашнего экрана личных сообщений и друзей (`[data-friends-home]`).
 *
 * Поддерживает вызов как `createFriendsHomeController({ container, ... })`,
 * так и `createFriendsHomeController(container, options)`.
 *
 * @param {FriendsHomeControllerOptions} configOrContainer
 * @param {FriendsHomeControllerOptions} [maybeOptions]
 */
export function createFriendsHomeController(configOrContainer, maybeOptions = {}) {
  const rawFirst = /** @type {any} */ (configOrContainer);
  const isFirstArgContainer =
    rawFirst &&
    typeof rawFirst === "object" &&
    !("container" in rawFirst) &&
    (typeof rawFirst.appendChild === "function" ||
      typeof rawFirst.createElement === "function");

  /** @type {FriendsHomeControllerOptions} */
  const options = isFirstArgContainer
    ? { ...maybeOptions, container: rawFirst }
    : (configOrContainer ?? {});

  const container = options.container;
  if (!container || typeof container.appendChild !== "function") {
    throw new TypeError("options.container must be a valid DOM container element");
  }
  const doc = resolveDocument(container);

  /** @type {RelationshipEntry[]} */
  let currentRelationships = Array.isArray(options.relationships)
    ? [...options.relationships]
    : [];
  /** @type {FriendsTabId} */
  let activeTab = isValidFriendsTab(options.initialTab) ? options.initialTab : "online";
  /** @type {string} */
  let currentSearchQuery =
    typeof options.searchQuery === "string" ? options.searchQuery : "";

  const root = doc.createElement("section");
  root.setAttribute("data-friends-home", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Друзья");

  const headerEl = doc.createElement("header");
  headerEl.setAttribute("data-friends-header", "");

  const tabBarEl = doc.createElement("div");
  tabBarEl.setAttribute("data-friends-tabbar", "");
  tabBarEl.setAttribute("role", "tablist");
  tabBarEl.setAttribute("aria-label", "Вкладки друзей");

  /** @type {Map<FriendsTabId, any>} */
  const tabButtons = new Map();

  for (const tabId of TAB_ORDER) {
    const btn = doc.createElement("button");
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "tab");
    btn.setAttribute("data-friends-tab", tabId);
    btn.textContent = FRIENDS_TABS_RU[tabId];
    btn.addEventListener("click", () => {
      setTab(tabId);
    });
    tabButtons.set(tabId, btn);
    tabBarEl.appendChild(btn);
  }

  const pendingBadgeEl = doc.createElement("span");
  pendingBadgeEl.setAttribute("data-friends-pending-badge", "");
  pendingBadgeEl.setAttribute("aria-label", "Входящие заявки в друзья");

  headerEl.appendChild(tabBarEl);
  headerEl.appendChild(pendingBadgeEl);

  const searchInputEl = doc.createElement("input");
  searchInputEl.setAttribute("type", "search");
  searchInputEl.setAttribute("data-friends-search", "");
  searchInputEl.setAttribute("placeholder", "Поиск");
  searchInputEl.setAttribute("aria-label", "Поиск друзей");
  searchInputEl.value = currentSearchQuery;
  searchInputEl.addEventListener("input", () => {
    currentSearchQuery = typeof searchInputEl.value === "string" ? searchInputEl.value : "";
    renderListDom();
  });

  const listEl = doc.createElement("div");
  listEl.setAttribute("data-friends-list", "");
  listEl.setAttribute("role", "list");

  const addFriendFormEl = doc.createElement("form");
  addFriendFormEl.setAttribute("data-add-friend-form", "");
  addFriendFormEl.setAttribute("aria-label", FRIENDS_TABS_RU.add);

  const addFriendInputEl = doc.createElement("input");
  addFriendInputEl.setAttribute("type", "text");
  addFriendInputEl.setAttribute("data-add-friend-input", "");
  addFriendInputEl.setAttribute("placeholder", "Вы можете добавить друзей по имени пользователя Discord");
  addFriendInputEl.setAttribute("aria-label", "Имя пользователя Discord");

  const addFriendSubmitBtn = doc.createElement("button");
  addFriendSubmitBtn.setAttribute("type", "submit");
  addFriendSubmitBtn.setAttribute("data-add-friend-submit", "");
  addFriendSubmitBtn.textContent = "Отправить запрос дружбы";

  addFriendFormEl.appendChild(addFriendInputEl);
  addFriendFormEl.appendChild(addFriendSubmitBtn);

  /**
   * Обрабатывает отправку формы добавления в друзья.
   *
   * @param {string} [overrideUsername]
   * @returns {boolean}
   */
  function handleSendFriendRequest(overrideUsername) {
    const rawValue =
      typeof overrideUsername === "string"
        ? overrideUsername
        : typeof addFriendInputEl.value === "string"
          ? addFriendInputEl.value
          : "";
    const trimmed = rawValue.trim();
    if (trimmed.length === 0) {
      return false;
    }
    if (typeof options.onSendFriendRequest === "function") {
      options.onSendFriendRequest(trimmed);
    }
    if (overrideUsername === undefined) {
      addFriendInputEl.value = "";
    }
    return true;
  }

  addFriendFormEl.addEventListener("submit", (/** @type {any} */ event) => {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    handleSendFriendRequest();
  });

  addFriendSubmitBtn.addEventListener("click", (/** @type {any} */ event) => {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    handleSendFriendRequest();
  });

  /**
   * Создаёт DOM-строку `[data-friend-row]` для записи отношения.
   *
   * @param {RelationshipEntry} entry
   * @returns {any}
   */
  function createFriendRowEl(entry) {
    const userId = String(entry.userId ?? "");
    const displayName = getRelationshipDisplayLabel(entry);
    const status = normalizePresenceStatus(entry.status);
    const relType = entry.type;

    const rowEl = doc.createElement("div");
    rowEl.setAttribute("data-friend-row", userId);
    rowEl.setAttribute("data-user-id", userId);
    rowEl.setAttribute("data-relationship-type", String(relType));
    rowEl.setAttribute("role", "listitem");

    const nameEl = doc.createElement("span");
    nameEl.setAttribute("data-friend-name", "");
    nameEl.textContent = displayName;
    rowEl.appendChild(nameEl);

    const statusEl = doc.createElement("span");
    statusEl.setAttribute("data-friend-status", status);
    statusEl.textContent = status;
    rowEl.appendChild(statusEl);

    const actionsEl = doc.createElement("div");
    actionsEl.setAttribute("data-friend-actions", "");

    const dmBtn = doc.createElement("button");
    dmBtn.setAttribute("type", "button");
    dmBtn.setAttribute("data-friend-dm-btn", userId);
    dmBtn.setAttribute("aria-label", "Написать сообщение");
    dmBtn.textContent = "Сообщение";
    dmBtn.hidden = relType === RELATIONSHIP_TYPES.BLOCKED;
    dmBtn.addEventListener("click", () => {
      if (typeof options.onOpenDm === "function") {
        options.onOpenDm(userId, entry);
      }
    });
    actionsEl.appendChild(dmBtn);

    const acceptBtn = doc.createElement("button");
    acceptBtn.setAttribute("type", "button");
    acceptBtn.setAttribute("data-friend-accept-btn", userId);
    acceptBtn.setAttribute("aria-label", "Принять запрос дружбы");
    acceptBtn.textContent = "Принять";
    acceptBtn.hidden = relType !== RELATIONSHIP_TYPES.PENDING_INCOMING;
    acceptBtn.addEventListener("click", () => {
      if (typeof options.onAcceptRequest === "function") {
        options.onAcceptRequest(userId, entry);
      }
    });
    actionsEl.appendChild(acceptBtn);

    const removeBtn = doc.createElement("button");
    removeBtn.setAttribute("type", "button");
    removeBtn.setAttribute("data-friend-remove-btn", userId);
    removeBtn.setAttribute(
      "aria-label",
      relType === RELATIONSHIP_TYPES.BLOCKED ? "Разблокировать" : "Удалить",
    );
    removeBtn.textContent =
      relType === RELATIONSHIP_TYPES.BLOCKED ? "Разблокировать" : "Удалить";
    removeBtn.addEventListener("click", () => {
      if (typeof options.onRemoveOrBlock === "function") {
        options.onRemoveOrBlock(userId, entry);
      }
    });
    actionsEl.appendChild(removeBtn);

    rowEl.appendChild(actionsEl);
    return rowEl;
  }

  function syncHeaderAndTabsDom() {
    const pendingCount = countIncomingPendingRequests(currentRelationships);
    pendingBadgeEl.textContent = String(pendingCount);
    pendingBadgeEl.setAttribute("data-count", String(pendingCount));
    pendingBadgeEl.hidden = pendingCount === 0;

    for (const tabId of TAB_ORDER) {
      const btn = tabButtons.get(tabId);
      if (!btn) continue;
      const isSelected = tabId === activeTab;
      btn.setAttribute("aria-selected", isSelected ? "true" : "false");
      btn.setAttribute("tabindex", isSelected ? "0" : "-1");
    }
  }

  function renderListDom() {
    const visible = filterRelationshipsByTab(
      currentRelationships,
      activeTab,
      currentSearchQuery,
    );
    const rowNodes = visible.map((entry) => createFriendRowEl(entry));
    replaceElementChildren(listEl, rowNodes);
  }

  function renderDom() {
    syncHeaderAndTabsDom();
    renderListDom();

    if (activeTab === "add") {
      replaceElementChildren(root, [headerEl, addFriendFormEl]);
    } else {
      replaceElementChildren(root, [headerEl, searchInputEl, listEl]);
    }
  }

  /**
   * Переключает активную вкладку экрана друзей.
   *
   * @param {FriendsTabId | string} nextTab
   * @returns {boolean}
   */
  function setTab(nextTab) {
    if (!isValidFriendsTab(nextTab)) {
      return false;
    }
    activeTab = nextTab;
    renderDom();
    return true;
  }

  /**
   * Обновляет поисковый запрос и перерисовывает список.
   *
   * @param {string} nextQuery
   */
  function setSearchQuery(nextQuery) {
    currentSearchQuery = typeof nextQuery === "string" ? nextQuery : "";
    searchInputEl.value = currentSearchQuery;
    renderListDom();
  }

  /**
   * Обновляет список связей и перерисовывает представление.
   *
   * @param {ReadonlyArray<RelationshipEntry>} [nextRelationships]
   */
  function setRelationships(nextRelationships) {
    if (Array.isArray(nextRelationships)) {
      currentRelationships = [...nextRelationships];
    }
    renderDom();
  }

  renderDom();
  replaceElementChildren(container, [root]);

  return {
    root,
    element: root,
    render: setRelationships,
    setRelationships,
    setTab,
    /** @returns {FriendsTabId} */
    getActiveTab() {
      return activeTab;
    },
    setSearchQuery,
    /** @returns {string} */
    getSearchQuery() {
      return currentSearchQuery;
    },
    /** @returns {number} */
    getPendingCount() {
      return countIncomingPendingRequests(currentRelationships);
    },
    /** @returns {RelationshipEntry[]} */
    getVisibleRelationships() {
      return filterRelationshipsByTab(currentRelationships, activeTab, currentSearchQuery);
    },
    submitAddFriend: handleSendFriendRequest,
    destroy() {
      if (typeof root.remove === "function") {
        root.remove();
      } else if (root.parentNode && typeof root.parentNode.removeChild === "function") {
        root.parentNode.removeChild(root);
      }
    },
  };
}
