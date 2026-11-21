// @ts-check
/**
 * rust/gui-spike/ui/features/voice-call-stage.js
 *
 * Каноническая сцена голосового звонка и стрима Discord (Cleanroom Parity D-03).
 * Полная визуальная и функциональная реализация по эталону Discord UI:
 * - Сетка крупных аватаров с анимацией зеленого кольца говорения (Скриншот 1).
 * - Полноэкранный/компактный видеопоток стрима с бейджами 'В ЭФИРЕ' и '720p 60fps' (Скриншот 2).
 * - Плавающая нижняя панель управления (Floating Dock) с микрофоном, камерой, стримом и красной трубкой.
 *
 * Архитектура: Тонкий реактивный мост на базе @preact/signals-core и Tauri IPC.
 */

import { signal, effect, batch } from "./reactive-store.js";
import { invoke } from "./auto-ipc-bridge.js";

// SVG-иконки органов управления по эталону Discord
const ICONS = {
  MIC_ON: `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"/><path d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"/></svg>`,
  MIC_OFF: `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M19 11h-1.7c0 .74-.16 1.43-.43 2.05l1.23 1.23c.56-.98.9-2.09.9-3.28zm-4.02.17L13.5 9.69V5c0-1.66-1.34-3-3-3-1.54 0-2.79 1.16-2.96 2.65L10.13 7.3 14.98 12.17zM4.27 3L3 4.27l6.01 6.01V11c0 1.66 1.33 3 2.99 3 .22 0 .44-.03.65-.08l1.66 1.66c-.71.33-1.5.52-2.31.52-2.76 0-5.3-2.24-5.3-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c1.33-.19 2.53-.74 3.53-1.54l3.19 3.19 1.27-1.27L4.27 3z"/></svg>`,
  CAMERA_OFF: `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M18 10.48V6c0-1.1-.9-2-2-2H6.83l2 2H16v7.17l2 2v-4.69zM3.27 2L2 3.27 4.73 6H4c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c.21 0 .39-.08.55-.18L19.73 23 21 21.73 3.27 2zM4 18V8h2.73l10 10H4z"/></svg>`,
  SCREEN_SHARE: `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M20 18c1.1 0 1.99-.9 1.99-2L22 6c0-1.11-.9-2-2-2H4c-1.11 0-2 .89-2 2v10c0 1.1.89 2 2 2H0v2h24v-2h-4zM4 6h16v10H4V6z"/></svg>`,
  ACTIVITIES: `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 14h-2v-2h2v2zm0-4h-2V7h2v5z"/></svg>`,
  SOUNDBOARD: `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>`,
  DISCONNECT: `<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08a.956.956 0 0 1-.29-.7c0-.28.11-.53.29-.71C3.34 8.78 7.46 7 12 7s8.66 1.78 11.71 4.67c.18.18.29.43.29.71 0 .28-.11.53-.29.71l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28a11.27 11.27 0 0 0-2.67-1.85.996.996 0 0 1-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z"/></svg>`,
  CHEVRON: `<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41z"/></svg>`,
  LIVE_ICON: `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="12" r="8"/></svg>`,
};

/**
 * Создает экземпляр контроллера сцены звонка с привязкой к DOM и реактивному состоянию.
 * @param {HTMLElement | null} mountPoint Родительский элемент (например, в шапке чата над сообщениями)
 * @param {any} [options]
 */
