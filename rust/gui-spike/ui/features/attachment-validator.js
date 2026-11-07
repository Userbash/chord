// @ts-check
/**
 * Attachment limits, formatting, kind detection and batch validation for the
 * composer attachment picker.
 *
 * Limits are the Discord client limits: at most 10 files per message and at
 * most 25 MiB per file. Batches are also capped at 25 MiB in total, which
 * matches the composer gate used by the WebView.
 */

/** @type {number} Maximum number of files per message. */
export const MAX_ATTACHMENTS_PER_MESSAGE = 10;

/** @type {number} Maximum size of a single file in bytes (25 MiB). */
export const MAX_FILE_SIZE_BYTES = 25 * 1024 * 1024;

/** @type {string} Prefix Discord puts on spoiler attachments. */
export const SPOILER_PREFIX = 'SPOILER_';

/** @type {ReadonlySet<string>} */
const IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp']);

/** @type {ReadonlySet<string>} */
const VIDEO_EXTENSIONS = new Set(['mp4', 'webm', 'mov']);

/** @type {ReadonlySet<string>} */
const AUDIO_EXTENSIONS = new Set(['mp3', 'ogg', 'wav', 'flac']);

/**
 * Lowercase extension of a filename without the dot; empty when absent.
 * @param {string} filename
 * @returns {string}
 */
function extensionOf(filename) {
  const base = String(filename ?? '').split(/[\\/]/).pop() ?? '';
  const dot = base.lastIndexOf('.');
  if (dot <= 0 || dot === base.length - 1) return '';
  return base.slice(dot + 1).toLowerCase();
}

/**
 * Normalises a MIME type: lowercase, parameters stripped.
 * @param {string} [contentType]
 * @returns {string}
 */
function normalizeContentType(contentType) {
  return (String(contentType ?? '').split(';')[0] ?? '').trim().toLowerCase();
}

/**
 * Human readable size in Russian: bytes, kilobytes (1 decimal) or megabytes
 * (2 decimals). Negative and non-finite input is clamped to zero.
 * @param {number} bytes
 * @returns {string}
 */
export function formatFileSizeRu(bytes) {
  const safe = Number.isFinite(bytes) && bytes > 0 ? bytes : 0;
  if (safe < 1024) return `${Math.round(safe)} Б`;
  if (safe < 1024 * 1024) return `${(safe / 1024).toFixed(1)} КБ`;
  return `${(safe / (1024 * 1024)).toFixed(2)} МБ`;
}

/**
 * Kind of attachment used for preview rendering.
 * @param {string} filename
 * @param {string} [contentType]
 * @returns {'image' | 'video' | 'audio' | 'file'}
 */
export function classifyAttachmentKind(filename, contentType = '') {
  const mime = normalizeContentType(contentType);
  if (mime.startsWith('image/')) return 'image';
  if (mime.startsWith('video/')) return 'video';
  if (mime.startsWith('audio/')) return 'audio';

  const ext = extensionOf(filename);
  if (IMAGE_EXTENSIONS.has(ext)) return 'image';
  if (VIDEO_EXTENSIONS.has(ext)) return 'video';
  if (AUDIO_EXTENSIONS.has(ext)) return 'audio';
  return 'file';
}

/**
 * Whether the basename already carries the spoiler prefix.
 * @param {string} filename
 * @returns {boolean}
 */
export function isSpoilerFilename(filename) {
  const base = String(filename ?? '').split(/[\\/]/).pop() ?? '';
  return base.startsWith(SPOILER_PREFIX);
}

/**
 * Adds or removes the spoiler prefix on the basename, keeping any directory
 * part untouched.
 * @param {string} filename
 * @param {boolean} [spoiler] `true` adds the prefix, `false` removes it.
 * @returns {string}
 */
export function toggleSpoilerFilename(filename, spoiler = true) {
  const raw = String(filename ?? '');
  const sep = raw.search(/[\\/](?!.*[\\/])/);
  const dir = sep >= 0 ? raw.slice(0, sep + 1) : '';
  const base = sep >= 0 ? raw.slice(sep + 1) : raw;

  if (spoiler) {
    return dir + (base.startsWith(SPOILER_PREFIX) ? base : SPOILER_PREFIX + base);
  }
  return dir + (base.startsWith(SPOILER_PREFIX) ? base.slice(SPOILER_PREFIX.length) : base);
}

/**
 * @typedef {{ name: string, size: number }} AttachmentFile
 * @typedef {{
 *   valid: boolean,
 *   errorCode: 'TOO_MANY_FILES' | 'FILE_TOO_LARGE' | 'BATCH_TOO_LARGE' | null,
 *   totalBytes: number,
 * }} BatchValidation
 */

/**
 * Validates a pending attachment batch: file count, per-file size and total
 * size. `totalBytes` is always the sum of the supplied sizes.
 * @param {ReadonlyArray<AttachmentFile>} files
 * @returns {BatchValidation}
 */
export function validateAttachmentBatch(files) {
  const list = Array.isArray(files) ? files : [];

  /** @type {number} */
  let totalBytes = 0;
  for (const file of list) {
    const size = Number(file?.size);
    totalBytes += Number.isFinite(size) ? size : 0;
  }

  if (list.length > MAX_ATTACHMENTS_PER_MESSAGE) {
    return { valid: false, errorCode: 'TOO_MANY_FILES', totalBytes };
  }

  for (const file of list) {
    const size = Number(file?.size);
    if (!Number.isFinite(size) || size < 0 || size > MAX_FILE_SIZE_BYTES) {
      return { valid: false, errorCode: 'FILE_TOO_LARGE', totalBytes };
    }
  }

  if (totalBytes > MAX_FILE_SIZE_BYTES) {
    return { valid: false, errorCode: 'BATCH_TOO_LARGE', totalBytes };
  }

  return { valid: true, errorCode: null, totalBytes };
}
