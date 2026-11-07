// @ts-check
/**
 * STUB-05-INVITES-MODAL-UI / G-06: модалка создания приглашения на сервер/канал.
 *
 * Спецификация:
 *   - `waves/RW-40/DESIGN/INVITES-SPEC.md` §1 (DOM-структура, a11y, состояния data-state, clipboard);
 *   - `waves/RW-40/RE-DEEP-REA.md` §4.3 (создание `POST /channels/{id}/invites`, опции max_age/max_uses);
 *   - `rust/gui-spike/src/invites.rs` (параметры create_invite, константы, коды ошибок).
 */
import { byId, make } from "./dom.js";

/** Допустимые значения срока действия приглашения в секундах (RE §4.3, INVITES-SPEC §1.2). */
export const VALID_INVITE_MAX_AGES = Object.freeze([0, 1800, 3600, 21600, 43200, 86400, 604800]);

/** Допустимые значения максимального количества использований (RE §4.3, INVITES-SPEC §1.2). */
export const VALID_INVITE_MAX_USES = Object.freeze([0, 1, 5, 10, 25, 50, 100]);

/** Битовый флаг гостевого приглашения (IS_GUEST_INVITE = 1 << 0). */
export const INVITE_FLAG_IS_GUEST = 1;

/**
 * Справочник опций срока действия приглашения на русском языке (T-231 / FE-P1-24).
 * @type {ReadonlyArray<Readonly<{ value: number, label: string }>>}
 */
export const INVITE_MAX_AGE_OPTIONS_RU = Object.freeze([
  Object.freeze({ value: 1800, label: "30 минут" }),
  Object.freeze({ value: 3600, label: "1 час" }),
  Object.freeze({ value: 21600, label: "6 часов" }),
  Object.freeze({ value: 43200, label: "12 часов" }),
  Object.freeze({ value: 86400, label: "1 день" }),
  Object.freeze({ value: 604800, label: "7 дней" }),
  Object.freeze({ value: 0, label: "Бессрочно" }),
]);

/**
 * Справочник опций лимита использований приглашения на русском языке (T-231 / FE-P1-24).
 * @type {ReadonlyArray<Readonly<{ value: number, label: string }>>}
 */
export const INVITE_MAX_USES_OPTIONS_RU = Object.freeze([
  Object.freeze({ value: 0, label: "Без ограничений" }),
  Object.freeze({ value: 1, label: "1 использование" }),
  Object.freeze({ value: 5, label: "5 использований" }),
  Object.freeze({ value: 10, label: "10 использований" }),
  Object.freeze({ value: 25, label: "25 использований" }),
  Object.freeze({ value: 100, label: "100 использований" }),
]);

/**
 * Форматирует код приглашения в полную ссылку `https://discord.gg/<code>`.
 * Обрезает крайние пробелы; возвращает пустую строку для пустого/невалидного кода.
 *
 * @param {string} code
 * @returns {string}
 */
export function formatInviteUrl(code) {
  if (typeof code !== "string") return "";
  const trimmed = code.trim();
  if (!trimmed) return "";
  return `https://discord.gg/${trimmed}`;
}

/** Текстовые метки для селекта срока действия приглашения. */
const AGE_LABELS = Object.freeze({
  1800: "30 минут",
  3600: "1 час",
  21600: "6 часов",
  43200: "12 часов",
  86400: "1 день",
  604800: "7 дней",
  0: "Бессрочно",
});

/** Текстовые метки для селекта количества использований. */
const USES_LABELS = Object.freeze({
  0: "Без ограничений",
  1: "1 использование",
  5: "5 использований",
  10: "10 использований",
  25: "25 использований",
  50: "50 использований",
  100: "100 использований",
});

/**
 * @typedef {{
 *   code: string,
 *   maxAge?: number,
 *   maxUses?: number,
 *   temporary?: boolean,
 * }} CreatedInviteResult
 *
 * @typedef {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   channelId?: string,
 *   onCreated?: (invite: CreatedInviteResult) => void,
 *   onClose?: () => void,
 * }} InviteModalOptions
 *
 * @typedef {{
 *   channelId?: string,
 *   channelName?: string,
 *   triggerElement?: HTMLElement | any | null,
 * }} OpenInviteOptions
 */

/**
 * Извлекает время ожидания retry_after в миллисекундах из ошибки IPC/REST.
 * @param {unknown} err
 * @returns {number}
 */
function extractRetryAfterMs(err) {
  if (!err || typeof err !== "object") return 0;
  const bag = /** @type {Record<string, unknown>} */ (err);
  if (typeof bag.retryAfterMs === "number" && Number.isFinite(bag.retryAfterMs)) return bag.retryAfterMs;
  if (typeof bag.retry_after_ms === "number" && Number.isFinite(bag.retry_after_ms)) return bag.retry_after_ms;
  if (typeof bag.retryAfter === "number" && Number.isFinite(bag.retryAfter)) return bag.retryAfter * 1000;
  if (typeof bag.retry_after === "number" && Number.isFinite(bag.retry_after)) return bag.retry_after * 1000;
  return 0;
}

/**
 * Создаёт контроллер модалки создания приглашения #invite-create-modal.
 * @param {InviteModalOptions} options
 */
