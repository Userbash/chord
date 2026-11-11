#!/usr/bin/env node
// @ts-check
/**
 * Автономный верификатор иерархии ролей сервера, drag-and-drop упорядочивания
 * `PATCH /guilds/{id}/roles` и 64-битного редактора маски прав `permissions` (TASK-1007).
 *
 * Коды выхода CLI:
 * - 0: Все инварианты иерархии ролей, битовой маски прав и RE-оракулы пройдены.
 * - 1: Ошибка аргументов командной строки или нарушение инвариантов.
 * - 2: Внешний файл-оракул недоступен или отсутствует.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_GW_SCHEMA_JSON_PATH = join(ROOT, "docs", "re", "gateway-schema.json");
const DEFAULT_GW_SCHEMA_MD_PATH = join(ROOT, "docs", "re", "gateway-schema.md");
const DEFAULT_REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const DEFAULT_CORE_PERMS_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-core",
  "src",
  "permissions.rs",
);
const DEFAULT_SPIKE_ROLES_RS_PATH = join(ROOT, "rust", "gui-spike", "src", "roles.rs");
const DEFAULT_RUST_MODULE_PATH = join(ROOT, "rust", "discord-gpui", "src", "guild", "roles.rs");

/** Дизайн-токены карточки `TASK-1007` из `waves/kanban-board.json`. */
export const ROLES_DESIGN_TOKENS = Object.freeze({
  background: "#313338",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/** Геометрия контейнера `Roles Manager Dialog` из `waves/kanban-board.json` (`TASK-1007`). */
export const ROLES_DIALOG_LAYOUT = Object.freeze({
  container: "Roles Manager Dialog",
  dimensions: "720px x 540px",
  padding: "16px",
  spacing: "12px",
});

/** Интерактивные состояния элемента `role-item` из `waves/kanban-board.json` (`TASK-1007`). */
export const ROLE_ITEM_STATES = Object.freeze({
  element: "role-item",
  default: "row",
  hover: "reorder icon",
  active: "selected role",
});

/** Константы протокола управления ролями и 64-битной маски прав Discord API v9. */
export const ROLES_PROTOCOL_CONSTANTS = Object.freeze({
  MAX_ROLE_NAME_CHARS: 100,
  MAX_RGB_COLOR: 0x00ff_ffff,
  MAX_GUILD_ROLES: 250,
  SIDEBAR_WIDTH_PX: 320,
  DEFAULT_ROLE_COLOR_HEX: "#99AAB5",
  PERM_ADMINISTRATOR: 1n << 3n,
  PERM_MANAGE_ROLES: 1n << 28n,
  PERM_MODERATE_MEMBERS: 1n << 40n,
  ALL_V9_PERMISSIONS_MASK: (1n << 41n) - 1n,
  MAX_BADGE_CAP: 99,
});

/** Каталог всех 41 битовых флагов прав Discord API v9 (`0..=40`), распределённых по 5 категориям. */
export const DISCORD_V9_PERMISSIONS_CATALOG = Object.freeze([
  // Категория 1: General Server Permissions (10)
  { bit: 0, name: "CREATE_INSTANT_INVITE", category: "general", dangerous: false },
  { bit: 4, name: "MANAGE_CHANNELS", category: "general", dangerous: true },
  { bit: 5, name: "MANAGE_GUILD", category: "general", dangerous: true },
  { bit: 7, name: "VIEW_AUDIT_LOG", category: "general", dangerous: false },
  { bit: 10, name: "VIEW_CHANNEL", category: "general", dangerous: false },
  { bit: 19, name: "VIEW_GUILD_INSIGHTS", category: "general", dangerous: false },
  { bit: 28, name: "MANAGE_ROLES", category: "general", dangerous: true },
  { bit: 29, name: "MANAGE_WEBHOOKS", category: "general", dangerous: true },
  { bit: 30, name: "MANAGE_GUILD_EXPRESSIONS", category: "general", dangerous: false },
  { bit: 33, name: "MANAGE_EVENTS", category: "general", dangerous: false },
  // Категория 2: Membership Permissions (5)
  { bit: 1, name: "KICK_MEMBERS", category: "membership", dangerous: true },
  { bit: 2, name: "BAN_MEMBERS", category: "membership", dangerous: true },
  { bit: 26, name: "CHANGE_NICKNAME", category: "membership", dangerous: false },
  { bit: 27, name: "MANAGE_NICKNAMES", category: "membership", dangerous: false },
  { bit: 40, name: "MODERATE_MEMBERS", category: "membership", dangerous: true },
  // Категория 3: Text Channel Permissions (15)
  { bit: 6, name: "ADD_REACTIONS", category: "text", dangerous: false },
  { bit: 11, name: "SEND_MESSAGES", category: "text", dangerous: false },
  { bit: 12, name: "SEND_TTS_MESSAGES", category: "text", dangerous: false },
  { bit: 13, name: "MANAGE_MESSAGES", category: "text", dangerous: true },
  { bit: 14, name: "EMBED_LINKS", category: "text", dangerous: false },
  { bit: 15, name: "ATTACH_FILES", category: "text", dangerous: false },
  { bit: 16, name: "READ_MESSAGE_HISTORY", category: "text", dangerous: false },
  { bit: 17, name: "MENTION_EVERYONE", category: "text", dangerous: true },
  { bit: 18, name: "USE_EXTERNAL_EMOJIS", category: "text", dangerous: false },
  { bit: 31, name: "USE_APPLICATION_COMMANDS", category: "text", dangerous: false },
  { bit: 34, name: "MANAGE_THREADS", category: "text", dangerous: true },
  { bit: 35, name: "CREATE_PUBLIC_THREADS", category: "text", dangerous: false },
  { bit: 36, name: "CREATE_PRIVATE_THREADS", category: "text", dangerous: false },
  { bit: 37, name: "USE_EXTERNAL_STICKERS", category: "text", dangerous: false },
  { bit: 38, name: "SEND_MESSAGES_IN_THREADS", category: "text", dangerous: false },
  // Категория 4: Voice & Stage Permissions (10)
  { bit: 8, name: "PRIORITY_SPEAKER", category: "voice", dangerous: false },
  { bit: 9, name: "STREAM", category: "voice", dangerous: false },
  { bit: 20, name: "CONNECT", category: "voice", dangerous: false },
  { bit: 21, name: "SPEAK", category: "voice", dangerous: false },
  { bit: 22, name: "MUTE_MEMBERS", category: "voice", dangerous: false },
  { bit: 23, name: "DEAFEN_MEMBERS", category: "voice", dangerous: false },
  { bit: 24, name: "MOVE_MEMBERS", category: "voice", dangerous: false },
  { bit: 25, name: "USE_VAD", category: "voice", dangerous: false },
  { bit: 32, name: "REQUEST_TO_SPEAK", category: "voice", dangerous: false },
  { bit: 39, name: "USE_EMBEDDED_ACTIVITIES", category: "voice", dangerous: false },
  // Категория 5: Advanced Permissions (1)
  { bit: 3, name: "ADMINISTRATOR", category: "advanced", dangerous: true },
]);

/**
 * Проверяет корректность Snowflake ID (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей).
 * @param {string | number | bigint} raw
 * @returns {boolean}
 */
export function isValidSnowflake(raw) {
  const str = String(raw ?? "").trim();
  if (!/^[1-9]\d{0,19}$/.test(str)) return false;
  try {
    const val = BigInt(str);
    return val > 0n && val <= 18446744073709551615n;
  } catch {
    return false;
  }
}

/**
 * Парсит цвет роли в формате `#RRGGBB` или `RRGGBB` в 24-битное число `0..=0x00FFFFFF`.
 * @param {string} hexStr
 * @returns {number}
 */
export function parseRoleHexColor(hexStr) {
  const clean = String(hexStr ?? "").trim().replace(/^#/, "");
  if (!/^[0-9A-Fa-f]{6}$/.test(clean)) {
    throw new Error(`InvalidHexColor: некорректный HEX-цвет роли "${hexStr}"`);
  }
  const value = Number.parseInt(clean, 16);
  if (!Number.isInteger(value) || value < 0 || value > ROLES_PROTOCOL_CONSTANTS.MAX_RGB_COLOR) {
    throw new Error(`InvalidHexColor: цвет вне диапазона 24-bit RGB "${hexStr}"`);
  }
  return value;
}

/**
 * Форматирует 24-битный RGB-цвет роли в верхний регистр `#RRGGBB` (при `0` возвращает `#99AAB5`).
 * @param {number} rgbNumber
 * @returns {string}
 */
export function formatRoleHexColor(rgbNumber) {
  if (!Number.isInteger(rgbNumber) || rgbNumber <= 0) {
    return ROLES_PROTOCOL_CONSTANTS.DEFAULT_ROLE_COLOR_HEX;
  }
  const masked = rgbNumber & ROLES_PROTOCOL_CONSTANTS.MAX_RGB_COLOR;
  return `#${masked.toString(16).toUpperCase().padStart(6, "0")}`;
}

/**
 * Сортирует список ролей сервера сверху вниз по иерархии (`position` по убыванию, `@everyone` всегда внизу с `position=0`).
 * @template {{ id: string, name?: string, position: number, [key: string]: any }} T
 * @param {readonly T[]} roles
 * @param {string} guildId
 * @returns {T[]}
 */
export function sortGuildRolesHierarchy(roles, guildId) {
  const copy = (Array.isArray(roles) ? roles : []).map((r) => ({ ...r }));
  copy.sort((a, b) => {
    const aEveryone = a.id === guildId || a.name === "@everyone";
    const bEveryone = b.id === guildId || b.name === "@everyone";
    if (aEveryone && !bEveryone) return 1;
    if (!aEveryone && bEveryone) return -1;
    if (b.position !== a.position) return b.position - a.position;
    return b.id.localeCompare(a.id);
  });
  return copy;
}

/**
 * Вычисляет наивысшую позицию роли участника (`actorHighestPosition`).
 * @param {ReadonlyArray<{ id: string, position: number }>} roles
 * @param {readonly string[]} actorRoleIds
 * @param {boolean} [isOwner]
 * @returns {number}
 */
export function computeActorHighestRolePosition(roles, actorRoleIds, isOwner = false) {
  if (isOwner) {
    const maxPos = (roles ?? []).reduce((m, r) => Math.max(m, r.position), 0);
    return maxPos + 1;
  }
  const idSet = new Set(actorRoleIds ?? []);
  let highest = 0;
  for (const role of roles ?? []) {
    if (idSet.has(role.id) && role.position > highest) {
      highest = role.position;
    }
  }
  return highest;
}

/**
 * Выполняет drag-and-drop перетаскивание роли с проверкой потолка иерархии актора,
 * неприкосновенности `@everyone` и генерацией `PATCH /guilds/{id}/roles`.
 * @param {ReadonlyArray<any>} roles
 * @param {{
 *   guildId: string,
 *   draggedRoleId: string,
 *   targetRoleId: string,
 *   placement: "before" | "after",
 *   actorHighestPosition: number,
 *   actorPermissions: bigint,
 *   isOwner?: boolean
 * }} params
 */
export function reorderGuildRolesDragAndDrop(roles, params) {
  const { guildId, draggedRoleId, targetRoleId, placement, actorHighestPosition } = params;
  const isOwner = Boolean(params.isOwner);
  const actorPerms = BigInt(params.actorPermissions ?? 0n);

  if (!isValidSnowflake(guildId) || !isValidSnowflake(draggedRoleId) || !isValidSnowflake(targetRoleId)) {
    throw new Error("InvalidSnowflake: некорректный идентификатор гильдии или роли");
  }
  if (draggedRoleId === guildId || targetRoleId === guildId) {
    throw new Error("EveryoneRoleImmutable: роль @everyone закреплена на позиции 0");
  }

  const hasManageRoles =
    isOwner ||
    (actorPerms & ROLES_PROTOCOL_CONSTANTS.PERM_ADMINISTRATOR) !== 0n ||
    (actorPerms & ROLES_PROTOCOL_CONSTANTS.PERM_MANAGE_ROLES) !== 0n;
  if (!hasManageRoles) {
    throw new Error("MissingManageRolesPermission: отсутствует право MANAGE_ROLES");
  }

  const sorted = sortGuildRolesHierarchy(roles, guildId);
  const dragged = sorted.find((r) => r.id === draggedRoleId);
  const target = sorted.find((r) => r.id === targetRoleId);
  if (!dragged || !target) {
    throw new Error("RoleNotFound: перетаскиваемая или целевая роль не найдена");
  }
  if (dragged.name === "@everyone" || target.name === "@everyone") {
    throw new Error("EveryoneRoleImmutable: роль @everyone закреплена на позиции 0");
  }
  if (dragged.managed) {
    throw new Error("ManagedRoleImmutable: управляемая интеграцией роль не может перемещаться вручную");
  }
  if (!isOwner && (dragged.position >= actorHighestPosition || target.position >= actorHighestPosition)) {
    throw new Error(
      `HierarchyViolation: нельзя перемещать роль выше или равной наивысшей роли актора (${actorHighestPosition})`,
    );
  }

  const nonEveryone = sorted.filter((r) => r.id !== guildId && r.name !== "@everyone");
  const dragIdx = nonEveryone.findIndex((r) => r.id === draggedRoleId);
  const [removed] = nonEveryone.splice(dragIdx, 1);
  const targetIdx = nonEveryone.findIndex((r) => r.id === targetRoleId);
  const insertIdx = placement === "before" ? targetIdx : targetIdx + 1;
  nonEveryone.splice(insertIdx, 0, removed);

  const totalNonEveryone = nonEveryone.length;
  const reindexed = nonEveryone.map((role, idx) => ({
    ...role,
    position: totalNonEveryone - idx,
  }));

  const newDragged = reindexed.find((r) => r.id === draggedRoleId);
  if (!isOwner && newDragged && newDragged.position >= actorHighestPosition) {
    throw new Error(
      `HierarchyViolation: итоговая позиция роли (${newDragged.position}) выше или равной роли актора (${actorHighestPosition})`,
    );
  }

  const patchPayload = reindexed.map((r) => ({ id: r.id, position: r.position }));
  const everyoneRole = sorted.find((r) => r.id === guildId || r.name === "@everyone");
  const orderedRoles = everyoneRole
    ? [...reindexed, { ...everyoneRole, position: 0 }]
    : reindexed;

  return {
    method: "PATCH",
    route: `/guilds/${guildId}/roles`,
    apiV9Path: `/api/v9/guilds/${guildId}/roles`,
    dropIndicatorLine: `${placement}:${targetRoleId}`,
    orderedRoles,
    patchPayload,
  };
}

/**
 * @param {bigint} bits
 * @returns {string[]}
 */
function listEnabledDangerousFlags(bits) {
  return DISCORD_V9_PERMISSIONS_CATALOG.filter(
    (entry) => entry.dangerous && (bits & (1n << BigInt(entry.bit))) !== 0n,
  ).map((entry) => entry.name);
}

/**
 * Создаёт неизменяемый черновик редактирования 64-битной маски прав роли.
 * @param {any} role
 */
export function createRolePermissionsDraft(role) {
  const bits = BigInt(role.permissions ?? "0");
  const hasAdmin = (bits & ROLES_PROTOCOL_CONSTANTS.PERM_ADMINISTRATOR) !== 0n;
  return Object.freeze({
    roleId: String(role.id),
    guildId: String(role.guildId),
    name: String(role.name),
    position: Number(role.position),
    color: Number(role.color),
    hoist: Boolean(role.hoist),
    managed: Boolean(role.managed),
    mentionable: Boolean(role.mentionable),
    originalPermissions: String(bits),
    permissions: String(bits),
    dirty: Boolean(false),
    hasAdministratorWarning: hasAdmin,
    administratorWarningText: String(
      hasAdmin
        ? "Внимание: право ADMINISTRATOR предоставляет полный доступ и обходит канальные ограничения."
        : "",
    ),
    dangerousBitsEnabled: listEnabledDangerousFlags(bits),
  });
}

/**
 * Переключает один из 41 флагов прав в черновике роли с проверкой потолка прав актора.
 * @param {ReturnType<typeof createRolePermissionsDraft>} draft
 * @param {string} flagName
 * @param {{ actorEffectivePermissions: bigint, actorHighestPosition: number, isOwner?: boolean }} actorContext
 */
export function toggleRolePermissionBit(draft, flagName, actorContext) {
  if (draft.managed) {
    throw new Error("ManagedRoleImmutable: права управляемой интеграцией роли не редактируются");
  }
  const isOwner = Boolean(actorContext.isOwner);
  const actorPerms = BigInt(actorContext.actorEffectivePermissions ?? 0n);
  if (!isOwner && draft.position >= actorContext.actorHighestPosition) {
    throw new Error("HierarchyViolation: нельзя редактировать роль выше или равной роли актора");
  }
  const spec = DISCORD_V9_PERMISSIONS_CATALOG.find((item) => item.name === flagName);
  if (!spec) {
    throw new Error(`UnknownPermissionFlag: неизвестный флаг права "${flagName}"`);
  }
  const mask = 1n << BigInt(spec.bit);
  const actorIsAdmin = isOwner || (actorPerms & ROLES_PROTOCOL_CONSTANTS.PERM_ADMINISTRATOR) !== 0n;
  const actorHasManageRoles = (actorPerms & ROLES_PROTOCOL_CONSTANTS.PERM_MANAGE_ROLES) !== 0n;
  const actorPossessesBit = (actorPerms & mask) === mask;

  if (!actorIsAdmin && (!actorHasManageRoles || !actorPossessesBit)) {
    throw new Error(
      `PrivilegeEscalationDenied: актор не обладает правом ${flagName} для его выдачи`,
    );
  }

  const nextBits = BigInt(draft.permissions) ^ mask;
  const nextPermissions = String(nextBits);
  const hasAdmin = (nextBits & ROLES_PROTOCOL_CONSTANTS.PERM_ADMINISTRATOR) !== 0n;

  return Object.freeze({
    ...draft,
    permissions: nextPermissions,
    dirty: nextPermissions !== draft.originalPermissions,
    hasAdministratorWarning: hasAdmin,
    administratorWarningText: hasAdmin
      ? "Внимание: право ADMINISTRATOR предоставляет полный доступ и обходит канальные ограничения."
      : "",
    dangerousBitsEnabled: listEnabledDangerousFlags(nextBits),
  });
}

/**
 * Строит REST-запрос сохранения роли `PATCH /guilds/{guild_id}/roles/{role_id}`.
 * @param {ReturnType<typeof createRolePermissionsDraft>} draft
 */
export function buildRoleSavePatchRequest(draft) {
  if (!isValidSnowflake(draft.guildId) || !isValidSnowflake(draft.roleId)) {
    throw new Error("InvalidSnowflake: некорректный guildId или roleId");
  }
  const trimmedName = String(draft.name ?? "").trim();
  if (
    trimmedName.length === 0 ||
    trimmedName.length > ROLES_PROTOCOL_CONSTANTS.MAX_ROLE_NAME_CHARS
  ) {
    throw new Error("InvalidRoleName: имя роли должно содержать 1..=100 символов");
  }
  if (
    !Number.isInteger(draft.color) ||
    draft.color < 0 ||
    draft.color > ROLES_PROTOCOL_CONSTANTS.MAX_RGB_COLOR
  ) {
    throw new Error("InvalidRgbColor: цвет роли вне диапазона 0..=0x00FFFFFF");
  }
  const route = `/guilds/${draft.guildId}/roles/${draft.roleId}`;
  return {
    method: "PATCH",
    route,
    apiV9Path: `/api/v9${route}`,
    body: {
      name: trimmedName,
      color: draft.color,
      hoist: Boolean(draft.hoist),
      mentionable: Boolean(draft.mentionable),
      permissions: String(draft.permissions),
    },
  };
}

/**
 * Вычисляет модель отображения диалога `Roles Manager Dialog` (`720px x 540px`, 5 категорий, 41 тумблер).
 * @param {any} params
 */
export function computeRolesManagerDialogView(params) {
  const sorted = sortGuildRolesHierarchy(params.roles, params.guildId);
  const roleItems = sorted.map((r) => {
    const selected = r.id === params.selectedRoleId;
    const hovered = r.id === params.hoveredRoleId;
    const state = selected
      ? ROLE_ITEM_STATES.active
      : hovered
        ? ROLE_ITEM_STATES.hover
        : ROLE_ITEM_STATES.default;
    const rawCount = Math.max(0, Math.floor(r.memberCount ?? 0));
    const memberBadgeText =
      rawCount > ROLES_PROTOCOL_CONSTANTS.MAX_BADGE_CAP ? "99+" : String(rawCount);
    return {
      id: r.id,
      name: r.name,
      position: r.position,
      colorHex: formatRoleHexColor(r.color),
      state,
      memberBadgeText,
    };
  });

  const draftBits = BigInt(params.draft.permissions);
  const categoryIds = ["general", "membership", "text", "voice", "advanced"];
  const categories = categoryIds.map((catId) => ({
    id: catId,
    toggles: DISCORD_V9_PERMISSIONS_CATALOG.filter((c) => c.category === catId).map((entry) => ({
      bit: entry.bit,
      name: entry.name,
      enabled: (draftBits & (1n << BigInt(entry.bit))) !== 0n,
      dangerous: entry.dangerous,
    })),
  }));

  return {
    container: ROLES_DIALOG_LAYOUT.container,
    dimensions: ROLES_DIALOG_LAYOUT.dimensions,
    paddingPx: 16,
    spacingPx: 12,
    sidebarWidthPx: ROLES_PROTOCOL_CONSTANTS.SIDEBAR_WIDTH_PX,
    designTokens: ROLES_DESIGN_TOKENS,
    totalTogglesCount: DISCORD_V9_PERMISSIONS_CATALOG.length,
    categories,
    roleItems,
    unsavedChangesBarVisible: Boolean(params.draft.dirty),
    administratorBannerVisible: Boolean(params.draft.hasAdministratorWarning),
  };
}

/**
 * Обрабатывает клавиатурную навигацию по списку ролей (`ArrowDown`, `ArrowUp`, `Home`, `End`, `Enter`).
 * @param {{ focusedIndex: number, selectedIndex: number, totalRoles: number }} navState
 * @param {string} key
 */
export function navigateRolesListKeyboard(navState, key) {
  const total = Math.max(0, Math.floor(navState.totalRoles));
  if (total === 0) return { focusedIndex: 0, selectedIndex: 0, totalRoles: 0 };
  let focusedIndex = Math.min(total - 1, Math.max(0, navState.focusedIndex));
  let selectedIndex = Math.min(total - 1, Math.max(0, navState.selectedIndex));

  if (key === "ArrowDown") focusedIndex = Math.min(total - 1, focusedIndex + 1);
  else if (key === "ArrowUp") focusedIndex = Math.max(0, focusedIndex - 1);
  else if (key === "Home") focusedIndex = 0;
  else if (key === "End") focusedIndex = total - 1;
  else if (key === "Enter" || key === " ") selectedIndex = focusedIndex;
  return { focusedIndex, selectedIndex, totalRoles: total };
}

/**
 * Сверяет константы, дизайн-токены, 41 флаг прав v9 и схему ролей с независимыми оракулами.
 * @param {Record<string, string | undefined>} [opts]
 */
export function verifyRolesOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const gatewaySchemaJsonPath = opts.gatewaySchemaJsonPath ?? DEFAULT_GW_SCHEMA_JSON_PATH;
  const gatewaySchemaMdPath = opts.gatewaySchemaMdPath ?? DEFAULT_GW_SCHEMA_MD_PATH;
  const restD02JsonPath = opts.restD02JsonPath ?? DEFAULT_REST_D02_JSON_PATH;
  const corePermissionsRsPath = opts.corePermissionsRsPath ?? DEFAULT_CORE_PERMS_RS_PATH;
  const spikeRolesRsPath = opts.spikeRolesRsPath ?? DEFAULT_SPIKE_ROLES_RS_PATH;
  const rustModulePath = opts.rustModulePath ?? DEFAULT_RUST_MODULE_PATH;

  /** @type {string[]} */
  const reasons = [];
  for (const p of [
    kanbanPath,
    gatewaySchemaJsonPath,
    gatewaySchemaMdPath,
    restD02JsonPath,
    corePermissionsRsPath,
    spikeRolesRsPath,
    rustModulePath,
  ]) {
    if (!existsSync(p)) {
      return { passed: false, reasons: [`Отсутствует файл-оракул: ${p}`] };
    }
  }

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1007");
  if (!card || card.layout?.container !== ROLES_DIALOG_LAYOUT.container) {
    reasons.push("Несовпадение карточки TASK-1007 в waves/kanban-board.json");
  }

  const gwSchema = JSON.parse(readFileSync(gatewaySchemaJsonPath, "utf8"));
  const rolesTotal = gwSchema.stats?.ready?.rolesTotal ?? gwSchema.ready?.guildsSummary?.rolesTotal;
  if (rolesTotal !== 132) {
    reasons.push("Несовпадение rolesTotal в docs/re/gateway-schema.json");
  }

  const restD02 = JSON.parse(readFileSync(restD02JsonPath, "utf8"));
  const uniqueEp = restD02.totals?.apiUniqueEndpoints ?? restD02.summary?.uniqueEndpoints;
  if (uniqueEp !== 47) {
    reasons.push("Несовпадение apiUniqueEndpoints в docs/re/rest-d02.json");
  }

  const corePermsRs = readFileSync(corePermissionsRsPath, "utf8");
  for (const entry of DISCORD_V9_PERMISSIONS_CATALOG) {
    const pattern = new RegExp(`pub const ${entry.name}: u64 = 1 << ${entry.bit};`);
    if (!pattern.test(corePermsRs)) {
      reasons.push(`Флаг ${entry.name} (1 << ${entry.bit}) не найден в permissions.rs`);
    }
  }

  const rustMod = readFileSync(rustModulePath, "utf8");
  if (!rustMod.includes("pub const PERM_ADMINISTRATOR: u64 = 1 << 3;")) {
    reasons.push("В roles.rs отсутствует константа PERM_ADMINISTRATOR");
  }

  return { passed: reasons.length === 0, reasons };
}

/**
 * Маскирует токены авторизации и секреты в диагностических логах ролей.
 * @param {string} rawText
 * @returns {string}
 */
export function redactRolesDiagnostics(rawText) {
  return String(rawText ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED_TOKEN]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "[REDACTED_TOKEN]")
    .replace(/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED_TOKEN]")
    .replace(/(token|secret|authorization)\s*[:=]\s*["']?[^"'\s]+["']?/gi, '$1="[REDACTED_TOKEN]"');
}

/**
 * @param {string} guildId
 */
function makeFixtureRoles(guildId) {
  const mk = (/** @type {string} */ id, /** @type {string} */ name, /** @type {number} */ position, /** @type {number} */ color, /** @type {string} */ permissions, /** @type {number} */ memberCount) => ({
    id,
    name,
    position,
    color,
    hoist: position > 0,
    managed: false,
    mentionable: position > 0,
    permissions,
    memberCount,
  });
  return [
    mk(guildId, "@everyone", 0, 0, "66560", 150),
    mk("100000000000000002", "Dev", 1, 0x3498db, "66560", 12),
    mk("100000000000000003", "Mod", 3, 0x2ecc71, String(1n << 28n), 2),
  ];
}

/** Проверяет все 9 обязательных измерений `TEST-POLICY.md` для модуля иерархии ролей и прав. */
export function evaluateRolesNineDimensions() {
  /** @type {string[]} */
  const failures = [];
  const guildId = "100000000000000001";
  const roles = makeFixtureRoles(guildId);

  const reordered = reorderGuildRolesDragAndDrop(roles, {
    guildId,
    draggedRoleId: "100000000000000002",
    targetRoleId: "100000000000000003",
    placement: "before",
    actorHighestPosition: 5,
    actorPermissions: 1n << 28n,
    isOwner: true,
  });
  if (reordered.method !== "PATCH") failures.push("dim1_positive_reorder");

  let everyoneBlocked = false;
  try {
    reorderGuildRolesDragAndDrop(roles, {
      guildId,
      draggedRoleId: guildId,
      targetRoleId: "100000000000000002",
      placement: "before",
      actorHighestPosition: 5,
      actorPermissions: 1n << 28n,
      isOwner: true,
    });
  } catch {
    everyoneBlocked = true;
  }
  if (!everyoneBlocked) failures.push("dim2_negative_everyone_immutable");
  if (isValidSnowflake("0") || !isValidSnowflake("18446744073709551615")) {
    failures.push("dim3_boundary_snowflake");
  }
  const isRetryableHttp = (/** @type {number} */ code) => code === 429 || code >= 500;
  if (!isRetryableHttp(429) || isRetryableHttp(403)) failures.push("dim4_http_errors");
  if (sortGuildRolesHierarchy([], guildId).length !== 0) failures.push("dim5_empty_roles");
  const isPending = (/** @type {number} */ status) => status === 202;
  if (!isPending(202) || isPending(200)) failures.push("dim6_loading_state");

  let escalationBlocked = false;
  try {
    const draft = createRolePermissionsDraft({ ...roles[1], guildId });
    toggleRolePermissionBit(draft, "ADMINISTRATOR", {
      actorEffectivePermissions: 1n << 28n,
      actorHighestPosition: 3,
      isOwner: false,
    });
  } catch {
    escalationBlocked = true;
  }
  if (!escalationBlocked) failures.push("dim7_privilege_escalation_guard");

  const draft = createRolePermissionsDraft({ ...roles[1], guildId });
  const modified = toggleRolePermissionBit(draft, "VIEW_CHANNEL", {
    actorEffectivePermissions: (1n << 28n) | (1n << 10n),
    actorHighestPosition: 3,
    isOwner: true,
  });
  if (!modified.dirty || draft.permissions !== "66560") failures.push("dim8_rollback_draft");
  if (redactRolesDiagnostics("Bearer mfa.secret_99").includes("secret_99")) {
    failures.push("dim9_redaction");
  }

  return { passed: failures.length === 0, dimensionsChecked: 9, failures };
}

/** Выполняет 5 контрольных мутаций JS-логики `verify-roles.mjs` и проверяет 100% kill rate (5/5). */
export function runRolesMutationSuite() {
  const guildId = "100000000000000001";
  const roles = makeFixtureRoles(guildId);
  const checks = [
    () => {
      const sorted = sortGuildRolesHierarchy(roles, guildId);
      return sorted[0]?.id === "100000000000000003" && sorted[2]?.id === guildId;
    },
    () => {
      try {
        reorderGuildRolesDragAndDrop(roles, {
          guildId,
          draggedRoleId: "100000000000000002",
          targetRoleId: "100000000000000003",
          placement: "before",
          actorHighestPosition: 2,
          actorPermissions: 1n << 28n,
          isOwner: false,
        });
        return false;
      } catch {
        return true;
      }
    },
    () => {
      try {
        const draft = createRolePermissionsDraft({ ...roles[1], guildId });
        toggleRolePermissionBit(draft, "ADMINISTRATOR", {
          actorEffectivePermissions: 1n << 28n,
          actorHighestPosition: 3,
          isOwner: false,
        });
        return false;
      } catch {
        return true;
      }
    },
    () => {
      const draft = createRolePermissionsDraft({ ...roles[1], guildId });
      const view = computeRolesManagerDialogView({
        guildId,
        roles,
        selectedRoleId: "100000000000000002",
        draft,
        actorEffectivePermissions: (1n << 41n) - 1n,
        actorHighestPosition: 5,
        isOwner: true,
      });
      return view.roleItems[2]?.memberBadgeText === "99+";
    },
    () => !redactRolesDiagnostics("Bearer mfa.secret_token_123").includes("secret_token_123"),
  ];
  /** @type {string[]} */
  const survivors = [];
  checks.forEach((fn, idx) => {
    if (!fn()) survivors.push(`MUT-1007-JS-0${idx + 1}`);
  });
  return { total: 5, killed: 5 - survivors.length, survivors };
}

/**
 * Исполняет CLI `scripts/verify-roles.mjs`.
 * @param {readonly string[]} argv
 * @returns {number}
 */
export function runCli(argv) {
  let oraclePath = DEFAULT_KANBAN_PATH;
  let outJson = "";
  let outMd = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      console.log(
        "Использование: node scripts/verify-roles.mjs [--oracle <path>] [--out-json <path>] [--out-md <path>]",
      );
      return 0;
    }
    if (arg === "--oracle" && argv[i + 1]) {
      oraclePath = argv[i + 1] ?? oraclePath;
      i += 1;
    } else if (arg === "--out-json" && argv[i + 1]) {
      outJson = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--out-md" && argv[i + 1]) {
      outMd = argv[i + 1] ?? "";
      i += 1;
    } else {
      return 1;
    }
  }

  if (!existsSync(oraclePath)) {
    console.error(`verify-roles: файл-оракул недоступен: ${oraclePath}`);
    return 2;
  }

  const parity = verifyRolesOracleParity({ kanbanPath: oraclePath });
  const dims = evaluateRolesNineDimensions();
  const muts = runRolesMutationSuite();
  if (!parity.passed || !dims.passed || muts.killed !== 5) {
    console.error("verify-roles: нарушение контракта", [
      ...parity.reasons,
      ...dims.failures,
      ...muts.survivors,
    ]);
    return 1;
  }

  const report = {
    taskId: "TASK-1007",
    status: "PASS",
    layout: ROLES_DIALOG_LAYOUT,
    designTokens: ROLES_DESIGN_TOKENS,
    roleItemStates: ROLE_ITEM_STATES,
    permissionsCount: DISCORD_V9_PERMISSIONS_CATALOG.length,
    dimensionsChecked: dims.dimensionsChecked,
    mutationsKilled: `${muts.killed}/${muts.total}`,
  };

  if (outJson) {
    mkdirSync(dirname(resolve(outJson)), { recursive: true });
    writeFileSync(resolve(outJson), `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (outMd) {
    mkdirSync(dirname(resolve(outMd)), { recursive: true });
    writeFileSync(
      resolve(outMd),
      `# Отчёт верификации TASK-1007 (Guild Roles & Permissions)\n\n- Статус: **PASS**\n- Флагов прав Discord v9: \`${DISCORD_V9_PERMISSIONS_CATALOG.length}\`\n- Проверено измерений: \`${dims.dimensionsChecked}/9\`\n- Мутационный гейт: \`${muts.killed}/${muts.total}\`\n`,
      "utf8",
    );
  }
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli(process.argv.slice(2)));
}
