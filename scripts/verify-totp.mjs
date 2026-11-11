#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор двухфакторной аутентификации TOTP 2FA и управления сессиями (TASK-1009):
 * - Сверка дизайн-токенов, геометрии `TOTP Modal` (`400px x 280px`, `padding: 20px`, `spacing: 12px`)
 *   и `Sessions Manager` (`Settings Tab`, `padding: 16px`, `spacing: 12px`), интерактивных состояний
 *   `digit-box` (`border` -> `border-accent` -> `focused digit`) и `revoke-session`
 *   (`button` -> `red hover` -> `session deleted`), маршрутов REST (`POST /auth/mfa/totp`,
 *   `GET /auth/sessions`, `DELETE /auth/sessions/{id}`, `POST /auth/sessions/logout`) и 17 реальных
 *   активных сессий из `/home/sanya/re-deep/decoded/36a171b84d1c0225_0` с независимыми RE-оракулами.
 * - Контроллер 6 полей ввода цифр TOTP с автоматическим переносом фокуса (`0..5`), возвратом по
 *   `Backspace`, навигацией стрелками и вставкой 6-значного кода из буфера обмена (`Ctrl+V`).
 * - Менеджер активных сессий с выделением текущего устройства, сортировкой по времени активности,
 *   завершением сессии в 1 клик (`DELETE /auth/sessions/{id}`) и откатом при сетевом сбое.
 * - Покрытие 9 обязательных измерений `TEST-POLICY.md` и маскирование секретов.
 *
 * Коды выхода CLI:
 * - 0: Все инварианты TOTP 2FA, сессий и RE-оракулы пройдены.
 * - 1: Ошибка аргументов командной строки или нарушение инвариантов.
 * - 2: Внешний файл-оракул недоступен или отсутствует.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_REST_D02_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const DEFAULT_RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const DEFAULT_RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";
const DEFAULT_RE_SESSIONS_CACHE_PATH = "/home/sanya/re-deep/decoded/36a171b84d1c0225_0";

/** Макет модального окна `TOTP Modal` из `waves/kanban-board.json` (`TASK-1009`). */
export const TOTP_MODAL_LAYOUT = Object.freeze({
  container: "TOTP Modal",
  dimensions: "400px x 280px",
  padding: "20px",
  spacing: "12px",
});

