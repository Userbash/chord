// @ts-check
/**
 * T-201: контроллер голосовой сцены (плитки участников, фокус, переполнение)
 * и диалог выбора источника демонстрации экрана («Демонстрация экрана»).
 * @module ui/features/voice-stage-ui
 */

import {
  computeVoiceGridDimensions,
  prioritizeVoiceParticipants,
  MAX_VISIBLE_VOICE_TILES,
} from './voice-grid.js';

export {
  STREAM_VIEWER_MODES,
  STREAM_VIEWER_ZOOM_LIMITS,
  clampStreamZoomScale,
  formatStreamVolumeLabel,
  createStreamViewerController,
} from './stream-viewer-ui.js';

/** Допустимые разрешения демонстрации экрана. */
export const SCREEN_SHARE_RESOLUTIONS = Object.freeze(
  /** @type {const} */ (['720p', '1080p', '1440p']),
);

/** Допустимые значения частоты кадров демонстрации экрана. */
export const SCREEN_SHARE_FPS_OPTIONS = Object.freeze(/** @type {const} */ ([30, 60]));

/**
 * @typedef {import('./voice-grid.js').VoiceParticipant} VoiceParticipant
 * @typedef {import('./voice-grid.js').VoiceGridDimensions} VoiceGridDimensions
 *
 * @typedef {'720p' | '1080p' | '1440p'} ScreenShareResolution
 * @typedef {30 | 60} ScreenShareFps
 *
 * @typedef {{
 *   id: string,
 *   name?: string,
 *   title?: string,
 *   kind?: 'screen' | 'window' | string,
 *   source_type?: 'screen' | 'window' | string,
 *   is_primary?: boolean,
 * }} ScreenShareSource
 *
 * @typedef {{
 *   sourceId: string,
 *   resolution: ScreenShareResolution,
 *   fps: ScreenShareFps,
 *   shareAudio: boolean,
 * }} StartStreamPayload
 *
 * @typedef {{
 *   sources?: ReadonlyArray<ScreenShareSource>,
 *   loadSources?: () => Promise<ReadonlyArray<ScreenShareSource>>,
 *   defaultResolution?: ScreenShareResolution | string,
 *   defaultFps?: ScreenShareFps | number,
 *   defaultShareAudio?: boolean,
 *   defaultCategory?: 'all' | 'screens' | 'windows' | string,
 * }} ScreenSharePickerOptions
 *
 * @typedef {{
 *   onFocusChange?: (focusedId: string | null) => void,
 *   onStartStream?: (payload: StartStreamPayload) => void,
 *   participants?: ReadonlyArray<VoiceParticipant>,
 *   focusedId?: string | null,
 * }} VoiceStageOptions
 */

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
    typeof containerEl.ownerDocument.createElement === 'function'
  ) {
    return containerEl.ownerDocument;
  }
  if (containerEl && typeof containerEl.createElement === 'function') {
    return containerEl;
  }
  const globalDoc = /** @type {any} */ (globalThis).document;
  if (globalDoc && typeof globalDoc.createElement === 'function') {
    return globalDoc;
  }
  throw new TypeError('Container element must provide ownerDocument.createElement');
}

/**
 * Заменяет дочерние узлы контейнера на переданный список.
 *
 * @param {any} parentEl
 * @param {any[]} nodes
 */
