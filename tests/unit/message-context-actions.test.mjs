// @ts-check
import test from "node:test";
import assert from "node:assert/strict";
import { loadApp } from "../support/gui-dom-harness.mjs";
import { parseChannelPage, renderChannelMessage } from "../../rust/gui-spike/ui/features/chat.js";
import { createMessageToolbar } from "../../rust/gui-spike/ui/features/message-toolbar.js";
import { createJoinServer } from "../../rust/gui-spike/ui/features/join-server.js";

test("TDD: parseChannelPage preserves messageType and call metadata", () => {
  const payload = {
    items: [
      {
        id: "1568204902230921359",
        author: { id: "1566799713523466370", username: "rustery0330", globalName: "Rustery" },
        content: "",
        timestamp: "2026-11-06T10:06:24.530000+00:00",
        message_type: 3,
        call: {
          ended_timestamp: "2026-11-06T10:10:26.135000+00:00",
          participants: ["384299970632876032", "1566799713523466370"],
        },
      },
    ],
  };

  const rows = parseChannelPage("1566834523038294047", payload);
  assert.equal(rows.length, 1);
  const firstRow = rows[0];
  assert.ok(firstRow);
  assert.equal(firstRow.messageType, 3);
  assert.ok(firstRow.call);
  assert.equal(firstRow.call.participants?.length, 2);
});

test("TDD: renderChannelMessage sets data-message-type and renders call duration & missed call states", async () => {
  const gui = await loadApp();
  const document = gui.window.document;

  /** @type {import("../../rust/gui-spike/ui/features/chat.js").ChannelMessage} */
  const endedCallItem = {
    id: "1568204902230921359",
    author: "Rustery",
    authorId: "1566799713523466370",
    authorAvatar: null,
    text: "",
    time: "10:06",
    iso: "2026-11-06T10:06:24.530000+00:00",
    day: "2026-11-06",
    dayLabel: "",
    group: "start",
    edited: false,
    bot: false,
    hue: 120,
    channelId: "1566834523038294047",
    reactions: [],
    attachments: [],
    embeds: [],
    flags: 0,
    isPoll: false,
    messageType: 3,
    call: {
      ended_timestamp: "2026-11-06T10:10:26.135000+00:00",
      participants: ["384299970632876032", "1566799713523466370"],
    },
  };

  const row = document.createElement("div");
  renderChannelMessage(/** @type {any} */ (row), endedCallItem, 0, 1, { channelId: "1566834523038294047" });

  assert.equal(row.getAttribute("data-message-type"), "3");
  const textBody = row.querySelector(".text-body");
  assert.ok(textBody);
  assert.match(textBody.textContent || "", /4\s*мин/i);
});

test("TDD: createMessageToolbar hides edit and delete buttons on system call messages and unauthorized rows", async () => {
  const gui = await loadApp();
  const document = gui.window.document;
  const list = document.createElement("div");
  const container = document.createElement("div");

  const systemRow = document.createElement("div");
  systemRow.className = "row channel-message";
  systemRow.setAttribute("data-index", "0");
  systemRow.setAttribute("data-message-id", "1568204902230921359");
  systemRow.setAttribute("data-author-id", "1566799713523466370");
  systemRow.setAttribute("data-message-type", "3");
  list.append(systemRow);

  const foreignDmRow = document.createElement("div");
  foreignDmRow.className = "row channel-message";
  foreignDmRow.setAttribute("data-index", "1");
  foreignDmRow.setAttribute("data-message-id", "1568204902230921360");
  foreignDmRow.setAttribute("data-author-id", "384299970632876032");
  foreignDmRow.setAttribute("data-message-type", "0");
  list.append(foreignDmRow);

  const toolbar = createMessageToolbar({
    list: /** @type {any} */ (list),
    container: /** @type {any} */ (container),
    renderRow: () => {},
    indexLabel: (i) => `#${i}`,
    authorOf: () => "Rustery",
    getComposer: () => null,
    canEdit: (i) => i === 2,
    canDelete: (i) => i === 2,
  });

  toolbar.open(0, /** @type {any} */ (systemRow));
  const editBtn = toolbar.button("Изменить сообщение");
  const deleteBtn = toolbar.button("Удалить сообщение");
  assert.ok(editBtn);
  assert.ok(deleteBtn);
  assert.equal(editBtn.style.display, "none", "Edit button must be hidden on system call messages");
  assert.equal(deleteBtn.style.display, "none", "Delete button must be hidden on system call messages");

  toolbar.open(1, /** @type {any} */ (foreignDmRow));
  assert.equal(editBtn.style.display, "none", "Edit button must be hidden when canEdit is false");
  assert.equal(deleteBtn.style.display, "none", "Delete button must be hidden when canDelete is false");
});

