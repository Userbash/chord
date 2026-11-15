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

export {
  DISCORD_AV_ERROR_CODES,
  RTC_QUALITY_COLORS,
  CALL_SOUND_EFFECTS,
  discordMinEncodeFpsThreshold,
  defeatDiscordAvError,
  evaluateStreamAvHealth,
  evaluateRtcConnectionQuality,
  createRtcQualityStatusBar,
} from './rtc-quality-status-bar.js';

import {
  SCREEN_SHARE_RESOLUTIONS,
  SCREEN_SHARE_FPS_OPTIONS,
} from './screen-share-dialog.js';

export {
  SCREEN_SHARE_RESOLUTIONS,
  SCREEN_SHARE_FPS_OPTIONS,
  DEFAULT_SCREEN_SHARE_CONFIG,
  SCREEN_SHARE_DIALOG_TOKENS,
  normalizeResolution,
  normalizeFps,
  filterSourcesByCategory,
  formatSourceItem,
  createScreenShareDialogController,
  openScreenShareDialog,
  openScreenSharePicker,
} from './screen-share-dialog.js';

import {
  createScreenShareDialogController,
} from './screen-share-dialog.js';

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
 *   onStartStream?: (payload: StartStreamPayload) => void,
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
      activeDialogEl = null;
    }

    const controller = createScreenShareDialogController(containerEl, {
      ...pickerOptions,
      onStartStream: (payload) => {
        if (typeof pickerOptions.onStartStream === 'function') {
          pickerOptions.onStartStream(payload);
        } else if (typeof options.onStartStream === 'function') {
          options.onStartStream(payload);
        }
      },
    });

    activeDialogEl = controller.element;
    const origClose = controller.close;
    controller.close = () => {
      if (activeDialogEl === controller.element) {
        activeDialogEl = null;
      }
      origClose();
    };

    return controller;
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
