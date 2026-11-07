// @ts-check
/**
 * task-290 (`RE-22-04`): чистая диагностика голосового соединения — пороги качества,
 * короткое имя RTC-сервера, разбор `Voice Privacy Code`, координаты спарклайна пинга,
 * русские строки попапа `RTCConnectionPopout` и состояние модалки `RTCDebugModal`.
 *
 * Оракул (read-only `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`):
 * - модуль `613681` (`offset 2469479`) — `RTCConnection.quality`:
 *   `ping > 500 || loss > 10` → `bad`, `ping > 250 || loss > 5` → `average`, иначе `fine`;
 *   буфер `_pings` до `199` точек (`2504200`), среднее по `20` отсчётам (`2474810`),
 *   таймаут пинга `4000` мс (`2504320`);
 * - модуль `978171` (`offset 7458500`) — попап: `getShortHostname` (`split(".")[0]`, `7459580`),
 *   `averagePing.toFixed(0)`/`lastPing`/`outboundLossRate.toFixed(1)` (`7459850..7460350`),
 *   пороги подсказки `badPing = 250` и `badLossRate = 10` (`7459160`), ширина `300px`;
 * - модуль `917592` (`offset 6025550`) — `Voice Privacy Code`: `30` цифр, сетка `6×5`;
 * - модуль `412780` (`offset 9264635`) — секции `RTCDebugStore` (`zWA`, 8 значений);
 * - `PopoutWindowRTCDebug` (`webpackId: 767675`) — окно отладки `854×480`.
 *
 * Модуль без DOM: только чистые функции и переносимое состояние контроллера,
 * поэтому проверяется в unit-тестах без шима документа.
 * @module ui/features/voice-diagnostics
 */

/** Верхняя граница «зелёного» пинга, мс (`RTCConnection.quality`). */
export const PING_FINE_MAX_MS = 250;

/** Верхняя граница «жёлтого» пинга, мс (`RTCConnection.quality`). */
export const PING_AVERAGE_MAX_MS = 500;

/** Верхняя граница «зелёной» потери исходящих пакетов, % (`RTCConnection.quality`). */
export const LOSS_FINE_MAX_PCT = 5;

/** Верхняя граница «жёлтой» потери исходящих пакетов, % (`RTCConnection.quality`). */
export const LOSS_AVERAGE_MAX_PCT = 10;

/** Ёмкость буфера `_pings`: `while (this._pings.length >= 200) this._pings.shift()`. */
export const PING_HISTORY_MAX_POINTS = 199;

/** Окно среднего пинга: `this._pings.slice(0, Math.min(this._pings.length, 20))`. */
export const PING_AVERAGE_WINDOW = 20;

/** Пинг-заглушка после трёх подряд таймаутов, мс (`_handlePingTimeout`). */
export const DEFAULT_PING_TIMEOUT_MS = 4000;

/** Ширина Canvas-спарклайна пинга (`<canvas width="258" height="80">`). */
export const SPARKLINE_WIDTH = 258;

/** Высота Canvas-спарклайна пинга. */
export const SPARKLINE_HEIGHT = 80;

/** Ширина попапа `RTCConnectionPopout`, px (`.container__50387`). */
export const POPOUT_WIDTH_PX = 300;

/** Ширина окна полной диагностики `PopoutWindowRTCDebug`, px. */
export const DEBUG_MODAL_WIDTH_PX = 854;

/** Высота окна полной диагностики `PopoutWindowRTCDebug`, px. */
export const DEBUG_MODAL_HEIGHT_PX = 480;

/** Секции модалки отладки в порядке `zWA` (`RTCDebugStore`, модуль `412780`). */
export const RTC_DEBUG_SECTIONS = Object.freeze(
  /** @type {const} */ ([
    'TRANSPORT',
    'OUTBOUND',
    'INBOUND',
    'SCREENSHARE',
    'CAMERA',
    'CLIPS',
    'STREAMS_OUTBOUND',
    'STREAMS_INBOUND',
  ]),
);

