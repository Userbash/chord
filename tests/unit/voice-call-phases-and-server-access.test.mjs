// @ts-check
import test from "node:test";
import assert from "node:assert/strict";
import { ShimDocument, loadApp } from "../support/gui-dom-harness.mjs";
import {
  VOICE_CALL_PHASE_LABELS,
  resolveVoiceCallPhaseView,
  renderVoiceRtcPhaseDom,
  mountReactiveVoiceRtcPanel,
} from "../../rust/gui-spike/ui/modules/voice-rtc-panel.js";
import {
  computeGuildChannelTreeModel,
  applyVoiceStateUpdateEvent,
  computeConfirmedVoiceParticipants,
  renderVoiceChannelParticipantsDom,
  syncServerAccessBannersAndComposerDom,
} from "../../rust/gui-spike/ui/modules/channel-tree-view.js";
import {
  createAppReactiveStore,
  globalAppReactiveStore,
} from "../../rust/gui-spike/ui/features/reactive-store.js";

test("V5.1: voice-rtc-panel отображает все 4 реальные фазы звонка из автомата Calls и кнопку повтора при error/timeout", () => {
  assert.equal(
    VOICE_CALL_PHASE_LABELS.signaling,
    "Ожидание ответа сервера Discord...",
  );
  assert.equal(
    VOICE_CALL_PHASE_LABELS.negotiating,
    "Проверка UDP IP Discovery и ключей DAVE...",
  );
  assert.equal(
    VOICE_CALL_PHASE_LABELS.connected,
    "Голосовая связь установлена • DAVE E2EE",
  );

  const sigView = resolveVoiceCallPhaseView({ phase: "signaling" });
  assert.equal(sigView.phase, "signaling");
  assert.equal(sigView.label, "Ожидание ответа сервера Discord...");
  assert.equal(sigView.canRetry, false);
  assert.equal(sigView.isError, false);

  const negView = resolveVoiceCallPhaseView({ phase: "negotiating" });
  assert.equal(negView.phase, "negotiating");
  assert.equal(negView.label, "Проверка UDP IP Discovery и ключей DAVE...");
  assert.equal(negView.canRetry, false);
  assert.equal(negView.isError, false);

  const connView = resolveVoiceCallPhaseView({ phase: "connected" });
  assert.equal(connView.phase, "connected");
  assert.equal(connView.label, "Голосовая связь установлена • DAVE E2EE");
  assert.equal(connView.canRetry, false);
  assert.equal(connView.isConnected, true);

  const errView = resolveVoiceCallPhaseView({
    phase: "error",
    error: "RTC WebSocket close 4006: сессия недействительна",
  });
  assert.equal(errView.phase, "error");
  assert.equal(errView.reason, "RTC WebSocket close 4006: сессия недействительна");
  assert.ok(errView.label.includes("RTC WebSocket close 4006: сессия недействительна"));
  assert.equal(errView.canRetry, true);
  assert.equal(errView.isError, true);

  const timeoutView = resolveVoiceCallPhaseView({
    phase: "timeout",
    reason: "Истёк таймаут ожидания VOICE_SERVER_UPDATE (10 с)",
  });
  assert.equal(timeoutView.phase, "timeout");
  assert.equal(timeoutView.reason, "Истёк таймаут ожидания VOICE_SERVER_UPDATE (10 с)");
  assert.ok(timeoutView.label.includes("Истёк таймаут ожидания VOICE_SERVER_UPDATE (10 с)"));
  assert.equal(timeoutView.canRetry, true);
  assert.equal(timeoutView.isError, true);
});

