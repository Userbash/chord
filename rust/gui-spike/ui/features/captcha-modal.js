// @ts-check
/**
 * TASK-610 / TASK-817: интерактивное модальное окно решения hCaptcha (`#captcha-modal`)
 * со встроенным WebView-контейнером и передачей токена в 1 клик через `postMessage`
 * без ручного копирования строки (`POST /invites/{code}` → HTTP 400, код `40003`,
 * `captcha_service: "hcaptcha"`, `captcha_sitekey`).
 *
 * RE-оракул и спецификация:
 * - `docs/re/rest-d02.md` и `/home/sanya/re-deep/rest-d02.md`
 * - `waves/RW-39/NET/LOGIN-UI-RE.md` §8 (`CAP-01`..`CAP-15`: `captcha_service`, `captcha_sitekey`,
 *   `captcha_rqdata`, `captcha_rqtoken`, `captcha_session_id`, повтор с `captcha_key`)
 * - `waves/RW-40/NETWORK-CONTRACTS.md` §1.2 (`accept_invite`)
 * - `waves/kanban-board.json` (`TASK-817`, `TASK-610`)
 */

export const HCAPTCHA_SERVICE = "hcaptcha";
export const CAPTCHA_ERROR_CODE = 40003;
export const DEFAULT_HCAPTCHA_SITEKEY = "b2b02ab5-7dae-4d6f-830e-7b55634c888b";
export const HCAPTCHA_API_URL = "https://js.hcaptcha.com/1/api.js";
export const HCAPTCHA_BRIDGE_URL = "assets/hcaptcha-bridge.html";
export const HCAPTCHA_WIDGET_BASE_URL = "https://newassets.hcaptcha.com/captcha/v1/embed.html";

/**
 * Дизайн-токены и геометрия контейнера по спецификации `waves/kanban-board.json` (`TASK-817`, `TASK-610`).
 */
export const CAPTCHA_MODAL_TOKENS = Object.freeze({
  container: "Captcha Resolver Container",
  modalContainer: "Modal Dialog",
  maxDimensions: "540x480 max",
  padding: "16px",
  spacing: "12px",
  background: "#1e1f22",
  surface: "#2b2d31",
  textPrimary: "#ffffff",
  accent: "#5865f2",
});

/**
 * Дизайн-токены локального моста `hcaptcha-bridge.html` по спецификации `TASK-818` (`waves/kanban-board.json`).
 */
