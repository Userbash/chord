// @ts-check
/**
 * task-244: чистые хелперы UI вложений — лимиты, категория MIME и размер по-русски.
 * Без DOM и без сети: модуль пригоден и для WebView, и для unit-тестов.
 */

/** Максимум файлов в одном сообщении (лимит Discord). */
export const MAX_ATTACHMENTS_PER_MESSAGE = 10;

/** Стандартный лимит размера файла — 25 МиБ. */
export const MAX_STANDARD_FILE_BYTES = 25 * 1024 * 1024;

/** Категории вложений для рендера и валидации. */
export const ATTACHMENT_CATEGORIES = Object.freeze({
  IMAGE: 'image',
  VIDEO: 'video',
  AUDIO: 'audio',
  DOCUMENT: 'document',
});

/**
 * Категория вложения по MIME-типу.
 * @param {string} mimeType MIME-тип из браузера или API.
 * @returns {string} Одно из значений ATTACHMENT_CATEGORIES.
 */
export function classifyAttachmentMime(mimeType) {
  const normalized = String(mimeType ?? '').trim().toLowerCase();
  if (normalized.startsWith('image/')) return ATTACHMENT_CATEGORIES.IMAGE;
  if (normalized.startsWith('video/')) return ATTACHMENT_CATEGORIES.VIDEO;
  if (normalized.startsWith('audio/')) return ATTACHMENT_CATEGORIES.AUDIO;
  return ATTACHMENT_CATEGORIES.DOCUMENT;
}

/**
 * Размер вложения для человека: байты, килобайты или мегабайты.
 * @param {number} bytes Размер в байтах.
 * @returns {string} Строка вида "512 Б", "1.5 КБ", "25.00 МБ".
 */
export function formatAttachmentSizeRu(bytes) {
  const safeBytes = Math.max(0, Math.trunc(Number(bytes) || 0));
  if (safeBytes < 1024) return `${safeBytes} Б`;
  if (safeBytes < 1024 * 1024) return `${(safeBytes / 1024).toFixed(1)} КБ`;
  return `${(safeBytes / (1024 * 1024)).toFixed(2)} МБ`;
}

/**
 * Проверка пачки файлов перед отправкой сообщения.
 * @param {Array<{ name: string, size: number }>} files Выбранные файлы.
 * @param {number} [maxBytes] Лимит на один файл в байтах.
 * @returns {{ valid: boolean, errorRu: string | null, totalBytes?: number }} Результат проверки.
 */
export function validateAttachmentsBatch(files, maxBytes = MAX_STANDARD_FILE_BYTES) {
  const list = Array.isArray(files) ? files : [];
  if (list.length === 0) {
    return { valid: false, errorRu: 'Нет файлов' };
  }
  if (list.length > MAX_ATTACHMENTS_PER_MESSAGE) {
    return { valid: false, errorRu: 'Максимум 10 файлов' };
  }

  let totalBytes = 0;
  for (const file of list) {
    const size = Number(file?.size) || 0;
    if (size <= 0 || size > maxBytes) {
      return { valid: false, errorRu: 'Файл превышает лимит размера' };
    }
    totalBytes += size;
  }

  return { valid: true, errorRu: null, totalBytes };
}
