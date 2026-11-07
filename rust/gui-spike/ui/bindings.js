// @ts-check
/**
 * Strongly-typed Tauri BFF command bindings & Discord PermissionCalculator (`specta` / `tauri-specta` + `twilight-util`).
 * Eliminates untyped `guild_id`/`guildId`, `is_member`/`isMember`, `my_roles`/`myRoles` drift between `data.rs` and `app.js`.
 */

export const PERM_CREATE_INSTANT_INVITE = 1n << 0n;
export const PERM_ADMINISTRATOR = 1n << 3n;
export const PERM_ADD_REACTIONS = 1n << 6n;
export const PERM_STREAM = 1n << 9n;
export const PERM_VIEW_CHANNEL = 1n << 10n;
export const PERM_SEND_MESSAGES = 1n << 11n;
export const PERM_MANAGE_MESSAGES = 1n << 13n;
export const PERM_EMBED_LINKS = 1n << 14n;
export const PERM_ATTACH_FILES = 1n << 15n;
export const PERM_READ_MESSAGE_HISTORY = 1n << 16n;
export const PERM_MENTION_EVERYONE = 1n << 17n;
export const PERM_CONNECT = 1n << 20n;
export const PERM_SPEAK = 1n << 21n;
export const PERM_USE_VAD = 1n << 25n;

/**
 * Discord's official `lurkerPermissionsMask` (`VIEW_CHANNEL | READ_MESSAGE_HISTORY` = `66560n`)
 * verified against `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0@9773248`.
 */
export const LURKER_PERMISSIONS_MASK = PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY;
export const ALL_PERMISSIONS_MASK = (1n << 53n) - 1n;

/**
 * @param {unknown} raw
 * @returns {bigint}
 */
export function parsePermissionBits(raw) {
  if (typeof raw === "bigint") return raw;
  if (typeof raw === "number" && Number.isFinite(raw)) return BigInt(Math.max(0, Math.trunc(raw)));
  if (typeof raw === "string" && raw.trim() !== "") {
    try {
      return BigInt(raw.trim());
    } catch {
      return 0n;
    }
  }
  return 0n;
}

/**
 * Normalizes a raw guild payload from READY / REST / cache into a strict `GuildSyncSummaryDto`.
 *
 * @param {Record<string, any> | null | undefined} raw
 * @returns {import("./bindings.d.ts").GuildSyncSummaryDto | null}
 */
export function normalizeGuildDto(raw) {
  if (!raw || typeof raw !== "object") return null;
  const props = raw.properties && typeof raw.properties === "object" ? raw.properties : {};
  const id = String(raw.id || props.id || "").trim();
  if (!id) return null;
  const name = String(raw.name || props.name || id);
  const icon = raw.icon ?? props.icon ?? null;
  const ownerId = raw.ownerId ?? raw.owner_id ?? props.owner_id ?? props.ownerId ?? null;
  const lurker = raw.lurker === true || raw.isMember === false || raw.is_member === false;
  const isMember = !lurker && raw.isMember !== false && raw.is_member !== false;
  const pending = raw.pending === true || raw.isPending === true;
  const rawMyRoles = Array.isArray(raw.myRoles)
    ? raw.myRoles
    : Array.isArray(raw.my_roles)
      ? raw.my_roles
      : [];
  const myRoles = lurker ? [] : rawMyRoles.map((r) => String(r)).filter(Boolean);
  const roles = Array.isArray(raw.roles) ? raw.roles : [];
  const features = Array.isArray(raw.features)
    ? raw.features.map(String)
    : Array.isArray(props.features)
      ? props.features.map(String)
      : [];
  const channels = Array.isArray(raw.channels) ? raw.channels : [];
  const channelCategories = Array.isArray(raw.channelCategories)
    ? raw.channelCategories
    : Array.isArray(raw.channel_categories)
      ? raw.channel_categories
      : [];

  return {
    id,
    name,
    icon: icon ? String(icon) : null,
    ownerId: ownerId ? String(ownerId) : null,
    isMember,
    lurker,
    pending,
    myRoles,
    roles,
    features,
    channels,
    channelCategories,
  };
}

/**
 * Computes effective channel permissions (`twilight-util::permission_calculator` + `lurkerPermissionsMask`).
 *
 * @param {{
 *   guild: Record<string, any> | null | undefined,
 *   channel: Record<string, any> | null | undefined,
 *   parentCategory?: Record<string, any> | null | undefined,
 *   currentUserId?: string | null
 * }} params
 * @returns {import("./bindings.d.ts").ChannelAccessDto}
 */
