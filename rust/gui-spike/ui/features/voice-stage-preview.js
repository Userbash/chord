// @ts-check
/**
 * TASK-821 (W6): Паритет превью голосовой комнаты (Voice Room Stage Banner)
 * и кнопки «Присоединиться к голосовому каналу» (frame 032).
 *
 * RE-оракул (/home/sanya/re-deep/ru-catalog.json):
 * - "96ANUN" / "9C444m": "Присоединиться к голосовому каналу"
 * - "BUyJIG": [[1,"a"]," сейчас в голосовом чате"]
 * - "dcyZf/": [[1,"a"]," и ",[1,"b"]," сейчас в голосовом чате"]
 * - "44/Obu": ["В голосовом чате сейчас ",[1,"a"],", ",[1,"b"]," и ещё ",[6,"n",{"few":[[7]," человека"],"other":[[7]," человек"]},0,"cardinal"]]
 * - "zSqdrS": ["Сейчас в голосовом чате никого нет"]
 *
 * Дизайн-токены (waves/kanban-board.json & waves/RW-42-VIDEO-AUDIT-AND-TASKS.md):
 *   background: #2b2d31, surface: #1e1f22, text_primary: #ffffff,
 *   accent: #eb459e, brand: #5865f2, border: #3f4147
 *
 * @module ui/features/voice-stage-preview
 */

/** Дизайн-токены Discord Dark для превью голосовой сцены (TASK-821). */
export const VOICE_STAGE_PREVIEW_TOKENS = Object.freeze({
  background: "#2b2d31",
  surface: "#1e1f22",
  textPrimary: "#ffffff",
  textSecondary: "#b5bac1",
  accent: "#eb459e",
  brand: "#5865f2",
  brandHover: "#4752c4",
  brandActive: "#3c45a5",
  border: "#3f4147",
  liveBadge: "#da373c",
  speakingRing: "#23a55a",
  joinPillBg: "#ffffff",
  joinPillText: "#060607",
});

/** Текстовые константы сцены голосового канала (из /home/sanya/re-deep/ru-catalog.json). */
export const VOICE_STAGE_PREVIEW_TEXT = Object.freeze({
  joinButton: "Присоединиться к голосовому каналу",
  joiningButton: "Подключение к голосовому каналу…",
  connectedButton: "Вы в голосовом канале",
  disconnectButton: "Отключиться",
  openChatButton: "Открыть чат",
  hideChatButton: "Скрыть чат",
  liveBadge: "В ЭФИРЕ",
  watchStreamButton: "Смотреть стрим",
  emptyParticipants: "Сейчас в голосовом чате никого нет",
  singleParticipantSuffix: "сейчас в голосовом чате",
});

/** Максимальное число карточек участников в превью до сворачивания в +N. */
export const MAX_STAGE_PREVIEW_AVATARS = 6;

/** Палитра карточек участников (первый цвет #eb459e соответствует кадру 032). */
export const STAGE_AVATAR_PALETTE = Object.freeze([
  "#eb459e",
  "#5865f2",
  "#23a55a",
  "#f0b232",
  "#ed4245",
  "#9b59b6",
]);

/**
 * @typedef {{
 *   userId?: string,
 *   id?: string,
 *   displayName?: string,
 *   username?: string,
 *   avatarUrl?: string | null,
 *   bannerColor?: string | null,
 *   accentColor?: string | null,
 *   roleColor?: string | null,
 *   muted?: boolean,
 *   selfMute?: boolean,
 *   serverMute?: boolean,
 *   deafened?: boolean,
 *   selfDeaf?: boolean,
 *   serverDeaf?: boolean,
 *   streaming?: boolean,
 *   video?: boolean,
 *   speaking?: boolean,
 * }} StagePreviewParticipant
 *
 * @typedef {{
 *   id: string,
 *   name?: string,
 *   type?: number,
 *   userLimit?: number,
 * }} StagePreviewChannel
 *
 * @typedef {{
 *   document?: Document,
 *   guildId?: string,
 *   guildName?: string,
 *   channel: StagePreviewChannel,
 *   participants?: ReadonlyArray<StagePreviewParticipant> | null,
 *   isConnected?: boolean,
 *   canConnect?: boolean,
 *   restrictionReason?: string | null,
 *   chatExpanded?: boolean,
 *   onJoinVoice?: (channel: StagePreviewChannel) => unknown,
 *   onDisconnectVoice?: (channel: StagePreviewChannel) => unknown,
 *   onWatchStream?: (participant: StagePreviewParticipant, channel: StagePreviewChannel) => unknown,
 *   onToggleChat?: (expanded: boolean, channel: StagePreviewChannel) => unknown,
 *   onPlaySound?: (kind: string) => void,
 * }} RenderVoiceStagePreviewOptions
 */