/**
 * @typedef {(typeof RTC_DEBUG_SECTIONS)[number]} RtcDebugSection
 * @typedef {'fine' | 'average' | 'bad' | 'unknown'} VoiceConnectionQuality
 * @typedef {'integration' | 'privacy'} VoiceDiagnosticsTab
 * @typedef {{ value?: unknown }} SparklineInputPoint
 * @typedef {{ x: number, y: number, value: number }} SparklinePoint
 */

/**
 * Входные данные для русских строк попапа (поля снимка `voice_diagnostics`).
 *
 * @typedef {{
 *   shortHostname?: string | null,
 *   averagePingMs?: number | null,
 *   lastPingMs?: number | null,
 *   outboundLossRatePct?: number | null,
 *   isEndToEndEncrypted?: boolean,
 *   secureFramesProtocolVersion?: number | null,
 * }} PopoutMetricsInput
 */

/**
 * Готовые строки попапа.
 *
 * @typedef {{
 *   headline: string,
 *   averagePingLabel: string,
 *   lastPingLabel: string,
 *   lossLabel: string,
 *   encryptionLabel: string,
 *   hasWarning: boolean,
 * }} PopoutMetrics
 */

/**
 * Снимок диагностики, который BFF отдаёт в попап.
 *
 * @typedef {{
 *   shortHostname?: string | null,
 *   averagePingMs?: number | null,
 *   lastPingMs?: number | null,
 *   outboundLossRatePct?: number | null,
 *   isEndToEndEncrypted?: boolean,
 *   secureFramesProtocolVersion?: number | null,
 *   privacyCode?: string | null,
 *   privacyCodeChunks?: ReadonlyArray<string> | null,
 *   pings?: ReadonlyArray<number | SparklineInputPoint> | null,
 * }} VoiceDiagnosticsSnapshot
 */

/**
 * Наблюдаемое состояние контроллера.
 *
 * @typedef {{
 *   isOpen: boolean,
 *   activeTab: VoiceDiagnosticsTab,
 *   isDebugModalOpen: boolean,
 *   activeDebugSection: RtcDebugSection,
 *   copiedPrivacyCode: boolean,
 *   snapshot: VoiceDiagnosticsSnapshot | null,
 * }} VoiceDiagnosticsState
 */

/**
 * Колбэки, которые контроллер вызывает наружу.
 *
 * @typedef {{
 *   onFetchDiagnostics?: () => void,
 *   onOpenDebugModal?: (section: RtcDebugSection) => void,
 *   onCopyPrivacyCode?: (code: string) => void,
 * }} VoiceDiagnosticsOptions
 */

/**
 * Событие клавиатуры в минимальном виде (совместимо с `KeyboardEvent`).
 *
 * @typedef {{
 *   key?: string,
 *   preventDefault?: () => void,
 * }} KeyboardLikeEvent
 */

/**
 * Классифицирует качество голосового соединения по последнему пингу и потерям
 * исходящих пакетов — точная копия геттера `RTCConnection.quality`.
 *
 * Контракт нормализации:
 * - `pingMs` принимается только числом; `null`, `undefined`, `NaN`, `""`, `-1`,
 *   строки и прочие типы → `"unknown"`;
 * - `outboundLossRatePct` опционален: `null`/`undefined` — потерь нет; если значение
 *   передано, оно приводится к числу и должно лежать в `[0..100]`, иначе → `"unknown"`
 *   (защита от битого снимка статистики);
 * - `> 500 мс` или `> 10%` → `"bad"`; `> 250 мс` или `> 5%` → `"average"`;
 *   иначе → `"fine"`.
 *
 * @param {unknown} pingMs
 * @param {unknown} [outboundLossRatePct]
 * @returns {VoiceConnectionQuality}
 */
