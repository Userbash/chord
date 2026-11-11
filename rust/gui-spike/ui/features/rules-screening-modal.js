// @ts-check
/**
 * TASK-811: Баннер и модальное окно экрана правил сервера (Rules Screening Banner & Modal).
 *
 * Назначение:
 * - Если участник гильдии находится в состоянии `pending: true` (не прошёл верификацию правил),
 *   внизу экрана чата отображается плашка Discord:
 *   «Вам необходимо выполнить несколько шагов, прежде чем вы сможете общаться» с кнопкой «Выполнить».
 * - Нажатие кнопки «Выполнить» открывает модальное окно `#rules-screening-modal` с текстом правил,
 *   чекбоксом подтверждения согласия и кнопками «Отмена» и «Отправить».
 * - Нажатие кнопки «Отправить» передаёт IPC `confirm_member_rules` с параметром `{ guildId }`,
 *   переводя участника в состояние `pending: false` и разблокируя поле ввода чата.
 *
 * Дизайн-токены:
 *   background: #2b2d31, surface: #313338, text_primary: #ffffff, accent: #23a55a
 *
 * @module ui/features/rules-screening-modal
 */

/** Дизайн-токены Discord по спецификации TASK-811. */
export const RULES_SCREENING_TOKENS = Object.freeze({
  background: "#2b2d31",
  surface: "#313338",
  textPrimary: "#ffffff",
  accent: "#23a55a",
});

/** IPC-команда подтверждения правил сервера для перехода участника из pending-состояния. */
export const CONFIRM_MEMBER_RULES_IPC_CMD = "confirm_member_rules";

/** Текстовые константы экрана правил. */
export const RULES_SCREENING_TEXT = Object.freeze({
  bannerNotice: "Вам необходимо выполнить несколько шагов, прежде чем вы сможете общаться",
  bannerButton: "Выполнить",
  modalTitle: "Правила сервера",
  modalSubtitle: "Прежде чем вы сможете общаться на этом сервере, вам необходимо ознакомиться с правилами и согласиться с ними.",
  checkboxLabel: "Я прочитал(-а) и согласен(-на) с правилами",
  submitButton: "Отправить",
  submittingButton: "Отправка…",
  cancelButton: "Отмена",
  closeLabel: "Закрыть окно правил",
  successNotice: "Правила сервера приняты! Теперь вы можете общаться.",
  errorNotice: "Не удалось подтвердить согласие с правилами сервера. Попробуйте ещё раз.",
});

/** Стандартные правила сообщества по умолчанию, если сервер не задал кастомные правила. */
export const DEFAULT_COMMUNITY_RULES = Object.freeze([
  "Относитесь ко всем участникам с уважением и вежливостью.",
  "Запрещены спам, реклама, вредоносные ссылки и флуд в любых каналах.",
  "Соблюдайте Правила сообщества Discord и условия использования платформы.",
  "Публикуйте сообщения и медиа только в соответствующих по тематике каналах.",
  "Следуйте указаниям модераторов и администрации сервера.",
]);

/**
 * Проверяет, требуется ли участнику пройти проверку правил на сервере.
 * @param {any} guild Область данных гильдии.
 * @returns {boolean} true, если гильдия находится в состоянии pending screening.
 */
export function isRulesScreeningRequired(guild) {
  if (!guild || typeof guild !== "object") return false;
  // Режим предпросмотра (гость/lurker) обрабатывается отдельным баннером
  if (guild.lurker === true || guild.isMember === false || guild.is_member === false) {
    return false;
  }
  return Boolean(guild.pending === true || guild.is_pending === true);
}

/**
 * Извлекает текстовый список правил из данных гильдии или возвращает дефолтный набор.
 * @param {any} guild Область данных гильдии.
 * @returns {ReadonlyArray<string>} Массив строк с правилами.
 */
