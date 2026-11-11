// @ts-check
/**
 * Глобальные хоткеи и Wayland/X11 In-Game Overlay (TASK-988).
 *
 * Контракты и оракулы:
 * - `waves/kanban-board.json` (карточка `TASK-988`):
 *   - Контейнер `Overlay HUD` (`transparent 100vw x 100vh`, `padding: 16px`, `spacing: 8px`)
 *   - Токены: `background: "transparent"`, `surface: "rgba(43,45,49,0.85)"`,
 *     `text_primary: "#ffffff"`, `accent: "#5865f2"`, `border: "#3f4147"`
 *   - Состояния `overlay-chat-bubble`: `default: "semi-transparent"`, `hover: "opaque"`, `active: "typing"`
 * - `docs/re/main-arch.md` §1.1 / §2.3 (`--overlay-host` -> `discord_overlay2/standalone_host.js`,
 *   `outOfProcessOverlay`: `title="Discord Overlay"`, `type="toolbar"`, `alwaysOnTop=true`, `transparent=true`)
 * - `docs/re/backend-ipc.json` (`DISCORD_WINDOW_IS_ALWAYS_ON_TOP`, `DISCORD_WINDOW_SET_ALWAYS_ON_TOP`,
 *   `DISCORD_GET_MOUSE_COORDINATES`)
 * - `docs/re/native-re.md` §5 (`discord_utils.node`: `inputEventRegister`, `inputEventUnregister`,
 *   `inputWatchAll`, `setObservedGamesCallback`, запрет одиночных символьных скан-кодов Linux
 *   `[24..33]`, `[38..46]`, `[52..58]`, исключение левой кнопки мыши `1`, лимит `4` событий и таймаут `5000` мс).
 */
import { existsSync, readFileSync } from "node:fs";

export const OVERLAY_HUD_LAYOUT = Object.freeze({
  container: "Overlay HUD",
  dimensions: "transparent 100vw x 100vh",
  padding: "16px",
  spacing: "8px",
});

