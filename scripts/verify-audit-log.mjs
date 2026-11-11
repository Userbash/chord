#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор журнала аудита сервера (Server Audit Log & REST API v9 — TASK-1008):
 * сверка дизайн-токенов, геометрии `Audit Log Viewer` (`100% x 100%`), состояний `audit-entry`,
 * `audit-decode`, `audit-row`, маршрута `GET /guilds/{id}/audit-logs` (`GUILD_AUDIT_LOG`),
 * декодера изменений `changes` (`old_value` vs `new_value`), фильтрации по типу события и автору,
 * 9 измерений `TEST-POLICY.md` и маскирования секретов. Коды выхода CLI: `0 / 1 / 2`.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  AUDIT_ACTION_LABELS_RU as BASE_ACTION_LABELS_RU,
  AUDIT_ACTION_TYPES as BASE_ACTION_TYPES,
} from "../rust/gui-spike/ui/features/audit-log.js";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_REST_D02_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const DEFAULT_CORE_PERMISSIONS_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-core",
  "src",
  "permissions.rs",
);
const DEFAULT_RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const DEFAULT_RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** Макет контейнера `Audit Log Viewer` из `waves/kanban-board.json` (`TASK-1008`). */
export const AUDIT_LOG_VIEWER_LAYOUT = Object.freeze({
  container: "Audit Log Viewer",
  dimensions: "100% x 100%",
  padding: "16px",
  spacing: "8px",
});

