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

/**
 * Русские заголовки стандартных групп участников («В сети» и «Не в сети»).
 * @type {Readonly<{ online: string, offline: string }>}
 */
export const DEFAULT_GROUP_LABELS_RU = Object.freeze({
  online: "В сети",
  offline: "Не в сети",
});

/**
 * Допустимые активные статусы присутствия (не считающиеся `offline`).
 * @type {ReadonlySet<string>}
 */
const ONLINE_PRESENCE_STATUSES = new Set(["online", "idle", "dnd"]);

/**
 * @typedef {"online" | "idle" | "dnd" | "offline"} MemberPresenceStatus
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
 * @typedef {object} MemberListControllerOptions
 * @property {any} [container]
 * @property {ReadonlyArray<GuildMemberEntry>} [members]
 * @property {ReadonlyArray<GuildRoleEntry>} [roles]
 * @property {boolean} [visible]
 * @property {(payload: SelectMemberPayload) => void} [onSelectMember]
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

/**
 * Сравнивает двух участников по алфавиту (`displayName || username`) через `localeCompare('ru')`.
 *
 * @param {GuildMemberEntry} a
 * @param {GuildMemberEntry} b
 * @returns {number}
 */
function compareMembersRu(a, b) {
  return getMemberSortName(a).localeCompare(getMemberSortName(b), "ru");
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
 * Создаёт контроллер правого сайдбара списка участников гильдии.
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

  /** @type {GuildMemberEntry[]} */
  let currentMembers = Array.isArray(options.members) ? [...options.members] : [];
  /** @type {GuildRoleEntry[]} */
  let currentRoles = Array.isArray(options.roles) ? [...options.roles] : [];
  /** @type {boolean} */
  let visible = options.visible !== false;

  const root = doc.createElement("aside");
  root.setAttribute("data-member-list", "");
  root.setAttribute("aria-label", "Участники сервера");

  function syncVisibilityDom() {
    root.hidden = !visible;
    root.setAttribute("aria-hidden", visible ? "false" : "true");
    if (visible) {
      root.removeAttribute("hidden");
    } else {
      root.setAttribute("hidden", "");
    }
  }

  function renderDom() {
    const groups = groupMembersByHoistedRole(currentMembers, currentRoles);
    /** @type {any[]} */
    const groupNodes = [];

    for (const group of groups) {
      const groupSection = doc.createElement("section");
      groupSection.setAttribute("data-member-group", group.id);
      groupSection.setAttribute("role", "group");
      groupSection.setAttribute("aria-label", `${group.label} — ${group.count}`);

      const headerEl = doc.createElement("h3");
      headerEl.setAttribute("data-member-group-header", group.id);
      headerEl.setAttribute("data-group-id", group.id);
      headerEl.setAttribute("data-group-count", String(group.count));
      headerEl.textContent = `${group.label} — ${group.count}`;
      groupSection.appendChild(headerEl);

      for (const member of group.members) {
        const userId = String(member.userId ?? "");
        const displayName = getMemberSortName(member);
        const status = normalizePresenceStatus(member.status);
        const roleHexColor = resolveMemberRoleHexColor(member, currentRoles);

        const itemEl = doc.createElement("div");
        itemEl.setAttribute("data-member-item", userId);
        itemEl.setAttribute("data-user-id", userId);
        itemEl.setAttribute("role", "listitem");
        itemEl.setAttribute("tabindex", "0");

        if (roleHexColor) {
          applyElementRoleColor(itemEl, roleHexColor);
        }

        const statusDotEl = doc.createElement("span");
        statusDotEl.setAttribute("data-status", status);
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

        groupSection.appendChild(itemEl);
      }

      groupNodes.push(groupSection);
    }

    replaceElementChildren(root, groupNodes);
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
    render,
    setMembers: render,
    toggleVisibility,
    /** @returns {boolean} */
    isVisible() {
      return visible;
    },
    /** @returns {MemberRoleGroup[]} */
    getGroups() {
      return groupMembersByHoistedRole(currentMembers, currentRoles);
    },
  };
}