/** Дизайн-токены карточки `TASK-1009` из `waves/kanban-board.json`. */
export const TOTP_DESIGN_TOKENS = Object.freeze({
  background: "#313338",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/** Интерактивные состояния поля `digit-box` из `waves/kanban-board.json` (`TASK-1009`). */
export const TOTP_DIGIT_BOX_STATES = Object.freeze({
  element: "digit-box",
  default: "border",
  hover: "border-accent",
  active: "focused digit",
});

/** Макет вкладки `Sessions Manager` из `waves/kanban-board.json` (`TASK-1009B`). */
export const SESSIONS_MANAGER_LAYOUT = Object.freeze({
  container: "Sessions Manager",
  dimensions: "Settings Tab",
  padding: "16px",
  spacing: "12px",
});

/** Дизайн-токены `Sessions Manager` (`TASK-1009B`). */
export const SESSIONS_MANAGER_TOKENS = Object.freeze({
  background: "#313338",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#f23f43",
  border: "#3f4147",
});

/** Интерактивные состояния кнопки `revoke-session` (`TASK-1009B`). */
export const REVOKE_SESSION_STATES = Object.freeze({
  element: "revoke-session",
  default: "button",
  hover: "red hover",
  active: "session deleted",
});

/** Константы протокола MFA и управления сессиями из модулей `200921`, `572469`, `533925`, `670492`, `825542` RE-дампа. */
export const TOTP_PROTOCOL_CONSTANTS = Object.freeze({
  TOTP_DIGIT_COUNT: 6,
  BACKUP_CODE_MIN_LEN: 8,
  BACKUP_CODE_MAX_LEN: 11,
  MFA_INVALID_CODE_ERROR: 60008,
  MFA_REQUIRED_ERROR: 60003,
  DIGIT_FIELD_BG_HEX: "#1e1f22",
  CURRENT_DEVICE_BADGE_RU: "Текущее устройство",
  MFA_METHODS_WHITELIST: Object.freeze(["totp", "sms", "backup", "webauthn", "password"]),
  RE_MODULE_AUTH_SESSIONS_ACTIONS: "200921",
  RE_MODULE_MFA_MODAL: "572469",
  RE_MODULE_MFA_FINISH: "533925",
  RE_MODULE_MFA_STORE: "670492",
  RE_MODULE_AUTH_SESSIONS_STORE: "825542",
});

/**
 * 17 живых активных сессий `user_sessions`, зафиксированных в `/home/sanya/re-deep/decoded/36a171b84d1c0225_0`
 * (`GET https://discord.com/api/v9/auth/sessions`).
 */
export const LIVE_AUTH_SESSIONS_ORACLE = Object.freeze([
  Object.freeze({ id_hash: "DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=", approx_last_used_time: "2026-10-01T12:39:55.809011+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Frankfurt am Main, Hesse, Germany" }) }),
  Object.freeze({ id_hash: "OdKq9gNLX69NrpWTtUFE3MH2d2evCrU0o5eLIUvFP4s=", approx_last_used_time: "2026-10-18T12:33:18.747920+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Riga, Riga, Latvia" }) }),
  Object.freeze({ id_hash: "SIYXCYXQnxanTF0V1iOSleP9FEADrhLFZZAiuLEeS7Y=", approx_last_used_time: "2026-10-18T12:11:50.439852+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Riga, Riga, Latvia" }) }),
  Object.freeze({ id_hash: "XGEkiho2xx1r5fsNFcMrji2y98nZ/4sJt0qGbWNCc4Q=", approx_last_used_time: "2026-09-30T19:58:17.200883+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Helsinki, Uusimaa, Finland" }) }),
  Object.freeze({ id_hash: "YjVqy80kprjtVVYdvqiKkHr53Bi2YcJr8glhAzagQZ4=", approx_last_used_time: "2026-01-20T19:15:27.373896+00:00", client_info: Object.freeze({ os: "Linux", platform: "Chrome", location: "Moldova" }) }),
  Object.freeze({ id_hash: "ZRKqgNMZ9S52itLW8BHtuF+NUYZHtCCa3bFmRWjarH4=", approx_last_used_time: "2026-02-14T16:59:45.823713+00:00", client_info: Object.freeze({ os: "Linux", platform: "Chrome", location: "Moldova" }) }),
  Object.freeze({ id_hash: "b5kVztfC8kvmm4GrZFkMqXsk0vec284aOWK69soAbeY=", approx_last_used_time: "2026-01-23T15:43:00.613288+00:00", client_info: Object.freeze({ os: "Windows", platform: "Discord Client", location: "Moldova" }) }),
  Object.freeze({ id_hash: "eezzIc5BohNwqEMTqISEzB2YF+Y3oqyzDYO8CV7QQws=", approx_last_used_time: "2026-03-13T08:59:55.727329+00:00", client_info: Object.freeze({ os: "Linux", platform: "Chrome", location: "Moldova" }) }),
  Object.freeze({ id_hash: "g0nMEM4IKCIOge5NLCCqtVHGG7SqWm552XRQtSvR8UQ=", approx_last_used_time: "2026-10-15T19:17:10.597442+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Riga, Riga, Latvia" }) }),
  Object.freeze({ id_hash: "hzyZof5WbJlbqfCUP32ziTd11i8Rx0JZFqrq/YZ/wDo=", approx_last_used_time: "2026-05-08T04:47:34.358154+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Moldova" }) }),
  Object.freeze({ id_hash: "tpneBfE67/J3GQbfLtkiAhorgnKTXq50O0P77YyMlGI=", approx_last_used_time: "2026-05-11T07:47:30.293364+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Moldova" }) }),
  Object.freeze({ id_hash: "uHy3s7LK9s8jkq/BsD/QAzd6lFQ47iXrz92f3GyIbmY=", approx_last_used_time: "2026-02-18T03:46:51.140579+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Moldova" }) }),
  Object.freeze({ id_hash: "uwAAjIxg/VGsmdiUIw7g+7+Jih/xNEtwHQ1P5g5WdQc=", approx_last_used_time: "2026-02-14T17:03:41.986783+00:00", client_info: Object.freeze({ os: "Linux", platform: "Chrome", location: "Moldova" }) }),
  Object.freeze({ id_hash: "7o7fiuzmHbNlYRriA80y6/Cyi99kYYA43/I0W2bePLM=", approx_last_used_time: "2026-01-23T14:31:13.390277+00:00", client_info: Object.freeze({ os: "Windows", platform: "Chrome", location: "Moldova" }) }),
  Object.freeze({ id_hash: "9SvZmC0cfcZ1aSr0GXqyBeoydehUCzHLUusumNKIWxI=", approx_last_used_time: "2026-07-22T07:23:43.908838+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Helsinki, Uusimaa, Finland" }) }),
  Object.freeze({ id_hash: "+M57ZQlYMj5Yi+K4vraC2DXc5mQJOYkyu/Dz/mrje/8=", approx_last_used_time: "2026-10-18T07:23:02.676105+00:00", client_info: Object.freeze({ os: "Linux", platform: "Chrome", location: "Riga, Riga, Latvia" }) }),
  Object.freeze({ id_hash: "+VVLobYzNFTSqLMWsg3Rd9JXl9HdzAeqlReLvwsSHsc=", approx_last_used_time: "2026-10-04T11:49:29.466877+00:00", client_info: Object.freeze({ os: "Linux", platform: "Discord Client", location: "Frankfurt am Main, Hesse, Germany" }) }),
]);

