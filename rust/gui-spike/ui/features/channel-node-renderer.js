// @ts-check
/**
 * Channel tree node DOM renderer and voice participant row builders.
 * Extracted from `app.js` as part of RW51-EPIC2-T3.
 * @module ui/features/channel-node-renderer
 */

/**
 * @typedef {{
 *   id: string,
 *   type: number,
 *   name: string,
 *   parentId: string,
 *   position: number,
 *   lastMessageId?: string,
 *   unread?: boolean,
 *   mentionCount?: number,
 *   permissionOverwrites?: Array<any>,
 *   userLimit?: number,
 * }} TreeChannel
 */

/**
 * @typedef {{ key: string, title: string, icon: string, openable: boolean, reason: string }} ChannelKind
 */

const SVG_VOICE_LOCKED =
  `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M11.383 3.079C11.009 2.925 10.579 3.01 10.293 3.296L6 8.002H3C2.45 8.002 2 8.453 2 9.002V15.002C2 15.552 2.45 16.002 3 16.002H6L10.293 20.71C10.579 20.996 11.009 21.082 11.383 20.927C11.757 20.772 12 20.407 12 20.002V4.002C12 3.599 11.757 3.232 11.383 3.079Z"/><path d="M16 4.5C16 3.12 17.12 2 18.5 2C19.88 2 21 3.12 21 4.5V6H21.5C22.05 6 22.5 6.45 22.5 7V10.5C22.5 11.05 22.05 11.5 21.5 11.5H15.5C14.95 11.5 14.5 11.05 14.5 10.5V7C14.5 6.45 14.95 6 15.5 6H16V4.5ZM17.3 6H19.7V4.5C19.7 3.84 19.16 3.3 18.5 3.3C17.84 3.3 17.3 3.84 17.3 4.5V6Z"/><path d="M14 13.2C15.4 13.7 16.4 15.0 16.5 16.6H14.8C14.7 15.8 14.4 15.2 14 14.9V13.2ZM14 16.8C16.8 16.8 19.2 14.9 19.8 12.5H21.6C20.9 15.9 17.8 18.6 14 18.6V16.8Z"/></svg>`;

const SVG_VOICE_OPEN =
  `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M11.383 3.07904C11.009 2.92504 10.579 3.01004 10.293 3.29604L6 8.00204H3C2.45 8.00204 2 8.45304 2 9.00204V15.002C2 15.552 2.45 16.002 3 16.002H6L10.293 20.71C10.579 20.996 11.009 21.082 11.383 20.927C11.757 20.772 12 20.407 12 20.002V4.00204C12 3.59904 11.757 3.23204 11.383 3.07904ZM14 5.00195V7.00195C16.757 7.00195 19 9.24595 19 12.002C19 14.759 16.757 17.002 14 17.002V19.002C17.86 19.002 21 15.863 21 12.002C21 8.14295 17.86 5.00195 14 5.00195ZM14 9.00195C15.654 9.00195 17 10.349 17 12.002C17 13.657 15.654 15.002 14 15.002V13.002C14.551 13.002 15 12.553 15 12.002C15 11.451 14.551 11.002 14 11.002V9.00195Z"/></svg>`;

const SVG_TEXT_LOCKED =
  `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M5.88 21L6.58 17H2.58L2.93 15H6.93L8.02 9H4.02L4.37 7H8.37L9.07 3H11.07L10.37 7H13.5V9H10.02L8.93 15H16.93L16.58 17H8.58L7.88 21H5.88Z"/><path d="M16 4.5C16 3.12 17.12 2 18.5 2C19.88 2 21 3.12 21 4.5V6H21.5C22.05 6 22.5 6.45 22.5 7V10.5C22.5 11.05 22.05 11.5 21.5 11.5H15.5C14.95 11.5 14.5 11.05 14.5 10.5V7C14.5 6.45 14.95 6 15.5 6H16V4.5ZM17.3 6H19.7V4.5C19.7 3.84 19.16 3.3 18.5 3.3C17.84 3.3 17.3 3.84 17.3 4.5V6Z"/></svg>`;

const SVG_CHAT_BUBBLE =
  `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M4.79805 3C3.80445 3 2.99805 3.8055 2.99805 4.8V15.6C2.99805 16.5936 3.80445 17.4 4.79805 17.4H7.49805V21L11.098 17.4H19.198C20.1925 17.4 20.998 16.5936 20.998 15.6V4.8C20.998 3.8055 20.1925 3 19.198 3H4.79805Z"/></svg>`;

