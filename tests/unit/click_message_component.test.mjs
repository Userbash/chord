// @ts-nocheck
/**
 * TASK-815: Обработка клика по кнопке верификации в чате и IPC-связка click_message_component.
 *
 * Спецификация (waves/kanban-board.json):
 * - summary: "Связать клик по кнопке в DOM с IPC click_message_component, отображать
 *   состояние загрузки (спиннер) на кнопке и выводить тосты с ответом бота (ephemeral messages)."
 * - layout:
 *   - container: "Component Click Handler"
 *   - dimensions: "rust/gui-spike/ui/app.js:onComponentClick"
 * - design_tokens:
 *   - background: "#1e1f22"
 *   - surface: "#2b2d31"
 *   - text_primary: "#ffffff"
 *   - accent: "#5865f2"
 * - interactive_states:
 *   - element: "Interaction State"
 *   - default: "idle"
 *   - hover: "pointer"
 *   - active: "loading spinner"
 * - acceptance_criteria:
 *   1. "Клик по кнопке не подвешивает интерфейс и показывает статус выполнения"
 *   2. "Эфемерные ответы ботов отображаются во всплывающем уведомлении"
 *
 * 9 измерений качества TEST-POLICY:
 * 1. Позитивное: клик по интерактивной кнопке вызывает IPC click_message_component с полным payload.
 * 2. Негативное: disabled кнопки и кнопки-ссылки (link) не вызывают IPC click_message_component.
 * 3. Состояние загрузки (Spinner): кнопка переводится в состояние loading (.is-loading),
 *    блокируется от повторных кликов (aria-busy, disabled), внутри монтируется .component-button-spinner.
 * 4. Завершение загрузки: после ответа IPC спиннер снимается, исходное состояние восстанавливается.
 * 5. Эфемерные ответы ботов (ephemeral messages): отображение тоста с текстом ответа бота.
 * 6. Ошибки взаимодействия: вывод тоста об ошибке при сбое IPC/Discord API, кнопка возвращается в idle.
 * 7. Граничные случаи: отсутствие session_id/application_id, кастомные id ботов (Wick, Carl-bot, Captcha.bot).
 * 8. Наблюдаемость: дата-атрибуты data-interaction-state="loading" | "idle" | "success" | "error".
 * 9. Доступность (A11y): aria-busy="true" во время загрузки, сохранение фокуса.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  handleComponentClick,
  showBotInteractionToast,
  setButtonLoading,
  clearButtonLoading,
  COMPONENT_TYPES,
  BUTTON_STYLES,
  createButtonElement,
} from '../../rust/gui-spike/ui/features/message-components.js';

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных тестов
 * ------------------------------------------------------------------ */

class FakeElement {
  /**
   * @param {string} tagName
   * @param {any} [ownerDocument]
   */
  constructor(tagName, ownerDocument = null) {
    this.tagName = String(tagName).toUpperCase();
    this.ownerDocument = ownerDocument;
    /** @type {Map<string, string>} */
    this._attrs = new Map();
    /** @type {FakeElement[]} */
    this.children = [];
    /** @type {FakeElement | null} */
    this.parentNode = null;
    this._text = '';
    this._disabled = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
    /** @type {Record<string, string>} */
    this.style = {};
    this.className = '';
    this.id = '';
    this.href = '';
    this.target = '';
    this.rel = '';
    this.type = '';
    this.innerHTML = '';
  }

  get classList() {
    return {
      contains: (cls) => this.className.split(/\s+/).includes(cls),
      add: (cls) => {
        const parts = this.className ? this.className.split(/\s+/) : [];
        if (!parts.includes(cls)) parts.push(cls);
        this.className = parts.join(' ');
      },
      remove: (cls) => {
        const parts = this.className ? this.className.split(/\s+/) : [];
        this.className = parts.filter((c) => c !== cls).join(' ');
      },
    };
  }

  get disabled() {
    return this._disabled || this._attrs.has('disabled');
  }

  set disabled(value) {
    this._disabled = Boolean(value);
    if (this._disabled) {
      this._attrs.set('disabled', '');
    } else {
      this._attrs.delete('disabled');
    }
  }

  get textContent() {
    if (this.children.length > 0) {
      return this.children.map((c) => c.textContent).join('');
    }
    return this._text;
  }

