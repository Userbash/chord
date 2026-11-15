// @ts-check
/**
 * Voice state synchronization, signal metrics, channel permissions, and participant queries.
 * Extracted from `app.js` as part of RW51-EPIC2-T2.
 * @module ui/features/voice-state-sync
 */

import { resolveVoiceCallPhaseView } from "../modules/voice-rtc-panel.js";
import { isConfirmedVoiceStateRecord } from "../modules/channel-tree-view.js";
import { HOME_GUILD_ID } from "./dm-friends.js";

/**
 * Create voice state sync and participant query helpers bound to the shared voice context.
 * @param {any} ctx
 */
export function createVoiceStateSync(ctx) {
  /**
   * Computes RTC connection signal quality metrics (ping ms, packet loss %, quality indicator)
   * matching Discord's reference thresholds (`RTCConnection.quality`: >500ms / >10% loss = bad,
   * >250ms / >5% loss = average).
   * @param {any} status
   */
  function computeRtcSignalMetrics(status) {
    const phaseView = resolveVoiceCallPhaseView(status);
    const st = String(status?.state || "signaling");
    const isErr =
      phaseView.isError ||
      phaseView.phase === "timeout" ||
      phaseView.phase === "error" ||
      st === "error" ||
      st === "timeout" ||
      st.startsWith("error");
    const isDisconnected = !isErr && (phaseView.phase === "disconnected" || st === "disconnected");
    const isConnecting =
      !isErr &&
      (phaseView.phase === "signaling" ||
        phaseView.phase === "negotiating" ||
        st === "connecting" ||
        st === "reconnecting" ||
        isDisconnected ||
        (ctx.isLiveSession() && !phaseView.isConnected));
    const rawLast = status?.lastPingMs;
    const rawAvg = status?.averagePingMs;
    const hasMeasuredPing = typeof rawLast === "number" && Number.isFinite(rawLast) && rawLast > 0;
    const effectiveLast = hasMeasuredPing ? Math.round(rawLast) : 0;
    const effectiveAvg =
      typeof rawAvg === "number" && Number.isFinite(rawAvg) && rawAvg > 0
        ? Math.round(rawAvg)
        : effectiveLast;
    const rawLoss = Number(status?.packetLossPct ?? status?.packetLossPercent ?? status?.outboundPacketLossPct ?? 0);
    const lossPct = Number.isFinite(rawLoss) && rawLoss >= 0 ? rawLoss : 0;
    const lossTxt = `${lossPct.toFixed(1)}%`;
    /** @type {"fine" | "average" | "bad"} */
    const quality =
      isErr || effectiveLast > 500 || lossPct > 10
        ? "bad"
        : isConnecting || effectiveLast > 250 || lossPct > 5
          ? "average"
          : "fine";
    const isAfkStatus = Boolean(phaseView.phase === "afk" || status?.isAfk);
    const pingColor = isAfkStatus
      ? "#949ba4"
      : quality === "bad"
        ? "#f23f43"
        : quality === "average"
          ? "#f0b232"
          : "#23a55a";
    const signalSvg =
      quality === "bad"
        ? ctx.SVG_ICONS.signalBad
        : quality === "average"
          ? ctx.SVG_ICONS.signalAverage
          : ctx.SVG_ICONS.signalFine;
    const pillText = isAfkStatus
      ? "AFK"
      : isErr
        ? "— мс"
        : isConnecting && !hasMeasuredPing
          ? "RTC…"
          : `${effectiveLast} мс`;
    return {
      phaseView,
      st,
      isErr,
      isDisconnected,
      isConnecting,
      hasMeasuredPing,
      effectiveLast,
      effectiveAvg,
      lossPct,
      lossTxt,
      quality,
      pingColor,
      signalSvg,
      pillText,
    };
  }

  /**
   * Plays standard Discord UI sound effects for voice and call actions.
   * @param {"join" | "leave" | "user_moved" | "disconnect" | "mute" | "unmute" | "deafen" | "undeafen" | "stream_on" | "stream_off" | "stream_user_joined" | "stream_user_left" | "camera_on" | "camera_off" | "message" | "outgoing_call"} kind
   */
  function playVoiceUiSound(kind) {
    if (!ctx.isLiveSession()) return;
    if (ctx.selfVoicePrefs.deafened && kind !== "deafen" && kind !== "undeafen" && kind !== "disconnect") {
      return;
    }
    const nowSoundMs = Date.now();
    const prevSoundMs = ctx.lastVoiceUiSoundAtByKind.get(kind) ?? 0;
    if (nowSoundMs - prevSoundMs < 200) {
      return;
    }
    ctx.lastVoiceUiSoundAtByKind.set(kind, nowSoundMs);
    const fileMap = {
      join: "user_join.mp3",
      leave: "user_leave.mp3",
      user_moved: "user_moved.mp3",
      disconnect: "disconnect.mp3",
      mute: "mute.mp3",
      unmute: "unmute.mp3",
      deafen: "deafen.mp3",
      undeafen: "undeafen.mp3",
      stream_on: "stream_started.mp3",
      stream_off: "stream_ended.mp3",
      stream_user_joined: "stream_user_joined.mp3",
      stream_user_left: "stream_user_left.mp3",
      camera_on: "stream_started.mp3",
      camera_off: "stream_ended.mp3",
      message: "message1.mp3",
      outgoing_call: "user_join.mp3",
    };
    const fileName = fileMap[kind] || "mute.mp3";
    void ctx
      .invoke(ctx.ipcCmds.setAudioDevice, {
        kind: "ui_sound",
        deviceId: kind,
        volumePercent: 85,
        muted: false,
      })
      .catch(() => {
        try {
          const audio = new globalThis.Audio(`/assets/sounds/${fileName}`);
          audio.volume = 0.75;
          void audio.play().catch(() => {});
        } catch {}
      });
  }

  /**
   * Closes all active voice popouts and optionally screen-share modals.
   * @param {boolean} [includeModals]
   */
  function closeLiveVoicePopouts(includeModals = false) {
    if (ctx.micTestTimer) {
      if (typeof ctx.micTestTimer.stop === "function") void ctx.micTestTimer.stop();
      else clearInterval(ctx.micTestTimer);
      ctx.micTestTimer = null;
      if (ctx.isLiveSession()) {
        void ctx
          .invoke(ctx.ipcCmds.setAudioDevice, {
            kind: "mic_test_stop",
            deviceId: "default",
            volumePercent: 100,
            muted: false,
          })
          .catch(() => {});
      }
    }
    if (ctx.activeVoiceMenuOutsideHandler) {
      document.removeEventListener?.("mousedown", ctx.activeVoiceMenuOutsideHandler, true);
      ctx.activeVoiceMenuOutsideHandler = null;
    }
    const popoutIds = [
      "live-rtc-popout",
      "live-krisp-popout",
      "live-audio-device-popout",
      "live-self-status-popout",
      "live-voice-user-context-menu",
      "live-soundboard-popout",
      "live-activities-popout",
      "live-stream-settings-menu",
      "live-stream-settings-submenu",
      "live-mic-muted-warning-badge",
      "live-msg-context-menu",
      "live-emoji-picker-popout",
    ];
    if (includeModals) {
      popoutIds.push(
        "live-screenshare-modal",
        "live-screenshare-portal-modal",
        "live-nitro-hd-modal",
      );
    }
    for (const id of popoutIds) {
      document.getElementById(id)?.remove();
    }
  }

  /**
   * Determines whether a voice channel is an AFK channel via `guild.afkChannelId`
   * or channel name heuristics.
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @param {string} [channelNameHint]
   * @returns {boolean}
   */
  function isChannelAfk(targetGuildId, targetChannelId, channelNameHint = "") {
    if (!targetChannelId) return false;
    const gObj = targetGuildId ? ctx.liveGuildsById?.get?.(String(targetGuildId)) : null;
    const guildAfkCid = ctx.cleanDiscordStr(gObj?.afkChannelId ?? gObj?.afk_channel_id);
    if (guildAfkCid && guildAfkCid === String(targetChannelId)) {
      return true;
    }
    const activeConn = ctx.getActiveVoiceConnection();
    let chName = String(channelNameHint || "");
    if (!chName && activeConn?.channelId === targetChannelId) {
      chName = String(activeConn.channelName || "");
    }
    if (!chName && typeof document !== "undefined") {
      const domBtn = document.querySelector(`.channel-item[data-channel-id="${targetChannelId}"] .channel-name`);
      if (domBtn?.textContent) chName = domBtn.textContent;
    }
    return /afk|афк|бездейств|sleeping|сон\b|вне\s*компа|zzz|💤|🚾/i.test(chName);
  }

  /**
   * @param {unknown} val
   * @returns {boolean}
   */
  function isTruthyVoiceFlag(val) {
    if (val === true || val === 1) return true;
    if (typeof val === "string") {
      const s = val.trim().toLowerCase();
      return s === "true" || s === "1" || s === "yes";
    }
    return false;
  }

  /**
   * @param {unknown} gid
   * @returns {string}
   */
  function normalizeVoiceGuildKey(gid) {
    const s = ctx.cleanDiscordStr(String(gid ?? ""));
    return !s || s === HOME_GUILD_ID ? HOME_GUILD_ID : s;
  }

  /**
   * Atomically merges incoming voice state records for a guild with key normalization
   * and preservation of confirmed participants.
   * @param {Map<string, any[]>} voiceStatesMap
   * @param {unknown} rawGuildId
   * @param {any[] | null | undefined} incomingStates
   * @returns {any[]}
   */
  function mergeVoiceStatesForGuild(voiceStatesMap, rawGuildId, incomingStates) {
    if (!voiceStatesMap || typeof voiceStatesMap.get !== "function") return [];
    const normGid = normalizeVoiceGuildKey(rawGuildId);
    const rawKey = typeof rawGuildId === "string" ? rawGuildId : "";
    /** @type {any[]} */
    const combinedExisting = [];
    const aliasKeys = new Set([normGid]);
    if (rawKey && rawKey !== normGid) aliasKeys.add(rawKey);
    if (normGid === HOME_GUILD_ID) {
      aliasKeys.add("");
      aliasKeys.add("@me");
    }
    for (const k of aliasKeys) {
      const list = voiceStatesMap.get(k);
      if (Array.isArray(list)) {
        combinedExisting.push(...list);
      }
      if (k !== normGid && voiceStatesMap.has(k)) {
        voiceStatesMap.delete(k);
      }
    }

    /** @type {Map<string, any>} */
    const byUser = new Map();
    for (const prev of combinedExisting) {
      if (!isConfirmedVoiceStateRecord(prev)) continue;
      const uid = ctx.cleanDiscordStr(
        String(prev?.user_id ?? prev?.userId ?? prev?.member?.user?.id ?? prev?.user?.id ?? ""),
      );
      const cid = ctx.cleanDiscordStr(String(prev?.channel_id ?? prev?.channelId ?? ""));
      if (!uid || !cid) continue;
      byUser.set(uid, {
        ...prev,
        guild_id: normGid,
        guildId: normGid,
        channel_id: cid,
        channelId: cid,
        user_id: uid,
        userId: uid,
        confirmed: true,
      });
    }

    if (Array.isArray(incomingStates)) {
      for (const inc of incomingStates) {
        if (!inc || typeof inc !== "object") continue;
        const uid = ctx.cleanDiscordStr(
          String(inc.user_id ?? inc.userId ?? inc.member?.user?.id ?? inc.user?.id ?? ""),
        );
        if (!uid) continue;

        const rawCid =
          inc.channel_id === null || inc.channelId === null
            ? ""
            : ctx.cleanDiscordStr(String(inc.channel_id ?? inc.channelId ?? ""));
        const phase = String(inc.phase ?? inc.state ?? "").trim().toLowerCase();
        const isExplicitRemoval =
          !rawCid ||
          phase === "disconnected" ||
          phase === "timeout" ||
          phase === "error" ||
          inc.timeout === true ||
          Boolean(inc.error);
        if (isExplicitRemoval) {
          byUser.delete(uid);
          continue;
        }

        if (!isConfirmedVoiceStateRecord(inc)) {
          continue;
        }

        const prev = byUser.get(uid);
        const prevMember = prev?.member && typeof prev.member === "object" ? prev.member : null;
        const incMember = inc.member && typeof inc.member === "object" ? inc.member : null;
        const prevUser = prev?.user && typeof prev.user === "object" ? prev.user : null;
        const incUser = inc.user && typeof inc.user === "object" ? inc.user : null;

        const selfMute = isTruthyVoiceFlag(
          inc.self_mute ?? inc.selfMute ?? prev?.self_mute ?? prev?.selfMute,
        );
        const selfDeaf = isTruthyVoiceFlag(
          inc.self_deaf ?? inc.selfDeaf ?? prev?.self_deaf ?? prev?.selfDeaf,
        );
        const selfStream = isTruthyVoiceFlag(
          inc.self_stream ?? inc.selfStream ?? prev?.self_stream ?? prev?.selfStream,
        );
        const selfVideo = isTruthyVoiceFlag(
          inc.self_video ?? inc.selfVideo ?? prev?.self_video ?? prev?.selfVideo,
        );
        const mute = isTruthyVoiceFlag(inc.mute ?? prev?.mute);
        const deaf = isTruthyVoiceFlag(inc.deaf ?? prev?.deaf);
        const suppress = isTruthyVoiceFlag(inc.suppress ?? prev?.suppress);

        /** @type {any} */
        const merged = {
          ...(prev ?? {}),
          ...inc,
          guild_id: normGid,
          guildId: normGid,
          channel_id: rawCid,
          channelId: rawCid,
          user_id: uid,
          userId: uid,
          self_mute: selfMute,
          selfMute,
          self_deaf: selfDeaf,
          selfDeaf,
          self_stream: selfStream,
          selfStream,
          self_video: selfVideo,
          selfVideo,
          mute,
          deaf,
          suppress,
          confirmed: true,
        };

        if (prevMember || incMember) {
          const mergedMember = {
            ...(prevMember ?? {}),
            ...(incMember ?? {}),
          };
          if (
            Array.isArray(prevMember?.roles) &&
            prevMember.roles.length > 0 &&
            (!Array.isArray(incMember?.roles) || incMember.roles.length === 0)
          ) {
            mergedMember.roles = Array.from(prevMember.roles);
          }
          if (ctx.cleanDiscordStr(prevMember?.nick) && !ctx.cleanDiscordStr(incMember?.nick)) {
            mergedMember.nick = prevMember.nick;
          }
          if (prevMember?.user || incMember?.user) {
            mergedMember.user = {
              ...(prevMember?.user && typeof prevMember.user === "object" ? prevMember.user : {}),
              ...(incMember?.user && typeof incMember.user === "object" ? incMember.user : {}),
            };
          }
          merged.member = mergedMember;
        }

        if (prevUser || incUser) {
          merged.user = {
            ...(prevUser ?? {}),
            ...(incUser ?? {}),
          };
        }

        byUser.set(uid, merged);
      }
    }

    const mergedList = Array.from(byUser.values());
    voiceStatesMap.set(normGid, mergedList);
    return mergedList;
  }

  /**
   * Atomically merges a snapshot of voice states across all guilds without destructive `clear()`.
   * @param {Map<string, any[]>} voiceStatesMap
   * @param {Record<string, any> | null | undefined} rawVs
   * @returns {string}
   */
  function mergeVoiceStatesSnapshot(voiceStatesMap, rawVs) {
    if (!voiceStatesMap || typeof voiceStatesMap.entries !== "function") return "";
    for (const [existingKey, existingArr] of Array.from(voiceStatesMap.entries())) {
      const normKey = normalizeVoiceGuildKey(existingKey);
      if (normKey !== existingKey) {
        mergeVoiceStatesForGuild(voiceStatesMap, existingKey, existingArr);
      }
    }
    if (rawVs && typeof rawVs === "object") {
      for (const [gid, arr] of Object.entries(rawVs)) {
        if (Array.isArray(arr)) {
          mergeVoiceStatesForGuild(voiceStatesMap, gid, arr);
        }
      }
    }
    const parts = [];
    const sortedEntries = Array.from(voiceStatesMap.entries()).sort(([a], [b]) =>
      String(a).localeCompare(String(b)),
    );
    for (const [gid, arr] of sortedEntries) {
      if (Array.isArray(arr) && arr.length > 0) {
        parts.push(
          `${gid}:${arr
            .map(
              (v) =>
                `${v?.user_id ?? v?.userId}:${v?.channel_id ?? v?.channelId}:${Boolean(v?.self_mute ?? v?.selfMute)}:${Boolean(v?.self_deaf ?? v?.selfDeaf)}:${Boolean(v?.self_stream ?? v?.selfStream)}:${Boolean(v?.mute)}:${Boolean(v?.deaf)}`,
            )
            .join(",")}`,
        );
      }
    }
    return parts.join("|");
  }

  /**
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @returns {boolean}
   */
  function isVoiceChannelConnected(targetGuildId, targetChannelId) {
    const activeConn = ctx.getActiveVoiceConnection();
    return Boolean(
      activeConn &&
        normalizeVoiceGuildKey(activeConn.guildId) === normalizeVoiceGuildKey(targetGuildId) &&
        activeConn.channelId === targetChannelId,
    );
  }

  /**
   * Formats the active call duration timer for a voice channel.
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @param {number} count
   * @returns {string}
   */
  function getVoiceChannelTimer(targetGuildId, targetChannelId, count) {
    const normGuild = normalizeVoiceGuildKey(targetGuildId);
    const key = `${normGuild}:${targetChannelId}`;
    if (count <= 0) {
      ctx.liveVoiceChannelStartEpochMs.delete(key);
      return "голос";
    }
    let startMs = ctx.liveVoiceChannelStartEpochMs.get(key);
    if (!startMs) {
      const activeConn = ctx.getActiveVoiceConnection();
      if (normGuild === HOME_GUILD_ID) {
        startMs = activeConn?.connectedAtMs || Date.now();
      } else {
        let seed = 0;
        for (let i = 0; i < targetChannelId.length; i += 1) {
          seed = (seed * 31 + targetChannelId.charCodeAt(i)) >>> 0;
        }
        const initialOffsetSec = 180 + (seed % 5400);
        startMs = Date.now() - initialOffsetSec * 1000;
      }
      ctx.liveVoiceChannelStartEpochMs.set(key, startMs);
    }
    const elapsedSec = Math.max(1, Math.floor((Date.now() - startMs) / 1000));
    const hrs = Math.floor(elapsedSec / 3600);
    const mins = Math.floor((elapsedSec % 3600) / 60);
    const secs = elapsedSec % 60;
    if (hrs > 0) {
      return `${hrs}:${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
    }
    return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }

  /**
   * Resolves all confirmed participants currently in a voice channel.
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @returns {Array<any>}
   */
  function getVoiceParticipants(targetGuildId, targetChannelId) {
    const normGuildKey = normalizeVoiceGuildKey(targetGuildId);
    const rawList =
      normGuildKey === HOME_GUILD_ID
        ? [
            ...(ctx.liveVoiceStatesByGuild.get(HOME_GUILD_ID) ?? []),
            ...(ctx.liveVoiceStatesByGuild.get("") ?? []),
          ]
        : [
            ...(ctx.liveVoiceStatesByGuild.get(targetGuildId) ?? []),
            ...(normGuildKey && normGuildKey !== targetGuildId
              ? ctx.liveVoiceStatesByGuild.get(normGuildKey) ?? []
              : []),
          ];
    /** @type {Map<string, any>} */
    const byUser = new Map();
    const selfUser = ctx.getLiveSelfUser();
    const selfId = ctx.cleanDiscordStr(String(selfUser?.id || ""));
    const channelIsAfk = isChannelAfk(targetGuildId, targetChannelId);
    const activeConn = ctx.getActiveVoiceConnection();
    const rtcStatus = ctx.getLiveRtcStatus();

    for (const vs of rawList) {
      if (!isConfirmedVoiceStateRecord(vs)) continue;
      const cid = ctx.cleanDiscordStr(String(vs?.channel_id ?? vs?.channelId ?? ""));
      const uid = ctx.cleanDiscordStr(String(vs?.user_id ?? vs?.userId ?? ""));
      if (!uid || !cid || cid !== targetChannelId) continue;
      const member = vs?.member && typeof vs.member === "object" ? vs.member : {};
      const activeDmRec = /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__;
      let dmKnownUser =
        activeDmRec && String(activeDmRec.id || "") === uid ? activeDmRec : null;
      if (!dmKnownUser && normGuildKey === HOME_GUILD_ID) {
        const pcs = ctx.getDmFriendsController?.()?.getPrivateChannels?.() ?? [];
        for (const pc of pcs) {
          const anyPc = /** @type {any} */ (pc);
          const recs = Array.isArray(anyPc?.rawRecipients)
            ? anyPc.rawRecipients
            : Array.isArray(anyPc?.recipients)
              ? anyPc.recipients
              : [];
          const foundRec = recs.find((/** @type {any} */ r) => String(r?.id || "") === uid);
          if (foundRec) {
            dmKnownUser = foundRec;
            break;
          }
        }
      }
      const u = ctx.liveUsersById.get(uid) ?? dmKnownUser ?? vs?.user ?? member?.user ?? {};
      if (u && typeof u === "object" && uid) {
        ctx.liveUsersById.set(uid, { ...(ctx.liveUsersById.get(uid) ?? {}), ...u, id: uid });
      }
      const userOverride = ctx.livePerUserVoiceState.get(uid);
      const rawRoles = Array.isArray(member?.roles)
        ? member.roles.map(String)
        : ctx.liveMemberRolesByGuildAndUser?.get(`${targetGuildId}:${uid}`) ?? [];
      if (rawRoles.length > 0) {
        ctx.liveMemberRolesByGuildAndUser?.set(`${targetGuildId}:${uid}`, rawRoles);
      }
      const roleInfo = ctx.resolveUserRoleInfo(targetGuildId, uid);
      const baseName =
        ctx.cleanDiscordStr(userOverride?.nickname) ||
        ctx.cleanDiscordStr(member?.nick) ||
        ctx.cleanDiscordStr(u?.global_name) ||
        ctx.cleanDiscordStr(u?.globalName) ||
        ctx.cleanDiscordStr(u?.display_name) ||
        ctx.cleanDiscordStr(u?.displayName) ||
        ctx.cleanDiscordStr(u?.username) ||
        ctx.cleanDiscordStr(member?.user?.global_name) ||
        ctx.cleanDiscordStr(member?.user?.username) ||
        (uid === selfId
          ? ctx.cleanDiscordStr(selfUser?.globalName) ||
            ctx.cleanDiscordStr(selfUser?.global_name) ||
            ctx.cleanDiscordStr(selfUser?.username) ||
            "Пользователь"
          : `Участник ${uid.slice(-4)}`);
      const clanTag =
        ctx.cleanDiscordStr(u?.clan?.tag) ||
        ctx.cleanDiscordStr(u?.primary_guild?.tag) ||
        ctx.cleanDiscordStr(member?.user?.clan?.tag) ||
        "";
      const avatarHash =
        ctx.cleanDiscordStr(member?.avatar) ||
        ctx.cleanDiscordStr(u?.avatar) ||
        ctx.cleanDiscordStr(member?.user?.avatar) ||
        (uid === selfId ? ctx.cleanDiscordStr(selfUser?.avatar) : "") ||
        null;
      const avatarUrl = uid && avatarHash
        ? `https://cdn.discordapp.com/avatars/${uid}/${avatarHash}.png?size=64`
        : null;
      const isSuppressed = channelIsAfk || isTruthyVoiceFlag(vs?.suppress);
      const selfMute =
        uid === selfId && activeConn
          ? Boolean(ctx.selfVoicePrefs.muted || channelIsAfk)
          : isTruthyVoiceFlag(vs?.self_mute ?? vs?.selfMute);
      const serverMute = isSuppressed || (userOverride?.serverMute !== undefined
        ? Boolean(userOverride.serverMute)
        : isTruthyVoiceFlag(vs?.mute));
      const selfDeaf =
        uid === selfId && activeConn
          ? Boolean(ctx.selfVoicePrefs.deafened)
          : isTruthyVoiceFlag(vs?.self_deaf ?? vs?.selfDeaf);
      const serverDeaf = userOverride?.serverDeaf !== undefined
        ? Boolean(userOverride.serverDeaf)
        : isTruthyVoiceFlag(vs?.deaf);
      const localMute = Boolean(userOverride?.localMute);
      const deafened = selfDeaf || serverDeaf;
      const muted = selfMute || serverMute || deafened || localMute || isSuppressed;
      const streaming = isTruthyVoiceFlag(vs?.self_stream ?? vs?.selfStream);
      const video = isTruthyVoiceFlag(vs?.self_video ?? vs?.selfVideo);
      const speakingSet = new Set(
        Array.isArray(rtcStatus?.speakingUsers) ? rtcStatus.speakingUsers.map(String) : [],
      );
      const speaking = !channelIsAfk && !isSuppressed && !muted && !deafened && speakingSet.has(uid);
      const resolvedClanTag =
        clanTag ||
        ctx.liveMemberClanTagByUser?.get(uid) ||
        "";
      const gameActivity = null;
      byUser.set(uid, {
        userId: uid,
        guildId: targetGuildId,
        channelId: targetChannelId,
        displayName: baseName,
        username: ctx.cleanDiscordStr(u?.username) || baseName,
        clanTag: resolvedClanTag,
        avatarUrl,
        roleColor: roleInfo.colorHex,
        topRoleName: roleInfo.topRoleName,
        muted,
        deafened,
        selfMute,
        serverMute,
        selfDeaf,
        serverDeaf,
        localMute,
        streaming,
        video,
        speaking,
        gameActivity,
        roles: roleInfo.roles.length > 0 ? roleInfo.roles : rawRoles,
        joinedAt: ctx.cleanDiscordStr(member?.joined_at) || "",
      });
    }
    const existingSelf = byUser.get(selfId);
    if (
      existingSelf &&
      activeConn &&
      normalizeVoiceGuildKey(activeConn.guildId) === normGuildKey &&
      activeConn.channelId === targetChannelId
    ) {
      const selfName =
        ctx.cleanDiscordStr(ctx.livePerUserVoiceState.get(selfId)?.nickname) ||
        ctx.cleanDiscordStr(selfUser?.globalName) ||
        ctx.cleanDiscordStr(selfUser?.global_name) ||
        ctx.cleanDiscordStr(selfUser?.username) ||
        "Пользователь";
      const selfAvatarHash = ctx.cleanDiscordStr(selfUser?.avatar);
      const selfAvatar = selfAvatarHash
        ? `https://cdn.discordapp.com/avatars/${selfId}/${selfAvatarHash}.png?size=64`
        : null;
      const selfOverride = ctx.livePerUserVoiceState.get(selfId);
      const serverMute = channelIsAfk || Boolean(selfOverride?.serverMute ?? existingSelf?.serverMute ?? false);
      const serverDeaf = Boolean(selfOverride?.serverDeaf ?? existingSelf?.serverDeaf ?? false);
      const deafened = Boolean(ctx.selfVoicePrefs.deafened || serverDeaf);
      const muted = Boolean(channelIsAfk || ctx.selfVoicePrefs.muted || serverMute || deafened);
      const speakingSet = new Set(
        Array.isArray(rtcStatus?.speakingUsers) ? rtcStatus.speakingUsers.map(String) : [],
      );
      const selfSpeaking =
        !channelIsAfk && !muted && !deafened && (speakingSet.has(selfId) || Boolean(rtcStatus?.localMicSpeaking));
      const selfRoleInfo = ctx.resolveUserRoleInfo(targetGuildId, selfId);
      byUser.set(selfId, {
        ...(existingSelf ?? {}),
        userId: selfId,
        guildId: targetGuildId,
        channelId: targetChannelId,
        displayName: selfName,
        username: ctx.cleanDiscordStr(selfUser?.username) || "user",
        clanTag: existingSelf?.clanTag || "",
        avatarUrl: selfAvatar || existingSelf?.avatarUrl || null,
        roleColor: selfRoleInfo.colorHex ?? existingSelf?.roleColor ?? null,
        topRoleName: selfRoleInfo.topRoleName || existingSelf?.topRoleName || "",
        muted,
        deafened,
        selfMute: Boolean(ctx.selfVoicePrefs.muted || channelIsAfk),
        serverMute: Boolean(serverMute),
        selfDeaf: Boolean(ctx.selfVoicePrefs.deafened),
        serverDeaf: Boolean(serverDeaf),
        localMute: false,
        streaming: Boolean(activeConn.screenShareOn),
        video: Boolean(activeConn.cameraOn),
        speaking: selfSpeaking,
        roles:
          selfRoleInfo.roles.length > 0
            ? selfRoleInfo.roles
            : existingSelf?.roles ?? ctx.liveMemberRolesByGuildAndUser?.get(`${targetGuildId}:${selfId}`) ?? [],
        joinedAt: existingSelf?.joinedAt || "24 сент. 2023 г.",
      });
    }
    return [...byUser.values()];
  }

  /**
   * Displays a temporary floating voice toast notice at the bottom of the screen.
   * @param {string} message
   */
  function showVoiceToastNotice(message) {
    if (typeof document === "undefined" || !document.body) return;
    document.getElementById("live-voice-toast-notice")?.remove();
    const toast = document.createElement("div");
    toast.id = "live-voice-toast-notice";
    toast.setAttribute("role", "status");
    toast.style.cssText =
      "position:fixed;bottom:76px;left:50%;transform:translateX(-50%);background:#111214;color:#f2f3f5;border:1px solid #f23f43;border-radius:8px;padding:10px 16px;font-size:13px;font-weight:600;box-shadow:0 12px 32px rgba(0,0,0,0.75);z-index:10200;display:flex;align-items:center;gap:8px;pointer-events:none;";
    toast.textContent = `⚠️ ${message}`;
    document.body.append(toast);
    setTimeout(() => {
      toast.remove();
    }, 3200);
  }

  /**
   * Resolves a channel object by ID or enriches a partial channel object with overwrites and parentId.
   * @param {any} channelOrId
   * @param {any[]} [allChannels]
   * @param {string} [targetGuildId]
   * @returns {any | null}
   */
  function resolveChannelObject(channelOrId, allChannels, targetGuildId) {
    const gId = ctx.cleanDiscordStr(targetGuildId || ctx.getRailSelectedId?.() || ctx.selectedGuildId() || "");
    const guildChs = gId ? ctx.liveGuildsById?.get?.(gId)?.channels : null;
    const treeChannels = ctx.getTreeChannels?.() ?? [];
    const chList = [
      ...(Array.isArray(allChannels) ? allChannels : []),
      ...(Array.isArray(treeChannels) ? treeChannels : []),
      ...(Array.isArray(guildChs) ? guildChs : []),
    ];
    if (channelOrId && typeof channelOrId === "object") {
      const hasOw =
        Array.isArray(channelOrId.permissionOverwrites) ||
        Array.isArray(channelOrId.permission_overwrites);
      const cid = ctx.cleanDiscordStr(String(channelOrId.id ?? ""));
      const found = cid ? chList.find((c) => String(c?.id ?? "") === cid) : null;
      if (hasOw || !found) {
        return channelOrId;
      }
      return {
        ...found,
        ...channelOrId,
        permissionOverwrites:
          channelOrId.permissionOverwrites ??
          channelOrId.permission_overwrites ??
          found.permissionOverwrites ??
          found.permission_overwrites,
        permission_overwrites:
          channelOrId.permission_overwrites ??
          channelOrId.permissionOverwrites ??
          found.permission_overwrites ??
          found.permissionOverwrites,
        parentId:
          channelOrId.parentId ??
          channelOrId.parent_id ??
          found.parentId ??
          found.parent_id,
      };
    }
    const cid = ctx.cleanDiscordStr(String(channelOrId ?? ""));
    if (!cid) return null;
    return chList.find((c) => String(c?.id ?? "") === cid) ?? { id: cid };
  }

  /**
   * Checks whether the current user has `STREAM` permission in the voice channel.
   * @param {string} targetGuildId
   * @param {any} channelOrId
   * @param {any[]} [allChannels]
   * @returns {boolean}
   */
  function canStreamInChannel(targetGuildId, channelOrId, allChannels) {
    if (!ctx.isLiveSession()) return true;
    const gId = ctx.cleanDiscordStr(targetGuildId);
    const chObj = resolveChannelObject(channelOrId, allChannels);
    const cId = ctx.cleanDiscordStr(String(chObj?.id ?? channelOrId ?? ""));
    if (!gId || !cId) return true;
    if (isChannelAfk(gId, cId)) return false;
    const selfId = ctx.cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
    const vList = ctx.liveVoiceStatesByGuild.get(gId) ?? [];
    const myVs = vList.find((/** @type {any} */ v) => String(v?.user_id ?? v?.userId ?? "") === selfId);
    if (myVs && isTruthyVoiceFlag(myVs.suppress)) return false;
    const REQUIRED = (1n << 10n) | (1n << 20n) | (1n << 9n);
    const perms = ctx.computeEffectiveChannelPermissions(gId, chObj, allChannels);
    if (perms === null) return true;
    return (perms & REQUIRED) === REQUIRED;
  }

  /**
   * Checks whether the current user has `SPEAK` permission in the voice channel.
   * @param {string} targetGuildId
   * @param {any} channelOrId
   * @param {any[]} [allChannels]
   * @returns {boolean}
   */
  function canSpeakInChannel(targetGuildId, channelOrId, allChannels) {
    if (!ctx.isLiveSession()) return true;
    const gId = ctx.cleanDiscordStr(targetGuildId);
    const chObj = resolveChannelObject(channelOrId, allChannels);
    const cId = ctx.cleanDiscordStr(String(chObj?.id ?? channelOrId ?? ""));
    if (!gId || !cId) return true;
    if (isChannelAfk(gId, cId)) return false;
    const selfId = ctx.cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
    const vList = ctx.liveVoiceStatesByGuild.get(gId) ?? [];
    const myVs = vList.find((/** @type {any} */ v) => String(v?.user_id ?? v?.userId ?? "") === selfId);
    if (myVs && (isTruthyVoiceFlag(myVs.mute) || isTruthyVoiceFlag(myVs.suppress))) return false;
    const REQUIRED = (1n << 10n) | (1n << 20n) | (1n << 21n);
    const perms = ctx.computeEffectiveChannelPermissions(gId, chObj, allChannels);
    if (perms === null) return true;
    return (perms & REQUIRED) === REQUIRED;
  }

  /**
   * Checks whether channel access is restricted by `@everyone` overwrites or role rules.
   * @param {string} targetGuildId
   * @param {any} channel
   * @param {any[]} [allChannels]
   * @returns {boolean}
   */
  function isChannelRoleRestricted(targetGuildId, channel, allChannels) {
    if (!channel || typeof channel !== "object") return false;
    const VIEW_OR_CONNECT = (1n << 10n) | (1n << 20n);
    /** @param {any[]} overwrites */
    const checkOverwrites = (overwrites) => {
      if (!Array.isArray(overwrites) || overwrites.length === 0) return false;
      let everyoneDenied = false;
      let roleOrMemberAllowed = false;
      for (const ow of overwrites) {
        const owId = String(ow?.id ?? "");
        let deny = 0n;
        let allow = 0n;
        try {
          deny = BigInt(String(ow?.deny ?? "0"));
        } catch {}
        try {
          allow = BigInt(String(ow?.allow ?? "0"));
        } catch {}
        if (owId === targetGuildId) {
          if ((deny & VIEW_OR_CONNECT) !== 0n) {
            everyoneDenied = true;
          }
        } else if ((allow & VIEW_OR_CONNECT) !== 0n) {
          roleOrMemberAllowed = true;
        }
      }
      return everyoneDenied || (overwrites.length > 0 && roleOrMemberAllowed && everyoneDenied);
    };
    const ownOverwrites = channel.permissionOverwrites ?? channel.permission_overwrites ?? [];
    if (checkOverwrites(ownOverwrites)) return true;
    const parentId = ctx.cleanDiscordStr(String(channel.parentId ?? channel.parent_id ?? ""));
    const chList = Array.isArray(allChannels) ? allChannels : ctx.getTreeChannels?.() ?? [];
    if (parentId && chList.length > 0) {
      const parentCh = chList.find((/** @type {any} */ c) => String(c?.id ?? "") === parentId);
      const parentOw = /** @type {any} */ (parentCh)?.permissionOverwrites ?? /** @type {any} */ (parentCh)?.permission_overwrites ?? [];
      if (checkOverwrites(parentOw)) return true;
    }
    const chName = String(channel.name || "");
    if (chName.includes("🔒") || chName.includes("🛑")) return true;
    return false;
  }

  /**
   * Returns remaining ms before hiding the "Invite to Voice Chat" banner when alone (30s window).
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @returns {number}
   */
  function getAloneInviteRemainingMs(targetGuildId, targetChannelId) {
    const activeConn = ctx.getActiveVoiceConnection();
    if (
      !activeConn ||
      activeConn.guildId !== targetGuildId ||
      activeConn.channelId !== targetChannelId
    ) {
      return 0;
    }
    const connectedAt = Number(activeConn.connectedAtMs || Date.now());
    const elapsed = Math.max(0, Date.now() - connectedAt);
    return Math.max(0, 30_000 - elapsed);
  }

  return {
    computeRtcSignalMetrics,
    playVoiceUiSound,
    closeLiveVoicePopouts,
    isChannelAfk,
    isTruthyVoiceFlag,
    normalizeVoiceGuildKey,
    mergeVoiceStatesForGuild,
    mergeVoiceStatesSnapshot,
    isVoiceChannelConnected,
    getVoiceChannelTimer,
    getVoiceParticipants,
    showVoiceToastNotice,
    resolveChannelObject,
    canStreamInChannel,
    canSpeakInChannel,
    isChannelRoleRestricted,
    getAloneInviteRemainingMs,
  };
}
