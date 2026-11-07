// @ts-check
/**
 * Reactive Voice / RTC Status Panel (`@preact/signals` architecture — Stage V5).
 * Displays all 4 real call phases from the `Calls` state machine:
 * - `'signaling'` -> `'Ожидание ответа сервера Discord...'`
 * - `'negotiating'` -> `'Проверка UDP IP Discovery и ключей DAVE...'`
 * - `'connected'` -> `'Голосовая связь установлена • DAVE E2EE'`
 * - `'error'` / `'timeout'` -> exact failure reason and a retry button (`Повторить`).
 */

import { effect } from "../features/reactive-store.js";

/**
 * Человекочитаемые подписи фаз автомата `Calls`.
 */
export const VOICE_CALL_PHASE_LABELS = Object.freeze({
  signaling: "Ожидание ответа сервера Discord...",
  negotiating: "Проверка UDP IP Discovery и ключей DAVE...",
  connected: "Голосовая связь установлена • DAVE E2EE",
});

/**
 * @typedef {"signaling" | "negotiating" | "connected" | "error" | "timeout" | "disconnected"} VoiceCallPhase
 */

/**
 * @typedef {{
 *   phase: VoiceCallPhase,
 *   label: string,
 *   reason: string,
 *   canRetry: boolean,
 *   isError: boolean,
 *   isConnected: boolean,
 *   statusColor: string,
 * }} VoiceCallPhaseView
 */

/**
 * Вычисляет представление текущей фазы звонка из автомата `Calls` или снимка `voice_rtc_status`.
 *
 * @param {string | Record<string, any> | null | undefined} statusOrPhase
 * @returns {VoiceCallPhaseView}
 */
export function resolveVoiceCallPhaseView(statusOrPhase) {
  if (!statusOrPhase) {
    return {
      phase: "disconnected",
      label: "",
      reason: "",
      canRetry: false,
      isError: false,
      isConnected: false,
      statusColor: "#80848e",
    };
  }

  const isObj = typeof statusOrPhase === "object";
  const rawPhase = String(
    isObj
      ? statusOrPhase.phase ?? statusOrPhase.state ?? "signaling"
      : statusOrPhase,
  )
    .trim()
    .toLowerCase();

  const rawReason = isObj
    ? String(
        statusOrPhase.error ??
          statusOrPhase.reason ??
          statusOrPhase.failureReason ??
          statusOrPhase.message ??
          "",
      ).trim()
    : "";

  if (rawPhase === "disconnected" && !rawReason) {
    return {
      phase: "disconnected",
      label: "",
      reason: "",
      canRetry: false,
      isError: false,
      isConnected: false,
      statusColor: "#80848e",
    };
  }

  const isTimeout =
    rawPhase === "timeout" ||
    (isObj && Boolean(statusOrPhase.timedOut)) ||
    (rawPhase === "error" && /timeout|таймаут/i.test(rawReason) && isObj && statusOrPhase.phase === "timeout");

  if (isTimeout || rawPhase === "error" || (rawReason.length > 0 && rawPhase !== "connected")) {
    /** @type {VoiceCallPhase} */
    const errPhase = isTimeout ? "timeout" : "error";
    const fallbackReason =
      errPhase === "timeout"
        ? "Превышено время ожидания ответа голосового сервера Discord"
        : "Ошибка подключения к голосовому каналу";
    const exactReason = rawReason || fallbackReason;
    return {
      phase: errPhase,
      label: exactReason,
      reason: exactReason,
      canRetry: true,
      isError: true,
      isConnected: false,
      statusColor: "#f23f43",
    };
  }

  if (rawPhase === "connected") {
    return {
      phase: "connected",
      label: VOICE_CALL_PHASE_LABELS.connected,
      reason: "",
      canRetry: false,
      isError: false,
      isConnected: true,
      statusColor: "#23a55a",
    };
  }

  if (
    rawPhase === "negotiating" ||
    rawPhase === "connecting" ||
    rawPhase === "reconnecting" ||
    rawPhase === "udp_discovery" ||
    rawPhase === "dave_handshake"
  ) {
    return {
      phase: "negotiating",
      label: VOICE_CALL_PHASE_LABELS.negotiating,
      reason: "",
      canRetry: false,
      isError: false,
      isConnected: false,
      statusColor: "#f0b232",
    };
  }

  return {
    phase: "signaling",
    label: VOICE_CALL_PHASE_LABELS.signaling,
    reason: "",
    canRetry: false,
    isError: false,
    isConnected: false,
    statusColor: "#f0b232",
  };
}

/**
 * Обновляет DOM-элемент панели голосовой связи (`#voice-panel`) согласно фазе звонка `Calls`.
 *
 * @param {any} panelEl
 * @param {string | Record<string, any> | null | undefined} statusOrPhase
 * @param {{
 *   doc?: any,
 *   onRetry?: (() => void) | null,
 * }} [options]
 * @returns {VoiceCallPhaseView}
 */
