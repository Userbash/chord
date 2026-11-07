// @ts-check
/**
 * Reactive Channel Tree, Confirmed Voice Participants & Server Access Synchronization (Stage V5).
 * - Renders voice channel participants strictly from confirmed `VOICE_STATE_UPDATE` events
 *   for the given `channelId` (no phantom/optimistic avatars before confirmation, and immediate
 *   removal on timeout/error/leave).
 * - Synchronizes banners and composer input blocking for `lurker`, `pending: true`,
 *   and missing `SEND_MESSAGES` / `CONNECT` permissions.
 */

import { evaluateChannelAccessDto, normalizeGuildDto } from "../bindings.js";

/**
 * Проверяет, является ли запись голосового состояния подтверждённым событием `VOICE_STATE_UPDATE`
 * для активного участника в голосовом канале (отсекает неподтверждённые/фантомные/ошибочные записи).
 *
 * @param {Record<string, any> | null | undefined} vs
 * @returns {boolean}
 */
export function isConfirmedVoiceStateRecord(vs) {
  if (!vs || typeof vs !== "object") return false;
  if (
    vs.confirmed === false ||
    vs.optimistic === true ||
    vs.pendingConfirmation === true ||
    vs.phantom === true
  ) {
    return false;
  }
  const phase = String(vs.phase ?? vs.state ?? "").trim().toLowerCase();
  if (
    phase === "signaling" ||
    phase === "timeout" ||
    phase === "error" ||
    vs.timeout === true ||
    Boolean(vs.error)
  ) {
    return false;
  }
  const uid = String(vs.user_id ?? vs.userId ?? vs.member?.user?.id ?? "").trim();
  const cid =
    vs.channel_id === null || vs.channelId === null
      ? ""
      : String(vs.channel_id ?? vs.channelId ?? "").trim();
  return Boolean(uid && cid);
}

/**
 * Применяет событие `VOICE_STATE_UPDATE` (или сигнал таймаута/ошибки звонка) к карте голосовых состояний гильдий.
 * Неподтверждённые (`confirmed: false` / `optimistic: true`) кандидаты игнорируются,
 * а при `channel_id: null` или `phase: 'timeout' | 'error'` участник немедленно удаляется.
 *
 * @param {Map<string, Array<Record<string, any>>>} voiceStatesByGuild
 * @param {Record<string, any> | null | undefined} eventPayload
 * @returns {boolean} `true`, если состояние изменилось
 */
export function applyVoiceStateUpdateEvent(voiceStatesByGuild, eventPayload) {
  if (!voiceStatesByGuild || typeof voiceStatesByGuild.get !== "function" || !eventPayload) {
    return false;
  }
  const raw =
    eventPayload.d && typeof eventPayload.d === "object" ? eventPayload.d : eventPayload;
  const userId = String(
    raw.user_id ?? raw.userId ?? raw.member?.user?.id ?? "",
  ).trim();
  if (!userId) return false;

  if (
    raw.confirmed === false ||
    raw.optimistic === true ||
    raw.pendingConfirmation === true ||
    raw.phantom === true
  ) {
    return false;
  }

  const guildKey = String(raw.guild_id ?? raw.guildId ?? "@me").trim() || "@me";
  const channelId =
    raw.channel_id === null || raw.channelId === null
      ? ""
      : String(raw.channel_id ?? raw.channelId ?? "").trim();
  const phase = String(raw.phase ?? raw.state ?? "").trim().toLowerCase();
  const isRemoval =
    !channelId ||
    phase === "timeout" ||
    phase === "error" ||
    phase === "disconnected" ||
    raw.timeout === true ||
    Boolean(raw.error);

  const prevList = voiceStatesByGuild.get(guildKey) ?? [];
  const filtered = prevList.filter(
    (item) => String(item?.user_id ?? item?.userId ?? "") !== userId,
  );

  if (isRemoval) {
    voiceStatesByGuild.set(guildKey, filtered);
    return true;
  }

  filtered.push({
    ...raw,
    guild_id: guildKey,
    channel_id: channelId,
    user_id: userId,
    confirmed: true,
  });
  voiceStatesByGuild.set(guildKey, filtered);
  return true;
}

