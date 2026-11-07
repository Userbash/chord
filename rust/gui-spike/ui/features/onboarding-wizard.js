// @ts-check
/**
 * Контроллер приветственного экрана и мастера адаптации сервера (task-262 / FE-P0-19).
 *
 * Оракул: `/home/sanya/re-deep/rest-static.json` (`GUILD_ONBOARDING`, `GUILD_WELCOME_SCREEN`;
 * до 5 приветственных каналов, описание до 140 символов, обязательные вопросы адаптации).
 *
 * Отвечает за:
 * - экспорт констант `MAX_WELCOME_CHANNELS = 5` и `MAX_WELCOME_DESCRIPTION_CHARS = 140`;
 * - чистую функцию `isValidWelcomeDescription(text)` для проверки описания приветственного экрана;
 * - чистую функцию `areRequiredPromptsSatisfied(prompts, selectedOptionIds)` для проверки
 *   выполнения всех обязательных вопросов адаптации и ограничения `singleSelect`;
 * - контроллер `createOnboardingWizardController(options)`, рендерящий регион
 *   `[data-onboarding-wizard]` (`role="region"`, `aria-label="Приветствие и адаптация сервера"`),
 *   до 5 приветственных каналов `[data-welcome-channel="<channelId>"]`, вопросы адаптации
 *   `[data-onboarding-prompt="<promptId>"]` с кнопками опций `[data-onboarding-option="<optionId>"]`
 *   (`aria-pressed="true"|"false"`), кнопку завершения `[data-onboarding-complete-btn]`
 *   и блок ошибки `[data-onboarding-error]`.
 *
 * @module ui/features/onboarding-wizard
 */

/** Максимальное количество приветственных каналов на экране приветствия сервера. */
export const MAX_WELCOME_CHANNELS = 5;

/** Максимальная длина описания приветственного экрана сервера в символах. */
export const MAX_WELCOME_DESCRIPTION_CHARS = 140;

/** Сообщение об ошибке при попытке завершить адаптацию без ответа на обязательные вопросы. */
const DEFAULT_ONBOARDING_ERROR_RU = "Ответьте на все обязательные вопросы адаптации сервера.";

/**
 * @typedef {object} WelcomeChannelItem
 * @property {string} channelId
 * @property {string} description
 * @property {string} [emojiName]
 */

/**
 * @typedef {object} OnboardingPromptOption
 * @property {string} id
 * @property {string} label
 */

/**
 * @typedef {object} OnboardingPromptItem
 * @property {string} id
 * @property {string} title
 * @property {boolean} [required]
 * @property {boolean} [singleSelect]
 * @property {ReadonlyArray<OnboardingPromptOption>} options
 */

/**
 * @typedef {object} NormalizedOnboardingPromptItem
 * @property {string} id
 * @property {string} title
 * @property {boolean} required
 * @property {boolean} singleSelect
 * @property {Array<OnboardingPromptOption>} options
 */

/**
 * @typedef {object} OnboardingWizardStatePatch
 * @property {string} [welcomeDescription]
 * @property {ReadonlyArray<WelcomeChannelItem>} [welcomeChannels]
 * @property {ReadonlyArray<OnboardingPromptItem>} [prompts]
 * @property {ReadonlyArray<string> | ReadonlySet<string>} [selectedOptionIds]
 */

/**
 * @typedef {object} OnboardingWizardStateSnapshot
 * @property {string} welcomeDescription
 * @property {Array<WelcomeChannelItem>} welcomeChannels
 * @property {Array<NormalizedOnboardingPromptItem>} prompts
 * @property {Array<string>} selectedOptionIds
 * @property {string} error
 */

/**
 * @typedef {object} OnboardingWizardControllerOptions
 * @property {any} container
 * @property {string} [welcomeDescription]
 * @property {ReadonlyArray<WelcomeChannelItem>} [welcomeChannels]
 * @property {ReadonlyArray<OnboardingPromptItem>} [prompts]
 * @property {ReadonlyArray<string> | ReadonlySet<string>} [selectedOptionIds]
 * @property {(selectedOptionIds: Array<string>) => void} [onCompleteOnboarding]
 * @property {(channelId: string) => void} [onOpenWelcomeChannel]
 */

/**
 * Проверяет корректность описания приветственного экрана сервера:
 * возвращает `true`, если `typeof text === 'string'` и длина `text.trim()`
 * находится в диапазоне `1..=140` символов; иначе `false`.
 *
 * @param {unknown} text
 * @returns {boolean}
 */
export function isValidWelcomeDescription(text) {
  if (typeof text !== "string") {
    return false;
  }
  const trimmedLength = text.trim().length;
  return trimmedLength >= 1 && trimmedLength <= MAX_WELCOME_DESCRIPTION_CHARS;
}