export function evaluatePingQuality(pingMs, outboundLossRatePct) {
  if (typeof pingMs !== 'number' || !Number.isFinite(pingMs) || pingMs < 0) {
    return 'unknown';
  }

  /** @type {number | null} */
  let lossRatePct = null;
  if (outboundLossRatePct !== null && outboundLossRatePct !== undefined) {
    const parsedLoss = Number(outboundLossRatePct);
    if (!Number.isFinite(parsedLoss) || parsedLoss < 0 || parsedLoss > 100) {
      return 'unknown';
    }
    lossRatePct = parsedLoss;
  }

  if (pingMs > PING_AVERAGE_MAX_MS || (lossRatePct !== null && lossRatePct > LOSS_AVERAGE_MAX_PCT)) {
    return 'bad';
  }
  if (pingMs > PING_FINE_MAX_MS || (lossRatePct !== null && lossRatePct > LOSS_FINE_MAX_PCT)) {
    return 'average';
  }
  return 'fine';
}

/**
 * Короткое имя RTC-сервера: часть до первой точки или двоеточия
 * (`hostname.split(".")[0]` из `getShortHostname`).
 *
 * @param {unknown} endpoint
 * @returns {string}
 */
export function extractShortHostname(endpoint) {
  if (typeof endpoint !== 'string') {
    return '';
  }
  const trimmed = endpoint.trim();
  const match = /^[^.:]*/.exec(trimmed);
  return match ? match[0] ?? '' : '';
}

/**
 * Разбивает 30-значный `Voice Privacy Code` на 6 групп по 5 цифр (сетка `6×5`).
 * Пробельные символы между группами допускаются.
 *
 * @param {unknown} code
 * @returns {ReadonlyArray<string> | null}
 */
export function formatVoicePrivacyCodeChunks(code) {
  if (typeof code !== 'string') {
    return null;
  }
  const digits = code.replace(/\s+/g, '');
  if (!/^\d{30}$/.test(digits)) {
    return null;
  }
  /** @type {string[]} */
  const chunks = [];
  for (let index = 0; index < digits.length; index += 5) {
    chunks.push(digits.slice(index, index + 5));
  }
  return Object.freeze(chunks);
}

/**
 * Переводит историю пинга в координаты Canvas-спарклайна `258×80`.
 *
 * Оракул: `TimelineGraphView` (модули `229659` / `546738`) рисует временной ряд в
 * окне `60 000` мс на холсте `258×80`. Здесь — геометрия без времени: `x` растянут
 * на всю ширину, `y` инвертирован (меньше пинг — выше линия).
 *
 * @param {ReadonlyArray<number | SparklineInputPoint> | null | undefined} pings
 * @param {number} [width]
 * @param {number} [height]
 * @returns {SparklinePoint[]}
 */
export function computeSparklinePoints(pings, width = SPARKLINE_WIDTH, height = SPARKLINE_HEIGHT) {
  const safeWidth = Number.isFinite(width) && width > 0 ? width : SPARKLINE_WIDTH;
  const safeHeight = Number.isFinite(height) && height > 0 ? height : SPARKLINE_HEIGHT;

  /** @type {number[]} */
  const values = [];
  if (Array.isArray(pings)) {
    for (const entry of pings) {
      const raw =
        typeof entry === 'number'
          ? entry
          : entry && typeof entry === 'object'
            ? (/** @type {SparklineInputPoint} */ (entry).value ?? Number.NaN)
            : Number.NaN;
      const numeric = typeof raw === 'number' ? raw : Number.NaN;
      if (Number.isFinite(numeric) && numeric >= 0) {
        values.push(numeric);
      }
    }
  }

  const window = values.slice(-PING_HISTORY_MAX_POINTS);
  if (window.length === 0) {
    return [];
  }

  let min = window[0] ?? 0;
  let max = min;
  for (const value of window) {
    if (value < min) min = value;
    if (value > max) max = value;
  }
  const span = max - min;

  return window.map((value, index) => {
    const x = window.length === 1 ? 0 : (index / (window.length - 1)) * safeWidth;
    const y = span === 0 ? safeHeight / 2 : safeHeight - ((value - min) / span) * safeHeight;
    return {
      x: Math.min(Math.max(x, 0), safeWidth),
      y: Math.min(Math.max(y, 0), safeHeight),
      value,
    };
  });
}

