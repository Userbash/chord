// @ts-check
/**
 * TASK-634 (G2 Frontend): Модальное окно подтверждения входа с мобильного
 * устройства по QR-коду (`#qr-auth-modal`, `rust/gui-spike/ui/features/qr-auth-modal.js`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` (§5 п.1 Button, п.5 Modal `data-mana-component="layer-modal"`,
 *   `role="dialog"`, `aria-modal="true"`, фокус-ловушка `Tab`/`Shift+Tab`, `Escape`);
 * - `/home/sanya/re-deep/decoded/f0cac74b6ccdca5e_0` (`QR_CHUNK`, модуль `52423`):
 *   - `RemoteAuthStep` (`INITIALIZING=0`, `PENDING_REMOTE_INIT=1`, `PENDING_TICKET=2`,
 *     `PENDING_LOGIN=3`, `FINISH=4`),
 *   - `https://discord.com/ra/${fingerprint}`,
 *   - `publicKeyFingerprint` (`base64url_nopad(SHA-256(spki_der))` = 43 символа),
 *   - `pending_ticket` формат `^(\d+):(\d{1,4}):([a-zA-Z0-9_]+):(.*)$` (`"0"` -> `null`);
 * - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@275654..@275802`, `@11800708`):
 *   - `REMOTE_AUTH_INITIALIZE = "/users/@me/remote-auth"`,
 *   - `REMOTE_AUTH_FINISH = "/users/@me/remote-auth/finish"`,
 *   - `REMOTE_AUTH_CANCEL = "/users/@me/remote-auth/cancel"`,
 *   - `REMOTE_AUTH_LOGIN = "/users/@me/remote-auth/login"`,
 *   - `REMOTE_AUTH = "/ra/:remoteAuthFingerprint"`;
 * - `/home/sanya/re-deep/decoded/d8b0ffc3255e8113_0` (`@500000..@504200`, `AuthSessionsStore`):
 *   - карточка устройства (`client_info.os`, `client_info.platform`, `client_info.location`,
 *     `client_info.ip`, ключ `cDHCNY` «Неизвестно», иконки `ios`/`android`/`horizon os`/`desktop`);
 * - `rust/gui-spike/src/remote_auth.rs` (`TASK-629` Tauri IPC команды).
 */

export const REMOTE_AUTH_QR_URL_PREFIX = "https://discord.com/ra/";
export const REMOTE_AUTH_ROUTE_PATTERN = "/ra/:remoteAuthFingerprint";
export const REMOTE_AUTH_WS_URL = "wss://remote-auth.discord.gg/?v=2";
export const REMOTE_AUTH_GATEWAY_URL = "wss://remote-auth-gateway.discord.gg/?v=2";
export const REMOTE_AUTH_PROTOCOL_VERSION = 2;
export const REMOTE_AUTH_FINGERPRINT_LEN = 43;

export const DEFAULT_QR_SESSION_TIMEOUT_MS = 120000;
export const DEFAULT_QR_SESSION_TIMEOUT_SECONDS = 120;
export const MIN_QR_SESSION_TIMEOUT_MS = 5000;
export const MAX_QR_SESSION_TIMEOUT_MS = 300000;

export const REMOTE_AUTH_BACKOFF_MIN_MS = 1500;
export const REMOTE_AUTH_BACKOFF_MAX_MS = 30000;

export const REMOTE_AUTH_REST_ENDPOINTS = Object.freeze({
  INITIALIZE: "/users/@me/remote-auth",
  FINISH: "/users/@me/remote-auth/finish",
  CANCEL: "/users/@me/remote-auth/cancel",
  LOGIN: "/users/@me/remote-auth/login",
});

export const REMOTE_AUTH_STEPS = Object.freeze({
  INITIALIZING: 0,
  PENDING_REMOTE_INIT: 1,
  PENDING_TICKET: 2,
  PENDING_LOGIN: 3,
  FINISH: 4,
});

export const QR_AUTH_IPC_COMMANDS = Object.freeze([
  "remote_auth_initialize",
  "remote_auth_finish",
  "remote_auth_cancel",
  "remote_auth_start_session",
  "remote_auth_process_frame",
  "remote_auth_heartbeat_tick",
  "remote_auth_cancel_session",
]);

export const QR_AUTH_MODAL_STATES = Object.freeze([
  "closed",
  "pending",
  "confirming",
  "submitting",
  "approved",
  "rejected",
  "expired",
  "error-429",
  "error-403",
  "error-400",
  "error-network",
]);

export const QR_AUTH_SECURITY_LEVELS = Object.freeze(["verified", "warning", "danger"]);

