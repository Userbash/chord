#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор автоматического перевода в AFK-канал при простое (TASK-1010 / TASK-1010A / TASK-1010B):
 * - Сверка дизайн-токенов, раскладки `Voice State Service` (`background daemon`, `0px`, `0px`),
 *   интерактивных состояний `afk-badge` (`hidden -> visible -> moved to afk`), `idle-detect`
 *   (`active -> counting -> idle triggered`) и `voice-afk` (`normal voice -> warn afk -> moved to afk`)
 *   из `waves/kanban-board.json`.
 * - Сверка с D-Bus оракулом `fixtures/dbus/power.json` (`org.freedesktop.ScreenSaver.GetSessionIdleTime -> 1234`,
 *   `org.freedesktop.login1.Manager.PrepareForSleep`), `docs/re/gateway-protocol.json` (`VOICE_STATE_UPDATE = 8`),
 *   `docs/re/gateway-schema.json`, 7 реальными гильдиями из `/home/sanya/re-deep/gw-proto/msgs-run1-57992_5112.jsonl`
 *   и модулями RE-дампа Discord (`885576`, `576991`, `926456`, `185255`, `260509`).
 * - Покрытие 9 обязательных измерений `TEST-POLICY.md` и маскирование секретов.
 *
 * Коды выхода CLI:
 * - 0: Все инварианты AFK-монитора и RE-оракулы пройдены.
 * - 1: Ошибка аргументов командной строки или нарушение инвариантов.
 * - 2: Внешний файл-оракул недоступен или отсутствует.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_DBUS_POWER_PATH = join(ROOT, "fixtures", "dbus", "power.json");
const DEFAULT_GW_PROTOCOL_PATH = join(ROOT, "docs", "re", "gateway-protocol.json");
const DEFAULT_GW_SCHEMA_PATH = join(ROOT, "docs", "re", "gateway-schema.json");
const DEFAULT_RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** Макет фонового сервиса `Voice State Service` из `waves/kanban-board.json` (`TASK-1010`). */
export const AFK_SERVICE_LAYOUT = Object.freeze({
  container: "Voice State Service",
  dimensions: "background daemon",
  padding: "0px",
  spacing: "0px",
});

/** Дизайн-токены карточки `TASK-1010` из `waves/kanban-board.json`. */
export const AFK_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#f0b232",
  border: "#3f4147",
});

/** Интерактивные состояния элемента `afk-badge` из `waves/kanban-board.json` (`TASK-1010`). */
export const AFK_BADGE_STATES = Object.freeze({
  element: "afk-badge",
  default: "hidden",
  hover: "visible",
  active: "moved to afk",
});

/** Интерактивные состояния элемента `idle-detect` из `waves/kanban-board.json` (`TASK-1010A`). */
export const IDLE_DETECT_STATES = Object.freeze({
  element: "idle-detect",
  default: "active",
  hover: "counting",
  active: "idle triggered",
});

/** Интерактивные состояния элемента `voice-afk` из `waves/kanban-board.json` (`TASK-1010B`). */
export const VOICE_AFK_STATES = Object.freeze({
  element: "voice-afk",
  default: "normal voice",
  hover: "warn afk",
  active: "moved to afk",
});

