// @ts-check
/**
 * DM call stage banner rendering, incoming call overlay, and ringing tone loop.
 * Extracted from `app.js` as part of RW51-EPIC2-T2.
 * @module ui/features/voice-dm-call-stage
 */

import { HOME_GUILD_ID } from "./dm-friends.js";
import { createVoiceCallStage } from "./voice-call-stage.js";

const SVG_PHONE_DISCONNECT = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08a.956.956 0 0 1-.29-.7c0-.28.11-.53.29-.71C3.34 8.78 7.46 7 12 7s8.66 1.78 11.71 4.67c.18.18.29.43.29.71 0 .28-.11.53-.29.71l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28a11.27 11.27 0 0 0-2.67-1.85.996.996 0 0 1-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z"/></svg>`;

/**
 * Create the DM call stage and incoming call overlay controller.
 * @param {any} ctx
 */
export function createVoiceDmCallStage(ctx) {
  /** @type {any} */
  let ringingAudioLoopTimer = null;
  /** @type {ReturnType<typeof createVoiceCallStage> | null} */
  let activeStage = null;
  /** @type {string} */
  let lastStageStateSig = "";

  function clearDmCallStage() {
    if (activeStage) {
      activeStage.destroy();
      activeStage = null;
    }
    lastStageStateSig = "";
    if (typeof document !== "undefined") {
      document.getElementById("dm-call-stage-banner")?.remove();
    }
  }

  /**
   * Renders the DM call stage banner above the chat timeline with participant tiles,
   * camera/screen share previews, and call action controls via `createVoiceCallStage`.
   */
  function renderLiveDmCallStage() {
    ctx.renderLiveGuildVoiceStagePreview();
    if (typeof document === "undefined") return;
    if (!ctx.isLiveSession()) {
      clearDmCallStage();
      return;
    }
    const isDmView =
      document.body?.dataset?.viewMode === "dm" ||
      ctx.getRailSelectedId?.() === HOME_GUILD_ID ||
      Boolean(document.getElementById("dm-header-call-actions"));
    const dmCid = ctx.cleanDiscordStr(String(ctx.getChatActiveChannelId?.() || ""));
    const activeConn = ctx.getActiveVoiceConnection();
    const isConnectedHere = Boolean(
      activeConn &&
        ctx.normalizeVoiceGuildKey(activeConn.guildId) === HOME_GUILD_ID &&
        String(activeConn.channelId || "") === dmCid,
    );
    if (!dmCid || (!isDmView && !isConnectedHere)) {
      clearDmCallStage();
      return;
    }
    const participants = ctx.getVoiceParticipants(HOME_GUILD_ID, dmCid);
    const hasActiveDmCall = ctx.activeDmCalls.has(dmCid);
    if (!isConnectedHere && participants.length === 0 && !hasActiveDmCall) {
      clearDmCallStage();
      document.getElementById("dm-call-join-banner")?.remove();
      return;
    }

    const dmChannelName =
      ctx.cleanDiscordStr(document.querySelector(".chat-header .chat-title")?.textContent) ||
      activeConn?.channelName ||
      "Звонок";

    if (!isConnectedHere) {
      clearDmCallStage();
      let callInviteBanner = document.getElementById("dm-call-join-banner");
      if (!callInviteBanner) {
        callInviteBanner = document.createElement("div");
        callInviteBanner.id = "dm-call-join-banner";
        callInviteBanner.style.cssText =
          "margin:8px 16px;padding:12px 16px;background:rgba(35,165,90,0.15);border:1px solid #23a55a;border-radius:8px;display:flex;align-items:center;justify-content:space-between;gap:12px;";
        const txt = document.createElement("div");
        txt.style.cssText = "color:#f2f3f5;font-size:14px;font-weight:600;display:flex;align-items:center;gap:8px;";
        txt.innerHTML = "<span>📞</span><span>В этом чате идёт голосовой звонок</span>";
        const joinBtn = document.createElement("button");
        joinBtn.type = "button";
        joinBtn.style.cssText =
          "background:#23a55a;color:#fff;border:none;border-radius:4px;padding:8px 16px;font-weight:600;font-size:13px;cursor:pointer;";
        joinBtn.textContent = "Присоединиться к звонку";
        joinBtn.onclick = () => {
          ctx.connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
        };
        callInviteBanner.append(txt, joinBtn);
        const chatScreen = document.getElementById("screen-chat");
        const chatMain = chatScreen?.querySelector(".chat-main");
        if (chatMain) {
          chatMain.prepend(callInviteBanner);
        }
      }
      return;
    } else {
      document.getElementById("dm-call-join-banner")?.remove();
    }

    const selfUser = ctx.getLiveSelfUser?.();
    const selfId = ctx.cleanDiscordStr(String(selfUser?.id || ""));
    const displayParticipants = [...participants];
    if (isConnectedHere && displayParticipants.length === 1 && displayParticipants[0]?.userId === selfId) {
      const pcs = ctx.getDmFriendsController?.()?.getPrivateChannels?.() ?? [];
      const pc = /** @type {any} */ (pcs.find((/** @type {any} */ c) => String(c?.id || "") === dmCid));
      const activeDmRec = /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__;
      const recs = Array.isArray(pc?.rawRecipients) && pc.rawRecipients.length > 0
        ? pc.rawRecipients
        : Array.isArray(pc?.recipients) && pc.recipients.length > 0
          ? pc.recipients
          : activeDmRec && activeDmRec.id
            ? [activeDmRec]
            : [];
      for (const r of recs) {
        const rid = ctx.cleanDiscordStr(String(r?.id || ""));
        if (rid && rid !== selfId && !displayParticipants.some((p) => p.userId === rid)) {
          const known = ctx.liveUsersById.get(rid) ?? r;
          const rName =
            ctx.cleanDiscordStr(
              known?.global_name ??
                known?.globalName ??
                known?.display_name ??
                known?.displayName ??
                known?.username,
            ) || dmChannelName;
          const rAvHash = ctx.cleanDiscordStr(known?.avatar);
          displayParticipants.push({
            userId: rid,
            guildId: HOME_GUILD_ID,
            channelId: dmCid,
            displayName: rName,
            username: ctx.cleanDiscordStr(known?.username) || rName,
            avatarUrl: rAvHash ? `https://cdn.discordapp.com/avatars/${rid}/${rAvHash}.png?size=128` : null,
            muted: false,
            deafened: false,
            streaming: false,
            speaking: false,
            ringing: true,
          });
        }
      }
    }

    const chatScreen = document.getElementById("screen-chat");
    const chatMain = /** @type {HTMLElement | null} */ (chatScreen?.querySelector(".chat-main") ?? chatScreen);
    if (!chatMain) return;

    const timerTxt = ctx.getVoiceChannelTimer(HOME_GUILD_ID, dmCid, Math.max(1, participants.length));
    const anyStreamer = displayParticipants.find(
      (p) => p.streaming || (p.userId === selfId && activeConn?.screenShareOn),
    );
    const qualityLabel = `${activeConn?.streamResolution || "720p"} ${activeConn?.fps || 60}fps`;

    const mappedParticipants = displayParticipants.map((p) => {
      const isSelf = p.userId === selfId;
      let avatarUrl = p.avatarUrl || null;
      if (!avatarUrl && isSelf && selfUser?.avatar && selfId) {
        avatarUrl = `https://cdn.discordapp.com/avatars/${selfId}/${selfUser.avatar}.png?size=128`;
      }
      return {
        id: p.userId,
        name: p.displayName || "Участник",
        avatarUrl,
        speaking: Boolean(p.speaking),
        muted: Boolean(p.muted || (isSelf && ctx.selfVoicePrefs?.muted)),
        deafened: Boolean(p.deafened || (isSelf && ctx.selfVoicePrefs?.deafened)),
        ringing: Boolean(p.ringing),
        raw: p,
      };
    });

    if (!activeStage) {
      activeStage = createVoiceCallStage(chatMain, {
        id: "dm-call-stage-banner",
        initialConnected: true,
        initialChannelName: dmChannelName,
        initialStreaming: Boolean(anyStreamer),
        initialStreamQuality: qualityLabel,
        initialMuted: Boolean(ctx.selfVoicePrefs?.muted),
        initialCameraOn: Boolean(activeConn?.cameraOn),
        initialParticipants: mappedParticipants,
        onMute: () => {
          ctx.toggleSelfMicOnly?.();
        },
        onSelectMic: (/** @type {HTMLElement} */ anchor) => {
          ctx.openAudioDevicePopover?.("input", anchor);
        },
        onCamera: () => {
          const conn = ctx.getActiveVoiceConnection?.();
          if (!conn) return;
          conn.cameraOn = !conn.cameraOn;
          ctx.playVoiceUiSound?.(conn.cameraOn ? "camera_on" : "camera_off");
          ctx.syncVoiceMuteStateWithBackend?.();
          ctx.renderLiveVoicePanel?.();
        },
        onScreenShare: () => {
          const conn = ctx.getActiveVoiceConnection?.();
          ctx.openScreenShareModal?.(!conn?.screenShareOn);
        },
        onActivities: () => {
          ctx.openActivitiesPopout?.();
        },
        onSoundboard: () => {
          ctx.openSoundboardPopout?.();
        },
        onDisconnect: () => {
          ctx.disconnectFromLiveVoiceChannel?.();
        },
        onStreamClick: () => {
          const conn = ctx.getActiveVoiceConnection?.();
          const currentParticipants = ctx.getVoiceParticipants(HOME_GUILD_ID, dmCid);
          const streamer = currentParticipants.find(
            (/** @type {any} */ p) => p.streaming || (p.userId === selfId && conn?.screenShareOn),
          );
          if (streamer) {
            ctx.openLiveStreamStageViewer?.(
              { ...streamer, guildId: "", channelId: dmCid, streaming: true },
              { id: dmCid, name: dmChannelName },
            );
          }
        },
        onParticipantContextMenu: (/** @type {any} */ p, /** @type {HTMLElement} */ el, /** @type {MouseEvent} */ ev) => {
          ctx.openVoiceParticipantContextMenu?.(p.raw ?? p, { id: dmCid, name: dmChannelName }, el, ev);
        },
      });
    } else {
      activeStage.mountTo(chatMain);
    }

    activeStage.setTimer(timerTxt);

    const stageStateSig = JSON.stringify({
      dmCid,
      dmChannelName,
      isConnectedHere,
      cam: Boolean(activeConn?.cameraOn),
      stream: Boolean(activeConn?.screenShareOn),
      anyStreamerId: anyStreamer?.userId || "",
      qualityLabel,
      muted: Boolean(ctx.selfVoicePrefs?.muted),
      deafened: Boolean(ctx.selfVoicePrefs?.deafened),
      previewDataUrl: String(ctx.cachedLiveScreenDataUrl || ""),
      participants: mappedParticipants.map((p) => ({
        u: p.id,
        n: p.name,
        a: p.avatarUrl,
        m: p.muted,
        d: p.deafened,
        sp: p.speaking,
        r: p.ringing,
      })),
    });

    if (lastStageStateSig !== stageStateSig) {
      lastStageStateSig = stageStateSig;
      activeStage.setChannel(dmChannelName);
      activeStage.setParticipants(mappedParticipants);
      activeStage.setStreaming(Boolean(anyStreamer), qualityLabel);
      activeStage.setStreamPreview({
        dataUrl: ctx.cachedLiveScreenDataUrl || "",
        streamerName: anyStreamer?.displayName || dmChannelName,
        quality: qualityLabel,
      });
      activeStage.setControlsState({
        muted: Boolean(ctx.selfVoicePrefs?.muted),
        cameraOn: Boolean(activeConn?.cameraOn),
        streaming: Boolean(activeConn?.screenShareOn),
      });
    }
  }

  /** @param {boolean} start */
  function playRingingTone(start) {
    if (ringingAudioLoopTimer) {
      clearInterval(ringingAudioLoopTimer);
      ringingAudioLoopTimer = null;
    }
    if (!start) return;
    ctx.playVoiceUiSound("outgoing_call");
    ringingAudioLoopTimer = setInterval(() => {
      const overlay = document.getElementById("incoming-call-overlay");
      const dmStage =
        document.getElementById("dm-call-stage-banner") ||
        document.getElementById("live-dm-call-stage");
      if (!overlay && !dmStage) {
        if (ringingAudioLoopTimer) {
          clearInterval(ringingAudioLoopTimer);
          ringingAudioLoopTimer = null;
        }
        return;
      }
      ctx.playVoiceUiSound("outgoing_call");
    }, 2800);
  }

  /**
   * Displays the incoming call notification overlay.
   * @param {any} data
   */
  function showIncomingCallOverlay(data) {
    const cid = ctx.cleanDiscordStr(String(data?.channel_id || ""));
    if (!cid) return;
    document.getElementById("incoming-call-overlay")?.remove();
    playRingingTone(true);

    const overlay = document.createElement("div");
    overlay.id = "incoming-call-overlay";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-label", "Входящий звонок");
    overlay.style.cssText =
      "position:fixed;top:20px;right:24px;z-index:10000;background:#1e1f22;border:1px solid #383a40;border-radius:12px;box-shadow:0 12px 36px rgba(0,0,0,0.85);padding:14px 18px;display:flex;align-items:center;gap:14px;min-width:320px;max-width:420px;font-family:inherit;";

    const pcs = ctx.getDmFriendsController?.()?.getPrivateChannels?.() ?? [];
    const pc = pcs.find((/** @type {any} */ c) => String(c?.id || "") === cid);
    const anyPc = /** @type {any} */ (pc);
    const selfId = ctx.cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
    let callerUser = null;
    if (Array.isArray(data.voice_states)) {
      const callerVs = data.voice_states.find((/** @type {any} */ v) => String(v?.user_id || "") !== selfId);
      if (callerVs) {
        callerUser = ctx.liveUsersById.get(String(callerVs.user_id)) || callerVs.member?.user;
      }
    }
    if (!callerUser && anyPc) {
      const recs = anyPc.rawRecipients || anyPc.recipients || [];
      callerUser = recs.find((/** @type {any} */ r) => String(r?.id || "") !== selfId) || recs[0];
    }
    const callerName = callerUser?.global_name || callerUser?.username || anyPc?.name || "Пользователь";
    const callerAvatar = callerUser?.avatar
      ? `https://cdn.discordapp.com/avatars/${callerUser.id}/${callerUser.avatar}.png?size=64`
      : null;

    const avatarEl = document.createElement("div");
    avatarEl.style.cssText =
      "width:46px;height:46px;border-radius:50%;background:#5865f2;display:flex;align-items:center;justify-content:center;font-size:20px;color:#fff;overflow:hidden;flex-shrink:0;";
    if (callerAvatar) {
      const img = document.createElement("img");
      img.src = callerAvatar;
      img.style.cssText = "width:100%;height:100%;object-fit:cover;";
      avatarEl.append(img);
    } else {
      avatarEl.textContent = callerName.charAt(0).toUpperCase();
    }

    const infoCol = document.createElement("div");
    infoCol.style.cssText = "flex:1;min-width:0;display:flex;flex-direction:column;gap:2px;";
    const nameEl = document.createElement("div");
    nameEl.style.cssText =
      "font-size:15px;font-weight:700;color:#f2f3f5;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
    nameEl.textContent = callerName;
    const titleEl = document.createElement("div");
    titleEl.style.cssText = "font-size:12px;color:#23a55a;font-weight:600;";
    titleEl.textContent = "Входящий голосовой звонок...";
    infoCol.append(nameEl, titleEl);

    const actionsRow = document.createElement("div");
    actionsRow.style.cssText = "display:flex;align-items:center;gap:8px;flex-shrink:0;";

    const declineBtn = document.createElement("button");
    declineBtn.type = "button";
    declineBtn.title = "Отклонить";
    declineBtn.setAttribute("aria-label", "Отклонить звонок");
    declineBtn.style.cssText =
      "width:38px;height:38px;border-radius:50%;border:none;background:#da373c;color:#fff;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;transition:background 120ms ease;";
    declineBtn.innerHTML = SVG_PHONE_DISCONNECT;
    declineBtn.onmouseenter = () => {
      declineBtn.style.background = "#a1282c";
    };
    declineBtn.onmouseleave = () => {
      declineBtn.style.background = "#da373c";
    };
    declineBtn.onclick = () => {
      dismissIncomingCallOverlay(cid);
      void ctx
        .sendDmRest?.({
          method: "POST",
          url: `/channels/${cid}/call/stop-ringing`,
          body: {},
        })
        ?.catch?.(() => {});
    };

    const acceptBtn = document.createElement("button");
    acceptBtn.type = "button";
    acceptBtn.title = "Принять";
    acceptBtn.setAttribute("aria-label", "Принять звонок");
    acceptBtn.style.cssText =
      "width:38px;height:38px;border-radius:50%;border:none;background:#23a55a;color:#fff;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;font-size:18px;transition:background 120ms ease;";
    acceptBtn.textContent = "📞";
    acceptBtn.onmouseenter = () => {
      acceptBtn.style.background = "#1a7c44";
    };
    acceptBtn.onmouseleave = () => {
      acceptBtn.style.background = "#23a55a";
    };
    acceptBtn.onclick = () => {
      dismissIncomingCallOverlay(cid);
      ctx.connectToLiveVoiceChannel("", callerName, { id: cid, name: callerName });
    };

    actionsRow.append(acceptBtn, declineBtn);
    overlay.append(avatarEl, infoCol, actionsRow);
    document.body.append(overlay);
  }

  /** @param {string} [cid] */
  function dismissIncomingCallOverlay(cid) {
    void cid;
    const el = document.getElementById("incoming-call-overlay");
    if (el) {
      el.remove();
      playRingingTone(false);
    }
  }

  return {
    renderLiveDmCallStage,
    playRingingTone,
    showIncomingCallOverlay,
    dismissIncomingCallOverlay,
    getVoiceCallStage: () => activeStage,
    clearDmCallStage,
  };
}
