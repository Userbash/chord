// @ts-check
/**
 * TASK-813 (G2 Frontend): Рендеринг кнопок компонентов сообщений Discord
 * (Message ActionRow & Buttons UI).
 * TASK-815 (G2 Frontend): Обработка клика по кнопке верификации в чате и IPC-связка click_message_component.
 *
 * @module ui/features/message-components
 */

/**
 * Типы компонентов сообщений Discord (Discord API Message Component Types).
 * @type {Readonly<{
 *   ACTION_ROW: 1,
 *   BUTTON: 2,
 *   STRING_SELECT: 3,
 *   TEXT_INPUT: 4,
 *   USER_SELECT: 5,
 *   ROLE_SELECT: 6,
 *   MENTIONABLE_SELECT: 7,
 *   CHANNEL_SELECT: 8,
 * }>}
 */
export const COMPONENT_TYPES = Object.freeze({
  ACTION_ROW: 1,
  BUTTON: 2,
  STRING_SELECT: 3,
  TEXT_INPUT: 4,
  USER_SELECT: 5,
  ROLE_SELECT: 6,
  MENTIONABLE_SELECT: 7,
  CHANNEL_SELECT: 8,
});

/**
 * Стили кнопок Discord (Discord Button Styles).
 * @type {Readonly<{
 *   PRIMARY: 1,
 *   SECONDARY: 2,
 *   SUCCESS: 3,
 *   DANGER: 4,
 *   LINK: 5,
 * }>}
 */
export const BUTTON_STYLES = Object.freeze({
  PRIMARY: 1,
  SECONDARY: 2,
  SUCCESS: 3,
  DANGER: 4,
  LINK: 5,
});

/**
 * Имена CSS-модификаторов для стилей кнопок.
 * @type {Readonly<Record<number, string>>}
 */
const BUTTON_STYLE_NAMES = Object.freeze({
  1: 'primary',
  2: 'secondary',
  3: 'success',
  4: 'danger',
  5: 'link',
});

/**
 * SVG-иконка внешней ссылки для кнопок Link (style 5).
 */
