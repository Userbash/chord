// @ts-check
/**
 * FE-P2-22 — guild role color and permission bitfield helper.
 *
 * Discord ships permissions as a decimal string bitfield that no longer fits in
 * a JS number (bits go past 53), so everything here works on BigInt. Role
 * colors arrive as `0xRRGGBB` integers with `0` meaning "no color"; the caller
 * falls back to the neutral `#99aab5`.
 */

/** @typedef {{ id: string, name: string, position: number, color: number }} GuildRole */

/** Discord permission bit positions used by the role editor. */
export const PERMISSION_FLAGS = Object.freeze({
  ADMINISTRATOR: 1n << 3n,
  MANAGE_CHANNELS: 1n << 4n,
  MANAGE_GUILD: 1n << 5n,
  VIEW_CHANNEL: 1n << 10n,
  SEND_MESSAGES: 1n << 11n,
  MANAGE_MESSAGES: 1n << 13n,
  ATTACH_FILES: 1n << 15n,
  CONNECT: 1n << 20n,
  SPEAK: 1n << 21n,
  MANAGE_THREADS: 1n << 34n,
});

/** Neutral role color used when a role has none. */
export const DEFAULT_ROLE_COLOR = '#99aab5';

const DECIMAL_RE = /^-?\d+$/;
const MAX_COLOR = 0xffffff;

/**
 * Parse a permission bitfield given as a decimal string or BigInt.
 * @param {unknown} value
 * @returns {bigint | null} `null` when the value is not a valid bitfield.
 */
function parseBitfield(value) {
  if (typeof value === 'bigint') return value;
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (!DECIMAL_RE.test(trimmed)) return null;
    try {
      return BigInt(trimmed);
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Check a single permission bit. ADMINISTRATOR implies every permission.
 * @param {unknown} bitfieldStrOrBigInt Decimal string or BigInt permission bitfield.
 * @param {bigint} flagBigInt One of {@link PERMISSION_FLAGS}.
 * @returns {boolean}
 */
export function hasPermissionBit(bitfieldStrOrBigInt, flagBigInt) {
  if (typeof flagBigInt !== 'bigint') return false;
  const bits = parseBitfield(bitfieldStrOrBigInt);
  if (bits === null) return false;
  if ((bits & PERMISSION_FLAGS.ADMINISTRATOR) !== 0n) return true;
  return (bits & flagBigInt) !== 0n;
}

/**
 * Convert a Discord role color integer to a CSS hex string.
 * @param {unknown} colorInt
 * @returns {string} `#rrggbb`, or {@link DEFAULT_ROLE_COLOR} when absent/invalid.
 */
export function intColorToHex(colorInt) {
  if (typeof colorInt !== 'number' || !Number.isInteger(colorInt)) return DEFAULT_ROLE_COLOR;
  if (colorInt <= 0 || colorInt > MAX_COLOR) return DEFAULT_ROLE_COLOR;
  return '#' + colorInt.toString(16).padStart(6, '0');
}

/**
 * Pick the highest positioned role that actually carries a color.
 * @param {ReadonlyArray<GuildRole> | null | undefined} roles
 * @returns {GuildRole | null}
 */
export function resolveHighestColoredRole(roles) {
  if (!Array.isArray(roles)) return null;
  let best = /** @type {GuildRole | null} */ (null);
  for (const role of roles) {
    if (!role || typeof role !== 'object') continue;
    if (!Number.isFinite(role.color) || role.color <= 0) continue;
    if (best === null || role.position > best.position) best = role;
  }
  return best;
}
