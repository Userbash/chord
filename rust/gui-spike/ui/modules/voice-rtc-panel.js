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
  afk: "🔕 AFK-канал (микрофон и звук отключены)",
});

/**
 * Человекочитаемые подписи состояний сквозного шифрования DAVE MLS v1 (`STAGE-10.BFF.6`).
 */
export const DAVE_E2EE_STATE_LABELS = Object.freeze({
  encrypted: "🔒 DAVE E2EE",
  waiting_for_peer: "⏳ DAVE E2EE",
  handshaking: "🔑 DAVE MLS",
  recovering: "🔄 DAVE MLS",
  downgrade_blocked: "⚠️ DAVE Защита",
  disconnected: "",
});

const DAVE_E2EE_STATE_COLORS = Object.freeze({
  encrypted: "#23a55a",
  waiting_for_peer: "#f0b232",
  handshaking: "#f0b232",
  recovering: "#f0b232",
  downgrade_blocked: "#f23f43",
  disconnected: "#80848e",
});

/**
 * @typedef {"encrypted" | "waiting_for_peer" | "handshaking" | "recovering" | "downgrade_blocked" | "disconnected"} DaveE2eeUiState
 */

/**
 * @typedef {{
 *   state: DaveE2eeUiState,
 *   isEncrypted: boolean,
 *   daveVerified: boolean,
 *   protocolVersion: number,
 *   epoch: number | null,
 *   privacyCode: string | null,
 *   privacyCodeChunks: string[],
 *   badgeLabel: string,
 *   tooltip: string,
 *   statusColor: string,
 * }} DaveE2eeIndicatorView
 */

/**
 * Вычисляет индикатор состояния DAVE E2EE (`DaveVoiceBridge`) для голосового интерфейса.
 *
 * @param {Record<string, any> | null | undefined} statusOrE2ee
 * @returns {DaveE2eeIndicatorView}
 */
export function resolveDaveE2eeIndicatorView(statusOrE2ee) {
  if (!statusOrE2ee || typeof statusOrE2ee !== "object") {
    return {
      state: "disconnected",
      isEncrypted: false,
      daveVerified: false,
      protocolVersion: 0,
      epoch: null,
      privacyCode: null,
      privacyCodeChunks: [],
      badgeLabel: DAVE_E2EE_STATE_LABELS.disconnected,
      tooltip: "Голосовое соединение не активно",
      statusColor: DAVE_E2EE_STATE_COLORS.disconnected,
    };
  }

  const raw =
    statusOrE2ee.e2ee && typeof statusOrE2ee.e2ee === "object"
      ? statusOrE2ee.e2ee
      : statusOrE2ee;
  const rawState = String(raw.state ?? "").trim().toLowerCase();
  const isStage = Boolean(raw.isStageChannel || raw.isStageVoiceChannel);
  const downgradeBlocked = Boolean(raw.downgradeBlocked ?? raw.downgrade_blocked);
  const protoVer = Number(
    raw.protocolVersion ?? raw.protocol_version ?? raw.daveProtocolVersion ?? 0,
  );
  const epochRaw = raw.epoch;
  const epoch =
    typeof epochRaw === "number" && Number.isFinite(epochRaw) && epochRaw > 0
      ? epochRaw
      : null;

  const isEncrypted = Boolean(
    (raw.isEndToEndEncrypted ?? raw.is_end_to_end_encrypted ?? rawState === "encrypted") &&
      protoVer >= 1,
  );

  const rawCode = typeof raw.privacyCode === "string"
    ? raw.privacyCode.trim()
    : typeof raw.privacy_code === "string"
      ? raw.privacy_code.trim()
      : "";
  /** @type {string[]} */
  const chunks = /^\d{30}$/.test(rawCode)
    ? Array.from({ length: 6 }, (_, idx) => rawCode.slice(idx * 5, (idx + 1) * 5))
    : rawCode
      ? rawCode.split(/\s+/)
      : [];
  const validChunks =
    !isStage && chunks.length === 6 && chunks.every((c) => /^\d{5}$/.test(c))
      ? chunks
      : [];
  const normalizedCode = validChunks.length === 6 ? validChunks.join(" ") : null;
  const daveVerified = isEncrypted && !isStage && validChunks.length === 6;

  /** @type {DaveE2eeUiState} */
  let state = "disconnected";
  if (isEncrypted) {
    state = "encrypted";
  } else if (downgradeBlocked || rawState === "downgrade_blocked") {
    state = "downgrade_blocked";
  } else if (rawState === "recovering" || Number(raw.resets ?? 0) > 0) {
    state = "recovering";
  } else if (rawState === "waiting_for_peer" || Boolean(raw.waitingForPeer ?? raw.waiting_for_peer)) {
    state = "waiting_for_peer";
  } else if (
    rawState === "handshaking" ||
    rawState === "negotiating" ||
    rawState === "signaling" ||
    rawState === "connected"
  ) {
    state = "handshaking";
  }

  const tooltip =
    state === "encrypted"
      ? `Сквозное шифрование DAVE MLS v1 активно${epoch ? ` • Эпоха ${epoch}` : ""}`
      : state === "waiting_for_peer"
        ? "DAVE MLS v1: ожидание собеседника для обмена ключами"
        : state === "handshaking"
          ? "DAVE MLS v1: выполняется согласование ключей группы"
          : state === "recovering"
            ? "DAVE MLS v1: пересборка ключевой эпохи (Op 31)"
            : state === "downgrade_blocked"
              ? "Заблокирована попытка понижения версии шифрования DAVE"
              : "Голосовое соединение не активно";

  return {
    state,
    isEncrypted,
    daveVerified,
    protocolVersion: Number.isFinite(protoVer) ? protoVer : 0,
    epoch,
    privacyCode: normalizedCode,
    privacyCodeChunks: validChunks,
    badgeLabel: DAVE_E2EE_STATE_LABELS[state],
    tooltip,
    statusColor: DAVE_E2EE_STATE_COLORS[state],
  };
}

