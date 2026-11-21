// @ts-nocheck
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createVoiceCallController } from "../../rust/gui-spike/ui/features/voice-call-orchestrator.js";

describe("RW51-EPIC2-T2: Modular Voice & Call Controller", () => {
  function makeStubDeps() {
    const elements = new Map();
    const doc = {
      getElementById: (id) => elements.get(id) ?? null,
      querySelector: () => null,
      querySelectorAll: () => [],
      createElement: (tag) => ({
        tagName: tag.toUpperCase(),
        style: {},
        dataset: {},
        className: "",
        textContent: "",
        innerHTML: "",
        children: [],
        classList: {
          add() {},
          remove() {},
          toggle() {},
          contains: () => false,
        },
        setAttribute() {},
        getAttribute: () => null,
        removeAttribute() {},
        append(...nodes) {
          this.children.push(...nodes);
        },
        appendChild(node) {
          this.children.push(node);
          return node;
        },
        addEventListener() {},
        remove() {},
      }),
    };
    const origDoc = globalThis.document;
    globalThis.document = /** @type {any} */ (doc);

    let activeVoiceConnection = null;
    let liveRtcStatus = {
      state: "disconnected",
      endpoint: "",
      shortHostname: "",
      protocol: "udp",
      mode: "aead_aes256_gcm_rtpsize",
      codec: "opus",
      lastPingMs: null,
      averagePingMs: null,
      pingHistory: [],
      packetLossPercent: 0,
      ssrc: 0,
      localIp: "",
      localPort: 0,
      speakingUsers: [],
      error: "",
    };

    const selfVoicePrefs = {
      muted: false,
      deafened: false,
      muteBeforeDeaf: false,
      krispEnabled: true,
      inputDeviceId: "default",
      outputDeviceId: "default",
      inputVolume: 100,
      outputVolume: 100,
      inputMode: "voice-activity",
      status: "online",
    };

    const controller = createVoiceCallController({
      byId: (id) => elements.get(id) ?? null,
      invoke: async () => ({}),
      commands: {},
      tauri: { core: { invoke: async () => ({}) } },
      surface: () => {},
      emitLiveTelemetry: () => {},
      isLiveSession: () => false,
      createTauriEventListener: () => ({ stop() {} }),
      HOME_GUILD_ID: "@me",
      USER_BAR_BTNS: {
        mute: null,
        deaf: null,
        settings: null,
      },
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
      },
      ipcCmds: {
        setAudioDevice: "voice_set_audio_device",
        audioDevices: "voice_audio_devices",
        rtcStatus: "voice_rtc_status",
        connect: "voice_connect",
        disconnect: "voice_disconnect",
      },
      selfVoicePrefs,
      liveVoiceStatesByGuild: new Map(),
      activeDmCalls: new Map(),
      liveVoiceChannelStartEpochMs: new Map(),
      livePerUserVoiceState: new Map(),
      knownLiveAudioDeviceIds: new Set(),
      ignoredHotplugDeviceIds: new Set(),
      liveGuildChannelsCache: new Map(),
      liveGuildMembersCache: new Map(),
      liveGuildRolesCache: new Map(),
      liveUsersById: new Map(),
      liveDmChannelsCache: new Map(),
      liveDmMembersCache: new Map(),
      getActiveVoiceConnection: () => activeVoiceConnection,
      setActiveVoiceConnection: (v) => {
        activeVoiceConnection = v;
      },
      getLiveRtcStatus: () => liveRtcStatus,
      setLiveRtcStatus: (v) => {
        liveRtcStatus = v;
      },
      getLiveSelfUser: () => ({ id: "1001", username: "testuser" }),
      getHotplugPromptsSuppressed: () => false,
      setHotplugPromptsSuppressed: () => {},
      getSkipVoiceSwitchConfirm: () => false,
      setSkipVoiceSwitchConfirm: () => {},
      getSyncVoiceSettingsScreenVisibility: () => null,
      setSyncVoiceSettingsScreenVisibility: () => {},
      getVoiceDevices: () => null,
      setVoiceDevices: () => {},
      getSettingsRef: () => null,
      getChatActiveChannelId: () => "2001",
      getRailSelectedId: () => "@me",
      selectedGuildId: () => "@me",
      currentGuildInfo: () => ({ id: "@me", name: "Home" }),
      getDmFriendsController: () => null,
      getAuthState: () => ({}),
      getVoicePanel: () => ({ setState() {}, render() {} }),
      getGlobalAppReactiveStore: () => ({ activeVoiceConnection: { value: null } }),
      cleanDiscordStr: (s) => String(s ?? "").trim(),
      resolveUserRoleInfo: () => ({ name: "User", color: "" }),
      computeEffectiveChannelPermissions: () => 0n,
      computeServerPermissionsAndHierarchy: () => ({ isAdmin: false }),
      buildLiveAvatarStyle: () => "",
      formatAvatarText: (s) => String(s ?? "?").slice(0, 2),
      getDisplayName: (u) => u?.username || "User",
      openSelfProfileStatusPopover: () => {},
      openSettingsAtTab: () => {},
      openChatChannelById: () => {},
      selectDmChannel: () => {},
      renderChannelTree: () => {},
      syncLiveChatHeader: () => {},
      refreshLiveGuildTree: () => {},
      refreshLiveDmList: () => {},
    });

    return {
      controller,
      cleanup: () => {
        globalThis.document = origDoc;
      },
    };
  }

  it("exposes all extracted voice and call functions", () => {
    const { controller, cleanup } = makeStubDeps();
    try {
      const expectedMethods = [
        "computeRtcSignalMetrics",
        "playVoiceUiSound",
        "closeLiveVoicePopouts",
        "isChannelAfk",
        "syncVoiceMuteStateWithBackend",
        "pollAudioDeviceLevels",
        "startAudioDeviceLevelsPolling",
        "stopAudioDeviceLevelsPolling",
        "toggleSelfMicOnly",
        "toggleSelfDeafenBoth",
        "ensureUserActivityTracking",
        "isUserActivelyOccupiedInApp",
        "checkServerVoiceAfkTimeout",
        "startRtcStatusPolling",
        "stopRtcStatusPolling",
        "stopVoiceSync",
        "retryActiveVoiceConnection",
        "connectToLiveVoiceChannel",
        "disconnectFromLiveVoiceChannel",
        "promptVoiceChannelSwitch",
        "isTruthyVoiceFlag",
        "normalizeVoiceGuildKey",
        "mergeVoiceStatesForGuild",
        "mergeVoiceStatesSnapshot",
        "isVoiceChannelConnected",
        "getVoiceChannelTimer",
        "getVoiceParticipants",
        "showVoiceToastNotice",
        "resolveChannelObject",
        "canStreamInChannel",
        "canSpeakInChannel",
        "isChannelRoleRestricted",
        "getAloneInviteRemainingMs",
        "openVoiceInviteModal",
        "openVoiceParticipantContextMenu",
        "openAudioDevicePopover",
        "openAudioDeviceHotplugPrompt",
        "openRtcDiagnosticsModal",
        "openRtcConnectionPopout",
        "showMutedMicFloatingWarning",
        "openKrispNoisePopout",
        "openSoundboardPopout",
        "openActivitiesPopout",
        "cancelPipAnimTimer",
        "requestLiveScreenPreviewFrame",
        "switchLiveStreamSource",
        "openLiveStreamStageViewer",
        "drawLiveCaptureCanvasFrame",
        "openNitroHdModal",
        "openStreamSettingsMenu",
        "openScreenSharePortalPicker",
        "syncFloatingStreamPipPreview",
        "openScreenShareModal",
        "updateLiveRtcDomInPlace",
        "ensureUserBarDeviceChevrons",
        "syncUserBarVoiceButtons",
        "renderLiveGuildVoiceStagePreview",
        "renderLiveDmCallStage",
        "playRingingTone",
        "showIncomingCallOverlay",
        "dismissIncomingCallOverlay",
        "renderLiveVoicePanel",
        "refreshLiveReadyVoiceAndUsers",
        "openVoiceParticipantActivityPopout",
        "mountVoiceDevices",
        "setVoiceDevicesExpanded",
        "refreshLiveAudioDevices",
        "ensureVoiceTroubleshootControls",
        "setVoicePanelVisibility",
        "cleanupLiveVoiceTimersAndAudio",
      ];
      for (const name of expectedMethods) {
        assert.equal(typeof controller[name], "function", `Missing method: ${name}`);
      }
    } finally {
      cleanup();
    }
  });

  it("normalizes voice guild keys and truthy voice flags", () => {
    const { controller, cleanup } = makeStubDeps();
    try {
      assert.equal(controller.normalizeVoiceGuildKey(""), "@me");
      assert.equal(controller.normalizeVoiceGuildKey(null), "@me");
      assert.equal(controller.normalizeVoiceGuildKey("12345"), "12345");
      assert.equal(controller.isTruthyVoiceFlag(true), true);
      assert.equal(controller.isTruthyVoiceFlag("true"), true);
      assert.equal(controller.isTruthyVoiceFlag(1), true);
      assert.equal(controller.isTruthyVoiceFlag(false), false);
      assert.equal(controller.isTruthyVoiceFlag("false"), false);
    } finally {
      cleanup();
    }
  });
});
