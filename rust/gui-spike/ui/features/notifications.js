// @ts-check
/**
 * N-05-UI-NOTIFICATIONS (RW-40): настройки уведомлений сервера и каналов,
 * каскад разрешения, пресеты таймера mute, индикаторы непрочитанного/бейджей
 * и контроллер модального окна `#guild-notif-modal` + контекстного меню `#channel-notif-menu`.
 *
 * Нормативные источники:
 * - `waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md` (§1–§6);
 * - `waves/RW-40/NOTIFICATIONS-RE.md` (§1–§12).
 */

/** Шесть канонических пресетов длительности mute в секундах (`@8349873` `Xx`, `@6225743` `b8()`). */
export const MUTE_TIMER_PRESETS = Object.freeze([900, 3600, 10800, 28800, 86400, -1]);

/** Метаданные 6 опций таймера приглушения (`[D] NOTIFICATIONS-SPEC.md §1.3.1`). */
export const MUTE_TIMER_OPTIONS = Object.freeze([
  Object.freeze({ id: "15-minutes", window: 900, selected_time_window: 900, label: "На 15 минут" }),
  Object.freeze({ id: "1-hour", window: 3600, selected_time_window: 3600, label: "На 1 час" }),
  Object.freeze({ id: "3-hours", window: 10800, selected_time_window: 10800, label: "На 3 часа" }),
  Object.freeze({ id: "8-hours", window: 28800, selected_time_window: 28800, label: "На 8 часов" }),
  Object.freeze({ id: "24-hours", window: 86400, selected_time_window: 86400, label: "На 24 часа" }),
  Object.freeze({ id: "forever", window: -1, selected_time_window: -1, label: "Пока не включу" }),
]);

/** Коды уровня уведомлений `message_notifications` (`orn`, модуль `652215`, `@313815`). */
export const MESSAGE_NOTIFICATION_LEVELS = Object.freeze({
  ALL_MESSAGES: 0,
  ONLY_MENTIONS: 1,
  NO_MESSAGES: 2,
  NULL: 3,
});

/** Русские подписи уровней уведомлений (`NOTIFICATIONS-SPEC.md §1.2.1`). */
export const MESSAGE_NOTIFICATION_LABELS = Object.freeze({
  0: "Все сообщения",
  1: "Только @упоминания",
  2: "Ничего",
  3: "По умолчанию",
});

/** Коды уведомлений о хайлайтах `notify_highlights` (`guM`, модуль `652215`, `@529852`). */
export const NOTIFY_HIGHLIGHTS = Object.freeze({
  NULL: 0,
  DISABLED: 1,
  ENABLED: 2,
});

/** Битовые маски `flags` гильдии (`модуль 355097`, `@8352891`). */
export const GUILD_NOTIFICATION_FLAGS = Object.freeze({
  UNREADS_ALL_MESSAGES: 2048,
  UNREADS_ONLY_MENTIONS: 4096,
  OPT_IN_CHANNELS_OFF: 8192,
  OPT_IN_CHANNELS_ON: 16384,
});

/** Битовые маски `flags` канала (`модуль 355097`, `@8352746`). */
export const CHANNEL_NOTIFICATION_FLAGS = Object.freeze({
  UNREADS_ONLY_MENTIONS: 512,
  UNREADS_ALL_MESSAGES: 1024,
  FAVORITED: 2048,
  OPT_IN_ENABLED: 4096,
  NEW_FORUM_THREADS_OFF: 8192,
  NEW_FORUM_THREADS_ON: 16384,
});

/** Пара битов unread для гильдии (`2048 | 4096 = 6144`). */
export const GUILD_UNREAD_MASK =
  GUILD_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES | GUILD_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS;

/** Пара битов unread для канала (`512 | 1024 = 1536`). */
export const CHANNEL_UNREAD_MASK =
  CHANNEL_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS | CHANNEL_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES;

/** Порог числа участников для предупреждения `Dh5p5j` (`NOTIFICATIONS-SPEC.md §1.2.1`). */
export const LARGE_GUILD_MEMBER_THRESHOLD = 2500;

/**
 * Строгий разбор булевых значений и строковых ETF-атомов `"true"` / `"false"` (закрытие D8).
 * В отличие от `Boolean("false") === true`, явно проверяет допустимые представления.
 * @param {unknown} value
 * @param {boolean} [fallback]
 * @returns {boolean}
 */
export function parseStrictBool(value, fallback = false) {
  if (value === true || value === "true") return true;
  if (value === false || value === "false") return false;
  return fallback;
}

/**
 * Разбирает ISO-8601 `end_time` из `mute_config` в миллисекунды эпохи.
 * `"nil"`, `null`, `undefined` или невалидная дата возвращают `null`.
 * @param {any} muteConfig
 * @returns {number | null}
 */
export function parseEndTimeMs(muteConfig) {
  if (!muteConfig || muteConfig === "nil" || typeof muteConfig !== "object") return null;
  const raw = muteConfig.end_time;
  if (raw === null || raw === undefined || raw === "nil" || raw === "") return null;
  const parsed = Date.parse(String(raw));
  return Number.isNaN(parsed) ? null : parsed;
}

/**
 * Формирует объект `mute_config` при выборе окна длительности (`@11993`).
 * Для `-1` («Пока не включу») `end_time = null`; для положительного окна — `nowMs + window * 1000`.
 * @param {number} selectedTimeWindow
 * @param {number} [nowMs]
 * @returns {{ selected_time_window: number, end_time: string | null }}
 */
export function buildMuteConfig(selectedTimeWindow, nowMs = Date.now()) {
  const win = Number(selectedTimeWindow);
  if (!MUTE_TIMER_PRESETS.includes(win) || win === -1) {
    return { selected_time_window: -1, end_time: null };
  }
  const endIso = new Date(nowMs + win * 1000).toISOString();
  return {
    selected_time_window: win,
    end_time: endIso,
  };
}

/**
 * Вычисляет эффективное состояние mute и истечение таймера (`@2364848`, модуль `544743`).
 * Инварианты:
 * - `muted: true` + `mute_config: null` (или `end_time: null/"nil"`) -> валидный бессрочный mute;
 * - `muted: true` + `endMs < nowMs` -> истёкший таймер (`isMuted = false`, `isExpired = true`).
 * @param {any} entry
 * @param {number} [nowMs]
 * @returns {{
 *   isMuted: boolean,
 *   isTemporarilyMuted: boolean,
 *   isExpired: boolean,
 *   endTimeMs: number | null,
 *   remainingMs: number | null,
 *   selectedTimeWindow: number | null,
 * }}
 */
export function evaluateMuteState(entry, nowMs = Date.now()) {
  if (!entry || typeof entry !== "object") {
    return {
      isMuted: false,
      isTemporarilyMuted: false,
      isExpired: false,
      endTimeMs: null,
      remainingMs: null,
      selectedTimeWindow: null,
    };
  }
  const rawMuted = parseStrictBool(entry.muted, false);
  const cfg = entry.mute_config && entry.mute_config !== "nil" ? entry.mute_config : null;
  const endTimeMs = parseEndTimeMs(cfg);
  const selectedTimeWindow =
    cfg && typeof cfg.selected_time_window === "number" ? cfg.selected_time_window : null;

  const isMuted = rawMuted && (endTimeMs === null || endTimeMs >= nowMs);
  const isTemporarilyMuted = rawMuted && endTimeMs !== null && endTimeMs >= nowMs;
  const isExpired = rawMuted && endTimeMs !== null && endTimeMs < nowMs;
  const remainingMs = isTemporarilyMuted && endTimeMs !== null ? Math.max(0, endTimeMs - nowMs) : null;

  return {
    isMuted,
    isTemporarilyMuted,
    isExpired,
    endTimeMs,
    remainingMs,
    selectedTimeWindow,
  };
}

/**
 * Устанавливает бит unread (`mD(flags, bit)` из `@11923745`), очищая парный бит unread
 * и сохраняя старшие биты (`OPT_IN_ENABLED = 4096`, `FAVORITED = 2048` и т.д.).
 * @param {number} flags
 * @param {number} bit
 * @param {number} [mask]
 * @returns {number}
 */
export function setUnreadFlag(flags, bit, mask = CHANNEL_UNREAD_MASK) {
  const base = Number.isFinite(flags) ? Number(flags) : 0;
  return (base & ~mask) | bit;
}

/**
 * Сбрасывает биты unread (`PW(flags)` из `@11923745`), сохраняя все старшие биты.
 * @param {number} flags
 * @param {number} [mask]
 * @returns {number}
 */
export function clearUnreadFlags(flags, mask = CHANNEL_UNREAD_MASK) {
  const base = Number.isFinite(flags) ? Number(flags) : 0;
  return base & ~mask;
}

/**
 * Нормализует `channel_overrides` из массива (форма READY) или словаря (форма апдейта/REST)
 * в словарь `{ [channelId]: NormalizedChannelOverride }` (`@9385738`).
 * @param {any} rawOverrides
 * @returns {Record<string, {
 *   channel_id: string,
 *   collapsed: boolean,
 *   flags?: number,
 *   message_notifications: number,
 *   mute_config: { end_time: string | null, selected_time_window: number } | null,
 *   muted: boolean,
 * }>}
 */
export function normalizeChannelOverrides(rawOverrides) {
  /** @type {Record<string, any>} */
  const map = {};
  if (!rawOverrides || rawOverrides === "nil") return map;

  /**
   * @param {string} keyId
   * @param {any} item
   */
  const putItem = (keyId, item) => {
    if (!item || typeof item !== "object") return;
    const id = String(item.channel_id ?? keyId ?? "").trim();
    if (!id) return;
    const rawLevel = Number(item.message_notifications);
    const level =
      rawLevel === 0 || rawLevel === 1 || rawLevel === 2 || rawLevel === 3 ? rawLevel : 3;
    const rawCfg = item.mute_config && item.mute_config !== "nil" ? item.mute_config : null;
    const muteConfig = rawCfg
      ? {
          end_time:
            rawCfg.end_time && rawCfg.end_time !== "nil" ? String(rawCfg.end_time) : null,
          selected_time_window:
            typeof rawCfg.selected_time_window === "number" ? rawCfg.selected_time_window : -1,
        }
      : null;

    /** @type {any} */
    const normalized = {
      channel_id: id,
      collapsed: parseStrictBool(item.collapsed, false),
      message_notifications: level,
      mute_config: muteConfig,
      muted: parseStrictBool(item.muted, false),
    };
    if (item.flags !== undefined && item.flags !== null && item.flags !== "nil") {
      normalized.flags = Number(item.flags);
    }
    map[id] = normalized;
  };

  if (Array.isArray(rawOverrides)) {
    for (const entry of rawOverrides) {
      if (entry && typeof entry === "object") {
        putItem(String(entry.channel_id ?? ""), entry);
      }
    }
  } else if (typeof rawOverrides === "object") {
    for (const [key, entry] of Object.entries(rawOverrides)) {
      putItem(key, entry);
    }
  }
  return map;
}

/**
 * Определяет, должна ли запись `channel_override` отображаться в UI-таблице
 * «Переопределения уведомлений каналов» (`NOTIFICATIONS-SPEC.md §1.4 п.2`).
 * Записи, созданные только ради `collapsed=true` или `OPT_IN_ENABLED=4096`, скрываются.
 * @param {any} override
 * @param {number} [nowMs]
 * @returns {boolean}
 */
export function hasVisibleNotificationOverride(override, nowMs = Date.now()) {
  if (!override || typeof override !== "object") return false;
  if (evaluateMuteState(override, nowMs).isMuted) return true;
  if (
    override.message_notifications !== undefined &&
    override.message_notifications !== null &&
    Number(override.message_notifications) !== MESSAGE_NOTIFICATION_LEVELS.NULL
  ) {
    return true;
  }
  const flags = Number(override.flags ?? 0);
  return (flags & CHANNEL_UNREAD_MASK) !== 0;
}

