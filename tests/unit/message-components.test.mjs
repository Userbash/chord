// @ts-check
/**
 * TASK-813 (G2 Frontend): Unit-тесты рендеринга кнопок и ActionRow компонентов сообщений Discord
 * (`rust/gui-spike/ui/features/message-components.js`, `rust/gui-spike/ui/components/message-components.css`).
 *
 * Требования:
 * - Отображение контейнеров ActionRow и кнопок Discord:
 *   - Primary (blurple, style 1)
 *   - Secondary (grey, style 2)
 *   - Success (green, style 3)
 *   - Danger (red, style 4)
 *   - Link (style 5, с иконкой внешней ссылки)
 * - Поддержка emoji (unicode и custom Discord cdn) и label
 * - Состояния disabled, hover, active (Verify Button: default #23a55a, hover opacity 0.85, active scale 0.98)
 * - Интеграция в renderChannelMessage (и экспорт renderMessage / renderMessageComponents)
 * - CSS файл message-components.css
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { ShimDocument, ShimElement } from '../support/gui-dom-harness.mjs';

import {
  COMPONENT_TYPES,
  BUTTON_STYLES,
  renderMessageComponents,
  createMessageComponentsContainer,
} from '../../rust/gui-spike/ui/features/message-components.js';

import {
  renderChannelMessage,
} from '../../rust/gui-spike/ui/features/chat.js';

import {
  renderMessage,
} from '../../rust/gui-spike/ui/app.js';

/**
 * @template T
 * @param {(doc: ShimDocument) => T} fn
 * @returns {T}
 */
