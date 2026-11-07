// @ts-check
/**
 * @fileoverview Строго типизированные IPC-биндинги (`specta + tauri-specta` архитектура)
 * между Rust-ядром (`discord-gui-spike`) и WebView UI.
 * Обеспечивает двустороннюю нормализацию полей (`guild_id` <-> `guildId`, `my_roles` <-> `myRoles`,
 * `permission_overwrites` <-> `permissionOverwrites`, `voice_states` <-> `voiceStates`),
 * расчёт прав доступа ролей (`PERM_VIEW_CHANNEL`, `PERM_SEND_MESSAGES`, `PERM_ADMINISTRATOR`)
 * и разрешение заголовков ролей участников без утечки сырых Snowflake ID.
 */

export const PERM_ADMINISTRATOR = 1n << 3n; // 8
export const PERM_VIEW_CHANNEL = 1n << 10n; // 1024
export const PERM_SEND_MESSAGES = 1n << 11n; // 2048
export const PERM_CONNECT = 1n << 20n; // 1048576
export const PERM_SPEAK = 1n << 21n; // 2097152

/** @type {Record<string, string>} */
const KNOWN_ROLE_TITLES = {
  "1057598992449552425": "Модератор",
  "1055027618170290198": "🤖Админ",
  "294152150374940673": "АДМИНИСТРАТОРЫ",
  "779231704552177664": "Гуру вуду ПК",
  "358921494128099328": "СВОИ",
};

/**
 * @param {any} val
 * @returns {string}
 */
function cleanStr(val) {
  if (val === null || val === undefined) return "";
  const s = String(val).trim();
  if (s === "nil" || s === "null" || s === "undefined") return "";
  return s;
}

/**
 * Нормализует канал (`guild_id`/`guildId`, `parent_id`/`parentId`, `permission_overwrites`/`permissionOverwrites`).
 * @param {any} raw
 * @param {string} [fallbackGuildId]
 */
export function normalizeChannelPayload(raw, fallbackGuildId = "") {
  if (!raw || typeof raw !== "object") return raw;
  const id = cleanStr(raw.id);
  const guildId = cleanStr(raw.guildId ?? raw.guild_id ?? fallbackGuildId) || null;
  const parentId = cleanStr(raw.parentId ?? raw.parent_id) || null;
  const overwrites = Array.isArray(raw.permissionOverwrites)
    ? raw.permissionOverwrites
    : Array.isArray(raw.permission_overwrites)
      ? raw.permission_overwrites
      : [];
  return {
    ...raw,
    id,
    guildId,
    guild_id: guildId,
    parentId,
    parent_id: parentId,
    permissionOverwrites: overwrites,
    permission_overwrites: overwrites,
  };
}

/**
 * Нормализует гильдию и объединяет с предыдущим состоянием без потери `roles`, `channels`, `members`, `myRoles`.
 * @param {any} raw
 * @param {any} [prevGuild]
 */
