// @ts-check
/**
 * TASK-616 (RW-40 / G2): Компонент плеера стрима и режим Picture-in-Picture (PiP).
 *
 * Требования координатора:
 * 1. Контейнер видеопотока с кнопками управления: Play/Pause, Mute/Unmute, Fullscreen, Toggle PiP.
 * 2. Режим PiP (Picture-in-Picture) в углу экрана при переключении каналов.
 * 3. Бейдж качества/FPS (720p 60fps) и статус DAVE MLS шифрования (замочек 'End-to-End Encrypted').
 *
 * Нормативные источники и спецификации:
 * - `waves/RW-40/DESIGN/VOICE-STREAM-UI-SPEC.md` §2.2.3, §2.3.1, §3.1, §4, §5, §6;
 * - `/home/sanya/re-deep/voice-deep.md`;
 * - `/home/sanya/re-deep/frontend-d03.md` §1;
 * - `docs/re/backend-ipc.json` (`DISCORD_WINDOW_OPEN_DOCUMENT_PIP`);
 * - `rust/gui-spike/ui/tokens.css` (100% token-binding).
 *
 * @module ui/features/stream-viewer
 */

/**
 * Допустимые режимы отображения просмотрщика стрима.
 */
export const STREAM_VIEWER_MODES = Object.freeze(
  /** @type {const} */ (['stage', 'pip', 'fullscreen', 'hidden']),
);

/**
 * Пределы и значения по умолчанию для масштабирования видеопотока.
 */
export const STREAM_VIEWER_ZOOM_LIMITS = Object.freeze({
  MIN: 1.0,
  MAX: 5.0,
  DEFAULT: 1.0,
  DOUBLE_CLICK: 2.25,
  STEP_FACTOR: 1.25,
});

/**
 * Триггер для интеграции с Document PiP / Electron preload (`DISCORD_WINDOW_OPEN_DOCUMENT_PIP`).
 */
export const DOCUMENT_PIP_TRIGGER_ID = '__document_pip_trigger';

/**
 * @typedef {'stage' | 'pip' | 'fullscreen' | 'hidden'} StreamViewerMode
 * @typedef {'idle' | 'connecting' | 'active' | 'paused' | 'reconnecting' | 'failed' | 'ended'} StreamViewerPhase
 *
 * @typedef {{
 *   streamKey?: string,
 *   userId?: string,
 *   displayName?: string,
 *   username?: string,
 *   channelId?: string,
 *   channelName?: string,
 *   guildId?: string,
 *   resolution?: string,
 *   fps?: number,
 *   codec?: string,
 *   token?: string,
 *   endpoint?: string,
 *   encrypted?: boolean,
 *   encryptionProtocol?: string,
 *   paused?: boolean,
 *   transportReady?: boolean,
 * }} StreamViewerData
 *
 * @typedef {{
 *   onWatchStream?: (streamKeyOrUserId: string) => void | Promise<unknown>,
 *   onLeaveWatch?: (streamKeyOrUserId?: string) => void | Promise<unknown>,
 *   onPauseToggle?: (paused: boolean) => void | Promise<unknown>,
 *   onModeChange?: (mode: StreamViewerMode) => void,
 *   onAudioChange?: (audio: { muted: boolean, volumePercent: number }) => void,
 *   service?: {
 *     voice_stream_watch?: (keyOrUserId: string) => Promise<unknown>,
 *     voice_stream_leave_watch?: (keyOrUserId?: string) => Promise<unknown>,
 *     voice_stream_set_paused?: (paused: boolean) => Promise<unknown>,
 *     voice_set_audio_device?: (args: any) => Promise<unknown>,
 *   } | null,
 * }} StreamViewerOptions
 */

/**
 * Нормализует масштаб видеопотока к допустимому диапазону `1.0..5.0`.
 *
 * @param {unknown} value
 * @returns {number}
 */
export function clampStreamZoomScale(value) {
  const num = Number(value);
  if (!Number.isFinite(num)) {
    return STREAM_VIEWER_ZOOM_LIMITS.DEFAULT;
  }
  return Math.max(STREAM_VIEWER_ZOOM_LIMITS.MIN, Math.min(STREAM_VIEWER_ZOOM_LIMITS.MAX, num));
}

/**
 * Форматирует отображаемую строку громкости звука трансляции.
 *
 * @param {number} volumePct
 * @param {boolean} isMuted
 * @returns {string}
 */
export function formatStreamVolumeLabel(volumePct, isMuted) {
  if (isMuted || volumePct <= 0) {
    return 'ВЫКЛ';
  }
  return `${Math.round(volumePct)}%`;
}

/**
 * Возвращает фабрику DOM-элементов из контейнера или глобального документа.
 *
 * @param {any} containerEl
 * @returns {any}
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
 * Создаёт контроллер интерфейса плеера стрима и режима PiP.
 *
 * @param {any} containerEl
 * @param {StreamViewerOptions} [options]
 */