test("TDD: join-server preview renders open button for guildId and accepts invite on second submit", async () => {
  const gui = await loadApp();
  const document = gui.window.document;

  /** @type {Array<{cmd: string, args: any}>} */
  const calls = [];
  let joinedGuildId = null;

  const modal = createJoinServer({
    invoke: async (cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "resolve_invite") {
        return {
          code: "eSrxp2YGUC",
          guildId: "274093052673392641",
          guildName: "ITSKIDON PC BOOST",
          channelId: "274093052673392642",
          memberCount: 1500,
          presenceCount: 250,
        };
      }
      if (cmd === "accept_invite") {
        return {
          guildId: "274093052673392641",
          targetChannelId: "274093052673392642",
        };
      }
      throw new Error(`Unexpected IPC: ${cmd}`);
    },
    onAccepted: (guild) => {
      joinedGuildId = guild.id;
    },
  });

  modal.open();
  const input = /** @type {any} */ (document.getElementById("join-invite-input"));
  input.value = "https://discord.com/invite/eSrxp2YGUC";
  input.dispatchEvent(/** @type {any} */ ({ type: "input", bubbles: true }));
  await modal.submit();

  assert.equal(modal.state, "preview_ready");
  assert.equal(modal.preview?.guildId, "274093052673392641");

  await modal.submit();
  assert.equal(joinedGuildId, "274093052673392641");
  assert.ok(calls.some((c) => c.cmd === "accept_invite"));
});

test("TDD: DM call stage banner mounts inside .chat-main (not flex-row #screen-chat), voice-panel subtitle omits duplicate guild name, and dm-call-stream opens screenshare modal", async () => {
  const prevImage = globalThis.Image;
  const prevDisabled = /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__;
  /** @type {any} */
  let gui = null;

  try {
    gui = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "messages_page") {
          return { items: [], hasMore: false, nextBefore: null };
        }
        if (
          cmd === "voice_connect" ||
          cmd === "voice_disconnect" ||
          cmd === "voice_set_audio_device" ||
          cmd === "voice_rtc_status" ||
          cmd === "voice_audio_devices"
        ) {
          return { ok: true, state: "connected", lastPingMs: 38 };
        }
        return undefined;
      },
    });
    globalThis.Image = /** @type {any} */ (class FakeImage {});
    /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = false;

    const document = gui.window.document;
    const app = gui.appModule.currentApp();
    assert.ok(app, "currentApp() must be initialized");

    await app.chat.openChannel({
      id: "1566834523038294047",
      name: "ShadowByte",
      type: 1,
    });

    const dmStreamBtn = document.querySelector('#dm-header-call-actions [data-action="dm-call-stream"]');
    assert.ok(dmStreamBtn, "dm-call-stream button must be rendered in DM header");
    dmStreamBtn.click();

    const chatScreen = document.getElementById("screen-chat");
    const chatMain = chatScreen?.querySelector(".chat-main");
    const stageBanner = document.getElementById("dm-call-stage-banner");
    assert.ok(stageBanner, "#dm-call-stage-banner must be rendered when DM call starts");
    assert.equal(
      stageBanner.parentNode?.className,
      "chat-main",
      "#dm-call-stage-banner must be mounted inside .chat-main (column flex), never directly in #screen-chat (row flex)",
    );
    assert.equal(stageBanner.parentNode === chatMain, true);

    const modal = document.getElementById("live-screenshare-modal");
    assert.ok(modal, "Clicking dm-call-stream must open #live-screenshare-modal");

    const voicePanelText = document.getElementById("voice-panel")?.textContent || "";
    assert.ok(
      !voicePanelText.includes("ShadowByte / ShadowByte") && !voicePanelText.includes("@ShadowByte / @ShadowByte"),
      `Voice panel subtitle must not duplicate DM name as guild name (got: ${voicePanelText})`,
    );
  } finally {
    gui?.window?.document?.querySelector('[data-action="dm-stage-leave"]')?.click();
    gui?.dispose();
    globalThis.Image = prevImage;
    /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = prevDisabled;
  }
});

test("ChatFeature call message (type 3) formats ended call duration with a single period (no double dot ..)", async () => {
  const gui = await loadApp();
  try {
    const document = gui.window.document;
    /** @type {import("../../rust/gui-spike/ui/features/chat.js").ChannelMessage} */
    const callMsg = {
      id: "1567070580172750969",
      author: "ShadowByte",
      authorId: "384299970632876032",
      authorAvatar: null,
      text: "",
      time: "11:30",
      iso: "2026-11-06T11:30:00.000Z",
      day: "2026-11-06",
      dayLabel: "",
      group: "start",
      edited: false,
      bot: false,
      hue: 120,
      channelId: "1566834523038294047",
      reactions: [],
      attachments: [],
      embeds: [],
      flags: 0,
      isPoll: false,
      messageType: 3,
      call: {
        ended_timestamp: "2026-11-06T11:33:00.000Z",
        participants: ["384299970632876032", "1566799713523466370"],
      },
    };
    const row = document.createElement("div");
    renderChannelMessage(/** @type {any} */ (row), callMsg, 0, 1, { channelId: "1566834523038294047" });
    const bodyText = row.querySelector(".text-body")?.textContent || "";
    assert.ok(
      bodyText.includes("3 мин."),
      `Expected call duration to include "3 мин.", got: ${bodyText}`,
    );
    assert.equal(
      bodyText.includes(".."),
      false,
      `Call duration text must never contain a double period "..", got: ${bodyText}`,
    );
  } finally {
    gui.dispose();
  }
});
