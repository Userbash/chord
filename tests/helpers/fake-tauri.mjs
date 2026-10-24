// @ts-check
/**
 * Детерминированный fake `window.__TAURI__` для browser-fixture сценариев M2.
 *
 * Контракт (Tauri 2):
 * - `__TAURI__.core.invoke(cmd, args)` → Promise; неизвестная команда **реджектится**
 *   (`{kind:"unknownCommand"}`), а не отдаёт `undefined` — иначе продуктовая ошибка
 *   «команды нет» маскируется пустым успехом;
 * - `__TAURI__.event.listen(name, handler)` → Promise<unlisten>; `__QA__.emit()` —
 *   доставка события в тесте (`{event, id, payload}`, как у Tauri v2).
 *
 * Детерминизм: никаких таймеров и сети. Ответы задаются декларативной спекой:
 * - `{ ok: value }`          — resolve `value`;
 * - `{ err: value }`         — reject `value` (включая `{kind:"forbidden"}`, как SearchError);
 * - `{ seq: [entry, ...] }`  — по одному ответу на вызов, последний повторяется;
 * - `{ byArg: { key, map, fallback } }` — выбор ответа по `args[key]`
 *   (нужно для `channels({guildId})` и `messages_page({channelId})`).
 *
 * Всё состояние живёт на странице: журнал `__QA__.calls` (не payload-секреты),
 * настройки — в `localStorage` (переживают `Page.reload`), аудио — счётчики
 * `__QA__.audio` с блокировкой захвата (сборка без голосового сервиса не должна
 * звать `getUserMedia`).
 *
 * Одна и та же функция исполняется и на странице (инъекция в `index.html`), и в
 * Node-тесте (`vm`), поэтому контракт транспорта проверяется без браузера.
 */

/**
 * @typedef {object} FakeTauriQa
 * @property {Array<{ cmd: string, args: any, at: number }>} calls
 * @property {string[]} consoleErrors
 * @property {string[]} consoleWarns
 * @property {{ getUserMedia: number, audioContext: number, audioPlay: number }} audio
 * @property {number} version
 * @property {(cmd: string) => Array<{ cmd: string, args: any, at: number }>} callsOf
 * @property {(cmd: string) => number} count
 * @property {(cmd: string, entry: any) => void} set
 * @property {() => void} reset
 * @property {(name: string, payload: any) => number} emit
 * @property {() => any} settings
 * @property {() => void} clearSettings
 */

/** Версия контракта: меняется вместе с формой `__QA__` (мета-тест сверяет поле). */
export const FAKE_TAURI_VERSION = 1;

/**
 * Устанавливает fake `__TAURI__` и `__QA__` в переданный `globalThis`.
 * ВАЖНО: функция должна быть самодостаточной — она сериализуется в страницу
 * через `toString()`, поэтому не использует импорты и замыкания модуля.
 * @param {Record<string, any>} [spec]
 * @returns {FakeTauriQa}
 */
