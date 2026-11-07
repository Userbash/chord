// @ts-check
/**
 * Композер (SPEC-P3-02 §4, CMP-01…CMP-15) + вложения и подарок 🎁 (RW-40, B2b-ATTACH-UI, STUB-12).
 *
 * Состояния: `empty` (пусто) → `typing` (есть текст) → `disabled` (нет прав/сети;
 * поле остаётся в DOM и фокусируемым, CMP-07). Лимит 2000 символов (K-10):
 * при превышении ввод обрезается, счётчик краснеет (CMP-13). Счётчик появляется
 * с 1800 символов (CMP-06). Отправки нет — спайк без сети, это видно в
 * `#composer-hint`/`.chat-notice`.
 */
import { UIError, asUIError } from "./errors.js";
import {
  createAttachmentsController,
  MAX_ATTACHMENT_COUNT,
  MAX_ATTACHMENT_SIZE_BYTES,
  MAX_FILE_SIZE_BYTES,
  SPOILER_PREFIX,
  ATTACHMENT_FLAG_IS_SPOILER,
  UPLOAD_ATTACHMENTS_COMMAND,
  ATTACHMENT_SHELF_TEXT,
  formatAttachmentSize,
  isSpoilerFilename,
  stripSpoilerPrefix,
  applySpoilerFilename,
  extractFilesFromTransferEvent,
} from "./attachments.js";

export {
  createAttachmentsController,
  MAX_ATTACHMENT_COUNT,
  MAX_ATTACHMENT_SIZE_BYTES,
  MAX_FILE_SIZE_BYTES,
  SPOILER_PREFIX,
  ATTACHMENT_FLAG_IS_SPOILER,
  UPLOAD_ATTACHMENTS_COMMAND,
  ATTACHMENT_SHELF_TEXT,
  formatAttachmentSize,
  isSpoilerFilename,
  stripSpoilerPrefix,
  applySpoilerFilename,
  extractFilesFromTransferEvent,
};

/** K-10: платформенный лимит текста сообщения (нужен контракт W39i). */
export const COMPOSER_LIMIT = 2000;
/** K-11: счётчик показывается, когда остаток ≤ 200 символов (`max/10`). */
export const COUNTER_REMAINING = 200;
/** CMP-06: счётчик появляется при length ≥ 1800 (2000 − 200). */
export const COUNTER_FROM = COMPOSER_LIMIT - COUNTER_REMAINING;

/** B2b / RE §1.5: базовый лимит на файл — 20 МиБ (`0x1400000` = 20971520, Math.max(0x1400000, tierLimit)). */
export const MAX_UPLOAD_FILE_BYTES = 20 * 1024 * 1024;
/** B2b / RE §1.5: максимум 10 вложений в сообщении (`n4=10`). */
export const MAX_ATTACHMENTS_PER_MESSAGE = 10;

/** Единая константа лимитов UI (AC-29). */
export const ATTACH_LIMITS = {
  maxFileSize: MAX_UPLOAD_FILE_BYTES,
  maxCount: MAX_ATTACHMENTS_PER_MESSAGE,
};

/** Канонические 64-битные маски прав доступа Discord API v9 (BigInt). */
export const DISCORD_PERM_CREATE_INSTANT_INVITE = 1n << 0n;
export const DISCORD_PERM_ADMINISTRATOR = 1n << 3n;
export const DISCORD_PERM_ADD_REACTIONS = 1n << 6n;
export const DISCORD_PERM_VIEW_CHANNEL = 1n << 10n;
export const DISCORD_PERM_SEND_MESSAGES = 1n << 11n;
export const DISCORD_PERM_ATTACH_FILES = 1n << 15n;
export const DISCORD_PERM_READ_MESSAGE_HISTORY = 1n << 16n;
export const DISCORD_PERM_CONNECT = 1n << 20n;
export const DISCORD_PERM_SPEAK = 1n << 21n;

const DISCORD_ALL_PERMISSIONS = (1n << 64n) - 1n;

const DEFAULT_BASE_PERMISSIONS =
  DISCORD_PERM_CREATE_INSTANT_INVITE |
  DISCORD_PERM_ADD_REACTIONS |
  DISCORD_PERM_VIEW_CHANNEL |
  DISCORD_PERM_SEND_MESSAGES |
  DISCORD_PERM_ATTACH_FILES |
  DISCORD_PERM_READ_MESSAGE_HISTORY |
  DISCORD_PERM_CONNECT |
  DISCORD_PERM_SPEAK;

const CHANNEL_SEND_DENIED_REASON = "У вас нет прав на отправку сообщений в этом канале";