/**
 * @typedef {"signaling" | "negotiating" | "connected" | "afk" | "error" | "timeout" | "disconnected"} VoiceCallPhase
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

  if (rawPhase === "afk" || (isObj && Boolean(statusOrPhase.isAfk))) {
    return {
      phase: "afk",
      label: VOICE_CALL_PHASE_LABELS.afk,
      reason: "Микрофон и звук отключены сервером",
      canRetry: false,
      isError: false,
      isConnected: true,
      statusColor: "#949ba4",
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
    panelEl.setAttribute("data-dave-e2ee-state", "disconnected");
    panelEl.setAttribute("data-dave-verified", "false");
    const statusDot =
      panelEl.querySelector?.(".voice-status-dot") ??
      panelEl.querySelector?.("[data-voice-status-dot]");
    if (statusDot && statusDot.dataset) {
      statusDot.dataset.state = "disconnected";
      statusDot.dataset.phase = "disconnected";
    }
    const e2eePill = panelEl.querySelector?.("#live-voice-e2ee-pill");
    if (e2eePill && e2eePill.style) {
      e2eePill.style.display = "none";
    }
    const existingRetry = panelEl.querySelector?.("[data-voice-retry-btn]");
    existingRetry?.remove?.();
    return view;
  }

  const isObj = typeof statusOrPhase === "object" && statusOrPhase !== null;
  const e2eeView = resolveDaveE2eeIndicatorView(isObj ? statusOrPhase : null);
  panelEl.setAttribute("data-dave-e2ee-state", e2eeView.state);
  panelEl.setAttribute("data-dave-protocol-version", String(e2eeView.protocolVersion));
  panelEl.setAttribute("data-dave-verified", e2eeView.daveVerified ? "true" : "false");
  const e2eePill =
    panelEl.querySelector?.("#live-voice-e2ee-pill") ??
    doc?.getElementById?.("live-voice-e2ee-pill");
  if (e2eePill) {
    if (view.phase === "afk" || e2eeView.state === "disconnected") {
      if (e2eePill.style) e2eePill.style.display = "none";
    } else {
      if (e2eePill.style) {
        e2eePill.style.display = "";
        e2eePill.style.color = e2eeView.statusColor;
      }
      e2eePill.textContent = e2eeView.badgeLabel;
      e2eePill.title = e2eeView.tooltip;
      e2eePill.setAttribute?.("data-dave-e2ee-state", e2eeView.state);
      e2eePill.setAttribute?.("data-dave-verified", e2eeView.daveVerified ? "true" : "false");
      e2eePill.setAttribute?.(
        "data-dave-epoch",
        e2eeView.epoch !== null ? String(e2eeView.epoch) : "0",
      );
    }
  }
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
  if (view.phase === "afk") {
    if (badgeEl && badgeEl !== statusEl) {
      if (badgeEl.getAttribute?.("data-hovered") !== "true") {
        badgeEl.textContent = "🔕 AFK-канал";
        badgeEl.setAttribute?.("data-call-phase", "afk");
        if (badgeEl.style) {
          badgeEl.style.color = view.statusColor;
        }
      }
    }
    if (statusEl) {
      statusEl.textContent = view.reason || "Микрофон и звук отключены сервером";
      statusEl.setAttribute?.("data-call-phase", "afk");
      if (statusEl.style) {
        statusEl.style.display = "";
        statusEl.style.color = view.statusColor;
      }
    }
    const rttPill =
      panelEl.querySelector?.("#live-voice-rtt-pill") ??
      doc?.getElementById?.("live-voice-rtt-pill");
    if (rttPill && rttPill.style) {
      rttPill.style.display = "none";
    }
  } else if (badgeEl && badgeEl !== statusEl) {
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
