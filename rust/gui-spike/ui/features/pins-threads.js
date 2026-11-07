// @ts-check
/**
 * STUB-10 (M-10 и M-11, RW-40): выпадающие панели пинов и веток в хедере чата.
 *
 * Спецификация:
 *   - waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md (§1, §2, §3, §5, §6)
 *   - M-10: выпадающая панель закреплённых сообщений (#chat-pins-panel)
 *   - M-11: выпадающая панель веток канала (#chat-threads-panel)
 *
 * Инварианты:
 *   1. Прогрессивное создание: если панелей нет в DOM, они создаются в рабочей области чата.
 *   2. Взаимное исключение: открытие pins автоматически закрывает threads и наоборот.
 *   3. 4 состояния (loading, empty, error, populated) для обеих панелей.
 *   4. Доступность (a11y): role='dialog', aria-label, aria-haspopup='dialog',
 *      aria-expanded, aria-controls, aria-busy.
 *   5. Фокус и закрытие: Escape, клик вне панели, кнопки закрытия — с возвратом фокуса
 *      на кнопку-триггер.
 */

/**
 * @typedef {{
 *   id: string,
 *   author?: string,
 *   content?: string,
 *   timestamp?: string,
 * }} PinItem
 *
 * @typedef {{
 *   id: string,
 *   name: string,
 *   replyCount?: number,
 *   lastMessageTime?: string,
 *   archived?: boolean,
 * }} ThreadItem
 *
 * @typedef {{
 *   document?: any,
 *   container?: any,
 *   pinsButton?: any,
 *   threadsButton?: any,
 *   pinsPanel?: any,
 *   threadsPanel?: any,
 *   onJumpToMessage?: (messageId: string) => void,
 *   onOpenThread?: (threadId: string) => void,
 *   onRetryPins?: () => void,
 *   onRetryThreads?: () => void,
 *   loadPins?: () => Promise<PinItem[]> | PinItem[],
 *   loadThreads?: () => Promise<ThreadItem[]> | ThreadItem[],
 *   router?: any,
 * }} PinsThreadsOptions
 *
 * @typedef {{
 *   pinsPanel: any,
 *   threadsPanel: any,
 *   pinsButton: any,
 *   threadsButton: any,
 *   openPins: () => void,
 *   closePins: (opts?: { restoreFocus?: boolean }) => void,
 *   togglePins: () => void,
 *   openThreads: () => void,
 *   closeThreads: (opts?: { restoreFocus?: boolean }) => void,
 *   toggleThreads: () => void,
 *   closeAll: (opts?: { restoreFocus?: boolean }) => void,
 *   setPinsState: (state: "loading" | "empty" | "error" | "populated", data?: any) => void,
 *   setThreadsState: (state: "loading" | "empty" | "error" | "populated", data?: any) => void,
 *   searchThreads: (query: string) => void,
 *   setActiveTab: (tabId: "active" | "archived") => void,
 *   destroy: () => void,
 * }} PinsThreadsController
 */

/**
 * Проверяет, лежит ли узел внутри контейнера (обход parentNode для совместимости с DOM-шимом).
 * @param {any} container
 * @param {any} node
 * @returns {boolean}
 */
export function isInside(container, node) {
  if (!container || !node) return false;
  /** @type {any} */
  let current = node;
  while (current) {
    if (current === container) return true;
    current = current.parentNode ?? null;
  }
  return false;
}

/**
 * Прогрессивное создание DOM-узла `#chat-pins-panel` (§2.2).
 * @param {any} doc
 * @param {any} [container]
 * @returns {any}
 */
export function ensurePinsPanel(doc, container) {
  const existing = doc.getElementById("chat-pins-panel");
  if (existing) return existing;

  const panel = doc.createElement("section");
  panel.id = "chat-pins-panel";
  panel.className = "header-dropdown-panel pins-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", "Закреплённые сообщения");
  panel.setAttribute("aria-modal", "false");
  panel.setAttribute("data-state", "empty");
  panel.setAttribute("tabindex", "-1");
  panel.setAttribute("hidden", "");

  const header = doc.createElement("header");
  header.className = "dropdown-header";

  const title = doc.createElement("h2");
  title.className = "dropdown-title";
  title.textContent = "Закреплённые сообщения";

  const closeBtn = doc.createElement("button");
  closeBtn.className = "icon-btn dropdown-close";
  closeBtn.type = "button";
  closeBtn.setAttribute("data-action", "close-pins");
  closeBtn.setAttribute("aria-label", "Закрыть");
  closeBtn.textContent = "✕";

  header.append(title, closeBtn);

  const content = doc.createElement("div");
  content.className = "dropdown-content pins-content";
  content.setAttribute("role", "feed");
  content.setAttribute("aria-busy", "false");

  panel.append(header, content);

  const target = container ?? doc.querySelector(".content") ?? doc.body;
  if (target && typeof target.append === "function") {
    target.append(panel);
  }

  return panel;
}

/**
 * Прогрессивное создание DOM-узла `#chat-threads-panel` (§3.2).
 * @param {any} doc
 * @param {any} [container]
 * @returns {any}
 */
export function ensureThreadsPanel(doc, container) {
  const existing = doc.getElementById("chat-threads-panel");
  if (existing) return existing;

  const panel = doc.createElement("section");
  panel.id = "chat-threads-panel";
  panel.className = "header-dropdown-panel threads-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", "Ветки");
  panel.setAttribute("aria-modal", "false");
  panel.setAttribute("data-state", "empty");
  panel.setAttribute("tabindex", "-1");
  panel.setAttribute("hidden", "");

  const header = doc.createElement("header");
  header.className = "dropdown-header";

  const tabs = doc.createElement("div");
  tabs.className = "threads-header-tabs";
  tabs.setAttribute("role", "tablist");

  const tabActive = doc.createElement("button");
  tabActive.setAttribute("role", "tab");
  tabActive.id = "tab-active-threads";
  tabActive.setAttribute("aria-selected", "true");
  tabActive.setAttribute("aria-controls", "threads-list");
  tabActive.setAttribute("tabindex", "0");
  tabActive.textContent = "Активные";

  const tabArchived = doc.createElement("button");
  tabArchived.setAttribute("role", "tab");
  tabArchived.id = "tab-archived-threads";
  tabArchived.setAttribute("aria-selected", "false");
  tabArchived.setAttribute("aria-controls", "threads-list");
  tabArchived.setAttribute("tabindex", "-1");
  tabArchived.textContent = "Архив";

  tabs.append(tabActive, tabArchived);

  const closeBtn = doc.createElement("button");
  closeBtn.className = "icon-btn dropdown-close";
  closeBtn.type = "button";
  closeBtn.setAttribute("data-action", "close-threads");
  closeBtn.setAttribute("aria-label", "Закрыть");
  closeBtn.textContent = "✕";

  header.append(tabs, closeBtn);

  const searchWrap = doc.createElement("div");
  searchWrap.className = "threads-search-wrap";

  const searchInput = doc.createElement("input");
  searchInput.type = "search";
  searchInput.className = "threads-search-input";
  searchInput.placeholder = "Поиск веток…";
  searchInput.setAttribute("aria-label", "Поиск веток");
  searchWrap.append(searchInput);

  const content = doc.createElement("div");
  content.id = "threads-list";
  content.className = "dropdown-content threads-content";
  content.setAttribute("role", "region");
  content.setAttribute("aria-busy", "false");

  panel.append(header, searchWrap, content);

  const target = container ?? doc.querySelector(".content") ?? doc.body;
  if (target && typeof target.append === "function") {
    target.append(panel);
  }

  return panel;
}

/**
 * Отрисовка состояния панели пинов (loading, empty, error, populated).
 * @param {any} doc
 * @param {any} panel
 * @param {"loading" | "empty" | "error" | "populated"} state
 * @param {PinItem[]} [items]
 * @param {{ onRetryPins?: () => void, onJumpToMessage?: (id: string) => void }} [callbacks]
 */
