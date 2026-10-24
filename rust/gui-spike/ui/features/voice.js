// @ts-check
/**
 * V-08 (RW-40): голосовая панель спайка на **фейковом** store.
 *
 * Контракт — `waves/RW-40/DESIGN/VOICE.md` §2/§4 и машинная спека
 * `waves/RW-40/DESIGN/voice-spec.json` (schema `rw40/voice-spec-v1`):
 * - `#voice-panel[data-state]` — 12 состояний, единственный источник стилей;
 * - в сборке без voice-сервиса (`service === null`) панель всегда `disabled`:
 *   вход заблокирован, «подключено» не имитируется (G6-REPORT §3.7);
 * - join/leave/reconnect не оптимистичны: состояние меняется только по событию
 *   store (`VOICE_STATE_UPDATE` в реальном клиенте), а не по клику;
 * - speaking — готовый флаг op5 (`bool` или число > 0), панель его не вычисляет;
 * - наружу панель не строит endpoint/token/ssrc и не вызывает IPC: все действия
 *   идут через инжектированный `service` (в app.js его нет). Медиа-API
 *   (`mediaDevices`, запись, превью экрана) запрещены §5.3.
 *
 * Новых IPC-команд и endpoint'ов карточка не добавляет: fake store живёт в
 * памяти, тесты подменяют `service` записывающей заглушкой.
 */
import { UIError } from "./errors.js";

/**
 * @typedef {"idle" | "loading" | "connecting" | "connected" | "reconnecting" | "empty" | "error" | "no-permission" | "ptt-required" | "device-missing" | "server-mute" | "disabled"} VoiceState
 * @typedef {"idle" | "held" | "blocked"} PttState
 * @typedef {"none" | "self" | "server"} ParticipantMute
 * @typedef {{ id: string, name: string, self: boolean, speaking: boolean | number, mute: ParticipantMute }} VoiceParticipant
 * @typedef {{
 *   state: VoiceState,
 *   guildId: string,
 *   channelId: string,
 *   channelName: string,
 *   participants: readonly VoiceParticipant[],
 *   ping: number | null,
 *   selfMute: boolean,
 *   selfDeaf: boolean,
 *   serverMute: boolean,
 *   ptt: PttState,
 * }} VoiceSnapshot
 * @typedef {{
 *   join: (channelId: string) => Promise<unknown>,
 *   leave: () => Promise<unknown>,
 *   reconnect?: (channelId: string) => Promise<unknown>,
 *   setSelfMute?: (mute: boolean) => Promise<unknown>,
 *   setSelfDeaf?: (deaf: boolean) => Promise<unknown>,
 *   enablePtt?: () => Promise<unknown>,
 * }} VoiceService
 */

/** Порядок заморожен `voice-spec.json` → `dom.states`. @type {readonly VoiceState[]} */
export const VOICE_STATES = Object.freeze(/** @type {const} */ ([
  "idle",
  "loading",
  "connecting",
  "connected",
  "reconnecting",
  "empty",
  "error",
  "no-permission",
  "ptt-required",
  "device-missing",
  "server-mute",
  "disabled",
]));

/** Строки `[D]` из `copy`-таблицы спеки (`disabled` — наш `[A]`-текст). */
/** @type {Record<VoiceState, string>} */
export const VOICE_TEXT = {
  idle: "",
  loading: "",
  connecting: "Подключение...",
  connected: "Голосовая связь подключена",
  reconnecting: "Переподключение",
  empty: "Сейчас в голосовом чате никого нет",
  error: "Не удаётся подключить вас к этому каналу. Повторите попытку позже.",
  "no-permission": "У вас нет права присоединиться к этому голосовому каналу.",
  "ptt-required": "Требуется режим рации",
  "device-missing": "Discord не может обнаружить звук с вашего микрофона.",
  "server-mute": "У вас откл. микрофон на сервере",
  disabled: "Голосовой канал недоступен в этой сборке",
};

