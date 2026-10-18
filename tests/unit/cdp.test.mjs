// @ts-check
// RED (M1): реализация — src/core/cdp/client.mjs. Контракт: CDP-клиент с
// инжектируемым WebSocket; unit-тесты не открывают реальных соединений.
import { afterEach, beforeEach, describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  createCdpClient,
  evaluate,
  listTargets,
  pickTarget,
  resolveEndpoint,
  resolveTargetHint,
} from "../../src/core/cdp/client.mjs";
import { FakeWebSocket } from "../helpers/fake-websocket.mjs";

/** @typedef {import("../../src/core/types.js").CdpClient} CdpClient */
/** @typedef {import("../../src/core/types.js").FetchLike} FetchLike */

describe("resolveEndpoint / resolveTargetHint", () => {
  it("значения по умолчанию", () => {
    assert.equal(resolveEndpoint({}), "http://127.0.0.1:9222");
    assert.equal(resolveTargetHint({}), "discord.com");
  });

  it("переменные окружения переопределяют значения", () => {
    assert.equal(resolveEndpoint({ CDP_ENDPOINT: "http://127.0.0.1:9223" }), "http://127.0.0.1:9223");
    assert.equal(resolveTargetHint({ CDP_TARGET: "index.html" }), "index.html");
  });

  it("без аргумента читает process.env", () => {
    assert.equal(typeof resolveEndpoint(), "string");
    assert.equal(typeof resolveTargetHint(), "string");
  });
});

describe("listTargets", () => {
  it("запрашивает <endpoint>/json/list и возвращает список", async () => {
    /** @type {string[]} */
    const urls = [];
    /** @type {FetchLike} */
    const fetchImpl = async (url) => {
      urls.push(url);
      return { ok: true, status: 200, json: async () => [{ type: "page", url: "https://discord.com" }] };
    };
    const targets = await listTargets("http://127.0.0.1:9222", fetchImpl);
    assert.deepEqual(urls, ["http://127.0.0.1:9222/json/list"]);
    assert.equal(targets.length, 1);
    assert.equal(targets[0]?.url, "https://discord.com");
  });

  it("ошибка HTTP → CDP list failed: <status>", async () => {
    /** @type {FetchLike} */
    const fetchImpl = async () => ({ ok: false, status: 500, json: async () => ({}) });
    await assert.rejects(() => listTargets("http://127.0.0.1:9222", fetchImpl), /CDP list failed: 500/);
  });

  it("нет ответа → таймаут", async () => {
    /** @type {FetchLike} */
    const fetchImpl = (_url, init) => new Promise((_resolve, reject) => {
      const fallback = setTimeout(() => reject(new Error("fetch did not receive an abort signal")), 100);
      init?.signal?.addEventListener("abort", () => {
        clearTimeout(fallback);
        reject(new DOMException("The operation was aborted", "TimeoutError"));
      }, { once: true });
    });
    await assert.rejects(
      () => listTargets("http://127.0.0.1:9222", fetchImpl, { timeoutMs: 25 }),
      /CDP list failed: таймаут 25 мс/,
    );
  });

  it("ответ не массив", async () => {
    /** @type {FetchLike} */
    const fetchImpl = async () => ({ ok: true, status: 200, json: async () => ({}) });
    await assert.rejects(
      () => listTargets("http://127.0.0.1:9222", fetchImpl),
      /CDP list failed: ответ не является массивом/,
    );
  });
});

describe("pickTarget", () => {
  const targets = [
    { type: "page", url: "https://canary.discord.com/channels/@me", title: "canary", webSocketDebuggerUrl: "ws://canary" },
    { type: "page", url: "devtools://devtools/bundled/inspector.html", title: "devtools", webSocketDebuggerUrl: "ws://devtools" },
    { type: "page", url: "about:blank", title: "blank", webSocketDebuggerUrl: "ws://blank" },
    { type: "service_worker", url: "https://discord.com/sw.js", webSocketDebuggerUrl: "ws://sw" },
  ];

  it("ищет страницу по hint", () => {
    assert.equal(pickTarget(targets, "discord.com")?.webSocketDebuggerUrl, "ws://canary");
  });

  it("без совпадения по hint — первая страница с 'discord' в url", () => {
    assert.equal(pickTarget(targets, "index.html")?.webSocketDebuggerUrl, "ws://canary");
  });

  it("страницы без webSocketDebuggerUrl пропускаются; fallback — первая страница", () => {
    const list = [
      { type: "page", url: "no-ws", title: "x" },
      { type: "page", url: "file:///app/index.html", title: "fixture", webSocketDebuggerUrl: "ws://fixture" },
    ];
    assert.equal(pickTarget(list, "index.html")?.webSocketDebuggerUrl, "ws://fixture");
    assert.equal(pickTarget(list, "nothing-matches")?.webSocketDebuggerUrl, "ws://fixture");
  });

  it("подходящих страниц нет → undefined", () => {
    assert.equal(pickTarget([{ type: "service_worker", url: "x", webSocketDebuggerUrl: "ws://x" }]), undefined);
    assert.equal(pickTarget([]), undefined);
  });
});