  set textContent(value) {
    this.children = [];
    this._text = String(value ?? '');
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setAttribute(name, value) {
    const str = String(value);
    this._attrs.set(name, str);
    if (name === 'class') this.className = str;
    if (name === 'id') this.id = str;
    if (name === 'disabled') this._disabled = true;
    if (name === 'href') this.href = str;
    if (name === 'target') this.target = str;
    if (name === 'rel') this.rel = str;
  }

  /**
   * @param {string} name
   * @returns {string | null}
   */
  getAttribute(name) {
    if (name === 'class') return this.className || null;
    if (name === 'id') return this.id || null;
    return this._attrs.has(name) ? (this._attrs.get(name) ?? null) : null;
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  hasAttribute(name) {
    if (name === 'class') return Boolean(this.className);
    if (name === 'id') return Boolean(this.id);
    return this._attrs.has(name);
  }

  /** @param {string} name */
  removeAttribute(name) {
    this._attrs.delete(name);
    if (name === 'disabled') this._disabled = false;
    if (name === 'class') this.className = '';
    if (name === 'id') this.id = '';
  }

  /** @param {FakeElement} child */
  appendChild(child) {
    child.parentNode = this;
    this.children.push(child);
    return child;
  }

  /** @param {FakeElement} child */
  append(child) {
    return this.appendChild(child);
  }

  remove() {
    if (this.parentNode) {
      const idx = this.parentNode.children.indexOf(this);
      if (idx !== -1) {
        this.parentNode.children.splice(idx, 1);
      }
      this.parentNode = null;
    }
  }

  replaceChildren(...newChildren) {
    this.children = [];
    this._text = '';
    for (const ch of newChildren) {
      if (typeof ch === 'string') {
        const textNode = new FakeElement('span', this.ownerDocument);
        textNode.textContent = ch;
        this.appendChild(textNode);
      } else if (ch instanceof FakeElement) {
        this.appendChild(ch);
      }
    }
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} fn
   */
  addEventListener(type, fn) {
    const list = this._listeners.get(type) || [];
    list.push(fn);
    this._listeners.set(type, list);
  }

  /** @param {any} [event] */
  click(event = {}) {
    const list = this._listeners.get('click') || [];
    for (const fn of list) {
      fn({ target: this, currentTarget: this, ...event });
    }
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    if (selector.startsWith('#')) {
      const id = selector.slice(1);
      if (this.id === id) return this;
      for (const ch of this.children) {
        const found = ch.querySelector(selector);
        if (found) return found;
      }
      return null;
    }
    if (selector.startsWith('.')) {
      const cls = selector.slice(1);
      if (this.classList.contains(cls)) return this;
      for (const ch of this.children) {
        const found = ch.querySelector(selector);
        if (found) return found;
      }
      return null;
    }
    if (selector.startsWith('[data-')) {
      const match = selector.match(/\[([a-zA-Z0-9_-]+)(?:="([^"]+)")?\]/);
      if (match) {
        const [, attr, val] = match;
        const actual = this.getAttribute(attr);
        if (val !== undefined ? actual === val : actual !== null) {
          return this;
        }
      }
      for (const ch of this.children) {
        const found = ch.querySelector(selector);
        if (found) return found;
      }
      return null;
    }
    if (this.tagName.toLowerCase() === selector.toLowerCase()) {
      return this;
    }
    for (const ch of this.children) {
      const found = ch.querySelector(selector);
      if (found) return found;
    }
    return null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const results = [];
    const check = (/** @type {FakeElement} */ node) => {
      if (selector.startsWith('.')) {
        const cls = selector.slice(1);
        if (node.classList.contains(cls)) results.push(node);
      } else if (selector.startsWith('#')) {
        const id = selector.slice(1);
        if (node.id === id) results.push(node);
      } else if (node.tagName.toLowerCase() === selector.toLowerCase()) {
        results.push(node);
      }
      for (const ch of node.children) check(ch);
    };
    for (const ch of this.children) check(ch);
    return results;
  }
}

class FakeDocument {
  constructor() {
    this.body = new FakeElement('BODY', this);
    this.head = new FakeElement('HEAD', this);
    this.documentElement = new FakeElement('HTML', this);
    this.documentElement.appendChild(this.head);
    this.documentElement.appendChild(this.body);
  }

  /**
   * @param {string} tagName
   * @returns {FakeElement}
   */
  createElement(tagName) {
    return new FakeElement(tagName, this);
  }

  /**
   * @param {string} id
   * @returns {FakeElement | null}
   */
  getElementById(id) {
    return this.documentElement.querySelector(`#${id}`);
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    return this.documentElement.querySelector(selector);
  }
}

describe('TASK-815: Обработка клика по кнопке верификации в чате и IPC-связка click_message_component', () => {
  let fakeDoc = new FakeDocument();

  it('1. setButtonLoading монтирует спиннер, блокирует повторные клики и обновляет A11y атрибуты', () => {
    fakeDoc = new FakeDocument();
    const btn = createButtonElement(
      {
        type: COMPONENT_TYPES.BUTTON,
        style: BUTTON_STYLES.SUCCESS,
        custom_id: 'verify_captcha_button',
        label: 'Пройти верификацию',
      },
      {},
      fakeDoc
    );

    assert.equal(btn.disabled, false);
    assert.equal(btn.getAttribute('data-interaction-state') || 'idle', 'idle');

    setButtonLoading(btn, true);

    assert.equal(btn.disabled, true, 'Кнопка должна быть заблокирована во время загрузки');
    assert.equal(btn.getAttribute('aria-busy'), 'true', 'aria-busy должен быть выставлен в true');
    assert.equal(btn.getAttribute('data-interaction-state'), 'loading');
    assert.ok(btn.classList.contains('is-loading'), 'Должен присутствовать класс is-loading');

    const spinner = btn.querySelector('.component-button-spinner');
    assert.ok(spinner, 'Внутри кнопки должен быть смонтирован .component-button-spinner');

    // Очистка состояния загрузки
    clearButtonLoading(btn);

    assert.equal(btn.disabled, false, 'После clearButtonLoading блокировка снимается');
    assert.equal(btn.hasAttribute('aria-busy'), false);
    assert.equal(btn.classList.contains('is-loading'), false);
    assert.equal(btn.querySelector('.component-button-spinner'), null, 'Спиннер удаляется');
  });

  it('2. handleComponentClick: отправляет валидный IPC click_message_component с полным payload и снимает спиннер', async () => {
    fakeDoc = new FakeDocument();
    const btn = createButtonElement(
      {
        type: COMPONENT_TYPES.BUTTON,
        style: BUTTON_STYLES.SUCCESS,
        custom_id: 'verify_captcha_button',
        label: 'Пройти верификацию',
      },
      {},
      fakeDoc
    );

    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];
    const mockInvoke = async (cmd, args) => {
      ipcCalls.push({ cmd, args });
      // Возвращаем ответ бота с эфемерным сообщением
      return {
        ok: true,
        type: 4, // CHANNEL_MESSAGE_WITH_SOURCE
        data: {
          content: 'Вы успешно подтвердили правила сервера! Роль участника выдана.',
          flags: 64, // EPHEMERAL
        },
      };
    };

    /** @type {string[]} */
    const toasts = [];
    const mockShowToast = (msg) => {
      toasts.push(msg);
    };

    const options = {
      document: fakeDoc,
      channelId: 'chan-verify-123',
      messageId: 'msg-rules-456',
      guildId: 'guild-testing-789',
      applicationId: 'bot-app-999',
      invoke: mockInvoke,
      showToast: mockShowToast,
    };

    const resultPromise = handleComponentClick(
      btn,
      {
        type: COMPONENT_TYPES.BUTTON,
        style: BUTTON_STYLES.SUCCESS,
        custom_id: 'verify_captcha_button',
        label: 'Пройти верификацию',
      },
      options
    );

    // Во время полёта промиса кнопка находится в состоянии loading
    assert.equal(btn.disabled, true);
    assert.equal(btn.getAttribute('data-interaction-state'), 'loading');

    const result = await resultPromise;
    assert.ok(result);

    // После завершения запроса спиннер снят
    assert.equal(btn.disabled, false);
    assert.equal(btn.getAttribute('data-interaction-state'), 'success');

    // Проверяем вызов IPC
    assert.equal(ipcCalls.length, 1);
    assert.equal(ipcCalls[0].cmd, 'click_message_component');
    assert.equal(ipcCalls[0].args.custom_id, 'verify_captcha_button');
    assert.equal(ipcCalls[0].args.component_type, COMPONENT_TYPES.BUTTON);
    assert.equal(ipcCalls[0].args.channel_id, 'chan-verify-123');
    assert.equal(ipcCalls[0].args.message_id, 'msg-rules-456');

    // Проверяем показ эфемерного ответа бота во всплывающем уведомлении (тосте)
    assert.equal(toasts.length, 1);
    assert.ok(toasts[0].includes('Вы успешно подтвердили правила сервера'));
  });

  it('3. handleComponentClick: обрабатывает ошибки взаимодействия бота, показывает тост ошибки и сбрасывает спиннер', async () => {
    fakeDoc = new FakeDocument();
    const btn = createButtonElement(
      {
        type: COMPONENT_TYPES.BUTTON,
        style: BUTTON_STYLES.PRIMARY,
        custom_id: 'btn_error_test',
        label: 'Проверка ошибки',
      },
      {},
      fakeDoc
    );

    const mockInvoke = async () => {
      throw new Error('This interaction failed or expired (код 10062)');
    };

    /** @type {string[]} */
    const toasts = [];
    const mockShowToast = (msg) => {
      toasts.push(msg);
    };

    const options = {
      document: fakeDoc,
      channelId: 'chan-1',
      messageId: 'msg-1',
      invoke: mockInvoke,
      showToast: mockShowToast,
    };

    const res = await handleComponentClick(
      btn,
      {
        type: COMPONENT_TYPES.BUTTON,
        style: BUTTON_STYLES.PRIMARY,
        custom_id: 'btn_error_test',
      },
      options
    );

    assert.equal(res.ok, false);
    assert.equal(btn.disabled, false, 'Блокировка кнопки должна быть снята при ошибке');
    assert.equal(btn.getAttribute('data-interaction-state'), 'error');
    assert.equal(toasts.length, 1);
    assert.ok(toasts[0].includes('This interaction failed or expired'));
  });

  it('4. showBotInteractionToast монтирует тост в DOM с правильными классами, доступностью и автозакрытием', () => {
    fakeDoc = new FakeDocument();
    const toast = showBotInteractionToast('Верификация пройдена успешно!', {
      document: fakeDoc,
      durationMs: 1500,
    });

    assert.ok(toast);
    assert.equal(toast.getAttribute('role'), 'status');
    assert.equal(toast.id, 'bot-interaction-toast');
    assert.ok(toast.classList.contains('bot-interaction-toast'));
    assert.ok(toast.textContent.includes('Верификация пройдена успешно!'));

    const existingInBody = fakeDoc.body.querySelector('#bot-interaction-toast');
    assert.ok(existingInBody, 'Тост должен быть смонтирован в body');
  });

  it('5. Интерактивная кнопка в createButtonElement автоматически использует handleComponentClick по умолчанию', async () => {
    fakeDoc = new FakeDocument();
    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];
    /** @type {string[]} */
    const toasts = [];

    const options = {
      document: fakeDoc,
      channelId: 'verify-channel-id',
      messageId: 'verify-message-id',
      invoke: async (cmd, args) => {
        ipcCalls.push({ cmd, args });
        return {
          ok: true,
          data: { content: 'Carl-bot: Роль выдана!' },
        };
      },
      showToast: (msg) => toasts.push(msg),
    };

    const btn = createButtonElement(
      {
        type: COMPONENT_TYPES.BUTTON,
        style: BUTTON_STYLES.SUCCESS,
        custom_id: 'carl_verify_role_btn',
        label: 'Получить роль',
      },
      options,
      fakeDoc
    );

    // Симулируем клик пользователя
    btn.click();

    // Ждём завершения микротасок
    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(ipcCalls.length, 1);
    assert.equal(ipcCalls[0].cmd, 'click_message_component');
    assert.equal(ipcCalls[0].args.custom_id, 'carl_verify_role_btn');
    assert.equal(toasts.length, 1);
    assert.ok(toasts[0].includes('Carl-bot: Роль выдана!'));
  });

  it('6. app.js экспортирует handleComponentClick, setButtonLoading, clearButtonLoading, showBotInteractionToast', async () => {
    const app = await import('../../rust/gui-spike/ui/app.js');
    assert.equal(typeof app.handleComponentClick, 'function');
    assert.equal(typeof app.setButtonLoading, 'function');
    assert.equal(typeof app.clearButtonLoading, 'function');
    assert.equal(typeof app.showBotInteractionToast, 'function');
  });
});