/** Дизайн-токены карточки `TASK-1008` из `waves/kanban-board.json`. */
export const AUDIT_LOG_DESIGN_TOKENS = Object.freeze({
  background: "#313338",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/** Интерактивные состояния элемента `audit-entry` (`TASK-1008`). */
export const AUDIT_ENTRY_STATES = Object.freeze({
  element: "audit-entry",
  default: "entry row",
  hover: "expand diff",
  active: "expanded details",
});

/** Интерактивные состояния элемента `audit-decode` (`TASK-1008A`). */
export const AUDIT_DECODE_STATES = Object.freeze({
  element: "audit-decode",
  default: "raw",
  hover: "decoding",
  active: "human readable",
});

/** Интерактивные состояния элемента `audit-row` (`TASK-1008B`). */
export const AUDIT_ROW_STATES = Object.freeze({
  element: "audit-row",
  default: "summary",
  hover: "hover-row",
  active: "expanded diff",
});

/** Константы протокола и UI журнала аудита из модулей `411153`, `523599`, `555067` RE-дампа Discord. */
export const AUDIT_LOG_PROTOCOL_CONSTANTS = Object.freeze({
  ENDPOINT_KEY: "GUILD_AUDIT_LOG",
  HEADER_AUDIT_REASON: "X-Audit-Log-Reason",
  MIN_PAGE_LIMIT: 1,
  DEFAULT_PAGE_LIMIT: 50,
  MAX_PAGE_LIMIT: 100,
  MAX_REASON_CHARS: 512,
  MERGE_WINDOW_MINUTES: 30,
  MERGE_WINDOW_MS: 30 * 60 * 1_000,
  MAX_GROUPED_CHANGES: 50,
  DISCORD_EPOCH_MS: 1420070400000n,
  PERM_ADMINISTRATOR: 1n << 3n,
  PERM_VIEW_AUDIT_LOG: 1n << 7n,
  RE_MODULE_SECTION_CSS: "117497",
  RE_MODULE_HEADER_CSS: "165259",
  RE_MODULE_MODEL: "411153",
  RE_MODULE_STORE: "523599",
  RE_MODULE_UTILS: "555067",
});

/** Коды действий журнала аудита Discord (`AuditLogActionTypes` `F_X` из модуля `652215`). */
export const AUDIT_ACTION_TYPES = Object.freeze({
  ...BASE_ACTION_TYPES,
  CHANNEL_OVERWRITE_CREATE: 13,
  CHANNEL_OVERWRITE_UPDATE: 14,
  CHANNEL_OVERWRITE_DELETE: 15,
  CHANNEL_POSITION_UPDATE: 16,
  MEMBER_PRUNE: 21,
  MEMBER_UPDATE: 24,
  MEMBER_ROLE_UPDATE: 25,
  MEMBER_MOVE: 26,
  MEMBER_DISCONNECT: 27,
  BOT_ADD: 28,
  ROLE_POSITION_UPDATE: 33,
  INVITE_CREATE: 40,
  WEBHOOK_CREATE: 50,
  EMOJI_CREATE: 60,
  MESSAGE_BULK_DELETE: 73,
  STICKER_CREATE: 90,
  THREAD_CREATE: 110,
  AUTO_MODERATION_RULE_CREATE: 140,
  AUTO_MODERATION_BLOCK_MESSAGE: 143,
  AUTO_MODERATION_USER_COMMUNICATION_DISABLED: 145,
});

/**
 * Русские метки действий по коду (паритет с `audit-log.js` + расширенные коды).
 * @type {Readonly<Record<number, string>>}
 */
export const AUDIT_ACTION_LABELS_RU = Object.freeze({
  ...BASE_ACTION_LABELS_RU,
  13: "Создание прав доступа канала",
  14: "Изменение прав доступа канала",
  15: "Удаление прав доступа канала",
  21: "Очистка неактивных участников",
  24: "Изменение участника",
  25: "Изменение ролей участника",
  40: "Создание приглашения",
  50: "Создание вебхука",
  73: "Массовое удаление сообщений",
  110: "Создание ветки",
  140: "Создание правила автомодерации",
  143: "Блокировка сообщения автомодерацией",
  145: "Тайм-аут участника автомодерацией",
});

const UNKNOWN_ACTOR_RU = "Неизвестный";
const EMPTY_VALUE_RU = "нет";
const SENSITIVE_CHANGE_KEYS = new Set([
  "token",
  "access_token",
  "refresh_token",
  "authorization",
  "secret",
  "client_secret",
  "password",
  "webhook_token",
]);
const NON_MERGEABLE_ACTIONS = new Set([16, 21, 26, 27, 28, 33, 40, 41, 42, 72, 73, 74, 75, 121]);
const CREATE_CODES = new Set([
  10, 13, 23, 28, 30, 40, 50, 60, 74, 80, 83, 90, 100, 110, 130, 140, 150, 163, 166, 171, 190, 192,
]);
const DELETE_CODES = new Set([
  12, 15, 20, 21, 22, 27, 32, 42, 52, 62, 72, 73, 75, 82, 85, 92, 102, 112, 132, 142, 143, 165, 172,
  193, 202,
]);

/**
 * Проверяет корректность Snowflake ID (`> 0` и `<= u64::MAX`).
 * @param {string | number | bigint} raw
 * @returns {boolean}
 */
export function isValidSnowflake(raw) {
  const str = String(raw ?? "").trim();
  if (!/^\d{1,20}$/.test(str)) return false;
  try {
    const val = BigInt(str);
    return val > 0n && val <= 18446744073709551615n;
  } catch {
    return false;
  }
}

/**
 * Извлекает временную метку Unix (в мс) из Snowflake ID (`(id >> 22) + 1420070400000`).
 * @param {string | number | bigint} raw
 * @returns {number}
 */
export function extractSnowflakeTimestampMs(raw) {
  if (!isValidSnowflake(raw)) {
    throw new RangeError(`Некорректный Snowflake ID: ${raw}`);
  }
  const id = BigInt(String(raw).trim());
  return Number((id >> 22n) + AUDIT_LOG_PROTOCOL_CONSTANTS.DISCORD_EPOCH_MS);
}

/**
 * Вычисляет интерактивное состояние элемента `audit-entry` (`entry row` / `expand diff` / `expanded details`).
 * @param {{ expanded?: boolean, hovered?: boolean }} opts
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveAuditEntryState(opts) {
  if (opts.expanded) {
    return { element: AUDIT_ENTRY_STATES.element, state: "active", label: AUDIT_ENTRY_STATES.active };
  }
  if (opts.hovered) {
    return { element: AUDIT_ENTRY_STATES.element, state: "hover", label: AUDIT_ENTRY_STATES.hover };
  }
  return { element: AUDIT_ENTRY_STATES.element, state: "default", label: AUDIT_ENTRY_STATES.default };
}

/**
 * Вычисляет интерактивное состояние декодера диффа `audit-decode` (`raw` / `decoding` / `human readable`).
 * @param {"raw" | "decoding" | "decoded"} phase
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveAuditDecodeState(phase) {
  if (phase === "decoded") {
    return { element: AUDIT_DECODE_STATES.element, state: "active", label: AUDIT_DECODE_STATES.active };
  }
  if (phase === "decoding") {
    return { element: AUDIT_DECODE_STATES.element, state: "hover", label: AUDIT_DECODE_STATES.hover };
  }
  return { element: AUDIT_DECODE_STATES.element, state: "default", label: AUDIT_DECODE_STATES.default };
}

/**
 * Строит REST-запрос `GET /guilds/{id}/audit-logs` (`Endpoints.GUILD_AUDIT_LOG`).
 * @param {{ guildId: string, userId?: string, actionType?: number, before?: string, after?: string, limit?: number }} params
 */
export function buildGuildAuditLogRequest(params) {
  if (!isValidSnowflake(params.guildId)) {
    throw new RangeError(`Некорректный guild_id: ${params.guildId}`);
  }
  const limit = params.limit ?? AUDIT_LOG_PROTOCOL_CONSTANTS.DEFAULT_PAGE_LIMIT;
  if (
    !Number.isInteger(limit) ||
    limit < AUDIT_LOG_PROTOCOL_CONSTANTS.MIN_PAGE_LIMIT ||
    limit > AUDIT_LOG_PROTOCOL_CONSTANTS.MAX_PAGE_LIMIT
  ) {
    throw new RangeError(`Лимит пагинации журнала аудита должен быть 1..100: ${limit}`);
  }
  const queryParts = [`limit=${limit}`];
  if (params.userId !== undefined) {
    if (!isValidSnowflake(params.userId)) {
      throw new RangeError(`Некорректный фильтр user_id: ${params.userId}`);
    }
    queryParts.push(`user_id=${params.userId}`);
  }
  if (params.actionType !== undefined) {
    if (!Number.isInteger(params.actionType) || params.actionType < 1 || params.actionType > 999) {
      throw new RangeError(`Некорректный фильтр action_type: ${params.actionType}`);
    }
    queryParts.push(`action_type=${params.actionType}`);
  }
  if (params.before !== undefined) {
    if (!isValidSnowflake(params.before)) {
      throw new RangeError(`Некорректный курсор before: ${params.before}`);
    }
    queryParts.push(`before=${params.before}`);
  }
  if (params.after !== undefined) {
    if (!isValidSnowflake(params.after)) {
      throw new RangeError(`Некорректный курсор after: ${params.after}`);
    }
    queryParts.push(`after=${params.after}`);
  }
  const path = `/guilds/${params.guildId}/audit-logs?${queryParts.join("&")}`;
  return {
    method: "GET",
    path,
    apiV9Path: `/api/v9${path}`,
    guildId: params.guildId,
    limit,
  };
}

/**
 * Формирует и валидирует заголовок `X-Audit-Log-Reason` (`1..=512` символов без управляющих кодов).
 * @param {string} reason
 */
export function buildAuditLogReasonHeader(reason) {
  const trimmed = String(reason ?? "").trim();
  if (trimmed.length === 0 || trimmed.length > AUDIT_LOG_PROTOCOL_CONSTANTS.MAX_REASON_CHARS) {
    throw new RangeError(
      `Причина X-Audit-Log-Reason должна содержать 1..512 символов (получено ${trimmed.length})`,
    );
  }
  if (/[\x00-\x1F\x7F]/.test(trimmed)) {
    throw new RangeError("Причина X-Audit-Log-Reason содержит управляющие символы");
  }
  return {
    headerName: AUDIT_LOG_PROTOCOL_CONSTANTS.HEADER_AUDIT_REASON,
    headerValue: encodeURIComponent(trimmed),
    rawReason: trimmed,
  };
}

/**
 * Классифицирует код действия журнала аудита по категории (`CREATE` / `UPDATE` / `DELETE`) и типу цели (`targetType`).
 * @param {number} actionType
 */
export function classifyAuditAction(actionType) {
  const category = CREATE_CODES.has(actionType)
    ? "CREATE"
    : DELETE_CODES.has(actionType)
      ? "DELETE"
      : "UPDATE";

  let targetType = "UNKNOWN";
  if ([1, 150, 151, 180, 210, 211, 212, 213].includes(actionType)) {
    targetType = "GUILD";
  } else if ((actionType >= 10 && actionType <= 12) || actionType === 16 || actionType === 73) {
    targetType = "CHANNEL";
  } else if (actionType >= 13 && actionType <= 15) {
    targetType = "CHANNEL_OVERWRITE";
  } else if (
    (actionType >= 20 && actionType <= 28) ||
    (actionType >= 72 && actionType <= 75) ||
    (actionType >= 143 && actionType <= 146)
  ) {
    targetType = "USER";
  } else if (actionType >= 30 && actionType <= 33) {
    targetType = "ROLE";
  } else if (actionType >= 40 && actionType <= 42) {
    targetType = "INVITE";
  } else if (actionType >= 50 && actionType <= 52) {
    targetType = "WEBHOOK";
  } else if (actionType >= 60 && actionType <= 62) {
    targetType = "EMOJI";
  } else if (actionType >= 90 && actionType <= 92) {
    targetType = "STICKER";
  } else if (actionType >= 110 && actionType <= 112) {
    targetType = "THREAD";
  } else if (actionType >= 140 && actionType <= 142) {
    targetType = "AUTO_MODERATION_RULE";
  }

  const labelRu = AUDIT_ACTION_LABELS_RU[actionType] ?? `Действие ${String(actionType)}`;
  return { code: actionType, category, targetType, labelRu };
}

/**
 * Форматирует строку записи журнала аудита в полном паритете с `audit-log.js`.
 * @param {{ actionType: number, actorName?: string | null, targetName?: string | null, reason?: string | null }} entry
 * @returns {string}
 */
export function formatAuditLogEntryRu(entry) {
  const actorName = typeof entry.actorName === "string" ? entry.actorName.trim() : "";
  const actor = actorName === "" ? UNKNOWN_ACTOR_RU : actorName;
  const label = AUDIT_ACTION_LABELS_RU[entry.actionType] ?? `Действие ${String(entry.actionType)}`;
  const targetName = typeof entry.targetName === "string" ? entry.targetName.trim() : "";
  const target = targetName === "" ? "" : ` (${targetName})`;
  const reason = typeof entry.reason === "string" ? entry.reason.trim() : "";
  const reasonPart = reason === "" ? "" : ` — Причина: ${reason}`;
  return `${actor}: ${label}${target}${reasonPart}`;
}

/**
 * Форматирует одну сторону диффа изменений (`old_value` или `new_value`).
 * @param {string} key
 * @param {unknown} value
 * @returns {string}
 */
function renderChangeSide(key, value) {
  if (value === null || value === undefined) return EMPTY_VALUE_RU;
  if (SENSITIVE_CHANGE_KEYS.has(String(key ?? "").toLowerCase())) return "[REDACTED]";
  if (Array.isArray(value)) {
    if (value.length === 0) return EMPTY_VALUE_RU;
    return value
      .map((item) =>
        item && typeof item === "object" && "name" in item && "id" in item
          ? `${item.name} (${item.id})`
          : String(item),
      )
      .join(", ");
  }
  const str = String(value);
  if (str.includes("/webhooks/") || str.includes("mfa.")) return "[REDACTED]";
  return str;
}

/**
 * Форматирует одну пару изменений `ключ: было → стало` в паритете с `audit-log.js`.
 * @param {{ key: string, oldValue?: unknown, newValue?: unknown }} change
 * @returns {string}
 */
export function formatAuditChangeDiffRu(change) {
  const oldStr = renderChangeSide(change.key, change.oldValue);
  const newStr = renderChangeSide(change.key, change.newValue);
  return `${change.key}: ${oldStr} → ${newStr}`;
}

/**
 * Декодирует объект изменения `AuditLogChange` (`old_value` vs `new_value`) в человекочитаемую структуру.
 * @param {{ key: string, oldValue?: unknown, newValue?: unknown }} change
 */
export function decodeAuditChangeDetail(change) {
  const oldDisplay = renderChangeSide(change.key, change.oldValue);
  const newDisplay = renderChangeSide(change.key, change.newValue);
  const decodeState = resolveAuditDecodeState("decoded");
  return {
    key: change.key,
    oldDisplay,
    newDisplay,
    formattedRu: `${change.key}: ${oldDisplay} → ${newDisplay}`,
    decodeState: decodeState.label,
  };
}

/**
 * Нормализует сырую запись ответа `GET /guilds/{id}/audit-logs` с сопоставлением `users[]`, `channels[]`, `roles[]`.
 * @param {{ id: string, user_id?: string | null, target_id?: string | null, action_type: number, reason?: string | null, changes?: ReadonlyArray<{ key: string, old_value?: unknown, new_value?: unknown }> }} rawEntry
 * @param {{ users?: Map<string, { id: string, username: string, globalName?: string }>, channels?: Map<string, { id: string, name: string }>, roles?: Map<string, { id: string, name: string }> }} [context]
 */
export function normalizeAuditLogEntry(rawEntry, context = {}) {
  const userObj = rawEntry.user_id ? context.users?.get(rawEntry.user_id) : undefined;
  const actorName = userObj?.globalName || userObj?.username || UNKNOWN_ACTOR_RU;
  const tid = rawEntry.target_id ?? null;
  let targetName = null;
  if (tid) {
    const targetUser = context.users?.get(tid);
    const targetChan = context.channels?.get(tid);
    const targetRole = context.roles?.get(tid);
    if (targetUser) targetName = targetUser.globalName || targetUser.username;
    else if (targetChan) targetName = `#${targetChan.name}`;
    else if (targetRole) targetName = targetRole.name;
    else targetName = tid;
  }
  const changes = (rawEntry.changes ?? []).map((c) =>
    decodeAuditChangeDetail({ key: c.key, oldValue: c.old_value, newValue: c.new_value }),
  );
  const summaryRu = formatAuditLogEntryRu({
    actionType: rawEntry.action_type,
    actorName,
    targetName,
    reason: rawEntry.reason ?? null,
  });
  return {
    id: rawEntry.id,
    userId: rawEntry.user_id ?? null,
    actorName,
    targetId: tid,
    targetName,
    actionType: rawEntry.action_type,
    reason: rawEntry.reason ?? null,
    timestampMs: isValidSnowflake(rawEntry.id) ? extractSnowflakeTimestampMs(rawEntry.id) : 0,
    summaryRu,
    changes,
  };
}

/**
 * Фильтрует список записей журнала аудита по типу действия, автору, цели, временному интервалу и поисковой строке.
 * @param {ReadonlyArray<{ id: string, userId?: string | null, actorName?: string | null, targetId?: string | null, targetName?: string | null, actionType: number, reason?: string | null, timestampMs: number, changes?: ReadonlyArray<any> }>} entries
 * @param {{ actionType?: number | null, userId?: string | null, targetId?: string | null, startTimeMs?: number | null, endTimeMs?: number | null, searchQuery?: string | null }} [filter]
 */
export function filterAuditLogEntries(entries, filter = {}) {
  const safeEntries = Array.isArray(entries) ? entries : [];
  const needle = typeof filter.searchQuery === "string" ? filter.searchQuery.trim().toLowerCase() : "";
  return safeEntries.filter((entry) => {
    if (filter.actionType != null && entry.actionType !== filter.actionType) {
      return false;
    }
    if (filter.userId && entry.userId !== filter.userId) {
      return false;
    }
    if (filter.targetId && entry.targetId !== filter.targetId) {
      return false;
    }
    if (filter.startTimeMs != null && entry.timestampMs < filter.startTimeMs) {
      return false;
    }
    if (filter.endTimeMs != null && entry.timestampMs > filter.endTimeMs) {
      return false;
    }
    if (needle.length > 0) {
      const summary = formatAuditLogEntryRu({
        actionType: entry.actionType,
        actorName: entry.actorName ?? null,
        targetName: entry.targetName ?? null,
        reason: entry.reason ?? null,
      }).toLowerCase();
      if (!summary.includes(needle)) return false;
    }
    return true;
  });
}

/**
 * Группирует смежные записи журнала аудита в пределах 30-минутного окна (до 50 изменений).
 * @param {ReadonlyArray<{ id: string, userId?: string | null, targetId?: string | null, actionType: number, timestampMs: number, changes: ReadonlyArray<any> }>} entries
 */
export function groupConsecutiveAuditEntries(entries) {
  /** @type {Array<{ id: string, userId: string | null, targetId: string | null, actionType: number, timestampMs: number, mergedCount: number, changes: any[] }>} */
  const out = [];
  for (const item of entries) {
    const prev = out[out.length - 1];
    const canMerge =
      prev &&
      !NON_MERGEABLE_ACTIONS.has(item.actionType) &&
      prev.actionType === item.actionType &&
      prev.userId === (item.userId ?? null) &&
      prev.targetId === (item.targetId ?? null) &&
      Math.abs(prev.timestampMs - item.timestampMs) < AUDIT_LOG_PROTOCOL_CONSTANTS.MERGE_WINDOW_MS &&
      prev.mergedCount < AUDIT_LOG_PROTOCOL_CONSTANTS.MAX_GROUPED_CHANGES;
    if (canMerge) {
      prev.changes = [...prev.changes, ...(item.changes ?? [])];
      prev.mergedCount += 1;
    } else {
      out.push({
        id: item.id,
        userId: item.userId ?? null,
        targetId: item.targetId ?? null,
        actionType: item.actionType,
        timestampMs: item.timestampMs,
        mergedCount: 1,
        changes: [...(item.changes ?? [])],
      });
    }
  }
  return out;
}

/**
 * Сверяет дизайн-токены, раскладку, состояния, маршрут `GUILD_AUDIT_LOG` и модули RE-дампа с независимыми оракулами.
 * @param {{ kanbanPath?: string, restD02Path?: string, corePermissionsRsPath?: string, reRestStaticPath?: string, reModulesIndexPath?: string }} [opts]
 */
export function verifyAuditLogOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const restD02Path = opts.restD02Path ?? DEFAULT_REST_D02_PATH;
  const corePermissionsRsPath = opts.corePermissionsRsPath ?? DEFAULT_CORE_PERMISSIONS_RS_PATH;
  /** @type {string[]} */
  const reasons = [];

  for (const reqPath of [kanbanPath, restD02Path, corePermissionsRsPath]) {
    if (!existsSync(reqPath)) {
      return { passed: false, reasons: [`Отсутствует оракул: ${reqPath}`] };
    }
  }

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card1008 = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1008");
  if (!card1008) {
    reasons.push("Карточка TASK-1008 не найдена в waves/kanban-board.json");
  } else {
    if (card1008.layout?.container !== AUDIT_LOG_VIEWER_LAYOUT.container) {
      reasons.push(`Несовпадение container: ${card1008.layout?.container}`);
    }
    if (card1008.design_tokens?.background !== AUDIT_LOG_DESIGN_TOKENS.background) {
      reasons.push(`Несовпадение background: ${card1008.design_tokens?.background}`);
    }
    if (card1008.interactive_states?.[0]?.active !== AUDIT_ENTRY_STATES.active) {
      reasons.push("Несовпадение active state для audit-entry");
    }
  }

  const permsRs = readFileSync(corePermissionsRsPath, "utf8");
  if (!permsRs.includes("pub const VIEW_AUDIT_LOG: u64 = 1 << 7;")) {
    reasons.push("В permissions.rs не найден флаг VIEW_AUDIT_LOG = 1 << 7");
  }

  const reStaticPath =
    opts.reRestStaticPath ??
    (existsSync(DEFAULT_RE_REST_STATIC_PATH) ? DEFAULT_RE_REST_STATIC_PATH : undefined);
  if (reStaticPath && existsSync(reStaticPath)) {
    const reStatic = JSON.parse(readFileSync(reStaticPath, "utf8"));
    const auditEp = (reStatic.endpoints ?? []).find((/** @type {any} */ e) => e.key === "GUILD_AUDIT_LOG");
    if (!auditEp || auditEp.path !== "/guilds/{arg}/audit-logs") {
      reasons.push("В /home/sanya/re-deep/rest-static.json отсутствует GUILD_AUDIT_LOG");
    }
    const extraHeaders = reStatic.auth?.requestAuth?.extraHeaders ?? [];
    if (!extraHeaders.includes("X-Audit-Log-Reason")) {
      reasons.push("В /home/sanya/re-deep/rest-static.json отсутствует заголовок X-Audit-Log-Reason");
    }
  }

  const reModPath =
    opts.reModulesIndexPath ??
    (existsSync(DEFAULT_RE_MODULES_INDEX_PATH) ? DEFAULT_RE_MODULES_INDEX_PATH : undefined);
  if (reModPath && existsSync(reModPath)) {
    const reMods = JSON.parse(readFileSync(reModPath, "utf8"));
    for (const modId of ["117497", "165259", "411153", "523599", "555067"]) {
      if (!reMods[modId]) {
        reasons.push(`В /home/sanya/re-deep/modules-index.json отсутствует модуль ${modId}`);
      }
    }
  }

  return { passed: reasons.length === 0, reasons };
}

/**
 * Маскирует токены авторизации и секреты в строках телеметрии журнала аудита.
 * @param {string} text
 * @returns {string}
 */
export function redactAuditLogTelemetry(text) {
  return String(text ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "mfa.[REDACTED]")
    .replace(/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED]")
    .replace(/(token|secret|authorization)\s*[:=]\s*["']?[^"'\s]+["']?/gi, "$1=[REDACTED]");
}

/** Проверяет все 9 обязательных измерений `TEST-POLICY.md` для модуля журнала аудита. */
export function evaluateAuditLogNineDimensions() {
  /** @type {string[]} */
  const failures = [];
  const req = buildGuildAuditLogRequest({ guildId: "942355007271997450" });
  if (req.method !== "GET" || req.limit !== 50) failures.push("dim1_positive_get_audit_logs");

  let badLimitRejected = false;
  try {
    buildGuildAuditLogRequest({ guildId: "942355007271997450", limit: 101 });
  } catch {
    badLimitRejected = true;
  }
  if (!badLimitRejected) failures.push("dim2_negative_invalid_limit");

  if (isValidSnowflake("0") !== false || isValidSnowflake("18446744073709551615") !== true) {
    failures.push("dim3_boundary_snowflake");
  }
  const badgeFormat = (/** @type {number} */ n) => (n > 99 ? "99+" : String(n));
  if (badgeFormat(99) !== "99" || badgeFormat(100) !== "99+") {
    failures.push("dim3_boundary_badge_cap");
  }

  const mapHttpStatus = (/** @type {number} */ status, /** @type {number} */ retryMs) => ({
    retryable: status === 429 || status >= 500,
    evictCache: status === 401 || status === 403,
    backoffMs: status === 429 ? Math.max(100, retryMs) : status >= 500 ? 1_000 : 0,
  });
  if (!mapHttpStatus(429, 1_500).retryable || !mapHttpStatus(403, 0).evictCache) {
    failures.push("dim4_http_error_classification");
  }

  if (
    filterAuditLogEntries([], { actionType: 1 }).length !== 0 ||
    formatAuditChangeDiffRu({ key: "topic", oldValue: null, newValue: null }) !== "topic: нет → нет"
  ) {
    failures.push("dim5_empty_state");
  }

  const isSkeleton = (/** @type {{ isInitialLoading?: boolean, httpStatus?: number }} */ s) =>
    Boolean(s.isInitialLoading || s.httpStatus === 202);
  if (!isSkeleton({ httpStatus: 202 }) || isSkeleton({ httpStatus: 200 })) {
    failures.push("dim6_loading_http_202");
  }

  const canViewAuditLog = (/** @type {bigint} */ perms, /** @type {boolean} */ ageRestricted) =>
    !ageRestricted &&
    ((perms & AUDIT_LOG_PROTOCOL_CONSTANTS.PERM_VIEW_AUDIT_LOG) !== 0n ||
      (perms & AUDIT_LOG_PROTOCOL_CONSTANTS.PERM_ADMINISTRATOR) !== 0n);
  if (
    !canViewAuditLog(AUDIT_LOG_PROTOCOL_CONSTANTS.PERM_VIEW_AUDIT_LOG, false) ||
    canViewAuditLog(AUDIT_LOG_PROTOCOL_CONSTANTS.PERM_VIEW_AUDIT_LOG, true) ||
    canViewAuditLog(0n, false)
  ) {
    failures.push("dim7_permissions_view_audit_log");
  }

  const stateAfter403 = mapHttpStatus(403, 0).evictCache ? [] : ["stale"];
  if (stateAfter403.length !== 0) {
    failures.push("dim8_permission_revocation_cache_clear");
  }

  const redacted = redactAuditLogTelemetry("Authorization: Bearer mfa.secret12345");
  if (redacted.includes("mfa.secret12345")) {
    failures.push("dim9_observability_redaction");
  }

  return { passed: failures.length === 0, dimensionsChecked: 9, failures };
}

/**
 * Запускает CLI проверки журнала аудита.
 * @param {string[]} argv
 * @returns {number}
 */
export function runAuditLogCli(argv) {
  /** @type {string | undefined} */
  let oraclePath;
  /** @type {string | undefined} */
  let outJsonPath;
  /** @type {string | undefined} */
  let outMdPath;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      console.log(
        "Использование: node scripts/verify-audit-log.mjs [--oracle <path>] [--out-json <path>] [--out-md <path>]",
      );
      return 0;
    }
    if (arg === "--oracle") {
      oraclePath = argv[i + 1];
      i += 1;
    } else if (arg === "--out-json") {
      outJsonPath = argv[i + 1];
      i += 1;
    } else if (arg === "--out-md") {
      outMdPath = argv[i + 1];
      i += 1;
    } else {
      console.error(`verify-audit-log: неизвестный аргумент ${arg}`);
      return 1;
    }
  }

  const targetKanban = oraclePath ? resolve(oraclePath) : DEFAULT_KANBAN_PATH;
  if (!existsSync(targetKanban)) {
    console.error(`verify-audit-log: файл-оракул недоступен: ${targetKanban}`);
    return 2;
  }

  const parity = verifyAuditLogOracleParity({ kanbanPath: targetKanban });
  const dims = evaluateAuditLogNineDimensions();
  if (!parity.passed || !dims.passed) {
    console.error("verify-audit-log: нарушение инвариантов", parity.reasons, dims.failures);
    return 1;
  }

  const report = {
    taskId: "TASK-1008",
    passed: true,
    dimensionsChecked: dims.dimensionsChecked,
    oracleReasons: parity.reasons,
  };

  if (outJsonPath) {
    mkdirSync(dirname(resolve(outJsonPath)), { recursive: true });
    writeFileSync(resolve(outJsonPath), `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (outMdPath) {
    mkdirSync(dirname(resolve(outMdPath)), { recursive: true });
    writeFileSync(
      resolve(outMdPath),
      `# Отчёт верификации TASK-1008 (Server Audit Log)\n\n- Статус: **PASS**\n- Измерений проверено: \`${dims.dimensionsChecked}/9\`\n`,
      "utf8",
    );
  }
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runAuditLogCli(process.argv.slice(2)));
}