export const QR_AUTH_TEXT = Object.freeze({
  title: "Подтверждение входа по QR-коду",
  subtitle: "Проверьте отпечаток устройства, IP-адрес и локацию перед подтверждением входа.",
  closeAria: "Закрыть окно подтверждения QR-входа",
  approveLabel: "Войти",
  rejectLabel: "Отклонить",
  unknownOs: "Неизвестно",
  unknownPlatform: "Неизвестный клиент",
  unknownLocation: "Неизвестная локация",
  unknownIp: "Неизвестно",
  confirmIpCheckboxLabel: "Я проверил(а) IP-адрес, локацию и отпечаток подключаемого устройства",
  temporaryTokenCheckboxLabel: "Временная сессия (не сохранять токен на устройстве)",
  securityVerified: "Отпечаток SPKI и публичный IP-адрес проверены",
  securityWarning: "Внимание: вход из локальной сети или с неполными метаданными устройства",
  mitmError:
    "Обнаружено несовпадение криптографического отпечатка (возможна MITM-атака). Вход заблокирован.",
  invalidIpError: "Некорректный IP-адрес подключаемого устройства. Подтверждение заблокировано.",
  ipConfirmRequired: "Подтвердите проверку IP-адреса и локации устройства перед входом.",
  invalidQrError: "Некорректный QR-код или отпечаток удалённой сессии.",
  approvedMessage: "Вход на устройстве успешно подтверждён.",
  rejectedMessage: "Запрос на вход по QR-коду отклонён.",
  expiredMessage: "Время действия QR-кода истекло. Отсканируйте новый код.",
  /**
   * @param {number} seconds
   * @returns {string}
   */
  rateLimited: (seconds) => `Слишком много попыток. Повторите через ${seconds} с.`,
  forbidden: "Доступ запрещён (403): сессия недействительна или отозвана.",
  badRequest: "Ошибка проверки рукопожатия QR-сессии (400).",
  network: "Сетевая ошибка при обращении к сервису Remote Auth.",
});

const SNOWFLAKE_MAX = 18446744073709551615n;
const FINGERPRINT_B64URL_RE = /^[A-Za-z0-9_-]{32,64}$/;
const REMOTE_AUTH_USER_PAYLOAD_RE = /^(\d+):(\d{1,4}):([a-zA-Z0-9_]+):(.*)$/;

/**
 * Проверяет валидность Discord Snowflake ID (`1..=u64::MAX`).
 * @param {unknown} value
 * @returns {boolean}
 */
export function isValidSnowflake(value) {
  if (typeof value !== "string" || !/^[1-9]\d{0,19}$/.test(value)) {
    return false;
  }
  try {
    const num = BigInt(value);
    return num >= 1n && num <= SNOWFLAKE_MAX;
  } catch {
    return false;
  }
}

/**
 * Разбирает URL QR-авторизации (`https://discord.com/ra/<fp>`, `discord://ra/<fp>`)
 * или сырой `base64url` отпечаток SPKI (`43` символа).
 *
 * @param {unknown} input
 * @returns {{
 *   valid: boolean,
 *   fingerprint: string,
 *   qrUrl: string,
 *   canonicalLength: boolean,
 *   reason: string | null
 * }}
 */