describe("createCdpClient", () => {
  beforeEach(() => {
    FakeWebSocket.instances.length = 0;
    FakeWebSocket.autoOpen = true;
  });

  afterEach(() => {
    FakeWebSocket.instances.length = 0;
    FakeWebSocket.autoOpen = true;
  });

  it("отправляет {id, method, params} и resolve-ит result", async () => {
    const client = await createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket });
    const promise = client.send("Page.enable", {});
    const ws = FakeWebSocket.latest();
    assert.deepEqual(ws.sentMessages(), [{ id: 1, method: "Page.enable", params: {} }]);
    ws.receive({ id: 1, result: { ok: true } });
    assert.deepEqual(await promise, { ok: true });
    client.close();
  });

  it("ответ с ошибкой → reject с методом в сообщении", async () => {
    const client = await createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket });
    const promise = client.send("Runtime.evaluate", { expression: "x" });
    FakeWebSocket.latest().receive({ id: 1, error: { code: -32000, message: "boom" } });
    await assert.rejects(() => promise, { message: "Runtime.evaluate: boom" });
    client.close();
  });

  it("события без id копятся в events; чужие id игнорируются", async () => {
    const client = await createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket });
    const ws = FakeWebSocket.latest();
    ws.receive({ method: "Runtime.consoleAPICalled", params: { type: "error" } });
    ws.receive({ id: 999, result: {} });
    assert.equal(client.events.length, 1);
    assert.equal(client.events[0]?.method, "Runtime.consoleAPICalled");
    assert.equal(typeof client.events[0]?.at, "number");
    client.close();
  });

  it("mark/eventsSince/drainEvents работают с окном событий", async () => {
    const client = /** @type {any} */ (await createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket }));
    const ws = FakeWebSocket.latest();
    ws.receive({ method: "Runtime.first", params: { n: 1 } });
    ws.receive({ method: "Runtime.second", params: { n: 2 } });
    assert.equal(client.mark(), 2);
    ws.receive({ method: "Runtime.third", params: { n: 3 } });
    assert.deepEqual(client.eventsSince(2).map((/** @type {{ method: string }} */ event) => event.method), ["Runtime.third"]);
    assert.deepEqual(client.drainEvents().map((/** @type {{ method: string }} */ event) => event.method), ["Runtime.first", "Runtime.second", "Runtime.third"]);
    assert.equal(client.events.length, 0);
    assert.deepEqual(client.eventsSince(0), []);
    ws.receive({ method: "Runtime.afterDrain" });
    assert.equal(client.events.length, 1);
    client.close();
  });

  it("битый JSON не роняет клиента", async () => {
    const client = await createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket });
    const ws = FakeWebSocket.latest();
    assert.doesNotThrow(() => ws.receiveRaw("{не json"));
    const promise = client.send("X");
    ws.receive({ id: 1, result: 1 });
    assert.equal(await promise, 1);
    client.close();
  });

  it("таймаут ответа → reject '<method>: таймаут <ms> мс'", async () => {
    const client = await createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket, timeoutMs: 30 });
    await assert.rejects(() => client.send("Slow.method"), { message: "Slow.method: таймаут 30 мс" });
    client.close();
  });

  it("close() закрывает сокет и отклоняет незавершённые запросы", async () => {
    const client = await createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket });
    const promise = client.send("Pending.method");
    client.close();
    await assert.rejects(() => promise, /Pending\.method: CDP соединение закрыто/);
  });

  it("ошибка соединения до open → createCdpClient reject-ит", async () => {
    FakeWebSocket.autoOpen = false;
    const clientPromise = createCdpClient({ wsUrl: "ws://test", WebSocket: FakeWebSocket });
    FakeWebSocket.latest().fail();
    await assert.rejects(() => clientPromise, /WebSocket error/);
  });

  it("синхронный throw send отклоняет запрос и закрытый клиент отвергает повторный send", async () => {
    class ThrowingWebSocket extends FakeWebSocket {
      send() {
        throw new Error("ws не готов");
      }
    }
    const client = await createCdpClient({ wsUrl: "ws://test", WebSocket: ThrowingWebSocket });
    await assert.rejects(() => client.send("X"), /ws не готов/);
    assert.doesNotThrow(() => client.close());
    await assert.rejects(() => client.send("X"), /CDP соединение закрыто/);
  });
});

describe("evaluate", () => {
  it("передаёт Runtime.evaluate с returnByValue/awaitPromise и возвращает value", async () => {
    /** @type {Array<{ method: string, params: Record<string, unknown> }>} */
    const calls = [];
    /** @type {CdpClient} */
    const cdp = {
      send: async (method, params = {}) => {
        calls.push({ method, params });
        return { result: { value: 42 } };
      },
      close: () => {},
    };
    assert.equal(await evaluate(cdp, "6*7"), 42);
    assert.equal(calls[0]?.method, "Runtime.evaluate");
    assert.equal(calls[0]?.params.expression, "6*7");
    assert.equal(calls[0]?.params.returnByValue, true);
    assert.equal(calls[0]?.params.awaitPromise, true);
  });

  it("исключение в странице → Error с description", async () => {
    /** @type {CdpClient} */
    const cdp = {
      send: async () => ({
        exceptionDetails: { exception: { description: "ReferenceError: x is not defined" }, text: "Uncaught" },
      }),
      close: () => {},
    };
    await assert.rejects(() => evaluate(cdp, "x"), /ReferenceError: x is not defined/);
  });

  it("если description нет — используется text", async () => {
    /** @type {CdpClient} */
    const cdp = {
      send: async () => ({ exceptionDetails: { text: "Uncaught" } }),
      close: () => {},
    };
    await assert.rejects(() => evaluate(cdp, "x"), /Uncaught/);
  });
});