const SVG_INVITE_USER =
  `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M14 8C14 10.21 12.21 12 10 12C7.79 12 6 10.21 6 8C6 5.79 7.79 4 10 4C12.21 4 14 5.79 14 8ZM2 18V20H18V18C18 15.34 12.67 14 10 14C7.33 14 2 15.34 2 18ZM20 10V7H18V10H15V12H18V15H20V12H23V10H20Z"/></svg>`;

/**
 * Attach standard hover highlight handlers to an element.
 * @param {HTMLElement} el
 * @param {{ enterBg: string, enterColor: string, leaveBg: string, leaveColor: string }} colors
 */
function attachHoverColors(el, colors) {
  el.addEventListener("mouseenter", () => {
    el.style.background = colors.enterBg;
    el.style.color = colors.enterColor;
  });
  el.addEventListener("mouseleave", () => {
    el.style.background = colors.leaveBg;
    el.style.color = colors.leaveColor;
  });
}

/**
 * Create a small icon button used inside voice channel quick actions.
 * @param {string} label
 * @param {string} svgMarkup
 * @param {(ev: MouseEvent) => void} onClick
 * @returns {HTMLSpanElement}
 */
function createVoiceQuickButton(label, svgMarkup, onClick) {
  const btn = document.createElement("span");
  btn.setAttribute("role", "button");
  btn.setAttribute("aria-label", label);
  btn.title = label;
  btn.style.cssText =
    "display:inline-flex;align-items:center;justify-content:center;width:20px;height:20px;border-radius:4px;color:#b5bac1;cursor:pointer;";
  btn.innerHTML = svgMarkup;
  attachHoverColors(btn, {
    enterBg: "rgba(255,255,255,0.08)",
    enterColor: "#f2f3f5",
    leaveBg: "transparent",
    leaveColor: "#b5bac1",
  });
  btn.addEventListener("click", (ev) => {
    ev.stopPropagation();
    onClick(ev);
  });
  return btn;
}

/**
 * Create the channel icon span with appropriate SVG or glyph.
 * @param {{
 *   channel: TreeChannel,
 *   kind: ChannelKind,
 *   liveSession: boolean,
 *   liveVoice: boolean,
 *   isRestricted: boolean,
 *   hasActiveUsers: boolean,
 * }} params
 * @returns {HTMLSpanElement}
 */
function buildChannelIcon({
  channel,
  kind,
  liveSession,
  liveVoice,
  isRestricted,
  hasActiveUsers,
}) {
  const icon = document.createElement("span");
  icon.className = "channel-icon";
  icon.setAttribute("aria-hidden", "true");

  if (liveVoice) {
    icon.style.color = hasActiveUsers ? "#23a55a" : "#80848e";
    icon.style.display = "inline-flex";
    icon.style.alignItems = "center";
    icon.innerHTML = isRestricted ? SVG_VOICE_LOCKED : SVG_VOICE_OPEN;
  } else if (liveSession && (channel.type === 0 || channel.type === 5) && isRestricted) {
    icon.style.display = "inline-flex";
    icon.style.alignItems = "center";
    icon.innerHTML = SVG_TEXT_LOCKED;
  } else {
    icon.textContent = kind.icon;
  }
  return icon;
}

/**
 * Create the voice channel reason/counter/quick-actions badge cluster.
 * @param {{
 *   channel: TreeChannel,
 *   guildId: string,
 *   button: HTMLButtonElement,
 *   liveVoice: boolean,
 *   voiceParticipants: Array<any>,
 *   isConnectedVoiceHere: boolean,
 *   isRestricted: boolean,
 *   options: Record<string, any>,
 * }} params
 * @returns {HTMLSpanElement}
 */
