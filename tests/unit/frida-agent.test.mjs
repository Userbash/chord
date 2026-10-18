// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { runInNewContext } from "node:vm";

const agentUrl = new URL("../../scripts/frida-log-calls.js", import.meta.url);

/** @param {string} value */
function pointer(value) {
  return {
    toString: () => value,
    /** @param {{ toString(): string }} other */
    sub(other) {
      return pointer(`0x${(Number.parseInt(value, 16) - Number.parseInt(other.toString(), 16)).toString(16)}`);
    },
  };
}

/**
 * @param {Record<string, unknown>} globals
 * @returns {Promise<{
 *   inventory: (options?: Record<string, unknown>) => Array<{ name: string, path: string, base: string, size: number, symbols: Array<{ address: string }>, exports: Array<unknown>, error: string | null }>,
 *   hook: (options?: Record<string, unknown>) => { hooked: number, targets: number },
 *   unhook: () => number
 * }>}
 */
async function loadAgent(globals) {
  const source = await readFile(agentUrl, "utf8");
  /** @type {{ rpc: { exports: unknown } }} */
  const context = { ...globals, rpc: { exports: {} } };
  runInNewContext(source, context);
  return /** @type {Awaited<ReturnType<typeof loadAgent>>} */ (context.rpc.exports);
}

describe("scripts/frida-log-calls.js", () => {
  it("inventory переживает отсутствующий модуль и разрешает функции доступного модуля", async () => {
    const base = pointer("0x1000");
    const functionA = { name: "f1", type: "function", address: pointer("0x1010") };
    const functionB = { name: "f2", type: "function", address: pointer("0x1020") };
    const module = {
      name: "ok.node",
      path: "/tmp/ok.node",
      base,
      size: 0x100,
      enumerateSymbols: () => [functionA, functionB],
      enumerateExports: () => [{ name: "exported", type: "function", address: pointer("0x1030") }],
    };
    const processFake = {
      enumerateModules: () => [module],
      /** @param {string} name */
      getModuleByName(name) {
        if (name === "bad.node") throw new Error("module missing");
        if (name === "ok.node") return module;
        throw new Error(`unexpected module: ${name}`);
      },
    };
    const interceptorFake = { attach: () => ({ detach() {} }) };
    const agent = await loadAgent({
      Process: /** @type {unknown} */ (processFake),
      Interceptor: /** @type {unknown} */ (interceptorFake),
      send() {},
    });

    const entries = agent.inventory({ modules: ["bad.node", "ok.node"], maxSymbols: 10 });
    assert.equal(entries.length, 2);
    assert.equal(entries[0]?.name, "bad.node");
    assert.equal(entries[0]?.path, "");
    assert.equal(entries[0]?.base, "0");
    assert.equal(entries[0]?.size, 0);
    assert.equal(entries[0]?.symbols.length, 0);
    assert.equal(entries[0]?.exports.length, 0);
    assert.equal(entries[0]?.error, "Error: module missing");
    assert.equal(entries[1]?.symbols.length, 2);
    assert.equal(entries[1]?.exports.length, 1);
    assert.deepEqual(entries[1]?.symbols.map((entry) => entry.address), ["0x1010", "0x1020"]);
    assert.equal(agent.hook({ modules: ["ok.node"], pattern: "f1", limit: 10 }).targets, 1);
  });

  it("повторный hook снимает предыдущие перехватчики и оставляет только новые", async () => {
    let detachCalls = 0;
    const module = {
      name: "m",
      path: "/tmp/m.node",
      base: pointer("0x1000"),
      size: 0x100,
      enumerateSymbols: () => ["f1", "f2", "f3"].map((name, index) => ({
        name, type: "function", address: pointer(`0x${(0x1010 + index * 0x10).toString(16)}`),
      })),
      enumerateExports: () => [],
    };
    const processFake = { enumerateModules: () => [module], getModuleByName: () => module };
    const interceptorFake = { attach: () => ({ detach: () => { detachCalls += 1; } }) };
    const agent = await loadAgent({
      Process: /** @type {unknown} */ (processFake),
      Interceptor: /** @type {unknown} */ (interceptorFake),
      send() {},
    });

    agent.hook({ modules: ["m"], pattern: "f", limit: 10 });
    const second = agent.hook({ modules: ["m"], pattern: "f", limit: 10 });
    assert.equal(second.hooked, 3);
    assert.equal(detachCalls, 3);
  });

  it("пишет stack только при backtrace и форматирует модульные и внешние адреса", async () => {
    const addrA = pointer("0x102a");
    const addrB = pointer("0x9000");
    const module = {
      name: "m",
      path: "/tmp/m.node",
      base: pointer("0x1000"),
      size: 0x100,
      enumerateSymbols: () => [{ name: "f", type: "function", address: pointer("0x1010") }],
      enumerateExports: () => [],
    };
    /** @type {Array<(this: { threadId: number, context: unknown }, args: unknown[]) => void>} */
    const handlers = [];
    const interceptorFake = {
      /**
       * @param {unknown} _address
       * @param {{ onEnter: (this: { threadId: number, context: unknown }, args: unknown[]) => void }} callbacks
       */
      attach(_address, callbacks) {
        handlers.push(callbacks.onEnter);
        return { detach() {} };
      },
    };
    const processFake = {
      enumerateModules: () => [module],
      getModuleByName: () => module,
      findModuleByAddress: /** @type {(address: { toString(): string }) => object | null} */ ((address) => address.toString() === addrA.toString() ? module : null),
    };
    /** @type {Record<string, unknown>[]} */
    const messages = [];
    /** @type {Record<string, unknown>} */
    const globals = {
      Process: /** @type {unknown} */ (processFake),
      Interceptor: /** @type {unknown} */ (interceptorFake),
      Thread: /** @type {unknown} */ ({ backtrace: () => [addrA, addrB] }),
      Backtracer: /** @type {unknown} */ ({ FUZZY: 1 }),
      send: /** @type {(message: Record<string, unknown>) => void} */ ((message) => { messages.push(message); }),
    };
    const agent = await loadAgent(globals);
    const invoke = /** @type {(handler: ((this: { threadId: number, context: unknown }, args: unknown[]) => void) | undefined) => void} */ ((handler) => {
      assert.ok(handler);
      handler.call({ threadId: 7, context: {} }, [pointer("0x1")]);
    });

    agent.hook({ modules: ["m"], pattern: "f", limit: 10 });
    invoke(handlers[0]);
    const first = messages[0];
    assert.ok(first);
    assert.equal("stack" in first, false);
    assert.equal(Array.isArray(messages[0]?.args), true);
    assert.equal(messages[0]?.threadId, 7);
    assert.equal(typeof messages[0]?.at, "number");

    agent.hook({ modules: ["m"], pattern: "f", limit: 10, backtrace: true });
    invoke(handlers[1]);
    assert.deepEqual(messages[1]?.stack, ["m+0x2a", addrB.toString()]);
  });
});
