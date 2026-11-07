// @ts-check
/**
 * Режим ввода голоса: активация по голосу (VAD) и рация (push-to-talk).
 *
 * Модуль чистый: держит замороженный набор режимов, клампы порога VAD
 * (−100..0 дБ) и задержки отпускания PTT (0..2000 мс) с фолбэками на
 * −60 дБ / 200 мс, правило «передаю ли я сейчас звук» и русскую сводку
 * режима для панели настроек.
 */

/** Режимы ввода голоса. */
export const VOICE_INPUT_MODES = Object.freeze({
  VOICE_ACTIVITY: "VOICE_ACTIVITY",
  PUSH_TO_TALK: "PUSH_TO_TALK",
});

/** Порог VAD по умолчанию, дБ. */
export const DEFAULT_VAD_THRESHOLD_DB = -60;

/** Задержка отпускания PTT по умолчанию, мс. */
export const DEFAULT_PTT_RELEASE_DELAY_MS = 200;

/** Нижняя граница порога VAD, дБ. */
export const MIN_VAD_THRESHOLD_DB = -100;

/** Верхняя граница порога VAD, дБ. */
export const MAX_VAD_THRESHOLD_DB = 0;

/** Верхняя граница задержки отпускания PTT, мс. */
export const MAX_PTT_RELEASE_DELAY_MS = 2000;

/**
 * @typedef {{ mode?: string, vadThresholdDb?: number, pttReleaseDelayMs?: number }} VoiceInputConfig
 * @typedef {{ currentLevelDb?: number, pttPressed?: boolean | unknown, muted?: boolean, deafened?: boolean }} VoiceInputState
 */

/**
 * Нормализует −0 к +0, чтобы сравнения и вывод не зависели от знака нуля.
 * @param {number} value
 * @returns {number}
 */
function normalizeZero(value) {
  return value === 0 ? 0 : value;
}

/**
 * Зажимает порог VAD в −100..0 дБ с округлением до целого.
 * @param {unknown} [db]
 * @returns {number}
 */
export function clampVadThresholdDb(db) {
  if (typeof db !== "number" || !Number.isFinite(db)) {
    return DEFAULT_VAD_THRESHOLD_DB;
  }
  const rounded = Math.round(db);
  return normalizeZero(Math.min(MAX_VAD_THRESHOLD_DB, Math.max(MIN_VAD_THRESHOLD_DB, rounded)));
}

/**
 * Зажимает задержку отпускания PTT в 0..2000 мс с округлением до целого.
 * @param {unknown} [ms]
 * @returns {number}
 */
export function clampPttReleaseDelayMs(ms) {
  if (typeof ms !== "number" || !Number.isFinite(ms)) {
    return DEFAULT_PTT_RELEASE_DELAY_MS;
  }
  const rounded = Math.round(ms);
  return normalizeZero(Math.min(MAX_PTT_RELEASE_DELAY_MS, Math.max(0, rounded)));
}

/**
 * Передаёт ли пользователь звук прямо сейчас.
 *
 * `muted`/`deafened` глушат передачу в любом режиме. В PTT решает только
 * `pttPressed`; в VOICE_ACTIVITY — конечный `currentLevelDb` не ниже порога.
 * Неизвестный режим трактуется как VOICE_ACTIVITY.
 * @param {VoiceInputConfig | null | undefined} config
 * @param {VoiceInputState | null | undefined} state
 * @returns {boolean}
 */
export function isVoiceTransmitting(config, state) {
  const current = state ?? {};
  if (current.muted === true || current.deafened === true) {
    return false;
  }
  if (config?.mode === VOICE_INPUT_MODES.PUSH_TO_TALK) {
    return Boolean(current.pttPressed);
  }
  return (
    Number.isFinite(current.currentLevelDb) &&
    /** @type {number} */ (current.currentLevelDb) >= clampVadThresholdDb(config?.vadThresholdDb)
  );
}

/**
 * Русская сводка режима для панели настроек.
 * @param {VoiceInputConfig | null | undefined} config
 * @returns {string}
 */
export function formatVoiceInputSummaryRu(config) {
  if (config?.mode === VOICE_INPUT_MODES.PUSH_TO_TALK) {
    return "Режим рации (задержка " + clampPttReleaseDelayMs(config?.pttReleaseDelayMs) + " мс)";
  }
  return "Активация по голосу (" + clampVadThresholdDb(config?.vadThresholdDb) + " дБ)";
}
