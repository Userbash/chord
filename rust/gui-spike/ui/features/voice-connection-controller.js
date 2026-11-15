// @ts-check
/**
 * Voice RTC connection lifecycle, mute/deafen synchronization, audio device level polling,
 * AFK timeout detection, and voice timer cleanup.
 * Extracted from `app.js` as part of RW51-EPIC2-T2.
 * @module ui/features/voice-connection-controller
 */

import { UIError } from "./errors.js";
import { HOME_GUILD_ID } from "./dm-friends.js";
import {
  applyVoiceStateUpdateEvent,
  isConfirmedVoiceStateRecord,
  syncServerAccessBannersAndComposerDom,
} from "../modules/channel-tree-view.js";

/**
 * Create the voice connection lifecycle controller bound to the shared voice context.
 * @param {any} ctx
 */
export function createVoiceConnectionController(ctx) {
  let rtcPollInFlight = false;
  let audioDeviceLevelsInFlight = false;
  let lastAudioDeviceSyncAtMs = 0;
  let lastAudioDeviceInputConfig = { deviceId: "", volume: -1, muted: /** @type {boolean | null} */ (null) };
  let lastAudioDeviceOutputConfig = { deviceId: "", volume: -1, muted: /** @type {boolean | null} */ (null) };
  let lastVoiceConnectState = {
    guildId: /** @type {string | null} */ (null),
    channelId: /** @type {string | null} */ (null),
    selfMute: /** @type {boolean | null} */ (null),
    selfDeaf: /** @type {boolean | null} */ (null),
    selfVideo: /** @type {boolean | null} */ (null),
    selfStream: /** @type {boolean | null} */ (null),
  };
  let highPingStrikes = 0;
  let lastAutoReconnectMs = 0;
  let lastUserActivityAtMs = Date.now();
  let userActivityListenersBound = false;
  let refreshLiveReadyInFlight = false;
  let lastVoiceStatesDigest = "";
  let syncTickCounter = 0;
  let lastDmFriendsSyncAt = 0;

  /**
   * Synchronizes local mic/speaker/camera/stream state with Discord Gateway (OP 4 / OP 18)
   * and native audio backend without spamming `voice_set_audio_device` when unchanged.
   * @param {boolean} [force]
   */
  function syncVoiceMuteStateWithBackend(force = false) {
    if (!ctx.isLiveSession()) return;
    const selfId = ctx.cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
    const activeConn = ctx.getActiveVoiceConnection();
    const isCurrAfk = Boolean(
      activeConn &&
        ctx.isChannelAfk(activeConn.guildId, activeConn.channelId, activeConn.channelName),
    );
    const effectiveMute = Boolean(ctx.selfVoicePrefs.muted || isCurrAfk);
    let fallbackGid = null;
    let fallbackCid = null;
    for (const [gidKey, arr] of ctx.liveVoiceStatesByGuild.entries()) {
      for (const v of arr) {
        if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
          v.self_mute = effectiveMute;
          v.selfMute = effectiveMute;
          v.self_deaf = ctx.selfVoicePrefs.deafened;
          v.selfDeaf = ctx.selfVoicePrefs.deafened;
          if (activeConn) {
            v.self_video = Boolean(activeConn.cameraOn);
            v.selfVideo = Boolean(activeConn.cameraOn);
            v.self_stream = Boolean(activeConn.screenShareOn);
            v.selfStream = Boolean(activeConn.screenShareOn);
          }
          const cidVal = ctx.cleanDiscordStr(v?.channel_id ?? v?.channelId);
          if (cidVal && !fallbackCid) {
            fallbackGid = gidKey;
            fallbackCid = cidVal;
          }
        }
      }
    }
    if (activeConn) {
      activeConn.muted = effectiveMute;
      activeConn.deafened = ctx.selfVoicePrefs.deafened;
    }
    const targetConnectCid = activeConn ? activeConn.channelId : fallbackCid;
    if (targetConnectCid) {
      const nextGuildId = activeConn ? activeConn.guildId : fallbackGid;
      const nextChannelId = String(targetConnectCid);
      const nextSelfMute = Boolean(effectiveMute);
      const nextSelfDeaf = Boolean(ctx.selfVoicePrefs.deafened);
      const nextSelfVideo = Boolean(activeConn?.cameraOn);
      const nextSelfStream = Boolean(activeConn?.screenShareOn);
      if (
        force ||
        lastVoiceConnectState.guildId !== nextGuildId ||
        lastVoiceConnectState.channelId !== nextChannelId ||
        lastVoiceConnectState.selfMute !== nextSelfMute ||
        lastVoiceConnectState.selfDeaf !== nextSelfDeaf ||
        lastVoiceConnectState.selfVideo !== nextSelfVideo ||
        lastVoiceConnectState.selfStream !== nextSelfStream
      ) {
        lastVoiceConnectState = {
          guildId: nextGuildId,
          channelId: nextChannelId,
          selfMute: nextSelfMute,
          selfDeaf: nextSelfDeaf,
          selfVideo: nextSelfVideo,
          selfStream: nextSelfStream,
        };
        void ctx
          .invoke(ctx.ipcCmds.connect, {
            guildId: nextGuildId,
            channelId: targetConnectCid,
            selfMute: nextSelfMute,
            selfDeaf: nextSelfDeaf,
            selfVideo: nextSelfVideo,
            selfStream: nextSelfStream,
          })
          .catch(() => {});
      }
    }
    const curInputDev = ctx.selfVoicePrefs.inputDeviceId || "default";
    const curInputVol = ctx.selfVoicePrefs.inputVolume;
    const curInputMuted = effectiveMute;
    if (
      force ||
      lastAudioDeviceInputConfig.deviceId !== curInputDev ||
      lastAudioDeviceInputConfig.volume !== curInputVol ||
      lastAudioDeviceInputConfig.muted !== curInputMuted
    ) {
      lastAudioDeviceInputConfig = { deviceId: curInputDev, volume: curInputVol, muted: curInputMuted };
      void ctx
        .invoke(ctx.ipcCmds.setAudioDevice, {
          kind: "input",
          deviceId: curInputDev,
          volumePercent: curInputVol,
          muted: curInputMuted,
        })
        .catch(() => {});
    }
    const curOutputDev = ctx.selfVoicePrefs.outputDeviceId || "default";
    const curOutputVol = ctx.selfVoicePrefs.outputVolume;
    const curOutputMuted = ctx.selfVoicePrefs.deafened;
    if (
      force ||
      lastAudioDeviceOutputConfig.deviceId !== curOutputDev ||
      lastAudioDeviceOutputConfig.volume !== curOutputVol ||
      lastAudioDeviceOutputConfig.muted !== curOutputMuted
    ) {
      lastAudioDeviceOutputConfig = { deviceId: curOutputDev, volume: curOutputVol, muted: curOutputMuted };
      void ctx
        .invoke(ctx.ipcCmds.setAudioDevice, {
          kind: "output",
          deviceId: curOutputDev,
          volumePercent: curOutputVol,
          muted: curOutputMuted,
        })
        .catch(() => {});
    }
  }

  /**
   * Polls audio device levels and syncs device selects when Voice & Video settings are open,
   * throttling background syncs during stable calls to at most once every 10 seconds.
   * @param {boolean} [force]
   * @returns {Promise<any>}
   */
  async function pollAudioDeviceLevels(force = false) {
    if (!ctx.isLiveSession() || audioDeviceLevelsInFlight) return null;
    audioDeviceLevelsInFlight = true;
    try {
      const now = Date.now();
      const settingsEl = typeof document !== "undefined" ? document.getElementById("settings") : null;
      const screenSettingsEl = typeof document !== "undefined" ? document.getElementById("screen-settings") : null;
      const isSettingsScreenVisible = !screenSettingsEl
        ? Boolean(settingsEl && !settingsEl.hidden && !settingsEl.hasAttribute?.("hidden"))
        : !screenSettingsEl.hidden &&
          !screenSettingsEl.hasAttribute?.("hidden") &&
          screenSettingsEl.getAttribute?.("aria-hidden") !== "true";
      const isVoiceSettingsOpen = Boolean(
        isSettingsScreenVisible &&
          settingsEl &&
          !settingsEl.hidden &&
          !settingsEl.hasAttribute?.("hidden") &&
          settingsEl.getAttribute?.("aria-hidden") !== "true" &&
          (settingsEl.dataset?.activePane === "voice" ||
            settingsEl.getAttribute?.("data-active-pane") === "voice"),
      );

      const activeConn = ctx.getActiveVoiceConnection();
      const rtcStatus = ctx.getLiveRtcStatus();
      const isStableCall = Boolean(
        activeConn &&
          (rtcStatus?.state === "connected" || rtcStatus?.phase === "connected") &&
          highPingStrikes === 0,
      );

      if (!isVoiceSettingsOpen && isStableCall && !force) {
        if (now - lastAudioDeviceSyncAtMs < 10000) {
          return null;
        }
      }

      lastAudioDeviceSyncAtMs = now;

      if (isVoiceSettingsOpen || force) {
        try {
          const devs = /** @type {any} */ (await ctx.invoke(ctx.ipcCmds.audioDevices));
          if (devs && typeof devs === "object") {
            const inputs = Array.isArray(devs?.inputs) ? devs.inputs : [];
            const outputs = Array.isArray(devs?.outputs) ? devs.outputs : [];
            if (inputs.length > 0 || outputs.length > 0) {
              const inSel = /** @type {HTMLSelectElement | null} */ (
                settingsEl?.querySelector('select[data-setting="voiceInputDevice"], #settings-voice-input-select')
              );
              if (inSel && inputs.length > 0) {
                inSel.replaceChildren();
                for (const dev of inputs) {
                  const opt = document.createElement("option");
                  opt.value = String(dev.id || "default");
                  opt.textContent = String(dev.name || dev.id || "Микрофон");
                  if (opt.value === ctx.selfVoicePrefs.inputDeviceId) opt.selected = true;
                  inSel.append(opt);
                }
              }
              const outSel = /** @type {HTMLSelectElement | null} */ (
                settingsEl?.querySelector('select[data-setting="voiceOutputDevice"], #settings-voice-output-select')
              );
              if (outSel && outputs.length > 0) {
                outSel.replaceChildren();
                for (const dev of outputs) {
                  const opt = document.createElement("option");
                  opt.value = String(dev.id || "default");
                  opt.textContent = String(dev.name || dev.id || "Динамики");
                  if (opt.value === ctx.selfVoicePrefs.outputDeviceId) opt.selected = true;
                  outSel.append(opt);
                }
              }
            }
          }
        } catch {}
      }

      syncVoiceMuteStateWithBackend(false);
      return null;
    } finally {
      audioDeviceLevelsInFlight = false;
    }
  }

  function startAudioDeviceLevelsPolling() {
    if (ctx.audioDeviceLevelsTimer) return;
    const settingsEl = typeof document !== "undefined" ? document.getElementById("settings") : null;
    const screenSettingsEl = typeof document !== "undefined" ? document.getElementById("screen-settings") : null;
    const isSettingsScreenVisible = !screenSettingsEl
      ? Boolean(settingsEl && !settingsEl.hidden && !settingsEl.hasAttribute?.("hidden"))
      : !screenSettingsEl.hidden &&
        !screenSettingsEl.hasAttribute?.("hidden") &&
        screenSettingsEl.getAttribute?.("aria-hidden") !== "true";
    const isVoiceSettingsOpen = Boolean(
      isSettingsScreenVisible &&
        settingsEl &&
        !settingsEl.hidden &&
        !settingsEl.hasAttribute?.("hidden") &&
        settingsEl.getAttribute?.("aria-hidden") !== "true" &&
        (settingsEl.dataset?.activePane === "voice" ||
          settingsEl.getAttribute?.("data-active-pane") === "voice"),
    );
    if (!ctx.getActiveVoiceConnection() && !isVoiceSettingsOpen) {
      stopAudioDeviceLevelsPolling();
      return;
    }
    const intervalMs = isVoiceSettingsOpen ? 500 : 10000;

    ctx.audioDeviceLevelsTimer = ctx.createTauriEventListener({
      event: "voice-audio-device-levels",
      intervalMs,
      initialFetch: () => pollAudioDeviceLevels(false),
      onEvent: () => {
        void pollAudioDeviceLevels(false);
      },
      autoStart: true,
    });
  }

  function stopAudioDeviceLevelsPolling() {
    if (ctx.audioDeviceLevelsTimer) {
      const activeTimer = /** @type {any} */ (ctx.audioDeviceLevelsTimer);
      if (typeof activeTimer.stop === "function") {
        void activeTimer.stop();
      } else {
        clearTimeout(activeTimer);
        clearInterval(activeTimer);
      }
      ctx.audioDeviceLevelsTimer = null;
    }
  }

  if (typeof globalThis !== "undefined") {
    /** @type {any} */ (globalThis).pollAudioDeviceLevels = pollAudioDeviceLevels;
    /** @type {any} */ (globalThis).startAudioDeviceLevelsPolling = startAudioDeviceLevelsPolling;
    /** @type {any} */ (globalThis).stopAudioDeviceLevelsPolling = stopAudioDeviceLevelsPolling;
  }

  /**
   * Toggles microphone mute only (`self_mute`). Unmuting while deafened also undeafens.
   * @param {boolean} [forceState]
   */
  function toggleSelfMicOnly(forceState) {
    if (typeof forceState === "boolean") {
      ctx.selfVoicePrefs.muted = forceState;
      if (!forceState && ctx.selfVoicePrefs.deafened) {
        ctx.selfVoicePrefs.deafened = false;
      }
    } else if (ctx.selfVoicePrefs.deafened) {
      ctx.selfVoicePrefs.deafened = false;
      ctx.selfVoicePrefs.muted = false;
    } else {
      ctx.selfVoicePrefs.muted = !ctx.selfVoicePrefs.muted;
    }
    ctx.selfVoicePrefs.muteBeforeDeaf = ctx.selfVoicePrefs.muted;
    ctx.playVoiceUiSound(ctx.selfVoicePrefs.muted ? "mute" : "unmute");
    syncVoiceMuteStateWithBackend();
    ctx.renderLiveVoicePanel();
    ctx.getTree?.()?.refreshView?.();
  }

  /**
   * Toggles both deafen and microphone mute (`self_deaf` + `self_mute`).
   * @param {boolean} [forceState]
   */
  function toggleSelfDeafenBoth(forceState) {
    const nextDeaf = typeof forceState === "boolean" ? forceState : !ctx.selfVoicePrefs.deafened;
    if (nextDeaf) {
      if (!ctx.selfVoicePrefs.deafened) {
        ctx.selfVoicePrefs.muteBeforeDeaf = ctx.selfVoicePrefs.muted;
      }
      ctx.selfVoicePrefs.deafened = true;
      ctx.selfVoicePrefs.muted = true;
    } else {
      ctx.selfVoicePrefs.deafened = false;
      ctx.selfVoicePrefs.muted = Boolean(ctx.selfVoicePrefs.muteBeforeDeaf);
      ctx.selfVoicePrefs.muteBeforeDeaf = false;
    }
    ctx.playVoiceUiSound(ctx.selfVoicePrefs.deafened ? "deafen" : "undeafen");
    syncVoiceMuteStateWithBackend();
    ctx.renderLiveVoicePanel();
    ctx.getTree?.()?.refreshView?.();
  }

  function ensureUserActivityTracking() {
    if (userActivityListenersBound || typeof document === "undefined") return;
    userActivityListenersBound = true;
    const markActive = () => {
      lastUserActivityAtMs = Date.now();
    };
    for (const evName of ["mousemove", "mousedown", "keydown", "wheel", "pointerdown", "touchstart"]) {
      document.addEventListener(evName, markActive, { passive: true, capture: true });
    }
  }

  function isUserActivelyOccupiedInApp() {
    if (ctx.getLiveRtcStatus()?.localMicSpeaking) return true;
    const activeConn = ctx.getActiveVoiceConnection();
    if (activeConn?.screenShareOn || activeConn?.cameraOn) return true;
    if (typeof document !== "undefined") {
      if (document.getElementById("live-stream-stage-viewer")) return true;
      const mediaEls = document.querySelectorAll("video, audio");
      for (const el of mediaEls) {
        const m = /** @type {HTMLMediaElement} */ (el);
        if (!m.paused && !m.ended) return true;
      }
    }
    return false;
  }

  /**
   * Evaluates server AFK timeout and moves the user to the guild AFK channel (or disconnects)
   * when idle longer than `afk_timeout`.
   */
  function checkServerVoiceAfkTimeout() {
    const activeConn = ctx.getActiveVoiceConnection();
    if (!activeConn || !ctx.isLiveSession()) return;
    if (isUserActivelyOccupiedInApp()) {
      lastUserActivityAtMs = Date.now();
      return;
    }
    const gId = String(activeConn.guildId || "");
    const cId = String(activeConn.channelId || "");
    if (!gId || !cId) return;
    if (ctx.isChannelAfk(gId, cId, activeConn.channelName)) {
      return;
    }
    const gObj = ctx.liveGuildsById?.get?.(gId);
    const rawAfkTimeoutSec = Number(gObj?.afkTimeout ?? gObj?.afk_timeout ?? 0);
    if (!Number.isFinite(rawAfkTimeoutSec) || rawAfkTimeoutSec < 60) {
      return;
    }
    const idleMs = Date.now() - lastUserActivityAtMs;
    if (idleMs < rawAfkTimeoutSec * 1000) {
      return;
    }
    lastUserActivityAtMs = Date.now();
    let afkCid = ctx.cleanDiscordStr(gObj?.afkChannelId ?? gObj?.afk_channel_id);
    let afkChName = "AFK";
    const chList = Array.isArray(ctx.getTreeChannels?.()) ? ctx.getTreeChannels() : [];
    if (afkCid) {
      const found = chList.find((/** @type {any} */ ch) => String(ch?.id ?? "") === afkCid);
      if (found?.name) afkChName = String(found.name);
    } else {
      const byName = chList.find(
        (/** @type {any} */ ch) =>
          ch?.type === 2 && ctx.isChannelAfk(gId, String(ch?.id ?? ""), String(ch?.name || "")),
      );
      if (byName?.id) {
        afkCid = String(byName.id);
        afkChName = String(byName.name || "AFK");
      }
    }
    if (afkCid && afkCid !== cId) {
      ctx.playVoiceUiSound("user_moved");
      connectToLiveVoiceChannel(gId, activeConn.guildName || "Сервер", {
        id: afkCid,
        name: afkChName,
      });
    } else if (!afkCid) {
      disconnectFromLiveVoiceChannel();
    }
  }

  /**
   * Subscribes to `voice-state-update` events while connected to update speaking indicators,
   * connection timers, and link quality telemetry.
   */
  function startRtcStatusPolling() {
    ensureUserActivityTracking();
    lastUserActivityAtMs = Date.now();
    if (ctx.rtcPollTimer) return;
    let pollTick = 0;

    /** @param {any} res */
    const handleRtcUpdate = (res) => {
      const activeConn = ctx.getActiveVoiceConnection();
      if (!activeConn || !res || typeof res !== "object") return;
      ctx.setLiveRtcStatus(res);
      ctx.updateLiveRtcDomInPlace();
      if (res.localMicSpeaking) {
        lastUserActivityAtMs = Date.now();
      }
      const pingVal = typeof res.lastPingMs === "number" ? res.lastPingMs : 0;
      const st = String(res.state || "connected");
      if (st === "error" || pingVal > 1200) {
        highPingStrikes += 1;
      } else if (pingVal > 0 && pingVal <= 800) {
        highPingStrikes = 0;
      }
      const nowMs = Date.now();
      if (
        highPingStrikes >= 45 &&
        nowMs - lastAutoReconnectMs > 30_000 &&
        ctx.getActiveVoiceConnection()
      ) {
        highPingStrikes = 0;
        lastAutoReconnectMs = nowMs;
        syncVoiceMuteStateWithBackend();
      }
      pollTick += 1;
      if (pollTick % 20 === 0) {
        checkServerVoiceAfkTimeout();
      }
    };

    const pollOnce = async () => {
      if (!ctx.getActiveVoiceConnection() || !ctx.isLiveSession() || rtcPollInFlight) return null;
      rtcPollInFlight = true;
      try {
        const res = await ctx.invoke(ctx.ipcCmds.rtcStatus);
        if (res && typeof res === "object") {
          handleRtcUpdate(res);
          return res;
        }
      } catch {
      } finally {
        rtcPollInFlight = false;
      }
      return null;
    };

    ctx.rtcPollTimer = ctx.createTauriEventListener({
      event: "voice-state-update",
      intervalMs: 1000,
      initialFetch: pollOnce,
      onEvent: (/** @type {any} */ payload) => {
        if (payload && typeof payload === "object") {
          handleRtcUpdate(payload);
        } else {
          void pollOnce();
        }
      },
      autoStart: true,
    });
  }

  function stopRtcStatusPolling() {
    if (ctx.rtcPollTimer) {
      const activeTimer = /** @type {any} */ (ctx.rtcPollTimer);
      if (typeof activeTimer.stop === "function") {
        void activeTimer.stop();
      } else {
        clearInterval(activeTimer);
      }
      ctx.rtcPollTimer = null;
    }
  }

  function stopVoiceSync() {
    if (ctx.voiceSyncTimer) {
      const activeTimer = /** @type {any} */ (ctx.voiceSyncTimer);
      if (typeof activeTimer.stop === "function") {
        void activeTimer.stop();
      } else {
        clearInterval(activeTimer);
      }
      ctx.voiceSyncTimer = null;
    }
  }

  function retryActiveVoiceConnection() {
    const activeConn = ctx.getActiveVoiceConnection();
    if (!activeConn) return;
    lastVoiceConnectState = {
      guildId: null,
      channelId: null,
      selfMute: null,
      selfDeaf: null,
      selfVideo: null,
      selfStream: null,
    };
    connectToLiveVoiceChannel(
      String(activeConn.guildId || ""),
      String(activeConn.guildName || "Сервер"),
      {
        id: String(activeConn.channelId || ""),
        name: String(activeConn.channelName || activeConn.channelId || ""),
      },
    );
  }

  /**
   * Connects to a live Discord voice channel (Gateway OP 4 + RTC WebSocket + UDP IP Discovery).
   * @param {string} targetGuildId
   * @param {string} targetGuildName
   * @param {{ id: string, name?: string }} channel
   */
  function connectToLiveVoiceChannel(targetGuildId, targetGuildName, channel) {
    const vsGuildKey = targetGuildId || "@me";
    if (ctx.isLiveSession() && targetGuildId && targetGuildId !== "@me") {
      const gObj = ctx.liveGuildsById?.get?.(ctx.cleanDiscordStr(targetGuildId));
      const isLurker =
        gObj?.lurker === true || gObj?.isMember === false || gObj?.is_member === false;
      if (isLurker) {
        syncServerAccessBannersAndComposerDom({
          doc: document,
          guild: gObj,
          channel: { ...channel, type: 2 },
          currentUserId: String(ctx.getLiveSelfUser()?.id || ""),
        });
        ctx.showVoiceToastNotice(
          "Режим предпросмотра сервера (вы не участник) — вступите на сервер, чтобы подключаться к голосовым каналам",
        );
        return;
      }
      if (gObj?.pending === true) {
        syncServerAccessBannersAndComposerDom({
          doc: document,
          guild: gObj,
          channel: { ...channel, type: 2 },
          currentUserId: String(ctx.getLiveSelfUser()?.id || ""),
        });
        ctx.showVoiceToastNotice(
          "Примите правила сервера (Membership Screening), чтобы подключаться к голосовым каналам",
        );
        return;
      }
      const effPerms = ctx.computeEffectiveChannelPermissions(targetGuildId, channel, ctx.getTreeChannels?.());
      const PERM_VIEW_AND_CONNECT = (1n << 10n) | (1n << 20n);
      if (effPerms !== null && (effPerms & PERM_VIEW_AND_CONNECT) !== PERM_VIEW_AND_CONNECT) {
        syncServerAccessBannersAndComposerDom({
          doc: document,
          guild: gObj,
          channel: {
            ...channel,
            type: 2,
            permission_overwrites: [
              { id: String(targetGuildId), type: 0, allow: "0", deny: String(1n << 20n) },
            ],
          },
          currentUserId: String(ctx.getLiveSelfUser()?.id || ""),
        });
        ctx.showVoiceToastNotice("У вашей роли нет прав для подключения к этому голосовому каналу");
        return;
      }
      const existingConnectBanner = ctx.byId("channel-readonly-access-banner");
      if (existingConnectBanner?.getAttribute("data-access-banner") === "missing-connect") {
        existingConnectBanner.remove();
      }
    }
    const chName = String(channel.name || "");
    const isAfkRoom = ctx.isChannelAfk(targetGuildId, channel.id, chName);
    const effectiveMute = Boolean(ctx.selfVoicePrefs.muted || isAfkRoom);
    const existingList = (
      ctx.liveVoiceStatesByGuild.get(vsGuildKey) ??
      ctx.liveVoiceStatesByGuild.get(targetGuildId) ??
      []
    ).filter(
      (/** @type {any} */ vs) => String(vs?.channel_id ?? vs?.channelId ?? "") === channel.id,
    );
    const nowMs = Date.now();
    if (existingList.length === 0) {
      ctx.liveVoiceChannelStartEpochMs.set(`${ctx.normalizeVoiceGuildKey(targetGuildId)}:${channel.id}`, nowMs);
    }
    const selfId = String(ctx.getLiveSelfUser()?.id || "");
    for (const [gid, arr] of ctx.liveVoiceStatesByGuild.entries()) {
      const filtered = arr.filter(
        (/** @type {any} */ v) =>
          String(v?.user_id ?? v?.userId ?? "") !== selfId ||
          (gid === vsGuildKey && String(v?.channel_id ?? v?.channelId ?? "") === channel.id && isConfirmedVoiceStateRecord(v)),
      );
      ctx.liveVoiceStatesByGuild.set(gid, filtered);
    }
    const curG = targetGuildId && targetGuildId !== "@me" ? ctx.liveGuildsById?.get?.(ctx.cleanDiscordStr(targetGuildId)) : null;
    const chAny = /** @type {any} */ (channel);
    const isAfkChannel = Boolean(
      chAny?.isAfk ||
      chAny?.is_afk ||
      (curG && (curG.afkChannelId === channel.id || curG.afk_channel_id === channel.id)) ||
      (typeof channel?.name === "string" && channel.name.toLowerCase().includes("afk")),
    );
    const newConn = {
      guildId: targetGuildId,
      guildName: targetGuildName || "Сервер",
      channelId: channel.id,
      channelName: channel.name || channel.id,
      muted: effectiveMute,
      deafened: ctx.selfVoicePrefs.deafened,
      krispEnabled: ctx.selfVoicePrefs.krispEnabled,
      cameraOn: false,
      screenShareOn: false,
      connectedAtMs: nowMs,
      isAfk: isAfkChannel,
    };
    ctx.setActiveVoiceConnection(newConn);
    ctx.setLiveRtcStatus({
      ...ctx.getLiveRtcStatus(),
      phase: "signaling",
      state: "signaling",
      error: null,
    });
    const reactiveStore = ctx.getGlobalAppReactiveStore();
    reactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: String(targetGuildId || "@me"),
      channelId: String(channel.id || ""),
      channelName: String(channel.name || channel.id),
      phase: "signaling",
      state: "signaling",
      error: null,
      selfMute: effectiveMute,
      selfDeaf: Boolean(ctx.selfVoicePrefs.deafened),
      selfVideo: false,
      selfStream: false,
    });
    ctx.playVoiceUiSound("join");
    ctx.renderLiveVoicePanel();
    ctx.getTree?.()?.refreshView?.();
    if (ctx.isLiveSession()) {
      lastVoiceConnectState = {
        guildId: targetGuildId ?? null,
        channelId: String(channel.id || ""),
        selfMute: Boolean(effectiveMute),
        selfDeaf: Boolean(ctx.selfVoicePrefs.deafened),
        selfVideo: false,
        selfStream: false,
      };
      void ctx
        .invoke(ctx.ipcCmds.connect, {
          guildId: targetGuildId,
          channelId: channel.id,
          selfMute: effectiveMute,
          selfDeaf: ctx.selfVoicePrefs.deafened,
        })
        .then((/** @type {any} */ snap) => {
          if (snap && typeof snap === "object") {
            ctx.setLiveRtcStatus(snap);
            const curr = ctx.getActiveVoiceConnection();
            if (curr) {
              reactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
                guildId: String(curr.guildId || "@me"),
                channelId: String(curr.channelId || ""),
                channelName: String(curr.channelName || ""),
                phase: snap.phase ?? snap.state ?? "signaling",
                state: snap.state ?? "signaling",
                error: snap.error ?? snap.reason ?? null,
                selfMute: Boolean(curr.muted),
                selfDeaf: Boolean(curr.deafened),
                selfVideo: Boolean(curr.cameraOn),
                selfStream: Boolean(curr.screenShareOn),
              });
            }
            ctx.updateLiveRtcDomInPlace();
          }
        })
        .catch((/** @type {any} */ err) => {
          lastVoiceConnectState = {
            guildId: null,
            channelId: null,
            selfMute: null,
            selfDeaf: null,
            selfVideo: null,
            selfStream: null,
          };
          const msg =
            typeof err === "string"
              ? err
              : String(err?.message || "Не удалось подключиться к голосовому каналу (отказано в доступе)");
          const errPhase = /timeout|таймаут/i.test(msg) ? "timeout" : "error";
          applyVoiceStateUpdateEvent(ctx.liveVoiceStatesByGuild, {
            guild_id: vsGuildKey,
            channel_id: channel.id,
            user_id: selfId,
            phase: errPhase,
            error: msg,
          });
          stopRtcStatusPolling();
          ctx.setLiveRtcStatus({
            ...ctx.getLiveRtcStatus(),
            phase: errPhase,
            state: errPhase,
            error: msg,
          });
          const curr = ctx.getActiveVoiceConnection();
          if (curr) {
            reactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
              guildId: String(curr.guildId || "@me"),
              channelId: String(curr.channelId || ""),
              channelName: String(curr.channelName || ""),
              phase: errPhase,
              state: errPhase,
              error: msg,
              selfMute: Boolean(curr.muted),
              selfDeaf: Boolean(curr.deafened),
              selfVideo: Boolean(curr.cameraOn),
              selfStream: Boolean(curr.screenShareOn),
            });
          }
          ctx.getTree?.()?.refreshView?.();
          ctx.updateLiveRtcDomInPlace();
          if (ctx.isLiveSession()) {
            ctx.showVoiceToastNotice(msg);
          } else {
            ctx.surface(new UIError("ACTION_FAILED", msg));
          }
        });
      const curInputDev = ctx.selfVoicePrefs.inputDeviceId || "default";
      const curInputVol = ctx.selfVoicePrefs.inputVolume;
      const curInputMuted = effectiveMute;
      if (
        lastAudioDeviceInputConfig.deviceId !== curInputDev ||
        lastAudioDeviceInputConfig.volume !== curInputVol ||
        lastAudioDeviceInputConfig.muted !== curInputMuted
      ) {
        lastAudioDeviceInputConfig = { deviceId: curInputDev, volume: curInputVol, muted: curInputMuted };
        void ctx
          .invoke(ctx.ipcCmds.setAudioDevice, {
            kind: "input",
            deviceId: curInputDev,
            volumePercent: curInputVol,
            muted: curInputMuted,
          })
          .catch(() => {});
      }
      const curOutputDev = ctx.selfVoicePrefs.outputDeviceId || "default";
      const curOutputVol = ctx.selfVoicePrefs.outputVolume;
      const curOutputMuted = ctx.selfVoicePrefs.deafened;
      if (
        lastAudioDeviceOutputConfig.deviceId !== curOutputDev ||
        lastAudioDeviceOutputConfig.volume !== curOutputVol ||
        lastAudioDeviceOutputConfig.muted !== curOutputMuted
      ) {
        lastAudioDeviceOutputConfig = { deviceId: curOutputDev, volume: curOutputVol, muted: curOutputMuted };
        void ctx
          .invoke(ctx.ipcCmds.setAudioDevice, {
            kind: "output",
            deviceId: curOutputDev,
            volumePercent: curOutputVol,
            muted: curOutputMuted,
          })
          .catch(() => {});
      }
      startRtcStatusPolling();
      startAudioDeviceLevelsPolling();
    }
  }

  /**
   * Disconnects from the active voice channel, stops any active screen share, and cleans up timers/popouts.
   */
  function disconnectFromLiveVoiceChannel() {
    const connAny = /** @type {any} */ (ctx.getActiveVoiceConnection());
    const prevGuildId = connAny?.guildId ?? null;
    const prevChannelId = connAny?.channelId ?? null;
    const hadStream = Boolean(
      connAny?.screenShareOn ||
        connAny?.activeStreamKey ||
        /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__,
    );
    const selfId = ctx.cleanDiscordStr(String(ctx.getLiveSelfUser()?.id || ""));
    const stopStreamKey =
      connAny?.activeStreamKey ||
      /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ ||
      (prevGuildId && prevGuildId !== HOME_GUILD_ID && prevGuildId !== "@me"
        ? `guild:${prevGuildId}:${prevChannelId}:${selfId}`
        : `call:${prevChannelId}:${selfId}`);

    ctx.cancelPipAnimTimer();
    document.getElementById("live-stream-pip-preview")?.remove();
    document.getElementById("live-stream-stage-viewer")?.remove();
    document.getElementById("live-stream-preview-overlay")?.remove();
    document.getElementById("live-screenshare-modal")?.remove();
    document.getElementById("live-screenshare-portal-modal")?.remove();
    document.getElementById("live-stream-settings-menu")?.remove();
    document.getElementById("live-stream-settings-submenu")?.remove();
    ctx.cachedLiveScreenImg = null;
    ctx.cachedLiveScreenDataUrl = "";
    ctx.lastLiveScreenSeq = 0;

    if (hadStream) {
      void ctx
        .invoke("voice_stream_stop", {
          stream_key: stopStreamKey,
          streamKey: stopStreamKey,
        })
        .catch(() => {});
      /** @type {any} */ (globalThis).__ACTIVE_STREAM_KEY__ = null;
      ctx.playVoiceUiSound("stream_off");
    }

    for (const [gid, arr] of ctx.liveVoiceStatesByGuild.entries()) {
      ctx.liveVoiceStatesByGuild.set(
        gid,
        arr.filter((/** @type {any} */ v) => String(v?.user_id ?? v?.userId ?? "") !== selfId),
      );
    }
    if (connAny) {
      ctx.playVoiceUiSound("disconnect");
    }
    ctx.setActiveVoiceConnection(null);
    stopRtcStatusPolling();
    stopAudioDeviceLevelsPolling();
    stopVoiceSync();
    lastAudioDeviceInputConfig = { deviceId: "", volume: -1, muted: null };
    lastAudioDeviceOutputConfig = { deviceId: "", volume: -1, muted: null };
    lastVoiceConnectState = {
      guildId: null,
      channelId: null,
      selfMute: null,
      selfDeaf: null,
      selfVideo: null,
      selfStream: null,
    };
    ctx.closeLiveVoicePopouts();
    ctx.setLiveRtcStatus({
      ...ctx.getLiveRtcStatus(),
      state: "disconnected",
      lastPingMs: null,
    });
    if (ctx.isLiveSession()) {
      void ctx.invoke(ctx.ipcCmds.disconnect, { guildId: prevGuildId }).catch(() => {});
    }
    ctx.renderLiveVoicePanel();
    ctx.getTree?.()?.refreshView?.();
  }

  /**
   * Opens the voice channel switch confirmation modal.
   * @param {string} targetGuildId
   * @param {string} targetGuildName
   * @param {{ id: string, name?: string }} channel
   */
  function promptVoiceChannelSwitch(targetGuildId, targetGuildName, channel) {
    document.getElementById("voice-switch-confirm-modal")?.remove();
    const backdrop = document.createElement("div");
    backdrop.id = "voice-switch-confirm-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.72);display:flex;align-items:center;justify-content:center;z-index:9999;";
    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.style.cssText =
      "width:440px;max-width:92vw;background:#313338;color:#f2f3f5;border-radius:10px;padding:24px;box-shadow:0 16px 40px rgba(0,0,0,0.55);display:flex;flex-direction:column;gap:16px;font-family:inherit;";
    const headerRow = document.createElement("div");
    headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;";
    const title = document.createElement("h3");
    title.style.cssText = "margin:0;font-size:20px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Вы уверены?";
    const closeX = document.createElement("button");
    closeX.type = "button";
    closeX.className = "icon-btn";
    closeX.textContent = "✕";
    closeX.style.cssText = "color:#b5bac1;font-size:18px;";
    closeX.addEventListener("click", () => backdrop.remove());
    headerRow.append(title, closeX);

    const desc = document.createElement("p");
    desc.style.cssText = "margin:0;font-size:14px;line-height:1.45;color:#dbdee1;";
    desc.textContent = `Кажется, вы находитесь в другом голосовом канале. Вы уверены, что хотите перейти в канал «${channel.name || channel.id}»?`;

    const checkLabel = document.createElement("label");
    checkLabel.style.cssText = "display:flex;align-items:center;gap:10px;font-size:14px;color:#dbdee1;cursor:pointer;user-select:none;";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.style.cssText = "width:18px;height:18px;accent-color:#5865f2;cursor:pointer;";
    const checkSpan = document.createElement("span");
    checkSpan.textContent = "Больше не спрашивать";
    checkLabel.append(checkbox, checkSpan);

    const footer = document.createElement("div");
    footer.style.cssText = "display:flex;gap:12px;margin-top:4px;";
    const cancelBtn = document.createElement("button");
    cancelBtn.type = "button";
    cancelBtn.textContent = "Отмена";
    cancelBtn.style.cssText =
      "flex:1;padding:10px 16px;border-radius:6px;border:none;background:#4e5058;color:#f2f3f5;font-size:14px;font-weight:600;cursor:pointer;";
    cancelBtn.addEventListener("click", () => backdrop.remove());
    const confirmBtn = document.createElement("button");
    confirmBtn.type = "button";
    confirmBtn.textContent = "Подтвердить";
    confirmBtn.style.cssText =
      "flex:1;padding:10px 16px;border-radius:6px;border:none;background:#5865f2;color:#ffffff;font-size:14px;font-weight:600;cursor:pointer;";
    confirmBtn.addEventListener("click", () => {
      if (checkbox.checked) ctx.setSkipVoiceSwitchConfirm(true);
      backdrop.remove();
      connectToLiveVoiceChannel(targetGuildId, targetGuildName, channel);
    });
    footer.append(cancelBtn, confirmBtn);
    card.append(headerRow, desc, checkLabel, footer);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) backdrop.remove();
    });
    document.body.append(backdrop);
  }

  /**
   * Refreshes voice states snapshot and periodically syncs DM/friends and chat authors.
   */
  async function refreshLiveReadyVoiceAndUsers() {
    if (!ctx.isLiveSession() || refreshLiveReadyInFlight) return;
    refreshLiveReadyInFlight = true;
    syncTickCounter += 1;
    try {
      const fastVs = await ctx.invoke(ctx.ipcCmds.setAudioDevice, { kind: "voice_states_sync" });
      const rawVs = fastVs?.voiceStates;
      let nextDigest = "";
      const activeConn = ctx.getActiveVoiceConnection();
      const rtcStatus = ctx.getLiveRtcStatus();
      if (rawVs && typeof rawVs === "object") {
        nextDigest = ctx.mergeVoiceStatesSnapshot(ctx.liveVoiceStatesByGuild, rawVs);
        ctx.getGlobalAppReactiveStore().applyLivePushPayload({
          voiceConnection: activeConn
            ? /** @type {any} */ ({
                guildId: String(activeConn.guildId || "@me"),
                channelId: String(activeConn.channelId || ""),
                channelName: String(activeConn.channelName || ""),
                phase: rtcStatus?.phase ?? rtcStatus?.state ?? "signaling",
                state: rtcStatus?.state ?? "signaling",
                error: rtcStatus?.error ?? rtcStatus?.reason ?? null,
                selfMute: Boolean(activeConn.muted),
                selfDeaf: Boolean(activeConn.deafened),
                selfVideo: Boolean(activeConn.cameraOn),
                selfStream: Boolean(activeConn.screenShareOn),
              })
            : null,
          voiceStates: Object.fromEntries(ctx.liveVoiceStatesByGuild.entries()),
        });
      }
      if (nextDigest !== lastVoiceStatesDigest) {
        lastVoiceStatesDigest = nextDigest;
        ctx.getTree?.()?.refreshView?.();
        ctx.renderLiveVoicePanel();
        ctx.renderLiveDmCallStage();
        ctx.updateLiveRtcDomInPlace();
      }
      const nowDmSync = Date.now();
      if (!document.getElementById("live-stream-stage-viewer") && nowDmSync - lastDmFriendsSyncAt >= 60000) {
        lastDmFriendsSyncAt = nowDmSync;
        const dmSync = await ctx.invoke(ctx.ipcCmds.setAudioDevice, {
          kind: "dm_friends_sync",
          deviceId: "poll",
        });
        if (dmSync && typeof dmSync === "object" && dmSync.ok !== false) {
          if (Array.isArray(dmSync.users)) {
            for (const u of dmSync.users) {
              if (u?.id) ctx.liveUsersById.set(String(u.id), u);
            }
          }
          if (Array.isArray(dmSync.guilds)) {
            ctx.syncLiveGuildRailFromSnapshot?.(dmSync.guilds);
          }
          ctx.getDmFriendsController?.()?.syncLiveState?.({
            relationships: dmSync.relationships,
            privateChannels: dmSync.privateChannels,
            users: dmSync.users,
            presences: dmSync.presences,
          });
        }
      }
      if (syncTickCounter % 10 === 0 && !document.getElementById("live-stream-stage-viewer")) {
        ctx.refreshLiveChatAuthorsInDom?.();
      }
    } catch {
    } finally {
      refreshLiveReadyInFlight = false;
    }
  }

  /**
   * Stops all active RTC, audio device, and voice sync timers, closes popouts, and releases audio context.
   */
  function cleanupLiveVoiceTimersAndAudio() {
    stopRtcStatusPolling();
    stopAudioDeviceLevelsPolling();
    stopVoiceSync();
    lastAudioDeviceInputConfig = { deviceId: "", volume: -1, muted: null };
    lastAudioDeviceOutputConfig = { deviceId: "", volume: -1, muted: null };
    lastVoiceConnectState = {
      guildId: null,
      channelId: null,
      selfMute: null,
      selfDeaf: null,
      selfVideo: null,
      selfStream: null,
    };
    if (ctx.micTestTimer) {
      if (typeof ctx.micTestTimer.stop === "function") void ctx.micTestTimer.stop();
      else clearInterval(ctx.micTestTimer);
      ctx.micTestTimer = null;
    }
    ctx.closeLiveVoicePopouts();
    if (ctx.sharedSoundboardAudioCtx && typeof ctx.sharedSoundboardAudioCtx.close === "function") {
      void ctx.sharedSoundboardAudioCtx.close().catch(() => {});
      ctx.sharedSoundboardAudioCtx = null;
    }
  }

  return {
    syncVoiceMuteStateWithBackend,
    pollAudioDeviceLevels,
    startAudioDeviceLevelsPolling,
    stopAudioDeviceLevelsPolling,
    toggleSelfMicOnly,
    toggleSelfDeafenBoth,
    ensureUserActivityTracking,
    isUserActivelyOccupiedInApp,
    checkServerVoiceAfkTimeout,
    startRtcStatusPolling,
    stopRtcStatusPolling,
    stopVoiceSync,
    retryActiveVoiceConnection,
    connectToLiveVoiceChannel,
    disconnectFromLiveVoiceChannel,
    promptVoiceChannelSwitch,
    refreshLiveReadyVoiceAndUsers,
    cleanupLiveVoiceTimersAndAudio,
  };
}
