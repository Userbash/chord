// @ts-check
// RED (M2): Frida adapter contract with injected device/session/script fakes.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
/** @typedef {import("frida").Device} Device */
/** @typedef {import("frida").Session} Session */
/** @typedef {import("frida").Script} Script */
import {
  attach,
  collectInventory,
  listProcesses,
  loadAgentSource,
  loadDevice,
  parseHookMessage,
  startHook,
  stopHook,
} from "../../src/adapters/frida.mjs";

describe("loadDevice", () => {
  it("is exported as a deferred device loader", () => {
    assert.equal(typeof loadDevice, "function");
  });
});

describe("listProcesses", () => {
  it("maps pid and name in source order", async () => {
    const device = /** @type {Pick<Device, "enumerateProcesses">} */ (/** @type {unknown} */ ({ enumerateProcesses: async () => [
      { pid: 1, name: "Discord", parameters: {} },
      { pid: 2, name: "helper", parameters: {} },
    ] }));
    assert.deepEqual(await listProcesses(device), [
      { pid: 1, name: "Discord" },
      { pid: 2, name: "helper" },
    ]);
  });

  it("returns an empty list for no processes", async () => {
    const device = /** @type {Pick<Device, "enumerateProcesses">} */ (/** @type {unknown} */ ({ enumerateProcesses: async () => [] }));
    assert.deepEqual(await listProcesses(device), []);
  });
});

describe("loadAgentSource", () => {
  it("loads the Frida logging agent", async () => {
    assert.match(await loadAgentSource(), /Interceptor\.attach\(/);
  });
});

describe("attach", () => {
  it("creates, connects before loading, and captures messages in the returned array", async () => {
    /** @type {string[]} */
    const calls = [];
    /** @type {((message: import("frida").Message) => void) | undefined} */
    let onMessage;
    const message = { type: "send", payload: { type: "ready" } };
    const script = /** @type {Script} */ (/** @type {unknown} */ ({
      message: { connect(/** @type {(message: import("frida").Message) => void} */ callback) { calls.push("connect"); onMessage = callback; } },
      load: async () => { calls.push("load"); },
    }));
    const session = /** @type {Session} */ (/** @type {unknown} */ ({
      createScript: async (/** @type {string} */ source, /** @type {{ name: string }} */ options) => {
        calls.push("createScript");
        assert.equal(source, "FAKE");
        assert.deepEqual(options, { name: "discord-lab" });
        return script;
      },
    }));
    const device = /** @type {Pick<Device, "attach">} */ (/** @type {unknown} */ ({ attach: async (/** @type {number} */ pid) => {
      assert.equal(pid, 42);
      return session;
    } }));
    const result = await attach(device, 42, { agentSource: "FAKE" });
    assert.deepEqual(calls, ["createScript", "connect", "load"]);
    assert.equal(result.session, session);
    assert.equal(result.script, script);
    onMessage?.(/** @type {import("frida").Message} */ (/** @type {unknown} */ (message)));
    assert.equal(result.messages[0], message);
  });
});

describe("collectInventory", () => {
  it("passes options to inventory and returns its result", async () => {
    const options = { modules: ["discord.node"], maxSymbols: 10 };
    const inventory = [{ name: "discord.node" }];
    const script = /** @type {import("../../src/adapters/frida.mjs").AgentScript} */ (/** @type {unknown} */ ({ exports: { inventory: async (/** @type {object} */ received) => {
      assert.equal(received, options);
      return inventory;
    } } }));
    assert.equal(await collectInventory(script, options), inventory);
  });
});

describe("parseHookMessage", () => {
  it("returns call payloads from send messages only", () => {
    const payload = { type: "call", module: "discord.node", name: "f", address: "0x1" };
    assert.equal(parseHookMessage({ type: "send", payload }), payload);
    assert.equal(parseHookMessage({ type: "error", description: "failed" }), null);
    assert.equal(parseHookMessage({ type: "send", payload: { type: "ready" } }), null);
    assert.equal(parseHookMessage({ type: "send", payload: null }), null);
  });
});

describe("startHook", () => {
  it("passes options to hook and returns its result", async () => {
    const options = { pattern: "Task", limit: 2 };
    const result = [{ module: "discord.node" }];
    const script = /** @type {import("../../src/adapters/frida.mjs").AgentScript} */ (/** @type {unknown} */ ({ exports: { hook: async (/** @type {object} */ received) => {
      assert.equal(received, options);
      return result;
    } } }));
    assert.equal(await startHook(script, options), result);
  });
});

describe("stopHook", () => {
  it("calls unhook and unload", async () => {
    /** @type {string[]} */
    const calls = [];
    const script = /** @type {import("../../src/adapters/frida.mjs").AgentScript} */ (/** @type {unknown} */ ({ exports: { unhook: async () => { calls.push("unhook"); } }, unload: async () => { calls.push("unload"); } }));
    await stopHook(script);
    assert.deepEqual(calls, ["unhook", "unload"]);
  });

  it("swallows unhook rejection and still unloads", async () => {
    /** @type {string[]} */
    const calls = [];
    const script = /** @type {import("../../src/adapters/frida.mjs").AgentScript} */ (/** @type {unknown} */ ({ exports: { unhook: async () => { calls.push("unhook"); throw new Error("gone"); } }, unload: async () => { calls.push("unload"); } }));
    await assert.doesNotReject(() => stopHook(script));
    assert.deepEqual(calls, ["unhook", "unload"]);
  });
});
