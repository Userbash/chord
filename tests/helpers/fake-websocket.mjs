// @ts-check
// Эмуляция WebSocket (API Node: addEventListener/send/close) для unit-тестов CDP-клиента.

/** @typedef {(event: any) => void} WsListener */

export class FakeWebSocket {
  /** Все созданные экземпляры (для доступа из тестов). @type {FakeWebSocket[]} */
  static instances = [];

  /** Автоматически «открываться» микротаском после создания. @type {boolean} */
  static autoOpen = true;

  /** @param {string} url */
  constructor(url) {
    /** @type {string} */
    this.url = url;
    /** @type {number} */
    this.readyState = 0; // CONNECTING
    /** Отправленные клиентом кадры (JSON-строки). @type {string[]} */
    this.sent = [];
    /** @type {Map<string, Set<WsListener>>} */
    this.listeners = new Map();
    FakeWebSocket.instances.push(this);
    queueMicrotask(() => {
      if (FakeWebSocket.autoOpen && this.readyState === 0) this.open();
    });
  }

  /** Последний созданный экземпляр. @returns {FakeWebSocket} */
  static latest() {
    const ws = FakeWebSocket.instances.at(-1);
    if (!ws) throw new Error("FakeWebSocket: ни одного экземпляра");
    return ws;
  }

  /**
   * @param {string} type
   * @param {WsListener} listener
   * @param {{ once?: boolean }} [options]
   */
  addEventListener(type, listener, options = {}) {
    /** @type {Set<WsListener>} */
    const set = this.listeners.get(type) ?? new Set();
    if (options.once) {
      /** @type {WsListener} */
      const wrapped = (event) => {
        set.delete(wrapped);
        listener(event);
      };
      set.add(wrapped);
    } else {
      set.add(listener);
    }
    this.listeners.set(type, set);
  }

  /** @param {string} type @param {any} event */
  dispatch(type, event) {
    const set = this.listeners.get(type);
    if (!set) return;
    for (const listener of [...set]) listener(event);
  }

  open() {
    this.readyState = 1;
    this.dispatch("open", {});
  }

  /** Сообщение «от сервера»: объект сериализуется в JSON. @param {unknown} message */
  receive(message) {
    this.dispatch("message", { data: JSON.stringify(message) });
  }

  /** Сырое сообщение (например, битый JSON). @param {string} data */
  receiveRaw(data) {
    this.dispatch("message", { data });
  }

  fail() {
    this.dispatch("error", {});
  }

  /** @param {string} data */
  send(data) {
    this.sent.push(String(data));
  }

  close() {
    this.readyState = 3; // CLOSED
    this.dispatch("close", {});
  }

  /** Отправленные кадры, разобранные как объекты. @returns {any[]} */
  sentMessages() {
    return this.sent.map((line) => JSON.parse(line));
  }
}