test("V5.2: renderVoiceRtcPhaseDom и mountReactiveVoiceRtcPanel синхронизируют DOM для signaling, negotiating, connected, error/timeout с кнопкой Повторить", () => {
  const doc = /** @type {any} */ (new ShimDocument());
  const panel = doc.createElement("section");
  panel.id = "voice-panel";
  const statusDot = doc.createElement("span");
  statusDot.className = "voice-status-dot";
  statusDot.setAttribute("data-voice-status-dot", "");
  const statusEl = doc.createElement("span");
  statusEl.id = "live-rtc-conn-status";
  panel.append(statusDot, statusEl);
  doc.body.append(panel);

  let retryCalls = 0;
  const onRetry = () => {
    retryCalls += 1;
  };

  renderVoiceRtcPhaseDom(panel, { phase: "signaling" }, { doc, onRetry });
  assert.equal(panel.getAttribute("data-call-phase"), "signaling");
  assert.equal(statusEl.textContent, "Ожидание ответа сервера Discord...");
  assert.equal(panel.querySelector("[data-voice-retry-btn]"), null);

  renderVoiceRtcPhaseDom(panel, { phase: "negotiating" }, { doc, onRetry });
  assert.equal(panel.getAttribute("data-call-phase"), "negotiating");
  assert.equal(statusEl.textContent, "Проверка UDP IP Discovery и ключей DAVE...");
  assert.equal(panel.querySelector("[data-voice-retry-btn]"), null);

  renderVoiceRtcPhaseDom(panel, { phase: "connected" }, { doc, onRetry });
  assert.equal(panel.getAttribute("data-call-phase"), "connected");
  assert.equal(statusEl.textContent, "Голосовая связь установлена • DAVE E2EE");
  assert.equal(panel.querySelector("[data-voice-retry-btn]"), null);

  renderVoiceRtcPhaseDom(
    panel,
    { phase: "timeout", error: "UDP IP Discovery timeout (5000 мс)" },
    { doc, onRetry },
  );
  assert.equal(panel.getAttribute("data-call-phase"), "timeout");
  assert.ok(
    String(statusEl.textContent).includes("UDP IP Discovery timeout (5000 мс)"),
  );
  const retryBtn = panel.querySelector("[data-voice-retry-btn]");
  assert.ok(retryBtn, "Кнопка повтора должна рендериться при фазе timeout/error");
  retryBtn.click();
  assert.equal(retryCalls, 1, "Клик по кнопке повтора вызывает onRetry");

  // Проверяем реактивную синхронизацию через store
  const store = createAppReactiveStore();
  const dispose = mountReactiveVoiceRtcPanel({ store, doc, onRetry });
  try {
    store.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: "g1",
      channelId: "v1",
      phase: "signaling",
    });
    assert.equal(panel.getAttribute("data-call-phase"), "signaling");
    assert.equal(statusEl.textContent, "Ожидание ответа сервера Discord...");

    store.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: "g1",
      channelId: "v1",
      phase: "negotiating",
    });
    assert.equal(panel.getAttribute("data-call-phase"), "negotiating");
    assert.equal(statusEl.textContent, "Проверка UDP IP Discovery и ключей DAVE...");

    store.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: "g1",
      channelId: "v1",
      phase: "connected",
    });
    assert.equal(panel.getAttribute("data-call-phase"), "connected");
    assert.equal(statusEl.textContent, "Голосовая связь установлена • DAVE E2EE");

    store.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: "g1",
      channelId: "v1",
      phase: "error",
      error: "Отказ DAVE MLS handshake",
    });
    assert.equal(panel.getAttribute("data-call-phase"), "error");
    assert.ok(String(statusEl.textContent).includes("Отказ DAVE MLS handshake"));
    const reactiveRetryBtn = panel.querySelector("[data-voice-retry-btn]");
    assert.ok(reactiveRetryBtn);
    reactiveRetryBtn.click();
    assert.equal(retryCalls, 2);
  } finally {
    dispose();
  }
});

