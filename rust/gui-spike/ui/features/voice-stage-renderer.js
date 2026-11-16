// @ts-check
/**
 * Voice panel, user bar voice buttons, guild voice stage preview, and in-place RTC DOM updates.
 * Extracted from `app.js` as part of RW51-EPIC2-T2.
 * @module ui/features/voice-stage-renderer
 */

import { renderVoiceRtcPhaseDom } from "../modules/voice-rtc-panel.js";
import { HOME_GUILD_ID } from "./dm-friends.js";
import {
  renderVoiceStagePreviewBanner,
  removeVoiceStagePreviewBanner,
} from "./voice-stage-preview.js";

const SVG_KRISP_BARS = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="4" y1="10" x2="4" y2="14"/><line x1="8" y1="7" x2="8" y2="17"/><line x1="12" y1="4" x2="12" y2="20"/><line x1="16" y1="7" x2="16" y2="17"/><line x1="20" y1="10" x2="20" y2="14"/></svg>`;
const SVG_PHONE_DISCONNECT = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08a.956.956 0 0 1-.29-.7c0-.28.11-.53.29-.71C3.34 8.78 7.46 7 12 7s8.66 1.78 11.71 4.67c.18.18.29.43.29.71 0 .28-.11.53-.29.71l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28a11.27 11.27 0 0 0-2.67-1.85.996.996 0 0 1-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z"/></svg>`;
const SVG_CAMERA_ON = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z"/></svg>`;
const SVG_CAMERA_OFF = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z" opacity="0.65"/><line x1="3" y1="3" x2="21" y2="21" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/></svg>`;
const SVG_SCREENSHARE = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M4 4h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-6v2h3a1 1 0 1 1 0 2H7a1 1 0 1 1 0-2h3v-2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zm8.5 3.5v2.2c-2.8.3-4.5 1.8-5.2 4.3 1.4-1.3 3-1.9 5.2-1.9v2.4l4.2-3.5-4.2-3.5z"/></svg>`;
const SVG_ACTIVITY = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="3.5" y="3.5" width="7" height="7" rx="2" transform="rotate(-10 7 7)"/><path d="M17 3.5l4.2 7.2H12.8L17 3.5z"/><circle cx="7" cy="17" r="3.6"/><path d="M17 12.8l1.3 2.9 2.9 1.3-2.9 1.3-1.3 2.9-1.3-2.9-2.9-1.3 2.9-1.3 1.3-2.9z"/></svg>`;
const SVG_SOUNDBOARD = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 18L13 6l5 5-12 9-2-2z" fill="currentColor"/><line x1="16" y1="4" x2="17.5" y2="2"/><line x1="19" y1="7" x2="21.5" y2="6"/><line x1="19.5" y1="11" x2="22" y2="11.5"/></svg>`;

/**
 * Create the voice stage and panel DOM renderer bound to the shared voice context.
 * @param {any} ctx
 */
