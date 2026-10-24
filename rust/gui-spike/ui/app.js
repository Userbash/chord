// @ts-check
/**
 * P0-01: статический фронтенд спайка (RW-39 / W39d).
 *
 * Без сборщика и фреймворков: честный замер оболочки, а не бандлера.
 * Модули фич — `ui/features/*.js`; здесь только сборка, UI_SNAPSHOT и замер.
 */
import { byId } from "./features/dom.js";
import { clearErrorState, reportError, showErrorState, UIError } from "./features/errors.js";
import { benchScroll, frontendReadyMs, startMetrics } from "./features/metrics.js";
import { createScreenRouter } from "./features/screens.js";
import { CHAT_ROWS, initChatScreen } from "./features/chat.js";
import { configureReactionTransport } from "./features/reactions.js";
import {
  initMemberScreen,
  MEMBER_GROUPS,
  MEMBER_ROWS,
  renderMemberPanel,
} from "./features/members.js";
import { initEmojiConsole } from "./features/emoji.js";
import { initSettingsConsole } from "./features/settings.js";
import { initActions } from "./features/actions.js";
import { createAuthGate } from "./features/auth.js";
import { createComposer } from "./features/composer.js";
import { createSearchOverlay, SEARCH_SECTIONS, selectedGuildId } from "./features/search-overlay.js";
import { createQuickSwitcher } from "./features/quick-switcher.js";
import { createVoicePanel, createVoiceStore } from "./features/voice.js";
import { createJoinServer } from "./features/join-server.js";
import { createTypingIndicator, createTypingSender } from "./features/typing-indicator.js";
import { asSnowflake, createDispatchStore } from "./features/dispatch-model.js";
import { createDispatchView } from "./features/dispatch-view.js";
import { createDispatchBridge, resolveDispatchListen } from "./features/dispatch-bridge.js";

/** @param {number} ms @returns {Promise<void>} */
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Тот же класс риска, что у benchScroll: если rAF не тикает, не ждём вечно.
const nextFrame = () => new Promise((resolve) => {
  let settled = false;
  const done = () => {
    if (settled) return;
    settled = true;
    clearTimeout(guard);
    resolve(undefined);
  };
  const guard = setTimeout(done, 1000);
  requestAnimationFrame(done);
});

/** Видимая поверхность ошибок: контент, не списки (у них ровно один sizer). */
const content = /** @type {HTMLElement | null} */ (document.querySelector(".content"));

// G2-02: `.error-state` — `position:absolute; inset:0`, поэтому containing block
// обязан быть `.content`, иначе оверлей накрывает viewport целиком. CSS-файлы
// вне файловой матрицы этой задачи (ui/components/*.css — read-only), поэтому
// позиционирующий контекст задаётся здесь; при переносе в style.css строка ниже
// останется безвредной (значение уже relative).
if (content && content.style.position === "") content.style.position = "relative";

/**
 * Команды-действия пользователя: успешный ответ снимает recoverable-ошибку
 * (G2-02, «ошибка → retry»). Служебные вызовы (метрики, снимки) оверлей не
 * трогают, иначе чужая ошибка исчезала бы от фонового опроса.
 */
const RECOVERABLE_COMMANDS = new Set([
  "search_remote",
  "send_message",
  "reply_message",
  "edit_message",
  "delete_message",
  "toggle_reaction",
  "settings_save",
]);

/**
 * Единая видимая поверхность ошибок: лог + оверлей в `.content`.
 * @param {unknown} error
 * @param {string} [code]
 */
function surface(error, code = "ACTION_FAILED") {
  const reported = reportError(code, error);
  showErrorState(content, reported);
  return reported;
}

/* ---------- L-01 (RW-40): видимый reconnect/offline state ---------- */

/** Интервал опроса `net_status`: чаще — лишний IPC, реже — долгий «залипший» статус. */
const NET_POLL_MS = 2000;
/** Команды сетевого слоя (`rust/gui-spike/src/main.rs`, RW-39/NET). */
const NET_COMMANDS = Object.freeze({ status: "net_status", connect: "net_connect" });
/** `GatewayStatus::Display` (`src/net.rs`): других значений контракт не отдаёт. */
const NET_STATUSES = new Set(["offline", "connecting", "ready", "reconnecting", "error"]);
/**
 * Тексты баннера. «Переподключение…»/«Нет соединения» — L-01; retry — ручной,
 * поэтому при ошибке `net_connect` текст объясняет причину, а не крутит цикл.
 */
const NET_TEXT = Object.freeze({
  reconnecting: "Переподключение…",
  offline: "Нет соединения",
  retry: "Повторить",
  retryBusy: "Подключение…",
  retryFailed: "Не удалось подключиться. Проверьте сеть и повторите.",
  sessionExpired: "Сессия истекла. Войдите снова.",
});

/**
 * Текст ошибки `net_connect` без тела ответа и секретов: 401/unauthorized и
 * сообщения про токен — «войдите снова», остальное — общий отказ.
 * @param {unknown} error
 * @returns {string}
 */
function netFailureText(error) {
  const bag = error && typeof error === "object"
    ? /** @type {Record<string, any>} */ (error)
    : {};
  const code = Number(bag.code ?? bag.status);
  const kind = String(bag.kind ?? "");
  const message = typeof error === "string" ? error : String(bag.message ?? "");
  if (code === 401 || /unauthor|invalidtoken/i.test(kind) || /401|unauthor|токен/i.test(message)) {
    return NET_TEXT.sessionExpired;
  }
  return NET_TEXT.retryFailed;
}

/**
 * Баннер сетевого состояния (L-01). Живёт в `.topbar` (в потоке, не поверх
 * чата), стартует только после `auth ready` и останавливается на dispose/logout.
 *
 * `net_status` вызывается без аргументов (токен в UI не попадает), `net_connect`
 * — только по клику «Повторить»: ошибка видна текстом и не порождает retry-loop.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   intervalMs?: number,
 * }} options
 */