export function extractRulesFromGuild(guild) {
  if (!guild || typeof guild !== "object") {
    return DEFAULT_COMMUNITY_RULES;
  }
  // 1. Поле rules: string[]
  if (Array.isArray(guild.rules) && guild.rules.length > 0) {
    return guild.rules.map((/** @type {any} */ r) => String(r?.text ?? r?.description ?? r)).filter(Boolean);
  }
  // 2. Поле verification_form / form_fields (тип TERMS)
  const formFields = Array.isArray(guild.form_fields)
    ? guild.form_fields
    : Array.isArray(guild.verification_form?.form_fields)
      ? guild.verification_form.form_fields
      : null;

  if (Array.isArray(formFields)) {
    for (const field of formFields) {
      if (
        (field?.field_type === "TERMS" || field?.kind === "TERMS" || field?.type === "TERMS") &&
        Array.isArray(field.values) &&
        field.values.length > 0
      ) {
        return field.values.map(String).filter(Boolean);
      }
    }
  }

  return DEFAULT_COMMUNITY_RULES;
}

/**
 * Параметры рендеринга баннера экрана правил.
 * @typedef {{
 *   document?: Document,
 *   onOpenModal: () => void,
 * }} RenderBannerOptions
 */

/**
 * Рендерит плашку уведомления о необходимости прохождения правил над полем ввода.
 * @param {HTMLElement} container Родительский контейнер (куда вставить или привязать баннер).
 * @param {RenderBannerOptions} options Опции баннера.
 * @returns {HTMLElement} Созданный или обновлённый элемент баннера.
 */
export function renderRulesScreeningBanner(container, options) {
  const doc = options.document || container.ownerDocument || globalThis.document;
  let banner = /** @type {HTMLElement | null} */ (container.querySelector("#guild-pending-screening-banner"));
  if (!banner) {
    banner = doc.createElement("div");
    banner.id = "guild-pending-screening-banner";
    banner.className = "rules-screening-banner";
    banner.setAttribute("role", "region");
    banner.setAttribute("aria-label", "Требуется подтверждение правил");
    banner.setAttribute("data-access-banner", "pending");
    container.appendChild(banner);
  } else {
    banner.replaceChildren();
  }

  const textSpan = doc.createElement("span");
  textSpan.className = "rules-screening-banner-text";
  textSpan.textContent = RULES_SCREENING_TEXT.bannerNotice;

  const completeBtn = doc.createElement("button");
  completeBtn.type = "button";
  completeBtn.className = "rules-screening-complete-btn";
  completeBtn.setAttribute("data-action", "open-rules-modal");
  completeBtn.textContent = RULES_SCREENING_TEXT.bannerButton;
  completeBtn.addEventListener("click", () => {
    options.onOpenModal();
  });

  banner.append(textSpan, completeBtn);
  return banner;
}

/**
 * Опции для контроллера модального окна экрана правил.
 * @typedef {{
 *   document?: Document,
 *   invoke: (cmd: string, args?: Record<string, unknown>) => Promise<any>,
 *   onSuccess?: (guildId: string) => void,
 *   onError?: (error: unknown) => void,
 *   onClose?: () => void,
 * }} RulesScreeningControllerOptions
 */

/**
 * Параметры открытия модального окна правил.
 * @typedef {{
 *   guildId: string,
 *   guildName?: string,
 *   rules?: ReadonlyArray<string>,
 * }} OpenRulesModalParams
 */

/**
 * Создаёт контроллер модального окна экрана правил сервера (#rules-screening-modal).
 * @param {RulesScreeningControllerOptions} options Опции контроллера.
 */