/**
 * Применяет частичный апдейт `USER_GUILD_SETTINGS_UPDATE` к записи гильдии (`NOTIFICATIONS-SPEC.md §1.6.1`).
 * Защитный инвариант: отсутствие ключа `channel_overrides` в апдейте сохраняет существующую карту,
 * а устаревшая версия (`incoming.version < current.version`) игнорируется.
 * @param {any} currentEntry
 * @param {any} incomingUpdate
 * @param {number} [defaultMessageNotifications]
 * @returns {any}
 */
export function mergeGuildSettingsUpdate(
  currentEntry,
  incomingUpdate,
  defaultMessageNotifications = MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
) {
  const fallbackLevel =
    defaultMessageNotifications === 0 || defaultMessageNotifications === 1
      ? defaultMessageNotifications
      : 1;
  const base = currentEntry && typeof currentEntry === "object" ? currentEntry : {};
  const incoming = incomingUpdate && typeof incomingUpdate === "object" ? incomingUpdate : {};

  const curVer = Number(base.version ?? 0);
  const incVer = Number(incoming.version ?? 0);
  if (curVer > 0 && incVer > 0 && incVer < curVer) {
    return {
      ...base,
      channel_overrides: normalizeChannelOverrides(base.channel_overrides),
    };
  }

  const existingOverrides = normalizeChannelOverrides(base.channel_overrides);
  const nextOverrides =
    "channel_overrides" in incoming && incoming.channel_overrides !== undefined
      ? normalizeChannelOverrides(incoming.channel_overrides)
      : existingOverrides;

  const rawLevel =
    incoming.message_notifications !== undefined
      ? Number(incoming.message_notifications)
      : base.message_notifications !== undefined
        ? Number(base.message_notifications)
        : fallbackLevel;
  const validGuildLevel = rawLevel === 0 || rawLevel === 1 || rawLevel === 2 ? rawLevel : fallbackLevel;

  return {
    guild_id: incoming.guild_id !== undefined ? incoming.guild_id : (base.guild_id ?? null),
    message_notifications: validGuildLevel,
    suppress_everyone:
      incoming.suppress_everyone !== undefined
        ? parseStrictBool(incoming.suppress_everyone, false)
        : parseStrictBool(base.suppress_everyone, false),
    suppress_roles:
      incoming.suppress_roles !== undefined
        ? parseStrictBool(incoming.suppress_roles, false)
        : parseStrictBool(base.suppress_roles, false),
    muted:
      incoming.muted !== undefined
        ? parseStrictBool(incoming.muted, false)
        : parseStrictBool(base.muted, false),
    mute_config:
      incoming.mute_config !== undefined
        ? incoming.mute_config === "nil"
          ? null
          : incoming.mute_config
        : base.mute_config === "nil"
          ? null
          : (base.mute_config ?? null),
    mobile_push:
      incoming.mobile_push !== undefined
        ? parseStrictBool(incoming.mobile_push, true)
        : parseStrictBool(base.mobile_push, true),
    notify_highlights:
      incoming.notify_highlights !== undefined
        ? Number(incoming.notify_highlights)
        : Number(base.notify_highlights ?? 0),
    flags: incoming.flags !== undefined ? Number(incoming.flags) : Number(base.flags ?? 0),
    hide_muted_channels:
      incoming.hide_muted_channels !== undefined
        ? parseStrictBool(incoming.hide_muted_channels, false)
        : parseStrictBool(base.hide_muted_channels, false),
    mute_scheduled_events:
      incoming.mute_scheduled_events !== undefined
        ? parseStrictBool(incoming.mute_scheduled_events, false)
        : parseStrictBool(base.mute_scheduled_events, false),
    version: Math.max(curVer, incVer),
    channel_overrides: nextOverrides,
  };
}

/**
 * Проверяет, является ли число валидным явным уровнем уведомлений `{0, 1, 2}` (не `3 = NULL`).
 * @param {unknown} value
 * @returns {value is 0 | 1 | 2}
 */
function isExplicitLevel(value) {
  return value === 0 || value === 1 || value === 2;
}

/**
 * Разрешает эффективные настройки уведомлений канала по каскаду:
 * `channel_override (!= 3)` -> `category_override (!= 3)` -> `guild_entry` -> `guild.default_message_notifications`
 * (`@9387678`, `@9388508`, `NOTIFICATIONS-SPEC.md §1.5`).
 * @param {{
 *   guildEntry?: any,
 *   channel?: { id?: string, parent_id?: string | null, parentId?: string | null } | null,
 *   channelOverride?: any,
 *   categoryOverride?: any,
 *   defaultMessageNotifications?: number,
 *   nowMs?: number,
 * }} [params]
 * @returns {{
 *   resolvedMessageNotifications: number,
 *   resolvedUnread: number,
 *   source: "channel" | "category" | "guild" | "default",
 *   isGuildMuted: boolean,
 *   isCategoryMuted: boolean,
 *   isChannelMuted: boolean,
 *   isMuted: boolean,
 *   isOptInSuppressed: boolean,
 *   allowNoMessages: boolean,
 *   allowDesktopNotifications: boolean,
 *   allowAllMessages: boolean,
 *   suppressEveryone: boolean,
 *   suppressRoles: boolean,
 *   notifyHighlights: number,
 *   muteScheduledEvents: boolean,
 *   mobilePush: boolean,
 * }}
 */
export function resolveEffectiveNotifications(params = {}) {
  const {
    guildEntry = null,
    channel = null,
    defaultMessageNotifications = MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
    nowMs = Date.now(),
  } = params;

  const overridesMap = normalizeChannelOverrides(guildEntry?.channel_overrides);
  const channelId = channel?.id ? String(channel.id) : "";
  const parentIdRaw = channel?.parent_id ?? channel?.parentId ?? null;
  const parentId = parentIdRaw && parentIdRaw !== "nil" ? String(parentIdRaw) : "";

  const chOverride =
    params.channelOverride !== undefined
      ? params.channelOverride
      : channelId
        ? (overridesMap[channelId] ?? null)
        : null;
  const catOverride =
    params.categoryOverride !== undefined
      ? params.categoryOverride
      : parentId
        ? (overridesMap[parentId] ?? null)
        : null;

  const fallbackDefault =
    defaultMessageNotifications === 0 || defaultMessageNotifications === 1
      ? defaultMessageNotifications
      : MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS;

  const chLevel = chOverride ? Number(chOverride.message_notifications) : 3;
  const catLevel = catOverride ? Number(catOverride.message_notifications) : 3;
  const guildLevel =
    guildEntry && guildEntry.message_notifications !== undefined
      ? Number(guildEntry.message_notifications)
      : NaN;

  /** @type {number} */
  let resolvedMessageNotifications = fallbackDefault;
  /** @type {"channel" | "category" | "guild" | "default"} */
  let source = "default";

  if (isExplicitLevel(chLevel)) {
    resolvedMessageNotifications = chLevel;
    source = "channel";
  } else if (isExplicitLevel(catLevel)) {
    resolvedMessageNotifications = catLevel;
    source = "category";
  } else if (isExplicitLevel(guildLevel)) {
    resolvedMessageNotifications = guildLevel;
    source = "guild";
  }

  const isGuildMuted = evaluateMuteState(guildEntry, nowMs).isMuted;
  const isCategoryMuted = evaluateMuteState(catOverride, nowMs).isMuted;
  const isChannelMuted = evaluateMuteState(chOverride, nowMs).isMuted;
  const isMuted = isGuildMuted || isCategoryMuted || isChannelMuted;

  const guildFlags = Number(guildEntry?.flags ?? 0);
  const chFlags = Number(chOverride?.flags ?? 0);
  const catFlags = Number(catOverride?.flags ?? 0);
  const optInChannelsOn = (guildFlags & GUILD_NOTIFICATION_FLAGS.OPT_IN_CHANNELS_ON) !== 0;
  const channelOptedIn =
    (chFlags & CHANNEL_NOTIFICATION_FLAGS.OPT_IN_ENABLED) !== 0 ||
    (catFlags & CHANNEL_NOTIFICATION_FLAGS.OPT_IN_ENABLED) !== 0;
  const isOptInSuppressed = optInChannelsOn && !channelOptedIn;

  const allowNoMessages =
    isMuted ||
    resolvedMessageNotifications === MESSAGE_NOTIFICATION_LEVELS.NO_MESSAGES ||
    isOptInSuppressed;
  const allowDesktopNotifications = !allowNoMessages;
  const allowAllMessages =
    !allowNoMessages && resolvedMessageNotifications === MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES;

  /** @type {number} */
  let resolvedUnread =
    resolvedMessageNotifications === MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES
      ? MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES
      : MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS;
  if ((chFlags & CHANNEL_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES) !== 0) {
    resolvedUnread = MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES;
  } else if ((chFlags & CHANNEL_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS) !== 0) {
    resolvedUnread = MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS;
  } else if ((catFlags & CHANNEL_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES) !== 0) {
    resolvedUnread = MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES;
  } else if ((catFlags & CHANNEL_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS) !== 0) {
    resolvedUnread = MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS;
  } else if ((guildFlags & GUILD_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES) !== 0) {
    resolvedUnread = MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES;
  } else if ((guildFlags & GUILD_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS) !== 0) {
    resolvedUnread = MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS;
  }

  const suppressEveryone = parseStrictBool(guildEntry?.suppress_everyone, false);
  const suppressRoles = parseStrictBool(guildEntry?.suppress_roles, false);
  const rawHighlights = Number(guildEntry?.notify_highlights ?? 0);
  const notifyHighlights =
    rawHighlights === 0 || rawHighlights === 1 || rawHighlights === 2 ? rawHighlights : 0;
  const muteScheduledEvents = parseStrictBool(guildEntry?.mute_scheduled_events, false);
  const mobilePush = parseStrictBool(guildEntry?.mobile_push, true);

  return {
    resolvedMessageNotifications,
    resolvedUnread,
    source,
    isGuildMuted,
    isCategoryMuted,
    isChannelMuted,
    isMuted,
    isOptInSuppressed,
    allowNoMessages,
    allowDesktopNotifications,
    allowAllMessages,
    suppressEveryone,
    suppressRoles,
    notifyHighlights,
    muteScheduledEvents,
    mobilePush,
  };
}

/**
 * Вычисляет реакцию клиента на входящее сообщение по матрице `NOTIFICATIONS-SPEC.md §1.5 п.3`:
 * - Прямое `@user` всегда увеличивает бейдж `mention_count` (даже при `muted` и `NO_MESSAGES`),
 *   но звуковое/desktop-уведомление срабатывает только при `!allowNoMessages`;
 * - `@role` увеличивает бейдж при `!suppress_roles`, а звук/desktop — при `!suppress_roles && !allowNoMessages`;
 * - `@everyone`/`@here` увеличивает бейдж при `!suppress_everyone`, а звук/desktop — при `!suppress_everyone && !allowNoMessages`;
 * - Обычное сообщение даёт звук/desktop только при `resolvedMessageNotifications === 0 && !allowNoMessages`.
 * @param {{
 *   guildEntry?: any,
 *   channel?: { id?: string, parent_id?: string | null, parentId?: string | null } | null,
 *   channelOverride?: any,
 *   categoryOverride?: any,
 *   defaultMessageNotifications?: number,
 *   message?: {
 *     mentionsUser?: boolean,
 *     mentionsRole?: boolean,
 *     mentionsEveryone?: boolean,
 *   } | null,
 *   nowMs?: number,
 * }} [params]
 * @returns {{
 *   incrementsMentionBadge: boolean,
 *   mentionBadgeDelta: number,
 *   shouldNotifyDesktop: boolean,
 *   marksUnread: boolean,
 *   allowNoMessages: boolean,
 *   resolvedMessageNotifications: number,
 * }}
 */
