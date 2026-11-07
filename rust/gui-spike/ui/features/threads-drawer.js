// @ts-check
/**
 * T-208: контроллер панели веток канала (`threads-drawer.js`).
 *
 * Отвечает за:
 * - справочник сроков автоархивации `AUTO_ARCHIVE_DURATIONS_RU` (`60 | 1440 | 4320 | 10080` мин);
 * - рендер панели веток с вкладками `Активные ветки` (`active`) и `Архивные ветки` (`archived`),
 *   полем поиска (`placeholder="Поиск веток..."`) и списком подходящих веток
 *   (или пустым состоянием `Ветки не найдены`);
 * - рендер каждой карточки ветки (`name`, `messageCount`, метка автоархивации из
 *   `AUTO_ARCHIVE_DURATIONS_RU`, кнопка `Покинуть ветку` / `Вступить в ветку` с вызовом
 *   `onToggleJoin(thread.id, !thread.joined)`) и открытие split-pane ветки по клику
 *   с вызовом `onOpenThread(thread.id)`;
 * - валидацию и создание новой ветки `createThread({ name, autoArchiveDuration = 1440, starterMessage })`.
 *
 * @module ui/features/threads-drawer
 */

/**
 * @typedef {60 | 1440 | 4320 | 10080} AutoArchiveDurationMinutes
 */

/**
 * @typedef {object} AutoArchiveDurationEntry
 * @property {AutoArchiveDurationMinutes} minutes
 * @property {string} labelRu
 */

/**
 * Допустимые сроки автоархивации веток с русскими подписями.
 * @type {ReadonlyArray<Readonly<AutoArchiveDurationEntry>>}
 */
export const AUTO_ARCHIVE_DURATIONS_RU = Object.freeze([
  Object.freeze({ minutes: /** @type {const} */ (60), labelRu: "1 ч." }),
  Object.freeze({ minutes: /** @type {const} */ (1440), labelRu: "24 ч." }),
  Object.freeze({ minutes: /** @type {const} */ (4320), labelRu: "3 дн." }),
  Object.freeze({ minutes: /** @type {const} */ (10080), labelRu: "1 нед." }),
]);

/**
 * @typedef {"active" | "archived"} ThreadsDrawerTab
 */

/**
 * @typedef {object} ThreadDrawerItem
 * @property {string} id
 * @property {string} name
 * @property {boolean} [archived]
 * @property {number} [messageCount]
 * @property {number} [autoArchiveDuration]
 * @property {boolean} [joined]
 */

/**
 * @typedef {object} ThreadsDrawerInitialData
 * @property {ThreadDrawerItem[]} [threads]
 * @property {ThreadsDrawerTab} [activeTab]
 * @property {string} [query]
 */

/**
 * @typedef {object} CreateThreadDraft
 * @property {string} [name]
 * @property {number} [autoArchiveDuration]
 * @property {string} [starterMessage]
 */

/**
 * @typedef {object} CreateThreadPayload
 * @property {string} name
 * @property {AutoArchiveDurationMinutes} autoArchiveDuration
 * @property {string} starterMessage
 */

/**
 * @typedef {object} CreateThreadResult
 * @property {boolean} ok
 * @property {string} [errorRu]
 */

/**
 * @typedef {object} ThreadsDrawerOptions
 * @property {(threadId: string) => void} [onOpenThread]
 * @property {(payload: CreateThreadPayload) => void} [onCreateThread]
 * @property {(threadId: string, nextJoined: boolean) => void} [onToggleJoin]
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
  throw new TypeError("Container element must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы контейнера на переданный массив элементов.
 *
 * @param {any} containerEl
 * @param {any[]} nodes
 */
function replaceContainerChildren(containerEl, nodes) {
  if (typeof containerEl.replaceChildren === "function") {
    containerEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(containerEl.children)) {
    containerEl.children.length = 0;
  }
  for (const node of nodes) {
    containerEl.appendChild(node);
  }
}

/**
 * Проверяет, является ли число допустимым сроком автоархивации из `AUTO_ARCHIVE_DURATIONS_RU`.
 *
 * @param {unknown} minutes
 * @returns {minutes is AutoArchiveDurationMinutes}
 */
function isValidAutoArchiveDuration(minutes) {
  return (
    typeof minutes === "number" &&
    Number.isFinite(minutes) &&
    AUTO_ARCHIVE_DURATIONS_RU.some((entry) => entry.minutes === minutes)
  );
}

/**
 * Находит русскую метку срока автоархивации по количеству минут.
 *
 * @param {unknown} minutes
 * @returns {string}
 */
function resolveAutoArchiveLabelRu(minutes) {
  const num = typeof minutes === "number" ? minutes : Number(minutes);
  const found = AUTO_ARCHIVE_DURATIONS_RU.find((entry) => entry.minutes === num);
  return found ? found.labelRu : "24 ч.";
}

/**
 * Нормализует входной элемент ветки.
 *
 * @param {ThreadDrawerItem} raw
 * @returns {Required<ThreadDrawerItem>}
 */