function withChatDoc(fn) {
  const prevDoc = globalThis.document;
  const doc = new ShimDocument();
  globalThis.document = /** @type {any} */ (doc);
  try {
    return fn(doc);
  } finally {
    if (prevDoc === undefined) {
      delete /** @type {any} */ (globalThis).document;
    } else {
      globalThis.document = prevDoc;
    }
  }
}

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных тестов рендера
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
    this.src = '';
    this.alt = '';
    this.title = '';
  }

  get classList() {
    return {
      /** @param {string} cls */
      contains: (cls) => this.className.split(/\s+/).includes(cls),
      /** @param {string} cls */
      add: (cls) => {
        const parts = this.className ? this.className.split(/\s+/) : [];
        if (!parts.includes(cls)) parts.push(cls);
        this.className = parts.join(' ');
      },
      /** @param {string} cls */
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

  /** @returns {string} */
  get textContent() {
    if (this.children.length > 0) {
      return this.children.map((c) => String(c.textContent)).join('');
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
    if (name === 'src') this.src = str;
    if (name === 'alt') this.alt = str;
    if (name === 'title') this.title = str;
  }

  /**
   * @param {...(FakeElement | string)} nodes
   */
  replaceChildren(...nodes) {
    this.children = [];
    this.append(...nodes);
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

  /** @param {...(FakeElement | string)} nodes */
  append(...nodes) {
    for (const node of nodes) {
      if (typeof node === 'string') {
        const textNode = new FakeElement('#text', this.ownerDocument);
        textNode._text = node;
        this.appendChild(textNode);
      } else if (node instanceof FakeElement) {
        this.appendChild(node);
      }
    }
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    const arr = this._listeners.get(type) ?? [];
    arr.push(listener);
    this._listeners.set(type, arr);
  }

  /**
   * @param {string} type
   * @param {any} [event]
   */
  dispatchEvent(type, event = {}) {
    const arr = this._listeners.get(type) ?? [];
    for (const fn of arr) {
      fn({ target: this, currentTarget: this, ...event });
    }
  }

  click() {
    this.dispatchEvent('click', { type: 'click' });
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    const all = this.querySelectorAll(selector);
    return all.length > 0 ? (all[0] ?? null) : null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const results = [];
    /** @param {FakeElement} el */
    const check = (el) => {
      let match = true;
      if (selector.startsWith('.')) {
        const cls = selector.slice(1);
        match = el.className.split(/\s+/).includes(cls);
      } else if (selector.startsWith('#')) {
        match = el.id === selector.slice(1);
      } else if (selector.startsWith('[') && selector.endsWith(']')) {
        const inner = selector.slice(1, -1);
        if (inner.includes('=')) {
          const [k = '', vRaw = ''] = inner.split('=');
          const v = vRaw.replace(/^["']|["']$/g, '');
          match = el.getAttribute(k) === v;
        } else {
          match = el.hasAttribute(inner);
        }
      } else if (/^[A-Za-z0-9_-]+$/.test(selector)) {
        match = el.tagName.toLowerCase() === selector.toLowerCase();
      }
      if (match) results.push(el);
      for (const child of el.children) {
        check(child);
      }
    };
    for (const child of this.children) {
      check(child);
    }
    return results;
  }
}

class FakeDocument {
  /** @param {string} tag */
  createElement(tag) {
    return new FakeElement(tag, this);
  }
}

const fakeDoc = new FakeDocument();

/* ------------------------------------------------------------------ *
 * Тестовые данные компонентов сообщений
 * ------------------------------------------------------------------ */

const sampleBotVerificationComponents = [
  {
    type: 1, // ActionRow
    components: [
      {
        type: 2, // Button
        style: 3, // Success (green)
        label: 'Пройти верификацию',
        custom_id: 'verify_captcha_button',
        emoji: { name: '🛡️' },
        disabled: false,
      },
      {
        type: 2, // Button
        style: 5, // Link
        label: 'Правила сервера',
        url: 'https://discord.com/guidelines',
        emoji: { name: '📜' },
      },
    ],
  },
  {
    type: 1, // ActionRow 2
    components: [
      {
        type: 2, // Button
        style: 1, // Primary (blurple)
        label: 'Подробнее',
        custom_id: 'btn_info',
      },
      {
        type: 2, // Button
        style: 2, // Secondary (grey)
        label: 'Помощь',
        custom_id: 'btn_help',
      },
      {
        type: 2, // Button
        style: 4, // Danger (red)
        label: 'Пожаловаться',
        custom_id: 'btn_report',
        disabled: true,
      },
    ],
  },
];

describe('TASK-813: Message Components (ActionRow & Buttons UI)', () => {
  it('1. Константы COMPONENT_TYPES и BUTTON_STYLES соответствуют спецификации Discord', () => {
    assert.equal(COMPONENT_TYPES.ACTION_ROW, 1);
    assert.equal(COMPONENT_TYPES.BUTTON, 2);
    assert.equal(COMPONENT_TYPES.STRING_SELECT, 3);

    assert.equal(BUTTON_STYLES.PRIMARY, 1);
    assert.equal(BUTTON_STYLES.SECONDARY, 2);
    assert.equal(BUTTON_STYLES.SUCCESS, 3);
    assert.equal(BUTTON_STYLES.DANGER, 4);
    assert.equal(BUTTON_STYLES.LINK, 5);
  });

  it('2. Рендерит контейнер .message-components-container и .message-action-row для каждого ActionRow', () => {
    const container = createMessageComponentsContainer(sampleBotVerificationComponents, {
      document: fakeDoc,
    });
    assert.ok(container, 'Контейнер должен быть создан');
    assert.equal(container.className, 'message-components-container');
    assert.equal(container.getAttribute('role'), 'group');
    assert.equal(container.getAttribute('aria-label'), 'Компоненты сообщения');

    const rows = container.querySelectorAll('.message-action-row');
    assert.equal(rows.length, 2, 'Должно быть 2 строки ActionRow');
    assert.equal(rows[0].getAttribute('data-component-type'), '1');
    assert.equal(rows[1].getAttribute('data-component-type'), '1');
  });

  it('3. Рендерит кнопки всех 5 стилей с правильными классами и цветами дизайна Discord', () => {
    const allStylesComponents = [
      {
        type: 1,
        components: [
          { type: 2, style: 1, label: 'Primary', custom_id: 'c1' },
          { type: 2, style: 2, label: 'Secondary', custom_id: 'c2' },
          { type: 2, style: 3, label: 'Success', custom_id: 'c3' },
          { type: 2, style: 4, label: 'Danger', custom_id: 'c4' },
          { type: 2, style: 5, label: 'Link', url: 'https://example.com' },
        ],
      },
    ];

    const container = createMessageComponentsContainer(allStylesComponents, { document: fakeDoc });
    assert.ok(container);
    const buttons = container.querySelectorAll('.component-button');
    assert.equal(buttons.length, 5);

    // Primary
    assert.ok(buttons[0].classList.contains('component-button--primary'));
    assert.equal(buttons[0].getAttribute('data-component-style'), '1');
    assert.equal(buttons[0].getAttribute('data-custom-id'), 'c1');
    assert.equal(buttons[0].textContent, 'Primary');

    // Secondary
    assert.ok(buttons[1].classList.contains('component-button--secondary'));
    assert.equal(buttons[1].getAttribute('data-component-style'), '2');

    // Success (Verify Button)
    assert.ok(buttons[2].classList.contains('component-button--success'));
    assert.equal(buttons[2].getAttribute('data-component-style'), '3');

    // Danger
    assert.ok(buttons[3].classList.contains('component-button--danger'));
    assert.equal(buttons[3].getAttribute('data-component-style'), '4');

    // Link (тег <a>)
    assert.ok(buttons[4].classList.contains('component-button--link'));
    assert.equal(buttons[4].getAttribute('data-component-style'), '5');
    assert.equal(buttons[4].tagName, 'A');
    assert.equal(buttons[4].href, 'https://example.com');
    assert.equal(buttons[4].target, '_blank');
    assert.equal(buttons[4].rel, 'noopener noreferrer');
    assert.ok(buttons[4].querySelector('.component-button-link-icon'), 'Должна быть иконка внешней ссылки');
  });

  it('4. Поддерживает unicode эмодзи и custom Discord эмодзи', () => {
    const emojiComponents = [
      {
        type: 1,
        components: [
          {
            type: 2,
            style: 3,
            label: 'Верификация',
            custom_id: 'v_btn',
            emoji: { name: '🛡️' },
          },
          {
            type: 2,
            style: 1,
            label: 'Discord Guild',
            custom_id: 'custom_emoji_btn',
            emoji: { id: '987654321012345678', name: 'discord_logo', animated: true },
          },
        ],
      },
    ];

    const container = createMessageComponentsContainer(emojiComponents, { document: fakeDoc });
    assert.ok(container);
    const buttons = container.querySelectorAll('.component-button');

    // Unicode emoji
    const unicodeEmojiSpan = buttons[0].querySelector('.component-button-emoji');
    assert.ok(unicodeEmojiSpan, 'Должен быть span для unicode эмодзи');
    assert.equal(unicodeEmojiSpan.textContent, '🛡️');

    // Custom Discord CDN emoji
    const customImg = buttons[1].querySelector('.component-button-custom-emoji');
    assert.ok(customImg, 'Должен быть img для custom эмодзи');
    assert.equal(
      customImg.src,
      'https://cdn.discordapp.com/emojis/987654321012345678.gif?size=48&quality=lossless'
    );
    assert.equal(customImg.alt, ':discord_logo:');
  });

  it('5. Корректно выставляет disabled состояние', () => {
    const disabledComponents = [
      {
        type: 1,
        components: [
          { type: 2, style: 3, label: 'Active', custom_id: 'a1', disabled: false },
          { type: 2, style: 4, label: 'Disabled', custom_id: 'd1', disabled: true },
        ],
      },
    ];

    const container = createMessageComponentsContainer(disabledComponents, { document: fakeDoc });
    assert.ok(container);
    const buttons = container.querySelectorAll('.component-button');

    assert.equal(buttons[0].disabled, false);
    assert.equal(buttons[0].hasAttribute('disabled'), false);

    assert.equal(buttons[1].disabled, true);
    assert.equal(buttons[1].hasAttribute('disabled'), true);
    assert.equal(buttons[1].getAttribute('aria-disabled'), 'true');
  });

  it('6. Обработка клика: вызывает onComponentClick для интерактивных кнопок и игнорирует disabled', () => {
    /** @type {any[]} */
    const clicks = [];
    const container = createMessageComponentsContainer(sampleBotVerificationComponents, {
      document: fakeDoc,
      messageId: 'msg-999',
      channelId: 'chan-123',
      onComponentClick: (btn, comp) => {
        clicks.push({ btn, comp });
      },
    });
    assert.ok(container);

    const buttons = container.querySelectorAll('.component-button');
    // buttons[0] is verify_captcha_button (Success)
    buttons[0].click();
    assert.equal(clicks.length, 1);
    assert.equal(clicks[0].comp.custom_id, 'verify_captcha_button');
    assert.equal(clicks[0].comp.style, 3);

    // buttons[4] is disabled button btn_report
    const disabledBtn = buttons[4];
    assert.equal(disabledBtn.getAttribute('data-custom-id'), 'btn_report');
    disabledBtn.click();
    assert.equal(clicks.length, 1, 'Клик по disabled кнопке не должен вызывать onComponentClick');
  });

  it('7. Интеграция в renderChannelMessage: рендерит кнопки под сообщением и добавляет components в data-extras', () => {
    withChatDoc((doc) => {
      const row = doc.createElement('div');
      const messageItem = {
        id: 'msg-verify-bot-01',
        channelId: 'chan-verify',
        author: 'Wick',
        authorId: 'bot-wick-id',
        text: 'Нажмите кнопку ниже для верификации на сервере.',
        components: sampleBotVerificationComponents,
      };

      renderChannelMessage(/** @type {any} */ (row), /** @type {any} */ (messageItem), 0, 1, {
        channelId: 'chan-verify',
      });

      const extras = row.getAttribute('data-extras') || '';
      assert.ok(extras.includes('components'), `data-extras должен содержать "components": "${extras}"`);

      const compContainer = row.querySelector('.message-components-container');
      assert.ok(compContainer, 'Внутри строки сообщения должен присутствовать .message-components-container');

      const verifyBtn = compContainer.querySelector('[data-custom-id="verify_captcha_button"]');
      assert.ok(verifyBtn, 'Кнопка верификации должна быть отрендерена');
      assert.ok(verifyBtn.classList.contains('component-button--success'));
    });
  });

  it('8. renderMessage в app.js экспортирует и корректно рендерит сообщение с компонентами', () => {
    withChatDoc((doc) => {
      const row = doc.createElement('div');
      const messageItem = {
        id: 'msg-carl-bot-01',
        channelId: 'chan-rules',
        author: 'Carl-bot',
        authorId: 'carl-bot-id',
        text: 'Верифицируйтесь для доступа ко всем каналам.',
        components: sampleBotVerificationComponents,
      };

      const rendered = renderMessage(/** @type {any} */ (row), messageItem, 0, 1, {
        channelId: 'chan-rules',
      });

      const targetRow = rendered || row;
      const compContainer = targetRow.querySelector('.message-components-container');
      assert.ok(compContainer, 'renderMessage должен рендерить компоненты сообщения');
      assert.equal(compContainer.querySelectorAll('.component-button').length, 5);
    });
  });

  it('9. Файл message-components.css существует и содержит стили для ActionRow и всех типов кнопок', () => {
    const cssPath = fileURLToPath(
      new URL('../../rust/gui-spike/ui/components/message-components.css', import.meta.url)
    );
    assert.ok(existsSync(cssPath), 'message-components.css должен существовать');
    const cssContent = readFileSync(cssPath, 'utf8');

    assert.ok(cssContent.includes('.message-components-container'), 'Должен содержать .message-components-container');
    assert.ok(cssContent.includes('.message-action-row'), 'Должен содержать .message-action-row');
    assert.ok(cssContent.includes('.component-button'), 'Должен содержать .component-button');
    assert.ok(cssContent.includes('.component-button--primary'), 'Должен содержать .component-button--primary');
    assert.ok(cssContent.includes('.component-button--secondary'), 'Должен содержать .component-button--secondary');
    assert.ok(cssContent.includes('.component-button--success'), 'Должен содержать .component-button--success');
    assert.ok(cssContent.includes('.component-button--danger'), 'Должен содержать .component-button--danger');
    assert.ok(cssContent.includes('.component-button--link'), 'Должен содержать .component-button--link');

    // Проверка токенов и интерактивных состояний из Kanban:
    // Verify button: default: bg: #23a55a, hover: opacity: 0.85, active: scale: 0.98
    assert.ok(cssContent.includes('#23a55a'), 'Success кнопка должна использовать #23a55a');
    assert.ok(cssContent.includes('#5865f2'), 'Primary кнопка должна использовать #5865f2');
    assert.ok(cssContent.includes('#4e5058'), 'Secondary кнопка должна использовать #4e5058');
    assert.ok(cssContent.includes('#da373c'), 'Danger кнопка должна использовать #da373c');
    assert.ok(cssContent.includes('0.85'), 'Hover состояние должно содержать opacity 0.85');
    assert.ok(cssContent.includes('0.98'), 'Active состояние должно содержать scale(0.98)');
  });
});