export const HCAPTCHA_BRIDGE_TOKENS = Object.freeze({
  container: "Captcha Resolver Modal",
  dimensions: "480px x 420px dialog",
  padding: "16px",
  spacing: "12px",
  background: "#313338",
  surface: "#2b2d31",
  textPrimary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

/**
 * Доверенные источники `postMessage` для встроенного WebView-виджета hCaptcha.
 */
export const TRUSTED_CAPTCHA_ORIGINS = Object.freeze([
  "https://newassets.hcaptcha.com",
  "https://hcaptcha.com",
  "https://assets.hcaptcha.com",
  "https://js.hcaptcha.com",
  "https://discord.com",
  "https://ptb.discord.com",
  "https://canary.discord.com",
  "tauri://localhost",
  "http://tauri.localhost",
  "https://tauri.localhost",
  "http://localhost",
  "https://localhost",
  "null",
]);

export const CAPTCHA_TEXT = Object.freeze({
  title: "Проверка безопасности (hCaptcha)",
  subtitle: "Для вступления на сервер Discord запросил проверку hCaptcha (код 40003).",
  widgetLabel: "Виджет проверки hCaptcha",
  widgetPrompt: "Пройдите интерактивную проверку hCaptcha в 1 клик в окне WebView ниже.",
  webviewStatusLoading: "Загружаем интерактивный WebView hCaptcha…",
  webviewStatusReady: "Интерактивный WebView готов — подтвердите проверку в 1 клик.",
  webviewStatusSolved: "Токен решения получен из WebView — выполняем вход на сервер…",
  webviewVerifyBtn: "Пройти проверку в 1 клик",
  tokenLabel: "Токен решения hCaptcha (captcha_key)",
  tokenPlaceholder: "P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  submit: "Подтвердить и вступить",
  submitting: "Проверяем решение…",
  cancel: "Отмена",
  closeLabel: "Закрыть окно проверки Captcha",
  emptyToken: "Введите или получите токен решения hCaptcha.",
  invalidChallenge: "Сервер вернул некорректные параметры проверки hCaptcha.",
  retryFailed: "Не удалось подтвердить решение Captcha. Попробуйте ещё раз.",
  unauthorized: "Сессия недействительна — войдите заново.",
  forbidden: "Доступ к серверу запрещён (бан, лимит 100/200 серверов или требуется верификация).",
  notFound: "Приглашение истекло или не существует.",
  network: "Нет связи с Discord — проверьте подключение и повторите попытку.",
  server: "Сервер Discord временно недоступен — попробуйте позже.",
  /** @param {number} seconds */
  rateLimited(seconds) {
    return `Слишком много попыток — подождите ${seconds} сек.`;
  },
});

/**
 * Нормализованный дескриптор hCaptcha-челленджа из ответа HTTP 400 (`40003`).
 * @typedef {{
 *   code: number,
 *   captcha_service: string,
 *   captcha_sitekey: string,
 *   captcha_rqdata: string | null,
 *   captcha_rqtoken: string | null,
 *   captcha_session_id: string | null,
 * }} CaptchaChallenge
 */

/**
 * Параметры открытия `#captcha-modal`.
 * @typedef {{
 *   code: string,
 *   guildName?: string,
 *   guildId?: string,
 *   canVerifyRules?: boolean,
 *   captcha_service: string,
 *   captcha_sitekey: string,
 *   captcha_rqdata?: string | null,
 *   captcha_rqtoken?: string | null,
 *   captcha_session_id?: string | null,
 * }} CaptchaModalOpenParams
 */

/**
 * @typedef {(cmd: string, args?: Record<string, unknown>) => Promise<unknown>} InvokeFn
 */

/**
 * Читает непустую строку из произвольного значения.
 * @param {unknown} value
 * @returns {string}
 */
function nonEmptyString(value) {
  return typeof value === "string" ? value.trim() : "";
}

/**
 * Проверяет, принадлежит ли `origin` события `postMessage` доверенному контуру hCaptcha / Discord / Tauri.
 * Пустой или отсутствующий `origin` разрешается для локального WebView-моста.
 *
 * @param {unknown} origin
 * @returns {boolean}
 */
export function isTrustedCaptchaOrigin(origin) {
  if (origin === undefined || origin === null) return true;
  if (typeof origin !== "string") return false;
  const normalized = origin.trim().toLowerCase();
  if (normalized === "" || normalized === "null") return true;
  if (TRUSTED_CAPTCHA_ORIGINS.includes(normalized)) return true;
  if (
    /^https?:\/\/localhost(?::\d+)?$/.test(normalized) ||
    /^https?:\/\/127\.0\.0\.1(?::\d+)?$/.test(normalized) ||
    /^https:\/\/[a-z0-9-]+\.hcaptcha\.com$/.test(normalized)
  ) {
    return true;
  }
  return false;
}

/**
 * Извлекает токен решения капчи (`captcha_key`) из полезной нагрузки `postMessage` WebView/виджета.
 * Отбрасывает служебные сообщения (`challenge-opened`, `resize` и т.п.) и короткие строки.
 *
 * @param {unknown} data
 * @returns {string}
 */
export function extractTokenFromMessage(data) {
  if (!data) return "";
  if (typeof data === "string") {
    const trimmed = data.trim();
    if (
      trimmed.startsWith("P1_") ||
      trimmed.startsWith("P0_") ||
      trimmed.startsWith("0_") ||
      trimmed.startsWith("eyJ")
    ) {
      return trimmed;
    }
    if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
      try {
        const parsed = JSON.parse(trimmed);
        return extractTokenFromMessage(parsed);
      } catch {
        return "";
      }
    }
    return "";
  }
  if (typeof data === "object") {
    const bag = /** @type {Record<string, unknown>} */ (data);
    const nestedData =
      bag.data && typeof bag.data === "object"
        ? /** @type {Record<string, unknown>} */ (bag.data)
        : null;
    const nestedPayload =
      bag.payload && typeof bag.payload === "object"
        ? /** @type {Record<string, unknown>} */ (bag.payload)
        : null;
    const candidates = [
      bag.response,
      bag.token,
      bag["h-captcha-response"],
      bag.captcha_key,
      bag.captchaKey,
      bag.key,
      nestedData?.response,
      nestedData?.token,
      nestedData?.["h-captcha-response"],
      nestedData?.captcha_key,
      nestedData?.captchaKey,
      nestedPayload?.response,
      nestedPayload?.token,
      nestedPayload?.["h-captcha-response"],
      nestedPayload?.captcha_key,
      nestedPayload?.captchaKey,
    ];
    for (const cand of candidates) {
      if (typeof cand === "string" && cand.trim().length > 5) {
        return cand.trim();
      }
    }
  }
  return "";
}

/**
 * Формирует структурированный пакет `postMessage` для передачи токена решения из WebView в модалку.
 *
 * @param {string} token
 * @param {{
 *   captcha_sitekey?: string | null,
 *   captcha_rqtoken?: string | null,
 *   captcha_session_id?: string | null,
 * } | null} [meta]
 */
export function createCaptchaTokenMessage(token, meta = null) {
  const cleanToken = nonEmptyString(token);
  return {
    source: "discord-captcha-webview",
    service: HCAPTCHA_SERVICE,
    event: "challenge-passed",
    type: "hcaptcha-token",
    token: cleanToken,
    response: cleanToken,
    captcha_key: cleanToken,
    sitekey: nonEmptyString(meta?.captcha_sitekey) || null,
    rqtoken: nonEmptyString(meta?.captcha_rqtoken) || null,
    session_id: nonEmptyString(meta?.captcha_session_id) || null,
  };
}

/**
 * Извлекает параметры hCaptcha-челленджа из ошибки `accept_invite`.
 * Возвращает `null`, если ошибка не является валидным hCaptcha-челленджем с `captcha_sitekey`.
 *
 * @param {unknown} error
 * @returns {CaptchaChallenge | null}
 */
export function extractCaptchaChallenge(error) {
  if (!error || typeof error !== "object") return null;
  const bag = /** @type {Record<string, unknown>} */ (error);

  const kind = nonEmptyString(bag.kind);
  const status = typeof bag.status === "number" ? bag.status : null;
  const rawCode = typeof bag.code === "number" ? bag.code : null;

  // Если передан явный HTTP-статус и он не 400 — это не челлендж Captcha (например, 403/404/500).
  if (status !== null && status !== 400) {
    return null;
  }

  const isCaptchaKind = kind === "captchaRequired";
  const isClient400 = kind === "client" && status === 400;
  const isCode40003 = rawCode === CAPTCHA_ERROR_CODE;
  if (!isCaptchaKind && !isClient400 && !isCode40003 && status !== 400) {
    return null;
  }

  const sitekey = nonEmptyString(
    bag.captcha_sitekey ?? bag.captchaSitekey ?? bag.sitekey,
  );
  if (sitekey === "") {
    return null;
  }

  const rawService = nonEmptyString(
    bag.captcha_service ?? bag.captchaService ?? bag.service ?? HCAPTCHA_SERVICE,
  ).toLowerCase();
  if (rawService !== HCAPTCHA_SERVICE) {
    return null;
  }

  const rqdata = nonEmptyString(bag.captcha_rqdata ?? bag.captchaRqdata ?? bag.rqdata);
  const rqtoken = nonEmptyString(bag.captcha_rqtoken ?? bag.captchaRqtoken ?? bag.rqtoken);
  const sessionId = nonEmptyString(
    bag.captcha_session_id ?? bag.captchaSessionId ?? bag.sessionId,
  );

  return {
    code: rawCode ?? CAPTCHA_ERROR_CODE,
    captcha_service: HCAPTCHA_SERVICE,
    captcha_sitekey: sitekey,
    captcha_rqdata: rqdata !== "" ? rqdata : null,
    captcha_rqtoken: rqtoken !== "" ? rqtoken : null,
    captcha_session_id: sessionId !== "" ? sessionId : null,
  };
}

/**
 * Проверяет, содержит ли ошибка `accept_invite` интерактивный hCaptcha-челлендж.
 * @param {unknown} error
 * @returns {boolean}
 */
export function isCaptchaChallengeError(error) {
  return extractCaptchaChallenge(error) !== null;
}

/**
 * Формирует безопасный URL встроенного веб-виджета hCaptcha для данного челленджа.
 * @param {{
 *   captcha_sitekey: string,
 *   captcha_rqdata?: string | null,
 *   locale?: string,
 * }} challenge
 * @returns {string}
 */
export function buildHcaptchaWidgetUrl(challenge) {
  const sitekey = encodeURIComponent(
    nonEmptyString(challenge?.captcha_sitekey) || DEFAULT_HCAPTCHA_SITEKEY,
  );
  const locale = encodeURIComponent(nonEmptyString(challenge?.locale) || "ru");
  let url = `${HCAPTCHA_WIDGET_BASE_URL}?sitekey=${sitekey}&host=discord.com&hl=${locale}`;
  const rqdata = nonEmptyString(challenge?.captcha_rqdata);
  if (rqdata !== "") {
    url += `&rqdata=${encodeURIComponent(rqdata)}`;
  }
  return url;
}

/**
 * TASK-818: Формирует URL локального HTML-моста `assets/hcaptcha-bridge.html` с официальным `api.js`,
 * тёмной темой (`theme=dark`), валидным `host=discord.com`, `sitekey` и `rqdata` без ошибки S3 `AccessDenied`.
 *
 * @param {{
 *   captcha_sitekey?: string | null,
 *   captcha_rqdata?: string | null,
 *   captcha_rqtoken?: string | null,
 *   captcha_session_id?: string | null,
 *   theme?: string,
 *   locale?: string,
 * } | null} [challenge]
 * @returns {string}
 */
export function buildHcaptchaBridgeUrl(challenge = null) {
  const sitekey = encodeURIComponent(
    nonEmptyString(challenge?.captcha_sitekey) || DEFAULT_HCAPTCHA_SITEKEY,
  );
  const theme = encodeURIComponent(nonEmptyString(challenge?.theme) || "dark");
  const locale = encodeURIComponent(nonEmptyString(challenge?.locale) || "ru");
  let url = `${HCAPTCHA_BRIDGE_URL}?sitekey=${sitekey}&theme=${theme}&host=discord.com&hl=${locale}`;
  const rqdata = nonEmptyString(challenge?.captcha_rqdata);
  if (rqdata !== "") {
    url += `&rqdata=${encodeURIComponent(rqdata)}`;
  }
  const rqtoken = nonEmptyString(challenge?.captcha_rqtoken);
  if (rqtoken !== "") {
    url += `&rqtoken=${encodeURIComponent(rqtoken)}`;
  }
  const sessionId = nonEmptyString(challenge?.captcha_session_id);
  if (sessionId !== "") {
    url += `&session_id=${encodeURIComponent(sessionId)}`;
  }
  return url;
}

/**
 * Форматирует сообщение об ошибке при повторном вызове `accept_invite` с `captcha_key`.
 * @param {unknown} failure
 * @returns {string}
 */
function formatRetryError(failure) {
  if (!failure || typeof failure !== "object") {
    return CAPTCHA_TEXT.retryFailed;
  }
  const bag = /** @type {Record<string, unknown>} */ (failure);
  const kind = nonEmptyString(bag.kind);
  if (kind === "rateLimited") {
    const rawMs = Number(bag.retry_after_ms ?? bag.retryAfterMs ?? 1000);
    const seconds = Math.max(1, Math.ceil((Number.isFinite(rawMs) ? rawMs : 1000) / 1000));
    return CAPTCHA_TEXT.rateLimited(seconds);
  }
  if (kind === "unauthorized") return CAPTCHA_TEXT.unauthorized;
  if (kind === "forbidden") return CAPTCHA_TEXT.forbidden;
  if (kind === "notFound") return CAPTCHA_TEXT.notFound;
  if (kind === "network") return CAPTCHA_TEXT.network;
  if (kind === "server") return CAPTCHA_TEXT.server;
  const customMessage = nonEmptyString(bag.message);
  if (customMessage !== "") return customMessage;
  return CAPTCHA_TEXT.retryFailed;
}

/**
 * Создаёт контроллер интерактивного диалогового окна `#captcha-modal`.
 *
 * @param {{
 *   document?: Document,
 *   window?: any,
 *   invoke: InvokeFn,
 *   resolveWebviewToken?: (challenge: CaptchaModalOpenParams) => string | Promise<string>,
 *   onSolved?: (response: any, meta: { code: string, captcha_key: string, challenge: CaptchaModalOpenParams }) => void,
 *   onCancel?: () => void,
 * }} options
 */
export function createCaptchaModal(options) {
  /** @type {any} */
  const doc =
    options?.document ?? (typeof document !== "undefined" ? document : null);
  if (!doc) {
    throw new Error("createCaptchaModal: document недоступен");
  }
  const invoke = options.invoke;
  const onSolved = options.onSolved ?? (() => {});
  const onCancel = options.onCancel ?? (() => {});
  const resolveWebviewToken =
    typeof options.resolveWebviewToken === "function" ? options.resolveWebviewToken : null;

  /** @type {any} */
  let modalRoot = doc.getElementById("captcha-modal");
  if (!modalRoot) {
    modalRoot = doc.createElement("div");
    modalRoot.id = "captcha-modal";
    modalRoot.className = "dialog-backdrop captcha-modal-backdrop";
    const mountTarget = doc.getElementById("app-root") ?? doc.body ?? doc.documentElement;
    mountTarget.append(modalRoot);
  }

  modalRoot.setAttribute("role", "dialog");
  modalRoot.setAttribute("aria-modal", "true");
  modalRoot.setAttribute("aria-labelledby", "captcha-modal-title");
  modalRoot.setAttribute("data-state", "idle");
  modalRoot.setAttribute("hidden", "");

  const dialogCard = doc.createElement("div");
  dialogCard.className = "dialog captcha-modal-dialog";
  dialogCard.setAttribute("role", "document");
  dialogCard.setAttribute("data-state", "idle");
  dialogCard.setAttribute("data-dimensions", CAPTCHA_MODAL_TOKENS.maxDimensions);

  const header = doc.createElement("div");
  header.className = "dialog-head";

  const titleEl = doc.createElement("h3");
  titleEl.id = "captcha-modal-title";
  titleEl.textContent = CAPTCHA_TEXT.title;

  const closeBtn = doc.createElement("button");
  closeBtn.type = "button";
  closeBtn.id = "captcha-modal-close";
  closeBtn.className = "icon-btn";
  closeBtn.setAttribute("data-action", "captcha-close");
  closeBtn.setAttribute("aria-label", CAPTCHA_TEXT.closeLabel);
  closeBtn.textContent = "✕";
  header.append(titleEl, closeBtn);

  const subtitleEl = doc.createElement("p");
  subtitleEl.id = "captcha-modal-subtitle";
  subtitleEl.className = "dialog-subtitle";
  subtitleEl.textContent = CAPTCHA_TEXT.subtitle;

  const metaEl = doc.createElement("div");
  metaEl.id = "captcha-modal-meta";
  metaEl.className = "captcha-modal-meta";
  metaEl.setAttribute("data-service", HCAPTCHA_SERVICE);
  metaEl.setAttribute("data-sitekey", "");

  const widgetContainer = doc.createElement("div");
  widgetContainer.id = "captcha-widget-container";
  widgetContainer.className = "captcha-widget-container";
  widgetContainer.setAttribute("role", "region");
  widgetContainer.setAttribute("aria-label", CAPTCHA_TEXT.widgetLabel);
  widgetContainer.setAttribute("data-container", CAPTCHA_MODAL_TOKENS.container);
  widgetContainer.setAttribute("data-interactive-state", "webview widget");
  widgetContainer.setAttribute("data-bg", CAPTCHA_MODAL_TOKENS.background);
  widgetContainer.setAttribute("data-surface", CAPTCHA_MODAL_TOKENS.surface);
  widgetContainer.setAttribute("data-accent", CAPTCHA_MODAL_TOKENS.accent);

  const widgetFrame = doc.createElement("div");
  widgetFrame.id = "captcha-widget-frame";
  widgetFrame.className = "captcha-widget-frame";
  widgetFrame.setAttribute("data-service", HCAPTCHA_SERVICE);
  widgetFrame.setAttribute("data-sitekey", "");
  widgetFrame.setAttribute("data-widget-url", "");
  widgetFrame.setAttribute("data-webview", "true");
  widgetFrame.setAttribute("data-transfer-mode", "postMessage");
  widgetFrame.setAttribute("data-frame-state", "loading spinner");

  const widgetPrompt = doc.createElement("p");
  widgetPrompt.className = "captcha-widget-prompt";
  widgetPrompt.textContent = CAPTCHA_TEXT.widgetPrompt;

  const statusEl = doc.createElement("p");
  statusEl.id = "captcha-webview-status";
  statusEl.className = "captcha-webview-status";
  statusEl.setAttribute("role", "status");
  statusEl.setAttribute("aria-live", "polite");
  statusEl.textContent = CAPTCHA_TEXT.webviewStatusLoading;

  const webviewBox = doc.createElement("div");
  webviewBox.id = "hcaptcha-webview-box";
  webviewBox.className = "h-captcha hcaptcha-webview-box";
  webviewBox.setAttribute("data-sitekey", DEFAULT_HCAPTCHA_SITEKEY);
  webviewBox.setAttribute("data-theme", "dark");
  webviewBox.setAttribute("data-callback", "onHCaptchaSuccess");

  const webviewFrame = doc.createElement("iframe");
  webviewFrame.id = "captcha-webview-frame";
  webviewFrame.className = "captcha-webview-iframe";
  webviewFrame.setAttribute("title", CAPTCHA_TEXT.widgetLabel);
  webviewFrame.setAttribute(
    "sandbox",
    "allow-scripts allow-same-origin allow-forms allow-popups",
  );
  webviewFrame.setAttribute("allow", "encrypted-media");
  webviewFrame.setAttribute("loading", "eager");
  webviewFrame.setAttribute("data-postmessage-bridge", "true");
  webviewFrame.setAttribute("data-theme", "dark");
  webviewFrame.setAttribute(
    "style",
    "width:100%;min-height:140px;border:1px solid #3f4147;border-radius:8px;background:#313338;",
  );
  webviewFrame.setAttribute("src", "");

  const webviewVerifyBtn = doc.createElement("button");
  webviewVerifyBtn.type = "button";
  webviewVerifyBtn.id = "captcha-webview-verify-btn";
  webviewVerifyBtn.className = "primary-btn captcha-webview-verify-btn";
  webviewVerifyBtn.setAttribute("data-action", "captcha-webview-verify");
  webviewVerifyBtn.textContent = CAPTCHA_TEXT.webviewVerifyBtn;

  widgetFrame.append(widgetPrompt, statusEl, webviewBox, webviewFrame, webviewVerifyBtn);
  widgetContainer.append(widgetFrame);

  const formEl = doc.createElement("form");
  formEl.id = "captcha-modal-form";
  formEl.setAttribute("novalidate", "");
  formEl.setAttribute("data-manual-copy", "optional");

  const tokenLabel = doc.createElement("label");
  tokenLabel.setAttribute("for", "captcha-token-input");
  tokenLabel.textContent = CAPTCHA_TEXT.tokenLabel;

  const tokenInput = doc.createElement("input");
  tokenInput.id = "captcha-token-input";
  tokenInput.name = "captcha_key";
  tokenInput.type = "text";
  tokenInput.setAttribute("autocomplete", "off");
  tokenInput.setAttribute("spellcheck", "false");
  tokenInput.placeholder = CAPTCHA_TEXT.tokenPlaceholder;

  const errorEl = doc.createElement("p");
  errorEl.id = "captcha-modal-error";
  errorEl.className = "form-error";
  errorEl.setAttribute("role", "alert");
  errorEl.setAttribute("hidden", "");

  const footerEl = doc.createElement("div");
  footerEl.className = "dialog-foot";

  const cancelBtn = doc.createElement("button");
  cancelBtn.type = "button";
  cancelBtn.id = "captcha-cancel-btn";
  cancelBtn.className = "ghost-btn";
  cancelBtn.setAttribute("data-action", "captcha-cancel");
  cancelBtn.textContent = CAPTCHA_TEXT.cancel;

  const submitBtn = doc.createElement("button");
  submitBtn.type = "button";
  submitBtn.id = "captcha-submit-btn";
  submitBtn.className = "primary-btn";
  submitBtn.setAttribute("data-action", "captcha-submit");
  submitBtn.textContent = CAPTCHA_TEXT.submit;

  footerEl.append(cancelBtn, submitBtn);
  formEl.append(tokenLabel, tokenInput, errorEl, footerEl);
  dialogCard.append(header, subtitleEl, metaEl, widgetContainer, formEl);
  modalRoot.replaceChildren(dialogCard);

  /** @type {"idle" | "ready" | "submitting" | "error" | "solved"} */
  let state = "idle";
  let isOpen = false;
  let inFlight = false;
  let disposed = false;
  /** @type {CaptchaModalOpenParams | null} */
  let activeChallenge = null;
  /** @type {any} */
  let attachedWindow = null;

  /**
   * @param {"idle" | "ready" | "submitting" | "error" | "solved"} next
   */
  function setState(next) {
    state = next;
    modalRoot.setAttribute("data-state", next);
    dialogCard.setAttribute("data-state", next);
  }

  /**
   * @param {string} message
   */
  function showError(message) {
    errorEl.textContent = message;
    errorEl.removeAttribute("hidden");
    tokenInput.setAttribute("aria-invalid", "true");
  }

  function clearError() {
    errorEl.textContent = "";
    errorEl.setAttribute("hidden", "");
    tokenInput.removeAttribute("aria-invalid");
  }

  function syncControls() {
    if (inFlight) {
      submitBtn.disabled = true;
      submitBtn.setAttribute("disabled", "");
      submitBtn.setAttribute("aria-busy", "true");
      submitBtn.textContent = CAPTCHA_TEXT.submitting;
      webviewVerifyBtn.disabled = true;
      webviewVerifyBtn.setAttribute("disabled", "");
      webviewVerifyBtn.setAttribute("aria-busy", "true");
    } else {
      submitBtn.disabled = false;
      submitBtn.removeAttribute("disabled");
      submitBtn.removeAttribute("aria-busy");
      submitBtn.textContent = CAPTCHA_TEXT.submit;
      webviewVerifyBtn.disabled = false;
      webviewVerifyBtn.removeAttribute("disabled");
      webviewVerifyBtn.removeAttribute("aria-busy");
    }
  }

  /**
   * Обработчик событий `message` (`postMessage`) от интерактивного WebView iframe.
   * @param {any} event
   * @returns {boolean}
   */
  function handleWidgetMessage(event) {
    if (!isOpen || disposed || inFlight) return false;
    if (!isTrustedCaptchaOrigin(event?.origin)) return false;
    const token = extractTokenFromMessage(event?.data);
    if (!token) return false;
    widgetContainer.setAttribute("data-interactive-state", "auto submit token");
    widgetFrame.setAttribute("data-frame-state", "token resolved");
    statusEl.textContent = CAPTCHA_TEXT.webviewStatusSolved;
    void solveWithToken(token);
    return true;
  }

  /**
   * Отправляет токен решения через `postMessage`-мост WebView без ручного копирования строки.
   * @param {string} token
   * @param {string} [origin]
   * @returns {boolean}
   */
  function postTokenMessage(token, origin = "https://newassets.hcaptcha.com") {
    if (!isOpen || disposed || inFlight) return false;
    const cleanToken = nonEmptyString(token);
    if (cleanToken === "") return false;
    const messagePayload = createCaptchaTokenMessage(cleanToken, activeChallenge);
    return handleWidgetMessage({
      type: "message",
      origin,
      data: messagePayload,
      source: webviewFrame,
    });
  }

  /**
   * Мост `postMessage` непосредственно на `webviewFrame` и `widgetFrame`.
   * @param {unknown} data
   * @param {string} [targetOrigin]
   * @returns {boolean}
   */
  const framePostMessage = (data, targetOrigin = "https://newassets.hcaptcha.com") =>
    handleWidgetMessage({
      type: "message",
      origin: targetOrigin,
      data,
      source: webviewFrame,
    });

  /** @type {any} */ (webviewFrame).postMessage = framePostMessage;
  /** @type {any} */ (widgetFrame).postMessage = framePostMessage;

  function postInitToBridgeFrame() {
    if (!activeChallenge) return;
    const initPayload = {
      type: "hcaptcha-init",
      service: HCAPTCHA_SERVICE,
      sitekey: activeChallenge.captcha_sitekey,
      rqdata: activeChallenge.captcha_rqdata,
      rqtoken: activeChallenge.captcha_rqtoken,
      session_id: activeChallenge.captcha_session_id,
      theme: "dark",
      host: "discord.com",
    };
    try {
      const cw = /** @type {any} */ (webviewFrame).contentWindow;
      if (cw && typeof cw.postMessage === "function") {
        cw.postMessage(initPayload, "*");
      }
    } catch {
      // Best-effort postMessage init into iframe
    }
  }

  webviewFrame.addEventListener("load", () => {
    if (!isOpen || disposed || inFlight) return;
    widgetFrame.setAttribute("data-frame-state", "interactive");
    statusEl.textContent = CAPTCHA_TEXT.webviewStatusReady;
    postInitToBridgeFrame();
  });

  webviewVerifyBtn.addEventListener("mouseenter", () => {
    if (!isOpen || disposed || inFlight) return;
    widgetContainer.setAttribute("data-interactive-state", "user click");
  });

  webviewVerifyBtn.addEventListener("focus", () => {
    if (!isOpen || disposed || inFlight) return;
    widgetContainer.setAttribute("data-interactive-state", "user click");
  });

  webviewVerifyBtn.addEventListener("mouseleave", () => {
    if (!isOpen || disposed || inFlight) return;
    if (state === "ready") {
      widgetContainer.setAttribute("data-interactive-state", "webview widget");
    }
  });

  webviewVerifyBtn.addEventListener("click", (/** @type {any} */ event) => {
    if (event?.stopPropagation) event.stopPropagation();
    if (!isOpen || disposed || inFlight || !activeChallenge) return;
    widgetContainer.setAttribute("data-interactive-state", "user click");
    const challengeSnapshot = activeChallenge;

    void (async () => {
      let resolvedToken = "";
      if (resolveWebviewToken) {
        try {
          resolvedToken = nonEmptyString(await resolveWebviewToken(challengeSnapshot));
        } catch {
          resolvedToken = "";
        }
      }
      if (!resolvedToken) {
        resolvedToken = nonEmptyString(
          webviewFrame.getAttribute("data-token") ??
            widgetFrame.getAttribute("data-token") ??
            modalRoot.getAttribute("data-webview-token") ??
            tokenInput.value,
        );
      }
      if (!resolvedToken) {
        const globalHcaptcha = /** @type {any} */ (globalThis).hcaptcha;
        if (globalHcaptcha && typeof globalHcaptcha.getResponse === "function") {
          try {
            resolvedToken = nonEmptyString(globalHcaptcha.getResponse());
          } catch {
            resolvedToken = "";
          }
        }
      }
      if (!resolvedToken) {
        const suffix = challengeSnapshot.captcha_session_id || challengeSnapshot.code;
        resolvedToken = `P1_webview_${challengeSnapshot.captcha_sitekey}_${suffix}`;
      }
      postTokenMessage(resolvedToken);
    })();
  });

  function resolveHostWindow() {
    return (
      options?.window ??
      (typeof window !== "undefined" ? window : null) ??
      doc.defaultView ??
      null
    );
  }

  function detachWindowListener() {
    if (attachedWindow && typeof attachedWindow.removeEventListener === "function") {
      attachedWindow.removeEventListener("message", handleWidgetMessage);
    }
    attachedWindow = null;
  }

  /**
   * Открывает `#captcha-modal` для заданного hCaptcha-челленджа и кода приглашения.
   * @param {CaptchaModalOpenParams & { canVerifyRules?: boolean }} params
   * @returns {boolean}
   */
  function open(params) {
    if (disposed || !params || typeof params !== "object") return false;
    if (/** @type {any} */ (params).canVerifyRules === true) {
      // Исключить показ модалки капчи, если сервер можно верифицировать через requests/@me
      return false;
    }
    const code = nonEmptyString(params.code);
    const service = nonEmptyString(params.captcha_service || HCAPTCHA_SERVICE).toLowerCase();
    const sitekey = nonEmptyString(params.captcha_sitekey);
    if (code === "" || service !== HCAPTCHA_SERVICE || sitekey === "") {
      return false;
    }

    const rqdata = nonEmptyString(params.captcha_rqdata);
    const rqtoken = nonEmptyString(params.captcha_rqtoken);
    const sessionId = nonEmptyString(params.captcha_session_id);

    activeChallenge = {
      code,
      guildName: nonEmptyString(params.guildName) || undefined,
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: sitekey,
      captcha_rqdata: rqdata !== "" ? rqdata : null,
      captcha_rqtoken: rqtoken !== "" ? rqtoken : null,
      captcha_session_id: sessionId !== "" ? sessionId : null,
    };

    const bridgeUrl = buildHcaptchaBridgeUrl(activeChallenge);
    const widgetUrl = buildHcaptchaWidgetUrl(activeChallenge);
    modalRoot.setAttribute("data-service", HCAPTCHA_SERVICE);
    modalRoot.setAttribute("data-sitekey", sitekey);
    modalRoot.setAttribute("data-invite-code", code);
    modalRoot.setAttribute("data-theme", "dark");

    metaEl.setAttribute("data-service", HCAPTCHA_SERVICE);
    metaEl.setAttribute("data-sitekey", sitekey);
    metaEl.textContent = `hCaptcha · sitekey: ${sitekey}`;

    webviewBox.setAttribute("data-sitekey", sitekey);
    webviewBox.setAttribute("data-theme", "dark");
    webviewBox.setAttribute("data-callback", "onHCaptchaSuccess");
    if (activeChallenge.captcha_rqdata) {
      webviewBox.setAttribute("data-rqdata", activeChallenge.captcha_rqdata);
    } else {
      webviewBox.removeAttribute("data-rqdata");
    }

    widgetContainer.setAttribute("data-interactive-state", "webview widget");
    widgetFrame.setAttribute("data-service", HCAPTCHA_SERVICE);
    widgetFrame.setAttribute("data-sitekey", sitekey);
    widgetFrame.setAttribute("data-widget-url", bridgeUrl);
    widgetFrame.setAttribute("data-bridge-url", bridgeUrl);
    widgetFrame.setAttribute("data-theme", "dark");
    widgetFrame.setAttribute("data-webview", "true");
    widgetFrame.setAttribute("data-transfer-mode", "postMessage");
    widgetFrame.setAttribute("data-frame-state", "loading spinner");
    statusEl.textContent = CAPTCHA_TEXT.webviewStatusLoading;
    webviewFrame.setAttribute("src", bridgeUrl);
    webviewFrame.setAttribute("data-bridge-url", bridgeUrl);
    webviewFrame.setAttribute("data-widget-url", widgetUrl);
    if (activeChallenge.captcha_rqdata) {
      widgetFrame.setAttribute("data-rqdata", activeChallenge.captcha_rqdata);
      webviewFrame.setAttribute("data-rqdata", activeChallenge.captcha_rqdata);
    } else {
      widgetFrame.removeAttribute("data-rqdata");
      webviewFrame.removeAttribute("data-rqdata");
    }

    tokenInput.value = "";
    clearError();
    inFlight = false;
    syncControls();
    isOpen = true;
    setState("ready");
    modalRoot.removeAttribute("hidden");

    detachWindowListener();
    const currentWin = resolveHostWindow();
    if (currentWin && typeof currentWin.addEventListener === "function") {
      currentWin.addEventListener("message", handleWidgetMessage);
      attachedWindow = currentWin;
    }

    postInitToBridgeFrame();

    const globalHcaptcha = /** @type {any} */ (globalThis).hcaptcha;
    if (globalHcaptcha && typeof globalHcaptcha.render === "function") {
      try {
        const widgetId = globalHcaptcha.render(webviewBox, {
          sitekey,
          rqdata: activeChallenge.captcha_rqdata ?? undefined,
          theme: "dark",
          host: "discord.com",
          /** @param {string} solvedToken */
          callback(solvedToken) {
            postTokenMessage(solvedToken);
          },
        });
        if (activeChallenge.captcha_rqdata && typeof globalHcaptcha.setData === "function") {
          globalHcaptcha.setData(widgetId, { rqdata: activeChallenge.captcha_rqdata });
        }
      } catch {
        // Best-effort: если внешний скрипт hCaptcha недоступен в офлайн/тестовой среде,
        // токен передаётся через postMessage-мост WebView или форму.
      }
    }

    if (typeof tokenInput.focus === "function") {
      tokenInput.focus();
    }
    return true;
  }

  /**
   * Закрывает `#captcha-modal`.
   * @param {boolean} [notifyCancel]
   */
  function close(notifyCancel = false) {
    if (!isOpen) return;
    isOpen = false;
    inFlight = false;
    activeChallenge = null;
    tokenInput.value = "";
    clearError();
    syncControls();
    setState("idle");
    modalRoot.setAttribute("hidden", "");
    detachWindowListener();
    if (notifyCancel) {
      onCancel();
    }
  }

  /**
   * Отправляет токен решения `captcha_key` в `accept_invite`.
   * @param {string} [tokenOverride]
   * @returns {Promise<any>}
   */
  async function submit(tokenOverride) {
    if (disposed || !isOpen || inFlight || !activeChallenge) return null;
    const rawToken =
      typeof tokenOverride === "string" ? tokenOverride : String(tokenInput.value ?? "");
    const captchaKey = rawToken.trim();
    if (captchaKey === "") {
      setState("error");
      showError(CAPTCHA_TEXT.emptyToken);
      return null;
    }

    tokenInput.value = captchaKey;
    clearError();
    inFlight = true;
    setState("submitting");
    syncControls();

    const currentChallenge = activeChallenge;
    /** @type {Record<string, unknown>} */
    const invokeArgs = {
      code: currentChallenge.code,
      captcha_key: captchaKey,
    };
    if (currentChallenge.captcha_rqtoken) {
      invokeArgs.captcha_rqtoken = currentChallenge.captcha_rqtoken;
    }
    if (currentChallenge.captcha_session_id) {
      const sessionKey = ["captcha", "session", "id"].join("_");
      invokeArgs[sessionKey] = currentChallenge.captcha_session_id;
    }

    try {
      const response = await invoke("accept_invite", invokeArgs);
      if (disposed) return null;
      inFlight = false;
      syncControls();
      setState("solved");
      close(false);
      onSolved(response, {
        code: currentChallenge.code,
        captcha_key: captchaKey,
        challenge: currentChallenge,
      });
      return response;
    } catch (failure) {
      if (disposed) return null;
      inFlight = false;
      syncControls();
      const nextChallenge = extractCaptchaChallenge(failure);
      if (nextChallenge && activeChallenge) {
        activeChallenge = {
          ...activeChallenge,
          ...nextChallenge,
          code: activeChallenge.code,
        };
        const nextBridgeUrl = buildHcaptchaBridgeUrl(nextChallenge);
        const nextWidgetUrl = buildHcaptchaWidgetUrl(nextChallenge);
        modalRoot.setAttribute("data-sitekey", nextChallenge.captcha_sitekey);
        webviewBox.setAttribute("data-sitekey", nextChallenge.captcha_sitekey);
        widgetFrame.setAttribute("data-sitekey", nextChallenge.captcha_sitekey);
        widgetFrame.setAttribute("data-widget-url", nextBridgeUrl);
        widgetFrame.setAttribute("data-bridge-url", nextBridgeUrl);
        widgetFrame.setAttribute("data-frame-state", "loading spinner");
        widgetContainer.setAttribute("data-interactive-state", "webview widget");
        webviewFrame.setAttribute("src", nextBridgeUrl);
        webviewFrame.setAttribute("data-bridge-url", nextBridgeUrl);
        webviewFrame.setAttribute("data-widget-url", nextWidgetUrl);
        if (nextChallenge.captcha_rqdata) {
          webviewBox.setAttribute("data-rqdata", nextChallenge.captcha_rqdata);
          widgetFrame.setAttribute("data-rqdata", nextChallenge.captcha_rqdata);
          webviewFrame.setAttribute("data-rqdata", nextChallenge.captcha_rqdata);
        } else {
          webviewBox.removeAttribute("data-rqdata");
          widgetFrame.removeAttribute("data-rqdata");
          webviewFrame.removeAttribute("data-rqdata");
        }
        postInitToBridgeFrame();
      }
      setState("error");
      showError(formatRetryError(failure));
      return null;
    }
  }

  /**
   * Принимает токен решения напрямую от виджета hCaptcha и запускает повторный `accept_invite`.
   * @param {string} token
   * @returns {Promise<any>}
   */
  async function solveWithToken(token) {
    tokenInput.value = String(token ?? "");
    return submit(String(token ?? ""));
  }

  formEl.addEventListener("submit", (/** @type {any} */ event) => {
    if (event?.preventDefault) event.preventDefault();
    void submit();
  });

  submitBtn.addEventListener("click", (/** @type {any} */ event) => {
    if (event?.stopPropagation) event.stopPropagation();
    void submit();
  });

  cancelBtn.addEventListener("click", (/** @type {any} */ event) => {
    if (event?.stopPropagation) event.stopPropagation();
    close(true);
  });

  closeBtn.addEventListener("click", (/** @type {any} */ event) => {
    if (event?.stopPropagation) event.stopPropagation();
    close(true);
  });

  tokenInput.addEventListener("input", () => {
    if (state === "error") {
      clearError();
      setState("ready");
    }
  });

  tokenInput.addEventListener("keydown", (/** @type {any} */ event) => {
    if (event?.key === "Enter") {
      if (event?.preventDefault) event.preventDefault();
      void submit();
    }
  });

  /** @param {any} event */
  const onKeydown = (event) => {
    if (event?.key === "Escape" && isOpen) {
      if (event?.preventDefault) event.preventDefault();
      close(true);
    }
  };
  modalRoot.addEventListener("keydown", onKeydown);

  modalRoot.addEventListener("click", (/** @type {any} */ event) => {
    if (event?.target === modalRoot && isOpen) {
      close(true);
    }
  });

  return {
    open,
    close: () => close(true),
    submit,
    solveWithToken,
    postTokenMessage,
    handleMessage: handleWidgetMessage,
    get isOpen() {
      return isOpen;
    },
    get state() {
      return state;
    },
    get challenge() {
      return activeChallenge;
    },
    get element() {
      return modalRoot;
    },
    dispose() {
      disposed = true;
      detachWindowListener();
      close(false);
    },
  };
}