function replaceElementChildren(parentEl, nodes) {
  if (typeof parentEl.replaceChildren === 'function') {
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
 * Выставляет состояние `disabled` как свойством, так и атрибутом.
 *
 * @param {any} buttonEl
 * @param {boolean} isDisabled
 */
function setButtonDisabled(buttonEl, isDisabled) {
  buttonEl.disabled = isDisabled;
  if (isDisabled) {
    if (typeof buttonEl.setAttribute === 'function') buttonEl.setAttribute('disabled', '');
  } else if (typeof buttonEl.removeAttribute === 'function') {
    buttonEl.removeAttribute('disabled');
  }
}

/**
 * Нормализует разрешение демонстрации экрана к допустимому значению.
 *
 * @param {unknown} value
 * @returns {ScreenShareResolution}
 */
function normalizeResolution(value) {
  if (value === '720p' || value === '1080p' || value === '1440p') {
    return value;
  }
  return '1080p';
}

/**
 * Нормализует частоту кадров демонстрации экрана к `30` или `60`.
 *
 * @param {unknown} value
 * @returns {ScreenShareFps}
 */
function normalizeFps(value) {
  const num = Number(value);
  if (num === 30 || num === 60) {
    return num;
  }
  return 60;
}

/**
 * Создаёт контроллер голосовой сцены и диалога демонстрации экрана.
 *
 * @param {any} containerEl
 * @param {VoiceStageOptions} [options]
 */
export function createVoiceStageController(containerEl, options = {}) {
  const doc = resolveDocument(containerEl);

  const gridEl = doc.createElement('div');
  gridEl.setAttribute('data-voice-grid', 'true');
  containerEl.appendChild(gridEl);

  /** @type {VoiceParticipant[]} */
  let participants = Array.isArray(options.participants) ? [...options.participants] : [];
  /** @type {string | null} */
  let focusedId = typeof options.focusedId === 'string' ? options.focusedId : null;
  /** @type {any | null} */
  let activeDialogEl = null;

  /**
   * Синхронизирует атрибуты `data-columns` и `data-rows` на сетке и контейнере.
   *
   * @param {VoiceGridDimensions} dimensions
   */
  function applyGridAttributes(dimensions) {
    const cols = String(dimensions.columns);
    const rows = String(dimensions.rows);
    gridEl.setAttribute('data-columns', cols);
    gridEl.setAttribute('data-rows', rows);
    if (typeof containerEl.setAttribute === 'function') {
      containerEl.setAttribute('data-columns', cols);
      containerEl.setAttribute('data-rows', rows);
    }
    if (gridEl.style && typeof gridEl.style.setProperty === 'function') {
      gridEl.style.setProperty('display', 'grid');
      gridEl.style.setProperty(
        'grid-template-columns',
        dimensions.columns > 0 ? `repeat(${dimensions.columns}, minmax(0, 1fr))` : 'none',
      );
      gridEl.style.setProperty(
        'grid-template-rows',
        dimensions.rows > 0 ? `repeat(${dimensions.rows}, minmax(0, 1fr))` : 'none',
      );
    }
  }

  /**
   * Создаёт DOM-плитку участника голосового канала.
   *
   * @param {VoiceParticipant} participant
   * @returns {any}
   */
  function createParticipantTile(participant) {
    const id = String(participant.id);
    const isSpeaking = participant.speaking === true;
    const isStreaming = participant.streaming === true;
    const isMuted = participant.muted === true;
    const isFocused = focusedId !== null && focusedId === id;

    const tile = doc.createElement('div');
    tile.setAttribute('data-participant-id', id);
    tile.setAttribute('data-speaking', isSpeaking ? 'true' : 'false');
    tile.setAttribute('data-streaming', isStreaming ? 'true' : 'false');
    tile.setAttribute('data-focused', isFocused ? 'true' : 'false');
    tile.setAttribute('aria-pressed', isFocused ? 'true' : 'false');

    const nameEl = doc.createElement('span');
    nameEl.setAttribute('data-participant-name', '');
    nameEl.textContent =
      typeof participant.displayName === 'string' && participant.displayName.length > 0
        ? participant.displayName
        : id;
    tile.appendChild(nameEl);

    if (isStreaming) {
      const liveBadge = doc.createElement('span');
      liveBadge.setAttribute('data-badge', 'streaming');
      liveBadge.textContent = 'В эфире';
      tile.appendChild(liveBadge);
    }

    if (isMuted) {
      const muteBadge = doc.createElement('span');
      muteBadge.setAttribute('data-badge', 'muted');
      muteBadge.textContent = 'Микрофон выкл.';
      tile.appendChild(muteBadge);
    }

    tile.addEventListener('click', () => {
      focusedId = focusedId === id ? null : id;
      renderStage();
      if (typeof options.onFocusChange === 'function') {
        options.onFocusChange(focusedId);
      }
    });

    return tile;
  }

  /**
   * Перерисовывает сетку участников и плитку переполнения.
   *
   * @returns {{ visible: VoiceParticipant[], overflow: VoiceParticipant[], grid: VoiceGridDimensions }}
   */
  function renderStage() {
    const prioritized = prioritizeVoiceParticipants(participants, focusedId);
    const dimensions = computeVoiceGridDimensions(participants.length);
    applyGridAttributes(dimensions);

    /** @type {any[]} */
    const tiles = [];
    const visibleSlice = prioritized.visible.slice(0, MAX_VISIBLE_VOICE_TILES);
    for (const participant of visibleSlice) {
      tiles.push(createParticipantTile(participant));
    }

    if (prioritized.overflow.length > 0) {
      const overflowTile = doc.createElement('div');
      overflowTile.setAttribute('data-voice-overflow', 'true');
      overflowTile.textContent = `+${prioritized.overflow.length} ещё`;
      tiles.push(overflowTile);
    }

    replaceElementChildren(gridEl, tiles);
    return {
      visible: visibleSlice,
      overflow: prioritized.overflow,
      grid: dimensions,
    };
  }

  /**
   * Обновляет список участников голосовой сцены и перерисовывает сетку.
   *
   * @param {ReadonlyArray<VoiceParticipant>} nextParticipants
   */
  function setParticipants(nextParticipants) {
    participants = Array.isArray(nextParticipants) ? [...nextParticipants] : [];
    if (focusedId !== null && !participants.some((p) => p && String(p.id) === focusedId)) {
      focusedId = null;
    }
    return renderStage();
  }

  /**
   * Открывает диалог «Демонстрация экрана» для настройки и запуска стрима.
   *
   * @param {ScreenSharePickerOptions} [pickerOptions]
   */
  function openScreenSharePicker(pickerOptions = {}) {
    if (activeDialogEl && typeof activeDialogEl.remove === 'function') {
      activeDialogEl.remove();
    }

    const rawSources = Array.isArray(pickerOptions.sources) ? [...pickerOptions.sources] : [];
    let sources = rawSources.filter((s) => s && typeof s.id === 'string' && s.id.length > 0);

    let selectedSourceId = sources[0]?.id ?? '';
    /** @type {ScreenShareResolution} */
    let selectedResolution = normalizeResolution(pickerOptions.defaultResolution ?? '1080p');
    /** @type {ScreenShareFps} */
    let selectedFps = normalizeFps(pickerOptions.defaultFps ?? 60);
    let shareAudio = Boolean(pickerOptions.defaultShareAudio ?? false);
    /** @type {'all' | 'screens' | 'windows'} */
    let activeCategory =
      pickerOptions.defaultCategory === 'screens' || pickerOptions.defaultCategory === 'windows'
        ? pickerOptions.defaultCategory
        : 'all';

    const dialog = doc.createElement('div');
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    dialog.setAttribute('aria-label', 'Демонстрация экрана');
    dialog.setAttribute('data-screen-share-dialog', 'true');

    const titleEl = doc.createElement('h2');
    titleEl.setAttribute('data-dialog-title', '');
    titleEl.textContent = 'Демонстрация экрана';
    dialog.appendChild(titleEl);

    // Вкладки категорий: Все, Экраны, Окна
    const categoryGroup = doc.createElement('div');
    categoryGroup.setAttribute('data-category-tabs', '');
    /** @type {Map<'all' | 'screens' | 'windows', any>} */
    const categoryButtons = new Map();

    const categories = [
      { id: /** @type {const} */ ('all'), label: 'Все' },
      { id: /** @type {const} */ ('screens'), label: 'Экраны' },
      { id: /** @type {const} */ ('windows'), label: 'Окна' },
    ];

    for (const cat of categories) {
      const btn = doc.createElement('button');
      btn.setAttribute('type', 'button');
      btn.setAttribute('data-category-tab', cat.id);
      btn.textContent = cat.label;
      btn.addEventListener('click', () => {
        activeCategory = cat.id;
        renderSourcesList();
        syncPickerState();
      });
      categoryButtons.set(cat.id, btn);
      categoryGroup.appendChild(btn);
    }
    dialog.appendChild(categoryGroup);

    const sourcesGroup = doc.createElement('div');
    sourcesGroup.setAttribute('data-sources-list', '');
    dialog.appendChild(sourcesGroup);

    /** @type {Map<string, any>} */
    const sourceButtons = new Map();

    function renderSourcesList() {
      sourceButtons.clear();
      const filteredSources = sources.filter((s) => {
        if (activeCategory === 'all') return true;
        const kind = String(s.kind || s.source_type || '').toLowerCase();
        const isScreen = kind === 'screen' || kind === 'screens' || s.id.startsWith('screen:');
        return activeCategory === 'screens' ? isScreen : !isScreen;
      });

      const buttons = [];
      for (const source of filteredSources) {
        const btn = doc.createElement('button');
        btn.setAttribute('type', 'button');
        btn.setAttribute('data-source-id', source.id);
        btn.textContent = source.name ?? source.title ?? source.id;
        btn.addEventListener('click', () => {
          selectedSourceId = source.id;
          syncPickerState();
        });
        sourceButtons.set(source.id, btn);
        buttons.push(btn);
      }
      replaceElementChildren(sourcesGroup, buttons);
    }

    renderSourcesList();

    const resolutionGroup = doc.createElement('div');
    resolutionGroup.setAttribute('data-resolution-group', '');
    /** @type {Map<ScreenShareResolution, any>} */
    const resolutionButtons = new Map();

    for (const res of SCREEN_SHARE_RESOLUTIONS) {
      const btn = doc.createElement('button');
      btn.setAttribute('type', 'button');
      btn.setAttribute('role', 'radio');
      btn.setAttribute('data-resolution', res);
      btn.textContent = res;
      btn.addEventListener('click', () => {
        selectedResolution = res;
        syncPickerState();
      });
      resolutionButtons.set(res, btn);
      resolutionGroup.appendChild(btn);
    }
    dialog.appendChild(resolutionGroup);

    const fpsGroup = doc.createElement('div');
    fpsGroup.setAttribute('data-fps-group', '');
    /** @type {Map<ScreenShareFps, any>} */
    const fpsButtons = new Map();

    for (const fps of SCREEN_SHARE_FPS_OPTIONS) {
      const btn = doc.createElement('button');
      btn.setAttribute('type', 'button');
      btn.setAttribute('role', 'radio');
      btn.setAttribute('data-fps', String(fps));
      btn.textContent = `${fps} FPS`;
      btn.addEventListener('click', () => {
        selectedFps = fps;
        syncPickerState();
      });
      fpsButtons.set(fps, btn);
      fpsGroup.appendChild(btn);
    }
    dialog.appendChild(fpsGroup);

    const audioLabel = doc.createElement('label');
    audioLabel.setAttribute('data-share-audio-label', '');

    const audioCheckbox = doc.createElement('input');
    audioCheckbox.setAttribute('type', 'checkbox');
    audioCheckbox.setAttribute('data-share-audio', 'true');

    const audioText = doc.createElement('span');
    audioText.textContent = 'Транслировать звук системы';

    const toggleAudio = () => {
      shareAudio = !shareAudio;
      syncPickerState();
    };
    audioCheckbox.addEventListener('click', (/** @type {any} */ event) => {
      if (event && typeof event.stopPropagation === 'function') {
        event.stopPropagation();
      }
      toggleAudio();
    });
    audioCheckbox.addEventListener('change', () => {
      if (typeof audioCheckbox.checked === 'boolean') {
        shareAudio = audioCheckbox.checked;
        syncPickerState();
      }
    });

    audioLabel.appendChild(audioCheckbox);
    audioLabel.appendChild(audioText);
    dialog.appendChild(audioLabel);

    const submitBtn = doc.createElement('button');
    submitBtn.setAttribute('type', 'button');
    submitBtn.setAttribute('data-action', 'start-stream');
    submitBtn.setAttribute('data-submit-stream', 'true');
    submitBtn.textContent = 'Начать трансляцию';

    submitBtn.addEventListener('click', () => {
      if (!selectedSourceId) return;
      if (typeof options.onStartStream === 'function') {
        options.onStartStream({
          sourceId: selectedSourceId,
          resolution: selectedResolution,
          fps: selectedFps,
          shareAudio,
        });
      }
    });
    dialog.appendChild(submitBtn);

    function syncPickerState() {
      for (const [cat, btn] of categoryButtons.entries()) {
        const isSelected = cat === activeCategory;
        btn.setAttribute('data-selected', isSelected ? 'true' : 'false');
        btn.setAttribute('aria-selected', isSelected ? 'true' : 'false');
      }
      for (const [id, btn] of sourceButtons.entries()) {
        const isSelected = id === selectedSourceId;
        btn.setAttribute('data-selected', isSelected ? 'true' : 'false');
        btn.setAttribute('aria-pressed', isSelected ? 'true' : 'false');
      }
      for (const [res, btn] of resolutionButtons.entries()) {
        const isSelected = res === selectedResolution;
        btn.setAttribute('data-selected', isSelected ? 'true' : 'false');
        btn.setAttribute('aria-checked', isSelected ? 'true' : 'false');
      }
      for (const [fps, btn] of fpsButtons.entries()) {
        const isSelected = fps === selectedFps;
        btn.setAttribute('data-selected', isSelected ? 'true' : 'false');
        btn.setAttribute('aria-checked', isSelected ? 'true' : 'false');
      }
      audioCheckbox.checked = shareAudio;
      audioCheckbox.setAttribute('data-checked', shareAudio ? 'true' : 'false');
      audioCheckbox.setAttribute('aria-checked', shareAudio ? 'true' : 'false');
      setButtonDisabled(submitBtn, !selectedSourceId);
    }

    /** @type {Promise<any> | null} */
    let loadPromise = null;
    if (typeof pickerOptions.loadSources === 'function') {
      loadPromise = pickerOptions
        .loadSources()
        .then((loaded) => {
          if (Array.isArray(loaded)) {
            sources = loaded.filter((s) => s && typeof s.id === 'string' && s.id.length > 0);
            if (!selectedSourceId && sources.length > 0) {
              selectedSourceId = sources[0].id;
            }
            renderSourcesList();
            syncPickerState();
          }
          return loaded;
        })
        .catch(() => []);
    } else if (!pickerOptions.sources && typeof globalThis !== 'undefined') {
      const tauri = /** @type {any} */ (globalThis).__TAURI__;
      const tauriInvoke = tauri?.core?.invoke ?? tauri?.invoke;
      if (typeof tauriInvoke === 'function') {
        loadPromise = tauriInvoke('voice_desktop_sources')
          .then((/** @type {any} */ res) => {
            if (res && Array.isArray(res.sources)) {
              sources = res.sources.map((/** @type {any} */ s) => ({
                id: String(s.id),
                name: String(s.name || s.id),
                kind: s.source_type,
                source_type: s.source_type,
                is_primary: Boolean(s.is_primary),
              }));
              if (!selectedSourceId && sources.length > 0) {
                selectedSourceId = sources[0].id;
              }
              renderSourcesList();
              syncPickerState();
            }
            return sources;
          })
          .catch(() => []);
      }
    }

    syncPickerState();
    containerEl.appendChild(dialog);
    activeDialogEl = dialog;

    return {
      element: dialog,
      loadPromise,
      /** @returns {StartStreamPayload} */
      getState() {
        return {
          sourceId: selectedSourceId,
          resolution: selectedResolution,
          fps: selectedFps,
          shareAudio,
        };
      },
      close() {
        if (activeDialogEl === dialog) {
          activeDialogEl = null;
        }
        if (typeof dialog.remove === 'function') {
          dialog.remove();
        }
      },
    };
  }

  renderStage();

  return {
    element: gridEl,
    setParticipants,
    render: setParticipants,
    /** @returns {string | null} */
    getFocusedId() {
      return focusedId;
    },
    openScreenSharePicker,
  };
}