export function renderVoiceRtcPhaseDom(panelEl, statusOrPhase, options = {}) {
  const view = resolveVoiceCallPhaseView(statusOrPhase);
  if (!panelEl || typeof panelEl.setAttribute !== "function") {
    return view;
  }

  const doc =
    options.doc ??
    panelEl.ownerDocument ??
    (typeof document !== "undefined" ? document : null);

  if (view.phase === "disconnected") {
    panelEl.removeAttribute?.("data-rtc-state");
    panelEl.setAttribute("data-call-phase", "disconnected");
    const statusDot =
      panelEl.querySelector?.(".voice-status-dot") ??
      panelEl.querySelector?.("[data-voice-status-dot]");
    if (statusDot && statusDot.dataset) {
      statusDot.dataset.state = "disconnected";
      statusDot.dataset.phase = "disconnected";
    }
    const existingRetry = panelEl.querySelector?.("[data-voice-retry-btn]");
    existingRetry?.remove?.();
    return view;
  }

  const isObj = typeof statusOrPhase === "object" && statusOrPhase !== null;
  const selfStream = Boolean(isObj && (statusOrPhase.selfStream || statusOrPhase.screenShareOn));
  const rtcState = view.isConnected ? (selfStream ? "streaming" : "connected") : view.phase;

  panelEl.setAttribute("data-rtc-state", rtcState);
  panelEl.setAttribute("data-call-phase", view.phase);

  const statusDot =
    panelEl.querySelector?.(".voice-status-dot") ??
    panelEl.querySelector?.("[data-voice-status-dot]");
  if (statusDot) {
    if (statusDot.dataset) {
      statusDot.dataset.state = rtcState;
      statusDot.dataset.phase = view.phase;
    }
    statusDot.setAttribute?.("data-call-phase", view.phase);
  }

  const statusEl =
    panelEl.querySelector?.("#live-rtc-conn-status") ??
    panelEl.querySelector?.("[data-voice-phase-label]") ??
    doc?.getElementById?.("live-rtc-conn-status");

  if (statusEl) {
    statusEl.textContent = view.label;
    statusEl.setAttribute?.("data-call-phase", view.phase);
    if (statusEl.style) {
      statusEl.style.color = view.statusColor;
    }
  }

  const badgeEl =
    panelEl.querySelector?.("#live-voice-status-badge") ??
    doc?.getElementById?.("live-voice-status-badge");
  if (badgeEl && badgeEl !== statusEl) {
    if (statusEl && statusEl.style) {
      statusEl.style.display = "none";
    }
    if (badgeEl.getAttribute?.("data-hovered") !== "true") {
      badgeEl.textContent = view.label;
      badgeEl.setAttribute?.("data-call-phase", view.phase);
      if (badgeEl.style) {
        badgeEl.style.color = view.statusColor;
      }
    }
  }

  const retryCallback =
    typeof options.onRetry === "function"
      ? options.onRetry
      : isObj && typeof statusOrPhase.onRetry === "function"
        ? statusOrPhase.onRetry
        : null;

  let retryBtn = panelEl.querySelector?.("[data-voice-retry-btn]");
  if (view.canRetry) {
    if (!retryBtn && doc && typeof doc.createElement === "function") {
      retryBtn = doc.createElement("button");
      retryBtn.type = "button";
      retryBtn.id = "live-rtc-retry-btn";
      retryBtn.className = "voice-rtc-retry-btn";
      retryBtn.setAttribute("data-voice-retry-btn", "true");
      retryBtn.setAttribute("aria-label", "Повторить подключение к голосовому каналу");
      retryBtn.textContent = "Повторить";
      if (retryBtn.style) {
        retryBtn.style.cssText =
          "margin-top:4px;padding:3px 8px;border-radius:4px;border:1px solid rgba(242,63,67,0.55);background:rgba(242,63,67,0.16);color:#f2f3f5;font-size:11px;font-weight:600;cursor:pointer;align-self:flex-start;";
      }
      const host = statusEl?.parentElement ?? panelEl;
      host.append(retryBtn);
    }
    if (retryBtn) {
      retryBtn.__onVoiceRetry = retryCallback;
      if (!retryBtn.__voiceRetryBound) {
        retryBtn.__voiceRetryBound = true;
        retryBtn.addEventListener("click", (/** @type {any} */ ev) => {
          ev?.stopPropagation?.();
          if (typeof retryBtn.__onVoiceRetry === "function") {
            retryBtn.__onVoiceRetry();
          }
        });
      }
    }
  } else if (retryBtn) {
    retryBtn.remove?.();
  }

  return view;
}

/**
 * Mounts reactive synchronization for the Voice/RTC panel.
 *
 * @param {{
 *   store: typeof import("../features/reactive-store.js").globalAppReactiveStore,
 *   doc?: any,
 *   onRetry?: (() => void) | null,
 * }} options
 * @returns {() => void} Dispose function
 */
export function mountReactiveVoiceRtcPanel({
  store,
  doc = typeof document !== "undefined" ? document : null,
  onRetry = null,
}) {
  if (!store || !store.activeVoiceConnection) {
    return () => {};
  }
  const dispose = effect(() => {
    const conn = store.activeVoiceConnection.value;
    const panel = doc?.getElementById?.("voice-panel");
    if (!panel) return;

    const activeCid = conn?.channelId ? String(conn.channelId) : "";
    if (!activeCid) {
      renderVoiceRtcPhaseDom(panel, { phase: "disconnected" }, { doc, onRetry });
      return;
    }

    renderVoiceRtcPhaseDom(panel, conn, { doc, onRetry });
  });

  return dispose;
}