export function evaluateChannelAccessDto({ guild, channel, parentCategory, currentUserId }) {
  const dto = normalizeGuildDto(guild);
  const chType = Number(channel?.type ?? 0);
  const isVoice = chType === 2 || chType === 13 || channel?.kind === "voice";
  if (!dto) {
    return {
      effectivePermissions: ALL_PERMISSIONS_MASK.toString(),
      canViewChannel: true,
      canReadHistory: true,
      canSendMessages: true,
      canAddReactions: true,
      canAttachFiles: true,
      canConnectVoice: true,
      canSpeakVoice: true,
      canStreamVoice: true,
      isLurker: false,
      isPending: false,
      readOnly: false,
      deniedReason: null,
    };
  }

  const uid = String(currentUserId || "").trim();
  const isOwner = Boolean(!dto.lurker && uid && dto.ownerId && dto.ownerId === uid);
  let effective = ALL_PERMISSIONS_MASK;

  if (!isOwner && dto.roles.length > 0) {
    const memberRoleSet = new Set(dto.myRoles);
    if (!dto.lurker && uid && Array.isArray(guild?.members)) {
      const me = guild.members.find((m) => String(m?.user?.id || m?.user_id || m?.id || "") === uid);
      if (me && Array.isArray(me.roles)) {
        for (const r of me.roles) memberRoleSet.add(String(r));
      }
    }

    let basePerms = 0n;
    let foundEveryone = false;
    for (const r of dto.roles) {
      if (!r || typeof r !== "object") continue;
      const rid = String(r.id || "");
      const p = parsePermissionBits(r.permissions);
      if (rid === dto.id) {
        basePerms |= p;
        foundEveryone = true;
      } else if (!dto.lurker && memberRoleSet.has(rid)) {
        basePerms |= p;
      }
    }
    if (!foundEveryone) {
      basePerms |= PERM_VIEW_CHANNEL | PERM_READ_MESSAGE_HISTORY | PERM_SEND_MESSAGES;
    }

    if (!dto.lurker && (basePerms & PERM_ADMINISTRATOR) === PERM_ADMINISTRATOR) {
      effective = ALL_PERMISSIONS_MASK;
    } else {
      effective = basePerms;
      const applyOverwrites = (overwrites) => {
        if (!Array.isArray(overwrites) || overwrites.length === 0) return;
        const everyoneOw = overwrites.find((ow) => ow && String(ow.id || "") === dto.id);
        if (everyoneOw) {
          effective &= ~parsePermissionBits(everyoneOw.deny);
          effective |= parsePermissionBits(everyoneOw.allow);
        }
        if (!dto.lurker) {
          let allowRoles = 0n;
          let denyRoles = 0n;
          for (const ow of overwrites) {
            if (!ow || typeof ow !== "object") continue;
            const owId = String(ow.id || "");
            const owType = ow.type;
            const isRoleOw = owType === 0 || owType === "0" || owType === "role" || owType === undefined;
            if (isRoleOw && owId !== dto.id && memberRoleSet.has(owId)) {
              allowRoles |= parsePermissionBits(ow.allow);
              denyRoles |= parsePermissionBits(ow.deny);
            }
          }
          effective &= ~denyRoles;
          effective |= allowRoles;

          if (uid) {
            const memberOw = overwrites.find(
              (ow) =>
                ow &&
                (ow.type === 1 || ow.type === "1" || ow.type === "member") &&
                String(ow.id || "") === uid
            );
            if (memberOw) {
              effective &= ~parsePermissionBits(memberOw.deny);
              effective |= parsePermissionBits(memberOw.allow);
            }
          }
        }
      };

      const catOverwrites = parentCategory?.permission_overwrites || parentCategory?.permissionOverwrites;
      const chOverwrites = channel?.permission_overwrites || channel?.permissionOverwrites;
      applyOverwrites(catOverwrites);
      applyOverwrites(chOverwrites);
    }
  }

  if ((effective & PERM_VIEW_CHANNEL) === 0n) {
    effective = 0n;
  } else {
    if ((effective & PERM_SEND_MESSAGES) === 0n) {
      effective &= ~(PERM_MENTION_EVERYONE | PERM_ATTACH_FILES | PERM_EMBED_LINKS);
    }
    if ((effective & PERM_CONNECT) === 0n) {
      effective &= ~(PERM_SPEAK | PERM_STREAM | PERM_USE_VAD);
    }
  }

  if (dto.lurker || dto.pending) {
    effective &= LURKER_PERMISSIONS_MASK;
  }

  const canViewChannel = (effective & PERM_VIEW_CHANNEL) !== 0n;
  const canReadHistory = canViewChannel && (effective & PERM_READ_MESSAGE_HISTORY) !== 0n;
  const canSendMessages = canViewChannel && !dto.lurker && !dto.pending && (effective & PERM_SEND_MESSAGES) !== 0n;
  const canAddReactions = canViewChannel && !dto.lurker && !dto.pending && (effective & PERM_ADD_REACTIONS) !== 0n;
  const canAttachFiles = canSendMessages && (effective & PERM_ATTACH_FILES) !== 0n;
  const canConnectVoice =
    canViewChannel && !dto.lurker && !dto.pending && (!isVoice || (effective & PERM_CONNECT) !== 0n);
  const canSpeakVoice = canConnectVoice && (effective & PERM_SPEAK) !== 0n;
  const canStreamVoice = canConnectVoice && (effective & PERM_STREAM) !== 0n;

  let deniedReason = null;
  if (!canViewChannel) {
    deniedReason = "Нет доступа к просмотру этого канала (требуется соответствующая роль)";
  } else if (dto.lurker) {
    deniedReason = isVoice
      ? "Режим предпросмотра сервера (вы не участник): вступите на сервер, чтобы подключаться к голосовым каналам."
      : "Режим предпросмотра сервера (вы не участник) — вступите на сервер, чтобы отправлять сообщения и ставить реакции.";
  } else if (dto.pending) {
    deniedReason =
      "Завершите принятие правил сервера (Membership Screening), чтобы отправлять сообщения и подключаться к голосовым каналам.";
  } else if (isVoice && !canConnectVoice) {
    deniedReason = "У вашей роли нет прав для подключения к этому голосовому каналу (CONNECT).";
  } else if (!isVoice && !canSendMessages) {
    deniedReason = "У вас нет прав на отправку сообщений в этом канале.";
  }

  return {
    effectivePermissions: effective.toString(),
    canViewChannel,
    canReadHistory,
    canSendMessages,
    canAddReactions,
    canAttachFiles,
    canConnectVoice,
    canSpeakVoice,
    canStreamVoice,
    isLurker: dto.lurker,
    isPending: dto.pending,
    readOnly: !canSendMessages,
    deniedReason,
  };
}

