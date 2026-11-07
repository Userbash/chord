// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  MAX_ATTACHMENTS_PER_MESSAGE,
  MAX_FILE_SIZE_BYTES,
  SPOILER_PREFIX,
  formatFileSizeRu,
  classifyAttachmentKind,
  isSpoilerFilename,
  toggleSpoilerFilename,
  validateAttachmentBatch,
} from '../../rust/gui-spike/ui/features/attachment-validator.js';

test('constants match Discord attachment limits', () => {
  assert.equal(MAX_ATTACHMENTS_PER_MESSAGE, 10);
  assert.equal(MAX_FILE_SIZE_BYTES, 25 * 1024 * 1024);
  assert.equal(MAX_FILE_SIZE_BYTES, 26214400);
  assert.equal(SPOILER_PREFIX, 'SPOILER_');
});

test('formatFileSizeRu clamps negative and non-finite input to zero', () => {
  assert.equal(formatFileSizeRu(-1), '0 Б');
  assert.equal(formatFileSizeRu(-1024 * 1024), '0 Б');
  assert.equal(formatFileSizeRu(NaN), '0 Б');
  assert.equal(formatFileSizeRu(Infinity), '0 Б');
  assert.equal(formatFileSizeRu(0), '0 Б');
});

test('formatFileSizeRu formats bytes below one kilobyte as integer bytes', () => {
  assert.equal(formatFileSizeRu(1), '1 Б');
  assert.equal(formatFileSizeRu(512), '512 Б');
  assert.equal(formatFileSizeRu(1023), '1023 Б');
});

test('formatFileSizeRu formats kilobytes with one decimal', () => {
  assert.equal(formatFileSizeRu(1024), '1.0 КБ');
  assert.equal(formatFileSizeRu(1536), '1.5 КБ');
  assert.equal(formatFileSizeRu(1024 * 1023), '1023.0 КБ');
});

test('formatFileSizeRu formats megabytes with two decimals', () => {
  assert.equal(formatFileSizeRu(1024 * 1024), '1.00 МБ');
  assert.equal(formatFileSizeRu(1024 * 1024 * 2.5), '2.50 МБ');
  assert.equal(formatFileSizeRu(26214400), '25.00 МБ');
});

test('classifyAttachmentKind detects image by extension and content type', () => {
  for (const name of ['a.png', 'b.jpg', 'c.jpeg', 'd.gif', 'e.webp', 'F.PNG']) {
    assert.equal(classifyAttachmentKind(name), 'image', name);
  }
  assert.equal(classifyAttachmentKind('blob', 'image/png'), 'image');
  assert.equal(classifyAttachmentKind('photo.bin', 'image/webp'), 'image');
});

test('classifyAttachmentKind detects video by extension and content type', () => {
  for (const name of ['a.mp4', 'b.webm', 'c.mov', 'D.MP4']) {
    assert.equal(classifyAttachmentKind(name), 'video', name);
  }
  assert.equal(classifyAttachmentKind('clip', 'video/mp4'), 'video');
});

test('classifyAttachmentKind detects audio by extension and content type', () => {
  for (const name of ['a.mp3', 'b.ogg', 'c.wav', 'd.flac', 'E.WAV']) {
    assert.equal(classifyAttachmentKind(name), 'audio', name);
  }
  assert.equal(classifyAttachmentKind('voice', 'audio/ogg'), 'audio');
});

test('classifyAttachmentKind falls back to file', () => {
  assert.equal(classifyAttachmentKind('archive.zip'), 'file');
  assert.equal(classifyAttachmentKind('no-extension'), 'file');
  assert.equal(classifyAttachmentKind('doc.pdf', 'application/pdf'), 'file');
  assert.equal(classifyAttachmentKind(''), 'file');
});

test('isSpoilerFilename checks the SPOILER_ prefix on the basename', () => {
  assert.equal(isSpoilerFilename('SPOILER_cat.png'), true);
  assert.equal(isSpoilerFilename('cat.png'), false);
  assert.equal(isSpoilerFilename(''), false);
  assert.equal(isSpoilerFilename('dir/SPOILER_cat.png'), true);
  assert.equal(isSpoilerFilename('SPOILER_cat.png'), true);
});

