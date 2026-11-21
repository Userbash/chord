// @ts-check

/**
 * Voice & Call Orchestrator (`RW51-EPIC2-T2`).
 * Composes all modular voice/call sub-controllers around a shared mutable context
 * and exposes the unified API surface consumed by `app.js`.
 */

import { createVoiceStateSync } from "./voice-state-sync.js";
import { createVoiceConnectionController } from "./voice-connection-controller.js";
import { createVoiceDmCallStage } from "./voice-dm-call-stage.js";
import { createVoiceCallStage } from "./voice-call-stage.js";
import { createVoiceStageRenderer } from "./voice-stage-renderer.js";
import { createVoiceParticipantMenu } from "./voice-participant-menu.js";
import { createVoiceInviteAndDevicesController } from "./voice-invite-and-devices.js";
import { createVoicePopoutsController } from "./voice-popouts-controller.js";
import { createVoiceStageViewer } from "./voice-stage-viewer.js";
import { createVoiceStreamMenus } from "./voice-stream-menus.js";
import { createVoiceStreamController } from "./voice-stream-controller.js";

/**
 * Create the unified voice and call controller.
 * @param {any} deps
 */
export function createVoiceCallController(deps) {
  // Internal state shared across voice/call sub-controllers
  let previewedVoiceChannel = /** @type {any} */ (null);
  let activeVoiceTimerInterval = /** @type {any} */ (null);
  let activeVoiceStatusTimer = /** @type {any} */ (null);
  let activeAudioLevelsTimer = /** @type {any} */ (null);
  let activeAfkCheckTimer = /** @type {any} */ (null);
  let lastLocalUserActivityAtMs = Date.now();
  let userActivityTrackingAttached = false;
  let lastAutoMovedAfkKey = "";
  let hotplugToastCooldownMs = 0;
  let audioLevelPollTicks = 0;
  let activeAudioLevelsPoller = /** @type {any} */ (null);
  let activeRtcStatusPoller = /** @type {any} */ (null);
  let voiceReadySyncInFlight = false;
  let lastVoiceReadySyncAtMs = 0;
  let activeVoiceMenuOutsideHandler = /** @type {any} */ (null);
  let hideVoiceActivityPopoutTimer = /** @type {any} */ (null);
  let micTestTimer = /** @type {any} */ (null);
  let sharedSoundboardAudioCtx = /** @type {any} */ (null);
  let ringtoneTimer = /** @type {any} */ (null);
  let activePipAnimTimer = /** @type {any} */ (null);
  let activePipDragCleanup = /** @type {(() => void) | null} */ (null);
  let floatingPipPosition = /** @type {{ left: number, top: number } | null} */ (null);
  let cachedLiveScreenImg = /** @type {HTMLImageElement | null} */ (null);
  let cachedLiveScreenDataUrl = "";
  let lastLiveScreenSeq = 0;
  let liveScreenCaptureInFlight = false;
  let lastLiveScreenCaptureAtMs = 0;

  const ipcCmds = deps.ipcCmds ?? {};

  /** @type {any} */
  const ctx = {
    // Pass-through helpers and stores from app.js
    ...deps,
    ipcCmds: {
      setAudioDevice: "voice_set_audio_device",
      audioDevices: "voice_audio_devices",
      rtcStatus: "voice_rtc_status",
      voiceState: "voice_state",
      e2eeStatus: "voice_e2ee_status",
      connect: "voice_connect",
      disconnect: "voice_disconnect",
      ...ipcCmds,
    },
    VOICE_SET_AUDIO_DEVICE_IPC_CMD:
      deps.VOICE_SET_AUDIO_DEVICE_IPC_CMD ?? ipcCmds.setAudioDevice ?? "voice_set_audio_device",
    VOICE_AUDIO_DEVICES_IPC_CMD:
      deps.VOICE_AUDIO_DEVICES_IPC_CMD ?? ipcCmds.audioDevices ?? "voice_audio_devices",
    VOICE_RTC_STATUS_IPC_CMD:
      deps.VOICE_RTC_STATUS_IPC_CMD ?? ipcCmds.rtcStatus ?? "voice_rtc_status",
    VOICE_STATE_IPC_CMD:
      deps.VOICE_STATE_IPC_CMD ?? ipcCmds.voiceState ?? "voice_state",
    VOICE_E2EE_STATUS_IPC_CMD:
      deps.VOICE_E2EE_STATUS_IPC_CMD ?? ipcCmds.e2eeStatus ?? "voice_e2ee_status",
    VOICE_CONNECT_IPC_CMD:
      deps.VOICE_CONNECT_IPC_CMD ?? ipcCmds.connect ?? "voice_connect",
    VOICE_DISCONNECT_IPC_CMD:
      deps.VOICE_DISCONNECT_IPC_CMD ?? ipcCmds.disconnect ?? "voice_disconnect",

    // Aliases for SVG icons
    SVG_ICONS: {
      micOn: "",
      micOff: "",
      deafOn: "",
      deafOff: "",
      chevronDown: "",
      signalBars: "",
      signalFine: "",
      signalAverage: "",
      signalBad: "",
      ...(deps.SVG_ICONS ?? {}),
    },
    SVG_MIC_ON: deps.SVG_MIC_ON ?? deps.SVG_ICONS?.micOn ?? "",
    SVG_MIC_OFF: deps.SVG_MIC_OFF ?? deps.SVG_ICONS?.micOff ?? "",
    SVG_DEAF_ON: deps.SVG_DEAF_ON ?? deps.SVG_ICONS?.deafOn ?? "",
    SVG_DEAF_OFF: deps.SVG_DEAF_OFF ?? deps.SVG_ICONS?.deafOff ?? "",
    SVG_CHEVRON_DOWN: deps.SVG_CHEVRON_DOWN ?? deps.SVG_ICONS?.chevronDown ?? "",
    SVG_SIGNAL_BARS: deps.SVG_SIGNAL_BARS ?? deps.SVG_ICONS?.signalBars ?? "",
    SVG_SIGNAL_FINE: deps.SVG_SIGNAL_FINE ?? deps.SVG_ICONS?.signalFine ?? "",
    SVG_SIGNAL_AVERAGE: deps.SVG_SIGNAL_AVERAGE ?? deps.SVG_ICONS?.signalAverage ?? "",
    SVG_SIGNAL_BAD: deps.SVG_SIGNAL_BAD ?? deps.SVG_ICONS?.signalBad ?? "",

    // User bar buttons & DOM hosts
    uMute: deps.uMute ?? deps.USER_BAR_BTNS?.mute ?? null,
    uDeaf: deps.uDeaf ?? deps.USER_BAR_BTNS?.deaf ?? null,
    uSettings: deps.uSettings ?? deps.USER_BAR_BTNS?.settings ?? null,
    lastVoiceUiSoundAtByKind: deps.lastVoiceUiSoundAtByKind ?? new Map(),
    getVoiceHost: () =>
      typeof deps.getVoiceHost === "function"
        ? deps.getVoiceHost()
        : deps.voiceHost ?? (typeof document !== "undefined" ? document.getElementById("voice-panel") : null),
    get voiceHost() {
      return typeof deps.getVoiceHost === "function"
        ? deps.getVoiceHost()
        : deps.voiceHost ?? (typeof document !== "undefined" ? document.getElementById("voice-panel") : null);
    },
    dmChatHost: deps.dmChatHost ?? null,
    dmHomeHost: deps.dmHomeHost ?? null,
    get tree() {
      return (typeof deps.getTree === "function" ? deps.getTree() : deps.tree) ?? { refreshView: deps.renderChannelTree ?? (() => {}), channels: [] };
    },
    get rail() {
      return (typeof deps.getRail === "function" ? deps.getRail() : deps.rail) ?? { selectedId: deps.getRailSelectedId?.() ?? "@me" };
    },
    get router() {
      return (typeof deps.getRouter === "function" ? deps.getRouter() : deps.router) ?? { switchScreen: () => {} };
    },
    liveGuildsById: deps.liveGuildsById ?? new Map(),

    // Internal shared state getters/setters
    getPreviewedVoiceChannel: () => previewedVoiceChannel,
    setPreviewedVoiceChannel: (/** @type {any} */ v) => {
      previewedVoiceChannel = v;
    },
    getActiveVoiceTimerInterval: () => activeVoiceTimerInterval,
    setActiveVoiceTimerInterval: (/** @type {any} */ v) => {
      activeVoiceTimerInterval = v;
    },
    getActiveVoiceStatusTimer: () => activeVoiceStatusTimer,
    setActiveVoiceStatusTimer: (/** @type {any} */ v) => {
      activeVoiceStatusTimer = v;
    },
    getActiveAudioLevelsTimer: () => activeAudioLevelsTimer,
    setActiveAudioLevelsTimer: (/** @type {any} */ v) => {
      activeAudioLevelsTimer = v;
    },
    getActiveAfkCheckTimer: () => activeAfkCheckTimer,
    setActiveAfkCheckTimer: (/** @type {any} */ v) => {
      activeAfkCheckTimer = v;
    },
    getLastLocalUserActivityAtMs: () => lastLocalUserActivityAtMs,
    setLastLocalUserActivityAtMs: (/** @type {number} */ v) => {
      lastLocalUserActivityAtMs = v;
    },
    getUserActivityTrackingAttached: () => userActivityTrackingAttached,
    setUserActivityTrackingAttached: (/** @type {boolean} */ v) => {
      userActivityTrackingAttached = v;
    },
    getLastAutoMovedAfkKey: () => lastAutoMovedAfkKey,
    setLastAutoMovedAfkKey: (/** @type {string} */ v) => {
      lastAutoMovedAfkKey = v;
    },
    getHotplugToastCooldownMs: () => hotplugToastCooldownMs,
    setHotplugToastCooldownMs: (/** @type {number} */ v) => {
      hotplugToastCooldownMs = v;
    },
    getAudioLevelPollTicks: () => audioLevelPollTicks,
    setAudioLevelPollTicks: (/** @type {number} */ v) => {
      audioLevelPollTicks = v;
    },
    getActiveAudioLevelsPoller: () => activeAudioLevelsPoller,
    setActiveAudioLevelsPoller: (/** @type {any} */ v) => {
      activeAudioLevelsPoller = v;
    },
    getActiveRtcStatusPoller: () => activeRtcStatusPoller,
    setActiveRtcStatusPoller: (/** @type {any} */ v) => {
      activeRtcStatusPoller = v;
    },
    getVoiceReadySyncInFlight: () => voiceReadySyncInFlight,
    setVoiceReadySyncInFlight: (/** @type {boolean} */ v) => {
      voiceReadySyncInFlight = v;
    },
    getLastVoiceReadySyncAtMs: () => lastVoiceReadySyncAtMs,
    setLastVoiceReadySyncAtMs: (/** @type {number} */ v) => {
      lastVoiceReadySyncAtMs = v;
    },
    getActiveVoiceMenuOutsideHandler: () => activeVoiceMenuOutsideHandler,
    setActiveVoiceMenuOutsideHandler: (/** @type {any} */ v) => {
      activeVoiceMenuOutsideHandler = v;
    },
    getHideVoiceActivityPopoutTimer: () => hideVoiceActivityPopoutTimer,
    setHideVoiceActivityPopoutTimer: (/** @type {any} */ v) => {
      hideVoiceActivityPopoutTimer = v;
    },
    getMicTestTimer: () => micTestTimer,
    setMicTestTimer: (/** @type {any} */ v) => {
      micTestTimer = v;
    },
    getSharedSoundboardAudioCtx: () => sharedSoundboardAudioCtx,
    setSharedSoundboardAudioCtx: (/** @type {any} */ v) => {
      sharedSoundboardAudioCtx = v;
    },
    getRingtoneTimer: () => ringtoneTimer,
    setRingtoneTimer: (/** @type {any} */ v) => {
      ringtoneTimer = v;
    },
    getActivePipAnimTimer: () => activePipAnimTimer,
    setActivePipAnimTimer: (/** @type {any} */ v) => {
      activePipAnimTimer = v;
    },
    getActivePipDragCleanup: () => activePipDragCleanup,
    setActivePipDragCleanup: (/** @type {(() => void) | null} */ v) => {
      activePipDragCleanup = v;
    },
    getFloatingPipPosition: () => floatingPipPosition,
    setFloatingPipPosition: (/** @type {{ left: number, top: number } | null} */ v) => {
      floatingPipPosition = v;
    },
    getCachedLiveScreenImg: () => cachedLiveScreenImg,
    setCachedLiveScreenImg: (/** @type {HTMLImageElement | null} */ v) => {
      cachedLiveScreenImg = v;
    },
    getCachedLiveScreenDataUrl: () => cachedLiveScreenDataUrl,
    setCachedLiveScreenDataUrl: (/** @type {string} */ v) => {
      cachedLiveScreenDataUrl = v;
    },
    getLastLiveScreenSeq: () => lastLiveScreenSeq,
    setLastLiveScreenSeq: (/** @type {number} */ v) => {
      lastLiveScreenSeq = v;
    },
    getLiveScreenCaptureInFlight: () => liveScreenCaptureInFlight,
    setLiveScreenCaptureInFlight: (/** @type {boolean} */ v) => {
      liveScreenCaptureInFlight = v;
    },
    getLastLiveScreenCaptureAtMs: () => lastLiveScreenCaptureAtMs,
    setLastLiveScreenCaptureAtMs: (/** @type {number} */ v) => {
      lastLiveScreenCaptureAtMs = v;
    },
  };

  // Instantiate all sub-controllers and bind their methods onto `ctx`
  const stateSync = createVoiceStateSync(ctx);
  Object.assign(ctx, stateSync);

  const connectionCtrl = createVoiceConnectionController(ctx);
  Object.assign(ctx, connectionCtrl);

  const dmCallStage = createVoiceDmCallStage(ctx);
  Object.assign(ctx, dmCallStage);

  const stageRenderer = createVoiceStageRenderer(ctx);
  Object.assign(ctx, stageRenderer);

  const participantMenu = createVoiceParticipantMenu(ctx);
  Object.assign(ctx, participantMenu);

  const inviteAndDevices = createVoiceInviteAndDevicesController(ctx);
  Object.assign(ctx, inviteAndDevices);

  const popoutsCtrl = createVoicePopoutsController(ctx);
  Object.assign(ctx, popoutsCtrl);

  const stageViewer = createVoiceStageViewer(ctx);
  Object.assign(ctx, stageViewer);

  const streamMenus = createVoiceStreamMenus(ctx);
  Object.assign(ctx, streamMenus);

  const streamCtrl = createVoiceStreamController(ctx);
  Object.assign(ctx, streamCtrl);

  return {
    ...stateSync,
    ...connectionCtrl,
    ...dmCallStage,
    ...stageRenderer,
    ...participantMenu,
    ...inviteAndDevices,
    ...popoutsCtrl,
    ...stageViewer,
    ...streamMenus,
    ...streamCtrl,
    createVoiceCallStage,
    getPreviewedVoiceChannel: ctx.getPreviewedVoiceChannel,
    setPreviewedVoiceChannel: ctx.setPreviewedVoiceChannel,
    setFloatingPipPosition: ctx.setFloatingPipPosition,
  };
}