const EXTERNAL_LINK_SVG = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>`;

/**
 * @typedef {{
 *   type: number,
 *   style?: number,
 *   label?: string,
 *   custom_id?: string,
 *   customId?: string,
 *   url?: string,
 *   disabled?: boolean,
 *   emoji?: {
 *     id?: string | null,
 *     name?: string,
 *     animated?: boolean,
 *   } | null,
 * }} MessageButtonData
 *
 * @typedef {{
 *   type: number,
 *   components?: MessageButtonData[],
 * }} MessageActionRowData
 *
 * @typedef {{
 *   document?: any,
 *   channelId?: string,
 *   messageId?: string,
 *   guildId?: string,
 *   applicationId?: string,
 *   sessionId?: string,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   showToast?: (message: string) => void,
 *   onComponentClick?: (btn: any, component: MessageButtonData, options: MessageComponentOptions) => void,
 * }} MessageComponentOptions
 */

/**
 * Безопасно находит фабрику DOM-элементов.
 * @param {any} [containerOrDoc]
 * @returns {any}
 */
function resolveDocument(containerOrDoc) {
  if (containerOrDoc && typeof containerOrDoc.createElement === 'function') {
    return containerOrDoc;
  }
  if (containerOrDoc && containerOrDoc.ownerDocument && typeof containerOrDoc.ownerDocument.createElement === 'function') {
    return containerOrDoc.ownerDocument;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === 'function') {
    return globalDoc;
  }
  throw new TypeError('Message components require a DOM document to createElement');
}

/**
 * Устанавливает состояние загрузки (спиннер) на кнопку компонента.
 *
 * @param {any} btn
 * @param {boolean} [loading]
 */
export function setButtonLoading(btn, loading = true) {
  if (!btn) return;
  if (loading) {
    btn.disabled = true;
    btn.setAttribute('disabled', '');
    btn.setAttribute('aria-busy', 'true');
    btn.setAttribute('data-interaction-state', 'loading');
    btn.classList.add('is-loading');

    // Если спиннер ещё не смонтирован — добавляем его
    if (!btn.querySelector('.component-button-spinner')) {
      const doc = resolveDocument(btn.ownerDocument || btn);
      const spinner = doc.createElement('span');
      spinner.className = 'component-button-spinner';
      spinner.setAttribute('aria-hidden', 'true');
      btn.append(spinner);
    }
  } else {
    clearButtonLoading(btn);
  }
}

/**
 * Очищает состояние загрузки с кнопки компонента.
 *
 * @param {any} btn
 * @param {string} [finalState]
 */
export function clearButtonLoading(btn, finalState = 'idle') {
  if (!btn) return;
  btn.disabled = false;
  btn.removeAttribute('disabled');
  btn.removeAttribute('aria-busy');
  btn.setAttribute('data-interaction-state', finalState);
  btn.classList.remove('is-loading');

  const spinner = btn.querySelector('.component-button-spinner');
  if (spinner && typeof spinner.remove === 'function') {
    spinner.remove();
  }
}

/**
 * Отображает всплывающее уведомление (тост) с ответом бота (ephemeral message)
 * или сообщением об ошибке взаимодействия.
 *
 * @param {string} message
 * @param {{ document?: any, durationMs?: number, isError?: boolean }} [options]
 * @returns {any}
 */
export function showBotInteractionToast(message, options = {}) {
  const doc = resolveDocument(options.document);
  if (!doc || !doc.body) return null;

  const existing = doc.getElementById('bot-interaction-toast');
  if (existing && typeof existing.remove === 'function') {
    existing.remove();
  }

  const toast = doc.createElement('div');
  toast.id = 'bot-interaction-toast';
  toast.className = `bot-interaction-toast${options.isError ? ' bot-interaction-toast--error' : ''}`;
  toast.setAttribute('role', 'status');
  toast.setAttribute('aria-live', 'polite');
  toast.style.cssText =
    'position:fixed;bottom:76px;left:50%;transform:translateX(-50%);background:#1e1f22;color:#ffffff;border:1px solid #3f4147;border-radius:8px;padding:10px 18px;font-size:13px;font-weight:600;box-shadow:0 12px 32px rgba(0,0,0,0.75);z-index:10200;display:flex;align-items:center;gap:10px;pointer-events:none;';

  const iconSpan = doc.createElement('span');
  iconSpan.setAttribute('aria-hidden', 'true');
  iconSpan.textContent = options.isError ? '⚠️' : '🤖';

  const textSpan = doc.createElement('span');
  textSpan.textContent = String(message);

  toast.append(iconSpan);
  toast.append(textSpan);
  doc.body.append(toast);

  const duration = options.durationMs || 3500;
  setTimeout(() => {
    if (typeof toast.remove === 'function') {
      toast.remove();
    }
  }, duration);

  return toast;
}

/**
 * Обрабатывает клик по кнопке компонента сообщения:
 * 1. Включает спиннер загрузки (.component-button-spinner, .is-loading, aria-busy="true")
 * 2. Вызывает IPC click_message_component
 * 3. Показывает тост с ответом бота (ephemeral response content)
 * 4. Снимает спиннер и возвращает кнопку в исходное состояние
 *
 * @param {any} btn
 * @param {MessageButtonData} buttonData
 * @param {MessageComponentOptions} [options]
 * @returns {Promise<{ ok: boolean, data?: any, error?: any }>}
 */
export async function handleComponentClick(btn, buttonData, options = {}) {
  const customId = buttonData.custom_id || buttonData.customId || '';
  if (!customId) {
    return { ok: false, error: 'missing custom_id' };
  }

  // Включаем спиннер загрузки
  setButtonLoading(btn, true);

  const invokeFn =
    typeof options.invoke === 'function'
      ? options.invoke
      : typeof (/** @type {any} */ (globalThis)).__TAURI_INVOKE__ === 'function'
      ? (/** @type {any} */ (globalThis)).__TAURI_INVOKE__
      : null;

  const showToastFn =
    typeof options.showToast === 'function'
      ? options.showToast
      : (/** @type {string} */ msg) => showBotInteractionToast(msg, { document: options.document });

  try {
    let result = null;
    if (invokeFn) {
      const ipcCommand = 'click_message_component';
      result = await invokeFn(ipcCommand, {
        custom_id: customId,
        component_type: buttonData.type || COMPONENT_TYPES.BUTTON,
        channel_id: options.channelId,
        message_id: options.messageId,
        guild_id: options.guildId,
        application_id: options.applicationId,
        session_id: options.sessionId,
      });
    }

    clearButtonLoading(btn, 'success');

    // Если бот вернул эфемерный ответ или текст
    const replyText =
      result?.data?.content ||
      result?.content ||
      result?.message ||
      (result?.ok ? 'Действие выполнено' : null);

    if (replyText) {
      showToastFn(String(replyText));
    }

    return { ok: true, data: result };
  } catch (err) {
    clearButtonLoading(btn, 'error');
    const errMsg = String(
      (err && typeof err === 'object' && /** @type {any} */ (err).message) ||
        err ||
        'Не удалось выполнить действие'
    );
    showToastFn(errMsg);
    return { ok: false, error: err };
  }
}

/**
 * Создаёт кнопку компонента Discord.
 *
 * @param {MessageButtonData} buttonData
 * @param {MessageComponentOptions} [options]
 * @param {any} [doc]
 * @returns {any}
 */
export function createButtonElement(buttonData, options = {}, doc = null) {
  const document = doc || resolveDocument(options.document);
  const style = Number(buttonData.style ?? (buttonData.url ? BUTTON_STYLES.LINK : BUTTON_STYLES.PRIMARY));
  const styleName = BUTTON_STYLE_NAMES[style] || 'secondary';
  const isLink = style === BUTTON_STYLES.LINK;

  const btn = document.createElement(isLink ? 'a' : 'button');
  btn.className = `component-button component-button--${styleName}`;
  btn.setAttribute('data-component-type', String(COMPONENT_TYPES.BUTTON));
  btn.setAttribute('data-component-style', String(style));

  const customId = buttonData.custom_id || buttonData.customId || '';
  if (customId) {
    btn.setAttribute('data-custom-id', customId);
  }

  if (isLink) {
    const rawUrl = String(buttonData.url || '').trim();
    if (rawUrl) {
      btn.setAttribute('href', rawUrl);
      btn.setAttribute('target', '_blank');
      btn.setAttribute('rel', 'noopener noreferrer');
    }
  } else {
    btn.setAttribute('type', 'button');
  }

  // Состояние disabled
  const isDisabled = Boolean(buttonData.disabled);
  if (isDisabled) {
    btn.disabled = true;
    btn.setAttribute('disabled', '');
    btn.setAttribute('aria-disabled', 'true');
  }

  // Эмодзи (Unicode или Custom CDN)
  if (buttonData.emoji && typeof buttonData.emoji === 'object') {
    const emoji = buttonData.emoji;
    if (emoji.id) {
      const img = document.createElement('img');
      img.className = 'component-button-custom-emoji';
      const ext = emoji.animated ? 'gif' : 'webp';
      img.setAttribute(
        'src',
        `https://cdn.discordapp.com/emojis/${emoji.id}.${ext}?size=48&quality=lossless`
      );
      img.setAttribute('alt', `:${emoji.name || 'emoji'}:`);
      img.setAttribute('title', `:${emoji.name || 'emoji'}:`);
      btn.append(img);
    } else if (emoji.name) {
      const emojiSpan = document.createElement('span');
      emojiSpan.className = 'component-button-emoji';
      emojiSpan.textContent = emoji.name;
      btn.append(emojiSpan);
    }
  }

  // Лейбл кнопки
  if (buttonData.label) {
    const labelSpan = document.createElement('span');
    labelSpan.className = 'component-button-label';
    labelSpan.textContent = buttonData.label;
    btn.append(labelSpan);
  }

  // Иконка для внешней ссылки
  if (isLink) {
    const iconSpan = document.createElement('span');
    iconSpan.className = 'component-button-link-icon';
    iconSpan.setAttribute('aria-hidden', 'true');
    iconSpan.innerHTML = EXTERNAL_LINK_SVG;
    btn.append(iconSpan);
  }

  // Обработка клика
  if (!isLink) {
    btn.addEventListener('click', (/** @type {any} */ event) => {
      if (btn.disabled || btn.getAttribute('aria-disabled') === 'true') {
        event?.preventDefault?.();
        return;
      }
      if (typeof options.onComponentClick === 'function') {
        options.onComponentClick(btn, buttonData, options);
      } else {
        void handleComponentClick(btn, buttonData, options);
      }
    });
  }

  return btn;
}

