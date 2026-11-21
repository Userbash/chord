// @ts-check
/**
 * Voice invite modal, audio device selection popover, hotplug prompt, and voice device settings mounting.
 * Extracted from `app.js` as part of RW51-EPIC2-T2.
 * @module ui/features/voice-invite-and-devices
 */

import { HOME_GUILD_ID } from "./dm-friends.js";
import { createVoiceDevicesSettings } from "./voice-devices-settings.js";

/**
 * Create the voice invite modal and audio device UI controller.
 * @param {any} ctx
 */
export function createVoiceInviteAndDevicesController(ctx) {
  /**
   * Opens the "Invite Friends" modal for a voice or group channel.
   * @param {{ id: string, name?: string, type?: number }} channel
   * @param {boolean} [isRestricted]
   */
  function openVoiceInviteModal(channel, isRestricted = false) {
    document.getElementById("live-voice-invite-modal")?.remove();
    const isGroupInvite =
      Number(channel?.type) === 1 || Number(channel?.type) === 3 || ctx.getRailSelectedId?.() === HOME_GUILD_ID;
    const gInfo = ctx.currentGuildInfo();
    let resolvedGuildId = "";
    if (!isGroupInvite) {
      const targetChId = String(channel?.id || "");
      if (targetChId) {
        for (const [candGid, gObj] of ctx.liveGuildsById.entries()) {
          if (
            Array.isArray(gObj?.channels) &&
            gObj.channels.some((/** @type {any} */ c) => String(c?.id || "") === targetChId)
          ) {
            resolvedGuildId = candGid;
            break;
          }
        }
      }
      if (!resolvedGuildId) {
        const fallbackLiveGid = Array.from(ctx.liveGuildsById.keys())[0] || "";
        resolvedGuildId =
          (gInfo.id && gInfo.id !== HOME_GUILD_ID ? gInfo.id : "") ||
          (ctx.getRailSelectedId?.() && ctx.getRailSelectedId() !== HOME_GUILD_ID ? ctx.getRailSelectedId() : "") ||
          ctx.selectedGuildId() ||
          (ctx.isLiveSession() ? fallbackLiveGid : "1014537851418980372");
      }
    }
    const gId = isGroupInvite ? HOME_GUILD_ID : resolvedGuildId;
    const chName = String(
      channel?.name || channel?.id || (isGroupInvite ? "Групповой чат" : "Голосовой канал"),
    );
    const liveGuildObj = !isGroupInvite && gId ? ctx.liveGuildsById.get(gId) : null;
    const gName = isGroupInvite ? chName : String(liveGuildObj?.name || gInfo.name || "Сервер");
    const restricted = !isGroupInvite && Boolean(isRestricted || ctx.isChannelRoleRestricted(gId, channel));

    const backdrop = document.createElement("div");
    backdrop.id = "live-voice-invite-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.74);display:flex;align-items:center;justify-content:center;z-index:9999;";

    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", `Пригласить друзей в ${gName}`);
    card.style.cssText =
      "width:460px;max-width:94vw;background:#2b2d31;color:#f2f3f5;border:1px solid #1e1f22;border-radius:12px;overflow:hidden;box-shadow:0 20px 50px rgba(0,0,0,0.78);display:flex;flex-direction:column;font-family:inherit;";

    const topSection = document.createElement("div");
    topSection.style.cssText = "padding:18px 20px 12px;display:flex;flex-direction:column;gap:8px;";

    const titleRow = document.createElement("div");
    titleRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:12px;";
    const titleEl = document.createElement("h3");
    titleEl.style.cssText = "margin:0;font-size:17px;font-weight:700;color:#f2f3f5;line-height:1.25;";
    titleEl.textContent = `Пригласить друзей в ${gName}`;
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon-btn";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText =
      "background:none;border:none;color:#b5bac1;font-size:18px;cursor:pointer;padding:2px 6px;border-radius:6px;";
    closeBtn.addEventListener("click", () => backdrop.remove());
    titleRow.append(titleEl, closeBtn);

    const subRow = document.createElement("div");
    subRow.style.cssText = "display:flex;align-items:center;gap:6px;font-size:13px;color:#b5bac1;";
    const chIconSpan = document.createElement("span");
    chIconSpan.style.cssText = "display:inline-flex;align-items:center;color:#949ba4;";
    chIconSpan.innerHTML = restricted
      ? `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M18 8h-1V6.5a2.5 2.5 0 0 0-5 0V8h-1a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V9a1 1 0 0 0-1-1zm-2 0h-2V6.5a1 1 0 0 1 2 0V8z"/><path d="M14.5 17.5a3.5 3.5 0 0 0 2.5-1.1l1.1 1.1a5 5 0 0 1-3.6 1.5z"/></svg>`
      : `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M11 5L6 9H2v6h4l5 4V5zm4.5 3.5v7c1.48-.63 2.5-2.06 2.5-3.5s-1.02-2.87-2.5-3.5zm0-4.27v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/></svg>`;
    const subText = document.createElement("span");
    subText.textContent = "Участники окажутся в";
    const subChName = document.createElement("span");
    subChName.style.cssText = "color:#dbdee1;font-weight:600;";
    subChName.textContent = chName;
    subRow.append(subText, chIconSpan, subChName);

    const searchWrap = document.createElement("div");
    searchWrap.style.cssText =
      "display:flex;align-items:center;gap:8px;background:#1e1f22;border:1px solid #3f4147;border-radius:8px;padding:7px 11px;margin-top:4px;";
    const searchIcon = document.createElement("span");
    searchIcon.style.cssText = "color:#949ba4;font-size:14px;display:inline-flex;";
    searchIcon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="7"/><line x1="16.5" y1="16.5" x2="21" y2="21"/></svg>`;
    const searchInput = document.createElement("input");
    searchInput.type = "search";
    searchInput.placeholder = "Найти друзей";
    searchInput.style.cssText =
      "flex:1;background:transparent;border:none;outline:none;color:#f2f3f5;font-size:14px;";
    searchWrap.append(searchIcon, searchInput);

    topSection.append(titleRow, subRow, searchWrap);

    if (restricted) {
      const warnBox = document.createElement("div");
      warnBox.style.cssText =
        "display:flex;align-items:flex-start;gap:8px;margin-top:4px;font-size:12px;color:#dbdee1;line-height:1.4;";
      const warnDot = document.createElement("span");
      warnDot.style.cssText =
        "width:16px;height:16px;border-radius:50%;background:#f0b232;color:#111214;font-weight:800;font-size:11px;display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;margin-top:1px;";
      warnDot.textContent = "!";
      const warnTxt = document.createElement("span");
      warnTxt.textContent =
        "Это приватный канал. Его могут просматривать только избранные участники и пользователи с определёнными ролями.";
      warnBox.append(warnDot, warnTxt);
      topSection.append(warnBox);
    }

    const sectionHeader = document.createElement("div");
    sectionHeader.style.cssText =
      "padding:4px 20px 6px;font-size:12px;font-weight:600;color:#b5bac1;display:flex;align-items:center;gap:4px;";
    sectionHeader.textContent = isGroupInvite ? "Друзья ⌄" : "Друзья и участники ⌄";

    const listWrap = document.createElement("div");
    listWrap.style.cssText =
      "max-height:310px;overflow-y:auto;padding:0 12px 8px;display:flex;flex-direction:column;gap:2px;";

    const selfId = String(ctx.getLiveSelfUser()?.id || "");
    /** @type {Array<{ id: string, displayName: string, username: string, avatarUrl: string | null }>} */
    const candidates = [];
    const seenIds = new Set([selfId]);
    const pushCandidate = (
      /** @type {string} */ id,
      /** @type {string} */ displayName,
      /** @type {string} */ username,
      /** @type {string | null} */ avatarHash,
    ) => {
      if (!id || seenIds.has(id)) return;
      seenIds.add(id);
      const avatarUrl =
        avatarHash && avatarHash.startsWith("http")
          ? avatarHash
          : avatarHash
            ? `https://cdn.discordapp.com/avatars/${id}/${avatarHash}.png?size=64`
            : null;
      candidates.push({ id, displayName, username, avatarUrl });
    };

    const populateInviteCandidates = () => {
      candidates.length = 0;
      seenIds.clear();
      seenIds.add(selfId);
      const dmFriends = ctx.getDmFriendsController?.();
      const liveFriends = dmFriends?.getRelationships?.() ?? [];
      for (const rel of liveFriends) {
        const relUid = String(rel?.user_id ?? rel?.id ?? rel?.user?.id ?? "");
        if (Number(rel?.type) === 1 && relUid) {
          const known = ctx.liveUsersById.get(relUid) || rel?.user;
          const disp = String(
            rel.displayName ||
              rel.nickname ||
              known?.global_name ||
              known?.globalName ||
              rel.user?.global_name ||
              rel.user?.username ||
              known?.username ||
              relUid,
          );
          const uname = String(rel.user?.username || known?.username || disp).replace(/^@/, "");
          pushCandidate(relUid, disp, uname, known?.avatar || rel.user?.avatar || null);
        }
      }
      const privateChs = dmFriends?.getPrivateChannels?.() ?? [];
      for (const pc of privateChs) {
        const anyPc = /** @type {any} */ (pc);
        const recs = Array.isArray(anyPc?.recipients) ? anyPc.recipients : [];
        for (const rec of recs) {
          const rUid = String(rec?.id ?? "");
          if (!rUid) continue;
          const known = ctx.liveUsersById.get(rUid) || rec;
          const disp = String(
            known?.global_name ||
              known?.globalName ||
              rec?.global_name ||
              rec?.displayName ||
              known?.username ||
              rec?.username ||
              rUid,
          );
          const uname = String(known?.username || rec?.username || disp).replace(/^@/, "");
          pushCandidate(rUid, disp, uname, known?.avatar || rec?.avatar || null);
        }
      }
      if (!isGroupInvite && gId && gId !== HOME_GUILD_ID) {
        const guildMembers = ctx.liveGuildsById.get(gId)?.members ?? [];
        for (const m of guildMembers) {
          const mUser = m?.user || ctx.liveUsersById.get(String(m?.user_id ?? m?.id ?? ""));
          const mUid = String(m?.user_id ?? mUser?.id ?? m?.id ?? "");
          if (!mUid) continue;
          const disp = String(
            m?.nick ||
              mUser?.global_name ||
              mUser?.globalName ||
              mUser?.displayName ||
              mUser?.username ||
              mUid,
          );
          const uname = String(mUser?.username || disp).replace(/^@/, "");
          pushCandidate(mUid, disp, uname, m?.avatar || mUser?.avatar || null);
        }
      }
      if (!ctx.isLiveSession()) {
        for (const [uid, u] of ctx.liveUsersById.entries()) {
          const disp = String(u?.global_name || u?.globalName || u?.displayName || u?.username || uid);
          const uname = String(u?.username || disp).replace(/^@/, "");
          pushCandidate(uid, disp, uname, u?.avatar || null);
        }
      }
    };
    populateInviteCandidates();

    let currentInviteUrl = ctx.isLiveSession() ? "Создание ссылки..." : `https://discord.gg/a92Pr5e8H`;

    const bottomSection = document.createElement("div");
    bottomSection.style.cssText =
      "padding:14px 20px 18px;background:#232428;border-top:1px solid #1e1f22;display:flex;flex-direction:column;gap:8px;";
    const linkLabel = document.createElement("div");
    linkLabel.style.cssText = "font-size:13px;font-weight:600;color:#f2f3f5;";
    linkLabel.textContent = isGroupInvite
      ? "Или отправьте другу ссылку-приглашение в группу"
      : "Или отправьте другу ссылку-приглашение на сервер";

    const linkBox = document.createElement("div");
    linkBox.style.cssText =
      "display:flex;align-items:center;gap:8px;background:#1e1f22;border:1px solid #111214;border-radius:8px;padding:4px 4px 4px 12px;";
    const linkInput = document.createElement("input");
    linkInput.type = "text";
    linkInput.readOnly = true;
    linkInput.value = currentInviteUrl;
    linkInput.style.cssText =
      "flex:1;background:transparent;border:none;outline:none;color:#dbdee1;font-size:13px;";
    const copyLinkBtn = document.createElement("button");
    copyLinkBtn.type = "button";
    copyLinkBtn.textContent = "Копировать";
    copyLinkBtn.style.cssText =
      "padding:8px 16px;border:none;border-radius:6px;background:#5865f2;color:#ffffff;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0;";
    copyLinkBtn.addEventListener("click", () => {
      void navigator.clipboard?.writeText(currentInviteUrl).catch(() => {});
      copyLinkBtn.textContent = "Скопировано";
      copyLinkBtn.style.background = "#23a55a";
      setTimeout(() => {
        copyLinkBtn.textContent = "Копировать";
        copyLinkBtn.style.background = "#5865f2";
      }, 1800);
    });
    linkBox.append(linkInput, copyLinkBtn);

    const expireHint = document.createElement("div");
    expireHint.style.cssText = "font-size:11px;color:#949ba4;";
    expireHint.innerHTML = `Ваша ссылка-приглашение перестанет действовать через 24 часа. <span style="color:#00a8fc;cursor:pointer;">Изменить ссылку-приглашение.</span>`;

    const renderCandidateRows = (/** @type {string} */ query) => {
      listWrap.replaceChildren();
      const q = query.trim().toLowerCase();
      const filtered = candidates.filter(
        (c) => !q || c.displayName.toLowerCase().includes(q) || c.username.toLowerCase().includes(q),
      );
      if (filtered.length === 0) {
        const emptyRow = document.createElement("div");
        emptyRow.style.cssText = "padding:16px 8px;color:#949ba4;font-size:13px;text-align:center;";
        emptyRow.textContent = "Список друзей пуст — скопируйте ссылку-приглашение ниже.";
        listWrap.append(emptyRow);
        return;
      }
      for (const c of filtered) {
        const row = document.createElement("div");
        row.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:10px;padding:6px 8px;border-radius:8px;";
        row.addEventListener("mouseenter", () => {
          row.style.background = "rgba(255,255,255,0.04)";
        });
        row.addEventListener("mouseleave", () => {
          row.style.background = "transparent";
        });

        const left = document.createElement("div");
        left.style.cssText = "display:flex;align-items:center;gap:10px;min-width:0;flex:1;";
        const av = document.createElement("div");
        av.style.cssText =
          "width:36px;height:36px;border-radius:50%;background:#5865f2;color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:14px;font-weight:700;flex-shrink:0;background-size:cover;background-position:center;";
        av.textContent = Array.from(c.displayName).slice(0, 1).join("").toUpperCase();
        if (c.avatarUrl) {
          av.style.backgroundImage = `url("${c.avatarUrl}")`;
          av.style.color = "transparent";
        }
        const names = document.createElement("div");
        names.style.cssText = "display:flex;flex-direction:column;min-width:0;line-height:1.25;";
        const dName = document.createElement("span");
        dName.style.cssText =
          "font-size:14px;font-weight:600;color:#f2f3f5;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
        dName.textContent = c.displayName;
        const uName = document.createElement("span");
        uName.style.cssText =
          "font-size:12px;color:#949ba4;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
        uName.textContent = c.username;
        names.append(dName, uName);
        left.append(av, names);

        const invBtn = document.createElement("button");
        invBtn.type = "button";
        invBtn.textContent = "Пригласить";
        invBtn.style.cssText =
          "padding:7px 14px;border-radius:6px;border:1px solid #4e5058;background:#35373c;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0;";
        invBtn.addEventListener("click", async () => {
          invBtn.disabled = true;
          invBtn.textContent = "Отправка...";
          try {
            if (ctx.isLiveSession()) {
              if (!currentInviteUrl.startsWith("http")) {
                const created = await ctx.invoke("create_invite", {
                  channelId: String(channel.id),
                  maxAge: 86400,
                  maxUses: 0,
                  temporary: false,
                  unique: false,
                });
                if (created?.url || created?.code) {
                  currentInviteUrl = String(created.url || `https://discord.gg/${created.code}`);
                  linkInput.value = currentInviteUrl;
                }
              }
              if (isGroupInvite) {
                const putRes = await ctx.sendDmRest({
                  method: "PUT",
                  url: `/channels/${channel.id}/recipients/${c.id}`,
                });
                const createdGroup = putRes?.body;
                if (createdGroup && typeof createdGroup === "object" && createdGroup.id) {
                  const dmSync = await ctx
                    .invoke(ctx.ipcCmds.setAudioDevice, {
                      kind: "dm_friends_sync",
                      deviceId: "poll",
                    })
                    .catch(() => null);
                  if (dmSync && typeof dmSync === "object") {
                    ctx.getDmFriendsController?.()?.syncLiveState?.({
                      relationships: dmSync.relationships,
                      privateChannels: dmSync.privateChannels,
                      users: dmSync.users,
                      presences: dmSync.presences,
                    });
                  }
                  ctx.getDmFriendsController?.()?.selectDmChannel?.(String(createdGroup.id));
                }
              } else {
                const dmRes = await ctx.sendDmRest({
                  method: "POST",
                  url: "/users/@me/channels",
                  body: { recipients: [c.id] },
                });
                const dmChannelId = String(dmRes?.body?.id || "");
                if (dmChannelId && currentInviteUrl.startsWith("http")) {
                  await ctx.invoke("send_message", {
                    channelId: dmChannelId,
                    content: currentInviteUrl,
                  });
                }
              }
            }
            invBtn.textContent = "Отправлено";
            invBtn.style.background = "transparent";
            invBtn.style.borderColor = "#23a55a";
            invBtn.style.color = "#23a55a";
          } catch {
            invBtn.disabled = false;
            invBtn.textContent = "Ошибка";
            invBtn.style.borderColor = "#f23f43";
            invBtn.style.color = "#f23f43";
          }
          ctx.emitLiveTelemetry({
            type: "voice_invite_friend",
            channelId: channel.id,
            channelName: chName,
            targetUserId: c.id,
            targetUsername: c.username,
          });
        });
        row.append(left, invBtn);
        listWrap.append(row);
      }
    };
    renderCandidateRows("");
    searchInput.addEventListener("input", () => {
      renderCandidateRows(searchInput.value);
    });

    if (ctx.isLiveSession() && channel?.id) {
      void ctx
        .invoke(ctx.ipcCmds.setAudioDevice, {
          kind: "dm_friends_sync",
          deviceId: "poll",
        })
        .then((/** @type {any} */ dmSync) => {
          if (dmSync && typeof dmSync === "object" && dmSync.ok !== false) {
            if (Array.isArray(dmSync.users)) {
              for (const u of dmSync.users) {
                if (u?.id) ctx.liveUsersById.set(String(u.id), u);
              }
            }
            if (Array.isArray(dmSync.guilds)) {
              for (const g of dmSync.guilds) {
                if (g?.id) ctx.hydrateGuildMembersAndRoles?.(g);
              }
            }
            ctx.getDmFriendsController?.()?.syncLiveState?.({
              relationships: dmSync.relationships,
              privateChannels: dmSync.privateChannels,
              users: dmSync.users,
              presences: dmSync.presences,
            });
            populateInviteCandidates();
            renderCandidateRows(searchInput.value);
          }
        })
        .catch(() => {});
      void ctx
        .invoke("create_invite", {
          channelId: String(channel.id),
          maxAge: 86400,
          maxUses: 0,
          temporary: false,
          unique: false,
        })
        .then((/** @type {any} */ res) => {
          if (res?.url || res?.code) {
            currentInviteUrl = String(res.url || `https://discord.gg/${res.code}`);
            linkInput.value = currentInviteUrl;
          } else {
            linkInput.value = "Не удалось создать ссылку (нет прав)";
          }
        })
        .catch(() => {
          linkInput.value = "Не удалось создать ссылку (нет прав)";
        });
    }

    bottomSection.append(linkLabel, linkBox, expireHint);
    card.append(topSection, sectionHeader, listWrap, bottomSection);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) backdrop.remove();
    });
    document.body.append(backdrop);
  }

  /**
   * Opens the microphone or speaker device selection popover from the user bar chevron.
   * @param {"input" | "output"} kind
   * @param {HTMLElement} anchor
   */
  async function openAudioDevicePopover(kind, anchor) {
    void anchor;
    const existing = document.getElementById("live-audio-device-popout");
    if (existing && existing.getAttribute("data-kind") === kind) {
      existing.remove();
      return;
    }
    ctx.closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-audio-device-popout";
    pop.setAttribute("data-kind", kind);
    pop.setAttribute("role", "dialog");
    pop.style.cssText =
      "position:fixed;left:68px;bottom:64px;width:288px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:12px;box-shadow:0 12px 28px rgba(0,0,0,0.65);z-index:9500;display:flex;flex-direction:column;gap:10px;font-size:13px;";

    const modeSection = document.createElement("div");
    modeSection.style.cssText = "display:flex;flex-direction:column;gap:4px;padding-bottom:8px;border-bottom:1px solid #2b2d31;";
    const modeHeading = document.createElement("div");
    modeHeading.style.cssText = "font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;margin-bottom:2px;";
    modeHeading.textContent = "Режим блокировки звука";

    const micOnlyBtn = document.createElement("button");
    micOnlyBtn.type = "button";
    micOnlyBtn.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;width:100%;padding:6px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;font-size:13px;cursor:pointer;text-align:left;";
    const isMicOnlyMuted = ctx.selfVoicePrefs.muted && !ctx.selfVoicePrefs.deafened;
    micOnlyBtn.innerHTML = `<span>🎙 Блокировать только микрофон</span><span style="color:${isMicOnlyMuted ? "#23a55a" : "#949ba4"};font-weight:700;">${isMicOnlyMuted ? "ВКЛ" : "ВЫКЛ"}</span>`;
    micOnlyBtn.addEventListener("click", () => {
      if (isMicOnlyMuted) {
        ctx.selfVoicePrefs.muted = false;
      } else {
        ctx.selfVoicePrefs.deafened = false;
        ctx.selfVoicePrefs.muted = true;
      }
      ctx.syncVoiceMuteStateWithBackend();
      ctx.renderLiveVoicePanel();
      ctx.getTree?.()?.refreshView?.();
      pop.remove();
    });

    const bothMuteBtn = document.createElement("button");
    bothMuteBtn.type = "button";
    bothMuteBtn.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;width:100%;padding:6px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;font-size:13px;cursor:pointer;text-align:left;";
    const isBothMuted = ctx.selfVoicePrefs.deafened;
    bothMuteBtn.innerHTML = `<span>🎧 Блокировать микрофон и динамики</span><span style="color:${isBothMuted ? "#f23f43" : "#949ba4"};font-weight:700;">${isBothMuted ? "ВКЛ" : "ВЫКЛ"}</span>`;
    bothMuteBtn.addEventListener("click", () => {
      ctx.toggleSelfDeafenBoth(!ctx.selfVoicePrefs.deafened);
      pop.remove();
    });
    modeSection.append(modeHeading, micOnlyBtn, bothMuteBtn);
    pop.append(modeSection);

    const devSection = document.createElement("div");
    devSection.style.cssText = "display:flex;flex-direction:column;gap:4px;padding-bottom:8px;border-bottom:1px solid #2b2d31;";
    const devTitle = document.createElement("div");
    devTitle.style.cssText = "font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;margin-bottom:2px;";
    devTitle.textContent = kind === "input" ? "Устройство ввода" : "Устройство вывода";
    devSection.append(devTitle);

    let deviceList =
      kind === "input"
        ? [{ id: "default", name: "Системный микрофон (По умолчанию)", isDefault: true }]
        : [{ id: "default", name: "Системные динамики (По умолчанию)", isDefault: true }];
    try {
      const fetched = await ctx.invoke(ctx.ipcCmds.audioDevices);
      const rawArr = kind === "input" ? fetched?.inputs : fetched?.outputs;
      if (Array.isArray(rawArr) && rawArr.length > 0) {
        deviceList = rawArr;
      }
    } catch {}

    const currentDevId =
      kind === "input" ? ctx.selfVoicePrefs.inputDeviceId : ctx.selfVoicePrefs.outputDeviceId;
    for (const dev of deviceList) {
      const devBtn = document.createElement("button");
      devBtn.type = "button";
      const selected = dev.id === currentDevId || (currentDevId === "default" && dev.isDefault);
      devBtn.style.cssText =
        `display:flex;align-items:center;justify-content:space-between;width:100%;padding:6px 8px;border:none;border-radius:6px;background:${selected ? "#35373c" : "transparent"};color:#f2f3f5;font-size:12px;cursor:pointer;text-align:left;gap:8px;`;
      const nameSpan = document.createElement("span");
      nameSpan.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;";
      nameSpan.textContent = String(dev.name || dev.id);
      const radioDot = document.createElement("span");
      radioDot.style.cssText = `width:14px;height:14px;border-radius:50%;border:2px solid ${selected ? "#5865f2" : "#80848e"};background:${selected ? "#5865f2" : "transparent"};flex-shrink:0;`;
      devBtn.append(nameSpan, radioDot);
      devBtn.addEventListener("click", () => {
        if (kind === "input") ctx.selfVoicePrefs.inputDeviceId = String(dev.id);
        else ctx.selfVoicePrefs.outputDeviceId = String(dev.id);
        ctx.syncVoiceMuteStateWithBackend();
        pop.remove();
      });
      devSection.append(devBtn);
    }
    pop.append(devSection);

    const volSection = document.createElement("div");
    volSection.style.cssText = "display:flex;flex-direction:column;gap:6px;";
    const volHeader = document.createElement("div");
    volHeader.style.cssText = "display:flex;justify-content:space-between;font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;";
    const volLabel = document.createElement("span");
    volLabel.textContent = kind === "input" ? "Громкость микрофона" : "Громкость звука";
    const volValue = document.createElement("span");
    const initVol = kind === "input" ? ctx.selfVoicePrefs.inputVolume : ctx.selfVoicePrefs.outputVolume;
    volValue.textContent = `${initVol}%`;
    volHeader.append(volLabel, volValue);

    const slider = document.createElement("input");
    slider.type = "range";
    slider.min = "0";
    slider.max = kind === "input" ? "100" : "200";
    slider.value = String(initVol);
    slider.style.cssText = "width:100%;accent-color:#5865f2;cursor:pointer;";
    slider.addEventListener("input", () => {
      const v = Number(slider.value);
      volValue.textContent = `${v}%`;
      if (kind === "input") ctx.selfVoicePrefs.inputVolume = v;
      else ctx.selfVoicePrefs.outputVolume = v;
      ctx.syncVoiceMuteStateWithBackend();
    });

    const hotplugBtn = document.createElement("button");
    hotplugBtn.type = "button";
    hotplugBtn.style.cssText =
      "margin-top:4px;padding:6px 8px;border:none;border-radius:6px;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;text-align:left;";
    hotplugBtn.textContent = "🔄 Быстрая смена аудиоустройства";
    hotplugBtn.addEventListener("click", () => {
      pop.remove();
      void openAudioDeviceHotplugPrompt();
    });

    const settingsBtn = document.createElement("button");
    settingsBtn.type = "button";
    settingsBtn.style.cssText =
      "margin-top:2px;padding:6px 8px;border:none;border-radius:6px;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;text-align:left;";
    settingsBtn.textContent = "⚙ Голос и видео (Все настройки)";
    settingsBtn.addEventListener("click", () => {
      pop.remove();
      ctx.getRouter?.()?.switchScreen?.("settings");
      ctx.getSettingsRef?.()?.showPane?.("voice");
    });

    volSection.append(volHeader, slider, hotplugBtn, settingsBtn);
    pop.append(volSection);
    document.body.append(pop);
  }

  /**
   * Shows the "New audio device detected" hotplug card above the voice panel.
   * @param {{ id?: string, name?: string, mode?: "both" | "selector" }} [opts]
   */
  async function openAudioDeviceHotplugPrompt(opts = {}) {
    if (ctx.getHotplugPromptsSuppressed()) return;
    document.getElementById("live-audio-hotplug-card")?.remove();
    document.getElementById("live-audio-hotplug-more-menu")?.remove();

    /** @type {Array<{ id: string, name: string }>} */
    let inputs = [];
    /** @type {Array<{ id: string, name: string }>} */
    let outputs = [];
    try {
      const res = await ctx.invoke(ctx.ipcCmds.audioDevices);
      if (Array.isArray(res?.inputs)) inputs = res.inputs;
      if (Array.isArray(res?.outputs)) outputs = res.outputs;
    } catch {}
    if (inputs.length === 0) {
      inputs = [{ id: "Easy Effects Source", name: "Easy Effects Source" }];
    }
    if (outputs.length === 0) {
      outputs = [{ id: "Easy Effects Sink", name: "Easy Effects Sink" }];
    }

    let selectedId = opts.id || inputs[0]?.id || "Easy Effects Source";
    let selectedName = opts.name || inputs.find((d) => d.id === selectedId)?.name || selectedId;
    const mode = opts.mode || "selector";

    const card = document.createElement("div");
    card.id = "live-audio-hotplug-card";
    card.setAttribute("role", "region");
    card.setAttribute("aria-label", "Обнаружено новое аудиоустройство");
    const bottomOffset = ctx.getActiveVoiceConnection() ? 156 : 64;
    card.style.cssText =
      `position:fixed;left:8px;bottom:${bottomOffset}px;width:232px;background:#18191c;color:#f2f3f5;border:1px solid #2b2d31;border-radius:10px;padding:12px;box-shadow:0 14px 36px rgba(0,0,0,0.78);z-index:9450;display:flex;flex-direction:column;gap:10px;font-size:12px;`;

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:flex-start;justify-content:space-between;gap:8px;";
    const titleSpan = document.createElement("span");
    titleSpan.style.cssText = "font-size:12px;font-weight:700;color:#f2f3f5;line-height:1.35;";
    titleSpan.textContent =
      mode === "both"
        ? `Сменить устройство ввода и вывода звука на ${selectedName}?`
        : "Обнаружено новое аудиоустройство";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "background:none;border:none;color:#949ba4;font-size:13px;cursor:pointer;padding:0;";
    closeBtn.addEventListener("click", () => {
      document.getElementById("live-audio-hotplug-more-menu")?.remove();
      card.remove();
    });
    headRow.append(titleSpan, closeBtn);
    card.append(headRow);

    if (mode === "selector") {
      const selWrap = document.createElement("div");
      selWrap.style.cssText = "position:relative;";
      const selBtn = document.createElement("button");
      selBtn.type = "button";
      selBtn.style.cssText =
        "width:100%;height:34px;padding:0 10px;border-radius:6px;border:1px solid #383a40;background:#232428;color:#f2f3f5;font-size:12px;display:flex;align-items:center;justify-content:space-between;gap:6px;cursor:pointer;text-align:left;";
      const selTxt = document.createElement("span");
      selTxt.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;";
      selTxt.textContent = `🎙 ${selectedName}`;
      const selChev = document.createElement("span");
      selChev.textContent = "⌄";
      selBtn.append(selTxt, selChev);

      const selMenu = document.createElement("div");
      selMenu.setAttribute("hidden", "");
      selMenu.style.cssText =
        "position:absolute;left:0;right:0;bottom:calc(100% + 4px);background:#232428;border:1px solid #383a40;border-radius:8px;padding:4px;box-shadow:0 12px 28px rgba(0,0,0,0.75);z-index:9550;max-height:180px;overflow-y:auto;display:flex;flex-direction:column;gap:2px;";
      const allDevs = [
        ...inputs.map((d) => ({ id: d.id, name: d.name, kind: "input" })),
        ...outputs
          .filter((o) => !inputs.some((i) => i.id === o.id))
          .map((d) => ({ id: d.id, name: d.name, kind: "output" })),
      ];
      for (const dev of allDevs) {
        const opt = document.createElement("button");
        opt.type = "button";
        opt.style.cssText =
          "padding:6px 8px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:12px;cursor:pointer;text-align:left;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
        opt.textContent = `${dev.kind === "output" ? "🎧" : "🎙"} ${dev.name}`;
        opt.addEventListener("click", () => {
          selectedId = dev.id;
          selectedName = dev.name;
          selTxt.textContent = `${dev.kind === "output" ? "🎧" : "🎙"} ${selectedName}`;
          selMenu.setAttribute("hidden", "");
        });
        selMenu.append(opt);
      }
      selBtn.addEventListener("click", () => {
        if (selMenu.hasAttribute("hidden")) selMenu.removeAttribute("hidden");
        else selMenu.setAttribute("hidden", "");
      });
      selWrap.append(selBtn, selMenu);
      card.append(selWrap);
    }

    const actionsRow = document.createElement("div");
    actionsRow.style.cssText = "display:flex;align-items:center;gap:6px;";
    const applyBtn = document.createElement("button");
    applyBtn.type = "button";
    applyBtn.style.cssText =
      "flex:1;height:32px;padding:0 10px;border:none;border-radius:6px;background:#5865f2;color:#ffffff;font-size:12px;font-weight:600;cursor:pointer;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
    applyBtn.textContent =
      mode === "both" ? "Сменить устройства ввода и вывода" : "Сменить устройство ввода";
    applyBtn.addEventListener("click", () => {
      ctx.selfVoicePrefs.inputDeviceId = selectedId;
      if (mode === "both") {
        const matchOut = outputs.find((o) => o.name === selectedName || o.id === selectedId);
        if (matchOut) ctx.selfVoicePrefs.outputDeviceId = matchOut.id;
      }
      ctx.syncVoiceMuteStateWithBackend();
      document.getElementById("live-audio-hotplug-more-menu")?.remove();
      card.remove();
    });

    const moreBtn = document.createElement("button");
    moreBtn.type = "button";
    moreBtn.id = "live-hotplug-more-btn";
    moreBtn.style.cssText =
      "width:32px;height:32px;border:none;border-radius:6px;background:#2b2d31;color:#dbdee1;font-size:14px;font-weight:700;cursor:pointer;flex-shrink:0;";
    moreBtn.textContent = "⋯";
    moreBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      const existingMenu = document.getElementById("live-audio-hotplug-more-menu");
      if (existingMenu) {
        existingMenu.remove();
        return;
      }
      const m = document.createElement("div");
      m.id = "live-audio-hotplug-more-menu";
      m.style.cssText =
        `position:fixed;left:24px;bottom:${bottomOffset + 48}px;width:236px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:8px;padding:6px;box-shadow:0 14px 34px rgba(0,0,0,0.8);z-index:9550;display:flex;flex-direction:column;gap:2px;font-size:12px;`;

      const addOpt = (
        /** @type {string} */ label,
        /** @type {string} */ icon,
        /** @type {boolean} */ danger,
        /** @type {() => void} */ fn,
      ) => {
        const b = document.createElement("button");
        b.type = "button";
        b.style.cssText =
          `display:flex;align-items:center;justify-content:space-between;gap:8px;padding:7px 8px;border:none;border-radius:5px;background:transparent;color:${danger ? "#f23f43" : "#dbdee1"};font-size:12px;font-weight:500;cursor:pointer;text-align:left;`;
        b.innerHTML = `<span style="overflow:hidden;text-overflow:ellipsis;">${label}</span><span style="flex-shrink:0;">${icon}</span>`;
        b.addEventListener("mouseenter", () => {
          b.style.background = danger ? "rgba(242,63,67,0.16)" : "#2b2d31";
        });
        b.addEventListener("mouseleave", () => {
          b.style.background = "transparent";
        });
        b.addEventListener("click", () => {
          fn();
          m.remove();
          card.remove();
        });
        m.append(b);
      };

      addOpt("Сменить только устройство ввода", "🎙", false, () => {
        ctx.selfVoicePrefs.inputDeviceId = selectedId;
        ctx.syncVoiceMuteStateWithBackend();
      });
      addOpt("Сменить только устройство вывода", "🎧", false, () => {
        const matchOut = outputs.find((o) => o.name === selectedName || o.id === selectedId) ?? outputs[0];
        if (matchOut) ctx.selfVoicePrefs.outputDeviceId = matchOut.id;
        ctx.syncVoiceMuteStateWithBackend();
      });
      const div = document.createElement("div");
      div.style.cssText = "height:1px;background:#2b2d31;margin:4px 0;";
      m.append(div);
      addOpt(`Не предлагать ${selectedName} снова`, "🎙̸", true, () => {
        ctx.ignoredHotplugDeviceIds.add(selectedId);
      });
      addOpt("Больше не показывать", "🎙̸", true, () => {
        ctx.setHotplugPromptsSuppressed(true);
      });
      document.body.append(m);
    });

    actionsRow.append(applyBtn, moreBtn);
    card.append(actionsRow);
    document.body.append(card);
  }

  /**
   * Lazily mounts the preliminary voice devices settings section.
   */
  const mountVoiceDevices = () => {
    const existing = ctx.getVoiceDevices();
    if (existing) return existing;
    const settings = ctx.getSettingsObject?.();
    settings?.voiceDevices?.destroy?.();
    const voiceDevicesHost = ctx.getVoiceDevicesHost?.();
    const voiceDevicesSession = ctx.getVoiceDevicesSession?.();
    const created = createVoiceDevicesSettings({
      host: voiceDevicesHost,
      devices: ctx.DEFAULT_VOICE_DEVICES,
      service: {
        selectInput: (deviceId) => {
          if (voiceDevicesSession) voiceDevicesSession.inputDeviceId = deviceId;
        },
        selectOutput: (deviceId) => {
          if (voiceDevicesSession) voiceDevicesSession.outputDeviceId = deviceId;
        },
        setInputVolume: (percent) => {
          if (voiceDevicesSession) voiceDevicesSession.inputVolume = percent;
        },
        setOutputVolume: (percent) => {
          if (voiceDevicesSession) voiceDevicesSession.outputVolume = percent;
        },
        setInputMode: (mode) => {
          if (voiceDevicesSession) voiceDevicesSession.inputMode = mode.kind;
        },
        setUserMute: (mute) => {
          if (voiceDevicesSession) voiceDevicesSession.mute = mute;
        },
        setSelfDeaf: (deaf) => {
          if (voiceDevicesSession) voiceDevicesSession.deaf = deaf;
        },
        setPttKey: (key) => {
          if (voiceDevicesSession) voiceDevicesSession.pttKey = key;
        },
      },
    });
    ctx.setVoiceDevices(created);
    voiceDevicesHost?.setAttribute("data-preliminary", "true");
    return created;
  };

  /**
   * @param {boolean} expanded
   */
  const setVoiceDevicesExpanded = (expanded) => {
    const voiceDevicesHost = ctx.getVoiceDevicesHost?.();
    const voiceDevicesExpand = ctx.getVoiceDevicesExpand?.();
    if (expanded) mountVoiceDevices();
    if (expanded) voiceDevicesHost?.removeAttribute("hidden");
    else voiceDevicesHost?.setAttribute("hidden", "");
    if (!voiceDevicesExpand) return;
    voiceDevicesExpand.setAttribute("aria-expanded", expanded ? "true" : "false");
    voiceDevicesExpand.textContent = expanded
      ? "Скрыть настройки устройств"
      : "Показать настройки устройств";
  };

  const refreshLiveAudioDevices = async () => {
    try {
      const settingsRoot = ctx.byId("settings");
      const fetched = /** @type {any} */ (await ctx.invoke(ctx.ipcCmds.audioDevices));
      const inputs = Array.isArray(fetched?.inputs) ? fetched.inputs : [];
      const outputs = Array.isArray(fetched?.outputs) ? fetched.outputs : [];
      const inSel = /** @type {HTMLSelectElement | null} */ (
        settingsRoot?.querySelector('select[data-setting="voiceInputDevice"], #settings-voice-input-select')
      );
      if (inSel && inputs.length > 0) {
        inSel.replaceChildren();
        for (const dev of inputs) {
          const opt = document.createElement("option");
          opt.value = String(dev.id || "default");
          opt.textContent = String(dev.name || dev.id || "Микрофон");
          inSel.append(opt);
        }
      }
      const outSel = /** @type {HTMLSelectElement | null} */ (
        settingsRoot?.querySelector('select[data-setting="voiceOutputDevice"], #settings-voice-output-select')
      );
      if (outSel && outputs.length > 0) {
        outSel.replaceChildren();
        for (const dev of outputs) {
          const opt = document.createElement("option");
          opt.value = String(dev.id || "default");
          opt.textContent = String(dev.name || dev.id || "Динамики");
          outSel.append(opt);
        }
      }
    } catch {}
  };

  const ensureVoiceTroubleshootControls = () => {
    const settingsRoot = ctx.byId("settings");
    if (!settingsRoot) return;
    const h264Input = /** @type {HTMLInputElement | null} */ (
      settingsRoot.querySelector('input[data-setting-id="advanced.openh264"]')
    );
    if (h264Input && h264Input.getAttribute("data-live-openh264-bound") !== "true") {
      h264Input.setAttribute("data-live-openh264-bound", "true");
      const lbl = h264Input.closest(".toggle")?.querySelector(".toggle-label");
      if (lbl) lbl.textContent = "Аппаратное ускорение H.264 (OpenH264)";
    }
  };

  const setVoicePanelVisibility = () => {
    const settingsRoot = ctx.byId("settings");
    const preliminaryBlock = ctx.getPreliminaryBlock?.();
    const settingsModal = ctx.getSettingsModal?.();
    const screenSettingsEl = typeof document !== "undefined" ? document.getElementById("screen-settings") : null;
    const isSettingsScreenOpen = !screenSettingsEl
      ? Boolean(settingsRoot && !settingsRoot.hidden && !settingsRoot.hasAttribute?.("hidden"))
      : !screenSettingsEl.hidden &&
        !screenSettingsEl.hasAttribute?.("hidden") &&
        screenSettingsEl.getAttribute?.("aria-hidden") !== "true";
    if (settingsRoot?.getAttribute("data-active-pane") === "voice") {
      if (ctx.isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) {
        preliminaryBlock?.setAttribute("hidden", "");
        settingsModal?.setAttribute("hidden", "");
        ensureVoiceTroubleshootControls();
        if (isSettingsScreenOpen) {
          ctx.stopAudioDeviceLevelsPolling();
          ctx.startAudioDeviceLevelsPolling();
          void ctx.pollAudioDeviceLevels(true);
        } else {
          ctx.stopAudioDeviceLevelsPolling();
          if (ctx.getActiveVoiceConnection()) {
            ctx.startAudioDeviceLevelsPolling();
          }
        }
      } else {
        settingsModal?.removeAttribute("hidden");
      }
    } else {
      settingsModal?.setAttribute("hidden", "");
      ctx.stopAudioDeviceLevelsPolling();
      if (ctx.getActiveVoiceConnection()) {
        ctx.startAudioDeviceLevelsPolling();
      }
    }
  };

  return {
    openVoiceInviteModal,
    openAudioDevicePopover,
    openAudioDeviceHotplugPrompt,
    mountVoiceDevices,
    setVoiceDevicesExpanded,
    refreshLiveAudioDevices,
    ensureVoiceTroubleshootControls,
    setVoicePanelVisibility,
  };
}
