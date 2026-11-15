// @ts-check
/**
 * TAURI-MODAL-01 (RW-50 / G2): Модульный контроллер диалога и портала демонстрации экрана (`screen-share-dialog.js`).
 *
 * Обеспечивает:
 * - Канонические константы разрешений (720p, 1080p, 1440p), FPS (30, 60) и дизайн-токены.
 * - Чистые функции нормализации и фильтрации источников (`normalizeResolution`, `normalizeFps`, `filterSourcesByCategory`).
 * - Контроллер диалога выбора экрана `createScreenShareDialogController` / `openScreenSharePicker`.
 * - Контроллер портала выбора экрана/окна `createScreenSharePortalPicker` / `openScreenSharePortalPicker` (с превью, поиском и rAF-циклом).
 * - Контроллер модального окна настроек стрима `createScreenShareModalController` / `openScreenShareModal`.
 *
 * @module ui/features/screen-share-dialog
 */

/** Допустимые разрешения демонстрации экрана. */
export const SCREEN_SHARE_RESOLUTIONS = Object.freeze(
  /** @type {const} */ (['720p', '1080p', '1440p']),
);

/** Допустимые значения частоты кадров демонстрации экрана. */
export const SCREEN_SHARE_FPS_OPTIONS = Object.freeze(/** @type {const} */ ([30, 60]));

/** Допустимые вкладки портала выбора источника. */
export const SCREEN_SHARE_PORTAL_TABS = Object.freeze(
  /** @type {const} */ (['screens', 'windows']),
);

/** Допустимые вкладки основного модального окна стрима. */
export const SCREEN_SHARE_MODAL_TABS = Object.freeze(
  /** @type {const} */ (['apps', 'devices']),
);

/** Конфигурация по умолчанию для демонстрации экрана. */
export const DEFAULT_SCREEN_SHARE_CONFIG = Object.freeze({
  resolution: /** @type {const} */ ('1080p'),
  fps: /** @type {const} */ (60),
  shareAudio: false,
  category: /** @type {const} */ ('all'),
});

/** Дизайн-токены диалога демонстрации экрана по спецификации Discord. */
export const SCREEN_SHARE_DIALOG_TOKENS = Object.freeze({
  backdropBg: 'rgba(0, 0, 0, 0.68)',
  backdropModalBg: 'rgba(0, 0, 0, 0.78)',
  modalBg: '#26282e',
  modalBorder: '#3f4147',
  cardBg: '#313338',
  cardBgHover: '#35373c',
  accent: '#5865f2',
  accentCyan: '#3daee9',
  textPrimary: '#f2f3f5',
  textMuted: '#949ba4',
  textSubtle: '#dbdee1',
});

/**
 * @typedef {'720p' | '1080p' | '1440p'} ScreenShareResolution
 * @typedef {30 | 60} ScreenShareFps
 * @typedef {'all' | 'screens' | 'windows'} ScreenShareCategory
 *
 * @typedef {{
 *   id: string,
 *   name?: string,
 *   title?: string,
 *   label?: string,
 *   kind?: 'screen' | 'window' | string,
 *   source_type?: 'screen' | 'window' | string,
 *   is_primary?: boolean,
 *   icon?: string,
 *   tab?: 'screens' | 'windows',
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
 *   defaultCategory?: ScreenShareCategory | string,
 *   onStartStream?: (payload: StartStreamPayload) => void,
 * }} ScreenSharePickerOptions
 *
 * @typedef {{
 *   doc?: any,
 *   initialPortalTab?: 'screens' | 'windows',
 *   sources?: ReadonlyArray<ScreenShareSource>,
 *   onSelectSource?: (picked: { id: string, label: string, tab: 'screens' | 'windows' }) => void,
 *   onClose?: () => void,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 * }} ScreenSharePortalOptions
 */

/**
 * Возвращает DOM-документ из контейнера, опций или глобального окружения.
 * @param {any} [target]
 * @returns {any}
 */