function createNetStatus(options) {
  const { invoke } = options;
  const intervalMs = options.intervalMs ?? NET_POLL_MS;
  const node = /** @type {HTMLElement} */ (document.createElement("section"));
  node.id = "net-status";
  node.className = "net-status";
  node.setAttribute("role", "status");
  node.setAttribute("aria-live", "polite");
  node.setAttribute("aria-atomic", "true");
  node.setAttribute("data-state", "ready");
  node.setAttribute("hidden", "");

  const text = document.createElement("span");
  text.className = "net-status-text";
  const retry = /** @type {any} */ (document.createElement("button"));
  retry.id = "net-retry";
  retry.className = "net-status-retry";
  retry.type = "button";
  retry.setAttribute("data-action", "net-retry");
  retry.textContent = NET_TEXT.retry;
  retry.setAttribute("hidden", "");
  node.append(text, retry);
  (document.querySelector(".topbar") ?? document.body).append(node);

  /** @type {string} */
  let state = "ready";
  /** @type {ReturnType<typeof setInterval> | null} */
  let timer = null;
  let polling = false;
  let disposed = false;
  let inFlight = false;
  /** Ручной retry в полёте: второй клик не шлёт второй IPC. */
  let busy = false;
  /** Последняя ошибка `net_connect`: держим текст, пока пользователь не повторит. */
  let failure = "";

  /** @param {string} name @param {string} [message] */
  function apply(name, message = "") {
    const next = NET_STATUSES.has(name) ? name : "offline";
    const retryFocused = document.activeElement === retry;
    state = next;
    if (message !== "") failure = message;
    const connecting = next === "connecting" || next === "reconnecting";
    // Успешный/идущий контакт с ядром снимает текст прошлой ошибки connect.
    if (next === "ready" || connecting) failure = "";
    node.setAttribute("data-state", next);
    const hidden = next === "ready";
    if (hidden) node.setAttribute("hidden", "");
    else node.removeAttribute("hidden");
    text.textContent = hidden
      ? ""
      : message || (connecting ? NET_TEXT.reconnecting : failure || NET_TEXT.offline);
    const showRetry = !hidden && !connecting && !busy;
    retry.textContent = busy ? NET_TEXT.retryBusy : NET_TEXT.retry;
    if (showRetry) {
      retry.removeAttribute("hidden");
      retry.removeAttribute("disabled");
      retry.removeAttribute("aria-disabled");
    } else {
      retry.setAttribute("hidden", "");
    }
    // Фокус не крадём появлением региона; но если баннер скрылся, пока фокус
    // был на «Повторить», возвращаем его в чат (иначе фокус в hidden-узле).
    if (hidden && retryFocused) {
      const list = /** @type {any} */ (byId("chat-list"));
      if (list && typeof list.focus === "function") list.focus();
    }
  }

  /** @returns {Promise<void>} */
  async function refresh() {
    if (disposed || !polling || inFlight) return;
    // Свёрнутое/скрытое окно не опрашивает ядро: приложение не активно.
    if (/** @type {any} */ (document).hidden === true) return;
    inFlight = true;
    try {
      const value = await invoke(NET_COMMANDS.status);
      // После dispose/stop (logout) поздний ответ не должен менять UI.
      if (disposed || !polling) return;
      const status = typeof value === "string"
        ? value
        : String(/** @type {any} */ (value)?.status ?? "");
      if (status !== "") apply(status);
    } catch {
      // IPC статуса недоступен: это видимое «Нет соединения», а не тишина.
      if (!disposed) apply("offline");
    } finally {
      inFlight = false;
    }
  }

  function start() {
    if (disposed) return;
    polling = true;
    if (timer === null) timer = setInterval(() => { void refresh(); }, intervalMs);
    void refresh();
  }

  function stop() {
    polling = false;
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  }

  /** Ручное переподключение: ровно один `net_connect` на клик. */
  async function retryNow() {
    if (disposed || busy) return;
    busy = true;
    failure = "";
    apply("connecting");
    retry.setAttribute("disabled", "");
    retry.setAttribute("aria-disabled", "true");
    try {
      const value = await invoke(NET_COMMANDS.connect);
      if (disposed) return;
      const status = typeof value === "string"
        ? value
        : String(/** @type {any} */ (value)?.status ?? "");
      apply(status === "" ? "error" : status);
    } catch (error) {
      if (!disposed) apply("error", netFailureText(error));
    } finally {
      busy = false;
      if (!disposed && (state === "offline" || state === "error")) {
        retry.removeAttribute("hidden");
        retry.removeAttribute("disabled");
        retry.removeAttribute("aria-disabled");
        retry.textContent = NET_TEXT.retry;
      }
    }
  }

  function dispose() {
    disposed = true;
    stop();
    node.setAttribute("hidden", "");
    node.setAttribute("data-state", "ready");
  }

  return {
    node,
    get state() { return state; },
    get polling() { return polling; },
    start,
    stop,
    refresh,
    retry: retryNow,
    dispose,
  };
}

/* ---------- G-01 (RW-40): рейл серверов из `ready()` ---------- */

/**
 * Существующая IPC-команда спайка (`main.rs`: `ready`). Отдельной REST-ручки
 * списка гильдий в клиенте нет (`NETWORK-CONTRACTS §1.3`), новых endpoint'ов не
 * добавляем: источник списка — READY fixture.
 */
const READY_COMMAND = "ready";
/** Плиток-skeleton на время `ready()`: высота рейла не «прыгает» после загрузки. */
const RAIL_SKELETONS = 3;

/**
 * Гильдия рейла. Только подтверждённые поля payload (`GuildSummary`): `icon` —
 * хеш CDN, а не URL, поэтому иконок нет — монограмма (`SERVER-MENU §2.1`).
 * @typedef {{ id: string, name: string }} RailGuild
 */

/**
 * Монограмма 2 буквы: инициалы первых двух слов; для одного слова — первые две
 * буквы. `[T]`-подсказка «как в существующем `.guild-item`».
 * @param {string} name
 * @returns {string}
 */
function guildMonogram(name) {
  const words = String(name)
    .split(/\s+/)
    .map((word) => word.replace(/^[^\p{L}]+/u, ""))
    .filter((word) => /^\p{L}/u.test(word));
  const first = words[0] ?? "";
  const second = words[1] ?? "";
  const monogram = words.length >= 2
    ? `${[...first][0] ?? ""}${[...second][0] ?? ""}`
    : [...first].slice(0, 2).join("");
  return monogram.toUpperCase();
}

/**
 * Рейл серверов: состояния loading/ready/empty/error, выбор и клавиатура.
 *
 * Данные — только `ready()`; загрузка стартует после `auth ready` (до входа
 * показывать чужие серверы нечего). Выбранный сервер — первый из payload;
 * восстановление `lastGuildId` — G-01b, вне scope (`UiSettings` без поля).
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   onSelect?: (guild: RailGuild) => void,
 *   onAddServer?: (opener: HTMLElement) => void,
 * }} options
 */