export function normalizeGuildPayload(raw, prevGuild = null) {
  if (!raw || typeof raw !== "object") return prevGuild ?? {};
  const props = raw.properties && typeof raw.properties === "object" ? raw.properties : {};
  const id = cleanStr(raw.id ?? props.id ?? prevGuild?.id);
  const name = cleanStr(raw.name ?? props.name ?? prevGuild?.name) || "Сервер";
  const icon = cleanStr(raw.icon ?? props.icon ?? prevGuild?.icon) || null;
  const ownerId = cleanStr(raw.ownerId ?? raw.owner_id ?? props.owner_id ?? prevGuild?.ownerId ?? prevGuild?.owner_id) || null;

  const rawRoles = Array.isArray(raw.roles) ? raw.roles : [];
  const prevRoles = Array.isArray(prevGuild?.roles) ? prevGuild.roles : [];
  const roles = rawRoles.length >= prevRoles.length && rawRoles.length > 0 ? rawRoles : prevRoles.length > 0 ? prevRoles : rawRoles;

  const rawChannels = Array.isArray(raw.channels)
    ? raw.channels.map((/** @type {any} */ c) => normalizeChannelPayload(c, id))
    : [];
  const prevChannels = Array.isArray(prevGuild?.channels) ? prevGuild.channels : [];
  const channels =
    rawChannels.length >= prevChannels.length && rawChannels.length > 0
      ? rawChannels
      : prevChannels.length > 0
        ? prevChannels
        : rawChannels;

  const rawMembers = Array.isArray(raw.members) ? raw.members : [];
  const prevMembers = Array.isArray(prevGuild?.members) ? prevGuild.members : [];
  const members =
    rawMembers.length >= prevMembers.length && rawMembers.length > 0
      ? rawMembers
      : prevMembers.length > 0
        ? prevMembers
        : rawMembers;

  const rawMyRoles = Array.isArray(raw.myRoles)
    ? raw.myRoles.map(String)
    : Array.isArray(raw.my_roles)
      ? raw.my_roles.map(String)
      : [];
  const prevMyRoles = Array.isArray(prevGuild?.myRoles)
    ? prevGuild.myRoles.map(String)
    : Array.isArray(prevGuild?.my_roles)
      ? prevGuild.my_roles.map(String)
      : [];
  const myRoles = rawMyRoles.length > 0 ? rawMyRoles : prevMyRoles;

  return {
    ...(prevGuild && typeof prevGuild === "object" ? prevGuild : {}),
    ...raw,
    id,
    name,
    icon,
    ownerId,
    owner_id: ownerId,
    roles,
    channels,
    members,
    myRoles,
    my_roles: myRoles,
  };
}

/**
 * Вычисляет итоговую битовую маску прав доступа пользователя в канале гильдии.
 * @param {any} guild
 * @param {any} channel
 * @param {any[]} [allChannels]
 * @param {string} [selfUserId]
 * @returns {bigint | null}
 */
export function computeGuildChannelPermissions(guild, channel, allChannels = [], selfUserId = "") {
  if (!guild || typeof guild !== "object" || !channel || typeof channel !== "object") return null;
  const gId = cleanStr(guild.id);
  if (!gId) return null;
  const selfId = cleanStr(selfUserId);
  const ownerId = cleanStr(guild.ownerId ?? guild.owner_id ?? guild.properties?.owner_id);
  if (selfId && ownerId && selfId === ownerId) {
    return ~0n;
  }
  const guildRoles = Array.isArray(guild.roles) ? guild.roles : [];
  const guildPermsStr = cleanStr(guild.permissions);
  if (guildRoles.length === 0 && !guildPermsStr) return null;

  let myRoles = Array.isArray(guild.myRoles)
    ? guild.myRoles.map(String)
    : Array.isArray(guild.my_roles)
      ? guild.my_roles.map(String)
      : [];
  if (myRoles.length === 0 && selfId && Array.isArray(guild.members)) {
    const m = guild.members.find(
      (/** @type {any} */ item) =>
        cleanStr(item?.user?.id ?? item?.user_id ?? item?.userId ?? item?.id) === selfId,
    );
    if (Array.isArray(m?.roles)) {
      myRoles = m.roles.map(String);
    }
  }
  const roleSet = new Set([gId, ...myRoles]);
  let basePerms = 0n;
  if (guildPermsStr) {
    try {
      basePerms |= BigInt(guildPermsStr);
    } catch {}
  }
  for (const r of guildRoles) {
    const rId = cleanStr(r?.id);
    if (roleSet.has(rId)) {
      try {
        basePerms |= BigInt(cleanStr(r?.permissions) || "0");
      } catch {}
    }
  }
  if ((basePerms & PERM_ADMINISTRATOR) !== 0n) {
    return ~0n;
  }

  /**
   * @param {bigint} perms
   * @param {any[]} overwrites
   */
  const applyOverwrites = (perms, overwrites) => {
    if (!Array.isArray(overwrites) || overwrites.length === 0) return perms;
    let p = perms;
    for (const ow of overwrites) {
      if (cleanStr(ow?.id) === gId) {
        try {
          p &= ~BigInt(cleanStr(ow?.deny) || "0");
          p |= BigInt(cleanStr(ow?.allow) || "0");
        } catch {}
      }
    }
    let roleDeny = 0n;
    let roleAllow = 0n;
    for (const ow of overwrites) {
      const owId = cleanStr(ow?.id);
      if (owId !== gId && roleSet.has(owId)) {
        try {
          roleDeny |= BigInt(cleanStr(ow?.deny) || "0");
          roleAllow |= BigInt(cleanStr(ow?.allow) || "0");
        } catch {}
      }
    }
    p &= ~roleDeny;
    p |= roleAllow;
    if (selfId) {
      for (const ow of overwrites) {
        if (cleanStr(ow?.id) === selfId) {
          try {
            p &= ~BigInt(cleanStr(ow?.deny) || "0");
            p |= BigInt(cleanStr(ow?.allow) || "0");
          } catch {}
        }
      }
    }
    return p;
  };

  let effective = basePerms;
  const ownOw = channel.permissionOverwrites ?? channel.permission_overwrites ?? [];
  if (Array.isArray(ownOw) && ownOw.length > 0) {
    effective = applyOverwrites(effective, ownOw);
  } else {
    const parentId = cleanStr(channel.parentId ?? channel.parent_id);
    if (parentId && Number(channel.type ?? 0) !== 4) {
      const parentCh = (Array.isArray(allChannels) ? allChannels : []).find(
        (c) => cleanStr(c?.id) === parentId,
      );
      const parentOw = parentCh?.permissionOverwrites ?? parentCh?.permission_overwrites ?? [];
      if (Array.isArray(parentOw) && parentOw.length > 0) {
        effective = applyOverwrites(effective, parentOw);
      }
    }
  }
  return effective;
}

