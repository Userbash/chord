// @ts-check
/**
 * Voice channel quality formatting helpers (RU).
 *
 * Pure functions only: no DOM, no IPC. The UI layer feeds them the payload from
 * the gateway (`voice_states` / `channel`), and they return ready-to-render labels
 * for the voice channel quality panel.
 */

/** Lower bound Discord accepts for a channel bitrate (bps). */
export const MIN_VOICE_BITRATE_BPS = 8000;
/** Default channel bitrate (64 kbit/s). */
export const DEFAULT_VOICE_BITRATE_BPS = 64000;
/** Upper bound Discord accepts for a channel bitrate (384 kbit/s). */
export const MAX_VOICE_BITRATE_BPS = 384000;

/** Russian labels for `video_quality_mode`. */
export const VIDEO_QUALITY_MODES_RU = Object.freeze({
  1: "Авто",
  2: "720p (Чёткое)",
});

/**
 * Clamp a channel bitrate into the legal Discord range.
 * Non-finite input falls back to {@link DEFAULT_VOICE_BITRATE_BPS}.
 *
 * @param {unknown} bps raw bitrate in bits per second
 * @returns {number} integer bitrate within [MIN_VOICE_BITRATE_BPS, MAX_VOICE_BITRATE_BPS]
 */
export function clampVoiceBitrateBps(bps) {
  const value = Number(bps);
  if (!Number.isFinite(value)) return DEFAULT_VOICE_BITRATE_BPS;
  return Math.min(MAX_VOICE_BITRATE_BPS, Math.max(MIN_VOICE_BITRATE_BPS, Math.round(value)));
}

/**
 * @typedef {object} VoiceQualityOptions
 * @property {number} [bitrateBps] channel bitrate in bps
 * @property {number} [userLimit] 0 or missing means "no limit"
 * @property {number} [connectedCount] members currently in the channel
 * @property {string | null} [rtcRegion] explicit RTC region, null/blank means automatic
 * @property {number} [videoQualityMode] 1 = auto, 2 = 720p
 */

/**
 * @typedef {object} VoiceQualityLabels
 * @property {string} bitrateLabel e.g. "64 кбит/с"
 * @property {string} capacityLabel e.g. "3/10" or "Без лимита"
 * @property {string} regionLabel region name or "Автоматический выбор"
 * @property {string} videoModeLabel "Авто" | "720p (Чёткое)"
 * @property {boolean} isFull true when the channel reached its user limit
 */

/**
 * Build the Russian labels for the voice channel quality panel.
 *
 * @param {VoiceQualityOptions} [options]
 * @returns {VoiceQualityLabels}
 */
export function formatVoiceChannelQualityRu(options = {}) {
  const { bitrateBps, userLimit, connectedCount, rtcRegion, videoQualityMode } = options;

  const limit = Math.trunc(Number(userLimit));
  const hasLimit = Number.isFinite(limit) && limit > 0;
  const connected = Math.max(0, Math.trunc(Number(connectedCount) || 0));
  const region = typeof rtcRegion === "string" ? rtcRegion.trim() : "";

  return {
    bitrateLabel: `${Math.round(clampVoiceBitrateBps(bitrateBps) / 1000)} кбит/с`,
    capacityLabel: hasLimit ? `${connected}/${limit}` : "Без лимита",
    regionLabel: region || "Автоматический выбор",
    videoModeLabel:
      VIDEO_QUALITY_MODES_RU[/** @type {1 | 2} */ (videoQualityMode)] || "Авто",
    isFull: hasLimit && connected >= limit,
  };
}
