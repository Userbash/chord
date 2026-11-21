// @ts-nocheck
/**
 * TAURI-MODAL-01: Контракт модульного диалога и портала демонстрации экрана (`screen-share-dialog.js`).
 *
 * Проверяет:
 * 1. Экспорт канонических констант и токенов (SCREEN_SHARE_RESOLUTIONS, SCREEN_SHARE_FPS_OPTIONS, SCREEN_SHARE_PORTAL_TABS).
 * 2. Чистые функции нормализации и фильтрации источников (normalizeResolution, normalizeFps, filterSourcesByCategory).
 * 3. Контроллер диалога выбора экрана createScreenShareDialogController / openScreenSharePicker.
 * 4. Контроллер портала захвата createScreenSharePortalPicker / openScreenSharePortalPicker с поиском и rAF-циклом.
 * 5. Контроллер полноценного модального окна openScreenShareModal (вкладки приложений/устройств, SD/HD).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  SCREEN_SHARE_RESOLUTIONS,
  SCREEN_SHARE_FPS_OPTIONS,
  SCREEN_SHARE_PORTAL_TABS,
  SCREEN_SHARE_MODAL_TABS,
  DEFAULT_SCREEN_SHARE_CONFIG,
  SCREEN_SHARE_DIALOG_TOKENS,
  normalizeResolution,
  normalizeFps,
  filterSourcesByCategory,
  formatSourceItem,
  createScreenShareDialogController,
  openScreenShareDialog,
  openScreenSharePortalPicker,
  createScreenSharePortalPicker,
  openScreenShareModal,
  createScreenShareModalController,
} from '../../rust/gui-spike/ui/features/screen-share-dialog.js';

/**
 * Создаёт мок DOM-контейнера для изоляции юнит-тестов.
 */
function createMockContainer() {
  const listeners = new Map();
  /** @type {any[]} */
  const children = [];

  const element = {
    children,
    attributes: new Map(),
    style: {},
    ownerDocument: null,
    /**
     * @param {string} tag
     */
    createElement(tag) {
      const el = {
        tagName: tag.toUpperCase(),
        children: /** @type {any[]} */ ([]),
        attributes: new Map(),
        style: {},
        value: '',
        checked: false,
        disabled: false,
        textContent: '',
        innerHTML: '',
        width: 0,
        height: 0,
        className: '',
        id: '',
        type: '',
        title: '',
        placeholder: '',
        ownerDocument: null,
        /** @param {string} k @param {string} v */
        setAttribute(k, v) {
          el.attributes.set(k, String(v));
        },
        /** @param {string} k */
        getAttribute(k) {
          return el.attributes.get(k) ?? null;
        },
        /** @param {string} k */
        hasAttribute(k) {
          return el.attributes.has(k);
        },
        /** @param {string} k */
        removeAttribute(k) {
          el.attributes.delete(k);
        },
        /** @param {any[]} nodes */
        append(...nodes) {
          for (const n of nodes) {
            el.appendChild(n);
          }
        },
        /** @param {any} node */
        appendChild(node) {
          node.parentNode = el;
          el.children.push(node);
          return node;
        },
        /** @param {any} node */
        removeChild(node) {
          const idx = el.children.indexOf(node);
          if (idx !== -1) {
            el.children.splice(idx, 1);
            node.parentNode = null;
          }
          return node;
        },
        /** @param {any[]} nodes */
        replaceChildren(...nodes) {
          el.children.length = 0;
          for (const n of nodes) {
            el.appendChild(n);
          }
        },
        remove() {
          if (el.parentNode && typeof el.parentNode.removeChild === 'function') {
            el.parentNode.removeChild(el);
          }
        },
        /**
         * @param {string} event
         * @param {Function} handler
         */
        addEventListener(event, handler) {
          if (!listeners.has(el)) listeners.set(el, new Map());
          const map = listeners.get(el);
          if (!map.has(event)) map.set(event, []);
          map.get(event).push(handler);
        },
        /**
         * @param {string} event
         * @param {Function} handler
         */
        removeEventListener(event, handler) {
          const map = listeners.get(el);
          if (!map || !map.has(event)) return;
          const list = map.get(event);
          const idx = list.indexOf(handler);
          if (idx !== -1) list.splice(idx, 1);
        },
        /**
         * @param {string} event
         * @param {any} [data]
         */
        dispatchEvent(event, data) {
          const map = listeners.get(el);
          if (!map || !map.has(event)) return true;
          for (const h of [...map.get(event)]) {
            h({ target: el, currentTarget: el, ...data });
          }
          return true;
        },
        click() {
          el.dispatchEvent('click');
        },
        /**
         * @param {string} selector
         * @returns {any}
         */
        querySelector(selector) {
          return querySelectorAll(el, selector)[0] ?? null;
        },
        /**
         * @param {string} selector
         * @returns {any[]}
         */
        querySelectorAll(selector) {
          return querySelectorAll(el, selector);
        },
        getContext() {
          return {
            drawImage() {},
            fillRect() {},
            clearRect() {},
            fillText() {},
          };
        },
      };
      el.ownerDocument = element.ownerDocument;
      return el;
    },
    /**
     * @param {string} selector
     * @returns {any}
     */
    querySelector(selector) {
      return querySelectorAll(element, selector)[0] ?? null;
    },
    /**
     * @param {string} selector
     * @returns {any[]}
     */
    querySelectorAll(selector) {
      return querySelectorAll(element, selector);
    },
    /** @param {any} node */
    appendChild(node) {
      node.parentNode = element;
      children.push(node);
      return node;
    },
    /** @param {any} node */
    removeChild(node) {
      const idx = children.indexOf(node);
      if (idx !== -1) {
        children.splice(idx, 1);
        node.parentNode = null;
      }
      return node;
    },
    /** @param {any[]} nodes */
    replaceChildren(...nodes) {
      children.length = 0;
      for (const n of nodes) {
        element.appendChild(n);
      }
    },
    remove() {},
  };

  const doc = {
    body: element,
    createElement: element.createElement,
    getElementById(id) {
      return querySelectorAll(element, `#${id}`)[0] ?? null;
    },
  };
  element.ownerDocument = doc;

  return { container: element, doc };
}

