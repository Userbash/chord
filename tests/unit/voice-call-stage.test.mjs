// @ts-check
import assert from "node:assert/strict";
import test from "node:test";

import { ShimDocument } from "../support/gui-dom-harness.mjs";
import { createVoiceCallStage } from "../../rust/gui-spike/ui/features/voice-call-stage.js";

test("VoiceCallStage: Cleanroom Parity голосовой сцены и стрима (Скриншоты 1 и 2)", async (t) => {
  const doc = new ShimDocument();
  globalThis.document = /** @type {any} */ (doc);

  const mountPoint = doc.createElement("div");
  mountPoint.id = "chat-main";

  await t.test("1. Монтирование сцены аудио-звонка с аватарами и плавающей панелью (Скриншот 1)", () => {
    const stage = createVoiceCallStage(/** @type {any} */ (mountPoint), {
      initialConnected: true,
      initialChannelName: "Rustery",
      initialStreaming: false,
    });

    const root = mountPoint.querySelector(".voice-call-stage-root");
    assert.ok(root, "Корневой элемент voice-call-stage-root должен быть смонтирован");
    assert.equal(root.getAttribute("data-mode"), "compact");

    // Заголовок
    const status = root.querySelector(".voice-call-header-status");
    assert.ok(status?.textContent?.includes("Rustery"), "Заголовок должен содержать имя Rustery");
    assert.ok(status?.textContent?.includes("Участвует в звонке"), "Заголовок должен содержать статус звонка");

    // Карточки аватаров
    const cards = root.querySelectorAll(".voice-call-participant-card");
    assert.equal(cards.length, 2, "Должно отображаться 2 участника звонка");

    // Плавающий док управления
    const dock = root.querySelector(".voice-call-floating-dock");
    assert.ok(dock, "Плавающий док управления должен присутствовать");

    const micBtn = dock?.querySelector("button[data-action=\"mute\"]");
    assert.ok(micBtn, "Кнопка микрофона присутствует");

    const streamBtn = dock?.querySelector("button[data-action=\"screen-share\"]");
    assert.ok(streamBtn, "Кнопка демонстрации экрана присутствует");

    const discBtn = dock?.querySelector("button[data-action=\"disconnect\"]");
    assert.ok(discBtn, "Красная кнопка завершения звонка присутствует");

    stage.destroy();
  });

  await t.test("2. Переключение в режим стрима с бейджем 'В ЭФИРЕ' (Скриншот 2)", () => {
    const stage = createVoiceCallStage(/** @type {any} */ (mountPoint), {
      initialConnected: true,
      initialChannelName: "Rustery",
      initialStreaming: true,
    });

    const root = mountPoint.querySelector(".voice-call-stage-root");
    assert.equal(root?.getAttribute("data-mode"), "stream");

    // Бейдж 'В ЭФИРЕ'
    const liveBadge = root?.querySelector(".voice-call-header-badge-live");
    assert.ok(liveBadge?.textContent?.includes("В ЭФИРЕ"), "Бейдж 'В ЭФИРЕ' должен быть виден");

    // Контейнер видеопотока
    const streamContainer = root?.querySelector(".voice-call-stream-container");
    assert.ok(streamContainer, "Контейнер стрима должен присутствовать");

    const overlay = root?.querySelector(".voice-call-stream-overlay-tag");
    assert.ok(overlay?.textContent?.includes("Rustery"), "Оверлей стрима должен показывать имя стримера Rustery");

    stage.destroy();
  });

  await t.test("3. Реактивное переключение индикатора говорения (зеленое кольцо)", () => {
    const stage = createVoiceCallStage(/** @type {any} */ (mountPoint), {
      initialConnected: true,
      initialChannelName: "Rustery",
      initialStreaming: false,
    });

    stage.setSpeaking("self", true);
    const selfCard = mountPoint.querySelector(".voice-call-participant-card[data-speaking=\"true\"]");
    assert.ok(selfCard, "Карточка говорящего участника должна получить data-speaking='true'");

    stage.setSpeaking("self", false);
    const notSpeaking = mountPoint.querySelector(".voice-call-participant-card[data-speaking=\"true\"]");
    assert.equal(notSpeaking, null, "data-speaking должен сброситься в false");

    stage.destroy();
  });

  await t.test("4. Динамическое обновление участников, аватаров, превью стрима (720p 60fps) и состояния дока", () => {
    const callbacks = { muted: 0, cam: 0, share: 0, disc: 0 };
    const stage = createVoiceCallStage(/** @type {any} */ (mountPoint), {
      id: "dm-call-stage-banner",
      initialConnected: true,
      initialChannelName: "Alice",
      initialStreaming: false,
      onMute: () => { callbacks.muted += 1; },
      onCamera: () => { callbacks.cam += 1; },
      onScreenShare: () => { callbacks.share += 1; },
      onDisconnect: () => { callbacks.disc += 1; },
    });

    assert.equal(stage.element.id, "dm-call-stage-banner");

    stage.setParticipants([
      { id: "1", name: "Alice", avatarUrl: "https://cdn.discordapp.com/avatars/1/abc.png", speaking: true, muted: false },
      { id: "2", name: "Bob", avatarUrl: null, speaking: false, muted: true, ringing: true },
    ]);

    const cards = stage.element.querySelectorAll(".voice-call-participant-card");
    assert.equal(cards.length, 2);
    assert.equal(cards[0]?.getAttribute("data-speaking"), "true");
    assert.ok(cards[0]?.querySelector(".voice-call-avatar-img"), "Участник с avatarUrl должен иметь img");
    assert.ok(cards[1]?.querySelector(".voice-call-avatar-placeholder"), "Участник без avatarUrl должен иметь placeholder");

    stage.setControlsState({ muted: true, cameraOn: true, streaming: true });
    const micBtn = /** @type {HTMLElement | null} */ (stage.element.querySelector("button[data-action=\"mute\"]"));
    const camBtn = /** @type {HTMLElement | null} */ (stage.element.querySelector("button[data-action=\"camera\"]"));
    assert.equal(micBtn?.getAttribute("data-muted"), "true");
    assert.equal(camBtn?.getAttribute("data-active"), "true");

    micBtn?.click();
    camBtn?.click();
    /** @type {HTMLElement | null} */ (stage.element.querySelector("button[data-action=\"screen-share\"]"))?.click();
    /** @type {HTMLElement | null} */ (stage.element.querySelector("button[data-action=\"disconnect\"]"))?.click();
    assert.deepEqual(callbacks, { muted: 1, cam: 1, share: 1, disc: 1 });

    stage.setStreaming(true, "720p 60fps");
    stage.setStreamPreview({
      dataUrl: "data:image/png;base64,AAAA",
      streamerName: "Alice",
      quality: "720p 60fps",
    });

    assert.equal(stage.element.dataset.mode, "stream");
    assert.equal(stage.element.querySelector(".voice-call-header-badge-live")?.textContent, "В ЭФИРЕ");
    assert.equal(stage.element.querySelector(".voice-call-header-badge-quality")?.textContent, "720p 60fps");
    const streamImg = stage.element.querySelector(".voice-call-stream-img");
    assert.ok(streamImg, "Превью стрима должно рендерить img");

    stage.destroy();
  });
});
