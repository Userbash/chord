// @ts-check
/**
 * Нормализация громкости голосового канала (task-211).
 *
 * Пользовательская громкость живёт в диапазоне 0..200 % (усиление до 2×),
 * громкость устройства — 0..100 %. Все входы из UI/состояния проходят через
 * clamp-функции: нечисловое значение откатывается к 100, дроби округляются к
 * целому. Множитель аудиоэлемента равен 0 при локальном mute или deaf.
 */

/** Минимальная пользовательская громкость, %. */
export const MIN_USER_VOLUME = 0;

/** Пользовательская громкость по умолчанию, %. */
export const DEFAULT_USER_VOLUME = 100;

/** Максимальная пользовательская громкость, % (усиление 2×). */
export const MAX_USER_VOLUME = 200;

/** Максимальная громкость устройства, %. */
export const MAX_DEVICE_VOLUME = 100;

/**
 * @param {unknown} value
 * @returns {boolean} true, если значение — конечное число
 */
function isFiniteNumber(value) {
  return typeof value === "number" && Number.isFinite(value);
}

/**
 * @param {number} value
 * @param {number} min
 * @param {number} max
 * @returns {number}
 */
function clamp(value, min, max) {
  return Math.min(max, Math.max(min, Math.round(value)));
}

/**
 * Приводит пользовательскую громкость к целому в 0..200; нечисловое — к 100.
 * @param {unknown} volumePercent
 * @returns {number}
 */
export function clampUserVolume(volumePercent) {
  if (!isFiniteNumber(volumePercent)) return DEFAULT_USER_VOLUME;
  return clamp(/** @type {number} */ (volumePercent), MIN_USER_VOLUME, MAX_USER_VOLUME);
}

/**
 * Приводит громкость устройства к целому в 0..100; нечисловое — к 100.
 * @param {unknown} volumePercent
 * @returns {number}
 */
export function clampDeviceVolume(volumePercent) {
  if (!isFiniteNumber(volumePercent)) return DEFAULT_USER_VOLUME;
  return clamp(/** @type {number} */ (volumePercent), MIN_USER_VOLUME, MAX_DEVICE_VOLUME);
}

/**
 * Множитель `audio.volume` для голосового потока: 0 при mute/deaf, иначе объём/100.
 * @param {unknown} volumePercent
 * @param {boolean} [localMuted]
 * @param {boolean} [deafened]
 * @returns {number}
 */
export function computeAudioGainMultiplier(volumePercent, localMuted = false, deafened = false) {
  if (localMuted === true || deafened === true) return 0;
  return clampUserVolume(volumePercent) / 100;
}

/**
 * Русская подпись громкости: «Без звука» при mute или 0 %, иначе «N%».
 * @param {unknown} volumePercent
 * @param {boolean} [localMuted]
 * @returns {string}
 */
export function formatVolumeLabelRu(volumePercent, localMuted = false) {
  const clamped = clampUserVolume(volumePercent);
  if (localMuted === true || clamped === 0) return "Без звука";
  return `${clamped}%`;
}
