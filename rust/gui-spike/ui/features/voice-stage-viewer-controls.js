// @ts-check

/**
 * Top bar and audio/zoom controls builder for the stream stage viewer.
 */

/**
 * Build the stream stage viewer top bar with quality badge, audio controls, zoom buttons,
 * fullscreen button, and stream action buttons.
 * @param {any} params
 */
export function buildStageViewerTopBar(params) {
  const {
    ctx,
    participant,
    channel,
    isSelfStream,
    streamKey,
    uId,
    getState,
    onZoomOut,
    onZoomReset,
    onZoomIn,
    onZoom200,
    onToggleFullscreen,
    onCloseStage,
  } = params;

  const topBar = document.createElement("div");
  topBar.style.cssText =
    "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:10px 16px;background:#111214;border-bottom:1px solid #2b2d31;flex-wrap:wrap;";
  const titleLeft = document.createElement("div");
  titleLeft.style.cssText = "display:flex;align-items:center;gap:10px;font-size:14px;font-weight:700;color:#f2f3f5;";

  const netBadge = document.createElement("span");
  netBadge.id = "stage-stream-quality-badge";
  netBadge.className = "stage-quality-badge";
  netBadge.style.cssText =
    "padding:2px 8px;border-radius:4px;background:rgba(35,165,90,0.16);border:1px solid rgba(35,165,90,0.28);color:#23a55a;font-size:11px;font-weight:700;font-variant-numeric:tabular-nums;cursor:pointer;";

  const updateNetBadge = () => {
    const { streamResLabel, targetStreamFps, measuredStreamFps } = getState();
    const connRef = /** @type {any} */ (ctx.getActiveVoiceConnection());
    const liveRtcStatus = ctx.getLiveRtcStatus();
    const effectiveRes = connRef?.streamResolution || connRef?.resolution || streamResLabel || "720p";
    const effectiveFps = Number(connRef?.streamFps || connRef?.fps || targetStreamFps || 60);
    const rtt = Number(liveRtcStatus?.lastPingMs ?? liveRtcStatus?.averagePingMs ?? 0);
    const gpuVendor = liveRtcStatus?.gpuVendor || "GPU";
    const codec = liveRtcStatus?.activeVideoCodec || "H264 HW";
    const gpuInfo = liveRtcStatus?.gpuModel
      ? `${gpuVendor}: ${liveRtcStatus.gpuModel} (${liveRtcStatus.gpuApi || "VA-API/Vulkan"})`
      : "Аппаратное ускорение GPU";
    const fpsDisplay = measuredStreamFps > 0 ? `${measuredStreamFps} FPS` : `${effectiveFps} FPS`;
    netBadge.title = `${gpuInfo} · Кодек: ${codec} · Декодирование потока: ${fpsDisplay}`;
    if (rtt > 450) {
      netBadge.style.background = "rgba(240,178,50,0.18)";
      netBadge.style.borderColor = "rgba(240,178,50,0.34)";
      netBadge.style.color = "#f0b232";
      netBadge.textContent = `${codec} · ${gpuVendor} · ${effectiveRes} · Поток: ${fpsDisplay} (${rtt} мс)`;
    } else if (rtt > 0) {
      netBadge.style.background = "rgba(35,165,90,0.16)";
      netBadge.style.borderColor = "rgba(35,165,90,0.28)";
      netBadge.style.color = "#23a55a";
      netBadge.textContent = `${codec} · ${gpuVendor} · ${effectiveRes} · Поток: ${fpsDisplay} (${rtt} мс)`;
    } else {
      netBadge.textContent = `${codec} · ${gpuVendor} · ${effectiveRes} · Поток: ${fpsDisplay}`;
    }
  };
  updateNetBadge();
  netBadge.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const conn = ctx.getActiveVoiceConnection();
    if (conn) {
      ctx.openStreamSettingsMenu(netBadge, conn, () => {
        updateNetBadge();
        ctx.renderLiveVoicePanel();
      });
    }
  });
  titleLeft.innerHTML = `<span style="background:#da373c;color:#fff;font-size:11px;font-weight:800;padding:2px 7px;border-radius:4px;">В ЭФИРЕ</span><span>${participant?.displayName || "Трансляция"} — ${channel?.name || "Голосовой канал"}</span>`;
  titleLeft.append(netBadge);

  const rightControls = document.createElement("div");
  rightControls.style.cssText = "display:flex;align-items:center;gap:6px;flex-wrap:wrap;";

  // Stream Audio Controls: Mute Toggle Button + Volume Slider (0%..200%)
  const liveRtc = ctx.getLiveRtcStatus();
  let streamMuted = Boolean(liveRtc?.streamAudioMuted ?? false);
  let streamVolumePct = Number(liveRtc?.streamAudioVolumePct ?? 100);
  if (!Number.isFinite(streamVolumePct)) streamVolumePct = 100;
  let prevUnmutedVolumePct = streamVolumePct > 0 ? streamVolumePct : 100;

  const audioGroup = document.createElement("div");
  audioGroup.style.cssText =
    "display:inline-flex;align-items:center;gap:6px;background:#1e1f22;border:1px solid #35373c;border-radius:6px;padding:3px 8px;";

  const muteStreamBtn = document.createElement("button");
  muteStreamBtn.type = "button";
  muteStreamBtn.style.cssText =
    "padding:2px 6px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:14px;cursor:pointer;line-height:1;";

  const streamVolSlider = document.createElement("input");
  streamVolSlider.type = "range";
  streamVolSlider.min = "0";
  streamVolSlider.max = "200";
  streamVolSlider.step = "1";
  streamVolSlider.value = String(streamMuted ? 0 : streamVolumePct);
  streamVolSlider.title = "Громкость звука трансляции (0% – 200%)";
  streamVolSlider.style.cssText = "width:84px;accent-color:#5865f2;cursor:pointer;";

  const streamVolLabel = document.createElement("span");
  streamVolLabel.style.cssText =
    "min-width:38px;text-align:right;font-size:11px;font-weight:700;color:#dbdee1;font-variant-numeric:tabular-nums;";

  const syncStreamAudioUi = (sendIpc = false) => {
    const effectiveVol = streamMuted ? 0 : streamVolumePct;
    muteStreamBtn.textContent = streamMuted || effectiveVol === 0 ? "🔇" : effectiveVol < 60 ? "🔉" : "🔊";
    muteStreamBtn.title = streamMuted ? "Включить звук трансляции" : "Отключить звук трансляции (Mute)";
    muteStreamBtn.style.color = streamMuted ? "#f23f43" : "#dbdee1";
    streamVolSlider.value = String(effectiveVol);
    streamVolLabel.textContent = streamMuted ? "ВЫКЛ" : `${streamVolumePct}%`;
    streamVolLabel.style.color = streamMuted ? "#f23f43" : "#dbdee1";
    const rtc = ctx.getLiveRtcStatus();
    if (rtc && typeof rtc === "object") {
      rtc.streamAudioMuted = streamMuted;
      rtc.streamAudioVolumePct = streamVolumePct;
    }
    if (sendIpc) {
      void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "stream_audio_volume",
        deviceId: `${streamMuted ? "1" : "0"}:${streamVolumePct}:${uId || ""}`,
        muted: streamMuted,
        volumePercent: streamVolumePct,
      }).catch(() => {});
    }
  };
  syncStreamAudioUi(false);

  muteStreamBtn.addEventListener("click", () => {
    if (!streamMuted) {
      if (streamVolumePct > 0) prevUnmutedVolumePct = streamVolumePct;
      streamMuted = true;
    } else {
      streamMuted = false;
      if (streamVolumePct === 0) streamVolumePct = prevUnmutedVolumePct || 100;
    }
    syncStreamAudioUi(true);
  });

  streamVolSlider.addEventListener("input", () => {
    const val = Math.max(0, Math.min(200, Number(streamVolSlider.value) || 0));
    streamVolumePct = val;
    if (val === 0) {
      streamMuted = true;
    } else {
      streamMuted = false;
      prevUnmutedVolumePct = val;
    }
    syncStreamAudioUi(true);
  });

  audioGroup.append(muteStreamBtn, streamVolSlider, streamVolLabel);
  rightControls.append(audioGroup);

  const zoomGroup = document.createElement("div");
  zoomGroup.style.cssText =
    "display:inline-flex;align-items:center;gap:2px;background:#1e1f22;border:1px solid #35373c;border-radius:6px;padding:2px;";
  /**
   * @param {string} label
   * @param {string} title
   */
  const mkZoomBtn = (label, title) => {
    const b = document.createElement("button");
    b.type = "button";
    b.textContent = label;
    b.title = title;
    b.style.cssText =
      "padding:4px 8px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:12px;font-weight:700;cursor:pointer;";
    return b;
  };
  const zoomOutBtn = mkZoomBtn("−", "Отдалить (колесо мыши вниз)");
  const zoomResetBtn = mkZoomBtn("100%", "Сбросить масштаб (100%)");
  const zoomInBtn = mkZoomBtn("+", "Приблизить (колесо мыши вверх)");
  const zoom200Btn = mkZoomBtn("200%", "Увеличить до 200% для чтения мелкого текста");
  zoomOutBtn.addEventListener("click", onZoomOut);
  zoomInBtn.addEventListener("click", onZoomIn);
  zoomResetBtn.addEventListener("click", onZoomReset);
  zoom200Btn.addEventListener("click", onZoom200);
  zoomGroup.append(zoomOutBtn, zoomResetBtn, zoomInBtn, zoom200Btn);
  rightControls.append(zoomGroup);

  const fullscreenBtn = document.createElement("button");
  fullscreenBtn.type = "button";
  fullscreenBtn.textContent = "⛶ На весь экран";
  fullscreenBtn.title = "Развернуть трансляцию на всё окно (F11 / двойной клик)";
  fullscreenBtn.style.cssText =
    "padding:6px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;";
  fullscreenBtn.addEventListener("click", onToggleFullscreen);
  rightControls.append(fullscreenBtn);

  if (!isSelfStream && streamKey) {
    const refreshStreamBtn = document.createElement("button");
    refreshStreamBtn.type = "button";
    refreshStreamBtn.textContent = "↻ Переподключить";
    refreshStreamBtn.style.cssText =
      "padding:6px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;";
    refreshStreamBtn.addEventListener("click", () => {
      void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "stream_watch",
        deviceId: streamKey,
      }).catch(() => {});
    });
    rightControls.append(refreshStreamBtn);
  }

  if (isSelfStream) {
    const changeSourceBtn = document.createElement("button");
    changeSourceBtn.type = "button";
    changeSourceBtn.id = "stage-change-source-btn";
    changeSourceBtn.textContent = "🖥 Сменить источник";
    changeSourceBtn.title = "Сменить окно или экран без перезапуска трансляции";
    changeSourceBtn.style.cssText =
      "padding:6px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;";
    changeSourceBtn.addEventListener("click", () => {
      ctx.openScreenSharePortalPicker("windows", ctx.switchLiveStreamSource);
    });

    const minimizeStageBtn = document.createElement("button");
    minimizeStageBtn.type = "button";
    minimizeStageBtn.id = "stage-minimize-viewer-btn";
    minimizeStageBtn.textContent = "Свернуть 🗕";
    minimizeStageBtn.title = "Свернуть окно просмотра в мини-плеер (стрим продолжится)";
    minimizeStageBtn.style.cssText =
      "padding:6px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;";
    minimizeStageBtn.addEventListener("click", () => {
      onCloseStage(false);
    });

    const stopStageBtn = document.createElement("button");
    stopStageBtn.type = "button";
    stopStageBtn.id = "stage-stop-stream-btn";
    stopStageBtn.textContent = "Прекратить стрим";
    stopStageBtn.title = "Остановить трансляцию экрана для всех участников";
    stopStageBtn.style.cssText =
      "padding:6px 12px;border-radius:6px;border:none;background:#da373c;color:#fff;font-size:12px;font-weight:700;cursor:pointer;";
    stopStageBtn.addEventListener("click", () => {
      onCloseStage(true);
    });

    const closeStageBtn = document.createElement("button");
    closeStageBtn.type = "button";
    closeStageBtn.id = "stage-close-viewer-btn";
    closeStageBtn.textContent = "✕";
    closeStageBtn.title = "Свернуть окно просмотра в мини-плеер (Escape)";
    closeStageBtn.style.cssText =
      "padding:6px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:700;cursor:pointer;";
    closeStageBtn.addEventListener("click", () => {
      onCloseStage(false);
    });

    rightControls.append(changeSourceBtn, minimizeStageBtn, stopStageBtn, closeStageBtn);
  } else {
    const closeStageBtn = document.createElement("button");
    closeStageBtn.type = "button";
    closeStageBtn.id = "stage-close-viewer-btn";
    closeStageBtn.textContent = "Прекратить просмотр ✕";
    closeStageBtn.style.cssText =
      "padding:6px 12px;border-radius:6px;border:none;background:#da373c;color:#fff;font-size:12px;font-weight:700;cursor:pointer;";
    closeStageBtn.addEventListener("click", () => {
      onCloseStage(false);
    });
    rightControls.append(closeStageBtn);
  }
  topBar.append(titleLeft, rightControls);

  return {
    topBar,
    updateNetBadge,
    zoomResetBtn,
    fullscreenBtn,
    muteStreamBtn,
  };
}
