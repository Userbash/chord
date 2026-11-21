// @ts-check
/**
 * T-220 (FE-P1-20 / RW-40 / G2): контроллер списка участников гильдии,
 * ролевых групп и правого сайдбара присутствия (`member-list.js`).
 *
 * RE-оракул:
 * - `/home/sanya/re-deep/frontend-d03.md` §3 (`aside[class*=membersWrap]`,
 *   группы `NAME — COUNT`, строки участников с именем, цветом роли, статусом
 *   присутствия, короной владельца и пользовательским статусом);
 * - `/home/sanya/re-deep/ui8/22-members.json`.
 *
 * @module ui/features/member-list
 */

import { createVirtualizer, isTanstackVirtualPowered } from "./virtualizer.js";
import {
  computeFloatingPosition,
  positionFloatingElement,
  isFloatingUiDomPowered,
} from "./floating-popover.js";

export { isTanstackVirtualPowered, isFloatingUiDomPowered };

/**
 * Высота заголовка ролевой группы по умолчанию (px).
 * @type {number}
 */
export const DEFAULT_GROUP_HEADER_HEIGHT_PX = 28;

/**
 * Высота строки участника по умолчанию (px).
 * @type {number}
 */
export const DEFAULT_MEMBER_ROW_HEIGHT_PX = 42;

/**
 * Высота вьюпорта по умолчанию (px).
 * @type {number}
 */
export const DEFAULT_VIEWPORT_HEIGHT_PX = 600;

/**
 * Количество запасных строк сверх видимого окна.
 * @type {number}
 */
export const DEFAULT_OVERSCAN = 8;

/**
 * Русские заголовки стандартных групп участников («В сети» и «Не в сети»).
 * @type {Readonly<{ online: string, offline: string }>}
 */
export const DEFAULT_GROUP_LABELS_RU = Object.freeze({
  online: "В сети",
  offline: "Не в сети",
});

/**
 * Человекочитаемые подписи статусов присутствия (включая mobile и streaming).
 * @type {Readonly<Record<string, string>>}
 */
export const PRESENCE_STATUS_LABELS = Object.freeze({
  online: "В сети",
  idle: "Неактивен",
  dnd: "Не беспокоить",
  offline: "Не в сети",
  mobile: "В сети (с телефона)",
  streaming: "Стримит",
});

/**
 * Допустимые активные статусы присутствия (не считающиеся `offline`).
 * @type {ReadonlySet<string>}
 */
const ONLINE_PRESENCE_STATUSES = new Set([
  "online",
  "idle",
  "dnd",
  "mobile",
  "streaming",
]);

/**
 * @typedef {"online" | "idle" | "dnd" | "offline" | "mobile" | "streaming"} MemberPresenceStatus
 */

/**
 * @typedef {object} GuildMemberEntry
 * @property {string} userId
 * @property {string} username
 * @property {string} [displayName]
 * @property {MemberPresenceStatus | string} [status]
 * @property {ReadonlyArray<string>} [roleIds]
 * @property {string} [customStatus]
 * @property {boolean} [isOwner]
 */

/**
 * @typedef {object} GuildRoleEntry
 * @property {string} id
 * @property {string} name
 * @property {number} position
 * @property {boolean} [hoist]
 * @property {number} [color]
 */

/**
 * @typedef {object} MemberRoleGroup
 * @property {string} id
 * @property {string} label
 * @property {number} count
 * @property {GuildMemberEntry[]} members
 */

/**
 * @typedef {object} SelectMemberPayload
 * @property {string} userId
 * @property {any} anchorElement
 */

/**
 * @typedef {object} MemberContextMenuPayload
 * @property {string} userId
 * @property {GuildMemberEntry} member
 * @property {any} anchorElement
 * @property {any} virtualReference
 * @property {{ x: number, y: number, placement: string }} position
 * @property {any} event
 */

/**
 * @typedef {object} MemberHoverPayload
 * @property {string} userId
 * @property {GuildMemberEntry} member
 * @property {any} anchorElement
 * @property {{ x: number, y: number, placement: string }} position
 * @property {any} event
 */

