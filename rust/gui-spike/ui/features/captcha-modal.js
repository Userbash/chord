// @ts-check
/**
 * TASK-610: интерактивное модальное окно решения hCaptcha (`#captcha-modal`)
 * при вступлении на сервер (`POST /invites/{code}` → HTTP 400, код `40003`,
 * `captcha_service: "hcaptcha"`, `captcha_sitekey`).
 *
 * RE-оракул:
 * - `docs/re/rest-d02.md` и `/home/sanya/re-deep/rest-d02.md`
 * - `waves/RW-39/NET/LOGIN-UI-RE.md` §8 (`CAP-01`..`CAP-15`: `captcha_service`, `captcha_sitekey`,
 *   `captcha_rqdata`, `captcha_rqtoken`, `captcha_session_id`, повтор с `captcha_key`)
 * - `waves/RW-40/NETWORK-CONTRACTS.md` §1.2 (`accept_invite`)
 */

export const HCAPTCHA_SERVICE = "hcaptcha";
export const CAPTCHA_ERROR_CODE = 40003;
export const HCAPTCHA_WIDGET_BASE_URL = "https://newassets.hcaptcha.com/captcha/v1/embed.html";

export const CAPTCHA_TEXT = Object.freeze({
  title: "Проверка безопасности (hCaptcha)",
  subtitle: "Для вступления на сервер Discord запросил проверку hCaptcha (код 40003).",
  widgetLabel: "Виджет проверки hCaptcha",
  widgetPrompt: "Пройдите интерактивную проверку hCaptcha или вставьте полученный токен решения ниже.",
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
  const sitekey = encodeURIComponent(nonEmptyString(challenge?.captcha_sitekey));
  const locale = encodeURIComponent(nonEmptyString(challenge?.locale) || "ru");
  let url = `${HCAPTCHA_WIDGET_BASE_URL}?sitekey=${sitekey}&host=discord.com&hl=${locale}`;
  const rqdata = nonEmptyString(challenge?.captcha_rqdata);
  if (rqdata !== "") {
    url += `&rqdata=${encodeURIComponent(rqdata)}`;
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
 *   invoke: InvokeFn,
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

  const widgetFrame = doc.createElement("div");
  widgetFrame.id = "captcha-widget-frame";
  widgetFrame.className = "captcha-widget-frame";
  widgetFrame.setAttribute("data-service", HCAPTCHA_SERVICE);
  widgetFrame.setAttribute("data-sitekey", "");
  widgetFrame.setAttribute("data-widget-url", "");

  const widgetPrompt = doc.createElement("p");
  widgetPrompt.className = "captcha-widget-prompt";
  widgetPrompt.textContent = CAPTCHA_TEXT.widgetPrompt;
  widgetFrame.append(widgetPrompt);
  widgetContainer.append(widgetFrame);

  const formEl = doc.createElement("form");
  formEl.id = "captcha-modal-form";
  formEl.setAttribute("novalidate", "");

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
    } else {
      submitBtn.disabled = false;
      submitBtn.removeAttribute("disabled");
      submitBtn.removeAttribute("aria-busy");
      submitBtn.textContent = CAPTCHA_TEXT.submit;
    }
  }

  /**
   * Открывает `#captcha-modal` для заданного hCaptcha-челленджа и кода приглашения.
   * @param {CaptchaModalOpenParams} params
   * @returns {boolean}
   */
  function open(params) {
    if (disposed || !params || typeof params !== "object") return false;
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

    const widgetUrl = buildHcaptchaWidgetUrl(activeChallenge);
    modalRoot.setAttribute("data-service", HCAPTCHA_SERVICE);
    modalRoot.setAttribute("data-sitekey", sitekey);
    modalRoot.setAttribute("data-invite-code", code);

    metaEl.setAttribute("data-service", HCAPTCHA_SERVICE);
    metaEl.setAttribute("data-sitekey", sitekey);
    metaEl.textContent = `hCaptcha · sitekey: ${sitekey}`;

    widgetFrame.setAttribute("data-service", HCAPTCHA_SERVICE);
    widgetFrame.setAttribute("data-sitekey", sitekey);
    widgetFrame.setAttribute("data-widget-url", widgetUrl);
    if (activeChallenge.captcha_rqdata) {
      widgetFrame.setAttribute("data-rqdata", activeChallenge.captcha_rqdata);
    } else {
      widgetFrame.removeAttribute("data-rqdata");
    }

    tokenInput.value = "";
    clearError();
    inFlight = false;
    syncControls();
    isOpen = true;
    setState("ready");
    modalRoot.removeAttribute("hidden");

    const globalHcaptcha = /** @type {any} */ (globalThis).hcaptcha;
    if (globalHcaptcha && typeof globalHcaptcha.render === "function") {
      try {
        globalHcaptcha.render(widgetFrame, {
          sitekey,
          rqdata: activeChallenge.captcha_rqdata ?? undefined,
          /** @param {string} solvedToken */
          callback(solvedToken) {
            void solveWithToken(solvedToken);
          },
        });
      } catch {
        // Best-effort: если внешний скрипт hCaptcha недоступен в офлайн/тестовой среде,
        // пользователь вводит/подтверждает токен через форму #captcha-token-input.
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
        modalRoot.setAttribute("data-sitekey", nextChallenge.captcha_sitekey);
        widgetFrame.setAttribute("data-sitekey", nextChallenge.captcha_sitekey);
        widgetFrame.setAttribute("data-widget-url", buildHcaptchaWidgetUrl(nextChallenge));
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

  submitBtn.addEventListener("click", () => {
    void submit();
  });

  cancelBtn.addEventListener("click", () => {
    close(true);
  });

  closeBtn.addEventListener("click", () => {
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
      close(false);
    },
  };
}
