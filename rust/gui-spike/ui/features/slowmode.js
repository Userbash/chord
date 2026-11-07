// @ts-check
/**
 * Slow mode (rate limit per user) helpers for the channel settings UI.
 * All values are Discord-compatible: 0..21600 seconds, integer.
 */

/** Maximum slow mode interval Discord accepts (6 hours). */
export const MAX_SLOWMODE_SECONDS = 21600;

/** Preset ladder shown in the slow mode picker. */
export const SLOWMODE_PRESETS_SECONDS = Object.freeze([0, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21600]);

/**
 * Clamp a numeric slow mode value into [0, MAX_SLOWMODE_SECONDS] and truncate to an integer.
 * Non-numeric and non-finite input defaults to 0.
 * @param {unknown} seconds
 * @returns {number}
 */
export function clampSlowmodeSeconds(seconds) {
  if (typeof seconds !== 'number' || !Number.isFinite(seconds)) return 0;
  if (seconds <= 0) return 0;
  if (seconds >= MAX_SLOWMODE_SECONDS) return MAX_SLOWMODE_SECONDS;
  return Math.trunc(seconds);
}

/**
 * Human readable Russian label for a slow mode interval.
 * @param {unknown} seconds
 * @returns {string}
 */
export function formatSlowmodeIntervalRu(seconds) {
  const clamped = clampSlowmodeSeconds(seconds);
  if (clamped === 0) return 'Медленный режим выключен';
  if (clamped < 60) return `${clamped} сек.`;
  if (clamped < 3600) return `${Math.floor(clamped / 60)} мин.`;
  return `${Math.floor(clamped / 3600)} ч.`;
}

/**
 * Seconds left until the user may post again in a slow mode channel.
 * @param {unknown} rateLimitPerUser slow mode interval in seconds
 * @param {number | null | undefined} [lastMessageSentAtMs] timestamp of the user's last message
 * @param {number | null | undefined} [nowMs] current timestamp
 * @param {boolean} [bypassSlowmode] user has bypass permission
 * @returns {number}
 */
export function computeSlowmodeRemainingSeconds(rateLimitPerUser, lastMessageSentAtMs, nowMs, bypassSlowmode = false) {
  if (bypassSlowmode === true) return 0;
  const rateLimit = clampSlowmodeSeconds(rateLimitPerUser);
  if (rateLimit === 0) return 0;
  if (typeof lastMessageSentAtMs !== "number" || !Number.isFinite(lastMessageSentAtMs) || lastMessageSentAtMs <= 0) return 0;
  const elapsedSec = Math.floor(Math.max(0, (Number(nowMs) || 0) - lastMessageSentAtMs) / 1000);
  return Math.max(0, rateLimit - elapsedSec);
}