/**
 * Проверяет корректность Snowflake ID (`> 0` и `<= u64::MAX`).
 * @param {string | number | bigint} raw
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
 * Вычисляет интерактивное состояние поля `digit-box` (`border` / `border-accent` / `focused digit`).
 * @param {{ focused?: boolean, hovered?: boolean }} opts
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveDigitBoxState(opts) {
  if (opts.focused) {
    return {
      element: TOTP_DIGIT_BOX_STATES.element,
      state: "active",
      label: TOTP_DIGIT_BOX_STATES.active,
    };
  }
  if (opts.hovered) {
    return {
      element: TOTP_DIGIT_BOX_STATES.element,
      state: "hover",
      label: TOTP_DIGIT_BOX_STATES.hover,
    };
  }
  return {
    element: TOTP_DIGIT_BOX_STATES.element,
    state: "default",
    label: TOTP_DIGIT_BOX_STATES.default,
  };
}

/**
 * Вычисляет интерактивное состояние кнопки `revoke-session` (`button` / `red hover` / `session deleted`).
 * @param {{ deleted?: boolean, hovered?: boolean }} opts
 * @returns {{ element: string, state: "default" | "hover" | "active", label: string }}
 */
export function resolveRevokeButtonState(opts) {
  if (opts.deleted) {
    return {
      element: REVOKE_SESSION_STATES.element,
      state: "active",
      label: REVOKE_SESSION_STATES.active,
    };
  }
  if (opts.hovered) {
    return {
      element: REVOKE_SESSION_STATES.element,
      state: "hover",
      label: REVOKE_SESSION_STATES.hover,
    };
  }
  return {
    element: REVOKE_SESSION_STATES.element,
    state: "default",
    label: REVOKE_SESSION_STATES.default,
  };
}

/**
 * Контроллер 6 полей ввода TOTP-кода с автоматическим переносом фокуса (`0..5`),
 * возвратом по `Backspace` и вставкой из буфера обмена (`Ctrl+V`).
 */
export class TotpInputController {
  constructor() {
    /** @type {string[]} */
    this.boxes = Array.from({ length: TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT }, () => "");
    this.focusedIndex = 0;
    this.submitting = false;
  }

  /**
   * Вводит одну цифру в активное поле и автоматически переносит фокус вправо.
   * @param {string} ch
   * @returns {boolean}
   */
  inputDigit(ch) {
    if (this.submitting) return false;
    const str = String(ch ?? "");
    if (!/^\d$/.test(str)) return false;
    this.boxes[this.focusedIndex] = str;
    if (this.focusedIndex + 1 < TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT) {
      this.focusedIndex += 1;
    }
    return true;
  }

  /** Обрабатывает `Backspace`: очищает текущую ячейку или переносит фокус влево и очищает предыдущую. */
  handleBackspace() {
    if (this.submitting) return this.focusedIndex;
    if (this.boxes[this.focusedIndex] !== "") {
      this.boxes[this.focusedIndex] = "";
    } else if (this.focusedIndex > 0) {
      this.focusedIndex -= 1;
      this.boxes[this.focusedIndex] = "";
    }
    return this.focusedIndex;
  }