/**
 * @typedef {object} MemberListControllerOptions
 * @property {any} [container]
 * @property {ReadonlyArray<GuildMemberEntry>} [members]
 * @property {ReadonlyArray<GuildRoleEntry>} [roles]
 * @property {boolean} [visible]
 * @property {number} [viewportHeight]
 * @property {number} [rowHeight]
 * @property {number} [headerHeight]
 * @property {number} [overscan]
 * @property {any} [contextMenuEl]
 * @property {any} [hoverCardEl]
 * @property {(payload: SelectMemberPayload) => void} [onSelectMember]
 * @property {(payload: MemberContextMenuPayload) => void} [onMemberContextMenu]
 * @property {(payload: MemberHoverPayload) => void} [onMemberHover]
 * @property {(payload: { userId: string, member: GuildMemberEntry, anchorElement: any, event: any }) => void} [onMemberHoverEnd]
 */

/**
 * Нормализует статус присутствия участника.
 *
 * @param {unknown} rawStatus
 * @returns {MemberPresenceStatus}
 */
function normalizePresenceStatus(rawStatus) {
  if (typeof rawStatus === "string" && ONLINE_PRESENCE_STATUSES.has(rawStatus)) {
    return /** @type {MemberPresenceStatus} */ (rawStatus);
  }
  return "offline";
}

/**
 * Возвращает ключ сортировки участника (`displayName || username`).
 *
 * @param {GuildMemberEntry} member
 * @returns {string}
 */
function getMemberSortName(member) {
  const display = typeof member?.displayName === "string" ? member.displayName.trim() : "";
  if (display.length > 0) {
    return display;
  }
  return typeof member?.username === "string" ? member.username : "";
}

const RU_COLLATOR = new Intl.Collator("ru");

/**
 * Сравнивает двух участников по алфавиту (`displayName || username`) через `Intl.Collator('ru')`.
 *
 * @param {GuildMemberEntry} a
 * @param {GuildMemberEntry} b
 * @returns {number}
 */
function compareMembersRu(a, b) {
  return RU_COLLATOR.compare(getMemberSortName(a), getMemberSortName(b));
}

/**
 * Преобразует числовой 24-битный RGB-цвет роли в `#rrggbb` (или `null`, если цвет не задан).
 *
 * @param {unknown} rawColor
 * @returns {string | null}
 */
function formatRoleHexColor(rawColor) {
  if (typeof rawColor === "string") {
    const trimmed = rawColor.trim();
    if (/^#[0-9a-fA-F]{6}$/.test(trimmed)) {
      return trimmed.toLowerCase();
    }
    return null;
  }
  if (
    typeof rawColor !== "number" ||
    !Number.isFinite(rawColor) ||
    rawColor <= 0
  ) {
    return null;
  }
  const clamped = Math.trunc(rawColor) & 0xffffff;
  if (clamped <= 0) {
    return null;
  }
  return `#${clamped.toString(16).padStart(6, "0")}`;
}

/**
 * Определяет HEX-цвет самой старшей по `position` цветной роли (`color > 0`) участника.
 *
 * @param {GuildMemberEntry} member
 * @param {ReadonlyArray<GuildRoleEntry>} roles
 * @returns {string | null}
 */
function resolveMemberRoleHexColor(member, roles) {
  if (!member || !Array.isArray(member.roleIds) || member.roleIds.length === 0) {
    return null;
  }
  if (!Array.isArray(roles) || roles.length === 0) {
    return null;
  }
  const roleIdSet = new Set(member.roleIds);
  /** @type {GuildRoleEntry | null} */
  let bestRole = null;

  for (const role of roles) {
    if (!role || typeof role.id !== "string" || !roleIdSet.has(role.id)) {
      continue;
    }
    const hex = formatRoleHexColor(role.color);
    if (!hex) {
      continue;
    }
    const pos = typeof role.position === "number" && Number.isFinite(role.position) ? role.position : 0;
    const bestPos =
      bestRole && typeof bestRole.position === "number" && Number.isFinite(bestRole.position)
        ? bestRole.position
        : -Infinity;
    if (!bestRole || pos > bestPos) {
      bestRole = role;
    }
  }

  return bestRole ? formatRoleHexColor(bestRole.color) : null;
}

/**
 * Группирует участников гильдии по hoisted-ролям (по убыванию `role.position`),
 * а также в корзины `'online'` («В сети») и `'offline'` («Не в сети»).
 *
 * Правила:
 * - Участники со `status === 'offline'` или отсутствующим статусом попадают в корзину `'offline'` (`Не в сети`).
 * - Участники со статусом `'online' | 'idle' | 'dnd'` попадают в корзину самой старшей
 *   по `position` роли с `hoist === true`, которой они обладают, либо в `'online'` (`В сети`),
 *   если у них нет ни одной hoisted-роли.
 * - Внутри каждой корзины участники сортируются по `(displayName || username)` через `localeCompare('ru')`.
 * - Возвращаются только непустые группы `[{ id, label, count, members }]`.
 *
 * @param {ReadonlyArray<GuildMemberEntry>} members
 * @param {ReadonlyArray<GuildRoleEntry>} roles
 * @returns {MemberRoleGroup[]}
 */