function createGuildRail(options) {
  const { invoke } = options;
  const onSelect = options.onSelect ?? (() => {});
  const onAddServer = options.onAddServer ?? (() => {});
  /** Последняя гильдия, для которой уже запрошены каналы (повтор клика — no-op). */
  let notified = "";
  const railNode = byId("guild-rail");
  if (!railNode) throw new UIError("ELEMENT_MISSING", "в DOM нет #guild-rail");
  /** Явная аннотация: замыкания ниже не должны видеть `null`. @type {HTMLElement} */
  const rail = railNode;

  // Контейнер плиток создаётся здесь: новый id/класс в html ломал бы
  // контракт шима (`gui-spike-dom`: id/классы index.html ⊆ шим-DOM).
  const list = document.createElement("div");
  list.className = "guild-list";
  // Контракт G6 §3.1: `#guild-rail [role="navigation"]` — навигация по серверам
  // внутри рейла (экраны остаются отдельными `.rail-btn`).
  list.setAttribute("role", "navigation");
  list.setAttribute("aria-label", "Серверы");

  /** @type {RailGuild[]} */
  let guilds = [];
  /** @type {"idle" | "loading" | "ready" | "empty" | "error"} */
  let state = "idle";
  let disposed = false;

  /** @param {"idle" | "loading" | "ready" | "empty" | "error"} next */
  function setState(next) {
    state = next;
    rail.setAttribute("data-state", next);
    if (next === "loading") rail.setAttribute("aria-busy", "true");
    else rail.removeAttribute("aria-busy");
  }

  /** @returns {HTMLElement[]} */
  function tiles() {
    return /** @type {HTMLElement[]} */ ([...list.querySelectorAll("button.guild-item")]);
  }

  /**
   * Контейнер вставляется перед разделителем (brand, гильдии, sep, экраны).
   * В DOM-шиме `insertBefore` нет — там порядок не проверяется, работает append.
   */
  function ensureList() {
    if (list.parentNode) return;
    for (const stub of [...rail.querySelectorAll(".guild-item")]) stub.remove();
    const separator = rail.querySelector(".rail-sep");
    const host = /** @type {any} */ (rail);
    if (separator && typeof host.insertBefore === "function") host.insertBefore(list, separator);
    else rail.append(list);
  }

  /** @returns {string} id выбранной плитки (читается из DOM: leave-flow правит DOM). */
  function selectedId() {
    const node = tiles().find((tile) => tile.classList.contains("is-selected"));
    return node?.getAttribute("data-guild-id") ?? "";
  }

  /**
   * @param {string} id
   * @param {boolean} focus
   */
  function select(id, focus) {
    const nodes = tiles();
    const target = nodes.find((node) => (node.getAttribute("data-guild-id") ?? "") === id) ?? null;
    if (!target) return;
    for (const node of nodes) {
      const current = node === target;
      if (current) {
        node.classList.add("is-selected");
        node.setAttribute("aria-current", "true");
        node.setAttribute("tabindex", "0");
      } else {
        node.classList.remove("is-selected");
        node.removeAttribute("aria-current");
        node.setAttribute("tabindex", "-1");
      }
    }
    // Выбранный сервер задаёт дерево каналов (C-02: `channels(guildId)`).
    const guild = guilds.find((item) => item.id === id);
    const heading = document.querySelector(".tree-guild");
    if (heading && guild) heading.textContent = guild.name;
    if (guild && guild.id !== notified) {
      notified = guild.id;
      onSelect(guild);
    }
    if (focus && typeof target.focus === "function") target.focus();
  }

  /**
   * Выбор без побочных эффектов `select()`: только классы/ARIA на существующих
   * плитках. Нужен при upsert из dispatch-события: `onSelect` (channels) не
   * вызывается, фокус и активный элемент не двигаются (PLAN §3.1).
   * @param {string} id
   */
  function paintSelection(id) {
    for (const node of tiles()) {
      const current = (node.getAttribute("data-guild-id") ?? "") === id;
      if (current) {
        node.classList.add("is-selected");
        node.setAttribute("aria-current", "true");
        node.setAttribute("tabindex", "0");
      } else {
        node.classList.remove("is-selected");
        node.removeAttribute("aria-current");
        node.setAttribute("tabindex", "-1");
      }
    }
  }

  /**
   * @param {number | "first" | "last"} to
   * @param {boolean} focus
   */
  function move(to, focus) {
    const nodes = tiles();
    if (nodes.length === 0) return;
    const index = nodes.findIndex((node) => node.classList.contains("is-selected"));
    let next = 0;
    if (to === "first") next = 0;
    else if (to === "last") next = nodes.length - 1;
    else next = index < 0 ? 0 : (index + to + nodes.length) % nodes.length;
    const id = nodes[next]?.getAttribute("data-guild-id") ?? "";
    if (id) select(id, focus);
  }

  /**
   * @param {HTMLElement} tile
   * @param {string} id
   */
  function bindTile(tile, id) {
    // Шим не всплывает события: слушатели прямые, как у `[data-action]` (§8.1).
    tile.addEventListener("click", () => select(id, false));
    tile.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        move(1, true);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        move(-1, true);
      } else if (event.key === "Home") {
        event.preventDefault();
        move("first", true);
      } else if (event.key === "End") {
        event.preventDefault();
        move("last", true);
      }
    });
  }

  /**
   * Поля только из payload; неизвестная форма — ошибка, а не выдуманное имя.
   * @param {unknown} value
   * @returns {RailGuild}
   */
  function parseGuild(value) {
    const bag = value && typeof value === "object"
      ? /** @type {Record<string, unknown>} */ (value)
      : {};
    const id = typeof bag.id === "string" ? bag.id : "";
    const name = typeof bag.name === "string" ? bag.name : "";
    if (id === "" || name === "") throw new UIError("IPC_FAILED", "ready: гильдия без id/name");
    return { id, name };
  }

  /** @param {RailGuild[]} items */
  function renderGuilds(items, /** @type {string} */ focusId = "") {
    guilds = items;
    list.replaceChildren();
    for (const guild of items) {
      const tile = document.createElement("button");
      tile.className = "guild-item";
      tile.type = "button";
      tile.setAttribute("data-guild-id", guild.id);
      // DOM-шим не выводит data-* из setAttribute в dataset — пишем оба
      // представления (в браузере это один и тот же атрибут).
      /** @type {any} */ (tile).dataset.guildId = guild.id;
      tile.setAttribute("aria-label", `Сервер ${guild.name}`);
      tile.textContent = guildMonogram(guild.name);
      bindTile(tile, guild.id);
      list.append(tile);
    }
    // G-03: «+» — последняя плитка группы «Серверы» (SERVER-MENU §4.1), но не
    // `.guild-item`: она не гильдия и не участвует в выборе/клавиатуре списка.
    list.append(addServerTile());
    const chosen = items.find((item) => item.id === focusId) ?? items[0];
    if (chosen) select(chosen.id, false);
    setState("ready");
  }

  /** @returns {HTMLElement} плитка «Присоединиться» (открывает `#join-server-dialog`). */
  function addServerTile() {
    const tile = document.createElement("button");
    tile.className = "guild-add";
    tile.type = "button";
    tile.setAttribute("data-action", "add-server");
    tile.setAttribute("aria-label", "Добавить сервер");
    tile.setAttribute("aria-haspopup", "dialog");
    tile.setAttribute("aria-expanded", "false");
    tile.textContent = "+";
    // Слушатель прямой: плитка создаётся после `initActions` (после auth ready),
    // поэтому общий обход `[data-action]` её уже не увидит.
    tile.addEventListener("click", () => onAddServer(tile));
    return tile;
  }

  function renderEmpty() {
    guilds = [];
    list.replaceChildren();
    const empty = document.createElement("p");
    empty.className = "guild-empty";
    empty.setAttribute("role", "status");
    empty.textContent = "Нет серверов";
    list.append(empty, addServerTile());
    setState("empty");
  }

  /**
   * Видимая ошибка вместо тишины. `console.error` не дублируем: он зарезервирован
   * за `features/errors.js`, а последствие здесь — `.error-state` с retry.
   * @param {unknown} _error
   */
  function renderError(_error) {
    guilds = [];
    list.replaceChildren();
    const node = document.createElement("div");
    node.className = "error-state";
    node.setAttribute("role", "alert");
    node.setAttribute("data-state", "error");
    node.setAttribute("data-error-code", "IPC_FAILED");
    const title = document.createElement("p");
    title.className = "error-state-title";
    title.textContent = "Ошибка: не удалось загрузить серверы";
    const hint = document.createElement("p");
    hint.className = "error-state-hint";
    hint.textContent = "Повторите запрос или перезапустите спайк";
    const retry = document.createElement("button");
    retry.className = "error-state-retry";
    retry.type = "button";
    retry.textContent = "Повторить";
    retry.addEventListener("click", () => { void load(); });
    node.append(title, hint, retry);
    list.append(node);
    setState("error");
  }

  /** @returns {Promise<void>} */
  async function load() {
    if (disposed || state === "loading") return;
    setState("loading");
    guilds = [];
    ensureList();
    list.replaceChildren();
    for (let index = 0; index < RAIL_SKELETONS; index += 1) {
      const skeleton = document.createElement("span");
      skeleton.className = "guild-skeleton";
      skeleton.setAttribute("aria-hidden", "true");
      list.append(skeleton);
    }
    try {
      const payload = await invoke(READY_COMMAND);
      if (disposed) return;
      const bag = /** @type {{ guilds?: unknown }} */ (payload ?? {});
      if (!Array.isArray(bag.guilds)) throw new UIError("IPC_FAILED", "ready: в ответе нет guilds[]");
      const items = bag.guilds.map(parseGuild);
      if (items.length === 0) renderEmpty();
      else renderGuilds(items);
    } catch (error) {
      if (disposed) return;
      renderError(error);
    }
  }

  return {
    node: list,
    get state() { return state; },
    get selectedId() { return selectedId(); },
    load,
    /**
     * L-02-UI: upsert по `guildId` из READY-снапшота. Выбор, `aria-current`,
     * роевер-навигация и `document.activeElement` сохраняются; `onSelect` не
     * вызывается (иначе каждое READY дёргало бы `channels(guildId)`).
     * Пустой массив — валидный полный снапшот (аккаунт без серверов):
     * очищает плитки и показывает пустое состояние.
     * @param {RailGuild[]} items
     */
    applyGuilds(items) {
      if (disposed || !Array.isArray(items)) return;
      ensureList();
      if (items.length === 0) {
        renderEmpty();
        return;
      }
      const previousSelected = selectedId();
      /** @type {Map<string, HTMLElement>} */
      const existing = new Map();
      for (const tile of tiles()) {
        const id = tile.getAttribute("data-guild-id") ?? "";
        if (id !== "") existing.set(id, tile);
      }
      /** @type {Set<string>} */
      const seen = new Set();
      /** @type {HTMLElement[]} */
      const ordered = [];
      guilds = items;
      for (const guild of items) {
        if (guild.id === "" || guild.name === "" || seen.has(guild.id)) continue;
        seen.add(guild.id);
        let tile = existing.get(guild.id) ?? null;
        if (!tile) {
          const created = document.createElement("button");
          created.className = "guild-item";
          created.type = "button";
          created.setAttribute("data-guild-id", guild.id);
          /** @type {any} */ (created).dataset.guildId = guild.id;
          bindTile(created, guild.id);
          tile = created;
        }
        const label = `Сервер ${guild.name}`;
        if (tile.getAttribute("aria-label") !== label) tile.setAttribute("aria-label", label);
        const mark = guildMonogram(guild.name);
        if (tile.textContent !== mark) tile.textContent = mark;
        ordered.push(tile);
      }
      // Отсутствующие плитки убираем только когда рейл уже готов: во время
      // `ready()`/ошибки skeleton и retry не трогаем.
      if (state === "ready") {
        for (const [id, tile] of existing) if (!seen.has(id)) tile.remove();
      }
      const add = /** @type {HTMLElement | null} */ (list.querySelector(".guild-add"));
      list.replaceChildren(...ordered, add ?? addServerTile());
      const keep = seen.has(previousSelected)
        ? previousSelected
        : (ordered[0]?.getAttribute("data-guild-id") ?? "");
      if (keep !== "") paintSelection(keep);
      setState("ready");
    },
    /**
     * G-03: после accept 2xx добавить гильдию в рейл. Имя приходит из preview
     * (backend accept отдаёт только `guildId`), повторная `ready()` не нужна.
     * Выбор новой плитки запускает `onSelect` → `channels(guildId)`.
     * @param {RailGuild} guild
     */
    addGuild(guild) {
      if (disposed || guild.id === "" || guild.name === "") return;
      if (!guilds.some((item) => item.id === guild.id)) guilds = [...guilds, guild];
      ensureList();
      renderGuilds(guilds, guild.id);
      select(guild.id, true);
    },
    dispose() {
      disposed = true;
      guilds = [];
      list.replaceChildren();
    },
  };
}