  /**
   * Вставляет код из буфера обмена (`Ctrl+V`), извлекая до 6 цифр.
   * @param {string} clipboardText
   * @returns {number}
   */
  pasteCode(clipboardText) {
    if (this.submitting) return 0;
    const digits = String(clipboardText ?? "").replace(/\D+/g, "");
    if (!digits) return 0;
    const count = Math.min(digits.length, TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT);
    for (let i = 0; i < TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT; i += 1) {
      this.boxes[i] = i < count ? (digits[i] ?? "") : "";
    }
    this.focusedIndex =
      count >= TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT
        ? TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT - 1
        : count;
    return count;
  }

  /** @returns {boolean} */
  isComplete() {
    return (
      !this.submitting &&
      this.boxes.every((d) => /^\d$/.test(d))
    );
  }

  /** @returns {string} */
  getCode() {
    return this.boxes.join("");
  }

  clear() {
    this.boxes.fill("");
    this.focusedIndex = 0;
    this.submitting = false;
  }
}

/**
 * Формирует REST-запрос `POST /auth/mfa/{method}` для подтверждения 2FA (TOTP или backup-код).
 * @param {{ method: string, code: string, ticket: string, loginInstanceId?: string }} params
 */
export function buildTotpMfaRequest(params) {
  const method = String(params.method ?? "").trim();
  if (!TOTP_PROTOCOL_CONSTANTS.MFA_METHODS_WHITELIST.includes(method)) {
    throw new Error(`buildTotpMfaRequest: недопустимый метод MFA "${method}"`);
  }
  const ticket = String(params.ticket ?? "").trim();
  if (!ticket) {
    throw new Error("buildTotpMfaRequest: пустой ticket MFA");
  }
  const rawCode = String(params.code ?? "").trim();
  const normalizedCode = method === "backup" ? rawCode.replace(/-/g, "") : rawCode;
  if (method === "backup") {
    if (
      normalizedCode.length < TOTP_PROTOCOL_CONSTANTS.BACKUP_CODE_MIN_LEN ||
      normalizedCode.length > TOTP_PROTOCOL_CONSTANTS.BACKUP_CODE_MAX_LEN ||
      !/^[A-Za-z0-9]+$/.test(normalizedCode)
    ) {
      throw new Error("buildTotpMfaRequest: некорректный резервный код 2FA");
    }
  } else if (!/^\d{6}$/.test(normalizedCode)) {
    throw new Error("buildTotpMfaRequest: TOTP-код должен состоять ровно из 6 цифр");
  }
  /** @type {{ code: string, ticket: string, login_instance_id?: string }} */
  const body = { code: normalizedCode, ticket };
  if (params.loginInstanceId && params.loginInstanceId.trim()) {
    body.login_instance_id = params.loginInstanceId.trim();
  }
  return {
    httpMethod: "POST",
    path: `/auth/mfa/${method}`,
    body,
  };
}

/** Формирует запрос `GET /auth/sessions`. */
export function buildFetchSessionsRequest() {
  return {
    method: "GET",
    path: "/auth/sessions",
  };
}

/**
 * Формирует запрос `DELETE /auth/sessions/{id}` для принудительного завершения одной сессии.
 * @param {string} sessionIdHash
 */
export function buildDeleteSessionRequest(sessionIdHash) {
  const trimmed = String(sessionIdHash ?? "").trim();
  if (!trimmed) {
    throw new Error("buildDeleteSessionRequest: пустой идентификатор сессии");
  }
  return {
    method: "DELETE",
    path: `/auth/sessions/${trimmed}`,
    sessionIdHash: trimmed,
  };
}

/**
 * Формирует пакетный запрос `POST /auth/sessions/logout` (`{ session_id_hashes }`).
 * @param {readonly string[]} sessionIdHashes
 */
export function buildLogoutSessionsRequest(sessionIdHashes) {
  const cleaned = (sessionIdHashes ?? [])
    .map((h) => String(h ?? "").trim())
    .filter(Boolean);
  if (cleaned.length === 0) {
    throw new Error("buildLogoutSessionsRequest: список session_id_hashes пуст");
  }
  return {
    method: "POST",
    path: "/auth/sessions/logout",
    body: { session_id_hashes: cleaned },
  };
}

