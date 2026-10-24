// @ts-check
// Подмена глобальных fetch/WebSocket для unit-тестов CLI-команд cdp:
// реальный CDP-эндпоинт и сеть не используются.

import { FakeWebSocket } from "./fake-websocket.mjs";

/**
 * Ответчик CDP-метода. Третий аргумент — сокет: позволяет инжектировать
 * события страницы (`Runtime.consoleAPICalled`/`exceptionThrown`) в тестах walk.
 * @typedef {(method: string, params: Record<string, unknown>, socket: any) => any} CdpResponder
 */

/**
 * @typedef {object} FakeCdpOptions
 * @property {any[]} [targets] ответ `<endpoint>/json/list` (по умолчанию одна fixture-страница)
 * @property {CdpResponder} [respond] ответ на CDP-метод
 * @property {number} [listStatus] HTTP-статус списка целей (не 200 → ошибка)
 * @property {Error} [listError] ошибка fetch списка целей
 */

/**
 * Устанавливает фейковые fetch и WebSocket на время теста.
 * @param {FakeCdpOptions} [options]
 * @returns {{
 *   calls: Array<{ method: string, params: Record<string, unknown> }>,
 *   urls: string[],
 *   sockets: () => FakeWebSocket[],
 *   restore: () => void,
 * }}
 */
export function installFakeCdp(options = {}) {
  /** @type {Array<{ method: string, params: Record<string, unknown> }>} */
  const calls = [];
  /** @type {string[]} */
  const urls = [];
  /** @type {FakeWebSocket[]} */
  const sockets = [];
  const targets = options.targets ?? [
    { type: "page", url: "file:///app/index.html", title: "fixture", webSocketDebuggerUrl: "ws://fixture" },
  ];
  const globalAny = /** @type {any} */ (globalThis);
  const previousFetch = globalAny.fetch;
  const previousWebSocket = globalAny.WebSocket;

  globalAny.fetch = async (/** @type {string} */ url) => {
    urls.push(String(url));
    if (options.listError) throw options.listError;
    const status = options.listStatus ?? 200;
    if (status !== 200) return { ok: false, status, json: async () => ({}) };
    return { ok: true, status, json: async () => targets };
  };

  class StubWebSocket extends FakeWebSocket {
    /** @param {string} data */
    send(data) {
      super.send(data);
      sockets.push(this);
      const frame = JSON.parse(String(data));
      calls.push({ method: frame.method, params: frame.params });
      const respond = options.respond ?? (() => ({}));
      queueMicrotask(() => {
        try {
          this.receive({ id: frame.id, result: respond(frame.method, frame.params, this) });
        } catch (error) {
          this.receive({ id: frame.id, error: { message: error instanceof Error ? error.message : String(error) } });
        }
      });
    }
  }

  globalAny.WebSocket = StubWebSocket;

  return {
    calls,
    urls,
    sockets: () => sockets,
    restore() {
      globalAny.fetch = previousFetch;
      globalAny.WebSocket = previousWebSocket;
    },
  };
}