/**
 * Склонение числительного для ключа 44/Obu ("человека" / "человек").
 * @param {number} count
 * @returns {string}
 */
function pluralizePeopleRu(count) {
  const n = Math.abs(Math.trunc(Number(count) || 0));
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) {
    return "человека";
  }
  return "человек";
}

/**
 * Извлекает отображаемое имя участника голосового канала.
 * @param {StagePreviewParticipant | null | undefined} participant
 * @returns {string}
 */
export function resolveParticipantDisplayName(participant) {
  if (!participant || typeof participant !== "object") {
    return "Участник";
  }
  const raw =
    participant.displayName ??
    participant.username ??
    participant.userId ??
    participant.id ??
    "";
  const trimmed = String(raw).trim();
  return trimmed.length > 0 ? trimmed : "Участник";
}

/**
 * Форматирует строку подзаголовка участников голосового канала по оракулу ru-catalog.json:
 * - 0 участников (zSqdrS): "Сейчас в голосовом чате никого нет"
 * - 1 участник (BUyJIG): "{a} сейчас в голосовом чате" (кадр 032: "Rustery сейчас в голосовом чате")
 * - 2 участника (dcyZf/): "{a} и {b} сейчас в голосовом чате"
 * - 3+ участников (44/Obu): "В голосовом чате сейчас {a}, {b} и ещё {n} человек(а)"
 *
 * @param {ReadonlyArray<StagePreviewParticipant> | null | undefined} participants
 * @returns {string}
 */
export function formatVoiceStageParticipantsSummary(participants) {
  if (!Array.isArray(participants) || participants.length === 0) {
    return VOICE_STAGE_PREVIEW_TEXT.emptyParticipants;
  }
  const valid = participants.filter((p) => p && typeof p === "object");
  if (valid.length === 0) {
    return VOICE_STAGE_PREVIEW_TEXT.emptyParticipants;
  }
  const first = resolveParticipantDisplayName(valid[0]);
  if (valid.length === 1) {
    return `${first} ${VOICE_STAGE_PREVIEW_TEXT.singleParticipantSuffix}`;
  }
  const second = resolveParticipantDisplayName(valid[1]);
  if (valid.length === 2) {
    return `${first} и ${second} ${VOICE_STAGE_PREVIEW_TEXT.singleParticipantSuffix}`;
  }
  const extra = valid.length - 2;
  return `В голосовом чате сейчас ${first}, ${second} и ещё ${extra} ${pluralizePeopleRu(extra)}`;
}

/**
 * Вычисляет цвет фона карточки участника на превью сцены.
 * Первый слот по умолчанию использует #eb459e (розовая карточка Discord из кадра 032).
 *
 * @param {StagePreviewParticipant | null | undefined} participant
 * @param {number} [index=0]
 * @returns {string}
 */
export function resolveParticipantCardColor(participant, index = 0) {
  if (participant && typeof participant === "object") {
    const custom =
      participant.bannerColor ||
      participant.accentColor ||
      null;
    if (typeof custom === "string" && custom.trim().length > 0) {
      return custom.trim();
    }
  }
  const idx = Math.max(0, Math.trunc(Number(index) || 0));
  return STAGE_AVATAR_PALETTE[idx % STAGE_AVATAR_PALETTE.length] ?? VOICE_STAGE_PREVIEW_TOKENS.accent;
}

/**
 * Рендерит баннер предпросмотра голосовой комнаты (#voice-stage-preview-banner)
 * внутри контейнера чата (.chat-main) над списком сообщений (#chat-list),
 * не перекрывая историю сообщений.
 *
 * @param {HTMLElement} container
 * @param {RenderVoiceStagePreviewOptions} options
 * @returns {HTMLElement}
 */