export function createVoiceCallStage(mountPoint, options = {}) {
  // Реактивные сигналы состояния звонка
  const isConnected = signal(options.initialConnected ?? false);
  const channelName = signal(options.initialChannelName ?? "Rustery");
  const isStreaming = signal(options.initialStreaming ?? false);
  const streamQuality = signal(options.initialStreamQuality ?? "720p 60fps");
  const isSelfMuted = signal(Boolean(options.initialMuted));
  const isCameraOn = signal(Boolean(options.initialCameraOn));

  const normalizeParticipant = (/** @type {any} */ p) => ({
    id: String(p?.id ?? p?.userId ?? ""),
    name: String(p?.name ?? p?.displayName ?? "Участник"),
    avatarUrl: p?.avatarUrl ?? null,
    avatarBg: p?.avatarBg ?? null,
    isDiscordLogo: Boolean(p?.isDiscordLogo),
    speaking: Boolean(p?.speaking),
    muted: Boolean(p?.muted),
    deafened: Boolean(p?.deafened),
    ringing: Boolean(p?.ringing),
    raw: p?.raw ?? p,
  });

  // Список участников звонка (минимум 2 по скриншоту по умолчанию)
  const initialList = Array.isArray(options.initialParticipants)
    ? options.initialParticipants.map(normalizeParticipant)
    : [
        {
          id: "self",
          name: "Я",
          avatarUrl: null,
          avatarBg: "#eb459e",
          isDiscordLogo: true,
          speaking: false,
          muted: false,
          deafened: false,
          ringing: false,
          raw: null,
        },
        {
          id: "partner",
          name: channelName.value,
          avatarUrl: "https://cdn.discordapp.com/embed/avatars/0.png",
          avatarBg: null,
          isDiscordLogo: false,
          speaking: false,
          muted: false,
          deafened: false,
          ringing: false,
          raw: null,
        },
      ];
  const participants = signal(initialList);

  // Корневой DOM-узел сцены
  const stageRoot = document.createElement("section");
  if (options.id) stageRoot.id = String(options.id);
  stageRoot.className = "voice-call-stage-root";
  stageRoot.setAttribute("role", "region");
  stageRoot.setAttribute("aria-label", "Голосовая сцена звонка");
  stageRoot.dataset.mode = isStreaming.value ? "stream" : "compact";

  // 1. Верхняя информационная полоса
  const header = document.createElement("div");
  header.className = "voice-call-stage-header";

  const headerInfo = document.createElement("div");
  headerInfo.className = "voice-call-header-info";

  const statusSpan = document.createElement("span");
  statusSpan.className = "voice-call-header-status";
  const iconSpan = document.createElement("span");
  iconSpan.textContent = "📞 ";
  iconSpan.style.color = "var(--status-positive, #23a55a)";
  const textSpan = document.createElement("span");
  textSpan.textContent = `${channelName.value} · Участвует в звонке`;
  statusSpan.append(iconSpan, textSpan);

  const timerSpan = document.createElement("span");
  timerSpan.className = "voice-call-header-timer";
  timerSpan.setAttribute("data-dm-stage-timer", "true");
  timerSpan.style.display = "none";

  const badgesWrap = document.createElement("div");
  badgesWrap.style.cssText = "display:flex;align-items:center;gap:8px;";

  const qualityBadge = document.createElement("span");
  qualityBadge.className = "voice-call-header-badge-quality";
  qualityBadge.textContent = streamQuality.value;

  const liveBadge = document.createElement("span");
  liveBadge.className = "voice-call-header-badge-live";
  liveBadge.textContent = "В ЭФИРЕ";

  badgesWrap.append(qualityBadge, liveBadge);
  headerInfo.append(statusSpan, timerSpan);
  header.append(headerInfo, badgesWrap);

  // 2. Центральная сетка: аватары или видеопоток
  const centerGrid = document.createElement("div");
  centerGrid.className = "voice-call-stage-grid";

  // Видео-контейнер для стрима
  const streamContainer = document.createElement("div");
  streamContainer.className = "voice-call-stream-container";
  streamContainer.style.display = isStreaming.value ? "flex" : "none";
  streamContainer.addEventListener("click", () => {
    if (typeof options.onStreamClick === "function") {
      options.onStreamClick();
    }
  });

  const streamCanvas = document.createElement("canvas");
  streamCanvas.className = "voice-call-stream-canvas";
  streamCanvas.width = 1280;
  streamCanvas.height = 720;

  const streamImg = document.createElement("img");
  streamImg.className = "voice-call-stream-img";
  streamImg.setAttribute("data-dm-stage-self-stream-thumb", "true");
  streamImg.alt = "Предпросмотр демонстрации экрана";
  streamImg.style.display = "none";

  const streamOverlay = document.createElement("div");
  streamOverlay.className = "voice-call-stream-overlay-tag";
  streamOverlay.textContent = `📺 ${channelName.value}`;

  streamContainer.append(streamCanvas, streamImg, streamOverlay);

  // 3. Плавающий док управления звонком
  const dock = document.createElement("div");
  dock.className = "voice-call-floating-dock";

  // Группа микрофона с шевроном
  const micGroup = document.createElement("div");
  micGroup.className = "voice-call-btn-group";

  const micBtn = document.createElement("button");
  micBtn.className = "voice-call-dock-btn";
  micBtn.type = "button";
  micBtn.setAttribute("data-action", "mute");
  micBtn.setAttribute("title", "Выключить микрофон");
  micBtn.innerHTML = ICONS.MIC_ON;
  const applyMicVisualState = (/** @type {boolean} */ muted) => {
    micBtn.innerHTML = muted ? ICONS.MIC_OFF : ICONS.MIC_ON;
    micBtn.setAttribute("data-active", String(!muted));
    micBtn.setAttribute("data-muted", String(muted));
    micBtn.setAttribute("title", muted ? "Включить микрофон" : "Выключить микрофон");
  };

  micBtn.addEventListener("click", () => {
    if (typeof options.onMute === "function") {
      options.onMute();
      return;
    }
    isSelfMuted.value = !isSelfMuted.value;
    applyMicVisualState(isSelfMuted.value);
    invoke("voice_mute", { selfMute: isSelfMuted.value });
  });

  const micChevron = document.createElement("button");
  micChevron.className = "voice-call-dock-chevron";
  micChevron.type = "button";
  micChevron.setAttribute("data-action", "select-mic");
  micChevron.setAttribute("title", "Выбор микрофона");
  micChevron.innerHTML = ICONS.CHEVRON;
  micChevron.addEventListener("click", () => {
    if (typeof options.onSelectMic === "function") {
      options.onSelectMic(micChevron);
    }
  });

  micGroup.append(micBtn, micChevron);

  // Кнопка камеры
  const camBtn = document.createElement("button");
  camBtn.className = "voice-call-dock-btn";
  camBtn.type = "button";
  camBtn.setAttribute("data-action", "camera");
  camBtn.setAttribute("title", "Включить камеру");
  camBtn.innerHTML = ICONS.CAMERA_OFF;
  camBtn.addEventListener("click", () => {
    if (typeof options.onCamera === "function") {
      options.onCamera();
      return;
    }
    isCameraOn.value = !isCameraOn.value;
    camBtn.setAttribute("data-active", String(isCameraOn.value));
  });

  // Кнопка демонстрации экрана
  const streamBtn = document.createElement("button");
  streamBtn.className = "voice-call-dock-btn";
  streamBtn.type = "button";
  streamBtn.setAttribute("data-action", "screen-share");
  streamBtn.setAttribute("title", "Поделиться экраном");
  streamBtn.innerHTML = ICONS.SCREEN_SHARE;
  streamBtn.addEventListener("click", async () => {
    if (typeof options.onScreenShare === "function") {
      options.onScreenShare();
      return;
    }
    isStreaming.value = !isStreaming.value;
    if (isStreaming.value) {
      await invoke("voice_stream_start", { sourceId: "screen_primary" });
    } else {
      await invoke("voice_stream_stop", {});
    }
  });

  // Кнопка активности
  const actBtn = document.createElement("button");
  actBtn.className = "voice-call-dock-btn";
  actBtn.type = "button";
  actBtn.setAttribute("data-action", "activities");
  actBtn.setAttribute("title", "Активности");
  actBtn.innerHTML = ICONS.ACTIVITIES;
  actBtn.addEventListener("click", () => {
    if (typeof options.onActivities === "function") {
      options.onActivities(actBtn);
    }
  });

  // Кнопка саундборда
  const soundBtn = document.createElement("button");
  soundBtn.className = "voice-call-dock-btn";
  soundBtn.type = "button";
  soundBtn.setAttribute("data-action", "soundboard");
  soundBtn.setAttribute("title", "Панель звуков");
  soundBtn.innerHTML = ICONS.SOUNDBOARD;
  soundBtn.addEventListener("click", () => {
    if (typeof options.onSoundboard === "function") {
      options.onSoundboard(soundBtn);
    }
  });

  // Красная кнопка отключения
  const discBtn = document.createElement("button");
  discBtn.className = "voice-call-dock-btn";
  discBtn.setAttribute("data-danger", "true");
  discBtn.setAttribute("data-action", "disconnect");
  discBtn.type = "button";
  discBtn.setAttribute("title", "Завершить звонок");
  discBtn.innerHTML = ICONS.DISCONNECT;
  discBtn.addEventListener("click", async () => {
    if (typeof options.onDisconnect === "function") {
      options.onDisconnect();
      return;
    }
    await invoke("voice_disconnect", {});
    isConnected.value = false;
    destroy();
  });

  dock.append(micGroup, camBtn, streamBtn, actBtn, soundBtn, discBtn);

  // Сборка всей сцены
  stageRoot.append(header, centerGrid, streamContainer, dock);

  // Реактивная отрисовка аватаров и режима
  const disposeEffect = effect(() => {
    const mode = isStreaming.value ? "stream" : "compact";
    stageRoot.setAttribute("data-mode", mode);
    stageRoot.dataset.mode = mode;
    badgesWrap.style.display = isStreaming.value ? "flex" : "none";
    streamContainer.style.display = isStreaming.value ? "flex" : "none";
    centerGrid.style.display = isStreaming.value ? "none" : "flex";

    if (!isStreaming.value) {
      centerGrid.replaceChildren();
      for (const p of participants.value) {
        const card = document.createElement("div");
        card.className = "voice-call-participant-card";
        card.setAttribute("data-speaking", String(p.speaking));
        card.setAttribute("data-participant-id", p.id);
        card.setAttribute("data-ringing", String(Boolean(p.ringing)));
        card.dataset.speaking = String(p.speaking);
        card.dataset.ringing = String(Boolean(p.ringing));

        const avWrap = document.createElement("div");
        avWrap.className = "voice-call-avatar-wrapper";
        avWrap.setAttribute("data-voice-avatar-uid", p.id);
        if (p.speaking) avWrap.setAttribute("data-speaking", "true");
        if (p.avatarBg) avWrap.style.background = p.avatarBg;

        const appendPlaceholder = () => {
          const initial = document.createElement("span");
          initial.className = "voice-call-avatar-placeholder";
          initial.textContent = (p.name || "U").charAt(0).toUpperCase();
          avWrap.append(initial);
        };

        if (p.isDiscordLogo) {
          avWrap.innerHTML = `<svg width="48" height="48" viewBox="0 0 24 24" fill="#ffffff"><path d="M19.27 5.33C17.94 4.71 16.5 4.26 15 4a.09.09 0 0 0-.07.03c-.18.33-.39.76-.53 1.09a16.09 16.09 0 0 0-4.8 0c-.14-.34-.35-.76-.54-1.09-.01-.02-.04-.03-.07-.03-1.5.26-2.93.71-4.27 1.33-.01 0-.02.01-.03.02-2.72 4.07-3.47 8.03-3.1 11.95 0 .02.01.04.03.05 1.8 1.32 3.53 2.12 5.24 2.65.03.01.06 0 .07-.02.4-.55.76-1.13 1.07-1.74.02-.04 0-.08-.04-.09-.57-.22-1.11-.48-1.64-.78-.04-.02-.04-.08-.01-.11.11-.08.22-.17.33-.25.02-.02.05-.02.07-.01 3.44 1.57 7.15 1.57 10.55 0 .02-.01.05-.01.07.01.11.09.22.17.33.26.04.03.04.09-.01.11-.52.31-1.07.56-1.64.78-.04.01-.05.06-.04.09.32.61.68 1.19 1.07 1.74.03.01.06.02.09.01 1.72-.53 3.45-1.33 5.25-2.65.02-.01.03-.03.03-.05.44-4.53-.76-8.46-3.1-11.95-.01-.01-.02-.02-.04-.02zM8.52 14.91c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12 0 1.17-.84 2.12-1.89 2.12zm6.97 0c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12 0 1.17-.83 2.12-1.89 2.12z"/></svg>`;
        } else if (p.avatarUrl) {
          const img = document.createElement("img");
          img.className = "voice-call-avatar-img";
          img.src = p.avatarUrl;
          img.alt = p.name;
          img.onerror = () => {
            img.remove();
            appendPlaceholder();
          };
          avWrap.append(img);
        } else {
          appendPlaceholder();
        }

        const nameLabel = document.createElement("span");
        nameLabel.className = "voice-call-participant-name";
        nameLabel.textContent = p.name;
        if (p.muted || p.deafened) {
          const mIcon = document.createElement("span");
          mIcon.className = "voice-call-participant-mute-icon";
          mIcon.textContent = p.deafened ? "🎧̸" : "🎙̸";
          nameLabel.append(mIcon);
        }

        card.append(avWrap, nameLabel);
        if (p.ringing) {
          const ringBadge = document.createElement("span");
          ringBadge.className = "voice-call-participant-ringing";
          ringBadge.textContent = "Ожидание ответа…";
          card.append(ringBadge);
        }
        if (typeof options.onParticipantContextMenu === "function") {
          card.addEventListener("contextmenu", (ev) => {
            ev.preventDefault();
            options.onParticipantContextMenu(p, card, ev);
          });
        }
        centerGrid.append(card);
      }
    }
  });

  // Монтирование в контейнер
  if (mountPoint) {
    mountPoint.prepend(stageRoot);
  }

  function mountTo(/** @type {HTMLElement | null} */ targetMountPoint) {
    if (targetMountPoint && stageRoot.parentNode !== targetMountPoint) {
      targetMountPoint.prepend(stageRoot);
    }
  }

  function setChannel(/** @type {string} */ name) {
    channelName.value = name;
    textSpan.textContent = `${name} · Участвует в звонке`;
  }

  function setTimer(/** @type {string} */ timerTxt) {
    if (!timerTxt) {
      timerSpan.style.display = "none";
      timerSpan.textContent = "";
      return;
    }
    timerSpan.style.display = "inline-flex";
    if (timerSpan.textContent !== timerTxt) {
      timerSpan.textContent = timerTxt;
    }
  }

  function setParticipants(/** @type {any[]} */ nextList) {
    if (!Array.isArray(nextList)) return;
    participants.value = nextList.map(normalizeParticipant);
  }

  function setStreaming(/** @type {boolean} */ enabled, quality = "720p 60fps") {
    batch(() => {
      isStreaming.value = Boolean(enabled);
      streamQuality.value = quality;
      qualityBadge.textContent = quality;
    });
  }

  function setStreamPreview(/** @type {{ dataUrl?: string, streamerName?: string, quality?: string }} */ opts = {}) {
    const dataUrl = String(opts.dataUrl || "");
    if (dataUrl) {
      if (streamImg.src !== dataUrl) streamImg.src = dataUrl;
      streamImg.style.display = "block";
      streamCanvas.style.display = "none";
    } else {
      streamImg.style.display = "none";
      streamCanvas.style.display = "block";
    }
    if (opts.streamerName) {
      streamOverlay.textContent = `📺 ${opts.streamerName}`;
    }
    if (opts.quality) {
      streamQuality.value = opts.quality;
      qualityBadge.textContent = opts.quality;
    }
  }

  function setControlsState(/** @type {{ muted?: boolean, cameraOn?: boolean, streaming?: boolean }} */ state = {}) {
    if (typeof state.muted === "boolean") {
      isSelfMuted.value = state.muted;
      applyMicVisualState(state.muted);
    }
    if (typeof state.cameraOn === "boolean") {
      isCameraOn.value = state.cameraOn;
      camBtn.setAttribute("data-active", String(state.cameraOn));
    }
    if (typeof state.streaming === "boolean") {
      streamBtn.setAttribute("data-active", String(state.streaming));
    }
  }

  function setSpeaking(/** @type {string} */ participantId, /** @type {boolean} */ isSpeaking) {
    participants.value = participants.value.map((/** @type {any} */ p) =>
      p.id === participantId ? { ...p, speaking: isSpeaking } : p
    );
  }

  function destroy() {
    if (typeof disposeEffect === "function") disposeEffect();
    stageRoot.remove();
  }

  return {
    element: stageRoot,
    mountTo,
    setChannel,
    setTimer,
    setParticipants,
    setStreaming,
    setStreamPreview,
    setControlsState,
    setSpeaking,
    destroy,
  };
}
