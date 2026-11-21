// @ts-nocheck
import test from "node:test";
import assert from "node:assert/strict";
import {
  invoke,
  createSignalCommand,
  bindGatewayEvent,
  createStateRouter,
} from "../../rust/gui-spike/ui/features/auto-ipc-bridge.js";

function withMockTauri({ invokeImpl, listenImpl } = {}) {
  const prevWindow = globalThis.window;
  globalThis.window = {
    __TAURI__: {
      core: {
        invoke: invokeImpl ?? (async () => null),
      },
      event: {
        listen: listenImpl ?? (async () => () => {}),
      },
    },
  };
  return () => {
    if (prevWindow === undefined) {
      delete globalThis.window;
    } else {
      globalThis.window = prevWindow;
    }
  };
}

test("invoke delegates to window.__TAURI__.core.invoke when present", async () => {
  const calls = [];
  const restore = withMockTauri({
    invokeImpl: async (cmd, args) => {
      calls.push({ cmd, args });
      return { ok: true };
    },
  });
  try {
    const res = await invoke("net_status", { verbose: true });
    assert.deepEqual(res, { ok: true });
    assert.deepEqual(calls, [{ cmd: "net_status", args: { verbose: true } }]);
  } finally {
    restore();
  }
});

test("createSignalCommand syncs Preact signal with Tauri command and tracks loading/error", async () => {
  let shouldFail = false;
  const restore = withMockTauri({
    invokeImpl: async (cmd, args) => {
      if (shouldFail) throw new Error("ipc failed");
      return { cmd, count: (args?.offset ?? 0) + 1 };
    },
  });

  try {
    const status = createSignalCommand("net_status", null);
    assert.equal(status.data.value, null);
    assert.equal(status.loading.value, false);
    assert.equal(status.error.value, null);

    const result = await status.refresh({ offset: 4 });
    assert.deepEqual(result, { cmd: "net_status", count: 5 });
    assert.deepEqual(status.data.value, { cmd: "net_status", count: 5 });
    assert.equal(status.loading.value, false);
    assert.equal(status.error.value, null);

    shouldFail = true;
    await assert.rejects(() => status.refresh(), /ipc failed/);
    assert.equal(status.loading.value, false);
    assert.equal(status.error.value?.message, "ipc failed");
  } finally {
    restore();
  }
});

test("bindGatewayEvent updates target signal on Tauri event and supports unlisten", async () => {
  const listeners = new Map();
  let unlistened = false;
  const restore = withMockTauri({
    listenImpl: async (eventName, cb) => {
      listeners.set(eventName, cb);
      return () => {
        unlistened = true;
        listeners.delete(eventName);
      };
    },
  });

  try {
    const { data } = createSignalCommand("channels", []);
    const unlisten = await bindGatewayEvent(
      "gateway:CHANNEL_CREATE",
      data,
      (prev, payload) => [...prev, payload],
    );

    const handler = listeners.get("gateway:CHANNEL_CREATE");
    assert.equal(typeof handler, "function");

    handler({ payload: { id: "c1", name: "general" } });
    assert.deepEqual(data.value, [{ id: "c1", name: "general" }]);

    unlisten();
    assert.equal(unlistened, true);
    assert.equal(listeners.has("gateway:CHANNEL_CREATE"), false);
  } finally {
    restore();
  }
});

test("createStateRouter wires commands and gateway events declaratively", async () => {
  const listeners = new Map();
  const restore = withMockTauri({
    invokeImpl: async (cmd) => {
      if (cmd === "net_status") return { connected: true };
      return null;
    },
    listenImpl: async (eventName, cb) => {
      listeners.set(eventName, cb);
      return () => listeners.delete(eventName);
    },
  });

  try {
    const router = createStateRouter({
      net: {
        command: "net_status",
        initialValue: { connected: false },
        event: "gateway:READY",
        reducer: (_prev, payload) => ({ connected: Boolean(payload?.connected) }),
      },
    });

    assert.deepEqual(router.signals.net.data.value, { connected: false });

    await router.start({ autoFetch: true });
    assert.deepEqual(router.signals.net.data.value, { connected: true });

    listeners.get("gateway:READY")({ payload: { connected: false } });
    assert.deepEqual(router.signals.net.data.value, { connected: false });

    await router.dispatch("net");
    assert.deepEqual(router.signals.net.data.value, { connected: true });

    router.stop();
    assert.equal(listeners.has("gateway:READY"), false);
  } finally {
    restore();
  }
});
