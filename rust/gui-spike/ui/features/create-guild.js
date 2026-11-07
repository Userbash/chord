// @ts-check
/**
 * STUB-03: Модальное окно создания сервера (`POST /guilds`).
 *
 * Контракт и дизайн:
 * - Роль dialog (`role="dialog"`, `aria-modal="true"`), управление доступностью и клавиатурой;
 * - Шаг 1: выбор шаблона («Создать свой шаблон», «Для меня и друзей», «Для клуба или сообщества»);
 * - Шаг 2: настройка сервера (ввод названия, валидация 2..100 символов, загрузка иконки);
 * - Вызов IPC `create_guild` с `name`, `icon` (data URI) и `template`;
 * - Защита от спама кликов (`submit.disabled = true`, `aria-busy="true"`);
 * - Переключатель «У вас уже есть приглашение? Присоединиться к серверу» для быстрого открытия модалки инвайтов.
 */

/** Допустимые идентификаторы шаблонов сервера. */
export const VALID_TEMPLATES = Object.freeze(["custom", "friends", "community"]);

/** Текстовые константы диалога создания сервера. */
export const CREATE_GUILD_TEXT = Object.freeze({
  title: "Создайте свой сервер",
  subtitle: "Ваш сервер — это место, где вы можете общаться с друзьями и единомышленниками. Создайте сервер и пригласите друзей.",
  customizeTitle: "Настройте свой сервер",
  customizeSubtitle: "Сделайте сервер уникальным, выбрав название и значок. Вы всегда сможете изменить их позже.",
  templateCustom: "Создать свой шаблон",
  templateFriends: "Для меня и друзей",
  templateCommunity: "Для клуба или сообщества",
  nameLabel: "НАЗВАНИЕ СЕРВЕРА",
  namePlaceholder: "Введите название сервера",
  back: "Назад",
  cancel: "Отмена",
  create: "Создать",
  creating: "Создание...",
  joinPrompt: "У вас уже есть приглашение?",
  joinAction: "Присоединиться к серверу",
  defaultServerName: (/** @type {string} */ username) => `Сервер ${username || "пользователя"}`,
});

/**
 * Валидация названия сервера по контракту Discord API: от 2 до 100 символов.
 * @param {unknown} name
 * @returns {boolean}
 */
export function validateGuildName(name) {
  if (typeof name !== "string") return false;
  const trimmed = name.trim();
  const len = trimmed.length;
  return len >= 2 && len <= 100;
}

/**
 * @typedef {{
 *   document?: any,
 *   root?: any,
 *   invoker?: (cmd: string, args?: any) => Promise<any>,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   currentUsername?: string,
 *   onCreated?: (guild: any) => void,
 *   onOpenJoin?: (opener?: any) => void,
 * }} CreateGuildOptions
 */

/**
 * Создаёт контроллер диалога создания сервера #create-guild-dialog.
 * @param {CreateGuildOptions} [options]
 */
