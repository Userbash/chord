// @ts-check
/**
 * T-227 (FE-P1-23 / RW-40 / G2) & TASK-620: унифицированный контроллер контекстного
 * меню и клавиатурной навигации для сообщений, каналов и участников (`context-menu.js`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` (§5 п.3–4 Menu / ContextMenu:
 *   `menu_c1e9c4`, `role="menu"`, `role="menuitem"`, `colorDanger`, `separator_c1e9c4`,
 *   `CONTEXT_MENU_OPEN` / `CONTEXT_MENU_CLOSE`);
 * - `/home/sanya/re-deep/ui/01-msg-menu-items.json`, `01-msg-menu.html`;
 * - `/home/sanya/re-deep/gateway-schema.md` (Discord Permissions v9 bitflags).
 *
 * @module ui/features/context-menu
 */

/**
 * Отступ контекстного меню от краёв viewport (px).
 * @type {8}
 */
const VIEWPORT_EDGE_PADDING_PX = 8;

/**
 * Ширина контекстного меню по умолчанию для расчёта границ viewport (px).
 * @type {220}
 */
const DEFAULT_MENU_WIDTH_PX = 220;

/**
 * Высота контекстного меню по умолчанию для расчёта границ viewport (px).
 * @type {240}
 */
const DEFAULT_MENU_HEIGHT_PX = 240;

/**
 * Ширина viewport по умолчанию (px).
 * @type {1280}
 */
const DEFAULT_VIEWPORT_WIDTH_PX = 1280;

/**
 * Высота viewport по умолчанию (px).
 * @type {720}
 */
const DEFAULT_VIEWPORT_HEIGHT_PX = 720;

/**
 * Шаг изменения громкости при нажатии клавиш ArrowLeft / ArrowRight (%).
 * @type {5}
 */
const KEYBOARD_VOLUME_STEP = 5;

const DECIMAL_BITFIELD_RE = /^\d+$/;

/**
 * Битовые флаги прав Discord API v9 для фильтрации контекстного меню.
 */
export const CONTEXT_MENU_PERMISSION_FLAGS = Object.freeze({
  ADMINISTRATOR: 1n << 3n,
  VIEW_CHANNEL: 1n << 10n,
  SEND_MESSAGES: 1n << 11n,
  MANAGE_MESSAGES: 1n << 13n,
  READ_MESSAGE_HISTORY: 1n << 16n,
  MENTION_EVERYONE: 1n << 17n,
  CONNECT: 1n << 20n,
  SPEAK: 1n << 21n,
  MUTE_MEMBERS: 1n << 22n,
  DEAFEN_MEMBERS: 1n << 23n,
  MOVE_MEMBERS: 1n << 24n,
  MANAGE_ROLES: 1n << 28n,
  MODERATE_MEMBERS: 1n << 40n,
});

/**
 * @typedef {"message" | "channel" | "user" | "member"} ContextMenuTargetType
 */

/**
 * @typedef {object} ContextMenuItem
 * @property {string} id
 * @property {string} label
 * @property {boolean} danger
 * @property {"item" | "slider" | "separator"} [type]
 * @property {number} [min]
 * @property {number} [max]
 * @property {number} [value]
 * @property {number} [step]
 */

/**
 * @typedef {object} ContextMenuPermissionObject
 * @property {boolean} [reply]
 * @property {boolean} [edit]
 * @property {boolean} [pin]
 * @property {boolean} [delete]
 * @property {boolean} [copyText]
 * @property {boolean} [copyId]
 * @property {boolean} [profile]
 * @property {boolean} [mention]
 * @property {boolean} [message]
 * @property {boolean} [volume]
 * @property {boolean} [mute]
 * @property {boolean} [manageMessages]
 * @property {boolean} [canReply]
 * @property {boolean} [canSendMessages]
 * @property {boolean} [canEdit]
 * @property {boolean} [canPin]
 * @property {boolean} [canDelete]
 * @property {boolean} [canManageMessages]
 * @property {boolean} [canCopyText]
 * @property {boolean} [canCopyId]
 * @property {boolean} [canProfile]
 * @property {boolean} [canMention]
 * @property {boolean} [canMessage]
 * @property {boolean} [canVolume]
 * @property {boolean} [canChangeVolume]
 * @property {boolean} [canMute]
 * @property {boolean} [canMuteMembers]
 */

/**
 * @typedef {object} BuildContextMenuItemsOptions
 * @property {ContextMenuTargetType} targetType
 * @property {boolean} [isOwnMessage]
 * @property {boolean} [isSystemMessage]
 * @property {boolean} [isPinned]
 * @property {boolean} [isMuted]
 * @property {boolean} [isSelf]
 * @property {boolean} [isOwner]
 * @property {boolean} [isAdmin]
 * @property {boolean} [canReply]
 * @property {boolean} [canSendMessages]
 * @property {boolean} [canManageMessages]
 * @property {boolean} [canEdit]
 * @property {boolean} [canPin]
 * @property {boolean} [canDelete]
 * @property {boolean} [canCopyText]
 * @property {boolean} [canCopyId]
 * @property {boolean} [canProfile]
 * @property {boolean} [canViewProfile]
 * @property {boolean} [canMention]
 * @property {boolean} [canMessage]
 * @property {boolean} [canVolume]
 * @property {boolean} [canChangeVolume]
 * @property {boolean} [canMute]
 * @property {boolean} [canMuteMembers]
 * @property {bigint | number | string | ContextMenuPermissionObject} [permissions]
 * @property {number} [volume]
 * @property {boolean} [hasVoice]
 * @property {boolean} [unified]
 * @property {string} [editLabel]
 * @property {string} [pinLabel]
 * @property {string} [deleteLabel]
 * @property {string} [copyIdLabel]
 * @property {string} [messageLabel]
 * @property {string} [mentionLabel]
 */

/**
 * @typedef {object} ResolvedContextMenuPermissions
 * @property {boolean} canReply
 * @property {boolean} canEdit
 * @property {boolean} canPin
 * @property {boolean} canDelete
 * @property {boolean} canCopyText
 * @property {boolean} canCopyId
 * @property {boolean} canProfile
 * @property {boolean} canMention
 * @property {boolean} canMessage
 * @property {boolean} canVolume
 * @property {boolean} canMute
 */