/** Вторая строка состояний (`[D]`): почему и что делать. */
const VOICE_HINT = Object.freeze({
  "device-missing": "Убедитесь, что вы выбрали правильное устройство ввода.",
  "ptt-required": "Чтобы говорить в этом канале, вы должны включить режим рации.",
});

/** Строка `juL9/L`: polite-статус при ошибке (сама ошибка — assertive-регион). */
const DISCONNECTED_TEXT = "Голосовая связь отключена";
/** `[D]` tjtv3P — подпись своей строки при mute. */
const SELF_MUTE_TEXT = "Этот микрофон отключён";
const OTHER_MUTE_TEXT = "микрофон отключён на сервере";
/** `[D]` vxfv7v — почему PTT не сработал. */
const PTT_BLOCKED_TEXT = "Горячая клавиша режима рации не работает во время игры из-за системных настроек доступа.";
const PTT_HELD_TEXT = "Режим рации: микрофон активен";

/** `[A]` лимит строк в панели 240 (VOICE.md §2.2). */
export const PARTICIPANT_LIMIT = 6;
/** `[A]` пороги `data-quality` (VG-06): наши числа, паритет не заявлен. */
const QUALITY_FINE_MAX = 120;
const QUALITY_AVERAGE_MAX = 250;

/** @type {ReadonlySet<string>} */
const STATE_SET = new Set(VOICE_STATES);
/** @type {ReadonlySet<string>} */
const PTT_SET = new Set(["idle", "held", "blocked"]);
/** Состояния, в которых пользователь уже «в канале» (есть сессия). */
/** @type {ReadonlySet<string>} */
const IN_CHANNEL = new Set([
  "loading",
  "connecting",
  "connected",
  "reconnecting",
  "empty",
  "error",
  "no-permission",
  "ptt-required",
  "device-missing",
  "server-mute",
]);

/**
 * `data-quality` по порогам спеки: `fine` ≤ 120, `average` ≤ 250, иначе `bad`.
 * @param {number | null} ping
 * @returns {"fine" | "average" | "bad" | null}
 */
export function pingQuality(ping) {
  if (typeof ping !== "number" || !Number.isFinite(ping) || ping < 0) return null;
  if (ping <= QUALITY_FINE_MAX) return "fine";
  if (ping <= QUALITY_AVERAGE_MAX) return "average";
  return "bad";
}

/**
 * Правило op5: `speaking` приходит готовым. Число считаем речью при `> 0` —
 * единственное наше правило (VOICE.md §2.3), громкость/анимацию не читаем.
 * @param {unknown} value
 * @returns {boolean}
 */
export function speakingFlag(value) {
  if (typeof value === "number") return Number.isFinite(value) && value > 0;
  return value === true;
}

/**
 * @param {number} count
 * @returns {string}
 */
function pluralParticipants(count) {
  const mod10 = count % 10;
  const mod100 = count % 100;
  if (mod10 === 1 && mod100 !== 11) return "участник";
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return "участника";
  return "участников";
}

/**
 * In-memory store панели. В реальном клиенте сюда пишут gateway-события
 * (`VOICE_STATE_UPDATE`), в спайке — фейк и тесты.
 * @param {Partial<VoiceSnapshot>} [initial]
 */