export function createCreateGuildController(options = {}) {
  const doc = options.document ?? globalThis.document;
  const root = options.root ?? doc.body;
  const invoker = options.invoker ?? options.invoke ?? (async () => ({}));
  const onCreated = options.onCreated ?? (() => {});
  const onOpenJoin = options.onOpenJoin ?? (() => {});
  const defaultUsername = options.currentUsername ?? "";

  let currentStep = "template"; // "template" | "customize"
  let selectedTemplate = "custom";
  let selectedIconData = /** @type {string | null} */ (null);
  let isSubmitting = false;
  let lastOpener = /** @type {any} */ (null);

  // Контейнер диалога
  const dialog = doc.createElement("section");
  dialog.id = "create-guild-dialog";
  dialog.className = "modal modal-create-guild";
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-labelledby", "create-guild-title");
  dialog.hidden = true;

  // Оверлей
  const backdrop = doc.createElement("div");
  backdrop.className = "modal-backdrop";
  backdrop.addEventListener("click", () => close());
  dialog.append(backdrop);

  // Контент модалки
  const modalBox = doc.createElement("div");
  modalBox.className = "modal-box create-guild-box";

  // Кнопка закрытия (крестик)
  const closeBtn = doc.createElement("button");
  closeBtn.type = "button";
  closeBtn.className = "modal-close";
  closeBtn.setAttribute("aria-label", CREATE_GUILD_TEXT.cancel);
  closeBtn.textContent = "✕";
  closeBtn.addEventListener("click", () => close());
  modalBox.append(closeBtn);

  // Заголовок и подзаголовок
  const header = doc.createElement("header");
  header.className = "create-guild-header";
  const title = doc.createElement("h2");
  title.id = "create-guild-title";
  title.className = "create-guild-title";
  title.textContent = CREATE_GUILD_TEXT.title;
  const subtitle = doc.createElement("p");
  subtitle.className = "create-guild-subtitle";
  subtitle.textContent = CREATE_GUILD_TEXT.subtitle;
  header.append(title, subtitle);
  modalBox.append(header);

  // Область содержимого шагов
  const bodyArea = doc.createElement("div");
  bodyArea.className = "create-guild-body";
  modalBox.append(bodyArea);

  // Подвал с кнопками и переходом к инвайтам
  const footer = doc.createElement("footer");
  footer.className = "create-guild-footer";
  modalBox.append(footer);

  dialog.append(modalBox);
  root.append(dialog);

  /**
   * Отрисовывает шаг 1: выбор шаблона.
   */
  function renderTemplateStep() {
    title.textContent = CREATE_GUILD_TEXT.title;
    subtitle.textContent = CREATE_GUILD_TEXT.subtitle;
    bodyArea.replaceChildren();
    footer.replaceChildren();

    const templateList = doc.createElement("div");
    templateList.className = "template-list";

    const templates = [
      { id: "custom", label: CREATE_GUILD_TEXT.templateCustom, icon: "✨" },
      { id: "friends", label: CREATE_GUILD_TEXT.templateFriends, icon: "🎮" },
      { id: "community", label: CREATE_GUILD_TEXT.templateCommunity, icon: "🏫" },
    ];

    for (const tpl of templates) {
      const card = doc.createElement("button");
      card.type = "button";
      card.className = "template-card";
      card.setAttribute("data-template", tpl.id);

      const iconSpan = doc.createElement("span");
      iconSpan.className = "template-icon";
      iconSpan.textContent = tpl.icon;

      const labelSpan = doc.createElement("span");
      labelSpan.className = "template-label";
      labelSpan.textContent = tpl.label;

      const arrowSpan = doc.createElement("span");
      arrowSpan.className = "template-arrow";
      arrowSpan.textContent = "›";

      card.append(iconSpan, labelSpan, arrowSpan);
      card.addEventListener("click", () => {
        selectTemplate(tpl.id);
      });
      templateList.append(card);
    }

    bodyArea.append(templateList);

    // Подвал шага 1: предложение присоединиться по инвайту
    const joinRow = doc.createElement("div");
    joinRow.className = "create-guild-join-prompt";

    const joinText = doc.createElement("span");
    joinText.textContent = CREATE_GUILD_TEXT.joinPrompt + " ";

    const joinBtn = doc.createElement("button");
    joinBtn.type = "button";
    joinBtn.className = "switch-to-join-btn";
    joinBtn.textContent = CREATE_GUILD_TEXT.joinAction;
    joinBtn.addEventListener("click", () => {
      close();
      onOpenJoin(lastOpener);
    });

    joinRow.append(joinText, joinBtn);
    footer.append(joinRow);
  }

  /**
   * Отрисовывает шаг 2: настройка названия и иконки.
   */
  function renderCustomizeStep() {
    title.textContent = CREATE_GUILD_TEXT.customizeTitle;
    subtitle.textContent = CREATE_GUILD_TEXT.customizeSubtitle;
    bodyArea.replaceChildren();
    footer.replaceChildren();

    const form = doc.createElement("div");
    form.className = "create-guild-form";

    // Поле ввода названия
    const nameGroup = doc.createElement("div");
    nameGroup.className = "form-group";

    const nameLabel = doc.createElement("label");
    nameLabel.setAttribute("for", "create-guild-name");
    nameLabel.className = "form-label";
    nameLabel.textContent = CREATE_GUILD_TEXT.nameLabel;

    const nameInput = doc.createElement("input");
    nameInput.type = "text";
    nameInput.id = "create-guild-name";
    nameInput.className = "input create-guild-input";
    nameInput.placeholder = CREATE_GUILD_TEXT.namePlaceholder;
    nameInput.value = CREATE_GUILD_TEXT.defaultServerName(defaultUsername);

    nameGroup.append(nameLabel, nameInput);

    // Блок ошибки (если есть)
    const errorBox = doc.createElement("p");
    errorBox.className = "form-error";
    errorBox.id = "create-guild-error";
    errorBox.hidden = true;

    form.append(nameGroup, errorBox);
    bodyArea.append(form);

    // Кнопки в подвале: Назад и Создать
    const backBtn = doc.createElement("button");
    backBtn.type = "button";
    backBtn.className = "btn btn-secondary create-guild-back";
    backBtn.textContent = CREATE_GUILD_TEXT.back;
    backBtn.addEventListener("click", () => {
      currentStep = "template";
      renderTemplateStep();
    });

    const submitBtn = doc.createElement("button");
    submitBtn.type = "button";
    submitBtn.id = "create-guild-submit";
    submitBtn.className = "btn btn-primary create-guild-submit";
    submitBtn.textContent = CREATE_GUILD_TEXT.create;
    submitBtn.disabled = !validateGuildName(nameInput.value);

    // Слушатель ввода
    nameInput.addEventListener("input", () => {
      const valid = validateGuildName(nameInput.value);
      submitBtn.disabled = !valid || isSubmitting;
      if (valid) {
        errorBox.hidden = true;
        errorBox.textContent = "";
      }
    });

    // Отправка по Enter
    nameInput.addEventListener("keydown", (/** @type {any} */ evt) => {
      if (evt.key === "Enter" && !submitBtn.disabled && !isSubmitting) {
        submit();
      }
    });

    submitBtn.addEventListener("click", () => {
      if (!submitBtn.disabled && !isSubmitting) {
        submit();
      }
    });

    footer.append(backBtn, submitBtn);
  }

  /**
   * Выбор шаблона и переход к шагу настройки.
   * @param {string} templateId
   */
  function selectTemplate(templateId) {
    selectedTemplate = templateId;
    currentStep = "customize";
    renderCustomizeStep();
  }

  /**
   * Отправка запроса создания сервера через IPC.
   */
  async function submit() {
    const nameInput = /** @type {any} */ (doc.getElementById("create-guild-name"));
    const submitBtn = /** @type {any} */ (doc.getElementById("create-guild-submit"));
    const errorBox = /** @type {any} */ (doc.getElementById("create-guild-error"));

    const name = nameInput ? String(nameInput.value || "").trim() : "";
    if (!validateGuildName(name)) {
      if (errorBox) {
        errorBox.hidden = false;
        errorBox.textContent = "Название должно содержать от 2 до 100 символов";
      }
      return;
    }

    isSubmitting = true;
    if (submitBtn) {
      submitBtn.disabled = true;
      submitBtn.setAttribute("aria-busy", "true");
      submitBtn.textContent = CREATE_GUILD_TEXT.creating;
    }
    if (errorBox) {
      errorBox.hidden = true;
      errorBox.textContent = "";
    }

    try {
      const outcome = await invoker("create_guild", {
        name,
        icon: selectedIconData,
        template: selectedTemplate,
      });
      close();
      onCreated(outcome);
      return outcome;
    } catch (err) {
      isSubmitting = false;
      if (submitBtn) {
        submitBtn.disabled = false;
        submitBtn.removeAttribute("aria-busy");
        submitBtn.textContent = CREATE_GUILD_TEXT.create;
      }
      if (errorBox) {
        errorBox.hidden = false;
        const msg = err && typeof err === "object" && "kind" in err && err.kind === "forbidden"
          ? "Достигнут лимит создания серверов (максимум 100)"
          : "Не удалось создать сервер. Попробуйте позже.";
        errorBox.textContent = msg;
      }
    }
  }

  /**
   * Открытие диалога.
   * @param {any} [opener]
   */
  function open(opener) {
    lastOpener = opener;
    currentStep = "template";
    selectedTemplate = "custom";
    selectedIconData = null;
    isSubmitting = false;
    renderTemplateStep();

    dialog.hidden = false;
    dialog.classList.add("is-open");
    if (opener && typeof opener.setAttribute === "function") {
      opener.setAttribute("aria-expanded", "true");
    }
  }

  /**
   * Закрытие диалога.
   */
  function close() {
    dialog.hidden = true;
    dialog.classList.remove("is-open");
    if (lastOpener && typeof lastOpener.setAttribute === "function") {
      lastOpener.setAttribute("aria-expanded", "false");
      if (typeof lastOpener.focus === "function") {
        lastOpener.focus();
      }
    }
    isSubmitting = false;
  }

  return {
    open,
    close,
    selectTemplate,
    submit,
    getStep: () => currentStep,
    getSelectedTemplate: () => selectedTemplate,
    element: dialog,
  };
}