export function renderPinsState(doc, panel, state, items = [], callbacks = {}) {
  panel.setAttribute("data-state", state);
  const content = panel.querySelector(".pins-content");
  if (!content) return;

  if (typeof content.replaceChildren === "function") {
    content.replaceChildren();
  }

  if (state === "loading") {
    content.setAttribute("aria-busy", "true");

    const sr = doc.createElement("span");
    sr.className = "sr-only";
    sr.setAttribute("role", "status");
    sr.textContent = "Загрузка закреплённых сообщений…";
    content.append(sr);

    for (let i = 0; i < 3; i++) {
      const skeleton = doc.createElement("div");
      skeleton.className = "pin-skeleton";
      content.append(skeleton);
    }
  } else if (state === "empty") {
    content.setAttribute("aria-busy", "false");

    const emptyBox = doc.createElement("div");
    emptyBox.className = "dropdown-empty-state";

    const icon = doc.createElement("span");
    icon.className = "empty-icon";
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = "📌";

    const title = doc.createElement("p");
    title.className = "empty-title";
    title.textContent = "В этом канале нет закреплённых сообщений";

    const desc = doc.createElement("p");
    desc.className = "empty-desc";
    desc.textContent = "Сообщения могут быть закреплены через меню действий сообщения.";

    emptyBox.append(icon, title, desc);
    content.append(emptyBox);
  } else if (state === "error") {
    content.setAttribute("aria-busy", "false");

    const errorBox = doc.createElement("div");
    errorBox.className = "dropdown-error-state";
    errorBox.setAttribute("role", "alert");

    const text = doc.createElement("p");
    text.className = "error-text";
    text.textContent = "Не удалось загрузить закреплённые сообщения";

    const retryBtn = doc.createElement("button");
    retryBtn.className = "btn btn-secondary btn-retry";
    retryBtn.type = "button";
    retryBtn.setAttribute("data-action", "retry-pins");
    retryBtn.textContent = "Повторить попытку";
    retryBtn.addEventListener("click", () => {
      callbacks.onRetryPins?.();
    });

    errorBox.append(text, retryBtn);
    content.append(errorBox);
  } else if (state === "populated") {
    content.setAttribute("aria-busy", "false");

    if (!items || items.length === 0) {
      renderPinsState(doc, panel, "empty", [], callbacks);
      return;
    }

    for (const item of items) {
      const card = doc.createElement("div");
      card.className = "pin-item";
      card.setAttribute("data-message-id", item.id);

      const header = doc.createElement("div");
      header.className = "pin-header";

      const avatar = doc.createElement("span");
      avatar.className = "avatar avatar--sm";
      avatar.setAttribute("aria-hidden", "true");
      avatar.textContent = (item.author || "?").slice(0, 2).toUpperCase();

      const author = doc.createElement("span");
      author.className = "pin-author";
      author.textContent = item.author || "Пользователь";

      const time = doc.createElement("span");
      time.className = "pin-time";
      time.textContent = item.timestamp || "";

      header.append(avatar, author, time);

      const body = doc.createElement("div");
      body.className = "pin-text";
      body.textContent = item.content || "";

      const jumpBtn = doc.createElement("button");
      jumpBtn.className = "btn-jump";
      jumpBtn.type = "button";
      jumpBtn.setAttribute("data-action", "jump-to-message");
      jumpBtn.setAttribute("data-message-id", item.id);
      jumpBtn.textContent = "Перейти";
      jumpBtn.addEventListener("click", () => {
        callbacks.onJumpToMessage?.(item.id);
      });

      card.append(header, body, jumpBtn);
      content.append(card);
    }
  }
}

/**
 * Отрисовка списка элементов веток.
 * @param {any} doc
 * @param {any} content
 * @param {ThreadItem[]} threads
 * @param {string} query
 * @param {"active" | "archived"} tab
 * @param {{ onOpenThread?: (id: string) => void }} callbacks
 */
function renderThreadListItems(doc, content, threads, query, tab, callbacks) {
  if (typeof content.replaceChildren === "function") {
    content.replaceChildren();
  }

  const normalizedQuery = query.trim().toLowerCase();
  const isArchive = tab === "archived";

  const filtered = threads.filter((t) => {
    const itemArchived = Boolean(t.archived);
    if (itemArchived !== isArchive) return false;
    if (!normalizedQuery) return true;
    return (t.name || "").toLowerCase().includes(normalizedQuery);
  });

  if (filtered.length === 0) {
    const emptyBox = doc.createElement("div");
    emptyBox.className = "dropdown-empty-state";

    const icon = doc.createElement("span");
    icon.className = "empty-icon";
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = "🧵";

    const title = doc.createElement("p");
    title.className = "empty-title";
    title.textContent = normalizedQuery
      ? "Ничего не найдено"
      : isArchive
        ? "В этом канале нет архивных веток"
        : "В этом канале нет активных веток";

    const desc = doc.createElement("p");
    desc.className = "empty-desc";
    desc.textContent = normalizedQuery
      ? "Попробуйте изменить поисковый запрос."
      : "Создайте ветку из любого сообщения для отдельного обсуждения.";

    emptyBox.append(icon, title, desc);
    content.append(emptyBox);
    return;
  }

  for (const t of filtered) {
    const item = doc.createElement("div");
    item.className = "thread-item";
    item.setAttribute("role", "button");
    item.setAttribute("tabindex", "0");
    item.setAttribute("data-thread-id", t.id);

    const title = doc.createElement("div");
    title.className = "thread-title";
    title.textContent = t.name;

    const meta = doc.createElement("div");
    meta.className = "thread-meta";

    const replies = doc.createElement("span");
    replies.className = "thread-replies";
    replies.textContent = `💬 ${t.replyCount ?? 0} ответов`;

    const sep = doc.createTextNode(" · ");

    const lastMsg = doc.createElement("span");
    lastMsg.className = "thread-last-msg";
    lastMsg.textContent = `посл. ${t.lastMessageTime || "недавно"}`;

    meta.append(replies, sep, lastMsg);
    item.append(title, meta);

    const activate = () => {
      callbacks.onOpenThread?.(t.id);
    };

    item.addEventListener("click", activate);
    item.addEventListener("keydown", (/** @type {any} */ event) => {
      const key = "key" in event ? String(event.key) : "";
      if (key === "Enter" || key === " ") {
        if (typeof event.preventDefault === "function") event.preventDefault();
        activate();
      }
    });

    content.append(item);
  }
}

/**
 * Отрисовка состояния панели веток (loading, empty, error, populated).
 * @param {any} doc
 * @param {any} panel
 * @param {"loading" | "empty" | "error" | "populated"} state
 * @param {ThreadItem[]} [items]
 * @param {string} [query]
 * @param {"active" | "archived"} [tab]
 * @param {{ onRetryThreads?: () => void, onOpenThread?: (id: string) => void }} [callbacks]
 */