const STRICT_READ_ONLY_NAME_RE =
  /(?:^|[-_\s#])(rules|правила|правила-discord|announcements|объявления|новости-проекта|новости-сервера|patch-notes|changelog)(?:$|[-_\s])/i;

const OVERWRITE_READ_ONLY_NAME_RE =
  /(?:^|[-_\s#])(rules|правила|announcements|объявления|новости|news|patch-notes|changelog|info|инфо)(?:$|[-_\s])/i;

/**
 * Разбирает 64-битную маску прав из BigInt, числа или десятичной строки.
 * @param {unknown} value
 * @returns {bigint}
 */
function parsePermissionBitfield(value) {
  if (typeof value === "bigint") return value >= 0n ? value : 0n;
  if (typeof value === "number") {
    return Number.isFinite(value) && value >= 0 ? BigInt(Math.trunc(value)) : 0n;
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (/^\d+$/.test(trimmed)) {
      try {
        return BigInt(trimmed);
      } catch {
        return 0n;
      }
    }
  }
  return 0n;
}

/**
 * Проверяет, совпадает ли имя канала со списком или регулярным выражением read-only каналов.
 * @param {string} channelName
 * @param {unknown} readOnlyNames
 * @param {boolean} hasOverwrites
 * @returns {boolean}
 */
function matchesReadOnlyChannelName(channelName, readOnlyNames, hasOverwrites) {
  const normalized = channelName.trim().replace(/^#+/, "").toLowerCase();
  if (!normalized) return false;

  if (readOnlyNames instanceof Set || Array.isArray(readOnlyNames)) {
    for (const entry of readOnlyNames) {
      if (entry instanceof RegExp && entry.test(normalized)) return true;
      if (typeof entry === "string") {
        const target = entry.trim().replace(/^#+/, "").toLowerCase();
        if (target && (normalized === target || normalized.includes(target))) {
          return true;
        }
      }
    }
  }

  if (STRICT_READ_ONLY_NAME_RE.test(normalized)) return true;
  if (hasOverwrites && OVERWRITE_READ_ONLY_NAME_RE.test(normalized)) return true;
  return false;
}

/**
 * Вычисляет эффективные права пользователя в канале по иерархии Discord:
 * `owner -> ADMINISTRATOR -> @everyone + memberRoles -> @everyone overwrite -> role overwrites -> member overwrite`,
 * а также определяет read-only каналы объявлений/правил и состояние композера.
 *
 * @param {{
 *   channel?: {
 *     id?: string | null,
 *     name?: string | null,
 *     type?: number | string | null,
 *     guild_id?: string | null,
 *     guildId?: string | null,
 *     owner_id?: string | null,
 *     ownerId?: string | null,
 *     permissions?: string | number | bigint | null,
 *     permission_overwrites?: Array<any> | null,
 *     overwrites?: Array<any> | null,
 *     canSend?: boolean | null,
 *     canSendMessages?: boolean | null,
 *     readOnly?: boolean | null,
 *     read_only?: boolean | null,
 *     permissionDeniedReason?: string | null,
 *   } | null,
 *   userId?: string | null,
 *   ownerId?: string | null,
 *   guildId?: string | null,
 *   guild_id?: string | null,
 *   isOwner?: boolean | null,
 *   permissions?: string | number | bigint | null,
 *   basePermissions?: string | number | bigint | null,
 *   guildRoles?: Array<{ id?: string, name?: string, permissions?: string | number | bigint, position?: number, isEveryone?: boolean, everyone?: boolean }> | null,
 *   memberRoles?: Array<string | { id?: string, permissions?: string | number | bigint }> | null,
 *   overwrites?: Array<{ id?: string, type?: number | string, allow?: string | number | bigint, deny?: string | number | bigint, isEveryone?: boolean, everyone?: boolean }> | null,
 *   permission_overwrites?: Array<{ id?: string, type?: number | string, allow?: string | number | bigint, deny?: string | number | bigint }> | null,
 *   readOnlyNames?: Array<string | RegExp> | Set<string> | null,
 * }} [options]
 * @returns {Readonly<{
 *   permissions: string,
 *   canView: boolean,
 *   canSend: boolean,
 *   canAttach: boolean,
 *   canReact: boolean,
 *   canConnect: boolean,
 *   canSpeak: boolean,
 *   placeholder: string,
 *   disabledReason: string,
 * }>}
 */
export function evaluateClientChannelPolicy(options = {}) {
  const opts = options && typeof options === "object" ? options : {};
  const channel = opts.channel && typeof opts.channel === "object" ? opts.channel : null;
  const rawName = typeof channel?.name === "string" ? channel.name.trim().replace(/^#+/, "") : "";
  const writablePlaceholder = rawName ? `Написать в #${rawName}` : "Написать сообщение…";

  const userId = typeof opts.userId === "string" ? opts.userId.trim() : "";
  const ownerId =
    typeof opts.ownerId === "string" && opts.ownerId.trim() !== ""
      ? opts.ownerId.trim()
      : typeof channel?.owner_id === "string" && channel.owner_id.trim() !== ""
        ? channel.owner_id.trim()
        : typeof channel?.ownerId === "string" && channel.ownerId.trim() !== ""
          ? channel.ownerId.trim()
          : "";

  const guildId =
    typeof opts.guildId === "string" && opts.guildId.trim() !== ""
      ? opts.guildId.trim()
      : typeof opts.guild_id === "string" && opts.guild_id.trim() !== ""
        ? opts.guild_id.trim()
        : typeof channel?.guild_id === "string" && channel.guild_id.trim() !== ""
          ? channel.guild_id.trim()
          : typeof channel?.guildId === "string" && channel.guildId.trim() !== ""
            ? channel.guildId.trim()
            : "";

  const isOwner =
    opts.isOwner === true || Boolean(userId !== "" && ownerId !== "" && userId === ownerId);

  if (isOwner) {
    return Object.freeze({
      permissions: String(DISCORD_ALL_PERMISSIONS),
      canView: true,
      canSend: true,
      canAttach: true,
      canReact: true,
      canConnect: true,
      canSpeak: true,
      placeholder: writablePlaceholder,
      disabledReason: "",
    });
  }

  const guildRoles = Array.isArray(opts.guildRoles) ? opts.guildRoles : [];
  const rawMemberRoles = Array.isArray(opts.memberRoles) ? opts.memberRoles : [];
  /** @type {Set<string>} */
  const memberRoleIds = new Set();
  let inlineMemberRolePerms = 0n;
  let hasInlineMemberRolePerms = false;

  for (const item of rawMemberRoles) {
    if (typeof item === "string" && item.trim() !== "") {
      memberRoleIds.add(item.trim());
    } else if (item && typeof item === "object") {
      if (typeof item.id === "string" && item.id.trim() !== "") {
        memberRoleIds.add(item.id.trim());
      }
      if (item.permissions !== undefined && item.permissions !== null) {
        inlineMemberRolePerms |= parsePermissionBitfield(item.permissions);
        hasInlineMemberRolePerms = true;
      }
    }
  }

  let everyoneRoleId = guildId;
  let basePerms = 0n;
  let hasExplicitBasePerms = false;
  let memberRolePerms = inlineMemberRolePerms;

  if (opts.basePermissions !== undefined && opts.basePermissions !== null) {
    basePerms |= parsePermissionBitfield(opts.basePermissions);
    hasExplicitBasePerms = true;
  }
  if (opts.permissions !== undefined && opts.permissions !== null) {
    basePerms |= parsePermissionBitfield(opts.permissions);
    hasExplicitBasePerms = true;
  }
  if (channel?.permissions !== undefined && channel?.permissions !== null) {
    basePerms |= parsePermissionBitfield(channel.permissions);
    hasExplicitBasePerms = true;
  }

  if (guildRoles.length > 0) {
    hasExplicitBasePerms = true;
    let matchedEveryoneRole = false;
    for (const role of guildRoles) {
      if (!role || typeof role !== "object") continue;
      const roleId = typeof role.id === "string" ? role.id.trim() : "";
      const roleName = typeof role.name === "string" ? role.name.trim().toLowerCase() : "";
      const isEveryone =
        role.isEveryone === true ||
        role.everyone === true ||
        (guildId !== "" && roleId === guildId) ||
        roleId === "@everyone" ||
        roleId === "everyone" ||
        roleId === "0" ||
        roleName === "@everyone" ||
        roleName === "everyone";

      if (isEveryone) {
        matchedEveryoneRole = true;
        if (!everyoneRoleId && roleId) everyoneRoleId = roleId;
        basePerms |= parsePermissionBitfield(role.permissions);
      }
      if (roleId && memberRoleIds.has(roleId)) {
        const rp = parsePermissionBitfield(role.permissions);
        basePerms |= rp;
        memberRolePerms |= rp;
      }
    }

    if (!matchedEveryoneRole && guildRoles.length > 0) {
      const candidate = guildRoles.find(
        (r) => r && typeof r === "object" && (r.position === 0 || !memberRoleIds.has(String(r.id ?? ""))),
      );
      if (candidate && memberRoleIds.size === 0) {
        if (!everyoneRoleId && typeof candidate.id === "string") {
          everyoneRoleId = candidate.id.trim();
        }
        basePerms |= parsePermissionBitfield(candidate.permissions);
      }
    }
  } else if (hasInlineMemberRolePerms) {
    hasExplicitBasePerms = true;
    basePerms |= inlineMemberRolePerms;
  }

  if (!hasExplicitBasePerms) {
    basePerms = DEFAULT_BASE_PERMISSIONS;
  }

  if ((basePerms & DISCORD_PERM_ADMINISTRATOR) !== 0n) {
    return Object.freeze({
      permissions: String(DISCORD_ALL_PERMISSIONS),
      canView: true,
      canSend: true,
      canAttach: true,
      canReact: true,
      canConnect: true,
      canSpeak: true,
      placeholder: writablePlaceholder,
      disabledReason: "",
    });
  }

  const rawOverwrites = Array.isArray(opts.overwrites)
    ? opts.overwrites
    : Array.isArray(opts.permission_overwrites)
      ? opts.permission_overwrites
      : Array.isArray(channel?.permission_overwrites)
        ? channel.permission_overwrites
        : Array.isArray(channel?.overwrites)
          ? channel.overwrites
          : null;

  const hasOverwritesProvided = Array.isArray(rawOverwrites) && rawOverwrites.length > 0;
  const overwrites = rawOverwrites ?? [];

  let everyoneAllow = 0n;
  let everyoneDeny = 0n;
  let roleAllow = 0n;
  let roleDeny = 0n;
  let memberAllow = 0n;
  let memberDeny = 0n;

  for (const ow of overwrites) {
    if (!ow || typeof ow !== "object") continue;
    const owId = typeof ow.id === "string" ? ow.id.trim() : "";
    const owType = ow.type;
    const allow = parsePermissionBitfield(ow.allow);
    const deny = parsePermissionBitfield(ow.deny);

    const isMemberType =
      owType === 1 ||
      owType === "1" ||
      owType === "member" ||
      owType === "user" ||
      (userId !== "" && owId === userId && !memberRoleIds.has(owId) && owId !== everyoneRoleId);

    if (isMemberType) {
      if (userId !== "" && owId === userId) {
        memberAllow |= allow;
        memberDeny |= deny;
      }
      continue;
    }

    const isEveryoneOverwrite =
      ow.isEveryone === true ||
      ow.everyone === true ||
      (everyoneRoleId !== "" && owId === everyoneRoleId) ||
      (guildId !== "" && owId === guildId) ||
      owId === "@everyone" ||
      owId === "everyone" ||
      owId === "0" ||
      (!everyoneRoleId &&
        !memberRoleIds.has(owId) &&
        /^(?:@?everyone|0|g(?:uild)?[-_]?\w*|server[-_]?\w*|default)$/i.test(owId));

    if (isEveryoneOverwrite) {
      everyoneAllow |= allow;
      everyoneDeny |= deny;
    } else if (owId !== "" && memberRoleIds.has(owId)) {
      roleAllow |= allow;
      roleDeny |= deny;
    }
  }

  let effectivePerms = basePerms;
  effectivePerms = (effectivePerms & ~everyoneDeny) | everyoneAllow;
  effectivePerms = (effectivePerms & ~roleDeny) | roleAllow;
  effectivePerms = (effectivePerms & ~memberDeny) | memberAllow;

  const viewExplicitlyDenied =
    (((everyoneDeny & ~everyoneAllow & ~roleAllow & ~memberAllow) |
      (roleDeny & ~roleAllow & ~memberAllow) |
      (memberDeny & ~memberAllow)) &
      DISCORD_PERM_VIEW_CHANNEL) !==
    0n;

  if (
    !viewExplicitlyDenied &&
    (effectivePerms & DISCORD_PERM_VIEW_CHANNEL) === 0n &&
    (effectivePerms & (DISCORD_PERM_SEND_MESSAGES | DISCORD_PERM_CONNECT)) !== 0n
  ) {
    effectivePerms |= DISCORD_PERM_VIEW_CHANNEL;
  }

  if (viewExplicitlyDenied || (effectivePerms & DISCORD_PERM_VIEW_CHANNEL) === 0n) {
    effectivePerms &= ~(
      DISCORD_PERM_VIEW_CHANNEL |
      DISCORD_PERM_SEND_MESSAGES |
      DISCORD_PERM_ATTACH_FILES |
      DISCORD_PERM_ADD_REACTIONS |
      DISCORD_PERM_CONNECT |
      DISCORD_PERM_SPEAK
    );
  }

  const hasExplicitSendAllow =
    ((roleAllow | memberAllow) & DISCORD_PERM_SEND_MESSAGES) !== 0n &&
    (memberDeny & DISCORD_PERM_SEND_MESSAGES) === 0n;

  const explicitChannelReadOnly =
    channel?.canSend === false ||
    channel?.canSendMessages === false ||
    channel?.readOnly === true ||
    channel?.read_only === true;

  if (
    explicitChannelReadOnly ||
    (!hasExplicitSendAllow &&
      (memberRolePerms & DISCORD_PERM_SEND_MESSAGES) === 0n &&
      matchesReadOnlyChannelName(rawName, opts.readOnlyNames, hasOverwritesProvided))
  ) {
    effectivePerms &= ~DISCORD_PERM_SEND_MESSAGES;
  }

  if ((effectivePerms & DISCORD_PERM_SEND_MESSAGES) === 0n) {
    effectivePerms &= ~DISCORD_PERM_ATTACH_FILES;
  }
  if ((effectivePerms & DISCORD_PERM_CONNECT) === 0n) {
    effectivePerms &= ~DISCORD_PERM_SPEAK;
  }

  const canView = (effectivePerms & DISCORD_PERM_VIEW_CHANNEL) !== 0n;
  const canSend = canView && (effectivePerms & DISCORD_PERM_SEND_MESSAGES) !== 0n;
  const canAttach = canSend && (effectivePerms & DISCORD_PERM_ATTACH_FILES) !== 0n;
  const canReact = canView && (effectivePerms & DISCORD_PERM_ADD_REACTIONS) !== 0n;
  const canConnect = canView && (effectivePerms & DISCORD_PERM_CONNECT) !== 0n;
  const canSpeak = canConnect && (effectivePerms & DISCORD_PERM_SPEAK) !== 0n;

  const customReason =
    typeof channel?.permissionDeniedReason === "string" && channel.permissionDeniedReason.trim() !== ""
      ? channel.permissionDeniedReason.trim()
      : CHANNEL_SEND_DENIED_REASON;
  const disabledReason = canSend ? "" : customReason;
  const placeholder = canSend ? writablePlaceholder : customReason;

  return Object.freeze({
    permissions: String(effectivePerms),
    canView,
    canSend,
    canAttach,
    canReact,
    canConnect,
    canSpeak,
    placeholder,
    disabledReason,
  });
}

/**
 * Форматирование размера файлов по спецификации §2.3.
 * @param {number} n
 * @returns {string}
 */
export function formatBytes(n) {
  if (n < 1024) return `${n}\u00A0Б`;
  /** @type {number} */
  let val;
  /** @type {string} */
  let unit;
  if (n < 1024 * 1024) {
    val = n / 1024;
    unit = "КиБ";
  } else if (n < 1024 * 1024 * 1024) {
    val = n / (1024 * 1024);
    unit = "МиБ";
  } else {
    val = n / (1024 * 1024 * 1024);
    unit = "ГиБ";
  }
  const formatted = val.toFixed(1).replace(".", ",");
  const cleaned = formatted.endsWith(",0") ? formatted.slice(0, -2) : formatted;
  return `${cleaned}\u00A0${unit}`;
}

/** Русские тексты контракта вложений (§3.2). */
export const ATTACH_TEXT = {
  button: "Прикрепить файл",
  buttonTitle: "Прикрепить файл (до 10)",
  countLabel: "Вложения к сообщению",
  statusRegion: "Состояние загрузки вложений",

  preparing: "Подготовка…",
  /** @param {number} p */
  uploading: (p) => `Загрузка… ${p}%`,
  ready: "Готово",

  /** @param {number} max */
  limitCount: (max) => `Можно прикрепить не больше ${max} файлов.`,
  /** @param {string} max */
  limitTotal: (max) => `Суммарный размер вложений больше ${max}.`,
  emptyFile: "Файл пустой — прикрепите другой.",
  /** @param {string} max */
  tooLarge: (max) => `Файл больше ${max}. Уменьшите размер и попробуйте снова.`,
  /** @param {number} sec */
  rateLimited: (sec) => `Слишком много загрузок. Повторить через ${sec} с.`,
  network: "Не удалось загрузить файл. Проверьте соединение.",
  rejected: "Сервер отклонил файл.",

  retry: "Повторить",
  /** @param {string} name */
  remove: (name) => `Убрать файл «${name}»`,

  sendBlocked: "Дождитесь окончания загрузки файлов.",
  sendBlockedError: "Уберите файлы с ошибкой или повторите загрузку.",
  dropActive: "Отпустите, чтобы прикрепить",

  /** @param {string} name */
  started: (name) => `Файл «${name}» добавлен в очередь.`,
  /** @param {string} name */
  uploaded: (name) => `Файл «${name}» загружен.`,
  /** @param {string} name, @param {string} reason */
  failed: (name, reason) => `Файл «${name}»: ${reason}`,
  /** @param {string} name */
  cancelled: (name) => `Загрузка файла «${name}» отменена.`,
};

/** Русские тексты контракта подарка (§4.5). */
export const GIFT_TEXT = {
  button: "Подарок",
  buttonTitle: "Отправить подарок",
  unavailable: "Подарки недоступны в этом канале",

  title: "Подарить Nitro",
  desc: "Подарок придёт выбранному получателю. Оплата спишется сразу.",
  recipientLabel: "Получатель",
  recipientEmpty: "Получатель не выбран",
  plansLabel: "Что подарить",
  customLabel: "Сообщение к подарку (необязательно)",
  customPlaceholder: "Напишите что-нибудь приятное…",
  /** @param {number} n, @param {number} max */
  customCounter: (n, max) => `${n} / ${max}`,
  loading: "Загружаем варианты подарка…",

  submit: "Подарить",
  submitting: "Отправляем…",
  close: "Закрыть",
  cancel: "Отмена",

  /** @param {number} sec */
  rateLimited: (sec) => `Слишком много попыток. Повторить через ${sec} с.`,
  forbidden: "Подарки недоступны для этого аккаунта.",
  badRequest: "Не удалось создать подарок. Проверьте данные.",
  network: "Нет связи с сервером. Подарок не отправлен.",

  successTitle: "Подарок отправлен",
  /** @param {string} code */
  successCode: (code) => `Код подарка: ${code}`,
  copyCode: "Скопировать код",
  copied: "Код скопирован.",
  copyFailed: "Не удалось скопировать код.",
  done: "Готово",
};

/** @typedef {"empty" | "typing" | "disabled"} ComposerState */
/** @typedef {"none" | "queued" | "uploading" | "ready" | "error"} AttachState */
/** @typedef {"queued" | "uploading" | "ready" | "error-413" | "error-429" | "error-size" | "error-limit" | "error-empty" | "error-network" | "error-other"} ChipState */

/**
 * @typedef {{
 *   id: string,
 *   name: string,
 *   size: number,
 *   type: string,
 *   file?: any,
 *   bytes?: Uint8Array | null,
 *   data?: Uint8Array | null,
 *   state: ChipState,
 *   progress: number,
 *   uploaded_filename?: string,
 *   upload_url?: string,
 *   error?: string,
 *   element?: HTMLElement,
 *   retryTimer?: any,
 *   abortController?: AbortController,
 * }} AttachmentItem
 */

/**
 * @param {{
 *   form: HTMLElement,
 *   input: HTMLTextAreaElement,
 *   hint?: HTMLElement | null,
 *   onError?: (error: UIError) => void,
 *   onEscape?: () => void,
 *   onSubmit?: (content: string, attachments?: Array<{ id: string, filename: string, uploaded_filename: string }>) => void,
 *   onType?: () => void,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   uploadPut?: (payload: {
 *     channelId: string,
 *     id: string,
 *     uploadUrl: string,
 *     uploadFilename: string,
 *     filename: string,
 *     fileSize: number,
 *     contentType: string,
 *     file: any,
 *     bytes: Uint8Array | null,
 *     data: Uint8Array | null,
 *   }) => Promise<any>,
 *   getChannelId?: () => string,
 *   channelId?: string,
 *   maxFileSize?: number,
 *   maxAttachments?: number,
 * }} options
 */
export function createComposer(options) {
  const { form, input } = options;
  const onError = options.onError ?? (() => {});
  const onSubmit = options.onSubmit ?? (() => {});
  const onType = options.onType ?? (() => {});
  if (!form || !input) {
    throw new UIError("ELEMENT_MISSING", "композер: нет #composer/#composer-input");
  }

  const doc = form.ownerDocument || (typeof document !== "undefined" ? document : null);
  if (!doc) {
    throw new UIError("ELEMENT_MISSING", "композер: отсутствует document");
  }

  // --- Progressive enhancement: Счётчик текста ---
  /** @type {HTMLElement} */
  let counter = form.querySelector(".composer-counter") || doc.createElement("span");
  if (!counter.parentElement) {
    counter.className = "composer-counter";
    counter.setAttribute("data-visible", "false");
    counter.setAttribute("data-exceeded", "false");
    counter.setAttribute("aria-hidden", "true");
    counter.textContent = `0/${COMPOSER_LIMIT}`;
    form.append(counter);
  }

  // --- Progressive enhancement: Чип ответа ---
  /** @type {HTMLElement} */
  let replyChip = form.querySelector(".composer-reply") || doc.createElement("div");
  /** @type {HTMLElement} */
  let replyText;
  /** @type {HTMLElement} */
  let replyCancel;
  if (!replyChip.parentElement) {
    replyChip.className = "composer-reply";
    replyChip.setAttribute("data-visible", "false");
    replyText = doc.createElement("span");
    replyText.className = "composer-reply-text";
    replyCancel = doc.createElement("button");
    replyCancel.className = "composer-reply-cancel";
    replyCancel.setAttribute("type", "button");
    replyCancel.setAttribute("aria-label", "Отменить ответ");
    replyCancel.textContent = "✕";
    replyChip.append(replyText, replyCancel);
    form.append(replyChip);
  } else {
    replyText = replyChip.querySelector(".composer-reply-text") || doc.createElement("span");
    replyCancel = replyChip.querySelector(".composer-reply-cancel") || doc.createElement("button");
  }

  // --- Progressive enhancement: Кнопка attach и скрытый file input ---
  /** @type {HTMLElement | null} */
  const attachBtn = form.querySelector('[data-action="attach"]');
  if (attachBtn) {
    attachBtn.setAttribute("title", ATTACH_TEXT.buttonTitle);
  }

  /** @type {HTMLInputElement | null} */
  let fileInput = /** @type {HTMLInputElement | null} */ (form.querySelector("#composer-file-input") || doc.getElementById("composer-file-input"));
  if (!fileInput) {
    fileInput = /** @type {HTMLInputElement} */ (doc.createElement("input"));
    fileInput.id = "composer-file-input";
    fileInput.setAttribute("type", "file");
    fileInput.setAttribute("multiple", "");
    fileInput.hidden = true;
    fileInput.setAttribute("aria-hidden", "true");
    fileInput.tabIndex = -1;
    form.append(fileInput);
  }

  // --- Progressive enhancement: Очередь вложений и счётчик вложений ---
  /** @type {HTMLElement | null} */
  let queueEl = form.querySelector("#composer-attachments") || doc.getElementById("composer-attachments");
  if (!queueEl) {
    queueEl = doc.createElement("div");
    queueEl.id = "composer-attachments";
    queueEl.className = "composer-attachments";
    queueEl.setAttribute("role", "list");
    queueEl.setAttribute("aria-label", ATTACH_TEXT.countLabel);
    queueEl.hidden = true;
    form.append(queueEl);
  }

  /** @type {HTMLElement | null} */
  let attachCountEl = form.querySelector("#composer-attach-count") || doc.getElementById("composer-attach-count");
  if (!attachCountEl) {
    attachCountEl = doc.createElement("span");
    attachCountEl.id = "composer-attach-count";
    attachCountEl.className = "composer-attach-count";
    attachCountEl.setAttribute("data-visible", "false");
    attachCountEl.textContent = `0 / ${MAX_ATTACHMENTS_PER_MESSAGE}`;
    form.append(attachCountEl);
  }

  // --- Progressive enhancement: Live-region и hint ---
  /** @type {HTMLElement | null} */
  let attachStatusEl = doc.getElementById("composer-attach-status");
  if (!attachStatusEl) {
    attachStatusEl = doc.createElement("p");
    attachStatusEl.id = "composer-attach-status";
    attachStatusEl.className = "visually-hidden";
    attachStatusEl.setAttribute("role", "status");
    attachStatusEl.setAttribute("aria-live", "polite");
    attachStatusEl.setAttribute("aria-atomic", "true");
    if (form.parentElement) form.parentElement.append(attachStatusEl);
    else if (doc.body) doc.body.append(attachStatusEl);
  }

  /** @type {HTMLElement | null} */
  let attachHintEl = doc.getElementById("composer-attach-hint");
  if (!attachHintEl) {
    attachHintEl = doc.createElement("p");
    attachHintEl.id = "composer-attach-hint";
    attachHintEl.className = "visually-hidden";
    if (form.parentElement) form.parentElement.append(attachHintEl);
    else if (doc.body) doc.body.append(attachHintEl);
  }

  // --- Progressive enhancement: Кнопка подарка и модалка подарка ---
  /** @type {HTMLElement | null} */
  const giftBtn = form.querySelector('[data-action="composer-gift"]') || form.querySelector('[data-action="gift"]');
  if (giftBtn) {
    giftBtn.removeAttribute("aria-disabled");
    giftBtn.setAttribute("title", GIFT_TEXT.buttonTitle);
    giftBtn.setAttribute("aria-haspopup", "dialog");
    giftBtn.setAttribute("aria-expanded", "false");
    giftBtn.setAttribute("aria-controls", "gift-modal");
  }

  /** @type {HTMLElement | null} */
  let giftModal = doc.getElementById("gift-modal");
  if (!giftModal) {
    giftModal = doc.createElement("div");
    giftModal.id = "gift-modal";
    giftModal.className = "dialog-backdrop";
    giftModal.hidden = true;

    const card = doc.createElement("div");
    card.className = "dialog gift-modal-card";
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-labelledby", "gift-modal-title");
    card.setAttribute("aria-describedby", "gift-modal-desc");
    card.setAttribute("data-state", "closed");

    const header = doc.createElement("header");
    header.className = "gift-head";
    const title = doc.createElement("h2");
    title.id = "gift-modal-title";
    title.className = "dialog-title";
    title.textContent = GIFT_TEXT.title;
    const closeBtn = doc.createElement("button");
    closeBtn.className = "icon-btn gift-close";
    closeBtn.setAttribute("type", "button");
    closeBtn.setAttribute("data-action", "gift-close");
    closeBtn.setAttribute("aria-label", GIFT_TEXT.close);
    closeBtn.textContent = "✕";
    header.append(title, closeBtn);

    const desc = doc.createElement("p");
    desc.id = "gift-modal-desc";
    desc.className = "dialog-text";
    desc.textContent = GIFT_TEXT.desc;

    const recipientBox = doc.createElement("div");
    recipientBox.className = "gift-recipient";
    const recipientLbl = doc.createElement("span");
    recipientLbl.className = "gift-label";
    recipientLbl.textContent = GIFT_TEXT.recipientLabel;
    const recipientName = doc.createElement("span");
    recipientName.id = "gift-recipient-name";
    recipientName.className = "gift-recipient-name";
    recipientName.textContent = GIFT_TEXT.recipientEmpty;
    recipientBox.append(recipientLbl, recipientName);

    const fieldset = doc.createElement("fieldset");
    fieldset.className = "gift-plans";
    fieldset.id = "gift-plans";
    const legend = doc.createElement("legend");
    legend.className = "gift-label";
    legend.textContent = GIFT_TEXT.plansLabel;
    const planLabel = doc.createElement("label");
    planLabel.className = "gift-plan";
    const planRadio = doc.createElement("input");
    planRadio.setAttribute("type", "radio");
    planRadio.setAttribute("name", "gift-plan");
    planRadio.setAttribute("value", "1");
    planRadio.setAttribute("checked", "");
    const planName = doc.createElement("span");
    planName.className = "gift-plan-name";
    planName.textContent = "1 месяц Nitro";
    const planPrice = doc.createElement("span");
    planPrice.className = "gift-plan-price";
    planLabel.append(planRadio, planName, planPrice);
    fieldset.append(legend, planLabel);

    const customLabel = doc.createElement("label");
    customLabel.className = "gift-custom";
    const customTitle = doc.createElement("span");
    customTitle.className = "gift-label";
    customTitle.textContent = GIFT_TEXT.customLabel;
    const customInput = doc.createElement("textarea");
    customInput.id = "gift-custom-message";
    customInput.className = "gift-custom-input";
    customInput.setAttribute("rows", "3");
    customInput.setAttribute("maxlength", "200");
    customInput.setAttribute("placeholder", GIFT_TEXT.customPlaceholder);
    const customCount = doc.createElement("span");
    customCount.id = "gift-custom-count";
    customCount.className = "gift-counter";
    customCount.textContent = "0 / 200";
    customLabel.append(customTitle, customInput, customCount);

    const catalogStatus = doc.createElement("div");
    catalogStatus.id = "gift-catalog-status";
    catalogStatus.className = "gift-catalog-status";
    const catalogNotice = doc.createElement("p");
    catalogNotice.className = "gift-catalog-notice";
    catalogNotice.textContent = "Каталог Nitro: режим просмотра. Реальная оплата отключена (спайк).";
    catalogStatus.append(catalogNotice);

    const errorEl = doc.createElement("p");
    errorEl.id = "gift-error";
    errorEl.className = "dialog-error";
    errorEl.setAttribute("role", "alert");
    errorEl.setAttribute("aria-live", "assertive");
    errorEl.hidden = true;

    const successEl = doc.createElement("div");
    successEl.id = "gift-success";
    successEl.className = "gift-success";
    successEl.hidden = true;
    const successTitle = doc.createElement("p");
    successTitle.className = "gift-success-title";
    successTitle.textContent = GIFT_TEXT.successTitle;
    const successCode = doc.createElement("p");
    successCode.id = "gift-success-code";
    successCode.className = "gift-success-code";
    const copyBtn = doc.createElement("button");
    copyBtn.className = "gift-copy";
    copyBtn.setAttribute("type", "button");
    copyBtn.setAttribute("data-action", "gift-copy-code");
    copyBtn.textContent = GIFT_TEXT.copyCode;
    successEl.append(successTitle, successCode, copyBtn);

    const actions = doc.createElement("div");
    actions.className = "dialog-actions";
    const cancelBtn = doc.createElement("button");
    cancelBtn.className = "btn btn-secondary";
    cancelBtn.setAttribute("type", "button");
    cancelBtn.setAttribute("data-action", "gift-close");
    cancelBtn.textContent = GIFT_TEXT.cancel;
    const submitBtn = doc.createElement("button");
    submitBtn.id = "gift-submit";
    submitBtn.className = "btn btn-primary";
    submitBtn.setAttribute("type", "button");
    submitBtn.setAttribute("data-action", "gift-submit");
    submitBtn.textContent = GIFT_TEXT.submit;
    actions.append(cancelBtn, submitBtn);

    card.append(header, desc, recipientBox, fieldset, customLabel, catalogStatus, errorEl, successEl, actions);
    giftModal.append(card);

    if (doc.body) doc.body.append(giftModal);
    else if (form.parentElement) form.parentElement.append(giftModal);
  }

  // --- Внутреннее состояние ---
  /** @type {ComposerState} */
  let state = "empty";
  /** @type {AttachState} */
  let attachState = "none";
  /** @type {{author: string, text: string, id?: string} | null} */
  let reply = null;
  let disabledReason = "";
  /** Значение на момент disabled: поле остаётся в DOM, но ввод не проходит. */
  let frozenValue = "";
  /** @type {AttachmentItem[]} */
  const attachments = [];

  let idCounter = 0;
  function generateAttachmentId() {
    idCounter += 1;
    return String(idCounter);
  }

  /** @param {string} msg */
  function announce(msg) {
    if (attachStatusEl) attachStatusEl.textContent = msg;
  }

  /** @param {ComposerState} next */
  function setState(next) {
    state = next;
    form.setAttribute("data-state", next);
    if (next === "disabled") {
      input.setAttribute("aria-disabled", "true");
      form.setAttribute("data-disabled-reason", disabledReason);
    } else {
      input.removeAttribute("aria-disabled");
      form.removeAttribute("data-disabled-reason");
    }
  }

  /** @returns {string} */
  function value() {
    return typeof input.value === "string" ? input.value : "";
  }

  function notifyType() {
    try {
      onType();
    } catch (error) {
      onError(asUIError("ACTION_FAILED", error));
    }
  }

  function refresh() {
    const length = value().length;
    const exceeded = length >= COMPOSER_LIMIT;
    form.setAttribute("data-limit", exceeded ? "exceeded" : "none");
    counter.textContent = `${length}/${COMPOSER_LIMIT}`;
    counter.setAttribute("data-visible", length >= COUNTER_FROM ? "true" : "false");
    counter.setAttribute("data-exceeded", exceeded ? "true" : "false");
    if (length >= COUNTER_FROM) counter.setAttribute("aria-hidden", "false");
    else counter.setAttribute("aria-hidden", "true");
  }

  function updateQueueState() {
    if (attachCountEl) {
      attachCountEl.textContent = `${attachments.length} / ${MAX_ATTACHMENTS_PER_MESSAGE}`;
      attachCountEl.setAttribute("data-visible", attachments.length > 0 ? "true" : "false");
      attachCountEl.setAttribute("data-full", attachments.length >= MAX_ATTACHMENTS_PER_MESSAGE ? "true" : "false");
    }

    if (attachments.length >= MAX_ATTACHMENTS_PER_MESSAGE) {
      attachBtn?.setAttribute("aria-disabled", "true");
    } else {
      attachBtn?.removeAttribute("aria-disabled");
    }

    if (attachments.length === 0) {
      attachState = "none";
      if (queueEl) queueEl.hidden = true;
      if (attachHintEl) attachHintEl.textContent = "";
    } else {
      if (queueEl) queueEl.hidden = false;
      if (attachments.some((a) => a.state.startsWith("error"))) {
        attachState = "error";
        if (attachHintEl) attachHintEl.textContent = ATTACH_TEXT.sendBlockedError;
      } else if (attachments.some((a) => a.state === "uploading")) {
        attachState = "uploading";
        if (attachHintEl) attachHintEl.textContent = ATTACH_TEXT.sendBlocked;
      } else if (attachments.some((a) => a.state === "queued")) {
        attachState = "queued";
        if (attachHintEl) attachHintEl.textContent = ATTACH_TEXT.sendBlocked;
      } else if (attachments.every((a) => a.state === "ready")) {
        attachState = "ready";
        if (attachHintEl) attachHintEl.textContent = "";
      }
    }
    form.setAttribute("data-attach-state", attachState);
  }

  /**
   * @param {AttachmentItem} item
   */
  function createChipEl(item) {
    if (!queueEl) return;
    const chip = doc.createElement("div");
    chip.className = "attachment-chip";
    chip.setAttribute("role", "listitem");
    chip.setAttribute("data-attachment-id", item.id);
    chip.setAttribute("data-state", item.state);
    chip.setAttribute("aria-busy", item.state === "uploading" ? "true" : "false");

    const thumb = doc.createElement("img");
    thumb.className = "attachment-thumb";
    thumb.setAttribute("src", "");
    thumb.setAttribute("alt", "");
    thumb.setAttribute("aria-hidden", "true");

    const meta = doc.createElement("div");
    meta.className = "attachment-meta";

    const name = doc.createElement("span");
    name.className = "attachment-name";
    name.setAttribute("title", item.name);
    name.textContent = item.name;

    const size = doc.createElement("span");
    size.className = "attachment-size";
    size.textContent = formatBytes(item.size);

    const statusEl = doc.createElement("span");
    statusEl.className = "attachment-status";
    statusEl.textContent = item.state === "ready" ? ATTACH_TEXT.ready : ATTACH_TEXT.preparing;

    const errorEl = doc.createElement("span");
    errorEl.className = "attachment-error";
    errorEl.setAttribute("role", "alert");
    if (item.error) {
      errorEl.textContent = item.error;
      errorEl.hidden = false;
    } else {
      errorEl.hidden = true;
    }

    meta.append(name, size, statusEl, errorEl);

    const progress = doc.createElement("div");
    progress.className = "attachment-progress";
    progress.setAttribute("role", "progressbar");
    progress.setAttribute("aria-label", `Загрузка «${item.name}»`);
    progress.setAttribute("aria-valuemin", "0");
    progress.setAttribute("aria-valuemax", "100");
    progress.setAttribute("aria-valuenow", String(item.progress || 0));
    progress.setAttribute("aria-valuetext", `${item.progress || 0}%`);

    const fill = doc.createElement("div");
    fill.className = "attachment-progress-fill";
    fill.style.width = `${item.progress || 0}%`;
    progress.append(fill);

    const actions = doc.createElement("div");
    actions.className = "attachment-actions";

    const retryBtn = doc.createElement("button");
    retryBtn.className = "attachment-action";
    retryBtn.setAttribute("type", "button");
    retryBtn.setAttribute("data-action", "attachment-retry");
    retryBtn.textContent = ATTACH_TEXT.retry;
    retryBtn.hidden = true;

    const removeBtn = doc.createElement("button");
    removeBtn.className = "attachment-action attachment-remove";
    removeBtn.setAttribute("type", "button");
    removeBtn.setAttribute("data-action", "attachment-remove");
    removeBtn.setAttribute("aria-label", ATTACH_TEXT.remove(item.name));
    removeBtn.setAttribute("title", "Убрать файл");
    removeBtn.textContent = "✕";

    actions.append(retryBtn, removeBtn);
    chip.append(thumb, meta, progress, actions);

    removeBtn.addEventListener("click", () => {
      removeAttachment(item.id);
    });

    retryBtn.addEventListener("click", () => {
      if (item.state === "error-network" || item.state === "error-429") {
        void startUpload(item);
      }
    });

    item.element = chip;
    queueEl.append(chip);
    updateChipEl(item);
  }

  /**
   * @param {AttachmentItem} item
   */
  function updateChipEl(item) {
    const chip = item.element;
    if (!chip) return;

    chip.setAttribute("data-state", item.state);
    chip.setAttribute("aria-busy", item.state === "uploading" ? "true" : "false");

    const statusEl = /** @type {HTMLElement | null} */ (chip.querySelector(".attachment-status"));
    const errorEl = /** @type {HTMLElement | null} */ (chip.querySelector(".attachment-error"));
    const progressEl = chip.querySelector(".attachment-progress");
    const fillEl = /** @type {HTMLElement | null} */ (chip.querySelector(".attachment-progress-fill"));
    const retryBtn = /** @type {HTMLElement | null} */ (chip.querySelector('[data-action="attachment-retry"]'));

    if (item.state === "ready") {
      if (statusEl) {
        statusEl.textContent = ATTACH_TEXT.ready;
        statusEl.hidden = false;
      }
      if (errorEl) errorEl.hidden = true;
      if (progressEl) {
        progressEl.setAttribute("aria-valuenow", "100");
        progressEl.setAttribute("aria-valuetext", "100%");
      }
      if (fillEl) fillEl.style.width = "100%";
      if (retryBtn) retryBtn.hidden = true;
    } else if (item.state === "uploading") {
      if (statusEl) {
        statusEl.textContent = ATTACH_TEXT.uploading(item.progress || 0);
        statusEl.hidden = false;
      }
      if (errorEl) errorEl.hidden = true;
      if (progressEl) {
        progressEl.setAttribute("aria-valuenow", String(item.progress || 0));
        progressEl.setAttribute("aria-valuetext", `${item.progress || 0}%`);
      }
      if (fillEl) fillEl.style.width = `${item.progress || 0}%`;
      if (retryBtn) retryBtn.hidden = true;
    } else if (item.state.startsWith("error")) {
      if (statusEl) statusEl.hidden = true;
      if (errorEl) {
        errorEl.textContent = item.error || "Ошибка";
        errorEl.hidden = false;
      }
      if (retryBtn) {
        retryBtn.hidden = !(item.state === "error-network" || item.state === "error-429");
      }
    } else {
      // queued
      if (statusEl) {
        statusEl.textContent = ATTACH_TEXT.preparing;
        statusEl.hidden = false;
      }
      if (errorEl) errorEl.hidden = true;
      if (retryBtn) retryBtn.hidden = true;
    }
  }

  /**
   * Кодирует крупный Uint8Array в base64, чтобы не сериализовать миллионы ключей JSON в IPC.
   * @param {Uint8Array} bytes
   * @returns {string}
   */
  function uint8ToBase64(bytes) {
    if (typeof btoa !== "function") {
      // Fallback для окружения без btoa
      const NodeBuffer = /** @type {any} */ (globalThis).Buffer;
      if (typeof NodeBuffer !== "undefined") {
        return NodeBuffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength).toString("base64");
      }
      return "";
    }
    const chunkSize = 0x8000;
    const chunks = [];
    for (let i = 0; i < bytes.byteLength; i += chunkSize) {
      const sub = bytes.subarray(i, Math.min(i + chunkSize, bytes.byteLength));
      chunks.push(String.fromCharCode.apply(null, /** @type {any} */ (sub)));
    }
    return btoa(chunks.join(""));
  }

  /**
   * @param {AttachmentItem} item
   */
  async function startUpload(item) {
    /** @type {any} */
    const tauri = typeof globalThis !== "undefined" ? /** @type {any} */ (globalThis).__TAURI__ : undefined;
    const invokeFn = options.invoke || tauri?.core?.invoke;
    if (!invokeFn && !options.uploadPut) return;

    item.state = "uploading";
    item.progress = 15;
    updateChipEl(item);
    updateQueueState();

    try {
      const channelId = options.getChannelId?.() ?? options.channelId ?? "default";
      if (!invokeFn) {
        throw new Error("Missing invoke function for upload slots");
      }
      const res = await invokeFn("attachments_upload_slots", {
        channelId,
        files: [
          {
            filename: item.name,
            file_size: item.size,
            id: item.id,
            is_clip: Boolean(item.file?.isClip),
          },
        ],
      });

      const slot =
        res?.attachments?.find((/** @type {any} */ s) => String(s.id) === String(item.id)) ||
        res?.attachments?.[0];
      if (!slot || !slot.upload_url || !slot.upload_filename) {
        throw new Error("Missing upload slot");
      }

      item.upload_url = slot.upload_url;
      item.progress = 50;
      updateChipEl(item);

      const rawBytes = item.bytes instanceof Uint8Array ? item.bytes : null;
      const useBase64FastPath =
        typeof options.uploadPut !== "function" &&
        rawBytes !== null &&
        rawBytes.byteLength > 8192;

      const putPayload = {
        channelId,
        id: slot.id,
        uploadUrl: slot.upload_url,
        uploadFilename: slot.upload_filename,
        filename: item.name,
        fileSize: item.size,
        contentType: item.type,
        file: useBase64FastPath ? null : (item.file ?? null),
        bytes: useBase64FastPath ? null : (item.bytes ?? null),
        data: useBase64FastPath
          ? /** @type {any} */ ("base64:" + uint8ToBase64(rawBytes))
          : (item.data ?? item.bytes ?? null),
      };

      if (typeof options.uploadPut === "function") {
        await options.uploadPut(putPayload);
      } else {
        await invokeFn("attachments_upload_put", putPayload);
      }

      item.uploaded_filename = slot.upload_filename;
      item.state = "ready";
      item.progress = 100;
      updateChipEl(item);
      announce(ATTACH_TEXT.uploaded(item.name));
    } catch (err) {
      const error = /** @type {any} */ (err);
      const msg = String(error?.message || error || "");
      const status = error?.status;
      if (status === 413 || msg.includes("EntityTooLarge") || msg.includes("40005") || msg.includes("413")) {
        item.state = "error-413";
        item.error = ATTACH_TEXT.tooLarge(formatBytes(MAX_UPLOAD_FILE_BYTES));
        announce(ATTACH_TEXT.failed(item.name, item.error));
      } else if (status === 429 || msg.includes("RateLimited") || error?.retry_after_ms) {
        item.state = "error-429";
        const sec = Math.ceil((error?.retry_after_ms || 5000) / 1000);
        item.error = ATTACH_TEXT.rateLimited(sec);
        announce(ATTACH_TEXT.failed(item.name, item.error));
      } else {
        item.state = "error-network";
        item.error = ATTACH_TEXT.network;
        announce(ATTACH_TEXT.failed(item.name, item.error));
      }
      updateChipEl(item);
    } finally {
      updateQueueState();
    }
  }

  /**
   * Извлекает байты из File/Blob или объекта с полем bytes/data.
   * @param {any} f
   * @returns {Promise<Uint8Array | null>}
   */
  async function extractFileBytes(f) {
    if (!f || typeof f !== "object") return null;
    if (f.bytes instanceof Uint8Array) return f.bytes;
    if (ArrayBuffer.isView(f.bytes)) {
      return new Uint8Array(f.bytes.buffer, f.bytes.byteOffset, f.bytes.byteLength);
    }
    if (f.bytes instanceof ArrayBuffer) return new Uint8Array(f.bytes);
    if (Array.isArray(f.bytes)) return new Uint8Array(f.bytes);
    if (f.data instanceof Uint8Array) return f.data;
    if (ArrayBuffer.isView(f.data)) {
      return new Uint8Array(f.data.buffer, f.data.byteOffset, f.data.byteLength);
    }
    if (f.data instanceof ArrayBuffer) return new Uint8Array(f.data);
    if (Array.isArray(f.data)) return new Uint8Array(f.data);
    if (typeof f.arrayBuffer === "function") {
      const buf = await f.arrayBuffer();
      return new Uint8Array(buf);
    }
    return null;
  }

  /**
   * Добавление списка файлов в очередь.
   * @param {Array<{ name: string, size?: number, type?: string, isClip?: boolean, bytes?: any, data?: any, arrayBuffer?: () => Promise<ArrayBuffer> }> | FileList} files
   * @returns {Promise<void>}
   */
  async function addFiles(files) {
    if (!files) return;
    const fileArray = Array.from(files);
    /** @type {Promise<any>[]} */
    const uploads = [];
    for (const f of fileArray) {
      const size = typeof f.size === "number" ? f.size : 0;
      /** @type {AttachmentItem} */
      const item = {
        id: generateAttachmentId(),
        name: f.name || "файл",
        size,
        type: f.type || "application/octet-stream",
        file: f,
        bytes: null,
        data: null,
        state: "queued",
        progress: 0,
      };

      if (attachments.length >= MAX_ATTACHMENTS_PER_MESSAGE) {
        item.state = "error-limit";
        item.error = ATTACH_TEXT.limitCount(MAX_ATTACHMENTS_PER_MESSAGE);
        attachments.push(item);
        createChipEl(item);
        announce(item.error);
        continue;
      }

      if (size === 0) {
        item.state = "error-empty";
        item.error = ATTACH_TEXT.emptyFile;
        attachments.push(item);
        createChipEl(item);
        announce(ATTACH_TEXT.failed(item.name, item.error));
        continue;
      }

      if (size > MAX_UPLOAD_FILE_BYTES) {
        item.state = "error-size";
        item.error = ATTACH_TEXT.tooLarge(formatBytes(MAX_UPLOAD_FILE_BYTES));
        attachments.push(item);
        createChipEl(item);
        announce(ATTACH_TEXT.failed(item.name, item.error));
        continue;
      }

      attachments.push(item);
      createChipEl(item);
      announce(ATTACH_TEXT.started(item.name));
      const bytes = await extractFileBytes(f);
      item.bytes = bytes;
      item.data = bytes;
      uploads.push(startUpload(item));
    }
    updateQueueState();
    await Promise.all(uploads);
    if (typeof input.focus === "function") {
      input.focus();
    }
  }

  /**
   * @param {string} id
   */
  function removeAttachment(id) {
    const idx = attachments.findIndex((a) => a.id === id);
    if (idx === -1) return;
    const [item] = attachments.splice(idx, 1);
    if (item?.retryTimer) clearInterval(item.retryTimer);
    if (item?.abortController) item.abortController.abort();
    if (item?.element) item.element.remove();
    if (item) announce(ATTACH_TEXT.cancelled(item.name));
    updateQueueState();
  }

  function clearAttachments() {
    while (attachments.length > 0) {
      const item = attachments.pop();
      if (item?.retryTimer) clearInterval(item.retryTimer);
      if (item?.abortController) item.abortController.abort();
      if (item?.element) item.element.remove();
    }
    updateQueueState();
  }

  // --- Модалка подарка: открытие / закрытие ---
  function openGiftModal() {
    if (!giftModal) return;
    giftModal.hidden = false;
    giftModal.removeAttribute("hidden");
    const card = giftModal.querySelector(".gift-modal-card");
    if (card) card.setAttribute("data-state", "ready");
    if (giftBtn) giftBtn.setAttribute("aria-expanded", "true");
    const firstFocusable = /** @type {HTMLElement | null} */ (
      giftModal.querySelector('button, [tabindex="0"], input')
    );
    if (firstFocusable && typeof firstFocusable.focus === "function") firstFocusable.focus();
  }

  function closeGiftModal() {
    if (!giftModal) return;
    const card = giftModal.querySelector(".gift-modal-card");
    if (card?.getAttribute("data-state") === "submitting") return;
    giftModal.hidden = true;
    giftModal.setAttribute("hidden", "");
    if (card) card.setAttribute("data-state", "closed");
    if (giftBtn) {
      giftBtn.setAttribute("aria-expanded", "false");
      if (typeof giftBtn.focus === "function") giftBtn.focus();
    }
  }

  if (giftModal) {
    giftModal.addEventListener("keydown", (/** @type {any} */ e) => {
      if (e.key === "Escape") closeGiftModal();
    });
    giftModal.addEventListener("click", (/** @type {any} */ e) => {
      if (e.target === giftModal) closeGiftModal();
    });
    const closeButtons = giftModal.querySelectorAll('[data-action="gift-close"]');
    for (const b of closeButtons) {
      b.addEventListener("click", () => closeGiftModal());
    }
  }

  const escapeTarget = /** @type {any} */ (doc.body || doc);
  if (typeof escapeTarget.addEventListener === "function") {
    escapeTarget.addEventListener("keydown", (/** @type {any} */ e) => {
      if (e.key === "Escape" && giftModal && !giftModal.hidden) {
        closeGiftModal();
      }
    });
  }

  if (attachBtn) {
    attachBtn.addEventListener("click", () => {
      if (attachBtn.getAttribute("aria-disabled") === "true") return;
      fileInput?.click();
    });
  }

  if (fileInput) {
    fileInput.addEventListener("change", () => {
      if (fileInput?.files) addFiles(fileInput.files);
      if (fileInput) fileInput.value = "";
    });
  }

  if (giftBtn) {
    giftBtn.addEventListener("click", () => {
      if (giftBtn.getAttribute("aria-disabled") === "true") return;
      openGiftModal();
    });
  }

  /** Обработчик ввода: бюджет + состояния. Ошибки — только через UIError. */
  function onInput() {
    try {
      if (state === "disabled" || input.disabled) {
        if (value() !== frozenValue) input.value = frozenValue;
        return;
      }
      const raw = value();
      if (raw.length > COMPOSER_LIMIT) {
        input.value = raw.slice(0, COMPOSER_LIMIT);
      }
      refresh();
      setState(value().length === 0 ? "empty" : "typing");
      if (value().length > 0) notifyType();
    } catch (error) {
      onError(asUIError("ACTION_FAILED", error));
    }
  }

  /**
   * Esc: сначала отменяет ответ, иначе отдаёт событие приложению.
   * G2-01: режим ответа — состояние, а не клавиша.
   */
  function onKeydown(/** @type {KeyboardEvent} */ event) {
    if (event.key === "Enter") {
      if (event.shiftKey) return;
      event.preventDefault();
      if (state === "disabled" || input.disabled) return;

      // §3.4: блокировка отправки при queued, uploading или error
      if (attachState === "queued" || attachState === "uploading" || attachState === "error") {
        return;
      }

      const text = value();
      const readyAttachments = attachments
        .filter((a) => a.state === "ready" && Boolean(a.uploaded_filename))
        .map((a) => ({
          id: a.id,
          filename: a.name,
          uploaded_filename: /** @type {string} */ (a.uploaded_filename),
        }));

      if (text.trim() !== "" || readyAttachments.length > 0) {
        input.value = "";
        refresh();
        setState("empty");
        onSubmit(text, readyAttachments);
        clearAttachments();
      }
      return;
    }
    if (event.key !== "Escape") return;
    if (reply) {
      clearReply();
      return;
    }
    options.onEscape?.();
  }

  /**
   * @param {{author: string, text: string, id?: string}} target
   */
  function setReply(target) {
    reply = target;
    replyText.textContent = `Ответ ${target.author}: ${target.text}`;
    replyText.setAttribute("title", target.text);
    replyChip.setAttribute("data-visible", "true");
    form.setAttribute("data-reply", "true");
  }

  function clearReply() {
    reply = null;
    replyChip.setAttribute("data-visible", "false");
    replyText.textContent = "";
    form.setAttribute("data-reply", "false");
  }

  /**
   * @param {boolean} flag
   * @param {string} [reason]
   */
  function setDisabled(flag, reason = "") {
    disabledReason = flag ? reason : "";
    if (flag) {
      frozenValue = value();
      input.disabled = true;
      setState("disabled");
      return;
    }
    input.disabled = false;
    refresh();
    setState(value().length === 0 ? "empty" : "typing");
  }

  /**
   * Применяет вычисленную политику канала к композеру и кнопке вложений.
   * @param {{
   *   canSend?: boolean,
   *   canAttach?: boolean,
   *   placeholder?: string,
   *   disabledReason?: string,
   * } | null | undefined} policy
   */
  function applyChannelPolicy(policy) {
    if (!policy || typeof policy !== "object") return;
    if (policy.canSend === false) {
      const reason =
        typeof policy.disabledReason === "string" && policy.disabledReason.length > 0
          ? policy.disabledReason
          : CHANNEL_SEND_DENIED_REASON;
      const nextPlaceholder =
        typeof policy.placeholder === "string" && policy.placeholder.length > 0
          ? policy.placeholder
          : reason;
      setDisabled(true, reason);
      input.placeholder = nextPlaceholder;
      if (typeof input.setAttribute === "function") {
        input.setAttribute("placeholder", nextPlaceholder);
      }
      if (attachBtn) {
        attachBtn.setAttribute("aria-disabled", "true");
        attachBtn.setAttribute("title", reason);
      }
    } else if (policy.canSend === true) {
      setDisabled(false);
      if (typeof policy.placeholder === "string") {
        input.placeholder = policy.placeholder;
        if (typeof input.setAttribute === "function") {
          input.setAttribute("placeholder", policy.placeholder);
        }
      }
      if (attachBtn) {
        if (policy.canAttach === false) {
          attachBtn.setAttribute("aria-disabled", "true");
          attachBtn.setAttribute("title", "У вас нет прав на прикрепление файлов в этом канале");
        } else if (attachments.length < MAX_ATTACHMENTS_PER_MESSAGE) {
          attachBtn.removeAttribute("aria-disabled");
          attachBtn.setAttribute("title", ATTACH_TEXT.buttonTitle);
        }
      }
    }
  }

  /**
   * Каретка/выделение поля.
   * @returns {{ start: number, end: number }}
   */
  function caretRange() {
    const length = value().length;
    const rawStart = typeof input.selectionStart === "number" ? input.selectionStart : null;
    const rawEnd = typeof input.selectionEnd === "number" ? input.selectionEnd : null;
    const start = rawStart === null ? length : Math.min(length, Math.max(0, rawStart));
    const end = rawEnd === null ? start : Math.min(length, Math.max(start, rawEnd));
    return { start, end };
  }

  /**
   * Обрезка до лимита без разрыва суррогатной пары.
   * @param {string} text
   * @returns {string}
   */
  function clipToLimit(text) {
    if (text.length <= COMPOSER_LIMIT) return text;
    let end = COMPOSER_LIMIT;
    const code = text.charCodeAt(end - 1);
    if (code >= 0xd800 && code <= 0xdbff) end -= 1;
    return text.slice(0, end);
  }

  /**
   * Вставка текста в позицию курсора с учётом лимита.
   * @param {string} text
   * @returns {boolean}
   */
  function insertText(text) {
    try {
      if (state === "disabled" || text === "") return false;
      const { start, end } = caretRange();
      const next = clipToLimit(value().slice(0, start) + text + value().slice(end));
      input.value = next;
      const caret = Math.min(start + text.length, next.length);
      input.setSelectionRange(caret, caret);
      refresh();
      setState(next.length === 0 ? "empty" : "typing");
      if (next.length > 0) notifyType();
      return true;
    } catch (error) {
      onError(asUIError("ACTION_FAILED", error));
      return false;
    }
  }

  /**
   * Вставка файлов и скриншотов из буфера обмена (Ctrl+V).
   * @param {any} event
   */
  function onPaste(event) {
    if (state === "disabled" || attachBtn?.getAttribute("aria-disabled") === "true") return;
    const cd = event?.clipboardData;
    /** @type {any[]} */
    const pastedFiles = [];
    if (cd?.files && cd.files.length > 0) {
      for (const f of Array.from(cd.files)) {
        pastedFiles.push(f);
      }
    } else if (cd?.items && cd.items.length > 0) {
      for (const item of Array.from(cd.items)) {
        if (item && item.kind === "file" && typeof item.getAsFile === "function") {
          const f = item.getAsFile();
          if (f) pastedFiles.push(f);
        }
      }
    }
    if (pastedFiles.length > 0) {
      event.preventDefault?.();
      void addFiles(pastedFiles);
      return;
    }
    const uriList = typeof cd?.getData === "function" ? String(cd.getData("text/uri-list") || "").trim() : "";
    const plainText = typeof cd?.getData === "function" ? String(cd.getData("text/plain") || "") : "";
    const fileUriCandidate = uriList || (plainText.trim().startsWith("file://") ? plainText.trim() : "");
    if (fileUriCandidate.startsWith("file://")) {
      event.preventDefault?.();
      const uris = fileUriCandidate
        .split(/\r?\n/)
        .map((line) => line.trim())
        .filter((line) => line.startsWith("file://"));
      const clipFiles = uris.map((uri) => ({
        name: uri,
        size: 1,
        type: "application/octet-stream",
        isClip: true,
        bytes: new Uint8Array([0]),
      }));
      if (clipFiles.length > 0) {
        void addFiles(clipFiles);
      }
      return;
    }
    if (cd && plainText === "") {
      event.preventDefault?.();
      void addFiles([
        {
          name: "clipboard.png",
          size: 1,
          type: "image/png",
          isClip: true,
          bytes: new Uint8Array([0]),
        },
      ]);
    }
  }

  /**
   * Drag-and-drop файлов в область композера.
   * @param {any} event
   */
  function onDrop(event) {
    if (state === "disabled" || attachBtn?.getAttribute("aria-disabled") === "true") return;
    const dt = event?.dataTransfer;
    if (dt?.files && dt.files.length > 0) {
      event.preventDefault?.();
      void addFiles(dt.files);
    }
  }

  input.addEventListener("input", onInput);
  input.addEventListener("keydown", onKeydown);
  input.addEventListener("paste", onPaste);
  form.addEventListener("dragover", (/** @type {any} */ e) => {
    if (e?.dataTransfer) e.preventDefault?.();
  });
  form.addEventListener("drop", onDrop);
  replyCancel.addEventListener("click", () => clearReply());

  form.setAttribute("data-reply", "false");
  form.setAttribute("data-limit", "none");
  form.setAttribute("data-attach-state", "none");
  setState("empty");

  return {
    /** @returns {ComposerState} */
    get state() {
      return state;
    },
    /** @returns {AttachState} */
    get attachState() {
      return attachState;
    },
    /** @returns {string} */
    get replyAuthor() {
      return reply?.author ?? "";
    },
    /** @returns {string} */
    get replyTo() {
      return reply?.id ?? "";
    },
    /** @returns {AttachmentItem[]} */
    get attachments() {
      return [...attachments];
    },
    /** @returns {Array<{ id: string, filename: string, uploaded_filename: string }>} */
    getUploadedAttachments() {
      return attachments
        .filter((a) => a.state === "ready" && Boolean(a.uploaded_filename))
        .map((a) => ({
          id: a.id,
          filename: a.name,
          uploaded_filename: /** @type {string} */ (a.uploaded_filename),
        }));
    },
    setDisabled,
    applyChannelPolicy,
    setReply,
    clearReply,
    refresh,
    insertText,
    addFiles,
    removeAttachment,
    clearAttachments,
    openGiftModal,
    closeGiftModal,
    /** @param {HTMLElement | null} [fallback] */
    focus(fallback = null) {
      const target = /** @type {any} */ (input);
      if (typeof target.focus === "function") target.focus();
      else if (fallback && typeof /** @type {any} */ (fallback).focus === "function") {
        /** @type {any} */ (fallback).focus();
      }
    },
    destroy() {
      clearAttachments();
      clearReply();
      if (typeof input.removeEventListener === "function") {
        input.removeEventListener("input", onInput);
        input.removeEventListener("keydown", onKeydown);
      }
    },
  };
}

/** @typedef {ReturnType<typeof createComposer>} Composer */

/** Имя IPC-команды отправки сообщения в канал. */
const SEND_MESSAGE_COMMAND = "send_message";

/**
 * Монтирует контроллер очереди вложений (`createAttachmentsController`) в DOM-зону композера:
 * - подключает кнопку `#composer-attach-btn` (`[data-action="attach"]`) и скрытый `<input type="file">`;
 * - подключает drag-and-drop и `paste` файлов на зону `#composer` и поле `#composer-input`;
 * - рендерит полку `#attachment-shelf` над полем ввода композера с карточками, спойлером `SPOILER_`,
 *   полем `alt text` (`description`) и кнопкой удаления из очереди.
 *
 * @param {{
 *   document?: any,
 *   composerEl?: any,
 *   form?: any,
 *   inputEl?: any,
 *   input?: any,
 *   attachBtnEl?: any,
 *   attachBtn?: any,
 *   button?: any,
 *   fileInputEl?: any,
 *   fileInput?: any,
 *   shelfEl?: any,
 *   shelf?: any,
 *   dropZone?: any,
 *   channelId?: string,
 *   getChannelId?: () => string,
 *   maxFiles?: number,
 *   maxFileSize?: number,
 *   invoke?: ((cmd: string, args?: any) => Promise<any>) | null,
 *   invokeFn?: ((cmd: string, args?: any) => Promise<any>) | null,
 *   upload?: ((files: any[], context: { channelId: string }) => Promise<any>) | null,
 *   onChange?: ((items: any[], state: any) => void) | null,
 *   onError?: ((error: any) => void) | null,
 *   attachmentsController?: ReturnType<typeof createAttachmentsController> | null,
 * }} [options]
 * @returns {ReturnType<typeof createAttachmentsController>}
 */
export function mountComposerAttachments(options = {}) {
  if (options.attachmentsController) {
    return options.attachmentsController;
  }
  const doc =
    options.document ??
    options.composerEl?.ownerDocument ??
    options.form?.ownerDocument ??
    options.inputEl?.ownerDocument ??
    options.input?.ownerDocument ??
    (typeof document !== "undefined" ? document : null);

  const composerEl =
    options.composerEl ??
    options.form ??
    doc?.getElementById?.("composer") ??
    null;

  const inputEl =
    options.inputEl ??
    options.input ??
    composerEl?.querySelector?.("#composer-input") ??
    doc?.getElementById?.("composer-input") ??
    null;

  const attachBtn =
    options.attachBtnEl ??
    options.attachBtn ??
    options.button ??
    composerEl?.querySelector?.("#composer-attach-btn") ??
    composerEl?.querySelector?.('[data-action="attach"]') ??
    doc?.getElementById?.("composer-attach-btn") ??
    null;

  const fileInput =
    options.fileInputEl ??
    options.fileInput ??
    composerEl?.querySelector?.("#composer-file-input") ??
    doc?.getElementById?.("composer-file-input") ??
    null;

  const shelfEl =
    options.shelfEl ??
    options.shelf ??
    composerEl?.querySelector?.("#attachment-shelf") ??
    doc?.getElementById?.("attachment-shelf") ??
    null;

  return createAttachmentsController({
    ...options,
    document: doc,
    composerEl,
    inputEl,
    attachBtn,
    fileInput,
    shelfEl,
    dropZone: options.dropZone ?? composerEl,
  });
}

/**
 * Создаёт композер сообщений с интегрированной очередью вложений `createAttachmentsController` (FE-P0-05 / task-151):
 * - кнопка `#composer-attach-btn` (`file input`), drag-and-drop и `paste` на зону композера добавляют файлы в очередь
 *   (до 10 файлов, `<= 25 МБ` на файл);
 * - над `#composer-input` рендерится полка `#attachment-shelf` с карточками файлов, переключателем `SPOILER_`,
 *   вводом описания (`alt text`) и кнопкой удаления;
 * - при отправке сообщения вызывает `attachments.uploadAll()` -> `upload_attachments` и передаёт массив готовых
 *   метаданных `attachments` вместе с текстом (и разрешает отправку с пустым текстом, если в очереди есть хотя бы 1 вложение);
 * - очищает полку вложений после успешной отправки и при `destroy()`.
 *
 * @param {{
 *   document?: any,
 *   composerEl?: any,
 *   form?: any,
 *   inputEl?: any,
 *   input?: any,
 *   sendBtnEl?: any,
 *   sendBtn?: any,
 *   attachBtnEl?: any,
 *   attachBtn?: any,
 *   fileInputEl?: any,
 *   fileInput?: any,
 *   shelfEl?: any,
 *   shelf?: any,
 *   dropZone?: any,
 *   channelId?: string,
 *   getChannelId?: () => string,
 *   maxFiles?: number,
 *   maxFileSize?: number,
 *   invoke?: ((cmd: string, args?: any) => Promise<any>) | null,
 *   invokeFn?: ((cmd: string, args?: any) => Promise<any>) | null,
 *   upload?: ((files: any[], context: { channelId: string }) => Promise<any>) | null,
 *   onSendMessage?: ((payload: { channelId: string, channel_id: string, content: string, text: string, attachments: any[] }) => Promise<any> | any) | null,
 *   onSubmit?: ((content: string, attachments: any[], payload: any) => Promise<any> | any) | null,
 *   onChange?: ((items: any[], state: any) => void) | null,
 *   onError?: ((error: any) => void) | null,
 *   attachmentsController?: ReturnType<typeof createAttachmentsController> | null,
 * }} [options]
 */
export function createMessageComposer(options = {}) {
  const doc =
    options.document ??
    options.composerEl?.ownerDocument ??
    options.form?.ownerDocument ??
    options.inputEl?.ownerDocument ??
    options.input?.ownerDocument ??
    (typeof document !== "undefined" ? document : null);

  const composerEl =
    options.composerEl ??
    options.form ??
    doc?.getElementById?.("composer") ??
    null;

  const inputEl =
    options.inputEl ??
    options.input ??
    composerEl?.querySelector?.("#composer-input") ??
    doc?.getElementById?.("composer-input") ??
    null;

  const sendBtnEl =
    options.sendBtnEl ??
    options.sendBtn ??
    composerEl?.querySelector?.("#composer-send") ??
    doc?.getElementById?.("composer-send") ??
    null;

  const attachments = mountComposerAttachments({
    ...options,
    document: doc,
    composerEl,
    inputEl,
  });

  let destroyed = false;
  let submitting = false;

  /**
   * Выполняет отправку сообщения: при наличии файлов в очереди сначала вызывает `attachments.uploadAll()`,
   * затем передаёт текст и готовые метаданные `attachments` в `onSendMessage` / `onSubmit` / IPC,
   * и очищает полку вложений и поле ввода после успеха.
   *
   * @returns {Promise<boolean>}
   */
  async function submit() {
    if (destroyed || submitting) return false;
    if (
      inputEl?.disabled ||
      inputEl?.getAttribute?.("aria-disabled") === "true" ||
      composerEl?.getAttribute?.("data-state") === "disabled"
    ) {
      return false;
    }

    const rawText = typeof inputEl?.value === "string" ? inputEl.value : "";
    const trimmedText = rawText.trim();
    const queuedCount = attachments.getCount();

    // Пустое сообщение без вложений не отправляется; пустой текст при >= 1 вложении разрешён
    if (trimmedText === "" && queuedCount === 0) {
      return false;
    }

    submitting = true;
    try {
      const channelId = options.getChannelId?.() ?? options.channelId ?? "default";
      const uploadedAttachments =
        queuedCount > 0 ? await attachments.uploadAll(channelId) : [];

      const payload = {
        channelId,
        channel_id: channelId,
        content: trimmedText,
        text: trimmedText,
        attachments: uploadedAttachments,
      };

      if (typeof options.onSendMessage === "function") {
        await options.onSendMessage(payload);
      } else if (typeof options.onSubmit === "function") {
        await options.onSubmit(trimmedText, uploadedAttachments, payload);
      } else {
        const rpc =
          options.invokeFn ??
          options.invoke ??
          (typeof globalThis !== "undefined"
            ? /** @type {any} */ (globalThis).__TAURI__?.core?.invoke
            : null);
        if (typeof rpc === "function") {
          await rpc(SEND_MESSAGE_COMMAND, payload);
        }
      }

      if (inputEl && typeof inputEl.value === "string") {
        inputEl.value = "";
      }
      attachments.clear();
      submitting = false;
      return true;
    } catch (error) {
      submitting = false;
      options.onError?.(error);
      return false;
    }
  }

  /** @type {Array<() => void>} */
  const cleanups = [];

  /**
   * @param {any} target
   * @param {string} type
   * @param {(event: any) => void} handler
   */
  function listen(target, type, handler) {
    if (!target || typeof target.addEventListener !== "function") return;
    target.addEventListener(type, handler);
    cleanups.push(() => {
      if (typeof target.removeEventListener === "function") {
        target.removeEventListener(type, handler);
      }
    });
  }

  if (composerEl) {
    listen(composerEl, "submit", (event) => {
      if (destroyed) return;
      event?.preventDefault?.();
      void submit();
    });
  }

  if (inputEl) {
    listen(inputEl, "keydown", (event) => {
      if (destroyed) return;
      if (event?.key === "Enter" && !event?.shiftKey) {
        event?.preventDefault?.();
        void submit();
      }
    });
  }

  if (sendBtnEl && (!composerEl || sendBtnEl.type !== "submit")) {
    listen(sendBtnEl, "click", (event) => {
      if (destroyed) return;
      event?.preventDefault?.();
      void submit();
    });
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    while (cleanups.length > 0) {
      const fn = cleanups.pop();
      try {
        fn?.();
      } catch {
        // best-effort cleanup
      }
    }
    attachments.destroy();
  }

  return {
    attachments,
    get attachmentsController() {
      return attachments;
    },
    get shelfEl() {
      return attachments.shelfEl;
    },
    get isSubmitting() {
      return submitting;
    },
    addFiles: attachments.addFiles,
    addFile: attachments.addFile,
    removeFile: attachments.removeFile,
    toggleSpoiler: attachments.toggleSpoiler,
    setDescription: attachments.setDescription,
    uploadAll: attachments.uploadAll,
    clearAttachments: attachments.clear,
    submit,
    sendMessage: submit,
    destroy,
  };
}