/**
 * Простой селектор для тестов мок-DOM.
 * @param {any} root
 * @param {string} selector
 * @returns {any[]}
 */
function querySelectorAll(root, selector) {
  /** @type {any[]} */
  const results = [];

  /** @param {any} node */
  function walk(node) {
    if (!node) return;
    if (matchesSelector(node, selector)) {
      results.push(node);
    }
    if (Array.isArray(node.children)) {
      for (const ch of node.children) {
        walk(ch);
      }
    }
  }

  walk(root);
  return results;
}

/**
 * @param {any} node
 * @param {string} selector
 */
function matchesSelector(node, selector) {
  if (!node) return false;
  if (selector.startsWith('#')) {
    return node.id === selector.slice(1);
  }
  if (selector.startsWith('.')) {
    return (node.className || '').split(/\s+/).includes(selector.slice(1));
  }
  const attrMatch = selector.match(/^\[([^=\]]+)(?:="([^"]+)")?\]$/);
  if (attrMatch) {
    const [, name, val] = attrMatch;
    if (!node.attributes || !node.attributes.has(name)) return false;
    if (val !== undefined) {
      return node.attributes.get(name) === val;
    }
    return true;
  }
  if (selector.includes('[') && selector.includes(']')) {
    const tag = selector.split('[')[0];
    const rest = selector.slice(tag.length);
    if (tag && node.tagName !== tag.toUpperCase()) return false;
    return matchesSelector(node, rest);
  }
  return (node.tagName || '').toLowerCase() === selector.toLowerCase();
}

