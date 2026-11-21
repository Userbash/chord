// @ts-check
/**
 * Contract, UI DOM & 5/5 Mutation Gate for `STAGE-10.BFF.6`:
 * DaveVoiceBridge <-> `voice_connect`, `voice_state`, `voice_e2ee_status`
 * and DAVE E2EE status indication in the user's voice interface.
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import {
  VOICE_CALL_PHASE_LABELS,
  DAVE_E2EE_STATE_LABELS,
  resolveDaveE2eeIndicatorView,
  renderVoiceRtcPhaseDom,
} from "../../rust/gui-spike/ui/modules/voice-rtc-panel.js";
import { createVoiceCallController } from "../../rust/gui-spike/ui/features/voice-call-orchestrator.js";
import { createVoiceStageRenderer } from "../../rust/gui-spike/ui/features/voice-stage-renderer.js";
import { createVoicePopoutsController } from "../../rust/gui-spike/ui/features/voice-popouts-controller.js";
import { evaluateMutationGate } from "../../scripts/quality-gate.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));

/**
 * Minimal deterministic DOM element & document double for testing `#voice-panel` and `#live-rtc-popout`.
 */
function createMiniDom() {
  /** @type {Map<string, any>} */
  const byId = new Map();

  /** @param {string} tag */
  function makeEl(tag) {
    /** @type {Record<string, string>} */
    const attrs = {};
    /** @type {Record<string, Function[]>} */
    const listeners = {};
    /** @type {any[]} */
    const children = [];
    /** @type {any} */
    const el = {
      tagName: tag.toUpperCase(),
      id: "",
      className: "",
      title: "",
      type: "",
      hidden: false,
      textContent: "",
      innerHTML: "",
      dataset: {},
      style: { cssText: "", display: "", color: "", background: "" },
      parentElement: null,
      ownerDocument: null,
      children,
      setAttribute(/** @type {string} */ k, /** @type {string} */ v) {
        attrs[k] = String(v);
        if (k === "id") {
          el.id = String(v);
          byId.set(el.id, el);
        }
        if (k.startsWith("data-")) {
          const camel = k
            .slice(5)
            .replace(/-([a-z])/g, (_, c) => c.toUpperCase());
          el.dataset[camel] = String(v);
        }
      },
      getAttribute(/** @type {string} */ k) {
        if (k === "id" && el.id) return el.id;
        return Object.prototype.hasOwnProperty.call(attrs, k) ? attrs[k] : null;
      },
      hasAttribute(/** @type {string} */ k) {
        return Object.prototype.hasOwnProperty.call(attrs, k);
      },
      removeAttribute(/** @type {string} */ k) {
        delete attrs[k];
      },
      /** @param {...any} nodes */
      append(...nodes) {
        for (const n of nodes) {
          if (n && typeof n === "object") {
            n.parentElement = el;
            if (n.id) byId.set(n.id, n);
            indexSubtree(n);
            children.push(n);
          }
        }
      },
      /** @param {...any} nodes */
      replaceChildren(...nodes) {
        for (const c of children) {
          unindexSubtree(c);
        }
        children.length = 0;
        el.append(...nodes);
      },
      remove() {
        unindexSubtree(el);
        if (el.parentElement && Array.isArray(el.parentElement.children)) {
          const idx = el.parentElement.children.indexOf(el);
          if (idx >= 0) el.parentElement.children.splice(idx, 1);
        }
      },
      addEventListener(/** @type {string} */ ev, /** @type {Function} */ fn) {
        (listeners[ev] ??= []).push(fn);
      },
      click() {
        for (const fn of listeners.click ?? []) {
          fn({ stopPropagation() {}, target: el });
        }
      },
      querySelector(/** @type {string} */ sel) {
        return findInTree(el, sel);
      },
      querySelectorAll(/** @type {string} */ sel) {
        /** @type {any[]} */
        const out = [];
        collectInTree(el, sel, out);
        return out;
      },
      getBoundingClientRect() {
        return { left: 24, top: 600, width: 240, height: 52 };
      },
    };
    return el;
  }

  /** @param {any} node */
  function indexSubtree(node) {
    if (!node || typeof node !== "object") return;
    if (node.id) byId.set(node.id, node);
    for (const c of node.children ?? []) indexSubtree(c);
  }

  /** @param {any} node */
  function unindexSubtree(node) {
    if (!node || typeof node !== "object") return;
    if (node.id && byId.get(node.id) === node) byId.delete(node.id);
    for (const c of node.children ?? []) unindexSubtree(c);
  }

  /**
   * @param {any} node
   * @param {string} sel
   */
  function matchesSelector(node, sel) {
    if (!node || typeof node !== "object") return false;
    if (sel.startsWith("#")) return node.id === sel.slice(1);
    if (sel.startsWith(".")) {
      return String(node.className || "")
        .split(/\s+/)
        .includes(sel.slice(1));
    }
    const attrMatch = /^\[([a-zA-Z0-9_-]+)(?:="([^"]*)")?\]$/.exec(sel);
    if (attrMatch) {
      const val = node.getAttribute(attrMatch[1]);
      return attrMatch[2] !== undefined ? val === attrMatch[2] : val !== null;
    }
    if (sel.includes(" ")) {
      const parts = sel.trim().split(/\s+/);
      return matchesSelector(node, parts[parts.length - 1] ?? "");
    }
    return node.tagName === sel.toUpperCase();
  }

  /**
   * @param {any} root
   * @param {string} sel
   * @returns {any}
   */
  function findInTree(root, sel) {
    for (const c of root.children ?? []) {
      if (matchesSelector(c, sel)) return c;
      const nested = findInTree(c, sel);
      if (nested) return nested;
    }
    return null;
  }

  /**
   * @param {any} root
   * @param {string} sel
   * @param {any[]} out
   */
  function collectInTree(root, sel, out) {
    for (const c of root.children ?? []) {
      if (matchesSelector(c, sel)) out.push(c);
      collectInTree(c, sel, out);
    }
  }

  const body = makeEl("body");
  const doc = {
    body,
    createElement(/** @type {string} */ tag) {
      const el = makeEl(tag);
      el.ownerDocument = doc;
      return el;
    },
    getElementById(/** @type {string} */ id) {
      return byId.get(id) ?? findInTree(body, `#${id}`);
    },
    querySelector(/** @type {string} */ sel) {
      return findInTree(body, sel);
    },
    querySelectorAll(/** @type {string} */ sel) {
      /** @type {any[]} */
      const out = [];
      collectInTree(body, sel, out);
      return out;
    },
    addEventListener() {},
  };
  body.ownerDocument = doc;
  return { doc, makeEl };
}

