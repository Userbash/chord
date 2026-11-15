// @ts-check

/**
 * Stream controller: screen preview capture, source switching, canvas preview drawing,
 * floating PiP preview, and the Screen Share modal.
 */

/**
 * Create the stream controller bound to the shared voice call context.
 * @param {any} ctx
 */
export function createVoiceStreamController(ctx) {
  const { cleanDiscordStr, HOME_GUILD_ID } = ctx;
  function cancelPipAnimTimer() {
    const activePipAnimTimer = ctx.getActivePipAnimTimer();
    if (activePipAnimTimer !== null) {
      if (typeof cancelAnimationFrame === "function") {
        cancelAnimationFrame(/** @type {number} */ (activePipAnimTimer));
      }
      clearInterval(/** @type {any} */ (activePipAnimTimer));
      clearTimeout(/** @type {any} */ (activePipAnimTimer));
      ctx.setActivePipAnimTimer(null);
    }
  }

  function requestLiveScreenPreviewFrame() {
    if (!ctx.isLiveSession() || ctx.getLiveScreenCaptureInFlight()) return;
    const now = Date.now();
    if (now - ctx.getLastLiveScreenCaptureAtMs() < 850) return;
    ctx.setLiveScreenCaptureInFlight(true);
    ctx.setLastLiveScreenCaptureAtMs(now);
    void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "screen_preview" })
      .then((/** @type {any} */ res) => {
        const seq = Number(res?.seq || 0);
        const dataUrl = res?.previewDataUrl;
        if (typeof dataUrl === "string" && dataUrl.startsWith("data:image/")) {
          if (seq > 0 && seq === ctx.getLastLiveScreenSeq() && ctx.getCachedLiveScreenImg()) return;
          ctx.setLastLiveScreenSeq(seq);
          ctx.setCachedLiveScreenDataUrl(dataUrl);
          const img = new globalThis.Image();
          img.onload = () => {
            ctx.setCachedLiveScreenImg(img);
          };
          img.src = dataUrl;
        }
      })
      .catch(() => {})
      .finally(() => {
        ctx.setLiveScreenCaptureInFlight(false);
      });
  }

  /**
   * Dynamically switch the active stream capture source (window or screen)
   * without tearing down the RTC connection or rejoining the voice channel.
   * @param {{ id: string, label?: string, tab?: "screens" | "windows" }} picked
   */
  function switchLiveStreamSource(picked) {
    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    if (!activeVoiceConnection || !picked?.id) return;
    const conn = /** @type {any} */ (activeVoiceConnection);
    conn.streamSourceId = picked.id;
    conn.streamSourceName = picked.label || "Окно или экран";
    conn.streamSourceLabel = picked.label || "Окно или экран";
    if (picked.tab) {
      conn.streamPortalKind = picked.tab;
    }
    ctx.setLastLiveScreenCaptureAtMs(0);
    void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
      kind: "set_capture_source",
      deviceId: picked.id,
    }).catch(() => {});
    requestLiveScreenPreviewFrame();
    ctx.playVoiceUiSound("stream_on");
    ctx.showVoiceToastNotice(`Источник трансляции изменён: ${picked.label || picked.id}`);
    ctx.renderLiveVoicePanel();
    syncFloatingStreamPipPreview();
  }

  /**
   * Draw a live capture preview frame for a selected screen or window.
   * @param {HTMLCanvasElement} canvas
   * @param {string} sourceId
   * @param {number} tick
   * @param {boolean} [showWallpaper]
   */
  function drawLiveCaptureCanvasFrame(canvas, sourceId, tick, showWallpaper = true) {
    const c2d = canvas.getContext("2d");
    if (!c2d) return;
    c2d.imageSmoothingEnabled = true;
    try {
      /** @type {any} */ (c2d).imageSmoothingQuality = "high";
    } catch {}
    const w = canvas.width;
    const h = canvas.height;
    c2d.clearRect(0, 0, w, h);

    if (ctx.isLiveSession() && sourceId !== "screen_virtual") {
      requestLiveScreenPreviewFrame();
      const cachedLiveScreenImg = ctx.getCachedLiveScreenImg();
      if (cachedLiveScreenImg && cachedLiveScreenImg.complete && cachedLiveScreenImg.naturalWidth > 0) {
        const iw = cachedLiveScreenImg.naturalWidth;
        const ih = cachedLiveScreenImg.naturalHeight;
        const scale = Math.min(w / iw, h / ih);
        const dw = Math.round(iw * scale);
        const dh = Math.round(ih * scale);
        const dx = Math.round((w - dw) / 2);
        const dy = Math.round((h - dh) / 2);
        c2d.fillStyle = "#090a0c";
        c2d.fillRect(0, 0, w, h);
        c2d.drawImage(cachedLiveScreenImg, dx, dy, dw, dh);
        if (sourceId === "screen_region") {
          c2d.strokeStyle = "#00a8fc";
          c2d.lineWidth = 2;
          c2d.setLineDash([4, 3]);
          c2d.strokeRect(18, 12, w - 36, h - 24);
          c2d.setLineDash([]);
        }
        return;
      }
    }

    if (sourceId === "screen_virtual") {
      c2d.fillStyle = "#090a0b";
      c2d.fillRect(0, 0, w, h);
      return;
    }

    if (showWallpaper) {
      const grad = c2d.createLinearGradient(0, 0, w, h);
      grad.addColorStop(0, "#1d1b38");
      grad.addColorStop(0.55, "#2b1d42");
      grad.addColorStop(1, "#121524");
      c2d.fillStyle = grad;
      c2d.fillRect(0, 0, w, h);
    } else {
      c2d.fillStyle = "#141518";
      c2d.fillRect(0, 0, w, h);
    }

    const pulse = Math.sin(tick * 0.18) * 4;
    if (sourceId === "win_orca" || sourceId.startsWith("screen_")) {
      c2d.fillStyle = "#18191c";
      c2d.fillRect(10, 8, w - 20, h - 18);
      c2d.strokeStyle = "#3f4147";
      c2d.lineWidth = 1;
      c2d.strokeRect(10, 8, w - 20, h - 18);
      c2d.fillStyle = "#111214";
      c2d.fillRect(10, 8, 34, h - 18);
      c2d.fillStyle = "#23a55a";
      for (let i = 0; i < 5; i += 1) {
        c2d.fillRect(16, 16 + i * 11, 20, 5);
      }
      c2d.fillStyle = "#5865f2";
      c2d.fillRect(50, 18, Math.max(40, w - 76 + pulse), 8);
      c2d.fillStyle = "#b5bac1";
      c2d.fillRect(50, 32, Math.max(30, w - 96), 5);
      c2d.fillRect(50, 42, Math.max(30, w - 84), 5);
      c2d.fillRect(50, 52, Math.max(30, w - 108), 5);
    } else if (sourceId === "win_firefox") {
      c2d.fillStyle = "#1c1b22";
      c2d.fillRect(8, 6, w - 16, h - 12);
      c2d.fillStyle = "#2b2a33";
      c2d.fillRect(8, 6, w - 16, 16);
      c2d.fillStyle = "#0060df";
      c2d.fillRect(20, 28, w - 40, h - 40);
      c2d.fillStyle = "#ffffff";
      c2d.fillRect(32, 40, Math.max(30, w - 68 + pulse), 8);
    } else if (sourceId === "win_chord") {
      c2d.fillStyle = "#313338";
      c2d.fillRect(8, 6, w - 16, h - 12);
      c2d.fillStyle = "#1e1f22";
      c2d.fillRect(8, 6, 36, h - 12);
      c2d.fillStyle = "#5865f2";
      c2d.beginPath();
      c2d.arc(26, 24, 8, 0, Math.PI * 2);
      c2d.fill();
      c2d.fillStyle = "#23a55a";
      c2d.fillRect(52, 20, Math.max(30, w - 76 + pulse), 7);
      c2d.fillStyle = "#dbdee1";
      c2d.fillRect(52, 34, Math.max(30, w - 90), 5);
      c2d.fillRect(52, 44, Math.max(30, w - 82), 5);
    } else {
      c2d.fillStyle = "#f5f6f8";
      c2d.fillRect(8, 6, w - 16, h - 12);
      c2d.fillStyle = "#e1e4e8";
      c2d.fillRect(8, 6, 38, h - 12);
      c2d.fillStyle = "#3daee9";
      for (let i = 0; i < 3; i += 1) {
        c2d.fillRect(54 + i * 34, 24, 26, 20);
      }
    }
    if (sourceId === "screen_region") {
      c2d.strokeStyle = "#00a8fc";
      c2d.lineWidth = 2;
      c2d.setLineDash([4, 3]);
      c2d.strokeRect(22, 14, w - 44, h - 28);
      c2d.setLineDash([]);
    }
  }

  /**
   * Synchronize the floating PiP preview window for an active stream (`#live-stream-pip-preview`).
   */
  function syncFloatingStreamPipPreview() {
    cancelPipAnimTimer();
    const prevDragCleanup = ctx.getActivePipDragCleanup();
    if (prevDragCleanup) {
      prevDragCleanup();
      ctx.setActivePipDragCleanup(null);
    }
    document.getElementById("live-stream-pip-preview")?.remove();
    const conn = /** @type {any} */ (ctx.getActiveVoiceConnection());
    if (!conn || !conn.screenShareOn || conn.hideStreamPreview) return;

    const floatingPipPosition = ctx.getFloatingPipPosition();
    const pip = document.createElement("div");
    pip.id = "live-stream-pip-preview";
    pip.setAttribute("role", "region");
    pip.setAttribute("aria-label", "Предпросмотр вашей демонстрации экрана");
    pip.setAttribute("data-draggable", "true");
    pip.setAttribute("data-dragging", "false");
    const pipPosCss = floatingPipPosition
      ? `left:${floatingPipPosition.left}px;top:${floatingPipPosition.top}px;right:auto;bottom:auto;`
      : "right:24px;bottom:24px;";
    pip.style.cssText =
      `position:fixed;${pipPosCss}width:320px;height:180px;border-radius:10px;overflow:hidden;background:#111214;border:1px solid #3f4147;box-shadow:0 18px 46px rgba(0,0,0,0.85);z-index:9200;display:flex;flex-direction:column;justify-content:space-between;user-select:none;cursor:grab;touch-action:none;`;
    pip.title = "Нажмите, чтобы развернуть окно стрима (перетащите для перемещения)";

    let isDraggingPip = false;
    let pipDragMoved = false;
    let pipDragStartX = 0;
    let pipDragStartY = 0;
    let pipOriginLeft = floatingPipPosition
      ? floatingPipPosition.left
      : Math.max(8, (Number(window.innerWidth) || 1280) - 320 - 24);
    let pipOriginTop = floatingPipPosition
      ? floatingPipPosition.top
      : Math.max(8, (Number(window.innerHeight) || 720) - 180 - 24);

    /** @param {MouseEvent | PointerEvent} ev */
    const onPipMouseDown = (ev) => {
      if (typeof ev.button === "number" && ev.button !== 0) return;
      const targetEl = /** @type {HTMLElement | null} */ (ev.target);
      if (targetEl && typeof targetEl.closest === "function" && targetEl.closest("button")) {
        return;
      }
      isDraggingPip = true;
      pipDragMoved = false;
      pipDragStartX = Number(ev.clientX) || 0;
      pipDragStartY = Number(ev.clientY) || 0;
      const rect = typeof pip.getBoundingClientRect === "function" ? pip.getBoundingClientRect() : null;
      const parsedLeft = Number.parseFloat(pip.style.left);
      const parsedTop = Number.parseFloat(pip.style.top);
      const curFloatPos = ctx.getFloatingPipPosition();
      if (Number.isFinite(parsedLeft)) {
        pipOriginLeft = parsedLeft;
      } else if (rect && Number.isFinite(rect.left) && (rect.left !== 0 || rect.top !== 0)) {
        pipOriginLeft = rect.left;
      } else if (curFloatPos) {
        pipOriginLeft = curFloatPos.left;
      } else {
        pipOriginLeft = Math.max(8, (Number(window.innerWidth) || 1280) - 320 - 24);
      }
      if (Number.isFinite(parsedTop)) {
        pipOriginTop = parsedTop;
      } else if (rect && Number.isFinite(rect.top) && (rect.left !== 0 || rect.top !== 0)) {
        pipOriginTop = rect.top;
      } else if (curFloatPos) {
        pipOriginTop = curFloatPos.top;
      } else {
        pipOriginTop = Math.max(8, (Number(window.innerHeight) || 720) - 180 - 24);
      }
      pip.setAttribute("data-dragging", "true");
      pip.style.cursor = "grabbing";
    };

    /** @param {MouseEvent | PointerEvent} ev */
    const onPipMouseMove = (ev) => {
      if (!isDraggingPip) return;
      const clientX = Number(ev.clientX) || 0;
      const clientY = Number(ev.clientY) || 0;
      const dx = clientX - pipDragStartX;
      const dy = clientY - pipDragStartY;
      if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
        pipDragMoved = true;
      }
      const vw = Number(window.innerWidth) || 1280;
      const vh = Number(window.innerHeight) || 720;
      const pw = Number(pip.offsetWidth) || 320;
      const ph = Number(pip.offsetHeight) || 180;
      const maxLeft = Math.max(8, vw - pw - 8);
      const maxTop = Math.max(8, vh - ph - 8);
      const nextLeft = Math.max(8, Math.min(maxLeft, Math.round(pipOriginLeft + dx)));
      const nextTop = Math.max(8, Math.min(maxTop, Math.round(pipOriginTop + dy)));
      ctx.setFloatingPipPosition({ left: nextLeft, top: nextTop });
      pip.style.left = `${nextLeft}px`;
      pip.style.top = `${nextTop}px`;
      pip.style.right = "auto";
      pip.style.bottom = "auto";
    };

    const onPipMouseUp = () => {
      if (!isDraggingPip) return;
      isDraggingPip = false;
      pip.setAttribute("data-dragging", "false");
      pip.style.cursor = "grab";
    };

    pip.addEventListener("mousedown", onPipMouseDown);
    pip.addEventListener("pointerdown", onPipMouseDown);
    pip.addEventListener("dragstart", (ev) => {
      ev.preventDefault?.();
    });
    window.addEventListener("mousemove", onPipMouseMove);
    window.addEventListener("pointermove", onPipMouseMove);
    window.addEventListener("mouseup", onPipMouseUp);
    window.addEventListener("pointerup", onPipMouseUp);
    ctx.setActivePipDragCleanup(() => {
      window.removeEventListener("mousemove", onPipMouseMove);
      window.removeEventListener("pointermove", onPipMouseMove);
      window.removeEventListener("mouseup", onPipMouseUp);
      window.removeEventListener("pointerup", onPipMouseUp);
    });

    pip.addEventListener("click", (ev) => {
      if (pipDragMoved) {
        pipDragMoved = false;
        ev.stopPropagation?.();
        return;
      }
      const targetEl = /** @type {HTMLElement | null} */ (ev.target);
      if (targetEl && typeof targetEl.closest === "function" && targetEl.closest("button")) return;
      const liveSelfUser = ctx.getLiveSelfUser();
      ctx.openLiveStreamStageViewer(
        {
          userId: String(liveSelfUser?.id || ""),
          guildId: conn.guildId,
          channelId: conn.channelId,
          displayName: cleanDiscordStr(liveSelfUser?.globalName || liveSelfUser?.username) || "Пользователь",
          streaming: true,
        },
        { id: conn.channelId, name: conn.channelName || "Стрим" },
      );
    });

    const cv = document.createElement("canvas");
    cv.width = 568;
    cv.height = 320;
    cv.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block;";
    let tick = 0;
    const srcId = String(conn.streamSourceId || "win_orca");
    drawLiveCaptureCanvasFrame(cv, srcId, tick, true);

    let lastPipTickTime = typeof performance !== "undefined" && performance.now ? performance.now() : Date.now();
    const PIP_FRAME_THROTTLE_MS = 550;

    /** @param {DOMHighResTimeStamp} [timestamp] */
    const stepPipFrame = (timestamp = 0) => {
      if (document.getElementById("live-stream-pip-preview") !== pip && !pip.isConnected && !document.body.contains(pip)) {
        ctx.setActivePipAnimTimer(null);
        return;
      }

      if (typeof requestAnimationFrame === "function") {
        ctx.setActivePipAnimTimer(requestAnimationFrame(stepPipFrame));
      } else {
        ctx.setActivePipAnimTimer(/** @type {any} */ (setTimeout(() => stepPipFrame(Date.now()), PIP_FRAME_THROTTLE_MS)));
      }

      if (typeof document !== "undefined" && document.hidden) return;
      if (document.getElementById("live-stream-stage-viewer")) return;

      const now = Number(timestamp) || (typeof performance !== "undefined" && performance.now ? performance.now() : Date.now());
      if (now - lastPipTickTime < PIP_FRAME_THROTTLE_MS) return;
      lastPipTickTime = now;

      tick += 1;
      drawLiveCaptureCanvasFrame(cv, srcId, tick, true);
    };

    if (typeof requestAnimationFrame === "function") {
      ctx.setActivePipAnimTimer(requestAnimationFrame(stepPipFrame));
    } else {
      ctx.setActivePipAnimTimer(/** @type {any} */ (setTimeout(() => stepPipFrame(Date.now()), PIP_FRAME_THROTTLE_MS)));
    }

    const topOverlay = document.createElement("div");
    topOverlay.className = "stream-pip-overlay-top";
    const chPill = document.createElement("span");
    chPill.style.cssText =
      "display:inline-flex;align-items:center;gap:5px;padding:3px 8px;border-radius:6px;background:rgba(17,18,20,0.78);color:#f2f3f5;font-size:12px;font-weight:700;";
    chPill.textContent = `← ${conn.channelName || "Стрим"}`;

    const topRightGroup = document.createElement("div");
    topRightGroup.style.cssText = "display:inline-flex;align-items:center;gap:4px;";

    const changePipSourceBtn = document.createElement("button");
    changePipSourceBtn.type = "button";
    changePipSourceBtn.id = "pip-change-source-btn";
    changePipSourceBtn.title = "Сменить источник";
    changePipSourceBtn.setAttribute("aria-label", "Сменить источник");
    changePipSourceBtn.style.cssText =
      "width:28px;height:28px;border-radius:6px;border:none;background:rgba(17,18,20,0.78);color:#dbdee1;font-size:13px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
    changePipSourceBtn.textContent = "🖥";
    changePipSourceBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      ctx.openScreenSharePortalPicker("windows", switchLiveStreamSource);
    });

    const gearBtn = document.createElement("button");
    gearBtn.type = "button";
    gearBtn.title = "Параметры стрима";
    gearBtn.style.cssText =
      "width:28px;height:28px;border-radius:6px;border:none;background:rgba(17,18,20,0.78);color:#dbdee1;font-size:14px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
    gearBtn.textContent = "⚙";
    gearBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      ctx.openStreamSettingsMenu(gearBtn, conn, () => {
        ctx.renderLiveVoicePanel();
      });
    });
    topRightGroup.append(changePipSourceBtn, gearBtn);
    topOverlay.append(chPill, topRightGroup);

    const bottomOverlay = document.createElement("div");
    bottomOverlay.style.cssText =
      "position:relative;z-index:1;display:flex;align-items:center;justify-content:space-between;padding:8px 10px;background:linear-gradient(0deg,rgba(0,0,0,0.75) 0%,transparent 100%);";
    const selfName = document.createElement("span");
    selfName.style.cssText =
      "padding:2px 8px;border-radius:4px;background:rgba(17,18,20,0.75);color:#f2f3f5;font-size:12px;font-weight:700;";
    const liveSelfUser = ctx.getLiveSelfUser();
    selfName.textContent = cleanDiscordStr(liveSelfUser?.globalName || liveSelfUser?.username) || "Пользователь";

    const stopBtn = document.createElement("button");
    stopBtn.type = "button";
    stopBtn.title = "Прекратить стрим";
    stopBtn.style.cssText =
      "width:30px;height:30px;border-radius:6px;border:none;background:rgba(17,18,20,0.85);color:#f2f3f5;font-size:14px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
    stopBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><rect x="3" y="4" width="18" height="12" rx="2"/><line x1="8" y1="20" x2="16" y2="20"/><line x1="9" y1="7" x2="15" y2="13"/><line x1="15" y1="7" x2="9" y2="13"/></svg>`;
    stopBtn.addEventListener("click", () => {
      conn.screenShareOn = false;
      conn.hideStreamPreview = true;
      cancelPipAnimTimer();
      const cleanup = ctx.getActivePipDragCleanup();
      if (cleanup) {
        cleanup();
        ctx.setActivePipDragCleanup(null);
      }
      document.getElementById("live-stream-stage-viewer")?.remove();
      document.getElementById("live-stream-pip-preview")?.remove();
      document.getElementById("live-stream-preview-overlay")?.remove();
      document.getElementById("live-screenshare-modal")?.remove();
      document.getElementById("live-screenshare-portal-modal")?.remove();
      document.getElementById("live-stream-settings-menu")?.remove();
      document.getElementById("live-stream-settings-submenu")?.remove();
      pip.remove();
      ctx.setCachedLiveScreenImg(null);
      ctx.setCachedLiveScreenDataUrl("");
      ctx.setLastLiveScreenSeq(0);
      const selfId = cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
      for (const gk of new Set([conn.guildId, ctx.normalizeVoiceGuildKey(conn.guildId)])) {
        const gList = ctx.liveVoiceStatesByGuild.get(gk) ?? [];
        for (const v of gList) {
          if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
            v.self_stream = false;
            v.selfStream = false;
            v.streaming = false;
          }
        }
      }
      ctx.playVoiceUiSound("stream_off");
      const isCall = !conn.guildId || conn.guildId === HOME_GUILD_ID || conn.guildId === "@me";
      const stopKey =
        conn.activeStreamKey ||
        /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ ||
        (isCall ? `call:${conn.channelId}:${selfId}` : `guild:${conn.guildId}:${conn.channelId}:${selfId}`);
      void ctx.invoke("voice_stream_stop", {
        stream_key: stopKey,
        streamKey: stopKey,
      }).catch(() => {});
      conn.activeStreamKey = null;
      /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ = null;
      ctx.syncVoiceMuteStateWithBackend();
      ctx.renderLiveVoicePanel();
      ctx.tree.refreshView();
    });
    bottomOverlay.append(selfName, stopBtn);

    pip.append(cv, topOverlay, bottomOverlay);
    document.body.append(pip);
  }

  /**
   * Screen Share modal dialog.
   * @param {boolean} [forceOpenConfig]
   */
  function openScreenShareModal(forceOpenConfig = false) {
    document.getElementById("live-screenshare-modal")?.remove();
    document.getElementById("live-stream-settings-menu")?.remove();
    document.getElementById("live-stream-settings-submenu")?.remove();
    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    if (!activeVoiceConnection) return;
    const conn = /** @type {any} */ (activeVoiceConnection);
    if (!conn.screenShareOn && !forceOpenConfig) {
      if (!ctx.canStreamInChannel(conn.guildId, conn.channelId)) {
        ctx.showVoiceToastNotice("Отсутствует право на проведение трансляции (STREAM) в этом канале");
        return;
      }
    }
    if (activeVoiceConnection.screenShareOn && !forceOpenConfig) {
      activeVoiceConnection.screenShareOn = false;
      conn.hideStreamPreview = true;
      cancelPipAnimTimer();
      document.getElementById("live-stream-stage-viewer")?.remove();
      document.getElementById("live-stream-pip-preview")?.remove();
      document.getElementById("live-stream-preview-overlay")?.remove();
      document.getElementById("live-screenshare-portal-modal")?.remove();
      ctx.setCachedLiveScreenImg(null);
      ctx.setCachedLiveScreenDataUrl("");
      ctx.setLastLiveScreenSeq(0);
      const selfId = cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
      for (const gk of new Set([
        activeVoiceConnection.guildId,
        ctx.normalizeVoiceGuildKey(activeVoiceConnection.guildId),
      ])) {
        const gList = ctx.liveVoiceStatesByGuild.get(gk) ?? [];
        for (const v of gList) {
          if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
            v.self_stream = false;
            v.selfStream = false;
            v.streaming = false;
          }
        }
      }
      ctx.playVoiceUiSound("stream_off");
      const isCall = !conn.guildId || conn.guildId === HOME_GUILD_ID || conn.guildId === "@me";
      const stopKey =
        conn.activeStreamKey ||
        /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ ||
        (isCall ? `call:${conn.channelId}:${selfId}` : `guild:${conn.guildId}:${conn.channelId}:${selfId}`);
      void ctx.invoke("voice_stream_stop", {
        stream_key: stopKey,
        streamKey: stopKey,
      }).catch(() => {});
      conn.activeStreamKey = null;
      /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ = null;
      ctx.syncVoiceMuteStateWithBackend();
      ctx.renderLiveVoicePanel();
      ctx.tree.refreshView();
      return;
    }
    ctx.emitLiveTelemetry({
      type: "voice_screenshare_modal_open",
      guildId: activeVoiceConnection.guildId,
      channelId: activeVoiceConnection.channelId,
      screenShareOn: Boolean(activeVoiceConnection.screenShareOn),
    });
    if (!conn.preset) conn.preset = "games";
    if (!conn.resolution) conn.resolution = conn.streamResolution || "720p";
    if (!conn.fps) conn.fps = Number(conn.streamFps || 60);
    if (conn.streamAudio === undefined) conn.streamAudio = true;
    if (!conn.qualityTier) conn.qualityTier = conn.resolution === "720p" ? "SD" : "HD";

    let mainTab = /** @type {"apps" | "devices"} */ (conn.streamMainTab === "devices" ? "devices" : "apps");
    let appCaptureKind = /** @type {"screens" | "windows"} */ (conn.streamPortalKind || "screens");

    const backdrop = document.createElement("div");
    backdrop.id = "live-screenshare-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.78);display:flex;align-items:center;justify-content:center;z-index:9999;";
    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", "Демонстрация экрана");
    card.style.cssText =
      "width:620px;max-width:95vw;background:#2b2d31;color:#f2f3f5;border:1px solid #3f4147;border-radius:12px;overflow:hidden;box-shadow:0 24px 64px rgba(0,0,0,0.88);display:flex;flex-direction:column;font-size:13px;font-family:inherit;";

    const cleanupModal = () => {
      document.getElementById("live-stream-settings-menu")?.remove();
      document.getElementById("live-stream-settings-submenu")?.remove();
      backdrop.remove();
    };

    const startStreamWithSource = (/** @type {{ id: string, label: string, tab?: string }} */ picked) => {
      conn.screenShareOn = true;
      conn.hideStreamPreview = false;
      conn.streamMainTab = mainTab;
      conn.streamPortalKind = picked.tab || appCaptureKind;
      conn.streamSourceId = picked.id;
      conn.streamSourceName = picked.label || "Окно, экран или приложение";
      conn.streamSourceLabel = picked.label || "Окно, экран или приложение";
      conn.streamResolution = conn.resolution || "720p";
      conn.streamFps = Number(conn.fps || 60);
      const liveSelfUser = ctx.getLiveSelfUser();
      const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
      for (const gk of new Set([conn.guildId, ctx.normalizeVoiceGuildKey(conn.guildId)])) {
        const gList = ctx.liveVoiceStatesByGuild.get(gk) ?? [];
        for (const v of gList) {
          if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
            v.self_stream = true;
            v.selfStream = true;
          }
        }
      }
      cleanupModal();
      ctx.playVoiceUiSound("stream_on");
      const isCall = !conn.guildId || conn.guildId === HOME_GUILD_ID || conn.guildId === "@me";
      void ctx.invoke("voice_stream_start", {
        spec: {
          guild_id: isCall ? null : conn.guildId,
          channel_id: conn.channelId || "default",
          source_id: picked.id,
          resolution: conn.streamResolution || "720p",
          fps: Number(conn.streamFps || 60),
          share_audio: conn.streamAudio !== false,
        },
      })
        .then((/** @type {any} */ res) => {
          if (res && typeof res === "object" && res.streamKey) {
            conn.activeStreamKey = String(res.streamKey);
            /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ = String(res.streamKey);
          }
        })
        .catch((/** @type {unknown} */ err) => {
          console.warn("[voice_stream_start err]", err);
        });
      void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "set_capture_source",
        deviceId: (picked.tab || appCaptureKind) + ":" + picked.id + ":" + (picked.label || ""),
      }).catch(() => {});
      ctx.syncVoiceMuteStateWithBackend();
      requestLiveScreenPreviewFrame();
      ctx.renderLiveVoicePanel();
      ctx.tree.refreshView();
      ctx.openLiveStreamStageViewer(
        {
          userId: selfId,
          guildId: conn.guildId,
          channelId: conn.channelId,
          displayName: cleanDiscordStr(liveSelfUser?.globalName || liveSelfUser?.username) || "Вы",
          streaming: true,
        },
        { id: conn.channelId, name: conn.channelName || "Стрим" },
      );
      ctx.emitLiveTelemetry({
        type: "voice_screenshare_start",
        source: picked.id,
        sourceLabel: conn.streamSourceLabel,
        resolution: conn.streamResolution,
        fps: conn.streamFps,
        audio: conn.streamAudio !== false,
        channelId: conn.channelId,
      });
    };

    const topArea = document.createElement("div");
    topArea.style.cssText = "padding:18px 20px 16px;display:flex;flex-direction:column;gap:16px;";

    const segmentedWrap = document.createElement("div");
    segmentedWrap.style.cssText =
      "display:grid;grid-template-columns:repeat(2,1fr);gap:4px;padding:4px;border-radius:10px;background:#1e1f22;";

    const centerContent = document.createElement("div");
    centerContent.style.cssText = "min-height:224px;display:flex;flex-direction:column;gap:12px;justify-content:center;";

    const bottomBar = document.createElement("div");
    bottomBar.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:12px 20px;background:#232428;border-top:1px solid #35373c;";

    const nitroBanner = document.createElement("div");
    nitroBanner.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;gap:10px;padding:10px 20px;background:linear-gradient(90deg,#3b1d5e 0%,#581c87 55%,#4c1d95 100%);color:#f2f3f5;font-size:13px;";
    const nitroLeft = document.createElement("span");
    nitroLeft.style.cssText = "font-weight:600;color:#f3e8ff;";
    nitroLeft.textContent = "Стримьте в разрешении HD с подпиской Nitro";
    const nitroRight = document.createElement("div");
    nitroRight.style.cssText = "display:flex;align-items:center;gap:8px;";
    const heartBtn = document.createElement("button");
    heartBtn.type = "button";
    heartBtn.style.cssText =
      "width:32px;height:30px;border-radius:6px;border:1px solid rgba(255,255,255,0.22);background:rgba(255,255,255,0.08);color:#f3e8ff;cursor:pointer;font-size:14px;";
    heartBtn.textContent = "♡";
    const nitroBtn = document.createElement("button");
    nitroBtn.type = "button";
    nitroBtn.style.cssText =
      "padding:6px 12px;border-radius:6px;border:none;background:#f2f3f5;color:#111214;font-size:12px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;gap:6px;";
    nitroBtn.innerHTML = `<span style="color:#9333ea;">✦</span><span>Подписаться на Nitro</span>`;
    nitroBtn.addEventListener("click", () => {
      ctx.openNitroHdModal();
    });
    nitroRight.append(heartBtn, nitroBtn);
    nitroBanner.append(nitroLeft, nitroRight);

    const renderModalState = () => {
      segmentedWrap.replaceChildren();
      for (const t of [
        { id: /** @type {const} */ ("apps"), icon: "🖥", label: "Приложения" },
        { id: /** @type {const} */ ("devices"), icon: "📹", label: "Устройства" },
      ]) {
        const active = mainTab === t.id;
        const b = document.createElement("button");
        b.type = "button";
        b.setAttribute("role", "tab");
        b.setAttribute("aria-selected", active ? "true" : "false");
        b.style.cssText =
          `padding:8px 14px;border-radius:8px;border:none;background:${active ? "#35373c" : "transparent"};color:${active ? "#f2f3f5" : "#949ba4"};font-size:13px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;gap:8px;`;
        b.innerHTML = `<span>${t.icon}</span><span>${t.label}</span>`;
        b.addEventListener("click", () => {
          mainTab = t.id;
          renderModalState();
        });
        segmentedWrap.append(b);
      }

      centerContent.replaceChildren();
      if (mainTab === "apps") {
        const heading = document.createElement("div");
        heading.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;";
        heading.textContent = "Демонстрируйте экран, окно или приложение";

        const cardsGrid = document.createElement("div");
        cardsGrid.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);gap:12px;flex:1;";

        const makeBigCard = (
          /** @type {"screens" | "windows"} */ kind,
          /** @type {string} */ label,
          /** @type {string} */ svgIcon,
        ) => {
          const isSel = appCaptureKind === kind;
          const btn = document.createElement("button");
          btn.type = "button";
          btn.setAttribute("data-capture-kind", kind);
          btn.style.cssText =
            `min-height:178px;border-radius:10px;border:1.5px solid ${isSel ? "#5865f2" : "#3f4147"};background:#313338;color:#f2f3f5;cursor:pointer;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;padding:20px;transition:border-color 120ms ease,background 120ms ease;`;
          btn.addEventListener("mouseenter", () => {
            btn.style.borderColor = "#5865f2";
            btn.style.background = "#35373c";
          });
          btn.addEventListener("mouseleave", () => {
            btn.style.borderColor = isSel ? "#5865f2" : "#3f4147";
            btn.style.background = "#313338";
          });
          const circle = document.createElement("span");
          circle.style.cssText =
            "width:54px;height:54px;border-radius:50%;background:#1e1f22;color:#dbdee1;display:inline-flex;align-items:center;justify-content:center;";
          circle.innerHTML = svgIcon;
          const txt = document.createElement("span");
          txt.style.cssText = "font-size:13px;font-weight:700;color:#f2f3f5;";
          txt.textContent = label;
          btn.append(circle, txt);
          btn.addEventListener("click", () => {
            appCaptureKind = kind;
            ctx.openScreenSharePortalPicker(kind, (/** @type {any} */ picked) => {
              startStreamWithSource(picked);
            });
          });
          return btn;
        };

        cardsGrid.append(
          makeBigCard(
            "screens",
            "Демонстрировать весь экран",
            `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>`,
          ),
          makeBigCard(
            "windows",
            "Демонстрировать окно",
            `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="16" rx="2"/><line x1="3" y1="9" x2="21" y2="9"/><circle cx="6.5" cy="6.5" r="1"/><circle cx="9.5" cy="6.5" r="1"/></svg>`,
          ),
        );
        centerContent.append(heading, cardsGrid);
      } else {
        const devHeading = document.createElement("div");
        devHeading.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;margin-bottom:8px;";
        devHeading.textContent = "Выберите видеоустройство для трансляции";
        const devGrid = document.createElement("div");
        devGrid.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);gap:12px;";
        const devicesList = [
          { id: "cam_v4l2_0", label: "USB HD Webcam (/dev/video0)", sub: "1080p • 60 FPS • V4L2" },
          { id: "cam_pipewire", label: "PipeWire Video Capture", sub: "Системный видеопоток • PipeWire" },
          { id: "cam_obs_virt", label: "OBS Virtual Camera", sub: "Виртуальная камера • 1080p" },
          { id: "screen_x11_0", label: "X11 / Wayland Захват монитора :0", sub: "Полный экран • Аппаратный" },
        ];
        for (const dev of devicesList) {
          const isSelected = (conn.selectedDeviceId || "cam_v4l2_0") === dev.id;
          const dCard = document.createElement("button");
          dCard.type = "button";
          dCard.style.cssText =
            `min-height:92px;border-radius:10px;border:1.5px solid ${isSelected ? "#5865f2" : "#3f4147"};background:${isSelected ? "rgba(88,101,242,0.14)" : "#313338"};color:#f2f3f5;cursor:pointer;display:flex;align-items:center;gap:12px;padding:14px;text-align:left;transition:border-color 120ms ease,background 120ms ease;`;
          dCard.innerHTML = `
            <span style="width:42px;height:42px;border-radius:10px;background:#1e1f22;display:inline-flex;align-items:center;justify-content:center;font-size:20px;flex-shrink:0;">📹</span>
            <span style="display:flex;flex-direction:column;gap:3px;min-width:0;">
              <span style="font-size:13px;font-weight:700;color:#f2f3f5;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${dev.label}</span>
              <span style="font-size:11px;color:#949ba4;">${dev.sub}</span>
            </span>
          `;
          dCard.addEventListener("click", () => {
            conn.selectedDeviceId = dev.id;
            conn.selectedDeviceLabel = dev.label;
            renderModalState();
          });
          dCard.addEventListener("dblclick", () => {
            ctx.playVoiceUiSound("stream_on");
            startStreamWithSource({ id: dev.id, label: dev.label, tab: "devices" });
          });
          devGrid.append(dCard);
        }
        centerContent.append(devHeading, devGrid);
      }

      bottomBar.replaceChildren();
      const presetNames = {
        games: "Игры",
        screen: "Демонстрация экрана",
        custom: "Пользовательские",
      };
      const presetTitle = presetNames[/** @type {keyof typeof presetNames} */ (conn.preset || "games")] || "Игры";
      const summaryLeft = document.createElement("div");
      summaryLeft.style.cssText = "display:flex;flex-direction:column;gap:2px;";
      const sumTop = document.createElement("div");
      sumTop.style.cssText = "font-size:13px;font-weight:700;color:#f2f3f5;";
      sumTop.textContent = presetTitle;
      const sumSub = document.createElement("div");
      sumSub.style.cssText = "display:flex;align-items:center;gap:8px;font-size:12px;color:#b5bac1;";
      sumSub.innerHTML = `<span>${conn.resolution || "720p"}</span><span>${conn.fps || 60}fps</span><span style="display:inline-flex;align-items:center;gap:4px;">${conn.streamAudio !== false ? "🔊 Звук системы" : "🔇 Без звука"}</span>`;
      summaryLeft.append(sumTop, sumSub);

      const controlsRight = document.createElement("div");
      controlsRight.style.cssText = "display:flex;align-items:center;gap:8px;";

      const sdHdWrap = document.createElement("div");
      sdHdWrap.style.cssText =
        "display:inline-flex;padding:3px;border-radius:8px;background:#1e1f22;border:1px solid #35373c;";
      for (const tier of ["SD", "HD"]) {
        const active = (conn.qualityTier || "SD") === tier;
        const tb = document.createElement("button");
        tb.type = "button";
        tb.style.cssText =
          `padding:5px 10px;border-radius:6px;border:none;background:${active ? "#35373c" : "transparent"};color:${active ? "#f2f3f5" : tier === "HD" ? "#c084fc" : "#949ba4"};font-size:12px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;gap:4px;`;
        tb.innerHTML = tier === "HD" ? `<span style="color:#c084fc;">✦</span><span>HD</span>` : `<span>SD</span>`;
        tb.addEventListener("click", () => {
          conn.qualityTier = tier;
          if (tier === "SD") {
            conn.resolution = "720p";
            conn.streamResolution = "720p";
            conn.fps = 60;
            conn.streamFps = 60;
          } else {
            conn.resolution = "1080p";
            conn.streamResolution = "1080p";
            conn.fps = 60;
            conn.streamFps = 60;
          }
          renderModalState();
        });
        sdHdWrap.append(tb);
      }

      const primaryBtn = document.createElement("button");
      primaryBtn.type = "button";
      primaryBtn.style.cssText =
        "padding:8px 16px;border-radius:8px;border:none;background:#5865f2;color:#ffffff;font-size:13px;font-weight:700;cursor:pointer;";
      primaryBtn.textContent = mainTab === "devices" ? "Стримить" : "Выбрать нужное";
      primaryBtn.addEventListener("click", () => {
        if (mainTab === "devices") {
          ctx.playVoiceUiSound("stream_on");
          startStreamWithSource({
            id: conn.selectedDeviceId || "cam_v4l2_0",
            label: conn.selectedDeviceLabel || "USB HD Webcam (/dev/video0)",
            tab: "devices",
          });
        } else {
          ctx.openScreenSharePortalPicker(appCaptureKind, (/** @type {any} */ picked) => {
            ctx.playVoiceUiSound("stream_on");
            startStreamWithSource(picked);
          });
        }
      });

      const gearBtn = document.createElement("button");
      gearBtn.type = "button";
      gearBtn.title = "Настройки режима стрима";
      gearBtn.setAttribute("aria-label", "Настройки режима стрима");
      gearBtn.style.cssText =
        "width:34px;height:34px;border-radius:8px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:16px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
      gearBtn.textContent = "⚙";
      gearBtn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        ctx.openStreamSettingsMenu(gearBtn, conn, () => {
          renderModalState();
        });
      });

      controlsRight.append(sdHdWrap, primaryBtn, gearBtn);
      bottomBar.append(summaryLeft, controlsRight);
    };

    renderModalState();
    topArea.append(segmentedWrap, centerContent);
    card.append(topArea, bottomBar, nitroBanner);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) cleanupModal();
    });
    document.body.append(backdrop);
  }

  return {
    cancelPipAnimTimer,
    requestLiveScreenPreviewFrame,
    switchLiveStreamSource,
    drawLiveCaptureCanvasFrame,
    syncFloatingStreamPipPreview,
    openScreenShareModal,
  };
}