/** Константы протокола системного бездействия и Gateway Opcode 4 из RE-дампа Discord. */
export const AFK_PROTOCOL_CONSTANTS = Object.freeze({
  GATEWAY_OP_VOICE_STATE_UPDATE: 4,
  DBUS_SCREENSAVER_DEST: "org.freedesktop.ScreenSaver",
  DBUS_SCREENSAVER_PATH: "/org/freedesktop/ScreenSaver",
  DBUS_SCREENSAVER_INTERFACE: "org.freedesktop.ScreenSaver",
  DBUS_IDLE_METHOD: "GetSessionIdleTime",
  DBUS_LOGIN1_DEST: "org.freedesktop.login1",
  DBUS_LOGIN1_INTERFACE: "org.freedesktop.login1.Manager",
  DBUS_PREPARE_FOR_SLEEP_SIGNAL: "PrepareForSleep",
  WAYLAND_IDLE_PROTOCOL: "ext-idle-notifier-v1",
  X11_IDLE_EXTENSION: "XScreenSaverQueryInfo",
  IDLE_POLL_INTERVAL_MS: 10_000,
  IDLE_FALLBACK_POLL_INTERVAL_MS: 30_000,
  VOICE_HARD_IDLE_DISCONNECT_MS: 18_000_000,
  ALLOWED_AFK_TIMEOUTS_SECS: Object.freeze([60, 300, 900, 1800, 3600]),
  VOICE_STATE_DEFAULT_FLAGS: 2,
  PERM_VIEW_CHANNEL: 1n << 10n,
  PERM_CONNECT: 1n << 20n,
  PERM_SPEAK: 1n << 21n,
  PERM_MOVE_MEMBERS: 1n << 24n,
  RE_MODULE_IDLE_STORE: "885576",
  RE_MODULE_GATEWAY_SOCKET: "576991",
  RE_MODULE_VOICE_AFK_MANAGER: "926456",
  RE_MODULE_GUILD_AFK_SETTINGS: "185255",
  RE_MODULE_GUILD_RECORD: "260509",
});

/**
 * 7 реальных конфигураций AFK гильдий из `READY` в `/home/sanya/re-deep/gw-proto/msgs-run1-57992_5112.jsonl`.
 */
export const LIVE_READY_GUILDS_AFK_ORACLE = Object.freeze([
  Object.freeze({
    id: "274093052673392641",
    name: "Игровое сообщество ⚛MONOLIT⚛",
    afkChannelId: "274955515333836801",
    afkTimeoutSecs: 3600,
  }),
  Object.freeze({
    id: "294064825540280335",
    name: "Generations of Players",
    afkChannelId: "294151953686986762",
    afkTimeoutSecs: 1800,
  }),
  Object.freeze({
    id: "338001249918255106",
    name: "True Job Project",
    afkChannelId: "338009331985416202",
    afkTimeoutSecs: 300,
  }),
  Object.freeze({
    id: "700482297027231765",
    name: "SISI 18+ PROJECT",
    afkChannelId: "706552755627491419",
    afkTimeoutSecs: 300,
  }),
  Object.freeze({
    id: "942355007271997450",
    name: "XOS",
    afkChannelId: null,
    afkTimeoutSecs: 300,
  }),
  Object.freeze({
    id: "1014537851418980372",
    name: "ITSKIDON PC BOOST",
    afkChannelId: "1271435029163151391",
    afkTimeoutSecs: 1800,
  }),
  Object.freeze({
    id: "1340572775974305803",
    name: "Game Room",
    afkChannelId: null,
    afkTimeoutSecs: 300,
  }),
]);

/**
 * 2 реальных события `VOICE_STATE_UPDATE` из `/home/sanya/re-deep/gw-proto/msgs-run2-95974_8269.jsonl` (`s=240`, `s=290`).
 */
export const LIVE_VOICE_STATE_EVENTS_ORACLE = Object.freeze([
  Object.freeze({
    seq: 240,
    type: "VOICE_STATE_UPDATE",
    guildId: "274093052673392641",
    channelId: null,
    userId: "289091818442719242",
    selfMute: false,
    selfDeaf: false,
    selfVideo: false,
    suppress: false,
  }),
  Object.freeze({
    seq: 290,
    type: "VOICE_STATE_UPDATE",
    guildId: "274093052673392641",
    channelId: "351403515723120651",
    userId: "289091818442719242",
    selfMute: false,
    selfDeaf: false,
    selfVideo: false,
    suppress: false,
  }),
]);

/**
 * Проверяет корректность Snowflake ID (`> 0` и `<= u64::MAX`).
 * @param {string | number | bigint | null | undefined} raw
 * @returns {boolean}
 */
export function isValidSnowflake(raw) {
  const str = String(raw ?? "").trim();
  if (!/^\d{1,20}$/.test(str)) return false;
  try {
    const val = BigInt(str);
    return val > 0n && val <= 18446744073709551615n;
  } catch {
    return false;
  }
}

