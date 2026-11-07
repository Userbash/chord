// @ts-check
/**
 * Центр «Почта» (`#inbox-popover` и `#chat-inbox-panel`): упоминания, непрочитанные, заявки в друзья.
 *
 * Предоставляет две формы интеграции:
 * 1. `createInboxController`: полнофункциональный контроллер попапа `#inbox-popover`.
 * 2. `createInboxPanel`: контроллер панели `#chat-inbox-panel` (STUB-11 / N-03) с вкладками
 *    «Упоминания» и «Непрочитанные», a11y атрибутами и обработчиками переходов.
 *
 * @typedef {{ id: string, title: string, kind: "mention" | "unread" | "friend_request", raw: any }} InboxItem
 * @typedef {{ mentions: InboxItem[], unreads: InboxItem[], friendRequests: InboxItem[] }} InboxItems
 * @typedef {{ onSelectMessage?: (item: any) => void, onAcceptFriend?: (item: any) => void }} InboxOptions
 *
 * @typedef {{
 *   document?: any,
 *   container?: any,
 *   inboxButton?: any,
 *   loadMentions?: () => Promise<any[]>,
 *   loadUnreads?: () => Promise<any[]>,
 *   onJumpToMessage?: (channelId: string, messageId: string) => void,
 *   onAckChannel?: (channelId: string) => void,
 * }} InboxPanelOptions
 */

const rawTabs = ["for_you", "unreads", "mentions", "friend_requests"];
Object.defineProperty(rawTabs, "for_you", { value: "for_you", enumerable: false });
Object.defineProperty(rawTabs, "unreads", { value: "unreads", enumerable: false });
Object.defineProperty(rawTabs, "mentions", { value: "mentions", enumerable: false });
Object.defineProperty(rawTabs, "friend_requests", { value: "friend_requests", enumerable: false });

export const INBOX_TABS = /** @type {readonly string[] & { mentions: string, unreads: string, for_you: string, friend_requests: string }} */ (
  Object.freeze(rawTabs)
);

export const INBOX_TEXT = Object.freeze({
  title: "Входящие",
  unreads: "Непрочитанные",
  mentions: "Упоминания",
  forYou: "Для вас",
  empty: "Входящие пусты",
  clearAll: "Очистить всё",
  jump: "Перейти",
});

/** Подписи вкладок для кнопок `[data-inbox-tab]`. */
export const INBOX_TAB_LABELS = /** @type {Record<string, string>} */ (Object.freeze({
  for_you: "Для вас",
  unreads: "Непрочитанные",
  mentions: "Упоминания",
  friend_requests: "Заявки в друзья",
}));

/**
 * Текст бейджа непрочитанного: пусто при `count <= 0`, `99+` при `count > 99`,
 * иначе строка числа.
 * @param {number} count
 * @returns {string}
 */
export function formatInboxBadge(count) {
  const value = Number(count);
  if (!Number.isFinite(value) || value <= 0) return "";
  if (value > 99) return "99+";
  return String(value);
}

/**
 * @param {any} doc
 * @param {InboxOptions} [options]
 */
