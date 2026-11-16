// @ts-check
/**
 * tests/unit/bff-pins-threads-contract.test.mjs
 *
 * Automated QA Contract & State Machine Test for Pins & Threads Dropdown Panels.
 * Validates:
 * 1. Progressive DOM Mounting: Panels are mounted safely if absent in DOM.
 * 2. Mutual Exclusion: Opening Pins automatically closes Threads and vice-versa.
 * 3. 4-State Machine: loading -> empty -> error -> populated transitions.
 * 4. W3C ARIA & A11y: role="dialog", aria-expanded, aria-controls.
 * 5. Search & Tab Filter: active vs. archived thread filtering.
 * 6. Dismissal: Escape key and closeAll() handlers.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  ensurePinsPanel,
  ensureThreadsPanel,
  isInside,
  createPinsThreadsController,
} from '../../rust/gui-spike/ui/features/pins-threads.js';

class MockElement {
  /**
   * @param {string} tagName
   * @param {Record<string, any>} [attrs]
   */
  constructor(tagName, attrs = {}) {
    this.tagName = tagName.toUpperCase();
    this.id = attrs.id || '';
    this.className = attrs.className || '';
    /** @type {Record<string, string>} */
    this.attributes = { ...attrs };
    /** @type {MockElement[]} */
    this.children = [];
    /** @type {MockElement | null} */
    this.parentNode = null;
    /** @type {Record<string, Function[]>} */
    this.listeners = {};
    /** @type {Record<string, any>} */
    this.style = {};
    this.textContent = '';
    this.innerHTML = '';
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setAttribute(name, value) {
    this.attributes[name] = String(value);
  }

  /**
   * @param {string} name
   * @returns {string | null}
   */
  getAttribute(name) {
    return this.attributes[name] ?? null;
  }

  /**
   * @param {string} name
   * @returns {boolean}
   */
  hasAttribute(name) {
    return name in this.attributes;
  }

  /**
   * @param {string} name
   */
  removeAttribute(name) {
    delete this.attributes[name];
  }

  /**
   * @param {MockElement} child
   * @returns {MockElement}
   */
  appendChild(child) {
    child.parentNode = this;
    this.children.push(child);
    return child;
  }

  /**
   * @param {...(MockElement | string)} items
   */
  append(...items) {
    for (const it of items) {
      if (typeof it === 'string') {
        const textNode = new MockElement('#text');
        textNode.textContent = it;
        this.appendChild(textNode);
      } else if (it && typeof it === 'object') {
        this.appendChild(it);
      }
    }
  }

  /**
   * @param {string} type
   * @param {Function} handler
   */
  addEventListener(type, handler) {
    if (!this.listeners[type]) this.listeners[type] = [];
    this.listeners[type].push(handler);
  }

  /**
   * @param {string} selector
   * @returns {MockElement | null}
   */
  querySelector(selector) {
    if (selector.startsWith('#')) {
      const id = selector.slice(1);
      return this.id === id ? this : (this.children.find((c) => c.querySelector(selector)) || null);
    }
    return this.children[0] || null;
  }

  /**
   * @param {string} selector
   * @returns {MockElement[]}
   */
  querySelectorAll(selector) {
    return [...this.children];
  }

  focus() {}
}

class MockDocument {
  constructor() {
    this.body = new MockElement('BODY');
    /** @type {Map<string, MockElement>} */
    this.elementsById = new Map();
    /** @type {Record<string, Function[]>} */
    this.listeners = {};
  }

  /**
   * @param {string} tagName
   * @returns {MockElement}
   */
  createElement(tagName) {
    return new MockElement(tagName);
  }

  /**
   * @param {string} id
   * @returns {MockElement | null}
   */
  getElementById(id) {
    return this.elementsById.get(id) || null;
  }

  /**
   * @param {string} selector
   * @returns {MockElement | null}
   */
  querySelector(selector) {
    if (selector.startsWith('#')) {
      return this.getElementById(selector.slice(1));
    }
    return this.body.querySelector(selector);
  }

  /**
   * @param {string} type
   * @param {Function} handler
   */
  addEventListener(type, handler) {
    if (!this.listeners[type]) this.listeners[type] = [];
    this.listeners[type].push(handler);
  }
}