/**
 * Собирает русские строки попапа `RTCConnectionPopout` по снимку диагностики.
 *
 * Формат повторяет ключи локализации `X58/lN`, `6iv2TF`, `VIBJM+`, `3BogKe`,
 * `ETIVvg` и подсказку `3pFz1P` (`badPing = 250`, `badLossRate = 10`).
 *
 * @param {PopoutMetricsInput | null | undefined} input
 * @returns {PopoutMetrics}
 */
export function formatPopoutMetricsRu(input) {
  /** @type {PopoutMetricsInput} */
  const source = input && typeof input === 'object' ? input : {};

  const shortHostname = typeof source.shortHostname === 'string' ? source.shortHostname.trim() : '';
  const averagePingMs = Math.round(Number(source.averagePingMs) || 0);
  const lastPingMs = Math.round(Number(source.lastPingMs) || 0);

  const rawLoss = source.outboundLossRatePct;
  const numericLoss = rawLoss === null || rawLoss === undefined ? Number.NaN : Number(rawLoss);
  const hasLoss = Number.isFinite(numericLoss);

  const isEncrypted = source.isEndToEndEncrypted === true;
  // secureFramesProtocolVersion описывает версию DAVE (0 — выкл, 1 — v1) и в строках
  // попапа не печатается: оракул показывает только баннер E2EE/SRTP.

  return {
    headline: shortHostname
      ? `Голосовая связь подключена — ${shortHostname}`
      : 'Голосовая связь подключена',
    averagePingLabel: `Средняя задержка: ${averagePingMs} мс`,
    lastPingLabel: `Последняя задержка: ${lastPingMs} мс`,
    lossLabel: hasLoss
      ? `Потеря исходящих пакетов: ${numericLoss.toFixed(1)}%`
      : 'Потеря исходящих пакетов: —',
    encryptionLabel: isEncrypted ? 'Защищено сквозным шифрованием' : 'Защищённое соединение',
    hasWarning:
      Number(source.lastPingMs) > PING_FINE_MAX_MS ||
      (hasLoss && numericLoss > LOSS_AVERAGE_MAX_PCT),
  };
}

/**
 * Проверяет, что строка — одна из секций модалки отладки.
 *
 * @param {unknown} value
 * @returns {RtcDebugSection | null}
 */
function toDebugSection(value) {
  if (typeof value !== 'string') {
    return null;
  }
  const normalized = value.trim().toUpperCase();
  for (const section of RTC_DEBUG_SECTIONS) {
    if (section === normalized) {
      return section;
    }
  }
  return null;
}

/**
 * Создаёт контроллер попапа диагностики и модалки отладки: чистое состояние без DOM.
 *
 * Поведение:
 * - `open()` идемпотентен и запрашивает снимок только при переходе «закрыт → открыт»;
 * - `close()` закрывает и попап, и модалку, сбрасывая `copiedPrivacyCode`;
 * - `Escape` закрывает сначала модалку, затем попап;
 * - `copyPrivacyCode()` копирует только валидный 30-значный код (из `privacyCodeChunks`
 *   или из `privacyCode`) и возвращает `false`, если кода нет.
 *
 * @param {VoiceDiagnosticsOptions} [options]
 */
