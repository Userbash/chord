// @ts-check
/**
 * T-201 (RW-40 / G2): unit-тесты контроллера голосовой сцены и пикера демонстрации экрана
 * (`rust/gui-spike/ui/features/voice-stage-ui.js`).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createVoiceStageController } from '../../rust/gui-spike/ui/features/voice-stage-ui.js';
import { MAX_VISIBLE_VOICE_TILES } from '../../rust/gui-spike/ui/features/voice-grid.js';

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
    this.checked = false;
    /** @type {Map<string, Array<(event: any) => void>>} */
    this._listeners = new Map();
    this.style = {
      /** @type {Record<string, string>} */
      _props: {},
      /**
       * @param {string} name
       * @param {string} value
       */
      setProperty(name, value) {
        this._props[name] = String(value);
      },
      /**
       * @param {string} name
       * @returns {string}
       */
      getPropertyValue(name) {
        return this._props[name] ?? '';
      },
    };
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

  remove() {
    if (this.parentNode) {
      this.parentNode.removeChild(this);
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
 * Тесты контроллера голосовой сцены
 * ------------------------------------------------------------------ */

describe('createVoiceStageController: сетка и плитки участников', () => {
  it('выставляет data-columns и data-rows из computeVoiceGridDimensions для разного числа участников', () => {
    const container = makeContainer();
    const stage = createVoiceStageController(container);

    const gridEl = container.querySelector('[data-voice-grid]') ?? container;
    assert.equal(gridEl.getAttribute('data-columns'), '0');
    assert.equal(gridEl.getAttribute('data-rows'), '0');

    stage.setParticipants([{ id: 'u1', displayName: 'Alice' }]);
    assert.equal(gridEl.getAttribute('data-columns'), '1');
    assert.equal(gridEl.getAttribute('data-rows'), '1');

    stage.setParticipants([
      { id: 'u1', displayName: 'Alice' },
      { id: 'u2', displayName: 'Bob' },
      { id: 'u3', displayName: 'Carol' },
    ]);
    assert.equal(gridEl.getAttribute('data-columns'), '2');
    assert.equal(gridEl.getAttribute('data-rows'), '2');

    stage.setParticipants(
      Array.from({ length: 6 }, (_, i) => ({ id: `u${i + 1}`, displayName: `User ${i + 1}` })),
    );
    assert.equal(gridEl.getAttribute('data-columns'), '3');
    assert.equal(gridEl.getAttribute('data-rows'), '2');
  });

  it('рендерит плитки с data-participant-id, data-speaking, data-streaming, data-focused и русскими бейджами', () => {
    const container = makeContainer();
    const stage = createVoiceStageController(container);

    stage.setParticipants([
      {
        id: 'p-muted',
        displayName: 'Иван',
        speaking: false,
        streaming: false,
        muted: true,
      },
      {
        id: 'p-stream',
        displayName: 'Анна',
        speaking: true,
        streaming: true,
        muted: false,
      },
    ]);

    const tiles = container.querySelectorAll('[data-participant-id]');
    assert.equal(tiles.length, 2);

    // По приоритету prioritizeVoiceParticipants стримящий участник идёт первым
    const streamTile = tiles[0];
    const mutedTile = tiles[1];

    assert.equal(streamTile?.getAttribute('data-participant-id'), 'p-stream');
    assert.equal(streamTile?.getAttribute('data-speaking'), 'true');
    assert.equal(streamTile?.getAttribute('data-streaming'), 'true');
    assert.equal(streamTile?.getAttribute('data-focused'), 'false');
    assert.equal(streamTile?.textContent.includes('В эфире'), true);
    assert.equal(streamTile?.textContent.includes('Микрофон выкл.'), false);

    assert.equal(mutedTile?.getAttribute('data-participant-id'), 'p-muted');
    assert.equal(mutedTile?.getAttribute('data-speaking'), 'false');
    assert.equal(mutedTile?.getAttribute('data-streaming'), 'false');
    assert.equal(mutedTile?.getAttribute('data-focused'), 'false');
    assert.equal(mutedTile?.textContent.includes('Микрофон выкл.'), true);
    assert.equal(mutedTile?.textContent.includes('В эфире'), false);
  });

  it('клик по плитке переключает фокус, обновляет data-focused и вызывает onFocusChange(focusedId)', () => {
    const container = makeContainer();
    /** @type {Array<string | null>} */
    const focusEvents = [];
    const stage = createVoiceStageController(container, {
      onFocusChange: (id) => focusEvents.push(id),
    });

    stage.setParticipants([
      { id: 'u1', displayName: 'Alice', streaming: true },
      { id: 'u2', displayName: 'Bob', speaking: false },
    ]);

    // Клик по u2 фокусирует его и поднимает на первое место
    const tileU2 = container.querySelector('[data-participant-id="u2"]');
    tileU2?.click();

    assert.deepEqual(focusEvents, ['u2']);
    assert.equal(
      container.querySelector('[data-participant-id="u2"]')?.getAttribute('data-focused'),
      'true',
    );
    assert.equal(
      container.querySelector('[data-participant-id="u1"]')?.getAttribute('data-focused'),
      'false',
    );
    const orderedAfterFocus = container
      .querySelectorAll('[data-participant-id]')
      .map((el) => el.getAttribute('data-participant-id'));
    assert.deepEqual(orderedAfterFocus, ['u2', 'u1']);

    // Клик по другой плитке u1 переносит фокус на u1
    container.querySelector('[data-participant-id="u1"]')?.click();
    assert.deepEqual(focusEvents, ['u2', 'u1']);
    assert.equal(
      container.querySelector('[data-participant-id="u1"]')?.getAttribute('data-focused'),
      'true',
    );
    assert.equal(
      container.querySelector('[data-participant-id="u2"]')?.getAttribute('data-focused'),
      'false',
    );

    // Повторный клик по сфокусированной плитке снимает фокус (null)
    container.querySelector('[data-participant-id="u1"]')?.click();
    assert.deepEqual(focusEvents, ['u2', 'u1', null]);
    assert.equal(
      container.querySelector('[data-participant-id="u1"]')?.getAttribute('data-focused'),
      'false',
    );
  });

  it('при переполнении (> 25 участников) рендерит плитку data-voice-overflow="true" с текстом "+N ещё"', () => {
    const container = makeContainer();
    const stage = createVoiceStageController(container);

    const exact25 = Array.from({ length: MAX_VISIBLE_VOICE_TILES }, (_, i) => ({
      id: `p-${i + 1}`,
      displayName: `User ${String(i + 1).padStart(2, '0')}`,
    }));
    stage.setParticipants(exact25);
    assert.equal(container.querySelectorAll('[data-participant-id]').length, 25);
    assert.equal(container.querySelector('[data-voice-overflow="true"]'), null);

    const total29 = Array.from({ length: MAX_VISIBLE_VOICE_TILES + 4 }, (_, i) => ({
      id: `p-${i + 1}`,
      displayName: `User ${String(i + 1).padStart(2, '0')}`,
    }));
    stage.setParticipants(total29);

    const gridEl = container.querySelector('[data-voice-grid]') ?? container;
    assert.equal(gridEl.getAttribute('data-columns'), '5');
    assert.equal(gridEl.getAttribute('data-rows'), '5');
    assert.equal(container.querySelectorAll('[data-participant-id]').length, 25);

    const overflowTile = container.querySelector('[data-voice-overflow="true"]');
    assert.ok(overflowTile);
    assert.equal(overflowTile.textContent, '+4 ещё');
  });
});

describe('createVoiceStageController: openScreenSharePicker', () => {
  it('рендерит русский диалог «Демонстрация экрана» с дефолтами 1080p / 60 FPS и вызывает onStartStream', () => {
    const container = makeContainer();
    /** @type {any[]} */
    const startedStreams = [];
    const stage = createVoiceStageController(container, {
      onStartStream: (payload) => startedStreams.push(payload),
    });

    stage.openScreenSharePicker({
      sources: [
        { id: 'screen:0', name: 'Экран 1' },
        { id: 'window:7', name: 'VS Code' },
      ],
    });

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog);
    assert.equal(dialog.textContent.includes('Демонстрация экрана'), true);
    assert.equal(dialog.textContent.includes('Транслировать звук системы'), true);

    const submitBtn = dialog.querySelector('[data-action="start-stream"]') ??
      dialog.querySelectorAll('button').find((b) => b.textContent === 'Начать трансляцию') ??
      null;
    assert.ok(submitBtn);
    assert.equal(submitBtn.textContent, 'Начать трансляцию');

    // Отправка с параметрами по умолчанию (первый источник, 1080p, 60 FPS, shareAudio=false)
    submitBtn.click();
    assert.deepEqual(startedStreams, [
      {
        sourceId: 'screen:0',
        resolution: '1080p',
        fps: 60,
        shareAudio: false,
      },
    ]);
  });

  it('позволяет выбрать источник, разрешение (720p|1080p|1440p), FPS (30|60) и чекбокс звука системы', () => {
    const container = makeContainer();
    /** @type {any[]} */
    const startedStreams = [];
    const stage = createVoiceStageController(container, {
      onStartStream: (payload) => startedStreams.push(payload),
    });

    stage.openScreenSharePicker({
      sources: [
        { id: 'screen:1', name: 'Весь экран' },
        { id: 'window:42', name: 'Firefox' },
      ],
      defaultResolution: '720p',
      defaultFps: 30,
    });

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog);

    // Проверяем наличие всех вариантов разрешения и FPS
    for (const res of ['720p', '1080p', '1440p']) {
      assert.ok(dialog.querySelector(`[data-resolution="${res}"]`));
    }
    for (const fps of ['30', '60']) {
      assert.ok(dialog.querySelector(`[data-fps="${fps}"]`));
    }

    // Выбираем второй источник, разрешение 1440p, 60 FPS и включаем трансляцию звука
    dialog.querySelector('[data-source-id="window:42"]')?.click();
    dialog.querySelector('[data-resolution="1440p"]')?.click();
    dialog.querySelector('[data-fps="60"]')?.click();
    dialog.querySelector('[data-share-audio]')?.click();

    const submitBtn = dialog.querySelector('[data-action="start-stream"]');
    assert.ok(submitBtn);
    submitBtn.click();

    assert.deepEqual(startedStreams, [
      {
        sourceId: 'window:42',
        resolution: '1440p',
        fps: 60,
        shareAudio: true,
      },
    ]);
  });

  it('блокирует отправку «Начать трансляцию», если список источников пуст', () => {
    const container = makeContainer();
    /** @type {any[]} */
    const startedStreams = [];
    const stage = createVoiceStageController(container, {
      onStartStream: (payload) => startedStreams.push(payload),
    });

    stage.openScreenSharePicker({ sources: [] });
    const dialog = container.querySelector('[role="dialog"]');
    const submitBtn = dialog?.querySelector('[data-action="start-stream"]');
    assert.ok(submitBtn);
    assert.equal(submitBtn.disabled, true);
    submitBtn.click();
    assert.equal(startedStreams.length, 0);
  });

  it('поддерживает переключение вкладок категорий (все, экраны, окна) для фильтрации источников', () => {
    const container = makeContainer();
    const stage = createVoiceStageController(container);

    stage.openScreenSharePicker({
      sources: [
        { id: 'screen:0', name: 'Экран 1', kind: 'screen' },
        { id: 'screen:1', name: 'Экран 2', kind: 'screen' },
        { id: 'window:42', name: 'Терминал', kind: 'window' },
      ],
    });

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog);

    const tabScreens = dialog.querySelector('[data-category-tab="screens"]');
    const tabWindows = dialog.querySelector('[data-category-tab="windows"]');
    const tabAll = dialog.querySelector('[data-category-tab="all"]');
    assert.ok(tabScreens, 'Вкладка экранов должна существовать');
    assert.ok(tabWindows, 'Вкладка окон должна существовать');
    assert.ok(tabAll, 'Вкладка "Все" должна существовать');

    // По умолчанию показаны все 3 источника
    let visibleSources = dialog.querySelectorAll('[data-source-id]');
    assert.equal(visibleSources.length, 3);

    // Кликаем вкладку "Экраны"
    tabScreens.click();
    visibleSources = dialog.querySelectorAll('[data-source-id]');
    assert.equal(visibleSources.length, 2);
    assert.ok(dialog.querySelector('[data-source-id="screen:0"]'));
    assert.ok(dialog.querySelector('[data-source-id="screen:1"]'));
    assert.equal(dialog.querySelector('[data-source-id="window:42"]'), null);

    // Кликаем вкладку "Окна"
    tabWindows.click();
    visibleSources = dialog.querySelectorAll('[data-source-id]');
    assert.equal(visibleSources.length, 1);
    assert.ok(dialog.querySelector('[data-source-id="window:42"]'));

    // Возвращаемся во вкладку "Все"
    tabAll.click();
    visibleSources = dialog.querySelectorAll('[data-source-id]');
    assert.equal(visibleSources.length, 3);
  });

  it('поддерживает асинхронную загрузку источников через loadSources', async () => {
    const container = makeContainer();
    /** @type {any[]} */
    const startedStreams = [];
    const stage = createVoiceStageController(container, {
      onStartStream: (payload) => startedStreams.push(payload),
    });

    const picker = stage.openScreenSharePicker({
      loadSources: async () => [
        { id: 'screen:pipewire', name: 'PipeWire Desktop', kind: 'screen' },
        { id: 'window:pipewire', name: 'PipeWire Window', kind: 'window' },
      ],
    });

    const dialog = container.querySelector('[role="dialog"]');
    assert.ok(dialog);

    // Ждём завершения промиса загрузки источников
    if (picker.loadPromise) {
      await picker.loadPromise;
    }

    const sources = dialog.querySelectorAll('[data-source-id]');
    assert.equal(sources.length, 2);
    assert.ok(dialog.querySelector('[data-source-id="screen:pipewire"]'));

    const submitBtn = dialog.querySelector('[data-action="start-stream"]');
    assert.ok(submitBtn);
    assert.equal(submitBtn.disabled, false);
    submitBtn.click();

    assert.equal(startedStreams.length, 1);
    assert.equal(startedStreams[0]?.sourceId, 'screen:pipewire');
  });

  it('автоматически запрашивает источники из Tauri IPC voice_desktop_sources при отсутствии явных источников', async () => {
    const container = makeContainer();
    /** @type {any[]} */
    const ipcCalls = [];
    const origTauri = /** @type {any} */ (globalThis).__TAURI__;
    /** @type {any} */ (globalThis).__TAURI__ = {
      core: {
        /**
         * @param {string} cmd
         * @param {any} [args]
         */
        invoke: async (cmd, args) => {
          ipcCalls.push({ cmd, args });
          if (cmd === 'voice_desktop_sources') {
            return {
              backend: 'PipeWirePortal',
              sources: [
                { id: 'screen:0', name: 'Основной экран DP-1', source_type: 'screen', is_primary: true },
                { id: 'window:101', name: 'Discord Dev', source_type: 'window', is_primary: false },
              ],
            };
          }
          return null;
        },
      },
    };

    try {
      const stage = createVoiceStageController(container);
      const picker = stage.openScreenSharePicker();
      if (picker.loadPromise) {
        await picker.loadPromise;
      }

      assert.equal(ipcCalls.length, 1);
      assert.equal(ipcCalls[0].cmd, 'voice_desktop_sources');

      const dialog = container.querySelector('[role="dialog"]');
      assert.ok(dialog);
      const sources = dialog.querySelectorAll('[data-source-id]');
      assert.equal(sources.length, 2);
      assert.ok(dialog.querySelector('[data-source-id="screen:0"]'));
      assert.ok(dialog.querySelector('[data-source-id="window:101"]'));
    } finally {
      /** @type {any} */ (globalThis).__TAURI__ = origTauri;
    }
  });
});