function buildVoiceReason({
  channel,
  guildId,
  button,
  liveVoice,
  voiceParticipants,
  isConnectedVoiceHere,
  isRestricted,
  options,
}) {
  const reason = document.createElement("span");
  reason.className = "channel-reason";
  reason.style.display = "inline-flex";
  reason.style.alignItems = "center";
  reason.style.gap = "6px";

  if (voiceParticipants.length > 0 && typeof options.getVoiceChannelTimer === "function") {
    const timerSpan = document.createElement("span");
    timerSpan.setAttribute("data-voice-timer-cid", channel.id);
    timerSpan.setAttribute("data-voice-timer-gid", guildId);
    timerSpan.setAttribute("data-voice-timer-count", String(voiceParticipants.length));
    timerSpan.textContent = options.getVoiceChannelTimer(guildId, channel.id, voiceParticipants.length);
    timerSpan.style.color = "#23a55a";
    timerSpan.style.fontWeight = "600";
    timerSpan.style.fontVariantNumeric = "tabular-nums";
    reason.append(timerSpan);
  }

  if (liveVoice && channel.userLimit && channel.userLimit > 0) {
    const countPill = document.createElement("span");
    countPill.style.cssText =
      "display:inline-flex;align-items:center;border-radius:8px;background:#2b2d31;border:1px solid rgba(255,255,255,0.06);color:#b5bac1;font-size:10px;font-weight:700;font-variant-numeric:tabular-nums;overflow:hidden;";
    const leftNum = document.createElement("span");
    leftNum.style.cssText = "padding:1px 4px 1px 5px;color:#dbdee1;";
    leftNum.textContent = String(voiceParticipants.length).padStart(2, "0");
    const sep = document.createElement("span");
    sep.style.cssText = "width:1px;height:10px;background:rgba(255,255,255,0.14);";
    const rightNum = document.createElement("span");
    rightNum.style.cssText = "padding:1px 5px 1px 4px;color:#949ba4;";
    rightNum.textContent = String(channel.userLimit).padStart(2, "0");
    countPill.append(leftNum, sep, rightNum);
    countPill.title = `В голосовом канале: ${voiceParticipants.length} из ${channel.userLimit}`;
    reason.append(countPill);
  } else if (voiceParticipants.length > 0) {
    const countPill = document.createElement("span");
    countPill.style.cssText =
      "padding:0 5px;border-radius:8px;background:#2b2d31;color:#b5bac1;font-size:10px;font-weight:700;";
    countPill.textContent = `${voiceParticipants.length}`;
    countPill.title = `В голосовом канале: ${voiceParticipants.length}`;
    reason.append(countPill);
  } else {
    const txtSpan = document.createElement("span");
    txtSpan.textContent = "голос";
    reason.append(txtSpan);
  }

  if (liveVoice) {
    const quickWrap = document.createElement("span");
    quickWrap.className = "voice-channel-quick-actions";
    quickWrap.style.cssText = `display:${isConnectedVoiceHere ? "inline-flex" : "none"};align-items:center;gap:4px;margin-left:2px;`;

    const chatBtn = createVoiceQuickButton("Открыть чат", SVG_CHAT_BUBBLE, () => {
      options.onOpen?.({ id: channel.id, name: channel.name || channel.id });
    });
    const inviteBtn = createVoiceQuickButton("Создание приглашения", SVG_INVITE_USER, () => {
      options.onInviteVoice?.(channel, isRestricted);
    });

    quickWrap.append(chatBtn, inviteBtn);
    reason.append(quickWrap);
    button.addEventListener("mouseenter", () => {
      quickWrap.style.display = "inline-flex";
    });
    button.addEventListener("mouseleave", () => {
      if (!isConnectedVoiceHere) {
        quickWrap.style.display = "none";
      }
    });
  }

  return reason;
}

/**
 * Create the inline voice channel status setter row when connected to this channel.
 * @returns {HTMLDivElement}
 */
function buildVoiceStatusSubRow() {
  const statusSub = document.createElement("div");
  statusSub.className = "voice-channel-status-sub";
  statusSub.style.cssText =
    "margin:1px 8px 2px 32px;font-size:11px;color:#949ba4;display:flex;align-items:center;gap:4px;cursor:pointer;";
  statusSub.innerHTML = `<span>Выбрать статус канала</span><span style="font-size:10px;opacity:0.85;">✏️</span>`;
  statusSub.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const nextStatus = globalThis.prompt?.("Статус голосового канала:", "") ?? "";
    if (nextStatus.trim()) {
      statusSub.innerHTML = `<span style="color:#dbdee1;">${nextStatus.trim()}</span><span style="font-size:10px;opacity:0.85;">✏️</span>`;
    }
  });
  return statusSub;
}

/**
 * Create a single voice participant row (`li.voice-channel-user`).
 * @param {{
 *   vp: any,
 *   channel: TreeChannel,
 *   isConnectedVoiceHere: boolean,
 *   options: Record<string, any>,
 * }} params
 * @returns {HTMLLIElement}
 */
