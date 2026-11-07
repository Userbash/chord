// @ts-check
/**
 * TASK-612 / STUB-03: Визард шаблонов создания сервера (`POST /guilds`, `#create-guild-dialog`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` §5.5 (`CreateGuildModal`: `role="dialog"`,
 *   `aria-modal="true"`, `aria-labelledby`, focus trap модуль `617986`, `--modal-width-small: 442px`);
 * - `/home/sanya/re-deep/decoded/a72f8c0298aeb52e_0` (модуль `819638`: шаги `guild-templates` / `customize-guild`);
 * - `/home/sanya/re-deep/decoded/9164065a2404f2fe_0` и `/home/sanya/re-deep/ru-catalog.json`:
 *   шаблоны `'Свой шаблон'`, `'Для меня и друзей'` (Gaming), `'Учебная группа'` (Study), `'Клуб или сообщество'`;
 * - `/home/sanya/re-deep/decoded/496925fc1facf986_0` (модуль `356343`: загрузка значка base64 DataURI);
 * - `rust/gui-spike/src/guilds.rs`: валидация названия `1..=100` символов после `trim`.
 */

/** Минимальная длина названия сервера после обрезки пробелов. */
export const MIN_GUILD_NAME_CHARS = 1;

/** Максимальная длина названия сервера после обрезки пробелов. */
export const MAX_GUILD_NAME_CHARS = 100;

/**
 * Описание шаблона создания сервера.
 * @typedef {{
 *   id: string,
 *   key: string,
 *   category: string,
 *   code: string | null,
 *   label: string,
 *   icon: string,
 *   aliases: readonly string[],
 * }} GuildTemplateSpec
 */

/** Справочник 4 шаблонов визарда создания сервера по RE-оракулу Discord. */
export const CREATE_GUILD_TEMPLATES = Object.freeze([
  Object.freeze({
    id: "custom",
    key: "CREATE",
    category: "custom",
    code: null,
    label: "Свой шаблон",
    icon: "✨",
    aliases: Object.freeze(["custom", "create", "Свой шаблон", "Создать свой шаблон"]),
  }),
  Object.freeze({
    id: "friends",
    key: "GAMING",
    category: "gaming",
    code: "hgM48av5Q69A",
    label: "Для меня и друзей",
    icon: "🎮",
    aliases: Object.freeze(["friends", "gaming", "Для меня и друзей"]),
  }),
  Object.freeze({
    id: "study",
    key: "STUDY",
    category: "study",
    code: "2JBhzzca2vfT",
    label: "Учебная группа",
    icon: "📚",
    aliases: Object.freeze(["study", "study_group", "Учебная группа"]),
  }),
  Object.freeze({
    id: "community",
    key: "COMMUNITY",
    category: "community",
    code: "64UDvRNCC52Y",
    label: "Клуб или сообщество",
    icon: "🏫",
    aliases: Object.freeze(["community", "club", "Клуб или сообщество", "Для клуба или сообщества"]),
  }),
]);

/** Допустимые идентификаторы шаблонов сервера. */
export const VALID_TEMPLATES = Object.freeze(["custom", "friends", "gaming", "study", "community"]);