/**
 * @typedef {object} ClampMenuCoordinatesInput
 * @property {number} x
 * @property {number} y
 * @property {number} menuWidth
 * @property {number} menuHeight
 * @property {number} viewportWidth
 * @property {number} viewportHeight
 */

/**
 * @typedef {object} OpenContextMenuOptions
 * @property {number} [x]
 * @property {number} [y]
 * @property {number} [menuWidth]
 * @property {number} [menuHeight]
 * @property {number} [viewportWidth]
 * @property {number} [viewportHeight]
 * @property {ContextMenuTargetType} [targetType]
 * @property {string} [targetId]
 * @property {boolean} [isOwnMessage]
 * @property {boolean} [isSystemMessage]
 * @property {boolean} [isPinned]
 * @property {boolean} [isMuted]
 * @property {boolean} [isSelf]
 * @property {boolean} [isOwner]
 * @property {boolean} [isAdmin]
 * @property {boolean} [canReply]
 * @property {boolean} [canSendMessages]
 * @property {boolean} [canManageMessages]
 * @property {boolean} [canEdit]
 * @property {boolean} [canPin]
 * @property {boolean} [canDelete]
 * @property {boolean} [canCopyText]
 * @property {boolean} [canCopyId]
 * @property {boolean} [canProfile]
 * @property {boolean} [canViewProfile]
 * @property {boolean} [canMention]
 * @property {boolean} [canMessage]
 * @property {boolean} [canVolume]
 * @property {boolean} [canChangeVolume]
 * @property {boolean} [canMute]
 * @property {boolean} [canMuteMembers]
 * @property {bigint | number | string | ContextMenuPermissionObject} [permissions]
 * @property {number} [volume]
 * @property {boolean} [hasVoice]
 * @property {boolean} [unified]
 */

/**
 * @typedef {object} ContextMenuSelectionPayload
 * @property {string} actionId
 * @property {string} id
 * @property {string} label
 * @property {boolean} danger
 * @property {"message" | "channel" | "user"} targetType
 * @property {string} targetId
 * @property {number} [volume]
 */

/**
 * @typedef {object} ContextMenuControllerOptions
 * @property {any} container
 * @property {(payload: ContextMenuSelectionPayload) => void} [onSelectAction]
 * @property {(volume: number) => void} [onVolumeChange]
 */

/**
 * Разбирает битовую маску прав Discord из `bigint | number | string`.
 *
 * @param {unknown} raw
 * @returns {bigint | null}
 */
