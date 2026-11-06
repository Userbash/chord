// @ts-check
/**
 * G-03-UI: модалка «Присоединиться к серверу» на backend `resolve_invite` /
 * `accept_invite` (D-116) и «Открыть сервер» на `join_guild` (G-03a, D-76).
 *
 * Контракт (`waves/RW-40/DESIGN/SERVER-MENU.md` §2.3/§3.1/§6, `G6-REPORT` §3.2/§3.3):
 * - три разных шага: `resolve_invite(code)` (preview), `accept_invite(code)`
 *   (членство) и `join_guild({guildId})` (lurker-просмотр, 204 без тела);
 *   UI рисует только то, что вернул backend, и не смешивает потоки;
 * - `join_guild` не добавляет плитку и не означает членство: успех — только 204,
 *   статус «Сервер открыт для просмотра» `[A]`, accept-поток остаётся доступен;
 * - accept получает **только** `code`: три поля тела (`session_id`,
 *   `invite_instance_id`, `received_installation_id`) nullable, и подставлять их
 *   на клиенте нечем — backend сам отправит `null`;
 * - bearer живёт только в backend (`Arc<AuthBackend>`), UI его не читает и не
 *   передаёт; в аргументах IPC нет токена;
 * - успех accept — только 2xx с непустым `guildId`; иначе типизированная ошибка,
 *   никакого «оптимистичного» успеха и фальшивых плиток.
 *
 * DOM собирается здесь одним код-путём (как `#auth-gate` в `features/auth.js`):
 * в `index.html` только host, поэтому DOM-шим тестов и браузер видят одно дерево.
 * Тексты — из §5.1 (`[D]`), коды ошибок — из §3.1 (`[A]`, live не наблюдался).
 */
import { byId, formatNumber, make } from "./dom.js";

/** Команды, принятые backend (`rust/gui-spike/src/main.rs`, D-116/D-76). */
export const JOIN_COMMANDS = Object.freeze({
  resolve: "resolve_invite",
  accept: "accept_invite",
  enter: "join_guild",
});

/** Тексты §5.1 (ключи `[D]`); авторские строки помечены в отчёте. */
export const JOIN_TEXT = Object.freeze({
  title: "Присоединиться к серверу",
  hint: "Введите приглашение, чтобы присоединиться к существующему серверу.",
  placeholder: "Введите ссылку-приглашение или код, чтобы присоединиться к серверу",
  codeHint: "Например: abc123",
  submit: "Присоединиться",
  open: "Открыть сервер",
  cancel: "Отмена",
  pending: "Приглашение обрабатывается",
  alreadyMember: "Вы уже участник",
  opened: "Сервер открыт для просмотра",
  success: (/** @type {string} */ name) => `Вы присоединились к серверу «${name}»!`,
});

/** Допустимый код: `[A-Za-z0-9-]`, 2…32 (`G6 §3.3`, наша валидация `[A]`). */
const CODE_RE = /^[A-Za-z0-9-]{2,32}$/;

/**
 * Нормализация ввода — **зеркало** backend `normalize_code`
 * (`rust/gui-spike/src/invites.rs`, принято D-116): trim → снять `?`/`#` →
 * снять хвостовые `/` → последний сегмент → `[A-Za-z0-9-]`, 2…32.
 *
 * Пустая строка = «нечего отправлять»: `resolve_invite` не вызывается.
 * @param {unknown} input
 * @returns {string}
 */