export function renderThreadsState(
  doc,
  panel,
  state,
  items = [],
  query = "",
  tab = "active",
  callbacks = {},
) {
  panel.setAttribute("data-state", state);
  const content = panel.querySelector(".threads-content");
  if (!content) return;

  if (typeof content.replaceChildren === "function") {
    content.replaceChildren();
  }

  if (state === "loading") {
    content.setAttribute("aria-busy", "true");

    const sr = doc.createElement("span");
    sr.className = "sr-only";
    sr.setAttribute("role", "status");
    sr.textContent = "Загрузка веток…";
    content.append(sr);

    for (let i = 0; i < 4; i++) {
      const skeleton = doc.createElement("div");
      skeleton.className = "thread-skeleton";
      content.append(skeleton);
    }
  } else if (state === "empty") {
    content.setAttribute("aria-busy", "false");

    const emptyBox = doc.createElement("div");
    emptyBox.className = "dropdown-empty-state";

    const icon = doc.createElement("span");
    icon.className = "empty-icon";
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = "🧵";

    const title = doc.createElement("p");
    title.className = "empty-title";
    title.textContent = "В этом канале нет активных веток";

    const desc = doc.createElement("p");
    desc.className = "empty-desc";
    desc.textContent = "Создайте ветку из любого сообщения для отдельного обсуждения.";

    emptyBox.append(icon, title, desc);
    content.append(emptyBox);
  } else if (state === "error") {
    content.setAttribute("aria-busy", "false");

    const errorBox = doc.createElement("div");
    errorBox.className = "dropdown-error-state";
    errorBox.setAttribute("role", "alert");

    const text = doc.createElement("p");
    text.className = "error-text";
    text.textContent = "Не удалось загрузить список веток";

    const retryBtn = doc.createElement("button");
    retryBtn.className = "btn btn-secondary btn-retry";
    retryBtn.type = "button";
    retryBtn.setAttribute("data-action", "retry-threads");
    retryBtn.textContent = "Повторить попытку";
    retryBtn.addEventListener("click", () => {
      callbacks.onRetryThreads?.();
    });

    errorBox.append(text, retryBtn);
    content.append(errorBox);
  } else if (state === "populated") {
    content.setAttribute("aria-busy", "false");
    renderThreadListItems(doc, content, items, query, tab, callbacks);
  }
}

/**
 * Создаёт контроллер управления выпадающими панелями пинов (M-10) и веток (M-11).
 *
 * @param {PinsThreadsOptions} [options]
 * @returns {PinsThreadsController}
 */
