// @ts-check
/**
 * Guild channel tree controller (`channels(guildId)` -> grouped/categorized DOM tree).
 * Extracted from `app.js` as part of RW51-EPIC2-T3.
 * @module ui/features/channel-tree-controller
 */

import { byId } from "./dom.js";
import { UIError } from "./errors.js";
import { renderChannelNode } from "./channel-node-renderer.js";

/**
 * Channel kind descriptors from READY (`G6-REPORT §3.4`): only `0` (text) and `5`
 * (announcement) open chat directly; `2` voice, `4` category, `15` forum remain
 * visible with a reason or live handler.
 * @typedef {{ key: string, title: string, icon: string, openable: boolean, reason: string }} ChannelKind
 */

/**
 * @typedef {{
 *   id: string,
 *   type: number,
 *   name: string,
 *   parentId: string,
 *   position: number,
 *   lastMessageId?: string,
 *   unread?: boolean,
 *   mentionCount?: number,
 *   permissionOverwrites?: Array<any>,
 *   userLimit?: number,
 * }} TreeChannel
 */

/** @type {Record<number, ChannelKind>} */
export const CHANNEL_KINDS = {
  0: { key: "text", title: "ТЕКСТОВЫЕ КАНАЛЫ", icon: "#", openable: true, reason: "" },
  5: { key: "announcement", title: "ОБЪЯВЛЕНИЯ", icon: "📢", openable: true, reason: "" },
  2: {
    key: "voice",
    title: "ГОЛОСОВЫЕ КАНАЛЫ",
    icon: "🔊",
    openable: false,
    reason: "Голосовой канал: подключение голоса в спайке недоступно",
  },
  4: {
    key: "category",
    title: "КАТЕГОРИИ",
    icon: "🗂",
    openable: false,
    reason: "Категория: чат не открывается",
  },
  15: {
    key: "forum",
    title: "ФОРУМЫ",
    icon: "💬",
    openable: false,
    reason: "Форум: вид веток — Phase 3",
  },
};

/** Display order of channel groups (not payload order): text channels above service groups. */
export const CHANNEL_GROUP_ORDER = [0, 5, 15, 2, 4];

/** Fallback descriptor for unknown channel types: visible, does not open chat. */
export const UNKNOWN_CHANNEL_KIND = Object.freeze({
  key: "unknown",
  title: "ДРУГИЕ КАНАЛЫ",
  icon: "•",
  openable: false,
  reason: "Неизвестный тип канала: чат не открывается",
});

const PREFERRED_GUILD_CHANNELS = new Set([
  "1163408591315812392",
  "429222300177399823",
  "274093052673392641",
  "816010274154479666",
  "703970377721970768",
  "1014537851867775060",
]);

/**
 * Format user-visible failure text for `channels()` DataError without leaking body or token.
 * @param {unknown} error
 * @returns {string}
 */
export function channelTreeFailureText(error) {
  const bag =
    error !== null && typeof error === "object"
      ? /** @type {Record<string, any>} */ (error)
      : {};
  const kind = String(bag.kind ?? "");
  const status = Number(bag.status ?? bag.code ?? 0);
  if (status === 401 || /unauthor/i.test(kind)) return "Сессия истекла. Войдите снова.";
  if (/notfound/i.test(kind) || status === 404) return "Сервер не найден в локальном снимке";
  if (/parse|io/i.test(kind)) return "Не удалось прочитать каналы сервера";
  return "Каналы сервера недоступны";
}

/**
 * Create the guild channel tree controller bound to `#channel-tree` and `.tree-scroll`.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   onOpen?: (channel: { id: string, name: string }) => void,
 *   onContextMenu?: (channel: TreeChannel, button: HTMLElement) => void,
 *   onRenderChannels?: (guildId: string, channels: TreeChannel[]) => void,
 *   getVoiceParticipants?: (guildId: string, channelId: string) => Array<any>,
 *   isVoiceChannelConnected?: (guildId: string, channelId: string) => boolean,
 *   getVoiceChannelTimer?: (guildId: string, channelId: string, count: number) => string,
 *   getAloneInviteRemainingMs?: (guildId: string, channelId: string) => number,
 *   isChannelRoleRestricted?: (guildId: string, channel: TreeChannel, allChannels: TreeChannel[]) => boolean,
 *   canViewChannel?: (guildId: string, channel: TreeChannel, allChannels: TreeChannel[]) => boolean,
 *   onInviteVoice?: (channel: TreeChannel, isRestricted: boolean) => void,
 *   onVoiceParticipantContextMenu?: (participant: any, channel: TreeChannel, rowEl: HTMLElement, ev: MouseEvent) => void,
 *   onVoiceParticipantHoverActivity?: (participant: any, rowEl: HTMLElement, show: boolean) => void,
 *   onWatchVoiceStream?: (participant: any, channel: TreeChannel) => void,
 *   onSelectVoice?: (channel: TreeChannel, button: HTMLElement) => void,
 * }} options
 */