export function installFakeTauri(spec) {
  "use strict";
  const scope = /** @type {any} */ (globalThis);
  const win = scope.window && typeof scope.window === "object" ? scope.window : scope;
  const SETTINGS_KEY = "__qa_settings_v1__";
  const started = Date.now();

  /**
   * JSON-копия: тест и продукт не должны делить одни и те же объекты.
   * @param {any} value
   * @returns {any}
   */
  function clone(value) {
    if (value === undefined) return null;
    try {
      return JSON.parse(JSON.stringify(value));
    } catch {
      return value;
    }
  }

  /** @type {Record<string, any>} */
  const state = clone(spec ?? {}) ?? {};

  /** @returns {any} */
  function defaultSettings() {
    return { version: 1, toggles: {}, density: "cozy", theme: "dark", locale: "ru" };
  }

  /** @returns {any} */
  function readSettings() {
    try {
      const raw = scope.localStorage ? scope.localStorage.getItem(SETTINGS_KEY) : null;
      if (typeof raw !== "string" || raw === "") return defaultSettings();
      const parsed = JSON.parse(raw);
      if (parsed === null || typeof parsed !== "object") return defaultSettings();
      return parsed;
    } catch {
      return defaultSettings();
    }
  }

  /** @param {any} settings @returns {boolean} */
  function writeSettings(settings) {
    try {
      if (!scope.localStorage) return false;
      scope.localStorage.setItem(SETTINGS_KEY, JSON.stringify(clone(settings) ?? {}));
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Дефолтные ответы: только безопасные, без токенов/сети.
   * @param {string} cmd
   * @param {any} args
   * @returns {any}
   */
  function defaultEntry(cmd, args) {
    const bag = args !== null && typeof args === "object" ? args : {};
    switch (cmd) {
      case "spike_config":
        return { ok: { autobench: false } };
      case "process_metrics":
        return { ok: { rss_kib: 184320, pss_kib: 131072, threads: 7, cpu_pct: 3.5 } };
      case "report_ui_snapshot":
      case "report_bench":
      case "window_ready":
      case "ack_message":
        return { ok: null };
      case "settings_load":
        return { ok: readSettings() };
      case "settings_save":
        writeSettings(bag.settings);
        return { ok: null };
      case "net_status":
        return { ok: "ready" };
      case "net_connect":
        return { ok: "ready" };
      case "read_state":
        return { ok: [] };
      case "send_message":
        return {
          ok: {
            id: "900000000000000001",
            channel_id: String(bag.channelId ?? ""),
            content: String(bag.content ?? ""),
            nonce: bag.nonce ?? null,
          },
        };
      default:
        return { err: { kind: "unknownCommand", cmd } };
    }
  }

  /**
   * @param {any} entry
   * @param {any} args
   * @returns {any}
   */
  function resolveEntry(entry, args) {
    if (entry === null || typeof entry !== "object") return entry;
    if (Array.isArray(entry.seq)) {
      const index = Number.isInteger(entry.index) ? entry.index : 0;
      entry.index = Math.min(index + 1, entry.seq.length - 1);
      return resolveEntry(entry.seq[Math.min(index, entry.seq.length - 1)], args);
    }
    if (entry.byArg && typeof entry.byArg === "object") {
      const key = String(entry.byArg.key ?? "");
      const bag = args !== null && typeof args === "object" ? args : {};
      const map = entry.byArg.map && typeof entry.byArg.map === "object" ? entry.byArg.map : {};
      const next = Object.prototype.hasOwnProperty.call(map, String(bag[key] ?? ""))
        ? map[String(bag[key] ?? "")]
        : entry.byArg.fallback;
      return resolveEntry(next, args);
    }
    return entry;
  }

  /** @type {Map<string, Array<(event: any) => void>>} */
  const listeners = new Map();

  /** @type {FakeTauriQa} */
  const qa = {
    calls: [],
    consoleErrors: [],
    consoleWarns: [],
    audio: { getUserMedia: 0, audioContext: 0, audioPlay: 0 },
    version: 1,
    callsOf(cmd) {
      return qa.calls.filter((call) => call.cmd === cmd);
    },
    count(cmd) {
      return qa.callsOf(cmd).length;
    },
    set(cmd, entry) {
      state[String(cmd)] = clone(entry);
    },
    reset() {
      qa.calls.length = 0;
      qa.consoleErrors.length = 0;
      qa.consoleWarns.length = 0;
    },
    emit(name, payload) {
      const bucket = listeners.get(String(name)) ?? [];
      let delivered = 0;
      for (const handler of [...bucket]) {
        handler({ event: String(name), id: 1, payload: clone(payload) });
        delivered += 1;
      }
      return delivered;
    },
    settings: readSettings,
    clearSettings() {
      try {
        if (scope.localStorage) scope.localStorage.removeItem(SETTINGS_KEY);
      } catch {
        // private mode / нет storage: состояние и так недоступно
      }
    },
  };

  /**
   * @param {string} cmd
   * @param {any} [args]
   * @returns {Promise<any>}
   */
  function invoke(cmd, args) {
    qa.calls.push({
      cmd: String(cmd),
      args: clone(args === undefined ? null : args),
      at: Date.now() - started,
    });
    const declared = Object.prototype.hasOwnProperty.call(state, String(cmd))
      ? state[String(cmd)]
      : defaultEntry(String(cmd), args);
    const entry = resolveEntry(declared, args);
    return Promise.resolve().then(() => {
      if (entry !== null && typeof entry === "object" && Object.prototype.hasOwnProperty.call(entry, "err")) {
        throw entry.err;
      }
      if (entry !== null && typeof entry === "object" && Object.prototype.hasOwnProperty.call(entry, "ok")) {
        return entry.ok;
      }
      return null;
    });
  }

  const tauri = {
    core: { invoke },
    event: {
      /**
       * @param {string} name
       * @param {(event: any) => void} handler
       * @returns {Promise<() => void>}
       */
      listen: async (name, handler) => {
        const key = String(name);
        const bucket = listeners.get(key) ?? [];
        bucket.push(handler);
        listeners.set(key, bucket);
        return () => {
          const current = listeners.get(key) ?? [];
          listeners.set(key, current.filter((item) => item !== handler));
        };
      },
      /** @param {string} name @param {(event: any) => void} handler */
      once: async (name, handler) => {
        const unlisten = await tauri.event.listen(name, (event) => {
          unlisten();
          handler(event);
        });
        return unlisten;
      },
      /** @param {string} name @param {any} [payload] */
      emit: (name, payload) => Promise.resolve(qa.emit(name, payload)),
    },
  };

  /* ---------- инструментация окружения ---------- */

  /**
   * @param {...any} args
   * @returns {void}
   */
  function errorProxy(...args) {
    qa.consoleErrors.push(args.map((value) => String(value)).join(" "));
    if (originalError) originalError.apply(scope.console, args);
  }
  /**
   * @param {...any} args
   * @returns {void}
   */
  function warnProxy(...args) {
    qa.consoleWarns.push(args.map((value) => String(value)).join(" "));
    if (originalWarn) originalWarn.apply(scope.console, args);
  }
  const originalError = scope.console && typeof scope.console.error === "function" ? scope.console.error : null;
  /** @type {((...args: any[]) => void) | null} */
  let originalWarn = null;
  if (scope.console) {
    scope.console.error = errorProxy;
    originalWarn = typeof scope.console.warn === "function" ? scope.console.warn : null;
    scope.console.warn = warnProxy;
  }

  // Headless-страница иногда считается скрытой (`document.hidden`), а polling
  // `net_status` в продукте в этом случае не тикает. Для fixture-сценария
  // фиксируем «видимо»: иначе M2-06 проверял бы не продукт, а режим headless.
  const doc = scope.document;
  if (doc) {
    try {
      Object.defineProperty(doc, "hidden", { configurable: true, get: () => false });
      Object.defineProperty(doc, "visibilityState", { configurable: true, get: () => "visible" });
    } catch {
      // Уже неконфигурируемо — сценарий сам проверит видимость баннера по таймауту.
    }
  }

  // Голос: сборка без сервиса не должна захватывать устройства. Счётчик + запрет.
  const navigatorLike = scope.navigator;
  if (navigatorLike && navigatorLike.mediaDevices && typeof navigatorLike.mediaDevices.getUserMedia === "function") {
    /**
     * @param {...any} _args
     * @returns {Promise<never>}
     */
    function blockedGetUserMedia(..._args) {
      qa.audio.getUserMedia += 1;
      return Promise.reject(new Error("QA_AUDIO_BLOCKED: getUserMedia запрещён в fixture-сценарии"));
    }
    navigatorLike.mediaDevices.getUserMedia = blockedGetUserMedia;
  }
  for (const name of ["AudioContext", "webkitAudioContext"]) {
    const Original = scope[name];
    if (typeof Original !== "function") continue;
    scope[name] = new Proxy(Original, {
      construct(target, args) {
        qa.audio.audioContext += 1;
        return Reflect.construct(/** @type {any} */ (target), args);
      },
    });
  }
  const media = scope.HTMLMediaElement;
  if (media && media.prototype && typeof media.prototype.play === "function") {
    const play = media.prototype.play;
    /**
     * @this {any}
     * @param {...any} args
     * @returns {any}
     */
    function playWrapped(...args) {
      qa.audio.audioPlay += 1;
      return play.apply(this, args);
    }
    media.prototype.play = playWrapped;
  }

  win.__QA__ = qa;
  win.__TAURI__ = tauri;
  return qa;
}

/**
 * Исходник для инъекции в `index.html` перед `<script type="module" src="app.js">`.
 * @param {Record<string, any>} [spec]
 * @returns {string}
 */
export function bootstrapSource(spec = {}) {
  return `(${installFakeTauri.toString()})(${JSON.stringify(spec)});`;
}