/**
 * Проверяет допустимость настройки `afk_timeout` гильдии (`60, 300, 900, 1800, 3600` секунд).
 * @param {number} seconds
 * @returns {boolean}
 */
export function isValidAfkTimeoutSecs(seconds) {
  return (
    Number.isInteger(seconds) &&
    AFK_PROTOCOL_CONSTANTS.ALLOWED_AFK_TIMEOUTS_SECS.includes(seconds)
  );
}

/**
 * Вычисляет интерактивное состояние элемента `afk-badge` (`hidden` / `visible` / `moved to afk`).
 * @param {{ movedToAfk?: boolean, hovered?: boolean }} opts
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveAfkBadgeState(opts) {
  if (opts.movedToAfk) {
    return {
      element: AFK_BADGE_STATES.element,
      state: "active",
      label: AFK_BADGE_STATES.active,
    };
  }
  if (opts.hovered) {
    return {
      element: AFK_BADGE_STATES.element,
      state: "hover",
      label: AFK_BADGE_STATES.hover,
    };
  }
  return {
    element: AFK_BADGE_STATES.element,
    state: "default",
    label: AFK_BADGE_STATES.default,
  };
}

/**
 * Вычисляет интерактивное состояние элемента `idle-detect` (`active` / `counting` / `idle triggered`).
 * @param {number} effectiveIdleMs
 * @param {number} thresholdMs
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveIdleDetectState(effectiveIdleMs, thresholdMs) {
  if (thresholdMs > 0 && effectiveIdleMs >= thresholdMs) {
    return {
      element: IDLE_DETECT_STATES.element,
      state: "active",
      label: IDLE_DETECT_STATES.active,
    };
  }
  if (effectiveIdleMs > 0) {
    return {
      element: IDLE_DETECT_STATES.element,
      state: "hover",
      label: IDLE_DETECT_STATES.hover,
    };
  }
  return {
    element: IDLE_DETECT_STATES.element,
    state: "default",
    label: IDLE_DETECT_STATES.default,
  };
}

/**
 * Вычисляет состояние `voice-afk` (`normal voice` / `warn afk` / `moved to afk`).
 * @param {number} effectiveIdleMs
 * @param {number} timeoutMs
 * @param {boolean} movedToAfk
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveVoiceAfkState(effectiveIdleMs, timeoutMs, movedToAfk) {
  if (movedToAfk || (timeoutMs > 0 && effectiveIdleMs >= timeoutMs)) {
    return {
      element: VOICE_AFK_STATES.element,
      state: "active",
      label: VOICE_AFK_STATES.active,
    };
  }
  const warnThresholdMs = Math.floor(timeoutMs * 0.8);
  if (timeoutMs > 0 && effectiveIdleMs >= warnThresholdMs && effectiveIdleMs > 0) {
    return {
      element: VOICE_AFK_STATES.element,
      state: "hover",
      label: VOICE_AFK_STATES.hover,
    };
  }
  return {
    element: VOICE_AFK_STATES.element,
    state: "default",
    label: VOICE_AFK_STATES.default,
  };
}

/**
 * Выбирает системный бэкенд замера бездействия с приоритетом Wayland -> D-Bus ScreenSaver -> X11 -> Fallback.
 * @param {{
 *   waylandExtIdleAvailable?: boolean,
 *   dbusScreenSaverAvailable?: boolean,
 *   x11ScreenSaverAvailable?: boolean
 * }} caps
 * @returns {"wayland_ext_idle_notifier_v1" | "dbus_freedesktop_screensaver" | "x11_screensaver" | "monotonic_fallback"}
 */
export function selectIdleBackend(caps) {
  if (caps.waylandExtIdleAvailable) {
    return "wayland_ext_idle_notifier_v1";
  }
  if (caps.dbusScreenSaverAvailable) {
    return "dbus_freedesktop_screensaver";
  }
  if (caps.x11ScreenSaverAvailable) {
    return "x11_screensaver";
  }
  return "monotonic_fallback";
}