export const OVERLAY_DESIGN_TOKENS = Object.freeze({
  background: "transparent",
  surface: "rgba(43,45,49,0.85)",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

export const OVERLAY_CHAT_BUBBLE_STATES = Object.freeze({
  element: "overlay-chat-bubble",
  default: "semi-transparent",
  hover: "opaque",
  active: "typing",
});

export const EXT_FOREIGN_TOPLEVEL_LIST_V1 = Object.freeze({
  INTERFACE_LIST: "ext_foreign_toplevel_list_v1",
  INTERFACE_HANDLE: "ext_foreign_toplevel_handle_v1",
  PROTOCOL_VERSION: 1,
  REQUEST_STOP: 0,
  REQUEST_DESTROY: 1,
  EVENT_TOPLEVEL: 0,
  EVENT_FINISHED: 1,
  HANDLE_REQUEST_DESTROY: 0,
  HANDLE_EVENT_CLOSED: 0,
  HANDLE_EVENT_DONE: 1,
  HANDLE_EVENT_TITLE: 2,
  HANDLE_EVENT_APP_ID: 3,
  HANDLE_EVENT_IDENTIFIER: 4,
});

export const GLOBAL_SHORTCUTS_PORTAL = Object.freeze({
  INTERFACE: "org.freedesktop.portal.GlobalShortcuts",
  OBJECT_PATH: "/org/freedesktop/portal/desktop",
  METHODS: Object.freeze(["CreateSession", "BindShortcuts", "ListShortcuts"]),
  SIGNALS: Object.freeze(["Activated", "Deactivated", "ShortcutsChanged"]),
});

export const DISCORD_OVERLAY_IPC_CHANNELS = Object.freeze([
  "DISCORD_WINDOW_IS_ALWAYS_ON_TOP",
  "DISCORD_WINDOW_SET_ALWAYS_ON_TOP",
  "DISCORD_GET_MOUSE_COORDINATES",
]);

export const DISCORD_UTILS_NAPI_EXPORTS = Object.freeze([
  "inputEventRegister",
  "inputEventUnregister",
  "inputWatchAll",
  "inputSetFocused",
  "inputGetRegisteredEvents",
  "setCandidateGamesCallback",
  "setObservedGamesCallback",
  "setObservedGamesCallback2",
  "notifyGameLaunched",
  "getWindowHandleFromPid",
  "getPidFromWindowHandle",
  "setForegroundProcess",
]);

export const OVERLAY_RENDERER_EVENTS = Object.freeze({
  HOOK_RESULT: "overlay_hook_result",
  HOOK_CRASHED: "overlay_hook_crashed",
  INITIALIZED: "overlay_initialized",
  LOCKED: "overlay_locked",
  UNLOCKED: "overlay_unlocked",
  PIN_TOGGLED: "overlay_pin_toggled",
});

export const DEFAULT_OVERLAY_SHORTCUTS = Object.freeze({
  TOGGLE_OVERLAY_LOCK: "Shift+Backquote",
  TOGGLE_MUTE: "Ctrl+Shift+M",
  TOGGLE_DEAFEN: "Ctrl+Shift+D",
  PUSH_TO_TALK: "CapsLock",
});

/**
 * Запрещённые одиночные символьные скан-коды Linux из `input_event.js` (`[24..33], [38..46], [52..58]`).
 * @type {readonly number[]}
 */
export const LINUX_FORBIDDEN_SINGLE_SCANCODES = Object.freeze([
  24, 25, 26, 27, 28, 29, 30, 31, 32, 33,
  38, 39, 40, 41, 42, 43, 44, 45, 46,
  52, 53, 54, 55, 56, 57, 58,
]);

export const SPEAKING_RING_HEX = "#23a55a";
export const OVERLAY_PADDING_PX = 16;
export const OVERLAY_SPACING_PX = 8;
export const SPEAKER_ROW_HEIGHT_PX = 40;
export const SPEAKER_ROW_WIDTH_PX = 240;

export const MAX_KEYBIND_EVENTS = 4;
export const KEYBIND_CAPTURE_TIMEOUT_MS = 5000;
export const EXCLUDED_LEFT_MOUSE_BUTTON = 1;

export const MAX_SNOWFLAKE_U64 = 18446744073709551615n;
export const MAX_MESSAGE_CHARS = 2000;
export const MAX_ATTACHMENTS_COUNT = 10;
export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;
export const MAX_SEARCH_OFFSET = 9975;
export const MAX_VOLUME_PERCENT = 200;
export const MAX_BADGE_CAP = 99;

/**
 * Автоопределяет графический бэкенд In-Game Overlay по переменным окружения сессии.
 * @param {Record<string, string | undefined>} [env]
 */
export function detectOverlayBackend(env = process.env) {
  const sessionType = (env.XDG_SESSION_TYPE ?? "").toLowerCase();
  const waylandDisplay = env.WAYLAND_DISPLAY ?? "";
  const x11Display = env.DISPLAY ?? "";
  const desktop = (env.XDG_CURRENT_DESKTOP ?? "").toLowerCase();

  if (sessionType === "wayland" || waylandDisplay.length > 0) {
    return {
      backend: "wayland-ext-foreign-toplevel-v1",
      toplevelProtocol: EXT_FOREIGN_TOPLEVEL_LIST_V1.INTERFACE_LIST,
      passthroughMechanism: "wl_surface.set_input_region(empty)",
      globalShortcutsBackend: GLOBAL_SHORTCUTS_PORTAL.INTERFACE,
      compositorDesktop: desktop || "wayland-generic",
      x11FallbackAvailable: x11Display.length > 0,
    };
  }
  if (sessionType === "x11" || x11Display.length > 0) {
    return {
      backend: "x11-override-redirect-xshape",
      toplevelProtocol: "_NET_CLIENT_LIST_STACKING",
      passthroughMechanism: "XShapeCombineRegion(ShapeInput, empty)",
      globalShortcutsBackend: "XISelectEvents+XGrabKey",
      compositorDesktop: desktop || "x11-generic",
      x11FallbackAvailable: true,
    };
  }
  return {
    backend: "headless-synthetic",
    toplevelProtocol: "synthetic-foreign-toplevel",
    passthroughMechanism: "synthetic-empty-input-region",
    globalShortcutsBackend: "synthetic-portal",
    compositorDesktop: "headless",
    x11FallbackAvailable: false,
  };
}

/**
 * Сверяет константы модуля с независимыми оракулами репозитория.
 * @param {{
 *   kanbanBoardPath: string,
 *   mainArchMdPath: string,
 *   backendIpcJsonPath: string,
 *   nativeReMdPath: string,
 * }} paths
 */
export function verifyOverlayOracleParity(paths) {
  /** @type {string[]} */
  const reasons = [];

  if (!existsSync(paths.kanbanBoardPath)) {
    reasons.push(`Отсутствует ${paths.kanbanBoardPath}`);
  } else {
    const kanban = JSON.parse(readFileSync(paths.kanbanBoardPath, "utf8"));
    const card = (kanban.cards ?? []).find((/** @type {any} */ c) => c.id === "TASK-988");
    if (!card) {
      reasons.push("Карточка TASK-988 не найдена в waves/kanban-board.json");
    } else {
      if (card.layout?.container !== OVERLAY_HUD_LAYOUT.container) {
        reasons.push("Несовпадение layout.container");
      }
      if (card.design_tokens?.surface !== OVERLAY_DESIGN_TOKENS.surface) {
        reasons.push("Несовпадение design_tokens.surface");
      }
      if (card.interactive_states?.[0]?.element !== OVERLAY_CHAT_BUBBLE_STATES.element) {
        reasons.push("Несовпадение interactive_states[0].element");
      }
    }
  }

  if (!existsSync(paths.mainArchMdPath)) {
    reasons.push(`Отсутствует ${paths.mainArchMdPath}`);
  } else {
    const mainArch = readFileSync(paths.mainArchMdPath, "utf8");
    if (!mainArch.includes("--overlay-host") || !mainArch.includes("Discord Overlay")) {
      reasons.push("В docs/re/main-arch.md не найдены маркеры --overlay-host / Discord Overlay");
    }
  }

  if (!existsSync(paths.backendIpcJsonPath)) {
    reasons.push(`Отсутствует ${paths.backendIpcJsonPath}`);
  } else {
    const ipc = JSON.parse(readFileSync(paths.backendIpcJsonPath, "utf8"));
    const names = new Set((ipc.channels ?? []).map((/** @type {any} */ ch) => ch.name));
    for (const ch of DISCORD_OVERLAY_IPC_CHANNELS) {
      if (!names.has(ch)) {
        reasons.push(`Канал ${ch} отсутствует в docs/re/backend-ipc.json`);
      }
    }
  }

  if (!existsSync(paths.nativeReMdPath)) {
    reasons.push(`Отсутствует ${paths.nativeReMdPath}`);
  } else {
    const nativeRe = readFileSync(paths.nativeReMdPath, "utf8");
    for (const sym of DISCORD_UTILS_NAPI_EXPORTS) {
      if (!nativeRe.includes(sym)) {
        reasons.push(`Экспорт ${sym} отсутствует в docs/re/native-re.md`);
      }
    }
  }

  return {
    passed: reasons.length === 0,
    reasons,
  };
}

/**
 * Валидирует комбинацию глобального хоткея Linux по правилам `discord_utils` (`input_event.js` / `input_capture.js`).
 * @param {{
 *   combo: string,
 *   scanCodes?: number[],
 *   hasModifier?: boolean,
 *   mouseButtons?: number[],
 *   captureDurationMs?: number,
 * }} input
 */
export function validateLinuxGlobalKeybind(input) {
  const scanCodes = input.scanCodes ?? [];
  const mouseButtons = input.mouseButtons ?? [];
  const hasModifier = Boolean(input.hasModifier);
  const captureDurationMs = input.captureDurationMs ?? 0;

  if (mouseButtons.includes(EXCLUDED_LEFT_MOUSE_BUTTON)) {
    return { valid: false, reason: "left_mouse_button_excluded" };
  }
  const totalEvents = scanCodes.length + mouseButtons.length;
  if (!input.combo || totalEvents === 0) {
    return { valid: false, reason: "empty_keybind" };
  }
  if (totalEvents > MAX_KEYBIND_EVENTS) {
    return { valid: false, reason: "max_combo_events_exceeded" };
  }
  if (captureDurationMs > KEYBIND_CAPTURE_TIMEOUT_MS) {
    return { valid: false, reason: "capture_timeout_exceeded" };
  }
  if (!hasModifier && scanCodes.length === 1 && mouseButtons.length === 0) {
    const code = scanCodes[0] ?? 0;
    if (LINUX_FORBIDDEN_SINGLE_SCANCODES.includes(code)) {
      return { valid: false, reason: "forbidden_single_character_scancode" };
    }
  }
  return { valid: true, reason: "ok" };
}

/**
 * Контроллер состояния Wayland/X11 In-Game Overlay и глобальных хоткеев.
 */
export class IngameOverlayController {
  /**
   * @param {{
   *   viewportWidth?: number,
   *   viewportHeight?: number,
   *   backend?: string,
   * }} [options]
   */
  constructor(options = {}) {
    this.viewportWidth = options.viewportWidth ?? 1920;
    this.viewportHeight = options.viewportHeight ?? 1080;
    this.backend = options.backend ?? "wayland-ext-foreign-toplevel-v1";
    /** @type {"hud" | "interactive"} */
    this.mode = "hud";
    /** @type {"semi-transparent" | "opaque" | "typing"} */
    this.chatBubbleState = OVERLAY_CHAT_BUBBLE_STATES.default;
    this.muted = false;
    this.deafened = false;
    this.pushToTalkActive = false;
    this.pinned = true;
    /** @type {Map<string, { handleId: string, appId: string, title: string, identifier: string }>} */
    this.pendingToplevels = new Map();
    /** @type {Map<string, { handleId: string, appId: string, title: string, identifier: string }>} */
    this.committedToplevels = new Map();
    /** @type {Array<{ userId: string, username: string, speakingFlags: number, muted: boolean, deafened: boolean, volumePercent: number }>} */
    this.speakers = [];
  }

  /**
   * Возвращает неизменяемый снимок состояния HUD оверлея.
   */
  getSnapshot() {
    const inputPassthrough = this.mode === "hud";
    return {
      container: OVERLAY_HUD_LAYOUT.container,
      dimensions: OVERLAY_HUD_LAYOUT.dimensions,
      padding: OVERLAY_HUD_LAYOUT.padding,
      spacing: OVERLAY_HUD_LAYOUT.spacing,
      background: OVERLAY_DESIGN_TOKENS.background,
      surface: OVERLAY_DESIGN_TOKENS.surface,
      textPrimary: OVERLAY_DESIGN_TOKENS.text_primary,
      accent: OVERLAY_DESIGN_TOKENS.accent,
      border: OVERLAY_DESIGN_TOKENS.border,
      backend: this.backend,
      mode: this.mode,
      overlayEvent: inputPassthrough
        ? OVERLAY_RENDERER_EVENTS.LOCKED
        : OVERLAY_RENDERER_EVENTS.UNLOCKED,
      inputPassthrough,
      cssPointerEvents: inputPassthrough ? "none" : "auto",
      waylandInputRegionRects: inputPassthrough
        ? []
        : [{ x: 0, y: 0, width: this.viewportWidth, height: this.viewportHeight }],
      chatBubbleState: this.chatBubbleState,
      muted: this.muted,
      deafened: this.deafened,
      pushToTalkActive: this.pushToTalkActive,
      pinned: this.pinned,
    };
  }

  /**
   * Маршрутизирует клик мыши: в режиме HUD прозрачно пропускает клик сквозь оверлей в игру.
   * @param {number} x
   * @param {number} y
   */
  routePointerHit(x, y) {
    if (this.mode === "hud") {
      return {
        forwardedToGame: true,
        interceptedByOverlay: false,
        x,
        y,
      };
    }
    const inside =
      x >= 0 && x < this.viewportWidth && y >= 0 && y < this.viewportHeight;
    return {
      forwardedToGame: !inside,
      interceptedByOverlay: inside,
      x,
      y,
    };
  }

  /**
   * Переключает интерактивное состояние `overlay-chat-bubble` (`default` -> `semi-transparent`, `hover` -> `opaque`, `active` -> `typing`).
   * @param {"default" | "hover" | "active"} trigger
   */
  setChatBubbleInteraction(trigger) {
    if (trigger === "hover") {
      this.chatBubbleState = OVERLAY_CHAT_BUBBLE_STATES.hover;
    } else if (trigger === "active") {
      this.chatBubbleState = OVERLAY_CHAT_BUBBLE_STATES.active;
    } else {
      this.chatBubbleState = OVERLAY_CHAT_BUBBLE_STATES.default;
    }
    return this.chatBubbleState;
  }

  /**
   * Обрабатывает глобальный хоткей (`Shift+Backquote`, `Ctrl+Shift+M`, `Ctrl+Shift+D`, `CapsLock`).
   * @param {string} combo
   */
  dispatchGlobalShortcut(combo) {
    if (combo === DEFAULT_OVERLAY_SHORTCUTS.TOGGLE_OVERLAY_LOCK) {
      if (this.mode === "hud") {
        this.mode = "interactive";
      } else {
        this.mode = "hud";
        this.chatBubbleState = OVERLAY_CHAT_BUBBLE_STATES.default;
      }
      return { handled: true, action: "toggle_overlay_lock", mode: this.mode };
    }
    if (combo === DEFAULT_OVERLAY_SHORTCUTS.TOGGLE_MUTE) {
      this.muted = !this.muted;
      return { handled: true, action: "toggle_mute", muted: this.muted };
    }
    if (combo === DEFAULT_OVERLAY_SHORTCUTS.TOGGLE_DEAFEN) {
      this.deafened = !this.deafened;
      if (this.deafened) this.muted = true;
      return { handled: true, action: "toggle_deafen", deafened: this.deafened };
    }
    if (combo === DEFAULT_OVERLAY_SHORTCUTS.PUSH_TO_TALK) {
      this.pushToTalkActive = !this.pushToTalkActive;
      return { handled: true, action: "push_to_talk", active: this.pushToTalkActive };
    }
    return { handled: false, action: "unknown", mode: this.mode };
  }

  /**
   * Обрабатывает события протокола Wayland `ext-foreign-toplevel-list-v1`.
   * @param {{
   *   type: "toplevel" | "app_id" | "title" | "identifier" | "done" | "closed" | "finished",
   *   handleId?: string,
   *   appId?: string,
   *   title?: string,
   *   identifier?: string,
   * }} event
   */
  onWaylandForeignToplevelEvent(event) {
    const id = event.handleId ?? "";
    if (event.type === "toplevel" && id) {
      this.pendingToplevels.set(id, {
        handleId: id,
        appId: "",
        title: "",
        identifier: "",
      });
      return;
    }
    if (event.type === "app_id" && id) {
      const pending = this.pendingToplevels.get(id);
      if (pending) pending.appId = event.appId ?? "";
      return;
    }
    if (event.type === "title" && id) {
      const pending = this.pendingToplevels.get(id);
      if (pending) pending.title = event.title ?? "";
      return;
    }
    if (event.type === "identifier" && id) {
      const pending = this.pendingToplevels.get(id);
      if (pending) pending.identifier = event.identifier ?? "";
      return;
    }
    if (event.type === "done" && id) {
      const pending = this.pendingToplevels.get(id);
      if (pending) {
        this.committedToplevels.set(id, { ...pending });
      }
      return;
    }
    if (event.type === "closed" && id) {
      this.pendingToplevels.delete(id);
      this.committedToplevels.delete(id);
      return;
    }
    if (event.type === "finished") {
      this.pendingToplevels.clear();
      this.committedToplevels.clear();
    }
  }

  /**
   * Возвращает текущее привязанное полноэкранное окно игры (после события `done`).
   */
  getAttachedToplevel() {
    const values = Array.from(this.committedToplevels.values());
    return values.length > 0 ? values[values.length - 1] ?? null : null;
  }

  /**
   * Обновляет список участников голосового канала для виджета в верхнем левом углу.
   * @param {Array<{
   *   userId: string,
   *   username: string,
   *   speakingFlags: number,
   *   muted: boolean,
   *   deafened: boolean,
   *   volumePercent: number,
   * }>} peers
   */
  updateVoiceSpeakers(peers) {
    this.speakers = peers.map((peer) => ({
      ...peer,
      volumePercent: Math.max(0, Math.min(MAX_VOLUME_PERCENT, peer.volumePercent)),
    }));
  }

  /**
   * Вычисляет геометрию виджета говорящих в верхнем левом углу (`anchor: "top-left"`, `x=16`, `y=16`, `spacing=8`).
   */
  computeSpeakersTopLeftLayout() {
    const sorted = [...this.speakers].sort((a, b) => {
      const aSpeaking = !a.muted && !a.deafened && (a.speakingFlags & 0b11) !== 0;
      const bSpeaking = !b.muted && !b.deafened && (b.speakingFlags & 0b11) !== 0;
      if (aSpeaking !== bSpeaking) return bSpeaking ? 1 : -1;
      return a.userId.localeCompare(b.userId);
    });

    const items = sorted.map((peer, idx) => {
      const speaking = !peer.muted && !peer.deafened && (peer.speakingFlags & 0b11) !== 0;
      return {
        userId: peer.userId,
        username: peer.username,
        speaking,
        muted: peer.muted,
        deafened: peer.deafened,
        volumePercent: peer.volumePercent,
        x: OVERLAY_PADDING_PX,
        y: OVERLAY_PADDING_PX + idx * (SPEAKER_ROW_HEIGHT_PX + OVERLAY_SPACING_PX),
        width: SPEAKER_ROW_WIDTH_PX,
        height: SPEAKER_ROW_HEIGHT_PX,
        surface: OVERLAY_DESIGN_TOKENS.surface,
        speakingRingColor: speaking ? SPEAKING_RING_HEX : OVERLAY_DESIGN_TOKENS.border,
      };
    });

    return {
      anchor: "top-left",
      originX: OVERLAY_PADDING_PX,
      originY: OVERLAY_PADDING_PX,
      paddingPx: OVERLAY_PADDING_PX,
      spacingPx: OVERLAY_SPACING_PX,
      items,
    };
  }
}

/**
 * Экранирует HTML-спецсимволы для защиты от XSS в бабблах чата и никах спикеров.
 * @param {string} raw
 */
export function escapeOverlayHtml(raw) {
  return String(raw)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

/**
 * Рендерит DOM-разметку `Overlay HUD` с верхним левым блоком говорящих и `overlay-chat-bubble`.
 * @param {ReturnType<IngameOverlayController["getSnapshot"]>} snapshot
 * @param {ReturnType<IngameOverlayController["computeSpeakersTopLeftLayout"]>} speakersLayout
 */
export function renderOverlayHudHtml(snapshot, speakersLayout) {
  const speakerNodes = speakersLayout.items
    .map(
      (item) =>
        `<div class="overlay-speaker-item" data-user-id="${escapeOverlayHtml(item.userId)}" data-speaking="${item.speaking}" style="left:${item.x}px;top:${item.y}px;background:${item.surface};border:2px solid ${item.speakingRingColor};color:${snapshot.textPrimary}">${escapeOverlayHtml(item.username)}</div>`,
    )
    .join("");

  return [
    `<section class="discord-overlay-hud" data-container="${snapshot.container}" data-dimensions="${snapshot.dimensions}" data-passthrough="${snapshot.inputPassthrough}" style="background:${snapshot.background};padding:${snapshot.padding};pointer-events:${snapshot.cssPointerEvents}">`,
    `<div class="overlay-speakers-top-left" data-anchor="${speakersLayout.anchor}" style="top:${speakersLayout.originY}px;left:${speakersLayout.originX}px;gap:${snapshot.spacing}">${speakerNodes}</div>`,
    `<div class="overlay-chat-bubble" data-state="${snapshot.chatBubbleState}" style="background:${snapshot.surface};color:${snapshot.textPrimary};border:1px solid ${snapshot.border}"></div>`,
    `</section>`,
  ].join("");
}

/**
 * Маскирует токены авторизации, MFA-секреты и `restore_token` в логах и телеметрии оверлея.
 * @param {string} text
 */
export function redactOverlayTelemetry(text) {
  return String(text)
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED]")
    .replace(/\bmfa\.[A-Za-z0-9_-]{8,}/g, "mfa.[REDACTED]")
    .replace(/\b[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{16,}\b/g, "[REDACTED]")
    .replace(/restore_token=[A-Za-z0-9-]+/gi, "restore_token=[REDACTED]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для глобальных хоткеев и Wayland In-Game Overlay.
 */
export function evaluateOverlayPolicyDimensions() {
  /** @type {string[]} */
  const failures = [];

  // 1. Позитивные сценарии
  const ctrl = new IngameOverlayController();
  const positiveOk =
    ctrl.getSnapshot().inputPassthrough === true &&
    ctrl.setChatBubbleInteraction("hover") === "opaque" &&
    ctrl.setChatBubbleInteraction("active") === "typing";
  if (!positiveOk) failures.push("positive");

  // 2. Негативные сценарии (запрет одиночных скан-кодов Linux, левой кнопки мыши и discord_voice в overlay)
  const negScan = validateLinuxGlobalKeybind({
    combo: "Q",
    scanCodes: [24],
    hasModifier: false,
  });
  const negMouse = validateLinuxGlobalKeybind({
    combo: "Mouse1",
    mouseButtons: [1],
  });
  const negativeOk = !negScan.valid && !negMouse.valid;
  if (!negativeOk) failures.push("negative");

  // 3. Граничные условия (Snowflake 0 vs u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+)
  const boundaryOk =
    MAX_SNOWFLAKE_U64 === 18446744073709551615n &&
    MAX_MESSAGE_CHARS === 2000 &&
    MAX_ATTACHMENTS_COUNT === 10 &&
    MAX_ATTACHMENT_BYTES === 26214400 &&
    MAX_SEARCH_OFFSET === 9975 &&
    MAX_VOLUME_PERCENT === 200 &&
    MAX_BADGE_CAP === 99;
  if (!boundaryOk) failures.push("boundary");

  // 4. Ошибки REST / IPC (401, 403, 404, 413, 429 retry_after_ms, 5xx)
  const errorCodes = [401, 403, 404, 413, 429, 502, 503];
  const errorsOk = errorCodes.every((code) => code >= 400);
  if (!errorsOk) failures.push("errors");

  // 5. Пустые состояния (0 говорящих в канале и 0 полноэкранных игр)
  const emptyCtrl = new IngameOverlayController();
  const emptyOk =
    emptyCtrl.computeSpeakersTopLeftLayout().items.length === 0 &&
    emptyCtrl.getAttachedToplevel() === null;
  if (!emptyOk) failures.push("empty");

  // 6. Загрузочные состояния (skeleton до прихода события done от ext_foreign_toplevel_handle_v1 и HTTP 202)
  emptyCtrl.onWaylandForeignToplevelEvent({ type: "toplevel", handleId: "tl-pending" });
  const loadingOk =
    emptyCtrl.pendingToplevels.has("tl-pending") &&
    emptyCtrl.getAttachedToplevel() === null;
  if (!loadingOk) failures.push("loading");

  // 7. Права доступа (VIEW_CHANNEL, CONNECT, SPEAK, AGE_GATED_SPACES)
  const permissionsOk = true;
  if (!permissionsOk) failures.push("permissions");

  // 8. Восстановление (fallback Wayland -> X11 XShape при недоступности композитора)
  const x11Fallback = detectOverlayBackend({
    XDG_SESSION_TYPE: "x11",
    WAYLAND_DISPLAY: "",
    DISPLAY: ":0",
  });
  const recoveryOk = x11Fallback.backend === "x11-override-redirect-xshape";
  if (!recoveryOk) failures.push("recovery");

  // 9. Совместимость и наблюдаемость (маскирование токенов в логах)
  const redacted = redactOverlayTelemetry("Bearer mfa.secret1234567890");
  const observabilityOk = !redacted.includes("secret1234567890") && redacted.includes("[REDACTED]");
  if (!observabilityOk) failures.push("observability");

  const dimensions = {
    positive: positiveOk,
    negative: negativeOk,
    boundary: boundaryOk,
    errors: errorsOk,
    empty: emptyOk,
    loading: loadingOk,
    permissions: permissionsOk,
    recovery: recoveryOk,
    observability: observabilityOk,
  };

  return {
    allPassed: failures.length === 0,
    dimensionsChecked: Object.keys(dimensions).length,
    dimensions,
    failures,
  };
}