export function createInviteModal(options) {
  const { invoke } = options;

  /** @param {string} tag @param {string} [className] @param {string} [value] @returns {any} */
  const node = (tag, className, value) => make(tag, className, value);

  // Host: backdrop диалога. В браузере может быть в разметке, в шиме монтируется динамически.
  const host = byId("create-invite-dialog") ?? node("div", "dialog-backdrop");
  host.id = "create-invite-dialog";
  host.className = "dialog-backdrop";
  host.setAttribute("role", "presentation");
  host.setAttribute("hidden", "");
  if (!host.parentNode) document.body.append(host);
  host.replaceChildren();

  // Card: сам диалог создания приглашения
  const card = node("div", "dialog invite-modal");
  card.id = "invite-create-modal";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("aria-label", "Пригласить людей");
  card.setAttribute("aria-labelledby", "create-invite-title");
  card.setAttribute("aria-describedby", "create-invite-desc");
  card.setAttribute("data-state", "idle");
  card.setAttribute("hidden", "");

  // Шапка с заголовком, подсказкой и кнопкой закрытия
  const header = node("header", "dialog-header");
  const title = node("h2", "dialog-title", "Пригласить людей");
  title.id = "create-invite-title";
  const desc = node("p", "dialog-hint");
  desc.id = "create-invite-desc";
  const descPrefix = node("span", "", "Ссылка пригласит пользователя в канал ");
  const channelNameNode = node("strong", "", "#общий");
  channelNameNode.id = "create-invite-channel-name";
  desc.append(descPrefix, channelNameNode);

  const closeBtn = node("button", "icon-btn dialog-close");
  closeBtn.setAttribute("type", "button");
  closeBtn.setAttribute("data-action", "create-invite-close");
  closeBtn.setAttribute("aria-label", "Закрыть");
  const closeIcon = node("span", "", "✕");
  closeIcon.setAttribute("aria-hidden", "true");
  closeBtn.append(closeIcon);
  header.append(title, desc, closeBtn);

  // Секция ссылки и кнопки копирования
  const linkSection = node("div", "invite-link-section");
  const linkLabel = node("label", "dialog-field-label", "Отправьте эту ссылку друзьям");
  linkLabel.setAttribute("for", "invite-link-input");

  const linkBox = node("div", "invite-link-box");
  const linkInput = /** @type {HTMLInputElement} */ (node("input", "dialog-input invite-link-input"));
  linkInput.id = "invite-link-input";
  linkInput.setAttribute("type", "text");
  linkInput.setAttribute("readonly", "");
  linkInput.setAttribute("spellcheck", "false");
  linkInput.setAttribute("aria-label", "Ссылка-приглашение");
  linkInput.placeholder = "https://discord.gg/...";

  const copyBtn = node("button", "btn btn-primary invite-copy-btn", "Копировать");
  copyBtn.id = "invite-copy-btn";
  copyBtn.setAttribute("type", "button");
  copyBtn.setAttribute("data-action", "create-invite-copy");
  linkBox.append(linkInput, copyBtn);

  const copyFeedback = node("p", "dialog-hint invite-copy-status");
  copyFeedback.id = "invite-copy-feedback";
  copyFeedback.setAttribute("role", "status");
  copyFeedback.setAttribute("aria-live", "polite");
  linkSection.append(linkLabel, linkBox, copyFeedback);

  // Аккордеон настроек
  const accordion = node("div", "invite-settings-accordion");
  const settingsToggle = node("button", "invite-settings-toggle");
  settingsToggle.id = "invite-settings-toggle";
  settingsToggle.setAttribute("type", "button");
  settingsToggle.setAttribute("data-action", "create-invite-toggle-settings");
  settingsToggle.setAttribute("aria-expanded", "false");
  settingsToggle.setAttribute("aria-controls", "invite-settings-panel");

  const toggleIcon = node("span", "invite-settings-icon", "⚙️");
  toggleIcon.setAttribute("aria-hidden", "true");
  const toggleLabel = node("span", "", "Настроить ссылку приглашения");
  const toggleArrow = node("span", "invite-accordion-arrow", "▼");
  toggleArrow.setAttribute("aria-hidden", "true");
  settingsToggle.append(toggleIcon, toggleLabel, toggleArrow);

  const settingsPanel = node("form", "invite-settings-panel");
  settingsPanel.id = "invite-settings-panel";
  settingsPanel.setAttribute("novalidate", "");
  settingsPanel.setAttribute("hidden", "");

  // Поле max_age
  const ageField = node("div", "dialog-field");
  const ageLabel = node("label", "dialog-field-label", "Устареет через");
  ageLabel.setAttribute("for", "invite-max-age");
  const maxAgeSelect = /** @type {HTMLSelectElement} */ (node("select", "dialog-select"));
  maxAgeSelect.id = "invite-max-age";
  maxAgeSelect.setAttribute("name", "max_age");
  maxAgeSelect.value = "86400";
  for (const age of VALID_INVITE_MAX_AGES) {
    const label = /** @type {Record<number, string>} */ (AGE_LABELS)[age] ?? `${age} с`;
    const opt = node("option", "", label);
    opt.setAttribute("value", String(age));
    if (age === 86400) opt.setAttribute("selected", "");
    maxAgeSelect.append(opt);
  }
  ageField.append(ageLabel, maxAgeSelect);

  // Поле max_uses
  const usesField = node("div", "dialog-field");
  const usesLabel = node("label", "dialog-field-label", "Максимальное количество использований");
  usesLabel.setAttribute("for", "invite-max-uses");
  const maxUsesSelect = /** @type {HTMLSelectElement} */ (node("select", "dialog-select"));
  maxUsesSelect.id = "invite-max-uses";
  maxUsesSelect.setAttribute("name", "max_uses");
  maxUsesSelect.value = "0";
  for (const uses of VALID_INVITE_MAX_USES) {
    const label = /** @type {Record<number, string>} */ (USES_LABELS)[uses] ?? `${uses}`;
    const opt = node("option", "", label);
    opt.setAttribute("value", String(uses));
    if (uses === 0) opt.setAttribute("selected", "");
    maxUsesSelect.append(opt);
  }
  usesField.append(usesLabel, maxUsesSelect);

  // Тумблер temporary
  const tempRow = node("div", "dialog-toggle-row");
  const tempTextBlock = node("div", "toggle-text-block");
  const tempLabel = node("label", "dialog-toggle-label", "Предоставить временное членство");
  tempLabel.setAttribute("for", "invite-temporary-toggle");
  const tempHint = node("span", "dialog-hint", "Пользователь будет исключён при отключении, если ему не назначена роль");
  tempTextBlock.append(tempLabel, tempHint);
  const tempInput = /** @type {HTMLInputElement} */ (node("input", "dialog-switch"));
  tempInput.id = "invite-temporary-toggle";
  tempInput.setAttribute("type", "checkbox");
  tempInput.setAttribute("name", "temporary");
  tempInput.checked = false;
  tempRow.append(tempTextBlock, tempInput);

  // Тумблер guest
  const guestRow = node("div", "dialog-toggle-row");
  const guestTextBlock = node("div", "toggle-text-block");
  const guestLabel = node("label", "dialog-toggle-label", "Гостевое приглашение");
  guestLabel.setAttribute("for", "invite-guest-toggle");
  const guestHint = node("span", "dialog-hint", "Даёт доступ только к текущему каналу без постоянного членства");
  guestTextBlock.append(guestLabel, guestHint);
  const guestInput = /** @type {HTMLInputElement} */ (node("input", "dialog-switch"));
  guestInput.id = "invite-guest-toggle";
  guestInput.setAttribute("type", "checkbox");
  guestInput.setAttribute("name", "guest");
  guestInput.checked = false;
  guestRow.append(guestTextBlock, guestInput);

  // Кнопка генерации новой ссылки
  const generateBtn = node("button", "btn btn-secondary invite-generate-btn", "Сгенерировать новую ссылку");
  generateBtn.id = "invite-generate-btn";
  generateBtn.setAttribute("type", "button");
  generateBtn.setAttribute("data-action", "create-invite-generate");

  settingsPanel.append(ageField, usesField, tempRow, guestRow, generateBtn);
  accordion.append(settingsToggle, settingsPanel);

  // Блок отображения ошибок
  const errorBanner = node("div", "dialog-error invite-error-banner");
  errorBanner.id = "create-invite-error";
  errorBanner.setAttribute("role", "alert");
  errorBanner.setAttribute("aria-live", "assertive");
  errorBanner.setAttribute("hidden", "");
  const errorIcon = node("span", "invite-error-icon", "⚠️");
  errorIcon.setAttribute("aria-hidden", "true");
  const errorText = node("span", "invite-error-text");
  errorText.id = "create-invite-error-text";
  errorBanner.append(errorIcon, errorText);

  // Футер
  const footer = node("footer", "dialog-actions");
  const doneBtn = node("button", "btn btn-secondary", "Готово");
  doneBtn.setAttribute("type", "button");
  doneBtn.setAttribute("data-action", "create-invite-close");
  footer.append(doneBtn);

  card.append(header, linkSection, accordion, errorBanner, footer);
  host.append(card);

  // Состояние контроллера
  /** @type {"idle" | "loading" | "populated" | "error-429" | "error-403" | "error"} */
  let phase = "idle";
  let activeChannelId = options.channelId ?? "";
  /** @type {HTMLElement | null} */
  let opener = null;
  let isOpen = false;
  let isGenerating = false;
  let isCooling429 = false;
  let isForbidden403 = false;
  let disposed = false;
  /** @type {ReturnType<typeof setTimeout> | null} */
  let copyResetTimer = null;
  /** @type {ReturnType<typeof setInterval> | null} */
  let countdownTimer = null;
  let lastCode = "";

  /**
   * Устанавливает фазу data-state на контейнере модалки.
   * @param {"idle" | "loading" | "populated" | "error-429" | "error-403" | "error"} next
   */
  function setState(next) {
    phase = next;
    card.setAttribute("data-state", next);
  }

  function clearErrorBanner() {
    errorBanner.setAttribute("hidden", "");
    errorText.textContent = "";
  }

  /**
   * Копирование ссылки в буфер обмена с честной обработкой ошибок.
   */
  async function copyLink() {
    if (!linkInput.value || phase !== "populated") return;
    const url = linkInput.value;

    /** @type {any} */
    let nav = typeof navigator !== "undefined" ? navigator : null;
    if (typeof window !== "undefined" && /** @type {any} */ (window).navigator) {
      nav = /** @type {any} */ (window).navigator;
    }

    if (nav && nav.clipboard && typeof nav.clipboard.writeText === "function") {
      try {
        await nav.clipboard.writeText(url);
        copyBtn.classList.add("btn-positive");
        const originalText = copyBtn.textContent || "Копировать";
        copyBtn.textContent = "Скопировано!";
        copyFeedback.textContent = "Ссылка скопирована";

        if (copyResetTimer !== null) clearTimeout(copyResetTimer);
        copyResetTimer = setTimeout(() => {
          copyBtn.classList.remove("btn-positive");
          copyBtn.textContent = originalText;
          copyFeedback.textContent = "";
          copyResetTimer = null;
        }, 2000);
      } catch {
        copyBtn.classList.remove("btn-positive");
        copyFeedback.textContent = "Не удалось скопировать";
        if (typeof linkInput.select === "function") {
          linkInput.select();
        } else if (typeof linkInput.setSelectionRange === "function") {
          linkInput.setSelectionRange(0, linkInput.value.length);
        }
      }
    } else {
      copyBtn.classList.remove("btn-positive");
      copyFeedback.textContent = "Не удалось скопировать";
      if (typeof linkInput.select === "function") {
        linkInput.select();
      } else if (typeof linkInput.setSelectionRange === "function") {
        linkInput.setSelectionRange(0, linkInput.value.length);
      }
    }
  }

  /**
   * Вызывает команду IPC create_invite и обновляет интерфейс.
   */
  async function generateNow() {
    if (disposed || isGenerating || !activeChannelId) return;
    if (isCooling429 || isForbidden403) return;

    isGenerating = true;
    setState("loading");
    clearErrorBanner();
    linkInput.placeholder = "Генерация ссылки...";
    linkInput.setAttribute("aria-busy", "true");
    copyBtn.setAttribute("disabled", "");
    generateBtn.setAttribute("disabled", "");

    const maxAge = Number(maxAgeSelect.value);
    const maxUses = Number(maxUsesSelect.value);
    const temporary = Boolean(tempInput.checked);
    const flags = guestInput.checked ? INVITE_FLAG_IS_GUEST : null;

    try {
      const res = await invoke("create_invite", {
        channelId: activeChannelId,
        maxAge,
        maxUses,
        temporary,
        flags,
      });
      if (disposed) return;
      const code = res && typeof res.code === "string" ? res.code : "";
      lastCode = code;
      linkInput.value = `https://discord.gg/${code}`;
      setState("populated");
      copyBtn.removeAttribute("disabled");
      if (typeof options.onCreated === "function") {
        options.onCreated(res);
      }
    } catch (err) {
      if (disposed) return;
      handleError(err);
    } finally {
      isGenerating = false;
      linkInput.removeAttribute("aria-busy");
      if (!isCooling429 && !isForbidden403) {
        generateBtn.removeAttribute("disabled");
      }
    }
  }

  /**
   * Обработка ошибок (429, 403, прочие).
   * @param {unknown} err
   */
  function handleError(err) {
    const kind = err && typeof err === "object" && "kind" in err ? String(/** @type {any} */ (err).kind) : "";
    const status = err && typeof err === "object" && "status" in err ? Number(/** @type {any} */ (err).status) : 0;

    if (kind === "rateLimited" || status === 429) {
      setState("error-429");
      isCooling429 = true;
      generateBtn.setAttribute("disabled", "");
      copyBtn.setAttribute("disabled", "");
      errorBanner.removeAttribute("hidden");

      const ms = extractRetryAfterMs(err);
      let seconds = Math.max(1, Math.ceil(ms / 1000));
      errorText.textContent = `Слишком много запросов. Повторите через ${seconds} с.`;

      if (countdownTimer !== null) clearInterval(countdownTimer);
      countdownTimer = setInterval(() => {
        seconds -= 1;
        if (seconds > 0) {
          errorText.textContent = `Слишком много запросов. Повторите через ${seconds} с.`;
        } else {
          if (countdownTimer !== null) clearInterval(countdownTimer);
          countdownTimer = null;
          isCooling429 = false;
          clearErrorBanner();
          generateBtn.removeAttribute("disabled");
          if (lastCode) {
            setState("populated");
            copyBtn.removeAttribute("disabled");
          } else {
            setState("idle");
          }
        }
      }, 1000);
      return;
    }

    if (kind === "forbidden" || status === 403) {
      setState("error-403");
      isForbidden403 = true;
      errorBanner.removeAttribute("hidden");
      errorText.textContent = "У вас нет прав на создание приглашений в этот канал.";
      generateBtn.setAttribute("disabled", "");
      copyBtn.setAttribute("disabled", "");
      maxAgeSelect.setAttribute("disabled", "");
      maxUsesSelect.setAttribute("disabled", "");
      tempInput.setAttribute("disabled", "");
      guestInput.setAttribute("disabled", "");
      return;
    }

    setState("error");
    errorBanner.removeAttribute("hidden");
    errorText.textContent = "Не удалось создать приглашение";
  }

  /**
   * Закрывает модалку и возвращает фокус на элемент-триггер.
   */
  function close() {
    if (!isOpen) return;
    isOpen = false;
    host.setAttribute("hidden", "");
    card.setAttribute("hidden", "");
    if (opener && typeof opener.focus === "function") {
      opener.focus();
    }
    if (typeof options.onClose === "function") {
      options.onClose();
    }
  }

  /**
   * Открывает модалку создания приглашения.
   * @param {string | HTMLElement | OpenInviteOptions | null} [target]
   */
  function open(target) {
    if (disposed) return;
    isOpen = true;
    isForbidden403 = false;
    isCooling429 = false;

    if (typeof target === "string") {
      activeChannelId = target;
    } else if (target && typeof target === "object") {
      if ("channelId" in target && typeof target.channelId === "string") {
        activeChannelId = target.channelId;
      }
      if ("channelName" in target && typeof target.channelName === "string") {
        channelNameNode.textContent = `#${target.channelName.replace(/^#/, "")}`;
      }
      if ("triggerElement" in target && target.triggerElement) {
        opener = target.triggerElement;
      } else if ("tagName" in target || "focus" in target) {
        opener = /** @type {HTMLElement} */ (target);
      }
    }

    if (!opener && typeof document !== "undefined" && document.activeElement) {
      opener = /** @type {HTMLElement} */ (document.activeElement);
    }

    host.removeAttribute("hidden");
    card.removeAttribute("hidden");

    if (typeof closeBtn.focus === "function") {
      closeBtn.focus();
    }

    if (activeChannelId) {
      void generateNow();
    } else {
      setState("idle");
    }
  }

  // Обработчики кнопок аккордеона, генерации и копирования
  settingsToggle.addEventListener("click", () => {
    const expanded = settingsToggle.getAttribute("aria-expanded") === "true";
    settingsToggle.setAttribute("aria-expanded", String(!expanded));
    if (expanded) {
      settingsPanel.setAttribute("hidden", "");
    } else {
      settingsPanel.removeAttribute("hidden");
    }
  });

  settingsPanel.addEventListener("submit", (/** @type {any} */ e) => {
    e.preventDefault();
    void generateNow();
  });

  generateBtn.addEventListener("click", () => {
    void generateNow();
  });

  copyBtn.addEventListener("click", () => {
    void copyLink();
  });

  closeBtn.addEventListener("click", () => close());
  doneBtn.addEventListener("click", () => close());

  // Клик по backdrop закрывает модалку, клик внутри карточки не всплывает
  host.addEventListener("click", (/** @type {any} */ e) => {
    if (e.target === host) close();
  });
  card.addEventListener("click", (/** @type {any} */ e) => {
    if (typeof e.stopPropagation === "function") e.stopPropagation();
  });

  // Клавиатура: закрытие по Escape с перехватом
  /** @param {any} event */
  function onKeyDown(event) {
    if (!isOpen) return;
    const key = event && "key" in event ? String(event.key) : "";
    if (key === "Escape") {
      if (typeof event.preventDefault === "function") event.preventDefault();
      if (typeof event.stopPropagation === "function") event.stopPropagation();
      close();
    }
  }

  if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
    document.addEventListener("keydown", onKeyDown);
  }
  if (typeof document !== "undefined" && document.body && typeof document.body.addEventListener === "function") {
    document.body.addEventListener("keydown", onKeyDown);
  }
  if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
    window.addEventListener("keydown", onKeyDown);
  }
  host.addEventListener("keydown", onKeyDown);
  card.addEventListener("keydown", onKeyDown);

  return {
    open,
    close,
    generate: generateNow,
    copy: copyLink,
    modal: card,
    card,
    dialog: card,
    host,
    input: linkInput,
    get state() {
      return card.getAttribute("data-state") || "idle";
    },
    get isOpen() {
      return isOpen;
    },
    dispose() {
      disposed = true;
      close();
      if (countdownTimer !== null) clearInterval(countdownTimer);
      if (copyResetTimer !== null) clearTimeout(copyResetTimer);
      if (typeof document !== "undefined" && typeof document.removeEventListener === "function") {
        document.removeEventListener("keydown", onKeyDown);
      }
      if (typeof document !== "undefined" && document.body && typeof document.body.removeEventListener === "function") {
        document.body.removeEventListener("keydown", onKeyDown);
      }
      if (typeof window !== "undefined" && typeof window.removeEventListener === "function") {
        window.removeEventListener("keydown", onKeyDown);
      }
      host.removeEventListener("keydown", onKeyDown);
      card.removeEventListener("keydown", onKeyDown);
    },
  };
}