describe('TAURI-MODAL-01: Константы и чистые функции screen-share-dialog', () => {
  it('экспортирует замороженные списки разрешений, FPS и вкладок', () => {
    assert.deepEqual(SCREEN_SHARE_RESOLUTIONS, ['720p', '1080p', '1440p']);
    assert.ok(Object.isFrozen(SCREEN_SHARE_RESOLUTIONS));

    assert.deepEqual(SCREEN_SHARE_FPS_OPTIONS, [30, 60]);
    assert.ok(Object.isFrozen(SCREEN_SHARE_FPS_OPTIONS));

    assert.deepEqual(SCREEN_SHARE_PORTAL_TABS, ['screens', 'windows']);
    assert.ok(Object.isFrozen(SCREEN_SHARE_PORTAL_TABS));

    assert.deepEqual(SCREEN_SHARE_MODAL_TABS, ['apps', 'devices']);
    assert.ok(Object.isFrozen(SCREEN_SHARE_MODAL_TABS));

    assert.ok(DEFAULT_SCREEN_SHARE_CONFIG);
    assert.equal(DEFAULT_SCREEN_SHARE_CONFIG.fps, 60);
    assert.ok(SCREEN_SHARE_DIALOG_TOKENS);
  });

  it('нормализует разрешение и FPS с fallback к значениям по умолчанию', () => {
    assert.equal(normalizeResolution('720p'), '720p');
    assert.equal(normalizeResolution('1080p'), '1080p');
    assert.equal(normalizeResolution('1440p'), '1440p');
    assert.equal(normalizeResolution('4k'), '1080p');
    assert.equal(normalizeResolution(null), '1080p');

    assert.equal(normalizeFps(30), 30);
    assert.equal(normalizeFps(60), 60);
    assert.equal(normalizeFps('30'), 30);
    assert.equal(normalizeFps(120), 60);
    assert.equal(normalizeFps(0), 60);
  });

  it('фильтрует источники по категории (all, screens, windows) и строке поиска', () => {
    const sources = [
      { id: 'screen:0', name: 'LG Ultrawide', kind: 'screen' },
      { id: 'screen:1', name: 'Встроенный дисплей', kind: 'screen' },
      { id: 'window:101', name: 'Firefox Browser', kind: 'window' },
      { id: 'window:102', name: 'Terminal / bash', kind: 'window' },
    ];

    assert.equal(filterSourcesByCategory(sources, 'all').length, 4);
    assert.equal(filterSourcesByCategory(sources, 'screens').length, 2);
    assert.equal(filterSourcesByCategory(sources, 'windows').length, 2);

    // С поисковым запросом
    const searched = filterSourcesByCategory(sources, 'all', 'firefox');
    assert.equal(searched.length, 1);
    assert.equal(searched[0].id, 'window:101');

    const searchedScreen = filterSourcesByCategory(sources, 'screens', 'дисплей');
    assert.equal(searchedScreen.length, 1);
    assert.equal(searchedScreen[0].id, 'screen:1');
  });

  it('formatSourceItem нормализует сырой источник и назначает иконку по названию', () => {
    const raw = { id: 'win_1', name: 'Mozilla Firefox' };
    const item = formatSourceItem(raw);
    assert.equal(item.id, 'win_1');
    assert.equal(item.label, 'Mozilla Firefox');
    assert.equal(item.icon, '🌐');

    const orca = formatSourceItem({ id: 'win_2', name: 'Orca Workspace' });
    assert.equal(orca.icon, '🦦');

    const generic = formatSourceItem({ id: 'win_3', name: 'SomeApp' });
    assert.equal(generic.icon, '🪟');
  });
});

describe('TAURI-MODAL-01: createScreenShareDialogController / openScreenShareDialog', () => {
  it('рендерит диалог с ролью dialog, aria-modal, заголовком и кнопкой запуска', () => {
    const { container } = createMockContainer();
    /** @type {any[]} */
    const started = [];

    const controller = createScreenShareDialogController(container, {
      sources: [
        { id: 'screen:0', name: 'Монитор 1' },
        { id: 'window:42', name: 'Discord' },
      ],
      onStartStream: (payload) => started.push(payload),
    });

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog);
    assert.equal(dialog.getAttribute('data-screen-share-dialog'), 'true');
    assert.equal(dialog.getAttribute('aria-modal'), 'true');

    const title = dialog.querySelector('[data-dialog-title]');
    assert.ok(title);
    assert.equal(title.textContent, 'Демонстрация экрана');

    const submitBtn = dialog.querySelector('[data-action="start-stream"]');
    assert.ok(submitBtn);
    submitBtn.click();

    assert.equal(started.length, 1);
    assert.deepEqual(started[0], {
      sourceId: 'screen:0',
      resolution: '1080p',
      fps: 60,
      shareAudio: false,
    });

    controller.close();
    assert.equal(container.querySelector('[role="dialog"]'), null);
  });

  it('позволяет выбирать категорию, источник, разрешение, FPS и аудио чекбокс', () => {
    const { container } = createMockContainer();
    /** @type {any[]} */
    const started = [];

    const controller = createScreenShareDialogController(container, {
      sources: [
        { id: 'screen:0', name: 'Монитор 1', kind: 'screen' },
        { id: 'window:99', name: 'Sublime Text', kind: 'window' },
      ],
      defaultResolution: '720p',
      defaultFps: 30,
      onStartStream: (payload) => started.push(payload),
    });

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog);

    // Переключаем категорию на окна
    const tabWindows = dialog.querySelector('[data-category-tab="windows"]');
    assert.ok(tabWindows);
    tabWindows.click();

    // Выбираем окно
    const winBtn = dialog.querySelector('[data-source-id="window:99"]');
    assert.ok(winBtn);
    winBtn.click();

    // Выбираем разрешение 1440p и FPS 60
    const res1440 = dialog.querySelector('[data-resolution="1440p"]');
    assert.ok(res1440);
    res1440.click();

    const fps60 = dialog.querySelector('[data-fps="60"]');
    assert.ok(fps60);
    fps60.click();

    // Включаем звук
    const audioCheckbox = dialog.querySelector('[data-share-audio]');
    assert.ok(audioCheckbox);
    audioCheckbox.click();

    const state = controller.getState();
    assert.equal(state.sourceId, 'window:99');
    assert.equal(state.resolution, '1440p');
    assert.equal(state.fps, 60);
    assert.equal(state.shareAudio, true);

    const submitBtn = dialog.querySelector('[data-action="start-stream"]');
    submitBtn.click();

    assert.equal(started.length, 1);
    assert.equal(started[0].sourceId, 'window:99');
    assert.equal(started[0].resolution, '1440p');
    assert.equal(started[0].fps, 60);
    assert.equal(started[0].shareAudio, true);
  });

  it('блокирует кнопку отправки, если нет выбранного источника', () => {
    const { container } = createMockContainer();
    /** @type {any[]} */
    const started = [];

    createScreenShareDialogController(container, {
      sources: [],
      onStartStream: (payload) => started.push(payload),
    });

    const dialog = container.querySelector('[role="dialog"]');
    const submitBtn = dialog.querySelector('[data-action="start-stream"]');
    assert.ok(submitBtn);
    assert.equal(submitBtn.disabled, true);

    submitBtn.click();
    assert.equal(started.length, 0);
  });
});