export function createVoiceStageRenderer(ctx) {
  /**
   * Updates RTC status badges, ping pills, signal icons, speaking rings, and channel timers in place.
   */
  function updateLiveRtcDomInPlace() {
    const appShellNode = document.getElementById("app-shell");
    if (appShellNode && appShellNode.style.visibility === "hidden") {
      return;
    }
    const liveRtcStatus = ctx.getLiveRtcStatus();
    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    const badge = document.getElementById("live-voice-status-badge");
    const rttPill = document.getElementById("live-voice-rtt-pill");
    const metrics = ctx.computeRtcSignalMetrics(liveRtcStatus);
    const {
      phaseView,
      isErr,
      isConnecting,
      hasMeasuredPing,
      effectiveLast,
      effectiveAvg,
      lossPct,
      lossTxt,
      quality,
      pingColor,
      signalSvg,
      pillText,
    } = metrics;
    const voiceHost =
      ctx.getVoiceHost?.() ??
      ctx.voiceHost ??
      (typeof document !== "undefined" ? document.getElementById("voice-panel") : null);
    if (voiceHost && activeVoiceConnection) {
      renderVoiceRtcPhaseDom(voiceHost, liveRtcStatus, {
        doc: document,
        onRetry: ctx.retryActiveVoiceConnection,
      });
    }
    const isAfkCall = Boolean(
      phaseView.phase === "afk" ||
        liveRtcStatus?.isAfk ||
        activeVoiceConnection?.isAfk,
    );
    if (isAfkCall) {
      if (badge && badge.getAttribute("data-hovered") !== "true") {
        if (badge.textContent !== "🔕 AFK-канал") badge.textContent = "🔕 AFK-канал";
        if (badge.style.color !== "#949ba4") badge.style.color = "#949ba4";
      }
      if (rttPill) {
        rttPill.style.display = "none";
      }
      const connStatus = document.getElementById("live-rtc-conn-status");
      if (connStatus) {
        connStatus.textContent = "Микрофон и звук отключены сервером";
        connStatus.style.display = "";
        connStatus.style.color = "#949ba4";
      }
    } else {
      if (badge && badge.getAttribute("data-hovered") !== "true") {
        const nextBadgeText =
          phaseView.label ||
          (isErr
            ? "Нет сигнала RTC"
            : isConnecting
              ? "Ожидание ответа сервера Discord..."
              : "Подключено");
        if (badge.textContent !== nextBadgeText) badge.textContent = nextBadgeText;
        if (badge.style.color !== pingColor) badge.style.color = pingColor;
      }
      if (rttPill) {
        rttPill.style.display = "";
        if (rttPill.textContent !== pillText) rttPill.textContent = pillText;
        if (rttPill.getAttribute("data-rtc-quality") !== quality) {
          rttPill.setAttribute("data-rtc-quality", quality);
        }
        rttPill.setAttribute("data-ping-ms", String(effectiveLast));
        rttPill.setAttribute("data-packet-loss", lossTxt);
        if (rttPill.style.color !== pingColor) rttPill.style.color = pingColor;
      }
    }
    const statusBtn = document.querySelector("#voice-panel .voice-status-trigger");
    if (statusBtn instanceof HTMLElement) {
      statusBtn.title = isAfkCall
        ? `Канал: ${activeVoiceConnection?.channelName || "AFK-комната"}\nСервер: ${activeVoiceConnection?.guildName || "Сервер"}\nСтатус: 🔕 AFK-канал (микрофон и звук отключены сервером)`
        : `Канал: ${activeVoiceConnection?.channelName || "Голосовой"}\nСервер: ${activeVoiceConnection?.guildName || "Сервер"}\nПинг: ${effectiveLast} мс (средний ${effectiveAvg} мс)\nПотери пакетов: ${lossTxt}\nРегион: ${liveRtcStatus?.region || "Frankfurt (Europe)"}\nШифрование: DAVE MLS v1 (E2EE)`;
    }
    const sigIcon = document.querySelector("#voice-panel [data-voice-signal-icon]");
    if (sigIcon instanceof HTMLElement) {
      if (sigIcon.getAttribute("data-rtc-quality") !== quality) {
        sigIcon.setAttribute("data-rtc-quality", quality);
        sigIcon.innerHTML = signalSvg;
      }
      if (sigIcon.style.color !== pingColor) {
        sigIcon.style.color = pingColor;
      }
      sigIcon.title = hasMeasuredPing
        ? `Пинг: ${effectiveLast} мс • Потери пакетов: ${lossTxt}`
        : `Статус RTC: ${quality} • Потери пакетов: ${lossTxt}`;
    }
    const popTitle = document.getElementById("live-rtc-popout-title");
    if (popTitle) {
      popTitle.textContent = isErr
        ? phaseView.label || "Ошибка RTC соединения"
        : isConnecting
          ? phaseView.label || "Подключение к RTC…"
          : "Голосовая связь подключена";
      popTitle.style.color = pingColor;
    }
    const popPingBadge = document.getElementById("live-rtc-popout-ping-badge");
    if (popPingBadge) {
      if (popPingBadge.textContent !== pillText) popPingBadge.textContent = pillText;
      if (popPingBadge.getAttribute("data-rtc-quality") !== quality) {
        popPingBadge.setAttribute("data-rtc-quality", quality);
      }
      popPingBadge.style.color = pingColor;
    }
    const popStats = document.getElementById("live-rtc-popout-stats");
    if (popStats) {
      const host = liveRtcStatus?.shortHostname || liveRtcStatus?.endpoint || "finland10153.discord.media";
      const avgTxt = hasMeasuredPing ? `${effectiveAvg} мс` : "—";
      const lastTxt = hasMeasuredPing ? `${effectiveLast} мс` : "—";
      const transportTxt = String(liveRtcStatus?.protocol || liveRtcStatus?.transport || "UDP / SRTP");
      const codecTxt = String(liveRtcStatus?.codec || "Opus 48 kHz");
      const cryptoTxt = String(liveRtcStatus?.encryptionMode || "DAVE MLS / AEAD-AES256-GCM");
      const ssrcTxt = String(liveRtcStatus?.ssrc || 87421);
      const lossColor = lossPct > 10 ? "#f23f43" : lossPct > 5 ? "#f0b232" : "#f2f3f5";
      popStats.innerHTML = `
        <div style="font-weight:700;color:#f2f3f5;font-size:14px;margin-bottom:2px;">${host}</div>
        <div>Средняя задержка: <b style="color:#f2f3f5;">${avgTxt}</b></div>
        <div>Последняя задержка: <b style="color:${pingColor};">${lastTxt}</b></div>
        <div>Потеря исходящих пакетов: <b style="color:${lossColor};">${lossTxt}</b></div>
        <div id="live-rtc-popout-diag" class="rtc-popout-diag-grid">
          <div class="rtc-popout-diag-cell"><span>Транспорт</span><b>${transportTxt}</b></div>
          <div class="rtc-popout-diag-cell"><span>Аудиокодек</span><b>${codecTxt}</b></div>
          <div class="rtc-popout-diag-cell"><span>Шифрование</span><b>${cryptoTxt}</b></div>
          <div class="rtc-popout-diag-cell"><span>SSRC</span><b>${ssrcTxt}</b></div>
        </div>
      `;
    }
    const popChart = document.getElementById("live-rtc-popout-chart");
    if (popChart) {
      const rawHist =
        Array.isArray(liveRtcStatus?.pingHistory) && liveRtcStatus.pingHistory.length > 0
          ? liveRtcStatus.pingHistory
          : [effectiveLast - 1, effectiveLast, effectiveLast + 1, effectiveLast, effectiveLast - 1, effectiveLast];
      const hist = rawHist.length === 1 ? [rawHist[0], rawHist[0], rawHist[0]] : rawHist;
      const yMax = Math.max(400, ...hist);
      const yMid = Math.round(yMax / 2);
      const pts = hist
        .map((/** @type {any} */ v, /** @type {number} */ idx) => {
          const x = Math.round((idx / Math.max(1, hist.length - 1)) * 270);
          const y = Math.round(54 - (Math.min(yMax, Math.max(0, Number(v))) / yMax) * 46);
          return `${x},${Math.max(4, Math.min(54, y))}`;
        })
        .join(" ");
      const now = new Date();
      const fmtTime = (/** @type {number} */ offsetMin) => {
        const d = new Date(now.getTime() - offsetMin * 60_000);
        return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
      };
      popChart.innerHTML = `
        <svg width="276" height="58" viewBox="0 0 300 58" style="display:block;width:100%;height:58px;">
          <line x1="0" y1="8" x2="270" y2="8" stroke="#2b2d31" stroke-width="1" />
          <line x1="0" y1="31" x2="270" y2="31" stroke="#2b2d31" stroke-width="1" />
          <line x1="0" y1="54" x2="270" y2="54" stroke="#2b2d31" stroke-width="1" />
          <polyline fill="none" stroke="${pingColor}" stroke-width="2.2" points="${pts}" />
          <text x="298" y="11" fill="#949ba4" font-size="9" text-anchor="end">${yMax}</text>
          <text x="298" y="34" fill="#949ba4" font-size="9" text-anchor="end">${yMid}</text>
          <text x="298" y="56" fill="#949ba4" font-size="9" text-anchor="end">0</text>
        </svg>
        <div style="display:flex;justify-content:space-between;padding-right:28px;margin-top:2px;font-size:10px;color:#949ba4;">
          <span>${fmtTime(3)}</span>
          <span>${fmtTime(2)}</span>
          <span>${fmtTime(1)}</span>
          <span>${fmtTime(0)}</span>
        </div>
      `;
    }

    const selfId = String(ctx.getLiveSelfUser()?.id || "");
    const currentGid =
      activeVoiceConnection?.guildId ||
      ctx.currentGuildInfo().id ||
      ctx.getRailSelectedId?.() ||
      ctx.selectedGuildId() ||
      "";
    const speakingSet = new Set(
      Array.isArray(liveRtcStatus?.speakingUsers) ? liveRtcStatus.speakingUsers.map(String) : [],
    );
    const avatarNodes = document.querySelectorAll(".voice-user-avatar[data-voice-avatar-uid]");
    for (const node of avatarNodes) {
      const el = /** @type {HTMLElement} */ (node);
      const uid = String(el.getAttribute("data-voice-avatar-uid") || "");
      const rowEl = el.closest(".voice-channel-user, .tree-voice-user");
      const rowCid = String(rowEl?.getAttribute("data-channel-id") || "");
      const rowInAfk = ctx.isChannelAfk(currentGid, rowCid);
      const isSelf = uid === selfId;
      const isMuted =
        rowInAfk ||
        (isSelf
          ? Boolean(ctx.selfVoicePrefs.muted || ctx.selfVoicePrefs.deafened)
          : Boolean(rowEl?.querySelector('[title*="Микрофон"], [title*="Звук"], [aria-label*="Микрофон"], [aria-label*="Звук"]')));
      const isSpeaking =
        !rowInAfk &&
        (!isMuted || (!isSelf && speakingSet.has(uid))) &&
        (speakingSet.has(uid) || (isSelf && Boolean(liveRtcStatus?.localMicSpeaking)));
      const wasSpeaking = el.getAttribute("data-speaking") === "true";
      if (isSpeaking !== wasSpeaking) {
        if (isSpeaking) {
          el.setAttribute("data-speaking", "true");
          el.style.boxShadow = "0 0 0 2px #23a55a, inset 0 0 0 1.5px #1e1f22";
        } else {
          el.removeAttribute("data-speaking");
          el.style.boxShadow = "none";
        }
        const nameSpan = /** @type {HTMLElement | null} */ (rowEl?.querySelector("span > span"));
        if (nameSpan) {
          if (isSpeaking) {
            nameSpan.style.color = "#f2f3f5";
            nameSpan.style.fontWeight = "600";
          } else {
            const roleCol = nameSpan.getAttribute("data-role-color");
            nameSpan.style.color = roleCol || "#dbdee1";
            nameSpan.style.fontWeight = "400";
          }
        }
      }
    }

    if (currentGid) {
      const timerNodes = document.querySelectorAll("[data-voice-timer-cid]");
      for (const node of timerNodes) {
        const el = /** @type {HTMLElement} */ (node);
        const cid = String(el.getAttribute("data-voice-timer-cid") || "");
        if (!cid) continue;
        const pCount = ctx.getVoiceParticipants(currentGid, cid).length;
        if (pCount > 0) {
          const nextTimer = ctx.getVoiceChannelTimer(currentGid, cid, pCount);
          if (el.textContent !== nextTimer) {
            el.textContent = nextTimer;
          }
        }
      }
      const aloneNodes = document.querySelectorAll("[data-alone-invite-cid]");
      for (const node of aloneNodes) {
        const el = /** @type {HTMLElement} */ (node);
        const cid = String(el.getAttribute("data-alone-invite-cid") || "");
        const remainMs = ctx.getAloneInviteRemainingMs(currentGid, cid);
        const pCount = ctx.getVoiceParticipants(currentGid, cid).length;
        if (remainMs <= 0 || pCount !== 1) {
          el.remove();
        } else {
          const ring = el.querySelector("[data-alone-ring-circle]");
          if (ring) {
            const circum = 62.83;
            const dashOffset = ((1 - Math.max(0, Math.min(1, remainMs / 30_000))) * circum).toFixed(2);
            ring.setAttribute("stroke-dashoffset", dashOffset);
          }
        }
      }
    }
  }

  function ensureUserBarDeviceChevrons() {
    if (!ctx.isLiveSession()) return;
    const actions = document.querySelector("#user-bar .user-actions");
    if (!actions || actions.querySelector(".user-bar-chevron")) return;
    const uMute = /** @type {HTMLElement | null} */ (actions.querySelector('[data-action="mute"]'));
    const uDeaf = /** @type {HTMLElement | null} */ (actions.querySelector('[data-action="deafen"]'));
    const makeChevron = (/** @type {"input" | "output"} */ kind, /** @type {string} */ label) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "icon-btn user-bar-chevron";
      btn.setAttribute("data-chevron-kind", kind);
      btn.setAttribute("aria-label", label);
      btn.title = label;
      btn.style.cssText =
        "width:16px;min-width:16px;height:32px;padding:0;margin-left:0;border:none;border-radius:0 4px 4px 0;display:inline-flex;align-items:center;justify-content:center;color:#b5bac1;background:transparent;cursor:pointer;";
      btn.innerHTML = ctx.SVG_ICONS.chevronDown;
      btn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        void ctx.openAudioDevicePopover(kind, btn);
      });
      return btn;
    };
    if (uMute && uMute.parentNode === actions) {
      uMute.after(makeChevron("input", "Параметры микрофона и блокировки"));
    }
    if (uDeaf && uDeaf.parentNode === actions) {
      uDeaf.after(makeChevron("output", "Параметры звука и блокировки"));
    }
  }

  function syncUserBarVoiceButtons() {
    if (!ctx.isLiveSession()) return;
    ensureUserBarDeviceChevrons();
    const uBar = /** @type {HTMLElement | null} */ (document.getElementById("user-bar"));
    if (uBar) {
      uBar.style.cssText =
        "height:52px;padding:0 8px;display:flex;align-items:center;justify-content:space-between;gap:4px;box-sizing:border-box;";
    }
    const uProf = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="profile"]')
    );
    if (uProf) {
      uProf.style.cssText =
        "display:flex;align-items:center;gap:8px;padding:4px 6px;border-radius:4px;min-width:0;flex:1 1 auto;overflow:hidden;background:transparent;cursor:pointer;border:none;";
    }
    const uActions = /** @type {HTMLElement | null} */ (
      document.querySelector("#user-bar .user-actions")
    );
    if (uActions) {
      uActions.style.cssText =
        "display:flex;align-items:center;gap:2px;flex-shrink:0;";
    }
    const uSettings = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="settings"]')
    );
    if (uSettings && !uSettings.dataset.uBarBound) {
      uSettings.dataset.uBarBound = "1";
      uSettings.style.cssText =
        "width:32px;min-width:32px;height:32px;border-radius:4px;padding:0;border:none;display:inline-flex;align-items:center;justify-content:center;color:#b5bac1;background:transparent;cursor:pointer;transition:background 100ms ease,color 100ms ease;";
      uSettings.onmouseenter = () => {
        uSettings.style.background = "rgba(255,255,255,0.08)";
        uSettings.style.color = "#dbdee1";
      };
      uSettings.onmouseleave = () => {
        uSettings.style.background = "transparent";
        uSettings.style.color = "#b5bac1";
      };
    }
    const uLogout = /** @type {HTMLElement | null} */ (document.getElementById("auth-logout"));
    if (uLogout) {
      uLogout.style.setProperty("display", "none", "important");
      uLogout.style.visibility = "hidden";
      uLogout.style.width = "0px";
      uLogout.style.height = "0px";
      uLogout.style.padding = "0px";
      uLogout.style.margin = "0px";
      uLogout.style.border = "none";
      uLogout.style.overflow = "hidden";
      uLogout.style.position = "absolute";
      uLogout.style.pointerEvents = "none";
      uLogout.setAttribute("aria-hidden", "true");
      uLogout.hidden = true;
    }
    const uMute = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="mute"]')
    );
    const uDeaf = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="deafen"]')
    );
    const uMuteChevron = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-chevron-kind="input"]')
    );
    const uDeafChevron = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-chevron-kind="output"]')
    );
    const muted = ctx.selfVoicePrefs.muted;
    const deafened = ctx.selfVoicePrefs.deafened;
    if (uMute) {
      uMute.setAttribute("data-pressed", muted ? "true" : "false");
      uMute.innerHTML = muted ? ctx.SVG_ICONS.micOff : ctx.SVG_ICONS.micOn;
      uMute.style.cssText =
        `width:32px;min-width:32px;height:32px;padding:0;border:none;border-radius:4px 0 0 4px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${muted ? "#f23f43" : "#b5bac1"};background:${muted ? "rgba(242, 63, 67, 0.18)" : "transparent"};border-right:${muted ? "1px solid rgba(242, 63, 67, 0.3)" : "1px solid transparent"};transition:background 100ms ease,color 100ms ease;`;
      uMute.title = muted
        ? "Ваш микрофон отключён! (Нажмите, чтобы включить микрофон)"
        : "Отключить только микрофон (динамики останутся включены)";
      uMute.setAttribute(
        "aria-label",
        muted ? "Включить микрофон" : "Отключить только микрофон",
      );
      if (!uMute.dataset.uBarBound) {
        uMute.dataset.uBarBound = "1";
        uMute.onmouseenter = () => {
          uMute.style.background = ctx.selfVoicePrefs.muted ? "rgba(242, 63, 67, 0.28)" : "rgba(255, 255, 255, 0.08)";
          uMute.style.color = ctx.selfVoicePrefs.muted ? "#f23f43" : "#dbdee1";
        };
        uMute.onmouseleave = () => {
          uMute.style.background = ctx.selfVoicePrefs.muted ? "rgba(242, 63, 67, 0.18)" : "transparent";
          uMute.style.color = ctx.selfVoicePrefs.muted ? "#f23f43" : "#b5bac1";
        };
      }
    }
    if (uMuteChevron) {
      uMuteChevron.style.cssText =
        `width:16px;min-width:16px;height:32px;padding:0;margin-left:0;margin-right:2px;border:none;border-radius:0 4px 4px 0;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${muted ? "#f23f43" : "#b5bac1"};background:${muted ? "rgba(242, 63, 67, 0.18)" : "transparent"};transition:background 100ms ease,color 100ms ease;`;
      if (!uMuteChevron.dataset.uBarBound) {
        uMuteChevron.dataset.uBarBound = "1";
        uMuteChevron.onmouseenter = () => {
          uMuteChevron.style.background = ctx.selfVoicePrefs.muted ? "rgba(242, 63, 67, 0.28)" : "rgba(255, 255, 255, 0.08)";
          uMuteChevron.style.color = ctx.selfVoicePrefs.muted ? "#f23f43" : "#dbdee1";
        };
        uMuteChevron.onmouseleave = () => {
          uMuteChevron.style.background = ctx.selfVoicePrefs.muted ? "rgba(242, 63, 67, 0.18)" : "transparent";
          uMuteChevron.style.color = ctx.selfVoicePrefs.muted ? "#f23f43" : "#b5bac1";
        };
      }
    }
    if (uDeaf) {
      uDeaf.setAttribute("data-pressed", deafened ? "true" : "false");
      uDeaf.innerHTML = deafened ? ctx.SVG_ICONS.deafOff : ctx.SVG_ICONS.deafOn;
      uDeaf.style.cssText =
        `width:32px;min-width:32px;height:32px;padding:0;border:none;border-radius:4px 0 0 4px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${deafened ? "#f23f43" : "#b5bac1"};background:${deafened ? "rgba(242, 63, 67, 0.18)" : "transparent"};border-right:${deafened ? "1px solid rgba(242, 63, 67, 0.3)" : "1px solid transparent"};transition:background 100ms ease,color 100ms ease;`;
      uDeaf.title = deafened
        ? "Включить звук и микрофон (Снять оглушение)"
        : "Отключить звук и микрофон (Оглушить)";
      uDeaf.setAttribute(
        "aria-label",
        deafened ? "Включить звук" : "Отключить звук и микрофон",
      );
      if (!uDeaf.dataset.uBarBound) {
        uDeaf.dataset.uBarBound = "1";
        uDeaf.onmouseenter = () => {
          uDeaf.style.background = ctx.selfVoicePrefs.deafened ? "rgba(242, 63, 67, 0.28)" : "rgba(255, 255, 255, 0.08)";
          uDeaf.style.color = ctx.selfVoicePrefs.deafened ? "#f23f43" : "#dbdee1";
        };
        uDeaf.onmouseleave = () => {
          uDeaf.style.background = ctx.selfVoicePrefs.deafened ? "rgba(242, 63, 67, 0.18)" : "transparent";
          uDeaf.style.color = ctx.selfVoicePrefs.deafened ? "#f23f43" : "#b5bac1";
        };
      }
    }
    if (uDeafChevron) {
      uDeafChevron.style.cssText =
        `width:16px;min-width:16px;height:32px;padding:0;margin-left:0;margin-right:2px;border:none;border-radius:0 4px 4px 0;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${deafened ? "#f23f43" : "#b5bac1"};background:${deafened ? "rgba(242, 63, 67, 0.18)" : "transparent"};transition:background 100ms ease,color 100ms ease;`;
      if (!uDeafChevron.dataset.uBarBound) {
        uDeafChevron.dataset.uBarBound = "1";
        uDeafChevron.onmouseenter = () => {
          uDeafChevron.style.background = ctx.selfVoicePrefs.deafened ? "rgba(242, 63, 67, 0.28)" : "rgba(255, 255, 255, 0.08)";
          uDeafChevron.style.color = ctx.selfVoicePrefs.deafened ? "#f23f43" : "#dbdee1";
        };
        uDeafChevron.onmouseleave = () => {
          uDeafChevron.style.background = ctx.selfVoicePrefs.deafened ? "rgba(242, 63, 67, 0.18)" : "transparent";
          uDeafChevron.style.color = ctx.selfVoicePrefs.deafened ? "#f23f43" : "#b5bac1";
        };
      }
    }

    const uAvatar = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="profile"] .avatar')
    );
    if (uAvatar) {
      uAvatar.style.position = "relative";
      let dot = /** @type {HTMLElement | null} */ (uAvatar.querySelector("#user-bar-status-dot"));
      if (!dot) {
        dot = document.createElement("span");
        dot.id = "user-bar-status-dot";
        uAvatar.append(dot);
      }
      const stKey = ctx.selfVoicePrefs.status || "online";
      const isInv = stKey === "invisible";
      const stColor =
        stKey === "idle"
          ? "#f0b232"
          : stKey === "dnd"
            ? "#f23f43"
            : isInv
              ? "#80848e"
              : "#23a55a";
      dot.style.cssText =
        `position:absolute;right:-2px;bottom:-2px;width:11px;height:11px;border-radius:50%;background:${isInv ? "#111214" : stColor};border:2.5px solid #111214;box-shadow:${isInv ? "inset 0 0 0 2.2px #80848e" : "none"};pointer-events:none;`;
    }
  }

  /**
   * Renders the guild voice channel stage preview banner (`#voice-stage-preview-banner`).
   */
  function renderLiveGuildVoiceStagePreview() {
    if (typeof document === "undefined") return;
    const isDmView =
      document.body?.dataset?.viewMode === "dm" ||
      ctx.getRailSelectedId?.() === HOME_GUILD_ID ||
      Boolean(document.getElementById("dm-header-call-actions"));
    const previewedVoiceChannel = ctx.getPreviewedVoiceChannel?.();
    if (isDmView || !previewedVoiceChannel || !previewedVoiceChannel.id) {
      removeVoiceStagePreviewBanner(document);
      return;
    }
    const activeGuildId =
      ctx.currentGuildInfo().id || ctx.getRailSelectedId?.() || ctx.selectedGuildId() || "";
    if (
      activeGuildId &&
      previewedVoiceChannel.guildId &&
      ctx.cleanDiscordStr(activeGuildId) !== ctx.cleanDiscordStr(previewedVoiceChannel.guildId)
    ) {
      removeVoiceStagePreviewBanner(document);
      return;
    }
    const gId = previewedVoiceChannel.guildId || activeGuildId;
    const gName = previewedVoiceChannel.guildName || ctx.currentGuildInfo().name || "Сервер";
    const cid = previewedVoiceChannel.id;
    const participants = ctx.getVoiceParticipants(gId, cid);
    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    const liveRtcStatus = ctx.getLiveRtcStatus();
    const isConnected = Boolean(
      activeVoiceConnection &&
        String(activeVoiceConnection.channelId || "") === cid &&
        ctx.cleanDiscordStr(String(activeVoiceConnection.guildId || "")) === ctx.cleanDiscordStr(gId),
    );
    const isConnecting = Boolean(
      isConnected &&
        liveRtcStatus &&
        liveRtcStatus.phase &&
        liveRtcStatus.phase !== "rtc_connected" &&
        liveRtcStatus.phase !== "connected",
    );
    renderVoiceStagePreviewBanner({
      doc: document,
      channel: {
        id: cid,
        name: previewedVoiceChannel.name,
        guildId: gId,
        type: previewedVoiceChannel.type,
      },
      participants,
      isConnected,
      isConnecting,
      onToggleChat: (/** @type {boolean} */ expanded) => {
        const chatMain = document.querySelector(".chat-main");
        if (chatMain) {
          if (expanded) {
            chatMain.classList.add("chat-sidebar-open");
          } else {
            chatMain.classList.remove("chat-sidebar-open");
          }
        }
      },
      onJoinVoice: (ch) => {
        const targetChannel = {
          id: String(ch?.id || cid),
          name: String(ch?.name || previewedVoiceChannel?.name || cid),
          type: Number(ch?.type || previewedVoiceChannel?.type || 2),
        };
        if (
          activeVoiceConnection &&
          activeVoiceConnection.channelId === targetChannel.id &&
          ctx.cleanDiscordStr(String(activeVoiceConnection.guildId || "")) === ctx.cleanDiscordStr(gId)
        ) {
          ctx.openRtcConnectionPopout();
          return;
        }
        if (
          activeVoiceConnection &&
          activeVoiceConnection.channelId !== targetChannel.id &&
          !ctx.getSkipVoiceSwitchConfirm()
        ) {
          ctx.promptVoiceChannelSwitch(gId, gName, targetChannel);
          return;
        }
        ctx.connectToLiveVoiceChannel(gId, gName, targetChannel);
      },
      onLeaveVoice: () => {
        ctx.disconnectFromLiveVoiceChannel();
      },
      onWatchStream: (participant, ch) => {
        const targetChannel = {
          id: String(ch?.id || cid),
          name: String(ch?.name || previewedVoiceChannel?.name || cid),
        };
        if (!isConnected) {
          ctx.connectToLiveVoiceChannel(gId, gName, targetChannel);
        }
        ctx.openLiveStreamStageViewer(
          { ...participant, guildId: gId, channelId: targetChannel.id, streaming: true },
          targetChannel,
        );
      },
      onInviteVoice: (ch) => {
        ctx.openVoiceInviteModal({
          id: String(ch?.id || cid),
          name: String(ch?.name || previewedVoiceChannel?.name || cid),
          type: Number(ch?.type || previewedVoiceChannel?.type || 2),
        });
      },
    });
  }

  /**
   * Renders the active voice connection panel (`#voice-panel`) above the user bar.
   */
  function renderLiveVoicePanel() {
    const voiceHost =
      ctx.getVoiceHost?.() ??
      ctx.voiceHost ??
      (typeof document !== "undefined" ? document.getElementById("voice-panel") : null);
    if (!voiceHost || !ctx.isLiveSession()) return;
    syncUserBarVoiceButtons();
    ctx.syncFloatingStreamPipPreview();
    ctx.renderLiveDmCallStage();
    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    if (/** @type {any} */ (globalThis).__QA__ && !activeVoiceConnection) return;
    if (!activeVoiceConnection) {
      voiceHost.setAttribute("data-state", "ready");
      voiceHost.style.display = "none";
      voiceHost.replaceChildren();
      return;
    }
    const conn = activeVoiceConnection;
    const liveRtcStatus = ctx.getLiveRtcStatus();
    const initPhase = ctx.computeRtcSignalMetrics(liveRtcStatus ?? "signaling").phaseView;
    const initMetrics = ctx.computeRtcSignalMetrics(liveRtcStatus ?? "signaling");
    voiceHost.style.display = "";
    voiceHost.setAttribute("data-state", initPhase.phase);
    voiceHost.replaceChildren();
    const wrap = document.createElement("div");
    wrap.style.cssText = "display:flex;flex-direction:column;gap:10px;width:100%;padding:2px 0;";

    const topRow = document.createElement("div");
    topRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;width:100%;";

    const statusBtn = document.createElement("button");
    statusBtn.id = "live-voice-status-trigger";
    statusBtn.type = "button";
    statusBtn.className = "voice-status-trigger";
    statusBtn.setAttribute("aria-haspopup", "dialog");
    statusBtn.style.cssText =
      "display:flex;align-items:center;gap:8px;min-width:0;flex:1 1 0%;background:none;border:none;padding:0;text-align:left;cursor:pointer;color:inherit;";
    statusBtn.title = "О голосовом звонке (RTC / RTT / Шифрование)";
    const sigIcon = document.createElement("span");
    sigIcon.className = "voice-signal-icon";
    sigIcon.setAttribute("data-voice-signal-icon", "true");
    sigIcon.setAttribute("data-rtc-quality", initMetrics.quality);
    sigIcon.style.cssText =
      `width:20px;height:20px;color:${initMetrics.pingColor};display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;`;
    sigIcon.innerHTML = initMetrics.signalSvg;

    const statusCol = document.createElement("div");
    statusCol.style.cssText = "min-width:0;flex:1 1 0%;overflow:hidden;display:flex;flex-direction:column;gap:1px;";
    const badgeRow = document.createElement("div");
    badgeRow.style.cssText = "display:flex;align-items:center;gap:6px;min-width:0;";
    const badge = document.createElement("span");
    badge.id = "live-voice-status-badge";
    badge.style.cssText =
      `font-size:13px;font-weight:600;color:${initMetrics.pingColor};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;line-height:1.2;`;
    badge.textContent = initPhase.label || "Ожидание ответа сервера Discord...";
    const rttPill = document.createElement("span");
    rttPill.id = "live-voice-rtt-pill";
    rttPill.className = "voice-rtt-pill";
    rttPill.setAttribute("data-voice-rtt-pill", "true");
    rttPill.setAttribute("data-rtc-quality", initMetrics.quality);
    rttPill.setAttribute("data-ping-ms", String(initMetrics.effectiveLast));
    rttPill.setAttribute("data-packet-loss", initMetrics.lossTxt);
    rttPill.style.color = initMetrics.pingColor;
    rttPill.textContent = initMetrics.pillText;
    badgeRow.append(badge, rttPill);

    const connStatus = document.createElement("div");
    connStatus.id = "live-rtc-conn-status";
    connStatus.setAttribute("data-voice-phase-label", "true");
    connStatus.style.cssText =
      "display:none;font-size:11px;font-weight:600;color:#f0b232;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;line-height:1.2;";
    connStatus.textContent = initPhase.label;

    const sub = document.createElement("div");
    sub.style.cssText =
      "font-size:12px;color:#949ba4;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;line-height:1.25;";
    const isDmCall =
      !conn.guildId || conn.guildId === HOME_GUILD_ID || conn.guildName === conn.channelName;
    sub.textContent = isDmCall
      ? String(conn.channelName || "").replace(/^@\s*/, "")
      : `${conn.channelName} / ${conn.guildName || "Сервер"}`;
    sub.addEventListener("mouseenter", () => {
      sub.style.textDecoration = "underline";
      sub.style.color = "#dbdee1";
    });
    sub.addEventListener("mouseleave", () => {
      sub.style.textDecoration = "none";
      sub.style.color = "#949ba4";
    });
    statusCol.append(badgeRow, connStatus, sub);
    statusBtn.append(sigIcon, statusCol);

    statusBtn.addEventListener("mouseenter", () => {
      badge.setAttribute("data-hovered", "true");
      badge.textContent = "О голосовом звонке";
    });
    statusBtn.addEventListener("mouseleave", () => {
      badge.removeAttribute("data-hovered");
      updateLiveRtcDomInPlace();
    });
    statusBtn.addEventListener("click", () => {
      ctx.openRtcConnectionPopout();
    });

    const topRightActions = document.createElement("div");
    topRightActions.style.cssText = "display:flex;align-items:center;gap:4px;flex-shrink:0;";

    const krispBtn = document.createElement("button");
    krispBtn.type = "button";
    krispBtn.className = "icon-btn";
    krispBtn.style.cssText =
      `width:32px;height:32px;border-radius:4px;border:none;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${ctx.selfVoicePrefs.krispEnabled ? "#dbdee1" : "#80848e"};background:transparent;`;
    krispBtn.innerHTML = SVG_KRISP_BARS;
    krispBtn.title = "Подавление шума с помощью Krisp";
    krispBtn.setAttribute("aria-label", "Подавление шума с помощью Krisp");
    krispBtn.addEventListener("mouseenter", () => {
      krispBtn.style.background = "rgba(255,255,255,0.08)";
      krispBtn.style.color = "#f2f3f5";
    });
    krispBtn.addEventListener("mouseleave", () => {
      krispBtn.style.background = "transparent";
      krispBtn.style.color = ctx.selfVoicePrefs.krispEnabled ? "#dbdee1" : "#80848e";
    });
    krispBtn.addEventListener("click", () => {
      ctx.openKrispNoisePopout();
    });

    const leaveBtn = document.createElement("button");
    leaveBtn.type = "button";
    leaveBtn.className = "icon-btn";
    leaveBtn.style.cssText =
      "width:32px;height:32px;border-radius:4px;border:none;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:#dbdee1;background:transparent;";
    leaveBtn.innerHTML = SVG_PHONE_DISCONNECT;
    leaveBtn.title = "Отключиться";
    leaveBtn.setAttribute("aria-label", "Отключиться");
    leaveBtn.addEventListener("mouseenter", () => {
      leaveBtn.style.background = "rgba(242,63,67,0.16)";
      leaveBtn.style.color = "#f23f43";
    });
    leaveBtn.addEventListener("mouseleave", () => {
      leaveBtn.style.background = "transparent";
      leaveBtn.style.color = "#dbdee1";
    });
    leaveBtn.addEventListener("click", () => {
      ctx.disconnectFromLiveVoiceChannel();
    });
    topRightActions.append(krispBtn, leaveBtn);
    topRow.append(statusBtn, topRightActions);

    const mediaRow = document.createElement("div");
    mediaRow.style.cssText = "display:grid;grid-template-columns:repeat(4,1fr);gap:6px;width:100%;margin-top:2px;";
    /**
     * @param {string} svg
     * @param {string} tooltip
     * @param {boolean} active
     * @param {boolean} mutedAppearance
     * @param {() => void} onClick
     */
    const makeMediaBtn = (svg, tooltip, active, mutedAppearance, onClick) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "icon-btn";
      btn.title = tooltip;
      btn.setAttribute("aria-label", tooltip);
      btn.setAttribute("data-active", active ? "true" : "false");
      const baseBg = active ? "#23a55a" : "#2b2d31";
      const baseColor = active ? "#ffffff" : mutedAppearance ? "#80848e" : "#f2f3f5";
      btn.style.cssText =
        `height:32px;border-radius:4px;border:none;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;background:${baseBg};color:${baseColor};transition:background 110ms ease,color 110ms ease;`;
      btn.innerHTML = svg;
      btn.addEventListener("mouseenter", () => {
        if (!active) {
          btn.style.background = "#35373c";
          btn.style.color = "#ffffff";
        }
      });
      btn.addEventListener("mouseleave", () => {
        btn.style.background = baseBg;
        btn.style.color = baseColor;
      });
      btn.addEventListener("click", onClick);
      return btn;
    };
    const hasStreamPerm = ctx.canStreamInChannel(conn.guildId, conn.channelId);
    const camBtn = makeMediaBtn(
      conn.cameraOn ? SVG_CAMERA_ON : SVG_CAMERA_OFF,
      !hasStreamPerm && !conn.cameraOn
        ? "У вашей роли нет разрешения «Видео / Трансляция» в этом канале"
        : conn.cameraOn
          ? "Выключить камеру"
          : "Включить камеру",
      Boolean(conn.cameraOn),
      !conn.cameraOn,
      () => {
        if (!conn.cameraOn && !ctx.canStreamInChannel(conn.guildId, conn.channelId)) {
          ctx.showVoiceToastNotice("У вашей роли нет разрешения «Видео / Трансляция» в этом голосовом канале");
          return;
        }
        conn.cameraOn = !conn.cameraOn;
        ctx.playVoiceUiSound(conn.cameraOn ? "camera_on" : "camera_off");
        ctx.syncVoiceMuteStateWithBackend();
        renderLiveVoicePanel();
        ctx.getTree?.()?.refreshView?.();
        ctx.emitLiveTelemetry({ type: "voice_camera_toggle", cameraOn: conn.cameraOn });
      },
    );
    if (!hasStreamPerm && !conn.cameraOn) {
      camBtn.style.opacity = "0.45";
      camBtn.style.cursor = "not-allowed";
    }
    const shareBtn = makeMediaBtn(
      SVG_SCREENSHARE,
      !hasStreamPerm && !conn.screenShareOn
        ? "У вашей роли нет разрешения «Видео / Трансляция» в этом канале"
        : conn.screenShareOn
          ? "Изменить окно или настройки стрима"
          : "Продемонстрировать свой экран",
      Boolean(conn.screenShareOn),
      false,
      () => {
        if (!conn.screenShareOn && !ctx.canStreamInChannel(conn.guildId, conn.channelId)) {
          ctx.showVoiceToastNotice("У вашей роли нет разрешения «Видео / Трансляция» в этом голосовом канале");
          return;
        }
        if (conn.screenShareOn) {
          ctx.openScreenSharePortalPicker("windows", ctx.switchLiveStreamSource);
        } else {
          ctx.openScreenShareModal(Boolean(conn.screenShareOn));
        }
      },
    );
    if (!hasStreamPerm && !conn.screenShareOn) {
      shareBtn.style.opacity = "0.45";
      shareBtn.style.cursor = "not-allowed";
    }
    mediaRow.append(
      camBtn,
      shareBtn,
      makeMediaBtn(SVG_ACTIVITY, "Начать активность", false, false, () => {
        ctx.openActivitiesPopout();
      }),
      makeMediaBtn(SVG_SOUNDBOARD, "Звуковая панель", false, false, () => {
        ctx.openSoundboardPopout();
      }),
    );

    wrap.append(topRow);
    if (conn.screenShareOn) {
      const streamRow = document.createElement("div");
      streamRow.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;gap:8px;padding:6px 2px 4px;border-top:1px solid #2b2d31;";
      const sLeft = document.createElement("div");
      sLeft.style.cssText = "display:flex;flex-direction:column;gap:1px;min-width:0;flex:1;";
      const sTitle = document.createElement("div");
      sTitle.style.cssText =
        "font-size:12px;font-weight:700;color:#f2f3f5;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;cursor:pointer;";
      sTitle.title = "Нажмите, чтобы сменить окно или экран трансляции";
      sTitle.textContent = conn.streamSourceName || "Окно, экран или приложение";
      sTitle.addEventListener("click", () => {
        ctx.openScreenSharePortalPicker("windows", ctx.switchLiveStreamSource);
      });
      const sRes = /** @type {any} */ (conn).streamResolution || /** @type {any} */ (conn).resolution || "720p";
      const sFps = /** @type {any} */ (conn).streamFps || /** @type {any} */ (conn).fps || 60;
      const sSub = document.createElement("div");
      sSub.style.cssText =
        "display:flex;align-items:center;gap:5px;font-size:11px;font-weight:600;color:#23a55a;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
      sSub.innerHTML = `<span style="font-size:11px;">📹</span><span>${sRes} ${sFps} кадров в секунду</span>`;
      sLeft.append(sTitle, sSub);

      const sRight = document.createElement("div");
      sRight.style.cssText = "display:flex;align-items:center;gap:4px;flex-shrink:0;";
      const changeStreamSourceBtn = document.createElement("button");
      changeStreamSourceBtn.type = "button";
      changeStreamSourceBtn.id = "voice-panel-change-source-btn";
      changeStreamSourceBtn.className = "icon-btn";
      changeStreamSourceBtn.title = "Сменить источник";
      changeStreamSourceBtn.setAttribute("aria-label", "Сменить источник");
      changeStreamSourceBtn.style.cssText =
        "width:28px;height:28px;border-radius:6px;border:none;background:transparent;color:#dbdee1;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;font-size:13px;";
      changeStreamSourceBtn.textContent = "🖥";
      changeStreamSourceBtn.addEventListener("mouseenter", () => {
        changeStreamSourceBtn.style.background = "rgba(88,101,242,0.18)";
        changeStreamSourceBtn.style.color = "#f2f3f5";
      });
      changeStreamSourceBtn.addEventListener("mouseleave", () => {
        changeStreamSourceBtn.style.background = "transparent";
        changeStreamSourceBtn.style.color = "#dbdee1";
      });
      changeStreamSourceBtn.addEventListener("click", () => {
        ctx.openScreenSharePortalPicker("windows", ctx.switchLiveStreamSource);
      });
      const stopStreamBtn = document.createElement("button");
      stopStreamBtn.type = "button";
      stopStreamBtn.className = "icon-btn";
      stopStreamBtn.title = "Прекратить стрим";
      stopStreamBtn.setAttribute("aria-label", "Прекратить стрим");
      stopStreamBtn.style.cssText =
        "width:28px;height:28px;border-radius:6px;border:none;background:transparent;color:#dbdee1;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
      stopStreamBtn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1"><rect x="3" y="4" width="18" height="12" rx="2"/><line x1="8" y1="20" x2="16" y2="20"/><line x1="9.5" y1="7.5" x2="14.5" y2="12.5"/><line x1="14.5" y1="7.5" x2="9.5" y2="12.5"/></svg>`;
      stopStreamBtn.addEventListener("mouseenter", () => {
        stopStreamBtn.style.background = "rgba(242,63,67,0.16)";
        stopStreamBtn.style.color = "#f23f43";
      });
      stopStreamBtn.addEventListener("mouseleave", () => {
        stopStreamBtn.style.background = "transparent";
        stopStreamBtn.style.color = "#dbdee1";
      });
      stopStreamBtn.addEventListener("click", () => {
        conn.screenShareOn = false;
        conn.hideStreamPreview = true;
        ctx.cancelPipAnimTimer();
        document.getElementById("live-stream-stage-viewer")?.remove();
        document.getElementById("live-stream-pip-preview")?.remove();
        document.getElementById("live-stream-preview-overlay")?.remove();
        document.getElementById("live-screenshare-modal")?.remove();
        document.getElementById("live-screenshare-portal-modal")?.remove();
        document.getElementById("live-stream-settings-menu")?.remove();
        document.getElementById("live-stream-settings-submenu")?.remove();
        ctx.cachedLiveScreenImg = null;
        ctx.cachedLiveScreenDataUrl = "";
        ctx.lastLiveScreenSeq = 0;
        const selfId = ctx.cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
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
        const panelConn = /** @type {any} */ (conn);
        const isCall = !panelConn.guildId || panelConn.guildId === HOME_GUILD_ID || panelConn.guildId === "@me";
        const stopKey =
          panelConn.activeStreamKey ||
          /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ ||
          (isCall ? `call:${panelConn.channelId}:${selfId}` : `guild:${panelConn.guildId}:${panelConn.channelId}:${selfId}`);
        void ctx
          .invoke("voice_stream_stop", {
            stream_key: stopKey,
            streamKey: stopKey,
          })
          .catch(() => {});
        panelConn.activeStreamKey = null;
        /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ = null;
        ctx.syncVoiceMuteStateWithBackend();
        renderLiveVoicePanel();
        ctx.getTree?.()?.refreshView?.();
      });
      sRight.append(changeStreamSourceBtn, stopStreamBtn);
      streamRow.append(sLeft, sRight);
      wrap.append(streamRow);
    }
    wrap.append(mediaRow);
    voiceHost.append(wrap);
    updateLiveRtcDomInPlace();
  }

  return {
    updateLiveRtcDomInPlace,
    ensureUserBarDeviceChevrons,
    syncUserBarVoiceButtons,
    renderLiveGuildVoiceStagePreview,
    renderLiveVoicePanel,
  };
}