function parsePermissionBitfield(raw) {
  if (typeof raw === "bigint") {
    return raw >= 0n ? raw : null;
  }
  if (typeof raw === "number") {
    if (!Number.isFinite(raw) || raw < 0 || !Number.isInteger(raw)) {
      return null;
    }
    return BigInt(raw);
  }
  if (typeof raw === "string") {
    const trimmed = raw.trim();
    if (!DECIMAL_BITFIELD_RE.test(trimmed)) {
      return null;
    }
    try {
      return BigInt(trimmed);
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Вычисляет эффективные флаги разрешений для действий контекстного меню
 * над сообщениями и участниками.
 *
 * @param {Partial<BuildContextMenuItemsOptions>} [options]
 * @returns {ResolvedContextMenuPermissions}
 */
export function resolveContextMenuPermissions(options = {}) {
  const opts =
    options && typeof options === "object"
      ? options
      : /** @type {Partial<BuildContextMenuItemsOptions>} */ ({});

  const rawPerms = opts.permissions;
  const permObj =
    rawPerms && typeof rawPerms === "object"
      ? /** @type {ContextMenuPermissionObject} */ (rawPerms)
      : null;
  const bitfield = permObj ? null : parsePermissionBitfield(rawPerms);

  const isSystemMessage = Boolean(opts.isSystemMessage);
  const isOwnMessage =
    typeof opts.isOwnMessage === "boolean" ? opts.isOwnMessage : undefined;
  const isSelf = Boolean(opts.isSelf);

  const isAdmin = Boolean(
    opts.isOwner ||
      opts.isAdmin ||
      (bitfield !== null &&
        (bitfield & CONTEXT_MENU_PERMISSION_FLAGS.ADMINISTRATOR) !== 0n),
  );
  const hasManageMessagesBit =
    isAdmin ||
    (bitfield !== null &&
      (bitfield & CONTEXT_MENU_PERMISSION_FLAGS.MANAGE_MESSAGES) !== 0n);
  const hasSendMessagesBit =
    isAdmin ||
    (bitfield !== null &&
      (bitfield &
        (CONTEXT_MENU_PERMISSION_FLAGS.SEND_MESSAGES |
          CONTEXT_MENU_PERMISSION_FLAGS.MANAGE_MESSAGES)) !==
        0n);
  const hasMuteMembersBit =
    isAdmin ||
    (bitfield !== null &&
      (bitfield & CONTEXT_MENU_PERMISSION_FLAGS.MUTE_MEMBERS) !== 0n);

  // 1. Действия над сообщениями: reply, edit, pin, delete
  let canReply = true;
  if (isSystemMessage) {
    canReply = opts.canReply === true;
  } else if (typeof opts.canReply === "boolean") {
    canReply = opts.canReply;
  } else if (typeof permObj?.canReply === "boolean") {
    canReply = permObj.canReply;
  } else if (typeof permObj?.reply === "boolean") {
    canReply = permObj.reply;
  } else if (typeof opts.canSendMessages === "boolean") {
    canReply = opts.canSendMessages;
  } else if (typeof permObj?.canSendMessages === "boolean") {
    canReply = permObj.canSendMessages;
  } else if (bitfield !== null) {
    canReply = hasSendMessagesBit;
  }

  let canEdit = false;
  if (isSystemMessage) {
    canEdit = false;
  } else if (typeof opts.canEdit === "boolean") {
    canEdit = opts.canEdit;
  } else if (typeof permObj?.canEdit === "boolean") {
    canEdit = permObj.canEdit;
  } else if (typeof permObj?.edit === "boolean") {
    canEdit = permObj.edit;
  } else if (isOwnMessage !== undefined) {
    canEdit = isOwnMessage;
  } else if (bitfield !== null || permObj !== null) {
    canEdit = false;
  } else {
    canEdit = true;
  }

  let canPin = false;
  if (isSystemMessage) {
    canPin = false;
  } else if (typeof opts.canPin === "boolean") {
    canPin = opts.canPin;
  } else if (typeof permObj?.canPin === "boolean") {
    canPin = permObj.canPin;
  } else if (typeof permObj?.pin === "boolean") {
    canPin = permObj.pin;
  } else if (typeof opts.canManageMessages === "boolean") {
    canPin = opts.canManageMessages;
  } else if (typeof permObj?.canManageMessages === "boolean") {
    canPin = permObj.canManageMessages;
  } else if (typeof permObj?.manageMessages === "boolean") {
    canPin = permObj.manageMessages;
  } else if (bitfield !== null) {
    canPin = hasManageMessagesBit;
  } else if (isAdmin) {
    canPin = true;
  }

  let canDelete = false;
  if (isSystemMessage) {
    canDelete = false;
  } else if (typeof opts.canDelete === "boolean") {
    canDelete = opts.canDelete;
  } else if (typeof permObj?.canDelete === "boolean") {
    canDelete = permObj.canDelete;
  } else if (typeof permObj?.delete === "boolean") {
    canDelete = permObj.delete;
  } else if (isOwnMessage === true) {
    canDelete = true;
  } else if (typeof opts.canManageMessages === "boolean") {
    canDelete = opts.canManageMessages;
  } else if (typeof permObj?.canManageMessages === "boolean") {
    canDelete = permObj.canManageMessages;
  } else if (typeof permObj?.manageMessages === "boolean") {
    canDelete = permObj.manageMessages;
  } else if (bitfield !== null) {
    canDelete = hasManageMessagesBit;
  } else if (isAdmin) {
    canDelete = true;
  } else if (isOwnMessage === false || permObj !== null) {
    canDelete = false;
  } else {
    canDelete = true;
  }

  const canCopyText =
    opts.canCopyText !== false &&
    permObj?.canCopyText !== false &&
    permObj?.copyText !== false;
  const canCopyId =
    opts.canCopyId !== false &&
    permObj?.canCopyId !== false &&
    permObj?.copyId !== false;

  // 2. Действия над участниками: profile, mention, volume, mute
  let canProfile = true;
  if (typeof opts.canProfile === "boolean") {
    canProfile = opts.canProfile;
  } else if (typeof opts.canViewProfile === "boolean") {
    canProfile = opts.canViewProfile;
  } else if (typeof permObj?.canProfile === "boolean") {
    canProfile = permObj.canProfile;
  } else if (typeof permObj?.profile === "boolean") {
    canProfile = permObj.profile;
  }

  let canMention = true;
  if (typeof opts.canMention === "boolean") {
    canMention = opts.canMention;
  } else if (typeof permObj?.canMention === "boolean") {
    canMention = permObj.canMention;
  } else if (typeof permObj?.mention === "boolean") {
    canMention = permObj.mention;
  } else if (typeof opts.canSendMessages === "boolean") {
    canMention = opts.canSendMessages;
  } else if (typeof permObj?.canSendMessages === "boolean") {
    canMention = permObj.canSendMessages;
  } else if (bitfield !== null) {
    canMention =
      isAdmin ||
      (bitfield &
        (CONTEXT_MENU_PERMISSION_FLAGS.SEND_MESSAGES |
          CONTEXT_MENU_PERMISSION_FLAGS.MENTION_EVERYONE)) !==
        0n;
  }

  let canMessage = true;
  if (isSelf && opts.canMessage !== true) {
    canMessage = false;
  } else if (typeof opts.canMessage === "boolean") {
    canMessage = opts.canMessage;
  } else if (typeof permObj?.canMessage === "boolean") {
    canMessage = permObj.canMessage;
  } else if (typeof permObj?.message === "boolean") {
    canMessage = permObj.message;
  }

  let canVolume = true;
  if (isSelf && opts.canVolume !== true && opts.canChangeVolume !== true) {
    canVolume = false;
  } else if (typeof opts.canVolume === "boolean") {
    canVolume = opts.canVolume;
  } else if (typeof opts.canChangeVolume === "boolean") {
    canVolume = opts.canChangeVolume;
  } else if (typeof permObj?.canVolume === "boolean") {
    canVolume = permObj.canVolume;
  } else if (typeof permObj?.canChangeVolume === "boolean") {
    canVolume = permObj.canChangeVolume;
  } else if (typeof permObj?.volume === "boolean") {
    canVolume = permObj.volume;
  } else if (opts.hasVoice === false) {
    canVolume = false;
  } else if (bitfield !== null) {
    canVolume = bitfield !== 0n;
  }

  let canMute = true;
  if (isSelf && opts.canMute !== true) {
    canMute = false;
  } else if (typeof opts.canMute === "boolean") {
    canMute = opts.canMute;
  } else if (typeof permObj?.canMute === "boolean") {
    canMute = permObj.canMute;
  } else if (typeof permObj?.mute === "boolean") {
    canMute = permObj.mute;
  } else if (typeof opts.canMuteMembers === "boolean") {
    canMute = opts.canMuteMembers;
  } else if (typeof permObj?.canMuteMembers === "boolean") {
    canMute = permObj.canMuteMembers;
  } else if (opts.hasVoice === false) {
    canMute = false;
  } else if (bitfield !== null) {
    canMute = hasMuteMembersBit;
  }

  return {
    canReply,
    canEdit,
    canPin,
    canDelete,
    canCopyText,
    canCopyId,
    canProfile,
    canMention,
    canMessage,
    canVolume,
    canMute,
  };
}

/**
 * Формирует список пунктов контекстного меню для указанного типа цели и флагов прав/состояния.
 *
 * @param {BuildContextMenuItemsOptions} options
 * @returns {ContextMenuItem[]}
 */
export function buildContextMenuItems(options) {
  const opts =
    options && typeof options === "object"
      ? options
      : /** @type {BuildContextMenuItemsOptions} */ ({});
  const targetType = opts.targetType === "member" ? "user" : opts.targetType;
  const isOwnMessage = opts.isOwnMessage;
  const isPinned = Boolean(opts.isPinned);
  const isMuted = Boolean(opts.isMuted);
  const canManageMessages = Boolean(opts.canManageMessages);

  if (targetType === "message") {
    if (opts.unified) {
      const perms = resolveContextMenuPermissions(opts);
      /** @type {ContextMenuItem[]} */
      const items = [];
      if (perms.canReply) {
        items.push({ id: "reply", label: "Ответить", danger: false });
      }
      if (perms.canEdit) {
        items.push({
          id: "edit",
          label: opts.editLabel ?? "Редактировать",
          danger: false,
        });
      }
      if (perms.canPin) {
        items.push({
          id: "pin",
          label: opts.pinLabel ?? (isPinned ? "Открепить" : "Закрепить"),
          danger: false,
        });
      }
      if (perms.canCopyText) {
        items.push({
          id: "copy-text",
          label: "Скопировать текст",
          danger: false,
        });
      }
      if (perms.canCopyId) {
        items.push({
          id: "copy-id",
          label: opts.copyIdLabel ?? "Скопировать ID",
          danger: false,
        });
      }
      if (perms.canDelete) {
        items.push({
          id: "delete",
          label: opts.deleteLabel ?? "Удалить",
          danger: true,
        });
      }
      return items;
    }

    /** @type {ContextMenuItem[]} */
    const items = [
      { id: "reply", label: "Ответить", danger: false },
      { id: "copy-text", label: "Скопировать текст", danger: false },
    ];
    if (canManageMessages || Boolean(opts.canPin)) {
      items.push({
        id: "pin",
        label: isPinned ? "Открепить сообщение" : "Закрепить сообщение",
        danger: false,
      });
    }
    if (Boolean(isOwnMessage) || Boolean(opts.canEdit)) {
      items.push({
        id: "edit",
        label: "Изменить сообщение",
        danger: false,
      });
    }
    if (Boolean(isOwnMessage) || canManageMessages || Boolean(opts.canDelete)) {
      items.push({
        id: "delete",
        label: "Удалить сообщение",
        danger: true,
      });
    }
    items.push({
      id: "copy-id",
      label: "Скопировать ID сообщения",
      danger: false,
    });
    return items;
  }

  if (targetType === "channel") {
    return [
      { id: "mark-read", label: "Отметить как прочитанное", danger: false },
      {
        id: "mute-channel",
        label: isMuted ? "Включить уведомления канала" : "Заглушить канал",
        danger: false,
      },
      { id: "copy-id", label: "Скопировать ID канала", danger: false },
    ];
  }

  if (targetType === "user") {
    const isUnifiedUser = Boolean(
      opts.unified ||
        opts.canMention !== undefined ||
        opts.canMute !== undefined ||
        opts.canMuteMembers !== undefined ||
        opts.canVolume !== undefined ||
        opts.canChangeVolume !== undefined ||
        opts.canProfile !== undefined ||
        opts.permissions !== undefined ||
        opts.volume !== undefined ||
        opts.hasVoice !== undefined ||
        opts.isSelf !== undefined,
    );

    if (isUnifiedUser) {
      const perms = resolveContextMenuPermissions(opts);
      const rawVol =
        typeof opts.volume === "number" && Number.isFinite(opts.volume)
          ? opts.volume
          : 100;
      const vol = Math.max(0, Math.min(200, Math.round(rawVol)));
      /** @type {ContextMenuItem[]} */
      const items = [];
      if (perms.canProfile) {
        items.push({ id: "profile", label: "Профиль", danger: false });
      }
      if (perms.canMention) {
        items.push({
          id: "mention-user",
          label: opts.mentionLabel ?? "Упомянуть",
          danger: false,
        });
      }
      if (perms.canMessage) {
        items.push({
          id: "message-user",
          label: opts.messageLabel ?? "Написать в ЛС",
          danger: false,
        });
      }
      if (perms.canMute) {
        items.push({
          id: "mute-user",
          label: isMuted ? "Включить звук" : "Заглушить",
          danger: false,
        });
      }
      if (perms.canVolume) {
        items.push({
          id: "user-volume",
          label: "Громкость",
          danger: false,
          type: "slider",
          min: 0,
          max: 200,
          value: vol,
        });
      }
      if (perms.canCopyId) {
        items.push({
          id: "copy-id",
          label: opts.copyIdLabel ?? "Скопировать ID",
          danger: false,
        });
      }
      return items;
    }

    return [
      { id: "profile", label: "Профиль", danger: false },
      { id: "message-user", label: "Написать сообщение", danger: false },
      { id: "copy-id", label: "Скопировать ID пользователя", danger: false },
    ];
  }

  return [];
}

/**
 * Формирует унифицированное контекстное меню для сообщения (TASK-620).
 *
 * @param {Omit<BuildContextMenuItemsOptions, "targetType">} [options]
 * @returns {ContextMenuItem[]}
 */
export function buildMessageContextMenuItems(options = {}) {
  return buildContextMenuItems({
    targetType: "message",
    unified: true,
    ...options,
  });
}

/**
 * Формирует унифицированное контекстное меню для пользователя / участника (TASK-620).
 *
 * @param {Omit<BuildContextMenuItemsOptions, "targetType">} [options]
 * @returns {ContextMenuItem[]}
 */
export function buildUserContextMenuItems(options = {}) {
  return buildContextMenuItems({
    targetType: "user",
    unified: true,
    ...options,
  });
}

/**
 * Алиас `buildUserContextMenuItems` для контекстного меню участника сервера (TASK-620).
 *
 * @param {Omit<BuildContextMenuItemsOptions, "targetType">} [options]
 * @returns {ContextMenuItem[]}
 */
export function buildMemberContextMenuItems(options = {}) {
  return buildUserContextMenuItems(options);
}

/**
 * Ограничивает координаты контекстного меню пределами viewport с отступом 8px:
 * - `x` зажимается в `[0, Math.max(0, viewportWidth - menuWidth - 8)]`,
 * - `y` зажимается в `[0, Math.max(0, viewportHeight - menuHeight - 8)]`.
 *
 * @param {ClampMenuCoordinatesInput} input
 * @returns {{ x: number, y: number }}
 */
export function clampMenuCoordinates(input) {
  const src =
    input && typeof input === "object"
      ? input
      : /** @type {ClampMenuCoordinatesInput} */ ({});
  const rawX = typeof src.x === "number" && Number.isFinite(src.x) ? src.x : 0;
  const rawY = typeof src.y === "number" && Number.isFinite(src.y) ? src.y : 0;
  const menuWidth =
    typeof src.menuWidth === "number" &&
    Number.isFinite(src.menuWidth) &&
    src.menuWidth >= 0
      ? src.menuWidth
      : 0;
  const menuHeight =
    typeof src.menuHeight === "number" &&
    Number.isFinite(src.menuHeight) &&
    src.menuHeight >= 0
      ? src.menuHeight
      : 0;
  const viewportWidth =
    typeof src.viewportWidth === "number" &&
    Number.isFinite(src.viewportWidth) &&
    src.viewportWidth >= 0
      ? src.viewportWidth
      : 0;
  const viewportHeight =
    typeof src.viewportHeight === "number" &&
    Number.isFinite(src.viewportHeight) &&
    src.viewportHeight >= 0
      ? src.viewportHeight
      : 0;

  const maxX = Math.max(0, viewportWidth - menuWidth - VIEWPORT_EDGE_PADDING_PX);
  const maxY = Math.max(
    0,
    viewportHeight - menuHeight - VIEWPORT_EDGE_PADDING_PX,
  );

  return {
    x: Math.min(Math.max(0, rawX), maxX),
    y: Math.min(Math.max(0, rawY), maxY),
  };
}

/**
 * Проверяет, содержится ли дочерний узел внутри родительского элемента (DOM / FakeElement).
 *
 * @param {any} parent
 * @param {any} child
 * @returns {boolean}
 */
function containsElement(parent, child) {
  if (!parent || !child) return false;
  if (typeof parent.contains === "function") {
    return Boolean(parent.contains(child));
  }
  let cur = child;
  while (cur) {
    if (cur === parent) return true;
    cur = cur.parentNode;
  }
  return false;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any, addEventListener?: any, removeEventListener?: any, body?: any }}
 */
function resolveDocument(containerEl) {
  if (!containerEl || typeof containerEl !== "object") {
    throw new TypeError("Container element must be a valid DOM object");
  }
  if (
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError(
    "Container element must provide ownerDocument.createElement",
  );
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
 * Читает булев data-атрибут вида `data-foo="true" | "false"`.
 *
 * @param {any} node
 * @param {string} attrName
 * @returns {boolean | undefined}
 */
function readBooleanAttr(node, attrName) {
  if (!node || typeof node.getAttribute !== "function") return undefined;
  const val = node.getAttribute(attrName);
  if (val === "true") return true;
  if (val === "false") return false;
  return undefined;
}

/**
 * Извлекает параметры контекстного меню из DOM-узла (сообщения, участника или канала),
 * поднимаясь вверх по дереву от `target` до `boundary`.
 *
 * @param {any} target
 * @param {any} [boundary]
 * @returns {Partial<OpenContextMenuOptions>}
 */
function extractTargetOptionsFromDom(target, boundary) {
  /** @type {any} */
  let cur = target;
  while (cur && typeof cur.getAttribute === "function") {
    const explicitType =
      cur.getAttribute("data-target-type") ??
      cur.getAttribute("data-context-type");
    const msgId = cur.getAttribute("data-message-id");
    const memberId =
      cur.getAttribute("data-member-id") ?? cur.getAttribute("data-user-id");
    const channelId = cur.getAttribute("data-channel-id");

    /** @type {ContextMenuTargetType | null} */
    let detectedType = null;
    let detectedId = "";

    if (
      explicitType === "message" ||
      explicitType === "user" ||
      explicitType === "member" ||
      explicitType === "channel"
    ) {
      detectedType = explicitType;
      detectedId =
        cur.getAttribute("data-target-id") ??
        msgId ??
        memberId ??
        channelId ??
        "";
    } else if (msgId !== null && msgId !== undefined) {
      detectedType = "message";
      detectedId = String(msgId);
    } else if (memberId !== null && memberId !== undefined) {
      detectedType = "user";
      detectedId = String(memberId);
    } else if (channelId !== null && channelId !== undefined) {
      detectedType = "channel";
      detectedId = String(channelId);
    }

    if (detectedType) {
      /** @type {Partial<OpenContextMenuOptions>} */
      const out = {
        targetType: detectedType,
        targetId: detectedId,
        unified: detectedType === "message" || detectedType === "user" || detectedType === "member",
      };

      const isOwnMessage = readBooleanAttr(cur, "data-own-message");
      if (isOwnMessage !== undefined) out.isOwnMessage = isOwnMessage;

      const isSystemMessage = readBooleanAttr(cur, "data-system-message");
      if (isSystemMessage !== undefined) out.isSystemMessage = isSystemMessage;

      const isPinned = readBooleanAttr(cur, "data-pinned");
      if (isPinned !== undefined) out.isPinned = isPinned;

      const isMuted = readBooleanAttr(cur, "data-muted");
      if (isMuted !== undefined) out.isMuted = isMuted;

      const isSelf = readBooleanAttr(cur, "data-self");
      if (isSelf !== undefined) out.isSelf = isSelf;

      const hasVoice = readBooleanAttr(cur, "data-has-voice");
      if (hasVoice !== undefined) out.hasVoice = hasVoice;

      const canReply = readBooleanAttr(cur, "data-can-reply");
      if (canReply !== undefined) out.canReply = canReply;

      const canEdit = readBooleanAttr(cur, "data-can-edit");
      if (canEdit !== undefined) out.canEdit = canEdit;

      const canPin = readBooleanAttr(cur, "data-can-pin");
      if (canPin !== undefined) out.canPin = canPin;

      const canDelete = readBooleanAttr(cur, "data-can-delete");
      if (canDelete !== undefined) out.canDelete = canDelete;

      const canManageMessages = readBooleanAttr(
        cur,
        "data-can-manage-messages",
      );
      if (canManageMessages !== undefined) {
        out.canManageMessages = canManageMessages;
      }

      const canProfile = readBooleanAttr(cur, "data-can-profile");
      if (canProfile !== undefined) out.canProfile = canProfile;

      const canMention = readBooleanAttr(cur, "data-can-mention");
      if (canMention !== undefined) out.canMention = canMention;

      const canVolume = readBooleanAttr(cur, "data-can-volume");
      if (canVolume !== undefined) out.canVolume = canVolume;

      const canMute = readBooleanAttr(cur, "data-can-mute");
      if (canMute !== undefined) out.canMute = canMute;

      const rawVolume = cur.getAttribute("data-volume");
      if (rawVolume !== null && rawVolume !== undefined && rawVolume !== "") {
        const numVol = Number(rawVolume);
        if (Number.isFinite(numVol)) {
          out.volume = numVol;
        }
      }

      const rawPerms = cur.getAttribute("data-permissions");
      if (rawPerms !== null && rawPerms !== undefined && rawPerms !== "") {
        out.permissions = rawPerms;
      }

      return out;
    }

    if (boundary && cur === boundary) break;
    cur = cur.parentNode;
  }
  return {};
}

/**
 * Создаёт контроллер контекстного меню с поддержкой клавиатурной навигации,
 * фильтрации по правам, слайдера громкости (0..200%) и закрытия по клику вне меню.
 *
 * @param {ContextMenuControllerOptions} options
 */
export function createContextMenuController(options) {
  const opts =
    options && typeof options === "object"
      ? options
      : /** @type {ContextMenuControllerOptions} */ ({});
  const container = opts.container;
  const doc = resolveDocument(container);
  const onSelectAction =
    typeof opts.onSelectAction === "function" ? opts.onSelectAction : null;
  const onVolumeChange =
    typeof opts.onVolumeChange === "function" ? opts.onVolumeChange : null;

  /** @type {ContextMenuItem[]} */
  let currentItems = [];
  /** @type {any[]} */
  let itemElements = [];
  /** @type {"message" | "channel" | "user" | null} */
  let currentTargetType = null;
  let currentTargetId = "";
  let focusedIndex = -1;
  let openState = false;
  let activeVolume = 100;
  let outsideListenerAttached = false;

  const menuEl = doc.createElement("div");
  menuEl.className = "context-menu";
  menuEl.setAttribute("data-context-menu", "");
  menuEl.setAttribute("role", "menu");
  menuEl.setAttribute("aria-label", "Контекстное меню");
  menuEl.setAttribute("tabindex", "-1");
  menuEl.setAttribute("data-open", "false");
  menuEl.setAttribute("aria-hidden", "true");
  menuEl.setAttribute("hidden", "");

  /**
   * Закрывает меню при клике вне его пределов.
   *
   * @param {any} ev
   */
  function handleOutsideClick(ev) {
    if (!openState) return;
    const target = ev?.target;
    if (target && containsElement(menuEl, target)) {
      return;
    }
    close();
  }

  function attachOutsideListener() {
    if (outsideListenerAttached) return;
    const eventRoot =
      doc && typeof doc.addEventListener === "function"
        ? doc
        : doc?.body && typeof doc.body.addEventListener === "function"
          ? doc.body
          : container;
    if (eventRoot && typeof eventRoot.addEventListener === "function") {
      eventRoot.addEventListener("click", handleOutsideClick);
      outsideListenerAttached = true;
    }
  }

  function detachOutsideListener() {
    if (!outsideListenerAttached) return;
    const eventRoot =
      doc && typeof doc.removeEventListener === "function"
        ? doc
        : doc?.body && typeof doc.body.removeEventListener === "function"
          ? doc.body
          : container;
    if (eventRoot && typeof eventRoot.removeEventListener === "function") {
      eventRoot.removeEventListener("click", handleOutsideClick);
    }
    outsideListenerAttached = false;
  }

  /**
   * Синхронизирует DOM-атрибуты фокуса на пунктах меню и переводит фокус на активный элемент.
   *
   * @param {number} nextIndex
   */
  function applyFocusAt(nextIndex) {
    if (!openState || currentItems.length === 0) {
      focusedIndex = -1;
      return;
    }
    const len = currentItems.length;
    focusedIndex = ((nextIndex % len) + len) % len;
    for (let i = 0; i < itemElements.length; i += 1) {
      const el = itemElements[i];
      const isFocused = i === focusedIndex;
      el.setAttribute("tabindex", isFocused ? "0" : "-1");
      el.setAttribute("data-focused", isFocused ? "true" : "false");
      if (isFocused && typeof el.focus === "function") {
        el.focus();
      }
    }
  }

  /**
   * Циклически сдвигает фокус по пунктам открытого меню на `delta`.
   *
   * @param {number} delta
   * @returns {number}
   */
  function moveFocus(delta) {
    if (!openState || currentItems.length === 0) {
      return -1;
    }
    const step =
      typeof delta === "number" && Number.isFinite(delta)
        ? Math.trunc(delta)
        : 0;
    const base = focusedIndex >= 0 ? focusedIndex : 0;
    applyFocusAt(base + step);
    return focusedIndex;
  }

  /**
   * Закрывает контекстное меню и очищает список пунктов.
   */
  function close() {
    detachOutsideListener();
    openState = false;
    currentItems = [];
    itemElements = [];
    currentTargetType = null;
    currentTargetId = "";
    focusedIndex = -1;
    replaceElementChildren(menuEl, []);
    menuEl.setAttribute("data-open", "false");
    menuEl.setAttribute("aria-hidden", "true");
    menuEl.setAttribute("hidden", "");
    menuEl.removeAttribute("data-target-type");
    menuEl.removeAttribute("data-target-id");
  }

  /**
   * Активирует пункт меню по индексу, вызывает `onSelectAction` и закрывает меню.
   *
   * @param {number} index
   * @returns {ContextMenuSelectionPayload | null}
   */
  function selectItemAt(index) {
    if (
      !openState ||
      index < 0 ||
      index >= currentItems.length ||
      !currentTargetType
    ) {
      return null;
    }
    const item = currentItems[index];
    if (!item || item.type === "separator") {
      return null;
    }
    /** @type {ContextMenuSelectionPayload} */
    const payload = {
      actionId: item.id,
      id: item.id,
      label: item.label,
      danger: item.danger,
      targetType: currentTargetType,
      targetId: currentTargetId,
      volume: item.id === "user-volume" ? activeVolume : undefined,
    };
    close();
    if (onSelectAction) {
      onSelectAction(payload);
    }
    return payload;
  }

  /**
   * Активирует пункт меню по его идентификатору `actionId`.
   *
   * @param {string} actionId
   * @returns {ContextMenuSelectionPayload | null}
   */
  function selectAction(actionId) {
    if (!openState || !actionId) return null;
    const idx = currentItems.findIndex((item) => item.id === actionId);
    if (idx === -1) return null;
    return selectItemAt(idx);
  }

  /**
   * Активирует текущий сфокусированный пункт меню.
   *
   * @returns {ContextMenuSelectionPayload | null}
   */
  function activateFocusedItem() {
    return selectItemAt(focusedIndex);
  }

  /**
   * Программно устанавливает уровень громкости пользователя (0..200%).
   *
   * @param {number} volume
   * @returns {number}
   */
  function setVolume(volume) {
    const num =
      typeof volume === "number" && Number.isFinite(volume) ? volume : 100;
    activeVolume = Math.max(0, Math.min(200, Math.round(num)));
    const sliderInput = menuEl.querySelector("[data-volume-slider]");
    if (sliderInput) {
      /** @type {any} */ (sliderInput).value = String(activeVolume);
      sliderInput.setAttribute("aria-valuenow", String(activeVolume));
    }
    const valDisplay = menuEl.querySelector("[data-volume-value]");
    if (valDisplay) {
      valDisplay.textContent = `${activeVolume}%`;
    }
    if (onVolumeChange) {
      onVolumeChange(activeVolume);
    }
    return activeVolume;
  }

  /**
   * Возвращает текущий уровень громкости пользователя.
   *
   * @returns {number}
   */
  function getVolume() {
    return activeVolume;
  }

  /**
   * Открывает контекстное меню в заданных координатах для указанной цели.
   *
   * @param {OpenContextMenuOptions} openOptions
   * @returns {boolean}
   */
  function open(openOptions) {
    const cfg =
      openOptions && typeof openOptions === "object"
        ? openOptions
        : /** @type {OpenContextMenuOptions} */ ({});
    const rawTargetType = cfg.targetType;
    const normalizedTargetType =
      rawTargetType === "member" ? "user" : rawTargetType;

    const builtItems = buildContextMenuItems({
      targetType: /** @type {ContextMenuTargetType} */ (normalizedTargetType),
      isOwnMessage: cfg.isOwnMessage,
      isSystemMessage: cfg.isSystemMessage,
      isPinned: cfg.isPinned,
      isMuted: cfg.isMuted,
      isSelf: cfg.isSelf,
      isOwner: cfg.isOwner,
      isAdmin: cfg.isAdmin,
      canReply: cfg.canReply,
      canSendMessages: cfg.canSendMessages,
      canManageMessages: cfg.canManageMessages,
      canEdit: cfg.canEdit,
      canPin: cfg.canPin,
      canDelete: cfg.canDelete,
      canCopyText: cfg.canCopyText,
      canCopyId: cfg.canCopyId,
      canProfile: cfg.canProfile,
      canViewProfile: cfg.canViewProfile,
      canMention: cfg.canMention,
      canMessage: cfg.canMessage,
      canVolume: cfg.canVolume,
      canChangeVolume: cfg.canChangeVolume,
      canMute: cfg.canMute,
      canMuteMembers: cfg.canMuteMembers,
      permissions: cfg.permissions,
      volume: cfg.volume,
      hasVoice: cfg.hasVoice,
      unified: cfg.unified,
    });
    if (builtItems.length === 0 || !normalizedTargetType) {
      close();
      return false;
    }

    const coords = clampMenuCoordinates({
      x: cfg.x ?? 0,
      y: cfg.y ?? 0,
      menuWidth: cfg.menuWidth ?? DEFAULT_MENU_WIDTH_PX,
      menuHeight: cfg.menuHeight ?? DEFAULT_MENU_HEIGHT_PX,
      viewportWidth: cfg.viewportWidth ?? DEFAULT_VIEWPORT_WIDTH_PX,
      viewportHeight: cfg.viewportHeight ?? DEFAULT_VIEWPORT_HEIGHT_PX,
    });

    currentItems = builtItems;
    currentTargetType = normalizedTargetType;
    currentTargetId =
      cfg.targetId !== undefined && cfg.targetId !== null
        ? String(cfg.targetId)
        : "";
    activeVolume =
      typeof cfg.volume === "number" && Number.isFinite(cfg.volume)
        ? Math.max(0, Math.min(200, Math.round(cfg.volume)))
        : 100;
    openState = true;

    itemElements = currentItems.map((item, idx) => {
      if (item.type === "separator") {
        const sep = doc.createElement("div");
        sep.className = "context-menu-separator";
        sep.setAttribute("role", "separator");
        sep.setAttribute("data-context-separator", "");
        return sep;
      }

      if (item.type === "slider" || item.id === "user-volume") {
        const wrap = doc.createElement("div");
        wrap.setAttribute("data-context-item", item.id);
        wrap.setAttribute("data-action-id", item.id);
        wrap.setAttribute("role", "presentation");
        wrap.className = "context-menu-volume-wrap";

        const header = doc.createElement("div");
        header.className = "context-menu-volume-header";

        const labelSpan = doc.createElement("span");
        labelSpan.className = "context-menu-volume-label";
        labelSpan.textContent = item.label;

        const valSpan = doc.createElement("span");
        valSpan.className = "context-menu-volume-value";
        valSpan.setAttribute("data-volume-value", "");
        valSpan.textContent = `${activeVolume}%`;

        header.appendChild(labelSpan);
        header.appendChild(valSpan);

        const slider = doc.createElement("input");
        slider.type = "range";
        slider.setAttribute("type", "range");
        slider.setAttribute("min", String(item.min ?? 0));
        slider.setAttribute("max", String(item.max ?? 200));
        slider.setAttribute("value", String(activeVolume));
        slider.value = String(activeVolume);
        slider.setAttribute("role", "slider");
        slider.setAttribute("aria-label", item.label);
        slider.setAttribute("aria-valuemin", String(item.min ?? 0));
        slider.setAttribute("aria-valuemax", String(item.max ?? 200));
        slider.setAttribute("aria-valuenow", String(activeVolume));
        slider.setAttribute("data-volume-slider", "");
        slider.className = "context-menu-slider";

        const handleVolumeUpdate = (/** @type {any} */ event) => {
          if (event && typeof event.stopPropagation === "function") {
            event.stopPropagation();
          }
          const num = Number(slider.value);
          const clamped = Math.max(
            0,
            Math.min(200, Number.isFinite(num) ? Math.round(num) : 100),
          );
          activeVolume = clamped;
          valSpan.textContent = `${clamped}%`;
          slider.setAttribute("aria-valuenow", String(clamped));
          if (onVolumeChange) {
            onVolumeChange(clamped);
          }
        };

        slider.addEventListener("input", handleVolumeUpdate);
        slider.addEventListener("change", handleVolumeUpdate);

        wrap.appendChild(header);
        wrap.appendChild(slider);
        return wrap;
      }

      const btn = doc.createElement("button");
      btn.className = item.danger
        ? "context-menu-item context-menu-item-danger"
        : "context-menu-item";
      btn.setAttribute("type", "button");
      btn.setAttribute("role", "menuitem");
      btn.setAttribute("data-context-item", item.id);
      btn.setAttribute("data-action-id", item.id);
      if (item.danger) {
        btn.setAttribute("data-danger", "true");
      }
      btn.textContent = item.label;
      btn.addEventListener("click", (/** @type {any} */ event) => {
        if (event && typeof event.stopPropagation === "function") {
          event.stopPropagation();
        }
        focusedIndex = idx;
        selectItemAt(idx);
      });
      return btn;
    });

    replaceElementChildren(menuEl, itemElements);
    menuEl.removeAttribute("hidden");
    menuEl.setAttribute("data-open", "true");
    menuEl.setAttribute("aria-hidden", "false");
    menuEl.setAttribute("data-target-type", String(currentTargetType));
    menuEl.setAttribute("data-target-id", currentTargetId);
    menuEl.setAttribute("data-x", String(coords.x));
    menuEl.setAttribute("data-y", String(coords.y));
    menuEl.setAttribute("style", `left: ${coords.x}px; top: ${coords.y}px;`);

    applyFocusAt(0);
    attachOutsideListener();
    return true;
  }

  /**
   * Открывает унифицированное контекстное меню для сообщения.
   *
   * @param {Omit<OpenContextMenuOptions, "targetType">} [msgOptions]
   * @returns {boolean}
   */
  function openForMessage(msgOptions = {}) {
    return open({
      ...msgOptions,
      targetType: "message",
      unified: true,
    });
  }

  /**
   * Открывает унифицированное контекстное меню для участника / пользователя.
   *
   * @param {Omit<OpenContextMenuOptions, "targetType">} [memberOptions]
   * @returns {boolean}
   */
  function openForMember(memberOptions = {}) {
    return open({
      ...memberOptions,
      targetType: "user",
      unified: true,
    });
  }

  /**
   * Обрабатывает DOM-событие `contextmenu` (ПКМ), извлекая атрибуты цели и открывая меню.
   *
   * @param {any} event
   * @param {Partial<OpenContextMenuOptions>} [overrideOptions]
   * @returns {boolean}
   */
  function handleContextMenuEvent(event, overrideOptions = {}) {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    if (event && typeof event.stopPropagation === "function") {
      event.stopPropagation();
    }
    const domOptions = extractTargetOptionsFromDom(event?.target);
    const clientX =
      event &&
      typeof event.clientX === "number" &&
      Number.isFinite(event.clientX)
        ? event.clientX
        : 0;
    const clientY =
      event &&
      typeof event.clientY === "number" &&
      Number.isFinite(event.clientY)
        ? event.clientY
        : 0;

    return open({
      x: clientX,
      y: clientY,
      ...domOptions,
      ...overrideOptions,
    });
  }

  menuEl.addEventListener("keydown", (/** @type {any} */ event) => {
    if (!openState) return;
    const key = event && typeof event.key === "string" ? event.key : "";
    if (key === "ArrowDown") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      moveFocus(1);
    } else if (key === "ArrowUp") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      moveFocus(-1);
    } else if (key === "ArrowRight") {
      const focusedItem = currentItems[focusedIndex];
      if (
        focusedItem &&
        (focusedItem.type === "slider" || focusedItem.id === "user-volume")
      ) {
        if (typeof event.preventDefault === "function") event.preventDefault();
        setVolume(activeVolume + (focusedItem.step ?? KEYBOARD_VOLUME_STEP));
      }
    } else if (key === "ArrowLeft") {
      const focusedItem = currentItems[focusedIndex];
      if (
        focusedItem &&
        (focusedItem.type === "slider" || focusedItem.id === "user-volume")
      ) {
        if (typeof event.preventDefault === "function") event.preventDefault();
        setVolume(activeVolume - (focusedItem.step ?? KEYBOARD_VOLUME_STEP));
      }
    } else if (key === "Home") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      applyFocusAt(0);
    } else if (key === "End") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      applyFocusAt(currentItems.length - 1);
    } else if (key === "Enter" || key === " ") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      activateFocusedItem();
    } else if (key === "Escape") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      close();
    }
  });

  container.appendChild(menuEl);

  return {
    open,
    openForMessage,
    openForMember,
    handleContextMenuEvent,
    moveFocus,
    activateFocusedItem,
    selectAction,
    close,
    setVolume,
    getVolume,
    isOpen: () => openState,
    getFocusedIndex: () => focusedIndex,
    getItems: () => [...currentItems],
    getElement: () => menuEl,
  };
}