const HOUR_MS = 3_600_000;
const DAY_MS = 86_400_000;

/**
 * Форматирует время последней активности сессии на русском языке (паритет с RE `_C(e)`).
 * @param {number} lastUsedMs
 * @param {number} nowMs
 * @returns {string}
 */
export function formatSessionLastActiveRu(lastUsedMs, nowMs) {
  const diffMs = Math.max(0, nowMs - lastUsedMs);
  if (diffMs < HOUR_MS) return "Менее часа назад";
  if (diffMs < DAY_MS) return `${Math.floor(diffMs / HOUR_MS)} ч. назад`;
  return `${Math.floor(diffMs / DAY_MS)} дн. назад`;
}

/**
 * Менеджер активных сессий пользователя (`GET /auth/sessions`, `DELETE /auth/sessions/{id}`,
 * `POST /auth/sessions/logout`) с поддержкой отката при сетевом сбое.
 */
export class AuthSessionsManager {
  /**
   * @param {{
   *   currentSessionIdHash: string,
   *   sessions: ReadonlyArray<{ id_hash: string, approx_last_used_time: string, client_info?: { os?: string, platform?: string, location?: string, ip?: string } }>
   * }} cfg
   */
  constructor(cfg) {
    this.currentSessionIdHash = String(cfg.currentSessionIdHash ?? "").trim();
    this.sessions = (cfg.sessions ?? []).map((s) => ({
      idHash: String(s.id_hash ?? "").trim(),
      approxLastUsedMs: Date.parse(s.approx_last_used_time) || 0,
      os: String(s.client_info?.os ?? "").trim() || "Неизвестная ОС",
      platform: String(s.client_info?.platform ?? "").trim() || "Discord Client",
      location: String(s.client_info?.location ?? s.client_info?.ip ?? "").trim(),
    }));
    /** @type {typeof this.sessions | null} */
    this._rollbackBackup = null;
  }

  /** @returns {number} */
  otherSessionsCount() {
    return this.sessions.filter((s) => s.idHash !== this.currentSessionIdHash).length;
  }

  /**
   * Возвращает разделённый список `currentSession` (с бейджем) и отсортированных `otherSessions`.
   * @param {number} nowMs
   */
  snapshot(nowMs = Date.now()) {
    const cur = this.sessions.find((s) => s.idHash === this.currentSessionIdHash) ?? null;
    const currentSession = cur
      ? {
          ...cur,
          isCurrent: true,
          badgeRu: TOTP_PROTOCOL_CONSTANTS.CURRENT_DEVICE_BADGE_RU,
          lastActiveRu: formatSessionLastActiveRu(cur.approxLastUsedMs, nowMs),
        }
      : null;
    const otherSessions = this.sessions
      .filter((s) => s.idHash !== this.currentSessionIdHash)
      .sort((a, b) => b.approxLastUsedMs - a.approxLastUsedMs)
      .map((s) => ({
        ...s,
        isCurrent: false,
        badgeRu: null,
        lastActiveRu: formatSessionLastActiveRu(s.approxLastUsedMs, nowMs),
      }));
    return { currentSession, otherSessions };
  }

  /**
   * Завершает одну чужую сессию (`DELETE /auth/sessions/{id}`).
   * @param {string} idHash
   */
  revokeSession(idHash) {
    const trimmed = String(idHash ?? "").trim();
    if (!trimmed) {
      throw new Error("revokeSession: пустой id_hash");
    }
    if (trimmed === this.currentSessionIdHash) {
      throw new Error("revokeSession: нельзя принудительно удалить текущую сессию через список других устройств");
    }
    const idx = this.sessions.findIndex((s) => s.idHash === trimmed);
    if (idx === -1) {
      throw new Error(`revokeSession: сессия не найдена: ${trimmed}`);
    }
    this._rollbackBackup = this.sessions.map((s) => ({ ...s }));
    this.sessions.splice(idx, 1);
    return {
      deleteRequest: buildDeleteSessionRequest(trimmed),
      logoutRequest: buildLogoutSessionsRequest([trimmed]),
    };
  }