test("V5.3: channel-tree-view рендерит участников голосового канала строго из подтверждённых VOICE_STATE_UPDATE без фантомных аватаров и немедленно удаляет при таймауте/выходе", () => {
  const doc = /** @type {any} */ (new ShimDocument());
  const slot = doc.createElement("div");
  slot.id = "voice-slot";
  doc.body.append(slot);

  /** @type {Map<string, Array<Record<string, any>>>} */
  const voiceStatesByGuild = new Map();

  // 1. Попытка оптимистичного (неподтверждённого) добавления до VOICE_STATE_UPDATE не должна создавать участника
  applyVoiceStateUpdateEvent(voiceStatesByGuild, {
    guild_id: "g1",
    channel_id: "vc-100",
    user_id: "u-self",
    confirmed: false,
    optimistic: true,
  });

  let participants = computeConfirmedVoiceParticipants({
    voiceStatesByGuild,
    guildId: "g1",
    channelId: "vc-100",
  });
  assert.equal(
    participants.length,
    0,
    "До подтверждённого VOICE_STATE_UPDATE список участников должен быть пустым (без фантомных аватаров)",
  );

  renderVoiceChannelParticipantsDom(slot, participants, { channelId: "vc-100", doc });
  assert.equal(slot.querySelectorAll(".voice-channel-user").length, 0);

  // 2. Приходит подтверждённый VOICE_STATE_UPDATE с session_id для u-self и u-peer
  applyVoiceStateUpdateEvent(voiceStatesByGuild, {
    t: "VOICE_STATE_UPDATE",
    guild_id: "g1",
    channel_id: "vc-100",
    user_id: "u-self",
    session_id: "sess-1",
    self_mute: false,
    self_deaf: false,
  });
  applyVoiceStateUpdateEvent(voiceStatesByGuild, {
    t: "VOICE_STATE_UPDATE",
    guild_id: "g1",
    channel_id: "vc-100",
    user_id: "u-peer",
    session_id: "sess-2",
    self_mute: true,
    self_deaf: false,
  });

  participants = computeConfirmedVoiceParticipants({
    voiceStatesByGuild,
    guildId: "g1",
    channelId: "vc-100",
  });
  assert.deepEqual(
    participants.map((p) => p.userId),
    ["u-self", "u-peer"],
  );

  renderVoiceChannelParticipantsDom(slot, participants, { channelId: "vc-100", doc });
  assert.equal(slot.querySelectorAll(".voice-channel-user").length, 2);

  // 3. Таймаут или ошибка звонка для u-self -> немедленное удаление из канала
  applyVoiceStateUpdateEvent(voiceStatesByGuild, {
    guild_id: "g1",
    channel_id: "vc-100",
    user_id: "u-self",
    phase: "timeout",
    reason: "Signaling timeout",
  });
  participants = computeConfirmedVoiceParticipants({
    voiceStatesByGuild,
    guildId: "g1",
    channelId: "vc-100",
  });
  assert.deepEqual(
    participants.map((p) => p.userId),
    ["u-peer"],
    "При таймауте участник должен немедленно удаляться",
  );

  // 4. Выход u-peer (VOICE_STATE_UPDATE с channel_id: null) -> немедленное удаление
  applyVoiceStateUpdateEvent(voiceStatesByGuild, {
    t: "VOICE_STATE_UPDATE",
    guild_id: "g1",
    channel_id: null,
    user_id: "u-peer",
    session_id: "sess-2",
  });
  participants = computeConfirmedVoiceParticipants({
    voiceStatesByGuild,
    guildId: "g1",
    channelId: "vc-100",
  });
  assert.equal(participants.length, 0);
  renderVoiceChannelParticipantsDom(slot, participants, { channelId: "vc-100", doc });
  assert.equal(slot.querySelectorAll(".voice-channel-user").length, 0);
});