export function renderVoiceStagePreview(container, options) {
  if (!container) {
    throw new Error("renderVoiceStagePreview: container обязателен");
  }
  const doc =
    options?.document ||
    container.ownerDocument ||
    /** @type {any} */ (globalThis).document;
  if (!doc || typeof doc.createElement !== "function") {
    throw new Error("renderVoiceStagePreview: document.createElement недоступен");
  }

  const channel = options?.channel || { id: "", name: "Голосовой канал", type: 2 };
  const channelId = String(channel.id || "");
  const channelName = String(channel.name || channel.id || "Голосовой канал");
  const guildId = String(options?.guildId || "");
  const participants = Array.isArray(options?.participants)
    ? options.participants.filter((p) => p && typeof p === "object")
    : [];
  const isConnected = Boolean(options?.isConnected);
  const canConnect = options?.canConnect !== false;
  const restrictionReason =
    typeof options?.restrictionReason === "string" && options.restrictionReason.trim().length > 0
      ? options.restrictionReason.trim()
      : null;

  let banner = /** @type {HTMLElement | null} */ (
    container.querySelector?.("#voice-stage-preview-banner") ??
      doc.getElementById?.("voice-stage-preview-banner") ??
      null
  );

  if (!banner) {
    banner = doc.createElement("section");
    banner.id = "voice-stage-preview-banner";
    banner.className = "voice-stage-preview";
  } else {
    banner.replaceChildren();
  }

  // Монтируем перед #chat-list (или в начало .chat-main), чтобы не перекрывать историю сообщений
  const chatListEl = /** @type {HTMLElement | null} */ (
    container.querySelector?.("#chat-list") ?? null
  );
  if (banner.parentNode !== container) {
    if (chatListEl && typeof container.insertBefore === "function") {
      container.insertBefore(banner, chatListEl);
    } else if (typeof container.prepend === "function") {
      container.prepend(banner);
    } else {
      container.appendChild(banner);
    }
  } else if (
    chatListEl &&
    Array.isArray(/** @type {any} */ (container).children) &&
    /** @type {any} */ (container).children.indexOf(banner) >
      /** @type {any} */ (container).children.indexOf(chatListEl) &&
    typeof container.insertBefore === "function"
  ) {
    container.insertBefore(banner, chatListEl);
  }

  banner.setAttribute("role", "region");
  banner.setAttribute("aria-label", `Голосовой канал: ${channelName}`);
  banner.setAttribute("data-voice-stage-preview", "true");
  banner.setAttribute("data-channel-id", channelId);
  banner.setAttribute("data-guild-id", guildId);
  banner.setAttribute("data-participant-count", String(participants.length));
  banner.setAttribute("data-connected", isConnected ? "true" : "false");
  banner.style.cssText =
    "position:relative;display:flex;flex-direction:column;align-items:center;justify-content:center;width:100%;min-height:280px;padding:24px;gap:16px;flex-shrink:0;box-sizing:border-box;background:radial-gradient(circle at 50% 42%, rgba(88, 101, 242, 0.28) 0%, rgba(30, 31, 68, 0.55) 48%, rgba(17, 18, 28, 0.96) 100%), #1e1f22;border-bottom:1px solid #3f4147;color:#ffffff;overflow:hidden;z-index:10;";

  // --- 1. Верхняя строка сцены (иконка динамика + название канала + кнопка чата) ---
  const topbar = doc.createElement("div");
  topbar.className = "voice-stage-preview-topbar";
  topbar.style.cssText =
    "width:100%;display:flex;align-items:center;justify-content:space-between;gap:12px;box-sizing:border-box;";

  const topbarLeft = doc.createElement("div");
  topbarLeft.className = "voice-stage-preview-topbar-left";
  topbarLeft.style.cssText =
    "display:inline-flex;align-items:center;gap:8px;min-width:0;color:#f2f3f5;font-size:15px;font-weight:600;";

  const speakerIcon = doc.createElement("span");
  speakerIcon.className = "voice-stage-preview-topbar-icon";
  speakerIcon.setAttribute("aria-hidden", "true");
  speakerIcon.textContent = "🔊";

  const topbarName = doc.createElement("span");
  topbarName.className = "voice-stage-preview-topbar-name";
  topbarName.textContent = channelName;

  topbarLeft.append(speakerIcon, topbarName);

  const topbarActions = doc.createElement("div");
  topbarActions.className = "voice-stage-preview-topbar-actions";

  let chatExpanded = options?.chatExpanded !== false;
  const chatToggleBtn = doc.createElement("button");
  chatToggleBtn.type = "button";
  chatToggleBtn.className = "voice-stage-chat-toggle-btn";
  chatToggleBtn.setAttribute("data-action", "toggle-voice-chat");
  chatToggleBtn.setAttribute("aria-label", VOICE_STAGE_PREVIEW_TEXT.openChatButton);
  chatToggleBtn.setAttribute("aria-expanded", chatExpanded ? "true" : "false");
  chatToggleBtn.textContent = `💬 ${VOICE_STAGE_PREVIEW_TEXT.openChatButton}`;
  chatToggleBtn.addEventListener("click", () => {
    chatExpanded = !chatExpanded;
    chatToggleBtn.setAttribute("aria-expanded", chatExpanded ? "true" : "false");
    if (chatListEl && chatListEl.style) {
      chatListEl.style.display = chatExpanded ? "" : "none";
    }
    if (typeof options?.onToggleChat === "function") {
      options.onToggleChat(chatExpanded, channel);
    }
  });

  topbarActions.appendChild(chatToggleBtn);
  topbar.append(topbarLeft, topbarActions);
  banner.appendChild(topbar);

  // --- 2. Центральная карточка предпросмотра (Frame 032) ---
  const card = doc.createElement("div");
  card.className = "voice-stage-preview-card";
  card.style.cssText =
    "display:flex;flex-direction:column;align-items:center;justify-content:center;gap:16px;max-width:680px;width:100%;text-align:center;margin:auto 0;box-sizing:border-box;";

  // --- 2a. Ряд карточек аватаров участников ---
  const avatarsRow = doc.createElement("div");
  avatarsRow.className = "voice-stage-preview-avatars";
  avatarsRow.setAttribute("data-voice-stage-avatars", "true");
  avatarsRow.style.cssText =
    "display:flex;align-items:center;justify-content:center;flex-wrap:wrap;gap:12px;";

  if (participants.length === 0) {
    const emptyTile = doc.createElement("div");
    emptyTile.className = "voice-stage-preview-empty-tile";
    emptyTile.setAttribute("data-voice-stage-empty", "true");
    emptyTile.style.cssText =
      "width:152px;height:88px;border-radius:10px;background:#2b2d31;border:1px solid #3f4147;display:flex;align-items:center;justify-content:center;color:#dbdee1;font-size:28px;";
    emptyTile.textContent = "🔊";
    avatarsRow.appendChild(emptyTile);
  } else {
    const visibleSlice = participants.slice(0, MAX_STAGE_PREVIEW_AVATARS);
    visibleSlice.forEach((participant, idx) => {
      const uid = String(participant.userId ?? participant.id ?? `user_${idx}`);
      const pName = resolveParticipantDisplayName(participant);
      const isSpeaking = Boolean(participant.speaking);
      const isStreaming = Boolean(participant.streaming);
      const isVideo = Boolean(participant.video);
      const isDeafened = Boolean(
        participant.deafened || participant.selfDeaf || participant.serverDeaf,
      );
      const isMuted = Boolean(
        participant.muted ||
          participant.selfMute ||
          participant.serverMute ||
          isDeafened,
      );
      const cardColor = resolveParticipantCardColor(participant, idx);

      const tile = doc.createElement("div");
      tile.className = "voice-stage-preview-tile";
      tile.setAttribute("data-participant-id", uid);
      tile.setAttribute("data-user-id", uid);
      tile.setAttribute("data-speaking", isSpeaking ? "true" : "false");
      tile.setAttribute("data-streaming", isStreaming ? "true" : "false");
      tile.style.cssText =
        `position:relative;width:152px;height:88px;border-radius:10px;background:${cardColor};border:2px solid ${isSpeaking ? "#23a55a" : "rgba(255,255,255,0.08)"};display:flex;flex-direction:column;align-items:center;justify-content:center;box-shadow:0 8px 24px rgba(0,0,0,0.38);overflow:hidden;box-sizing:border-box;`;

      const avatarEl = doc.createElement("div");
      avatarEl.className = "voice-stage-preview-avatar";
      avatarEl.setAttribute("data-voice-avatar-uid", uid);
      avatarEl.style.cssText =
        "width:44px;height:44px;border-radius:50%;background:rgba(0,0,0,0.22);color:#ffffff;display:flex;align-items:center;justify-content:center;font-size:18px;font-weight:700;background-size:cover;background-position:center;";
      avatarEl.textContent = Array.from(pName).slice(0, 1).join("").toUpperCase() || "U";
      if (participant.avatarUrl) {
        avatarEl.style.backgroundImage = `url("${participant.avatarUrl}")`;
        avatarEl.style.color = "transparent";
      }
      tile.appendChild(avatarEl);

      // Бейджи участника (В ЭФИРЕ, видео, микрофон, звук)
      const badgesWrap = doc.createElement("div");
      badgesWrap.className = "voice-stage-preview-badges";
      badgesWrap.style.cssText =
        "position:absolute;top:6px;right:6px;display:inline-flex;align-items:center;gap:4px;";

      if (isStreaming) {
        const liveBadge = doc.createElement("span");
        liveBadge.className = "voice-stage-badge voice-stage-badge--live";
        liveBadge.setAttribute("data-badge", "live");
        liveBadge.style.cssText =
          "display:inline-flex;align-items:center;justify-content:center;border-radius:4px;font-size:10px;font-weight:800;padding:2px 6px;background:#da373c;color:#ffffff;cursor:pointer;";
        liveBadge.textContent = VOICE_STAGE_PREVIEW_TEXT.liveBadge;
        liveBadge.addEventListener("click", (ev) => {
          if (ev && typeof ev.stopPropagation === "function") ev.stopPropagation();
          if (typeof options?.onWatchStream === "function") {
            options.onWatchStream(participant, channel);
          }
        });
        badgesWrap.appendChild(liveBadge);
      }

      if (isVideo) {
        const videoBadge = doc.createElement("span");
        videoBadge.className = "voice-stage-badge voice-stage-badge--video";
        videoBadge.setAttribute("data-badge", "video");
        videoBadge.title = "Камера включена";
        videoBadge.textContent = "📹";
        badgesWrap.appendChild(videoBadge);
      }

      if (isMuted) {
        const mutedBadge = doc.createElement("span");
        mutedBadge.className = "voice-stage-badge voice-stage-badge--muted";
        mutedBadge.setAttribute("data-badge", "muted");
        mutedBadge.title = "Микрофон выключен";
        mutedBadge.textContent = "🔇";
        badgesWrap.appendChild(mutedBadge);
      }

      if (isDeafened) {
        const deafBadge = doc.createElement("span");
        deafBadge.className = "voice-stage-badge voice-stage-badge--deafened";
        deafBadge.setAttribute("data-badge", "deafened");
        deafBadge.title = "Звук выключен";
        deafBadge.textContent = "🎧";
        badgesWrap.appendChild(deafBadge);
      }

      tile.appendChild(badgesWrap);

      const tileName = doc.createElement("span");
      tileName.className = "voice-stage-preview-tile-name";
      tileName.setAttribute("data-participant-name", "");
      tileName.textContent = pName;
      tile.appendChild(tileName);

      if (isStreaming && typeof options?.onWatchStream === "function") {
        tile.addEventListener("click", () => {
          options.onWatchStream?.(participant, channel);
        });
      }

      avatarsRow.appendChild(tile);
    });

    if (participants.length > MAX_STAGE_PREVIEW_AVATARS) {
      const overflowCount = participants.length - MAX_STAGE_PREVIEW_AVATARS;
      const overflowPill = doc.createElement("div");
      overflowPill.className = "voice-stage-preview-overflow";
      overflowPill.setAttribute("data-voice-stage-overflow", "true");
      overflowPill.textContent = `+${overflowCount}`;
      avatarsRow.appendChild(overflowPill);
    }
  }

  card.appendChild(avatarsRow);

  // --- 2b. Заголовок канала (крупный текст с эмодзи, кадр 032) ---
  const titleEl = doc.createElement("h2");
  titleEl.className = "voice-stage-preview-title";
  titleEl.setAttribute("data-voice-stage-title", "true");
  titleEl.style.cssText =
    "margin:0;font-size:24px;font-weight:700;line-height:1.25;color:#ffffff;";
  titleEl.textContent = channelName;
  card.appendChild(titleEl);

  // --- 2c. Подзаголовок со списком участников в голосовом чате ---
  const subtitleEl = doc.createElement("p");
  subtitleEl.className = "voice-stage-preview-subtitle";
  subtitleEl.setAttribute("data-voice-stage-subtitle", "true");
  subtitleEl.style.cssText =
    "margin:-6px 0 0;font-size:14px;font-weight:500;line-height:1.4;color:#b5bac1;";
  subtitleEl.textContent = formatVoiceStageParticipantsSummary(participants);
  card.appendChild(subtitleEl);

  // --- 2d. Уведомление об ограничении прав, если подключение запрещено ---
  const errorBox = doc.createElement("p");
  errorBox.className = "voice-stage-preview-error";
  errorBox.setAttribute("data-voice-stage-error", "true");
  if (restrictionReason) {
    errorBox.textContent = restrictionReason;
  } else {
    errorBox.setAttribute("hidden", "");
  }
  card.appendChild(errorBox);

  // --- 2e. Кнопка «Присоединиться к голосовому каналу» (TASK-821.3) ---
  const actionsRow = doc.createElement("div");
  actionsRow.className = "voice-stage-preview-actions";
  actionsRow.style.cssText =
    "display:flex;align-items:center;justify-content:center;flex-wrap:wrap;gap:10px;margin-top:2px;";

  const joinBtn = doc.createElement("button");
  joinBtn.type = "button";
  joinBtn.className = "voice-stage-join-btn";
  joinBtn.setAttribute("data-action", "join-voice-channel");
  joinBtn.setAttribute("data-pill-theme", "light");
  joinBtn.style.cssText =
    "display:inline-flex;align-items:center;justify-content:center;gap:8px;padding:10px 22px;border-radius:20px;border:1px solid rgba(255,255,255,0.14);background:#ffffff;color:#060607;font-size:14px;font-weight:600;cursor:pointer;";
  joinBtn.textContent = isConnected
    ? VOICE_STAGE_PREVIEW_TEXT.connectedButton
    : VOICE_STAGE_PREVIEW_TEXT.joinButton;

  if (!canConnect) {
    joinBtn.disabled = true;
    joinBtn.setAttribute("disabled", "");
    joinBtn.setAttribute("aria-disabled", "true");
    joinBtn.style.opacity = "0.55";
    joinBtn.style.cursor = "not-allowed";
  }

  let isConnecting = false;
  joinBtn.addEventListener("click", () => {
    if (!canConnect || joinBtn.disabled || isConnecting) return;
    isConnecting = true;
    joinBtn.classList.add("is-connecting");
    joinBtn.setAttribute("data-connecting", "true");

    if (typeof options?.onPlaySound === "function") {
      options.onPlaySound("join");
    }

    if (typeof options?.onJoinVoice === "function") {
      try {
        const res = options.onJoinVoice(channel);
        if (res && typeof (/** @type {any} */ (res).catch) === "function") {
          /** @type {Promise<unknown>} */ (res).catch((err) => {
            isConnecting = false;
            joinBtn.classList.remove("is-connecting");
            joinBtn.setAttribute("data-connecting", "false");
            errorBox.removeAttribute("hidden");
            errorBox.textContent = String(
              err?.message || err || "Не удалось подключиться к голосовому каналу",
            );
          });
        }
      } catch (err) {
        isConnecting = false;
        joinBtn.classList.remove("is-connecting");
        joinBtn.setAttribute("data-connecting", "false");
        errorBox.removeAttribute("hidden");
        errorBox.textContent = String(
          /** @type {any} */ (err)?.message || err || "Не удалось подключиться к голосовому каналу",
        );
      }
    }
  });

  actionsRow.appendChild(joinBtn);

  const activeStreamer = participants.find((p) => p && p.streaming === true);
  if (activeStreamer && typeof options?.onWatchStream === "function") {
    const watchBtn = doc.createElement("button");
    watchBtn.type = "button";
    watchBtn.className = "voice-stage-watch-btn";
    watchBtn.setAttribute("data-action", "watch-voice-stream");
    watchBtn.textContent = `🔴 ${VOICE_STAGE_PREVIEW_TEXT.watchStreamButton}`;
    watchBtn.addEventListener("click", () => {
      options.onWatchStream?.(activeStreamer, channel);
    });
    actionsRow.appendChild(watchBtn);
  }

  card.appendChild(actionsRow);
  banner.appendChild(card);

  return banner;
}