export function createPinsThreadsPanels(options = {}) {
  const doc = options.document ?? globalThis.document;
  if (!doc) throw new Error("createPinsThreadsPanels: document не определён");

  // Поиск или прогрессивное создание панелей
  const pinsPanel = options.pinsPanel ?? ensurePinsPanel(doc, options.container);
  const threadsPanel = options.threadsPanel ?? ensureThreadsPanel(doc, options.container);

  // Кнопки-триггеры
  const pinsBtn =
    typeof options.pinsButton === "string"
      ? doc.querySelector(options.pinsButton)
      : options.pinsButton ?? doc.querySelector('[data-action="pins"]');

  const threadsBtn =
    typeof options.threadsButton === "string"
      ? doc.querySelector(options.threadsButton)
      : options.threadsButton ?? doc.querySelector('[data-action="threads"]');

  // Выставление начальных атрибутов доступности на кнопки
  if (pinsBtn && typeof pinsBtn.setAttribute === "function") {
    pinsBtn.setAttribute("aria-haspopup", "dialog");
    pinsBtn.setAttribute("aria-expanded", "false");
    pinsBtn.setAttribute("aria-controls", "chat-pins-panel");
  }

  if (threadsBtn && typeof threadsBtn.setAttribute === "function") {
    threadsBtn.setAttribute("aria-haspopup", "dialog");
    threadsBtn.setAttribute("aria-expanded", "false");
    threadsBtn.setAttribute("aria-controls", "chat-threads-panel");
  }

  /** @type {PinItem[]} */
  let pinsData = [];
  /** @type {"loading" | "empty" | "error" | "populated"} */
  let pinsCurrentState = "empty";

  /** @type {ThreadItem[]} */
  let threadsData = [];
  /** @type {"loading" | "empty" | "error" | "populated"} */
  let threadsCurrentState = "empty";
  /** @type {"active" | "archived"} */
  let activeThreadsTab = "active";
  /** @type {string} */
  let threadsSearchQuery = "";

  const renderPins = () => {
    renderPinsState(doc, pinsPanel, pinsCurrentState, pinsData, {
      onRetryPins: () => {
        options.onRetryPins?.();
        if (typeof options.loadPins === "function") {
          controller.setPinsState("loading");
          Promise.resolve(options.loadPins())
            .then((res) => controller.setPinsState("populated", res))
            .catch(() => controller.setPinsState("error"));
        }
      },
      onJumpToMessage: (id) => {
        options.onJumpToMessage?.(id);
      },
    });
  };

  const renderThreads = () => {
    renderThreadsState(
      doc,
      threadsPanel,
      threadsCurrentState,
      threadsData,
      threadsSearchQuery,
      activeThreadsTab,
      {
        onRetryThreads: () => {
          options.onRetryThreads?.();
          if (typeof options.loadThreads === "function") {
            controller.setThreadsState("loading");
            Promise.resolve(options.loadThreads())
              .then((res) => controller.setThreadsState("populated", res))
              .catch(() => controller.setThreadsState("error"));
          }
        },
        onOpenThread: (id) => {
          options.onOpenThread?.(id);
          if (options.router && typeof options.router.openThread === "function") {
            options.router.openThread(id);
          }
        },
      },
    );
  };

  /** @param {{ restoreFocus?: boolean }} [opts] */
  const closePins = (opts = {}) => {
    const isAlreadyHidden = pinsPanel.hasAttribute("hidden");
    if (!isAlreadyHidden) {
      pinsPanel.setAttribute("hidden", "");
    }
    if (pinsBtn && typeof pinsBtn.setAttribute === "function") {
      pinsBtn.setAttribute("aria-expanded", "false");
    }
    if (opts.restoreFocus && pinsBtn && typeof pinsBtn.focus === "function") {
      pinsBtn.focus();
    }
  };

  /** @param {{ restoreFocus?: boolean }} [opts] */
  const closeThreads = (opts = {}) => {
    const isAlreadyHidden = threadsPanel.hasAttribute("hidden");
    if (!isAlreadyHidden) {
      threadsPanel.setAttribute("hidden", "");
    }
    if (threadsBtn && typeof threadsBtn.setAttribute === "function") {
      threadsBtn.setAttribute("aria-expanded", "false");
    }
    if (opts.restoreFocus && threadsBtn && typeof threadsBtn.focus === "function") {
      threadsBtn.focus();
    }
  };

  const openPins = () => {
    // Взаимное исключение: закрываем панель веток
    closeThreads({ restoreFocus: false });

    pinsPanel.removeAttribute("hidden");
    if (pinsBtn && typeof pinsBtn.setAttribute === "function") {
      pinsBtn.setAttribute("aria-expanded", "true");
    }

    renderPins();

    // Перемещение фокуса внутрь панели
    const closeBtn = pinsPanel.querySelector('[data-action="close-pins"]');
    if (closeBtn && typeof closeBtn.focus === "function") {
      closeBtn.focus();
    } else if (typeof pinsPanel.focus === "function") {
      pinsPanel.focus();
    }
  };

  const openThreads = () => {
    // Взаимное исключение: закрываем панель пинов
    closePins({ restoreFocus: false });

    threadsPanel.removeAttribute("hidden");
    if (threadsBtn && typeof threadsBtn.setAttribute === "function") {
      threadsBtn.setAttribute("aria-expanded", "true");
    }

    renderThreads();

    // Перемещение фокуса внутрь панели
    const activeTab = threadsPanel.querySelector('[role="tab"][aria-selected="true"]');
    const searchInput = threadsPanel.querySelector(".threads-search-input");
    const closeBtn = threadsPanel.querySelector('[data-action="close-threads"]');

    if (activeTab && typeof activeTab.focus === "function") {
      activeTab.focus();
    } else if (searchInput && typeof searchInput.focus === "function") {
      searchInput.focus();
    } else if (closeBtn && typeof closeBtn.focus === "function") {
      closeBtn.focus();
    } else if (typeof threadsPanel.focus === "function") {
      threadsPanel.focus();
    }
  };

  const togglePins = () => {
    if (pinsPanel.hasAttribute("hidden")) {
      openPins();
    } else {
      closePins({ restoreFocus: true });
    }
  };

  const toggleThreads = () => {
    if (threadsPanel.hasAttribute("hidden")) {
      openThreads();
    } else {
      closeThreads({ restoreFocus: true });
    }
  };

  /** @param {{ restoreFocus?: boolean }} [opts] */
  const closeAll = (opts = {}) => {
    closePins(opts);
    closeThreads(opts);
  };

  // Обработчики кнопок закрытия внутри панелей
  const closePinsBtn = pinsPanel.querySelector('[data-action="close-pins"]');
  const onPinsCloseClick = () => closePins({ restoreFocus: true });
  if (closePinsBtn && typeof closePinsBtn.addEventListener === "function") {
    closePinsBtn.addEventListener("click", onPinsCloseClick);
  }

  const closeThreadsBtn = threadsPanel.querySelector('[data-action="close-threads"]');
  const onThreadsCloseClick = () => closeThreads({ restoreFocus: true });
  if (closeThreadsBtn && typeof closeThreadsBtn.addEventListener === "function") {
    closeThreadsBtn.addEventListener("click", onThreadsCloseClick);
  }

  // Обработчики кликов по кнопкам в хедере
  const onPinsBtnClick = () => togglePins();
  if (pinsBtn && typeof pinsBtn.addEventListener === "function") {
    pinsBtn.addEventListener("click", onPinsBtnClick);
  }

  const onThreadsBtnClick = () => toggleThreads();
  if (threadsBtn && typeof threadsBtn.addEventListener === "function") {
    threadsBtn.addEventListener("click", onThreadsBtnClick);
  }

  // Табы веток
  const tabActive = threadsPanel.querySelector("#tab-active-threads");
  const tabArchived = threadsPanel.querySelector("#tab-archived-threads");

  const onActiveTabClick = () => {
    controller.setActiveTab("active");
  };
  const onArchivedTabClick = () => {
    controller.setActiveTab("archived");
  };

  if (tabActive && typeof tabActive.addEventListener === "function") {
    tabActive.addEventListener("click", onActiveTabClick);
  }
  if (tabArchived && typeof tabArchived.addEventListener === "function") {
    tabArchived.addEventListener("click", onArchivedTabClick);
  }

  // Поиск веток
  const searchInput = threadsPanel.querySelector(".threads-search-input");
  /** @param {any} event */
  const onSearchInput = (event) => {
    const val = event?.target?.value ?? searchInput.value ?? "";
    controller.searchThreads(String(val));
  };
  if (searchInput && typeof searchInput.addEventListener === "function") {
    searchInput.addEventListener("input", onSearchInput);
  }

  // Глобальные события Escape и Outside Click
  /** @param {any} event */
  const onGlobalKeyDown = (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (key === "Escape") {
      if (!pinsPanel.hasAttribute("hidden")) {
        if (typeof event.preventDefault === "function") event.preventDefault();
        closePins({ restoreFocus: true });
      } else if (!threadsPanel.hasAttribute("hidden")) {
        if (typeof event.preventDefault === "function") event.preventDefault();
        closeThreads({ restoreFocus: true });
      }
    }
  };

  /** @param {any} event */
  const onGlobalClick = (event) => {
    const target = event?.target ?? null;
    if (!target) return;

    if (!pinsPanel.hasAttribute("hidden")) {
      if (!isInside(pinsPanel, target) && target !== pinsBtn && !isInside(pinsBtn, target)) {
        closePins({ restoreFocus: true });
      }
    }

    if (!threadsPanel.hasAttribute("hidden")) {
      if (!isInside(threadsPanel, target) && target !== threadsBtn && !isInside(threadsBtn, target)) {
        closeThreads({ restoreFocus: true });
      }
    }
  };

  const listenTarget = doc.body ?? doc;
  if (listenTarget && typeof listenTarget.addEventListener === "function") {
    listenTarget.addEventListener("keydown", onGlobalKeyDown);
    listenTarget.addEventListener("click", onGlobalClick);
  }

  /** @type {PinsThreadsController} */
  const controller = {
    pinsPanel,
    threadsPanel,
    pinsButton: pinsBtn ?? null,
    threadsButton: threadsBtn ?? null,
    openPins,
    closePins,
    togglePins,
    openThreads,
    closeThreads,
    toggleThreads,
    closeAll,
    setPinsState(state, data = []) {
      pinsCurrentState = state;
      pinsData = Array.isArray(data) ? data : [];
      renderPins();
    },
    setThreadsState(state, data = []) {
      threadsCurrentState = state;
      threadsData = Array.isArray(data) ? data : [];
      renderThreads();
    },
    searchThreads(query) {
      threadsSearchQuery = query;
      renderThreads();
    },
    setActiveTab(tabId) {
      activeThreadsTab = tabId;
      if (tabActive && typeof tabActive.setAttribute === "function") {
        tabActive.setAttribute("aria-selected", tabId === "active" ? "true" : "false");
        tabActive.setAttribute("tabindex", tabId === "active" ? "0" : "-1");
      }
      if (tabArchived && typeof tabArchived.setAttribute === "function") {
        tabArchived.setAttribute("aria-selected", tabId === "archived" ? "true" : "false");
        tabArchived.setAttribute("tabindex", tabId === "archived" ? "0" : "-1");
      }
      renderThreads();
    },
    destroy() {
      if (pinsBtn && typeof pinsBtn.removeEventListener === "function") {
        pinsBtn.removeEventListener("click", onPinsBtnClick);
      }
      if (threadsBtn && typeof threadsBtn.removeEventListener === "function") {
        threadsBtn.removeEventListener("click", onThreadsBtnClick);
      }
      if (closePinsBtn && typeof closePinsBtn.removeEventListener === "function") {
        closePinsBtn.removeEventListener("click", onPinsCloseClick);
      }
      if (closeThreadsBtn && typeof closeThreadsBtn.removeEventListener === "function") {
        closeThreadsBtn.removeEventListener("click", onThreadsCloseClick);
      }
      if (tabActive && typeof tabActive.removeEventListener === "function") {
        tabActive.removeEventListener("click", onActiveTabClick);
      }
      if (tabArchived && typeof tabArchived.removeEventListener === "function") {
        tabArchived.removeEventListener("click", onArchivedTabClick);
      }
      if (searchInput && typeof searchInput.removeEventListener === "function") {
        searchInput.removeEventListener("input", onSearchInput);
      }
      if (listenTarget && typeof listenTarget.removeEventListener === "function") {
        listenTarget.removeEventListener("keydown", onGlobalKeyDown);
        listenTarget.removeEventListener("click", onGlobalClick);
      }
    },
  };

  return controller;
}

/* =============================================================================
 * FE-P0-06 (task-152, RW-40): createPinsThreadsController и mountPinsThreadsDom
 * ========================================================================== */

export const THREAD_NAME_MIN_LEN = 1;
export const THREAD_NAME_MAX_LEN = 100;
export const THREAD_ARCHIVE_DURATIONS = Object.freeze([60, 1440, 4320, 10080]);
export const DEFAULT_THREAD_ARCHIVE_DURATION = 1440;

/**
 * Извлекает отображаемое имя автора из строки или объекта пользователя.
 * @param {any} author
 * @returns {string}
 */
function resolvePinAuthorName(author) {
  if (typeof author === "string" && author.trim()) return author.trim();
  if (author && typeof author === "object") {
    const name = author.globalName ?? author.global_name ?? author.username ?? author.name;
    if (typeof name === "string" && name.trim()) return name.trim();
  }
  return "Пользователь";
}