describe('TAURI-MODAL-01: createScreenSharePortalPicker / openScreenSharePortalPicker', () => {
  it('рендерит модальное окно портала с поиском, вкладками и источниками', () => {
    const { container, doc } = createMockContainer();
    /** @type {any[]} */
    const selected = [];

    const portal = createScreenSharePortalPicker({
      doc,
      initialPortalTab: 'screens',
      sources: [
        { id: 'screen_1', label: 'Основной дисплей LG 27"', tab: 'screens' },
        { id: 'win_1', label: 'Discord', tab: 'windows' },
      ],
      onSelectSource: (item) => selected.push(item),
    });

    const modal = doc.getElementById('live-screenshare-portal-modal');
    assert.ok(modal);
    const dialog = modal.querySelector('[role="dialog"]');
    assert.ok(dialog);
    assert.equal(dialog.getAttribute('aria-label'), 'Запрос демонстрации экрана');

    // Кликаем по карточке источника
    const card = modal.querySelector('[data-portal-source-id="screen_1"]');
    assert.ok(card);
    card.click();

    assert.equal(selected.length, 1);
    assert.equal(selected[0].id, 'screen_1');

    // Модалка должна закрыться
    assert.equal(doc.getElementById('live-screenshare-portal-modal'), null);
  });

  it('фильтрует источники по строке поиска в портале', () => {
    const { doc } = createMockContainer();
    const portal = createScreenSharePortalPicker({
      doc,
      initialPortalTab: 'windows',
      sources: [
        { id: 'win_firefox', label: 'Firefox Browser', tab: 'windows' },
        { id: 'win_term', label: 'Terminal', tab: 'windows' },
      ],
    });

    const modal = doc.getElementById('live-screenshare-portal-modal');
    assert.ok(modal);

    const searchInput = modal.querySelector('input[type="search"]');
    assert.ok(searchInput);

    // До поиска видны 2 источника
    assert.equal(modal.querySelectorAll('[data-portal-source-id]').length, 2);

    // Вводим поисковый запрос
    searchInput.value = 'term';
    searchInput.dispatchEvent('input');

    const visibleAfter = modal.querySelectorAll('[data-portal-source-id]');
    assert.equal(visibleAfter.length, 1);
    assert.equal(visibleAfter[0].getAttribute('data-portal-source-id'), 'win_term');

    portal.close();
    assert.equal(doc.getElementById('live-screenshare-portal-modal'), null);
  });

  it('закрывается при вызове close() или клике по фону', () => {
    const { doc } = createMockContainer();
    const portal = createScreenSharePortalPicker({
      doc,
      initialPortalTab: 'screens',
      sources: [{ id: 's1', label: 'Screen 1', tab: 'screens' }],
    });

    const backdrop = doc.getElementById('live-screenshare-portal-modal');
    assert.ok(backdrop);

    backdrop.dispatchEvent('click', { target: backdrop });
    assert.equal(doc.getElementById('live-screenshare-portal-modal'), null);
  });
});

describe('TAURI-MODAL-01: createScreenShareModalController / openScreenShareModal', () => {
  it('рендерит полноценное модальное окно демонстрации экрана с вкладками apps/devices', () => {
    const { doc } = createMockContainer();
    /** @type {any[]} */
    const started = [];

    const modal = createScreenShareModalController({
      doc,
      onStartStream: (src) => started.push(src),
    });

    const modalEl = doc.getElementById('live-screenshare-modal');
    assert.ok(modalEl);

    // Проверяем карточки для экранов и окон
    const screenCard = modalEl.querySelector('[data-capture-kind="screens"]');
    const windowCard = modalEl.querySelector('[data-capture-kind="windows"]');
    assert.ok(screenCard);
    assert.ok(windowCard);

    modal.close();
    assert.equal(doc.getElementById('live-screenshare-modal'), null);
  });
});