/** Текстовые константы диалога создания сервера. */
export const CREATE_GUILD_TEXT = Object.freeze({
  title: "Создайте свой сервер",
  subtitle:
    "Ваш сервер — это место, где вы можете общаться с друзьями и единомышленниками. Создайте сервер и пригласите друзей.",
  customizeTitle: "Настройте свой сервер",
  customizeSubtitle:
    "Сделайте сервер уникальным, выбрав название и значок. Вы всегда сможете изменить их позже.",
  templateCustom: "Свой шаблон",
  templateFriends: "Для меня и друзей",
  templateGaming: "Для меня и друзей",
  templateStudy: "Учебная группа",
  templateCommunity: "Клуб или сообщество",
  uploadIconLabel: "Загрузить значок сервера",
  uploadIconBadge: "Загрузить",
  clearIconLabel: "Удалить значок",
  iconPreviewAlt: "Ваш значок",
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

const ICON_DATA_URI_RE =
  /^data:image\/(?:png|jpeg|jpg|gif|webp);base64,([A-Za-z0-9+/]+={0,2})$/;

/**
 * Обрезает пробельные символы по краям названия сервера.
 * @param {unknown} name
 * @returns {string}
 */
export function normalizeGuildName(name) {
  if (typeof name !== "string") return "";
  return name.trim();
}

/**
 * Валидация названия сервера по контракту Discord API: от 1 до 100 символов после обрезки пробелов.
 * @param {unknown} name
 * @returns {boolean}
 */
export function validateGuildName(name) {
  if (typeof name !== "string") return false;
  const trimmed = name.trim();
  const len = Array.from(trimmed).length;
  return len >= MIN_GUILD_NAME_CHARS && len <= MAX_GUILD_NAME_CHARS;
}

/**
 * Проверяет корректность base64 DataURI иконки сервера (`data:image/(png|jpeg|jpg|gif|webp);base64,...`).
 * Пустое значение (`null`, `undefined`, `""`) считается валидным (иконка опциональна).
 * @param {unknown} dataUri
 * @returns {boolean}
 */
export function validateGuildIconDataUri(dataUri) {
  if (dataUri === null || dataUri === undefined || dataUri === "") return true;
  if (typeof dataUri !== "string") return false;
  const trimmed = dataUri.trim();
  if (trimmed === "") return false;
  const match = ICON_DATA_URI_RE.exec(trimmed);
  if (!match) return false;
  const payload = match[1] ?? "";
  return payload.length >= 4 && payload.length % 4 === 0;
}

/**
 * Находит спецификацию шаблона по его идентификатору, категории или русской подписи.
 * @param {unknown} templateIdOrLabel
 * @returns {GuildTemplateSpec | null}
 */
export function resolveGuildTemplate(templateIdOrLabel) {
  if (typeof templateIdOrLabel !== "string") return null;
  const raw = templateIdOrLabel.trim();
  if (!raw) return null;
  const lower = raw.toLowerCase();
  for (const tpl of CREATE_GUILD_TEMPLATES) {
    if (
      tpl.id === lower ||
      tpl.category === lower ||
      tpl.key.toLowerCase() === lower ||
      tpl.label.toLowerCase() === lower ||
      tpl.aliases.some((a) => a.toLowerCase() === lower)
    ) {
      return tpl;
    }
  }
  return null;
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
 * Создаёт контроллер диалога визарда создания сервера `#create-guild-dialog`.
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
  let activeTemplateIndex = 0;
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
  dialog.setAttribute("hidden", "");

  // Оверлей
  const backdrop = doc.createElement("div");
  backdrop.className = "modal-backdrop";
  backdrop.addEventListener("click", () => {
    if (!isSubmitting) close();
  });
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
  closeBtn.addEventListener("click", () => {
    if (!isSubmitting) close();
  });
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
   * Возвращает список фокусируемых не-отключённых элементов внутри диалога для focus trap.
   * @returns {any[]}
   */
  function getFocusableElements() {
    const candidates = dialog.querySelectorAll("button, input");
    /** @type {any[]} */
    const focusable = [];
    for (const el of candidates) {
      if (!el || el.disabled || el.hidden || el.getAttribute("hidden") !== null) continue;
      if (el.getAttribute("tabindex") === "-1") continue;
      focusable.push(el);
    }
    return focusable;
  }

  // Клавиатурный обработчик диалога: Escape и Focus Trap (Tab / Shift+Tab)
  dialog.addEventListener("keydown", (/** @type {any} */ evt) => {
    if (dialog.hidden) return;
    if (evt.key === "Escape") {
      if (isSubmitting) return;
      if (typeof evt.preventDefault === "function") evt.preventDefault();
      if (typeof evt.stopPropagation === "function") evt.stopPropagation();
      close();
      return;
    }

    if (evt.key === "Tab") {
      const focusable = getFocusableElements();
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = doc.activeElement;

      if (evt.shiftKey) {
        if (active === first || !focusable.includes(active)) {
          if (typeof evt.preventDefault === "function") evt.preventDefault();
          if (first && last && typeof last.focus === "function") {
            last.focus();
          }
        }
      } else if (active === last || !focusable.includes(active)) {
        if (typeof evt.preventDefault === "function") evt.preventDefault();
        if (first && typeof first.focus === "function") {
          first.focus();
        }
      }
    }
  });

  /**
   * Обновляет roving tabindex на карточках шаблонов и переводит фокус.
   * @param {any[]} cards
   * @param {number} nextIndex
   * @param {boolean} [shouldFocus]
   */
  function setActiveTemplateCard(cards, nextIndex, shouldFocus = true) {
    if (cards.length === 0) return;
    const clamped = ((nextIndex % cards.length) + cards.length) % cards.length;
    activeTemplateIndex = clamped;
    for (let i = 0; i < cards.length; i += 1) {
      const card = cards[i];
      if (!card) continue;
      const isCurrent = i === clamped;
      card.setAttribute("tabindex", isCurrent ? "0" : "-1");
      card.setAttribute("aria-selected", isCurrent ? "true" : "false");
    }
    const target = cards[clamped];
    if (shouldFocus && target && typeof target.focus === "function") {
      target.focus();
    }
  }

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
    templateList.setAttribute("role", "listbox");
    templateList.setAttribute("aria-label", CREATE_GUILD_TEXT.title);

    /** @type {any[]} */
    const cards = [];

    for (let idx = 0; idx < CREATE_GUILD_TEMPLATES.length; idx += 1) {
      const tpl = CREATE_GUILD_TEMPLATES[idx];
      if (!tpl) continue;
      const card = doc.createElement("button");
      card.type = "button";
      card.className = "template-card";
      card.setAttribute("role", "option");
      card.setAttribute("data-template", tpl.id);
      card.setAttribute("data-template-category", tpl.category);
      card.setAttribute("tabindex", idx === activeTemplateIndex ? "0" : "-1");
      card.setAttribute("aria-selected", idx === activeTemplateIndex ? "true" : "false");

      const iconSpan = doc.createElement("span");
      iconSpan.className = "template-icon";
      iconSpan.setAttribute("aria-hidden", "true");
      iconSpan.textContent = tpl.icon;

      const labelSpan = doc.createElement("span");
      labelSpan.className = "template-label";
      labelSpan.textContent = tpl.label;

      const arrowSpan = doc.createElement("span");
      arrowSpan.className = "template-arrow";
      arrowSpan.setAttribute("aria-hidden", "true");
      arrowSpan.textContent = "›";

      card.append(iconSpan, labelSpan, arrowSpan);
      card.addEventListener("click", () => {
        activeTemplateIndex = idx;
        selectTemplate(tpl.id);
      });
      card.addEventListener("keydown", (/** @type {any} */ evt) => {
        if (evt.key === "ArrowDown" || evt.key === "ArrowRight") {
          if (typeof evt.preventDefault === "function") evt.preventDefault();
          setActiveTemplateCard(cards, idx + 1, true);
        } else if (evt.key === "ArrowUp" || evt.key === "ArrowLeft") {
          if (typeof evt.preventDefault === "function") evt.preventDefault();
          setActiveTemplateCard(cards, idx - 1, true);
        } else if (evt.key === "Home") {
          if (typeof evt.preventDefault === "function") evt.preventDefault();
          setActiveTemplateCard(cards, 0, true);
        } else if (evt.key === "End") {
          if (typeof evt.preventDefault === "function") evt.preventDefault();
          setActiveTemplateCard(cards, cards.length - 1, true);
        } else if (evt.key === "Enter" || evt.key === " ") {
          if (typeof evt.preventDefault === "function") evt.preventDefault();
          activeTemplateIndex = idx;
          selectTemplate(tpl.id);
        }
      });

      cards.push(card);
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
   * Обновляет визуальное превью иконки сервера на шаге 2.
   */
  function syncIconPreviewDom() {
    const iconPreview = /** @type {any} */ (doc.getElementById("create-guild-icon-preview"));
    const clearBtn = /** @type {any} */ (doc.getElementById("create-guild-icon-clear"));
    if (!iconPreview) return;

    iconPreview.replaceChildren();
    if (selectedIconData) {
      const img = doc.createElement("img");
      img.className = "create-guild-icon-img";
      img.setAttribute("src", selectedIconData);
      img.setAttribute("alt", CREATE_GUILD_TEXT.iconPreviewAlt);
      iconPreview.append(img);
      if (clearBtn) {
        clearBtn.hidden = false;
        clearBtn.removeAttribute("hidden");
      }
    } else {
      const placeholder = doc.createElement("span");
      placeholder.className = "create-guild-icon-placeholder";
      placeholder.textContent = CREATE_GUILD_TEXT.uploadIconBadge;
      iconPreview.append(placeholder);
      if (clearBtn) {
        clearBtn.hidden = true;
        clearBtn.setAttribute("hidden", "");
      }
    }
  }

  /**
   * Устанавливает или сбрасывает иконку сервера (base64 DataURI).
   * @param {string | null | undefined} dataUri
   * @returns {boolean}
   */
  function setIcon(dataUri) {
    const errorBox = /** @type {any} */ (doc.getElementById("create-guild-error"));
    if (dataUri === null || dataUri === undefined || dataUri === "") {
      selectedIconData = null;
      syncIconPreviewDom();
      return true;
    }
    if (!validateGuildIconDataUri(dataUri)) {
      if (errorBox) {
        errorBox.hidden = false;
        errorBox.removeAttribute("hidden");
        errorBox.textContent =
          "Некорректный формат значка (ожидается base64 DataURI PNG, JPEG, GIF или WebP)";
      }
      return false;
    }
    selectedIconData = dataUri.trim();
    if (errorBox) {
      errorBox.hidden = true;
      errorBox.setAttribute("hidden", "");
      errorBox.textContent = "";
    }
    syncIconPreviewDom();
    return true;
  }

  /**
   * Сбрасывает выбранную иконку сервера.
   */
  function clearIcon() {
    const iconInput = /** @type {any} */ (doc.getElementById("create-guild-icon-input"));
    if (iconInput) {
      iconInput.value = "";
    }
    setIcon(null);
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

    // Загрузчик иконки сервера (base64 DataURI)
    const uploaderGroup = doc.createElement("div");
    uploaderGroup.className = "create-guild-icon-uploader";

    const iconPreview = doc.createElement("div");
    iconPreview.id = "create-guild-icon-preview";
    iconPreview.className = "create-guild-icon-preview";

    const iconInput = doc.createElement("input");
    iconInput.type = "file";
    iconInput.id = "create-guild-icon-input";
    iconInput.className = "create-guild-icon-input";
    iconInput.setAttribute("accept", "image/png,image/jpeg,image/gif,image/webp");
    iconInput.setAttribute("aria-label", CREATE_GUILD_TEXT.uploadIconLabel);

    const clearBtn = doc.createElement("button");
    clearBtn.type = "button";
    clearBtn.id = "create-guild-icon-clear";
    clearBtn.className = "create-guild-icon-clear";
    clearBtn.textContent = CREATE_GUILD_TEXT.clearIconLabel;
    clearBtn.hidden = !selectedIconData;
    if (!selectedIconData) {
      clearBtn.setAttribute("hidden", "");
    }
    clearBtn.addEventListener("click", () => {
      clearIcon();
    });

    const handleIconInputChange = () => {
      const candidate =
        (iconInput.dataset && iconInput.dataset.dataUri) || iconInput.value || "";
      if (!candidate) {
        setIcon(null);
        return;
      }
      setIcon(candidate);
    };
    iconInput.addEventListener("change", handleIconInputChange);
    iconInput.addEventListener("input", handleIconInputChange);

    uploaderGroup.append(iconPreview, iconInput, clearBtn);

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
    nameInput.setAttribute("maxlength", String(MAX_GUILD_NAME_CHARS));
    nameInput.setAttribute("aria-invalid", "false");

    nameGroup.append(nameLabel, nameInput);

    // Блок ошибки (если есть)
    const errorBox = doc.createElement("p");
    errorBox.className = "form-error";
    errorBox.id = "create-guild-error";
    errorBox.setAttribute("role", "alert");
    errorBox.hidden = true;
    errorBox.setAttribute("hidden", "");

    form.append(uploaderGroup, nameGroup, errorBox);
    bodyArea.append(form);

    syncIconPreviewDom();

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
      nameInput.setAttribute("aria-invalid", valid ? "false" : "true");
      submitBtn.disabled = !valid || isSubmitting;
      if (valid) {
        errorBox.hidden = true;
        errorBox.setAttribute("hidden", "");
        errorBox.textContent = "";
      }
    });

    // Отправка по Enter
    nameInput.addEventListener("keydown", (/** @type {any} */ evt) => {
      if (evt.key === "Enter" && !submitBtn.disabled && !isSubmitting) {
        if (typeof evt.preventDefault === "function") evt.preventDefault();
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
    const resolved = resolveGuildTemplate(templateId);
    selectedTemplate = resolved ? resolved.id : templateId;
    currentStep = "customize";
    renderCustomizeStep();
  }

  /**
   * Отправка запроса создания сервера через IPC.
   */
  async function submit() {
    if (isSubmitting) return undefined;
    const nameInput = /** @type {any} */ (doc.getElementById("create-guild-name"));
    const submitBtn = /** @type {any} */ (doc.getElementById("create-guild-submit"));
    const errorBox = /** @type {any} */ (doc.getElementById("create-guild-error"));

    const rawName = nameInput ? String(nameInput.value ?? "") : "";
    const name = normalizeGuildName(rawName);
    if (!validateGuildName(name)) {
      if (nameInput) nameInput.setAttribute("aria-invalid", "true");
      if (errorBox) {
        errorBox.hidden = false;
        errorBox.removeAttribute("hidden");
        errorBox.textContent = "Название должно содержать от 1 до 100 символов";
      }
      return undefined;
    }

    if (!validateGuildIconDataUri(selectedIconData)) {
      if (errorBox) {
        errorBox.hidden = false;
        errorBox.removeAttribute("hidden");
        errorBox.textContent =
          "Некорректный формат значка (ожидается base64 DataURI PNG, JPEG, GIF или WebP)";
      }
      return undefined;
    }

    isSubmitting = true;
    if (submitBtn) {
      submitBtn.disabled = true;
      submitBtn.setAttribute("aria-busy", "true");
      submitBtn.textContent = CREATE_GUILD_TEXT.creating;
    }
    if (errorBox) {
      errorBox.hidden = true;
      errorBox.setAttribute("hidden", "");
      errorBox.textContent = "";
    }

    try {
      const outcome = await invoker("create_guild", {
        name,
        icon: selectedIconData,
        template: selectedTemplate,
      });
      isSubmitting = false;
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
        errorBox.removeAttribute("hidden");
        const kind =
          err && typeof err === "object" && "kind" in err ? String(err.kind) : "";
        let msg = "Не удалось создать сервер. Попробуйте позже.";
        if (kind === "forbidden" || kind === "maximumGuilds") {
          msg = "Достигнут лимит создания серверов (максимум 100)";
        } else if (kind === "unauthorized") {
          msg = "Сессия истекла. Войдите снова.";
        } else if (kind === "rateLimited") {
          msg = "Слишком много запросов. Подождите немного.";
        }
        errorBox.textContent = msg;
      }
      return undefined;
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
    activeTemplateIndex = 0;
    selectedIconData = null;
    isSubmitting = false;
    renderTemplateStep();

    dialog.hidden = false;
    dialog.removeAttribute("hidden");
    dialog.classList.add("is-open");
    if (opener && typeof opener.setAttribute === "function") {
      opener.setAttribute("aria-expanded", "true");
    }
    const firstCard = dialog.querySelector(".template-card");
    if (firstCard && typeof firstCard.focus === "function") {
      firstCard.focus();
    }
  }

  /**
   * Закрытие диалога.
   */
  function close() {
    dialog.hidden = true;
    dialog.setAttribute("hidden", "");
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
    setIcon,
    clearIcon,
    getIcon: () => selectedIconData,
    submit,
    getStep: () => currentStep,
    getSelectedTemplate: () => selectedTemplate,
    element: dialog,
  };
}

/**
 * Алиас фабрики контроллера визарда создания сервера (TASK-612).
 */
export const createCreateGuildWizard = createCreateGuildController;