test("V5.4: синхронизация баннеров и блокировки ввода для lurker, pending:true и отсутствия прав SEND_MESSAGES / CONNECT", () => {
  const doc = /** @type {any} */ (new ShimDocument());
  const chatMain = doc.createElement("div");
  chatMain.className = "chat-main";
  const compForm = doc.createElement("form");
  compForm.id = "composer";
  const input = doc.createElement("textarea");
  input.id = "composer-input";
  const submitBtn = doc.createElement("button");
  submitBtn.id = "composer-submit";
  submitBtn.type = "submit";
  compForm.append(input, submitBtn);
  chatMain.append(compForm);
  doc.body.append(chatMain);

  const baseGuild = {
    id: "100",
    owner_id: "999",
    is_member: true,
    lurker: false,
    pending: false,
    roles: [
      {
        id: "100",
        // VIEW_CHANNEL (1<<10) | SEND_MESSAGES (1<<11) | CONNECT (1<<20)
        permissions: String((1n << 10n) | (1n << 11n) | (1n << 20n)),
      },
    ],
    my_roles: [],
  };
  const textChannel = { id: "c-text", name: "general", type: 0, permission_overwrites: [] };
  const voiceChannel = { id: "c-voice", name: "Lobby", type: 2, permission_overwrites: [] };

  // 1. Lurker (is_member: false / lurker: true)
  const lurkerGuild = { ...baseGuild, is_member: false, lurker: true };
  const lurkerTree = computeGuildChannelTreeModel({
    guild: lurkerGuild,
    channels: [textChannel, voiceChannel],
    currentUserId: "u1",
  });
  assert.equal(lurkerTree.isPreviewMode, true);
  assert.equal(lurkerTree.visibleChannels[0]?.access.canSendMessages, false);
  assert.equal(lurkerTree.visibleChannels[1]?.access.canConnectVoice, false);

  syncServerAccessBannersAndComposerDom({
    doc,
    guild: lurkerGuild,
    channel: textChannel,
    currentUserId: "u1",
  });
  assert.equal(input.disabled, true);
  assert.equal(submitBtn.disabled, true);
  const previewBanner = doc.getElementById("guild-preview-join-banner");
  assert.ok(previewBanner, "Баннер предпросмотра для lurker должен отображаться");
  assert.equal(previewBanner.getAttribute("data-access-banner"), "lurker");

  // 2. Pending: true (Membership Screening не пройден)
  const pendingGuild = { ...baseGuild, is_member: true, lurker: false, pending: true };
  const pendingTree = computeGuildChannelTreeModel({
    guild: pendingGuild,
    channels: [textChannel, voiceChannel],
    currentUserId: "u1",
  });
  assert.equal(pendingTree.isPendingScreening, true);
  assert.equal(pendingTree.visibleChannels[0]?.access.canSendMessages, false);
  assert.equal(pendingTree.visibleChannels[1]?.access.canConnectVoice, false);

  syncServerAccessBannersAndComposerDom({
    doc,
    guild: pendingGuild,
    channel: textChannel,
    currentUserId: "u1",
  });
  assert.equal(doc.getElementById("guild-preview-join-banner"), null, "Баннер lurker должен исчезнуть");
  const pendingBanner = doc.getElementById("guild-pending-screening-banner");
  assert.ok(pendingBanner, "Баннер pending:true должен отображаться");
  assert.equal(pendingBanner.getAttribute("data-access-banner"), "pending");
  assert.equal(input.disabled, true);
  assert.equal(submitBtn.disabled, true);
  assert.ok(input.placeholder.includes("Примите правила сервера"));

  // 3. Отсутствие права SEND_MESSAGES у обычного участника
  const noSendChannel = {
    ...textChannel,
    permission_overwrites: [
      { id: "100", type: 0, allow: "0", deny: String(1n << 11n) },
    ],
  };
  syncServerAccessBannersAndComposerDom({
    doc,
    guild: baseGuild,
    channel: noSendChannel,
    currentUserId: "u1",
  });
  assert.equal(doc.getElementById("guild-pending-screening-banner"), null);
  const readOnlyBanner = doc.getElementById("channel-readonly-access-banner");
  assert.ok(readOnlyBanner, "Баннер отсутствия SEND_MESSAGES должен отображаться");
  assert.equal(readOnlyBanner.getAttribute("data-access-banner"), "missing-send-messages");
  assert.equal(input.disabled, true);
  assert.equal(submitBtn.disabled, true);
  assert.ok(input.placeholder.includes("нет прав на отправку сообщений"));

  // 4. Отсутствие права CONNECT в голосовом канале
  const noConnectVoiceChannel = {
    ...voiceChannel,
    permission_overwrites: [
      { id: "100", type: 0, allow: "0", deny: String(1n << 20n) },
    ],
  };
  const noConnectState = syncServerAccessBannersAndComposerDom({
    doc,
    guild: baseGuild,
    channel: noConnectVoiceChannel,
    currentUserId: "u1",
  });
  assert.equal(noConnectState.canConnectVoice, false);
  const noConnectBanner = doc.getElementById("channel-readonly-access-banner");
  assert.ok(noConnectBanner, "Баннер отсутствия CONNECT должен отображаться");
  assert.equal(noConnectBanner.getAttribute("data-access-banner"), "missing-connect");

  // 5. Полный доступ: все баннеры убираются, ввод разблокируется
  syncServerAccessBannersAndComposerDom({
    doc,
    guild: baseGuild,
    channel: textChannel,
    currentUserId: "u1",
  });
  assert.equal(doc.getElementById("guild-preview-join-banner"), null);
  assert.equal(doc.getElementById("guild-pending-screening-banner"), null);
  assert.equal(doc.getElementById("channel-readonly-access-banner"), null);
  assert.equal(input.disabled, false);
  assert.equal(submitBtn.disabled, false);
});