export function createChannelTree(options) {
  const { invoke } = options;
  const isLiveSession = () => typeof globalThis.Image === "function";

  const treeNode = byId("channel-tree");
  if (!treeNode) throw new UIError("ELEMENT_MISSING", "в DOM нет #channel-tree");
  /** @type {HTMLElement} */
  const tree = treeNode;
  const scrollNode = tree.querySelector(".tree-scroll");
  if (!scrollNode) throw new UIError("ELEMENT_MISSING", "в дереве нет .tree-scroll");
  /** @type {HTMLElement} */
  const scroll = /** @type {HTMLElement} */ (scrollNode);

  const note = document.createElement("p");
  note.className = "channel-note";
  note.setAttribute("role", "status");
  note.setAttribute("aria-live", "polite");
  note.textContent = "Каналы загружаются вместе с сервером";

  /** @type {"idle" | "loading" | "ready" | "empty" | "error"} */
  let state = "idle";
  /** @type {string} */
  let guildId = "";
  /** @type {TreeChannel[]} */
  let channels = [];
  /** Load token so stale responses from a previous guild never overwrite a newer load. */
  let loadToken = 0;
  let disposed = false;

  /** @param {"idle" | "loading" | "ready" | "empty" | "error"} next */
  function setState(next) {
    state = next;
    tree.setAttribute("data-state", next);
    if (next === "loading") tree.setAttribute("aria-busy", "true");
    else tree.removeAttribute("aria-busy");
  }

  /** @returns {HTMLElement[]} */
  function channelButtons() {
    return /** @type {HTMLElement[]} */ ([...scroll.querySelectorAll(".channel-item")]);
  }

  /** @returns {HTMLElement[]} */
  function openableButtons() {
    return /** @type {HTMLElement[]} */ ([
      ...scroll.querySelectorAll('.channel-item[data-openable="true"]'),
    ]);
  }

  /**
   * Roving tabindex: only the focused (or first) tile participates in Tab order.
   * @param {HTMLElement | null} focused
   */
  function syncTabindex(focused) {
    const items = openableButtons();
    for (const item of items) item.setAttribute("tabindex", item === focused ? "0" : "-1");
  }

  /** @param {HTMLElement} item */
  function selectItem(item) {
    for (const other of channelButtons()) {
      if (other === item) other.setAttribute("aria-current", "page");
      else other.removeAttribute("aria-current");
    }
    syncTabindex(item);
  }

  /**
   * Activates or opens the selected channel, voice room, forum, or category toggle.
   * @param {TreeChannel} channel
   * @param {ChannelKind} kind
   * @param {HTMLElement} button
   */
  function activate(channel, kind, button) {
    const name = channel.name || channel.id;
    if (!kind.openable) {
      if (isLiveSession() && kind.key === "voice" && typeof options.onSelectVoice === "function") {
        selectItem(button);
        note.textContent = `Голосовой канал «${name}» подключён`;
        options.onSelectVoice(channel, button);
        return;
      }
      if (isLiveSession() && kind.key === "forum") {
        selectItem(button);
        note.textContent = `Открыт форум «${name}»`;
        options.onOpen?.({ id: channel.id, name });
        return;
      }
      if (isLiveSession() && kind.key === "category") {
        note.textContent = `Категория «${name}»`;
        const parentLi = button.parentElement;
        if (parentLi) {
          const existingSub = parentLi.querySelector(".category-inline-children");
          if (existingSub) {
            existingSub.remove();
            button.setAttribute("aria-expanded", "false");
            return;
          }
          const childRows = channels.filter(
            (ch) => ch.parentId === channel.id && ch.id !== channel.id && ch.name !== "___hidden___",
          );
          if (childRows.length > 0) {
            button.setAttribute("aria-expanded", "true");
            const subList = document.createElement("ul");
            subList.className = "channel-list category-inline-children";
            subList.setAttribute("role", "list");
            subList.style.margin = "4px 0 8px 14px";
            subList.style.paddingLeft = "8px";
            subList.style.borderLeft = "2px solid var(--divider)";
            for (const ch of childRows) {
              subList.append(channelNode(ch));
            }
            parentLi.append(subList);
            const firstText = childRows.find((ch) => ch.type === 0 || ch.type === 5);
            if (firstText) {
              options.onOpen?.({ id: firstText.id, name: firstText.name || firstText.id });
            }
            return;
          }
        }
        return;
      }
      button.setAttribute("data-denied", "true");
      note.textContent = kind.reason;
      return;
    }
    selectItem(button);
    note.textContent = `Открыт канал «${name}»`;
    options.onOpen?.({ id: channel.id, name });
  }

  /**
   * @param {"ArrowDown" | "ArrowUp" | "Home" | "End"} key
   * @param {HTMLElement} from
   */
  function moveFocus(key, from) {
    const items = openableButtons();
    if (items.length === 0) return;
    const index = items.indexOf(from);
    let next = 0;
    if (key === "ArrowDown") next = index < 0 ? 0 : (index + 1) % items.length;
    else if (key === "ArrowUp") next = index <= 0 ? items.length - 1 : index - 1;
    else if (key === "End") next = items.length - 1;
    const target = items[next];
    if (!target) return;
    syncTabindex(target);
    if (typeof target.focus === "function") target.focus();
  }

  /**
   * @param {TreeChannel} channel
   * @returns {HTMLElement}
   */
  function channelNode(channel) {
    return renderChannelNode({
      channel,
      guildId,
      channels,
      channelKinds: CHANNEL_KINDS,
      unknownChannelKind: UNKNOWN_CHANNEL_KIND,
      isLiveSession,
      options,
      onActivate: activate,
      onMoveFocus: moveFocus,
    });
  }

  /**
   * Build a collapsible section for either a channel type group or a category.
   * @param {{
   *   dataType: string,
   *   categoryId?: string,
   *   titleText: string,
   *   items: TreeChannel[],
   *   isFirstGroup: boolean,
   * }} cfg
   * @returns {HTMLElement}
   */
  function buildCollapsibleSection({ dataType, categoryId, titleText, items, isFirstGroup }) {
    const section = document.createElement("section");
    section.className = "channel-group";
    section.setAttribute("data-channel-type", dataType);
    if (categoryId) section.setAttribute("data-category-id", categoryId);
    const heading = document.createElement("h3");
    heading.className = "channel-group-title";
    const toggle = document.createElement("button");
    toggle.className = "group-toggle";
    toggle.type = "button";
    toggle.setAttribute("aria-expanded", "true");
    toggle.textContent = titleText;
    const list = document.createElement("ul");
    if (isFirstGroup) list.id = "channel-list";
    list.className = "channel-list";
    list.setAttribute("role", "list");
    for (const channel of items) list.append(channelNode(channel));
    toggle.addEventListener("click", () => {
      const expanded = toggle.getAttribute("aria-expanded") !== "false";
      toggle.setAttribute("aria-expanded", expanded ? "false" : "true");
      if (expanded) list.setAttribute("hidden", "");
      else list.removeAttribute("hidden");
    });
    heading.append(toggle);
    section.append(heading, list);
    return section;
  }

  /**
   * @param {number} type
   * @param {TreeChannel[]} items
   * @param {boolean} [isFirstGroup]
   * @returns {HTMLElement}
   */
  function groupSection(type, items, isFirstGroup = false) {
    const kind = CHANNEL_KINDS[type] ?? UNKNOWN_CHANNEL_KIND;
    return buildCollapsibleSection({
      dataType: String(type),
      titleText: kind.title,
      items,
      isFirstGroup,
    });
  }

  function appendHiddenChannelListFallback() {
    if (scroll.querySelector("#channel-list")) return;
    const hiddenList = document.createElement("ul");
    hiddenList.id = "channel-list";
    hiddenList.className = "channel-list";
    hiddenList.setAttribute("role", "list");
    hiddenList.setAttribute("hidden", "");
    scroll.append(hiddenList);
  }

  function appendHiddenDmList() {
    appendHiddenChannelListFallback();
    const hiddenDm = document.createElement("ul");
    hiddenDm.id = "dm-channel-list";
    hiddenDm.className = "channel-list dm-channel-list";
    hiddenDm.setAttribute("role", "list");
    hiddenDm.setAttribute("hidden", "");
    scroll.append(hiddenDm);
  }

  /**
   * @param {TreeChannel} category
   * @param {TreeChannel[]} items
   * @param {boolean} [isFirstGroup]
   * @returns {HTMLElement}
   */
  function categorySection(category, items, isFirstGroup = false) {
    return buildCollapsibleSection({
      dataType: "4",
      categoryId: category.id,
      titleText: (category.name || category.id).toUpperCase(),
      items,
      isFirstGroup,
    });
  }

  /** @param {TreeChannel[]} rows */
  function renderTree(rows) {
    channels = rows;
    if (tree.getAttribute("data-nav-mode") === "home") return;
    const prevScrollTop = scroll.scrollTop;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Выберите канал";
    const visibleRows = isLiveSession()
      ? rows.filter((ch) => {
          if (ch.name === "___hidden___") return false;
          if (typeof options.canViewChannel === "function") {
            return Boolean(options.canViewChannel(guildId, ch, rows));
          }
          return true;
        })
      : rows;
    let firstGroupRendered = false;
    const liveCategories = isLiveSession()
      ? visibleRows.filter((ch) => ch.type === 4).sort((a, b) => a.position - b.position)
      : [];
    if (isLiveSession() && liveCategories.length > 0) {
      const catIds = new Set(liveCategories.map((c) => c.id));
      const nonCatRows = visibleRows.filter((ch) => ch.type !== 4);
      /** @param {TreeChannel[]} list */
      const sortCatChildren = (list) =>
        [...list].sort((a, b) => {
          const aVoice = a.type === 2 || a.type === 13 ? 1 : 0;
          const bVoice = b.type === 2 || b.type === 13 ? 1 : 0;
          if (aVoice !== bVoice) return aVoice - bVoice;
          return a.position - b.position;
        });
      const uncategorized = sortCatChildren(
        nonCatRows.filter((ch) => !ch.parentId || !catIds.has(ch.parentId)),
      );
      if (uncategorized.length > 0) {
        const topSec = document.createElement("section");
        topSec.className = "channel-group";
        topSec.setAttribute("data-channel-type", "0");
        const topList = document.createElement("ul");
        topList.id = "channel-list";
        topList.className = "channel-list";
        topList.setAttribute("role", "list");
        for (const ch of uncategorized) topList.append(channelNode(ch));
        topSec.append(topList);
        scroll.append(topSec);
        firstGroupRendered = true;
      }
      for (const cat of liveCategories) {
        const children = sortCatChildren(nonCatRows.filter((ch) => ch.parentId === cat.id));
        if (children.length === 0) continue;
        scroll.append(categorySection(cat, children, !firstGroupRendered));
        firstGroupRendered = true;
      }
    } else {
      /** @type {Map<number, TreeChannel[]>} */
      const buckets = new Map();
      for (const channel of visibleRows) {
        const bucket = buckets.get(channel.type);
        if (bucket) bucket.push(channel);
        else buckets.set(channel.type, [channel]);
      }
      const extra = [...buckets.keys()].filter((type) => !CHANNEL_GROUP_ORDER.includes(type));
      for (const type of [...CHANNEL_GROUP_ORDER, ...extra]) {
        const items = buckets.get(type);
        if (!items || items.length === 0) continue;
        scroll.append(groupSection(type, items, !firstGroupRendered));
        firstGroupRendered = true;
      }
    }
    appendHiddenDmList();
    const first = openableButtons()[0] ?? null;
    syncTabindex(first);
    setState("ready");
    if (prevScrollTop > 0) {
      scroll.scrollTop = prevScrollTop;
    }
    options.onRenderChannels?.(guildId, visibleRows);
  }

  function renderEmpty() {
    channels = [];
    if (tree.getAttribute("data-nav-mode") === "home") return;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "В этом сервере нет каналов";
    const empty = document.createElement("p");
    empty.className = "channel-tree-state";
    empty.setAttribute("data-state", "empty");
    empty.setAttribute("role", "status");
    empty.textContent = "Каналы не найдены";
    scroll.append(empty);
    appendHiddenDmList();
    setState("empty");
  }

  /** @param {unknown} error */
  function renderError(error) {
    channels = [];
    if (tree.getAttribute("data-nav-mode") === "home") return;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Каналы не загрузились";
    const node = document.createElement("div");
    node.className = "channel-tree-error";
    node.setAttribute("data-state", "error");
    node.setAttribute("role", "alert");
    node.setAttribute("data-error-code", "IPC_FAILED");
    const title = document.createElement("p");
    title.className = "channel-tree-error-title";
    title.textContent = "Не удалось загрузить каналы";
    const hint = document.createElement("p");
    hint.className = "channel-tree-error-hint";
    hint.textContent = channelTreeFailureText(error);
    const retry = document.createElement("button");
    retry.className = "channel-tree-retry";
    retry.type = "button";
    retry.textContent = "Повторить";
    retry.addEventListener("click", () => {
      void load(guildId);
    });
    node.append(title, hint, retry);
    scroll.append(node);
    appendHiddenDmList();
    setState("error");
  }

  function renderLoading() {
    channels = [];
    if (tree.getAttribute("data-nav-mode") === "home") return;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Загрузка каналов…";
    const loading = document.createElement("p");
    loading.className = "channel-tree-state";
    loading.setAttribute("data-state", "loading");
    loading.setAttribute("role", "status");
    loading.textContent = "Загрузка каналов…";
    scroll.append(loading);
    appendHiddenDmList();
    setState("loading");
  }

  /**
   * @param {unknown} payload
   * @returns {TreeChannel[]}
   */
  function parseChannels(payload) {
    if (!Array.isArray(payload)) {
      throw new UIError("IPC_FAILED", "channels: ответ не массив");
    }
    return payload.map((value, index) => {
      const bag =
        value !== null && typeof value === "object"
          ? /** @type {Record<string, any>} */ (value)
          : {};
      const id = typeof bag.id === "string" ? bag.id : "";
      const type = Number(bag.type);
      if (id === "" || !Number.isInteger(type)) {
        throw new UIError("IPC_FAILED", `channels: канал ${index} без строкового id/type`);
      }
      const lastMessageId =
        typeof bag.last_message_id === "string"
          ? bag.last_message_id
          : typeof bag.lastMessageId === "string"
            ? bag.lastMessageId
            : "";
      const unread = typeof bag.unread === "boolean" ? bag.unread : undefined;
      const mentionCount =
        typeof bag.mention_count === "number"
          ? bag.mention_count
          : typeof bag.mentionCount === "number"
            ? bag.mentionCount
            : undefined;
      const permissionOverwrites = Array.isArray(bag.permissionOverwrites)
        ? bag.permissionOverwrites
        : Array.isArray(bag.permission_overwrites)
          ? bag.permission_overwrites
          : undefined;
      const userLimit =
        typeof bag.userLimit === "number"
          ? bag.userLimit
          : typeof bag.user_limit === "number"
            ? bag.user_limit
            : undefined;
      const rawParentId =
        typeof bag.parentId === "string"
          ? bag.parentId
          : typeof bag.parent_id === "string"
            ? bag.parent_id
            : "";
      return {
        id,
        type,
        name: typeof bag.name === "string" && bag.name !== "nil" ? bag.name : "",
        parentId: rawParentId !== "nil" ? rawParentId : "",
        position: typeof bag.position === "number" ? bag.position : 0,
        ...(lastMessageId && lastMessageId !== "nil" ? { lastMessageId } : {}),
        ...(unread !== undefined ? { unread } : {}),
        ...(mentionCount !== undefined ? { mentionCount } : {}),
        ...(permissionOverwrites !== undefined ? { permissionOverwrites } : {}),
        ...(userLimit !== undefined ? { userLimit } : {}),
      };
    });
  }

  /** @param {string} id */
  async function load(id) {
    if (disposed) return;
    const next = String(id ?? "").trim();
    if (next === "") {
      renderEmpty();
      return;
    }
    guildId = next;
    const token = ++loadToken;
    renderLoading();
    try {
      const payload = await invoke("channels", { guildId: next });
      if (disposed || token !== loadToken) return;
      const rows = parseChannels(payload);
      if (rows.length === 0) {
        renderEmpty();
      } else {
        renderTree(rows);
        if (isLiveSession() && tree.getAttribute("data-nav-mode") !== "home") {
          const openable = openableButtons();
          const preferredBtn =
            openable.find((btn) =>
              PREFERRED_GUILD_CHANNELS.has(btn.getAttribute("data-channel-id") ?? ""),
            ) ??
            openable[0] ??
            null;
          if (preferredBtn) preferredBtn.click();
        }
      }
    } catch (error) {
      if (disposed || token !== loadToken) return;
      renderError(error);
    }
  }

  function refreshView() {
    if (disposed || state !== "ready" || channels.length === 0) return;
    renderTree(channels);
  }

  return {
    node: scroll,
    load,
    refreshView,
    /** @returns {"idle" | "loading" | "ready" | "empty" | "error"} */
    get state() {
      return state;
    },
    /** @returns {TreeChannel[]} */
    get channels() {
      return channels;
    },
    /** Soft reset on account switch: clears channels and DOM without setting disposed=true. */
    reset() {
      channels = [];
      state = "idle";
      scroll.replaceChildren();
    },
    dispose() {
      disposed = true;
      channels = [];
      scroll.replaceChildren();
    },
  };
}