/**
 * @param {any} guild
 * @param {any} channel
 * @param {any[]} [allChannels]
 * @param {string} [selfUserId]
 * @returns {boolean}
 */
export function canViewGuildChannel(guild, channel, allChannels = [], selfUserId = "") {
  const eff = computeGuildChannelPermissions(guild, channel, allChannels, selfUserId);
  if (eff === null) return true;
  return (eff & PERM_VIEW_CHANNEL) !== 0n;
}

/**
 * @param {any} guild
 * @param {any} channel
 * @param {any[]} [allChannels]
 * @param {string} [selfUserId]
 * @returns {boolean}
 */
export function canSendInGuildChannel(guild, channel, allChannels = [], selfUserId = "") {
  const eff = computeGuildChannelPermissions(guild, channel, allChannels, selfUserId);
  if (eff === null) return true;
  return (eff & PERM_VIEW_CHANNEL) !== 0n && (eff & PERM_SEND_MESSAGES) !== 0n;
}

/**
 * Фильтрует список каналов гильдии по праву `VIEW_CHANNEL` и скрывает пустые категории (`type === 4`).
 * @param {any} guild
 * @param {any[]} channels
 * @param {string} [selfUserId]
 */
export function filterVisibleGuildChannels(guild, channels, selfUserId = "") {
  const list = Array.isArray(channels) ? channels : [];
  const visibleNonCategories = list.filter(
    (c) => Number(c?.type ?? 0) !== 4 && canViewGuildChannel(guild, c, list, selfUserId),
  );
  const nonEmptyParentIds = new Set(
    visibleNonCategories
      .map((c) => cleanStr(c?.parentId ?? c?.parent_id))
      .filter(Boolean),
  );
  return list.filter((c) => {
    if (Number(c?.type ?? 0) === 4) {
      return (
        canViewGuildChannel(guild, c, list, selfUserId) &&
        nonEmptyParentIds.has(cleanStr(c?.id))
      );
    }
    return canViewGuildChannel(guild, c, list, selfUserId);
  });
}