function normalizeThreadItem(raw) {
  const messageCount =
    typeof raw?.messageCount === "number" &&
    Number.isFinite(raw.messageCount) &&
    raw.messageCount >= 0
      ? Math.floor(raw.messageCount)
      : 0;
  const duration = isValidAutoArchiveDuration(raw?.autoArchiveDuration)
    ? raw.autoArchiveDuration
    : 1440;
  return {
    id: String(raw?.id ?? ""),
    name: typeof raw?.name === "string" ? raw.name : "",
    archived: Boolean(raw?.archived),
    messageCount,
    autoArchiveDuration: duration,
    joined: Boolean(raw?.joined),
  };
}

/**
 * Создаёт контроллер панели веток внутри `containerEl`.
 *
 * @param {any} containerEl
 * @param {ThreadsDrawerInitialData | ThreadDrawerItem[]} [initialData]
 * @param {ThreadsDrawerOptions} [options]
 */
export function createThreadsDrawerController(containerEl, initialData = {}, options = {}) {
  const doc = resolveDocument(containerEl);

  const normalizedInit = Array.isArray(initialData)
    ? { threads: initialData }
    : (initialData ?? {});

  /** @type {Array<Required<ThreadDrawerItem>>} */
  let threads = Array.isArray(normalizedInit.threads)
    ? normalizedInit.threads.map((item) => normalizeThreadItem(item))
    : [];

  /** @type {ThreadsDrawerTab} */
  let activeTab = normalizedInit.activeTab === "archived" ? "archived" : "active";

  /** @type {string} */
  let query = typeof normalizedInit.query === "string" ? normalizedInit.query : "";

  /** @type {string | null} */
  let openedThreadId = null;

  const root = doc.createElement("section");
  root.setAttribute("data-threads-drawer", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Ветки");

  const headerEl = doc.createElement("header");
  headerEl.setAttribute("data-threads-header", "");

  const tabsEl = doc.createElement("div");
  tabsEl.setAttribute("data-threads-tabs", "");
  tabsEl.setAttribute("role", "tablist");
  tabsEl.setAttribute("aria-label", "Разделы веток");

  const tabActiveBtn = doc.createElement("button");
  tabActiveBtn.setAttribute("type", "button");
  tabActiveBtn.setAttribute("role", "tab");
  tabActiveBtn.setAttribute("data-tab", "active");
  tabActiveBtn.setAttribute("data-threads-tab", "active");
  tabActiveBtn.textContent = "Активные ветки";
  tabActiveBtn.addEventListener("click", () => {
    setTab("active");
  });

  const tabArchivedBtn = doc.createElement("button");
  tabArchivedBtn.setAttribute("type", "button");
  tabArchivedBtn.setAttribute("role", "tab");
  tabArchivedBtn.setAttribute("data-tab", "archived");
  tabArchivedBtn.setAttribute("data-threads-tab", "archived");
  tabArchivedBtn.textContent = "Архивные ветки";
  tabArchivedBtn.addEventListener("click", () => {
    setTab("archived");
  });

  tabsEl.appendChild(tabActiveBtn);
  tabsEl.appendChild(tabArchivedBtn);
  headerEl.appendChild(tabsEl);

  const searchInput = doc.createElement("input");
  searchInput.setAttribute("type", "search");
  searchInput.setAttribute("data-threads-search", "");
  searchInput.setAttribute("placeholder", "Поиск веток...");
  searchInput.setAttribute("aria-label", "Поиск веток");
  searchInput.placeholder = "Поиск веток...";
  searchInput.value = query;
  searchInput.addEventListener("input", (/** @type {any} */ event) => {
    const nextValue = event?.target?.value ?? searchInput.value ?? "";
    query = String(nextValue);
    renderThreads();
  });
  headerEl.appendChild(searchInput);

  const listContainer = doc.createElement("div");
  listContainer.setAttribute("data-threads-list", "");
  listContainer.setAttribute("role", "tabpanel");

  root.appendChild(headerEl);
  root.appendChild(listContainer);
  replaceContainerChildren(containerEl, [root]);

  function syncTabs() {
    const isActive = activeTab === "active";
    tabActiveBtn.setAttribute("aria-selected", isActive ? "true" : "false");
    tabActiveBtn.setAttribute("aria-pressed", isActive ? "true" : "false");
    tabActiveBtn.setAttribute("tabindex", isActive ? "0" : "-1");

    tabArchivedBtn.setAttribute("aria-selected", !isActive ? "true" : "false");
    tabArchivedBtn.setAttribute("aria-pressed", !isActive ? "true" : "false");
    tabArchivedBtn.setAttribute("tabindex", !isActive ? "0" : "-1");
  }

  /**
   * Переключает активную вкладку (`active` | `archived`) и перерисовывает список.
   *
   * @param {ThreadsDrawerTab} nextTab
   */
  function setTab(nextTab) {
    activeTab = nextTab === "archived" ? "archived" : "active";
    syncTabs();
    renderThreads();
  }

  function renderThreads() {
    const wantArchived = activeTab === "archived";
    const normalizedQuery = query.trim().toLowerCase();

    const matching = threads.filter((thread) => {
      if (Boolean(thread.archived) !== wantArchived) return false;
      if (normalizedQuery.length === 0) return true;
      return thread.name.toLowerCase().includes(normalizedQuery);
    });

    if (matching.length === 0) {
      const emptyEl = doc.createElement("p");
      emptyEl.setAttribute("data-threads-empty", "");
      emptyEl.textContent = "Ветки не найдены";
      replaceContainerChildren(listContainer, [emptyEl]);
      return;
    }

    /** @type {any[]} */
    const itemNodes = [];
    for (const thread of matching) {
      const isJoined = Boolean(thread.joined);
      const archiveLabel = resolveAutoArchiveLabelRu(thread.autoArchiveDuration);

      const itemEl = doc.createElement("article");
      itemEl.setAttribute("data-thread-item", "");
      itemEl.setAttribute("data-thread-id", thread.id);
      itemEl.setAttribute("data-archived", thread.archived ? "true" : "false");
      itemEl.setAttribute("data-joined", isJoined ? "true" : "false");
      itemEl.setAttribute("data-auto-archive-duration", String(thread.autoArchiveDuration));
      itemEl.setAttribute("role", "button");
      itemEl.setAttribute("tabindex", "0");

      const nameEl = doc.createElement("h3");
      nameEl.setAttribute("data-thread-name", "");
      nameEl.textContent = thread.name;
      itemEl.appendChild(nameEl);

      const msgCountEl = doc.createElement("span");
      msgCountEl.setAttribute("data-message-count", String(thread.messageCount));
      msgCountEl.textContent = String(thread.messageCount);
      itemEl.appendChild(msgCountEl);

      const archiveLabelEl = doc.createElement("span");
      archiveLabelEl.setAttribute("data-auto-archive-label", "");
      archiveLabelEl.setAttribute("data-auto-archive-minutes", String(thread.autoArchiveDuration));
      archiveLabelEl.textContent = archiveLabel;
      itemEl.appendChild(archiveLabelEl);

      const joinBtn = doc.createElement("button");
      joinBtn.setAttribute("type", "button");
      joinBtn.setAttribute("data-thread-join-btn", "");
      joinBtn.setAttribute("data-joined", isJoined ? "true" : "false");
      joinBtn.setAttribute("aria-pressed", isJoined ? "true" : "false");
      joinBtn.textContent = isJoined ? "Покинуть ветку" : "Вступить в ветку";
      joinBtn.addEventListener("click", (/** @type {any} */ event) => {
        if (event && typeof event.stopPropagation === "function") {
          event.stopPropagation();
        }
        const nextJoined = !Boolean(thread.joined);
        thread.joined = nextJoined;
        if (typeof options.onToggleJoin === "function") {
          options.onToggleJoin(thread.id, nextJoined);
        }
        renderThreads();
      });
      itemEl.appendChild(joinBtn);

      itemEl.addEventListener("click", (/** @type {any} */ event) => {
        if (event?.target === joinBtn) return;
        openedThreadId = thread.id;
        root.setAttribute("data-open-thread-id", thread.id);
        if (typeof options.onOpenThread === "function") {
          options.onOpenThread(thread.id);
        }
      });

      itemNodes.push(itemEl);
    }

    replaceContainerChildren(listContainer, itemNodes);
  }

  /**
   * Валидирует параметры и создаёт новую ветку.
   *
   * @param {CreateThreadDraft} [draft]
   * @returns {CreateThreadResult}
   */
  function createThread(draft = {}) {
    const trimmedName = typeof draft?.name === "string" ? draft.name.trim() : "";
    if (trimmedName.length === 0) {
      return {
        ok: false,
        errorRu: "Введите название ветки",
      };
    }

    const duration =
      draft?.autoArchiveDuration === undefined ? 1440 : draft.autoArchiveDuration;
    if (!isValidAutoArchiveDuration(duration)) {
      return {
        ok: false,
        errorRu: "Некорректный срок автоархивации",
      };
    }

    const starterMessage =
      typeof draft?.starterMessage === "string" ? draft.starterMessage.trim() : "";

    /** @type {CreateThreadPayload} */
    const payload = {
      name: trimmedName,
      autoArchiveDuration: duration,
      starterMessage,
    };

    if (typeof options.onCreateThread === "function") {
      options.onCreateThread(payload);
    }

    return { ok: true };
  }

  syncTabs();
  renderThreads();

  return {
    element: root,
    createThread,
    setTab,
    /** @returns {ThreadsDrawerTab} */
    getActiveTab() {
      return activeTab;
    },
    /** @returns {string} */
    getQuery() {
      return query;
    },
    /** @returns {string | null} */
    getOpenedThreadId() {
      return openedThreadId;
    },
  };
}
