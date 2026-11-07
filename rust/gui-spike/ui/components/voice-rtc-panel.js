// @ts-check
/**
 * Reactive Voice / RTC Status Panel (`@preact/signals` architecture).
 * Keeps the bottom-left voice connection bar (`#voice-panel`) and RTC diagnostic badge
 * declaratively synchronized with `globalAppReactiveStore.activeVoiceConnection` and `voiceStatesByGuild`
 * without manual DOM drift.
 */

import { effect } from "../features/reactive-store.js";

/**
 * Mounts reactive synchronization for the Voice/RTC panel.
 *
 * @param {{
 *   store: typeof import("../features/reactive-store.js").globalAppReactiveStore,
 *   doc?: Document,
 * }} options
 * @returns {() => void} Dispose function
 */
export function mountReactiveVoiceRtcPanel({ store, doc = document }) {
  if (!store || !store.activeVoiceConnection) {
    return () => {};
  }
  const dispose = effect(() => {
    const conn = store.activeVoiceConnection.value;
    const panel = doc?.getElementById?.("voice-panel");
    if (!panel) return;

    const statusDot = panel.querySelector(".voice-status-dot, [data-voice-status-dot]");
    const activeCid = conn?.channelId ? String(conn.channelId) : "";

    if (!activeCid) {
      panel.removeAttribute("data-rtc-state");
      if (statusDot instanceof HTMLElement) {
        statusDot.dataset.state = "disconnected";
      }
      return;
    }

    const rtcState = conn?.selfStream ? "streaming" : "connected";
    panel.setAttribute("data-rtc-state", rtcState);
    if (statusDot instanceof HTMLElement) {
      statusDot.dataset.state = rtcState;
    }
  });

  return dispose;
}