/**
 * Преобразует массив или множество идентификаторов выбранных опций в `Set<string>`.
 *
 * @param {ReadonlyArray<string> | ReadonlySet<string> | Iterable<string> | null | undefined} rawSelected
 * @returns {Set<string>}
 */
function toSelectedOptionSet(rawSelected) {
  /** @type {Set<string>} */
  const set = new Set();
  if (!rawSelected || typeof rawSelected !== "object") {
    return set;
  }
  if (
    Array.isArray(rawSelected) ||
    rawSelected instanceof Set ||
    typeof (/** @type {any} */ (rawSelected)[Symbol.iterator]) === "function"
  ) {
    for (const item of /** @type {Iterable<unknown>} */ (rawSelected)) {
      if (item !== undefined && item !== null) {
        const strId = String(item);
        if (strId.length > 0) {
          set.add(strId);
        }
      }
    }
  }
  return set;
}

/**
 * Проверяет, выполнены ли все обязательные вопросы адаптации (`prompts`):
 * возвращает `true`, если для каждого вопроса с `required === true` выбрана
 * хотя бы одна опция из его `options`, а для вопросов с `singleSelect === true`
 * выбрано не более одной опции из этого вопроса; иначе `false`.
 *
 * @param {ReadonlyArray<OnboardingPromptItem>} prompts
 * @param {ReadonlyArray<string> | ReadonlySet<string>} selectedOptionIds
 * @returns {boolean}
 */