export function createInboxController(doc, options = {}) {
  const root = doc.body || doc.documentElement;

  const previous = doc.querySelector("#inbox-popover");
  if (previous) previous.remove();

  const popover = doc.createElement("div");
  popover.id = "inbox-popover";
  popover.setAttribute("role", "dialog");
  popover.setAttribute("aria-label", "Почта");
  popover.setAttribute("aria-modal", "false");
  popover.hidden = true;
  popover.setAttribute("aria-hidden", "true");

  const header = doc.createElement("div");
  header.setAttribute("data-inbox-header", "");
  const title = doc.createElement("h2");
  title.textContent = "Почта";
  const badge = doc.createElement("span");
  badge.setAttribute("data-inbox-badge", "");
  badge.setAttribute("aria-live", "polite");
  const markAllButton = doc.createElement("button");
  markAllButton.setAttribute("type", "button");
  markAllButton.setAttribute("data-action", "inbox-mark-all-read");
  markAllButton.textContent = "Прочитать все";
  header.appendChild(title);
  header.appendChild(badge);
  header.appendChild(markAllButton);

  const tabList = doc.createElement("div");
  tabList.setAttribute("role", "tablist");
  tabList.setAttribute("aria-label", "Разделы почты");
  /** @type {Map<string, any>} */
  const tabButtons = new Map();
  for (const tab of INBOX_TABS) {
    const button = doc.createElement("button");
    button.setAttribute("type", "button");
    button.setAttribute("role", "tab");
    button.setAttribute("data-inbox-tab", tab);
    button.textContent = INBOX_TAB_LABELS[tab];
    button.addEventListener("click", () => setTab(tab));
    tabButtons.set(tab, button);
    tabList.appendChild(button);
  }

  const list = doc.createElement("div");
  list.setAttribute("data-inbox-list", "");
  list.setAttribute("role", "tabpanel");

  popover.appendChild(header);
  popover.appendChild(tabList);
  popover.appendChild(list);
  root.appendChild(popover);

  let activeTab = "for_you";
  let destroyed = false;
  /** @type {InboxItems} */
  let items = { mentions: [], unreads: [], friendRequests: [] };
  /** @type {Set<string>} */
  const readIds = new Set();

  /**
   * @param {any} raw
   * @param {number} index
   * @param {InboxItem["kind"]} kind
   * @returns {InboxItem | null}
   */
  function normalize(raw, index, kind) {
    if (raw === null || raw === undefined) return null;
    const source = typeof raw === "object" ? raw : { id: String(raw), title: String(raw) };
    const rawId = source.id ?? source.messageId ?? source.userId ?? `${kind}-${index}`;
    const id = String(rawId);
    const title = source.title ?? source.text ?? source.name ?? source.username ?? id;
    return { id, title: String(title), kind, raw: source };
  }

  /**
   * @param {any[]} value
   * @param {InboxItem["kind"]} kind
   * @returns {InboxItem[]}
   */
  function normalizeList(value, kind) {
    if (!Array.isArray(value)) return [];
    return value
      .map((entry, index) => normalize(entry, index, kind))
      .filter(/** @returns {entry is InboxItem} */ (entry) => entry !== null);
  }

  /** @returns {InboxItem[]} */
  function emptyList() { return []; }

  /** @returns {number} */
  function getUnreadCount() {
    return items.mentions.length + items.friendRequests.length;
  }

  /** @returns {InboxItem[]} */
  function visibleItems() {
    if (activeTab === "unreads") return items.unreads;
    if (activeTab === "mentions") return items.mentions;
    if (activeTab === "friend_requests") return items.friendRequests;
    return [...items.mentions, ...items.friendRequests];
  }

  /** @param {InboxItem} item */
  function renderItem(item) {
    const row = doc.createElement("div");
    row.setAttribute("data-inbox-item", item.id);
    row.setAttribute("data-inbox-kind", item.kind);

    const select = doc.createElement("button");
    select.setAttribute("type", "button");
    select.textContent = item.title;
    select.addEventListener("click", () => {
      if (typeof options.onSelectMessage === "function") options.onSelectMessage(item.raw);
    });
    row.appendChild(select);

    if (item.kind === "friend_request") {
      const accept = doc.createElement("button");
      accept.setAttribute("type", "button");
      accept.setAttribute("data-action", "inbox-accept-friend");
      accept.setAttribute("data-inbox-item-id", item.id);
      accept.textContent = "Принять";
      accept.addEventListener("click", () => {
        if (typeof options.onAcceptFriend === "function") options.onAcceptFriend(item.raw);
      });
      row.appendChild(accept);
    }
    return row;
  }

  function render() {
    if (destroyed) return;
    badge.textContent = formatInboxBadge(getUnreadCount());
    badge.setAttribute("data-count", String(getUnreadCount()));

    for (const tab of INBOX_TABS) {
      const button = tabButtons.get(tab);
      if (!button) continue;
      const selected = tab === activeTab;
      button.setAttribute("aria-selected", selected ? "true" : "false");
      button.setAttribute("tabindex", selected ? "0" : "-1");
    }

    const entries = visibleItems();
    /** @type {any[]} */
    const nodes = [];
    if (entries.length === 0) {
      const empty = doc.createElement("p");
      empty.setAttribute("data-inbox-empty", "");
      empty.textContent = "Всё прочитано";
      nodes.push(empty);
    } else {
      for (const item of entries) nodes.push(renderItem(item));
    }
    list.replaceChildren(...nodes);
  }

  /** @param {any} input */
  function setItems(input = {}) {
    if (destroyed || !input || typeof input !== "object") return getUnreadCount();
    if (input.mentions !== undefined) items.mentions = normalizeList(input.mentions, "mention");
    if (input.unreads !== undefined) items.unreads = normalizeList(input.unreads, "unread");
    const friends = input.friendRequests !== undefined ? input.friendRequests : input.friend_requests;
    if (friends !== undefined) items.friendRequests = normalizeList(friends, "friend_request");
    readIds.clear();
    render();
    return getUnreadCount();
  }

  /** @param {string} tab */
  function setTab(tab) {
    if (destroyed || !INBOX_TABS.includes(tab)) return false;
    activeTab = tab;
    render();
    return true;
  }

  function open() {
    if (destroyed) return false;
    popover.hidden = false;
    popover.setAttribute("aria-hidden", "false");
    return true;
  }

  function close() {
    if (destroyed) return false;
    popover.hidden = true;
    popover.setAttribute("aria-hidden", "true");
    return true;
  }

  function toggle() {
    if (destroyed) return false;
    return popover.hidden ? open() : close();
  }

  /** @param {string | number} id */
  function markItemRead(id) {
    if (destroyed) return false;
    const key = String(id);
    if (readIds.has(key)) return false;
    let removed = false;
    for (const keyName of /** @type {const} */ (["mentions", "unreads", "friendRequests"])) {
      const before = items[keyName].length;
      items[keyName] = items[keyName].filter((item) => item.id !== key);
      if (items[keyName].length !== before) removed = true;
    }
    if (!removed) return false;
    readIds.add(key);
    render();
    return true;
  }

  function markAllRead() {
    if (destroyed) return 0;
    const cleared = items.mentions.length + items.unreads.length + items.friendRequests.length;
    items = { mentions: emptyList(), unreads: emptyList(), friendRequests: emptyList() };
    readIds.clear();
    render();
    return cleared;
  }

  function getActiveTab() { return activeTab; }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    popover.remove();
    items = { mentions: emptyList(), unreads: emptyList(), friendRequests: emptyList() };
    readIds.clear();
  }

  markAllButton.addEventListener("click", () => { markAllRead(); });
  render();

  return {
    element: popover,
    open,
    close,
    toggle,
    setTab,
    getActiveTab,
    setItems,
    markItemRead,
    markAllRead,
    getUnreadCount,
    destroy,
  };
}