export function createVoiceDiagnosticsController(options = {}) {
  const callbacks = options ?? {};

  /** @type {VoiceDiagnosticsState} */
  const state = {
    isOpen: false,
    activeTab: 'integration',
    isDebugModalOpen: false,
    activeDebugSection: 'TRANSPORT',
    copiedPrivacyCode: false,
    snapshot: null,
  };

  /** @returns {VoiceDiagnosticsState} */
  function getState() {
    return { ...state };
  }

  /** @returns {VoiceDiagnosticsState} */
  function open() {
    if (!state.isOpen) {
      state.isOpen = true;
      state.copiedPrivacyCode = false;
      if (typeof callbacks.onFetchDiagnostics === 'function') {
        callbacks.onFetchDiagnostics();
      }
    }
    return getState();
  }

  /** @returns {VoiceDiagnosticsState} */
  function close() {
    state.isOpen = false;
    state.isDebugModalOpen = false;
    state.copiedPrivacyCode = false;
    return getState();
  }

  /** @returns {VoiceDiagnosticsState} */
  function toggle() {
    return state.isOpen ? close() : open();
  }

  /**
   * @param {string} tab
   * @returns {VoiceDiagnosticsState}
   */
  function selectTab(tab) {
    if (tab === 'integration' || tab === 'privacy') {
      state.activeTab = tab;
    }
    return getState();
  }

  /**
   * @param {string} [section]
   * @returns {VoiceDiagnosticsState}
   */
  function openDebugModal(section) {
    const nextSection = toDebugSection(section);
    if (nextSection !== null) {
      state.activeDebugSection = nextSection;
    }
    state.isDebugModalOpen = true;
    if (typeof callbacks.onOpenDebugModal === 'function') {
      callbacks.onOpenDebugModal(state.activeDebugSection);
    }
    return getState();
  }

  /** @returns {VoiceDiagnosticsState} */
  function closeDebugModal() {
    state.isDebugModalOpen = false;
    return getState();
  }

  /** @returns {boolean} */
  function copyPrivacyCode() {
    /** @type {ReadonlyArray<string> | null} */
    let chunks = null;
    if (state.snapshot !== null) {
      const snapshotChunks = state.snapshot.privacyCodeChunks;
      if (Array.isArray(snapshotChunks) && snapshotChunks.length > 0) {
        chunks = Object.freeze(snapshotChunks.map(String));
      } else {
        chunks = formatVoicePrivacyCodeChunks(state.snapshot.privacyCode);
      }
    }

    if (chunks === null || chunks.length === 0) {
      state.copiedPrivacyCode = false;
      return false;
    }

    state.copiedPrivacyCode = true;
    if (typeof callbacks.onCopyPrivacyCode === 'function') {
      callbacks.onCopyPrivacyCode(chunks.join(''));
    }
    return true;
  }

  /**
   * Обрабатывает клавиатуру попапа: `Escape` закрывает модалку, затем попап.
   *
   * @param {KeyboardLikeEvent | null | undefined} event
   * @returns {boolean} `true`, если событие перехвачено.
   */
  function handleKeyDown(event) {
    if (!event || typeof event !== 'object') {
      return false;
    }
    const key = typeof event.key === 'string' ? event.key : '';
    if (key !== 'Escape' && key !== 'Esc') {
      return false;
    }
    if (state.isDebugModalOpen) {
      state.isDebugModalOpen = false;
    } else if (state.isOpen) {
      state.isOpen = false;
      state.copiedPrivacyCode = false;
    } else {
      return false;
    }

    if (typeof event.preventDefault === 'function') {
      event.preventDefault();
    }
    return true;
  }

  /**
   * Заменяет снимок диагностики и сбрасывает флаг «Скопировано».
   *
   * @param {VoiceDiagnosticsSnapshot | null | undefined} snapshot
   * @returns {VoiceDiagnosticsState}
   */
  function updateSnapshot(snapshot) {
    state.snapshot = snapshot && typeof snapshot === 'object' ? { ...snapshot } : null;
    state.copiedPrivacyCode = false;
    return getState();
  }

  return {
    open,
    close,
    toggle,
    selectTab,
    openDebugModal,
    closeDebugModal,
    copyPrivacyCode,
    handleKeyDown,
    updateSnapshot,
    getState,
  };
}