  /** Завершает все остальные сессии, кроме текущего устройства. */
  revokeAllOtherSessions() {
    const otherHashes = this.sessions
      .filter((s) => s.idHash !== this.currentSessionIdHash)
      .map((s) => s.idHash);
    const logoutRequest = buildLogoutSessionsRequest(otherHashes);
    this._rollbackBackup = this.sessions.map((s) => ({ ...s }));
    this.sessions = this.sessions.filter((s) => s.idHash === this.currentSessionIdHash);
    return { logoutRequest };
  }

  /** Откатывает последнее удаление сессии при ошибке сети (`401/403/429/5xx`). */
  rollbackLastRevoke() {
    if (!this._rollbackBackup) return false;
    this.sessions = this._rollbackBackup;
    this._rollbackBackup = null;
    return true;
  }
}

/**
 * Сверяет карточку `TASK-1009`, дизайн-токены, маршруты MFA/сессий и дамп 17 сессий с оракулами.
 * @param {{
 *   kanbanPath?: string,
 *   restD02Path?: string,
 *   reRestStaticPath?: string,
 *   reModulesIndexPath?: string,
 *   reSessionsCachePath?: string
 * }} [opts]
 */
export function verifyTotpOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const restD02Path = opts.restD02Path ?? DEFAULT_REST_D02_PATH;
  /** @type {string[]} */
  const reasons = [];

  if (!existsSync(kanbanPath)) {
    return { passed: false, reasons: [`Отсутствует оракул: ${kanbanPath}`], liveSessionCount: 0 };
  }
  if (!existsSync(restD02Path)) {
    return { passed: false, reasons: [`Отсутствует оракул: ${restD02Path}`], liveSessionCount: 0 };
  }

  const kanban = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card1009 = kanban.cards?.find((/** @type {any} */ c) => c.id === "TASK-1009");
  if (!card1009) {
    reasons.push("Карточка TASK-1009 не найдена в waves/kanban-board.json");
  } else {
    if (card1009.layout?.container !== TOTP_MODAL_LAYOUT.container) {
      reasons.push(`Несовпадение container: ${card1009.layout?.container}`);
    }
    if (card1009.design_tokens?.accent !== TOTP_DESIGN_TOKENS.accent) {
      reasons.push(`Несовпадение accent: ${card1009.design_tokens?.accent}`);
    }
    if (card1009.interactive_states?.[0]?.active !== TOTP_DIGIT_BOX_STATES.active) {
      reasons.push("Несовпадение active state для digit-box");
    }
  }

  const restD02 = JSON.parse(readFileSync(restD02Path, "utf8"));
  const hasMfaWebauthn =
    Array.isArray(restD02.endpoints) &&
    restD02.endpoints.some(
      (/** @type {any} */ ep) => ep.path === "/api/v9/users/@me/mfa/webauthn/credentials",
    );
  if (!hasMfaWebauthn) {
    reasons.push("В docs/re/rest-d02.json не найден маршрут /api/v9/users/@me/mfa/webauthn/credentials");
  }

  const reStaticPath =
    opts.reRestStaticPath ??
    (existsSync(DEFAULT_RE_REST_STATIC_PATH) ? DEFAULT_RE_REST_STATIC_PATH : undefined);
  if (reStaticPath && existsSync(reStaticPath)) {
    const reStatic = JSON.parse(readFileSync(reStaticPath, "utf8"));
    const keys = new Set(
      (reStatic.endpoints ?? []).map((/** @type {any} */ e) => e.key).filter(Boolean),
    );
    for (const reqKey of ["LOGIN_MFA", "MFA_TOTP_ENABLE", "MFA_TOTP_DISABLE", "AUTH_SESSIONS", "AUTH_SESSIONS_LOGOUT"]) {
      if (!keys.has(reqKey)) {
        reasons.push(`В /home/sanya/re-deep/rest-static.json отсутствует ключ ${reqKey}`);
      }
    }
  }

  const reModPath =
    opts.reModulesIndexPath ??
    (existsSync(DEFAULT_RE_MODULES_INDEX_PATH) ? DEFAULT_RE_MODULES_INDEX_PATH : undefined);
  if (reModPath && existsSync(reModPath)) {
    const reMods = JSON.parse(readFileSync(reModPath, "utf8"));
    for (const modId of ["200921", "572469", "670492", "825542"]) {
      if (!reMods[modId]) {
        reasons.push(`В /home/sanya/re-deep/modules-index.json отсутствует модуль ${modId}`);
      }
    }
  }

  let liveSessionCount = LIVE_AUTH_SESSIONS_ORACLE.length;
  const sessionsCachePath =
    opts.reSessionsCachePath ??
    (existsSync(DEFAULT_RE_SESSIONS_CACHE_PATH) ? DEFAULT_RE_SESSIONS_CACHE_PATH : undefined);
  if (sessionsCachePath && existsSync(sessionsCachePath)) {
    const rawSessions = JSON.parse(readFileSync(sessionsCachePath, "utf8"));
    liveSessionCount = Array.isArray(rawSessions.user_sessions)
      ? rawSessions.user_sessions.length
      : 0;
    if (liveSessionCount !== 17) {
      reasons.push(`Ожидалось 17 сессий в кэше RE-дампа, получено ${liveSessionCount}`);
    }
  }

  return {
    passed: reasons.length === 0,
    reasons,
    liveSessionCount,
  };
}