export function groupMembersByHoistedRole(members, roles) {
  if (!Array.isArray(members) || members.length === 0) {
    return [];
  }

  const safeRoles = Array.isArray(roles) ? roles : [];

  // Отбираем hoisted-роли и сортируем по position по убыванию
  const hoistedRoles = safeRoles
    .filter((role) => Boolean(role && typeof role.id === "string" && role.hoist === true))
    .slice()
    .sort((a, b) => {
      const posA = typeof a.position === "number" && Number.isFinite(a.position) ? a.position : 0;
      const posB = typeof b.position === "number" && Number.isFinite(b.position) ? b.position : 0;
      return posB - posA;
    });

  /** @type {Map<string, GuildMemberEntry[]>} */
  const hoistedBuckets = new Map();
  for (const role of hoistedRoles) {
    hoistedBuckets.set(role.id, []);
  }

  /** @type {GuildMemberEntry[]} */
  const onlineBucket = [];
  /** @type {GuildMemberEntry[]} */
  const offlineBucket = [];

  for (const rawMember of members) {
    if (!rawMember || typeof rawMember !== "object") {
      continue;
    }
    const status = normalizePresenceStatus(rawMember.status);
    if (status === "offline") {
      offlineBucket.push(rawMember);
      continue;
    }

    const memberRoleIds = Array.isArray(rawMember.roleIds)
      ? new Set(rawMember.roleIds)
      : new Set();

    /** @type {GuildRoleEntry | null} */
    let targetHoistedRole = null;
    for (const hoistedRole of hoistedRoles) {
      if (memberRoleIds.has(hoistedRole.id)) {
        targetHoistedRole = hoistedRole;
        break;
      }
    }

    if (targetHoistedRole) {
      const bucket = hoistedBuckets.get(targetHoistedRole.id);
      if (bucket) {
        bucket.push(rawMember);
      } else {
        onlineBucket.push(rawMember);
      }
    } else {
      onlineBucket.push(rawMember);
    }
  }

  /** @type {MemberRoleGroup[]} */
  const groups = [];

  for (const role of hoistedRoles) {
    const bucket = hoistedBuckets.get(role.id);
    if (!bucket || bucket.length === 0) {
      continue;
    }
    const sorted = bucket.slice().sort(compareMembersRu);
    groups.push({
      id: role.id,
      label: String(role.name ?? role.id),
      count: sorted.length,
      members: sorted,
    });
  }

  if (onlineBucket.length > 0) {
    const sortedOnline = onlineBucket.slice().sort(compareMembersRu);
    groups.push({
      id: "online",
      label: DEFAULT_GROUP_LABELS_RU.online,
      count: sortedOnline.length,
      members: sortedOnline,
    });
  }

  if (offlineBucket.length > 0) {
    const sortedOffline = offlineBucket.slice().sort(compareMembersRu);
    groups.push({
      id: "offline",
      label: DEFAULT_GROUP_LABELS_RU.offline,
      count: sortedOffline.length,
      members: sortedOffline,
    });
  }

  return groups;
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
 * Применяет HEX-цвет роли к элементу и его стилю.
 *
 * @param {any} el
 * @param {string} hexColor
 */
function applyElementRoleColor(el, hexColor) {
  el.setAttribute("data-role-color", hexColor);
  if (el.style) {
    if (typeof el.style.setProperty === "function") {
      el.style.setProperty("color", hexColor);
    } else {
      el.style.color = hexColor;
    }
  } else {
    el.setAttribute("style", `color: ${hexColor}`);
  }
}

/**
 * Выравнивает список ролевых групп в плоский массив элементов для виртуализации.
 *
 * @param {ReadonlyArray<MemberRoleGroup>} groups
 * @returns {Array<
 *   | { type: "header", key: string, groupId: string, label: string, count: number }
 *   | { type: "member", key: string, groupId: string, member: GuildMemberEntry }
 * >}
 */
export function flattenMemberGroups(groups) {
  /** @type {Array<{ type: "header", key: string, groupId: string, label: string, count: number } | { type: "member", key: string, groupId: string, member: GuildMemberEntry }>} */
  const flat = [];
  for (const group of groups) {
    flat.push({
      type: "header",
      key: `h:${group.id}`,
      groupId: group.id,
      label: group.label,
      count: group.count,
    });
    for (let i = 0; i < group.members.length; i++) {
      const member = group.members[i];
      if (!member) continue;
      flat.push({
        type: "member",
        key: `m:${group.id}:${String(member.userId ?? i)}`,
        groupId: group.id,
        member,
      });
    }
  }
  return flat;
}

/**
 * Вычисляет координаты плавающего элемента через `@floating-ui/dom` (с синхронным фолбэком для DOM-шимов).
 *
 * @param {any} reference
 * @param {any} event
 * @param {{ placement?: "top" | "top-start" | "top-end" | "bottom" | "bottom-start" | "bottom-end" | "right" | "left", offset?: number, padding?: number, width?: number, height?: number }} [opts]
 */
function computeAnchorPosition(reference, event, opts = {}) {
  const hasPointer =
    event &&
    typeof event.clientX === "number" &&
    typeof event.clientY === "number" &&
    (event.clientX !== 0 || event.clientY !== 0);
  const rect = hasPointer
    ? { x: event.clientX, y: event.clientY, width: 0, height: 0 }
    : typeof reference?.getBoundingClientRect === "function"
      ? reference.getBoundingClientRect()
      : { left: 0, top: 0, width: 220, height: 42 };
  const refRect = {
    x: Number(rect.left ?? rect.x ?? 0),
    y: Number(rect.top ?? rect.y ?? 0),
    width: Number(rect.width ?? 0),
    height: Number(rect.height ?? 0),
  };
  const virtualReference = {
    getBoundingClientRect: () => ({
      x: refRect.x,
      y: refRect.y,
      left: refRect.x,
      top: refRect.y,
      right: refRect.x + refRect.width,
      bottom: refRect.y + refRect.height,
      width: refRect.width,
      height: refRect.height,
    }),
  };
  const position = computeFloatingPosition(
    refRect,
    { width: opts.width ?? 220, height: opts.height ?? 180 },
    {
      placement: opts.placement ?? "bottom-start",
      offset: opts.offset ?? 6,
      padding: opts.padding ?? 8,
    },
  );
  return { virtualReference, position };
}

/**
 * Создаёт контроллер правого сайдбара списка участников гильдии
 * с виртуализацией на базе `@tanstack/virtual-core` и триггерами `@floating-ui/dom`.
 *
 * Поддерживает вызов как `createMemberListController({ container, onSelectMember, members, roles })`,
 * так и `createMemberListController(container, options)`.
 *
 * @param {MemberListControllerOptions} configOrContainer
 * @param {MemberListControllerOptions} [maybeOptions]
 */
export function createMemberListController(configOrContainer, maybeOptions = {}) {
  const rawFirst = /** @type {any} */ (configOrContainer);
  const isFirstArgContainer =
    rawFirst &&
    typeof rawFirst === "object" &&
    !("container" in rawFirst) &&
    (typeof rawFirst.appendChild === "function" ||
      typeof rawFirst.createElement === "function");

  /** @type {MemberListControllerOptions} */
  const options = isFirstArgContainer
    ? { ...maybeOptions, container: rawFirst }
    : (configOrContainer ?? {});

  const container = options.container;
  if (!container || typeof container.appendChild !== "function") {
    throw new TypeError("options.container must be a valid DOM container element");
  }
  const doc = resolveDocument(container);

  const headerHeight =
    typeof options.headerHeight === "number" && options.headerHeight > 0
      ? options.headerHeight
      : DEFAULT_GROUP_HEADER_HEIGHT_PX;
  const rowHeight =
    typeof options.rowHeight === "number" && options.rowHeight > 0
      ? options.rowHeight
      : DEFAULT_MEMBER_ROW_HEIGHT_PX;
  const overscan =
    typeof options.overscan === "number" && options.overscan >= 0
      ? options.overscan
      : DEFAULT_OVERSCAN;
  let currentViewportHeight =
    typeof options.viewportHeight === "number" && options.viewportHeight > 0
      ? options.viewportHeight
      : Number(container.clientHeight || container.boxHeight || DEFAULT_VIEWPORT_HEIGHT_PX);

  /** @type {GuildMemberEntry[]} */
  let currentMembers = Array.isArray(options.members) ? [...options.members] : [];
  /** @type {GuildRoleEntry[]} */
  let currentRoles = Array.isArray(options.roles) ? [...options.roles] : [];
  /** @type {boolean} */
  let visible = options.visible !== false;
  /** @type {MemberRoleGroup[]} */
  let currentGroups = [];
  /** @type {ReturnType<typeof flattenMemberGroups>} */
  let flatItems = [];

  const root = doc.createElement("aside");
  root.setAttribute("data-member-list", "");
  root.setAttribute("aria-label", "Участники сервера");
  root.setAttribute("role", "list");
  if (root.style && typeof root.style.setProperty === "function") {
    root.style.setProperty("overflow-y", "auto");
    root.style.setProperty("position", "relative");
  }

  const sizer = doc.createElement("div");
  sizer.setAttribute("data-member-list-sizer", "");
  if (sizer.style && typeof sizer.style.setProperty === "function") {
    sizer.style.setProperty("position", "relative");
    sizer.style.setProperty("width", "100%");
  }
  root.appendChild(sizer);

  const virtualizer = createVirtualizer({
    count: 0,
    estimateSize: (idx) => (flatItems[idx]?.type === "header" ? headerHeight : rowHeight),
    overscan,
    scrollOffset: 0,
    viewportSize: currentViewportHeight,
    getItemKey: (idx) => flatItems[idx]?.key ?? String(idx),
  });

  function syncVisibilityDom() {
    root.hidden = !visible;
    root.setAttribute("aria-hidden", visible ? "false" : "true");
    if (visible) {
      root.removeAttribute("hidden");
    } else {
      root.setAttribute("hidden", "");
    }
  }

  /**
   * @param {{ type: "header", groupId: string, label: string, count: number }} entry
   * @param {{ index: number, start: number, size: number }} vItem
   */
  function buildHeaderNode(entry, vItem) {
    const headerEl = doc.createElement("h3");
    headerEl.setAttribute("data-member-group-header", entry.groupId);
    headerEl.setAttribute("data-group-id", entry.groupId);
    headerEl.setAttribute("data-group-count", String(entry.count));
    headerEl.setAttribute("data-index", String(vItem.index));
    headerEl.setAttribute("role", "heading");
    headerEl.textContent = `${entry.label} — ${entry.count}`;
    if (headerEl.style && typeof headerEl.style.setProperty === "function") {
      headerEl.style.setProperty("position", "absolute");
      headerEl.style.setProperty("top", "0px");
      headerEl.style.setProperty("left", "0px");
      headerEl.style.setProperty("width", "100%");
      headerEl.style.setProperty("height", `${vItem.size}px`);
      headerEl.style.setProperty("transform", `translateY(${vItem.start}px)`);
    }
    return headerEl;
  }

  /**
   * @param {{ type: "member", groupId: string, member: GuildMemberEntry }} entry
   * @param {{ index: number, start: number, size: number }} vItem
   */
  function buildMemberNode(entry, vItem) {
    const member = entry.member;
    const userId = String(member.userId ?? "");
    const displayName = getMemberSortName(member);
    const status = normalizePresenceStatus(member.status);
    const statusLabel = PRESENCE_STATUS_LABELS[status] ?? status;
    const roleHexColor = resolveMemberRoleHexColor(member, currentRoles);

    const itemEl = doc.createElement("div");
    itemEl.setAttribute("data-member-item", userId);
    itemEl.setAttribute("data-user-id", userId);
    itemEl.setAttribute("data-group-id", entry.groupId);
    itemEl.setAttribute("data-index", String(vItem.index));
    itemEl.setAttribute("data-status", status);
    itemEl.setAttribute("data-context-menu-trigger", "member");
    itemEl.setAttribute("data-hover-card-trigger", "member");
    itemEl.setAttribute("role", "listitem");
    itemEl.setAttribute("tabindex", "0");
    itemEl.setAttribute("aria-label", `${displayName}, ${statusLabel}`);

    if (itemEl.style && typeof itemEl.style.setProperty === "function") {
      itemEl.style.setProperty("position", "absolute");
      itemEl.style.setProperty("top", "0px");
      itemEl.style.setProperty("left", "0px");
      itemEl.style.setProperty("width", "100%");
      itemEl.style.setProperty("height", `${vItem.size}px`);
      itemEl.style.setProperty("transform", `translateY(${vItem.start}px)`);
    }

    if (roleHexColor) {
      applyElementRoleColor(itemEl, roleHexColor);
    }

    const statusDotEl = doc.createElement("span");
    statusDotEl.setAttribute("data-status", status);
    statusDotEl.setAttribute("data-presence-badge", status);
    statusDotEl.setAttribute("title", statusLabel);
    statusDotEl.setAttribute("aria-hidden", "true");
    itemEl.appendChild(statusDotEl);

    const nameEl = doc.createElement("span");
    nameEl.setAttribute("data-member-name", "");
    nameEl.textContent = displayName;
    if (roleHexColor) {
      applyElementRoleColor(nameEl, roleHexColor);
    }
    itemEl.appendChild(nameEl);

    if (member.isOwner === true) {
      const ownerBadgeEl = doc.createElement("span");
      ownerBadgeEl.setAttribute("data-owner-badge", "");
      ownerBadgeEl.setAttribute("aria-label", "Владелец сервера");
      ownerBadgeEl.setAttribute("title", "Владелец сервера");
      ownerBadgeEl.textContent = "👑";
      itemEl.appendChild(ownerBadgeEl);
    }

    if (typeof member.customStatus === "string" && member.customStatus.trim().length > 0) {
      const customStatusEl = doc.createElement("span");
      customStatusEl.setAttribute("data-custom-status", "");
      customStatusEl.setAttribute("data-member-custom-status", "");
      customStatusEl.textContent = member.customStatus;
      itemEl.appendChild(customStatusEl);
    }

    itemEl.addEventListener("click", () => {
      if (typeof options.onSelectMember === "function") {
        options.onSelectMember({
          userId,
          anchorElement: itemEl,
        });
      }
    });

    /**
     * @param {any} event
     */
    const handleContextMenuTrigger = (event) => {
      if (typeof event?.preventDefault === "function") {
        event.preventDefault();
      }
      const { virtualReference, position } = computeAnchorPosition(itemEl, event, {
        placement: "bottom-start",
        offset: 4,
        padding: 8,
        width: 220,
        height: 240,
      });
      if (options.contextMenuEl) {
        void positionFloatingElement(virtualReference, options.contextMenuEl, {
          placement: "bottom-start",
          offset: 4,
          padding: 8,
        });
      }
      if (typeof options.onMemberContextMenu === "function") {
        options.onMemberContextMenu({
          userId,
          member,
          anchorElement: itemEl,
          virtualReference,
          position,
          event,
        });
      }
    };

    itemEl.addEventListener("contextmenu", handleContextMenuTrigger);
    itemEl.addEventListener("keydown", (/** @type {any} */ event) => {
      const key = event && "key" in event ? String(event.key) : "";
      if (key === "ContextMenu" || (key === "F10" && event.shiftKey)) {
        handleContextMenuTrigger(event);
      }
    });

    /**
     * @param {any} event
     */
    const handleHoverStart = (event) => {
      const { position } = computeAnchorPosition(itemEl, null, {
        placement: "left",
        offset: 8,
        padding: 12,
        width: 300,
        height: 220,
      });
      if (options.hoverCardEl) {
        void positionFloatingElement(itemEl, options.hoverCardEl, {
          placement: "left",
          offset: 8,
          padding: 12,
        });
      }
      if (typeof options.onMemberHover === "function") {
        options.onMemberHover({
          userId,
          member,
          anchorElement: itemEl,
          position,
          event,
        });
      }
    };

    /**
     * @param {any} event
     */
    const handleHoverEnd = (event) => {
      if (typeof options.onMemberHoverEnd === "function") {
        options.onMemberHoverEnd({
          userId,
          member,
          anchorElement: itemEl,
          event,
        });
      }
    };

    itemEl.addEventListener("mouseenter", handleHoverStart);
    itemEl.addEventListener("mouseleave", handleHoverEnd);

    return itemEl;
  }

  function paintVisibleWindow() {
    const totalSize = virtualizer.getTotalSize();
    if (sizer.style && typeof sizer.style.setProperty === "function") {
      sizer.style.setProperty("height", `${totalSize}px`);
    } else {
      sizer.setAttribute("style", `position: relative; width: 100%; height: ${totalSize}px`);
    }

    const vItems = virtualizer.getVirtualItems();
    /** @type {any[]} */
    const nodes = [];
    for (const vItem of vItems) {
      const entry = flatItems[vItem.index];
      if (!entry) continue;
      if (entry.type === "header") {
        nodes.push(buildHeaderNode(entry, vItem));
      } else {
        nodes.push(buildMemberNode(entry, vItem));
      }
    }
    replaceElementChildren(sizer, nodes);
  }

  function renderDom() {
    currentGroups = groupMembersByHoistedRole(currentMembers, currentRoles);
    flatItems = flattenMemberGroups(currentGroups);
    virtualizer.setCount(flatItems.length);
    virtualizer.setScrollWindow(Number(root.scrollTop || 0), currentViewportHeight);
    paintVisibleWindow();
  }

  if (typeof root.addEventListener === "function") {
    root.addEventListener("scroll", () => {
      const nextViewport = Number(root.clientHeight || root.boxHeight || currentViewportHeight);
      if (nextViewport > 0) {
        currentViewportHeight = nextViewport;
      }
      virtualizer.setScrollWindow(Number(root.scrollTop || 0), currentViewportHeight);
      paintVisibleWindow();
    });
  }

  /**
   * Прокручивает виртуализованный список участников к заданному `scrollTop`.
   *
   * @param {number} scrollTop
   * @param {number} [nextViewportHeight]
   */
  function scrollTo(scrollTop, nextViewportHeight) {
    const clampedTop = Math.max(0, Number(scrollTop) || 0);
    if (typeof nextViewportHeight === "number" && nextViewportHeight > 0) {
      currentViewportHeight = nextViewportHeight;
    }
    root.scrollTop = clampedTop;
    virtualizer.setScrollWindow(clampedTop, currentViewportHeight);
    paintVisibleWindow();
  }

  /**
   * Прокручивает виртуализованный список к индексу плоского элемента.
   *
   * @param {number} index
   * @param {{ align?: "start" | "center" | "end" | "auto" }} [opts]
   * @returns {number}
   */
  function scrollToIndex(index, opts = {}) {
    const nextOffset = virtualizer.scrollToIndex(index, opts);
    root.scrollTop = nextOffset;
    paintVisibleWindow();
    return nextOffset;
  }

  /**
   * Обновляет список участников и ролей и перерисовывает сайдбар.
   *
   * @param {ReadonlyArray<GuildMemberEntry> | { members?: ReadonlyArray<GuildMemberEntry>, roles?: ReadonlyArray<GuildRoleEntry> }} [membersOrPayload]
   * @param {ReadonlyArray<GuildRoleEntry>} [maybeRoles]
   */
  function render(membersOrPayload, maybeRoles) {
    if (
      membersOrPayload &&
      typeof membersOrPayload === "object" &&
      !Array.isArray(membersOrPayload)
    ) {
      const payload = /** @type {{ members?: ReadonlyArray<GuildMemberEntry>, roles?: ReadonlyArray<GuildRoleEntry> }} */ (
        membersOrPayload
      );
      if (Array.isArray(payload.members)) {
        currentMembers = [...payload.members];
      }
      if (Array.isArray(payload.roles)) {
        currentRoles = [...payload.roles];
      }
    } else {
      if (Array.isArray(membersOrPayload)) {
        currentMembers = [...membersOrPayload];
      }
      if (Array.isArray(maybeRoles)) {
        currentRoles = [...maybeRoles];
      }
    }
    renderDom();
  }

  /**
   * Переключает видимость сайдбара участников, выставляя/снимая атрибут `hidden`,
   * и возвращает новое булево состояние видимости (`true` — видим, `false` — скрыт).
   *
   * @returns {boolean}
   */
  function toggleVisibility() {
    visible = !visible;
    syncVisibilityDom();
    return visible;
  }

  syncVisibilityDom();
  renderDom();
  replaceElementChildren(container, [root]);

  return {
    root,
    element: root,
    sizer,
    virtualizer,
    render,
    setMembers: render,
    toggleVisibility,
    scrollTo,
    scrollToIndex,
    /** @returns {boolean} */
    isVisible() {
      return visible;
    },
    /** @returns {MemberRoleGroup[]} */
    getGroups() {
      return groupMembersByHoistedRole(currentMembers, currentRoles);
    },
    getFlatItems() {
      return flatItems;
    },
    getVirtualItems() {
      return virtualizer.getVirtualItems();
    },
    getTotalSize() {
      return virtualizer.getTotalSize();
    },
  };
}
