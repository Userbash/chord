// @ts-check
/**
 * Lazy initialization and DOM button wiring for channel pins and threads panels.
 * Extracted from `app.js` as part of RW51-EPIC2-T4.
 * @module ui/features/pins-threads-wiring
 */

import { createPinsThreadsPanels as defaultCreatePinsThreadsPanels } from "./pins-threads.js";

/**
 * Create a lazy pins/threads panel controller and button binder.
 * Keeps DOM clean until the first user interaction.
 *
 * @param {any} ctx
 */
export function createPinsThreadsWiring(ctx) {
  const {
    document: doc,
    container,
    pinsButton,
    threadsButton,
    chat,
    invoke,
    router,
    createPinsThreadsPanels = defaultCreatePinsThreadsPanels,
  } = ctx;

  /** @type {ReturnType<typeof defaultCreatePinsThreadsPanels> | null} */
  let pinsThreadsInstance = null;

  /**
   * Load pinned messages for the active channel.
   * @returns {Promise<any[]> | any[]}
   */
  function loadPins() {
    if (chat.mode === "channel" && chat.channelId) {
      return invoke("pins_list", { channelId: chat.channelId });
    }
    return [];
  }

  /**
   * Load active/archived threads for the active channel.
   * @returns {Promise<any[]> | any[]}
   */
  function loadThreads() {
    if (chat.mode === "channel" && chat.channelId) {
      return invoke("threads_list", { channelId: chat.channelId });
    }
    return [];
  }

  /**
   * Lazily instantiate and return the pins/threads panel controller.
   * @returns {ReturnType<typeof defaultCreatePinsThreadsPanels>}
   */
  function getPinsThreads() {
    if (!pinsThreadsInstance) {
      pinsThreadsInstance = createPinsThreadsPanels({
        document: doc,
        container: container ?? doc.querySelector(".content") ?? doc.body,
        pinsButton: pinsButton ?? doc.querySelector('[data-action="pins"]'),
        threadsButton: threadsButton ?? doc.querySelector('[data-action="threads"]'),
        loadPins,
        loadThreads,
        router,
      });
    }
    return /** @type {ReturnType<typeof defaultCreatePinsThreadsPanels>} */ (pinsThreadsInstance);
  }

  /**
   * Bind click handlers for the header pins and threads trigger buttons.
   */
  function bindPinsThreadsButtons() {
    const pinsBtn = pinsButton ?? doc.querySelector('[data-action="pins"]');
    const threadsBtn = threadsButton ?? doc.querySelector('[data-action="threads"]');
    if (pinsBtn && typeof pinsBtn.addEventListener === "function") {
      pinsBtn.addEventListener("click", () => {
        if (doc.getElementById("chat-pins-panel")) return;
        getPinsThreads()?.openPins();
      });
    }
    if (threadsBtn && typeof threadsBtn.addEventListener === "function") {
      threadsBtn.addEventListener("click", () => {
        if (doc.getElementById("chat-threads-panel")) return;
        getPinsThreads()?.openThreads();
      });
    }
  }

  return {
    getPinsThreads,
    bindPinsThreadsButtons,
  };
}