export function createStreamViewerController(containerEl, options = {}) {
  const doc = resolveDocument(containerEl);
  const bodyEl = doc.body ?? doc.documentElement ?? containerEl;

  /** @type {StreamViewerMode} */
  let currentMode = 'hidden';
  /** @type {StreamViewerPhase} */
  let currentPhase = 'idle';
  let isPaused = false;
  let isTransportReady = false;
  let isEncrypted = true;

  /** @type {StreamViewerData | null} */
  let activeStream = null;

  let streamVolumePct = 100;
  let isAudioMuted = false;
  let prevUnmutedVolumePct = 100;

  /** @type {number} */
  let zoomScale = STREAM_VIEWER_ZOOM_LIMITS.DEFAULT;
  let panX = 0;
  let panY = 0;

  /* ------------------------------------------------------------------ *
   * Построение узлов разметки: Оверлей просмотра (#stream-viewer-overlay)
   * ------------------------------------------------------------------ */

  const overlayEl = doc.createElement('section');
  overlayEl.id = 'stream-viewer-overlay';
  overlayEl.className = 'stream-viewer-overlay';
  overlayEl.setAttribute('role', 'region');
  overlayEl.setAttribute('aria-label', 'Просмотр трансляции');
  overlayEl.setAttribute('data-phase', 'idle');
  overlayEl.setAttribute('data-paused', 'false');
  overlayEl.setAttribute('data-transport-ready', 'false');
  overlayEl.setAttribute('data-encrypted', 'true');
  overlayEl.setAttribute('data-user-id', '');
  overlayEl.setAttribute('hidden', '');
  overlayEl.hidden = true;

  const canvasEl = doc.createElement('div');
  canvasEl.id = 'stream-viewer-canvas';
  canvasEl.className = 'stream-viewer-canvas';
  canvasEl.setAttribute('role', 'img');
  canvasEl.setAttribute('aria-label', 'Окно просмотра трансляции');
  canvasEl.setAttribute('data-empty', 'true');

  const statusBannerEl = doc.createElement('p');
  statusBannerEl.id = 'stream-viewer-status-banner';
  statusBannerEl.className = 'stream-viewer-status-banner';
  statusBannerEl.setAttribute('role', 'status');
  statusBannerEl.setAttribute('aria-live', 'polite');
  statusBannerEl.textContent = 'Смотрит стрим';
  canvasEl.appendChild(statusBannerEl);

  const controlsEl = doc.createElement('footer');
  controlsEl.id = 'stream-viewer-controls';
  controlsEl.className = 'stream-viewer-controls';
  controlsEl.setAttribute('role', 'toolbar');
  controlsEl.setAttribute('aria-label', 'Управление просмотром трансляции');

  // Информация о стримере и бейджи качества/шифрования
  const streamerInfoEl = doc.createElement('div');
  streamerInfoEl.className = 'stream-viewer-streamer-info';

  const streamerNameEl = doc.createElement('span');
  streamerNameEl.id = 'stream-viewer-streamer-name';
  streamerNameEl.className = 'stream-viewer-streamer-name';

  const phaseBadgeEl = doc.createElement('span');
  phaseBadgeEl.id = 'stream-viewer-phase-badge';
  phaseBadgeEl.className = 'stream-live-badge';
  phaseBadgeEl.setAttribute('data-state', 'watching');
  phaseBadgeEl.textContent = 'Смотрит стрим';

  // Бейдж качества / FPS (720p 60fps)
  const qualityBadgeEl = doc.createElement('span');
  qualityBadgeEl.id = 'stream-viewer-quality-badge';
  qualityBadgeEl.className = 'stream-quality-badge';
  qualityBadgeEl.textContent = '720p • 60 FPS';

  // Статус DAVE MLS шифрования (замочек 'End-to-End Encrypted')
  const encryptionBadgeEl = doc.createElement('span');
  encryptionBadgeEl.id = 'stream-viewer-encryption-badge';
  encryptionBadgeEl.className = 'stream-encryption-badge';
  encryptionBadgeEl.setAttribute('data-encrypted', 'true');
  encryptionBadgeEl.setAttribute('title', 'DAVE MLS Protocol: End-to-End Encrypted');
  encryptionBadgeEl.textContent = '🔒 End-to-End Encrypted';

  streamerInfoEl.appendChild(streamerNameEl);
  streamerInfoEl.appendChild(phaseBadgeEl);
  streamerInfoEl.appendChild(qualityBadgeEl);
  streamerInfoEl.appendChild(encryptionBadgeEl);

  // Контролы воспроизведения и звука (Play/Pause, Mute/Unmute, Volume)
  const audioGroupEl = doc.createElement('div');
  audioGroupEl.className = 'stream-viewer-audio-group';
  audioGroupEl.setAttribute('role', 'group');
  audioGroupEl.setAttribute('aria-label', 'Воспроизведение и звук');

  // Кнопка Play/Pause
  const playPauseBtn = doc.createElement('button');
  playPauseBtn.id = 'stream-viewer-play-pause';
  playPauseBtn.className = 'stream-btn stream-btn--secondary';
  playPauseBtn.setAttribute('type', 'button');
  playPauseBtn.setAttribute('data-stream-action', 'play-pause');
  playPauseBtn.setAttribute('aria-pressed', 'false');
  playPauseBtn.setAttribute('aria-label', 'Приостановить воспроизведение');
  playPauseBtn.textContent = '⏸ Пауза';

  // Кнопка Mute/Unmute
  const muteToggleBtn = doc.createElement('button');
  muteToggleBtn.id = 'stream-viewer-mute-toggle';
  muteToggleBtn.className = 'stream-btn stream-btn--secondary';
  muteToggleBtn.setAttribute('type', 'button');
  muteToggleBtn.setAttribute('data-stream-action', 'mute');
  muteToggleBtn.setAttribute('aria-pressed', 'false');
  muteToggleBtn.setAttribute('aria-label', 'Отключить звук трансляции');
  muteToggleBtn.textContent = '🔊';

  const volumeSliderEl = doc.createElement('input');
  volumeSliderEl.id = 'stream-viewer-volume-slider';
  volumeSliderEl.className = 'stream-volume-slider';
  volumeSliderEl.setAttribute('type', 'range');
  volumeSliderEl.setAttribute('min', '0');
  volumeSliderEl.setAttribute('max', '200');
  volumeSliderEl.setAttribute('step', '1');
  volumeSliderEl.setAttribute('value', '100');
  volumeSliderEl.setAttribute('data-stream-action', 'volume');
  volumeSliderEl.setAttribute('aria-label', 'Громкость трансляции');

  const volumeLabelEl = doc.createElement('span');
  volumeLabelEl.id = 'stream-viewer-volume-label';
  volumeLabelEl.className = 'stream-volume-label';
  volumeLabelEl.textContent = '100%';

  audioGroupEl.appendChild(playPauseBtn);
  audioGroupEl.appendChild(muteToggleBtn);
  audioGroupEl.appendChild(volumeSliderEl);
  audioGroupEl.appendChild(volumeLabelEl);

  // Контролы масштабирования
  const zoomGroupEl = doc.createElement('div');
  zoomGroupEl.className = 'stream-viewer-zoom-group';
  zoomGroupEl.setAttribute('role', 'group');
  zoomGroupEl.setAttribute('aria-label', 'Масштаб изображения');

  const zoomOutBtn = doc.createElement('button');
  zoomOutBtn.id = 'stream-viewer-zoom-out';
  zoomOutBtn.className = 'stream-btn stream-btn--secondary';
  zoomOutBtn.setAttribute('type', 'button');
  zoomOutBtn.setAttribute('data-stream-action', 'zoom-out');
  zoomOutBtn.setAttribute('aria-label', 'Отдалить');
  zoomOutBtn.textContent = '−';

  const zoomResetBtn = doc.createElement('button');
  zoomResetBtn.id = 'stream-viewer-zoom-reset';
  zoomResetBtn.className = 'stream-btn stream-btn--secondary';
  zoomResetBtn.setAttribute('type', 'button');
  zoomResetBtn.setAttribute('data-stream-action', 'zoom-reset');
  zoomResetBtn.setAttribute('aria-label', 'Сбросить масштаб');
  zoomResetBtn.textContent = '100%';

  const zoomInBtn = doc.createElement('button');
  zoomInBtn.id = 'stream-viewer-zoom-in';
  zoomInBtn.className = 'stream-btn stream-btn--secondary';
  zoomInBtn.setAttribute('type', 'button');
  zoomInBtn.setAttribute('data-stream-action', 'zoom-in');
  zoomInBtn.setAttribute('aria-label', 'Приблизить');
  zoomInBtn.textContent = '+';

  const zoom200Btn = doc.createElement('button');
  zoom200Btn.id = 'stream-viewer-zoom-200';
  zoom200Btn.className = 'stream-btn stream-btn--secondary';
  zoom200Btn.setAttribute('type', 'button');
  zoom200Btn.setAttribute('data-stream-action', 'zoom-200');
  zoom200Btn.setAttribute('aria-label', 'Увеличить до 200%');
  zoom200Btn.textContent = '200%';

  zoomGroupEl.appendChild(zoomOutBtn);
  zoomGroupEl.appendChild(zoomResetBtn);
  zoomGroupEl.appendChild(zoomInBtn);
  zoomGroupEl.appendChild(zoom200Btn);

  // Основные действия зрителя: Toggle PiP, Fullscreen, Retry, Leave
  const buttonsGroupEl = doc.createElement('div');
  buttonsGroupEl.className = 'stream-viewer-buttons';
  buttonsGroupEl.setAttribute('role', 'group');
  buttonsGroupEl.setAttribute('aria-label', 'Действия трансляции');

  const pipToggleBtn = doc.createElement('button');
  pipToggleBtn.id = 'stream-viewer-pip-toggle';
  pipToggleBtn.className = 'stream-btn stream-btn--secondary';
  pipToggleBtn.setAttribute('type', 'button');
  pipToggleBtn.setAttribute('data-stream-action', 'pip');
  pipToggleBtn.setAttribute('aria-label', 'В режим «Картинка в картинке»');
  pipToggleBtn.textContent = '⧉ Мини-плеер';

  const fullscreenToggleBtn = doc.createElement('button');
  fullscreenToggleBtn.id = 'stream-fullscreen-toggle';
  fullscreenToggleBtn.className = 'stream-btn stream-btn--secondary';
  fullscreenToggleBtn.setAttribute('type', 'button');
  fullscreenToggleBtn.setAttribute('data-stream-action', 'fullscreen');
  fullscreenToggleBtn.setAttribute('aria-pressed', 'false');
  fullscreenToggleBtn.setAttribute('aria-label', 'Открыть на весь экран');
  fullscreenToggleBtn.textContent = '⛶ На весь экран';

  const retryBtn = doc.createElement('button');
  retryBtn.id = 'stream-viewer-retry';
  retryBtn.className = 'stream-btn stream-btn--secondary';
  retryBtn.setAttribute('type', 'button');
  retryBtn.setAttribute('data-stream-action', 'retry');
  retryBtn.setAttribute('aria-label', 'Переподключить стрим');
  retryBtn.textContent = '↻ Переподключить';

  const leaveWatchBtn = doc.createElement('button');
  leaveWatchBtn.id = 'stream-leave-watch';
  leaveWatchBtn.className = 'stream-btn stream-btn--danger';
  leaveWatchBtn.setAttribute('type', 'button');
  leaveWatchBtn.setAttribute('data-stream-action', 'leave-watch');
  leaveWatchBtn.setAttribute('aria-label', 'Прекратить просмотр стрима');
  leaveWatchBtn.textContent = 'Выйти из просмотра';

  buttonsGroupEl.appendChild(pipToggleBtn);
  buttonsGroupEl.appendChild(fullscreenToggleBtn);
  buttonsGroupEl.appendChild(retryBtn);
  buttonsGroupEl.appendChild(leaveWatchBtn);

  controlsEl.appendChild(streamerInfoEl);
  controlsEl.appendChild(audioGroupEl);
  controlsEl.appendChild(zoomGroupEl);
  controlsEl.appendChild(buttonsGroupEl);

  overlayEl.appendChild(canvasEl);
  overlayEl.appendChild(controlsEl);
  containerEl.appendChild(overlayEl);

  /* ------------------------------------------------------------------ *
   * Полноэкранный модальный контейнер (#stream-fullscreen)
   * ------------------------------------------------------------------ */

  const fullscreenModalEl = doc.createElement('div');
  fullscreenModalEl.id = 'stream-fullscreen';
  fullscreenModalEl.className = 'stream-fullscreen';
  fullscreenModalEl.setAttribute('role', 'dialog');
  fullscreenModalEl.setAttribute('aria-modal', 'true');
  fullscreenModalEl.setAttribute('aria-label', 'Полноэкранный режим');
  fullscreenModalEl.setAttribute('data-controls-visible', 'true');
  fullscreenModalEl.setAttribute('hidden', '');
  fullscreenModalEl.hidden = true;

  const fullscreenViewportEl = doc.createElement('div');
  fullscreenViewportEl.id = 'stream-fullscreen-viewport';
  fullscreenViewportEl.className = 'stream-fullscreen-viewport';
  fullscreenViewportEl.setAttribute('data-empty', 'true');

  const fullscreenBarEl = doc.createElement('footer');
  fullscreenBarEl.id = 'stream-fullscreen-bar';
  fullscreenBarEl.className = 'stream-fullscreen-bar';
  fullscreenBarEl.setAttribute('role', 'toolbar');
  fullscreenBarEl.setAttribute('aria-label', 'Контролы полноэкранного режима');

  const fullscreenTitleEl = doc.createElement('span');
  fullscreenTitleEl.id = 'stream-fullscreen-title';
  fullscreenTitleEl.className = 'stream-fullscreen-title';

  const fullscreenActionsEl = doc.createElement('div');
  fullscreenActionsEl.className = 'stream-fullscreen-actions';

  const fullscreenCloseBtn = doc.createElement('button');
  fullscreenCloseBtn.id = 'stream-fullscreen-close';
  fullscreenCloseBtn.className = 'stream-btn stream-btn--danger';
  fullscreenCloseBtn.setAttribute('type', 'button');
  fullscreenCloseBtn.setAttribute('data-stream-action', 'exit-fullscreen');
  fullscreenCloseBtn.setAttribute('aria-label', 'Выйти из полноэкранного режима');
  fullscreenCloseBtn.textContent = 'Закрыть';

  fullscreenActionsEl.appendChild(fullscreenCloseBtn);
  fullscreenBarEl.appendChild(fullscreenTitleEl);
  fullscreenBarEl.appendChild(fullscreenActionsEl);

  fullscreenModalEl.appendChild(fullscreenViewportEl);
  fullscreenModalEl.appendChild(fullscreenBarEl);
  bodyEl.appendChild(fullscreenModalEl);

  /* ------------------------------------------------------------------ *
   * Document PiP триггер (__document_pip_trigger)
   * ------------------------------------------------------------------ */

  let docPipTrigger = doc.getElementById(DOCUMENT_PIP_TRIGGER_ID);
  if (!docPipTrigger) {
    docPipTrigger = doc.createElement('button');
    docPipTrigger.id = DOCUMENT_PIP_TRIGGER_ID;
    docPipTrigger.setAttribute('hidden', '');
    docPipTrigger.setAttribute('type', 'button');
    docPipTrigger.setAttribute('aria-hidden', 'true');
    docPipTrigger.hidden = true;
    bodyEl.appendChild(docPipTrigger);
  }

  /* ------------------------------------------------------------------ *
   * Синхронизация состояния и вспомогательные методы
   * ------------------------------------------------------------------ */

  /** @type {any | null} */
  let activePipEl = null;

  /**
   * Синхронизирует отображение контролов звука.
   */
  function syncAudioUi() {
    const isMutedEffective = isAudioMuted || streamVolumePct === 0;
    muteToggleBtn.textContent = isMutedEffective ? '🔇' : streamVolumePct < 60 ? '🔉' : '🔊';
    muteToggleBtn.setAttribute('aria-pressed', isMutedEffective ? 'true' : 'false');
    volumeSliderEl.value = String(isMutedEffective ? 0 : streamVolumePct);
    volumeLabelEl.textContent = formatStreamVolumeLabel(streamVolumePct, isAudioMuted);

    if (activePipEl) {
      const pipMute = activePipEl.querySelector('#stream-viewer-pip-mute');
      if (pipMute) {
        pipMute.textContent = isMutedEffective ? '🔇' : '🔊';
        pipMute.setAttribute('aria-pressed', isMutedEffective ? 'true' : 'false');
      }
    }

    if (typeof options.onAudioChange === 'function') {
      options.onAudioChange({
        muted: isMutedEffective,
        volumePercent: streamVolumePct,
      });
    }

    if (options.service?.voice_set_audio_device) {
      void options.service
        .voice_set_audio_device({
          kind: 'stream_audio_volume',
          muted: isMutedEffective,
          volumePercent: streamVolumePct,
        })
        .catch(() => {});
    }
  }

  /**
   * Применяет геометрическую трансформацию к холсту просмотра.
   */
  function applyTransform() {
    zoomScale = clampStreamZoomScale(zoomScale);
    zoomResetBtn.textContent = `${Math.round(zoomScale * 100)}%`;
    const tf = `translate3d(${Math.round(panX)}px, ${Math.round(panY)}px, 0) scale(${zoomScale.toFixed(3)})`;
    canvasEl.style.transform = tf;
    canvasEl.style.cursor = zoomScale > 1.01 ? 'grab' : 'zoom-in';
  }

  /**
   * Монтирует плавающий мини-плеер Picture-in-Picture.
   */
  function mountPipWindow() {
    if (activePipEl) {
      activePipEl.remove();
      activePipEl = null;
    }

    const pip = doc.createElement('div');
    pip.id = 'stream-viewer-pip';
    pip.className = 'stream-viewer-pip';
    pip.setAttribute('role', 'region');
    pip.setAttribute('aria-label', 'Мини-плеер трансляции');
    pip.setAttribute('data-pip-active', 'true');

    // Шапка мини-плеера
    const pipHeader = doc.createElement('header');
    pipHeader.className = 'stream-pip-header';

    const pipMeta = doc.createElement('div');
    pipMeta.className = 'stream-pip-meta';

    const pipLiveBadge = doc.createElement('span');
    pipLiveBadge.className = 'stream-live-badge stream-pip-live-badge';
    pipLiveBadge.textContent = 'В ЭФИРЕ';

    const pipTitle = doc.createElement('span');
    pipTitle.className = 'stream-pip-title';
    pipTitle.textContent =
      activeStream?.displayName || activeStream?.username || 'Трансляция';

    const pipChannel = doc.createElement('span');
    pipChannel.className = 'stream-pip-channel';
    pipChannel.textContent = activeStream?.channelName || 'Голосовой канал';

    pipMeta.appendChild(pipLiveBadge);
    pipMeta.appendChild(pipTitle);
    pipMeta.appendChild(pipChannel);

    const pipHeaderActions = doc.createElement('div');
    pipHeaderActions.className = 'stream-pip-header-actions';

    const pipExpandBtn = doc.createElement('button');
    pipExpandBtn.id = 'stream-viewer-pip-expand';
    pipExpandBtn.className = 'stream-btn stream-btn--secondary';
    pipExpandBtn.setAttribute('type', 'button');
    pipExpandBtn.setAttribute('data-stream-action', 'expand');
    pipExpandBtn.setAttribute('aria-label', 'Развернуть стрим');
    pipExpandBtn.textContent = '⛶';
    pipExpandBtn.addEventListener('click', (/** @type {any} */ ev) => {
      ev.stopPropagation();
      setMode('stage');
    });

    pipHeaderActions.appendChild(pipExpandBtn);
    pipHeader.appendChild(pipMeta);
    pipHeader.appendChild(pipHeaderActions);

    // Вьюпорт мини-плеера
    const pipViewport = doc.createElement('div');
    pipViewport.className = 'stream-pip-viewport';
    pipViewport.setAttribute('data-empty', 'true');
    pipViewport.addEventListener('click', () => {
      setMode('stage');
    });

    const pipCanvas = doc.createElement('div');
    pipCanvas.className = 'stream-pip-canvas';
    pipViewport.appendChild(pipCanvas);

    // Подвал мини-плеера
    const pipFooter = doc.createElement('footer');
    pipFooter.className = 'stream-pip-footer';

    const pipFooterActions = doc.createElement('div');
    pipFooterActions.className = 'stream-pip-footer-actions';

    const pipMuteBtn = doc.createElement('button');
    pipMuteBtn.id = 'stream-viewer-pip-mute';
    pipMuteBtn.className = 'stream-btn stream-btn--secondary';
    pipMuteBtn.setAttribute('type', 'button');
    pipMuteBtn.setAttribute('data-stream-action', 'mute');
    pipMuteBtn.setAttribute('aria-pressed', isAudioMuted ? 'true' : 'false');
    pipMuteBtn.textContent = isAudioMuted ? '🔇' : '🔊';
    pipMuteBtn.addEventListener('click', (/** @type {any} */ ev) => {
      ev.stopPropagation();
      toggleMute();
    });

    const pipCloseBtn = doc.createElement('button');
    pipCloseBtn.id = 'stream-viewer-pip-close';
    pipCloseBtn.className = 'stream-btn stream-btn--danger';
    pipCloseBtn.setAttribute('type', 'button');
    pipCloseBtn.setAttribute('data-stream-action', 'leave-watch');
    pipCloseBtn.setAttribute('aria-label', 'Прекратить просмотр');
    pipCloseBtn.textContent = '✕';
    pipCloseBtn.addEventListener('click', (/** @type {any} */ ev) => {
      ev.stopPropagation();
      leaveWatch();
    });

    pipFooterActions.appendChild(pipMuteBtn);
    pipFooterActions.appendChild(pipCloseBtn);
    pipFooter.appendChild(pipFooterActions);

    pip.appendChild(pipHeader);
    pip.appendChild(pipViewport);
    pip.appendChild(pipFooter);

    bodyEl.appendChild(pip);
    activePipEl = pip;
  }

  /**
   * Снимает и удаляет мини-плеер Picture-in-Picture.
   */
  function unmountPipWindow() {
    if (activePipEl) {
      activePipEl.remove();
      activePipEl = null;
    }
  }

  /**
   * Переключает режим отображения ('stage' | 'pip' | 'fullscreen' | 'hidden').
   *
   * @param {StreamViewerMode} nextMode
   */
  function setMode(nextMode) {
    if (!STREAM_VIEWER_MODES.includes(nextMode)) return;
    const prevMode = currentMode;
    currentMode = nextMode;

    if (currentMode === 'stage') {
      unmountPipWindow();
      fullscreenModalEl.setAttribute('hidden', '');
      fullscreenModalEl.hidden = true;
      overlayEl.removeAttribute('hidden');
      overlayEl.hidden = false;
      fullscreenToggleBtn.setAttribute('aria-pressed', 'false');
      applyTransform();
    } else if (currentMode === 'pip') {
      overlayEl.setAttribute('hidden', '');
      overlayEl.hidden = true;
      fullscreenModalEl.setAttribute('hidden', '');
      fullscreenModalEl.hidden = true;
      mountPipWindow();
    } else if (currentMode === 'fullscreen') {
      unmountPipWindow();
      overlayEl.removeAttribute('hidden');
      overlayEl.hidden = false;
      fullscreenModalEl.removeAttribute('hidden');
      fullscreenModalEl.hidden = false;
      fullscreenToggleBtn.setAttribute('aria-pressed', 'true');
      fullscreenTitleEl.textContent =
        activeStream?.displayName || activeStream?.username || 'Трансляция';
    } else if (currentMode === 'hidden') {
      unmountPipWindow();
      overlayEl.setAttribute('hidden', '');
      overlayEl.hidden = true;
      fullscreenModalEl.setAttribute('hidden', '');
      fullscreenModalEl.hidden = true;
    }

    if (prevMode !== currentMode && typeof options.onModeChange === 'function') {
      options.onModeChange(currentMode);
    }
  }

  /**
   * Переключает режим Picture-in-Picture.
   *
   * @param {boolean} [force]
   */
  function togglePiP(force) {
    const shouldBePip = force !== undefined ? Boolean(force) : currentMode !== 'pip';
    setMode(shouldBePip ? 'pip' : 'stage');
  }

  /**
   * Переключает полноэкранный режим.
   *
   * @param {boolean} [force]
   */
  function toggleFullscreen(force) {
    const shouldBeFs = force !== undefined ? Boolean(force) : currentMode !== 'fullscreen';
    setMode(shouldBeFs ? 'fullscreen' : 'stage');
  }

  /**
   * Открывает просмотрщик чужого стрима.
   *
   * @param {StreamViewerData} streamData
   */
  function openViewer(streamData) {
    activeStream = { ...streamData };
    const rawKey = streamData.streamKey || '';
    const userId = String(streamData.userId || '');
    const dispName = streamData.displayName || streamData.username || 'Участник';

    // Защита от утечки секретов в DOM (T-STREAM-UI-OBS-01)
    overlayEl.setAttribute('data-user-id', userId);
    currentPhase = 'connecting';
    overlayEl.setAttribute('data-phase', currentPhase);
    isPaused = Boolean(streamData.paused);
    overlayEl.setAttribute('data-paused', isPaused ? 'true' : 'false');
    isTransportReady = Boolean(streamData.transportReady);
    overlayEl.setAttribute('data-transport-ready', isTransportReady ? 'true' : 'false');

    isEncrypted = streamData.encrypted !== false;
    overlayEl.setAttribute('data-encrypted', isEncrypted ? 'true' : 'false');
    encryptionBadgeEl.setAttribute('data-encrypted', isEncrypted ? 'true' : 'false');
    encryptionBadgeEl.textContent = isEncrypted
      ? '🔒 End-to-End Encrypted'
      : '🔓 Standard Stream';

    streamerNameEl.textContent = dispName;
    fullscreenTitleEl.textContent = dispName;
    statusBannerEl.textContent = isPaused ? 'Стрим приостановлен' : 'Смотрит стрим';

    if (streamData.resolution || streamData.fps) {
      qualityBadgeEl.textContent = `${streamData.resolution || '720p'} • ${streamData.fps || 60} FPS`;
    }

    setMode('stage');
    syncAudioUi();

    const targetKey = rawKey || userId;
    if (typeof options.onWatchStream === 'function') {
      options.onWatchStream(targetKey);
    }
    if (options.service?.voice_stream_watch) {
      void options.service.voice_stream_watch(targetKey).catch(() => {});
    }
  }

  /**
   * Прекращает просмотр стрима и закрывает окно.
   */
  function leaveWatch() {
    const targetKey = activeStream?.streamKey || activeStream?.userId || '';
    setMode('hidden');
    currentPhase = 'idle';
    activeStream = null;

    if (typeof options.onLeaveWatch === 'function') {
      options.onLeaveWatch(targetKey);
    }
    if (options.service?.voice_stream_leave_watch) {
      void options.service.voice_stream_leave_watch(targetKey).catch(() => {});
    }
  }

  /**
   * Устанавливает статус шифрования потока.
   *
   * @param {boolean} encrypted
   */
  function setEncryption(encrypted) {
    isEncrypted = Boolean(encrypted);
    overlayEl.setAttribute('data-encrypted', isEncrypted ? 'true' : 'false');
    encryptionBadgeEl.setAttribute('data-encrypted', isEncrypted ? 'true' : 'false');
    encryptionBadgeEl.textContent = isEncrypted
      ? '🔒 End-to-End Encrypted'
      : '🔓 Standard Stream';
  }

  /**
   * Устанавливает фазу стрима ('connecting' | 'active' | 'paused' | 'reconnecting' | 'ended' | 'failed').
   *
   * @param {StreamViewerPhase} nextPhase
   */
  function setPhase(nextPhase) {
    currentPhase = nextPhase;
    overlayEl.setAttribute('data-phase', currentPhase);
    if (currentPhase === 'paused') {
      isPaused = true;
      overlayEl.setAttribute('data-paused', 'true');
      playPauseBtn.setAttribute('aria-pressed', 'true');
      playPauseBtn.textContent = '▶ Возобновить';
      statusBannerEl.textContent = 'Стрим приостановлен';
    } else if (currentPhase === 'active') {
      isPaused = false;
      overlayEl.setAttribute('data-paused', 'false');
      playPauseBtn.setAttribute('aria-pressed', 'false');
      playPauseBtn.textContent = '⏸ Пауза';
      statusBannerEl.textContent = 'Смотрит стрим';
    }
  }

  /**
   * Устанавливает флаг паузы стрима.
   *
   * @param {boolean} paused
   */
  function setPaused(paused) {
    isPaused = Boolean(paused);
    overlayEl.setAttribute('data-paused', isPaused ? 'true' : 'false');
    playPauseBtn.setAttribute('aria-pressed', isPaused ? 'true' : 'false');
    playPauseBtn.textContent = isPaused ? '▶ Возобновить' : '⏸ Пауза';
    setPhase(isPaused ? 'paused' : 'active');
    if (typeof options.onPauseToggle === 'function') {
      options.onPauseToggle(isPaused);
    }
    if (options.service?.voice_stream_set_paused) {
      void options.service.voice_stream_set_paused(isPaused).catch(() => {});
    }
  }

  /**
   * Переключает воспроизведение/паузу.
   */
  function togglePlayPause() {
    setPaused(!isPaused);
  }

  /**
   * Устанавливает готовность RTC транспорта.
   *
   * @param {boolean} ready
   */
  function setTransportReady(ready) {
    isTransportReady = Boolean(ready);
    overlayEl.setAttribute('data-transport-ready', isTransportReady ? 'true' : 'false');
  }

  /**
   * Устанавливает уровень громкости звука трансляции (0..200%).
   *
   * @param {number} volume
   */
  function setVolume(volume) {
    const clamped = Math.max(0, Math.min(200, Number(volume) || 0));
    streamVolumePct = clamped;
    if (clamped === 0) {
      isAudioMuted = true;
    } else {
      isAudioMuted = false;
      prevUnmutedVolumePct = clamped;
    }
    syncAudioUi();
  }

  /**
   * Переключает mute звука трансляции.
   */
  function toggleMute() {
    if (!isAudioMuted) {
      if (streamVolumePct > 0) prevUnmutedVolumePct = streamVolumePct;
      isAudioMuted = true;
    } else {
      isAudioMuted = false;
      if (streamVolumePct === 0) streamVolumePct = prevUnmutedVolumePct || 100;
    }
    syncAudioUi();
  }

  /**
   * Увеличивает масштаб видео.
   */
  function zoomIn() {
    zoomScale = clampStreamZoomScale(zoomScale * STREAM_VIEWER_ZOOM_LIMITS.STEP_FACTOR);
    applyTransform();
  }

  /**
   * Уменьшает масштаб видео.
   */
  function zoomOut() {
    zoomScale = clampStreamZoomScale(zoomScale / STREAM_VIEWER_ZOOM_LIMITS.STEP_FACTOR);
    applyTransform();
  }

  /**
   * Сбрасывает масштаб к 100%.
   */
  function resetZoom() {
    zoomScale = STREAM_VIEWER_ZOOM_LIMITS.DEFAULT;
    panX = 0;
    panY = 0;
    applyTransform();
  }

  /**
   * Устанавливает точное значение масштаба и смещения.
   *
   * @param {number} scale
   * @param {number} [targetPanX]
   * @param {number} [targetPanY]
   */
  function setZoom(scale, targetPanX = 0, targetPanY = 0) {
    zoomScale = clampStreamZoomScale(scale);
    panX = targetPanX;
    panY = targetPanY;
    applyTransform();
  }

  /**
   * Реагирует на переключение активного канала пользователем в интерфейсе.
   * Если стрим просматривается в `stage`, автоматически сворачивает его в `pip`.
   *
   * @param {string} channelId
   */
  function onChannelSwitch(channelId) {
    if (!channelId || currentMode !== 'stage') return;
    if (activeStream?.channelId && activeStream.channelId !== channelId) {
      setMode('pip');
    }
  }

  /* ------------------------------------------------------------------ *
   * Привязка обработчиков событий
   * ------------------------------------------------------------------ */

  playPauseBtn.addEventListener('click', () => togglePlayPause());
  muteToggleBtn.addEventListener('click', () => toggleMute());
  volumeSliderEl.addEventListener('input', () => {
    setVolume(Number(volumeSliderEl.value));
  });

  zoomOutBtn.addEventListener('click', () => zoomOut());
  zoomInBtn.addEventListener('click', () => zoomIn());
  zoomResetBtn.addEventListener('click', () => resetZoom());
  zoom200Btn.addEventListener('click', () => {
    if (Math.abs(zoomScale - 2.0) < 0.1) {
      resetZoom();
    } else {
      setZoom(2.0);
    }
  });

  pipToggleBtn.addEventListener('click', () => togglePiP());
  fullscreenToggleBtn.addEventListener('click', () => toggleFullscreen());
  fullscreenCloseBtn.addEventListener('click', () => setMode('stage'));

  retryBtn.addEventListener('click', () => {
    if (activeStream) {
      openViewer(activeStream);
    }
  });

  leaveWatchBtn.addEventListener('click', () => leaveWatch());

  // Двойной клик по холсту масштабирует до 225% или сбрасывает к 100%
  canvasEl.addEventListener('dblclick', () => {
    if (zoomScale > 1.15) {
      resetZoom();
    } else {
      setZoom(STREAM_VIEWER_ZOOM_LIMITS.DOUBLE_CLICK);
    }
  });

  // Привязка Document PiP триггера
  const onDocumentPipClick = () => {
    if (currentMode === 'stage' || currentMode === 'fullscreen') {
      setMode('pip');
    } else if (currentMode === 'pip') {
      setMode('stage');
    }
  };
  docPipTrigger.addEventListener('click', onDocumentPipClick);

  /**
   * Обработчик клавиатурных клавиш для доступности и хоткеев.
   *
   * @param {{ key: string }} ev
   */
  function handleKeyDown(ev) {
    const key = String(ev?.key || '');
    if (key === 'Escape') {
      if (currentMode === 'fullscreen') {
        setMode('stage');
      } else if (currentMode === 'stage') {
        leaveWatch();
      }
    } else if (key === 'p' || key === 'P') {
      togglePiP();
    } else if (key === 'f' || key === 'F') {
      toggleFullscreen();
    } else if (key === 'm' || key === 'M') {
      toggleMute();
    } else if (key === ' ') {
      togglePlayPause();
    } else if (key === '+' || key === '=') {
      zoomIn();
    } else if (key === '-') {
      zoomOut();
    } else if (key === '0') {
      resetZoom();
    }
  }

  return {
    overlayEl,
    fullscreenModalEl,
    getMode() {
      return currentMode;
    },
    setMode,
    togglePiP,
    toggleFullscreen,
    openViewer,
    leaveWatch,
    setPhase,
    setPaused,
    togglePlayPause,
    setTransportReady,
    setEncryption,
    isEncrypted() {
      return isEncrypted;
    },
    setVolume,
    toggleMute,
    getAudioState() {
      return {
        muted: isAudioMuted,
        volumePercent: streamVolumePct,
      };
    },
    zoomIn,
    zoomOut,
    resetZoom,
    setZoom,
    getZoomState() {
      return {
        scale: zoomScale,
        panX,
        panY,
      };
    },
    onChannelSwitch,
    handleKeyDown,
    dispose() {
      unmountPipWindow();
      docPipTrigger.removeEventListener('click', onDocumentPipClick);
      overlayEl.remove();
      fullscreenModalEl.remove();
    },
  };
}