/* ---------- C-02 (RW-40): дерево каналов из `channels(guildId)` ---------- */

/**
 * Тип канала из READY (`G6-REPORT §3.4`): открывают чат только `0` (текстовый) и
 * `5` (объявление); `2` голос, `4` категория, `15` форум — нет, но остаются
 * видны с причиной. Плитки собираются из `id`/`type`/`name` payload.
 * @typedef {{ key: string, title: string, icon: string, openable: boolean, reason: string }} ChannelKind
 */

/** @type {Record<number, ChannelKind>} */
const CHANNEL_KINDS = {
  0: { key: "text", title: "ТЕКСТОВЫЕ КАНАЛЫ", icon: "#", openable: true, reason: "" },
  5: { key: "announcement", title: "ОБЪЯВЛЕНИЯ", icon: "📢", openable: true, reason: "" },
  2: {
    key: "voice",
    title: "ГОЛОСОВЫЕ КАНАЛЫ",
    icon: "🔊",
    openable: false,
    reason: "Голосовой канал: подключение голоса в спайке недоступно",
  },
  4: {
    key: "category",
    title: "КАТЕГОРИИ",
    icon: "🗂",
    openable: false,
    reason: "Категория: чат не открывается",
  },
  15: {
    key: "forum",
    title: "ФОРУМЫ",
    icon: "💬",
    openable: false,
    reason: "Форум: вид веток — Phase 3",
  },
};

/** Порядок групп дерева (не порядок payload): чат-каналы выше служебных. */
const CHANNEL_GROUP_ORDER = [0, 5, 15, 2, 4];

/** Тип, которого нет в контракте: виден, но чат не открывает. */
const UNKNOWN_CHANNEL_KIND = Object.freeze({
  key: "unknown",
  title: "ДРУГИЕ КАНАЛЫ",
  icon: "•",
  openable: false,
  reason: "Неизвестный тип канала: чат не открывается",
});