/**
 * @callback ContextMenuTriggerResolver
 * @param {any} event
 * @returns {Partial<OpenContextMenuOptions>}
 */

/**
 * Привязывает обработчик контекстного меню (ПКМ) к элементу или контейнеру.
 *
 * @param {any} element
 * @param {ReturnType<typeof createContextMenuController>} controller
 * @param {ContextMenuTriggerResolver | Partial<OpenContextMenuOptions>} [resolverOrOptions]
 * @returns {() => void} Функция отписки (unbind)
 */
export function bindContextMenuTrigger(element, controller, resolverOrOptions) {
  if (!element || typeof element.addEventListener !== "function") {
    throw new TypeError(
      "Element must be a valid DOM object with addEventListener",
    );
  }
  if (!controller || typeof controller.open !== "function") {
    throw new TypeError("Controller must provide an open method");
  }

  const handler = (/** @type {any} */ event) => {
    if (event && typeof event.preventDefault === "function") {
      event.preventDefault();
    }
    if (event && typeof event.stopPropagation === "function") {
      event.stopPropagation();
    }
    const domOptions = extractTargetOptionsFromDom(event?.target, element);
    const resolved =
      typeof resolverOrOptions === "function"
        ? resolverOrOptions(event)
        : resolverOrOptions && typeof resolverOrOptions === "object"
          ? resolverOrOptions
          : {};
    const clientX =
      event &&
      typeof event.clientX === "number" &&
      Number.isFinite(event.clientX)
        ? event.clientX
        : 0;
    const clientY =
      event &&
      typeof event.clientY === "number" &&
      Number.isFinite(event.clientY)
        ? event.clientY
        : 0;

    controller.open({
      x: clientX,
      y: clientY,
      ...domOptions,
      ...resolved,
    });
  };

  element.addEventListener("contextmenu", handler);

  return function unbind() {
    if (typeof element.removeEventListener === "function") {
      element.removeEventListener("contextmenu", handler);
    }
  };
}