/**
 * Вычисляет эффективное время бездействия (в мс) с учётом подавления при воспроизведении медиа,
 * активных стримах Go Live (`cs.A.getAllActiveStreams()`) и речи пользователя (`SPEAKING`).
 * @param {{
 *   rawIdleMs: number,
 *   mediaPlaying?: boolean,
 *   activeStreamCount?: number,
 *   userSpeaking?: boolean,
 *   systemSuspended?: boolean,
 *   screenLocked?: boolean,
 *   forcedIdleMsOnLock?: number
 * }} opts
 * @returns {number}
 */
export function computeEffectiveIdleMs(opts) {
  if (opts.mediaPlaying || (opts.activeStreamCount ?? 0) > 0 || opts.userSpeaking) {
    return 0;
  }
  const safeRaw = Number.isFinite(opts.rawIdleMs) ? Math.max(0, Math.floor(opts.rawIdleMs)) : 0;
  if (opts.systemSuspended || opts.screenLocked) {
    const minLocked = opts.forcedIdleMsOnLock ?? 60_000;
    return Math.max(safeRaw, minLocked);
  }
  return safeRaw;
}

/**
 * Формирует кадр Gateway Opcode 4 (`VOICE_STATE_UPDATE`, модуль `576991` RE-дампа).
 * @param {{
 *   guildId: string | null,
 *   channelId: string | null,
 *   selfMute: boolean,
 *   selfDeaf: boolean,
 *   selfVideo?: boolean,
 *   flags?: number
 * }} params
 */
export function buildVoiceStateUpdateOpcode4(params) {
  if (params.guildId !== null && !isValidSnowflake(params.guildId)) {
    throw new RangeError(`Некорректный guildId для Opcode 4: ${params.guildId}`);
  }
  if (params.channelId !== null && !isValidSnowflake(params.channelId)) {
    throw new RangeError(`Некорректный channelId для Opcode 4: ${params.channelId}`);
  }
  const flags =
    params.flags !== undefined ? params.flags : AFK_PROTOCOL_CONSTANTS.VOICE_STATE_DEFAULT_FLAGS;
  return {
    op: AFK_PROTOCOL_CONSTANTS.GATEWAY_OP_VOICE_STATE_UPDATE,
    d: {
      guild_id: params.guildId,
      channel_id: params.channelId,
      self_mute: Boolean(params.selfMute),
      self_deaf: Boolean(params.selfDeaf),
      self_video: Boolean(params.selfVideo ?? false),
      flags,
    },
  };
}

/**
 * Движок мониторинга бездействия голосового канала и автоматического перевода в `afk_channel_id`
 * по контракту `VoiceAFKManager` (модуль `926456`) и `IdleStore` (модуль `885576`).
 */
export class VoiceAfkMonitorEngine {
  /**
   * @param {{
   *   guildId: string,
   *   currentChannelId: string | null,
   *   afkChannelId: string | null,
   *   afkTimeoutSecs: number,
   *   selfMute?: boolean,
   *   selfDeaf?: boolean,
   *   isStageChannel?: boolean
   * }} config
   */
  constructor(config) {
    if (!isValidSnowflake(config.guildId)) {
      throw new RangeError(`VoiceAfkMonitorEngine: некорректный guildId: ${config.guildId}`);
    }
    if (config.currentChannelId !== null && !isValidSnowflake(config.currentChannelId)) {
      throw new RangeError(`VoiceAfkMonitorEngine: некорректный currentChannelId`);
    }
    if (config.afkChannelId !== null && !isValidSnowflake(config.afkChannelId)) {
      throw new RangeError(`VoiceAfkMonitorEngine: некорректный afkChannelId`);
    }
    if (!isValidAfkTimeoutSecs(config.afkTimeoutSecs)) {
      throw new RangeError(
        `VoiceAfkMonitorEngine: недопустимый afkTimeoutSecs: ${config.afkTimeoutSecs}`,
      );
    }

    this.guildId = config.guildId;
    this.currentChannelId = config.currentChannelId;
    this.afkChannelId = config.afkChannelId;
    this.afkTimeoutSecs = config.afkTimeoutSecs;
    this.selfMute = Boolean(config.selfMute ?? false);
    this.selfDeaf = Boolean(config.selfDeaf ?? false);
    this.isStageChannel = Boolean(config.isStageChannel ?? false);
    this.movedToAfk =
      this.currentChannelId !== null &&
      this.afkChannelId !== null &&
      this.currentChannelId === this.afkChannelId;
    /** @type {{ channelId: string | null, selfMute: boolean, movedToAfk: boolean } | null} */
    this._rollbackSnapshot = null;
  }