export function createVoiceStore(initial = {}) {
  /** @type {VoiceSnapshot} */
  let snapshot = Object.freeze({
    state: "idle",
    guildId: "",
    channelId: "",
    channelName: "",
    participants: Object.freeze(/** @type {VoiceParticipant[]} */ ([])),
    ping: null,
    selfMute: false,
    selfDeaf: false,
    serverMute: false,
    ptt: "idle",
  });
  /** @type {Set<(snapshot: VoiceSnapshot) => void>} */
  const listeners = new Set();

  /**
   * Участник — только поля события (`id`, `name`, `mute`, `speaking`, `self`);
   * сеть/аватар не запрашиваются (§5.1 п.8).
   * @param {unknown} value
   * @returns {VoiceParticipant}
   */
  function parseParticipant(value) {
    const bag = value && typeof value === "object"
      ? /** @type {Record<string, unknown>} */ (value)
      : {};
    const id = typeof bag.id === "string" ? bag.id : "";
    const name = typeof bag.name === "string" ? bag.name : "";
    if (id === "" || name === "") {
      throw new UIError("IPC_FAILED", "voice: участник без id/name");
    }
    const mute = bag.mute === "self" || bag.mute === "server" ? bag.mute : "none";
    const speaking = typeof bag.speaking === "number" || typeof bag.speaking === "boolean"
      ? bag.speaking
      : false;
    return { id, name, self: bag.self === true, speaking, mute };
  }

  /** @param {VoiceSnapshot} next */
  function commit(next) {
    snapshot = Object.freeze(next);
    for (const listener of [...listeners]) listener(snapshot);
    return snapshot;
  }

  /**
   * Частичное обновление. Неизвестное состояние/`ptt` игнорируется: панель не
   * должна «повиснуть» в состоянии вне контракта.
   * @param {Partial<VoiceSnapshot> & { state?: string, ptt?: string }} patch
   * @returns {VoiceSnapshot}
   */
  function set(patch) {
    /** @type {VoiceSnapshot} */
    const next = { ...snapshot };
    if (typeof patch.state === "string" && STATE_SET.has(patch.state)) {
      next.state = /** @type {VoiceState} */ (patch.state);
    }
    if (typeof patch.ptt === "string" && PTT_SET.has(patch.ptt)) {
      next.ptt = /** @type {PttState} */ (patch.ptt);
    }
    if (typeof patch.guildId === "string") next.guildId = patch.guildId;
    if (typeof patch.channelId === "string") next.channelId = patch.channelId;
    if (typeof patch.channelName === "string") next.channelName = patch.channelName;
    if (patch.ping === null) next.ping = null;
    else if (typeof patch.ping === "number" && Number.isFinite(patch.ping) && patch.ping >= 0) {
      next.ping = patch.ping;
    }
    if (typeof patch.selfMute === "boolean") next.selfMute = patch.selfMute;
    if (typeof patch.selfDeaf === "boolean") next.selfDeaf = patch.selfDeaf;
    if (typeof patch.serverMute === "boolean") next.serverMute = patch.serverMute;
    if (Array.isArray(patch.participants)) {
      next.participants = Object.freeze(patch.participants.map(parseParticipant));
    }
    return commit(next);
  }

  // `initial` — тот же путь, что и `set`: валидация одна.
  if (Object.keys(initial).length > 0) set(initial);

  return {
    /** @returns {VoiceSnapshot} */
    get() {
      return snapshot;
    },
    get state() {
      return snapshot.state;
    },
    set,
    /**
     * @param {(snapshot: VoiceSnapshot) => void} listener
     * @returns {() => void} отписка
     */
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

/**
 * Панель. `service === null` → `disabled` (текущая сборка); fake store + fake
 * service дают все 12 состояний.
 * @param {{ host: HTMLElement, store: ReturnType<typeof createVoiceStore>, service?: VoiceService | null }} options
 */
export function createVoicePanel(options) {
  const host = options.host;
  const store = options.store;
  const service = options.service ?? null;
  if (!host) throw new UIError("ELEMENT_MISSING", "voice: нет host-элемента #voice-panel");
  if (!store) throw new UIError("ELEMENT_MISSING", "voice: нет store");
  // Тест/браузер: документ берём у host — глобальный `document` не нужен.
  const doc = /** @type {Document} */ (host.ownerDocument);

  /** @param {string} tag @param {string} [className] @param {string} [text] @returns {HTMLElement} */
  function el(tag, className = "", text = "") {
    const node = doc.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  }

  /**
   * Инлайн-иконка (никаких растров/шрифтов — VK-08).
   * @param {string[]} paths
   * @returns {Element}
   */
  function icon(paths) {
    const svg = doc.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("class", "icon");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    for (const d of paths) {
      const path = doc.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", d);
      path.setAttribute("fill", "none");
      path.setAttribute("stroke", "currentColor");
      path.setAttribute("stroke-width", "2");
      svg.append(path);
    }
    return svg;
  }

  /**
   * @param {string} id
   * @param {string} className
   * @param {string} label
   * @param {string} action
   * @returns {HTMLButtonElement}
   */
  function actionButton(id, className, label, action) {
    const node = /** @type {HTMLButtonElement} */ (doc.createElement("button"));
    node.id = id;
    node.className = className;
    node.type = "button";
    node.setAttribute("aria-label", label);
    node.setAttribute("data-voice-action", action);
    return node;
  }

  /* ---------- разметка (один код-путь в браузере и шиме) ---------- */

  host.setAttribute("aria-label", "Голосовая связь");

  const status = el("div", "voice-status");
  status.id = "voice-status";
  status.setAttribute("aria-live", "polite");
  status.setAttribute("aria-atomic", "true");

  const ping = actionButton("voice-ping", "voice-ping", "Качество соединения", "ping");
  ping.setAttribute("aria-describedby", "voice-ping-value");
  ping.append(icon(["M4 15a5 5 0 0 1 5-5", "M2 20h.01", "M7 20h.01", "M12 20h.01"]));
  const pingValue = el("span", "visually-hidden");
  pingValue.id = "voice-ping-value";

  const statusText = el("div", "voice-status-text");
  const statusLabel = el("p", "voice-status-label");
  statusLabel.id = "voice-status-label";
  const channelLink = el("a", "voice-channel");
  channelLink.id = "voice-channel-link";
  channelLink.setAttribute("href", "");
  const statusHint = el("p", "voice-status-hint");
  statusHint.id = "voice-status-hint";
  statusHint.setAttribute("hidden", "");
  statusText.append(statusLabel, channelLink, statusHint);

  const statusActions = el("div", "voice-status-actions");
  const join = actionButton("voice-join", "voice-join", "Присоединиться к голосовому каналу", "join");
  join.textContent = "Войти";
  const leave = actionButton("voice-leave", "voice-leave", "Отключиться", "leave");
  leave.append(icon(["M4 4l16 16", "M7 7a8 8 0 0 0 10 10"]));
  const reconnect = actionButton("voice-reconnect", "voice-reconnect", "Переподключиться", "reconnect");
  reconnect.textContent = "Переподключиться";
  statusActions.append(join, leave, reconnect);
  status.append(ping, pingValue, statusText, statusActions);

  const participants = el("div", "voice-participants");
  participants.id = "voice-participants";
  participants.setAttribute("role", "list");
  participants.setAttribute("aria-label", "Участники голосового канала");
  const overflow = el("p", "voice-overflow");
  overflow.id = "voice-overflow";
  overflow.setAttribute("hidden", "");
  const empty = el("p", "voice-empty", VOICE_TEXT.empty);
  empty.id = "voice-empty";
  empty.setAttribute("role", "status");
  empty.setAttribute("hidden", "");

  const error = el("p", "voice-error");
  error.id = "voice-error";
  error.setAttribute("role", "alert");
  error.setAttribute("hidden", "");

  const extraActions = el("div", "voice-actions");
  const enablePtt = actionButton("voice-enable-ptt", "voice-action", "Перейти на режим рации", "enable-ptt");
  enablePtt.textContent = "Перейти на режим рации";
  enablePtt.setAttribute("hidden", "");
  const micTest = actionButton("voice-mic-test", "voice-action", "Проверка микрофона", "mic-test");
  micTest.textContent = "Проверка микрофона";
  micTest.setAttribute("hidden", "");
  extraActions.append(enablePtt, micTest);

  const userRow = el("div", "voice-user-row");
  userRow.id = "voice-user-row";
  /** @type {HTMLButtonElement} */
  const mute = /** @type {HTMLButtonElement} */ (doc.createElement("button"));
  mute.id = "voice-mute";
  mute.className = "voice-toggle";
  mute.type = "button";
  mute.setAttribute("role", "switch");
  mute.setAttribute("aria-checked", "false");
  mute.setAttribute("aria-label", "Заглушить");
  mute.setAttribute("data-voice-action", "mute");
  mute.append(icon(["M12 19v3", "M8 22h8", "M12 15a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v7a3 3 0 0 0 3 3z"]));
  /** @type {HTMLButtonElement} */
  const deafen = /** @type {HTMLButtonElement} */ (doc.createElement("button"));
  deafen.id = "voice-deafen";
  deafen.className = "voice-toggle";
  deafen.type = "button";
  deafen.setAttribute("role", "switch");
  deafen.setAttribute("aria-checked", "false");
  deafen.setAttribute("aria-label", "Откл. звук");
  deafen.setAttribute("data-voice-action", "deafen");
  deafen.append(icon(["M4 14v-2a8 8 0 0 1 16 0v2", "M4 14h3v6H5a1 1 0 0 1-1-1z", "M20 14h-3v6h2a1 1 0 0 0 1-1z"]));
  userRow.append(mute, deafen);

  const a11y = el("span", "visually-hidden");
  a11y.id = "voice-a11y";
  a11y.setAttribute("aria-live", "polite");

  host.replaceChildren(status, participants, overflow, empty, error, extraActions, userRow, a11y);

  /* ---------- поведение ---------- */

  /** Сборка без голосового сервиса: единственное честное состояние — disabled. */
  const available = service !== null;
  /** Защита от повторного клика, пока действие в полёте (VP-08). */
  let pending = false;

  /** @param {string} message */
  function announce(message) {
    a11y.textContent = message;
  }

  /**
   * @param {Node | null} node
   * @returns {boolean}
   */
  function insidePanel(node) {
    /** @type {Node | null} */
    let current = node;
    while (current) {
      if (current === host) return true;
      current = current.parentNode;
    }
    return false;
  }

  function canUseAudio() {
    const state = host.getAttribute("data-state") ?? "";
    return available && IN_CHANNEL.has(state);
  }

  /** Вход/повтор: ровно один вызов на клик, без оптимистичного `connected`. */
  async function joinChannel() {
    if (!available || pending || !service) return;
    const focusInside = insidePanel(doc.activeElement);
    pending = true;
    try {
      await service.join(store.get().channelId);
    } catch {
      // Ошибку входа показываем только фактом store (assertive-регион).
      store.set({ state: "error" });
      if (focusInside) reconnect.focus();
    } finally {
      pending = false;
    }
  }

  async function leaveChannel() {
    if (!available || pending || !service) return;
    pending = true;
    try {
      await service.leave();
    } catch {
      store.set({ state: "error" });
    } finally {
      pending = false;
    }
  }

  /**
   * @param {"selfMute" | "selfDeaf"} key
   * @param {(mute: boolean) => Promise<unknown>} call
   */
  async function toggleSelf(key, call) {
    if (!canUseAudio() || pending) return;
    pending = true;
    try {
      // Не оптимистично: aria-checked меняет только store-событие.
      await call(!store.get()[key]);
    } catch {
      store.set({ state: "error" });
    } finally {
      pending = false;
    }
  }

  join.addEventListener("click", () => {
    if (!available) {
      // Не выдаём disabled за успех: причина уходит в live-регион.
      announce(VOICE_TEXT.disabled);
      return;
    }
    void joinChannel();
  });
  reconnect.addEventListener("click", () => {
    void joinChannel();
  });
  leave.addEventListener("click", () => {
    void leaveChannel();
  });
  mute.addEventListener("click", () => {
    if (!service) return;
    if (host.getAttribute("data-state") === "server-mute") return;
    void toggleSelf("selfMute", (value) => service.setSelfMute?.(value) ?? Promise.resolve());
  });
  deafen.addEventListener("click", () => {
    if (!service) return;
    void toggleSelf("selfDeaf", (value) => service.setSelfDeaf?.(value) ?? Promise.resolve());
  });
  enablePtt.addEventListener("click", () => {
    if (!service || typeof service.enablePtt !== "function") return;
    void service.enablePtt().catch(() => {
      store.set({ state: "error" });
    });
  });
  micTest.addEventListener("click", () => {
    // Захват микрофона запрещён §5.3 — только честное сообщение.
    announce("Проверка микрофона недоступна в этой сборке");
  });

  /** @param {HTMLElement} node @param {boolean} hide */
  function setHidden(node, hide) {
    if (hide) node.setAttribute("hidden", "");
    else node.removeAttribute("hidden");
  }

  /* ---------- рендер состояний ---------- */

  function render() {
    const snapshot = store.get();
    const state = /** @type {VoiceState} */ (available ? snapshot.state : "disabled");
    const hasChannel = IN_CHANNEL.has(state) && snapshot.channelId !== "";

    host.setAttribute("data-state", state);
    host.setAttribute("data-self-mute", snapshot.selfMute ? "true" : "false");
    host.setAttribute("data-self-deaf", snapshot.selfDeaf ? "true" : "false");
    host.setAttribute("data-ptt", snapshot.ptt);
    if (state === "disabled") host.setAttribute("aria-disabled", "true");
    else host.removeAttribute("aria-disabled");

    // Блок связи: idle скрыт целиком; disabled — видимый честный статус.
    setHidden(status, state === "idle");

    const statusBusy = state === "loading";
    if (statusBusy) status.setAttribute("aria-busy", "true");
    else status.removeAttribute("aria-busy");

    if (state === "error") statusLabel.textContent = DISCONNECTED_TEXT;
    else statusLabel.textContent = VOICE_TEXT[state];
    if (statusBusy) {
      // Skeleton — наша сборка (`[A]`), строки статуса ещё нет.
      statusLabel.textContent = "";
      if (statusText.querySelector(".voice-skeleton") === null) {
        for (let index = 0; index < 2; index += 1) {
          const skeleton = el("span", "voice-skeleton");
          skeleton.setAttribute("aria-hidden", "true");
          statusText.append(skeleton);
        }
      }
    } else {
      for (const skeleton of [...statusText.querySelectorAll(".voice-skeleton")]) skeleton.remove();
    }

    const errorVisible = state === "error";
    error.textContent = errorVisible ? VOICE_TEXT.error : "";
    setHidden(error, !errorVisible);

    const hint = /** @type {Record<string, string>} */ (VOICE_HINT)[state] ?? "";
    statusHint.textContent = hint;
    setHidden(statusHint, hint === "");

    if (hasChannel) {
      channelLink.textContent = `📞┃${snapshot.channelName}`;
      channelLink.setAttribute("title", snapshot.channelName);
      channelLink.setAttribute("href", `#/channels/${snapshot.guildId}/${snapshot.channelId}`);
      setHidden(channelLink, false);
    } else {
      channelLink.textContent = "";
      channelLink.removeAttribute("title");
      channelLink.setAttribute("href", "");
      setHidden(channelLink, true);
    }

    // Ping: видимое значение только для connected (иначе aria-disabled).
    const quality = state === "connected" ? pingQuality(snapshot.ping) : null;
    const pingValueText = quality !== null && snapshot.ping !== null
      ? `${Math.round(snapshot.ping)} ms`
      : "";
    pingValue.textContent = pingValueText;
    if (quality !== null) {
      ping.removeAttribute("aria-disabled");
      ping.setAttribute("data-quality", quality);
    } else {
      ping.setAttribute("aria-disabled", "true");
      ping.removeAttribute("data-quality");
    }

    const joinVisible = state === "idle" || state === "disabled";
    setHidden(join, !joinVisible);
    if (state === "disabled") {
      join.setAttribute("aria-disabled", "true");
      join.setAttribute("title", VOICE_TEXT.disabled);
    } else {
      join.removeAttribute("aria-disabled");
      join.removeAttribute("title");
    }
    setHidden(leave, !IN_CHANNEL.has(state));
    setHidden(reconnect, !errorVisible);
    setHidden(enablePtt, state !== "ptt-required");
    setHidden(micTest, state !== "device-missing");

    // Участники: скрыты в idle/disabled и пока идёт подключение.
    const participantsVisible = IN_CHANNEL.has(state)
      && state !== "loading"
      && state !== "connecting"
      && state !== "reconnecting";
    setHidden(participants, !participantsVisible);
    setHidden(overflow, !participantsVisible);
    setHidden(empty, !(participantsVisible && state === "empty" && snapshot.participants.length === 0));
    if (participantsVisible) renderParticipants(snapshot);
    else {
      participants.replaceChildren();
      overflow.textContent = "";
      setHidden(overflow, true);
    }

    const audio = canUseAudio();
    mute.setAttribute("aria-checked", snapshot.selfMute ? "true" : "false");
    deafen.setAttribute("aria-checked", snapshot.selfDeaf ? "true" : "false");
    if (!audio || state === "server-mute") mute.setAttribute("aria-disabled", "true");
    else mute.removeAttribute("aria-disabled");
    if (!audio) deafen.setAttribute("aria-disabled", "true");
    else deafen.removeAttribute("aria-disabled");

    if (snapshot.ptt === "held") a11y.textContent = PTT_HELD_TEXT;
    else if (snapshot.ptt === "blocked") a11y.textContent = PTT_BLOCKED_TEXT;
  }

  /** @param {VoiceSnapshot} snapshot */
  function renderParticipants(snapshot) {
    const ordered = [...snapshot.participants].sort(
      (left, right) => Number(right.self) - Number(left.self),
    );
    const visible = ordered.slice(0, PARTICIPANT_LIMIT);
    /** @type {HTMLElement[]} */
    const rows = [];
    for (const person of visible) {
      const muteState = person.self && snapshot.serverMute
        ? "server"
        : person.self && snapshot.selfMute
          ? "self"
          : person.mute;
      const speaking = speakingFlag(person.speaking) || (person.self && snapshot.ptt === "held");
      const row = el("div", "voice-user");
      row.id = `voice-user-${person.id}`;
      row.setAttribute("role", "listitem");
      row.setAttribute("data-user-id", person.id);
      row.setAttribute("data-self", person.self ? "true" : "false");
      row.setAttribute("data-mute", muteState);
      row.setAttribute("data-speaking", speaking ? "true" : "false");
      if (person.self) row.classList.add("is-self");
      const avatar = el("span", "voice-user-avatar", person.self ? "SB" : person.name.slice(0, 2).toUpperCase());
      avatar.setAttribute("aria-hidden", "true");
      const name = el("span", "voice-user-name", person.name);
      const stateText = muteState === "self"
        ? SELF_MUTE_TEXT
        : muteState === "server"
          ? OTHER_MUTE_TEXT
          : "";
      const stateNode = el("span", "voice-user-state", stateText);
      const parts = [person.name];
      if (speaking) parts.push("говорит");
      if (muteState === "self") parts.push(SELF_MUTE_TEXT);
      else if (muteState === "server") parts.push(OTHER_MUTE_TEXT);
      row.setAttribute("aria-label", parts.join(", "));
      row.append(avatar, name, stateNode);
      rows.push(row);
    }
    participants.replaceChildren(...rows);
    const rest = ordered.length - visible.length;
    if (rest > 0) {
      overflow.textContent = `Ещё ${rest} ${pluralParticipants(rest)} в голосовом чате`;
      setHidden(overflow, false);
    } else {
      overflow.textContent = "";
      setHidden(overflow, true);
    }
  }

  const unsubscribe = store.subscribe(render);
  render();

  return {
    host,
    store,
    get state() {
      return host.getAttribute("data-state") ?? "";
    },
    render,
    dispose() {
      unsubscribe();
      host.replaceChildren();
    },
  };
}