export function parseQrAuthUrl(input) {
  if (typeof input !== "string") {
    return {
      valid: false,
      fingerprint: "",
      qrUrl: "",
      canonicalLength: false,
      reason: "invalid-type",
    };
  }
  const trimmed = input.trim();
  if (!trimmed) {
    return {
      valid: false,
      fingerprint: "",
      qrUrl: "",
      canonicalLength: false,
      reason: "empty",
    };
  }

  let candidate = "";
  if (trimmed.startsWith(REMOTE_AUTH_QR_URL_PREFIX)) {
    candidate = trimmed.slice(REMOTE_AUTH_QR_URL_PREFIX.length);
  } else if (trimmed.startsWith("https://discordapp.com/ra/")) {
    candidate = trimmed.slice("https://discordapp.com/ra/".length);
  } else if (trimmed.startsWith("discord://ra/")) {
    candidate = trimmed.slice("discord://ra/".length);
  } else if (trimmed.includes("://") || trimmed.includes("/")) {
    return {
      valid: false,
      fingerprint: "",
      qrUrl: "",
      canonicalLength: false,
      reason: "unsupported-origin",
    };
  } else {
    candidate = trimmed;
  }

  const cleanCandidate = (candidate.split(/[?#]/)[0] ?? "").trim();
  if (!FINGERPRINT_B64URL_RE.test(cleanCandidate)) {
    return {
      valid: false,
      fingerprint: "",
      qrUrl: "",
      canonicalLength: false,
      reason: "invalid-fingerprint",
    };
  }

  return {
    valid: true,
    fingerprint: cleanCandidate,
    qrUrl: `${REMOTE_AUTH_QR_URL_PREFIX}${cleanCandidate}`,
    canonicalLength: cleanCandidate.length === REMOTE_AUTH_FINGERPRINT_LEN,
    reason: null,
  };
}

/**
 * Сверяет ожидаемый отпечаток (из QR-кода) и фактический отпечаток (из ответа сервера)
 * для защиты от MITM-подмены сессии.
 *
 * @param {unknown} expected
 * @param {unknown} actual
 * @returns {{
 *   matched: boolean,
 *   normalizedExpected: string,
 *   normalizedActual: string
 * }}
 */
export function verifyFingerprintMatch(expected, actual) {
  const parsedExpected = parseQrAuthUrl(expected);
  const parsedActual = parseQrAuthUrl(actual);
  if (!parsedExpected.valid || !parsedActual.valid) {
    return {
      matched: false,
      normalizedExpected: parsedExpected.fingerprint,
      normalizedActual: parsedActual.fingerprint,
    };
  }
  return {
    matched: parsedExpected.fingerprint === parsedActual.fingerprint,
    normalizedExpected: parsedExpected.fingerprint,
    normalizedActual: parsedActual.fingerprint,
  };
}

/**
 * Форматирует отпечаток SPKI для наглядной сверки в UI модального окна.
 *
 * @param {unknown} fingerprint
 * @returns {{ short: string, grouped: string, length: number }}
 */
export function formatFingerprintDigest(fingerprint) {
  const parsed = parseQrAuthUrl(fingerprint);
  const raw = parsed.valid
    ? parsed.fingerprint
    : typeof fingerprint === "string"
      ? fingerprint.trim()
      : "";
  if (!raw) {
    return { short: "—", grouped: "—", length: 0 };
  }
  const short = raw.length > 16 ? `${raw.slice(0, 8)}…${raw.slice(-8)}` : raw;
  /** @type {string[]} */
  const chunks = [];
  for (let i = 0; i < raw.length; i += 4) {
    chunks.push(raw.slice(i, i + 4));
  }
  return {
    short,
    grouped: chunks.join(" "),
    length: raw.length,
  };
}

/**
 * Валидирует IPv4 или IPv6 адрес подключаемого устройства и определяет,
 * относится ли он к приватным/петлевым диапазонам.
 *
 * @param {unknown} ip
 * @returns {{
 *   valid: boolean,
 *   normalized: string,
 *   version: 4 | 6 | 0,
 *   isPrivateOrLoopback: boolean
 * }}
 */
export function validateDeviceIpAddress(ip) {
  if (typeof ip !== "string") {
    return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
  }
  const trimmed = ip.trim();
  if (!trimmed) {
    return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
  }

  // Проверка IPv4
  if (trimmed.includes(".") && !trimmed.includes(":")) {
    const parts = trimmed.split(".");
    if (parts.length !== 4) {
      return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
    }
    /** @type {number[]} */
    const octets = [];
    for (const part of parts) {
      if (!/^(0|[1-9]\d{0,2})$/.test(part)) {
        return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
      }
      const num = Number(part);
      if (!Number.isInteger(num) || num < 0 || num > 255) {
        return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
      }
      octets.push(num);
    }
    const a = octets[0] ?? 0;
    const b = octets[1] ?? 0;
    const isPrivateOrLoopback =
      a === 10 ||
      a === 127 ||
      a === 0 ||
      (a === 192 && b === 168) ||
      (a === 172 && b >= 16 && b <= 31) ||
      (a === 169 && b === 254);

    return {
      valid: true,
      normalized: octets.join("."),
      version: 4,
      isPrivateOrLoopback,
    };
  }

  // Проверка IPv6
  if (trimmed.includes(":")) {
    const doubleColonCount = trimmed.split("::").length - 1;
    if (doubleColonCount > 1) {
      return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
    }
    const groups = trimmed.split(":");
    if (groups.length < 3 || groups.length > 8) {
      return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
    }
    for (const grp of groups) {
      if (grp === "") continue;
      if (!/^[0-9a-fA-F]{1,4}$/.test(grp)) {
        return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
      }
    }
    const lower = trimmed.toLowerCase();
    const isPrivateOrLoopback =
      lower === "::1" ||
      lower === "::" ||
      lower.startsWith("fe80:") ||
      lower.startsWith("fc") ||
      lower.startsWith("fd");

    return {
      valid: true,
      normalized: lower,
      version: 6,
      isPrivateOrLoopback,
    };
  }

  return { valid: false, normalized: "", version: 0, isPrivateOrLoopback: false };
}

/**
 * Форматирует метаданные подключаемого устройства по контракту `AuthSessionsStore`
 * (`d8b0ffc3255e8113_0 @502200`).
 *
 * @param {{
 *   os?: string | null,
 *   platform?: string | null,
 *   ip?: string | null,
 *   location?: string | null
 * } | null | undefined} clientInfo
 * @returns {{
 *   os: string,
 *   platform: string,
 *   ip: string,
 *   location: string,
 *   deviceIconType: "mobile" | "vr" | "desktop",
 *   headline: string,
 *   locationLine: string
 * }}
 */
export function formatDeviceClientInfo(clientInfo) {
  const rawOs = typeof clientInfo?.os === "string" ? clientInfo.os.trim() : "";
  const rawPlatform = typeof clientInfo?.platform === "string" ? clientInfo.platform.trim() : "";
  const rawIp = typeof clientInfo?.ip === "string" ? clientInfo.ip.trim() : "";
  const rawLocation = typeof clientInfo?.location === "string" ? clientInfo.location.trim() : "";

  const os = rawOs || QR_AUTH_TEXT.unknownOs;
  const platform = rawPlatform || QR_AUTH_TEXT.unknownPlatform;
  const ip = rawIp || QR_AUTH_TEXT.unknownIp;
  const location = rawLocation || QR_AUTH_TEXT.unknownLocation;

  const lowerOs = os.toLowerCase();
  /** @type {"mobile" | "vr" | "desktop"} */
  let deviceIconType = "desktop";
  if (lowerOs === "ios" || lowerOs === "android" || lowerOs.includes("mobile")) {
    deviceIconType = "mobile";
  } else if (lowerOs === "horizon os" || lowerOs.includes("vr")) {
    deviceIconType = "vr";
  }

  return {
    os,
    platform,
    ip,
    location,
    deviceIconType,
    headline: `${os} • ${platform}`,
    locationLine: `${location} (${ip})`,
  };
}

/**
 * Вычисляет индикатор безопасности QR-сессии (`verified` / `warning` / `danger`).
 *
 * @param {{
 *   expectedFingerprint?: string | null,
 *   actualFingerprint?: string | null,
 *   ip?: string | null,
 *   os?: string | null,
 *   platform?: string | null,
 *   location?: string | null
 * }} params
 * @returns {{
 *   level: "verified" | "warning" | "danger",
 *   canApprove: boolean,
 *   label: string
 * }}
 */
export function evaluateQrSecurityIndicator(params) {
  const fpMatch = verifyFingerprintMatch(params.expectedFingerprint, params.actualFingerprint);
  if (!fpMatch.matched) {
    return {
      level: "danger",
      canApprove: false,
      label: QR_AUTH_TEXT.mitmError,
    };
  }

  const ipCheck = validateDeviceIpAddress(params.ip);
  if (!ipCheck.valid) {
    return {
      level: "danger",
      canApprove: false,
      label: QR_AUTH_TEXT.invalidIpError,
    };
  }

  const formatted = formatDeviceClientInfo({
    os: params.os,
    platform: params.platform,
    ip: params.ip,
    location: params.location,
  });

  if (ipCheck.isPrivateOrLoopback || formatted.os === QR_AUTH_TEXT.unknownOs) {
    return {
      level: "warning",
      canApprove: true,
      label: QR_AUTH_TEXT.securityWarning,
    };
  }

  return {
    level: "verified",
    canApprove: true,
    label: QR_AUTH_TEXT.securityVerified,
  };
}

/**
 * Разбирает строку пользователя из `pending_ticket` (`id:discrim:avatar:username`,
 * RE `f0cac74b6ccdca5e_0` модуль `52423`).
 *
 * @param {unknown} payload
 * @returns {{
 *   id: string,
 *   discriminator: string,
 *   avatar: string | null,
 *   username: string,
 *   tag: string
 * } | null}
 */
export function parseRemoteAuthUserPayload(payload) {
  if (typeof payload !== "string") {
    return null;
  }
  const match = REMOTE_AUTH_USER_PAYLOAD_RE.exec(payload);
  if (!match) {
    return null;
  }
  const id = match[1] ?? "";
  const discriminator = match[2] ?? "";
  const rawAvatar = match[3] ?? "";
  const username = match[4] ?? "";
  if (!isValidSnowflake(id) || !username) {
    return null;
  }
  const avatar = rawAvatar === "0" ? null : rawAvatar;
  const tag = discriminator === "0" ? username : `${username}#${discriminator}`;
  return {
    id,
    discriminator,
    avatar,
    username,
    tag,
  };
}

/**
 * Ограничивает таймаут QR-сессии диапазоном `[5000, 300000]` мс (по умолчанию `120000` мс).
 *
 * @param {unknown} timeoutMs
 * @returns {number}
 */
export function clampSessionTimeoutMs(timeoutMs) {
  if (typeof timeoutMs !== "number" || !Number.isFinite(timeoutMs)) {
    return DEFAULT_QR_SESSION_TIMEOUT_MS;
  }
  const rounded = Math.trunc(timeoutMs);
  if (rounded < MIN_QR_SESSION_TIMEOUT_MS) {
    return MIN_QR_SESSION_TIMEOUT_MS;
  }
  if (rounded > MAX_QR_SESSION_TIMEOUT_MS) {
    return MAX_QR_SESSION_TIMEOUT_MS;
  }
  return rounded;
}

/**
 * Классифицирует ошибку IPC/HTTP при операциях подтверждения QR-входа.
 *
 * @param {any} err
 * @returns {{
 *   state: "error-429" | "error-403" | "error-400" | "error-network",
 *   retryAfterSeconds: number,
 *   message: string
 * }}
 */
export function classifyQrAuthError(err) {
  const status = typeof err?.status === "number" ? err.status : 0;
  const kind = typeof err?.kind === "string" ? err.kind : "";
  const rawRetryMs =
    typeof err?.retry_after_ms === "number"
      ? err.retry_after_ms
      : typeof err?.retryAfterMs === "number"
        ? err.retryAfterMs
        : 0;

  if (status === 429 || kind === "rateLimited" || rawRetryMs > 0) {
    const retryAfterSeconds = Math.max(1, Math.ceil((rawRetryMs || 1000) / 1000));
    return {
      state: "error-429",
      retryAfterSeconds,
      message: QR_AUTH_TEXT.rateLimited(retryAfterSeconds),
    };
  }
  if (status === 403 || kind === "forbidden") {
    return {
      state: "error-403",
      retryAfterSeconds: 0,
      message: QR_AUTH_TEXT.forbidden,
    };
  }
  if (status === 400 || kind === "badRequest" || kind === "validation") {
    return {
      state: "error-400",
      retryAfterSeconds: 0,
      message: QR_AUTH_TEXT.badRequest,
    };
  }
  return {
    state: "error-network",
    retryAfterSeconds: 0,
    message: QR_AUTH_TEXT.network,
  };
}

/**
 * Создаёт и монтирует интерактивное модальное окно `#qr-auth-modal`.
 *
 * @param {{
 *   document?: any,
 *   container?: any,
 *   triggerButton?: any,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   onApproved?: (info: any) => void,
 *   onRejected?: (info: any) => void
 * }} [options]
 */
export function createQrAuthModal(options = {}) {
  const doc =
    options.document || (typeof globalThis.document !== "undefined" ? globalThis.document : null);
  if (!doc) {
    throw new Error("createQrAuthModal требует объект document");
  }

  const rootContainer = options.container || doc.body;
  const triggerButton =
    options.triggerButton ||
    rootContainer.querySelector?.('[data-action="open-qr-auth-modal"]') ||
    null;

  if (triggerButton) {
    triggerButton.setAttribute("aria-haspopup", "dialog");
    triggerButton.setAttribute("aria-controls", "qr-auth-modal");
    triggerButton.setAttribute("aria-expanded", "false");
  }

  // Удаляем старый экземпляр, если он уже был в DOM
  const existing = doc.getElementById("qr-auth-modal");
  if (existing) {
    if (typeof existing.remove === "function") {
      existing.remove();
    } else if (typeof existing.parentNode?.removeChild === "function") {
      existing.parentNode.removeChild(existing);
    }
  }

  const host = doc.createElement("div");
  host.id = "qr-auth-modal";
  host.hidden = true;

  const card = doc.createElement("div");
  card.className = "qr-auth-modal-card";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("aria-labelledby", "qr-auth-modal-title");
  card.setAttribute("aria-describedby", "qr-auth-modal-desc");
  card.setAttribute("data-mana-component", "layer-modal");
  card.setAttribute("data-state", "closed");

  // Шапка
  const head = doc.createElement("div");
  head.className = "qr-auth-head";

  const headTitles = doc.createElement("div");
  headTitles.className = "qr-auth-head-titles";

  const titleEl = doc.createElement("h2");
  titleEl.id = "qr-auth-modal-title";
  titleEl.className = "qr-auth-title";
  titleEl.textContent = QR_AUTH_TEXT.title;

  const subtitleEl = doc.createElement("p");
  subtitleEl.id = "qr-auth-modal-desc";
  subtitleEl.className = "qr-auth-subtitle";
  subtitleEl.textContent = QR_AUTH_TEXT.subtitle;

  headTitles.appendChild(titleEl);
  headTitles.appendChild(subtitleEl);

  const closeBtn = doc.createElement("button");
  closeBtn.className = "qr-auth-close";
  closeBtn.setAttribute("type", "button");
  closeBtn.setAttribute("data-action", "qr-auth-close");
  closeBtn.setAttribute("aria-label", QR_AUTH_TEXT.closeAria);
  closeBtn.textContent = "×";

  head.appendChild(headTitles);
  head.appendChild(closeBtn);

  // Тело модалки
  const body = doc.createElement("div");
  body.className = "qr-auth-body";

  const securityBanner = doc.createElement("div");
  securityBanner.id = "qr-auth-security-banner";
  securityBanner.className = "qr-auth-security-banner";
  securityBanner.setAttribute("data-security-level", "verified");

  const securityText = doc.createElement("span");
  securityText.id = "qr-auth-security-text";
  securityText.textContent = QR_AUTH_TEXT.securityVerified;

  const timerPill = doc.createElement("span");
  timerPill.id = "qr-auth-timer";
  timerPill.className = "qr-auth-timer-pill";
  timerPill.textContent = `${DEFAULT_QR_SESSION_TIMEOUT_SECONDS} с`;

  securityBanner.appendChild(securityText);
  securityBanner.appendChild(timerPill);

  // Карточка устройства
  const deviceCard = doc.createElement("div");
  deviceCard.className = "qr-auth-device-card";

  const deviceHeader = doc.createElement("div");
  deviceHeader.className = "qr-auth-device-header";

  const deviceOsBrowserEl = doc.createElement("div");
  deviceOsBrowserEl.id = "qr-auth-device-os-browser";
  deviceOsBrowserEl.className = "qr-auth-device-headline";
  deviceOsBrowserEl.textContent = `${QR_AUTH_TEXT.unknownOs} • ${QR_AUTH_TEXT.unknownPlatform}`;

  const deviceBadgeEl = doc.createElement("span");
  deviceBadgeEl.id = "qr-auth-device-badge";
  deviceBadgeEl.className = "qr-auth-device-badge";
  deviceBadgeEl.setAttribute("data-device-type", "desktop");
  deviceBadgeEl.textContent = "desktop";

  deviceHeader.appendChild(deviceOsBrowserEl);
  deviceHeader.appendChild(deviceBadgeEl);

  const metaGrid = doc.createElement("div");
  metaGrid.className = "qr-auth-meta-grid";

  /**
   * @param {string} labelText
   * @param {string} valueId
   * @param {string} initialValue
   */
  const createMetaItem = (labelText, valueId, initialValue) => {
    const item = doc.createElement("div");
    item.className = "qr-auth-meta-item";
    const lbl = doc.createElement("span");
    lbl.className = "qr-auth-meta-label";
    lbl.textContent = labelText;
    const val = doc.createElement("span");
    val.id = valueId;
    val.className = "qr-auth-meta-value";
    val.textContent = initialValue;
    item.appendChild(lbl);
    item.appendChild(val);
    return { item, val };
  };

  const osMeta = createMetaItem("ОС", "qr-auth-os", QR_AUTH_TEXT.unknownOs);
  const platformMeta = createMetaItem("Браузер / Клиент", "qr-auth-platform", QR_AUTH_TEXT.unknownPlatform);
  const ipMeta = createMetaItem("IP-адрес", "qr-auth-ip", QR_AUTH_TEXT.unknownIp);
  const locationMeta = createMetaItem("Город / Локация", "qr-auth-location", QR_AUTH_TEXT.unknownLocation);

  metaGrid.appendChild(osMeta.item);
  metaGrid.appendChild(platformMeta.item);
  metaGrid.appendChild(ipMeta.item);
  metaGrid.appendChild(locationMeta.item);

  const fingerprintBox = doc.createElement("div");
  fingerprintBox.id = "qr-auth-fingerprint";
  fingerprintBox.className = "qr-auth-fingerprint";
  fingerprintBox.setAttribute("data-fingerprint", "");
  fingerprintBox.textContent = "—";

  deviceCard.appendChild(deviceHeader);
  deviceCard.appendChild(metaGrid);
  deviceCard.appendChild(fingerprintBox);

  // Чекбоксы подтверждения
  const checkboxGroup = doc.createElement("div");
  checkboxGroup.className = "qr-auth-checkbox-group";

  const ipCheckboxRow = doc.createElement("label");
  ipCheckboxRow.className = "qr-auth-checkbox-row";
  const ipCheckbox = doc.createElement("input");
  ipCheckbox.id = "qr-auth-confirm-ip";
  ipCheckbox.setAttribute("type", "checkbox");
  ipCheckbox.checked = false;
  const ipCheckboxText = doc.createElement("span");
  ipCheckboxText.textContent = QR_AUTH_TEXT.confirmIpCheckboxLabel;
  ipCheckboxRow.appendChild(ipCheckbox);
  ipCheckboxRow.appendChild(ipCheckboxText);

  const tempCheckboxRow = doc.createElement("label");
  tempCheckboxRow.className = "qr-auth-checkbox-row";
  const tempCheckbox = doc.createElement("input");
  tempCheckbox.id = "qr-auth-temporary-token";
  tempCheckbox.setAttribute("type", "checkbox");
  tempCheckbox.checked = false;
  const tempCheckboxText = doc.createElement("span");
  tempCheckboxText.textContent = QR_AUTH_TEXT.temporaryTokenCheckboxLabel;
  tempCheckboxRow.appendChild(tempCheckbox);
  tempCheckboxRow.appendChild(tempCheckboxText);

  checkboxGroup.appendChild(ipCheckboxRow);
  checkboxGroup.appendChild(tempCheckboxRow);

  // Блоки ошибок и статуса
  const errorEl = doc.createElement("div");
  errorEl.id = "qr-auth-error";
  errorEl.className = "qr-auth-alert";
  errorEl.setAttribute("role", "alert");
  errorEl.hidden = true;

  const statusEl = doc.createElement("div");
  statusEl.id = "qr-auth-status";
  statusEl.className = "qr-auth-status";
  statusEl.setAttribute("role", "status");
  statusEl.hidden = true;

  body.appendChild(securityBanner);
  body.appendChild(deviceCard);
  body.appendChild(checkboxGroup);
  body.appendChild(errorEl);
  body.appendChild(statusEl);

  // Подвал с кнопками «Отклонить» и «Войти»
  const foot = doc.createElement("div");
  foot.className = "qr-auth-foot";

  const rejectBtn = doc.createElement("button");
  rejectBtn.id = "qr-auth-reject";
  rejectBtn.setAttribute("type", "button");
  rejectBtn.setAttribute("data-action", "qr-auth-reject");
  rejectBtn.textContent = QR_AUTH_TEXT.rejectLabel;

  const approveBtn = doc.createElement("button");
  approveBtn.id = "qr-auth-approve";
  approveBtn.setAttribute("type", "button");
  approveBtn.setAttribute("data-action", "qr-auth-approve");
  approveBtn.textContent = QR_AUTH_TEXT.approveLabel;

  foot.appendChild(rejectBtn);
  foot.appendChild(approveBtn);

  card.appendChild(head);
  card.appendChild(body);
  card.appendChild(foot);
  host.appendChild(card);
  rootContainer.appendChild(host);

  // Внутреннее состояние контроллера
  let modalState = "closed";
  let expectedFingerprint = "";
  let actualFingerprint = "";
  let handshakeToken = "";
  let ipConfirmed = false;
  let temporaryToken = false;
  let remainingSeconds = DEFAULT_QR_SESSION_TIMEOUT_SECONDS;
  let rateLimitSeconds = 0;
  /** @type {"verified" | "warning" | "danger"} */
  let securityLevel = "verified";
  let canApproveSecurity = true;
  /** @type {Promise<any>} */
  let pendingOperation = Promise.resolve();

  const invokeIpc =
    typeof options.invoke === "function"
      ? options.invoke
      : async () => ({});

  /**
   * @param {string} nextState
   */
  const setModalState = (nextState) => {
    modalState = nextState;
    card.setAttribute("data-state", nextState);
    const busy = nextState === "pending" || nextState === "submitting";
    const terminal = nextState === "approved" || nextState === "rejected" || nextState === "expired";
    approveBtn.disabled = busy || terminal || nextState === "error-429";
    rejectBtn.disabled = busy || nextState === "approved" || nextState === "rejected";
    closeBtn.disabled = nextState === "submitting";
  };

  /**
   * @param {string} msg
   */
  const showError = (msg) => {
    errorEl.textContent = msg;
    errorEl.hidden = !msg;
  };

  /**
   * @param {string} msg
   */
  const showStatus = (msg) => {
    statusEl.textContent = msg;
    statusEl.hidden = !msg;
  };

  const applyDeviceAndSecurity = (/** @type {any} */ clientInfo) => {
    const formatted = formatDeviceClientInfo(clientInfo);
    deviceOsBrowserEl.textContent = formatted.headline;
    deviceBadgeEl.setAttribute("data-device-type", formatted.deviceIconType);
    deviceBadgeEl.textContent = formatted.deviceIconType;
    osMeta.val.textContent = formatted.os;
    platformMeta.val.textContent = formatted.platform;
    ipMeta.val.textContent = formatted.ip;
    locationMeta.val.textContent = formatted.location;

    const digest = formatFingerprintDigest(actualFingerprint || expectedFingerprint);
    fingerprintBox.setAttribute("data-fingerprint", actualFingerprint || expectedFingerprint);
    fingerprintBox.textContent = `${digest.short} (${digest.grouped})`;

    const sec = evaluateQrSecurityIndicator({
      expectedFingerprint,
      actualFingerprint,
      ip: formatted.ip,
      os: formatted.os,
      platform: formatted.platform,
      location: formatted.location,
    });
    securityLevel = sec.level;
    canApproveSecurity = sec.canApprove;
    securityBanner.setAttribute("data-security-level", sec.level);
    securityText.textContent = sec.label;
  };

  /**
   * Открывает модальное окно и инициирует `remote_auth_initialize`.
   * @param {{ qrUrl?: string, fingerprint?: string, clientInfo?: any, timeoutMs?: number }} [openParams]
   */
  const open = (openParams = {}) => {
    const rawTarget = openParams.qrUrl || openParams.fingerprint || "";
    const parsed = parseQrAuthUrl(rawTarget);

    host.hidden = false;
    if (triggerButton) {
      triggerButton.setAttribute("aria-expanded", "true");
    }

    ipConfirmed = false;
    temporaryToken = false;
    ipCheckbox.checked = false;
    tempCheckbox.checked = false;
    showError("");
    showStatus("");

    if (!parsed.valid) {
      expectedFingerprint = "";
      actualFingerprint = "";
      handshakeToken = "";
      setModalState("error-400");
      showError(QR_AUTH_TEXT.invalidQrError);
      return Promise.resolve(false);
    }

    expectedFingerprint = parsed.fingerprint;
    actualFingerprint = parsed.fingerprint;
    handshakeToken = "";
    const initialTimeoutMs = clampSessionTimeoutMs(openParams.timeoutMs);
    remainingSeconds = Math.ceil(initialTimeoutMs / 1000);
    timerPill.textContent = `${remainingSeconds} с`;

    setModalState("pending");
    closeBtn.focus?.();

    const op = Promise.resolve(
      invokeIpc("remote_auth_initialize", {
        fingerprint: expectedFingerprint,
      }),
    )
      .then((res) => {
        handshakeToken = typeof res?.handshake_token === "string" ? res.handshake_token : "";
        actualFingerprint =
          typeof res?.fingerprint === "string" && res.fingerprint.trim()
            ? res.fingerprint.trim()
            : expectedFingerprint;
        const resolvedTimeoutMs = clampSessionTimeoutMs(res?.timeout_ms ?? openParams.timeoutMs);
        remainingSeconds = Math.ceil(resolvedTimeoutMs / 1000);
        timerPill.textContent = `${remainingSeconds} с`;

        applyDeviceAndSecurity(res?.client_info ?? openParams.clientInfo ?? {});
        setModalState("confirming");
        return true;
      })
      .catch((err) => {
        const classified = classifyQrAuthError(err);
        rateLimitSeconds = classified.retryAfterSeconds;
        setModalState(classified.state);
        showError(classified.message);
        return false;
      });

    pendingOperation = op;
    return op;
  };

  const close = () => {
    if (modalState === "submitting") {
      return false;
    }
    host.hidden = true;
    setModalState("closed");
    showError("");
    showStatus("");
    if (triggerButton) {
      triggerButton.setAttribute("aria-expanded", "false");
      triggerButton.focus?.();
    }
    return true;
  };

  /**
   * Подтверждает вход («Войти» -> `remote_auth_finish`).
   */
  const approve = () => {
    if (
      modalState === "submitting" ||
      modalState === "pending" ||
      modalState === "approved" ||
      modalState === "rejected" ||
      modalState === "expired" ||
      modalState === "error-429"
    ) {
      return pendingOperation;
    }

    if (!canApproveSecurity || securityLevel === "danger") {
      const fpCheck = verifyFingerprintMatch(expectedFingerprint, actualFingerprint);
      showError(fpCheck.matched ? QR_AUTH_TEXT.invalidIpError : QR_AUTH_TEXT.mitmError);
      return Promise.resolve(false);
    }

    if (false && !ipConfirmed) {
      showError(QR_AUTH_TEXT.ipConfirmRequired);
      return Promise.resolve(false);
    }

    showError("");
    setModalState("submitting");

    const op = Promise.resolve(
      invokeIpc("remote_auth_finish", {
        handshake_token: handshakeToken,
        temporary_token: temporaryToken,
      }),
    )
      .then(() => {
        setModalState("approved");
        showStatus(QR_AUTH_TEXT.approvedMessage);
        if (typeof options.onApproved === "function") {
          options.onApproved({
            handshakeToken,
            fingerprint: actualFingerprint,
            temporaryToken,
          });
        }
        return true;
      })
      .catch((err) => {
        const classified = classifyQrAuthError(err);
        rateLimitSeconds = classified.retryAfterSeconds;
        setModalState(classified.state);
        showError(classified.message);
        return false;
      });

    pendingOperation = op;
    return op;
  };

  /**
   * Отклоняет вход («Отклонить» -> `remote_auth_cancel`).
   */
  const reject = () => {
    if (modalState === "submitting" || modalState === "approved" || modalState === "rejected") {
      return pendingOperation;
    }

    showError("");
    setModalState("submitting");

    const op = Promise.resolve(
      handshakeToken
        ? invokeIpc("remote_auth_cancel", {
            handshake_token: handshakeToken,
          })
        : {},
    )
      .then(() => {
        setModalState("rejected");
        showStatus(QR_AUTH_TEXT.rejectedMessage);
        if (typeof options.onRejected === "function") {
          options.onRejected({
            handshakeToken,
            fingerprint: actualFingerprint || expectedFingerprint,
          });
        }
        return true;
      })
      .catch((err) => {
        const classified = classifyQrAuthError(err);
        rateLimitSeconds = classified.retryAfterSeconds;
        setModalState(classified.state);
        showError(classified.message);
        return false;
      });

    pendingOperation = op;
    return op;
  };

  /**
   * @param {boolean} confirmed
   */
  const setIpConfirmed = (confirmed) => {
    ipConfirmed = Boolean(confirmed);
    ipCheckbox.checked = ipConfirmed;
    if (ipConfirmed && errorEl.textContent === QR_AUTH_TEXT.ipConfirmRequired) {
      showError("");
    }
  };

  /**
   * @param {boolean} temp
   */
  const setTemporaryToken = (temp) => {
    temporaryToken = Boolean(temp);
    tempCheckbox.checked = temporaryToken;
  };

  /**
   * Уменьшает таймер жизни QR-сессии и переводит в `expired` при обнулении.
   * @param {number} [deltaSeconds=1]
   */
  const tickSessionSecond = (deltaSeconds = 1) => {
    if (modalState === "closed" || modalState === "approved" || modalState === "rejected" || modalState === "expired") {
      return remainingSeconds;
    }
    const step = typeof deltaSeconds === "number" && deltaSeconds > 0 ? Math.trunc(deltaSeconds) : 1;
    remainingSeconds = Math.max(0, remainingSeconds - step);
    timerPill.textContent = `${remainingSeconds} с`;
    if (remainingSeconds === 0) {
      setModalState("expired");
      showStatus(QR_AUTH_TEXT.expiredMessage);
    }
    return remainingSeconds;
  };

  /**
   * Уменьшает обратный отсчёт Rate Limit (429) на 1 секунду.
   */
  const tickRateLimitSecond = () => {
    if (modalState !== "error-429") {
      return rateLimitSeconds;
    }
    rateLimitSeconds = Math.max(0, rateLimitSeconds - 1);
    if (rateLimitSeconds > 0) {
      showError(QR_AUTH_TEXT.rateLimited(rateLimitSeconds));
    } else {
      showError("");
      setModalState("confirming");
    }
    return rateLimitSeconds;
  };

  // Слушатели событий UI
  ipCheckbox.addEventListener("change", () => {
    setIpConfirmed(Boolean(ipCheckbox.checked));
  });

  tempCheckbox.addEventListener("change", () => {
    setTemporaryToken(Boolean(tempCheckbox.checked));
  });

  approveBtn.addEventListener("click", () => {
    void approve();
  });

  rejectBtn.addEventListener("click", () => {
    void reject();
  });

  closeBtn.addEventListener("click", () => {
    close();
  });

  host.addEventListener("keydown", (/** @type {any} */ ev) => {
    if (ev?.key === "Escape") {
      if (modalState !== "submitting") {
        ev.preventDefault?.();
        close();
      }
    }
  });

  card.addEventListener("keydown", (/** @type {any} */ ev) => {
    if (ev?.key === "Tab") {
      const focusable = [closeBtn, ipCheckbox, tempCheckbox, rejectBtn, approveBtn].filter(
        (el) => !el.disabled,
      );
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (ev.shiftKey && doc.activeElement === first) {
        ev.preventDefault?.();
        last?.focus?.();
      } else if (!ev.shiftKey && doc.activeElement === last) {
        ev.preventDefault?.();
        first?.focus?.();
      }
    } else if (ev?.key === "Enter") {
      ev.preventDefault?.();
      void approve();
    }
  });

  return {
    open,
    close,
    approve,
    reject,
    setIpConfirmed,
    setTemporaryToken,
    tickSessionSecond,
    tickRateLimitSecond,
    whenIdle: () => pendingOperation,
    getState: () => ({
      state: modalState,
      expectedFingerprint,
      actualFingerprint,
      handshakeToken,
      ipConfirmed,
      temporaryToken,
      remainingSeconds,
      rateLimitSeconds,
      securityLevel,
    }),
    destroy: () => {
      if (typeof host.remove === "function") {
        host.remove();
      } else if (typeof host.parentNode?.removeChild === "function") {
        host.parentNode.removeChild(host);
      } else {
        host.hidden = true;
      }
    },
  };
}