/**
 * Вычисляет список подтверждённых участников голосового канала строго из `VOICE_STATE_UPDATE`
 * для данного `channelId` (без фантомных аватаров до подтверждения).
 *
 * @param {{
 *   voiceStatesByGuild?: Map<string, Array<Record<string, any>>> | null,
 *   voiceStates?: Array<Record<string, any>> | null,
 *   guildId?: string | null,
 *   channelId: string,
 *   resolveParticipant?: ((vs: Record<string, any>, userId: string) => Record<string, any>) | null,
 * }} params
 * @returns {Array<Record<string, any>>}
 */
export function computeConfirmedVoiceParticipants({
  voiceStatesByGuild = null,
  voiceStates = null,
  guildId = "",
  channelId,
  resolveParticipant = null,
}) {
  const targetCid = String(channelId ?? "").trim();
  if (!targetCid) return [];

  const gKey = String(guildId ?? "").trim() || "@me";
  const rawStates = Array.isArray(voiceStates)
    ? voiceStates
    : voiceStatesByGuild instanceof Map
      ? (voiceStatesByGuild.get(gKey) ??
          (guildId ? voiceStatesByGuild.get(String(guildId)) : undefined) ??
          [])
      : [];

  /** @type {Map<string, Record<string, any>>} */
  const byUser = new Map();
  for (const vs of rawStates) {
    if (!isConfirmedVoiceStateRecord(vs)) continue;
    const vsCid = String(vs.channel_id ?? vs.channelId ?? "").trim();
    if (vsCid !== targetCid) continue;
    const uid = String(vs.user_id ?? vs.userId ?? vs.member?.user?.id ?? "").trim();
    if (!uid) continue;

    if (typeof resolveParticipant === "function") {
      const resolved = resolveParticipant(vs, uid);
      if (resolved) {
        byUser.set(uid, resolved);
      }
      continue;
    }

    const memberUser = vs.member?.user ?? vs.user ?? null;
    const displayName = String(
      vs.member?.nick ??
        memberUser?.global_name ??
        memberUser?.globalName ??
        memberUser?.username ??
        vs.displayName ??
        uid,
    );
    byUser.set(uid, {
      userId: uid,
      channelId: targetCid,
      guildId: gKey,
      displayName,
      avatarUrl: vs.avatarUrl ?? null,
      roleColor: vs.roleColor ?? null,
      muted: Boolean(vs.self_mute || vs.mute || vs.selfMute),
      deafened: Boolean(vs.self_deaf || vs.deaf || vs.selfDeaf),
      selfStream: Boolean(vs.self_stream || vs.selfStream),
      selfVideo: Boolean(vs.self_video || vs.selfVideo),
      speaking: Boolean(vs.speaking),
      sessionId: String(vs.session_id ?? vs.sessionId ?? ""),
    });
  }

  return Array.from(byUser.values());
}

/**
 * Рендерит DOM-список участников голосового канала (`.voice-channel-users`) строго по подтверждённому списку.
 * При пустом списке немедленно очищает/удаляет `.voice-channel-users`.
 *
 * @param {any} containerEl
 * @param {Array<Record<string, any>>} participants
 * @param {{
 *   channelId: string,
 *   doc?: any,
 * }} options
 * @returns {any}
 */
export function renderVoiceChannelParticipantsDom(containerEl, participants, options) {
  if (!containerEl) return null;
  const doc =
    options?.doc ??
    containerEl.ownerDocument ??
    (typeof document !== "undefined" ? document : null);
  const cid = String(options?.channelId ?? "");

  const existingList = containerEl.querySelector?.(".voice-channel-users");
  if (!Array.isArray(participants) || participants.length === 0) {
    existingList?.remove?.();
    return null;
  }

  if (!doc || typeof doc.createElement !== "function") return null;
  const vList = existingList ?? doc.createElement("ul");
  vList.className = "voice-channel-users";
  vList.setAttribute("role", "list");
  vList.textContent = "";

  for (const vp of participants) {
    const uid = String(vp?.userId ?? vp?.user_id ?? "").trim();
    if (!uid) continue;
    const vRow = doc.createElement("li");
    vRow.className = "voice-channel-user";
    vRow.setAttribute("data-user-id", uid);
    vRow.setAttribute("data-channel-id", cid);

    const vAvatar = doc.createElement("span");
    vAvatar.className = "avatar avatar--sm voice-user-avatar";
    vAvatar.setAttribute("data-voice-avatar-uid", uid);
    const disp = String(vp.displayName || uid).trim();
    vAvatar.textContent = Array.from(disp).slice(0, 1).join("").toUpperCase() || "U";

    const vName = doc.createElement("span");
    vName.className = "voice-user-name";
    vName.textContent = disp;

    vRow.append(vAvatar, vName);
    vList.append(vRow);
  }

  if (!existingList) {
    containerEl.append(vList);
  }
  return vList;
}