/**
 * @typedef {object} InviteFriendEntry
 * @property {string} [id]
 * @property {string} [userId]
 * @property {string} username
 * @property {string} [displayName]
 */

/**
 * @typedef {object} InviteSettingsState
 * @property {number} maxAge
 * @property {number} maxUses
 * @property {boolean} temporary
 */

/**
 * @typedef {object} OpenInviteModalPayload
 * @property {string} [channelName]
 * @property {string} [inviteCode]
 * @property {ReadonlyArray<InviteFriendEntry>} [friends]
 * @property {number} [maxAge]
 * @property {number} [maxUses]
 * @property {boolean} [temporary]
 */

/**
 * @typedef {object} InviteModalControllerOptions
 * @property {any} [container]
 * @property {(url: string) => void} [onCopyLink]
 * @property {(userId: string) => void} [onSendFriendInvite]
 * @property {(settings: InviteSettingsState) => any} [onRegenerateInvite]
 */

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveControllerDocument(containerEl) {
  if (
    containerEl &&
    containerEl.ownerDocument &&
    typeof containerEl.ownerDocument.createElement === "function"
  ) {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === "function") {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === "function") {
    return globalDoc;
  }
  throw new TypeError("Container element must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы контейнера на переданный массив элементов.
 *
 * @param {any} containerEl
 * @param {any[]} nodes
 */
function replaceControllerChildren(containerEl, nodes) {
  if (!containerEl) return;
  if (typeof containerEl.replaceChildren === "function") {
    containerEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(containerEl.children)) {
    containerEl.children.length = 0;
  }
  for (const node of nodes) {
    containerEl.appendChild(node);
  }
}

/**
 * Проверяет, допустимо ли значение срока действия инвайта в `INVITE_MAX_AGE_OPTIONS_RU`.
 *
 * @param {unknown} val
 * @returns {val is number}
 */
function isValidMaxAgeOption(val) {
  return (
    typeof val === "number" &&
    Number.isFinite(val) &&
    INVITE_MAX_AGE_OPTIONS_RU.some((opt) => opt.value === val)
  );
}

/**
 * Проверяет, допустимо ли значение лимита использований в `INVITE_MAX_USES_OPTIONS_RU`.
 *
 * @param {unknown} val
 * @returns {val is number}
 */
function isValidMaxUsesOption(val) {
  return (
    typeof val === "number" &&
    Number.isFinite(val) &&
    INVITE_MAX_USES_OPTIONS_RU.some((opt) => opt.value === val)
  );
}

/**
 * Создаёт контроллер модалки создания приглашения на сервер, настроек ссылки
 * и прямого приглашения друзей (T-231 / FE-P1-24).
 *
 * @param {InviteModalControllerOptions} [options]
 */
export function createInviteModalController(options = {}) {
  const {
    container = null,
    onCopyLink,
    onSendFriendInvite,
    onRegenerateInvite,
  } = options ?? {};

  let isOpen = false;
  let channelName = "";
  let inviteCode = "";
  /** @type {Array<{ id: string, username: string, displayName: string }>} */
  let friends = [];
  /** @type {Set<string>} */
  const invitedUserIds = new Set();
  let searchQuery = "";
  /** @type {InviteSettingsState} */
  const settings = {
    maxAge: 86400,
    maxUses: 0,
    temporary: false,
  };

  /** @type {any} */
  let friendListHostEl = null;
  /** @type {any} */
  let searchInputEl = null;
  /** @type {any} */
  let urlInputEl = null;
  /** @type {any} */
  let copyBtnEl = null;
  /** @type {any} */
  let maxAgeSelectEl = null;
  /** @type {any} */
  let maxUsesSelectEl = null;
  /** @type {any} */
  let tempCheckboxEl = null;

  /**
   * Рендерит отфильтрованные строки друзей внутри `friendListHostEl`.
   */
  function renderFriendRows() {
    if (!friendListHostEl || !container) return;
    const doc = resolveControllerDocument(container);
    const needle = searchQuery.trim().toLowerCase();

    const filtered = friends.filter((f) => {
      if (!needle) return true;
      return (
        f.displayName.toLowerCase().includes(needle) ||
        f.username.toLowerCase().includes(needle)
      );
    });

    /** @type {any[]} */
    const rowNodes = [];
    for (const friend of filtered) {
      const row = doc.createElement("div");
      row.setAttribute("class", "invite-friend-row");
      row.setAttribute("data-invite-friend-row", "true");
      row.setAttribute("data-user-id", friend.id);

      const nameSpan = doc.createElement("span");
      nameSpan.setAttribute("class", "invite-friend-name");
      nameSpan.textContent =
        friend.displayName && friend.displayName !== friend.username
          ? `${friend.displayName} (${friend.username})`
          : friend.displayName || friend.username;
      row.appendChild(nameSpan);

      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("class", "btn btn-secondary invite-friend-btn");
      btn.setAttribute("data-invite-friend-btn", "true");
      btn.setAttribute("data-user-id", friend.id);

      const alreadyInvited = invitedUserIds.has(friend.id);
      btn.textContent = alreadyInvited ? "Отправлено" : "Пригласить";
      btn.disabled = alreadyInvited;
      if (alreadyInvited) {
        btn.setAttribute("disabled", "");
      }

      btn.addEventListener("click", () => {
        if (invitedUserIds.has(friend.id) || btn.disabled) return;
        invitedUserIds.add(friend.id);
        btn.textContent = "Отправлено";
        btn.disabled = true;
        btn.setAttribute("disabled", "");
        if (typeof onSendFriendInvite === "function") {
          onSendFriendInvite(friend.id);
        }
      });

      row.appendChild(btn);
      rowNodes.push(row);
    }

    replaceControllerChildren(friendListHostEl, rowNodes);
  }

  /**
   * Синхронизирует DOM-элементы панели настроек с объектом `settings`.
   */
  function syncSettingsDom() {
    if (maxAgeSelectEl) {
      maxAgeSelectEl.value = String(settings.maxAge);
    }
    if (maxUsesSelectEl) {
      maxUsesSelectEl.value = String(settings.maxUses);
    }
    if (tempCheckboxEl) {
      tempCheckboxEl.checked = Boolean(settings.temporary);
      if (settings.temporary) {
        tempCheckboxEl.setAttribute("checked", "");
      } else {
        tempCheckboxEl.removeAttribute("checked");
      }
    }
  }

  /**
   * Рендерит модальное окно `[data-invite-modal]` в `container`.
   */
  function renderModal() {
    if (!container) return;
    if (!isOpen) {
      replaceControllerChildren(container, []);
      return;
    }

    const doc = resolveControllerDocument(container);

    const dialog = doc.createElement("div");
    dialog.setAttribute("class", "dialog invite-modal");
    dialog.setAttribute("data-invite-modal", "true");
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");

    // Заголовок канала
    const header = doc.createElement("header");
    header.setAttribute("class", "dialog-header");
    const titleEl = doc.createElement("h2");
    titleEl.setAttribute("class", "dialog-title");
    titleEl.setAttribute("data-invite-channel-title", "true");
    const cleanChannel = channelName.replace(/^#/, "").trim() || "общий";
    titleEl.textContent = `Пригласить друзей в #${cleanChannel}`;
    header.appendChild(titleEl);
    dialog.appendChild(header);

    // Поиск друзей и список друзей
    const friendsSection = doc.createElement("section");
    friendsSection.setAttribute("class", "invite-friends-section");

    const searchInput = doc.createElement("input");
    searchInput.setAttribute("type", "search");
    searchInput.setAttribute("class", "dialog-input invite-friend-search");
    searchInput.setAttribute("data-invite-friend-search", "true");
    searchInput.setAttribute("placeholder", "Поиск друзей");
    searchInput.value = searchQuery;
    searchInput.addEventListener("input", () => {
      searchQuery = String(searchInput.value ?? "");
      renderFriendRows();
    });
    searchInputEl = searchInput;
    friendsSection.appendChild(searchInput);

    const friendListHost = doc.createElement("div");
    friendListHost.setAttribute("class", "invite-friend-list");
    friendListHost.setAttribute("data-invite-friend-list", "true");
    friendListHostEl = friendListHost;
    friendsSection.appendChild(friendListHost);
    dialog.appendChild(friendsSection);

    // Поле ссылки и кнопка копирования
    const linkSection = doc.createElement("section");
    linkSection.setAttribute("class", "invite-link-section");

    const urlInput = doc.createElement("input");
    urlInput.setAttribute("type", "text");
    urlInput.setAttribute("readonly", "");
    urlInput.setAttribute("class", "dialog-input invite-url-input");
    urlInput.setAttribute("data-invite-url-input", "true");
    urlInput.value = formatInviteUrl(inviteCode);
    urlInputEl = urlInput;
    linkSection.appendChild(urlInput);

    const copyBtn = doc.createElement("button");
    copyBtn.setAttribute("type", "button");
    copyBtn.setAttribute("class", "btn btn-primary invite-copy-btn");
    copyBtn.setAttribute("data-invite-copy-btn", "true");
    copyBtn.textContent = "Копировать";
    copyBtn.addEventListener("click", () => {
      const currentUrl = urlInputEl ? String(urlInputEl.value ?? "") : formatInviteUrl(inviteCode);
      copyBtn.textContent = "Скопировано!";
      if (typeof onCopyLink === "function") {
        onCopyLink(currentUrl);
      }
    });
    copyBtnEl = copyBtn;
    linkSection.appendChild(copyBtn);
    dialog.appendChild(linkSection);

    // Панель настроек приглашения
    const settingsSection = doc.createElement("section");
    settingsSection.setAttribute("class", "invite-settings-panel");
    settingsSection.setAttribute("data-invite-settings-panel", "true");

    const maxAgeSelect = doc.createElement("select");
    maxAgeSelect.setAttribute("class", "dialog-select");
    maxAgeSelect.setAttribute("data-invite-max-age", "true");
    for (const optDef of INVITE_MAX_AGE_OPTIONS_RU) {
      const opt = doc.createElement("option");
      opt.setAttribute("value", String(optDef.value));
      opt.textContent = optDef.label;
      if (optDef.value === settings.maxAge) {
        opt.setAttribute("selected", "");
      }
      maxAgeSelect.appendChild(opt);
    }
    maxAgeSelect.value = String(settings.maxAge);
    maxAgeSelect.addEventListener("change", () => {
      const nextAge = Number(maxAgeSelect.value);
      if (isValidMaxAgeOption(nextAge)) {
        settings.maxAge = nextAge;
      }
    });
    maxAgeSelectEl = maxAgeSelect;
    settingsSection.appendChild(maxAgeSelect);

    const maxUsesSelect = doc.createElement("select");
    maxUsesSelect.setAttribute("class", "dialog-select");
    maxUsesSelect.setAttribute("data-invite-max-uses", "true");
    for (const optDef of INVITE_MAX_USES_OPTIONS_RU) {
      const opt = doc.createElement("option");
      opt.setAttribute("value", String(optDef.value));
      opt.textContent = optDef.label;
      if (optDef.value === settings.maxUses) {
        opt.setAttribute("selected", "");
      }
      maxUsesSelect.appendChild(opt);
    }
    maxUsesSelect.value = String(settings.maxUses);
    maxUsesSelect.addEventListener("change", () => {
      const nextUses = Number(maxUsesSelect.value);
      if (isValidMaxUsesOption(nextUses)) {
        settings.maxUses = nextUses;
      }
    });
    maxUsesSelectEl = maxUsesSelect;
    settingsSection.appendChild(maxUsesSelect);

    const tempCheckbox = doc.createElement("input");
    tempCheckbox.setAttribute("type", "checkbox");
    tempCheckbox.setAttribute("class", "dialog-switch");
    tempCheckbox.setAttribute("data-invite-temporary", "true");
    tempCheckbox.checked = Boolean(settings.temporary);
    tempCheckbox.addEventListener("change", () => {
      settings.temporary = Boolean(tempCheckbox.checked);
    });
    tempCheckboxEl = tempCheckbox;
    settingsSection.appendChild(tempCheckbox);

    const regenBtn = doc.createElement("button");
    regenBtn.setAttribute("type", "button");
    regenBtn.setAttribute("class", "btn btn-secondary invite-regenerate-btn");
    regenBtn.setAttribute("data-invite-regenerate-btn", "true");
    regenBtn.textContent = "Сгенерировать новую ссылку";
    regenBtn.addEventListener("click", () => {
      if (typeof onRegenerateInvite === "function") {
        const result = onRegenerateInvite({
          maxAge: settings.maxAge,
          maxUses: settings.maxUses,
          temporary: settings.temporary,
        });
        if (typeof result === "string" && result.trim()) {
          inviteCode = result.trim();
          if (urlInputEl) urlInputEl.value = formatInviteUrl(inviteCode);
          if (copyBtnEl) copyBtnEl.textContent = "Копировать";
        } else if (result && typeof result === "object" && typeof result.code === "string") {
          inviteCode = result.code.trim();
          if (urlInputEl) urlInputEl.value = formatInviteUrl(inviteCode);
          if (copyBtnEl) copyBtnEl.textContent = "Копировать";
        }
      }
    });
    settingsSection.appendChild(regenBtn);
    dialog.appendChild(settingsSection);

    replaceControllerChildren(container, [dialog]);
    renderFriendRows();
  }

  return {
    /**
     * Открывает модалку приглашения и рендерит её содержимое.
     * @param {OpenInviteModalPayload} [payload]
     */
    open(payload = {}) {
      isOpen = true;
      channelName = typeof payload?.channelName === "string" ? payload.channelName : "";
      inviteCode = typeof payload?.inviteCode === "string" ? payload.inviteCode : "";
      searchQuery = "";
      invitedUserIds.clear();

      settings.maxAge = isValidMaxAgeOption(payload?.maxAge) ? payload.maxAge : 86400;
      settings.maxUses = isValidMaxUsesOption(payload?.maxUses) ? payload.maxUses : 0;
      settings.temporary = typeof payload?.temporary === "boolean" ? payload.temporary : false;

      const rawFriends = Array.isArray(payload?.friends) ? payload.friends : [];
      friends = rawFriends
        .filter((item) => item && typeof item === "object")
        .map((item) => {
          const id = String(item.id ?? item.userId ?? "").trim();
          const username = String(item.username ?? "").trim();
          const displayName = String(item.displayName ?? item.username ?? "").trim();
          return { id, username, displayName };
        })
        .filter((item) => item.id.length > 0);

      renderModal();
    },

    /**
     * Обновляет поисковый запрос для фильтрации списка друзей.
     * @param {string} q
     */
    setSearchQuery(q) {
      searchQuery = typeof q === "string" ? q : "";
      if (searchInputEl) {
        searchInputEl.value = searchQuery;
      }
      if (isOpen) {
        renderFriendRows();
      }
    },

    /**
     * Обновляет настройки ссылки приглашения (maxAge, maxUses, temporary).
     * @param {Partial<InviteSettingsState>} [partial]
     * @returns {InviteSettingsState}
     */
    updateSettings(partial = {}) {
      if (partial && typeof partial === "object") {
        if (isValidMaxAgeOption(partial.maxAge)) {
          settings.maxAge = partial.maxAge;
        }
        if (isValidMaxUsesOption(partial.maxUses)) {
          settings.maxUses = partial.maxUses;
        }
        if (typeof partial.temporary === "boolean") {
          settings.temporary = partial.temporary;
        }
      }
      if (isOpen) {
        syncSettingsDom();
      }
      return { ...settings };
    },

    /**
     * Закрывает модальное окно и очищает контейнер.
     */
    close() {
      isOpen = false;
      friendListHostEl = null;
      searchInputEl = null;
      urlInputEl = null;
      copyBtnEl = null;
      maxAgeSelectEl = null;
      maxUsesSelectEl = null;
      tempCheckboxEl = null;
      replaceControllerChildren(container, []);
    },

    /**
     * Возвращает снимок текущего состояния контроллера.
     */
    getState() {
      return {
        isOpen,
        channelName,
        inviteCode,
        inviteUrl: formatInviteUrl(inviteCode),
        searchQuery,
        invitedUserIds: Array.from(invitedUserIds),
        settings: { ...settings },
      };
    },
  };
}