/**
 * Создаёт контроллер жизненного цикла превью голосовой сцены (TASK-821).
 *
 * @param {{
 *   document?: Document,
 *   container?: HTMLElement | null,
 *   onJoinVoice?: (channel: StagePreviewChannel, guildId: string, guildName: string) => unknown,
 *   onWatchStream?: (participant: StagePreviewParticipant, channel: StagePreviewChannel) => unknown,
 *   onToggleChat?: (expanded: boolean, channel: StagePreviewChannel) => unknown,
 *   onPlaySound?: (kind: string) => void,
 * }} [options]
 */
export function createVoiceStagePreviewController(options = {}) {
  const doc = options.document || /** @type {any} */ (globalThis).document;

  /** @type {StagePreviewChannel | null} */
  let currentChannel = null;
  let currentGuildId = "";
  let currentGuildName = "";
  /** @type {StagePreviewParticipant[]} */
  let currentParticipants = [];
  let currentIsConnected = false;
  let currentCanConnect = true;
  /** @type {string | null} */
  let currentRestrictionReason = null;
  let chatExpanded = true;

  function resolveContainer() {
    if (options.container) return options.container;
    if (!doc) return null;
    return (
      doc.querySelector?.("#screen-chat .chat-main") ??
      doc.getElementById?.("screen-chat") ??
      null
    );
  }

  function renderCurrent() {
    const container = resolveContainer();
    if (!container || !currentChannel) return null;
    return renderVoiceStagePreview(container, {
      document: doc,
      guildId: currentGuildId,
      guildName: currentGuildName,
      channel: currentChannel,
      participants: currentParticipants,
      isConnected: currentIsConnected,
      canConnect: currentCanConnect,
      restrictionReason: currentRestrictionReason,
      chatExpanded,
      onPlaySound: options.onPlaySound,
      onJoinVoice: (ch) =>
        options.onJoinVoice?.(ch, currentGuildId, currentGuildName),
      onWatchStream: options.onWatchStream,
      onToggleChat: (expanded, ch) => {
        chatExpanded = expanded;
        options.onToggleChat?.(expanded, ch);
      },
    });
  }

  return {
    /**
     * Открывает или обновляет превью голосового канала.
     * @param {{
     *   guildId?: string,
     *   guildName?: string,
     *   channel: StagePreviewChannel,
     *   participants?: ReadonlyArray<StagePreviewParticipant>,
     *   isConnected?: boolean,
     *   canConnect?: boolean,
     *   restrictionReason?: string | null,
     * }} params
     */
    open(params) {
      if (!params || !params.channel || !params.channel.id) return null;
      currentChannel = { ...params.channel };
      currentGuildId = String(params.guildId || "");
      currentGuildName = String(params.guildName || "");
      currentParticipants = Array.isArray(params.participants)
        ? [...params.participants]
        : [];
      currentIsConnected = Boolean(params.isConnected);
      currentCanConnect = params.canConnect !== false;
      currentRestrictionReason = params.restrictionReason ?? null;
      return renderCurrent();
    },

    /**
     * Обновляет список участников на открытом баннере в реальном времени (VOICE_STATE_UPDATE).
     * @param {ReadonlyArray<StagePreviewParticipant>} nextParticipants
     */
    updateParticipants(nextParticipants) {
      if (!currentChannel) return null;
      currentParticipants = Array.isArray(nextParticipants)
        ? [...nextParticipants]
        : [];
      return renderCurrent();
    },

    /**
     * Обновляет статус подключения и доступности кнопки входа.
     * @param {{ isConnected?: boolean, canConnect?: boolean, restrictionReason?: string | null }} state
     */
    updateConnectionState(state) {
      if (!currentChannel) return null;
      if (typeof state?.isConnected === "boolean") {
        currentIsConnected = state.isConnected;
      }
      if (typeof state?.canConnect === "boolean") {
        currentCanConnect = state.canConnect;
      }
      if (state && "restrictionReason" in state) {
        currentRestrictionReason = state.restrictionReason ?? null;
      }
      return renderCurrent();
    },

    /** Закрывает и удаляет баннер предпросмотра голосовой комнаты из DOM. */
    close() {
      currentChannel = null;
      currentParticipants = [];
      const container = resolveContainer();
      const existing =
        container?.querySelector?.("#voice-stage-preview-banner") ??
        doc?.getElementById?.("voice-stage-preview-banner") ??
        null;
      if (existing && typeof existing.remove === "function") {
        existing.remove();
      }
    },

    /** @returns {boolean} */
    isOpen() {
      if (!currentChannel) return false;
      const container = resolveContainer();
      const existing =
        container?.querySelector?.("#voice-stage-preview-banner") ??
        doc?.getElementById?.("voice-stage-preview-banner") ??
        null;
      return Boolean(existing);
    },

    /** @returns {StagePreviewChannel | null} */
    getCurrentChannel() {
      return currentChannel ? { ...currentChannel } : null;
    },

    /** @returns {string} */
    getCurrentGuildId() {
      return currentGuildId;
    },
  };
}