/**
 * Filters a guild's channel list and attaches computed `ChannelAccessDto` and confirmed voice participants per channel.
 *
 * @param {{
 *   guild: Record<string, any> | null | undefined,
 *   channels: Array<Record<string, any>>,
 *   currentUserId: string | null | undefined,
 *   voiceStatesByGuild?: Map<string, Array<Record<string, any>>> | null,
 * }} params
 */
export function computeGuildChannelTreeModel({
  guild,
  channels,
  currentUserId,
  voiceStatesByGuild = null,
}) {
  const guildDto = normalizeGuildDto(guild);
  const byId = new Map();
  for (const ch of channels || []) {
    if (ch && ch.id) byId.set(String(ch.id), ch);
  }

  const visibleChannels = [];
  for (const ch of channels || []) {
    if (!ch || typeof ch !== "object") continue;
    const parentCategory = ch.parent_id ? byId.get(String(ch.parent_id)) : null;
    const access = evaluateChannelAccessDto({
      guild,
      channel: ch,
      parentCategory,
      currentUserId,
    });
    if (access.canViewChannel) {
      const chType = Number(ch.type ?? 0);
      const isVoice = chType === 2 || chType === 13;
      const voiceParticipants =
        isVoice && voiceStatesByGuild
          ? computeConfirmedVoiceParticipants({
              voiceStatesByGuild,
              guildId: String(guildDto?.id ?? guild?.id ?? ""),
              channelId: String(ch.id ?? ""),
            })
          : [];
      visibleChannels.push({
        channel: ch,
        access,
        voiceParticipants,
      });
    }
  }

  return {
    guildDto,
    isPreviewMode: Boolean(guildDto?.lurker),
    isPendingScreening: Boolean(guildDto?.pending),
    visibleChannels,
  };
}

/**
 * Синхронизирует баннеры доступа к серверу/каналу и блокировку поля ввода `#composer-input`
 * для `lurker`, `pending: true` и отсутствия прав `SEND_MESSAGES` / `CONNECT`.
 *
 * @param {{
 *   doc?: any,
 *   guild: Record<string, any> | null | undefined,
 *   channel: Record<string, any> | null | undefined,
 *   parentCategory?: Record<string, any> | null | undefined,
 *   currentUserId?: string | null | undefined,
 *   onJoinGuild?: (() => void) | null,
 * }} params
 */