/**
 * Creates typed Tauri IPC command wrappers bound to an `invoke(cmd, args)` function.
 *
 * @param {(cmd: string, args?: Record<string, unknown>) => Promise<any>} invokeFn
 */
export function createTypedCommands(invokeFn) {
  return Object.freeze({
    /**
     * Fetches the full live state snapshot (`LiveSyncSnapshotDto`).
     * @returns {Promise<import("./bindings.d.ts").LiveSyncSnapshotDto>}
     */
    async syncLiveState() {
      const raw = await invokeFn("voice_set_audio_device", {
        kind: "dm_friends_sync",
        deviceId: "",
      });
      const guilds = Array.isArray(raw?.guilds)
        ? raw.guilds.map((g) => normalizeGuildDto(g)).filter(Boolean)
        : [];
      return {
        ok: Boolean(raw?.ok),
        live: Boolean(raw?.live),
        currentUser: raw?.currentUser || {},
        dmChannels: Array.isArray(raw?.dmChannels) ? raw.dmChannels : [],
        relationships: Array.isArray(raw?.relationships) ? raw.relationships : [],
        guilds: /** @type {import("./bindings.d.ts").GuildSyncSummaryDto[]} */ (guilds),
        voiceStates: raw?.voiceStates || {},
        readStates: raw?.readStates || {},
        userGuildSettings: raw?.userGuildSettings || {},
        rtc: raw?.rtc || undefined,
      };
    },

    /**
     * Connects or updates the live voice state (`OP 4 VOICE_STATE_UPDATE` + RTC).
     * @param {{ guildId: string | null, channelId: string | null, selfMute: boolean, selfDeaf: boolean, selfVideo?: boolean, selfStream?: boolean }} opts
     */
    async voiceConnect(opts) {
      const encoded = [
        opts.guildId && opts.guildId !== "@me" ? opts.guildId : "",
        opts.channelId || "",
        opts.selfMute ? "1" : "0",
        opts.selfDeaf ? "1" : "0",
        opts.selfVideo ? "1" : "0",
        opts.selfStream ? "1" : "0",
      ].join(":");
      return invokeFn("voice_set_audio_device", {
        kind: "voice_connect",
        deviceId: encoded,
      });
    },

    /**
     * Queries real-time UDP/SRTP voice transport telemetry (`VoiceRtcStatusDto`).
     * @returns {Promise<import("./bindings.d.ts").VoiceRtcStatusDto>}
     */
    async voiceRtcStatus() {
      return invokeFn("voice_set_audio_device", {
        kind: "voice_rtc_status",
        deviceId: "",
      });
    },
  });
}