/**
 * Создаёт ActionRow элемент и наполняет его кнопками.
 *
 * @param {MessageButtonData[]} buttons
 * @param {MessageComponentOptions} [options]
 * @param {any} [doc]
 * @returns {any}
 */
export function createActionRowElement(buttons, options = {}, doc = null) {
  const document = doc || resolveDocument(options.document);
  const row = document.createElement('div');
  row.className = 'message-action-row';
  row.setAttribute('data-component-type', String(COMPONENT_TYPES.ACTION_ROW));

  if (Array.isArray(buttons)) {
    for (const btnData of buttons) {
      if (!btnData || typeof btnData !== 'object') continue;
      const btnEl = createButtonElement(btnData, options, document);
      row.append(btnEl);
    }
  }

  return row;
}

/**
 * Создаёт контейнер компонентов сообщения Discord (.message-components-container).
 *
 * @param {Array<MessageActionRowData | MessageButtonData>} components
 * @param {MessageComponentOptions} [options]
 * @returns {any | null}
 */
export function createMessageComponentsContainer(components, options = {}) {
  if (!Array.isArray(components) || components.length === 0) {
    return null;
  }

  const document = resolveDocument(options.document);
  const container = document.createElement('div');
  container.className = 'message-components-container';
  container.setAttribute('role', 'group');
  container.setAttribute('aria-label', 'Компоненты сообщения');

  let rowCount = 0;

  for (const item of components) {
    if (!item || typeof item !== 'object') continue;

    const rowComponents = /** @type {any} */ (item).components;
    if (item.type === COMPONENT_TYPES.ACTION_ROW && Array.isArray(rowComponents)) {
      if (rowComponents.length > 0) {
        const rowEl = createActionRowElement(rowComponents, options, document);
        container.append(rowEl);
        rowCount += 1;
      }
    } else if (item.type === COMPONENT_TYPES.BUTTON) {
      // Прямая кнопка без явного ActionRow: оборачиваем в строку
      const rowEl = createActionRowElement([/** @type {MessageButtonData} */ (item)], options, document);
      container.append(rowEl);
      rowCount += 1;
    }
  }

  if (rowCount === 0) {
    return null;
  }

  return container;
}