export function syncServerAccessBannersAndComposerDom({
  doc = typeof document !== "undefined" ? document : null,
  guild,
  channel,
  parentCategory = null,
  currentUserId = "",
  onJoinGuild = null,
}) {
  const access = evaluateChannelAccessDto({
    guild,
    channel,
    parentCategory,
    currentUserId,
  });

  if (!doc || typeof doc.getElementById !== "function") {
    return access;
  }

  const compForm = doc.getElementById("composer");
  const compInput = doc.getElementById("composer-input");
  const compSubmit =
    doc.getElementById("composer-submit") ??
    compForm?.querySelector?.('button[type="submit"]');
  const bannerHost = compForm?.parentElement ?? doc.querySelector?.(".chat-main") ?? doc.body;

  const existingLurkerBanner = doc.getElementById("guild-preview-join-banner");
  const existingPendingBanner = doc.getElementById("guild-pending-screening-banner");
  const existingReadOnlyBanner = doc.getElementById("channel-readonly-access-banner");

  const chType = Number(channel?.type ?? 0);
  const isVoiceOrStage = chType === 2 || chType === 13;

  // 1. Баннер режима предпросмотра (lurker)
  if (access.isLurker) {
    existingPendingBanner?.remove?.();
    existingReadOnlyBanner?.remove?.();
    if (bannerHost && typeof doc.createElement === "function") {
      const banner = existingLurkerBanner || doc.createElement("div");
      banner.id = "guild-preview-join-banner";
      banner.setAttribute("data-access-banner", "lurker");
      if (banner.style) {
        banner.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:10px 16px;margin:0 16px 8px;border-radius:8px;background:rgba(88,101,242,0.18);border:1px solid rgba(88,101,242,0.45);color:var(--text-normal,#f2f3f5);font-size:13px;";
      }
      if (!existingLurkerBanner) {
        const label = doc.createElement("span");
        label.textContent =
          "Вы просматриваете этот сервер в режиме предпросмотра. Вступите на сервер, чтобы писать в чат и заходить в голосовые каналы.";
        banner.append(label);
        if (typeof onJoinGuild === "function") {
          const joinBtn = doc.createElement("button");
          joinBtn.type = "button";
          joinBtn.className = "btn-primary";
          joinBtn.textContent = "Присоединиться к серверу";
          joinBtn.addEventListener("click", onJoinGuild);
          banner.append(joinBtn);
        }
        if (compForm && typeof bannerHost.insertBefore === "function") {
          bannerHost.insertBefore(banner, compForm);
        } else {
          bannerHost.append(banner);
        }
      }
    }
  } else {
    existingLurkerBanner?.remove?.();
  }

  // 2. Баннер ожидания принятия правил сервера (pending: true / Membership Screening)
  if (!access.isLurker && access.isPending) {
    existingReadOnlyBanner?.remove?.();
    if (bannerHost && typeof doc.createElement === "function") {
      const banner = existingPendingBanner || doc.createElement("div");
      banner.id = "guild-pending-screening-banner";
      banner.setAttribute("data-access-banner", "pending");
      if (banner.style) {
        banner.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:10px 16px;margin:0 16px 8px;border-radius:8px;background:rgba(240,178,50,0.16);border:1px solid rgba(240,178,50,0.45);color:var(--text-normal,#f2f3f5);font-size:13px;";
      }
      banner.textContent =
        "Примите правила сервера в Discord, чтобы отправлять сообщения и подключаться к голосовым каналам.";
      if (!existingPendingBanner) {
        if (compForm && typeof bannerHost.insertBefore === "function") {
          bannerHost.insertBefore(banner, compForm);
        } else {
          bannerHost.append(banner);
        }
      }
    }
  } else {
    existingPendingBanner?.remove?.();
  }

  // 3. Баннер отсутствия прав SEND_MESSAGES или CONNECT
  const missingConnect = !access.isLurker && !access.isPending && isVoiceOrStage && !access.canConnectVoice;
  const missingSend = !access.isLurker && !access.isPending && !isVoiceOrStage && !access.canSendMessages;

  if (missingConnect || missingSend) {
    if (bannerHost && typeof doc.createElement === "function") {
      const banner = existingReadOnlyBanner || doc.createElement("div");
      banner.id = "channel-readonly-access-banner";
      banner.setAttribute(
        "data-access-banner",
        missingConnect ? "missing-connect" : "missing-send-messages",
      );
      if (banner.style) {
        banner.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:8px 16px;margin:0 16px 8px;border-radius:8px;background:rgba(242,63,67,0.14);border:1px solid rgba(242,63,67,0.4);color:var(--text-normal,#f2f3f5);font-size:12px;";
      }
      banner.textContent = missingConnect
        ? "У вашей роли нет права CONNECT для входа в этот голосовой канал."
        : "У вас нет прав на отправку сообщений в этом канале.";
      if (!existingReadOnlyBanner) {
        if (compForm && typeof bannerHost.insertBefore === "function") {
          bannerHost.insertBefore(banner, compForm);
        } else {
          bannerHost.append(banner);
        }
      }
    }
  } else {
    existingReadOnlyBanner?.remove?.();
  }

  // 4. Синхронизация поля ввода #composer-input и кнопки отправки
  const shouldBlockInput = Boolean(access.readOnly || !access.canSendMessages);
  if (compInput) {
    compInput.disabled = shouldBlockInput;
    compInput.setAttribute?.("aria-disabled", shouldBlockInput ? "true" : "false");
    if (shouldBlockInput) {
      if (access.isLurker) {
        compInput.placeholder =
          "Режим предпросмотра сервера (вы не участник) — вступите на сервер, чтобы отправлять сообщения и ставить реакции.";
      } else if (access.isPending) {
        compInput.placeholder = "Примите правила сервера в Discord, чтобы отправлять сообщения.";
      } else {
        compInput.placeholder = "У вас нет прав на отправку сообщений в этом канале.";
      }
    } else if (channel?.name) {
      const isDm = chType === 1 || chType === 3 || !guild;
      compInput.placeholder = isDm
        ? `Написать @${channel.name}`
        : `Написать в #${channel.name}`;
    }
  }
  if (compSubmit) {
    compSubmit.disabled = shouldBlockInput;
    compSubmit.setAttribute?.("aria-disabled", shouldBlockInput ? "true" : "false");
  }

  return access;
}