export function normalizeInvite(input) {
  const raw = typeof input === "string" ? input : "";
  const trimmed = raw.trim();
  const cut = trimmed.search(/[?#]/);
  const withoutQuery = (cut < 0 ? trimmed : trimmed.slice(0, cut)).replace(/\/+$/, "");
  const code = withoutQuery.slice(withoutQuery.lastIndexOf("/") + 1);
  // Ссылка без кода (`…/invite/`) не должна превращаться в код «invite».
  if ((code === "invite" || code === "invites") && trimmed.includes("/")) return "";
  return CODE_RE.test(code) ? code : "";
}

/**
 * @typedef {{
 *   code: string,
 *   guildId: string,
 *   guildName: string,
 *   icon: string,
 *   memberCount: number | null,
 *   presenceCount: number | null,
 *   expiresAt: string,
 *   channelId: string,
 * }} JoinPreview
 * @typedef {{ id: string, name: string }} JoinedGuild
 * @typedef {"resolve" | "accept" | "enter"} JoinStep
 * @typedef {{ text: string, retry: boolean, invalidInput?: boolean, cooldown?: number }} JoinFailure
 */

/** @param {unknown} error @returns {string} */
function errorKind(error) {
  if (typeof error !== "object" || error === null || !("kind" in error)) return "";
  const kind = /** @type {{ kind?: unknown }} */ (error).kind;
  return typeof kind === "string" ? kind : "";
}

/**
 * `retry_after_ms` в целых секундах; 0 — сервер не прислал задержку.
 * @param {unknown} error
 * @returns {number}
 */
function retryAfterSeconds(error) {
  if (typeof error !== "object" || error === null) return 0;
  const bag = /** @type {Record<string, unknown>} */ (error);
  const raw = bag.retry_after_ms ?? bag.retryAfterMs;
  return typeof raw === "number" && Number.isFinite(raw) && raw > 0 ? Math.ceil(raw / 1000) : 0;
}

function isLivePhoneVerificationRequired() {
  if (!Boolean(/** @type {any} */ (globalThis).__TAURI_INTERNALS__)) return false;
  return (
    /** @type {any} */ (globalThis).__DISCORD_ACCOUNT_REQUIRED_ACTION__ ===
      "REQUIRE_VERIFIED_PHONE" ||
    /** @type {any} */ (globalThis).__DISCORD_LAST_API_ERROR_CODE__ === 40002
  );
}

function isLiveCaptchaChallengeRequired() {
  if (!Boolean(/** @type {any} */ (globalThis).__TAURI_INTERNALS__)) return false;
  return /** @type {any} */ (globalThis).__DISCORD_LAST_API_ERROR_CODE__ === 40003;
}

/**
 * Видимая ошибка lurker-шага `join_guild` (G-03a, D-76). Тело ответа не
 * разбирается; 403 — «Нет доступа» (G6 §3.2), 429 уважает `retry_after_ms`.
 * @param {unknown} error
 * @returns {JoinFailure}
 */
function enterFailure(error) {
  switch (errorKind(error)) {
    case "unauthorized":
    case "missingToken":
      return { text: "Сессия истекла. Войдите снова.", retry: true };
    case "invalidArg":
      return { text: "Не удалось открыть сервер: неверный идентификатор.", retry: false };
    case "forbidden":
      return {
        text: isLivePhoneVerificationRequired()
          ? "Нет доступа к серверу. Discord требует подтверждения номера телефона на этом аккаунте (REQUIRE_VERIFIED_PHONE, код 40002)."
          : "Нет доступа к серверу.",
        retry: true,
      };
    case "notFound":
      return { text: "Сервер не найден. Возможно, вы уже вышли.", retry: true };
    case "rateLimited": {
      const seconds = retryAfterSeconds(error);
      return {
        text: seconds > 0
          ? `Слишком много попыток. Повторите через ${seconds} с.`
          : "Слишком много попыток. Повторите позже.",
        retry: true,
        cooldown: seconds,
      };
    }
    case "server":
    case "parse":
      return { text: "Сервер не ответил. Попробуйте ещё раз.", retry: true };
    case "network":
      return { text: "Сетевое подключение ограничено или недоступно.", retry: true };
    case "storage":
      return { text: "Хранилище токена недоступно. Повторите позже.", retry: true };
    default:
      return { text: "Не удалось открыть сервер. Повторите попытку.", retry: true };
  }
}

/**
 * Видимая ошибка шага: текст по машинному `kind`, без разбора тела и токена
 * (§3.1). «Таймаут/неизвестный исход» успехом не считается.
 * @param {unknown} error
 * @param {JoinStep} [step]
 * @returns {JoinFailure}
 */
export function inviteFailure(error, step = "resolve") {
  if (step === "enter") return enterFailure(error);
  switch (errorKind(error)) {
    case "unauthorized":
    case "missingToken":
      return { text: "Сессия истекла. Войдите снова.", retry: true };
    case "invalidArg":
      return { text: "Ссылка-приглашение выглядит неверно", retry: true, invalidInput: true };
    case "forbidden":
      return {
        text: isLivePhoneVerificationRequired()
          ? "Недостаточно прав, чтобы присоединиться к этому серверу. Discord требует подтверждения номера телефона на этом аккаунте (REQUIRE_VERIFIED_PHONE, код 40002)."
          : "Недостаточно прав, чтобы присоединиться к этому серверу.",
        retry: true,
      };
    case "notFound":
      return {
        text: "Приглашение недействительно. Приглашение содержит ошибку, или истёк срок его действия.",
        retry: true,
        invalidInput: true,
      };
    case "rateLimited": {
      const seconds = retryAfterSeconds(error);
      return {
        text: seconds > 0
          ? `Слишком много попыток. Повторите через ${seconds} с.`
          : "Слишком много попыток. Повторите позже.",
        retry: true,
        cooldown: seconds,
      };
    }
    case "server":
      return { text: "Сервер не ответил. Попробуйте ещё раз.", retry: true };
    case "network":
      return { text: "Сетевое подключение ограничено или недоступно.", retry: true };
    case "storage":
      return { text: "Хранилище токена недоступно. Повторите позже.", retry: true };
    case "invalidResponse":
      return {
        text: step === "resolve"
          ? "Не удалось загрузить профиль для этого сервера."
          : isLiveCaptchaChallengeRequired()
            ? "Discord запросил проверку Captcha при вступлении на сервер. Примите приглашение один раз в браузере или телефоне — сервер и все его каналы сразу синхронизируются."
            : "Невозможно присоединиться к серверу. Неизвестная ошибка.",
        retry: true,
      };
    default:
      return {
        text: isLiveCaptchaChallengeRequired()
          ? "Discord запросил проверку Captcha при вступлении на сервер. Примите приглашение один раз в браузере или телефоне — сервер и все его каналы сразу синхронизируются."
          : "Невозможно присоединиться к серверу. Неизвестная ошибка.",
        retry: true,
      };
  }
}

/**
 * Монограмма 2 буквы (как `.guild-item`): `icon` от backend — хеш CDN, а не URL,
 * поэтому адрес иконки не склеиваем (`SERVER-MENU §2.1`).
 * @param {string} name
 * @returns {string}
 */
function monogram(name) {
  const words = String(name)
    .split(/\s+/)
    .map((word) => word.replace(/^[^\p{L}]+/u, ""))
    .filter((word) => /^\p{L}/u.test(word));
  const first = words[0] ?? "";
  const second = words[1] ?? "";
  const value = words.length >= 2
    ? `${[...first][0] ?? ""}${[...second][0] ?? ""}`
    : [...first].slice(0, 2).join("");
  return value.toUpperCase();
}

/** @param {unknown} value @param {string} code @returns {JoinPreview} */
function parsePreview(value, code) {
  const bag = value !== null && typeof value === "object"
    ? /** @type {Record<string, unknown>} */ (value)
    : {};
  const guildId = typeof bag.guildId === "string" ? bag.guildId : "";
  const guildName = typeof bag.guildName === "string" ? bag.guildName : "";
  if (guildId === "" || guildName === "") {
    // 2xx без разбираемого объекта — не успех (`InviteError::InvalidResponse`).
    throw /** @type {any} */ ({ kind: "invalidResponse" });
  }
  /** @param {unknown} raw @returns {number | null} */
  const count = (raw) => (typeof raw === "number" && Number.isInteger(raw) && raw >= 0 ? raw : null);
  return {
    code: typeof bag.code === "string" && bag.code !== "" ? bag.code : code,
    guildId,
    guildName,
    icon: typeof bag.icon === "string" ? bag.icon : "",
    memberCount: count(bag.memberCount),
    presenceCount: count(bag.presenceCount),
    expiresAt: typeof bag.expiresAt === "string" ? bag.expiresAt : "",
    channelId: typeof bag.channelId === "string" ? bag.channelId : "",
  };
}

/** @param {unknown} value @returns {string} `guildId` из `AcceptedInvite` или "". */
function parseAcceptedGuildId(value) {
  const bag = value !== null && typeof value === "object"
    ? /** @type {Record<string, unknown>} */ (value)
    : {};
  return typeof bag.guildId === "string" ? bag.guildId : "";
}

/**
 * Модалка join: состояние `idle|resolving|preview_ready|accepting|accepted|error`.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   onAccepted?: (guild: JoinedGuild) => void,
 * }} options
 */
export function createJoinServer(options) {
  const { invoke } = options;
  const onAccepted = options.onAccepted ?? (() => {});

  /** @param {string} tag @param {string} [className] @param {string} [value] @returns {any} */
  const node = (tag, className, value) => make(tag, className, value);

  // Host: в браузере приходит из `index.html`, в шиме создаётся здесь (как
  // `#auth-gate`), поэтому id/структура одинаковы в обеих средах.
  const host = byId("join-server-dialog") ?? node("div", "dialog-backdrop");
  host.id = "join-server-dialog";
  host.className = "dialog-backdrop";
  host.setAttribute("hidden", "");
  if (!host.parentNode) document.body.append(host);
  host.replaceChildren();

  const card = node("div", "dialog");
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("aria-labelledby", "join-server-title");
  card.setAttribute("aria-describedby", "join-server-hint");
  card.setAttribute("data-state", "idle");

  const title = node("h2", "dialog-title", JOIN_TEXT.title);
  title.id = "join-server-title";
  const hint = node("p", "dialog-text", JOIN_TEXT.hint);
  hint.id = "join-server-hint";

  const form = node("form", "join-server-form");
  form.id = "join-server-form";
  form.setAttribute("novalidate", "");

  const label = node("label", "visually-hidden", "Ссылка-приглашение");
  label.setAttribute("for", "join-invite-input");
  const input = /** @type {HTMLInputElement} */ (node("input", "dialog-input"));
  input.id = "join-invite-input";
  input.setAttribute("type", "text");
  input.setAttribute("autocomplete", "off");
  input.setAttribute("spellcheck", "false");
  input.placeholder = JOIN_TEXT.placeholder;
  input.setAttribute("aria-describedby", "join-invite-hint join-error");
  input.setAttribute("data-autofocus", "true");
  const codeHint = node("p", "dialog-hint", JOIN_TEXT.codeHint);
  codeHint.id = "join-invite-hint";

  const preview = node("div", "join-preview");
  preview.id = "join-preview";
  preview.setAttribute("hidden", "");

  const error = node("p", "dialog-error");
  error.id = "join-error";
  error.setAttribute("role", "alert");
  error.setAttribute("aria-live", "assertive");
  error.setAttribute("hidden", "");

  const status = node("p", "visually-hidden");
  status.id = "join-status";
  status.setAttribute("aria-live", "polite");

  const actions = node("div", "dialog-actions");
  const cancel = node("button", "btn btn-secondary", JOIN_TEXT.cancel);
  cancel.setAttribute("type", "button");
  cancel.setAttribute("data-action", "join-cancel");
  const submit = node("button", "btn btn-primary", JOIN_TEXT.submit);
  submit.id = "join-submit";
  submit.setAttribute("type", "button");
  submit.setAttribute("data-action", "join-submit");
  submit.setAttribute("disabled", "");
  actions.append(cancel, submit);

  form.append(label, input, codeHint, preview, error, status, actions);
  const close = node("button", "icon-btn dialog-close", "×");
  close.setAttribute("type", "button");
  close.setAttribute("data-action", "dialog-close");
  close.setAttribute("aria-label", "Закрыть");
  card.append(title, hint, form, close);
  host.append(card);

  // Backdrop click: клик вне карточки закрывает модал (только если не pending).
  host.addEventListener("click", (/** @type {MouseEvent} */ event) => {
    if (event.target === host) {
      if (!pending && !entering) hide(true);
    }
  });

  // Общий live-регион успеха (§6): `#server-status` уже создаёт actions.js,
  // но модалка может смонтироваться раньше — тогда создаём его здесь.
  const statusRegion = byId("server-status") ?? node("p", "visually-hidden");
  statusRegion.id = "server-status";
  statusRegion.className = "visually-hidden";
  statusRegion.setAttribute("role", "status");
  statusRegion.setAttribute("aria-live", "polite");
  if (!statusRegion.parentNode) document.body.append(statusRegion);

  /** @type {"idle" | "resolving" | "preview_ready" | "accepting" | "accepted" | "entering" | "entered" | "error"} */
  let phase = "idle";
  /** @type {JoinStep} */
  let step = "resolve";
  /** @type {JoinPreview | null} */
  let current = null;
  let pending = false;
  /** G-03a: отдельный lurker-поток; не смешивается с resolve/accept. */
  let entering = false;
  let entered = false;
  let enterCooling = false;
  /** @type {HTMLElement | null} */
  let openButton = null;
  let cooling = false;
  let open = false;
  let disposed = false;
  /** @type {HTMLElement | null} */
  let opener = null;
  /** @type {number | null} */
  /** @type {ReturnType<typeof setTimeout> | null} */
  let cooldownTimer = null;
  /** @type {ReturnType<typeof setTimeout> | null} */
  let enterTimer = null;

  /** @param {"idle" | "resolving" | "preview_ready" | "accepting" | "accepted" | "entering" | "entered" | "error"} next */
  function setPhase(next) {
    phase = next;
    card.setAttribute("data-state", next);
  }

  /** Единая точка блокировки: pending/entering → обе кнопки, 429 → только шаг. */
  function syncSubmit() {
    const empty = String(input.value ?? "").trim() === "" && current === null;
    if (pending || entering || cooling || empty) submit.setAttribute("disabled", "");
    else submit.removeAttribute("disabled");
    // Кнопка «Отмена» всегда активна — пользователь должен иметь выход.
    cancel.removeAttribute("disabled");
    if (pending || entering) form.setAttribute("aria-busy", "true");
    else form.removeAttribute("aria-busy");
  }

  /** G-03a: кнопка «Открыть сервер» — один pending-флаг, повтор игнорируется. */
  function syncEnter() {
    if (!openButton) return;
    if (entering || entered || enterCooling) openButton.setAttribute("disabled", "");
    else openButton.removeAttribute("disabled");
    if (entering) openButton.setAttribute("aria-busy", "true");
    else openButton.removeAttribute("aria-busy");
  }

  /** @param {string} message */
  function showError(message) {
    error.textContent = message;
    error.removeAttribute("hidden");
  }

  function clearError() {
    error.textContent = "";
    error.setAttribute("hidden", "");
    input.removeAttribute("aria-invalid");
  }

  /** G-03a: сброс lurker-состояния при новом preview/закрытии. */
  function resetEnter() {
    entering = false;
    entered = false;
    enterCooling = false;
    if (enterTimer !== null) clearTimeout(enterTimer);
    enterTimer = null;
    openButton = null;
  }

  function hidePreview() {
    resetEnter();
    preview.replaceChildren();
    preview.setAttribute("hidden", "");
    preview.removeAttribute("data-state");
  }

  /** @param {JoinPreview} data */
  function renderPreview(data) {
    resetEnter();
    preview.replaceChildren();
    const avatar = node("span", "join-preview-avatar", monogram(data.guildName));
    avatar.setAttribute("aria-hidden", "true");
    const body = node("div", "join-preview-body");
    const name = node("p", "join-preview-name", data.guildName);
    body.append(name);
    /** @type {string[]} */
    const meta = [];
    if (data.memberCount !== null) meta.push(`${formatNumber(data.memberCount)} участников`);
    if (data.presenceCount !== null) meta.push(`${formatNumber(data.presenceCount)} в сети`);
    if (meta.length > 0) body.append(node("p", "join-preview-meta", meta.join(" · ")));
    preview.append(avatar, body);
    // G-03a: «Открыть сервер» — отдельный lurker-шаг; без guildId кнопки нет.
    if (data.guildId !== "") {
      const openBtn = node("button", "btn btn-secondary join-preview-open", JOIN_TEXT.open);
      openBtn.setAttribute("type", "button");
      openBtn.setAttribute("data-action", "enter-guild");
      openBtn.addEventListener("click", () => {
        void enterStep();
      });
      openButton = /** @type {HTMLElement} */ (openBtn);
      preview.append(openBtn);
      syncEnter();
    }
    preview.setAttribute("data-state", "ready");
    // Только поля из ответа: ни имени, ни счётчиков, ни иконки до 2xx.
    preview.removeAttribute("hidden");
  }

  /** @param {unknown} failure @param {JoinStep} failedStep */
  function showFailure(failure, failedStep) {
    const value = inviteFailure(failure, failedStep);
    if (value.invalidInput) input.setAttribute("aria-invalid", "true");
    if (failedStep === "resolve") {
      current = null;
      hidePreview();
    }
    setPhase("error");
    showError(value.text);
    status.textContent = "";
    if (value.cooldown && value.cooldown > 0) {
      if (failedStep === "enter") startEnterCooldown(value.cooldown);
      else startCooldown(value.cooldown);
    }
  }

  /** @param {number} seconds */
  function startCooldown(seconds) {
    cooling = true;
    submit.setAttribute("data-retry-after", String(seconds));
    syncSubmit();
    if (cooldownTimer !== null) clearTimeout(cooldownTimer);
    cooldownTimer = setTimeout(() => {
      cooldownTimer = null;
      cooling = false;
      submit.removeAttribute("data-retry-after");
      syncSubmit();
    }, seconds * 1000);
  }

  /** G-03a: 429 lurker-шага блокирует только «Открыть сервер». @param {number} seconds */
  function startEnterCooldown(seconds) {
    enterCooling = true;
    if (openButton) openButton.setAttribute("data-retry-after", String(seconds));
    syncEnter();
    if (enterTimer !== null) clearTimeout(enterTimer);
    enterTimer = setTimeout(() => {
      enterTimer = null;
      enterCooling = false;
      if (openButton) openButton.removeAttribute("data-retry-after");
      syncEnter();
    }, seconds * 1000);
  }

  /** @param {boolean} returnFocus */
  function hide(returnFocus) {
    open = false;
    host.setAttribute("hidden", "");
    setPhase("idle");
    clearError();
    hidePreview();
    current = null;
    step = "resolve";
    input.value = "";
    status.textContent = "";
    syncSubmit();
    const previous = opener;
    opener = null;
    if (previous) {
      previous.setAttribute("aria-expanded", "false");
      if (returnFocus && typeof previous.focus === "function") previous.focus();
    }
  }

  /** @param {HTMLElement | null} [openerNode] */
  function openDialog(openerNode) {
    if (disposed) return;
    opener = openerNode ?? /** @type {HTMLElement | null} */ (/** @type {any} */ (document).activeElement);
    open = true;
    host.removeAttribute("hidden");
    setPhase("idle");
    clearError();
    hidePreview();
    current = null;
    step = "resolve";
    input.value = "";
    status.textContent = "";
    syncSubmit();
    if (opener) opener.setAttribute("aria-expanded", "true");
    if (typeof input.focus === "function") input.focus();
  }

  /** Шаг A: `GET /invites/{code}` → preview (§2.3 п.3). */
  async function resolveStep() {
    const code = normalizeInvite(input.value);
    if (code === "") {
      input.setAttribute("aria-invalid", "true");
      setPhase("error");
      showError("Ссылка-приглашение выглядит неверно");
      return;
    }
    pending = true;
    step = "resolve";
    setPhase("resolving");
    syncSubmit();
    status.textContent = JOIN_TEXT.pending;
    try {
      // Таймаут 15 сек: Discord иногда не отвечает на GET /invites/{code}.
      const resolveTimeoutMs = 15_000;
      void invoke("voice_set_audio_device", {
        kind: "ui_telemetry",
        deviceId: JSON.stringify({ type: "JOIN-DEBUG", fn: "resolveStart", code }),
      }).catch(() => {});
      const raw = await Promise.race([
        invoke(JOIN_COMMANDS.resolve, { code }),
        new Promise((_, reject) =>
          setTimeout(() => reject({ kind: "network" }), resolveTimeoutMs)
        ),
      ]);
      if (disposed) return;
      current = parsePreview(raw, code);
      renderPreview(current);
      clearError();
      setPhase("preview_ready");
      status.textContent = `Приглашение в «${current.guildName}» готово`;
      void invoke("voice_set_audio_device", {
        kind: "ui_telemetry",
        deviceId: JSON.stringify({ type: "JOIN-DEBUG", fn: "resolveOK", code, guild: current.guildName, guildId: current.guildId }),
      }).catch(() => {});
    } catch (failure) {
      if (disposed) return;
      void invoke("voice_set_audio_device", {
        kind: "ui_telemetry",
        deviceId: JSON.stringify({ type: "JOIN-DEBUG", fn: "resolveERR", code, failure }),
      }).catch(() => {});
      showFailure(failure, "resolve");
    } finally {
      pending = false;
      syncSubmit();
    }
  }

  /** Шаг B: `POST /invites/{code}` → членство (§2.3 п.4–5). */
  async function acceptStep() {
    const data = current;
    if (!data) return;
    void invoke("voice_set_audio_device", {
      kind: "ui_telemetry",
      deviceId: JSON.stringify({ type: "JOIN-DEBUG", fn: "acceptStep", code: data.code, guild: data.guildName }),
    }).catch(() => {});
    pending = true;
    step = "accept";
    setPhase("accepting");
    syncSubmit();
    status.textContent = JOIN_TEXT.pending;
    try {
      // Таймаут 15 сек: Discord иногда медленно отвечает на POST /invites/{code}.
      const timeoutMs = 15_000;
      const raw = await Promise.race([
        invoke(JOIN_COMMANDS.accept, { code: data.code }),
        new Promise((_, reject) =>
          setTimeout(() => reject({ kind: "network" }), timeoutMs)
        ),
      ]);
      if (disposed) return;
      const guildId = parseAcceptedGuildId(raw);
      void invoke("voice_set_audio_device", {
        kind: "ui_telemetry",
        deviceId: JSON.stringify({ type: "JOIN-DEBUG", fn: "acceptRaw", raw, guildId }),
      }).catch(() => {});
      if (guildId === "") throw /** @type {any} */ ({ kind: "invalidResponse" });
      const guild = { id: guildId, name: data.guildName };
      // Успех подтверждён только 2xx с guild_id: диалог закрывается, рейл/канал
      // обновляет вызывающая сторона (`rail.addGuild` → `channels(guildId)`).
      hide(false);
      setPhase("accepted");
      statusRegion.textContent = JOIN_TEXT.success(guild.name);
      onAccepted(guild);
    } catch (failure) {
      if (disposed) return;
      void invoke("voice_set_audio_device", {
        kind: "ui_telemetry",
        deviceId: JSON.stringify({ type: "JOIN-DEBUG", fn: "acceptERR", failure }),
      }).catch(() => {});
      showFailure(failure, "accept");
    } finally {
      pending = false;
      syncSubmit();
    }
  }

  /**
   * Шаг C (G-03a): `join_guild({guildId})` → lurker-просмотр, 204 без тела.
   * Ни плитки, ни членства, ни accept: успех подтверждается только ответом IPC.
   */
  async function enterStep() {
    const data = current;
    if (disposed || !open || pending || entering || entered || enterCooling) return;
    if (data === null || data.guildId === "") return;
    entering = true;
    step = "enter";
    setPhase("entering");
    clearError();
    syncSubmit();
    syncEnter();
    try {
      await invoke(JOIN_COMMANDS.enter, { guildId: data.guildId });
      if (disposed) return;
      // Тело ответа не читается (успех — 204), полей не выдумываем.
      entered = true;
      clearError();
      setPhase("entered");
      status.textContent = "";
      statusRegion.textContent = JOIN_TEXT.opened;
      if (Boolean(/** @type {any} */ (globalThis).__TAURI_INTERNALS__)) {
        const accepted = {
          id: data.guildId,
          name: data.guildName,
          icon: data.icon,
          memberCount: data.memberCount,
          presenceCount: data.presenceCount,
          targetChannelId: data.channelId,
          targetMessageId: null,
        };
        hide(false);
        onAccepted?.(accepted);
      }
    } catch (failure) {
      if (disposed) return;
      showFailure(failure, "enter");
    } finally {
      entering = false;
      syncSubmit();
      syncEnter();
    }
  }

  // Enter в поле: один путь submit (в браузере гасим нативный submit формы).
  form.addEventListener("submit", (/** @type {any} */ event) => {
    event.preventDefault();
    void submitNow();
  });
  input.addEventListener("keydown", (/** @type {any} */ event) => {
    const key = event && "key" in event ? String(event.key) : "";
    if (key !== "Enter") return;
    event.preventDefault();
    void submitNow();
  });
  input.addEventListener("input", () => {
    input.removeAttribute("aria-invalid");
    if (phase === "error") setPhase("idle");
    syncSubmit();
  });

  /** Публичный submit: 1 клик = 1 IPC; повтор игнорируется. */
  async function submitNow() {
    // Отладочный лог состояния при каждом вызове submit
    void invoke("voice_set_audio_device", {
      kind: "ui_telemetry",
      deviceId: JSON.stringify({ type: "JOIN-DEBUG", fn: "submitNow", disposed, pending, entering, open, hasPreview: current !== null, phase }),
    }).catch(() => {});
    if (disposed || pending || entering || !open) return;
    if (current !== null) {
      await acceptStep();
      return;
    }
    await resolveStep();
  }

  syncSubmit();

  return {
    /** @param {HTMLElement | null} [node] */
    open: openDialog,
    submit: submitNow,
    /** G-03a: «Открыть сервер» (lurker); повтор в entering/entered игнорируется. */
    enterGuild: enterStep,
    /** @param {boolean} [returnFocus] */
    cancel(returnFocus = true) {
      // Разрешаем отмену даже в pending: пользователь должен иметь выход если IPC завис.
      // При отмене во время pending pending=false сбросится когда invoke завершится (finally).
      if (entering) {
        if (!Boolean(/** @type {any} */ (globalThis).__TAURI_INTERNALS__)) return;
        entering = false;
        syncEnter();
        syncSubmit();
      }
      // Флаг disposed=false — окно прячется, но если invoke ответит после hide,
      // disposed проверяется первым — ответ просто игнорируется.
      hide(returnFocus);
    },
    get state() { return phase; },
    get step() { return step; },
    get pending() { return pending; },
    get busy() { return pending || entering; },
    get entered() { return entered; },
    get isOpen() { return open; },
    get card() { return card; },
    get preview() { return current; },
    get opener() { return opener; },
    dispose() {
      disposed = true;
      if (cooldownTimer !== null) clearTimeout(cooldownTimer);
      cooldownTimer = null;
      hide(false);
    },
  };
}