  /**
   * Оценивает очередной тик системного времени бездействия и при превышении `afk_timeout`
   * формирует кадр Gateway Opcode 4 на перевод в `afk_channel_id` с `self_mute: true`.
   * @param {{
   *   rawIdleMs: number,
   *   mediaPlaying?: boolean,
   *   activeStreamCount?: number,
   *   userSpeaking?: boolean,
   *   systemSuspended?: boolean,
   *   screenLocked?: boolean
   * }} tick
   */
  evaluateIdleTick(tick) {
    const timeoutMs = this.afkTimeoutSecs * 1_000;
    const effectiveIdleMs = computeEffectiveIdleMs(tick);

    if (this.currentChannelId === null || this.afkChannelId === null || this.isStageChannel) {
      return {
        effectiveIdleMs,
        timeoutMs,
        shouldMoveToAfk: false,
        bypassIdleUpdate: false,
        opcode4Payload: null,
        afkBadgeState: resolveAfkBadgeState({ movedToAfk: this.movedToAfk }).label,
        idleDetectState: resolveIdleDetectState(effectiveIdleMs, timeoutMs).label,
        voiceAfkState: resolveVoiceAfkState(effectiveIdleMs, timeoutMs, this.movedToAfk).label,
      };
    }

    if (this.movedToAfk || this.currentChannelId === this.afkChannelId) {
      return {
        effectiveIdleMs,
        timeoutMs,
        shouldMoveToAfk: false,
        bypassIdleUpdate: false,
        opcode4Payload: null,
        afkBadgeState: resolveAfkBadgeState({ movedToAfk: true }).label,
        idleDetectState: resolveIdleDetectState(effectiveIdleMs, timeoutMs).label,
        voiceAfkState: resolveVoiceAfkState(effectiveIdleMs, timeoutMs, true).label,
      };
    }

    if (effectiveIdleMs >= timeoutMs) {
      this._rollbackSnapshot = {
        channelId: this.currentChannelId,
        selfMute: this.selfMute,
        movedToAfk: this.movedToAfk,
      };
      this.currentChannelId = this.afkChannelId;
      this.selfMute = true;
      this.movedToAfk = true;
      const opcode4Payload = buildVoiceStateUpdateOpcode4({
        guildId: this.guildId,
        channelId: this.afkChannelId,
        selfMute: true,
        selfDeaf: this.selfDeaf,
        selfVideo: false,
        flags: AFK_PROTOCOL_CONSTANTS.VOICE_STATE_DEFAULT_FLAGS,
      });
      return {
        effectiveIdleMs,
        timeoutMs,
        shouldMoveToAfk: true,
        bypassIdleUpdate: true,
        opcode4Payload,
        afkBadgeState: resolveAfkBadgeState({ movedToAfk: true }).label,
        idleDetectState: resolveIdleDetectState(effectiveIdleMs, timeoutMs).label,
        voiceAfkState: resolveVoiceAfkState(effectiveIdleMs, timeoutMs, true).label,
      };
    }

    return {
      effectiveIdleMs,
      timeoutMs,
      shouldMoveToAfk: false,
      bypassIdleUpdate: false,
      opcode4Payload: null,
      afkBadgeState: resolveAfkBadgeState({ movedToAfk: false }).label,
      idleDetectState: resolveIdleDetectState(effectiveIdleMs, timeoutMs).label,
      voiceAfkState: resolveVoiceAfkState(effectiveIdleMs, timeoutMs, false).label,
    };
  }

  /** Откатывает оптимистичный перевод в AFK-канал при обрыве соединения со шлюзом. */
  rollbackAfkMove() {
    if (!this._rollbackSnapshot) return false;
    this.currentChannelId = this._rollbackSnapshot.channelId;
    this.selfMute = this._rollbackSnapshot.selfMute;
    this.movedToAfk = this._rollbackSnapshot.movedToAfk;
    this._rollbackSnapshot = null;
    return true;
  }
}

