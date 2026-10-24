// @ts-check
import { make as makeNode, setAriaHidden } from "./dom.js";

/**
 * Локальный адаптер: возвращает DOM-узел без узкой типизации тега, чтобы
 * `type`/`value` можно было ставить у button/input без кастов в каждой строке.
 * @param {string} tag
 * @param {string} [className]
 * @param {string} [value]
 * @returns {any}
 */
const make = (tag, className, value) => makeNode(tag, className, value);

export const AUTH_IDS = Object.freeze({ gate: "auth-gate" });
export const AUTH_COMMANDS = Object.freeze({ status: "auth_status", login: "auth_login_token", restore: "auth_restore", logout: "auth_logout", password: "auth_login_password", mfa: "auth_mfa", sms: "auth_sms" });
export const AUTH_MESSAGES = Object.freeze({ empty: "Введите токен", invalid: "Токен недействителен — проверьте вставку", rateLimited: "Слишком часто: подождите и повторите", offline: "Нет связи с сервером — проверьте сеть", ipc: "Нет связи с ядром приложения", keyring: "Хранилище токена недоступно — проверьте системный keyring", parse: "Сервер ответил неожиданно — повторите вход", internal: "Внутренняя ошибка входа — повторите позже" });
const byId = (/** @type {string} */ id) => document.getElementById(id);
const errText = (/** @type {unknown} */ err) => {
  if (typeof err === "string" || err instanceof Error) return AUTH_MESSAGES.offline;
  const e = err && typeof err === "object" ? /** @type {any} */ (err) : {};
  const code = e.code ?? e.error?.code;
  const kind = e.kind ?? e.error?.kind;
  if (kind === "InvalidCredentials" || code === 50018 || code === 50022) return "Неверный логин или пароль.";
  if (kind === "RateLimited" || code === 429) return `Слишком много попыток. Повторите через ${Math.max(1, Math.ceil(Number(e.retry_after_ms ?? e.retryAfterMs ?? 1000) / 1000))} с.`;
  if (kind === "Network") return AUTH_MESSAGES.offline;
  if (kind === "Keyring") return AUTH_MESSAGES.keyring;
  if (kind === "InvalidToken") return AUTH_MESSAGES.invalid;
  if (code === 70007 || code === 40002) return "Требуется подтверждение в официальном клиенте.";
  if (code === 60006 || code === 60009) return "Сессия входа истекла. Войдите снова.";
  if (code === 60008 || kind === "MfaRejected") return "Неверный код.";
  return "Вход не выполнен. Проверьте данные и повторите.";
};