/**
 * Удаляет баннер предпросмотра голосовой комнаты (#voice-stage-preview-banner) из документа.
 * @param {Document | undefined | null} [doc]
 */
export function removeVoiceStagePreviewBanner(doc) {
  const d = doc || /** @type {any} */ (globalThis).document;
  if (!d) return;
  const existing =
    d.getElementById?.("voice-stage-preview-banner") ??
    d.querySelector?.("#voice-stage-preview-banner") ??
    null;
  if (existing && typeof existing.remove === "function") {
    existing.remove();
  }
}

/**
 * Рендерит баннер предпросмотра голосовой комнаты в `.chat-main` документа.
 * @param {{
 *   doc?: Document,
 *   container?: HTMLElement | null,
 *   channel: StagePreviewChannel & { guildId?: string },
 *   participants?: ReadonlyArray<StagePreviewParticipant> | null,
 *   isConnected?: boolean,
 *   isConnecting?: boolean,
 *   canConnect?: boolean,
 *   restrictionReason?: string | null,
 *   onJoinVoice?: (channel: StagePreviewChannel) => unknown,
 *   onLeaveVoice?: (channel: StagePreviewChannel) => unknown,
 *   onWatchStream?: (participant: StagePreviewParticipant, channel: StagePreviewChannel) => unknown,
 *   onInviteVoice?: (channel: StagePreviewChannel) => unknown,
 *   onToggleChat?: (expanded: boolean, channel: StagePreviewChannel) => unknown,
 * }} params
 * @returns {HTMLElement | null}
 */
export function renderVoiceStagePreviewBanner(params) {
  const doc = params?.doc || /** @type {any} */ (globalThis).document;
  if (!doc) return null;
  const container = /** @type {HTMLElement | null} */ (
    params?.container ??
      doc.querySelector?.("#screen-chat .chat-main") ??
      doc.querySelector?.(".chat-main") ??
      doc.getElementById?.("screen-chat") ??
      null
  );
  if (!container) return null;
  return renderVoiceStagePreview(container, {
    document: doc,
    guildId: String(params?.channel?.guildId || ""),
    channel: params.channel,
    participants: params.participants,
    isConnected: params.isConnected,
    canConnect: params.canConnect,
    restrictionReason: params.restrictionReason,
    onJoinVoice: params.onJoinVoice,
    onDisconnectVoice: params.onLeaveVoice,
    onWatchStream: params.onWatchStream,
  });
}