function buildVoiceParticipantRow({ vp, channel, isConnectedVoiceHere, options }) {
  const vRow = document.createElement("li");
  vRow.className = "voice-channel-user";
  vRow.setAttribute("data-user-id", vp.userId);
  vRow.setAttribute("data-channel-id", channel.id);
  vRow.style.display = "flex";
  vRow.style.alignItems = "center";
  vRow.style.gap = "8px";
  vRow.style.padding = "4px 8px";
  vRow.style.borderRadius = "6px";
  vRow.style.fontSize = "13px";
  vRow.style.color = "#dbdee1";
  vRow.style.cursor = "pointer";
  attachHoverColors(vRow, {
    enterBg: "rgba(78, 80, 88, 0.32)",
    enterColor: "#f2f3f5",
    leaveBg: "transparent",
    leaveColor: "#dbdee1",
  });

  const vAvatar = document.createElement("span");
  vAvatar.className = "avatar avatar--sm voice-user-avatar";
  vAvatar.setAttribute("data-voice-avatar-uid", vp.userId);
  vAvatar.style.width = "24px";
  vAvatar.style.height = "24px";
  vAvatar.style.fontSize = "11px";
  vAvatar.style.flex = "0 0 24px";
  vAvatar.style.borderRadius = "50%";
  vAvatar.style.transition = "box-shadow 90ms ease";
  if (vp.speaking) {
    vAvatar.style.boxShadow = "0 0 0 2px #23a55a, inset 0 0 0 1.5px #1e1f22";
  }
  vAvatar.textContent = Array.from(vp.displayName.trim()).slice(0, 1).join("").toUpperCase() || "U";
  if (vp.avatarUrl && typeof globalThis.Image === "function") {
    const img = new globalThis.Image();
    img.onload = () => {
      vAvatar.style.backgroundImage = `url("${vp.avatarUrl}")`;
      vAvatar.style.backgroundSize = "cover";
      vAvatar.style.backgroundPosition = "center";
      vAvatar.style.color = "transparent";
    };
    img.src = vp.avatarUrl;
  }

  const vNameWrap = document.createElement("span");
  vNameWrap.style.display = "inline-flex";
  vNameWrap.style.alignItems = "center";
  vNameWrap.style.gap = "5px";
  vNameWrap.style.flex = "1";
  vNameWrap.style.minWidth = "0";
  const vName = document.createElement("span");
  vName.style.overflow = "hidden";
  vName.style.textOverflow = "ellipsis";
  vName.style.whiteSpace = "nowrap";
  if (vp.speaking) {
    vName.style.color = "#f2f3f5";
    vName.style.fontWeight = "600";
  } else if (vp.roleColor) {
    vName.style.color = String(vp.roleColor);
    vName.setAttribute("data-role-color", String(vp.roleColor));
  }
  vName.textContent = vp.displayName;
  vNameWrap.append(vName);

  if (vp.clanTag) {
    const clanPill = document.createElement("span");
    clanPill.style.cssText =
      "display:inline-flex;align-items:center;gap:3px;padding:1px 5px;border-radius:4px;background:#2b2d31;border:1px solid #3f4147;color:#dbdee1;font-size:10px;font-weight:700;flex-shrink:0;";
    const cleanTag = String(vp.clanTag);
    clanPill.textContent = cleanTag.toUpperCase() === "BOJE" ? `💗 ${cleanTag}` : `🍃 ${cleanTag}`;
    vNameWrap.append(clanPill);
  }

  const vBadges = document.createElement("span");
  vBadges.style.display = "inline-flex";
  vBadges.style.alignItems = "center";
  vBadges.style.gap = "5px";
  vBadges.style.fontSize = "11px";
  vBadges.style.color = "#949ba4";
  vBadges.style.flexShrink = "0";

  if (vp.gameActivity && typeof vp.gameActivity === "object") {
    const gameBadge = document.createElement("span");
    gameBadge.className = "voice-user-game-badge";
    const isAc = vp.gameActivity.iconKind === "ac";
    gameBadge.style.cssText = `display:inline-flex;align-items:center;justify-content:center;width:16px;height:16px;border-radius:4px;background:${isAc ? "#da373c" : "linear-gradient(135deg,#f59e0b 0%,#b45309 100%)"};color:#fff;font-size:10px;font-weight:800;box-shadow:0 1px 2px rgba(0,0,0,0.35);`;
    gameBadge.title = `Сейчас играет: ${vp.gameActivity.title}`;
    gameBadge.innerHTML = isAc
      ? `<svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2L4 20l3.5 2L12 11l4.5 11L20 20 12 2z"/></svg>`
      : `<svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M21 6H3c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm-10 7H8v3H6v-3H3v-2h3V8h2v3h3v2zm4.5 2c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm4-3c-.83 0-1.5-.67-1.5-1.5S18.67 9 19.5 9s1.5.67 1.5 1.5-.67 1.5-1.5 1.5z"/></svg>`;
    vBadges.append(gameBadge);
    vRow.addEventListener("mouseenter", () => {
      options.onVoiceParticipantHoverActivity?.(vp, vRow, true);
    });
    vRow.addEventListener("mouseleave", () => {
      options.onVoiceParticipantHoverActivity?.(vp, vRow, false);
    });
  }

  if (vp.streaming === true) {
    const liveTag = document.createElement("span");
    liveTag.style.background = isConnectedVoiceHere ? "#da373c" : "#5c5e66";
    liveTag.style.color = "#fff";
    liveTag.style.fontSize = "10px";
    liveTag.style.fontWeight = "800";
    liveTag.style.padding = "1px 5px";
    liveTag.style.borderRadius = "4px";
    liveTag.style.letterSpacing = "0.02em";
    liveTag.style.cursor = isConnectedVoiceHere ? "pointer" : "not-allowed";
    liveTag.style.opacity = isConnectedVoiceHere ? "1" : "0.75";
    liveTag.title = isConnectedVoiceHere
      ? "Нажмите, чтобы смотреть трансляцию"
      : "Подключитесь к голосовому каналу, чтобы смотреть трансляцию";
    liveTag.textContent = "В ЭФИРЕ";
    liveTag.addEventListener("click", (ev) => {
      ev.stopPropagation();
      options.onWatchVoiceStream?.(vp, channel);
    });
    vBadges.append(liveTag);
  }

  if (vp.muted || vp.deafened) {
    const mIcon = document.createElement("span");
    const isServerMute = Boolean(vp.serverMute);
    mIcon.style.color = isServerMute ? "#f23f43" : "#949ba4";
    mIcon.style.display = "inline-flex";
    mIcon.style.alignItems = "center";
    mIcon.style.justifyContent = "center";
    mIcon.style.width = "16px";
    mIcon.style.height = "16px";
    mIcon.title = isServerMute ? "Микрофон отключён сервером" : "Микрофон отключён";
    mIcon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2a3 3 0 0 0-3 3v4.17l5.83 5.83A3 3 0 0 0 15 12V5a3 3 0 0 0-3-3Zm7 9a1 1 0 1 0-2 0c0 1.24-.45 2.38-1.2 3.26l1.42 1.42A6.96 6.96 0 0 0 19 11ZM4.27 3 3 4.27l6 6V12a3 3 0 0 0 4.13 2.77l1.55 1.55A5 5 0 0 1 7 12a1 1 0 1 0-2 0 7 7 0 0 0 6 6.92V21H9a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2h-2v-2.08c1.18-.17 2.27-.63 3.18-1.31L19.73 21 21 19.73 4.27 3Z"/></svg>`;
    vBadges.append(mIcon);
  }

  if (vp.deafened) {
    const dIcon = document.createElement("span");
    const isServerDeaf = Boolean(vp.serverDeaf);
    dIcon.style.color = isServerDeaf ? "#f23f43" : "#949ba4";
    dIcon.style.display = "inline-flex";
    dIcon.style.alignItems = "center";
    dIcon.style.justifyContent = "center";
    dIcon.style.width = "16px";
    dIcon.style.height = "16px";
    dIcon.title = isServerDeaf ? "Звук отключён сервером" : "Звук отключён";
    dIcon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M12 3a9 9 0 0 0-6.12 2.4l1.42 1.42A7 7 0 0 1 19 12v1h-2a2 2 0 0 0-2 2v1.17l3.88 3.88A2.99 2.99 0 0 0 21 17v-5a9 9 0 0 0-9-9ZM4.27 3 3 4.27l2.06 2.06A8.96 8.96 0 0 0 3 12v5a3 3 0 0 0 3 3h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H5v-1c0-1.61.55-3.09 1.46-4.27L19.73 21 21 19.73 4.27 3Z"/></svg>`;
    vBadges.append(dIcon);
  }

  vRow.append(vAvatar, vNameWrap, vBadges);
  vRow.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    ev.stopPropagation();
    options.onVoiceParticipantContextMenu?.(vp, channel, vRow, ev);
  });
  vRow.addEventListener("click", (ev) => {
    ev.stopPropagation();
    options.onVoiceParticipantContextMenu?.(vp, channel, vRow, ev);
  });
  return vRow;
}