export function resolveDocument(target) {
  if (target?.ownerDocument?.createElement) return target.ownerDocument;
  if (target?.doc?.createElement) return target.doc;
  if (typeof target?.createElement === 'function') return target;
  if (typeof globalThis !== 'undefined') {
    const doc = /** @type {any} */ (globalThis).document;
    if (doc?.createElement) return doc;
  }
  throw new TypeError('resolveDocument: Document is not available');
}

/**
 * Заменяет дочерние узлы контейнера переданным списком узлов.
 * @param {any} parentEl
 * @param {any[]} nodes
 */
export function replaceElementChildren(parentEl, nodes) {
  if (typeof parentEl.replaceChildren === 'function') {
    parentEl.replaceChildren(...nodes);
    return;
  }
  if (Array.isArray(parentEl.children)) {
    parentEl.children.length = 0;
  }
  while (parentEl.firstChild) {
    parentEl.removeChild(parentEl.firstChild);
  }
  for (const node of nodes) {
    parentEl.appendChild(node);
  }
}

/**
 * Управляет атрибутом и свойством disabled для кнопки.
 * @param {any} buttonEl
 * @param {boolean} isDisabled
 */
export function setButtonDisabled(buttonEl, isDisabled) {
  buttonEl.disabled = isDisabled;
  if (isDisabled) {
    if (typeof buttonEl.setAttribute === 'function') buttonEl.setAttribute('disabled', '');
  } else if (typeof buttonEl.removeAttribute === 'function') {
    buttonEl.removeAttribute('disabled');
  }
}

/**
 * Нормализует разрешение экрана к допустимому значению (по умолчанию 1080p).
 * @param {unknown} value
 * @returns {ScreenShareResolution}
 */
export function normalizeResolution(value) {
  if (value === '720p' || value === '1080p' || value === '1440p') {
    return value;
  }
  return '1080p';
}

/**
 * Нормализует частоту кадров экрана к 30 или 60 (по умолчанию 60).
 * @param {unknown} value
 * @returns {ScreenShareFps}
 */
export function normalizeFps(value) {
  const num = Number(value);
  if (num === 30 || num === 60) {
    return num;
  }
  return 60;
}

/**
 * Фильтрует источники по категории и опциональной поисковой строке.
 * @param {ReadonlyArray<ScreenShareSource>} sources
 * @param {ScreenShareCategory | string} [category='all']
 * @param {string} [search='']
 * @returns {ScreenShareSource[]}
 */
export function filterSourcesByCategory(sources, category = 'all', search = '') {
  if (!Array.isArray(sources)) return [];
  const q = String(search || '').trim().toLowerCase();

  return sources.filter((s) => {
    if (!s || !s.id) return false;

    if (category !== 'all') {
      const kind = String(s.kind || s.source_type || '').toLowerCase();
      const isScreen =
        kind === 'screen' ||
        kind === 'screens' ||
        String(s.id).startsWith('screen:') ||
        s.tab === 'screens';
      if (category === 'screens' && !isScreen) return false;
      if (category === 'windows' && isScreen) return false;
    }

    if (q) {
      const label = String(s.label || s.name || s.title || s.id).toLowerCase();
      if (!label.includes(q)) return false;
    }

    return true;
  });
}

/**
 * Приводит сырой объект источника к стандартному виду и определяет подходящую иконку.
 * @param {any} raw
 * @returns {ScreenShareSource & { label: string, icon: string }}
 */
export function formatSourceItem(raw) {
  const id = String(raw?.id || '').trim();
  const label = String(raw?.label || raw?.name || raw?.title || id).trim();
  const lower = label.toLowerCase();
  const kind = String(
    raw?.kind ||
    raw?.source_type ||
    (raw?.tab === 'screens' ? 'screen' : raw?.tab === 'windows' ? 'window' : '') ||
    (id.startsWith('screen:') || id.startsWith('screen_') ? 'screen' : 'window')
  );
  const tab = (kind === 'screen' || raw?.tab === 'screens') ? 'screens' : 'windows';

  let icon = '🪟';
  if (kind === 'screen' || tab === 'screens') {
    icon = '🖥';
  } else if (lower.includes('orca')) {
    icon = '🦦';
  } else if (lower.includes('dolphin')) {
    icon = '📁';
  } else if (lower.includes('firefox') || lower.includes('brave') || lower.includes('chrom')) {
    icon = '🌐';
  } else if (lower.includes('discord') || lower.includes('chord')) {
    icon = '🎮';
  }

  return {
    ...raw,
    id,
    label,
    name: label,
    kind,
    tab,
    icon,
  };
}

