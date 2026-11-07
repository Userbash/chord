// @ts-check
/**
 * task-244: контракт UI-хелпера вложений (чистая логика, без DOM).
 * Оракул — числа и строки из ТЗ, а не текущая реализация.
 */
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  MAX_ATTACHMENTS_PER_MESSAGE,
  MAX_STANDARD_FILE_BYTES,
  ATTACHMENT_CATEGORIES,
  classifyAttachmentMime,
  formatAttachmentSizeRu,
  validateAttachmentsBatch,
} from '../../rust/gui-spike/ui/features/attachment-helper.js';

test('константы лимитов совпадают с лимитами Discord', () => {
  assert.equal(MAX_ATTACHMENTS_PER_MESSAGE, 10);
  assert.equal(MAX_STANDARD_FILE_BYTES, 25 * 1024 * 1024);
  assert.equal(MAX_STANDARD_FILE_BYTES, 26214400);
});

test('ATTACHMENT_CATEGORIES заморожен и содержит 4 категории', () => {
  assert.equal(Object.isFrozen(ATTACHMENT_CATEGORIES), true);
  assert.deepEqual({ ...ATTACHMENT_CATEGORIES }, {
    IMAGE: 'image',
    VIDEO: 'video',
    AUDIO: 'audio',
    DOCUMENT: 'document',
  });
  assert.throws(() => {
    // @ts-expect-error — намеренная запись в замороженный объект
    ATTACHMENT_CATEGORIES.IMAGE = 'picture';
  }, TypeError);
  assert.equal(ATTACHMENT_CATEGORIES.IMAGE, 'image');
});

test('classifyAttachmentMime: префиксы image/video/audio, остальное document', () => {
  assert.equal(classifyAttachmentMime('image/png'), 'image');
  assert.equal(classifyAttachmentMime('IMAGE/PNG'), 'image');
  assert.equal(classifyAttachmentMime('  image/jpeg  '), 'image');
  assert.equal(classifyAttachmentMime('video/mp4'), 'video');
  assert.equal(classifyAttachmentMime(' Audio/Ogg '), 'audio');
  assert.equal(classifyAttachmentMime('application/pdf'), 'document');
  assert.equal(classifyAttachmentMime('text/plain; charset=utf-8'), 'document');
  assert.equal(classifyAttachmentMime(''), 'document');
  assert.equal(classifyAttachmentMime('imagex/png'), 'document');
  assert.equal(classifyAttachmentMime('multipart/related'), 'document');
});

test('formatAttachmentSizeRu: байты (< 1024)', () => {
  assert.equal(formatAttachmentSizeRu(0), '0 Б');
  assert.equal(formatAttachmentSizeRu(1), '1 Б');
  assert.equal(formatAttachmentSizeRu(999), '999 Б');
  assert.equal(formatAttachmentSizeRu(1023), '1023 Б');
  assert.equal(formatAttachmentSizeRu(1023.9), '1023 Б');
  assert.equal(formatAttachmentSizeRu(-5), '0 Б');
  assert.equal(formatAttachmentSizeRu(NaN), '0 Б');
});

test('formatAttachmentSizeRu: килобайты (< 1 МиБ)', () => {
  assert.equal(formatAttachmentSizeRu(1024), '1.0 КБ');
  assert.equal(formatAttachmentSizeRu(1536), '1.5 КБ');
  assert.equal(formatAttachmentSizeRu(1024 * 1024 - 1), '1024.0 КБ');
});

test('formatAttachmentSizeRu: мегабайты (>= 1 МиБ)', () => {
  assert.equal(formatAttachmentSizeRu(1024 * 1024), '1.00 МБ');
  assert.equal(formatAttachmentSizeRu(1.5 * 1024 * 1024), '1.50 МБ');
  assert.equal(formatAttachmentSizeRu(MAX_STANDARD_FILE_BYTES), '25.00 МБ');
});

test('validateAttachmentsBatch: пустой список — ошибка', () => {
  const result = validateAttachmentsBatch([]);
  assert.equal(result.valid, false);
  assert.equal(result.errorRu, 'Нет файлов');
});

test('validateAttachmentsBatch: больше 10 файлов — ошибка даже при малых размерах', () => {
  const files = Array.from({ length: 11 }, (_, i) => ({ name: `f${i}.png`, size: 1 }));
  const result = validateAttachmentsBatch(files);
  assert.equal(result.valid, false);
  assert.equal(result.errorRu, 'Максимум 10 файлов');
});

test('validateAttachmentsBatch: ровно 10 файлов допустимо, totalBytes суммируется', () => {
  const files = Array.from({ length: 10 }, (_, i) => ({ name: `f${i}.png`, size: 100 }));
  const result = validateAttachmentsBatch(files);
  assert.equal(result.valid, true);
  assert.equal(result.errorRu, null);
  assert.equal(result.totalBytes, 1000);
});

test('validateAttachmentsBatch: нулевой размер — ошибка', () => {
  const result = validateAttachmentsBatch([{ name: 'a.png', size: 0 }]);
  assert.equal(result.valid, false);
  assert.equal(result.errorRu, 'Файл превышает лимит размера');
});

test('validateAttachmentsBatch: отрицательный размер — ошибка', () => {
  const result = validateAttachmentsBatch([{ name: 'a.png', size: -1 }]);
  assert.equal(result.valid, false);
  assert.equal(result.errorRu, 'Файл превышает лимит размера');
});

test('validateAttachmentsBatch: превышение стандартного лимита 25 МиБ и граница ровно 25 МиБ', () => {
  const over = validateAttachmentsBatch([
    { name: 'ok.png', size: 10 },
    { name: 'big.zip', size: MAX_STANDARD_FILE_BYTES + 1 },
  ]);
  assert.equal(over.valid, false);
  assert.equal(over.errorRu, 'Файл превышает лимит размера');

  const edge = validateAttachmentsBatch([{ name: 'big.zip', size: MAX_STANDARD_FILE_BYTES }]);
  assert.equal(edge.valid, true);
  assert.equal(edge.errorRu, null);
  assert.equal(edge.totalBytes, MAX_STANDARD_FILE_BYTES);
});

test('validateAttachmentsBatch: явный maxBytes перекрывает стандартный лимит', () => {
  const result = validateAttachmentsBatch([{ name: 'a.bin', size: 51 }], 50);
  assert.equal(result.valid, false);
  assert.equal(result.errorRu, 'Файл превышает лимит размера');

  const ok = validateAttachmentsBatch([{ name: 'a.bin', size: 50 }], 50);
  assert.equal(ok.valid, true);
  assert.equal(ok.totalBytes, 50);
});

test('validateAttachmentsBatch: порядок проверок — сначала количество, затем размеры', () => {
  const files = [
    ...Array.from({ length: 10 }, (_, i) => ({ name: `f${i}.png`, size: 1 })),
    { name: 'big.zip', size: MAX_STANDARD_FILE_BYTES + 1 },
  ];
  const result = validateAttachmentsBatch(files);
  assert.equal(result.valid, false);
  assert.equal(result.errorRu, 'Максимум 10 файлов');
});