/**
 * Create the "Invite to voice chat" row with countdown ring when alone in a voice channel.
 * @param {{
 *   channel: TreeChannel,
 *   guildId: string,
 *   remainMs: number,
 *   isRestricted: boolean,
 *   options: Record<string, any>,
 * }} params
 * @returns {HTMLLIElement}
 */
function buildAloneInviteRow({ channel, guildId, remainMs, isRestricted, options }) {
  const inviteRow = document.createElement("li");
  inviteRow.className = "voice-channel-invite-alone";
  inviteRow.setAttribute("data-alone-invite-cid", channel.id);
  inviteRow.setAttribute("data-alone-invite-gid", guildId);
  inviteRow.style.cssText =
    "display:flex;align-items:center;gap:8px;padding:4px 8px;border-radius:6px;font-size:12px;color:#949ba4;cursor:pointer;margin-top:2px;";
  attachHoverColors(inviteRow, {
    enterBg: "rgba(78, 80, 88, 0.32)",
    enterColor: "#f2f3f5",
    leaveBg: "transparent",
    leaveColor: "#949ba4",
  });

  const progressFrac = Math.min(1, Math.max(0, 1 - remainMs / 30_000));
  const dashOffset = (progressFrac * 62.83).toFixed(2);
  const ringWrap = document.createElement("span");
  ringWrap.style.cssText =
    "position:relative;width:24px;height:24px;flex:0 0 24px;display:inline-flex;align-items:center;justify-content:center;";
  ringWrap.innerHTML = `
    <svg width="24" height="24" viewBox="0 0 24 24" style="position:absolute;inset:0;transform:rotate(-90deg);">
      <circle cx="12" cy="12" r="10" fill="rgba(43,45,49,0.85)" stroke="#3f4147" stroke-width="2"/>
      <circle data-alone-ring-circle="${channel.id}" cx="12" cy="12" r="10" fill="none" stroke="#949ba4" stroke-width="2" stroke-dasharray="62.83" stroke-dashoffset="${dashOffset}" stroke-linecap="round"/>
    </svg>
    <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" style="position:relative;z-index:1;">
      <path d="M14 8C14 10.21 12.21 12 10 12C7.79 12 6 10.21 6 8C6 5.79 7.79 4 10 4C12.21 4 14 5.79 14 8ZM2 18V20H18V18C18 15.34 12.67 14 10 14C7.33 14 2 15.34 2 18ZM20 10V7H18V10H15V12H18V15H20V12H23V10H20Z"/>
    </svg>
  `;
  const inviteTxt = document.createElement("span");
  inviteTxt.style.cssText =
    "flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:500;";
  inviteTxt.textContent = "Пригласить в голосовой чат";
  const chev = document.createElement("span");
  chev.style.cssText = "font-size:15px;line-height:1;opacity:0.8;";
  chev.textContent = "›";
  inviteRow.append(ringWrap, inviteTxt, chev);
  inviteRow.addEventListener("click", (ev) => {
    ev.stopPropagation();
    options.onInviteVoice?.(channel, isRestricted);
  });
  return inviteRow;
}