/**
 * @typedef {{
 *   id: string,
 *   type: number,
 *   name: string,
 *   parentId: string,
 *   position: number,
 * }} TreeChannel
 */

/**
 * Видимый текст отказа `channels()`: типизированная ошибка `DataError`
 * (`{kind}`) без тела ответа и без токена.
 * @param {unknown} error
 * @returns {string}
 */
function channelTreeFailureText(error) {
  const bag = error !== null && typeof error === "object"
    ? /** @type {Record<string, any>} */ (error)
    : {};
  const kind = String(bag.kind ?? "");
  const status = Number(bag.status ?? bag.code ?? 0);
  if (status === 401 || /unauthor/i.test(kind)) return "Сессия истекла. Войдите снова.";
  if (/notfound/i.test(kind) || status === 404) return "Сервер не найден в локальном снимке";
  if (/parse|io/i.test(kind)) return "Не удалось прочитать каналы сервера";
  return "Каналы сервера недоступны";
}

/**
 * Дерево каналов выбранной гильдии: `channels({guildId})` → группы по типам.
 * Текстовый/announcement кликабельны и открывают реальный канал (`onOpen`);
 * voice/category/forum показывают причину отказа и чат не трогают.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   onOpen?: (channel: { id: string, name: string }) => void,
 * }} options
 */
function createChannelTree(options) {
  const { invoke } = options;
  const treeNode = byId("channel-tree");
  if (!treeNode) throw new UIError("ELEMENT_MISSING", "в DOM нет #channel-tree");
  /** @type {HTMLElement} */
  const tree = treeNode;
  const scrollNode = tree.querySelector(".tree-scroll");
  if (!scrollNode) throw new UIError("ELEMENT_MISSING", "в дереве нет .tree-scroll");
  /** @type {HTMLElement} */
  const scroll = /** @type {HTMLElement} */ (scrollNode);

  // Видимая причина отказа живёт над группами и обновляется кликом/загрузкой.
  const note = document.createElement("p");
  note.className = "channel-note";
  note.setAttribute("role", "status");
  note.setAttribute("aria-live", "polite");
  note.textContent = "Каналы загружаются вместе с сервером";

  /** @type {"idle" | "loading" | "ready" | "empty" | "error"} */
  let state = "idle";
  /** @type {string} */
  let guildId = "";
  /** @type {TreeChannel[]} */
  let channels = [];
  /** Токен загрузки: ответ прошлой гильдии не рисуется поверх новой. */
  let loadToken = 0;
  let disposed = false;

  /** @param {"idle" | "loading" | "ready" | "empty" | "error"} next */
  function setState(next) {
    state = next;
    tree.setAttribute("data-state", next);
    if (next === "loading") tree.setAttribute("aria-busy", "true");
    else tree.removeAttribute("aria-busy");
  }

  /** @returns {HTMLElement[]} */
  function channelButtons() {
    return /** @type {HTMLElement[]} */ ([...scroll.querySelectorAll(".channel-item")]);
  }

  /** @returns {HTMLElement[]} */
  function openableButtons() {
    return /** @type {HTMLElement[]} */ ([
      ...scroll.querySelectorAll('.channel-item[data-openable="true"]'),
    ]);
  }

  /**
   * Roving tabindex: в tab-порядке только фокусная (или первая) плитка.
   * @param {HTMLElement | null} focused
   */
  function syncTabindex(focused) {
    const items = openableButtons();
    for (const item of items) item.setAttribute("tabindex", item === focused ? "0" : "-1");
  }

  /** @param {HTMLElement} item */
  function selectItem(item) {
    for (const other of channelButtons()) {
      if (other === item) other.setAttribute("aria-current", "page");
      else other.removeAttribute("aria-current");
    }
    syncTabindex(item);
  }

  /**
   * @param {TreeChannel} channel
   * @param {ChannelKind} kind
   * @param {HTMLElement} button
   */
  function activate(channel, kind, button) {
    if (!kind.openable) {
      // Видимая причина: атрибут на плитке + общий live-регион дерева.
      button.setAttribute("data-denied", "true");
      note.textContent = kind.reason;
      return;
    }
    selectItem(button);
    const name = channel.name || channel.id;
    note.textContent = `Открыт канал «${name}»`;
    options.onOpen?.({ id: channel.id, name });
  }

  /**
   * @param {"ArrowDown" | "ArrowUp" | "Home" | "End"} key
   * @param {HTMLElement} from
   */
  function moveFocus(key, from) {
    const items = openableButtons();
    if (items.length === 0) return;
    const index = items.indexOf(from);
    /** @type {number} */
    let next = 0;
    if (key === "ArrowDown") next = index < 0 ? 0 : (index + 1) % items.length;
    else if (key === "ArrowUp") next = index <= 0 ? items.length - 1 : index - 1;
    else if (key === "End") next = items.length - 1;
    const target = items[next];
    if (!target) return;
    syncTabindex(target);
    if (typeof target.focus === "function") target.focus();
  }

  /**
   * @param {TreeChannel} channel
   * @returns {HTMLElement}
   */
  function channelNode(channel) {
    const kind = CHANNEL_KINDS[channel.type] ?? UNKNOWN_CHANNEL_KIND;
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.className = "channel-item";
    button.type = "button";
    button.setAttribute("data-channel-id", channel.id);
    button.setAttribute("data-channel-type", String(channel.type));
    if (channel.parentId !== "") button.setAttribute("data-parent-id", channel.parentId);
    button.setAttribute("data-openable", kind.openable ? "true" : "false");
    const icon = document.createElement("span");
    icon.className = "channel-icon";
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = kind.icon;
    const name = document.createElement("span");
    name.className = "channel-name";
    name.textContent = channel.name || channel.id;
    button.append(icon, name);
    if (!kind.openable) {
      const reason = document.createElement("span");
      reason.className = "channel-reason";
      reason.textContent = kind.key === "voice"
        ? "голос"
        : kind.key === "category"
          ? "категория"
          : kind.key === "forum"
            ? "форум"
            : "недоступно";
      button.append(reason);
      button.setAttribute("aria-disabled", "true");
      button.setAttribute("title", kind.reason);
    }
    button.addEventListener("click", () => activate(channel, kind, button));
    button.addEventListener("keydown", (event) => {
      const key = "key" in event ? String(event.key) : "";
      if (key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End") {
        event.preventDefault();
        moveFocus(key, button);
        return;
      }
      if (key === "Enter" || key === " ") {
        event.preventDefault();
        activate(channel, kind, button);
      }
    });
    item.append(button);
    return item;
  }

  /**
   * @param {number} type
   * @param {TreeChannel[]} items
   * @returns {HTMLElement}
   */
  function groupSection(type, items) {
    const kind = CHANNEL_KINDS[type] ?? UNKNOWN_CHANNEL_KIND;
    const section = document.createElement("section");
    section.className = "channel-group";
    section.setAttribute("data-channel-type", String(type));
    const heading = document.createElement("h3");
    heading.className = "channel-group-title";
    const toggle = document.createElement("button");
    toggle.className = "group-toggle";
    toggle.type = "button";
    toggle.setAttribute("aria-expanded", "true");
    toggle.textContent = kind.title;
    const list = document.createElement("ul");
    list.className = "channel-list";
    list.setAttribute("role", "list");
    for (const channel of items) list.append(channelNode(channel));
    toggle.addEventListener("click", () => {
      const expanded = toggle.getAttribute("aria-expanded") !== "false";
      toggle.setAttribute("aria-expanded", expanded ? "false" : "true");
      if (expanded) list.setAttribute("hidden", "");
      else list.removeAttribute("hidden");
    });
    heading.append(toggle);
    section.append(heading, list);
    return section;
  }

  /** @param {TreeChannel[]} rows */
  function renderTree(rows) {
    channels = rows;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Выберите канал";
    /** @type {Map<number, TreeChannel[]>} */
    const buckets = new Map();
    for (const channel of rows) {
      const bucket = buckets.get(channel.type);
      if (bucket) bucket.push(channel);
      else buckets.set(channel.type, [channel]);
    }
    const extra = [...buckets.keys()].filter((type) => !CHANNEL_GROUP_ORDER.includes(type));
    for (const type of [...CHANNEL_GROUP_ORDER, ...extra]) {
      const items = buckets.get(type);
      if (!items || items.length === 0) continue;
      scroll.append(groupSection(type, items));
    }
    const first = openableButtons()[0] ?? null;
    syncTabindex(first);
    setState("ready");
  }

  function renderEmpty() {
    channels = [];
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "В этом сервере нет каналов";
    const empty = document.createElement("p");
    empty.className = "channel-tree-state";
    empty.setAttribute("data-state", "empty");
    empty.setAttribute("role", "status");
    empty.textContent = "Каналы не найдены";
    scroll.append(empty);
    setState("empty");
  }

  /** @param {unknown} error */
  function renderError(error) {
    channels = [];
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Каналы не загрузились";
    const node = document.createElement("div");
    node.className = "channel-tree-error";
    node.setAttribute("data-state", "error");
    node.setAttribute("role", "alert");
    node.setAttribute("data-error-code", "IPC_FAILED");
    const title = document.createElement("p");
    title.className = "channel-tree-error-title";
    title.textContent = "Не удалось загрузить каналы";
    const hint = document.createElement("p");
    hint.className = "channel-tree-error-hint";
    hint.textContent = channelTreeFailureText(error);
    const retry = document.createElement("button");
    retry.className = "channel-tree-retry";
    retry.type = "button";
    retry.textContent = "Повторить";
    retry.addEventListener("click", () => { void load(guildId); });
    node.append(title, hint, retry);
    scroll.append(node);
    setState("error");
  }

  function renderLoading() {
    channels = [];
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Загрузка каналов…";
    const loading = document.createElement("p");
    loading.className = "channel-tree-state";
    loading.setAttribute("data-state", "loading");
    loading.setAttribute("role", "status");
    loading.textContent = "Загрузка каналов…";
    scroll.append(loading);
    setState("loading");
  }

  /**
   * @param {unknown} payload
   * @returns {TreeChannel[]}
   */
  function parseChannels(payload) {
    if (!Array.isArray(payload)) {
      throw new UIError("IPC_FAILED", "channels: ответ не массив");
    }
    return payload.map((value, index) => {
      const bag = value !== null && typeof value === "object"
        ? /** @type {Record<string, any>} */ (value)
        : {};
      const id = typeof bag.id === "string" ? bag.id : "";
      const type = Number(bag.type);
      if (id === "" || !Number.isInteger(type)) {
        throw new UIError("IPC_FAILED", `channels: канал ${index} без строкового id/type`);
      }
      return {
        id,
        type,
        name: typeof bag.name === "string" ? bag.name : "",
        parentId: typeof bag.parentId === "string" ? bag.parentId : "",
        position: typeof bag.position === "number" ? bag.position : 0,
      };
    });
  }

  /** @param {string} id */
  async function load(id) {
    if (disposed) return;
    const next = String(id ?? "").trim();
    if (next === "") {
      renderEmpty();
      return;
    }
    guildId = next;
    const token = ++loadToken;
    renderLoading();
    try {
      const payload = await invoke("channels", { guildId: next });
      if (disposed || token !== loadToken) return;
      const rows = parseChannels(payload);
      if (rows.length === 0) renderEmpty();
      else renderTree(rows);
    } catch (error) {
      if (disposed || token !== loadToken) return;
      renderError(error);
    }
  }

  return {
    node: scroll,
    load,
    /** @returns {"idle" | "loading" | "ready" | "empty" | "error"} */
    get state() { return state; },
    /** @returns {TreeChannel[]} */
    get channels() { return channels; },
    dispose() {
      disposed = true;
      channels = [];
      scroll.replaceChildren();
    },
  };
}