/**
 * Отрисовывает фиктивный или реальный кадр захвата окна/экрана на холсте canvas.
 * @param {any} canvas
 * @param {string} sourceId
 * @param {number} [tick=0]
 * @param {boolean} [showWallpaper=true]
 */
export function drawCapturePreviewFrame(canvas, sourceId, tick = 0, showWallpaper = true) {
  if (!canvas || typeof canvas.getContext !== 'function') return;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;

  const w = canvas.width || 188;
  const h = canvas.height || 104;

  if (typeof ctx.clearRect === 'function') ctx.clearRect(0, 0, w, h);
  if (typeof ctx.fillRect === 'function') {
    ctx.fillStyle = showWallpaper ? '#1e2025' : '#111214';
    ctx.fillRect(0, 0, w, h);
  }
}

/**
 * Создаёт контроллер интерактивного диалога «Демонстрация экрана» (`createScreenShareDialogController`).
 *
 * @param {any} containerOrOptions
 * @param {ScreenSharePickerOptions} [maybeOptions]
 */
export function createScreenShareDialogController(containerOrOptions, maybeOptions) {
  const hasContainer = containerOrOptions && (containerOrOptions.nodeType || containerOrOptions.createElement || containerOrOptions.children);
  const containerEl = hasContainer ? containerOrOptions : null;
  /** @type {ScreenSharePickerOptions} */
  const options = hasContainer ? (maybeOptions || {}) : (containerOrOptions || {});

  const doc = resolveDocument(containerEl);
  const rawSources = Array.isArray(options.sources) ? [...options.sources] : [];
  let sources = rawSources.filter((s) => s && typeof s.id === 'string' && s.id.length > 0);

  let selectedSourceId = sources[0]?.id ?? '';
  let selectedResolution = normalizeResolution(options.defaultResolution ?? DEFAULT_SCREEN_SHARE_CONFIG.resolution);
  let selectedFps = normalizeFps(options.defaultFps ?? DEFAULT_SCREEN_SHARE_CONFIG.fps);
  let shareAudio = Boolean(options.defaultShareAudio ?? DEFAULT_SCREEN_SHARE_CONFIG.shareAudio);
  /** @type {ScreenShareCategory} */
  let activeCategory =
    options.defaultCategory === 'screens' || options.defaultCategory === 'windows'
      ? options.defaultCategory
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
  /** @type {Map<ScreenShareCategory, any>} */
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
    const filteredSources = filterSourcesByCategory(sources, activeCategory);

    const buttons = [];
    for (const source of filteredSources) {
      const btn = doc.createElement('button');
      btn.setAttribute('type', 'button');
      btn.setAttribute('data-source-id', source.id);
      btn.textContent = source.name ?? source.title ?? source.label ?? source.id;
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
  if (typeof options.loadSources === 'function') {
    loadPromise = options
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
  } else if (!options.sources && typeof globalThis !== 'undefined') {
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
  if (containerEl && typeof containerEl.appendChild === 'function') {
    containerEl.appendChild(dialog);
  }

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
    syncPickerState,
    close() {
      if (typeof dialog.remove === 'function') {
        dialog.remove();
      }
    },
  };
}

/** Алиас для вызова диалога демонстрации экрана. */
export const openScreenSharePicker = createScreenShareDialogController;
export const openScreenShareDialog = createScreenShareDialogController;

/**
 * Создаёт модальное окно выбора конкретного экрана или окна (портал выбора источника).
 *
 * @param {ScreenSharePortalOptions} options
 */
export function createScreenSharePortalPicker(options = {}) {
  const doc = resolveDocument(options.doc);
  doc.getElementById?.('live-screenshare-portal-modal')?.remove?.();

  const backdrop = doc.createElement('div');
  backdrop.id = 'live-screenshare-portal-modal';
  backdrop.style.cssText =
    'position:fixed;inset:0;background:rgba(0,0,0,0.68);display:flex;align-items:center;justify-content:center;z-index:10005;';

  const dialog = doc.createElement('div');
  dialog.setAttribute('role', 'dialog');
  dialog.setAttribute('aria-modal', 'true');
  dialog.setAttribute('aria-label', 'Запрос демонстрации экрана');
  dialog.style.cssText =
    'width:480px;max-width:94vw;background:#26282e;color:#f2f3f5;border:1px solid #3f4147;border-radius:12px;overflow:hidden;box-shadow:0 24px 64px rgba(0,0,0,0.88);display:flex;flex-direction:column;font-family:inherit;';

  /** @type {number | ReturnType<typeof setTimeout> | null} */
  let portalRafId = null;

  const closePortal = () => {
    if (portalRafId !== null) {
      if (typeof cancelAnimationFrame === 'function') {
        cancelAnimationFrame(/** @type {number} */ (portalRafId));
      } else {
        clearTimeout(/** @type {any} */ (portalRafId));
      }
      portalRafId = null;
    }
    if (typeof backdrop.remove === 'function') {
      backdrop.remove();
    }
    options.onClose?.();
  };

  const titleBar = doc.createElement('div');
  titleBar.style.cssText =
    'display:flex;align-items:center;justify-content:space-between;padding:10px 14px;background:#1e2025;border-bottom:1px solid #35373c;';
  const leftTitle = doc.createElement('div');
  leftTitle.style.cssText = 'display:flex;align-items:center;gap:8px;font-size:13px;font-weight:700;color:#dbdee1;';
  leftTitle.innerHTML = `<span style="width:18px;height:18px;border-radius:50%;background:#5865f2;color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:10px;">🎮</span><span>Запрос демонстрации экрана</span>`;
  const closeBtn = doc.createElement('button');
  closeBtn.type = 'button';
  closeBtn.textContent = '✕';
  closeBtn.style.cssText =
    'width:22px;height:22px;border-radius:50%;border:none;background:#35373c;color:#dbdee1;font-size:12px;cursor:pointer;';
  closeBtn.addEventListener('click', closePortal);
  titleBar.append(leftTitle, closeBtn);

  const searchBarWrap = doc.createElement('div');
  searchBarWrap.style.cssText = 'padding:10px 14px 6px;';
  const searchInput = doc.createElement('input');
  searchInput.type = 'search';
  searchInput.setAttribute('type', 'search');
  searchInput.placeholder = '🔍 Поиск...';
  searchInput.style.cssText =
    'width:100%;padding:7px 11px;border-radius:6px;border:1px solid #3daee9;background:#1b1c20;color:#f2f3f5;font-size:13px;outline:none;';
  searchBarWrap.append(searchInput);

  let portalTab = options.initialPortalTab === 'windows' ? 'windows' : 'screens';
  let showWallpaper = true;
  const tabsBar = doc.createElement('div');
  tabsBar.style.cssText = 'display:grid;grid-template-columns:repeat(2,1fr);padding:0 14px;border-bottom:1px solid #35373c;';

  const contentArea = doc.createElement('div');
  contentArea.style.cssText = 'padding:14px;max-height:360px;overflow-y:auto;display:flex;flex-direction:column;gap:12px;';

  const footerArea = doc.createElement('div');
  footerArea.style.cssText = 'padding:10px 14px;border-top:1px solid #35373c;background:#1e2025;display:flex;align-items:center;gap:8px;';

  /** @type {Array<ScreenShareSource & { label: string, icon?: string }>} */
  const screenItems = [];
  /** @type {Array<ScreenShareSource & { label: string, icon?: string }>} */
  const windowItems = [];

  if (Array.isArray(options.sources) && options.sources.length > 0) {
    for (const s of options.sources) {
      const item = formatSourceItem(s);
      if (item.tab === 'screens' || item.kind === 'screen') {
        screenItems.push(item);
      } else {
        windowItems.push(item);
      }
    }
  } else {
    // Дефолтные элементы по спецификации Discord
    screenItems.push(
      { id: 'screen_dp6', label: 'Открыть доступ к «LG Electronics LG QHD (DP-6)»', tab: 'screens', icon: '🖥' },
      { id: 'screen_region', label: 'Открыть доступ к прямоугольной области', tab: 'screens', icon: '🖥' },
      { id: 'screen_virtual', label: 'Открыть доступ к новому виртуальному экрану', tab: 'screens', icon: '🖥' },
    );
    windowItems.push(
      { id: 'win_orca', label: 'Orca', icon: '🦦', tab: 'windows' },
      { id: 'win_firefox', label: 'Mozilla Firefox', icon: '🌐', tab: 'windows' },
      { id: 'win_chord', label: 'Discord / Chord', icon: '🎮', tab: 'windows' },
      { id: 'win_dolphin', label: 'Видеофайлы — Dolphin', icon: '📁', tab: 'windows' },
    );
  }

  // Асинхронный опрос источников через invoke при наличии
  const tauriInvoke = options.invoke || (typeof globalThis !== 'undefined' ? (/** @type {any} */ (globalThis).__TAURI__?.core?.invoke ?? (/** @type {any} */ (globalThis).__TAURI__?.invoke)) : null);
  if (typeof tauriInvoke === 'function' && (!options.sources || options.sources.length === 0)) {
    tauriInvoke('voice_desktop_sources')
      .then((/** @type {any} */ res) => {
        if (res && Array.isArray(res.sources) && res.sources.length > 0) {
          screenItems.length = 0;
          windowItems.length = 0;
          for (const s of res.sources) {
            const formatted = formatSourceItem(s);
            if (formatted.tab === 'screens' || formatted.kind === 'screen') {
              screenItems.push(formatted);
            } else {
              windowItems.push(formatted);
            }
          }
          renderPortalBody();
        }
      })
      .catch(() => {});
  }

  /** @type {Array<{ canvas: any, id: string }>} */
  const activeCanvases = [];
  let tick = 0;

  const renderPortalBody = () => {
    tabsBar.replaceChildren();
    for (const t of [
      { id: /** @type {const} */ ('screens'), label: '🖥 Экраны' },
      { id: /** @type {const} */ ('windows'), label: '🪟 Окна' },
    ]) {
      const active = portalTab === t.id;
      const b = doc.createElement('button');
      b.type = 'button';
      b.style.cssText =
        `padding:8px 12px;border:none;border-bottom:2px solid ${active ? '#3daee9' : 'transparent'};background:${active ? 'rgba(61,174,233,0.12)' : 'transparent'};color:${active ? '#f2f3f5' : '#949ba4'};font-size:13px;font-weight:700;cursor:pointer;`;
      b.textContent = t.label;
      b.addEventListener('click', () => {
        portalTab = t.id;
        renderPortalBody();
      });
      tabsBar.append(b);
    }

    contentArea.replaceChildren();
    activeCanvases.length = 0;
    const q = String(searchInput.value || '').trim().toLowerCase();

    const grid = doc.createElement('div');
    grid.style.cssText = 'display:grid;grid-template-columns:repeat(2,1fr);gap:10px;';

    const list = portalTab === 'screens' ? screenItems : windowItems;
    for (const item of list) {
      if (q && !item.label.toLowerCase().includes(q)) continue;
      const cardBtn = doc.createElement('button');
      cardBtn.type = 'button';
      cardBtn.setAttribute('data-portal-source-id', item.id);
      cardBtn.style.cssText =
        'display:flex;flex-direction:column;gap:8px;padding:10px;border-radius:8px;border:1.5px solid #3f4147;background:#2f323a;color:#f2f3f5;cursor:pointer;text-align:center;align-items:center;';

      const cv = doc.createElement('canvas');
      cv.width = 188;
      cv.height = 104;
      cv.style.cssText = 'width:100%;height:96px;border-radius:4px;background:#111214;display:block;';
      activeCanvases.push({ canvas: cv, id: item.id });
      drawCapturePreviewFrame(cv, item.id, tick, showWallpaper);

      const lbl = doc.createElement('div');
      lbl.style.cssText =
        'font-size:12px;font-weight:600;color:#f2f3f5;line-height:1.35;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;';
      lbl.textContent = item.label;
      cardBtn.append(cv, lbl);
      cardBtn.addEventListener('click', () => {
        closePortal();
        options.onSelectSource?.({
          id: item.id,
          label: item.label,
          tab: /** @type {'screens' | 'windows'} */ (portalTab),
        });
      });
      grid.append(cardBtn);
    }
    contentArea.append(grid);

    footerArea.replaceChildren();
    if (portalTab === 'screens') {
      const wpLabel = doc.createElement('label');
      wpLabel.style.cssText = 'display:flex;align-items:center;gap:8px;font-size:12px;color:#dbdee1;cursor:pointer;user-select:none;';
      const wpCheck = doc.createElement('input');
      wpCheck.type = 'checkbox';
      wpCheck.checked = showWallpaper;
      wpCheck.style.cssText = 'width:15px;height:15px;accent-color:#3daee9;cursor:pointer;';
      wpCheck.addEventListener('change', () => {
        showWallpaper = wpCheck.checked;
        for (const c of activeCanvases) {
          drawCapturePreviewFrame(c.canvas, c.id, tick, showWallpaper);
        }
      });
      const wpTxt = doc.createElement('span');
      wpTxt.textContent = 'Показывать обои рабочего стола';
      wpLabel.append(wpCheck, wpTxt);
      footerArea.append(wpLabel);
    } else {
      const hint = doc.createElement('span');
      hint.className = 'stream-portal-hint';
      hint.textContent = 'Выберите окно приложения для начала трансляции';
      footerArea.append(hint);
    }
  };

  searchInput.addEventListener('input', renderPortalBody);
  renderPortalBody();

  let lastPortalDraw = 0;
  /** @param {number} [timestamp] */
  const portalLoop = (timestamp = 0) => {
    if (portalRafId === null) return;
    const now = Number(timestamp) || (typeof performance !== 'undefined' ? performance.now() : Date.now());
    if (lastPortalDraw === 0) {
      lastPortalDraw = now;
    } else if (now - lastPortalDraw >= 150) {
      lastPortalDraw = now;
      tick += 1;
      for (const c of activeCanvases) {
        drawCapturePreviewFrame(c.canvas, c.id, tick, showWallpaper);
      }
    }
    if (typeof requestAnimationFrame === 'function') {
      portalRafId = requestAnimationFrame(portalLoop);
    }
  };

  if (typeof requestAnimationFrame === 'function') {
    portalRafId = requestAnimationFrame(portalLoop);
  }

  dialog.append(titleBar, searchBarWrap, tabsBar, contentArea, footerArea);
  backdrop.append(dialog);

  backdrop.addEventListener('click', (/** @type {any} */ ev) => {
    if (ev.target === backdrop) closePortal();
  });

  const mountTarget = doc.body || doc;
  if (typeof mountTarget.append === 'function') {
    mountTarget.append(backdrop);
  } else if (typeof mountTarget.appendChild === 'function') {
    mountTarget.appendChild(backdrop);
  }

  return {
    element: backdrop,
    dialog,
    close: closePortal,
    render: renderPortalBody,
  };
}

/**
 * Алиас для открытия портала выбора окна/экрана.
 *
 * @param {'screens' | 'windows'} initialPortalTab
 * @param {(picked: { id: string, label: string, tab: 'screens' | 'windows' }) => void} onSelectSource
 * @param {ScreenSharePortalOptions} [extraOptions]
 */
export function openScreenSharePortalPicker(initialPortalTab, onSelectSource, extraOptions = {}) {
  return createScreenSharePortalPicker({
    initialPortalTab,
    onSelectSource,
    ...extraOptions,
  });
}

/**
 * Создаёт полноценное модальное окно «Демонстрация экрана» (`createScreenShareModalController`).
 *
 * @param {any} [options]
 */
export function createScreenShareModalController(options = {}) {
  const doc = resolveDocument(options.doc);
  doc.getElementById?.('live-screenshare-modal')?.remove?.();

  const backdrop = doc.createElement('div');
  backdrop.id = 'live-screenshare-modal';
  backdrop.style.cssText =
    'position:fixed;inset:0;background:rgba(0,0,0,0.78);display:flex;align-items:center;justify-content:center;z-index:9999;';

  const card = doc.createElement('div');
  card.setAttribute('role', 'dialog');
  card.setAttribute('aria-modal', 'true');
  card.setAttribute('aria-label', 'Демонстрация экрана');
  card.style.cssText =
    'width:620px;max-width:95vw;background:#2b2d31;color:#f2f3f5;border:1px solid #3f4147;border-radius:12px;overflow:hidden;box-shadow:0 24px 64px rgba(0,0,0,0.88);display:flex;flex-direction:column;font-size:13px;font-family:inherit;';

  const cleanup = () => {
    if (typeof backdrop.remove === 'function') {
      backdrop.remove();
    }
    options.onClose?.();
  };

  let mainTab = options.initialTab === 'devices' ? 'devices' : 'apps';
  let appCaptureKind = options.initialKind === 'windows' ? 'windows' : 'screens';
  let qualityTier = options.qualityTier || 'SD';

  const topArea = doc.createElement('div');
  topArea.style.cssText = 'padding:18px 20px 16px;display:flex;flex-direction:column;gap:16px;';

  const segmentedWrap = doc.createElement('div');
  segmentedWrap.style.cssText =
    'display:grid;grid-template-columns:repeat(2,1fr);gap:4px;padding:4px;border-radius:10px;background:#1e1f22;';

  const centerContent = doc.createElement('div');
  centerContent.style.cssText = 'min-height:224px;display:flex;flex-direction:column;gap:12px;justify-content:center;';

  const bottomBar = doc.createElement('div');
  bottomBar.style.cssText =
    'display:flex;align-items:center;justify-content:space-between;gap:12px;padding:12px 20px;background:#232428;border-top:1px solid #35373c;';

  const renderModalState = () => {
    segmentedWrap.replaceChildren();
    for (const t of [
      { id: /** @type {const} */ ('apps'), icon: '🖥', label: 'Приложения' },
      { id: /** @type {const} */ ('devices'), icon: '📹', label: 'Устройства' },
    ]) {
      const active = mainTab === t.id;
      const b = doc.createElement('button');
      b.type = 'button';
      b.setAttribute('role', 'tab');
      b.setAttribute('aria-selected', active ? 'true' : 'false');
      b.style.cssText =
        `padding:8px 14px;border-radius:8px;border:none;background:${active ? '#35373c' : 'transparent'};color:${active ? '#f2f3f5' : '#949ba4'};font-size:13px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;gap:8px;`;
      b.innerHTML = `<span>${t.icon}</span><span>${t.label}</span>`;
      b.addEventListener('click', () => {
        mainTab = t.id;
        renderModalState();
      });
      segmentedWrap.append(b);
    }

    centerContent.replaceChildren();
    if (mainTab === 'apps') {
      const heading = doc.createElement('div');
      heading.style.cssText = 'font-size:15px;font-weight:700;color:#f2f3f5;';
      heading.textContent = 'Демонстрируйте экран, окно или приложение';

      const cardsGrid = doc.createElement('div');
      cardsGrid.style.cssText = 'display:grid;grid-template-columns:repeat(2,1fr);gap:12px;flex:1;';

      const makeCard = (
        /** @type {'screens' | 'windows'} */ kind,
        /** @type {string} */ label,
        /** @type {string} */ iconSvg,
      ) => {
        const isSel = appCaptureKind === kind;
        const btn = doc.createElement('button');
        btn.type = 'button';
        btn.setAttribute('data-capture-kind', kind);
        btn.style.cssText =
          `min-height:178px;border-radius:10px;border:1.5px solid ${isSel ? '#5865f2' : '#3f4147'};background:#313338;color:#f2f3f5;cursor:pointer;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;padding:20px;`;
        const circle = doc.createElement('span');
        circle.style.cssText =
          'width:54px;height:54px;border-radius:50%;background:#1e1f22;color:#dbdee1;display:inline-flex;align-items:center;justify-content:center;';
        circle.innerHTML = iconSvg;
        const txt = doc.createElement('span');
        txt.style.cssText = 'font-size:13px;font-weight:700;color:#f2f3f5;';
        txt.textContent = label;
        btn.append(circle, txt);
        btn.addEventListener('click', () => {
          appCaptureKind = kind;
          if (typeof options.onOpenPortal === 'function') {
            options.onOpenPortal(kind);
          } else {
            createScreenSharePortalPicker({
              doc,
              initialPortalTab: kind,
              onSelectSource: (src) => {
                cleanup();
                options.onStartStream?.(src);
              },
            });
          }
        });
        return btn;
      };

      cardsGrid.append(
        makeCard(
          'screens',
          'Демонстрировать весь экран',
          '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/></svg>',
        ),
        makeCard(
          'windows',
          'Демонстрировать окно',
          '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="16" rx="2"/></svg>',
        ),
      );
      centerContent.append(heading, cardsGrid);
    } else {
      const devHeading = doc.createElement('div');
      devHeading.style.cssText = 'font-size:15px;font-weight:700;color:#f2f3f5;margin-bottom:8px;';
      devHeading.textContent = 'Выберите видеоустройство для трансляции';
      const devGrid = doc.createElement('div');
      devGrid.style.cssText = 'display:grid;grid-template-columns:repeat(2,1fr);gap:12px;';
      const devices = [
        { id: 'cam_v4l2_0', label: 'USB HD Webcam (/dev/video0)' },
        { id: 'cam_pipewire', label: 'PipeWire Video Capture' },
      ];
      for (const d of devices) {
        const btn = doc.createElement('button');
        btn.type = 'button';
        btn.setAttribute('data-device-id', d.id);
        btn.textContent = d.label;
        btn.addEventListener('click', () => {
          cleanup();
          options.onStartStream?.({ id: d.id, label: d.label, tab: 'devices' });
        });
        devGrid.append(btn);
      }
      centerContent.append(devHeading, devGrid);
    }

    bottomBar.replaceChildren();
    const summary = doc.createElement('div');
    summary.textContent = qualityTier === 'HD' ? '1080p • 60fps' : '720p • 60fps';

    const rightControls = doc.createElement('div');
    const closeActionBtn = doc.createElement('button');
    closeActionBtn.type = 'button';
    closeActionBtn.textContent = 'Отмена';
    closeActionBtn.addEventListener('click', cleanup);
    rightControls.append(closeActionBtn);

    bottomBar.append(summary, rightControls);
  };

  renderModalState();
  topArea.append(segmentedWrap, centerContent);
  card.append(topArea, bottomBar);
  backdrop.append(card);

  backdrop.addEventListener('click', (/** @type {any} */ ev) => {
    if (ev.target === backdrop) cleanup();
  });

  const mountTarget = doc.body || doc;
  if (typeof mountTarget.append === 'function') {
    mountTarget.append(backdrop);
  } else if (typeof mountTarget.appendChild === 'function') {
    mountTarget.appendChild(backdrop);
  }

  return {
    element: backdrop,
    close: cleanup,
  };
}

/** Алиас для открытия основного модального окна стрима. */
export const openScreenShareModal = createScreenShareModalController;