/**
 * Сводка состояния сквозного шифрования DAVE (MLS) и переходов эпох.
 *
 * @param {{
 *   protocolVersion?: number | null,
 *   transitionCount?: number | null,
 *   rosterSize?: number | null,
 *   timeToDaveGroupMs?: number | null
 * } | null | undefined} input
 * @returns {Readonly<{
 *   isActive: boolean,
 *   statusLabel: string,
 *   epochTransitionLabel: string,
 *   rosterLabel: string,
 *   joinLatencyLabel: string
 * }>}
 */
export function formatDaveTransitionSummary(input) {
  const protocolVersion = input && typeof input === 'object' ? input.protocolVersion : null;
  const transitionCount = input && typeof input === 'object' ? input.transitionCount : null;
  const rosterSize = input && typeof input === 'object' ? input.rosterSize : null;
  const timeToDaveGroupMs = input && typeof input === 'object' ? input.timeToDaveGroupMs : null;

  const isActive = Number.isInteger(protocolVersion) && /** @type {number} */ (protocolVersion) > 0;
  const statusLabel = isActive
    ? `DAVE v${protocolVersion} (MLS)`
    : 'Транспортное шифрование (SRTP)';

  const epochTransitionLabel =
    Number.isFinite(transitionCount) && /** @type {number} */ (transitionCount) >= 0
      ? `Переходов эпох: ${Math.trunc(/** @type {number} */ (transitionCount))}`
      : 'Переходов эпох: —';

  const rosterLabel =
    Number.isFinite(rosterSize) && /** @type {number} */ (rosterSize) >= 0
      ? `Участников MLS: ${Math.trunc(/** @type {number} */ (rosterSize))}`
      : 'Участников MLS: —';

  const joinLatencyLabel =
    Number.isFinite(timeToDaveGroupMs) && /** @type {number} */ (timeToDaveGroupMs) >= 0
      ? `Вход в DAVE-группу: ${Math.round(/** @type {number} */ (timeToDaveGroupMs))} мс`
      : 'Вход в DAVE-группу: —';

  return Object.freeze({
    isActive,
    statusLabel,
    epochTransitionLabel,
    rosterLabel,
    joinLatencyLabel,
  });
}

/**
 * Имена событий аналитики/буфера обмена при копировании проверочного кода E2EE.
 */
export const E2EE_VERIFICATION_COPY_EVENTS = Object.freeze({
  USER: 'e2ee_user_verification_code_copied',
  CALL: 'e2ee_call_verification_code_copied',
  STREAM: 'e2ee_stream_verification_code_copied',
});

/**
 * Сводка состояния настройки постоянных кодов сквозного шифрования (SecureFramesPersistedStore).
 *
 * @param {{
 *   persistentCodesEnabled?: unknown,
 *   uploadedKeyVersions?: ReadonlyArray<unknown> | null
 * } | null | undefined} input
 * @returns {Readonly<{
 *   enabled: boolean,
 *   uploadedVersions: ReadonlyArray<number>,
 *   statusLabel: string
 * }>}
 */
export function formatPersistentCodesSettingState(input) {
  const enabled = Boolean(
    input && typeof input === 'object' && /** @type {Record<string, unknown>} */ (input).persistentCodesEnabled === true,
  );

  /** @type {number[]} */
  const uploadedVersions = [];
  const rawList =
    input && typeof input === 'object' && Array.isArray(/** @type {Record<string, unknown>} */ (input).uploadedKeyVersions)
      ? /** @type {ReadonlyArray<unknown>} */ (input.uploadedKeyVersions)
      : null;

  if (rawList) {
    const seen = new Set();
    for (const v of rawList) {
      if (typeof v === 'number' && Number.isInteger(v) && v >= 0 && v <= 65535 && !seen.has(v)) {
        seen.add(v);
        uploadedVersions.push(v);
      }
    }
  }

  const statusLabel = enabled
    ? 'Постоянные коды проверки включены'
    : 'Постоянные коды проверки отключены';

  return Object.freeze({
    enabled,
    uploadedVersions: Object.freeze(uploadedVersions),
    statusLabel,
  });
}