/**
 * STUB-11 (N-03): Выпадающая панель «Входящие» с вкладками Упоминания и Непрочитанные.
 * @param {InboxPanelOptions} options
 */
export function createInboxPanel(options = {}) {
  const doc = options.document || (typeof document !== "undefined" ? document : null);
  if (!doc) throw new Error("Document is required for createInboxPanel");

  const container = options.container || doc.body || doc.documentElement;
  const inboxButton = options.inboxButton;

  const panel = doc.createElement("div");
  panel.id = "chat-inbox-panel";
  panel.className = "chat-inbox-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", INBOX_TEXT.title);
  panel.hidden = true;

  if (inboxButton) {
    inboxButton.setAttribute("aria-expanded", "false");
    inboxButton.setAttribute("aria-haspopup", "dialog");
    inboxButton.setAttribute("aria-controls", "chat-inbox-panel");
    inboxButton.addEventListener("click", () => toggle());
  }

  // Header
  const header = doc.createElement("div");
  header.className = "inbox-header";

  const title = doc.createElement("h3");
  title.className = "inbox-title";
  title.textContent = INBOX_TEXT.title;
  header.appendChild(title);

  // Tabs container
  const tabsNav = doc.createElement("div");
  tabsNav.className = "inbox-tabs";
  tabsNav.setAttribute("role", "tablist");

  const tabMentions = doc.createElement("button");
  tabMentions.setAttribute("type", "button");
  tabMentions.setAttribute("role", "tab");
  tabMentions.setAttribute("data-tab", "mentions");
  tabMentions.className = "inbox-tab-btn active";
  tabMentions.setAttribute("aria-selected", "true");
  tabMentions.textContent = INBOX_TEXT.mentions;

  const tabUnreads = doc.createElement("button");
  tabUnreads.setAttribute("type", "button");
  tabUnreads.setAttribute("role", "tab");
  tabUnreads.setAttribute("data-tab", "unreads");
  tabUnreads.className = "inbox-tab-btn";
  tabUnreads.setAttribute("aria-selected", "false");
  tabUnreads.textContent = INBOX_TEXT.unreads;

  tabsNav.appendChild(tabMentions);
  tabsNav.appendChild(tabUnreads);
  header.appendChild(tabsNav);

  // Actions
  const actions = doc.createElement("div");
  actions.className = "inbox-actions";

  const ackAllBtn = doc.createElement("button");
  ackAllBtn.setAttribute("type", "button");
  ackAllBtn.setAttribute("data-action", "inbox-ack-all");
  ackAllBtn.className = "inbox-ack-all-btn";
  ackAllBtn.textContent = INBOX_TEXT.clearAll;
  actions.appendChild(ackAllBtn);
  header.appendChild(actions);

  panel.appendChild(header);

  // List container
  const list = doc.createElement("div");
  list.className = "inbox-list";
  list.setAttribute("role", "tabpanel");
  panel.appendChild(list);

  container.appendChild(panel);

  let activeTab = "mentions";
  /** @type {any[]} */
  let currentMentions = [];
  /** @type {any[]} */
  let currentUnreads = [];

  function open() {
    panel.hidden = false;
    panel.classList.add("is-open");
    if (inboxButton) {
      inboxButton.setAttribute("aria-expanded", "true");
    }
  }

  function close() {
    panel.hidden = true;
    panel.classList.remove("is-open");
    if (inboxButton) {
      inboxButton.setAttribute("aria-expanded", "false");
    }
  }

  function toggle() {
    if (panel.hidden) {
      open();
    } else {
      close();
    }
  }

  function getActiveTab() {
    return activeTab;
  }

  /** @param {string} tab */
  function setActiveTab(tab) {
    activeTab = tab;
    tabMentions.setAttribute("aria-selected", tab === "mentions" ? "true" : "false");
    tabMentions.classList.toggle("active", tab === "mentions");
    tabUnreads.setAttribute("aria-selected", tab === "unreads" ? "true" : "false");
    tabUnreads.classList.toggle("active", tab === "unreads");
    renderList();
  }

  tabMentions.addEventListener("click", () => setActiveTab("mentions"));
  tabUnreads.addEventListener("click", () => setActiveTab("unreads"));

  ackAllBtn.addEventListener("click", () => {
    if (activeTab === "unreads") {
      const itemsToAck = [...currentUnreads];
      currentUnreads = [];
      for (const item of itemsToAck) {
        if (typeof options.onAckChannel === "function") {
          options.onAckChannel(item.channelId);
        }
      }
    } else {
      currentMentions = [];
    }
    renderList();
  });

  function renderList() {
    list.replaceChildren();

    if (activeTab === "mentions") {
      if (currentMentions.length === 0) {
        const empty = doc.createElement("p");
        empty.className = "inbox-empty";
        empty.textContent = INBOX_TEXT.empty;
        list.appendChild(empty);
        return;
      }

      for (const mention of currentMentions) {
        const item = doc.createElement("div");
        item.className = "inbox-item";
        item.setAttribute("data-mention-id", mention.id || "");

        const meta = doc.createElement("div");
        meta.className = "inbox-item-meta";
        const author = doc.createElement("strong");
        author.className = "inbox-item-author";
        author.textContent = mention.author || "Пользователь";
        const channel = doc.createElement("span");
        channel.className = "inbox-item-channel";
        channel.textContent = mention.channelName ? `#${mention.channelName}` : "";
        meta.appendChild(author);
        meta.appendChild(channel);
        item.appendChild(meta);

        const content = doc.createElement("div");
        content.className = "inbox-item-content";
        content.textContent = mention.content || "";
        item.appendChild(content);

        const actionsDiv = doc.createElement("div");
        actionsDiv.className = "inbox-item-actions";
        const jumpBtn = doc.createElement("button");
        jumpBtn.setAttribute("type", "button");
        jumpBtn.setAttribute("data-action", "jump");
        jumpBtn.className = "inbox-jump-btn";
        jumpBtn.textContent = INBOX_TEXT.jump;
        jumpBtn.addEventListener("click", () => {
          if (typeof options.onJumpToMessage === "function") {
            options.onJumpToMessage(mention.channelId, mention.id);
          }
          close();
        });
        actionsDiv.appendChild(jumpBtn);
        item.appendChild(actionsDiv);

        list.appendChild(item);
      }
    } else {
      if (currentUnreads.length === 0) {
        const empty = doc.createElement("p");
        empty.className = "inbox-empty";
        empty.textContent = INBOX_TEXT.empty;
        list.appendChild(empty);
        return;
      }

      for (const unread of currentUnreads) {
        const item = doc.createElement("div");
        item.className = "inbox-item";
        item.setAttribute("data-channel-id", unread.channelId || "");

        const meta = doc.createElement("div");
        meta.className = "inbox-item-meta";
        const channel = doc.createElement("strong");
        channel.className = "inbox-item-channel";
        channel.textContent = unread.channelName ? `#${unread.channelName}` : unread.channelId;
        const count = doc.createElement("span");
        count.className = "inbox-item-count";
        count.textContent = `(${unread.unreadCount || 1})`;
        meta.appendChild(channel);
        meta.appendChild(count);
        item.appendChild(meta);

        const actionsDiv = doc.createElement("div");
        actionsDiv.className = "inbox-item-actions";
        const jumpBtn = doc.createElement("button");
        jumpBtn.setAttribute("type", "button");
        jumpBtn.setAttribute("data-action", "jump");
        jumpBtn.className = "inbox-jump-btn";
        jumpBtn.textContent = INBOX_TEXT.jump;
        jumpBtn.addEventListener("click", () => {
          if (typeof options.onJumpToMessage === "function") {
            options.onJumpToMessage(unread.channelId, "");
          }
          close();
        });
        actionsDiv.appendChild(jumpBtn);
        item.appendChild(actionsDiv);

        list.appendChild(item);
      }
    }
  }

  async function refresh() {
    if (typeof options.loadMentions === "function") {
      try {
        currentMentions = (await options.loadMentions()) || [];
      } catch {
        currentMentions = [];
      }
    }
    if (typeof options.loadUnreads === "function") {
      try {
        currentUnreads = (await options.loadUnreads()) || [];
      } catch {
        currentUnreads = [];
      }
    }
    renderList();
  }

  function destroy() {
    panel.remove();
  }

  return {
    panel,
    open,
    close,
    toggle,
    getActiveTab,
    setActiveTab,
    refresh,
    destroy,
  };
}
