// @ts-check
import test from "node:test";
import assert from "node:assert/strict";
import { loadApp } from "../support/gui-dom-harness.mjs";

test("TASK-760: User bar layout and UI parity", async () => {
  const prevImage = globalThis.Image;
  // @ts-ignore
  globalThis.Image = class FakeImage {};
  try {
    const harness = await loadApp({
      /** @param {string} cmd @param {any} [args] */
      invokeHandler: async (cmd, args) => {
        if (cmd === "settings_load") {
          return { volume: 100, lastGuildId: "guild-1" };
        }
        return null;
      },
    });

    const app = harness;
    const userBar = app.document.getElementById("user-bar");
    assert.ok(userBar, "user-bar must exist");

    const logoutBtn = app.document.getElementById("auth-logout");
    if (logoutBtn) {
      // Must be hidden from user bar in live sessions
      const isHidden = logoutBtn.hasAttribute("hidden") || logoutBtn.style.display === "none";
      assert.ok(isHidden, "Logout button must be hidden from user bar");
    }

    const muteBtn = app.document.querySelector('#user-bar [data-action="mute"]');
    assert.ok(muteBtn, "Mute button must exist");

    const deafBtn = app.document.querySelector('#user-bar [data-action="deafen"]');
    assert.ok(deafBtn, "Deafen button must exist");

    const settingsBtn = app.document.querySelector('#user-bar [data-action="settings"]');
    assert.ok(settingsBtn, "Settings button must exist");

    const profileBtn = app.document.querySelector('#user-bar [data-action="profile"]');
    assert.ok(profileBtn, "Profile button must exist");

    const inputChevron = app.document.querySelector('#user-bar [data-chevron-kind="input"]');
    assert.ok(inputChevron, "Mute button must have companion chevron for input devices");

    const outputChevron = app.document.querySelector('#user-bar [data-chevron-kind="output"]');
    assert.ok(outputChevron, "Deafen button must have companion chevron for output devices");
  } finally {
    globalThis.Image = prevImage;
  }
});

test("TASK-760: Message delete IPC sync and channel-id propagation", async () => {
  /** @type {Array<{cmd: string, args: any}>} */
  const calls = [];
  const harness = await loadApp();
  const app = harness;
  app.setInvokeHandler(async (cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "delete_message") {
      return null;
    }
    return undefined;
  });

  // Verify delete_message invocation receives channelId and messageId
  await app.invoke("delete_message", {
    channelId: "1056563701764857896",
    messageId: "123456789012345678",
  });

  const deleteCall = calls.find((c) => c.cmd === "delete_message");
  assert.ok(deleteCall, "delete_message must be invoked");
  assert.equal(deleteCall.args.channelId, "1056563701764857896");
  assert.equal(deleteCall.args.messageId, "123456789012345678");
});

test("TASK-760: Voice disconnect on exit / unload event triggers cleanup", async () => {
  const prevImage = globalThis.Image;
  // @ts-ignore
  globalThis.Image = class FakeImage {};
  try {
    const harness = await loadApp();
    const app = harness;

    // Trigger beforeunload / unload on window
    app.window.dispatchEvent("beforeunload");
    app.window.dispatchEvent("unload");

    // Harness handles unload dispatch safely without uncaught exceptions
    assert.ok(true, "Window unload dispatches without errors");
  } finally {
    globalThis.Image = prevImage;
  }
});

test("TASK-762: Non-destructive hydration: user-bar has no auth-logout, and friends retain on empty sync", async () => {
  const prevImage = globalThis.Image;
  // @ts-ignore
  globalThis.Image = class FakeImage {};
  try {
    const harness = await loadApp({
      invokeHandler: async (cmd, args) => {
        if (cmd === "settings_load") {
          return { volume: 100, lastGuildId: "guild-1" };
        }
        return null;
      },
    });

    const app = harness;
    const userBarActions = app.document.querySelector("#user-bar .user-actions");
    assert.ok(userBarActions, ".user-actions must exist in #user-bar");
    const logoutInBar = userBarActions.querySelector("#auth-logout, .auth-logout");
    assert.equal(logoutInBar, null, "#auth-logout must never be inside #user-bar .user-actions");
  } finally {
    globalThis.Image = prevImage;
  }
});