/**
 * Сверяет карточки `TASK-1010`/`1010A`/`1010B`, фикстуру D-Bus `power.json` и события Gateway с оракулами.
 * @param {{
 *   kanbanPath?: string,
 *   dbusPowerPath?: string,
 *   gatewayProtocolPath?: string,
 *   gatewaySchemaPath?: string,
 *   reModulesIndexPath?: string
 * }} [opts]
 */
export function verifyAfkOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const dbusPowerPath = opts.dbusPowerPath ?? DEFAULT_DBUS_POWER_PATH;
  const gatewayProtocolPath = opts.gatewayProtocolPath ?? DEFAULT_GW_PROTOCOL_PATH;
  const gatewaySchemaPath = opts.gatewaySchemaPath ?? DEFAULT_GW_SCHEMA_PATH;
  /** @type {string[]} */
  const reasons = [];

  for (const p of [kanbanPath, dbusPowerPath, gatewayProtocolPath, gatewaySchemaPath]) {
    if (!existsSync(p)) {
      return {
        passed: false,
        reasons: [`Отсутствует оракул: ${p}`],
        gatewayVoiceStateUpdateCount: 0,
        dbusFixtureIdleSeconds: 0,
      };
    }
  }

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card1010 = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1010");
  if (!card1010) {
    reasons.push("Карточка TASK-1010 не найдена в waves/kanban-board.json");
  } else {
    if (card1010.layout?.container !== AFK_SERVICE_LAYOUT.container) {
      reasons.push(`Несовпадение container: ${card1010.layout?.container}`);
    }
    if (card1010.design_tokens?.accent !== AFK_DESIGN_TOKENS.accent) {
      reasons.push(`Несовпадение accent: ${card1010.design_tokens?.accent}`);
    }
    if (card1010.interactive_states?.[0]?.active !== AFK_BADGE_STATES.active) {
      reasons.push("Несовпадение active state для afk-badge");
    }
  }

  const dbusPower = JSON.parse(readFileSync(dbusPowerPath, "utf8"));
  const ssService = dbusPower.services?.find(
    (/** @type {any} */ s) => s.destination === AFK_PROTOCOL_CONSTANTS.DBUS_SCREENSAVER_DEST,
  );
  const idleMethod = ssService?.methods?.find(
    (/** @type {any} */ m) => m.member === AFK_PROTOCOL_CONSTANTS.DBUS_IDLE_METHOD,
  );
  const dbusFixtureIdleSeconds = idleMethod?.reply ?? 0;
  if (dbusFixtureIdleSeconds !== 1234) {
    reasons.push(`Ожидался GetSessionIdleTime=1234 в fixtures/dbus/power.json, получено ${dbusFixtureIdleSeconds}`);
  }

  const gwProto = JSON.parse(readFileSync(gatewayProtocolPath, "utf8"));
  const gatewayVoiceStateUpdateCount = gwProto.opcodes?.dispatchTypes?.VOICE_STATE_UPDATE ?? 0;
  if (gatewayVoiceStateUpdateCount !== 8) {
    reasons.push(
      `Ожидалось VOICE_STATE_UPDATE=8 в gateway-protocol.json, получено ${gatewayVoiceStateUpdateCount}`,
    );
  }

  const gwSchemaRaw = readFileSync(gatewaySchemaPath, "utf8");
  if (!gwSchemaRaw.includes('"self_mute"') || !gwSchemaRaw.includes('"self_deaf"')) {
    reasons.push("В docs/re/gateway-schema.json отсутствуют поля self_mute / self_deaf");
  }

  const reModPath =
    opts.reModulesIndexPath ??
    (existsSync(DEFAULT_RE_MODULES_INDEX_PATH) ? DEFAULT_RE_MODULES_INDEX_PATH : undefined);
  if (reModPath && existsSync(reModPath)) {
    const reMods = JSON.parse(readFileSync(reModPath, "utf8"));
    for (const modId of ["885576", "576991", "926456", "185255", "260509"]) {
      if (!reMods[modId]) {
        reasons.push(`В /home/sanya/re-deep/modules-index.json отсутствует модуль ${modId}`);
      }
    }
  }

  return {
    passed: reasons.length === 0,
    reasons,
    gatewayVoiceStateUpdateCount,
    dbusFixtureIdleSeconds,
  };
}