describe("STAGE-10.BFF.6: DaveVoiceBridge UI E2EE Status Indicator & IPC Wiring", () => {
  it("1. resolveDaveE2eeIndicatorView maps all DAVE states, 30-digit privacy codes, Stage channel, and negative inputs", () => {
    assert.ok(Object.isFrozen(VOICE_CALL_PHASE_LABELS));
    assert.ok(Object.isFrozen(DAVE_E2EE_STATE_LABELS));

    // Null / disconnected
    const disc = resolveDaveE2eeIndicatorView(null);
    assert.equal(disc.state, "disconnected");
    assert.equal(disc.isEncrypted, false);
    assert.equal(disc.daveVerified, false);
    assert.equal(disc.privacyCode, null);
    assert.deepEqual(disc.privacyCodeChunks, []);

    // Sole member waiting_for_peer
    const waiting = resolveDaveE2eeIndicatorView({
      e2ee: {
        state: "waiting_for_peer",
        waitingForPeer: true,
        isEndToEndEncrypted: false,
        protocolVersion: 0,
        configuredVersion: 1,
        participantCount: 1,
      },
    });
    assert.equal(waiting.state, "waiting_for_peer");
    assert.equal(waiting.isEncrypted, false);
    assert.equal(waiting.statusColor, "#f0b232");
    assert.match(waiting.badgeLabel, /DAVE/);

    // Handshaking with peer
    const handshaking = resolveDaveE2eeIndicatorView({
      state: "handshaking",
      waitingForPeer: false,
      isEndToEndEncrypted: false,
      participantCount: 2,
    });
    assert.equal(handshaking.state, "handshaking");
    assert.equal(handshaking.isEncrypted, false);
    assert.equal(handshaking.statusColor, "#f0b232");

    // Encrypted with verified 30-digit code (6 groups of 5 digits)
    const validCode = "48291 03948 19283 74650 91827 36455";
    const encrypted = resolveDaveE2eeIndicatorView({
      e2ee: {
        state: "encrypted",
        isEndToEndEncrypted: true,
        daveVerified: true,
        protocolVersion: 1,
        epoch: 3,
        privacyCode: validCode,
      },
    });
    assert.equal(encrypted.state, "encrypted");
    assert.equal(encrypted.isEncrypted, true);
    assert.equal(encrypted.daveVerified, true);
    assert.equal(encrypted.protocolVersion, 1);
    assert.equal(encrypted.epoch, 3);
    assert.equal(encrypted.privacyCode, validCode);
    assert.deepEqual(encrypted.privacyCodeChunks, [
      "48291",
      "03948",
      "19283",
      "74650",
      "91827",
      "36455",
    ]);
    assert.equal(encrypted.statusColor, "#23a55a");

    // Contiguous 30-digit wire format from `DaveySession::voice_privacy_code` (`generate_displayable_code`)
    const contiguous = resolveDaveE2eeIndicatorView({
      state: "encrypted",
      isEndToEndEncrypted: true,
      protocolVersion: 1,
      privacyCode: "482910394819283746509182736455",
    });
    assert.equal(contiguous.daveVerified, true);
    assert.equal(contiguous.privacyCode, validCode);
    assert.deepEqual(contiguous.privacyCodeChunks, [
      "48291",
      "03948",
      "19283",
      "74650",
      "91827",
      "36455",
    ]);

    // Contiguous 30-digit privacy code from DaveySession::voice_privacy_code() (`generate_displayable_code(&auth, 30, 5)`)
    const compactEncrypted = resolveDaveE2eeIndicatorView({
      state: "encrypted",
      isEndToEndEncrypted: true,
      protocolVersion: 1,
      epoch: 1,
      privacyCode: "482910394819283746509182736455",
    });
    assert.equal(compactEncrypted.daveVerified, true);
    assert.equal(compactEncrypted.privacyCode, validCode);
    assert.deepEqual(compactEncrypted.privacyCodeChunks, [
      "48291",
      "03948",
      "19283",
      "74650",
      "91827",
      "36455",
    ]);

    // Malformed / truncated / wrong-grouped privacy code is rejected (`daveVerified = false`, `privacyCode = null`)
    const badCode = resolveDaveE2eeIndicatorView({
      state: "encrypted",
      isEndToEndEncrypted: true,
      protocolVersion: 1,
      privacyCode: "12345 67890",
    });
    assert.equal(badCode.isEncrypted, true);
    assert.equal(badCode.daveVerified, false);
    assert.equal(badCode.privacyCode, null);

    const wrongGrouping = resolveDaveE2eeIndicatorView({
      state: "encrypted",
      isEndToEndEncrypted: true,
      protocolVersion: 1,
      privacyCode: "4829103948 1928374650 9182736455",
    });
    assert.equal(wrongGrouping.daveVerified, false);
    assert.equal(wrongGrouping.privacyCode, null);

    // Downgrade blocked warning
    const downgrade = resolveDaveE2eeIndicatorView({
      state: "downgrade_blocked",
      downgradeBlocked: true,
      isEndToEndEncrypted: false,
      protocolVersion: 0,
    });
    assert.equal(downgrade.state, "downgrade_blocked");
    assert.equal(downgrade.statusColor, "#f23f43");
  });

  it("2. renderVoiceRtcPhaseDom and voice-stage-renderer update #live-voice-e2ee-pill in #voice-panel", () => {
    const { doc } = createMiniDom();
    const prevDoc = globalThis.document;
    const prevHtmlEl = globalThis.HTMLElement;
    /** @type {any} */ (globalThis).document = doc;
    /** @type {any} */ (globalThis).HTMLElement = Object;
    try {
      const voiceHost = doc.createElement("div");
      voiceHost.setAttribute("id", "voice-panel");
      doc.body.append(voiceHost);

      let rtcStatus = /** @type {any} */ ({
        state: "connected",
        phase: "connected",
        lastPingMs: 22,
        averagePingMs: 24,
        packetLossPct: 0,
        e2ee: {
          state: "waiting_for_peer",
          waitingForPeer: true,
          isEndToEndEncrypted: false,
          protocolVersion: 0,
          configuredVersion: 1,
          privacyCode: null,
        },
      });

      const activeConn = {
        guildId: "111",
        guildName: "Chord Lab",
        channelId: "222",
        channelName: "Voice General",
        muted: false,
        deafened: false,
        cameraOn: false,
        screenShareOn: false,
      };
      /** @type {Array<string | undefined>} */
      const openedPopoutTabs = [];

      const renderer = createVoiceStageRenderer({
        getVoiceHost: () => voiceHost,
        voiceHost,
        isLiveSession: () => true,
        getLiveSelfUser: () => ({ id: "100" }),
        currentGuildInfo: () => ({ id: "111" }),
        selectedGuildId: () => "111",
        isChannelAfk: () => false,
        getVoiceParticipants: () => [],
        getActiveVoiceConnection: () => activeConn,
        getLiveRtcStatus: () => rtcStatus,
        selfVoicePrefs: { muted: false, deafened: false, krispEnabled: true },
        syncFloatingStreamPipPreview() {},
        renderLiveDmCallStage() {},
        canStreamInChannel: () => true,
        openRtcConnectionPopout(/** @type {string | undefined} */ tab) {
          openedPopoutTabs.push(tab);
        },
        openKrispNoisePopout() {},
        disconnectFromLiveVoiceChannel() {},
        retryActiveVoiceConnection() {},
        computeRtcSignalMetrics: (/** @type {any} */ st) => ({
          phaseView: {
            phase: st?.phase || "connected",
            label: VOICE_CALL_PHASE_LABELS.connected,
            isConnected: true,
            isError: false,
            canRetry: false,
            statusColor: "#23a55a",
          },
          isErr: false,
          isConnecting: false,
          hasMeasuredPing: true,
          effectiveLast: 22,
          effectiveAvg: 24,
          lossPct: 0,
          lossTxt: "0.0%",
          quality: "fine",
          pingColor: "#23a55a",
          signalSvg: "<svg></svg>",
          pillText: "22 мс",
        }),
      });

      renderer.renderLiveVoicePanel();
      const e2eePill = doc.getElementById("live-voice-e2ee-pill");
      assert.ok(e2eePill, "#live-voice-e2ee-pill must be rendered inside #voice-panel");
      assert.equal(e2eePill.getAttribute("data-dave-e2ee-state"), "waiting_for_peer");
      assert.equal(e2eePill.style.color, "#f0b232");

      // Clicking #live-voice-e2ee-pill opens RTC popout directly on the Privacy tab
      e2eePill.click();
      assert.deepEqual(openedPopoutTabs, ["privacy"]);

      // Upgrade to encrypted MLS epoch 2 -> in-place DOM update turns green and marks verified
      rtcStatus = {
        ...rtcStatus,
        e2ee: {
          state: "encrypted",
          waitingForPeer: false,
          isEndToEndEncrypted: true,
          daveVerified: true,
          protocolVersion: 1,
          epoch: 2,
          privacyCode: "11111 22222 33333 44444 55555 66666",
        },
      };
      renderer.updateLiveRtcDomInPlace();
      assert.equal(e2eePill.getAttribute("data-dave-e2ee-state"), "encrypted");
      assert.equal(e2eePill.getAttribute("data-dave-verified"), "true");
      assert.equal(e2eePill.getAttribute("data-dave-epoch"), "2");
      assert.equal(e2eePill.style.color, "#23a55a");
      assert.equal(voiceHost.getAttribute("data-dave-e2ee-state"), "encrypted");

      // Disconnected clears DAVE state on panel
      renderVoiceRtcPhaseDom(voiceHost, { phase: "disconnected" }, { doc });
      assert.equal(voiceHost.getAttribute("data-dave-e2ee-state"), "disconnected");
    } finally {
      /** @type {any} */ (globalThis).document = prevDoc;
      /** @type {any} */ (globalThis).HTMLElement = prevHtmlEl;
    }
  });

  it("3. openRtcConnectionPopout Privacy tab displays real DaveVoiceBridge privacy code and invokes voice_e2ee_status", async () => {
    const { doc } = createMiniDom();
    const prevDoc = globalThis.document;
    const prevWin = globalThis.window;
    /** @type {any} */ (globalThis).document = doc;
    /** @type {any} */ (globalThis).window = { innerHeight: 900, innerWidth: 1280 };
    try {
      /** @type {Array<{cmd: string, args: any}>} */
      const ipcCalls = [];
      const realCode = "90123 45678 91234 56789 01234 56780";
      let liveRtcStatus = /** @type {any} */ ({
        state: "connected",
        ssrc: 4242,
        e2ee: {
          state: "encrypted",
          isEndToEndEncrypted: true,
          daveVerified: true,
          protocolVersion: 1,
          epoch: 4,
          privacyCode: realCode,
          privacyCodeChunks: realCode.split(" "),
        },
      });

      const popouts = createVoicePopoutsController({
        voiceHost: null,
        VOICE_E2EE_STATUS_IPC_CMD: "voice_e2ee_status",
        VOICE_SET_AUDIO_DEVICE_IPC_CMD: "voice_set_audio_device",
        isLiveSession: () => true,
        closeLiveVoicePopouts() {},
        getLiveRtcStatus: () => liveRtcStatus,
        setLiveRtcStatus: (/** @type {any} */ next) => {
          liveRtcStatus = next;
        },
        getActiveVoiceConnection: () => ({ guildId: "111", channelId: "222" }),
        getVoiceParticipants: () => [],
        computeRtcSignalMetrics: () => ({
          isErr: false,
          isConnecting: false,
          quality: "fine",
          pingColor: "#23a55a",
          pillText: "19 мс",
        }),
        updateLiveRtcDomInPlace() {},
        invoke: async (/** @type {string} */ cmd, /** @type {any} */ args) => {
          ipcCalls.push({ cmd, args });
          return liveRtcStatus.e2ee;
        },
      });

      // Verify Integration tab footer does NOT lie when E2EE is still waiting_for_peer
      const origE2ee = liveRtcStatus.e2ee;
      liveRtcStatus = {
        ...liveRtcStatus,
        e2ee: {
          state: "waiting_for_peer",
          waitingForPeer: true,
          isEndToEndEncrypted: false,
          protocolVersion: 0,
          configuredVersion: 1,
        },
      };
      popouts.openRtcConnectionPopout();
      const popWaiting = doc.getElementById("live-rtc-popout");
      assert.ok(popWaiting, "RTC connection popout must mount");
      const waitingFooter = popWaiting.querySelector(".rtc-popout-footer");
      assert.ok(waitingFooter, "Integration tab must render .rtc-popout-footer");
      assert.equal(
        waitingFooter.getAttribute("data-dave-e2ee-state"),
        "waiting_for_peer",
        "Integration tab footer must reflect waiting_for_peer state",
      );
      assert.doesNotMatch(
        waitingFooter.textContent,
        /Защищено сквозным шифрованием/,
        "Integration tab footer must not falsely claim E2EE while waiting_for_peer",
      );
      // Close popout
      popouts.openRtcConnectionPopout();

      // Restore encrypted state and open directly on "privacy" tab via openRtcConnectionPopout("privacy")
      liveRtcStatus = { ...liveRtcStatus, e2ee: origE2ee };
      popouts.openRtcConnectionPopout("privacy");
      const pop = doc.getElementById("live-rtc-popout");
      assert.ok(pop, "RTC connection popout must mount on privacy tab");

      const tabs = pop.querySelectorAll(".rtc-popout-tab");
      assert.equal(tabs.length, 2);
      assert.equal(tabs[1].getAttribute("aria-selected"), "true");
      await Promise.resolve();

      assert.ok(
        ipcCalls.some((c) => c.cmd === "voice_e2ee_status"),
        "Privacy tab must query voice_e2ee_status IPC command",
      );

      // Verify all 6 cryptographic 5-digit blocks from DaveVoiceBridge are rendered
      const spans = pop.querySelectorAll("span");
      const renderedDigits = spans.map((/** @type {any} */ s) => s.textContent);
      for (const expectedChunk of realCode.split(" ")) {
        assert.ok(
          renderedDigits.includes(expectedChunk),
          `Privacy tab must render real DaveVoiceBridge chunk ${expectedChunk}, got ${renderedDigits.join(",")}`,
        );
      }
    } finally {
      /** @type {any} */ (globalThis).document = prevDoc;
      /** @type {any} */ (globalThis).window = prevWin;
    }
  });

  it("4. createVoiceCallController wires voice_connect, voice_state, and voice_e2ee_status IPC commands", async () => {
    const { doc } = createMiniDom();
    const prevDoc = globalThis.document;
    const prevHtmlEl = globalThis.HTMLElement;
    /** @type {any} */ (globalThis).document = doc;
    /** @type {any} */ (globalThis).HTMLElement = Object;
    /** @type {Array<{cmd: string, args: any}>} */
    const calls = [];
    let activeConn = /** @type {any} */ (null);
    let rtcStatus = /** @type {any} */ ({});

    try {
      const controller = createVoiceCallController({
      isLiveSession: () => true,
      cleanDiscordStr: (/** @type {any} */ s) => String(s ?? "").trim(),
      normalizeVoiceGuildKey: (/** @type {any} */ g) => String(g || "@me"),
      isChannelAfk: () => false,
      getLiveSelfUser: () => ({ id: "100000000000000001" }),
      getActiveVoiceConnection: () => activeConn,
      setActiveVoiceConnection: (/** @type {any} */ c) => {
        activeConn = c;
      },
      getLiveRtcStatus: () => rtcStatus,
      setLiveRtcStatus: (/** @type {any} */ s) => {
        rtcStatus = s;
      },
      selfVoicePrefs: {
        muted: false,
        deafened: false,
        krispEnabled: true,
        inputDeviceId: "default",
        outputDeviceId: "default",
        inputVolume: 100,
        outputVolume: 100,
      },
      liveVoiceStatesByGuild: new Map(),
      liveVoiceChannelStartEpochMs: new Map(),
      liveGuildsById: new Map(),
      liveUsersById: new Map(),
      computeEffectiveChannelPermissions: () => null,
      byId: () => null,
      playVoiceUiSound() {},
      showVoiceToastNotice() {},
      closeLiveVoicePopouts() {},
      cancelPipAnimTimer() {},
      getGlobalAppReactiveStore: () => ({
        activeVoiceConnection: { value: null },
        applyLivePushPayload() {},
      }),
      createTauriEventListener: (/** @type {any} */ cfg) => {
        if (typeof cfg.initialFetch === "function") {
          void cfg.initialFetch();
        }
        return { stop() {} };
      },
      computeRtcSignalMetrics: () => ({
        phaseView: { phase: "signaling", label: "", isConnected: false, isError: false, canRetry: false, statusColor: "#f0b232" },
        quality: "fine",
        pingColor: "#23a55a",
        signalSvg: "",
        pillText: "—",
        effectiveLast: 0,
        effectiveAvg: 0,
        lossPct: 0,
        lossTxt: "0%",
        hasMeasuredPing: false,
        isErr: false,
        isConnecting: true,
      }),
      invoke: async (/** @type {string} */ cmd, /** @type {any} */ args) => {
        calls.push({ cmd, args });
        if (cmd === "voice_connect") {
          return {
            state: "signaling",
            phase: "signaling",
            e2ee: { state: "waiting_for_peer", waitingForPeer: true, isEndToEndEncrypted: false },
          };
        }
        if (cmd === "voice_e2ee_status") {
          return {
            state: "encrypted",
            isEndToEndEncrypted: true,
            daveVerified: true,
            protocolVersion: 1,
            epoch: 1,
            privacyCode: "12345 67890 12345 67890 12345 67890",
          };
        }
        if (cmd === "voice_set_audio_device" && args?.kind === "voice_states_sync") {
          return {
            voiceStates: {
              "777": [
                { user_id: "100000000000000001", channel_id: "888" },
                { user_id: "200000000000000002", channel_id: "888" },
              ],
            },
          };
        }
        return { state: "connected", phase: "connected" };
      },
      mergeVoiceStatesSnapshot: (/** @type {Map<string, any[]>} */ map, /** @type {any} */ raw) => {
        for (const [k, v] of Object.entries(raw)) map.set(k, /** @type {any[]} */ (v));
        return "digest-1";
      },
    });

    controller.connectToLiveVoiceChannel("777", "Guild 777", { id: "888", name: "Voice 888" });
    await new Promise((r) => setTimeout(r, 15));
    await controller.refreshLiveReadyVoiceAndUsers();
    await new Promise((r) => setTimeout(r, 15));

      const invokedNames = calls.map((c) => c.cmd);
      assert.ok(invokedNames.includes("voice_connect"), "must invoke voice_connect");
      assert.ok(invokedNames.includes("voice_e2ee_status"), "must invoke voice_e2ee_status");
      assert.ok(
        invokedNames.includes("voice_state"),
        `must invoke voice_state when peer voice_states change, got: ${invokedNames.join(", ")}`,
      );
      const voiceStateCountBeforeDisconnect = calls.filter((c) => c.cmd === "voice_state").length;
      assert.equal(voiceStateCountBeforeDisconnect, 1);

      // Disconnect clears rtcStatus.e2ee and resets lastSyncedPeerRosterKey so rejoining syncs peers again
      controller.disconnectFromLiveVoiceChannel();
      assert.equal(rtcStatus.e2ee, null, "disconnectFromLiveVoiceChannel must clear stale e2ee status");

      controller.connectToLiveVoiceChannel("777", "Guild 777", { id: "888", name: "Voice 888" });
      await new Promise((r) => setTimeout(r, 15));
      await controller.refreshLiveReadyVoiceAndUsers();
      await new Promise((r) => setTimeout(r, 15));

      const voiceStateCountAfterReconnect = calls.filter((c) => c.cmd === "voice_state").length;
      assert.equal(
        voiceStateCountAfterReconnect,
        2,
        "reconnecting to same channel with same peer must re-invoke voice_state",
      );
    } finally {
      /** @type {any} */ (globalThis).document = prevDoc;
      /** @type {any} */ (globalThis).HTMLElement = prevHtmlEl;
    }
  });

  it("5. kills 5/5 control mutations on resolveDaveE2eeIndicatorView (100% mutation gate)", () => {
    const panelPath = join(ROOT, "rust/gui-spike/ui/modules/voice-rtc-panel.js");
    const reactiveStorePath = join(ROOT, "rust/gui-spike/ui/features/reactive-store.js");
    const src = readFileSync(panelPath, "utf8");
    const scratch = mkdtempSync(join(tmpdir(), "dave-ui-mut-"));

    try {
      const mutants = [
        {
          id: "MUT-UI-DAVE-01",
          description: "Force isEncrypted true even when isEndToEndEncrypted is false",
          target: "const isEncrypted = Boolean(",
          replacement: "const isEncrypted = true || Boolean(",
          assertion:
            "assert.equal(resolveDaveE2eeIndicatorView({ state: 'waiting_for_peer', isEndToEndEncrypted: false }).isEncrypted, false);",
        },
        {
          id: "MUT-UI-DAVE-02",
          description: "Accept malformed privacy code without 6x5 digit validation",
          target: "chunks.length === 6 && chunks.every((c) => /^\\d{5}$/.test(c))",
          replacement: "chunks.length >= 1",
          assertion:
            "assert.equal(resolveDaveE2eeIndicatorView({ state: 'encrypted', isEndToEndEncrypted: true, protocolVersion: 1, privacyCode: '123 456 789 012 345 678' }).daveVerified, false);",
        },
        {
          id: "MUT-UI-DAVE-03",
          description: "Swap encrypted color #23a55a to #80848e",
          target: 'encrypted: "#23a55a"',
          replacement: 'encrypted: "#80848e"',
          assertion:
            "assert.equal(resolveDaveE2eeIndicatorView({ state: 'encrypted', isEndToEndEncrypted: true, protocolVersion: 1 }).statusColor, '#23a55a');",
        },
        {
          id: "MUT-UI-DAVE-04",
          description: "Ignore downgradeBlocked flag and treat as waiting_for_peer",
          target: "if (downgradeBlocked || rawState === \"downgrade_blocked\")",
          replacement: "if (false && (downgradeBlocked || rawState === \"downgrade_blocked\"))",
          assertion:
            "assert.equal(resolveDaveE2eeIndicatorView({ downgradeBlocked: true }).state, 'downgrade_blocked');",
        },
        {
          id: "MUT-UI-DAVE-05",
          description: "Suppress Stage channel privacyCode hiding check",
          target: "const isStage = Boolean(raw.isStageChannel || raw.isStageVoiceChannel);",
          replacement: "const isStage = false;",
          assertion:
            "assert.equal(resolveDaveE2eeIndicatorView({ state: 'encrypted', isEndToEndEncrypted: true, protocolVersion: 1, isStageChannel: true, privacyCode: '11111 22222 33333 44444 55555 66666' }).daveVerified, false);",
        },
      ];

      const mutantSpecs = mutants.map((m) => {
        assert.ok(src.includes(m.target), `Mutation target missing in voice-rtc-panel.js: ${m.id}`);
        const mutated = src
          .replace("../features/reactive-store.js", reactiveStorePath)
          .replace(m.target, m.replacement);
        return {
          id: m.id,
          moduleName: "voice-rtc-panel.js",
          operator: m.description,
          mutatedSource: mutated,
          assertion: m.assertion,
        };
      });

      const gate = evaluateMutationGate(mutantSpecs, (m, idx) => {
        const spec = mutantSpecs[idx];
        assert.ok(spec);
        const mutFile = join(scratch, `${m.id}.mjs`);
        writeFileSync(mutFile, m.mutatedSource, "utf8");
        const probe = `import assert from "node:assert/strict"; import { resolveDaveE2eeIndicatorView } from ${JSON.stringify(mutFile)}; ${spec.assertion}`;
        const res = spawnSync(process.execPath, ["--input-type=module", "-e", probe], {
          encoding: "utf8",
        });
        return {
          killed: res.status !== 0,
          reason: res.status !== 0 ? "killed by Stage-10.BFF.6 assertion" : "survived",
        };
      });
      assert.equal(gate.passed, true, `Mutation gate failed: ${JSON.stringify(gate)}`);
      assert.equal(gate.killed, 5);
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  });
});