export function createRulesScreeningModalController(options) {
  const doc = options.document || globalThis.document;
  if (!doc) {
    throw new Error("createRulesScreeningModalController: document недоступен");
  }

  let currentGuildId = "";
  let currentGuildName = "";
  /** @type {ReadonlyArray<string>} */
  let currentRules = DEFAULT_COMMUNITY_RULES;
  let isSubmitting = false;

  let modalEl = /** @type {HTMLElement | null} */ (doc.getElementById("rules-screening-modal"));
  if (!modalEl) {
    modalEl = doc.createElement("div");
    modalEl.id = "rules-screening-modal";
    modalEl.className = "rules-modal-overlay";
    modalEl.setAttribute("role", "dialog");
    modalEl.setAttribute("aria-modal", "true");
    modalEl.setAttribute("aria-labelledby", "rules-modal-title");
    modalEl.setAttribute("hidden", "");
    doc.body.appendChild(modalEl);
  }

  /**
   * Обработчик нажатия клавиши Escape для закрытия модалки.
   * @param {KeyboardEvent} event
   */
  const handleKeydown = (event) => {
    if (event.key === "Escape" && isOpen()) {
      event.preventDefault();
      close();
    }
  };

  doc.addEventListener("keydown", handleKeydown);

  function isOpen() {
    return Boolean(modalEl && !modalEl.hasAttribute("hidden") && modalEl.style.display !== "none");
  }

  /**
   * Рендерит внутреннее содержимое карточки модального окна.
   */
  function renderContent() {
    if (!modalEl) return;
    modalEl.replaceChildren();

    const card = doc.createElement("div");
    card.className = "rules-modal-card";

    // --- Header ---
    const header = doc.createElement("header");
    header.className = "rules-modal-header";

    const titleGroup = doc.createElement("div");
    titleGroup.className = "rules-modal-title-group";

    const title = doc.createElement("h2");
    title.id = "rules-modal-title";
    title.className = "rules-modal-title";
    title.textContent = currentGuildName
      ? `${RULES_SCREENING_TEXT.modalTitle} · ${currentGuildName}`
      : RULES_SCREENING_TEXT.modalTitle;

    const subtitle = doc.createElement("p");
    subtitle.className = "rules-modal-subtitle";
    subtitle.textContent = RULES_SCREENING_TEXT.modalSubtitle;

    titleGroup.append(title, subtitle);

    const closeBtn = doc.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "rules-modal-close-btn";
    closeBtn.setAttribute("data-action", "close");
    closeBtn.setAttribute("aria-label", RULES_SCREENING_TEXT.closeLabel);
    closeBtn.textContent = "✕";
    closeBtn.addEventListener("click", () => close());

    header.append(titleGroup, closeBtn);

    // --- Body ---
    const body = doc.createElement("div");
    body.className = "rules-modal-body";

    const rulesContainer = doc.createElement("div");
    rulesContainer.className = "rules-modal-rules-container";
    rulesContainer.setAttribute("role", "list");

    currentRules.forEach((ruleText, idx) => {
      const item = doc.createElement("div");
      item.className = "rules-modal-rule-item";
      item.setAttribute("role", "listitem");

      const num = doc.createElement("span");
      num.className = "rules-modal-rule-index";
      num.textContent = `${idx + 1}.`;

      const txt = doc.createElement("span");
      txt.className = "rules-modal-rule-text";
      txt.textContent = ruleText;

      item.append(num, txt);
      rulesContainer.appendChild(item);
    });

    // Чекбокс согласия
    const agreementLabel = doc.createElement("label");
    agreementLabel.className = "rules-modal-agreement";

    const checkbox = /** @type {HTMLInputElement} */ (doc.createElement("input"));
    checkbox.type = "checkbox";
    checkbox.className = "rules-modal-checkbox";
    checkbox.id = "rules-agreement-checkbox";

    const agreementText = doc.createElement("span");
    agreementText.textContent = RULES_SCREENING_TEXT.checkboxLabel;

    agreementLabel.append(checkbox, agreementText);

    body.append(rulesContainer, agreementLabel);

    // Ошибки
    const errorBox = doc.createElement("div");
    errorBox.className = "rules-modal-error";
    errorBox.setAttribute("role", "alert");
    errorBox.style.display = "none";
    body.appendChild(errorBox);

    // --- Footer ---
    const footer = doc.createElement("footer");
    footer.className = "rules-modal-footer";

    const cancelBtn = doc.createElement("button");
    cancelBtn.type = "button";
    cancelBtn.className = "rules-modal-cancel-btn";
    cancelBtn.setAttribute("data-action", "cancel");
    cancelBtn.textContent = RULES_SCREENING_TEXT.cancelButton;
    cancelBtn.addEventListener("click", () => close());

    const submitBtn = /** @type {HTMLButtonElement} */ (doc.createElement("button"));
    submitBtn.type = "button";
    submitBtn.className = "rules-modal-submit-btn";
    submitBtn.setAttribute("data-action", "submit");
    submitBtn.textContent = RULES_SCREENING_TEXT.submitButton;
    submitBtn.disabled = true;

    // Взаимодействие чекбокса и кнопки «Отправить»
    checkbox.addEventListener("change", () => {
      if (!isSubmitting) {
        submitBtn.disabled = !checkbox.checked;
      }
    });

    submitBtn.addEventListener("click", () => {
      void submit();
    });

    footer.append(cancelBtn, submitBtn);

    card.append(header, body, footer);
    modalEl.appendChild(card);

    // Клик по подложке вне карточки закрывает модалку
    modalEl.onclick = (e) => {
      if (e.target === modalEl) {
        close();
      }
    };
  }

  /**
   * Открывает модальное окно правил для указанной гильдии.
   * @param {OpenRulesModalParams} params
   */
  function open(params) {
    currentGuildId = String(params.guildId || "").trim();
    currentGuildName = String(params.guildName || "").trim();
    currentRules = Array.isArray(params.rules) && params.rules.length > 0
      ? params.rules
      : DEFAULT_COMMUNITY_RULES;
    isSubmitting = false;

    renderContent();
    if (modalEl) {
      modalEl.removeAttribute("hidden");
      modalEl.style.display = "flex";
      // Фокусируемся на первом интерактивном элементе
      const checkbox = /** @type {any} */ (modalEl.querySelector('input[type="checkbox"]'));
      if (checkbox && typeof checkbox.focus === "function") {
        checkbox.focus();
      }
    }
  }

  /**
   * Закрывает модальное окно правил.
   */
  function close() {
    if (isSubmitting) return; // Не прерывать сетевой запрос
    if (modalEl) {
      modalEl.setAttribute("hidden", "");
      modalEl.style.display = "none";
    }
    options.onClose?.();
  }

  /**
   * Подтверждает согласие с правилами и отправляет IPC confirm_member_rules.
   * @returns {Promise<any>}
   */
  async function submit() {
    if (isSubmitting || !modalEl) return;
    const checkbox = /** @type {HTMLInputElement | null} */ (modalEl.querySelector('input[type="checkbox"]'));
    if (!checkbox || !checkbox.checked) return;

    const submitBtn = /** @type {HTMLButtonElement | null} */ (modalEl.querySelector('[data-action="submit"]'));
    const errorBox = /** @type {HTMLElement | null} */ (modalEl.querySelector(".rules-modal-error"));

    isSubmitting = true;
    if (submitBtn) {
      submitBtn.disabled = true;
      submitBtn.textContent = RULES_SCREENING_TEXT.submittingButton;
    }
    if (errorBox) {
      errorBox.style.display = "none";
      errorBox.textContent = "";
    }

    try {
      const result = await options.invoke(CONFIRM_MEMBER_RULES_IPC_CMD, {
        guildId: currentGuildId,
      });

      isSubmitting = false;
      close();
      options.onSuccess?.(currentGuildId);
      return result;
    } catch (err) {
      isSubmitting = false;
      if (submitBtn) {
        submitBtn.disabled = false;
        submitBtn.textContent = RULES_SCREENING_TEXT.submitButton;
      }
      if (errorBox) {
        errorBox.style.display = "block";
        errorBox.textContent = RULES_SCREENING_TEXT.errorNotice;
      }
      options.onError?.(err);
    }
  }

  return {
    open,
    close,
    isOpen,
    submit,
    getModalElement: () => modalEl,
    destroy: () => {
      doc.removeEventListener("keydown", handleKeydown);
      modalEl?.remove();
    },
  };
}
