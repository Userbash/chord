// @ts-check

/**
 * Full-screen / overlay Stream Stage Viewer (`openLiveStreamStageViewer`) supporting
 * remote and local stream preview, zoom/pan, fullscreen toggle, and frame decoding loop.
 */

import { buildStageViewerTopBar } from "./voice-stage-viewer-controls.js";

/**
 * Create the stream stage viewer controller bound to the shared voice call context.
 * @param {any} ctx
 */
export function createVoiceStageViewer(ctx) {
  const { cleanDiscordStr, HOME_GUILD_ID } = ctx;
  /**
   * Open the full-size stream stage viewer and send `OP 20 STREAM_WATCH` when watching a remote stream.
   * Supports mouse-wheel zoom (100%..500%), drag-to-pan, double-click zoom toggle, and fullscreen (`⛶`).
   * @param {any} participant
   * @param {{ id: string, name?: string }} channel
   */
  function openLiveStreamStageViewer(participant, channel) {
    document.getElementById("live-stream-stage-viewer")?.remove();
    const liveSelfUser = ctx.getLiveSelfUser();
    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const rawGid = cleanDiscordStr(
      String(participant?.guildId ?? activeVoiceConnection?.guildId ?? ctx.rail.selectedId ?? ctx.selectedGuildId() ?? ""),
    );
    const normGid = ctx.normalizeVoiceGuildKey(rawGid);
    const gId = normGid === HOME_GUILD_ID ? "" : normGid;
    const cId = String(channel?.id || participant?.channelId || "");
    const uId = cleanDiscordStr(String(participant?.userId || selfId));
    const isSelfStream = Boolean(uId && uId === selfId);
    const streamKey = cId && uId ? (gId ? `guild:${gId}:${cId}:${uId}` : `call:${cId}:${uId}`) : "";

    if (
      ctx.isLiveSession() &&
      cId &&
      (!activeVoiceConnection ||
        activeVoiceConnection.channelId !== cId ||
        (gId && ctx.normalizeVoiceGuildKey(activeVoiceConnection.guildId) !== normGid))
    ) {
      ctx.connectToLiveVoiceChannel(
        gId,
        ctx.currentGuildInfo().name || "Сервер",
        { id: cId, name: channel?.name || "Голосовой канал" },
      );
    }

    if (ctx.isLiveSession() && streamKey) {
      if (!isSelfStream) {
        ctx.playVoiceUiSound("stream_user_joined");
        void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "stream_watch",
          deviceId: streamKey,
        }).catch(() => {});
      }
    }
    if (isSelfStream) {
      ctx.requestLiveScreenPreviewFrame();
    }

    const overlay = document.createElement("div");
    overlay.id = "live-stream-stage-viewer";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-label", `Трансляция ${participant?.displayName || "Участника"}`);
    const normalOverlayCss =
      "position:fixed;left:312px;top:48px;right:248px;bottom:64px;background:#0b0c0e;border:1px solid #2b2d31;border-radius:12px;box-shadow:0 24px 64px rgba(0,0,0,0.9);z-index:9250;display:flex;flex-direction:column;overflow:hidden;user-select:none;contain:strict;";
    const expandedOverlayCss =
      "position:fixed;left:0;top:0;right:0;bottom:0;background:#070809;border:none;border-radius:0;z-index:10500;display:flex;flex-direction:column;overflow:hidden;user-select:none;contain:strict;";
    let isExpandedFullscreen = false;
    overlay.style.cssText = normalOverlayCss;

    const appShellEl = document.getElementById("app-shell");
    const messagesEl = document.getElementById("messages");
    if (messagesEl) {
      messagesEl.style.visibility = "hidden";
      messagesEl.style.contentVisibility = "hidden";
    }
    const syncUnderlyingDomVisibility = () => {
      if (appShellEl) {
        appShellEl.style.visibility = isExpandedFullscreen ? "hidden" : "";
        appShellEl.style.contentVisibility = isExpandedFullscreen ? "hidden" : "";
      }
      if (messagesEl) {
        messagesEl.style.visibility = "hidden";
        messagesEl.style.contentVisibility = "hidden";
      }
    };
    const restoreUnderlyingDomVisibility = () => {
      if (appShellEl) {
        appShellEl.style.visibility = "";
        appShellEl.style.contentVisibility = "";
      }
      if (messagesEl) {
        messagesEl.style.visibility = "";
        messagesEl.style.contentVisibility = "";
      }
    };

    const currentConn = ctx.getActiveVoiceConnection();
    let streamResLabel = /** @type {any} */ (currentConn)?.streamResolution || "720p";
    const targetStreamFps = /** @type {any} */ (currentConn)?.streamFps || 60;
    let measuredStreamFps = 0;
    let renderedFramesInWindow = 0;
    let lastStreamFpsCalcMs = performance.now();

    let zoomScale = 1.0;
    let panX = 0;
    let panY = 0;
    let isDraggingPan = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let dragOriginPanX = 0;
    let dragOriginPanY = 0;

    let zoomResetBtnRef = /** @type {HTMLButtonElement | null} */ (null);
    let fullscreenBtnRef = /** @type {HTMLButtonElement | null} */ (null);

    const stageWrap = document.createElement("div");
    stageWrap.style.cssText =
      "flex:1;position:relative;display:flex;align-items:center;justify-content:center;background:#070809;overflow:hidden;cursor:zoom-in;";

    const stageCanvas = document.createElement("canvas");
    stageCanvas.width = 1280;
    stageCanvas.height = 720;
    stageCanvas.style.cssText =
      "width:100%;height:100%;object-fit:contain;display:none;transform-origin:center center;will-change:transform;pointer-events:none;backface-visibility:hidden;";
    const stageCtx =
      /** @type {CanvasRenderingContext2D | null} */ (
        stageCanvas.getContext("2d", { alpha: false }) ||
          stageCanvas.getContext("2d")
      );

    const stageImg = document.createElement("img");
    stageImg.alt = "Трансляция";
    stageImg.decoding = "async";
    stageImg.style.cssText =
      "width:100%;height:100%;object-fit:contain;display:none;transform-origin:center center;will-change:transform;pointer-events:none;backface-visibility:hidden;";

    const emptyNotice = document.createElement("div");
    const dispName = String(participant?.displayName || "Участник");
    emptyNotice.style.cssText =
      "display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;color:#dbdee1;text-align:center;padding:24px;";
    emptyNotice.innerHTML = `<div style="width:68px;height:68px;border-radius:50%;background:#18191c;border:3px solid #5865f2;display:flex;align-items:center;justify-content:center;font-size:24px;font-weight:800;color:#f2f3f5;">${Array.from(dispName).slice(0, 1).join("").toUpperCase() || "S"}</div><div style="font-size:16px;font-weight:700;color:#f2f3f5;">Трансляция ${dispName}</div><div style="font-size:13px;color:#949ba4;">Подключение к видеопотоку… Колесо мыши — зум (до 500%), перетаскивание — панорамирование</div>`;
    if (isSelfStream) {
      emptyNotice.style.display = "none";
      emptyNotice.style.pointerEvents = "none";
      const cachedLiveScreenDataUrl = ctx.getCachedLiveScreenDataUrl();
      if (cachedLiveScreenDataUrl && cachedLiveScreenDataUrl.startsWith("data:image/")) {
        stageImg.src = cachedLiveScreenDataUrl;
        stageImg.style.display = "block";
      } else {
        stageImg.src = "/dev/shm/discord_live_preview_thumb.jpg";
        stageImg.style.display = "block";
        stageImg.onerror = () => {
          if (stageImg.src.includes("discord_live_preview_thumb")) {
            stageImg.src = "http://127.0.0.1:49281/preview.jpg";
          }
        };
      }
    }

    const zoomHintPill = document.createElement("div");
    zoomHintPill.style.cssText =
      "position:absolute;left:14px;bottom:12px;padding:4px 10px;border-radius:6px;background:rgba(17,18,20,0.78);color:#b5bac1;font-size:11px;font-weight:600;pointer-events:none;z-index:2;";
    zoomHintPill.textContent = "🔍 Колесо мыши — масштаб (100%–500%) · Зажать ЛКМ — перемещение · Двойной клик — 225%";

    stageWrap.append(stageImg, stageCanvas, emptyNotice, zoomHintPill);

    const clampPan = () => {
      if (zoomScale <= 1.01) {
        zoomScale = 1.0;
        panX = 0;
        panY = 0;
        return;
      }
      const rect = stageWrap.getBoundingClientRect();
      const maxX = Math.max(0, ((rect.width || 960) * (zoomScale - 1)) / 2);
      const maxY = Math.max(0, ((rect.height || 540) * (zoomScale - 1)) / 2);
      panX = Math.max(-maxX, Math.min(maxX, panX));
      panY = Math.max(-maxY, Math.min(maxY, panY));
    };

    const applyTransform = () => {
      clampPan();
      if (zoomResetBtnRef) {
        zoomResetBtnRef.textContent = `${Math.round(zoomScale * 100)}%`;
      }
      const tf = `translate3d(${Math.round(panX)}px, ${Math.round(panY)}px, 0) scale(${zoomScale.toFixed(3)})`;
      stageCanvas.style.transform = tf;
      stageImg.style.transform = tf;
      stageWrap.style.cursor = zoomScale > 1.01 ? (isDraggingPan ? "grabbing" : "grab") : "zoom-in";
    };

    /**
     * @param {number} nextScale
     * @param {number} [focalClientX]
     * @param {number} [focalClientY]
     */
    const setZoomLevel = (nextScale, focalClientX, focalClientY) => {
      const prevScale = zoomScale;
      const clamped = Math.max(1.0, Math.min(5.0, nextScale));
      if (Math.abs(clamped - prevScale) < 0.005) return;
      if (
        typeof focalClientX === "number" &&
        typeof focalClientY === "number" &&
        clamped > 1.01
      ) {
        const rect = stageWrap.getBoundingClientRect();
        const cx = focalClientX - (rect.left + rect.width / 2);
        const cy = focalClientY - (rect.top + rect.height / 2);
        const ratio = clamped / prevScale;
        panX = cx - (cx - panX) * ratio;
        panY = cy - (cy - panY) * ratio;
      }
      zoomScale = clamped;
      applyTransform();
    };

    const toggleStageFullscreen = () => {
      isExpandedFullscreen = !isExpandedFullscreen;
      overlay.style.cssText = isExpandedFullscreen ? expandedOverlayCss : normalOverlayCss;
      overlay.setAttribute("data-fullscreen", isExpandedFullscreen ? "true" : "false");
      if (fullscreenBtnRef) {
        fullscreenBtnRef.textContent = isExpandedFullscreen ? "🗗 Свернуть в окно" : "⛶ На весь экран";
      }
      syncUnderlyingDomVisibility();
      applyTransform();
    };

    let viewerClosed = false;
    let remotePollInFlight = false;
    let lastRemoteFrameSeq = 0;
    let lastRemoteFrameLoadedAt = Date.now();
    let hasReceivedFirstFrame = false;
    let lastStreamWatchRetryAt = Date.now();
    let badgeTick = 0;
    /** @type {ImageBitmap | HTMLImageElement | null} */
    let pendingBitmap = null;
    let rafPresentScheduled = false;

    const topBarUi = buildStageViewerTopBar({
      ctx,
      participant,
      channel,
      isSelfStream,
      streamKey,
      uId,
      getState: () => ({ streamResLabel, targetStreamFps, measuredStreamFps }),
      onZoomOut: () => setZoomLevel(zoomScale / 1.25),
      onZoomIn: () => setZoomLevel(zoomScale * 1.25),
      onZoomReset: () => {
        zoomScale = 1.0;
        panX = 0;
        panY = 0;
        applyTransform();
      },
      onZoom200: () => {
        if (Math.abs(zoomScale - 2.0) < 0.1) {
          zoomScale = 1.0;
          panX = 0;
          panY = 0;
        } else {
          zoomScale = 2.0;
        }
        applyTransform();
      },
      onToggleFullscreen: toggleStageFullscreen,
      onCloseStage: (/** @type {boolean} */ stopStream) => closeStageViewer(stopStream),
    });
    const { topBar, updateNetBadge, zoomResetBtn, fullscreenBtn, muteStreamBtn } = topBarUi;
    zoomResetBtnRef = zoomResetBtn;
    fullscreenBtnRef = fullscreenBtn;

    stageWrap.addEventListener(
      "wheel",
      (ev) => {
        ev.preventDefault();
        const factor = ev.deltaY < 0 ? 1.18 : 1 / 1.18;
        setZoomLevel(zoomScale * factor, ev.clientX, ev.clientY);
      },
      { passive: false },
    );

    stageWrap.addEventListener("dblclick", (ev) => {
      ev.preventDefault();
      if (zoomScale > 1.15) {
        zoomScale = 1.0;
        panX = 0;
        panY = 0;
        applyTransform();
      } else {
        setZoomLevel(2.25, ev.clientX, ev.clientY);
      }
    });

    stageWrap.addEventListener("mousedown", (ev) => {
      if (ev.button !== 0 || zoomScale <= 1.01) return;
      isDraggingPan = true;
      dragStartX = ev.clientX;
      dragStartY = ev.clientY;
      dragOriginPanX = panX;
      dragOriginPanY = panY;
      stageWrap.style.cursor = "grabbing";
    });
    /** @param {MouseEvent} ev */
    const onWinMouseMove = (ev) => {
      if (!isDraggingPan) return;
      panX = dragOriginPanX + (ev.clientX - dragStartX);
      panY = dragOriginPanY + (ev.clientY - dragStartY);
      applyTransform();
    };
    const onWinMouseUp = () => {
      if (!isDraggingPan) return;
      isDraggingPan = false;
      applyTransform();
    };
    window.addEventListener("mousemove", onWinMouseMove);
    window.addEventListener("mouseup", onWinMouseUp);

    const retryRemoteStreamWatchIfStalled = () => {
      if (viewerClosed || isSelfStream || !ctx.isLiveSession() || !streamKey) return;
      if (hasReceivedFirstFrame || lastRemoteFrameSeq > 0) return;
      const now = Date.now();
      if (now - lastStreamWatchRetryAt >= 12000) {
        lastStreamWatchRetryAt = now;
        void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "stream_watch",
          deviceId: streamKey,
        }).catch(() => {});
      }
    };

    const recordRenderedFrameFps = () => {
      renderedFramesInWindow += 1;
      const nowPerf = performance.now();
      const elapsedPerf = nowPerf - lastStreamFpsCalcMs;
      if (elapsedPerf >= 950) {
        const calcFps = Math.round((renderedFramesInWindow * 1000) / elapsedPerf);
        if (calcFps > 0) {
          measuredStreamFps = Math.min(60, calcFps);
        }
        renderedFramesInWindow = 0;
        lastStreamFpsCalcMs = nowPerf;
      }
    };

    const presentPendingBitmap = () => {
      rafPresentScheduled = false;
      if (viewerClosed || !pendingBitmap) return;
      const bmp = pendingBitmap;
      pendingBitmap = null;
      const bw = Number(/** @type {any} */ (bmp).naturalWidth || bmp.width) || 1280;
      const bh = Number(/** @type {any} */ (bmp).naturalHeight || bmp.height) || 720;
      if (bh >= 1080) streamResLabel = "1080p";
      else if (bh >= 720) streamResLabel = "720p";
      let drawnOnCanvas = false;
      if (stageCtx) {
        try {
          if (stageCanvas.width !== bw || stageCanvas.height !== bh) {
            stageCanvas.width = bw;
            stageCanvas.height = bh;
          }
          stageCtx.drawImage(bmp, 0, 0, bw, bh);
          drawnOnCanvas = true;
        } catch {}
      }
      if (typeof (/** @type {any} */ (bmp).close) === "function") {
        try {
          /** @type {any} */ (bmp).close();
        } catch {}
      }
      if (drawnOnCanvas) {
        hasReceivedFirstFrame = true;
        if (stageCanvas.style.display === "none") {
          stageCanvas.style.display = "block";
          stageImg.style.display = "none";
          emptyNotice.style.display = "none";
        }
        recordRenderedFrameFps();
      }
    };

    /** @param {ImageBitmap | HTMLImageElement} decodedFrame */
    const enqueueDecodedFrame = (decodedFrame) => {
      if (viewerClosed) {
        if (typeof (/** @type {any} */ (decodedFrame).close) === "function") {
          try {
            /** @type {any} */ (decodedFrame).close();
          } catch {}
        }
        return;
      }
      if (pendingBitmap && typeof (/** @type {any} */ (pendingBitmap).close) === "function") {
        try {
          /** @type {any} */ (pendingBitmap).close();
        } catch {}
      }
      pendingBitmap = decodedFrame;
      if (!rafPresentScheduled) {
        rafPresentScheduled = true;
        requestAnimationFrame(presentPendingBitmap);
      }
    };

    let prevBlobUrl = "";
    /** @param {Blob} blob */
    const decodeAndPresentBlob = async (blob) => {
      if (viewerClosed || !blob || blob.size < 64) return;
      if (typeof globalThis.createImageBitmap === "function") {
        try {
          const bmp = await globalThis.createImageBitmap(blob);
          enqueueDecodedFrame(bmp);
          return;
        } catch {}
      }
      if (typeof URL !== "undefined" && typeof URL.createObjectURL === "function") {
        const objUrl = URL.createObjectURL(blob);
        stageImg.onload = () => {
          if (prevBlobUrl && prevBlobUrl !== objUrl) {
            try {
              URL.revokeObjectURL(prevBlobUrl);
            } catch {}
          }
          prevBlobUrl = objUrl;
          hasReceivedFirstFrame = true;
          if (stageImg.style.display === "none" && stageCanvas.style.display === "none") {
            stageImg.style.display = "block";
            emptyNotice.style.display = "none";
          }
          recordRenderedFrameFps();
        };
        stageImg.src = objUrl;
      }
    };

    /** @param {string} dataUrl */
    const showFrameDataUrl = (dataUrl) => {
      if (
        viewerClosed ||
        typeof dataUrl !== "string" ||
        (!dataUrl.startsWith("data:image/") &&
          !dataUrl.startsWith("/dev/shm/") &&
          !dataUrl.startsWith("http://"))
      ) {
        return;
      }
      if (isSelfStream) {
        emptyNotice.style.display = "none";
        emptyNotice.style.pointerEvents = "none";
      }
      stageImg.onload = () => {
        hasReceivedFirstFrame = true;
        if (stageCanvas.style.display === "none") {
          stageImg.style.display = "block";
          emptyNotice.style.display = "none";
        }
        enqueueDecodedFrame(stageImg);
      };
      stageImg.src = dataUrl;
    };

    const pollRemoteFrameIpcFallback = async () => {
      if (viewerClosed || !ctx.isLiveSession() || remotePollInFlight) return;
      if (isSelfStream) {
        ctx.requestLiveScreenPreviewFrame();
        return;
      }
      if (!streamKey) return;
      remotePollInFlight = true;
      try {
        const res = await ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "remote_stream_frame",
          deviceId: `${streamKey}|since=${lastRemoteFrameSeq}`,
        });
        const seq = Number(res?.seq || 0);
        const backendFps = Number(res?.streamFps || 0);
        if (backendFps > 0) {
          measuredStreamFps = Math.max(measuredStreamFps, Math.min(60, backendFps));
        }
        if (res?.unchanged && seq > 0) {
          hasReceivedFirstFrame = true;
          lastRemoteFrameLoadedAt = Date.now();
          return;
        }
        const dataUrl = res?.previewDataUrl;
        if (typeof dataUrl === "string" && dataUrl.startsWith("data:image/")) {
          hasReceivedFirstFrame = true;
          lastRemoteFrameLoadedAt = Date.now();
          if (seq === 0 || seq !== lastRemoteFrameSeq) {
            lastRemoteFrameSeq = seq;
            showFrameDataUrl(dataUrl);
          }
        }
      } catch {
      } finally {
        remotePollInFlight = false;
      }
    };

    const runFastRemoteStreamLoop = async () => {
      let consecutiveHttpMisses = 0;
      let ipcKickTicks = 0;
      void pollRemoteFrameIpcFallback();
      while (!viewerClosed) {
        if (!overlay.isConnected || !ctx.isLiveSession() || (!streamKey && !isSelfStream)) break;
        retryRemoteStreamWatchIfStalled();
        let sleepMs = 14;
        ipcKickTicks += 1;
        if (typeof globalThis.fetch === "function") {
          try {
            const resp = await globalThis.fetch(
              `http://127.0.0.1:49281/frame.jpg?since=${lastRemoteFrameSeq}`,
              { cache: "no-store" },
            );
            const hdrFps = Number(resp.headers.get("X-Stream-Fps") || 0);
            if (hdrFps > 0) {
              measuredStreamFps = Math.min(60, hdrFps);
            }
            if (resp.status === 200) {
              consecutiveHttpMisses = 0;
              const seq = Number(resp.headers.get("X-Frame-Seq") || 0);
              const blob = await resp.blob();
              if (blob.size > 64) {
                hasReceivedFirstFrame = true;
                lastRemoteFrameLoadedAt = Date.now();
                if (seq > 0) lastRemoteFrameSeq = seq;
                await decodeAndPresentBlob(blob);
                sleepMs = 8;
              }
            } else if (resp.status === 204 && lastRemoteFrameSeq > 0) {
              consecutiveHttpMisses = 0;
              hasReceivedFirstFrame = true;
              lastRemoteFrameLoadedAt = Date.now();
              if (ipcKickTicks % 60 === 0) {
                void pollRemoteFrameIpcFallback();
              }
              sleepMs = 10;
            } else if (resp.status === 404 && isSelfStream) {
              const previewResp = await globalThis.fetch(
                `http://127.0.0.1:49281/preview.jpg?t=${Date.now()}`,
                { cache: "no-store" },
              );
              if (previewResp.status === 200) {
                const blob = await previewResp.blob();
                if (blob.size > 64) {
                  consecutiveHttpMisses = 0;
                  hasReceivedFirstFrame = true;
                  lastRemoteFrameLoadedAt = Date.now();
                  lastRemoteFrameSeq += 1;
                  await decodeAndPresentBlob(blob);
                  sleepMs = 40;
                } else {
                  await pollRemoteFrameIpcFallback();
                  sleepMs = 60;
                }
              } else {
                consecutiveHttpMisses += 1;
                await pollRemoteFrameIpcFallback();
                sleepMs = 60;
              }
            } else {
              consecutiveHttpMisses += 1;
              if (ipcKickTicks % 15 === 0) {
                void pollRemoteFrameIpcFallback();
              }
              sleepMs = hasReceivedFirstFrame ? 25 : 60;
            }
          } catch {
            consecutiveHttpMisses += 1;
            if (ipcKickTicks % 15 === 0) {
              void pollRemoteFrameIpcFallback();
            }
            sleepMs = hasReceivedFirstFrame ? 25 : 60;
          }
        } else {
          await pollRemoteFrameIpcFallback();
          sleepMs = 24;
        }
        await new Promise((r) => setTimeout(r, sleepMs));
      }
    };

    overlay.append(topBar, stageWrap);
    document.body.append(overlay);

    if (isSelfStream) {
      emptyNotice.style.display = "none";
      emptyNotice.style.pointerEvents = "none";
      const srcId = String(ctx.getActiveVoiceConnection()?.streamSourceId || "screen_0");
      const renderSelfStreamFallback = () => {
        if (viewerClosed) return;
        emptyNotice.style.display = "none";
        emptyNotice.style.pointerEvents = "none";
        const cachedLiveScreenDataUrl = ctx.getCachedLiveScreenDataUrl();
        if (cachedLiveScreenDataUrl && cachedLiveScreenDataUrl.startsWith("data:image/")) {
          showFrameDataUrl(cachedLiveScreenDataUrl);
          return;
        }
        ctx.drawLiveCaptureCanvasFrame(stageCanvas, srcId, badgeTick, true);
        stageCanvas.style.display = "block";
        stageImg.style.display = "none";
        emptyNotice.style.display = "none";
        recordRenderedFrameFps();
      };
      renderSelfStreamFallback();
      ctx.requestLiveScreenPreviewFrame();
      void runFastRemoteStreamLoop();
    } else {
      void runFastRemoteStreamLoop();
    }

    let resizeRafPending = false;
    const onWinResize = () => {
      if (resizeRafPending || viewerClosed) return;
      resizeRafPending = true;
      requestAnimationFrame(() => {
        resizeRafPending = false;
        if (!viewerClosed) applyTransform();
      });
    };
    window.addEventListener("resize", onWinResize, { passive: true });

    const stageTimer = setInterval(() => {
      if (!overlay.isConnected) {
        viewerClosed = true;
        clearInterval(stageTimer);
        restoreUnderlyingDomVisibility();
        window.removeEventListener("mousemove", onWinMouseMove);
        window.removeEventListener("mouseup", onWinMouseUp);
        window.removeEventListener("resize", onWinResize);
        window.removeEventListener("keydown", onStageKeyDown);
        return;
      }
      badgeTick += 1;
      if (badgeTick % 3 === 0) {
        updateNetBadge();
      }
      if (!isSelfStream) {
        retryRemoteStreamWatchIfStalled();
      }
      if (isSelfStream) {
        emptyNotice.style.display = "none";
        emptyNotice.style.pointerEvents = "none";
        ctx.requestLiveScreenPreviewFrame();
        const cachedLiveScreenDataUrl = ctx.getCachedLiveScreenDataUrl();
        const lastLiveScreenSeq = ctx.getLastLiveScreenSeq();
        if (cachedLiveScreenDataUrl && cachedLiveScreenDataUrl.startsWith("data:image/")) {
          if (lastLiveScreenSeq !== lastRemoteFrameSeq || stageImg.style.display === "none") {
            lastRemoteFrameSeq = lastLiveScreenSeq;
            showFrameDataUrl(cachedLiveScreenDataUrl);
          }
        } else {
          const srcId = String(ctx.getActiveVoiceConnection()?.streamSourceId || "screen_0");
          ctx.drawLiveCaptureCanvasFrame(stageCanvas, srcId, badgeTick, true);
          stageCanvas.style.display = "block";
          stageImg.style.display = "none";
          emptyNotice.style.display = "none";
          recordRenderedFrameFps();
        }
      }
    }, 250);

    const closeStageViewer = (stopStream = false) => {
      viewerClosed = true;
      clearInterval(stageTimer);
      if (pendingBitmap && typeof (/** @type {any} */ (pendingBitmap).close) === "function") {
        try {
          /** @type {any} */ (pendingBitmap).close();
        } catch {}
      }
      pendingBitmap = null;
      restoreUnderlyingDomVisibility();
      window.removeEventListener("mousemove", onWinMouseMove);
      window.removeEventListener("mouseup", onWinMouseUp);
      window.removeEventListener("resize", onWinResize);
      window.removeEventListener("keydown", onStageKeyDown);
      overlay.remove();

      const activeConn = ctx.getActiveVoiceConnection();
      if (isSelfStream) {
        if (stopStream && activeConn) {
          const selfConn = /** @type {any} */ (activeConn);
          selfConn.screenShareOn = false;
          selfConn.hideStreamPreview = true;
          ctx.cancelPipAnimTimer();
          document.getElementById("live-stream-pip-preview")?.remove();
          document.getElementById("live-stream-stage-viewer")?.remove();
          document.getElementById("live-stream-preview-overlay")?.remove();
          document.getElementById("live-screenshare-modal")?.remove();
          document.getElementById("live-screenshare-portal-modal")?.remove();
          document.getElementById("live-stream-settings-menu")?.remove();
          document.getElementById("live-stream-settings-submenu")?.remove();
          ctx.setCachedLiveScreenImg(null);
          ctx.setCachedLiveScreenDataUrl("");
          ctx.setLastLiveScreenSeq(0);
          const sId = cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
          for (const gk of new Set([
            selfConn.guildId,
            ctx.normalizeVoiceGuildKey(selfConn.guildId),
          ])) {
            const gList = ctx.liveVoiceStatesByGuild.get(gk) ?? [];
            for (const v of gList) {
              if (String(v?.user_id ?? v?.userId ?? "") === sId) {
                v.self_stream = false;
                v.selfStream = false;
                v.streaming = false;
              }
            }
          }
          const isCall =
            !selfConn.guildId ||
            selfConn.guildId === HOME_GUILD_ID ||
            selfConn.guildId === "@me";
          const stopKey =
            selfConn.activeStreamKey ||
            /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ ||
            (isCall
              ? `call:${selfConn.channelId}:${sId}`
              : `guild:${selfConn.guildId}:${selfConn.channelId}:${sId}`);
          void ctx.invoke("voice_stream_stop", {
            stream_key: stopKey,
            streamKey: stopKey,
          }).catch(() => {});
          selfConn.activeStreamKey = null;
          /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ = null;
          ctx.playVoiceUiSound("stream_off");
          ctx.syncVoiceMuteStateWithBackend();
          ctx.renderLiveVoicePanel();
          ctx.tree.refreshView();
        } else if (!stopStream) {
          if (activeConn) {
            /** @type {any} */ (activeConn).hideStreamPreview = false;
          }
          ctx.syncFloatingStreamPipPreview();
        }
      } else {
        ctx.playVoiceUiSound("stream_user_left");
        void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "stream_stop_watch",
          deviceId: streamKey,
        }).catch(() => {});
      }
    };

    /** @param {KeyboardEvent} ev */
    const onStageKeyDown = (ev) => {
      if (ev.key === "Escape") {
        ev.preventDefault?.();
        if (isExpandedFullscreen) {
          isExpandedFullscreen = false;
          overlay.style.cssText = normalOverlayCss;
          overlay.setAttribute("data-fullscreen", "false");
          fullscreenBtn.textContent = "⛶ На весь экран";
          syncUnderlyingDomVisibility();
          applyTransform();
        } else {
          closeStageViewer(false);
        }
      } else if (
        !ev.ctrlKey &&
        !ev.metaKey &&
        !ev.altKey &&
        (ev.key === "f" || ev.key === "F" || ev.key === "а" || ev.key === "А" || ev.key === "F11")
      ) {
        const targetTag = String(/** @type {any} */ (ev.target)?.tagName || "").toUpperCase();
        const targetType = String(/** @type {any} */ (ev.target)?.type || "").toLowerCase();
        if (
          targetTag === "TEXTAREA" ||
          Boolean(/** @type {any} */ (ev.target)?.isContentEditable) ||
          (targetTag === "INPUT" && targetType !== "range" && targetType !== "button" && targetType !== "checkbox")
        ) {
          return;
        }
        ev.preventDefault?.();
        const prevExpanded = isExpandedFullscreen;
        if (typeof fullscreenBtn.click === "function") {
          fullscreenBtn.click();
        }
        if (isExpandedFullscreen === prevExpanded) {
          toggleStageFullscreen();
        }
      } else if (
        !ev.ctrlKey &&
        !ev.metaKey &&
        !ev.altKey &&
        (ev.key === "m" || ev.key === "M" || ev.key === "ь" || ev.key === "Ь")
      ) {
        const targetTag = String(/** @type {any} */ (ev.target)?.tagName || "").toUpperCase();
        const targetType = String(/** @type {any} */ (ev.target)?.type || "").toLowerCase();
        if (
          targetTag === "TEXTAREA" ||
          Boolean(/** @type {any} */ (ev.target)?.isContentEditable) ||
          (targetTag === "INPUT" && targetType !== "range" && targetType !== "button" && targetType !== "checkbox")
        ) {
          return;
        }
        muteStreamBtn.click();
      }
    };
    window.addEventListener("keydown", onStageKeyDown);
  }

  return {
    openLiveStreamStageViewer,
  };
}