/**
 * @typedef {{
 *   channelId?: string,
 *   pins?: PinItem[],
 *   threads?: ThreadItem[],
 *   activeThreads?: ThreadItem[],
 *   archivedThreads?: ThreadItem[],
 *   loadPins?: () => Promise<PinItem[]> | PinItem[],
 *   loadThreads?: (opts?: { archived?: boolean }) => Promise<ThreadItem[]> | ThreadItem[],
 *   unpinMessage?: (messageId: string, ctx?: { channelId: string }) => Promise<any> | any,
 *   pinMessage?: (message: PinItem) => Promise<any> | any,
 *   createThread?: (payload: { name: string, auto_archive_duration: number, autoArchiveDuration: number, channelId: string }) => Promise<any> | any,
 *   onJumpToMessage?: (messageId: string) => void,
 *   jumpToMessage?: (messageId: string) => void,
 *   onOpenThread?: (threadId: string) => void,
 *   openThread?: (threadId: string) => void,
 *   confirmUnpin?: (messageId: string) => Promise<boolean> | boolean,
 * }} PinsThreadsControllerOptions
 */

/**
 * Создаёт контроллер состояния закреплённых сообщений и веток канала (FE-P0-06).
 * @param {PinsThreadsControllerOptions} [options]
 */
export function createPinsThreadsController(options = {}) {
  const channelId = options.channelId ?? "general";
  /** @type {PinItem[]} */
  let pins = Array.isArray(options.pins) ? options.pins.map((p) => ({ ...p })) : [];

  const allInitialThreads = Array.isArray(options.threads) ? options.threads : [];
  /** @type {ThreadItem[]} */
  let activeThreads = Array.isArray(options.activeThreads)
    ? options.activeThreads.map((t) => ({ ...t, archived: false }))
    : allInitialThreads.filter((t) => !t.archived).map((t) => ({ ...t, archived: false }));
  /** @type {ThreadItem[]} */
  let archivedThreads = Array.isArray(options.archivedThreads)
    ? options.archivedThreads.map((t) => ({ ...t, archived: true }))
    : allInitialThreads.filter((t) => Boolean(t.archived)).map((t) => ({ ...t, archived: true }));

  let pinsOpen = false;
  let threadsOpen = false;
  /** @type {"active" | "archived"} */
  let activeTab = "active";
  let searchQuery = "";
  /** @type {string | null} */
  let activeThreadId = null;
  /** @type {string | null} */
  let pendingUnpinId = null;
  let destroyed = false;

  /** @type {Set<(state: any) => void>} */
  const subscribers = new Set();

  const getState = () => ({
    channelId,
    pins: [...pins],
    activeThreads: [...activeThreads],
    archivedThreads: [...archivedThreads],
    pinsOpen,
    threadsOpen,
    activeTab,
    searchQuery,
    activeThreadId,
    pendingUnpinId,
    destroyed,
  });

  const notify = () => {
    if (destroyed) return;
    const snapshot = getState();
    for (const fn of [...subscribers]) {
      fn(snapshot);
    }
  };

  /**
   * @param {(state: ReturnType<typeof getState>) => void} listener
   * @returns {() => void}
   */
  const subscribe = (listener) => {
    subscribers.add(listener);
    return () => {
      subscribers.delete(listener);
    };
  };

  const openPins = () => {
    if (destroyed) return;
    threadsOpen = false;
    pinsOpen = true;
    notify();
  };

  const closePins = () => {
    if (destroyed) return;
    pinsOpen = false;
    pendingUnpinId = null;
    notify();
  };

  const togglePins = () => {
    if (pinsOpen) closePins();
    else openPins();
  };

  const openThreads = () => {
    if (destroyed) return;
    pinsOpen = false;
    pendingUnpinId = null;
    threadsOpen = true;
    notify();
  };

  const closeThreads = () => {
    if (destroyed) return;
    threadsOpen = false;
    notify();
  };

  const toggleThreads = () => {
    if (threadsOpen) closeThreads();
    else openThreads();
  };

  /** @param {"active" | "archived"} tab */
  const setActiveTab = (tab) => {
    if (destroyed) return;
    activeTab = tab === "archived" ? "archived" : "active";
    notify();
  };

  /** @param {string} query */
  const setSearchQuery = (query) => {
    if (destroyed) return;
    searchQuery = String(query ?? "");
    notify();
  };

  /** @param {string} messageId */
  const requestUnpin = (messageId) => {
    if (destroyed) return;
    pendingUnpinId = String(messageId);
    notify();
  };

  const cancelUnpin = () => {
    if (destroyed) return;
    pendingUnpinId = null;
    notify();
  };

  /** @param {string} messageId */
  const unpinMessage = async (messageId) => {
    if (destroyed) return;
    const id = String(messageId);
    if (typeof options.unpinMessage === "function") {
      await options.unpinMessage(id, { channelId });
    }
    pins = pins.filter((item) => item.id !== id);
    if (pendingUnpinId === id) {
      pendingUnpinId = null;
    }
    notify();
  };

  /** @param {string} messageId */
  const jumpToMessage = (messageId) => {
    if (destroyed) return;
    const id = String(messageId);
    options.onJumpToMessage?.(id);
    options.jumpToMessage?.(id);
  };

  /** @param {string} threadId */
  const openThread = (threadId) => {
    if (destroyed) return;
    const id = String(threadId);
    activeThreadId = id;
    options.onOpenThread?.(id);
    options.openThread?.(id);
    notify();
  };

  const closeThreadView = () => {
    if (destroyed) return;
    activeThreadId = null;
    notify();
  };

  /**
   * Создаёт новую ветку с валидацией названия (1..100 символов) и auto_archive_duration.
   * @param {{ name: string, auto_archive_duration?: number, autoArchiveDuration?: number }} payload
   */
  const createThread = async (payload) => {
    if (destroyed) return null;
    const trimmed = String(payload?.name ?? "").trim();
    if (trimmed.length < THREAD_NAME_MIN_LEN || trimmed.length > THREAD_NAME_MAX_LEN) {
      throw new Error("Название ветки должно содержать от 1 до 100 символов");
    }
    const rawDuration = Number(
      payload?.auto_archive_duration ?? payload?.autoArchiveDuration ?? DEFAULT_THREAD_ARCHIVE_DURATION,
    );
    const auto_archive_duration = THREAD_ARCHIVE_DURATIONS.includes(rawDuration)
      ? rawDuration
      : DEFAULT_THREAD_ARCHIVE_DURATION;

    /** @type {any} */
    let createdResult = null;
    if (typeof options.createThread === "function") {
      createdResult = await options.createThread({
        name: trimmed,
        auto_archive_duration,
        autoArchiveDuration: auto_archive_duration,
        channelId,
      });
    }

    /** @type {ThreadItem} */
    const createdThread = {
      id: String(createdResult?.id ?? `thread-${activeThreads.length + archivedThreads.length + 1}`),
      name: String(createdResult?.name ?? trimmed),
      replyCount: Number(createdResult?.replyCount ?? createdResult?.message_count ?? 0),
      lastMessageTime: String(createdResult?.lastMessageTime ?? "только что"),
      archived: false,
    };

    activeThreads = [createdThread, ...activeThreads.filter((t) => t.id !== createdThread.id)];
    activeTab = "active";
    activeThreadId = createdThread.id;
    options.onOpenThread?.(createdThread.id);
    options.openThread?.(createdThread.id);
    notify();
    return createdThread;
  };

  return {
    getState,
    subscribe,
    openPins,
    closePins,
    togglePins,
    openThreads,
    closeThreads,
    toggleThreads,
    setActiveTab,
    setSearchQuery,
    requestUnpin,
    cancelUnpin,
    unpinMessage,
    jumpToMessage,
    openThread,
    closeThreadView,
    createThread,
    confirmUnpin: options.confirmUnpin,
    isDestroyed: () => destroyed,
    destroy() {
      destroyed = true;
      subscribers.clear();
    },
  };
}

