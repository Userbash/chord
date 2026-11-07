// @ts-check
/**
 * L-02-UI (RW-40): DOM-адаптер dispatch-проекций.
 *
 * Единственный вход — `Change[]` из `dispatch-model.js`; здесь только селекторы
 * контракта (PLAN §4): `#guild-rail .guild-item[data-guild-id]`, `.tree-guild`,
 * `#member-panel-list .member[data-user-id]`, `#member-online`,
 * `#chat-list .row[data-message-id]`. Новых id/разметки/CSS нет:
 * `index.html` заморожен, поэтому live-состояние выражается атрибутами
 * `data-source` / `data-dispatch-source` / `data-live-order`.
 *
 * Инварианты §4.4: ничего не перезапрашиваем (никакого IPC), не пересобираем
 * `#chat-list`, не двигаем фокус/экран/скролл, переиспользуем узлы строк.
 */
import { memberHeight } from "../model.js";
import { formatNumber } from "./dom.js";
import { renderMemberRow } from "./members.js";
import { renderChannelMessage } from "./chat.js";

/**
 * Стабильный оттенок аватара по id (не зависит от индекса окна).
 * @param {string} key
 * @returns {number}
 */
function hueOf(key) {
  let hash = 0;
  for (const char of key) hash = (hash * 31 + (char.codePointAt(0) ?? 0)) % 360;
  return hash;
}

/**
 * Форматирование времени ISO в HH:MM.
 * @param {string | null | undefined} timestamp
 * @returns {string}
 */
function formatTime(timestamp) {
  if (!timestamp) return "";
  const match = String(timestamp).match(/T(\d{2}:\d{2})/);
  if (match && typeof match[1] === "string") return match[1];
  try {
    const d = new Date(timestamp);
    if (!isNaN(d.getTime())) {
      return `${String(d.getUTCHours()).padStart(2, "0")}:${String(d.getUTCMinutes()).padStart(2, "0")}`;
    }
  } catch {}
  return "";
}

/**
 * Одна live-строка участника: тот же шаблон §7.2, что у bench, но высота 42,
 * адресация `data-user-id` и `aria-setsize` по размеру окна (PLAN §4.3).
 * @param {HTMLElement} node
 * @param {import("./dispatch-model.js").MemberRow} row
 * @param {number} index
 * @param {number} total
 */
function paintLiveRow(node, row, index, total) {
  const activity = row.activity !== "" ? row.activity : row.statusLabel;
  /** @type {import("./members.js").Member} */
  const item = {
    name: row.name,
    status: row.status,
    statusLabel: row.statusLabel,
    role: "",
    badge: "",
    activity,
    hue: hueOf(row.userId),
  };
  const label = `${row.name}, ${row.statusLabel}${row.activity !== "" ? `, ${row.activity}` : ""}`;
  renderMemberRow(node, item, index, { total, userId: row.userId, label });
  node.style.height = `${memberHeight()}px`;
  // Строка больше не принадлежит bench-группам (группировка — MB-01).
  node.removeAttribute("data-group");
  node.removeAttribute("data-window");
}

/**
 * @param {{
 *   body?: HTMLElement | null,
 *   memberPanelList?: HTMLElement | null,
 *   memberOnline?: HTMLElement | null,
 *   treeHeading?: HTMLElement | null,
 *   rail?: { applyGuilds?: (items: any[]) => void, selectedId?: string } | null,
 *   chat?: {
 *     applyReaction?: (change: any) => string,
 *     syncLiveMessage?: (channelId: string, rawMsg: any) => boolean,
 *     channelId?: string,
 *     mode?: string,
 *     channel?: { id: string, name: string } | null,
 *     unreadHeight?: number,
 *   } | null,
 *   netStatus?: HTMLElement | null,
 *   selectedGuildId?: string,
 *   chatList?: HTMLElement | null,
 *   messageList?: HTMLElement | null,
 *   voicePanel?: HTMLElement | null,
 *   typingIndicator?: HTMLElement | null,
 *   composer?: HTMLElement | null,
 *   getActiveChannelId?: () => string,
 *   onVoiceStateChange?: (change: import("./dispatch-model.js").VoiceStateChange) => void,
 *   onVoiceServerChange?: (change: import("./dispatch-model.js").VoiceServerChange) => void,
 * }} [options]
 */