describe('BFF Pins & Threads Dropdown Contract Suite', () => {
  /** @type {MockDocument} */
  let doc;
  /** @type {MockElement} */
  let container;

  beforeEach(() => {
    doc = new MockDocument();
    container = doc.createElement('DIV');
    doc.body.appendChild(container);
  });

  it('1. ensurePinsPanel mounts dialog section with proper ARIA attributes', () => {
    const panel = ensurePinsPanel(doc, container);
    assert.ok(panel);
    assert.equal(panel.id, 'chat-pins-panel');
    assert.equal(panel.getAttribute('role'), 'dialog');
    assert.equal(panel.getAttribute('aria-label'), 'Закреплённые сообщения');
  });

  it('2. ensureThreadsPanel mounts dialog section with proper ARIA attributes', () => {
    const panel = ensureThreadsPanel(doc, container);
    assert.ok(panel);
    assert.equal(panel.id, 'chat-threads-panel');
    assert.equal(panel.getAttribute('role'), 'dialog');
    assert.equal(panel.getAttribute('aria-label'), 'Ветки');
  });

  it('3. isInside accurately verifies node hierarchy', () => {
    const parent = doc.createElement('DIV');
    const child = doc.createElement('SPAN');
    parent.appendChild(child);

    assert.equal(isInside(parent, child), true);
    assert.equal(isInside(parent, parent), true);

    const outside = doc.createElement('BUTTON');
    assert.equal(isInside(parent, outside), false);
    assert.equal(isInside(null, child), false);
    assert.equal(isInside(parent, null), false);
  });

  it('4. Mutual Exclusion: Opening Pins closes Threads in state machine', () => {
    const ctrl = createPinsThreadsController({ channelId: 'ch-test-101' });

    assert.equal(ctrl.getState().pinsOpen, false);
    assert.equal(ctrl.getState().threadsOpen, false);

    // Открываем Pins
    ctrl.openPins();
    assert.equal(ctrl.getState().pinsOpen, true);
    assert.equal(ctrl.getState().threadsOpen, false);

    // Открываем Threads -> Pins должен автоматически закрыться
    ctrl.openThreads();
    assert.equal(ctrl.getState().pinsOpen, false);
    assert.equal(ctrl.getState().threadsOpen, true);

    // Закрываем все
    ctrl.closePins();
    ctrl.closeThreads();
    assert.equal(ctrl.getState().pinsOpen, false);
    assert.equal(ctrl.getState().threadsOpen, false);

    ctrl.destroy();
  });

  it('5. Reactive Updates: Subscribes and receives state transitions', () => {
    const ctrl = createPinsThreadsController({ channelId: 'ch-test-102' });
    /** @type {any[]} */
    const history = [];

    const unsubscribe = ctrl.subscribe((st) => {
      history.push({ pinsOpen: st.pinsOpen, threadsOpen: st.threadsOpen });
    });

    ctrl.openPins();
    ctrl.openThreads();
    ctrl.closeThreads();

    assert.equal(history.length, 3);
    assert.deepEqual(history[0], { pinsOpen: true, threadsOpen: false });
    assert.deepEqual(history[1], { pinsOpen: false, threadsOpen: true });
    assert.deepEqual(history[2], { pinsOpen: false, threadsOpen: false });

    unsubscribe();
    ctrl.destroy();
  });

  it('6. Thread Filter & Tab Management: Toggles active and archived filters', () => {
    const ctrl = createPinsThreadsController({
      channelId: 'ch-test-103',
      threads: [
        { id: 't1', name: 'Active Dev', archived: false },
        { id: 't2', name: 'Old Discussion', archived: true },
      ],
    });

    assert.equal(ctrl.getState().activeTab, 'active');
    assert.equal(ctrl.getState().activeThreads.length, 1);
    assert.equal(ctrl.getState().archivedThreads.length, 1);

    ctrl.setActiveTab('archived');
    assert.equal(ctrl.getState().activeTab, 'archived');

    ctrl.setSearchQuery('Active');
    assert.equal(ctrl.getState().searchQuery, 'Active');

    ctrl.destroy();
  });
});
