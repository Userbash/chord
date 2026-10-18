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
  it("creates and connects before loading, then drains messages by default", async () => {
    /** @type {string[]} */
    const calls = [];
    /** @type {((message: import("frida").Message) => void) | undefined} */
    let onMessage;
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
    for (let index = 0; index < 1000; index += 1) {
      onMessage?.(/** @type {import("frida").Message} */ (/** @type {unknown} */ ({ type: "send", payload: { index } })));
    }
    assert.equal(result.messages.length, 0);
  });

  it("captures messages when captureMessages is true", async () => {
    /** @type {string[]} */
    const calls = [];
    /** @type {((message: import("frida").Message) => void) | undefined} */
    let onMessage;
    const first = { type: "send", payload: { type: "ready" } };
    const second = { type: "send", payload: { type: "call" } };
    const script = /** @type {Script} */ (/** @type {unknown} */ ({
      message: { connect(/** @type {(message: import("frida").Message) => void} */ callback) { calls.push("connect"); onMessage = callback; } },
      load: async () => { calls.push("load"); },
    }));
    const session = /** @type {Session} */ (/** @type {unknown} */ ({
      createScript: async () => { calls.push("createScript"); return script; },
    }));
    const device = /** @type {Pick<Device, "attach">} */ (/** @type {unknown} */ ({ attach: async () => session }));
    const result = await attach(device, 42, { agentSource: "FAKE", captureMessages: true });
    assert.deepEqual(calls, ["createScript", "connect", "load"]);
    onMessage?.(/** @type {import("frida").Message} */ (/** @type {unknown} */ (first)));
    onMessage?.(/** @type {import("frida").Message} */ (/** @type {unknown} */ (second)));
    assert.equal(result.messages.length, 2);
    assert.equal(result.messages[0], first);
  });

  it("без agentSource читает встроенный агент по умолчанию", async () => {
    /** @type {string[]} */
    const calls = [];
    let capturedSource = "";
    const script = /** @type {Script} */ (/** @type {unknown} */ ({
      message: { connect() {} },
      load: async () => { calls.push("load"); },
    }));
    const session = /** @type {Session} */ (/** @type {unknown} */ ({
      createScript: async (/** @type {string} */ source, /** @type {{ name: string }} */ options) => {
        calls.push("createScript");
        capturedSource = source;
        assert.deepEqual(options, { name: "discord-lab" });
        return script;
      },
    }));
    const device = /** @type {Pick<Device, "attach">} */ (/** @type {unknown} */ ({ attach: async () => session }));
    const result = await attach(device, 42);
    assert.deepEqual(calls, ["createScript", "load"]);
    assert.equal(result.session, session);
    assert.ok(typeof capturedSource === "string" && capturedSource.includes("Interceptor.attach"));
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
  it("нормализует RegExp-паттерн в строку до отправки агенту", async () => {
    /** @type {{ pattern?: unknown, limit?: number }} */
    let captured = {};
    const script = /** @type {import("../../src/adapters/frida.mjs").AgentScript} */ (/** @type {unknown} */ ({ exports: { hook: async (/** @type {object} */ received) => { captured = /** @type {{ pattern?: unknown, limit?: number }} */ (received); return {}; } } }));
    await startHook(script, { pattern: /OnSystemInputEvent/, limit: 7 });
    assert.equal(typeof captured.pattern, "string");
    assert.equal(captured.pattern, "OnSystemInputEvent");
  });

  it("passes options to hook and returns its result", async () => {
    const options = { pattern: "Task", limit: 2 };
    const result = [{ module: "discord.node" }];
    const script = /** @type {import("../../src/adapters/frida.mjs").AgentScript} */ (/** @type {unknown} */ ({ exports: { hook: async (/** @type {object} */ received) => {
      assert.deepEqual(received, { pattern: "Task", limit: 2 });
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