export function areRequiredPromptsSatisfied(prompts, selectedOptionIds) {
  if (!Array.isArray(prompts)) {
    return false;
  }

  const selectedSet = toSelectedOptionSet(selectedOptionIds);

  for (const prompt of prompts) {
    if (!prompt || typeof prompt !== "object") {
      return false;
    }

    const rawOptions = Array.isArray(prompt.options) ? prompt.options : [];
    /** @type {Set<string>} */
    const seenOptionIds = new Set();
    let matchedCount = 0;

    for (const option of rawOptions) {
      if (!option || typeof option !== "object" || option.id === undefined || option.id === null) {
        continue;
      }
      const optionId = String(option.id);
      if (optionId.length === 0 || seenOptionIds.has(optionId)) {
        continue;
      }
      seenOptionIds.add(optionId);
      if (selectedSet.has(optionId)) {
        matchedCount += 1;
      }
    }

    if (prompt.required === true && matchedCount < 1) {
      return false;
    }
    if (prompt.singleSelect === true && matchedCount > 1) {
      return false;
    }
  }

  return true;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {{ createElement: (tag: string) => any }}
 */
function resolveDocument(containerEl) {
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
  throw new TypeError("options.container must provide ownerDocument.createElement");
}

/**
 * Заменяет дочерние узлы элемента на переданный массив узлов.
 *
 * @param {any} parentEl
 * @param {any[]} nodes
 */
function replaceElementChildren(parentEl, nodes) {
  if (typeof parentEl.replaceChildren === "function") {
    parentEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(parentEl.children)) {
    parentEl.children.length = 0;
  }
  for (const node of nodes) {
    parentEl.appendChild(node);
  }
}

/**
 * Клонирует и нормализует элемент приветственного канала.
 *
 * @param {WelcomeChannelItem} item
 * @returns {WelcomeChannelItem}
 */
function cloneWelcomeChannel(item) {
  /** @type {WelcomeChannelItem} */
  const cloned = {
    channelId: item.channelId !== undefined && item.channelId !== null ? String(item.channelId) : "",
    description: typeof item.description === "string" ? item.description : "",
  };
  if (typeof item.emojiName === "string" && item.emojiName.length > 0) {
    cloned.emojiName = item.emojiName;
  }
  return cloned;
}

/**
 * Нормализует список приветственных каналов (до `MAX_WELCOME_CHANNELS`).
 *
 * @param {ReadonlyArray<WelcomeChannelItem> | null | undefined} rawChannels
 * @returns {Array<WelcomeChannelItem>}
 */
function normalizeWelcomeChannels(rawChannels) {
  if (!Array.isArray(rawChannels)) return [];
  /** @type {Array<WelcomeChannelItem>} */
  const list = [];
  for (const item of rawChannels) {
    if (!item || typeof item !== "object") continue;
    const cloned = cloneWelcomeChannel(item);
    if (cloned.channelId.length === 0) continue;
    list.push(cloned);
    if (list.length >= MAX_WELCOME_CHANNELS) {
      break;
    }
  }
  return list;
}

/**
 * Клонирует и нормализует вопрос адаптации и его опции.
 *
 * @param {OnboardingPromptItem} prompt
 * @returns {NormalizedOnboardingPromptItem}
 */
function cloneOnboardingPrompt(prompt) {
  /** @type {Array<OnboardingPromptOption>} */
  const options = [];
  if (Array.isArray(prompt.options)) {
    for (const opt of prompt.options) {
      if (!opt || typeof opt !== "object") continue;
      const optId = opt.id !== undefined && opt.id !== null ? String(opt.id) : "";
      if (optId.length === 0) continue;
      options.push({
        id: optId,
        label: typeof opt.label === "string" ? opt.label : "",
      });
    }
  }
  return {
    id: prompt.id !== undefined && prompt.id !== null ? String(prompt.id) : "",
    title: typeof prompt.title === "string" ? prompt.title : "",
    required: prompt.required === true,
    singleSelect: prompt.singleSelect === true,
    options,
  };
}

/**
 * Нормализует массив вопросов адаптации.
 *
 * @param {ReadonlyArray<OnboardingPromptItem> | null | undefined} rawPrompts
 * @returns {Array<NormalizedOnboardingPromptItem>}
 */
function normalizeOnboardingPrompts(rawPrompts) {
  if (!Array.isArray(rawPrompts)) return [];
  /** @type {Array<NormalizedOnboardingPromptItem>} */
  const list = [];
  for (const item of rawPrompts) {
    if (!item || typeof item !== "object") continue;
    const cloned = cloneOnboardingPrompt(item);
    if (cloned.id.length === 0) continue;
    list.push(cloned);
  }
  return list;
}

/**
 * Нормализует список выбранных идентификаторов опций с сохранением порядка и дедупликацией.
 *
 * @param {ReadonlyArray<string> | ReadonlySet<string> | null | undefined} rawSelected
 * @returns {Array<string>}
 */
function normalizeSelectedOptionIds(rawSelected) {
  return Array.from(toSelectedOptionSet(rawSelected));
}

/**
 * Создаёт контроллер приветственного экрана и мастера адаптации сервера.
 *
 * @param {OnboardingWizardControllerOptions} options
 */
export function createOnboardingWizardController(options) {
  if (!options || !options.container) {
    throw new TypeError("options.container is required");
  }

  const container = options.container;
  const doc = resolveDocument(container);

  let welcomeDescription =
    typeof options.welcomeDescription === "string" ? options.welcomeDescription : "";
  /** @type {Array<WelcomeChannelItem>} */
  let welcomeChannels = normalizeWelcomeChannels(options.welcomeChannels);
  /** @type {Array<NormalizedOnboardingPromptItem>} */
  let prompts = normalizeOnboardingPrompts(options.prompts);
  /** @type {Array<string>} */
  let selectedOptionIds = normalizeSelectedOptionIds(options.selectedOptionIds);
  let errorText = "";
  let destroyed = false;

  const root = doc.createElement("section");
  root.setAttribute("data-onboarding-wizard", "");
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Приветствие и адаптация сервера");

  const descriptionEl = doc.createElement("p");
  descriptionEl.setAttribute("data-welcome-description", "");

  const channelsListEl = doc.createElement("div");
  channelsListEl.setAttribute("data-welcome-channels-list", "");

  const promptsListEl = doc.createElement("div");
  promptsListEl.setAttribute("data-onboarding-prompts-list", "");

  const errorEl = doc.createElement("div");
  errorEl.setAttribute("data-onboarding-error", "");
  errorEl.setAttribute("role", "alert");
  errorEl.textContent = "";

  const completeBtn = doc.createElement("button");
  completeBtn.setAttribute("type", "button");
  completeBtn.setAttribute("data-onboarding-complete-btn", "");
  completeBtn.textContent = "Завершить адаптацию";

  completeBtn.addEventListener("click", () => {
    if (destroyed) return;
    if (!areRequiredPromptsSatisfied(prompts, selectedOptionIds)) {
      errorText = DEFAULT_ONBOARDING_ERROR_RU;
      errorEl.textContent = errorText;
      return;
    }
    errorText = "";
    errorEl.textContent = "";
    if (typeof options.onCompleteOnboarding === "function") {
      options.onCompleteOnboarding([...selectedOptionIds]);
    }
  });

  /**
   * Переключает выбор опции внутри вопроса с учётом флага `singleSelect`.
   *
   * @param {NormalizedOnboardingPromptItem} prompt
   * @param {string} optionId
   */
  function handleToggleOption(prompt, optionId) {
    if (destroyed) return;
    const isAlreadySelected = selectedOptionIds.includes(optionId);

    if (isAlreadySelected) {
      selectedOptionIds = selectedOptionIds.filter((id) => id !== optionId);
    } else if (prompt.singleSelect) {
      const promptOptionIdSet = new Set(prompt.options.map((opt) => opt.id));
      selectedOptionIds = [
        ...selectedOptionIds.filter((id) => !promptOptionIdSet.has(id)),
        optionId,
      ];
    } else {
      selectedOptionIds = [...selectedOptionIds, optionId];
    }

    renderPrompts();
  }

  /**
   * Перерисовывает описание и до 5 приветственных каналов.
   */
  function renderWelcomeSection() {
    if (destroyed) return;
    descriptionEl.textContent = welcomeDescription;

    /** @type {any[]} */
    const channelNodes = [];
    const visibleChannels = welcomeChannels.slice(0, MAX_WELCOME_CHANNELS);

    for (const channel of visibleChannels) {
      const btn = doc.createElement("button");
      btn.setAttribute("type", "button");
      btn.setAttribute("data-welcome-channel", channel.channelId);

      if (channel.emojiName) {
        const emojiSpan = doc.createElement("span");
        emojiSpan.setAttribute("data-welcome-channel-emoji", "");
        emojiSpan.textContent = channel.emojiName;
        btn.appendChild(emojiSpan);
      }

      const descSpan = doc.createElement("span");
      descSpan.setAttribute("data-welcome-channel-description", "");
      descSpan.textContent = channel.description;
      btn.appendChild(descSpan);

      btn.addEventListener("click", () => {
        if (destroyed) return;
        if (typeof options.onOpenWelcomeChannel === "function") {
          options.onOpenWelcomeChannel(channel.channelId);
        }
      });

      channelNodes.push(btn);
    }

    replaceElementChildren(channelsListEl, channelNodes);
  }

  /**
   * Перерисовывает вопросы адаптации и кнопки опций.
   */
  function renderPrompts() {
    if (destroyed) return;
    const selectedSet = new Set(selectedOptionIds);
    /** @type {any[]} */
    const promptNodes = [];

    for (const prompt of prompts) {
      const promptEl = doc.createElement("div");
      promptEl.setAttribute("data-onboarding-prompt", prompt.id);

      const titleEl = doc.createElement("h3");
      titleEl.setAttribute("data-onboarding-prompt-title", "");
      titleEl.textContent = prompt.title;
      promptEl.appendChild(titleEl);

      const optionsGroupEl = doc.createElement("div");
      optionsGroupEl.setAttribute("data-onboarding-prompt-options", "");

      for (const option of prompt.options) {
        const isSelected = selectedSet.has(option.id);
        const optBtn = doc.createElement("button");
        optBtn.setAttribute("type", "button");
        optBtn.setAttribute("data-onboarding-option", option.id);
        optBtn.setAttribute("aria-pressed", isSelected ? "true" : "false");
        optBtn.textContent = option.label;

        optBtn.addEventListener("click", () => {
          handleToggleOption(prompt, option.id);
        });

        optionsGroupEl.appendChild(optBtn);
      }

      promptEl.appendChild(optionsGroupEl);
      promptNodes.push(promptEl);
    }

    replaceElementChildren(promptsListEl, promptNodes);
  }

  /**
   * Возвращает иммутабельную копию текущего состояния контроллера.
   *
   * @returns {OnboardingWizardStateSnapshot}
   */
  function getState() {
    return {
      welcomeDescription,
      welcomeChannels: welcomeChannels.map(cloneWelcomeChannel),
      prompts: prompts.map(cloneOnboardingPrompt),
      selectedOptionIds: [...selectedOptionIds],
      error: errorText,
    };
  }

  /**
   * Частично обновляет состояние контроллера и синхронизирует DOM.
   *
   * @param {OnboardingWizardStatePatch} [patch]
   * @returns {OnboardingWizardStateSnapshot}
   */
  function setState(patch) {
    if (patch && typeof patch === "object") {
      if (typeof patch.welcomeDescription === "string") {
        welcomeDescription = patch.welcomeDescription;
      }
      if (Array.isArray(patch.welcomeChannels)) {
        welcomeChannels = normalizeWelcomeChannels(patch.welcomeChannels);
      }
      if (Array.isArray(patch.prompts)) {
        prompts = normalizeOnboardingPrompts(patch.prompts);
      }
      if (patch.selectedOptionIds !== undefined) {
        selectedOptionIds = normalizeSelectedOptionIds(patch.selectedOptionIds);
      }
    }
    renderWelcomeSection();
    renderPrompts();
    return getState();
  }

  /**
   * Размонтирует контроллер и удаляет корневой элемент из контейнера.
   */
  function destroy() {
    destroyed = true;
    if (root.parentNode === container && typeof container.removeChild === "function") {
      container.removeChild(root);
    } else if (typeof root.remove === "function" && root.parentNode === container) {
      root.remove();
    } else {
      replaceElementChildren(container, []);
    }
  }

  renderWelcomeSection();
  renderPrompts();
  replaceElementChildren(root, [
    descriptionEl,
    channelsListEl,
    promptsListEl,
    errorEl,
    completeBtn,
  ]);
  replaceElementChildren(container, [root]);

  return {
    setState,
    getState,
    destroy,
  };
}