test("V5.5: интеграция с app.js — отсутствие фантомных аватаров до VOICE_STATE_UPDATE и смена 4 фаз звонка в #voice-panel", async () => {
  const prevImage = globalThis.Image;
  const prevDisabled = /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__;
  /** @type {any} */
  let gui = null;
  let rtcMockState = {
    state: "signaling",
    error: null,
  };
  let voiceConnectCalls = 0;

  try {
    gui = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "messages_page") {
          return { items: [], hasMore: false, nextBefore: null };
        }
        if (cmd === "voice_connect") {
          voiceConnectCalls += 1;
          return { ...rtcMockState };
        }
        if (
          cmd === "voice_disconnect" ||
          cmd === "voice_set_audio_device" ||
          cmd === "voice_rtc_status" ||
          cmd === "voice_audio_devices"
        ) {
          return { ...rtcMockState };
        }
        return undefined;
      },
    });
    globalThis.Image = /** @type {any} */ (class FakeImage {});
    /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = false;

    const document = gui.window.document;
    const app = gui.appModule.currentApp();
    assert.ok(app);

    await app.chat.openChannel({
      id: "1566834523038294047",
      name: "DemoUser",
      type: 1,
    });

    const dmVoiceBtn = document.querySelector('#dm-header-call-actions [data-action="dm-call-voice"]');
    assert.ok(dmVoiceBtn, "Кнопка голосового звонка в DM должна существовать");
    dmVoiceBtn.click();
    await gui.settle(5);

    const voicePanel = document.getElementById("voice-panel");
    const statusEl = document.getElementById("live-rtc-conn-status");
    assert.ok(voicePanel);
    assert.ok(statusEl);
    assert.equal(
      statusEl.textContent,
      "Ожидание ответа сервера Discord...",
      "В фазе signaling должен выводиться текст 'Ожидание ответа сервера Discord...'",
    );
    assert.equal(voiceConnectCalls, 1);

    // Фаза 2: приход VOICE_SERVER_UPDATE переводит звонок в фазу negotiating
    rtcMockState = {
      state: "negotiating",
      error: null,
    };
    app.dispatch.handle({
      kind: "VOICE_SERVER_UPDATE",
      data: {
        guild_id: "@me",
        channel_id: "1566834523038294047",
        endpoint: "c-fra18-233f52bd.discord.media:443",
        token: "secret-voice-token",
      },
    });
    await gui.settle(3);
    assert.equal(
      statusEl.textContent,
      "Проверка UDP IP Discovery и ключей DAVE...",
      "После VOICE_SERVER_UPDATE должна отображаться фаза negotiating",
    );

    // Фаза 3: переход в connected
    rtcMockState = {
      state: "connected",
      error: null,
    };
    globalAppReactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: "@me",
      channelId: "1566834523038294047",
      phase: "connected",
      state: "connected",
    });
    assert.equal(
      statusEl.textContent,
      "Голосовая связь установлена • DAVE E2EE",
      "В фазе connected должен отображаться текст 'Голосовая связь установлена • DAVE E2EE'",
    );

    // Фаза 4: ошибка/таймаут и кнопка повтора
    rtcMockState = {
      state: "signaling",
      error: null,
    };
    globalAppReactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: "@me",
      channelId: "1566834523038294047",
      phase: "timeout",
      state: "timeout",
      error: "Превышен интервал UDP IP Discovery (5000 мс)",
    });
    assert.equal(
      statusEl.textContent,
      "Превышен интервал UDP IP Discovery (5000 мс)",
    );
    const retryBtn = voicePanel.querySelector("[data-voice-retry-btn]");
    assert.ok(retryBtn, "В фазе timeout должна отображаться кнопка повтора в #voice-panel");
    retryBtn.click();
    await gui.settle(5);
    assert.equal(
      voiceConnectCalls,
      2,
      "Клик по кнопке повтора в #voice-panel должен повторно вызывать voice_connect",
    );
  } finally {
    globalThis.Image = prevImage;
    /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = prevDisabled;
    gui?.dispose?.();
  }
});