export function evaluateIncomingMessageNotification(params = {}) {
  const effective = resolveEffectiveNotifications(params);
  const msg = params.message ?? {};
  const mentionsUser = Boolean(msg.mentionsUser);
  const mentionsRole = Boolean(msg.mentionsRole);
  const mentionsEveryone = Boolean(msg.mentionsEveryone);

  const effectiveRoleMention = mentionsRole && !effective.suppressRoles;
  const effectiveEveryoneMention = mentionsEveryone && !effective.suppressEveryone;

  const incrementsMentionBadge =
    mentionsUser || effectiveRoleMention || effectiveEveryoneMention;
  const mentionBadgeDelta = incrementsMentionBadge ? 1 : 0;

  const hasAnyExplicitMention = mentionsUser || mentionsRole || mentionsEveryone;
  let shouldNotifyDesktop = false;
  if (!effective.allowNoMessages) {
    if (mentionsUser || effectiveRoleMention || effectiveEveryoneMention) {
      shouldNotifyDesktop = true;
    } else if (
      !hasAnyExplicitMention &&
      effective.resolvedMessageNotifications === MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES
    ) {
      shouldNotifyDesktop = true;
    }
  }

  const chMutedOrCatMuted = effective.isChannelMuted || effective.isCategoryMuted;
  const marksUnread =
    mentionsUser ||
    (!chMutedOrCatMuted &&
      (effectiveRoleMention ||
        effectiveEveryoneMention ||
        (!effective.isOptInSuppressed &&
          effective.resolvedUnread === MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES)));

  return {
    incrementsMentionBadge,
    mentionBadgeDelta,
    shouldNotifyDesktop,
    marksUnread,
    allowNoMessages: effective.allowNoMessages,
    resolvedMessageNotifications: effective.resolvedMessageNotifications,
  };
}

/**
 * Склоняет существительное «упоминание» для русского `aria-label`.
 * @param {number} count
 * @returns {string}
 */
function formatMentionCountRu(count) {
  const abs = Math.abs(Math.trunc(count));
  const mod100 = abs % 100;
  const mod10 = abs % 10;
  if (mod100 >= 11 && mod100 <= 19) return `${abs} упоминаний`;
  if (mod10 === 1) return `${abs} упоминание`;
  if (mod10 >= 2 && mod10 <= 4) return `${abs} упоминания`;
  return `${abs} упоминаний`;
}

/**
 * Вычисляет состояние индикаторов строки канала в `#channel-tree` (`NOTIFICATIONS-SPEC.md §2.4.1, §4.2`).
 * @param {{
 *   name?: string,
 *   unread?: boolean,
 *   mentionCount?: number,
 *   isChannelMuted?: boolean,
 *   isCategoryMuted?: boolean,
 *   resolvedMessageNotifications?: number,
 *   hideMutedChannels?: boolean,
 *   isCurrentPage?: boolean,
 * }} [params]
 * @returns {{
 *   unread: boolean,
 *   isMuted: boolean,
 *   hasMention: boolean,
 *   mentionCount: number,
 *   badgeText: string,
 *   showUnreadDot: boolean,
 *   showMuteIcon: boolean,
 *   hiddenByHideMuted: boolean,
 *   notifLevel: number,
 *   ariaLabel: string,
 * }}
 */
export function computeChannelIndicatorState(params = {}) {
  const name = String(params.name ?? "").trim() || "канал";
  const isMuted = Boolean(params.isChannelMuted || params.isCategoryMuted);
  const mentionCount = Math.max(0, Math.trunc(Number(params.mentionCount ?? 0)));
  const hasMention = mentionCount > 0;
  const badgeText = hasMention ? (mentionCount > 99 ? "99+" : String(mentionCount)) : "";
  const unread = Boolean(params.unread) || hasMention;
  const showUnreadDot = Boolean(params.unread) && !isMuted && !hasMention;
  const showMuteIcon = isMuted && !hasMention;
  const hiddenByHideMuted =
    Boolean(params.hideMutedChannels) && isMuted && !hasMention && !params.isCurrentPage;
  const notifLevel =
    typeof params.resolvedMessageNotifications === "number"
      ? params.resolvedMessageNotifications
      : MESSAGE_NOTIFICATION_LEVELS.NULL;

  /** @type {string[]} */
  const labelParts = [name];
  if (isMuted) {
    labelParts.push("канал заглушён");
    if (hasMention) labelParts.push(formatMentionCountRu(mentionCount));
  } else if (hasMention) {
    labelParts.push("непрочитано", formatMentionCountRu(mentionCount));
  } else if (showUnreadDot) {
    labelParts.push("непрочитано");
  }

  return {
    unread,
    isMuted,
    hasMention,
    mentionCount,
    badgeText,
    showUnreadDot,
    showMuteIcon,
    hiddenByHideMuted,
    notifLevel,
    ariaLabel: labelParts.join(", "),
  };
}

/**
 * Применяет вычисленное состояние индикаторов к кнопке `.channel-item` в `#channel-tree`.
 * @param {any} buttonEl
 * @param {ReturnType<typeof computeChannelIndicatorState>} state
 */
export function applyChannelIndicatorToDom(buttonEl, state) {
  if (!buttonEl || !state) return;
  const doc = buttonEl.ownerDocument;
  const showUnreadClass = state.unread && !state.isMuted;

  if (showUnreadClass) buttonEl.classList.add("is-unread");
  else buttonEl.classList.remove("is-unread");

  if (state.isMuted) buttonEl.classList.add("is-muted");
  else buttonEl.classList.remove("is-muted");

  if (state.hasMention) buttonEl.classList.add("has-mention");
  else buttonEl.classList.remove("has-mention");

  buttonEl.setAttribute("data-unread", state.unread && !state.isMuted ? "true" : "false");
  buttonEl.setAttribute("data-muted", state.isMuted ? "true" : "false");
  buttonEl.setAttribute("data-mention-count", String(state.mentionCount));
  buttonEl.setAttribute("data-notif-level", String(state.notifLevel));
  buttonEl.setAttribute("aria-label", state.ariaLabel);

  /** @type {any} */
  let treeEl = buttonEl.parentNode ?? null;
  while (treeEl) {
    if (
      typeof treeEl.getAttribute === "function" &&
      (treeEl.id === "channel-tree" || treeEl.getAttribute("data-hide-muted") !== null)
    ) {
      break;
    }
    treeEl = treeEl.parentNode ?? null;
  }
  const treeHideAttr = treeEl && typeof treeEl.getAttribute === "function" ? treeEl.getAttribute("data-hide-muted") : null;
  const effectiveHideMuted =
    treeHideAttr !== null ? treeHideAttr === "true" : Boolean(state.hiddenByHideMuted);
  const isCurrentPage = buttonEl.getAttribute("aria-current") === "page";
  const shouldHideMuted =
    effectiveHideMuted && state.isMuted && !state.hasMention && !isCurrentPage;

  buttonEl.setAttribute("data-muted-hidden", shouldHideMuted ? "true" : "false");
  if (shouldHideMuted) buttonEl.setAttribute("hidden", "");
  else buttonEl.removeAttribute("hidden");

  let badge = buttonEl.querySelector(".mention-badge");
  if (state.hasMention) {
    if (!badge && doc) {
      badge = doc.createElement("span");
      badge.className = "mention-badge";
      badge.setAttribute("aria-hidden", "true");
      buttonEl.append(badge);
    }
    if (badge) {
      badge.textContent = state.badgeText;
      badge.removeAttribute("hidden");
    }
  } else if (badge) {
    badge.remove();
  }

  let muteIcon = buttonEl.querySelector(".channel-mute-icon");
  if (state.showMuteIcon) {
    if (!muteIcon && doc) {
      muteIcon = doc.createElement("span");
      muteIcon.className = "channel-mute-icon";
      muteIcon.setAttribute("aria-hidden", "true");
      muteIcon.setAttribute("title", "Канал заглушён");
      muteIcon.textContent = "🔕";
      buttonEl.append(muteIcon);
    }
  } else if (muteIcon) {
    muteIcon.remove();
  }
}

/**
 * Вычисляет состояние индикаторов плитки сервера `.guild-item` в `#guild-rail` (`NOTIFICATIONS-SPEC.md §2.4.2, §4.2`).
 * @param {{
 *   name?: string,
 *   unread?: boolean,
 *   mentionCount?: number,
 *   isGuildMuted?: boolean,
 *   isSelected?: boolean,
 * }} [params]
 * @returns {{
 *   unread: boolean,
 *   isMuted: boolean,
 *   hasMention: boolean,
 *   mentionCount: number,
 *   badgeText: string,
 *   showUnreadPill: boolean,
 *   showMentionBadge: boolean,
 *   ariaLabel: string,
 * }}
 */
export function computeGuildRailIndicatorState(params = {}) {
  const name = String(params.name ?? "").trim() || "Сервер";
  const isMuted = Boolean(params.isGuildMuted);
  const mentionCount = Math.max(0, Math.trunc(Number(params.mentionCount ?? 0)));
  const hasMention = mentionCount > 0;
  const badgeText = hasMention ? (mentionCount > 99 ? "99+" : String(mentionCount)) : "";
  const unread = Boolean(params.unread) || hasMention;
  const showUnreadPill = Boolean(params.unread) && !isMuted && !params.isSelected;
  const showMentionBadge = hasMention;

  /** @type {string[]} */
  const parts = [`Сервер ${name}`];
  if (isMuted) parts.push("заглушён");
  else if (unread) parts.push("непрочитано");
  if (hasMention) parts.push(formatMentionCountRu(mentionCount));

  return {
    unread,
    isMuted,
    hasMention,
    mentionCount,
    badgeText,
    showUnreadPill,
    showMentionBadge,
    ariaLabel: parts.join(", "),
  };
}

/**
 * Применяет состояние индикаторов к плитке `.guild-item` в `#guild-rail`.
 * @param {any} tileEl
 * @param {ReturnType<typeof computeGuildRailIndicatorState>} state
 */
export function applyGuildRailIndicatorToDom(tileEl, state) {
  if (!tileEl || !state) return;
  const doc = tileEl.ownerDocument;
  tileEl.setAttribute("data-unread", state.unread ? "true" : "false");
  tileEl.setAttribute("data-muted", state.isMuted ? "true" : "false");
  tileEl.setAttribute("data-mention-count", String(state.mentionCount));
  tileEl.setAttribute("aria-label", state.ariaLabel);

  let badge = tileEl.querySelector(".guild-mention-badge");
  if (state.showMentionBadge) {
    if (!badge && doc) {
      badge = doc.createElement("span");
      badge.className = "guild-mention-badge";
      badge.setAttribute("aria-hidden", "true");
      tileEl.append(badge);
    }
    if (badge) {
      badge.textContent = state.badgeText;
      badge.removeAttribute("hidden");
    }
  } else if (badge) {
    badge.setAttribute("hidden", "");
  }
}

/**
 * Форматирует подпись истечения mute для UI.
 * @param {{ isMuted: boolean, isTemporarilyMuted: boolean, endTimeMs: number | null }} muteState
 * @returns {string}
 */
function formatMuteExpiryText(muteState) {
  if (!muteState.isMuted) return "";
  if (!muteState.isTemporarilyMuted || muteState.endTimeMs === null) return "Бессрочно";
  const date = new Date(muteState.endTimeMs);
  const hh = String(date.getUTCHours()).padStart(2, "0");
  const mm = String(date.getUTCMinutes()).padStart(2, "0");
  return `До ${hh}:${mm}`;
}

/**
 * Создаёт контроллер модалки настроек уведомлений сервера (`#guild-notif-modal` в `#guild-notifications-dialog`)
 * и контекстного меню уведомлений канала (`#channel-notif-menu`).
 * @param {{
 *   document?: any,
 *   invoke?: (cmd: string, args: any) => Promise<any>,
 *   guildId?: string,
 *   guildName?: string,
 *   memberCount?: number,
 *   defaultMessageNotifications?: number,
 *   guildEntry?: any,
 *   channels?: Array<{
 *     id: string,
 *     name?: string | null,
 *     type?: number,
 *     parent_id?: string | null,
 *     parentId?: string | null,
 *     position?: number,
 *   }>,
 *   now?: () => number,
 *   onAuthRequired?: () => void,
 * }} [options]
 */