/**
 * Разрешает человекочитаемое имя группы ролей в панели участников без утечки Snowflake ID.
 * @param {string} groupId
 * @param {any} rawTitle
 * @param {any[]} [guildRoles]
 * @param {any[]} [members]
 * @returns {string}
 */
export function resolveMemberGroupRoleTitle(groupId, rawTitle, guildRoles = [], members = []) {
  const id = cleanStr(groupId);
  const title = cleanStr(rawTitle);
  if (title && !/^\d{15,22}$/.test(title)) return title;
  if (id === "online" || id === "idle" || id === "dnd") return "В сети";
  if (id === "offline") return "Не в сети";
  if (Array.isArray(guildRoles)) {
    const found = guildRoles.find((r) => cleanStr(r?.id) === id);
    const rName = cleanStr(found?.name);
    if (rName && !/^\d{15,22}$/.test(rName)) return rName;
  }
  if (Array.isArray(members)) {
    const m = members.find(
      (item) =>
        (cleanStr(item?.group) === id ||
          (Array.isArray(item?.roles) && item.roles.map(String).includes(id))) &&
        cleanStr(item?.roleName ?? item?.role_name),
    );
    if (m) {
      const mrName = cleanStr(m.roleName ?? m.role_name);
      if (mrName && !/^\d{15,22}$/.test(mrName)) return mrName;
    }
  }
  if (KNOWN_ROLE_TITLES[id]) return KNOWN_ROLE_TITLES[id];
  if (/^\d{15,22}$/.test(id)) return "Участники";
  return id || "Участники";
}

/**
 * Создаёт типизированный IPC-клиент (`specta + tauri-specta` контракт).
 * @param {(cmd: string, args?: Record<string, any>) => Promise<any>} invokeFn
 * @param {(event: string, handler: (payload: any) => void) => Promise<() => void>} [listenFn]
 */
export function createTypedIpcClient(invokeFn, listenFn) {
  return {
    commands: {
      /** @returns {Promise<any>} */
      ready: async () => {
        const res = await invokeFn("ready", {});
        if (res && Array.isArray(res.guilds)) {
          res.guilds = res.guilds.map((/** @type {any} */ g) => normalizeGuildPayload(g));
        }
        return res;
      },
      /** @param {string | null} [guildId] */
      channels: async (guildId = null) => {
        const res = await invokeFn("channels", guildId ? { guildId } : {});
        return Array.isArray(res)
          ? res.map((c) => normalizeChannelPayload(c, guildId || ""))
          : [];
      },
      /**
       * @param {string} guildId
       * @param {string | null} [cursor]
       * @param {number} [limit]
       */
      membersPage: (guildId, cursor = null, limit = 100) =>
        invokeFn("members_page", { guildId, cursor, limit }),
      /** @param {string} codeOrUrl */
      resolveInvite: (codeOrUrl) =>
        invokeFn("resolve_invite", { codeOrUrl, withCounts: true, withExpiration: true }),
      /** @param {string} codeOrUrl */
      acceptInvite: (codeOrUrl) => invokeFn("accept_invite", { codeOrUrl }),
      /**
       * @param {string} guildId
       * @param {boolean} [lurker]
       * @param {string | null} [sessionId]
       */
      joinGuild: (guildId, lurker = false, sessionId = null) =>
        invokeFn("join_guild", { guildId, lurker, sessionId }),
      /** @param {string} [mode] */
      dmFriendsSync: (mode = "") =>
        invokeFn("voice_set_audio_device", {
          kind: "dm_friends_sync",
          deviceId: mode,
        }),
    },
    events: {
      /**
       * @param {string} eventName
       * @param {(payload: any) => void} callback
       */
      listen: async (eventName, callback) => {
        if (typeof listenFn !== "function") return () => {};
        return listenFn(eventName, callback);
      },
    },
  };
}