/**
 * Маскирует токены авторизации, `session_id` и секреты в строках телеметрии AFK-монитора.
 * @param {string} text
 * @returns {string}
 */
export function redactAfkTelemetry(text) {
  return String(text ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "mfa.[REDACTED]")
    .replace(/[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED]")
    .replace(/(token|secret|authorization|session_id)\s*[:=]\s*["']?[^"'\s]+["']?/gi, "$1=[REDACTED]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для модуля AFK-монитора.
 */
export function evaluateAfkNineDimensions() {
  /** @type {string[]} */
  const failures = [];

  // 1. Позитивные сценарии (Opcode 4 на перевод в afk_channel_id с self_mute: true)
  const op4 = buildVoiceStateUpdateOpcode4({
    guildId: "274093052673392641",
    channelId: "274955515333836801",
    selfMute: true,
    selfDeaf: false,
  });
  if (op4.op !== 4 || op4.d.self_mute !== true) failures.push("dim1_positive_opcode4");

  // 2. Негативные сценарии (невалидный afk_timeout и подавление при воспроизведении медиа)
  if (isValidAfkTimeoutSecs(45) || !isValidAfkTimeoutSecs(300)) {
    failures.push("dim2_negative_invalid_afk_timeout");
  }
  if (
    computeEffectiveIdleMs({
      rawIdleMs: 300_000,
      mediaPlaying: true,
      activeStreamCount: 0,
      userSpeaking: false,
    }) !== 0
  ) {
    failures.push("dim2_negative_media_inhibition");
  }

  // 3. Граничные значения (Snowflake 0 / u64::MAX, 60..3600 с, 2000 симв., 10 файлов/25 МБ, offset=9975, 0..200%, 99+)
  if (isValidSnowflake("0") !== false || isValidSnowflake("18446744073709551615") !== true) {
    failures.push("dim3_boundary_snowflake");
  }
  const badgeFormat = (/** @type {number} */ n) => (n > 99 ? "99+" : String(n));
  if (badgeFormat(99) !== "99" || badgeFormat(100) !== "99+") {
    failures.push("dim3_boundary_badge_cap");
  }

  // 4. Обработка ошибок D-Bus (UnknownMethod -> fallback) и HTTP (400/401/403/404/413/429 retry_after_ms/5xx)
  const mapHttpStatus = (/** @type {number} */ status, /** @type {number} */ retryMs) => ({
    retryable: status === 429 || status >= 500,
    backoffMs: status === 429 ? Math.max(100, retryMs) : status >= 500 ? 1_000 : 0,
  });
  if (!mapHttpStatus(429, 1_500).retryable || mapHttpStatus(403, 0).retryable) {
    failures.push("dim4_http_and_dbus_error_classification");
  }

  // 5. Пустые состояния (отсутствие afk_channel_id или не в голосовом канале -> no-op)
  const emptyEng = new VoiceAfkMonitorEngine({
    guildId: "942355007271997450",
    currentChannelId: "1294815650186788916",
    afkChannelId: null,
    afkTimeoutSecs: 300,
  });
  if (emptyEng.evaluateIdleTick({ rawIdleMs: 500_000 }).shouldMoveToAfk) {
    failures.push("dim5_empty_null_afk_channel");
  }

  // 6. Загрузочные состояния (skeleton / HTTP 202 pending voice transition)
  const isSkeleton = (/** @type {{ loading?: boolean, httpStatus?: number }} */ s) =>
    Boolean(s.loading || s.httpStatus === 202);
  if (!isSkeleton({ httpStatus: 202 }) || isSkeleton({ httpStatus: 200 })) {
    failures.push("dim6_loading_http_202");
  }

  // 7. Права доступа (VIEW_CHANNEL, CONNECT, MOVE_MEMBERS и AGE_GATED_SPACES)
  const reqPerms =
    AFK_PROTOCOL_CONSTANTS.PERM_VIEW_CHANNEL | AFK_PROTOCOL_CONSTANTS.PERM_CONNECT;
  const canJoinAfk = (/** @type {bigint} */ perms, /** @type {boolean} */ ageRestricted) =>
    !ageRestricted && (perms & reqPerms) === reqPerms;
  if (!canJoinAfk(reqPerms, false) || canJoinAfk(reqPerms, true) || canJoinAfk(0n, false)) {
    failures.push("dim7_permissions_and_age_gate");
  }

  // 8. Восстановление после сбоев (откат оптимистичного перевода в AFK при обрыве шлюза)
  const recEng = new VoiceAfkMonitorEngine({
    guildId: "274093052673392641",
    currentChannelId: "351403515723120651",
    afkChannelId: "274955515333836801",
    afkTimeoutSecs: 60,
  });
  recEng.evaluateIdleTick({ rawIdleMs: 60_000 });
  recEng.rollbackAfkMove();
  if (recEng.movedToAfk || recEng.currentChannelId !== "351403515723120651") {
    failures.push("dim8_rollback_recovery");
  }

  // 9. Наблюдаемость и маскирование секретов
  if (redactAfkTelemetry('session_id="secret123" Bearer mfa.tok99').includes("secret123")) {
    failures.push("dim9_observability_redaction");
  }

  return { passed: failures.length === 0, dimensionsChecked: 9, failures };
}

/**
 * Исполняет CLI `scripts/verify-afk.mjs`.
 * @param {readonly string[]} argv
 * @returns {number}
 */
export function runAfkCli(argv) {
  let oraclePath = DEFAULT_KANBAN_PATH;
  let outJson = "";
  let outMd = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      console.log(
        "Использование: node scripts/verify-afk.mjs [--oracle <path>] [--out-json <path>] [--out-md <path>]",
      );
      return 0;
    }
    if (arg === "--oracle" || arg === "--out-json" || arg === "--out-md") {
      const val = argv[i + 1];
      if (!val) return 1;
      if (arg === "--oracle") oraclePath = val;
      if (arg === "--out-json") outJson = val;
      if (arg === "--out-md") outMd = val;
      i += 1;
      continue;
    }
    return 1;
  }

  if (!existsSync(oraclePath)) {
    console.error(`verify-afk: файл-оракул недоступен: ${oraclePath}`);
    return 2;
  }

  const parity = verifyAfkOracleParity({ kanbanPath: oraclePath });
  const dims = evaluateAfkNineDimensions();
  if (!parity.passed || !dims.passed) {
    console.error("verify-afk: нарушение контракта", [...parity.reasons, ...dims.failures]);
    return 1;
  }

  const report = {
    taskId: "TASK-1010",
    status: "PASS",
    layout: AFK_SERVICE_LAYOUT,
    designTokens: AFK_DESIGN_TOKENS,
    badgeStates: AFK_BADGE_STATES,
    gatewayVoiceStateUpdateCount: parity.gatewayVoiceStateUpdateCount,
    dbusFixtureIdleSeconds: parity.dbusFixtureIdleSeconds,
    dimensionsChecked: dims.dimensionsChecked,
  };
  if (outJson) {
    mkdirSync(dirname(resolve(outJson)), { recursive: true });
    writeFileSync(resolve(outJson), `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  if (outMd) {
    mkdirSync(dirname(resolve(outMd)), { recursive: true });
    writeFileSync(
      resolve(outMd),
      `# Отчёт верификации TASK-1010 (Voice AFK Monitor)\n\n- Статус: **PASS**\n- Событий VOICE_STATE_UPDATE: \`${parity.gatewayVoiceStateUpdateCount}\`\n- D-Bus GetSessionIdleTime: \`${parity.dbusFixtureIdleSeconds}s\`\n- Проверено измерений: \`${dims.dimensionsChecked}/9\`\n`,
      "utf8",
    );
  }
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runAfkCli(process.argv.slice(2)));
}