/**
 * Рендерит компоненты сообщения в DOM-контейнер либо возвращает созданный контейнер.
 *
 * @param {any} containerOrComponents
 * @param {any} [componentsOrOptions]
 * @param {MessageComponentOptions} [maybeOptions]
 * @returns {any | null}
 */
export function renderMessageComponents(containerOrComponents, componentsOrOptions, maybeOptions = {}) {
  // Вызов renderMessageComponents(components, options) -> возвращает узел
  if (Array.isArray(containerOrComponents)) {
    const opts = componentsOrOptions && typeof componentsOrOptions === 'object' ? componentsOrOptions : {};
    return createMessageComponentsContainer(containerOrComponents, opts);
  }

  // Вызов renderMessageComponents(containerEl, components, options) -> добавляет в контейнер
  const targetContainer = containerOrComponents;
  const components = Array.isArray(componentsOrOptions) ? componentsOrOptions : [];
  const options = maybeOptions && typeof maybeOptions === 'object' ? maybeOptions : {};

  if (!targetContainer || components.length === 0) return null;

  const doc = resolveDocument(options.document || targetContainer);
  const compContainer = createMessageComponentsContainer(components, { ...options, document: doc });
  if (compContainer) {
    targetContainer.append(compContainer);
  }
  return compContainer;
}