/**
 * task-347: Выбирает текст/копирайт экрана верификационного кода (Persistent vs Ephemeral).
 *
 * RE-GAP-27 §5.4-5.5, §6.2-6.3, V-27-L1-07:
 * - оба ключа persistent -> mode "both_persistent", i18nKey "FJN+kh"
 * - только currentUserKey -> mode "current_persistent_only", i18nKey "p/9PGp"
 * - только otherUserKey -> mode "other_persistent_only", i18nKey "qT5z87"
 * - иначе -> mode "both_ephemeral", i18nKey "6JLy+i"
 *
 * @param {{ isCurrentUserKeyPersistent?: boolean, isOtherUserKeyPersistent?: boolean }} [options]
 * @returns {Readonly<{
 *   mode: 'both_persistent' | 'current_persistent_only' | 'other_persistent_only' | 'both_ephemeral',
 *   i18nKey: string
 * }>}
 */
export function selectVerificationCodeScreenCopy({ isCurrentUserKeyPersistent, isOtherUserKeyPersistent } = {}) {
  const isCurrent = isCurrentUserKeyPersistent === true;
  const isOther = isOtherUserKeyPersistent === true;

  if (isCurrent && isOther) {
    return Object.freeze({ mode: 'both_persistent', i18nKey: 'FJN+kh' });
  }
  if (isCurrent) {
    return Object.freeze({ mode: 'current_persistent_only', i18nKey: 'p/9PGp' });
  }
  if (isOther) {
    return Object.freeze({ mode: 'other_persistent_only', i18nKey: 'qT5z87' });
  }
  return Object.freeze({ mode: 'both_ephemeral', i18nKey: '6JLy+i' });
}

/**
 * task-347: Форматирует бакет относительного времени подтверждённого ключа / верификации.
 *
 * RE-GAP-27 §5.4-5.5, §6.2-6.3, V-27-L1-08:
 * Константы: MINUTE = 60, HOUR = 3600, DAY = 86400, DAYS_30 = 2592000, YEAR_12X30 = 12 * 2592000.
 * Строгие знаки `>` и Math.round для деления.
 *
 * @param {number} elapsedSeconds
 * @returns {Readonly<{
 *   unit: 'years' | 'months' | 'weeks' | 'days' | 'hours' | 'minutes' | 'seconds',
 *   i18nKey: string,
 *   count: number
 * }>}
 */
export function formatVerifiedRelativeTimeBucket(elapsedSeconds) {
  const MINUTE = 60;
  const HOUR = 3600;
  const DAY = 86400;
  const DAYS_30 = 2592000;
  const YEAR_12X30 = 12 * 2592000;

  const n = elapsedSeconds;

  if (n > YEAR_12X30) {
    return Object.freeze({ unit: 'years', i18nKey: 'F1wqkD', count: Math.round(n / YEAR_12X30) });
  }
  if (n > DAYS_30) {
    return Object.freeze({ unit: 'months', i18nKey: 'iT+b+2', count: Math.round(n / DAYS_30) });
  }
  if (n > 7 * DAY) {
    return Object.freeze({ unit: 'weeks', i18nKey: 'dLurKZ', count: Math.round(n / (7 * DAY)) });
  }
  if (n > DAY) {
    return Object.freeze({ unit: 'days', i18nKey: 'LE8a2H', count: Math.round(n / DAY) });
  }
  if (n > HOUR) {
    return Object.freeze({ unit: 'hours', i18nKey: 'KULxVS', count: Math.round(n / HOUR) });
  }
  if (n > MINUTE) {
    return Object.freeze({ unit: 'minutes', i18nKey: 'ws6rWq', count: Math.round(n / MINUTE) });
  }
  return Object.freeze({ unit: 'seconds', i18nKey: '/w0Qpw', count: Number.isFinite(n) ? n : 0 });
}