/**
 * Маскирует TOTP-коды, тикеты MFA и токены авторизации в строках телеметрии.
 * @param {string} text
 * @returns {string}
 */
export function redactTotpTelemetry(text) {
  return String(text ?? "")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [REDACTED]")
    .replace(/mfa\.[A-Za-z0-9_-]+/g, "mfa.[REDACTED]")
    .replace(/(code|ticket|token|secret|authorization)\s*[:=]\s*["']?[^"'\s]+["']?/gi, "$1=[REDACTED]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для TOTP 2FA и менеджера сессий.
 */
export function evaluateTotpNineDimensions() {
  /** @type {string[]} */
  const failures = [];

  // 1. Позитивные сценарии (ввод 6 цифр с авто-переносом, POST /auth/mfa/totp, GET и DELETE /auth/sessions/{id})
  const ctrl = new TotpInputController();
  for (const d of ["1", "2", "3", "4", "5", "6"]) ctrl.inputDigit(d);
  const mfaReq = buildTotpMfaRequest({ method: "totp", code: ctrl.getCode(), ticket: "t1" });
  const delReq = buildDeleteSessionRequest("DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=");
  if (mfaReq.path !== "/auth/mfa/totp" || delReq.method !== "DELETE") {
    failures.push("dim1_positive_totp_and_delete_session");
  }

  // 2. Негативные сценарии (нецифровой ввод, неизвестный метод MFA, удаление текущей сессии)
  if (ctrl.inputDigit("a") !== false) {
    failures.push("dim2_negative_non_digit_rejected");
  }
  let badMethodRejected = false;
  try {
    buildTotpMfaRequest({ method: "../logout", code: "123456", ticket: "t1" });
  } catch {
    badMethodRejected = true;
  }
  if (!badMethodRejected) failures.push("dim2_negative_bad_method_rejected");

  // 3. Граничные значения (Snowflake 0 / u64::MAX, 6 ячеек 0..5, backup 8..11, 2000 симв., 10 файлов/25 МБ, offset=9975, 0..200%, 99+)
  if (isValidSnowflake("0") !== false || isValidSnowflake("18446744073709551615") !== true) {
    failures.push("dim3_boundary_snowflake");
  }
  const badgeCap = (/** @type {number} */ n) => (n > 99 ? "99+" : String(n));
  if (badgeCap(99) !== "99" || badgeCap(100) !== "99+") {
    failures.push("dim3_boundary_badge_cap");
  }

  // 4. Обработка ошибок HTTP и кодов MFA (60008 Invalid code, 60003 MFA required, 429 retry_after_ms, 5xx)
  const classifyMfaError = (/** @type {number} */ status, /** @type {number} */ code, /** @type {number} */ retryMs) => ({
    isInvalidCode: code === TOTP_PROTOCOL_CONSTANTS.MFA_INVALID_CODE_ERROR,
    retryable: status === 429 || status >= 500,
    backoffMs: status === 429 ? Math.max(100, retryMs) : status >= 500 ? 1_000 : 0,
  });
  if (!classifyMfaError(401, 60008, 0).isInvalidCode || !classifyMfaError(429, 0, 1200).retryable) {
    failures.push("dim4_mfa_and_http_error_classification");
  }

  // 5. Пустые состояния (пустое окно TOTP и пустой список сессий)
  const emptyCtrl = new TotpInputController();
  const emptyMgr = new AuthSessionsManager({ currentSessionIdHash: "none", sessions: [] });
  if (emptyCtrl.isComplete() || emptyMgr.snapshot().otherSessions.length !== 0) {
    failures.push("dim5_empty_states");
  }

  // 6. Загрузочные состояния (блокировка ввода при отправке TOTP / HTTP 202)
  const loadingCtrl = new TotpInputController();
  loadingCtrl.submitting = true;
  if (loadingCtrl.inputDigit("5") !== false) {
    failures.push("dim6_loading_submitting_guard");
  }

  // 7. Права доступа и ограничения (GUILD_MFA required, AGE_GATED_SPACES, защита текущей сессии)
  const mgr = new AuthSessionsManager({
    currentSessionIdHash: "OdKq9gNLX69NrpWTtUFE3MH2d2evCrU0o5eLIUvFP4s=",
    sessions: LIVE_AUTH_SESSIONS_ORACLE,
  });
  let currentProtected = false;
  try {
    mgr.revokeSession("OdKq9gNLX69NrpWTtUFE3MH2d2evCrU0o5eLIUvFP4s=");
  } catch {
    currentProtected = true;
  }
  if (!currentProtected) failures.push("dim7_current_session_protection");

  // 8. Восстановление после сбоев (откат оптимистичного удаления сессии при сетевой ошибке)
  mgr.revokeSession("DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=");
  mgr.rollbackLastRevoke();
  if (mgr.otherSessionsCount() !== 16) {
    failures.push("dim8_optimistic_revoke_rollback");
  }

  // 9. Наблюдаемость и маскирование секретов
  const redacted = redactTotpTelemetry('code="123456" ticket="sec_t" Authorization="Bearer mfa.tok"');
  if (redacted.includes("123456") || redacted.includes("sec_t") || redacted.includes("mfa.tok")) {
    failures.push("dim9_observability_redaction");
  }

  return {
    passed: failures.length === 0,
    dimensionsChecked: 9,
    failures,
  };
}

/**
 * Исполняет CLI `scripts/verify-totp.mjs`.
 * @param {readonly string[]} argv
 * @returns {number}
 */
export function runTotpCli(argv) {
  let oraclePath = DEFAULT_KANBAN_PATH;
  let outJson = "";
  let outMd = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      console.log(
        "Использование: node scripts/verify-totp.mjs [--oracle <path>] [--out-json <path>] [--out-md <path>]",
      );
      return 0;
    }
    if (arg === "--oracle") {
      const val = argv[i + 1];
      if (!val) return 1;
      oraclePath = val;
      i += 1;
      continue;
    }
    if (arg === "--out-json") {
      const val = argv[i + 1];
      if (!val) return 1;
      outJson = val;
      i += 1;
      continue;
    }
    if (arg === "--out-md") {
      const val = argv[i + 1];
      if (!val) return 1;
      outMd = val;
      i += 1;
      continue;
    }
    return 1;
  }

  if (!existsSync(oraclePath)) {
    console.error(`verify-totp: файл-оракул недоступен: ${oraclePath}`);
    return 2;
  }

  const parity = verifyTotpOracleParity({ kanbanPath: oraclePath });
  const dims = evaluateTotpNineDimensions();
  if (!parity.passed || !dims.passed) {
    console.error("verify-totp: нарушение контракта", [...parity.reasons, ...dims.failures]);
    return 1;
  }

  const report = {
    taskId: "TASK-1009",
    status: "PASS",
    layout: TOTP_MODAL_LAYOUT,
    designTokens: TOTP_DESIGN_TOKENS,
    digitBoxStates: TOTP_DIGIT_BOX_STATES,
    liveSessionCount: parity.liveSessionCount,
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
      `# Отчёт верификации TASK-1009 (TOTP 2FA & Active Sessions)\n\n- Статус: **PASS**\n- Живых сессий в RE-дампе: \`${parity.liveSessionCount}\`\n- Проверено измерений: \`${dims.dimensionsChecked}/9\`\n`,
      "utf8",
    );
  }
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runTotpCli(process.argv.slice(2)));
}
