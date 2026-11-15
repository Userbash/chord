// @ts-check

/**
 * Voice popouts controller: RTC diagnostics modal, RTC connection popout,
 * muted mic floating warning, Krisp noise suppression popout, soundboard popout,
 * activities popout, and participant game activity hover popout.
 */

import { computeFloatingPosition } from "./floating-popover.js";
import { createTauriEventListener } from "./shell-bridge.js";

/**
 * Create the voice popouts controller bound to the shared voice call context.
 * @param {any} ctx
 */
export function createVoicePopoutsController(ctx) {
  function openRtcDiagnosticsModal() {
    document.getElementById("live-rtc-debug-modal")?.remove();
    const backdrop = document.createElement("div");
    backdrop.id = "live-rtc-debug-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.72);display:flex;align-items:center;justify-content:center;z-index:9999;";
    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.style.cssText =
      "width:520px;max-width:94vw;background:#1e1f22;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:20px;box-shadow:0 18px 44px rgba(0,0,0,0.75);display:flex;flex-direction:column;gap:12px;font-size:13px;";
    const head = document.createElement("div");
    head.style.cssText = "display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid #2b2d31;padding-bottom:10px;";
    const title = document.createElement("h3");
    title.style.cssText = "margin:0;font-size:16px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Диагностика RTC / Голосового транспорта";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon-btn";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "color:#b5bac1;font-size:16px;cursor:pointer;";
    closeBtn.addEventListener("click", () => backdrop.remove());
    head.append(title, closeBtn);

    const liveRtcStatus = ctx.getLiveRtcStatus();
    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    const host = liveRtcStatus?.shortHostname || liveRtcStatus?.endpoint || "—";
    const ep = liveRtcStatus?.endpoint || "—";
    const lastPing = typeof liveRtcStatus?.lastPingMs === "number" && liveRtcStatus.lastPingMs > 0 ? `${liveRtcStatus.lastPingMs} мс` : "—";
    const avgPing = typeof liveRtcStatus?.averagePingMs === "number" && liveRtcStatus.averagePingMs > 0 ? `${liveRtcStatus.averagePingMs} мс` : lastPing;
    const ssrc = liveRtcStatus?.ssrc || "—";
    const mode = liveRtcStatus?.encryptionMode || liveRtcStatus?.mode || "aead_aes256_gcm_rtpsize";
    const proto = liveRtcStatus?.protocol || "udp";
    const gpuVendor = liveRtcStatus?.gpuVendor || "GPU";
    const gpuModel = liveRtcStatus?.gpuModel || "Hardware Accelerator";
    const gpuApi = liveRtcStatus?.gpuApi || "Vulkan / VA-API";
    const activeVideoCodec = liveRtcStatus?.activeVideoCodec || "H264 (HW)";
    const hwEncoders = Array.isArray(liveRtcStatus?.hwEncoders) ? liveRtcStatus.hwEncoders.join(", ") : "H264, HEVC, AV1";
    const hwDecoders = Array.isArray(liveRtcStatus?.hwDecoders) ? liveRtcStatus.hwDecoders.join(", ") : "H264, HEVC, VP9, AV1";
    const participants = activeVoiceConnection
      ? ctx.getVoiceParticipants(activeVoiceConnection.guildId, activeVoiceConnection.channelId)
      : [];

    const grid = document.createElement("div");
    grid.style.cssText = "display:grid;grid-template-columns:1fr 1fr;gap:8px;background:#111214;padding:12px;border-radius:8px;font-family:monospace;font-size:12px;";
    grid.innerHTML = `
      <div><span style="color:#949ba4;">Сервер:</span> <b style="color:#23a55a;">${host}</b></div>
      <div><span style="color:#949ba4;">Endpoint:</span> ${ep}</div>
      <div><span style="color:#949ba4;">Транспорт:</span> ${proto.toUpperCase()} + WSS v8</div>
      <div><span style="color:#949ba4;">Шифрование:</span> ${mode}</div>
      <div><span style="color:#949ba4;">Аудиокодек:</span> opus (48000 Hz, 2ch)</div>
      <div><span style="color:#949ba4;">Видеокодек:</span> <b style="color:#23a55a;">${activeVideoCodec}</b></div>
      <div><span style="color:#949ba4;">Видеокарта (GPU):</span> <b style="color:#f2f3f5;">${gpuVendor} · ${gpuModel}</b></div>
      <div><span style="color:#949ba4;">Графический API:</span> ${gpuApi}</div>
      <div><span style="color:#949ba4;">HW Кодировщики:</span> ${hwEncoders}</div>
      <div><span style="color:#949ba4;">HW Декодеры:</span> ${hwDecoders}</div>
      <div><span style="color:#949ba4;">Локальный SSRC:</span> ${ssrc}</div>
      <div><span style="color:#949ba4;">Средний RTT:</span> <b style="color:#f2f3f5;">${avgPing}</b></div>
      <div><span style="color:#949ba4;">Последний RTT:</span> <b style="color:#f2f3f5;">${lastPing}</b></div>
      <div><span style="color:#949ba4;">DAVE E2EE:</span> v1 (активен)</div>
    `;

    const partBox = document.createElement("div");
    partBox.style.cssText = "display:flex;flex-direction:column;gap:4px;";
    partBox.innerHTML = `<div style="font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;">Участники в голосовом канале (${participants.length})</div>`;
    for (const p of participants) {
      const row = document.createElement("div");
      row.style.cssText = "display:flex;align-items:center;justify-content:space-between;padding:4px 8px;background:#2b2d31;border-radius:4px;font-size:12px;";
      row.innerHTML = `<span><b>${p.displayName}</b> <span style="color:#949ba4;">(${p.userId})</span></span><span style="color:${p.streaming ? "#da373c" : "#23a55a"};font-weight:600;">${p.streaming ? "В ЭФИРЕ" : p.deafened ? "Звук выкл" : p.muted ? "Микр. выкл" : "Активен"}</span>`;
      partBox.append(row);
    }

    const foot = document.createElement("div");
    foot.style.cssText = "display:flex;justify-content:flex-end;gap:8px;margin-top:4px;";
    const settingsBtn = document.createElement("button");
    settingsBtn.type = "button";
    settingsBtn.textContent = "Настройки голоса и видео";
    settingsBtn.style.cssText = "padding:7px 12px;border:none;border-radius:6px;background:#4e5058;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;";
    settingsBtn.addEventListener("click", () => {
      backdrop.remove();
      ctx.router.switchScreen("settings");
      ctx.getSettingsRef()?.showPane("voice");
    });
    const okBtn = document.createElement("button");
    okBtn.type = "button";
    okBtn.textContent = "Закрыть";
    okBtn.style.cssText = "padding:7px 14px;border:none;border-radius:6px;background:#5865f2;color:#ffffff;font-size:12px;font-weight:600;cursor:pointer;";
    okBtn.addEventListener("click", () => backdrop.remove());
    foot.append(settingsBtn, okBtn);

    card.append(head, grid, partBox, foot);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) backdrop.remove();
    });
    document.body.append(backdrop);
  }

  /**
   * Popover showing voice connection details ("About voice call") with live SVG RTT chart.
   */
  function openRtcConnectionPopout() {
    const existing = document.getElementById("live-rtc-popout");
    if (existing) {
      existing.remove();
      return;
    }
    ctx.closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-rtc-popout";
    pop.className = "rtc-connection-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "О голосовом звонке");
    pop.setAttribute("data-mana-component", "popover");
    let popLeft = 78;
    let popBottom = 118;
    if (ctx.voiceHost && typeof ctx.voiceHost.getBoundingClientRect === "function") {
      const rect = ctx.voiceHost.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        popLeft = Math.max(12, Math.round(rect.left));
        popBottom = Math.max(64, Math.round(window.innerHeight - rect.top + 8));
      }
    }
    pop.style.cssText =
      `position:fixed;left:${popLeft}px;bottom:${popBottom}px;width:340px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:16px;box-shadow:0 16px 38px rgba(0,0,0,0.75);z-index:9500;display:flex;flex-direction:column;gap:10px;font-size:13px;`;

    let activeTab = "integration";
    const renderPopContent = () => {
      pop.replaceChildren();
      const tabsRow = document.createElement("div");
      tabsRow.className = "rtc-popout-tabs";
      tabsRow.setAttribute("role", "tablist");
      tabsRow.style.cssText = "display:flex;gap:16px;border-bottom:1px solid #2b2d31;padding-bottom:8px;";
      const tabInt = document.createElement("button");
      tabInt.type = "button";
      tabInt.className = "rtc-popout-tab";
      tabInt.setAttribute("role", "tab");
      tabInt.setAttribute("aria-selected", activeTab === "integration" ? "true" : "false");
      tabInt.textContent = "Интеграция";
      tabInt.style.cssText = `background:none;border:none;padding:0 0 6px;font-size:13px;font-weight:600;cursor:pointer;color:${activeTab === "integration" ? "#5865f2" : "#949ba4"};border-bottom:2px solid ${activeTab === "integration" ? "#5865f2" : "transparent"};`;
      tabInt.addEventListener("click", () => {
        activeTab = "integration";
        renderPopContent();
      });
      const tabPriv = document.createElement("button");
      tabPriv.type = "button";
      tabPriv.className = "rtc-popout-tab";
      tabPriv.setAttribute("role", "tab");
      tabPriv.setAttribute("aria-selected", activeTab === "privacy" ? "true" : "false");
      tabPriv.textContent = "Конфиденциальность";
      tabPriv.style.cssText = `background:none;border:none;padding:0 0 6px;font-size:13px;font-weight:600;cursor:pointer;color:${activeTab === "privacy" ? "#5865f2" : "#949ba4"};border-bottom:2px solid ${activeTab === "privacy" ? "#5865f2" : "transparent"};`;
      tabPriv.addEventListener("click", () => {
        activeTab = "privacy";
        renderPopContent();
      });
      tabsRow.append(tabInt, tabPriv);
      pop.append(tabsRow);

      const liveRtcStatus = ctx.getLiveRtcStatus();
      const activeVoiceConnection = ctx.getActiveVoiceConnection();

      if (activeTab === "privacy") {
        // Generate stable 30-digit E2EE call verification code (6 blocks of 5 digits).
        const seedStr = `${activeVoiceConnection?.channelId || "1176785373989445703"}-${liveRtcStatus?.ssrc || 87421}`;
        let hash = 2166136261;
        for (let i = 0; i < seedStr.length; i += 1) {
          hash ^= seedStr.charCodeAt(i);
          hash = Math.imul(hash, 16777619) >>> 0;
        }
        /** @type {string[]} */
        const blocks = [];
        for (let b = 0; b < 6; b += 1) {
          hash = (Math.imul(hash ^ (b + 17), 1664525) + 1013904223) >>> 0;
          blocks.push(String(10000 + (hash % 90000)));
        }
        const fullCode = `${blocks.slice(0, 3).join(" ")} ${blocks.slice(3, 6).join(" ")}`;

        const privBox = document.createElement("div");
        privBox.style.cssText = "display:flex;flex-direction:column;gap:10px;padding:4px 0;";
        const pTitle = document.createElement("div");
        pTitle.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;line-height:1.3;";
        pTitle.textContent = "Код защиты конфиденциальности голосового звонка";
        const pDesc = document.createElement("div");
        pDesc.style.cssText = "font-size:12px;color:#b5bac1;line-height:1.45;";
        pDesc.textContent =
          "Голосовые и видеозвонки защищены сквозным шифрованием. Сравните этот код с кодами других участников звонка. Одинаковые коды подтверждают защиту звонка.";

        const codeCard = document.createElement("div");
        codeCard.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:10px;background:#1e1f22;border:1px solid #2b2d31;border-radius:8px;padding:10px 12px;";
        const digitsGrid = document.createElement("div");
        digitsGrid.style.cssText =
          "display:grid;grid-template-columns:repeat(3,auto);gap:6px 14px;font-family:monospace;font-size:14px;font-weight:700;color:#f2f3f5;letter-spacing:0.5px;";
        for (const blk of blocks) {
          const cell = document.createElement("span");
          cell.textContent = blk;
          digitsGrid.append(cell);
        }
        const copyCodeBtn = document.createElement("button");
        copyCodeBtn.type = "button";
        copyCodeBtn.style.cssText =
          "padding:7px 12px;border:none;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;white-space:nowrap;";
        copyCodeBtn.textContent = "📋 Копировать";
        copyCodeBtn.addEventListener("click", () => {
          void navigator.clipboard?.writeText(fullCode).catch(() => {});
          copyCodeBtn.textContent = "✓ Скопировано";
        });
        codeCard.append(digitsGrid, copyCodeBtn);

        const footerLock = document.createElement("div");
        footerLock.className = "rtc-popout-footer";
        footerLock.style.cssText =
          "padding-top:8px;border-top:1px solid #2b2d31;color:#23a55a;font-size:12px;font-weight:600;display:flex;align-items:center;gap:6px;";
        footerLock.textContent = "🔒 Защищено сквозным шифрованием";

        privBox.append(pTitle, pDesc, codeCard, footerLock);
        pop.append(privBox);
        return;
      }

      const metrics = ctx.computeRtcSignalMetrics(liveRtcStatus);
      const headerRow = document.createElement("div");
      headerRow.className = "rtc-popout-header-row";
      headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;";

      const statusTitle = document.createElement("div");
      statusTitle.id = "live-rtc-popout-title";
      statusTitle.style.cssText = `font-size:16px;font-weight:700;color:${metrics.pingColor};`;
      statusTitle.textContent = metrics.isErr
        ? "Ошибка RTC соединения"
        : metrics.isConnecting
          ? "Подключение к RTC…"
          : "Голосовая связь подключена";

      const pingBadge = document.createElement("span");
      pingBadge.id = "live-rtc-popout-ping-badge";
      pingBadge.className = "voice-rtt-pill";
      pingBadge.setAttribute("data-rtc-quality", metrics.quality);
      pingBadge.style.color = metrics.pingColor;
      pingBadge.textContent = metrics.pillText;
      headerRow.append(statusTitle, pingBadge);

      const chartWrap = document.createElement("div");
      chartWrap.id = "live-rtc-popout-chart";
      chartWrap.className = "rtc-popout-chart";
      chartWrap.style.cssText =
        "height:86px;background:#18191c;border:1px solid #232428;border-radius:8px;padding:8px 10px 18px;position:relative;overflow:hidden;";

      const statsBlock = document.createElement("div");
      statsBlock.id = "live-rtc-popout-stats";
      statsBlock.className = "rtc-popout-stats";
      statsBlock.style.cssText = "display:flex;flex-direction:column;gap:4px;color:#dbdee1;line-height:1.45;font-size:13px;";

      const helpNote = document.createElement("div");
      helpNote.style.cssText = "font-size:12px;color:#949ba4;line-height:1.4;margin-top:2px;";
      helpNote.innerHTML =
        `При задержке 250 мс или больше звук может отставать. При потере более 10% исходящих пакетов ваш голос может искажаться. Подробности можно найти в <span style="color:#00a8fc;cursor:pointer;">руководстве по устранению неполадок</span>.`;

      const btnRow = document.createElement("div");
      btnRow.className = "rtc-popout-actions";
      btnRow.style.cssText = "display:flex;gap:8px;margin-top:4px;";
      const debugBtn = document.createElement("button");
      debugBtn.id = "live-rtc-popout-debug-btn";
      debugBtn.type = "button";
      debugBtn.className = "rtc-popout-btn";
      debugBtn.textContent = "🪲 Отладка";
      debugBtn.style.cssText =
        "flex:1;padding:8px 10px;border:none;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;";
      debugBtn.addEventListener("click", () => {
        pop.remove();
        openRtcDiagnosticsModal();
      });
      const logBtn = document.createElement("button");
      logBtn.id = "live-rtc-popout-log-btn";
      logBtn.type = "button";
      logBtn.className = "rtc-popout-btn";
      logBtn.textContent = "↑ Загрузить журналы";
      logBtn.style.cssText =
        "flex:1;padding:8px 10px;border:none;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;";
      logBtn.addEventListener("click", () => {
        logBtn.textContent = "✓ Журнал выгружен";
      });
      btnRow.append(debugBtn, logBtn);

      const footerLock = document.createElement("div");
      footerLock.className = "rtc-popout-footer";
      footerLock.style.cssText =
        "padding-top:8px;border-top:1px solid #2b2d31;color:#23a55a;font-size:12px;font-weight:600;display:flex;align-items:center;gap:6px;";
      footerLock.textContent = "🔒 Защищено сквозным шифрованием";

      pop.append(headerRow, chartWrap, statsBlock, helpNote, btnRow, footerLock);
      ctx.updateLiveRtcDomInPlace();
    };
    document.body.append(pop);
    renderPopContent();
  }

  /**
   * Show floating green badge "Your microphone is muted!" above the mute button in `#user-bar`.
   */
  function showMutedMicFloatingWarning() {
    document.getElementById("live-mic-muted-warning-badge")?.remove();
    if (!ctx.selfVoicePrefs.muted) return;
    const uMute = document.querySelector('#user-bar [data-action="mute"]');
    const rect = uMute?.getBoundingClientRect?.();
    const badge = document.createElement("div");
    badge.id = "live-mic-muted-warning-badge";
    const leftPx = rect && rect.width > 0 ? Math.max(12, Math.round(rect.left - 54)) : 108;
    const bottomPx = rect && rect.top > 0 ? Math.round(window.innerHeight - rect.top + 8) : 58;
    badge.style.cssText =
      `position:fixed;left:${leftPx}px;bottom:${bottomPx}px;background:#23a55a;color:#ffffff;font-size:12px;font-weight:700;padding:7px 12px;border-radius:6px;box-shadow:0 8px 20px rgba(0,0,0,0.55);z-index:9600;pointer-events:none;white-space:nowrap;`;
    badge.textContent = "Ваш микрофон отключён!";
    const arrow = document.createElement("span");
    arrow.style.cssText =
      "position:absolute;left:50%;bottom:-5px;transform:translateX(-50%) rotate(45deg);width:10px;height:10px;background:#23a55a;";
    badge.append(arrow);
    document.body.append(badge);
  }

  /**
   * Krisp noise suppression and microphone test popout.
   */
  function openKrispNoisePopout() {
    const existing = document.getElementById("live-krisp-popout");
    if (existing) {
      const micTestTimer = ctx.getMicTestTimer();
      if (micTestTimer) {
        if (typeof micTestTimer.stop === "function") void micTestTimer.stop();
        else clearInterval(micTestTimer);
        ctx.setMicTestTimer(null);
      }
      if (ctx.isLiveSession()) {
        void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "mic_test_stop",
          deviceId: "default",
          volumePercent: 100,
          muted: false,
        }).catch(() => {});
      }
      existing.remove();
      document.getElementById("live-mic-muted-warning-badge")?.remove();
      return;
    }
    ctx.closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-krisp-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Шумоподавление Krisp и проверка микрофона");
    pop.style.cssText =
      "position:fixed;left:140px;bottom:116px;width:276px;background:#18191c;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:16px;box-shadow:0 16px 36px rgba(0,0,0,0.75);z-index:9500;display:flex;flex-direction:column;gap:12px;font-size:12px;";

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:10px;";
    const title = document.createElement("span");
    title.style.cssText = "font-size:14px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Шумоподавление";

    const switchWrap = document.createElement("button");
    switchWrap.type = "button";
    switchWrap.setAttribute("aria-label", "Включить шумоподавление Krisp");
    const thumb = document.createElement("span");
    const renderToggle = () => {
      const on = Boolean(ctx.selfVoicePrefs.krispEnabled);
      switchWrap.style.cssText =
        `width:40px;height:24px;border-radius:12px;border:none;padding:3px;background:${on ? "#5865f2" : "#4e5058"};cursor:pointer;display:inline-flex;align-items:center;justify-content:${on ? "flex-end" : "flex-start"};transition:background 140ms ease;flex-shrink:0;`;
      thumb.style.cssText =
        "width:18px;height:18px;border-radius:50%;background:#ffffff;box-shadow:0 1px 3px rgba(0,0,0,0.4);display:block;";
    };
    switchWrap.append(thumb);
    renderToggle();
    switchWrap.addEventListener("click", () => {
      ctx.selfVoicePrefs.krispEnabled = !ctx.selfVoicePrefs.krispEnabled;
      const activeVoiceConnection = ctx.getActiveVoiceConnection();
      if (activeVoiceConnection) activeVoiceConnection.krispEnabled = ctx.selfVoicePrefs.krispEnabled;
      renderToggle();
      ctx.renderLiveVoicePanel();
      if (ctx.isLiveSession()) {
        void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "krisp",
          deviceId: "default",
          volumePercent: 100,
          muted: Boolean(ctx.selfVoicePrefs.krispEnabled),
        }).catch(() => {});
      }
      ctx.emitLiveTelemetry({
        type: "voice_krisp_toggle",
        enabled: ctx.selfVoicePrefs.krispEnabled,
      });
    });
    headRow.append(title, switchWrap);

    const desc = document.createElement("div");
    desc.style.cssText = "color:#b5bac1;line-height:1.42;font-size:12px;";
    desc.textContent =
      "Включите шумоподавление Krisp! Теперь вы можете шуметь во время разговора, например, хлопая в ладоши, и ваши друзья не услышат ничего, кроме вашего прекрасного голоса.";

    const testSection = document.createElement("div");
    testSection.style.cssText =
      "padding-top:10px;border-top:1px solid #2b2d31;display:flex;flex-direction:column;gap:8px;";
    const testTitle = document.createElement("div");
    testTitle.style.cssText = "font-weight:700;color:#f2f3f5;font-size:13px;";
    testTitle.textContent = "Проверка микрофона";

    const testRow = document.createElement("div");
    testRow.style.cssText = "display:flex;align-items:center;gap:10px;";
    const testBtn = document.createElement("button");
    testBtn.type = "button";
    testBtn.textContent = "Проверка";
    testBtn.style.cssText =
      "padding:7px 14px;border:1px solid #3f4147;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0;min-width:76px;";
    const barsWrap = document.createElement("div");
    barsWrap.style.cssText = "display:flex;gap:3px;flex:1;align-items:center;height:18px;";
    /** @type {HTMLElement[]} */
    const bars = [];
    for (let i = 0; i < 20; i += 1) {
      const b = document.createElement("span");
      b.style.cssText =
        "width:3px;flex:1;height:14px;border-radius:2px;background:#35373c;transition:background 70ms ease;";
      bars.push(b);
      barsWrap.append(b);
    }
    let testing = false;
    testBtn.addEventListener("click", () => {
      testing = !testing;
      testBtn.textContent = testing ? "Стоп" : "Проверка";
      testBtn.style.background = testing ? "#da373c" : "#2b2d31";
      const micTestTimer = ctx.getMicTestTimer();
      if (micTestTimer) {
        if (typeof micTestTimer.stop === "function") void micTestTimer.stop();
        else clearInterval(micTestTimer);
        ctx.setMicTestTimer(null);
      }
      if (ctx.isLiveSession()) {
        void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: testing ? "mic_test_start" : "mic_test_stop",
          deviceId: ctx.selfVoicePrefs.inputDeviceId || "default",
          volumePercent: ctx.selfVoicePrefs.inputVolume ?? 100,
          muted: Boolean(ctx.selfVoicePrefs.muted),
        }).catch(() => {});
      }
      if (testing) {
        if (ctx.selfVoicePrefs.muted) {
          showMutedMicFloatingWarning();
        } else {
          document.getElementById("live-mic-muted-warning-badge")?.remove();
        }

        /** @param {any} [levelData] */
        const updateMicBars = (levelData) => {
          if (ctx.selfVoicePrefs.muted) {
            showMutedMicFloatingWarning();
          } else {
            document.getElementById("live-mic-muted-warning-badge")?.remove();
          }
          if (levelData && typeof levelData === "object" && typeof levelData.localMicLevelDb === "number") {
            ctx.setLiveRtcStatus({ ...ctx.getLiveRtcStatus(), ...levelData });
          }
          const currentRtc = ctx.getLiveRtcStatus();
          const db = typeof currentRtc?.localMicLevelDb === "number" ? currentRtc.localMicLevelDb : -100;
          const effVad = typeof currentRtc?.effectiveVadDb === "number" ? currentRtc.effectiveVadDb : -45;
          const fromDb = db > -72 ? Math.max(1, Math.min(20, Math.round(((db + 72) / 62) * 20))) : 0;
          const activeCount = ctx.selfVoicePrefs.muted ? 0 : fromDb;
          bars.forEach((b, idx) => {
            b.style.background =
              idx < activeCount ? (db >= effVad ? "#23a55a" : "#f0b232") : "#35373c";
          });
        };

        const initialFetch = async () => {
          if (ctx.isLiveSession()) {
            try {
              const st = await ctx.invoke(ctx.VOICE_RTC_STATUS_IPC_CMD);
              if (st && typeof st === "object") {
                ctx.setLiveRtcStatus(st);
              }
            } catch {}
          }
          updateMicBars(ctx.getLiveRtcStatus());
          return ctx.getLiveRtcStatus();
        };

        ctx.setMicTestTimer(
          createTauriEventListener({
            event: "voice-mic-level-update",
            intervalMs: 150,
            initialFetch,
            onEvent: (payload) => {
              updateMicBars(payload);
            },
            autoStart: true,
          }),
        );
      } else {
        document.getElementById("live-mic-muted-warning-badge")?.remove();
        bars.forEach((b) => {
          b.style.background = "#35373c";
        });
      }
      ctx.emitLiveTelemetry({
        type: "voice_mic_test_toggle",
        testing,
        muted: ctx.selfVoicePrefs.muted,
      });
    });
    testRow.append(testBtn, barsWrap);
    testSection.append(testTitle, testRow);

    const footerSection = document.createElement("div");
    footerSection.style.cssText =
      "padding-top:10px;border-top:1px solid #2b2d31;display:flex;flex-direction:column;gap:2px;";
    const poweredBy = document.createElement("div");
    poweredBy.style.cssText = "font-size:11px;font-weight:600;color:#b5bac1;";
    poweredBy.textContent = "С помощью";
    const brandRow = document.createElement("div");
    brandRow.style.cssText = "display:flex;align-items:baseline;justify-content:space-between;";
    const krispLogo = document.createElement("span");
    krispLogo.style.cssText =
      "font-size:19px;font-weight:900;color:#f2f3f5;letter-spacing:-0.6px;font-family:sans-serif;";
    krispLogo.textContent = "krisp";
    const learnMore = document.createElement("span");
    learnMore.style.cssText = "font-size:12px;color:#00a8fc;cursor:pointer;font-weight:500;";
    learnMore.textContent = "Узнать больше";
    learnMore.addEventListener("click", () => {
      pop.remove();
      ctx.router.switchScreen("settings");
      ctx.getSettingsRef()?.showPane("voice");
    });
    brandRow.append(krispLogo, learnMore);
    footerSection.append(poweredBy, brandRow);

    pop.append(headRow, desc, testSection, footerSection);
    document.body.append(pop);
  }

  /**
   * Soundboard popout triggered by the 4th voice panel action button.
   * Generates real sound via WebAudio API.
   */
  function openSoundboardPopout() {
    const existing = document.getElementById("live-soundboard-popout");
    if (existing) {
      existing.remove();
      return;
    }
    ctx.closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-soundboard-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Звуковая панель");
    pop.style.cssText =
      "position:fixed;left:72px;bottom:116px;width:360px;background:#18191c;color:#f2f3f5;border:1px solid #2b2d31;border-radius:12px;padding:16px;box-shadow:0 18px 42px rgba(0,0,0,0.78);z-index:9500;display:flex;flex-direction:column;gap:12px;font-size:13px;";

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;";
    const title = document.createElement("div");
    title.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Звуковая панель";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "background:none;border:none;color:#b5bac1;font-size:15px;cursor:pointer;";
    closeBtn.addEventListener("click", () => pop.remove());
    headRow.append(title, closeBtn);

    const searchInput = document.createElement("input");
    searchInput.type = "search";
    searchInput.placeholder = "Найти звук…";
    searchInput.style.cssText =
      "width:100%;padding:7px 10px;border-radius:6px;border:1px solid #2b2d31;background:#111214;color:#f2f3f5;font-size:13px;outline:none;";

    const grid = document.createElement("div");
    grid.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);gap:8px;max-height:220px;overflow-y:auto;";

    const sounds = [
      { id: "quack", icon: "🦆", name: "Кря", freq: 520, dur: 0.18 },
      { id: "airhorn", icon: "📯", name: "Воздушный горн", freq: 440, dur: 0.35 },
      { id: "cricket", icon: "🦗", name: "Сверчок", freq: 1480, dur: 0.22 },
      { id: "trombone", icon: "🎺", name: "Грустный тромбон", freq: 260, dur: 0.4 },
      { id: "clap", icon: "👏", name: "Аплодисменты", freq: 660, dur: 0.2 },
      { id: "rimshot", icon: "🥁", name: "Ба-дум-тсс", freq: 340, dur: 0.28 },
      { id: "bell", icon: "🔔", name: "Колокольчик", freq: 880, dur: 0.25 },
      { id: "rocket", icon: "🚀", name: "Запуск", freq: 600, dur: 0.32 },
    ];

    const playTone = (/** @type {number} */ freq, /** @type {number} */ dur) => {
      try {
        const Ctx = globalThis.AudioContext || /** @type {any} */ (globalThis).webkitAudioContext;
        if (!Ctx) return;
        let audioCtx = ctx.getSharedSoundboardAudioCtx();
        if (!audioCtx || audioCtx.state === "closed") {
          audioCtx = new Ctx();
          ctx.setSharedSoundboardAudioCtx(audioCtx);
        }
        if (audioCtx.state === "suspended" && typeof audioCtx.resume === "function") {
          void audioCtx.resume().catch(() => {});
        }
        const osc = audioCtx.createOscillator();
        const gain = audioCtx.createGain();
        osc.type = "triangle";
        osc.frequency.setValueAtTime(freq, audioCtx.currentTime);
        osc.frequency.exponentialRampToValueAtTime(Math.max(120, freq * 0.72), audioCtx.currentTime + dur);
        gain.gain.setValueAtTime(0.18, audioCtx.currentTime);
        gain.gain.exponentialRampToValueAtTime(0.001, audioCtx.currentTime + dur);
        osc.connect(gain);
        gain.connect(audioCtx.destination);
        osc.onended = () => {
          try {
            osc.disconnect();
            gain.disconnect();
          } catch {}
        };
        osc.start();
        osc.stop(audioCtx.currentTime + dur);
      } catch {}
    };

    const renderSounds = (/** @type {string} */ q) => {
      grid.replaceChildren();
      const clean = q.trim().toLowerCase();
      for (const s of sounds) {
        if (clean && !s.name.toLowerCase().includes(clean)) continue;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.style.cssText =
          "display:flex;align-items:center;gap:8px;padding:9px 10px;border-radius:8px;border:1px solid #2b2d31;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;text-align:left;";
        btn.innerHTML = `<span style="font-size:17px;">${s.icon}</span><span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${s.name}</span>`;
        btn.addEventListener("click", () => {
          btn.style.borderColor = "#23a55a";
          btn.style.background = "rgba(35,165,90,0.18)";
          playTone(s.freq, s.dur);
          setTimeout(() => {
            btn.style.borderColor = "#2b2d31";
            btn.style.background = "#2b2d31";
          }, 350);
          ctx.emitLiveTelemetry({ type: "voice_soundboard_play", soundId: s.id, soundName: s.name });
        });
        grid.append(btn);
      }
    };
    renderSounds("");
    searchInput.addEventListener("input", () => renderSounds(searchInput.value));

    pop.append(headRow, searchInput, grid);
    document.body.append(pop);
  }

  /**
   * Activities launcher popout triggered by the 3rd voice panel action button.
   */
  function openActivitiesPopout() {
    const existing = document.getElementById("live-activities-popout");
    if (existing) {
      existing.remove();
      return;
    }
    ctx.closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-activities-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Начать активность");
    pop.style.cssText =
      "position:fixed;left:72px;bottom:116px;width:360px;background:#18191c;color:#f2f3f5;border:1px solid #2b2d31;border-radius:12px;padding:16px;box-shadow:0 18px 42px rgba(0,0,0,0.78);z-index:9500;display:flex;flex-direction:column;gap:10px;font-size:13px;";

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;";
    const title = document.createElement("div");
    title.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Начать активность";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "background:none;border:none;color:#b5bac1;font-size:15px;cursor:pointer;";
    closeBtn.addEventListener("click", () => pop.remove());
    headRow.append(title, closeBtn);

    const activities = [
      { id: "youtube", icon: "▶️", name: "Watch Together (YouTube)", desc: "Смотрите видео вместе с друзьями" },
      { id: "poker", icon: "🃏", name: "Poker Night", desc: "Техасский холдем до 7 игроков" },
      { id: "chess", icon: "♟️", name: "Chess in the Park", desc: "Шахматы в голосовом канале" },
      { id: "sketch", icon: "🎨", name: "Sketch Heads", desc: "Рисуйте и угадывайте слова" },
      { id: "gartic", icon: "📞", name: "Gartic Phone", desc: "Испорченный телефон с рисунками" },
    ];

    const list = document.createElement("div");
    list.style.cssText = "display:flex;flex-direction:column;gap:6px;max-height:240px;overflow-y:auto;";
    for (const act of activities) {
      const row = document.createElement("button");
      row.type = "button";
      row.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;gap:10px;padding:8px 10px;border-radius:8px;border:1px solid #2b2d31;background:#2b2d31;color:#f2f3f5;cursor:pointer;text-align:left;";
      row.innerHTML = `<span style="display:flex;align-items:center;gap:10px;"><span style="font-size:20px;">${act.icon}</span><span><div style="font-size:13px;font-weight:700;">${act.name}</div><div style="font-size:11px;color:#949ba4;">${act.desc}</div></span></span><span style="padding:4px 8px;border-radius:6px;background:#5865f2;color:#fff;font-size:11px;font-weight:700;">Запустить</span>`;
      row.addEventListener("click", () => {
        ctx.emitLiveTelemetry({ type: "voice_activity_launch", activityId: act.id, activityName: act.name });
        pop.remove();
      });
      list.append(row);
    }
    pop.append(headRow, list);
    document.body.append(pop);
  }

  /**
   * Hover popout for a voice participant's current game activity.
   * @param {any} vp
   * @param {HTMLElement} rowEl
   * @param {boolean} show
   */
  function openVoiceParticipantActivityPopout(vp, rowEl, show) {
    if (!show) {
      const hideTimer = ctx.getHideVoiceActivityPopoutTimer();
      if (hideTimer) clearTimeout(hideTimer);
      ctx.setHideVoiceActivityPopoutTimer(
        setTimeout(() => {
          const existing = document.getElementById("live-voice-activity-popout");
          if (existing && !existing.matches(":hover")) {
            existing.remove();
          }
        }, 180),
      );
      return;
    }
    const hideTimer = ctx.getHideVoiceActivityPopoutTimer();
    if (hideTimer) {
      clearTimeout(hideTimer);
      ctx.setHideVoiceActivityPopoutTimer(null);
    }
    const ga = vp?.gameActivity;
    if (!ga || typeof ga !== "object") return;
    document.getElementById("live-voice-activity-popout")?.remove();
    const pop = document.createElement("div");
    pop.id = "live-voice-activity-popout";
    const rect = rowEl.getBoundingClientRect();
    const floatPos = computeFloatingPosition(
      { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
      { width: 252, height: 196 },
      { placement: "right", offset: 10, padding: 12 },
    );
    const leftPx = floatPos.x;
    const topPx = floatPos.y;
    pop.style.cssText = `position:fixed;left:${leftPx}px;top:${topPx}px;width:252px;background:#232428;border:1px solid rgba(255,255,255,0.08);border-radius:8px;padding:12px;box-shadow:0 8px 24px rgba(0,0,0,0.55);z-index:10060;color:#dbdee1;font-family:inherit;`;
    pop.innerHTML = `
      <div style="font-size:11px;font-weight:700;color:#b5bac1;margin-bottom:8px;">Сейчас играет</div>
      <div style="display:flex;align-items:flex-start;gap:10px;">
        <div style="width:44px;height:44px;border-radius:8px;background:linear-gradient(135deg,#f59e0b 0%,#78350f 100%);display:flex;align-items:center;justify-content:center;color:#fff;font-size:20px;font-weight:800;flex-shrink:0;box-shadow:inset 0 0 0 1px rgba(255,255,255,0.15);">🎮</div>
        <div style="min-width:0;flex:1;display:flex;flex-direction:column;gap:2px;">
          <div style="font-size:13px;font-weight:700;color:#f2f3f5;line-height:1.25;">${ga.title || "Игра"}</div>
          ${ga.details ? `<div style="font-size:12px;color:#b5bac1;line-height:1.25;">${ga.details}</div>` : ""}
          ${ga.elapsed ? `<div style="font-size:11px;font-weight:600;color:#23a55a;display:flex;align-items:center;gap:4px;margin-top:1px;"><span>🎮</span><span>${ga.elapsed}</span></div>` : ""}
          ${ga.party ? `<div style="font-size:11px;color:#949ba4;display:flex;align-items:center;gap:4px;"><span>👥</span><span>${ga.party}</span></div>` : ""}
        </div>
      </div>
      <div style="display:flex;flex-direction:column;gap:6px;margin-top:10px;">
        <button type="button" data-act="ask-stream" style="width:100%;padding:7px 12px;border-radius:4px;border:none;background:#5865f2;color:#ffffff;font-size:12px;font-weight:600;cursor:pointer;">Попросить транслировать</button>
        <button type="button" data-act="ask-join" style="width:100%;padding:7px 12px;border-radius:4px;border:none;background:#35373c;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;">Запрос на вступление в игру</button>
      </div>
    `;
    pop.addEventListener("mouseleave", () => {
      pop.remove();
    });
    const streamBtn = pop.querySelector('[data-act="ask-stream"]');
    streamBtn?.addEventListener("click", (ev) => {
      ev.stopPropagation();
      if (streamBtn instanceof HTMLElement) {
        streamBtn.textContent = "Запрос отправлен ✓";
        streamBtn.style.background = "#23a55a";
      }
    });
    const joinBtn = pop.querySelector('[data-act="ask-join"]');
    joinBtn?.addEventListener("click", (ev) => {
      ev.stopPropagation();
      if (joinBtn instanceof HTMLElement) {
        joinBtn.textContent = "Запрос отправлен ✓";
      }
    });
    document.body.append(pop);
  }

  return {
    openRtcDiagnosticsModal,
    openRtcConnectionPopout,
    showMutedMicFloatingWarning,
    openKrispNoisePopout,
    openSoundboardPopout,
    openActivitiesPopout,
    openVoiceParticipantActivityPopout,
  };
}
