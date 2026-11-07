// @ts-check
/**
 * Rich embed limits, safe URL validation and render normalization for the
 * WebView message renderer.
 *
 * Limits mirror the Discord client rich embed limits: at most 10 embeds per
 * message, 256 characters for a title or a field name, 4096 for a
 * description, 1024 for a field value, 2048 for a footer and at most 25
 * fields. URLs are only rendered when they use an absolute http(s) scheme;
 * everything else ("javascript:", "data:", "file:", relative paths) becomes
 * `null` so the renderer never creates a clickable link from it.
 */

/** @type {Readonly<{ MAX_EMBEDS_PER_MESSAGE: number, MAX_TITLE_CHARS: number, MAX_DESCRIPTION_CHARS: number, MAX_FIELDS: number, MAX_FIELD_NAME_CHARS: number, MAX_FIELD_VALUE_CHARS: number, MAX_FOOTER_CHARS: number }>} */
export const EMBED_LIMITS = Object.freeze({
  MAX_EMBEDS_PER_MESSAGE: 10,
  MAX_TITLE_CHARS: 256,
  MAX_DESCRIPTION_CHARS: 4096,
  MAX_FIELDS: 25,
  MAX_FIELD_NAME_CHARS: 256,
  MAX_FIELD_VALUE_CHARS: 1024,
  MAX_FOOTER_CHARS: 2048,
});

/** @type {string} Discord's default embed sidebar colour. */
const DEFAULT_EMBED_COLOR = '#2b2d31';

/** @type {number} Largest valid 24-bit RGB integer. */
const MAX_COLOR_INT = 0xffffff;

/**
 * True when the value is an absolute http(s) URL. Rejects dangerous schemes,
 * relative references, whitespace-only strings and non-strings.
 * @param {unknown} url
 * @returns {boolean}
 */
export function isSafeEmbedUrl(url) {
  if (typeof url !== 'string') return false;
  const trimmed = url.trim();
  if (trimmed === '') return false;
  /** @type {URL} */
  let parsed;
  try {
    parsed = new URL(trimmed);
  } catch {
    return false;
  }
  return parsed.protocol === 'https:' || parsed.protocol === 'http:';
}

/**
 * Truncates a value to `max` characters; non-strings become `''`.
 * @param {unknown} value
 * @param {number} max
 * @returns {string}
 */
function text(value, max) {
  if (typeof value !== 'string') return '';
  return value.length > max ? value.slice(0, max) : value;
}

/**
 * Maps an embed colour integer to `#rrggbb`, or the Discord default.
 * @param {unknown} color
 * @returns {string}
 */
function normalizedColor(color) {
  if (typeof color !== 'number' || !Number.isInteger(color)) return DEFAULT_EMBED_COLOR;
  if (color < 0 || color > MAX_COLOR_INT) return DEFAULT_EMBED_COLOR;
  return `#${color.toString(16).padStart(6, '0')}`;
}

/**
 * @typedef {object} NormalizedEmbedField
 * @property {string} name
 * @property {string} value
 * @property {boolean} inline
 *
 * @typedef {object} NormalizedEmbed
 * @property {string} title
 * @property {string} description
 * @property {string | null} url
 * @property {string} color
 * @property {NormalizedEmbedField[]} fields
 * @property {{ text: string } | null} footer
 */

/**
 * Normalizes a raw embed payload for rendering: clamps every text field to its
 * limit, drops unsafe URLs, converts the colour to hex and caps the field list.
 * Never mutates the input.
 * @param {unknown} rawEmbed
 * @returns {NormalizedEmbed}
 */
export function normalizeEmbedForRender(rawEmbed) {
  const raw =
    rawEmbed !== null && typeof rawEmbed === 'object'
      ? /** @type {Record<string, unknown>} */ (rawEmbed)
      : {};

  const rawFooter =
    raw.footer !== null && typeof raw.footer === 'object'
      ? /** @type {Record<string, unknown>} */ (raw.footer)
      : null;

  const rawFields = Array.isArray(raw.fields) ? raw.fields.slice(0, EMBED_LIMITS.MAX_FIELDS) : [];

  return {
    title: text(raw.title, EMBED_LIMITS.MAX_TITLE_CHARS),
    description: text(raw.description, EMBED_LIMITS.MAX_DESCRIPTION_CHARS),
    url: isSafeEmbedUrl(raw.url) ? String(raw.url).trim() : null,
    color: normalizedColor(raw.color),
    fields: rawFields.map((entry) => {
      const field =
        entry !== null && typeof entry === 'object'
          ? /** @type {Record<string, unknown>} */ (entry)
          : {};
      return {
        name: text(field.name, EMBED_LIMITS.MAX_FIELD_NAME_CHARS),
        value: text(field.value, EMBED_LIMITS.MAX_FIELD_VALUE_CHARS),
        inline: field.inline === true,
      };
    }),
    footer: rawFooter === null ? null : { text: text(rawFooter.text, EMBED_LIMITS.MAX_FOOTER_CHARS) },
  };
}