/**
 * Монтирует DOM-адаптер поповера закреплённых сообщений (#pins-popover) и
 * боковой панели веток (#threads-sidebar / #thread-view-panel) в шапку и рабочую область канала.
 *
 * @param {any} rootEl
 * @param {ReturnType<typeof createPinsThreadsController> | PinsThreadsControllerOptions} [controllerOrOpts]
 */
export function mountPinsThreadsDom(rootEl, controllerOrOpts = {}) {
  if (!rootEl) {
    throw new Error("mountPinsThreadsDom: rootEl обязателен");
  }

  const doc =
    rootEl.ownerDocument ??
    (typeof rootEl.createElement === "function" ? rootEl : globalThis.document);
  if (!doc) {
    throw new Error("mountPinsThreadsDom: document не найден");
  }

  const container =
    typeof rootEl.querySelector === "function" && rootEl.tagName
      ? rootEl
      : doc.body ?? doc.documentElement;

  const isControllerInstance =
    controllerOrOpts &&
    "getState" in controllerOrOpts &&
    typeof controllerOrOpts.getState === "function" &&
    "subscribe" in controllerOrOpts &&
    typeof controllerOrOpts.subscribe === "function";

  const controller = isControllerInstance
    ? /** @type {ReturnType<typeof createPinsThreadsController>} */ (controllerOrOpts)
    : createPinsThreadsController(controllerOrOpts);

  /** @type {Array<() => void>} */
  const cleanups = [];
  let isAdapterDestroyed = false;

  /**
   * @param {any} target
   * @param {string} type
   * @param {(event: any) => void} handler
   */
  const listen = (target, type, handler) => {
    if (!target || typeof target.addEventListener !== "function") return;
    target.addEventListener(type, handler);
    cleanups.push(() => {
      if (typeof target.removeEventListener === "function") {
        target.removeEventListener(type, handler);
      }
    });
  };

  // 1. Поиск или динамическое создание кнопок шапки (#btn-TogglePins и #btn-ToggleThreads)
  const headerHost =
    container.querySelector(".chat-actions") ??
    container.querySelector("#chat-header") ??
    container;

  let btnPins =
    container.querySelector("#btn-TogglePins") ?? doc.getElementById("btn-TogglePins");
  if (!btnPins) {
    btnPins = doc.createElement("button");
    btnPins.id = "btn-TogglePins";
    btnPins.type = "button";
    btnPins.className = "icon-btn";
    btnPins.setAttribute("data-action", "pins");
    btnPins.setAttribute("aria-label", "Закреплённые сообщения");
    btnPins.textContent = "📌";
    headerHost.append(btnPins);
  }
  btnPins.setAttribute("aria-haspopup", "dialog");
  btnPins.setAttribute("aria-expanded", "false");
  btnPins.setAttribute("aria-controls", "pins-popover");

  let btnThreads =
    container.querySelector("#btn-ToggleThreads") ?? doc.getElementById("btn-ToggleThreads");
  if (!btnThreads) {
    btnThreads = doc.createElement("button");
    btnThreads.id = "btn-ToggleThreads";
    btnThreads.type = "button";
    btnThreads.className = "icon-btn";
    btnThreads.setAttribute("data-action", "threads");
    btnThreads.setAttribute("aria-label", "Ветки");
    btnThreads.textContent = "🧵";
    headerHost.append(btnThreads);
  }
  btnThreads.setAttribute("aria-haspopup", "dialog");
  btnThreads.setAttribute("aria-expanded", "false");
  btnThreads.setAttribute("aria-controls", "threads-sidebar");

  // 2. Поиск или создание #pins-popover (role='dialog')
  let pinsPopover =
    container.querySelector("#pins-popover") ?? doc.getElementById("pins-popover");
  if (!pinsPopover) {
    pinsPopover = doc.createElement("section");
    pinsPopover.id = "pins-popover";
    pinsPopover.className = "header-dropdown-panel pins-panel pins-popover";
    pinsPopover.setAttribute("role", "dialog");
    pinsPopover.setAttribute("aria-label", "Закреплённые сообщения");
    pinsPopover.setAttribute("aria-modal", "false");
    pinsPopover.setAttribute("tabindex", "-1");
    pinsPopover.setAttribute("hidden", "");

    const header = doc.createElement("header");
    header.className = "dropdown-header";

    const title = doc.createElement("h2");
    title.className = "dropdown-title";
    title.textContent = "Закреплённые сообщения";

    const closeBtn = doc.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon-btn dropdown-close";
    closeBtn.setAttribute("data-action", "close-pins");
    closeBtn.setAttribute("aria-label", "Закрыть");
    closeBtn.textContent = "✕";

    header.append(title, closeBtn);

    const content = doc.createElement("div");
    content.className = "dropdown-content pins-content";
    content.setAttribute("role", "feed");
    content.setAttribute("aria-busy", "false");

    pinsPopover.append(header, content);
    container.append(pinsPopover);
  } else {
    pinsPopover.setAttribute("role", "dialog");
  }

  // 3. Поиск или создание #threads-sidebar
  let threadsSidebar =
    container.querySelector("#threads-sidebar") ?? doc.getElementById("threads-sidebar");
  if (!threadsSidebar) {
    threadsSidebar = doc.createElement("aside");
    threadsSidebar.id = "threads-sidebar";
    threadsSidebar.className = "header-dropdown-panel threads-panel threads-sidebar";
    threadsSidebar.setAttribute("role", "dialog");
    threadsSidebar.setAttribute("aria-label", "Ветки");
    threadsSidebar.setAttribute("aria-modal", "false");
    threadsSidebar.setAttribute("tabindex", "-1");
    threadsSidebar.setAttribute("hidden", "");

    const header = doc.createElement("header");
    header.className = "dropdown-header";

    const tabs = doc.createElement("div");
    tabs.className = "threads-header-tabs";
    tabs.setAttribute("role", "tablist");

    const tabActive = doc.createElement("button");
    tabActive.id = "tab-active-threads";
    tabActive.type = "button";
    tabActive.setAttribute("role", "tab");
    tabActive.setAttribute("data-tab", "active");
    tabActive.setAttribute("aria-selected", "true");
    tabActive.setAttribute("aria-controls", "threads-list");
    tabActive.setAttribute("tabindex", "0");
    tabActive.textContent = "Активные";

    const tabArchived = doc.createElement("button");
    tabArchived.id = "tab-archived-threads";
    tabArchived.type = "button";
    tabArchived.setAttribute("role", "tab");
    tabArchived.setAttribute("data-tab", "archived");
    tabArchived.setAttribute("aria-selected", "false");
    tabArchived.setAttribute("aria-controls", "threads-list");
    tabArchived.setAttribute("tabindex", "-1");
    tabArchived.textContent = "Архивные";

    tabs.append(tabActive, tabArchived);

    const closeBtn = doc.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon-btn dropdown-close";
    closeBtn.setAttribute("data-action", "close-threads");
    closeBtn.setAttribute("aria-label", "Закрыть");
    closeBtn.textContent = "✕";

    header.append(tabs, closeBtn);

    // Поле поиска веток
    const searchWrap = doc.createElement("div");
    searchWrap.className = "threads-search-wrap";
    const searchInput = doc.createElement("input");
    searchInput.type = "search";
    searchInput.className = "threads-search-input";
    searchInput.placeholder = "Поиск веток…";
    searchInput.setAttribute("aria-label", "Поиск веток");
    searchWrap.append(searchInput);

    // Форма создания новой ветки (название 1..100 символов + auto_archive_duration)
    const createForm = doc.createElement("form");
    createForm.id = "create-thread-form";
    createForm.className = "create-thread-form";

    const nameInput = doc.createElement("input");
    nameInput.id = "thread-name-input";
    nameInput.type = "text";
    nameInput.className = "thread-name-input";
    nameInput.setAttribute("name", "thread-name");
    nameInput.setAttribute("maxlength", String(THREAD_NAME_MAX_LEN));
    nameInput.setAttribute("aria-label", "Название ветки");
    nameInput.setAttribute("aria-invalid", "false");
    nameInput.placeholder = "Название новой ветки (1–100 символов)";

    const durationSelect = doc.createElement("select");
    durationSelect.id = "thread-archive-duration";
    durationSelect.className = "thread-archive-duration";
    durationSelect.setAttribute("name", "auto_archive_duration");
    durationSelect.setAttribute("aria-label", "Автоархивация");
    for (const [val, label] of [
      ["60", "1 час"],
      ["1440", "24 часа"],
      ["4320", "3 дня"],
      ["10080", "1 неделя"],
    ]) {
      const opt = doc.createElement("option");
      opt.value = val;
      opt.textContent = label;
      durationSelect.append(opt);
    }
    durationSelect.value = String(DEFAULT_THREAD_ARCHIVE_DURATION);

    const createBtn = doc.createElement("button");
    createBtn.id = "btn-create-thread";
    createBtn.type = "button";
    createBtn.className = "btn btn-primary btn-create-thread";
    createBtn.setAttribute("data-action", "create-thread");
    createBtn.textContent = "Создать ветку";

    const formError = doc.createElement("p");
    formError.id = "thread-form-error";
    formError.className = "thread-form-error";
    formError.setAttribute("role", "alert");
    formError.setAttribute("hidden", "");

    createForm.append(nameInput, durationSelect, createBtn, formError);

    const list = doc.createElement("div");
    list.id = "threads-list";
    list.className = "dropdown-content threads-content";
    list.setAttribute("role", "region");
    list.setAttribute("aria-busy", "false");

    threadsSidebar.append(header, searchWrap, createForm, list);
    container.append(threadsSidebar);
  }

  // 4. Поиск или создание панели просмотра ветки (#thread-view-panel)
  let threadViewPanel =
    container.querySelector("#thread-view-panel") ?? doc.getElementById("thread-view-panel");
  if (!threadViewPanel) {
    threadViewPanel = doc.createElement("section");
    threadViewPanel.id = "thread-view-panel";
    threadViewPanel.className = "thread-view-panel";
    threadViewPanel.setAttribute("role", "region");
    threadViewPanel.setAttribute("aria-label", "Просмотр ветки");
    threadViewPanel.setAttribute("hidden", "");
    container.append(threadViewPanel);
  }

  const renderPinsPopover = () => {
    const st = controller.getState();
    if (st.pinsOpen) {
      pinsPopover.removeAttribute("hidden");
      btnPins.setAttribute("aria-expanded", "true");
    } else {
      pinsPopover.setAttribute("hidden", "");
      btnPins.setAttribute("aria-expanded", "false");
    }

    const content = pinsPopover.querySelector(".pins-content") ?? pinsPopover;
    if (typeof content.replaceChildren === "function") {
      content.replaceChildren();
    }

    if (st.pins.length === 0) {
      pinsPopover.setAttribute("data-state", "empty");
      const emptyBox = doc.createElement("div");
      emptyBox.className = "dropdown-empty-state pins-empty";

      const icon = doc.createElement("span");
      icon.className = "empty-icon";
      icon.setAttribute("aria-hidden", "true");
      icon.textContent = "📌";

      const title = doc.createElement("p");
      title.className = "empty-title";
      title.textContent = "В этом канале нет закреплённых сообщений";

      emptyBox.append(icon, title);
      content.append(emptyBox);
      return;
    }

    pinsPopover.setAttribute("data-state", "populated");
    for (const item of st.pins) {
      const card = doc.createElement("div");
      card.className = "pin-item";
      card.setAttribute("data-message-id", item.id);

      const header = doc.createElement("div");
      header.className = "pin-header";

      const authorName = resolvePinAuthorName(item.author);
      const author = doc.createElement("span");
      author.className = "pin-author";
      author.textContent = authorName;

      const time = doc.createElement("span");
      time.className = "pin-time";
      time.textContent = item.timestamp || "";

      header.append(author, time);

      const body = doc.createElement("div");
      body.className = "pin-text";
      body.textContent = item.content || "";

      const actionsRow = doc.createElement("div");
      actionsRow.className = "pin-actions";

      const jumpBtn = doc.createElement("button");
      jumpBtn.type = "button";
      jumpBtn.className = "btn-jump";
      jumpBtn.setAttribute("data-action", "jump-to-message");
      jumpBtn.setAttribute("data-message-id", item.id);
      jumpBtn.textContent = "Перейти";
      jumpBtn.addEventListener("click", () => {
        if (isAdapterDestroyed) return;
        controller.jumpToMessage(item.id);
      });

      const unpinBtn = doc.createElement("button");
      unpinBtn.type = "button";
      unpinBtn.className = "btn-unpin";
      unpinBtn.setAttribute("data-action", "unpin-message");
      unpinBtn.setAttribute("data-message-id", item.id);
      unpinBtn.textContent = "Открепить";
      unpinBtn.addEventListener("click", async () => {
        if (isAdapterDestroyed) return;
        if (typeof controller.confirmUnpin === "function") {
          const confirmed = await controller.confirmUnpin(item.id);
          if (confirmed) {
            await controller.unpinMessage(item.id);
          }
          return;
        }
        controller.requestUnpin(item.id);
      });

      actionsRow.append(jumpBtn, unpinBtn);
      card.append(header, body, actionsRow);

      if (st.pendingUnpinId === item.id) {
        const confirmBox = doc.createElement("div");
        confirmBox.className = "pin-unpin-confirm";
        confirmBox.setAttribute("role", "alertdialog");
        confirmBox.setAttribute("aria-label", "Подтверждение открепления");

        const prompt = doc.createElement("span");
        prompt.className = "pin-unpin-prompt";
        prompt.textContent = "Открепить сообщение?";

        const confirmBtn = doc.createElement("button");
        confirmBtn.type = "button";
        confirmBtn.className = "btn btn-danger btn-confirm-unpin";
        confirmBtn.setAttribute("data-action", "confirm-unpin");
        confirmBtn.setAttribute("data-message-id", item.id);
        confirmBtn.textContent = "Подтвердить";
        confirmBtn.addEventListener("click", () => {
          if (isAdapterDestroyed) return;
          void controller.unpinMessage(item.id);
        });

        const cancelBtn = doc.createElement("button");
        cancelBtn.type = "button";
        cancelBtn.className = "btn btn-secondary btn-cancel-unpin";
        cancelBtn.setAttribute("data-action", "cancel-unpin");
        cancelBtn.setAttribute("data-message-id", item.id);
        cancelBtn.textContent = "Отмена";
        cancelBtn.addEventListener("click", () => {
          if (isAdapterDestroyed) return;
          controller.cancelUnpin();
        });

        confirmBox.append(prompt, confirmBtn, cancelBtn);
        card.append(confirmBox);
      }

      content.append(card);
    }
  };

  const renderThreadsSidebarAndView = () => {
    const st = controller.getState();
    if (st.threadsOpen) {
      threadsSidebar.removeAttribute("hidden");
      btnThreads.setAttribute("aria-expanded", "true");
    } else {
      threadsSidebar.setAttribute("hidden", "");
      btnThreads.setAttribute("aria-expanded", "false");
    }

    const tabActive = threadsSidebar.querySelector("#tab-active-threads");
    const tabArchived = threadsSidebar.querySelector("#tab-archived-threads");
    if (tabActive) {
      tabActive.setAttribute("aria-selected", st.activeTab === "active" ? "true" : "false");
      tabActive.setAttribute("tabindex", st.activeTab === "active" ? "0" : "-1");
    }
    if (tabArchived) {
      tabArchived.setAttribute("aria-selected", st.activeTab === "archived" ? "true" : "false");
      tabArchived.setAttribute("tabindex", st.activeTab === "archived" ? "0" : "-1");
    }

    const listEl = threadsSidebar.querySelector(".threads-content") ?? threadsSidebar;
    const sourceList = st.activeTab === "archived" ? st.archivedThreads : st.activeThreads;
    threadsSidebar.setAttribute("data-state", sourceList.length > 0 ? "populated" : "empty");

    renderThreadListItems(doc, listEl, sourceList, st.searchQuery, st.activeTab, {
      onOpenThread: (id) => {
        if (isAdapterDestroyed) return;
        controller.openThread(id);
      },
    });

    // Отрисовка #thread-view-panel при переходе внутрь ветки
    if (!st.activeThreadId) {
      threadViewPanel.setAttribute("hidden", "");
      threadViewPanel.removeAttribute("data-thread-id");
      if (typeof threadViewPanel.replaceChildren === "function") {
        threadViewPanel.replaceChildren();
      }
      return;
    }

    const allThreads = [...st.activeThreads, ...st.archivedThreads];
    const currentThread = allThreads.find((t) => t.id === st.activeThreadId) ?? {
      id: st.activeThreadId,
      name: st.activeThreadId,
      replyCount: 0,
      archived: false,
    };

    threadViewPanel.removeAttribute("hidden");
    threadViewPanel.setAttribute("data-thread-id", currentThread.id);
    if (typeof threadViewPanel.replaceChildren === "function") {
      threadViewPanel.replaceChildren();
    }

    const viewHeader = doc.createElement("header");
    viewHeader.className = "thread-view-header";

    const viewTitle = doc.createElement("h3");
    viewTitle.className = "thread-view-title";
    viewTitle.textContent = currentThread.name;

    const closeViewBtn = doc.createElement("button");
    closeViewBtn.id = "btn-close-thread-view";
    closeViewBtn.type = "button";
    closeViewBtn.className = "icon-btn thread-view-close";
    closeViewBtn.setAttribute("data-action", "close-thread-view");
    closeViewBtn.setAttribute("aria-label", "Закрыть ветку");
    closeViewBtn.textContent = "✕";
    closeViewBtn.addEventListener("click", () => {
      if (isAdapterDestroyed) return;
      controller.closeThreadView();
    });

    viewHeader.append(viewTitle, closeViewBtn);

    const viewBody = doc.createElement("div");
    viewBody.className = "thread-view-messages";
    viewBody.setAttribute("role", "log");

    threadViewPanel.append(viewHeader, viewBody);
  };

  const renderAll = () => {
    if (isAdapterDestroyed) return;
    renderPinsPopover();
    renderThreadsSidebarAndView();
  };

  // Подписка на контроллер
  const unsubscribe = controller.subscribe(() => {
    renderAll();
  });
  cleanups.push(unsubscribe);

  // Обработчики кнопок шапки
  listen(btnPins, "click", () => {
    if (isAdapterDestroyed) return;
    const wasOpen = controller.getState().pinsOpen;
    controller.togglePins();
    if (wasOpen && typeof btnPins.focus === "function") {
      btnPins.focus();
    }
  });

  listen(btnThreads, "click", () => {
    if (isAdapterDestroyed) return;
    const wasOpen = controller.getState().threadsOpen;
    controller.toggleThreads();
    if (wasOpen && typeof btnThreads.focus === "function") {
      btnThreads.focus();
    }
  });

  // Кнопки закрытия внутри поповера и сайдбара
  const closePinsBtn = pinsPopover.querySelector('[data-action="close-pins"]');
  listen(closePinsBtn, "click", () => {
    if (isAdapterDestroyed) return;
    controller.closePins();
    if (typeof btnPins.focus === "function") btnPins.focus();
  });

  const closeThreadsBtn = threadsSidebar.querySelector('[data-action="close-threads"]');
  listen(closeThreadsBtn, "click", () => {
    if (isAdapterDestroyed) return;
    controller.closeThreads();
    if (typeof btnThreads.focus === "function") btnThreads.focus();
  });

  // Вкладки Активные / Архивные
  const tabActive = threadsSidebar.querySelector("#tab-active-threads");
  const tabArchived = threadsSidebar.querySelector("#tab-archived-threads");
  listen(tabActive, "click", () => {
    if (isAdapterDestroyed) return;
    controller.setActiveTab("active");
  });
  listen(tabArchived, "click", () => {
    if (isAdapterDestroyed) return;
    controller.setActiveTab("archived");
  });

  // Поиск веток
  const searchInput = threadsSidebar.querySelector(".threads-search-input");
  listen(searchInput, "input", (event) => {
    if (isAdapterDestroyed) return;
    const val = event?.target?.value ?? searchInput?.value ?? "";
    controller.setSearchQuery(String(val));
  });

  // Форма создания новой ветки
  const createForm = threadsSidebar.querySelector("#create-thread-form");
  const nameInput = threadsSidebar.querySelector("#thread-name-input");
  const durationSelect = threadsSidebar.querySelector("#thread-archive-duration");
  const createBtn = threadsSidebar.querySelector("#btn-create-thread");
  const formError = threadsSidebar.querySelector("#thread-form-error");

  const handleCreateThread = async (/** @type {any} */ event) => {
    if (isAdapterDestroyed) return;
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    const rawName = String(nameInput?.value ?? "").trim();
    if (rawName.length < THREAD_NAME_MIN_LEN || rawName.length > THREAD_NAME_MAX_LEN) {
      if (nameInput) nameInput.setAttribute("aria-invalid", "true");
      if (formError) {
        formError.textContent = "Название ветки должно содержать от 1 до 100 символов";
        formError.removeAttribute("hidden");
      }
      return;
    }

    if (nameInput) nameInput.setAttribute("aria-invalid", "false");
    if (formError) {
      formError.textContent = "";
      formError.setAttribute("hidden", "");
    }

    const duration = Number(durationSelect?.value ?? DEFAULT_THREAD_ARCHIVE_DURATION);
    if (nameInput) {
      nameInput.value = "";
    }
    try {
      await controller.createThread({
        name: rawName,
        auto_archive_duration: duration,
      });
    } catch (err) {
      if (nameInput) {
        nameInput.value = rawName;
        nameInput.setAttribute("aria-invalid", "true");
      }
      if (formError) {
        formError.textContent = err instanceof Error ? err.message : String(err);
        formError.removeAttribute("hidden");
      }
    }
  };

  listen(createBtn, "click", handleCreateThread);
  listen(createForm, "submit", handleCreateThread);

  // Закрытие по Escape
  const onKeyDown = (/** @type {any} */ event) => {
    if (isAdapterDestroyed) return;
    const key = "key" in event ? String(event.key) : "";
    if (key !== "Escape") return;

    const st = controller.getState();
    if (st.pinsOpen) {
      if (typeof event.preventDefault === "function") event.preventDefault();
      controller.closePins();
      if (typeof btnPins.focus === "function") btnPins.focus();
    } else if (st.threadsOpen) {
      if (typeof event.preventDefault === "function") event.preventDefault();
      controller.closeThreads();
      if (typeof btnThreads.focus === "function") btnThreads.focus();
    } else if (st.activeThreadId) {
      if (typeof event.preventDefault === "function") event.preventDefault();
      controller.closeThreadView();
      if (typeof btnThreads.focus === "function") btnThreads.focus();
    }
  };

  const globalTarget = doc.body ?? doc;
  listen(globalTarget, "keydown", onKeyDown);
  if (container !== globalTarget) {
    listen(container, "keydown", onKeyDown);
  }

  renderAll();

  return {
    controller,
    pinsButton: btnPins,
    threadsButton: btnThreads,
    pinsPopover,
    threadsSidebar,
    threadViewPanel,
    destroy() {
      if (isAdapterDestroyed) return;
      isAdapterDestroyed = true;
      while (cleanups.length > 0) {
        const fn = cleanups.pop();
        fn?.();
      }
    },
  };
}