/**
 * @typedef {{
 *   router: ReturnType<typeof createScreenRouter>,
 *   chat: ReturnType<typeof initChatScreen>,
 *   tree: ReturnType<typeof createChannelTree>,
 *   members: ReturnType<typeof initMemberScreen>,
 *   emoji: ReturnType<typeof initEmojiConsole>,
 *   settings: ReturnType<typeof initSettingsConsole>,
 *   composer: ReturnType<typeof createComposer>,
 *   search: ReturnType<typeof createSearchOverlay>,
 *   quickSwitcher: ReturnType<typeof createQuickSwitcher>,
 *   auth: ReturnType<typeof createAuthGate>,
 *   net: ReturnType<typeof createNetStatus>,
 *   rail: ReturnType<typeof createGuildRail>,
 *   voice: ReturnType<typeof createVoicePanel>,
 *   join: ReturnType<typeof createJoinServer>,
 *   typing: ReturnType<typeof createTypingIndicator>,
 *   typingSender: ReturnType<typeof createTypingSender>,
 *   dispatch: ReturnType<typeof createDispatchBridge>,
 * }} App
 */

/** @type {App | null} */
let app = null;

/**
 * Доступ к собранному приложению для диагностики и поведенческих тестов
 * (DOM-шим не умеет события мыши/фокуса, поэтому состояния проверяются API).
 * @returns {App | null}
 */
export function currentApp() {
  return app;
}

