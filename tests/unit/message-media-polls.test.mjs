// @ts-check
/**
 * T-195 (RW-40 / G2): unit-тесты рендера вложений сообщений и виджета опросов
 * (`rust/gui-spike/ui/features/message-media-polls.js`).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  isSafeAttachmentUrl,
  renderMessageAttachments,
  createPollWidget,
} from '../../rust/gui-spike/ui/features/message-media-polls.js';

/* ------------------------------------------------------------------ *
 * Минимальный DOM-шим для изолированных unit-тестов
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
  }

  /** @returns {boolean} */
  get disabled() {
    return this._disabled || this._attrs.has('disabled');
  }

  /** @param {boolean} value */
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
      return this.children.map((c) => c.textContent).join('');
    }
    return this._text;
  }

  /** @param {string} value */
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
    if (name === 'disabled') this._disabled = true;
  }

  /**
   * @param {string} name
   * @returns {string | null}
   */
  getAttribute(name) {
    return this._attrs.has(name) ? (this._attrs.get(name) ?? null) : null;
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  hasAttribute(name) {
    return this._attrs.has(name);
  }

  /** @param {string} name */
  removeAttribute(name) {
    this._attrs.delete(name);
    if (name === 'disabled') this._disabled = false;
  }

  /** @param {FakeElement} child */
  appendChild(child) {
    if (child.parentNode) {
      child.parentNode.removeChild(child);
    }
    child.parentNode = this;
    if (!child.ownerDocument && this.ownerDocument) {
      child.ownerDocument = this.ownerDocument;
    }
    this.children.push(child);
    return child;
  }

  /** @param {FakeElement} child */
  removeChild(child) {
    this.children = this.children.filter((c) => c !== child);
    child.parentNode = null;
    return child;
  }

  /** @param {...FakeElement} nodes */
  replaceChildren(...nodes) {
    for (const child of this.children) {
      child.parentNode = null;
    }
    this.children = [];
    this._text = '';
    for (const node of nodes) {
      this.appendChild(node);
    }
  }

  /**
   * @param {string} type
   * @param {(event: any) => void} listener
   */
  addEventListener(type, listener) {
    if (!this._listeners.has(type)) {
      this._listeners.set(type, []);
    }
    this._listeners.get(type)?.push(listener);
  }

  /** @param {string} type */
  dispatchEvent(type) {
    const event = {
      type,
      target: this,
      currentTarget: this,
      defaultPrevented: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {},
    };
    for (const listener of this._listeners.get(type) ?? []) {
      listener(event);
    }
    return !event.defaultPrevented;
  }

  click() {
    if (this.disabled) return false;
    return this.dispatchEvent('click');
  }

  /**
   * @param {string} selector
   * @returns {FakeElement | null}
   */
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] ?? null;
  }

  /**
   * @param {string} selector
   * @returns {FakeElement[]}
   */
  querySelectorAll(selector) {
    /** @type {FakeElement[]} */
    const out = [];
    /** @param {FakeElement} el */
    const walk = (el) => {
      for (const child of el.children) {
        if (matchesSelector(child, selector)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
}

/**
 * @param {FakeElement} el
 * @param {string} selector
 * @returns {boolean}
 */
function matchesSelector(el, selector) {
  const tokens = selector.match(/\[[^\]]+\]|[#.][\w-]+|[a-zA-Z][\w-]*/g) ?? [];
  if (tokens.length === 0) return false;
  return tokens.every((token) => {
    if (token.startsWith('#')) return el.getAttribute('id') === token.slice(1);
    if (token.startsWith('.')) {
      const classes = (el.getAttribute('class') ?? '').split(/\s+/).filter(Boolean);
      return classes.includes(token.slice(1));
    }
    if (token.startsWith('[')) {
      const m = token.slice(1, -1).match(/^([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?$/);
      if (!m || !m[1]) return false;
      const attrName = m[1];
      const expected = m[2] ?? m[3] ?? m[4];
      if (expected === undefined) return el.hasAttribute(attrName);
      return el.getAttribute(attrName) === expected;
    }
    return el.tagName === token.toUpperCase();
  });
}

function makeContainer() {
  const doc = {
    /** @param {string} tag */
    createElement(tag) {
      return new FakeElement(tag, doc);
    },
  };
  return new FakeElement('div', doc);
}

/* ------------------------------------------------------------------ *
 * Валидация безопасных URL и рендер вложений
 * ------------------------------------------------------------------ */

describe('isSafeAttachmentUrl', () => {
  it('пропускает только https://, blob: и asset: URL', () => {
    assert.equal(isSafeAttachmentUrl('https://cdn.discordapp.com/attachments/1/2/cat.png'), true);
    assert.equal(isSafeAttachmentUrl('HTTPS://cdn.discordapp.com/file.mp4'), true);
    assert.equal(isSafeAttachmentUrl('blob:https://discord.com/550e8400-e29b-41d4-a716-446655440000'), true);
    assert.equal(isSafeAttachmentUrl('asset://localhost/path/to/image.webp'), true);
    assert.equal(isSafeAttachmentUrl('asset:local-preview-1'), true);
  });

  it('отклоняет javascript:, data:, http:// и пустые значения', () => {
    assert.equal(isSafeAttachmentUrl('javascript:alert(1)'), false);
    assert.equal(isSafeAttachmentUrl('  JavaScript:alert(document.cookie)'), false);
    assert.equal(isSafeAttachmentUrl('data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg=='), false);
    assert.equal(isSafeAttachmentUrl('vbscript:msgbox(1)'), false);
    assert.equal(isSafeAttachmentUrl('http://cdn.discordapp.com/insecure.png'), false);
    assert.equal(isSafeAttachmentUrl(''), false);
    assert.equal(isSafeAttachmentUrl('   '), false);
    assert.equal(isSafeAttachmentUrl(null), false);
    assert.equal(isSafeAttachmentUrl(undefined), false);
  });
});

describe('renderMessageAttachments', () => {
  it('рендерит карточки с data-attachment-id, data-attachment-kind и русским размером через formatFileSizeRu', () => {
    const container = makeContainer();
    const cards = renderMessageAttachments(container, [
      { id: 'att-1', filename: 'photo.png', size: 512, url: 'https://cdn.discordapp.com/photo.png' },
      { id: 'att-2', filename: 'clip.mp4', size: 1536, url: 'blob:https://discord.com/clip-1' },
      { id: 'att-3', filename: 'voice.ogg', size: 2 * 1024 * 1024, url: 'asset://localhost/voice.ogg' },
      { id: 'att-4', filename: 'archive.zip', size: 0, url: 'https://cdn.discordapp.com/archive.zip' },
    ]);

    assert.equal(cards.length, 4);
    const rendered = container.querySelectorAll('[data-attachment-id]');
    assert.equal(rendered.length, 4);

    assert.equal(rendered[0]?.getAttribute('data-attachment-id'), 'att-1');
    assert.equal(rendered[0]?.getAttribute('data-attachment-kind'), 'image');
    assert.equal(rendered[0]?.querySelector('[data-attachment-size]')?.textContent, '512 Б');
    assert.equal(rendered[0]?.querySelector('img')?.getAttribute('src'), 'https://cdn.discordapp.com/photo.png');

    assert.equal(rendered[1]?.getAttribute('data-attachment-id'), 'att-2');
    assert.equal(rendered[1]?.getAttribute('data-attachment-kind'), 'video');
    assert.equal(rendered[1]?.querySelector('[data-attachment-size]')?.textContent, '1.5 КБ');
    assert.equal(rendered[1]?.querySelector('video')?.getAttribute('src'), 'blob:https://discord.com/clip-1');

    assert.equal(rendered[2]?.getAttribute('data-attachment-id'), 'att-3');
    assert.equal(rendered[2]?.getAttribute('data-attachment-kind'), 'audio');
    assert.equal(rendered[2]?.querySelector('[data-attachment-size]')?.textContent, '2.00 МБ');
    assert.equal(rendered[2]?.querySelector('audio')?.getAttribute('src'), 'asset://localhost/voice.ogg');

    assert.equal(rendered[3]?.getAttribute('data-attachment-id'), 'att-4');
    assert.equal(rendered[3]?.getAttribute('data-attachment-kind'), 'file');
    assert.equal(rendered[3]?.querySelector('[data-attachment-size]')?.textContent, '0 Б');
  });

  it('определяет data-attachment-kind по contentType при расширении общего вида', () => {
    const container = makeContainer();
    renderMessageAttachments(container, [
      { id: 'mime-img', filename: 'upload.bin', contentType: 'image/webp', size: 1024, url: 'https://cdn.discordapp.com/u.bin' },
    ]);
    const card = container.querySelector('[data-attachment-id="mime-img"]');
    assert.equal(card?.getAttribute('data-attachment-kind'), 'image');
    assert.equal(card?.querySelector('[data-attachment-size]')?.textContent, '1.0 КБ');
  });

  it('блокирует небезопасные javascript: и data: URL и не вызывает onDownload для них', () => {
    const container = makeContainer();
    /** @type {any[]} */
    const downloaded = [];
    renderMessageAttachments(
      container,
      [
        { id: 'bad-js', filename: 'exploit.png', size: 100, url: 'javascript:alert(1)' },
        { id: 'bad-data', filename: 'inline.png', size: 200, url: 'data:image/png;base64,AAAA' },
        { id: 'good-https', filename: 'safe.pdf', size: 300, url: 'https://cdn.discordapp.com/safe.pdf' },
      ],
      { onDownload: (att) => downloaded.push(att) },
    );

    const jsCard = container.querySelector('[data-attachment-id="bad-js"]');
    const dataCard = container.querySelector('[data-attachment-id="bad-data"]');
    const safeCard = container.querySelector('[data-attachment-id="good-https"]');

    assert.equal(jsCard?.getAttribute('data-url-safe'), 'false');
    assert.equal(jsCard?.querySelector('img'), null);
    assert.equal(jsCard?.getAttribute('data-attachment-url'), null);
    jsCard?.querySelector('[data-action="download-attachment"]')?.click();
    assert.equal(downloaded.length, 0);

    assert.equal(dataCard?.getAttribute('data-url-safe'), 'false');
    assert.equal(dataCard?.querySelector('img'), null);
    dataCard?.querySelector('[data-action="download-attachment"]')?.click();
    assert.equal(downloaded.length, 0);

    assert.equal(safeCard?.getAttribute('data-url-safe'), 'true');
    assert.equal(safeCard?.getAttribute('data-attachment-url'), 'https://cdn.discordapp.com/safe.pdf');
    safeCard?.querySelector('[data-action="download-attachment"]')?.click();
    assert.equal(downloaded.length, 1);
    assert.equal(downloaded[0]?.id, 'good-https');
  });

  it('выставляет data-spoiler="hidden", aria-expanded="false" и бейдж «СПОЙЛЕР» для spoiler=true и SPOILER_ файла, раскрывая по клику', () => {
    const container = makeContainer();
    renderMessageAttachments(container, [
      { id: 'sp-flag', filename: 'secret.png', size: 2048, spoiler: true, url: 'https://cdn.discordapp.com/secret.png' },
      { id: 'sp-prefix', filename: 'SPOILER_plot.mp4', size: 4096, url: 'https://cdn.discordapp.com/plot.mp4' },
      { id: 'normal', filename: 'public.png', size: 1024, url: 'https://cdn.discordapp.com/public.png' },
    ]);

    const flagCard = container.querySelector('[data-attachment-id="sp-flag"]');
    const prefixCard = container.querySelector('[data-attachment-id="sp-prefix"]');
    const normalCard = container.querySelector('[data-attachment-id="normal"]');

    assert.equal(flagCard?.getAttribute('data-spoiler'), 'hidden');
    assert.equal(flagCard?.getAttribute('aria-expanded'), 'false');
    assert.equal(flagCard?.querySelector('[data-spoiler-badge]')?.textContent, 'СПОЙЛЕР');

    assert.equal(prefixCard?.getAttribute('data-spoiler'), 'hidden');
    assert.equal(prefixCard?.getAttribute('aria-expanded'), 'false');
    assert.equal(prefixCard?.querySelector('[data-spoiler-badge]')?.textContent, 'СПОЙЛЕР');

    assert.equal(normalCard?.getAttribute('data-spoiler'), null);
    assert.equal(normalCard?.querySelector('[data-spoiler-badge]'), null);

    // Клик по карточке-спойлеру раскрывает её
    flagCard?.click();
    assert.equal(flagCard?.getAttribute('data-spoiler'), 'revealed');
    assert.equal(flagCard?.getAttribute('aria-expanded'), 'true');

    // Повторный клик снова скрывает спойлер
    flagCard?.click();
    assert.equal(flagCard?.getAttribute('data-spoiler'), 'hidden');
    assert.equal(flagCard?.getAttribute('aria-expanded'), 'false');

    // Клик по бейджу также переключает состояние спойлера
    prefixCard?.querySelector('[data-spoiler-badge]')?.click();
    assert.equal(prefixCard?.getAttribute('data-spoiler'), 'revealed');
    assert.equal(prefixCard?.getAttribute('aria-expanded'), 'true');
  });
});

/* ------------------------------------------------------------------ *
 * Виджет опросов (createPollWidget)
 * ------------------------------------------------------------------ */

describe('createPollWidget', () => {
  const nowMs = 1_700_000_000_000;

  it('рендерит вопрос, варианты с голосами и процентами, подсветку лидера (data-leading="true") и футер истечения', () => {
    const container = makeContainer();
    createPollWidget(
      container,
      {
        question: 'Какой стек выбираем?',
        answers: [
          { answerId: 'a1', text: 'Rust + Tauri' },
          { answerId: 'a2', text: 'Electron' },
          { answerId: 'a3', text: 'Qt' },
        ],
        counts: [
          { id: 'a1', count: 3, meVoted: false },
          { id: 'a2', count: 1, meVoted: false },
          { id: 'a3', count: 0, meVoted: false },
        ],
        expiryMs: nowMs + 2 * 3_600_000,
      },
      { nowMs },
    );

    assert.equal(container.querySelector('[data-poll-question]')?.textContent, 'Какой стек выбираем?');

    const options = container.querySelectorAll('[data-poll-answer]');
    assert.equal(options.length, 3);

    assert.equal(options[0]?.getAttribute('data-answer-id'), 'a1');
    assert.equal(options[0]?.getAttribute('data-leading'), 'true');
    assert.equal(options[0]?.querySelector('[data-poll-answer-text]')?.textContent, 'Rust + Tauri');
    assert.equal(options[0]?.querySelector('[data-poll-answer-count]')?.textContent, '3');
    assert.equal(options[0]?.querySelector('[data-poll-answer-percentage]')?.textContent, '75%');

    assert.equal(options[1]?.getAttribute('data-answer-id'), 'a2');
    assert.equal(options[1]?.getAttribute('data-leading'), 'false');
    assert.equal(options[1]?.querySelector('[data-poll-answer-count]')?.textContent, '1');
    assert.equal(options[1]?.querySelector('[data-poll-answer-percentage]')?.textContent, '25%');

    assert.equal(options[2]?.getAttribute('data-answer-id'), 'a3');
    assert.equal(options[2]?.getAttribute('data-leading'), 'false');
    assert.equal(options[2]?.querySelector('[data-poll-answer-count]')?.textContent, '0');
    assert.equal(options[2]?.querySelector('[data-poll-answer-percentage]')?.textContent, '0%');

    assert.equal(container.querySelector('[data-poll-expiry]')?.textContent, 'Осталось 2 ч.');
    const submitBtn = container.querySelector('[data-poll-submit]');
    assert.equal(submitBtn?.textContent, 'Голосовать');
    assert.equal(submitBtn?.disabled, true);
  });

  it('при нуле голосов ни один вариант не помечается data-leading="true"', () => {
    const container = makeContainer();
    createPollWidget(
      container,
      {
        question: 'Новый опрос',
        answers: [
          { answerId: 'a1', text: 'Вариант 1' },
          { answerId: 'a2', text: 'Вариант 2' },
        ],
        counts: [],
        expiryMs: nowMs + 30 * 60_000,
      },
      { nowMs },
    );

    const options = container.querySelectorAll('[data-poll-answer]');
    assert.deepEqual(
      options.map((o) => o.getAttribute('data-leading')),
      ['false', 'false'],
    );
    assert.equal(container.querySelector('[data-poll-expiry]')?.textContent, 'Осталось 30 мин.');
  });

  it('поддерживает одиночный выбор (single-select) и отправляет голос через кнопку «Голосовать»', () => {
    const container = makeContainer();
    /** @type {string[][]} */
    const votes = [];

    createPollWidget(
      container,
      {
        question: 'Одиночный выбор',
        answers: [
          { answerId: 'a1', text: 'Первый' },
          { answerId: 'a2', text: 'Второй' },
        ],
        counts: [],
        allowMultiselect: false,
        expiryMs: nowMs + 10 * 60_000,
      },
      {
        nowMs,
        onVote: (selected) => votes.push(selected),
      },
    );

    const opt1 = container.querySelector('[data-answer-id="a1"]');
    const opt2 = container.querySelector('[data-answer-id="a2"]');
    const submitBtn = container.querySelector('[data-poll-submit]');

    assert.equal(submitBtn?.disabled, true);
    submitBtn?.click();
    assert.equal(votes.length, 0);

    // Выбираем первый вариант
    opt1?.click();
    assert.equal(opt1?.getAttribute('data-selected'), 'true');
    assert.equal(opt1?.getAttribute('aria-checked'), 'true');
    assert.equal(opt2?.getAttribute('data-selected'), 'false');
    assert.equal(submitBtn?.disabled, false);

    // Переключаемся на второй вариант (радио-режим)
    opt2?.click();
    assert.equal(opt1?.getAttribute('data-selected'), 'false');
    assert.equal(opt2?.getAttribute('data-selected'), 'true');
    assert.equal(submitBtn?.disabled, false);

    // Повторный клик снимает выбор и блокирует кнопку «Голосовать»
    opt2?.click();
    assert.equal(opt2?.getAttribute('data-selected'), 'false');
    assert.equal(submitBtn?.disabled, true);

    // Снова выбираем a1 и нажимаем «Голосовать»
    opt1?.click();
    submitBtn?.click();
    assert.deepEqual(votes, [['a1']]);
  });

  it('поддерживает мультивыбор (multiselect) с учётом maxChoices', () => {
    const container = makeContainer();
    /** @type {string[][]} */
    const votes = [];

    createPollWidget(
      container,
      {
        question: 'Мультивыбор (до 2)',
        answers: [
          { answerId: 'a1', text: 'Один' },
          { answerId: 'a2', text: 'Два' },
          { answerId: 'a3', text: 'Три' },
        ],
        counts: [],
        allowMultiselect: true,
        maxChoices: 2,
        expiryMs: nowMs + 86_400_000,
      },
      {
        nowMs,
        onVote: (selected) => votes.push(selected),
      },
    );

    const opt1 = container.querySelector('[data-answer-id="a1"]');
    const opt2 = container.querySelector('[data-answer-id="a2"]');
    const opt3 = container.querySelector('[data-answer-id="a3"]');
    const submitBtn = container.querySelector('[data-poll-submit]');

    opt1?.click();
    opt2?.click();
    // Попытка выбрать третий сверх maxChoices=2 игнорируется
    opt3?.click();

    assert.equal(opt1?.getAttribute('data-selected'), 'true');
    assert.equal(opt2?.getAttribute('data-selected'), 'true');
    assert.equal(opt3?.getAttribute('data-selected'), 'false');

    submitBtn?.click();
    assert.deepEqual(votes, [['a1', 'a2']]);
  });

  it('блокирует выбор и кнопку «Голосовать», когда опрос завершён (expired)', () => {
    const container = makeContainer();
    /** @type {string[][]} */
    const votes = [];

    createPollWidget(
      container,
      {
        question: 'Завершённый опрос',
        answers: [
          { answerId: 'a1', text: 'Да' },
          { answerId: 'a2', text: 'Нет' },
        ],
        counts: [{ id: 'a1', count: 5 }],
        selectedAnswerIds: ['a1'],
        expiryMs: nowMs - 1000,
      },
      {
        nowMs,
        onVote: (selected) => votes.push(selected),
      },
    );

    assert.equal(container.querySelector('[data-poll-expiry]')?.textContent, 'Опрос завершён');
    const submitBtn = container.querySelector('[data-poll-submit]');
    assert.equal(submitBtn?.disabled, true);

    const opt2 = container.querySelector('[data-answer-id="a2"]');
    opt2?.click();
    assert.equal(opt2?.getAttribute('data-selected'), 'false');

    submitBtn?.click();
    assert.equal(votes.length, 0);
  });
});