/**
 * task-356: Форматирует читаемый код верификации Secure Frames (RE-GAP-28 §1.2 & §2).
 * Оракул: модуль 277515 (экспорт DB / useReadableSecureFramesCode).
 *
 * @param {Uint8Array} bytes
 * @param {number} [desiredLength=30]
 * @param {number} [groupSize=5]
 * @returns {Readonly<{
 *   rawDigits: string,
 *   chunks: readonly string[],
 *   formatted: string
 * }>}
 */
export function formatReadableVerificationCode(bytes, desiredLength = 30, groupSize = 5) {
  if (!(bytes instanceof Uint8Array)) {
    throw new TypeError('bytes must be a Uint8Array');
  }
  if (bytes.byteLength < desiredLength) {
    throw new Error('data.byteLength must be greater than or equal to desiredLength');
  }
  if (desiredLength % groupSize !== 0) {
    throw new Error('desiredLength must be a multiple of groupSize');
  }
  if (groupSize > 8) {
    throw new Error('groupSize must be less than or equal to 8');
  }

  const modulus = BigInt(10 ** groupSize);
  /** @type {string[]} */
  const chunks = [];
  let rawDigits = '';

  for (let offset = 0; offset < desiredLength; offset += groupSize) {
    let blockValue = 0n;
    for (let r = groupSize; r > 0; --r) {
      const byte = bytes[offset + (groupSize - r)];
      if (byte === undefined) {
        throw new Error('Out of bounds access from data array');
      }
      blockValue = (blockValue << 8n) | BigInt(byte);
    }
    blockValue %= modulus;
    const chunkStr = blockValue.toString().padStart(groupSize, '0');
    chunks.push(chunkStr);
    rawDigits += chunkStr;
  }

  return Object.freeze({
    rawDigits,
    chunks: Object.freeze(chunks),
    formatted: chunks.join(' '),
  });
}

/**
 * task-369: Оракул модуля 259374 (useShouldShowVerificationCode, RE-GAP-26 §4.2 / RE-GAP-28 §2).
 * Возвращает true тогда и только тогда, когда для указанного канала следует показывать
 * код верификации Secure Frames:
 * 1) params != null && typeof params === 'object'
 * 2) typeof params.channelId === 'string' && params.channelId.length > 0
 * 3) params.channelId === params.activeVoiceChannelId
 * 4) params.isStageVoiceChannel !== true
 * 5) typeof params.secureFramesVersion === 'number' && Number.isFinite(params.secureFramesVersion) && params.secureFramesVersion !== 0
 *
 * @param {Readonly<{
 *   channelId?: string | null | undefined,
 *   activeVoiceChannelId?: string | null | undefined,
 *   isStageVoiceChannel?: boolean | null | undefined,
 *   secureFramesVersion?: number | null | undefined,
 * }> | null | undefined} params
 * @returns {boolean}
 */
export function shouldShowVerificationCodeForChannel(params) {
  if (params == null || typeof params !== 'object') {
    return false;
  }
  if (typeof params.channelId !== 'string' || params.channelId.length === 0) {
    return false;
  }
  if (params.channelId !== params.activeVoiceChannelId) {
    return false;
  }
  if (params.isStageVoiceChannel === true) {
    return false;
  }
  if (
    typeof params.secureFramesVersion !== 'number' ||
    !Number.isFinite(params.secureFramesVersion) ||
    params.secureFramesVersion === 0
  ) {
    return false;
  }
  return true;
}