export function createDispatchView(options = {}) {
  const body = options.body ?? null;
  const memberPanelList = options.memberPanelList ?? null;
  const memberOnline = options.memberOnline ?? null;
  const treeHeading = options.treeHeading ?? null;
  const rail = options.rail ?? null;
  const chat = options.chat ?? null;
  const netStatus = options.netStatus ?? null;
  let selectedGuildId = String(options.selectedGuildId ?? "").trim();
  /** READY не перебивает точный `online_count` из LIST_UPDATE (PLAN §3.1). */
  let onlineCountKnown = false;

  const doc = body?.ownerDocument ?? (typeof document !== "undefined" ? document : null);

  /** @type {Map<string, Set<string>>} channelId -> Set<userId> */
  const activeTyping = new Map();
  /** @type {Map<string, Map<string, import("./dispatch-model.js").VoiceStateChange>>} guildId -> (userId -> state) */
  const voiceParticipants = new Map();
  /** @type {Map<string, number>} guildId -> active thread count */
  const threadCounts = new Map();

  /** @returns {string} */
  function currentSelection() {
    if (selectedGuildId !== "") return selectedGuildId;
    return String(rail?.selectedId ?? "");
  }

  /** @returns {string} */
  function currentChannelId() {
    if (typeof options.getActiveChannelId === "function") {
      return String(options.getActiveChannelId() ?? "");
    }
    if (chat && typeof chat.mode === "string") {
      return chat.mode === "channel" ? String(chat.channelId ?? "") : "";
    }
    return "";
  }

  /**
   * Список сообщений чата.
   * @returns {HTMLElement | null}
   */
  function getMessageList() {
    return options.messageList ?? options.chatList
      ?? (doc ? (doc.getElementById("message-list") ?? doc.getElementById("chat-list")) : null);
  }

  /**
   * Элемент канала в дереве.
   * @param {string} channelId
   * @returns {HTMLElement | null}
   */
  function getChannelTile(channelId) {
    if (!doc || !channelId) return null;
    return doc.querySelector(`.channel-item[data-channel-id="${channelId}"]`);
  }

  /**
   * Индикатор набора сообщения.
   * @returns {HTMLElement | null}
   */
  function getTypingIndicator() {
    return options.typingIndicator ?? (doc ? doc.getElementById("typing-indicator") : null);
  }

  /**
   * Форма ввода композера.
   * @returns {HTMLElement | null}
   */
  function getComposer() {
    return options.composer ?? (doc ? (doc.getElementById("composer") ?? doc.getElementById("composer-form")) : null);
  }

  /**
   * Контейнер чата.
   * @returns {HTMLElement | null}
   */
  function getChatView() {
    return doc ? (doc.getElementById("chat-view") ?? doc.getElementById("screen-chat")) : null;
  }

  /**
   * Панель голосовой связи.
   * @returns {HTMLElement | null}
   */
  function getVoicePanel() {
    return options.voicePanel ?? (doc ? doc.getElementById("voice-panel") : null);
  }

  /**
   * Честный источник dispatch без нового текста/баннера (`CONTRACTS/dispatch.md`
   * §8): live | absent | unavailable.
   * @param {string} source
   */
  function setSource(source) {
    const value = source === "live" || source === "unavailable" ? source : "absent";
    body?.setAttribute("data-dispatch-source", value);
    const node = netStatus
      ?? (typeof document !== "undefined" ? document.getElementById("net-status") : null);
    node?.setAttribute("data-dispatch-source", value);
  }

  /** @param {string} guildId */
  function selectGuild(guildId) {
    selectedGuildId = String(guildId ?? "").trim();
  }

  /** @param {import("./dispatch-model.js").GuildsChange} change */
  function applyGuilds(change) {
    const items = Array.isArray(change.guilds) ? change.guilds : [];
    if (rail && typeof rail.applyGuilds === "function") rail.applyGuilds(items);
    const selected = currentSelection();
    const chosen = items.find((guild) => guild.id === selected)
      ?? items.find((guild) => guild.id === String(rail?.selectedId ?? ""));
    if (treeHeading && chosen) treeHeading.textContent = chosen.name;
    if (memberOnline && !onlineCountKnown && chosen && chosen.memberCount !== null) {
      memberOnline.textContent = formatNumber(chosen.memberCount);
    }
  }

  /**
   * @param {import("./dispatch-model.js").MembersChange} change
   * @returns {number}
   */
  function applyMembers(change) {
    // Гильдия не выбрана — только store (DOM другой гильдии не трогаем).
    if (change.guildId !== currentSelection()) return 1;
    if (!memberPanelList) return 1;
    if (memberPanelList.getAttribute("data-source") === "live-grouped") return 1;
    memberPanelList.setAttribute("data-source", "live");
    // В live-списке есть и offline: нейтральное имя вместо bench «в сети».
    memberPanelList.setAttribute("aria-label", "Участники сервера");
    const order = Array.isArray(change.order) ? change.order : [];
    const rows = Array.isArray(change.rows) ? change.rows : [];
    /** @type {Map<string, import("./dispatch-model.js").MemberRow>} */
    const byId = new Map(rows.map((row) => [row.userId, row]));
    /** @type {Map<string, HTMLElement>} */
    const existing = new Map();
    for (const child of /** @type {HTMLElement[]} */ ([...memberPanelList.children])) {
      const id = child.getAttribute("data-user-id") ?? "";
      if (id !== "") existing.set(id, child);
    }
    const nodes = order.map((userId, index) => {
      const node = existing.get(userId) ?? (doc ? doc.createElement("div") : document.createElement("div"));
      const row = byId.get(userId) ?? {
        userId,
        name: userId,
        status: "offline",
        statusLabel: "не в сети",
        activity: "",
      };
      paintLiveRow(node, row, index, order.length);
      return node;
    });
    // Переиспользуем узлы (идентичность строк важна для C3/C11), меняя порядок.
    memberPanelList.replaceChildren(...nodes);
    memberPanelList.setAttribute("data-live-order", String(order.length));
    if (memberOnline) {
      // Счётчик уже live-регион (PLAN §4.5): повторная установка безвредна и
      // держит контракт, даже если bench-панель не успела отрисоваться.
      memberOnline.setAttribute("aria-live", "polite");
      if (typeof change.onlineCount === "number") {
        memberOnline.textContent = formatNumber(change.onlineCount);
        onlineCountKnown = true;
      } else if (!onlineCountKnown && typeof change.memberCount === "number") {
        memberOnline.textContent = formatNumber(change.memberCount);
      }
    }
    return 1;
  }

  /**
   * @param {import("./dispatch-model.js").PresenceChange} change
   * @returns {number}
   */
  function applyPresence(change) {
    if (!memberPanelList) return 1;
    // Presence адресован гильдии панели: чужой scope не трогает выбранные строки.
    if (change.guildId !== currentSelection()) return 1;
    const row = /** @type {HTMLElement | null} */ (
      [...memberPanelList.children].find(
        (node) => node.getAttribute("data-user-id") === change.userId,
      ) ?? null
    );
    // Строки нет в окне: store уже обновлён моделью — не «тихо потеряли».
    if (!row) return 1;
    row.setAttribute("data-status", change.status);
    const dot = row.querySelector(".status-dot");
    if (dot) dot.setAttribute("data-status", change.status);
    const nameNode = row.querySelector(".member-label");
    const name = nameNode?.textContent ?? change.userId;
    const role = row.querySelector(".member-role");
    if (role) role.textContent = change.activity !== "" ? change.activity : change.statusLabel;
    row.setAttribute(
      "aria-label",
      `${name}, ${change.statusLabel}${change.activity !== "" ? `, ${change.activity}` : ""}`,
    );
    return 1;
  }

  /**
   * @param {import("./dispatch-model.js").ReactionChange} change
   * @returns {{ applied: number, notOpen: number, notLoaded: number }}
   */
  function applyReaction(change) {
    if (!chat || typeof chat.applyReaction !== "function") return { applied: 0, notOpen: 0, notLoaded: 0 };
    const result = chat.applyReaction(change);
    if (result === "not-open") return { applied: 0, notOpen: 1, notLoaded: 0 };
    if (result === "not-loaded") return { applied: 0, notOpen: 0, notLoaded: 1 };
    return { applied: 1, notOpen: 0, notLoaded: 0 };
  }

  /**
   * Снять статус печатающего пользователя и обновить UI.
   * @param {string} channelId
   * @param {string} userId
   */
  function clearTypingUser(channelId, userId) {
    const users = activeTyping.get(channelId);
    if (!users) return;
    users.delete(userId);
    if (channelId === currentChannelId()) {
      const indicator = getTypingIndicator();
      if (indicator) {
        if (users.size === 0) {
          indicator.setAttribute("hidden", "");
          indicator.setAttribute("data-visible", "false");
          indicator.setAttribute("data-count", "0");
          indicator.removeAttribute("data-typing-users");
          const text = indicator.querySelector(".typing-indicator-text") ?? indicator;
          text.textContent = "";
        } else {
          indicator.setAttribute("data-count", String(users.size));
          indicator.setAttribute("data-typing-users", [...users].join(","));
          const text = indicator.querySelector(".typing-indicator-text") ?? indicator;
          text.textContent = users.size > 1 ? "Несколько человек печатают…" : "Печатает…";
        }
      }
      const composer = getComposer();
      if (users.size === 0) composer?.removeAttribute("data-typing-users");
      else composer?.setAttribute("data-typing-users", [...users].join(","));
      const chatView = getChatView();
      if (users.size === 0) chatView?.removeAttribute("data-typing-users");
      else chatView?.setAttribute("data-typing-users", [...users].join(","));
    }
  }

  /**
   * C17: живой MESSAGE_CREATE для активного канала добавляет новую строку в список,
   * для неактивного — увеличивает unread/mention индикатор в дереве.
   * @param {import("./dispatch-model.js").MessageCreateChange} change
   * @returns {number}
   */
  function applyMessageCreate(change) {
    const d = /** @type {any} */ (change?.message ?? {});
    if (change?.channelId && change?.message) {
      if (typeof chat?.syncLiveMessage === "function") {
        chat.syncLiveMessage(change.channelId, change.message);
      }
      const globalSync = /** @type {any} */ (globalThis).__SYNC_LIVE_CHAT_MESSAGE__;
      if (typeof globalSync === "function" && globalSync !== chat?.syncLiveMessage) {
        globalSync(change.channelId, change.message);
      }
    }
    const activeId = currentChannelId();
    if (change.channelId === activeId && activeId !== "") {
      const list = getMessageList();
      if (list) {
        const emptyState = list.querySelector?.('.chat-channel-state[data-state="empty"]');
        if (emptyState) emptyState.remove();
        let row = /** @type {HTMLElement | null} */ (
          [...list.children].find((c) => c.getAttribute("data-message-id") === change.messageId) ?? null
        );
        if (!row && doc) {
          row = doc.createElement("div");
          list.append(row);
        }
        if (row) {
          const authorName = change.message.authorName || change.message.authorId || "Пользователь";
          const rawStickers = Array.isArray(d.sticker_items)
            ? d.sticker_items
            : (Array.isArray(d.stickers) ? d.stickers : []);
          const msgType = typeof d.messageType === "number"
            ? d.messageType
            : (typeof d.type === "number" ? d.type : (typeof d.message_type === "number" ? d.message_type : 0));
          /** @type {import("./chat.js").ChannelMessage} */
          const item = {
            id: change.message.id,
            author: authorName,
            authorId: change.message.authorId,
            authorAvatar: change.message.authorAvatar ?? null,
            hue: hueOf(change.message.authorId || authorName),
            time: formatTime(change.message.timestamp),
            iso: change.message.timestamp || new Date().toISOString(),
            day: "",
            channelId: change.channelId,
            bot: Boolean(change.message.authorBot),
            text: change.message.content || "",
            group: "start",
            edited: Boolean(change.message.edited),
            dayLabel: "",
            reactions: [],
            attachments: /** @type {any[]} */ (change.message.attachments ?? []),
            embeds: /** @type {any[]} */ (change.message.embeds ?? []),
            sticker_items: rawStickers,
            type: msgType,
            messageType: msgType,
          };
          const index = [...list.children].indexOf(row);
          renderChannelMessage(row, item, Math.max(0, index), list.children.length, { channelId: change.channelId });
          if (typeof list.scrollHeight === "number" && list.scrollHeight > 0) {
            list.scrollTop = list.scrollHeight;
          }
          const localPending = doc?.querySelector?.(`.chat-sent-messages .local-message[data-message-id="${change.message.id}"]`);
          if (localPending) localPending.remove();
        }
      }
      // Приход сообщения автора гасит его тайпинг
      if (change.message?.authorId) {
        clearTypingUser(change.channelId, change.message.authorId);
      }
    } else {
      // Неактивный канал: unread и mentions
      const tile = getChannelTile(change.channelId);
      if (tile) {
        tile.classList.add("is-unread");
        tile.setAttribute("data-unread", "true");
        const hasMention = change.message?.mentionEveryone
          || (Array.isArray(change.message?.mentions) && change.message.mentions.length > 0);
        if (hasMention) {
          const currentCount = Number(tile.getAttribute("data-mention-count") || "0");
          const nextCount = currentCount + 1;
          tile.setAttribute("data-mention-count", String(nextCount));
          let badge = tile.querySelector(".channel-badge");
          if (!badge && doc) {
            badge = doc.createElement("span");
            badge.className = "channel-badge";
            tile.append(badge);
          }
          if (badge) badge.textContent = String(nextCount);
        }
      }
      if (change.message?.authorId) {
        clearTypingUser(change.channelId, change.message.authorId);
      }
    }
    return 1;
  }

  /**
   * C18: живой MESSAGE_UPDATE обновляет текст сообщения и плашку '(изменено)'.
   * @param {import("./dispatch-model.js").MessageUpdateChange} change
   * @returns {{ applied: number, notLoaded: number }}
   */
  function applyMessageUpdate(change) {
    const list = getMessageList();
    const row = list
      ? /** @type {HTMLElement | null} */ (
        [...list.children].find((c) => c.getAttribute("data-message-id") === change.messageId) ?? null
      )
      : null;
    if (!row) return { applied: 0, notLoaded: 1 };

    const textBody = row.querySelector(".text-body");
    if (textBody) textBody.textContent = change.content;

    if (change.edited) {
      const text = row.querySelector(".text");
      if (text) {
        text.setAttribute("data-stack", "true");
        let edited = text.querySelector(".edited");
        if (!edited && doc) {
          edited = doc.createElement("span");
          edited.className = "edited";
          edited.textContent = "(изменено)";
          text.append(edited);
        }
      }
      const extras = (row.getAttribute("data-extras") || "").split(/\s+/).filter(Boolean);
      if (!extras.includes("edited")) {
        extras.push("edited");
        row.setAttribute("data-extras", extras.join(" "));
      }
      const currentAria = row.getAttribute("aria-label") || "";
      if (!currentAria.includes("(изменено)")) {
        row.setAttribute("aria-label", `${currentAria} (изменено)`.trim());
      }
    }
    if (
      change.message &&
      ((Array.isArray(change.message.attachments) && change.message.attachments.length > 0) ||
        (Array.isArray(change.message.embeds) && change.message.embeds.length > 0) ||
        change.message.authorAvatar)
    ) {
      const authorName = change.message.authorName || change.message.authorId || "Пользователь";
      /** @type {import("./chat.js").ChannelMessage} */
      const item = {
        id: change.message.id,
        author: authorName,
        authorId: change.message.authorId,
        authorAvatar: change.message.authorAvatar ?? null,
        hue: hueOf(change.message.authorId || authorName),
        time: formatTime(change.message.timestamp),
        iso: change.message.timestamp || new Date().toISOString(),
        day: "",
        channelId: change.channelId,
        bot: Boolean(change.message.authorBot),
        text: change.message.content || "",
        group: "start",
        edited: Boolean(change.message.edited),
        dayLabel: "",
        reactions: [],
        attachments: /** @type {any[]} */ (change.message.attachments ?? []),
        embeds: /** @type {any[]} */ (change.message.embeds ?? []),
      };
      const index = list ? [...list.children].indexOf(row) : 0;
      renderChannelMessage(row, item, Math.max(0, index), list ? list.children.length : 1, { channelId: change.channelId });
    }
    return { applied: 1, notLoaded: 0 };
  }

  /**
   * C19: живой MESSAGE_ACK очищает unread-индикатор канала в #channel-list.
   * @param {import("./dispatch-model.js").MessageAckChange} change
   * @returns {number}
   */
  function applyMessageAck(change) {
    const tile = getChannelTile(change.channelId);
    if (tile) {
      tile.classList.remove("is-unread");
      tile.removeAttribute("data-unread");
      tile.removeAttribute("data-mention-count");
      tile.querySelector(".channel-badge")?.remove();
    }
    if (change.channelId === currentChannelId()) {
      const band = /** @type {HTMLElement | null} */ (doc?.querySelector(".chat-unread-divider") ?? null);
      if (band) {
        band.setAttribute("hidden", "");
        band.style.display = "none";
        band.setAttribute("data-cleared", "true");
      }
    }
    return 1;
  }

  /**
   * C20: живой TYPING_START показывает/обновляет индикатор печатающего пользователя.
   * @param {import("./dispatch-model.js").TypingChange} change
   * @returns {number}
   */
  function applyTyping(change) {
    let users = activeTyping.get(change.channelId);
    if (!users) {
      users = new Set();
      activeTyping.set(change.channelId, users);
    }
    users.add(change.userId);

    if (change.channelId === currentChannelId()) {
      const indicator = getTypingIndicator();
      if (indicator) {
        indicator.removeAttribute("hidden");
        indicator.setAttribute("data-visible", "true");
        indicator.setAttribute("data-count", String(users.size));
        indicator.setAttribute("data-typing-users", [...users].join(","));
        const text = indicator.querySelector(".typing-indicator-text") ?? indicator;
        text.textContent = users.size > 1 ? "Несколько человек печатают…" : "Печатает…";
      }
      const composer = getComposer();
      composer?.setAttribute("data-typing-users", [...users].join(","));
      const chatView = getChatView();
      chatView?.setAttribute("data-typing-users", [...users].join(","));
    }
    return 1;
  }

  /**
   * C21: живой VOICE_STATE_UPDATE обновляет счётчик участников и статус стрима.
   * @param {import("./dispatch-model.js").VoiceStateChange} change
   * @returns {number}
   */
  function applyVoiceState(change) {
    let guildMap = voiceParticipants.get(change.guildId);
    if (!guildMap) {
      guildMap = new Map();
      voiceParticipants.set(change.guildId, guildMap);
    }
    if (change.channelId === null) {
      guildMap.delete(change.userId);
    } else {
      guildMap.set(change.userId, change);
    }
    const count = guildMap.size;
    const hasStream = [...guildMap.values()].some((p) => p.selfStream);

    const voicePanel = getVoicePanel();
    if (voicePanel) {
      voicePanel.setAttribute("data-voice-participants", String(count));
      voicePanel.setAttribute("data-stream-status", hasStream ? "live" : "idle");
      voicePanel.setAttribute("data-self-stream", (change.selfStream || hasStream) ? "true" : "false");
    }

    if (change.channelId) {
      const tile = getChannelTile(change.channelId);
      if (tile) {
        tile.setAttribute("data-voice-participants", String(count));
        tile.setAttribute("data-stream-status", hasStream ? "live" : "idle");
      }
    }
    options.onVoiceStateChange?.(change);
    return 1;
  }

  /**
   * C21: живой VOICE_SERVER_UPDATE обновляет endpoint голосового сервера.
   * @param {import("./dispatch-model.js").VoiceServerChange} change
   * @returns {number}
   */
  function applyVoiceServer(change) {
    const voicePanel = getVoicePanel();
    if (voicePanel) {
      if (change.endpoint) {
        voicePanel.setAttribute("data-endpoint", change.endpoint);
        voicePanel.setAttribute("data-voice-server", "connected");
      } else {
        voicePanel.removeAttribute("data-endpoint");
        voicePanel.setAttribute("data-voice-server", "idle");
      }
    }
    options.onVoiceServerChange?.(change);
    return 1;
  }

  /**
   * C22: живой THREAD_LIST_SYNC обновляет счётчик активных тредов гильдии/канала.
   * @param {import("./dispatch-model.js").ThreadsChange} change
   * @returns {number}
   */
  function applyThreads(change) {
    const threads = Array.isArray(change.threads) ? change.threads : [];
    const activeThreads = threads.filter((t) => !t.archived);
    const totalActive = activeThreads.length;
    threadCounts.set(change.guildId, totalActive);

    // 1. Кнопка веток
    const threadsBtn = doc?.querySelector('[data-action="threads"]');
    if (threadsBtn) {
      threadsBtn.setAttribute("data-thread-count", String(totalActive));
      threadsBtn.setAttribute("aria-label", `Ветки (${totalActive})`);
    }

    // 2. Дерево каналов
    const tree = doc?.getElementById("channel-tree");
    tree?.setAttribute("data-active-threads", String(totalActive));

    // 3. Подсчёт по родительским каналам
    /** @type {Map<string, number>} */
    const byParent = new Map();
    for (const thread of activeThreads) {
      if (thread.parentId) {
        byParent.set(thread.parentId, (byParent.get(thread.parentId) ?? 0) + 1);
      }
    }
    for (const thread of threads) {
      if (thread.parentId) {
        const tile = getChannelTile(thread.parentId);
        if (tile) {
          const count = byParent.get(thread.parentId) ?? 0;
          tile.setAttribute("data-thread-count", String(count));
        }
      }
    }
    return 1;
  }

  /**
   * Применить пачку изменений. Ошибка одного change не роняет остальные
   * (N13); мост считает `failed` и продолжает.
   * @param {import("./dispatch-model.js").DispatchChange[]} changes
   * @returns {{ applied: number, ignored: number, failed: number, notOpen: number, notLoaded: number }}
   */
  function apply(changes) {
    let applied = 0;
    let ignored = 0;
    let failed = 0;
    let notOpen = 0;
    let notLoaded = 0;
    for (const change of Array.isArray(changes) ? changes : []) {
      try {
        switch (change.type) {
          case "guilds":
            applyGuilds(change);
            applied += 1;
            break;
          case "members":
            applied += applyMembers(change);
            break;
          case "presence":
            applied += applyPresence(change);
            break;
          case "reaction": {
            const result = applyReaction(change);
            applied += result.applied;
            notOpen += result.notOpen;
            notLoaded += result.notLoaded;
            break;
          }
          case "message_create":
            applied += applyMessageCreate(change);
            break;
          case "message_update": {
            const result = applyMessageUpdate(change);
            applied += result.applied;
            notLoaded += result.notLoaded;
            break;
          }
          case "message_ack":
            applied += applyMessageAck(change);
            break;
          case "typing":
            applied += applyTyping(change);
            break;
          case "voice_state":
            applied += applyVoiceState(change);
            break;
          case "voice_server":
            applied += applyVoiceServer(change);
            break;
          case "threads":
            applied += applyThreads(change);
            break;
          default:
            ignored += 1;
            break;
        }
      } catch {
        failed += 1;
      }
    }
    return { applied, ignored, failed, notOpen, notLoaded };
  }

  return {
    apply,
    selectGuild,
    setSource,
    /** @returns {string} */
    get selectedGuildId() {
      return currentSelection();
    },
  };
}

/** @typedef {ReturnType<typeof createDispatchView>} DispatchView */