function boot() {
  /** @type {Map<string, import("./features/virtual-list.js").VirtualList>} */
  const lists = new Map();
  /** @type {ReturnType<typeof initMemberScreen> | null} */
  let membersRef = null;
  const router = createScreenRouter({
    onShow: (name) => {
      const list = lists.get(name);
      // Показ экрана меняет clientHeight с 0 на реальную высоту — перерисовываем.
      if (list) requestAnimationFrame(() => list.paint());
      // Счётчики 6 групп зависят от загруженного окна участников.
      if (name === "members") membersRef?.refreshGroupChips();
    },
    onError: (error) => surface(error),
  });
  const tauri = /** @type {any} */ (window).__TAURI__;
  /**
   * P-03-UI: индикатор набора создаётся вместе с композером ниже; `invoke`
   * запоминает свой `user.id` из `ready()`, чтобы не показывать себя.
   * @type {ReturnType<typeof createTypingIndicator> | null}
   */
  let typing = null;
  /**
   * L-02-UI: подписка на `gateway://dispatch` и её DOM-проекция создаются до
   * первого IPC (см. ниже), а `start()` вызывается после `auth ready`.
   * @type {ReturnType<typeof createDispatchView> | null}
   */
  let dispatchView = null;
  /** @type {ReturnType<typeof createDispatchBridge> | null} */
  let dispatch = null;
  /**
   * Единственная точка IPC: успех пользовательского действия снимает
   * recoverable-ошибку (G2-02). Ошибки по-прежнему идут в `surface`.
   * @param {string} cmd
   * @param {any} args
   * @returns {Promise<any>}
   */
  const invoke = async (cmd, args) => {
    const result = await tauri.core.invoke(cmd, args);
    if (cmd === "ready") {
      // P2: id принимается только строкой или safe integer. Unsafe integer
      // потерял точность ещё при разборе JSON, поэтому String() дал бы чужой
      // id и ложный `me` у реакций (REVIEW-L02-UI).
      const userId = asSnowflake(result?.user?.id);
      if (typing && userId !== "") typing.setSelfUserId(userId);
      // Авторитетный selfUserId для реакций: READY dispatch его не перебивает.
      if (dispatch && userId !== "") dispatch.setSelfUserId(userId);
    }
    if (RECOVERABLE_COMMANDS.has(cmd)) clearErrorState(content);
    return result;
  };
  // L-01: баннер сетевого состояния. Создаётся до `initActions` — его
  // `data-action="net-retry"` должен попасть в общий прямой слушатель §8.1.
  // Поллинг стартует только после auth ready (ниже), не на гейте.
  const net = createNetStatus({ invoke });
  // V-08: голосовая панель. Голосового backend'а в сборке нет, поэтому
  // `service: null` → честное состояние `disabled` и заблокированный вход;
  // остальные 11 состояний доступны только через fake store (тесты).
  const voiceHost = byId("voice-panel");
  if (!voiceHost) throw new UIError("ELEMENT_MISSING", "в DOM нет #voice-panel");
  const voice = createVoicePanel({ host: voiceHost, store: createVoiceStore(), service: null });
  const chat = initChatScreen((error) => surface(error), invoke);
  // C-02: дерево каналов выбранной гильдии. Клик по текстовому/announcement
  // открывает реальный канал (`messages_page`) с настоящим channelId.
  const tree = createChannelTree({
    invoke,
    onOpen: (channel) => {
      void chat.openChannel(channel)
        .then(() => typing?.refresh())
        .catch((/** @type {unknown} */ error) => surface(error));
    },
  });
  // G-01: рейл серверов. Данные — только `ready()`, поэтому `load()` вызывается
  // после `auth ready` (ниже), а не на гейте входа. Выбор гильдии задаёт дерево:
  // `channels({guildId})` (C-02).
  const rail = createGuildRail({
    invoke,
    onSelect: (guild) => {
      // L-02-UI: выбранная гильдия — адресат live-проекций участников.
      dispatchView?.selectGuild(guild.id);
      void tree.load(guild.id);
    },
    // G-03: «+» открывает модалку join-by-invite.
    onAddServer: (opener) => join.open(opener),
  });
  /**
   * G-03: модалка join. Создаётся до `initActions`: её `data-action`
   * (`join-submit`/`join-cancel`/`dialog-close`) должны попасть в общий прямой
   * слушатель §8.1. Успех accept обновляет рейл и канал.
   */
  const join = createJoinServer({
    invoke,
    onAccepted: (guild) => rail.addGuild(guild),
  });

  /* ---------- L-02-UI: dispatch → экран без reload ---------- */
  // Подписка — только после `auth ready` (ниже), но view/store собираются здесь:
  // `data-dispatch-source` честен с первого кадра, а `handle()` доступен тестам.
  dispatchView = createDispatchView({
    body: document.body,
    memberPanelList: byId("member-panel-list"),
    memberOnline: byId("member-online"),
    treeHeading: document.querySelector(".tree-guild"),
    rail,
    chat,
  });
  dispatch = createDispatchBridge({
    listen: resolveDispatchListen(window),
    view: dispatchView,
    store: createDispatchStore(),
  });

  // Реакции идут через тот же `invoke`: один путь IPC и один сброс ошибки.
  configureReactionTransport(invoke, (error) => surface(error));  lists.set("chat", chat.list);

  // Композер: состояние empty/typing/disabled, лимит 2000 (K-10), reply.
  const composerForm = byId("composer");
  const composerInput = byId("composer-input");
  if (!composerForm || !composerInput) {
    throw new UIError("ELEMENT_MISSING", "в DOM нет #composer/#composer-input");
  }
  // P-03-UI (G6 §3.6): строка «Печатает…» над композером и исходящий typing.
  // Индикатор читает текущий канал из chat (бенч → пустая строка), исходящий
  // sender троттлит сам (`TYPING_RESEND_MS`, guard ≤5 каналов). Ошибки видны
  // текстом в том же регионе — без retry-цикла и без тела ответа/токена.
  typing = createTypingIndicator({
    host: composerForm,
    getChannelId: () => (chat.mode === "channel" ? chat.channelId : ""),
    onError: (error) => surface(error),
  });
  const typingSender = createTypingSender({
    invoke,
    onNotice: (text, kind) => typing?.notice(text, kind),
  });
  const composer = createComposer({
    form: composerForm,
    input: /** @type {HTMLTextAreaElement} */ (/** @type {unknown} */ (composerInput)),
    hint: byId("composer-hint"),
    onError: (error) => surface(error),
    onEscape: () => {
      const list = byId("chat-list");
      if (list && typeof /** @type {any} */ (list).focus === "function") {
        /** @type {any} */ (list).focus();
      }
    },
    onSubmit: (content) => {
      const nonce = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
      const replyTo = composer.replyTo;
      if (replyTo !== "") {
        void invoke("reply_message", { channelId: chat.channelId, content, replyTo }).catch((/** @type {unknown} */ error) => surface(error));
        composer.clearReply();
      } else {
        void chat.sendMessage({ content, nonce }, invoke);
      }
    },
    // P-03-UI: «печатает» отправляется только в реальном канале. Бенч
    // (`CHAT_ROWS`, синтетический «news») не имеет канала Discord — IPC нет.
    onType: () => {
      if (chat.mode !== "channel") return;
      void typingSender.note(chat.channelId);
    },
  });
  chat.bindComposer(composer);

  // Search overlay: 7 секций + 3 сортировки; B7b — серверный поиск через
  // `search_remote` по выбранному серверу, локальное окно — fallback/обзор.
  // QA-M2-03: на boot рейл пуст (плитки появляются только после `ready()`),
  // поэтому guildId нельзя снимать один раз. Источник — выбранная плитка
  // (`data-guild-id`, legacy `data-guild` только если атрибут есть),
  // читается на каждый запрос.
  const search = createSearchOverlay({
    chat,
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
    getGuildId: selectedGuildId,
    header: /** @type {HTMLInputElement | null} */ (byId("chat-search")),
    onError: (error) => surface(error),
  });
  const quickSwitcher = createQuickSwitcher({ router });

  const members = initMemberScreen((error) => surface(error));
  membersRef = members;
  lists.set("members", members.list);
  const emoji = initEmojiConsole({
    // §4/EMO-16: выбор эмодзи вставляет глиф в позицию курсора композера.
    onSelect: (glyph) => composer.insertText(glyph),
  });

  const settings = initSettingsConsole({ invoke });
  initActions({
    router,
    chat,
    members,
    emoji,
    settings,
    composer,
    search,
    net,
    join,
    invoke,
    onError: (error) => surface(error),
  });
  startMetrics({ onError: (error) => surface(error, "IPC_FAILED") });

  // Экран входа (RW-39/NET, ADR-2): idle → validating → ready | error.
  // Токен вводится только здесь и сразу уходит в `auth_login_token` → keyring.
  const auth = createAuthGate({
    onReady: () => {
      // После ready чат доступен: роутер на chat, фокус уходит из закрытого гейта.
      router.switchScreen("chat");
      const list = byId("chat-list");
      const target = /** @type {any} */ (list);
      if (target && typeof target.focus === "function") target.focus();
      // L-01: сетевой статус актуален только в активном приложении (после входа).
      net.start();
      // L-02-UI: подписка на dispatch — после ready, как у net/typing.
      void dispatch?.start();
      // G-01: список серверов — из READY; без входа грузить нечего.
      void rail.load();
    },
  });
  // Выход из аккаунта возвращает гейт: опрос ядра прекращается до следующего ready.
  // P-03-UI: блокировка отправки typing (401/403) и записи индикатора тоже
  // сбрасываются — иначе следующий вход унаследует чужое состояние.
  byId("auth-logout")?.addEventListener("click", () => {
    net.stop();
    void dispatch?.stop();
    typingSender.reset();
    typing?.clear();
  });

  const panelList = byId("member-panel-list");
  if (panelList) {
    renderMemberPanel({
      container: panelList,
      store: members.store,
      onlineCounter: byId("member-online"),
      onError: (error) => surface(error),
    }).catch((/** @type {unknown} */ error) => surface(error));
  }
  return { router, chat, tree, members, emoji, settings, composer, search, quickSwitcher, auth, net, rail, voice, join, typing, typingSender, dispatch };
}