export function createNotificationSettingsController(options = {}) {
  const doc =
    options.document ?? (typeof document !== "undefined" ? document : null);
  if (!doc) {
    throw new Error("createNotificationSettingsController requires a document instance");
  }

  const invoke =
    options.invoke ??
    (async () => ({ ok: true }));
  const now = options.now ?? (() => Date.now());

  let guildId = String(options.guildId ?? "1014537851418980372");
  let guildName = String(options.guildName ?? "Сервер");
  let memberCount = Number(options.memberCount ?? 0);
  const defaultMessageNotifications =
    options.defaultMessageNotifications === 0 || options.defaultMessageNotifications === 1
      ? options.defaultMessageNotifications
      : MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS;

  let guildEntry = mergeGuildSettingsUpdate(
    {},
    options.guildEntry ?? {
      guild_id: guildId,
      message_notifications: defaultMessageNotifications,
      suppress_everyone: false,
      suppress_roles: false,
      muted: false,
      mute_config: null,
      mobile_push: true,
      notify_highlights: 0,
      flags: 0,
      hide_muted_channels: false,
      mute_scheduled_events: false,
      version: 1,
      channel_overrides: {},
    },
    defaultMessageNotifications,
  );

  /** @type {Array<{ id: string, name: string, type: number, parent_id: string | null, position: number }>} */
  let channels = (options.channels ?? []).map((ch, idx) => ({
    id: String(ch.id),
    name: String(ch.name ?? ch.id),
    type: Number(ch.type ?? 0),
    parent_id:
      ch.parent_id && ch.parent_id !== "nil"
        ? String(ch.parent_id)
        : ch.parentId && ch.parentId !== "nil"
          ? String(ch.parentId)
          : null,
    position: Number(ch.position ?? idx),
  }));

  /** @type {"idle" | "ready" | "pending" | "error"} */
  let modalState = "ready";
  /** @type {any} */
  let modalOpener = null;
  /** @type {any} */
  let channelMenuOpener = null;
  let activeMenuChannelId = "";

  /**
   * @param {string} tag
   * @param {string} [className]
   * @param {string} [text]
   * @returns {any}
   */
  const make = (tag, className = "", text = "") => {
    const node = doc.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  };

  /**
   * @param {string} message
   */
  const announceStatus = (message) => {
    const statusEl = doc.getElementById("server-status");
    if (statusEl) statusEl.textContent = message;
  };

  /* ========================================================================
   * 1. Сборка DOM модалки #guild-notifications-dialog > #guild-notif-modal
   * ======================================================================== */
  const backdrop = doc.getElementById("guild-notifications-dialog") ?? make("div", "dialog-backdrop");
  backdrop.id = "guild-notifications-dialog";
  backdrop.className = "dialog-backdrop";
  backdrop.setAttribute("role", "presentation");
  backdrop.setAttribute("hidden", "");
  if (!backdrop.parentNode) doc.body.append(backdrop);
  backdrop.replaceChildren();

  const modal = make("div", "dialog notif-modal");
  modal.id = "guild-notif-modal";
  modal.setAttribute("role", "dialog");
  modal.setAttribute("aria-modal", "true");
  modal.setAttribute("aria-labelledby", "guild-notif-title");
  modal.setAttribute("aria-describedby", "guild-notif-subtitle");
  modal.setAttribute("data-state", "ready");
  modal.setAttribute("data-guild-id", guildId);
  backdrop.append(modal);

  // 1.1 Шапка модалки
  const header = make("header", "notif-modal-header");
  const titleGroup = make("div", "notif-modal-title-group");
  const titleEl = make("h2", "dialog-title", "Параметры уведомлений");
  titleEl.id = "guild-notif-title";
  const subtitleEl = make("p", "dialog-hint", guildName);
  subtitleEl.id = "guild-notif-subtitle";
  titleGroup.append(titleEl, subtitleEl);

  const closeHeaderBtn = make("button", "icon-btn dialog-close");
  closeHeaderBtn.setAttribute("type", "button");
  closeHeaderBtn.setAttribute("data-action", "guild-notif-close");
  closeHeaderBtn.setAttribute("aria-label", "Закрыть");
  const closeGlyph = make("span", "", "✕");
  closeGlyph.setAttribute("aria-hidden", "true");
  closeHeaderBtn.append(closeGlyph);
  header.append(titleGroup, closeHeaderBtn);
  modal.append(header);

  // 1.2 Секция приглушения сервера (mute + 6 окон длительности)
  const muteSection = make("section", "notif-section");
  muteSection.setAttribute("role", "group");
  muteSection.setAttribute("aria-labelledby", "guild-notif-mute-heading");
  const muteHeading = make("h3", "visually-hidden", "Приглушение сервера");
  muteHeading.id = "guild-notif-mute-heading";

  const muteLabel = make("label", "toggle");
  muteLabel.setAttribute("for", "guild-notif-mute-switch");
  const muteLabelText = make("span", "toggle-label");
  muteLabelText.append(
    make("span", "", "Заглушить сервер"),
    make("span", "toggle-hint", "Отключает звуковые и всплывающие уведомления со всего сервера"),
  );
  const muteSwitch = make("input", "toggle-input");
  muteSwitch.id = "guild-notif-mute-switch";
  muteSwitch.setAttribute("type", "checkbox");
  muteSwitch.setAttribute("role", "switch");
  muteSwitch.setAttribute("data-setting", "muted");
  muteSwitch.setAttribute("aria-checked", "false");
  const muteTrack = make("span", "toggle-track");
  muteTrack.setAttribute("aria-hidden", "true");
  muteTrack.append(make("span", "toggle-thumb"));
  muteLabel.append(muteLabelText, muteSwitch, muteTrack);

  const muteWindowRow = make("div", "settings-field notif-mute-window");
  muteWindowRow.id = "guild-notif-mute-window-row";
  muteWindowRow.setAttribute("hidden", "");
  const muteWindowLabel = make("label", "settings-field-label", "Длительность заглушения");
  muteWindowLabel.id = "guild-notif-window-label";
  muteWindowLabel.setAttribute("for", "guild-notif-window-select");
  const muteWindowSelect = make("select", "settings-select");
  muteWindowSelect.id = "guild-notif-window-select";
  muteWindowSelect.setAttribute("data-setting", "selected_time_window");
  muteWindowSelect.setAttribute("aria-labelledby", "guild-notif-window-label");
  for (const preset of MUTE_TIMER_OPTIONS) {
    const opt = make("option", "", preset.label);
    opt.value = String(preset.selected_time_window);
    opt.setAttribute("value", String(preset.selected_time_window));
    muteWindowSelect.append(opt);
  }
  const muteExpirySpan = make("span", "settings-slider-value", "Бессрочно");
  muteExpirySpan.id = "guild-notif-mute-expiry";
  muteExpirySpan.setAttribute("data-role", "mute-expiry-text");
  muteWindowRow.append(muteWindowLabel, muteWindowSelect, muteExpirySpan);
  muteSection.append(muteHeading, muteLabel, muteWindowRow);
  modal.append(muteSection);

  // 1.3 Радиогруппа базового уровня уведомлений сервера (.notif-level-list)
  const levelSection = make("section", "notif-section");
  levelSection.setAttribute("role", "radiogroup");
  levelSection.setAttribute("aria-labelledby", "guild-notif-level-heading");
  const levelHeading = make("h3", "notif-section-title", "Настройки уведомлений сервера");
  levelHeading.id = "guild-notif-level-heading";
  const levelList = make("div", "notif-level-list");

  const LEVEL_CARDS_SPEC = [
    {
      level: 0,
      title: "Все сообщения",
      desc: "Уведомления о каждом сообщении в незаглушённых каналах",
    },
    {
      level: 1,
      title: "Только @упоминания",
      desc: "Уведомления только при упоминании вас или вашей роли",
    },
    {
      level: 2,
      title: "Ничего",
      desc: "Никаких звуковых или всплывающих уведомлений",
    },
  ];

  /** @type {any[]} */
  const levelCardButtons = [];
  for (const item of LEVEL_CARDS_SPEC) {
    const cardBtn = make("button", "notif-level-card");
    cardBtn.setAttribute("type", "button");
    cardBtn.setAttribute("role", "radio");
    cardBtn.setAttribute("data-action", "modal-guild-level");
    cardBtn.setAttribute("data-level", String(item.level));
    cardBtn.setAttribute("aria-checked", "false");
    cardBtn.setAttribute("tabindex", "-1");
    const radioInd = make("span", "notif-radio-indicator");
    radioInd.setAttribute("aria-hidden", "true");
    const bodySpan = make("span", "notif-level-body");
    bodySpan.append(
      make("span", "notif-level-title", item.title),
      make("span", "notif-level-desc", item.desc),
    );
    cardBtn.append(radioInd, bodySpan);
    levelList.append(cardBtn);
    levelCardButtons.push(cardBtn);
  }

  const largeWarningBanner = make(
    "p",
    "notif-large-guild-banner",
    "На этом сервере много участников — включение «Всех сообщений» может привести к частому потоку уведомлений.",
  );
  largeWarningBanner.id = "guild-notif-large-warning";
  largeWarningBanner.setAttribute("role", "note");
  largeWarningBanner.setAttribute("hidden", "");
  levelSection.append(levelHeading, levelList, largeWarningBanner);
  modal.append(levelSection);

  // 1.4 Тумблеры подавления упоминаний и событий
  const suppressSection = make("section", "notif-section");
  suppressSection.setAttribute("role", "group");
  suppressSection.setAttribute("aria-labelledby", "guild-notif-suppress-heading");
  const suppressHeading = make("h3", "notif-section-title", "Подавление упоминаний и события");
  suppressHeading.id = "guild-notif-suppress-heading";
  suppressSection.append(suppressHeading);

  const SWITCH_SPECS = [
    {
      id: "guild-notif-suppress-everyone",
      setting: "suppress_everyone",
      title: "Подавлять @everyone и @here",
      hint: "Отключает уведомления и бейджи от упоминаний @everyone и @here",
    },
    {
      id: "guild-notif-suppress-roles",
      setting: "suppress_roles",
      title: "Подавлять все @роли",
      hint: "Отключает уведомления и бейджи от упоминаний любых ролей",
    },
    {
      id: "guild-notif-highlights",
      setting: "notify_highlights",
      title: "Уведомления о хайлайтах",
      hint: "Сводки активных обсуждений с участием друзей на сервере",
    },
    {
      id: "guild-notif-scheduled-events",
      setting: "mute_scheduled_events",
      title: "Заглушить новые события",
      hint: "Отключает уведомления о старте запланированных мероприятий",
    },
    {
      id: "guild-notif-mobile-push",
      setting: "mobile_push",
      title: "Мобильные push-уведомления",
      hint: "Отправлять уведомления на мобильные устройства, когда вы не за ПК",
    },
  ];

  /** @type {Record<string, any>} */
  const switchInputs = {};
  for (const sw of SWITCH_SPECS) {
    const lbl = make("label", "toggle");
    lbl.setAttribute("for", sw.id);
    const txt = make("span", "toggle-label");
    txt.append(make("span", "", sw.title), make("span", "toggle-hint", sw.hint));
    const inp = make("input", "toggle-input");
    inp.id = sw.id;
    inp.setAttribute("type", "checkbox");
    inp.setAttribute("role", "switch");
    inp.setAttribute("data-setting", sw.setting);
    inp.setAttribute("aria-checked", "false");
    const trk = make("span", "toggle-track");
    trk.setAttribute("aria-hidden", "true");
    trk.append(make("span", "toggle-thumb"));
    lbl.append(txt, inp, trk);
    suppressSection.append(lbl);
    switchInputs[sw.setting] = inp;
  }
  modal.append(suppressSection);

  // 1.5 Таблица переопределений каналов (.notif-override-table)
  const overridesSection = make("section", "notif-section");
  overridesSection.setAttribute("role", "group");
  overridesSection.setAttribute("aria-labelledby", "guild-notif-overrides-heading");
  const overridesHeading = make(
    "h3",
    "notif-section-title",
    "Переопределения уведомлений каналов",
  );
  overridesHeading.id = "guild-notif-overrides-heading";

  const overrideToolbar = make("div", "notif-override-toolbar");
  const overrideSelectLabel = make(
    "label",
    "visually-hidden",
    "Выберите канал или категорию для переопределения",
  );
  overrideSelectLabel.setAttribute("for", "notif-override-channel-select");
  const overrideChannelSelect = make("select", "settings-select");
  overrideChannelSelect.id = "notif-override-channel-select";
  overrideChannelSelect.setAttribute(
    "aria-label",
    "Выберите канал или категорию для переопределения",
  );
  const overrideAddBtn = make("button", "btn btn-secondary", "Добавить");
  overrideAddBtn.setAttribute("type", "button");
  overrideAddBtn.setAttribute("data-action", "notif-override-add");
  overrideToolbar.append(overrideSelectLabel, overrideChannelSelect, overrideAddBtn);

  const tableWrap = make("div", "notif-override-table-wrap");
  const overrideTable = make("table", "notif-override-table");
  overrideTable.setAttribute("aria-labelledby", "guild-notif-overrides-heading");
  const thead = make("thead");
  const headRow = make("tr");
  const COLUMNS = [
    { text: "Канал или категория", cls: "col-channel" },
    { text: "Все", cls: "col-radio" },
    { text: "Упоминания", cls: "col-radio" },
    { text: "Ничего", cls: "col-radio" },
    { text: "Mute", cls: "col-mute" },
  ];
  for (const col of COLUMNS) {
    const th = make("th", col.cls, col.text);
    th.setAttribute("scope", "col");
    headRow.append(th);
  }
  thead.append(headRow);
  const tbody = make("tbody");
  tbody.id = "notif-override-tbody";
  overrideTable.append(thead, tbody);

  const overrideEmptyHint = make(
    "p",
    "dialog-hint notif-override-empty",
    "Нет переопределённых каналов — для всех каналов действуют правила сервера.",
  );
  overrideEmptyHint.id = "notif-override-empty";
  overrideEmptyHint.setAttribute("hidden", "");
  tableWrap.append(overrideTable, overrideEmptyHint);
  overridesSection.append(overridesHeading, overrideToolbar, tableWrap);
  modal.append(overridesSection);

  // 1.6 Ошибка сохранения и подвал диалога
  const errorAlert = make("p", "dialog-error");
  errorAlert.id = "guild-notif-error";
  errorAlert.setAttribute("role", "alert");
  errorAlert.setAttribute("hidden", "");
  const footer = make("footer", "dialog-actions");
  const doneBtn = make("button", "btn btn-primary", "Готово");
  doneBtn.setAttribute("type", "button");
  doneBtn.setAttribute("data-action", "guild-notif-close");
  footer.append(doneBtn);
  modal.append(errorAlert, footer);

  /* ========================================================================
   * 2. Сборка DOM контекстного меню канала #channel-notif-menu
   * ======================================================================== */
  const channelMenu =
    doc.getElementById("channel-notif-menu") ?? make("div", "menu server-menu channel-notif-menu");
  channelMenu.id = "channel-notif-menu";
  channelMenu.className = "menu server-menu channel-notif-menu";
  channelMenu.setAttribute("role", "menu");
  channelMenu.setAttribute("aria-label", "Уведомления канала");
  channelMenu.setAttribute("data-channel-id", "");
  channelMenu.setAttribute("hidden", "");
  if (!channelMenu.parentNode) doc.body.append(channelMenu);
  channelMenu.replaceChildren();

  const chUnmuteBtn = make("button", "menu-item");
  chUnmuteBtn.setAttribute("type", "button");
  chUnmuteBtn.setAttribute("role", "menuitem");
  chUnmuteBtn.setAttribute("data-action", "channel-unmute");
  chUnmuteBtn.setAttribute("tabindex", "-1");
  chUnmuteBtn.setAttribute("hidden", "");
  const chUnmuteText = make("span", "menu-item-text");
  const chUnmuteSubtitle = make("span", "menu-item-subtitle");
  chUnmuteSubtitle.setAttribute("data-role", "channel-mute-until");
  chUnmuteText.append(
    make("span", "menu-item-label", "Включить уведомления канала"),
    chUnmuteSubtitle,
  );
  chUnmuteBtn.append(chUnmuteText);
  channelMenu.append(chUnmuteBtn);

  const muteGroupHeading = make("div", "menu-group-heading", "Заглушить канал");
  muteGroupHeading.setAttribute("role", "presentation");
  channelMenu.append(muteGroupHeading);

  /** @type {any[]} */
  const channelMutePresetBtns = [];
  for (const preset of MUTE_TIMER_OPTIONS) {
    const btn = make("button", "menu-item");
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "menuitemradio");
    btn.setAttribute("data-action", "channel-mute-preset");
    btn.setAttribute("data-window", String(preset.selected_time_window));
    btn.setAttribute("aria-checked", "false");
    btn.setAttribute("tabindex", "-1");
    const txt = make("span", "menu-item-text");
    txt.append(make("span", "menu-item-label", preset.label));
    btn.append(txt);
    channelMenu.append(btn);
    channelMutePresetBtns.push(btn);
  }

  const menuSep = make("div", "menu-sep");
  menuSep.setAttribute("role", "separator");
  const levelGroupHeading = make("div", "menu-group-heading", "Уровень уведомлений");
  levelGroupHeading.setAttribute("role", "presentation");
  channelMenu.append(menuSep, levelGroupHeading);

  const CHANNEL_MENU_LEVELS = [
    { level: 3, label: "По умолчанию", hasInheritedSubtitle: true },
    { level: 0, label: "Все сообщения", hasInheritedSubtitle: false },
    { level: 1, label: "Только @упоминания", hasInheritedSubtitle: false },
    { level: 2, label: "Ничего", hasInheritedSubtitle: false },
  ];

  /** @type {any[]} */
  const channelLevelBtns = [];
  /** @type {any} */
  let inheritedSubtitleSpan = null;
  for (const item of CHANNEL_MENU_LEVELS) {
    const btn = make("button", item.hasInheritedSubtitle ? "menu-item has-subtitle" : "menu-item");
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "menuitemradio");
    btn.setAttribute("data-action", "channel-notif-level");
    btn.setAttribute("data-level", String(item.level));
    btn.setAttribute("aria-checked", "false");
    btn.setAttribute("tabindex", "-1");
    const txt = make("span", "menu-item-text");
    txt.append(make("span", "menu-item-label", item.label));
    if (item.hasInheritedSubtitle) {
      inheritedSubtitleSpan = make("span", "menu-item-subtitle", "Только @упоминания");
      inheritedSubtitleSpan.setAttribute("data-role", "channel-inherited-label");
      txt.append(inheritedSubtitleSpan);
    }
    const radioSpan = make("span", "menu-item-radio");
    radioSpan.setAttribute("aria-hidden", "true");
    btn.append(txt, radioSpan);
    channelMenu.append(btn);
    channelLevelBtns.push(btn);
  }

  /* ========================================================================
   * 3. Синхронизация состояния с DOM, таймеры истечения mute и 429 cooldown
   * ======================================================================== */

  /** @type {any[]} */
  const muteExpiryTimers = [];
  let rateLimitRemainingSec = 0;
  /** @type {any} */
  let rateLimitIntervalId = null;

  /**
   * Очищает все запланированные таймеры истечения временного mute.
   */
  function clearMuteExpiryTimers() {
    while (muteExpiryTimers.length > 0) {
      const id = muteExpiryTimers.pop();
      clearTimeout(id);
    }
  }

  /**
   * Планирует таймеры истечения (`max(0, endMs - nowMs)`) для активных временных mute
   * сервера и каналов (`NOTIFICATIONS-SPEC.md §1.3.2 п.2`).
   */
  function scheduleMuteExpiryTimers() {
    clearMuteExpiryTimers();
    const nowMs = now();
    const guildMuteState = evaluateMuteState(guildEntry, nowMs);
    if (guildMuteState.isTemporarilyMuted && guildMuteState.remainingMs !== null) {
      const delay = Math.max(0, guildMuteState.remainingMs);
      const timerId = setTimeout(() => {
        guildEntry = {
          ...guildEntry,
          muted: false,
          mute_config: null,
        };
        renderModal();
        if (channelMenu.getAttribute("hidden") === null && activeMenuChannelId) {
          renderChannelMenu(activeMenuChannelId);
        }
        announceStatus("Время заглушения сервера истекло");
      }, delay);
      muteExpiryTimers.push(timerId);
    }

    const overridesMap = normalizeChannelOverrides(guildEntry.channel_overrides);
    for (const [chId, ov] of Object.entries(overridesMap)) {
      const chMuteState = evaluateMuteState(ov, nowMs);
      if (chMuteState.isTemporarilyMuted && chMuteState.remainingMs !== null) {
        const delay = Math.max(0, chMuteState.remainingMs);
        const timerId = setTimeout(() => {
          const curMap = normalizeChannelOverrides(guildEntry.channel_overrides);
          if (curMap[chId]) {
            curMap[chId] = {
              ...curMap[chId],
              muted: false,
              mute_config: null,
            };
            guildEntry = {
              ...guildEntry,
              channel_overrides: curMap,
            };
          }
          renderModal();
          if (channelMenu.getAttribute("hidden") === null && activeMenuChannelId) {
            renderChannelMenu(activeMenuChannelId);
          }
          announceStatus("Время заглушения канала истекло");
        }, delay);
        muteExpiryTimers.push(timerId);
      }
    }
  }

  /**
   * Проверяет, является ли ошибка IPC/REST ответом `429 RateLimited`.
   * @param {unknown} err
   * @returns {boolean}
   */
  function isRateLimitedError(err) {
    if (!err || typeof err !== "object") return false;
    const bag = /** @type {Record<string, any>} */ (err);
    const status = Number(bag.status ?? bag.code ?? 0);
    if (status === 429) return true;
    const kind = String(bag.kind ?? bag.message ?? "");
    return /429|ratelimit/i.test(kind);
  }

  /**
   * Извлекает длительность ожидания `retry_after` в целых секундах из ошибки `429`.
   * @param {unknown} err
   * @returns {number}
   */
  function extractRetryAfterSeconds(err) {
    if (!err || typeof err !== "object") return 1;
    const bag = /** @type {Record<string, any>} */ (err);
    const rawMs = bag.retry_after_ms ?? bag.retryAfterMs;
    if (typeof rawMs === "number" && Number.isFinite(rawMs) && rawMs > 0) {
      return Math.max(1, Math.ceil(rawMs / 1000));
    }
    const rawSec = bag.retry_after ?? bag.retryAfter;
    if (typeof rawSec === "number" && Number.isFinite(rawSec) && rawSec > 0) {
      return Math.max(1, Math.ceil(rawSec));
    }
    return 1;
  }

  /**
   * Выставляет или снимает блокировку `disabled` / `aria-disabled` на интерактивных контролах модалки.
   * @param {boolean} disabled
   */
  function setModalControlsDisabled(disabled) {
    const controls = [
      muteSwitch,
      muteWindowSelect,
      ...levelCardButtons,
      ...Object.values(switchInputs),
      overrideChannelSelect,
      overrideAddBtn,
      ...Array.from(tbody.querySelectorAll("button")),
    ];
    for (const ctrl of controls) {
      if (!ctrl) continue;
      ctrl.disabled = disabled;
      if (disabled) {
        ctrl.setAttribute("disabled", "");
        ctrl.setAttribute("aria-disabled", "true");
      } else {
        ctrl.removeAttribute("disabled");
        ctrl.removeAttribute("aria-disabled");
      }
    }
  }

  /**
   * Останавливает посекундный таймер `429 retry_after` и разблокирует контролы.
   */
  function stopRateLimitCooldown() {
    if (rateLimitIntervalId !== null) {
      clearInterval(rateLimitIntervalId);
      rateLimitIntervalId = null;
    }
    rateLimitRemainingSec = 0;
    setModalControlsDisabled(false);
  }

  /**
   * Запускает посекундный обратный отсчёт `429 retry_after` (`NOTIFICATIONS-SPEC.md §4.1`).
   * @param {number} seconds
   */
  function startRateLimitCooldown(seconds) {
    stopRateLimitCooldown();
    const total = Math.max(1, Math.trunc(seconds));
    rateLimitRemainingSec = total;
    setModalControlsDisabled(true);
    setModalState("error", `Слишком много запросов. Повторите через ${rateLimitRemainingSec} с.`);
    rateLimitIntervalId = setInterval(() => {
      rateLimitRemainingSec = Math.max(0, rateLimitRemainingSec - 1);
      if (rateLimitRemainingSec > 0) {
        setModalState(
          "error",
          `Слишком много запросов. Повторите через ${rateLimitRemainingSec} с.`,
        );
      } else {
        stopRateLimitCooldown();
        setModalState("ready");
      }
    }, 1000);
  }

  /**
   * @param {"idle" | "ready" | "pending" | "error"} nextState
   * @param {string} [errMsg]
   */
  function setModalState(nextState, errMsg = "") {
    modalState = nextState;
    modal.setAttribute("data-state", nextState);
    if (nextState === "pending") {
      modal.setAttribute("aria-busy", "true");
    } else {
      modal.removeAttribute("aria-busy");
    }
    if (nextState === "error" && errMsg) {
      errorAlert.textContent = errMsg;
      errorAlert.removeAttribute("hidden");
    } else if (nextState === "ready" || nextState === "idle") {
      errorAlert.textContent = "";
      errorAlert.setAttribute("hidden", "");
    }
  }

  /**
   * Выполняет мутацию серверных настроек через IPC `update_guild_notification_settings`
   * без оптимистичного переключения `aria-checked` до ответа (`NOTIFICATIONS-SPEC.md §4.1`).
   * @param {Record<string, any>} patch
   * @returns {Promise<boolean>}
   */
  async function submitGuildPatch(patch) {
    if (modalState === "pending" || rateLimitRemainingSec > 0) return false;
    setModalState("pending");
    const isDm = !guildId || guildId === "@me";
    const endpoint = isDm ? "/users/@me/guilds/@me/settings" : "/users/@me/guilds/settings";
    const body = isDm ? { ...patch } : { guilds: { [guildId]: patch } };

    try {
      await invoke("update_guild_notification_settings", {
        guildId: isDm ? "@me" : guildId,
        endpoint,
        patch,
        settings: patch,
        body,
      });
      guildEntry = mergeGuildSettingsUpdate(
        guildEntry,
        { ...patch, version: Number(guildEntry.version ?? 0) + 1 },
        defaultMessageNotifications,
      );
      renderModal();
      setModalState("ready");
      announceStatus("Настройки уведомлений сервера обновлены");
      return true;
    } catch (err) {
      const status = err && typeof err === "object" ? Number(/** @type {any} */ (err).status ?? 0) : 0;
      if (status === 401 && options.onAuthRequired) {
        closeGuildModal();
        options.onAuthRequired();
        return false;
      }
      if (isRateLimitedError(err)) {
        renderModal();
        startRateLimitCooldown(extractRetryAfterSeconds(err));
        return false;
      }
      const msg =
        err instanceof Error && err.message
          ? err.message
          : "Не удалось сохранить настройки уведомлений";
      renderModal();
      setModalState("error", msg);
      return false;
    }
  }

  /**
   * Выполняет мутацию переопределения канала через IPC `update_channel_notification_override`
   * без оптимистичной мутации до ответа (`NOTIFICATIONS-SPEC.md §1.6.3, §4.1`).
   * @param {string} channelId
   * @param {Record<string, any>} overridePatch
   * @param {number} [focusRadioLevelAfter]
   * @returns {Promise<boolean>}
   */
  async function submitChannelOverridePatch(channelId, overridePatch, focusRadioLevelAfter) {
    if (!channelId || modalState === "pending" || rateLimitRemainingSec > 0) return false;
    setModalState("pending");
    const patch = {
      channel_overrides: {
        [channelId]: overridePatch,
      },
    };
    const isDm = !guildId || guildId === "@me";
    const endpoint = isDm ? "/users/@me/guilds/@me/settings" : "/users/@me/guilds/settings";
    const body = isDm ? patch : { guilds: { [guildId]: patch } };

    try {
      await invoke("update_channel_notification_override", {
        guildId: isDm ? "@me" : guildId,
        channelId,
        endpoint,
        override: overridePatch,
        patch,
        body,
      });
      const currentOverrides = normalizeChannelOverrides(guildEntry.channel_overrides);
      const prev = currentOverrides[channelId] ?? {
        channel_id: channelId,
        collapsed: false,
        message_notifications: MESSAGE_NOTIFICATION_LEVELS.NULL,
        mute_config: null,
        muted: false,
      };
      currentOverrides[channelId] = {
        ...prev,
        ...overridePatch,
        channel_id: channelId,
      };
      guildEntry = {
        ...guildEntry,
        version: Number(guildEntry.version ?? 0) + 1,
        channel_overrides: currentOverrides,
      };
      renderModal(focusRadioLevelAfter !== undefined ? { channelId, level: focusRadioLevelAfter } : undefined);
      if (channelMenu.getAttribute("hidden") === null && activeMenuChannelId === channelId) {
        renderChannelMenu(channelId);
      }
      setModalState("ready");
      announceStatus("Переопределение уведомлений канала обновлено");
      return true;
    } catch (err) {
      if (isRateLimitedError(err)) {
        renderModal();
        startRateLimitCooldown(extractRetryAfterSeconds(err));
        return false;
      }
      const msg =
        err instanceof Error && err.message
          ? err.message
          : "Не удалось сохранить настройки канала";
      renderModal();
      setModalState("error", msg);
      return false;
    }
  }

  /**
   * Полностью синхронизирует DOM модалки `#guild-notif-modal` с текущим `guildEntry`.
   * @param {{ channelId: string, level: number }} [restoreRadioFocus]
   */
  function renderModal(restoreRadioFocus) {
    const nowMs = now();
    subtitleEl.textContent = guildName;
    modal.setAttribute("data-guild-id", guildId);

    // 1. Секция mute гильдии
    const muteState = evaluateMuteState(guildEntry, nowMs);
    muteSwitch.checked = muteState.isMuted;
    muteSwitch.setAttribute("aria-checked", muteState.isMuted ? "true" : "false");
    if (muteState.isMuted) {
      muteWindowRow.removeAttribute("hidden");
      const winVal = muteState.selectedTimeWindow ?? -1;
      muteWindowSelect.value = String(winVal);
      muteExpirySpan.textContent = formatMuteExpiryText(muteState);
    } else {
      muteWindowRow.setAttribute("hidden", "");
      muteWindowSelect.value = "-1";
      muteExpirySpan.textContent = "Бессрочно";
    }

    // 2. Радиогруппа уровня гильдии (.notif-level-list)
    const activeGuildLevel = isExplicitLevel(guildEntry.message_notifications)
      ? guildEntry.message_notifications
      : defaultMessageNotifications;
    for (const btn of levelCardButtons) {
      const lvl = Number(btn.getAttribute("data-level"));
      const checked = lvl === activeGuildLevel;
      btn.setAttribute("aria-checked", checked ? "true" : "false");
      btn.setAttribute("tabindex", checked ? "0" : "-1");
    }

    // Баннер большого сервера (Dh5p5j: level === 0 && memberCount >= 2500)
    if (
      activeGuildLevel === MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES &&
      memberCount >= LARGE_GUILD_MEMBER_THRESHOLD
    ) {
      largeWarningBanner.removeAttribute("hidden");
    } else {
      largeWarningBanner.setAttribute("hidden", "");
    }

    // 3. Тумблеры подавления и доп. настроек
    const supEveryone = parseStrictBool(guildEntry.suppress_everyone, false);
    switchInputs.suppress_everyone.checked = supEveryone;
    switchInputs.suppress_everyone.setAttribute("aria-checked", supEveryone ? "true" : "false");

    const supRoles = parseStrictBool(guildEntry.suppress_roles, false);
    switchInputs.suppress_roles.checked = supRoles;
    switchInputs.suppress_roles.setAttribute("aria-checked", supRoles ? "true" : "false");

    // notify_highlights: 0 (NULL) и 2 (ENABLED) трактуются как включено, 1 (DISABLED) — как выключено
    const highlightsVal = Number(guildEntry.notify_highlights ?? 0);
    const highlightsOn = highlightsVal !== NOTIFY_HIGHLIGHTS.DISABLED;
    switchInputs.notify_highlights.checked = highlightsOn;
    switchInputs.notify_highlights.setAttribute("aria-checked", highlightsOn ? "true" : "false");

    const muteEvents = parseStrictBool(guildEntry.mute_scheduled_events, false);
    switchInputs.mute_scheduled_events.checked = muteEvents;
    switchInputs.mute_scheduled_events.setAttribute("aria-checked", muteEvents ? "true" : "false");

    const mobPush = parseStrictBool(guildEntry.mobile_push, true);
    switchInputs.mobile_push.checked = mobPush;
    switchInputs.mobile_push.setAttribute("aria-checked", mobPush ? "true" : "false");

    // 4. Селект добавления канала и таблица .notif-override-table
    const overridesMap = normalizeChannelOverrides(guildEntry.channel_overrides);
    const channelById = new Map(channels.map((ch) => [ch.id, ch]));

    overrideChannelSelect.replaceChildren();
    const placeholderOpt = make("option", "", "Выберите канал или категорию…");
    placeholderOpt.value = "";
    placeholderOpt.setAttribute("value", "");
    overrideChannelSelect.append(placeholderOpt);
    for (const ch of channels) {
      const opt = make("option", "", ch.name);
      opt.value = ch.id;
      opt.setAttribute("value", ch.id);
      overrideChannelSelect.append(opt);
    }

    tbody.replaceChildren();
    const visibleOverrides = Object.values(overridesMap).filter((ov) =>
      hasVisibleNotificationOverride(ov, nowMs),
    );

    if (visibleOverrides.length === 0) {
      overrideEmptyHint.removeAttribute("hidden");
    } else {
      overrideEmptyHint.setAttribute("hidden", "");
    }

    for (const ov of visibleOverrides) {
      const chId = ov.channel_id;
      const chInfo = channelById.get(chId);
      const chName = chInfo?.name ?? chId;
      const parentInfo = chInfo?.parent_id ? channelById.get(chInfo.parent_id) : null;
      const catName = parentInfo?.name ?? "";

      const tr = make("tr", "notif-override-row");
      tr.setAttribute("data-channel-id", chId);

      const th = make("th", "notif-override-cell-channel");
      th.setAttribute("scope", "row");
      const iconSpan = make("span", "channel-icon", chInfo?.type === 4 ? "▾" : "#");
      iconSpan.setAttribute("aria-hidden", "true");
      const metaSpan = make("span", "notif-override-channel-meta");
      metaSpan.append(make("span", "notif-override-channel-name", chName));
      if (catName) {
        metaSpan.append(make("span", "notif-override-category-name", catName));
      }
      th.append(iconSpan, metaSpan);
      tr.append(th);

      // 3 радиокнопки уровня (0, 1, 2) с roving tabindex в пределах строки
      const ovLevel = ov.message_notifications;
      const activeRowRadioLevel = isExplicitLevel(ovLevel) ? ovLevel : 0;
      const rowRadioLabels = ["Все сообщения", "Только @упоминания", "Ничего"];
      /** @type {any[]} */
      const rowRadios = [];

      for (let lvl = 0; lvl <= 2; lvl += 1) {
        const td = make("td", "notif-override-cell-radio");
        const radioBtn = make("button", "notif-cell-radio");
        const isChecked = ovLevel === lvl;
        const isTabbable = lvl === activeRowRadioLevel;
        radioBtn.setAttribute("type", "button");
        radioBtn.setAttribute("role", "radio");
        radioBtn.setAttribute("data-action", "override-set-level");
        radioBtn.setAttribute("data-channel-id", chId);
        radioBtn.setAttribute("data-level", String(lvl));
        radioBtn.setAttribute("aria-checked", isChecked ? "true" : "false");
        radioBtn.setAttribute("tabindex", isTabbable ? "0" : "-1");
        radioBtn.setAttribute("aria-label", `${chName}: ${rowRadioLabels[lvl]}`);

        radioBtn.addEventListener("click", () => {
          const curMuted = evaluateMuteState(ov, now()).isMuted;
          void submitChannelOverridePatch(
            chId,
            { message_notifications: lvl, muted: curMuted },
            lvl,
          );
        });

        radioBtn.addEventListener("keydown", (/** @type {any} */ event) => {
          const key = event.key;
          if (
            key === "ArrowRight" ||
            key === "ArrowDown" ||
            key === "ArrowLeft" ||
            key === "ArrowUp"
          ) {
            event.preventDefault();
            event.stopPropagation();
            const delta = key === "ArrowRight" || key === "ArrowDown" ? 1 : -1;
            const nextLvl = (lvl + delta + 3) % 3;
            for (let i = 0; i < rowRadios.length; i += 1) {
              rowRadios[i].setAttribute("tabindex", i === nextLvl ? "0" : "-1");
            }
            rowRadios[nextLvl]?.focus();
            const curMuted = evaluateMuteState(ov, now()).isMuted;
            void submitChannelOverridePatch(
              chId,
              { message_notifications: nextLvl, muted: curMuted },
              nextLvl,
            );
          } else if (key === " " || key === "Enter") {
            event.preventDefault();
            event.stopPropagation();
            const curMuted = evaluateMuteState(ov, now()).isMuted;
            void submitChannelOverridePatch(
              chId,
              { message_notifications: lvl, muted: curMuted },
              lvl,
            );
          }
        });

        radioBtn.addEventListener("keydown", handleModalKeydown);

        td.append(radioBtn);
        tr.append(td);
        rowRadios.push(radioBtn);
      }

      // Колонка Mute + кнопка сброса override
      const muteTd = make("td", "notif-override-cell-mute");
      const chMuted = evaluateMuteState(ov, nowMs).isMuted;
      const muteChk = make("button", "notif-cell-checkbox");
      muteChk.setAttribute("type", "button");
      muteChk.setAttribute("role", "checkbox");
      muteChk.setAttribute("data-action", "override-toggle-mute");
      muteChk.setAttribute("data-channel-id", chId);
      muteChk.setAttribute("aria-checked", chMuted ? "true" : "false");
      muteChk.setAttribute("tabindex", "0");
      muteChk.setAttribute("aria-label", `Заглушить канал ${chName}`);

      const toggleChannelMute = () => {
        const nextMuted = !evaluateMuteState(ov, now()).isMuted;
        const curLvl = ov.message_notifications ?? MESSAGE_NOTIFICATION_LEVELS.NULL;
        void submitChannelOverridePatch(chId, {
          message_notifications: curLvl,
          muted: nextMuted,
          mute_config: nextMuted ? buildMuteConfig(-1, now()) : null,
        });
      };

      muteChk.addEventListener("click", toggleChannelMute);
      muteChk.addEventListener("keydown", (/** @type {any} */ event) => {
        if (event.key === " " || event.key === "Enter") {
          event.preventDefault();
          event.stopPropagation();
          toggleChannelMute();
          return;
        }
        handleModalKeydown(event);
      });

      const removeBtn = make("button", "icon-btn notif-override-remove");
      removeBtn.setAttribute("type", "button");
      removeBtn.setAttribute("data-action", "override-remove");
      removeBtn.setAttribute("data-channel-id", chId);
      removeBtn.setAttribute("tabindex", "0");
      removeBtn.setAttribute("aria-label", `Сбросить настройки канала ${chName}`);
      removeBtn.setAttribute("title", "Сбросить переопределение");
      const removeGlyph = make("span", "", "✕");
      removeGlyph.setAttribute("aria-hidden", "true");
      removeBtn.append(removeGlyph);

      removeBtn.addEventListener("click", () => {
        const cleanedFlags = clearUnreadFlags(Number(ov.flags ?? 0), CHANNEL_UNREAD_MASK);
        const resetPatch = {
          message_notifications: MESSAGE_NOTIFICATION_LEVELS.NULL,
          muted: false,
          flags: cleanedFlags,
        };
        void submitChannelOverridePatch(chId, resetPatch);
      });
      removeBtn.addEventListener("keydown", handleModalKeydown);

      muteTd.append(muteChk, removeBtn);
      tr.append(muteTd);
      tbody.append(tr);

      if (restoreRadioFocus && restoreRadioFocus.channelId === chId) {
        const targetRadio = rowRadios[restoreRadioFocus.level];
        if (targetRadio) targetRadio.focus();
      }
    }

    scheduleMuteExpiryTimers();
  }

  /* ========================================================================
   * 4. Обработчики событий модалки #guild-notif-modal (включая focus trap)
   * ======================================================================== */

  /**
   * Проверяет, скрыт ли элемент (или один из его предков внутри модалки) атрибутом `hidden`.
   * @param {any} el
   * @returns {boolean}
   */
  function isElementHiddenInModal(el) {
    /** @type {any} */
    let cur = el;
    while (cur && cur !== backdrop) {
      if (typeof cur.getAttribute === "function" && cur.getAttribute("hidden") !== null) {
        return true;
      }
      cur = cur.parentNode ?? null;
    }
    return false;
  }

  /**
   * Возвращает упорядоченный список доступных для `Tab`-обхода элементов внутри `#guild-notif-modal`.
   * @returns {any[]}
   */
  function getModalFocusableElements() {
    return Array.from(modal.querySelectorAll("button, input, select, textarea")).filter(
      (/** @type {any} */ el) => {
        if (isElementHiddenInModal(el)) return false;
        if (el.disabled === true || el.getAttribute("disabled") !== null) return false;
        if (el.getAttribute("aria-disabled") === "true") return false;
        if (el.getAttribute("tabindex") === "-1") return false;
        return true;
      },
    );
  }

  /**
   * Обрабатывает `Escape` и циклический focus trap (`Tab` / `Shift+Tab`) внутри `#guild-notif-modal`
   * (`NOTIFICATIONS-SPEC.md §5.2 п.4`).
   * @param {any} event
   */
  function handleModalKeydown(event) {
    if (event.defaultPrevented || event.prevented) return;
    if (backdrop.getAttribute("hidden") !== null) return;
    if (event.key === "Escape") {
      if (modalState === "pending") return;
      event.preventDefault();
      event.stopPropagation();
      closeGuildModal();
      return;
    }
    if (event.key === "Tab") {
      const focusables = getModalFocusableElements();
      if (focusables.length === 0) return;
      event.preventDefault();
      event.stopPropagation();
      const active = doc.activeElement;
      const currentIdx = active ? focusables.indexOf(active) : -1;
      const isShift = Boolean(event.shiftKey);
      const nextIdx =
        currentIdx < 0
          ? isShift
            ? focusables.length - 1
            : 0
          : (currentIdx + (isShift ? -1 : 1) + focusables.length) % focusables.length;
      focusables[nextIdx]?.focus();
    }
  }

  // Закрытие модалки
  const handleCloseClick = () => {
    if (modalState === "pending") return;
    closeGuildModal();
  };
  closeHeaderBtn.addEventListener("click", handleCloseClick);
  doneBtn.addEventListener("click", handleCloseClick);

  // Тумблер mute гильдии
  const toggleGuildMute = () => {
    const curMuted = evaluateMuteState(guildEntry, now()).isMuted;
    const nextMuted = !curMuted;
    // Восстанавливаем визуальное состояние input до ответа IPC (никаких оптимистичных мутаций!)
    muteSwitch.checked = curMuted;
    void submitGuildPatch(
      nextMuted ? { muted: true } : { muted: false, mute_config: null },
    );
  };
  muteSwitch.addEventListener("click", toggleGuildMute);
  muteSwitch.addEventListener("keydown", (/** @type {any} */ event) => {
    if (event.key === " ") {
      event.preventDefault();
      event.stopPropagation();
      toggleGuildMute();
      return;
    }
    handleModalKeydown(event);
  });

  // Селект окна длительности mute гильдии
  muteWindowSelect.addEventListener("change", () => {
    const winSec = Number(muteWindowSelect.value);
    const cfg = buildMuteConfig(winSec, now());
    void submitGuildPatch({
      muted: true,
      mute_config: cfg,
    });
  });

  // Радиогруппа уровня уведомлений гильдии (.notif-level-list)
  for (let idx = 0; idx < levelCardButtons.length; idx += 1) {
    const btn = levelCardButtons[idx];
    const lvl = Number(btn.getAttribute("data-level"));
    btn.addEventListener("click", () => {
      void submitGuildPatch({ message_notifications: lvl });
    });
    btn.addEventListener("keydown", (/** @type {any} */ event) => {
      const key = event.key;
      if (
        key === "ArrowDown" ||
        key === "ArrowRight" ||
        key === "ArrowUp" ||
        key === "ArrowLeft"
      ) {
        event.preventDefault();
        event.stopPropagation();
        const delta = key === "ArrowDown" || key === "ArrowRight" ? 1 : -1;
        const nextIdx = (idx + delta + levelCardButtons.length) % levelCardButtons.length;
        const nextBtn = levelCardButtons[nextIdx];
        const nextLvl = Number(nextBtn.getAttribute("data-level"));
        nextBtn.focus();
        void submitGuildPatch({ message_notifications: nextLvl });
      } else if (key === " " || key === "Enter") {
        event.preventDefault();
        event.stopPropagation();
        void submitGuildPatch({ message_notifications: lvl });
      } else {
        handleModalKeydown(event);
      }
    });
  }

  // Тумблеры подавления и доп. настроек
  for (const sw of SWITCH_SPECS) {
    const inp = switchInputs[sw.setting];
    const triggerSwitch = () => {
      if (sw.setting === "notify_highlights") {
        const curVal = Number(guildEntry.notify_highlights ?? 0);
        const curOn = curVal !== NOTIFY_HIGHLIGHTS.DISABLED;
        inp.checked = curOn;
        const nextVal = curOn ? NOTIFY_HIGHLIGHTS.DISABLED : NOTIFY_HIGHLIGHTS.ENABLED;
        void submitGuildPatch({ notify_highlights: nextVal });
      } else {
        const defaultBool = sw.setting === "mobile_push";
        const curBool = parseStrictBool(guildEntry[sw.setting], defaultBool);
        inp.checked = curBool;
        void submitGuildPatch({ [sw.setting]: !curBool });
      }
    };
    inp.addEventListener("click", triggerSwitch);
    inp.addEventListener("keydown", (/** @type {any} */ event) => {
      if (event.key === " ") {
        event.preventDefault();
        event.stopPropagation();
        triggerSwitch();
        return;
      }
      handleModalKeydown(event);
    });
  }

  // Добавление канала в таблицу переопределений
  overrideAddBtn.addEventListener("click", () => {
    const selectedChannelId = String(overrideChannelSelect.value ?? "").trim();
    if (!selectedChannelId) return;
    const chObj = channels.find((c) => c.id === selectedChannelId) ?? {
      id: selectedChannelId,
      parent_id: null,
    };
    const effective = resolveEffectiveNotifications({
      guildEntry,
      channel: chObj,
      defaultMessageNotifications,
      nowMs: now(),
    });
    const overridesMap = normalizeChannelOverrides(guildEntry.channel_overrides);
    const existingFlags = Number(overridesMap[selectedChannelId]?.flags ?? 0);
    const unreadBit =
      effective.resolvedMessageNotifications === MESSAGE_NOTIFICATION_LEVELS.ALL_MESSAGES
        ? CHANNEL_NOTIFICATION_FLAGS.UNREADS_ALL_MESSAGES
        : CHANNEL_NOTIFICATION_FLAGS.UNREADS_ONLY_MENTIONS;
    const nextFlags = setUnreadFlag(existingFlags, unreadBit, CHANNEL_UNREAD_MASK);
    void submitChannelOverridePatch(selectedChannelId, {
      muted: false,
      message_notifications: effective.resolvedMessageNotifications,
      flags: nextFlags,
    });
  });

  // Escape и focus trap на модалке и её статических контролах
  modal.addEventListener("keydown", handleModalKeydown);
  for (const staticCtrl of [
    closeHeaderBtn,
    muteWindowSelect,
    overrideChannelSelect,
    overrideAddBtn,
    doneBtn,
  ]) {
    staticCtrl.addEventListener("keydown", handleModalKeydown);
  }

  /* ========================================================================
   * 5. Контекстное меню канала #channel-notif-menu (roving tabindex + Escape)
   * ======================================================================== */

  /**
   * Возвращает список всех видимых интерактивных пунктов в `#channel-notif-menu`.
   * @returns {any[]}
   */
  function getEnabledChannelMenuItems() {
    return Array.from(
      channelMenu.querySelectorAll('[role="menuitem"], [role="menuitemradio"]'),
    ).filter(
      (/** @type {any} */ item) =>
        item.getAttribute("hidden") === null && item.getAttribute("aria-disabled") !== "true",
    );
  }

  /**
   * Выставляет roving `tabindex="0"` на указанный индекс в `#channel-notif-menu`.
   * @param {number} targetIndex
   * @param {boolean} [shouldFocus]
   */
  function setChannelMenuRovingFocus(targetIndex, shouldFocus = true) {
    const items = getEnabledChannelMenuItems();
    if (items.length === 0) return;
    const clamped = ((targetIndex % items.length) + items.length) % items.length;
    for (let i = 0; i < items.length; i += 1) {
      items[i].setAttribute("tabindex", i === clamped ? "0" : "-1");
    }
    if (shouldFocus) {
      items[clamped]?.focus();
    }
  }

  /**
   * Синхронизирует состояние пунктов `#channel-notif-menu` для канала `channelId`.
   * @param {string} channelId
   */
  function renderChannelMenu(channelId) {
    const nowMs = now();
    channelMenu.setAttribute("data-channel-id", channelId);
    const overridesMap = normalizeChannelOverrides(guildEntry.channel_overrides);
    const ov = overridesMap[channelId] ?? null;
    const muteState = evaluateMuteState(ov, nowMs);

    if (muteState.isMuted) {
      chUnmuteBtn.removeAttribute("hidden");
      chUnmuteSubtitle.textContent = formatMuteExpiryText(muteState);
    } else {
      chUnmuteBtn.setAttribute("hidden", "");
      chUnmuteSubtitle.textContent = "";
    }

    for (const btn of channelMutePresetBtns) {
      const win = Number(btn.getAttribute("data-window"));
      const isChecked = muteState.isMuted && muteState.selectedTimeWindow === win;
      btn.setAttribute("aria-checked", isChecked ? "true" : "false");
    }

    const chObj = channels.find((c) => c.id === channelId) ?? { id: channelId, parent_id: null };
    const inheritedResolved = resolveEffectiveNotifications({
      guildEntry,
      channel: { id: "__inherited__", parent_id: chObj.parent_id },
      defaultMessageNotifications,
      nowMs,
    });
    if (inheritedSubtitleSpan) {
      const lbl =
        MESSAGE_NOTIFICATION_LABELS[
          /** @type {0 | 1 | 2} */ (inheritedResolved.resolvedMessageNotifications)
        ] ?? "Только @упоминания";
      inheritedSubtitleSpan.textContent = lbl;
    }

    const currentLevel = ov?.message_notifications ?? MESSAGE_NOTIFICATION_LEVELS.NULL;
    for (const btn of channelLevelBtns) {
      const lvl = Number(btn.getAttribute("data-level"));
      btn.setAttribute("aria-checked", lvl === currentLevel ? "true" : "false");
    }

    scheduleMuteExpiryTimers();
  }

  // Действия внутри #channel-notif-menu
  chUnmuteBtn.addEventListener("click", () => {
    if (!activeMenuChannelId) return;
    const overridesMap = normalizeChannelOverrides(guildEntry.channel_overrides);
    const curLvl =
      overridesMap[activeMenuChannelId]?.message_notifications ?? MESSAGE_NOTIFICATION_LEVELS.NULL;
    void submitChannelOverridePatch(activeMenuChannelId, {
      message_notifications: curLvl,
      muted: false,
      mute_config: null,
    });
  });

  for (const btn of channelMutePresetBtns) {
    btn.addEventListener("click", () => {
      if (!activeMenuChannelId) return;
      const win = Number(btn.getAttribute("data-window"));
      const overridesMap = normalizeChannelOverrides(guildEntry.channel_overrides);
      const curLvl =
        overridesMap[activeMenuChannelId]?.message_notifications ?? MESSAGE_NOTIFICATION_LEVELS.NULL;
      const cfg = buildMuteConfig(win, now());
      void submitChannelOverridePatch(activeMenuChannelId, {
        message_notifications: curLvl,
        muted: true,
        mute_config: cfg,
      });
    });
  }

  for (const btn of channelLevelBtns) {
    btn.addEventListener("click", () => {
      if (!activeMenuChannelId) return;
      const lvl = Number(btn.getAttribute("data-level"));
      const overridesMap = normalizeChannelOverrides(guildEntry.channel_overrides);
      const curMuted = evaluateMuteState(overridesMap[activeMenuChannelId], now()).isMuted;
      void submitChannelOverridePatch(activeMenuChannelId, {
        message_notifications: lvl,
        muted: curMuted,
      });
    });
  }

  /**
   * Обработчик клавиатуры для `#channel-notif-menu` и его пунктов (поскольку в шиме события не всплывают).
   * @param {any} event
   */
  const handleChannelMenuKeydown = (event) => {
    const key = event.key;
    const items = getEnabledChannelMenuItems();
    const currentIndex = items.indexOf(doc.activeElement);

    if (key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation?.();
      closeChannelMenu({ restoreFocus: true });
      return;
    }
    if (key === "ArrowDown") {
      event.preventDefault();
      event.stopPropagation();
      setChannelMenuRovingFocus(currentIndex < 0 ? 0 : currentIndex + 1, true);
      return;
    }
    if (key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      setChannelMenuRovingFocus(currentIndex <= 0 ? items.length - 1 : currentIndex - 1, true);
      return;
    }
    if (key === "Home") {
      event.preventDefault();
      event.stopPropagation();
      setChannelMenuRovingFocus(0, true);
      return;
    }
    if (key === "End") {
      event.preventDefault();
      event.stopPropagation();
      setChannelMenuRovingFocus(items.length - 1, true);
      return;
    }
    if (key === "Enter" || key === " ") {
      if (currentIndex >= 0 && items[currentIndex]) {
        event.preventDefault();
        event.stopPropagation();
        items[currentIndex].click();
      }
    }
  };

  channelMenu.addEventListener("keydown", handleChannelMenuKeydown);
  for (const item of [chUnmuteBtn, ...channelMutePresetBtns, ...channelLevelBtns]) {
    item.addEventListener("keydown", handleChannelMenuKeydown);
  }

  /* ========================================================================
   * 6. Публичный API контроллера
   * ======================================================================== */

  /**
   * Открывает модальное окно настроек уведомлений сервера `#guild-notif-modal`.
   * @param {{
   *   guildId?: string,
   *   guildName?: string,
   *   memberCount?: number,
   *   guildEntry?: any,
   *   channels?: any[],
   *   triggerElement?: any,
   * }} [openOpts]
   */
  function openGuildModal(openOpts = {}) {
    if (openOpts.guildId !== undefined) guildId = String(openOpts.guildId);
    if (openOpts.guildName !== undefined) guildName = String(openOpts.guildName);
    if (openOpts.memberCount !== undefined) memberCount = Number(openOpts.memberCount);
    if (openOpts.guildEntry !== undefined) {
      guildEntry = mergeGuildSettingsUpdate({}, openOpts.guildEntry, defaultMessageNotifications);
    }
    if (openOpts.channels !== undefined) {
      channels = openOpts.channels.map((ch, idx) => ({
        id: String(ch.id),
        name: String(ch.name ?? ch.id),
        type: Number(ch.type ?? 0),
        parent_id:
          ch.parent_id && ch.parent_id !== "nil"
            ? String(ch.parent_id)
            : ch.parentId && ch.parentId !== "nil"
              ? String(ch.parentId)
              : null,
        position: Number(ch.position ?? idx),
      }));
    }

    stopRateLimitCooldown();
    modalOpener = openOpts.triggerElement ?? doc.activeElement ?? null;
    closeChannelMenu({ restoreFocus: false });
    setModalState("ready");
    renderModal();
    backdrop.removeAttribute("hidden");
    muteSwitch.focus();
  }

  /**
   * Закрывает модальное окно `#guild-notif-modal` и возвращает фокус на `opener`.
   * @param {{ restoreFocus?: boolean }} [closeOpts]
   */
  function closeGuildModal(closeOpts = {}) {
    backdrop.setAttribute("hidden", "");
    stopRateLimitCooldown();
    if (channelMenu.getAttribute("hidden") !== null) {
      clearMuteExpiryTimers();
    }
    const shouldRestore = closeOpts.restoreFocus !== false;
    if (shouldRestore && modalOpener && typeof modalOpener.focus === "function") {
      modalOpener.focus();
    }
    modalOpener = null;
  }

  /**
   * Открывает поповер быстрых настроек канала `#channel-notif-menu`.
   * @param {{
   *   channelId: string,
   *   triggerElement?: any,
   * }} menuOpts
   */
  function openChannelMenu(menuOpts) {
    const chId = String(menuOpts?.channelId ?? "").trim();
    if (!chId) return;
    activeMenuChannelId = chId;
    channelMenuOpener = menuOpts.triggerElement ?? doc.activeElement ?? null;
    if (channelMenuOpener && typeof channelMenuOpener.setAttribute === "function") {
      channelMenuOpener.setAttribute("aria-expanded", "true");
    }
    renderChannelMenu(chId);
    channelMenu.removeAttribute("hidden");
    setChannelMenuRovingFocus(0, true);
  }

  /**
   * Закрывает поповер `#channel-notif-menu` и возвращает фокус на сохранённый `opener`.
   * @param {{ restoreFocus?: boolean }} [closeOpts]
   */
  function closeChannelMenu(closeOpts = {}) {
    channelMenu.setAttribute("hidden", "");
    if (backdrop.getAttribute("hidden") !== null) {
      clearMuteExpiryTimers();
    }
    if (channelMenuOpener && typeof channelMenuOpener.setAttribute === "function") {
      channelMenuOpener.setAttribute("aria-expanded", "false");
    }
    const shouldRestore = closeOpts.restoreFocus !== false;
    if (shouldRestore && channelMenuOpener && typeof channelMenuOpener.focus === "function") {
      channelMenuOpener.focus();
    }
    channelMenuOpener = null;
    activeMenuChannelId = "";
  }

  return {
    openGuildModal,
    closeGuildModal,
    openChannelMenu,
    closeChannelMenu,
    getState() {
      return {
        guildId,
        guildName,
        memberCount,
        modalState,
        guildEntry,
        activeMenuChannelId,
      };
    },
    destroy() {
      stopRateLimitCooldown();
      clearMuteExpiryTimers();
      backdrop.remove();
      channelMenu.remove();
    },
  };
}
