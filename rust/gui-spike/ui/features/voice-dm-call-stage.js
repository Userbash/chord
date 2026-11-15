// @ts-check
/**
 * DM call stage banner rendering, incoming call overlay, and ringing tone loop.
 * Extracted from `app.js` as part of RW51-EPIC2-T2.
 * @module ui/features/voice-dm-call-stage
 */

import { HOME_GUILD_ID } from "./dm-friends.js";

const SVG_PHONE_DISCONNECT = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08a.956.956 0 0 1-.29-.7c0-.28.11-.53.29-.71C3.34 8.78 7.46 7 12 7s8.66 1.78 11.71 4.67c.18.18.29.43.29.71 0 .28-.11.53-.29.71l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28a11.27 11.27 0 0 0-2.67-1.85.996.996 0 0 1-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z"/></svg>`;

/**
 * Create the DM call stage and incoming call overlay controller.
 * @param {any} ctx
 */
export function createVoiceDmCallStage(ctx) {
  /** @type {any} */
  let ringingAudioLoopTimer = null;

  /**
   * Renders the DM call stage banner above the chat timeline with participant tiles,
   * camera/screen share previews, and call action controls.
   */
  function renderLiveDmCallStage() {
    ctx.renderLiveGuildVoiceStagePreview();
    if (typeof document === "undefined") return;
    const existing = document.getElementById("dm-call-stage-banner");
    if (!ctx.isLiveSession()) {
      existing?.remove();
      return;
    }
    const isDmView =
      document.body?.dataset?.viewMode === "dm" ||
      ctx.getRailSelectedId?.() === HOME_GUILD_ID ||
      Boolean(document.getElementById("dm-header-call-actions"));
    const dmCid = ctx.cleanDiscordStr(String(ctx.getChatActiveChannelId?.() || ""));
    if (!isDmView || !dmCid) {
      existing?.remove();
      return;
    }
    const participants = ctx.getVoiceParticipants(HOME_GUILD_ID, dmCid);
    const hasActiveDmCall = ctx.activeDmCalls.has(dmCid);
    const activeConn = ctx.getActiveVoiceConnection();
    const isConnectedHere = Boolean(
      activeConn &&
        ctx.normalizeVoiceGuildKey(activeConn.guildId) === HOME_GUILD_ID &&
        String(activeConn.channelId || "") === dmCid,
    );
    if (!isConnectedHere && participants.length === 0 && !hasActiveDmCall) {
      existing?.remove();
      document.getElementById("dm-call-join-banner")?.remove();
      return;
    }

    const dmChannelName =
      ctx.cleanDiscordStr(document.querySelector(".chat-header .chat-title")?.textContent) ||
      activeConn?.channelName ||
      "Звонок";

    if (!isConnectedHere) {
      existing?.remove();
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
    const selfId = ctx.cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
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
            avatarUrl: rAvHash ? `https://cdn.discordapp.com/avatars/${rid}/${rAvHash}.png?size=64` : null,
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
    const chatMain = /** @type {HTMLElement | null} */ (chatScreen?.querySelector(".chat-main"));
    let banner = existing;
    if (!banner) {
      banner = document.createElement("section");
      banner.id = "dm-call-stage-banner";
      banner.setAttribute("role", "region");
      banner.setAttribute("aria-label", `Голосовой звонок: ${dmChannelName}`);
      if (chatMain) {
        chatMain.prepend(banner);
      } else if (chatScreen) {
        chatScreen.prepend(banner);
      }
    } else if (chatMain && banner.parentNode !== chatMain) {
      chatMain.prepend(banner);
    }
    const timerTxt = ctx.getVoiceChannelTimer(HOME_GUILD_ID, dmCid, Math.max(1, participants.length));
    const stageStateSig = JSON.stringify({
      dmCid,
      dmChannelName,
      isConnectedHere,
      cam: Boolean(activeConn?.cameraOn),
      stream: Boolean(activeConn?.screenShareOn),
      streamSrc: String(activeConn?.streamSourceId || ""),
      muted: Boolean(ctx.selfVoicePrefs.muted),
      deafened: Boolean(ctx.selfVoicePrefs.deafened),
      hasPreview: Boolean(ctx.cachedLiveScreenDataUrl),
      participants: displayParticipants.map((p) => ({
        u: p.userId,
        n: p.displayName,
        m: Boolean(p.muted),
        d: Boolean(p.deafened),
        s: Boolean(p.streaming),
        v: Boolean(p.video),
        sp: Boolean(p.speaking),
        r: Boolean(p.ringing),
      })),
    });
    if (banner.dataset.stageStateSig === stageStateSig) {
      const timerPill = banner.querySelector("[data-dm-stage-timer]");
      if (timerPill && timerPill.textContent !== timerTxt) {
        timerPill.textContent = timerTxt;
      }
      if (activeConn?.screenShareOn && ctx.cachedLiveScreenDataUrl) {
        const selfThumb = /** @type {HTMLImageElement | null} */ (
          banner.querySelector("img[data-dm-stage-self-stream-thumb]")
        );
        if (selfThumb && selfThumb.src !== ctx.cachedLiveScreenDataUrl) {
          selfThumb.src = ctx.cachedLiveScreenDataUrl;
        }
      }
      return;
    }
    banner.dataset.stageStateSig = stageStateSig;
    banner.style.cssText =
      "width:100%;box-sizing:border-box;background:linear-gradient(180deg,#111214 0%,#18191c 100%);border-bottom:1px solid #2b2d31;padding:14px 18px;display:flex;flex-direction:column;gap:12px;flex-shrink:0;z-index:12;";
    banner.replaceChildren();

    const topStatusRow = document.createElement("div");
    topStatusRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:10px;";
    const statusLeft = document.createElement("div");
    statusLeft.style.cssText = "display:flex;align-items:center;gap:8px;font-size:13px;font-weight:700;color:#f2f3f5;";
    statusLeft.innerHTML = `<span style="color:#23a55a;">📞</span><span>Звонок: ${dmChannelName}</span><span data-dm-stage-timer="true" style="padding:2px 7px;border-radius:10px;background:#2b2d31;color:#b5bac1;font-size:11px;font-weight:600;">${timerTxt}</span>`;
    topStatusRow.append(statusLeft);
    banner.append(topStatusRow);

    const tilesRow = document.createElement("div");
    tilesRow.style.cssText = "display:flex;align-items:center;justify-content:center;gap:14px;flex-wrap:wrap;min-height:112px;";
    for (const p of displayParticipants) {
      const isSelf = p.userId === selfId;
      const isStreaming = Boolean(p.streaming || (isSelf && activeConn?.screenShareOn));
      const isCam = Boolean(p.video || (isSelf && activeConn?.cameraOn));
      const tile = document.createElement("div");
      tile.className = "voice-channel-user dm-call-stage-tile";
      tile.setAttribute("data-user-id", p.userId);
      tile.setAttribute("data-channel-id", dmCid);
      tile.style.cssText =
        `width:${isStreaming || isCam ? "224px" : "156px"};min-height:118px;border-radius:12px;background:#1e1f22;border:2px solid ${p.speaking ? "#23a55a" : isStreaming ? "#5865f2" : isCam ? "#23a55a" : "#2b2d31"};padding:10px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;position:relative;cursor:${isStreaming ? "pointer" : "default"};overflow:hidden;transition:border-color 120ms ease;`;

      if (isStreaming && isSelf && ctx.cachedLiveScreenDataUrl) {
        const previewImg = document.createElement("img");
        previewImg.setAttribute("data-dm-stage-self-stream-thumb", "true");
        previewImg.src = ctx.cachedLiveScreenDataUrl;
        previewImg.alt = "Предпросмотр демонстрации экрана";
        previewImg.style.cssText =
          "width:100%;height:78px;object-fit:cover;border-radius:8px;border:1px solid rgba(255,255,255,0.12);background:#0b0c0e;";
        tile.append(previewImg);
      } else if (isCam && !isStreaming) {
        const camPreview = document.createElement("canvas");
        camPreview.width = 200;
        camPreview.height = 76;
        camPreview.style.cssText =
          "width:100%;height:76px;object-fit:cover;border-radius:8px;border:1px solid rgba(35,165,90,0.45);background:#0f172a;";
        ctx.drawLiveCaptureCanvasFrame(camPreview, "cam_v4l2_0", 1, true);
        tile.append(camPreview);
      } else {
        const av = document.createElement("div");
        av.className = "voice-user-avatar";
        av.setAttribute("data-voice-avatar-uid", p.userId);
        if (p.speaking) av.setAttribute("data-speaking", "true");
        av.style.cssText =
          `width:48px;height:48px;border-radius:50%;background:#5865f2;color:#fff;display:flex;align-items:center;justify-content:center;font-size:18px;font-weight:700;background-size:cover;background-position:center;opacity:${p.ringing ? "0.6" : "1"};${p.speaking ? "box-shadow:0 0 0 3px #23a55a;" : ""}`;
        av.textContent = Array.from(String(p.displayName || "U")).slice(0, 1).join("").toUpperCase();
        if (p.avatarUrl) {
          av.style.backgroundImage = `url("${p.avatarUrl}")`;
          av.style.color = "transparent";
        }
        tile.append(av);
      }

      const nameRow = document.createElement("div");
      nameRow.style.cssText =
        "display:flex;align-items:center;gap:5px;max-width:100%;font-size:12px;font-weight:700;color:#f2f3f5;";
      const nameSpan = document.createElement("span");
      nameSpan.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
      nameSpan.textContent = p.displayName;
      nameRow.append(nameSpan);
      if (p.muted) {
        const mIcon = document.createElement("span");
        mIcon.style.cssText = "color:#f23f43;font-size:11px;";
        mIcon.textContent = p.deafened ? "🎧̸" : "🎙̸";
        nameRow.append(mIcon);
      }

      tile.append(nameRow);

      if (p.ringing) {
        const ringBadge = document.createElement("span");
        ringBadge.style.cssText =
          "padding:2px 7px;border-radius:8px;background:rgba(240,178,50,0.18);color:#f0b232;font-size:10px;font-weight:700;";
        ringBadge.textContent = "Ожидание ответа…";
        tile.append(ringBadge);
      } else if (isStreaming) {
        const liveBtn = document.createElement("button");
        liveBtn.type = "button";
        liveBtn.setAttribute("data-action", "dm-watch-stream");
        liveBtn.style.cssText =
          "padding:4px 10px;border-radius:6px;border:none;background:#da373c;color:#fff;font-size:11px;font-weight:800;cursor:pointer;display:inline-flex;align-items:center;gap:5px;";
        liveBtn.innerHTML = `<span>🔴 В ЭФИРЕ</span><span>• Смотреть</span>`;
        liveBtn.addEventListener("click", (ev) => {
          ev.stopPropagation();
          if (!isConnectedHere) {
            ctx.connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
          }
          ctx.openLiveStreamStageViewer(
            { ...p, guildId: "", channelId: dmCid, streaming: true },
            { id: dmCid, name: dmChannelName },
          );
        });
        tile.append(liveBtn);
      } else if (isCam) {
        const camBadge = document.createElement("span");
        camBadge.style.cssText =
          "padding:2px 8px;border-radius:6px;background:rgba(35,165,90,0.2);color:#4ade80;font-size:10px;font-weight:700;display:inline-flex;align-items:center;gap:4px;";
        camBadge.textContent = "📹 Камера вкл.";
        tile.append(camBadge);
      }

      tile.addEventListener("click", () => {
        if (isStreaming) {
          if (!isConnectedHere) {
            ctx.connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
          }
          ctx.openLiveStreamStageViewer(
            { ...p, guildId: "", channelId: dmCid, streaming: true },
            { id: dmCid, name: dmChannelName },
          );
        }
      });
      tile.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        ctx.openVoiceParticipantContextMenu(p, { id: dmCid, name: dmChannelName }, tile, ev);
      });
      tilesRow.append(tile);
    }
    banner.append(tilesRow);

    const controlsRow = document.createElement("div");
    controlsRow.style.cssText = "display:flex;align-items:center;justify-content:center;gap:8px;flex-wrap:wrap;";
    const makeStageBtn = (
      /** @type {string} */ label,
      /** @type {string} */ bg,
      /** @type {string} */ color,
      /** @type {string} */ actionId,
      /** @type {() => void} */ onClick,
    ) => {
      const b = document.createElement("button");
      b.type = "button";
      b.setAttribute("data-action", actionId);
      b.style.cssText =
        `padding:7px 13px;border-radius:8px;border:1px solid rgba(255,255,255,0.08);background:${bg};color:${color};font-size:12px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;gap:6px;`;
      b.textContent = label;
      b.addEventListener("click", (ev) => {
        ev.stopPropagation();
        onClick();
      });
      return b;
    };

    if (!isConnectedHere) {
      controlsRow.append(
        makeStageBtn("📞 Присоединиться к звонку", "#23a55a", "#ffffff", "dm-stage-join-voice", () => {
          if (!ctx.selfVoicePrefs.deafened) ctx.selfVoicePrefs.muted = false;
          ctx.connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
        }),
        makeStageBtn("📹 Присоединиться с видео", "#2b2d31", "#f2f3f5", "dm-stage-join-video", () => {
          if (!ctx.selfVoicePrefs.deafened) ctx.selfVoicePrefs.muted = false;
          ctx.connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
          const nextConn = ctx.getActiveVoiceConnection();
          if (nextConn) {
            nextConn.cameraOn = true;
            ctx.syncVoiceMuteStateWithBackend();
            ctx.renderLiveVoicePanel();
          }
        }),
      );
    } else {
      const conn = activeConn;
      const anyStreamer = displayParticipants.find(
        (p) => p.streaming || (p.userId === selfId && conn?.screenShareOn),
      );
      controlsRow.append(
        makeStageBtn(
          conn?.cameraOn ? "📹 Выкл. камеру" : "📹 Камера",
          conn?.cameraOn ? "#23a55a" : "#2b2d31",
          "#ffffff",
          "dm-stage-toggle-camera",
          () => {
            if (!conn) return;
            conn.cameraOn = !conn.cameraOn;
            ctx.playVoiceUiSound(conn.cameraOn ? "camera_on" : "camera_off");
            ctx.syncVoiceMuteStateWithBackend();
            ctx.renderLiveVoicePanel();
          },
        ),
        makeStageBtn(
          conn?.screenShareOn ? "🖥 Изменить экран" : "🖥 Демонстрация экрана",
          conn?.screenShareOn ? "#5865f2" : "#2b2d31",
          "#ffffff",
          "dm-stage-screenshare",
          () => {
            ctx.openScreenShareModal(true);
          },
        ),
      );
      if (conn?.screenShareOn) {
        controlsRow.append(
          makeStageBtn(
            "⏹ Остановить стрим",
            "#da373c",
            "#ffffff",
            "dm-stage-stop-screenshare",
            () => {
              ctx.openScreenShareModal(false);
            },
          ),
        );
      }
      if (anyStreamer) {
        controlsRow.append(
          makeStageBtn(
            "📺 Смотреть трансляцию",
            "#5865f2",
            "#ffffff",
            "dm-stage-open-stream-viewer",
            () => {
              ctx.openLiveStreamStageViewer(
                { ...anyStreamer, guildId: "", channelId: dmCid, streaming: true },
                { id: dmCid, name: dmChannelName },
              );
            },
          ),
        );
      }
      controlsRow.append(
        makeStageBtn(
          ctx.selfVoicePrefs.muted ? "🎙 Вкл. микрофон" : "🎙 Выкл. микрофон",
          ctx.selfVoicePrefs.muted ? "rgba(242,63,67,0.22)" : "#2b2d31",
          ctx.selfVoicePrefs.muted ? "#f23f43" : "#f2f3f5",
          "dm-stage-toggle-mic",
          () => {
            ctx.toggleSelfMicOnly();
          },
        ),
        makeStageBtn(
          ctx.selfVoicePrefs.deafened ? "🎧 Вкл. звук" : "🎧 Выкл. звук",
          ctx.selfVoicePrefs.deafened ? "rgba(242,63,67,0.22)" : "#2b2d31",
          ctx.selfVoicePrefs.deafened ? "#f23f43" : "#f2f3f5",
          "dm-stage-toggle-deaf",
          () => {
            ctx.toggleSelfDeafenBoth();
          },
        ),
        makeStageBtn(
          "👥+ Добавить в звонок",
          "#2b2d31",
          "#f2f3f5",
          "dm-stage-invite",
          () => {
            ctx.openVoiceInviteModal({ id: dmCid, name: dmChannelName, type: 3 });
          },
        ),
        makeStageBtn(
          "📞 Завершить звонок",
          "#da373c",
          "#ffffff",
          "dm-stage-leave",
          () => {
            ctx.disconnectFromLiveVoiceChannel();
          },
        ),
      );
    }
    banner.append(controlsRow);
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
      const dmStage = document.getElementById("live-dm-call-stage");
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
  };
}