/** Build auth UI and bind the backend contract. */
/** @param {{invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<any>, onReady?: () => void}} [options] */
export function createAuthGate(options = {}) {
  const gate = byId("auth-gate") ?? document.createElement("div"); gate.id = "auth-gate"; if (!gate.parentNode) document.body.append(gate);
  gate.className = "auth-gate"; gate.setAttribute("role", "dialog"); gate.setAttribute("aria-modal", "true");
  gate.replaceChildren();
  const card = make("section", "auth-card"); card.id = "auth-card";
  const body = make("div", "auth-body");
  const col = make("div", "auth-form-col"); col.id = "auth-form-col";
  const title = make("h2", "auth-title", "Вход в Discord");
  const form = /** @type {HTMLFormElement} */ (make("form", "auth-form")); form.id = "auth-form";
  const loginLabel = make("label", "auth-label", "Email или телефон"); loginLabel.setAttribute("for", "auth-login");
  const login = /** @type {HTMLInputElement} */ (make("input", "auth-input")); login.id = "auth-login"; login.setAttribute("type", "text"); login.setAttribute("autocomplete", "username"); login.setAttribute("spellcheck", "false"); login.setAttribute("aria-describedby", "auth-login-error");
  const loginError = make("p", "auth-field-error"); loginError.id = "auth-login-error";
  const passLabel = make("label", "auth-label", "Пароль"); passLabel.setAttribute("for", "auth-password");
  const password = /** @type {HTMLInputElement} */ (make("input", "auth-input")); password.id = "auth-password"; password.setAttribute("type", "password"); password.setAttribute("autocomplete", "current-password");
  const passReveal = /** @type {HTMLButtonElement} */ (make("button", "auth-reveal", "Показать")); passReveal.id = "auth-password-reveal"; passReveal.type = "button"; passReveal.setAttribute("aria-label", "Показать пароль"); passReveal.setAttribute("aria-pressed", "false"); passReveal.setAttribute("aria-controls", password.id);
  const passError = make("p", "auth-field-error"); passError.id = "auth-password-error";
  const submit = /** @type {HTMLButtonElement} */ (make("button", "auth-submit", "Войти")); submit.id = "auth-submit"; submit.type = "submit";
  const forgot = make("button", "auth-link", "Забыли пароль?"); forgot.id = "auth-forgot"; forgot.type = "button";
  const register = make("button", "auth-link", "Зарегистрироваться"); register.id = "auth-register"; register.type = "button";
  const error = make("p", "auth-error"); error.id = "auth-error"; error.setAttribute("role", "alert"); error.setAttribute("aria-live", "assertive");
  const status = make("p", "auth-status"); status.id = "auth-status"; status.setAttribute("aria-live", "polite");
  const retry = make("button", "auth-restore", "Повторить"); retry.id = "auth-retry"; retry.type = "button";
  const restore = make("button", "auth-restore", "Восстановить сессию"); restore.id = "auth-restore"; restore.type = "button";
  const tokenBlock = document.createElement("details"); tokenBlock.id = "auth-token-block"; const summary = make("summary", "auth-link", "Войти по токену");
  const token = /** @type {HTMLInputElement} */ (make("input", "auth-input")); token.id = "auth-token"; token.setAttribute("type", "password"); token.setAttribute("autocomplete", "off");
  const reveal = make("button", "auth-reveal", "Показать токен"); reveal.id = "auth-reveal"; reveal.type = "button"; reveal.setAttribute("aria-pressed", "false");
  const tokenSubmit = make("button", "auth-restore", "Войти по токену"); tokenSubmit.id = "auth-token-submit"; tokenSubmit.type = "button";
  tokenBlock.append(summary, token, reveal, tokenSubmit);
  form.append(loginLabel, login, loginError, passLabel, password, passReveal, passError, forgot, submit, register, tokenBlock, restore, retry, error, status);
  col.append(title, form);
  const qr = make("aside", "auth-qr"); qr.id = "auth-qr"; qr.append(make("h3", "", "Вход по QR-коду"), make("p", "", "Отсканируйте код в мобильном приложении, чтобы войти."));
  const qrCode = make("div", "auth-qr-code"); qrCode.id = "auth-qr-code"; qrCode.setAttribute("role", "img"); qrCode.setAttribute("aria-label", "QR-код для входа");
  const svg = document.createElement("svg"); svg.setAttribute("width", "160"); svg.setAttribute("height", "160"); svg.setAttribute("viewBox", "0 0 160 160"); svg.setAttribute("aria-hidden", "true");
  const qrRects = document.createElement("path"); qrRects.setAttribute("d", "M8 8h48v48H8zm8 8v32h32V16zm88-8h48v48h-48zm8 8v32h32V16zM8 104h48v48H8zm8 8v32h32v-32zm56-56h16v16H72zm24 24h16v16H96zm-24 24h16v16H72zm48-8h16v16h-16zm-24 24h16v16H96zm32 24h16v16h-16z"); qrRects.setAttribute("fill", "currentColor"); svg.append(qrRects); qrCode.append(svg); qr.append(qrCode);
  for (const [id, label, tip] of [["auth-qr-disabled", "QR-вход (Phase 3)", "QR-вход пока недоступен"], ["auth-passkey-disabled", "Или войдите с ключом доступа", "Ключ доступа пока недоступен"]]) { const b = make("button", "auth-link", label); b.id = id; b.type = "button"; b.setAttribute("aria-disabled", "true"); b.title = tip; qr.append(b); }
  body.append(col, qr); card.append(body); gate.append(card);
  const captcha = make("section", "auth-captcha"); captcha.id = "auth-captcha"; captcha.setAttribute("role", "group"); captcha.setAttribute("aria-labelledby", "auth-captcha-title"); captcha.setAttribute("hidden", "");
  const captchaTitle = make("h3", "", "Проверка безопасности"); captchaTitle.id = "auth-captcha-title";
  const captchaWidget = make("div", ""); captchaWidget.id = "auth-captcha-widget";
  const captchaContinue = make("button", "auth-restore", "Продолжить"); captchaContinue.id = "auth-captcha-continue"; captchaContinue.type = "button"; captchaContinue.setAttribute("aria-disabled", "true"); captchaContinue.title = "Сначала пройдите проверку"; captcha.append(captchaTitle, captchaWidget, captchaContinue); gate.append(captcha);
  const mfa = make("section", "auth-mfa"); mfa.id = "auth-mfa"; mfa.setAttribute("hidden", "");
  const mfaClose = make("button", "auth-link", "Отмена"); mfaClose.id = "auth-mfa-close"; mfaClose.type = "button";
  const methods = make("div", "auth-mfa-methods"); methods.id = "auth-mfa-methods";
  const mfaForm = /** @type {HTMLFormElement} */ (make("form", "auth-mfa-form")); mfaForm.id = "auth-mfa-form"; mfaForm.setAttribute("hidden", "");
  const subtitle = make("p", "auth-status"); subtitle.id = "auth-mfa-subtitle";
  const code = /** @type {HTMLInputElement} */ (make("input", "auth-input")); code.id = "auth-mfa-code"; code.setAttribute("inputmode", "numeric"); code.setAttribute("autocomplete", "one-time-code"); code.setAttribute("maxlength", "6"); code.setAttribute("type", "text"); code.setAttribute("hidden", "");
  const mfaSubmit = make("button", "auth-submit", "Подтвердить"); mfaSubmit.id = "auth-mfa-submit"; mfaSubmit.type = "submit"; mfaSubmit.setAttribute("aria-disabled", "true"); mfaSubmit.title = "Введите код";
  const other = make("button", "auth-link", "Выбрать другой способ"); other.id = "auth-mfa-other"; other.type = "button"; other.setAttribute("hidden", "");
  const ack = make("button", "auth-restore", "Понятно"); ack.id = "auth-mfa-backup-ack"; ack.type = "button"; ack.setAttribute("hidden", ""); ack.setAttribute("aria-disabled", "true"); ack.title = "Подождите 10 с";
  const resend = make("button", "auth-restore", "Отправить ещё раз"); resend.id = "auth-mfa-sms-resend"; resend.type = "button"; resend.setAttribute("hidden", "");
  mfaForm.append(subtitle, code, ack, resend, mfaSubmit, other, error); mfa.append(mfaClose, methods, mfaForm); gate.append(mfa);
  const logout = byId("auth-logout") ?? make("button", "icon-btn auth-logout", "Выйти"); logout.id = "auth-logout"; logout.type = "button"; logout.setAttribute("data-action", "auth-logout"); logout.setAttribute("aria-label", "Выйти из аккаунта"); logout.title = "Выйти из аккаунта"; logout.setAttribute("hidden", ""); (document.querySelector(".user-actions") ?? document.querySelector("#user-bar") ?? document.body).append(logout);
  const shell = document.querySelector(".shell"); const nameNode = document.querySelector(".user-name"); const initialName = nameNode?.textContent ?? "";
  /** @type {any} */ let user = null;
  /** @type {string | null} */ let instance = null;
  /** @type {ReturnType<typeof setInterval> | undefined} */ let timer;
  let phase = "checking", stored = false, ticket = "", method = "totp", captchaKey = "", captchaRqtoken = "", captchaSessionId = "", captchaRqdata = "", shouldServeInvisible, userFlow = "", loginValue = "", passValue = "";
  /** P1: повторный клик по токену/restore/MFA до ответа IPC не шлёт вторую команду. */
  let tokenBusy = false, restoreBusy = false, mfaBusy = false;
  /** @type {(cmd: string, args?: any) => Promise<any>} */
  const invoke = options.invoke ?? ((cmd, args) => /** @type {any} */ (window).__TAURI__.core.invoke(cmd, args));
  /** @param {Element} node @param {boolean} yes */
  const hide = (node, yes) => { if (yes) node.setAttribute("hidden", ""); else node.removeAttribute("hidden"); };
  /** @param {string} text */
  const setError = (text) => { error.textContent = text; };
  /**
   * Блокировка кнопки на время IPC: disabled + aria-disabled + подсказка,
   * чтобы повторный клик не порождал вторую auth-команду.
   * @param {HTMLElement} node
   * @param {boolean} busy
   * @param {string} [label]
   */
  function setBusy(node, busy, label = "") {
    if (busy) {
      node.setAttribute("disabled", "");
      node.setAttribute("aria-disabled", "true");
      node.title = label;
      return;
    }
    node.removeAttribute("disabled");
    node.removeAttribute("aria-disabled");
    node.removeAttribute("title");
  }
  /** @param {any} s */
  function setState(s) { phase = s; gate.setAttribute("data-state", s); gate.setAttribute("aria-busy", s === "checking" || s === "validating" ? "true" : "false"); hide(gate, s === "ready"); gate.style.display = s === "ready" ? "none" : ""; setAriaHidden(gate, s === "ready"); if (shell) setAriaHidden(/** @type {any} */ (shell), s !== "ready"); hide(form, !["no-session", "validating", "error"].includes(s)); hide(qr, s !== "no-session"); card.setAttribute("data-qr", s === "no-session" ? "true" : "false"); hide(captcha, s !== "captcha"); const mfaVisible = s === "mfa" || (s === "validating" && mfaForm.getAttribute("hidden") === null); hide(mfa, !mfaVisible); status.textContent = s === "checking" ? "Проверяем сохранённую сессию…" : s === "validating" ? "Проверяем…" : s === "ready" ? "Вход выполнен" : ""; if (s === "validating") { submit.setAttribute("disabled", ""); submit.setAttribute("aria-disabled", "true"); submit.title = "Проверяем данные…"; login.setAttribute("disabled", ""); password.setAttribute("disabled", ""); } else { submit.removeAttribute("disabled"); if (submit.getAttribute("aria-disabled") === "true") submit.removeAttribute("aria-disabled"); if (s !== "error") submit.removeAttribute("title"); const captchaState = s === "captcha"; if (captchaState) { login.setAttribute("disabled", ""); password.setAttribute("disabled", ""); } else { login.removeAttribute("disabled"); password.removeAttribute("disabled"); } } hide(restore, !stored || s === "ready"); hide(logout, s !== "ready"); if (nameNode) nameNode.textContent = s === "ready" && user ? user.global_name || user.username : initialName; }
  /** @param {any} u */
  function ready(u) { user = u ?? null; setError(""); setState("ready"); options.onReady?.(); }
  function fail(/** @type {unknown} */ e, /** @type {boolean} */ keepMfa = false) { const m = errText(e); setError(m); setState(keepMfa ? "mfa" : "error"); if (m.includes("Неверный логин")) password.setAttribute("aria-invalid", "true"); const details = e && typeof e === "object" ? /** @type {any} */ (e) : {}; if (details.code === 70007) hide(retry, false); if (details.kind === "RateLimited" || details.code === 429) { let left = Math.max(1, Math.ceil(Number(details.retry_after_ms ?? details.retryAfterMs ?? 1000) / 1000)); submit.setAttribute("disabled", ""); submit.setAttribute("aria-disabled", "true"); submit.title = `Повторите через ${left} с`; timer = setInterval(() => { left--; if (left <= 0) { clearInterval(timer); submit.removeAttribute("disabled"); submit.removeAttribute("aria-disabled"); submit.removeAttribute("title"); } else submit.title = `Повторите через ${left} с`; }, 1000); } }
  /** @param {any} r */
  function result(r) { if (!r || typeof r !== "object") { fail({}); return; } if (r.status === "error") { fail(r.error); return; } const k = r.kind ?? (r.status === "ready" ? "token" : ""); if (k === "ready" || k === "token" || r.status === "ready") { ready(r.user); return; } if (k === "captcha") { captchaRqtoken = r.rqtoken ?? ""; captchaSessionId = r.captcha_session_id ?? ""; captchaRqdata = r.captcha_rqdata ?? ""; shouldServeInvisible = r.should_serve_invisible; userFlow = r.user_flow ?? ""; setError("Нужна проверка безопасности."); captchaWidget.replaceChildren(); captchaKey = ""; setState("captcha"); const hostWindow = /** @type {any} */ (globalThis).window;
        const adapter = hostWindow?.__AUTH_CAPTCHA__ ?? /** @type {any} */ (globalThis).__AUTH_CAPTCHA__;
        if (adapter?.mount) {
          try {
            const ctx = /** @type {Record<string, unknown>} */ ({
              sitekey: r.sitekey ?? "",
              service: r.service ?? "",
              rqtoken: captchaRqtoken,
              onToken: (/** @type {any} */ v) => {
                captchaKey = String(v);
                captchaContinue.removeAttribute("aria-disabled");
                captchaContinue.removeAttribute("title");
              },
            });
            if (captchaSessionId) ctx.captcha_session_id = captchaSessionId;
            if (captchaRqdata) ctx.captcha_rqdata = captchaRqdata;
            if (shouldServeInvisible !== undefined) ctx.should_serve_invisible = shouldServeInvisible;
            if (userFlow) ctx.user_flow = userFlow;
            adapter.mount(captchaWidget, ctx);
          } catch (mountError) {
            captchaWidget.textContent = mountError instanceof Error && mountError.message
              ? `Проверка безопасности недоступна: ${mountError.message}`
              : "Проверка безопасности недоступна. Интерактивный виджет не загружен.";
          }
        } else {
          captchaWidget.textContent = "Проверка безопасности недоступна. Интерактивный виджет не загружен.";
        }
        return;
      }
      if (k === "mfa") {
        ticket = r.ticket ?? "";
        instance = r.login_instance_id ?? null;
        methods.replaceChildren();
        const list = Array.isArray(r.methods)
          ? r.methods.filter((/** @type {any} */ m) => typeof m === "string" && m.length > 0)
          : [];
        for (const m of list) {
          const b = make("button", "auth-mfa-method", m);
          b.id = `auth-mfa-method-${m}`;
          b.type = "button";
          b.setAttribute("data-method", m);
          b.addEventListener("click", () => selectMethod(m));
          methods.append(b);
        }
        const wa = make("button", "auth-mfa-method", "Ключ доступа");
        wa.id = "auth-mfa-method-webauthn";
        wa.type = "button";
        wa.setAttribute("aria-disabled", "true");
        wa.title = "Нативная подпись ключом доступа недоступна";
        mfa.append(wa);
        setState("mfa");
        if (list.length === 1) selectMethod(list[0] ?? "");
        return;
      }
      fail(r);
  }
  /** @param {any} m */
  function selectMethod(m) { method = m; if (m === "webauthn") return; hide(methods, true); hide(mfaForm, false); hide(other, false); hide(code, false); code.value = ""; code.type = m === "password" ? "password" : "text"; code.setAttribute("autocomplete", m === "password" ? "current-password" : "one-time-code"); code.setAttribute("maxlength", m === "backup" ? "11" : m === "sms" ? "10" : "6"); updateMfa(); if (m === "backup") { code.setAttribute("minlength", "8"); let left = 10; subtitle.textContent = `Сохраните резервные коды. ${left} с`; timer = setInterval(() => { left--; subtitle.textContent = `Сохраните резервные коды. ${Math.max(0,left)} с`; if (!left) { clearInterval(timer); ack.removeAttribute("aria-disabled"); ack.removeAttribute("title"); } }, 1000); hide(ack, false); } if (m === "sms") { hide(resend, false); void invoke(AUTH_COMMANDS.sms, { ticket }).then((/** @type {any} */ r) => { subtitle.textContent = r.phone ?? r.phoneMasked ?? "Код отправлен"; }).catch(fail); } }
  function passwordSubmit() { if (phase === "validating") return; const l = login.value.trim(), p = password.value; if (!l) { loginError.textContent = "Введите email или телефон."; login.setAttribute("aria-invalid", "true"); login.focus(); return; } if (!p) { passError.textContent = "Введите пароль."; password.setAttribute("aria-invalid", "true"); password.focus(); return; } loginValue = l; passValue = p; password.value = ""; setError(""); setState("validating"); const args = /** @type {Record<string, unknown>} */ ({ login: loginValue, password: passValue }); if (captchaKey) { args.captcha_key = captchaKey; if (captchaRqtoken) args.captcha_rqtoken = captchaRqtoken; } void invoke(AUTH_COMMANDS.password, args).then((r) => { if (!r || r.kind !== "captcha") passValue = ""; result(r); }).catch((e) => { passValue = ""; fail(e); }); }
  form.addEventListener("submit", (e) => { e.preventDefault(); passwordSubmit(); }); submit.addEventListener("click", (e) => { e.preventDefault(); passwordSubmit(); });
  passReveal.addEventListener("click", () => { const show = password.type === "password"; password.type = show ? "text" : "password"; passReveal.setAttribute("aria-pressed", String(show)); passReveal.setAttribute("aria-label", show ? "Скрыть пароль" : "Показать пароль"); });
  reveal.addEventListener("click", () => { token.type = token.type === "password" ? "text" : "password"; reveal.setAttribute("aria-pressed", String(token.type === "text")); reveal.setAttribute("aria-label", token.type === "text" ? "Скрыть токен" : "Показать токен"); });
  tokenSubmit.addEventListener("click", () => {
    if (tokenBusy) return;
    const v = token.value.trim();
    if (!v) { setError(AUTH_MESSAGES.empty); token.focus(); return; }
    tokenBusy = true;
    setBusy(/** @type {HTMLElement} */ (tokenSubmit), true, "Проверяем токен…");
    token.value = "";
    setState("validating");
    void invoke(AUTH_COMMANDS.login, { token: v }).then(result).catch(fail).finally(() => {
      tokenBusy = false;
      setBusy(/** @type {HTMLElement} */ (tokenSubmit), false);
    });
  });
  restore.addEventListener("click", () => {
    if (restoreBusy) return;
    restoreBusy = true;
    setBusy(restore, true, "Проверяем сессию…");
    setState("validating");
    void invoke(AUTH_COMMANDS.restore).then(result).catch(fail).finally(() => {
      restoreBusy = false;
      setBusy(restore, false);
    });
  });
  function clearSensitive() { login.value = ""; password.value = ""; token.value = ""; code.value = ""; ticket = ""; instance = null; captchaKey = ""; captchaRqtoken = ""; captchaSessionId = ""; captchaRqdata = ""; shouldServeInvisible = undefined; userFlow = ""; loginValue = ""; passValue = ""; }
  logout.addEventListener("click", () => { clearSensitive(); setState("validating"); void invoke(AUTH_COMMANDS.logout).then(() => { user = null; stored = false; setError(""); setState("no-session"); login.focus(); }).catch(fail); });
  retry.addEventListener("click", () => { setError(""); setState("no-session"); login.focus(); });
  mfaClose.addEventListener("click", () => { setError(""); hide(mfa, true); setState("no-session"); login.focus(); }); other.addEventListener("click", () => { hide(mfaForm, true); hide(methods, false); hide(other, true); });
  ack.addEventListener("click", () => { hide(ack, true); hide(code, false); code.setAttribute("maxlength", "11"); code.setAttribute("minlength", "8"); });
  const updateMfa = () => { const v = code.value; const ok = method === "backup" ? v.replaceAll("-", "").length >= 8 : method === "password" ? v.length > 0 : method === "sms" ? v.length >= 4 : /^\d{6}$/.test(v); if (ok) { mfaSubmit.removeAttribute("aria-disabled"); mfaSubmit.removeAttribute("title"); } else { mfaSubmit.setAttribute("aria-disabled", "true"); mfaSubmit.title = "Введите код"; } };
  code.addEventListener("input", updateMfa);
  const submitMfa = (/** @type {any} */ e) => {
    e.preventDefault();
    // P1: пока ответ в полёте — ни Enter, ни повторный клик не шлют пустой код.
    if (mfaBusy) return;
    const v = code.value;
    if (mfaSubmit.getAttribute("aria-disabled") === "true") return;
    mfaBusy = true;
    code.value = "";
    // После очистки поле пустое: updateMfa обязан вернуть кнопку в disabled,
    // иначе быстрый второй клик отправит auth_mfa с code="".
    updateMfa();
    setState("validating");
    const args = { method, code: method === "backup" ? v.replaceAll("-", "") : v, ticket, login_instance_id: instance };
    void invoke(AUTH_COMMANDS.mfa, args)
      .then(result)
      .catch((err) => { if ((/** @type {any} */ (err))?.code === 60006 || (/** @type {any} */ (err))?.code === 60009) { setError(errText(err)); setState("no-session"); } else fail(err, true); })
      .finally(() => { mfaBusy = false; });
  };
  mfaForm.addEventListener("submit", submitMfa); mfaSubmit.addEventListener("click", submitMfa); captchaContinue.addEventListener("click", () => { if (!captchaKey) return; const pwd = passValue; passValue = ""; setState("validating"); const args = /** @type {Record<string, unknown>} */ ({ login: loginValue, password: pwd, captcha_key: captchaKey, captcha_rqtoken: captchaRqtoken }); if (captchaSessionId) args.captcha_session_id = captchaSessionId; if (captchaRqdata) args.captcha_rqdata = captchaRqdata; void invoke(AUTH_COMMANDS.password, args).then(result).catch(fail); }); resend.addEventListener("click", () => { void invoke(AUTH_COMMANDS.sms, { ticket }).then((r) => { subtitle.textContent = r.phone ?? r.phoneMasked ?? "Код отправлен"; }).catch(fail); });
  hide(form, true); hide(qr, true); hide(captcha, true); hide(mfa, true); setState("checking");
  if (/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) {
    setState("ready");
  } else void invoke(AUTH_COMMANDS.status).then((s) => { stored = Boolean(s?.stored); if (s?.status === "ready" && s.user) ready(s.user); else if (s?.status === "error") fail(s.error); else { setError(""); setState("no-session"); login.focus(); } }).catch(fail);
  return { get state() { return phase; }, get user() { return user; }, submit: passwordSubmit, restore: () => restore.click(), logout: () => logout.click() };
}