/**
 * Render a single channel node (`li`) including icon, badges, keyboard/click handlers,
 * and nested voice participant rows.
 *
 * @param {{
 *   channel: TreeChannel,
 *   guildId: string,
 *   channels: TreeChannel[],
 *   channelKinds: Record<number, ChannelKind>,
 *   unknownChannelKind: ChannelKind,
 *   isLiveSession: () => boolean,
 *   options: Record<string, any>,
 *   onActivate: (channel: TreeChannel, kind: ChannelKind, button: HTMLElement) => void,
 *   onMoveFocus: (key: "ArrowDown" | "ArrowUp" | "Home" | "End", from: HTMLElement) => void,
 * }} params
 * @returns {HTMLLIElement}
 */
export function renderChannelNode({
  channel,
  guildId,
  channels,
  channelKinds,
  unknownChannelKind,
  isLiveSession,
  options,
  onActivate,
  onMoveFocus,
}) {
  const kind = channelKinds[channel.type] ?? unknownChannelKind;
  const liveSession = isLiveSession();
  const liveVoice = liveSession && kind.key === "voice";
  const liveForum = liveSession && kind.key === "forum";
  const liveCategory = liveSession && kind.key === "category";
  const isRestricted =
    liveSession && typeof options.isChannelRoleRestricted === "function"
      ? Boolean(options.isChannelRoleRestricted(guildId, channel, channels))
      : false;
  const voiceParticipants =
    liveVoice && typeof options.getVoiceParticipants === "function"
      ? options.getVoiceParticipants(guildId, channel.id)
      : [];
  const isConnectedVoiceHere =
    liveVoice && typeof options.isVoiceChannelConnected === "function"
      ? Boolean(options.isVoiceChannelConnected(guildId, channel.id))
      : false;

  const item = document.createElement("li");
  const button = document.createElement("button");
  button.className = "channel-item";
  button.type = "button";
  button.setAttribute("data-channel-id", channel.id);
  button.setAttribute("data-channel-type", String(channel.type));
  if (isRestricted) button.setAttribute("data-role-restricted", "true");
  if (channel.parentId !== "") button.setAttribute("data-parent-id", channel.parentId);
  button.setAttribute("data-openable", kind.openable ? "true" : "false");
  if (isConnectedVoiceHere) {
    button.style.background = "rgba(78, 80, 88, 0.36)";
    button.style.color = "#f2f3f5";
  }

  const hasActiveUsers = voiceParticipants.length > 0 || isConnectedVoiceHere;
  const icon = buildChannelIcon({
    channel,
    kind,
    liveSession,
    liveVoice,
    isRestricted,
    hasActiveUsers,
  });

  const name = document.createElement("span");
  name.className = "channel-name";
  name.textContent = channel.name || channel.id;
  if (liveVoice && hasActiveUsers) {
    name.style.color = "#f2f3f5";
    name.style.fontWeight = isConnectedVoiceHere ? "600" : "500";
  }
  button.append(icon, name);

  if (!kind.openable) {
    if (kind.key === "voice") {
      button.append(
        buildVoiceReason({
          channel,
          guildId,
          button,
          liveVoice,
          voiceParticipants,
          isConnectedVoiceHere,
          isRestricted,
          options,
        }),
      );
    } else {
      const reason = document.createElement("span");
      reason.className = "channel-reason";
      reason.textContent =
        kind.key === "category"
          ? "категория"
          : kind.key === "forum"
            ? "форум"
            : "недоступно";
      button.append(reason);
    }
    if (!liveVoice && !liveForum && !liveCategory) {
      button.setAttribute("aria-disabled", "true");
      button.setAttribute("title", kind.reason);
    }
  }

  button.addEventListener("click", () => onActivate(channel, kind, button));
  button.addEventListener("contextmenu", (event) => {
    if (typeof event.preventDefault === "function") event.preventDefault();
    options.onContextMenu?.(channel, button);
  });
  button.addEventListener("keydown", (event) => {
    const key = "key" in event ? String(event.key) : "";
    if (key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End") {
      event.preventDefault();
      onMoveFocus(key, button);
      return;
    }
    if (key === "Enter" || key === " ") {
      event.preventDefault();
      onActivate(channel, kind, button);
    }
  });
  item.append(button);

  if (isConnectedVoiceHere) {
    item.append(buildVoiceStatusSubRow());
  }

  if (liveVoice && voiceParticipants.length > 0) {
    const vList = document.createElement("ul");
    vList.className = "voice-channel-users";
    vList.setAttribute("role", "list");
    vList.style.listStyle = "none";
    vList.style.margin = "2px 0 6px 24px";
    vList.style.padding = "0";
    for (const vp of voiceParticipants) {
      vList.append(
        buildVoiceParticipantRow({
          vp,
          channel,
          isConnectedVoiceHere,
          options,
        }),
      );
    }
    if (isConnectedVoiceHere && voiceParticipants.length === 1) {
      const remainMs =
        typeof options.getAloneInviteRemainingMs === "function"
          ? options.getAloneInviteRemainingMs(guildId, channel.id)
          : 30_000;
      if (remainMs > 0) {
        vList.append(
          buildAloneInviteRow({
            channel,
            guildId,
            remainMs,
            isRestricted,
            options,
          }),
        );
      }
    }
    item.append(vList);
  }

  return item;
}