test('toggleSpoilerFilename adds the prefix by default', () => {
  assert.equal(toggleSpoilerFilename('cat.png'), 'SPOILER_cat.png');
  assert.equal(toggleSpoilerFilename('cat.png', true), 'SPOILER_cat.png');
  assert.equal(toggleSpoilerFilename('SPOILER_cat.png', true), 'SPOILER_cat.png');
  assert.equal(toggleSpoilerFilename('dir/cat.png'), 'dir/SPOILER_cat.png');
});

test('toggleSpoilerFilename removes the prefix when spoiler is false', () => {
  assert.equal(toggleSpoilerFilename('SPOILER_cat.png', false), 'cat.png');
  assert.equal(toggleSpoilerFilename('cat.png', false), 'cat.png');
  assert.equal(toggleSpoilerFilename('dir/SPOILER_cat.png', false), 'dir/cat.png');
});

test('validateAttachmentBatch accepts an empty batch', () => {
  assert.deepEqual(validateAttachmentBatch([]), {
    valid: true,
    errorCode: null,
    totalBytes: 0,
  });
});

test('validateAttachmentBatch accepts a valid mixed batch and sums bytes', () => {
  const result = validateAttachmentBatch([
    { name: 'a.png', size: 1024 },
    { name: 'b.mp4', size: 2048 },
  ]);
  assert.deepEqual(result, { valid: true, errorCode: null, totalBytes: 3072 });
});

test('validateAttachmentBatch rejects more than ten files', () => {
  const files = Array.from({ length: 11 }, (_, i) => ({ name: `f${i}.txt`, size: 1 }));
  const result = validateAttachmentBatch(files);
  assert.equal(result.valid, false);
  assert.equal(result.errorCode, 'TOO_MANY_FILES');
  assert.equal(result.totalBytes, 11);
});

test('validateAttachmentBatch accepts exactly ten files', () => {
  const files = Array.from({ length: 10 }, (_, i) => ({ name: `f${i}.txt`, size: 1 }));
  assert.deepEqual(validateAttachmentBatch(files), {
    valid: true,
    errorCode: null,
    totalBytes: 10,
  });
});

test('validateAttachmentBatch rejects a file above the per-file limit', () => {
  const result = validateAttachmentBatch([
    { name: 'ok.txt', size: 1 },
    { name: 'huge.bin', size: MAX_FILE_SIZE_BYTES + 1 },
  ]);
  assert.equal(result.valid, false);
  assert.equal(result.errorCode, 'FILE_TOO_LARGE');
});

test('validateAttachmentBatch accepts a file exactly at the limit', () => {
  assert.deepEqual(validateAttachmentBatch([{ name: 'big.bin', size: MAX_FILE_SIZE_BYTES }]), {
    valid: true,
    errorCode: null,
    totalBytes: MAX_FILE_SIZE_BYTES,
  });
});

test('validateAttachmentBatch rejects negative sizes', () => {
  const result = validateAttachmentBatch([{ name: 'bad.bin', size: -1 }]);
  assert.equal(result.valid, false);
  assert.equal(result.errorCode, 'FILE_TOO_LARGE');
});

test('validateAttachmentBatch rejects a batch whose total exceeds the limit', () => {
  const result = validateAttachmentBatch([
    { name: 'a.bin', size: 20 * 1024 * 1024 },
    { name: 'b.bin', size: 6 * 1024 * 1024 },
  ]);
  assert.equal(result.valid, false);
  assert.equal(result.errorCode, 'BATCH_TOO_LARGE');
  assert.equal(result.totalBytes, 27262976);
});

test('validateAttachmentBatch accepts a batch whose total is exactly the limit', () => {
  const result = validateAttachmentBatch([
    { name: 'a.bin', size: 12 * 1024 * 1024 },
    { name: 'b.bin', size: 13 * 1024 * 1024 },
  ]);
  assert.deepEqual(result, {
    valid: true,
    errorCode: null,
    totalBytes: MAX_FILE_SIZE_BYTES,
  });
});