try {
  app = boot();
} catch (error) {
  // Ошибка загрузки фич не должна молча оставить пустой экран: показываем оверлей.
  surface(error, "ELEMENT_MISSING");
}

/* ---------- автозамер и UI_SNAPSHOT ---------- */

async function autoBench() {
  if (!app) return;
  const tauri = /** @type {any} */ (window).__TAURI__;
  if (!tauri?.core?.invoke) return;
  try {
    const cfg = await tauri.core.invoke("spike_config");
    if (!cfg.autobench) return;
  } catch (error) {
    surface(error, "IPC_FAILED");
    return;
  }

  await sleep(1200);
  app.router.switchScreen("chat");
  const chatList = byId("chat-list");
  const chat = chatList ? await benchScroll(chatList, 5000) : null;
  app.router.switchScreen("members");
  await sleep(300);
  const memberList = byId("member-list");
  const members = memberList ? await benchScroll(memberList, 5000) : null;
  const payload = { frontend_ready_ms: frontendReadyMs, chat, members };
  console.log("[autobench]", payload);
  try {
    await tauri.core.invoke("report_bench", { payload: JSON.stringify(payload) });
  } catch (error) {
    surface(error, "IPC_FAILED");
  }
}

async function reportUiSnapshot() {
  if (!app) return;
  const tauri = /** @type {any} */ (window).__TAURI__;
  if (!tauri?.core?.invoke) return;
  const { router, chat, members, emoji, settings, search } = app;

  const actions = { screenClicks: 0, categoryClicks: false, toggleClick: false, listScroll: false };
  for (const button of document.querySelectorAll(".rail-btn")) {
    /** @type {HTMLElement} */ (button).click();
    if (button.classList.contains("active")) actions.screenClicks += 1;
  }
  const secondTab = document.querySelectorAll(".tab")[1];
  if (secondTab instanceof HTMLButtonElement) {
    secondTab.click();
    actions.categoryClicks = secondTab.classList.contains("active") && emoji.getActiveCategory() === 1;
  }
  const toggle = settings.toggles[0];
  if (toggle instanceof HTMLInputElement) {
    const oldValue = toggle.checked;
    toggle.click();
    actions.toggleClick = toggle.checked !== oldValue;
    toggle.click();
  }
  router.switchScreen("chat");
  await nextFrame();
  const oldScroll = chat.list.container.scrollTop;
  chat.list.scrollToIndex(500);
  actions.listScroll = chat.list.container.scrollTop !== oldScroll && chat.list.rows.size < CHAT_ROWS;
  const chatRenderedRows = chat.list.rows.size;
  chat.list.scrollToIndex(CHAT_ROWS - 1);
  // Меряем участников, когда экран показан и получил высоту (P1-1).
  router.switchScreen("members");
  await nextFrame();
  const memberRenderedRows = document.querySelectorAll("#member-list .member").length;
  router.switchScreen("chat");

  const snapshot = {
    screenCount: document.querySelectorAll(".screen").length,
    chatRows: CHAT_ROWS,
    memberRows: MEMBER_ROWS,
    emojiCategories: emoji.categories.length,
    settingsToggles: settings.toggles.length,
    chatRenderedRows,
    memberRenderedRows,
    // Оконная модель (ADR-1): материализованные строки в JS, а не только в DOM.
    chatLoadedRows: chat.store.stats().loadedRows,
    memberLoadedRows: members.store.stats().loadedRows,
    actions,
    // P3-02: новые панели (аддитивные поля, контракт RW-38 не меняют).
    memberGroups: MEMBER_GROUPS.length,
    searchSections: SEARCH_SECTIONS.length,
    searchMatches: search.matchesCount,
    composerState: app?.composer.state ?? "empty",
    unreadHeight: chat.unreadHeight,
    toolbarItems: chat.toolbar?.itemCount ?? 0,
    // L-02-UI: наблюдаемость live-канала без payload/секретов (PLAN §6).
    dispatch: app?.dispatch?.stats ?? { source: "absent" },
  };
  tauri.core.invoke("report_ui_snapshot", { payload: JSON.stringify(snapshot) }).catch(
    (/** @type {unknown} */ error) => {
      surface(error, "IPC_FAILED");
    },
  );
}

reportUiSnapshot().catch((/** @type {unknown} */ error) => {
  surface(error, "IPC_FAILED");
});
autoBench().catch((/** @type {unknown} */ error) => {
  surface(error, "IPC_FAILED");
});
